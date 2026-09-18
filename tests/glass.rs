
//! Batch 38b's glass: the two halves that no capture can check, and the one that no
//! `--no-glass` control can revert.
//!
//! The feature splits cleanly into a *shading* half and a *world* half, and they fail in
//! opposite ways. The shading half is loud -- a pane that stops transmitting is a blue box in
//! the frame, which the `glass` vantage catches in one image. The world half is silent: glass
//! is `opaque: false`, so a mistake there changes how a room is lit and nothing in the shader
//! is wrong, no control moves, and the only symptom is a slightly different number somewhere
//! in a brick. So most of this file is about the quiet half.
//!
//! The rest are cross-file mirrors, and they are here for the reason
//! `faces_per_block_matches_the_shader` is: the shader cannot see this crate, so `GLASS_ID`
//! and `ATTR_HAS_GLASS` are hand-copied literals, and a wrong one does not crash -- it
//! silently shades a *different block* as glass.

use glam::IVec3;
use voxelcraft::block;
use voxelcraft::light;
use voxelcraft::render;
use voxelcraft::voxel::{ChunkKey, World, VOL};
use voxelcraft::voxel::tree::dense_index;

/// A `const NAME: u32 = Nu;` read off the shader rather than retyped here. The same helper
/// `tests/cutout.rs` uses, and for the same reason.
fn shader_const(name: &str) -> u32 {
    let src = render::shader_source();
    let pat = format!("const {name}: u32 = ");
    let at = src
        .find(&pat)
        .unwrap_or_else(|| panic!("{name} is not declared in the shader"));
    let rest = &src[at + pat.len()..];
    let end = rest.find('u').expect("a u32 literal");
    rest[..end].parse().expect("a u32 literal")
}

/// `common.wgsl` carries `block::GLASS` as a literal, exactly as it carries `WATER_ID`.
///
/// The `const _: () = assert!(block::GLASS == 10)` beside `render::FLAG_GLASS` pins the Rust
/// half to 10; this pins the *shader* half to the same number, which is the half that assert
/// structurally cannot reach. Getting it wrong is not a crash: id 9 is cobblestone, and a
/// build that shaded cobblestone as glass would look like a bug in the marcher.
#[test]
fn glass_id_matches_the_shader() {
    assert_eq!(
        shader_const("GLASS_ID"),
        u32::from(block::GLASS),
        "common.wgsl's GLASS_ID and block::GLASS are one number in two files"
    );
}

/// The fourth `attr_flags` bit, and the three it must not collide with.
///
/// `render::upload` writes these as bare literals next to each other, so the failure mode is a
/// copy-paste: an `attr_flags |= 8` under `has_glass` would make every chunk holding a window
/// claim to hold a carved canopy as well, which costs a leaf-block lookup per solid cell and
/// changes no pixel. Exactly the kind of thing nothing else here would notice.
#[test]
fn attr_glass_bit_is_its_own() {
    let uniform = shader_const("ATTR_UNIFORM");
    let water = shader_const("ATTR_HAS_WATER");
    let foliage = shader_const("ATTR_HAS_FOLIAGE");
    let cutout = shader_const("ATTR_HAS_CUTOUT");
    let glass = shader_const("ATTR_HAS_GLASS");
    assert_eq!(glass, 16, "the next free bit");
    for (name, other) in [
        ("ATTR_UNIFORM", uniform),
        ("ATTR_HAS_WATER", water),
        ("ATTR_HAS_FOLIAGE", foliage),
        ("ATTR_HAS_CUTOUT", cutout),
    ] {
        assert_eq!(glass & other, 0, "ATTR_HAS_GLASS overlaps {name}");
        assert!(other.is_power_of_two(), "{name} is one bit");
    }
    assert!(glass.is_power_of_two(), "ATTR_HAS_GLASS is one bit");
}

/// **Every secondary ray skips glass and the primary ray does not**, which is the whole of
/// batch 38b's depth-complexity story and is stated nowhere but in four argument lists.
///
/// The research asked for a hard cap at two interfaces so a double-pane greenhouse resolves.
/// There is no counter anywhere that enforces one: the cap *is* this asymmetry, because the
/// primary hit is interface one and every pane behind it is air to the ray that goes looking.
/// Flip the `march.wgsl` argument and the camera sees through every window in the world; flip
/// either of the others and a pane becomes a wall to its own transmitted ray, which is a black
/// rectangle where the view should be.
#[test]
fn only_the_primary_ray_stops_at_a_pane() {
    let src = render::shader_source();
    // The declaration says which argument this is, so the test cannot drift if a later batch
    // adds a seventh bool: `skip_glass` precedes the new sunlight-policy parameter.
    assert!(
        src.contains("skip_foliage: bool, skip_glass: bool, sun_ray: bool)"),
        "skip_glass must precede the independent sun-ray policy"
    );
    // Balanced, not the first `)`: `trace_world`'s call passes `min(t_exit, max_dist)` and a
    // naive scan stops inside it -- which is how the first draft of this test read `max_dist`
    // as the last argument and failed on correct code.
    let calls: Vec<(&str, &str)> = src
        .match_indices("march_chunk(")
        .map(|(at, _)| {
            let rest = &src[at..];
            let open = rest.find('(').expect("an argument list");
            let mut depth = 0i32;
            let mut end = 0;
            for (i, ch) in rest[open..].char_indices() {
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = open + i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            // The call text alone cannot always say whose ray it marches: batch 91's bent
            // segment (`let h2 = march_chunk(...)`) is camera-class by *what precedes it*,
            // and the first repair of this census looked for `h2` inside the arguments,
            // where the variable name never appears -- which is why the same site turned up
            // as the fork's failure line. Keep the bounded context before the match too.
            let before = &src[at.saturating_sub(600)..at];
            (&rest[..end], before)
        })
        // The declaration itself matches too, and it is the one with a typed parameter list.
        .filter(|(c, _)| !c.contains(": bool"))
        .collect();
    // Batch 95d: the census's *count* failed in two trees at once -- batch 94 pinned 4, the
    // P12 sibling fork already carried a fifth site, and both statements were beside the
    // point. The property is two classes, not an integer: sites at the camera's service
    // (`frame.cam_pos`, and batch 91 A2c's `h2`, segment two of the same eye ray) must say
    // `false`; every helper march must say `true`. Whatever the count is this week, that
    // sentence is the claim.
    assert!(
        !calls.is_empty(),
        "the scanner found no `march_chunk` call sites at all -- that is the scan, not the rule"
    );
    let mut primary = 0;
    let mut secondary = 0;
    for (c, before) in &calls {
        let last = c.rsplit(',').nth(1).expect("the glass-policy argument").trim();
        // Two classes, and class is decided by the ray, not the module: the straight first
        // march passes the camera position, A2c's refracted continuation is exactly the
        // text `let h2 = march_chunk(`, and both must say `false` -- a pane stops the eye
        // in either segment -- while every helper ray must pass through one.
        if c.contains("frame.cam_pos") || before.trim_end().ends_with("let h2 =") {
            assert_eq!(last, "false", "a camera-ray march must stop at a pane: {c}");
            primary += 1;
        } else {
            assert_eq!(last, "true", "a secondary ray must pass through a pane: {c}");
            secondary += 1;
        }
    }
    assert!(
        primary >= 1 && secondary >= 2,
        "expected the camera's own ray (>= 1 site) and its helpers (>= 2: shadow and          trace_world, or more in a fork that marches its own secondaries);          got {primary}/{secondary}"
    );
}

/// **The transmitted ray is the one `trace_world` caller that must not skip water**, and it
/// shipped skipping it.
///
/// `trace_world` hard-coded `skip_water = true` from batch 8 until this was found, and was right
/// to for every caller water had: water's own refraction starts *inside* the medium, its
/// reflection wants the world above the surface, and a shadow ray must not be stopped by a sea.
/// A ray that has come through a pane is in none of those situations. With the literal in place
/// **the sea was invisible through glass** -- a window onto a bay came back with the unlit
/// seabed, which is what the user saw and no vantage could.
///
/// It stayed invisible to the fixture because **every vantage holding water holds no glass and
/// the one holding glass holds no water**, which is a gap a test can close and a capture cannot.
#[test]
fn only_the_transmitted_ray_looks_for_water() {
    let src = render::shader_source();
    assert!(
        src.contains("max_dist: f32, skip_water: bool) -> WorldHit"),
        "skip_water is trace_world's last parameter, not a literal inside it"
    );
    let calls: Vec<&str> = src
        .match_indices("trace_world(")
        .map(|(at, _)| {
            let rest = &src[at..];
            let end = rest.find(" -> WorldHit").unwrap_or(usize::MAX);
            let stop = rest.find(");").unwrap_or(rest.len());
            if end < stop { "DECL" } else { &rest[..stop] }
        })
        .filter(|c| *c != "DECL")
        .collect();
    assert_eq!(calls.len(), 5, "five call sites: shoal, water refract/reflect, glass transmit/reflect");
    let mut skipping = 0;
    let mut looking = 0;
    for c in &calls {
        let last = c.rsplit(',').next().expect("the water-policy argument").trim();
        if c.contains("GLASS_TRANSMIT_DIST") {
            assert_eq!(last, "false", "a ray through a pane has to see the sea");
            looking += 1;
        } else {
            assert_eq!(last, "true", "water's own rays keep batch 8's policy: {c}");
            skipping += 1;
        }
    }
    assert_eq!((looking, skipping), (2, 3));
}

/// Stopping at water is only half the fix: it has to be *shaded* as water.
///
/// Handing a water hit to `shade_hit` would draw the sea through a window as a flat blue cube
/// face, which is worse than the bug it replaced because it looks deliberate. This is
/// `../CLAUDE.md`'s "a reflection shaded by a cheaper model reads as a different material",
/// reached from a third direction.
///
/// **Batch 67 changed how that is done and not whether it is done, which is why this test
/// changed shape rather than going away.** Until then the arm called `shade_water`, inlining a
/// second complete copy of it -- two `trace_world` marches and two `shade_hit` calls -- inside a
/// function that already had a march live. That was the last thing holding `resolve` at 96
/// registers, worth 0.103 ms at `cave` and 0.399 at `coastline`, neither of which has a pane in
/// it. It is now analytic: the water body colour, the water Fresnel, and a sky reflection.
///
/// **So what is asserted is the claim rather than the call.** The sea through a pane must still
/// be water -- `WATER_BODY` and `schlick`, water's own F0, not `schlick_glass` and not
/// `shade_hit`. Batch 54 is the precedent for trusting the analytic colour: it measured a traced
/// version against `underwater_body()` at a max channel delta of 3 to 7 and deleted the ray.
#[test]
fn water_seen_through_a_pane_is_shaded_as_water() {
    let src = render::shader_source();
    let at = src
        .find("if id2 == WATER_ID {")
        .expect("shade_glass still dispatches on a water hit");
    let arm = &src[at..at + src[at..].find("} else {").expect("the arm has an else")];
    assert!(
        arm.contains("WATER_BODY"),
        "the water arm must carry water's own body colour, or the sea through a pane is not          water. Arm was:
{arm}"
    );
    assert!(
        arm.contains("schlick("),
        "the water arm must use water's Fresnel (`schlick`, at WATER_F0) and not glass's.          Arm was:
{arm}"
    );
    assert!(
        !arm.contains("shade_hit("),
        "a water hit through a pane must never go to shade_hit -- that is the flat blue cube          face this test exists to prevent. Arm was:
{arm}"
    );
}

/// `shade_glass` is reached through the override and not merely guarded by it.
///
/// Batch 38 measured what the other spelling costs: written so the constant reaches the cheap
/// test instead of the expensive one, a folded-away flag left **5,120 bytes** of machine code
/// in `resolve` and moved 201 pixels through a control that had to be bit-exact. `SPEC_GLASS`
/// is spelled before the id compare for exactly that reason, and this is what holds it there.
#[test]
fn the_glass_arm_is_gated_on_the_override_first() {
    let src = render::shader_source();
    assert!(
        src.contains("} else if SPEC_GLASS && id == GLASS_ID {"),
        "the override has to be the first term, or the arm stays in the module"
    );
    assert!(
        src.contains("if SPEC_GLASS {"),
        "march_chunk's glass_aware sits inside the override's own block"
    );
}

/// Glass answers the three questions the way water does with the middle one alone changed.
///
/// **The point is that the three are independent**, and this is the fourth block in the tree
/// to prove it. A later batch reaching for `opaque: false` and taking `solid` with it would
/// let the player walk through a window, and nothing in the renderer would complain.
#[test]
fn glass_transmits_light_and_stops_everything_else() {
    let g = block::def(block::GLASS);
    assert!(!g.opaque, "the flood runs through a pane -- batch 38b's world half");
    assert!(g.solid, "a window still stops the player");
    assert!(block::fills_voxel(block::GLASS), "and still stops a ray");
    assert!(!g.foliage && !g.cutout, "a pane is a whole cube, not a quad or a mask");
    assert!(block::is_glass(block::GLASS));
    assert!(!block::is_glass(block::WATER), "water is the other transmissive block");
}

/// A closed glass roof lets the sky in and a closed plank roof does not.
///
/// **This is the batch's world half, and it is the one claim `--no-glass` cannot make.** That
/// control is a pipeline override; `opaque` is read by `light.rs` while a chunk is being built,
/// long before a pipeline exists. So the only way to check it is here, against the flood
/// itself -- and the paired plank row is what makes it a measurement rather than a restatement
/// of the flag: the same room, the same fill, one block id different, and the answers are
/// fifteen and zero.
///
/// It is also why `scene::build_glass_structure` roofs its glasshouse *solid* where
/// `build_demo_structure` leaves a hole in the hut's planks. The hole is there because planks
/// are opaque. Glass needs none, and the capture shows it.
#[test]
fn a_glass_roof_lets_the_sky_through_and_planks_do_not() {
    // One room: a floor at y = 8, walls of the roofing block, a roof at y = 12, and the
    // interior open air. Everything above the roof is open sky.
    let room = |roof: block::BlockId| {
        let mut dense = vec![block::AIR; VOL];
        for z in 20..30usize {
            for x in 20..30usize {
                dense[dense_index(x, 8, z)] = block::STONE;
                dense[dense_index(x, 12, z)] = roof;
                for y in 9..12usize {
                    let edge = x == 20 || x == 29 || z == 20 || z == 29;
                    if edge {
                        dense[dense_index(x, y, z)] = roof;
                    }
                }
            }
        }
        let open = vec![true; 64 * 64];
        let data = light::compute(&dense, &open);
        // The middle of the room, one block above its floor.
        read_packed(&data, 25, 9, 25)
    };
    let through_glass = light::sky_of(room(block::GLASS));
    let through_planks = light::sky_of(room(block::PLANKS));
    assert_eq!(
        through_glass,
        light::MAX_LEVEL,
        "a glass roof is optically clear to the vertical pour -- unlike water, which costs a \
         level a block, a pane costs nothing"
    );
    assert_eq!(
        through_planks, 0,
        "the paired negative: the same room roofed in planks is dark, which is what says this \
         test is reading the roof and not the walls"
    );
}

/// Decode one voxel's packed light cell out of a [`light::LightData`], by coordinate.
///
/// A transliteration of `pack_bricks` run backwards, and it takes `(x, y, z)` rather than a
/// dense index on purpose: `dense_index` is `x | z << 6 | y << 12`, which is **not** the
/// obvious order, and the first draft of this file inverted it as `x + y*64 + z*4096` and read
/// a voxel in open sky. The paired plank assertion is what caught it -- a room that reported
/// full daylight under a *plank* roof is not a room anybody was reading.
fn read_packed(data: &light::LightData, x: usize, y: usize, z: usize) -> u16 {
    if let Some(u) = data.uniform {
        return u;
    }
    let cell = (x >> 2) | ((y >> 2) << 4) | ((z >> 2) << 8);
    let entry = data.cells[cell];
    if entry & (1 << 31) != 0 {
        return (entry & 0xFFFF) as u16;
    }
    let brick = &data.bricks[entry as usize];
    let vb = (x & 3) | ((y & 3) << 2) | ((z & 3) << 4);
    ((brick[vb >> 1] >> ((vb & 1) * 16)) & 0xFFFF) as u16
}

/// An edit that places glass tells the renderer so.
///
/// **The gate the whole feature hangs on, and its failure is silent.** `ATTR_HAS_GLASS` is
/// what `march_chunk` tests before it will look a block id up; a chunk that forgets to set it
/// marches its glass as an ordinary opaque cube, with no error, no warning and a frame that
/// looks like the `--no-glass` control nobody asked for. No generator places glass, so an edit
/// is in practice the only way this bit is ever set -- which makes `set_block` the one code
/// path that has to get it right.
#[test]
fn an_edit_marks_its_chunk_as_holding_glass() {
    let mut world = World::new();
    let key = ChunkKey::new(0, IVec3::new(0, 0, 0));
    world.insert_local(key, voxelcraft::voxel::build_local(&vec![block::AIR; VOL]));
    assert!(
        !world.chunks[&key].has_glass,
        "an empty chunk holds no glass"
    );
    assert!(world.set_block(IVec3::new(5, 5, 5), block::GLASS));
    assert!(
        world.chunks[&key].has_glass,
        "set_block has to raise the bit the marcher reads"
    );

    // The neighbouring claim, and the reason this is a test rather than a line of prose: a
    // *stone* edit must not raise it. `has_glass` is conservative upward only -- it is never
    // cleared -- so a bit set by the wrong block id would stay wrong for the life of the run.
    let mut other = World::new();
    let k2 = ChunkKey::new(0, IVec3::new(1, 0, 0));
    other.insert_local(k2, voxelcraft::voxel::build_local(&vec![block::AIR; VOL]));
    assert!(other.set_block(IVec3::new(64 + 5, 5, 5), block::STONE));
    assert!(!other.chunks[&k2].has_glass, "stone is not glass");
}

/// A chunk built from a dense array with glass in it carries the flag too.
///
/// The other half of the sentence above: `set_block` is one writer and `build_local` is the
/// other, and a world that loads from a journal takes the second path. Batch 33's replay puts
/// every recorded edit into the dense array *before* the tree is built, so a glasshouse that
/// comes back from disk is built rather than edited -- and it would be glass with the flag
/// clear if only `set_block` set it.
#[test]
fn a_chunk_built_with_glass_in_it_carries_the_flag() {
    let mut dense = vec![block::AIR; VOL];
    dense[dense_index(3, 3, 3)] = block::GLASS;
    let t = voxelcraft::voxel::build_local(&dense);
    assert!(t.has_glass, "build_local has to see it as well as set_block");

    let mut plain = vec![block::AIR; VOL];
    plain[dense_index(3, 3, 3)] = block::COBBLE;
    assert!(
        !voxelcraft::voxel::build_local(&plain).has_glass,
        "and only for glass"
    );
}


