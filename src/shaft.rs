//! The light envelope: terrain occlusion for the crepuscular shaft, as a height rather than
//! as a march.
//!
//! **The whole batch is one substitution.** `sun_shaft` asks, at four points along every view
//! ray, whether the sun reaches that point. Batch 11 answered it with a `shadow_ray` and
//! measured **+10.57 ms** of `resolve` against a frame that costs 4 -- the sweep is in the
//! batch-35 row of [`docs/ledger.md`] and the rejection in `docs/errors.md`. The cheap corner
//! of that sweep finds nothing, because the ridge casting the shaft is 300 to 600 blocks away.
//!
//! What replaces it is a projection. For a *directional* light the shadow of a heightfield is
//! itself a heightfield: at each column there is one altitude below which the sun is blocked
//! and above which it is not. Call that altitude the **envelope**. Then
//!
//! ```text
//!     lit(p)  <=>  p.y > E(p.xz)
//! ```
//!
//! and the search along the third dimension -- the part that costs `O(reach)` per query in a
//! tree -- has been done once for the whole world instead of once per sample.
//!
//! **`E` is an altitude and not an angle, and that distinction is the entire reason this
//! works.** Horizon mapping stores the maximum elevation angle around each column, which
//! answers a query *on the surface* and cannot answer one in mid-air: the angle to the ridge
//! that set it depends on the distance to that ridge, and a horizon map has thrown the
//! distance away. An envelope has no such problem, because the occluder's height and range
//! have already been combined into the one number the comparison needs.
//!
//! ## What it approximates away
//!
//! The envelope is exact for a world that *is* a heightfield and wrong exactly where ours is
//! not, which is the bargain to check rather than to assume:
//!
//! - **Overhangs and arches** cast a solid column of shadow to the ground. Ours come only from
//!   a cave breaking the surface, which is rare and sub-pixel at ridge range.
//! - **Caves** read as solid mountain. For an outdoor shaft that is *correct*; it is wrong only
//!   looking out of a cave mouth, and a cave is not a vantage the haze lobe survives.
//! - **Trees** are absent, so a shaft slices through a forested crest that should have stopped
//!   it. The canopy is a known lift over the terrain and folding it in is a one-line change --
//!   deliberately not taken here, because it is a look decision with its own constant and this
//!   batch is the mechanism.
//! - **Player edits** are absent, so a tower on a ridge casts a ground shadow and no shaft.
//!
//! All four are in `docs/errors.md` with what it would take to fix them.
//!
//! ## Why the heightfield is free here and is not free in general
//!
//! Two properties of *this* engine, neither of which a shadow map has:
//!
//! - `WorldGen::height` is a pure function of the column, so the envelope can cover ground
//!   **no chunk has been streamed for**. A structure built by projecting resident geometry
//!   loses the ridge the moment it falls outside the streaming radius, which at a low sun is
//!   exactly when the shaft wants it.
//! - The field is static per column. Only the *sweep* depends on where the sun is, so the
//!   per-frame cost is a scan and the fill is paid when the window scrolls.

use crate::worldgen::WorldGen;

/// Texels per side of the window. 512 at [`TEXEL`] blocks each is a 2048-block square, which
/// puts the camera 1024 blocks from every edge -- and 1024 is [`STEPS`]' reach, so a sample
/// anywhere in the window can see an occluder as far away as the scan can carry one. Sizing
/// these two independently would mean one of them was paid for and unused.
pub const DIM: usize = 512;

/// Blocks per texel. **The resolution of a shadow edge, and the one knob to reach for if the
/// shaft reads blocky.** 4 is chosen against the terrain rather than against the screen: the
/// worldgen amplitudes are kept well under the noise wavelengths so slopes stay walkable, so
/// a 4-block cell cannot hide a crest that matters at ridge range. On screen 4 blocks at 400
/// is about seven pixels at 720p, which the bilinear tap spreads into a ramp rather than a
/// step -- and a ramp is what a penumbra is anyway. Doubling the resolution costs 4x the fill
/// and 4x the buffer.
pub const TEXEL: i32 = 4;

/// Doubling passes in the scan. Each pass doubles the reach, so `n` passes see `2^n` texels:
/// 8 gives 256 texels, which is **1024 blocks** -- past the 300-to-600 the ridges sit at, and
/// matched to the half-width of the window above.
///
/// The scan is a max-plus prefix scan (Hillis-Steele in the `(max, +)` semiring), which is what
/// buys a 256-texel reach for eight taps instead of 256. **It is near the brute-force maximum
/// rather than equal to it**: the doubling is exact over an integer lattice and the sun's
/// azimuth is not axis-aligned, so each pass resamples bilinearly. Measured at **0.6 blocks**
/// against the real generator, a seventh of [`SOFT`], and pinned against that constant rather
/// than a float epsilon by `the_scan_is_the_envelope_it_claims_to_be`.
pub const STEPS: u32 = 8;

/// How far the shadow edge is ramped, in blocks of altitude.
///
/// **Two jobs and one number.** A shadow cast from `d` blocks away is blurred by the sun's own
/// angular size, `d * SUN_ANGLE` -- about 3.7 blocks at 400, which is the penumbra a hard edge
/// would be wrong to omit. And the envelope is quantised at [`TEXEL`], so the same ramp is what
/// keeps the cell grid from reading as a staircase. Both want the same few blocks, so this is
/// one constant rather than two that would have to be kept apart.
pub const SOFT: f32 = 4.0;

/// Pass `k` writes slice `k & 1`, so an even number of passes leaves the answer in slice 1.
/// `SHAFT_RESULT` in `common.wgsl` is the shader's copy of that and this is what pins it.
const _: () = assert!((STEPS - 1) & 1 == 1);

/// Floats in the GPU buffer: two slices the scan ping-pongs between, and one holding the
/// static heights the CPU fills. One buffer rather than three because the scan reads a slice
/// and writes a slice, and a single storage array makes that an offset instead of a second
/// binding in a different address space.
pub const BUFFER_FLOATS: usize = 3 * DIM * DIM;

/// Where the static heights live in that buffer.
pub const HEIGHTS_BASE: usize = 2 * DIM * DIM;

/// The terrain heights under a window of the world, anchored on the camera.
///
/// Owned by the caller and not by the renderer, because filling it needs `WorldGen` and the
/// renderer has never been given one -- and should not be, since the same field also answers
/// where the ground is for a dozen callers that have nothing to do with drawing.
pub struct ShaftField {
    heights: Vec<f32>,
    /// Texel coordinates of texel (0, 0). World X of texel `i` is `(origin[0] + i) * texel`.
    origin: [i32; 2],
    /// Batch 92 (roadmap A8): the knob, as a value rather than [`TEXEL`]'s pin. Shipped at
    /// 4 on a slope argument, never swept since -- every test in `tests/shaft.rs` builds
    /// `new()` and so locks the shipping constant, and `with_texel` is the arm the sweep hangs
    /// on. The scan, the upload and the sampler never knew the constant: the shader has
    /// always read `frame.shaft_texel`.
    texel: i32,
    filled: bool,
    dirty: bool,
}

impl Default for ShaftField {
    fn default() -> Self {
        Self::new()
    }
}

impl ShaftField {
    pub fn new() -> Self {
        Self::with_texel(TEXEL)
    }

    /// The same field at a different quantisation (`--shaft-texel`, roadmap A8's never-swept
    /// knob). The window stays [`DIM`] texels per side: 512 times the texel is the span, so a
    /// finer grid is also a **shorter** window -- the trade the sweep is meant to read, not
    /// hide. Asserted range: the scan's climb reaches at most `DIM` texels and the win would
    /// be invisible below 1 or lost past 64.
    pub fn with_texel(texel: i32) -> Self {
        assert!((1..=64).contains(&texel), "TEXEL past the scan's reach");
        Self {
            heights: vec![0.0; DIM * DIM],
            origin: [0, 0],
            texel,
            filled: false,
            dirty: false,
        }
    }

    /// Blocks per texel of this field: what the frame uniform must say for the shader and
    /// the fill to share a co-ordinate system.
    pub fn texel(&self) -> i32 {
        self.texel
    }

    /// Re-anchor the window on the camera and fill whatever the shift exposed.
    ///
    /// **The shift is what keeps this off the frame budget.** Moving one texel exposes two
    /// strips of 512 columns; only a jump wider than the window pays for all 262,144, and the
    /// one place that happens is the first frame. An unmoved camera -- which is every headless
    /// capture -- fills once and never again.
    pub fn update(&mut self, gen: &WorldGen, cam_x: f32, cam_z: f32, canopy_lift: f32) {
        let half = (DIM / 2) as i32;
        let want = [
            (cam_x / self.texel as f32).floor() as i32 - half,
            (cam_z / self.texel as f32).floor() as i32 - half,
        ];
        self.dirty = false;
        if self.filled && want == self.origin {
            return;
        }
        let d = [want[0] - self.origin[0], want[1] - self.origin[1]];
        let mut next = vec![0.0f32; DIM * DIM];
        for zz in 0..DIM {
            let oz = zz as i32 + d[1];
            let row = zz * DIM;
            for xx in 0..DIM {
                let ox = xx as i32 + d[0];
                let carried = self.filled
                    && oz >= 0
                    && oz < DIM as i32
                    && ox >= 0
                    && ox < DIM as i32;
                next[row + xx] = if carried {
                    self.heights[oz as usize * DIM + ox as usize]
                } else {
                    // The texel's *centre*, so a cell is not biased toward the corner it
                    // happens to be indexed by. One sample and not the four corners: the
                    // slope bound in `TEXEL`'s comment is the argument, and four would be
                    // four times the fill for a couple of blocks of a quantity `SOFT`
                    // already ramps over.
                    let wx = (want[0] + xx as i32) * self.texel + self.texel / 2;
                    let wz = (want[1] + zz as i32) * self.texel + self.texel / 2;
                    let h = gen.height(wx, wz);
                    // Batch 90, roadmap A1 (`--canopy-lift`). The field the shafts march was
                    // filled from bare terrain height, so beams pass through canopy the
                    // camera renders. Lifting the read inside a tree-holding column hides
                    // the canopy from the field by exactly the authored amount; the gates
                    // replicate the tree placer's own three (`tree_scale` / above the
                    // beach / under the tree line) so a beach or a peak never lifts. At
                    // 0.0 the block folds to the unchanged expression -- it is the same
                    // sum of calls that was already inline -- and to the exact pre-arm
                    // field, which is the control this batch's A/B measures.
                    if canopy_lift > 0.0 {
                        let cbi = gen.column_biome(wx, wz);
                        if cbi.def().tree_scale > 0.0 && h > crate::worldgen::SEA_LEVEL + 2
                            && h <= cbi.tree_line
                        {
                            h as f32 + canopy_lift
                        } else {
                            h as f32
                        }
                    } else {
                        h as f32
                    }
                };
            }
        }
        self.heights = next;
        self.origin = want;
        self.filled = true;
        self.dirty = true;
    }

    /// World XZ of texel (0, 0) -- the shader turns a world position into a texel with this
    /// and [`TEXEL`], and it travels in `Frame` rather than being recomputed there because the
    /// camera the window was anchored on is not necessarily this frame's.
    pub fn origin_world(&self) -> [f32; 2] {
        [
            (self.origin[0] * self.texel) as f32,
            (self.origin[1] * self.texel) as f32,
        ]
    }

    pub fn heights(&self) -> &[f32] {
        &self.heights
    }

    /// Whether [`update`](Self::update) changed anything since the last upload.
    pub fn dirty(&self) -> bool {
        self.dirty
    }

    /// The envelope this field would produce on the CPU, for one column, by walking the
    /// window directly. **Only `tests/` calls it**: it is the independent implementation the
    /// GPU scan is checked against, and a max-plus scan is exactly the kind of thing that can
    /// be off by one doubling and still look plausible.
    pub fn envelope_reference(&self, sun: [f32; 3], tx: usize, tz: usize) -> f32 {
        let horiz = (sun[0] * sun[0] + sun[2] * sun[2]).sqrt().max(1e-3);
        let dir = [sun[0] / horiz, sun[2] / horiz];
        let slope = sun[1] / horiz;
        let mut best = self.heights[tz * DIM + tx];
        let reach = 1usize << STEPS;
        for m in 1..=reach {
            let sx = tx as f32 + dir[0] * m as f32;
            let sz = tz as f32 + dir[1] * m as f32;
            let h = self.sample(sx, sz);
            best = best.max(h - m as f32 * self.texel as f32 * slope);
        }
        best
    }

    /// Bilinear, clamped at the edge -- the same rule `shaft_sample` follows in WGSL, and it
    /// has to be the same rule or the reference above would be checking a different function.
    fn sample(&self, x: f32, z: f32) -> f32 {
        let hi = (DIM - 1) as f32;
        let cx = x.clamp(0.0, hi);
        let cz = z.clamp(0.0, hi);
        let x0 = cx.floor();
        let z0 = cz.floor();
        let fx = cx - x0;
        let fz = cz - z0;
        let x0 = x0 as usize;
        let z0 = z0 as usize;
        let x1 = (x0 + 1).min(DIM - 1);
        let z1 = (z0 + 1).min(DIM - 1);
        let a = self.heights[z0 * DIM + x0];
        let b = self.heights[z0 * DIM + x1];
        let c = self.heights[z1 * DIM + x0];
        let d = self.heights[z1 * DIM + x1];
        let top = a + (b - a) * fx;
        let bot = c + (d - c) * fx;
        top + (bot - top) * fz
    }
}



