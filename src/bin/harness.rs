//! Measurement fixture driver for VoxelCraft.
//!
//! Renders nothing directly; drives the built voxelcraft executable by path.
//!
//! Pinned by tests/bins.rs:
//!   - harness_subcommands_are_the_documented_vocabulary
//!   - harness_flags_are_the_documented_vocabulary
//!
//! Flags:
//!   --vantage <name>      single vantage to run
//!   --only <names>        comma-separated vantage filter
//!   --rounds <N>          number of paired rounds for bench
//!   --out <dir>           destination directory for captures
//!   --rect <x0,y0,x1,y1>  explicit crop rectangle in fractions
//!   --crop <name>         named crop of a vantage
//!   --tiles               print per-cell grid breakdown
//!   --profile <N>         split comparison into N profile slices
//!   --mip0                force base mip level (--mip0 sets the engine's --mip-bias 0)
//!   --taa                 enable temporal anti-aliasing
//!   --samples <N>         sub-pixel reference grid samples
//!   --exe-a <path>        path to primary executable
//!   --exe-b <path>        path to secondary/revert executable
//!   --extra <args>        extra arguments passed to both executables
//!   --extra-b <args>      extra arguments passed only to secondary executable

use std::path::{Path, PathBuf};
use std::process::{self, Command};
use voxelcraft::harness::bench::{self, PASSES, RESOLVE};
use voxelcraft::harness::metric::{self, Img, Rect};
use voxelcraft::harness::vantage::{self, Content, VANTAGES};
use voxelcraft::harness::{self, Capture, Composes, Recipe, CHECKS, IMPLIES};

fn default_exe() -> PathBuf {
    let cand_unix = PathBuf::from("./target/release/voxelcraft");
    let cand_win = PathBuf::from("./target/release/voxelcraft.exe");
    if cand_win.exists() {
        cand_win
    } else if cand_unix.exists() {
        cand_unix
    } else if cfg!(windows) {
        PathBuf::from("./voxelcraft.exe")
    } else {
        PathBuf::from("./voxelcraft")
    }
}

struct Opts {
    subcommand: String,
    vantage: Option<String>,
    only: Option<String>,
    rounds: usize,
    out: PathBuf,
    rect: Option<[f32; 4]>,
    crop: Option<String>,
    tiles: bool,
    profile: Option<usize>,
    mip0: bool,
    taa: bool,
    samples: u32,
    exe_a: PathBuf,
    exe_b: Option<PathBuf>,
    extra: Vec<String>,
    extra_b: Vec<String>,
    compare_files: Vec<String>,
}

impl Opts {
    fn parse() -> Opts {
        let mut args: Vec<String> = std::env::args().skip(1).collect();
        if args.is_empty() {
            print_usage();
            process::exit(2);
        }

        let subcommand = args.remove(0);
        let mut vantage = None;
        let mut only = None;
        let mut rounds = 8;
        let mut out = PathBuf::from("target/harness/capture");
        let mut rect = None;
        let mut crop = None;
        let mut tiles = false;
        let mut profile = None;
        let mut mip0 = false;
        let mut taa = false;
        let mut samples = 16;
        let mut exe_a = default_exe();
        let mut exe_b = None;
        let mut extra = Vec::new();
        let mut extra_b = Vec::new();
        let mut compare_files = Vec::new();

        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            if !arg.starts_with("--") {
                compare_files.push(arg.clone());
                i += 1;
                continue;
            }

            match arg.as_str() {
                "--vantage" => {
                    i += 1;
                    vantage = args.get(i).cloned();
                }
                "--only" => {
                    i += 1;
                    only = args.get(i).cloned();
                }
                "--rounds" => {
                    i += 1;
                    if let Some(r) = args.get(i).and_then(|s| s.parse().ok()) {
                        rounds = r;
                    }
                }
                "--out" => {
                    i += 1;
                    if let Some(p) = args.get(i) {
                        out = PathBuf::from(p);
                    }
                }
                "--rect" => {
                    i += 1;
                    if let Some(s) = args.get(i) {
                        let parts: Vec<f32> = s.split(',').filter_map(|p| p.parse().ok()).collect();
                        if parts.len() == 4 {
                            rect = Some([parts[0], parts[1], parts[2], parts[3]]);
                        }
                    }
                }
                "--crop" => {
                    i += 1;
                    crop = args.get(i).cloned();
                }
                "--tiles" => {
                    tiles = true;
                }
                "--profile" => {
                    i += 1;
                    profile = args.get(i).and_then(|s| s.parse().ok());
                }
                "--mip0" => {
                    // --mip0 sets the engine's --mip-bias 0
                    mip0 = true;
                }
                "--taa" => {
                    taa = true;
                }
                "--samples" => {
                    i += 1;
                    if let Some(s) = args.get(i).and_then(|s| s.parse().ok()) {
                        samples = s;
                    }
                }
                "--exe-a" => {
                    i += 1;
                    if let Some(p) = args.get(i) {
                        exe_a = PathBuf::from(p);
                    }
                }
                "--exe-b" => {
                    i += 1;
                    if let Some(p) = args.get(i) {
                        exe_b = Some(PathBuf::from(p));
                    }
                }
                "--extra" => {
                    i += 1;
                    if let Some(s) = args.get(i) {
                        extra.extend(s.split_whitespace().map(String::from));
                    }
                }
                "--extra-b" => {
                    i += 1;
                    if let Some(s) = args.get(i) {
                        extra_b.extend(s.split_whitespace().map(String::from));
                    }
                }
                unknown => {
                    eprintln!("harness: unknown flag `{unknown}`");
                    eprintln!("valid flags: --vantage --only --rounds --out --rect --crop --tiles --profile --mip0 --taa --samples --exe-a --exe-b --extra --extra-b");
                    process::exit(2);
                }
            }
            i += 1;
        }

        Opts {
            subcommand,
            vantage,
            only,
            rounds,
            out,
            rect,
            crop,
            tiles,
            profile,
            mip0,
            taa,
            samples,
            exe_a,
            exe_b,
            extra,
            extra_b,
            compare_files,
        }
    }
}

fn print_usage() {
    eprintln!("Usage: harness <subcommand> [flags]");
    eprintln!("Subcommands: \"list\", \"capture\", \"validate\", \"implies\", \"bitexact\", \"bench\", \"march\", \"reference\", \"compare\"");
}

fn main() {
    let opts = Opts::parse();

    match opts.subcommand.as_str() {
        "list" => cmd_list(),
        "capture" => cmd_capture(&opts),
        "validate" => cmd_validate(&opts),
        "implies" => cmd_implies(&opts),
        "bitexact" => cmd_bitexact(&opts),
        "bench" => cmd_bench(&opts),
        "march" => cmd_march(&opts),
        "reference" => cmd_reference(&opts),
        "compare" => cmd_compare(&opts),
        other => {
            eprintln!("harness: unknown subcommand `{other}`");
            print_usage();
            process::exit(2);
        }
    }
}

fn cmd_list() {
    for v in VANTAGES {
        println!("{:<16} ({} crops)", v.name, v.crops.len());
        for c in v.crops {
            let kind = match c.content {
                Content::Water => "water",
                Content::Land => "land",
                Content::Sky => "sky",
                Content::Mixed => "mixed",
            };
            println!("  crop {:<14} [{kind:<5}] ({:.3}, {:.3}, {:.3}, {:.3})", c.name, c.x0, c.y0, c.x1, c.y1);
        }
    }
}

fn resolve_vantages(opts: &Opts) -> Vec<&'static vantage::Vantage> {
    if let Some(ref v_name) = opts.vantage {
        match vantage::find(v_name) {
            Some(v) => vec![v],
            None => {
                eprintln!("harness: unknown vantage `{v_name}`");
                process::exit(2);
            }
        }
    } else if let Some(ref only_str) = opts.only {
        match vantage::select(only_str) {
            Ok(list) => list,
            Err(e) => {
                eprintln!("harness: {e}");
                process::exit(2);
            }
        }
    } else {
        VANTAGES.iter().collect()
    }
}

fn cmd_capture(opts: &Opts) {
    let vantages = resolve_vantages(opts);
    std::fs::create_dir_all(&opts.out).unwrap_or_default();
    let recipe = Recipe {
        width: 1280,
        height: 720,
        taa: opts.taa,
    };

    println!("capturing {} vantage(s) to {}...", vantages.len(), opts.out.display());
    for v in vantages {
        let out_file = opts.out.join(format!("{}.png", v.name));
        match harness::render(&opts.exe_a, &out_file, v, &recipe, &opts.extra) {
            Ok(cap) => {
                let status = if cap.truncated { " (TRUNCATED)" } else { "" };
                println!("  {:<16} -> {}{status}", v.name, out_file.display());
            }
            Err(e) => {
                eprintln!("  {:<16} FAILED: {e}", v.name);
                process::exit(1);
            }
        }
    }
}

fn cmd_validate(opts: &Opts) {
    let recipe = Recipe {
        width: 1280,
        height: 720,
        taa: opts.taa,
    };

    let tmp_dir = std::env::temp_dir().join("voxelcraft_harness_validate");
    std::fs::create_dir_all(&tmp_dir).unwrap_or_default();

    let mut failed = 0;
    println!("running {} metric validation check(s)...", CHECKS.len());

    for (idx, chk) in CHECKS.iter().enumerate() {
        if let Some(ref only) = opts.only {
            if !only.split(',').any(|s| s.trim() == chk.vantage) {
                continue;
            }
        }

        let v = match vantage::find(chk.vantage) {
            Some(v) => v,
            None => {
                eprintln!("check {idx}: unknown vantage `{}`", chk.vantage);
                failed += 1;
                continue;
            }
        };

        // Render baseline (A) with both flags
        let out_a = tmp_dir.join(format!("val_{idx}_a.png"));
        let mut extra_a = opts.extra.clone();
        extra_a.extend(chk.both.iter().map(|s| s.to_string()));

        let cap_a = match harness::render(&opts.exe_a, &out_a, v, &recipe, &extra_a) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("check {idx} ({}) render A failed: {e}", chk.vantage);
                failed += 1;
                continue;
            }
        };

        // Render control (B) with control + both flags
        let out_b = tmp_dir.join(format!("val_{idx}_b.png"));
        let mut extra_b = opts.extra.clone();
        extra_b.extend(chk.control.iter().map(|s| s.to_string()));
        extra_b.extend(chk.both.iter().map(|s| s.to_string()));

        let cap_b = match harness::render(&opts.exe_a, &out_b, v, &recipe, &extra_b) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("check {idx} ({}) render B failed: {e}", chk.vantage);
                failed += 1;
                continue;
            }
        };

        if cap_a.truncated || cap_b.truncated {
            eprintln!("check {idx}: geometry truncated");
            failed += 1;
            continue;
        }

        let img_a = match Img::load(&out_a) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("check {idx}: {e}");
                failed += 1;
                continue;
            }
        };
        let img_b = match Img::load(&out_b) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("check {idx}: {e}");
                failed += 1;
                continue;
            }
        };

        let rect = if chk.crop.is_empty() {
            Rect::full(img_a.w, img_a.h)
        } else if let Some(c) = v.crop(chk.crop) {
            c.rect(img_a.w, img_a.h)
        } else {
            Rect::full(img_a.w, img_a.h)
        };

        let (passed, val_str) = match chk.metric {
            "mae" => {
                let m = metric::mae(&img_a, &img_b, rect);
                match chk.expect {
                    harness::Response::DeltaAtLeast(floor) => (m >= floor, format!("MAE {m:.2} >= {floor:.2}")),
                    _ => (false, "unexpected response type for MAE".into()),
                }
            }
            "max" => {
                let m = metric::max_delta(&img_a, &img_b, rect) as f64;
                match chk.expect {
                    harness::Response::DeltaAtLeast(floor) => (m >= floor, format!("max delta {m:.0} >= {floor:.0}")),
                    _ => (false, "unexpected response type for max".into()),
                }
            }
            "pixels" => {
                let m = metric::pixels_differing(&img_a, &img_b, rect) as f64;
                match chk.expect {
                    harness::Response::DeltaAtLeast(floor) => (m >= floor, format!("pixels {m:.0} >= {floor:.0}")),
                    _ => (false, "unexpected response type for pixels".into()),
                }
            }
            "speckle" => {
                let sa = metric::speckle(&img_a, rect);
                let sb = metric::speckle(&img_b, rect);
                let ratio = sb / sa.max(1e-6);
                match chk.expect {
                    harness::Response::RatioAtMost(r) => (ratio <= r, format!("ratio {ratio:.3} <= {r:.3}")),
                    harness::Response::RatioAtLeast(r) => (ratio >= r, format!("ratio {ratio:.3} >= {r:.3}")),
                    _ => (false, "unexpected response type".into()),
                }
            }
            "rg_std" => {
                let ra = metric::rg_std(&img_a, rect);
                let rb = metric::rg_std(&img_b, rect);
                let ratio = rb / ra.max(1e-6);
                match chk.expect {
                    harness::Response::RatioAtMost(r) => (ratio <= r, format!("ratio {ratio:.3} <= {r:.3}")),
                    harness::Response::RatioAtLeast(r) => (ratio >= r, format!("ratio {ratio:.3} >= {r:.3}")),
                    _ => (false, "unexpected response type".into()),
                }
            }
            "rg_speckle" => {
                let ra = metric::rg_speckle(&img_a, rect);
                let rb = metric::rg_speckle(&img_b, rect);
                let ratio = rb / ra.max(1e-6);
                match chk.expect {
                    harness::Response::RatioAtMost(r) => (ratio <= r, format!("ratio {ratio:.3} <= {r:.3}")),
                    harness::Response::RatioAtLeast(r) => (ratio >= r, format!("ratio {ratio:.3} >= {r:.3}")),
                    _ => (false, "unexpected response type".into()),
                }
            }
            "coherence.peak" => {
                let ca = metric::coherence(&img_a, rect).peak;
                let cb = metric::coherence(&img_b, rect).peak;
                let ratio = cb / ca.max(1e-6);
                match chk.expect {
                    harness::Response::RatioAtLeast(r) => (ratio >= r, format!("ratio {ratio:.3} >= {r:.3}")),
                    harness::Response::RatioAtMost(r) => (ratio <= r, format!("ratio {ratio:.3} <= {r:.3}")),
                    _ => (false, "unexpected response type".into()),
                }
            }
            other => (false, format!("unknown metric `{other}`")),
        };

        if passed {
            println!("  [PASS] {:<12} {:<12} {:<14} -> {val_str}", chk.vantage, chk.crop, chk.metric);
        } else {
            eprintln!("  [FAIL] {:<12} {:<12} {:<14} -> {val_str}", chk.vantage, chk.crop, chk.metric);
            failed += 1;
        }
    }

    if failed > 0 {
        eprintln!("\nvalidation failed: {failed} check(s) did not meet response floor");
        process::exit(1);
    } else {
        println!("\nvalidation: all checks passed");
    }
}

fn cmd_implies(opts: &Opts) {
    let recipe = Recipe {
        width: 1280,
        height: 720,
        taa: opts.taa,
    };

    let tmp_dir = std::env::temp_dir().join("voxelcraft_harness_implies");
    std::fs::create_dir_all(&tmp_dir).unwrap_or_default();

    let mut failed = 0;
    println!("verifying {} control implication claim(s)...", IMPLIES.len());

    for (idx, imp) in IMPLIES.iter().enumerate() {
        let vantages = match vantage::select(imp.only) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("implication {idx}: {e}");
                failed += 1;
                continue;
            }
        };

        let mut imp_pass = true;
        for v in vantages {
            let out_a = tmp_dir.join(format!("imp_{idx}_{}_a.png", v.name));
            let mut extra_a = opts.extra.clone();
            extra_a.extend(imp.base.iter().map(|s| s.to_string()));

            let out_b = tmp_dir.join(format!("imp_{idx}_{}_b.png", v.name));
            let mut extra_b = opts.extra.clone();
            extra_b.extend(imp.base.iter().map(|s| s.to_string()));
            extra_b.extend(imp.also.iter().map(|s| s.to_string()));

            if harness::render(&opts.exe_a, &out_a, v, &recipe, &extra_a).is_err()
                || harness::render(&opts.exe_a, &out_b, v, &recipe, &extra_b).is_err()
            {
                imp_pass = false;
                break;
            }

            let img_a = Img::load(&out_a).unwrap();
            let img_b = Img::load(&out_b).unwrap();
            let diff = metric::pixels_differing(&img_a, &img_b, Rect::full(img_a.w, img_a.h));

            match imp.expect {
                Composes::Nothing => {
                    if diff != 0 {
                        eprintln!("  [FAIL] claim {idx} on {}: expected 0 differing pixels, got {diff}", v.name);
                        imp_pass = false;
                    }
                }
                Composes::Something => {
                    if diff == 0 {
                        eprintln!("  [FAIL] claim {idx} on {}: expected differing pixels, got 0", v.name);
                        imp_pass = false;
                    }
                }
            }
        }

        if imp_pass {
            println!("  [PASS] claim {idx}: base {:?} also {:?}", imp.base, imp.also);
        } else {
            failed += 1;
        }
    }

    if failed > 0 {
        eprintln!("\nimplies failed: {failed} claim(s) did not hold");
        process::exit(1);
    } else {
        println!("\nimplies: all claims verified");
    }
}

fn cmd_bitexact(opts: &Opts) {
    let exe_b = opts.exe_b.as_ref().unwrap_or(&opts.exe_a);
    let vantages = resolve_vantages(opts);
    let recipe = Recipe {
        width: 1280,
        height: 720,
        taa: opts.taa,
    };

    let tmp_dir = std::env::temp_dir().join("voxelcraft_harness_bitexact");
    std::fs::create_dir_all(&tmp_dir).unwrap_or_default();

    println!("bitexact sweep: {} vantage(s) comparing {} against {}", vantages.len(), opts.exe_a.display(), exe_b.display());
    let mut any_diff = false;

    for v in vantages {
        let out_a = tmp_dir.join(format!("bit_{}_a.png", v.name));
        let out_b = tmp_dir.join(format!("bit_{}_b.png", v.name));

        let mut args_b = opts.extra.clone();
        if !opts.extra_b.is_empty() {
            args_b = opts.extra_b.clone();
        }

        let cap_a = match harness::render(&opts.exe_a, &out_a, v, &recipe, &opts.extra) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{:<16} render A error: {e}", v.name);
                any_diff = true;
                continue;
            }
        };

        let cap_b = match harness::render(exe_b, &out_b, v, &recipe, &args_b) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{:<16} render B error: {e}", v.name);
                any_diff = true;
                continue;
            }
        };

        if cap_a.truncated || cap_b.truncated {
            eprintln!("{:<16} geometry truncated", v.name);
            any_diff = true;
            continue;
        }

        let img_a = Img::load(&out_a).unwrap();
        let img_b = Img::load(&out_b).unwrap();
        let rect = Rect::full(img_a.w, img_a.h);

        let diff = metric::pixels_differing(&img_a, &img_b, rect);
        let mae = metric::mae(&img_a, &img_b, rect);
        let max_d = metric::max_delta(&img_a, &img_b, rect);

        if diff == 0 {
            println!("{:<16} bit-exact (0 pixels differing, MAE 0.0000, max 0)", v.name);
        } else {
            eprintln!("{:<16} DIFFER: {diff} pixels differing, MAE {mae:.4}, max {max_d}", v.name);
            any_diff = true;
        }
    }

    if any_diff {
        eprintln!("\nbitexact: regression detected (renders differed)");
        process::exit(1);
    } else {
        println!("\nbitexact: all vantages bit-exact");
    }
}

fn cmd_bench(opts: &Opts) {
    let exe_b = opts.exe_b.as_ref().unwrap_or(&opts.exe_a);
    let v_name = opts.vantage.as_deref().unwrap_or("coastline");
    let v = vantage::find(v_name).unwrap_or_else(|| {
        eprintln!("bench: unknown vantage `{v_name}`");
        process::exit(2);
    });

    let args_b = if opts.extra_b.is_empty() {
        opts.extra.clone()
    } else {
        opts.extra_b.clone()
    };

    println!(
        "bench: vantage `{}`, {} paired rounds, comparing {} to {}",
        v.name,
        opts.rounds,
        opts.exe_a.display(),
        exe_b.display()
    );

    let paired = bench::paired(
        &opts.exe_a,
        &opts.extra,
        exe_b,
        &args_b,
        v.args,
        opts.rounds,
        |round, _a, _b, _first| {
            print!(".");
            use std::io::Write;
            std::io::stdout().flush().ok();
        },
    );
    println!();

    let res = match paired {
        Ok(p) => p,
        Err(e) => {
            eprintln!("bench failed: {e}");
            process::exit(1);
        }
    };

    println!("\n=== Paired Bench Results (Hi-Z ON) ===");
    println!("{:<10} {:>10} {:>10} {:>10} {:>10}", "Pass", "A mean", "B mean", "Delta", "StdErr");
    println!("{:-<54}", "");

    for (idx, &name) in PASSES.iter().enumerate() {
        let cell = res.cell(true, idx);
        println!(
            "{:<10} {:>9.3}m {:>9.3}m {:>+9.3}m {:>9.3}m",
            name, cell.a_mean, cell.b_mean, cell.delta, cell.stderr
        );
    }

    let r_cell = res.cell(true, RESOLVE);
    println!("\nSummary: resolve {:+.3} ms +/- {:.3}", r_cell.delta, r_cell.stderr);
}

fn cmd_march(opts: &Opts) {
    let vantages = resolve_vantages(opts);
    let mut args = opts.extra.clone();
    args.push("--march-stats".into());

    println!("march-stats sweep across {} vantage(s)...", vantages.len());
    let tmp_png = std::env::temp_dir().join("voxelcraft_harness_march.png");

    for v in vantages {
        let mut cmd = Command::new(&opts.exe_a);
        cmd.arg("--screenshot").arg(&tmp_png);
        cmd.args(v.args);
        cmd.args(&args);

        match cmd.output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                println!("--- vantage {} ---", v.name);
                for line in stdout.lines() {
                    if line.contains("march-stats:") || line.contains("pairs marched") || line.contains("steps") {
                        println!("  {line}");
                    }
                }
            }
            Err(e) => {
                eprintln!("{}: failed to run: {e}", v.name);
            }
        }
    }
}

fn cmd_reference(opts: &Opts) {
    let v_name = opts.vantage.as_deref().unwrap_or("lattice");
    let v = vantage::find(v_name).unwrap_or_else(|| {
        eprintln!("reference: unknown vantage `{v_name}`");
        process::exit(2);
    });

    let mut args = opts.extra.clone();
    args.push("--reference".into());
    args.push(opts.samples.to_string());

    if opts.mip0 {
        // Map --mip0 to the engine's --mip-bias 0
        args.push("--mip-bias".into());
        args.push("0".into());
    }

    let out_png = opts.out.join(format!("ref_{}.png", v.name));
    if let Some(parent) = out_png.parent() {
        std::fs::create_dir_all(parent).unwrap_or_default();
    }

    let mut cmd = Command::new(&opts.exe_a);
    cmd.arg("--screenshot").arg(&out_png);
    cmd.args(v.args);
    cmd.args(&args);

    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("reference run failed: {e}");
            process::exit(1);
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("{stdout}");

    // Invariant check: must refuse a reference run with no noise-floor line
    if !stdout.contains("noise floor") {
        eprintln!("error: reference output contained no noise floor measurement");
        process::exit(1);
    }
}

fn cmd_compare(opts: &Opts) {
    if opts.compare_files.len() < 2 {
        eprintln!("compare requires two .png files: harness compare a.png b.png");
        process::exit(2);
    }

    let img_a = match Img::load(Path::new(&opts.compare_files[0])) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("failed to load {}: {e}", opts.compare_files[0]);
            process::exit(1);
        }
    };

    let img_b = match Img::load(Path::new(&opts.compare_files[1])) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("failed to load {}: {e}", opts.compare_files[1]);
            process::exit(1);
        }
    };

    let rect = if let Some(q) = opts.rect {
        let f = |v: f32, m: u32| (v.clamp(0.0, 1.0) * m as f32).round() as u32;
        Rect {
            x0: f(q[0], img_a.w),
            y0: f(q[1], img_a.h),
            x1: f(q[2], img_a.w),
            y1: f(q[3], img_a.h),
        }
    } else if let Some(ref v_name) = opts.vantage {
        let v = vantage::find(v_name).unwrap();
        if let Some(ref c_name) = opts.crop {
            v.crop(c_name).map(|c| c.rect(img_a.w, img_a.h)).unwrap_or(Rect::full(img_a.w, img_a.h))
        } else {
            Rect::full(img_a.w, img_a.h)
        }
    } else {
        Rect::full(img_a.w, img_a.h)
    };

    let diff = metric::pixels_differing(&img_a, &img_b, rect);
    let mae = metric::mae(&img_a, &img_b, rect);
    let max_d = metric::max_delta(&img_a, &img_b, rect);

    println!(
        "differing {} of {} ({:.2}%)  MAE {:.4}  max {}",
        diff,
        rect.pixels(),
        (diff as f64 / rect.pixels().max(1) as f64) * 100.0,
        mae,
        max_d
    );

    if opts.tiles {
        println!("tile coherence / difference breakdown:");
        let tiles = metric::coherence_tiles(&img_a, rect);
        for (peak, lag) in tiles.iter().take(10) {
            println!("  tile lag ({}, {}) -> peak correlation {:.4}", lag.0, lag.1, peak);
        }
    }

    if let Some(p) = opts.profile {
        println!("profile slices ({p}):");
        let step = (rect.x1 - rect.x0) / p as u32;
        for i in 0..p as u32 {
            let slice = Rect {
                x0: rect.x0 + i * step,
                y0: rect.y0,
                x1: rect.x0 + (i + 1) * step,
                y1: rect.y1,
            };
            let slice_mae = metric::mae(&img_a, &img_b, slice);
            println!("  slice {:2}: MAE {:.4}", i, slice_mae);
        }
    }
}
