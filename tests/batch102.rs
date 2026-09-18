//! Batch 102a (`--soft-shadows`): the sun-disc penumbra, pinned where it can be pinned
//! without a device. The renders stay where renders live -- the closing hardware round
//! re-shoots the lookbook (now four views by fourteen combos) and takes the arm's cost
//! at the `--grass-dense` station, the bench the round before it standardised.
//!
//! What this file is really pinning is the *cost story*. DS01/DS02 replaced the
//! three-tap cone with one geometric ray per eligible receiver plus a screen-space
//! bilateral reconstruction, and the dead `SOFT_TAPS`/`SOFT_MISS_DIST` overrides went
//! with it; what is pinned here is the one-ray shape of `primary_shadow` and the pass
//! family around it. A change to any of that is a look or frame-time decision that
//! must update this file deliberately.

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

/// **Mask membership leads.** Batch 101's hardware round found a hi bit that was
/// defined, parsed, set and read by `make_spec` while absent from `SPEC_HI_MASK` --
/// it compiled both arms into one pipeline and was inert in silence, pixel-identical.
/// Every hi flag now takes its mask row first. (`tests/spec_hi.rs` proves the same
/// thing as a property of the enumeration; this assertion is the per-flag spelling
/// that a future single-flag revert cannot leave behind by accident.)
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
    // The word is what `make_spec` turns into the override; the entry must exist,
    // because an override left at its default is a flag that does nothing in silence.
    assert!(RENDER_RS.contains("\"SPEC_SOFT_SHADOWS\""));
    assert!(RENDER_RS.contains("spec_hi & FLAG_HI_SOFT_SHADOWS"));
}

/// Parse, default, and the free-function wiring into the second spec word. Default
/// off like every look arm before it: the by-eye pass sets the bit it reads.
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

// The original multi-ray contracts are intentionally superseded by DS01/DS02.
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
