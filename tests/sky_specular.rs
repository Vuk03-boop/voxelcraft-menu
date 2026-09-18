use voxelcraft::render;

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

fn schlick_ground(cos_i: f32) -> f32 {
    let f0 = const_f32("GROUND_F0");
    let top = (1.0 - const_f32("GROUND_ROUGHNESS")).max(f0);
    let m = (1.0f32 - cos_i).clamp(0.0, 1.0);
    f0 + (top - f0) * m * m * m * m * m
}

#[test]
fn schlick_ground_is_exactly_f0_at_normal_incidence() {
    let f0 = const_f32("GROUND_F0");
    assert!(
        (schlick_ground(1.0) - f0).abs() < 1e-7,
        "normal incidence is {} and GROUND_F0 is {f0}",
        schlick_ground(1.0)
    );
}

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

#[test]
fn schlick_ground_still_rises_with_angle() {
    assert!(
        schlick_ground(0.0) > schlick_ground(1.0) * 2.0,
        "grazing {} is not meaningfully above normal incidence {}",
        schlick_ground(0.0),
        schlick_ground(1.0)
    );
}

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
