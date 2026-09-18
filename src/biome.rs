use crate::block::*;

pub type BiomeId = u8;

pub const PLAINS: BiomeId = 0;
pub const FOREST: BiomeId = 1;
pub const SAVANNA: BiomeId = 2;
pub const DESERT: BiomeId = 3;
pub const TAIGA: BiomeId = 4;
pub const TUNDRA: BiomeId = 5;
pub const BIOME_COUNT: usize = 6;

#[derive(Clone, Copy, Debug)]
pub struct Biome {
    pub name: &'static str,

    pub surface: BlockId,

    pub subsurface: BlockId,

    pub leaf: BlockId,

    pub mountain_amp: f32,

    pub tree_scale: f32,

    pub grass_scale: f32,

    pub snow_line: i32,

    pub tree_line: i32,
}

#[rustfmt::skip]
pub const BIOMES: [Biome; BIOME_COUNT] = [
    Biome { name: "plains",  surface: GRASS, subsurface: DIRT,   leaf: LEAVES,      mountain_amp:  88.0, tree_scale: 1.00, grass_scale: 1.00, snow_line: 250, tree_line: 236 },
    Biome { name: "forest",  surface: GRASS, subsurface: DIRT,   leaf: LEAVES,      mountain_amp: 105.0, tree_scale: 1.80, grass_scale: 0.85, snow_line: 250, tree_line: 236 },
    Biome { name: "savanna", surface: GRASS, subsurface: DIRT,   leaf: LEAVES,      mountain_amp:  60.0, tree_scale: 0.50, grass_scale: 0.70, snow_line: 265, tree_line: 250 },

    Biome { name: "desert",  surface: SAND,  subsurface: SAND,   leaf: LEAVES,      mountain_amp:  45.0, tree_scale: 0.50, grass_scale: 0.00, snow_line: 300, tree_line: 300 },
    Biome { name: "taiga",   surface: GRASS, subsurface: PODZOL, leaf: PINE_LEAVES, mountain_amp: 170.0, tree_scale: 1.50, grass_scale: 0.45, snow_line: 205, tree_line: 176 },

    Biome { name: "tundra",  surface: LICHEN, subsurface: DIRT,  leaf: PINE_LEAVES, mountain_amp: 210.0, tree_scale: 0.00, grass_scale: 0.00, snow_line: 175, tree_line: 150 },
];

#[rustfmt::skip]
pub const GRID: [[BiomeId; 3]; 3] = [
    [TUNDRA, PLAINS, DESERT],
    [TAIGA, PLAINS, SAVANNA],
    [TAIGA, FOREST, FOREST],
];

pub const BAND: f32 = 0.22;

pub const BLEND: f32 = 0.18;

const _: () = assert!(BLEND < BAND);

pub const TEMP_FREQ: f32 = 0.00055;
pub const HUMID_FREQ: f32 = 0.0007;

pub const TEMP_SEED: i32 = 7;
pub const HUMID_SEED: i32 = 8;

#[inline]
pub fn def(id: BiomeId) -> &'static Biome {
    &BIOMES[(id as usize).min(BIOME_COUNT - 1)]
}

#[inline]
fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
pub fn band(n: f32) -> usize {
    if n < -BAND {
        0
    } else if n > BAND {
        2
    } else {
        1
    }
}

#[inline]
pub fn band_weights(n: f32) -> [f32; 3] {
    let hot = smoothstep(BAND - BLEND, BAND + BLEND, n);
    let cold = 1.0 - smoothstep(-BAND - BLEND, -BAND + BLEND, n);
    [cold, 1.0 - cold - hot, hot]
}

#[inline]
pub fn lookup(temperature: f32, humidity: f32) -> (BiomeId, f32, f32, f32) {
    let wt = band_weights(temperature);
    let wh = band_weights(humidity);
    let mut amp = 0.0;
    let mut snow = 0.0;
    let mut trees = 0.0;
    for (j, &wj) in wh.iter().enumerate() {
        if wj <= 0.0 {
            continue;
        }
        for (i, &wi) in wt.iter().enumerate() {
            let w = wj * wi;
            if w <= 0.0 {
                continue;
            }
            let b = def(GRID[j][i]);
            amp += w * b.mountain_amp;
            snow += w * b.snow_line as f32;
            trees += w * b.tree_line as f32;
        }
    }
    (GRID[band(humidity)][band(temperature)], amp, snow, trees)
}

const PRIME_X: i32 = 501125321;
const PRIME_Y: i32 = 1136930381;

#[allow(clippy::excessive_precision)]
#[rustfmt::skip]
const GRAD24: [f32; 48] = [
     0.130526192220052,  0.99144486137381,   0.38268343236509,   0.923879532511287,  0.608761429008721,  0.793353340291235,  0.793353340291235,  0.608761429008721,
     0.923879532511287,  0.38268343236509,   0.99144486137381,   0.130526192220051,  0.99144486137381,  -0.130526192220051,  0.923879532511287, -0.38268343236509,
     0.793353340291235, -0.60876142900872,   0.608761429008721, -0.793353340291235,  0.38268343236509,  -0.923879532511287,  0.130526192220052, -0.99144486137381,
    -0.130526192220052, -0.99144486137381,  -0.38268343236509,  -0.923879532511287, -0.608761429008721, -0.793353340291235, -0.793353340291235, -0.608761429008721,
    -0.923879532511287, -0.38268343236509,  -0.99144486137381,  -0.130526192220052, -0.99144486137381,   0.130526192220051, -0.923879532511287,  0.38268343236509,
    -0.793353340291235,  0.608761429008721, -0.608761429008721,  0.793353340291235, -0.38268343236509,   0.923879532511287, -0.130526192220052,  0.99144486137381,
];

#[allow(clippy::excessive_precision)]
#[rustfmt::skip]
const GRAD8: [f32; 16] = [
     0.38268343236509,   0.923879532511287,  0.923879532511287,  0.38268343236509,   0.923879532511287, -0.38268343236509,   0.38268343236509,  -0.923879532511287,
    -0.38268343236509,  -0.923879532511287, -0.923879532511287, -0.38268343236509,  -0.923879532511287,  0.38268343236509,  -0.38268343236509,   0.923879532511287,
];

#[inline]
fn fast_floor(f: f32) -> i32 {
    if f >= 0.0 {
        f as i32
    } else {
        f as i32 - 1
    }
}

#[inline]
fn grad(seed: i32, x_primed: i32, y_primed: i32, xd: f32, yd: f32) -> f32 {
    let mut h = seed ^ x_primed ^ y_primed;
    h = h.wrapping_mul(0x27d4eb2d);
    h ^= h >> 15;

    let idx = (h & (127 << 1)) as usize;
    let (xg, yg) = if idx < 240 {
        (GRAD24[idx % 48], GRAD24[(idx % 48) | 1])
    } else {
        (GRAD8[idx - 240], GRAD8[(idx - 240) | 1])
    };
    xd * xg + yd * yg
}

#[allow(clippy::excessive_precision)]
#[inline]
pub fn simplex2(seed: i32, freq: f32, x: f32, y: f32) -> f32 {
    let sqrt3: f32 = 1.7320508075688772935274463415059;
    let g2 = (3.0 - sqrt3) / 6.0;

    let f2 = 0.5 * (sqrt3 - 1.0);
    let mut x = x * freq;
    let mut y = y * freq;
    let s = (x + y) * f2;
    x += s;
    y += s;

    let i0 = fast_floor(x);
    let j0 = fast_floor(y);
    let xi = x - i0 as f32;
    let yi = y - j0 as f32;

    let t = (xi + yi) * g2;
    let x0 = xi - t;
    let y0 = yi - t;

    let i = i0.wrapping_mul(PRIME_X);
    let j = j0.wrapping_mul(PRIME_Y);

    let a = 0.5 - x0 * x0 - y0 * y0;
    let n0 = if a <= 0.0 {
        0.0
    } else {
        (a * a) * (a * a) * grad(seed, i, j, x0, y0)
    };

    let c = (2.0 * (1.0 - 2.0 * g2) * (1.0 / g2 - 2.0)) * t
        + ((-2.0 * (1.0 - 2.0 * g2) * (1.0 - 2.0 * g2)) + a);
    let n2 = if c <= 0.0 {
        0.0
    } else {
        let x2 = x0 + (2.0 * g2 - 1.0);
        let y2 = y0 + (2.0 * g2 - 1.0);
        (c * c)
            * (c * c)
            * grad(
                seed,
                i.wrapping_add(PRIME_X),
                j.wrapping_add(PRIME_Y),
                x2,
                y2,
            )
    };

    let n1 = if y0 > x0 {
        let x1 = x0 + g2;
        let y1 = y0 + (g2 - 1.0);
        let b = 0.5 - x1 * x1 - y1 * y1;
        if b <= 0.0 {
            0.0
        } else {
            (b * b) * (b * b) * grad(seed, i, j.wrapping_add(PRIME_Y), x1, y1)
        }
    } else {
        let x1 = x0 + (g2 - 1.0);
        let y1 = y0 + g2;
        let b = 0.5 - x1 * x1 - y1 * y1;
        if b <= 0.0 {
            0.0
        } else {
            (b * b) * (b * b) * grad(seed, i.wrapping_add(PRIME_X), j, x1, y1)
        }
    };

    (n0 + n1 + n2) * 99.83685446303647
}
