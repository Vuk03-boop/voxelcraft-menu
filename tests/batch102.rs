mod support;
use voxelcraft::config::{parse_from, Config};
use voxelcraft::render as r;

const COMMON: &str = include_str!("../src/render/shaders/common.wgsl");
const RESOLVE: &str = include_str!("../src/render/shaders/resolve.wgsl");
const RENDER_RS: &str = include_str!("../src/render/mod.rs");
const SCENE_RS: &str = include_str!("../src/scene.rs");

fn cfg_of(args: &[&str]) -> Config {
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (cfg, _, unknown) = parse_from(&owned);
    assert!(unknown.is_empty(), "unexpected unknown args: {unknown:?}");
    cfg
}

#[test]
fn the_soft_shadows_bit_rides_the_pipeline_key_word() {
    assert_eq!(
        r::FLAG_HI_SOFT_SHADOWS, 65536,
        "the next free hi bit after wind-sway's 32768"
    );
    assert_ne!(
        r::SPEC_HI_MASK & r::FLAG_HI_SOFT_SHADOWS,
        0,
        "a hi bit outside SPEC_HI_MASK keys no pipeline at all -- the arm compiles inert"
    );

    assert!(RENDER_RS.contains("\"SPEC_SOFT_SHADOWS\""));
    assert!(RENDER_RS.contains("spec_hi & FLAG_HI_SOFT_SHADOWS"));
}

#[test]
fn the_flag_parses_off_by_default_and_keys_the_spec_word() {
    assert!(!cfg_of(&[]).soft_shadows, "default off");
    assert!(
        cfg_of(&["--soft-shadows"]).soft_shadows,
        "the arm takes the flag"
    );
    assert!(
        SCENE_RS.contains("cfg.soft_shadows"),
        "spec_hi_from must read the config field"
    );
    assert!(
        SCENE_RS.contains("render::FLAG_HI_SOFT_SHADOWS"),
        "spec_hi_from must set the bit"
    );
}

#[test]
fn soft_shadows_reconstruct_a_single_ray_in_screen_space() {
    let shadow=include_str!("../src/render/shaders/shadow.wgsl");
    assert!(!COMMON.contains("fn sun_vis_soft("));
    assert!(!RESOLVE.contains("sun_vis_soft("));
    let raw=support::wgsl_function(shadow,"primary_shadow");
    assert_eq!(raw.matches("shadow_ray_t(").count(),1);
    assert!(shadow.contains("shadow_horizontal"));
    assert!(shadow.contains("shadow_vertical"));
    assert!(shadow.contains("texture_storage_2d<r16float, write>"));
    assert!(shadow.contains("texture_storage_2d<r8unorm, write>"));
}
#[test]
fn only_secondary_schedules_retain_inline_hard_shadow_calls() {
    for arm in ["shade_hit_legacy", "shade_hit_compact"] {
        assert_eq!(support::wgsl_function(RESOLVE,arm).matches("shadow_ray(").count(),1);
    }
    for arm in ["shade_hit_legacy_cached", "shade_hit_compact_cached"] {
        assert!(!support::wgsl_function(RESOLVE,arm).contains("shadow_ray("));
    }
    assert!(RENDER_RS.contains("SHADOW_RADIUS_LIMIT"));
}
