pub mod render;
pub mod glyphs;
pub mod ansi;
pub mod detect;

#[cfg(any(test, feature = "html"))]
pub mod html;
