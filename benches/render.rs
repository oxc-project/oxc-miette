//! Benchmarks for miette's diagnostic rendering pipeline.
//!
//! These mirror how oxc actually consumes this crate: oxlint and oxfmt build a
//! `GraphicalReportHandler` (via [`GraphicalReportHandler::new`], i.e. a 400
//! column width and a single context line) and call
//! [`GraphicalReportHandler::render_report`] once per diagnostic. A typical
//! diagnostic is a single-label `Warning` carrying a rule code and a help line,
//! pointing somewhere into a source file.
//!
//! The fixtures are real-world TypeScript/JSX files pulled from
//! <https://github.com/oxc-project/benchmark-files> (pinned to a fixed
//! revision). They are downloaded once and cached under `target/` so that
//! neither the repository nor the published crate carries multi-megabyte blobs.

use std::{fs, path::PathBuf, time::Duration};

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use miette::{
    Diagnostic, GraphicalReportHandler, GraphicalTheme, NamedSource, SourceCode, SourceSpan,
};
use thiserror::Error;

/// Pinned revision of <https://github.com/oxc-project/benchmark-files>, so the
/// inputs (and therefore CodSpeed instruction counts) stay reproducible.
const BENCHMARK_FILES_REV: &str = "c61afec7dd5a66cd5ecfc57ac74cf00687a0ca39";

/// Fixtures fetched from `benchmark-files`, spanning a range of sizes.
const FIXTURES: &[&str] = &[
    "RadixUIAdoptionSection.jsx", // small  (~2.5 KB)
    "kitchen-sink.tsx",           // large  (~716 KB)
    "cal.com.ts",                 // xlarge (~1.4 MB)
];

/// A label is placed this fraction of the way through each file — near the end,
/// to exercise the full forward scan miette performs to locate a span's line and
/// column.
const SPAN_FRACTION: f64 = 0.9;
/// Byte length of the benchmarked label span.
const SPAN_LEN: usize = 8;

struct Fixture {
    name: &'static str,
    source: String,
    span: SourceSpan,
}

/// A single-label warning with a code and help text — the shape of the vast
/// majority of `OxcDiagnostic`s emitted by oxlint.
#[derive(Debug, Diagnostic, Error)]
#[error("'resolve' is assigned a value but never used")]
#[diagnostic(
    severity = "Warning",
    code = "eslint(no-unused-vars)",
    help = "Consider removing this declaration or prefixing it with an underscore"
)]
struct LintDiagnostic {
    #[source_code]
    src: NamedSource<String>,
    #[label("'resolve' is declared here")]
    span: SourceSpan,
}

/// A diagnostic with several labels spread across the file — the shape of
/// oxlint diagnostics that point at a declaration plus related uses. Each
/// label makes the renderer locate a line/column far into the source.
#[derive(Debug, Diagnostic, Error)]
#[error("'resolve' is assigned a value but never used")]
#[diagnostic(
    severity = "Warning",
    code = "eslint(no-unused-vars)",
    help = "Consider removing this declaration or prefixing it with an underscore"
)]
struct MultiLabelDiagnostic {
    #[source_code]
    src: NamedSource<String>,
    #[label("'resolve' is declared here")]
    decl: SourceSpan,
    #[label("it is written here")]
    write: SourceSpan,
    #[label("but never read after this point")]
    last: SourceSpan,
}

/// oxc's interactive default: `GraphicalReportHandler::new()` in a terminal
/// resolves to unicode characters + RGB colors at a 400 column width.
fn colored_handler() -> GraphicalReportHandler {
    GraphicalReportHandler::new().with_theme(GraphicalTheme::unicode())
}

/// oxc's piped/CI output: `GraphicalTheme::none()` — ascii art, no color styling.
fn monochrome_handler() -> GraphicalReportHandler {
    GraphicalReportHandler::new().with_theme(GraphicalTheme::none())
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

    let span = span_at(&source, SPAN_FRACTION, SPAN_LEN);
    Fixture { name, source, span }
}

/// Snap `byte` down to the nearest UTF-8 char boundary within `source`.
///
/// Hand-rolled rather than `str::floor_char_boundary` because that method is
/// only stable since 1.93, above this crate's 1.85 MSRV (enforced by clippy).
fn floor_char_boundary(source: &str, mut byte: usize) -> usize {
    byte = byte.min(source.len());
    while byte > 0 && !source.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}

/// A [`SourceSpan`] of roughly `len` bytes located `fraction` of the way through
/// `source`, snapped to char boundaries so it is always valid.
fn span_at(source: &str, fraction: f64, len: usize) -> SourceSpan {
    let start = floor_char_boundary(source, (source.len() as f64 * fraction) as usize);
    let end = floor_char_boundary(source, start + len);
    (start as u32, (end - start) as u32).into()
}

fn bench(c: &mut Criterion) {
    let fixtures: Vec<Fixture> = FIXTURES.iter().copied().map(load_fixture).collect();

    // `SourceCode::read_span` is the "find the line/column for this span" scan
    // that every rendered snippet depends on — the hot path optimized in #211
    // and #212. A context of 1 line matches the renderer's default.
    let mut group = c.benchmark_group("read_span");
    for fixture in &fixtures {
        group.throughput(Throughput::Bytes(fixture.source.len() as u64));
        group.bench_function(BenchmarkId::from_parameter(fixture.name), |b| {
            b.iter(|| {
                let contents = fixture
                    .source
                    .read_span(black_box(&fixture.span), 1, 1)
                    .expect("span within source");
                black_box(contents);
            });
        });
    }
    group.finish();

    // Full `render_report` — the call oxlint/oxfmt make per diagnostic — with
    // both real themes: colored `unicode()` (interactive terminal) and `none()`
    // (piped/CI output).
    for (name, handler) in
        [("render", colored_handler()), ("render_monochrome", monochrome_handler())]
    {
        let mut group = c.benchmark_group(name);
        for fixture in &fixtures {
            let diagnostic = LintDiagnostic {
                src: NamedSource::new(fixture.name, fixture.source.clone()),
                span: fixture.span,
            };
            group.bench_function(BenchmarkId::from_parameter(fixture.name), |b| {
                b.iter(|| {
                    let mut out = String::new();
                    handler
                        .render_report(&mut out, black_box(&diagnostic as &dyn Diagnostic))
                        .expect("render succeeds");
                    black_box(out);
                });
            });
        }
        group.finish();
    }

    // Several labels near each other — a declaration and two related uses in
    // the same stretch of code, the realistic multi-label shape. The snippet
    // contexts overlap, so the renderer combines them into one window; every
    // label read and every merge attempt historically issued its own
    // `read_span`, i.e. its own scan of the source from byte 0 (five scans
    // for this shape), so this measures how rendering scales with label count.
    let mut group = c.benchmark_group("render_multi_label");
    let handler = colored_handler();
    for fixture in &fixtures {
        let start = fixture.span.offset() as usize;
        // A little under a screen of code apart, scaled down for tiny files
        // and clamped so every span stays in bounds.
        let delta = (fixture.source.len() / 20).clamp(1, 250);
        let near = |n: usize| {
            let at = (start + n * delta).min(fixture.source.len().saturating_sub(SPAN_LEN));
            span_at(&fixture.source, at as f64 / fixture.source.len() as f64, SPAN_LEN)
        };
        let diagnostic = MultiLabelDiagnostic {
            src: NamedSource::new(fixture.name, fixture.source.clone()),
            decl: near(0),
            write: near(1),
            last: near(2),
        };
        group.bench_function(BenchmarkId::from_parameter(fixture.name), |b| {
            b.iter(|| {
                let mut out = String::new();
                handler
                    .render_report(&mut out, black_box(&diagnostic as &dyn Diagnostic))
                    .expect("render succeeds");
                black_box(out);
            });
        });
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
