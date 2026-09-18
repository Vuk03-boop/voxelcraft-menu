//! Batch 91: roadmap A5 (blue-noise canopy decimation; its behavioural tests sit in
//! `tests/worldgen.rs` beside the generator) and A2c (the marcher's two-segment Snell bend).
//!
//! What a source-level suite can pin of a marcher arm without a device: the flag's default,
//! its spelling, its bit, and the *architecture* the entry ruled on -- that `resolve` gains
//! nothing, because the bend hands it the same `rd` and a `dist` it already understands, and
//! that the ray-nudge epsilon exists once for both files. The wave-normal successor (moving
//! the wave field to `common.wgsl`) is expected to touch this file's "resolve does not read
//! the arm" pin exactly once, and the pin's message says so.

use voxelcraft::config::{parse_from, Config};
use voxelcraft::render::{self, FLAG_HI_SNELL_BEND};

fn cfg_of(args: &[&str]) -> Config {
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (cfg, _, unknown) = parse_from(&owned);
    assert!(unknown.is_empty(), "unexpected unknown args: {unknown:?}");
    cfg
}

#[test]
fn a2c_flag_defaults_off_and_parses() {
    assert!(!Config::default().snell_bend);
    assert!(cfg_of(&["--snell-bend"]).snell_bend);
}

/// The word's third batch-90-family bit. Membership, distinctness and union-completeness
/// are `tests/spec_hi.rs`'s properties; this is the one allocation fact the batch took.
#[test]
fn a2c_bit_allocation() {
    assert_eq!(FLAG_HI_SNELL_BEND, 256);
}

/// The override default matches the flag default: dark unless built for -- a module
/// compiled for every other batch's specs must not carry the bend in any form.
#[test]
fn a2c_override_default_is_dark() {
    let src = render::shader_source();
    assert!(src.contains("override SPEC_SNELL_BEND: bool = false;"));
}

/// **The entry's ruling, pinned in the one place it can live**: the cone, the Fresnel split
/// and the mirror term are derived in `resolve` from the original `rd` and from `dist`, and
/// the bend preserves both -- so if `resolve` ever names this arm, the architecture has
/// changed and the change deserves a batch note, not a silent patch. The one sanctioned
/// reason to touch this pin is the wave-normal successor the march arm's comment names
/// (moving `wave_field` to `common.wgsl`), and even that reason does not require
/// `SPEC_SNELL_BEND` to appear in `resolve.wgsl`.
#[test]
fn a2c_resolve_gains_nothing() {
    let resolve = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/render/shaders/resolve.wgsl"
    ))
    .expect("resolve.wgsl");
    assert!(
        !resolve.contains("SPEC_SNELL_BEND"),
        "resolve read the bend's override -- see the pin's comment"
    );
}

/// One epsilon, one home: the bend starts a ray on the sea plane, and `resolve` starts legs
/// on surfaces, so both files share `SURFACE_EPS` from `common.wgsl`. A second definition
/// anywhere else is a mirror waiting to drift.
#[test]
fn a2c_one_surface_epsilon() {
    let common = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/render/shaders/common.wgsl"
    ))
    .expect("common.wgsl");
    assert!(common.contains("const SURFACE_EPS: f32 = 0.02;"));
    let mut defs = 0;
    for f in ["resolve.wgsl", "march.wgsl", "common.wgsl"] {
        let src = std::fs::read_to_string(format!(
            "{}/src/render/shaders/{f}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect(f);
        defs += src.matches("const SURFACE_EPS").count();
    }
    assert_eq!(defs, 1, "SURFACE_EPS is defined exactly once, in common.wgsl");
}


