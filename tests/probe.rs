mod support;
use glam::{IVec3, Vec3};
use voxelcraft::block;
use voxelcraft::probe;
use voxelcraft::render;
use voxelcraft::voxel::{ChunkKey, World};

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

#[test]
fn the_removal_is_gated_by_the_override_alone() {
    let src = render::shader_source();
    let sites: Vec<&str> = src
        .lines()
        .filter(|l| l.contains("SPEC_PROBE_AMBIENT") && !l.trim_start().starts_with("//"))
        .collect();
    assert!(
        sites.len() >= 7,
        "expected the removal to gate every site it claims -- blk_n, the corner tint, the amb          bilinear, both cheap branches, face_shade and the ambient floor -- found {}:
{}",
        sites.len(),
        sites.join("
")
    );
    for line in sites {
        assert!(
            !line.contains("frame.flags"),
            "a SPEC_PROBE_AMBIENT site reads frame.flags, so the driver cannot fold it and the              terms this batch claims to delete stay compiled into all four of shade_hit's              inlined copies. Line was: {line}"
        );
    }
}

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

#[test]
fn a_buried_probe_falls_back_to_the_table() {

    let buried = probe::cube_from_tangents(&[64.0; probe::AZIMUTHS]);
    for (id, &v) in buried.iter().enumerate() {
        assert!(
            (v - 1.0).abs() < 1e-3,
            "a probe with no sky has to return 1 on every axis, or a cave multiplies to black. \
             Axis {id} got {v}"
        );
    }

    let slot = probe::cube_from_tangents(&[3.0; probe::AZIMUTHS]);
    assert!(
        slot[3] > 0.2 && slot[3] < 0.95,
        "a probe inside the ramp has to be lifted off its raw occlusion of 0.1 without \
         reaching 1. Got {}",
        slot[3]
    );
}

#[test]
fn a_one_sided_horizon_darkens_only_the_face_that_looks_at_it() {
    let mut tan = [0.0f32; probe::AZIMUTHS];

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

#[test]
fn reference_ground_leaves_the_exposure_alone() {

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

fn heights_of(gen: &voxelcraft::worldgen::WorldGen, o: IVec3) -> Box<[i32; 64 * 64]> {
    let mut h = Box::new([0i32; 64 * 64]);
    for z in 0..64usize {
        for x in 0..64usize {
            h[x + z * 64] = gen.height(o.x + x as i32, o.z + z as i32);
        }
    }
    h
}

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

#[test]
fn the_bounce_varies_between_axes() {
    let gen = voxelcraft::worldgen::WorldGen::new(1337);
    let o = mixed_surface_chunk(&gen);
    let d = probe::bake(&gen, &heights_of(&gen, o), o, true);

    let mut worst = 0.0f32;
    for pz in 0..probe::PER_AXIS {
        for py in 0..probe::PER_AXIS {
            for px in 0..probe::PER_AXIS {

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

#[test]
fn the_brighter_ground_is_on_the_brighter_axis() {
    let gen = voxelcraft::worldgen::WorldGen::new(1337);
    let o = mixed_surface_chunk(&gen);
    let d = probe::bake(&gen, &heights_of(&gen, o), o, true);

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

    let near = IVec3::new(64, 64, 128);
    let far = near + IVec3::new(512, 0, 0);
    assert_eq!(texel_block(near), texel_block(far));
    assert!(!probe::uploadable(far, near.as_vec3() + Vec3::new(32.0, 0.0, 32.0)));
}

#[test]
fn the_window_is_what_the_arithmetic_says_it_is() {

    let reach = (voxelcraft::voxel::DIM as f32)
        * voxelcraft::config::Config::default().lod.factor
        * voxelcraft::config::Config::default().lod.fade_band;
    let half_diagonal_xz = (2.0 * (voxelcraft::voxel::DIM as f32 / 2.0).powi(2)).sqrt();
    assert!(
        probe::UPLOAD_REACH > reach + half_diagonal_xz,
        "the plan can want a bake `uploadable` would defer"
    );

    const {
        assert!((2.0 * probe::UPLOAD_REACH) <= 512.0 - 64.0);
    }
}

#[test]
fn a_deferred_bake_survives_to_its_own_window() {

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
