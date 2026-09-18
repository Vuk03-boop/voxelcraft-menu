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

#[test]
fn surface_is_the_biomes_own_block() {
    let gen = WorldGen::new(1337);
    let mut checked = 0;
    let mut seen = [0u32; biome::BIOME_COUNT];

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

#[test]
fn coarse_canopy_density_matches_lod0() {
    use voxelcraft::block::{AIR, LEAVES};
    use voxelcraft::voxel::dense_index;
    let gen = WorldGen::new(1337);
    let mut dense = vec![0u16; VOL];
    let mut heights = Box::new([0i32; 64 * 64]);
    let mut cover = [0f64; 4];
    for lod in 0..=3u8 {

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

#[test]
fn per_biome_canopy_matches_lod0() {
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

#[test]
fn trees_grow_on_every_biomes_own_surface() {
    let gen = WorldGen::new(1337);
    let mut dense = vec![AIR; VOL];
    let mut heights = Box::new([0i32; 64 * 64]);
    let mut trunks = [0u64; biome::BIOME_COUNT];
    let mut eligible = [0u64; biome::BIOME_COUNT];

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

const A5_SEED: i32 = 90210;

fn coarse_keys(gen: &WorldGen) -> Vec<ChunkKey> {

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

    assert!(
        (0.45..=0.85).contains(&ratio),
        "decimated share {ratio:.3} of {off_leaves}..{on_leaves} leaves is not near LEAF_FILL"
    );

    let key = ChunkKey::new(2, IVec3::new(1, 0, 0));
    assert_eq!(gen_chunk(&on, key), gen_chunk(&on, key));
}

#[test]
fn a5_rank_field_is_not_white_noise() {

    let mut on = WorldGen::new(A5_SEED);
    on.tree_blue_noise = true;

    let mut pairs = [0u64; 3];
    let mut both = [0u64; 3];

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
    let coin = cov * cov / (1.0 - (1.0 - cov) * (1.0 - cov));
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

    assert_eq!(
        both[1], 0,
        "a LOD-2 proxy is exactly one cell (r = 2/s == 0, canopy = 4/s == 1): a surviving \
         vertical leaf pair means the shape constants moved, not the rank field"
    );

    let mut fpairs = [0u64; 3];
    let mut fboth = [0u64; 3];
    const STEP: i32 = 4;
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
