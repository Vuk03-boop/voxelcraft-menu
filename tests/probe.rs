//! The probe field: batch 55's diagnostic tap, batch 56's removal, and batch 57's ambient cube.
//!
//! The feature is one hardware trilinear tap into the probe field per shaded surface, and under
//! `--probe-tap` that field holds a constant 1.0 -- so `shade_hit`'s `amb = amb * probe_tap(hit)`
//! is bit-exact for every finite `amb` and the whole output of the batch is a millisecond. The
//! constant is no longer the field's default: batch 57 filled it with a real bake, and
//! `--probe-tap` implies `--probe-fill 1.0` so that this paragraph stays true.
//!
//! **That bit-exactness is also the hazard this file exists for.** `lessons.md` records it as
//! *a no-op claim cannot be told apart from a not-wired-up claim*, and a sweep whose pass
//! condition is **0 pixels differing** cannot tell a tap that costs nothing from a tap that was
//! never compiled in. Batch 55's answer is three readings from three different places, and two
//! of them are here:
//!
//! - the picture, from `--probe-fill 0.5`, which **must** move (the fixture's job, not this file's)
//! - the machine code, from `shaderstats`: `resolve` is **+1280 bytes** with the override at 1
//!   and **96 registers on both sides**
//! - the source, which is what the two tests below claim
//!
//! These are claims about the shader *source*, the way `tests/secondary.rs` and `tests/cutout.rs`
//! make them, because there is no Rust function to call and neither defect shows up in any image.
//!
//! **Batch 57 turned that field into a real one and added the second half of this file.** The
//! bake is Rust, so those tests can call it -- and what they check is not that it runs but that
//! it *reduces* correctly: an unoccluded probe has to reproduce `face_shade`'s own numbers, or
//! shipping the cube changes how bright the world is rather than how it is shaped, and no
//! before/after screenshot could separate the two.

mod support;
use glam::{IVec3, Vec3};
use voxelcraft::block;
use voxelcraft::probe;
use voxelcraft::render;
use voxelcraft::voxel::{ChunkKey, World};

/// The body of one WGSL function, with its comments stripped.
///
/// A transliteration of `tests/secondary.rs`'s helper, deliberately: `pitfalls.md` warns that a
/// second copy of a thing nothing uses rots, and this one has a caller in each file. Comments go
/// first for `hit_t_does_not_branch_on_the_cutout_override`'s reason -- the explanation of *why*
/// a name is absent from the code names it, and a test its own documentation can fail is not
/// checking the code.
fn fn_body(name: &str) -> String {
    let src = render::shader_source();
    let pat = format!("fn {name}(");
    let at = src
        .find(&pat)
        .unwrap_or_else(|| panic!("{name} is not declared in the shader"));
    let body = &src[at..];
    let end = body[1..]
        .find("\nfn ")
        .unwrap_or_else(|| panic!("another function follows {name}"));
    body[..end]
        .lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The gate is the override alone, which is batch 45's rule applied to the batch that measures it.
///
/// **The failure this refuses would be self-inflicted in a way none of the others would be.** The
/// arm sits in `shade_hit`, which `resolve` inlines four times; written the house way as
/// `SPEC_PROBE_TAP && (frame.flags & FLAG_PROBE_TAP) != 0u`, `frame.flags` is a run-time value,
/// the driver cannot prove which arm runs, and the texture sample stays compiled into every copy.
/// The shipping build would then pay for a tap it never takes -- and the number the batch exists
/// to report would be the cost of the measurement rather than the cost of the thing. Batch 45
/// measured that spelling at **+1.000 ms** against **-1.584** for the folded one, same arm taken.
#[test]
fn the_probe_tap_is_gated_by_the_override_alone() {
    let src = render::shader_source();
    let at = src
        .find("if SPEC_PROBE_TAP {")
        .expect("shade_hit gates the probe tap on SPEC_PROBE_TAP");
    let gate = &src[at..at + "if SPEC_PROBE_TAP {".len()];
    assert!(
        !gate.contains("frame.flags"),
        "the probe tap's gate reads frame.flags, so it is a run-time value and the texture \
         sample stays in all four of shade_hit's inlined copies -- the shipping build would pay \
         for a tap it never takes, and batch 55's whole reading would be of itself. The bit is \
         still real and still in SPEC_MASK; it keys the pipeline cache exactly as \
         FLAG_WATER_SHADOW_CUT does. Gate was: {gate}"
    );
}

/// The tap is a multiply by the field and nothing else, which is what makes it bit-exact.
///
/// **The thing that would quietly break this is a helpful edit, not a wrong one.** Any other
/// combiner -- an add, a `mix`, a `clamp`, a second term -- is defensible on its face and
/// destroys the one property the batch rests on: `x * 1.0` is `x` for every finite `x`, where
/// `x + 1.0` is not, and `bitexact` would then be measuring a look change nobody intended. It is
/// the same shape as batch 54's control bug, where a gate one level too deep made `--no-snell`
/// revert the detail of a mirror rather than the mirror.
#[test]
fn the_tap_combines_by_multiply_so_a_unit_field_is_invisible() {
    let src = render::shader_source();
    let at = src
        .find("if SPEC_PROBE_TAP {")
        .expect("shade_hit gates the probe tap on SPEC_PROBE_TAP");
    let arm: String = src[at..]
        .lines()
        .take(3)
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        arm.contains("amb = amb * probe_tap(hit)"),
        "the probe tap has to combine by multiplying `amb`, because that is the whole of why a \
         field of 1.0 is bit-exact and the batch's reading is a millisecond rather than a look \
         change. Arm was:\n{arm}"
    );

    // And the field it taps is the 3D texture, not something a later batch pointed elsewhere.
    let body = fn_body("probe_tap");
    assert!(
        body.contains("textureSampleLevel(probe_tex, linear_samp"),
        "probe_tap should take a hardware trilinear tap into probe_tex -- eight loads and a \
         manual lerp is a different cost and not the one L1 would pay. Body was:\n{body}"
    );
    assert!(
        body.contains("fract("),
        "probe_tap should wrap its coordinate itself: `linear_samp` is the history sampler and \
         clamps, so an unwrapped tap sends every distant pixel to one edge texel and reads \
         artificially cheap. Body was:\n{body}"
    );
}

/// The diagnostic ships **off**, and that is a property of `Config::default` rather than of prose.
///
/// **This is the one switch in the tree whose off state is the shipping build**, and the reason
/// is the argument `CLAUDE.md`'s control table makes about cost: a control that costs something
/// taxes every measurement taken through it until somebody reaches the roadmap entry.
/// `--no-foliage` is the precedent at **+1.24 to +2.07 ms**, the largest regression this project
/// has shipped, and it was in a control. A diagnostic that ships *on* would be that failure with
/// the excuse removed, since this one's entire purpose is to be measured.
#[test]
fn the_probe_tap_ships_off_and_the_field_ships_at_one() {
    let cfg = voxelcraft::config::Config::default();
    assert!(
        !cfg.probe_tap,
        "--probe-tap has to default off: it is a diagnostic, and one left switched on would \
         charge every later measurement the time it was built to measure"
    );
    assert_eq!(
        cfg.probe_fill, None,
        "the probe field has to default to the bake, which is batch 57: `Some(f)` pins it to a \
         constant and switches the bake's uploads off, and that is a diagnostic rather than \
         the shipping build"
    );
    // ...and asking for either diagnostic has to bring the constant field back with it, or
    // batch 55's and batch 56's rows in `CLAUDE.md`'s control table stop being true. The pin
    // is what keeps `--probe-tap` bit-exact now that the texture holds real shading factors.
    let (tap, _, _) = voxelcraft::config::parse_from(&["--probe-tap".to_string()]);
    assert_eq!(
        tap.probe_fill,
        Some(1.0),
        "--probe-tap has to pin the field to 1.0: `amb * 1.0` being bit-exact is the whole of \
         why batch 55's reading is a millisecond and not a look change, and batch 57 filled \
         that same texture with a bake"
    );
    let (explicit, _, _) = voxelcraft::config::parse_from(&[
        "--probe-fill".to_string(),
        "0.5".to_string(),
        "--probe-tap".to_string(),
    ]);
    assert_eq!(
        explicit.probe_fill,
        Some(0.5),
        "an explicit --probe-fill has to win in either order -- it is batch 55's paired \
         positive claim, and a pin that overwrote it would make the positive unreachable"
    );
    assert!(
        !cfg.probe_noise,
        "--probe-noise is the build that rules out driver compression of a constant fill, and \
         it moves the picture -- it cannot be the default"
    );
}

/// `--probe-ambient` implies `--probe-tap`, and the implication lives at the parser.
///
/// **The removal without the tap is a shader that reads no light at all**, which would render a
/// frame and report a millisecond and mean nothing -- the fastest build is always the one that
/// computes nothing. Putting the implication in `config::parse_from` rather than in `flags_from`
/// means no consumer can construct the broken pair: `harness`, `headless` and `app` all read the
/// same `Config`, and a second copy of the rule in `scene.rs` is a second copy that could drift.
#[test]
fn probe_ambient_implies_probe_tap() {
    let (cfg, _, _) = voxelcraft::config::parse_from(&["--probe-ambient".to_string()]);
    assert!(
        cfg.probe_ambient && cfg.probe_tap,
        "--probe-ambient has to set probe_tap too: the removal deletes the terms that produced          `amb`, so without the tap supplying it the pass shades from nothing and the bench          measures a shader that does no work. Got probe_ambient={} probe_tap={}",
        cfg.probe_ambient,
        cfg.probe_tap
    );

    let (plain, _, _) = voxelcraft::config::parse_from(&["--probe-tap".to_string()]);
    assert!(
        plain.probe_tap && !plain.probe_ambient,
        "the implication is one-way: --probe-tap alone is batch 55's bit-exact diagnostic and          must not drag the removal in with it"
    );
}

/// The removal folds at pipeline-compile time, which is the only way it removes anything.
///
/// Every site gated below sits inside `shade_hit`, which `resolve` inlines four times. A
/// run-time gate keeps both arms in every copy -- the deleted terms would still be compiled, the
/// build would measure nothing, and `shaderstats` is what says otherwise: `resolve` is
/// **582,400 bytes shipping, 583,680 with the tap and 575,360 with the removal**. If a later
/// edit gives any of these a `frame.flags` companion, that last number goes back up and the
/// batch's whole reading is void.
#[test]
fn the_removal_is_gated_by_the_override_alone() {
    let src = render::shader_source();
    let sites: Vec<&str> = src
        .lines()
        .filter(|l| l.contains("SPEC_PROBE_AMBIENT") && !l.trim_start().starts_with("//"))
        .collect();
    assert!(
        sites.len() >= 7,
        "expected the removal to gate every site it claims -- blk_n, the corner tint, the amb          bilinear, both cheap branches, face_shade and the ambient floor -- found {}:\n\
{}",
        sites.len(),
        sites.join("\n")
    );
    for line in sites {
        assert!(
            !line.contains("frame.flags"),
            "a SPEC_PROBE_AMBIENT site reads frame.flags, so the driver cannot fold it and the              terms this batch claims to delete stay compiled into all four of shade_hit's              inlined copies. Line was: {line}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Batch 57: the ambient cube
// ---------------------------------------------------------------------------------------------

/// An unoccluded probe returns exactly 1 on every axis.
///
/// **This is the exposure claim, and it is the one thing the screenshot pair cannot check.**
/// The cube's job is to change how the world is *shaped*, not how bright it is, and the field
/// stores occlusion precisely so that "unchanged" has an exact spelling: `shade_hit` computes
/// `face_shade(id) * tap`, so 1 is the identity and anything else is a global exposure shift
/// wearing an occlusion change's name. If an open field came back at 0.987 -- exactly where
/// sixteen azimuths land without the discrete normalisation -- every side face in the world
/// would darken 1.3% uniformly, which no before/after could tell from the feature working.
///
/// The first build of this batch got it wrong in the other direction and by far more, and
/// `probe`'s module note has that story.
#[test]
fn an_open_probe_reproduces_the_constants_it_replaces() {
    let open = probe::cube_from_tangents(&[0.0; probe::AZIMUTHS]);
    for (id, &v) in open.iter().enumerate() {
        assert!(
            (v - 1.0).abs() < 1e-4,
            "an unoccluded probe has to return exactly 1 on axis {id}, because the shader \
             multiplies face_shade by it and 1 is the only value that leaves the pre-57 frame \
             alone. Got {v}"
        );
    }
}

/// A buried probe returns 1, which is what makes `cave` bit-exact rather than nearly so.
///
/// **`lessons.md` calls a bit-exact vantage the failure mode indistinguishable from success**,
/// so this exists to say *why* `cave` does not move rather than to let the sweep imply it. A
/// probe under the height field sees no sky in any direction and its raw occlusion is 0 on
/// every axis, which would multiply the whole cave to black. `SKY_BLEND` is the constant that
/// refuses to, and the second half of this test says it is a *ramp* and not a cliff: a probe
/// that can still see a tenth of the sky has to come back well above its raw occlusion and
/// well below 1, because a step in the field is a step the trilinear tap would smear over
/// eight blocks.
#[test]
fn a_buried_probe_falls_back_to_the_table() {
    // atan(64) is 89.1 degrees: the height field is overhead in every direction.
    let buried = probe::cube_from_tangents(&[64.0; probe::AZIMUTHS]);
    for (id, &v) in buried.iter().enumerate() {
        assert!(
            (v - 1.0).abs() < 1e-3,
            "a probe with no sky has to return 1 on every axis, or a cave multiplies to black. \
             Axis {id} got {v}"
        );
    }

    // atan(3) is 71.6 degrees all round: a deep slot, about a tenth of the sky, inside the ramp.
    let slot = probe::cube_from_tangents(&[3.0; probe::AZIMUTHS]);
    assert!(
        slot[3] > 0.2 && slot[3] < 0.95,
        "a probe inside the ramp has to be lifted off its raw occlusion of 0.1 without \
         reaching 1. Got {}",
        slot[3]
    );
}

/// The cube is directional, which is the entire feature and the one thing a constant cannot do.
///
/// A horizon that rises in the +X azimuths and nowhere else has to darken the +X face and
/// leave the -X face alone. **Without this the batch could ship a field that is merely a
/// scalar AO term**, which would look like something and would not be what roadmap R1 is
/// about: the ladder's rung is *direction*, and a six-entry cube answering the same number six
/// times is the one-coefficient basis the flood already has.
#[test]
fn a_one_sided_horizon_darkens_only_the_face_that_looks_at_it() {
    let mut tan = [0.0f32; probe::AZIMUTHS];
    // Azimuth k runs from +X anticlockwise toward +Z, so k = 0 is +X and k = 8 is -X.
    for (k, t) in tan.iter_mut().enumerate() {
        if k <= 2 || k >= probe::AZIMUTHS - 2 {
            *t = 4.0;
        }
    }
    let c = probe::cube_from_tangents(&tan);
    assert!(
        c[1] < c[0] - 0.05,
        "a wall in the +X azimuths has to darken the +X face well below the -X one. Got +X {} \
         against -X {}",
        c[1],
        c[0]
    );
    assert!(
        c[3] < 1.0,
        "the same wall has to take some sky off the up-facing entry too. Got {}",
        c[3]
    );
    assert!(
        (c[0] - 1.0).abs() < 0.02,
        "the face pointing away from the wall should be near unoccluded: the cosine lobe about \
         -X has almost no weight in the +X azimuths. Got {}",
        c[0]
    );
    assert_eq!(
        c[2], 1.0,
        "the down-facing entry is unoccluded by construction -- no sky lies below the horizon, \
         so terrain cannot take any away. Got {}",
        c[2]
    );
}

/// The cube's gate is the override alone, which is batch 45's rule and batch 55's reason.
///
/// The arm sits in `shade_hit`, which `resolve` inlines four times. A `frame.flags` companion
/// would keep the texture sample compiled into every copy, so `--no-probe-cube` would pay for a
/// tap it never takes -- and a control that costs a millisecond taxes every measurement made
/// through it, which is the argument `CLAUDE.md`'s cost column exists to make.
#[test]
fn the_ambient_cube_is_gated_by_the_override_alone() {
    let src = render::shader_source();
    let sites: Vec<&str> = src
        .lines()
        .filter(|l| l.contains("SPEC_PROBE_CUBE") && !l.trim_start().starts_with("//"))
        .collect();
    assert!(
        !sites.is_empty(),
        "shade_hit has to gate the ambient cube on SPEC_PROBE_CUBE"
    );
    for line in sites {
        assert!(
            !line.contains("frame.flags"),
            "the ambient cube's gate reads frame.flags, so the driver cannot fold it and the \
             control build carries the texture sample in all four of shade_hit's inlined \
             copies. The bit is still real and still in SPEC_MASK; it keys the pipeline cache \
             exactly as FLAG_WATER_SHADOW_CUT does. Line was: {line}"
        );
    }
}

/// The cube ships **on**, which is the opposite of the two diagnostics above it in this file.
#[test]
fn the_ambient_cube_ships_on() {
    let cfg = voxelcraft::config::Config::default();
    assert!(
        cfg.probe_cube,
        "batch 57's cube is a feature and not a diagnostic: it ships on and --no-probe-cube is \
         the control"
    );
    let (off, _, _) = voxelcraft::config::parse_from(&["--no-probe-cube".to_string()]);
    assert!(!off.probe_cube, "--no-probe-cube has to clear it");
    assert!(
        off.probe_fill.is_none(),
        "the control must not pin the field: the claim is that the *shader* reads face_shade \
         again, and pinning would make the control pass even if the arm were still taking the \
         tap"
    );
}

/// `probe_field` reads the slab its `normal_id` names, and the Rust bake writes them in that
/// order.
///
/// **It was `probe_cube` until batch 58 gave the same texel a second field**, and this test is
/// what made the rename declare itself -- it failed on the name before anything else did.
///
/// **A wrong slab stride reads a plausible neighbouring axis rather than garbage**, which is
/// exactly why `block_faces`' stride is an invariant rather than a bug report. The two halves
/// are `write_probe`'s `axis * PROBE_DIM_Y` in Rust and `f32(slab) * PROBE_DIM` in WGSL, and
/// the thing that ties them is that `slab` is `normal_id` with no remap.
#[test]
fn the_slab_index_is_the_normal_id() {
    let src = render::shader_source();
    let at = src
        .find("fn probe_field(")
        .expect("probe_field is declared in the shader");
    let body: String = src[at..]
        .lines()
        .take_while(|l| !l.starts_with("}"))
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        body.contains("select(normal_id, 3u, cross)"),
        "the slab has to be `normal_id` itself, with the cross-quads sent to the +Y slab -- a \
         remap here is a second encoding of what `normal_of` already decides, and a wrong one \
         reads a neighbouring axis and looks plausible. Body was:\n{body}"
    );
    assert!(
        body.contains("probe_samp"),
        "probe_field has to use the probe's own sampler: `linear_samp` clamps on every axis, and \
         the field wraps in X and Z. Body was:\n{body}"
    );
    assert_eq!(
        probe::AXES,
        6,
        "six slabs is what the shader's PROBE_SLABS says"
    );
}

// ---- batch 58: the ground bounce ----

/// The bounce ships **on**, and its control must not pin the field.
///
/// Same shape as `the_ambient_cube_ships_on` above it and for the same reason: the claim
/// `--no-probe-bounce` makes is that the *shader* multiplies by nothing, and pinning the field
/// would make the control pass even if the arm were still reading the texel.
#[test]
fn the_ground_bounce_ships_on() {
    let cfg = voxelcraft::config::Config::default();
    assert!(
        cfg.probe_bounce,
        "batch 58's bounce is a feature and not a diagnostic: it ships on and --no-probe-bounce is the control"
    );
    let (off, _, _) = voxelcraft::config::parse_from(&["--no-probe-bounce".to_string()]);
    assert!(!off.probe_bounce, "--no-probe-bounce has to clear it");
    assert!(
        off.probe_cube,
        "the two fields are independent controls sharing one texel: clearing the bounce must not clear the cube, or neither can be A/B'd against the other"
    );
    assert!(
        off.probe_fill.is_none(),
        "the control must not pin the field, for the cube control's reason exactly"
    );
}

/// `probe::FLOOR_TINT` is the same three numbers `resolve.wgsl` multiplies `frame.ambient` by.
///
/// **The bake divides this back out**, so that what the field stores is a multiplier on the
/// shader's constant whose identity is 1.0. If the two copies drifted, the identity would stop
/// being 1.0, every absence of the feature would stop being exact -- an unbaked texel, a coarse
/// LOD, `--no-probe-bounce` -- and **nothing in any capture would announce it**, because the
/// error is a uniform tint on a term that is a tenth of the ambient. That is the same argument
/// `faces_per_block_matches_the_shader` makes, and the reason this is a test rather than a
/// comment saying "keep these in sync".
#[test]
fn floor_tint_matches_the_shader() {
    let src = render::shader_source();
    let lit = format!(
        "vec3<f32>({:.2}, {:.2}, {:.2}) * frame.ambient",
        probe::FLOOR_TINT[0],
        probe::FLOOR_TINT[1],
        probe::FLOOR_TINT[2]
    );
    assert!(
        src.contains(&lit),
        "probe::FLOOR_TINT is {:?}, so resolve.wgsl's floor has to read `{lit}`. The bake divides that constant out and the shader multiplies it back in, so a disagreement moves the identity off 1.0 and quietly stops every absence of the feature being bit-exact.",
        probe::FLOOR_TINT
    );
}

/// `block::ALBEDO` is the mean linear albedo of each block's +Y face in the atlas it claims to
/// come from.
///
/// **A hand-written table is exactly the thing that drifts**, and it drifts silently: a texture
/// tweak in `textures.rs` moves the picture in a way a capture shows, and moves the *bounce* in
/// a way no capture attributes to it. Rebuilding the atlas here costs a millisecond and removes
/// the whole failure mode.
///
/// The tolerance is 1e-3 because the table is written to four places and the mean is taken in
/// linear over 256 texels, so the only difference should be the decimal rounding.
#[test]
fn albedo_matches_the_atlas() {
    let atlas = voxelcraft::textures::build_atlas(0.0, false);
    let n = (voxelcraft::textures::TEX_SIZE * voxelcraft::textures::TEX_SIZE) as usize;
    for (id, def) in block::BLOCKS.iter().enumerate() {
        let base = def.faces[3] as usize * n * 4;
        let mut acc = [0.0f32; 3];
        for i in 0..n {
            for (c, a) in acc.iter_mut().enumerate() {
                let u = atlas[base + i * 4 + c] as f32 / 255.0;
                // The atlas is `Rgba8UnormSrgb`, so the mean is taken after decoding and never
                // before -- averaging the bytes biases every entry dark, which is the argument
                // `textures::downsample` already makes for the mips.
                *a += if u <= 0.04045 {
                    u / 12.92
                } else {
                    ((u + 0.055) / 1.055).powf(2.4)
                };
            }
        }
        for (c, &sum) in acc.iter().enumerate() {
            let want = sum / n as f32;
            let got = block::ALBEDO[id][c];
            assert!(
                (want - got).abs() < 1e-3,
                "block::ALBEDO[{id}] ({}) channel {c} is {got}, atlas says {want}",
                def.name
            );
        }
    }
}

/// Ground of the reference colour leaves the floor's **brightness** exactly where it was.
///
/// **This is batch 57's exposure assert with one rung's worth of the same argument.** That
/// batch asserted an unoccluded probe returns exactly 1, because a screenshot pair cannot
/// separate a global exposure shift from an occlusion appearing. The same is true here and
/// worse: a bounce that multiplied every floor in the world by 1.4 would look like a feature
/// working, in every vantage at once, and no metric in the fixture would call it.
#[test]
fn reference_ground_leaves_the_exposure_alone() {
    // A flat field of one block type is the whole test: the gather normalises by its own
    // weight, so the form factor and the azimuth count both divide out.
    //
    // **The claim is in luminance and not per channel, and the difference is the feature.**
    // Over grass the multiplier is about (0.416, 1.259, 0.156) -- the floor turns green -- and
    // what must not move is what that does to the *brightness* of the floor. The per-channel
    // identity is a different claim about a different input: an unbaked texel returns 1.0, and
    // that is what makes the control bit-exact. `probe::BOUNCE_REF` separates the two.
    let want = block::luma(probe::FLOOR_TINT);
    let m = probe::bounce_of_uniform_ground(block::ALBEDO[block::GRASS as usize]);
    for (axis, v) in m.iter().enumerate() {
        let lit = block::luma([
            probe::FLOOR_TINT[0] * v[0],
            probe::FLOOR_TINT[1] * v[1],
            probe::FLOOR_TINT[2] * v[2],
        ]);
        assert!(
            (lit - want).abs() < 1e-4,
            "axis {axis}: the floor over reference ground has luminance {lit}, was {want} -- the bake's normalisation has moved and every frame in the world just changed exposure, which no capture in the fixture would attribute to this"
        );
    }
    // And the paired positives, which is `lessons.md`'s rule: a no-op claim alone cannot be
    // told from a not-wired-up one, so the quantity is read again where it must *not* be 1.
    let s = probe::bounce_of_uniform_ground(block::ALBEDO[block::SNOW as usize]);
    assert!(
        s[3][1] > 2.0,
        "snow bounces about 3.3x what grass does and the field has to carry it; got {}",
        s[3][1]
    );
    let w = probe::bounce_of_uniform_ground(block::ALBEDO[block::WATER as usize]);
    assert!(
        w[3][2] > 3.0 * w[3][0],
        "open water bounces blue and almost no red, which is the clearest claim in the table a picture can be checked against; got {:?}",
        w[3]
    );
}

/// One tap serves both fields, which is the whole cost claim of this batch.
///
/// **`probe_field` returns a `vec4` for exactly this reason**, and a later edit splitting it
/// into two functions taking the sample twice would double the read cost of the feature while
/// leaving every picture identical -- so no capture, no control and no `bitexact` row could
/// catch it, and the only thing that can is a claim about the source.
#[test]
fn one_tap_serves_both_fields() {
    let src = render::shader_source();
    let at = src
        .find("fn probe_field(")
        .expect("probe_field is declared in the shader");
    let body: String = src[at..]
        .lines()
        .take_while(|l| !l.starts_with("}"))
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        body.matches("textureSampleLevel").count(),
        1,
        "probe_field has to take exactly one sample: the cube is its `.a` and the bounce its `.rgb`, and that identity is what makes a coloured directional field cost what the scalar one cost. Body was:\n{body}"
    );
    assert_eq!(src.matches("probe_field(").count(), 5, "one declaration and one call per inline/cached schedule");
    for arm in ["shade_hit_legacy", "shade_hit_compact"] {
        let body = support::wgsl_function(&src, arm);
        assert_eq!(body.matches("probe_field(").count(), 1,
            "one combined probe tap per mutually exclusive schedule");
    }
}

// ---- batch 58: the claims a sweep that moved everything structurally cannot make ----
//
// **`bitexact` says all 18 vantages moved, and a global tint would say exactly that too.**
// `lessons.md` records the negative form of this -- *a no-op claim cannot be told apart from a
// not-wired-up claim* -- and these three are its positive form: a feature that moved the whole
// frame cannot be told from a constant that moved the whole frame, and the control table's
// "18 of 18 move" row is the number that looks like proof and is not.
//
// So the three things the field claims to be, each read somewhere the sweep cannot reach:
// it varies with **position**, it varies with **direction**, and the direction is the **right
// way round**. The first two would pass on a badly broken build; only the third pins the sign.

/// A chunk origin whose columns show more than one surface block, found rather than hardcoded.
///
/// **Searched with an assert rather than written down**, which is `lessons.md`'s *put the assert
/// in the tool, not in the plan*: a hardcoded shoreline that stopped being a shoreline would
/// leave every test below passing vacuously over uniform grass, and nothing in any output would
/// say so.
fn mixed_surface_chunk(gen: &voxelcraft::worldgen::WorldGen) -> IVec3 {
    for cz in -4..4i32 {
        for cx in -4..4i32 {
            let o = IVec3::new(cx * 64, 128, cz * 64);
            let mut seen = std::collections::HashSet::new();
            for i in 0..64 {
                let (x, z) = (o.x + i, o.z + (i * 7) % 64);
                let h = gen.height(x, z);
                seen.insert(gen.surface_block_at(x, z, h));
            }
            if seen.len() >= 2 {
                return o;
            }
        }
    }
    panic!(
        "no chunk in the searched range shows two surface block types -- every test below \
         would pass vacuously over uniform ground, which is the failure mode that looks like a \
         pass"
    );
}

/// One chunk's heights, the way `stream::build` hands them to the bake.
fn heights_of(gen: &voxelcraft::worldgen::WorldGen, o: IVec3) -> Box<[i32; 64 * 64]> {
    let mut h = Box::new([0i32; 64 * 64]);
    for z in 0..64usize {
        for x in 0..64usize {
            h[x + z * 64] = gen.height(o.x + x as i32, o.z + z as i32);
        }
    }
    h
}

/// The bounce varies with **position**, which a constant cannot do.
#[test]
fn the_bounce_varies_across_a_chunk() {
    let gen = voxelcraft::worldgen::WorldGen::new(1337);
    let o = mixed_surface_chunk(&gen);
    let d = probe::bake(&gen, &heights_of(&gen, o), o, true);

    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for &v in &d.bounce {
        lo = lo.min(v);
        hi = hi.max(v);
    }
    assert!(
        hi / lo > 1.5,
        "the bounce spans {lo} to {hi} across one chunk over mixed ground -- a field that is \
         really keyed on the ground under each probe cannot be this flat, and a global tint \
         would read exactly 1.0 here while still moving all 18 vantages"
    );
}

/// The bounce varies with **direction**, which is what six slabs are for.
///
/// If `bounce_lobes` returned one lobe for every axis, or if the azimuthal weighting were
/// dropped, every axis at a probe would hold the same mean and this would read exactly 0 --
/// while the picture still changed everywhere and every other test in this file still passed.
#[test]
fn the_bounce_varies_between_axes() {
    let gen = voxelcraft::worldgen::WorldGen::new(1337);
    let o = mixed_surface_chunk(&gen);
    let d = probe::bake(&gen, &heights_of(&gen, o), o, true);

    let mut worst = 0.0f32;
    for pz in 0..probe::PER_AXIS {
        for py in 0..probe::PER_AXIS {
            for px in 0..probe::PER_AXIS {
                // The four horizontal axes, which are the ones carrying an azimuthal lobe.
                let l = block::luma(d.get_bounce(0, px, py, pz));
                let r = block::luma(d.get_bounce(1, px, py, pz));
                let b = block::luma(d.get_bounce(4, px, py, pz));
                let f = block::luma(d.get_bounce(5, px, py, pz));
                let spread = l.max(r).max(b).max(f) - l.min(r).min(b).min(f);
                worst = worst.max(spread);
            }
        }
    }
    assert!(
        worst > 0.05,
        "the widest disagreement between a probe's four horizontal axes is {worst} -- the field \
         is carrying no direction, which makes six slabs of storage a per-position tint with \
         five copies"
    );
}

/// And the direction is the **right way round**, which is the only one of the three that can
/// catch a sign error.
///
/// **The expectation is recomputed by a different route on purpose.** The bake weights its
/// samples by an azimuthal cosine lobe and a radial form factor; this takes a plain unweighted
/// mean of the surface albedo in each half-plane. A transliteration of the gather would agree
/// with a mirrored lobe table as happily as with a correct one, and a mirrored lobe table is
/// exactly the bug that would leave every other assertion in this file passing.
#[test]
fn the_brighter_ground_is_on_the_brighter_axis() {
    let gen = voxelcraft::worldgen::WorldGen::new(1337);
    let o = mixed_surface_chunk(&gen);
    let d = probe::bake(&gen, &heights_of(&gen, o), o, true);

    // The probe whose -X and +X slabs disagree most, which is where the sign is legible.
    let (mut best, mut at) = (0.0f32, (0usize, 0usize, 0usize));
    for pz in 0..probe::PER_AXIS {
        for py in 0..probe::PER_AXIS {
            for px in 0..probe::PER_AXIS {
                let gap = block::luma(d.get_bounce(1, px, py, pz))
                    - block::luma(d.get_bounce(0, px, py, pz));
                if gap.abs() > best.abs() {
                    best = gap;
                    at = (px, py, pz);
                }
            }
        }
    }
    assert!(
        best.abs() > 0.05,
        "no probe's -X and +X slabs disagree by more than {} -- there is no sign to check",
        best.abs()
    );

    // The ground on each side, by a plain box mean out to the radius the form factor favours.
    let (px, _, pz) = at;
    let cx = o.x + probe::SPACING / 2 + probe::SPACING * px as i32;
    let cz = o.z + probe::SPACING / 2 + probe::SPACING * pz as i32;
    let mut side = [0.0f32; 2];
    for (s, acc) in side.iter_mut().enumerate() {
        let sign = if s == 0 { -1 } else { 1 };
        let mut sum = 0.0;
        let mut n = 0.0;
        for dx in 1..=24i32 {
            for dz in -24..=24i32 {
                let (x, z) = (cx + sign * dx, cz + dz);
                let h = gen.height(x, z);
                sum += block::luma(block::ALBEDO[gen.surface_block_at(x, z, h) as usize]);
                n += 1.0;
            }
        }
        *acc = sum / n;
    }

    assert_eq!(
        best > 0.0,
        side[1] > side[0],
        "the +X slab is {} than the -X slab while the ground to +X is {} than the ground to -X \
         ({:.4} against {:.4}) -- the azimuthal lobe is mirrored, which is a bug no picture \
         would read as wrong and every other test here passes through",
        if best > 0.0 { "brighter" } else { "darker" },
        if side[1] > side[0] { "brighter" } else { "darker" },
        side[1],
        side[0]
    );
}

/// Batch 60, roadmap R3. The second bounce can only take light away, and it takes some.
///
/// **Both halves are the assertion and neither alone is.** That the shadowed field is nowhere
/// *brighter* is the claim that the term is an occlusion rather than a re-colouring -- it
/// weights the gather's numerator and leaves its weight alone, so every probe can only lose.
/// That it is somewhere *darker* is the claim that it fires at all, which is the failure
/// `docs/lessons.md` calls indistinguishable from a feature that was never wired up: a bake
/// that ignored `bounce_shadow` entirely would pass the first assertion perfectly.
#[test]
fn shadowed_ground_bounces_no_more_than_open_ground() {
    let gen = voxelcraft::worldgen::WorldGen::new(1337);
    let o = mixed_surface_chunk(&gen);
    let h = heights_of(&gen, o);
    let open = probe::bake(&gen, &h, o, false);
    let shaded = probe::bake(&gen, &h, o, true);
    let mut moved = 0usize;
    for (i, (a, b)) in shaded.bounce.iter().zip(open.bounce.iter()).enumerate() {
        assert!(
            *a <= *b + 1e-6,
            "channel {i} bounces {a} with the ground's own exposure and {b} without it -- the              exposure multiplies the gather's numerator by a number in (0, 1], so no probe              anywhere can come out brighter for knowing that its ground is shadowed"
        );
        if (*a - *b).abs() > 1e-6 {
            moved += 1;
        }
    }
    assert!(
        moved > 0,
        "not one of the {} stored channels moved between the two bakes over mixed terrain,          which is what a bake that never read `bounce_shadow` would also produce",
        shaded.bounce.len()
    );
}

/// The second bounce touches the bounce and **nothing else**.
///
/// Batch 57's occlusion cube shares the texel and shares the horizon march, and the exposure
/// term is computed from the same `col_h` that feeds `tan_h` -- so the one way this batch could
/// go wrong invisibly is by perturbing the sky occlusion on its way past. `--no-probe-cube` and
/// `--no-probe-shadow` would then stop being independent controls, and every batch-57 number in
/// the ledger would quietly stop describing the shipping build.
/// Batch 71, roadmap D1. The lattice upload can collide between two resident chunks only
/// when their origins differ by the field's 512-block toroidal period in an axis -- which
/// the plan's 160-block LOD-0 reach makes impossible in the steady state, and the unload
/// grace plus a teleport makes transiently possible. `sync_world` therefore defers any
/// bake outside `UPLOAD_REACH`, and these three tests are the constant's claim: inside
/// it, collisions are arithmetically impossible, so no order of writes -- hash, timing or
/// sorted -- can change the field.
///
/// The texel block a chunk owns starts at `(origin / SPACING) mod 64`, mirroring
/// `write_probe`; asserting against the formula here is the same shape the rest of this
/// file uses against the shader.
fn texel_block(origin: IVec3) -> [i32; 3] {
    let o = origin.to_array();
    [
        (o[0] / probe::SPACING).rem_euclid(render::PROBE_DIM_XZ as i32),
        o[1] / probe::SPACING,
        (o[2] / probe::SPACING).rem_euclid(render::PROBE_DIM_XZ as i32),
    ]
}

#[test]
fn two_uploadable_chunks_can_never_share_a_texel() {
    let cam = Vec3::new(1000.5, 80.0, -700.5);
    // Origins arrive on chunk grid; sweep them on one Y so equality of the texel block
    // really is the toroidal X/Z collision the race needed.
    let gx = (cam.x as i32 / 64) * 64;
    let gz = (cam.z as i32 / 64) * 64;
    let mut in_window = Vec::new();
    for ox in (-320..=384).step_by(64) {
        for oz in (-320..=384).step_by(64) {
            let o = IVec3::new(gx + ox, 64, gz + oz);
            if probe::uploadable(o, cam) {
                in_window.push(o);
            }
        }
    }
    assert!(!in_window.is_empty());
    for (i, &a) in in_window.iter().enumerate() {
        for &b in &in_window[i + 1..] {
            assert_ne!(
                texel_block(a),
                texel_block(b),
                "uploadable chunks {a} and {b} alias one 8x8x8 block on the torus"
            );
        }
    }
}

#[test]
fn the_alias_the_race_needed_is_real_outside_the_window() {
    // Guard the guard: the collision the window forbids is otherwise reachable -- the
    // chunk one period over maps to the very texels the near one does. If either of
    // these properties moves, the review above has to be re-done rather than assumed.
    let near = IVec3::new(64, 64, 128);
    let far = near + IVec3::new(512, 0, 0);
    assert_eq!(texel_block(near), texel_block(far));
    assert!(!probe::uploadable(far, near.as_vec3() + Vec3::new(32.0, 0.0, 32.0)));
}

#[test]
fn the_window_is_what_the_arithmetic_says_it_is() {
    // The reach computed from itself, not retyped: LOD 0's 64-block chunks are wanted
    // while their *center* is within `size * factor * fade_band` of the camera, and that
    // is the set a deferred bake must never belong to. The window is horizontal, so the
    // horizontal component of that reach -- no more than the reach itself at any camera
    // height -- is what has to fit, plus the corner's horizontal half-diagonal of
    // 32*sqrt(2) ~= 45.3: the origin a chunk center is measured from can be that far
    // out in XZ. 224 - (160 + 45.3) = 18.7 blocks of margin at *any* altitude; an
    // earlier spherical predicate spent the margin on camera height instead, and the
    // measured A/B is why the predicate is now XZ-only.
    let reach = (voxelcraft::voxel::DIM as f32)
        * voxelcraft::config::Config::default().lod.factor
        * voxelcraft::config::Config::default().lod.fade_band;
    let half_diagonal_xz = (2.0 * (voxelcraft::voxel::DIM as f32 / 2.0).powi(2)).sqrt();
    assert!(
        probe::UPLOAD_REACH > reach + half_diagonal_xz,
        "the plan can want a bake `uploadable` would defer"
    );
    // Chunk origins sit on a 64 grid, so the first representable distance over 448 is
    // 512 -- the alias distance itself. Two uploads both inside the window differ by at
    // most 2 * UPLOAD_REACH in XZ, and 448 just fits, so the bound holds with no grid
    // slack to spend: this assertion is what keeps that from being read as headroom.
    const {
        assert!((2.0 * probe::UPLOAD_REACH) <= 512.0 - 64.0);
    }
}

#[test]
fn a_deferred_bake_survives_to_its_own_window() {
    // `sync_world`'s re-queue, without the renderer: a far bake dropped back into the map
    // is offered again next frame, so a camera returning to its chunk uploads it rather
    // than leaving the texels on the field's identity fill.
    let mut w = World::new();
    let far = ChunkKey::new(0, IVec3::new(8, 0, 0));
    let data = probe::ProbeData {
        occl: vec![0.0; probe::AXES * probe::PROBES],
        bounce: vec![1.0; probe::AXES * probe::PROBES * 3],
    };
    w.set_probe(far, data);
    let cam = Vec3::new(32.0, 80.0, 32.0);
    for (key, data) in w.take_probe_dirty() {
        if probe::uploadable(key.origin(), cam) {
            panic!("the far chunk reported uploadable");
        } else {
            w.set_probe(key, data);
        }
    }
    let deferred: Vec<ChunkKey> = w.take_probe_dirty().into_keys().collect();
    assert_eq!(deferred, vec![far]);
    // And when the camera does come back, the same doorway passes it through.
    assert!(probe::uploadable(
        far.origin(),
        far.origin().as_vec3() + Vec3::new(32.0, 0.0, 32.0)
    ));
}

#[test]
fn the_second_bounce_leaves_the_occlusion_cube_alone() {
    let gen = voxelcraft::worldgen::WorldGen::new(1337);
    let o = mixed_surface_chunk(&gen);
    let h = heights_of(&gen, o);
    let open = probe::bake(&gen, &h, o, false);
    let shaded = probe::bake(&gen, &h, o, true);
    assert_eq!(
        open.occl, shaded.occl,
        "the sky occlusion moved when only the bounce's ground exposure was switched on"
    );
}



