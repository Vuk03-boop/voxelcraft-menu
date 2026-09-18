use crate::math::WORLD_HEIGHT;
use crate::voxel::ChunkKey;
use glam::{IVec3, Vec3};
use rustc_hash::FxHashSet;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Residency {
    Absent,
    Empty,
    Solid,
}

impl Residency {
    #[inline]
    pub fn is_resident(self) -> bool {
        self != Residency::Absent
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LodConfig {
    pub max_lod: u8,
    pub factor: f32,
    pub view_distance: f32,

    pub fade_band: f32,

    pub fade_schedule: FadeSchedule,

    pub cover_depth: u8,

    pub cross_fade: bool,

    pub shadow_share: bool,

    pub offscreen_shadows: bool,
}

impl Default for LodConfig {
    fn default() -> Self {
        Self {
            max_lod: 4,
            factor: 2.0,
            view_distance: 2600.0,
            fade_band: 1.25,
            fade_schedule: FadeSchedule::Smoothstep,
            cover_depth: 2,
            cross_fade: true,
            shadow_share: true,
            offscreen_shadows: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderItem {
    pub key: ChunkKey,

    pub fade: f32,
}

impl RenderItem {

    pub const fn solid(key: ChunkKey) -> Self {
        Self { key, fade: 1.0 }
    }

    #[inline]
    pub fn share(self) -> f32 {
        if self.fade >= 0.0 {
            self.fade
        } else {
            1.0 + self.fade
        }
    }

    #[inline]
    pub fn is_fading(self) -> bool {
        self.fade < 1.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FadeSchedule {

    Smoothstep,

    Smootherstep,
}

pub fn histogram(items: &[RenderItem], max_lod: u8) -> Vec<usize> {
    let mut h = vec![0usize; max_lod as usize + 1];
    for it in items {
        h[it.key.lod as usize] += 1;
    }
    h
}

#[derive(Default)]
pub struct Planner {

    split: FxHashSet<ChunkKey>,
    scratch: FxHashSet<ChunkKey>,
}

impl Planner {

    pub fn plan(
        &mut self,
        cam: Vec3,
        cfg: &LodConfig,
        status: &impl Fn(ChunkKey) -> Residency,
        desired: &mut Vec<ChunkKey>,
        render: &mut Vec<RenderItem>,
    ) {
        desired.clear();
        render.clear();
        self.scratch.clear();
        let root = 64i32 << cfg.max_lod;
        let r = (cfg.view_distance / root as f32).ceil() as i32 + 1;
        let cx = (cam.x / root as f32).floor() as i32;
        let cz = (cam.z / root as f32).floor() as i32;
        let mut ctx = Ctx {
            cam,
            cfg,
            status,
            was: &self.split,
            now: &mut self.scratch,
            desired,
            render,
        };
        for dz in -r..=r {
            for dx in -r..=r {
                visit(
                    ChunkKey::new(cfg.max_lod, IVec3::new(cx + dx, 0, cz + dz)),
                    &mut ctx,
                );
            }
        }
        std::mem::swap(&mut self.split, &mut self.scratch);
    }
}

struct Ctx<'a, F: Fn(ChunkKey) -> Residency> {
    cam: Vec3,
    cfg: &'a LodConfig,
    status: &'a F,
    was: &'a FxHashSet<ChunkKey>,
    now: &'a mut FxHashSet<ChunkKey>,
    desired: &'a mut Vec<ChunkKey>,
    render: &'a mut Vec<RenderItem>,
}

fn vacant(key: ChunkKey, cam: Vec3, cfg: &LodConfig) -> bool {
    key.origin().y >= WORLD_HEIGHT || key.bounds().distance_to(cam) > cfg.view_distance
}

fn fine_share<F: Fn(ChunkKey) -> Residency>(key: ChunkKey, ctx: &Ctx<F>) -> f32 {
    if key.lod == 0 {
        return 0.0;
    }
    let inner = key.size() as f32 * ctx.cfg.factor;
    let outer = inner * ctx.cfg.fade_band;
    let d = key.center().distance(ctx.cam);
    if !ctx.cfg.cross_fade {
        let threshold = if ctx.was.contains(&key) { outer } else { inner };
        return f32::from(d < threshold);
    }
    if d <= inner {
        return 1.0;
    }
    if d >= outer {
        return 0.0;
    }
    let x = (outer - d) / (outer - inner);
    match ctx.cfg.fade_schedule {
        FadeSchedule::Smoothstep => x * x * (3.0 - 2.0 * x),
        FadeSchedule::Smootherstep => x * x * x * (x * (6.0 * x - 15.0) + 10.0),
    }
}

fn visit<F: Fn(ChunkKey) -> Residency>(key: ChunkKey, ctx: &mut Ctx<F>) -> bool {
    if vacant(key, ctx.cam, ctx.cfg) {
        return true;
    }
    let f = fine_share(key, ctx);
    if f > 0.0 {
        ctx.now.insert(key);
        if f < 1.0 {

            ctx.desired.push(key);
        }
        let start = ctx.render.len();
        let mut covered = true;
        for dz in 0..2 {
            for dy in 0..2 {
                for dx in 0..2 {
                    covered &= visit(key.child(dx, dy, dz), ctx);
                }
            }
        }
        if covered {
            if f < 1.0 {
                dissolve(key, f, start, ctx);
            }
            return true;
        }

        if (ctx.status)(key).is_resident() {
            ctx.render.truncate(start);
            ctx.render.push(RenderItem::solid(key));
            return true;
        }
        return false;
    }

    ctx.desired.push(key);
    cover(key, ctx, ctx.cfg.cover_depth)
}

fn dissolve<F: Fn(ChunkKey) -> Residency>(key: ChunkKey, f: f32, start: usize, ctx: &mut Ctx<F>) {
    if (ctx.status)(key) != Residency::Solid
        || ctx.render.len() == start
        || ctx.render[start..].iter().any(|it| it.is_fading())
    {
        return;
    }
    for it in &mut ctx.render[start..] {
        it.fade = f;
    }
    ctx.render.push(RenderItem { key, fade: -f });
}

fn cover<F: Fn(ChunkKey) -> Residency>(key: ChunkKey, ctx: &mut Ctx<F>, depth: u8) -> bool {
    if vacant(key, ctx.cam, ctx.cfg) {
        return true;
    }
    if (ctx.status)(key).is_resident() {
        ctx.render.push(RenderItem::solid(key));
        return true;
    }
    if key.lod == 0 || depth == 0 {
        return false;
    }
    let start = ctx.render.len();
    for dz in 0..2 {
        for dy in 0..2 {
            for dx in 0..2 {
                if !cover(key.child(dx, dy, dz), ctx, depth - 1) {
                    ctx.render.truncate(start);
                    return false;
                }
            }
        }
    }
    true
}
