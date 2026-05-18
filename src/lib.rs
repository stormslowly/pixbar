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

pub use render::{Cell, CellKind};
use crate::render::classify;

#[derive(Clone, Debug)]
pub struct Bar {
    width: usize,
    primary: f64,
    secondary: f64,
    theme: Theme,
    capability: Capability,
}

impl Bar {
    pub fn new(width: usize) -> Self {
        Self {
            width,
            primary: 0.0,
            secondary: 0.0,
            theme: Theme::default(),
            capability: detect::detect(),
        }
    }
    pub fn primary(mut self, v: f64) -> Self { self.primary = v; self }
    pub fn secondary(mut self, v: f64) -> Self { self.secondary = v; self }
    pub fn theme(mut self, t: Theme) -> Self { self.theme = t; self }
    pub fn capability(mut self, c: Capability) -> Self { self.capability = c; self }

    fn sanitized(&self) -> (f64, f64) {
        let s = |x: f64| if x.is_nan() { 0.0 } else { x.clamp(0.0, 1.0) };
        let (a, b) = (s(self.primary), s(self.secondary));
        if a > b { (b, a) } else { (a, b) }
    }

    pub fn cells(&self) -> Vec<Cell> {
        let (p1, p2) = self.sanitized();
        classify(self.width, p1, p2, self.capability)
    }

    pub fn render(&self) -> String {
        ansi::encode(&self.cells(), &self.theme, self.capability)
    }
}

#[cfg(test)]
mod bar_tests {
    use super::*;

    #[test] fn clamps_out_of_range() {
        let (p1, p2) = Bar::new(10).primary(-1.0).secondary(2.0).sanitized();
        assert_eq!(p1, 0.0);
        assert_eq!(p2, 1.0);
    }
    #[test] fn swaps_when_primary_above_secondary() {
        let (p1, p2) = Bar::new(10).primary(0.9).secondary(0.1).sanitized();
        assert_eq!(p1, 0.1);
        assert_eq!(p2, 0.9);
    }
    #[test] fn nan_becomes_zero() {
        let (p1, _) = Bar::new(10).primary(f64::NAN).sanitized();
        assert_eq!(p1, 0.0);
    }
    #[test] fn zero_width_no_cells() {
        assert!(Bar::new(0).primary(0.5).secondary(0.7).cells().is_empty());
    }
    #[test] fn render_is_non_empty_for_nonzero_width() {
        let s = Bar::new(8).primary(0.5).secondary(0.7).render();
        assert!(!s.is_empty());
    }
}
