//! Water invariants: the basins flood, `--no-water` leaves the identical terrain dry,
//! coarse levels cover the same area of sea, and sky light dims with depth.

mod support;
use glam::IVec3;
use voxelcraft::block::*;
use voxelcraft::light::{self, LightData};
use voxelcraft::render;
use voxelcraft::voxel::*;
use voxelcraft::worldgen::{WorldGen, SEA_LEVEL};

fn gen_chunk(gen: &WorldGen, key: ChunkKey) -> Vec<BlockId> {
    let mut dense = vec![AIR; VOL];
    let mut heights = Box::new([0i32; 64 * 64]);
    gen.generate(key, &mut dense, &mut heights);
    dense
}

/// The chunk spanning internal Y 64..128 holds every water voxel there is, since water
/// only ever sits between a column's height and the sea surface.
const SEA_SLAB: i32 = 1;

#[test]
fn basins_flood_to_sea_level() {
    let gen = WorldGen::new(1337);
    let key = ChunkKey::new(0, IVec3::new(0, SEA_SLAB, 0));
    let dense = gen_chunk(&gen, key);
    let o = key.origin();
    let mut submerged = 0;
    let mut sand_floor = 0;
    for z in 0..64usize {
        for x in 0..64usize {
            let h = gen.height(o.x + x as i32, o.z + z as i32);
            if h >= SEA_LEVEL || h < o.y {
                continue;
            }
            submerged += 1;
            for y in 0..64usize {
                let wy = o.y + y as i32;
                let got = dense[dense_index(x, y, z)];
                if wy >= h && wy < SEA_LEVEL {
                    assert_eq!(
                        got, WATER,
                        "column {x},{z} (h={h}) should be water at y={wy}"
                    );
                } else if wy >= SEA_LEVEL {
                    assert_eq!(
                        got, AIR,
                        "column {x},{z} should be air above the sea at y={wy}"
                    );
                }
            }
            // A submerged floor is sand, not grass: it is a sea bed, and the tide line is
            // what the shore rule keys on. A cave mouth can open right under it, which is
            // the only other thing the top block is allowed to be.
            let ly = h - o.y;
            if ly >= 1 {
                let floor = dense[dense_index(x, ly as usize - 1, z)];
                assert!(
                    matches!(floor, SAND | AIR),
                    "sea floor at {x},{z} (h={h}) was {floor}"
                );
                sand_floor += usize::from(floor == SAND);
            }
        }
    }
    assert!(
        submerged > 200,
        "expected many submerged columns near spawn, got {submerged}"
    );
    assert!(
        sand_floor * 10 > submerged * 9,
        "only {sand_floor} of {submerged} sea floors are sand; caves cannot account for that"
    );
}

/// `--no-water` has to be a control and nothing more: the same heightfield, the same block
/// underneath, with air wherever the water was. Every measurement in batch 8 leans on that.
#[test]
fn no_water_leaves_the_identical_terrain_dry() {
    let wet = WorldGen::new(1337);
    let dry = WorldGen::with_sea_level(1337, 0);
    for cy in 0..3 {
        let key = ChunkKey::new(0, IVec3::new(0, cy, 0));
        let a = gen_chunk(&wet, key);
        let b = gen_chunk(&dry, key);
        for i in 0..VOL {
            let expect = if a[i] == WATER { AIR } else { a[i] };
            assert_eq!(
                b[i], expect,
                "chunk y={cy} voxel {i}: dry world diverges from wet"
            );
        }
    }
}

/// The same test the canopy proxies get: a coarse level has to cover the same fraction of
/// the world in sea as LOD 0 does, or a shoreline would walk in and out as it refines.
#[test]
fn coarse_water_area_matches_lod0() {
    let gen = WorldGen::new(1337);
    let mut dense = vec![0u16; VOL];
    let mut heights = Box::new([0i32; 64 * 64]);
    let mut cover = [0f64; 4];
    for lod in 0..=3u8 {
        let side = 8 >> lod;
        let slabs = 512 >> (6 + lod);
        let n = (side * 64) as usize;
        let mut top_block = vec![AIR; n * n];
        for cz in 0..side {
            for cx in 0..side {
                for cy in 0..slabs {
                    let key = ChunkKey::new(lod, IVec3::new(cx, cy, cz));
                    gen.generate(key, &mut dense, &mut heights);
                    for z in 0..64usize {
                        for x in 0..64usize {
                            if let Some(t) =
                                (0..64).rev().find(|&y| dense[dense_index(x, y, z)] != AIR)
                            {
                                let gx = cx as usize * 64 + x;
                                let gz = cz as usize * 64 + z;
                                top_block[gx + gz * n] = dense[dense_index(x, t, z)];
                            }
                        }
                    }
                }
            }
        }
        let wet = top_block.iter().filter(|&&b| b == WATER).count();
        cover[lod as usize] = wet as f64 / top_block.len() as f64;
        println!(
            "  lod {lod}: sea covers {:.2}% of columns",
            cover[lod as usize] * 100.0
        );
    }
    assert!(
        cover[0] > 0.05,
        "expected a real sea at lod 0, got {:.2}%",
        cover[0] * 100.0
    );
    for lod in 1..4 {
        let ratio = cover[lod] / cover[0];
        assert!(
            (0.85..1.2).contains(&ratio),
            "lod {lod} sea coverage is {ratio:.2}x lod0 ({:.2}% vs {:.2}%)",
            cover[lod] * 100.0,
            cover[0] * 100.0
        );
    }
}

const UNIFORM_BIT: u32 = 1 << 31;

fn probe(d: &LightData, x: usize, y: usize, z: usize) -> u16 {
    if let Some(u) = d.uniform {
        return u;
    }
    let cell = (x >> 2) | ((y >> 2) << 4) | ((z >> 2) << 8);
    let entry = d.cells[cell];
    if entry & UNIFORM_BIT != 0 {
        return entry as u16;
    }
    let vb = tree::cell_bit((x & 3) as u32, (y & 3) as u32, (z & 3) as u32) as usize;
    (d.bricks[entry as usize][vb >> 1] >> ((vb & 1) * 16)) as u16
}

/// Sky light loses one level per block of water and none per block of air, which is what
/// makes a sea floor darken with depth. Water does *not* stop the flood, so the light
/// still reaches sideways under a ledge.
#[test]
fn sky_light_dims_with_depth_through_water() {
    const FLOOR: usize = 20;
    const SURFACE: usize = 40;
    let mut dense = vec![AIR; VOL];
    for z in 0..64 {
        for x in 0..64 {
            for y in 0..FLOOR {
                dense[dense_index(x, y, z)] = STONE;
            }
            for y in FLOOR..SURFACE {
                dense[dense_index(x, y, z)] = WATER;
            }
        }
    }
    let data = light::compute(&dense, &vec![true; 64 * 64]);

    // Air above the surface keeps full sky.
    for y in SURFACE..64 {
        assert_eq!(
            light::sky_of(probe(&data, 32, y, 32)),
            15,
            "air at y={y} should be full sky"
        );
    }
    // One level per block of water, top-down.
    for d in 0..(SURFACE - FLOOR) {
        let y = SURFACE - 1 - d;
        let want = 15u8.saturating_sub(d as u8);
        assert_eq!(
            light::sky_of(probe(&data, 32, y, 32)),
            want,
            "water at depth {d} (y={y}) should be sky {want}"
        );
    }
    // The floor sits 20 blocks down, past where the pour has run out.
    assert_eq!(
        light::sky_of(probe(&data, 32, FLOOR, 32)),
        0,
        "20 blocks down should be dark"
    );
    // And a shallower bottom is still lit: 4 blocks of water keeps most of it.
    assert!(light::sky_of(probe(&data, 32, SURFACE - 5, 32)) >= 11);
}

// ------------------------------------------------------- batch 41: the shadow budget under water

/// Strip line comments, so a test cannot pass or fail on its own subject's documentation.
///
/// Joined with a space rather than a newline because every assertion below is a `contains`
/// over a single call, and a call that the formatter has wrapped is easier to match with the
/// newlines gone than with them kept.
fn code_only(src: &str) -> String {
    src.lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The budget is one number written in two files, and this is the only thing tying them.
///
/// `render::WATER_SHADOW_DIST` is what `make_spec` hands the pipeline; the `override` in
/// `common.wgsl` is what a reader of the shader sees, and what any pipeline built without the
/// constants list would get. They have to agree or the file lies about the shipping build --
/// the same arrangement `water_body_matches_the_atlas_layer` makes for the medium's colour.
#[test]
fn the_water_shadow_budget_agrees_with_the_shader() {
    let src = render::shader_source();
    let key = "override SPEC_WATER_SHADOW_DIST: f32 = ";
    let at = src
        .find(key)
        .expect("SPEC_WATER_SHADOW_DIST is not in the module any more");
    let open = at + key.len();
    let close = open + src[open..].find(';').expect("unterminated override");
    let wgsl: f32 = src[open..close]
        .trim()
        .parse()
        .expect("SPEC_WATER_SHADOW_DIST is not a number");
    assert_eq!(
        wgsl,
        render::WATER_SHADOW_DIST,
        "the shader's default budget and the one `make_spec` passes disagree"
    );
    // The `const` this replaced was read by name from `resolve.wgsl` for thirty-three
    // batches. A stale copy left behind would compile and would quietly be the number the
    // refraction leg used.
    assert!(
        !src.contains("const WATER_SHADOW_DIST"),
        "a second copy of the budget is still in the module"
    );
}

/// Truncating the march moves the handoff; it does not delete the shadow.
///
/// **This is the whole reason 48 -> 16 costs a max channel delta of 1 rather than a missing
/// shadow**, and it is a property of batch 37's code rather than of batch 41's: `shade_hit`
/// marches `shadow_ray` to `shadow_dist` and then hands `terrain_shade_beyond` *the same
/// distance*, so the two partition `[0, d)` and `[d, reach)` with no gap and nothing counted
/// twice -- see the algebra at `terrain_shade_beyond`. Pass a different number to either and
/// the partition opens a gap or double-counts, and the measurement that chose 16 stops
/// holding, with nothing in the frame to say so at the vantages this fixture has.
#[test]
fn the_envelope_resumes_where_the_water_shadow_march_stops() {
    let src = render::shader_source();
    for arm in ["shade_hit_legacy", "shade_hit_compact"] {
    let code = support::wgsl_function(&src, arm);
    assert!(
        code.contains("shadow_ray(hit + n * (0.02 * vs), frame.sun_dir, shadow_dist)"),
        "shade_hit's own shadow ray no longer marches to its `shadow_dist` parameter"
    );
    assert!(
        code.contains("terrain_shade_beyond(hit, shadow_dist)"),
        "the envelope is no longer handed the same distance the march just used, so the two \
         occluders have stopped partitioning the ray -- batch 41's truncation is only cheap \
         to look at while they do"
    );
    }
}

/// The refracted ray is the budget's only reader, which is what `IMPLIES` claims from a render.
///
/// The two readings are worth having together for the reason `lessons.md` gives: the
/// composition check says a build tracing no refraction cannot see the control, and this says
/// why -- there is one call site. A second one would make that `Nothing` a coincidence of the
/// vantages rather than a fact about the code.
#[test]
fn the_water_shadow_budget_has_one_reader() {
    let src = render::shader_source();
    let code = code_only(&src);
    let uses = code.matches("SPEC_WATER_SHADOW_DIST").count();
    assert_eq!(
        uses, 2,
        "SPEC_WATER_SHADOW_DIST should appear exactly twice in the module -- its `override` \
         and the one `shade_hit` call under the refraction -- and it appears {uses} times"
    );
    // Batch 81 (P12) moved the traces *with* the legs into functions, so the reader is
    // where the refraction is. Same invariant, re-pointed: exactly one `shade_hit` in the
    // module passes this budget, and the reflect leg is not it.
    let at = src.find("fn water_refract_leg(").expect("water_refract_leg exists");
    let body = &src[at..];
    let end = body.find("\nfn ").expect("another function follows water_refract_leg");
    let code = code_only(&body[..end]);
    assert!(
        code.contains("SPEC_WATER_SHADOW_DIST"),
        "the one reader is no longer in the refracted leg"
    );
    let at = src.find("fn water_reflect_leg(").expect("water_reflect_leg exists");
    let body = &src[at..];
    let end = body.find("\nfn ").expect("another function follows water_reflect_leg");
    let code = code_only(&body[..end]);
    // The reflected leg shades through air and takes the frame's own 220. If it ever
    // took this one, `--no-water-shadow-cut` would silently become a reflection control.
    assert!(
        code.contains("mirror_t, t + h3.t,"),
        "the reflected leg's shade_hit call has moved; check which budget it now passes"
    );
    assert!(
        !code.contains("SPEC_WATER_SHADOW_DIST"),
        "the budget grew a second reader, in the reflect leg"
    );
}



