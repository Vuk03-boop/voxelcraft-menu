#![allow(clippy::int_plus_one)]
#![allow(clippy::manual_is_multiple_of)]
#![allow(clippy::manual_range_contains)]

use glam::IVec3;
use voxelcraft::block::*;
use voxelcraft::voxel::*;

fn dense_from(f: impl Fn(u32, u32, u32) -> BlockId) -> Vec<BlockId> {
    let mut d = vec![AIR; VOL];
    for y in 0..64u32 {
        for z in 0..64u32 {
            for x in 0..64u32 {
                d[dense_index(x as usize, y as usize, z as usize)] = f(x, y, z);
            }
        }
    }
    d
}

fn hash3(x: u32, y: u32, z: u32, seed: u32) -> u32 {
    let mut h =
        x.wrapping_mul(0x8da6b343) ^ y.wrapping_mul(0xd8163841) ^ z.wrapping_mul(0xcb1ab31f) ^ seed;
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1e995);
    h ^ (h >> 15)
}

fn terrain(x: u32, y: u32, z: u32, seed: u32) -> BlockId {
    let fx = (x as f32 + seed as f32 * 13.0) * 0.11;
    let fz = (z as f32 + seed as f32 * 7.0) * 0.09;
    let h = (30.0 + 10.0 * fx.sin() * fz.cos() + 4.0 * (fx * 3.1).cos()) as u32;
    if y >= h {
        return AIR;
    }
    if y > 8 && y + 6 < h && hash3(x / 3, y / 3, z / 3, seed) % 11 == 0 {
        return AIR;
    }
    if y + 1 == h {
        GRASS
    } else if y + 4 >= h {
        DIRT
    } else if hash3(x, y, z, seed) % 23 == 0 {
        GRAVEL
    } else if hash3(x, y, z, seed + 1) % 41 == 0 {
        GLOWSTONE
    } else {
        STONE
    }
}

fn key(x: i32, y: i32, z: i32) -> ChunkKey {
    ChunkKey::new(0, IVec3::new(x, y, z))
}

fn assert_world_matches(w: &World, k: ChunkKey, dense: &[BlockId]) {
    let o = k.origin();
    for y in 0..64u32 {
        for z in 0..64u32 {
            for x in 0..64u32 {
                let want = dense[dense_index(x as usize, y as usize, z as usize)];
                let got = w.get_block(o + IVec3::new(x as i32, y as i32, z as i32));
                assert_eq!(got, want, "mismatch at {x} {y} {z}");
            }
        }
    }
}

fn assert_pools_empty(w: &World) {
    assert_eq!(w.leaves.live_elems, 0, "leaf pool leaked");
    assert_eq!(w.inners.live_elems, 0, "inner pool leaked");
    assert_eq!(w.leaves.referenced_elems, 0);
    assert_eq!(w.inners.referenced_elems, 0);
    assert_eq!(w.bricks.used_words, 0, "brick pool leaked");
}

#[test]
fn local_roundtrip() {
    let dense = dense_from(|x, y, z| terrain(x, y, z, 1));
    let t = build_local(&dense);
    assert!(t.solid_count > 0);
    for y in 0..64u32 {
        for z in 0..64u32 {
            for x in 0..64u32 {
                assert_eq!(
                    local_block(&t, x, y, z),
                    dense[dense_index(x as usize, y as usize, z as usize)]
                );
            }
        }
    }
}

#[test]
fn world_roundtrip_and_aabb() {
    let dense = dense_from(|x, y, z| terrain(x, y, z, 2));
    let mut w = World::new();
    w.insert_local(key(3, 1, -2), build_local(&dense));
    assert_world_matches(&w, key(3, 1, -2), &dense);
    let rec = &w.chunks[&key(3, 1, -2)];
    assert_eq!(rec.aabb_min.y, 0);
    assert!(rec.aabb_max.y <= 45);
    assert_eq!(rec.aabb_min.x, 0);
    assert_eq!(rec.aabb_max.x, 64);
    assert_eq!(w.get_block(IVec3::new(0, -1, 0)), AIR);
    assert_eq!(w.get_block(IVec3::new(1000, 10, 1000)), AIR);
    w.remove_chunk(key(3, 1, -2));
    assert_pools_empty(&w);
}

#[test]
fn identical_chunks_share_runs() {
    let dense = dense_from(|x, y, z| terrain(x, y, z, 3));
    let mut w = World::new();
    w.insert_local(key(0, 0, 0), build_local(&dense));
    let live_leaves = w.leaves.live_elems;
    let live_inners = w.inners.live_elems;
    assert!(live_leaves > 0 && live_inners > 0);
    w.insert_local(key(5, 0, 5), build_local(&dense));
    assert_eq!(
        w.leaves.live_elems, live_leaves,
        "second identical chunk must add no leaf data"
    );
    assert_eq!(
        w.inners.live_elems, live_inners,
        "second identical chunk must add no inner data"
    );
    assert!(w.inners.dedup_ratio() >= 2.0 - 1e-9);
    assert_world_matches(&w, key(5, 0, 5), &dense);
    w.remove_chunk(key(0, 0, 0));
    assert_eq!(w.leaves.live_elems, live_leaves);
    assert_world_matches(&w, key(5, 0, 5), &dense);
    w.remove_chunk(key(5, 0, 5));
    assert_pools_empty(&w);
}

#[test]
fn uniform_chunk_costs_nothing() {
    let dense = dense_from(|_, _, _| STONE);
    let t = build_local(&dense);
    assert!(t.is_full_solid());
    assert_eq!(t.uniform, Some(STONE));
    let mut w = World::new();
    w.insert_local(key(0, 0, 0), t);
    let rec = &w.chunks[&key(0, 0, 0)];
    assert!(rec.root.is_full());
    assert_eq!(rec.attr, AttrRef::Uniform(STONE));
    assert_eq!(w.leaves.live_elems, 0);
    assert_eq!(w.inners.live_elems, 0);
    assert_eq!(w.bricks.used_words, 0);
    assert_eq!(w.get_block(IVec3::new(17, 33, 60)), STONE);
    assert_world_matches(&w, key(0, 0, 0), &dense);
}

#[test]
fn empty_chunk() {
    let dense = dense_from(|_, _, _| AIR);
    let t = build_local(&dense);
    assert!(t.is_empty());
    let mut w = World::new();
    w.insert_local(key(0, 0, 0), t);
    assert_eq!(w.get_block(IVec3::new(1, 2, 3)), AIR);
    assert!(w.set_block(IVec3::new(1, 2, 3), PLANKS));
    assert_eq!(w.get_block(IVec3::new(1, 2, 3)), PLANKS);
    assert_eq!(w.chunks[&key(0, 0, 0)].solid_count, 1);
    assert!(w.set_block(IVec3::new(1, 2, 3), AIR));
    assert_eq!(w.get_block(IVec3::new(1, 2, 3)), AIR);
    assert!(w.chunks[&key(0, 0, 0)].root.is_empty());
    assert_pools_empty(&w);
}

#[test]
fn edit_full_chunk() {
    let dense = dense_from(|_, _, _| STONE);
    let mut w = World::new();
    w.insert_local(key(0, 0, 0), build_local(&dense));
    let p = IVec3::new(20, 21, 22);
    assert!(w.set_block(p, AIR));
    assert_eq!(w.get_block(p), AIR);
    assert_eq!(w.get_block(p + IVec3::X), STONE);
    assert_eq!(w.get_block(p - IVec3::Y), STONE);
    assert!(!w.chunks[&key(0, 0, 0)].root.is_full());
    assert_eq!(w.chunks[&key(0, 0, 0)].solid_count, 64 * 64 * 64 - 1);

    assert!(w.set_block(p, STONE));
    assert!(w.chunks[&key(0, 0, 0)].root.is_full());
    assert_eq!(w.chunks[&key(0, 0, 0)].attr, AttrRef::Uniform(STONE));
    assert_eq!(w.leaves.live_elems, 0);
    assert_eq!(w.inners.live_elems, 0);

    assert!(w.set_block(p, GLOWSTONE));
    assert_eq!(w.get_block(p), GLOWSTONE);
    assert_eq!(w.get_block(p + IVec3::Z), STONE);
    assert!(matches!(
        w.chunks[&key(0, 0, 0)].attr,
        AttrRef::Table { .. }
    ));
    w.remove_chunk(key(0, 0, 0));
    assert_pools_empty(&w);
}

#[test]
fn random_edits_match_dense_mirror() {
    let mut dense = dense_from(|x, y, z| terrain(x, y, z, 4));
    let mut w = World::new();
    let k = key(-1, 0, 2);
    w.insert_local(k, build_local(&dense));
    let o = k.origin();
    let ids = [AIR, STONE, DIRT, GRASS, SAND, PLANKS, GLOWSTONE, AIR, AIR];
    let mut seed = 12345u32;
    for i in 0..2000u32 {
        seed = hash3(seed, i, 7, 99);
        let x = seed % 64;
        let y = (seed >> 6) % 64;
        let z = (seed >> 12) % 64;
        let id = ids[((seed >> 18) % ids.len() as u32) as usize];
        assert!(w.set_block(o + IVec3::new(x as i32, y as i32, z as i32), id));
        dense[dense_index(x as usize, y as usize, z as usize)] = id;
        assert_eq!(
            w.get_block(o + IVec3::new(x as i32, y as i32, z as i32)),
            id
        );
        if i % 250 == 0 {
            assert_world_matches(&w, k, &dense);
        }
    }
    assert_world_matches(&w, k, &dense);
    let rec = &w.chunks[&k];
    let solid = dense.iter().filter(|&&b| b != AIR).count() as u32;
    assert_eq!(rec.solid_count, solid);

    let fresh = build_local(&dense);
    let live_before = w.leaves.live_elems;
    w.insert_local(key(9, 9, 9), fresh);
    assert_eq!(
        w.leaves.live_elems, live_before,
        "edited chunk and rebuilt chunk should share every leaf run"
    );
    w.remove_chunk(k);
    w.remove_chunk(key(9, 9, 9));
    assert_pools_empty(&w);
}

#[test]
fn many_block_types_attribute_recovery() {
    let dense = dense_from(|x, y, z| {
        if y > 40 {
            AIR
        } else {
            (1 + (x * 7 + y * 3 + z * 5) % (BLOCK_COUNT as u32 - 1)) as BlockId
        }
    });
    let mut w = World::new();
    w.insert_local(key(0, 0, 0), build_local(&dense));
    assert_world_matches(&w, key(0, 0, 0), &dense);
    w.remove_chunk(key(0, 0, 0));
    assert_pools_empty(&w);
}

#[test]
fn chunk_key_math() {
    assert_eq!(
        ChunkKey::of_block(IVec3::new(-1, 0, 63)).pos,
        IVec3::new(-1, 0, 0)
    );
    assert_eq!(
        ChunkKey::of_block(IVec3::new(-64, 64, 128)).pos,
        IVec3::new(-1, 1, 2)
    );
    let k = ChunkKey::new(2, IVec3::new(-1, 0, 1));
    assert_eq!(k.size(), 256);
    assert_eq!(k.origin(), IVec3::new(-256, 0, 256));
    assert_eq!(k.child(1, 0, 1), ChunkKey::new(1, IVec3::new(-1, 0, 3)));
    assert_eq!(k.child(1, 0, 1).parent(), k);
    assert_eq!(
        ChunkKey::new(0, IVec3::new(-3, 0, 5)).parent(),
        ChunkKey::new(1, IVec3::new(-2, 0, 2))
    );
}

#[test]
fn water_line_packing_and_stratification() {

    let dense = dense_from(|_x, y, _z| {
        if y <= 9 {
            DIRT
        } else if y <= 12 {
            WATER
        } else {
            AIR
        }
    });
    let t = build_local(&dense);

    for n in &t.l1 {

        if n.full {

            assert_eq!(n.water_line, 15, "full solid cell must report 15");
        } else {

            assert_eq!(n.water_line, 9 % 16, "mixed cell keeps the land top");
        }

        assert_eq!(
            (n.leaf_prefix | ((n.water_line as u32) << 28)) & PREFIX_MASK,
            n.leaf_prefix,
            "water_line must live strictly above PREFIX_MASK"
        );
    }

    assert_eq!(t.dry_mask, t.root_mask);
    assert_ne!(t.root_mask, 0);
}

#[test]
fn water_only_cells_report_fifteen_and_leave_the_dry_mask() {
    let dense = dense_from(|_x, y, _z| {
        if y <= 4 {
            STONE
        } else if y <= 30 {
            WATER
        } else {
            AIR
        }
    });
    let t = build_local(&dense);

    let column: Vec<_> = t.l1.iter().collect();
    assert!(!column.is_empty());

    let water_only = t
        .l1
        .iter()
        .filter(|n| n.water_line == 15 && !n.full)
        .count();
    assert_eq!(water_only, 16, "16 water-only cells in the y=1 band (4x4 in x/z)");
    let dry_solid = t.l1.iter().filter(|n| n.water_line == 15 && n.full).count();
    let carrying_top = t.l1.iter().filter(|n| n.water_line == 4).count();
    assert_eq!(dry_solid + carrying_top, 16, "the y=0 band's sixteen cells");

    assert_eq!(t.dry_mask.count_ones(), 16);
    assert!(t.dry_mask != t.root_mask, "root mask must include water-only cells");
    assert_eq!(t.root_mask.count_ones(), 32);
}
