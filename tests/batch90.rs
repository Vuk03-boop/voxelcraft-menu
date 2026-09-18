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

    assert!(!cfg_of(&["--tone-map", "filmic"]).tone_map_aces);
    assert!(!cfg_of(&["--tone-map", "knee"]).tone_map_aces);
    assert_eq!(cfg_of(&["--canopy-lift", "6"]).canopy_lift, 6.0);

    assert_eq!(cfg_of(&["--canopy-lift", "-3"]).canopy_lift, 0.0);
}

#[test]
fn flag_word_allocations() {
    assert_eq!(FLAG_HI_CAUSTICS, 64);
    assert_eq!(FLAG_HI_GLASS_REFLECT, 128);
}

fn texel_centre(idx: usize, dim: usize, texel: i32) -> (i32, i32) {
    let x = (idx % dim) as i32 - (dim / 2) as i32;
    let z = (idx / dim) as i32 - (dim / 2) as i32;
    (x * texel + texel / 2, z * texel + texel / 2)
}

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

    assert!(gated > 0, "no tree column in the window; pick another seed");
    assert!(gated < f.heights().len());

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
