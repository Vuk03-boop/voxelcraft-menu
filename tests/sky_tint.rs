//! Batch 63, roadmap R7: the ambient's sky hue, derived from the sky model the renderer draws.
//!
//! **Every claim here is about a number that is spelled twice**, which is the one failure mode
//! this batch could have that no picture would catch. `sky_ambient_tint` integrates the same
//! gradient `sky_base` evaluates, and two of its constants are *results* of expressions over
//! that gradient which WGSL cannot fold for itself -- there is no const `mix` and no const
//! `pow` -- so they ship as literals. A literal that has drifted from the expression it stands
//! for renders a perfectly plausible frame at a slightly wrong hue, and the control would still
//! be bit-exact, and `bitexact` would still pass, because the control reverts the *whole*
//! function rather than the constant inside it.
//!
//! So these tests do to R7 what `tests/shaft.rs` does to the envelope: recompute the claim in
//! Rust and compare. They are not a second copy of the sky model -- they are the arithmetic that
//! turns four endpoints already in the shader into the two literals beside them, which is a
//! different thing and is exactly what `CLAUDE.md` means by a constant carrying its own why.
//!
//! **What is deliberately not here.** That `FLAG_SKY_TINT` is in `SPEC_MASK` and that it is bit
//! 31 are `const _: () = assert!`s in `render`, which fail the build rather than a test run;
//! restating them at run time is what clippy calls an assertion on a constant. And the look
//! itself is not testable here at all -- it is a screenshot pair, and `docs/shading.md` carries
//! the numbers.

use voxelcraft::{block, render};

/// A `vec3<f32>` constant's three components, read out of the shader source.
///
/// Anchored on `const NAME:` so that a mention of the name in a comment or in another
/// expression cannot satisfy it -- the same care `tests/secondary.rs` takes for the same reason.
fn const_vec3(name: &str) -> [f32; 3] {
    let src = render::shader_source();
    let pat = format!("const {name}: vec3<f32> = vec3<f32>(");
    let at = src
        .find(&pat)
        .unwrap_or_else(|| panic!("{name} is not declared in the shader"));
    let rest = &src[at + pat.len()..];
    let end = rest.find(')').expect("unterminated vec3 constant");
    let parts: Vec<f32> = rest[..end]
        .split(',')
        .map(|p| {
            p.trim()
                .parse()
                .unwrap_or_else(|_| panic!("{name} has a non-numeric component: {p}"))
        })
        .collect();
    assert_eq!(parts.len(), 3, "{name} is not three components");
    [parts[0], parts[1], parts[2]]
}

/// An `f32` constant's value, read out of the shader source.
fn const_f32(name: &str) -> f32 {
    let src = render::shader_source();
    let pat = format!("const {name}: f32 = ");
    let at = src
        .find(&pat)
        .unwrap_or_else(|| panic!("{name} is not declared in the shader"));
    let rest = &src[at + pat.len()..];
    let end = rest.find(';').expect("unterminated f32 constant");
    rest[..end]
        .trim()
        .parse()
        .unwrap_or_else(|_| panic!("{name} is not a number: {}", &rest[..end]))
}

/// `SKY_AMBIENT_REF` is the gradient at the reference condition, and not a number someone typed.
///
/// **This is the constant the whole batch is calibrated against.** R7 ships a *departure from*
/// `SKY_TINT` rather than a replacement of it, and the departure is measured against the sky a
/// face pointing straight up sees at full daylight. If this literal drifts from
/// `mix(SKY_DAY_HORIZON, SKY_DAY_ZENITH, SKY_W_UP)`, then the identity that makes the feature
/// rankable -- that the reference condition returns `SKY_TINT` to the bit -- quietly stops
/// holding, every surface in the world shifts hue, and nothing fails.
#[test]
fn the_reference_is_the_gradient_it_says_it_is() {
    let h = const_vec3("SKY_DAY_HORIZON");
    let z = const_vec3("SKY_DAY_ZENITH");
    let w = const_f32("SKY_W_UP");
    let got = const_vec3("SKY_AMBIENT_REF");
    for k in 0..3 {
        let want = h[k] + (z[k] - h[k]) * w;
        assert!(
            (got[k] - want).abs() < 2e-7,
            "SKY_AMBIENT_REF[{k}] is {} where mix(SKY_DAY_HORIZON, SKY_DAY_ZENITH, SKY_W_UP) is \
             {want}. The literal has drifted from the gradient it stands for, so the reference \
             condition no longer returns SKY_TINT and every shaded pixel in the world is off by \
             the difference -- invisibly, because --no-sky-tint reverts the whole function and \
             would still be bit-exact.",
            got[k]
        );
    }
}

/// `SKY_TINT_LUMA` is the luminance of `SKY_TINT`, in `block::luma`'s own weights.
///
/// The second identity the batch rests on is that **only the hue moves**: `sky_ambient_tint`
/// renormalises to this number, so if it is not `luma3(SKY_TINT)` then the feature is an
/// exposure change wearing a colour change's name -- which is precisely the trap roadmap R7
/// spends a paragraph on, and precisely the ambiguity that cost batch 57 a retreat.
#[test]
fn the_luminance_constant_is_sky_tints_own() {
    let tint = const_vec3("SKY_TINT");
    let got = const_f32("SKY_TINT_LUMA");
    let want = block::luma(tint);
    assert!(
        (got - want).abs() < 2e-7,
        "SKY_TINT_LUMA is {got} where block::luma(SKY_TINT) is {want}. The renormalisation \
         target is not SKY_TINT's luminance, so the derived tint is brighter or darker than the \
         constant it replaces and the before/after pair ranks an exposure rather than a colour."
    );
}

/// The shader's `luma3` carries `block::luma`'s weights and not a second opinion.
///
/// `block::luma`'s own comment says a second copy of these three weights is a second answer
/// waiting to disagree with the first. The shader needs a copy because it cannot call Rust, so
/// the copy is checked instead of avoided.
#[test]
fn the_shader_luminance_weights_match_the_rust_ones() {
    let src = render::shader_source();
    let at = src.find("fn luma3(").expect("luma3 is not declared in the shader");
    let end = src[at..].find('}').expect("unterminated luma3");
    let body = &src[at..at + end];
    for (w, axis) in [(0.2126_f32, "r"), (0.7152, "g"), (0.0722, "b")] {
        let spelled = format!("{w} * c.{axis}");
        assert!(
            body.contains(&spelled),
            "luma3 does not weight c.{axis} by {w}, which is what block::luma uses. Rec.709 in \
             one file and something else in the other makes probe.rs's BOUNCE_REF and R7's \
             reference incommensurable. Body was:\n{body}"
        );
    }
    // The Rust side, so a later edit there fails here rather than silently splitting the pair.
    assert!(
        (block::luma([1.0, 0.0, 0.0]) - 0.2126).abs() < 1e-7
            && (block::luma([0.0, 1.0, 0.0]) - 0.7152).abs() < 1e-7
            && (block::luma([0.0, 0.0, 1.0]) - 0.0722).abs() < 1e-7,
        "block::luma is no longer Rec.709, so the shader's copy above is now the stale one"
    );
}

/// `SKY_W_UP` is the closed form of the integral, which is what says the quadrature was right.
///
/// The up-facing case is the one weight with an analytic answer: the whole hemisphere is sky, so
/// the cosine-weighted mean of `u^SKY_GRAD_EXP` is `2 / (2 + SKY_GRAD_EXP)`. The side weight has
/// no such form and was taken by quadrature -- and the only reason to believe that quadrature is
/// that the same routine reproduces this number, so if the exponent ever moves and this constant
/// does not, the side weight beside it is unbelievable too.
#[test]
fn the_up_weight_is_the_closed_form_of_the_gradient_exponent() {
    let e = const_f32("SKY_GRAD_EXP");
    let got = const_f32("SKY_W_UP");
    let want = 2.0 / (2.0 + e);
    assert!(
        (got - want).abs() < 1e-7,
        "SKY_W_UP is {got} where 2 / (2 + SKY_GRAD_EXP) is {want}. Either the exponent moved \
         without the weights being re-derived, or the weight was typed rather than computed -- \
         and SKY_W_SIDE, which has no closed form to check, came out of the same quadrature."
    );
    // The side weight is bounded by construction rather than checked against a formula: it
    // averages the same power over *less* of the sky, all of it lower, so it has to be smaller.
    let side = const_f32("SKY_W_SIDE");
    assert!(
        side > 0.0 && side < got,
        "SKY_W_SIDE is {side}, which is not between zero and SKY_W_UP's {got}. A wall sees a \
         strictly lower-elevation part of the gradient than a floor does, so this ordering is \
         what makes a wall warmer than a floor rather than the other way round."
    );
}

/// The four gradient endpoints are spelled once, in `common.wgsl`, and read by both users.
///
/// Until this batch they were literals inside `sky_base` and had one reader. Hoisting them was
/// the precondition for R7, because the alternative -- `sky_ambient_tint` carrying its own copy
/// -- is the exact shape of batch 26, where four places described one amplitude and the one the
/// renderer read was the odd one out.
#[test]
fn the_gradient_endpoints_have_exactly_one_spelling() {
    let src = render::shader_source();
    for (name, want) in [
        ("SKY_NIGHT_HORIZON", [0.0049_f32, 0.0060, 0.0134]),
        ("SKY_DAY_HORIZON", [0.4480, 0.6210, 0.8900]),
        ("SKY_NIGHT_ZENITH", [0.0008, 0.0015, 0.0039]),
        ("SKY_DAY_ZENITH", [0.0637, 0.2140, 0.8276]),
    ] {
        assert_eq!(
            const_vec3(name),
            want,
            "{name} is not the value sky_base shipped with, so hoisting it changed the sky"
        );
        let spelled = format!(
            "vec3<f32>({:.4}, {:.4}, {:.4})",
            want[0], want[1], want[2]
        );
        let n = src.matches(&spelled).count();
        assert_eq!(
            n, 1,
            "{spelled} appears {n} times in the shader, not once. {name} exists so that \
             sky_base and sky_ambient_tint read one copy; a second spelling is a second answer \
             waiting to disagree, and the frame it renders is plausible either way."
        );
    }
}

/// The control is the override alone, and it returns the constant rather than approximating it.
///
/// `--no-sky-tint` has to be a *pure* revert, and the cheapest way to be sure is that the
/// cleared arm returns `SKY_TINT` itself -- not a formula that happens to evaluate to it, which
/// would be a claim about how naga reassociates and is the thing batch 58 lost two pixels at
/// `terraces` to.
#[test]
fn the_control_returns_the_constant_itself() {
    let src = render::shader_source();
    let at = src
        .find("fn sky_ambient_tint(")
        .expect("sky_ambient_tint is not declared in the shader");
    let body: String = src[at..]
        .lines()
        .take_while(|l| !l.starts_with("fn ") || l.contains("sky_ambient_tint"))
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        body.contains("if !SPEC_SKY_TINT") && body.contains("return SKY_TINT;"),
        "sky_ambient_tint does not return SKY_TINT unchanged when the override is clear, so \
         --no-sky-tint is not a pure revert and every pre-63 vantage can differ by an ULP. \
         Body was:\n{body}"
    );
    assert!(
        !body.contains("frame.flags"),
        "sky_ambient_tint reads frame.flags, so the gate is a run-time value and both arms stay \
         compiled into all four of shade_hit's inlined copies -- which is batch 45's +1.42 ms in \
         miniature. The flag is still real and still keys the pipeline. Body was:\n{body}"
    );
}



