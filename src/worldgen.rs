use crate::biome::{self, Biome, BiomeId};
use crate::block::*;
use crate::math::WORLD_HEIGHT;
use crate::voxel::{dense_index, ChunkKey, VOL};
use fastnoise_lite::{FastNoiseLite, FractalType, NoiseType};
use glam::IVec3;

pub const SEA_LEVEL: i32 = 128;

const LOD0_CANOPY_COLUMNS: f32 = 22.0;

#[derive(Clone, Copy, Debug)]
pub struct ColumnBiome {
    pub id: BiomeId,

    pub mountain_amp: f32,
    pub snow_line: i32,
    pub tree_line: i32,
}

impl ColumnBiome {
    const EMPTY: Self = Self {
        id: 0,
        mountain_amp: 0.0,
        snow_line: 0,
        tree_line: 0,
    };

    #[inline]
    pub fn def(&self) -> &'static Biome {
        biome::def(self.id)
    }
}

pub struct WorldGen {
    pub seed: i32,

    pub sea_level: i32,

    pub biomes: bool,

    pub foliage: bool,

    pub meadow: bool,

    pub bounce_shadow: bool,

    pub tree_blue_noise: bool,

    pub grass_dense: bool,

    pub meadow_side: bool,
    continental: FastNoiseLite,
    hills: FastNoiseLite,
    detail: FastNoiseLite,
    forest: FastNoiseLite,
    cave_a: FastNoiseLite,
    cave_b: FastNoiseLite,
    gravel: FastNoiseLite,
}

fn noise(seed: i32, freq: f32, octaves: i32) -> FastNoiseLite {
    let mut n = FastNoiseLite::with_seed(seed);
    n.set_noise_type(Some(NoiseType::OpenSimplex2));
    n.set_frequency(Some(freq));
    if octaves > 1 {
        n.set_fractal_type(Some(FractalType::FBm));
        n.set_fractal_octaves(Some(octaves));
    }
    n
}

#[inline]

#[doc(hidden)]
pub fn canopy_rank3(wx: i32, wy: i32, wz: i32, seed: i32) -> f32 {
    const A1: f64 = 0.682_327_803_828_019_3;
    const A2: f64 = 0.465_571_231_876_854_6;
    const A3: f64 = 0.317_667_222_756_072_5;

    let phase = (seed as f64).fract().abs();
    let r = (wx as f64).mul_add(A1, (wz as f64).mul_add(A3, (wy as f64).mul_add(A2, phase)));
    (r.fract()).abs() as f32
}

#[doc(hidden)]
pub const COARSE_CANOPY_KEEP: f32 = 0.62;

const _: () = assert!(
    COARSE_CANOPY_KEEP.to_bits() == crate::render::LEAF_FILL.to_bits(),
    "COARSE_CANOPY_KEEP must equal render::LEAF_FILL bit for bit"
);

fn hash2(x: i32, z: i32, seed: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8da6b343)
        ^ (z as u32).wrapping_mul(0xd8163841)
        ^ (seed as u32).wrapping_mul(0xcb1ab31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1e995);
    h ^ (h >> 15)
}

pub const fn has_foliage(foliage: bool, biomes: bool) -> bool {
    foliage && biomes
}

pub const MEADOW_SPREAD: f32 = 1.75;

pub const G2_GRASS_DENSITY: f32 = 2.5;

pub const G2_TALL_SHARE: u32 = 24;

pub const G2_COARSE_KEEP: f32 = 0.5;

pub const G2_COARSE_KEEP_FAR: f32 = 0.25;

pub const G2_COARSE_STRIDE_MAX: i32 = 4;

pub const G2_REED_FRACTION: f32 = 0.35;

pub const fn has_meadow(meadow: bool, foliage: bool, biomes: bool) -> bool {
    meadow && has_foliage(foliage, biomes)
}

#[inline]
pub fn ground_cover(forest: f32, grass_scale: f32) -> f32 {
    (0.55 + 0.40 * forest) * grass_scale
}

impl WorldGen {
    pub fn new(seed: i32) -> Self {
        Self::with_options(seed, SEA_LEVEL, true, true, true)
    }

    pub fn with_sea_level(seed: i32, sea_level: i32) -> Self {
        Self::with_options(seed, sea_level, true, true, true)
    }

    pub fn with_options(
        seed: i32,
        sea_level: i32,
        biomes: bool,
        foliage: bool,
        meadow: bool,
    ) -> Self {
        Self {
            seed,
            sea_level,
            biomes,
            foliage: has_foliage(foliage, biomes),
            meadow: has_meadow(meadow, foliage, biomes),

            bounce_shadow: true,

            tree_blue_noise: false,
            grass_dense: false,
            meadow_side: false,
            continental: noise(seed, 0.0008, 3),
            hills: noise(seed + 1, 0.0022, 3),
            detail: noise(seed + 2, 0.020, 2),
            forest: noise(seed + 3, 0.004, 1),
            cave_a: noise(seed + 4, 0.028, 1),
            cave_b: noise(seed + 5, 0.028, 1),
            gravel: noise(seed + 6, 0.05, 1),
        }
    }

    pub fn column_biome(&self, x: i32, z: i32) -> ColumnBiome {
        if !self.biomes {

            return ColumnBiome {
                id: biome::PLAINS,
                mountain_amp: biome::def(biome::PLAINS).mountain_amp,
                snow_line: i32::MAX,
                tree_line: i32::MAX,
            };
        }
        let (fx, fz) = (x as f32, z as f32);

        let t = biome::simplex2(self.seed + biome::TEMP_SEED, biome::TEMP_FREQ, fx, fz);
        let m = biome::simplex2(self.seed + biome::HUMID_SEED, biome::HUMID_FREQ, fx, fz);
        let (id, amp, snow, trees) = biome::lookup(t, m);
        ColumnBiome {
            id,
            mountain_amp: amp,
            snow_line: snow.round() as i32,
            tree_line: trees.round() as i32,
        }
    }

    pub fn biome_at(&self, x: i32, z: i32) -> BiomeId {
        self.column_biome(x, z).id
    }

    pub fn height(&self, x: i32, z: i32) -> i32 {
        self.height_with(x, z, self.column_biome(x, z).mountain_amp)
    }

    fn height_with(&self, x: i32, z: i32, mountain_amp: f32) -> i32 {
        let (fx, fz) = (x as f32, z as f32);
        let c = self.continental.get_noise_2d(fx, fz);
        let h = self.hills.get_noise_2d(fx, fz);
        let d = self.detail.get_noise_2d(fx, fz);

        let base = 122.0 + c * 22.0;
        let mask = ((c - 0.05) * 2.4).clamp(0.0, 1.0);
        let peaks = h.max(0.0);
        let mountains = peaks * peaks * mountain_amp * mask;
        let ht = base + h * 15.0 + mountains + d * 2.2;
        (ht.round() as i32).clamp(2, WORLD_HEIGHT - 8)
    }

    #[inline]
    fn column_block(&self, y: i32, h: i32, cb: &ColumnBiome) -> BlockId {
        if y >= h {
            return if y < self.sea_level { WATER } else { AIR };
        }
        if y == 0 {
            return BEDROCK;
        }

        let beach = h <= SEA_LEVEL + 2;

        #[allow(clippy::int_plus_one)]
        let snowy = h - 1 >= cb.snow_line;
        #[allow(clippy::int_plus_one)]
        let barren = h - 1 >= cb.tree_line;
        if y == h - 1 {
            if beach {
                return SAND;
            }
            if snowy {
                return SNOW;
            }
            return if barren { STONE } else { cb.def().surface };
        }
        if y >= h - 4 {
            if beach {
                return SAND;
            }
            return if snowy || barren {
                STONE
            } else {
                cb.def().subsurface
            };
        }
        STONE
    }

    #[inline]
    pub fn column_top(&self, h: i32) -> i32 {
        h.max(self.sea_level)
    }

    #[inline]
    pub fn surface_block_at(&self, x: i32, z: i32, h: i32) -> BlockId {
        let cb = self.column_biome(x, z);
        self.column_block(self.column_top(h) - 1, h, &cb)
    }

    fn column_field(
        &self,
        o: IVec3,
        s: i32,
        heights: &mut [i32; 64 * 64],
        cols: &mut [ColumnBiome; 64 * 64],
    ) -> i32 {
        let mut max_top = 0;
        for z in 0..64usize {
            for x in 0..64usize {
                let wx = o.x + x as i32 * s + s / 2;
                let wz = o.z + z as i32 * s + s / 2;
                let cb = self.column_biome(wx, wz);
                let h = self.height_with(wx, wz, cb.mountain_amp);
                let i = x + z * 64;
                heights[i] = h;
                cols[i] = cb;
                max_top = max_top.max(self.column_top(h));
            }
        }
        max_top
    }

    pub fn generate(&self, key: ChunkKey, out: &mut [BlockId], heights: &mut [i32; 64 * 64]) {
        debug_assert_eq!(out.len(), VOL);
        out.fill(AIR);
        if key.lod == 0 {
            self.generate_lod0(key, out, heights);
        } else {
            self.generate_coarse(key, out, heights);
        }
    }

    fn generate_lod0(&self, key: ChunkKey, out: &mut [BlockId], heights: &mut [i32; 64 * 64]) {
        let o = key.origin();
        let mut cols = [ColumnBiome::EMPTY; 64 * 64];
        let max_top = self.column_field(o, 1, heights, &mut cols);
        if o.y >= max_top {
            return;
        }

        let mut ca = [0f32; 17 * 17 * 17];
        let mut cb = [0f32; 17 * 17 * 17];
        let mut cg = [0f32; 17 * 17 * 17];
        let carve = o.y + 64 > 4;
        if carve {
            for gz in 0..17 {
                for gy in 0..17 {
                    for gx in 0..17 {
                        let (wx, wy, wz) = (
                            (o.x + gx as i32 * 4) as f32,
                            (o.y + gy as i32 * 4) as f32,
                            (o.z + gz as i32 * 4) as f32,
                        );
                        let i = gx + gy * 17 + gz * 289;
                        ca[i] = self.cave_a.get_noise_3d(wx, wy * 1.6, wz);
                        cb[i] = self.cave_b.get_noise_3d(wx, wy * 1.6, wz);
                        cg[i] = self.gravel.get_noise_3d(wx, wy, wz);
                    }
                }
            }
        }
        let lerp = |g: &[f32], x: usize, y: usize, z: usize| -> f32 {
            let (gx, gy, gz) = (x >> 2, y >> 2, z >> 2);
            let (fx, fy, fz) = (
                (x & 3) as f32 * 0.25,
                (y & 3) as f32 * 0.25,
                (z & 3) as f32 * 0.25,
            );
            let at =
                |dx: usize, dy: usize, dz: usize| g[(gx + dx) + (gy + dy) * 17 + (gz + dz) * 289];
            let x00 = at(0, 0, 0) + (at(1, 0, 0) - at(0, 0, 0)) * fx;
            let x10 = at(0, 1, 0) + (at(1, 1, 0) - at(0, 1, 0)) * fx;
            let x01 = at(0, 0, 1) + (at(1, 0, 1) - at(0, 0, 1)) * fx;
            let x11 = at(0, 1, 1) + (at(1, 1, 1) - at(0, 1, 1)) * fx;
            let y0 = x00 + (x10 - x00) * fy;
            let y1 = x01 + (x11 - x01) * fy;
            y0 + (y1 - y0) * fz
        };

        for z in 0..64usize {
            for x in 0..64usize {
                let h = heights[x + z * 64];
                let cbi = cols[x + z * 64];
                let top = self.column_top(h);
                for y in 0..64usize {
                    let wy = o.y + y as i32;
                    if wy >= top {
                        break;
                    }

                    if wy >= h {
                        out[dense_index(x, y, z)] = WATER;
                        continue;
                    }
                    let mut id = self.column_block(wy, h, &cbi);

                    if carve && id == STONE && wy < h - 1 && lerp(&cg, x, y, z) > 0.42 {
                        id = GRAVEL;
                    }
                    if carve && wy > 3 && wy < h {
                        let a = lerp(&ca, x, y, z);
                        let b = lerp(&cb, x, y, z);

                        let w =
                            0.055 + 0.03 * (1.0 - (wy as f32 / SEA_LEVEL as f32).clamp(0.0, 1.0));
                        if a.abs() < w && b.abs() < w {
                            id = AIR;
                        }
                    }
                    out[dense_index(x, y, z)] = id;
                }
            }
        }

        for z in 3..61usize {
            for x in 3..61usize {
                let h = heights[x + z * 64];
                let cbi = cols[x + z * 64];
                let b = cbi.def();

                if b.tree_scale <= 0.0 || h <= SEA_LEVEL + 2 || h > cbi.tree_line {
                    continue;
                }
                let (wx, wz) = (o.x + x as i32, o.z + z as i32);
                let f = self.forest.get_noise_2d(wx as f32, wz as f32);
                let prob = (0.0015 + 0.012 * f.max(0.0)) * b.tree_scale * 10000.0;
                if hash2(wx, wz, self.seed) % 10000 >= prob as u32 {
                    continue;
                }
                let ly = h - o.y;
                if ly < 1 || ly + 7 >= 64 {
                    continue;
                }

                if out[dense_index(x, ly as usize - 1, z)] != b.surface {
                    continue;
                }
                let ly = ly as usize;
                for dy in 0..5 {
                    out[dense_index(x, ly + dy, z)] = LOG;
                }
                for dy in 3..=6usize {
                    let r: i32 = if dy >= 6 { 1 } else { 2 };
                    for dz in -r..=r {
                        for dx in -r..=r {
                            if dy < 6
                                && dx.abs() == r
                                && dz.abs() == r
                                && !hash2(wx + dx, wz + dz, dy as i32).is_multiple_of(3)
                            {
                                continue;
                            }
                            if dy == 6 && dx.abs() + dz.abs() > 1 {
                                continue;
                            }
                            let i = dense_index(
                                (x as i32 + dx) as usize,
                                ly + dy,
                                (z as i32 + dz) as usize,
                            );
                            if out[i] == AIR {
                                out[i] = b.leaf;
                            }
                        }
                    }
                }
                out[dense_index(x, ly + 6, z)] = b.leaf;
            }
        }

        if self.foliage {
            for z in 0..64usize {
                for x in 0..64usize {
                    let h = heights[x + z * 64];
                    let cbi = cols[x + z * 64];
                    let b = cbi.def();

                    if b.grass_scale <= 0.0 || h <= SEA_LEVEL + 1 || h > cbi.tree_line {
                        continue;
                    }
                    let ly = h - o.y;
                    if !(1..64).contains(&ly) {
                        continue;
                    }
                    let ly = ly as usize;

                    if out[dense_index(x, ly - 1, z)] != b.surface {
                        continue;
                    }
                    if out[dense_index(x, ly, z)] != AIR {
                        continue;
                    }
                    let (wx, wz) = (o.x + x as i32, o.z + z as i32);

                    let f = self.forest.get_noise_2d(wx as f32, wz as f32);
                    let mut prob = ground_cover(f, b.grass_scale);
                    if self.grass_dense {

                        prob = (prob * G2_GRASS_DENSITY).min(0.95);
                    }
                    if (hash2(wx, wz, self.seed ^ 0x6720) % 10000) as f32 >= prob * 10000.0 {
                        continue;
                    }

                    if self.grass_dense
                        && (hash2(wx, wz, self.seed ^ 0xa455) % 100) < G2_TALL_SHARE
                        && ly + 1 < 64
                        && out[dense_index(x, ly + 1, z)] == AIR
                    {
                        out[dense_index(x, ly, z)] = GRASS_TALL;
                        out[dense_index(x, ly + 1, z)] = GRASS_TALL;
                        continue;
                    }
                    out[dense_index(x, ly, z)] = TALL_GRASS;
                }
            }
        }

        if self.foliage && self.grass_dense {
            for z in 0..64usize {
                for x in 0..64usize {
                    let h = heights[x + z * 64];

                    if !(SEA_LEVEL..=SEA_LEVEL + 2).contains(&h) {
                        continue;
                    }
                    let cbi = cols[x + z * 64];
                    let b = cbi.def();
                    if b.grass_scale <= 0.0 {
                        continue;
                    }

                    let open = (x > 0 && heights[x - 1 + z * 64] < SEA_LEVEL)
                        || (x < 63 && heights[x + 1 + z * 64] < SEA_LEVEL)
                        || (z > 0 && heights[x + (z - 1) * 64] < SEA_LEVEL)
                        || (z < 63 && heights[x + (z + 1) * 64] < SEA_LEVEL)
                        || h < SEA_LEVEL;
                    if !open {
                        continue;
                    }
                    let ly = h - o.y;
                    if !(1..62).contains(&ly) {
                        continue;
                    }
                    let ly = ly as usize;
                    let base = out[dense_index(x, ly - 1, z)];
                    if base != SAND && base != b.surface {
                        continue;
                    }
                    if out[dense_index(x, ly, z)] != AIR {
                        continue;
                    }

                    let (wx, wz) = (o.x + x as i32, o.z + z as i32);
                    let f = self.forest.get_noise_2d(wx as f32, wz as f32);
                    let prob = ground_cover(f, b.grass_scale) * G2_REED_FRACTION;
                    if (hash2(wx, wz, self.seed ^ 0x5eed) % 10000) as f32 >= prob * 10000.0 {
                        continue;
                    }

                    let stem = if (hash2(wx, wz, self.seed ^ 0x7eed) % 100) < 45 { 3 } else { 2 };
                    for dy in 0..stem {
                        let i = dense_index(x, ly + dy, z);
                        if out[i] != AIR {
                            break;
                        }
                        out[i] = REEDS;
                    }
                }
            }
        }
    }

    fn generate_coarse(&self, key: ChunkKey, out: &mut [BlockId], heights: &mut [i32; 64 * 64]) {
        let o = key.origin();
        let s = key.voxel_size();
        let mut cols = [ColumnBiome::EMPTY; 64 * 64];
        self.column_field(o, s, heights, &mut cols);
        for z in 0..64usize {
            for x in 0..64usize {
                let h = heights[x + z * 64];
                let cbi = cols[x + z * 64];
                let ceiling = self.column_top(h);
                for y in 0..64usize {
                    let wy = o.y + y as i32 * s;
                    if wy >= ceiling {
                        break;
                    }

                    let top = (wy + s - 1).min(ceiling - 1);
                    out[dense_index(x, y, z)] = self.column_block(top, h, &cbi);
                }
            }
        }

        let mut thinned = [0u64; 4 * 1024];
        self.coarse_trees(key, out, heights, &cols, &mut thinned);
        self.coarse_meadow(key, out, heights, &cols, &thinned);

        self.coarse_tufts(key, out, heights, &cols, &thinned);
    }

    fn coarse_meadow(
        &self,
        key: ChunkKey,
        out: &mut [BlockId],
        heights: &[i32; 64 * 64],
        cols: &[ColumnBiome; 64 * 64],
        thinned: &[u64; 4 * 1024],
    ) {
        if !self.meadow {
            return;
        }
        let s = key.voxel_size();
        let o = key.origin();
        for z in 0..64usize {
            for x in 0..64usize {
                let h = heights[x + z * 64];
                let cbi = cols[x + z * 64];
                let b = cbi.def();

                if b.grass_scale <= 0.0 || h <= SEA_LEVEL + 1 || h > cbi.tree_line {
                    continue;
                }
                let ly = (h - 1 - o.y).div_euclid(s);
                if !(0..64).contains(&ly) {
                    continue;
                }
                let ly = ly as usize;

                if out[dense_index(x, ly, z)] != b.surface {
                    continue;
                }
                if ly + 1 < 64 {
                    let above = dense_index(x, ly + 1, z);

                    if out[above] != AIR || (thinned[above >> 6] >> (above & 63)) & 1 == 1 {
                        continue;
                    }
                }

                let (wx, wz) = (o.x + x as i32 * s + s / 2, o.z + z as i32 * s + s / 2);
                let f = self.forest.get_noise_2d(wx as f32, wz as f32);

                let cover = (ground_cover(f, b.grass_scale) * MEADOW_SPREAD).min(1.0);

                if (hash2(wx, wz, self.seed ^ 0x4d13) % 10000) as f32 >= cover * 10000.0 {
                    continue;
                }

                out[dense_index(x, ly, z)] = if self.meadow_side {
                    MEADOW_SIDE
                } else {
                    MEADOW
                };
            }
        }
    }

    fn coarse_tufts(
        &self,
        key: ChunkKey,
        out: &mut [BlockId],
        heights: &[i32; 64 * 64],
        cols: &[ColumnBiome; 64 * 64],
        thinned: &[u64; 4 * 1024],
    ) {
        if !self.grass_dense {
            return;
        }
        let s = key.voxel_size();
        if s > G2_COARSE_STRIDE_MAX {
            return;
        }
        let o = key.origin();
        for z in 0..64usize {
            for x in 0..64usize {
                let h = heights[x + z * 64];
                let cbi = cols[x + z * 64];
                let b = cbi.def();

                if b.grass_scale <= 0.0 || h <= SEA_LEVEL + 1 || h > cbi.tree_line {
                    continue;
                }

                let ly = (h - o.y).div_euclid(s);
                if !(1..63).contains(&ly) {
                    continue;
                }
                let ly = ly as usize;

                if out[dense_index(x, ly - 1, z)] != b.surface {
                    continue;
                }
                let i = dense_index(x, ly, z);
                if out[i] != AIR {
                    continue;
                }
                let above = dense_index(x, ly + 1, z);
                if out[above] != AIR || (thinned[above >> 6] >> (above & 63)) & 1 == 1 {
                    continue;
                }

                let (wx, wz) = (o.x + x as i32 * s + s / 2, o.z + z as i32 * s + s / 2);
                let f = self.forest.get_noise_2d(wx as f32, wz as f32);
                let keep = if s > 2 { G2_COARSE_KEEP_FAR } else { G2_COARSE_KEEP };
                let ceiling = (ground_cover(f, b.grass_scale) * keep).min(1.0);
                let wy = o.y + ly as i32 * s;
                if canopy_rank3(wx, wy, wz, self.seed) >= ceiling {
                    continue;
                }
                out[i] = TALL_GRASS;
            }
        }
    }

    fn coarse_trees(
        &self,
        key: ChunkKey,
        out: &mut [BlockId],
        heights: &[i32; 64 * 64],
        cols: &[ColumnBiome; 64 * 64],
        thinned: &mut [u64; 4 * 1024],
    ) {
        let s = key.voxel_size();

        if s > 8 {
            return;
        }
        let o = key.origin();
        let trunk = (3 / s) as usize;
        let canopy = ((4 / s).max(1)) as usize;
        let r = 2 / s;
        let lo = r as usize;
        let hi = 64 - r as usize;

        let footprint = ((2 * r + 1) * (2 * r + 1)) as f32;
        let density = LOD0_CANOPY_COLUMNS / footprint;
        for z in lo..hi {
            for x in lo..hi {
                let h = heights[x + z * 64];
                let cbi = cols[x + z * 64];
                let b = cbi.def();

                if b.tree_scale <= 0.0 || h <= SEA_LEVEL + 2 || h > cbi.tree_line {
                    continue;
                }
                let (wx, wz) = (o.x + x as i32 * s + s / 2, o.z + z as i32 * s + s / 2);
                let f = self.forest.get_noise_2d(wx as f32, wz as f32);
                let p =
                    ((0.0015 + 0.012 * f.max(0.0)) * b.tree_scale * density).min(1.0) * 1_000_000.0;
                if hash2(wx, wz, self.seed) % 1_000_000 >= p as u32 {
                    continue;
                }

                let ly = (h - 1 - o.y).div_euclid(s);
                if ly < 0 || ly as usize + trunk + canopy >= 64 {
                    continue;
                }
                let ly = ly as usize;
                if out[dense_index(x, ly, z)] != b.surface {
                    continue;
                }
                for dy in 1..=trunk {
                    out[dense_index(x, ly + dy, z)] = LOG;
                }
                for dy in trunk + 1..=trunk + canopy {
                    for dz in -r..=r {
                        for dx in -r..=r {
                            let i = dense_index(
                                (x as i32 + dx) as usize,
                                ly + dy,
                                (z as i32 + dz) as usize,
                            );
                            if out[i] == AIR {

                                let keeps = !self.tree_blue_noise
                                    || canopy_rank3(
                                        o.x + (x as i32 + dx) * s,
                                        o.y + (ly + dy) as i32 * s,
                                        o.z + (z as i32 + dz) * s,
                                        self.seed,
                                    ) < COARSE_CANOPY_KEEP;
                                if keeps {
                                    out[i] = b.leaf;
                                } else {

                                    thinned[i >> 6] |= 1u64 << (i & 63);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
