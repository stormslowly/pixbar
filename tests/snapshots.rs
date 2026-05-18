use almost_perfect_progressbar::{Bar, Capability};

fn fixture(width: usize, p1: f64, p2: f64, cap: Capability) -> String {
    Bar::new(width).primary(p1).secondary(p2).capability(cap).render()
}

#[test] fn snap_7_33_67_sixteenth() {
    insta::assert_snapshot!(fixture(7, 0.33, 0.67, Capability::PatchedSixteenth));
}
#[test] fn snap_13_33_67_eighth() {
    insta::assert_snapshot!(fixture(13, 0.33, 0.67, Capability::EighthBlock));
}
#[test] fn snap_13_12_13_eighth_degrade() {
    insta::assert_snapshot!(fixture(13, 0.12, 0.13, Capability::EighthBlock));
}
#[test] fn snap_40_50_50_sixteenth() {
    insta::assert_snapshot!(fixture(40, 0.50, 0.50, Capability::PatchedSixteenth));
}
#[test] fn snap_100_01_99_ascii() {
    insta::assert_snapshot!(fixture(100, 0.01, 0.99, Capability::Ascii));
}
