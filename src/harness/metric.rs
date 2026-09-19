use std::path::Path;

#[derive(Clone)]
pub struct Img {
    pub w: u32,
    pub h: u32,

    pub px: Vec<u8>,
}

impl Img {
    pub fn new(w: u32, h: u32) -> Img {
        Img {
            w,
            h,
            px: vec![0; (w as usize) * (h as usize) * 4],
        }
    }

    pub fn load(path: &Path) -> Result<Img, String> {
        let img = image::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let rgba = img.to_rgba8();
        Ok(Img {
            w: rgba.width(),
            h: rgba.height(),
            px: rgba.into_raw(),
        })
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        image::save_buffer(
            path,
            &self.px,
            self.w,
            self.h,
            image::ColorType::Rgba8,
        )
        .map_err(|e| format!("{}: {e}", path.display()))
    }

    #[inline]
    pub fn get(&self, x: u32, y: u32, c: usize) -> u8 {
        let x = x.min(self.w - 1) as usize;
        let y = y.min(self.h - 1) as usize;
        self.px[(y * self.w as usize + x) * 4 + c]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}

impl Rect {
    pub fn full(w: u32, h: u32) -> Rect {
        Rect {
            x0: 0,
            y0: 0,
            x1: w,
            y1: h,
        }
    }

    pub fn pixels(&self) -> u64 {
        (self.x1.saturating_sub(self.x0) as u64) * (self.y1.saturating_sub(self.y0) as u64)
    }

    pub fn is_empty(&self) -> bool {
        self.x1 <= self.x0 || self.y1 <= self.y0
    }

    fn clamped(&self, img: &Img) -> Rect {
        Rect {
            x0: self.x0.min(img.w),
            y0: self.y0.min(img.h),
            x1: self.x1.min(img.w),
            y1: self.y1.min(img.h),
        }
    }
}

impl std::fmt::Display for Rect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}x{}+{}+{}",
            self.x1 - self.x0,
            self.y1 - self.y0,
            self.x0,
            self.y0
        )
    }
}

pub fn mae(a: &Img, b: &Img, r: Rect) -> f64 {
    let r = r.clamped(a);
    let mut sum = 0.0f64;
    for y in r.y0..r.y1 {
        for x in r.x0..r.x1 {
            for c in 0..3 {
                sum += (a.get(x, y, c) as f64 - b.get(x, y, c) as f64).abs();
            }
        }
    }
    let n = r.pixels().max(1) as f64 * 3.0;
    sum / n
}

pub fn max_delta(a: &Img, b: &Img, r: Rect) -> u32 {
    let r = r.clamped(a);
    let mut m = 0u32;
    for y in r.y0..r.y1 {
        for x in r.x0..r.x1 {
            for c in 0..3 {
                let d = (a.get(x, y, c) as i32 - b.get(x, y, c) as i32).unsigned_abs();
                m = m.max(d);
            }
        }
    }
    m
}

pub fn diff_bbox(a: &Img, b: &Img) -> Option<Rect> {
    if a.w != b.w || a.h != b.h {
        return None;
    }
    let mut lo = [u32::MAX; 2];
    let mut hi = [0u32; 2];
    for y in 0..a.h {
        for x in 0..a.w {
            if (0..3).any(|c| a.get(x, y, c) != b.get(x, y, c)) {
                lo[0] = lo[0].min(x);
                lo[1] = lo[1].min(y);
                hi[0] = hi[0].max(x + 1);
                hi[1] = hi[1].max(y + 1);
            }
        }
    }
    (hi[0] > 0).then(|| Rect {
        x0: lo[0],
        y0: lo[1],
        x1: hi[0],
        y1: hi[1],
    })
}

pub fn pixels_differing(a: &Img, b: &Img, r: Rect) -> u64 {
    let r = r.clamped(a);
    let mut n = 0u64;
    for y in r.y0..r.y1 {
        for x in r.x0..r.x1 {
            if (0..3).any(|c| a.get(x, y, c) != b.get(x, y, c)) {
                n += 1;
            }
        }
    }
    n
}

pub fn speckle(a: &Img, r: Rect) -> f64 {
    let r = r.clamped(a);
    let mut sum = 0.0f64;
    for y in r.y0..r.y1 {
        for x in r.x0..r.x1 {
            for c in 0..3 {
                let mut m = 0.0f64;
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let sx = (x as i32 + dx).clamp(0, a.w as i32 - 1) as u32;
                        let sy = (y as i32 + dy).clamp(0, a.h as i32 - 1) as u32;
                        m += a.get(sx, sy, c) as f64;
                    }
                }
                sum += (a.get(x, y, c) as f64 - m / 9.0).abs();
            }
        }
    }
    sum / (r.pixels().max(1) as f64 * 3.0)
}

pub fn rg_std(a: &Img, r: Rect) -> f64 {
    let r = r.clamped(a);
    let n = r.pixels().max(1) as f64;
    let mut sum = 0.0f64;
    let mut sum2 = 0.0f64;
    for y in r.y0..r.y1 {
        for x in r.x0..r.x1 {
            let d = a.get(x, y, 0) as f64 - a.get(x, y, 1) as f64;
            sum += d;
            sum2 += d * d;
        }
    }
    (sum2 / n - (sum / n) * (sum / n)).max(0.0).sqrt()
}

pub fn rg_speckle(a: &Img, r: Rect) -> f64 {
    let r = r.clamped(a);
    let mut sum = 0.0f64;
    for y in r.y0..r.y1 {
        for x in r.x0..r.x1 {
            let mut m = 0.0f64;
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let sx = (x as i32 + dx).clamp(0, a.w as i32 - 1) as u32;
                    let sy = (y as i32 + dy).clamp(0, a.h as i32 - 1) as u32;
                    m += a.get(sx, sy, 0) as f64 - a.get(sx, sy, 1) as f64;
                }
            }
            let d = a.get(x, y, 0) as f64 - a.get(x, y, 1) as f64;
            sum += (d - m / 9.0).abs();
        }
    }
    sum / r.pixels().max(1) as f64
}

pub const COH_HIGHPASS: u32 = 3;

pub const COH_TILE: u32 = 64;

pub const COH_LAG_MIN: u32 = 6;

pub const COH_LAG_MAX: u32 = 32;

#[derive(Clone, Copy, Debug)]
pub struct Coherence {

    pub peak: f64,

    pub lag: (i32, i32),

    pub tiles: u32,
}

pub fn coherence(a: &Img, r: Rect) -> Coherence {
    coherence_with(a, r, COH_PARAMS)
}

#[derive(Clone, Copy, Debug)]
pub struct CohParams {
    pub highpass: u32,
    pub tile: u32,
    pub lag_min: u32,
    pub lag_max: u32,
}

pub const COH_PARAMS: CohParams = CohParams {
    highpass: COH_HIGHPASS,
    tile: COH_TILE,
    lag_min: COH_LAG_MIN,
    lag_max: COH_LAG_MAX,
};

pub fn coherence_with(a: &Img, r: Rect, p: CohParams) -> Coherence {
    let tiles = coherence_tiles_with(a, r, p);
    let n = tiles.len() as u32;
    let best = tiles
        .iter()
        .copied()
        .fold((0.0f64, (0, 0)), |b, t| if t.0 > b.0 { t } else { b });
    Coherence {
        peak: summarise_tiles(&tiles),
        lag: best.1,
        tiles: n,
    }
}
pub fn coherence_tiles_with(a: &Img, r: Rect, p: CohParams) -> Vec<(f64, (i32, i32))> {
    let s = scale(a);
    let lo = lag_px(p.lag_min, s);
    let hi = lag_px(p.lag_max, s);
    let side = lag_px(p.tile, s);
    let field = Field::new(a, odd_win(p.highpass, s));
    let r = r.clamped(a);
    let mut out: Vec<(f64, (i32, i32))> = Vec::new();
    let mut y = r.y0 as i32;
    while y + side <= r.y1 as i32 {
        let mut x = r.x0 as i32;
        while x + side <= r.x1 as i32 {
            let tile = Rect {
                x0: x as u32,
                y0: y as u32,
                x1: (x + side) as u32,
                y1: (y + side) as u32,
            };
            let mut peak = 0.0f64;
            let mut at = (0, 0);

            for dy in 0..=hi {
                for dx in -hi..=hi {
                    if dy == 0 && dx <= 0 {
                        continue;
                    }
                    let d2 = dx * dx + dy * dy;
                    if d2 < lo * lo || d2 > hi * hi {
                        continue;
                    }
                    let rho = field.rho(tile, dx, dy);
                    if rho > peak {
                        peak = rho;
                        at = (dx, dy);
                    }
                }
            }
            out.push((peak, at));
            x += side / 2;
        }
        y += side / 2;
    }
    out
}

fn summarise_tiles(tiles: &[(f64, (i32, i32))]) -> f64 {
    if tiles.is_empty() {
        return 0.0;
    }
    tiles.iter().map(|t| t.0).sum::<f64>() / tiles.len() as f64
}
fn scale(a: &Img) -> f64 {
    a.h as f64 / super::vantage::STD_HEIGHT as f64
}

fn lag_px(v: u32, s: f64) -> i32 {
    ((v as f64 * s).round() as i32).max(1)
}
fn odd_win(v: u32, s: f64) -> u32 {
    ((v as f64 * s).round() as u32).max(3) | 1
}

struct Field {
    w: i32,
    f: Vec<f64>,

    sat: Vec<f64>,
    sat2: Vec<f64>,
}

impl Field {
    fn new(a: &Img, win: u32) -> Field {
        let (w, h) = (a.w as i32, a.h as i32);

        let mut isum = vec![0u64; ((w + 1) * (h + 1)) as usize];
        for y in 0..h {
            let mut row = 0u64;
            for x in 0..w {
                let p = ((y * w + x) * 4) as usize;
                row += (a.px[p] as u64) + (a.px[p + 1] as u64) + (a.px[p + 2] as u64);
                isum[((y + 1) * (w + 1) + x + 1) as usize] =
                    isum[(y * (w + 1) + x + 1) as usize] + row;
            }
        }
        let rad = (win / 2) as i32;
        let mut f = vec![0.0f64; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let x0 = (x - rad).max(0);
                let y0 = (y - rad).max(0);
                let x1 = (x + rad + 1).min(w);
                let y1 = (y + rad + 1).min(h);
                let s = isum[(y1 * (w + 1) + x1) as usize] + isum[(y0 * (w + 1) + x0) as usize]
                    - isum[(y0 * (w + 1) + x1) as usize]
                    - isum[(y1 * (w + 1) + x0) as usize];
                let n = ((x1 - x0) * (y1 - y0)) as f64 * 3.0;
                let p = ((y * w + x) * 4) as usize;
                let v = (a.px[p] as f64 + a.px[p + 1] as f64 + a.px[p + 2] as f64) / 3.0;
                f[(y * w + x) as usize] = v - s as f64 / n;
            }
        }
        let mut sat = vec![0.0f64; ((w + 1) * (h + 1)) as usize];
        let mut sat2 = vec![0.0f64; ((w + 1) * (h + 1)) as usize];
        for y in 0..h {
            let (mut r1, mut r2) = (0.0f64, 0.0f64);
            for x in 0..w {
                let v = f[(y * w + x) as usize];
                r1 += v;
                r2 += v * v;
                let i = ((y + 1) * (w + 1) + x + 1) as usize;
                sat[i] = sat[(y * (w + 1) + x + 1) as usize] + r1;
                sat2[i] = sat2[(y * (w + 1) + x + 1) as usize] + r2;
            }
        }
        Field {
            w,
            f,
            sat,
            sat2,
        }
    }

    fn box_sum(&self, s: &[f64], x0: i32, y0: i32, x1: i32, y1: i32) -> f64 {
        let i = |x: i32, y: i32| s[(y * (self.w + 1) + x) as usize];
        i(x1, y1) + i(x0, y0) - i(x0, y1) - i(x1, y0)
    }

    fn rho(&self, r: Rect, dx: i32, dy: i32) -> f64 {
        let x0 = (r.x0 as i32).max(r.x0 as i32 - dx);
        let y0 = (r.y0 as i32).max(r.y0 as i32 - dy);
        let x1 = (r.x1 as i32).min(r.x1 as i32 - dx);
        let y1 = (r.y1 as i32).min(r.y1 as i32 - dy);
        if x1 <= x0 || y1 <= y0 {
            return 0.0;
        }
        let n = ((x1 - x0) * (y1 - y0)) as f64;
        let sx = self.box_sum(&self.sat, x0, y0, x1, y1);
        let sxx = self.box_sum(&self.sat2, x0, y0, x1, y1);
        let sy = self.box_sum(&self.sat, x0 + dx, y0 + dy, x1 + dx, y1 + dy);
        let syy = self.box_sum(&self.sat2, x0 + dx, y0 + dy, x1 + dx, y1 + dy);
        let mut sxy = 0.0f64;
        for y in y0..y1 {
            let a = (y * self.w + x0) as usize;
            let b = ((y + dy) * self.w + x0 + dx) as usize;
            for i in 0..(x1 - x0) as usize {
                sxy += self.f[a + i] * self.f[b + i];
            }
        }
        let cov = sxy - sx * sy / n;
        let vx = sxx - sx * sx / n;
        let vy = syy - sy * sy / n;

        if vx <= 1e-9 || vy <= 1e-9 {
            return 0.0;
        }
        cov / (vx * vy).sqrt()
    }
}

pub fn srgb_to_linear(v: u8) -> f32 {
    let c = v as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub fn linear_to_srgb(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let v = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (v * 255.0 + 0.5).clamp(0.0, 255.0) as u8
}

pub fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let mut msg = data.to_vec();
    let bits = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_be_bytes());

    for block in msg.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d) = (h[0], h[1], h[2], h[3]);
        let (mut e, mut f, mut g, mut hh) = (h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    h.iter().map(|v| format!("{v:08x}")).collect()
}
