use voxelcraft::config::{parse_from, Config};

fn cfg_of(args: &[&str]) -> Config {
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (cfg, _, unknown) = parse_from(&owned);
    assert!(unknown.is_empty(), "unexpected unknown args: {unknown:?}");
    cfg
}

const BLIT: &str = include_str!("../src/render/shaders/blit.wgsl");
const BLIT_RS: &str = include_str!("../src/render/blit.rs");

#[test]
fn the_dial_parses_and_defaults_to_explicit_one() {
    assert_eq!(
        cfg_of(&[]).grade_strength,
        1.0,
        "off-dial is the batch-97 grade frame"
    );
    assert_eq!(
        cfg_of(&["--grade-strength", "0.5"]).grade_strength,
        0.5,
        "the dial takes a float"
    );
    assert_eq!(
        cfg_of(&["--grade-strength", "banana"]).grade_strength,
        1.0,
        "unparseable falls to the shipped amount, the parser's standing reason"
    );
}

#[test]
fn the_guard_keeps_the_grade_exact_by_construction() {
    assert_eq!(
        BLIT.matches("if blit.strength < 1.0 {").count(),
        2,
        "both casts take the dial behind the same guard"
    );
    assert_eq!(
        BLIT.matches("mix(before, c, clamp(blit.strength, 0.0, 1.0))")
            .count(),
        2,
        "one lerp form, twice"
    );
    assert!(BLIT.contains("let before = c;"), "the pre-grade pixel is kept");
}

#[test]
fn the_uniform_is_still_sixteen_bytes_with_four_fields() {
    for needle in ["encode", "tone", "grade", "strength"] {
        assert!(
            BLIT.contains(&format!("{needle}:")),
            "blit.wgsl must keep the {needle} field"
        );
    }
    assert!(BLIT.contains("strength: f32,"));
    assert!(BLIT_RS.contains("strength: f32,"));
    assert!(
        BLIT_RS.contains("size: 16,"),
        "the uniform buffer descriptor still says sixteen bytes"
    );

    assert!(
        !BLIT_RS.contains("pad: u32"),
        "no unnamed pads remain: every word of the uniform has a name now"
    );
}

#[test]
fn the_ripple_takes_the_wave_fields_nyquist_class() {
    let src = voxelcraft::render::shader_source();
    assert!(
        src.contains("let pix = px_world / max(abs(rd.y), WAVE_GRAZE_MIN);"),
        "the ripple's footprint is the grazing-stretched one, same as the wave aniso"
    );
    assert!(
        src.contains("smoothstep(2.0, 4.0, 1.15 / max(pix, 1e-4));"),
        "the long train dies where its wavelength falls under the footprint"
    );
    assert!(
        src.contains("smoothstep(2.0, 4.0, 0.62 / max(pix, 1e-4));"),
        "the short train -- the chevron/ring maker -- dies first"
    );

    assert!(src.contains("0.60 * fade_115"));
    assert!(src.contains("0.40 * fade_62"));
    assert!(src.contains("const WATER_LOOK_RIPPLE_AMP: f32 = 0.16;"));

    assert_eq!(src.matches("let k1 = 6.2832 / 1.15;").count(), 1);
    assert_eq!(src.matches("let k2 = 6.2832 / 0.62;").count(), 1);
}

use glam::IVec3;
use voxelcraft::block;
use voxelcraft::block::BlockId;
use voxelcraft::voxel::ChunkKey;
use voxelcraft::worldgen::WorldGen;

#[test]
fn the_grass_dense_flag_is_off_by_default_and_parseable() {
    assert!(!Config::default().grass_dense, "`--grass-dense` defaults off");
    let (cfg, _, _) = parse_from(&["voxelcraft".to_string(), "--grass-dense".to_string()]);
    assert!(cfg.grass_dense);
}

fn chunk_voxels(gen: &WorldGen, key: ChunkKey) -> Vec<BlockId> {
    let mut dense = vec![0 as BlockId; 64 * 64 * 64];
    let mut heights = [0i32; 64 * 64];
    gen.generate(key, &mut dense, &mut heights);
    dense
}

#[test]
fn the_g2_arm_is_invisible_unarmed_and_presents_every_mechanism_armed() {
    const SEED: i32 = 90210;
    let off = WorldGen::new(SEED);
    let mut on = WorldGen::new(SEED);
    on.grass_dense = true;

    let mut off_tufts = 0usize;
    let mut off_new = 0usize;
    let mut on_tufts = 0usize;
    let mut on_new = 0usize;
    for cx in 0..12 {
        for cz in 0..6 {
            for cy in 0..3 {
                let d_off = chunk_voxels(&off, ChunkKey::new(0, IVec3::new(cx, cy, cz)));
                for &b in &d_off {
                    if b == block::TALL_GRASS {
                        off_tufts += 1;
                    }
                    if b == block::GRASS_TALL || b == block::REEDS {
                        off_new += 1;
                    }
                }
                let d_on = chunk_voxels(&on, ChunkKey::new(0, IVec3::new(cx, cy, cz)));
                for &b in &d_on {
                    if b == block::TALL_GRASS {
                        on_tufts += 1;
                    }
                    if b == block::GRASS_TALL || b == block::REEDS {
                        on_new += 1;
                    }
                }
            }
        }
    }
    assert_eq!(off_new, 0, "the unarmed world has no 101e voxel to find");
    assert!(off_tufts > 0, "the control carpet itself is present in this window");

    assert!(
        on_tufts + on_new > off_tufts,
        "armed carpet ({}) is not thicker than the control's {}",
        on_tufts + on_new,
        off_tufts
    );
    assert!(on_new > 0, "the armed window grew no tussock stack at all");
    assert!(
        on_new < on_tufts + on_new,
        "every tuft became a tussock -- the 76% normal share is gone"
    );

    for cx in 0..6 {
        for cz in 0..3 {
            let off_d = chunk_voxels(&off, ChunkKey::new(1, IVec3::new(cx, 1, cz)));
            assert!(
                !off_d.contains(&block::TALL_GRASS),
                "an unarmed coarse chunk holds a tuft -- the LOD 0 fence moved"
            );
        }
    }
    let mut shell = 0usize;
    for cx in 0..6 {
        for cz in 0..3 {
            let d = chunk_voxels(&on, ChunkKey::new(1, IVec3::new(cx, 1, cz)));
            shell += d.iter().filter(|&&b| b == block::TALL_GRASS).count();
        }
    }
    assert!(shell > 0, "the armed coarse shell grew no tuft proxy in this window");
}

#[test]
fn the_new_cover_rides_is_tuft_and_nothing_else() {
    let src = voxelcraft::render::shader_source();
    assert!(src.contains("const GRASS_TALL_ID: u32 = 24u;"));
    assert!(src.contains("const REEDS_ID: u32 = 25u;"));
    assert!(src.contains("fn is_tuft(b: u32) -> bool {"));

    assert_eq!(
        src.matches("== TALL_GRASS_ID").count(),
        1,
        "the tuft question must be asked by is_tuft alone"
    );
    assert_eq!(
        src.matches("!= TALL_GRASS_ID").count(),
        0,
        "no negative tuft special-case outside is_tuft"
    );
    assert!(
        src.matches("is_tuft(").count() >= 4,
        "the fast-path, the marcher, the occluder *and the decl* all ask is_tuft"
    );
}

#[test]
fn the_g2_worldgen_construction_gates() {
    let src = include_str!("../src/worldgen.rs");
    assert!(src.contains("pub grass_dense: bool,"));

    assert!(src.contains("prob = (prob * G2_GRASS_DENSITY).min(0.95);"));

    assert!(src.contains("self.seed ^ 0xa455"));
    assert!(src.contains("self.seed ^ 0x5eed"));
    assert!(src.contains("self.seed ^ 0x7eed"));

    let reeds = src.find("let stem = ").expect("the reed stem rule");
    let window = &src[reeds..reeds + 400];
    assert!(window.contains("if out[i] != AIR"));
    assert!(window.contains("out[i] = REEDS;"));

    let shell = src.find("fn coarse_tufts").expect("coarse_tufts");
    let window = &src[shell..shell + 3400];
    assert!(window.contains("if !self.grass_dense {"));
    assert!(window.contains("return;"));

    assert!(window.contains("if s > G2_COARSE_STRIDE_MAX {"));
    assert!(window.contains("let keep = if s > 2 { G2_COARSE_KEEP_FAR } else { G2_COARSE_KEEP };"));

    assert!(window.contains("canopy_rank3(wx, wy, wz, self.seed) >= ceiling"));
    assert!(window.contains("out[i] = TALL_GRASS;"));

    assert!(window.contains("(thinned[above >> 6] >> (above & 63)) & 1 == 1"));
}

#[test]
fn the_g2_block_table_is_appended_and_forget_proof() {
    assert_eq!(block::GRASS_TALL, 24);
    assert_eq!(block::REEDS, 25);
    assert_eq!(block::BLOCK_COUNT, 26);
    let tall = block::BLOCKS[block::GRASS_TALL as usize];
    let reeds = block::BLOCKS[block::REEDS as usize];
    assert!(tall.foliage && !tall.solid && !tall.opaque && !tall.cutout);
    assert!(reeds.foliage && !reeds.solid && !reeds.opaque && !reeds.cutout);
    assert_eq!(block::tex::COUNT, 27);
    assert!(
        block::bar_slot_refusal(block::GRASS_TALL).is_some(),
        "a placed tussock is invisible in a no-foliage build, same as the tuft's"
    );
}

#[test]
fn the_wind_sway_flag_is_off_by_default_and_parseable() {
    assert!(!Config::default().wind_sway, "`--wind-sway` defaults off");
    let (cfg, _, _) = parse_from(&["voxelcraft".to_string(), "--wind-sway".to_string()]);
    assert!(cfg.wind_sway);
}

#[test]
fn the_shear_is_a_plane_and_the_control_path_is_byte_for_byte_the_legacy() {
    let src = voxelcraft::render::shader_source();
    assert!(src.contains("override SPEC_WIND_SWAY: bool = false;"));
    assert!(src.contains("const WIND_SWAY_AMP: f32 = 0.09;"));
    assert!(src.contains("const WIND_SWAY_T1: f32 = 1.9;"));
    assert!(src.contains("const WIND_SWAY_T2: f32 = 3.1;"));

    assert!(src.contains("fn wind_shear(wx: f32, wz: f32, time: f32) -> vec2<f32> {"));
    assert!(src.contains("sin(time * WIND_SWAY_T1 + wx * 0.11 + wz * 0.07)"));

    assert!(src.contains("let dy = ro.y - base.y;"));
    assert!(
        !src.contains("WIND_SWAY_ROOT"),
        "a global lean anchor displaces the spine by lean*height -- the per-cell anchor won"
    );

    assert!(src.contains("kA * dy - (ro.x - ro.z)) / dA"));
    assert!(src.contains("kB * dy - (ro.x + ro.z)) / dB"));

    assert!(
        src.matches("ts.x = ((base.x - base.z) - (ro.x - ro.z)) / dA;").count() == 1,
        "the legacy ts.x must survive verbatim, once, in the guarded else"
    );
    assert!(
        src.matches("ts.y = ((base.x + base.z + 1.0) - (ro.x + ro.z)) / dB;").count() == 1,
        "the legacy ts.y must survive verbatim, once, in the guarded else"
    );

    assert_eq!(src.matches("fn cross_quad(").count(), 1);
    assert_eq!(src.matches("cross_quad(uc, ro, rd, face_layer").count(), 1);
}

#[test]
fn the_wind_sway_bit_rides_the_pipeline_key_word() {
    use voxelcraft::render as r;
    assert_eq!(r::FLAG_HI_WIND_SWAY, 32768, "the next free hi bit after canopy-relief");

    assert_ne!(
        r::SPEC_HI_MASK & r::FLAG_HI_WIND_SWAY,
        0,
        "a hi bit outside SPEC_HI_MASK keys no pipeline at all -- the arm compiles inert"
    );

    let src = include_str!("../src/render/mod.rs");
    assert!(src.contains("\"SPEC_WIND_SWAY\""));
    assert!(src.contains("spec_hi & FLAG_HI_WIND_SWAY"));
}
