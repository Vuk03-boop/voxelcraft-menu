//! Procedural 16x16 texture atlas (no external assets). Layers match `block::tex`.

use crate::block::tex;

pub const TEX_SIZE: u32 = 16;
pub const MIP_LEVELS: u32 = 4;

/// The amplitude of the water layer's per-block mottling. **0.0 ships**; this is the value
/// that reproduces the pre-batch-21b layer bit for bit, and the only reason it has a name is
/// that `--water-mottle` and `tests/textures.rs` both have to quote the same number.
/// Why the mottle went: see the `tex::WATER` arm of `texel`.
///
/// **"0.0 ships" was written while batch 21b's first fix was live, and was false for the six
/// batches after it.** That fix was abandoned -- flattening this layer moved 0 pixels, which
/// is what revealed the real bug -- and 21b put `Config::default()` back to 0.20 without
/// retracting this line, the `shipping` binding in `tests/textures.rs`, or the claim in the
/// `tex::WATER` arm. So the tree carried three descriptions of an abandoned fix and one
/// working default, and the default was the copy the renderer read. Batch 26 made all four
/// agree by taking the lever the roadmap had queued for 21a, and the claim is a test now
/// rather than a sentence: `the_shipping_mottle_is_the_amplitude_the_tests_call_shipping`.
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

/// **Alpha is a tint mask, not opacity.** Nothing sampled it before batch 12 -- every layer
/// wrote 255 and `resolve` took `.rgb` -- so it was a free per-texel channel already carried
/// correctly down the mip chain by `downsample`. `px` is an untinted texel and `pxt` a
/// tinted one; the split is per *texel* and not per layer because grass's side texture is
/// half green fringe and half dirt, and only the fringe may take a biome's colour.
fn px(c: [f32; 3], v: f32) -> [u8; 4] {
    let f = |x: f32| ((x * v).clamp(0.0, 1.0) * 255.0) as u8;
    [f(c[0]), f(c[1]), f(c[2]), 0]
}

/// A texel the biome tint may recolour.
fn pxt(c: [f32; 3], v: f32) -> [u8; 4] {
    let mut p = px(c, v);
    p[3] = 255;
    p
}

/// Grass's albedo, named because batch 18 defines the meadow as a multiple of it and the two
/// must not drift: repaint the grass and the coarse stand-in for a carpet of grass follows.
const GRASS: [f32; 3] = [0.36, 0.62, 0.24];

/// How much light batch 18's coarse ground keeps once it is standing in for a carpet of the
/// ground cover that cannot exist at that stride. **Measured, not authored.**
///
/// Swept at `--cam-height 240 --cam-pitch -20 --cam-yaw 90` against `--streaming-factor 8`,
/// which draws that same ground at LOD 0 and is the only ground truth there is for what the
/// coarse band is imitating: over the carpeted area the mean error against it falls from
/// 15.03 code values to **1.02**, and the green channel to 0.01. 0.63 and 0.55 bracket it at
/// 2.23 and 1.11. See `docs/terrain.md`.
///
/// **A scalar, and that is the finding.** A per-channel fit was tried and measured *worse*
/// (1.17): the carpet is the same green as the ground it hides, only darker, because a blade
/// is a *vertical* face at a high sun and takes far less light than the horizontal ground
/// under it. What made it look like a hue change in the first place was the haze, which
/// lifts blue toward the sky colour faster than red and green -- price the compensation
/// through the fog it will be seen through, not against the albedo it replaces.
///
/// Which is also why this is nowhere near `TALL_GRASS`'s own albedo ratio to `GRASS`
/// (0.53 / 0.72 / 0.55, and not even monotone against this). A blade is not a partial
/// covering of the ground; it is a differently lit surface in front of it.
///
/// Applied to grass's own *value* rather than to its colour, so the two cannot drift in hue:
/// repaint `GRASS` and the meadow follows it exactly.
const MEADOW_SHADE: f32 = 0.59;

/// Blobby low-frequency noise for stone-like textures.
fn blob(x: u32, y: u32, seed: u32) -> f32 {
    let mut acc = 0.0;
    for (dx, dy) in [(0i32, 0i32), (1, 0), (0, 1), (-1, 0), (0, -1)] {
        let xx = ((x as i32 + dx).rem_euclid(16)) as u32;
        let yy = ((y as i32 + dy).rem_euclid(16)) as u32;
        acc += hash(xx / 2, yy / 2, seed);
    }
    acc / 5.0
}

/// One emissive cube: bright veins of `core` over a `body` ground, which is the shape
/// `tex::GLOWSTONE` has drawn since batch 1 and which batch 59's three lamps reuse.
///
/// **Shared rather than copied, because the three differ only in two colours and a seed** --
/// and because what a lamp *looks* like and what it *emits* are two tables that a reader has
/// to be able to compare. `BlockDef::light` is the emission; this is the albedo, and they are
/// authored to agree by eye and by nothing stronger: nothing in the engine derives one from
/// the other, and an emitter whose texture disagreed with its light would be odd rather than
/// wrong.
///
/// `seed` differs per lamp so two of them side by side are not the same blob pattern in two
/// colours, which is the defect batch 36 spent a whole batch on one scale up.
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
                // The fringe, and only the fringe. Tinting the whole layer would put the
                // biome's colour on the dirt below it, which is the thing Minecraft's own
                // side *overlay* exists to avoid.
                pxt(GRASS, 0.78 + 0.32 * n)
            } else {
                px([0.53, 0.38, 0.26], 0.75 + 0.35 * n)
            }
        }
        // Batch 92 (roadmap A8): the desert's surface. Shipping alpha 0 keeps the sand its
        // own colour everywhere -- beaches in *every* biome included, and that is the known
        // spill the arm accepts: a mask is not biome-aware, so a taiga shore's sand would
        // take taiga blue under it. Whether that reads as detail or as staining is the
        // verdict the by-eye pass is for; armed, the desert multiplier that has existed
        // since batch 12 finally reaches the ground it was authored for.
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
        // Batch 59 folded this into `lamp` rather than leaving it beside three copies of
        // itself. The colours and the seed are the ones it has carried since batch 1, so the
        // layer is byte-identical and every capture holding a glowstone stays where it was.
        tex::GLOWSTONE => lamp(x, y, n, 13, [1.0, 0.95, 0.65], [0.80, 0.62, 0.30]),
        tex::BEDROCK => px([0.30, 0.30, 0.32], 0.5 + 0.9 * blob(x, y, 21)),
        // The water layer is not an opaque albedo: `resolve` uses it as the medium's
        // *scattering* colour, the thing the refracted image is absorbed toward with depth.
        //
        // **It ships flat since batch 26**, and the mottle it used to carry is batch 21's
        // diamond weave -- not the salmon marks, which were `tex::GLASS` reached through a
        // bad stride and are batch 21b's subject. The chain is worth stating because
        // four of its five links are somewhere else. `face_uv` takes the position *inside*
        // the voxel, so every water block samples the identical 0..1 of this layer, in
        // phase, forever -- so any structure here is a per-block pattern rather than a
        // texture. `shade_water` mixes it in as `body * (1 - tr)`, and at a few blocks of
        // depth `tr` for red is about a third, so this layer supplies **two thirds of the
        // red** arriving at the eye from under the water. Red is also the channel water
        // takes out fastest, so it arrives dark, which lands it in the steep part of the
        // sRGB transfer. The result is that a variation invisible in the texture is a large
        // variation on screen: the old `0.90 + 0.20 * blob` moved red over codes 23..27,
        // which is 3.7% of the encoded value and **27.9% of the linear one**.
        //
        // So the mottling was never read as the "varying turbidity" the previous comment
        // here claimed. Blue spanned 46.6% in linear too, but blue arrives bright and takes
        // the same relative step to a much smaller visible one.
        //
        // `water_mottle` is the amplitude and **0.20 reproduces the pre-batch-21b layer bit
        // for bit** -- which is what `--water-mottle 0.2` is for, and what
        // `tests/textures.rs` pins against a hash taken before this batch was written.
        //
        // It is written centred on 0.5 so that changing it *nearly* holds the layer's average
        // colour still, and "nearly" is measured rather than hoped for: flattening the mottle
        // lifts the layer's linear mean by **0.15, 0.38 and 1.14 code values** in R, G and B.
        // Two things stop that being exact and neither is worth a fitted constant to fix.
        // `blob`'s mean over this 16x16 tile is **0.4622 and not 0.5**, so centring on 0.5
        // leaves 0.76% of scale on the table; and the sRGB transfer is convex, so the mean of
        // a modulated value sits above the value of its mean whatever it is centred on. The
        // flat texel is [25, 86, 107] where `downsample`'s linear averaging already converges
        // to [25, 86, 106] -- so distant water, which reads the coarse mips, was within one
        // code value of this colour before the batch and only the near field changes.
        tex::WATER => px(
            [0.10, 0.34, 0.42],
            (1.0 - 0.5 * water_mottle) + water_mottle * blob(x, y, 33),
        ),
        // Snow is the brightest albedo in the atlas by a wide margin, which is the point:
        // it is what puts a peak against the sky. Kept at 0.88 rather than 1.0 so the tone
        // map's shoulder still has something to roll off under full sun.
        tex::SNOW => px([0.88, 0.91, 0.97], 0.94 + 0.10 * blob(x, y, 41)),
        tex::PODZOL => px([0.34, 0.23, 0.13], 0.78 + 0.34 * n),
        // The second leaf. Darker and bluer than `LEAVES`, which is the whole of what makes
        // a taiga read as a taiga -- the canopy shape is shared (see `coarse_trees`).
        tex::PINE_LEAVES => {
            let dark = hash(x, y, 6) < 0.40;
            if dark {
                pxt([0.09, 0.24, 0.17], 0.9 + 0.2 * n)
            } else {
                pxt([0.15, 0.36, 0.24], 0.85 + 0.3 * n)
            }
        }
        // Cold, dry ground: lichen and moss over gravel. Deliberately desaturated -- it has
        // to sit between the forest's green and the snow's white without competing with
        // either, and a brown here reads as mud over the wide low areas the biome covers.
        tex::LICHEN => {
            let stone = hash(x, y, 8) < 0.30;
            if stone {
                px([0.47, 0.47, 0.44], 0.82 + 0.3 * n)
            } else {
                // Batch 92 (roadmap A8): the tundra's surface, and the *only* biome whose
                // ground, trees and tufts all escape batch 12's tint without this mask --
                // its `tree_scale` and `grass_scale` are both 0. The stone blobs stay bare,
                // the same rule the grass-side fringe works under: a mask paints biomass,
                // not rock.
                let p = px([0.45, 0.50, 0.38], 0.80 + 0.32 * blob(x, y, 17));
                if tint_balance { pxt([p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0], 1.0) } else { p }
            }
        }
        // Batch 14's ground cover. The cross-quad's *geometry* is only two planes, so the
        // grass is almost entirely in this alpha channel -- this texture is where the look
        // of the batch actually lives, and the geometry just holds it up.
        //
        // **Alpha here is opacity, not the batch-12 tint mask.** `BlockDef::foliage` is what
        // tells `resolve` to read the channel that way; a foliage block's tint mask is
        // implied 1, so a blade still takes its biome's colour on the whole texel.
        tex::TALL_GRASS => {
            // Blade roots spread across the tile, each leaning and tapering to a tip. Six is
            // what fills 16 px without the gaps closing up, and the gaps are the point: they
            // are what makes a field read as blades rather than as a green wall. They are
            // also what the batch costs, because a ray that misses every blade in a voxel
            // has to keep marching and test the next one.
            const BLADES: [(f32, f32, f32); 6] = [
                // root x, tip x, height in texels
                (1.5, 0.4, 12.0),
                (4.0, 5.2, 15.0),
                (6.5, 5.8, 9.0),
                (8.5, 9.9, 14.0),
                (11.0, 10.2, 11.0),
                (13.5, 14.6, 13.0),
            ];
            let fx = x as f32 + 0.5;
            // Row 0 is the tile's bottom. `face_uv` puts v = 1 - local.y, so texture y = 0
            // is the top of the quad and a blade has to grow the other way.
            let ry = (TEX_SIZE - 1 - y) as f32;
            let mut tip_frac = -1.0f32;
            for &(root, tip, h) in BLADES.iter() {
                if ry > h {
                    continue;
                }
                let s = ry / h;
                // Squared so the lean accelerates toward the tip instead of shearing the
                // whole blade, which is what makes a straight line read as a blade at all.
                let cx = root + (tip - root) * s * s;
                let half = 1.15 - 0.85 * s;
                if (fx - cx).abs() <= half {
                    tip_frac = tip_frac.max(s);
                }
            }
            if tip_frac < 0.0 {
                [0, 0, 0, 0]
            } else {
                // Tips lighter and yellower than roots. Without this a field of these reads
                // as one flat colour under a low sun, which is the thing the reference
                // images most obviously are not.
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
        // Batch 18. Grass under a carpet of the layer above, at the strides where a tuft
        // cannot exist. The *same* mottling as `GRASS_TOP`, from the same `n` -- this is
        // the same ground seen through something, not a second kind of ground, and giving
        // it a texture of its own would draw the LOD boundary in detail rather than in
        // colour, which is the defect the batch exists to remove.
        //
        // Tinted, because what it stands in for is tinted twice over: batch 12 tints the
        // grass under the carpet and batch 14's blades take a biome's colour on the whole
        // texel. A savanna's coarse ground has to go straw the way its LOD 0 ground does.
        tex::MEADOW_TOP => pxt(GRASS, (0.78 + 0.32 * n) * MEADOW_SHADE),
        // Batch 92 (roadmap A8, piece 4): the same tile, shaped as a side. Only the meadow
        // *top* was repainted at batch 18, so a steep coarse slope showed LOD 0's bright
        // grass fringe up its face and read as bare ground at treeline height. The fringe
        // band is the carpet's edge seen level-on, so it keeps the tint mask and the same
        // `MEADOW_SHADE` as the top -- one scalar, both faces, the standing rule that the
        // stand-in tracks the thing it stands in for. **Its mottle is its own**, no more
        // byte-equal to `MEADOW_TOP` than any two layers are, because `n` is hashed with
        // the layer id in the seed: the formula is shared, the noise is not, and batch 94
        // corrected the comment that said otherwise.
        tex::MEADOW_SIDE => {
            let edge = 3 + (hash(x, 0, 99) * 2.5) as u32;
            if y < edge {
                pxt(GRASS, (0.78 + 0.32 * n) * MEADOW_SHADE)
            } else {
                px([0.53, 0.38, 0.26], (0.75 + 0.35 * n) * MEADOW_SHADE)
            }
        }
        // Batch 101e (`--grass-dense`): the tall tussock of the references' 1-2 m stands.
        // The tuft's blade algebra, with three differences authored: the blades climb
        // nearly the whole tile (worldgen stacks this id on the cell above, so one image
        // shows twice per tussock --- the read is tall *and* dense), the roots pull toward
        // the tile's middle so the two stacked images read as one clump rather than as two
        // carpets, and the tips run dry --- yellow-green, not the tuft's fresh yellow,
        // which is the colour the references' tall stands actually carry.
        tex::GRASS_TALL => {
            const BLADES: [(f32, f32, f32); 7] = [
                // root x, tip x, height in texels
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
        // Batch 101e: the wet-bank fringe of the references' lakes --- straight reed
        // stalks, placed by the generator only where open water meets a bank column. A
        // stalk does not shear the way a blade does, so its lean is linear and small and
        // its width nearly keeps: a 2-3 cell stack must stay in one piece as it pokes
        // through the tide line. Muted rather than lawn-green, because reeds read
        // cool-grey-green at the distances a bank is seen from.
        tex::REEDS => {
            const STALKS: [(f32, f32, f32); 5] = [
                // root x, tip x, height in texels
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
        // Batch 59's emitters. Each albedo is the hue its `BlockDef::light` emits, lightened
        // toward white at the core the way `GLOWSTONE`'s is -- a lamp reads as a lamp because
        // its brightest texels are near-white, not because they are saturated.
        tex::AMBER_LAMP => lamp(x, y, n, 41, [1.0, 0.78, 0.42], [0.74, 0.34, 0.10]),
        tex::AZURE_LAMP => lamp(x, y, n, 43, [0.68, 0.88, 1.0], [0.14, 0.34, 0.80]),
        tex::VERDANT_LAMP => lamp(x, y, n, 47, [0.74, 1.0, 0.58], [0.18, 0.62, 0.20]),
        _ => [255, 0, 255, 0],
    }
}

/// All layers, mip 0, RGBA8, layer-major.
///
/// `tint_balance` (batch 92, roadmap A8) is *content*, not a pipeline: at `false` the atlas
/// is the file every batch since 12 mip-chained, at `true` the sand and lichen layers gain
/// the tint mask their two biomes' multipliers need to reach the ground at all. It arrives
/// here rather than in the shader because no override can put alpha into a file that was
/// written without it, and it is a constructor parameter of `Renderer::new` for that
/// signature's own documented reason: a build-once argument a lenient call shape would lose.
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

/// Box-filter one mip level down. `size` is the source edge length.
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

/// Box-filter one mip level. Colour is averaged in **linear** space and re-encoded;
/// averaging the sRGB bytes directly biases every mip dark, worst on high-contrast texels.
/// Alpha is already linear and is averaged as-is.
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



