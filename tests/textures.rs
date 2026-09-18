//! The atlas, batch 21b's control, and the guards that parse `render::shader_source()`.
//!
//! The last of those is why this file is larger than its name suggests. A claim that
//! crosses the Rust/WGSL boundary can only be checked by reading the module string, and
//! there is one place that does it; batch 23 added the third such guard here rather than
//! starting a fourth test binary for one function.
//!
//! There was no test file for `textures.rs` before this batch, which is why the water
//! layer's mottling could sit in the tree for thirteen batches putting salmon diamonds on
//! every submerged floor without anything noticing. The layer is generated, so a hash of it
//! is the whole test.

use voxelcraft::block::{self, tex};
use voxelcraft::harness::metric::sha256_hex;
use voxelcraft::textures::{self, WATER_MOTTLE_PRE21B, TEX_SIZE};

/// One layer's worth of RGBA8 bytes.
const LAYER: usize = (TEX_SIZE * TEX_SIZE * 4) as usize;

fn layer_of(atlas: &[u8], layer: u32) -> &[u8] {
    let l = layer as usize;
    &atlas[l * LAYER..(l + 1) * LAYER]
}

/// The control for batch 21b, in the shape batches 9, 14 and 18 established for theirs:
/// with the mottle amplitude set back to its old value the atlas is the pre-batch-21b atlas
/// **byte for byte**.
///
/// Both constants were measured through `sha256_hex` on the untouched tree, **before a line
/// of the batch was written**, which is batch 9's rule and the only thing that lets a
/// control pin a regression rather than describe one.
///
/// Two hashes and not one on purpose. The water layer alone is what the batch claims to
/// change, and the rest of the atlas is what says it changed *nothing else* -- `texel` is one
/// match over every layer and a mistake in threading the new argument could easily have
/// reached another arm.
///
/// **The second hash covers the layers that existed when it was taken, and not the whole
/// atlas, which is a batch-59 correction rather than a weakening.** It read `build_atlas(.., false)`
/// entire until that batch appended three emissive layers, at which point a control for the
/// *water* layer failed because the atlas had grown -- a true statement about neither the
/// control nor the mottle. Appending is the one edit this test must not object to, since
/// `block::tex` ids are appended and never inserted exactly as block ids are; changing any
/// layer below the cut is what it exists to catch, and it still does.
#[test]
fn water_mottle_reproduces_the_pre_batch21b_atlas() {
    const PRE_BATCH21B_ATLAS: &str =
        "179583743b32bbdc3081df661b7538aca6b939ccd209b5029766e2f52d6577ac";
    const PRE_BATCH21B_WATER: &str =
        "7d80aaba25eb2a1b58fa4f693d73957b87d5cd7cc33127aec565b600036d4597";
    /// How many layers the atlas held when the hash above was taken. `tex::MEADOW_TOP` was the
    /// last one from batch 18 until batch 59, so this is `tex::COUNT` as batch 21b saw it.
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

    // The other half, and the one batch 9's lesson is about: a control that reproduced the
    // old atlas because the *new* one is also the old one would satisfy the assertions above
    // perfectly and would mean the batch did nothing.
    let shipping = textures::build_atlas(0.0, false);
    assert_ne!(
        sha256_hex(layer_of(&shipping, tex::WATER)),
        PRE_BATCH21B_WATER,
        "flattening the mottle changed nothing; the control is measuring itself"
    );
}

/// Everything except water is untouched by the amplitude.
///
/// This is the assertion that would have caught the threading mistake the test above can only
/// catch in aggregate: it names the layer that may move and holds all twenty of the others to
/// equality one at a time, so a failure says which arm went wrong.
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

/// The amplitude barely moves the layer's **average** colour, which is what keeps the far
/// sea the same colour at every setting -- and "barely" is a measured bound rather than an
/// exact property, which is the correction this test exists to record.
///
/// Centring the modulation on 0.5 does *not* preserve the mean, for two reasons that were
/// both measured rather than assumed: `blob`'s mean over the 16x16 tile is 0.4622, not 0.5,
/// and the sRGB transfer is convex so the mean of a modulated value sits above the value of
/// its mean. Measured, flattening lifts the linear mean by 0.15, 0.38 and 1.14 code values
/// in R, G and B.
///
/// Scored in linear, because `downsample` averages in linear and the coarse mips distant
/// water reads are that average. The bound is **1.5 code values at each channel's own
/// level** -- per channel because one code value is worth very different amounts of linear
/// at 25 and at 107, and 1.5 rather than the measured 1.14 because a floor at the measured
/// value would be a second copy of it.
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
    // The flat layer is one repeated texel, so its own code values are what quantisation
    // acts on -- and one code value is worth very different amounts of linear in a channel
    // at 25 and a channel at 107, which is the whole reason this tolerance is per channel
    // and not one number for all three.
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

/// Every WGSL index into `block_faces` strides by `FACES_PER_BLOCK`.
///
/// This is the test that would have caught batch 21b's actual bug, and it is worth having in
/// preference to the compile-time assert beside it: the assert pins the Rust constant at 8,
/// which was never what went wrong. What went wrong is that one of three WGSL call sites
/// multiplied by a *different* number, and nothing on either side of the boundary could see
/// the disagreement -- `shade_water` read block 10 face 1 instead of water's +Y face, which
/// is a perfectly valid index into a table that is 160 entries long.
///
/// It reads `render::shader_source()` rather than the files, for the same reason
/// `--bin shaderstats` does: that function is the only statement of what the module is, and a
/// test that re-concatenated the shaders would be testing a different shader.
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
    // A test that found no call sites would pass silently, which is the failure mode the
    // `--demo-edits` lesson is about: a check that verifies nothing still reports success.
    assert!(
        sites >= 3,
        "found {sites} block_faces[ call sites, expected at least the three that exist"
    );
}

/// Every `block_faces` read goes through `face_layer`, and no shader uses a raw word as a layer.
///
/// The sibling above pins the *stride* of the table, which is the trap batch 21b fell into. This
/// pins its *contents*: since batch 36 a word is the atlas layer with `block::PERM_CLASS` packed
/// above it, so a call site that forgot the accessor would index the atlas with
/// `layer | class << 16` -- an enormous layer id, which is not a plausible-looking wrong texture
/// but an out-of-range one, and therefore exactly the kind of thing that is obvious in a capture
/// only if the capture happens to contain that block.
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
        // The one other shape allowed: binding the raw word to a local whose name *says* it
        // is a packed word. That keeps the load off the hot path from happening twice while
        // leaving the raw value greppable -- and every use of it is checked below.
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
    // A raw word is a layer with `block::PERM_CLASS` packed above it, so using one as a layer
    // indexes the atlas far past its end. Every read of such a local has to go through an
    // accessor.
    for name in &words {
        let mut uses = 0;
        for (at, _) in src.match_indices(name.as_str()) {
            let before = src[..at].trim_end();
            if before.ends_with('=') || before.ends_with("let") {
                continue; // the binding itself
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
    // A test that found no call sites would pass silently, which is the failure mode the
    // `--demo-edits` lesson is about: a check that verifies nothing still reports success.
    assert!(
        sites >= 3,
        "found {sites} block_faces[ call sites, expected at least the three that exist"
    );
}

/// The permutation class table says only what `textures.rs` actually draws.
///
/// Three properties, and the third is the one that is invisible in a capture until a wall of
/// stone stops being a wall. `blob()` wraps with `rem_euclid(16)`, so every layer built from it
/// tiles seamlessly across block boundaries *today*; a permutation breaks that join, and batch 36
/// deliberately declined to take it. If a later batch wants that variety it has to change this
/// test on purpose, which is the point of writing it down.
#[test]
fn permutation_classes_match_what_the_generator_draws() {
    use voxelcraft::block::{perm, tex, PERM_CLASS};

    // Torus-periodic: built from `blob`, which wraps. These tile seamlessly and must not turn.
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
    // Directional: rotating the grain, the plank rows or a blade is a new defect.
    for layer in [
        tex::LOG_SIDE,
        tex::LOG_TOP,
        tex::PLANKS,
        tex::GLASS,
        tex::TALL_GRASS,
    ] {
        assert_eq!(PERM_CLASS[layer as usize], perm::NONE, "layer {layer} is directional");
    }
    // The batch's subject: bounded in v, arbitrary in u.
    assert_eq!(PERM_CLASS[tex::GRASS_SIDE as usize], perm::FLIP_U);
    // Isotropic per-texel noise, free to take the full group.
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

/// A packed face word round-trips, and the class never eats the layer.
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

/// Bind group 0's Rust layout and its WGSL declarations agree, in count and in numbering.
///
/// `CLAUDE.md` carried "bind group 0 has 19 entries (0-18)" as a sentence for several batches.
/// Batch 28 deleted two counts exactly like it that had gone stale -- a test count and a
/// bytes-per-voxel figure -- so this one moved into `render::BIND_GROUP_0_ENTRIES` and into
/// this check rather than staying in prose. The array annotation in `make_layout` pins the
/// Rust half; this is the half that annotation cannot see, and the contiguity assert is the
/// "(0-18)" part, which a bare count would miss if a binding were duplicated and one skipped.
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

/// `WATER_BODY` in `resolve.wgsl` and the water atlas layer are the same colour.
///
/// Its comment says so -- "the same authored value the atlas layer is built from" -- and until
/// batch 21b nothing checked it, which is how the two came to disagree in the worst possible
/// way. `underwater_body` used this constant and was right; `shade_water` read the layer
/// through a bad stride and got glass. The engine contained its own ground truth for the
/// colour of water, one function above the bug, and no test compared them.
///
/// That is why the `underwater` and `cave` vantages are the only two of fourteen the batch
/// moves zero pixels at: the inside-the-medium path never touched the atlas.
///
/// Parsed out of `render::shader_source()` for the reason the stride test is: that string is
/// the only statement of what the module is.
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

    // The layer's mean in linear, which is what the coarse mips converge to and so the one
    // number a single constant can be compared against.
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
        // 4% -- loose enough for the two to have been authored by hand and rounded
        // differently, tight enough that reading a different layer entirely (glass's red is
        // 0.61 against water's 0.0096, a factor of 63) cannot pass.
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

/// `frame.time` is read by the wave phase and the cloud drift and by nothing else.
///
/// This is the invariant `--anim-time` is worth having. The flag claims to be *the* knob for
/// everything that animates in a headless render, and that claim is only true while those two
/// products are the whole of what `frame.time` reaches -- add a third reader and the flag
/// silently stops meaning what the control table says it means, with no build error and no
/// pixel moving in any existing capture to give it away.
///
/// It is also the test behind the measured scope: `--no-waves --cloud-cover 0 --anim-time 4`
/// against `--no-waves --cloud-cover 0` is 0 pixels at all fourteen vantages, with the water
/// and the world otherwise fully intact. That measurement is true of the build it was taken
/// on; this is what keeps it true.
///
/// CLAUDE.md's own lesson, from the batch that found `shade_water` striding by six: a comment
/// asserting that two things agree is a test nobody has written.
#[test]
fn frame_time_is_read_only_by_the_two_animation_phases() {
    let src = voxelcraft::render::shader_source();
    // Strip line comments first. Two of them name `frame.time` in prose -- both say it is
    // pinned to zero headlessly, which is exactly the sentence batch 23 made obsolete -- and
    // a guard that its own documentation can trip is a guard that gets deleted.
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
        // `frame.time_of_day` is not `frame.time`. There is no such field today; this is so
        // that adding one fails on its own merits rather than here.
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

    // A test that found no call sites would pass silently, which is the `--demo-edits`
    // failure mode: a check that verifies nothing still reports success.
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

/// **The guard batch 26 added, and the one whose absence let an abandoned fix keep its
/// documentation.**
///
/// Batch 21b built the mottle flattening as its candidate fix, measured it moving 0 pixels at
/// fourteen vantages -- the null that revealed the glass-layer stride -- and then correctly
/// abandoned it, putting `Config::default()` back to 0.20 and leaving the lever in the roadmap
/// for batch 21a. What it did not put back were the fix's descriptions: `WATER_MOTTLE_PRE21B`
/// still said "0.0 ships", the `tex::WATER` arm still said the layer shipped flat, and
/// `shipping` in `water_mottle_reproduces_the_pre_batch21b_atlas` above is still
/// `build_atlas(0.0, false)`. Three statements of a fix that had been reverted, against one default
/// that had not -- and the default is the copy the renderer reads.
///
/// Nothing failed, and the reason is the shape of this file rather than any missing case:
/// **every other assertion here calls `build_atlas` with a literal**, so the suite could say
/// what 0.0 and 0.2 each produce while no test anywhere asked which of them the *game* passes.
/// A comment claiming a default is a test nobody has written, exactly as `WATER_BODY`'s was.
///
/// So this asserts the join and not the endpoints: the amplitude the game builds its atlas
/// from is the one the tests call `shipping`, and it is not the control.
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

    // The other half, in the atlas rather than in the constant: the layer the game builds
    // must not be the layer the control reproduces. This is what would still fail if some
    // later batch made the two amplitudes agree by moving `WATER_MOTTLE_PRE21B` instead.
    let shipped_atlas = textures::build_atlas(shipped, false);
    let control_atlas = textures::build_atlas(WATER_MOTTLE_PRE21B, false);
    assert_ne!(
        layer_of(&shipped_atlas, tex::WATER),
        layer_of(&control_atlas, tex::WATER),
        "the shipping water layer and the pre-batch-21b one are byte for byte the same"
    );
}



