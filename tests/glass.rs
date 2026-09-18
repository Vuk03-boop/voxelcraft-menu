use glam::IVec3;
use voxelcraft::block;
use voxelcraft::light;
use voxelcraft::render;
use voxelcraft::voxel::{ChunkKey, World, VOL};
use voxelcraft::voxel::tree::dense_index;

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

#[test]
fn glass_id_matches_the_shader() {
    assert_eq!(
        shader_const("GLASS_ID"),
        u32::from(block::GLASS),
        "common.wgsl's GLASS_ID and block::GLASS are one number in two files"
    );
}

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

#[test]
fn only_the_primary_ray_stops_at_a_pane() {
    let src = render::shader_source();

    assert!(
        src.contains("skip_foliage: bool, skip_glass: bool, sun_ray: bool)"),
        "skip_glass must precede the independent sun-ray policy"
    );

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

            let before = &src[at.saturating_sub(600)..at];
            (&rest[..end], before)
        })

        .filter(|(c, _)| !c.contains(": bool"))
        .collect();

    assert!(
        !calls.is_empty(),
        "the scanner found no `march_chunk` call sites at all -- that is the scan, not the rule"
    );
    let mut primary = 0;
    let mut secondary = 0;
    for (c, before) in &calls {
        let last = c.rsplit(',').nth(1).expect("the glass-policy argument").trim();

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

#[test]
fn water_seen_through_a_pane_is_shaded_as_water() {
    let src = render::shader_source();
    let at = src
        .find("if id2 == WATER_ID {")
        .expect("shade_glass still dispatches on a water hit");
    let arm = &src[at..at + src[at..].find("} else {").expect("the arm has an else")];
    assert!(
        arm.contains("shade_water("),
        "the water arm must route the hit into water's shared shading entry -- any local          rewrite of what 'water' means here is the divergence this test guards. Arm was:
{arm}"
    );
    assert!(
        !arm.contains("shade_hit("),
        "a water hit through a pane must never go to shade_hit -- that is the flat blue cube          face this test exists to prevent. Arm was:
{arm}"
    );

    // Pin the mechanism, not its narration: an earlier version of this test matched WATER_BODY
    // and schlick( inside a *comment* above the call, and read green while the comments were
    // the only place those names survived. The shared entry, the Fresnel compose, and water's
    // own body colour are the load-bearing words; assert each where it actually lives.
    let entry = src
        .find("fn shade_water(")
        .expect("shade_water is the shared water entry the arm must use");
    let entry_body = &src[entry..src.len().min(entry + src[entry..].find("\nfn ").unwrap_or(src.len() - entry))];
    assert!(
        entry_body.contains("water_compose("),
        "shade_water must hand off to water_compose -- the Fresnel compose that makes 'water'          mean the same thing from every ray. Entry was:
{entry_body}"
    );
    let compose = src
        .find("fn water_compose(")
        .expect("water_compose is water's own Fresnel compose");
    let compose_body =
        &src[compose..src.len().min(compose + src[compose..].find("\nfn ").unwrap_or(src.len() - compose))];
    assert!(
        compose_body.contains("schlick("),
        "water_compose must use water's own Fresnel (`schlick`, at WATER_F0) and not glass's.          Compose was:
{compose_body}"
    );
    assert!(
        src.contains("const WATER_BODY:"),
        "water's own body colour is part of the shared path the arm now routes through"
    );
}

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

#[test]
fn a_glass_roof_lets_the_sky_through_and_planks_do_not() {

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

    let mut other = World::new();
    let k2 = ChunkKey::new(0, IVec3::new(1, 0, 0));
    other.insert_local(k2, voxelcraft::voxel::build_local(&vec![block::AIR; VOL]));
    assert!(other.set_block(IVec3::new(64 + 5, 5, 5), block::STONE));
    assert!(!other.chunks[&k2].has_glass, "stone is not glass");
}

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
