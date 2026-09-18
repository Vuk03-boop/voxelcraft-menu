//! Batch 90's four look arms (roadmap A1, A4, A9, A10), each behind its own flag and each
//! default-off. What is pinned here is the part without a device: the flags' off-by-default
//! contract and their parser spellings, the flag-word allocations, and the one arm whose
//! effect is pure data -- A1's canopy lift on the shaft field, where zero *is* the pre-arm
//! field bit for bit and any positive value lifts exactly the columns the tree placer
//! would plant. The shader halves (A4's traced pane reflection, A9's caustic sheets and
//! the A10 curve arm at the blit) fold out of the compiled pipeline when their bits are
//! clear, which is what makes the shipping frame's bit-exactness a construction argument
//! rather than a measured one; their A/B numbers belong to the by-eye pass, not to tests.

use voxelcraft::config::{parse_from, Config};
use voxelcraft::render::{FLAG_HI_CAUSTICS, FLAG_HI_GLASS_REFLECT};
use voxelcraft::shaft::ShaftField;
use voxelcraft::worldgen::{WorldGen, SEA_LEVEL};

fn cfg_of(args: &[&str]) -> Config {
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (cfg, _, unknown) = parse_from(&owned);
    assert!(unknown.is_empty(), "unexpected unknown args: {unknown:?}");
    cfg
}

/// Every arm of the batch wears the shipping picture when its flag is not given. This is
/// the batch's one promise to every other measurement in the ledger, so the test that
/// names it says so in one table rather than four times in prose.
#[test]
fn arms_default_off() {
    let c = Config::default();
    assert!(!c.caustics);
    assert!(!c.glass_reflect);
    assert!(!c.tone_map_aces);
    assert_eq!(c.canopy_lift, 0.0);
}

#[test]
fn parser_spellings() {
    assert!(cfg_of(&["--caustics"]).caustics);
    assert!(cfg_of(&["--glass-reflect"]).glass_reflect);
    assert!(cfg_of(&["--tone-map", "aces"]).tone_map_aces);
    // An unrecognised shoulder falls back to the knee, per the flag's comment: the engine
    // not running a requested law silently is worse than the spelling it did not get.
    assert!(!cfg_of(&["--tone-map", "filmic"]).tone_map_aces);
    assert!(!cfg_of(&["--tone-map", "knee"]).tone_map_aces);
    assert_eq!(cfg_of(&["--canopy-lift", "6"]).canopy_lift, 6.0);
    // A negative lift would *expose* the canopy to the field, which is a different bug
    // with a different name; the parser clamps it to the no-arms value instead.
    assert_eq!(cfg_of(&["--canopy-lift", "-3"]).canopy_lift, 0.0);
}

/// The two allocation values the batch leaned on: 64 and 128 were free. Properties of the
/// word -- power-of-two, distinctness, mask membership, no anonymous mask bits -- are
/// asserted for *all* FLAG_HI bits by `tests/spec_hi.rs`, which is also where a new bit's
/// one line goes; this file pins only the numbers 90 actually took.
#[test]
fn flag_word_allocations() {
    assert_eq!(FLAG_HI_CAUSTICS, 64);
    assert_eq!(FLAG_HI_GLASS_REFLECT, 128);
}

/// Helper mirroring the fill coordinates in `ShaftField::update` for a field that has
/// never scrolled (`update` from empty at camera 0, 0). Keeping the arithmetic here --
/// and only here -- means the pinning below reads as world facts, not texel bookkeeping.
fn texel_centre(idx: usize, dim: usize, texel: i32) -> (i32, i32) {
    let x = (idx % dim) as i32 - (dim / 2) as i32;
    let z = (idx / dim) as i32 - (dim / 2) as i32;
    (x * texel + texel / 2, z * texel + texel / 2)
}

/// A1 at 0.0 is the pre-arm field, bit for bit: every texel is exactly `gen.height` at
/// its centre. This is the batch-53/73 convention in data form -- the control of all four
/// arms is construction, not coincidence.
#[test]
fn zero_lift_is_the_legacy_field() {
    let gen = WorldGen::new(7);
    let mut f = ShaftField::new();
    f.update(&gen, 0.0, 0.0, 0.0);
    for (i, &h) in f.heights().iter().enumerate() {
        let (wx, wz) = texel_centre(i, shaft_dim(), shaft_texel());
        assert_eq!(h, gen.height(wx, wz) as f32, "texel {i}");
    }
}

/// A1 at a positive lift moves exactly the columns the tree placer gates admit, by
/// exactly the lift, and nothing else: biome tree scale, beach, tree line -- the three
/// rules copied from `coarse_trees`, because a column that cannot hold a tree has no
/// canopy to hide. A field that had scrolled once must behave identically, since the
/// recycled texels were written under the same rule -- that is what the two-step loop at
/// the end guards.
#[test]
fn lift_obeys_the_placers_gates() {
    let gen = WorldGen::new(7);
    let lift = 6.0f32;
    let mut f = ShaftField::new();
    f.update(&gen, 0.0, 0.0, lift);
    let mut gated = 0usize;
    for (i, &h) in f.heights().iter().enumerate() {
        let (wx, wz) = texel_centre(i, shaft_dim(), shaft_texel());
        let base = gen.height(wx, wz);
        let cbi = gen.column_biome(wx, wz);
        let holds_tree =
            cbi.def().tree_scale > 0.0 && base > SEA_LEVEL + 2 && base <= cbi.tree_line;
        if holds_tree {
            gated += 1;
            assert_eq!(h, base as f32 + lift, "lifted texel {i}");
        } else {
            assert_eq!(h, base as f32, "unlifted texel {i}");
        }
    }
    // The world under this seed had better actually exercise both branches, or the test
    // proves a tautology.
    assert!(gated > 0, "no tree column in the window; pick another seed");
    assert!(gated < f.heights().len());

    // Zero-lift equality must ALSO hold on a scrolled fill, not only a fresh one: the
    // carried texels are where a half-applied arm would leak.
    let mut g = ShaftField::new();
    g.update(&gen, 0.0, 0.0, 0.0);
    g.update(&gen, 3.0, -2.0, 0.0);
    let mut h2 = ShaftField::new();
    h2.update(&gen, 0.0, 0.0, 0.0);
    h2.update(&gen, 3.0, -2.0, 0.0);
    assert_eq!(g.heights(), h2.heights(), "scrolled zero-lift is deterministic");
}

fn shaft_dim() -> usize {
    voxelcraft::shaft::DIM
}

fn shaft_texel() -> i32 {
    voxelcraft::shaft::TEXEL
}

