//! How much a block's face looks like its neighbour's -- scored on the atlas, not on a frame.
//!
//! **This is the instrument, and `coherence` in the fixture is the sanity check on it.** The
//! two answer the same question at different ends. `harness compare` reads a rendered frame,
//! where the quantity is diluted by everything else in the picture: at `default/slope` the
//! whole of batch 36 was worth 1.2053x, because the lattice the eye reads there is the
//! terrain's staircase and the texture's phase together and only one of them moved -- and it
//! is worth **1.0755x** since batch 38 carved the canopy out from under it. Nothing can be
//! *ranked* at either number. Here there is no perspective, no geometry, no haze and no crop -- two
//! blocks show the same layer under two draws of the same hash, and the correlation between
//! what they show is a closed form in the atlas and `block::PERM_CLASS`.
//!
//! `tests/waves.rs` is the precedent and the shape is deliberately its: a number about a
//! *field*, re-derived from the constants actually compiled in, printed so a later session can
//! quote it, and asserted against the thing it is quoted about. That file exists because three
//! documents once disagreed about the wave field's autocorrelation, each having run an FFT
//! over a window nobody recorded. The same trap is open here and wider, because an image-space
//! estimator of this quantity has a crop, a band, a tile and an annulus in it and every one of
//! them is a choice.
//!
//! **What it is for.** Both halves of roadmap item 36b are ranking questions this answers
//! directly and a picture answers badly:
//!
//! - *N procedural variants per material.* A variant set is a set of images exactly as a
//!   permutation orbit is, so [`repeat_score`] takes it unchanged and the two land on one axis.
//! - *The stone seam.* The thirteen layers still at `perm::NONE` score exactly 1 here, which is
//!   what "in phase, forever" means as a number. A world-space planar projection would replace
//!   that 1 with the field's own correlation at a lag of `TEX_SIZE`.

use voxelcraft::block::{self, perm, tex, PERM_CLASS};
use voxelcraft::textures::{self, MIP_LEVELS, TEX_SIZE};

/// The states a class draws from, as the hash bits that select them.
///
/// `permute_uv` reads bit 0 as the transpose, bit 1 as the u mirror and bit 2 as the v mirror,
/// and gates the first and third on `D4` -- so `FLIP_U` has two states and not eight, and the
/// two are `h & 2`. Anything else in the word is ignored by the shader and has to be ignored
/// here, or this scores a larger group than the frame draws from.
fn states(pclass: u32) -> Vec<u32> {
    match pclass {
        perm::NONE => vec![0],
        perm::FLIP_U => vec![0, 2],
        perm::D4 => (0..8).collect(),
        other => panic!("no such permutation class: {other}"),
    }
}

/// One layer under one state, as the shader would sample it.
///
/// A transliteration of `permute_uv`, and exact rather than approximate for the reason batch 36
/// records at its definition: the tiles are 16x16 and the sampler magnifies with `Nearest`, so
/// `1 - (x + 0.5) / 16` lands exactly on texel `15 - x` and a dihedral map is a bijection of
/// the texel grid with no resampling in it. That is what lets this be a permutation of bytes
/// rather than a render.
fn permuted(layer: &[u8], size: u32, pclass: u32, h: u32) -> Vec<u8> {
    let d4 = pclass == perm::D4;
    let mut out = vec![0u8; layer.len()];
    for y in 0..size {
        for x in 0..size {
            let (mut sx, mut sy) = (x, y);
            if d4 && (h & 1) != 0 {
                std::mem::swap(&mut sx, &mut sy);
            }
            if pclass != perm::NONE && (h & 2) != 0 {
                sx = size - 1 - sx;
            }
            if d4 && (h & 4) != 0 {
                sy = size - 1 - sy;
            }
            let d = ((y * size + x) * 4) as usize;
            let s = ((sy * size + sx) * 4) as usize;
            out[d..d + 4].copy_from_slice(&layer[s..s + 4]);
        }
    }
    out
}

/// Pearson correlation between two layers, **with each channel centred on its own mean**.
///
/// The centring is the whole of the difference between a metric and a mistake here, and the
/// first version of this file got it wrong: centre all three channels on one global mean and
/// most of the variance in the vector is the *palette* -- dirt's red sitting 50 code values
/// above its blue -- which no permutation touches. Every `D4` layer then scored 0.88 to 0.94
/// and the table said the dihedral group buys almost nothing, which is false and would have
/// been quoted. It is `rg_std`'s defect exactly: two quantities charged at once, and the one
/// that moves is the smaller.
///
/// **Alpha is excluded, and that is not tidiness**: the atlas's alpha is a tint mask rather
/// than opacity, so including it would score how alike two tint masks are -- a question about
/// the biome field and not about how the block looks. `mae` excludes it for the same reason.
fn corr(a: &[u8], b: &[u8]) -> f64 {
    let px = a.len() / 4;
    let centred = |s: &[u8]| -> Vec<f64> {
        let mut v = Vec::with_capacity(px * 3);
        for c in 0..3 {
            let mean =
                (0..px).map(|i| s[i * 4 + c] as f64).sum::<f64>() / px as f64;
            v.extend((0..px).map(|i| s[i * 4 + c] as f64 - mean));
        }
        v
    };
    let (x, y) = (centred(a), centred(b));
    let mut sxy = 0.0;
    let mut sxx = 0.0;
    let mut syy = 0.0;
    for i in 0..x.len() {
        sxy += x[i] * y[i];
        sxx += x[i] * x[i];
        syy += y[i] * y[i];
    }
    // A layer of one flat colour has nothing to be alike or unalike, and two of them are
    // perfectly alike by any reading anybody would want. 1 is the honest answer; a division
    // would return the rounding.
    if sxx <= 1e-9 || syy <= 1e-9 {
        return 1.0;
    }
    sxy / (sxx * syy).sqrt()
}

/// **The score.** Mean correlation between what two independent neighbours draw from `set`.
///
/// The diagonal is in it, because two neighbours may draw the same state and when they do the
/// face really is identical -- dropping it would score a world where a block is forbidden to
/// repeat its neighbour, which is not the world. That is also the floor: a set of `N` mutually
/// uncorrelated images scores `1/N` and never 0, which is the number to compare a variant count
/// against rather than against zero.
///
/// One image scores exactly 1. That is `perm::NONE`, it is thirteen of the twenty-one layers,
/// and it is what "every block samples the identical 0..1 of its layer, in phase, forever"
/// says as a number.
fn repeat_score(set: &[Vec<u8>]) -> f64 {
    let n = set.len();
    let mut sum = 0.0;
    for a in set {
        for b in set {
            sum += corr(a, b);
        }
    }
    sum / (n * n) as f64
}

fn layer_of(atlas: &[u8], layer: u32, size: u32) -> Vec<u8> {
    let l = layer as usize;
    let bytes = (size * size * 4) as usize;
    atlas[l * bytes..(l + 1) * bytes].to_vec()
}

/// The orbit of one layer under its own class, at mip `level`.
fn orbit(atlas: &[u8], layer: u32, size: u32) -> Vec<Vec<u8>> {
    let pclass = PERM_CLASS[layer as usize];
    let src = layer_of(atlas, layer, size);
    states(pclass)
        .into_iter()
        .map(|h| permuted(&src, size, pclass, h))
        .collect()
}

// ---------------------------------------------------------------------- the table

/// Every layer's repeat score at mip 0, printed. **This is the table 36b exists to produce**,
/// and the assertions under it are the three claims batch 36 made in prose.
#[test]
fn the_repeat_score_of_every_layer() {
    let atlas = textures::build_atlas(0.0, false);
    println!("| layer | class | states | repeat |");
    println!("|---|---|---|---|");
    let mut none = Vec::new();
    let mut flip = Vec::new();
    let mut d4 = Vec::new();
    for layer in 0..tex::COUNT {
        let pclass = PERM_CLASS[layer as usize];
        let set = orbit(&atlas, layer, TEX_SIZE);
        let s = repeat_score(&set);
        let name = match pclass {
            perm::NONE => "NONE",
            perm::FLIP_U => "FLIP_U",
            _ => "D4",
        };
        println!("| {layer} | {name} | {} | {s:.4} |", set.len());
        match pclass {
            perm::NONE => none.push((layer, s)),
            perm::FLIP_U => flip.push((layer, s)),
            _ => d4.push((layer, s)),
        }
    }

    // 1. Every layer left alone repeats perfectly, by construction rather than by measurement
    //    -- which is the point. A future batch that gives one of them a class will move it
    //    off this list, and that is the batch the stone-seam question turns into.
    //
    //    **Counted against the classed layers and not written down, because the literal went
    //    stale the first time it could.** This read `13` from batch 36 until batch 59 added
    //    three emissive layers, at which point a test whose subject is the *permutation*
    //    table failed because the *atlas* had grown -- which says nothing about either. The
    //    nine below is the claim worth pinning: **two `FLIP_U` and seven `D4`** -- batch 92
    //    made the pair of sides (`GRASS_SIDE`, `MEADOW_SIDE`) both `FLIP_U` for `GRASS_SIDE`'s
    //    own reason -- and a batch that classes a new layer has to come here and say so.
    assert_eq!(
        none.len(),
        tex::COUNT as usize - 9,
        "a layer changed permutation class without this census being told"
    );
    for (layer, s) in &none {
        assert_eq!(*s, 1.0, "layer {layer} is at NONE and scores {s:.4}");
    }

    // 2. `GRASS_SIDE` is the layer the whole batch was about, and it takes the smaller group.
    //    Batch 92 put `MEADOW_SIDE` in beside it, for the same banded reason.
    assert_eq!(flip.len(), 2);
    let grass = flip[0].1;
    assert!(
        grass < 1.0,
        "GRASS_SIDE scores {grass:.4}: its mirror is not changing the picture at all"
    );

    // 3. The seven isotropic ones take the full group, and the group is what buys the variety:
    //    a set of eight uncorrelated images floors at 1/8, so anything near that is as far as
    //    eight states can go.
    assert_eq!(d4.len(), 7);
    for (layer, s) in &d4 {
        assert!(
            *s < 0.5,
            "layer {layer} takes D4 and still scores {s:.4}, so the eight states are not \
             eight different pictures"
        );
    }
}

/// **The finding the frame could only hint at.** `FLIP_U` removes far less of the repeat than
/// `D4` does, and the gap is not small.
///
/// A mirror in u leaves every row of the texture where it was: `GRASS_SIDE`'s green band stays
/// at the top, its soil stays at the bottom, and what alternates is only the fringe profile
/// between them. That is the whole of what batch 36 bought on the layer it was aimed at, and
/// it is the number a variant batch has to beat rather than the 0.13 the isotropic layers
/// already sit at.
///
/// **What this table does not say is which layer a frame control actually moves, and batch 39
/// found that the obvious reading of it was backwards.** This doc used to finish "which is why
/// `--no-tex-variation` is worth 1.20x on a frame and not 5x", crediting `GRASS_SIDE`'s 0.84
/// as the limiter. The frame says otherwise: 79% of that control's response at `default/slope`
/// came from the tiles holding tree canopy. The swing a control produces is the distance from
/// the permuted score *to 1.0*, so a `D4` layer at 0.14 swings six times as far as a `FLIP_U`
/// layer at 0.84 -- and `LEAVES` is `D4` and covered a great deal of that frame. **The score
/// here times the area the layer covers is the prediction; the score alone is not.**
#[test]
fn a_mirror_removes_less_than_the_dihedral_group() {
    let atlas = textures::build_atlas(0.0, false);
    let grass = repeat_score(&orbit(&atlas, tex::GRASS_SIDE, TEX_SIZE));
    let dirt = repeat_score(&orbit(&atlas, tex::DIRT, TEX_SIZE));
    println!("GRASS_SIDE (FLIP_U) {grass:.4}, DIRT (D4) {dirt:.4}");
    assert!(
        grass > dirt,
        "GRASS_SIDE scores {grass:.4} against DIRT's {dirt:.4}; the claim that the bounded \
         layer keeps most of its repeat no longer holds and the roadmap's ranking of a variant \
         set against it is unsupported"
    );
    // Two states can do no better than the half of itself each pair of them shares, so this is
    // the ceiling a mirror could ever have reached. A `FLIP_U` layer at the floor would mean
    // the mirror image was unrelated to the original, which for a bounded texture would be a
    // new defect rather than a success.
    assert!(
        grass >= 0.5,
        "GRASS_SIDE scores {grass:.4}, below the 0.5 floor of a two-state set: its mirror is \
         anticorrelated with it, which is not variety"
    );
}

/// What distance does to it: **nothing, and the prediction this test was written to check
/// was the opposite.**
///
/// It was written asserting that the score must climb with the mip level -- that a filter takes
/// away the detail which told two states apart, so the repeat comes back at range. It does not,
/// and the mechanism says why: the `D4` layers are per-texel noise, a 2x2 box average of
/// independent noise is independent noise at the coarser scale, and the correlation between a
/// noise field and its own transpose is about zero at every scale. What *does* grow is the
/// sampling error on that zero, because mip 3 is four texels.
///
/// So the claim is the one the mechanism supports: the score stays at the `1/N` floor of an
/// eight-state set, to within the standard error of the texels left at that level. **That is
/// batch 36's "it commutes with `downsample`" as a measurement rather than as an argument**, and
/// it is the half of the question a still capture at one vantage cannot ask.
#[test]
fn the_variety_survives_minification() {
    let mut atlas = textures::build_atlas(0.0, false);
    let mut size = TEX_SIZE;
    for level in 0..MIP_LEVELS {
        if level > 0 {
            atlas = textures::downsample(&atlas, size, tex::COUNT);
            size /= 2;
        }
        let d4: Vec<f64> = (0..tex::COUNT)
            .filter(|l| PERM_CLASS[*l as usize] == perm::D4)
            .map(|l| repeat_score(&orbit(&atlas, l, size)))
            .collect();
        let mean = d4.iter().sum::<f64>() / d4.len() as f64;
        // Three channels of `size * size` texels behind each correlation, so the off-diagonal
        // terms carry about this much noise each. Four of them is the width of a claim that
        // the floor is still the floor, not a claim about the third decimal.
        let se = 4.0 / ((size * size * 3) as f64).sqrt();
        println!(
            "mip {level} ({size}x{size}): D4 mean repeat {mean:.4}, floor 0.1250, tolerance {se:.4}"
        );
        assert!(
            (mean - 0.125).abs() <= se,
            "mip {level} scores {mean:.4} against the 0.1250 floor of eight states, outside the {se:.4} the texel count allows -- the permutation is not surviving the filter"
        );
    }
}

// ---------------------------------------------------------------------- the hash

/// `hash_u32` from `common.wgsl`.
fn hash_u32(x: u32) -> u32 {
    let mut h = x;
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^= h >> 16;
    h
}

/// `perm_key` from `resolve.wgsl`: the integer world corner of the voxel, and the face.
fn perm_key(c: [i32; 3], normal_id: u32) -> u32 {
    hash_u32(
        (c[0] as u32).wrapping_mul(0x9e37_79b9)
            ^ (c[1] as u32).wrapping_mul(0x85eb_ca6b)
            ^ (c[2] as u32).wrapping_mul(0xc2b2_ae35)
            ^ normal_id.wrapping_mul(0x27d4_eb2f),
    )
}

/// **The score above assumes two neighbours draw independently and uniformly. This is the
/// assumption, measured.**
///
/// It is the one place the closed form could be quietly wrong: a hash that favoured a state,
/// or that gave a block and the block beside it the same three bits more often than chance,
/// would leave the world more repetitive than the table says while every number in it stayed
/// true of the atlas. So the same quantity is read a second way -- the empirical mean
/// correlation over real adjacent block pairs, with the states the hash actually hands out --
/// and the two have to agree.
#[test]
fn the_hash_is_uniform_and_neighbours_are_independent() {
    const N: i32 = 40;
    const FACE: u32 = 2;
    let mut joint = [[0u32; 8]; 8];
    let mut marginal = [0u32; 8];
    let mut total = 0u32;
    for x in -N..N {
        for y in -N..N {
            for z in -N..N {
                let a = (perm_key([x, y, z], FACE) & 7) as usize;
                let b = (perm_key([x + 1, y, z], FACE) & 7) as usize;
                joint[a][b] += 1;
                marginal[a] += 1;
                total += 1;
            }
        }
    }
    let n = total as f64;
    for (s, count) in marginal.iter().enumerate() {
        let f = *count as f64 / n;
        assert!(
            (f - 0.125).abs() < 0.01,
            "state {s} comes up {:.4} of the time against a uniform 0.125",
            f
        );
    }
    let same = (0..8).map(|i| joint[i][i]).sum::<u32>() as f64 / n;
    println!("adjacent blocks share a state {same:.4} of the time (independent: 0.1250)");
    assert!(
        (same - 0.125).abs() < 0.01,
        "adjacent blocks share a state {same:.4} of the time, so they are not independent and \
         the closed form overstates the variety"
    );

    // And the quantity itself, read off the hash instead of off the group. The two are the
    // same number only if the two assumptions above hold, which is the point of computing it
    // twice.
    let atlas = textures::build_atlas(0.0, false);
    let src = layer_of(&atlas, tex::DIRT, TEX_SIZE);
    let sets: Vec<Vec<u8>> = (0..8)
        .map(|h| permuted(&src, TEX_SIZE, perm::D4, h))
        .collect();
    let mut empirical = 0.0;
    for a in 0..8 {
        for b in 0..8 {
            empirical += joint[a][b] as f64 / n * corr(&sets[a], &sets[b]);
        }
    }
    let closed = repeat_score(&sets);
    println!("DIRT: empirical {empirical:.4}, closed form {closed:.4}");
    assert!(
        (empirical - closed).abs() < 0.01,
        "the hash gives {empirical:.4} where the uniform closed form gives {closed:.4}"
    );
}

/// The cross-boundary guard, in the shape `tests/textures.rs` already uses for this pair.
///
/// Everything above is a transliteration of `permute_uv`, and a transliteration that has
/// drifted from its original scores a group the frame does not draw from. The bit assignment
/// is the part that would drift silently -- swapping which bit gates the transpose changes
/// nothing about `FLIP_U`'s *count* and everything about which two states it is.
#[test]
fn the_state_bits_are_the_shaders() {
    let src = voxelcraft::render::shader_source();
    let fun = src
        .split("fn permute_uv")
        .nth(1)
        .expect("permute_uv is gone from the module");
    let body = &fun[..fun.find("\n}").expect("unterminated permute_uv")];
    assert!(
        body.contains("d4 && (h & 1u) != 0u"),
        "bit 0 no longer gates the transpose"
    );
    assert!(
        body.contains("pclass != PERM_NONE && (h & 2u) != 0u"),
        "bit 1 no longer gates the u mirror, which is the only bit FLIP_U has"
    );
    assert!(
        body.contains("d4 && (h & 4u) != 0u"),
        "bit 2 no longer gates the v mirror"
    );
    // And that the class the table hands out is the class the packing carries, which is the
    // hazard `face_layer_at_every_call_site` was written for.
    assert_eq!(
        block::pack_face(tex::GRASS_SIDE) >> block::FACE_LAYER_BITS,
        perm::FLIP_U
    );
}



