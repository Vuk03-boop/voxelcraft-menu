use std::path::Path;
use std::process::Command;

use super::UNKNOWN_ARG_MARK;

pub const WIDTH: u32 = 1920;
pub const HEIGHT: u32 = 1080;

pub const FRAMES: u32 = 140;

pub const PASSES: &[&str] = &[
    "select", "march", "hiz", "recover", "march2", "resolve", "taa", "total",
];

pub const RESOLVE: usize = 5;

#[derive(Clone, Copy, Debug, Default)]
pub struct Run {
    pub off: [f32; 8],
    pub on: [f32; 8],
}

pub fn parse(out: &str) -> Result<Run, String> {
    let mut blocks = Vec::new();
    for line in out.lines() {
        let l = line.trim();
        if !l.starts_with("gpu") {
            continue;
        }

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

pub struct Paired {
    pub a: Vec<Run>,
    pub b: Vec<Run>,
}

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
