//! What may cast a shadow must not depend on where the player is looking.
//!
//! Roadmap D3, batch 64. Until that batch the renderer had one chunk list: `build_chunk_list`
//! frustum-culled `render_set` and the surviving entries were both what `tile_select`
//! box-tested *and* what filled `grid`. `grid` is what every shadow ray and every cross-chunk
//! light lookup reads, so a chunk the frustum rejected occluded nothing -- turn until a
//! mountain leaves the frustum and its shadow leaves the ground it was falling on, which is
//! ground still on screen. The user found it by playing and described it as a shadow that
//! *"becomes shorter or larger like a weird pop in pop out"*.
//!
//! **This file exists because that claim cannot be a capture.** Every vantage is a single
//! still from one camera and this defect is a *disagreement between two cameras*: each frame
//! is internally consistent, and there is nothing inside either one to compare against. The
//! `offscreen-shadow` vantage records that the fix moves 28,128 pixels; only the equality
//! below says what it was supposed to move them *to*.
//!
//! It runs against a real streamed world and needs no device, because `partition_for_grid`
//! takes boxes rather than a `World`.

use glam::{IVec3, Vec3};
use std::sync::Arc;
use std::time::Duration;
use voxelcraft::lod::{LodConfig, RenderItem};
use voxelcraft::math::{Aabb, Frustum};
use voxelcraft::render::{grid_origin, grid_span, partition_for_grid, GRID_X, GRID_Y, GRID_Z};
use voxelcraft::stream::ChunkManager;
use voxelcraft::voxel::{ChunkKey, World};
use voxelcraft::worldgen::WorldGen;

/// The `offscreen-shadow` vantage's camera: 80 blocks up over the spawn column, which is where
/// the defect is worth 28,128 pixels of 921,600. The yaw is what this file sweeps.
const EYE: Vec3 = Vec3::new(0.0, 80.0, 0.0);
const FOV_DEG: f32 = 70.0;
const ASPECT: f32 = 1280.0 / 720.0;

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

/// `app.rs`'s camera basis, transcribed. A deliberate transliteration: what this file asserts
/// is that the *frustum* does not reach the grid, so it has to build the same frustum the
/// renderer does or it is asserting something about a different shape.
fn frustum_at(pos: Vec3, yaw_deg: f32, pitch_deg: f32, view_distance: f32) -> Frustum {
    let (y, p) = (yaw_deg.to_radians(), pitch_deg.to_radians());
    let fwd = Vec3::new(y.cos() * p.cos(), p.sin(), y.sin() * p.cos()).normalize();
    let right = fwd.cross(Vec3::Y).normalize();
    let up = right.cross(fwd).normalize();
    Frustum::from_camera(
        pos,
        fwd,
        right,
        up,
        (FOV_DEG.to_radians() * 0.5).tan(),
        ASPECT,
        view_distance,
    )
}

/// `build_chunk_list`'s own gather: resident, non-empty, and its world box.
fn plan_of(m: &ChunkManager, w: &World) -> Vec<(RenderItem, Aabb)> {
    m.render_set
        .iter()
        .filter_map(|&item| {
            let rec = w.chunks.get(&item.key)?;
            (rec.solid_count != 0).then(|| (item, rec.world_aabb()))
        })
        .collect()
}

fn keys(items: &[RenderItem]) -> Vec<ChunkKey> {
    let mut v: Vec<ChunkKey> = items.iter().map(|it| it.key).collect();
    v.sort_by_key(|k| (k.lod, k.pos.x, k.pos.y, k.pos.z));
    v
}

/// The chunks that actually write a cell, which is the set the invariant is about.
///
/// **Not the whole partition, and the first draft of this file got that wrong in a way worth
/// keeping.** The drawn list is *not* filtered against the lattice -- it does not need to be,
/// since `tile_select` reads it out of `chunks` by index and never through `grid` -- so at any
/// yaw it carries some chunks past the lattice edge that the fill loop then skips. Asserting
/// on the raw union therefore fails at 337 keys against 340, which reads like the feature
/// leaking and is the frustum showing through a set it has every right to decide. What has to
/// be yaw-independent is what a secondary ray can *reach*.
fn in_lattice(items: &[RenderItem], gmin: IVec3) -> Vec<ChunkKey> {
    let mut v: Vec<ChunkKey> = items
        .iter()
        .filter(|it| grid_span(it.key, gmin).is_some())
        .map(|it| it.key)
        .collect();
    v.sort_by_key(|k| (k.lod, k.pos.x, k.pos.y, k.pos.z));
    v
}

fn world_at(eye: Vec3) -> (ChunkManager, World, LodConfig) {
    let cfg = LodConfig::default();
    let gen = Arc::new(WorldGen::new(1337));
    let mut m = ChunkManager::new(cfg, gen);
    let mut w = World::new();
    settle(&mut m, &mut w, eye);
    (m, w, cfg)
}

/// **The batch's claim, as an equality.** Eight yaws from one position: the set of chunks that
/// may occlude is byte-for-byte the same at every one of them.
///
/// The partition returns two lists and neither of them is asserted on its own, because which
/// list a chunk lands in is exactly what the yaw decides -- and has to, since the first list is
/// what `tile_select` box-tests and box-testing the world behind the camera is what the frustum
/// cull is for. What must not depend on the yaw is what a *secondary ray* can reach, which is
/// their union through `in_lattice`.
#[test]
fn the_grid_set_is_the_same_at_every_yaw() {
    let (m, w, cfg) = world_at(EYE);
    let plan = plan_of(&m, &w);
    let gmin = grid_origin(EYE);

    let mut reference: Option<Vec<ChunkKey>> = None;
    for yaw in [0.0, 45.0, 90.0, 135.0, 180.0, 225.0, 265.0, 315.0] {
        let f = frustum_at(EYE, yaw, -14.0, cfg.view_distance);
        let (vis, off) = partition_for_grid(&plan, EYE, &f, gmin, true);
        let mut all = vis;
        all.extend_from_slice(&off);
        let got = in_lattice(&all, gmin);
        match &reference {
            None => reference = Some(got),
            Some(want) => assert_eq!(
                &got, want,
                "the grid-eligible set changed at yaw {yaw}: {} keys against {}",
                got.len(),
                want.len()
            ),
        }
    }
}

/// The other half, and without it the test above passes on an empty world or on a partition
/// that lost the frustum entirely. The *drawn* set must still depend on the yaw -- that is the
/// cull doing its job -- and the gap between the two sets is what batch 64 added.
#[test]
fn the_drawn_set_still_depends_on_the_yaw() {
    let (m, w, cfg) = world_at(EYE);
    let plan = plan_of(&m, &w);
    let gmin = grid_origin(EYE);

    let mut seen: Vec<Vec<ChunkKey>> = Vec::new();
    let mut tails = Vec::new();
    for yaw in [0.0, 90.0, 180.0, 265.0] {
        let f = frustum_at(EYE, yaw, -14.0, cfg.view_distance);
        let (vis, off) = partition_for_grid(&plan, EYE, &f, gmin, true);
        assert!(!vis.is_empty(), "nothing drawn at yaw {yaw}");
        assert!(
            !off.is_empty(),
            "nothing shadow-only at yaw {yaw}: the tail is what this batch added"
        );
        tails.push(off.len());
        seen.push(keys(&vis));
    }
    assert!(
        seen.windows(2).any(|p| p[0] != p[1]),
        "the drawn set is yaw-independent, so the frustum cull is not running and the test \
         above proves nothing"
    );
    println!("shadow-only tail per yaw: {tails:?}");
}

/// With the control clear the tail is empty, so `chunk_scratch` holds the same entries in the
/// same order as it did before batch 64 and the grid fill runs over the same indices.
///
/// **This is the control's whole purity argument and it is structural rather than measured.**
/// A `bitexact` row can say the two builds agree at nineteen cameras; only this says why they
/// must.
#[test]
fn the_control_leaves_exactly_the_frustum_culled_set() {
    let (m, w, cfg) = world_at(EYE);
    let plan = plan_of(&m, &w);
    let gmin = grid_origin(EYE);
    let f = frustum_at(EYE, 265.0, -14.0, cfg.view_distance);

    let (on_vis, on_off) = partition_for_grid(&plan, EYE, &f, gmin, true);
    let (off_vis, off_off) = partition_for_grid(&plan, EYE, &f, gmin, false);
    assert!(off_off.is_empty(), "the control left a shadow-only tail");
    assert!(!on_off.is_empty(), "the feature added no shadow-only tail");
    assert_eq!(
        on_vis, off_vis,
        "the control changed the drawn list, so it is not a pure revert"
    );
}

/// Every chunk in the tail writes at least one cell, and the filter that admits it is the same
/// function the fill loop walks.
///
/// A chunk admitted but writing nothing is only a wasted upload; a chunk *rejected* that would
/// have written is an occluder silently missing from the grid -- this batch's own defect one
/// level down, and invisible in exactly the same way. One function is what rules that out, and
/// this is the assertion that the one function is the one both readers use.
#[test]
fn every_shadow_only_chunk_reaches_the_lattice() {
    let (m, w, cfg) = world_at(EYE);
    let plan = plan_of(&m, &w);
    let gmin = grid_origin(EYE);
    let f = frustum_at(EYE, 265.0, -14.0, cfg.view_distance);
    let (_, off) = partition_for_grid(&plan, EYE, &f, gmin, true);

    let lattice = IVec3::new(GRID_X as i32, GRID_Y as i32, GRID_Z as i32);
    for it in &off {
        let span = grid_span(it.key, gmin)
            .unwrap_or_else(|| panic!("{:?} is in the tail and covers no grid cell", it.key));
        assert!(span.0.cmplt(span.1).all());
        assert!(span.0.cmpge(IVec3::ZERO).all() && span.1.cmple(lattice).all());
    }
}



