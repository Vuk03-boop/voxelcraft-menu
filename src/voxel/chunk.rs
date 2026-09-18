//! Chunk identity and the per-chunk record held by the world.

use super::geometry::Inner;
use crate::math::Aabb;
use glam::{IVec3, Vec3};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ChunkKey {
    pub lod: u8,
    /// Position in units of this LOD level chunk size.
    pub pos: IVec3,
}

impl ChunkKey {
    pub fn new(lod: u8, pos: IVec3) -> Self {
        Self { lod, pos }
    }
    #[inline]
    pub fn size(&self) -> i32 {
        64 << self.lod
    }
    #[inline]
    pub fn voxel_size(&self) -> i32 {
        1 << self.lod
    }
    #[inline]
    pub fn origin(&self) -> IVec3 {
        self.pos * self.size()
    }
    pub fn bounds(&self) -> Aabb {
        let o = self.origin().as_vec3();
        Aabb::new(o, o + Vec3::splat(self.size() as f32))
    }
    pub fn center(&self) -> Vec3 {
        self.origin().as_vec3() + Vec3::splat(self.size() as f32 * 0.5)
    }
    pub fn parent(&self) -> ChunkKey {
        ChunkKey {
            lod: self.lod + 1,
            pos: self.pos >> 1,
        }
    }
    pub fn child(&self, dx: i32, dy: i32, dz: i32) -> ChunkKey {
        ChunkKey {
            lod: self.lod - 1,
            pos: self.pos * 2 + IVec3::new(dx, dy, dz),
        }
    }
    /// LOD-0 chunk containing a block position.
    pub fn of_block(p: IVec3) -> ChunkKey {
        ChunkKey {
            lod: 0,
            pos: p >> 6,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttrRef {
    Uniform(u16),
    Table { base: u32, len: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightRef {
    /// Whole chunk has this light cell (sky:4 | r:4 | g:4 | b:4 since batch 59).
    Uniform(u16),
    /// 4096-entry cell table in the brick pool.
    Table { base: u32 },
}

pub struct ChunkRecord {
    pub key: ChunkKey,
    pub root: Inner,
    pub attr: AttrRef,
    /// Extra pool regions owned by this chunk (palette regions, per-edit blocks).
    pub regions: Vec<(u32, usize)>,
    pub light: LightRef,
    /// Pool region holding this chunk's light bricks, if it has any.
    pub light_region: Option<(u32, usize)>,
    /// Tight bounds of solid voxels in local voxel units, max exclusive.
    pub aabb_min: IVec3,
    pub aabb_max: IVec3,
    pub solid_count: u32,
    pub leaf_count: u32,
    /// Any water in this chunk. Conservative: an edit that adds water sets it, and one
    /// that removes the last of it does not clear it, because clearing would mean a scan.
    /// A stale `true` only costs the shading passes an attribute lookup they did not need.
    pub has_water: bool,
    /// `root_mask` minus the 16^3 cells that hold nothing but water (batch 53). See
    /// `LocalTree::dry_mask`; like `has_water` it is only ever widened by an edit, because a
    /// stale set bit is slow and a stale clear bit would delete geometry.
    pub dry_mask: u64,
    /// Any cross-quad foliage in this chunk. Conservative in the same direction and for the
    /// same reason as `has_water`.
    pub has_foliage: bool,
    /// Any leaf block. Conservative in the same direction and for the same reason as
    /// `has_water`: an edit that adds one sets it, and removing the last one does not clear
    /// it, because clearing would mean a scan.
    pub has_cutout: bool,
    /// Any glass. Conservative in the same direction and for the same reason as `has_water`,
    /// and the flag an *edit* is by far the likeliest source of: no generator places glass,
    /// so in practice this turns on when a player builds a window and never before.
    pub has_glass: bool,
    /// Any emitter block (roadmap P11). CPU-side only: it rolls up into `World`'s
    /// emitter count, which keys `SPEC_EMITTER_GATHER`, and never reaches the GPU per
    /// chunk. Maintained as a **count** (`emitters`, keyed off every edit's (old, new)
    /// pair), not conservatively: D5's "what un-sticks the arming" is this field's zero.
    pub has_emitter: bool,
    /// Emitting voxels. The zero crossings of this number are the only transitions of
    /// `World::emitter_chunks`; see `world.rs`'s `set_block`.
    pub emitters: u32,
    /// Bumped on every edit so the renderer can refresh its list entry.
    pub version: u32,
}

impl ChunkRecord {
    pub fn world_aabb(&self) -> Aabb {
        let o = self.key.origin().as_vec3();
        let vs = self.key.voxel_size() as f32;
        Aabb::new(
            o + self.aabb_min.as_vec3() * vs,
            o + self.aabb_max.as_vec3() * vs,
        )
    }
}



