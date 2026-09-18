use voxelcraft::render;

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
