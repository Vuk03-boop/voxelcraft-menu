//! GPU-driven voxel ray tracer.
//!
//! Per frame: select (tile, chunk) pairs against chunk bounding boxes, ray march the
//! survivors into a 64-bit visibility buffer, then shade only the pixels that survived.

pub mod blit;
pub mod font;
pub mod pools;
pub mod timer;
pub mod ui;
mod shadow;

use crate::biome;
use crate::block;
use crate::lod::RenderItem;
use crate::probe::{self, ProbeData};
use crate::math::{Aabb, Frustum};
use crate::textures;
use crate::voxel::{AttrRef, ChunkKey, LightRef, World};
use bytemuck::{Pod, Zeroable};
use glam::{IVec3, Mat4, Vec2, Vec3};
use pools::{DynBuffer, GpuMirror};
use timer::{q, Readback};

pub const TILE: u32 = 8;
pub const MAX_CHUNKS: usize = 8191;
/// The (tile, chunk) pair list's **floor**, and until batch 53 its fixed size.
///
/// Kept as the floor rather than deleted because it is the capacity every measurement in this
/// project was taken at: at the fixture's 1280x720 and at `PERF.md`'s 1920x1080 the
/// resolution-scaled budget below comes out under this, so both render with exactly the
/// capacity they always had and nothing in the ledger moves.
pub const MAX_PAIRS: u32 = 1 << 20;

/// Pairs budgeted per 8x8 tile when the screen asks for more than [`MAX_PAIRS`].
///
/// **Sixteen against a worst *observed* 10.1**, measured at `default`: 6.6 select plus 3.5
/// deferred per tile at 5120x2880, against 2.4 and 7.9 at 1920x1080 -- the split moves with
/// the camera and the total does not, which is what makes one budget for both lists safe.
/// It is not a bound: the true worst case is `MAX_CHUNKS` pairs for one tile and no buffer can
/// be sized for that, which is why `warn_if_truncated` stays the backstop it has been since
/// batch 22.
const PAIRS_PER_TILE: u64 = 16;

/// The pair list's capacity for a screen of `tiles` 8x8 tiles.
///
/// **Batch 53, and the defect it fixes was mis-stated in two documents.** The roadmap and
/// `gpu.md` both said the fixed 1 << 20 was "reachable at 8K". Measured at `default`:
/// **1,529,818 pairs at 5120x2880**, 46% over the cap and truncating, and **80.5% of it at
/// plain 3840x2160** -- so a 4K capture was one busier camera or one `--scale 2` from losing
/// geometry, and `--screenshot` offers both. The list is screen-sized, so it is sized from the
/// screen, exactly as `vis`, `hiz` and `dbg` already are.
pub fn pairs_for(tiles: (u32, u32)) -> u32 {
    let want = u64::from(tiles.0) * u64::from(tiles.1) * PAIRS_PER_TILE;
    want.max(u64::from(MAX_PAIRS)).min(u64::from(u32::MAX) / 2) as u32
}

/// Bits the pair word spends on the tile index: 32 less the 13 the chunk id takes.
///
/// **A separate limit from the pair count, narrower than it looks, and unguarded until batch
/// 53.** `tile_select` packs `(tile_id << 13) | chunk_id` into a `u32`, so a screen may hold
/// 2^19 tiles and no more -- 33,554,432 pixels. 7680x4320 is 518,400 tiles, **98.9% of it**,
/// and 8192x4320 is over: the shift would drop the high bits and every overflowing tile would
/// march somebody else's chunk. That is a plausible picture rather than a crash, which is the
/// failure mode this project refuses to leave silent. [`Renderer::warn_if_oversized`] is the
/// guard, and `--scale 2` is the flag most likely to reach it, since it multiplies the
/// *internal* resolution the pair list is sized from.
pub const TILE_ID_BITS: u32 = 32 - 13;
pub const MAX_TILES: u32 = 1 << TILE_ID_BITS;

/// `march_chunk`'s loop guard, mirrored from `common.wgsl` so `MarchStats` can say when a ray
/// hit it. `march_iters_cap_matches_the_shader` is the only thing tying the two copies.
///
/// **A ray that reaches this returns `hit = false`, which is a hole in the world and not a
/// slow pixel** -- so the count of pixels at the cap is a correctness reading as much as a
/// cost one, and until batch 53 nothing anywhere could produce it.
pub const MARCH_MAX_ITERS: u32 = 512;

/// The phrase [`Renderer::warn_if_truncated`] prints, so the fixture can refuse to score a
/// truncated capture instead of relying on whoever reads the log to notice.
pub const TRUNCATION_MARK: &str = "pair list truncated";

/// The phrase [`Renderer::warn_if_oversized`] prints. Separate from [`TRUNCATION_MARK`]
/// because the two are different defects with different fixes: that one loses geometry and
/// says which, this one renders the wrong geometry and cannot.
pub const OVERSIZE_MARK: &str = "tile index overflows the pair word";

/// What `tile_select` asked the pair buffer for, against what it holds.
///
/// Raw counts, taken before `min(n, frame.max_pairs)`. Which subset of an overflowing list
/// survives is a function of the workgroup shape, so an overflowing frame is not merely
/// missing geometry -- it is missing geometry that a reshape of `tile_select` would change,
/// which is the thing that would quietly invalidate a bit-exact A/B.
#[derive(Clone, Copy, Debug)]
pub struct PairDemand {
    /// Pairs `tile_select` emitted, before the clamp.
    pub select: u32,
    /// Pairs it deferred to the recovery pass, before the clamp.
    pub deferred: u32,
    /// Pairs the recovery pass re-admitted, before the clamp.
    pub recover: u32,
    pub cap: u32,
}

impl PairDemand {
    pub fn overflowed(&self) -> bool {
        self.select > self.cap || self.deferred > self.cap || self.recover > self.cap
    }

    pub fn worst(&self) -> u32 {
        self.select.max(self.deferred).max(self.recover)
    }
}
/// What one frame's `march` cost in work rather than in milliseconds (batch 53).
///
/// `iters` is the sum over every pixel of the loop counter `march_chunk` returns, so it counts
/// **DDA steps across every (tile, chunk) pair that reached the marcher**, not rays. A pixel
/// covered by four chunks contributes all four walks. That is the quantity a traversal change
/// moves, and it is why this is the instrument roadmap P4 wanted rather than a ray count.
#[derive(Clone, Copy, Debug)]
pub struct MarchStats {
    pub pixels: u64,
    pub iters: u64,
    /// The most any single pixel paid -- the number that says whether `MAX_ITERS` is near.
    pub worst: u32,
    /// Pixels a marcher reached at all, which separates *fewer steps* from *fewer pixels*.
    pub touched: u64,
    /// Pixels whose walks ran past [`MARCH_MAX_ITERS`]. Every one of those is a ray that gave
    /// up, so this is the guard rail's own reading and there has never been one before.
    pub capped: u64,
    /// Share of all steps spent in the hottest 1% and 10% of pixels. **The row that matters**:
    /// a mean cannot show a distribution this skewed, and every heatmap in the set puts the
    /// cost in a thin band of rays grazing the water.
    pub top_1: f64,
    pub top_10: f64,
    pub p50: u32,
    pub p90: u32,
    pub p99: u32,
    pub pairs: Option<PairDemand>,
    pub chunks: usize,
}

pub const GRID_X: u32 = 32;
pub const GRID_Y: u32 = 8;
pub const GRID_Z: u32 = 32;
const GRID_LEN: usize = (GRID_X * GRID_Y * GRID_Z) as usize;
const NO_CHUNK: u32 = 0xFFFF_FFFF;

/// Where the grid's lattice starts, in LOD-0 cells, for a camera at `cam`.
///
/// One definition, because `build_chunk_list` needs it *before* the partition -- what bounds
/// the shadow-only tail is the lattice -- and `GpuFrame` needs it after.
pub fn grid_origin(cam: Vec3) -> IVec3 {
    let cc = (cam / 64.0).floor().as_ivec3();
    IVec3::new(cc.x - GRID_X as i32 / 2, 0, cc.z - GRID_Z as i32 / 2)
}

/// The grid cells one chunk covers, clamped to the lattice, or `None` when it covers none.
///
/// **One definition, two readers, and that is the point.** `partition_for_grid` uses it to
/// decide whether an off-screen chunk is worth uploading at all and `build_chunk_list`'s fill
/// loop uses it to write the cells; if the two ever disagreed, the disagreement that costs
/// something is the one where the filter drops a chunk the fill would have written -- an
/// occluder silently missing from the grid, which is the defect batch 64 exists to remove,
/// reintroduced one level down and invisible in exactly the same way.
pub fn grid_span(key: ChunkKey, gmin: IVec3) -> Option<(IVec3, IVec3)> {
    let span = 1i32 << key.lod;
    let base = key.pos * span - gmin;
    let lo = base.max(IVec3::ZERO);
    let hi = (base + IVec3::splat(span)).min(IVec3::new(
        GRID_X as i32,
        GRID_Y as i32,
        GRID_Z as i32,
    ));
    if lo.cmplt(hi).all() {
        Some((lo, hi))
    } else {
        None
    }
}

/// Split a draw plan into the entries `tile_select` box-tests and the entries that are in the
/// frame for their **shadows alone**, both nearest-first.
///
/// **The frustum decides the first list and must not decide the second, and that is roadmap
/// D3.** Until batch 64 there was one list: a chunk the frustum rejected was not uploaded, so
/// it was not in `grid`, so it occluded nothing -- and `grid` is what every shadow ray and
/// every cross-chunk light lookup reads. Turn until a mountain leaves the frustum and its
/// shadow leaves the ground it was falling on, which is on ground still in frame. The user
/// reported it as a shadow that *"becomes shorter or larger like a weird pop in pop out"* as
/// they turned, and it is the only kind of error a fixture of single stills cannot see: each
/// frame is internally consistent and only the disagreement between two of them is wrong.
///
/// **The two lists are concatenated rather than merged, and the order is what makes that
/// free.** `frame.chunk_count` is the *visible* length, and `tile_select` -- the only reader
/// of that field -- loops `0 .. chunk_count`, so an entry in the tail is unreachable from the
/// pass whose cost scales with the list. The tail is reachable only through `grid`, which is
/// indexed and carries no count. So the marcher's per-tile box test sees exactly the set it
/// saw before this batch and costs exactly what it cost before it.
///
/// The tail is bounded by the grid rather than by a distance of its own: a chunk covering no
/// cell of the lattice can never be named by `grid`, so uploading it would be uploading
/// something nothing can read. That is also why no new constant appears here -- `GRID_X/Y/Z`
/// already describe a 2048 x 512 x 2048 block box around the camera, far wider than the
/// frustum and wider than any ray that reads it.
///
/// **Public for `tests/shadow_grid.rs`**, which is the one place the batch's claim can be
/// stated as an equality rather than as a pixel count: the grid-eligible set is the same at
/// every yaw from one position, and the drawn set is not. That test needs no device, because
/// this takes boxes rather than a `World` -- which is also why it takes boxes.
pub fn partition_for_grid(
    items: &[(RenderItem, Aabb)],
    cam: Vec3,
    frustum: &Frustum,
    gmin: IVec3,
    offscreen_shadows: bool,
) -> (Vec<RenderItem>, Vec<RenderItem>) {
    let mut visible: Vec<(f32, RenderItem)> = Vec::with_capacity(items.len());
    let mut offscreen: Vec<(f32, RenderItem)> = Vec::new();
    for &(item, aabb) in items {
        if frustum.contains_aabb(&aabb) {
            visible.push((aabb.distance_to(cam), item));
        } else if offscreen_shadows && grid_span(item.key, gmin).is_some() {
            offscreen.push((aabb.distance_to(cam), item));
        }
    }
    // Nearest first: the camera's chunk lands early and the 13-bit id stays in range.
    let by_distance = |a: &(f32, RenderItem), b: &(f32, RenderItem)| {
        a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal)
    };
    visible.sort_by(by_distance);
    visible.truncate(MAX_CHUNKS);
    // The budget the drawn set does not want. Nearest first here too, because what a shadow
    // ray can reach is bounded by `shadow_dist` and the near occluders are the ones inside it.
    offscreen.sort_by(by_distance);
    offscreen.truncate(MAX_CHUNKS - visible.len());
    (
        visible.into_iter().map(|(_, it)| it).collect(),
        offscreen.into_iter().map(|(_, it)| it).collect(),
    )
}

/// Only the clip-z range of `prev_view_proj` depends on this; the marcher has no near plane.
const NEAR: f32 = 0.05;

/// Fraction of a new frame the temporal accumulator keeps. Ten frames of effective
/// history: long enough for the eight jitter phases to average out, short enough that a
/// sample the variance clip lets through does not linger.
const TAA_FEEDBACK: f32 = 0.15;

pub const FLAG_HIZ: u32 = 1;
pub const FLAG_HEATMAP: u32 = 2;
pub const FLAG_SHADOWS: u32 = 4;
pub const FLAG_AO: u32 = 8;
/// `common.wgsl` carries the water block id as a literal, since a shader cannot see this
/// crate. This is the only thing tying the two together.
const _: () = assert!(block::WATER == 13);
/// The same for batch 14's ground cover, which `common.wgsl` also carries as a literal.
const _: () = assert!(block::TALL_GRASS == 18);
/// Batch 101e appends two more ids `common.wgsl` knows by number (the `is_tuft` contract),
/// under the same copy-in-two-files argument.
const _: () = assert!(block::GRASS_TALL == 24);
const _: () = assert!(block::REEDS == 25);
// Batch 38. `common.wgsl` tells a carved cube from a whole one with two id compares, because
// a shader cannot call `block::is_cutout`. These two asserts and the one below are the whole
// of what ties the copies together -- and the second half matters more than the first: a block
// whose `cutout` flag the shader does not know about is drawn as a solid cube, silently.
const _: () = assert!(block::LEAVES == 7);
const _: () = assert!(block::PINE_LEAVES == 16);
const _: () = assert!(block::BLOCKS[block::LEAVES as usize].cutout);
const _: () = assert!(block::BLOCKS[block::PINE_LEAVES as usize].cutout);
/// And the stride of the face table, which `resolve.wgsl` and `common.wgsl` carry as the
/// literal `8u` at three call sites. One of the three had it as `6u` from batch 8 to batch
/// 21b, which made the sea sample the glass layer -- see `docs/water.md`.
/// This assert is cheap and the bug cost eight batches of wrong water.
const _: () = assert!(block::FACES_PER_BLOCK == 8);

pub const FLAG_WATER_REFLECT: u32 = 16;
pub const FLAG_WATER_REFRACT: u32 = 32;
/// The eye is inside a water voxel. Frame-wide, because the medium a ray starts in is a
/// property of the camera and not of the ray: it turns water non-blocking for the primary
/// march and swaps the aerial haze for the underwater one.
pub const FLAG_UNDERWATER: u32 = 64;

/// Tint the vegetation by the biome under it. Off is the batch-12 control and is bit-exact,
/// and it is a *flag* rather than a strength of zero on purpose: the biome field is two
/// octaves of simplex per tinted pixel, and batch 10's lesson is that a control only earns
/// "free" by being unreachable rather than by evaluating to nothing.
pub const FLAG_TINT: u32 = 128;

/// The generator placed ground cover. A property of the *world* and not of one ray, which
/// is what makes it foldable at all: `skip_foliage` says whether a given ray traces blades,
/// and a shadow ray passing `true` still has to recognise a tuft in order to step past one.
/// Only "there is none anywhere" takes `cross_quad` out of the module.
///
/// Set from `worldgen::has_foliage`, the generator's own answer, rather than from a second
/// copy of `foliage && biomes` -- because the two must agree. A flag claiming foliage the
/// generator never placed costs only the time this batch exists to recover; a flag denying
/// foliage that *is* there compiles away the marcher's ability to recognise a tuft and
/// draws every one of them as an opaque cube.
///
/// Note the shaders never test this. `SPEC_FOLIAGE` is not ANDed with it the way `SPEC_TINT`
/// is ANDed with `FLAG_TINT`, because the extra term costs `resolve` the whole fold -- the
/// override's own comment in `common.wgsl` has the two numbers. This word is a pipeline key
/// and nothing else.
pub const FLAG_FOLIAGE: u32 = 256;

/// What lets `SPEC_FOLIAGE` skip that run-time test: the generator is the *only* source of
/// ground cover, so a run has tufts in its world or has none and there is no third state.
/// Putting `TALL_GRASS` in the hotbar would create one -- a `--no-foliage` run whose player
/// places a tuft the marcher has been compiled unable to see -- and this is what stops that
/// arriving quietly.
const _: () = {
    let mut i = 0;
    while i < block::HOTBAR.len() {
        assert!(block::HOTBAR[i] != block::TALL_GRASS);
        i += 1;
    }
};

/// The sea carries micro-normals (batch 16). A *flag* and not `Waves::amp == 0` for batch
/// 14's reason rather than batch 12's: the wave field is four cosine pairs and a Toksvig
/// variance term compiled into `resolve`, which is the pass that bills code for existing.
/// The strength still travels as a float and the two are set together in `flags_from`, so
/// what the bit buys is the *code* being gone rather than one branch being skipped.
///
/// Like `FLAG_FOLIAGE` and unlike `FLAG_TINT` this is a property of the run and not of a
/// ray: it is read once off the parsed config and never toggled, which is what lets it sit
/// in `SPEC_MASK`.
pub const FLAG_WAVES: u32 = 512;

/// The wave band limit reads the grazing angle (batch 19). Off, `wave_octave` filters each
/// octave against the pixel's width *across* the view ray, which is the footprint on a
/// surface facing the camera and not the footprint on the sea. The water plane is
/// horizontal, so a view ray at `g` degrees above it crosses `1 / sin(g)` times as much of
/// the field per pixel, and at the vantage this batch was measured from that factor reaches
/// **15.9x** by the horizon -- the field was being asked to resolve detail eight times finer
/// than the pixel it lands in, and answered with per-pixel speckle.
///
/// Only ever set alongside `FLAG_WAVES`, which is what lets the two overrides be independent
/// without a key that means nothing. `--no-wave-aniso` clears it and reproduces the
/// pre-batch-19 sea bit for bit; being in `SPEC_MASK`, it does so for free.
pub const FLAG_WAVE_ANISO: u32 = 1024;

/// The wave amplitude ramps with water depth (batch 20). Off, a two-block lagoon over sand
/// carries the same open-ocean swell as a point 900 blocks out, which is what reads as "too
/// much" from a shore-level camera -- the near field is the one place nothing has yet been
/// filtered away.
///
/// It is depth and deliberately **not** distance from the camera: an amplitude that fell off
/// with range would calm as you approached and build as you retreated, and `taa` would fight
/// it the whole way. Depth is a property of the world, so a given patch of sea looks the same
/// from everywhere -- which is the only version of this that can be A/B'd at all.
///
/// Only ever set alongside `FLAG_WAVES`. `--no-wave-shoal` clears it and reproduces the
/// pre-batch-20 sea bit for bit.
pub const FLAG_WAVE_SHOAL: u32 = 2048;

/// The wave field subdivides its band into eight octaves instead of four (batch 25). Off,
/// `wave_field` is the four cosines batches 16-20 measured, whose peak autocorrelation over
/// the pinned lag annulus is **0.999** -- a field that repeats almost exactly somewhere
/// inside 64 blocks, which is the "plaid" batch 20 diagnosed and could not fix by
/// re-spreading four directions.
///
/// What the batch changes is the *spacing inside the band the band limit passes*, and not
/// the count for its own sake: octaves added below 3.14 blocks land where batch 19's filter
/// has already faded them to zero, so they cost a cosine each and move the repeat by 0.016.
/// Subdividing 32..3.14 instead moves it to 0.79. The amplitudes stay on the shipped
/// `lambda^0.536` law and are renormalised to the shipped total slope variance, so the sea
/// is exactly as rough as it was and `--wave-amp` and `--wave-scale` keep their meanings.
///
/// Only ever set alongside `FLAG_WAVES`. `--no-wave-fill` clears it and reproduces the
/// pre-batch-25 sea bit for bit.
pub const FLAG_WAVE_FILL: u32 = 4096;

/// Terrain-cast crepuscular rays (batch 35): the shaft integral reads the light envelope as a
/// second occluder beside the cloud deck. `--no-terrain-shafts` clears it, and being in
/// [`SPEC_MASK`] it does so for free -- the tap, the bilinear and the four loads leave
/// `resolve` entirely rather than being branched past.
///
/// **It stopped reproducing the pre-batch-35 frame on its own at batch 37**, which gave the
/// envelope a second reader in [`FLAG_DISTANT_SHADOWS`]: this flag now speaks for the haze's
/// reader alone, and the revert is the pair. That is the same thing this flag did to
/// `--cloud-shadow 0` one batch earlier, and the reason both corrections are recorded at the
/// flag rather than only in a document is that a control's prose is what a later batch reads.
///
/// **It also changes what `--cloud-cover 0` and `--cloud-shadow 0` mean**, which is the one
/// consequence worth reading before benching anything here. Both used to skip the shaft
/// integral outright, because the deck was its only occluder; a deck-less frame still has
/// terrain in it, so neither is a pre-batch-11 revert on its own any more. `--cloud-shadow 0
/// --no-terrain-shafts` is.
pub const FLAG_TERRAIN_SHAFTS: u32 = 8192;

/// Per-block texture permutation (batch 36): a face's texture coordinates are turned into one
/// of its layer's `block::PERM_CLASS` states, keyed on the voxel's integer world position.
/// `--no-tex-variation` clears it and reproduces the pre-batch-36 frame bit for bit; being in
/// [`SPEC_MASK`] it does so for free.
///
/// Unconditional on the world, the way `FLAG_TERRAIN_SHAFTS` is and `FLAG_WAVES` is not:
/// there is no configuration that removes the ground, so there is no "the field could never
/// be reached" case to nest it under. `--no-biomes` changes which blocks the generator places
/// and cannot remove `GRASS_SIDE` from the world.
pub const FLAG_TEX_VARIATION: u32 = 16384;

/// Distant terrain shadows (batch 37): `shade_hit`'s sun term reads the light envelope past
/// the end of its own `shadow_dist` march. `--no-distant-shadows` clears it and reproduces
/// the pre-batch-37 frame bit for bit; being in [`SPEC_MASK`] it does so for free.
///
/// **It is what `--no-terrain-shafts` stopped being able to say on its own**, and that is the
/// one consequence to read before benching here. Batch 35's control reproduced the
/// pre-batch-35 build because the envelope had exactly one reader; it now has two, and the
/// ground's reader is this flag. `--no-terrain-shafts --no-distant-shadows` is the
/// pre-batch-35 revert, exactly as `--cloud-shadow 0 --no-terrain-shafts` became the
/// pre-batch-11 one. The same shape of change, one batch later, and [`crate::harness::IMPLIES`]
/// carries it as a pair of claims rather than one.
///
/// Unconditional on the world for the reason [`FLAG_TERRAIN_SHAFTS`] is: no control removes
/// the terrain, so there is no "the field could never be reached" case to nest it under.
pub const FLAG_DISTANT_SHADOWS: u32 = 32768;

/// Leaf cutouts (batch 38): a leaf block is a 4^3 sub-voxel occupancy mask carved out of its
/// cube rather than the whole cube, so a canopy has holes in it. `--no-leaf-cutout` clears it
/// and reproduces the pre-batch-38 frame bit for bit; being in [`SPEC_MASK`] it does so for
/// free -- the micro-DDA and its hash leave `march` and `resolve` entirely rather than being
/// branched past, which is the only kind of free this project counts.
///
/// **It is reachable from `resolve` and not only from `march`**, which is the thing to know
/// before pricing it: water's secondary rays call `march_chunk` from inside the deferred pass,
/// so anything added to the traversal is billed to the pass that is 65% of the frame whether
/// or not a leaf is ever on screen. That is exactly how batch 14 paid +1.85 ms for a feature
/// `resolve` never draws, and why this flag is in the mask rather than being a uniform.
///
/// Unconditional on the world for the reason [`FLAG_TEX_VARIATION`] is: no control removes the
/// trees, and `--no-biomes` changes which leaf a biome plants rather than whether it plants
/// one.
pub const FLAG_LEAF_CUTOUT: u32 = 65536;

/// Glass transmits (batch 38b): the primary ray stops at the first pane and `resolve` shades
/// it with a refraction leg and a Fresnel-weighted sky reflection instead of an albedo.
/// Being in [`SPEC_MASK`] the control is free -- `shade_glass`, its trace and its Fresnel
/// leave `resolve` entirely rather than being branched past.
///
/// **`--no-glass` is the first control in this project that is not a pure revert, and the
/// reason is `BlockDef::opaque`.** The batch's light scope was chosen deliberately: glass is
/// `opaque: false`, so the flood runs through a pane and a glass house is lit from the sky
/// rather than being a black box with a window in it. That is a property of the *world* --
/// `light.rs` reads it while building a chunk, long before any pipeline exists -- so a shader
/// override cannot put it back. At a vantage holding glass this control therefore reproduces
/// the pre-38b *shading* over post-38b *lighting*, and at the fourteen vantages that hold no
/// glass it reproduces the parent build exactly, because a flood through a block nobody
/// placed is the flood that was already there. The control table in `../CLAUDE.md` carries
/// the same sentence; this is the copy a later batch reads.
///
/// Unconditional on the world, and this one is *not* free the way [`FLAG_TEX_VARIATION`]'s
/// version of that claim is. There is no "the world has no glass" nesting available even
/// though the generator places none, because `block::GLASS` sits in `block::HOTBAR`: a player
/// can build a window at any moment of any run, so a flag denying glass would compile the
/// marcher unable to see through one it is about to be handed. That is `FLAG_FOLIAGE`'s
/// hazard with the opposite remedy -- foliage could assert its way out because only the
/// generator makes tufts, and glass cannot, so it never nests.
pub const FLAG_GLASS: u32 = 131072;

/// The short shadow march under water (batch 41): a surface reached through the sea marches
/// its own shadow ray [`WATER_SHADOW_DIST`] blocks instead of [`WATER_SHADOW_DIST_PRE41`],
/// and batch 37's light envelope covers the rest. `--no-water-shadow-cut` clears it and
/// reproduces the pre-batch-41 frame bit for bit; being in [`SPEC_MASK`] it does so for free.
///
/// **It is the only flag in this list the shader never reads**, and that is a consequence of
/// what it moves rather than an oversight. Every other one is ANDed with its own `SPEC_`
/// override, so the flag word stays the thing that decides and a pipeline key disagreeing
/// with `frame.flags` draws the picture the flag asked for. A *distance* cannot be carried in
/// one bit, so here the override is the decision and this flag exists to key the pipeline
/// cache. That is safe for the reason [`SPEC_MASK`] is safe at all -- every bit in it is
/// fixed for the life of the process, read once out of the parsed config.
///
/// Unconditional on the world, and for a stronger reason than [`FLAG_TEX_VARIATION`]'s: this
/// does not gate a feature that could be absent, it *is* the value of a constant the deferred
/// pass always reads. A dry world reaches it zero times because `shade_water` is never
/// called, which is the has-water bit of [`GpuChunk::attr_flags`] doing its job and not this
/// flag's -- that bit is per chunk, and this one would be per process.
pub const FLAG_WATER_SHADOW_CUT: u32 = 262144;

/// How far a shadow ray cast from a surface reached *through* the water marches, in blocks,
/// against `frame.shadow_dist`'s 220 for one reached through air.
///
/// **This is the whole of batch 41 and it is one number.** Batch 40 measured the ray it bounds
/// at **4.100 ms +/- 0.068 of a 13.080 ms `resolve`** at `coastline` -- 31% of the pass, and
/// the largest single cost anybody has measured in this engine -- and measured the shape of
/// the curve under it: the first eight blocks cost three to five times per block what the last
/// sixteen do, because most rays terminate early, so the near blocks are paid by every ray and
/// the far ones only by survivors. 48 -> 16 hands back **1.672 ms +/- 0.007** of that at
/// `coastline`, 0.531 at the default camera and 0.465 at `shore`, benched paired against
/// `voxelcraft-pre41.exe` over 8 rounds -- and batch 40 read the same step at 1.707 from the
/// other direction, which is two independent pairings 2% apart.
///
/// **What makes it nearly free to look at is batch 37, not this batch.** `shade_hit` passes
/// the same budget to `terrain_shade_beyond`, so the light envelope resumes exactly where the
/// march stops and the two partition `[0, d)` and `[d, reach)` with no gap -- shortening `d`
/// moves the handoff rather than deleting the shadow. Measured across six vantages the whole
/// truncation is **at most 1,309 pixels of 921,600 at a max channel delta of 1**, against
/// 22,183 at a delta of 109 for the reflection-reach truncation that was the other candidate.
///
/// **What is given up is the band's worth of occluders the envelope cannot represent**, and
/// that is why this is 16 and not 0. The envelope is `WorldGen::height`: it knows the terrain
/// and it does not know a tree, a player's wall, or anything the edit journal put there. A
/// 16-block exact march still catches a tree-height occluder at a mid sun; the fixture holds
/// no vantage that can see one, which is a reason to keep the margin and not a reason to
/// believe there is nothing there. The sweep behind both halves is in `PERF.md`.
/// A thinner canopy (batch 43): a leaf block's 4^3 carve keeps [`LEAF_FILL`] of its cells
/// instead of [`LEAF_FILL_PRE43`], so a tree's silhouette breaks up against the sky rather
/// than reading as the blocky edge of a cube. `--no-leaf-thin` clears it and reproduces the
/// pre-batch-43 frame bit for bit; being in [`SPEC_MASK`] it does so for free.
///
/// **It is the second flag the shader never reads**, for [`FLAG_WATER_SHADOW_CUT`]'s reason:
/// `SPEC_LEAF_FILL` carries a fraction, and a fraction does not fit in a bit of `frame.flags`,
/// so the override is the decision and this exists to key the pipeline cache.
///
/// Unconditional on the world, and **not** nested under [`FLAG_LEAF_CUTOUT`] even though a
/// build with the cutout off never reads the fraction. Nesting would make `--no-leaf-cutout`
/// quietly a second control for this as well, which is the mistake `FLAG_DISTANT_SHADOWS`
/// avoided against `FLAG_TERRAIN_SHAFTS` -- two readers of one field, one control each.
pub const FLAG_LEAF_THIN: u32 = 524288;

/// Flat lighting for a hit reached *through* water or glass (batch 45): the three secondary
/// `shade_hit` call sites take one light lookup where the primary keeps the nine-cell gather.
/// `--no-flat-secondary` clears it and reproduces the pre-batch-45 frame bit for bit; being in
/// [`SPEC_MASK`] it does so for free.
///
/// **The flag the shader reads, unlike the two above it**, because this one is a switch rather
/// than a number: `secondary_smooth()` in `resolve.wgsl` is `SPEC_FLAT_SECONDARY` ANDed with
/// this bit, which is the shape every override before batch 41 used.
///
/// Unconditional on the world for [`FLAG_GLASS`]'s reason rather than [`FLAG_TEX_VARIATION`]'s:
/// water is placed by the generator and glass by the player, so there is no "this world has no
/// secondary rays" nesting available, and a dry world reaches the arm zero times anyway
/// because `shade_water` is never called.
pub const FLAG_FLAT_SECONDARY: u32 = 1048576;

/// The submerged reach cut (batch 48): a `tile_select` tile whose every ray is still inside
/// the water at `WATER_FAR_DIST` stops testing chunks there rather than at `frame.far`.
/// `--no-water-far` clears it and reproduces the pre-batch-48 frame bit for bit.
///
/// **Deliberately not in [`SPEC_MASK`], and the reason is [`FLAG_UNDERWATER`] rather than a
/// judgement about cost.** This feature does nothing unless the camera is submerged, and that
/// is a run-time fact about where the player is standing -- so the arm cannot be folded away
/// at pipeline-build time whatever this bit did, and a `SPEC_` partner would key a second
/// pipeline on a decision the shader still has to make per frame. The two flags are ANDed
/// together in `tile_select`, which is the plain run-time shape every flag outside the mask
/// uses.
///
/// **The pass it lives in could not take an override anyway**, which is the sharper half:
/// `SpecPipes` holds `march` and `resolve` and nothing else, so `tile_select` is built once
/// with no constants. A number here would have had to be a `GpuFrame` field -- and that struct
/// has no pads left -- or a plain WGSL `const`, which is what it is.
///
/// Unconditional on the world, for [`FLAG_WATER_SHADOW_CUT`]'s reason: a dry world reaches the
/// test zero times because [`FLAG_UNDERWATER`] is never set in one, which is `underwater_flag`
/// doing its job rather than this bit's.
pub const FLAG_WATER_FAR: u32 = 2097152;

/// The dark-water rung of that same reach (batch 53): a submerged tile still **below the sky
/// flood's reach** at [`FLAG_WATER_FAR`]'s distance stops at the much shorter
/// `WATER_DARK_DIST` instead. `--no-water-dark` clears it and reproduces the pre-batch-53
/// frame bit for bit.
///
/// **This one is derived rather than authored, and the derivation is the entry.** `light.rs`
/// pours sky light down a column and takes one level off per block of *water*, so a cell with
/// [`crate::light::MAX_LEVEL`] blocks of water above it has sky level exactly 0 -- and the
/// flood decrements per step in every direction, so no lateral path can beat that bound.
/// Everything past this reach is then covered twice over: it is **unlit** while the ray is
/// still that deep, and **absorbed** from `WATER_FAR_DIST` on, which is the argument batch 48
/// already made. The test asks for the two intervals to *overlap* -- the ray still under
/// `WATER_DARK_DEPTH` at `WATER_FAR_DIST` -- which is why the rung is nested inside batch 48's
/// in `tile_select` rather than standing beside it.
///
/// **What it is not is a second control for the same field.** It cuts a band batch 48's rung
/// had already decided was worth cutting, and cuts it shorter; a camera that fails this test
/// gets exactly the batch-52 frame. That is the distinction [`FLAG_DISTANT_SHADOWS`] drew
/// against [`FLAG_TERRAIN_SHAFTS`] -- two readers of one field, one control each -- and it is
/// why this is its own bit rather than a wider `WATER_FAR_DIST`.
///
/// Outside [`SPEC_MASK`] for [`FLAG_WATER_FAR`]'s reason and in the same pass, so the same two
/// paragraphs above apply verbatim: the camera's medium is a run-time fact and `tile_select`
/// gets one pipeline.
pub const FLAG_WATER_DARK: u32 = 4194304;

/// The water surface seen from **underneath** (batch 54): total internal reflection and its
/// Fresnel curve, so a submerged eye stops seeing through the surface below the critical angle.
/// `--no-snell` clears it and reproduces the pre-batch-54 frame bit for bit.
///
/// **Reported from play rather than found by a metric**, at 32 blocks down with a mountain
/// still drawn above the horizon. Water's critical angle is `asin(1 / WATER_IOR)` = 48.6
/// degrees from the normal, so the world above the surface belongs in a cone 41.4 degrees above
/// horizontal and everything below that is a mirror -- of the medium's own colour, since a
/// mirrored ray is absorbed to exactly that and the traced version was measured worth a max
/// channel delta of 3 to 7 for 4 ms. Nothing here modelled the
/// boundary at all: [`FLAG_UNDERWATER`] makes water non-blocking for the primary march, so the
/// ray crossed it as though it were not there.
///
/// **No reach constant could have fixed it**, which is why this is a shading flag and not a
/// tighter [`FLAG_WATER_DARK`]. A tile clamp fires only when every ray in it is still submerged
/// at the clamp distance, and a ray aimed at a peak that breaks the surface leaves the water --
/// so the wrong band starts at `atan(depth / WATER_FAR_DIST)`, above which a clamp would delete
/// the sky as well.
///
/// Outside [`SPEC_MASK`] for [`FLAG_WATER_FAR`]'s reason exactly: the arm is only ever reached
/// with [`FLAG_UNDERWATER`] set, which is a run-time fact about where the player is standing, so
/// a pipeline key could not fold it away whatever this bit did.
pub const FLAG_SNELL: u32 = 8388608;

/// Batch 75, roadmap A3's land half: sand within `WET_BAND` blocks of the waterline is
/// drawn wet. Nested under `cfg.sea_level > 0` in `spec_hi_from`, because a run that
/// floods nothing has no waterline and the band could never be authored against one. The
/// term is an albedo, which is what wetness physically *is*, and authoring it through
/// `shade_hit`'s albedo is also what gives the reflected and refracted calls the same
/// answer for free: they pass through the same function with the same `hit`.
///
/// **Second word, and the first version put it in the first: the verification pass caught
/// this bit colliding with `FLAG_PROBE_BOUNCE` (134217728), which has owned bit 27 since
/// batch 61.** `frame.flags` has been full since batch 63 spent bit 31, and two sessions
/// have now failed the same grep's absence: before taking a `frame.flags` bit, grep
/// `^pub const FLAG_` and count. The override alone is the shader's gate -- the second
/// word never leaves the CPU, and batch 65 measured adding the parallel runtime test at
/// **+1.42 ms** for nothing the pipeline key cannot already say. `--no-shore-wet`
/// reproduces the pre-batch shore bit-exactly by reverting the thing it names: the band
/// is nothing but this multiply, and with the override off the text of it is not in the
/// module.
pub const FLAG_HI_SHORE_WET: u32 = 8;

/// Batch 75, roadmap A3's water half: where the traced refracted leg finds a shallow
/// column -- land within wave reach of the surface -- the sea gets a foam mottle, animated
/// by `frame.time` so a still frame is deterministic and a live one moves. Nested under
/// `cfg.sea_level > 0` in `spec_hi_from`, measured against the same quantity the refraction
/// already computed, which is the whole structural shape of the feature: **a second term
/// on the depth the refracted leg had to find anyway**.
///
/// **Second word for [`FLAG_HI_SHORE_WET`]'s reason, and it collided with
/// `FLAG_LIGHT_RGB` (268435456) in exactly the same paste.** Because the depth the term
/// needs is in the refracted leg, `--no-water-refract` mutes the foam along with the leg:
/// the colour the column would have shown is gone from that build entirely, so nothing is
/// authored against it there either. `--no-shore-foam` is the control and a pure revert.
pub const FLAG_HI_SHORE_FOAM: u32 = 16;

/// Batch 81, roadmap P12: the water legs traced once per 2x2 block of pixels, read back
/// by their owner texel. **The entry's diagnostic arm, explicitly and on purpose** --
/// dumb point upsample, no guided filter, no edge stop -- built to be measured against
/// `--reference` at `open-sea`/`shore`/`terraces` and interpreted by one number. Near the
/// 2.87 by-eye line the guided pass is worth its session and this arm is where it hangs;
/// at 15 the spatial-sharing premise is wrong before the design was, and that finding is
/// written down instead.
///
/// `--no-water-sec` is a free bit-exact revert: with the bit clear the override folds
/// `shade_water_sec` and the whole buffer path out of `resolve`, whose pipeline then
/// contains the batches-8 shade path unchanged. The two half-res textures still exist in
/// that build (one bind group serves every spec variant), and nothing dispatches into
/// them: 1.6 MB of VRAM holding an unused value, the price of one binding table.
pub const FLAG_HI_WATER_SEC: u32 = 32;

/// Batch 90, roadmap A9 (`--caustics`): sun caustics on the flooded floor, from the wave
/// field's own octaves at the sun-projected waterline point. Nested under `sea_level > 0`
/// beside this batch's shore trio -- a waterless world has no surface to focus through --
/// and read in the shader only through `SPEC_CAUSTICS`, which the shipping build never
/// sees set. That makes the off build's compiled resolve bit-identical, the only revert
/// guarantee a look arm of pure ALU can give.
pub const FLAG_HI_CAUSTICS: u32 = 64;

/// Batch 90, roadmap A4 (`--glass-reflect`): replace a pane's sky-only reflection with
/// one traced `water_reflect_leg` march. **Not nested under `sea_level`** -- glass shades
/// in mountains and caves the same -- and off by default, so the shipping pane keeps the
/// assumption until the by-eye pass names which picture is the pane. Shader reads only
/// the `SPEC_GLASS_REFLECT` override.
pub const FLAG_HI_GLASS_REFLECT: u32 = 128;

/// Batch 91, roadmap A2c (`--snell-bend`): the marcher's two-segment bend at the sea
/// plane, so the Snell window is filled with the bent world, not a straight hole in it.
/// **`resolve` needs nothing** -- its cone, Fresnel and mirror read the original `rd` and
/// `dist`, which the bend preserves as `t_surf + t2` -- so the arm's whole cost lives in
/// `march`, and that is the pass whose register read this entry has owed since batch 54.
/// Off by default, same standing law as the rest of batch 90-91.
pub const FLAG_HI_SNELL_BEND: u32 = 256;

/// Batch 92, roadmap A8 (`--tint-balance`): the balanced biome tint table plus atlas masks
/// on the sand and lichen layers, so the three biomes batch 12's ground tint never reached
/// take their colour. **Two halves, one bit**: the shader half swaps `TINT_PLAINS` for the
/// balanced table through `SPEC_TINT_BALANCE` inside the existing `SPEC_TINT` gate -- off,
/// the constant folds and the compiled module is bit-identical -- and the atlas half is a
/// `build_atlas` argument, because alpha-is-tint-mask is *content*: no pipeline can switch
/// a mask the file does not contain. Off by default, standing law.
pub const FLAG_HI_TINT_BALANCE: u32 = 512;

/// Batch 95, roadmap G1: the four sky-look knobs -- `--cloud-patch`, `--cloud-relief`,
/// `--haze-warm`, `--zenith-deep`. **One bit for the family, taken because the uniform-guard
/// class failed its own gate.** Batch 95a/b guarded each arm as `if frame.knob > 0.0` and
/// the 21-vantage control read the price of that choice: seven vantages moved 1-4 pixels at
/// max delta 1, pure reassociation in the arithmetic *around* branches the compiler never
/// removes. The by-eye pass this goal lives by needs a control that is bit-exact by
/// construction, and only the override gives one: with the bit clear, naga folds every
/// `SPEC_SKY_LOOK && ...` to false and the module is the pre-95 one. The knobs stay
/// uniforms *past* the override, so the sweep's ladders walk magnitudes without a pipeline
/// rebuild -- the bit flips once, on the first non-zero value, exactly where
/// `ensure_spec`'s cache expects it. Bit 1024; the ledger of spent bits lives in
/// `tests/spec_hi.rs`'s one enumeration.
pub const FLAG_HI_SKY_LOOK: u32 = 1024;

/// Batch 97a, roadmap G3's sky half (`--sky-cool`): the gradient and the deck swap their
/// day constants -- horizon and zenith toward clean cornflower, cloud bellies toward
/// grey-blue -- aimed at the reference frames' cool midday sky. **`SPEC_TINT_BALANCE`'s
/// class and [`FLAG_HI_SKY_LOOK`]'s reason at once**: it is a constant swap, so the
/// instructions stay the same and only the immediates move, and it lives behind a
/// specialization override because the swap sites stand in `sky_base` and
/// `cloud_radiance` -- code the marcher inlines -- where a uniform guard would bill the
/// control for its presence, exactly what batch 95's re-gate measured. Bit 2048.
pub const FLAG_HI_SKY_COOL: u32 = 2048;

/// Batch 97c, roadmap G3's water half (`--water-look`): the references' loudest water
/// cue -- animated metre-scale micro-ripples on the sky-facing normal, a murk
/// absorption tint that grows with the traced column, and a sunlit turquoise band
/// where the column shallows. **New code, pure ALU, behind an override** -- `water_ctx`
/// and `water_compose` compile into `resolve` twice over (batch 98's inlines), so the
/// control rides a fold the compiler cannot reassociate around, never a `frame.flags`
/// test. Nested under `cfg.sea_level > 0` in `spec_hi_from` for the shore family's
/// reason: a waterless world has no surface to ripple. Bit 4096.
pub const FLAG_HI_WATER_LOOK: u32 = 4096;

/// Batch 97d, roadmap G2's palate (`--foliage-rich`): ground-cover and grass-top
/// species richness -- per-voxel hashed dry-tan stands, value jitter and a rare flower
/// pink, mixed **into the albedo** in `shade_hit` the way the biome tint is, because an
/// appearance of a material is an albedo and not a light. Geometry untouched, so the
/// marcher sees the same world: this is the sanctioned richer-procedural class, shader
/// side. Bit 8192.
pub const FLAG_HI_FOLIAGE_RICH: u32 = 8192;

/// Batch 97e, roadmap G2's canopy (`--canopy-relief`): leaves take per-clump brightness
/// relief (sun-side bright clumps, dark interior), per-voxel micro jitter and a
/// sun-facing modulation, the references' canopy grain. Same albedo argument as its
/// foliage sibling, same fold class, same default-off law. Bit 16384.
pub const FLAG_HI_CANOPY_RELIEF: u32 = 16384;
/// Batch 101f (`--wind-sway`): the foliage cross-quads *leanen* -- their grass-sway is a
/// shear field, so the closed-form hit stays one division, off by default and in the
/// pipeline-key word like every look arm before it: folding it away is what keeps the
/// unarmed shaders' register bill and the 21-vantage control exactly where they were.
pub const FLAG_HI_WIND_SWAY: u32 = 32768;

/// Batch 102a (`--soft-shadows`): **the sun is a disc and the disc's one ground
/// phenomenon is the penumbra** -- `shadow_ray` answers lit-or-not and nothing between,
/// which is why every tree shadow in the lookbook is a razor edge the references do not
/// have. Three cone taps around the centre verdict, their march budget scaled by the
/// blocker distance the centre tap reports, so contact stays hard and only the fringe
/// pays. Behind the override for batch 95's measured reason: a uniform guard would bill
/// the 21-vantage control for taps it never runs, and with the bit clear the arm's text
/// is not in the module. Bit 65536; the ledger of spent bits lives in `tests/spec_hi.rs`.
///
/// **Measured, third hardware round (2026-09-17): built correctly and did not ship.**
/// +4.545 ms against a +0.35 ms bar -- the cost is the taps themselves (1.3-1.8 ms
/// apiece however short their march; the scaffolding is 0.073) -- and 86% of the moved
/// pixels go *lighter*, a shadow-lifter where a penumbra must split ~50/50. The local
/// reach is real (MAE 2.6480 against the 2.87 crop rung at its strongest crop), so the
/// verdict is fix rather than revert: roadmap G5's boundary gate -- fire the cone only
/// where the blocker distance says an edge can exist -- is the sequel this arm waits
/// for. Do not tune `SOFT_PENUMBRA`; the ladder is measured and empty.
pub const FLAG_HI_SOFT_SHADOWS: u32 = 65536;
/// Opt-in material-after-lighting schedule, shader-never-read cache-key bit.
pub const FLAG_HI_COMPACT_SHADE_HIT: u32 = 131072;
pub const FLAG_HI_ISOLATE_GLASS: u32 = 262144;
pub const FLAG_HI_SHADOW_PASS: u32 = 524288;
pub const FLAG_HI_LEGACY_LIGHTING: u32 = 1048576;
pub const FLAG_HI_LEGACY_WATER: u32 = 2097152;
pub const FLAG_HI_SOFT_HQ: u32 = 4194304;
pub const FLAG_HI_DEBUG_A: u32 = 8388608;
pub const FLAG_HI_DEBUG_B: u32 = 16777216;
pub const FLAG_HI_DEBUG_C: u32 = 33554432;
pub const FLAG_HI_SUN_NARROW: u32 = 67108864;
pub const FLAG_HI_SUN_WIDE: u32 = 134217728;

/// Batch 55's probe tap, which shades nothing and exists to price roadmap L1's read side.
///
/// **The only flag here that ships *off*, and that is a requirement rather than a default.**
/// The arm it gates is one hardware trilinear tap into a 3D texture holding a constant 1.0, so
/// the picture it produces is bit-exact with the picture without it -- `x * 1.0` for every
/// finite `x` -- and its entire output is a millisecond. A diagnostic left switched on would
/// cost every later measurement the time it was built to measure, which is the tax
/// `--no-foliage`'s +1.24 to +2.07 ms put on every measurement taken through it until somebody
/// reached the roadmap entry.
///
/// **In [`SPEC_MASK`] for [`FLAG_WATER_SHADOW_CUT`]'s reason and not [`FLAG_TINT`]'s.** The
/// shader never tests this bit: `SPEC_PROBE_TAP` is the whole gate, deliberately, because the
/// arm sits in `shade_hit` and `resolve` inlines that four times -- a `frame.flags` companion
/// would carry the texture sample in every copy and the shipping build would pay for a tap it
/// never takes. So there is no run-time test that could disagree with a stale pipeline key, and
/// a bit outside the mask would leave `ensure_spec` unable to tell the two builds apart.
///
/// **What it does not price.** One tap is a DC-only RGB field, the cheapest thing L1 could
/// store; an SH L1 or a 6-axis ambient cube is three, and `PROBE_TAPS` in `resolve.wgsl` is the
/// literal that says which is being measured.
pub const FLAG_PROBE_TAP: u32 = 16777216;

/// Batch 56's removal: the probe field *replaces* `shade_hit`'s ambient instead of multiplying
/// into it, and the terms a pre-composed directional field would make redundant leave the module.
///
/// **The build this makes is deliberately incorrect and its picture is thrown away**, which is
/// batch 53's method -- patch, measure, discard. The field holds a constant, so the frame is
/// wrong; what it reports is the *other* half of batch 55's question. That batch priced what a
/// tap **adds** and found it does not scale with the coefficient count; roadmap L1's architecture
/// only pays if what it **removes** is larger, and nothing had measured that.
///
/// **What leaves**: nine `blk_n` `light_curve` evaluations, four per-corner `bk/(sk+bk)`
/// divisions with their `mix(SKY_TINT, BLOCK_TINT, ..)`, the `amb` bilinear, [`face_shade`] and
/// the ambient floor. **What stays**: the nine gather lookups, `sky_n`, the AO rule and `sky_sm`
/// -- everything a coarse probe could not carry, because AO is a contact cue and `sky_sm` gates
/// the sun, and both are high-frequency where indirect light is not.
///
/// It implies [`FLAG_PROBE_TAP`], set together in `flags_from`: the ambient has to come from
/// somewhere, and the removal without the tap would measure a shader that reads no light at all.
pub const FLAG_PROBE_AMBIENT: u32 = 33554432;

/// Batch 57's ambient cube: `shade_hit` takes its directional shading factor from the baked
/// probe lattice instead of from [`face_shade`]'s per-normal constants. `--no-probe-cube`
/// clears it and reproduces the pre-batch-57 frame bit for bit.
///
/// **The first rung of roadmap R1, and what it deletes is a constant.** 1.00/0.80/0.62/0.50 by
/// normal is the whole of directional occlusion in the pre-57 build: a wall at the foot of a
/// cliff is lit exactly like the same wall in an open field, and a block's top is bright
/// because it is a top rather than because it can see sky. The cube is baked from the height
/// field on a rayon worker -- the resource `CLAUDE.md`'s table measures as idle -- and read as
/// the one trilinear tap batch 55 priced.
///
/// **In [`SPEC_MASK`] for [`FLAG_PROBE_TAP`]'s reason exactly**: the arm lives in `shade_hit`,
/// `resolve` inlines that four times, and a `frame.flags` companion would carry the texture
/// sample into every copy so the control build paid for a tap it never takes. That is batch
/// 45's +1.42 ms, and it is why `SPEC_PROBE_CUBE` is the whole gate and there is no run-time
/// test that could disagree with a stale pipeline key.
///
/// **The control is free and it is a pure revert**, which the field being created full of
/// **the lattice being created holding 1.0** is what guarantees: with the bit clear the shader reads the constants
/// out of the same switch it always did, and with it set but nothing baked the lattice returns
/// those same constants. So an unstreamed chunk, a coarse LOD and a pinned `--probe-fill` all
/// render the pre-57 frame rather than something that merely resembles it.
pub const FLAG_PROBE_CUBE: u32 = 67108864;

/// Batch 58's ground bounce: `shade_hit`'s ambient floor takes its colour and its magnitude
/// from the baked probe lattice instead of from one authored constant. `--no-probe-bounce`
/// clears it and reproduces the pre-batch-58 frame bit for bit.
///
/// **Roadmap R2's first rung, and what it deletes is the constant that says so itself.**
/// `floor_rgb` is `(0.85, 0.90, 1.00) * frame.ambient` and the comment at its own definition
/// site reads *"it is what stands in for the bounce light the two-source model never
/// simulates"*. The bake simulates one: `probe::bake` gathers the albedo of the ground each
/// axis can see, off the horizon march batch 57 already pays for, and stores a per-channel
/// multiplier on that constant.
///
/// **It costs nothing to read, and that is not an estimate.** The multiplier rides in the
/// `.rgb` of the texel whose `.a` carries [`FLAG_PROBE_CUBE`]'s occlusion -- the same texel,
/// the same slab, the same address -- so one `textureSampleLevel` returns both and the read
/// side is exactly the tap batch 55 priced and batch 57 shipped. The field was `Rgba16Float`
/// from the start for this.
///
/// **In [`SPEC_MASK`] for [`FLAG_PROBE_CUBE`]'s reason exactly**, and with its own: the arm is
/// in `shade_hit`, which `resolve` inlines four times, and the shader never reads the bit, so
/// nothing at run time could catch a pipeline key that had gone stale and `--no-probe-bounce`
/// would render the shipping frame into a sweep whose pass condition is *0 pixels differing*.
///
/// **The control is free and it is a pure revert by construction rather than by luck.** The
/// field stores a multiplier whose identity is 1.0 and the lattice is created holding 1.0, so
/// an unbaked texel, a coarse LOD, a pinned `--probe-fill 1.0` and a probe with no sky over it
/// all render `floor_rgb * 1.0`, which is `floor_rgb` to the bit. That is batch 57's own
/// argument for storing occlusion rather than shading, one rung on.
pub const FLAG_PROBE_BOUNCE: u32 = 134217728;

/// Batch 59's coloured block light: the flood carries three channels and `shade_hit` derives
/// the tint from what reached the cell, instead of one channel wearing one authored constant.
/// `--no-light-rgb` clears it and reproduces the pre-batch-59 frame bit for bit.
///
/// **Roadmap R4, and what it deletes is `BLOCK_TINT`.** `(1.0, 0.65, 0.32)` was the colour of
/// every emitter in the world, which is the shape of thing the main sequence retires -- and the
/// entry's own finding was that the world had exactly one emitter to be wrong about, so the
/// batch is content as much as it is code. Three lamps and a widened cell.
///
/// **The channel had to widen rather than the block table, and the flood is why.** A scalar
/// level with a per-block tint applied at read time cannot mix: the cell between a red lamp and
/// a blue one holds one number and no memory of which block put it there. Three independent
/// floods hold both, and `light::pack` is now two bytes -- `sky:4 | r:4 | g:4 | b:4`.
///
/// **In [`SPEC_MASK`] for [`FLAG_PROBE_CUBE`]'s reason exactly**: the arms are in `shade_hit`,
/// which `resolve` inlines four times, the shader reads no `frame.flags` bit for this, and a
/// bit outside the mask would leave `ensure_spec` unable to tell the control's pipeline from
/// the shipping one -- so `--no-light-rgb` would render the shipping frame into a sweep whose
/// pass condition is *0 pixels differing*.
///
/// **The control is a pure revert at every world that predates the batch, and not at one that
/// does not.** That is [`FLAG_GLASS`]'s shape rather than [`FLAG_PROBE_CUBE`]'s: the three new
/// blocks are a *world* change no pipeline override can undo, so at a vantage holding one the
/// control is the old shading over a world the old build never had. What makes the revert exact
/// everywhere else is `light::block_level` -- every channel of an emitter decrements by one per
/// flood step, so `max(r, g, b)` is the level the single pre-59 flood held at every cell it
/// reached, and glowstone's row is authored to keep that true.
pub const FLAG_LIGHT_RGB: u32 = 268435456;

/// Batch 60's bounced sunlight: the ambient floor stops being an authored constant and becomes
/// a term proportional to the sun, the ground's albedo and the sky that ground can see.
/// `--no-probe-sun` clears it and reproduces the pre-batch-60 floor bit for bit.
///
/// **Roadmap R3, and what it answers is ADR 0001's retry condition rather than a new idea.**
/// That ADR rejected the elevation form factor because `Config::ambient` is 0.08 and
/// `floor_rgb` is therefore about 7% of the light on a lit surface, so no *redistribution* of
/// it can be worth more than that -- measured, the term moved `default` by a mean of 2.87 code
/// values and was rejected by eye. Batch 60's own transport half lands at **0.372**, eight
/// times under the thing already rejected, which is that ceiling measured a second time rather
/// than argued about. The ADR names the only way out: *a build where the ambient floor carries
/// materially more energy*. This is that build.
///
/// **It is additive and gated on what a cave does not have.** The authored floor stays exactly
/// where it was and keeps being the sole light in an unlit cave; what is added is
/// `probe.rgb * PROBE_SUN_GAIN * frame.daylight * probe.a`, every factor of which goes to zero
/// underground or at night -- `probe.a` is the baked sky occlusion and `believe` has already
/// driven `probe.rgb` to 1.0 where there is no sky. So the term appears exactly where sunlight
/// reaches ground and nowhere else, which is the definition of the thing it is standing in for.
///
/// **The transport half is a prerequisite and not a companion.** Raising the energy of a floor
/// that does not know which ground is shadowed would brighten a ravine exactly as much as an
/// open field, which is worse than the constant it replaces. `--no-probe-shadow` is the other
/// half and lives on `WorldGen` rather than here, because it changes the bake.
pub const FLAG_PROBE_SUN: u32 = 536870912;

/// The louder of batch 60's two bounce gains. `--probe-sun-high` sets it; **the shipping build
/// clears it**, which makes this the one bit in [`SPEC_MASK`] whose *default is off*.
///
/// It is a sweep knob rather than a control: it does not reproduce an earlier build, it picks
/// between `PROBE_SUN_GAIN` and `PROBE_SUN_GAIN_HIGH` so the strength of one bounce can be
/// ranked by eye without a rebuild. `--no-probe-sun` is the control; this is the ladder.
pub const FLAG_PROBE_SUN_HIGH: u32 = 1073741824;

/// Batch 63's derived sky hue: the ambient's sky colour is the sky model's own, integrated over
/// the hemisphere each face can see, instead of one authored constant. `--no-sky-tint` clears it
/// and reproduces the pre-batch-63 frame bit for bit.
///
/// **Roadmap R7, and what it retires is [`SKY_TINT`'s role rather than its value].** That
/// constant multiplied `amb` on *every shaded pixel in the world* and was scaled only by
/// `frame.daylight`; nothing coupled it to the sky the renderer actually drew, which sat in the
/// next file and was read by water's reflections and the sky path and by nothing else. It is now
/// the value the derivation returns at one named reference condition -- a face pointing straight
/// up, full daylight, no halo -- which is what `face_shade` became at batch 57 and `floor_rgb` at
/// batch 58. The main sequence's admission test, met by a rung nobody planned.
///
/// **This is bit 31 and `frame.flags` has no more.** [`FLAG_PROBE_SUN_HIGH`] at bit 30 was the
/// highest in the shipped tree -- batch 61 defined a bit 31 and was reverted, so nothing had
/// spent it. A later batch wanting a control has to widen the word, take a `SPEC_MASK` value
/// override the way batches 41 and 43 did, or find its control outside the shader the way
/// `--no-shadow-share` did.
///
/// **In [`SPEC_MASK`] for [`FLAG_LIGHT_RGB`]'s reason exactly**: the arms are in `shade_hit`,
/// which `resolve` inlines four times, the shader reads no `frame.flags` bit for this, and a bit
/// outside the mask would leave `ensure_spec` unable to tell the control's pipeline from the
/// shipping one -- so `--no-sky-tint` would render the shipping frame into a sweep whose pass
/// condition is *0 pixels differing*.
///
/// **Only the hue moves.** The derived colour is renormalised to `SKY_TINT`'s own luminance, so
/// no exposure anywhere in the world changes and the before/after pair ranks a colour rather
/// than a brightness. R7's entry names that as the trap that could waste the batch: the real sky
/// is far darker than the constant standing in for it, day zenith being (0.0637, 0.2140, 0.8276)
/// against (0.6038, 0.7084, 1.0000), so a naive substitution would darken every shadow in the
/// world and read as the feature.
pub const FLAG_SKY_TINT: u32 = 2147483648;

// ---------------------------------------------------------------------------
// The second specialization word (batch 65)
// ---------------------------------------------------------------------------
//
// **`frame.flags` ran out at bit 31 and this is the exit [`FLAG_SKY_TINT`] named, taken in the
// cheap direction.** That doc offered three ways out for the next batch that wanted a control:
// widen the word, take a value override the way batches 41 and 43 did, or find the control
// outside the shader the way `--no-shadow-share` did. The second is not actually an exit --
// `SPEC_WATER_SHADOW_DIST` and `SPEC_LEAF_FILL` are *driven* by [`FLAG_WATER_SHADOW_CUT`] and
// [`FLAG_LEAF_THIN`], so a value override still costs a bit -- and the third is not available to
// a term that lives in the shader. So it is the first, and the finding is that "widen the word"
// is two different changes with very different prices.
//
// **What had to widen is the pipeline *key*, not the uniform.** Every `SPEC_MASK` bit from
// [`FLAG_PROBE_TAP`] onward is one the shader never reads: the `SPEC_` override is the whole
// gate, and the bit exists only so `ensure_spec` can tell one pipeline from another. Such a bit
// is a passenger in `GpuFrame` -- uploaded, never looked at. So a control of that shape needs a
// bit in the *key* and none in the uniform, and this word is CPU-side only: it is never written
// into `GpuFrame`, which therefore does not grow, keeps its 16-byte `mat4x4` alignment, needs no
// invented pad and invalidates none of the seven `offset_of!` asserts. Widening `flags` itself to
// `u64` would have touched seventeen call sites and every `FLAG_` constant to buy the same thing.
//
// **The rule this buys, and it is load-bearing: a bit here must be one no shader reads.** There
// is no `frame.spec_hi` for it to read. A later batch that needs a control the *shader* tests --
// `FLAG_UNDERWATER`'s shape, where the arm is chosen at run time rather than compiled -- cannot
// use this word and has to widen `GpuFrame` after all. `spec_hi_is_never_uploaded` is the guard.
//
/// Batch 65, roadmap R6. Fresnel-weighted sky specular on every opaque surface.
///
/// `--no-sky-specular` reproduces the pre-batch-65 frame bit for bit; being keyed into the
/// specialization it does so for free, exactly as a `SPEC_MASK` bit would.
pub const FLAG_HI_SKY_SPECULAR: u32 = 1;

/// Batch 72, roadmap P10. A ray that ignores water now marches a mixed-full chunk's
/// water instead of stopping at its face, the synthetic dedup of the tree being swapped
/// for the loop's own walk.
///
/// `--no-full-march` reproduces the pre-batch-72 frame bit for bit; being keyed into the
/// specialization it does so for free, exactly as `FLAG_HI_SKY_SPECULAR` does above.
pub const FLAG_HI_FULL_MARCH: u32 = 2;

/// Batch 73, roadmap P11: the block-light gather's data-dependent gate is skipped only
/// for the build in which it could never fire -- no emitter resident in the world. This
/// is a **world** key, not a config toggle: the app recomputes it from
/// `World::any_emitter_resident` every frame, and `ensure_spec` caches a pipeline pair
/// per value just as it does for a flag -- with both arms pre-warmed (see `render`), so
/// the transition frame draws rather than compiles.
///
/// There is no `--no-...` beside it because there is no pre-batch frame it could mimic by
/// a switch: with the bit set the shader is the same text it always was, and with it
/// clear the world provably has no emitter, so `any_blk` is 0 everywhere and every gated
/// read folds to the picture the shipping build drew. The two arms are bit-exact by
/// argument, and the A/B per vantage is the check.
///
/// **D5's question, and the exact answer it got.** The first build of this gate was
/// "conservative-upward": an edit armed it and nothing ever disarmed it until chunk
/// unload, so one long-broken lamp could keep a session permanently armed and quietly
/// spend the 0.351 ms the feature exists to save. The answer sitting one line up in
/// `set_block` is the boring one -- the edit *already knows both block ids*, so the world
/// keeps an exact per-chunk emitter count and the gate arms and disarms on the very frame
/// either happens. Conservative upward survives only as the reason not to scan: the count
/// is the scan, amortized across every edit ever made.
pub const FLAG_HI_EMITTER_WORLD: u32 = 4;

/// Which bits of the second word key a pipeline. Every one of them must be shader-never-read;
/// see the note above.
pub const SPEC_HI_MASK: u32 = FLAG_HI_SKY_SPECULAR
    | FLAG_HI_FULL_MARCH
    | FLAG_HI_EMITTER_WORLD
    | FLAG_HI_SHORE_WET
    | FLAG_HI_SHORE_FOAM
    | FLAG_HI_WATER_SEC
    | FLAG_HI_CAUSTICS
    | FLAG_HI_GLASS_REFLECT
    | FLAG_HI_SNELL_BEND
    | FLAG_HI_TINT_BALANCE
    | FLAG_HI_SKY_LOOK
    | FLAG_HI_SKY_COOL
    | FLAG_HI_WATER_LOOK
    | FLAG_HI_FOLIAGE_RICH
    | FLAG_HI_CANOPY_RELIEF
    | FLAG_HI_WIND_SWAY
    | FLAG_HI_SOFT_SHADOWS
    | FLAG_HI_COMPACT_SHADE_HIT
    | FLAG_HI_ISOLATE_GLASS
    | FLAG_HI_SHADOW_PASS
    | FLAG_HI_LEGACY_LIGHTING
    | FLAG_HI_LEGACY_WATER
    | FLAG_HI_SOFT_HQ
    | FLAG_HI_DEBUG_A
    | FLAG_HI_DEBUG_B
    | FLAG_HI_DEBUG_C
    | FLAG_HI_SUN_NARROW
    | FLAG_HI_SUN_WIDE;
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_SKY_SPECULAR != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_FULL_MARCH != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_EMITTER_WORLD != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_SHORE_WET != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_SHORE_FOAM != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_WATER_SEC != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_CAUSTICS != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_GLASS_REFLECT != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_SNELL_BEND != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_TINT_BALANCE != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_SKY_LOOK != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_SKY_COOL != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_WATER_LOOK != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_FOLIAGE_RICH != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_CANOPY_RELIEF != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_WIND_SWAY != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_SOFT_SHADOWS != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_COMPACT_SHADE_HIT != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_ISOLATE_GLASS != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_SHADOW_PASS != 0);

/// The fraction of a leaf block's 4^3 micro-cells that are solid.
///
/// **The one look constant batch 38 authored rather than derived, and batch 43 is its sweep.**
/// It shipped at 0.72 against the research's 0.45 on an argument -- that our leaf blocks are a
/// solid 5x5 stamp where the paper's were already sparse -- and the argument was never checked
/// against a picture. Seven builds, a wooded crest against open sky, and the same canopy seen
/// from above say the two halves of the frame want opposite directions: **the silhouette wants
/// it low** (at 0.82 and above a crest is the same blocky edge a solid cube gives, so the
/// feature is invisible), and **the crown's interior wants it high** (every carved cell opening
/// onto shadow is a dark speckle, and 0.45 is visibly moth-eaten from above). 0.62 is where
/// they meet. `docs/foliage.md` has the sweep.
pub const LEAF_FILL: f32 = 0.62;

/// The same fraction before batch 43, which is what [`FLAG_LEAF_THIN`] cleared restores.
pub const LEAF_FILL_PRE43: f32 = 0.72;

pub const WATER_SHADOW_DIST: f32 = 16.0;

/// The same budget before batch 41, which is what [`FLAG_WATER_SHADOW_CUT`] cleared restores.
/// It has been 48 since batch 8, where it was chosen against an uncapped march that put
/// `resolve` at 7.62 ms on a sea-filling frame -- see [`WATER_SHADOW_DIST`] for what replaced
/// the reasoning behind it, and `docs/water.md` for the original.
pub const WATER_SHADOW_DIST_PRE41: f32 = 48.0;

/// `common.wgsl` carries `GLASS_ID` as a literal, exactly as it carries `WATER_ID`, and this
/// is the only thing tying the two copies together. Same shape as the `block::WATER == 13`
/// assert below it and for the same reason: a shader cannot see this crate.
const _: () = assert!(block::GLASS == 10);

/// The flags folded into the specialized pipelines as `override` constants instead of being
/// branched on at run time (batch 13; extended to `march` by batch 15). Everything in this
/// mask is fixed for the life of the process -- `flags_from` reads them straight off the
/// parsed config -- which is what makes one pipeline per value affordable; `FLAG_HIZ`,
/// `FLAG_HEATMAP` and `FLAG_UNDERWATER` are toggled by a key or by where the camera is
/// standing and are deliberately *not* here.
///
/// It is a mask and not a bool so that adding the next flag is a `|` and a line in
/// `make_spec`. A bit costs a line here rather than a compile -- a run builds the one
/// combination it asks for, never the four -- and what earns a bit is a measured number.
/// Both of these have one: `FLAG_TINT` was +0.07 ms of `resolve` at the default camera
/// while executing nothing, and `FLAG_FOLIAGE` +1.24.
pub const SPEC_MASK: u32 = FLAG_TINT
    | FLAG_FOLIAGE
    | FLAG_WAVES
    | FLAG_WAVE_ANISO
    | FLAG_WAVE_SHOAL
    | FLAG_WAVE_FILL
    | FLAG_TERRAIN_SHAFTS
    | FLAG_TEX_VARIATION
    | FLAG_DISTANT_SHADOWS
    | FLAG_LEAF_CUTOUT
    | FLAG_GLASS
    | FLAG_WATER_SHADOW_CUT
    | FLAG_LEAF_THIN
    | FLAG_FLAT_SECONDARY
    | FLAG_PROBE_TAP
    | FLAG_PROBE_AMBIENT
    | FLAG_PROBE_CUBE
    | FLAG_PROBE_BOUNCE
    | FLAG_LIGHT_RGB
    | FLAG_PROBE_SUN
    | FLAG_PROBE_SUN_HIGH
    | FLAG_SKY_TINT;

/// Batch 41's bit has to be *in* the mask, and its two distances have to point the way the
/// batch says they do. Compile-time facts, so they fail the build rather than a test run --
/// the idiom the `block::WATER == 13` assert below uses, and for a sharper reason here.
///
/// [`FLAG_WATER_SHADOW_CUT`] is the only flag the shader never reads: `SPEC_WATER_SHADOW_DIST`
/// carries a number, so there is no `frame.flags` test to disagree with a stale pipeline key.
/// A bit outside the mask would leave `ensure_spec` unable to tell the control's pipeline from
/// the shipping one, `--no-water-shadow-cut` would render the shipping frame, and a sweep
/// whose pass condition is *0 pixels differing* would call that a pass.
const _: () = assert!(SPEC_MASK & FLAG_WATER_SHADOW_CUT != 0);
const _: () = assert!(WATER_SHADOW_DIST < WATER_SHADOW_DIST_PRE41);
const _: () = assert!(WATER_SHADOW_DIST_PRE41 == 48.0);
const _: () = assert!(SPEC_MASK & FLAG_LEAF_THIN != 0);
const _: () = assert!(SPEC_MASK & FLAG_FLAT_SECONDARY != 0);
// Batch 55's bit, in the mask for `FLAG_WATER_SHADOW_CUT`'s reason one line up: the shader
// never reads it, `SPEC_PROBE_TAP` is the whole gate, so nothing at run time could catch a
// pipeline key that had gone stale and `--probe-tap` would render the shipping frame.
const _: () = assert!(SPEC_MASK & FLAG_PROBE_TAP != 0);
// Batch 56's bit, in the mask for the same reason and with one of its own: the removal has to
// fold at pipeline-compile time or the deleted terms stay in the module and the batch measures
// nothing. A run-time gate here would be the measurement measuring itself twice over.
const _: () = assert!(SPEC_MASK & FLAG_PROBE_AMBIENT != 0);
// Batch 57's bit, in for batch 55's reason and with the sharper consequence: the shader never
// reads it, so a bit outside the mask would leave `ensure_spec` unable to tell the control's
// pipeline from the shipping one and `--no-probe-cube` would render the shipping frame -- into
// a sweep whose pass condition is *0 pixels differing*.
const _: () = assert!(SPEC_MASK & FLAG_PROBE_CUBE != 0);
// Batch 58's bit, in for exactly batch 57's reason one line up, and sharing that batch's
// texel: the bounce is the `.rgb` of the probe texel whose `.a` is the cube, so the two are
// one tap and one stale pipeline key would silently revert whichever of them the sweep was
// not looking at.
const _: () = assert!(SPEC_MASK & FLAG_PROBE_BOUNCE != 0);
const _: () = assert!(SPEC_MASK & FLAG_PROBE_SUN != 0);
const _: () = assert!(SPEC_MASK & FLAG_PROBE_SUN_HIGH != 0);
// Batch 63's bit, in for batch 59's reason: the shader reads no `frame.flags` bit for it,
// `SPEC_SKY_TINT` is the whole gate, so a bit outside the mask would leave `ensure_spec`
// unable to tell the two pipelines apart and `--no-sky-tint` would render the shipping frame
// into a sweep whose pass condition is *0 pixels differing*.
const _: () = assert!(SPEC_MASK & FLAG_SKY_TINT != 0);
// It is bit 31, and the statement that it is the last one `frame.flags` has is a fact rather
// than a sentence: the next batch to want a control bit fails this build instead of
// discovering at run time that its flag aliases nothing.
const _: () = assert!(FLAG_SKY_TINT == 1u32 << 31);
// Batch 59's bit, in for batch 57's reason: the shader never reads it, `SPEC_LIGHT_RGB` is the
// whole gate, so a bit outside the mask would leave `ensure_spec` unable to tell the two
// pipelines apart and `--no-light-rgb` would render the shipping frame into a sweep whose pass
// condition is *0 pixels differing*.
const _: () = assert!(SPEC_MASK & FLAG_LIGHT_RGB != 0);
// The control's own claim, as a compile-time fact rather than a sentence: glowstone's three
// channels reduce under `light::block_level` to the level it emitted before batch 59. Every
// channel decrements by one per flood step, so this identity at the source is the identity at
// every cell the flood reaches -- which is what makes `--no-light-rgb` bit-exact rather than
// close, and it would fail the build rather than a sweep if a later edit re-authored the row.
const _: () = assert!(block::BLOCKS[block::GLOWSTONE as usize].light[0] == 15);
const _: () = assert!(block::GLOWSTONE == 11); // WGSL GLOWSTONE_ID
// Batch 48's bit has to stay *out* of the mask, which is the inverse of every assert above
// it. `tile_select` is not a specialized pass, so a bit in `SPEC_MASK` would key a second
// `march`/`resolve` pair on a flag neither of them reads -- two identical pipelines, and a
// `bitexact` sweep whose pass condition is *0 pixels differing* would call that a pass.
const _: () = assert!(SPEC_MASK & FLAG_WATER_FAR == 0);
// Batch 53's bit, out for batch 48's reason and in batch 48's pass.
const _: () = assert!(SPEC_MASK & FLAG_WATER_DARK == 0);
// Batch 54's bit, out for the same reason and reached through the same run-time flag.
const _: () = assert!(SPEC_MASK & FLAG_SNELL == 0);
// `SAND_ID` (4) is mirrored into `common.wgsl` by hand exactly the way `WATER_ID` (13) is;
// this is the Rust-side pin for that mirror, so a renumbering fails the build and not the day.
const _: () = assert!(crate::block::SAND as u32 == 4);
const _: () = assert!(LEAF_FILL < LEAF_FILL_PRE43);
const _: () = assert!(LEAF_FILL_PRE43 == 0.72);

/// `resolve.wgsl` evaluates the biome field a second time, per pixel, and it cannot see
/// this crate -- so it carries `band_weights`, the two seed offsets and the 3x3 of biome
/// ids as literals. The frequencies it gets through `GpuFrame`; these four are what is left
/// mirrored by hand, and this is the only thing tying the copies together.
///
/// The colours themselves are not pinned here because they are not mirrored: they live in
/// `resolve.wgsl` alone, since nothing on this side of the wire has an opinion about them.
/// A wrong colour is visible in one capture; a wrong `BAND` is not, which is what makes
/// these four the ones worth an assert.
const _: () = assert!(biome::BAND == 0.22 && biome::BLEND == 0.18);
const _: () = assert!(biome::TEMP_SEED == 7 && biome::HUMID_SEED == 8);
#[rustfmt::skip]
const _: () = assert!(
    biome::GRID[0][0] == biome::TUNDRA && biome::GRID[0][1] == biome::PLAINS && biome::GRID[0][2] == biome::DESERT &&
    biome::GRID[1][0] == biome::TAIGA  && biome::GRID[1][1] == biome::PLAINS && biome::GRID[1][2] == biome::SAVANNA &&
    biome::GRID[2][0] == biome::TAIGA  && biome::GRID[2][1] == biome::FOREST && biome::GRID[2][2] == biome::FOREST
);

#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable)]
struct GpuFrame {
    cam_pos: [f32; 3],
    time: f32,
    cam_fwd: [f32; 3],
    tan_half_fov: f32,
    cam_right: [f32; 3],
    aspect: f32,
    cam_up: [f32; 3],
    far: f32,
    sun_dir: [f32; 3],
    daylight: f32,
    res: [u32; 2],
    tiles: [u32; 2],
    chunk_count: u32,
    camera_chunk: u32,
    flags: u32,
    max_pairs: u32,
    fog_density: f32,
    fog_falloff: f32,
    shadow_dist: f32,
    ambient: f32,
    fog_scatter: f32,
    fog_g: f32,
    fog_height: f32,
    mip_bias: f32,
    grid_min: [i32; 3],
    sea_level: f32,
    grid_dim: [u32; 3],
    water_absorb: f32,
    jitter: [f32; 2],
    taa_feedback: f32,
    frame_index: u32,
    cloud_cover: f32,
    cloud_height: f32,
    cloud_scale: f32,
    cloud_speed: f32,
    godray_strength: f32,
    godray_steps: u32,
    godray_dist: f32,
    cloud_shadow: f32,
    tint_strength: f32,
    tint_seed: i32,
    tint_freq_t: f32,
    tint_freq_h: f32,
    wave_amp: f32,
    wave_scale: f32,
    wave_speed: f32,
    wave_reflect_slope: f32,
    shaft_origin: [f32; 2],
    shaft_texel: f32,
    shaft_soft: f32,
    cloud_patch: f32,
    cloud_relief: f32,
    haze_warm: f32,
    zenith_deep: f32,
    prev_view_proj: [[f32; 4]; 4],
}

// Nothing validates the Rust struct against the WGSL one -- `min_binding_size` is None and
// naga never sees this side -- so pin the size. The mat4x4 wants 16-byte alignment, which
// is what `jitter_pad` buys: jitter at 176, the matrix at 192. `fog_pad` buys the same for
// `grid_min` at 144, which WGSL aligns as a vec3.
//
// Batch 8 spent the last two of those alignment pads: `grid_pad` became `sea_level` and
// `grid_pad2` became `water_absorb`. Both still sit in the slot a vec3 leaves behind, so
// nothing moved and the assert did not have to.
//
// Batch 10 had no pad left to spend and did the only other thing available: four cloud
// parameters appended before the matrix, which pushes `prev_view_proj` from 192 to 208 and
// the struct from 256 to 272. 208 is still a multiple of 16, which is all the mat4x4 asks.
//
// Batch 11 did the same thing again for the same reason, and *four* is again the number:
// `godray_strength`, `godray_steps`, `godray_dist` and `cloud_shadow` take 208..224, the
// matrix moves to 224 and the struct to 288. Any other count would have needed a pad to
// keep the mat4x4 16-byte aligned.
//
// Batch 12 is the third in a row, and four again: `tint_strength`, `tint_seed` and the two
// biome frequencies take 224..240, the matrix moves to 240 and the struct to 304. The
// frequencies ride here rather than sitting as literals in the shader for a reason -- they
// are two of the constants the WGSL biome field would otherwise have to mirror, and a
// number that travels cannot drift.
// Batch 16 is the fourth, and four for the fourth time: `wave_amp`, `wave_scale`,
// `wave_speed` and `wave_reflect_slope` take 240..256, the matrix moves to 256 and the
// struct to 320. By now the count is not a coincidence being noticed -- it is the rule,
// because 16-byte alignment is what the mat4x4 asks and four f32s are what satisfy it
// without inventing a pad.
//
// Batch 35 is the fifth, and four for the fifth time: `shaft_origin` (a vec2, so two words),
// `shaft_texel` and `shaft_soft` take 256..272, the matrix moves to 272 and the struct to
// 336. The two that are constants on the Rust side ride here rather than being mirrored as
// WGSL literals for `tint_freq_t`'s reason -- a number that travels cannot drift -- and
// `shaft_origin` has to travel whatever happens, because the window is anchored where the
// field was last filled and not where this frame's camera is.
//
// Batch 95 is the sixth, four for the sixth time -- G1's four look knobs take 272..288,
// the matrix moves to 288 and the struct to 352. The WGSL block spells out what each is;
// what matters here is that all four default to 0, because the pre-95 frame is the one
// control the whole look goal is judged against.
const _: () = assert!(std::mem::size_of::<GpuFrame>() == 352);
// The size alone cannot catch a field inserted in the wrong place -- swap two and it is
// unchanged -- and nothing else compares this struct to the WGSL one. These seven pin the
// seams the six append batches actually moved.
const _: () = assert!(std::mem::offset_of!(GpuFrame, cloud_cover) == 192);
const _: () = assert!(std::mem::offset_of!(GpuFrame, godray_strength) == 208);
const _: () = assert!(std::mem::offset_of!(GpuFrame, tint_strength) == 224);
const _: () = assert!(std::mem::offset_of!(GpuFrame, wave_amp) == 240);
const _: () = assert!(std::mem::offset_of!(GpuFrame, shaft_origin) == 256);
const _: () = assert!(std::mem::offset_of!(GpuFrame, cloud_patch) == 272);
const _: () = assert!(std::mem::offset_of!(GpuFrame, prev_view_proj) == 288);

#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable)]
struct GpuChunk {
    aabb_min: [f32; 3],
    voxel_size: f32,
    aabb_max: [f32; 3],
    lod: u32,
    origin: [f32; 3],
    /// Signed dither threshold; `RenderItem::fade` copied straight through. This slot was
    /// a `flags` word that was written zero and never read.
    fade: f32,
    root: [u32; 4],
    attr_base: u32,
    /// Bit 0: the whole chunk is one block id, held in `attr_base`. Bit 1: the chunk holds
    /// water somewhere, which is what lets the shading passes skip the attribute lookup
    /// that tells water apart from opaque geometry on every chunk that has none.
    attr_flags: u32,
    light_base: u32,
    light_flags: u32,
    /// `root` 's 64-bit occupancy mask with every 16^3 cell holding nothing but water cleared
    /// (batch 53). `march_chunk` tests this one instead of `root`'s whenever the ray ignores
    /// water, which is the submerged primary ray and **every** secondary ray in the frame.
    ///
    /// It is the traversal mask only. The **index** into `inners` still comes from `root`'s
    /// own mask, because that is what the interned run is laid out against -- swapping both
    /// would read a neighbouring node and the picture would be plausible.
    dry_mask: [u32; 2],
    /// **Named rather than implied, because this struct now has slack and a later batch should
    /// know where it is.** A `vec3<f32>` gives `Chunk` 16-byte alignment, so the array stride
    /// is a multiple of 16 whatever is declared: adding `dry_mask`'s 8 bytes took the struct
    /// from 80 to 88 and the stride to 96 regardless. Two words spare here, plus `root[3]` --
    /// the root `Inner`'s `leaf_prefix`, which `make_root` writes 0 and no shader reads.
    /// **Three free words, and unlike `GpuFrame` this struct can also simply grow**: 8,191
    /// chunks at 96 bytes is 786 KB against 4 GB of which `CLAUDE.md` measures 97% free.
    _reserved: [u32; 2],
}

/// Atmosphere: exponential height fog plus the forward-scattering lobe it shares with the
/// sky. Density at altitude `y` is `density * exp(-falloff * (y - height))`.
#[derive(Clone, Copy, Debug)]
pub struct Fog {
    /// Extinction coefficient per block at `height`. Zero disables the haze entirely.
    pub density: f32,
    /// Reciprocal scale height, per block. Zero is constant density at every altitude.
    pub falloff: f32,
    /// Strength of the Henyey-Greenstein lobe. It is counted once: `sky_color` adds it and
    /// the fog mixes toward `sky_color`, so raising it warms the haze and the sky together.
    pub scatter: f32,
    /// HG asymmetry. 0 is isotropic, 1 a needle along the sun; the useful band is 0.75-0.85.
    pub g: f32,
    /// The altitude `density` is quoted at, in **internal** world Y (0..512), not the
    /// displayed Y. Sea level, so a valley floor is the reference and peaks sit above it.
    pub height: f32,
}

impl Default for Fog {
    fn default() -> Self {
        Self {
            density: 5.0e-4,
            falloff: 1.0 / 64.0,
            scatter: 0.020,
            g: 0.80,
            height: crate::worldgen::SEA_LEVEL as f32,
        }
    }
}

/// The cloud deck: one horizontal plane of noise, composited into the sky miss and into a
/// water reflection that misses. Nothing here is stored per chunk, because a cloud touches
/// nothing the world holds.
#[derive(Clone, Copy, Debug)]
pub struct Clouds {
    /// Coverage threshold, 0 to 1. **Zero is the control**: `sky_color` returns `sky_base`
    /// by an early exit, so the frame is bit-identical to the pre-batch-10 build and costs
    /// what it cost then. It is a threshold on a bell-shaped noise rather than the sky
    /// fraction it produces; `PERF.md` carries the measured curve between the two.
    pub cover: f32,
    /// Altitude of the deck in **internal** world Y (0..512), like `Fog::height` and
    /// `sea_level` and unlike the Y the HUD prints. The default is 128 blocks above the
    /// tallest terrain the generator makes, so every documented vantage is underneath it.
    pub height: f32,
    /// Blocks per unit of the base octave, so the finest of the four is an eighth of this.
    /// Clamped away from zero on the way to the GPU, which divides by it.
    pub scale: f32,
    /// Blocks per second of drift. It multiplies `FrameParams::time`, which every headless
    /// mode pins to zero, so the deck moves in the game and a capture stays deterministic.
    pub speed: f32,
    /// G1 (`--cloud-patch`, batch 95): cumulus arrive in *banks*; this is how far a coarse
    /// sixth-of-the-span octave swings the coverage threshold, 0 to 1. **Zero is the
    /// control** and is guarded in the shader, so the pre-95 deck is bit-exact.
    pub patch: f32,
    /// G1 (`--cloud-relief`, batch 95): sun-side shading that fakes verticality on a flat
    /// plane -- lighting only, it can darken a texel's `depth` but never add density.
    /// **Zero is the control**, guarded the same way.
    pub relief: f32,
}

impl Default for Clouds {
    fn default() -> Self {
        Self {
            cover: 0.45,
            height: 448.0,
            scale: 320.0,
            speed: 3.0,
            patch: 0.0,
            relief: 0.0,
        }
    }
}

/// The two G1 levers that are the sky's rather than the deck's (batch 95, roadmap G1):
/// the warm horizon band over the treeline, and the richer blue upstairs. Both are
/// strength-of-effect, both default to 0 -- the pre-95 sky is the control every one of
/// the look goal's verdicts is read against -- and both are guarded at their use sites
/// in `sky_base`, never folded to a multiply-by-zero.
#[derive(Clone, Copy, Debug)]
pub struct SkyLook {
    /// Warm band hugging the horizon, sun-tinted and day-weighted, 0 to 1.
    pub haze_warm: f32,
    /// How much bluer the zenith runs than `SKY_DAY_ZENITH` at day, 0 to 1.
    pub zenith_deep: f32,
}

impl Default for SkyLook {
    fn default() -> Self {
        Self {
            haze_warm: 0.0,
            zenith_deep: 0.0,
        }
    }
}

/// Crepuscular rays, and the cloud shadow that has to come with them.
///
/// Batch 11 adds no radiance of its own. The haze already carries one Henyey-Greenstein
/// lobe of in-scattered sunlight -- `resolve` mixes a fogged surface toward `sky_base(rd)`
/// and `sky_base` adds `scatter_lobe(rd)` -- and that term was *unshadowed*, which put a
/// warm patch on the shadowed face of the very ridge blocking the sun from the air in front
/// of it. What this struct configures is the visibility that lobe is multiplied by.
#[derive(Clone, Copy, Debug)]
pub struct GodRays {
    /// How far the shadowing is taken, 0 to 1. **Zero is the control**: `sun_shaft` returns
    /// 1.0 by an early exit, so the lobe is untouched, the frame is bit-identical to the
    /// pre-batch-11 build, and no step is walked. Above 1 deepens the shafts past physical.
    pub strength: f32,
    /// Samples along the view ray, placed by importance rather than spaced evenly. The one
    /// number that sets the cost, which is why it is a flag and not a constant: this is the
    /// item in the roadmap with no bound by construction, so the bound has to be measurable.
    /// Four is the knee -- against a 32-sample reference it leaves 0.17% of the frame off by
    /// more than a code value, where six evenly spaced samples left 1.8% for the same money.
    pub steps: u32,
    /// How far along the view ray in blocks the samples are spread. **A resolution knob and
    /// not a cost one**, which is why it defaults past the far plane: the occluder is
    /// analytic, so asking about a point ten kilometres out costs what a near one costs, and
    /// truncating a horizon ray measurably biases it -- at six steps, cutting the reach to
    /// 1024 moves 3.0% of the frame against the untruncated answer where the full reach moves
    /// 1.8%. What a shorter reach buys is a fixed number of samples concentrated in the near
    /// field, and the shaft is then scaled by the share of the path's whole in-scattering
    /// that lay inside it, which is what keeps asking for one honest.
    pub dist: f32,
    /// How much of the sun the cloud deck's own opacity takes away, 0 to 1, used both by the
    /// shaft and by `shade_hit`'s sun term. **Zero is its own control**, separately
    /// bit-exact, and it is clamped to 0..1 on the way to the GPU because more than all of
    /// the sun is negative radiance. It is below 1 because a cloud scatters light through
    /// rather than stopping it dead, so a fully opaque one still leaves the ground dim
    /// rather than black.
    pub cloud_shadow: f32,
}

impl Default for GodRays {
    fn default() -> Self {
        Self {
            strength: 1.0,
            steps: 4,
            dist: 4096.0,
            cloud_shadow: 0.85,
        }
    }
}

/// The per-biome vegetation tint (batch 12). Whether it runs at all is `FLAG_TINT`, not a
/// field here.
#[derive(Clone, Copy, Debug)]
pub struct Tint {
    /// How far the biome's own multiplier is taken. 1.0 is the authored value; 0 leaves
    /// every albedo exactly where the atlas put it, but still pays for the field -- use the
    /// flag, not this, to measure the cost.
    pub strength: f32,
    /// The world seed `WorldGen` was built with. The shader evaluates the same field from
    /// world XZ, so it needs the same seed; `biome::TEMP_SEED` and `biome::HUMID_SEED` are
    /// added on the shader side.
    pub seed: i32,
}

/// Water micro-normals (batch 16). Whether the field is compiled into `resolve` at all is
/// `FLAG_WAVES`, not `amp` here -- see that constant for why the two travel together.
///
/// The field is a *slope* field and never a height field. Nothing is displaced, because the
/// visibility buffer holds one flat voxel face per pixel and `hit_t` re-derives the hit
/// point from that face for both `resolve` and `taa`; a displaced surface would have to be
/// traced, stored and reprojected, and there is nowhere in the key to put it. So this
/// perturbs shading and nothing else, which is the whole reason it is affordable.
#[derive(Clone, Copy, Debug)]
pub struct Waves {
    /// Peak slope of the summed octaves, dimensionless -- the tangent of the steepest tilt
    /// the unfiltered normal reaches. 0 is exactly the batch-8 sheet of glass.
    pub amp: f32,
    /// Blocks per period of the base octave. The other three run at 2.17, 4.71 and 10.2
    /// times its frequency: ratios chosen irrational-ish so the sum has no period short
    /// enough for a still frame to read as a grid.
    pub scale: f32,
    /// Periods per second the phases advance. It multiplies `frame.time`, which every
    /// headless mode pins to zero -- so the sea moves in the game and a capture stays the
    /// deterministic single image the A/B method is built on. Exactly the arrangement
    /// `cloud_speed` already has, and for exactly the same reason.
    pub speed: f32,
    /// The largest slope the *traced* reflection's normal may take, as a tangent, and it
    /// **defaults to zero** -- the traced reflection keeps the flat normal, exactly as the
    /// refraction does.
    ///
    /// That is a measurement and not caution. Batch 8 declined a wavy normal because it
    /// would take the secondary rays' coherence with it, and batch 16 set out to buy the
    /// wobble back for the terms that never traverse. It found a second reason the traced
    /// ray has to stay flat, and a harder one: a traced reflection is a *hit or a miss*,
    /// one bit per pixel with no filter over it, so bending it by an angle large enough to
    /// see puts salt-and-pepper along every reflected shoreline -- visible at 0.008, ugly
    /// at 0.020, and gone at 0. The normal's own aliasing is fixed by the band limit; this
    /// one cannot be, because there is no mip chain over a ray cast.
    ///
    /// The knob stays because it is the A/B partner for that claim: `--wave-clamp 0.02`
    /// reproduces the speckle in one capture. What it would take to raise it is a
    /// reflection that can be pre-filtered -- a cone trace, or a blurred reflection buffer
    /// -- and neither is this batch.
    pub reflect_slope: f32,
}

impl Default for Waves {
    fn default() -> Self {
        Self {
            amp: 0.12,
            scale: 32.0,
            speed: 0.32,
            reflect_slope: 0.0,
        }
    }
}

/// Everything the renderer needs about the camera and time for one frame.
pub struct FrameParams {
    pub cam_pos: Vec3,
    pub cam_fwd: Vec3,
    pub cam_right: Vec3,
    pub cam_up: Vec3,
    pub fov_y: f32,
    pub far: f32,
    pub sun_dir: Vec3,
    pub daylight: f32,
    pub time: f32,
    pub flags: u32,
    /// The second specialization word, CPU-side only -- see [`SPEC_HI_MASK`]. It keys a
    /// pipeline and is never uploaded, so every bit in it must be one no shader reads.
    pub spec_hi: u32,
    pub ambient: f32,
    pub fog: Fog,
    /// The cloud deck. `Clouds::cover` 0 is the batch-10 control and is bit-exact.
    pub clouds: Clouds,
    /// Batch 95's G1 sky levers. `SkyLook::default()` -- both zero -- is the control and
    /// is bit-exact against the pre-95 build by the shader's own guards.
    pub sky: SkyLook,
    /// Crepuscular rays and cloud shadow. `GodRays::strength` 0 and `cloud_shadow` 0 are
    /// the batch-11 controls, and both are bit-exact.
    pub godrays: GodRays,
    /// Internal world Y of the water surface. The underwater path needs the plane, not the
    /// voxels: from inside the medium the marcher cannot find the boundary, because a
    /// water-to-air transition is not an occupancy edge.
    pub sea_level: f32,
    /// Multiplier on water's per-channel extinction; 0 is perfectly clear.
    pub water_absorb: f32,
    /// Static sub-pixel offset for every primary ray, in pixels. `None` walks the eight
    /// Halton phases when `taa` is on and sits on the plain grid when it is not.
    pub jitter: Option<Vec2>,
    /// Run the temporal pass. Off leaves `resolve`'s output on screen untouched.
    pub taa: bool,
    /// Extra mip bias on top of the render-scale one the renderer derives itself.
    pub mip_bias: f32,
    /// The per-biome vegetation tint. Switched off through `FLAG_TINT` in `flags`.
    pub tint: Tint,
    /// The water micro-normals. Switched off through `FLAG_WAVES` in `flags`.
    pub waves: Waves,
    /// World XZ of texel (0, 0) of the light envelope's height window, from
    /// `ShaftField::origin_world`. Switched off through `FLAG_TERRAIN_SHAFTS` in `flags`.
    pub shaft_origin: [f32; 2],
    /// Blocks per envelope texel, from `ShaftField::texel`. A uniform since batch 35 by
    /// design -- **the field and the frame must agree, and the constructor is where they
    /// get told**; batch 92 is merely the first caller that says anything but 4.
    pub shaft_texel: f32,
    /// Run the envelope's scan this frame. False skips eight dispatches, and the caller sets
    /// it false for exactly the cases where nothing would read the result: the feature is off,
    /// no shadowing is asked for, or the sun is below the horizon.
    pub shaft_scan: bool,
}

pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter: wgpu::Adapter,
    pub has_timestamps: bool,
}

pub fn make_instance() -> wgpu::Instance {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::VULKAN | wgpu::Backends::DX12;
    wgpu::Instance::new(desc)
}

/// The adapter must come from the same instance that owns the surface, and that
/// instance has to outlive the surface, so the caller owns it.
pub async fn init_gpu(
    instance: &wgpu::Instance,
    compatible: Option<&wgpu::Surface<'static>>,
) -> anyhow_lite::Result<Gpu> {
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: compatible,
            force_fallback_adapter: false,
            ..Default::default()
        })
        .await
        .map_err(|e| format!("no suitable GPU adapter: {e}"))?;

    let required = wgpu::Features::SHADER_INT64 | wgpu::Features::SHADER_INT64_ATOMIC_MIN_MAX | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
    let available = adapter.features();
    if !available.contains(required) {
        return Err(format!(
            "GPU {} lacks required native texture-format features or 64-bit shader atomics (SHADER_INT64 | SHADER_INT64_ATOMIC_MIN_MAX), which the \
             visibility buffer needs",
            adapter.get_info().name
        )
        .into());
    }
    for format in [wgpu::TextureFormat::R16Float,wgpu::TextureFormat::Rg16Float,wgpu::TextureFormat::R8Unorm] {
        let uses=wgpu::TextureUsages::STORAGE_BINDING|wgpu::TextureUsages::TEXTURE_BINDING;
        if !adapter.get_texture_format_features(format).allowed_usages.contains(uses) {
            return Err(format!("GPU lacks sampled/storage support for shadow format {format:?}").into());
        }
    }
    let has_timestamps = available.contains(wgpu::Features::TIMESTAMP_QUERY);
    let features = required | (available & wgpu::Features::TIMESTAMP_QUERY);
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("voxelcraft"),
            required_features: features,
            required_limits: adapter.limits(),
            ..Default::default()
        })
        .await
        .map_err(|e| format!("device request failed: {e}"))?;
    Ok(Gpu {
        device,
        queue,
        adapter,
        has_timestamps,
    })
}

/// Minimal error type so the crate needs no error-handling dependency.
pub mod anyhow_lite {
    pub type Error = Box<dyn std::error::Error + Send + Sync>;
    pub type Result<T> = std::result::Result<T, Error>;
}

struct Pipelines {
    tile_select: wgpu::ComputePipeline,
    recover_select: wgpu::ComputePipeline,
    finalize: wgpu::ComputePipeline,
    finalize_recover: wgpu::ComputePipeline,
    finalize_deferred: wgpu::ComputePipeline,
    build_hiz: wgpu::ComputePipeline,
    taa: wgpu::ComputePipeline,
}

/// The two passes that compile `SPEC_MASK` into different code, built together because they
/// have to agree: `foliage_aware` lives in `common.wgsl`, which both of them include, so one
/// `override` value serves both and a run where they differed would march a tuft the shading
/// pass cannot see. The other seven entry points are bit-identical across every value in the
/// mask -- `shaderstats` says so, byte for byte -- and stay in `Pipelines`, compiled once.
struct SpecPipes {
    primary_shadow: Option<wgpu::ComputePipeline>,
    shadow_blurs: [wgpu::ComputePipeline;2],
    glass_resolve: Option<wgpu::ComputePipeline>,
    water_resolve: Option<wgpu::ComputePipeline>,
    march: wgpu::ComputePipeline,
    resolve: wgpu::ComputePipeline,
    /// Batch 81 (P12): the half-res leg evaluation feeding `resolve`'s shared arm. In the
    /// per-spec pair -- not `Pipelines` -- because it compiles `water_ctx` and both legs
    /// with the same overrides `resolve` sees, and two spec words that disagreed about
    /// `SPEC_WAVES` would trace different directions off the same donor.
    water_sec: wgpu::ComputePipeline,
}

pub struct Renderer {
    pub gpu: Gpu,
    layout: wgpu::BindGroupLayout,
    pipes: Pipelines,
    /// The one compiled module and the layout every pipeline is built against, kept alive
    /// because `resolve` is no longer built once in `new`. See `SPEC_MASK`.
    module: wgpu::ShaderModule,
    pipeline_layout: wgpu::PipelineLayout,
    /// `march` and `resolve`, one entry per specialization actually asked for. Four at most
    /// today, and in a given run always one: the flags in `SPEC_MASK` do not move after
    /// startup, so the cache exists to make that fact cheap rather than to exploit variation.
    spec_pipes: Vec<((u32, u32), SpecPipes)>,
    bind_group: wgpu::BindGroup,
    /// One per parity: the temporal pass writes `hist[i]` and reads `hist[i ^ 1]`, and a
    /// texture cannot be a write storage binding and a sampled binding in one bind group.
    taa_bind_group: [wgpu::BindGroup; 2],
    bind_dirty: bool,

    frame_buf: wgpu::Buffer,
    chunk_buf: DynBuffer,
    grid_buf: wgpu::Buffer,
    inners: GpuMirror,
    leaves: GpuMirror,
    bricks: GpuMirror,
    faces: wgpu::Buffer,
    pairs: DynBuffer,
    /// This screen's pair-list capacity; see [`pairs_for`].
    max_pairs: u32,
    counters: wgpu::Buffer,
    indirect_storage: wgpu::Buffer,
    indirect: wgpu::Buffer,
    vis: DynBuffer,
    hiz: DynBuffer,
    dbg: DynBuffer,
    /// The light envelope (batch 35): two ping-pong slices and the static heights, in one
    /// buffer. Fixed size, because the window is a fixed number of texels -- it scrolls
    /// under the camera rather than growing with anything.
    shaft_buf: wgpu::Buffer,
    /// One pipeline per doubling of the scan, differing only in the `SHAFT_STEP` override.
    /// Built here rather than keyed like `spec_pipes` because the count is a constant and
    /// every run uses all of them.
    shaft_pipes: Vec<wgpu::ComputePipeline>,

    out_tex: wgpu::Texture,
    out_view: wgpu::TextureView,
    /// log2 of the water legs' block edge (batch 81c; 0/1/2 for scale 1/2/4).
    pub water_sec_shift: u32,
    /// Batch 81 (P12)'s half-res `(under, depth)` / `(lit, validity)` pair, kept alive
    /// beside their views the way `probe_tex` is: the bind group holds the views, and a
    /// resize remakes all four. Half the trace dimensions in each axis, matching the
    /// `water_sec` entry's coverage convention exactly.
    water_sec_view: (wgpu::TextureView, wgpu::TextureView),
    water_sec_tex: (wgpu::Texture, wgpu::Texture),
    /// Batch 81b's groups on the water pair, one per role, and the layouts that built
    /// them (kept because a `resize` remakes the views and therefore these groups). The
    /// pair never shares a group; see the group(1)/group(2) declarations' comment.
    water_read_group: wgpu::BindGroup,
    water_store_group: wgpu::BindGroup,
    water_read_layout: wgpu::BindGroupLayout,
    water_store_layout: wgpu::BindGroupLayout,
    /// Group 1 *of water_sec's layout*, holding nothing, so the layout can name group 2.
    water_empty_group: wgpu::BindGroup,
    shadow_targets: shadow::Targets,
    shadow_layouts: [wgpu::BindGroupLayout;3],
    shadow_pipe_layouts: [wgpu::PipelineLayout;3],
    split_layout: wgpu::BindGroupLayout,
    /// The resolve and water_sec pipeline layouts (march and the long-lived passes keep
    /// the plain [`pipeline_layout`]). Rebuilt in `ensure_spec` reads, so kept on self.
    water_read_pipe_layout: wgpu::PipelineLayout,
    water_sec_pipe_layout: wgpu::PipelineLayout,
    /// Ping-ponged temporal history, same format as `out_tex` so the accumulation stays in
    /// the linear HDR the blit tone-maps from.
    hist_view: [wgpu::TextureView; 2],
    atlas_view: wgpu::TextureView,
    /// Batch 57's ambient-cube lattice, and batch 55's constant field when pinned -- the only reason it
    /// is a texture rather than a number is that the quantity being measured is the tap.
    probe_view: wgpu::TextureView,
    /// The lattice itself, kept beside its view because a bake writes into it every time a
    /// chunk lands -- which is the difference between batch 55's field and this one.
    probe_tex: wgpu::Texture,
    /// **Repeat in U and W, clamp in V**, which is the sampler encoding the field's shape:
    /// X and Z are toroidal so a tap at the lattice seam filters across it instead of
    /// clamping, and Y spans the world exactly so its edges are the world's own. The shared
    /// `linear_sampler` clamps on every axis and would put a four-block band of wrong
    /// shading on one plane every 512 blocks.
    probe_sampler: wgpu::Sampler,
    /// `--probe-fill` was given, so the field is a constant and no bake may overwrite it.
    /// Batch 55 and 56's diagnostics are the only readers of that state and the reason it
    /// exists; see [`make_probe`].
    probe_pinned: bool,
    atlas_sampler: wgpu::Sampler,
    linear_sampler: wgpu::Sampler,

    pub blit: blit::BlitPass,
    pub ui: ui::UiRenderer,
    pub timer: Readback,

    /// Internal render resolution.
    pub size: (u32, u32),
    /// Window resolution, which the overlay is laid out against.
    pub surface_size: (u32, u32),
    tiles: (u32, u32),
    surface_format: wgpu::TextureFormat,
    grid_min: IVec3,

    chunk_scratch: Vec<GpuChunk>,
    grid_scratch: Vec<u32>,
    /// Whether the shadow/AO grid resolves a cross-fade to the half carrying most of the
    /// rays, rather than to the fine half unconditionally. Batch 62, roadmap D2;
    /// `--no-shadow-share` clears it and restores the pre-62 ordering exactly.
    ///
    /// **Not a `frame.flags` bit and not in `SPEC_MASK`**, because no shader reads it: what
    /// it changes is which chunk index the CPU writes into `grid`. So the control is free by
    /// construction rather than by specialization, which is `--no-fade`'s shape.
    shadow_share: bool,
    /// Whether a chunk the view frustum rejects is still uploaded and written into `grid`.
    /// Batch 64, roadmap D3; `--no-offscreen-shadows` clears it and restores the pre-64 set
    /// exactly -- with it false the off-screen list is empty, so `chunk_scratch` holds the
    /// same entries in the same order and the fill runs over the same indices.
    ///
    /// **Not a `frame.flags` bit and not in `SPEC_MASK`, for `shadow_share`'s reason**: no
    /// shader reads it, and what it changes is which chunks the CPU uploads at all.
    offscreen_shadows: bool,
    /// Entries `tile_select` box-tests: the frustum-culled length, and what `frame.chunk_count`
    /// carries.
    pub last_chunk_count: usize,
    /// Entries uploaded, drawn and shadow-only together. It is `last_chunk_count` exactly when
    /// `offscreen_shadows` is off, and the gap between the two is what batch 64 added.
    pub last_grid_chunks: usize,
    pub last_camera_chunk: Option<usize>,
    /// Last frame's world -> clip matrix; `None` until the first frame has been submitted.
    prev_view_proj: Option<Mat4>,
    /// Drives the jitter phase and the history parity.
    frame_index: u64,
    /// False whenever the history holds nothing worth reprojecting: the first frame, the
    /// frame after a resize, and the frame after the temporal pass was switched off.
    history_valid: bool,
    /// Which of the blit's source slots the last full render presented from: `0` for
    /// `out_tex` with the temporal pass off, `1 + parity` for the history slice just
    /// converged. Batch 70's [`Renderer::repaint`] re-presents exactly that slot and no
    /// pass runs between the two presents, so the texture is still the one the index
    /// named then -- which is what makes a repaint bit-identical rather than merely close.
    last_blit: usize,
}

impl Renderer {
    /// `water_mottle` is an *atlas* constant and so belongs here rather than in `GpuFrame`:
    /// the texture is built once, and nothing in the frame loop can change it. 0 ships and 0.2
    /// reproduces the pre-batch-21b atlas -- see `textures::WATER_MOTTLE_PRE21B`.
    ///
    /// `lod` arrives here for a *different* reason, which is worth the sentence: the two fields
    /// taken off it are per-frame policies rather than build-once constants, so either could
    /// have been a setter. They are constructor state so that the compiler names every call
    /// site when one is added -- a setter one of the three headless paths forgot to call would
    /// be a control that silently does nothing on that path, which is the one failure mode
    /// indistinguishable from success.
    ///
    /// `tint_balance` (batch 92, roadmap A8) is the same class once more -- an atlas
    /// constant, because a tint mask is content the texture file has to hold. It sits
    /// beside `water_mottle`, the other build-once texture argument.
    ///
    /// **It is the whole `LodConfig` rather than the two `bool`s because batch 64 would have
    /// made them two adjacent `bool`s**, and three call sites passing `a, b` positionally is a
    /// swap nothing catches: both compile, both render, and each is the other's control. Only
    /// `shadow_share` and `offscreen_shadows` are read here; the rest of that struct belongs to
    /// the planner and must not be consulted from the renderer, where it would go stale against
    /// the `ChunkManager` that actually owns it.
    ///
    /// `probe_fill` and `probe_noise` are batch 55's and arrive here for exactly the same
    /// reason: the probe field is built once and nothing in the frame loop can change it. 1.0
    /// ships, and it is 1.0 rather than any other number because `amb * 1.0` is bit-exact --
    /// the tap has to be invisible for its cost to be the only thing it reports. `probe_noise`
    /// fills per texel instead, which is the build that rules out the driver compressing a
    /// constant texture into a cost nobody would pay for a real field.
    pub fn new(
        gpu: Gpu,
        size: (u32, u32),
        surface_format: wgpu::TextureFormat,
        _water_mottle: f32,
        tint_balance: bool,
        probe_fill: Option<f32>,
        probe_noise: bool,
        lod: crate::lod::LodConfig,
        // Batch 81c (P12's fraction sweep): log2 of the water legs' block edge, 0..2
        // for `--water-sec-scale` 1/2/4. Config-fixed at startup, so it keys nothing --
        // the override is baked per `make_spec` call.
        water_sec_shift: u32,
        // A10 (`--tone-map`): 0 for the knee every constant was tuned against, 1 for the
        // Narkowicz ACES fit. Presentation-only, set once: two pipelines for a shoulder
        // choice would be machinery for nothing.
        tone: u32,
        // G1's warm grade (`--grade`, batch 95): same presentation-only one-uniform rule.
        grade: u32,
        // Batch 101's `--grade-strength`: the grade dial, passed through to the blit
        // uniform's last padding word. Default 1.0 makes the lerp branch never run --
        // the shipping grade frame is the batch-97 instruction stream bit for bit.
        grade_strength: f32,
    ) -> Self {
        let device = &gpu.device;
        let queue = &gpu.queue;
        let size = (size.0.max(8), size.1.max(8));
        let tiles = (size.0.div_ceil(TILE), size.1.div_ceil(TILE));

        // ---- shaders: one module, many entry points, so `common.wgsl` compiles once.
        let source = shader_source();
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("voxel passes"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });

        let layout = make_layout(device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("voxel layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        // Batch 81b: three layouts encoding the validator's rule (see the group(1)/group(2)
        // declarations in common.wgsl). `wread` is resolve's guest pair, `wstore` is
        // water_sec's own pair, and the empty one exists so `water_sec`'s pipeline layout
        // can *span* group 1 without binding anything there -- the one texture may hold no
        // second role anywhere in a dispatch, and a placeholder pointing at it would be one.
        let wread_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("water sec read"),
            entries: &[sampled_entry(0), sampled_entry(1)],
        });
        let wstore_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("water sec store"),
            entries: &[storage_tex_entry(0), storage_tex_entry(1)],
        });
        let wempty_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("water sec group-1 placeholder"),
            entries: &[],
        });
        let split_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("primary sun mask R8"), entries: &[shadow::sampled(0)],
        });
        let shadow_layouts=shadow::layouts(device);
        let shadow_pipe_layouts=std::array::from_fn(|i|device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label:Some("shadow stage pipeline layout"),
            bind_group_layouts:&[Some(&layout),Some(&wempty_layout),Some(&wempty_layout),Some(&shadow_layouts[i])],
            immediate_size:0,
        }));
        let shadow_targets=shadow::Targets::new(device,(1,1),&split_layout,&shadow_layouts);
        let water_read_pipe_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("resolve layout"),
                bind_group_layouts: &[Some(&layout), Some(&wread_layout), Some(&wempty_layout), Some(&split_layout)],
                immediate_size: 0,
            });
        let water_sec_pipe_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("water sec layout"),
                bind_group_layouts: &[Some(&layout), Some(&wempty_layout), Some(&wstore_layout)],
                immediate_size: 0,
            });
        let wempty_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("water sec group-1 placeholder"),
            layout: &wempty_layout,
            entries: &[],
        });
        let mk = |name: &'static str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(name),
                layout: Some(&pipeline_layout),
                module: &module,
                entry_point: Some(name),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let pipes = Pipelines {
            tile_select: mk("tile_select"),
            recover_select: mk("recover_select"),
            finalize: mk("finalize"),
            finalize_recover: mk("finalize_recover"),
            finalize_deferred: mk("finalize_deferred"),
            build_hiz: mk("build_hiz"),
            taa: mk("taa"),
        };
        // One pipeline per doubling of the light envelope's scan. The step index is an
        // `override` and not a uniform for the reason every override here is one: it is fixed
        // per pipeline, so `1u << k` and the two slice bases all fold and the pass comes out
        // as a load, a lerp, a subtract and a max. Eight compiles at startup, which is where
        // `SPEC_MASK`'s own argument puts them -- a pipeline compile is startup cost, and no
        // GPU pass time in `PERF.md` can see it.
        let shaft_pipes: Vec<wgpu::ComputePipeline> = (0..crate::shaft::STEPS)
            .map(|k| {
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("shaft scan"),
                    layout: Some(&pipeline_layout),
                    module: &module,
                    entry_point: Some("shaft_scan"),
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants: &[("SHAFT_STEP", k as f64)],
                        ..Default::default()
                    },
                    cache: None,
                })
            })
            .collect();

        // Compile specialized pipelines from the actual first FrameParams in render().
        // Bootstrapping SPEC_HI_MASK would compile every opt-in effect even when off.
        // ensure_spec still warms the actual emitter sibling before the first dispatch.
        let spec_pipes = Vec::new();

        // ---- buffers
        use wgpu::BufferUsages as U;
        let frame_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame"),
            size: std::mem::size_of::<GpuFrame>() as u64,
            usage: U::UNIFORM | U::COPY_DST,
            mapped_at_creation: false,
        });
        let chunk_buf = DynBuffer::new(
            device,
            "chunk list",
            (MAX_CHUNKS * std::mem::size_of::<GpuChunk>()) as u64,
            U::STORAGE,
        );
        let grid_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("chunk grid"),
            size: (GRID_LEN * 4) as u64,
            usage: U::STORAGE | U::COPY_DST,
            mapped_at_creation: false,
        });
        let inners = GpuMirror::new(device, "inner nodes", U::STORAGE);
        let leaves = GpuMirror::new(device, "leaf masks", U::STORAGE);
        let bricks = GpuMirror::new(device, "bricks", U::STORAGE);

        let face_table = block::face_table();
        let faces = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("block faces"),
            size: (face_table.len() * 4) as u64,
            usage: U::STORAGE | U::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&faces, 0, bytemuck::cast_slice(&face_table));

        let max_pairs = pairs_for((size.0.div_ceil(TILE), size.1.div_ceil(TILE)));
        let pairs = DynBuffer::new(device, "pairs", (max_pairs as u64) * 2 * 4, U::STORAGE);
        let counters = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("counters"),
            size: 64,
            usage: U::STORAGE | U::COPY_DST | U::COPY_SRC,
            mapped_at_creation: false,
        });
        // The shader writes dispatch sizes here as a storage buffer...
        let indirect_storage = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("indirect storage"),
            size: 32,
            usage: U::STORAGE | U::COPY_SRC | U::COPY_DST,
            mapped_at_creation: false,
        });
        // ...and they are copied here, which is the only buffer bound as INDIRECT.
        let indirect = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("indirect"),
            size: 32,
            usage: U::INDIRECT | U::COPY_DST,
            mapped_at_creation: false,
        });
        let vis = DynBuffer::new(
            device,
            "visibility",
            (size.0 as u64) * (size.1 as u64) * 8,
            U::STORAGE,
        );
        let hiz = DynBuffer::new(
            device,
            "hi-z",
            (tiles.0 as u64) * (tiles.1 as u64) * 4,
            U::STORAGE,
        );
        // `COPY_SRC` so `march_stats` can read the iteration counts back (batch 53). The
        // heatmap wrote them for four years of batches and nothing on the CPU had ever
        // looked; a usage flag is the whole cost of looking.
        let dbg = DynBuffer::new(
            device,
            "debug",
            (size.0 as u64) * (size.1 as u64) * 4,
            U::STORAGE | U::COPY_SRC,
        );
        // 3 MB and it does not move with the window size, which is the point of a world-space
        // structure: the envelope is the same field whether it is being sampled by a 1280x720
        // capture or a 4K window.
        let shaft_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("light envelope"),
            size: (crate::shaft::BUFFER_FLOATS * 4) as u64,
            usage: U::STORAGE | U::COPY_DST,
            mapped_at_creation: false,
        });

        // ---- textures
        let (out_tex, out_view) = make_hdr_texture(device, size, "trace output");
        let hist_view = [
            make_hdr_texture(device, size, "taa history 0").1,
            make_hdr_texture(device, size, "taa history 1").1,
        ];
        // `1 << water_sec_shift` of the trace grid in each axis, rounded up --
        // `water_sec` covers exactly the blocks its dispatch grid implies, and the
        // round-up is what keeps odd resolutions' last row and column owned.
        let span = 1u32 << water_sec_shift;
        let ws = (size.0.div_ceil(span), size.1.div_ceil(span));
        let water_sec_tex = (
            make_hdr_texture(device, ws, "water sec refr").0,
            make_hdr_texture(device, ws, "water sec refl").0,
        );
        let water_sec_view = (
            water_sec_tex.0.create_view(&Default::default()),
            water_sec_tex.1.create_view(&Default::default()),
        );
        let water_read_group =
            make_water_sec_group(device, &wread_layout, (&water_sec_view.0, &water_sec_view.1), "water sec read");
        let water_store_group = make_water_sec_group(
            device,
            &wstore_layout,
            (&water_sec_view.0, &water_sec_view.1),
            "water sec store",
        );
        let (atlas_view, atlas_sampler) = make_atlas(device, queue, 0.0, tint_balance);
        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("history sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let (probe_tex, probe_view) = make_probe(device, queue, probe_fill, probe_noise);
        let probe_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("probe sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::Repeat,
            ..Default::default()
        });

        let bind_group = make_bind_group(
            device,
            &layout,
            &frame_buf,
            &chunk_buf,
            &inners,
            &leaves,
            &bricks,
            &faces,
            &pairs.buf,
            &counters,
            &indirect_storage,
            &vis,
            &hiz,
            &out_view,
            &atlas_view,
            &atlas_sampler,
            &dbg,
            &grid_buf,
            (&hist_view[0], &hist_view[1]),
            &linear_sampler,
            &shaft_buf,
            &probe_view,
            &probe_sampler,
        );
        let taa_bind_group = std::array::from_fn(|i| {
            make_bind_group(
                device,
                &layout,
                &frame_buf,
                &chunk_buf,
                &inners,
                &leaves,
                &bricks,
                &faces,
                &pairs.buf,
                &counters,
                &indirect_storage,
                &vis,
                &hiz,
                &hist_view[i],
                &atlas_view,
                &atlas_sampler,
                &dbg,
                &grid_buf,
                (&hist_view[i ^ 1], &out_view),
                &linear_sampler,
                &shaft_buf,
                &probe_view,
                &probe_sampler,
            )
        });
        let blit = blit::BlitPass::new(
            device,
            queue,
            surface_format,
            tone,
            grade,
            grade_strength,
            &[&out_view, &hist_view[0], &hist_view[1]],
        );
        let ui = ui::UiRenderer::new(device, queue, surface_format);
        let timer = Readback::new(device, queue, gpu.has_timestamps);

        Self {
            gpu,
            layout,
            pipes,
            module,
            pipeline_layout,
            spec_pipes,
            bind_group,
            taa_bind_group,
            bind_dirty: false,
            frame_buf,
            chunk_buf,
            grid_buf,
            inners,
            leaves,
            bricks,
            faces,
            pairs,
            max_pairs,
            counters,
            indirect_storage,
            indirect,
            vis,
            hiz,
            dbg,
            shaft_buf,
            shaft_pipes,
            out_tex,
            out_view,
            water_sec_shift,
            water_sec_view,
            water_sec_tex,
            water_read_group,
            water_store_group,
            water_read_layout: wread_layout,
            water_store_layout: wstore_layout,
            water_empty_group: wempty_group,
            shadow_targets, shadow_layouts, shadow_pipe_layouts, split_layout,
            water_read_pipe_layout,
            water_sec_pipe_layout,
            hist_view,
            atlas_view,
            probe_view,
            probe_tex,
            probe_sampler,
            probe_pinned: probe_fill.is_some(),
            atlas_sampler,
            linear_sampler,
            blit,
            ui,
            timer,
            size,
            surface_size: size,
            tiles,
            surface_format,
            grid_min: IVec3::ZERO,
            prev_view_proj: None,
            frame_index: 0,
            history_valid: false,
            last_blit: 0,
            chunk_scratch: Vec::with_capacity(MAX_CHUNKS),
            grid_scratch: vec![NO_CHUNK; GRID_LEN],
            shadow_share: lod.shadow_share,
            offscreen_shadows: lod.offscreen_shadows,
            last_chunk_count: 0,
            last_grid_chunks: 0,
            last_camera_chunk: None,
        }
    }

    /// `surface` is the window size; `size` is the internal trace resolution.
    /// Settings changes invalidate HDR history without reallocating its textures.
    pub fn invalidate_history(&mut self) {
        self.history_valid = false;
        self.prev_view_proj = None;
    }

    pub fn set_presentation(&self, tone: u32, grade: u32, strength: f32) {
        self.blit.set_presentation(&self.gpu.queue, self.surface_format, tone, grade, strength);
    }

    pub fn resize(&mut self, size: (u32, u32), surface: (u32, u32)) {
        self.surface_size = (surface.0.max(1), surface.1.max(1));
        let size = (size.0.max(8), size.1.max(8));
        if size == self.size {
            return;
        }
        self.size = size;
        self.tiles = (size.0.div_ceil(TILE), size.1.div_ceil(TILE));
        let device = &self.gpu.device;
        self.vis
            .resize(device, (size.0 as u64) * (size.1 as u64) * 8);
        self.dbg
            .resize(device, (size.0 as u64) * (size.1 as u64) * 4);
        self.hiz
            .resize(device, (self.tiles.0 as u64) * (self.tiles.1 as u64) * 4);
        // Batch 53: the pair list is screen-sized and so is sized from the screen. At the
        // fixture's 1280x720 and at 1920x1080 `pairs_for` returns the `MAX_PAIRS` floor, so
        // both render with the capacity every number in `PERF.md` was taken at.
        self.max_pairs = pairs_for(self.tiles);
        self.pairs
            .resize(device, (self.max_pairs as u64) * 2 * 4);
        let (t, v) = make_hdr_texture(device, size, "trace output");
        self.out_tex = t;
        self.out_view = v;
        self.hist_view = [
            make_hdr_texture(device, size, "taa history 0").1,
            make_hdr_texture(device, size, "taa history 1").1,
        ];
        let span = 1u32 << self.water_sec_shift;
        let ws = (size.0.div_ceil(span), size.1.div_ceil(span));
        self.water_sec_tex = (
            make_hdr_texture(device, ws, "water sec refr").0,
            make_hdr_texture(device, ws, "water sec refl").0,
        );
        self.water_sec_view = (
            self.water_sec_tex.0.create_view(&Default::default()),
            self.water_sec_tex.1.create_view(&Default::default()),
        );
        self.blit.set_sources(
            device,
            &[&self.out_view, &self.hist_view[0], &self.hist_view[1]],
        );
        // Nothing in the old history maps onto the new grid.
        self.history_valid = false;
        self.bind_dirty = true;
    }

    fn rebuild_bind_group(&mut self) {
        self.bind_group = make_bind_group(
            &self.gpu.device,
            &self.layout,
            &self.frame_buf,
            &self.chunk_buf,
            &self.inners,
            &self.leaves,
            &self.bricks,
            &self.faces,
            &self.pairs.buf,
            &self.counters,
            &self.indirect_storage,
            &self.vis,
            &self.hiz,
            &self.out_view,
            &self.atlas_view,
            &self.atlas_sampler,
            &self.dbg,
            &self.grid_buf,
            (&self.hist_view[0], &self.hist_view[1]),
            &self.linear_sampler,
            &self.shaft_buf,
            &self.probe_view,
            &self.probe_sampler,
        );
        // 81b: the two views may have been remade by the same resize that dirtied this
        // group; the water groups ride the same event.
        self.water_read_group = make_water_sec_group(
            &self.gpu.device,
            &self.water_read_layout,
            (&self.water_sec_view.0, &self.water_sec_view.1),
            "water sec read",
        );
        self.water_store_group = make_water_sec_group(
            &self.gpu.device,
            &self.water_store_layout,
            (&self.water_sec_view.0, &self.water_sec_view.1),
            "water sec store",
        );
        self.taa_bind_group = std::array::from_fn(|i| {
            make_bind_group(
                &self.gpu.device,
                &self.layout,
                &self.frame_buf,
                &self.chunk_buf,
                &self.inners,
                &self.leaves,
                &self.bricks,
                &self.faces,
                &self.pairs.buf,
                &self.counters,
                &self.indirect_storage,
                &self.vis,
                &self.hiz,
                &self.hist_view[i],
                &self.atlas_view,
                &self.atlas_sampler,
                &self.dbg,
                &self.grid_buf,
                (&self.hist_view[i ^ 1], &self.out_view),
                &self.linear_sampler,
                &self.shaft_buf,
                &self.probe_view,
                &self.probe_sampler,
            )
        });
        self.bind_dirty = false;
    }

    /// Push any pool changes to the GPU. Call once per frame before `render`.
    /// Hand the light envelope's static heights to the GPU. The caller does this only when
    /// `ShaftField::dirty` says the window moved, which on an unmoved camera is once.
    ///
    /// It writes slice 2 and never the other two: the scan owns those, and a CPU write into
    /// one would be a second writer of a buffer whose whole determinism argument is that each
    /// pass has exactly one.
    pub fn upload_shaft(&self, heights: &[f32]) {
        debug_assert_eq!(heights.len(), crate::shaft::DIM * crate::shaft::DIM);
        self.gpu.queue.write_buffer(
            &self.shaft_buf,
            (crate::shaft::HEIGHTS_BASE * 4) as u64,
            bytemuck::cast_slice(heights),
        );
    }

    /// `cam` has been here since batch 71 and is only ever read by the probe upload: the
    /// lattice is toroidal, two resident chunks 512 blocks apart own the same texels, and
    /// which of them wins used to follow rayon completion timing -- roadmap D1's one
    /// flickering pixel at `lattice`. [`probe::UPLOAD_REACH`] is the argument and the
    /// bound; deferred bakes are re-queued, not dropped, so a chunk the camera returns to
    /// is uploaded then and never silently reset to the field's identity fill.
    pub fn sync_world(&mut self, world: &mut World, cam: Vec3) {
        let device = &self.gpu.device;
        let queue = &self.gpu.queue;
        let mut dirty = world.inners.take_dirty();
        if self.inners.sync(
            device,
            queue,
            bytemuck::cast_slice(world.inners.data()),
            &mut dirty,
            16,
        ) {
            self.bind_dirty = true;
        }
        let mut dirty = world.leaves.take_dirty();
        if self.leaves.sync(
            device,
            queue,
            bytemuck::cast_slice(world.leaves.data()),
            &mut dirty,
            8,
        ) {
            self.bind_dirty = true;
        }
        let mut dirty = world.bricks.take_dirty();
        if self.bricks.sync(
            device,
            queue,
            bytemuck::cast_slice(world.bricks.data()),
            &mut dirty,
            4,
        ) {
            self.bind_dirty = true;
        }
        for (key, data) in world.take_probe_dirty() {
            if probe::uploadable(key.origin(), cam) {
                self.write_probe(key.origin(), &data);
            } else {
                // Deferred, not dropped: a chunk built for a camera that has already moved
                // away. See `probe::UPLOAD_REACH`.
                world.set_probe(key, data);
            }
        }
    }

    /// Write one chunk's baked ambient cubes into the lattice.
    ///
    /// **Six writes and not one**, because the slabs are `PROBE_DIM_Y` apart in Y and a chunk
    /// owns eight probe rows inside each of them. Each is 8x8x8 texels -- 4 KB, 24 KB for the
    /// chunk -- and `queue.write_texture` restages a row pitch that is not 256-aligned, so the
    /// 64-byte rows here need no padding of their own.
    ///
    /// The bake is pinned off entirely under `--probe-fill`, which is what keeps batch 55's and
    /// batch 56's diagnostics measuring the constant field their numbers were read from.
    fn write_probe(&self, origin: IVec3, data: &ProbeData) {
        if self.probe_pinned {
            return;
        }
        let sp = probe::SPACING;
        // `origin` is a multiple of 64 and `SPACING` divides it, so the chunk's block of texels
        // is 8-aligned in every axis and can never straddle the toroidal seam.
        let bx = (origin.x / sp).rem_euclid(PROBE_DIM_XZ as i32) as u32;
        let bz = (origin.z / sp).rem_euclid(PROBE_DIM_XZ as i32) as u32;
        let by = (origin.y / sp).clamp(0, PROBE_DIM_Y as i32 - probe::PER_AXIS as i32) as u32;
        let n = probe::PER_AXIS as u32;
        let mut texels: Vec<u8> = Vec::with_capacity(probe::PROBES * PROBE_TEXEL as usize);
        for axis in 0..probe::AXES {
            // **Batch 58 is where the day the comment here anticipated arrived, and the only
            // thing that changed is the bake.** `.rgb` is the ground bounce's per-channel
            // multiplier on `floor_rgb` and `.a` is batch 57's sky occlusion, both identity at
            // 1.0 and both read by one `textureSampleLevel` -- so a directional *coloured*
            // field and a directional scalar one cost the same tap, which is the finding batch
            // 55 went looking for and the reason the format was `Rgba16Float` from the start.
            texels.clear();
            for i in 0..probe::PROBES {
                let o = (axis * probe::PROBES + i) * 3;
                for c in 0..3 {
                    texels.extend_from_slice(&f16_bits(data.bounce[o + c]).to_le_bytes());
                }
                texels.extend_from_slice(
                    &f16_bits(data.occl[axis * probe::PROBES + i]).to_le_bytes(),
                );
            }
            self.gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.probe_tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: bx,
                        y: axis as u32 * PROBE_DIM_Y + by,
                        z: bz,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &texels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(n * PROBE_TEXEL),
                    rows_per_image: Some(n),
                },
                wgpu::Extent3d {
                    width: n,
                    height: n,
                    depth_or_array_layers: n,
                },
            );
        }
    }

    /// Build the per-frame chunk list and the shadow/AO lookup grid.
    pub fn build_chunk_list(
        &mut self,
        render_set: &[RenderItem],
        world: &World,
        cam: Vec3,
        frustum: &Frustum,
    ) {
        self.chunk_scratch.clear();
        // The grid's own origin, needed before the partition rather than after it: what bounds
        // the off-screen tail is the lattice, so the tail cannot be chosen without it.
        let gmin = grid_origin(cam);

        let mut plan: Vec<(RenderItem, Aabb)> = Vec::with_capacity(render_set.len());
        for &item in render_set {
            let Some(rec) = world.chunks.get(&item.key) else {
                continue;
            };
            if rec.solid_count == 0 {
                continue;
            }
            plan.push((item, rec.world_aabb()));
        }
        let (visible, offscreen) =
            partition_for_grid(&plan, cam, frustum, gmin, self.offscreen_shadows);

        // `visible` first and `offscreen` after it, because the index into this array *is* the
        // chunk id: everything `tile_select` may reach has to sit below `frame.chunk_count`.
        let mut camera_chunk = None;
        for item in visible.iter().chain(offscreen.iter()) {
            let key = &item.key;
            let rec = &world.chunks[key];
            let aabb = rec.world_aabb();
            let idx = self.chunk_scratch.len();
            // **The drawn prefix only.** `tile_select` compares this against its own loop
            // counter, which never reaches the tail, so a tail index here would be a
            // camera-chunk exemption that silently never fires. The camera's chunk is in the
            // frustum by construction -- the four side planes pass through the eye -- so this
            // is a guard on the invariant rather than a case that happens.
            if idx < visible.len()
                && camera_chunk.is_none()
                && cam.cmpge(aabb.min).all()
                && cam.cmple(aabb.max).all()
            {
                camera_chunk = Some(idx);
            }
            let (attr_base, mut attr_flags) = match rec.attr {
                AttrRef::Uniform(id) => (id as u32, 1),
                AttrRef::Table { base, .. } => (base, 0),
            };
            if rec.has_water {
                attr_flags |= 2;
            }
            if rec.has_foliage {
                attr_flags |= 4;
            }
            if rec.has_cutout {
                attr_flags |= 8;
            }
            if rec.has_emitter { attr_flags |= 32; }
            if rec.has_glass {
                attr_flags |= 16;
            }
            let (light_base, light_flags) = match rec.light {
                LightRef::Uniform(b) => (b as u32, 1),
                LightRef::Table { base } => (base, 0),
            };
            let root: [u32; 4] = bytemuck::cast(rec.root);
            self.chunk_scratch.push(GpuChunk {
                aabb_min: aabb.min.to_array(),
                voxel_size: key.voxel_size() as f32,
                aabb_max: aabb.max.to_array(),
                lod: key.lod as u32,
                origin: key.origin().as_vec3().to_array(),
                fade: item.fade,
                root,
                attr_base,
                attr_flags,
                light_base,
                light_flags,
                dry_mask: [rec.dry_mask as u32, (rec.dry_mask >> 32) as u32],
                _reserved: [0; 2],
            });
        }
        // **The drawn count, not the uploaded one.** This is what `frame.chunk_count` carries
        // and `tile_select` loops over, so it has to stay the frustum-culled length however
        // long the array behind it gets.
        self.last_chunk_count = visible.len();
        self.last_grid_chunks = self.chunk_scratch.len();
        self.last_camera_chunk = camera_chunk;

        // Grid of LOD-0 sized cells for shadow rays and cross-chunk ambient occlusion.
        self.grid_scratch.fill(NO_CHUNK);
        // Coarse chunks first so finer ones overwrite them where both cover a cell, and
        // **across a cross-fade the half carrying most of the rays wins** -- batch 62,
        // roadmap D2.
        //
        // The grid holds one representation per cell and dithering the secondary rays too
        // would only add noise the temporal pass has no motion vector for, which is why
        // this is an ordering and not a blend. What changed is *which* half it resolves to.
        // It used to be the fine one unconditionally, and that is a pop: `fade` is a pure
        // function of distance, so the fine half enters `visible` the instant a chunk
        // crosses the outer edge of the band, when its share is still ~0. The surface was
        // therefore ~100% coarse, crisp and stable, while its shadow had already switched
        // to the fine silhouette **in one frame**. Measured at `lod`, the two
        // representations differ by **31,530 pixels of 921,600 at max delta 34** -- so what
        // snapped was large, and the user reported it as a mountain's shadow popping in.
        //
        // **Sorting by share puts the discontinuity at the middle of the fade instead of at
        // its start**, which is the one moment the frame is already in flux: the surface is
        // 50/50 dithered between the two representations and the temporal pass is blending
        // them. The switch is still a switch -- one grid cell cannot hold two chunks -- but
        // it lands where nothing else is stable either.
        //
        // **Outside a transition every share is 1.0**, so the tie-break decides and it is
        // the old rule exactly: coarser first, finer overwrites. That is what keeps every
        // vantage with no fading chunk in it bit-exact.
        //
        // **Both lists, and that is batch 64.** The ordering rule is unchanged and so is what
        // it is applied to -- one entry per chunk, coarse before fine -- but the set it runs
        // over is now everything resident that touches the lattice rather than everything the
        // frustum kept. An off-screen entry can only ever *fill* a cell the old fill left as
        // `NO_CHUNK` or replace a coarser stand-in with a finer one, because two entries share
        // a cell only when one is the other's ancestor; it cannot take a cell away from a
        // chunk that is on screen.
        let grid_set: Vec<RenderItem> =
            visible.iter().chain(offscreen.iter()).copied().collect();
        let mut order: Vec<usize> = (0..grid_set.len()).collect();
        let share_match = self.shadow_share;
        order.sort_by(|&a, &b| {
            let (ia, ib) = (grid_set[a], grid_set[b]);
            let key = |it: crate::lod::RenderItem| if share_match { it.share() } else { 1.0 };
            key(ia)
                .total_cmp(&key(ib))
                .then(ib.key.lod.cmp(&ia.key.lod))
        });
        for i in order {
            let Some((lo, hi)) = grid_span(grid_set[i].key, gmin) else {
                continue;
            };
            for z in lo.z..hi.z {
                for y in lo.y..hi.y {
                    let row = (y as u32 * GRID_X + z as u32 * GRID_X * GRID_Y) as usize;
                    for x in lo.x..hi.x {
                        self.grid_scratch[row + x as usize] = i as u32;
                    }
                }
            }
        }
        let queue = &self.gpu.queue;
        queue.write_buffer(&self.grid_buf, 0, bytemuck::cast_slice(&self.grid_scratch));
        if self.chunk_buf.write(
            &self.gpu.device,
            queue,
            bytemuck::cast_slice(&self.chunk_scratch),
        ) {
            self.bind_dirty = true;
        }
        self.grid_min = gmin;
    }

    fn ensure_shadow_targets(&mut self,size:(u32,u32)) {
        if self.shadow_targets.size!=size {
            self.shadow_targets=shadow::Targets::new(&self.gpu.device,size,&self.split_layout,&self.shadow_layouts);
        }
    }

    /// Encode and submit all compute passes. `target` is the surface view, if any.
    /// Index of the `march`/`resolve` pair specialized to `spec`, compiling it if this run
    /// has not asked for that combination before.
    ///
    /// The key is the same `frame.flags` word the shaders still test, masked -- not a
    /// separate opinion about which features are on. That is what keeps the folded constant
    /// and the run-time branch from ever disagreeing.
    fn ensure_spec(&mut self, spec: (u32, u32)) -> usize {
        if let Some(i) = self.spec_pipes.iter().position(|(k, _)| *k == spec) {
            return i;
        }
        let pipes = make_spec(
            &self.gpu.device,
            &self.module,
            (
                &self.pipeline_layout,
                &self.water_sec_pipe_layout,
                &self.water_read_pipe_layout,
                &self.shadow_pipe_layouts[0], &self.shadow_pipe_layouts[1], &self.shadow_pipe_layouts[2],
            ),
            spec.0,
            spec.1,
            self.water_sec_shift,
        );
        self.spec_pipes.push((spec, pipes));
        self.spec_pipes.len() - 1
    }

    pub fn render(&mut self, p: &FrameParams, target: Option<&wgpu::TextureView>) {
        if self.bind_dirty {
            self.rebuild_bind_group();
        }
        let (w, h) = self.size;
        // Before the encoder, because this can compile a pipeline and nothing recorded into
        // a pass may take `&mut self`.
        let spec_key = (p.flags & SPEC_MASK, (p.spec_hi | FLAG_HI_SHADOW_PASS) & SPEC_HI_MASK);
        let spec_idx = self.ensure_spec(spec_key);
        self.ensure_shadow_targets((w,h));
        // D5, batch 73's own blind spot named and closed at the key point: the emitter
        // count is *runtime* state -- placing the first lamp in a lamp-less world, or
        // breaking the last one, flips `SPEC_EMITTER_GATHER` mid-play, and `ensure_spec`
        // is lazy, which would make that lamp the frame that compiles a shader (the
        // mid-play hitch batch 54's blind spot warned of, produced by design). Any spec
        // word that can render is therefore asked to also build its emitter sibling the
        // first time it appears; after that both calls below are two cache hits, priced
        // as two Vec scans a frame against one player-visible hitch per session.
        // Control words (`--no-foliage` and friends) get the same treatment here rather
        // than only the boot-shipping word, which is what `Renderer::new` compiles.
        let sibling = (spec_key.0, spec_key.1 ^ FLAG_HI_EMITTER_WORLD);
        if sibling != spec_key {
            self.ensure_spec(sibling);
        }
        let heatmap = p.flags & FLAG_HEATMAP != 0;
        let hiz_on = p.flags & FLAG_HIZ != 0;

        let taa_on = p.taa;
        let parity = (self.frame_index & 1) as usize;
        // `--jitter` pins the offset; that is what makes the bit-exactness A/B possible, and
        // a sequence would destroy it. Otherwise the eight Halton phases walk on their own.
        let jitter = match p.jitter {
            Some(j) => j,
            None if taa_on => crate::math::halton_jitter(self.frame_index),
            None => Vec2::ZERO,
        };
        let feedback = if taa_on && self.history_valid {
            TAA_FEEDBACK
        } else {
            1.0
        };
        // The mip footprint in `resolve` divides by the *render* height, so a lower internal
        // resolution picks blurrier mips and leaves the reconstructor nothing to work with.
        // Deriving the bias here rather than asking the caller for it is the point: `--scale`
        // would otherwise cost sharpness silently.
        let mip_bias = (h as f32 / self.surface_size.1 as f32).log2() + p.mip_bias;

        let tan_half_fov = (p.fov_y * 0.5).tan();
        let aspect = w as f32 / h as f32;
        // Jitter-free on purpose: a reprojection wants the pixel grid, not this frame's
        // sample offset within it. The first frame reprojects onto itself, so a consumer
        // reads zero motion rather than a garbage matrix.
        let view_proj = crate::math::view_proj(
            p.cam_pos,
            p.cam_fwd,
            p.cam_right,
            p.cam_up,
            tan_half_fov,
            aspect,
            NEAR,
            p.far,
        );
        let prev_view_proj = self.prev_view_proj.unwrap_or(view_proj);

        let frame = GpuFrame {
            cam_pos: p.cam_pos.to_array(),
            time: p.time,
            cam_fwd: p.cam_fwd.to_array(),
            tan_half_fov,
            cam_right: p.cam_right.to_array(),
            aspect,
            cam_up: p.cam_up.to_array(),
            far: p.far,
            sun_dir: p.sun_dir.to_array(),
            daylight: p.daylight,
            res: [w, h],
            tiles: [self.tiles.0, self.tiles.1],
            chunk_count: self.last_chunk_count as u32,
            camera_chunk: self.last_camera_chunk.map_or(NO_CHUNK, |i| i as u32),
            flags: p.flags,
            max_pairs: self.max_pairs,
            fog_density: p.fog.density,
            fog_falloff: p.fog.falloff,
            shadow_dist: 220.0,
            ambient: p.ambient,
            fog_scatter: p.fog.scatter,
            fog_g: p.fog.g,
            fog_height: p.fog.height,
            mip_bias,
            grid_min: self.grid_min.to_array(),
            sea_level: p.sea_level,
            grid_dim: [GRID_X, GRID_Y, GRID_Z],
            water_absorb: p.water_absorb,
            jitter: jitter.to_array(),
            taa_feedback: feedback,
            // Wrapped well clear of f32 precision in the dither's multiply, and well clear
            // of both the eight jitter phases and the 32 frames a screenshot converges
            // over, so a capture never sees the pattern repeat.
            frame_index: (self.frame_index % 64) as u32,
            cloud_cover: p.clouds.cover,
            cloud_height: p.clouds.height,
            // The shader divides by this. One clamp here beats a `max` on every sky ray.
            cloud_scale: p.clouds.scale.max(1.0),
            cloud_speed: p.clouds.speed,
            godray_strength: p.godrays.strength,
            // The shader loops on this and divides by it. Zero steps would divide by zero
            // and a runaway would be a hang, so the clamp lives here rather than in the
            // inner loop -- one place, and the same reasoning as `cloud_scale` above.
            godray_steps: p.godrays.steps.clamp(1, 64),
            godray_dist: p.godrays.dist.max(0.0),
            // A cloud cannot take away more sun than there is. Above 1 the shader's
            // `1 - dens * cloud_shadow` goes negative, which is negative radiance out of
            // `shade_hit`'s sun term -- clamped here, in the one place, rather than guarded
            // in two shader functions.
            cloud_shadow: p.godrays.cloud_shadow.clamp(0.0, 1.0),
            // Above 1 the multiplier would push past the authored colour into whatever is
            // on the other side of it, which is a different biome's tint and not more of
            // this one. Clamped here, in the one place, like `cloud_shadow` above.
            tint_strength: p.tint.strength.clamp(0.0, 1.0),
            tint_seed: p.tint.seed,
            tint_freq_t: biome::TEMP_FREQ,
            tint_freq_h: biome::HUMID_FREQ,
            wave_amp: p.waves.amp.max(0.0),
            // The shader divides by this, once per water pixel. One clamp here beats a
            // `max` there, the same trade `cloud_scale` makes.
            wave_scale: p.waves.scale.max(1.0),
            wave_speed: p.waves.speed,
            wave_reflect_slope: p.waves.reflect_slope.max(0.0),
            shaft_origin: p.shaft_origin,
            shaft_texel: p.shaft_texel,
            shaft_soft: crate::shaft::SOFT,
            // G1 knobs, parse-clamped already (`--tint-balance`'s one-place rule);
            // 0 is the pre-95 frame by each field's own guard in the shader.
            cloud_patch: p.clouds.patch,
            cloud_relief: p.clouds.relief,
            haze_warm: p.sky.haze_warm,
            zenith_deep: p.sky.zenith_deep,
            prev_view_proj: prev_view_proj.to_cols_array_2d(),
        };
        self.gpu
            .queue
            .write_buffer(&self.frame_buf, 0, bytemuck::bytes_of(&frame));
        self.prev_view_proj = Some(view_proj);

        let mut enc = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        enc.clear_buffer(&self.vis.buf, 0, None);
        enc.clear_buffer(&self.counters, 0, None);
        enc.clear_buffer(&self.indirect_storage, 0, None);
        if heatmap {
            enc.clear_buffer(&self.dbg.buf, 0, None);
        }
        if !hiz_on {
            enc.clear_buffer(&self.hiz.buf, 0, None);
        }

        // The envelope's scan, before anything that reads it. Eight dispatches over a fixed
        // 512x512 grid -- the one pass in this frame whose cost does not depend on the window
        // size, on how much geometry is on screen, or on where the camera is looking.
        if p.shaft_scan {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("light envelope"),
                timestamp_writes: self.timer.writes(q::SHAFT),
            });
            pass.set_bind_group(0, &self.bind_group, &[]);
            let groups = (crate::shaft::DIM as u32).div_ceil(8);
            for pipe in &self.shaft_pipes {
                pass.set_pipeline(pipe);
                pass.dispatch_workgroups(groups, groups, 1);
            }
        }
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("tile select"),
                timestamp_writes: self.timer.writes(q::TILE_SELECT),
            });
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_pipeline(&self.pipes.tile_select);
            // 8x8 threads per block, matching tile_select.wgsl's workgroup_size.
            pass.dispatch_workgroups(self.tiles.0.div_ceil(8), self.tiles.1.div_ceil(8), 1);
            pass.set_pipeline(&self.pipes.finalize);
            pass.dispatch_workgroups(1, 1, 1);
        }
        enc.copy_buffer_to_buffer(&self.indirect_storage, 0, &self.indirect, 0, 32);
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("march"),
                timestamp_writes: self.timer.writes(q::MARCH),
            });
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_pipeline(&self.spec_pipes[spec_idx].1.march);
            pass.dispatch_workgroups_indirect(&self.indirect, 0);
        }
        if hiz_on {
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("hi-z"),
                    timestamp_writes: self.timer.writes(q::HIZ),
                });
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_pipeline(&self.pipes.build_hiz);
                pass.dispatch_workgroups(self.tiles.0, self.tiles.1, 1);
                pass.set_pipeline(&self.pipes.finalize_deferred);
                pass.dispatch_workgroups(1, 1, 1);
            }
            enc.copy_buffer_to_buffer(&self.indirect_storage, 0, &self.indirect, 0, 32);
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("recover"),
                    timestamp_writes: self.timer.writes(q::RECOVER),
                });
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_pipeline(&self.pipes.recover_select);
                pass.dispatch_workgroups_indirect(&self.indirect, 12);
                pass.set_pipeline(&self.pipes.finalize_recover);
                pass.dispatch_workgroups(1, 1, 1);
            }
            enc.copy_buffer_to_buffer(&self.indirect_storage, 0, &self.indirect, 0, 32);
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("march recovered"),
                    timestamp_writes: self.timer.writes(q::MARCH2),
                });
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_pipeline(&self.spec_pipes[spec_idx].1.march);
                pass.dispatch_workgroups_indirect(&self.indirect, 0);
            }
        }
        if spec_key.1 & (FLAG_HI_ISOLATE_GLASS | FLAG_HI_SHADOW_PASS) == 0 {
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("resolve"),
                timestamp_writes: self.timer.writes(q::RESOLVE),
            });
            pass.set_bind_group(0, &self.bind_group, &[]);
            // Batch 81 (roadmap P12)'s half-res legs, dispatched only when the arm that
            // will read them is in this frame's spec word -- between the last `march` and
            // `resolve`, since it reads `vis` like the one and feeds the other. The grid
            // is the shader's own rounding, `ceil(wh / 8)` over half dimensions.
            if spec_key.1 & FLAG_HI_WATER_SEC != 0 {
                // 81b's groups, attached in the layout's own order: group 0 the world,
                // group 1 the empty span, group 2 the storage pair this pass writes.
                pass.set_bind_group(1, &self.water_empty_group, &[]);
                pass.set_bind_group(2, &self.water_store_group, &[]);
                pass.set_pipeline(&self.spec_pipes[spec_idx].1.water_sec);
                let span = 1u32 << self.water_sec_shift;
                let (hw, hh) = (w.div_ceil(span), h.div_ceil(span));
                pass.dispatch_workgroups(hw.div_ceil(8), hh.div_ceil(8), 1);
            }
            // Resolve reads the same textures through group 1 of *its* layout -- the
            // reading half of the split, and the pair's only other role in this encoder.
            pass.set_bind_group(1, &self.water_read_group, &[]);
            pass.set_bind_group(2, &self.water_empty_group, &[]);
            pass.set_bind_group(3, &self.shadow_targets.read_group, &[]);
            pass.set_pipeline(&self.spec_pipes[spec_idx].1.resolve);
            pass.dispatch_workgroups(w.div_ceil(16), h.div_ceil(8), 1);
        }
        } else {
            // The family timestamp encloses ALL work, barriers and secondary passes.
            // This keeps existing `resolve`/GPU-total benchmark columns comparable.
            {
                let _pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("primary shadow / split family begin"),
                    timestamp_writes: self.timer.family_timestamp(true),
                });
            }
            for stage in 0..3 {
                let label=["shadow one ray", "shadow bilateral horizontal", "shadow bilateral vertical"][stage];
                let mut pass=enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label:Some(label), timestamp_writes:self.timer.writes(q::SHADOW_RAY+stage as u32*2),
                });
                pass.set_bind_group(0,&self.bind_group,&[]);
                pass.set_bind_group(1,&self.water_empty_group,&[]);
                pass.set_bind_group(2,&self.water_empty_group,&[]);
                pass.set_bind_group(3,&self.shadow_targets.groups[stage],&[]);
                let pipe=if stage==0 { self.spec_pipes[spec_idx].1.primary_shadow.as_ref().unwrap() }
                    else { &self.spec_pipes[spec_idx].1.shadow_blurs[stage-1] };
                pass.set_pipeline(pipe);
                pass.dispatch_workgroups(w.div_ceil(16),h.div_ceil(8),1);
            }
            if spec_key.1 & FLAG_HI_WATER_SEC != 0 {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("water secondary legs"), timestamp_writes: None,
                });
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_bind_group(1, &self.water_empty_group, &[]);
                pass.set_bind_group(2, &self.water_store_group, &[]);
                pass.set_pipeline(&self.spec_pipes[spec_idx].1.water_sec);
                let span=1u32 << self.water_sec_shift;
                pass.dispatch_workgroups(w.div_ceil(span).div_ceil(8), h.div_ceil(span).div_ceil(8), 1);
            }
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some(if spec_key.1 & FLAG_HI_SHADOW_PASS != 0 { "opaque resolve" } else { "main resolve (glass excluded)" }), timestamp_writes: None,
                });
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_bind_group(1, &self.water_read_group, &[]);
                pass.set_bind_group(2, &self.water_empty_group, &[]);
                pass.set_bind_group(3, &self.shadow_targets.read_group, &[]);
                pass.set_pipeline(&self.spec_pipes[spec_idx].1.resolve);
                pass.dispatch_workgroups(w.div_ceil(16), h.div_ceil(8), 1);
            }
            for (label, pipe) in [
                ("glass resolve", self.spec_pipes[spec_idx].1.glass_resolve.as_ref()),
                ("water resolve", self.spec_pipes[spec_idx].1.water_resolve.as_ref()),
            ] {
                if let Some(pipe) = pipe {
                    let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: Some(label), timestamp_writes: None,
                    });
                    pass.set_bind_group(0, &self.bind_group, &[]);
                    pass.set_bind_group(1, &self.water_read_group, &[]);
                    pass.set_bind_group(2, &self.water_empty_group, &[]);
                    pass.set_bind_group(3, &self.shadow_targets.read_group, &[]);
                    pass.set_pipeline(pipe);
                    pass.dispatch_workgroups(w.div_ceil(16), h.div_ceil(8), 1);
                }
            }
            // End after the last material writer and before TAA consumes the whole image.
            let _end = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("split family end"), timestamp_writes: self.timer.family_timestamp(false),
            });
        }
        if taa_on {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("taa"),
                timestamp_writes: self.timer.writes(q::TAA),
            });
            pass.set_bind_group(0, &self.taa_bind_group[parity], &[]);
            pass.set_pipeline(&self.pipes.taa);
            pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
        }

        if let Some(view) = target {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("present"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            self.last_blit = if taa_on { 1 + parity } else { 0 };
            self.blit.draw(&mut pass, self.last_blit);
            self.ui.draw(&self.gpu.queue, &mut pass, self.surface_size);
        }

        self.timer.encode(&mut enc, &self.counters);
        self.gpu.queue.submit(Some(enc.finish()));
        self.timer.map_latest();
        self.frame_index += 1;
        // A frame rendered without the temporal pass leaves the history stale, so switching
        // it back on has to start over rather than reproject onto whatever was there.
        self.history_valid = taa_on;
    }

    /// Encode and submit only the present pass: the blit over the last converged frame,
    /// and the HUD over that, at roughly a tenth of a millisecond of GPU.
    ///
    /// **Batch 70's idle half, and the other half of the claim `idle.rs` makes.** A
    /// still frame costs this instead of the ~10 ms the seven passes cost at a shore
    /// view. Nothing here advances `frame_index`, the jitter phase, the history parity
    /// or the timer ring: the blit reads [`Self::last_blit`], the same slot the last
    /// `render` presented from, so the image is bit-identical to re-presenting that
    /// frame -- which is the entire claim, and why it needs no vantage to verify. The
    /// HUD is the one thing that moves, and it is rebuilt every repaint.
    pub fn repaint(&mut self, target: &wgpu::TextureView) {
        let mut enc = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("repaint"),
            });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("present"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            self.blit.draw(&mut pass, self.last_blit);
            self.ui.draw(&self.gpu.queue, &mut pass, self.surface_size);
        }
        self.gpu.queue.submit(Some(enc.finish()));
    }

    /// What `tile_select` asked the pair buffer for on the last frame the readback ring
    /// delivered, or `None` before it has delivered one.
    pub fn pair_demand(&self) -> Option<PairDemand> {
        self.timer.valid.then(|| PairDemand {
            select: self.timer.counters[4],
            deferred: self.timer.counters[1],
            recover: self.timer.counters[5],
            cap: self.max_pairs,
        })
    }

    /// Print the truncation warning to stderr if the last frame overflowed the pair buffer,
    /// and say whether it did.
    ///
    /// Called from the headless modes rather than from `render`, because those are where a
    /// silently truncated frame turns into a number in a table. It stays quiet when the
    /// readback has nothing yet -- a warning that fires on the first frame of every run would
    /// be worth less than no warning at all.
    pub fn warn_if_truncated(&self) -> bool {
        let Some(d) = self.pair_demand() else {
            return false;
        };
        if !d.overflowed() {
            return false;
        }
        eprintln!(
            "warning: {TRUNCATION_MARK} -- tile_select emitted {} (tile, chunk) pairs against \
             MAX_PAIRS = {}.",
            d.worst(),
            d.cap
        );
        eprintln!(
            "         Geometry is missing from this frame and it will not look like it is. \
             Lower --width/--height/--scale, or --view-distance."
        );
        true
    }

    /// What `march` did, counted rather than timed (batch 53).
    ///
    /// **A count is the measurement a hot laptop cannot move**, which is the whole reason this
    /// exists: `docs/pitfalls.md` records `resolve`'s A side climbing 19% over seven rounds
    /// under heat soak, and roadmap P4 is an entry about a *traversal* -- a thing whose work
    /// is exactly countable. Every figure here is integer and reproducible to the unit.
    ///
    /// **It reads a buffer the shipping build already fills and adds nothing to any shader.**
    /// `march` has accumulated `h.iters` into `dbg` under `FLAG_HEATMAP` since the heatmap
    /// existed; all that was missing was a reader. A per-workgroup reduction would have been
    /// tidier and is exactly what must not be built -- `march` declares no workgroup memory
    /// and no barrier, and adding either to count the pass would change the pass being
    /// counted. See [`crate::config::Config::march_stats`].
    ///
    /// The flag's `atomicAdd` is real work, so a run taken this way is **not** a timing
    /// sample and the mode says so where it prints.
    pub fn march_stats(&self) -> MarchStats {
        let pixels = (self.size.0 as u64) * (self.size.1 as u64);
        let bytes = pixels * 4;
        let staging = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("march stats"),
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        enc.copy_buffer_to_buffer(&self.dbg.buf, 0, &staging, 0, bytes);
        self.gpu.queue.submit(Some(enc.finish()));
        staging.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        self.gpu
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .ok();
        let view = staging.slice(..).get_mapped_range().expect("march stats map");
        let iters: &[u32] = bytemuck::cast_slice(&view);
        let mut total: u64 = 0;
        let mut worst = 0u32;
        let mut touched: u64 = 0;
        let mut capped: u64 = 0;
        let mut hot: Vec<u32> = Vec::with_capacity(iters.len());
        for &i in iters {
            total += i as u64;
            worst = worst.max(i);
            touched += u64::from(i > 0);
            capped += u64::from(i > MARCH_MAX_ITERS);
            hot.push(i);
        }
        drop(view);
        staging.unmap();
        // Sorted descending, so the head is the expensive tail of the frame. The whole point
        // of the percentile row is that this distribution is nothing like flat: the heatmaps
        // at every vantage put the cost in a thin band of grazing rays, and a mean over the
        // frame is the one summary that cannot show that.
        hot.sort_unstable_by(|a, b| b.cmp(a));
        let share = |n: usize| -> f64 {
            let n = n.min(hot.len());
            let s: u64 = hot[..n].iter().map(|&i| u64::from(i)).sum();
            if total == 0 {
                0.0
            } else {
                100.0 * s as f64 / total as f64
            }
        };
        let pct = |p: f64| -> u32 {
            if hot.is_empty() {
                0
            } else {
                hot[(((100.0 - p) / 100.0) * hot.len() as f64) as usize % hot.len()]
            }
        };
        MarchStats {
            pixels,
            iters: total,
            worst,
            touched,
            capped,
            top_1: share(hot.len() / 100),
            top_10: share(hot.len() / 10),
            p50: pct(50.0),
            p90: pct(90.0),
            p99: pct(99.0),
            pairs: self.pair_demand(),
            chunks: self.last_chunk_count,
        }
    }

    /// Say -- loudly -- if this screen has more 8x8 tiles than the pair word can address.
    ///
    /// **The failure it catches is the one this project calls indistinguishable from success.**
    /// `tile_select` packs `(tile_id << 13) | chunk_id` into a `u32`, so past [`MAX_TILES`] the
    /// shift drops the high bits and a tile marches the chunks some *other* tile selected. The
    /// result is a frame -- lit, shaded, temporally stable, and wrong -- with nothing in the
    /// output to say so. `warn_if_truncated` has covered the pair *count* since batch 22 and
    /// this limit sat beside it unguarded for thirty-one batches, narrower than it looks:
    /// 7680x4320 is 518,400 tiles against 524,288, **98.9%**.
    ///
    /// Called from the headless paths beside `warn_if_truncated`, and from `resize`'s caller
    /// rather than `resize` itself so an interactive window that is dragged past the limit says
    /// so once per size rather than once per frame.
    pub fn warn_if_oversized(&self) -> bool {
        let n = u64::from(self.tiles.0) * u64::from(self.tiles.1);
        if n <= u64::from(MAX_TILES) {
            return false;
        }
        eprintln!(
            "warning: {OVERSIZE_MARK} -- {}x{} is {n} tiles of 8x8 against a {TILE_ID_BITS}-bit              tile field, which holds {MAX_TILES}.",
            self.size.0, self.size.1
        );
        eprintln!(
            "         Tiles past that index wrap and march another tile's chunks. The frame              will look plausible and be wrong. Lower --width/--height/--scale."
        );
        true
    }

    /// Read any RGBA8 texture back as tightly packed rows.
    pub fn read_texture(&self, tex: &wgpu::Texture, size: (u32, u32)) -> Vec<u8> {
        let (w, h) = size;
        let unpadded = w * 4;
        let padded = (unpadded + 255) & !255;
        let buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("screenshot"),
            size: (padded * h) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.gpu.queue.submit(Some(enc.finish()));
        buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        self.gpu
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .ok();
        let view = buf.slice(..).get_mapped_range().expect("screenshot map");
        let mut out = Vec::with_capacity((unpadded * h) as usize);
        for y in 0..h {
            let s = (y * padded) as usize;
            out.extend_from_slice(&view[s..s + unpadded as usize]);
        }
        drop(view);
        buf.unmap();
        out
    }

    pub fn vram_estimate(&self) -> u64 {
        self.inners.capacity()
            + self.leaves.capacity()
            + self.bricks.capacity()
            + self.chunk_buf.capacity()
            + self.vis.capacity()
            + self.hiz.capacity()
            + self.dbg.capacity()
            + self.shadow_targets.bytes()
            + (self.max_pairs as u64) * 8
            // Visibility buffer, plus the trace target and two history buffers at 8 bytes
            // a pixel each.
            + (self.size.0 as u64 * self.size.1 as u64 * 8) * 4
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.surface_format
    }
}

fn make_hdr_texture(
    device: &wgpu::Device,
    size: (u32, u32),
    label: &str,
) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        // Linear HDR. sRGB is not a legal storage format, so `resolve` cannot encode; the
        // blit tone-maps and encodes instead.
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&Default::default());
    (tex, view)
}

fn make_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    water_mottle: f32,
    tint_balance: bool,
) -> (wgpu::TextureView, wgpu::Sampler) {
    let layers = block::tex::COUNT;
    let size = textures::TEX_SIZE;
    let mips = textures::MIP_LEVELS;
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("block atlas"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: layers,
        },
        mip_level_count: mips,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        // Textures are authored as display colours; the sRGB suffix decodes them to linear
        // on every fetch, for free, and makes bilinear/mip filtering happen in linear too.
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let mut level = textures::build_atlas(water_mottle, tint_balance);
    let mut dim = size;
    for mip in 0..mips {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: mip,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &level,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(dim * 4),
                rows_per_image: Some(dim),
            },
            wgpu::Extent3d {
                width: dim,
                height: dim,
                depth_or_array_layers: layers,
            },
        );
        if mip + 1 < mips {
            level = textures::downsample(&level, dim, layers);
            dim /= 2;
        }
    }
    let view = tex.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("atlas sampler"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    });
    (view, sampler)
}

/// Batch 57's ambient-cube lattice, and batch 55's constant field when `fill` pins it.
///
/// **Six stacked slabs of one 3D texture, one per `normal_id`**, each `PROBE_DIM_XZ` x
/// `PROBE_DIM_Y` x `PROBE_DIM_XZ` probes at [`probe::SPACING`] blocks apart. A face samples
/// only the slab its own normal names, which is what makes a six-entry directional basis cost
/// the *one* tap batch 55 measured rather than six.
///
/// **World space, toroidal in X and Z, exact in Y.** There is no per-chunk slot and no slot
/// allocator: a probe's texel is `world / SPACING` taken modulo the field, so two chunks that
/// share a boundary write adjacent texels of one continuous lattice and the trilinear tap
/// reads across that boundary with nothing to interpolate between but two values of the same
/// function. That is roadmap P5b answered by construction for this field -- the apron the
/// roadmap planned exists only because a chunk-local grid has edges, and this one has none.
/// Y is exact rather than toroidal because `WORLD_HEIGHT / SPACING` is 128 on the nose, so the
/// field covers the world's whole column and the sampler clamps at the top and bottom of it.
///
/// **The 512-block period is safe because LOD 0 reaches 160 blocks.** `LodConfig::factor` is
/// 2.0 over a 64-block chunk with a 1.25 fade band, so two LOD-0 chunks that alias onto the
/// same texels are 512 blocks apart and cannot both be in front of the camera. What they can
/// both be is *resident*, for as long as the 240-frame unload grace lasts after a fast
/// traverse, and the loser of that race gets one stale cube until it is rebuilt -- a shading
/// factor, four blocks wide, on a chunk that is behind the camera.
///
/// **It is created holding 1.0**, and the field stores occlusion rather than shading for
/// exactly that reason. `shade_hit` multiplies `face_shade` by the tap, so a texel no bake has
/// reached yet renders `face_shade(id) * 1.0` -- the pre-batch-57 frame *to the bit*, not
/// merely close to it. An unstreamed chunk, a coarse LOD and a pinned field are all then
/// indistinguishable from the build before this one, which is the property `--no-probe-cube`
/// has to be able to claim.
///
/// **Baking the product instead is what the first build of this batch did**, and the sweep
/// caught it: `cave` moved 367,222 pixels at max delta 1, because 0.62 and 0.80 do not survive
/// a round trip through `f16` and 1.0 does.
///
/// `fill` is batch 55's and batch 56's: `Some(f)` pins every texel to `f` and **disables the
/// bake's uploads entirely**, which is the only way those two diagnostics still mean what
/// their batches measured. `--probe-tap --probe-fill 1.0` is bit-exact for the reason it
/// always was, `x * 1.0` for finite `x`; without the pin the field now holds real shading
/// factors and the tap would move the picture. `noise` fills per texel from a hash instead,
/// which is the build that ruled out the driver compressing a constant texture into a cost no
/// real field would pay.
///
/// 12 MB against 3.9 GB of idle VRAM, which is the resource this whole line of work exists to
/// spend -- see `CLAUDE.md`'s table.
/// f32 to IEEE half, for the probe field's fill and nothing else.
///
/// Hand-rolled rather than pulling in `half` for one diagnostic: the values written here are
/// positive and inside [0.04, 4] -- batch 57's occlusion is a fraction and batch 58's bounce
/// reaches 3.5 under snow -- so every exponent is well within range and a truncating mantissa
/// is exact for the one value that has to be. **1.0 goes to 0x3C00, which reads back as
/// exactly 1.0f**, and that is what makes every absence of both features bit-exact rather than
/// close: `amb * tap`, `face_shade * occl` and `floor_rgb * bounce` all reduce to their left
/// operand. The range in that first sentence used to read [0.5, 2] and batch 58 widened it;
/// nothing about the function changed, because the guard it needs is an exponent check and it
/// always had one.
fn f16_bits(v: f32) -> u16 {
    let b = v.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let exp = ((b >> 23) & 0xFF) as i32 - 127 + 15;
    let mant = ((b >> 13) & 0x3FF) as u16;
    if exp <= 0 {
        return sign;
    }
    if exp >= 31 {
        return sign | 0x7C00;
    }
    sign | ((exp as u16) << 10) | mant
}

/// Probes across the field in X and Z. **Held at a 512-block period under L5**: the note on
/// [`make_probe`] is why 512 is safe ("LOD 0 reaches 160"), and every claim [`probe::UPLOAD_REACH`]
/// makes names that period, so the axis widens with the spacing instead of the period
/// narrowing with it -- 512 is the constant and 128x4 is how it is spelled now.
pub const PROBE_DIM_XZ: u32 = (512 / probe::SPACING) as u32;
const _: () = assert!(PROBE_DIM_XZ == 128);
/// Probes up the field. `WORLD_HEIGHT / SPACING` exactly, so the field spans the world's whole
/// column and Y never wraps -- which is why the sampler clamps in V and repeats in U and W.
pub const PROBE_DIM_Y: u32 = (crate::math::WORLD_HEIGHT / probe::SPACING) as u32;
const _: () = assert!(PROBE_DIM_Y == 128);
// L5's arithmetic, checked against the adapter rather than assumed: the texture is
// PROBE_DIM_XZ x (PROBE_DIM_Y * 6) x PROBE_DIM_XZ = 128 x 768 x 128 of RGBA16F, ~100.7 MB
// of the hardware's 3.9 GB VRAM, and every axis is eightfold inside the 16384-texel 3D
// limit this GPU reports -- the tree once rejected a design on a limit it never probed,
// which is why the limit is named here and not hand-waved.
/// Bytes a probe occupies: `Rgba16Float`.
const PROBE_TEXEL: u32 = 8;

fn make_probe(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    fill: Option<f32>,
    noise: bool,
) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("probe field"),
        size: wgpu::Extent3d {
            width: PROBE_DIM_XZ,
            height: PROBE_DIM_Y * probe::AXES as u32,
            depth_or_array_layers: PROBE_DIM_XZ,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        // Half float rather than `Rgba8Unorm` because the field carries linear radiance with
        // no upper bound the moment roadmap R2 puts a bounce in it -- the same argument
        // `out_tex` makes -- and because the format decides the bytes each tap moves, which is
        // the half of batch 55's cost that is not instructions. Changing it invalidates both
        // of that batch's numbers.
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    // Unoccluded everywhere, so an unbaked lattice is `face_shade(id) * 1.0`.
    let texels = (PROBE_DIM_XZ * PROBE_DIM_Y * PROBE_DIM_XZ) as usize * probe::AXES;
    let mut data: Vec<u8> = Vec::with_capacity(texels * PROBE_TEXEL as usize);
    for i in 0..texels {
        for c in 0..4u32 {
            let v = match (fill, noise) {
                (Some(_), true) => {
                    // Any cheap decorrelated value in a sane range; the point is that no two
                    // texels agree, not what they say.
                    let h = (i as u32)
                        .wrapping_mul(2654435761)
                        .wrapping_add(c * 0x9E3779B9);
                    0.5 + (h >> 8) as f32 / (1u32 << 25) as f32
                }
                (Some(f), false) => f,
                (None, _) => 1.0,
            };
            data.extend_from_slice(&f16_bits(v).to_le_bytes());
        }
    }
    // The slabs stack along Y, so the whole texture is one write and the rows come out in the
    // order the loop pushed them: x fastest, then y, then z, with the slab index above y.
    //
    // **That ordering is the thing to get right and the thing nothing would report**, because
    // a wrong stride here reads a plausible neighbouring slab rather than garbage -- the same
    // failure mode `block_faces`' stride has, and the reason that one is an invariant.
    // `probe_slab_layout_matches_the_shader` in `tests/probe.rs` pins it against `probe_cube`.
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(PROBE_DIM_XZ * PROBE_TEXEL),
            rows_per_image: Some(PROBE_DIM_Y * probe::AXES as u32),
        },
        wgpu::Extent3d {
            width: PROBE_DIM_XZ,
            height: PROBE_DIM_Y * probe::AXES as u32,
            depth_or_array_layers: PROBE_DIM_XZ,
        },
    );
    let view = tex.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D3),
        ..Default::default()
    });
    (tex, view)
}

fn storage_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

/// The one module every compute pass is compiled from: `common.wgsl` and the six pass
/// files, concatenated in this order and no other.
///
/// It is public and it is the *only* copy because `--bin shaderstats` compiles the same
/// text offline to read register counts off the driver. A second concatenation over there
/// would be a second shader, and a register count for a shader nobody runs is worse than no
/// register count at all.
pub fn shader_source() -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}",
        include_str!("shaders/common.wgsl"),
        include_str!("shaders/tile_select.wgsl"),
        include_str!("shaders/march.wgsl"),
        include_str!("shaders/resolve.wgsl"),
        include_str!("shaders/taa.wgsl"),
        include_str!("shaders/shaft.wgsl"),
        include_str!("shaders/shadow.wgsl"),
    )
}

/// Every entry point `Renderer::new` builds a pipeline for, in dispatch order. `shaderstats`
/// walks this list, so a pass added without an entry here is a pass with no register count
/// rather than a silently wrong one.
pub const ENTRY_POINTS: [&str; 11] = [
    "shaft_scan",
    "tile_select",
    "finalize",
    "recover_select",
    "finalize_recover",
    "finalize_deferred",
    "march",
    "build_hiz",
    "water_sec",
    "resolve",
    "taa",
];

/// `march` and `resolve`, with the flags in `SPEC_MASK` folded in as `override` constants.
///
/// One line per specialized flag, and the value is read off the same mask the caller keyed
/// the cache with. `f64` is what WGSL overrides travel as; a bool takes 0.0 or 1.0.
///
/// Both passes get the identical constant list because both include `common.wgsl`, and it is
/// cheaper to hand `march` an override it does not read -- `SPEC_TINT` -- than to keep two
/// lists that could drift. naga folds the unread one away and the driver never sees it.
/// `layouts` are `(march, water_sec, resolve)`: only `march` keeps the plain one, since
/// batch 81b's split gives the other two their guest groups (see `SpecPipes.water_sec`).
fn make_spec(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    layouts: (&wgpu::PipelineLayout, &wgpu::PipelineLayout, &wgpu::PipelineLayout, &wgpu::PipelineLayout, &wgpu::PipelineLayout, &wgpu::PipelineLayout),
    spec: u32,
    spec_hi: u32,
    // 81c's override-value: config-fixed at startup, which is why it is not a spec bit.
    water_sec_shift: u32,
) -> SpecPipes {
    let constants = [
        ("SPEC_TINT", if spec & FLAG_TINT != 0 { 1.0 } else { 0.0 }),
        (
            "SPEC_FOLIAGE",
            if spec & FLAG_FOLIAGE != 0 { 1.0 } else { 0.0 },
        ),
        ("SPEC_WAVES", if spec & FLAG_WAVES != 0 { 1.0 } else { 0.0 }),
        // Batch 75 (roadmap A3), in FLAG_WAVES's shape and one flag-word later: the wet band
        // is an albedo multiply and the foam is a term on a depth already traced, but both
        // run every resident frame, so both fold out of the build whose world has no water.

        (
            "SPEC_WAVE_ANISO",
            if spec & FLAG_WAVE_ANISO != 0 { 1.0 } else { 0.0 },
        ),
        (
            "SPEC_WAVE_SHOAL",
            if spec & FLAG_WAVE_SHOAL != 0 { 1.0 } else { 0.0 },
        ),
        (
            "SPEC_WAVE_FILL",
            if spec & FLAG_WAVE_FILL != 0 { 1.0 } else { 0.0 },
        ),
        (
            "SPEC_TERRAIN_SHAFTS",
            if spec & FLAG_TERRAIN_SHAFTS != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_TEX_VARIATION",
            if spec & FLAG_TEX_VARIATION != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_DISTANT_SHADOWS",
            if spec & FLAG_DISTANT_SHADOWS != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_LEAF_CUTOUT",
            if spec & FLAG_LEAF_CUTOUT != 0 {
                1.0
            } else {
                0.0
            },
        ),
        ("SPEC_GLASS", if spec & FLAG_GLASS != 0 { 1.0 } else { 0.0 }),
        (
            "SPEC_FLAT_SECONDARY",
            if spec & FLAG_FLAT_SECONDARY != 0 {
                1.0
            } else {
                0.0
            },
        ),
        // The one entry here that is a number rather than a switch, and the only reason both
        // values live on this side is that a WGSL literal cannot be A/B'd from a flag. See
        // `WATER_SHADOW_DIST`.
        (
            "SPEC_WATER_SHADOW_DIST",
            if spec & FLAG_WATER_SHADOW_CUT != 0 {
                WATER_SHADOW_DIST as f64
            } else {
                WATER_SHADOW_DIST_PRE41 as f64
            },
        ),
        // The second number here, and the reason both values live on this side is the one
        // above it: a WGSL literal cannot be A/B'd from a flag. See `LEAF_FILL`.
        (
            "SPEC_PROBE_AMBIENT",
            if spec & FLAG_PROBE_AMBIENT != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_PROBE_TAP",
            if spec & FLAG_PROBE_TAP != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_PROBE_CUBE",
            if spec & FLAG_PROBE_CUBE != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_PROBE_BOUNCE",
            if spec & FLAG_PROBE_BOUNCE != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_LIGHT_RGB",
            if spec & FLAG_LIGHT_RGB != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_PROBE_SUN",
            if spec & FLAG_PROBE_SUN != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_PROBE_SUN_HIGH",
            if spec & FLAG_PROBE_SUN_HIGH != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_SKY_TINT",
            if spec & FLAG_SKY_TINT != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_LEAF_FILL",
            if spec & FLAG_LEAF_THIN != 0 {
                LEAF_FILL as f64
            } else {
                LEAF_FILL_PRE43 as f64
            },
        ),
        // The first entry driven by the second specialization word rather than by
        // `frame.flags`, which had no bit left. See `SPEC_HI_MASK`.
        (
            "SPEC_SKY_SPECULAR",
            if spec_hi & FLAG_HI_SKY_SPECULAR != 0 {
                1.0
            } else {
                0.0
            },
        ),
        // Batch 72, second word as well, for the same reason.
        (
            "SPEC_FULL_MARCH",
            if spec_hi & FLAG_HI_FULL_MARCH != 0 {
                1.0
            } else {
                0.0
            },
        ),
        // Batch 73 (roadmap P11), second word and the first *world*-keyed entry on it:
        // on when an emitter is resident, off when the world provably has none.
        (
            "SPEC_EMITTER_GATHER",
            if spec_hi & FLAG_HI_EMITTER_WORLD != 0 {
                1.0
            } else {
                0.0
            },
        ),
        // Batch 75's pair, moved here from the first word by the verification pass: the
        // override table is the only gate the shader reads, so these two sit beside the
        // other hi entries rather than among things `frame.flags` carries.
        (
            "SPEC_SHORE_WET",
            if spec_hi & FLAG_HI_SHORE_WET != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_SHORE_FOAM",
            if spec_hi & FLAG_HI_SHORE_FOAM != 0 {
                1.0
            } else {
                0.0
            },
        ),
        // Batch 81 (roadmap P12). On the second word for the same reason the whole row
        // above it is: the shader reads the override, never the bit.
        (
            "SPEC_WATER_SEC",
            if spec_hi & FLAG_HI_WATER_SEC != 0 {
                1.0
            } else {
                0.0
            },
        ),
        // Batch 90 (roadmap A9). Second word, shader reads the override never the bit:
        // the two `wave_octave` reads below it are pure ALU and exist solely to be A/B'd
        // by the by-eye pass, so the shipping build must not contain them at all.
        (
            "SPEC_CAUSTICS",
            if spec_hi & FLAG_HI_CAUSTICS != 0 {
                1.0
            } else {
                0.0
            },
        ),
        // Batch 90 (roadmap A4). Same shape: the traced reflection replaces a cheaper
        // assumption at the very end of `shade_glass`, and holding the extra march out of
        // the shipping pipeline is the entire reason the entry is a flag and not a commit.
        (
            "SPEC_GLASS_REFLECT",
            if spec_hi & FLAG_HI_GLASS_REFLECT != 0 {
                1.0
            } else {
                0.0
            },
        ),
        // Batch 91 (roadmap A2c). First batch-90-family bit whose override the *marcher*
        // reads rather than `resolve` -- the door is the same `override` mechanism, because
        // every pass compiles from one module and this one's block folds out at the same
        // price as the glass arm's.
        (
            "SPEC_SNELL_BEND",
            if spec_hi & FLAG_HI_SNELL_BEND != 0 {
                1.0
            } else {
                0.0
            },
        ),
        // Batch 92 (roadmap A8). A constant *swap* rather than new code: the balanced
        // table reads the same registers the unbalanced one did, so the off pipeline is
        // bit-identical and the on one is the same instructions with other immediates.
        (
            "SPEC_TINT_BALANCE",
            if spec_hi & FLAG_HI_TINT_BALANCE != 0 {
                1.0
            } else {
                0.0
            },
        ),
        // Batch 95 (roadmap G1). One bit folds four guarded look arms out of the unarmed
        // module -- the bit-exact-by-construction control the uniform-guard class failed to
        // give; the knobs stay uniforms past it, so only the first non-zero value pays a
        // rebuild and the ladders sweep for free.
        (
            "SPEC_SKY_LOOK",
            if spec_hi & FLAG_HI_SKY_LOOK != 0 {
                1.0
            } else {
                0.0
            },
        ),
        // Batch 97a (roadmap G3's sky half). Constant-swap class: one bit recolours the
        // gradient and the deck; off folds the selects dead and the pre-97 immediates
        // stand bit for bit. No knobs -- the by-eye pass reads warm vs cool, period.
        (
            "SPEC_SKY_COOL",
            if spec_hi & FLAG_HI_SKY_COOL != 0 {
                1.0
            } else {
                0.0
            },
        ),
        // Batch 97c (roadmap G3's water half), 97d and 97e (roadmap G2's two halves):
        // new code behind overrides, each folding its whole guarded block out of the
        // unarmed module. Bit clear, the arm's text is not in the compiled shader --
        // the only revert story a pure-ALU look arm can honestly give.
        (
            "SPEC_WATER_LOOK",
            if spec_hi & FLAG_HI_WATER_LOOK != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_FOLIAGE_RICH",
            if spec_hi & FLAG_HI_FOLIAGE_RICH != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_CANOPY_RELIEF",
            if spec_hi & FLAG_HI_CANOPY_RELIEF != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_WIND_SWAY",
            if spec_hi & FLAG_HI_WIND_SWAY != 0 {
                1.0
            } else {
                0.0
            },
        ),
        // Batch 102a (`--soft-shadows`), in the 101f shape exactly: new code behind the
        // override, folded whole out of the unarmed module -- the only revert story a
        // feature whose cost is its rays can honestly give. With the bit clear the tap
        // cone is not in the compiled shader and `shadow_ray`'s one bit stands as before.
        (
            "SPEC_SOFT_SHADOWS",
            if spec_hi & FLAG_HI_SOFT_SHADOWS != 0 {
                1.0
            } else {
                0.0
            },
        ),
        // ... and its scale is an override *value* like `WATER_SHADOW_DIST`, for the same
        // reason: the shader divides, shifts and loops against it instead of guarding.
        ("SPEC_COMPACT_SHADE_HIT", if spec_hi & FLAG_HI_COMPACT_SHADE_HIT != 0 { 1.0 } else { 0.0 }),
        ("SPEC_ISOLATE_GLASS", if spec_hi & FLAG_HI_ISOLATE_GLASS != 0 { 1.0 } else { 0.0 }),
        ("SPEC_SHADOW_PASS", 1.0),
        ("SHADOW_RADIUS_LIMIT", if spec_hi & FLAG_HI_SOFT_HQ != 0 { 8.0 } else { 4.0 }),
        ("SPEC_LIGHTING_REPAIR", if spec_hi & FLAG_HI_LEGACY_LIGHTING == 0 { 1.0 } else { 0.0 }),
        ("SPEC_WATER_REPAIR", if spec_hi & FLAG_HI_LEGACY_WATER == 0 { 1.0 } else { 0.0 }),
        ("SOFT_PENUMBRA", if spec_hi & FLAG_HI_SUN_NARROW != 0 { 1.0 } else if spec_hi & FLAG_HI_SUN_WIDE != 0 { 3.0 } else { 1.5 }),
        ("SURFACE_DEBUG", ((spec_hi >> 23) & 7) as f64),
        ("WATER_SEC_SHIFT", water_sec_shift as f64),
    ];
    let mk = |name: &'static str, label: &'static str, layout: &wgpu::PipelineLayout, lane: f64| {
        let mut selected = constants.to_vec();
        selected.push(("RESOLVE_LANE", lane));
        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(label), layout: Some(layout), module, entry_point: Some(name),
            compilation_options: wgpu::PipelineCompilationOptions {
                constants: &selected, zero_initialize_workgroup_memory: true,
            }, cache: None,
        })
    };
    let shadow = true;
    let glass = shadow || spec_hi & FLAG_HI_ISOLATE_GLASS != 0;
    SpecPipes {
        march: mk("march", "march", layouts.0, 0.0),
        resolve: mk("resolve", "resolve main", layouts.2, 0.0),
        water_sec: mk("water_sec", "water secondary legs", layouts.1, 0.0),
        primary_shadow: shadow.then(|| mk("primary_shadow", "primary shadow", layouts.3, 0.0)),
        shadow_blurs: [mk("shadow_horizontal", "shadow horizontal", layouts.4, 0.0),mk("shadow_vertical", "shadow vertical", layouts.5, 0.0)],
        glass_resolve: glass.then(|| mk("resolve", "glass resolve", layouts.2, 1.0)),
        water_resolve: shadow.then(|| mk("resolve", "water resolve", layouts.2, 2.0)),
    }
}

/// Entries in bind group 0, which must match the `@group(0) @binding(N)` declarations at the
/// top of `common.wgsl` one for one.
///
/// This number used to live in a sentence in `CLAUDE.md`, which is the same place the test
/// count and the bytes-per-voxel figure lived before batch 28 found both of them stale. A
/// count that only the code knows does not belong in prose: the annotation below fails to
/// compile if an entry is added here, and `bind_group_0_matches_the_shader` fails if one is
/// added on the WGSL side.
pub const BIND_GROUP_0_ENTRIES: usize = 22;

fn make_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let entries: [wgpu::BindGroupLayoutEntry; BIND_GROUP_0_ENTRIES] = [
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            storage_entry(1, true),
            storage_entry(2, true),
            storage_entry(3, true),
            storage_entry(4, true),
            storage_entry(5, true),
            storage_entry(6, false),
            storage_entry(7, false),
            storage_entry(8, false),
            storage_entry(9, false),
            storage_entry(10, false),
            wgpu::BindGroupLayoutEntry {
                binding: 11,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::Rgba16Float,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 12,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 13,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            storage_entry(14, false),
            storage_entry(15, true),
            sampled_entry(16),
            sampled_entry(17),
            wgpu::BindGroupLayoutEntry {
                binding: 18,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            storage_entry(19, false),
            // Batch 55's probe field. `D3` and filterable, because the whole quantity being
            // measured is one hardware trilinear tap; a storage buffer would be eight loads and
            // a manual lerp, which is a different cost and not the one L1 would pay.
            wgpu::BindGroupLayoutEntry {
                binding: 20,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D3,
                    multisampled: false,
                },
                count: None,
            },
            // Batch 57's probe sampler. Its own rather than `linear_sampler` because the
            // field wraps in X and Z and does not in Y -- see the field on `Renderer`.
            wgpu::BindGroupLayoutEntry {
                binding: 21,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
    ];
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("voxel bgl"),
        entries: &entries,
    })
}

/// The two-view group 81b's split consults: one layout, the two water textures in order
/// -- refr then refl -- regardless of the role the layout claims for them.
fn make_water_sec_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    views: (&wgpu::TextureView, &wgpu::TextureView),
    label: &str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(views.0),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(views.1),
            },
        ],
    })
}

/// A WriteOnly `Rgba16Float` storage 2D -- the `water_sec` half of batch 81b's split.
fn storage_tex_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format: wgpu::TextureFormat::Rgba16Float,
            view_dimension: wgpu::TextureViewDimension::D2,
        },
        count: None,
    }
}

fn sampled_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn make_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    frame: &wgpu::Buffer,
    chunks: &DynBuffer,
    inners: &GpuMirror,
    leaves: &GpuMirror,
    bricks: &GpuMirror,
    faces: &wgpu::Buffer,
    pairs: &wgpu::Buffer,
    counters: &wgpu::Buffer,
    indirect: &wgpu::Buffer,
    vis: &DynBuffer,
    hiz: &DynBuffer,
    out_view: &wgpu::TextureView,
    atlas: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    dbg: &DynBuffer,
    grid: &wgpu::Buffer,
    // Bindings 16 and 17: the history the temporal pass reads, and the colour it reads.
    // The main passes use neither, and bind the two history buffers here so that nothing in
    // the group aliases `out_view`, which they write.
    temporal: (&wgpu::TextureView, &wgpu::TextureView),
    linear: &wgpu::Sampler,
    shaft: &wgpu::Buffer,
    probe: &wgpu::TextureView,
    probe_samp: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("voxel bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: frame.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: chunks.buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: inners.buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: leaves.buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: bricks.buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: faces.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: pairs.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: counters.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 8,
                resource: indirect.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 9,
                resource: vis.buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 10,
                resource: hiz.buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 11,
                resource: wgpu::BindingResource::TextureView(out_view),
            },
            wgpu::BindGroupEntry {
                binding: 12,
                resource: wgpu::BindingResource::TextureView(atlas),
            },
            wgpu::BindGroupEntry {
                binding: 13,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 14,
                resource: dbg.buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 15,
                resource: grid.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 16,
                resource: wgpu::BindingResource::TextureView(temporal.0),
            },
            wgpu::BindGroupEntry {
                binding: 17,
                resource: wgpu::BindingResource::TextureView(temporal.1),
            },
            wgpu::BindGroupEntry {
                binding: 18,
                resource: wgpu::BindingResource::Sampler(linear),
            },
            wgpu::BindGroupEntry {
                binding: 19,
                resource: shaft.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 20,
                resource: wgpu::BindingResource::TextureView(probe),
            },
            wgpu::BindGroupEntry {
                binding: 21,
                resource: wgpu::BindingResource::Sampler(probe_samp),
            },
        ],
    })
}




