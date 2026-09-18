//! Batch 95: roadmap G1, the first face of the look goal -- sky, deck, horizon and grade --
//! everything dark behind its own flag under the batch-90 standing law, aimed by eye at the
//! two reference frames in `docs/look-reference-*.png`.
//!
//! The arms are deliberately two kinds of uniform. The grade rides the blit's one selector
//! word (a `pad` spent, so the presentation's layout never moved), and the three sky knobs
//! are batch 95's four-word frame append, the sixth of its line. All four default to the
//! pre-95 frame -- that is the control -- and the first hardware round measured *why the
//! control may not ride a uniform guard*: `if frame.knob > 0.0` is a branch the compiler
//! still surrounds, and the 21-vantage flagship came back with seven vantages moving 1-4
//! pixels at max delta 1, pure reassociation. Batch 95d moved all four guards behind one
//! `SPEC_SKY_LOOK` override (`SPEC_TINT_BALANCE`'s folding class): bit clear, the arm's
//! text is not in the module, and the control is bit-exact by construction rather than by
//! luck. The knobs stay uniforms past the fold, which is what keeps the by-eye ladders
//! one-build cheap.
//!
//! Same evidence class as every source suite: what can be pinned without a device is the
//! architecture and the defaults; the acceptance readings (which banks and which warmth the
//! eye prefers against the references) are the hardware round's.

use voxelcraft::config::{parse_from, Config};

const BLIT: &str = include_str!("../src/render/shaders/blit.wgsl");

fn cfg_of(args: &[&str]) -> Config {
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (cfg, _, unknown) = parse_from(&owned);
    assert!(unknown.is_empty(), "unexpected unknown args: {unknown:?}");
    cfg
}

/// The parser carries the cast the way `--tone-map` carries its shoulder: an unknown
/// spelling falls to the one the engine has always shipped rather than failing loudly --
/// a presentation the engine won't run silently is worse than the spelling it did not get.
#[test]
fn g1_grade_parses_like_its_tone_map_sibling() {
    assert!(cfg_of(&["--grade", "warm"]).grade_warm);
    assert!(!cfg_of(&["--grade", "none"]).grade_warm);
    assert!(!cfg_of(&["--grade", "golden"]).grade_warm, "unknown casts fall to off");
    assert!(!Config::default().grade_warm, "off is the control");
}

/// The grade is a branch on a uniform, in the one file that tone-maps, *after* the curve --
/// so it cannot re-tune a lighting constant (it is luma-preserving, and that is in the
/// shader's own comment where the next reader will look for it) and off the flag the frame
/// is the pre-95 frame bit for bit. Spelling pinned rather than presence guessed, the same
/// correction batch 92's a8_1 took.
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

/// The four sky knobs parse and clamp like A1's lift: values outside 0..1 cannot reach the
/// GPU at all, because the clamp sits in the parser rather than being re-invented at every
/// read (`--tint-balance`'s one-place rule).
#[test]
fn g1_sky_knobs_parse_and_clamp() {
    assert_eq!(cfg_of(&["--cloud-patch", "0.7"]).clouds.patch, 0.7);
    assert_eq!(cfg_of(&["--cloud-patch", "9.9"]).clouds.patch, 1.0);
    assert_eq!(cfg_of(&["--cloud-relief", "0.4"]).clouds.relief, 0.4);
    assert_eq!(cfg_of(&["--haze-warm", "-1"]).sky.haze_warm, 0.0);
    assert_eq!(cfg_of(&["--zenith-deep", "2"]).sky.zenith_deep, 1.0);
}

/// The control is a *property*, not a number somebody typed: an unflagged build carries all
/// four zeros, and zeros are what the shader's guards early-exit on -- so the control frame
/// is the pre-95 frame bit for bit, and the 21-vantage flagship run has a real partner.
#[test]
fn g1_controls_default_to_the_pre95_frame() {
    let d = Config::default();
    assert_eq!(d.clouds.patch, 0.0, "patch: banks are opt-in");
    assert_eq!(d.clouds.relief, 0.0, "relief: flat deck is the control");
    assert_eq!(d.sky.haze_warm, 0.0, "the warm band is opt-in");
    assert_eq!(d.sky.zenith_deep, 0.0, "the deep zenith is opt-in");
    assert!(!d.grade_warm, "the cast is opt-in (pinned with its own tests)");
}

/// Each armed look is a *guarded branch* in the shader, spelled as the shader spells it --
/// batch 14 priced code compiled into the pass whether it runs or not, so none of these may
/// be a multiply-by-zero hiding in front of real work. Batch 92's a8_1 lesson applied:
/// these strings were read out of the WGSL after it was written, not guessed alongside it.
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

/// The patch octave's band limit follows its own pixel size and its span is named, because
/// both were the batch-10 deck's own correctness arguments and a copy that drops them
/// re-learns them at the horizon. Shading-only relief, likewise: `dens` is fixed *before*
/// the relief block runs, so a shading term cannot grow a cloud -- that sentence is the
/// property, and the property is the pin.
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

/// The guards fold through the one override, and the override reads the *clamped* parse
/// values -- so `--cloud-patch 0.0` is the same build as no flag, and the unflagged build
/// folds the arm's text out of the module. This is the 95d repair's shape, pinned so a
/// future "guard is enough" refactor trips it: the bit is declared in the word, fed to the
/// pipeline constants table, and default-off in the shader, three places one name ties.
#[test]
fn g1_the_fold_is_one_override_reading_clamped_knobs() {
    let src = voxelcraft::render::shader_source();
    assert!(
        src.contains("override SPEC_SKY_LOOK: bool = false;"),
        "false is the law here, same as the family it sits in"
    );
    // The bit is the word's by the spec_hi property suite; what this file pins is *why*:
    // the guard that failed the 21-vantage control was a uniform branch, and anything
    // reaching for `if frame.cloud_patch > 0.0 {` without the fold is that failure again.
    assert!(
        !src.contains("if frame.cloud_patch > 0.0 {"),
        "a uniform-only guard re-arms the reassociation miss the 95d fold closed"
    );
}

