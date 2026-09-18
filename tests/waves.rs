//! Batch 25's wave schedule, parsed out of the shader and scored.
//!
//! The batch's headline claim is a number about a *field*, not about an image: the peak
//! autocorrelation of the slope field over a pinned lag annulus falls from 0.999 to 0.79.
//! Every previous statement of that quantity in this project is unreproducible -- batch 20
//! quotes 0.903 in one paragraph and 0.839 three paragraphs later for the same field, and
//! batch 25's own brief gets 0.833 or 0.788 for it depending on a window size neither
//! document recorded. This file is the fixture that ends that, in the shape `tests/textures.rs`
//! already uses for cross-boundary constants: it reads `render::shader_source()`, which is the
//! only statement of what the module is, and re-derives the number from the constants actually
//! compiled into it.
//!
//! Why there is no window, no padding and no variance normalisation to pin here. The field is
//!
//! ```text
//!     g(p) = sum_k  d_k a_k fade_k cos(w_k . p + phi_k),   w_k = (2 pi / lambda_k) d_k
//! ```
//!
//! so over an infinite domain every cross term between distinct octaves integrates away and
//! the autocorrelation is exactly
//!
//! ```text
//!     rho(L) = sum_k v_k cos(w_k . L) / sum_k v_k,      v_k = (a_k fade_k)^2
//! ```
//!
//! -- a closed form in the constants, with no estimator in it. The three things the brief says
//! were unpinned are not choices this formulation has. What it *does* have to pin is the
//! footprint the band limit is evaluated at and the lag annulus the peak is taken over, and
//! those two are [`EXTS`] and [`LAG_MIN`]/[`LAG_MAX`] below.

use std::f64::consts::PI;

/// Footprints the band limit is evaluated at, in blocks, from `resolve.wgsl`'s own
/// expression. 0 is the raw field; 1.79 is 120 blocks out at 5 degrees of grazing and 4.46 is
/// 300 blocks out at the same angle -- the two rows of batch 25's brief where the lattice is
/// actually reported. Scoring at one footprint only is how a direction set gets tuned for a
/// vantage: at 4.46 the short octaves are faded to nothing, so their directions stop being
/// visible to the score and come back arbitrary.
const EXTS: [f64; 3] = [0.0, 1.79, 4.46];

/// The lag annulus. Below `LAG_MIN` every field scores near 1 trivially -- that is the central
/// lobe of the longest octave, which is the sea being smooth rather than the sea repeating.
/// `LAG_MAX` is twice the longest wavelength: a recurrence further out than that is not what
/// "the texture repeats" means at any vantage this engine has.
const LAG_MIN: f64 = 8.0;
const LAG_MAX: f64 = 64.0;
/// 0.25 blocks, which is a twelfth of the shortest octave -- fine enough that the grid cannot
/// step over a peak of the fastest term in the sum.
const LAG_STEP: f64 = 0.25;

/// `WAVE_FILTER_K` in `resolve.wgsl`.
const FILTER_K: f64 = 1.0;
/// `config::Config::wave_scale`'s default, which is what `l0` is in a stock run.
const LAMBDA0: f64 = 32.0;

/// One octave, as the shader declares it.
#[derive(Debug, Clone, Copy)]
struct Octave {
    dir: [f64; 2],
    lambda: f64,
    amp: f64,
    rate: f64,
}

/// Pull `const WAVE_x: vec2<f32> = vec2<f32>(a, b);` out of the module.
fn direction(src: &str, name: &str) -> [f64; 2] {
    let key = format!("const {name}: vec2<f32> = vec2<f32>(");
    let at = src
        .find(&key)
        .unwrap_or_else(|| panic!("{name} is not in the module any more"));
    let open = at + key.len();
    let close = open + src[open..].find(')').expect("unterminated direction");
    let v: Vec<f64> = src[open..close]
        .split(',')
        .map(|p| p.trim().parse().expect("a direction component"))
        .collect();
    assert_eq!(v.len(), 2, "{name} is not two components");
    [v[0], v[1]]
}

/// Pull the `wave_octave(p, WAVE_x, l0 / R, a * W, ph * P, ...)` call lines out of the module.
///
/// `prefix` is `WAVE_D` for the four shipped octaves and `WAVE_F` for the fill schedule, which
/// is why the two are named apart: the control path has to stay parseable next to the path
/// that replaced it, and a test that silently found only one of them would be the
/// `--demo-edits` failure -- a check that verifies nothing and reports success.
fn schedule(src: &str, prefix: &str) -> Vec<Octave> {
    let mut out = Vec::new();
    for (at, _) in src.match_indices("wave_octave(p, ") {
        let open = at + "wave_octave(p, ".len();
        let close = open + src[open..].find(')').expect("unterminated wave_octave call");
        let args: Vec<&str> = src[open..close].split(',').map(str::trim).collect();
        if !args[0].starts_with(prefix) {
            continue;
        }
        // `l0` alone is the shipped first octave; `l0 / R` is every other line.
        let ratio = match args[1].strip_prefix("l0") {
            Some(rest) if rest.trim().is_empty() => 1.0,
            Some(rest) => rest
                .trim()
                .strip_prefix('/')
                .expect("a wavelength that is not l0 or l0 / R")
                .trim()
                .parse()
                .expect("a wavelength divisor"),
            None => panic!("wave_octave wavelength is not built from l0"),
        };
        let amp: f64 = args[2]
            .strip_prefix("a *")
            .expect("an amplitude that is not a * W")
            .trim()
            .parse()
            .expect("an amplitude");
        let rate: f64 = args[3]
            .strip_prefix("ph *")
            .expect("a phase that is not ph * P")
            .trim()
            .parse()
            .expect("a phase rate");
        out.push(Octave {
            dir: direction(src, args[0]),
            lambda: LAMBDA0 / ratio,
            amp,
            rate,
        });
    }
    out
}

/// `exp(-K (ext/lambda)^2)`, `wave_octave`'s band limit, isotropic.
fn fade(lambda: f64, ext: f64) -> f64 {
    (-FILTER_K * (ext / lambda).powi(2)).exp()
}

/// Peak of `rho` over the pinned annulus, maximised over the pinned footprints.
fn peak(oct: &[Octave]) -> f64 {
    let mut worst: f64 = 0.0;
    for ext in EXTS {
        let v: Vec<f64> = oct
            .iter()
            .map(|o| (o.amp * fade(o.lambda, ext)).powi(2))
            .collect();
        let total: f64 = v.iter().sum();
        if total <= 0.0 {
            continue;
        }
        let w: Vec<[f64; 2]> = oct
            .iter()
            .map(|o| {
                let k = 2.0 * PI / o.lambda;
                [k * o.dir[0], k * o.dir[1]]
            })
            .collect();
        let n = (LAG_MAX / LAG_STEP) as i32;
        let mut best: f64 = -1.0;
        for iy in -n..=n {
            let ly = iy as f64 * LAG_STEP;
            for ix in -n..=n {
                let lx = ix as f64 * LAG_STEP;
                let r2 = lx * lx + ly * ly;
                if r2 < LAG_MIN * LAG_MIN || r2 > LAG_MAX * LAG_MAX {
                    continue;
                }
                let mut rho = 0.0;
                for (wi, vi) in w.iter().zip(&v) {
                    rho += vi * (wi[0] * lx + wi[1] * ly).cos();
                }
                best = best.max(rho / total);
            }
        }
        worst = worst.max(best);
    }
    worst
}

/// Smallest pairwise angular separation, in degrees, on the circle mod 180.
fn min_separation(oct: &[Octave]) -> f64 {
    let ang: Vec<f64> = oct
        .iter()
        .map(|o| o.dir[1].atan2(o.dir[0]).rem_euclid(PI))
        .collect();
    let mut m = f64::MAX;
    for i in 0..ang.len() {
        for j in (i + 1)..ang.len() {
            let d = (ang[i] - ang[j]).abs().rem_euclid(PI).to_degrees();
            m = m.min(d.min(180.0 - d));
        }
    }
    m
}

/// The metric can see the defect it is used to rule out.
///
/// Batch 22's rule, and the reason it is a test rather than a remark: a measure paired with no
/// known-positive is not fit to rule anything out, and this one is being used to say the fill
/// schedule fixed something. The four shipped octaves are the positive -- they are what batch
/// 20 diagnosed as a plaid -- and they have to score near 1 for the number quoted about their
/// replacement to mean anything.
#[test]
fn the_repeat_metric_sees_the_four_octave_field() {
    let src = voxelcraft::render::shader_source();
    let shipped = schedule(&src, "WAVE_D");
    assert_eq!(
        shipped.len(),
        4,
        "the pre-batch-25 control path is not four octaves any more"
    );
    let p = peak(&shipped);
    println!("four-octave peak autocorrelation: {p:.4}");
    assert!(
        p >= 0.95,
        "the four-octave field scores {p:.4}, so this metric can no longer see the repeat it \
         is quoted about -- every number derived from it is unsupported"
    );
    // And the clustering batch 20 measured: 11.4, 24.1, 112.9, 118.5 degrees, two pairs.
    let sep = min_separation(&shipped);
    assert!(
        sep < 15.0,
        "the shipped four are separated by {sep:.1} degrees, which contradicts batch 20's \
         finding that they are two tight clusters"
    );
}

/// The fill schedule is what `FLAG_WAVE_FILL`'s documentation says it is.
#[test]
fn the_fill_schedule_subdivides_the_shipped_band() {
    let src = voxelcraft::render::shader_source();
    let fill = schedule(&src, "WAVE_F");
    assert_eq!(fill.len(), 8, "the fill schedule is not eight octaves");

    // Same band as the four it replaces. The point of the batch is the *spacing*: octaves
    // added below the short end land where batch 19's filter has already faded them out.
    assert!(
        (fill[0].lambda - LAMBDA0).abs() < 1e-6,
        "the longest fill octave is {:.4} and not l0",
        fill[0].lambda
    );
    let shortest = LAMBDA0 / 10.2;
    assert!(
        (fill[7].lambda - shortest).abs() < 1e-3,
        "the shortest fill octave is {:.4} where the shipped band ends at {shortest:.4}",
        fill[7].lambda
    );

    // Geometric, so no two octaves sit on top of each other in frequency either.
    let r = (10.2f64).powf(1.0 / 7.0);
    for i in 1..8 {
        let got = fill[i - 1].lambda / fill[i].lambda;
        assert!(
            (got - r).abs() < 5e-3,
            "octave {i} is {got:.4} times the one before it, not the geometric {r:.4}"
        );
    }

    // Deep-water dispersion: short waves travel slower, rate = sqrt(frequency ratio). The
    // shipped four do this and the animation reads as a sea because of it.
    for o in &fill {
        let want = (LAMBDA0 / o.lambda).sqrt();
        assert!(
            (o.rate - want).abs() < 5e-3,
            "an octave of wavelength {:.4} has phase rate {:.4}, not the dispersive {want:.4}",
            o.lambda,
            o.rate
        );
    }
}

/// The sea is exactly as rough as it was, and the amplitudes are on the shipped law.
///
/// This is what lets the batch be a change of *spectrum* rather than a change of sea state,
/// and it is what keeps `--wave-amp` and `--wave-scale` meaning what they meant. A schedule
/// that quietly carried more total slope variance would score better on the repeat for the
/// most boring possible reason.
#[test]
fn the_fill_schedule_keeps_the_shipped_slope_variance() {
    let src = voxelcraft::render::shader_source();
    let shipped = schedule(&src, "WAVE_D");
    let fill = schedule(&src, "WAVE_F");

    let var = |o: &[Octave]| -> f64 { o.iter().map(|o| o.amp * o.amp).sum() };
    let (a, b) = (var(&shipped), var(&fill));
    assert!(
        (a - b).abs() <= 0.005 * a,
        "the fill schedule carries slope variance {b:.4} against the shipped {a:.4} -- the \
         two seas are not equally rough, so any comparison between them is confounded"
    );

    // w ~ lambda^0.536, which is what `0.66 = 2.17^-0.536` means in the shipped field.
    for o in &fill {
        let want = fill[0].amp * (o.lambda / LAMBDA0).powf(0.536);
        assert!(
            (o.amp - want).abs() <= 0.01 * want,
            "an octave of wavelength {:.4} has amplitude {:.4} where the shipped amplitude \
             law wants {want:.4}",
            o.lambda,
            o.amp
        );
    }
}

/// The directions are unit, spread, and the field they make does not repeat the way the four
/// did.
///
/// The separation floor is the batch's one authored constraint and it is not free: the
/// unconstrained optimum put two octaves at the *same* angle and scored 0.03 to 0.05 better
/// for it. A batch whose whole premise is batch 20's "the four are really two, crossed" cannot
/// ship an exact duplicate to buy back a hundredth, so the search is constrained and the cost
/// is recorded in the batch doc rather than hidden.
#[test]
fn the_fill_directions_are_spread_and_the_field_does_not_repeat() {
    let src = voxelcraft::render::shader_source();
    let fill = schedule(&src, "WAVE_F");

    for o in &fill {
        let n = (o.dir[0] * o.dir[0] + o.dir[1] * o.dir[1]).sqrt();
        assert!(
            (n - 1.0).abs() < 1e-5,
            "a wave direction has length {n:.6}: an octave whose direction is not unit has \
             both the wrong wavelength and the wrong amplitude"
        );
    }

    let sep = min_separation(&fill);
    assert!(
        sep >= 14.9,
        "two fill octaves are {sep:.1} degrees apart, under the 15 the search was \
         constrained to -- that is the clustering batch 20 diagnosed, reintroduced"
    );

    let p = peak(&fill);
    println!("fill peak autocorrelation: {p:.4}, min separation {sep:.1} deg");
    assert!(
        p <= 0.86,
        "the fill schedule's peak autocorrelation is {p:.4}; it shipped at 0.79 and the four \
         octaves it replaced score 0.999, so this is most of the batch undone"
    );
}



