//! Paired interleaved benching, with the dispersion `--bench-frames` does not print.
//!
//! Batch 24's whole content, and it exists for the reason `vantage.rs` exists: every batch
//! since 8 has hand-rolled a bench A/B, the word they all used for the protocol was
//! "interleaved", and that word does not specify one. Batch 20's `--no-wave-shoal` control
//! measured **+0.07 to +0.11 ms while compiling byte for byte identical to its revert**, which
//! is impossible, and it sat in the roadmap as an emergency for four batches. It was the
//! protocol.
//!
//! The mechanism, measured rather than supposed. `resolve` at the coastline over sixteen
//! consecutive runs of the *identical* command:
//!
//! ```text
//! 9.25 9.25 9.27 9.32 9.32 9.32 9.37 9.37 9.37 9.37 9.37 9.43 9.43 9.43 9.43 9.43
//! ```
//!
//! A monotone +0.18 ms warm-up ramp, and the true difference between those runs is zero by
//! construction. Analysed as first-half against second-half -- which is what any protocol that
//! puts one side's runs systematically later in the session computes -- that null experiment
//! reports **+0.099 ms**, squarely inside the range the emergency had been carrying.
//!
//! Three rules follow, and all three are in [`paired`]:
//!
//! - **Pair inside a round.** Both halves of a pair see nearly the same machine state, so the
//!   ramp cancels instead of being charged to whichever side ran later.
//! - **Alternate which side runs first.** Pairing alone leaves a within-pair ordering effect --
//!   the second process of a pair starts on a warmer GPU, worth about +0.03 ms here -- and
//!   alternating is what cancels *that* rather than charging it all to B.
//! - **Report a standard error.** `--bench-frames` prints a mean per pass and nothing else, so
//!   a single run cannot say whether a difference is real, and the difference this project
//!   most often wants to detect is about 1% of the pass.

use std::path::Path;
use std::process::Command;

use super::UNKNOWN_ARG_MARK;

/// The documented bench configuration. 1920x1080 is what every `PERF.md` cost table since
/// batch 8 was taken at, and changing it would make this command's numbers incomparable with
/// all of them.
pub const WIDTH: u32 = 1920;
pub const HEIGHT: u32 = 1080;
/// Frames per run. The bench discards the first 20 as warm-up, so this is 120 scored frames --
/// batch 20's count, kept for the same comparability reason.
pub const FRAMES: u32 = 140;

/// The per-pass columns the bench prints, in its order, plus `total` appended.
pub const PASSES: &[&str] = &[
    "select", "march", "hiz", "recover", "march2", "resolve", "taa", "total",
];
/// Index of `resolve` in [`PASSES`]. It is 55-70% of the frame and is the pass every cost
/// question in this project has turned out to be about, so it is what the summary line quotes.
pub const RESOLVE: usize = 5;

/// One process: the two blocks the bench prints, hi-z off and hi-z on.
///
/// Both are kept because they are different questions. Hi-z off is the honest cost of `march`;
/// hi-z on is the shipping configuration. A change that moves one and not the other is saying
/// something, and a fixture that averaged them would hide it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Run {
    pub off: [f32; 8],
    pub on: [f32; 8],
}

/// Pull both blocks out of a bench's stdout.
///
/// Deliberately strict: a missing field is an error and never a zero. A parser that returned
/// `0.0` for a pass it could not find would make any A/B against it read as a large, confident
/// improvement.
pub fn parse(out: &str) -> Result<Run, String> {
    let mut blocks = Vec::new();
    for line in out.lines() {
        let l = line.trim();
        if !l.starts_with("gpu") {
            continue;
        }
        // `gpu: NVIDIA GeForce RTX 3050 Laptop GPU` is the adapter banner and also starts with
        // `gpu`. Requiring the first column's name is what separates the two, and getting this
        // wrong is a hard error rather than a silent miscount only because the loop below
        // treats an absent field as an error instead of a zero.
        let is_total = l.starts_with("gpu   total ");
        if !is_total && !l.contains(" select ") {
            continue;
        }
        if is_total {
            let rest = &l["gpu   total ".len()..];
            let v: f32 = rest
                .trim_end_matches("ms")
                .trim()
                .parse()
                .map_err(|_| format!("unparsable total: {l}"))?;
            match blocks.last_mut() {
                Some((_, t)) => *t = v,
                None => return Err(format!("a total with no pass line before it: {l}")),
            }
            continue;
        }
        // `gpu   select 0.13  march 2.26  hiz 0.00  recover 0.00  march2 0.00  resolve 9.20 ...`
        let mut vals = [0f32; 8];
        let toks: Vec<&str> = l.split_whitespace().collect();
        let mut found = 0;
        for (i, name) in PASSES.iter().enumerate().take(7) {
            let at = toks
                .iter()
                .position(|t| t == name)
                .ok_or_else(|| format!("no `{name}` in: {l}"))?;
            vals[i] = toks
                .get(at + 1)
                .ok_or_else(|| format!("`{name}` with no value: {l}"))?
                .parse()
                .map_err(|_| format!("unparsable `{name}`: {l}"))?;
            found += 1;
        }
        debug_assert_eq!(found, 7);
        blocks.push((vals, 0f32));
    }
    if blocks.len() != 2 {
        return Err(format!(
            "expected two `gpu` blocks (hi-z off then on), found {}",
            blocks.len()
        ));
    }
    let (mut off, mut on) = (blocks[0].0, blocks[1].0);
    off[7] = blocks[0].1;
    on[7] = blocks[1].1;
    Ok(Run { off, on })
}

/// One bench process at one vantage.
pub fn run(exe: &Path, vantage_args: &[&str], extra: &[String]) -> Result<Run, String> {
    let mut cmd = Command::new(exe);
    cmd.arg("--bench-frames").arg(FRAMES.to_string());
    cmd.arg("--width").arg(WIDTH.to_string());
    cmd.arg("--height").arg(HEIGHT.to_string());
    cmd.args(vantage_args);
    cmd.args(extra);
    let o = cmd
        .output()
        .map_err(|e| format!("running {}: {e}", exe.display()))?;
    let stdout = String::from_utf8_lossy(&o.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
    if !o.status.success() {
        return Err(format!("{} exited {}: {}", exe.display(), o.status, stderr.trim()));
    }
    // The same refusal `harness::render` makes, and the more dangerous of the two directions.
    // The parser drops a flag it cannot read and exits 0, so a misspelled control benches the
    // *same configuration on both sides* -- which reports a difference of zero, and zero is
    // exactly what a control is hoping to prove. An ignored flag in a bit-exactness sweep looks
    // like a pass; an ignored flag here looks like "free".
    if stderr.contains(UNKNOWN_ARG_MARK) {
        let ignored: Vec<&str> = stderr
            .lines()
            .filter(|l| l.contains(UNKNOWN_ARG_MARK))
            .map(str::trim)
            .collect();
        return Err(format!(
            "{} ignored an argument, so both sides of this A/B are the same run: {}",
            exe.display(),
            ignored.join("; ")
        ));
    }
    parse(&stdout)
}

/// Every round's reading of one side, paired with the other.
pub struct Paired {
    pub a: Vec<Run>,
    pub b: Vec<Run>,
}

/// Mean, paired delta and standard error for one (hi-z state, pass) cell.
pub struct Cell {
    pub a_mean: f32,
    pub b_mean: f32,
    pub delta: f32,
    pub stderr: f32,
    pub positive: usize,
}

impl Paired {
    pub fn rounds(&self) -> usize {
        self.a.len()
    }

    pub fn cell(&self, hiz_on: bool, pass: usize) -> Cell {
        let pick = |r: &Run| if hiz_on { r.on[pass] } else { r.off[pass] };
        let n = self.a.len() as f32;
        let a: Vec<f32> = self.a.iter().map(pick).collect();
        let b: Vec<f32> = self.b.iter().map(pick).collect();
        // The *paired* difference, never the difference of the means. They are equal in
        // expectation and not in variance: pairing is the whole point, and taking two means
        // separately throws the cancellation away.
        let d: Vec<f32> = a.iter().zip(&b).map(|(x, y)| y - x).collect();
        let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
        let dm = mean(&d);
        let stderr = if d.len() > 1 {
            let var = d.iter().map(|x| (x - dm) * (x - dm)).sum::<f32>() / (d.len() - 1) as f32;
            (var / n).sqrt()
        } else {
            f32::NAN
        };
        Cell {
            a_mean: mean(&a),
            b_mean: mean(&b),
            delta: dm,
            stderr,
            positive: d.iter().filter(|x| **x > 0.0).count(),
        }
    }
}

/// Run both sides `rounds` times, alternating which goes first, and return every reading.
///
/// `progress` is called after each round so the driver can print something during a run that
/// takes minutes; the fixture itself prints nothing.
pub fn paired(
    exe_a: &Path,
    args_a: &[String],
    exe_b: &Path,
    args_b: &[String],
    vantage_args: &[&str],
    rounds: usize,
    mut progress: impl FnMut(usize, &Run, &Run, bool),
) -> Result<Paired, String> {
    let mut out = Paired {
        a: Vec::new(),
        b: Vec::new(),
    };
    for r in 0..rounds {
        // Alternate. Pairing removes the session-long ramp; this removes the within-pair
        // ordering effect, which is the residue pairing alone leaves behind and is worth
        // about +0.03 ms on this machine -- half of what the batch-20 emergency reported.
        let a_first = r % 2 == 0;
        let (a, b) = if a_first {
            let a = run(exe_a, vantage_args, args_a)?;
            let b = run(exe_b, vantage_args, args_b)?;
            (a, b)
        } else {
            let b = run(exe_b, vantage_args, args_b)?;
            let a = run(exe_a, vantage_args, args_a)?;
            (a, b)
        };
        progress(r, &a, &b, a_first);
        out.a.push(a);
        out.b.push(b);
    }
    Ok(out)
}



