//! Cell-IR → ANSI string serializer with run-length-merged SGR sequences.
//!
//! Adjacent cells that share fg/bg do not re-emit `\x1b[…m`. Empty cells
//! emit only a space (no SGR) so the terminal's native background shows
//! through trailing whitespace. The output always ends with a final
//! `\x1b[0m` reset when any color was written.

use crate::{render::{Cell, CellKind}, glyphs::glyph_for, Capability, Rgb, Theme};

/// Serialize a [`Cell`] sequence to an ANSI truecolor string.
///
/// `theme` provides the three layer colors; `cap` selects the glyph table.
/// The returned string contains `ESC[38;2;…m` / `ESC[48;2;…m` SGR escapes
/// with same-color runs deduplicated and a trailing `ESC[0m` reset.
pub fn encode(cells: &[Cell], theme: &Theme, cap: Capability) -> String {
    let mut out = String::with_capacity(cells.len() * 24);
    let mut last_fg: Option<Rgb> = None;
    let mut last_bg: Option<Rgb> = None;

    for c in cells {
        let (fg, bg, empty) = layer_colors(c.kind, theme);
        if empty {
            if last_fg.is_some() || last_bg.is_some() {
                out.push_str("\x1b[0m");
                last_fg = None;
                last_bg = None;
            }
            out.push(' ');
            continue;
        }
        if last_fg != Some(fg) {
            out.push_str(&format!("\x1b[38;2;{};{};{}m", fg.0, fg.1, fg.2));
            last_fg = Some(fg);
        }
        match bg {
            Some(b) if last_bg != Some(b) => {
                out.push_str(&format!("\x1b[48;2;{};{};{}m", b.0, b.1, b.2));
                last_bg = Some(b);
            }
            None if last_bg.is_some() => {
                out.push_str("\x1b[49m");
                last_bg = None;
            }
            _ => {}
        }
        out.push(glyph_for(c.kind, c.sub_fill, cap));
    }
    if last_fg.is_some() || last_bg.is_some() {
        out.push_str("\x1b[0m");
    }
    out
}

/// Serialize a [`Cell`] sequence to a colorless glyph-only string.
///
/// Emits one glyph per cell with no SGR escapes — suitable for `NO_COLOR`
/// environments, log files, pipelines that don't understand ANSI, or any
/// consumer that wants to apply its own styling. Boundary cells lose
/// their bg-paint slice (see the
/// [boundary-cells note on `CellKind`](crate::render::CellKind#boundary-cells-expect-a-per-cell-background-paint)).
pub fn encode_plain(cells: &[Cell], cap: Capability) -> String {
    let mut out = String::with_capacity(cells.len());
    for c in cells {
        out.push(crate::glyphs::glyph_for(c.kind, c.sub_fill, cap));
    }
    out
}

/// Returns (fg, optional bg, is_empty_cell).
fn layer_colors(kind: CellKind, theme: &Theme) -> (Rgb, Option<Rgb>, bool) {
    match kind {
        CellKind::Empty                                       => (theme.primary, None, true),
        CellKind::PrimaryFull                                 => (theme.primary,   None, false),
        CellKind::SecondaryFull                               => (theme.secondary, None, false),
        CellKind::PrimaryBoundary | CellKind::DegradedOverlap => (theme.primary,   Some(theme.secondary), false),
        CellKind::SecondaryBoundary                           => (theme.secondary, None, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test] fn empty_cells_emit_spaces_only() {
        let cells = vec![Cell { kind: CellKind::Empty, sub_fill: 0 }; 3];
        assert_eq!(encode(&cells, &Theme::default(), Capability::EighthBlock), "   ");
    }

    #[test] fn encode_plain_emits_glyphs_no_escapes() {
        let cells = vec![
            Cell { kind: CellKind::PrimaryFull,    sub_fill: 0 },
            Cell { kind: CellKind::PrimaryBoundary, sub_fill: 4 },
            Cell { kind: CellKind::Empty,          sub_fill: 0 },
        ];
        let out = encode_plain(&cells, Capability::EighthBlock);
        assert_eq!(out, "█▌ ");
        assert!(!out.contains('\x1b'));
    }

    #[test] fn rle_does_not_resend_same_fg() {
        let cells = vec![
            Cell { kind: CellKind::PrimaryFull, sub_fill: 0 },
            Cell { kind: CellKind::PrimaryFull, sub_fill: 0 },
        ];
        let out = encode(&cells, &Theme::default(), Capability::EighthBlock);
        // Exactly one SGR fg + glyphs + reset.
        assert_eq!(out.matches("\x1b[38;2;").count(), 1);
        assert!(out.ends_with("██\x1b[0m"));
    }
}
