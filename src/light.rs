//! Sky and block light: a 15-level flood fill stored in 4^3 bricks with uniform flags.
//!
//! Light lives in air voxels, which have no leaf index in a geometry-only tree, so it
//! cannot share the attribute indexing. It shares the brick pool instead: one 4096-entry
//! cell table per chunk, each entry either an inline uniform **half-word** or a 32-word brick.
//!
//! **Block light has been three channels since batch 59, and the cell doubled to carry them.**
//! Roadmap R4's structural half: a cell was `sky:4 | block:4` in one byte and is now
//! `sky:4 | r:4 | g:4 | b:4` in two. What that buys is *mixing* -- the thing a per-block tint
//! applied at read time cannot do, because the cell between a red lamp and a blue one holds
//! one number and no memory of which block put it there. Three independent floods hold both
//! and their overlap reads purple.
//!
//! **The propagation rule did not change and that is the point.** [`flood`] is called three
//! times on three arrays rather than rewritten to know about colour: each channel decrements
//! by one per step exactly as the single channel did, so a white emitter's three channels
//! stay equal at every cell they reach and `max(r, g, b)` reproduces the pre-59 level
//! everywhere. That identity is what makes `--no-light-rgb` a bit-exact control rather than a
//! close one, and `tests/light.rs` asserts it rather than leaving it to the sweep.

use crate::block::{self, BlockId, AIR, WATER};
use crate::voxel::tree::{cell_bit, dense_index};
use crate::voxel::VOL;

pub const CELLS: usize = 4096;
/// **32 since batch 59, and it is the only number here that a shader also has to know.** A
/// brick is 64 cells of two bytes, so two cells share a word; `light_at` in `common.wgsl`
/// indexes `bit >> 1` and shifts by `(bit & 1) * 16`, and `light_cell_is_two_bytes_wide` in
/// `tests/light.rs` is what holds the two copies together.
pub const BRICK_WORDS: usize = 32;
pub const MAX_LEVEL: u8 = 15;

/// Packed light cell: sky in bits 12..15, then block red, green and blue a nibble each.
///
/// **Sky stays at the top for the reason it was at the top when this was a byte**: it is the
/// channel every reader wants and the one a missing chunk has to fake, so `world_sample`'s
/// out-of-range answer is the single literal `0xF000` -- full sky, no block light -- rather
/// than a composition.
#[inline]
pub fn pack(sky: u8, blk: [u8; 3]) -> u16 {
    ((sky as u16) << 12)
        | ((blk[0] as u16 & 0xF) << 8)
        | ((blk[1] as u16 & 0xF) << 4)
        | (blk[2] as u16 & 0xF)
}

#[inline]
pub fn sky_of(b: u16) -> u8 {
    (b >> 12) as u8
}

#[inline]
pub fn block_of(b: u16) -> [u8; 3] {
    [
        ((b >> 8) & 0xF) as u8,
        ((b >> 4) & 0xF) as u8,
        (b & 0xF) as u8,
    ]
}

/// The pre-59 scalar block level, which is `max` over the three channels.
///
/// **Named rather than spelled out, because it is a claim and not a convenience.** Every
/// channel of an emitter decrements by one per step, so at any cell the three hold
/// `(r0 - d, g0 - d, b0 - d)` clamped at zero for the nearest emitter that reaches it -- and
/// the largest of those is the largest source level minus the same `d`, which is exactly what
/// the single pre-59 flood from `def(..).light` would have held. `--no-light-rgb` reads this
/// and gets the old frame to the bit; `tests/light.rs::the_scalar_control_reproduces_the_old_flood`
/// is the assert, and `resolve.wgsl` carries the same `max` under `SPEC_LIGHT_RGB` being off.
#[inline]
pub fn block_level(b: u16) -> u8 {
    let c = block_of(b);
    c[0].max(c[1]).max(c[2])
}

/// Result of lighting one chunk: a cell table plus the bricks it points at.
pub struct LightData {
    /// 4096 entries; bit 31 set means an inline uniform cell, else a brick index into
    /// `bricks` (the caller turns it into a pool offset).
    pub cells: Vec<u32>,
    pub bricks: Vec<[u32; BRICK_WORDS]>,
    /// Set when the whole chunk is one value, in which case `cells` is empty.
    pub uniform: Option<u16>,
}

const UNIFORM_BIT: u32 = 1 << 31;

/// Compute light for one chunk from its dense block array and the sky level entering
/// from above. `above` is the sky light of the voxel directly above the chunk's top.
pub fn compute(dense: &[BlockId], above_open: &[bool]) -> LightData {
    debug_assert_eq!(dense.len(), VOL);

    // Two cases cover most of a world and skip the fill entirely: chunks wholly above
    // the terrain are uniformly lit, and chunks with no sky access and no emitters are
    // uniformly dark.
    let any_emitter = dense.iter().any(|&b| block::def(b).light != [0, 0, 0]);
    let open_count = above_open.iter().filter(|&&o| o).count();
    // `all(AIR)` rather than `all(!opaque)`: a chunk full of water is not uniformly lit,
    // it is a depth gradient, and the pour below is what computes it.
    if open_count == above_open.len() && !any_emitter && dense.iter().all(|&b| b == AIR) {
        return LightData {
            cells: Vec::new(),
            bricks: Vec::new(),
            uniform: Some(pack(MAX_LEVEL, [0, 0, 0])),
        };
    }
    if open_count == 0 && !any_emitter {
        return LightData {
            cells: Vec::new(),
            bricks: Vec::new(),
            uniform: Some(0),
        };
    }

    let mut sky = vec![0u8; VOL];
    // Three channels, allocated only where there is something to put in them. A world with no
    // emitter in it is the overwhelmingly common case -- roadmap R4's own premise was that one
    // block in the table emitted at all -- so the empty arms below cost the zeroing of nothing
    // rather than of 768 KB per chunk build.
    let mut blk: [Vec<u8>; 3] = if any_emitter {
        [vec![0u8; VOL], vec![0u8; VOL], vec![0u8; VOL]]
    } else {
        [Vec::new(), Vec::new(), Vec::new()]
    };
    let mut queue: Vec<u32> = Vec::with_capacity(4096);

    // Sky light falls straight down through open columns. Through air it keeps full
    // strength; through water it loses one level per block, which is Minecraft's own rule
    // and the reason a sea floor darkens with depth instead of reading as uniformly lit.
    // The *colour* of that depth is the renderer's Beer-Lambert term and not this: fifteen
    // integer levels are far too coarse to carry a per-channel absorption curve.
    for z in 0..64usize {
        for x in 0..64usize {
            if !above_open[x + z * 64] {
                continue;
            }
            let mut level = MAX_LEVEL;
            for y in (0..64usize).rev() {
                let i = dense_index(x, y, z);
                if block::casts_sun_shadow(dense[i]) {
                    break;
                }
                sky[i] = level;
                queue.push(i as u32);
                if dense[i] == WATER {
                    if level == 0 {
                        break;
                    }
                    level -= 1;
                }
            }
        }
    }
    flood(&mut sky, dense, &mut queue, true);

    // Block light from emitters, one flood per channel (batch 59).
    //
    // **The dense array is scanned once and the three floods walk a list.** A channel-major
    // loop that re-scanned 262144 voxels three times would triple the *scan* as well as the
    // fill, and the scan is the part that runs whether or not a chunk has an emitter in it.
    if any_emitter {
        let emitters: Vec<u32> = dense
            .iter()
            .enumerate()
            .filter(|(_, &id)| block::def(id).light != [0, 0, 0])
            .map(|(i, _)| i as u32)
            .collect();
        for (c, level) in blk.iter_mut().enumerate() {
            queue.clear();
            for &i in &emitters {
                let e = block::def(dense[i as usize]).light[c];
                if e > 0 {
                    level[i as usize] = e;
                    queue.push(i);
                }
            }
            flood(level, dense, &mut queue, false);
        }
    }

    pack_bricks(&sky, &blk)
}

/// Blocks the flood entirely. Water does not: it is transparent to the fill and attenuates
/// only the vertical sky pour, so light still reaches sideways under a ledge the way it
/// does through air and a shallow bottom stays lit.
#[inline]
fn opaque(id: BlockId) -> bool {
    block::def(id).opaque
}

/// Standard decrement-by-one flood fill over the 6 neighbours, chunk-local.
fn flood(level: &mut [u8], dense: &[BlockId], queue: &mut Vec<u32>, sunlight: bool) {
    let mut head = 0;
    while head < queue.len() {
        let i = queue[head] as usize;
        head += 1;
        let l = level[i];
        if l <= 1 {
            continue;
        }
        let x = i & 63;
        let z = (i >> 6) & 63;
        let y = i >> 12;
        let next = l - 1;
        let mut visit = |nx: usize, ny: usize, nz: usize, q: &mut Vec<u32>| {
            let ni = dense_index(nx, ny, nz);
            let blocked = if sunlight { block::casts_sun_shadow(dense[ni]) } else { opaque(dense[ni]) };
            if blocked || level[ni] >= next {
                return;
            }
            level[ni] = next;
            q.push(ni as u32);
        };
        if x > 0 {
            visit(x - 1, y, z, queue);
        }
        if x < 63 {
            visit(x + 1, y, z, queue);
        }
        if y > 0 {
            visit(x, y - 1, z, queue);
        }
        if y < 63 {
            visit(x, y + 1, z, queue);
        }
        if z > 0 {
            visit(x, y, z - 1, queue);
        }
        if z < 63 {
            visit(x, y, z + 1, queue);
        }
        // Cap the working set; light never travels more than 15 steps anyway.
        if head > VOL * 2 {
            break;
        }
    }
}

/// `blk` is three channels, each either `VOL` long or **empty** -- empty being the chunk with
/// no emitter in it, where every block channel is zero by construction. The emptiness is
/// resolved once, into `lit`, rather than tested per cell: this loop runs 262144 times per
/// chunk and the answer is the same at every one of them.
fn pack_bricks(sky: &[u8], blk: &[Vec<u8>; 3]) -> LightData {
    let lit = !blk[0].is_empty();
    let at = |i: usize| -> [u8; 3] {
        if lit {
            [blk[0][i], blk[1][i], blk[2][i]]
        } else {
            [0, 0, 0]
        }
    };
    let mut cells = vec![0u32; CELLS];
    let mut bricks: Vec<[u32; BRICK_WORDS]> = Vec::new();
    let first = pack(sky[0], at(0));
    let mut chunk_uniform = true;
    for cz in 0..16usize {
        for cy in 0..16usize {
            for cx in 0..16usize {
                let mut brick = [0u32; BRICK_WORDS];
                let mut uniform = true;
                let mut val = 0u16;
                for vz in 0..4usize {
                    for vy in 0..4usize {
                        for vx in 0..4usize {
                            let i = dense_index(cx * 4 + vx, cy * 4 + vy, cz * 4 + vz);
                            let b = pack(sky[i], at(i));
                            let bit = cell_bit(vx as u32, vy as u32, vz as u32) as usize;
                            if bit == 0 {
                                val = b;
                            } else if b != val {
                                uniform = false;
                            }
                            // Two cells to a word since batch 59. `common.wgsl` mirrors this
                            // shift exactly; `BRICK_WORDS`' own comment says what holds them
                            // together.
                            brick[bit >> 1] |= (b as u32) << ((bit & 1) * 16);
                        }
                    }
                }
                let ci = cx | (cy << 4) | (cz << 8);
                if uniform {
                    cells[ci] = UNIFORM_BIT | val as u32;
                    if val != first {
                        chunk_uniform = false;
                    }
                } else {
                    cells[ci] = bricks.len() as u32;
                    bricks.push(brick);
                    chunk_uniform = false;
                }
            }
        }
    }
    if chunk_uniform {
        LightData {
            cells: Vec::new(),
            bricks: Vec::new(),
            uniform: Some(first),
        }
    } else {
        LightData {
            cells,
            bricks,
            uniform: None,
        }
    }
}

/// Which columns of a chunk see open sky, given the terrain heights above it.
/// `chunk_top` is the internal Y of the voxel just above the chunk.
pub fn open_columns(heights: &[i32; 64 * 64], chunk_top: i32) -> Vec<bool> {
    heights.iter().map(|&h| h <= chunk_top).collect()
}



