use voxelcraft::shaft::{self, ShaftField};
use voxelcraft::worldgen::WorldGen;

fn scan(field: &ShaftField, sun: [f32; 3], dim: usize) -> Vec<f32> {
    let horiz = (sun[0] * sun[0] + sun[2] * sun[2]).sqrt().max(1e-3);
    let dir = [sun[0] / horiz, sun[2] / horiz];
    let slope = sun[1] / horiz;

    let sample = |buf: &[f32], x: f32, z: f32| -> f32 {
        let hi = (dim - 1) as f32;
        let cx = x.clamp(0.0, hi);
        let cz = z.clamp(0.0, hi);
        let (x0f, z0f) = (cx.floor(), cz.floor());
        let (fx, fz) = (cx - x0f, cz - z0f);
        let (x0, z0) = (x0f as usize, z0f as usize);
        let (x1, z1) = ((x0 + 1).min(dim - 1), (z0 + 1).min(dim - 1));
        let a = buf[z0 * dim + x0];
        let b = buf[z0 * dim + x1];
        let c = buf[z1 * dim + x0];
        let d = buf[z1 * dim + x1];
        let top = a + (b - a) * fx;
        let bot = c + (d - c) * fx;
        top + (bot - top) * fz
    };

    let mut src: Vec<f32> = field.heights()[..dim * dim].to_vec();
    let mut dst = vec![0.0f32; dim * dim];
    for k in 0..shaft::STEPS {
        let span = (1u32 << k) as f32;
        let climb = span * shaft::TEXEL as f32 * slope;
        for z in 0..dim {
            for x in 0..dim {
                let here = src[z * dim + x];
                let carried =
                    sample(&src, x as f32 + dir[0] * span, z as f32 + dir[1] * span) - climb;
                dst[z * dim + x] = here.max(carried);
            }
        }
        std::mem::swap(&mut src, &mut dst);
    }
    src
}

#[test]
fn the_scan_is_the_envelope_it_claims_to_be() {
    let gen = WorldGen::new(1337);
    let mut field = ShaftField::new();
    field.update(&gen, 0.0, 0.0, 0.0);

    let sun = [0.82f32, 0.18, 0.54];
    let got = scan(&field, sun, shaft::DIM);

    let mut worst = 0.0f32;
    let mut checked = 0;
    let mut moved = 0;
    for tz in (0..shaft::DIM).step_by(37) {
        for tx in (0..shaft::DIM).step_by(37) {
            let want = field.envelope_reference(sun, tx, tz);
            let have = got[tz * shaft::DIM + tx];
            worst = worst.max((want - have).abs());
            checked += 1;

            if have > field.heights()[tz * shaft::DIM + tx] + 0.5 {
                moved += 1;
            }
        }
    }
    assert!(checked > 150, "only {checked} columns sampled");
    assert!(
        worst < shaft::SOFT * 0.5,
        "the scan and the gather disagree by {worst} blocks, which is no longer inside the \
         {} the edge is ramped over",
        shaft::SOFT
    );
    assert!(
        moved * 4 > checked,
        "only {moved} of {checked} columns are shadowed above their own ground, so this \
         would pass on a scan that did nothing"
    );
}

#[test]
fn the_shadow_falls_away_from_the_sun() {
    let gen = WorldGen::new(7);
    let mut field = ShaftField::new();
    field.update(&gen, 0.0, 0.0, 0.0);
    let sun = [1.0f32, 0.25, 0.0];

    let (mut best, mut at, mut row) = (0.0f32, 0usize, 0usize);
    for z in 0..shaft::DIM {
        for x in 1..shaft::DIM - 1 {
            let drop = field.heights()[z * shaft::DIM + x] - field.heights()[z * shaft::DIM + x - 1];
            if drop > best {
                best = drop;
                at = x;
                row = z;
            }
        }
    }
    assert!(best > 6.0, "no cliff anywhere in the window: best rise {best}");
    let mid = row;

    let shadowed = field.envelope_reference(sun, at - 1, mid);
    let ground_below = field.heights()[mid * shaft::DIM + at - 1];
    assert!(
        shadowed > ground_below + 1.0,
        "the column below the cliff is not shadowed: envelope {shadowed}, ground {ground_below}"
    );

    let (peak, _) = field
        .heights()
        .iter()
        .enumerate()
        .fold((0usize, f32::MIN), |(bi, bh), (i, &h)| {
            if h > bh {
                (i, h)
            } else {
                (bi, bh)
            }
        });
    let lit = field.envelope_reference(sun, peak % shaft::DIM, peak / shaft::DIM);
    let ground_peak = field.heights()[peak];
    assert!(
        lit <= ground_peak + 1e-3,
        "the highest column in the window shadows itself: envelope {lit}, ground {ground_peak}"
    );
}

#[test]
fn a_high_sun_leaves_the_terrain_alone() {
    let gen = WorldGen::new(3);
    let mut field = ShaftField::new();
    field.update(&gen, 0.0, 0.0, 0.0);

    let sun = [0.001f32, 1.0, 0.0];
    for tz in (0..shaft::DIM).step_by(101) {
        for tx in (0..shaft::DIM).step_by(101) {
            let e = field.envelope_reference(sun, tx, tz);
            let h = field.heights()[tz * shaft::DIM + tx];
            assert!(
                (e - h).abs() < 1e-3,
                "an overhead sun shadowed ({tx}, {tz}): envelope {e}, ground {h}"
            );
        }
    }
}

#[test]
fn the_window_scrolls_without_changing_what_it_holds() {
    let gen = WorldGen::new(11);
    let mut a = ShaftField::new();
    a.update(&gen, 0.0, 0.0, 0.0);
    let origin_a = a.origin_world();

    let shift = (40 * shaft::TEXEL) as f32;
    let mut b = ShaftField::new();
    b.update(&gen, shift, shift, 0.0);
    let origin_b = b.origin_world();
    assert_eq!(origin_b[0] - origin_a[0], shift);
    assert_eq!(origin_b[1] - origin_a[1], shift);

    let mut scrolled = ShaftField::new();
    scrolled.update(&gen, 0.0, 0.0, 0.0);
    scrolled.update(&gen, shift, shift, 0.0);
    assert_eq!(
        scrolled.origin_world(),
        origin_b,
        "a scrolled window landed somewhere a fresh one would not"
    );
    assert!(
        scrolled.heights() == b.heights(),
        "a window that scrolled into place does not hold what a fresh one there holds"
    );
}

#[test]
fn an_unmoved_camera_refills_nothing() {
    let gen = WorldGen::new(5);
    let mut field = ShaftField::new();
    field.update(&gen, 100.0, -60.0, 0.0);
    assert!(field.dirty(), "the first update has to fill");
    for _ in 0..8 {
        field.update(&gen, 100.0, -60.0, 0.0);
        assert!(!field.dirty(), "an unmoved camera refilled the window");
    }

    field.update(&gen, 100.0 + (shaft::TEXEL as f32) * 0.4, -60.0, 0.0);
    assert!(!field.dirty(), "a sub-texel step refilled the window");
}

#[test]
fn shaft_constants_match_the_shader() {
    let src = voxelcraft::render::shader_source();
    let read = |name: &str| -> u32 {
        let key = format!("const {name}: u32 = ");
        let at = src
            .find(&key)
            .unwrap_or_else(|| panic!("{name} is not declared in the shader"));
        let rest = &src[at + key.len()..];
        let end = rest.find('u').expect("a u32 literal ends in `u`");
        rest[..end].parse().expect("a decimal literal")
    };
    assert_eq!(read("SHAFT_DIM") as usize, shaft::DIM);
    assert_eq!(read("SHAFT_STEPS"), shaft::STEPS);

    let result_slice = (shaft::STEPS - 1) & 1;
    assert!(
        src.contains(&format!(
            "const SHAFT_RESULT: u32 = {};",
            if result_slice == 1 {
                "SHAFT_SLICE".to_string()
            } else {
                "0u".to_string()
            }
        )),
        "SHAFT_RESULT does not name the slice pass {} writes",
        shaft::STEPS - 1
    );
}

fn envelope_from(field: &ShaftField, sun: [f32; 3], tx: f32, tz: f32, m0: u32) -> f32 {
    let horiz = (sun[0] * sun[0] + sun[2] * sun[2]).sqrt().max(1e-3);
    let dir = [sun[0] / horiz, sun[2] / horiz];
    let slope = sun[1] / horiz;
    let dim = shaft::DIM;
    let sample = |x: f32, z: f32| -> f32 {
        let hi = (dim - 1) as f32;
        let (cx, cz) = (x.clamp(0.0, hi), z.clamp(0.0, hi));
        let (x0f, z0f) = (cx.floor(), cz.floor());
        let (fx, fz) = (cx - x0f, cz - z0f);
        let (x0, z0) = (x0f as usize, z0f as usize);
        let (x1, z1) = ((x0 + 1).min(dim - 1), (z0 + 1).min(dim - 1));
        let h = field.heights();
        let top = h[z0 * dim + x0] + (h[z0 * dim + x1] - h[z0 * dim + x0]) * fx;
        let bot = h[z1 * dim + x0] + (h[z1 * dim + x1] - h[z1 * dim + x0]) * fx;
        top + (bot - top) * fz
    };
    let reach = 1u32 << shaft::STEPS;
    let mut best = f32::NEG_INFINITY;
    for m in m0..=m0 + reach {
        let h = sample(tx + dir[0] * m as f32, tz + dir[1] * m as f32);
        best = best.max(h - m as f32 * shaft::TEXEL as f32 * slope);
    }
    best
}

#[test]
fn the_offset_is_the_envelope_with_the_near_occluders_struck_out() {
    let gen = WorldGen::new(1337);
    let mut field = ShaftField::new();
    field.update(&gen, 0.0, 0.0, 0.0);

    let sun = [0.82f32, 0.18, 0.54];
    let horiz = (sun[0] * sun[0] + sun[2] * sun[2]).sqrt().max(1e-3);
    let dir = [sun[0] / horiz, sun[2] / horiz];
    let slope = sun[1] / horiz;

    const HANDOFF_TEXELS: u32 = 55;
    let lift = HANDOFF_TEXELS as f32 * shaft::TEXEL as f32 * slope;

    let (mut worst, mut checked, mut restricted_lower, mut above_full) = (0.0f32, 0, 0, 0);
    for tz in (0..shaft::DIM).step_by(37) {
        for tx in (0..shaft::DIM).step_by(37) {
            let (fx, fz) = (tx as f32, tz as f32);

            let want = envelope_from(&field, sun, fx, fz, HANDOFF_TEXELS);

            let qx = fx + dir[0] * HANDOFF_TEXELS as f32;
            let qz = fz + dir[1] * HANDOFF_TEXELS as f32;
            let have = envelope_from(&field, sun, qx, qz, 0) - lift;

            worst = worst.max((want - have).abs());
            checked += 1;

            let full = envelope_from(&field, sun, fx, fz, 0);
            if want < full - 0.5 {
                restricted_lower += 1;
            }
            if want > full + 1e-2 {
                above_full += 1;
            }
        }
    }

    assert!(checked > 150, "only {checked} columns sampled");
    assert!(
        worst < 1e-2,
        "the offset query and the restricted maximum disagree by {worst} blocks, so the \
         partition `shade_hit` relies on is not exact and the handoff is either \
         double-counting the near field or leaving a gap in it"
    );
    assert_eq!(
        above_full, 0,
        "{above_full} columns have the restricted envelope *above* the full one, which is \
         impossible for a maximum over a subset -- the distant term could then darken a pixel \
         the whole envelope would have left lit"
    );
    assert!(
        restricted_lower * 4 > checked,
        "the handoff only removes an occluder at {restricted_lower} of {checked} columns, so \
         this would pass on an offset of zero -- which is the version that puts the envelope's \
         {}-block quantisation back on top of ground the marcher had already resolved",
        shaft::TEXEL
    );
}
