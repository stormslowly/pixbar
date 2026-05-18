//! Patch a TrueType font with PUA glyphs for 1/16-cell horizontal fills.
//!
//! The 17 boundary fills (k = 0..=16) are placed at two PUA ranges:
//! - `U+E100..=U+E110` — primary layer
//! - `U+E120..=U+E130` — secondary layer
//!
//! Both ranges share the same glyph geometry; the renderer picks the color
//! via ANSI escapes.

use anyhow::{Context, Result, anyhow, bail, ensure};
use clap::Parser;
use std::path::PathBuf;

use write_fonts::{
    FontBuilder,
    from_obj::{FromTableRef, ToOwnedTable},
    read::{FontRef, TableProvider, types::GlyphId},
    tables::{
        cmap::Cmap,
        glyf::{GlyfLocaBuilder, Glyph, SimpleGlyph},
        head::Head,
        hhea::Hhea,
        hmtx::{Hmtx, LongMetric},
        loca::LocaFormat,
        maxp::Maxp,
    },
};

const PRIMARY_BASE: u32 = 0xE100;
const SECONDARY_BASE: u32 = 0xE120;
const NUM_STEPS: u32 = 17; // k = 0..=16

#[derive(Parser)]
#[command(
    version,
    about = "Patch a font: add PUA glyphs for 1/16-cell horizontal fills"
)]
struct Args {
    /// Input font file (TrueType / .ttf).
    input: PathBuf,
    /// Output file path.
    #[arg(short, long)]
    output: PathBuf,
    /// Number of horizontal sub-positions (only 16 is implemented).
    #[arg(long, default_value_t = 16)]
    sub_positions: u32,
    /// Error out if any target PUA codepoint is already mapped.
    #[arg(long)]
    no_overwrite: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    ensure!(
        args.sub_positions == 16,
        "only --sub-positions 16 is implemented (got {})",
        args.sub_positions
    );

    let bytes = std::fs::read(&args.input)
        .with_context(|| format!("reading {}", args.input.display()))?;
    let font = FontRef::new(&bytes).map_err(|e| anyhow!("parsing input font: {e}"))?;

    let patched = patch(&font, args.no_overwrite)?;
    std::fs::write(&args.output, &patched)
        .with_context(|| format!("writing {}", args.output.display()))?;

    eprintln!(
        "patched {} ({} bytes) -> {} ({} bytes), added {} PUA glyphs",
        args.input.display(),
        bytes.len(),
        args.output.display(),
        patched.len(),
        NUM_STEPS * 2,
    );
    Ok(())
}

fn patch(font: &FontRef<'_>, no_overwrite: bool) -> Result<Vec<u8>> {
    let head_src = font.head().context("missing 'head' table")?;
    let hhea_src = font.hhea().context("missing 'hhea' table")?;
    let maxp_src = font.maxp().context("missing 'maxp' table")?;
    let hmtx_src = font.hmtx().context("missing 'hmtx' table")?;
    let glyf_src = font.glyf().context("missing 'glyf' table")?;
    let loca_src = font.loca(None).context("missing 'loca' table")?;
    let cmap_src = font.cmap().context("missing 'cmap' table")?;

    let upem = head_src.units_per_em();
    let ascender = hhea_src.ascender().to_i16();
    let num_glyphs_old = maxp_src.num_glyphs();
    ensure!(num_glyphs_old > 0, "font has no glyphs");

    // Existing cmap as `(codepoint, glyph_id)` pairs, deduplicated by codepoint.
    let mut existing_mappings: Vec<(char, GlyphId)> = Vec::new();
    {
        let mut seen = std::collections::HashSet::new();
        for record in cmap_src.encoding_records() {
            let Ok(subtable) = record.subtable(cmap_src.offset_data()) else {
                continue;
            };
            for (cp, gid) in subtable.iter() {
                if let Some(ch) = char::from_u32(cp) {
                    if seen.insert(cp) {
                        existing_mappings.push((ch, gid));
                    }
                }
            }
        }
    }

    if no_overwrite {
        let conflict = existing_mappings.iter().find_map(|(ch, _)| {
            let cp = *ch as u32;
            let in_primary = (PRIMARY_BASE..PRIMARY_BASE + NUM_STEPS).contains(&cp);
            let in_secondary = (SECONDARY_BASE..SECONDARY_BASE + NUM_STEPS).contains(&cp);
            (in_primary || in_secondary).then_some(cp)
        });
        if let Some(cp) = conflict {
            bail!("--no-overwrite: input cmap already maps U+{cp:04X}");
        }
    }

    // Determine the new glyph cell width. Use the first non-zero advance from
    // hmtx if available (typically the .notdef or space glyph), else fall back
    // to UPM/2 — anything reasonable works since the renderer treats these as
    // monospace-cell boundary fills.
    let advance = pick_advance(&hmtx_src, upem);
    let cell_top = (ascender.max(0)) as f64;
    // If ascender is non-positive (degenerate), fall back to UPM * 0.8.
    let cell_top = if cell_top > 0.0 {
        cell_top
    } else {
        (upem as f64) * 0.8
    };

    // Build new glyf+loca, preserving every existing glyph in original order
    // (so glyph_ids 0..num_glyphs_old remain stable), then appending 17 new
    // rectangle glyphs.
    let mut builder = GlyfLocaBuilder::new();
    for gid_u in 0..num_glyphs_old as u32 {
        let gid = GlyphId::new(gid_u);
        let glyph: Glyph = match loca_src.get_glyf(gid, &glyf_src) {
            Ok(Some(g)) => Glyph::from_table_ref(&g),
            Ok(None) => Glyph::Empty,
            Err(e) => bail!("reading glyph {gid_u}: {e}"),
        };
        builder
            .add_glyph(&glyph)
            .map_err(|e| anyhow!("re-emitting glyph {gid_u}: {e}"))?;
    }

    let mut new_glyph_ids = Vec::with_capacity(NUM_STEPS as usize);
    let advance_f = advance as f64;
    for k in 0..NUM_STEPS {
        let glyph = build_fill_glyph(k, advance_f, cell_top)?;
        builder
            .add_glyph(&glyph)
            .map_err(|e| anyhow!("emitting PUA glyph k={k}: {e}"))?;
        new_glyph_ids.push(GlyphId::new(num_glyphs_old as u32 + k));
    }
    let (glyf_new, loca_new, loca_format) = builder.build();

    // Build new hmtx: copy existing metrics + 17 new ones with side_bearing=0.
    let hmtx_new = build_hmtx(&hmtx_src, num_glyphs_old, advance);
    let number_of_h_metrics: u16 = hmtx_new
        .h_metrics
        .len()
        .try_into()
        .context("hmtx h_metrics overflow")?;

    // Build new cmap. Merge old mappings + new PUA mappings.
    let mut new_mappings = existing_mappings;
    // Remove any existing mappings that overlap our PUA ranges (we are about
    // to redefine them).
    new_mappings.retain(|(ch, _)| {
        let cp = *ch as u32;
        !((PRIMARY_BASE..PRIMARY_BASE + NUM_STEPS).contains(&cp)
            || (SECONDARY_BASE..SECONDARY_BASE + NUM_STEPS).contains(&cp))
    });
    for k in 0..NUM_STEPS {
        let gid = new_glyph_ids[k as usize];
        if let Some(ch) = char::from_u32(PRIMARY_BASE + k) {
            new_mappings.push((ch, gid));
        }
        if let Some(ch) = char::from_u32(SECONDARY_BASE + k) {
            new_mappings.push((ch, gid));
        }
    }
    let cmap_new = Cmap::from_mappings(new_mappings).map_err(|e| anyhow!("building cmap: {e}"))?;

    // Owned write copies of head/hhea/maxp with updated fields.
    let mut head_new: Head = head_src.to_owned_table();
    head_new.index_to_loc_format = match loca_format {
        LocaFormat::Short => 0,
        LocaFormat::Long => 1,
    };

    let mut hhea_new: Hhea = hhea_src.to_owned_table();
    hhea_new.number_of_h_metrics = number_of_h_metrics;
    // Track advance_width_max so we don't accidentally regress it.
    if advance > hhea_new.advance_width_max.to_u16() {
        hhea_new.advance_width_max = write_fonts::types::UfWord::new(advance);
    }

    let mut maxp_new: Maxp = maxp_src.to_owned_table();
    let new_num_glyphs = num_glyphs_old
        .checked_add(NUM_STEPS as u16)
        .context("num_glyphs overflow")?;
    maxp_new.num_glyphs = new_num_glyphs;
    // A rectangle has 4 points / 1 contour. Bump if smaller.
    if let Some(mp) = maxp_new.max_points.as_mut() {
        if *mp < 4 {
            *mp = 4;
        }
    }
    if let Some(mc) = maxp_new.max_contours.as_mut() {
        if *mc < 1 {
            *mc = 1;
        }
    }

    // Assemble final binary via FontBuilder.
    let mut fb = FontBuilder::new();
    fb.add_table(&head_new).map_err(|e| anyhow!("{e}"))?;
    fb.add_table(&hhea_new).map_err(|e| anyhow!("{e}"))?;
    fb.add_table(&maxp_new).map_err(|e| anyhow!("{e}"))?;
    fb.add_table(&hmtx_new).map_err(|e| anyhow!("{e}"))?;
    fb.add_table(&cmap_new).map_err(|e| anyhow!("{e}"))?;
    fb.add_table(&glyf_new).map_err(|e| anyhow!("{e}"))?;
    fb.add_table(&loca_new).map_err(|e| anyhow!("{e}"))?;
    fb.copy_missing_tables(font.clone());

    Ok(fb.build())
}

fn pick_advance(hmtx: &write_fonts::read::tables::hmtx::Hmtx<'_>, upem: u16) -> u16 {
    for m in hmtx.h_metrics() {
        let adv = m.advance();
        if adv > 0 {
            return adv;
        }
    }
    upem / 2
}

fn build_hmtx(
    hmtx: &write_fonts::read::tables::hmtx::Hmtx<'_>,
    num_glyphs_old: u16,
    new_advance: u16,
) -> Hmtx {
    // h_metrics in source covers the first `number_of_h_metrics` glyphs.
    // For glyphs beyond that, advance is reused from the last `LongMetric`
    // and only their left_side_bearings are stored. To simplify, expand
    // every old glyph to a full LongMetric.
    let src_metrics = hmtx.h_metrics();
    let src_lsb = hmtx.left_side_bearings();
    let last_advance = src_metrics.last().map(|m| m.advance()).unwrap_or(0);

    let mut h_metrics = Vec::with_capacity(num_glyphs_old as usize + NUM_STEPS as usize);
    for gid in 0..num_glyphs_old as usize {
        let (advance, sb) = if gid < src_metrics.len() {
            (src_metrics[gid].advance(), src_metrics[gid].side_bearing())
        } else {
            let lsb_idx = gid - src_metrics.len();
            let sb = src_lsb.get(lsb_idx).map(|v| v.get()).unwrap_or(0);
            (last_advance, sb)
        };
        h_metrics.push(LongMetric::new(advance, sb));
    }
    for _ in 0..NUM_STEPS {
        h_metrics.push(LongMetric::new(new_advance, 0));
    }
    Hmtx::new(h_metrics, Vec::new())
}

fn build_fill_glyph(k: u32, advance: f64, cell_top: f64) -> Result<Glyph> {
    if k == 0 {
        // Empty glyph — still occupies a glyph id and an advance, but draws
        // nothing.
        return Ok(Glyph::Empty);
    }
    let width = advance * (k as f64) / 16.0;
    let mut path = kurbo::BezPath::new();
        // Clockwise in Y-up TrueType coordinate space; this is the outer-contour
        // convention for a filled glyph (CW = filled, CCW = hole).
    path.move_to((0.0, 0.0));
    path.line_to((width, 0.0));
    path.line_to((width, cell_top));
    path.line_to((0.0, cell_top));
    path.close_path();
    let simple = SimpleGlyph::from_bezpath(&path)
        .map_err(|e| anyhow!("building rectangle glyph k={k}: {e:?}"))?;
    Ok(Glyph::Simple(simple))
}
