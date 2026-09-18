//! Batch 101, piece by piece: the second-round verdicts become dials and the generator
//! rebuild. This file pins the pieces that need no device; the renders stay where
//! renders live -- the hardware round re-runs the lookbook when the batch closes.
//!
//! Pieces so far:
//! - 101b `MIDDAY-REEDS` (pinned in `tests/batch98.rs`: noon, cam-height 2, coastline yaw).
//! - 101c `--grade-strength` (this file): the cine sweep verdict's dial, and CINE-HALF's
//!   presence in the lookbook matrix (pinned in `tests/batch98.rs`).

use voxelcraft::config::{parse_from, Config};

fn cfg_of(args: &[&str]) -> Config {
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (cfg, _, unknown) = parse_from(&owned);
    assert!(unknown.is_empty(), "unexpected unknown args: {unknown:?}");
    cfg
}

/// The blit's one stage, read as text: the struct, the two casts and the two dials.
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

/// The exactness claim is *construction*, not arithmetic luck: at 1.0 the lerp branch
/// never runs, so the batch-97 casts are the same instruction stream as before --
/// trusting `mix(a, b, 1.0)` to equal `b` bit-for-bit would wager a control on which
/// mix formula the driver picks, and that is a class of wager this suite does not make.
/// Both grade branches take the dial, spelled the same way.
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

/// The uniform is still sixteen bytes: the dial spends the last padding word, and the
/// wgsl struct and the rust struct are transliterations naming the same four fields.
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
    // The two pads are gone on the rust side: both words are now spoken for.
    assert!(
        !BLIT_RS.contains("pad: u32"),
        "no unnamed pads remain: every word of the uniform has a name now"
    );
}

/// 101d: the ripple term's one-amplitude-everywhere could only read as invisible or
/// moire (the second round's aerial ring fields *and* near-band chevrons, and the
/// pre-existing swell carrying banding through the same ranges). The cure is the wave
/// field's own Nyquist class a level down: each train's weight is shaped by the
/// grazing-stretched footprint, full weight above four samples per period and gone at
/// two -- the exact cut the control-side octaves receive, spelled where a drift could
/// re-break it.
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
    // The arm's written strength inspects weights, not the arm's shape: the weighted
    // trains still ride `nw` only, and the AMP constant is untouched by 101d.
    assert!(src.contains("0.60 * fade_115"));
    assert!(src.contains("0.40 * fade_62"));
    assert!(src.contains("const WATER_LOOK_RIPPLE_AMP: f32 = 0.16;"));
    // The 1.15 and 0.62 periods are the algebra of the fades, not turnables: they must
    // appear *inside* `look_ripple` as wave numbers, once each (no third train sneaks
    // its own fade past the pin).
    assert_eq!(src.matches("let k1 = 6.2832 / 1.15;").count(), 1);
    assert_eq!(src.matches("let k2 = 6.2832 / 0.62;").count(), 1);
}

// --- 101e: the G2 generator rebuild --------------------------------------------------
//
// The references' grass is three questions -- density, profile, range -- and the arm
// answers them with two new foliage ids riding the tuft's `is_tuft` contract, never a
// re-roll of the canopy's coin. The pins below are construction, not numbers: ids,
// tables, gates, salts, and the one behavioral property the arm claims (the unarmed
// world has no trace of it), which is the exact claim A5's tests made for its proxies.

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
    const SEED: i32 = 90210; // tests/worldgen.rs's A5 window: forest biome, so grass.
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
    // The multiplicands the window pins: armed cover strictly exceeds unarmed, and the
    // two new forms are a share of the whole rather than its replacement.
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

    // The coarse shell: stride 2 keeps tuft *proxies* under the arm, and has none
    // without it -- batch 14's "foliage is LOD 0 only" is the unarmed law, pinned here
    // for the first time with an exception to state.
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

/// The two ids ride the tuft's shader contract once -- `is_tuft` is the one site the
/// question is asked, so a fourth caller copy-pasting the id is how a new cover block
/// ends up a green cube in one pass and air in another.
#[test]
fn the_new_cover_rides_is_tuft_and_nothing_else() {
    let src = voxelcraft::render::shader_source();
    assert!(src.contains("const GRASS_TALL_ID: u32 = 24u;"));
    assert!(src.contains("const REEDS_ID: u32 = 25u;"));
    assert!(src.contains("fn is_tuft(b: u32) -> bool {"));
    // The law is over *operators*, not tokens: comments are text too (the sky-cool census
    // lesson), so count the question's shapes -- `== TALL_GRASS_ID` inside is_tuft and
    // nothing asking it negatively anywhere. That is what makes a fourth caller unable
    // to copy-paste a direct compare past the tester.
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

/// Worldgen's arm ends where it begins: every change behind the flag, salts separated
/// from the carpet's coin, the coarse shell early-returning unarmed.
#[test]
fn the_g2_worldgen_construction_gates() {
    let src = include_str!("../src/worldgen.rs");
    assert!(src.contains("pub grass_dense: bool,"));
    // The detailed carpet's density is a multiplier on the same clump field, capped at
    // the tundra-logger argument of batch 14 (a closed carpet hides its terrain).
    assert!(src.contains("prob = (prob * G2_GRASS_DENSITY).min(0.95);"));
    // The tall share, the reed coin and the stem coin each have their own salt and none
    // of them is the carpet's 0x6720 -- two decisions about the same ground are never
    // the same coin (coarse_meadow's salt comment, batch 18).
    assert!(src.contains("self.seed ^ 0xa455"));
    assert!(src.contains("self.seed ^ 0x5eed"));
    assert!(src.contains("self.seed ^ 0x7eed"));
    // Reeds only into air, cell by cell, and never further than the stem says.
    let reeds = src.find("let stem = ").expect("the reed stem rule");
    let window = &src[reeds..reeds + 400];
    assert!(window.contains("if out[i] != AIR"));
    assert!(window.contains("out[i] = REEDS;"));
    // The coarse shell is an early return off its arm, exactly as coarse_meadow is off
    // its own -- the cheap no-op is what the unarmed stream is allowed to keep.
    let shell = src.find("fn coarse_tufts").expect("coarse_tufts");
    let window = &src[shell..shell + 3400];
    assert!(window.contains("if !self.grass_dense {"));
    assert!(window.contains("return;"));
    // 101j: stride cap is a named constant now (the far-band experiment rides on it);
    // and the far band is *sparser*, not denser -- the quad-wall class the cap feared.
    assert!(window.contains("if s > G2_COARSE_STRIDE_MAX {"));
    assert!(window.contains("let keep = if s > 2 { G2_COARSE_KEEP_FAR } else { G2_COARSE_KEEP };"));
    // ...driven by A5's rank field, not the tuft placer's coin.
    assert!(window.contains("canopy_rank3(wx, wy, wz, self.seed) >= ceiling"));
    assert!(window.contains("out[i] = TALL_GRASS;"));
    // And the gate's thinned-canopy discipline is batch 96's, verbatim shape.
    assert!(window.contains("(thinned[above >> 6] >> (above & 63)) & 1 == 1"));
}

/// The block table carries the pair under batch 94b's one-list rule: appended, foliage,
/// never solid, and the albedo table says why the atlas test tolerates them.
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

// --- 101f: wind sway, the G2 movement half -------------------------------------------
//
// The references' grass moves and the fork's stills gap lists "grass stands still" under
// the top three. There is no vertex stage to move -- the ray marcher *is* geometry here --
// so the sway is a shear field: a shear is still a plane, and the cross-quad's hit stays
// one division.

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
    // The gust field travels (world x/z phase), two sines, never a per-cell coin.
    assert!(src.contains("fn wind_shear(wx: f32, wz: f32, time: f32) -> vec2<f32> {"));
    assert!(src.contains("sin(time * WIND_SWAY_T1 + wx * 0.11 + wz * 0.07)"));
    // Anchor discipline: the lean is measured from the cell's own base, which is the one
    // construction that cannot blink a blade out of its voxel far from any global root.
    assert!(src.contains("let dy = ro.y - base.y;"));
    assert!(
        !src.contains("WIND_SWAY_ROOT"),
        "a global lean anchor displaces the spine by lean*height -- the per-cell anchor won"
    );
    // The armed branch carries the lean terms, exactly one division per plane.
    assert!(src.contains("kA * dy - (ro.x - ro.z)) / dA"));
    assert!(src.contains("kB * dy - (ro.x + ro.z)) / dB"));
    // The else-branch keeps *the legacy arithmetic shaped as the legacy arithmetic* --
    // the control's ts are the same statement, not a formula that would reduce to it.
    assert!(
        src.matches("ts.x = ((base.x - base.z) - (ro.x - ro.z)) / dA;").count() == 1,
        "the legacy ts.x must survive verbatim, once, in the guarded else"
    );
    assert!(
        src.matches("ts.y = ((base.x + base.z + 1.0) - (ro.x + ro.z)) / dB;").count() == 1,
        "the legacy ts.y must survive verbatim, once, in the guarded else"
    );
    // One caller and one defn, so a second hand cannot solder an unguarded caller in
    // (the defn line carries its parameter types, so the caller's string is the pin).
    assert_eq!(src.matches("fn cross_quad(").count(), 1);
    assert_eq!(src.matches("cross_quad(uc, ro, rd, face_layer").count(), 1);
}

#[test]
fn the_wind_sway_bit_rides_the_pipeline_key_word() {
    use voxelcraft::render as r;
    assert_eq!(r::FLAG_HI_WIND_SWAY, 32768, "the next free hi bit after canopy-relief");
    // Round-101-hardware's finding: this test as first written passed *while the bit
    // did not ride the key* -- the bit was defined, set from config, and read by
    // `make_spec`, and absent from SPEC_HI_MASK all at once. The mask membership is
    // now the leading assertion, not the trailing one.
    assert_ne!(
        r::SPEC_HI_MASK & r::FLAG_HI_WIND_SWAY,
        0,
        "a hi bit outside SPEC_HI_MASK keys no pipeline at all -- the arm compiles inert"
    );
    // The hi word is what `make_spec` turns into the override; the entry must exist,
    // because an override left at its default is a flag that does nothing in silence.
    let src = include_str!("../src/render/mod.rs");
    assert!(src.contains("\"SPEC_WIND_SWAY\""));
    assert!(src.contains("spec_hi & FLAG_HI_WIND_SWAY"));
}

