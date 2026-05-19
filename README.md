# pixbar

Sub-cell-precision two-value progress bar for narrow terminal widths.
Uses standard Unicode 1/8 block characters (no font installation required)
to get **0.96% step at 13 cells, 0.31% at 40 cells** — meaningful resolution
even when you only have a handful of columns to spare.

```text
[ ████████▌ buffer here ░░░░░ ]
```

```toml
[dependencies]
pixbar = "0.1"
```

## Quick start

```rust
use pixbar::Bar;

let s = Bar::new(40)
    .primary(0.33)       // 0.0 .. 1.0
    .secondary(0.67)     // semantically >= primary; rendered as buffer / lookahead
    .render();
print!("{}", s);
```

`Bar::new(width)` auto-detects two things from the environment:

- **Capability** (glyph set) defaults to `EighthBlock`; override with
  `.capability(Capability::Ascii)` or `APB_FORCE_CAP=ascii`.
- **Color** is on if `stdout` is a TTY and `NO_COLOR` is unset, off otherwise.
  Force with `.color(true)` / `.color(false)` or call `.render_plain()`.

## What's distinctive

- **Two-value overlay**: a primary progress and a secondary "buffer / lookahead"
  marker on the same bar — like a video player's "played + buffered" indicator.
  None of the standard Rust progress crates ship this.
- **Sub-cell precision**: 8 sub-positions per cell via `▏▎▍▌▋▊▉█`.
  1% precision at 13 cells; 0.31% at 40 cells.
- **Pure-function renderer**: `(width, primary, secondary, capability) -> Vec<Cell>`
  is referentially transparent. Snapshot- and property-tested.
- **Zero runtime dependencies in the library** — only the optional `pixbar-bench`
  binary pulls in `crossterm`.

## What this crate does NOT do

It is a renderer, not a framework. Out of scope:

- ETA / elapsed / throughput tracking — bring your own time math
- Spinner / indeterminate progress
- `MultiProgress` orchestration
- Iterator wrapping (`for x in collection.progress()`)
- Style templating (`"{bar:40} {pos}/{len} {eta}"`)
- Auto-hide on non-TTY

Reach for [`indicatif`](https://crates.io/crates/indicatif) if you need those.

## Capability tiers

| Capability    | Sub-positions per cell | Min width for 1% precision | Characters used |
|---------------|-----------------------:|---------------------------:|-----------------|
| `Ascii`       |                      1 |                       100  | `█` and space |
| `EighthBlock` |                      8 |                         13 | `U+2580..U+258F` (standard Unicode block elements) |

Auto-detected at runtime (defaults to `EighthBlock`); override via
`APB_FORCE_CAP=ascii|eighth`.

## Examples

```bash
cargo run --example basic             # static bar at both capability tiers
cargo run --example animated          # 4-second fill
cargo run --example narrow            # exercises 7 / 13 / 25 / 40 cell widths
cargo run --example degrade_ladder    # ASCII vs EighthBlock side by side
cargo run --bin pixbar-bench          # interactive TUI; q to quit
```

## Consumers other than stdout

`Bar::render()` to stdout is the easy path, but the renderer is also designed
to feed TUI libraries, HTML reports, and pipelines. The Cell IR is the
extension point.

### Piping to a file / `NO_COLOR` / log output

`Bar::new()` auto-detects this. If you'd rather not rely on detection (e.g.
you're building a string for elsewhere), be explicit:

```rust
use pixbar::Bar;

let plain = Bar::new(40).primary(0.5).secondary(0.7).render_plain();
// "████████████████████▌                  " — glyphs only, no SGR escapes.
```

`.render_plain()` is equivalent to `.color(false).render()`.

### `ratatui` (or any cell-grid TUI)

Consume the [`Cell`] IR directly and paint your own spans. Boundary cells
carry their secondary segment in the *background* color — see the
[boundary-bg note](https://docs.rs/pixbar/latest/pixbar/render/enum.CellKind.html#boundary-cells-expect-a-per-cell-background-paint).
A simple consumer that respects this:

```rust,ignore
use pixbar::{glyphs::glyph_for, Bar, Capability, CellKind, Theme};
use ratatui::prelude::*;

fn pixbar_line(bar: &Bar, theme: &Theme, cap: Capability) -> Line<'static> {
    let primary   = Color::Rgb(theme.primary.0,   theme.primary.1,   theme.primary.2);
    let secondary = Color::Rgb(theme.secondary.0, theme.secondary.1, theme.secondary.2);

    let spans: Vec<Span> = bar.cells().into_iter().map(|c| {
        let glyph = glyph_for(c.kind, c.sub_fill, cap).to_string();
        match c.kind {
            CellKind::Empty         => Span::raw(" "),
            CellKind::PrimaryFull   => Span::styled(glyph, Style::default().fg(primary)),
            CellKind::SecondaryFull => Span::styled(glyph, Style::default().fg(secondary)),
            // Boundary cells: fg = boundary's color, bg = the *other* segment.
            CellKind::PrimaryBoundary
            | CellKind::DegradedOverlap => Span::styled(
                glyph,
                Style::default().fg(primary).bg(secondary),
            ),
            CellKind::SecondaryBoundary => Span::styled(
                glyph,
                Style::default().fg(secondary),
            ),
        }
    }).collect();
    Line::from(spans)
}
```

If your renderer cannot paint cell backgrounds, you have two reasonable
fallbacks: snap boundary cells to the nearest full cell (lose sub-cell
precision), or call `bar.render_plain()` and accept that boundary glyphs
will look slightly off near the edge.

### HTML

Enable the `html` feature and use `pixbar::html::to_html`:

```toml
pixbar = { version = "0.1", features = ["html"] }
```

```rust,ignore
use pixbar::{html::to_html, Bar, Theme, Capability};

let bar  = Bar::new(40).primary(0.33).secondary(0.67);
let html = to_html(&bar.cells(), &Theme::default(), Capability::EighthBlock);
// Returns a `<pre>…</pre>` block with inline `style="color:…;background:…"`.
```

### Driving capability and color detection yourself

`pixbar::detect()` and `pixbar::detect_color()` are public if you want to
consult them outside the builder:

```rust
use pixbar::{Bar, detect, detect_color};

let bar = Bar::new(40)
    .capability(detect())
    .color(detect_color())
    .primary(0.5)
    .secondary(0.7);
```

## Architecture

A pure function classifies each cell into one of `Empty / PrimaryFull /
SecondaryFull / PrimaryBoundary / SecondaryBoundary / DegradedOverlap`, with
an integer sub-position. Glyph and color tables are end-of-pipe lookups, so
the renderer can target ANSI (default) or HTML (test-only) from the same
intermediate representation. See
[the design doc](docs/superpowers/specs/2026-05-18-almost-perfect-progressbar-design.md)
for the full algorithm; the spec describes an additional `PatchedSixteenth`
tier that was implemented and then removed once measurement confirmed
`EighthBlock` already exceeds the 1% target at any reasonable width.

## License

MIT — see [LICENSE](LICENSE).
