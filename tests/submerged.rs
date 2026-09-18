//! Batch 48's submerged reach cut, and the distinction the whole feature rests on.
//!
//! A camera under water tests every resident chunk out to `frame.far` -- 2600 blocks -- and
//! marches whatever survives, through a medium that has already taken the image apart. The
//! roadmap's prescription for that was to clamp `frame.far` itself when the camera is
//! submerged: "a uniform change, no shader edit". It does not work, and the reason is not a
//! cost:
//!
//! **A ray's water path is not its length.** `water_path` is analytic against the sea plane,
//! so a ray leaves the medium at `(sea_level - cam.y) / rd.y` and travels every block after
//! that in clear air. At the fixture's own submerged eye -- three blocks down, pitched -14
//! degrees, 70-degree field of view -- the top of the frame is **+21 degrees**, and a ray
//! there is out of the water within **nine blocks**. Clamping `frame.far` deletes the world
//! above the waterline for the whole top third of the frame.
//!
//! So the test is per *tile* and on the water path: the most upward of a tile's four corner
//! rays decides, because it is the one that escapes soonest. If even that ray is still
//! submerged at `WATER_FAR_DIST`, every ray the tile marches is, and the tile can stop there.
//!
//! The second thing worth knowing, because it is what sets the constant: the bound is **blue's**
//! extinction. `WATER_EXT` is (0.280, 0.055, 0.020) per block, and sizing the reach off red --
//! or off the roadmap's own "absorption kills the image within tens of blocks", which is red's
//! number -- makes it about twenty times too short.
//!
//! These are claims about the shader *source*, the way `tests/secondary.rs` and
//! `tests/cutout.rs` make them, because there is no Rust function to call. That the flag is
//! **outside** `render::SPEC_MASK` is not tested here: `render` carries it as a
//! `const _: () = assert!`, which fails the build rather than a test run.

use voxelcraft::render;

/// A named WGSL `u32` const, as the shader declares it.
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

/// A named WGSL `f32` const, as the shader declares it.
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

/// The `tile_select` entry point's body, with comments stripped.
///
/// Stripped for `tests/secondary.rs`'s reason: the explanation of why a name is *absent* names
/// it, and a test its own documentation can fail is not checking the code.
fn tile_select_body() -> String {
    let src = render::shader_source();
    let at = src
        .find("fn tile_select(")
        .expect("tile_select is declared in the shader");
    let body = &src[at..];
    let end = body[1..]
        .find("\nfn ")
        .expect("another function follows tile_select");
    body[..end]
        .lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The shader's copy of the flag is the same bit as the renderer's.
///
/// Unlike every flag in `SPEC_MASK`, this one is read at run time and ANDed with
/// `FLAG_UNDERWATER`, so a disagreement here is not a pipeline-key mismatch that a sweep would
/// catch -- it is a tile that silently never clamps, or one that clamps when the camera is dry.
#[test]
fn water_far_flag_matches_the_shader() {
    assert_eq!(
        shader_const_u32("FLAG_WATER_FAR"),
        render::FLAG_WATER_FAR,
        "common.wgsl's FLAG_WATER_FAR and render::FLAG_WATER_FAR are one bit kept in two places"
    );
    assert_eq!(
        shader_const_u32("FLAG_UNDERWATER"),
        render::FLAG_UNDERWATER,
        "the reach test ANDs the two flags, so both halves have to agree across the boundary"
    );
}

/// **The load-bearing test.** The clamp is gated on being submerged, not on the feature alone.
///
/// A gate on `FLAG_WATER_FAR` by itself compiles, passes every other test in this file, and
/// cuts a *dry* camera's view distance to `WATER_FAR_DIST` -- which is 128 blocks against a
/// 2600-block default. It is the shape a tidy-up reaches for when it notices that a feature
/// flag is being ANDed with something else, and `underwater_flag` sets the other half per
/// frame from where the player is standing, so nothing at pipeline-build time can catch it.
#[test]
fn the_reach_is_gated_on_being_submerged() {
    let code = tile_select_body();
    assert!(
        code.contains("FLAG_UNDERWATER") && code.contains("FLAG_WATER_FAR"),
        "tile_select's reach has to test both flags: FLAG_WATER_FAR alone would clamp a dry \
         camera to WATER_FAR_DIST. Body was:\n{code}"
    );
    let gate = code
        .find("FLAG_WATER_FAR")
        .expect("the feature flag is tested in tile_select");
    let underwater = code
        .find("FLAG_UNDERWATER")
        .expect("the medium flag is tested in tile_select");
    assert!(
        gate.abs_diff(underwater) < 120,
        "the two flags should be tested in one expression rather than in separate branches, so \
         there is a single place that decides. Body was:\n{code}"
    );
}

/// The rule reads the ray's *water path*, which means it has to reach `sea_level`.
///
/// This is the difference between the shipped feature and the one the roadmap asked for. A
/// version that clamps on distance alone -- no `sea_level`, no corner-ray elevation -- is
/// exactly the uniform clamp, and it deletes everything above the waterline for every ray that
/// leaves the medium. Nothing in a *downward* capture can see that, which is why it is pinned
/// here rather than left to the fixture.
#[test]
fn the_reach_tests_the_water_path_and_not_the_ray_length() {
    let code = tile_select_body();
    assert!(
        code.contains("frame.sea_level"),
        "the reach test has to find where the ray leaves the water, which means reading \
         frame.sea_level. Without it this is a uniform far-clamp wearing a per-tile shape, and \
         it deletes the world above the waterline. Body was:\n{code}"
    );
    assert!(
        code.contains("d00.y") || code.contains(".y)"),
        "the most upward corner ray is what decides, so the test reads the corner rays' \
         elevation. Body was:\n{code}"
    );
}

/// The chunk loop culls against the tile's reach, not against `frame.far`.
///
/// The whole feature is this one substitution. Putting `frame.far` back at the cull site leaves
/// every other assertion in this file passing, the control bit-exact, and the feature doing
/// nothing at all -- which is the failure mode that looks exactly like a pass.
#[test]
fn the_cull_reads_the_tile_reach() {
    let code = tile_select_body();
    assert!(
        code.contains("dmin >= reach"),
        "tile_select should cull against the per-tile reach. A `dmin >= frame.far` here is the \
         pre-batch-48 behaviour with the reach computed and thrown away. Body was:\n{code}"
    );
    assert!(
        !code.contains("dmin >= frame.far"),
        "the old cull is still present, so the reach is dead code. Body was:\n{code}"
    );
}

/// The constant is shorter than the view distance, and long enough that the frame says so.
///
/// Both halves are the batch. **Shorter** is what makes it a feature at all -- a reach at or
/// past `--view-distance`'s 2600 is a no-op no capture could distinguish from the control.
///
/// **Long enough used to be the half no capture could check**, and batch 48 wrote this floor
/// from the medium's extinction instead: blue is 0.020 per block, so 200 is where
/// `exp(-0.020 d)` falls under 2%. **Batch 52 built the vantage, measured the whole curve, and
/// the user took the fast end of it.** The full ladder is at the constant itself and in
/// `PERF.md`; the three rows this floor rests on, measured at `sea-horizon` against an
/// unlimited reach, are **0 pixels at 264 and up**, **67,621 at max delta 23 for -1.930 ms at
/// the shipped 128**, and **175,890 at max delta 50 at 64**.
///
/// **So this floor is lower than batch 48's, and that needs saying plainly rather than
/// quietly.** `harness.md` records the shape of self-dealing exactly -- *lowering a floor
/// because the batch doing the lowering just made it harder to meet* -- and this is not it,
/// but it is close enough to it that the distinction has to be on the record. What changed is
/// not the difficulty: it is that 200 was **never measured**. It was blue's extinction standing
/// in for a capture nobody had, and the capture now exists and disagrees with it in both
/// directions at once -- the frame is exact at 264, *above* the old floor, and the shipped
/// value is 128, *below* it. A number that is wrong on both sides is not a floor that was
/// loosened; it is one that was replaced.
///
/// **The floor is 128 because 128 is a decision, and the next rung down is not.** 64 moves
/// 175,890 pixels at max delta 50 -- two and a half times the pixels at twice the delta -- and
/// nothing has priced what it buys. Going under this needs the bench and the picture again, at
/// this vantage, plus somebody deciding; it does not need a passing sweep, and a passing sweep
/// is all the other sixteen vantages can offer, since every one of them is bit-exact at every
/// reach above 32.
#[test]
fn the_reach_is_shorter_than_the_view_distance_and_sized_off_the_crop() {
    let reach = shader_const_f32("WATER_FAR_DIST");
    let default_view = voxelcraft::config::Config::default().lod.view_distance;
    assert!(
        reach > 0.0 && reach < default_view,
        "WATER_FAR_DIST is {reach}, against a default view distance of {default_view}; at or \
         past that it is a no-op"
    );
    assert!(
        reach >= 128.0,
        "WATER_FAR_DIST is {reach}, under the 128 batch 52 shipped. That value is a decided \
         trade -- -1.930 ms for 67,621 pixels of 921,600 at max delta 23, measured at \
         `sea-horizon` -- and the next rung down, 64, moves 175,890 at max delta 50 for a \
         saving nobody has priced. Shortening this needs that bench and that picture again, \
         not a passing sweep: the other sixteen vantages are bit-exact at every reach above \
         32 and will agree with anything. See the constant's own comment and `docs/water.md`."
    );
}

/// The floor above is an argument about a *vantage*, so the vantage has to still be there.
///
/// **This is the seam batch 48 fell through, inverted.** That batch's zeros were correct
/// readings of a camera that could not see the feature, and nothing in the tree said so. The
/// floor above is now founded on `sea-horizon` instead of on blue's extinction, which trades
/// one silent failure for another: delete the vantage, or point it somewhere with no distance
/// in it, and the floor becomes a number with no argument under it -- still passing.
///
/// `harness::CHECKS` carries the half that needs a GPU (the reach moves at least 150 pixels
/// there under `--no-water-far`). This carries the half that does not: the camera is still
/// submerged and still aimed along the horizontal. **Pitch is the one that matters** -- the
/// clamp fires only for tiles whose most upward corner ray is still under water at the reach,
/// which three blocks down is 0.67 degrees, so a vantage that drifted to the default -14
/// degree pitch would put the whole frame in the clamped region and no islands in it at all.
#[test]
fn the_reach_floor_still_has_a_vantage_under_it() {
    let v = voxelcraft::harness::vantage::find("sea-horizon")
        .expect("`sea-horizon` is the vantage the WATER_FAR_DIST floor is measured at");
    let arg = |name: &str| {
        v.args
            .iter()
            .position(|a| *a == name)
            .and_then(|i| v.args.get(i + 1))
            .and_then(|s| s.parse::<f32>().ok())
    };
    assert_eq!(
        arg("--cam-submerge"),
        Some(3.0),
        "`sea-horizon` has to be under the water for the clamp to fire at all; args were {:?}",
        v.args
    );
    assert_eq!(
        arg("--cam-pitch"),
        Some(0.0),
        "`sea-horizon` looks along the horizontal, and the default pitch is -14. A vantage \
         pitched down has every tile in the clamped region and no horizon in the frame, which \
         is `underwater` again -- the camera the reach could not be ranked at. Args were {:?}",
        v.args
    );
}

// ---------------------------------------------------------------- batch 53: the dark rung

/// **The one constant in this pair that is derived, and this is the derivation held down.**
///
/// `light.rs` pours sky light down an open column at `MAX_LEVEL` and takes one level off per
/// block of water, and `flood` decrements one per step in every direction -- so any path from
/// the surface to a cell `d` blocks down is at least `d` steps long, and that cell's sky light
/// is bounded by `max(MAX_LEVEL - d, 0)` however it was reached. At `d = MAX_LEVEL` the bound
/// is exactly zero, which is the whole claim the rung rests on.
///
/// Widen the light nibble and this constant has to move with it or the rung starts culling lit
/// water -- and nothing in any frame would say so, because what it culls sits at the far end of
/// a medium that has already taken the image apart.
#[test]
fn dark_depth_matches_the_light_flood() {
    assert_eq!(
        shader_const_f32("WATER_DARK_DEPTH"),
        f32::from(voxelcraft::light::MAX_LEVEL),
        "WATER_DARK_DEPTH is the depth at which the sky flood reaches zero, which is \
         light::MAX_LEVEL and not a number anybody swept. A wider light nibble floods deeper, \
         and this rung would then cull water the sun still reaches."
    );
}

/// The shader's copy of the flag is the renderer's bit, for `water_far_flag_matches_the_shader`'s
/// reason: this one is read at run time as well, so a disagreement is a silent behaviour change
/// rather than a pipeline-key mismatch a sweep would catch.
#[test]
fn water_dark_flag_matches_the_shader() {
    assert_eq!(
        shader_const_u32("FLAG_WATER_DARK"),
        render::FLAG_WATER_DARK,
        "common.wgsl's FLAG_WATER_DARK and render::FLAG_WATER_DARK are one bit in two places"
    );
    assert_ne!(
        render::FLAG_WATER_DARK,
        render::FLAG_WATER_FAR,
        "the two rungs need separate bits, or `--no-water-dark` clears batch 48's cut too and \
         the control table's costs are both wrong"
    );
}

/// **The load-bearing test of the rung, and it is about an interval rather than a distance.**
///
/// A ray's depth is linear in `t`, so it is under `WATER_DARK_DEPTH` across the whole of
/// `[0, WATER_FAR_DIST]` exactly when it is under at both endpoints -- and which endpoint binds
/// flips with the sign of `up`. A rising ray is shallowest at the far end; a descending one is
/// shallowest at the eye. **Each endpoint alone compiles, renders, and is wrong in a direction
/// no still capture at the previously shipped vantages can show:**
///
/// - the far end alone admits every steeply-downward tile at any depth at all, which cost
///   **98,323 pixels at max delta 9 at `sea-horizon`** when the batch first built it that way;
/// - the eye alone admits a deep camera looking up, where the ray leaves the dark zone long
///   before absorption takes over.
///
/// Both are in `docs/errors.md`. This test is what stops either coming back as a tidy-up, since
/// from any one camera one of the two terms always looks redundant.
#[test]
fn the_dark_rung_tests_both_ends_of_the_interval() {
    let code = tile_select_body();
    let at = code
        .find("FLAG_WATER_DARK")
        .expect("the dark rung is gated in tile_select");
    let tail = &code[at..];
    let end = tail
        .find("reach = min(reach, WATER_DARK_DIST)")
        .expect("the dark rung narrows the reach");
    let cond = &tail[..end];
    assert!(
        cond.contains("WATER_DARK_DEPTH"),
        "the rung has to compare against the sky flood's reach. Condition was:\n{cond}"
    );
    assert!(
        cond.contains("submerged"),
        "the eye's own depth is one of the two endpoints, and without it a shallow camera \
         pitched down turns the rung on -- which makes the clamp a property of where the \
         player is *looking* rather than of where they are. Condition was:\n{cond}"
    );
    assert!(
        code[..at].contains("submerged - up * WATER_FAR_DIST"),
        "the far endpoint is the depth where batch 48's rung stops, so it is measured at \
         WATER_FAR_DIST and not at this rung's own distance. Body was:\n{code}"
    );
}

/// The rung narrows a reach it was handed, which is the only shape that keeps it nested.
///
/// `--no-water-far` has to clear this control as well, and it does so structurally: the whole
/// block sits inside batch 48's `if`. A version written as a sibling would need `frame.far`
/// here, having no `reach` to narrow -- so the spelling is the evidence of the nesting.
/// `harness::IMPLIES` carries the half of this claim that needs a render.
#[test]
fn the_dark_rung_narrows_the_reach_it_was_handed() {
    let code = tile_select_body();
    assert!(
        code.contains("reach = min(reach, WATER_DARK_DIST)"),
        "the dark rung should narrow the reach batch 48's rung set. Body was:\n{code}"
    );
    assert!(
        !code.contains("min(frame.far, WATER_DARK_DIST)"),
        "the dark rung is reading frame.far, so it has been lifted out of batch 48's block and \
         `--no-water-far` no longer clears it. Body was:\n{code}"
    );
}

/// Shorter than the reach it narrows, and not shorter than the point where shortening stops
/// doing anything.
///
/// **This floor is structural rather than fitted, which is the one thing this constant has that
/// `WATER_FAR_DIST` does not.** A reach culls whole chunks on `box_distance`, and an LOD 0 chunk
/// is `voxel::DIM` blocks on a side -- so at `deep-water` the reaches 64, 48, 32, 24 and 16
/// produce **one image with one sha256**. Under the chunk edge there is nothing left to remove,
/// so a smaller value is not a faster build; it is the same build with a smaller number in it,
/// and it would read as headroom.
#[test]
fn the_dark_reach_is_the_shorter_of_the_two_and_not_under_the_chunk() {
    let dark = shader_const_f32("WATER_DARK_DIST");
    let far = shader_const_f32("WATER_FAR_DIST");
    assert!(
        dark < far,
        "WATER_DARK_DIST is {dark} against WATER_FAR_DIST {far}; at or past it the rung is a \
         no-op no capture could tell from the control"
    );
    assert!(
        dark >= voxelcraft::voxel::DIM as f32,
        "WATER_DARK_DIST is {dark}, under an LOD 0 chunk's {} blocks. The cull is per chunk \
         AABB, so every value below the chunk edge renders the identical frame -- measured at \
         `deep-water`, where 64, 48, 32, 24 and 16 share one sha256. Going lower buys nothing \
         while looking like it bought something.",
        voxelcraft::voxel::DIM
    );
}

/// The rung's numbers were read off a vantage, so the vantage has to still be deep.
///
/// **This is `the_reach_floor_still_has_a_vantage_under_it` one batch on, and what it guards is
/// sharper.** That one polices a camera that could stop *seeing* the feature; this one polices a
/// camera that would stop *triggering* it. `deep-water` fires the rung because the eye itself is
/// under the sky flood's reach -- raise it nine blocks and the control goes bit-exact at all
/// eighteen vantages, which reads exactly like a clean sweep. The fixture would be measuring
/// nothing and saying so nowhere, which is the failure `docs/harness.md` named after batch 48.
#[test]
fn the_dark_rung_still_has_a_deep_vantage_under_it() {
    let v = voxelcraft::harness::vantage::find("deep-water")
        .expect("`deep-water` is the vantage the dark rung is measured at");
    let arg = |name: &str| {
        v.args
            .iter()
            .position(|a| *a == name)
            .and_then(|i| v.args.get(i + 1))
            .and_then(|s| s.parse::<f32>().ok())
    };
    let depth = arg("--cam-submerge").expect("`deep-water` is a submerged camera");
    let dark = shader_const_f32("WATER_DARK_DEPTH");
    assert!(
        depth >= dark,
        "`deep-water` sits {depth} blocks down against a WATER_DARK_DEPTH of {dark}. The rung \
         tests the eye's own depth, so a shallower camera cannot fire it at any pitch and the \
         control reads bit-exact everywhere -- which is also what a working feature reads like. \
         Args were {:?}",
        v.args
    );
    assert_eq!(
        arg("--cam-pitch"),
        Some(0.0),
        "`deep-water` looks along the horizontal. The band a reach can delete here is the \
         near-horizontal rays just above the horizon -- everything below is near sea floor \
         within a chunk or two -- so a pitched-down camera has nothing to cut and would read \
         zero at every rung. Args were {:?}",
        v.args
    );
}



