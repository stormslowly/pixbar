use pixbar::{Bar, Capability};

fn fixture(width: usize, p1: f64, p2: f64, cap: Capability) -> String {
    Bar::new(width).primary(p1).secondary(p2).capability(cap).render()
}

#[test] fn snap_13_33_67_eighth() {
    insta::assert_snapshot!(fixture(13, 0.33, 0.67, Capability::EighthBlock));
}
#[test] fn snap_13_12_13_eighth_degrade() {
    insta::assert_snapshot!(fixture(13, 0.12, 0.13, Capability::EighthBlock));
}
#[test] fn snap_40_33_67_eighth() {
    insta::assert_snapshot!(fixture(40, 0.33, 0.67, Capability::EighthBlock));
}
#[test] fn snap_100_01_99_ascii() {
    insta::assert_snapshot!(fixture(100, 0.01, 0.99, Capability::Ascii));
}

#[cfg(feature = "html")]
#[test]
fn html_fixture_renders() {
    use pixbar::{html::to_html, Theme};
    let bar = Bar::new(13).primary(0.33).secondary(0.67).capability(Capability::EighthBlock);
    let html = to_html(&bar.cells(), &Theme::default(), Capability::EighthBlock);
    std::fs::create_dir_all("tests/snapshots/visual").ok();
    std::fs::write("tests/snapshots/visual/13-33-67-eighth.html", &html).unwrap();
    assert!(html.starts_with("<pre"));
    assert!(html.ends_with("</pre>"));
    assert!(html.contains("rgb(88,166,255)")); // primary fg
}
