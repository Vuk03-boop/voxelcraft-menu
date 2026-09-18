use voxelcraft::render;

fn shader_const_f32(name: &str) -> f32 {
    let src = render::shader_source();
    let pat = format!("const {name}: f32 = ");
    let at = src
        .find(&pat)
        .unwrap_or_else(|| panic!("{name} is not declared in the shader"));
    let rest = &src[at + pat.len()..];
    let end = rest.find(';').expect("an f32 literal");
    rest[..end].trim().parse().expect("an f32 literal")
}

fn shader_const_u32(name: &str) -> u32 {
    let src = render::shader_source();
    let pat = format!("const {name}: u32 = ");
    let at = src
        .find(&pat)
        .unwrap_or_else(|| panic!("{name} is not declared in the shader"));
    let rest = &src[at + pat.len()..];
    let end = rest.find('u').expect("a u32 literal");
    rest[..end].parse().expect("a u32 literal")
}

fn term_body() -> String {
    let src = render::shader_source();
    let at = src
        .find("fn surface_from_below(")
        .expect("surface_from_below is declared in the shader");
    let body = &src[at..];
    let end = body[1..]
        .find("\nfn ")
        .expect("another function follows surface_from_below");
    body[..end]
        .lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn snell_flag_matches_the_shader() {
    assert_eq!(shader_const_u32("FLAG_SNELL"), render::FLAG_SNELL);
    assert_eq!(
        render::SPEC_MASK & render::FLAG_SNELL,
        0,
        "FLAG_SNELL has to stay out of SPEC_MASK for FLAG_WATER_FAR's reason: the arm is only \
         ever reached with FLAG_UNDERWATER set, which is a run-time fact about where the player \
         is standing, so no pipeline key could fold it away"
    );
}

#[test]
fn the_control_gates_the_whole_term_and_not_just_a_part_of_it() {
    let code = term_body();
    let gate = code
        .find("FLAG_SNELL")
        .expect("the term is gated on FLAG_SNELL");
    let fresnel = code
        .find("sin2_t")
        .expect("the term computes the transmitted angle");
    let mixed = code
        .rfind("return mix(")
        .expect("the term ends by mixing what it decided into the caller's colour");
    assert!(
        gate < fresnel && gate < mixed,
        "the FLAG_SNELL gate must come before the Fresnel term and the final mix, or          `--no-snell` reverts only part of the feature. Body was:
{code}"
    );
    let head = &code[gate..fresnel];
    assert!(
        head.contains("return above"),
        "the gate has to return the caller's colour untouched, which is what makes the control          bit-exact rather than merely close. Body was:
{code}"
    );
}

#[test]
fn the_critical_angle_is_derived_from_the_index_and_not_written_down() {
    let code = term_body();
    assert!(
        code.contains("WATER_IOR"),
        "the term has to read WATER_IOR; it is the only thing that decides the critical angle. \
         Body was:\n{code}"
    );
    for bad in ["48.6", "41.4", "0.848", "0.661"] {
        assert!(
            !code.contains(bad),
            "the term contains `{bad}`, which is the critical angle written down instead of \
             derived. WATER_IOR already decides it and `refract` in `shade_water` already reads \
             that constant; a second copy can drift. Body was:\n{code}"
        );
    }
}

#[test]
fn the_ramp_reads_the_eye_and_not_the_ray() {
    let code = term_body();
    let at = code
        .find("smoothstep(SNELL_MIN_DEPTH")
        .expect("the depth ramp is a smoothstep between the two constants");
    let head = &code[..at];
    assert!(
        head.contains("frame.sea_level - frame.cam_pos.y"),
        "the ramp's argument has to be the eye's own depth. Body was:\n{code}"
    );
    assert!(
        code[at..].contains("WATER_DARK_DEPTH"),
        "the ramp's far end is batch 53's derived depth -- where the sky flood reaches zero -- \
         so the two halves of the engine agree about where deep water starts. Body was:\n{code}"
    );
}

#[test]
fn the_ramp_runs_from_the_user_s_three_blocks_to_the_flood_s_reach() {
    let near = shader_const_f32("SNELL_MIN_DEPTH");
    let far = shader_const_f32("WATER_DARK_DEPTH");
    assert!(
        near > 0.0 && near < far,
        "SNELL_MIN_DEPTH is {near} against WATER_DARK_DEPTH {far}; the ramp has to have a \
         positive width and start above the surface"
    );
    assert_eq!(
        near, 3.0,
        "SNELL_MIN_DEPTH is {near}. Three is the user's own call, given on the first build: \
         \"the first 3 blocks of water should not have a mirror, the mirror should be below \
         them\". Changing it is allowed and re-deciding it silently is not -- the constant's \
         own comment carries the sentence."
    );
}

#[test]
fn the_surface_light_fade_is_capped_below_one() {
    let dim = shader_const_f32("SNELL_DIM_MAX");
    assert!(
        dim > 0.0 && dim < 1.0,
        "SNELL_DIM_MAX is {dim}. At 1.0 a frame 24 blocks down is uniformly near-black, because \
         the body colour it fades to bottoms out at frame.ambient once the sky flood has run \
         out. At 0 the feature's second half does nothing."
    );
}

#[test]
fn the_mirror_does_not_trace() {
    let code = term_body();
    assert!(
        !code.contains("trace_world"),
        "the surface-from-below term should not trace: the traced mirror is worth a max channel          delta of 3 to 7 and cost 4 ms of `resolve` at `deep-water`, because it converges to          `underwater_body()`, which is free. Body was:
{code}"
    );
    assert!(
        !code.contains("shade_hit"),
        "nothing here shades a hit any more; the mirror is the medium's own colour. Body          was:
{code}"
    );
    assert!(
        code.contains("underwater_body()"),
        "the mirror is `underwater_body()` -- the limit the traced version converged to. Body          was:
{code}"
    );
}
