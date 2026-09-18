use glam::Vec3;
use std::sync::Arc;
use std::time::Duration;
use voxelcraft::lod::{LodConfig, Planner, RenderItem, Residency};
use voxelcraft::stream::ChunkManager;
use voxelcraft::voxel::World;
use voxelcraft::worldgen::WorldGen;

const FRAME: Duration = Duration::from_micros(7400);

fn mgr(cfg: LodConfig) -> (ChunkManager, World) {
    let gen = Arc::new(WorldGen::new(1337));
    (ChunkManager::new(cfg, gen), World::new())
}

fn settle(m: &mut ChunkManager, w: &mut World, cam: Vec3) {
    // A wall-clock budget, not an iteration count. The loop advances one 50 ms frame
    // per step and the whole selection has to stream in, so a fixed cap encodes the
    // speed of the machine that set it: this world settles in 6-14 s on 2 cores and
    // inside 6000 on 16 threads, and `docs/ledger.md` row 101-hw closed the original
    // report as a sandbox artifact for exactly that reason. `docs/audit-triage.md`
    // section 3.1 has the readings. A budget that cannot be met on a slow box is a
    // test that measures the host, so the claim here is only `it settles in time`.
    const BUDGET: Duration = Duration::from_secs(90);
    let start = std::time::Instant::now();
    let mut iters = 0u32;
    while start.elapsed() < BUDGET {
        iters += 1;
        m.update(cam, w, Duration::from_millis(50));
        if m.is_settled(w) {
            return;
        }
        std::thread::sleep(Duration::from_micros(200));
    }
    panic!(
        "settle budget of {:?} exhausted after {} updates on this machine: the world is still streaming, which is a budget too small and not a world that cannot settle",
        BUDGET, iters
    );
}

fn covering(render: &[RenderItem], p: Vec3) -> impl Iterator<Item = RenderItem> + '_ {
    render.iter().copied().filter(move |it| {
        let b = it.key.bounds();
        p.cmpge(b.min).all() && p.cmplt(b.max).all()
    })
}

fn coverage(render: &[RenderItem], p: Vec3) -> f32 {
    covering(render, p).map(|it| it.share()).sum()
}

fn mix_lod(render: &[RenderItem], p: Vec3) -> Option<f32> {
    let mut acc = 0.0;
    let mut w = 0.0;
    for it in covering(render, p) {
        acc += it.share() * it.key.lod as f32;
        w += it.share();
    }
    (w > 0.0).then_some(acc / w)
}

fn drawn_lod(render: &[RenderItem], p: Vec3) -> Option<u8> {
    covering(render, p)
        .max_by(|a, b| a.share().total_cmp(&b.share()))
        .map(|it| it.key.lod)
}

fn are_partners(a: RenderItem, b: RenderItem) -> bool {
    let (coarse, fine) = if a.key.lod > b.key.lod {
        (a, b)
    } else {
        (b, a)
    };
    if coarse.fade >= 0.0 || fine.fade <= 0.0 || coarse.fade != -fine.fade {
        return false;
    }
    let mut k = fine.key;
    while k.lod < coarse.key.lod {
        k = k.parent();
    }
    k == coarse.key
}

fn probe_grid(gen: &WorldGen) -> Vec<Vec3> {
    let mut v = Vec::new();
    for x in (-900..=-100).step_by(48) {
        for z in (100..=1300).step_by(48) {
            v.push(Vec3::new(
                x as f32 + 0.5,
                gen.height(x, z) as f32 - 1.5,
                z as f32 + 0.5,
            ));
        }
    }
    v
}

fn sweep(cfg: LodConfig, from: Vec3, to: Vec3, steps: usize) -> Vec<Vec<RenderItem>> {
    let mut planner = Planner::default();
    let mut desired = Vec::new();
    let mut out = Vec::with_capacity(steps);
    for i in 0..steps {
        let mut render = Vec::new();
        let cam = from.lerp(to, i as f32 / (steps - 1) as f32);
        planner.plan(cam, &cfg, &|_| Residency::Solid, &mut desired, &mut render);
        out.push(render);
    }
    out
}

const A: Vec3 = Vec3::new(-450.0, 140.0, 1100.0);
const B: Vec3 = Vec3::new(-450.0, 140.0, 300.0);

#[test]
fn plan_covers_every_point_exactly_once() {
    let gen = WorldGen::new(1337);
    let probes = probe_grid(&gen);
    let cases = [
        (
            "popping",
            LodConfig {
                cross_fade: false,
                ..Default::default()
            },
        ),
        ("default", LodConfig::default()),
        (
            "band 1.0",
            LodConfig {
                fade_band: 1.0,
                ..Default::default()
            },
        ),
        (
            "band 3.0",
            LodConfig {
                fade_band: 3.0,
                ..Default::default()
            },
        ),
        (
            "factor 1.0",
            LodConfig {
                factor: 1.0,
                ..Default::default()
            },
        ),
    ];
    let mut faded = Vec::new();
    for (name, cfg) in cases {
        let plans = sweep(cfg, A, B, 200);
        let mut n = 0;
        for (step, render) in plans.iter().enumerate() {
            for &p in &probes {
                let c = coverage(render, p);
                assert!(
                    (c - 1.0).abs() < 1e-4,
                    "{name}, step {step}: {p} covered {c} times"
                );
            }
            n += render.iter().filter(|it| it.is_fading()).count();
        }
        println!("  {name}: {n} fading entries over 200 plans");
        faded.push(n);
    }
    assert_eq!(faded[0], 0, "--no-fade emitted a cross-fade");
    assert!(faded[1] > 0, "the default plan never faded anything");
    assert_eq!(faded[2], 0, "a zero-width band emitted a cross-fade");
}

#[test]
fn overlaps_are_only_cross_fade_partners() {
    let plans = sweep(LodConfig::default(), A, B, 60);
    let mut peak = 0;
    for (step, render) in plans.iter().enumerate() {
        peak = peak.max(render.len());
        assert!(
            render.len() < 4096,
            "step {step}: draw list is {} (MAX_CHUNKS is 8191)",
            render.len()
        );
        for (i, &x) in render.iter().enumerate() {
            for &y in &render[i + 1..] {
                if x.key.bounds().intersects(&y.key.bounds()) {
                    assert!(are_partners(x, y), "step {step}: {x:?} overlaps {y:?}");
                }
            }
        }
    }
    println!("  peak draw list {peak}");
}

#[test]
fn cross_fade_removes_the_pop() {
    let gen = WorldGen::new(1337);
    let probes = probe_grid(&gen);
    let mut worst = [0.0f32; 2];
    for (i, cross_fade) in [false, true].into_iter().enumerate() {
        let cfg = LodConfig {
            cross_fade,
            ..Default::default()
        };
        let plans = sweep(cfg, A, B, 300);
        for pair in plans.windows(2) {
            for &p in &probes {
                if let (Some(a), Some(b)) = (mix_lod(&pair[0], p), mix_lod(&pair[1], p)) {
                    worst[i] = worst[i].max((b - a).abs());
                }
            }
        }
    }
    println!(
        "  worst per-frame LOD step: {:.3} popping, {:.3} dissolving",
        worst[0], worst[1]
    );
    assert!(
        worst[0] > 0.99,
        "popping plan never stepped a whole level ({:.3})",
        worst[0]
    );
    assert!(
        worst[1] < 0.34,
        "dissolving plan stepped {:.3} of a level in one frame",
        worst[1]
    );
}

#[test]
fn stationary_camera_streams_nothing() {
    let (mut m, mut w) = mgr(LodConfig::default());
    let cam = Vec3::new(-450.0, 140.0, 512.0);
    settle(&mut m, &mut w, cam);
    let mut interned = 0;

    let mut first_at = None;
    let mut pending_then = 0usize;
    for f in 0..400usize {
        m.update(cam, &mut w, Duration::from_secs_f32(0.002));
        interned += m.last_drained;
        if first_at.is_none() && m.last_drained != 0 {
            first_at = Some(f);
            pending_then = m.pending();
        }
        std::thread::sleep(FRAME);
    }
    assert_eq!(
        interned, 0,
        "stationary camera interned {interned} chunks (first at frame {first_at:?},          pending then {pending_then})"
    );
    assert_eq!(m.pending(), 0);
}

fn flight(cfg: LodConfig, from: Vec3, to: Vec3, probes: &[Vec3]) -> (usize, usize) {
    let (mut m, mut w) = mgr(cfg);
    settle(&mut m, &mut w, from);
    let mut prev: Vec<Option<u8>> = probes
        .iter()
        .map(|&p| drawn_lod(&m.render_set, p))
        .collect();
    let (mut flips, mut losses) = (0, 0);
    for f in 0..300 {
        m.update(
            from.lerp(to, f as f32 / 299.0),
            &mut w,
            Duration::from_secs_f32(0.002),
        );
        for (i, &p) in probes.iter().enumerate() {
            let now = drawn_lod(&m.render_set, p);
            if now != prev[i] {
                flips += 1;
                losses += usize::from(now.is_none() && prev[i].is_some());
            }
            prev[i] = now;
        }
        std::thread::sleep(FRAME);
    }
    (flips, losses)
}

#[test]
fn transitions_are_symmetric_and_never_uncover() {
    let gen = WorldGen::new(1337);
    let probes = probe_grid(&gen);
    for cross_fade in [false, true] {
        let cfg = LodConfig {
            cross_fade,
            ..Default::default()
        };
        let (fwd, fwd_lost) = flight(cfg, A, B, &probes);
        let (back, back_lost) = flight(cfg, B, A, &probes);
        let ratio = back as f32 / fwd.max(1) as f32;
        println!(
            "  fade {cross_fade}: approach {fwd} flips / {fwd_lost} uncovered, \
             retreat {back} / {back_lost}, asymmetry {ratio:.2}x"
        );

        assert!(
            ratio < 4.0,
            "retreat pops {ratio:.2}x as much as approach ({back} vs {fwd})"
        );
    }
}

#[test]
fn boundary_flutter_is_damped_without_the_fade() {
    let gen = WorldGen::new(1337);
    let probes = probe_grid(&gen);
    let base = Vec3::new(-450.0, 140.0, 556.0);
    let (mut m, mut w) = mgr(LodConfig {
        cross_fade: false,
        ..Default::default()
    });
    settle(&mut m, &mut w, base);
    let mut prev: Vec<Option<u8>> = probes
        .iter()
        .map(|&p| drawn_lod(&m.render_set, p))
        .collect();
    let mut flips = 0;
    for f in 0..300 {
        let cam = base + Vec3::new(0.0, 0.0, (f as f32 * 0.12).sin() * 24.0);
        m.update(cam, &mut w, Duration::from_secs_f32(0.002));
        for (i, &p) in probes.iter().enumerate() {
            let now = drawn_lod(&m.render_set, p);
            flips += usize::from(now != prev[i]);
            prev[i] = now;
        }
        std::thread::sleep(FRAME);
    }
    assert!(flips < 20, "{flips} lod flips while hovering on a boundary");
}

#[test]
fn an_edit_rebuilds_the_coarse_stand_ins_over_it_and_then_stops() {
    use voxelcraft::block::{AIR, GLOWSTONE};
    use voxelcraft::voxel::{dense_index, ChunkKey, VOL};

    let cfg = LodConfig::default();
    let gen = Arc::new(WorldGen::new(1337));
    let journal = voxelcraft::journal::EditJournal::empty();
    let mut m = ChunkManager::new(cfg, gen);
    m.attach_journal(journal.clone());
    let mut w = World::new();
    w.attach_journal(journal);

    let cam = Vec3::new(-450.0, 140.0, 512.0);
    settle(&mut m, &mut w, cam);

    let mut dense = vec![0u16; VOL];
    let coarse: Vec<ChunkKey> = w.chunks.keys().copied().filter(|k| k.lod == 1).collect();
    let found = coarse.iter().find_map(|&k| {
        w.extract_dense(k, &mut dense).then_some(())?;
        let parent = dense.clone();
        (0..8).find_map(|i| {
            let c = k.child(i & 1, (i >> 1) & 1, (i >> 2) & 1);
            let p = c.origin() + glam::IVec3::new(32, 63, 32);
            let v = (p - k.origin()) / k.voxel_size();
            let pi = dense_index(v.x as usize, v.y as usize, v.z as usize);
            let air = w.contains(&c) && w.get_block(p) == AIR && parent[pi] == AIR;
            air.then_some((k, p, pi))
        })
    });
    let (coarse_key, p, pi) =
        found.expect("no resident coarse stand-in with open air under it -- fixture is blind");

    assert!(w.set_block(p, GLOWSTONE));
    settle(&mut m, &mut w, cam);
    assert!(w.extract_dense(coarse_key, &mut dense), "still resident");
    assert_eq!(
        dense[pi], GLOWSTONE,
        "{coarse_key:?} is still showing the world without the edit at {p}"
    );

    let mut interned = 0;
    for _ in 0..400 {
        m.update(cam, &mut w, Duration::from_secs_f32(0.002));
        interned += m.last_drained;
        std::thread::sleep(FRAME);
    }
    assert_eq!(interned, 0, "the dirty set never drained: {interned} rebuilds");
}

#[test]
fn a_settled_plan_survives_further_updates() {
    let (mut m, mut w) = mgr(LodConfig::default());
    let cam = Vec3::new(8.5, 210.0, 8.5);
    settle(&mut m, &mut w, cam);

    let selected = m.selected.clone();
    let plan: Vec<(u8, i32, i32, i32, u32)> = m
        .render_set
        .iter()
        .map(|it| {
            (
                it.key.lod,
                it.key.pos.x,
                it.key.pos.y,
                it.key.pos.z,
                it.fade.to_bits(),
            )
        })
        .collect();
    assert!(!plan.is_empty(), "nothing to draw -- the fixture is blind");

    for i in 1..=600 {
        m.update(cam, &mut w, Duration::from_millis(50));
        assert_eq!(m.selected.len(), selected.len(), "selection moved at +{i}");
        let now: Vec<(u8, i32, i32, i32, u32)> = m
            .render_set
            .iter()
            .map(|it| {
                (
                    it.key.lod,
                    it.key.pos.x,
                    it.key.pos.y,
                    it.key.pos.z,
                    it.fade.to_bits(),
                )
            })
            .collect();
        assert_eq!(now, plan, "the draw plan moved at +{i} with a still camera");
        for it in &m.render_set {
            assert!(
                w.contains(&it.key),
                "the retention sweep took {:?}, which the plan draws, at +{i}",
                it.key
            );
        }
    }
}

#[test]
fn re_settling_at_a_moved_camera_gives_that_camera_s_plan() {
    let key = |it: &RenderItem| {
        (
            it.key.lod,
            it.key.pos.x,
            it.key.pos.y,
            it.key.pos.z,
            it.fade.to_bits(),
        )
    };

    let spawn = Vec3::new(8.5, 210.0, 8.5);
    let moved = Vec3::new(6.5, 188.0, 6.5);

    let (mut m, mut w) = mgr(LodConfig::default());
    settle(&mut m, &mut w, spawn);
    let stale: Vec<_> = m.render_set.iter().map(key).collect();
    settle(&mut m, &mut w, moved);
    let replanned: Vec<_> = m.render_set.iter().map(key).collect();

    let (mut m2, mut w2) = mgr(LodConfig::default());
    settle(&mut m2, &mut w2, moved);
    let from_scratch: Vec<_> = m2.render_set.iter().map(key).collect();

    assert_ne!(
        stale, replanned,
        "the eye moved 22 blocks and the plan did not -- this test can no longer see the defect"
    );
    let mut a = replanned;
    let mut b = from_scratch;
    a.sort_unstable();
    b.sort_unstable();
    assert_eq!(
        a, b,
        "re-settling did not reach the plan a world streamed at that camera has"
    );
}
