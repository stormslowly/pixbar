use crate::{render::{Cell, CellKind}, glyphs::glyph_for, Capability, Rgb, Theme};

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
