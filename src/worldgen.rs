//! Procedural terrain: biome field, noise heightmap, spaghetti caves, trees. LOD-aware.

use crate::biome::{self, Biome, BiomeId};
use crate::block::*;
use crate::math::WORLD_HEIGHT;
use crate::voxel::{dense_index, ChunkKey, VOL};
use fastnoise_lite::{FastNoiseLite, FractalType, NoiseType};
use glam::IVec3;

/// Internal sea level (world y = 64). Every column whose terrain height is below this is
/// flooded to it, which is what turns the low basins into ocean rather than grass.
pub const SEA_LEVEL: i32 = 128;

/// Ground columns under one LOD-0 canopy: 5x5 with the four corners mostly cut away.
const LOD0_CANOPY_COLUMNS: f32 = 22.0;

/// Everything about a column that the biome field decides, sampled once and threaded
/// through the height, the palette and the tree placer. Two noise lookups produce it, so
/// it is worth the 16 bytes a chunk keeps one of per column rather than asking three times.
#[derive(Clone, Copy, Debug)]
pub struct ColumnBiome {
    pub id: BiomeId,
    /// Blended, not the table value: see `biome.rs` for why the shape has to be continuous
    /// where the palette does not.
    pub mountain_amp: f32,
    pub snow_line: i32,
    pub tree_line: i32,
}

impl ColumnBiome {
    const EMPTY: Self = Self {
        id: 0,
        mountain_amp: 0.0,
        snow_line: 0,
        tree_line: 0,
    };

    #[inline]
    pub fn def(&self) -> &'static Biome {
        biome::def(self.id)
    }
}

pub struct WorldGen {
    pub seed: i32,
    /// Water fills every column from its terrain height up to here. `--no-water` sets it
    /// to zero, which leaves the identical terrain with no water in it -- the A/B partner
    /// for every claim about water, in the world as much as in the renderer.
    ///
    /// The shore and tree-line rules deliberately stay on the `SEA_LEVEL` **constant**, so
    /// that toggling this changes the water and nothing else.
    pub sea_level: i32,
    /// The biome field. `--no-biomes` clears it and collapses every column to plains with
    /// no snow line and no tree line, which is exactly the pre-batch-9 rule set: the same
    /// shape of control `--no-water` is for batch 8, and pinned bit for bit by
    /// `tests/worldgen.rs::no_biomes_reproduces_the_pre_batch9_world`.
    pub biomes: bool,
    /// Batch 14's ground cover. `--no-foliage` clears it and the generator produces the
    /// pre-batch-14 world bit for bit, pinned by `tests/biome.rs::PRE_BATCH14`.
    ///
    /// **Forced off when `biomes` is off**, which is not a shortcut: `--no-biomes` is defined
    /// as the pre-batch-9 rule set and ground cover did not exist then, so letting it survive
    /// that flag would break `PRE_BATCH9` and cost batch 9 its control. Batch 12's tint is
    /// cleared by `--no-biomes` for the same reason and this follows it.
    pub foliage: bool,
    /// Batch 18's coarse stand-in for the above. `--no-meadow` clears it and the generator
    /// produces the pre-batch-18 world bit for bit, pinned by `tests/biome.rs::PRE_BATCH18`.
    ///
    /// **Forced off when `foliage` is off**, and so off when `biomes` is off as well: it is
    /// a compensation for ground cover ending at the LOD 0 boundary, and a world with no
    /// ground cover has no boundary to compensate for. That AND is also what keeps
    /// `PRE_BATCH14` and `PRE_BATCH9` reproducing worlds that predate this.
    pub meadow: bool,
    /// Batch 60's second bounce: weight each ground sample the probe bake gathers by the sky
    /// that sample itself can see. `--no-probe-shadow` clears it and the bake returns batch
    /// 58's field bit for bit, because the term it removes is a multiply by exactly 1.0.
    ///
    /// **It lives here rather than in a `SPEC_MASK` override, and the reason is which side of
    /// the frame it is on.** Every other probe control switches off a *read*: the field is
    /// baked either way and the shader ignores it, which costs nothing and reverts exactly.
    /// This one changes what is *in* the field, so no pipeline override can undo it and the
    /// flag has to reach `probe::bake` -- which receives `gen` and nothing else that could
    /// carry it. It is `--no-glass`'s shape without `--no-glass`'s caveat: that control could
    /// not revert the lighting its world change had baked, and this one reverts completely,
    /// because an exposure of 1.0 is the identity of the only operation it takes part in.
    pub bounce_shadow: bool,
    /// Batch 91 (roadmap A5), `--tree-blue-noise`: thin the coarse canopy proxies with the
    /// R3 rank field instead of stamping them solid. **Default off** per batch 90's standing
    /// law, and off is the pre-arm world exactly: the only code the flag adds is a rank
    /// comparison in front of one `b.leaf` write.
    pub tree_blue_noise: bool,
    /// Batch 101e (roadmap G2), `--grass-dense`: the floor of the reference gap in ground
    /// cover -- dense two-height tufts at LOD 0, blue-noise tuft proxies on the coarse
    /// shell, and reeds around the wet banks. A **separate** arm class from batch 98's
    /// `--foliage-rich`: G1's marcher polish is dead until this arm's albedo exists, and
    /// this arm's extra geometry is where the marcher will be judged.
    pub grass_dense: bool,
    /// Batch 92 (roadmap A8, piece 4), `--meadow-side`: stamp `MEADOW_SIDE` instead of
    /// `MEADOW` -- one id write -- so the coarse repaint reaches the top row's *side* faces
    /// on steep ground. The entry said one atlas slice and only that: the albedo-and-view
    /// question and the proxy-tree residual stay exactly as open as they were.
    pub meadow_side: bool,
    continental: FastNoiseLite,
    hills: FastNoiseLite,
    detail: FastNoiseLite,
    forest: FastNoiseLite,
    cave_a: FastNoiseLite,
    cave_b: FastNoiseLite,
    gravel: FastNoiseLite,
}

fn noise(seed: i32, freq: f32, octaves: i32) -> FastNoiseLite {
    let mut n = FastNoiseLite::with_seed(seed);
    n.set_noise_type(Some(NoiseType::OpenSimplex2));
    n.set_frequency(Some(freq));
    if octaves > 1 {
        n.set_fractal_type(Some(FractalType::FBm));
        n.set_fractal_octaves(Some(octaves));
    }
    n
}

#[inline]
/// Batch 91 (roadmap A5): the blue-noise rank field the canopy decimation reads.
///
/// **Why an R3 sequence and not a hash.** White noise per voxel keeps 62% of cells but keeps
/// them in clumps, and a clumped hole pattern is just the LOD cliff with finer grain. The
/// generalized-golden-ratio recurrence (`phi_3 = 1.4656`, steps `1/phi_3, 1/phi_3^2, 1/phi_3^3`)
/// evaluated on the integer lattice is the classic low-discrepancy dither: its sub-threshold
/// sets are maximally even, which is what a rank field *is* -- cell ranks fill space evenly
/// at every density, so thinning to any `p` reads as haze rather than pepper. `frac` on f64,
/// which is exact for every world coordinate this generator will ever see.
///
/// The published threshold-tolerance note: consecutive cells are distant in rank only
/// *locally* -- the sequence cycles, so very long runs re-correlate. At a canopy proxy of
/// five cells across it never sees five.
///
/// Exposed (hidden) for the worldgen test's blue-noise measurement: batch 94's
/// blue/brown claims were measured on this *function* at world-coordinate step 4,
/// and the test re-earns them directly here -- on the stamped shell, vertical pairs
/// among survivors are structurally absent at LOD 2, so chunk pixels cannot see the
/// field's y property. Pipeline readers stay on the coarse-stamp call site below.
#[doc(hidden)]
pub fn canopy_rank3(wx: i32, wy: i32, wz: i32, seed: i32) -> f32 {
    const A1: f64 = 0.682_327_803_828_019_3;
    const A2: f64 = 0.465_571_231_876_854_6;
    const A3: f64 = 0.317_667_222_756_072_5;
    // The seed decorrelates worlds, not adjacent cells: a constant phase shift keeps every
    // cell's *relative* rank on the lattice, so two seeds differ by which trees a panel of
    // canopy survives, not by the pattern.
    let phase = (seed as f64).fract().abs();
    let r = (wx as f64).mul_add(A1, (wz as f64).mul_add(A3, (wy as f64).mul_add(A2, phase)));
    (r.fract()).abs() as f32
}

/// How much of a coarse canopy proxy survives the batch-91 rank field: exactly
/// [`crate::render::LEAF_FILL`], because the flag's own claim is *mean transmittance between
/// LOD 0 and the coarser levels is equal* -- the LOD-0 carve keeps a `LEAF_FILL` share of each
/// leaf block's 4^3 cells, so a slice through a canopy passes 1 - 0.62 there, and a coarse
/// slice passes 1 - keep here. Mirrored rather than imported for the usual reason (worldgen
/// otherwise never reads `render`), and pinned on that line by the test below. Exposed
/// (hidden) so the worldgen test's field-lattice measurement gates survivors with the
/// production threshold instead of a drifted literal.
#[doc(hidden)]
pub const COARSE_CANOPY_KEEP: f32 = 0.62;

// The pin the comment promises: the mirrored value and `render::LEAF_FILL` cannot drift --
// mirroring was a line-numbering decision, the value is a correctness one, and a const
// assert costs a pipeline nothing.
const _: () = assert!(
    COARSE_CANOPY_KEEP.to_bits() == crate::render::LEAF_FILL.to_bits(),
    "COARSE_CANOPY_KEEP must equal render::LEAF_FILL bit for bit"
);

fn hash2(x: i32, z: i32, seed: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8da6b343)
        ^ (z as u32).wrapping_mul(0xd8163841)
        ^ (seed as u32).wrapping_mul(0xcb1ab31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1e995);
    h ^ (h >> 15)
}

/// Whether a world generated with these two options holds ground cover. See the field: the
/// pre-batch-9 control has to keep reproducing a world that predates it.
///
/// It is a function rather than an expression written twice because batch 15 gave the answer
/// a second reader. `render::FLAG_FOLIAGE` is set from this, `WorldGen` stores it, and
/// `SPEC_FOLIAGE` compiles the marcher's ability to recognise a tuft out of the module when
/// it is false -- so a renderer that disagreed with the generator would not merely look like
/// a different control, it would draw every tuft in the world as an opaque cube.
pub const fn has_foliage(foliage: bool, biomes: bool) -> bool {
    foliage && biomes
}

/// How much more ground a tuft hides than the column it stands in.
///
/// A blade is a *vertical* quad, so from any vantage that is not straight down it leans
/// across its neighbours and hides them too. Measured at the vantage the batch is for:
/// inside a dense carpet 87.1% of the ground pixels are blade, where the columns under them
/// carry a tuft only about half the time. 1.75 is that ratio.
///
/// This is what `coarse_meadow` stamps at, and it is a **shape** constant rather than a
/// fitted one -- in the biome the sweep was taken in it saturates at 1.0 and constrains
/// nothing. What it decides is how fast a thinner carpet reaches a closed one, which is the
/// only thing separating plains from taiga here: at `grass_scale` 1.00 the coarse ground is
/// painted almost everywhere, at 0.45 it is painted on about four columns in ten.
///
/// The alternative was stamping at `ground_cover` unscaled, so the coarse *column* rate
/// matched LOD 0's exactly. That is the wrong quantity by exactly this factor, and paid for
/// it twice: the ground came out 1.6x too light, and darkening the paint to compensate put
/// 8-block blotches of near-black green across a stride-8 hillside.
pub const MEADOW_SPREAD: f32 = 1.75;

// --- Batch 101e (`--grass-dense`, the G2 generator arm) -----------------------------
//
// The references' grass is not one question but three, and the numbers below are the
// three answers, each authored against the references and each independently necessary:
//
// * **Density**: the plates' fields are meadows, not tufts-on-dirt -- LOD 0's baseline
//   carpet is sparser than any of them at the same noise field. Multiplied rather than
//   re-noised so cover still thickens and thins with `ground_cover`'s forest field, only
//   thicker in half.
pub const G2_GRASS_DENSITY: f32 = 2.5;
/// * **Height**: the references' stands have a profile -- a green under-layer with dry
///   twos--metre tussocks through it. A *share* (parts per 100) rather than a noise, so
///   the tussocks keep batch 14's clumping exactly and only trade height.
pub const G2_TALL_SHARE: u32 = 24;
/// * **Range**: batch 14's tufts are LOD-0 voxels and vanish in a chunk-aligned line -- batch
///   18 repaints the coarse ground to hide exactly that. A5 showed the honest version for
///   canopies (blue-noise proxy thinned to transmittance); this is the same construction
///   for grass, one shared rank field so coarse tufts never pepper foreign biomes at one
///   flat rate.
pub const G2_COARSE_KEEP: f32 = 0.5;
/// * **Range, one stride further**: batch-101's first hardware round measured the
///   stride-2-only ceiling and it is *invisibility at aerial range* (`lod` at 240 up:
///   7,712 px, MAE 0.0817, below the 0.372 by-eye rung, only the corner right under the
///   camera). The far band was the experiment the round paid for: stride 4 proxies at a
///   quarter of the carpet's keep, because a full-density wall of 4 m cross-quads is
///   exactly the "banner" the original cap feared.
///   **Measured by batch 102's hardware round: 7,753 px (`lod`, was 7,712) and 227,748
///   (80 up, was 227,712) -- 41 and 36 pixels. By the batch-94 rule the band is NOT
///   built.** The mechanism is structural: the gate is a stride cap, so 2 -> 4 buys
///   exactly one LOD level, and the aerial frame is dominated by strides 8/16/32 --
///   aerial visibility wants a proxy form that survives those strides, not another
///   constant. Left in place at 0.25 because it is neutral where seen and its
///   measurement *is* this documentation; the aerial entry stays open on G2's row in
///   `docs/roadmap.md`. Revert options are both one-constant: set this to 0.0 for the
///   far band to vanish, or set [`G2_COARSE_STRIDE_MAX`] back to 2.
pub const G2_COARSE_KEEP_FAR: f32 = 0.25;
/// The furthest stride a tuft proxy may stamp at. Raised from 2 to 4 at the same round
/// [`G2_COARSE_KEEP_FAR`]'s doc transcribes: the coarse paint batch 18 already carries
/// anything coarser, so 4 is where geometry stops and paint resumes by design.
pub const G2_COARSE_STRIDE_MAX: i32 = 4;
/// The wet-bank fringe: the references' lakes run reeds around their shallows. Parts per
/// 10000 of a shoreline column, under the same clump field the tufts take -- straight
/// matching fraction rather than a third density word, because the fringe is only the
/// carpet walking down to the water.
pub const G2_REED_FRACTION: f32 = 0.35;

/// Whether a world generated with these options paints its coarse ground as meadow. Chained
/// through `has_foliage` rather than written as a three-way AND so that there is exactly one
/// statement of what turns ground cover off, and this cannot come apart from it.
///
/// Nothing on the render side reads this: a meadow is an ordinary opaque cube, so unlike
/// `FLAG_FOLIAGE` there is no shader that has to agree with the generator about it.
pub const fn has_meadow(meadow: bool, foliage: bool, biomes: bool) -> bool {
    meadow && has_foliage(foliage, biomes)
}

/// The fraction of a column's ground that carries a tuft at LOD 0.
///
/// **Both placers call this and neither writes it out.** `generate_lod0` reads it as the
/// probability that one column gets a tuft and `coarse_meadow` reads it as the fraction of
/// the `s*s` columns a coarse voxel covers that would have one -- the same number under two
/// descriptions, which is only true while there is one copy of it. Two copies that agreed
/// on the day they were written are what would put the coarse ground at a density the fine
/// ground never had, and the whole of batch 18 is the claim that those two densities match.
///
/// Clamped below 1.0 by the 0.55/0.40 split rather than by a `min`: even the densest biome
/// at the crest of the forest field keeps some bare ground, because a fully closed carpet
/// hides the terrain it grows on.
#[inline]
pub fn ground_cover(forest: f32, grass_scale: f32) -> f32 {
    (0.55 + 0.40 * forest) * grass_scale
}

impl WorldGen {
    pub fn new(seed: i32) -> Self {
        Self::with_options(seed, SEA_LEVEL, true, true, true)
    }

    pub fn with_sea_level(seed: i32, sea_level: i32) -> Self {
        Self::with_options(seed, sea_level, true, true, true)
    }

    pub fn with_options(
        seed: i32,
        sea_level: i32,
        biomes: bool,
        foliage: bool,
        meadow: bool,
    ) -> Self {
        Self {
            seed,
            sea_level,
            biomes,
            foliage: has_foliage(foliage, biomes),
            meadow: has_meadow(meadow, foliage, biomes),
            // On unless a caller clears it. Every existing construction site -- three in
            // `src/` and a few dozen in `tests/` -- therefore keeps the shipping behaviour
            // without being touched, which is why this is a field with a default rather than
            // a sixth positional parameter on a function the test suite calls everywhere.
            bounce_shadow: true,
            // Batch 91 (roadmap A5), `--tree-blue-noise`: decimate the coarse canopy proxies
            // with the R3 rank field below instead of stamping them solid. **Off** means the
            // pre-arm proxies exactly -- one `bool` read per stamped cell that changes no
            // write -- which is the only revert a world-shape arm can offer.
            tree_blue_noise: false,
            grass_dense: false,
            meadow_side: false,
            continental: noise(seed, 0.0008, 3),
            hills: noise(seed + 1, 0.0022, 3),
            detail: noise(seed + 2, 0.020, 2),
            forest: noise(seed + 3, 0.004, 1),
            cave_a: noise(seed + 4, 0.028, 1),
            cave_b: noise(seed + 5, 0.028, 1),
            gravel: noise(seed + 6, 0.05, 1),
        }
    }

    /// The biome field at a column. Two noise lookups, one threshold and one blend.
    pub fn column_biome(&self, x: i32, z: i32) -> ColumnBiome {
        if !self.biomes {
            // The control. Plains carries the pre-batch-9 amplitude, and an unreachable
            // snow and tree line switch off the two altitude rules entirely, so what is
            // left is the old generator exactly.
            return ColumnBiome {
                id: biome::PLAINS,
                mountain_amp: biome::def(biome::PLAINS).mountain_amp,
                snow_line: i32::MAX,
                tree_line: i32::MAX,
            };
        }
        let (fx, fz) = (x as f32, z as f32);
        // Not a `FastNoiseLite` like the six above: the biome field is the one noise the
        // *renderer* also evaluates, so it is spelled out in `biome::simplex2` where the
        // WGSL port has something tested to be a transliteration of. It is bit-identical to
        // the `FastNoiseLite` this used to hold; `tests/biome.rs` is what says so.
        let t = biome::simplex2(self.seed + biome::TEMP_SEED, biome::TEMP_FREQ, fx, fz);
        let m = biome::simplex2(self.seed + biome::HUMID_SEED, biome::HUMID_FREQ, fx, fz);
        let (id, amp, snow, trees) = biome::lookup(t, m);
        ColumnBiome {
            id,
            mountain_amp: amp,
            snow_line: snow.round() as i32,
            tree_line: trees.round() as i32,
        }
    }

    /// The biome id alone, for callers that only want to name a column.
    pub fn biome_at(&self, x: i32, z: i32) -> BiomeId {
        self.column_biome(x, z).id
    }

    /// Terrain height (first air block) at a column, in internal coordinates.
    pub fn height(&self, x: i32, z: i32) -> i32 {
        self.height_with(x, z, self.column_biome(x, z).mountain_amp)
    }

    /// The height field with the mountain amplitude supplied, so a caller that already has
    /// the column's biome does not pay for it twice.
    fn height_with(&self, x: i32, z: i32, mountain_amp: f32) -> i32 {
        let (fx, fz) = (x as f32, z as f32);
        let c = self.continental.get_noise_2d(fx, fz);
        let h = self.hills.get_noise_2d(fx, fz);
        let d = self.detail.get_noise_2d(fx, fz);
        // Amplitudes are kept well under the noise wavelengths so slopes stay walkable
        // instead of turning into spikes: hills vary over ~450 blocks, mountains likewise.
        // `mountain_amp` was the constant 88.0 before batch 9 and is now the blended
        // per-biome term, which is where the world's variety in relief comes from.
        let base = 122.0 + c * 22.0;
        let mask = ((c - 0.05) * 2.4).clamp(0.0, 1.0);
        let peaks = h.max(0.0);
        let mountains = peaks * peaks * mountain_amp * mask;
        let ht = base + h * 15.0 + mountains + d * 2.2;
        (ht.round() as i32).clamp(2, WORLD_HEIGHT - 8)
    }

    /// Block at a column depth, ignoring caves. `h` is the column height.
    #[inline]
    fn column_block(&self, y: i32, h: i32, cb: &ColumnBiome) -> BlockId {
        if y >= h {
            return if y < self.sea_level { WATER } else { AIR };
        }
        if y == 0 {
            return BEDROCK;
        }
        // Sand from the tide line down. The old rule kept a narrow band around sea level
        // and left the basins grassy so the world would not read as one desert; the basins
        // are sea floor now, and a sand floor under water is what a shore actually looks
        // like. Keyed to the constant, not to `sea_level`, so `--no-water` changes only
        // whether the water is there.
        let beach = h <= SEA_LEVEL + 2;
        // Two altitude rules, both read off the column's own height the same way the beach
        // rule reads it -- never off a configured value -- so a coarse chunk resampling the
        // field cannot reach a different answer than LOD 0 about where either one starts.
        //
        // Above the tree line nothing grows, so the ground is the rock it always was; above
        // the snow line that rock is under snow. Below both, the biome's own pair. That is
        // one profile for every biome: ground, then scree, then a white cap. Putting snow in
        // a biome's *palette* instead makes its lowlands white down to the tide line, which
        // reads as one alp from beach to peak rather than as a mountain.
        // `h - 1 >= line` and not clippy's `h > line`: they are the same integers, but `h`
        // is the first *air* voxel and `h - 1` is the top solid block, which is the thing the
        // rule is about. Keeping the subtraction keeps the code reading as the rule.
        #[allow(clippy::int_plus_one)]
        let snowy = h - 1 >= cb.snow_line;
        #[allow(clippy::int_plus_one)]
        let barren = h - 1 >= cb.tree_line;
        if y == h - 1 {
            if beach {
                return SAND;
            }
            if snowy {
                return SNOW;
            }
            return if barren { STONE } else { cb.def().surface };
        }
        if y >= h - 4 {
            if beach {
                return SAND;
            }
            return if snowy || barren {
                STONE
            } else {
                cb.def().subsurface
            };
        }
        STONE
    }

    /// First block of a column that is neither ground nor water.
    ///
    /// Public because the coarse levels are a heightfield and `journal::apply` has to
    /// aggregate edits against the same surface `generate_coarse` filled up to. A second
    /// copy of `h.max(sea_level)` over there would be one `--no-water` could pull apart,
    /// silently, in the direction of clearing coarse voxels that still hold water.
    #[inline]
    pub fn column_top(&self, h: i32) -> i32 {
        h.max(self.sea_level)
    }

    /// The block a column presents to the sky, given the height the caller already has.
    ///
    /// **One call into [`Self::column_block`], which is what makes this the generator's own
    /// rule rather than a second copy of the beach, snow and tree lines.** `column_top(h) - 1`
    /// is the column's top voxel whatever fills it: over land that is `h - 1` and the surface
    /// palette answers, and over sea it is the voxel just under the surface, where
    /// `column_block` returns `WATER` because `y >= h`. So a probe over the ocean gathers
    /// water's albedo and not the sea floor's, with no test here saying so.
    ///
    /// **`h` is a parameter rather than re-derived**, because every caller is walking a
    /// height field it has already sampled and a second evaluation is a second answer waiting
    /// to disagree with the first -- the same argument `column_field` makes for threading
    /// `mountain_amp` through. What this call costs is the biome lookup alone.
    ///
    /// Read by `probe::bake` and nothing else: roadmap R2's bounce needs the albedo of the
    /// ground a probe can see. **It cannot see a tree, and that is the height field's limit
    /// rather than this function's** -- see `probe`'s module note.
    #[inline]
    pub fn surface_block_at(&self, x: i32, z: i32, h: i32) -> BlockId {
        let cb = self.column_biome(x, z);
        self.column_block(self.column_top(h) - 1, h, &cb)
    }

    /// Biome and height for every column of a chunk. `s` is the voxel stride, so LOD 0 and
    /// the coarse levels sample the identical field at the identical points -- which is
    /// what makes the snow line and the tree line agree across a transition rather than
    /// merely agree on average. Returns the highest `column_top` in the chunk.
    fn column_field(
        &self,
        o: IVec3,
        s: i32,
        heights: &mut [i32; 64 * 64],
        cols: &mut [ColumnBiome; 64 * 64],
    ) -> i32 {
        let mut max_top = 0;
        for z in 0..64usize {
            for x in 0..64usize {
                let wx = o.x + x as i32 * s + s / 2;
                let wz = o.z + z as i32 * s + s / 2;
                let cb = self.column_biome(wx, wz);
                let h = self.height_with(wx, wz, cb.mountain_amp);
                let i = x + z * 64;
                heights[i] = h;
                cols[i] = cb;
                max_top = max_top.max(self.column_top(h));
            }
        }
        max_top
    }

    /// Fill `out` with blocks and `heights` with the terrain height of each column.
    pub fn generate(&self, key: ChunkKey, out: &mut [BlockId], heights: &mut [i32; 64 * 64]) {
        debug_assert_eq!(out.len(), VOL);
        out.fill(AIR);
        if key.lod == 0 {
            self.generate_lod0(key, out, heights);
        } else {
            self.generate_coarse(key, out, heights);
        }
    }

    fn generate_lod0(&self, key: ChunkKey, out: &mut [BlockId], heights: &mut [i32; 64 * 64]) {
        let o = key.origin();
        let mut cols = [ColumnBiome::EMPTY; 64 * 64];
        let max_top = self.column_field(o, 1, heights, &mut cols);
        if o.y >= max_top {
            return; // entirely air
        }

        // Cave field sampled every 4 blocks on a 17^3 lattice, interpolated per voxel.
        let mut ca = [0f32; 17 * 17 * 17];
        let mut cb = [0f32; 17 * 17 * 17];
        let mut cg = [0f32; 17 * 17 * 17];
        let carve = o.y + 64 > 4;
        if carve {
            for gz in 0..17 {
                for gy in 0..17 {
                    for gx in 0..17 {
                        let (wx, wy, wz) = (
                            (o.x + gx as i32 * 4) as f32,
                            (o.y + gy as i32 * 4) as f32,
                            (o.z + gz as i32 * 4) as f32,
                        );
                        let i = gx + gy * 17 + gz * 289;
                        ca[i] = self.cave_a.get_noise_3d(wx, wy * 1.6, wz);
                        cb[i] = self.cave_b.get_noise_3d(wx, wy * 1.6, wz);
                        cg[i] = self.gravel.get_noise_3d(wx, wy, wz);
                    }
                }
            }
        }
        let lerp = |g: &[f32], x: usize, y: usize, z: usize| -> f32 {
            let (gx, gy, gz) = (x >> 2, y >> 2, z >> 2);
            let (fx, fy, fz) = (
                (x & 3) as f32 * 0.25,
                (y & 3) as f32 * 0.25,
                (z & 3) as f32 * 0.25,
            );
            let at =
                |dx: usize, dy: usize, dz: usize| g[(gx + dx) + (gy + dy) * 17 + (gz + dz) * 289];
            let x00 = at(0, 0, 0) + (at(1, 0, 0) - at(0, 0, 0)) * fx;
            let x10 = at(0, 1, 0) + (at(1, 1, 0) - at(0, 1, 0)) * fx;
            let x01 = at(0, 0, 1) + (at(1, 0, 1) - at(0, 0, 1)) * fx;
            let x11 = at(0, 1, 1) + (at(1, 1, 1) - at(0, 1, 1)) * fx;
            let y0 = x00 + (x10 - x00) * fy;
            let y1 = x01 + (x11 - x01) * fy;
            y0 + (y1 - y0) * fz
        };

        for z in 0..64usize {
            for x in 0..64usize {
                let h = heights[x + z * 64];
                let cbi = cols[x + z * 64];
                let top = self.column_top(h);
                for y in 0..64usize {
                    let wy = o.y + y as i32;
                    if wy >= top {
                        break;
                    }
                    // Water sits above the ground and below the sea surface. Caves are
                    // carved strictly below `h`, so they never open into the water column.
                    if wy >= h {
                        out[dense_index(x, y, z)] = WATER;
                        continue;
                    }
                    let mut id = self.column_block(wy, h, &cbi);
                    // Never the surface voxel: the coarse levels have no gravel pass, so a
                    // mottled rock band at LOD 0 would disagree with its own proxy. Before
                    // batch 9 this was already true by construction -- the top of a column
                    // was grass or sand, never stone.
                    if carve && id == STONE && wy < h - 1 && lerp(&cg, x, y, z) > 0.42 {
                        id = GRAVEL;
                    }
                    if carve && wy > 3 && wy < h {
                        let a = lerp(&ca, x, y, z);
                        let b = lerp(&cb, x, y, z);
                        // Spaghetti caves where both fields are near zero; wider at depth.
                        let w =
                            0.055 + 0.03 * (1.0 - (wy as f32 / SEA_LEVEL as f32).clamp(0.0, 1.0));
                        if a.abs() < w && b.abs() < w {
                            id = AIR;
                        }
                    }
                    out[dense_index(x, y, z)] = id;
                }
            }
        }

        // Trees: placed by column hash, kept fully inside the chunk.
        for z in 3..61usize {
            for x in 3..61usize {
                let h = heights[x + z * 64];
                let cbi = cols[x + z * 64];
                let b = cbi.def();
                // A treeless biome costs one compare, not a noise lookup.
                if b.tree_scale <= 0.0 || h <= SEA_LEVEL + 2 || h > cbi.tree_line {
                    continue;
                }
                let (wx, wz) = (o.x + x as i32, o.z + z as i32);
                let f = self.forest.get_noise_2d(wx as f32, wz as f32);
                let prob = (0.0015 + 0.012 * f.max(0.0)) * b.tree_scale * 10000.0;
                if hash2(wx, wz, self.seed) % 10000 >= prob as u32 {
                    continue;
                }
                let ly = h - o.y;
                if ly < 1 || ly + 7 >= 64 {
                    continue;
                }
                // **This biome's own surface block**, not `GRASS`. Gating on grass is what
                // would quietly leave every desert and every snow cap bald without a single
                // test failing.
                if out[dense_index(x, ly as usize - 1, z)] != b.surface {
                    continue;
                }
                let ly = ly as usize;
                for dy in 0..5 {
                    out[dense_index(x, ly + dy, z)] = LOG;
                }
                for dy in 3..=6usize {
                    let r: i32 = if dy >= 6 { 1 } else { 2 };
                    for dz in -r..=r {
                        for dx in -r..=r {
                            if dy < 6
                                && dx.abs() == r
                                && dz.abs() == r
                                && !hash2(wx + dx, wz + dz, dy as i32).is_multiple_of(3)
                            {
                                continue;
                            }
                            if dy == 6 && dx.abs() + dz.abs() > 1 {
                                continue;
                            }
                            let i = dense_index(
                                (x as i32 + dx) as usize,
                                ly + dy,
                                (z as i32 + dz) as usize,
                            );
                            if out[i] == AIR {
                                out[i] = b.leaf;
                            }
                        }
                    }
                }
                out[dense_index(x, ly + 6, z)] = b.leaf;
            }
        }

        // Ground cover, batch 14. **After the trees and only into air**, so a trunk always
        // wins its own column and the grass fills in around it. Running it first would put a
        // tuft under every trunk, where nothing can see it and it still costs a voxel.
        //
        // Unlike the tree placer this covers the full 0..64 span rather than 3..61: a tuft is
        // one voxel and cannot overhang a chunk edge the way a 5-wide canopy can, and leaving
        // a 3-voxel bald margin around every chunk would draw the chunk grid on the ground.
        if self.foliage {
            for z in 0..64usize {
                for x in 0..64usize {
                    let h = heights[x + z * 64];
                    let cbi = cols[x + z * 64];
                    let b = cbi.def();
                    // A bare biome costs one compare, not a noise lookup -- the same shape
                    // the tree placer's `tree_scale` gate has.
                    if b.grass_scale <= 0.0 || h <= SEA_LEVEL + 1 || h > cbi.tree_line {
                        continue;
                    }
                    let ly = h - o.y;
                    if !(1..64).contains(&ly) {
                        continue;
                    }
                    let ly = ly as usize;
                    // The column's own surface block, never `GRASS`. Above the snow line the
                    // surface is SNOW over STONE and below the tide line it is SAND, and in
                    // both cases the biome's ground is not there -- so neither is its grass.
                    if out[dense_index(x, ly - 1, z)] != b.surface {
                        continue;
                    }
                    if out[dense_index(x, ly, z)] != AIR {
                        continue;
                    }
                    let (wx, wz) = (o.x + x as i32, o.z + z as i32);
                    // Modulated by the same forest noise the trees use, so cover thickens and
                    // thins with the same field the canopy does instead of being a uniform
                    // wash at one density. Clamped below 1.0 so even a dense biome keeps some
                    // bare ground: a fully closed carpet hides the terrain it grows on.
                    let f = self.forest.get_noise_2d(wx as f32, wz as f32);
                    let mut prob = ground_cover(f, b.grass_scale);
                    if self.grass_dense {
                        // Batch 101e: same clump field, thicker carpet. The ceiling at
                        // 0.95 is batch 14's own argument made again: a fully closed
                        // carpet hides the terrain it grows on, and the references have
                        // no such carpet either -- their stands are dense, not solid.
                        prob = (prob * G2_GRASS_DENSITY).min(0.95);
                    }
                    if (hash2(wx, wz, self.seed ^ 0x6720) % 10000) as f32 >= prob * 10000.0 {
                        continue;
                    }
                    // Batch 101e: the references' stands have a *profile* -- a green
                    // floor with dry tussocks through it. A share of the armed tufts
                    // grows the stacked form, so the tall stands live wherever the
                    // carpet does, never as a field of their own. The above-cell rule is
                    // the trunk's own: a tussock only ever stacks into air.
                    if self.grass_dense
                        && (hash2(wx, wz, self.seed ^ 0xa455) % 100) < G2_TALL_SHARE
                        && ly + 1 < 64
                        && out[dense_index(x, ly + 1, z)] == AIR
                    {
                        out[dense_index(x, ly, z)] = GRASS_TALL;
                        out[dense_index(x, ly + 1, z)] = GRASS_TALL;
                        continue;
                    }
                    out[dense_index(x, ly, z)] = TALL_GRASS;
                }
            }
        }

        // Batch 101e (`--grass-dense`): the wet-bank fringe. Reeds stand where the
        // references always stand them -- in the row of columns between open water and
        // its sand -- and only into air, cell by cell, so a stalk pokes through the
        // shallowest water rather than straddling it. Gated hard on the arm: with the
        // arm off, none of this stream executes and the unarmed banks stay bare to the
        // bit, which is the same claim A5's canopy arm keeps for its proxies.
        //
        // The 4-neighbour is read inside this chunk only: at chunk edges the fringe
        // understates itself near open water on the far side. That reads as a missing
        // stalk, which no vantage can separate from a dry bank -- an understatement,
        // never an artifact, and the last argument for not giving reeds a cross-chunk
        // query no other cover has.
        if self.foliage && self.grass_dense {
            for z in 0..64usize {
                for x in 0..64usize {
                    let h = heights[x + z * 64];
                    // The tide band: the rows where the surface is the beach's sand or
                    // the grass that meets it. Deeper columns stand in open water here
                    // and reeds wouldn't be seen; higher ones are the dry bank.
                    if !(SEA_LEVEL..=SEA_LEVEL + 2).contains(&h) {
                        continue;
                    }
                    let cbi = cols[x + z * 64];
                    let b = cbi.def();
                    if b.grass_scale <= 0.0 {
                        continue;
                    }
                    // Open water beside the column, in-chunk: the fringe exists because
                    // the lake does.
                    let open = (x > 0 && heights[x - 1 + z * 64] < SEA_LEVEL)
                        || (x < 63 && heights[x + 1 + z * 64] < SEA_LEVEL)
                        || (z > 0 && heights[x + (z - 1) * 64] < SEA_LEVEL)
                        || (z < 63 && heights[x + (z + 1) * 64] < SEA_LEVEL)
                        || h < SEA_LEVEL;
                    if !open {
                        continue;
                    }
                    let ly = h - o.y;
                    if !(1..62).contains(&ly) {
                        continue;
                    }
                    let ly = ly as usize;
                    let base = out[dense_index(x, ly - 1, z)];
                    if base != SAND && base != b.surface {
                        continue;
                    }
                    if out[dense_index(x, ly, z)] != AIR {
                        continue;
                    }
                    // The carpet's own clump field, walked down to the tide line -- so
                    // the fringe is grass at the shore and not a ring drawn around every
                    // pond at one flat rate.
                    let (wx, wz) = (o.x + x as i32, o.z + z as i32);
                    let f = self.forest.get_noise_2d(wx as f32, wz as f32);
                    let prob = ground_cover(f, b.grass_scale) * G2_REED_FRACTION;
                    if (hash2(wx, wz, self.seed ^ 0x5eed) % 10000) as f32 >= prob * 10000.0 {
                        continue;
                    }
                    // Height paid from the same wallet the tall share pays for: a stem of
                    // two or three cells, each into air only.
                    let stem = if (hash2(wx, wz, self.seed ^ 0x7eed) % 100) < 45 { 3 } else { 2 };
                    for dy in 0..stem {
                        let i = dense_index(x, ly + dy, z);
                        if out[i] != AIR {
                            break;
                        }
                        out[i] = REEDS;
                    }
                }
            }
        }
    }

    fn generate_coarse(&self, key: ChunkKey, out: &mut [BlockId], heights: &mut [i32; 64 * 64]) {
        let o = key.origin();
        let s = key.voxel_size();
        let mut cols = [ColumnBiome::EMPTY; 64 * 64];
        self.column_field(o, s, heights, &mut cols);
        for z in 0..64usize {
            for x in 0..64usize {
                let h = heights[x + z * 64];
                let cbi = cols[x + z * 64];
                let ceiling = self.column_top(h);
                for y in 0..64usize {
                    let wy = o.y + y as i32 * s;
                    if wy >= ceiling {
                        break;
                    }
                    // Represent the voxel by its topmost covered block, water included: a
                    // coarse voxel straddling a shoreline is mostly water and reads as
                    // water, which is the same rule that already picks grass over the dirt
                    // beneath it.
                    let top = (wy + s - 1).min(ceiling - 1);
                    out[dense_index(x, y, z)] = self.column_block(top, h, &cbi);
                }
            }
        }
        // Batch 96 (A5's own tests, third round): the rank field's *holes* are AIR to the
        // marcher but must not be AIR to `coarse_meadow`, whose "nothing over this column"
        // gate was authored against solid proxies -- a thinned canopy cell overhead would
        // otherwise admit a meadow stamp under a tree (`grass -> MEADOW` at exactly the
        // columns the arm thinned, which is what `a5_flag_off...` caught at its line 716:
        // two consumers of the same cell, and the arm had secretly re-contracted the
        // second). `thinned` records every rejected proxy cell; when the arm is off the
        // rank never rejects, the bitmap stays zero, and the unarmed world is bit-identical
        // by construction -- the one property that makes adding a side bitmap a repair
        // rather than a change.
        let mut thinned = [0u64; 4 * 1024];
        self.coarse_trees(key, out, heights, &cols, &mut thinned);
        self.coarse_meadow(key, out, heights, &cols, &thinned);
        // Batch 101e: the coarse *geometry* of the grass the meadow only paints. Runs
        // last so the tufts stamp onto ground whose fate (meadow, canopy, bare) is
        // already decided -- the one order in which nobody can erase anybody.
        self.coarse_tufts(key, out, heights, &cols, &thinned);
    }

    /// Batch 18's compensation for ground cover ending at the LOD 0 boundary.
    ///
    /// A tuft is one voxel and cannot exist at stride 2, so foliage is LOD 0 only and the
    /// dense carpet batch 14 ships stops in a chunk-aligned line that is plainly visible
    /// from height. This repaints the coarse surface toward the colour of the carpet that
    /// is missing, so the transition reads as a change in shade rather than a change in
    /// geometry. It is an albedo and nothing else: `MEADOW` is an opaque cube, no shader
    /// knows it exists, and `resolve` is untouched -- which is batch 14's own lesson, that
    /// anything added to that pass is billed for existing whether it runs or not.
    ///
    /// **Stamped by probability, not by a threshold on cover.** `p = ground_cover(..)` is
    /// exactly the LOD 0 tuft rate, so the *mean* albedo over any patch matches the fine
    /// ground's automatically and per biome -- taiga at `grass_scale` 0.45 gets its lighter
    /// carpet for free, with no second constant and no per-biome table. It is the bargain
    /// `coarse_trees` already strikes one function above, for the same reason: an LOD is
    /// allowed to disagree about *which* cells, never about how many.
    ///
    /// A threshold on cover was the alternative and is worse in the one place that matters.
    /// It would put a hard albedo contour in the middle of an open plain where the forest
    /// field crosses it -- a *new* edge, traded for the one being removed -- and it would
    /// give a biome below the threshold no compensation at all. The stamp's own noise is at
    /// the coarse voxel's scale, which at the LOD 0 boundary is stride 2: two blocks, next
    /// to the one-block tufts it stands for. At stride 8 it is eight, and eight blocks at
    /// the range stride 8 is drawn at is already most of the way into the haze.
    ///
    /// Runs **after** `coarse_trees` and skips a column with anything standing on it, which
    /// is both halves of one rule: the proxy placer tests for `b.surface` and would stop
    /// seeing a repainted column, and LOD 0 puts no tuft under a trunk either.
    fn coarse_meadow(
        &self,
        key: ChunkKey,
        out: &mut [BlockId],
        heights: &[i32; 64 * 64],
        cols: &[ColumnBiome; 64 * 64],
        thinned: &[u64; 4 * 1024],
    ) {
        if !self.meadow {
            return;
        }
        let s = key.voxel_size();
        let o = key.origin();
        for z in 0..64usize {
            for x in 0..64usize {
                let h = heights[x + z * 64];
                let cbi = cols[x + z * 64];
                let b = cbi.def();
                // The same three gates LOD 0's placer opens with, read off the same field:
                // a bare biome, the tide line, and the tree line above which nothing grows.
                if b.grass_scale <= 0.0 || h <= SEA_LEVEL + 1 || h > cbi.tree_line {
                    continue;
                }
                let ly = (h - 1 - o.y).div_euclid(s);
                if !(0..64).contains(&ly) {
                    continue;
                }
                let ly = ly as usize;
                // The column's own surface block, never `GRASS` -- batch 9's rule, and the
                // one that keeps a snow cap and a beach out of this without naming either.
                if out[dense_index(x, ly, z)] != b.surface {
                    continue;
                }
                if ly + 1 < 64 {
                    let above = dense_index(x, ly + 1, z);
                    // A thinned proxy still counts as canopy for this gate: batch 96,
                    // see the driver's comment. Unarmed, `thinned` is all zeros and the
                    // term is free of effect.
                    if out[above] != AIR || (thinned[above >> 6] >> (above & 63)) & 1 == 1 {
                        continue;
                    }
                }
                // The coarse voxel's centre, the same sample point `coarse_trees` uses, so
                // the two agree about where a forest is.
                let (wx, wz) = (o.x + x as i32 * s + s / 2, o.z + z as i32 * s + s / 2);
                let f = self.forest.get_noise_2d(wx as f32, wz as f32);
                // The *apparent* cover, which is what a coarse voxel is standing in for --
                // see `MEADOW_SPREAD`. Saturating rather than clamping the noise: a biome
                // dense enough to close its carpet gets a solid repaint, which is also what
                // keeps the stamp from speckling where it matters most.
                let cover = (ground_cover(f, b.grass_scale) * MEADOW_SPREAD).min(1.0);
                // A distinct salt from the tuft placer's `0x6720`: the two decisions are
                // about the same ground and must not be the same coin.
                if (hash2(wx, wz, self.seed ^ 0x4d13) % 10000) as f32 >= cover * 10000.0 {
                    continue;
                }
                // Batch 92's arm is exactly this id and nothing else -- the gates, the
                // cover curve and the coin are the placer's either way, so an armed world
                // differs from the legacy one in *which cube* lands, never in *whether one
                // does*, which is what keeps the legacy hash control meaningful.
                out[dense_index(x, ly, z)] = if self.meadow_side {
                    MEADOW_SIDE
                } else {
                    MEADOW
                };
            }
        }
    }

    /// Proxy trees for a coarse chunk.
    ///
    /// A tree is 7 blocks tall and 5 wide, so at stride 2 or more it cannot be generated
    /// the way `generate_lod0` does it — and sampling the placement hash on the coarse
    /// lattice would miss all but 1 column in `s*s`, which is why coarse terrain used to
    /// come out bald. Instead each coarse voxel column asks whether *any* of the `s*s`
    /// fine columns it covers would hold a tree (`p * s*s`, the same forest noise driving
    /// the density) and stamps one scaled-down proxy if so. Positions inside a forest do
    /// not match LOD 0 tree for tree, but the forests land in the same places and the
    /// hill keeps its bumpy, green silhouette across the transition.
    /// Batch 101e (`--grass-dense`): ground-cover proxies for the coarse shell, so the
    /// references' grass keeps its *shape* past the LOD 0 fence that paints and clear
    /// air cannot fake. Batch 18 covered the same gap in colour by repainting the ground
    /// as meadow; A5 covered it in canopy holes with thinned proxies. This is both
    /// constructions at once and neither's surprise: the gate cell-for-cell is
    /// `coarse_meadow`'s (own surface block, nothing overhead, a thinned canopy still
    /// overhead), and the *placement* field is A5's rank field, so proxies thin by
    /// habitat rather than pepper all biomes at one rate.
    ///
    /// Capped at stride 2: a cross-quad standing for four metres of grass reads as a
    /// banner, not as cover -- coarser than that the meadow paint is the whole story,
    /// the rule batch 18 already shipped. Gated on the arm alone; unarmed this fn is an
    /// early return and the rough path keeps its bit.
    fn coarse_tufts(
        &self,
        key: ChunkKey,
        out: &mut [BlockId],
        heights: &[i32; 64 * 64],
        cols: &[ColumnBiome; 64 * 64],
        thinned: &[u64; 4 * 1024],
    ) {
        if !self.grass_dense {
            return;
        }
        let s = key.voxel_size();
        if s > G2_COARSE_STRIDE_MAX {
            return;
        }
        let o = key.origin();
        for z in 0..64usize {
            for x in 0..64usize {
                let h = heights[x + z * 64];
                let cbi = cols[x + z * 64];
                let b = cbi.def();
                // `coarse_meadow`'s own three gates, in its own order: bare biome, tide,
                // tree line.
                if b.grass_scale <= 0.0 || h <= SEA_LEVEL + 1 || h > cbi.tree_line {
                    continue;
                }
                // The first *air* slab over the surface -- where a LOD 0 tuft would sit.
                let ly = (h - o.y).div_euclid(s);
                if !(1..63).contains(&ly) {
                    continue;
                }
                let ly = ly as usize;
                // Own surface block only -- the snow cap and the beach stay out without
                // being named, which is batch 9's rule brought up the chain.
                if out[dense_index(x, ly - 1, z)] != b.surface {
                    continue;
                }
                let i = dense_index(x, ly, z);
                if out[i] != AIR {
                    continue;
                }
                let above = dense_index(x, ly + 1, z);
                if out[above] != AIR || (thinned[above >> 6] >> (above & 63)) & 1 == 1 {
                    continue;
                }
                // A5's placement field rather than the tuft placer's coin: proxies are
                // presentation voxels seen at range, and pepper-at-distance is exactly
                // the failure that field was built to forbid. The threshold is the
                // carpet's own cover of the same forest noise, ranked rather than
                // hashed, at `G2_COARSE_KEEP` of ceiling -- and far thinner past stride 2:
                // aerial reads want sparse stable pattern, not a quad wall (the
                // batch-101-hardware finding, `G2_COARSE_KEEP_FAR`'s doc).
                let (wx, wz) = (o.x + x as i32 * s + s / 2, o.z + z as i32 * s + s / 2);
                let f = self.forest.get_noise_2d(wx as f32, wz as f32);
                let keep = if s > 2 { G2_COARSE_KEEP_FAR } else { G2_COARSE_KEEP };
                let ceiling = (ground_cover(f, b.grass_scale) * keep).min(1.0);
                let wy = o.y + ly as i32 * s;
                if canopy_rank3(wx, wy, wz, self.seed) >= ceiling {
                    continue;
                }
                out[i] = TALL_GRASS;
            }
        }
    }

    fn coarse_trees(
        &self,
        key: ChunkKey,
        out: &mut [BlockId],
        heights: &[i32; 64 * 64],
        cols: &[ColumnBiome; 64 * 64],
        thinned: &mut [u64; 4 * 1024],
    ) {
        let s = key.voxel_size();
        // Beyond stride 8 a proxy would be taller than the tree it stands for, and at
        // those ranges (past 1024 blocks) a canopy is well under a pixel anyway.
        if s > 8 {
            return;
        }
        let o = key.origin();
        let trunk = (3 / s) as usize;
        let canopy = ((4 / s).max(1)) as usize;
        let r = 2 / s;
        let lo = r as usize;
        let hi = 64 - r as usize;
        // Match LOD 0's canopy *area*, not its tree count. One coarse voxel already
        // shadows `s*s` columns, so past stride 4 stamping a proxy wherever a tree could
        // stand would carpet the world in leaves; scaling by the proxy's own footprint
        // keeps the green fraction of the ground the same at every level. The canopy shape
        // is shared across biomes precisely so this stays one constant.
        let footprint = ((2 * r + 1) * (2 * r + 1)) as f32;
        let density = LOD0_CANOPY_COLUMNS / footprint;
        for z in lo..hi {
            for x in lo..hi {
                let h = heights[x + z * 64];
                let cbi = cols[x + z * 64];
                let b = cbi.def();
                // The same three gates LOD 0 uses, read off the same field.
                if b.tree_scale <= 0.0 || h <= SEA_LEVEL + 2 || h > cbi.tree_line {
                    continue;
                }
                let (wx, wz) = (o.x + x as i32 * s + s / 2, o.z + z as i32 * s + s / 2);
                let f = self.forest.get_noise_2d(wx as f32, wz as f32);
                let p =
                    ((0.0015 + 0.012 * f.max(0.0)) * b.tree_scale * density).min(1.0) * 1_000_000.0;
                if hash2(wx, wz, self.seed) % 1_000_000 >= p as u32 {
                    continue;
                }
                // Index of the topmost voxel this column actually filled.
                let ly = (h - 1 - o.y).div_euclid(s);
                if ly < 0 || ly as usize + trunk + canopy >= 64 {
                    continue;
                }
                let ly = ly as usize;
                if out[dense_index(x, ly, z)] != b.surface {
                    continue;
                }
                for dy in 1..=trunk {
                    out[dense_index(x, ly + dy, z)] = LOG;
                }
                for dy in trunk + 1..=trunk + canopy {
                    for dz in -r..=r {
                        for dx in -r..=r {
                            let i = dense_index(
                                (x as i32 + dx) as usize,
                                ly + dy,
                                (z as i32 + dz) as usize,
                            );
                            if out[i] == AIR {
                                // Batch 91 (roadmap A5), `--tree-blue-noise`. A solid proxy
                                // is what makes the cross-fade *partition* rays: porous
                                // cutout canopy meets opaque box and no sample can split
                                // between them. The rank field thins the proxy to LOD 0's
                                // own transmittance (`COARSE_CANOPY_KEEP`, one line of
                                // arithmetic above `hash2`) while the mask stays strictly
                                // binary -- the research's priced -40%, against the float
                                // scalar the same entry priced at +180% to +250%. Trunks
                                // are not thinned, because LOD 0's are not porous either.
                                let keeps = !self.tree_blue_noise
                                    || canopy_rank3(
                                        o.x + (x as i32 + dx) * s,
                                        o.y + (ly + dy) as i32 * s,
                                        o.z + (z as i32 + dz) * s,
                                        self.seed,
                                    ) < COARSE_CANOPY_KEEP;
                                if keeps {
                                    out[i] = b.leaf;
                                } else {
                                    // A hole in the *representation*; the canopy is still
                                    // here for every other reader of this cell -- most
                                    // pressingly `coarse_meadow`'s overhead gate, which
                                    // was authored against solid proxies and must not see
                                    // the carpet's stand-in fire under a tree.
                                    thinned[i >> 6] |= 1u64 << (i & 63);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}



