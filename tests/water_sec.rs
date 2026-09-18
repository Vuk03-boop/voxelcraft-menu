//! Batch 81 (roadmap P12)'s load-bearing seams, all CPU-side.
//!
//! The diagnostic arm's whole contract is "the same pixels, the same legs, a quarter of
//! the evaluations" -- so what it must never do is change *which* pixels are water. That
//! decision is made twice now: once in `resolve` from the stored block id, once in
//! `water_sec` from the same id decoded out of `vis`. These tests pin that the decision
//! stays one decision, and the table contracts the new pass rides on.

use voxelcraft::block::{self, WATER};
use voxelcraft::render;

/// Block-taxonomy exclusivity: the WGSL donor test is `block_at(ci, v) != WATER_ID`, and
/// the four macro classes each carry their own WGSL test beside that loop
/// (`attr_uniform` reads foliage/cutout, `id == GLASS_ID` the glass arm, `any_blk` the
/// emitters). Water in ANY of them would make two tests claim one block -- a pane of
/// glass marked water would donate legs it cannot legitimately produce.
#[test]
fn water_is_its_own_class_only() {
    assert!(!block::is_foliage(WATER));
    assert!(!block::is_cutout(WATER));
    assert!(!block::is_glass(WATER));
    assert!(!block::is_emitter(WATER));
}

/// The new pass is on the measured list and the table it rides is the size the
/// layout-count test holds it to: `water_sec` in `ENTRY_POINTS` exactly once, ahead of
/// the `resolve` that reads it, and `BIND_GROUP_0_ENTRIES` naming 26, which is the count
/// the WGSL file keeps at its own 0..21; 81b moved the water pair to groups 1/2.
#[test]
fn entry_points_and_binding_table_carry_the_pass() {
    let eps = &render::ENTRY_POINTS;
    assert_eq!(eps.len(), 11);
    assert_eq!(eps.iter().filter(|e| **e == "water_sec").count(), 1);
    let at = eps.iter().position(|e| *e == "water_sec").unwrap();
    assert_eq!(eps[at + 1], "resolve", "the reader must follow the pass");
    // Batch 81b: group 0 went back to its 22 entries; the two water textures live at
    // group(1)/group(2) because the validator forbids one texture holding both roles
    // anywhere in a dispatch's attached groups.
    assert_eq!(render::BIND_GROUP_0_ENTRIES, 22);
    let src = render::shader_source();
    assert!(src.contains("@group(1) @binding(0) var water_refr_sample: texture_2d<f32>;"));
    assert!(src.contains("@group(2) @binding(0) var water_refr_tex: texture_storage_2d<rgba16float, write>;"));
    assert!(src.contains("@group(1) @binding(1) var water_refl_sample: texture_2d<f32>;"));
    assert!(src.contains("@group(2) @binding(1) var water_refl_tex: texture_storage_2d<rgba16float, write>;"));
}

/// The pipeline key: the diagnostic ships ON by default, and since the shader reads the
/// override and never the bit, the bit exists only so `ensure_spec` can tell the two
/// builds apart -- which makes "in the mask" the whole of the contract. The sibling bits
/// are pinned by the `const _` asserts in render/mod.rs; this is the one place a member
/// is pinned from outside.
#[test]
fn water_sec_is_a_spec_word_member() {
    assert_ne!(render::SPEC_HI_MASK & render::FLAG_HI_WATER_SEC, 0);
    // And it shares no bit and the mask holds no anonymous ones -- these were once a
    // `count_ones() == N` here, which is "distinct and accounted for" misspelled as a
    // number, and it fired in the wrong file each time a batch added a bit. The property
    // form lives in `tests/spec_hi.rs`; this file's job is the one word above.
}

/// Batch 81c: the fraction the sweep prices is one `--water-sec-scale` away, and the
/// default stands at 2 where the diagnostic measured P12 going. The shader's override is
/// the shift, the config carries the edge, and `trailing_zeros` is the map between them
/// -- all three pinned because a fourth spelling would silently re-bake pipelines.
#[test]
fn water_sec_scale_carries_the_fraction_sweep() {
    use voxelcraft::config::Config;
    assert_eq!(Config::default().water_sec_scale, 2);
    assert_eq!(2u32.trailing_zeros(), 1);
    assert_eq!(1u32.trailing_zeros(), 0);
    assert_eq!(4u32.trailing_zeros(), 2);
    let src = render::shader_source();
    assert!(src.contains("override WATER_SEC_SHIFT: u32 = 1u;"), "the shift override");
}

