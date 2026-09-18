use glam::Vec3;
use std::time::{Duration, Instant};
use voxelcraft::stream::ChunkManager;
use voxelcraft::voxel::World;

/// Extract a WGSL function after stripping line comments; braces are then code only.
pub fn wgsl_function(source: &str, name: &str) -> String {
    let code = source.lines().map(|l| l.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n");
    let pattern = format!("fn {name}(");
    let start = code.find(&pattern).expect("WGSL function exists");
    let open = start + code[start..].find('{').expect("function body");
    let mut depth = 0;
    for (offset, ch) in code[open..].char_indices() {
        if ch == '{' { depth += 1; }
        if ch == '}' { depth -= 1; if depth == 0 { return code[start..open+offset+1].into(); } }
    }
    panic!("unterminated WGSL function");
}

/// Drive streaming until the world settles, on a wall-clock budget.
///
/// The budget is deliberately wall-clock and generous rather than an iteration
/// count: the previous 6,000-iteration helper was a machine-speed assumption in
/// disguise. A 2-vCPU sandbox was observed to need ~260,000 iterations (~14 s
/// of `update` plus sleep) for the default world, so `lod_stream` and
/// `shadow_grid` failed with "world never settled" on machines the engine is
/// perfectly healthy on. A deadline makes the helper machine-independent, and
/// the panic names the budget so a genuine stall is not mistaken for a slow box.
pub fn settle(m: &mut ChunkManager, w: &mut World, cam: Vec3) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        m.update(cam, w, Duration::from_millis(50));
        if m.is_settled(w) {
            return;
        }
        if Instant::now() >= deadline {
            panic!("settle budget exhausted: the world had not settled after 30 s of driving -- a genuine streaming stall, or a machine too slow for the budget");
        }
        std::thread::sleep(Duration::from_micros(200));
    }
}
