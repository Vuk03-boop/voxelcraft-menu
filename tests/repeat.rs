use voxelcraft::block::{self, perm, tex, PERM_CLASS};
use voxelcraft::textures::{self, MIP_LEVELS, TEX_SIZE};

fn states(pclass: u32) -> Vec<u32> {
    match pclass {
        perm::NONE => vec![0],
        perm::FLIP_U => vec![0, 2],
        perm::D4 => (0..8).collect(),
        other => panic!("no such permutation class: {other}"),
    }
}

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

    if sxx <= 1e-9 || syy <= 1e-9 {
        return 1.0;
    }
    sxy / (sxx * syy).sqrt()
}

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

fn orbit(atlas: &[u8], layer: u32, size: u32) -> Vec<Vec<u8>> {
    let pclass = PERM_CLASS[layer as usize];
    let src = layer_of(atlas, layer, size);
    states(pclass)
        .into_iter()
        .map(|h| permuted(&src, size, pclass, h))
        .collect()
}

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

    assert_eq!(
        none.len(),
        tex::COUNT as usize - 9,
        "a layer changed permutation class without this census being told"
    );
    for (layer, s) in &none {
        assert_eq!(*s, 1.0, "layer {layer} is at NONE and scores {s:.4}");
    }

    assert_eq!(flip.len(), 2);
    let grass = flip[0].1;
    assert!(
        grass < 1.0,
        "GRASS_SIDE scores {grass:.4}: its mirror is not changing the picture at all"
    );

    assert_eq!(d4.len(), 7);
    for (layer, s) in &d4 {
        assert!(
            *s < 0.5,
            "layer {layer} takes D4 and still scores {s:.4}, so the eight states are not \
             eight different pictures"
        );
    }
}

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

    assert!(
        grass >= 0.5,
        "GRASS_SIDE scores {grass:.4}, below the 0.5 floor of a two-state set: its mirror is \
         anticorrelated with it, which is not variety"
    );
}

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

fn hash_u32(x: u32) -> u32 {
    let mut h = x;
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^= h >> 16;
    h
}

fn perm_key(c: [i32; 3], normal_id: u32) -> u32 {
    hash_u32(
        (c[0] as u32).wrapping_mul(0x9e37_79b9)
            ^ (c[1] as u32).wrapping_mul(0x85eb_ca6b)
            ^ (c[2] as u32).wrapping_mul(0xc2b2_ae35)
            ^ normal_id.wrapping_mul(0x27d4_eb2f),
    )
}

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

    assert_eq!(
        block::pack_face(tex::GRASS_SIDE) >> block::FACE_LAYER_BITS,
        perm::FLIP_U
    );
}
