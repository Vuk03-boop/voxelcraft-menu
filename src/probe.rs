use crate::block::{self, BlockId};
use crate::voxel::chunk::ChunkKey;
use crate::worldgen::WorldGen;
use glam::{IVec3, Vec3};

pub const SPACING: i32 = 4;

pub const PER_AXIS: usize = 64 / SPACING as usize;

pub const AXES: usize = 6;

pub const PROBES: usize = PER_AXIS * PER_AXIS * PER_AXIS;

const REACH: f32 = 192.0;

const STEPS: usize = 20;

pub const AZIMUTHS: usize = 16;

const SKY_BLEND: f32 = 0.25;

const ALBEDO_STRIDE: usize = 2;

pub const FLOOR_TINT: [f32; 3] = [0.85, 0.90, 1.00];

const BOUNCE_REF: BlockId = block::GRASS;

pub const UPLOAD_REACH: f32 = 224.0;

pub fn uploadable(origin: IVec3, cam: Vec3) -> bool {
    let dx = origin.x as f32 - cam.x;
    let dz = origin.z as f32 - cam.z;
    dx * dx + dz * dz <= UPLOAD_REACH * UPLOAD_REACH
}

pub struct ProbeData {
    pub occl: Vec<f32>,

    pub bounce: Vec<f32>,
}

impl ProbeData {
    #[inline]
    pub fn get(&self, axis: usize, px: usize, py: usize, pz: usize) -> f32 {
        self.occl[axis * PROBES + (pz * PER_AXIS + py) * PER_AXIS + px]
    }

    #[inline]
    pub fn get_bounce(&self, axis: usize, px: usize, py: usize, pz: usize) -> [f32; 3] {
        let o = (axis * PROBES + (pz * PER_AXIS + py) * PER_AXIS + px) * 3;
        [self.bounce[o], self.bounce[o + 1], self.bounce[o + 2]]
    }
}

pub struct BakeCache {
    map: rustc_hash::FxHashMap<ChunkKey, (u64, ProbeData)>,
    tick: u64,
    cap: usize,
}

impl Default for BakeCache {
    fn default() -> Self {
        Self {
            map: rustc_hash::FxHashMap::default(),
            tick: 0,
            cap: 384,
        }
    }
}

impl BakeCache {

    pub fn get(&mut self, key: &ChunkKey) -> Option<ProbeData> {
        self.tick += 1;
        let (t, d) = self.map.get_mut(key)?;
        *t = self.tick;
        Some(ProbeData {
            occl: d.occl.clone(),
            bounce: d.bounce.clone(),
        })
    }

    pub fn insert(&mut self, key: ChunkKey, d: ProbeData) {
        self.tick += 1;
        if self.map.len() >= self.cap {

            let oldest = *self
                .map
                .iter()
                .min_by_key(|(_, (t, _))| *t)
                .expect("nonempty")
                .0;
            self.map.remove(&oldest);
        }
        self.map.insert(key, (self.tick, d));
    }
}

pub fn cube_from_tangents(tan_h: &[f32; AZIMUTHS]) -> [f32; AXES] {
    let (_, lobe, lobe_sum) = tables();
    cube_from_horizon(tan_h, &lobe, &lobe_sum).0
}

#[allow(clippy::type_complexity)]
fn tables() -> (
    [(f32, f32); AZIMUTHS],
    [[f32; AZIMUTHS]; AXES],
    [f32; AXES],
) {
    let mut dir = [(0.0f32, 0.0f32); AZIMUTHS];
    for (k, d) in dir.iter_mut().enumerate() {
        let a = std::f32::consts::TAU * k as f32 / AZIMUTHS as f32;
        *d = (a.cos(), a.sin());
    }
    let mut lobe = [[0.0f32; AZIMUTHS]; AXES];
    let mut lobe_sum = [0.0f32; AXES];
    for (axis, ax_lobe) in lobe.iter_mut().enumerate() {

        let (ax, az) = match axis {
            0 => (-1.0f32, 0.0f32),
            1 => (1.0, 0.0),
            4 => (0.0, -1.0),
            5 => (0.0, 1.0),
            _ => (0.0, 0.0),
        };
        for (k, w) in ax_lobe.iter_mut().enumerate() {
            *w = (dir[k].0 * ax + dir[k].1 * az).max(0.0);
            lobe_sum[axis] += *w;
        }
    }
    (dir, lobe, lobe_sum)
}

pub fn bake(
    gen: &WorldGen,
    heights: &[i32; 64 * 64],
    origin: IVec3,
    bounce_shadow: bool,
) -> ProbeData {
    let mut occl = vec![0.0f32; AXES * PROBES];
    let mut bounce = vec![1.0f32; AXES * PROBES * 3];

    let (dir, lobe, lobe_sum) = tables();
    let blobe = bounce_lobes(&lobe);

    let scale = bounce_scale();
    let mut radius = [0.0f32; STEPS];
    let ratio = (REACH / 1.0f32).powf(1.0 / (STEPS - 1) as f32);
    let mut r = 1.0f32;
    for rad in radius.iter_mut() {
        *rad = r;
        r *= ratio;
    }

    let mut tan_h = [[0.0f32; AZIMUTHS]; PER_AXIS];
    let mut alb = [[[0.0f32; 3]; AZIMUTHS]; PER_AXIS];
    let mut alb_w = [[0.0f32; AZIMUTHS]; PER_AXIS];

    let mut col_h = [0.0f32; STEPS];
    let mut col_a = [[0.0f32; 3]; STEPS];
    for pz in 0..PER_AXIS {
        for px in 0..PER_AXIS {
            let cx = origin.x + SPACING / 2 + SPACING * px as i32;
            let cz = origin.z + SPACING / 2 + SPACING * pz as i32;
            for row in tan_h.iter_mut() {
                row.fill(0.0);
            }
            for row in alb.iter_mut() {
                row.fill([0.0; 3]);
            }
            for row in alb_w.iter_mut() {
                row.fill(0.0);
            }
            for (k, &(dx, dz)) in dir.iter().enumerate() {

                for (si, &r) in radius.iter().enumerate() {
                    let wx = cx + (dx * r).round() as i32;
                    let wz = cz + (dz * r).round() as i32;
                    let hi = height_at(gen, heights, origin, wx, wz);
                    col_h[si] = hi as f32;

                    col_a[si] = if si % ALBEDO_STRIDE == 0 {
                        block::ALBEDO[gen.surface_block_at(wx, wz, hi) as usize]
                    } else {
                        [0.0; 3]
                    };
                }

                let lit = if bounce_shadow {
                    ray_exposure(&col_h, &radius)
                } else {
                    [1.0f32; STEPS]
                };
                for (si, &r) in radius.iter().enumerate() {
                    let h = col_h[si];
                    let gathering = si % ALBEDO_STRIDE == 0;

                    let a = [
                        col_a[si][0] * lit[si],
                        col_a[si][1] * lit[si],
                        col_a[si][2] * lit[si],
                    ];

                    for py in 0..PER_AXIS {
                        let y = (origin.y + SPACING / 2 + SPACING * py as i32) as f32;
                        let t = (h - y) / r;
                        if t > tan_h[py][k] {
                            tan_h[py][k] = t;
                        }
                        if !gathering {
                            continue;
                        }

                        let d = (y - h).abs().max(1.0);
                        let dr = d * r;
                        let s = d * d + r * r;
                        let w = (dr * dr) / (s * s);
                        alb[py][k][0] += w * a[0];
                        alb[py][k][1] += w * a[1];
                        alb[py][k][2] += w * a[2];
                        alb_w[py][k] += w;
                    }
                }
            }

            for py in 0..PER_AXIS {
                let (cube, believe) = cube_from_horizon(&tan_h[py], &lobe, &lobe_sum);
                let b = bounce_from_albedo(&alb[py], &alb_w[py], &blobe, &scale, believe);
                let i = (pz * PER_AXIS + py) * PER_AXIS + px;
                for axis in 0..AXES {
                    occl[axis * PROBES + i] = cube[axis];
                    let o = (axis * PROBES + i) * 3;
                    bounce[o] = b[axis][0];
                    bounce[o + 1] = b[axis][1];
                    bounce[o + 2] = b[axis][2];
                }
            }
        }
    }

    ProbeData { occl, bounce }
}

fn bounce_scale() -> [f32; 3] {
    let gain = block::luma(block::ALBEDO[BOUNCE_REF as usize]);
    [
        block::luma(FLOOR_TINT) / (gain * FLOOR_TINT[0]),
        block::luma(FLOOR_TINT) / (gain * FLOOR_TINT[1]),
        block::luma(FLOOR_TINT) / (gain * FLOOR_TINT[2]),
    ]
}

pub fn bounce_of_uniform_ground(albedo: [f32; 3]) -> [[f32; 3]; AXES] {
    let (_, lobe, _) = tables();
    let blobe = bounce_lobes(&lobe);

    let alb = [albedo; AZIMUTHS];
    let w = [1.0f32; AZIMUTHS];
    bounce_from_albedo(&alb, &w, &blobe, &bounce_scale(), 1.0)
}

fn bounce_lobes(lobe: &[[f32; AZIMUTHS]; AXES]) -> [[f32; AZIMUTHS]; AXES] {
    let mut out = *lobe;
    out[2] = [1.0; AZIMUTHS];
    out[3] = [1.0; AZIMUTHS];
    out
}

fn bounce_from_albedo(
    alb: &[[f32; 3]; AZIMUTHS],
    alb_w: &[f32; AZIMUTHS],
    blobe: &[[f32; AZIMUTHS]; AXES],
    scale: &[f32; 3],
    believe: f32,
) -> [[f32; 3]; AXES] {
    let mut out = [[1.0f32; 3]; AXES];
    for (axis, o) in out.iter_mut().enumerate() {
        let mut num = [0.0f32; 3];
        let mut den = 0.0f32;
        for k in 0..AZIMUTHS {
            let w = blobe[axis][k];
            if w <= 0.0 {
                continue;
            }
            num[0] += w * alb[k][0];
            num[1] += w * alb[k][1];
            num[2] += w * alb[k][2];
            den += w * alb_w[k];
        }
        if den <= 0.0 {
            continue;
        }
        let inv = 1.0 / den;
        for c in 0..3 {
            o[c] = 1.0 + (num[c] * inv * scale[c] - 1.0) * believe;
        }
    }
    out
}

fn ray_exposure(col_h: &[f32; STEPS], radius: &[f32; STEPS]) -> [f32; STEPS] {
    let mut out = [1.0f32; STEPS];
    for si in 0..STEPS {

        if si % ALBEDO_STRIDE != 0 {
            continue;
        }
        let h = col_h[si];

        let (mut t_out, mut t_in) = (0.0f32, 0.0f32);
        for sj in 0..STEPS {
            if sj == si {
                continue;
            }

            let d = (radius[sj] - radius[si]).abs().max(1.0);
            let t = (col_h[sj] - h) / d;
            if sj > si {
                if t > t_out {
                    t_out = t;
                }
            } else if t > t_in {
                t_in = t;
            }
        }

        out[si] = 0.5 * (1.0 / (1.0 + t_out * t_out) + 1.0 / (1.0 + t_in * t_in));
    }
    out
}

#[inline]
fn height_at(gen: &WorldGen, heights: &[i32; 64 * 64], origin: IVec3, wx: i32, wz: i32) -> i32 {
    let lx = wx - origin.x;
    let lz = wz - origin.z;
    if (0..64).contains(&lx) && (0..64).contains(&lz) {
        heights[lx as usize + lz as usize * 64]
    } else {
        gen.height(wx, wz)
    }
}

fn cube_from_horizon(
    tan_h: &[f32; AZIMUTHS],
    lobe: &[[f32; AZIMUTHS]; AXES],
    lobe_sum: &[f32; AXES],
) -> ([f32; AXES], f32) {
    const QUARTER_PI: f32 = std::f32::consts::FRAC_PI_4;

    let mut up = 0.0f32;
    let mut elev = [0.0f32; AZIMUTHS];
    for (k, &t) in tan_h.iter().enumerate() {
        let t = t.max(0.0);
        up += 1.0 / (1.0 + t * t);
        elev[k] = QUARTER_PI - 0.5 * t.atan() - 0.5 * t / (1.0 + t * t);
    }
    up /= AZIMUTHS as f32;

    let t = (up / SKY_BLEND).clamp(0.0, 1.0);
    let believe = t * t * (3.0 - 2.0 * t);

    let mut out = [0.0f32; AXES];
    for (axis, o) in out.iter_mut().enumerate() {
        let occl = match axis {
            3 => up,
            2 => 1.0,
            _ => {
                let mut num = 0.0f32;
                for k in 0..AZIMUTHS {
                    num += lobe[axis][k] * elev[k];
                }
                num / (lobe_sum[axis] * QUARTER_PI)
            }
        };
        *o = 1.0 + (occl - 1.0) * believe;
    }

    (out, believe)
}
