// These tests hold **transliterations** of formulations in `src/`, on purpose: the point of
// the copy is that it reads the same as the original, so a divergence is visible. Clippy's
// modernisations here would silently make the two halves of each mirror look different, which
// costs exactly the thing the duplication buys.
#![allow(clippy::int_plus_one)]
#![allow(clippy::manual_is_multiple_of)]
#![allow(clippy::manual_range_contains)]

use glam::IVec3;
use voxelcraft::block::*;
use voxelcraft::voxel::*;

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

/// Regression: a new leaf in an L1 cell that did not exist yet was inserted at index 0 of
/// the chunk's attribute table instead of at its true prefix, silently rewriting an
/// unrelated block elsewhere in the chunk. An absent L1 node carries `leaf_prefix == 0`,
/// so the insertion point has to be derived from the sibling counts, not from the node.
/// This walks the whole chunk after every edit, which is how the corruption was found.
#[test]
fn edits_never_corrupt_unrelated_blocks() {
    let mut dense = vec![AIR; VOL];
    for y in 0..64u32 {
        for z in 0..64u32 {
            for x in 0..64u32 {
                dense[dense_index(x as usize, y as usize, z as usize)] = terrain(x, y, z, 4);
            }
        }
    }
    let mut w = World::new();
    let k = ChunkKey::new(0, IVec3::new(-1, 0, 2));
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
        let p = o + IVec3::new(x as i32, y as i32, z as i32);
        let prev = w.get_block(p);
        let prev_dense = dense[dense_index(x as usize, y as usize, z as usize)];
        let attr_before = w.chunks[&k].attr;
        assert!(w.set_block(p, id));
        dense[dense_index(x as usize, y as usize, z as usize)] = id;
        let got = w.get_block(p);
        if got != id {
            let rec = &w.chunks[&k];
            panic!(
                "iteration {i}: pos {x} {y} {z} prev={prev} (dense {prev_dense}) set={id} got={got}\n attr before {:?} after {:?}\n root {:?} leaf_count {} solid {}",
                attr_before, rec.attr, rec.root, rec.leaf_count, rec.solid_count
            );
        }
        // Also check a full mirror every step for the first few hundred.
        if i < 400 {
            for yy in 0..64u32 {
                for zz in 0..64u32 {
                    for xx in 0..64u32 {
                        let want = dense[dense_index(xx as usize, yy as usize, zz as usize)];
                        let g = w.get_block(o + IVec3::new(xx as i32, yy as i32, zz as i32));
                        if g != want {
                            panic!("iteration {i}: after edit at {x} {y} {z} (prev={prev} set={id}), mismatch at {xx} {yy} {zz}: got {g} want {want}");
                        }
                    }
                }
            }
        }
    }
}



