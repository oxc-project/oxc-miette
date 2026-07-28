#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::print_stderr,
    reason = "benchmark fixtures are bounded and progress output is intentional"
)]

//! Benchmarks for miette's diagnostic rendering pipeline.
//!
//! These mirror how oxc actually consumes this crate: oxlint and oxfmt build a
//! [`GraphicalReportHandler`] with its 400-column width and single context line,
//! attach an `Arc<NamedSource<_>>` to each diagnostic, and call
//! [`GraphicalReportHandler::render_report`]. The cases below match common
//! `OxcDiagnostic` shapes emitted by oxlint's `no-unused-vars` rule.
//!
//! The fixtures are real-world TypeScript/JSX files pulled from
//! <https://github.com/oxc-project/benchmark-files> (pinned to a fixed
//! revision). They are downloaded once and cached under `target/` so that
//! neither the repository nor the published crate carries multi-megabyte blobs.

use std::{fs, path::PathBuf, sync::Arc, time::Duration};

use criterion::{
    BatchSize, BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main,
};
use miette::{
    Diagnostic, Error, GraphicalReportHandler, GraphicalTheme, LabeledSpan, MietteDiagnostic,
    NamedSource, Severity, SourceCode, SourceSpan,
};

/// Pinned revision of <https://github.com/oxc-project/benchmark-files>, so the
/// inputs (and therefore CodSpeed instruction counts) stay reproducible.
const BENCHMARK_FILES_REV: &str = "c61afec7dd5a66cd5ecfc57ac74cf00687a0ca39";

/// Fixtures fetched from `benchmark-files`, spanning a range of sizes.
const FIXTURES: &[&str] = &[
    "RadixUIAdoptionSection.jsx", // small  (~2.5 KB)
    "cal.com.tsx",                // large  (~1.0 MB)
    "cal.com.ts",                 // xlarge (~1.4 MB)
];

/// Labels are placed near the end of each file to exercise the forward scan
/// miette performs to locate a span's line and column.
const SPAN_FRACTION: f64 = 0.9;
/// Distance between the declaration and assignment labels.
const RELATED_SPAN_DELTA: usize = 250;

struct Fixture {
    name: &'static str,
    source: Arc<NamedSource<String>>,
    source_len: usize,
    declaration_span: SourceSpan,
    assignment_span: SourceSpan,
}

struct DiagnosticCase {
    name: &'static str,
    build: fn(&Fixture) -> Error,
}

const DIAGNOSTIC_CASES: &[DiagnosticCase] = &[
    DiagnosticCase { name: "declared", build: declared_diagnostic },
    DiagnosticCase { name: "assigned", build: assigned_diagnostic },
];

/// oxlint's interactive output: unicode, color, and terminal hyperlinks.
fn terminal_handler() -> GraphicalReportHandler {
    GraphicalReportHandler::new().with_theme(GraphicalTheme::unicode()).with_links(true)
}

/// oxlint's piped/CI output: ASCII, no color, and a textual documentation URL.
fn ci_handler() -> GraphicalReportHandler {
    GraphicalReportHandler::new().with_theme(GraphicalTheme::none()).with_links(false)
}

/// Download `name` from `benchmark-files` (once) and cache it under `target/`.
/// The cache is keyed by revision, so bumping [`BENCHMARK_FILES_REV`] re-fetches.
fn load_fixture(name: &'static str) -> Fixture {
    let cache_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("benchmark-files")
        .join(BENCHMARK_FILES_REV);
    let cached = cache_dir.join(name);

    let source = fs::read_to_string(&cached).unwrap_or_else(|_| {
        let url = format!(
            "https://cdn.jsdelivr.net/gh/oxc-project/benchmark-files@{BENCHMARK_FILES_REV}/{name}"
        );
        eprintln!("downloading benchmark fixture `{name}` from {url}");
        let source = ureq::get(&url)
            .call()
            .unwrap_or_else(|err| panic!("failed to download {url}: {err}"))
            .body_mut()
            .read_to_string()
            .unwrap_or_else(|err| panic!("failed to read {url}: {err}"));
        fs::create_dir_all(&cache_dir)
            .unwrap_or_else(|err| panic!("failed to create {}: {err}", cache_dir.display()));
        fs::write(&cached, &source)
            .unwrap_or_else(|err| panic!("failed to write {}: {err}", cached.display()));
        source
    });

    let source_len = source.len();
    let declaration_offset = (source_len as f64 * SPAN_FRACTION) as usize;
    let declaration_span = identifier_span_at(&source, declaration_offset);
    let remaining = source_len - declaration_span.offset() as usize;
    let assignment_offset =
        declaration_span.offset() as usize + RELATED_SPAN_DELTA.min(remaining / 2);
    let assignment_span = identifier_span_at(&source, assignment_offset);
    let source = Arc::new(NamedSource::new(name, source));

    Fixture { name, source, source_len, declaration_span, assignment_span }
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$')
}

fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'$')
}

/// Find an identifier-shaped token at or immediately after `offset`.
fn identifier_span_at(source: &str, offset: usize) -> SourceSpan {
    let bytes = source.as_bytes();
    let offset = offset.min(bytes.len().saturating_sub(1));
    let at = (offset..bytes.len())
        .find(|&index| is_identifier_start(bytes[index]))
        .or_else(|| (0..offset).rev().find(|&index| is_identifier_start(bytes[index])))
        .expect("benchmark fixtures contain identifiers");

    let mut start = at;
    while start > 0 && is_identifier_byte(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = at + 1;
    while end < bytes.len() && is_identifier_byte(bytes[end]) {
        end += 1;
    }

    (start as u32, (end - start) as u32).into()
}

/// A one-label warning representative of most oxlint diagnostics.
fn declared_diagnostic(fixture: &Fixture) -> Error {
    declared_diagnostic_with_source(fixture, Arc::clone(&fixture.source))
}

/// The one-label warning against a freshly created source, whose embedded
/// scan cache is necessarily empty.
fn cold_declared_diagnostic(fixture: &Fixture) -> Error {
    let source = Arc::new(NamedSource::new(fixture.name, fixture.source.inner().clone()));
    declared_diagnostic_with_source(fixture, source)
}

fn declared_diagnostic_with_source(fixture: &Fixture, source: Arc<NamedSource<String>>) -> Error {
    let diagnostic = lint_diagnostic(
        "Variable 'resolve' is declared but never used.",
        "Consider removing this declaration.",
    )
    .with_label(LabeledSpan::new_with_span(
        Some("'resolve' is declared here".to_string()),
        fixture.declaration_span,
    ));

    Error::new(diagnostic).with_source_code(source)
}

/// The two-label form emitted when `no-unused-vars` sees a later assignment.
fn assigned_diagnostic(fixture: &Fixture) -> Error {
    let diagnostic = lint_diagnostic(
        "Variable 'resolve' is assigned a value but never used.",
        "Did you mean to use this variable?",
    )
    .with_labels([
        LabeledSpan::new_with_span(
            Some("'resolve' is declared here".to_string()),
            fixture.declaration_span,
        ),
        LabeledSpan::new_with_span(
            Some("it was last assigned here".to_string()),
            fixture.assignment_span,
        ),
    ]);

    Error::new(diagnostic).with_source_code(Arc::clone(&fixture.source))
}

/// Add the rule metadata that `LintContext` attaches to every oxlint diagnostic.
fn lint_diagnostic(message: &str, help: &str) -> MietteDiagnostic {
    MietteDiagnostic::new(message)
        .with_severity(Severity::Warning)
        .with_code("eslint(no-unused-vars)")
        .with_url("https://oxc.rs/docs/guide/usage/linter/rules/eslint/no-unused-vars.html")
        .with_help(help)
}

fn bench(c: &mut Criterion) {
    let fixtures: Vec<Fixture> = FIXTURES.iter().copied().map(load_fixture).collect();

    // `SourceCode::read_span` is the "find the line/column for this span" scan
    // that every rendered snippet depends on — the hot path optimized in #211
    // and #212. A context of 1 line matches the renderer's default.
    let mut group = c.benchmark_group("read_span");
    for fixture in &fixtures {
        group.throughput(Throughput::Bytes(fixture.source_len as u64));
        group.bench_function(BenchmarkId::from_parameter(fixture.name), |b| {
            b.iter(|| {
                let contents = fixture
                    .source
                    .inner()
                    .read_span(black_box(&fixture.declaration_span), 1, 1)
                    .expect("span within source");
                black_box(contents);
            });
        });
    }
    group.finish();

    // The render benches below reuse one `NamedSource` across iterations, so
    // any state the source carries across reports turns them into warm
    // steady-state measurements — matching how oxlint renders a file with
    // several diagnostics. This group rebuilds the source every iteration
    // (outside the timing) to keep the cold first-render cost, a full prefix
    // scan, measured. The diagnostic is returned so its teardown — dropping
    // the sole `Arc` to the megabyte-sized source — also stays outside the
    // timing, like the reused sources below.
    let mut group = c.benchmark_group("render_cold/ci/declared");
    let handler = ci_handler();
    for fixture in &fixtures {
        group.throughput(Throughput::Bytes(fixture.source_len as u64));
        group.bench_function(BenchmarkId::from_parameter(fixture.name), |b| {
            b.iter_batched(
                || cold_declared_diagnostic(fixture),
                |diagnostic| {
                    let mut out = String::new();
                    handler
                        .render_report(&mut out, black_box(diagnostic.as_ref()))
                        .expect("render succeeds");
                    (out, diagnostic)
                },
                BatchSize::LargeInput,
            );
        });
    }
    group.finish();

    // Full `render_report`, using the same report/source wrapper and the two
    // diagnostic shapes used by oxlint.
    for (mode, handler) in [("terminal", terminal_handler()), ("ci", ci_handler())] {
        let mut group = c.benchmark_group(format!("render/{mode}"));
        for case in DIAGNOSTIC_CASES {
            for fixture in &fixtures {
                let diagnostic = (case.build)(fixture);
                group.throughput(Throughput::Bytes(fixture.source_len as u64));
                group.bench_function(BenchmarkId::new(case.name, fixture.name), |b| {
                    b.iter(|| {
                        let mut out = String::new();
                        handler
                            .render_report(&mut out, black_box(diagnostic.as_ref()))
                            .expect("render succeeds");
                        black_box(out);
                    });
                });
            }
        }
        group.finish();
    }

    // Oxlint sends all diagnostics for one file together. Measure the same
    // reports rendered independently and through a temporary source batch,
    // including reverse input order to exercise the batch's internal source
    // ordering and output-order restoration.
    let mut group = c.benchmark_group("render_batch/ci");
    let handler = ci_handler();
    for fixture in &fixtures {
        for (order, reverse) in [("ascending", false), ("descending", true)] {
            let mut diagnostics: Vec<Error> = (1..=16)
                .map(|part| {
                    let offset = fixture.source_len * part / 17;
                    let span = identifier_span_at(fixture.source.inner(), offset);
                    let diagnostic = lint_diagnostic(
                        "Variable 'resolve' is declared but never used.",
                        "Consider removing this declaration.",
                    )
                    .with_label(LabeledSpan::new_with_span(
                        Some("'resolve' is declared here".to_string()),
                        span,
                    ));
                    Error::new(diagnostic).with_source_code(Arc::clone(&fixture.source))
                })
                .collect();
            if reverse {
                diagnostics.reverse();
            }
            let diagnostics: Vec<&dyn Diagnostic> = diagnostics.iter().map(AsRef::as_ref).collect();

            group.throughput(Throughput::Bytes(fixture.source_len as u64));
            group.bench_function(
                BenchmarkId::new(format!("standalone/{order}"), fixture.name),
                |b| {
                    b.iter(|| {
                        diagnostics
                            .iter()
                            .map(|diagnostic| {
                                let mut out = String::new();
                                handler
                                    .render_report(&mut out, black_box(*diagnostic))
                                    .expect("render succeeds");
                                out
                            })
                            .collect::<Vec<_>>()
                    });
                },
            );
            group.bench_function(BenchmarkId::new(format!("batched/{order}"), fixture.name), |b| {
                b.iter(|| {
                    handler
                        .render_reports(fixture.source.as_ref(), black_box(&diagnostics))
                        .into_iter()
                        .map(|(result, rendered)| {
                            result.expect("render succeeds");
                            rendered
                        })
                        .collect::<Vec<_>>()
                });
            });
        }
    }
    group.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    targets = bench
);
criterion_main!(benches);
