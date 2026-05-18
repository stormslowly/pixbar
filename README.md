# almost-perfect-progressbar

A research demo: a two-value overlay progress bar with 1% precision at 7-cell
width, using a custom-patched font for 1/16-cell horizontal sub-positions on
modern terminals (wezterm + ghostty).

## Quick start

```bash
cargo run --example basic            # static bar at 3 capability tiers
cargo run --example animated         # 4-second fill (EighthBlock)
cargo run --example narrow           # exercises 4 / 7 / 13 / 25 cell widths
cargo run --example degrade_ladder   # same bar across capability tiers
cargo run --bin apb-bench            # interactive TUI; q to quit
```

## Unlock 1/16-cell precision (patched font)

```bash
# 1. Drop a JetBrains Mono Regular TTF at fonts/JetBrainsMono-Regular.ttf.
#    See fonts/README.md.
# 2. Generate the patched font:
scripts/build-font.sh
# 3. Install fonts/JetBrainsMono-APB.ttf via your OS font manager.
# 4. Tell apb the patched font is available:
export APB_FONT_PATCHED=1
cargo run --example narrow
```

## Capability tiers

| Capability         | Sub-positions per cell | Min width for 1% precision |
|--------------------|-----------------------:|---------------------------:|
| `Ascii`            |                      1 |                       100  |
| `EighthBlock`      |                      8 |                        13  |
| `PatchedSixteenth` |                     16 |                         7  |

Auto-detected at runtime; override with `APB_FORCE_CAP=ascii|eighth|sixteenth`.

## Library use

```rust
use almost_perfect_progressbar::{Bar, Capability};

let s = Bar::new(40)
    .primary(0.33)       // 0.0 .. 1.0
    .secondary(0.67)     // semantically >= primary
    .capability(Capability::EighthBlock)
    .render();
print!("{}", s);
```

## Architecture

`(width, primary, secondary, capability) -> Vec<Cell>` is a pure function
(`render::classify`); ANSI/HTML serializers and glyph tables are end-of-pipe
lookups. See
[the design doc](docs/superpowers/specs/2026-05-18-almost-perfect-progressbar-design.md)
for full detail.

## Asciinema casts

Run `scripts/record-casts.sh` after installing asciinema; recordings land in
`casts/`. Play back with `asciinema play casts/01-overview.cast`.

To render the patched-font casts faithfully in a browser, embed the patched
TTF alongside the asciinema-player web component (see
<https://docs.asciinema.org/manual/player/quick-start/> for embedding).
