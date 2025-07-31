use criterion::{black_box, criterion_group, criterion_main, Criterion};
use miette::*;
use std::fmt::Write;

#[derive(Debug, miette::Diagnostic, thiserror::Error)]
#[error("test error")]
#[diagnostic(code(test::error))]
struct TestError {
    #[source_code]
    src: NamedSource<String>,
    #[label("problem here")]
    span: SourceSpan,
}

fn create_large_source() -> String {
    // Create a large source file for testing
    let line = "    let some_variable = some_function_call(param1, param2, param3);\n";
    line.repeat(1000)
}

fn create_medium_source() -> String {
    let line = "    let x = y + z;\n";
    line.repeat(100)
}

fn create_small_source() -> String {
    "fn main() {\n    println!(\"Hello, world!\");\n}\n".to_string()
}

fn bench_diagnostic_creation(c: &mut Criterion) {
    c.bench_function("diagnostic_creation_small", |b| {
        let src = create_small_source();
        b.iter(|| {
            let error = TestError {
                src: NamedSource::new("test.rs", src.clone()),
                span: (15, 10).into(),
            };
            black_box(error)
        })
    });

    c.bench_function("diagnostic_creation_medium", |b| {
        let src = create_medium_source();
        b.iter(|| {
            let error = TestError {
                src: NamedSource::new("test.rs", src.clone()),
                span: (500, 10).into(),
            };
            black_box(error)
        })
    });

    c.bench_function("diagnostic_creation_large", |b| {
        let src = create_large_source();
        b.iter(|| {
            let error = TestError {
                src: NamedSource::new("test.rs", src.clone()),
                span: (5000, 10).into(),
            };
            black_box(error)
        })
    });
}

fn bench_source_span_reading(c: &mut Criterion) {
    c.bench_function("source_span_small", |b| {
        let src = create_small_source();
        let span = SourceSpan::new(15.into(), 10);
        b.iter(|| {
            let contents = src.read_span(&span, 1, 1).unwrap();
            black_box(contents)
        })
    });

    c.bench_function("source_span_medium", |b| {
        let src = create_medium_source();
        let span = SourceSpan::new(500.into(), 10);
        b.iter(|| {
            let contents = src.read_span(&span, 1, 1).unwrap();
            black_box(contents)
        })
    });

    c.bench_function("source_span_large", |b| {
        let src = create_large_source();
        let span = SourceSpan::new(5000.into(), 10);
        b.iter(|| {
            let contents = src.read_span(&span, 3, 3).unwrap();
            black_box(contents)
        })
    });
}

fn bench_graphical_rendering(c: &mut Criterion) {
    let handler = GraphicalReportHandler::new();
    
    c.bench_function("graphical_render_small", |b| {
        let src = create_small_source();
        let error = TestError {
            src: NamedSource::new("test.rs", src),
            span: (15, 10).into(),
        };
        b.iter(|| {
            let mut output = String::new();
            handler.render_report(&mut output, &error).unwrap();
            black_box(output)
        })
    });

    c.bench_function("graphical_render_medium", |b| {
        let src = create_medium_source();
        let error = TestError {
            src: NamedSource::new("test.rs", src),
            span: (500, 10).into(),
        };
        b.iter(|| {
            let mut output = String::new();
            handler.render_report(&mut output, &error).unwrap();
            black_box(output)
        })
    });

    c.bench_function("graphical_render_large", |b| {
        let src = create_large_source();
        let error = TestError {
            src: NamedSource::new("test.rs", src),
            span: (5000, 10).into(),
        };
        b.iter(|| {
            let mut output = String::new();
            handler.render_report(&mut output, &error).unwrap();
            black_box(output)
        })
    });
}

fn bench_narratable_rendering(c: &mut Criterion) {
    let handler = NarratableReportHandler::new();
    
    c.bench_function("narratable_render_small", |b| {
        let src = create_small_source();
        let error = TestError {
            src: NamedSource::new("test.rs", src),
            span: (15, 10).into(),
        };
        b.iter(|| {
            let mut output = String::new();
            handler.render_report(&mut output, &error).unwrap();
            black_box(output)
        })
    });

    c.bench_function("narratable_render_medium", |b| {
        let src = create_medium_source();
        let error = TestError {
            src: NamedSource::new("test.rs", src),
            span: (500, 10).into(),
        };
        b.iter(|| {
            let mut output = String::new();
            handler.render_report(&mut output, &error).unwrap();
            black_box(output)
        })
    });
}

fn bench_multiple_labels(c: &mut Criterion) {
    #[derive(Debug, miette::Diagnostic, thiserror::Error)]
    #[error("multiple errors")]
    #[diagnostic(code(test::multiple))]
    struct MultiError {
        #[source_code]
        src: NamedSource<String>,
        #[label("first error")]
        span1: SourceSpan,
        #[label("second error")]
        span2: SourceSpan,
        #[label("third error")]
        span3: SourceSpan,
        #[label("fourth error")]
        span4: SourceSpan,
        #[label("fifth error")]
        span5: SourceSpan,
    }

    let handler = GraphicalReportHandler::new();
    
    c.bench_function("multiple_labels_render", |b| {
        let src = create_medium_source();
        let error = MultiError {
            src: NamedSource::new("test.rs", src),
            span1: (100, 5).into(),
            span2: (200, 8).into(),
            span3: (300, 12).into(),
            span4: (400, 6).into(),
            span5: (500, 10).into(),
        };
        b.iter(|| {
            let mut output = String::new();
            handler.render_report(&mut output, &error).unwrap();
            black_box(output)
        })
    });
}

criterion_group!(
    benches,
    bench_diagnostic_creation,
    bench_source_span_reading,
    bench_graphical_rendering,
    bench_narratable_rendering,
    bench_multiple_labels
);
criterion_main!(benches);