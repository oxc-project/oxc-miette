#[rustversion::attr(not(nightly), ignore = "UI diagnostics are pinned to nightly")]
#[cfg_attr(miri, ignore = "trybuild is unsupported under Miri")]
#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
