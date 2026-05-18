# almost-perfect-progressbar — Design

Date: 2026-05-18
Status: Approved for planning

A research demo for an extreme-precision two-value progress bar in modern terminals (wezterm + ghostty), built on a custom-patched font for sub-1/8-cell precision.

---

## 1. Goals & non-goals

### Goals

- Render a two-value overlay progress bar (player-style: main progress + buffer/lookahead, with `secondary ≥ primary`).
- Achieve 1% precision at a minimum bar width of **7 cells** using a patched font (1/16 cell sub-position).
- Provide a graceful degradation ladder: `PatchedSixteenth → EighthBlock → Ascii`.
- Ship as a research demo with three live deliverables: `cargo run --example`, asciinema casts, and an interactive `apb-bench` TUI binary.
- Ship a companion `apb-font-patch` CLI that adds new glyphs to a font's Private Use Area (PUA).

### Non-goals

- Not a general-purpose progress bar library (no spinners, no ETA, no multi-bar coordination).
- No backward compatibility with terminals lacking truecolor (a startup warning is acceptable).
- The font patcher will not install fonts into the OS — only emit a patched file.
- No CSI-based active terminal capability detection — environment variables only.

## 2. Fixed decisions

| Topic | Decision |
|---|---|
| Form factor | Single Rust crate (Rust 2024) |
| Two-value semantics | Primary + Buffer/lookahead (player), `secondary ≥ primary` |
| Precision target | 1/16 cell sub-position via patched font; 1/8 block fallback; ASCII last resort |
| Minimum bar width @ 1% | 7 cells (PatchedSixteenth) / 13 cells (EighthBlock) / 100 cells (Ascii) |
| Target terminals | wezterm + ghostty first; any truecolor terminal works |
| Codepoint strategy | PUA only (no override of existing Unicode block glyphs) |
| Same-cell 3-color edge case | Degrade — paint primary boundary, secondary boundary defers to next cell |
| Optional extension | `feature = "split-cell"` for vertical half-block precision in the overlap case (out of initial scope) |

## 3. Architecture overview

Rendering is a **pure function** `(width, primary, secondary, theme, capability) → Vec<Cell>`. The `Cell` intermediate representation is decoupled from ANSI serialization and from the capability tier.

Three benefits:

1. **Snapshot-testable** — every `(width, p1, p2)` combination produces a deterministic `Vec<Cell>` we can diff.
2. **Pluggable backends** — the same Cell IR can be serialized to ANSI (terminal), HTML (README preview), or SVG (screenshot).
3. **bench-friendly** — the interactive TUI consumes Cells directly instead of parsing ANSI strings.

### Module layout

```
src/
├── lib.rs            Public API: Bar, Theme, Capability, Rgb
├── render.rs         Pure classify(): widths/positions → Vec<Cell>
├── glyphs.rs         Three static glyph tables (ASCII, 1/8 BMP, 1/16 PUA)
├── ansi.rs           Vec<Cell> + Theme → ANSI string (RLE-merged SGR)
└── detect.rs         Env-only capability auto-detect

src/bin/
├── apb_bench.rs      crossterm-based interactive TUI (depends on lib)
└── apb_font_patch.rs Font patcher (does not depend on lib; deps: write-fonts, clap, anyhow)

examples/             cargo run --example targets
fonts/                Pre-built patched sample fonts (e.g., JetBrains Mono)
casts/                asciinema .cast recordings
tests/snapshots/      insta snapshots (cells + ANSI + HTML)
```

### Capability tiers

| Capability | Sub-positions per cell | Min width for 1% |
|---|---|---|
| `Ascii` | 1 | 100 |
| `EighthBlock` | 8 | 13 |
| `PatchedSixteenth` | 16 | 7 |

The rendering algorithm is the same across tiers; only the glyph table lookup differs.

## 4. Components

### `lib.rs` — public API

```rust
pub struct Bar {
    width: usize,
    primary: f64,    // 0.0..=1.0
    secondary: f64,  // 0.0..=1.0; semantically secondary >= primary
    theme: Theme,
    capability: Capability,
}

impl Bar {
    pub fn new(width: usize) -> Self;             // theme=default, capability=auto-detect
    pub fn primary(self, v: f64) -> Self;
    pub fn secondary(self, v: f64) -> Self;
    pub fn theme(self, t: Theme) -> Self;
    pub fn capability(self, c: Capability) -> Self;

    pub fn cells(&self) -> Vec<Cell>;             // pure, no I/O
    pub fn render(&self) -> String;                // = ansi::encode(self.cells(), &self.theme, cap)
}

#[derive(Clone, Copy)]
pub enum Capability { Ascii, EighthBlock, PatchedSixteenth }

#[derive(Clone, Copy)]
pub struct Theme {
    pub primary: Rgb,    // default Rgb(88, 166, 255)   — #58a6ff
    pub secondary: Rgb,  // default Rgb(60,  90, 160)   — pre-dimmed blue, no alpha math needed
    pub empty: Rgb,      // default Rgb(33, 38, 45)     — used by HTML export only; ANSI leaves empty cells uncolored so the terminal's own background shows through
}

#[derive(Clone, Copy)]
pub struct Rgb(pub u8, pub u8, pub u8);
```

### `render.rs` — classify (capability-agnostic)

```rust
pub struct Cell {
    pub kind: CellKind,
    pub sub_fill: u8,   // 0..=N where N is the capability's sub-position count
}

pub enum CellKind {
    Empty,
    PrimaryFull,
    SecondaryFull,
    PrimaryBoundary,     // primary boundary lands in this cell
    SecondaryBoundary,   // secondary boundary lands in this cell
    DegradedOverlap,     // both boundaries fall in the same cell — show primary, drop secondary
}

pub fn classify(width: usize, p1: f64, p2: f64, cap: Capability) -> Vec<Cell>;
```

Why `Cell` does not carry `glyph + fg + bg`: a `CellKind::PrimaryBoundary { sub_fill: 5 }` resolves to `▌` under `EighthBlock` but to `U+E105` under `PatchedSixteenth`. Capability is a single lookup decision at serialization time, not a re-classification.

### `glyphs.rs` — three static tables

```rust
pub fn glyph_for(kind: CellKind, sub_fill: u8, cap: Capability) -> char;
```

PUA allocation (initial):

- `U+E100..=U+E110` — primary-layer boundary, left `sub_fill`/16 filled (17 glyphs)
- `U+E120..=U+E130` — secondary-layer boundary, left `sub_fill`/16 filled (17 glyphs)

The 1/8 fallback uses the standard `▏▎▍▌▋▊▉█` block. ASCII uses only `█` and space.

### `ansi.rs` — serialization with SGR RLE

```rust
pub fn encode(cells: &[Cell], theme: &Theme, cap: Capability) -> String;
```

Emit truecolor SGR (`ESC[38;2;…m` for fg, `ESC[48;2;…m` for bg) only when fg/bg differs from the previous cell. End with `ESC[0m`. Empty cells emit a plain space with no SGR change — the terminal's own background color shows through. (`Theme.empty` is only consulted by the test-only `to_html()` exporter.)

### `detect.rs` — env-only auto-detect

```rust
pub fn detect() -> Capability;
```

Priority:

1. `APB_FORCE_CAP` (`ascii` / `eighth` / `sixteenth`) — explicit override
2. `APB_FONT_PATCHED=1` → `PatchedSixteenth`
3. Heuristic: presence of `WEZTERM_PANE` / `GHOSTTY_RESOURCES_DIR` / `KITTY_WINDOW_ID` and `TERM_PROGRAM` indicating a modern terminal → `EighthBlock`
4. Default → `EighthBlock`

`Ascii` is never auto-selected; user opts in via override.

### `bin/apb_bench.rs` — interactive TUI

`crossterm`-based event loop:

- `← / →` adjust primary; `Shift+← / Shift+→` adjust secondary
- `+ / −` resize bar width
- `t` cycle Capability (verify degradation ladder)
- `s` toggle degrade-vs-split (split-cell behind feature flag)
- `q` quit

Consumes `bar.cells()` directly and renders via crossterm primitives.

### `bin/apb_font_patch.rs` — font patcher

```
apb-font-patch <input.ttf> -o <output.ttf>
  --sub-positions 16
  --range PUA              (default; 17 primary + 17 secondary glyphs)
  [--no-overwrite]         (error if PUA codepoints already occupied)
```

Implementation: load with `write-fonts` (fontations), generate 17×2 vector outlines (each a horizontally-clipped filled rectangle of the cell's em square), assign to PUA codepoints in `cmap`, write out.

## 5. Data flow (worked example)

`Bar::new(7).primary(0.33).secondary(0.67).capability(PatchedSixteenth).render()`

### Step 1 — `render::classify`

Sub-positions per cell `N = 16`; total `T = 7 × 16 = 112`.

```
p1_sub = round(0.33 × 112) = 37
p2_sub = round(0.67 × 112) = 75
```

| cell | range | p1=37 in? | p2=75 in? | classification |
|---|---|---|---|---|
| 0 | 0..16 | no | no | `PrimaryFull` |
| 1 | 16..32 | no | no | `PrimaryFull` |
| 2 | 32..48 | **yes** (5) | no | `PrimaryBoundary(sub_fill=5)` |
| 3 | 48..64 | no | no | `SecondaryFull` |
| 4 | 64..80 | no | **yes** (11) | `SecondaryBoundary(sub_fill=11)` |
| 5 | 80..96 | no | no | `Empty` |
| 6 | 96..112 | no | no | `Empty` |

Degrade trigger: `floor(p1_sub/N) == floor(p2_sub/N) && p1_sub != p2_sub`. Not triggered here.

### Step 2 — `glyphs::glyph_for`

| cell | kind | sub_fill | glyph (PatchedSixteenth) |
|---|---|---|---|
| 0 | `PrimaryFull` | — | `█` |
| 1 | `PrimaryFull` | — | `█` |
| 2 | `PrimaryBoundary` | 5 | `\u{E105}` |
| 3 | `SecondaryFull` | — | `█` |
| 4 | `SecondaryBoundary` | 11 | `\u{E12B}` |
| 5 | `Empty` | — | space |
| 6 | `Empty` | — | space |

### Step 3 — `ansi::encode`

Per-cell fg/bg:

| cell | fg | bg |
|---|---|---|
| 0–1 | primary | default |
| 2 | primary | secondary (boundary right half belongs to the secondary layer) |
| 3 | secondary | default |
| 4 | secondary | default (boundary right half is empty) |
| 5–6 | — | default |

SGR RLE output:

```
ESC[38;2;88;166;255m           fg=primary
██                              cells 0,1
ESC[48;2;31;111;235m           +bg=secondary
\u{E105}                        cell 2
ESC[38;2;31;111;235;49m         fg=secondary, bg=default
█                               cell 3
\u{E12B}                        cell 4 (no SGR change)
ESC[0m                          reset
                                cells 5,6 = 2 spaces
```

### Design promises

1. `classify` is a pure function — same inputs, same output; no env reads, no `detect` calls.
2. `render` allocates at most `~4 × width × 24 bytes` (worst case: every cell emits full SGR+glyph; ~24 bytes/cell).
3. Degradation is local — only the overlap cell becomes `DegradedOverlap`; no neighbors are affected.

## 6. Error handling

Demo project — no `Result` exposed from the library. Inputs are sanitized at the boundary; impossible internal states use `debug_assert!`.

### Library inputs

| Input | Behavior |
|---|---|
| `width == 0` | Return empty `Vec<Cell>` / empty string; no panic |
| `primary` or `secondary` outside `[0.0, 1.0]` | `clamp(0.0, 1.0)` |
| `primary > secondary` | Swap (a caller mistake; secondary ≥ primary is the semantic contract) |
| `primary` or `secondary` is NaN | Treat as `0.0` |
| `capability` mismatched with runtime (e.g., `PatchedSixteenth` but font not installed) | Render anyway; `apb-bench` lets users cycle capability with `t` to compare |

### bench binary

Terminal lacks truecolor (rare) → print one-line warning at startup; continue. The terminal will dither to 256 colors itself.

### font patcher

| Error | Behavior |
|---|---|
| Input file missing / invalid OpenType | `anyhow::bail!` with a friendly message, exit 1 |
| Output path unwritable | Same |
| Target PUA codepoints already in use | Warn and overwrite by default; `--no-overwrite` errors out |

Out of scope: auto-installing the patched font into the OS (cross-platform messy). README documents manual installation.

## 7. Testing strategy

### Unit — `render::classify`

```rust
#[test] fn boundaries_in_different_cells() { ... }
#[test] fn primary_equals_secondary_no_secondary_band() { ... }
#[test] fn same_cell_overlap_triggers_degrade() { ... }
#[test] fn width_zero_returns_empty() { ... }
#[test] fn full_progress_all_full_blocks() { ... }
```

Add one **proptest** invariant:

```rust
proptest! {
  #[test] fn classify_invariants(
      width in 1usize..=200,
      p1 in 0.0f64..=1.0,
      p2 in 0.0f64..=1.0,
      cap in any_capability(),
  ) {
    let cells = classify(width, p1.min(p2), p1.max(p2), cap);
    prop_assert_eq!(cells.len(), width);
    // ordering: PrimaryFull run ⊂ SecondaryFull run union ⊂ pre-Empty
    // counting: sum of filled sub-positions ≈ p1*N and p2*N within ±1 sub-position
  }
}
```

### Snapshot — `Vec<Cell>` debug text

`insta` snapshots over a fixed fixture set:

```
(width, p1%, p2%, capability)
(  7, 33, 67, PatchedSixteenth)
( 13, 33, 67, EighthBlock)
( 13, 12, 13, EighthBlock)        ← exercises degrade
( 40, 50, 50, PatchedSixteenth)   ← exercises p1==p2
(100,  1, 99, Ascii)
```

### Snapshot — ANSI byte stream

Same fixture set, `bar.render()` output through `insta`. Captures regressions in SGR merging and ordering.

### Visual regression — HTML export

Test-only `Bar::to_html()` renders Cell IR to inline-styled `<span>` runs. One `.html` per fixture under `tests/snapshots/visual/`. CI flags diffs; PR reviewers eyeball the rendered HTML.

### font patcher

```rust
#[test] fn patched_font_loads_back() {
  let out = patch("fonts/JetBrainsMono.ttf");
  let face = ttf_parser::Face::parse(&out, 0).unwrap();
  for cp in 0xE100..=0xE110u32 {
      assert!(face.glyph_index(char::from_u32(cp).unwrap()).is_some());
  }
}
```

Not tested: "does the patched font actually render the right shape in a real terminal." That's a human-eye review job, captured by README screenshots and asciinema casts.

### Out of scope

- No tests for `Theme` (pure parameter passthrough, no logic).
- No env-mocked tests for `detect.rs` (short, hard to portably mock; verified by hand in `apb-bench`).

## 8. Dependencies

| Crate | Purpose |
|---|---|
| `crossterm` | bench binary event loop and terminal primitives |
| `anyhow` | font patcher error reporting |
| `clap` | font patcher argument parsing |
| `write-fonts` (fontations) | font patcher core |
| `ttf-parser` | font patcher round-trip verification in tests |
| `insta` | snapshot tests (dev-dependency) |
| `proptest` | property tests (dev-dependency) |

Library `lib.rs` itself has **no runtime dependencies** — pure Rust standard library. Binaries pull in the above only as needed.
