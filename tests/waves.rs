use std::f64::consts::PI;

const EXTS: [f64; 3] = [0.0, 1.79, 4.46];

const LAG_MIN: f64 = 8.0;
const LAG_MAX: f64 = 64.0;

const LAG_STEP: f64 = 0.25;

const FILTER_K: f64 = 1.0;

const LAMBDA0: f64 = 32.0;

#[derive(Debug, Clone, Copy)]
struct Octave {
    dir: [f64; 2],
    lambda: f64,
    amp: f64,
    rate: f64,
}

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

fn schedule(src: &str, prefix: &str) -> Vec<Octave> {
    let mut out = Vec::new();
    for (at, _) in src.match_indices("wave_octave(p, ") {
        let open = at + "wave_octave(p, ".len();
        let close = open + src[open..].find(')').expect("unterminated wave_octave call");
        let args: Vec<&str> = src[open..close].split(',').map(str::trim).collect();
        if !args[0].starts_with(prefix) {
            continue;
        }

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

fn fade(lambda: f64, ext: f64) -> f64 {
    (-FILTER_K * (ext / lambda).powi(2)).exp()
}

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

    let sep = min_separation(&shipped);
    assert!(
        sep < 15.0,
        "the shipped four are separated by {sep:.1} degrees, which contradicts batch 20's \
         finding that they are two tight clusters"
    );
}

#[test]
fn the_fill_schedule_subdivides_the_shipped_band() {
    let src = voxelcraft::render::shader_source();
    let fill = schedule(&src, "WAVE_F");
    assert_eq!(fill.len(), 8, "the fill schedule is not eight octaves");

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

    let r = (10.2f64).powf(1.0 / 7.0);
    for i in 1..8 {
        let got = fill[i - 1].lambda / fill[i].lambda;
        assert!(
            (got - r).abs() < 5e-3,
            "octave {i} is {got:.4} times the one before it, not the geometric {r:.4}"
        );
    }

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
