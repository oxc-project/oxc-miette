use miette::Diagnostic;

#[derive(Debug, miette::Diagnostic, thiserror::Error)]
#[error("A complex error happened")]
struct SourceError {
    #[source_code]
    code: String,
    #[help]
    help: String,
    #[label("here")]
    label: (usize, usize),
}

#[derive(Debug, miette::Diagnostic, thiserror::Error)]
#[error("AnErr")]
struct AnErr;

#[derive(Debug, miette::Diagnostic, thiserror::Error)]
#[error("TestError")]
struct TestStructError {
    #[diagnostic_source]
    asdf_inner_foo: SourceError,
}

#[derive(Debug, miette::Diagnostic, thiserror::Error)]
#[error("TestError")]
enum TestEnumError {
    Without,
    WithTuple(#[diagnostic_source] AnErr),
    WithStruct {
        #[diagnostic_source]
        inner: AnErr,
    },
}

#[derive(Debug, miette::Diagnostic, thiserror::Error)]
#[error("TestError")]
struct TestTupleError(#[diagnostic_source] AnErr);

#[derive(Debug, miette::Diagnostic, thiserror::Error)]
#[error("TestError")]
struct TestBoxedError(#[diagnostic_source] Box<dyn Diagnostic>);

#[derive(Debug, miette::Diagnostic, thiserror::Error)]
#[error("TestError")]
struct TestBoxedSendError(#[diagnostic_source] Box<dyn Diagnostic + Send>);

#[derive(Debug, miette::Diagnostic, thiserror::Error)]
#[error("TestError")]
struct TestBoxedSendSyncError(#[diagnostic_source] Box<dyn Diagnostic + Send + Sync>);

#[derive(Debug, miette::Diagnostic, thiserror::Error)]
#[error("TestError")]
struct TestArcedError(#[diagnostic_source] std::sync::Arc<dyn Diagnostic>);

#[test]
fn test_diagnostic_source() {
    let error = TestStructError {
        asdf_inner_foo: SourceError { code: String::new(), help: String::new(), label: (0, 0) },
    };
    assert!(error.diagnostic_source().is_some());

    let error = TestEnumError::Without;
    assert!(error.diagnostic_source().is_none());

    let error = TestEnumError::WithTuple(AnErr);
    assert!(error.diagnostic_source().is_some());

    let error = TestEnumError::WithStruct { inner: AnErr };
    assert!(error.diagnostic_source().is_some());

    let error = TestTupleError(AnErr);
    assert!(error.diagnostic_source().is_some());

    let error = TestBoxedError(Box::new(AnErr));
    assert!(error.diagnostic_source().is_some());

    let error = TestBoxedSendError(Box::new(AnErr));
    assert!(error.diagnostic_source().is_some());

    let error = TestBoxedSendSyncError(Box::new(AnErr));
    assert!(error.diagnostic_source().is_some());

    let error = TestArcedError(std::sync::Arc::new(AnErr));
    assert!(error.diagnostic_source().is_some());
}

#[allow(dead_code)]
#[derive(Debug, miette::Diagnostic, thiserror::Error)]
#[error("A nested error happened")]
struct NestedError {
    #[source_code]
    code: String,
    #[label("here")]
    label: (usize, usize),
    #[diagnostic_source]
    the_other_err: Box<dyn Diagnostic>,
}

#[allow(unused)]
#[derive(Debug, miette::Diagnostic, thiserror::Error)]
#[error("A multi-error happened")]
struct MultiError {
    #[related]
    related_errs: Vec<Box<dyn Diagnostic>>,
}
