//! The biome table: one 3x3 grid of temperature x humidity, read two ways.
//!
//! **The palette snaps and the shape blends.** A column's biome — its surface block, its
//! subsurface, the leaf its trees wear — is a threshold lookup on the two noises, so it
//! changes at a line. The three numbers that describe the *shape* of the ground —
//! mountain amplitude, snow line, tree line — are a weighted sum over the same grid, so
//! they change over a couple of hundred blocks and the terrain has no seam where its
//! colour has one. A height seam would be a cliff; a palette seam is a coastline.
//!
//! The weights are centred on the same thresholds the palette snaps at (`BAND`), with a
//! half-width of `BLEND` in noise units, so a boundary column is exactly half of each
//! neighbour's shape while already wearing one of the two palettes.

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
    /// The block the column's top voxel gets, below the snow line and above the tide line.
    /// **Both tree placers gate on this**, not on `GRASS` — a biome whose surface is not
    /// grass would otherwise go silently bald at every level.
    pub surface: BlockId,
    /// The four blocks under the surface.
    pub subsurface: BlockId,
    /// Canopy block. The tree *shape* is shared across biomes on purpose: `coarse_trees`
    /// scales its proxy density by one `LOD0_CANOPY_COLUMNS` constant, and a second canopy
    /// shape would need a second one to keep the LOD parity test meaningful.
    pub leaf: BlockId,
    /// Multiplies the `mountains` term in `WorldGen::height`. 88.0 is what the whole world
    /// used before batch 9, which is why plains carries it: `--no-biomes` collapses to this
    /// entry and has to reproduce the old world bit for bit.
    pub mountain_amp: f32,
    /// Multiplies the forest noise's tree probability. Zero is a treeless biome, and it is
    /// checked before the noise is sampled.
    pub tree_scale: f32,
    /// Batch 14. Fraction of eligible columns that get a tuft of ground cover, before the
    /// forest noise modulates it. Zero is a bare biome and is checked before any hashing.
    ///
    /// This exists as a per-biome scale rather than as a `surface == GRASS` test for exactly
    /// the reason the comment on `surface` gives: a gate on one block id leaves any biome
    /// that does not use it silently bare, and nothing fails. Desert and tundra are bare here
    /// **because their entries say so**, which is a decision that can be read and changed,
    /// not an accident of which block their ground happens to be.
    pub grass_scale: f32,
    /// Internal Y above which the surface is `SNOW` over `STONE` instead of the biome's own
    /// pair. Read off the column's height, never off a configured value, so LOD 0 and the
    /// coarse levels cannot disagree about it.
    pub snow_line: i32,
    /// Internal Y above which nothing grows: no tree is placed, and the surface is the bare
    /// rock underneath instead of the biome's own ground. Below every biome's snow line, so
    /// the band between the two is scree rather than snow-capped forest.
    pub tree_line: i32,
}

/// Temperate plains reach 209 with `mountain_amp` 88 (measured over 16M columns), so the
/// temperate snow line at 250 is above anything temperate terrain can build. Snow is a cold
/// biome's feature; what puts a snow line on a *plains* mountain is the blend pulling it
/// down toward taiga's 205 on the shoulder between them.
#[rustfmt::skip]
pub const BIOMES: [Biome; BIOME_COUNT] = [
    Biome { name: "plains",  surface: GRASS, subsurface: DIRT,   leaf: LEAVES,      mountain_amp:  88.0, tree_scale: 1.00, grass_scale: 1.00, snow_line: 250, tree_line: 236 },
    Biome { name: "forest",  surface: GRASS, subsurface: DIRT,   leaf: LEAVES,      mountain_amp: 105.0, tree_scale: 1.80, grass_scale: 0.85, snow_line: 250, tree_line: 236 },
    Biome { name: "savanna", surface: GRASS, subsurface: DIRT,   leaf: LEAVES,      mountain_amp:  60.0, tree_scale: 0.50, grass_scale: 0.70, snow_line: 265, tree_line: 250 },
    // The desert's sparse palms are what make the surface gate load-bearing: they stand on
    // SAND, so a placer that still asked for GRASS would leave the desert bald at every
    // level and nothing would fail. `per_biome_canopy_matches_lod0` is what fails.
    Biome { name: "desert",  surface: SAND,  subsurface: SAND,   leaf: LEAVES,      mountain_amp:  45.0, tree_scale: 0.50, grass_scale: 0.00, snow_line: 300, tree_line: 300 },
    Biome { name: "taiga",   surface: GRASS, subsurface: PODZOL, leaf: PINE_LEAVES, mountain_amp: 170.0, tree_scale: 1.50, grass_scale: 0.45, snow_line: 205, tree_line: 176 },
    // Cold and dry, and the tallest terrain in the world. Its lichen ground gives way to
    // scree and then to snow entirely through the two altitude rules -- putting snow in the
    // *palette* instead, as this entry first did, makes the lowlands white down to the tide
    // line and the whole biome reads as one alp from beach to peak rather than as a mountain.
    Biome { name: "tundra",  surface: LICHEN, subsurface: DIRT,  leaf: PINE_LEAVES, mountain_amp: 210.0, tree_scale: 0.00, grass_scale: 0.00, snow_line: 175, tree_line: 150 },
];

/// `GRID[humidity][temperature]`, both indexed dry/mid/wet and cold/mid/hot.
#[rustfmt::skip]
pub const GRID: [[BiomeId; 3]; 3] = [
    [TUNDRA, PLAINS, DESERT],
    [TAIGA, PLAINS, SAVANNA],
    [TAIGA, FOREST, FOREST],
];

/// Where the palette switches, in noise units.
pub const BAND: f32 = 0.22;
/// Half-width of the shape blend, centred on `BAND`. Must stay under `BAND` or the two
/// smoothsteps overlap and the three weights stop summing to one.
pub const BLEND: f32 = 0.18;

const _: () = assert!(BLEND < BAND);

// The two noises the field is made of. Both live here rather than in `WorldGen` because
// batch 12 sends them to the GPU: `resolve.wgsl` evaluates the same field per pixel and
// takes the seed offsets, the frequencies, `BAND` and `BLEND` from these definitions --
// the frequencies through `GpuFrame`, the rest mirrored by hand and pinned by a
// `const _: () = assert!` in `render/mod.rs`.
//
// The frequency is a constraint, not a taste choice. Coarse chunks resample this field at
// stride 2^n, so a biome smaller than a few hundred blocks would alias into speckle at
// stride 8 the way tree placement did before `coarse_trees` existed. At ~1800 and ~1400
// blocks a biome spans over 170 coarse voxels, and its boundary is the only thing that can
// alias at all.
pub const TEMP_FREQ: f32 = 0.00055;
pub const HUMID_FREQ: f32 = 0.0007;
/// Added to the world seed. Two offsets into one seed rather than two seeds so the shader
/// needs one word of `GpuFrame` for both.
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

/// Which of the three bands a noise value falls in. This is the discrete half.
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

/// Blend weights over the three bands, summing to 1 and reaching exactly 1 at a band's
/// interior. This is the continuous half; it is 0.5/0.5 exactly where `band` switches.
#[inline]
pub fn band_weights(n: f32) -> [f32; 3] {
    let hot = smoothstep(BAND - BLEND, BAND + BLEND, n);
    let cold = 1.0 - smoothstep(-BAND - BLEND, -BAND + BLEND, n);
    [cold, 1.0 - cold - hot, hot]
}

/// The biome a (temperature, humidity) pair names, and the blend of the three shape numbers
/// around it. Returned as `(id, mountain_amp, snow_line, tree_line)`.
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

// =============================================================================
// The field's own noise, transliterated.
// =============================================================================
//
// Batch 12 evaluates this same field a second time in WGSL, per pixel, to tint the ground
// from the shaded position. That port has to agree with *this* one about where a biome is
// -- a boundary in the wrong place puts taiga's cold green on desert sand -- and nothing in
// `tests/` can run WGSL to check it.
//
// So the chain is: `simplex2` below is a line-for-line transliteration of
// `fastnoise-lite`'s `single_simplex_2d` plus its `OpenSimplex2` coordinate skew;
// `biome_noise` in `resolve.wgsl` is a line-for-line transliteration of `simplex2`; and
// `noise_matches_fastnoise_lite` in `tests/biome.rs` pins the first link bit-exactly over
// millions of columns. The second link is a transliteration of something *tested*, which is
// the most a shader with no test harness can be given, and a typo in it fails loudly rather
// than subtly -- a mis-transliterated gradient does not shift a boundary, it scrambles the
// whole field.
//
// `simplex2` is what `WorldGen` actually calls, so it is not a reference copy sitting beside
// the real thing: there is one biome field and this is it.

/// FastNoiseLite's own primes and gradient set, because this is FastNoiseLite's algorithm.
const PRIME_X: i32 = 501125321;
const PRIME_Y: i32 = 1136930381;

/// `GRADIENTS_2D`'s first 24 directions -- 15 degrees apart, starting at 82.5 and turning
/// clockwise. The published table is 128 pairs: these 24 repeated five times, then the
/// eight in `GRAD8`. The index arithmetic in `grad` folds the repetition away; the values
/// are copied digit for digit, asymmetric last places included, because the point of this
/// function is to be bit-identical to the crate's.
// Clippy wants these trimmed to the shortest f32 that round-trips. They stay at the
// crate's own precision because the digit-for-digit match *is* the documentation: this
// table is only worth anything if a reader can diff it against `GRADIENTS_2D` by eye.
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

/// The last eight of `GRADIENTS_2D`, 45 degrees apart. Reached only by pair index 120..127,
/// which is 8 of the 128 the hash can land on.
#[allow(clippy::excessive_precision)]
#[rustfmt::skip]
const GRAD8: [f32; 16] = [
     0.38268343236509,   0.923879532511287,  0.923879532511287,  0.38268343236509,   0.923879532511287, -0.38268343236509,   0.38268343236509,  -0.923879532511287,
    -0.38268343236509,  -0.923879532511287, -0.923879532511287, -0.38268343236509,  -0.923879532511287,  0.38268343236509,  -0.38268343236509,   0.923879532511287,
];

/// Truncation toward zero, then a step down for negatives. `as i32` truncates, which is
/// why the negative case needs the correction at all.
#[inline]
fn fast_floor(f: f32) -> i32 {
    if f >= 0.0 {
        f as i32
    } else {
        f as i32 - 1
    }
}

/// The gradient at a lattice point, dotted with the offset to it.
#[inline]
fn grad(seed: i32, x_primed: i32, y_primed: i32, xd: f32, yd: f32) -> f32 {
    let mut h = seed ^ x_primed ^ y_primed;
    h = h.wrapping_mul(0x27d4eb2d);
    h ^= h >> 15;
    // Even, and 0..=254: the table is indexed in pairs and `| 1` picks the second of one.
    let idx = (h & (127 << 1)) as usize;
    let (xg, yg) = if idx < 240 {
        (GRAD24[idx % 48], GRAD24[(idx % 48) | 1])
    } else {
        (GRAD8[idx - 240], GRAD8[(idx - 240) | 1])
    };
    xd * xg + yd * yg
}

/// One octave of OpenSimplex2 at `freq`, in -1..1. Bit-identical to
/// `FastNoiseLite::get_noise_2d` with `NoiseType::OpenSimplex2`, one octave, no fractal.
///
/// Every literal here is written at the crate's full precision and every operation is left
/// in the order the crate wrote it, because f32 addition does not associate: reordering the
/// three corner terms would move the last bit and, at a threshold, a biome boundary.
// Same reason as the gradient tables: `sqrt3` and the final scale are the crate's literals.
#[allow(clippy::excessive_precision)]
#[inline]
pub fn simplex2(seed: i32, freq: f32, x: f32, y: f32) -> f32 {
    let sqrt3: f32 = 1.7320508075688772935274463415059;
    let g2 = (3.0 - sqrt3) / 6.0;

    // The skew into simplex space. The crate does this in `transform_noise_coordinate_2d`,
    // before dispatching on noise type, which is why it reads as a separate step.
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



