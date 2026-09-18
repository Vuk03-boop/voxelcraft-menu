//! Dense 64^3 scratch to local (thread-private) tree conversion, and cell arithmetic.

use super::attributes::{decode_leaf, encode_leaf, UNIFORM_BIT};
use crate::block::{BlockId, AIR, WATER};

pub const DIM: usize = 64;
pub const VOL: usize = DIM * DIM * DIM;

/// Dense scratch index; x fastest, then z, then y (power-of-two shifts only).
#[inline]
pub fn dense_index(x: usize, y: usize, z: usize) -> usize {
    x | (z << 6) | (y << 12)
}

/// Bit index of a cell within its 4^3 parent: x | y<<2 | z<<4.
#[inline]
pub fn cell_bit(x: u32, y: u32, z: u32) -> u32 {
    x | (y << 2) | (z << 4)
}

#[inline]
pub fn l1_bit(x: u32, y: u32, z: u32) -> u32 {
    cell_bit(x >> 4, y >> 4, z >> 4)
}

#[inline]
pub fn leaf_bit(x: u32, y: u32, z: u32) -> u32 {
    cell_bit((x >> 2) & 3, (y >> 2) & 3, (z >> 2) & 3)
}

#[inline]
pub fn voxel_bit(x: u32, y: u32, z: u32) -> u32 {
    cell_bit(x & 3, y & 3, z & 3)
}

/// Local coordinates (0..4 each) of a cell bit.
#[inline]
pub fn bit_xyz(bit: u32) -> (u32, u32, u32) {
    (bit & 3, (bit >> 2) & 3, (bit >> 4) & 3)
}

#[derive(Default)]
pub struct LocalL1 {
    pub mask: u64,
    /// Start of the leaf run in `LocalTree::leaves` (unused when `full`).
    pub leaf_start: u32,
    pub leaf_prefix: u32,
    pub full: bool,
    /// Roadmap P9: the highest node-local y (0..=15) of any occupied non-water voxel in
    /// this 16^3 node, or **15 for "no bound"**. Everything strictly above it is water or
    /// air, so a ray that ignores water may take the whole 16-row in one step. Packed
    /// into the interned node's top nibble at `world.rs`'s interning -- `leaf_prefix` is
    /// a count bounded by 4096 and never reaches the high four bits.
    pub water_line: u8,
}

/// A chunk tree built on a worker thread, before interning into the global pools.
#[derive(Default)]
pub struct LocalTree {
    pub root_mask: u64,
    /// `root_mask` with every 16^3 cell holding **nothing but water** cleared (batch 53).
    ///
    /// A ray that ignores water is looking at a different world from the one the occupancy
    /// tree describes, because **water is in that tree** -- which `docs/water.md` calls the
    /// load-bearing choice, and it is, in both directions. A submerged primary ray, and every
    /// refracted, reflected and shadow ray, therefore grinds through an ocean interior that
    /// reads as solid at every level and is empty to it. This is the same 64-bit mask with
    /// those cells taken out, so `march_chunk` can skip a water-only 16^3 in one step instead
    /// of four, and a water-only 32^3 in one instead of sixteen.
    ///
    /// **Glass and foliage count as dry**, deliberately: a secondary ray ignores those too,
    /// but the primary ray does not, and one mask that is right for every ray class is worth
    /// more than three that are each right for one. The cost is that a pane in open water
    /// stops the skip, and there is no such thing in a generated world.
    ///
    /// A stale **set** bit is slow and correct; a stale **clear** bit would delete geometry.
    /// Every writer therefore only ever sets, which is the convention `has_water` already
    /// uses one field along and for the same reason.
    pub dry_mask: u64,
    pub l1: Vec<LocalL1>,
    pub leaves: Vec<u64>,
    /// One entry per leaf (in DFS order): inline uniform id or offset into `palette`.
    pub attrs: Vec<u32>,
    pub palette: Vec<u32>,
    /// Set when every solid voxel has the same block id.
    pub uniform: Option<BlockId>,
    /// Any water in this chunk at all. The shading passes need to tell water apart from
    /// opaque geometry, and that costs an attribute lookup per cell; this flag is what
    /// keeps the cost at zero for the overwhelming majority of chunks, which have none.
    pub has_water: bool,
    /// Any cross-quad foliage in this chunk. Same job as `has_water` and the same reason:
    /// the marcher can only tell a tuft from a cube by looking its block id up, and this
    /// flag is what keeps that lookup off every chunk that has none -- which is every
    /// coarse chunk, every chunk underground and every chunk in the sky.
    pub has_foliage: bool,
    /// Any sub-voxel cutout block -- leaves. Third of the same kind and for the third time
    /// the same reason: the marcher can only tell a carved cube from a whole one by looking
    /// its block id up, and this keeps that lookup off every chunk holding no canopy.
    pub has_cutout: bool,
    /// Any glass. Fourth of the same kind, and the one whose zero is load-bearing rather
    /// than merely cheap: the generator places no glass anywhere, so every chunk of every
    /// generated world carries `false` here and batch 38b's traversal cost is *zero* until
    /// somebody builds with it. That is what makes the fourteen fixture vantages bit-exact
    /// against the pre-38b binary by construction instead of by measurement.
    pub has_glass: bool,
    /// Any emitter block. Fifth of the same kind, but read on the CPU rather than in the
    /// marcher: roadmap P11's gate is per *frame* and uniform across it, so the flag
    /// rolls up into `World`'s emitter count and never reaches the GPU per chunk.
    /// **D5 turned the bool into the count it always implied** (see [`Self::emitters`]):
    /// `set_block` can then disarm the gate on the frame the last lamp breaks instead of
    /// on the someday its chunk streams out.
    pub has_emitter: bool,
    /// Emitting voxels in the chunk. The exact datum behind `has_emitter`, kept per voxel
    /// so an edit's (old, new) pair can move `World::emitter_chunks` at zero crossings.
    pub emitters: u32,
    pub solid_count: u32,
    pub leaf_count: u32,
    /// Tight bounds of solid voxels in local voxel units, max exclusive.
    pub aabb_min: [u8; 3],
    pub aabb_max: [u8; 3],
}

impl LocalTree {
    pub fn is_empty(&self) -> bool {
        self.root_mask == 0
    }

    pub fn is_full_solid(&self) -> bool {
        self.l1.len() == 64 && self.l1.iter().all(|n| n.full)
    }
}

/// Build a local tree from a dense block array.
pub fn build_local(dense: &[BlockId]) -> LocalTree {
    debug_assert_eq!(dense.len(), VOL);
    let mut t = LocalTree::default();
    let mut ids = [AIR; 64];
    let mut first: Option<BlockId> = None;
    let mut all_same = true;
    let mut amin = [64u8; 3];
    let mut amax = [0u8; 3];

    for l1z in 0..4u32 {
        for l1y in 0..4u32 {
            for l1x in 0..4u32 {
                let mut l1mask = 0u64;
                let leaf_start = t.leaves.len() as u32;
                let mut all_full = true;
                // Anything in this 16^3 that a water-ignoring ray would still stop at.
                let mut l1_dry = false;
                let mut max_nonwater_y = 0u32;
                for lz in 0..4u32 {
                    for ly in 0..4u32 {
                        for lx in 0..4u32 {
                            let mut m = 0u64;
                            let bx = l1x * 16 + lx * 4;
                            let by = l1y * 16 + ly * 4;
                            let bz = l1z * 16 + lz * 4;
                            for vz in 0..4u32 {
                                for vy in 0..4u32 {
                                    let row = dense_index(
                                        bx as usize,
                                        (by + vy) as usize,
                                        (bz + vz) as usize,
                                    );
                                    for vx in 0..4u32 {
                                        let id = dense[row + vx as usize];
                                        if id != AIR {
                                            let vb = cell_bit(vx, vy, vz);
                                            m |= 1u64 << vb;
                                            ids[vb as usize] = id;
                                            t.has_water |= id == WATER;
                                            l1_dry |= id != WATER;
                                            if id != WATER {
                                                max_nonwater_y = max_nonwater_y.max(ly * 4 + vy);
                                            }
                                            t.has_foliage |= crate::block::is_foliage(id);
                                            t.has_cutout |= crate::block::is_cutout(id);
                                            t.has_glass |= crate::block::is_glass(id);
                                            let e = crate::block::is_emitter(id);
                                            t.has_emitter |= e;
                                            t.emitters += e as u32;
                                            match first {
                                                None => first = Some(id),
                                                Some(f) if f != id => all_same = false,
                                                _ => {}
                                            }
                                            let (x, y, z) =
                                                ((bx + vx) as u8, (by + vy) as u8, (bz + vz) as u8);
                                            amin = [amin[0].min(x), amin[1].min(y), amin[2].min(z)];
                                            amax = [
                                                amax[0].max(x + 1),
                                                amax[1].max(y + 1),
                                                amax[2].max(z + 1),
                                            ];
                                        }
                                    }
                                }
                            }
                            if m != 0 {
                                l1mask |= 1u64 << cell_bit(lx, ly, lz);
                                t.leaves.push(m);
                                let e = encode_leaf(&ids, m, &mut t.palette);
                                t.attrs.push(e);
                                if m != u64::MAX {
                                    all_full = false;
                                }
                            } else {
                                all_full = false;
                            }
                        }
                    }
                }
                if l1mask != 0 {
                    if l1_dry {
                        t.dry_mask |= 1u64 << cell_bit(l1x, l1y, l1z);
                    }
                    let full = all_full && l1mask == u64::MAX;
                    if full {
                        t.leaves.truncate(leaf_start as usize);
                    }
                    t.l1.push(LocalL1 {
                        mask: l1mask,
                        leaf_start,
                        leaf_prefix: t.leaf_count,
                        full,
                        water_line: if l1_dry { max_nonwater_y as u8 } else { 15 },
                    });
                    t.leaf_count += l1mask.count_ones();
                    t.root_mask |= 1u64 << cell_bit(l1x, l1y, l1z);
                }
            }
        }
    }
    t.solid_count = t.leaves.iter().map(|m| m.count_ones()).sum::<u32>()
        + t.l1.iter().filter(|n| n.full).count() as u32 * 4096;
    if let Some(f) = first {
        if all_same {
            t.uniform = Some(f);
            t.attrs.clear();
            t.palette.clear();
        }
    }
    if t.solid_count > 0 {
        t.aabb_min = amin;
        t.aabb_max = amax;
    }
    t
}

/// Read back one voxel from a local tree (test helper; slow).
pub fn local_block(t: &LocalTree, x: u32, y: u32, z: u32) -> BlockId {
    let l1b = l1_bit(x, y, z);
    if (t.root_mask >> l1b) & 1 == 0 {
        return AIR;
    }
    let l1i = (t.root_mask & ((1u64 << l1b) - 1)).count_ones() as usize;
    let n = &t.l1[l1i];
    let lfb = leaf_bit(x, y, z);
    if (n.mask >> lfb) & 1 == 0 {
        return AIR;
    }
    let li = (n.mask & ((1u64 << lfb) - 1)).count_ones();
    let vb = voxel_bit(x, y, z);
    let m = if n.full {
        u64::MAX
    } else {
        t.leaves[(n.leaf_start + li) as usize]
    };
    if (m >> vb) & 1 == 0 {
        return AIR;
    }
    if let Some(u) = t.uniform {
        return u;
    }
    let entry = t.attrs[(n.leaf_prefix + li) as usize];
    if entry & UNIFORM_BIT != 0 {
        entry as BlockId
    } else {
        decode_leaf(&t.palette, entry, vb)
    }
}



