use crate::block::{Bar, BlockId, HOTBAR, SLOTS, WATER};
use crate::journal::PlayerState;
use crate::math::{block_of, Aabb};
use crate::voxel::World;
use glam::{IVec3, Vec3};
use winit::keyboard::KeyCode;

pub const WIDTH: f32 = 0.6;
pub const HEIGHT: f32 = 1.8;
pub const EYE: f32 = 1.62;
const WALK_SPEED: f32 = 4.3;
const SPRINT_SPEED: f32 = 6.5;
const FLY_SPEED: f32 = 24.0;
const FLY_FAST: f32 = 90.0;
const JUMP_SPEED: f32 = 8.4;
const GRAVITY: f32 = 28.0;
const REACH: f32 = 6.0;

const SWIM_SPEED: f32 = 3.2;
const SWIM_UP: f32 = 3.0;
const SWIM_DOWN: f32 = 3.0;
const SINK_SPEED: f32 = 1.2;
const WATER_DRAG: f32 = 5.0;

#[derive(Default)]
pub struct Input {
    pub down: rustc_hash::FxHashSet<KeyCode>,
    pub pressed: rustc_hash::FxHashSet<KeyCode>,
    pub mouse_dx: f32,
    pub mouse_dy: f32,
    pub scroll: f32,
    pub lmb: bool,
    pub rmb: bool,
    pub lmb_down: bool,
    pub rmb_down: bool,

    pub mmb: bool,
    pub mmb_down: bool,
}

impl Input {
    pub fn is_down(&self, k: KeyCode) -> bool {
        self.down.contains(&k)
    }
    pub fn was_pressed(&self, k: KeyCode) -> bool {
        self.pressed.contains(&k)
    }
    pub fn end_frame(&mut self) {
        self.pressed.clear();
        self.mouse_dx = 0.0;
        self.mouse_dy = 0.0;
        self.scroll = 0.0;
        self.lmb = false;
        self.rmb = false;
        self.mmb = false;
    }
}

pub struct Player {

    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub fly: bool,
    pub on_ground: bool,
    pub hotbar: usize,

    pub bar: Bar,
    pub sensitivity: f32,
    pub place_cooldown: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Pick {
    pub block: IVec3,

    pub adjacent: IVec3,
    pub normal: IVec3,
    pub distance: f32,
}

impl Player {
    pub fn new(pos: Vec3) -> Self {
        Self {
            pos,
            vel: Vec3::ZERO,
            yaw: 0.0,
            pitch: -0.2,
            fly: true,
            on_ground: false,
            hotbar: 0,
            bar: HOTBAR,
            sensitivity: 0.0022,
            place_cooldown: 0.0,
        }
    }

    pub fn eye(&self) -> Vec3 {
        self.pos + Vec3::new(0.0, EYE, 0.0)
    }

    pub fn state(&self) -> PlayerState {
        PlayerState::new(
            self.pos.into(),
            self.yaw,
            self.pitch,
            self.hotbar as u32,
            self.fly,
        )
    }

    pub fn restore(&mut self, s: PlayerState) {
        self.pos = Vec3::from(s.pos);
        self.vel = Vec3::ZERO;
        self.yaw = s.yaw;
        self.pitch = s.pitch;
        self.hotbar = s.hotbar as usize;
        self.fly = s.fly != 0;
    }

    pub fn look_dir(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(cy * cp, sp, sy * cp).normalize()
    }

    pub fn forward_flat(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        Vec3::new(cy, 0.0, sy)
    }

    pub fn right(&self) -> Vec3 {
        self.forward_flat().cross(Vec3::Y)
    }

    pub fn aabb(&self) -> Aabb {
        Aabb::new(
            self.pos - Vec3::new(WIDTH * 0.5, 0.0, WIDTH * 0.5),
            self.pos + Vec3::new(WIDTH * 0.5, HEIGHT, WIDTH * 0.5),
        )
    }

    pub fn selected_block(&self) -> u16 {
        self.bar[self.hotbar]
    }

    pub fn pick_into_bar(&mut self, id: BlockId) -> bool {
        if crate::block::bar_slot_refusal(id).is_some() {
            return false;
        }
        self.bar[self.hotbar] = id;
        true
    }

    pub fn in_water(&self, world: &World) -> bool {
        world.get_block(block_of(self.pos + Vec3::new(0.0, HEIGHT * 0.5, 0.0))) == WATER
    }

    pub fn eye_in_water(&self, world: &World) -> bool {
        world.get_block(block_of(self.eye())) == WATER
    }

    pub fn update(&mut self, dt: f32, input: &Input, world: &World, physics: bool) {
        self.yaw += input.mouse_dx * self.sensitivity;
        self.pitch = (self.pitch - input.mouse_dy * self.sensitivity).clamp(-1.55, 1.55);
        if input.was_pressed(KeyCode::KeyF) {
            self.fly = !self.fly;
            self.vel = Vec3::ZERO;
        }
        if input.scroll.abs() > 0.0 {
            let n = SLOTS as i32;
            let step = if input.scroll > 0.0 { -1 } else { 1 };
            self.hotbar = ((self.hotbar as i32 + step).rem_euclid(n)) as usize;
        }
        for (i, k) in [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::Digit5,
            KeyCode::Digit6,
            KeyCode::Digit7,
            KeyCode::Digit8,
            KeyCode::Digit9,
            KeyCode::Digit0,
        ]
        .iter()
        .enumerate()
        {
            if input.was_pressed(*k) {
                self.hotbar = i;
            }
        }

        let mut wish = Vec3::ZERO;
        if input.is_down(KeyCode::KeyW) {
            wish += self.forward_flat();
        }
        if input.is_down(KeyCode::KeyS) {
            wish -= self.forward_flat();
        }
        if input.is_down(KeyCode::KeyD) {
            wish += self.right();
        }
        if input.is_down(KeyCode::KeyA) {
            wish -= self.right();
        }
        let wish = wish.normalize_or_zero();
        let fast = input.is_down(KeyCode::ControlLeft);

        if self.fly {
            let speed = if fast { FLY_FAST } else { FLY_SPEED };
            let mut v = wish * speed;
            if input.is_down(KeyCode::Space) {
                v.y += speed;
            }
            if input.is_down(KeyCode::ShiftLeft) {
                v.y -= speed;
            }
            self.vel = v;
            let d = self.vel * dt;
            if physics {
                self.move_collide(world, d);
            } else {
                self.pos += d;
            }
        } else if physics && self.in_water(world) {

            let target = if input.is_down(KeyCode::Space) {
                SWIM_UP
            } else if input.is_down(KeyCode::ShiftLeft) {
                -SWIM_DOWN
            } else {
                -SINK_SPEED
            };
            self.vel.x = wish.x * SWIM_SPEED;
            self.vel.z = wish.z * SWIM_SPEED;
            self.vel.y += (target - self.vel.y) * (1.0 - (-WATER_DRAG * dt).exp());
            self.move_collide(world, self.vel * dt);
        } else {
            let speed = if fast { SPRINT_SPEED } else { WALK_SPEED };
            self.vel.x = wish.x * speed;
            self.vel.z = wish.z * speed;
            if physics {
                self.vel.y -= GRAVITY * dt;
                self.vel.y = self.vel.y.max(-60.0);
                if self.on_ground && input.is_down(KeyCode::Space) {
                    self.vel.y = JUMP_SPEED;
                    self.on_ground = false;
                }
                self.move_collide(world, self.vel * dt);
            } else {
                self.vel.y = 0.0;
            }
        }
        self.place_cooldown = (self.place_cooldown - dt).max(0.0);
    }

    fn move_collide(&mut self, world: &World, d: Vec3) {

        let steps = ((d.abs().max_element() / 0.4).ceil() as i32).clamp(1, 32);
        let sd = d / steps as f32;
        for _ in 0..steps {
            self.move_axis(world, 1, sd.y);
            self.move_axis(world, 0, sd.x);
            self.move_axis(world, 2, sd.z);
        }
    }

    fn move_axis(&mut self, world: &World, axis: usize, delta: f32) {
        if delta == 0.0 {
            if axis == 1 {
                self.on_ground = self.touching_ground(world);
            }
            return;
        }
        self.pos[axis] += delta;
        let b = self.aabb();
        let min = block_of(b.min);
        let max = block_of(b.max - Vec3::splat(1e-4));
        let mut hit = false;
        'outer: for y in min.y..=max.y {
            for z in min.z..=max.z {
                for x in min.x..=max.x {
                    if world.is_solid(IVec3::new(x, y, z)) {
                        hit = true;
                        break 'outer;
                    }
                }
            }
        }
        if hit {
            let half = match axis {
                1 => 0.0,
                _ => WIDTH * 0.5,
            };
            let extent = match axis {
                1 => HEIGHT,
                _ => WIDTH * 0.5,
            };
            if delta > 0.0 {

                let lead = self.pos[axis] + extent;
                self.pos[axis] = lead.floor() - extent - 1e-4;
            } else {
                let lead = self.pos[axis] - half;
                self.pos[axis] = lead.ceil() + half + 1e-4;
            }
            self.vel[axis] = 0.0;
            if axis == 1 && delta < 0.0 {
                self.on_ground = true;
            }
        } else if axis == 1 {
            self.on_ground = delta < 0.0 && self.touching_ground(world);
        }
    }

    fn touching_ground(&self, world: &World) -> bool {
        let b = self.aabb();
        let y = (b.min.y - 0.02).floor() as i32;
        let min = block_of(b.min);
        let max = block_of(b.max - Vec3::splat(1e-4));
        for z in min.z..=max.z {
            for x in min.x..=max.x {
                if world.is_solid(IVec3::new(x, y, z)) {
                    return true;
                }
            }
        }
        false
    }

    pub fn pick(&self, world: &World) -> Option<Pick> {
        let o = self.eye();
        let d = self.look_dir();
        let mut cell = block_of(o);
        let step = IVec3::new(
            d.x.signum() as i32,
            d.y.signum() as i32,
            d.z.signum() as i32,
        );
        let inv = Vec3::new(1.0 / d.x, 1.0 / d.y, 1.0 / d.z).abs();
        let mut tmax = Vec3::new(
            if d.x > 0.0 {
                (cell.x as f32 + 1.0 - o.x) * inv.x
            } else {
                (o.x - cell.x as f32) * inv.x
            },
            if d.y > 0.0 {
                (cell.y as f32 + 1.0 - o.y) * inv.y
            } else {
                (o.y - cell.y as f32) * inv.y
            },
            if d.z > 0.0 {
                (cell.z as f32 + 1.0 - o.z) * inv.z
            } else {
                (o.z - cell.z as f32) * inv.z
            },
        );
        let mut normal = IVec3::ZERO;
        let mut t = 0.0;
        for _ in 0..64 {
            if world.is_solid(cell) {
                return Some(Pick {
                    block: cell,
                    adjacent: cell + normal,
                    normal,
                    distance: t,
                });
            }
            if tmax.x < tmax.y && tmax.x < tmax.z {
                t = tmax.x;
                tmax.x += inv.x;
                cell.x += step.x;
                normal = IVec3::new(-step.x, 0, 0);
            } else if tmax.y < tmax.z {
                t = tmax.y;
                tmax.y += inv.y;
                cell.y += step.y;
                normal = IVec3::new(0, -step.y, 0);
            } else {
                t = tmax.z;
                tmax.z += inv.z;
                cell.z += step.z;
                normal = IVec3::new(0, 0, -step.z);
            }
            if t > REACH {
                break;
            }
        }
        None
    }

    pub fn intersects_block(&self, p: IVec3) -> bool {
        let b = Aabb::new(p.as_vec3(), p.as_vec3() + Vec3::ONE);
        self.aabb().intersects(&b)
    }
}
