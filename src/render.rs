//! Pure-function cell classification — the core of the renderer.
//!
//! [`classify`] subdivides the bar into integer sub-positions, then assigns
//! each cell a [`CellKind`] and (where relevant) a `sub_fill` index into
//! the capability's sub-position ladder. The result is a capability-agnostic
//! intermediate representation; glyph and color lookups happen in
//! [`crate::ansi`] / [`crate::glyphs`].

use crate::Capability;

/// What a single cell on the bar represents.
///
/// The companion `sub_fill` field on [`Cell`] carries the boundary's
/// position inside the cell (in capability sub-positions) for the three
/// boundary variants, and is `0` otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellKind {
    /// Cell is past both `primary` and `secondary` — render as background.
    Empty,
    /// Cell is fully inside the primary segment.
    PrimaryFull,
    /// Cell is past primary but fully inside the secondary segment.
    SecondaryFull,
    /// Cell straddles the primary boundary. `sub_fill` is the boundary's
    /// position within the cell (in capability sub-positions, `1..=N-1`).
    PrimaryBoundary,
    /// Cell straddles the secondary boundary. `sub_fill` is its position.
    SecondaryBoundary,
    /// Both boundaries fall inside the same cell and disagree. The
    /// renderer paints the primary boundary; the secondary boundary is
    /// suppressed in this cell and will appear in the next one (or be
    /// lost if the bar ends). `sub_fill` carries the primary position.
    DegradedOverlap,
}

/// A single cell of the rendered bar.
///
/// Produced by [`classify`] and consumed by [`crate::ansi::encode`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    /// The cell's role.
    pub kind: CellKind,
    /// Boundary position inside the cell (`0..N` for capability `N`).
    /// Always `0` for `Empty` / `PrimaryFull` / `SecondaryFull`.
    pub sub_fill: u8,
}

/// Classify each cell of a bar of `width` cells given `primary ≤ secondary`
/// in `[0.0, 1.0]` and a [`Capability`] tier.
///
/// The function is referentially transparent: same inputs, same output, no
/// I/O, no environment access. Boundary positions use strict inequalities,
/// so a sub-position lying exactly on a cell edge is classified as the
/// preceding cell's *Full variant — no spurious half-filled glyph.
pub fn classify(width: usize, p1: f64, p2: f64, cap: Capability) -> Vec<Cell> {
    let n = cap.sub_positions();
    let total = (width as u32).saturating_mul(n);
    let p1s = (p1 * total as f64).round() as u32;
    let p2s = (p2 * total as f64).round() as u32;

    let mut out = Vec::with_capacity(width);
    for i in 0..width as u32 {
        let s = i * n;
        let e = s + n;
        let p1_in = p1s > s && p1s < e;
        let p2_in = p2s > s && p2s < e;

        let kind = if p1_in && p2_in && p1s != p2s {
            CellKind::DegradedOverlap
        } else if p1_in {
            CellKind::PrimaryBoundary
        } else if p2_in {
            CellKind::SecondaryBoundary
        } else if e <= p1s {
            CellKind::PrimaryFull
        } else if e <= p2s {
            CellKind::SecondaryFull
        } else {
            CellKind::Empty
        };
        let sub_fill = if p1_in { (p1s - s) as u8 } else if p2_in { (p2s - s) as u8 } else { 0 };
        out.push(Cell { kind, sub_fill });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_zero_returns_empty() {
        let v = classify(0, 0.5, 0.7, Capability::EighthBlock);
        assert!(v.is_empty());
    }

    #[test]
    fn full_progress_all_primary_full() {
        let v = classify(5, 1.0, 1.0, Capability::EighthBlock);
        assert_eq!(v.len(), 5);
        assert!(v.iter().all(|c| c.kind == CellKind::PrimaryFull));
    }

    #[test]
    fn zero_progress_all_empty() {
        let v = classify(5, 0.0, 0.0, Capability::EighthBlock);
        assert!(v.iter().all(|c| c.kind == CellKind::Empty));
    }

    #[test]
    fn boundaries_in_distinct_cells_13_at_8ths() {
        // Worked example: width=13, p1=33%, p2=67%, EighthBlock.
        // T=104, p1s=round(34.32)=34, p2s=round(69.68)=70.
        let v = classify(13, 0.33, 0.67, Capability::EighthBlock);
        for i in 0..4   { assert_eq!(v[i].kind, CellKind::PrimaryFull, "cell {i}"); }
        assert_eq!(v[4].kind, CellKind::PrimaryBoundary);
        assert_eq!(v[4].sub_fill, 2);
        for i in 5..8   { assert_eq!(v[i].kind, CellKind::SecondaryFull, "cell {i}"); }
        assert_eq!(v[8].kind, CellKind::SecondaryBoundary);
        assert_eq!(v[8].sub_fill, 6);
        for i in 9..13  { assert_eq!(v[i].kind, CellKind::Empty, "cell {i}"); }
    }

    #[test]
    fn same_cell_overlap_triggers_degrade() {
        // 13 cells × 8 sub = 104. p1=0.12 → round(12.48)=12; p2=0.13 → round(13.52)=14.
        // Cell 1 (range 8..16) strictly contains both 12 and 14 (since 12>8 && 14<16). DegradedOverlap.
        let v = classify(13, 0.12, 0.13, Capability::EighthBlock);
        assert_eq!(v[1].kind, CellKind::DegradedOverlap);
        assert_eq!(v[1].sub_fill, 4);
    }

    #[test]
    fn p1_equals_p2_no_secondary_band() {
        // width=8, p1=p2=0.4 → T=64, ps=round(25.6)=26 inside cell 3 (24..32).
        let v = classify(8, 0.4, 0.4, Capability::EighthBlock);
        assert_eq!(v[3].kind, CellKind::PrimaryBoundary);
        assert!(v[4..].iter().all(|c| c.kind == CellKind::Empty));
    }

    #[test]
    fn boundary_exactly_on_cell_edge_falls_into_full() {
        // Width 4, EighthBlock (n=8), T=32. p1=0.25 → p1s=8, which is exactly
        // the boundary between cell 0 (0..8) and cell 1 (8..16). Strict
        // inequalities make `p1_in` false for both cells; cell 0 has e=8 and
        // e <= p1s (8<=8) → PrimaryFull. Cell 1 has s=8, and we expect Empty
        // or SecondaryFull depending on p2.
        let v = classify(4, 0.25, 0.25, Capability::EighthBlock);
        assert_eq!(v[0].kind, CellKind::PrimaryFull);
        assert_eq!(v[1].kind, CellKind::Empty);
        // No PrimaryBoundary cell exists for this exact-boundary position.
        assert!(!v.iter().any(|c| c.kind == CellKind::PrimaryBoundary));
    }
}

#[cfg(test)]
mod prop {
    use super::*;
    use proptest::prelude::*;

    fn any_cap() -> impl Strategy<Value = Capability> {
        prop_oneof![
            Just(Capability::Ascii),
            Just(Capability::EighthBlock),
        ]
    }

    proptest! {
        #[test]
        fn length_matches_width(
            width in 1usize..=200,
            p1 in 0.0f64..=1.0,
            p2 in 0.0f64..=1.0,
            cap in any_cap(),
        ) {
            let (lo, hi) = (p1.min(p2), p1.max(p2));
            let cells = classify(width, lo, hi, cap);
            prop_assert_eq!(cells.len(), width);
        }

        #[test]
        fn sub_fill_within_bounds(
            width in 1usize..=200,
            p1 in 0.0f64..=1.0,
            p2 in 0.0f64..=1.0,
            cap in any_cap(),
        ) {
            let (lo, hi) = (p1.min(p2), p1.max(p2));
            let n = cap.sub_positions() as u8;
            for c in classify(width, lo, hi, cap) {
                prop_assert!(c.sub_fill < n.max(1));
            }
        }
    }
}
