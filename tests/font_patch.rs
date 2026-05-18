#![cfg(feature = "font-patch-integration")]

use std::path::Path;
use std::process::Command;

#[test]
fn patched_font_round_trips() {
    let fixture_dir = "tests/fixtures";
    let input  = format!("{fixture_dir}/Sample.ttf");
    let output = format!("{fixture_dir}/Sample.patched.ttf");

    if !Path::new(&input).exists() {
        eprintln!(
            "SKIP: {} not present. To enable, copy any TrueType font to that path \
             (a small free-licensed font like JetBrainsMono-Regular.ttf works).",
            input
        );
        return;
    }

    let status = Command::new(env!("CARGO_BIN_EXE_apb-font-patch"))
        .args([&input, "-o", &output])
        .status()
        .expect("failed to spawn apb-font-patch");
    assert!(status.success(), "patcher exited non-zero");

    let bytes = std::fs::read(&output).expect("patched font missing");
    let face  = ttf_parser::Face::parse(&bytes, 0).expect("patched font invalid");

    for cp in 0xE100..=0xE110u32 {
        let ch = char::from_u32(cp).unwrap();
        assert!(face.glyph_index(ch).is_some(), "missing PUA U+{cp:04X}");
    }
    for cp in 0xE120..=0xE130u32 {
        let ch = char::from_u32(cp).unwrap();
        assert!(face.glyph_index(ch).is_some(), "missing PUA U+{cp:04X}");
    }
}
