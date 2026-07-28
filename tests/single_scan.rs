#![cfg(feature = "fancy-no-backtrace")]
#![expect(
    clippy::cast_possible_truncation,
    reason = "deterministic fuzz inputs are tightly bounded"
)]
//! Differential fuzz test for the graphical renderer's two span-reading paths.
//!
//! `render_snippets` reads each label's span either through a shared
//! `SpanScanner` scan (when the source exposes its backing buffer) or through
//! one `SourceCode::read_span` call per label. Both paths must render
//! byte-identical reports. See the test's own doc comment for the full matrix.

use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

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
    let alphabets: &[&[&str]] = &[
        &["a", "\n"],
        &["a", "b", "c", "\n"],
        &["a", "b", "\r\n"],
        &["a", "\r", "\n", "\r\n"],
        &["x", "y", "\n", "é", "🦀", "\r\n"],
    ];
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    let mut checked = 0usize;
    for alpha in alphabets {
        for _ in 0..300 {
            let n = rng.below(24);
            let mut src = String::new();
            for _ in 0..n {
                src.push_str(alpha[rng.below(alpha.len())]);
            }
            // Snap span edges to char boundaries: rendering (unlike
            // `read_span`) slices the data as UTF-8 text.
            let floor = |mut byte: usize| {
                byte = byte.min(src.len());
                while byte > 0 && !src.is_char_boundary(byte) {
                    byte -= 1;
                }
                byte
            };
            for context_lines in 0..=2 {
                let labels: Vec<LabeledSpan> = (0..=rng.below(4))
                    .map(|i| {
                        // One in six labels lands just past EOF, making
                        // the whole render fail; both paths must agree on
                        // that too.
                        let offset = if rng.below(6) == 0 {
                            src.len() + 1
                        } else {
                            floor(rng.below(src.len() + 1))
                        };
                        let mut len =
                            floor(offset + [0, 1, 2, 6][rng.below(4)]).saturating_sub(offset);
                        // A zero-length span at offset 0 with zero
                        // context makes `read_span` return the window
                        // `[0, 1)` (a pre-existing quirk of its
                        // saturating span-end arithmetic), which need not
                        // be valid UTF-8 — rendering panics on both paths
                        // alike. Widen such spans to the first char.
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
                    .collect();

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

#[test]
fn batch_matches_standalone_renders_in_any_order() {
    let source =
        Arc::new(NamedSource::new("shared.rs", "zero\none\ntwo\nthree\nfour\nfive\n".to_string()));
    let diagnostics: Vec<TestDiag<Arc<NamedSource<String>>>> =
        [(24u32, 4u32), (5, 3), (19, 4), (9, 3)]
            .into_iter()
            .map(|span| TestDiag {
                src: Arc::clone(&source),
                labels: vec![LabeledSpan::new_with_span(Some("here".into()), span)],
            })
            .collect();

    let handler = GraphicalReportHandler::new_themed(GraphicalTheme::none());
    let diagnostic_refs: Vec<&dyn Diagnostic> =
        diagnostics.iter().map(|diagnostic| diagnostic as &dyn Diagnostic).collect();
    let batched = handler.render_reports(source.as_ref(), &diagnostic_refs);
    for (diagnostic, batched) in diagnostics.iter().zip(batched) {
        let mut standalone = String::new();
        let standalone_result = handler.render_report(&mut standalone, diagnostic);
        assert_eq!(batched, (standalone_result, standalone));
    }
}

#[derive(Debug)]
struct SwitchingSource {
    second: AtomicBool,
}

impl SwitchingSource {
    const FIRST: &'static str = "zero\nfirst\nend\n";
    const SECOND: &'static str = "zero\nsecond\nend\n";

    fn current(&self) -> &'static str {
        if self.second.load(Ordering::Relaxed) { Self::SECOND } else { Self::FIRST }
    }
}

impl SourceCode for SwitchingSource {
    fn read_span<'a>(
        &'a self,
        span: &SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<MietteSpanContents<'a>, MietteError> {
        self.current().read_span(span, context_lines_before, context_lines_after)
    }

    fn name(&self) -> Option<&str> {
        Some("switch.rs")
    }

    fn contiguous_bytes(&self) -> Option<&[u8]> {
        Some(self.current().as_bytes())
    }
}

#[derive(Debug)]
struct SourceLessDiag {
    labels: Vec<LabeledSpan>,
}

impl fmt::Display for SourceLessDiag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("test diagnostic")
    }
}

impl std::error::Error for SourceLessDiag {}

impl Diagnostic for SourceLessDiag {
    fn labels(&self) -> Labels {
        Labels::Many(self.labels.clone())
    }
}

#[test]
fn a_new_batch_observes_a_mutable_sources_current_buffer() {
    let source = SwitchingSource { second: AtomicBool::new(false) };
    let diagnostic =
        SourceLessDiag { labels: vec![LabeledSpan::new_with_span(Some("here".into()), (5, 1))] };
    let handler = GraphicalReportHandler::new_themed(GraphicalTheme::none());

    {
        let mut first_batch = handler.begin_batch(&source);
        let mut first = String::new();
        first_batch.render_report(&mut first, &diagnostic).unwrap();
        assert!(first.contains("first"));

        source.second.store(true, Ordering::Relaxed);
        let mut same_batch = String::new();
        first_batch.render_report(&mut same_batch, &diagnostic).unwrap();
        assert!(same_batch.contains("first"), "a batch renders one coherent source snapshot");
    }

    let mut second_batch = handler.begin_batch(&source);
    let mut second = String::new();
    second_batch.render_report(&mut second, &diagnostic).unwrap();
    assert!(second.contains("second"), "a later batch must not reuse stale scan state");
}
