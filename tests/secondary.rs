//! Batch 45's flat secondary shading, and the one line the whole saving rests on.
//!
//! The feature is small -- a hit reached *through* water or glass takes one light lookup where
//! the primary hit keeps the nine-cell gather -- and it was measured twice, in two shapes that
//! differ by a single expression:
//!
//! | how `secondary_smooth()` is written | `resolve` at `terraces` | at `default` |
//! |---|---|---|
//! | `!(SPEC_FLAT_SECONDARY && (frame.flags & FLAG_FLAT_SECONDARY) != 0u)` | **+1.000 ms** | **+1.420** |
//! | `!SPEC_FLAT_SECONDARY` | **-1.584 ms** | **-0.502** |
//!
//! A 2.58 ms swing at one vantage, same arm taken, same pixels out. The first form is what
//! every *switch* override before batch 41 does, so it is the shape a later batch tidying this
//! up would naturally reach for -- and it is wrong here for a reason that is invisible in the
//! source: `frame.flags` is a run-time value, so the driver cannot prove which arm `shade_hit`
//! takes and keeps **both** the flat lookup and the nine-cell gather in each of the three
//! secondary copies it inlines. `default` is the row that proves it -- a frame with water only
//! at its right edge paid the largest regression of the six, which can only be code existing.
//!
//! That the flag is in `SPEC_MASK` is not tested here: `render` carries it as a
//! `const _: () = assert!`, which fails the *build* rather than a test run, and a runtime copy
//! of a compile-time assert is what clippy calls an assertion on a constant.
//!
//! These are claims about the shader *source*, the way `tests/cutout.rs` makes them, because
//! there is no Rust function to call and the defect does not show up in any image.

use voxelcraft::render;

/// The body of one WGSL function, with its comments stripped.
///
/// Comments are stripped first for `hit_t_does_not_branch_on_the_cutout_override`'s reason:
/// the explanation of *why* a name is absent from the code names it, and a test its own
/// documentation can fail is not checking the code.
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

/// The gate has to fold at pipeline-compile time, which means the override and nothing else.
///
/// This is the entire batch. See the table at the top of this file for what the other spelling
/// costs; the reasoning is at the function itself.
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

/// The primary hit keeps the gather, and says so with a literal rather than a variable.
///
/// The literal is what makes the fold exact: `shade_hit` is inlined per call site, and the
/// primary's copy has to reduce to the code that was there before batch 45 or the control
/// loses its bit-exactness -- which is the trap batch 38 recorded three shapes of at `hit_t`.
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

/// All four secondary call sites go through the one gate, and none open-codes it.
///
/// Four call sites -- water's refraction, water's reflection, glass's transmission, and
/// glass's reflection -- and a fifth transmissive material would be a fifth. Open-coding the override
/// at one of them would work and would put a second copy of the decision in the module, which is
/// the thing the function exists to prevent.
#[test]
fn every_secondary_hit_goes_through_the_gate() {
    let src = render::shader_source();
    let calls = src.matches("shade_hit(").count();
    // One declaration plus four calls.
    assert_eq!(
        calls, 5,
        "expected `shade_hit(` to appear five times -- one declaration and four secondary calls."
    );
    let gated = src.matches("secondary_smooth()").count();
    // One declaration, one `return` mentioning the override is not a call, four call sites.
    assert_eq!(
        gated, 5,
        "expected `secondary_smooth()` at its declaration and at each of the four secondary \
         call sites; a call site that open-codes the override instead puts a second copy of \
         the decision in the module"
    );
}



