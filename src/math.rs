use glam::{IVec3, Mat4, Vec3, Vec4};

pub const WORLD_HEIGHT: i32 = 512;
pub const Y_OFFSET: i32 = 64;
pub const CHUNK_SHIFT: i32 = 6;
pub const CHUNK_DIM_I: i32 = 64;

#[inline]
pub fn block_of(p: Vec3) -> IVec3 {
    p.floor().as_ivec3()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }
    pub fn intersects(&self, o: &Aabb) -> bool {
        self.min.x < o.max.x
            && self.max.x > o.min.x
            && self.min.y < o.max.y
            && self.max.y > o.min.y
            && self.min.z < o.max.z
            && self.max.z > o.min.z
    }
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn distance_to(&self, p: Vec3) -> f32 {
        let d = (self.min - p).max(p - self.max).max(Vec3::ZERO);
        d.length()
    }
}

#[allow(clippy::too_many_arguments)]
pub fn view_proj(
    pos: Vec3,
    fwd: Vec3,
    right: Vec3,
    up: Vec3,
    tan_half_fov: f32,
    aspect: f32,
    near: f32,
    far: f32,
) -> Mat4 {

    let view = Mat4::from_cols(
        Vec4::new(right.x, up.x, -fwd.x, 0.0),
        Vec4::new(right.y, up.y, -fwd.y, 0.0),
        Vec4::new(right.z, up.z, -fwd.z, 0.0),
        Vec4::new(-right.dot(pos), -up.dot(pos), fwd.dot(pos), 1.0),
    );
    let f = 1.0 / tan_half_fov;
    let proj = Mat4::from_cols(
        Vec4::new(f / aspect, 0.0, 0.0, 0.0),
        Vec4::new(0.0, f, 0.0, 0.0),
        Vec4::new(0.0, 0.0, far / (near - far), -1.0),
        Vec4::new(0.0, 0.0, near * far / (near - far), 0.0),
    );
    proj * view
}

pub struct Frustum {
    pub planes: [(Vec3, f32); 6],
}

impl Frustum {
    pub fn from_camera(
        pos: Vec3,
        fwd: Vec3,
        right: Vec3,
        up: Vec3,
        tan_half_fov: f32,
        aspect: f32,
        far: f32,
    ) -> Self {
        let hx = tan_half_fov * aspect;
        let hy = tan_half_fov;
        let mk = |n: Vec3| {
            let n = n.normalize();
            (n, -n.dot(pos))
        };

        let left = mk(fwd * hx + right);
        let rightp = mk(fwd * hx - right);
        let bottom = mk(fwd * hy + up);
        let top = mk(fwd * hy - up);
        let near = mk(fwd);
        let farp = {
            let n = -fwd;
            (n, -n.dot(pos + fwd * far))
        };
        Self {
            planes: [left, rightp, bottom, top, near, farp],
        }
    }

    pub fn contains_aabb(&self, b: &Aabb) -> bool {
        for (n, d) in self.planes.iter() {
            let p = Vec3::new(
                if n.x > 0.0 { b.max.x } else { b.min.x },
                if n.y > 0.0 { b.max.y } else { b.min.y },
                if n.z > 0.0 { b.max.z } else { b.min.z },
            );
            if n.dot(p) + d < 0.0 {
                return false;
            }
        }
        true
    }
}

pub const TAA_PHASES: u64 = 8;

pub fn halton_jitter(index: u64) -> glam::Vec2 {
    let i = (index % TAA_PHASES) as u32 + 1;
    glam::Vec2::new(halton(i, 2) - 0.5, halton(i, 3) - 0.5)
}

fn halton(mut i: u32, base: u32) -> f32 {
    let (mut f, mut r) = (1.0f32, 0.0f32);
    while i > 0 {
        f /= base as f32;
        r += f * (i % base) as f32;
        i /= base;
    }
    r
}
