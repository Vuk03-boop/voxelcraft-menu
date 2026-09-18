use glam::{IVec3, Vec3};
use voxelcraft::block::*;
use voxelcraft::player::{Input, Player, EYE, HEIGHT};
use voxelcraft::voxel::*;
use winit::keyboard::KeyCode;

const FLOOR: usize = 32;

fn flat_world() -> World {
    let mut dense = vec![AIR; VOL];
    for y in 0..FLOOR {
        for z in 0..64 {
            for x in 0..64 {
                dense[dense_index(x, y, z)] = STONE;
            }
        }
    }
    let mut w = World::new();
    w.insert_local(ChunkKey::new(0, IVec3::ZERO), build_local(&dense));
    w
}

fn flooded_world(depth: usize) -> World {
    let mut dense = vec![AIR; VOL];
    for z in 0..64 {
        for x in 0..64 {
            for y in 0..FLOOR {
                dense[dense_index(x, y, z)] = STONE;
            }
            for y in FLOOR..FLOOR + depth {
                dense[dense_index(x, y, z)] = WATER;
            }
        }
    }
    let mut w = World::new();
    w.insert_local(ChunkKey::new(0, IVec3::ZERO), build_local(&dense));
    w
}

fn walk(p: &mut Player, w: &World, input: &Input, seconds: f32) {
    let dt = 1.0 / 60.0;
    for _ in 0..(seconds / dt) as i32 {
        p.update(dt, input, w, true);
    }
}

fn grounded_player(pos: Vec3) -> Player {
    let mut p = Player::new(pos);
    p.fly = false;
    p
}

#[test]
fn falls_and_lands_on_the_surface() {
    let w = flat_world();
    let mut p = grounded_player(Vec3::new(32.5, 50.0, 32.5));
    walk(&mut p, &w, &Input::default(), 3.0);
    assert!(p.on_ground, "player should be standing after falling");
    assert!(
        (p.pos.y - FLOOR as f32).abs() < 0.05,
        "expected to land on y={FLOOR}, got {}",
        p.pos.y
    );

    walk(&mut p, &w, &Input::default(), 2.0);
    assert!(
        (p.pos.y - FLOOR as f32).abs() < 0.05,
        "sank to {} after resting",
        p.pos.y
    );
}

#[test]
fn does_not_fall_through_at_terminal_velocity() {
    let w = flat_world();
    let mut p = grounded_player(Vec3::new(32.5, 63.0, 32.5));
    p.vel.y = -400.0;
    walk(&mut p, &w, &Input::default(), 2.0);
    assert!(
        p.pos.y >= FLOOR as f32 - 0.05,
        "tunnelled through the floor to {}",
        p.pos.y
    );
}

#[test]
fn walls_block_horizontal_movement() {
    let mut w = flat_world();

    for y in FLOOR..FLOOR + 4 {
        for z in 0..64 {
            w.set_block(IVec3::new(40, y as i32, z), COBBLE);
        }
    }
    let mut p = grounded_player(Vec3::new(32.5, FLOOR as f32, 32.5));
    let mut input = Input::default();
    input.down.insert(KeyCode::KeyW);
    p.yaw = 0.0;
    walk(&mut p, &w, &input, 4.0);
    assert!(p.pos.x < 40.0, "walked into the wall, x = {}", p.pos.x);
    assert!(
        p.pos.x > 39.0,
        "stopped far short of the wall, x = {}",
        p.pos.x
    );
}

#[test]
fn jump_clears_the_ground_and_returns() {
    let w = flat_world();
    let mut p = grounded_player(Vec3::new(32.5, FLOOR as f32, 32.5));
    walk(&mut p, &w, &Input::default(), 0.5);
    assert!(p.on_ground);

    let mut jump = Input::default();
    jump.down.insert(KeyCode::Space);
    let dt = 1.0 / 60.0;
    p.update(dt, &jump, &w, true);
    let mut peak: f32 = p.pos.y;
    for _ in 0..90 {
        p.update(dt, &Input::default(), &w, true);
        peak = peak.max(p.pos.y);
    }
    assert!(peak > FLOOR as f32 + 1.0, "jump only reached {peak}");
    assert!(
        peak < FLOOR as f32 + 2.5,
        "jump reached {peak}, far too high"
    );
    assert!(p.on_ground, "should be back on the ground");
}

#[test]
fn ceilings_stop_upward_movement() {
    let mut w = flat_world();
    let ceiling = FLOOR + 4;
    for z in 0..64 {
        for x in 0..64 {
            w.set_block(IVec3::new(x, ceiling as i32, z), COBBLE);
        }
    }
    let mut p = Player::new(Vec3::new(32.5, FLOOR as f32, 32.5));
    p.fly = true;
    let mut up = Input::default();
    up.down.insert(KeyCode::Space);
    walk(&mut p, &w, &up, 3.0);

    assert!(
        p.pos.y + HEIGHT <= ceiling as f32 + 0.01,
        "head at {} passed the ceiling at {ceiling}",
        p.pos.y + HEIGHT
    );
}

#[test]
fn flying_fast_into_terrain_does_not_tunnel() {
    let mut w = flat_world();
    for y in FLOOR..FLOOR + 6 {
        for z in 0..64 {
            w.set_block(IVec3::new(50, y as i32, z), COBBLE);
        }
    }
    let mut p = Player::new(Vec3::new(2.5, FLOOR as f32 + 1.0, 32.5));
    p.fly = true;
    p.yaw = 0.0;
    let mut input = Input::default();
    input.down.insert(KeyCode::KeyW);
    input.down.insert(KeyCode::ControlLeft);
    walk(&mut p, &w, &input, 3.0);
    assert!(
        p.pos.x < 50.0,
        "fast flight tunnelled through the wall to x = {}",
        p.pos.x
    );
}

#[test]
fn water_is_not_solid() {

    let w = flooded_world(8);
    let mut p = Player::new(Vec3::new(32.5, FLOOR as f32 + 3.0, 32.5));
    p.pitch = -std::f32::consts::FRAC_PI_2 * 0.999;
    let pick = p
        .pick(&w)
        .expect("looking down through water should reach the floor");
    assert_eq!(
        pick.block,
        IVec3::new(32, FLOOR as i32 - 1, 32),
        "pick stopped in the water"
    );
    assert!(
        p.in_water(&w),
        "standing inside the pool should read as in water"
    );
}

#[test]
fn swimming_sinks_slowly_and_rises_on_demand() {
    let w = flooded_world(10);
    let surface = (FLOOR + 10) as f32;

    let mut p = grounded_player(Vec3::new(32.5, surface + 6.0, 32.5));
    walk(&mut p, &w, &Input::default(), 1.5);
    assert!(
        p.pos.y < surface,
        "should have entered the water, y = {}",
        p.pos.y
    );
    assert!(
        p.vel.y > -4.0,
        "still falling at {} m/s inside the water",
        p.vel.y
    );

    let before = p.pos.y;
    walk(&mut p, &w, &Input::default(), 12.0);
    assert!(p.pos.y < before, "did not sink at all");
    assert!(
        (p.pos.y - FLOOR as f32).abs() < 0.05,
        "should be resting on the bottom, y = {}",
        p.pos.y
    );

    let mut up = Input::default();
    up.down.insert(KeyCode::Space);
    walk(&mut p, &w, &up, 8.0);
    assert!(
        p.pos.y > surface - 2.0,
        "swimming up only reached {}",
        p.pos.y
    );
    assert!(
        p.pos.y < surface + 1.0,
        "swimming up launched to {}",
        p.pos.y
    );
}

#[test]
fn block_pick_finds_the_face_you_look_at() {
    let w = flat_world();
    let mut p = Player::new(Vec3::new(32.5, FLOOR as f32 + 2.0, 32.5));
    p.pitch = -std::f32::consts::FRAC_PI_2 * 0.999;
    let pick = p.pick(&w).expect("looking down at the floor should hit");
    assert_eq!(pick.block, IVec3::new(32, FLOOR as i32 - 1, 32));
    assert_eq!(pick.normal, IVec3::Y, "top face of the block below");
    assert_eq!(pick.adjacent, IVec3::new(32, FLOOR as i32, 32));
    assert!(
        pick.distance <= 2.0 + EYE + 1e-3,
        "distance {} is implausible",
        pick.distance
    );
}
