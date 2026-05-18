pub mod render;
pub mod glyphs;
pub mod ansi;
pub mod detect;

#[cfg(any(test, feature = "html"))]
pub mod html;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capability {
    Ascii,
    EighthBlock,
    PatchedSixteenth,
}

impl Capability {
    pub fn sub_positions(self) -> u32 {
        match self {
            Capability::Ascii => 1,
            Capability::EighthBlock => 8,
            Capability::PatchedSixteenth => 16,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub primary: Rgb,
    pub secondary: Rgb,
    pub empty: Rgb,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            primary:   Rgb(88, 166, 255),
            secondary: Rgb(60,  90, 160),
            empty:     Rgb(33,  38,  45),
        }
    }
}
