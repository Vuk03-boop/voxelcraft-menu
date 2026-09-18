//! Batch 54: the water surface seen from underneath, and the four things about it that no
//! capture can check.
//!
//! The feature is Snell's window -- a submerged eye sees the world above the surface only inside
//! a cone 41.4 degrees above horizontal, and outside it the surface is a mirror. It was reported
//! from play rather than found by a metric, at 32 blocks down with a mountain still drawn above
//! the horizon.
//!
//! **Seventeen of the eighteen vantages are bit-exact under the control**, which is what makes
//! these tests load-bearing rather than decorative: the fixture can see this feature at exactly
//! one camera, so everything about it that lives in the *source* has to be held here.

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

/// The body of `surface_from_below`, comments stripped.
///
/// Stripped for `tests/secondary.rs`'s reason: the explanation of why a thing is absent names
/// it, and a test its own documentation can fail is not checking the code.
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

/// The shader's flag is the renderer's bit; both halves are read at run time.
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

/// **The load-bearing test, and it is here because the batch shipped this bug once.**
///
/// The first build put the `FLAG_SNELL` test around the *traced mirror* only, so `--no-snell`
/// still ran the Fresnel mix against a flat body colour. The control then reverted the
/// **detail** of the mirror rather than the mirror: the frame moved by a max channel delta of 3
/// where it should have moved by 136, every other assertion in this file passed, and the only
/// thing that would have caught it is a `bitexact` row against the parent -- failing for a
/// reason no image explains.
///
/// A control has to reproduce the build it names, so the gate has to be the **first** thing the
/// function does and it has to return the argument untouched.
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
        "the FLAG_SNELL gate must come before the Fresnel term and the final mix, or          `--no-snell` reverts only part of the feature. Body was:\n\
{code}"
    );
    let head = &code[gate..fresnel];
    assert!(
        head.contains("return above"),
        "the gate has to return the caller's colour untouched, which is what makes the control          bit-exact rather than merely close. Body was:\n\
{code}"
    );
}

/// **No angle constant anywhere, and that is the design rather than tidiness.**
///
/// The critical angle is `asin(1 / WATER_IOR)` and nothing else decides it. Writing 48.6, or
/// 0.848, or a cosine of either, would put a *second* copy of the index of refraction in the
/// tree -- and the first copy already feeds `refract` in `shade_water`, so the two could drift
/// and the surface would reflect at one angle and bend at another.
///
/// Schlick against the **transmitted** cosine is what makes one expression do both jobs:
/// `sin2_t >= 1` is the total-internal-reflection test and the same `sin2_t` gives the curve
/// inside the window.
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

/// The ramp is a property of the **eye**, not of the ray.
///
/// Both of the user's corrections to the first build -- no mirror in the top three blocks, and
/// less surface light with depth -- are the two ends of one `smoothstep` over the *camera's*
/// depth. Driving it from the ray's own crossing instead would make the effect come and go as
/// the player turned rather than as they swam, which is the same distinction batch 53 drew for
/// the dark-water reach and rejected the loose version over.
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

/// The two ends of the ramp are ordered, and the near one is where the user put it.
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

/// **The fade stops short of one, and the reason is a limitation elsewhere.**
///
/// Fading the transmitted leg all the way to `underwater_body()` makes a deep frame uniformly
/// near-black, because that colour is `WATER_BODY * medium_light(light at the eye)` and the sky
/// flood is exactly zero below `WATER_DARK_DEPTH` -- so the only thing left in it is
/// `frame.ambient`. That is the 4-bit light nibble showing through, not clear water, and it is
/// roadmap L1's territory rather than this feature's.
///
/// If L1 ever gives deep water real in-scattered light, this cap is the first thing that should
/// go back up, and this test is where a reader will find out that it was ever held down.
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

/// **The mirror is the body colour and does not trace, and that was measured rather than
/// assumed.**
///
/// The obvious build traces a ray down into the water from the crossing point and shades what
/// it finds. It is **invisible and it cost four milliseconds**: against a mirror that is simply
/// `underwater_body()`, the traced one differs by a max channel delta of **3 at a level camera,
/// 3 pitched up 20 degrees, 7 at 45** -- while `resolve` at `deep-water` went from 3.28 ms to
/// 9.77. Dropping it took the whole feature from **+6.49 ms to +0.234**.
///
/// The reason is the reason it should never have been built: a mirrored ray at these angles
/// travels a long way through water and `water_medium` converges it to `underwater_body()` on
/// the way back, so **the free answer is the limit the expensive one was walking towards**.
///
/// This test is what stops it being rebuilt. If a later batch wants the traced mirror -- and
/// there is a case, at a *shallow* camera over a bright floor where the return path is short --
/// it should come back with a distance gate that keeps it off `deep-water`, and with its own
/// numbers.
#[test]
fn the_mirror_does_not_trace() {
    let code = term_body();
    assert!(
        !code.contains("trace_world"),
        "the surface-from-below term should not trace: the traced mirror is worth a max channel          delta of 3 to 7 and cost 4 ms of `resolve` at `deep-water`, because it converges to          `underwater_body()`, which is free. Body was:\n\
{code}"
    );
    assert!(
        !code.contains("shade_hit"),
        "nothing here shades a hit any more; the mirror is the medium's own colour. Body          was:\n\
{code}"
    );
    assert!(
        code.contains("underwater_body()"),
        "the mirror is `underwater_body()` -- the limit the traced version converged to. Body          was:\n\
{code}"
    );
}



