#[test]
fn errors() {
    trycmd::TestCases::new().case("tests/cmd/*.trycmd").run();
}
