//! # The waterline skip bets on a direction, not just a height (batch 78)
//!
//! Batch 74's skip read the line as covering the whole 16^3 node the skip box spans; it
//! covers only the region strictly above the line. A descending ray can leave the box
//! through its bottom face -- through the slab the line disclaims -- and the hit inside
//! is never visited: captured in play as full-chunk navy water tiles (refraction legs
//! marching out at max depth, `refr` collapsing to `body`).
//!
//! These tests hold **transliterations** of the skip predicate from `common.wgsl`'s
//! `line_skip` block, over the case table the repair derived: the current-in-tree
//! predicate must empty the unsafe set (a box transit that dives at/below the line) and
//! keep the near-horizontal rays the feature was written for, and the old predicate is
//! quoted here as the documented counterexample it replaces.

#![allow(clippy::unnecessary_cast)]

/// The pre-repair predicate, kept only so the table below can show *exactly* what was
/// wrong with it: `wl < 15 && ly > wl`, no ray term. Not consulted by any fixed code.
fn predicate_v1(wl: u32, ly: u32) -> bool {
    wl < 15 && ly > wl
}

/// The shipped predicate, transliterated from `common.wgsl` at the `line_skip` block:
/// `rd.y >= 0` cannot descend at all, and otherwise the transit bound -- the dominant-axis
/// drop over the box is at most `16 * |dy| / max(|dx|, |dz|)` -- must not carry the
/// local y back to the line. The divisor mirrors the WGSL's guard for a straight-down ray.
fn predicate_v2(wl: u32, ly: u32, rd: (f32, f32, f32)) -> bool {
    let (dx, dy, dz) = rd;
    if wl >= 15 || (ly as f32) <= wl as f32 {
        return false;
    }
    if dy >= 0.0 {
        return true;
    }
    let h = dx.abs().max(dz.abs());
    h > 1e-6 && (ly as f32 - wl as f32) > 16.0 * (dy.abs() / h)
}

/// The unsafe set, from the derivation beside the WGSL: a skip inside it would cross the
/// sea-floor slab.
fn transit_is_unsafe(wl: u32, ly: u32, rd: (f32, f32, f32)) -> bool {
    let (dx, dy, dz) = rd;
    let h = dx.abs().max(dz.abs());
    if h < 1e-6 {
        return true; // straight down certainly leaves the covered region
    }
    let drop = 16.0 * dy.abs() / h;
    dy < 0.0 && (ly as f32 - drop) <= wl as f32
}

/// The repair's acceptance **is the unsafe set**: for every entry height above the line
/// and every slope at which the box transit could reach the slab, the old predicate
/// skipped and the new one cannot. The whole of the navy defect, restated as a table.
#[test]
fn the_skip_cannot_dive_through_the_slab_it_disclaims() {
    use std::f32::consts::PI;
    let mut unsafe_total = 0u32;
    let mut v1_skipped = 0u32;
    for wl in [4u32, 8, 12] {
        for ly in (wl + 1)..16 {
            for e10 in 0..80i32 {
                let elev = e10 as f32 / 10.0;
                let rd = ((elev * PI / 180.0).cos(), -(elev * PI / 180.0).sin(), 0.0);
                if !transit_is_unsafe(wl, ly, rd) {
                    continue;
                }
                unsafe_total += 1;
                if predicate_v1(wl, ly) {
                    v1_skipped += 1;
                }
                assert!(
                    !predicate_v2(wl, ly, rd),
                    "skip allowed inside the slab's reach: wl {wl}, ly {ly}, slope {elev} deg"
                );
            }
        }
    }
    // The defect must actually be in this table -- a test whose counterexample set is
    // empty proves a vocabulary, not a repair.
    assert!(unsafe_total >= 100, "the unsafe set must be non-trivial");
    assert!(
        v1_skipped == unsafe_total,
        "every unsafe transit was skippable under the old predicate ({v1_skipped}/{unsafe_total})\n        -- that is what the navy tiles were"
    );
}

/// And the benefit the feature was written for must survive: near-horizontal rays (the
/// open-sea reflection fan, 0..15 degrees below the horizon) overwhelmingly keep their
/// skip. The bar is a majority, pinned loose so a future *better* bound can only ease it.
#[test]
fn the_horizontal_ray_keeps_the_skip_the_entry_was_written_for() {
    use std::f32::consts::PI;
    let mut kept = 0u32;
    let mut total = 0u32;
    for (wl, lyo) in [(4u32, 1u32), (8, 3), (12, 1)] {
        for e10 in 0..150i32 {
            let elev = e10 as f32 / 10.0;
            let ly = (wl + lyo + (e10 as u32 % 3)).min(15);
            let rd = ((elev * PI / 180.0).cos(), -(elev * PI / 180.0).sin(), 0.0);
            if predicate_v1(wl, ly) {
                total += 1;
                if predicate_v2(wl, ly, rd) {
                    kept += 1;
                }
            }
        }
    }
    assert!(
        kept * 2 > total,
        "the majority of the 0..15-degree fan must keep the skip ({kept}/{total})"
    );
}

/// The two degenerate directions of the bound, transliterated so they cannot rot in
/// place: straight-down is never a skip, exactly-horizontal always is (when above the line).
#[test]
fn the_degenerate_rays_are_named_by_the_formula() {
    // straight down: the divisor has nothing to divide by, and the covered region ends
    // at the bottom face in one step.
    assert!(!predicate_v2(8, 15, (0.0, -1.0, 0.0)));
    // exactly horizontal and above the line: the drop is zero, the skip is the picture
    // the entry was written for at rest.
    assert!(predicate_v2(8, 15, (0.9, 0.0, 0.44)));
    // ascending is covered by the first arm, whatever the magnitude of dy.
    assert!(predicate_v2(8, 15, (0.2, 0.8, 0.1)));
    // and one hand-computed midpoint of the exact boundary, so the constant 16 itself
    // is pinned and not merely the shape: at 9 degrees below horizontal the drop over
    // the dominant transit is 16 * (0.1564 / 0.9877) = 2.53 blocks, so a local y two and
    // three above the line sit on either side of the bound.
    let s9 = (9.0f32 * core::f32::consts::PI / 180.0).sin();
    let c9 = (9.0f32 * core::f32::consts::PI / 180.0).cos();
    let rd = (c9, -s9, 0.0);
    assert!(!predicate_v2(8, 10, rd), "two above the line cannot skip at 9 degrees");
    assert!(predicate_v2(8, 11, rd), "three above the line still can");
}

