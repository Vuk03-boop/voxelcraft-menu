//! The entry point, and the dispatch to one of four modes.
//!
//! Batch 22b split the rest of this file into three modules, on the line of **who calls
//! what** rather than by subject: [`scene`] is what the window and the headless modes both
//! need, [`headless`] is the modes the measurement fixture drives, and [`app`] is the window
//! loop. It was 1,509 lines holding all three plus this dispatch.

mod app;
mod headless;
mod scene;

use voxelcraft::config::{parse_args, Mode};

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let (cfg, mode) = parse_args();
    match mode {
        Mode::BenchTerrain { chunks } => headless::bench_terrain(&cfg, chunks),
        Mode::Screenshot { path } => {
            if let Err(e) = headless::screenshot(&cfg, &path) {
                eprintln!("screenshot failed: {e}");
                std::process::exit(1);
            }
        }
        Mode::Lookbook { dir } => {
            if let Err(e) = headless::lookbook(&cfg, &dir) {
                eprintln!("lookbook failed: {e}");
                std::process::exit(1);
            }
        }
        Mode::BenchFrames { frames } => {
            if let Err(e) = headless::bench_frames(&cfg, frames) {
                eprintln!("bench failed: {e}");
                std::process::exit(1);
            }
        }
        Mode::Diff { a, b, grid, crop } => {
            if let Err(e) = headless::diff(&a, &b, grid, crop) {
                eprintln!("diff failed: {e}");
                std::process::exit(1);
            }
        }
        Mode::Run => app::run(cfg),
    }
}



