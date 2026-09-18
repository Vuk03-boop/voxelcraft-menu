//! Implicit chunk octree, LOD selection and the per-frame draw plan.
//!
//! `LODError = size * factor - distance(center, camera)`; positive cells subdivide.
//!
//! Selection alone is not enough to draw with. A node's children stream in over many
//! frames, so the planner also decides *what stands in* for a node that is not ready
//! yet, and it has to answer that question the same way whether the camera is moving
//! toward the node or away from it. See `Planner::plan`.
//!
//! A node does not switch to its children at a distance, it **dissolves** into them
//! across a band of distances: both representations are emitted, each carrying the share
//! of rays it owns, and the temporal pass averages the per-ray choice into a cross-fade.
//! `RenderItem::fade` is how the share is carried, and `fine_share` is the whole of the
//! transition schedule.

use crate::math::WORLD_HEIGHT;
use crate::voxel::ChunkKey;
use glam::{IVec3, Vec3};
use rustc_hash::FxHashSet;

/// What the world can tell the planner about one node. `Empty` is resident with nothing
/// to draw, which covers a volume perfectly well but cannot be half of a cross-fade:
/// the partner would be dissolving into sky.
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
    /// Outer edge of the parent/child transition, as a multiple of the split distance.
    /// A node is entirely its children inside `size * factor` and entirely itself outside
    /// `size * factor * fade_band`; in between the two are dissolved into each other.
    ///
    /// With `cross_fade` off the same band is a plain hysteresis deadband -- a node that
    /// is already split stays split out to the outer edge -- which is what it was before
    /// the fade existed, and what `--no-fade` restores.
    pub fade_band: f32,
    /// The shape of [`fine_share`] inside the band. This is D2b's surviving candidate,
    /// kept behind a flag rather than flipped blind: the dither mechanism was ruled out
    /// (the TAA-arm sweep holds both spikes to within 0.0044), which leaves *how fast the
    /// share itself moves* -- and smoothstep is only flat in `x`, never in distance. The
    /// rate-per-block is `1.5/(outer-inner)` at mid-band for it and
    /// `1.875/(outer-inner)` at the same point for [`FadeSchedule::Smootherstep`], whose
    /// payment for killing the edge curvature is a *faster* middle -- which is why
    /// this is a measurement arm and not a default, until the sweep separates
    /// "the band is too short" (width) from "the share jumps at the edges" (shape).
    pub fade_schedule: FadeSchedule,
    /// How many levels below a selected node the planner may look for a stand-in.
    pub cover_depth: u8,
    /// Dissolve across the band instead of popping somewhere inside it.
    pub cross_fade: bool,
    /// Whether the shadow/AO grid resolves a cross-fade to the half carrying most of the
    /// rays rather than to the fine half unconditionally. Batch 62, roadmap D2.
    ///
    /// **Here rather than beside the renderer's other knobs because it is an LOD policy**:
    /// what it decides is which of a transition's two representations secondary rays see,
    /// and `cross_fade` beside it decides whether there is a transition at all. With
    /// `cross_fade` off nothing fades, every share is 1.0, and this cannot be reached.
    pub shadow_share: bool,
    /// Whether a chunk the view frustum rejects is still written into the shadow/AO grid.
    /// Batch 64, roadmap D3; `--no-offscreen-shadows` clears it.
    ///
    /// **Beside `shadow_share` because it is the same policy asked one level up**: that one
    /// decides *which representation* of a chunk the grid holds, this one decides *whether
    /// the chunk is in the grid at all*. Both are answers to "what may a secondary ray see",
    /// and neither is anything the marcher knows about -- what changes is which chunk index
    /// the CPU writes into `grid`.
    ///
    /// It is an LOD policy for the same reason `shadow_share` is: the plan this filters is
    /// `render_set`, and how far that reaches is `view_distance` and `factor` beside it.
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

/// One entry of the draw plan: a chunk, and which rays get to see it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderItem {
    pub key: ChunkKey,
    /// **Signed dither threshold, not this entry's own share.** `|fade|` is the fraction
    /// of rays belonging to the *fine* side of a transition, and the sign says which side
    /// this entry is: positive marches the rays whose dither falls below the threshold,
    /// negative the rays at or above it. A chunk outside every transition carries `1.0`
    /// and marches everything.
    ///
    /// Storing the threshold rather than each entry's own share is what makes the two
    /// halves exactly complementary. They must be: a pixel both sides march resolves by
    /// depth in the visibility buffer and punches the coarse surface through the fine one,
    /// and a pixel neither marches is a hole.
    pub fade: f32,
}

impl RenderItem {
    /// An entry every ray marches.
    pub const fn solid(key: ChunkKey) -> Self {
        Self { key, fade: 1.0 }
    }

    /// Fraction of rays that actually march this entry.
    #[inline]
    pub fn share(self) -> f32 {
        if self.fade >= 0.0 {
            self.fade
        } else {
            1.0 + self.fade
        }
    }

    /// Half of a cross-fade rather than a whole chunk.
    #[inline]
    pub fn is_fading(self) -> bool {
        self.fade < 1.0
    }
}

/// The share curve across a transition band.
///
/// Both are zero, one and flat at the band edges; they differ in how hard they push
/// *through* the middle, and that difference is the only lever D2b has left. `--no-fade`'s
/// hysteresis is a third shape entirely and does not pass through this enum: it bypasses
/// `fine_share`'s smooth arm wholesale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FadeSchedule {
    /// `x * x * (3 - 2x)` -- what every build since the dissolve shipped has run, and the
    /// default until the sweep says otherwise. Zero slope at the edges, peak slope 1.5 at
    /// mid-band.
    Smoothstep,
    /// `x^3 * (6x^2 - 15x + 10)`: zero slope AND zero curvature at the edges, peak slope
    /// 1.875 at mid-band. The shape that makes schedule and band width independent
    /// questions -- with it a wider band does not have to double as the only way to slow
    /// the edge down.
    Smootherstep,
}

/// Number of chunks per LOD level in a plan.
pub fn histogram(items: &[RenderItem], max_lod: u8) -> Vec<usize> {
    let mut h = vec![0usize; max_lod as usize + 1];
    for it in items {
        h[it.key.lod as usize] += 1;
    }
    h
}

#[derive(Default)]
pub struct Planner {
    /// Nodes split on the previous plan; the source of the merge deadband. Read only when
    /// `cross_fade` is off -- the fade schedule is a pure function of distance and needs
    /// no history, which is exactly why it cannot flutter.
    split: FxHashSet<ChunkKey>,
    scratch: FxHashSet<ChunkKey>,
}

impl Planner {
    /// Walk the octree once and fill both lists.
    ///
    /// `desired` is what the camera wants and drives streaming. `render` is what can
    /// actually be drawn this frame. Every point of the visible world is covered by
    /// entries whose shares sum to exactly one: outside a transition that is a single
    /// entry tiling the volume, and inside one it is a coarse entry and its fine subtree
    /// splitting the rays between them.
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

/// Nothing here needs drawing, whatever the residency.
fn vacant(key: ChunkKey, cam: Vec3, cfg: &LodConfig) -> bool {
    key.origin().y >= WORLD_HEIGHT || key.bounds().distance_to(cam) > cfg.view_distance
}

/// Fraction of this node's rays that belong to its children: 1.0 fully split, 0.0 fully
/// merged, and strictly in between inside the transition band.
///
/// With the fade on this is a pure function of distance, and that is the point. The old
/// deadband needed last frame's answer to produce this frame's, which is what a camera
/// sitting on a boundary could make flutter; a smoothstep of distance gives the same
/// picture whichever direction the camera arrived from, and cannot oscillate because there
/// is nothing to oscillate between. The band edges are where the draw list changes, and
/// smoothstep is flat at both, so an entry joins and leaves at zero share *and* zero rate
/// of change -- a camera crossing the outer edge in a single frame still cannot make the
/// list change visible.
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

/// Returns whether `key`'s volume ended up fully covered by pushes into `render`.
fn visit<F: Fn(ChunkKey) -> Residency>(key: ChunkKey, ctx: &mut Ctx<F>) -> bool {
    if vacant(key, ctx.cam, ctx.cfg) {
        return true;
    }
    let f = fine_share(key, ctx);
    if f > 0.0 {
        ctx.now.insert(key);
        if f < 1.0 {
            // Mid-transition the node is drawn beside its children, so it is wanted in its
            // own right and has to stream and stay resident like any other selected chunk.
            // This is one level above the children and no further: the request walk in
            // `stream` still stops at the first resident ancestor.
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
        // Part of the subtree is still streaming. Standing in with this node covers the
        // hole, and replacing the ready children rather than drawing over them is what
        // keeps a half-refined node from showing coarse geometry poking through fine.
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

/// Turn the subtree pushed since `start` into the fine half of a cross-fade, with `key`
/// itself as the coarse half.
///
/// Three ways out, all of which leave the children drawn whole -- which still covers the
/// volume exactly once, so the fallback is a pop and never a hole:
///
/// - the coarse side has no geometry to dissolve from, so half the rays would be
///   dissolving into sky rather than into a coarser surface;
/// - the subtree is empty, so there is no fine side to pair with;
/// - the subtree already contains a transition of its own. At the default `factor` and
///   `fade_band` two levels' bands cannot overlap -- a node's band starts at `2 S` and its
///   children's ends at `1.25 S`, and a child centre is at most `0.43 S` off its parent's
///   -- but a small `--streaming-factor` closes that gap, and nested dithers would need
///   two independent thresholds to stay complementary against one dither value.
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

/// Draw `key`, or the finer chunks that tile it exactly. Descending matters on the
/// merge: the children the camera is backing away from are still resident, so they
/// stand in for their parent for free instead of the view falling back to a much
/// coarser ancestor or to nothing at all.
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



