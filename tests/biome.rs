#![allow(clippy::int_plus_one)]
#![allow(clippy::manual_is_multiple_of)]
#![allow(clippy::manual_range_contains)]

use glam::IVec3;
use voxelcraft::biome::{self, BiomeId};
use voxelcraft::block::*;
use voxelcraft::voxel::*;
use voxelcraft::worldgen::{WorldGen, SEA_LEVEL};

fn world_hash(gen: &WorldGen) -> u64 {
    let mut dense = vec![AIR; VOL];
    let mut heights = Box::new([0i32; 64 * 64]);
    let mut keys = Vec::new();
    for &cx in &[0i32, 8, 16] {
        for &cz in &[0i32, 8, 16] {
            for cy in 1..3 {
                keys.push(ChunkKey::new(0, IVec3::new(cx, cy, cz)));
            }
        }
    }
    for &(lod, c) in &[
        (1u8, IVec3::new(0, 1, 0)),
        (2, IVec3::new(0, 0, 0)),
        (2, IVec3::new(1, 0, 1)),
        (3, IVec3::new(0, 0, 0)),
    ] {
        keys.push(ChunkKey::new(lod, c));
    }
    let mut h: u64 = 0xcbf29ce484222325;
    for key in keys {
        gen.generate(key, &mut dense, &mut heights);
        for &b in dense.iter() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    h
}

#[test]
fn no_biomes_reproduces_the_pre_batch9_world() {
    const PRE_BATCH9: u64 = 0xe70d077e75656998;
    let off = WorldGen::with_options(1337, SEA_LEVEL, false, true, true);
    assert_eq!(
        world_hash(&off),
        PRE_BATCH9,
        "--no-biomes no longer reproduces the world batch 9 started from"
    );
    let on = WorldGen::with_options(1337, SEA_LEVEL, true, true, true);
    assert_ne!(
        world_hash(&on),
        PRE_BATCH9,
        "the biome field changed nothing; the control is measuring itself"
    );
}

#[test]
fn no_foliage_reproduces_the_pre_batch14_world() {
    const PRE_BATCH14: u64 = 0xbc5c5c4e9b53c3c3;
    let off = WorldGen::with_options(1337, SEA_LEVEL, true, false, true);
    assert_eq!(
        world_hash(&off),
        PRE_BATCH14,
        "--no-foliage no longer reproduces the world batch 14 started from"
    );
    let on = WorldGen::with_options(1337, SEA_LEVEL, true, true, true);
    assert_ne!(
        world_hash(&on),
        PRE_BATCH14,
        "ground cover changed nothing; the control is measuring itself"
    );
}

#[test]
fn no_meadow_reproduces_the_pre_batch18_world() {
    const PRE_BATCH18: u64 = 0x6da77cbbe6381ccb;
    let off = WorldGen::with_options(1337, SEA_LEVEL, true, true, false);
    assert_eq!(
        world_hash(&off),
        PRE_BATCH18,
        "--no-meadow no longer reproduces the world batch 18 started from"
    );
    let on = WorldGen::with_options(1337, SEA_LEVEL, true, true, true);
    assert_ne!(
        world_hash(&on),
        PRE_BATCH18,
        "the coarse meadow changed nothing; the control is measuring itself"
    );
}

#[test]
fn ground_cover_follows_the_biome_and_not_the_block() {
    let gen = WorldGen::new(1337);
    let mut cover = [0u32; biome::BIOME_COUNT];
    let mut columns = [0u32; biome::BIOME_COUNT];
    let mut dense = vec![AIR; VOL];
    let mut heights = Box::new([0i32; 64 * 64]);
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
            gen.generate(key, &mut dense, &mut heights);
            let o = key.origin();
            for z in 0..64usize {
                for x in 0..64usize {
                    let h = heights[x + z * 64];
                    let ly = h - o.y;
                    if ly < 1 || ly >= 64 {
                        continue;
                    }
                    let cb = gen.column_biome(o.x + x as i32, o.z + z as i32);

                    if h <= SEA_LEVEL + 1 || h > cb.tree_line {
                        continue;
                    }
                    if dense[dense_index(x, ly as usize - 1, z)] != cb.def().surface {
                        continue;
                    }
                    columns[cb.id as usize] += 1;
                    if dense[dense_index(x, ly as usize, z)] == TALL_GRASS {
                        cover[cb.id as usize] += 1;
                    }
                }
            }
        }
    }
    for id in 0..biome::BIOME_COUNT {
        if columns[id] < 200 {
            continue;
        }
        let b = biome::def(id as biome::BiomeId);
        let rate = cover[id] as f32 / columns[id] as f32;
        if b.grass_scale <= 0.0 {
            assert_eq!(
                cover[id], 0,
                "{} has grass_scale 0 but {} of {} eligible columns grew cover",
                b.name, cover[id], columns[id]
            );
        } else {
            assert!(
                rate > 0.15,
                "{} has grass_scale {} but only {:.1}% of {} eligible columns grew cover",
                b.name,
                b.grass_scale,
                rate * 100.0,
                columns[id]
            );
        }
    }
}

#[test]
fn the_biome_field_survives_stride_8() {
    let gen = WorldGen::new(1337);
    let n = 512usize;

    let survey = |stride: i32| -> (f64, f64) {
        let field: Vec<BiomeId> = (0..n * n)
            .map(|i| {
                let x = (i % n) as i32 * stride - (n as i32 * stride) / 2;
                let z = (i / n) as i32 * stride - (n as i32 * stride) / 2;
                gen.biome_at(x, z)
            })
            .collect();
        let (mut edges, mut isolated, mut interior) = (0u64, 0u64, 0u64);
        for z in 1..n - 1 {
            for x in 1..n - 1 {
                let me = field[x + z * n];
                let nb = [
                    field[x - 1 + z * n],
                    field[x + 1 + z * n],
                    field[x + (z - 1) * n],
                    field[x + (z + 1) * n],
                ];
                interior += 1;
                if nb.iter().any(|&b| b != me) {
                    edges += 1;
                }
                if nb.iter().all(|&b| b != me) {
                    isolated += 1;
                }
            }
        }
        (
            edges as f64 / interior as f64,
            isolated as f64 / interior as f64,
        )
    };
    let (fine, _) = survey(1);
    let (coarse, speckle) = survey(8);
    println!(
        "  boundary cells: {:.3}% at stride 1, {:.3}% at stride 8 ({:.1}x), speckle {:.4}%",
        fine * 100.0,
        coarse * 100.0,
        coarse / fine,
        speckle * 100.0
    );
    assert!(
        (4.0..12.0).contains(&(coarse / fine)),
        "the boundary fraction grew {:.1}x when the stride grew 8x ({:.3}% -> {:.3}%);          a resolved boundary scales linearly, a saturating one is aliasing",
        coarse / fine,
        fine * 100.0,
        coarse * 100.0
    );

    assert!(
        speckle < 0.001,
        "{:.4}% of stride-8 samples disagree with all four neighbours; the biome field is          aliasing at the coarse lattice rather than merely cornering",
        speckle * 100.0
    );
}

#[test]
fn the_shape_blends_where_the_palette_snaps() {
    let gen = WorldGen::new(1337);
    let mut borders = 0u64;
    let mut worst_amp = 0.0f32;
    let mut worst_snow = 0;
    let mut worst_at_border = 0;
    let mut worst_anywhere = 0;
    for z in (-4000..4000i32).step_by(9) {
        for x in -4000..4000i32 {
            let step = (gen.height(x, z) - gen.height(x + 1, z)).abs();
            worst_anywhere = worst_anywhere.max(step);
            let a = gen.column_biome(x, z);
            let b = gen.column_biome(x + 1, z);
            if a.id == b.id {
                continue;
            }
            borders += 1;
            worst_amp = worst_amp.max((a.mountain_amp - b.mountain_amp).abs());
            worst_snow = worst_snow.max((a.snow_line - b.snow_line).abs());
            worst_at_border = worst_at_border.max(step);
        }
    }
    let spread = biome::BIOMES
        .iter()
        .map(|b| b.mountain_amp)
        .fold(0.0f32, f32::max)
        - biome::BIOMES
            .iter()
            .map(|b| b.mountain_amp)
            .fold(f32::MAX, f32::min);
    println!(
        "  {borders} palette borders: worst amplitude step {worst_amp:.3} of a {spread:.0} spread, \
snow line {worst_snow}, terrain {worst_at_border} against {worst_anywhere} anywhere"
    );
    assert!(borders > 10_000, "only {borders} borders sampled");
    assert!(
        worst_amp < spread * 0.02,
        "mountain amplitude jumps {worst_amp:.2} across a palette border; the shape is \
         following the threshold instead of the blend"
    );
    assert!(
        worst_snow <= 2,
        "the snow line steps {worst_snow} blocks across a palette border"
    );

    assert!(
        worst_at_border <= worst_anywhere,
        "terrain steps {worst_at_border} blocks across a palette border against          {worst_anywhere} anywhere; the border is a wall"
    );
}

#[test]
fn band_weights_partition_the_field() {
    for i in -2000..=2000 {
        let n = i as f32 / 1000.0;
        let w = biome::band_weights(n);
        let sum: f32 = w.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5, "weights at {n} sum to {sum}");
        assert!(
            w.iter().all(|&x| (0.0..=1.0).contains(&x)),
            "weights at {n} are {w:?}"
        );

        assert!(
            w[biome::band(n)] >= 0.5 - 1e-6,
            "at {n} the palette says band {} but it carries only {:.3} of the weight",
            biome::band(n),
            w[biome::band(n)]
        );
    }

    let w = biome::band_weights(biome::BAND);
    assert!(
        (w[1] - 0.5).abs() < 1e-5 && (w[2] - 0.5).abs() < 1e-5,
        "at the threshold: {w:?}"
    );
}

#[test]
fn the_surface_palette_agrees_across_lods() {
    let gen = WorldGen::new(1337);
    let mut dense = vec![AIR; VOL];
    let mut heights = Box::new([0i32; 64 * 64]);
    let mut checked = [0u32; 4];
    let mut snow_seen = 0;
    let mut meadow_seen = 0;
    for lod in 0..4u8 {
        let s = 1i32 << lod;
        for &(cx, cz) in &[(0i32, 0i32), (5, -3), (-4, 6), (7, 9)] {
            for cy in 0..6 {
                let key = ChunkKey::new(lod, IVec3::new(cx >> lod, cy >> lod, cz >> lod));
                gen.generate(key, &mut dense, &mut heights);
                let o = key.origin();
                for z in (0..64usize).step_by(5) {
                    for x in (0..64usize).step_by(5) {
                        let wx = o.x + x as i32 * s + s / 2;
                        let wz = o.z + z as i32 * s + s / 2;

                        let Some(ty) = (0..64usize).rev().find(|&y| {
                            let b = dense[dense_index(x, y, z)];
                            b != AIR && b != WATER && b != LEAVES && b != PINE_LEAVES && b != LOG
                        }) else {
                            continue;
                        };
                        let h = gen.height(wx, wz);
                        let (y0, y1) = (o.y + ty as i32 * s, o.y + (ty as i32 + 1) * s - 1);

                        if h - 1 < y0 || h - 1 > y1 {
                            continue;
                        }
                        let cb = gen.column_biome(wx, wz);

                        let expected = if h <= SEA_LEVEL + 2 {
                            SAND
                        } else if h - 1 >= cb.snow_line {
                            SNOW
                        } else if h - 1 >= cb.tree_line {
                            STONE
                        } else {
                            cb.def().surface
                        };
                        if expected == SNOW {
                            snow_seen += 1;
                        }

                        let mut got = dense[dense_index(x, ty, z)];
                        if got == MEADOW {
                            assert!(
                                lod > 0 && expected == GRASS,
                                "meadow at lod {lod}, {wx},{wz}: height {h} wanted {expected}"
                            );
                            meadow_seen += 1;
                            got = GRASS;
                        }
                        assert_eq!(
                            got,
                            expected,
                            "lod {lod} at {wx},{wz}: height {h}, {} with snow line {}",
                            cb.def().name,
                            cb.snow_line
                        );
                        checked[lod as usize] += 1;
                    }
                }
            }
        }
        assert!(
            checked[lod as usize] > 200,
            "lod {lod} only checked {} columns",
            checked[lod as usize]
        );
    }
    println!(
        "  columns checked per lod: {checked:?} ({snow_seen} above a snow line,          {meadow_seen} coarse meadow)"
    );

    assert!(
        meadow_seen > 0,
        "no coarse column came back as meadow; batch 18's substitution is not running"
    );
}

#[test]
fn the_snow_line_and_tree_line_bite() {
    let gen = WorldGen::new(1337);
    let mut snowy = 0u64;
    let mut above_tree_line = 0u64;
    let mut bare_band = 0u64;
    let mut land = 0u64;
    for z in (-4000..4000i32).step_by(11) {
        for x in (-4000..4000i32).step_by(11) {
            let cb = gen.column_biome(x, z);
            let h = gen.height(x, z);
            if h <= SEA_LEVEL + 2 {
                continue;
            }
            land += 1;
            if h - 1 >= cb.snow_line {
                snowy += 1;
            }
            if h > cb.tree_line {
                above_tree_line += 1;
                if h - 1 < cb.snow_line {
                    bare_band += 1;
                }
            }

            assert!(
                cb.tree_line <= cb.snow_line,
                "at {x},{z} the tree line {} is above the snow line {}",
                cb.tree_line,
                cb.snow_line
            );
        }
    }
    println!(
        "  of {land} land columns: {:.2}% snow-capped, {:.2}% above the tree line ({bare_band} of \
         those bare rather than snowy)",
        snowy as f64 / land as f64 * 100.0,
        above_tree_line as f64 / land as f64 * 100.0
    );
    assert!(
        snowy > 0 && (snowy as f64) < land as f64 * 0.5,
        "{snowy} of {land} columns are snowy"
    );
    assert!(
        above_tree_line > 0,
        "no column anywhere is above its tree line"
    );
    assert!(
        bare_band > 0,
        "the band between the tree line and the snow line is empty"
    );
}

#[test]
fn every_biome_appears_in_the_world() {
    let gen = WorldGen::new(1337);
    let mut counts = [0u64; biome::BIOME_COUNT];
    let mut n = 0u64;
    for z in (-4000..4000i32).step_by(5) {
        for x in (-4000..4000i32).step_by(5) {
            counts[gen.biome_at(x, z) as usize] += 1;
            n += 1;
        }
    }
    for (i, &c) in counts.iter().enumerate() {
        let frac = c as f64 / n as f64;
        println!(
            "  {:8} {:5.2}%",
            biome::def(i as BiomeId).name,
            frac * 100.0
        );
        assert!(
            frac > 0.03,
            "{} covers only {:.2}% of the world",
            biome::def(i as BiomeId).name,
            frac * 100.0
        );
    }
}

#[test]
fn noise_matches_fastnoise_lite() {
    use fastnoise_lite::{FastNoiseLite, NoiseType};

    let reference = |seed: i32, freq: f32| {
        let mut n = FastNoiseLite::with_seed(seed);
        n.set_noise_type(Some(NoiseType::OpenSimplex2));
        n.set_frequency(Some(freq));
        n
    };

    const SEED: i32 = 1337;
    let cases = [
        (SEED + biome::TEMP_SEED, biome::TEMP_FREQ),
        (SEED + biome::HUMID_SEED, biome::HUMID_FREQ),

        (SEED, 0.37),
    ];

    let mut checked = 0u64;
    for &(seed, freq) in &cases {
        let r = reference(seed, freq);

        for xi in (-2_000_000..2_000_000).step_by(9_377) {
            for zi in (-2_000_000..2_000_000).step_by(9_377) {
                let (x, z) = (xi as f32, zi as f32);
                let ours = biome::simplex2(seed, freq, x, z);
                let theirs = r.get_noise_2d(x, z);
                assert_eq!(
                    ours.to_bits(),
                    theirs.to_bits(),
                    "simplex2(seed {seed}, freq {freq}) at ({x}, {z}): {ours} vs {theirs}"
                );
                checked += 1;
            }
        }
    }

    assert!(checked > 500_000, "only {checked} samples compared");
}
