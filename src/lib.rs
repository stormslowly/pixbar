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

/// Foreground/background palette for the render layers.
///
/// `primary` colors the "played" segment, `secondary` the "buffered /
/// lookahead" segment, `overflow` the slice where primary exceeds
/// secondary under [`OverflowPolicy::Distinct`], and `empty` is the
/// HTML backend's background (the ANSI backend leaves empty cells
/// transparent so the terminal's own background shows through).
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    /// Fill color for the primary (main) progress segment.
    pub primary: Rgb,
    /// Fill color for the secondary (buffer / lookahead) segment.
    pub secondary: Rgb,
    /// Background color for empty cells. ANSI ignores it; HTML uses it.
    pub empty: Rgb,
    /// Fill color for the overflow slice (`primary > secondary` under
    /// [`OverflowPolicy::Distinct`]). Defaults to a red close to the
    /// GitHub "deletion" tone so over-budget / over-pace progress reads
    /// as an alert at a glance.
    pub overflow: Rgb,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            primary:   Rgb(88, 166, 255),
            secondary: Rgb(60,  90, 160),
            empty:     Rgb(33,  38,  45),
            overflow:  Rgb(248, 81, 73),
        }
    }
}

/// How [`Bar`] reconciles a `primary` greater than `secondary`.
///
/// The original two-value contract assumes `secondary ≥ primary` (e.g.
/// "buffered ≥ played"). For semantics where primary may legitimately
/// run past secondary — over-budget spend vs. budget, used vs. expected
/// pace — pick [`OverflowPolicy::Distinct`] to render the excess as a
/// dedicated overflow segment instead of folding it back.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverflowPolicy {
    /// If `primary > secondary`, silently swap them so `secondary` is
    /// always the higher value. Preserves the original visual but loses
    /// the directional information. **Default.**
    #[default]
    Swap,
    /// Cap `primary` at `secondary`'s value when `primary > secondary`.
    /// The bar shows `primary == secondary` instead of the original
    /// over-run. Useful when the caller treats `secondary` as a hard
    /// ceiling.
    Clamp,
    /// Preserve `primary > secondary`. Cells in `(secondary, primary]`
    /// are emitted with overflow [`CellKind`] variants and rendered
    /// using [`Theme::overflow`] — letting over-budget / over-pace
    /// progress paint as a distinct color band.
    Distinct,
}

pub use render::{Cell, CellKind};
pub use detect::{detect, detect_color};
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
/// - When `primary > secondary`, behavior depends on [`Bar::overflow`]:
///   default [`OverflowPolicy::Swap`] silently swaps them;
///   [`OverflowPolicy::Clamp`] caps primary at secondary;
///   [`OverflowPolicy::Distinct`] preserves both and renders the excess
///   with [`CellKind::OverflowFull`] and friends in [`Theme::overflow`].
/// - `width == 0` produces an empty render.
#[derive(Clone, Debug)]
pub struct Bar {
    width: usize,
    primary: f64,
    secondary: f64,
    theme: Theme,
    capability: Capability,
    color: bool,
    min_visible: bool,
    overflow: OverflowPolicy,
}

impl Bar {
    /// Create a bar of the given cell width with default theme and
    /// auto-detected capability and color.
    ///
    /// Color emission defaults to [`detect_color`] — i.e. `false` when
    /// `NO_COLOR` is set or `stdout` is not a TTY, `true` otherwise.
    /// Use [`Bar::color`] to force a specific value (e.g. `.color(true)`
    /// when you are building a string for a non-stdout consumer that
    /// will paint it itself, or `.color(false)` to suppress escapes
    /// unconditionally).
    pub fn new(width: usize) -> Self {
        Self {
            width,
            primary: 0.0,
            secondary: 0.0,
            theme: Theme::default(),
            capability: detect::detect(),
            color: detect::detect_color(),
            min_visible: false,
            overflow: OverflowPolicy::default(),
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
    /// Force color on (`true`) or off (`false`) for [`Bar::render`].
    ///
    /// When `false`, `render()` emits a glyph-only string with no SGR
    /// escapes — the same output as [`Bar::render_plain`]. Use this when
    /// piping to a file, when `NO_COLOR` is set, or when a downstream
    /// renderer will apply its own styling.
    pub fn color(mut self, on: bool) -> Self { self.color = on; self }
    /// Set color emission from [`detect_color`] — `false` when `NO_COLOR`
    /// is set or `stdout` is not a TTY, `true` otherwise.
    ///
    /// Equivalent to `self.color(pixbar::detect_color())`. Library
    /// callers that wire pixbar into a CLI usually want this on every
    /// `Bar::new(...)` so behavior matches the surrounding tool.
    pub fn auto_color(self) -> Self {
        let c = detect::detect_color();
        self.color(c)
    }
    /// Guarantee that any positive `primary` / `secondary` fraction
    /// renders as at least one sub-position — i.e. never disappears to
    /// zero cells under coarse capability or narrow widths.
    ///
    /// Without this, e.g. `primary(0.01)` on an 8-cell `Ascii` bar
    /// rounds to zero cells and the user sees no progress at all.
    /// With `min_visible(true)`, the value is bumped up to the smallest
    /// representable sub-position (`1 / (width × sub_positions)`)
    /// before classification. The post-bump secondary is never pulled
    /// below the post-bump primary.
    ///
    /// Off by default to preserve referential transparency between
    /// `primary` and the rendered output.
    pub fn min_visible(mut self, on: bool) -> Self { self.min_visible = on; self }
    /// Set the [`OverflowPolicy`] that decides how `primary > secondary`
    /// is rendered. Default: [`OverflowPolicy::Swap`].
    pub fn overflow(mut self, p: OverflowPolicy) -> Self { self.overflow = p; self }

    /// Sanitize raw inputs to `lo ≤ hi`. Only retained for unit tests
    /// that pre-date [`Bar::resolved`]; rendering uses `resolved`
    /// directly so it can honor [`OverflowPolicy::Distinct`].
    #[cfg(test)]
    fn sanitized(&self) -> (f64, f64) {
        let (lo, hi, _) = self.resolved();
        (lo, hi)
    }

    /// Returns `(lo, hi, is_overflow)` after applying `min_visible` and
    /// the active [`OverflowPolicy`]. `lo ≤ hi` always; `is_overflow` is
    /// `true` only when policy is `Distinct` and the original primary
    /// exceeded the original secondary.
    fn resolved(&self) -> (f64, f64, bool) {
        let s = |x: f64| if x.is_nan() { 0.0 } else { x.clamp(0.0, 1.0) };
        let p = s(self.primary);
        let q = s(self.secondary);

        let (mut lo, mut hi, is_overflow) = match self.overflow {
            OverflowPolicy::Swap     => (p.min(q), p.max(q), false),
            OverflowPolicy::Clamp    => (p.min(q), q,        false),
            OverflowPolicy::Distinct => {
                if p > q { (q, p, true) } else { (p, q, false) }
            }
        };

        if self.min_visible {
            let total = (self.width as u32)
                .saturating_mul(self.capability.sub_positions())
                .max(1) as f64;
            let floor = 1.0 / total;
            if lo > 0.0 && lo < floor { lo = floor; }
            if hi > 0.0 && hi < floor { hi = floor; }
            if hi < lo { hi = lo; }
        }
        (lo, hi, is_overflow)
    }

    /// Produce the capability-agnostic [`Cell`] sequence for this bar.
    ///
    /// Use this if you want to drive a custom backend (TUI library, HTML,
    /// SVG). Prefer [`Bar::render`] for direct terminal output.
    ///
    /// # Boundary cells carry their second segment in the background
    ///
    /// Cells of kind [`CellKind::PrimaryBoundary`],
    /// [`CellKind::SecondaryBoundary`], [`CellKind::DegradedOverlap`]
    /// and the overflow-boundary variants only encode the boundary glyph;
    /// the *other* side of the boundary is conveyed by a colored
    /// background paint on the same cell. Consumers targeting backends
    /// without per-cell background support (some `ratatui` cell builders,
    /// plain `print!`, log files) must either snap each boundary to the
    /// nearest full cell or composite the glyph themselves. See the
    /// [boundary-cells note on `CellKind`](CellKind#boundary-cells-expect-a-per-cell-background-paint).
    pub fn cells(&self) -> Vec<Cell> {
        let (lo, hi, is_overflow) = self.resolved();
        let mut cells = classify(self.width, lo, hi, self.capability);
        if is_overflow {
            for c in cells.iter_mut() {
                c.kind = match c.kind {
                    CellKind::SecondaryFull       => CellKind::OverflowFull,
                    CellKind::PrimaryBoundary     => CellKind::OverflowInnerBoundary,
                    CellKind::SecondaryBoundary   => CellKind::OverflowOuterBoundary,
                    other => other,
                };
            }
        }
        cells
    }

    /// Serialize the bar.
    ///
    /// If [`Bar::color`] is `true` (the default), emits an ANSI truecolor
    /// string with run-length-merged SGR sequences, ending in `\x1b[0m`.
    /// If `color` is `false`, emits a colorless glyph-only string — same
    /// behavior as [`Bar::render_plain`].
    pub fn render(&self) -> String {
        if self.color {
            ansi::encode(&self.cells(), &self.theme, self.capability)
        } else {
            ansi::encode_plain(&self.cells(), self.capability)
        }
    }

    /// Serialize the bar to a colorless glyph-only string regardless of
    /// the [`Bar::color`] setting.
    ///
    /// Equivalent to `self.clone().color(false).render()`. Boundary cells
    /// will only show their boundary glyph (no bg-painted second segment)
    /// — see the
    /// [boundary-cells note on `CellKind`](CellKind#boundary-cells-expect-a-per-cell-background-paint).
    pub fn render_plain(&self) -> String {
        ansi::encode_plain(&self.cells(), self.capability)
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
    #[test] fn min_visible_off_lets_tiny_pct_round_to_zero() {
        // width=8, Ascii → 8 sub-positions total. p=0.05 → 0.4 → round → 0.
        let cells = Bar::new(8)
            .capability(Capability::Ascii)
            .primary(0.05).secondary(0.05)
            .cells();
        assert!(cells.iter().all(|c| c.kind == CellKind::Empty));
    }
    #[test] fn min_visible_on_bumps_tiny_pct_to_one_cell() {
        let cells = Bar::new(8)
            .capability(Capability::Ascii)
            .primary(0.05).secondary(0.05)
            .min_visible(true)
            .cells();
        assert_eq!(cells[0].kind, CellKind::PrimaryFull);
        assert!(cells[1..].iter().all(|c| c.kind == CellKind::Empty));
    }
    #[test] fn min_visible_does_not_bump_zero() {
        let cells = Bar::new(8)
            .capability(Capability::Ascii)
            .primary(0.0).secondary(0.0)
            .min_visible(true)
            .cells();
        assert!(cells.iter().all(|c| c.kind == CellKind::Empty));
    }
    #[test] fn overflow_swap_is_default_and_matches_legacy() {
        // p=0.9 > q=0.1 under default Swap → resolved as lo=0.1, hi=0.9, no overflow.
        let b = Bar::new(10).primary(0.9).secondary(0.1);
        let (lo, hi, ov) = b.resolved();
        assert_eq!((lo, hi, ov), (0.1, 0.9, false));
    }
    #[test] fn overflow_clamp_caps_primary_at_secondary() {
        let b = Bar::new(10).primary(0.9).secondary(0.1).overflow(OverflowPolicy::Clamp);
        let (lo, hi, ov) = b.resolved();
        // primary clamped to 0.1, so lo=hi=0.1, no overflow.
        assert_eq!((lo, hi, ov), (0.1, 0.1, false));
    }
    #[test] fn overflow_distinct_preserves_order_and_sets_flag() {
        let b = Bar::new(10).primary(0.9).secondary(0.1).overflow(OverflowPolicy::Distinct);
        let (lo, hi, ov) = b.resolved();
        assert_eq!((lo, hi, ov), (0.1, 0.9, true));
    }
    #[test] fn overflow_distinct_when_primary_le_secondary_no_overflow() {
        let b = Bar::new(10).primary(0.3).secondary(0.7).overflow(OverflowPolicy::Distinct);
        let (_, _, ov) = b.resolved();
        assert!(!ov);
    }
    #[test] fn overflow_distinct_produces_overflow_kinds() {
        // width=13, p1=0.67, p2=0.33, EighthBlock, Distinct.
        // Without Distinct (default Swap) this is identical to the existing
        // 13/33/67 snapshot — same cells, just labeled.
        // With Distinct, cells (5..8) that would be SecondaryFull become OverflowFull,
        // boundaries get renamed too.
        let cells = Bar::new(13)
            .primary(0.67).secondary(0.33)
            .capability(Capability::EighthBlock)
            .overflow(OverflowPolicy::Distinct)
            .cells();
        assert!(cells.iter().any(|c| c.kind == CellKind::OverflowFull));
        // Inner boundary at lo=0.33 → cell 4.
        assert_eq!(cells[4].kind, CellKind::OverflowInnerBoundary);
        // Outer boundary at hi=0.67 → cell 8.
        assert_eq!(cells[8].kind, CellKind::OverflowOuterBoundary);
    }
    #[test] fn min_visible_bumps_secondary_independently() {
        // primary=0, secondary=0.05 with min_visible → secondary becomes 1/8 ≈ 0.125
        // → should produce a SecondaryBoundary or SecondaryFull in cell 0.
        let cells = Bar::new(8)
            .capability(Capability::EighthBlock)
            .primary(0.0).secondary(0.05)
            .min_visible(true)
            .cells();
        assert!(matches!(
            cells[0].kind,
            CellKind::SecondaryFull | CellKind::SecondaryBoundary
        ));
    }
}
