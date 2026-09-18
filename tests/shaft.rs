//! The light envelope (batch 35): the scan, and the constants it is mirrored by.
//!
//! **What is testable here and what is not.** The scan runs on the GPU and nothing in
//! `tests/` opens a device, which is `app.rs`'s old complaint arriving at a new file. So the
//! split is the same one the biome field makes: the *algorithm* is pinned here against a
//! brute-force gather, in Rust, and the WGSL is a transliteration of the pinned algorithm.
//! That is worth doing rather than skipping, because a max-plus doubling scan is exactly the
//! kind of thing that is off by one doubling and still looks entirely plausible on screen --
//! it would simply have a shorter reach, which reads as "the far ridge does not cast", which
//! reads as a tuning problem.

use voxelcraft::shaft::{self, ShaftField};
use voxelcraft::worldgen::WorldGen;

/// The doubling scan, in Rust, over a slice of the window. A transliteration of
/// `shaft_scan` in `shaft.wgsl` and deliberately line for line with it: `src` and `dst`
/// alternate, pass `k` steps `2^k` texels, and the climb is the horizontal run times the
/// tangent of the sun's elevation.
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

/// The scan computes the envelope a brute-force gather does, to well inside the softness the
/// shadow edge is ramped over.
///
/// **This is the whole correctness claim of the batch, and the tolerance is the finding.** The
/// first version of this test asserted agreement to 1e-3, on the argument that a `(max, +)`
/// Hillis-Steele scan is *exact* over a window of `2^STEPS`. It is -- over an integer lattice.
/// The sun's azimuth is not axis-aligned, so every pass resamples bilinearly, and a value
/// carried three texels by a one-step and a two-step pass has been interpolated twice where
/// the gather interpolates it once. The two are therefore near rather than equal, and the
/// honest bound is not a float epsilon but [`shaft::SOFT`]: a disagreement smaller than the
/// ramp the edge is already blurred over cannot move a pixel. Tie the assert to that constant
/// rather than to a number typed here, so shrinking the ramp tightens the test with it.
///
/// Run over the real generator rather than a synthetic ramp, because a ramp is monotone and a
/// monotone field would pass under several wrong reaches: what separates them is a ridge with
/// ground behind it, which is what terrain is and what a test fixture would have to be
/// carefully built to be.
#[test]
fn the_scan_is_the_envelope_it_claims_to_be() {
    let gen = WorldGen::new(1337);
    let mut field = ShaftField::new();
    field.update(&gen, 0.0, 0.0, 0.0);

    // A low sun, which is the case the batch exists for: a high one makes every envelope
    // equal to the terrain and the test would pass on a scan that did nothing at all.
    let sun = [0.82f32, 0.18, 0.54];
    let got = scan(&field, sun, shaft::DIM);

    // Sampled rather than exhaustive: the reference is `2^STEPS` taps per column, so the
    // whole window would be 67 million of them. A stride that is not a power of two is what
    // keeps the sample from landing on the scan's own doubling boundaries every time.
    let mut worst = 0.0f32;
    let mut checked = 0;
    let mut moved = 0;
    for tz in (0..shaft::DIM).step_by(37) {
        for tx in (0..shaft::DIM).step_by(37) {
            let want = field.envelope_reference(sun, tx, tz);
            let have = got[tz * shaft::DIM + tx];
            worst = worst.max((want - have).abs());
            checked += 1;
            // How many of the sampled columns the scan actually shadowed above their own
            // ground. Without this the assert above is satisfied by a scan that returned the
            // heights untouched, since the reference would then have to be wrong the same way.
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

/// A column behind a ridge is shadowed and the ridge itself is not.
///
/// The sign convention is the one thing here a sweep cannot catch: an envelope propagated the
/// wrong way along the sun vector is still a smooth, plausible field, still agrees with a
/// reference that shares the error, and puts every shaft on the sunward side of its ridge.
#[test]
fn the_shadow_falls_away_from_the_sun() {
    let gen = WorldGen::new(7);
    let mut field = ShaftField::new();
    field.update(&gen, 0.0, 0.0, 0.0);
    let sun = [1.0f32, 0.25, 0.0];

    // The steepest single-texel rise along +X anywhere in the window: that is a cliff facing
    // the sun, and the texel behind it is where a shadow has to be. Searched over the whole
    // window and not one row, because one row of this generator does not reliably hold a step
    // worth testing -- the first version of this asked the middle row for an 8-block drop and
    // the biggest it had was 2.
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

    // The sun is at +X, so the shadow falls toward -X -- the low side of a step whose high
    // side is at `at`. Flip the sign of the propagation and this column is the lit one.
    let shadowed = field.envelope_reference(sun, at - 1, mid);
    let ground_below = field.heights()[mid * shaft::DIM + at - 1];
    assert!(
        shadowed > ground_below + 1.0,
        "the column below the cliff is not shadowed: envelope {shadowed}, ground {ground_below}"
    );

    // The other end of the claim, and it needs a column that is lit *by construction* rather
    // than by inspection. The cliff top is not one -- the steepest step in the window is not
    // the highest point in it, and an earlier version of this assert assumed it was and
    // failed on a ridge with more mountain still rising behind it. The window's global
    // maximum is the column that cannot be shadowed, because nothing anywhere is higher and
    // the beam only climbs on its way in.
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

/// An overhead sun casts no long shadow, and a level field casts none at all.
///
/// The first is the guard on the division by `length(sun.xz)` that `shaft_scan` clamps rather
/// than branches on; the second says the recurrence's `d = 0` term is present, since without
/// it a flat world would come back below its own ground.
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

/// The window re-anchors on the camera, and what it carries over is what it already had.
///
/// The scroll is the only part of this field that is an optimisation rather than a
/// definition, and the failure it can have is silent: a shifted window whose carried rows are
/// off by one is a heightfield that still looks like terrain.
#[test]
fn the_window_scrolls_without_changing_what_it_holds() {
    let gen = WorldGen::new(11);
    let mut a = ShaftField::new();
    a.update(&gen, 0.0, 0.0, 0.0);
    let origin_a = a.origin_world();

    // Far enough to shift and not so far as to replace: 40 texels of a 512-texel window.
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

/// An unmoved camera fills once. The whole cost argument for doing this on the CPU is that
/// every headless capture pays exactly one fill, so a second one would be a per-frame upload
/// nothing asked for.
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
    // Inside the same texel is still the same window -- the anchor is a texel and not a
    // position, which is what stops the envelope crawling under a walking player.
    field.update(&gen, 100.0 + (shaft::TEXEL as f32) * 0.4, -60.0, 0.0);
    assert!(!field.dirty(), "a sub-texel step refilled the window");
}

/// `shaft.rs`'s constants and `common.wgsl`'s copies of them are the same numbers.
///
/// The two halves cannot share a definition -- one is Rust and one is WGSL -- so this is the
/// thing that keeps them from drifting, and the drift would be quiet: a `SHAFT_DIM` the
/// shader disagreed with would index a correctly sized buffer with the wrong stride and
/// return a *plausible* row of the envelope, which is the same failure mode `block_faces`'
/// stride has and the same reason it is pinned rather than commented.
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

    // The slice the last pass writes, which is the one `terrain_shade` reads. Pass `k` writes
    // slice `k & 1`, so this is a statement about the parity of `STEPS` and not a free
    // constant -- `shaft.rs` carries the `assert!` and this is the shader's side of it.
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

/// The envelope at one column, over occluders between `m0` and `m0 + reach` texels up-sun.
///
/// `m0 = 0` is [`ShaftField::envelope_reference`] with the column's own ground folded in; any
/// other `m0` is the restricted maximum batch 37 claims to be able to ask for. Written here
/// rather than beside that method because only this file has a use for the restriction, and
/// the bilinear is the same rule both of them and `shaft_sample` follow.
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

/// **Batch 37's whole correctness claim: sampling the envelope `d` blocks up the sun ray is
/// the envelope at the surface with every occluder nearer than `d` struck out of the maximum.**
///
/// That identity is why `shade_hit` can hand the marcher `[0, shadow_dist]` and the envelope
/// `[shadow_dist, reach)` and have the two partition the ray rather than blend -- nothing falls
/// between them and nothing is counted twice. It is also why the batch authored no crossover
/// distance and no blend width: the two `D*tan` terms cancel, so there is nothing left to tune.
///
/// The algebra it is checking, with `dir` the sun's azimuth, `s` a horizontal run and
/// `tan = sun.y / horiz`:
///
/// ```text
///     E(q.xz) = D*tan + max over s >= D of ( h(p.xz + s*dir) - s*tan )
///     q.y     = p.y + d*sun.y = p.y + D*tan
/// ```
///
/// so `q.y > E(q.xz)` reduces to `p.y > max over s >= D`, which is the restricted comparison.
/// The test is in *texels* rather than blocks so both sides land on the same lattice points and
/// the claim is checked to float precision rather than to an interpolation tolerance -- the
/// shader's own drift against a gather is `the_scan_is_the_envelope_it_claims_to_be`'s subject
/// and is bounded there by `shaft::SOFT`, and this is a different claim that must not inherit
/// that slack.
///
/// **Two assertions under the identity, and the second is the one that keeps it honest.** A
/// restricted maximum can only be *lower* than the full one, so the distant term can never
/// darken a pixel the full envelope would have left lit -- that is what makes it safe to
/// multiply into a march rather than mix with it. And the restriction has to actually remove
/// something at a decent share of columns, or this would pass on a handoff of zero, which is
/// exactly the bug that would put the envelope's 4-block quantisation back on near geometry.
#[test]
fn the_offset_is_the_envelope_with_the_near_occluders_struck_out() {
    let gen = WorldGen::new(1337);
    let mut field = ShaftField::new();
    field.update(&gen, 0.0, 0.0, 0.0);

    // Low, for the reason the scan test is: a high sun makes every envelope equal to its own
    // ground and the identity would hold on a field that had nothing in it.
    let sun = [0.82f32, 0.18, 0.54];
    let horiz = (sun[0] * sun[0] + sun[2] * sun[2]).sqrt().max(1e-3);
    let dir = [sun[0] / horiz, sun[2] / horiz];
    let slope = sun[1] / horiz;

    // `shadow_dist` is 220 blocks and `TEXEL` is 4, so the handoff is 55 texels. Named as the
    // integer it has to be: a fractional offset would put the two sides on different lattice
    // points and turn a float-precision claim into an interpolation one.
    const HANDOFF_TEXELS: u32 = 55;
    let lift = HANDOFF_TEXELS as f32 * shaft::TEXEL as f32 * slope;

    let (mut worst, mut checked, mut restricted_lower, mut above_full) = (0.0f32, 0, 0, 0);
    for tz in (0..shaft::DIM).step_by(37) {
        for tx in (0..shaft::DIM).step_by(37) {
            let (fx, fz) = (tx as f32, tz as f32);
            // The restriction, stated in the surface's own frame.
            let want = envelope_from(&field, sun, fx, fz, HANDOFF_TEXELS);
            // The shader's form: one tap at the offset column, and the sun ray's own climb
            // over that offset subtracted back off. `terrain_shade_beyond` spends the climb by
            // raising the query point instead, which is the same subtraction on the other side.
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



