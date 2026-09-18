//! The guard for batch 70, and the reason the rule is in `src/idle.rs` rather than in
//! `app.rs`: the window path is the one module no capture drives, so the decision of
//! when a presented frame may be a repaint has to be reachable from here.
//!
//! The claim under test is the batch's only one: **a repaint is presented only when a
//! fresh render could not have drawn anything new** -- the token matches, the sun has
//! not crossed its tick, and the animation cadence has room.

use glam::{IVec3, Vec3};
use voxelcraft::block::*;
use voxelcraft::idle::{sun_tick, FrameKind, IdleRepaint, StaticFrame, LIVE_ANIM_EVERY, SUN_TICK_DEG};
use voxelcraft::player::{Input, Player};
use voxelcraft::voxel::*;

fn token() -> StaticFrame {
    StaticFrame {
        cam: [100.5, 64.0, -32.5],
        yaw: 1.25,
        pitch: -0.07,
        world: 3,
        probes_pending: false,
        relight_pending: false,
        flags: 0xDEAD,
        spec_hi: 0xBEEF,
        taa: true,
    }
}

#[test]
fn the_first_frame_renders() {
    let mut idle = IdleRepaint::new();
    assert_eq!(idle.classify(token(), false, 0), FrameKind::Render);
}

#[test]
fn a_still_world_repaints() {
    let mut idle = IdleRepaint::new();
    assert_eq!(idle.classify(token(), false, 0), FrameKind::Render);
    for _ in 0..64 {
        assert_eq!(idle.classify(token(), false, 0), FrameKind::Repaint);
    }
}

#[test]
fn every_token_field_forces_a_render() {
    for i in 0..9 {
        let mut idle = IdleRepaint::new();
        assert_eq!(idle.classify(token(), false, 0), FrameKind::Render);
        assert_eq!(idle.classify(token(), false, 0), FrameKind::Repaint);
        let mut moved = token();
        match i {
            0 => moved.cam[1] += 0.002,
            1 => moved.yaw -= 0.001,
            2 => moved.pitch += 0.001,
            3 => moved.world += 1,
            4 => moved.probes_pending = true,
            5 => moved.relight_pending = true,
            6 => moved.flags ^= 1,
            7 => moved.spec_hi ^= 1,
            _ => moved.taa = false,
        }
        assert_eq!(
            idle.classify(moved, false, 0),
            FrameKind::Render,
            "field {i} changed under a still world and no render followed"
        );
        // And the cadence restarts: the new token is the one repaints compare against.
        assert_eq!(idle.classify(moved, false, 0), FrameKind::Repaint);
    }
}

#[test]
fn the_sun_forces_a_render_on_its_tick() {
    let mut idle = IdleRepaint::new();
    assert_eq!(idle.classify(token(), false, 41), FrameKind::Render);
    assert_eq!(idle.classify(token(), false, 41), FrameKind::Repaint);
    assert_eq!(idle.classify(token(), false, 42), FrameKind::Render);
}

#[test]
fn a_live_field_caps_the_cadence() {
    let mut idle = IdleRepaint::new();
    let mut pattern = Vec::new();
    for _ in 0..8 {
        pattern.push(idle.classify(token(), true, 0));
    }
    // Render, then at most LIVE_ANIM_EVERY - 1 repaints before the next render.
    let mut expected = Vec::new();
    for _ in 0..4 {
        expected.push(FrameKind::Render);
        for _ in 1..LIVE_ANIM_EVERY {
            expected.push(FrameKind::Repaint);
        }
    }
    assert_eq!(pattern, expected);
}

#[test]
fn the_cadence_yields_to_a_changed_token() {
    let mut idle = IdleRepaint::new();
    assert_eq!(idle.classify(token(), true, 0), FrameKind::Render);
    // Halfway through the cadence the world version moves: render now, cadence restarts.
    let mut moved = token();
    moved.world += 1;
    assert_eq!(idle.classify(moved, true, 0), FrameKind::Render);
    assert_eq!(idle.classify(moved, true, 0), FrameKind::Repaint);
}

#[test]
fn invalidation_forces_a_render() {
    let mut idle = IdleRepaint::new();
    assert_eq!(idle.classify(token(), false, 0), FrameKind::Render);
    assert_eq!(idle.classify(token(), false, 0), FrameKind::Repaint);
    idle.invalidate();
    assert_eq!(idle.classify(token(), false, 0), FrameKind::Render);
}

#[test]
fn sun_tick_bins_floor_and_wrap() {
    assert_eq!(sun_tick(0.0), 0);
    // Just inside tick 41, and just past it -- the boundary is the tick.
    let tick = SUN_TICK_DEG / 360.0;
    assert_eq!(sun_tick(41.0 * tick + tick * 0.49), 41);
    assert_eq!(sun_tick(41.0 * tick + tick * 0.51), 41);
    assert_eq!(sun_tick(42.0 * tick), 42);
    // The day wraps: the last tick sits next to tick 0, and a negative or >1 input
    // still lands in a real bin rather than panicking the cast.
    let last = sun_tick(1.0 - tick * 0.5);
    assert!(last > 0);
    assert_eq!(sun_tick(1.5), sun_tick(0.5));
    assert_eq!(sun_tick(-tick), last);
}

#[test]
fn the_tick_is_sub_pixel_by_construction() {
    // The envelope's longest shadow run is ~600 blocks; a tick moves its edge by the
    // run times the tangent of the tick. Under half a block is sub-pixel at any camera
    // that can see a run that long, so the repaint creep the tick permits cannot show.
    let creep = 600.0 * (SUN_TICK_DEG as f64).to_radians().tan();
    assert!(
        creep < 0.5,
        "SUN_TICK_DEG moved the longest shadow edge by {creep} blocks"
    );
}

/// The premise the whole batch stands on, proven against the real physics rather than
/// assumed: the token compares the camera **bit-exactly**, which is honest only if a
/// stationary player is bit-stable. Ground velocity is written from the wish direction
/// rather than integrated with friction, and the floor snap is an exact constant, so a
/// zero-input frame reproduces the previous one to the bit. If a physics batch ever
/// changes that, this guard fails before the window loop repaints a moving scene.
#[test]
fn a_still_player_yields_a_bit_stable_camera() {
    const FLOOR: usize = 32;
    for water in [0usize, 1] {
        let mut dense = vec![AIR; VOL];
        for z in 0..64 {
            for x in 0..64 {
                for y in 0..FLOOR {
                    dense[dense_index(x, y, z)] = STONE;
                }
                for y in FLOOR..FLOOR + water {
                    dense[dense_index(x, y, z)] = WATER;
                }
            }
        }
        let mut w = World::new();
        w.insert_local(ChunkKey::new(0, IVec3::ZERO), build_local(&dense));

        let mut p = Player::new(Vec3::new(32.5, FLOOR as f32 + 8.0, 32.5));
        p.fly = false;
        let input = Input::default();
        let dt = 1.0 / 60.0;
        for _ in 0..600 {
            p.update(dt, &input, &w, true);
        }
        // Settled by ten seconds. The next four hundred frames may not move a single bit:
        // what `classify` compares is what the physics produced, epsilons never handmade.
        let eye = p.eye();
        let bits = p.eye().to_array().map(f32::to_bits);
        for _ in 0..400 {
            p.update(dt, &input, &w, true);
            assert_eq!(
                p.eye().to_array().map(f32::to_bits),
                bits,
                "settled player drifted by an f32 with water={water}"
            );
        }
        assert_eq!(p.eye(), eye);
    }
}

