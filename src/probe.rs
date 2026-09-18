//! The directional sky-visibility field: a six-axis ambient cube every 8 blocks, baked on a
//! worker and read by `shade_hit` as one trilinear tap.
//!
//! **What it replaces is a constant.** `face_shade` is a hardcoded 1.00/0.80/0.62/0.50 per
//! normal standing in for the whole of directional occlusion -- the top of a block is bright
//! because it is a top, not because it can see the sky, and a wall at the foot of a cliff is
//! lit exactly like the same wall in an open field. This module derives that number instead,
//! and it derives it on the resource nothing is using: sixteen rayon threads that are idle
//! once the world is resident, against the frame ALU that is the wall. See `CLAUDE.md`'s
//! scarcity table and roadmap R1.
//!
//! **Six axes is not an approximation here.** Every cube face in this world has an
//! axis-aligned normal, so a six-entry cube -- one value per +/-X, +/-Y, +/-Z -- is *exact*
//! for a cube face rather than a low-order fit to a hemisphere, and the shader evaluates it by
//! selecting one entry instead of reconstructing an SH basis it would have no other use for.
//! It is also why the read is **one** tap: the six entries are six stacked slabs of one 3D
//! texture and a face samples only the slab its normal names.
//!
//! **It stores transfer and not radiance, and that is a correctness requirement rather than an
//! optimisation.** `day_length_seconds` is 600 and `freeze_time` is false, so a session walks a
//! full sun cycle every ten minutes while `stream::relight` fires only on an edit. Anything
//! baked as a *lit result* would be wrong within seconds of being built -- and would look
//! right in every still capture in the fixture, because every vantage pins `--time`. What is
//! stored here is a shading *factor* that `frame.daylight` and the flood's own levels are
//! multiplied through at read time, exactly as the sky nibble already is.
//!
//! ## What it sees, and what it deliberately does not
//!
//! **The occluder is the height field and nothing else.** A probe's horizon is marched against
//! `WorldGen::height`, not against the chunk's voxels, and that is a decision with three
//! consequences worth stating before the first one surprises somebody:
//!
//! - **It is a pure function of world position, so the field has no seams.** This is the whole
//!   of roadmap P5b for this field. `light::flood` never leaves its own 64^3, so two
//!   neighbouring chunks disagree at their shared face; a trilinear tap would smear that
//!   disagreement into a four-block band on *both* sides of every chunk boundary in the world,
//!   which is far worse than the 132 pixels batch 51 measured for the nine-cell gather. The
//!   apron the roadmap planned -- bake a 10^3 seeded from the neighbours' edge cells, re-bake
//!   when a neighbour lands -- solves that with a residency dependency and an invalidation
//!   graph. Marching a field that does not know which chunk is asking solves it by
//!   construction, with no apron, no dependency and no re-bake.
//! - **It reaches past the neighbours.** A mountain 190 blocks away shades a wall here. The
//!   10^3 apron could not have done that at all: it sees one chunk in each direction.
//! - **It cannot see trees, caves, overhangs or edits.** Those are exactly the geometry the
//!   height field does not carry. The *flood* still carries them -- a cell under a canopy has a
//!   lower sky nibble and the nine-cell gather reads it -- so what is lost is their
//!   contribution to *directionality*, not to level. Roadmap R2 gathers a bounce at the probe
//!   and needs real geometry to do it; that is the batch that pays for the apron, and it will
//!   have a reason to.
//!
//! ## What the cube stores, and why the old table survives as its calibration
//!
//! **The cube is a directional occlusion term normalised to 1 in an open field**, multiplying
//! `face_shade`'s per-normal base rather than replacing the number outright. That is a
//! deliberate retreat from the first build of this batch and the reason is worth keeping.
//!
//! Storing the visibility *absolutely* -- mapping an unoccluded side face onto 0.75 between the
//! 0.62 and 0.80 the table gives X and Z -- is the more honest physics: in an open field the sky
//! is azimuthally symmetric, so two side faces of one cube genuinely do receive the same
//! ambient, and the asymmetry is a readability convention Minecraft invented and this engine
//! inherited. It was built that way first, and the picture says the physics is not the point
//! here: the default vantage came back **brighter and flatter**, because that frame is mostly
//! X-facing steps going 0.62 -> 0.75, and the occlusion the batch exists to add was buried
//! under an exposure shift nobody asked for. A before/after pair cannot separate two changes,
//! and a before/after pair is what ranks this.
//!
//! So the cube answers *how much of an open field's sky does this position still see, along
//! this axis* -- 1 everywhere the terrain is open, below 1 in a valley, at the foot of a cliff
//! or inside a gully -- and the image changes in exactly those places and nowhere else. The
//! table keeps two jobs it is still the best answer for: the X/Z asymmetry, and the open-sky
//! calibration. **Roadmap R2 and R5 are what retire it** -- a real bounce and a real sky
//! integral give the absolute number a reference that is not an authored constant, and at that
//! point the normalisation has something better to divide by.
//!
//! **Where there is no sky the field says nothing and the old table stands.** A probe buried in
//! rock -- which is every probe inside a cave, because the height field says solid there --
//! sees no sky in any direction, and a cube of pure visibility would flatten a cave to one
//! value and delete the face relief that makes it readable. The sky term genuinely has no
//! opinion there, so [`SKY_BLEND`] fades the occlusion back to 1 and leaves `face_shade`'s own
//! constants standing, which remain the answer for block-lit directionality until R2 and R4
//! replace them.
//!
//! **And 1 is the number that makes that exact rather than nearly exact.** The field stores the
//! occlusion *alone* and the shader multiplies `face_shade` by it, so an unoccluded probe, a
//! buried one, an unbaked texel and a chunk at a coarse LOD all arrive at `face_shade(id) *
//! 1.0`, which is `face_shade(id)` to the bit. The first build of this batch baked the
//! *product* instead, and the sweep caught it: `cave` moved 367,222 pixels at max delta 1,
//! because 0.62 and 0.80 do not survive a round trip through `f16` and 1.0 does. A whole
//! vantage's worth of noise, from storing a number the shader already had.

use crate::block::{self, BlockId};
use crate::voxel::chunk::ChunkKey;
use crate::worldgen::WorldGen;
use glam::{IVec3, Vec3};

/// Blocks between probes. **4, and the batch handing it down said exactly when**: the
/// original 8 stood on "indirect light is low frequency, 4 is one constant away if a lit
/// wall ever visibly bands" -- and the field then spent two batches adding has to say it
/// bands. A doorway, an overhang lip and a trunk base are all features *smaller* than
/// eight blocks, so at 8 they share one probe between them and read each other's
/// occlusion. At 4 they are four probes apart and do not. The whole change is constants:
/// the tap stays one trilinear read, the lattice keeps its 512-block XZ period (the
/// texture's X and Z axes widen 64 -> 128 to hold it, which is what lets every
/// `UPLOAD_REACH` claim in this file stand unedited), and the cost lands in the two
/// resources the scarcity table has marked idle since batch 1 -- 8x the bake on the
/// workers, 8x the VRAM (12.6 MB -> ~100 MB, which is roadmap L5's whole argument).
pub const SPACING: i32 = 4;

/// Probes per chunk axis. Not independent -- a chunk is 64 blocks and the probes tile it.
pub const PER_AXIS: usize = 64 / SPACING as usize;

/// The cube's entries, indexed by `normal_id`: 0 = -X, 1 = +X, 2 = -Y, 3 = +Y, 4 = -Z, 5 = +Z.
/// The same encoding `normal_of` uses in `common.wgsl`, which is what lets the shader use
/// `normal_id` as the slab index with no remap.
pub const AXES: usize = 6;

/// Probes in one chunk.
pub const PROBES: usize = PER_AXIS * PER_AXIS * PER_AXIS;

/// How far a horizon ray is marched, in blocks.
///
/// Past this a ridge subtends so little of the hemisphere that it cannot move a shading factor
/// stored at 8-block spacing, and the march is the batch's whole worker cost. It is well past
/// the 160 blocks LOD 0 itself reaches, so a probe is never occluded by terrain it is rendered
/// beside and not by the terrain behind that.
const REACH: f32 = 192.0;

/// Samples along one azimuth, spaced geometrically from one block out to [`REACH`].
///
/// Geometric rather than uniform because a horizon is an angular quantity: a ridge at 4 blocks
/// and the same ridge at 128 need the same *angular* resolution, and uniform steps would spend
/// the whole budget on the far field where it buys nothing.
const STEPS: usize = 20;

/// Azimuths per probe column. Sixteen is 22.5 degrees, which resolves a chunk-sized cliff face
/// at the distance one is usually seen from; the cost is linear in it and it is the first thing
/// to cut if the bake ever needs to be cheaper -- `cut directions before cutting probes`, since
/// the basis has six entries and this is already under three azimuths per entry.
pub const AZIMUTHS: usize = 16;

/// How much sky a probe has to see before its occlusion is believed at all.
///
/// Below this the cube fades back to 1 -- no occlusion, `face_shade`'s constants standing --
/// which is the module note's cave rule. A quarter of the sky is well under anything outdoors
/// and well over anything buried, so the blend band is narrow and sits entirely inside solid
/// rock or a deep slot. **Without it a probe under the height field reads visibility 0 on every
/// axis and multiplies the whole cave to black**, which is not a subtle failure but is a silent
/// one: every vantage outdoors would look right.
const SKY_BLEND: f32 = 0.25;

/// How many of [`STEPS`] radii the **bounce** gather samples, as a stride.
///
/// The horizon needs every step because it is placing an *edge* -- a ridge one step further
/// out is a different shadow. The bounce is a weighted *mean* over the same samples, and a
/// mean is low frequency in exactly the way an edge is not, so it can be read off half the
/// schedule for half the biome lookups. **This is the one knob that trades bake cost against
/// the bounce and nothing else**, and it is a stride into the existing schedule rather than a
/// second radius list, so the two gathers cannot disagree about where a sample is.
const ALBEDO_STRIDE: usize = 2;

/// `resolve`'s own ambient floor tint, which the bake divides back out.
///
/// **This is the second copy of a literal that lives in `resolve.wgsl`**, and
/// `tests/probe.rs::floor_tint_matches_the_shader` is what stops the two drifting -- the same
/// shape of guard `faces_per_block_matches_the_shader` is, and for the same reason: a
/// disagreement here changes the identity value of the whole field and every absence of the
/// feature stops being exact, which no capture would announce.
pub const FLOOR_TINT: [f32; 3] = [0.85, 0.90, 1.00];

/// The ground the bounce is normalised against, so that ground of this colour reproduces the
/// authored floor's **brightness** exactly and changes only its colour.
///
/// **Grass, and the choice is the whole of what keeps this batch rankable.** The field stores a
/// per-channel multiplier on `resolve`'s `floor_rgb`, and over grass that multiplier is
/// `(0.416, 1.259, 0.156)` -- so the floor over grass becomes green at *exactly* the luminance
/// it had. A before/after pair cannot separate a global exposure shift from a bounce appearing,
/// which is the mistake batch 57 made once and retreated from; pinning the luminance at the
/// world's commonest ground is what stops this batch making it again. Grass is what `default`,
/// `terraces` and `slope` stand on.
///
/// **Be precise about which identity is which, because there are two and only one of them is
/// per channel.** An unbaked texel, a coarse LOD, a probe with no sky and `--no-probe-bounce`
/// all return 1.0 on every channel, and *that* is what makes every absence of the feature
/// bit-exact. Reference ground returns 1.0 in **luminance** and not per channel, and that is
/// what makes the feature's arrival a change of hue rather than of exposure. An earlier draft
/// of this comment claimed the first identity for the second and the arithmetic is three lines:
/// `albedo * luma(FLOOR_TINT) / (luma(albedo_ref) * FLOOR_TINT)` is 1 in luminance by
/// construction and is 1 per channel only if the reference ground is grey.
///
/// **So the authored constant keeps the job it is still the best answer for, and stops being
/// the answer itself**: `(0.85, 0.90, 1.00)` was calibrated by eye against a mostly-grass
/// world, and reading it as *the floor over grass* is what lets everything else be derived
/// from it. That is exactly what batch 57 did to `face_shade`, one rung earlier.
///
/// **The magnitude is normalised in luminance and the colour is not normalised at all**,
/// which is the point: dividing per channel by grass's own albedo would put grass's green
/// straight back and leave a field carrying nothing, and it would divide by 0.0407 in blue,
/// so snow would come back at twenty-two times the floor. Luminance carries the *how much*
/// and the raw albedo carries the *what colour*.
const BOUNCE_REF: BlockId = block::GRASS;

/// One chunk's baked cubes, laid out slab-major so each axis is one contiguous 8x8x8 run and
/// the upload is six `write_texture` calls with no repacking.
///
/// Index: `axis * PROBES + (pz * PER_AXIS + py) * PER_AXIS + px`, x fastest, which is the order
/// a 3D texture write wants its rows in.
///
/// **The values are occlusion and not shading**: 1 is unoccluded, and `shade_hit` multiplies
/// `face_shade` by whatever comes back. The module note is why that split, and it is the
/// difference between a bit-exact control and one that is merely close.
/// How close to the camera a chunk's bake must be before it may write the lattice
/// (batch 71, roadmap D1).
///
/// **The race this closes.** The field is one world-space lattice with a 512-block
/// toroidal period in X and Z, and a chunk writes only the 8x8x8 texel block its origin
/// owns -- so two chunks collide only when their origins differ by exactly 512 in an
/// axis. The plan makes that impossible in the steady state: LOD 0 reaches 160 blocks,
/// so one camera authors a single tight cluster. But residency is not the plan: a chunk
/// unloaded at a previous position survives 240 frames of grace in `World`, and a
/// teleport -- or a capture vantage far from spawn, which is what `lattice` is -- can
/// therefore hold two clusters 512 apart while builds from the old one are still landing.
/// Both map to the same texels, the window over the round is the drain loop, and the
/// `FxHashMap` iteration order -- which follows rayon completion timing -- picked a
/// different winner from run to run. One pixel flickered by +1 on all three channels
/// because of it, about one run in fifteen; the hunt is the D1 entry and its
/// discriminator table, and the bake's own purity is what corners this file as the only
/// place left that could move.
///
/// **The fix is a window rather than an order, and the arithmetic is why.** Choose the
/// reach so that *no two bakes inside it can ever collide*: uploading origins are
/// multiples of 64, both ends within the reach of one camera, so pair origins differ by
/// at most `2 * UPLOAD_REACH` -- and with that bounded under 512, the alias condition
/// *"differs by 512 in an axis"* cannot be met. The drained set's writes are then
/// pairwise disjoint, no order -- hash-based or sorted -- can change the outcome, and
/// the lattice becomes a pure function of the resident set at the capture. 224 is
/// `2 * 224 = 448 <= 512 - 64`, and origins sit on a 64 grid, so 448 is the last
/// representable shortfall before the period itself.
///
/// **The window measures XZ and never Y, and that is the batch-71 repair the measured
/// frame forced.** The alias the window forbids lives in X and Z only: `write_probe`
/// wraps the base texel by `rem_euclid(PROBE_DIM_XZ)` in those axes, while Y is a plain
/// division then a clamp -- nothing wraps in Y, so two chunks equal in XZ texel base but
/// different in Y write different texel rows, and Y separation can never *cause* an
/// alias. But as a spherical-3D predicate it consumed the sizing margin with camera
/// *height*: the plan's reach is horizontal, and at a 240-block-high camera the chunk
/// directly beneath sat 16 blocks past a 224 reach, deferred its bake every frame, and
/// left the lattice's 1.0 fill -- unoccluded, not dark -- under the whole pillar below,
/// flattening the picture in proportion to altitude (measured: +1.830 mean luminance
/// at `lod`, +0.173 at `open-sea`, +0.005 at `default`, vanishing as the camera drops).
/// Testing XZ alone is stricter where it must be and unbounded where it may be: the
/// horizontal part of the plan's 160-center reach is no more than 160, the origin
/// corner sits a horizontal half-diagonal (32*sqrt(2) ~= 45.3) outside that, and 224
/// covers the sum with 18.7 blocks of margin at *any* camera height, while the alias
/// bound `2R <= 448 < 512` holds for the same reason it always did. `tests/probe.rs`
/// asserts both sides of this.
///
/// **A bake outside it is not dropped, it is deferred.** `sync_world`'s caller re-queues
/// a deferred entry, and the next frame within the window uploads it -- which is also
/// why the teleport case converges: the stale island's queued bakes are held outside the
/// window until the camera actually returns there, and the 1.0 the texels held in the
/// meantime is the pre-batch-57 identity the field was created full of, not garbage.
pub const UPLOAD_REACH: f32 = 224.0;

/// Whether `origin`'s bake may write the lattice this frame (see [`UPLOAD_REACH`]).
/// Horizontal by design: the lattice aliases toroidally in X and Z and never in Y, so
/// the window that keeps two writes disjoint is a horizontal one, and a camera's
/// altitude must not be able to spend the plan-reach margin (batch 71's measured
/// failure mode, fixed from the A/B diff rather than argued).
pub fn uploadable(origin: IVec3, cam: Vec3) -> bool {
    let dx = origin.x as f32 - cam.x;
    let dz = origin.z as f32 - cam.z;
    dx * dx + dz * dz <= UPLOAD_REACH * UPLOAD_REACH
}

pub struct ProbeData {
    pub occl: Vec<f32>,
    /// Batch 58's ground bounce: three floats per entry, in the same order, holding a
    /// **per-channel multiplier on `resolve`'s `floor_rgb` whose identity is 1.0**. It rides
    /// in the same texel as `occl` -- `.rgb` here and `.a` there -- so the read stays the one
    /// trilinear tap batch 55 priced and batch 57 shipped. See [`bounce_from_albedo`].
    pub bounce: Vec<f32>,
}

impl ProbeData {
    #[inline]
    pub fn get(&self, axis: usize, px: usize, py: usize, pz: usize) -> f32 {
        self.occl[axis * PROBES + (pz * PER_AXIS + py) * PER_AXIS + px]
    }

    /// The bounce multiplier at one probe and axis.
    #[inline]
    pub fn get_bounce(&self, axis: usize, px: usize, py: usize, pz: usize) -> [f32; 3] {
        let o = (axis * PROBES + (pz * PER_AXIS + py) * PER_AXIS + px) * 3;
        [self.bounce[o], self.bounce[o + 1], self.bounce[o + 2]]
    }
}

/// **Batch 79, tracking document P-D: the bake's pure-function property, spent as a cache.**
/// The bake touches `WorldGen::height` and nothing resident -- that is its whole
/// correctness argument (no apron, no seams, 1.0 texels painting identity on any texel no
/// bake has reached) -- and the same argument is also *free*: two bakes of the same chunk
/// at the same world position must answer identically to the bit, so the second one
/// needn't run. The streamer caches by `ChunkKey` (which carries the position) with an
/// LRU at a fixed count and evicts silently: a miss costs exactly the bake it always did.
///
/// "Per chunk" here is honest arithmetic: `AXES` slabs of `PER_AXIS`^3 occlusion floats
/// (96 KB at 4-block spacing) plus three bounce floats apiece (288 KB); the cap below was
/// chosen against that number, not a round one -- 384 chunks of 384 KB is about 147 MB of
/// host RAM against a 13,000 us re-bake apiece, and the paid-off-frame rule says the RAM
/// is the idle resource and the 13 ms is not.
pub struct BakeCache {
    map: rustc_hash::FxHashMap<ChunkKey, (u64, ProbeData)>,
    tick: u64,
    cap: usize,
}

impl Default for BakeCache {
    fn default() -> Self {
        Self {
            map: rustc_hash::FxHashMap::default(),
            tick: 0,
            cap: 384,
        }
    }
}

impl BakeCache {
    /// A resident answer, cloned out. **Never takes the caller's lock-region along** -- the
    /// bake between `get` and `insert` is deliberately outside any lock, or all sixteen
    /// streaming threads would serialize on the one 13 ms stage they are for.
    pub fn get(&mut self, key: &ChunkKey) -> Option<ProbeData> {
        self.tick += 1;
        let (t, d) = self.map.get_mut(key)?;
        *t = self.tick;
        Some(ProbeData {
            occl: d.occl.clone(),
            bounce: d.bounce.clone(),
        })
    }

    /// Remember a fresh bake. Identical by the pure-function argument, so a duplicate is
    /// harmless; the eviction is LRU at a count chosen against the 384 KB entry.
    pub fn insert(&mut self, key: ChunkKey, d: ProbeData) {
        self.tick += 1;
        if self.map.len() >= self.cap {
            // O(cap) per admit. The cap is 384; the miss being paid next to it is 13 ms.
            let oldest = *self
                .map
                .iter()
                .min_by_key(|(_, (t, _))| *t)
                .expect("nonempty")
                .0;
            self.map.remove(&oldest);
        }
        self.map.insert(key, (self.tick, d));
    }
}


/// One probe's cube from its horizon alone, building the weight tables this call needs.
///
/// **[`bake`] hoists those tables out of its loops and calls the same inner function**, so this
/// is a thin entry point and not a second implementation -- which matters, because what it
/// exists for is `tests/probe.rs` asserting the value the whole batch's exposure rests on: an
/// unoccluded probe has to return exactly 1 on every axis, or shipping this changes how bright
/// the world is rather than how it is shaped, and the screenshot pair that ranks it cannot
/// separate the two.
pub fn cube_from_tangents(tan_h: &[f32; AZIMUTHS]) -> [f32; AXES] {
    let (_, lobe, lobe_sum) = tables();
    cube_from_horizon(tan_h, &lobe, &lobe_sum).0
}

/// The azimuth directions and the per-axis cosine lobes, which depend on nothing but the
/// constants above.
#[allow(clippy::type_complexity)]
fn tables() -> (
    [(f32, f32); AZIMUTHS],
    [[f32; AZIMUTHS]; AXES],
    [f32; AXES],
) {
    let mut dir = [(0.0f32, 0.0f32); AZIMUTHS];
    for (k, d) in dir.iter_mut().enumerate() {
        let a = std::f32::consts::TAU * k as f32 / AZIMUTHS as f32;
        *d = (a.cos(), a.sin());
    }
    let mut lobe = [[0.0f32; AZIMUTHS]; AXES];
    let mut lobe_sum = [0.0f32; AXES];
    for (axis, ax_lobe) in lobe.iter_mut().enumerate() {
        // Axis 0/1 are -X/+X and 4/5 are -Z/+Z; the azimuth each one points along.
        let (ax, az) = match axis {
            0 => (-1.0f32, 0.0f32),
            1 => (1.0, 0.0),
            4 => (0.0, -1.0),
            5 => (0.0, 1.0),
            _ => (0.0, 0.0),
        };
        for (k, w) in ax_lobe.iter_mut().enumerate() {
            *w = (dir[k].0 * ax + dir[k].1 * az).max(0.0);
            lobe_sum[axis] += *w;
        }
    }
    (dir, lobe, lobe_sum)
}

/// Bake one chunk's probes.
///
/// `heights` is the chunk's own 64x64 column heights, already in hand from `gen.generate`, and
/// is read for the columns inside the chunk purely to skip re-evaluating noise this thread has
/// already evaluated. **It is not a second source of truth**: `WorldGen::height` returns the
/// same value for the same column, which is what keeps the bake a pure function of world
/// position and therefore seamless -- `heights_agree_with_the_generator` in `tests/probe.rs`
/// is the guard, and the reason it exists is that a divergence here would show up as a chunk
/// boundary in the light and as nothing at all in any test.
pub fn bake(
    gen: &WorldGen,
    heights: &[i32; 64 * 64],
    origin: IVec3,
    bounce_shadow: bool,
) -> ProbeData {
    let mut occl = vec![0.0f32; AXES * PROBES];
    let mut bounce = vec![1.0f32; AXES * PROBES * 3];

    // Azimuth directions, cosine lobes and the geometric step schedule, all loop-invariant.
    let (dir, lobe, lobe_sum) = tables();
    let blobe = bounce_lobes(&lobe);
    // One division for the whole chunk rather than one per probe per channel.
    let scale = bounce_scale();
    let mut radius = [0.0f32; STEPS];
    let ratio = (REACH / 1.0f32).powf(1.0 / (STEPS - 1) as f32);
    let mut r = 1.0f32;
    for rad in radius.iter_mut() {
        *rad = r;
        r *= ratio;
    }

    // One horizon march per probe *column*, shared by the eight probes stacked in it: the
    // occluder set does not depend on the probe's height, only the angle it subtends does.
    // That is the difference between 20,480 height samples per chunk and eight times as many.
    //
    // **The bounce gather rides the same march and the same samples, and that is the whole
    // reason it is affordable.** What it adds per sample is one biome lookup on
    // [`ALBEDO_STRIDE`] and a form factor; what it cannot add is a sample the horizon does not
    // already take. The albedo *is* per probe height, unlike the occluder set, because the
    // form factor below is: ground eight blocks under a probe subtends four times what the
    // same ground subtends sixteen blocks under it.
    let mut tan_h = [[0.0f32; AZIMUTHS]; PER_AXIS];
    let mut alb = [[[0.0f32; 3]; AZIMUTHS]; PER_AXIS];
    let mut alb_w = [[0.0f32; AZIMUTHS]; PER_AXIS];
    // Per *ray* rather than per column: refilled for each azimuth, and only so that the two
    // passes over a ray can share one set of samples. See the loop below.
    let mut col_h = [0.0f32; STEPS];
    let mut col_a = [[0.0f32; 3]; STEPS];
    for pz in 0..PER_AXIS {
        for px in 0..PER_AXIS {
            let cx = origin.x + SPACING / 2 + SPACING * px as i32;
            let cz = origin.z + SPACING / 2 + SPACING * pz as i32;
            for row in tan_h.iter_mut() {
                row.fill(0.0);
            }
            for row in alb.iter_mut() {
                row.fill([0.0; 3]);
            }
            for row in alb_w.iter_mut() {
                row.fill(0.0);
            }
            for (k, &(dx, dz)) in dir.iter().enumerate() {
                // **Batch 60 splits one loop into two and takes no extra sample doing it.**
                // `ray_exposure` needs the whole height profile of the ray before it can say
                // what any one step on it can see, and the step loop below used to consume
                // each height the moment it was taken. Sampling into `col_h` first is what
                // lets the second bounce be free; `wx`, `wz` and the `surface_block_at` rule
                // stay in exactly one place, which is the property that matters more than the
                // loop count -- a second copy of the address arithmetic is a second answer
                // waiting to disagree with the first.
                for (si, &r) in radius.iter().enumerate() {
                    let wx = cx + (dx * r).round() as i32;
                    let wz = cz + (dz * r).round() as i32;
                    let hi = height_at(gen, heights, origin, wx, wz);
                    col_h[si] = hi as f32;
                    // The bounce's colour comes from the block this column shows the sky,
                    // which over water is water and not the sea floor -- `surface_block_at`
                    // is where that is decided, and it is the generator's own rule rather
                    // than a second copy of it.
                    col_a[si] = if si % ALBEDO_STRIDE == 0 {
                        block::ALBEDO[gen.surface_block_at(wx, wz, hi) as usize]
                    } else {
                        [0.0; 3]
                    };
                }
                // Batch 60, roadmap R3. Off, this is `[1.0; STEPS]` and every product below is
                // unchanged to the bit, which is what makes `--no-probe-shadow` a revert of the
                // *bake* rather than a second value stored beside the first.
                let lit = if bounce_shadow {
                    ray_exposure(&col_h, &radius)
                } else {
                    [1.0f32; STEPS]
                };
                for (si, &r) in radius.iter().enumerate() {
                    let h = col_h[si];
                    let gathering = si % ALBEDO_STRIDE == 0;
                    // **The exposure multiplies the numerator and not the weight.** `alb_w`
                    // stays the unlit sum, so what the mean below carries is
                    // `albedo * exposure` against `1` rather than an exposure-weighted average
                    // of albedo -- the first darkens a shadowed valley, the second would only
                    // re-colour it. Fully open ground is `lit = 1` and reduces to batch 58's
                    // own expression exactly.
                    let a = [
                        col_a[si][0] * lit[si],
                        col_a[si][1] * lit[si],
                        col_a[si][2] * lit[si],
                    ];
                    // The occluding silhouette is the *top* of the column, which is the y of
                    // the first air block: `h - 1` is the top solid voxel and it occupies
                    // [h-1, h). The same off-by-one the snow and tree lines carry, and
                    // `docs/pitfalls.md` records clippy wanting to collapse it.
                    for py in 0..PER_AXIS {
                        let y = (origin.y + SPACING / 2 + SPACING * py as i32) as f32;
                        let t = (h - y) / r;
                        if t > tan_h[py][k] {
                            tan_h[py][k] = t;
                        }
                        if !gathering {
                            continue;
                        }
                        // The cosine-weighted solid angle an annulus of ground at radius `r`
                        // and drop `d` subtends at the probe. Derived rather than authored:
                        // the annulus is `2*pi*r*dr` at distance `sqrt(r^2+d^2)`, tilted by
                        // `cos = d/sqrt(r^2+d^2)` at both ends -- once because the ground is
                        // foreshortened and once because the receiver is -- which is
                        // `r*dr*d^2 / (r^2+d^2)^2`, and the radii are geometric so `dr` is
                        // proportional to `r`. Every constant factor divides out in the mean
                        // below, so only the shape survives.
                        //
                        // **It peaks at `r = d` and that is the whole of what makes this
                        // directional rather than a global average**: a probe four blocks
                        // above the ground is coloured by the ground four blocks away, and
                        // one sixty blocks up by the ground sixty blocks away.
                        //
                        // `abs` and a one-block floor: a probe exactly at the surface would
                        // otherwise give every sample weight zero, and a *buried* one has a
                        // ground above it that is just as much of an occluder. What saves the
                        // buried case from meaning anything is `believe`, which is zero there.
                        let d = (y - h).abs().max(1.0);
                        let dr = d * r;
                        let s = d * d + r * r;
                        let w = (dr * dr) / (s * s);
                        alb[py][k][0] += w * a[0];
                        alb[py][k][1] += w * a[1];
                        alb[py][k][2] += w * a[2];
                        alb_w[py][k] += w;
                    }
                }
            }

            for py in 0..PER_AXIS {
                let (cube, believe) = cube_from_horizon(&tan_h[py], &lobe, &lobe_sum);
                let b = bounce_from_albedo(&alb[py], &alb_w[py], &blobe, &scale, believe);
                let i = (pz * PER_AXIS + py) * PER_AXIS + px;
                for axis in 0..AXES {
                    occl[axis * PROBES + i] = cube[axis];
                    let o = (axis * PROBES + i) * 3;
                    bounce[o] = b[axis][0];
                    bounce[o + 1] = b[axis][1];
                    bounce[o + 2] = b[axis][2];
                }
            }
        }
    }

    ProbeData { occl, bounce }
}

/// What the gathered albedo is multiplied by on its way into the field.
///
/// Two jobs in one vector, and they are separated in [`BOUNCE_REF`]'s note: divide by the
/// reference ground's luminance, so the floor's brightness over that ground is unchanged; and
/// divide by [`FLOOR_TINT`], so what is stored is a multiplier on the shader's own constant
/// rather than a replacement for it -- which is what keeps the identity at 1.0 and every
/// absence of the feature bit-exact.
fn bounce_scale() -> [f32; 3] {
    let gain = block::luma(block::ALBEDO[BOUNCE_REF as usize]);
    [
        block::luma(FLOOR_TINT) / (gain * FLOOR_TINT[0]),
        block::luma(FLOOR_TINT) / (gain * FLOOR_TINT[1]),
        block::luma(FLOOR_TINT) / (gain * FLOOR_TINT[2]),
    ]
}

/// One probe's bounce over an idealised flat field of a single block's albedo.
///
/// **[`bake`] hoists the lobes and the scale out of its loops and calls the same inner
/// function**, so this is a thin entry point and not a second implementation -- exactly what
/// [`cube_from_tangents`] is for the occlusion half, and for the same reason: the number that
/// has to come back at exactly the reference luminance cannot be checked against a
/// reimplementation of the thing being checked. `tests/probe.rs` is the caller.
///
/// `believe` is 1, because what this asks is what the bake does where it has an opinion.
pub fn bounce_of_uniform_ground(albedo: [f32; 3]) -> [[f32; 3]; AXES] {
    let (_, lobe, _) = tables();
    let blobe = bounce_lobes(&lobe);
    // Uniform ground is uniform at every radius, so one unit of weight per azimuth is the
    // whole of it: the gather normalises by its own weight and the form factor divides out.
    let alb = [albedo; AZIMUTHS];
    let w = [1.0f32; AZIMUTHS];
    bounce_from_albedo(&alb, &w, &blobe, &bounce_scale(), 1.0)
}

/// The azimuthal weights the **bounce** gather uses, which are the horizon's own for the four
/// sideways axes and flat for the two vertical ones.
///
/// **A horizontal face has no azimuthal preference and that is the only difference.** `tables`
/// leaves axes 2 and 3 with an all-zero lobe because a vertical normal has no azimuth to
/// point along, which is right for the sky integral -- the up axis is closed form in the
/// elevation alone and the down axis sees no sky at all. The bounce cannot use that: the down
/// axis is the one face in the world that receives *nothing but* bounce, and its lobe has to
/// admit every azimuth rather than none.
///
/// **What this field's per-axis variation therefore is, is azimuthal.** A +X face is coloured
/// by the ground on its +X side and a -X face by the ground on its -X side, which is the
/// directional half of one bounce and what six slabs are for. A +Y face and a -Y face at the
/// same probe read the same value, because the same ground surrounds them; the elevation half
/// -- that a ceiling sees ground where a floor sees sky -- is a form factor this rung does not
/// carry and the next one can.
fn bounce_lobes(lobe: &[[f32; AZIMUTHS]; AXES]) -> [[f32; AZIMUTHS]; AXES] {
    let mut out = *lobe;
    out[2] = [1.0; AZIMUTHS];
    out[3] = [1.0; AZIMUTHS];
    out
}

/// Turn one probe's gathered ground albedo into six per-channel multipliers on `floor_rgb`.
///
/// **1.0 is the identity and every absence of the feature reaches it**: an unbaked texel, a
/// coarse LOD, a pinned `--probe-fill 1.0`, a probe with no ground in its lobe and a probe
/// with no sky over it all return exactly 1.0, and `resolve` multiplies `floor_rgb` by it, so
/// the pre-batch-58 frame comes back *to the bit* rather than close to it. That is batch 57's
/// own argument for storing occlusion rather than shading, one rung on.
///
/// The mean is weighted by the azimuthal lobe times the radial form factor already folded
/// into `alb`, and then rescaled so that [`BOUNCE_REF`] ground returns 1.0 in luminance --
/// `scale` carries that and the division by [`FLOOR_TINT`] together, so the value stored is a
/// multiplier on the shader's constant and not a replacement for it.
///
/// **`believe` is batch 57's weight and is reused rather than recomputed.** Where a probe sees
/// no sky there is no sky-lit bounce either, and the authored floor -- whose comment calls it
/// "the sole light in an unlit cave" -- is still the best answer. One weight for both fields
/// is also what keeps a cave from acquiring a green cast off the grass above it.
fn bounce_from_albedo(
    alb: &[[f32; 3]; AZIMUTHS],
    alb_w: &[f32; AZIMUTHS],
    blobe: &[[f32; AZIMUTHS]; AXES],
    scale: &[f32; 3],
    believe: f32,
) -> [[f32; 3]; AXES] {
    let mut out = [[1.0f32; 3]; AXES];
    for (axis, o) in out.iter_mut().enumerate() {
        let mut num = [0.0f32; 3];
        let mut den = 0.0f32;
        for k in 0..AZIMUTHS {
            let w = blobe[axis][k];
            if w <= 0.0 {
                continue;
            }
            num[0] += w * alb[k][0];
            num[1] += w * alb[k][1];
            num[2] += w * alb[k][2];
            den += w * alb_w[k];
        }
        if den <= 0.0 {
            continue;
        }
        let inv = 1.0 / den;
        for c in 0..3 {
            o[c] = 1.0 + (num[c] * inv * scale[c] - 1.0) * believe;
        }
    }
    out
}

/// How much sky each *gathered ground sample* along one ray can see, from the samples the
/// horizon march has already taken.
///
/// **This is the term that makes the bounce a second bounce rather than a first.** Batch 58
/// gathers the albedo of the ground a probe can see and weights it by a form factor; what it
/// does not ask is whether that ground is itself lit. A valley floor and an open field of the
/// same grass bounce the same amount under that rule, and they do not: light has to reach the
/// ground before the ground can send any of it on.
///
/// **It costs no new height samples, which is the whole reason it is affordable.** The march
/// walks `STEPS` columns outward along each azimuth and already knows every one of their
/// heights; the sky a sample at step `si` can see is decided by the same columns, read as a
/// maximum of slopes rather than of heights. So this is a second pass over an array that is
/// already in cache, and the chunk's 20,480 height samples stay 20,480.
///
/// **What it can see is one azimuth and its reverse**, because that is the only direction the
/// ray carries data about: a ravine running *along* the ray reads as open. The estimate is
/// therefore a lower bound on occlusion and an upper bound on exposure, and it is biased the
/// safe way -- towards batch 58's answer, which is the one this has to reduce to.
///
/// The per-direction factor is `1 / (1 + t^2)`, which is [`cube_from_horizon`]'s **up** axis
/// exactly and is quoted from it rather than re-derived: `sin^2(atan(t))` is `t^2/(1+t^2)`, so
/// a cosine-weighted hemisphere cut off at elevation `atan(t)` passes `1/(1+t^2)` of an open
/// one. Flat ground gives `t = 0` in both directions and therefore **exactly 1.0**, which is
/// what makes the whole feature a pure revert over open country rather than a close one.
fn ray_exposure(col_h: &[f32; STEPS], radius: &[f32; STEPS]) -> [f32; STEPS] {
    let mut out = [1.0f32; STEPS];
    for si in 0..STEPS {
        // Only the gathering steps are ever read, and the rest cost a fill of 1.0 above.
        if si % ALBEDO_STRIDE != 0 {
            continue;
        }
        let h = col_h[si];
        // Outward and inward are kept apart rather than maxed together, because two ridges on
        // opposite sides cut two different parts of the hemisphere: taking one maximum would
        // charge the taller one for both halves and call a notch a pit.
        let (mut t_out, mut t_in) = (0.0f32, 0.0f32);
        for sj in 0..STEPS {
            if sj == si {
                continue;
            }
            // A one-block floor on the separation, for the reason the form factor has one: the
            // innermost radii are a fraction of a block apart, and a neighbouring column half a
            // block away would otherwise read as a vertical wall.
            let d = (radius[sj] - radius[si]).abs().max(1.0);
            let t = (col_h[sj] - h) / d;
            if sj > si {
                if t > t_out {
                    t_out = t;
                }
            } else if t > t_in {
                t_in = t;
            }
        }
        // Negative slopes never enter: ground *below* the sample takes no sky away from it, and
        // both maxima start at 0 so a sample on a crest keeps the full 1.0.
        out[si] = 0.5 * (1.0 / (1.0 + t_out * t_out) + 1.0 / (1.0 + t_in * t_in));
    }
    out
}

/// The column height for a world column, from the chunk's own array where that covers it.
#[inline]
fn height_at(gen: &WorldGen, heights: &[i32; 64 * 64], origin: IVec3, wx: i32, wz: i32) -> i32 {
    let lx = wx - origin.x;
    let lz = wz - origin.z;
    if (0..64).contains(&lx) && (0..64).contains(&lz) {
        heights[lx as usize + lz as usize * 64]
    } else {
        gen.height(wx, wz)
    }
}

/// Integrate one probe's horizon into six occlusion factors.
///
/// Each is the cosine-weighted fraction of the hemisphere about its axis that still reaches
/// sky **relative to an open field** -- so 1 wherever the terrain is flat and open, and the
/// module note is why that normalisation rather than the absolute visibility. With the horizon
/// given as one elevation per azimuth both integrals are closed form in the elevation and a
/// plain sum over the azimuths:
///
/// - **up**: `(1/N) * sum(1 - sin^2(theta))`, and `sin^2(atan(t))` is `t^2/(1+t^2)`, so the
///   whole term is `1/(1+t^2)` and no trigonometry is evaluated at all. It is already relative:
///   an unoccluded hemisphere sums to exactly 1.
/// - **sideways**: `sum(w * I(theta)) / sum(w * I(0))`, where
///   `I(theta) = pi/4 - theta/2 - sin(2 theta)/4` is the elevation integral of `cos^2` from the
///   horizon to the zenith and `w` is the azimuthal cosine lobe. **Dividing by the *discrete*
///   `sum(w * I(0))` rather than by its analytic value is what makes an open field read exactly
///   1** instead of the 0.987 sixteen azimuths would otherwise land on -- a 1.3% darkening of
///   every side face in the world, uniform, and therefore indistinguishable in a screenshot
///   from the feature working.
/// - **down**: 1, unoccluded. No sky lies below the horizon at all, so terrain cannot take any
///   away; what a downward face actually receives is bounce, and `floor_rgb` stands in for it
///   until roadmap R2 computes one.
fn cube_from_horizon(
    tan_h: &[f32; AZIMUTHS],
    lobe: &[[f32; AZIMUTHS]; AXES],
    lobe_sum: &[f32; AXES],
) -> ([f32; AXES], f32) {
    const QUARTER_PI: f32 = std::f32::consts::FRAC_PI_4;

    let mut up = 0.0f32;
    let mut elev = [0.0f32; AZIMUTHS];
    for (k, &t) in tan_h.iter().enumerate() {
        let t = t.max(0.0);
        up += 1.0 / (1.0 + t * t);
        elev[k] = QUARTER_PI - 0.5 * t.atan() - 0.5 * t / (1.0 + t * t);
    }
    up /= AZIMUTHS as f32;

    // Where there is no sky the cube has no opinion, so the occlusion fades back to 1 and
    // `face_shade` alone remains the answer. One weight for all six axes: it is a statement
    // about the probe, not about a face.
    //
    // **Smoothstep and not a linear ramp, and the sweep is what asked for it.** A probe well
    // under the terrain still sees a fraction of a percent of the sky, which a linear ramp
    // turns into a visible-at-8-bit darkening: `cave` moved **369,260 pixels of 921,600 at max
    // delta 1** under one, a whole vantage of noise from a feature with nothing to say there.
    // The cubic's flat tail takes it to **30,300**, still at max delta 1 and now confined to
    // the shallow part of the pocket where the probes genuinely do see sky. It is also the
    // better shape to read through a trilinear tap, which interpolates across the kink a
    // linear ramp has.
    //
    // **It is not bit-exact and an earlier draft of this comment claimed it was.** What made
    // that claim survive a re-measurement is the trap now at the top of `docs/pitfalls.md`:
    // `cargo run --bin harness` does not rebuild `voxelcraft.exe`, so the sweep that was
    // supposed to check the ramp re-ran the binary from before it.
    let t = (up / SKY_BLEND).clamp(0.0, 1.0);
    let believe = t * t * (3.0 - 2.0 * t);

    let mut out = [0.0f32; AXES];
    for (axis, o) in out.iter_mut().enumerate() {
        let occl = match axis {
            3 => up,
            2 => 1.0,
            _ => {
                let mut num = 0.0f32;
                for k in 0..AZIMUTHS {
                    num += lobe[axis][k] * elev[k];
                }
                num / (lobe_sum[axis] * QUARTER_PI)
            }
        };
        *o = 1.0 + (occl - 1.0) * believe;
    }
    // `believe` is returned rather than recomputed by the bounce gather beside this one: the
    // two fields answer different questions and have to agree about where the sky stops, and
    // a second `up` integral is a second answer waiting to disagree with the first.
    (out, believe)
}



