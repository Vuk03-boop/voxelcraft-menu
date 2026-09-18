use super::geometry::Inner;
use crate::math::Aabb;
use glam::{IVec3, Vec3};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ChunkKey {
    pub lod: u8,

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

    Uniform(u16),

    Table { base: u32 },
}

pub struct ChunkRecord {
    pub key: ChunkKey,
    pub root: Inner,
    pub attr: AttrRef,

    pub regions: Vec<(u32, usize)>,
    pub light: LightRef,

    pub light_region: Option<(u32, usize)>,

    pub aabb_min: IVec3,
    pub aabb_max: IVec3,
    pub solid_count: u32,
    pub leaf_count: u32,

    pub has_water: bool,

    pub dry_mask: u64,

    pub has_foliage: bool,

    pub has_cutout: bool,

    pub has_glass: bool,

    pub has_emitter: bool,

    pub emitters: u32,

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
