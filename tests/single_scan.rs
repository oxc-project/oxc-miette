#![cfg(feature = "fancy-no-backtrace")]
#![expect(
    clippy::cast_possible_truncation,
    reason = "deterministic fuzz inputs are tightly bounded"
)]
//! Differential fuzz tests for the graphical renderer's span-reading paths.
//!
//! `render_snippets` reads each label's span either through a shared
//! `SpanScanner` scan (when the source exposes its backing buffer) or through
//! one `SourceCode::read_span` call per label, and a `NamedSource` carries a
//! scan memo from one report to the next. Every combination — buffered or
//! not, memo warm or cold — must render byte-identical reports. See each
//! test's own doc comment for its matrix.

use std::{fmt, sync::Arc};

use miette::{
    Diagnostic, GraphicalReportHandler, GraphicalTheme, LabeledSpan, Labels, MietteError,
    MietteSpanContents, NamedSource, SourceCode, SourceSpan,
};

/// Deterministic xorshift so any failure reproduces from a fixed seed.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Wraps a `NamedSource` but hides its backing buffer, forcing the
/// renderer down the one-`read_span`-per-label path.
#[derive(Debug)]
struct Opaque(NamedSource<String>);

impl SourceCode for Opaque {
    fn read_span<'a>(
        &'a self,
        span: &SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<MietteSpanContents<'a>, MietteError> {
        self.0.read_span(span, context_lines_before, context_lines_after)
    }

    fn name(&self) -> Option<&str> {
        SourceCode::name(&self.0)
    }
}

#[derive(Debug)]
struct TestDiag<S> {
    src: S,
    labels: Vec<LabeledSpan>,
}

impl<S> fmt::Display for TestDiag<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("test diagnostic")
    }
}

impl<S: fmt::Debug> std::error::Error for TestDiag<S> {}

impl<S: SourceCode + fmt::Debug> Diagnostic for TestDiag<S> {
    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(&self.src)
    }

    fn labels(&self) -> Labels {
        Labels::Many(self.labels.clone())
    }
}

fn render(diagnostic: &dyn Diagnostic, context_lines: usize) -> (fmt::Result, String) {
    let handler = GraphicalReportHandler::new_themed(GraphicalTheme::none())
        .with_context_lines(context_lines);
    let mut out = String::new();
    let result = handler.render_report(&mut out, diagnostic);
    (result, out)
}

/// Line-ending mixes the fuzzers sweep: LF-only, CRLF, lone-CR, and
/// multibyte text around all three.
const ALPHABETS: &[&[&str]] = &[
    &["a", "\n"],
    &["a", "b", "c", "\n"],
    &["a", "b", "\r\n"],
    &["a", "\r", "\n", "\r\n"],
    &["x", "y", "\n", "é", "🦀", "\r\n"],
];

/// A random source of up to 23 segments drawn from `alpha`.
fn random_source(rng: &mut Rng, alpha: &[&str]) -> String {
    let n = rng.below(24);
    let mut src = String::new();
    for _ in 0..n {
        src.push_str(alpha[rng.below(alpha.len())]);
    }
    src
}

/// 1–4 random labels over `src`, snapped to char boundaries: rendering
/// (unlike `read_span`) slices the data as UTF-8 text.
fn random_labels(rng: &mut Rng, src: &str, context_lines: usize) -> Vec<LabeledSpan> {
    let floor = |mut byte: usize| {
        byte = byte.min(src.len());
        while byte > 0 && !src.is_char_boundary(byte) {
            byte -= 1;
        }
        byte
    };
    (0..=rng.below(4))
        .map(|i| {
            // One in six labels lands just past EOF, making the whole render
            // fail; every rendering path must agree on that too.
            let offset =
                if rng.below(6) == 0 { src.len() + 1 } else { floor(rng.below(src.len() + 1)) };
            let mut len = floor(offset + [0, 1, 2, 6][rng.below(4)]).saturating_sub(offset);
            // A zero-length span at offset 0 with zero context makes
            // `read_span` return the window `[0, 1)` (a pre-existing quirk of
            // its saturating span-end arithmetic), which need not be valid
            // UTF-8 — rendering panics on every path alike. Widen such spans
            // to the first char.
            if context_lines == 0 && offset == 0 && len == 0 {
                len = src.chars().next().map_or(0, char::len_utf8);
            }
            let label = Some(format!("label {i}"));
            let span = (offset as u32, len as u32);
            if rng.below(4) == 0 {
                LabeledSpan::new_primary_with_span(label, span)
            } else {
                LabeledSpan::new_with_span(label, span)
            }
        })
        .collect()
}

/// `render_snippets` has two ways to read spans — a shared
/// `SpanScanner` scan when the source
/// exposes [`SourceCode::contiguous_bytes`], and one `read_span` per
/// lookup otherwise. Rendering the same diagnostic through both must
/// produce byte-identical reports (or fail identically, for labels past
/// the end of the source), across LF / CRLF / lone-CR and multibyte
/// sources, 1–4 labels of every overlap shape, and all context sizes.
#[test]
#[cfg_attr(
    miri,
    ignore = "renders thousands of graphical diagnostics to differentially fuzz two safe \
              code paths — Miri finds no UB here and interpreting the renders is far too slow; \
              both paths are exercised under Miri by the normal snapshot tests"
)]
fn buffer_and_read_span_paths_render_identically() {
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    let mut checked = 0usize;
    for alpha in ALPHABETS {
        for _ in 0..300 {
            let src = random_source(&mut rng, alpha);
            for context_lines in 0..=2 {
                let labels = random_labels(&mut rng, &src, context_lines);

                let with_buffer = TestDiag {
                    src: NamedSource::new("fuzz.rs", src.clone()),
                    labels: labels.clone(),
                };
                let without_buffer = TestDiag {
                    src: Opaque(NamedSource::new("fuzz.rs", src.clone())),
                    labels: labels.clone(),
                };
                let (buffer_result, buffer_out) = render(&with_buffer, context_lines);
                let (read_span_result, read_span_out) = render(&without_buffer, context_lines);
                assert_eq!(
                    (buffer_result, &buffer_out),
                    (read_span_result, &read_span_out),
                    "diverged for src={src:?} context_lines={context_lines} labels={labels:?}"
                );
                checked += 1;
            }
        }
    }
    assert!(checked >= 4000, "expected a broad sweep, only checked {checked}");
}

/// A batch render must be indistinguishable from per-report renders.
/// [`GraphicalReportHandler::render_reports`] shares one line scan across
/// consecutive reports over the same backing buffer; whatever the sharing
/// state, each yielded report must be byte-identical to a standalone
/// [`GraphicalReportHandler::render_report`] of the same diagnostic — across
/// LF / CRLF / lone-CR and multibyte sources, spans in any order (backwards
/// ones fall off the memoized path), sources switching mid-batch (which
/// resets the sharing), diagnostics with no source at all, and labels past
/// EOF (both renders must fail identically).
#[test]
#[cfg_attr(
    miri,
    ignore = "renders thousands of graphical diagnostics to differentially fuzz two safe \
              code paths — Miri finds no UB here and interpreting the renders is far too slow; \
              the batched path is exercised under Miri by \
              `batch_shares_scan_across_reports`"
)]
fn batched_renders_match_sequential_renders() {
    let mut rng = Rng(0x6C62_272E_07BB_0142);
    for alpha in ALPHABETS {
        for _ in 0..150 {
            let src = random_source(&mut rng, alpha);
            let other_src = random_source(&mut rng, alpha);
            let context_lines = rng.below(3);
            let shared = Arc::new(NamedSource::new("fuzz.rs", src.clone()));

            let count = 2 + rng.below(3);
            let mut diagnostics: Vec<TestDiag<Arc<NamedSource<String>>>> =
                std::iter::repeat_with(|| TestDiag {
                    src: Arc::clone(&shared),
                    labels: random_labels(&mut rng, &src, context_lines),
                })
                .take(count)
                .collect();
            // Sometimes switch buffers mid-batch, which must reset the
            // sharing rather than resume the wrong source's scan.
            let interloper = TestDiag {
                src: Arc::new(NamedSource::new("other.rs", other_src.clone())),
                labels: random_labels(&mut rng, &other_src, context_lines),
            };
            if rng.below(2) == 0 {
                let at = rng.below(diagnostics.len());
                diagnostics.insert(at, interloper);
            }

            let refs: Vec<&dyn Diagnostic> =
                diagnostics.iter().map(|d| d as &dyn Diagnostic).collect();
            let handler = GraphicalReportHandler::new_themed(GraphicalTheme::none())
                .with_context_lines(context_lines);
            let batched: Vec<(fmt::Result, String)> = handler.render_reports(&refs).collect();
            for (diagnostic, batched) in refs.iter().zip(&batched) {
                let mut out = String::new();
                let result = handler.render_report(&mut out, *diagnostic);
                assert_eq!(
                    (result, &out),
                    (batched.0, &batched.1),
                    "diverged for src={src:?} context_lines={context_lines}"
                );
            }
        }
    }
}

/// Three ascending reports and one backwards report over a shared source:
/// each batch item resumes (or falls off) the shared scan and must match its
/// standalone render. Small deterministic twin of
/// `batched_renders_match_sequential_renders` that also runs under Miri.
#[test]
fn batch_shares_scan_across_reports() {
    let src = "zero\none\ntwo\nthree\nfour\nfive\n";
    let shared = Arc::new(NamedSource::new("shared.rs", src.to_string()));
    let diagnostics: Vec<TestDiag<Arc<NamedSource<String>>>> =
        [(5u32, 3u32), (13, 5), (24, 4), (9, 3)]
            .into_iter()
            .map(|span| TestDiag {
                src: Arc::clone(&shared),
                labels: vec![LabeledSpan::new_with_span(Some("here".into()), span)],
            })
            .collect();
    let refs: Vec<&dyn Diagnostic> = diagnostics.iter().map(|d| d as &dyn Diagnostic).collect();

    let handler = GraphicalReportHandler::new_themed(GraphicalTheme::none()).with_context_lines(1);
    for (diagnostic, (batched_result, batched_out)) in
        refs.iter().zip(handler.render_reports(&refs))
    {
        let mut out = String::new();
        let result = handler.render_report(&mut out, *diagnostic);
        assert_eq!((result, out), (batched_result, batched_out));
    }
}
