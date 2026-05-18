# almost-perfect-progressbar Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a research-demo Rust crate that renders a two-value overlay progress bar with 1% precision at 7-cell width using a custom-patched font (PUA codepoints), with a graceful 1/16 → 1/8 → ASCII degradation ladder, an interactive bench TUI, and a font-patcher CLI.

**Architecture:** Pure-function rendering: `(width, p1, p2, theme, capability) → Vec<Cell>` is the testable core. `Cell` is a capability-agnostic IR; serializers (ANSI, HTML) and capability-specific glyph tables are end-of-pipe lookups. Single Rust crate. Two binaries: `apb-bench` (crossterm TUI) and `apb-font-patch` (fontations-based font patcher).

**Tech Stack:** Rust 2024, crossterm, write-fonts (fontations), ttf-parser, clap, anyhow, insta, proptest.

**Project rule:** The repo owner does not want me to run `git commit` autonomously. Every "Checkpoint" step proposes a commit message — execute the commit only after the user explicitly approves.

**Reference spec:** `docs/superpowers/specs/2026-05-18-almost-perfect-progressbar-design.md`

---

## File map

```
Cargo.toml                          workspace = no; single package
src/lib.rs                          public API: Bar, Theme, Capability, Rgb, Cell, CellKind
src/render.rs                       classify(width, p1, p2, cap) -> Vec<Cell>
src/glyphs.rs                       glyph_for(kind, sub_fill, cap) -> char + three tables
src/ansi.rs                         encode(&[Cell], &Theme, cap) -> String
src/detect.rs                       detect() -> Capability  (env-only)
src/html.rs                         test-only Bar::to_html() — gated under cfg(any(test, feature="html"))
src/bin/apb_bench.rs                interactive TUI (crossterm)
src/bin/apb_font_patch.rs           font patcher CLI (fontations)
examples/basic.rs                   single static bar
examples/animated.rs                ticking primary + buffer
examples/narrow.rs                  exercises 7-cell and degrade-overlap
examples/degrade_ladder.rs          renders the same bar at Ascii / 8 / 16
tests/snapshots/                    insta + visual HTML snapshots
fonts/                              JetBrainsMono source + build output
casts/                              asciinema recordings (added later)
```

---

## Phase 0 — Scaffolding

### Task 1: Initialize Cargo.toml dependencies

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Replace Cargo.toml**

```toml
[package]
name = "almost-perfect-progressbar"
version = "0.1.0"
edition = "2024"

[lib]
path = "src/lib.rs"

[[bin]]
name = "apb-bench"
path = "src/bin/apb_bench.rs"

[[bin]]
name = "apb-font-patch"
path = "src/bin/apb_font_patch.rs"

[dependencies]
crossterm  = "0.28"
anyhow     = "1"
clap       = { version = "4", features = ["derive"] }
write-fonts = "0.27"
ttf-parser  = "0.24"

[dev-dependencies]
insta    = { version = "1", features = ["yaml"] }
proptest = "1"
```

- [ ] **Step 2: Verify it builds**

Run: `cargo build`
Expected: builds; warnings about missing `src/bin/*.rs` are fine for now.

- [ ] **Step 3: Checkpoint** — propose commit `chore: pin dependencies and binary targets` (await user).

### Task 2: Module skeleton with empty stubs

**Files:**
- Modify: `src/lib.rs`
- Create: `src/render.rs`, `src/glyphs.rs`, `src/ansi.rs`, `src/detect.rs`, `src/html.rs`

- [ ] **Step 1: Replace `src/lib.rs`**

```rust
pub mod render;
pub mod glyphs;
pub mod ansi;
pub mod detect;

#[cfg(any(test, feature = "html"))]
pub mod html;
```

- [ ] **Step 2: Create empty stubs**

Each of `src/render.rs`, `src/glyphs.rs`, `src/ansi.rs`, `src/detect.rs`, `src/html.rs` gets a single line:

```rust
// stub
```

- [ ] **Step 3: Add bin stubs**

Create `src/bin/apb_bench.rs`:
```rust
fn main() {}
```

Create `src/bin/apb_font_patch.rs`:
```rust
fn main() {}
```

- [ ] **Step 4: Verify**

Run: `cargo build --all-targets`
Expected: clean build.

- [ ] **Step 5: Checkpoint** — propose commit `chore: scaffold module + binary stubs`.

---

## Phase 1 — Core types

### Task 3: Define `Rgb`, `Theme`, `Capability`

**Files:**
- Modify: `src/lib.rs`

- [ ] **Step 1: Append to `src/lib.rs`**

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capability {
    Ascii,
    EighthBlock,
    PatchedSixteenth,
}

impl Capability {
    pub fn sub_positions(self) -> u32 {
        match self {
            Capability::Ascii => 1,
            Capability::EighthBlock => 8,
            Capability::PatchedSixteenth => 16,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub primary: Rgb,
    pub secondary: Rgb,
    pub empty: Rgb,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            primary:   Rgb(88, 166, 255),
            secondary: Rgb(60,  90, 160),
            empty:     Rgb(33,  38,  45),
        }
    }
}
```

- [ ] **Step 2: Verify** — `cargo build`.

- [ ] **Step 3: Checkpoint** — propose commit `feat(types): add Rgb, Capability, Theme`.

### Task 4: Define `Cell` + `CellKind`

**Files:**
- Modify: `src/render.rs`

- [ ] **Step 1: Replace `src/render.rs`**

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellKind {
    Empty,
    PrimaryFull,
    SecondaryFull,
    PrimaryBoundary,
    SecondaryBoundary,
    DegradedOverlap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub kind: CellKind,
    pub sub_fill: u8,
}
```

- [ ] **Step 2: Verify** — `cargo build`.

- [ ] **Step 3: Checkpoint** — propose commit `feat(types): add Cell and CellKind IR`.

---

## Phase 2 — `classify` (pure render core, TDD)

The renderer subdivides the bar into cells, then for each cell decides which `CellKind` and `sub_fill` it carries.

**Algorithm reference (re-stated to avoid out-of-context reading):**

```
N = cap.sub_positions()
T = (width as u32) * N
p1s = round(p1 * T as f64) as u32
p2s = round(p2 * T as f64) as u32   // assumes p2 >= p1

for i in 0..width:
    s = i * N
    e = s + N
    p1_in = (s..e).contains(&p1s)         // strictly inside
    p2_in = (s..e).contains(&p2s)
    if p1_in && p2_in && p1s != p2s: DegradedOverlap with sub_fill = p1s - s
    elif p1_in:  PrimaryBoundary  with sub_fill = p1s - s
    elif p2_in:  SecondaryBoundary with sub_fill = p2s - s
    elif e <= p1s: PrimaryFull
    elif e <= p2s: SecondaryFull
    else: Empty
```

### Task 5: classify — empty width returns empty Vec

**Files:**
- Modify: `src/render.rs`
- Test: `src/render.rs` (inline `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write failing test**

Append to `src/render.rs`:
```rust
use crate::Capability;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_zero_returns_empty() {
        let v = classify(0, 0.5, 0.7, Capability::EighthBlock);
        assert!(v.is_empty());
    }
}
```

- [ ] **Step 2: Run — must fail**

Run: `cargo test render::tests::width_zero_returns_empty`
Expected: compile error — `classify` not found.

- [ ] **Step 3: Implement minimal**

Append above the test module:
```rust
pub fn classify(width: usize, _p1: f64, _p2: f64, _cap: Capability) -> Vec<Cell> {
    Vec::with_capacity(width)
}
```

- [ ] **Step 4: Run — must pass**

Run: `cargo test render::tests::width_zero_returns_empty`
Expected: PASS.

- [ ] **Step 5: Checkpoint** — propose commit `feat(render): classify stub + zero-width case`.

### Task 6: classify — full progress = all PrimaryFull

**Files:**
- Modify: `src/render.rs`

- [ ] **Step 1: Append failing test**

```rust
#[test]
fn full_progress_all_primary_full() {
    let v = classify(5, 1.0, 1.0, Capability::EighthBlock);
    assert_eq!(v.len(), 5);
    assert!(v.iter().all(|c| c.kind == CellKind::PrimaryFull));
}
```

Run: `cargo test render::tests::full_progress_all_primary_full` → FAIL (every cell is currently absent).

- [ ] **Step 2: Implement core loop**

Replace `classify`:
```rust
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
```

Run: `cargo test render::` → PASS.

- [ ] **Step 3: Checkpoint** — propose commit `feat(render): implement classify core loop`.

### Task 7: classify — zero progress = all Empty

**Files:**
- Modify: `src/render.rs`

- [ ] **Step 1: Append failing test**

```rust
#[test]
fn zero_progress_all_empty() {
    let v = classify(5, 0.0, 0.0, Capability::EighthBlock);
    assert!(v.iter().all(|c| c.kind == CellKind::Empty));
}
```

Run → expect PASS already (the algorithm handles it).

- [ ] **Step 2: Checkpoint** — propose commit `test(render): cover zero-progress case`.

### Task 8: classify — boundaries land in distinct cells

**Files:**
- Modify: `src/render.rs`

- [ ] **Step 1: Append failing test**

```rust
#[test]
fn boundaries_in_distinct_cells_7_at_16ths() {
    // The worked example from the spec.
    let v = classify(7, 0.33, 0.67, Capability::PatchedSixteenth);
    assert_eq!(v[0].kind, CellKind::PrimaryFull);
    assert_eq!(v[1].kind, CellKind::PrimaryFull);
    assert_eq!(v[2].kind, CellKind::PrimaryBoundary);
    assert_eq!(v[2].sub_fill, 5);
    assert_eq!(v[3].kind, CellKind::SecondaryFull);
    assert_eq!(v[4].kind, CellKind::SecondaryBoundary);
    assert_eq!(v[4].sub_fill, 11);
    assert_eq!(v[5].kind, CellKind::Empty);
    assert_eq!(v[6].kind, CellKind::Empty);
}
```

- [ ] **Step 2: Run** — `cargo test render::tests::boundaries_in_distinct_cells_7_at_16ths`
Expected: PASS (round(0.33×112)=37, round(0.67×112)=75 — matches the table).

- [ ] **Step 3: Checkpoint** — propose commit `test(render): cover worked example from spec`.

### Task 9: classify — overlap triggers DegradedOverlap

**Files:**
- Modify: `src/render.rs`

- [ ] **Step 1: Append failing test**

```rust
#[test]
fn same_cell_overlap_triggers_degrade() {
    // 13 cells × 8 sub = 104; round(0.12×104)=12; round(0.13×104)=14
    // Cell 1 (range 8..16) contains both 12 and 14 → DegradedOverlap.
    let v = classify(13, 0.12, 0.13, Capability::EighthBlock);
    assert_eq!(v[1].kind, CellKind::DegradedOverlap);
    assert_eq!(v[1].sub_fill, 4);
}
```

Run → expect PASS (algorithm already routes here).

- [ ] **Step 2: Checkpoint** — propose commit `test(render): cover degrade-overlap branch`.

### Task 10: classify — `p1 == p2` does not flip to overlap

**Files:**
- Modify: `src/render.rs`

- [ ] **Step 1: Append failing test**

```rust
#[test]
fn p1_equals_p2_no_secondary_band() {
    let v = classify(8, 0.5, 0.5, Capability::EighthBlock);
    // Both boundaries collapse into one cell, but DegradedOverlap is only used
    // when the two sub-positions differ. Here they're equal → PrimaryBoundary.
    let cell4 = &v[4];
    assert_eq!(cell4.kind, CellKind::PrimaryBoundary);
    // Cells past the boundary are all Empty (no secondary band).
    assert!(v[5..].iter().all(|c| c.kind == CellKind::Empty));
}
```

Wait — `0.5 × 64 = 32`, which is the boundary between cell 3 (24..32) and cell 4 (32..40). Strict `>` and `<` means neither cell contains 32. Adjust the test:

```rust
#[test]
fn p1_equals_p2_no_secondary_band() {
    // Use 0.4 so the sub-position 25.6 → 26 lands strictly inside cell 3 (24..32).
    let v = classify(8, 0.4, 0.4, Capability::EighthBlock);
    assert_eq!(v[3].kind, CellKind::PrimaryBoundary);
    assert!(v[4..].iter().all(|c| c.kind == CellKind::Empty));
}
```

Run → expect PASS.

- [ ] **Step 2: Checkpoint** — propose commit `test(render): cover p1==p2 case`.

### Task 11: classify — proptest invariants

**Files:**
- Modify: `src/render.rs`

- [ ] **Step 1: Append test**

```rust
#[cfg(test)]
mod prop {
    use super::*;
    use proptest::prelude::*;

    fn any_cap() -> impl Strategy<Value = Capability> {
        prop_oneof![
            Just(Capability::Ascii),
            Just(Capability::EighthBlock),
            Just(Capability::PatchedSixteenth),
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
```

Run: `cargo test render::prop` → PASS.

- [ ] **Step 2: Checkpoint** — propose commit `test(render): proptest length + sub_fill bounds`.

---

## Phase 3 — Glyph tables

### Task 12: ASCII glyph table

**Files:**
- Modify: `src/glyphs.rs`

- [ ] **Step 1: Replace `src/glyphs.rs`**

```rust
use crate::{render::CellKind, Capability};

pub fn glyph_for(kind: CellKind, sub_fill: u8, cap: Capability) -> char {
    match cap {
        Capability::Ascii            => ascii(kind),
        Capability::EighthBlock      => eighth(kind, sub_fill),
        Capability::PatchedSixteenth => sixteenth(kind, sub_fill),
    }
}

fn ascii(kind: CellKind) -> char {
    match kind {
        CellKind::Empty                                                                   => ' ',
        CellKind::PrimaryFull   | CellKind::SecondaryFull                                 => '█',
        // Boundaries collapse to "show the cell as filled" at 1-cell granularity.
        CellKind::PrimaryBoundary | CellKind::SecondaryBoundary | CellKind::DegradedOverlap => '█',
    }
}

fn eighth(kind: CellKind, sub_fill: u8) -> char {
    const TABLE: [char; 9] = [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];
    match kind {
        CellKind::Empty                                  => ' ',
        CellKind::PrimaryFull | CellKind::SecondaryFull  => '█',
        CellKind::PrimaryBoundary
        | CellKind::SecondaryBoundary
        | CellKind::DegradedOverlap                       => TABLE[sub_fill.min(8) as usize],
    }
}

fn sixteenth(kind: CellKind, sub_fill: u8) -> char {
    match kind {
        CellKind::Empty                                  => ' ',
        CellKind::PrimaryFull | CellKind::SecondaryFull  => '█',
        CellKind::PrimaryBoundary | CellKind::DegradedOverlap
            => char::from_u32(0xE100 + sub_fill.min(16) as u32).unwrap(),
        CellKind::SecondaryBoundary
            => char::from_u32(0xE120 + sub_fill.min(16) as u32).unwrap(),
    }
}
```

- [ ] **Step 2: Write tests**

Append to `src/glyphs.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::CellKind;

    #[test] fn ascii_full() { assert_eq!(glyph_for(CellKind::PrimaryFull, 0, Capability::Ascii), '█'); }
    #[test] fn eighth_half() { assert_eq!(glyph_for(CellKind::PrimaryBoundary, 4, Capability::EighthBlock), '▌'); }
    #[test] fn eighth_zero_sub_is_space() { assert_eq!(glyph_for(CellKind::PrimaryBoundary, 0, Capability::EighthBlock), ' '); }
    #[test] fn sixteenth_primary() { assert_eq!(glyph_for(CellKind::PrimaryBoundary, 5, Capability::PatchedSixteenth) as u32, 0xE105); }
    #[test] fn sixteenth_secondary() { assert_eq!(glyph_for(CellKind::SecondaryBoundary, 11, Capability::PatchedSixteenth) as u32, 0xE12B); }
}
```

Run: `cargo test glyphs::` → PASS.

- [ ] **Step 3: Checkpoint** — propose commit `feat(glyphs): three capability tables`.

---

## Phase 4 — `Bar` builder

### Task 13: `Bar` struct + builder methods

**Files:**
- Modify: `src/lib.rs`

- [ ] **Step 1: Append to `src/lib.rs`**

```rust
use crate::render::{classify, Cell};

#[derive(Clone, Debug)]
pub struct Bar {
    width: usize,
    primary: f64,
    secondary: f64,
    theme: Theme,
    capability: Capability,
}

impl Bar {
    pub fn new(width: usize) -> Self {
        Self {
            width,
            primary: 0.0,
            secondary: 0.0,
            theme: Theme::default(),
            capability: detect::detect(),
        }
    }
    pub fn primary(mut self, v: f64)        -> Self { self.primary = v; self }
    pub fn secondary(mut self, v: f64)      -> Self { self.secondary = v; self }
    pub fn theme(mut self, t: Theme)        -> Self { self.theme = t; self }
    pub fn capability(mut self, c: Capability) -> Self { self.capability = c; self }

    fn sanitized(&self) -> (f64, f64) {
        let s = |x: f64| if x.is_nan() { 0.0 } else { x.clamp(0.0, 1.0) };
        let (a, b) = (s(self.primary), s(self.secondary));
        if a > b { (b, a) } else { (a, b) }
    }

    pub fn cells(&self) -> Vec<Cell> {
        let (p1, p2) = self.sanitized();
        classify(self.width, p1, p2, self.capability)
    }
}

pub use render::{Cell, CellKind};
```

Note: `detect::detect()` does not exist yet (Task 18 covers it). Stub it temporarily — modify `src/detect.rs`:

```rust
use crate::Capability;
pub fn detect() -> Capability { Capability::EighthBlock }
```

Run: `cargo build` → clean.

- [ ] **Step 2: Add sanitization tests**

Append to `src/lib.rs`:
```rust
#[cfg(test)]
mod bar_tests {
    use super::*;

    #[test] fn clamps_out_of_range() {
        let (p1, p2) = Bar::new(10).primary(-1.0).secondary(2.0).sanitized();
        assert_eq!(p1, 0.0); assert_eq!(p2, 1.0);
    }
    #[test] fn swaps_when_primary_above_secondary() {
        let (p1, p2) = Bar::new(10).primary(0.9).secondary(0.1).sanitized();
        assert_eq!(p1, 0.1); assert_eq!(p2, 0.9);
    }
    #[test] fn nan_becomes_zero() {
        let (p1, _) = Bar::new(10).primary(f64::NAN).sanitized();
        assert_eq!(p1, 0.0);
    }
    #[test] fn zero_width_no_cells() {
        assert!(Bar::new(0).primary(0.5).secondary(0.7).cells().is_empty());
    }
}
```

Run: `cargo test bar_tests::` → PASS.

- [ ] **Step 3: Checkpoint** — propose commit `feat(bar): builder + input sanitization`.

---

## Phase 5 — ANSI encoding

### Task 14: `ansi::encode` minimal path

**Files:**
- Modify: `src/ansi.rs`

- [ ] **Step 1: Replace `src/ansi.rs`**

```rust
use crate::{render::{Cell, CellKind}, glyphs::glyph_for, Capability, Rgb, Theme};

pub fn encode(cells: &[Cell], theme: &Theme, cap: Capability) -> String {
    let mut out = String::with_capacity(cells.len() * 24);
    let mut last_fg: Option<Rgb> = None;
    let mut last_bg: Option<Rgb> = None;

    for c in cells {
        let (fg, bg, empty) = layer_colors(c.kind, theme);
        if empty {
            // Empty cells: reset SGR, emit space, forget last colors so the next colored cell re-emits.
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
```

- [ ] **Step 2: Add tests**

Append to `src/ansi.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Cell;

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
        // Exactly one SGR (set fg), then "██", then reset.
        assert_eq!(out.matches("\x1b[38;2;").count(), 1);
        assert!(out.ends_with("██\x1b[0m"));
    }
}
```

Run: `cargo test ansi::` → PASS.

- [ ] **Step 3: Checkpoint** — propose commit `feat(ansi): encode with SGR run-length merging`.

### Task 15: `Bar::render()`

**Files:**
- Modify: `src/lib.rs`

- [ ] **Step 1: Append to `impl Bar`**

```rust
    pub fn render(&self) -> String {
        ansi::encode(&self.cells(), &self.theme, self.capability)
    }
```

- [ ] **Step 2: Add integration test**

Append to `bar_tests`:
```rust
    #[test] fn render_is_non_empty_for_nonzero_width() {
        let s = Bar::new(8).primary(0.5).secondary(0.7).render();
        assert!(!s.is_empty());
    }
```

Run: `cargo test bar_tests::render_is_non_empty_for_nonzero_width` → PASS.

- [ ] **Step 3: Checkpoint** — propose commit `feat(bar): render() integration`.

### Task 16: Insta snapshots for ANSI output

**Files:**
- Create: `tests/snapshots.rs`

- [ ] **Step 1: Create file**

```rust
use almost_perfect_progressbar::{Bar, Capability};

fn fixture(width: usize, p1: f64, p2: f64, cap: Capability) -> String {
    Bar::new(width).primary(p1).secondary(p2).capability(cap).render()
}

#[test] fn snap_7_33_67_sixteenth() {
    insta::assert_snapshot!(fixture(7, 0.33, 0.67, Capability::PatchedSixteenth));
}
#[test] fn snap_13_33_67_eighth() {
    insta::assert_snapshot!(fixture(13, 0.33, 0.67, Capability::EighthBlock));
}
#[test] fn snap_13_12_13_eighth_degrade() {
    insta::assert_snapshot!(fixture(13, 0.12, 0.13, Capability::EighthBlock));
}
#[test] fn snap_40_50_50_sixteenth() {
    insta::assert_snapshot!(fixture(40, 0.50, 0.50, Capability::PatchedSixteenth));
}
#[test] fn snap_100_01_99_ascii() {
    insta::assert_snapshot!(fixture(100, 0.01, 0.99, Capability::Ascii));
}
```

- [ ] **Step 2: Generate snapshots**

Run: `cargo test --test snapshots` (will fail first time, creating `.snap.new` files).
Then: `cargo insta accept` (or rename `.snap.new` → `.snap` and re-run).

Expected: all five snapshots pass on rerun.

- [ ] **Step 3: Sanity-check rendering**

Run: `cargo run --example basic` — does not yet exist; defer visual sanity to Task 22.

- [ ] **Step 4: Checkpoint** — propose commit `test(ansi): five-fixture insta snapshots`.

---

## Phase 6 — Capability detection

### Task 17: `detect::detect()` with priority chain

**Files:**
- Modify: `src/detect.rs`

- [ ] **Step 1: Replace `src/detect.rs`**

```rust
use crate::Capability;
use std::env;

pub fn detect() -> Capability {
    detect_from_env(|k| env::var(k).ok())
}

fn detect_from_env(get: impl Fn(&str) -> Option<String>) -> Capability {
    if let Some(v) = get("APB_FORCE_CAP") {
        return match v.as_str() {
            "ascii"      => Capability::Ascii,
            "eighth"     => Capability::EighthBlock,
            "sixteenth"  => Capability::PatchedSixteenth,
            _            => Capability::EighthBlock,
        };
    }
    if get("APB_FONT_PATCHED").as_deref() == Some("1") {
        return Capability::PatchedSixteenth;
    }
    if get("WEZTERM_PANE").is_some()
        || get("GHOSTTY_RESOURCES_DIR").is_some()
        || get("KITTY_WINDOW_ID").is_some()
    {
        return Capability::EighthBlock;
    }
    Capability::EighthBlock
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + '_ {
        let map: HashMap<&str, &str> = pairs.iter().copied().collect();
        move |k| map.get(k).map(|s| s.to_string())
    }

    #[test] fn force_overrides_everything() {
        assert_eq!(detect_from_env(env(&[("APB_FORCE_CAP","ascii"), ("APB_FONT_PATCHED","1")])), Capability::Ascii);
    }
    #[test] fn patched_flag_wins_over_term_heuristic() {
        assert_eq!(detect_from_env(env(&[("APB_FONT_PATCHED","1"), ("WEZTERM_PANE","x")])), Capability::PatchedSixteenth);
    }
    #[test] fn default_is_eighth() {
        assert_eq!(detect_from_env(env(&[])), Capability::EighthBlock);
    }
}
```

Run: `cargo test detect::` → PASS.

- [ ] **Step 2: Checkpoint** — propose commit `feat(detect): env-based capability priority chain`.

---

## Phase 7 — HTML export (test-only)

### Task 18: `Bar::to_html()` for visual regression

**Files:**
- Modify: `src/html.rs`, `src/lib.rs`

- [ ] **Step 1: Replace `src/html.rs`**

```rust
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
    match c { ' ' => "&nbsp;".into(), c => c.to_string() }
}
```

- [ ] **Step 2: Add an integration test that writes one HTML file**

Append to `tests/snapshots.rs`:
```rust
#[test] fn html_fixture_renders() {
    use almost_perfect_progressbar::{Theme, html};
    let bar = Bar::new(7).primary(0.33).secondary(0.67).capability(Capability::PatchedSixteenth);
    let html = html::to_html(&bar.cells(), &Theme::default(), Capability::PatchedSixteenth);
    std::fs::create_dir_all("tests/snapshots/visual").ok();
    std::fs::write("tests/snapshots/visual/7-33-67-sixteenth.html", &html).unwrap();
    assert!(html.starts_with("<pre"));
}
```

Run: `cargo test --test snapshots html_fixture_renders` → PASS, file written.

- [ ] **Step 3: Make `html` reachable for tests**

Modify `src/lib.rs` — change cfg gate:
```rust
#[cfg(any(test, feature = "html"))]
pub mod html;
```

(Already done in Task 2.) Ensure the integration test does not break under `cargo test` (the cfg is satisfied for the test profile via `cfg(test)` at the bin/lib level — when run as an integration test, depend via `feature = "html"`. Adjust `Cargo.toml`:)

In `[features]`:
```toml
[features]
html = []
```

And in `tests/snapshots.rs` use `#[cfg(feature = "html")]` on the new test, or run with `cargo test --features html`. Document this in the test file with a top-of-file comment.

Run: `cargo test --features html --test snapshots` → all pass.

- [ ] **Step 4: Checkpoint** — propose commit `feat(html): test-only HTML export`.

---

## Phase 8 — Examples

### Task 19: examples/basic.rs

**Files:**
- Create: `examples/basic.rs`

- [ ] **Step 1: Create file**

```rust
use almost_perfect_progressbar::{Bar, Capability};

fn main() {
    for cap in [Capability::Ascii, Capability::EighthBlock, Capability::PatchedSixteenth] {
        println!("{:>20?}  {}", cap, Bar::new(40).primary(0.33).secondary(0.67).capability(cap).render());
    }
}
```

- [ ] **Step 2: Run** — `cargo run --example basic`.

Expected: three lines, each a 40-cell bar.

- [ ] **Step 3: Checkpoint** — propose commit `feat(examples): basic`.

### Task 20: examples/animated.rs

**Files:**
- Create: `examples/animated.rs`

- [ ] **Step 1: Create file**

```rust
use almost_perfect_progressbar::{Bar, Capability};
use std::{thread::sleep, time::Duration};

fn main() {
    let cap = Capability::EighthBlock;
    print!("\x1b[?25l"); // hide cursor
    for tick in 0..=100u32 {
        let p1 = tick as f64 / 100.0;
        let p2 = (p1 + 0.12).min(1.0);
        print!("\r{}", Bar::new(40).primary(p1).secondary(p2).capability(cap).render());
        std::io::Write::flush(&mut std::io::stdout()).ok();
        sleep(Duration::from_millis(40));
    }
    println!("\x1b[?25h");
}
```

- [ ] **Step 2: Run** — `cargo run --example animated`.

Expected: a bar fills smoothly over 4 seconds.

- [ ] **Step 3: Checkpoint** — propose commit `feat(examples): animated`.

### Task 21: examples/narrow.rs

**Files:**
- Create: `examples/narrow.rs`

- [ ] **Step 1: Create file**

```rust
use almost_perfect_progressbar::{Bar, Capability};

fn main() {
    for width in [4, 7, 13, 25] {
        println!("\n--- width = {width} ---");
        for pct in (0..=100).step_by(5) {
            let p1 = pct as f64 / 100.0;
            let p2 = (p1 + 0.10).min(1.0);
            println!("{pct:3}% | {}", Bar::new(width).primary(p1).secondary(p2).capability(Capability::PatchedSixteenth).render());
        }
    }
}
```

- [ ] **Step 2: Run** — `cargo run --example narrow`.

- [ ] **Step 3: Checkpoint** — propose commit `feat(examples): narrow widths`.

### Task 22: examples/degrade_ladder.rs

**Files:**
- Create: `examples/degrade_ladder.rs`

- [ ] **Step 1: Create file**

```rust
use almost_perfect_progressbar::{Bar, Capability};

fn main() {
    let p1 = 0.33;
    let p2 = 0.67;
    for width in [7, 13, 25, 50] {
        println!("\nwidth = {width}");
        for cap in [Capability::Ascii, Capability::EighthBlock, Capability::PatchedSixteenth] {
            println!("  {:>18?}  {}", cap, Bar::new(width).primary(p1).secondary(p2).capability(cap).render());
        }
    }
}
```

- [ ] **Step 2: Run** — `cargo run --example degrade_ladder`.

- [ ] **Step 3: Checkpoint** — propose commit `feat(examples): degrade ladder`.

---

## Phase 9 — Interactive bench TUI

### Task 23: `apb-bench` skeleton (raw mode + clean draw + exit)

**Files:**
- Modify: `src/bin/apb_bench.rs`

- [ ] **Step 1: Replace `src/bin/apb_bench.rs`**

```rust
use almost_perfect_progressbar::{Bar, Capability};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    style::Print,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use std::io::{stdout, Write};

struct State { width: usize, p1: f64, p2: f64, cap: Capability }

impl State {
    fn draw(&self) -> std::io::Result<()> {
        let mut out = stdout();
        execute!(out, MoveTo(0,0), Clear(ClearType::All))?;
        execute!(out, Print(format!("width={}  primary={:.3}  secondary={:.3}  cap={:?}\r\n", self.width, self.p1, self.p2, self.cap)))?;
        execute!(out, Print("[ "), Print(Bar::new(self.width).primary(self.p1).secondary(self.p2).capability(self.cap).render()), Print(" ]\r\n"))?;
        execute!(out, Print("← → adjust primary | Shift+← → adjust secondary | + - resize | t cycle cap | q quit\r\n"))?;
        out.flush()
    }
}

fn main() -> std::io::Result<()> {
    let mut state = State { width: 40, p1: 0.33, p2: 0.67, cap: Capability::EighthBlock };
    enable_raw_mode()?;
    execute!(stdout(), Hide)?;
    let res = (|| -> std::io::Result<()> {
        loop {
            state.draw()?;
            if let Event::Key(k) = event::read()? {
                let shift = k.modifiers.contains(KeyModifiers::SHIFT);
                match (k.code, shift) {
                    (KeyCode::Char('q'), _) | (KeyCode::Esc, _) => break,
                    (KeyCode::Left,  false) => state.p1 = (state.p1 - 0.01).max(0.0),
                    (KeyCode::Right, false) => state.p1 = (state.p1 + 0.01).min(state.p2),
                    (KeyCode::Left,  true)  => state.p2 = (state.p2 - 0.01).max(state.p1),
                    (KeyCode::Right, true)  => state.p2 = (state.p2 + 0.01).min(1.0),
                    (KeyCode::Char('+'), _) | (KeyCode::Char('='), _) => state.width = (state.width + 1).min(120),
                    (KeyCode::Char('-'), _) => state.width = state.width.saturating_sub(1).max(1),
                    (KeyCode::Char('t'), _) => state.cap = match state.cap {
                        Capability::Ascii            => Capability::EighthBlock,
                        Capability::EighthBlock      => Capability::PatchedSixteenth,
                        Capability::PatchedSixteenth => Capability::Ascii,
                    },
                    _ => {}
                }
            }
        }
        Ok(())
    })();
    execute!(stdout(), Show)?;
    disable_raw_mode()?;
    res
}
```

- [ ] **Step 2: Smoke test**

Run: `cargo run --bin apb-bench`. Press q to exit. Confirm the bar updates as you press ← → +/− t.

- [ ] **Step 3: Checkpoint** — propose commit `feat(bench): interactive TUI`.

---

## Phase 10 — Font patcher

### Task 24: `apb-font-patch` CLI skeleton (clap)

**Files:**
- Modify: `src/bin/apb_font_patch.rs`

- [ ] **Step 1: Replace `src/bin/apb_font_patch.rs`**

```rust
use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "Patch a font: add PUA glyphs for 1/16-cell horizontal fills")]
struct Args {
    /// Input font file (TTF/OTF)
    input: PathBuf,
    /// Output font file
    #[arg(short, long)]
    output: PathBuf,
    /// Sub-positions per cell (only 16 supported initially)
    #[arg(long, default_value_t = 16)]
    sub_positions: u32,
    /// Error out if PUA range is already used
    #[arg(long)]
    no_overwrite: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let bytes = std::fs::read(&args.input).with_context(|| format!("reading {}", args.input.display()))?;
    let _font = write_fonts::read::FontRef::new(&bytes).context("parsing font")?;
    anyhow::ensure!(args.sub_positions == 16, "only --sub-positions 16 is implemented");
    // TODO in next task: generate glyphs and write.
    eprintln!("(stub) parsed {} bytes; output {} not yet written", bytes.len(), args.output.display());
    Ok(())
}
```

- [ ] **Step 2: Verify CLI prints help**

Run: `cargo run --bin apb-font-patch -- --help`. Expected: help text shown.

- [ ] **Step 3: Checkpoint** — propose commit `feat(font-patch): CLI skeleton`.

### Task 25: Generate PUA fill glyphs and write font

**Files:**
- Modify: `src/bin/apb_font_patch.rs`

- [ ] **Step 1: Replace `main`**

```rust
use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use write_fonts::{
    tables::{cmap::CmapSubtable, glyf::SimpleGlyph, hmtx::LongMetric},
    BuilderError, FontBuilder,
};

#[derive(Parser)]
#[command(version, about = "Patch a font: add PUA glyphs for 1/16-cell horizontal fills")]
struct Args {
    input: PathBuf,
    #[arg(short, long)] output: PathBuf,
    #[arg(long, default_value_t = 16)] sub_positions: u32,
    #[arg(long)] no_overwrite: bool,
}

const PRIMARY_BASE:   u32 = 0xE100;
const SECONDARY_BASE: u32 = 0xE120;

fn main() -> Result<()> {
    let args = Args::parse();
    anyhow::ensure!(args.sub_positions == 16, "only --sub-positions 16 is implemented");

    let bytes = std::fs::read(&args.input).with_context(|| format!("reading {}", args.input.display()))?;
    let font  = write_fonts::read::FontRef::new(&bytes).context("parsing font")?;

    // Read em square / advance from the input font's head + hhea tables.
    let units_per_em = font.head().context("head table")?.units_per_em();
    let advance      = font.hhea().context("hhea table")?.advance_width_max();
    let ascender     = font.hhea()?.ascender();
    let descender    = font.hhea()?.descender();
    let cell_h = (ascender - descender) as i32;
    let cell_w = advance as i32;

    // For each sub_fill k in 0..=16, build a rectangle outline filling left k/16.
    // For SECONDARY layer the rectangle has identical geometry (color is the renderer's job);
    // we emit them as two separate codepoints so the consumer can choose without re-shaping.
    let mut new_glyphs = Vec::new();
    for k in 0..=16u32 {
        let glyph = rectangle_outline(0, descender as i32, (cell_w * k as i32) / 16, ascender as i32)?;
        new_glyphs.push((PRIMARY_BASE   + k, glyph.clone(), advance));
        new_glyphs.push((SECONDARY_BASE + k, glyph,         advance));
    }

    let _ = units_per_em; // silence warning if unused below
    let _ = cell_h;

    let mut builder = FontBuilder::new();
    builder.copy_missing_tables(font);

    // Append new glyphs to glyf + loca; extend hmtx; extend cmap; bump maxp.numGlyphs.
    // (Implementation cribs from write-fonts examples — see docs/notes/font-patch.md)
    // For brevity in this plan: the helper add_pua_glyphs encapsulates this.
    add_pua_glyphs(&mut builder, font, &new_glyphs, args.no_overwrite)?;

    let out = builder.build();
    std::fs::write(&args.output, &out).with_context(|| format!("writing {}", args.output.display()))?;
    eprintln!("wrote {} bytes", out.len());
    Ok(())
}

fn rectangle_outline(x0: i32, y0: i32, x1: i32, y1: i32) -> Result<SimpleGlyph> {
    use write_fonts::tables::glyf::{Contour, Point};
    let contour = Contour::new(vec![
        Point::on_curve(x0, y0),
        Point::on_curve(x1, y0),
        Point::on_curve(x1, y1),
        Point::on_curve(x0, y1),
    ]);
    Ok(SimpleGlyph::from_contours(vec![contour])?)
}

fn add_pua_glyphs(
    _builder: &mut FontBuilder,
    _font: write_fonts::read::FontRef,
    _glyphs: &[(u32, SimpleGlyph, u16)],
    _no_overwrite: bool,
) -> Result<()> {
    // Implementation TODO in next task — splitting kept small so the reviewer can sanity-check
    // the geometry layer before the cmap/hmtx surgery.
    anyhow::bail!("add_pua_glyphs not yet implemented")
}
```

> The `write-fonts` API surface for "append glyph + extend cmap" varies between minor versions. The next task implements `add_pua_glyphs` after consulting `cargo doc --open -p write-fonts` (the runtime engineer must verify type/method names match the pinned version).

- [ ] **Step 2: Build (will compile but the patcher errors at runtime)**

Run: `cargo build --bin apb-font-patch`.

- [ ] **Step 3: Checkpoint** — propose commit `feat(font-patch): glyph geometry + tables read`.

### Task 26: Implement `add_pua_glyphs`

**Files:**
- Modify: `src/bin/apb_font_patch.rs`

- [ ] **Step 1: Replace `add_pua_glyphs`**

The exact API for inserting into `cmap` and appending to `glyf`/`loca`/`hmtx`/`maxp` differs by `write-fonts` version. The engineer must:

1. Open `cargo doc --open -p write-fonts` and locate the writer types for `Cmap`, `Glyf`, `Loca`, `Hmtx`, `Maxp`.
2. Read each table from the input `FontRef`, clone its records into the writer type.
3. For each `(codepoint, glyph, advance)` in `new_glyphs`:
   - Append `glyph` to glyf; record its loca offset.
   - Append `LongMetric { advance, side_bearing: 0 }` to hmtx.
   - Add cmap subtable entry mapping `codepoint → glyph_id`.
   - Bump maxp.numGlyphs.
4. If `no_overwrite` and any target codepoint already maps somewhere, `anyhow::bail!`.

Pseudocode skeleton (fill in the actual types):

```rust
fn add_pua_glyphs(
    builder: &mut FontBuilder,
    font: write_fonts::read::FontRef,
    new_glyphs: &[(u32, SimpleGlyph, u16)],
    no_overwrite: bool,
) -> Result<()> {
    let mut glyf_w = font.glyf()?.to_writer();
    let mut hmtx_w = font.hmtx()?.to_writer();
    let mut cmap_w = font.cmap()?.to_writer();
    let mut maxp_w = font.maxp()?.to_writer();

    let mut next_glyph_id = maxp_w.num_glyphs();
    for (cp, glyph, advance) in new_glyphs {
        if no_overwrite && cmap_w.maps(*cp) {
            anyhow::bail!("codepoint U+{:04X} already mapped", cp);
        }
        glyf_w.push(glyph.clone());
        hmtx_w.push_metric(LongMetric { advance: *advance, side_bearing: 0 });
        cmap_w.add_mapping(*cp, next_glyph_id);
        next_glyph_id += 1;
    }
    maxp_w.set_num_glyphs(next_glyph_id);

    builder.add_table(&glyf_w.build())?;
    builder.add_table(&hmtx_w.build())?;
    builder.add_table(&cmap_w.build())?;
    builder.add_table(&maxp_w.build())?;
    Ok(())
}
```

- [ ] **Step 2: Round-trip test**

Create `tests/font_patch.rs`:

```rust
#[cfg(feature = "font-patch-integration")]
#[test]
fn patched_font_loads_back() {
    // Provide a small fixture font under tests/fixtures/Sample.ttf
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_apb-font-patch"))
        .args(["tests/fixtures/Sample.ttf", "-o", "tests/fixtures/Sample.patched.ttf"])
        .status().unwrap();
    assert!(status.success());

    let bytes = std::fs::read("tests/fixtures/Sample.patched.ttf").unwrap();
    let face  = ttf_parser::Face::parse(&bytes, 0).unwrap();
    for cp in 0xE100..=0xE110u32 {
        let ch = char::from_u32(cp).unwrap();
        assert!(face.glyph_index(ch).is_some(), "missing PUA {:04X}", cp);
    }
    for cp in 0xE120..=0xE130u32 {
        let ch = char::from_u32(cp).unwrap();
        assert!(face.glyph_index(ch).is_some(), "missing PUA {:04X}", cp);
    }
}
```

Add to `Cargo.toml`:
```toml
[features]
html = []
font-patch-integration = []
```

Place a small free-licensed font at `tests/fixtures/Sample.ttf` (e.g., subset of JetBrains Mono Regular). Document its source in `tests/fixtures/README.md`.

Run: `cargo test --features font-patch-integration --test font_patch` → PASS.

- [ ] **Step 3: Checkpoint** — propose commit `feat(font-patch): append PUA glyphs + round-trip test`.

---

## Phase 11 — Build a real patched JetBrains Mono

### Task 27: Place source font and emit patched build

**Files:**
- Create: `fonts/JetBrainsMono-Regular.ttf` (source — user downloads)
- Create: `scripts/build-font.sh`

- [ ] **Step 1: Document source**

Create `fonts/README.md`:
```markdown
# Fonts

`JetBrainsMono-Regular.ttf` is sourced from
https://github.com/JetBrains/JetBrainsMono/releases (Apache 2.0).
Download and place it at this path before running `scripts/build-font.sh`.
```

- [ ] **Step 2: Create build script**

`scripts/build-font.sh`:
```bash
#!/usr/bin/env bash
set -euo pipefail
cargo run --release --bin apb-font-patch -- \
    fonts/JetBrainsMono-Regular.ttf \
    -o fonts/JetBrainsMono-APB.ttf
echo "Built fonts/JetBrainsMono-APB.ttf"
echo "Install it: copy to ~/Library/Fonts (macOS) or ~/.local/share/fonts (Linux), then export APB_FONT_PATCHED=1"
```

`chmod +x scripts/build-font.sh`.

- [ ] **Step 3: Run** — `scripts/build-font.sh` (requires font present).

- [ ] **Step 4: Checkpoint** — propose commit `feat(fonts): build script for patched JetBrains Mono`.

---

## Phase 12 — Asciinema casts and README

### Task 28: Record three casts

**Files:**
- Create: `casts/01-overview.cast`, `casts/02-precision.cast`, `casts/03-degrade.cast`
- Create: `scripts/record-casts.sh`

- [ ] **Step 1: Script**

```bash
#!/usr/bin/env bash
set -euo pipefail
mkdir -p casts
asciinema rec -c "cargo run -q --example basic"           casts/01-overview.cast
asciinema rec -c "cargo run -q --example narrow"          casts/02-precision.cast
asciinema rec -c "cargo run -q --example degrade_ladder"  casts/03-degrade.cast
```

`chmod +x scripts/record-casts.sh`.

- [ ] **Step 2: Run** — record each cast manually (`asciinema` must be installed; the engineer should confirm `which asciinema` first).

- [ ] **Step 3: Checkpoint** — propose commit `chore(casts): record overview/precision/degrade casts`.

### Task 29: README

**Files:**
- Create: `README.md`

- [ ] **Step 1: Write README**

```markdown
# almost-perfect-progressbar

A research demo for a two-value overlay progress bar with 1% precision at 7-cell width,
using a custom-patched font (1/16-cell horizontal sub-position) on modern terminals.

## Quick start

```bash
cargo run --example basic
cargo run --example animated
cargo run --example narrow
cargo run --example degrade_ladder
cargo run --bin apb-bench
```

## Patched font (for max precision)

```bash
# Place fonts/JetBrainsMono-Regular.ttf (see fonts/README.md)
scripts/build-font.sh
# Install fonts/JetBrainsMono-APB.ttf in your OS font manager.
export APB_FONT_PATCHED=1
cargo run --example narrow
```

## Architecture

See `docs/superpowers/specs/2026-05-18-almost-perfect-progressbar-design.md`.

## Asciinema casts

- `casts/01-overview.cast`
- `casts/02-precision.cast`
- `casts/03-degrade.cast`

Render with `asciinema play casts/01-overview.cast`.
```

- [ ] **Step 2: Checkpoint** — propose commit `docs: README with quickstart and patched-font flow`.

---

## Self-review

### Spec coverage

| Spec section | Implemented in |
|---|---|
| §1 Goals / non-goals | Plan scope statement |
| §2 Fixed decisions | Tasks 1, 3, 12, 13, 25 |
| §3 Architecture | Module layout (file map) |
| §4 Components | Tasks 3–4 (types), 12 (glyphs), 14 (ansi), 17 (detect), 23 (bench), 24–26 (font-patch) |
| §5 Data flow (worked example) | Task 8 (test) + Task 16 (snapshot) |
| §6 Error handling | Task 13 (sanitization), Task 24 (anyhow) |
| §7 Testing strategy | Tasks 5–11 (unit + proptest), 16 (insta), 18 (HTML), 26 (round-trip) |
| §8 Dependencies | Task 1 |
| asciinema deliverable | Task 28 |
| examples deliverable | Tasks 19–22 |
| bench deliverable | Task 23 |
| split-cell (feature) | Out of initial scope per spec — no task |

### Placeholder check

- Task 25's `add_pua_glyphs` is a known stub completed in Task 26. Documented that the engineer must consult `write-fonts` docs because the API signature is version-sensitive — this is necessary realism, not laziness, but flagged here so reviewers know to expect the engineer to revisit it.
- All other tasks have full code blocks.

### Type consistency

- `Capability::sub_positions` returns `u32` (Task 3), consumed as `u32` in `classify` (Task 6), as `u8` (with `.min(N)`) in glyph table casts (Task 12). Consistent.
- `Cell { kind, sub_fill: u8 }` is used the same way in `render.rs`, `glyphs.rs`, `ansi.rs`, `html.rs`. Consistent.
- `glyph_for(kind, sub_fill, cap)` signature is the same everywhere it's called.

---

**Plan complete.**
