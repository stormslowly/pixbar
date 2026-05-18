//! Sub-cell-precision two-value progress bar for narrow terminal widths.
//!
//! `pixbar` renders a progress bar with two stacked values — a primary
//! "played" position and a secondary "buffered / lookahead" marker — on a
//! single line, using standard Unicode 1/8 block characters (`▏▎▍▌▋▊▉█`).
//! No font installation is required. At 13 cells the bar resolves better
//! than 1%, at 40 cells better than 0.3%.
//!
//! # Quick start
//!
//! ```
//! use pixbar::{Bar, Capability};
//!
//! let s = Bar::new(40)
//!     .primary(0.33)                       // 0.0 ..= 1.0
//!     .secondary(0.67)                     // semantically >= primary
//!     .capability(Capability::EighthBlock) // optional; auto-detected
//!     .render();
//! print!("{}", s);
//! ```
//!
//! # Architecture
//!
//! The renderer is a pure function: `(width, primary, secondary, capability)`
//! produces a [`Vec<Cell>`](Cell) intermediate representation, which is then
//! serialized to ANSI by [`ansi::encode`]. The Cell IR is capability-agnostic
//! — glyph and color lookups happen end-of-pipe, which makes the renderer
//! snapshot- and property-testable and lets the same IR target multiple
//! backends (ANSI for terminals, optionally HTML via the `html` feature).
//!
//! # Scope
//!
//! This crate is a renderer, not a progress-reporting framework. Out of scope:
//! ETA / elapsed / throughput, spinners, `MultiProgress` orchestration,
//! iterator wrapping, style templating, auto-hide on non-TTY. Reach for
//! [`indicatif`](https://crates.io/crates/indicatif) if you need those.

#![warn(missing_docs)]

pub mod ansi;
pub mod detect;
pub mod glyphs;
pub mod render;

/// HTML serializer for the [`Cell`] intermediate representation.
///
/// Available under `#[cfg(test)]` and the `html` feature. Useful for
/// rendering visual regression fixtures or embedding bars in reports.
#[cfg(any(test, feature = "html"))]
pub mod html;

/// 24-bit truecolor channel triple (red, green, blue).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// Rendering tier — controls sub-cell precision and glyph set.
///
/// Auto-detected at runtime by [`detect::detect`]; override with the builder
/// or via the `APB_FORCE_CAP=ascii|eighth` environment variable. `Ascii` is
/// never auto-selected and exists for users in environments without Unicode
/// support.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capability {
    /// One position per cell. Uses only `█` and space. Requires 100-cell
    /// width to resolve a 1% step.
    Ascii,
    /// Eight sub-positions per cell using Unicode block elements
    /// `U+2580..U+258F`. Resolves a 1% step at 13 cells, 0.3% at 40 cells.
    /// Supported by every modern monospace font without patching.
    EighthBlock,
}

impl Capability {
    /// Number of sub-positions a single cell can encode under this tier.
    ///
    /// `Ascii → 1`, `EighthBlock → 8`. The total addressable sub-positions
    /// on a bar of width `W` is `W × sub_positions()`.
    pub fn sub_positions(self) -> u32 {
        match self {
            Capability::Ascii => 1,
            Capability::EighthBlock => 8,
        }
    }
}

/// Foreground/background palette for the three render layers.
///
/// `primary` colors the "played" segment, `secondary` the "buffered /
/// lookahead" segment, and `empty` is the test-only HTML backend's
/// background (the ANSI backend leaves empty cells transparent so the
/// terminal's own background shows through).
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    /// Fill color for the primary (main) progress segment.
    pub primary: Rgb,
    /// Fill color for the secondary (buffer / lookahead) segment.
    pub secondary: Rgb,
    /// Background color for empty cells. ANSI ignores it; HTML uses it.
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

/// A configured progress bar. Build with [`Bar::new`] then chain setters.
///
/// `Bar` is the public entry point. Calling [`Bar::render`] produces an
/// ANSI string ready for `stdout`; calling [`Bar::cells`] returns the raw
/// [`Cell`] IR for callers that want to consume the bar in another form
/// (e.g. a TUI library, a custom renderer, an HTML export).
///
/// # Input sanitization
///
/// - `primary` and `secondary` are clamped to `[0.0, 1.0]`.
/// - `NaN` becomes `0.0`.
/// - If `primary > secondary` the two are swapped (the contract is
///   `secondary ≥ primary`, e.g. "buffered ≥ played").
/// - `width == 0` produces an empty render.
#[derive(Clone, Debug)]
pub struct Bar {
    width: usize,
    primary: f64,
    secondary: f64,
    theme: Theme,
    capability: Capability,
}

impl Bar {
    /// Create a bar of the given cell width with default theme and
    /// auto-detected capability.
    pub fn new(width: usize) -> Self {
        Self {
            width,
            primary: 0.0,
            secondary: 0.0,
            theme: Theme::default(),
            capability: detect::detect(),
        }
    }
    /// Set the primary ("played") progress in `[0.0, 1.0]`.
    pub fn primary(mut self, v: f64) -> Self { self.primary = v; self }
    /// Set the secondary ("buffered / lookahead") progress in `[0.0, 1.0]`.
    /// Should be `>= primary`; values below `primary` are silently swapped.
    pub fn secondary(mut self, v: f64) -> Self { self.secondary = v; self }
    /// Override the color palette.
    pub fn theme(mut self, t: Theme) -> Self { self.theme = t; self }
    /// Override the auto-detected rendering tier.
    pub fn capability(mut self, c: Capability) -> Self { self.capability = c; self }

    fn sanitized(&self) -> (f64, f64) {
        let s = |x: f64| if x.is_nan() { 0.0 } else { x.clamp(0.0, 1.0) };
        let (a, b) = (s(self.primary), s(self.secondary));
        if a > b { (b, a) } else { (a, b) }
    }

    /// Produce the capability-agnostic [`Cell`] sequence for this bar.
    ///
    /// Use this if you want to drive a custom backend (TUI library, HTML,
    /// SVG). Prefer [`Bar::render`] for direct terminal output.
    pub fn cells(&self) -> Vec<Cell> {
        let (p1, p2) = self.sanitized();
        classify(self.width, p1, p2, self.capability)
    }

    /// Serialize the bar to an ANSI string. Truecolor escape sequences are
    /// run-length-merged so adjacent same-color cells share one `\x1b[…m`.
    ///
    /// The string ends with `\x1b[0m`; empty cells trailing the bar are
    /// rendered as plain spaces so the terminal's native background shows
    /// through.
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
