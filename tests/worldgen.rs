//! Worldgen invariants: caves are carved, terrain is walkable, LODs agree on shape.
// These tests hold **transliterations** of formulations in `src/`, on purpose: the point of
// the copy is that it reads the same as the original, so a divergence is visible. Clippy's
// modernisations here would silently make the two halves of each mirror look different, which
// costs exactly the thing the duplication buys.
#![allow(clippy::int_plus_one)]
#![allow(clippy::manual_is_multiple_of)]
#![allow(clippy::manual_range_contains)]

use glam::IVec3;
use voxelcraft::biome::{self, BiomeId};
use voxelcraft::block::*;
use voxelcraft::voxel::*;
use voxelcraft::worldgen::{canopy_rank3, WorldGen, COARSE_CANOPY_KEEP, MEADOW_SPREAD, SEA_LEVEL};

fn gen_chunk(gen: &WorldGen, key: ChunkKey) -> Vec<BlockId> {
    let mut dense = vec![AIR; VOL];
    let mut heights = Box::new([0i32; 64 * 64]);
    gen.generate(key, &mut dense, &mut heights);
    dense
}

#[test]
fn caves_are_carved_underground() {
    let gen = WorldGen::new(1337);
    // A band of chunks well below the surface everywhere, so any air is a cave.
    let mut air_below_surface = 0u64;
    let mut total = 0u64;
    for cx in 0..3 {
        for cz in 0..3 {
            let key = ChunkKey::new(0, IVec3::new(cx, 1, cz));
            let dense = gen_chunk(&gen, key);
            let o = key.origin();
            for z in 0..64usize {
                for x in 0..64usize {
                    let h = gen.height(o.x + x as i32, o.z + z as i32);
                    for y in 0..64usize {
                        let wy = o.y + y as i32;
                        // Only count well below the surface and above bedrock.
                        if wy > 8 && wy < h - 8 {
                            total += 1;
                            if dense[dense_index(x, y, z)] == AIR {
                                air_below_surface += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(
        total > 100_000,
        "expected a large subsurface sample, got {total}"
    );
    let frac = air_below_surface as f64 / total as f64;
    assert!(
        frac > 0.005,
        "no caves carved: only {:.4}% of subsurface is air",
        frac * 100.0
    );
    assert!(
        frac < 0.40,
        "caves ate the world: {:.1}% of subsurface is air",
        frac * 100.0
    );
}

/// The named assertion is the point of this test: the top block of a column is *this
/// biome's* surface, not merely something solid. It grew with the table in batch 9 rather
/// than being relaxed, because "anything solid" is exactly what would pass while a desert
/// quietly rendered as bare stone.
#[test]
fn surface_is_the_biomes_own_block() {
    let gen = WorldGen::new(1337);
    let mut checked = 0;
    let mut seen = [0u32; biome::BIOME_COUNT];
    // A wide spread of chunks, so the sample crosses biomes instead of describing one.
    // Terrain now spans internal Y 94..295, so a single slab reaches only a slice of the
    // world; taiga and tundra peaks live two slabs above a desert floor.
    for &(cx, cz) in &[
        (0i32, 0i32),
        (12, -7),
        (-9, 14),
        (23, 21),
        (-18, -22),
        (31, -30),
    ] {
        for cy in 1..5 {
            let key = ChunkKey::new(0, IVec3::new(cx, cy, cz));
            let dense = gen_chunk(&gen, key);
            let o = key.origin();
            for z in (0..64usize).step_by(7) {
                for x in (0..64usize).step_by(7) {
                    let (wx, wz) = (o.x + x as i32, o.z + z as i32);
                    let h = gen.height(wx, wz);
                    let ly = h - o.y;
                    if ly < 2 || ly >= 64 {
                        continue;
                    }
                    let cb = gen.column_biome(wx, wz);
                    let b = cb.def();
                    let top = dense[dense_index(x, ly as usize - 1, z)];
                    // Below the tide line the shore rule wins; above the snow line the snow
                    // rule does; a tree stands its trunk on top of either. A cave can also
                    // open at the surface, which leaves the subsurface or stone exposed.
                    let expected: &[BlockId] = if h <= SEA_LEVEL + 2 {
                        &[SAND]
                    } else if h - 1 >= cb.snow_line {
                        &[SNOW]
                    } else if h - 1 >= cb.tree_line {
                        &[STONE]
                    } else {
                        &[b.surface]
                    };
                    assert!(
                    expected.contains(&top) || top == LOG || top == b.subsurface || top == STONE || top == GRAVEL || top == AIR,
                    "surface at {wx},{wz} in {} was {top}, expected {expected:?} (height {h}, snow line {})",
                    b.name,
                    cb.snow_line
                );
                    if expected.contains(&top) {
                        seen[cb.id as usize] += 1;
                    }
                    // Below the tide line the column carries on as water, not air. Since
                    // batch 14 a tuft of ground cover can stand there too -- and where one
                    // does, it has to have obeyed the placer's own gates, so this checks
                    // them rather than merely tolerating the new id.
                    let above = dense[dense_index(x, ly as usize, z)];
                    assert!(
                        above == AIR || above == WATER || above == TALL_GRASS,
                        "block above the surface at {wx},{wz} was {above}"
                    );
                    if above == TALL_GRASS {
                        assert!(
                            b.grass_scale > 0.0,
                            "ground cover at {wx},{wz} in {}, which has grass_scale 0",
                            b.name
                        );
                        assert_eq!(
                            top, b.surface,
                            "ground cover at {wx},{wz} in {} stands on {top}, not the \
                             biome's own surface",
                            b.name
                        );
                        assert!(
                            h > SEA_LEVEL + 1 && h <= cb.tree_line,
                            "ground cover at {wx},{wz} outside the tide/tree line band \
                             (height {h}, tree line {})",
                            cb.tree_line
                        );
                    }
                    checked += 1;
                }
            }
        }
    }
    assert!(
        checked > 100,
        "expected to check many columns, got {checked}"
    );
    let named: Vec<&str> = seen
        .iter()
        .enumerate()
        .filter(|(_, &c)| c > 0)
        .map(|(i, _)| biome::def(i as BiomeId).name)
        .collect();
    assert!(
        named.len() >= 4,
        "sample only reached {named:?}; the test is describing one biome"
    );
}

#[test]
fn slopes_stay_walkable() {
    // Terrain amplitude must stay well under the noise wavelength, or the world turns
    // into unclimbable spikes. Sample adjacent columns and bound the step.
    //
    // The area has to be wide enough to cross biome boundaries. A biome is ~1500 blocks,
    // so the old +/-200 patch sat inside a single one and could not see the thing batch 9
    // actually risks: the mountain amplitude sliding from 45 to 250 across a boundary.
    // 6000 blocks on a stride-3 lattice covers many of them for the same sample count.
    let gen = WorldGen::new(1337);
    let mut worst = 0;
    let mut steep = 0;
    let mut n = 0;
    for z in (-3000..3000i32).step_by(15) {
        for x in -3000..3000i32 {
            let a = gen.height(x, z);
            let b = gen.height(x + 1, z);
            let d = (a - b).abs();
            worst = worst.max(d);
            if d > 2 {
                steep += 1;
            }
            n += 1;
        }
    }
    let frac = steep as f64 / n as f64;
    assert!(worst <= 8, "single-block step of {worst} is a cliff wall");
    assert!(
        frac < 0.02,
        "{:.1}% of steps exceed 2 blocks; terrain is too spiky",
        frac * 100.0
    );
}

#[test]
fn coarse_lod_follows_the_same_surface() {
    // A LOD-2 chunk samples the same height field, so its top voxel should sit near the
    // real terrain height rather than drifting.
    let gen = WorldGen::new(1337);
    let key = ChunkKey::new(2, IVec3::new(0, 0, 0));
    let dense = gen_chunk(&gen, key);
    let s = key.voxel_size();
    let o = key.origin();
    let mut checked = 0;
    for z in (0..64usize).step_by(9) {
        for x in (0..64usize).step_by(9) {
            let wx = o.x + x as i32 * s + s / 2;
            let wz = o.z + z as i32 * s + s / 2;
            let h = gen.height(wx, wz);
            // Skip proxy vegetation and the sea: like a LOD-0 tree they sit above the
            // surface on purpose, and the invariant under test is that the *ground* does
            // not drift. `tests/water.rs` covers the sea's own coverage.
            let top = (0..64usize).rev().find(|&y| {
                let b = dense[dense_index(x, y, z)];
                b != AIR && b != LEAVES && b != PINE_LEAVES && b != LOG && b != WATER
            });
            if let Some(ty) = top {
                let voxel_top = o.y + (ty as i32 + 1) * s;
                assert!(
                    (voxel_top - h).abs() <= s,
                    "LOD2 surface at {wx},{wz} is {voxel_top} but terrain is {h}"
                );
                checked += 1;
            }
        }
    }
    assert!(
        checked > 10,
        "expected coarse surface samples, got {checked}"
    );
}

/// Canopy coverage over the same patch of world at every LOD. The proxies exist to keep
/// a forested hill looking forested across a transition, so the fraction of ground under
/// leaves has to land near the LOD-0 value, not merely be non-zero.
#[test]
fn coarse_canopy_density_matches_lod0() {
    use voxelcraft::block::{AIR, LEAVES};
    use voxelcraft::voxel::dense_index;
    let gen = WorldGen::new(1337);
    let mut dense = vec![0u16; VOL];
    let mut heights = Box::new([0i32; 64 * 64]);
    let mut cover = [0f64; 4];
    for lod in 0..=3u8 {
        // Cover the same 512-block square of world at every level.
        let side = 8 >> lod;
        let slabs = 512 >> (6 + lod);
        let n = (side * 64) as usize;
        let mut top_block = vec![AIR; n * n];
        for cz in 0..side {
            for cx in 0..side {
                for cy in 0..slabs {
                    let key = ChunkKey::new(lod, IVec3::new(cx, cy, cz));
                    gen.generate(key, &mut dense, &mut heights);
                    for z in 0..64usize {
                        for x in 0..64usize {
                            // Slabs are visited bottom-up, so a higher one overwrites.
                            if let Some(t) =
                                (0..64).rev().find(|&y| dense[dense_index(x, y, z)] != AIR)
                            {
                                let gx = cx as usize * 64 + x;
                                let gz = cz as usize * 64 + z;
                                top_block[gx + gz * n] = dense[dense_index(x, t, z)];
                            }
                        }
                    }
                }
            }
        }
        let leaf = top_block.iter().filter(|&&b| b == LEAVES).count();
        cover[lod as usize] = leaf as f64 / top_block.len() as f64;
        println!(
            "  lod {lod}: canopy covers {:.2}% of columns",
            cover[lod as usize] * 100.0
        );
    }
    for lod in 1..4 {
        let ratio = cover[lod] / cover[0];
        assert!(
            (0.7..1.6).contains(&ratio),
            "lod {lod} canopy coverage is {ratio:.2}x lod0 ({:.2}% vs {:.2}%)",
            cover[lod] * 100.0,
            cover[0] * 100.0
        );
    }
}

/// Top block of every column of a 512-block square at one LOD, paired with the biome id at
/// the point the generator itself sampled -- the column centre at that stride. Only the
/// slabs that can hold the surface are generated: the canopy lives at the terrain height,
/// and a chunk wholly above it already early-outs, so this is the same answer for a third
/// of the work.
fn surface_survey(gen: &WorldGen, lod: u8, origin: IVec3) -> Vec<(BiomeId, BlockId)> {
    let side = 8 >> lod;
    let chunk = 64i32 << lod;
    let stride = 1i32 << lod;
    let n = (side * 64) as usize;
    let (mut lo, mut hi) = (i32::MAX, i32::MIN);
    for z in (0..512).step_by(8) {
        for x in (0..512).step_by(8) {
            let h = gen.height(origin.x + x, origin.z + z);
            lo = lo.min(h);
            hi = hi.max(h);
        }
    }
    let mut dense = vec![AIR; VOL];
    let mut heights = Box::new([0i32; 64 * 64]);
    let mut top = vec![AIR; n * n];
    for cz in 0..side {
        for cx in 0..side {
            for cy in (lo - 8).div_euclid(chunk)..=(hi + 12).div_euclid(chunk) {
                let base = IVec3::new(
                    origin.x.div_euclid(chunk) + cx,
                    cy,
                    origin.z.div_euclid(chunk) + cz,
                );
                gen.generate(ChunkKey::new(lod, base), &mut dense, &mut heights);
                for z in 0..64usize {
                    for x in 0..64usize {
                        // Slabs are visited bottom-up, so a higher one overwrites.
                        if let Some(t) = (0..64).rev().find(|&y| dense[dense_index(x, y, z)] != AIR)
                        {
                            top[(cx as usize * 64 + x) + (cz as usize * 64 + z) * n] =
                                dense[dense_index(x, t, z)];
                        }
                    }
                }
            }
        }
    }
    top.iter()
        .enumerate()
        .map(|(i, &b)| {
            let wx = origin.x + (i % n) as i32 * stride + stride / 2;
            let wz = origin.z + (i / n) as i32 * stride + stride / 2;
            (gen.biome_at(wx, wz), b)
        })
        .collect()
}

/// 9d, and the trap batch 9 was warned about. `coarse_canopy_density_matches_lod0` averages
/// over whatever its one patch happens to contain, so a biome that went bald at every level
/// -- a desert whose palms stand on sand while the placer still asks for grass -- would move
/// that average by a fraction of a per cent and pass. This asks each biome separately.
///
/// It attributes every column to its own biome rather than looking for a patch that is only
/// one biome, because two of the six never form one: plains and savanna sit in the *middle*
/// temperature and humidity band, and on smooth noise a middle band is a ribbon a few
/// hundred blocks wide, not a region. Four regions and two ribbons is what a 3x3 grid over
/// two scalars gives you.
#[test]
fn per_biome_canopy_matches_lod0() {
    let gen = WorldGen::new(1337);
    // Spread far enough apart to reach every cell of the grid between them.
    let regions = [
        IVec3::new(0, 0, 0),
        IVec3::new(2048, 0, -1536),
        IVec3::new(-2560, 0, 1024),
        IVec3::new(1536, 0, 3072),
        IVec3::new(-3584, 0, -2048),
        IVec3::new(3072, 0, 2560),
        IVec3::new(-1024, 0, -3584),
        IVec3::new(4096, 0, -3072),
        IVec3::new(-4608, 0, 3584),
        IVec3::new(5120, 0, 1024),
        IVec3::new(512, 0, -5120),
        IVec3::new(-2048, 0, -5632),
    ];
    let mut columns = [[0u64; biome::BIOME_COUNT]; 4];
    let mut canopy = [[0u64; biome::BIOME_COUNT]; 4];
    for &origin in &regions {
        for lod in 0..4usize {
            for (id, block) in surface_survey(&gen, lod as u8, origin) {
                columns[lod][id as usize] += 1;
                if block == biome::def(id).leaf {
                    canopy[lod][id as usize] += 1;
                }
            }
        }
    }
    for id in 0..biome::BIOME_COUNT {
        let b = biome::def(id as BiomeId);
        let cover: Vec<f64> = (0..4)
            .map(|l| canopy[l][id] as f64 / columns[l][id].max(1) as f64)
            .collect();
        println!(
            "  {:8} {:>9} columns: {:.2}% {:.2}% {:.2}% {:.2}%",
            b.name,
            columns[0][id],
            cover[0] * 100.0,
            cover[1] * 100.0,
            cover[2] * 100.0,
            cover[3] * 100.0
        );
        assert!(
            columns[0][id] > 50_000,
            "{} is barely in the sample ({} columns)",
            b.name,
            columns[0][id]
        );
        if b.tree_scale == 0.0 {
            // Not exactly zero: a canopy is 5 columns wide and its trunk decides the
            // biome, so a taiga tree standing one column inside the border overhangs two
            // columns of tundra. Two orders of magnitude under the sparsest treed biome is
            // an overhang; anything near it would be trees actually being placed here.
            for (lod, c) in cover.iter().enumerate() {
                assert!(
                    *c < 0.0001,
                    "{} is treeless but {:.4}% of it is under leaves at lod {lod}",
                    b.name,
                    c * 100.0
                );
            }
            continue;
        }
        assert!(
            cover[0] > 0.002,
            "{} has no canopy at LOD 0 ({:.3}%) -- the surface gate is rejecting its own surface block",
            b.name,
            cover[0] * 100.0
        );
        for lod in 1..4 {
            let ratio = cover[lod] / cover[0];
            assert!(
                (0.5..2.0).contains(&ratio),
                "{} canopy at lod {lod} is {ratio:.2}x lod0 ({:.2}% vs {:.2}%)",
                b.name,
                cover[lod] * 100.0,
                cover[0] * 100.0
            );
        }
    }
}

/// Batch 18's whole claim in one number: the coarse ground wears meadow at the rate the
/// fine ground wears tufts, per biome.
///
/// This is the density test `coarse_canopy_density_matches_lod0` is for trees, and it can
/// afford to be far tighter than that one for a reason worth saying out loud. A proxy tree
/// has to match a *canopy area* it cannot reproduce -- one coarse voxel shadows `s*s`
/// columns, so the rate has to be rescaled by the proxy's own footprint and the result
/// drifts 1.25-1.7x per biome. A meadow is one voxel standing for one voxel's worth of
/// colour, so `coarse_meadow` stamps at `ground_cover` **unscaled** and the two rates are
/// the same expression evaluated at two strides. What is left to drift is only the sampling:
/// the coarse column asks the forest field once at its centre where the fine columns ask it
/// `s*s` times, and coarse height resampling moves a few columns across the tide and tree
/// lines.
///
/// The per-biome check is what makes the shared expression load-bearing rather than merely
/// tidy. A second copy that agreed on the day it was written would pass an aggregate test
/// and fail this one the moment `grass_scale` moved in one biome.
#[test]
fn coarse_meadow_density_matches_lod0() {
    let gen = WorldGen::new(1337);
    let regions = [
        IVec3::new(0, 0, 0),
        IVec3::new(2048, 0, -1536),
        IVec3::new(-2560, 0, 1024),
        IVec3::new(1536, 0, 3072),
        IVec3::new(-3584, 0, -2048),
        IVec3::new(3072, 0, 2560),
        IVec3::new(-1024, 0, -3584),
        IVec3::new(4096, 0, -3072),
        IVec3::new(-4608, 0, 3584),
        IVec3::new(5120, 0, 1024),
        IVec3::new(512, 0, -5120),
        IVec3::new(-2048, 0, -5632),
    ];
    let mut columns = [[0u64; biome::BIOME_COUNT]; 4];
    let mut cover = [[0u64; biome::BIOME_COUNT]; 4];
    for &origin in &regions {
        for lod in 0..4usize {
            for (id, block) in surface_survey(&gen, lod as u8, origin) {
                columns[lod][id as usize] += 1;
                // The two representations of the same thing: a real tuft at LOD 0, and the
                // repainted surface that stands in for one everywhere else.
                let painted = if lod == 0 { TALL_GRASS } else { MEADOW };
                if block == painted {
                    cover[lod][id as usize] += 1;
                }
            }
        }
    }
    for id in 0..biome::BIOME_COUNT {
        let b = biome::def(id as BiomeId);
        let rate: Vec<f64> = (0..4)
            .map(|l| cover[l][id] as f64 / columns[l][id].max(1) as f64)
            .collect();
        println!(
            "  {:8} grass_scale {:.2}: {:.2}% {:.2}% {:.2}% {:.2}%  (x{:.2} x{:.2} x{:.2})",
            b.name,
            b.grass_scale,
            rate[0] * 100.0,
            rate[1] * 100.0,
            rate[2] * 100.0,
            rate[3] * 100.0,
            rate[1] / rate[0].max(1e-9),
            rate[2] / rate[0].max(1e-9),
            rate[3] / rate[0].max(1e-9)
        );
        if b.grass_scale == 0.0 {
            // Exactly zero, and unlike the canopy's treeless check there is no overhang to
            // allow for: a meadow is one column wide, so a bare biome that shows any at all
            // has a gate reading the wrong column.
            for (lod, r) in rate.iter().enumerate() {
                assert_eq!(
                    cover[lod][id],
                    0,
                    "{} has grass_scale 0 but {:.3}% of it is meadow at lod {lod}",
                    b.name,
                    r * 100.0
                );
            }
            continue;
        }
        assert!(
            rate[0] > 0.02,
            "{} has almost no ground cover at LOD 0 ({:.2}%) -- the surface gate is              rejecting its own surface block",
            b.name,
            rate[0] * 100.0
        );
        for lod in 1..4 {
            let ratio = rate[lod] / rate[0];
            // The band is *derived*, not fitted. `coarse_meadow` stamps at
            // `min(1, cover * MEADOW_SPREAD)`, so its rate over any set of columns is
            // between `cover` and `cover * MEADOW_SPREAD` for every distribution of the
            // forest field there is: at or above 1.0 because the spread is at least 1, and
            // at or below the spread because saturation only ever takes rate away. A ratio
            // outside it means the two placers are no longer reading one expression.
            //
            // The 5% is for the denominators and not for the rule: both rates are counted
            // against *all* columns of the biome, and a column under a canopy is not one
            // either placer may paint -- so the coarse canopy running 1.25-1.7x LOD 0's,
            // which is a caveat this engine has carried since batch 9, moves the two
            // denominators apart. Savanna comes out at 1.78 against a ceiling of 1.75 for
            // exactly that reason and nothing else.
            assert!(
                (1.0..=MEADOW_SPREAD * 1.05).contains(&(ratio as f32)),
                "{} meadow at lod {lod} is {ratio:.3}x its LOD 0 cover ({:.2}% vs {:.2}%),                  outside 1.0..={:.2}",
                MEADOW_SPREAD * 1.05,
                b.name,
                rate[lod] * 100.0,
                rate[0] * 100.0
            );
        }
    }
}

/// The trap batch 9 was warned about, measured where it would actually bite. Both tree
/// placers used to ask for `GRASS`; they now ask for **this biome's own surface**, and the
/// difference is invisible in aggregate coverage — a desert whose palms all failed the gate
/// would move the world's canopy fraction by hundredths of a per cent and every other test
/// would pass.
///
/// So this counts trunks against eligible columns and divides by the biome's own
/// `tree_scale`. That normalised rate is a property of the forest noise alone, so it has to
/// land in the same place for every biome regardless of surface block; a biome whose gate
/// rejects its own surface reads exactly zero.
#[test]
fn trees_grow_on_every_biomes_own_surface() {
    let gen = WorldGen::new(1337);
    let mut dense = vec![AIR; VOL];
    let mut heights = Box::new([0i32; 64 * 64]);
    let mut trunks = [0u64; biome::BIOME_COUNT];
    let mut eligible = [0u64; biome::BIOME_COUNT];
    // Every third chunk over +/-2000 blocks. Biomes are ~1500 blocks across, so a solid
    // block of chunks around the origin reaches two or three of them; a sparse lattice over
    // a wide area reaches all six for a third of the generation cost.
    for cz in (-32..32i32).step_by(3) {
        for cx in (-32..32i32).step_by(3) {
            for cy in 1..4 {
                let key = ChunkKey::new(0, IVec3::new(cx, cy, cz));
                gen.generate(key, &mut dense, &mut heights);
                let o = key.origin();
                for z in 3..61usize {
                    for x in 3..61usize {
                        let (wx, wz) = (o.x + x as i32, o.z + z as i32);
                        let cb = gen.column_biome(wx, wz);
                        let h = heights[x + z * 64];
                        let ly = h - o.y;
                        // The placer's own gates, so "eligible" means what it means there.
                        if ly < 1 || ly + 7 >= 64 || h <= SEA_LEVEL + 2 || h > cb.tree_line {
                            continue;
                        }
                        eligible[cb.id as usize] += 1;
                        if dense[dense_index(x, ly as usize, z)] == LOG {
                            trunks[cb.id as usize] += 1;
                        }
                    }
                }
            }
        }
    }
    let mut rates = Vec::new();
    for id in 0..biome::BIOME_COUNT {
        let b = biome::def(id as BiomeId);
        let rate = trunks[id] as f64 / eligible[id].max(1) as f64;
        println!(
            "  {:8} surface {:2}: {:5} trunks / {:7} eligible = {:.4}%, per unit scale {:.4}%",
            b.name,
            b.surface,
            trunks[id],
            eligible[id],
            rate * 100.0,
            rate * 100.0 / b.tree_scale.max(0.01) as f64
        );
        if b.tree_scale == 0.0 {
            assert_eq!(
                trunks[id], 0,
                "{} is treeless but grew {} trunks",
                b.name, trunks[id]
            );
            continue;
        }
        assert!(
            eligible[id] > 20_000,
            "{} has only {} eligible columns in the sample",
            b.name,
            eligible[id]
        );
        assert!(
            trunks[id] > 0,
            "{} grew no trees at all: its placer is gating on a block that is not its \
             surface ({})",
            b.name,
            b.surface
        );
        rates.push((b.name, rate / b.tree_scale as f64));
    }
    let lo = rates.iter().map(|r| r.1).fold(f64::MAX, f64::min);
    let hi = rates.iter().map(|r| r.1).fold(0.0, f64::max);
    assert!(
        hi / lo < 2.0,
        "normalised tree rates disagree by {:.2}x across biomes: {rates:?}",
        hi / lo
    );
}

// ---- Batch 91, roadmap A5 (`--tree-blue-noise`): the canopy's LOD cliff, closed in the
// generator ----
//
// What is pinned here is the flag's behavioural contract, without a renderer: at rest the
// world is the legacy one (proxy cells all written), and armed it is the *same trees* read
// through the R3 rank field -- holes only where proxies already were, trunks never touched,
// the survivor count near `LEAF_FILL`. The transmittance-matching constant itself is
// compile-time pinned in `src/worldgen.rs` against `render::LEAF_FILL`, so a drift between
// the carve and the decimation cannot build.

const A5_SEED: i32 = 90210;

fn coarse_keys(gen: &WorldGen) -> Vec<ChunkKey> {
    // LOD 2 (stride 4): 256-block chunks at y = 0, above which the generator's tree-line
    // rules already exclude the placer. **Locate the canopy, don't assume it.** Batch 94
    // fixed the (x, y, z) transposition -- and the corrected fixed grid became the next
    // fragile premise: this seed's 1 km slice at mz 0..2 keeps only 337 leaf-touching
    // pairs total, where a clustering test asks four digits per axis. So scan a bounded
    // window for the four leaf-richest chunks: deterministic for a fixed seed, and honest
    // about what it means to fail -- if even the window holds no canopy, the tree placer
    // is the finding, named by the assert below, not a margin silently retested.
    let mut ranked: Vec<(usize, ChunkKey)> = Vec::new();
    for mz in 0..6 {
        for mx in 0..12 {
            let key = ChunkKey::new(2, IVec3::new(mx, 0, mz));
            let d = gen_chunk(gen, key);
            let (leaves, _) = leaf_and_log(&d);
            ranked.push((leaves, key));
        }
    }
    ranked.sort_by_key(|&(n, _)| std::cmp::Reverse(n));
    assert!(
        ranked.len() >= 4 && ranked[3].0 >= 200,
        "seed {A5_SEED} has no canopy in its (0..12)x(0..6) window at LOD 2: best four \
         leaf counts {:?}",
        ranked.iter().take(4).map(|&(n, _)| n).collect::<Vec<_>>()
    );
    ranked.into_iter().take(4).map(|(_, k)| k).collect()
}

fn leaf_and_log(dense: &[BlockId]) -> (usize, usize) {
    dense.iter().fold((0, 0), |(l, t), &b| match b {
        LEAVES => (l + 1, t),
        LOG => (l, t + 1),
        _ => (l, t),
    })
}

#[test]
fn a5_flag_off_writes_solid_proxies_everywhere_and_on_thins_them() {
    let off = WorldGen::new(A5_SEED);
    assert!(!off.tree_blue_noise, "`--tree-blue-noise` defaults off");
    let mut on = WorldGen::new(A5_SEED);
    on.tree_blue_noise = true;

    let mut off_leaves = 0usize;
    let mut off_logs = 0usize;
    let mut on_leaves = 0usize;
    let mut on_logs = 0usize;
    let mut proxied = 0usize;
    for key in coarse_keys(&off) {
        let a = gen_chunk(&off, key);
        let b = gen_chunk(&on, key);
        let (la, ta) = leaf_and_log(&a);
        let (lb, tb) = leaf_and_log(&b);
        off_leaves += la;
        off_logs += ta;
        on_leaves += lb;
        on_logs += tb;
        if la > 0 {
            proxied += 1;
        }
        // Holes may open only inside cells that the legacy proxy would have stamped, and
        // every other kind of cell must be *identical*: same trees, same terrain, one
        // difference -- the rank decision on a leaf write. This is the A/B discriminant a
        // reverted-by-construction arm cannot give, because the legacy map is the input.
        // Foliage here means *either* species: the stamp writes the biome's own leaf id
        // (`b.leaf`), so pine parcels open holes in PINE_LEAVES. A predicate naming only
        // `LEAVES` pattern-matches one species and shouts at a legal pine hole -- batch
        // 98's hardware round ate exactly that (its failure line read: 16 -> 0).
        for (&x, &y) in a.iter().zip(b.iter()) {
            assert!(
                y == x || ((x == LEAVES || x == PINE_LEAVES) && y == AIR),
                "armed cell differs outside a leaf hole: {x} -> {y}"
            );
        }
    }
    assert!(
        proxied >= 4,
        "world has no coarse canopy to thin: {proxied}"
    );
    assert!(on_leaves > 0, "the rank field thinned everything");
    assert_eq!(
        on_logs, off_logs,
        "trunks are not porous at LOD 0 and must not be thinned"
    );

    let ratio = on_leaves as f64 / off_leaves as f64;
    // `binomial(p, n)` at p = 0.62 across this many cells is a knife edge; the window is
    // the *contract* -- near LEAF_FILL, not a point estimate -- so it lands visibly wide.
    assert!(
        (0.45..=0.85).contains(&ratio),
        "decimated share {ratio:.3} of {off_leaves}..{on_leaves} leaves is not near LEAF_FILL"
    );

    // Deterministic by construction: the same fields drawn twice must be the same field,
    // because the temporal pipeline's claim that a chunk's content is a pure function of
    // its key cannot survive an unstable rank.
    let key = ChunkKey::new(2, IVec3::new(1, 0, 0));
    assert_eq!(gen_chunk(&on, key), gen_chunk(&on, key));
}

#[test]
fn a5_rank_field_is_not_white_noise() {
    // The blue-noise claim, at the scale it matters: white noise would keep 62% of cells
    // too, so counting cannot tell the fields apart. What *does* is the survivor pattern's
    // clustering -- under i.i.d. Bernoulli(p = 0.62) a share 0.4493 of mixed neighbour
    // pairs survive both cells (p^2/(1 - (1-p)^2)); an even rank field separates them.
    //
    // Batch 94 measured all three axes of the generator's own field (threshold R3 = 0.62,
    // rank drawn at world-coordinate step 4, so one axis of stride crosses a different
    // amount of the hash's period per direction):
    //     x: 0.3922 -- better than the coin but not much; batch 91's bound 0.40 sat 0.008
    //         off the field's own value and was a knife-edge in disguise.
    //     z: 0.2793 -- the axis where the field is visibly blue.
    //     y: 0.6366 -- ABOVE the coin: vertical neighbours *re*-correlate. That is a
    //         property to record, not a defect to fix: the claim this flag keeps is the
    //         mean transmittance (0.62, pinned by the share test above), and canopy
    //         columns re-correlating says nothing about that mean. Blue in plan, brown
    //         in profile, coin or better on every axis.
    // The claim is therefore pinned on z, whose measurement has real margin, and the
    // other two axes are pinned as *properties* (x beats the coin, y beats it from above)
    // so a future field change that collapses any of the three is loud rather than silent.
    //
    // Batch 98's sampling repair moved *where* the y property can be read. These loops
    // now watch the actual LOD-2 proxies, where `canopy = (trunk_h / s).max(1)` is one
    // voxel tall: the stamped shell carries no vertical pair among survivors at all
    // (the round measured 0.000 vs the coin's 0.449). The 0.6366 figure was the *rank
    // field's*, not the shell's -- so chunk pixels pin the shell's structural bound
    // instead, and the field lattice at the bottom of this test re-earns the axis
    // properties on `canopy_rank3` at the same step-4 stride. One honest correction to
    // 94's table: on the full 3D lattice the z share reads 0.391, better than the coin
    // in x's league; the 0.279 figure was one canopy *slice*, where a constant A2*y
    // phase reshuffles the z stride's fractional iterates -- slice-constrained numbers
    // are not the field's property, and the pin below targets the field.
    let mut on = WorldGen::new(A5_SEED);
    on.tree_blue_noise = true;
    // Count, per axis, the share of leaf-touching orthogonal pairs in which both cells
    // survived: pairs over bare air say nothing about the pattern, pairs of one are the
    // count, pairs of two are the clustering case.
    let mut pairs = [0u64; 3]; // x, y, z adjacency
    let mut both = [0u64; 3];
    // Sample where the canopy actually is (unarmed counts -- the holes do not move the
    // forest, only the representation of it), then measure the armed field on those chunks.
    for key in coarse_keys(&WorldGen::new(A5_SEED)) {
        let d = gen_chunk(&on, key);
        for x in 0..64usize {
            for z in 0..64usize {
                for y in 0..64usize {
                    let here = x + z * 64 + y * 64 * 64;
                    for (axis, there) in [(0usize, here + 1), (1, here + 64 * 64), (2, here + 64)] {
                        if (axis == 0 && x == 63) || (axis == 1 && y == 63) || (axis == 2 && z == 63) {
                            continue;
                        }
                        let l_lo = d[here] == LEAVES;
                        let l_hi = d[there] == LEAVES;
                        if !l_lo && !l_hi {
                            continue;
                        }
                        // `there` is always the higher-addressed neighbour, so each
                        // touching pair is counted exactly once.
                        pairs[axis] += 1;
                        both[axis] += (l_lo && l_hi) as u64;
                    }
                }
            }
        }
    }
    assert!(
        pairs.iter().all(|&p| p > 1000),
        "too few pairs to read after canopy-located sampling: {pairs:?} -- the window \
         search says the canopy vanished, not the margin"
    );
    let cov = 0.62f64;
    let coin = cov * cov / (1.0 - (1.0 - cov) * (1.0 - cov)); // 0.4493
    let shared = |axis: usize| both[axis] as f64 / pairs[axis] as f64;
    assert!(
        shared(2) < coin * 0.8,
        "along z the survivors cluster like white noise ({:.3} of touching pairs share \
         both cells; 94's field read was 0.279 on one canopy slice, the coin would read \
         {coin:.3}; the bound sits at 0.8x the coin so a different canopy window keeps \
         its margin without drifting into the coin's own territory)",
        shared(2)
    );
    assert!(
        shared(0) < coin,
        "along x the field must at least beat the coin ({:.3} vs {coin:.3})",
        shared(0)
    );
    // The third chunk-pixel axis is not a share at all at LOD 2: `r = 2 / s` is 0 and
    // `canopy = 4 / s` is 1 at stride 4, so a proxy is exactly one cell, every tree owns
    // one column, and a leaf's vertical neighbour cannot also be a leaf -- no vertical
    // pair can survive both cells whatever the hash does (batch 97's quoted 0.6366
    // therefore could never have come from stride-4 chunk pixels; it was measured on
    // the field, which is where the lattice below re-earns it). The fork pinned the
    // same construction from the other side at its second gate: a nonzero `both` is
    // precisely the loud signal that `coarse_trees`' shape constants moved.
    assert_eq!(
        both[1], 0,
        "a LOD-2 proxy is exactly one cell (r = 2/s == 0, canopy = 4/s == 1): a surviving \
         vertical leaf pair means the shape constants moved, not the rank field"
    );

    // Batch 94's blue-in-plan / brown-in-profile claim lives on the rank field, so
    // re-earn it there: gate `canopy_rank3` against `COARSE_CANOPY_KEEP` on a
    // world-coordinate lattice at the same step 4, and count the touching-pair shares
    // per axis. Window base cannot move the figures -- the field is
    // `frac(ax + by + cz + phase)`, translation splits into the phase.
    let mut fpairs = [0u64; 3];
    let mut fboth = [0u64; 3];
    const STEP: i32 = 4; // the LOD-2 world step 94's measurement was taken at
    for wx in 0..24i32 {
        for wy in 0..24i32 {
            for wz in 0..24i32 {
                let (vx, vy, vz) = (wx * STEP, wy * STEP, wz * STEP);
                let here = canopy_rank3(vx, vy, vz, A5_SEED) < COARSE_CANOPY_KEEP;
                for (axis, (dx, dy, dz)) in
                    [(0usize, (STEP, 0, 0)), (1, (0, STEP, 0)), (2, (0, 0, STEP))]
                {
                    let there =
                        canopy_rank3(vx + dx, vy + dy, vz + dz, A5_SEED) < COARSE_CANOPY_KEEP;
                    if !here && !there {
                        continue;
                    }
                    fpairs[axis] += 1;
                    fboth[axis] += (here && there) as u64;
                }
            }
        }
    }
    let fshared = |axis: usize| fboth[axis] as f64 / fpairs[axis] as f64;
    assert!(
        fshared(1) > coin,
        "along y the *field* re-correlates, and 94 pinned that as a property ({:.3} vs \
         coin {coin:.3}, 94 read 0.637); if this flips, the rank's hash landscape changed",
        fshared(1)
    );
    assert!(
        fshared(0) < coin && fshared(2) < coin,
        "the plan-view properties live on the field too ({:.3} along x, {:.3} along z, \
         coin {coin:.3}; this lattice reads 0.391 on both, about 13 sigma of headroom -- \
         94's 0.279 z figure was a single canopy slice, not the field)",
        fshared(0),
        fshared(2)
    );
}

