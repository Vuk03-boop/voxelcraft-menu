use voxelcraft::config::{parse_from, Config};

const BLIT: &str = include_str!("../src/render/shaders/blit.wgsl");

fn cfg_of(args: &[&str]) -> Config {
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (cfg, _, unknown) = parse_from(&owned);
    assert!(unknown.is_empty(), "unexpected unknown args: {unknown:?}");
    cfg
}

#[test]
fn g1_grade_parses_like_its_tone_map_sibling() {
    assert!(cfg_of(&["--grade", "warm"]).grade_warm);
    assert!(!cfg_of(&["--grade", "none"]).grade_warm);
    assert!(!cfg_of(&["--grade", "golden"]).grade_warm, "unknown casts fall to off");
    assert!(!Config::default().grade_warm, "off is the control");
}

#[test]
fn g1_grade_is_a_branch_not_a_multiply() {
    assert!(
        BLIT.contains("if blit.grade == 1u {"),
        "the cast must be a guarded branch: batch 14 priced code billed whether it runs"
    );
    assert!(
        BLIT.contains("let luma = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));"),
        "the balance is luma-preserving; dropping the luma pivot re-tunes the world"
    );
    assert!(
        BLIT.contains("grade: u32,"),
        "the selector is one word of the existing uniform -- a pad spent, never a pipeline"
    );
}

#[test]
fn g1_sky_knobs_parse_and_clamp() {
    assert_eq!(cfg_of(&["--cloud-patch", "0.7"]).clouds.patch, 0.7);
    assert_eq!(cfg_of(&["--cloud-patch", "9.9"]).clouds.patch, 1.0);
    assert_eq!(cfg_of(&["--cloud-relief", "0.4"]).clouds.relief, 0.4);
    assert_eq!(cfg_of(&["--haze-warm", "-1"]).sky.haze_warm, 0.0);
    assert_eq!(cfg_of(&["--zenith-deep", "2"]).sky.zenith_deep, 1.0);
}

#[test]
fn g1_controls_default_to_the_pre95_frame() {
    let d = Config::default();
    assert_eq!(d.clouds.patch, 0.0, "patch: banks are opt-in");
    assert_eq!(d.clouds.relief, 0.0, "relief: flat deck is the control");
    assert_eq!(d.sky.haze_warm, 0.0, "the warm band is opt-in");
    assert_eq!(d.sky.zenith_deep, 0.0, "the deep zenith is opt-in");
    assert!(!d.grade_warm, "the cast is opt-in (pinned with its own tests)");
}

#[test]
fn g1_every_arm_is_a_guarded_branch() {
    let src = voxelcraft::render::shader_source();
    for spelling in [
        "if SPEC_SKY_LOOK && frame.cloud_patch > 0.0 {",
        "if SPEC_SKY_LOOK && frame.cloud_relief > 0.0 {",
        "if SPEC_SKY_LOOK && frame.haze_warm > 0.0 {",
        "if SPEC_SKY_LOOK && frame.zenith_deep > 0.0 {",
    ] {
        assert!(
            src.contains(spelling),
            "missing guarded branch {spelling:?} -- an armed look must cost nothing unarmed"
        );
    }
}

#[test]
fn g1_the_patch_is_a_bank_and_the_relief_only_shades() {
    let src = voxelcraft::render::shader_source();
    assert!(src.contains("const CLOUD_PATCH_SPAN: f32 = 6.0;"));
    assert!(src.contains("fp / CLOUD_PATCH_SPAN"), "the band limit follows the span");
    let dens = src.find("let dens = smoothstep(thr, thr + CLOUD_SOFT, n);").expect("dens");
    let relief = src.find("if SPEC_SKY_LOOK && frame.cloud_relief > 0.0 {").expect("relief block");
    assert!(
        dens < relief,
        "density is decided before shading runs -- relief can never grow a cloud"
    );
}

#[test]
fn g1_the_fold_is_one_override_reading_clamped_knobs() {
    let src = voxelcraft::render::shader_source();
    assert!(
        src.contains("override SPEC_SKY_LOOK: bool = false;"),
        "false is the law here, same as the family it sits in"
    );

    assert!(
        !src.contains("if frame.cloud_patch > 0.0 {"),
        "a uniform-only guard re-arms the reassociation miss the 95d fold closed"
    );
}
