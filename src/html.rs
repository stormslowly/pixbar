use crate::{render::{Cell, CellKind}, glyphs::glyph_for, Capability, Rgb, Theme};

pub fn to_html(cells: &[Cell], theme: &Theme, cap: Capability) -> String {
    let mut s = String::new();
    s.push_str(r#"<pre style="font-family:'JetBrains Mono',monospace;font-size:18px;background:#0d1117;padding:8px;border-radius:6px;margin:0;display:inline-block;">"#);
    for c in cells {
        let g = glyph_for(c.kind, c.sub_fill, cap);
        let (fg, bg) = match c.kind {
            CellKind::Empty                                       => (theme.empty,     theme.empty),
            CellKind::PrimaryFull                                 => (theme.primary,   theme.empty),
            CellKind::SecondaryFull                               => (theme.secondary, theme.empty),
            CellKind::PrimaryBoundary | CellKind::DegradedOverlap => (theme.primary,   theme.secondary),
            CellKind::SecondaryBoundary                           => (theme.secondary, theme.empty),
        };
        s.push_str(&format!(
            r#"<span style="color:{};background:{};">{}</span>"#,
            rgb_css(fg), rgb_css(bg), html_escape(g),
        ));
    }
    s.push_str("</pre>");
    s
}

fn rgb_css(c: Rgb) -> String { format!("rgb({},{},{})", c.0, c.1, c.2) }

fn html_escape(c: char) -> String {
    match c {
        ' '  => "&nbsp;".into(),
        '<'  => "&lt;".into(),
        '>'  => "&gt;".into(),
        '&'  => "&amp;".into(),
        c    => c.to_string(),
    }
}
