#[test]
fn test_handler() {
    use miette::{Report, miette};

    let error: Report = miette!("oh no!");
    let _ = error.handler();
}
