use crate::block::tex;

pub const TEX_SIZE: u32 = 16;
pub const MIP_LEVELS: u32 = 4;

pub const WATER_MOTTLE_PRE21B: f32 = 0.20;

#[inline]
fn hash(x: u32, y: u32, seed: u32) -> f32 {
    let mut h =
        x.wrapping_mul(0x8da6b343) ^ y.wrapping_mul(0xd8163841) ^ seed.wrapping_mul(0xcb1ab31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1e995);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65535.0
}

fn px(c: [f32; 3], v: f32) -> [u8; 4] {
    let f = |x: f32| ((x * v).clamp(0.0, 1.0) * 255.0) as u8;
    [f(c[0]), f(c[1]), f(c[2]), 0]
}

fn pxt(c: [f32; 3], v: f32) -> [u8; 4] {
    let mut p = px(c, v);
    p[3] = 255;
    p
}

const GRASS: [f32; 3] = [0.36, 0.62, 0.24];

const MEADOW_SHADE: f32 = 0.59;

fn blob(x: u32, y: u32, seed: u32) -> f32 {
    let mut acc = 0.0;
    for (dx, dy) in [(0i32, 0i32), (1, 0), (0, 1), (-1, 0), (0, -1)] {
        let xx = ((x as i32 + dx).rem_euclid(16)) as u32;
        let yy = ((y as i32 + dy).rem_euclid(16)) as u32;
        acc += hash(xx / 2, yy / 2, seed);
    }
    acc / 5.0
}

fn lamp(x: u32, y: u32, n: f32, seed: u32, core: [f32; 3], body: [f32; 3]) -> [u8; 4] {
    if blob(x, y, seed) > 0.55 {
        px(core, 1.0)
    } else {
        px(body, 0.85 + 0.2 * n)
    }
}

fn texel(layer: u32, x: u32, y: u32, water_mottle: f32, tint_balance: bool) -> [u8; 4] {
    let n = hash(x, y, layer * 31 + 7);
    match layer {
        tex::STONE => px([0.50, 0.50, 0.50], 0.78 + 0.28 * blob(x, y, 1)),
        tex::DIRT => px([0.53, 0.38, 0.26], 0.75 + 0.35 * n),
        tex::GRASS_TOP => pxt(GRASS, 0.78 + 0.32 * n),
        tex::GRASS_SIDE => {
            let edge = 3 + (hash(x, 0, 99) * 2.5) as u32;
            if y < edge {

                pxt(GRASS, 0.78 + 0.32 * n)
            } else {
                px([0.53, 0.38, 0.26], 0.75 + 0.35 * n)
            }
        }

        tex::SAND => {
            let p = px([0.86, 0.80, 0.58], 0.86 + 0.16 * n);
            if tint_balance { pxt([p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0], 1.0) } else { p }
        }
        tex::GRAVEL => px([0.48, 0.46, 0.44], 0.65 + 0.5 * blob(x, y, 5)),
        tex::LOG_SIDE => {
            let stripe = 0.8 + 0.2 * ((x as f32 * 1.7).sin() * 0.5 + 0.5) + 0.1 * n;
            px([0.42, 0.31, 0.18], stripe)
        }
        tex::LOG_TOP => {
            let cx = x as f32 - 7.5;
            let cy = y as f32 - 7.5;
            let r = (cx * cx + cy * cy).sqrt();
            let ring = 0.75 + 0.25 * ((r * 1.9).sin() * 0.5 + 0.5) + 0.05 * n;
            if r > 7.2 {
                px([0.42, 0.31, 0.18], 0.9)
            } else {
                px([0.72, 0.58, 0.36], ring)
            }
        }
        tex::LEAVES => {
            let dark = hash(x, y, 4) < 0.35;
            if dark {
                pxt([0.16, 0.36, 0.12], 0.9 + 0.2 * n)
            } else {
                pxt([0.24, 0.52, 0.18], 0.85 + 0.3 * n)
            }
        }
        tex::PLANKS => {
            let plank = (y / 4) % 2;
            let seam = y.is_multiple_of(4) || (x == (plank * 8 + 3) % 16);
            if seam {
                px([0.55, 0.42, 0.25], 0.6)
            } else {
                px([0.72, 0.56, 0.33], 0.88 + 0.2 * n)
            }
        }
        tex::COBBLE => {
            let b = blob(x, y, 9);
            let seam = b < 0.36;
            if seam {
                px([0.30, 0.30, 0.30], 0.9)
            } else {
                px([0.56, 0.56, 0.56], 0.75 + 0.35 * b)
            }
        }
        tex::GLASS => {
            let border = x == 0 || y == 0 || x == 15 || y == 15;
            let streak = (x + y).is_multiple_of(9);
            if border {
                px([0.80, 0.90, 0.95], 0.9)
            } else if streak {
                px([0.90, 0.96, 1.0], 0.95)
            } else {
                px([0.70, 0.85, 0.92], 0.95)
            }
        }

        tex::GLOWSTONE => lamp(x, y, n, 13, [1.0, 0.95, 0.65], [0.80, 0.62, 0.30]),
        tex::BEDROCK => px([0.30, 0.30, 0.32], 0.5 + 0.9 * blob(x, y, 21)),

        tex::WATER => px(
            [0.10, 0.34, 0.42],
            (1.0 - 0.5 * water_mottle) + water_mottle * blob(x, y, 33),
        ),

        tex::SNOW => px([0.88, 0.91, 0.97], 0.94 + 0.10 * blob(x, y, 41)),
        tex::PODZOL => px([0.34, 0.23, 0.13], 0.78 + 0.34 * n),

        tex::PINE_LEAVES => {
            let dark = hash(x, y, 6) < 0.40;
            if dark {
                pxt([0.09, 0.24, 0.17], 0.9 + 0.2 * n)
            } else {
                pxt([0.15, 0.36, 0.24], 0.85 + 0.3 * n)
            }
        }

        tex::LICHEN => {
            let stone = hash(x, y, 8) < 0.30;
            if stone {
                px([0.47, 0.47, 0.44], 0.82 + 0.3 * n)
            } else {

                let p = px([0.45, 0.50, 0.38], 0.80 + 0.32 * blob(x, y, 17));
                if tint_balance { pxt([p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0], 1.0) } else { p }
            }
        }

        tex::TALL_GRASS => {

            const BLADES: [(f32, f32, f32); 6] = [

                (1.5, 0.4, 12.0),
                (4.0, 5.2, 15.0),
                (6.5, 5.8, 9.0),
                (8.5, 9.9, 14.0),
                (11.0, 10.2, 11.0),
                (13.5, 14.6, 13.0),
            ];
            let fx = x as f32 + 0.5;

            let ry = (TEX_SIZE - 1 - y) as f32;
            let mut tip_frac = -1.0f32;
            for &(root, tip, h) in BLADES.iter() {
                if ry > h {
                    continue;
                }
                let s = ry / h;

                let cx = root + (tip - root) * s * s;
                let half = 1.15 - 0.85 * s;
                if (fx - cx).abs() <= half {
                    tip_frac = tip_frac.max(s);
                }
            }
            if tip_frac < 0.0 {
                [0, 0, 0, 0]
            } else {

                let c = [
                    0.13 + 0.12 * tip_frac,
                    0.30 + 0.30 * tip_frac,
                    0.10 + 0.06 * tip_frac,
                ];
                let mut p = px(c, 0.85 + 0.3 * n);
                p[3] = 255;
                p
            }
        }

        tex::MEADOW_TOP => pxt(GRASS, (0.78 + 0.32 * n) * MEADOW_SHADE),

        tex::MEADOW_SIDE => {
            let edge = 3 + (hash(x, 0, 99) * 2.5) as u32;
            if y < edge {
                pxt(GRASS, (0.78 + 0.32 * n) * MEADOW_SHADE)
            } else {
                px([0.53, 0.38, 0.26], (0.75 + 0.35 * n) * MEADOW_SHADE)
            }
        }

        tex::GRASS_TALL => {
            const BLADES: [(f32, f32, f32); 7] = [

                (1.0, 0.2, 15.0),
                (3.0, 4.4, 16.0),
                (5.5, 4.9, 13.0),
                (7.5, 8.6, 15.0),
                (9.5, 8.9, 14.0),
                (11.5, 12.8, 16.0),
                (14.0, 13.2, 14.0),
            ];
            let fx = x as f32 + 0.5;
            let ry = (TEX_SIZE - 1 - y) as f32;
            let mut tip_frac = -1.0f32;
            for &(root, tip, h) in BLADES.iter() {
                if ry > h {
                    continue;
                }
                let s = ry / h;
                let cx = root + (tip - root) * s * s;
                let half = 1.2 - 0.8 * s;
                if (fx - cx).abs() <= half {
                    tip_frac = tip_frac.max(s);
                }
            }
            if tip_frac < 0.0 {
                [0, 0, 0, 0]
            } else {
                let c = [
                    0.14 + 0.30 * tip_frac,
                    0.33 + 0.26 * tip_frac,
                    0.09 + 0.07 * tip_frac,
                ];
                let mut p = px(c, 0.82 + 0.3 * n);
                p[3] = 255;
                p
            }
        }

        tex::REEDS => {
            const STALKS: [(f32, f32, f32); 5] = [

                (2.0, 2.7, 16.0),
                (5.0, 4.6, 15.0),
                (7.8, 8.4, 16.0),
                (10.6, 10.1, 14.0),
                (13.6, 14.1, 16.0),
            ];
            let fx = x as f32 + 0.5;
            let ry = (TEX_SIZE - 1 - y) as f32;
            let mut tip_frac = -1.0f32;
            for &(root, tip, h) in STALKS.iter() {
                if ry > h {
                    continue;
                }
                let s = ry / h;
                let cx = root + (tip - root) * s;
                let half = 0.85 - 0.25 * s;
                if (fx - cx).abs() <= half {
                    tip_frac = tip_frac.max(s);
                }
            }
            if tip_frac < 0.0 {
                [0, 0, 0, 0]
            } else {
                let c = [
                    0.20 + 0.16 * tip_frac,
                    0.28 + 0.16 * tip_frac,
                    0.10 + 0.06 * tip_frac,
                ];
                let mut p = px(c, 0.80 + 0.32 * n);
                p[3] = 255;
                p
            }
        }

        tex::AMBER_LAMP => lamp(x, y, n, 41, [1.0, 0.78, 0.42], [0.74, 0.34, 0.10]),
        tex::AZURE_LAMP => lamp(x, y, n, 43, [0.68, 0.88, 1.0], [0.14, 0.34, 0.80]),
        tex::VERDANT_LAMP => lamp(x, y, n, 47, [0.74, 1.0, 0.58], [0.18, 0.62, 0.20]),
        _ => [255, 0, 255, 0],
    }
}

pub fn build_atlas(water_mottle: f32, tint_balance: bool) -> Vec<u8> {
    let mut v = Vec::with_capacity((tex::COUNT * TEX_SIZE * TEX_SIZE * 4) as usize);
    for layer in 0..tex::COUNT {
        for y in 0..TEX_SIZE {
            for x in 0..TEX_SIZE {
                v.extend_from_slice(&texel(layer, x, y, water_mottle, tint_balance));
            }
        }
    }
    v
}

fn srgb_to_linear(u: u8) -> f32 {
    let c = u as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> u8 {
    let s = if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

pub fn downsample(src: &[u8], size: u32, layers: u32) -> Vec<u8> {
    let half = size / 2;
    let mut out = vec![0u8; (layers * half * half * 4) as usize];
    for l in 0..layers {
        for y in 0..half {
            for x in 0..half {
                let texel = |c: usize| {
                    let mut acc = 0.0f32;
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let sx = x * 2 + dx;
                        let sy = y * 2 + dy;
                        let v = src[((l * size * size + sy * size + sx) * 4) as usize + c];
                        acc += if c == 3 {
                            v as f32 / 255.0
                        } else {
                            srgb_to_linear(v)
                        };
                    }
                    acc / 4.0
                };
                let o = ((l * half * half + y * half + x) * 4) as usize;
                for c in 0..3usize {
                    out[o + c] = linear_to_srgb(texel(c));
                }
                out[o + 3] = (texel(3).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            }
        }
    }
    out
}
