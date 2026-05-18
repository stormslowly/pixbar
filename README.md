# almost-perfect-progressbar

A two-value overlay progress bar (primary + buffer/lookahead) that uses
standard Unicode 1/8 block characters for sub-cell precision in modern
terminals. **Sub-1% step at every width ≥ 13 cells, 0.31% at the typical
40-cell width — no font installation required.**

## Quick start

```bash
cargo run --example basic            # static bar at both capability tiers
cargo run --example animated         # 4-second fill
cargo run --example narrow           # exercises 7 / 13 / 25 / 40 cell widths
cargo run --example degrade_ladder   # ASCII vs EighthBlock side-by-side
cargo run --bin apb-bench            # interactive TUI; q to quit
```

## Capability tiers

| Capability    | Sub-positions per cell | Min width for 1% precision | Char range |
|---------------|-----------------------:|---------------------------:|------------|
| `Ascii`       |                      1 |                       100  | `█` and space |
| `EighthBlock` |                      8 |                         13 | `U+2580..U+258F` (standard Unicode block elements) |

Auto-detected at runtime (defaults to `EighthBlock`); override with
`APB_FORCE_CAP=ascii|eighth`.

A precision breakdown with real rendered comparisons:
[`CLAUDE_PRECISION_REPORT.html`](CLAUDE_PRECISION_REPORT.html).

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
for full detail. (Note: the spec describes a `PatchedSixteenth` tier which
was implemented and then removed once measurement confirmed `EighthBlock`
already exceeds the 1% target at any reasonable width.)

## Asciinema casts

Run `scripts/record-casts.sh` after installing asciinema; recordings land in
`casts/`. Play back with `asciinema play casts/01-overview.cast`.
