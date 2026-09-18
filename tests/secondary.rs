use voxelcraft::render;

fn fn_body(name: &str) -> String {
    let src = render::shader_source();
    let pat = format!("fn {name}(");
    let at = src
        .find(&pat)
        .unwrap_or_else(|| panic!("{name} is not declared in the shader"));
    let body = &src[at..];
    let end = body[1..]
        .find("\nfn ")
        .unwrap_or_else(|| panic!("another function follows {name}"));
    body[..end]
        .lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn secondary_smooth_does_not_read_frame_flags() {
    let code = fn_body("secondary_smooth");
    assert!(
        !code.contains("frame.flags"),
        "secondary_smooth reads frame.flags, so the gate is a run-time value and both arms of \
         shade_hit's light section stay compiled into all three secondary copies -- measured at \
         +1.000 ms of `resolve` at `terraces` against -1.584 for the folded form. The flag is \
         still real and still in SPEC_MASK; it keys the pipeline cache, exactly as \
         FLAG_WATER_SHADOW_CUT does, and a second copy of the decision inside the shader can \
         only cost. Body was:\n{code}"
    );
    assert!(
        code.contains("SPEC_FLAT_SECONDARY"),
        "secondary_smooth should return the override -- otherwise the control cannot reach it \
         at all. Body was:\n{code}"
    );
}

#[test]
fn the_primary_hit_asks_for_smooth_light_by_literal() {
    let src = render::shader_source();
    let at = src
        .find("fn shade_hit_cached(")
        .expect("the shade_hit_cached function exists");
    let call = &src[at..at + 300];
    assert!(
        call.contains("frame.shadow_dist,true,true,pix)"),
        "the primary cached shade_hit calls should pass `true` for smooth_light as a literal. Call was:\n{call}"
    );
}

#[test]
fn every_secondary_hit_goes_through_the_gate() {
    let src = render::shader_source();
    let calls = src.matches("shade_hit(").count();

    assert_eq!(
        calls, 5,
        "expected `shade_hit(` to appear five times -- one declaration and four secondary calls."
    );
    let gated = src.matches("secondary_smooth()").count();

    assert_eq!(
        gated, 5,
        "expected `secondary_smooth()` at its declaration and at each of the four secondary \
         call sites; a call site that open-codes the override instead puts a second copy of \
         the decision in the module"
    );
}
