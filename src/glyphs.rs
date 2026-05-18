use crate::{render::CellKind, Capability};

pub fn glyph_for(kind: CellKind, sub_fill: u8, cap: Capability) -> char {
    match cap {
        Capability::Ascii       => ascii(kind),
        Capability::EighthBlock => eighth(kind, sub_fill),
    }
}

fn ascii(kind: CellKind) -> char {
    match kind {
        CellKind::Empty => ' ',
        CellKind::PrimaryFull
        | CellKind::SecondaryFull
        | CellKind::PrimaryBoundary
        | CellKind::SecondaryBoundary
        | CellKind::DegradedOverlap => '█',
    }
}

fn eighth(kind: CellKind, sub_fill: u8) -> char {
    // Unicode left 1/8 block ladder: space ▏▎▍▌▋▊▉█ — indexed 0..=8.
    const TABLE: [char; 9] = [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];
    match kind {
        CellKind::Empty => ' ',
        CellKind::PrimaryFull | CellKind::SecondaryFull => '█',
        CellKind::PrimaryBoundary
        | CellKind::SecondaryBoundary
        | CellKind::DegradedOverlap => TABLE[sub_fill.min(8) as usize],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test] fn ascii_full() {
        assert_eq!(glyph_for(CellKind::PrimaryFull, 0, Capability::Ascii), '█');
    }
    #[test] fn ascii_empty() {
        assert_eq!(glyph_for(CellKind::Empty, 0, Capability::Ascii), ' ');
    }
    #[test] fn eighth_half() {
        assert_eq!(glyph_for(CellKind::PrimaryBoundary, 4, Capability::EighthBlock), '▌');
    }
    #[test] fn eighth_zero_sub_is_space() {
        assert_eq!(glyph_for(CellKind::PrimaryBoundary, 0, Capability::EighthBlock), ' ');
    }
    #[test] fn eighth_full_sub_is_full_block() {
        assert_eq!(glyph_for(CellKind::PrimaryBoundary, 8, Capability::EighthBlock), '█');
    }
}
