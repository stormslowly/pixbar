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
use pixbar::{Bar, Capability};

let s = Bar::new(40)
    .primary(0.33)       // 0.0 .. 1.0
    .secondary(0.67)     // semantically >= primary; rendered as buffer / lookahead
    .capability(Capability::EighthBlock)
    .render();
print!("{}", s);
```

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
