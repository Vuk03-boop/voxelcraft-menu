use voxelcraft::block::{self, tex};
use voxelcraft::harness::metric::sha256_hex;
use voxelcraft::textures::{self, WATER_MOTTLE_PRE21B, TEX_SIZE};

const LAYER: usize = (TEX_SIZE * TEX_SIZE * 4) as usize;

fn layer_of(atlas: &[u8], layer: u32) -> &[u8] {
    let l = layer as usize;
    &atlas[l * LAYER..(l + 1) * LAYER]
}

#[test]
fn water_mottle_reproduces_the_pre_batch21b_atlas() {
    const PRE_BATCH21B_ATLAS: &str =
        "179583743b32bbdc3081df661b7538aca6b939ccd209b5029766e2f52d6577ac";
    const PRE_BATCH21B_WATER: &str =
        "7d80aaba25eb2a1b58fa4f693d73957b87d5cd7cc33127aec565b600036d4597";

    const LAYERS_AT_BATCH21B: usize = 21;

    let old = textures::build_atlas(WATER_MOTTLE_PRE21B, false);
    assert!(
        tex::COUNT as usize >= LAYERS_AT_BATCH21B,
        "layers were removed rather than appended, and this hash no longer means anything"
    );
    assert_eq!(
        sha256_hex(&old[..LAYERS_AT_BATCH21B * LAYER]),
        PRE_BATCH21B_ATLAS,
        "--water-mottle 0.2 no longer reproduces the atlas batch 21b started from"
    );
    assert_eq!(
        sha256_hex(layer_of(&old, tex::WATER)),
        PRE_BATCH21B_WATER,
        "the water layer moved at the amplitude that is supposed to reproduce it"
    );

    let shipping = textures::build_atlas(0.0, false);
    assert_ne!(
        sha256_hex(layer_of(&shipping, tex::WATER)),
        PRE_BATCH21B_WATER,
        "flattening the mottle changed nothing; the control is measuring itself"
    );
}

#[test]
fn the_mottle_amplitude_reaches_water_and_nothing_else() {
    let a = textures::build_atlas(0.0, false);
    let b = textures::build_atlas(WATER_MOTTLE_PRE21B, false);
    assert_eq!(a.len(), b.len());
    for layer in 0..tex::COUNT {
        let same = layer_of(&a, layer) == layer_of(&b, layer);
        if layer == tex::WATER {
            assert!(!same, "the water layer did not respond to the amplitude");
        } else {
            assert!(same, "layer {layer} moved with the water mottle amplitude");
        }
    }
}

#[test]
fn the_amplitude_barely_moves_the_layers_mean_colour() {
    let mean = |m: f32| {
        let a = textures::build_atlas(m, false);
        let w = layer_of(&a, tex::WATER);
        let mut s = [0.0f64; 3];
        for px in w.as_chunks::<4>().0 {
            for c in 0..3 {
                s[c] += metric_srgb_to_linear(px[c]) as f64;
            }
        }
        let n = (w.len() / 4) as f64;
        [s[0] / n, s[1] / n, s[2] / n]
    };
    let flat = mean(0.0);
    let old = mean(WATER_MOTTLE_PRE21B);

    let flat_texel = {
        let a = textures::build_atlas(0.0, false);
        let w = layer_of(&a, tex::WATER);
        [w[0], w[1], w[2]]
    };
    for c in 0..3 {
        let code = flat_texel[c];
        let step = (metric_srgb_to_linear(code + 1) - metric_srgb_to_linear(code)) as f64;
        let drift = (flat[c] - old[c]).abs() / step;
        assert!(
            drift <= 1.5,
            "channel {c}: flat mean {:.6} vs the mottle's {:.6}, {drift:.2} code values apart",
            flat[c],
            old[c]
        );
    }
}

#[test]
fn faces_per_block_matches_the_shader() {
    let src = voxelcraft::render::shader_source();
    let want = format!("* {}u", block::FACES_PER_BLOCK);
    let mut sites = 0;
    for (at, _) in src.match_indices("block_faces[") {
        let open = at + "block_faces[".len();
        let close = open
            + src[open..]
                .find(']')
                .expect("an unterminated block_faces[ index");
        let index = &src[open..close];
        sites += 1;
        assert!(
            index.contains(&want),
            "block_faces[{index}] does not stride by {}: that is the batch-21b bug, where \
             `shade_water` strode by 6 into a table of 8 and sampled the glass layer",
            block::FACES_PER_BLOCK
        );
    }

    assert!(
        sites >= 3,
        "found {sites} block_faces[ call sites, expected at least the three that exist"
    );
}

#[test]
fn face_layer_at_every_call_site() {
    let src = voxelcraft::render::shader_source();
    let mut sites = 0;
    let mut words = Vec::new();
    for (at, _) in src.match_indices("block_faces[") {
        sites += 1;
        let before = src[..at].trim_end();
        if before.ends_with("face_layer(") || before.ends_with("face_perm(") {
            continue;
        }

        let name = before
            .strip_suffix(" =")
            .and_then(|b| b.rsplit(|c: char| c.is_whitespace()).next())
            .unwrap_or("");
        assert!(
            name.ends_with("_word"),
            "a block_faces[ read is neither wrapped in face_layer()/face_perm() nor bound to              a `*_word` local: ...{}",
            &src[at.saturating_sub(80)..at + 40]
        );
        words.push(name.to_string());
    }

    for name in &words {
        let mut uses = 0;
        for (at, _) in src.match_indices(name.as_str()) {
            let before = src[..at].trim_end();
            if before.ends_with('=') || before.ends_with("let") {
                continue;
            }
            uses += 1;
            assert!(
                before.ends_with("face_layer(") || before.ends_with("face_perm("),
                "`{name}` is read without an accessor: ...{}",
                &src[at.saturating_sub(60)..at + 40]
            );
        }
        assert!(uses > 0, "`{name}` is bound and never read");
    }

    assert!(
        sites >= 3,
        "found {sites} block_faces[ call sites, expected at least the three that exist"
    );
}

#[test]
fn permutation_classes_match_what_the_generator_draws() {
    use voxelcraft::block::{perm, tex, PERM_CLASS};

    for layer in [
        tex::STONE,
        tex::GRAVEL,
        tex::COBBLE,
        tex::GLOWSTONE,
        tex::BEDROCK,
        tex::SNOW,
        tex::LICHEN,
    ] {
        assert_eq!(
            PERM_CLASS[layer as usize],
            perm::NONE,
            "layer {layer} is built from blob() and tiles seamlessly across block edges;              permuting it would break that join"
        );
    }

    for layer in [
        tex::LOG_SIDE,
        tex::LOG_TOP,
        tex::PLANKS,
        tex::GLASS,
        tex::TALL_GRASS,
    ] {
        assert_eq!(PERM_CLASS[layer as usize], perm::NONE, "layer {layer} is directional");
    }

    assert_eq!(PERM_CLASS[tex::GRASS_SIDE as usize], perm::FLIP_U);

    for layer in [
        tex::DIRT,
        tex::GRASS_TOP,
        tex::SAND,
        tex::PODZOL,
        tex::LEAVES,
        tex::PINE_LEAVES,
        tex::MEADOW_TOP,
    ] {
        assert_eq!(PERM_CLASS[layer as usize], perm::D4, "layer {layer} is isotropic");
    }
}

#[test]
fn packed_face_words_round_trip() {
    let table = block::face_table();
    assert_eq!(table.len(), block::BLOCK_COUNT * block::FACES_PER_BLOCK);
    for (i, &word) in table.iter().enumerate() {
        let layer = word & block::FACE_LAYER_MASK;
        let class = word >> block::FACE_LAYER_BITS;
        assert!(
            layer < block::tex::COUNT,
            "entry {i} decodes to layer {layer}, past the {} in the atlas",
            block::tex::COUNT
        );
        assert_eq!(
            class,
            block::PERM_CLASS[layer as usize],
            "entry {i} carries a class its layer does not"
        );
    }
}

#[test]
fn bind_group_0_matches_the_shader() {
    const WANT: usize = voxelcraft::render::BIND_GROUP_0_ENTRIES;
    let src = voxelcraft::render::shader_source();

    let mut seen: Vec<u32> = Vec::new();
    for (at, _) in src.match_indices("@group(0) @binding(") {
        let open = at + "@group(0) @binding(".len();
        let close = open
            + src[open..]
                .find(')')
                .expect("an unterminated @group(0) @binding(");
        seen.push(
            src[open..close]
                .parse()
                .expect("a binding index is a number"),
        );
    }

    assert_eq!(
        seen.len(),
        WANT,
        "the shader declares {} group-0 bindings and make_layout builds {WANT}",
        seen.len()
    );
    seen.sort_unstable();
    let want: Vec<u32> = (0..WANT as u32).collect();
    assert_eq!(
        seen, want,
        "group-0 binding indices are not 0..{WANT} exactly once each"
    );
}

#[test]
fn water_body_matches_the_atlas_layer() {
    let src = voxelcraft::render::shader_source();
    let at = src
        .find("const WATER_BODY: vec3<f32> = vec3<f32>(")
        .expect("WATER_BODY is not in the module any more");
    let open = at + "const WATER_BODY: vec3<f32> = vec3<f32>(".len();
    let close = open + src[open..].find(')').expect("unterminated WATER_BODY");
    let wgsl: Vec<f32> = src[open..close]
        .split(',')
        .map(|p| p.trim().parse().expect("a WATER_BODY component"))
        .collect();
    assert_eq!(wgsl.len(), 3, "WATER_BODY is not three components");

    let atlas = textures::build_atlas(textures::WATER_MOTTLE_PRE21B, false);
    let w = layer_of(&atlas, tex::WATER);
    let mut mean = [0.0f64; 3];
    for px in w.as_chunks::<4>().0 {
        for c in 0..3 {
            mean[c] += metric_srgb_to_linear(px[c]) as f64;
        }
    }
    let n = (w.len() / 4) as f64;

    for c in 0..3 {
        let layer = mean[c] / n;
        let shader = wgsl[c] as f64;

        assert!(
            (shader - layer).abs() <= 0.04 * layer.max(1e-6),
            "channel {c}: WATER_BODY has {shader:.4} where the atlas layer averages \
             {layer:.4} -- the surface and the underwater medium disagree about the colour \
             of water"
        );
    }
}

fn metric_srgb_to_linear(v: u8) -> f32 {
    voxelcraft::harness::metric::srgb_to_linear(v)
}

#[test]
fn frame_time_is_read_only_by_the_two_animation_phases() {
    let src = voxelcraft::render::shader_source();

    let code = src
        .lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("
");

    const READERS: [&str; 2] = [" * frame.wave_speed", " * frame.cloud_speed"];
    let mut seen = [0usize; READERS.len()];
    let mut sites = 0;
    for (at, _) in code.match_indices("frame.time") {
        let rest = &code[at + "frame.time".len()..];

        if rest.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
            continue;
        }
        sites += 1;
        match READERS.iter().position(|r| rest.starts_with(r)) {
            Some(k) => seen[k] += 1,
            None => panic!(
                "a `frame.time` here is multiplied by neither wave_speed nor cloud_speed:                  `frame.time{}`. Batch 23's `--anim-time` claims to be the only animation                  knob a headless capture has, and a third reader of the clock breaks that                  claim without breaking the build",
                &rest[..rest.len().min(40)]
            ),
        }
    }

    assert!(
        sites >= 3,
        "found {sites} `frame.time` readers, expected at least the three that exist"
    );
    assert!(
        seen[0] >= 1 && seen[1] >= 1,
        "the wave phase has {} readers and the cloud drift {}, and each needs at least one:          a batch that deleted one of the two would otherwise leave this test passing while          `--anim-time` had quietly become half a flag",
        seen[0],
        seen[1]
    );
}

#[test]
fn the_shipping_mottle_is_the_amplitude_the_tests_call_shipping() {
    let shipped = voxelcraft::config::Config::default().water_mottle;

    assert_eq!(
        shipped, 0.0,
        "`Config::default().water_mottle` is {shipped}, but `WATER_MOTTLE_PRE21B`'s doc          comment and `shipping` above both say 0.0 ships. One of the three is wrong, and          the default is the copy the renderer actually reads"
    );
    assert_ne!(
        shipped, WATER_MOTTLE_PRE21B,
        "the shipping amplitude and the control amplitude are the same number, so          `--water-mottle 0.2` reproduces the previous build by doing nothing -- batch 9's          rule, and the failure this file's other control check cannot see"
    );

    let shipped_atlas = textures::build_atlas(shipped, false);
    let control_atlas = textures::build_atlas(WATER_MOTTLE_PRE21B, false);
    assert_ne!(
        layer_of(&shipped_atlas, tex::WATER),
        layer_of(&control_atlas, tex::WATER),
        "the shipping water layer and the pre-batch-21b one are byte for byte the same"
    );
}
