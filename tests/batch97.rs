use voxelcraft::config::{parse_from, Config};

fn cfg_of(args: &[&str]) -> Config {
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (cfg, _, unknown) = parse_from(&owned);
    assert!(unknown.is_empty(), "unexpected unknown args: {unknown:?}");
    cfg
}

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

    assert!(src.contains("const GRASS_ID: u32 = 3u;"));
    assert!(src.contains("const MEADOW_ID: u32 = 19u;"));
    assert_eq!(voxelcraft::block::GRASS, 3);
    assert_eq!(voxelcraft::block::MEADOW, 19);

    let rich = src.find("albedo = albedo * rich * mix(0.88, 1.10, hv);").expect("rich term");
    let light = src.find("let sky_tint = surface_sky_fill(n);").expect("light half");
    assert!(rich < light, "the species terms run before the light half of shade_hit");
}

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

#[test]
fn water_look_is_three_guarded_terms_and_nt_never_moves() {
    let src = voxelcraft::render::shader_source();
    assert!(src.contains("override SPEC_WATER_LOOK: bool = false;"));
    assert!(src.contains("if SPEC_WATER_LOOK && normal_id == NORMAL_PY {"), "ripple block");
    assert!(src.contains("if SPEC_WATER_LOOK && ctx.refract_detail > 0.0 {"), "murk/shore block");

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

    let murk = src.find("let depth_f = clamp(column_depth / WATER_LOOK_MURK_DIST, 0.0, 1.0);").expect("murk");
    let foam = src.find("if SPEC_SHORE_FOAM && ctx.refract_detail > 0.0").expect("foam");
    assert!(murk < foam, "the hue grade precedes the foam term it must not tint");
}

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

#[test]
fn sky_cool_is_four_swaps_and_no_more() {
    let src = voxelcraft::render::shader_source();

    let sites = src.matches(", SPEC_SKY_COOL)").count();

    assert_eq!(
        sites, 4,
        "expected exactly 4 select-swap sites, found {sites}"
    );
    assert!(src.contains("override SPEC_SKY_COOL: bool = false;"));
    assert!(src.contains("const SKY_COOL_DAY_HORIZON: vec3<f32>"));
    assert!(src.contains("const SKY_COOL_DAY_ZENITH: vec3<f32>"));
    assert!(src.contains("const CLOUD_COOL_LIT: vec3<f32>"));
    assert!(src.contains("const CLOUD_COOL_BASE: vec3<f32>"));

    assert!(
        src.contains("select(SKY_DAY_HORIZON, SKY_COOL_DAY_HORIZON, SPEC_SKY_COOL)"),
        "the horizon swap must keep SKY_DAY_HORIZON as its false arm"
    );
    assert!(
        src.contains("select(SKY_DAY_ZENITH, SKY_COOL_DAY_ZENITH, SPEC_SKY_COOL)"),
        "the zenith swap must keep SKY_DAY_ZENITH as its false arm"
    );
}

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
