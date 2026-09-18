//! Batch 97: roadmap G3 -- "the five" from `docs/look-gap-analysis.md`, the four flagship
//! frames against the two references, ranked cheapest first. Every arm is **flags-only** by
//! the user's own wording: an unflagged run is the pre-97 frame and a flagged run is the
//! edited look, so each arm's control is construction, not a remembered number.
//!
//! The arms:
//! - 97a `--sky-cool` (SPEC_SKY_COOL, bit 2048): constant-swap class -- the gradient and
//!   the deck recolour toward the references' cornflower midday; same instructions, other
//!   immediates, folded dead when off.
//! - 97b `--grade cine` (blit selector value 2, no bit): the filmic print -- S-curve
//!   micro-contrast, slightly desaturated mids, cool shadows against warm highlights -- in
//!   the A10 blit class where a uniform selector already rides one spent pad word.
//! - 97c `--water-look` (SPEC_WATER_LOOK, bit 4096): animated micro-ripples on the
//!   sky-facing normal, murk-absorption tint growing with the refracted column, and a
//!   sunlit-turquoise shore band -- the references' loudest water cue.
//! - 97d `--foliage-rich` (SPEC_FOLIAGE_RICH, bit 8192): ground-cover and grass-top
//!   species richness -- dry-tan stands, value jitter, a rare flower pink -- hashed per
//!   voxel in the shader; worldgen geometry untouched, so the marcher sees the same world.
//! - 97e `--canopy-relief` (SPEC_CANOPY_RELIEF, bit 16384): clump-scale brightness relief
//!   on leaves with sun-facing modulation and per-voxel micro jitter.
//!
//! (Batch 96b while `coarse_keys` taught its sampler to *find* canopy, 97c's contract
//! grew one more sentence: the arm reads the same column the refraction leg traced, so
//! its murk and its shore band are graded by a depth the frame already paid to find.)
//!
//! Same evidence class as every sandbox batch: what can be pinned without a device is the
//! architecture, the defaults and the spellings (read out of the WGSL after writing, never
//! guessed -- the a8_1 lesson); the by-eye acceptance is the hardware round's.

use voxelcraft::config::{parse_from, Config};

fn cfg_of(args: &[&str]) -> Config {
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (cfg, _, unknown) = parse_from(&owned);
    assert!(unknown.is_empty(), "unexpected unknown args: {unknown:?}");
    cfg
}

/// The user's contract is a property: five arms, five parse spellings, every default off.
/// An unflagged run constructing the same flagged word as day one is the control's whole
/// guarantee, because every `SPEC_` fold reads the CPU word these booleans feed.
#[test]
fn the_five_parse_and_default_off() {
    assert!(cfg_of(&["--sky-cool"]).sky_cool);
    assert!(!Config::default().sky_cool, "the gradient is the control");
    let cine = cfg_of(&["--grade", "cine"]);
    assert!(cine.grade_cine && !cine.grade_warm);
    let warm = cfg_of(&["--grade", "warm"]);
    assert!(warm.grade_warm && !warm.grade_cine);
    let none = cfg_of(&["--grade", "none"]);
    assert!(!none.grade_warm && !none.grade_cine);
    assert!(!Config::default().grade_cine, "the presentation is the control");
    assert!(cfg_of(&["--water-look"]).water_look);
    assert!(!Config::default().water_look, "the sea is the control");
    assert!(cfg_of(&["--foliage-rich"]).foliage_rich);
    assert!(!Config::default().foliage_rich, "the field is the control");
    assert!(cfg_of(&["--canopy-relief"]).canopy_relief);
    assert!(!Config::default().canopy_relief, "the canopy is the control");
}

/// 97d and 97e are albedo terms -- the meadow lesson on the engine side: they multiply
/// what the light engine multiplies, never the light, so sun, AO and the flood treat
/// the varied world exactly like the plain one. Both are spelled from the shader; the
/// rarity bits are part of the look claim (a field of flowers is a different art
/// direction), so the thresholds are pinned too.
#[test]
fn foliage_and_canopy_are_guarded_albedo_terms() {
    let src = voxelcraft::render::shader_source().replace("\r\n", "\n");
    assert!(src.contains("override SPEC_FOLIAGE_RICH: bool = false;"));
    assert!(src.contains("override SPEC_CANOPY_RELIEF: bool = false;"));
    assert!(
        src.contains(
            "if SPEC_FOLIAGE_RICH\n            && (foliage || ((id == GRASS_ID || id == MEADOW_ID) && normal_id == NORMAL_PY))"
        ),
        "richness covers cross-quads and grass/meadow tops -- never a side face"
    );
    assert!(src.contains("if hr < 0.16 {"), "the straw stand is a sixth of a field");
    assert!(src.contains("} else if hr > 0.97 {"), "the flower is rare by construction");
    assert!(src.contains("if SPEC_CANOPY_RELIEF && is_cutout_id(id) {"), "relief is leaves-only");
    // The Wgsl side's id mirrors are pinned against the Rust truth: the constants live
    // in two files, which is the batch-91 drift the const-assert class exists for.
    assert!(src.contains("const GRASS_ID: u32 = 3u;"));
    assert!(src.contains("const MEADOW_ID: u32 = 19u;"));
    assert_eq!(voxelcraft::block::GRASS, 3);
    assert_eq!(voxelcraft::block::MEADOW, 19);
    // Albedo, not light: both blocks run *between* the tint/shore albedo terms and the
    // light gather -- that ordering is the claim, so it is placed, not implied.
    let rich = src.find("albedo = albedo * rich * mix(0.88, 1.10, hv);").expect("rich term");
    let light = src.find("let sky_tint = surface_sky_fill(n);").expect("light half");
    assert!(rich < light, "the species terms run before the light half of shade_hit");
}

/// Both spec_hi arms stand outside the sea_level block, beside the tint sibling -- land
/// colour must not drown with the sea. Spelled from scene.rs, and the spice term's
/// worldgen-mirror claim is explicit: geometry never moves.
#[test]
fn foliage_arms_stand_on_land_and_geometry_never_moves() {
    let scene = include_str!("../src/scene.rs");
    let tint = scene.find("if cfg.tint_balance {").expect("tint arm");
    let rich = scene.find("if cfg.foliage_rich {").expect("rich arm");
    let relief = scene.find("if cfg.canopy_relief {").expect("relief arm");
    assert!(relief > rich && rich > tint, "the land arms line up beside the tint arm");
    let worldgen = include_str!("../src/worldgen.rs");
    assert!(
        !worldgen.contains("foliage_rich") && !worldgen.contains("canopy_relief"),
        "worldgen must not know these arms exist -- the marcher sees the same world"
    );
}

/// 97c's three terms are three guarded blocks spelled as the shader spells them, and its
/// written numbers are the ones the by-eye pass tunes from. The ripple touches `nw` and
/// never `nt` -- the traced normal is the march's cost class, and a look arm that moved
/// it would be a perf arm pretending to be a look one.
#[test]
fn water_look_is_three_guarded_terms_and_nt_never_moves() {
    let src = voxelcraft::render::shader_source();
    assert!(src.contains("override SPEC_WATER_LOOK: bool = false;"));
    assert!(src.contains("if SPEC_WATER_LOOK && normal_id == NORMAL_PY {"), "ripple block");
    assert!(src.contains("if SPEC_WATER_LOOK && ctx.refract_detail > 0.0 {"), "murk/shore block");
    // 101d's cure made the trains' weights the caller's -- a per-wavelength fade can
    // only be applied where the weights live, so the signature carries them.
    assert!(src.contains(
        "fn look_ripple(p: vec2<f32>, time: f32, w1: f32, w2: f32) -> vec2<f32> {"
    ));
    assert!(src.contains("const WATER_LOOK_RIPPLE_AMP: f32 = 0.16;"));
    assert!(src.contains("const WATER_LOOK_MURK_DIST: f32 = 7.0;"));
    assert!(src.contains("const WATER_LOOK_SHORE_DEPTH: f32 = 2.0;"));
    let ripple = src.find("let rp = look_ripple(").expect("ripple site");
    let ctx_end = src.find("return WaterCtx(").expect("ctx return");
    let between = &src[ripple..ctx_end];
    assert!(
        !between.contains("nt = normalize") && !between.contains("nt.x"),
        "the arm refreshes nw only; nt runs to the march unchanged"
    );
    // The murk rides the foam's hinge: gated by the same refract_detail that says this
    // pixel actually traced a column depth.
    let murk = src.find("let depth_f = clamp(column_depth / WATER_LOOK_MURK_DIST, 0.0, 1.0);").expect("murk");
    let foam = src.find("if SPEC_SHORE_FOAM && ctx.refract_detail > 0.0").expect("foam");
    assert!(murk < foam, "the hue grade precedes the foam term it must not tint");
}

/// The spec_hi arm nests under `sea_level > 0` beside the shore family -- a waterless
/// world holds nothing for the arm to shade, and the nesting is the pin (spelled, not
/// assumed): the bit must appear *inside* the water block, after the snell sibling.
#[test]
fn water_look_nests_with_the_shore_family() {
    let scene = include_str!("../src/scene.rs");
    let snell = scene.find("if cfg.snell_bend {").expect("snell arm");
    let look = scene.find("if cfg.water_look {").expect("water arm");
    assert!(look > snell, "97c nests inside the sea_level block, after batch 91's bend");
    let caustics = scene.find("f |= render::FLAG_HI_CAUSTICS;").expect("caustics bit");
    assert!(
        look > caustics && look - caustics < 600,
        "the arm stands with the shore family, not out with the sky arms"
    );
}

/// The selector word is constructed in exactly one place: the config accessor. A call
/// site that spells its own cast arithmetic is a fifth variant nobody authored, so the
/// writers pass the accessor's word and the accessor's full truth table is pinned here.
#[test]
fn cine_is_selector_value_two_constructed_in_one_place() {
    assert_eq!(Config::default().grade_value(), 0);
    assert_eq!(cfg_of(&["--grade", "warm"]).grade_value(), 1);
    assert_eq!(cfg_of(&["--grade", "cine"]).grade_value(), 2);
    let app = include_str!("../src/app.rs");
    let headless = include_str!("../src/headless.rs");
    assert!(!app.contains("grade_warm as u32"), "app spells its own cast");
    assert!(!headless.contains("grade_warm as u32"), "headless spells its own cast");
}

/// 97b is a second branch on the same uniform, after the curve, on the luma pivot -- the
/// A10 law its sibling arm was measured under. Spellings from the file, applied: an
/// arm that desaturates without a luma pivot is a brightness re-tune, and that is the
/// one thing a blit arm may not be.
#[test]
fn cine_is_a_branch_after_the_warm_branch_on_the_luma_pivot() {
    let blit = include_str!("../src/render/shaders/blit.wgsl");
    let warm = blit.find("if blit.grade == 1u {").expect("warm branch");
    let cine = blit.find("if blit.grade == 2u {").expect("cine branch");
    assert!(cine > warm, "the cine block follows the warm block it extends");
    assert!(
        blit[cine..].contains("let luma = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));"),
        "cine must pivot on luma the way the warm cast does"
    );
    let encode = blit.find("if blit.encode != 0u {").expect("encode");
    assert!(cine < encode, "the sRGB encode is the last word, after every grade");
}

/// 97a is a constant swap, and the swap sites are spelled the way the shader spells them:
/// two `select(..., SPEC_SKY_COOL)` pairs feeding `sky_base`'s gradient and the deck's
/// lit/base pair. The number of swaps is the claim -- a fourth site would be a look the
/// arm's comment did not author.
#[test]
fn sky_cool_is_four_swaps_and_no_more() {
    let src = voxelcraft::render::shader_source();
    // Count usage *shapes*, not the bare name: counting the token counts comments as
    // text (batch 98's hardware round found a comment's mention posing as a site -- the
    // same lesson the pane census taught). The four select sites all end the name with
    // `, SPEC_SKY_COOL)`; the declaration is its own line.
    let sites = src.matches(", SPEC_SKY_COOL)").count();
    // Any other *usage* is an authorship drift the comment beside the override will not
    // have described.
    assert_eq!(
        sites, 4,
        "expected exactly 4 select-swap sites, found {sites}"
    );
    assert!(src.contains("override SPEC_SKY_COOL: bool = false;"));
    assert!(src.contains("const SKY_COOL_DAY_HORIZON: vec3<f32>"));
    assert!(src.contains("const SKY_COOL_DAY_ZENITH: vec3<f32>"));
    assert!(src.contains("const CLOUD_COOL_LIT: vec3<f32>"));
    assert!(src.contains("const CLOUD_COOL_BASE: vec3<f32>"));
    // The swap pair keeps the pre-97 constants as the select's false arm -- spelled, so
    // the *value* of the off build is pinned, not merely its construction.
    assert!(
        src.contains("select(SKY_DAY_HORIZON, SKY_COOL_DAY_HORIZON, SPEC_SKY_COOL)"),
        "the horizon swap must keep SKY_DAY_HORIZON as its false arm"
    );
    assert!(
        src.contains("select(SKY_DAY_ZENITH, SKY_COOL_DAY_ZENITH, SPEC_SKY_COOL)"),
        "the zenith swap must keep SKY_DAY_ZENITH as its false arm"
    );
}

/// The bit reaches the pipeline: named at mod.rs, present in the mask (pinned as a
/// property by spec_hi.rs's four assertions), and translated to a `SPEC_SKY_COOL`
/// constant in the override table -- the three wires a flag needs before the fold is real.
/// And `spec_hi_from` sets the bit *unconditionally on the world* (the sky is behind
/// everything), spelled from scene.rs rather than assumed.
#[test]
fn sky_cool_wires_from_cli_to_override() {
    let scene = include_str!("../src/scene.rs").replace("\r\n", "\n");
    assert!(
        scene.contains("if cfg.sky_cool {\n        f |= render::FLAG_HI_SKY_COOL;\n    }"),
        "the spec_hi arm is spelled once, unconditional on the world"
    );
    let render = include_str!("../src/render/mod.rs");
    assert!(render.contains("pub const FLAG_HI_SKY_COOL: u32 = 2048;"));
    assert!(render.contains("\"SPEC_SKY_COOL\""), "the override table knows the fold");
}

