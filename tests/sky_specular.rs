//! Batch 65, roadmap R6: a Fresnel-weighted reflection of the sky on every opaque surface.
//!
//! **The claims here are the ones no picture and no `bitexact` row can make.** `--no-sky-specular`
//! reverts the whole term, so a sweep is bit-exact whatever `GROUND_F0` and `GROUND_ROUGHNESS`
//! have drifted to -- the control cannot see inside the function it switches off. What these
//! tests hold is the shape of `schlick_ground` against the two functions it was deliberately
//! *not* folded into, and the one rule the second specialization word carries.
//!
//! **What is deliberately not here.** That `FLAG_HI_SKY_SPECULAR` is in `SPEC_HI_MASK` is a
//! `const _: () = assert!` in `render`, which fails the build rather than a test run. And the
//! look is a screenshot pair; `docs/shading.md` carries the numbers and the gain that was chosen.

use voxelcraft::render;

/// An `f32` constant's value, read out of the shader source. Anchored on `const NAME:` so that
/// a mention in a comment cannot satisfy it -- `tests/sky_tint.rs`'s helper, for its reason.
fn const_f32(name: &str) -> f32 {
    let src = render::shader_source();
    let pat = format!("const {name}: f32 = ");
    let at = src
        .find(&pat)
        .unwrap_or_else(|| panic!("{name} is not declared in the shader"));
    let rest = &src[at + pat.len()..];
    let end = rest.find(';').expect("unterminated f32 constant");
    rest[..end].trim().parse().expect("non-numeric f32 constant")
}

/// Schlick, as the shader spells it for ordinary ground: F0 at normal incidence, climbing to
/// `max(1 - roughness, F0)` at grazing rather than to 1.0.
fn schlick_ground(cos_i: f32) -> f32 {
    let f0 = const_f32("GROUND_F0");
    let top = (1.0 - const_f32("GROUND_ROUGHNESS")).max(f0);
    let m = (1.0f32 - cos_i).clamp(0.0, 1.0);
    f0 + (top - f0) * m * m * m * m * m
}

/// **The identity that makes the constant a calibration rather than a number.** At normal
/// incidence the curve is exactly F0, whatever the roughness is -- the roughness may only
/// change how far it climbs, never where it starts. A refactor that folded the cap into the
/// base would break this and nothing in any frame would say so.
#[test]
fn schlick_ground_is_exactly_f0_at_normal_incidence() {
    let f0 = const_f32("GROUND_F0");
    assert!(
        (schlick_ground(1.0) - f0).abs() < 1e-7,
        "normal incidence is {} and GROUND_F0 is {f0}",
        schlick_ground(1.0)
    );
}

/// **The defect the roughness cap was added to fix, stated as an inequality.** The first build
/// of this batch had no cap: Schlick reaches 1.0 at grazing for every F0, a hillside seen from
/// above is at grazing incidence over most of its extent, and the frame came back a blue-white
/// haze at MAE 15.65. The cap is what makes the term a sheen. If this ever reads 1.0 again the
/// picture is back to that build, and `--no-sky-specular` would still be bit-exact.
#[test]
fn schlick_ground_does_not_reach_one_at_grazing() {
    let grazing = schlick_ground(0.0);
    let top = (1.0 - const_f32("GROUND_ROUGHNESS")).max(const_f32("GROUND_F0"));
    assert!(
        (grazing - top).abs() < 1e-7,
        "grazing is {grazing}, expected the roughness ceiling {top}"
    );
    assert!(
        grazing < 0.5,
        "grazing reflectance {grazing} is high enough to wash the frame out; \
         the first build of batch 65 measured MAE 15.65 doing exactly this"
    );
}

/// The curve still has angular shape. A roughness of 1.0 would flatten it to a constant F0,
/// which is not a specular term at all -- it is a slightly brighter ambient, and it would be
/// invisible in exactly the way roadmap R7 was.
#[test]
fn schlick_ground_still_rises_with_angle() {
    assert!(
        schlick_ground(0.0) > schlick_ground(1.0) * 2.0,
        "grazing {} is not meaningfully above normal incidence {}",
        schlick_ground(0.0),
        schlick_ground(1.0)
    );
}

/// **The rule the second specialization word carries, and the only one that can go wrong
/// silently.** `spec_hi` is CPU-side: it keys a pipeline and is never written into `GpuFrame`,
/// so there is no `frame.spec_hi` for a shader to read. A future bit placed there and then
/// tested at run time would compile, would read whatever `frame.flags` happens to hold at that
/// offset, and would render a plausible frame. See `render::SPEC_HI_MASK`.
#[test]
fn spec_hi_is_never_uploaded() {
    let src = render::shader_source();
    assert!(
        !src.contains("frame.spec_hi"),
        "a shader reads `frame.spec_hi`, which is never uploaded -- see render::SPEC_HI_MASK"
    );
    assert!(
        !src.contains("spec_hi"),
        "the shader mentions `spec_hi`; that word exists only on the CPU"
    );
}

/// The override is the whole gate, exactly as it is for every `SPEC_MASK` bit since
/// `FLAG_PROBE_TAP`. Batch 45 measured what adding the run-time `frame.flags` test costs on
/// this same function -- `shade_hit`, which `resolve` inlines four times -- and it was +1.42 ms.
#[test]
fn sky_specular_is_gated_by_the_override_alone() {
    let src = render::shader_source();
    assert!(
        src.contains("override SPEC_SKY_SPECULAR: bool"),
        "SPEC_SKY_SPECULAR is not declared as an override"
    );
    assert!(
        src.contains("if SPEC_SKY_SPECULAR && do_spec {"),
        "the specular arm is not gated on the override plus the primary-hit flag"
    );
}

/// **The whole of P13's fix, stated so it cannot be undone by accident.** `shade_hit` is
/// inlined *four* times -- the primary surface, water's refracted and reflected legs, and
/// glass's transmitted leg -- and with the specular arm live in all four, `resolve` compiled to
/// **128 registers against 96 without it**. That occupancy loss, and not the arithmetic, was
/// the entire cost of batch 65: +0.295 ms at `cave`, where the term changes **zero pixels**.
///
/// Passing a literal `false` at the three secondary call sites folds the arm away in those
/// copies and takes the shader back to **96**, recovering 83-87% of the bill.
///
/// **`do_spec` is a separate parameter and not a reuse of `smooth_light`, deliberately.**
/// `smooth_light` is `secondary_smooth()` at those three sites, which is `!SPEC_FLAT_SECONDARY`
/// and folds identically today -- so reusing it would work and would quietly make
/// `--no-flat-secondary` a second control for this feature, which is the coupling the roadmap
/// warns about every time it comes up.
#[test]
fn the_specular_arm_is_dead_in_the_three_secondary_call_sites() {
    let src = render::shader_source();
    let calls: Vec<&str> = src.match_indices("shade_hit(").map(|(i, _)| &src[i..]).collect();
    assert_eq!(
        calls.len(),
        5,
        "expected one definition and four call sites of shade_hit; found {}",
        calls.len()
    );
    // Four call sites: exactly one passes `true` for `do_spec` and three pass a literal
    // `false`. A `secondary_smooth()` in the last position would compile and would be the
    // coupling described above, so the literal is what is asserted.
    let secondary = src.matches("secondary_smooth(), false)").count()
        + src.matches("secondary_smooth(),false)").count();
    assert_eq!(
        secondary, 4,
        "expected four secondary shade_hit calls passing a literal false for do_spec, found {secondary} -- see roadmap P13"
    );
    assert!(
        src.contains("frame.shadow_dist,true,true,pix)"),
        "the primary cached shade_hit call asks for the specular term"
    );
}

/// `schlick_ground` is its own five lines and is **not** a parameterised `schlick`. Water's
/// Fresnel is read by every sea pixel in the fixture and glass's by every pane; `errors.md`'s
/// batch-38 entry is four paragraphs on what this compiler does when one value reaches a call
/// site through a different expression, and it cost that batch a control's bit-exactness for
/// 201 pixels. All three must still exist separately.
#[test]
fn the_three_fresnel_functions_are_separate() {
    let src = render::shader_source();
    for f in ["fn schlick(", "fn schlick_glass(", "fn schlick_ground("] {
        assert!(src.contains(f), "{f} is gone -- see errors.md on batch 38");
    }
    assert!(
        !src.contains("fn schlick(cos_i: f32, f0: f32)"),
        "schlick has grown an f0 parameter; that is the batch-38 ULP hazard"
    );
}



