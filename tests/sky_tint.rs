use voxelcraft::{block, render};

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

    assert!(
        (block::luma([1.0, 0.0, 0.0]) - 0.2126).abs() < 1e-7
            && (block::luma([0.0, 1.0, 0.0]) - 0.7152).abs() < 1e-7
            && (block::luma([0.0, 0.0, 1.0]) - 0.0722).abs() < 1e-7,
        "block::luma is no longer Rec.709, so the shader's copy above is now the stale one"
    );
}

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

    let side = const_f32("SKY_W_SIDE");
    assert!(
        side > 0.0 && side < got,
        "SKY_W_SIDE is {side}, which is not between zero and SKY_W_UP's {got}. A wall sees a \
         strictly lower-elevation part of the gradient than a floor does, so this ordering is \
         what makes a wall warmer than a floor rather than the other way round."
    );
}

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
