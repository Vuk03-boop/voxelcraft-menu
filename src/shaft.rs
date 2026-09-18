use crate::worldgen::WorldGen;

pub const DIM: usize = 512;

pub const TEXEL: i32 = 4;

pub const STEPS: u32 = 8;

pub const SOFT: f32 = 4.0;

const _: () = assert!((STEPS - 1) & 1 == 1);

pub const BUFFER_FLOATS: usize = 3 * DIM * DIM;

pub const HEIGHTS_BASE: usize = 2 * DIM * DIM;

pub struct ShaftField {
    heights: Vec<f32>,

    origin: [i32; 2],

    texel: i32,
    filled: bool,
    dirty: bool,
}

impl Default for ShaftField {
    fn default() -> Self {
        Self::new()
    }
}

impl ShaftField {
    pub fn new() -> Self {
        Self::with_texel(TEXEL)
    }

    pub fn with_texel(texel: i32) -> Self {
        assert!((1..=64).contains(&texel), "TEXEL past the scan's reach");
        Self {
            heights: vec![0.0; DIM * DIM],
            origin: [0, 0],
            texel,
            filled: false,
            dirty: false,
        }
    }

    pub fn texel(&self) -> i32 {
        self.texel
    }

    pub fn update(&mut self, gen: &WorldGen, cam_x: f32, cam_z: f32, canopy_lift: f32) {
        let half = (DIM / 2) as i32;
        let want = [
            (cam_x / self.texel as f32).floor() as i32 - half,
            (cam_z / self.texel as f32).floor() as i32 - half,
        ];
        self.dirty = false;
        if self.filled && want == self.origin {
            return;
        }
        let d = [want[0] - self.origin[0], want[1] - self.origin[1]];
        let mut next = vec![0.0f32; DIM * DIM];
        for zz in 0..DIM {
            let oz = zz as i32 + d[1];
            let row = zz * DIM;
            for xx in 0..DIM {
                let ox = xx as i32 + d[0];
                let carried = self.filled
                    && oz >= 0
                    && oz < DIM as i32
                    && ox >= 0
                    && ox < DIM as i32;
                next[row + xx] = if carried {
                    self.heights[oz as usize * DIM + ox as usize]
                } else {

                    let wx = (want[0] + xx as i32) * self.texel + self.texel / 2;
                    let wz = (want[1] + zz as i32) * self.texel + self.texel / 2;
                    let h = gen.height(wx, wz);

                    if canopy_lift > 0.0 {
                        let cbi = gen.column_biome(wx, wz);
                        if cbi.def().tree_scale > 0.0 && h > crate::worldgen::SEA_LEVEL + 2
                            && h <= cbi.tree_line
                        {
                            h as f32 + canopy_lift
                        } else {
                            h as f32
                        }
                    } else {
                        h as f32
                    }
                };
            }
        }
        self.heights = next;
        self.origin = want;
        self.filled = true;
        self.dirty = true;
    }

    pub fn origin_world(&self) -> [f32; 2] {
        [
            (self.origin[0] * self.texel) as f32,
            (self.origin[1] * self.texel) as f32,
        ]
    }

    pub fn heights(&self) -> &[f32] {
        &self.heights
    }

    pub fn dirty(&self) -> bool {
        self.dirty
    }

    pub fn envelope_reference(&self, sun: [f32; 3], tx: usize, tz: usize) -> f32 {
        let horiz = (sun[0] * sun[0] + sun[2] * sun[2]).sqrt().max(1e-3);
        let dir = [sun[0] / horiz, sun[2] / horiz];
        let slope = sun[1] / horiz;
        let mut best = self.heights[tz * DIM + tx];
        let reach = 1usize << STEPS;
        for m in 1..=reach {
            let sx = tx as f32 + dir[0] * m as f32;
            let sz = tz as f32 + dir[1] * m as f32;
            let h = self.sample(sx, sz);
            best = best.max(h - m as f32 * self.texel as f32 * slope);
        }
        best
    }

    fn sample(&self, x: f32, z: f32) -> f32 {
        let hi = (DIM - 1) as f32;
        let cx = x.clamp(0.0, hi);
        let cz = z.clamp(0.0, hi);
        let x0 = cx.floor();
        let z0 = cz.floor();
        let fx = cx - x0;
        let fz = cz - z0;
        let x0 = x0 as usize;
        let z0 = z0 as usize;
        let x1 = (x0 + 1).min(DIM - 1);
        let z1 = (z0 + 1).min(DIM - 1);
        let a = self.heights[z0 * DIM + x0];
        let b = self.heights[z0 * DIM + x1];
        let c = self.heights[z1 * DIM + x0];
        let d = self.heights[z1 * DIM + x1];
        let top = a + (b - a) * fx;
        let bot = c + (d - c) * fx;
        top + (bot - top) * fz
    }
}
