// Shared declarations for the compute passes: bindings, mask helpers, chunk marching.

struct Frame {
    cam_pos: vec3<f32>,
    time: f32,
    cam_fwd: vec3<f32>,
    tan_half_fov: f32,
    cam_right: vec3<f32>,
    aspect: f32,
    cam_up: vec3<f32>,
    far: f32,
    sun_dir: vec3<f32>,
    daylight: f32,
    res: vec2<u32>,
    tiles: vec2<u32>,
    chunk_count: u32,
    camera_chunk: u32,
    flags: u32,
    max_pairs: u32,
    // Atmosphere. Density falls off exponentially with altitude:
    // `rho(y) = fog_density * exp(-fog_falloff * (y - fog_height))`, so `fog_density` is an
    // extinction coefficient per block *at* `fog_height` and `fog_falloff` is a reciprocal
    // scale height. `fog_height` is **internal** world Y (0..512), not the displayed Y the
    // HUD prints. `fog_scatter` and `fog_g` drive the shared forward-scattering lobe.
    fog_density: f32,
    fog_falloff: f32,
    shadow_dist: f32,
    ambient: f32,
    fog_scatter: f32,
    fog_g: f32,
    fog_height: f32,
    // Added to the mip level `resolve` picks. The footprint divides by `res.y`, so a lower
    // internal resolution would otherwise pick blurrier mips and leave the temporal
    // reconstructor nothing to work with; the renderer sets this to
    // log2(render height / window height), plus whatever --mip-bias asks for.
    mip_bias: f32,
    grid_min: vec3<i32>,
    // Internal world Y of the water surface. The underwater path needs the *plane* and not
    // the voxels: from inside the medium the marcher cannot find the boundary, because a
    // water-to-air transition is not an occupancy edge and the tree stores nothing else.
    sea_level: f32,
    grid_dim: vec3<u32>,
    // Multiplier on water's per-channel extinction. 0 is perfectly clear water and is the
    // A/B partner for every claim about depth colour.
    water_absorb: f32,
    // Sub-pixel offset added to every primary ray, in pixels. (0,0) reproduces the
    // unjittered grid bit for bit; --jitter pins it, otherwise it walks the Halton cycle.
    jitter: vec2<f32>,
    // How much of the new frame the temporal pass keeps. 1.0 is "history is worthless",
    // which is what the first frame and a resize get.
    taa_feedback: f32,
    // Frames since start, wrapped. Only the LOD dither reads it, and only to walk its
    // pattern to a fresh phase every frame so the temporal pass has something to average.
    frame_index: u32,
    // The cloud deck. `cloud_cover` is a *threshold* on a bell-shaped noise and not the sky
    // fraction it produces -- PERF.md carries the measured curve between the two -- but 0 is
    // exactly no cloud, which is what makes it the control: `sky_color` returns `sky_base`
    // untouched and not a term that happens to evaluate to zero.
    cloud_cover: f32,
    // **Internal** world Y of the deck, like `sea_level` and `fog_height` and unlike the Y
    // the HUD prints.
    cloud_height: f32,
    // Blocks per unit of the base octave, so the finest of the four is `cloud_scale / 8`.
    cloud_scale: f32,
    // Blocks per second of drift along `CLOUD_WIND`. It multiplies `frame.time`, which every
    // headless mode pins to zero -- so the deck moves in the game and a capture stays the
    // deterministic single image the whole A/B method is built on.
    cloud_speed: f32,
    // Crepuscular rays. `godray_strength` is how far the shadowing of the haze's own
    // scattering lobe is taken; 0 is exactly no shadowing, which is what makes it the
    // control -- `sun_shaft` returns 1.0 through an early exit and not a mean that happens
    // to come out as one. `godray_steps` and `godray_dist` are the two bounds on the work --
    // samples along the path, and the radius of the ball around the eye the shaft is allowed
    // to know about -- and they are runtime parameters rather than constants precisely
    // because this is the feature whose cost is not bounded by construction.
    godray_strength: f32,
    godray_steps: u32,
    godray_dist: f32,
    // How much of the sun the cloud deck's opacity takes away, used by the shaft *and* by
    // `shade_hit`'s sun term. 0 is its own control and is separately bit-exact.
    cloud_shadow: f32,
    // The per-biome vegetation tint. `tint_strength` is how far the biome's own multiplier
    // is taken and is *not* the control -- `FLAG_TINT` is, because zero strength still
    // samples the field. `tint_seed` is the world seed `WorldGen` was built with, and the
    // two frequencies are `biome::TEMP_FREQ` and `biome::HUMID_FREQ`: they travel rather
    // than sitting here as literals precisely so they cannot drift from the Rust field the
    // terrain was actually generated from.
    tint_strength: f32,
    tint_seed: i32,
    tint_freq_t: f32,
    tint_freq_h: f32,
    // The water micro-normals (batch 16). `wave_amp` is the peak slope of the four summed
    // octaves and 0 is exactly flat, but the *control* is `FLAG_WAVES` and not this: batch
    // 14's finding is that code compiled into `resolve` is billed whether it runs or not,
    // so an amplitude of zero would still cost what the field costs to exist. The two are
    // set together by `flags_from`.
    wave_amp: f32,
    // Blocks per period of the base octave, so the finest of the four is `wave_scale /
    // 10.2`. Clamped to >= 1 on the Rust side because the shader divides by it.
    wave_scale: f32,
    // Periods per second the phases advance, against `frame.time` -- which every headless
    // mode pins to zero, exactly as `cloud_speed` relies on. So the sea moves in the game
    // and a capture stays the deterministic single image the A/B method needs.
    wave_speed: f32,
    // The slope cap on the *traced* reflection's normal, as a tangent. Batch 8 chose a
    // planar surface to keep the secondary rays coherent across a workgroup, and that
    // argument survives batch 16 for exactly one of the four terms that read a normal:
    // Fresnel, the analytic glint and the sky lookup take the full perturbed normal
    // because none of them touches the tree, and the refraction ray keeps the flat one
    // outright. Only the reflection both traverses and wobbles, and this is how far.
    wave_reflect_slope: f32,
    // The light envelope (batch 35). `shaft_origin` is the world XZ of texel (0, 0) of the
    // height window and `shaft_texel` its stride in blocks, so `(p.xz - origin) / texel` is
    // the texel coordinate of a world point. They travel rather than being derived here
    // because the window is anchored on the camera the *field* was last filled at, which is
    // not necessarily this frame's -- it only re-anchors when the camera crosses a texel.
    // `shaft_soft` is how far the shadow edge is ramped in blocks of altitude; see
    // `shaft::SOFT` for why one number does both the penumbra and the cell grid.
    shaft_origin: vec2<f32>,
    shaft_texel: f32,
    shaft_soft: f32,
    // Batch 95 (roadmap G1, the look goal's first face) takes the sixth four-word append --
    // the Rust mirror's history block is where the rule is kept. All four are 0 by default
    // and each is *guarded at its use site*, never a multiply-by-zero: batch 14 priced code
    // compiled into the pass whether it runs or not.
    // Cumulus arrive in banks, not a uniform scatter: 0 is the pre-95 deck, bit-exact.
    // Strength of the sun-side shading that fakes verticality on a flat deck: 0 is flat.
    // The warm band over the horizon treeline, and the richer blue upstairs. Both
    // strength-of-effect, both 0 = the pre-95 sky.
    cloud_patch: f32,
    cloud_relief: f32,
    haze_warm: f32,
    zenith_deep: f32,
    // Last frame's world -> clip matrix, **jitter-free**: a reprojection wants the pixel
    // grid, not this frame's sample position inside it. `taa` subtracts the current
    // jitter from the reprojected position for the same reason.
    prev_view_proj: mat4x4<f32>,
};

struct Chunk {
    aabb_min: vec3<f32>,
    voxel_size: f32,
    aabb_max: vec3<f32>,
    lod: u32,
    origin: vec3<f32>,
    // Signed dither threshold for the stochastic LOD cross-fade; see `ray_skips`.
    fade: f32,
    root: vec4<u32>,
    attr_base: u32,
    attr_flags: u32,
    light_base: u32,
    light_flags: u32,
    // Batch 53: `root`'s occupancy mask with every 16^3 cell that holds nothing but water
    // taken out. The mask a ray that ignores water should be traversing, since water is *in*
    // the occupancy tree and so an ocean interior reads as solid at every level to a ray that
    // cannot hit any of it. Traversal only -- `mask_below` still indexes off `root`'s mask.
    dry_mask: vec2<u32>,
    reserved: vec2<u32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> chunks: array<Chunk>;
@group(0) @binding(2) var<storage, read> inners: array<vec4<u32>>;
@group(0) @binding(3) var<storage, read> leaves: array<vec2<u32>>;
@group(0) @binding(4) var<storage, read> bricks: array<u32>;
@group(0) @binding(5) var<storage, read> block_faces: array<u32>;
// A `block_faces` word is the atlas layer with its batch-36 permutation class packed above it,
// and **nothing may index the table without going through one of these two**. Mirrored from
// `block::FACE_LAYER_BITS`; `faces_per_block_matches_the_shader` holds every call site to the
// stride of eight and `face_layer_at_every_call_site` holds every one of them to these.
const FACE_LAYER_BITS: u32 = 16u;
fn face_layer(word: u32) -> u32 { return word & 0xffffu; }
fn face_perm(word: u32) -> u32 { return word >> FACE_LAYER_BITS; }
// `block::perm`. Three states, and the fourth spare value of the field is deliberate.
const PERM_NONE: u32 = 0u;
const PERM_FLIP_U: u32 = 1u;
const PERM_D4: u32 = 2u;
@group(0) @binding(6) var<storage, read_write> pairs: array<u32>;
@group(0) @binding(7) var<storage, read_write> counters: array<atomic<u32>>;
@group(0) @binding(8) var<storage, read_write> indirect: array<u32>;
@group(0) @binding(9) var<storage, read_write> vis: array<atomic<u64>>;
@group(0) @binding(10) var<storage, read_write> hiz: array<u32>;
@group(0) @binding(11) var out_tex: texture_storage_2d<rgba16float, write>;
@group(0) @binding(12) var atlas: texture_2d_array<f32>;
@group(0) @binding(13) var atlas_samp: sampler;
@group(0) @binding(14) var<storage, read_write> dbg: array<atomic<u32>>;
@group(0) @binding(15) var<storage, read> grid: array<u32>;
// The three temporal bindings. `out_tex` is the pass's own output in every pass, so the
// temporal pass gets its own bind group where binding 11 is the history buffer it writes
// and `taa_src` is the colour `resolve` wrote: one texture can never be a write storage
// binding and a sampled binding in the same bind group.
@group(0) @binding(16) var taa_hist: texture_2d<f32>;
@group(0) @binding(17) var taa_src: texture_2d<f32>;
@group(0) @binding(18) var linear_samp: sampler;
// The light envelope's three slices, in one buffer: slice 0 and slice 1 are what the scan
// ping-pongs between and slice 2 holds the static terrain heights the CPU fills. One binding
// because the scan reads a slice and writes a slice, and a single storage array makes that an
// offset rather than a second binding whose address space would have to differ. `resolve`
// only ever reads it.
@group(0) @binding(19) var<storage, read_write> shaft: array<f32>;

// Batch 55's probe field, and it holds nothing. It exists to price the *read side* of roadmap
// L1 before any of L1 is designed: a directional irradiance field would be sampled here, once
// per shaded surface, with one hardware trilinear tap -- and the open question that decides
// L1's whole shape is whether `resolve` can afford that tap at all. The pass is ALU-bound at 96
// registers and the nine-cell gather inside it is instruction-bound (batch 47), so the honest
// prior was that anything added here costs.
//
// **The contents are a constant 1.0, so the picture cannot move**: `shade_hit` multiplies `amb`
// by the tap and `x * 1.0` is bit-exact for every finite `x`. The whole finding is a
// millisecond. `--probe-fill F` fills it with F instead, which is the paired positive claim --
// bit-exact alone cannot be told from not-wired-up.
@group(0) @binding(20) var probe_tex: texture_3d<f32>;

// Batch 57's sampler for it, and it is its own rather than `linear_samp` because the field's
// three axes do not agree. X and Z are toroidal -- the lattice repeats every
// `PROBE_DIM_XZ * PROBE_SPACING` blocks -- so a tap at the seam has to *filter across* it, and
// `linear_samp` clamps, which would put a four-block band of wrong shading on one plane
// every 512 blocks. Y is not toroidal: `WORLD_HEIGHT / PROBE_SPACING` is exactly the field's
// height, so clamping there is the world's own top and bottom. Repeat, clamp, repeat.
@group(0) @binding(21) var probe_samp: sampler;

// Batch 81 (roadmap P12)'s half-resolution water buffers. Two `Rgba16Float` 2D targets at
// half the trace dimensions in each axis: `water_refr_tex` carries `(under, column_depth)`
// per half-texel and `water_refl_tex` carries `(lit, validity)` -- the exact pair
// `water_refract_leg` and `water_reflect_leg` return, sampled by `resolve`'s shared arm.
// See `water_sec` in resolve.wgsl for the pass and the diagnostic status of both.
// Batch 81 (roadmap P12)'s half-resolution water buffers, in the ONLY arrangement the
// validator permits. The discovered rule (batch 81b): `STORAGE_WRITE_ONLY` conflicts
// with `RESOURCE` for one texture within the usage scope of a *dispatch*, and the scope
// is every resource in every bind group attached -- what the shader actually reads is
// irrelevant. Two roles of one texture may therefore never share a *group*, so the roles
// share nothing: the sampled pair is group 1 (resolve's guests) and the storage pair is
// group 2 (water_sec's own), and each pass attaches exactly one of them.
@group(1) @binding(0) var water_refr_sample: texture_2d<f32>;
@group(1) @binding(1) var water_refl_sample: texture_2d<f32>;
@group(2) @binding(0) var water_refr_tex: texture_storage_2d<rgba16float, write>;
@group(2) @binding(1) var water_refl_tex: texture_storage_2d<rgba16float, write>;

const FULL_BIT: u32 = 0x80000000u;
const PTR_MASK: u32 = 0x7FFFFFFFu;
const UNIFORM_BIT: u32 = 0x80000000u;
const NO_CHUNK: u32 = 0xFFFFFFFFu;
const FLAG_HIZ: u32 = 1u;
const FLAG_HEATMAP: u32 = 2u;
const FLAG_SHADOWS: u32 = 4u;
const FLAG_AO: u32 = 8u;
const FLAG_WATER_REFLECT: u32 = 16u;
const FLAG_WATER_REFRACT: u32 = 32u;
const FLAG_UNDERWATER: u32 = 64u;
const FLAG_TINT: u32 = 128u;
// 256 is `FLAG_FOLIAGE`, which is deliberately absent: no shader tests it, because
// `SPEC_FOLIAGE` is not ANDed with a run-time flag. See that override.
const FLAG_WAVES: u32 = 512u;
const FLAG_WAVE_ANISO: u32 = 1024u;
const FLAG_WAVE_SHOAL: u32 = 2048u;
const FLAG_WAVE_FILL: u32 = 4096u;
const FLAG_TERRAIN_SHAFTS: u32 = 8192u;
const FLAG_TEX_VARIATION: u32 = 16384u;
const FLAG_DISTANT_SHADOWS: u32 = 32768u;
// 65536 is `FLAG_LEAF_CUTOUT`, 131072 `FLAG_GLASS`, 262144 `FLAG_WATER_SHADOW_CUT` and
// 524288 `FLAG_LEAF_THIN` -- none of which any shader tests, the first two because their
// overrides are not ANDed with a run-time flag and the last two because they carry numbers.
const FLAG_FLAT_SECONDARY: u32 = 1048576u;
const FLAG_WATER_FAR: u32 = 2097152u;
const FLAG_WATER_DARK: u32 = 4194304u;
const FLAG_SNELL: u32 = 8388608u;
const MAX_ITERS: u32 = 512u;

// How far a tile whose every ray stays inside the water may look, in blocks (batch 48).
//
// **The bound is blue's extinction and nothing else.** `WATER_EXT` is
// (0.280, 0.055, 0.020) per block, so red is spent within a few blocks and blue is the
// channel that decides when a surface behind the water has stopped contributing. Sizing this
// off red -- or off the roadmap entry's own "absorption kills the image within tens of
// blocks", which is red's number -- makes it about twenty times too short.
//
// **This is a decided trade and not a derived number, which is why the whole curve is here.**
// Batch 48 authored 256 off blue's extinction and could not check it; batch 52 built the camera
// that can and the user took the fast end of what it measured.
//
// `underwater` is a submerged eye with nothing in front of it -- its content ends within 32
// blocks, so every reach above 32 was bit-exact there at every extinction down to
// `--water-absorb 0.05`, and those zeros were the vantage rather than the constant.
// `sea-horizon` is the same medium aimed along the horizontal at islands 264 blocks out.
// Measured there: pixels of 921,600 against an **unlimited** reach at 1280x720, milliseconds
// paired 8 rounds against the 256 build at 1920x1080.
//
//   reach        pixels      max delta     frame
//   264 and up        0              0     (exact -- this is where the content ends)
//   256             316              1     baseline
//   224           1,640              3     -0.168 +/- 0.014
//   192          24,179              6     -0.604 +/- 0.041
//   128 (here)   67,621             23     -1.930 +/- 0.016   <-- 18% of the frame
//    64         175,890             50     unpriced
//
// **The two arguments that used to agree still agree, and they are both about 256.** The frame
// goes exact at 264 and blue's own 1/255 bound is `ln(255)/0.020` = 277 -- 5% apart, and batch
// 48's authored 256 sat 8 blocks under the measured edge. **128 is not that number.** It is
// half of it, and what it costs is on the row above: a pale seam where a distant shoreline's
// *underwater* silhouette is deleted and the brighter open-water haze behind it shows through.
// 5,032 of those 67,621 pixels are in the `horizon` crop, where the islands are; the other
// 62,589 are open water at a max delta of **2**.
//
// **Ranked against the precedent, this is the expensive kind of truncation, and it was taken
// anyway.** Batch 41 bought 1.672 ms for 1,309 pixels at max delta 1; this buys 1.930 for 51
// times the pixels at 23 times the delta. It is the first distance cut in this engine bought at
// a visible price rather than an empty band, and the reason is the size of the number: 18% of
// the frame at the vantage, against a `sea-horizon` frame that is 10.6 ms.
//
// **What no still frame ranked, and what would justify putting this back up:** the seam sits at
// a fixed distance from the *camera*, so it slides across the water as the player swims rather
// than staying with the shoreline. That was said before the decision and not measured. If it
// reads as a moving band in play, 224 is the row that costs almost nothing (1,640 pixels at max
// delta 3, batch 41's own signature) and 192 the one that still keeps a third of the money.
const WATER_FAR_DIST: f32 = 128.0;

// How deep a ray has to be for the sky flood never to have reached it, and how far a tile
// all of whose rays are that deep may look (batch 53).
//
// **`WATER_DARK_DEPTH` is derived and `WATER_DARK_DIST` is decided, and the two halves of this
// pair should not be read the same way.**
//
// The derivation: `light.rs` pours sky light straight down an open column at `MAX_LEVEL` = 15
// and takes one level off per block of *water* it passes, so a cell with 15 blocks of water
// above it holds sky level exactly 0. Nothing reaches it sideways either -- `flood` decrements
// one per step in **every** direction, and any path from the surface to depth `d` is at least
// `d` steps long, so sky at depth `d` is bounded by `max(15 - d, 0)` however it is reached.
// That is a bound rather than an observation, which is why this constant is 15 and not a
// number somebody swept. `dark_depth_matches_the_light_flood` holds it to `light::MAX_LEVEL`.
//
// **What the pair buys is an interval that is covered twice.** Past `WATER_FAR_DIST` a ray's
// content is absorbed -- batch 48's argument, blue's extinction, and the reason that constant
// exists. Below `WATER_DARK_DEPTH` its content is unlit. A tile that is under that depth for
// the **whole** of `[WATER_DARK_DIST, WATER_FAR_DIST)` has the two covers meeting with no gap
// between them, so it can stop at the shorter distance. The test is written as exactly that,
// nested inside batch 48's own rung in `tile_select`, because the absorption half is what
// covers the far end and this rung does not replace it.
//
// **Both ends of the interval have to be tested and the first cut of this tested one**, which
// is worth knowing because the version that is wrong is the one that reads more natural. Depth
// is linear in `t`, so "under `WATER_DARK_DEPTH` throughout" is "under it at both ends" -- and
// a descending ray is shallowest at the near end where a rising one is shallowest at the far
// end. Testing the far end alone admits every steeply-downward tile at any depth at all; it
// cost **98,323 pixels at max delta 9 at `sea-horizon`**, a vantage three blocks down where
// nothing is dark. `the_dark_rung_tests_the_eye_as_well` is the guard.
//
// **The interval runs from the eye and not from `WATER_DARK_DIST`**, which is stricter than
// the derivation needs and is the batch's one deliberate conservatism. See `tile_select` for
// the argument -- it is about what moves when the player turns rather than about the physics
// -- and [`errors.md`](../../docs/errors.md) for what the looser version was worth.
//
// **What is still a trade, and it is the same trade batch 52 took.** Unlit is not invisible:
// a surface with no sky light is a dark silhouette against the brighter open-water haze behind
// it, and deleting it lets the haze through. What the depth buys is that the silhouette
// carries no *detail* -- there is no lit texture on it to lose, which is what a shallow camera
// loses at the same distance and why `WATER_FAR_DIST` could not simply be lowered instead. The
// measured price says so plainly: batch 52's cut costs **max delta 23** and this one, on a
// band twice as near, costs **max delta 3**.
//
// **`WATER_DARK_DIST` is 64 because 64 is where the ladder stops moving, and that floor is
// structural rather than fitted.** A reach culls whole chunks on `box_distance`, and an LOD 0
// chunk is 64 blocks on a side, so below the chunk edge there is nothing left to remove. At
// `deep-water`, against an unlimited reach:
//
//   reach       pixels of 921,600   max delta   note
//   128 (off)      34,909              28       batch 52's rung alone -- the parent frame
//    96            36,902              28       +1,993 px at max delta 2 over the parent
//    64 (here)     54,485              28       +19,576 px at max delta **3** over the parent
//    48, 32, 24, 16   identical -- same sha256 as 64, four rungs of nothing
//
// So 96 is the retreat value and there is no rung below this one. The frame numbers are in
// `PERF.md` and the reasoning in `docs/water.md`.
const WATER_DARK_DEPTH: f32 = 15.0;

// How deep the eye has to be before the surface starts behaving like a surface (batch 54).
//
// **The one number in this file the user set by eye, and it is recorded as theirs.** Asked to
// judge the first build of Snell's window, they gave two corrections: *"the first 3 blocks of
// water should not have a mirror, the mirror should be below them"*, and *"it should be dark at
// depths ... midway to the bottom you dont really see the surface light that much if at all"*.
//
// **Those are the two ends of one ramp**, which is why there is one constant here and not two
// features. `smoothstep(SNELL_MIN_DEPTH, WATER_DARK_DEPTH, depth)` is 0 in the top three blocks
// -- where the frame is then *bit-identical* to the build without the feature, not merely close
// -- and 1 at the depth where the sky flood has already reached zero. Between them it fades the
// mirror in and the surface light out together.
//
// **The far end is derived and the near end is authored, and they should not be read the same
// way.** `WATER_DARK_DEPTH` is `light::MAX_LEVEL`: batch 53 established that a cell that deep
// holds sky level exactly 0 however the light got there, so ending the surface light at the same
// place makes the two halves of the engine agree rather than inventing a second horizon. Three
// is a look call and the physics does not support it -- the critical angle is 48.6 degrees at
// any depth. What supports it is that the flat-surface model is worst exactly there: a few
// blocks under a wave field, the facet slope is large against the solid angle a pixel covers, so
// a real swimmer near the surface does see through it at angles a flat sheet would mirror.
const SNELL_MIN_DEPTH: f32 = 3.0;

// How much of the surface light the ramp above is allowed to take away.
//
// **1.0 was built first and it is unusable, for a reason that is not this feature's fault.**
// Fading the transmitted leg all the way to `underwater_body()` makes a frame 24 blocks down
// uniformly near-black -- because that body colour is `WATER_BODY * medium_light(light at the
// eye)`, and the **sky flood is exactly zero below `WATER_DARK_DEPTH`**, so the only thing left
// in it is `frame.ambient`'s 0.08. The engine's deep water is far darker than clear water is,
// and that is the 4-bit light nibble showing through rather than physics.
//
// So the fade stops short, and what survives is the fraction of the surface light that keeps a
// deep frame readable. **This is a look constant in the sense `MEADOW_SHADE` is** -- swept by
// eye against the picture, at the one vantage that can show it.
// **Swept by eye at `deep-water`, 24 blocks down and pitched up into the window**, which is
// the only camera in the set where this constant does anything: 0.75 keeps a clearly readable
// window, 0.9 dims it to a deep blue that still carries the cloud shapes, and 1.0 is the flat
// near-black that started this. 0.9 is the user's *"not that much, if at all"* landing on the
// side of still being able to play. Moving it is one line and one render.
const SNELL_DIM_MAX: f32 = 0.9;
const WATER_DARK_DIST: f32 = 64.0;

// The light envelope's geometry, mirrored from `shaft.rs` and pinned there by
// `shaft_constants_match_the_shader`. `SHAFT_SLICE` is one slice's float count, `SHAFT_HEIGHTS`
// the base of the static heights, and `SHAFT_RESULT` the slice the last scan pass wrote --
// pass `k` writes slice `k & 1`, so an even `SHAFT_STEPS` leaves it in slice 1 and
// `shaft::STEPS` carries the `assert!` that keeps this true.
const SHAFT_DIM: u32 = 512u;
const SHAFT_STEPS: u32 = 8u;
const SHAFT_SLICE: u32 = SHAFT_DIM * SHAFT_DIM;
const SHAFT_HEIGHTS: u32 = 2u * SHAFT_SLICE;
const SHAFT_RESULT: u32 = SHAFT_SLICE;

// Batch 13. `SPEC_TINT` is the *same question* as `FLAG_TINT`, asked where the answer can be
// folded: `wgpu` hands overrides to naga before the SPIR-V is written, so a false one takes
// `biome_tint` -- and the two octaves of `biome_noise` under it -- out of the module the
// driver ever sees. A uniform branch on `frame.flags` cannot do that, and batch 12 measured
// what it costs not to: `--no-tint` was +0.07 ms at the default camera while executing
// nothing at all, because a shader is not only its control flow.
//
// It is ANDed with the flag rather than replacing it, and that is deliberate. `false && x`
// folds to `false` and `true && x` folds to `x`, so the specialized build gets the whole win
// and the default build is the same instructions it was -- while the flag stays the thing
// that decides, so a pipeline key that ever disagreed with `frame.flags` would draw the
// picture the flag asked for rather than a silently wrong one.
override SPEC_TINT: bool = true;

// Batch 15, and batch 13's machinery pointed at the pass that is 58% of the frame from the
// *other* side. `cross_quad` is reached from `march_chunk`, which `resolve` calls through
// `trace_world` for water's refraction and reflection rays, so batch 14's intersection is
// compiled into a pass that never draws a blade. Priced against a build with that code
// compiled out, it cost `resolve` +1.24 ms at the default camera and +2.07 down a coastline
// *while executing nothing at all*.
//
// It has to reach `foliage_aware` itself. Guarding the `cross_quad` call on a specialized
// bool folds nothing -- batch 14 tried exactly that, in both operand orders -- because the
// marcher still has to recognise a tuft in order to step *past* one, and the driver keeps
// the intersection alive for the branch it cannot prove dead. Killing the recognition is
// what strips it.
//
// **It is deliberately NOT ANDed with a run-time flag, and that is the one way it differs
// from `SPEC_TINT` above.** Written `SPEC_FOLIAGE && (frame.flags & FLAG_FOLIAGE) != 0u &&
// ...`, which is the shape `SPEC_TINT` uses and the obvious thing to reach for, `march`
// still folds to 11520 bytes and 52 registers and `resolve` folds to *224000 and 72* --
// against 181504 and 64 for the override alone. The extra term leaves `foliage_aware` a
// run-time value, and `resolve`'s inlined call tree is large enough that the driver stops
// propagating it where `march`'s is not. Both were measured; do not restore the flag test
// without re-measuring `resolve`.
//
// What makes that safe is that the flag has nothing to add. Ground cover is placed by the
// generator and by nothing else -- `TALL_GRASS` is not in `block::HOTBAR`, pinned by a
// `const _: () = assert!` beside `FLAG_FOLIAGE` in `render/mod.rs` -- so a run either has
// tufts in its world or has none, and `ensure_spec` keys the pipeline on the same word
// `flags_from` built from the generator's own answer. There is no third state for a
// run-time test to catch.
override SPEC_FOLIAGE: bool = true;

// Batch 16, and written in `SPEC_TINT`'s shape rather than `SPEC_FOLIAGE`'s -- ANDed with
// `FLAG_WAVES`, so the flag stays the thing that decides and a pipeline key that ever
// disagreed with `frame.flags` would draw what the flag asked for. Batch 15's lesson is
// that this choice is not free and not transferable, so it was measured on this pass too;
// the number is in `PERF.md` and in `docs/batch-16-waves.md`.
//
// The field is reached only from `shade_water`, which only `resolve` calls, so unlike
// `SPEC_FOLIAGE` this one has no business in `march` at all -- and `march`'s own numbers
// staying byte-identical is what says so.
override SPEC_WAVES: bool = true;

// Batch 19, in `SPEC_WAVES`'s shape and for the same reasons -- one more pass through the
// same argument, and one more pass through `shaderstats` rather than an inheritance of its
// answer. What this one removes when it is false is small: one dot, one fma and one sqrt
// per octave, four times per water pixel. It is in `SPEC_MASK` anyway, because a control
// that costs something is a tax on every A/B taken with it, and batch 15 paid 1.25 ms to
// learn that the cheap moment to avoid it is the batch that adds the code.
//
// `FLAG_WAVE_ANISO` is only ever set alongside `FLAG_WAVES`, so this override is dead
// whenever that one is -- there is no key with waves off and the grazing footprint on.
override SPEC_WAVE_ANISO: bool = true;

// Batch 20. Unlike the three above, what this one removes is not a few instructions: it is a
// `trace_world` call, and the only reason that is affordable at all is that `shade_water`
// already reaches the marcher twice for the refraction and the reflection, so the inlined
// body is code the pass was carrying anyway. The ray is a dozen blocks long and straight
// down, which is the most coherent ray in the frame -- every water pixel in a workgroup
// walks the same axis in the same direction.
//
// In `SPEC_MASK` for the usual reason and one extra: a control that switches off a *ray*
// and still pays for it would be the most misleading control in the file.
override SPEC_WAVE_SHOAL: bool = true;

// Batch 25. The one override here that changes how many terms the sum has rather than what
// each term does: off, `wave_field` is the four octaves of batches 16-20, on, it is eight
// subdividing the same 32..3.14 band. Both paths are written out in `resolve.wgsl` and this
// constant picks one at pipeline-compile time, so the build that is not running the fill
// schedule does not carry its four extra cosines.
//
// ANDed with `FLAG_WAVE_FILL`, which is `SPEC_TINT`'s shape and the shape both wave
// overrides above already use -- and batch 16 is why that is not a coin flip: it wrote this
// exact shape into this exact function after batch 15 had watched the same three lines lose
// `resolve`'s fold in `shade_hit`, and here it folded to the revert byte for byte. The fold
// is a property of the call tree the constant sits in, so the neighbours' answer is the
// best available prior and still not a proof; `shaderstats` and a SPIR-V diff are in the
// batch doc.
override SPEC_WAVE_FILL: bool = true;

// Batch 35, and the reason the terrain shaft's control can be free rather than nearly free.
// `resolve` bills for code that merely exists -- batch 12 measured +0.07 ms and batch 14
// +1.24, both while executing nothing -- so a run-time flag would leave `terrain_shade`, its
// bilinear tap and the four loads under it compiled into the pass that is 58% of the frame.
// A false override takes them out of the module the driver ever sees.
//
// ANDed with `FLAG_TERRAIN_SHAFTS` rather than replacing it, exactly as `SPEC_TINT` is: the
// flag stays the thing that decides, so a pipeline key that ever disagreed with `frame.flags`
// would draw the picture the flag asked for rather than a silently wrong one.
override SPEC_TERRAIN_SHAFTS: bool = true;

// Batch 36, and free for the same reason every override above it is: `resolve` bills for code
// that merely exists, so the hash, the permutation and the class load leave the module rather
// than being branched past. The mechanism finally has a name -- the L0 instruction cache, whose
// misses report as `Wait inst fetch` in RGP and as warp stall "No Instruction" in Nsight, and
// which is why batch 12's +0.07 ms and batch 14's +1.24 both showed up as neither ALU nor
// bandwidth. See `docs/research-review.md`.
override SPEC_TEX_VARIATION: bool = true;

// Batch 37, and free for the reason every override above it is: `shade_hit` runs per visible
// pixel and is most of what `resolve` costs, so a run-time flag would leave the offset, the
// bilinear tap and the four loads compiled into the pass whether or not anything asked for a
// distant shadow. A false override takes them out of the module the driver ever sees.
//
// ANDed with `FLAG_DISTANT_SHADOWS` rather than replacing it, exactly as `SPEC_TERRAIN_SHAFTS`
// is: the flag stays the thing that decides, so a pipeline key that ever disagreed with
// `frame.flags` would draw the picture the flag asked for rather than a silently wrong one.
override SPEC_DISTANT_SHADOWS: bool = true;

// Batch 38's leaf cutout. Folded rather than branched for the reason every override in this
// list is: `march_chunk` is reachable from `resolve` through water's secondary rays, so the
// micro-march would be billed to the pass that is 65% of the frame whether or not a canopy is
// on screen. With this false the hash, the 4^3 loop and the micro half of `hit_t` all leave
// the module.
override SPEC_LEAF_CUTOUT: bool = true;

// Batch 72: a root-FULL chunk marches as if its tree existed. Default on, and the
// default is the fix: the pre-batch path stopped every ray at the chunk's face without
// asking whether it ignores water, and a mixed full chunk -- sea floor above stone, no
// air -- was therefore wrong to `resolve`'s refracted, reflected and shadow legs
// (31,815 pixels at `coastline`, max delta 95, measured in batch 53; roadmap P10).
// `--no-full-march` is the control and reproduces the pre-batch frame bit-exactly.
override SPEC_FULL_MARCH: bool = true;

// Batch 75 (roadmap A3): the wet-sand band and the shoreline foam. Both are authored
// against a waterline, both cost work in every resident frame that has one, and a world
// with `sea_level` zeroed provably has neither -- so both fold out of that build at
// compile time, which is `SPEC_WAVES`'s own argument moved two rungs down the word.
override SPEC_SHORE_WET: bool = true;
override SPEC_SHORE_FOAM: bool = true;

// Batch 73 (roadmap P11): whether the world holds any emitter at all. Off, the
// data-dependent `any_blk` gate and the three-channel gather it feeds fold out of the
// build -- they measured 0.351 ms at `cave` against an always-zero read and the read
// cannot be nonzero in such a world, so the picture is bit-exact by argument. On, the
// text is the shipping one (`lamps` takes this arm).
override SPEC_EMITTER_GATHER: bool = true;

// ---- the gate shape, measured in batch 46 and left alone ----
//
// **Eight of the switch overrides above and below are written `SPEC_X && (frame.flags & FLAG_X)
// != 0u`, and batch 46 folded all eight to the override alone and threw the build away.**
// `SPEC_TERRAIN_SHAFTS`, `SPEC_TEX_VARIATION`, `SPEC_TINT`, `SPEC_DISTANT_SHADOWS`,
// `SPEC_WAVES`, `SPEC_WAVE_ANISO`, `SPEC_WAVE_SHOAL` and `SPEC_WAVE_FILL` -- `SPEC_FOLIAGE`
// was already in the folded shape and `SPEC_LEAF_CUTOUT` and `SPEC_GLASS` are gated
// differently again.
//
// The reason to try was batch 45, where exactly this rewrite was worth **2.58 ms** at
// `terraces`: the run-time half of the test stopped the driver proving which arm `shade_hit`
// took, so both arms stayed in all three inlined copies. The reason it does not generalise is
// that batch 45's arm was the **nine-cell light gather in a function `resolve` inlines four
// times**, and these eight guard small arms. All eight together are
// **-0.119 ms +/- 0.029 at `terraces`** and inside their own standard error at `coastline`,
// `default`, `shore` and `low-sun`; the two largest movers alone are -0.058 +/- 0.039 and
// **+0.001 at `open-sea`**. `resolve` drops 12,544 bytes against batch 45's 60,928.
//
// **So the `frame.flags` half stays**, and it is not merely inertia: it is what keeps the flag
// word the thing that decides, so a pipeline key that ever disagreed with `frame.flags` would
// still draw the picture the flag asked for. Batch 45 spent that property once, for a
// millisecond and a half. A tenth of a millisecond at one vantage does not buy it eight more
// times. **Reach for the fold when the arm is large and the function is inlined several
// times, and check with `shaderstats` first** -- the byte delta predicted the whole of this
// result and cost two minutes. See `docs/gpu.md` and `PERF.md`.


// Batch 38b's glass. Folded rather than branched for the same reason, and with a second one
// on top of it: `shade_glass` is a whole secondary trace plus a Fresnel mix living in the
// pass that is 65% of the frame, and glass is a block the *generator never places*. A run
// that never sees a window would otherwise carry every byte of it. With this false,
// `shade_glass`, its `trace_world` call and the glass arm of the marcher leave the module,
// and a glass block shades as the opaque cube it was before this batch.
override SPEC_GLASS: bool = true;

// Batch 41, and **the only override in this list that carries a number rather than a switch**.
// How far a shadow ray cast from a surface reached *through* the water may march, against
// `frame.shadow_dist`'s 220 through air: 16 blocks since batch 41, 48 before it, and
// `--no-water-shadow-cut` is what asks for the second. It is a value and not a bool because
// there is no code to fold away here -- the same one `shadow_ray` runs either way, for a
// different number of steps -- so a bool would have had to `select` between two literals on
// the path of the call, which is exactly the shape batch 38 measured costing a control its
// bit-exactness. An override that *is* the number puts the same literal in the same place the
// `const` used to sit.
//
// **The consequence of that choice is that the flag does not appear in the shader at all**,
// where every override above it is ANDed with its own `frame.flags` bit. There is nothing to
// AND: a distance cannot be expressed in one bit of a flag word, so the pipeline key is the
// only copy of the decision. `render::FLAG_WATER_SHADOW_CUT` is in `SPEC_MASK` and fixed for
// the life of the process, which is what makes that safe.
//
// **Why 16 is not a loss of shadow**, which is the whole finding: batch 37 gave `shade_hit` a
// second occluder, and `terrain_shade_beyond` is passed *this* distance, so the light envelope
// picks up exactly where the march stops. Cutting the march short does not delete the sea
// floor's shadow, it moves the handoff -- and what is given up is the 16-to-48-block band's
// worth of *non-terrain* occluders, since the envelope is `WorldGen::height` and knows nothing
// about a tree or a player's wall. See `render::WATER_SHADOW_DIST` and `docs/water.md`.
override SPEC_WATER_SHADOW_DIST: f32 = 16.0;

// How much of a leaf block is solid -- the second override here to carry a number rather than
// a switch, and the one *look* constant batch 38 authored rather than derived. It sets how
// much sky a canopy lets through and therefore how much of a tree's silhouette breaks up.
//
// **0.62, swept by eye in batch 43; 0.72 before that and 0.45 in the research.** The sweep is
// the thing to read before moving it, because the two halves of the frame want opposite
// directions and the number is where they meet:
//
// - *Against sky*, the silhouette wants it **low**. At 0.82 and 0.90 the crest of a wooded
//   ridge is the same blocky edge a solid cube gives, so the whole of batch 38 is invisible
//   there; the breakup only becomes legible below about 0.65.
// - *Against ground*, the crown's interior wants it **high**. Every carved cell that opens
//   onto shadowed interior reads as a dark speckle, and at 0.45 a canopy seen from above is
//   visibly moth-eaten -- which is exactly the "tree that has been shot at" the old comment
//   was guarding against, now seen rather than argued.
//
// 0.62 is the balance: a clearly broken skyline with only a subtle interior speckle.
// `--no-leaf-thin` restores 0.72 and is bit-exact against `voxelcraft-pre43.exe`; being in
// `SPEC_MASK` it costs nothing, because the two builds differ in one immediate operand and in
// no code at all. See `render::LEAF_FILL` and `docs/foliage.md`.
override SPEC_LEAF_FILL: f32 = 0.62;

// Batch 45. Whether a hit reached *through* water or glass is shaded with the nine-cell
// gather or with one light lookup. True is the shipping build; `--no-flat-secondary` clears
// the flag and every `shade_hit` call site passes what the primary passes, which is the
// pre-batch-45 module exactly.
//
// **It folds rather than branching for the reason every override above it does**, and with one
// extra: the arm it removes is inside `shade_hit`, which `resolve` inlines *four* times -- once
// for the primary hit and once for each of water's two legs and glass's one -- so a run-time
// test would be carried in four copies to answer a question that cannot change during the run.
// With this false the flat arm leaves the module and the primary's copy is untouched either
// way, because that call site passes a literal `true`.
//
// **Why a secondary hit can take the cheaper model**, which is the look half: the nine-cell
// gather buys per-corner smoothing and AO, and both are a cue about where a face meets its
// neighbours. A surface seen in a reflection is Fresnel-weighted at a grazing angle off a
// wave-perturbed normal; one seen through refracting water is multiplied by
// `exp(-extinction * depth)`; one behind a pane is dimmed by the pane's own texel. Each of
// those destroys the cue before the pixel is written. See `render::FLAG_FLAT_SECONDARY`.
override SPEC_FLAT_SECONDARY: bool = true;

// Batch 55. Whether `shade_hit` takes the probe tap above. **Ships false**, which is the one
// thing about this override that is not the house style and is the whole point: a diagnostic
// that costs the frame the time it was built to measure would tax every measurement taken
// through it afterwards, which is what the `--no-foliage` regression cost this project.
//
// It is the *whole* gate, with no `frame.flags` companion, for batch 45's reason twice over:
// the arm sits in `shade_hit`, which `resolve` inlines four times, and a run-time test would
// keep the texture sample in every copy -- so the shipping build would pay for the tap it does
// not take, and the measurement would be of the measurement. See `render::FLAG_PROBE_TAP`.
override SPEC_PROBE_TAP: bool = false;

// Batch 56. Whether the probe field *replaces* `shade_hit`'s ambient rather than merely being
// sampled beside it. **Ships false, and the build it makes is deliberately incorrect** -- the
// field holds a constant, so the frame it renders is wrong and is thrown away. What it measures
// is the other half of batch 55's question: that batch priced what a probe tap **adds**, and
// roadmap L1's architecture only pays if what it **removes** is larger.
//
// With it on, the terms a pre-composed directional field would make redundant leave the module:
// the nine `blk_n` `light_curve` evaluations, the four per-corner `bk/(sk+bk)` divisions and
// their `mix(SKY_TINT, BLOCK_TINT, ..)`, the `amb` bilinear, `face_shade` and `floor_rgb`. What
// stays is everything the field could not carry -- the nine gather lookups, `sky_n`, the AO rule
// and `sky_sm`, which gates the sun and is high-frequency in a way a coarse probe is not.
//
// **It implies `SPEC_PROBE_TAP` rather than repeating it**: the ambient has to come from
// somewhere, and a build with the removal and no tap would be measuring a shader that reads no
// light at all. `flags_from` sets both bits and `tests/probe.rs` holds the implication.
override SPEC_PROBE_AMBIENT: bool = false;

// Batch 57. Whether `shade_hit` takes its directional shading factor from the baked ambient
// cube instead of from `face_shade`'s per-normal constants. **Ships true**, and
// `--no-probe-cube` is the control.
//
// The override is the whole gate, with no `frame.flags` companion, for `SPEC_PROBE_TAP`'s
// reason: the arm is in `shade_hit`, `resolve` inlines that four times, and a run-time test
// would keep the texture sample in all four copies of the control build. See
// `render::FLAG_PROBE_CUBE`.
// **The declared default is the *shipping* value, which is the convention every override in
// this file follows and the reason this one is `true` where the two diagnostics above it are
// `false`.** Nothing in the renderer reads it -- `make_spec` sets every override explicitly --
// but `shaderstats` folds the declared defaults, so a default that disagreed with the shipping
// build would have it reporting the control's register count and code size as the shipping
// one, silently and with every row internally consistent.
override SPEC_PROBE_CUBE: bool = true;

// Batch 58. Whether `shade_hit`'s ambient floor takes its colour and magnitude from the baked
// ground bounce instead of from one authored constant. **Ships true**, and
// `--no-probe-bounce` is the control.
//
// The override is the whole gate for `SPEC_PROBE_CUBE`'s reason, and it shares that field's
// tap rather than adding one: `probe_field` returns a `vec4` whose `.a` is the cube and whose
// `.rgb` is this. See `render::FLAG_PROBE_BOUNCE`.
//
// The declared default is the shipping value, for the reason spelled out above it: nothing in
// the renderer reads it, but `shaderstats` folds the declared defaults, so a default that
// disagreed with the shipping build would have it reporting the control's numbers as the
// shipping ones with every row internally consistent.
override SPEC_PROBE_BOUNCE: bool = true;
override SPEC_PROBE_SUN: bool = true;
override SPEC_PROBE_SUN_HIGH: bool = false;

// Batch 59. Whether block light is read as three channels or as the single pre-59 level
// wearing `BLOCK_TINT`. **Ships true**, and `--no-light-rgb` is the control.
//
// **The override gates two *textually separate* arms rather than one expression with a
// `select` in it, and that is deliberate.** The control's whole claim is that it reproduces
// the pre-59 frame to the bit, and the cheapest way to guarantee that is for the pre-59 code
// to still be there, unedited, with the pipeline folding the other arm away. A single shared
// expression that happened to reduce to the old one -- a vec3 sum whose three lanes are equal,
// say -- is a claim about how naga reassociates, and batch 58 lost two pixels at `terraces` to
// exactly that. The duplication is the proof.
//
// The override is the whole gate for `SPEC_PROBE_CUBE`'s reason: the arms are in `shade_hit`,
// which `resolve` inlines four times. See `render::FLAG_LIGHT_RGB`.
//
// The declared default is the shipping value, for the reason spelled out above `SPEC_PROBE_CUBE`.
override SPEC_LIGHT_RGB: bool = true;

// Batch 63, roadmap R7: the ambient's sky hue is the sky model's own rather than `SKY_TINT`.
// Cleared, `sky_ambient_tint` returns that constant unchanged and the frame is the pre-63 one
// to the bit -- the revert is the `else` arm of one function and needs no claim about naga.
//
// The override is the whole gate, for `SPEC_LIGHT_RGB`'s reason directly above: the arms are in
// `shade_hit`, which `resolve` inlines four times, and `resolve` reads no `frame.flags` bit for
// this. See `render::FLAG_SKY_TINT`, which is bit 31 and the last one `frame.flags` has.
//
// The declared default is the shipping value, for the reason spelled out above `SPEC_PROBE_CUBE`.
override SPEC_SKY_TINT: bool = true;

// Batch 65, roadmap R6. Every opaque surface in this world has been perfectly diffuse since
// batch 1; with this on, each one carries a Fresnel-weighted reflection of the sky.
//
// **Driven by `FLAG_HI_SKY_SPECULAR` and not by a `frame.flags` bit, because there is no bit
// left** -- batch 63 spent 31. See `SPEC_HI_MASK` in `render/mod.rs` for what that word is and
// the one rule it carries. The arm is in `shade_hit`, which `resolve` inlines four times, so
// this override is the whole gate and no `frame.flags` test appears anywhere: batch 45 measured
// what adding the run-time test costs on this exact function and it was +1.42 ms.
override SPEC_SKY_SPECULAR: bool = true;

// Batch 81, roadmap P12's **diagnostic arm** and nothing more yet: the secondary water
// legs evaluated once per 2x2 block of pixels and read back by their owner texel, point
// sampled, with no guided filter and no edge stop. It exists to be measured against
// `--reference` at `open-sea`/`shore`/`terraces` and discarded -- its MAE decides whether
// the guided pass is worth its session (near 2.87) or changes design first (at 15), and
// nothing about it is judgement-shaped. Being in the second specialization word, the
// `--no-water-sec` build contains the unmodified batches-8 shade path bit for bit; see
// `FLAG_HI_WATER_SEC` in render/mod.rs for the decisions that lock in *after* the read.
override SPEC_WATER_SEC: bool = true;

// log2 of the block edge the legs are traced at: an override *value* because the shader
// divides, shifts and loops against it rather than guarding. At `1` (`--water-sec-scale
// 1`) donors are 1x1 and the pass shades exactly per pixel -- a sanity rung for the
// fraction sweep, not the shipping one.
override WATER_SEC_SHIFT: u32 = 1u;

// Batch 90, roadmap A9 (`--caustics`): sun caustics on the flooded floor, folded out of
// the shipping build. The arm multiplies the direct sun term of any hit lying between
// sea level and `CAUSTIC_DEPTH` by sheets read off the engine's own wave octaves at the
// sun-projected waterline point -- the field the surface shades, refocused. Nested under
// `sea_level > 0` on the CPU side exactly where `FLAG_HI_WATER_SEC` is: waterless, the
// bit never sets and this override never sees anything but the `false` below.
override SPEC_CAUSTICS: bool = false;

// Batch 90, roadmap A4 (`--glass-reflect`): a pane's reflection traced -- one
// `water_reflect_leg` march replacing the sky-only `refl` -- instead of assumed. Roadmap
// wording says the function keeps its (by now generic-but-misnamed) name; the legality
// note at `shade_glass` is why this is safe from inside it. Off by default: the entry is
// an A/B the by-eye pass reads, and the shipping pane keeps the assumption.
override SPEC_GLASS_REFLECT: bool = false;

// Batch 91, roadmap A2c (`--snell-bend`): the marcher's two-segment bend at the water
// boundary, folded out of the shipping module. What ships since batch 54 is the window's
// *weight* with a hole's geometry: inside the cone the above-world is undistorted because
// `march` traced a straight line. The arm bends the transmitted half of that line at the
// sea plane and re-marches; `resolve` needs nothing -- its Fresnel/TIR/mirror terms all
// read the original `rd` and a `dist` the bend preserves as `t_surf + t2`, so the weight
// inside the window is unchanged and only which geometry it lands on moves.
override SPEC_SNELL_BEND: bool = false;

// Batch 92, roadmap A8 (--tint-balance). Not new code: a *swap of one constant* in
// biome_tint (plus atlas content the CPU builds), so the off pipeline is bit-identical
// by the same fold-as-false argument SPEC_TINT under it makes, and the on pipeline costs
// the same instructions with different immediates. false here is the law, same as the
// family it sits in: the by-eye pass sets the bit it reads.
override SPEC_TINT_BALANCE: bool = false;

// Batch 95 (roadmap G1): one override for the four sky-look knobs, in `SPEC_TINT_BALANCE`'s
// own folding class -- and this time the class is the whole point. Batch 95a/b shipped the
// guards as `if frame.knob > 0.0` and the hardware round measured what that class actually
// buys: seven of twenty-one control vantages moved 1-4 pixels at max delta 1, floating-point
// reassociation in the arithmetic *around* branches the spec never folds. With
// `SPEC_SKY_LOOK &&` ahead of each uniform test the door closes again by construction: the
// unarmed module is the pre-95 one, because the text of the arm is not in it. The knobs stay
// uniforms past the override, so the by-eye ladders sweep magnitudes without a rebuild.
override SPEC_SKY_LOOK: bool = false;

// Batch 97a, roadmap G3's sky half (`--sky-cool`): the same fold class as
// `SPEC_SKY_LOOK`, because the constants it swaps stand in the same two functions and
// the control is the same claim -- with the bit clear every one of its selects folds
// dead and the module is the pre-97 one bit for bit.
override SPEC_SKY_COOL: bool = false;

// Batch 97's flagship arms (`--water-look`, `--foliage-rich`, `--canopy-relief`): all
// three live in code the marcher inlines, where a uniform guard bills the control for
// its presence -- batch 95's re-gate measured that class once, and it is not being
// re-measured three more times.
override SPEC_WATER_LOOK: bool = false;
override SPEC_FOLIAGE_RICH: bool = false;
override SPEC_CANOPY_RELIEF: bool = false;
// Batch 101f (`--wind-sway`), in exactly the 97d/97e shape: a look argument to geometry
// the marcher inlines, so it is a spec bit and never a uniform the control pays to see.
override SPEC_WIND_SWAY: bool = false;
// Batch 102a (`--soft-shadows`): the sun-disc penumbra, in the same shape -- the taps
// stand in `shade_hit`, which `resolve` inlines four times, so a uniform guard would
// carry the cone four times over in the control that never runs it. With the bit clear
// the arm's text is not in the module and `shadow_ray`'s one bit stands bit for bit.
override SPEC_SOFT_SHADOWS: bool = false;

// `block::WATER`. Mirrored by hand, and pinned on the Rust side by a `const _: () = assert!`
// next to the flag constants, because nothing else connects the two files.
const WATER_ID: u32 = 13u;
// `block::SAND`, mirrored and pinned exactly as `WATER_ID` above it is (batch 75: the wet
// band is an id compare, and the pin is `const _: () = assert!(crate::block::SAND == 4)`,
// placed next to the flag constants that read it).
const SAND_ID: u32 = 4u;
// `block::GLASS`, mirrored and pinned exactly as `WATER_ID` above it is. An id compare and
// not a flag bit in the key: the key has had no spare bit since batch 38 spent the voxel
// field's last six on the micro-cell, and it turns out never to have needed one -- `resolve`
// already reads the block id back out of the attributes with `block_at` to tell water from
// everything else, and this is the same read with a second answer.
const GLASS_ID: u32 = 10u;
const GLOWSTONE_ID: u32 = 11u;
override SPEC_LIGHTING_REPAIR: bool = true;
override SPEC_WATER_REPAIR: bool = true;
override SURFACE_DEBUG: u32 = 0u;
fn casts_sun_shadow(id: u32) -> bool { return id != GLOWSTONE_ID; }
// Point material query used only to reject water/water interior interfaces.
fn world_block_at(p: vec3<f32>) -> u32 {
    let cell = vec3<i32>(floor(p / 64.0)) - frame.grid_min;
    if any(cell < vec3<i32>(0)) || any(cell >= vec3<i32>(frame.grid_dim)) { return 0u; }
    let u = vec3<u32>(cell);
    let ci = grid[u.x + u.y * frame.grid_dim.x + u.z * frame.grid_dim.x * frame.grid_dim.y];
    if ci == NO_CHUNK { return 0u; }
    let c = chunks[ci];
    let v = vec3<i32>(floor((p - c.origin) / c.voxel_size));
    if any(v < vec3<i32>(0)) || any(v >= vec3<i32>(64)) { return 0u; }
    if !voxel_occupied(ci, v) { return 0u; }
    return block_at(ci, vec3<u32>(v));
}
// `attr_flags`: bit 0 says the whole chunk is one block id, bit 1 that it holds water
// somewhere. The second exists purely so every chunk that has none pays nothing at all for
// water: telling water apart from opaque geometry costs an attribute lookup, and outside
// the shoreline chunks there is never anything to tell apart.
const ATTR_UNIFORM: u32 = 1u;
const ATTR_HAS_WATER: u32 = 2u;
const ATTR_HAS_FOLIAGE: u32 = 4u;
const ATTR_HAS_CUTOUT: u32 = 8u;
// Batch 38b. Fourth of the same kind, and the one whose zero covers essentially the whole
// world: no generator places glass, so this bit is clear in every chunk of an unedited run
// and the glass arm of the marcher is never entered at all.
const ATTR_HAS_GLASS: u32 = 16u;
const ATTR_HAS_EMITTER: u32 = 32u;

// Batch 14. Carried as a literal for the same reason `WATER_ID` is -- a shader cannot see
// the crate -- and pinned by `const _: () = assert!(block::TALL_GRASS == 18)` in
// `render/mod.rs`, which is again the only thing tying the two files together.
const TALL_GRASS_ID: u32 = 18u;
// Batch 101e (`--grass-dense`): the two ids that ride the tuft's contract. Ground cover
// is *air until a blade is hit* -- the marcher's skip-one-cell trick, the AO/flood
// non-occlusion, and the uniform-fast-path's foliage exception all answer that one
// question, so the question is asked once, here; a fourth site a copy-paste away from
// the other three is how a new cover block ends up a green cube in some pass and air in
// another. Pinned beside `render/mod.rs`'s other id asserts.
const GRASS_TALL_ID: u32 = 24u;
const REEDS_ID: u32 = 25u;
fn is_tuft(b: u32) -> bool {
    return b == TALL_GRASS_ID || b == GRASS_TALL_ID || b == REEDS_ID;
}

// The two cross-quad planes, in the normal field's states 6 and 7. Cube faces use 0..5 and
// a 3-bit field has eight values, so the second primitive needed no bit reallocation
// anywhere -- not in the key, not in the 24 bits of voxel coordinate beside it.
//
// Plane A holds local `x - z = 0` and plane B holds `x + z = 1`, which is what lets both
// `hit_t` and `normal_of` re-derive everything about the surface from the voxel and these
// two states alone. Which *side* of a blade is being looked at is not stored: a blade is
// double-sided and the shading flips the normal against the view ray.
// +Y. Named because batch 16's wave field is a height field over XZ and means nothing on
// any other face: a shoreline's vertical water wall stays exactly the batch-8 surface.
const NORMAL_PY: u32 = 3u;
const NORMAL_CROSS_A: u32 = 6u;
const NORMAL_CROSS_B: u32 = 7u;

fn mask_has(m: vec2<u32>, bit: u32) -> bool {
    if bit < 32u {
        return ((m.x >> bit) & 1u) != 0u;
    }
    return ((m.y >> (bit - 32u)) & 1u) != 0u;
}

fn mask_below(m: vec2<u32>, bit: u32) -> u32 {
    if bit < 32u {
        return countOneBits(m.x & ((1u << bit) - 1u));
    }
    if bit == 32u {
        return countOneBits(m.x);
    }
    return countOneBits(m.x) + countOneBits(m.y & ((1u << (bit - 32u)) - 1u));
}

fn cell_bit(c: vec3<u32>) -> u32 {
    return c.x | (c.y << 2u) | (c.z << 4u);
}

// Block id of a solid voxel through the decoupled attribute table (Dolonius-style leaf
// index), plus whether the whole 4^3 leaf carries that one id.
//
// The second component is what makes water cheap to skip: a leaf that is uniformly water
// is four voxels of nothing to a ray that ignores water, so the marcher steps the entire
// leaf instead of testing its 64 voxels one at a time. Ocean interiors are exactly that
// kind of leaf, which is where a ray under the surface spends all of its time.
// Decode one voxel out of a leaf's attribute entry: an inline uniform id, or an index into
// the palette-compressed block that follows. Split out of `block_at` so `march_chunk` can
// keep the entry itself in a register across the four voxels of a leaf, which is the
// difference between one load per water step and a walk from the root per water step.
fn leaf_block(entry: u32, v: vec3<u32>) -> u32 {
    if (entry & UNIFORM_BIT) != 0u {
        return entry & 0xFFFFu;
    }
    let hdr = bricks[entry];
    let n = hdr & 0xFFu;
    let bits = (hdr >> 8u) & 0xFFu;
    let pal_words = (n + 1u) / 2u;
    let vb = cell_bit(v & vec3<u32>(3u));
    let bitpos = vb * bits;
    let w = bricks[entry + 1u + pal_words + (bitpos >> 5u)];
    let idx = (w >> (bitpos & 31u)) & ((1u << bits) - 1u);
    let pw = bricks[entry + 1u + idx / 2u];
    return (pw >> ((idx & 1u) * 16u)) & 0xFFFFu;
}

// Attribute entry for the leaf containing `v`.
fn leaf_attr(ci: u32, v: vec3<u32>) -> u32 {
    let c = chunks[ci];
    let root_mask = vec2<u32>(c.root.x, c.root.y);
    let l1b = cell_bit(v >> vec3<u32>(4u));
    var l1: vec4<u32>;
    if (c.root.z & FULL_BIT) != 0u {
        l1 = vec4<u32>(0xFFFFFFFFu, 0xFFFFFFFFu, FULL_BIT, l1b * 64u);
    } else {
        l1 = inners[(c.root.z & PTR_MASK) + mask_below(root_mask, l1b)];
    }
    let lfb = cell_bit((v >> vec3<u32>(2u)) & vec3<u32>(3u));
    return bricks[c.attr_base + (l1.w & 0x0FFFFFFFu) + mask_below(vec2<u32>(l1.x, l1.y), lfb)];
}

// Block id of a solid voxel through the decoupled attribute table (Dolonius-style leaf
// index).
fn block_at(ci: u32, v: vec3<u32>) -> u32 {
    let c = chunks[ci];
    if (c.attr_flags & ATTR_UNIFORM) != 0u {
        return c.attr_base;
    }
    return leaf_block(leaf_attr(ci, v), v);
}

fn ray_dir(px: f32, py: f32) -> vec3<f32> {
    // Every ray in the frame shifts by the same sub-pixel amount, tile corners included,
    // so tile_select's frustum still bounds exactly the rays its own tile marches.
    let ndc_x = ((px + frame.jitter.x) / f32(frame.res.x)) * 2.0 - 1.0;
    let ndc_y = 1.0 - ((py + frame.jitter.y) / f32(frame.res.y)) * 2.0;
    var d = normalize(frame.cam_fwd
        + frame.cam_right * (ndc_x * frame.tan_half_fov * frame.aspect)
        + frame.cam_up * (ndc_y * frame.tan_half_fov));
    // Keep every component non-zero so reciprocals stay finite.
    let tiny = abs(d) < vec3<f32>(1e-6);
    return select(d, vec3<f32>(1e-6, 1e-6, 1e-6) * sign(d + vec3<f32>(1e-9)), tiny);
}

// ---- stochastic LOD cross-fade ----

// Interleaved gradient noise (Jimenez), animated by walking the sample point along x.
//
// Not void-and-cluster blue noise: that wants a texture and a twentieth binding, and this
// has the one property the dissolve actually needs, which is that neighbouring pixels get
// well-separated values. A threshold then cuts an 8x8 tile into a fine interleaved pattern
// rather than into clumps, so the 3x3 neighbourhood the temporal clip builds contains both
// representations and the clip box is wide enough to let the other one's history through.
// That box is the mechanism the whole cross-fade resolves by.
//
// The animation offsets the *coordinates*, not the result. Adding a per-frame constant to
// the value would move every pixel the same way at once and sweep the dissolve across the
// screen like a wipe; offsetting the sample point keeps the spatial spectrum and gives each
// pixel its own sequence, which is what lets a ~7-frame temporal window average it out.
fn dither(px: u32, py: u32) -> f32 {
    let p = vec2<f32>(f32(px) + 5.588238 * f32(frame.frame_index), f32(py));
    return fract(52.9829189 * fract(dot(p, vec2<f32>(0.06711056, 0.00583715))));
}

// Does this ray belong to the other representation? `fade` is a **signed threshold**:
// 1.0 marches everything, `+f` marches the rays whose dither falls below `f`, and `-f`
// marches the rays at or above it. A parent carrying `-f` and its children carrying `+f`
// therefore split every pixel between them exactly once, which is the only safe split --
// a pixel both sides march resolves by depth in the visibility buffer and punches the
// coarse surface through the fine one, and a pixel neither marches is a hole.
fn ray_skips(fade: f32, px: u32, py: u32) -> bool {
    if fade >= 1.0 {
        return false;
    }
    let d = dither(px, py);
    // Two ifs rather than one `select`: WGSL's template-list disambiguation reads the `<`
    // and `>` of `select(d < -fade, d >= fade, ...)` as a generic argument list and fails
    // to parse it.
    if fade >= 0.0 {
        return d >= fade;
    }
    return d < -fade;
}

fn normal_of(id: u32) -> vec3<f32> {
    if id >= NORMAL_CROSS_A {
        // The cross-quad planes, unit length so every lighting term that assumes a unit
        // normal keeps working unchanged. Plane A (`x - z = 0`) faces (1,0,-1); plane B
        // (`x + z = 1`) faces (1,0,1). The sign is *not* meaningful on its own -- a blade is
        // lit from whichever side the eye is on, and `shade_hit` flips this against the view
        // ray, which is why two states are enough for what would otherwise need four.
        let s = select(-1.0, 1.0, id == NORMAL_CROSS_B);
        return vec3<f32>(0.70710678, 0.0, s * 0.70710678);
    }
    let axis = id >> 1u;
    let s = select(-1.0, 1.0, (id & 1u) == 1u);
    if axis == 0u { return vec3<f32>(s, 0.0, 0.0); }
    if axis == 1u { return vec3<f32>(0.0, s, 0.0); }
    return vec3<f32>(0.0, 0.0, s);
}

// Hashed alpha threshold for the foliage cutout, anchored to the surface point rather than
// to the pixel.
//
// Anchoring is the whole trick. A threshold that varied per pixel would swim under camera
// motion and TAA would fight it; one that is a pure function of *where the blade edge is*
// gives the same answer from every camera position, so the history lines up and the
// temporal pass integrates the noise into coverage. That is the same bargain batch 7's LOD
// dissolve strikes with `ray_skips`, and it is why a hashed cutout beats a hard one here:
// a binary test crawls as a tuft shrinks toward a pixel, and this dissolves instead.
//
// The point is in chunk-local voxel units, which is world space up to a per-chunk
// translation because foliage only ever exists at LOD 0 where `voxel_size` is 1. Stability
// per point is what matters and translation does not disturb it.
fn hash_alpha(p: vec3<f32>) -> f32 {
    let q = vec3<i32>(floor(p * 64.0));
    var h = (u32(q.x) * 73856093u) ^ (u32(q.y) * 19349663u) ^ (u32(q.z) * 83492791u);
    h = h ^ (h >> 13u);
    h = h * 1274126177u;
    return f32((h >> 8u) & 0xFFFFFFu) / 16777216.0;
}

struct CrossHit {
    hit: bool,
    t: f32,
    normal: u32,
};

// Ray against the two diagonal quads standing in one voxel, in that chunk's voxel space.
//
// Both planes are tested, the nearer one first, and each is alpha-tested before it may stop
// the ray -- a ray that misses every blade in a voxel has to carry on to the next one, which
// is exactly where this batch's cost lives. `t` comes back in voxel units like the rest of
// `march_chunk`; the caller scales it.
// ---- batch 101f (`--wind-sway`): the grass the references have *moves* ------------
//
// The field is two traveling sines, not a per-cell coin: real gusts arrive as bands
// across a meadow and every screenshot of peppered white-noise sway is what tells a
// shader it was written this week. The periods, 1.9 and 3.1, are incommensurate, so the
// two never line up in lockstep within a by-eye pass's patience.
const WIND_SWAY_T1: f32 = 1.9;
const WIND_SWAY_T2: f32 = 3.1;
/// Blocks of lean per block of height at full gust, after the two-sine sum. A 2 m
/// tussock's tip travels about a fifth of a block here, which was the whole of the
/// tuning: more reads as noodles, less reads as the screenshot gap the arm exists to
/// close. The lean is **anchored per cell** (its `base.y`), deliberately traded for a
/// global anchor: a global one would displace the blade spine by `lean * (y - anchor)`
/// and sail a blade several hundred blocks above it clean out of its voxel -- the
/// clip below would make tufts blink at gust extremes. The price of the per-cell
/// anchor is a lean discontinuity of at most `AMP` blocks at a stacked cell seam,
/// which reads as exactly the bend real grass has at a joint.
const WIND_SWAY_AMP: f32 = 0.09;

/// The gust vector at one world column and instant: `dx/dy, dz/dy` of the lean field.
/// One heading -- 0.8/0.6 -- so the grass and the clouds agree about which way the
/// weather is going (see `CLOUD_WIND`'s drift). Amplitude multiplies the two sines:
/// 0.65 of the slow band, 0.35 of the quick one, and the spatial phases ride world
/// coordinates, so the bands *travel* rather than shimmer in place.
fn wind_shear(wx: f32, wz: f32, time: f32) -> vec2<f32> {
    let g = 0.65 * sin(time * WIND_SWAY_T1 + wx * 0.11 + wz * 0.07)
        + 0.35 * sin(time * WIND_SWAY_T2 + wx * 0.173 + wz * 0.129 + 1.7);
    return vec2<f32>(g * 0.8, g * 0.6) * WIND_SWAY_AMP;
}

fn cross_quad(uc: vec3<u32>, ro: vec3<f32>, rd: vec3<f32>, layer: u32, vs: f32, shear: vec2<f32>) -> CrossHit {
    var r: CrossHit;
    r.hit = false;
    r.t = 0.0;
    r.normal = NORMAL_CROSS_A;

    let base = vec3<f32>(uc);
    // A ray travelling *along* a plane never meets it. Both denominators go to zero on the
    // two diagonal headings, and the guard is what keeps that from becoming an infinity that
    // survives the range test.
    var ts = vec2<f32>(-1.0, -1.0);
    if SPEC_WIND_SWAY {
        // The wind turns both diagonals into *leaning* planes: (x - s*dy) - (z - q*dy)
        // = c keeps the same diagonal family, because a shear field is still a plane --
        // so the closed-form hit stays one division, which is the whole economic
        // argument for sway on a ray marcher (there is no vertex stage to move, and a
        // pasted-on bend would buy the curve with a second trigonometry bill). kA/kB
        // are the lean in the quads' own two diagonal coordinates.
        // Ray substitution: ro + rd*t must satisfy (x - s*dy) - (z - q*dy) = c with
        // y = ro.y + rd.y*t measured from the *cell's* base (see WIND_SWAY_AMP for why
        // the anchor is per-cell). t*(rd.x - rd.z - k*rd.y) = c + k*(ro.y - base.y) -
        // (ro.x - ro.z), one division, exactly the legacy shape plus the lean terms.
        let dy = ro.y - base.y;
        let kA = shear.x - shear.y;
        let kB = shear.x + shear.y;
        let dA = rd.x - rd.z - kA * rd.y;
        if abs(dA) > 1e-6 {
            ts.x = ((base.x - base.z) + kA * dy - (ro.x - ro.z)) / dA;
        }
        let dB = rd.x + rd.z - kB * rd.y;
        if abs(dB) > 1e-6 {
            ts.y = ((base.x + base.z + 1.0) + kB * dy - (ro.x + ro.z)) / dB;
        }
    } else {
        let dA = rd.x - rd.z;
        if abs(dA) > 1e-6 {
            ts.x = ((base.x - base.z) - (ro.x - ro.z)) / dA;
        }
        let dB = rd.x + rd.z;
        if abs(dB) > 1e-6 {
            ts.y = ((base.x + base.z + 1.0) - (ro.x + ro.z)) / dB;
        }
    }
    var order = vec2<u32>(NORMAL_CROSS_A, NORMAL_CROSS_B);
    if ts.y < ts.x {
        ts = ts.yx;
        order = order.yx;
    }
    for (var i = 0u; i < 2u; i = i + 1u) {
        let tq = ts[i];
        if tq < 0.0 {
            continue;
        }
        let local = ro + rd * tq - base;
        // The planes are infinite; the quads are not. This is the clip to the voxel, and it
        // doubles as the "did the ray cross this cell at all" test.
        if any(local < vec3<f32>(-1e-4)) || any(local > vec3<f32>(1.0 + 1e-4)) {
            continue;
        }
        let uv = vec2<f32>(local.x, 1.0 - local.y);
        // The same footprint-to-mip form `shade_hit` uses, minus its grazing stretch: this
        // is choosing how *thin* the blades read at range, and a mip that thins them is the
        // point rather than a shimmer to be clamped away.
        let footprint = max(tq * vs, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y) * 16.0 / vs;
        let lod = clamp(log2(max(footprint, 1e-4)) + frame.mip_bias, 0.0, 3.0);
        // **Alpha is opacity here, not batch 12's tint mask.** `BlockDef::foliage` is what
        // says so on the Rust side; on this side it is the fact that only a foliage block
        // ever reaches this function.
        let a = textureSampleLevel(atlas, atlas_samp, uv, i32(layer), lod).a;
        if a <= hash_alpha(ro + rd * tq) {
            continue;
        }
        r.hit = true;
        r.t = tq;
        r.normal = order[i];
        return r;
    }
    return r;
}

struct Hit {
    hit: bool,
    t: f32,
    normal: u32,
    voxel: vec3<u32>,
    // Which 4^3 cell inside `voxel` the ray actually met, batch 38. **Set on every solid hit
    // and not only on a carved one**: `hit_t` reconstructs the face plane from it, and for a
    // whole cube the entry cell is 0 on the face the ray came in through and MICRO-1 on the
    // opposite one, which is what makes the micro form of that plane reduce to the cube form
    // exactly rather than approximately.
    micro: vec3<u32>,
    iters: u32,
};

// Batch 38. `block::LEAVES` and `block::PINE_LEAVES`, mirrored by hand for the same reason
// `WATER_ID` and `TALL_GRASS_ID` are, and pinned by four `const _: () = assert!` beside
// `render::FLAG_LEAF_CUTOUT`. Two of those assert the *flag* and not just the id, because a
// leaf whose `cutout` the shader does not know about is drawn as a solid cube in silence.
const LEAVES_ID: u32 = 7u;
const PINE_LEAVES_ID: u32 = 16u;
// `block::GRASS` (3) and `block::MEADOW` (19), mirrored by hand for the same reason the
// leaves pair above is, and pinned against the Rust side in tests/batch97.rs -- 97d's
// species richness colours the ground's *tops* and needs the same two names.
const GRASS_ID: u32 = 3u;
const MEADOW_ID: u32 = 19u;

fn is_cutout_id(id: u32) -> bool {
    return id == LEAVES_ID || id == PINE_LEAVES_ID;
}

// The sub-voxel grid a cutout block is carved on. **Four is not a taste**: 4^3 is 64 cells,
// which is one native `u64` and the same branching factor every other level of this tree
// uses, so the micro-march below is the arithmetic `march_chunk` already compiles rather
// than a second kind of traversal. It is also the largest grid whose coordinate fits the
// key: 2 bits an axis is 6, and six is exactly what the voxel field has spare.
const MICRO: u32 = 4u;
// Where the micro coordinate sits inside each byte of the voxel field. 64^3 needs 6 bits an
// axis and the field gives 8, so bits 6 and 7 of each byte have been free since the key was
// designed. This is what spends them.
const MICRO_SHIFT: u32 = 6u;
const VOXEL_MASK: u32 = 63u;

// How much of a leaf block is solid: `SPEC_WATER_SHADOW_DIST`'s neighbour among the overrides
// at the top of this file, because batch 43 swept it and a swept constant needs a control.
// The argument for the number is at that declaration.

// Is one micro-cell of one leaf block solid?
//
// **The key is the integer world position of the micro-cell and nothing about a ray touches
// it** -- the same rule `perm_key` follows and for the same reason: a key drawn from the float
// hit point would flip as `taa` jittered the sample, and the 3x3 neighbourhood clip would read
// a static canopy as a moving one. `wv` is the block's world voxel coordinate, which is exact
// because this only ever runs at LOD 0.
fn leaf_solid(wv: vec3<i32>, m: vec3<u32>) -> bool {
    let q = wv * i32(MICRO) + vec3<i32>(m);
    let h = hash_u32((u32(q.x) * 73856093u) ^ (u32(q.y) * 19349663u) ^ (u32(q.z) * 83492791u));
    return f32(h >> 8u) * (1.0 / 16777216.0) < SPEC_LEAF_FILL;
}

// The micro-cell a ray is standing in as it enters a voxel across `axis`.
//
// Two components come from the position and the third is snapped to the face actually
// crossed, exactly as `march_chunk` snaps its own entry cell and for exactly the same reason:
// rounding at a boundary must not start the march one cell outside the thing it just entered.
fn micro_entry(uc: vec3<u32>, ro: vec3<f32>, rd: vec3<f32>, t: f32, axis: u32,
               pos_dir: vec3<bool>, snap: bool) -> vec3<u32> {
    let p = (ro + rd * t - vec3<f32>(uc)) * f32(MICRO);
    var m = clamp(vec3<i32>(floor(p)), vec3<i32>(0), vec3<i32>(i32(MICRO) - 1));
    if snap {
        m[axis] = select(i32(MICRO) - 1, 0, pos_dir[axis]);
    }
    return vec3<u32>(m);
}

struct MicroHit {
    hit: bool,
    t: f32,
    normal: u32,
    micro: vec3<u32>,
};

// March the 4^3 micro-grid inside one leaf block. `t` comes in and goes out in the same voxel
// units the rest of `march_chunk` uses, so the caller scales it once at the end.
//
// This is the outer loop's own body at a quarter of the scale and deliberately so -- the same
// entry snap, the same "which boundary is nearest" step, the same normal formation. What it
// does *not* have is any of the hierarchy: 64 cells is small enough that a skip would cost
// more than the cells it saves, and `leaf_solid` is pure ALU with no memory behind it.
fn cutout_march(wv: vec3<i32>, uc: vec3<u32>, ro: vec3<f32>, rd: vec3<f32>, t_in: f32,
                axis_in: u32, pos_dir: vec3<bool>, snap: bool) -> MicroHit {
    var r: MicroHit;
    r.hit = false;
    r.t = t_in;
    r.normal = 0u;
    r.micro = vec3<u32>(0u);
    let base = vec3<f32>(uc);
    let inv = 1.0 / rd;
    let ms = 1.0 / f32(MICRO);
    var m = vec3<i32>(micro_entry(uc, ro, rd, t_in, axis_in, pos_dir, snap));
    var t = t_in;
    var axis = axis_in;
    // A straight line crosses at most 3*(MICRO-1)+1 cells of a MICRO^3 grid; the bound is
    // that, rounded up, and it is a guard rather than a budget.
    for (var i = 0u; i < 3u * MICRO; i = i + 1u) {
        if any(m < vec3<i32>(0)) || any(m >= vec3<i32>(i32(MICRO))) {
            return r;
        }
        if leaf_solid(wv, vec3<u32>(m)) {
            r.hit = true;
            r.t = t;
            r.normal = axis * 2u + select(1u, 0u, pos_dir[axis]);
            r.micro = vec3<u32>(m);
            return r;
        }
        let bound = vec3<f32>(select(m, m + vec3<i32>(1), pos_dir));
        let tn = (base + bound * ms - ro) * inv;
        if tn.x <= tn.y && tn.x <= tn.z {
            t = tn.x;
            axis = 0u;
        } else if tn.y <= tn.z {
            t = tn.y;
            axis = 1u;
        } else {
            t = tn.z;
            axis = 2u;
        }
        m[axis] += select(-1, 1, pos_dir[axis]);
    }
    return r;
}

// March a ray through one chunk's 4^3 tree. Origin is world space, `t_limit` world units.
// Skips whole 16^3 and 4^3 cells straight from the occupancy masks, plus the coalesced
// 2^3 test, so empty space costs very few steps.
//
// `skip_water` makes water transparent to the ray -- what the refracted and reflected rays
// want, what a shadow ray wants (a sea floor is dimmed by the light flood and the
// absorption curve, not by being told the sun never reaches it), and what the primary ray
// wants when the eye is already under the surface. It is *intersected with the chunk's own
// `ATTR_HAS_WATER` bit*, so a chunk with no water in it runs the identical instructions it
// ran before batch 8 and the whole feature costs nothing away from a shoreline.
// `skip_glass` is batch 38b's, and is exactly `skip_water` one block later: the primary ray
// passes `false` so it stops at the first pane, and **every secondary ray passes `true`**, so
// glass is invisible to a transmission, a reflection and a shadow alike. That is water's own
// answer to the same question rather than a new one -- `trace_world` has passed
// `skip_water = true` unconditionally since batch 8 -- and it is what caps the depth
// complexity at the two interfaces the research asked for without a counter anywhere: the
// primary hit is interface one, and every pane behind it is air to the ray that goes looking.
// A double-pane greenhouse therefore resolves to what is *behind* it rather than to a second
// wall, and the second pane's own Fresnel is the named cost of that.
fn march_chunk(ci: u32, ro_w: vec3<f32>, rd: vec3<f32>, t_limit: f32, skip_water: bool,
               skip_foliage: bool, skip_glass: bool, sun_ray: bool) -> Hit {
    var h: Hit;
    h.hit = false;
    h.iters = 0u;
    h.t = 0.0;
    h.normal = 0u;
    h.voxel = vec3<u32>(0u);
    h.micro = vec3<u32>(0u);
    let c = chunks[ci];
    let vs = c.voxel_size;
    let ro = (ro_w - c.origin) / vs;
    // A zero component would make the raw reciprocal infinite, and the DDA would then
    // step on 0 * inf and lose the ray. Positions and normals keep using the untouched
    // rd, so every ray with no near-zero component stays bit-identical.
    let rd_dda = select(rd, vec3<f32>(1e-6, 1e-6, 1e-6) * sign(rd + vec3<f32>(1e-9)), abs(rd) < vec3<f32>(1e-6));
    let inv = 1.0 / rd_dda;
    let bmin = (c.aabb_min - c.origin) / vs;
    let bmax = (c.aabb_max - c.origin) / vs;
    let t0 = (bmin - ro) * inv;
    let t1 = (bmax - ro) * inv;
    let tmin3 = min(t0, t1);
    let tmax3 = max(t0, t1);
    let t_enter = max(max(tmin3.x, tmin3.y), tmin3.z);
    let t_exit = min(min(tmax3.x, tmax3.y), tmax3.z);
    let t_end = min(t_exit, t_limit / vs);
    var t = max(t_enter, 0.0);
    if t >= t_end {
        return h;
    }
    var axis: u32 = 0u;
    if tmin3.x >= tmin3.y && tmin3.x >= tmin3.z {
        axis = 0u;
    } else if tmin3.y >= tmin3.z {
        axis = 1u;
    } else {
        axis = 2u;
    }
    let pos_dir = rd_dda > vec3<f32>(0.0);
    var pos = ro + rd * t;
    var cell = vec3<i32>(floor(pos));
    if t_enter > 0.0 {
        // Snap the entry axis to the face we crossed, so rounding cannot start us outside.
        cell[axis] = select(i32(ceil(bmax[axis])) - 1, i32(floor(bmin[axis])), pos_dir[axis]);
    }
    cell = clamp(cell, vec3<i32>(0), vec3<i32>(63));
    let root_mask = vec2<u32>(c.root.x, c.root.y);
    let root_ptr = c.root.z & PTR_MASK;
    // A chunk whose every voxel carries the same block id needs no lookup at all: either it
    // is entirely water, and a ray that ignores water passes straight through it, or it
    // holds none and the loop below is the identical loop it was before batch 8. (A
    // *hand-built* chunk mixing water with something else under one uniform entry cannot
    // exist -- uniform means one id -- so this is exhaustive, not a fast path.)
    var water_aware = skip_water && (c.attr_flags & ATTR_HAS_WATER) != 0u;
    if water_aware && (c.attr_flags & ATTR_UNIFORM) != 0u {
        if c.attr_base == WATER_ID {
            return h;
        }
        water_aware = false;
    }
    // Batch 14, and deliberately the same shape as the two lines above it: the marcher can
    // only tell a tuft from a cube by looking its block id up, and this flag is what keeps
    // that lookup off every chunk holding no foliage -- every coarse chunk, since ground
    // cover is LOD 0 only, plus everything underground and everything in the sky.
    //
    // `skip_foliage` is a separate question from `skip_water` and is not folded into it. A
    // shadow ray wants to pass through grass; a *reflection* wants to see it, and both of
    // those already pass `skip_water = true`. One bool answering both would make a grassy
    // bank vanish from the water reflecting it.
    let attr_uniform = (c.attr_flags & ATTR_UNIFORM) != 0u;
    // The `if` shape and not `SPEC_FOLIAGE && ...`, which is worth 384 bytes of `resolve` in
    // the shipping build: all three fold identically when the override is false (181504
    // bytes, 64 registers) but differ when it is true -- this 234240, either operand order
    // of the `&&` 234624, against 233344 for the batch-14 source with no override at all.
    var foliage_aware = false;
    if SPEC_FOLIAGE {
        foliage_aware = (c.attr_flags & ATTR_HAS_FOLIAGE) != 0u;
    }
    if foliage_aware && attr_uniform && !is_tuft(c.attr_base) {
        foliage_aware = false;
    }
    // The attribute entry has two readers now, so it is loaded for either of them.
    // Batch 38, and the same three lines a third time. **Gated to LOD 0 by `vs == 1.0`**: a
    // micro-cell inside a stride-2 voxel stands for eight blocks and means nothing, which is
    // the argument that keeps ground cover at LOD 0 as well. That gate is also what makes the
    // cliff between a porous LOD 0 canopy and a solid coarse one a real thing to measure.
    //
    // **No `skip_cutout` beside it, and that is the feature rather than an omission.** Water
    // and foliage each need a per-ray bool because a shadow ray wants to pass through them; a
    // leaf's holes are geometry, so a shadow ray meets exactly what the camera meets and the
    // canopy dapples with no ray-class plumbing anywhere.
    // **Both tests nested inside the override's own block, and the id compare spelled out
    // rather than called, and neither of those is style.** Written the way the foliage flag
    // above is -- the second `if` outside the first -- this block left **5,120 bytes** of
    // machine code in `resolve` with the constant false, measured with `shaderstats`, and the
    // frame moved 201 pixels at a max channel delta of 1 against `voxelcraft-pre38.exe`.
    // Nesting took it to 1,664 and spelling out `is_cutout_id` to 1,024. `march` folds to
    // byte-identical either way; it is the deferred pass, which reaches `march_chunk` through
    // water's secondary rays, that is fussy. Same finding as `errors.md`'s foliage entry, one
    // flag later: the constant has to reach every use, not merely the expensive one.
    var cutout_aware = false;
    if SPEC_LEAF_CUTOUT {
        cutout_aware = (c.attr_flags & ATTR_HAS_CUTOUT) != 0u && vs == 1.0;
        if cutout_aware && attr_uniform
            && c.attr_base != LEAVES_ID && c.attr_base != PINE_LEAVES_ID {
            cutout_aware = false;
        }
    }
    // Batch 38b, and the same three lines a fourth time -- nested inside the override's own
    // block for the reason batch 38 measured rather than guessed: written as
    // `SPEC_GLASS && (...)` outside an `if`, the constant reaches the cheap test and not the
    // expensive one, and `resolve` keeps the machine code with the override false. Unlike the
    // cutout there is **no `vs == 1.0` gate**: a pane is a whole voxel at every stride, so a
    // coarse glass block is as transmissive as a fine one and there is no LOD cliff here at
    // all. That is the one way this feature is cheaper than the one above it.
    var glass_aware = false;
    if SPEC_GLASS {
        glass_aware = skip_glass && (c.attr_flags & ATTR_HAS_GLASS) != 0u;
        if glass_aware && attr_uniform {
            // A chunk that is uniformly glass is nothing at all to a ray that ignores glass,
            // which is the `ATTR_UNIFORM` shortcut water takes three blocks above.
            if c.attr_base == GLASS_ID {
                return h;
            }
            glass_aware = false;
        }
    }
    var noncaster_aware = false;
    // This override is keyed by actual resident emitters, independent of RGB-light mode.
    // No-emitter worlds compile out the additional material lookup entirely.
    if SPEC_EMITTER_GATHER { noncaster_aware = sun_ray && (c.attr_flags & ATTR_HAS_EMITTER) != 0u; }
    if noncaster_aware && attr_uniform && !casts_sun_shadow(c.attr_base) { return h; }
    let water_interface = SPEC_WATER_REPAIR && !skip_water && (c.attr_flags & ATTR_HAS_WATER) != 0u;
    let attr_needed = (noncaster_aware && !attr_uniform) || (water_interface && !attr_uniform) || water_aware || glass_aware
        || ((foliage_aware || cutout_aware) && !attr_uniform);
    // Whether this ray traces blades or merely passes through them, kept as its own
    // condition so that `!skip_foliage` reaches the guard on `cross_quad` directly. Every
    // caller passing `true` passes a literal, so this folds to false when the function is
    // inlined there and the intersection -- a texture fetch and a hash -- leaves that call
    // tree entirely. Written as `foliage_aware && !skip_foliage` rather than testing
    // `skip_foliage` inside the branch precisely because the driver only removed the code
    // when the constant reached the *guard*; with the test one level in, it folded nothing
    // and `resolve` kept all 55 KB of it.
    let foliage_trace = !skip_foliage && foliage_aware;
    // Batch 53: **the mask this ray should be walking, which is not always the one the tree
    // is built from.** Water occupies its voxels, so an ocean interior is `1` at every level
    // of the occupancy tree -- and to a ray that ignores water it is empty space that costs
    // four steps per 16 blocks and an attribute load per step. `dry_mask` is `root_mask` with
    // the water-only 16^3 cells cleared, so such a cell falls into the `else` arm below and
    // takes the coalesced skip: 16 blocks in one step, or 32 when its 2x2x2 neighbourhood is
    // water too.
    //
    // **Traversal only.** `mask_below(root_mask, l1b)` still indexes the interned `inners`
    // run, which is laid out against the real mask; using this one there would read a
    // neighbouring node, and a neighbouring node is a plausible picture rather than a crash.
    //
    // `dry_mask` is a subset of `root_mask` by construction, so this can only ever admit
    // fewer cells -- and when the ray *does* see water the two are the same word and every
    // instruction below is the one it was before.
    var trav_mask = root_mask;
    if water_aware {
        trav_mask = c.dry_mask;
    }
    // Batch 72 (roadmap P10): a root-FULL chunk marches as if its tree existed. The pre-batch
    // path returned a hit at the chunk's face below for every ray; to a ray that ignores water
    // a *mixed* full chunk -- sea above sea floor, every voxel occupied, which is exactly
    // what makes the builder collapse it -- was wrong, at the footprint batch 53 measured:
    // 31,815 pixels at `coastline`, max delta 95, all belonging to `resolve`'s refracted,
    // reflected and shadow legs. And the tree under a full root is free to re-derive: the
    // occupancy is all ones at every level, and the attribute run is dense -- 64 entries
    // per 16^3 cell, `l1b * 64` -- which `leaf_attr` has always relied on and
    // `world.rs`'s `l1_node` states in Rust. So instead of returning here, the loop below
    // walks one synthetic L1 node per cell and asks it the same questions it asks a real
    // tree. A ray whose answer was right before -- it does not ignore water, or the chunk
    // has none -- still stops in its entry cell, because a full chunk is solid from its
    // face to those rays; the march is the price paid only by the legs that were wrong.
    var root_full = (c.root.z & FULL_BIT) != 0u;
    if root_full && !SPEC_FULL_MARCH && !noncaster_aware && !water_interface {
        // The pre-batch-72 path, kept bit-exact as the control (`--no-full-march`): hit the
        // face, whatever is behind it.
        h.hit = true;
        h.t = t * vs;
        h.normal = axis * 2u + select(1u, 0u, pos_dir[axis]);
        h.voxel = vec3<u32>(cell);
        h.micro = micro_entry(vec3<u32>(cell), ro, rd, t, axis, pos_dir, t > 0.0);
        return h;
    }
    // Batch 53 reads the traversal mask here too, which subsumes the `ATTR_UNIFORM` water
    // shortcut above for every chunk that is water *without* being uniform -- the common case
    // in open sea, where one chunk holds the surface, the body and nothing else.
    if trav_mask.x == 0u && trav_mask.y == 0u {
        return h;
    }
    var l1_cached: u32 = 0xFFFFFFFFu;
    var l1: vec4<u32> = vec4<u32>(0u);
    var leaf_cached: u32 = 0xFFFFFFFFu;
    var leaf: vec2<u32> = vec2<u32>(0u);
    // Attribute entry of the cached leaf, only ever loaded when this ray ignores water.
    var attr: u32 = 0u;
    loop {
        h.iters += 1u;
        if h.iters > MAX_ITERS {
            break;
        }
        if any(cell < vec3<i32>(0)) || any(cell > vec3<i32>(63)) {
            break;
        }
        let uc = vec3<u32>(cell);
        var skip: i32 = 16;
        var solid = false;
        let l1b = cell_bit(uc >> vec3<u32>(4u));
        if mask_has(trav_mask, l1b) {
            if l1b != l1_cached {
                l1_cached = l1b;
                if root_full {
                    // Batch 72: the node a full root's dense tree would hold at this cell,
                    // synthesized exactly as `leaf_attr` and `world.rs`'s `l1_node` make it.
                    // The branch must not be a `select`: the real arm's index comes from a
                    // junk `run_ptr` under FULL_BIT, and a select's operand would still load.
                    // The high nibble is roadmap P9's waterline; a synthesized node carries
                    // no tree to have measured it from, so the bound is disabled here
                    // (0xF = "no line"), and the attr readers mask it off anyway.
                    l1 = vec4<u32>(0xFFFFFFFFu, 0xFFFFFFFFu, FULL_BIT, l1b * 64u | 0xF0000000u);
                } else {
                    l1 = inners[root_ptr + mask_below(root_mask, l1b)];
                }
                leaf_cached = 0xFFFFFFFFu;
            }
            // Batch 74 (roadmap P9): the node's waterline, one level under the root's
            // dry mask. The top nibble of `l1.w` is the highest node-local y of an
            // occupied non-water voxel, or 15 for "no bound"; strictly above it every
            // voxel is water or air, and to a ray that ignores water the whole 16-row
            // of this node is one step of nothing -- the root's own 16-block march,
            // one level down. The ray this entry was written for is horizontal through
            // open sea a few blocks under the surface, which is exactly where an
            // all-water line forms; a cave under a headland keeps 15 and walks as before.
            var line_skip = false;
            if water_aware {
                let wl = l1.w >> 28u;
                let ly = f32(uc.y & 15u);
                if wl < 15u && ly > f32(wl) {
                    // **The line covers only what is strictly above it, and the skip box
                    // covers the whole y column of the node -- and the first build of this
                    // batch read those as the same claim.** The skip below advances the
                    // DDA out of a 16^3 box; a ray may leave that box through any face,
                    // and for a ray looking at the seabed that face is the bottom one.
                    // Through the middle of the box the ray has already crossed back under
                    // the line, **into the slab the line disclaims**, and the hit was never
                    // visited: measured in play as full-chunk navy water tiles -- `depth`
                    // at its maximum, `under` black, `refr` collapsed to `body` -- because
                    // the seabed sat close enough under the surface that every ray
                    // seeking it was given the skip that missed it. Sky-blue above the
                    // surface because the same skip by definition cannot cost a sky ray,
                    // and nothing pages the failure.
                    //
                    // **The fix is the transit bound, not the deletion of the feature**:
                    // the box transit's dominant-axis drop is at most `16 * |dy| / max(|dx|,
                    // |dz|)`, so the skip is sound exactly when that drop cannot carry the
                    // local y back down to the line (and trivially when the ray cannot
                    // descend at all). Arithmetic acceptance run blind: the unsafe set is
                    // 156 entry-height/slope cases, the old predicate skips 156 of them,
                    // this one skips 0 -- and keeps ~70% of the 0..15-degree rays P9 was
                    // written for. `a miss through the box's middle` is also why || is
                    // correct here and `&&` would not be: rd.y >= 0 makes the bound
                    // vacuous rather than zero.
                    line_skip = rd.y >= 0.0;
                    if !line_skip {
                        let h = max(abs(rd.x), abs(rd.z));
                        if h > 1e-6 && (ly - f32(wl)) > 16.0 * (abs(rd.y) / h) {
                            line_skip = true;
                        }
                    }
                }
            }
            if line_skip {
                skip = 16;
            } else if (l1.z & FULL_BIT) != 0u {
                solid = true;
                if attr_needed {
                    let lfb = cell_bit((uc >> vec3<u32>(2u)) & vec3<u32>(3u));
                    if lfb != leaf_cached {
                        leaf_cached = lfb;
                        attr = bricks[c.attr_base + (l1.w & 0x0FFFFFFFu) + lfb];
                    }
                }
            } else {
                let l1m = vec2<u32>(l1.x, l1.y);
                let lfb = cell_bit((uc >> vec3<u32>(2u)) & vec3<u32>(3u));
                if mask_has(l1m, lfb) {
                    if lfb != leaf_cached {
                        leaf_cached = lfb;
                        let li = mask_below(l1m, lfb);
                        leaf = leaves[(l1.z & PTR_MASK) + li];
                        if attr_needed {
                            attr = bricks[c.attr_base + (l1.w & 0x0FFFFFFFu) + li];
                        }
                    }
                    let vb = cell_bit(uc & vec3<u32>(3u));
                    if mask_has(leaf, vb) {
                        solid = true;
                    } else {
                        // Coalesced 2^3 test: one shift and mask proves an empty 2x2x2.
                        let sh = vb & 42u;
                        var lo: u32;
                        if sh == 0u {
                            lo = leaf.x;
                        } else if sh < 32u {
                            lo = (leaf.x >> sh) | (leaf.y << (32u - sh));
                        } else {
                            lo = leaf.y >> (sh - 32u);
                        }
                        skip = select(1, 2, (lo & 0x00330033u) == 0u);
                    }
                } else {
                    skip = 4;
                }
            }
        } else {
            // Batch 53: the coalesced test this function has run at the **leaf** level since
            // the tree was written, lifted to the root, where nothing had ever looked.
            //
            // `cell_bit` is `x | y<<2 | z<<4` at both levels and the root mask is the same
            // 64-bit `vec2<u32>` a leaf is, so this is the leaf's own four lines with nothing
            // changed but which mask they read: `l1b & 42u` clears the low bit of each 2-bit
            // axis field to give the 2-aligned base, the shift brings that 2x2x2 group of
            // 16^3 cells to the bottom of the word, and `0x00330033` is where its eight bits
            // land. One shift, one or, one and.
            //
            // **What it is worth is the whole reason the ceiling was priced first.** A ray in
            // open air steps 16 blocks at a time because the root is the top level of a
            // three-level 4^3 tree and 16 is the coarsest cell it has -- and batch 53 measured
            // that cells with no root bit hold **62% of `default`'s DDA steps**, by building
            // an incorrect `skip = 64` and counting. This is the exact half of that: an empty
            // 32^3 costs one step instead of two to eight.
            //
            // There is no rung above it. A 4x4x4 group is the whole root mask, and
            // `root_mask == 0` returns before the loop starts.
            let sh = l1b & 42u;
            var rlo: u32;
            if sh == 0u {
                rlo = trav_mask.x;
            } else if sh < 32u {
                rlo = (trav_mask.x >> sh) | (trav_mask.y << (32u - sh));
            } else {
                rlo = trav_mask.y >> (sh - 32u);
            }
            skip = select(16, 32, (rlo & 0x00330033u) == 0u);
        }
        if solid && noncaster_aware {
            var id = c.attr_base;
            if !attr_uniform { id = leaf_block(attr, uc); }
            if !casts_sun_shadow(id) {
                solid = false;
                skip = 1;
            }
        }
        if solid && water_interface {
            var id = c.attr_base;
            if !attr_uniform { id = leaf_block(attr, uc); }
            if id == WATER_ID {
                let nid = axis * 2u + select(1u, 0u, pos_dir[axis]);
                let outward = normal_of(nid);
                let boundary = ro_w + rd * (t * vs);
                // Preserve exposed side faces; only water on BOTH sides is invisible.
                if world_block_at(boundary + outward * (0.002 * vs)) == WATER_ID {
                    solid = false;
                    skip = 1;
                }
            }
        }
        if solid && water_aware {
            if leaf_block(attr, uc) == WATER_ID {
                solid = false;
                // A leaf that is uniformly water is four voxels of nothing to this ray, so
                // step the whole cell. That is the difference between crossing an ocean
                // interior in a handful of iterations and crossing it one voxel at a time.
                skip = select(1, 4, (attr & UNIFORM_BIT) != 0u);
            }
        }
        // Batch 38b, and deliberately the water arm above it written twice rather than the two
        // folded into one id-set test: `water_aware` and `glass_aware` are different rays'
        // questions -- the primary ray underwater ignores water and still stops at a pane --
        // and a single `id == WATER_ID || id == GLASS_ID` would have to be guarded by an `or`
        // of the two flags, which is the shape that costs the fold. Kept separate, a chunk
        // with no glass in it never evaluates this at all.
        if solid && glass_aware {
            if leaf_block(attr, uc) == GLASS_ID {
                solid = false;
                skip = select(1, 4, (attr & UNIFORM_BIT) != 0u);
            }
        }
        if solid && foliage_aware {
            var bid = c.attr_base;
            if !attr_uniform {
                bid = leaf_block(attr, uc);
            }
            if is_tuft(bid) {
                // A tuft is *air* until a blade is actually hit, and to a ray that does not
                // trace blades it stays air. Getting that backwards is what turns grass into
                // a blocky shadow under a shadow ray and into a green box on the bank of a
                // reflection -- which is what the first cut of this did.
                //
                // One cell of step and never the coalesced skip: the 2^3 test above was
                // answered before the cutout was known, and the neighbours it cleared are
                // ordinary geometry this ray still has to meet.
                solid = false;
                skip = 1;
                if foliage_trace {
                    // Both cross-quad states index the same atlas layer -- one blade
                    // texture on two planes -- so either entry of the widened table does.
                    // 101f: the gust field is sampled at the voxel's own world column.
                    // **The clock is the clouds' own** (`frame.cloud_speed`, and pinned
                    // as such by tests/textures' census): the sway is the weather the
                    // clouds already ride, so a headless capture's animation knob
                    // freezes grass together with cloud and sea -- batch 23's rule that
                    // every animation answers to the one time hand a capture can set,
                    // rather than growing the grass its own clock.
                    var sway = vec2<f32>(0.0, 0.0);
                    if SPEC_WIND_SWAY {
                        sway = wind_shear(
                            f32(c.origin.x) + f32(uc.x) * vs,
                            f32(c.origin.z) + f32(uc.z) * vs,
                            frame.time * frame.cloud_speed,
                        );
                    }
                    let q = cross_quad(uc, ro, rd, face_layer(block_faces[bid * 8u + NORMAL_CROSS_A]), vs, sway);
                    if q.hit {
                        h.hit = true;
                        h.t = q.t * vs;
                        h.normal = q.normal;
                        h.voxel = uc;
                        return h;
                    }
                }
            }
        }
        if solid && cutout_aware {
            var bid = c.attr_base;
            if !attr_uniform {
                bid = leaf_block(attr, uc);
            }
            if is_cutout_id(bid) {
                // `t > 0.0` is the same test the entry snap above makes, and it is false only
                // in the cell the camera is standing in -- the one cell reached without
                // crossing a face.
                let wv = vec3<i32>(c.origin) + vec3<i32>(uc);
                let q = cutout_march(wv, uc, ro, rd, t, axis, pos_dir, t > 0.0);
                if q.hit {
                    h.hit = true;
                    h.t = q.t * vs;
                    h.normal = q.normal;
                    h.voxel = uc;
                    h.micro = q.micro;
                    return h;
                }
                // Every micro-cell along this ray's path through the block was a hole, so the
                // block is air to it. One cell of step and never the coalesced skip, for the
                // reason the tuft above does not take one either: the 2^3 test was answered
                // before the carving was known.
                solid = false;
                skip = 1;
            }
        }
        if solid {
            h.hit = true;
            h.t = t * vs;
            h.normal = axis * 2u + select(1u, 0u, pos_dir[axis]);
            h.voxel = uc;
            h.micro = micro_entry(uc, ro, rd, t, axis, pos_dir, t > 0.0);
            return h;
        }
        let bmin_i = cell - (cell & vec3<i32>(skip - 1));
        let bmax_i = bmin_i + vec3<i32>(skip);
        let bounds = select(vec3<f32>(bmin_i), vec3<f32>(bmax_i), pos_dir);
        let tn = (bounds - ro) * inv;
        if tn.x <= tn.y && tn.x <= tn.z {
            t = tn.x;
            axis = 0u;
        } else if tn.y <= tn.z {
            t = tn.y;
            axis = 1u;
        } else {
            t = tn.z;
            axis = 2u;
        }
        if t >= t_end {
            break;
        }
        pos = ro + rd * t;
        cell = vec3<i32>(floor(pos));
        cell[axis] = select(bmin_i[axis] - 1, bmax_i[axis], pos_dir[axis]);
    }
    return h;
}

// Does a local voxel of a chunk block light? Out-of-range coordinates read as empty.
//
// Water occupies its voxel but does not occlude: the light flood runs through it and the
// smooth-lighting gather has to see it as open, or every face of a sea floor would count
// three water neighbours, land on the fully-occluded corner and read as black under a lit
// surface. The block lookup that separates the two only runs on chunks that hold water.
fn voxel_occupied(ci: u32, v: vec3<i32>) -> bool {
    if any(v < vec3<i32>(0)) || any(v > vec3<i32>(63)) {
        return false;
    }
    let c = chunks[ci];
    let uv = vec3<u32>(v);
    let root_mask = vec2<u32>(c.root.x, c.root.y);
    let l1b = cell_bit(uv >> vec3<u32>(4u));
    if !mask_has(root_mask, l1b) {
        return false;
    }
    var occupied = false;
    if (c.root.z & FULL_BIT) != 0u {
        occupied = true;
    } else {
        let l1 = inners[(c.root.z & PTR_MASK) + mask_below(root_mask, l1b)];
        if (l1.z & FULL_BIT) != 0u {
            occupied = true;
        } else {
            let l1m = vec2<u32>(l1.x, l1.y);
            let lfb = cell_bit((uv >> vec3<u32>(2u)) & vec3<u32>(3u));
            if mask_has(l1m, lfb) {
                let leaf = leaves[(l1.z & PTR_MASK) + mask_below(l1m, lfb)];
                occupied = mask_has(leaf, cell_bit(uv & vec3<u32>(3u)));
            }
        }
    }
    return occupied;
}

fn voxel_occludes(ci: u32, v: vec3<i32>) -> bool {
    if any(v < vec3<i32>(0)) || any(v > vec3<i32>(63)) {
        return false;
    }
    let c = chunks[ci];
    let uv = vec3<u32>(v);
    let root_mask = vec2<u32>(c.root.x, c.root.y);
    let l1b = cell_bit(uv >> vec3<u32>(4u));
    if !mask_has(root_mask, l1b) {
        return false;
    }
    var occupied = false;
    if (c.root.z & FULL_BIT) != 0u {
        occupied = true;
    } else {
        let l1 = inners[(c.root.z & PTR_MASK) + mask_below(root_mask, l1b)];
        if (l1.z & FULL_BIT) != 0u {
            occupied = true;
        } else {
            let l1m = vec2<u32>(l1.x, l1.y);
            let lfb = cell_bit((uv >> vec3<u32>(2u)) & vec3<u32>(3u));
            if mask_has(l1m, lfb) {
                let leaf = leaves[(l1.z & PTR_MASK) + mask_below(l1m, lfb)];
                occupied = mask_has(leaf, cell_bit(uv & vec3<u32>(3u)));
            }
        }
    }
    // Neither water nor a grass tuft occludes. Water because the flood runs through it, and
    // foliage for the same reason plus a second one: a tuft stands in the air voxel directly
    // above the ground, so counting it as an occluder would put an AO shadow under every
    // blade in the world and darken the ground the batch exists to decorate. One lookup
    // answers both, and the flags keep it off every chunk that has neither.
    if occupied && (c.attr_flags & (ATTR_HAS_WATER | ATTR_HAS_FOLIAGE)) != 0u {
        let b = block_at(ci, uv);
        return b != WATER_ID && !is_tuft(b);
    }
    return occupied;
}


// ---- the packed light cell, which is two bytes since batch 59 ----
//
// `sky:4 | r:4 | g:4 | b:4`, sky at the top. Before roadmap R4 it was one byte, `sky:4 |
// block:4`, and the widening is what lets two lamps of different colours *mix* -- a per-block
// tint applied at read time cannot, because the cell between them holds one number and no
// memory of which block put it there.
//
// **These four accessors are the only way in, for `face_layer`'s reason.** A raw cell used as
// a level indexes nothing and silently reads as an absurdly bright surface rather than as
// garbage, which is the failure mode that looks like a look change. `light::pack` in Rust is
// the other copy of this layout and `tests/light.rs` pins the pair.
const LIGHT_OPEN_SKY: u32 = 0xF000u;

fn light_sky(packed: u32) -> f32 {
    return f32(packed >> 12u);
}

// The three block channels as levels, which `resolve` runs through `light_curve` per channel.
fn light_block_rgb(packed: u32) -> vec3<f32> {
    return vec3<f32>(
        f32((packed >> 8u) & 0xFu),
        f32((packed >> 4u) & 0xFu),
        f32(packed & 0xFu),
    );
}

// The pre-59 scalar block level. Every channel of an emitter decrements by one per flood step,
// so the largest of the three is the largest source level minus the same distance -- which is
// what the single pre-59 flood held. `--no-light-rgb` reads this and gets the old frame to the
// bit; `light::block_level` is the Rust copy of the same claim.
fn light_block_level(packed: u32) -> f32 {
    let c = light_block_rgb(packed);
    return max(c.r, max(c.g, c.b));
}

// Packed light cell (sky:4 | r:4 | g:4 | b:4) for a local voxel.
fn light_at(ci: u32, v: vec3<i32>) -> u32 {
    let c = chunks[ci];
    if (c.light_flags & 1u) != 0u {
        return c.light_base;
    }
    if any(v < vec3<i32>(0)) || any(v > vec3<i32>(63)) {
        return LIGHT_OPEN_SKY;
    }
    let uv = vec3<u32>(v);
    let cell = (uv.x >> 2u) | ((uv.y >> 2u) << 4u) | ((uv.z >> 2u) << 8u);
    let entry = bricks[c.light_base + cell];
    if (entry & UNIFORM_BIT) != 0u {
        return entry & 0xFFFFu;
    }
    let vb = cell_bit(uv & vec3<u32>(3u));
    let w = bricks[entry + (vb >> 1u)];
    return (w >> ((vb & 1u) * 16u)) & 0xFFFFu;
}

// ---- world-space lookups through the chunk grid (LOD-0 sized cells) ----

fn grid_lookup(p: vec3<f32>) -> u32 {
    let cc = vec3<i32>(floor(p / 64.0)) - frame.grid_min;
    if any(cc < vec3<i32>(0)) || any(cc >= vec3<i32>(frame.grid_dim)) {
        return NO_CHUNK;
    }
    let u = vec3<u32>(cc);
    return grid[u.x + u.y * frame.grid_dim.x + u.z * frame.grid_dim.x * frame.grid_dim.y];
}

// Light byte and solidity of one point, in one grid walk. Smooth lighting wants both for
// every cell it reads, and the walk is the expensive half — which is why `resolve` only
// comes here for a neighbourhood that crosses out of the chunk being shaded, and takes
// `gather_face` below for the rest. Separate `world_light` and `world_solid` entry points
// used to serve the flat lighting; nothing calls them now. Water reads as *open* here, for
// the reason `voxel_occludes` gives.
fn world_sample(p: vec3<f32>) -> vec2<u32> {
    let ci = grid_lookup(p);
    if ci == NO_CHUNK {
        return vec2<u32>(LIGHT_OPEN_SKY, 0u);
    }
    let c = chunks[ci];
    let v = vec3<i32>(floor((p - c.origin) / c.voxel_size));
    return vec2<u32>(light_at(ci, v), select(0u, 1u, voxel_occludes(ci, v)));
}

// The nine cells of one 3x3 plane inside a single chunk, as (light byte, solid) — the
// neighbourhood smooth lighting reads around a face. Nine `world_sample` calls would walk
// from the root nine times, where in this order the cells move one step at a time and a
// 4^3 leaf covers four of them, so the L1 node, the leaf and the light brick are each
// usually the one already in hand. Every cell must lie inside the chunk; `resolve` tests
// that before choosing this path over the world-space one.
fn gather_face(ci: u32, base: vec3<i32>, ta: vec3<i32>, tb: vec3<i32>,
               out: ptr<function, array<vec2<u32>, 9>>) {
    let c = chunks[ci];
    let root_mask = vec2<u32>(c.root.x, c.root.y);
    let root_full = (c.root.z & FULL_BIT) != 0u;
    let root_ptr = c.root.z & PTR_MASK;
    let light_uniform = (c.light_flags & 1u) != 0u;
    let has_water = (c.attr_flags & ATTR_HAS_WATER) != 0u;
    var l1_key: u32 = 0xFFFFFFFFu;
    var l1: vec4<u32> = vec4<u32>(0u);
    var leaf_key: u32 = 0xFFFFFFFFu;
    var leaf: vec2<u32> = vec2<u32>(0u);
    var brick_key: u32 = 0xFFFFFFFFu;
    var brick: u32 = 0u;
    for (var i = 0u; i < 9u; i = i + 1u) {
        let uv = vec3<u32>(base + ta * (i32(i % 3u) - 1) + tb * (i32(i / 3u) - 1));

        var solid = false;
        let l1b = cell_bit(uv >> vec3<u32>(4u));
        if mask_has(root_mask, l1b) {
            if root_full {
                solid = true;
            } else {
                if l1b != l1_key {
                    l1_key = l1b;
                    l1 = inners[root_ptr + mask_below(root_mask, l1b)];
                    leaf_key = 0xFFFFFFFFu;
                }
                let l1m = vec2<u32>(l1.x, l1.y);
                let lfb = cell_bit((uv >> vec3<u32>(2u)) & vec3<u32>(3u));
                if (l1.z & FULL_BIT) != 0u {
                    solid = true;
                } else if mask_has(l1m, lfb) {
                    if lfb != leaf_key {
                        leaf_key = lfb;
                        leaf = leaves[(l1.z & PTR_MASK) + mask_below(l1m, lfb)];
                    }
                    solid = mask_has(leaf, cell_bit(uv & vec3<u32>(3u)));
                }
            }
        }
        // Water fills its voxel but does not occlude; see `voxel_occludes`. The lookup is
        // gated on the chunk holding water at all, so the nine cells of a face anywhere
        // inland cost exactly what they cost before batch 8.
        if solid && has_water {
            solid = block_at(ci, uv) != WATER_ID;
        }

        var packed = c.light_base;
        if !light_uniform {
            let cell = (uv.x >> 2u) | ((uv.y >> 2u) << 4u) | ((uv.z >> 2u) << 8u);
            if cell != brick_key {
                brick_key = cell;
                brick = bricks[c.light_base + cell];
            }
            if (brick & UNIFORM_BIT) != 0u {
                packed = brick & 0xFFFFu;
            } else {
                let vb = cell_bit(uv & vec3<u32>(3u));
                packed = (bricks[brick + (vb >> 1u)] >> ((vb & 1u) * 16u)) & 0xFFFFu;
            }
        }
        (*out)[i] = vec2<u32>(packed, select(0u, 1u, solid));
    }
}

// Any-hit ray through the chunk grid; used for sun shadows.
fn shadow_ray(ro: vec3<f32>, rd: vec3<f32>, max_dist: f32) -> bool {
    var r_ro = ro;
    var r_dist = max_dist;
    // Water surface repairs: a ray starting underwater (sea floor) skips submerged occluders.
    // Water depth and absorption already darken the seabed; only occluders above the waterline
    // (trees, cliffs on the shore) cast shadows down into water, preventing dark triangular spikes
    // from submerged 1-block steps in the shallows.
    if SPEC_WATER_REPAIR && rd.y > 1e-4 && ro.y < frame.sea_level {
        let t_surf = (frame.sea_level - ro.y) / rd.y;
        if t_surf >= r_dist {
            return false;
        }
        r_ro = ro + rd * t_surf;
        r_dist = r_dist - t_surf;
    }
    // A zero component would make the raw reciprocal infinite, and the DDA would then
    // step on 0 * inf and lose the ray. Positions and normals keep using the untouched
    // rd, so every ray with no near-zero component stays bit-identical.
    let rd_dda = select(rd, vec3<f32>(1e-6, 1e-6, 1e-6) * sign(rd + vec3<f32>(1e-9)), abs(rd) < vec3<f32>(1e-6));
    let inv = 1.0 / rd_dda;
    let pos_dir = rd_dda > vec3<f32>(0.0);
    var cell = vec3<i32>(floor(r_ro / 64.0));
    var t = 0.0;
    var last: u32 = NO_CHUNK;
    for (var i = 0u; i < 24u; i = i + 1u) {
        let rel = cell - frame.grid_min;
        if any(rel < vec3<i32>(0)) || any(rel >= vec3<i32>(frame.grid_dim)) {
            return false;
        }
        let u = vec3<u32>(rel);
        let ci = grid[u.x + u.y * frame.grid_dim.x + u.z * frame.grid_dim.x * frame.grid_dim.y];
        if ci != NO_CHUNK && ci != last {
            last = ci;
            // Water does not cast a shadow. A sea floor is darkened by the sky-light flood
            // attenuating with depth and by the absorption curve on the way back to the
            // eye; telling it the sun never arrives at all would count the same darkening
            // a third time, and a shallow bottom would read like a cave floor. It costs
            // real time -- see `SPEC_WATER_SHADOW_DIST`, which is what bounds it. Batch 40
            // measured the bounded ray at **4.100 ms of a 13.080 ms coastal `resolve`** with
            // that bound at 48, and batch 41 took the bound to 16 for 1.68 of them.
            //
            // **Glass does not cast one either, and that is batch 38b's third `true`.** It is
            // the same argument reaching a different conclusion: a shadow ray is one bit, so
            // the only two answers available are "full shadow" and "none", and a pane that
            // transmits most of what arrives is far better described by the second. It also
            // has to agree with `BlockDef::opaque`, which this batch set to false -- a glass
            // house lit by the sky flood and then blacked out by its own roof's shadow ray
            // would be lit by two rules that disagree.
            if march_chunk(ci, r_ro, rd, r_dist, true, true, true, true).hit {
                return true;
            }
        }
        // Step to the next 64-block cell.
        let bounds = vec3<f32>(select(cell, cell + vec3<i32>(1), pos_dir)) * 64.0;
        let tn = (bounds - r_ro) * inv;
        var axis = 0u;
        if tn.x <= tn.y && tn.x <= tn.z {
            t = tn.x;
        } else if tn.y <= tn.z {
            t = tn.y;
            axis = 1u;
        } else {
            t = tn.z;
            axis = 2u;
        }
        if t >= r_dist {
            return false;
        }
        cell[axis] += select(-1, 1, pos_dir[axis]);
    }
    return false;
}

// ---- sun disc (batch 102a, `--soft-shadows`) ----

// **The sun is a disc, and the disc's one ground phenomenon is the penumbra** -- what
// the references have and this engine's every tree shadow lacks, because `shadow_ray`
// answers lit-or-not and nothing between. The cone picture inverts the usual PCF bill:
// instead of a screen-space kernel of neighbour samples (a second pass this renderer
// has no buffer for), the extra taps here are *rays through the same grid*, and their
// cost is bounded by geometry rather than by the framebuffer. The centre tap keeps the
// old verdict verbatim; the offset taps march only a little past the blocker the
// centre tap found, so a deep-umbra pixel early-outs on every ray and a contact pixel
// pays for a cone tight against its own stem. The fringe is the only payer.
//
// The primitive worth the batch's name is the blocker *distance*, which the one-bit
// `shadow_ray` throws away. Hence a sibling, not a rewrite: **the batch-101 lesson is
// that the control's bit-exactness is a construction property**, true only while the
// unarmed module compiles from text this batch did not open -- so `shadow_ray` above
// keeps its body to the character, and the distance-returning walk is transcribed
// beside it (the same shape `harness/vantage.rs` takes when it transliterates the
// renderer instead of sharing with it).

/// The tap cone's half-angle as a multiple of the sun's true angular radius
/// ([`SUN_ANGLE`]). 6 is authored against a 20-block tree: a ~1.1-block fringe under
/// it, ~0.3 under a tuft-height stem -- contact hardening for free, since the gap
/// *is* the occluder distance. **Measured at batch 102's hardware round, the whole
/// ladder, and the verdict is there is no sweet spot**: 6/10/24/48 give whole-frame
/// MAE 0.180/0.250/0.398/0.546 with max delta pinned at 26 -- no setting is both
/// visible and honest, so do not tune this; G5's gated redesign changes *where* the
/// cone fires, not how wide it is.
override SOFT_PENUMBRA: f32 = 1.5;

/// Offset taps beyond the centre verdict. 3 at golden-angle phases plus the centre
/// give five visibility levels (0, .25, .5, .75, 1) across the fringe, rotated
/// per-surface so the steps read as grain rather than bands. **0 is the one-constant
/// revert** (with the arm still armed: the fold below returns the centre bit and the
/// loop never compiles a tap), each +1 is one third of the arm's cost again.
/// **Measured: the arm's cost *is* this number -- scaffolding 0.073 ms, first tap
/// +1.824, taps two and three +1.33 each, and the march length of a tap is noise
/// against the fire.** A ray's bill is that it was fired; the lesson lives in
/// `docs/lessons.md` and G5's gate is its application.
override SOFT_TAPS: u32 = 3u;

/// March budget for the offset taps when the centre tap found open sky -- **the one
/// number this batch's cost story *was authored* to rest on, and measured not to.**
/// A hit tells us where the blocker is and the cone budgets against it; a miss gives
/// no such anchor, and three more taps launched to `max_dist` over open meadow was the
/// profile the design feared. **The hardware round cut it 64 -> 8 and saved 7%**: the
/// miss branch's length is not the bill, the tap existing is. Kept at 64 as the honest
/// default -- the cap still costs nothing to state, and the G5 gate may make the miss
/// branch rare enough that its budget stops mattering at all. Revert: 220.0 pays the
/// full march and must show pixels to justify itself.
const SOFT_MISS_DIST: f32 = 64.0;

// `shadow_ray`, returning the blocker distance instead of the one bit: **a
// transliteration, not a shared body** -- see the header above. -1.0 means open sky
// to `max_dist`. **Measured free: the whole arm's scaffolding, this walk included, is
// 0.073 ms of a +4.55 ms round -- the taps are the bill, not the bookkeeping, and this
// function is exactly what G5's boundary gate reads instead of firing them.** The per-cell march is handed the same `max_dist` (never the cell
// exit) for `trace_world`'s documented reason: a coarse chunk's AABB may reach past
// the cell it was entered from, so the returned distance can overshoot the true
// nearest blocker by a cell chord. That is exact enough for a *budget*: the cone
// margin below (2x the aperture chord) absorbs it.
fn shadow_ray_t(ro: vec3<f32>, rd: vec3<f32>, max_dist: f32) -> f32 {
    // A zero component would make the raw reciprocal infinite, and the DDA would then
    // step on 0 * inf and lose the ray. Positions and normals keep using the untouched
    // rd, so every ray with no near-zero component stays bit-identical.
    let rd_dda = select(rd, vec3<f32>(1e-6, 1e-6, 1e-6) * sign(rd + vec3<f32>(1e-9)), abs(rd) < vec3<f32>(1e-6));
    let inv = 1.0 / rd_dda;
    let pos_dir = rd_dda > vec3<f32>(0.0);
    var cell = vec3<i32>(floor(ro / 64.0));
    var t = 0.0;
    var last: u32 = NO_CHUNK;
    for (var i = 0u; i < 24u; i = i + 1u) {
        let rel = cell - frame.grid_min;
        if any(rel < vec3<i32>(0)) || any(rel >= vec3<i32>(frame.grid_dim)) {
            return -1.0;
        }
        let u = vec3<u32>(rel);
        let ci = grid[u.x + u.y * frame.grid_dim.x + u.z * frame.grid_dim.x * frame.grid_dim.y];
        if ci != NO_CHUNK && ci != last {
            last = ci;
            // Water, foliage and glass cast no shadow, exactly as `shadow_ray` reads
            // them -- the penumbra must soften the same shadow, not a different one.
            let m = march_chunk(ci, ro, rd, max_dist, true, true, true, true);
            if m.hit {
                return m.t;
            }
        }
        let bounds = vec3<f32>(select(cell, cell + vec3<i32>(1), pos_dir)) * 64.0;
        let tn = (bounds - ro) * inv;
        var axis = 0u;
        if tn.x <= tn.y && tn.x <= tn.z {
            t = tn.x;
        } else if tn.y <= tn.z {
            t = tn.y;
            axis = 1u;
        } else {
            t = tn.z;
            axis = 2u;
        }
        if t >= max_dist {
            return -1.0;
        }
        cell[axis] += select(-1, 1, pos_dir[axis]);
    }
    return -1.0;
}

// The penumbral verdict: the centre bit plus `SOFT_TAPS` cone taps, evenly weighted.
// The rotation hash is taken from the *surface*, quantised to half-blocks on
// `hash_alpha`'s own lattice family (same primes, same negative-coordinate wrap), so
// the fringe grain belongs to the ground: it does not crawl under a moving camera,
// and it asks nothing of the TAA accumulation the probe suite pins the use of.


// ---- water ----

struct WorldHit {
    hit: bool,
    t: f32,
    normal: u32,
    voxel: vec3<u32>,
    ci: u32,
};

// Closest-hit ray through the chunk grid with water transparent. This is the second march,
// and it is the whole of batch 8b's answer: the visibility buffer holds exactly one opaque
// hit per pixel by construction -- `atomicMax` on a 64-bit key *is* the depth test -- so
// the radiance behind the water surface has to come from somewhere else. A second
// visibility layer would mean a second 64-bit buffer, a second atomic per hit and a second
// full march dispatch, all paid on every pixel of every frame for a feature that touches
// the few per cent of them where water is visible. This runs from `resolve` instead, so it
// costs exactly nothing on the pixels that never take it -- and the reflection in 8c is
// then the same function pointed the other way.
//
// Each grid cell limits its own march to that cell's exit, so the first hit found really is
// the nearest. `build_chunk_list` fills the grid coarse-first and lets finer chunks
// overwrite, so a coarse chunk's AABB can reach into cells that belong to its own finer
// children; an unlimited march of it would report a surface those children stand in front
// of. The cost is that a chunk spanning several cells is entered once per cell, which over
// the tens of blocks these rays travel is one or two extra entries.
// `skip_water` was a hard-coded `true` inside this function from batch 8 until batch 38b's
// follow-up, and it was never a property of *tracing the world* -- it was water's own policy,
// correct for all three callers water had. Glass is the first caller that wants the opposite,
// and while the literal sat here the sea was **invisible through a pane**: the transmitted ray
// passed straight through the surface and came back with the sea floor, unlit and with no
// medium over it, so a window onto a bay showed a dark seabed where the bay should be.
//
// The lesson is the one `march_chunk`'s own `skip_water` comment nearly states: a policy
// belongs to the ray, not to the traversal, and a default that is right for every caller you
// have is still a default. It stayed invisible because every fixture vantage holding water
// holds no glass and the one holding glass holds no water -- the user found it in the window.
fn trace_world(ro: vec3<f32>, rd: vec3<f32>, max_dist: f32, skip_water: bool) -> WorldHit {
    var out: WorldHit;
    out.hit = false;
    out.t = max_dist;
    out.normal = 0u;
    out.voxel = vec3<u32>(0u);
    out.ci = NO_CHUNK;
    let tiny = abs(rd) < vec3<f32>(1e-6);
    let dir = select(rd, vec3<f32>(1e-6, 1e-6, 1e-6) * sign(rd + vec3<f32>(1e-9)), tiny);
    let inv = 1.0 / dir;
    let pos_dir = dir > vec3<f32>(0.0);
    var cell = vec3<i32>(floor(ro / 64.0));
    for (var i = 0u; i < 16u; i = i + 1u) {
        let rel = cell - frame.grid_min;
        if any(rel < vec3<i32>(0)) || any(rel >= vec3<i32>(frame.grid_dim)) {
            return out;
        }
        let u = vec3<u32>(rel);
        let ci = grid[u.x + u.y * frame.grid_dim.x + u.z * frame.grid_dim.x * frame.grid_dim.y];
        let bounds = vec3<f32>(select(cell, cell + vec3<i32>(1), pos_dir)) * 64.0;
        let tn = (bounds - ro) * inv;
        var t_exit = tn.x;
        var axis = 0u;
        if tn.y < t_exit {
            t_exit = tn.y;
            axis = 1u;
        }
        if tn.z < t_exit {
            t_exit = tn.z;
            axis = 2u;
        }
        if ci != NO_CHUNK {
            let h = march_chunk(ci, ro, rd, min(t_exit, max_dist), skip_water, false, true, false);
            if h.hit {
                out.hit = true;
                out.t = h.t;
                out.normal = h.normal;
                out.voxel = h.voxel;
                out.ci = ci;
                return out;
            }
        }
        if t_exit >= max_dist {
            return out;
        }
        cell[axis] += select(-1, 1, pos_dir[axis]);
    }
    return out;
}

// Extinction of water per block, per channel, in linear radiance -- roughly the measured
// coefficients for clear sea water. Red is gone within a few blocks and blue survives tens,
// which is the whole reason a shallow sandy bottom reads sandy and a deep one reads blue.
//
// It is applied to the *view* leg only. The downward leg -- sunlight reaching the sea floor
// in the first place -- is already carried by the sky-light flood, which loses a level per
// block of water in `light.rs`. Splitting the two that way is what keeps the chromatic part
// on the GPU, where it can be a per-channel exponential, and the scalar part in the fifteen
// integer levels that are all the light bricks can hold.
const WATER_EXT: vec3<f32> = vec3<f32>(0.280, 0.055, 0.020);
const WATER_IOR: f32 = 1.333;

// Ray-origin nudge off a surface the next march must not re-hit. **One value for both
// files now**: batch 91 moved it up from `resolve.wgsl` when `march.wgsl` gained a second
// segment that starts on the sea plane (`SPEC_SNELL_BEND`), and the constant's contract
// is unchanged -- large enough to escape a voxel face a float could still call inside,
// small enough to never be the reason a ray finds a different surface.
const SURFACE_EPS: f32 = 0.02;
// Water's normal-incidence reflectance, ((n-1)/(n+1))^2 for n = 1.333. It is small, which
// is why a lake looks through at your feet and mirrors at the horizon: the Schlick term
// climbs to 1 at grazing angles no matter how small this is.
const WATER_F0: f32 = 0.02;

// Beer-Lambert toward the medium's own in-scattered colour, rather than toward black. A
// pure `exp()` would make deep water read as a hole in the world; the body colour is what
// it converges to instead, and it is the reason the deep parts still look like water.
fn water_medium(c: vec3<f32>, d: f32, body: vec3<f32>) -> vec3<f32> {
    let tr = exp(-WATER_EXT * frame.water_absorb * max(d, 0.0));
    return c * tr + body * (vec3<f32>(1.0) - tr);
}

fn schlick(cos_i: f32) -> f32 {
    let m = clamp(1.0 - cos_i, 0.0, 1.0);
    let m2 = m * m;
    return WATER_F0 + (1.0 - WATER_F0) * m2 * m2 * m;
}

// Normal-incidence reflectance of soda-lime glass: `((n-1)/(n+1))^2` at **n = 1.52**, the index
// everything from a window to a bottle sits within a per cent of. Double water's 0.02, which is
// most of why a pane catches the sky at angles a lake still looks through -- though the *shape*
// of both curves is the same and both reach 1 at grazing.
//
// **The index itself is not a constant here, because nothing bends by it.** A `GLASS_IOR` was
// declared in the first version of batch 38b and fed `refract()` in `shade_glass`, which put a
// double image inside every glasshouse -- a slab refracts twice and the halves cancel, and the
// transmitted ray never finds the exit face to cancel against. The reflectance survives that
// correction because `F0` depends on the index and not on the path; the bend does not. The
// reasoning is at the transmission in `shade_glass`, and re-deriving this number needs only the
// 1.52 in the line above.
const GLASS_F0: f32 = 0.0426;

// Schlick again, at glass's F0 rather than water's.
//
// **A second function and not a parameter on the one above, deliberately.** Adding an `f0`
// argument and passing `WATER_F0` at the old call sites is the *same value* through a
// different expression, and `errors.md`'s batch-38 entry is four paragraphs on what this
// compiler does with that: a folded-away difference of one ULP cost that batch its control's
// bit-exactness for 201 pixels. Water's Fresnel is read by every sea pixel in the fixture and
// is the single most-measured expression in the shader; five lines of duplication is a very
// cheap way to guarantee it is byte-for-byte the function batch 8 shipped.
fn schlick_glass(cos_i: f32) -> f32 {
    let m = clamp(1.0 - cos_i, 0.0, 1.0);
    let m2 = m * m;
    return GLASS_F0 + (1.0 - GLASS_F0) * m2 * m2 * m;
}

// Normal-incidence reflectance of an ordinary opaque dielectric -- stone, soil, bark, grass,
// snow, sand. `((n-1)/(n+1))^2` at **n = 1.5**, which is where almost every non-metal in a
// world like this one sits: the spread from n = 1.45 to n = 1.6 is 0.033 to 0.053, well inside
// what the authored gain below is swept over anyway. **Nothing in this world is a metal**, and
// that is what makes one number enough -- a metal's F0 is its albedo and would have to be per
// block, which is a *content* change in batch 59's sense and would widen a per-voxel store.
// This is a field change: one constant, no new attribute, no bit in the visibility key.
const GROUND_F0: f32 = 0.04;

// Schlick a third time, at the ground's F0 rather than water's or glass's.
//
// **A third function and not a parameter on either of the two above, for the reason
// `schlick_glass` gives at length**: `errors.md`'s batch-38 entry is four paragraphs on what
// this compiler does when the same value reaches a call site through a different expression --
// a folded-away difference of one ULP cost that batch its control's bit-exactness for 201
// pixels. Water's Fresnel is read by every sea pixel in the fixture and glass's by every pane.
// Five lines of duplication is a very cheap way to leave both of them byte-for-byte the
// functions they were, which is what lets `--no-sky-specular` be a *bit-exact* revert rather
// than a close one.
// How rough an ordinary block surface is, on the usual 0 = mirror, 1 = fully diffuse scale.
//
// **This is the constant that makes the term readable, and the first build shipped without it
// and was wrong.** Schlick's curve reaches 1.0 at grazing for *every* F0, so a hillside seen
// from above -- where almost every ground pixel is at grazing incidence -- came back reflecting
// 60% of the sky and read as a blue-white haze over the whole frame rather than as a sheen:
// MAE 15.65 at `default`, max delta 202. That is not a gain being too high, it is the *shape*
// being wrong, and turning `SKY_SPEC_GAIN` down would have suppressed the honest 4% at normal
// incidence to fix a grazing artefact.
//
// **What is missing from a bare Fresnel is the geometric shadowing-masking term.** On a rough
// surface the microfacets that would mirror a grazing ray are occluded by their own neighbours,
// which is exactly the angle where Schlick says reflectance is highest -- so the two effects
// fight, and a model with the first and not the second blows out. A full microfacet BRDF is not
// wanted here; the standard cheap stand-in caps how far the curve may climb, at `1 - roughness`.
// At 0.9 that ceiling is **0.10** rather than 1.0, so grass and stone gain a tenth of the sky at
// the horizon instead of nearly all of it, and the normal-incidence 4% is untouched.
//
// 0.9 rather than 1.0 because 1.0 would flatten the curve entirely and leave a constant 4% with
// no angular shape at all, which is not a specular term, it is a slightly brighter ambient.
const GROUND_ROUGHNESS: f32 = 0.9;

fn schlick_ground(cos_i: f32) -> f32 {
    let m = clamp(1.0 - cos_i, 0.0, 1.0);
    let m2 = m * m;
    // The grazing ceiling, which is 1.0 in the two functions above and is not here. `max`
    // against F0 so that a hypothetical roughness of 1.0 cannot invert the curve.
    let top = max(1.0 - GROUND_ROUGHNESS, GROUND_F0);
    return GROUND_F0 + (top - GROUND_F0) * m2 * m2 * m;
}

// How much of a ray of length `t` from the eye is spent inside the water, when the eye is
// already under the surface.
//
// Analytic against the sea *plane*, not traced: from inside the medium the marcher cannot
// find the boundary at all, because a water-to-air transition is not an occupancy edge and
// the tree stores nothing but occupancy. The plane is exact for an ocean and wrong for a
// horizontal ray inside a small pond, which never reaches the far bank and stays fogged as
// though the pond were the sea. That is the one approximation in the underwater path.
fn water_path(t: f32, rd: vec3<f32>) -> f32 {
    if rd.y <= 0.0 {
        return t;
    }
    return clamp((frame.sea_level - frame.cam_pos.y) / rd.y, 0.0, t);
}

// World-space distance to the hit a visibility key describes, re-derived from the stored
// face rather than read back out of the key's 24-bit depth -- that field exists to make
// `atomicMax` do the depth test, and is far too coarse to reconstruct a position from.
// `resolve` shades from this and `taa` reprojects from it, and they must agree exactly or
// a surface would sample its own history from a slightly wrong place.
fn hit_t(ci: u32, v: vec3<u32>, micro: vec3<u32>, normal_id: u32, rd: vec3<f32>) -> f32 {
    let c = chunks[ci];
    let box_min = c.origin + vec3<f32>(v) * c.voxel_size;
    if normal_id >= NORMAL_CROSS_A {
        // Batch 14's cross-quad, re-derived exactly the way a cube face is and for exactly
        // the same reason: `resolve` shades from this and `taa` reprojects from it, and if
        // the two disagreed by a hair a blade would sample its own history from the wrong
        // place. The 24-bit depth in the key is far too coarse to reconstruct from.
        //
        // Both diagonal planes are fixed by the voxel alone, so the two normal states are
        // the entire extra payload -- nothing had to be stored anywhere. In world space the
        // voxel scale cancels out of plane A entirely and survives in plane B only as the
        // cell's own width:
        //
        //   A:  x - z = bx - bz          B:  x + z = bx + bz + vs
        if normal_id == NORMAL_CROSS_A {
            return ((box_min.x - box_min.z) - (frame.cam_pos.x - frame.cam_pos.z))
                / (rd.x - rd.z);
        }
        return ((box_min.x + box_min.z + c.voxel_size) - (frame.cam_pos.x + frame.cam_pos.z))
            / (rd.x + rd.z);
    }
    let axis = normal_id >> 1u;
    // Batch 38. The plane is the **micro-cell's** face, unconditionally -- there is no
    // `SPEC_LEAF_CUTOUT` anywhere on this path, and that is the point.
    //
    // It reduces to the cube's own face exactly, not approximately, because `march_chunk`
    // records the *entry* cell on every solid hit: the near face is cell 0 at offset 0 and the
    // far face is cell MICRO-1 at offset MICRO, and `MICRO * (vs / MICRO)` is `vs` on the nose
    // for every power-of-two stride. So one expression serves a carved block and a whole one.
    //
    // **Unconditional because every conditional shape failed.** An `if SPEC_LEAF_CUTOUT` here
    // -- as an early return, as a selected offset, or hoisted into its own dispatch function
    // behind a plain `let` -- made the driver rebuild the folded function one ULP away from
    // what it built before, for **201 pixels of 230,400 at a max channel delta of 1** against
    // `voxelcraft-pre38.exe`. Bisected across nine builds: the widened decode is free, the
    // extra parameter is free, the branch is not. Same family as `errors.md`'s foliage fold.
    let ms = c.voxel_size / f32(MICRO);
    let plane = box_min[axis]
        + f32(micro[axis] + select(0u, 1u, (normal_id & 1u) == 1u)) * ms;
    return (plane - frame.cam_pos[axis]) / rd[axis];
}

fn depth_key(t: f32) -> u32 {
    return u32(clamp(1.0 - t / frame.far, 0.0, 1.0) * 16777215.0);
}

fn make_key(t: f32, normal: u32, ci: u32, v: vec3<u32>, micro: vec3<u32>) -> u64 {
    let d = depth_key(t);
    // The six bits the voxel field has always had spare, spent: 64^3 needs 6 bits an axis and
    // the field gives 8, so the micro coordinate rides in bits 6 and 7 of each byte and the
    // layout above it -- depth:24, normal:3, chunk:13 -- does not move by one bit.
    let b = v | (micro << vec3<u32>(MICRO_SHIFT));
    return (u64(d) << 40u) | (u64(normal) << 37u) | (u64(ci) << 24u)
        | u64((b.x << 16u) | (b.y << 8u) | b.z);
}

// ---------------------------------------------------------------------------
// Atmosphere and sky (batches 5 and 10)
// ---------------------------------------------------------------------------
//
// `transmittance_from` and `transmittance` lived in `resolve.wgsl` until batch 10, which
// gave the haze a second caller on this side of the concatenation: the cloud deck fades
// into the distance through the identical closed form a ridge does. Moving them is a move
// and nothing else -- same body, same callers in `resolve`.

// The fraction of a surface's radiance that survives the haze between it and the eye.
//
// Density falls off exponentially with altitude, so the optical depth along a ray is a
// closed form and never a march:
//
//   tau(s) = rho0 * exp(-lambda * (O_y - h0)) * (1 - exp(-lambda * s * D_y)) / (lambda * D_y)
//
// written below with `lambda * D_y` substituted as `x / s`, which removes the division by
// a quantity that vanishes for a horizontal ray. What is left is `s * (1 - exp(-x)) / x`,
// whose limit as `x -> 0` is plainly `s`: the constant-density case, where the ray never
// changes altitude. That is the same guard the horizontal-ray case would need, in one
// place instead of two.
//
// Heights are **internal** world Y (0..512), not the displayed Y the HUD prints;
// `frame.fog_height` is the altitude at which `fog_density` is the density.
//
// The exponent clamps are for a ray pointing steeply down over a long distance, where
// `exp(-x)` overflows f32 long before the answer stops meaning "fully fogged". Clamping
// only ever fires where the result has already saturated.
// `y0` is the altitude the segment starts at. It was `frame.cam_pos.y` outright until batch
// 8 gave the haze two other places to start from: the point where an underwater ray leaves
// the water, and the water surface a reflected ray sets off from. Passing it in is exact
// where reusing the camera's altitude would have been a guess.
fn transmittance_from(y0: f32, s: f32, rd: vec3<f32>) -> f32 {
    let lambda = frame.fog_falloff;
    let x = clamp(lambda * s * rd.y, -60.0, 60.0);
    var path = s;
    if abs(x) > 1e-4 {
        path = s * (1.0 - exp(-x)) / x;
    }
    let rho = frame.fog_density * exp(-clamp(lambda * (y0 - frame.fog_height), -60.0, 60.0));
    return exp(-clamp(rho * path, 0.0, 60.0));
}

fn transmittance(s: f32, rd: vec3<f32>) -> f32 {
    return transmittance_from(frame.cam_pos.y, s, rd);
}

// Linear radiance. The palette is the same one that was authored as display colours,
// decoded once by hand; the scalar dims are the old gamma-space factors raised to 2.2, so
// the sky looks as it did while every blend along the way now happens in linear space.
// Sun-tinted scattering, authored as a display colour and decoded like the rest.
const SCATTER_TINT: vec3<f32> = vec3<f32>(1.0000, 0.4770, 0.1474);

// ---- the sky gradient's four endpoints, named rather than spelled inline (batch 63) ----
//
// **These were four literals inside `sky_base` until roadmap R7, and the batch that hoisted
// them is the batch that gave them a second reader.** `sky_ambient_tint` in `resolve.wgsl`
// integrates this same gradient over the hemisphere a face can see, so the two now have to
// agree by construction rather than by someone remembering. That is this tree's standing rule
// about a number living in two places, applied before it could go wrong rather than after --
// batch 26 is what it looks like when it goes wrong first.
//
// Nothing here changed value. `sky_base` reads these and folds to the same constants it always
// had, which is what lets R7's control be a pure revert.
const SKY_NIGHT_HORIZON: vec3<f32> = vec3<f32>(0.0049, 0.0060, 0.0134);
const SKY_DAY_HORIZON: vec3<f32> = vec3<f32>(0.4480, 0.6210, 0.8900);
const SKY_NIGHT_ZENITH: vec3<f32> = vec3<f32>(0.0008, 0.0015, 0.0039);
const SKY_DAY_ZENITH: vec3<f32> = vec3<f32>(0.0637, 0.2140, 0.8276);
// 97a's `--sky-cool` swap pair (gated by the cool-sky override -- and note the night
// half does not swap: the complaint lives in daylight). Horizon less lilac and cleaner,
// zenith deeper but *bluer* -- the reference frames read clean cornflower where this
// gradient runs violet. A constant swap of the tint-balance class: off, the selects
// fold and the gradient is every pre-97 constant unchanged. Comments must not name the
// override: batch 97's census counts the token, and a comment's mention is text too.
const SKY_COOL_DAY_HORIZON: vec3<f32> = vec3<f32>(0.4074, 0.6057, 0.9150);
const SKY_COOL_DAY_ZENITH: vec3<f32> = vec3<f32>(0.0385, 0.1892, 0.8600);
// The gradient's own elevation exponent. `sky_base` raises `clamp(rd.y, 0, 1)` to it, and R7's
// hemisphere weights below are the cosine-weighted means of that same power -- so this number
// appears once and the two readers cannot disagree about which curve they are averaging.
const SKY_GRAD_EXP: f32 = 0.55;

// Henyey-Greenstein, normalised so isotropic scattering reads 1.0 rather than 1/(4*pi).
// `SCATTER_TINT` is a look constant and not an irradiance, so carrying the 1/(4*pi)
// through it would only scale `fog_scatter` by four pi and tell nobody anything. The
// `pow(d, 1.5)` is the phase function's own exponent, not a gamma curve -- everything
// here is still linear radiance.
fn hg_gain(cos_t: f32, g: f32) -> f32 {
    let g2 = g * g;
    let d = 1.0 + g2 - 2.0 * g * cos_t;
    return (1.0 - g2) / pow(max(d, 1e-4), 1.5);
}

// The forward-scattering lobe, in linear radiance. **One lobe serves both the sky miss and
// the fog**: `sky_base` adds it below and `resolve` mixes fogged surfaces toward
// `sky_base(rd)`, so the sun's halo is counted exactly once and a hazed ridge matches the
// sky directly above it. Batch 10 gave it a third user, the cloud deck, on the same
// argument. It replaces the `pow(sd, 16) * 0.25` term that used to sit in the sky
// function -- the same job, except HG's tail keeps a wide skirt of glow where cos^16
// had fallen to nothing by 45 degrees off the sun. Adding a halo *beside* that term
// instead of in place of it would double-count the horizon.
fn scatter_lobe(rd: vec3<f32>) -> vec3<f32> {
    let sd = dot(rd, frame.sun_dir);
    return SCATTER_TINT * (hg_gain(sd, frame.fog_g) * frame.fog_scatter * frame.daylight);
}

// The sky without the cloud deck: the gradient, the sun disc and the one scattering lobe.
//
// **This is what the fog mixes toward, and that is the whole reason the split exists.** A
// ridge two kilometres out is hazed by the air in front of it, and that air glows with the
// sky *behind* the ridge -- not with a cloud another eight kilometres past it. Mixing a
// fogged surface toward a cloud-bearing sky lands the deck's brightness on the hillside,
// which reads as a spotlight nobody can find the source of.
//
// `shaft` (batch 11) multiplies the lobe and nothing else, because the lobe is the only part
// of this function that is sunlight scattered by the air the view ray just crossed. The
// gradient is the sky itself, and a ridge does not shadow the sky.
fn sky_base(rd: vec3<f32>, shaft: f32) -> vec3<f32> {
    let day = frame.daylight;
    let up = clamp(rd.y, 0.0, 1.0);
    let horizon = mix(SKY_NIGHT_HORIZON,
                      select(SKY_DAY_HORIZON, SKY_COOL_DAY_HORIZON, SPEC_SKY_COOL), day);
    let zenith = mix(SKY_NIGHT_ZENITH,
                     select(SKY_DAY_ZENITH, SKY_COOL_DAY_ZENITH, SPEC_SKY_COOL), day);
    var c = mix(horizon, zenith, pow(up, SKY_GRAD_EXP));
    let sd = dot(rd, frame.sun_dir);
    c += vec3<f32>(1.0, 0.8276, 0.5225) * smoothstep(0.9975, 0.9990, sd) * max(day, 0.2);
    // **The one term batch 11 touches.** `shaft` is the fraction of the lobe that survived
    // the air between the eye and whatever this sky stands behind; 1.0 is the pre-batch-11
    // sky exactly, and every caller with no shaft to offer passes it.
    c += scatter_lobe(rd) * shaft;
    if SPEC_SKY_LOOK && frame.haze_warm > 0.0 {
        // G1 (`--haze-warm`): the band the reference frames wear -- a warm skim over the
        // treeline even at midday, strongest toward the sun, gone at the zenith. It lives
        // *here* and not in the fog path on purpose: `sky_base` is what every hazed surface
        // mixes toward, so the far treeline picks the identical band up through the air
        // that is genuinely in front of it, the batch-10 argument in the other direction.
        let band = pow(1.0 - up, 3.0);
        let toward = 0.35 + 0.65 * pow(max(sd, 0.0), 4.0);
        c += SCATTER_TINT * (band * toward * frame.haze_warm * day * 0.22);
    }
    if SPEC_SKY_LOOK && frame.zenith_deep > 0.0 {
        // G1 (`--zenith-deep`): the reference sky runs a much richer blue upstairs than
        // SKY_DAY_ZENITH while staying pale at the horizon. A multiply by the gradient's
        // own weight, not a recolour -- at 0 the factor is 1.0 everywhere, so the gradient
        // (and every constant tuned against it) is the pre-95 gradient bit for bit.
        let z = pow(up, SKY_GRAD_EXP);
        c *= mix(vec3<f32>(1.0), vec3<f32>(0.80, 0.90, 1.16), frame.zenith_deep * z * day);
    }
    c = mix(c, horizon * 0.26, smoothstep(0.0, -0.35, rd.y));
    return c;
}

// ---------------------------------------------------------------------------
// The cloud deck (batch 10)
// ---------------------------------------------------------------------------
//
// One horizontal plane of 2D noise, intersected by the rays that reach the sky. Not
// volumetrics: there is no march, no second occupancy structure and nothing per chunk,
// because a cloud touches nothing the world stores. The cost is therefore bounded by
// construction to rays that miss geometry -- the way `ATTR_HAS_WATER` bounds water from
// below -- so it is nothing in a frame full of hillside and real in a frame full of sky.

// Four octaves. A fifth is under the pixel footprint everywhere the deck is near enough to
// have visible shape, so it would only be something for the band limit below to remove.
const CLOUD_OCTAVES: u32 = 4u;
// **Opacity and depth are two ramps off the same noise, and they are deliberately not the
// same width.** `CLOUD_SOFT` is the coverage edge: narrow, so a cloud is opaque a little way
// in from its rim and does not read as a cut-out. `CLOUD_DEEP` is how far above the
// threshold the noise has to climb before the underside is fully shadowed, and it is four
// times wider on purpose -- one ramp for both is what makes every cloud a flat grey blob,
// because the pixel that just turned opaque is also the pixel that just turned black.
//
// It is wider than the noise's own headroom, which is the point: with the default cover a
// four-octave peak reaches about 43% of the way to `CLOUD_BASE`, so scattered cumulus stay
// bright. Push `cloud_cover` toward 1 and the threshold drops to the floor, the whole field
// clears `CLOUD_DEEP`, and the same two lines give a dark overcast.
const CLOUD_SOFT: f32 = 0.14;
const CLOUD_DEEP: f32 = 0.55;
// G1's bank field (batch 95): how many times wider than the deck's own span the clustering
// octave runs. `cloud_scale * 6` puts bank wavelengths at two kilometres and change -- the
// distance between the blue gaps in the reference frames -- and any finer reads as weather
// noise rather than geography.
const CLOUD_PATCH_SPAN: f32 = 6.0;
// Drift direction. Deliberately not axis-aligned: a pure +X drift slides the field back
// over its own lattice once per `cloud_scale` and the deck visibly repeats itself.
const CLOUD_WIND: vec2<f32> = vec2<f32>(0.94, 0.34);
// Where the deck closes out. `CLOUD_FADE` starts a smooth ramp to zero alpha and `CLOUD_MAX`
// is the hard stop, which exists because a near-horizontal ray puts the plane intersection
// past f32 and a NaN multiplied by weight zero is still a NaN. At the default haze the deck
// is under 1% alpha long before `CLOUD_FADE`, and the band limit has flattened the noise to
// its own mean before that, so the ramp guards the one corner where both of those are
// switched off (`--fog-density 0 --cloud-cover 1`) rather than being a falloff anyone sees.
const CLOUD_FADE: f32 = 3.0e4;
const CLOUD_MAX: f32 = 1.0e5;

// A thin sunlit rim, and the shadowed underside of a thick core. Authored as display
// colours and decoded, like every other colour since batch 2.
const CLOUD_LIT: vec3<f32> = vec3<f32>(1.0000, 0.9575, 0.8963);
const CLOUD_BASE: vec3<f32> = vec3<f32>(0.4854, 0.5312, 0.6462);
// 97a under `--sky-cool`: whiter lit faces and grey-blue bellies in place of the warm
// parchment pair -- parchment reads as haze on a *cool* sky. Same total light (the
// mixes keep `thick`'s volumes), so the deck's shape ramps are the pre-97 ramp exactly.
const CLOUD_COOL_LIT: vec3<f32> = vec3<f32>(1.0000, 1.0000, 1.0000);
const CLOUD_COOL_BASE: vec3<f32> = vec3<f32>(0.4420, 0.4880, 0.6010);
// What is left of a cloud at night: above the night zenith, below the night horizon, which
// is where the light that still leaves it comes from.
const CLOUD_NIGHT: vec3<f32> = vec3<f32>(0.0021, 0.0026, 0.0052);
// How far toward `SCATTER_TINT` a low sun drags the lit rim. 1.0 would make a sunset cloud
// the colour of the lobe itself, which is a coal; the rim is lit *through* the reddening
// air, not made of it.
const CLOUD_WARMTH: f32 = 0.72;
// The deck's own forward-scattering lobe. A cloud scatters harder forward than the haze
// does, and `CLOUD_SCATTER` is small because it multiplies an HG gain that peaks near 30.
const CLOUD_G: f32 = 0.82;
const CLOUD_SCATTER: f32 = 0.055;

fn hash_u32(x: u32) -> u32 {
    var h = x;
    h ^= h >> 16u;
    h *= 0x7feb352du;
    h ^= h >> 15u;
    h *= 0x846ca68bu;
    h ^= h >> 16u;
    return h;
}

// One lattice value in [0, 1). An integer hash rather than the usual `fract(sin(dot(...)))`:
// it is cheaper, and `sin` of a large argument is the one thing in this file whose result
// genuinely differs between drivers, which would put a driver-dependent image behind every
// A/B in `docs/`.
fn cloud_lattice(c: vec2<i32>) -> f32 {
    let h = hash_u32(bitcast<u32>(c.x) ^ (hash_u32(bitcast<u32>(c.y)) * 0x9e3779b9u));
    return f32(h >> 8u) * (1.0 / 16777216.0);
}

fn cloud_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = p - i;
    // Quintic rather than the cubic smoothstep: its second derivative vanishes at the
    // lattice too, and a coverage threshold applied to a merely C1 field prints the cell
    // edges as faint creases across every cloud.
    let w = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let c = vec2<i32>(i);
    let a = cloud_lattice(c);
    let b = cloud_lattice(c + vec2<i32>(1, 0));
    let d = cloud_lattice(c + vec2<i32>(0, 1));
    let e = cloud_lattice(c + vec2<i32>(1, 1));
    return mix(mix(a, b, w.x), mix(d, e, w.x), w.y);
}

// Four octaves, each faded out as its period shrinks toward the size of a pixel. `fp` is
// that pixel, measured in units of the base octave.
//
// **The sum is renormalised by the surviving weights, so a dropped octave falls back to the
// running mean and not to zero.** That is the difference between a deck that softens into a
// smooth sheet at the horizon and one that dissolves into clear sky there -- and clear sky
// would be a hole with a cloud edge around it, which is worse than the aliasing this is
// here to remove. Where every octave is gone the field is its own mean, 0.5, uniformly.
fn cloud_fbm(p: vec2<f32>, fp: f32) -> f32 {
    var freq = 1.0;
    var amp = 1.0;
    var sum = 0.0;
    var norm = 0.0;
    for (var i = 0u; i < CLOUD_OCTAVES; i = i + 1u) {
        let w = amp * (1.0 - smoothstep(0.25, 0.5, fp * freq));
        if w > 0.0 {
            sum += w * cloud_noise(p * freq);
            norm += w;
        }
        freq *= 2.0;
        amp *= 0.5;
    }
    if norm <= 0.0 {
        return 0.5;
    }
    return sum / norm;
}

// Radiance of the deck, linear like everything else the compute passes produce.
//
// A flat layer has no normal to shade with, so **thickness is the only shading variable
// there is** and `dens` does the whole job: a thin edge passes sunlight and reads bright, a
// thick core passes none and reads as its own shadowed underside. Seen from *above* that
// inverts -- the top is the face the sun actually lands on -- and one `select` is the whole
// of that case.
fn cloud_radiance(dens: f32, depth: f32, rd: vec3<f32>) -> vec3<f32> {
    let day = frame.daylight;
    let thick = select(depth, 0.0, rd.y < 0.0);
    // A low sun reddens on the way through the air *below* the deck, which is why a golden
    // hour has golden clouds. The reddening is `SCATTER_TINT`, the same one the haze and the
    // sky already use: a cloud lit by a different red than the air under it reads as a decal
    // pasted onto the sky.
    let warm = 1.0 - smoothstep(0.02, 0.30, frame.sun_dir.y);
    let sun_tint = mix(vec3<f32>(1.0), SCATTER_TINT, warm * CLOUD_WARMTH);
    let day_c = mix(select(CLOUD_LIT, CLOUD_COOL_LIT, SPEC_SKY_COOL) * sun_tint,
                    select(CLOUD_BASE, CLOUD_COOL_BASE, SPEC_SKY_COOL), thick);
    // Forward scattering *through* the deck, weighted by what is thin enough to transmit it.
    // Same `hg_gain`, same tint: one lobe now serves the sky, the haze and the cloud.
    let glow = SCATTER_TINT
        * (hg_gain(dot(rd, frame.sun_dir), CLOUD_G) * CLOUD_SCATTER * day * (1.0 - dens));
    return mix(CLOUD_NIGHT, day_c, day) + glow;
}

// The sky a ray that reaches it actually sees: `sky_base`, with the deck composited over.
//
// **`ro` is the ray's own origin and it is not always the camera.** `sky_base` needs only a
// direction, because a gradient at infinity has no parallax to get wrong -- but the deck sits
// at a finite altitude, so where the ray *starts* decides which piece of it the ray hits. The
// water reflection sets off from the surface, not from the eye, and passing the camera there
// gives every water pixel in the frame the same origin: the reflected deck collapses to a
// function of direction alone, stops being anchored to the world, and slides along with the
// camera while the deck directly overhead stays put. `transmittance_from` one line below
// already drew this distinction for the traced reflection leg.
//
// `cloud_cover` 0 returns `sky_base` through an early exit rather than through an expression
// that evaluates to zero, so the control is bit-exact *and* free and the two claims are the
// same line of code. The coverage threshold would give zero density there anyway -- the
// noise is bounded above by 1.0 and the threshold sits at exactly 1.0 -- but a term costing
// four octaves of noise to come out as nothing is not a control anyone can bench against.
fn sky_color(ro: vec3<f32>, rd: vec3<f32>, travelled: f32, shaft: f32) -> vec3<f32> {
    let base = sky_base(rd, shaft);
    if frame.cloud_cover <= 0.0 {
        return base;
    }
    // A plane is reached only by travelling toward it: from below looking up, or from above
    // looking down. The product is both of those at once, and it catches the horizontal ray
    // the division would otherwise send to infinity.
    let dy = frame.cloud_height - ro.y;
    if dy * rd.y <= 0.0 {
        return base;
    }
    let t = dy / rd.y;
    if t > CLOUD_MAX {
        return base;
    }
    let p = ro.xz + rd.xz * t;

    // The plane-space width of one pixel: a pixel subtends `2 * tan_half_fov / res.y`, and
    // perturbing the ray by that angle slides the intersection by `1 / |rd.y|` times the
    // distance. This is `resolve`'s mip footprint with a plane in place of a voxel face, and
    // it carries `mip_bias` for the identical reason -- at `--scale 0.5` the footprint has to
    // stay the one the *window* resolves, or the temporal pass is handed a deck with no
    // detail left in it to reconstruct.
    //
    // **`travelled` is the distance the ray already covered before `ro`, and it belongs in
    // here.** The cone spreads from the eye, not from wherever this leg started: a reflection
    // off water 600 blocks away arrives at the deck with a cone 600 blocks wider than `t`
    // alone would say, and dropping that term keeps octaves the pixel cannot resolve. It is
    // the same accumulated distance `shade_water` already hands `shade_hit` as `t + h2.t` so
    // the refracted image picks the right mip.
    let px_angle = 2.0 * frame.tan_half_fov / f32(frame.res.y) * exp2(frame.mip_bias);
    let fp = (travelled + t) * px_angle / (abs(rd.y) * frame.cloud_scale);

    let drift = CLOUD_WIND * (frame.time * frame.cloud_speed);
    let n = cloud_fbm((p + drift) / frame.cloud_scale, fp);
    var thr = 1.0 - frame.cloud_cover;
    if SPEC_SKY_LOOK && frame.cloud_patch > 0.0 {
        // G1 (`--cloud-patch`): real cumulus arrive in *banks* -- big blue between groups --
        // which one threshold on one field can never draw. A coarse tap swings the threshold
        // itself, by up to the whole noise range at strength 1. The deck's own drift keeps
        // banks and deck glued to the same geography; the clamp holds the `dens` contract:
        // density still follows `thr` per texel exactly the way the pre-95 deck's does --
        // only *where the texels are* moved. `fp / CLOUD_PATCH_SPAN` because the band limit
        // must follow the octave's own pixel size, like every cloud_fbm call site.
        let m = cloud_fbm(
            (p + drift) / (frame.cloud_scale * CLOUD_PATCH_SPAN),
            fp / CLOUD_PATCH_SPAN
        );
        thr = clamp(thr + (m - 0.5) * frame.cloud_patch, 0.0, 0.99);
    }
    let dens = smoothstep(thr, thr + CLOUD_SOFT, n);
    if dens <= 0.0 {
        return base;
    }
    var depth = smoothstep(thr, thr + CLOUD_DEEP, n);
    if SPEC_SKY_LOOK && frame.cloud_relief > 0.0 {
        // G1 (`--cloud-relief`): a flat plane has no verticality to shade, but the reference
        // clouds are lit from the *side* at midday -- white caps against grey-blue bellies.
        // One second tap, displaced sunward, prices the slope: where the field climbs
        // toward the sun from here, this texel sits in its lee and reads underside; where
        // it falls away, the cap catches. The displacement is in *fbm units* (the deck's
        // own scale) and grows with sun elevation because a high sun displaces more deck
        // height. Density stays the original field's -- relief is shading only, and a
        // shading term must never grow a cloud.
        let az = normalize(frame.sun_dir.xz + vec2<f32>(1.0e-3, 0.0));
        let step = (8.0 + frame.sun_dir.y * 48.0) / frame.cloud_scale;
        let ns = cloud_fbm((p + drift) / frame.cloud_scale + az * step, fp);
        depth = clamp(depth + (ns - n) * (2.2 * frame.cloud_relief), 0.0, 1.0);
    }

    // **The haze is the horizon fade.** A cloud at three kilometres is hazed by exactly the
    // air a ridge at three kilometres is, through batch 5's closed form and not a second
    // falloff invented for the sky -- and because a near-horizontal ray reaches the deck
    // kilometres out, that same term is what keeps the deck from aliasing into the horizon.
    // `--fog-density 0` is therefore a control for both at once: crisp cloud to the horizon,
    // with only the octave band limit holding it together.
    let reach = 1.0 - smoothstep(CLOUD_FADE, CLOUD_MAX, t);
    let a = dens * reach * transmittance_from(ro.y, t, rd);
    return mix(base, cloud_radiance(dens, depth, rd), a);
}

// ---------------------------------------------------------------------------
// Crepuscular rays (batch 11)
// ---------------------------------------------------------------------------
//
// **Nothing here adds radiance.** `sky_base` already carries one Henyey-Greenstein lobe of
// in-scattered sunlight and `resolve` already mixes a fogged surface toward it, so the air
// between the eye and a ridge already glows toward the sun -- *unshadowed*, which put that
// glow on the shadowed face of the very ridge blocking the sun from the air in front of it.
// What batch 11 computes is the factor that lobe is multiplied by. A beam added *beside* the
// lobe would double-count the horizon, which is the trap batch 5's retrospective named and
// the reason this is a weight and not a `+`.

// The importance ramp, in linear radiance. `max(scatter_lobe(rd)) * alpha` is the largest
// amount of light the shaft could possibly move at this pixel, so below `GODRAY_CUT_LO` no
// visibility whatsoever moves it by a code value and the integral is skipped outright.
//
// It is a ramp and not a threshold for the `WATER_REFRACT_NEAR/FAR` reason: what fades is
// the *shadowing* and not the trace, so the seam sits where the shadowed quantity is already
// invisible rather than where the geometry changes. `hg_gain` at g = 0.8 spans 700x between
// the sun and the anti-sun, so this is what makes a frame looking away from the sun cost
// nothing at all -- and it is the bound that had to be *tightened* after the first
// measurement. The original 0.0015 only cut past 109 degrees off the sun, which is to say it
// cut almost nothing: a vantage where the shafts moved zero pixels still paid a full
// millisecond, because a smooth ramp at weight 0.07 costs exactly what it costs at 1.0. The
// numbers here cut at about 55 degrees, and `PERF.md` carries both.
const GODRAY_CUT_LO: f32 = 0.0040;
const GODRAY_CUT_HI: f32 = 0.0120;

// The sun's angular diameter in radians, 0.53 degrees -- the real one, and the same disc
// `sky_base` draws with its 0.9975/0.9990 smoothstep. It is here because it sets how wide the
// penumbra of a cloud shadow is, which is the one place in this engine where the sun's finite
// size is not a rounding error: the deck is kilometres away along a low sun.
const SUN_ANGLE: f32 = 0.0093;

// How much of the sun survives the cloud deck on its way to `p`. One plane intersection
// along the *sun* direction plus one `cloud_fbm`: a handful of ALU and no memory, where a
// shadow ray into the world is a march. That is the whole reason the deck can be sampled at
// every step of the shaft in places the world shadow ray cannot reach.
//
// **Two callers, deliberately.** The shaft integral below, and `shade_hit`'s sun term. If the
// deck occludes a beam it has to occlude the ground under the beam, or a shaft goes missing
// from beneath a cloud that casts no shadow -- a live caveat until this batch, and one
// function now, so the two cannot drift apart.
//
// It reads `CLOUD_SOFT` and not `CLOUD_DEEP`: what stops sunlight is how opaque the cloud is,
// where `CLOUD_DEEP` is how far into a thick one its own underside goes dark.
// `frame.cloud_shadow` is below 1 because a cloud scatters light through rather than stopping
// it dead, so an overcast ground reads dim and not black.
//
// **`fp` is a footprint in blocks here, and the sun's own angular size joins it.** The band
// limit `cloud_fbm` already carries for the sky is exactly the right tool for a penumbra: a
// shadow cast from `t` blocks away is blurred by `t * SUN_ANGLE`, which is 26 blocks for a
// deck 2.8 km along a five-degree sun and only 4 for the same deck under a midday one. Handing
// that to the band limit gives the soft edge for free -- and because it drops octaves the
// penumbra has already erased, it is *also* what makes the lookup cheap enough to sample
// eight times per pixel. Physically right and faster is not the usual trade.
fn cloud_shade(p: vec3<f32>, fp: f32) -> f32 {
    if frame.cloud_shadow <= 0.0 || frame.cloud_cover <= 0.0 {
        return 1.0;
    }
    // The same both-directions product `sky_color` makes, and it does the whole of the
    // geometry: a point above the deck with the sun above it has nothing in the way, the
    // product goes negative, and the early exit is also the "no occluder" case.
    let dy = frame.cloud_height - p.y;
    if dy * frame.sun_dir.y <= 0.0 {
        return 1.0;
    }
    let t = dy / frame.sun_dir.y;
    if t > CLOUD_MAX {
        return 1.0;
    }
    let q = p.xz + frame.sun_dir.xz * t;
    let drift = CLOUD_WIND * (frame.time * frame.cloud_speed);
    let n = cloud_fbm((q + drift) / frame.cloud_scale, max(fp, t * SUN_ANGLE) / frame.cloud_scale);
    let thr = 1.0 - frame.cloud_cover;
    return 1.0 - smoothstep(thr, thr + CLOUD_SOFT, n) * frame.cloud_shadow;
}

// A bilinear tap into one slice of the light envelope, in texel coordinates.
//
// **Clamped at the edge rather than returning "lit" outside the window.** A sample past the
// 1024-block half-width -- `godray_dist` defaults past the far plane, so one can -- would
// otherwise cross a hard line from shadowed to lit that has nothing to do with the terrain.
// Clamping extends the rim outward instead, which is both smoother and roughly what the
// ground beyond the window is doing anyway.
//
// The Rust twin of this is `ShaftField::sample`, and it exists because the reference envelope
// `tests/shaft.rs` checks the scan against has to interpolate the same way or it would be
// checking a different function.
fn shaft_sample(base: u32, t: vec2<f32>) -> f32 {
    let hi = f32(SHAFT_DIM) - 1.0;
    let c = clamp(t, vec2<f32>(0.0), vec2<f32>(hi));
    let f0 = floor(c);
    let fr = c - f0;
    let i0 = vec2<u32>(f0);
    let i1 = min(i0 + vec2<u32>(1u), vec2<u32>(u32(hi)));
    let a = shaft[base + i0.y * SHAFT_DIM + i0.x];
    let b = shaft[base + i0.y * SHAFT_DIM + i1.x];
    let c0 = shaft[base + i1.y * SHAFT_DIM + i0.x];
    let d = shaft[base + i1.y * SHAFT_DIM + i1.x];
    return mix(mix(a, b, fr.x), mix(c0, d, fr.x), fr.y);
}

// Batch 35. How much sun reaches `p` past the *terrain*, which is `cloud_shade`'s question
// asked of the other occluder. One tap and a ramp, where the honest march is +10.57 ms.
//
// **It has one caller, and that asymmetry with `cloud_shade` is deliberate.** `cloud_shade`
// has two -- the shaft and `shade_hit`'s sun term -- because a deck that occludes a beam must
// occlude the ground under it, and one function is what stops them drifting. Terrain is the
// other way round: `shade_hit` already casts a real `shadow_ray` per *visible pixel*, which is
// exact and affordable at that rate, and replacing it with this would trade a correct ground
// shadow for a quantised one to buy nothing. So the ground gets the march and the volume gets
// the envelope, and where they disagree it is by `shaft_soft` at ridge range, under a lobe
// that is a glow rather than an edge.
fn terrain_shade(p: vec3<f32>) -> f32 {
    // **The half is not a fudge.** `ShaftField` samples each texel at the *centre* of the
    // cell it stands for, so the value at index `i` sits at world `origin + (i + 0.5) * texel`
    // and the texel coordinate of a world point is half a cell below the naive quotient.
    // Without it every shadow is displaced two blocks in +X and +Z -- a uniform translation,
    // smaller than `shaft_soft`, and therefore exactly the kind of wrong that looks right.
    let t = (p.xz - frame.shaft_origin) / frame.shaft_texel - 0.5;
    let e = shaft_sample(SHAFT_RESULT, t);
    return smoothstep(e - frame.shaft_soft, e + frame.shaft_soft, p.y);
}

// Batch 37. The same envelope, asked only about occluders **beyond** `d` blocks along the sun
// ray -- which is what lets it stand behind `shade_hit`'s march instead of competing with it.
//
// **The offset is the whole design, and it partitions the ray exactly rather than blending
// two answers.** Write the envelope out: `E(c) = max over s >= 0 of h(c + s*dir) - s*tan`,
// where `dir` is the sun's azimuth, `s` a horizontal distance and `tan = sun_dir.y / horiz`.
// Query it at `q = p + sun_dir * d`, whose column is `D = d * horiz` blocks up-sun of `p`:
//
//     E(q.xz) = D*tan + max over s >= D of ( h(p.xz + s*dir) - s*tan )
//     q.y     = p.y + d*sun_dir.y = p.y + D*tan
//
// The two `D*tan` cancel, so `q.y > E(q.xz)` is exactly `p.y > max over s >= D`. **Sampling
// the envelope `d` blocks up-sun is the envelope at the surface with every occluder nearer
// than `d` removed from the maximum** -- so passing `d = shadow_dist` hands the marcher
// [0, shadow_dist] and this the rest, with no gap between them and nothing counted twice.
//
// That identity is why this is not the obvious `lit * terrain_shade(hit)`, which was the first
// thing tried and is wrong in a way a screenshot flatters: `E(hit.xz)` is a maximum over *all*
// occluders including the ones the march just answered exactly, so the envelope's 4-block
// quantisation gets a second vote on near geometry and paints soft false shadows across ground
// the marcher had already resolved to the voxel. `tests/shaft.rs` asserts the identity against
// the reference `ShaftField` rather than trusting this comment.
//
// It also costs nothing to be exact here: the cancellation means no new constant is authored,
// no crossover distance is tuned and no blend width is chosen. The only softness is
// `shaft_soft`, which batch 35 already sized as a penumbra at ridge range -- and every
// occluder this function can see is at least `shadow_dist` away, which is where that argument
// was made.
fn terrain_shade_beyond(p: vec3<f32>, d: f32) -> f32 {
    return terrain_shade(p + frame.sun_dir * d);
}

// The fraction of `scatter_lobe`'s glow that survives the air between the eye and `dist`.
//
// In-scattering along a ray weights each point by `T(0,s) * rho(s)`, and batch 5 already
// wrote both in closed form. **That weight is a probability density with an invertible CDF,
// so the samples are placed by importance rather than spread evenly and weighted.** Its
// integral over `0..D` is exactly `1 - T(0, D)` -- the very alpha `resolve` fogs with -- so
// drawing `s` with `T(0,s)` stratified uniformly through that range makes the estimator a
// plain *mean* of visibility, with no weights to carry and no denominator to divide by.
//
// It is better and cheaper at once, which is not the usual trade. Better, because a fixed
// number of samples lands where the light actually comes from: an evenly spaced set spends
// most of itself in the far half of the path that `T` has already extinguished, and the
// variance the temporal pass then has to clean up is four times larger. Cheaper, because
// each step is two `log`s instead of the three `exp`s the weighted form needed.
//
// Inverting it: `path(s) = (1 - exp(-k s)) / k` for `k = lambda * rd.y`, so
// `s = -log(1 - k * path) / k`, with the `k -> 0` limit `s = path` handling a horizontal
// ray in the one place, exactly as `transmittance_from` handles the same singularity.
//
// The offset is `dither(px, py)` -- batch 7's interleaved gradient noise, walked one step per
// frame -- so the stratification's own banding becomes a fine pattern with neighbours well
// separated, which is exactly the input the temporal pass was built to resolve. Measured in a
// sky patch, the shaft raises the high-frequency residual 4.7x in a single frame and 1.4x
// once the temporal pass has it: it removes 90% of what the sampler adds. `--no-taa` shows
// the other 10%, honestly, as noise.
//
// **`godray_dist` is how far along the view ray the samples are spread**, and it is a
// resolution knob rather than a cost one now that the occluder is analytic: the deck can be
// asked about a point ten kilometres away for the same handful of ALU as one nearby, so the
// only thing the reach buys or loses is how finely a fixed number of samples resolves the
// near field against the far. `covered` below is what keeps a short reach honest.
fn sun_shaft(ro: vec3<f32>, rd: vec3<f32>, dist: f32, alpha: f32, px: u32, py: u32) -> f32 {
    // Ways to have no occluder at all, and each is a control someone benches against: no
    // shadowing asked for, no shadow from the deck, no deck, or -- since batch 35 -- no
    // terrain occluder either. **The deck's three mattered as much as the first** --
    // `--cloud-cover 0` is batch 10's control and was measured free, and a loop that walks
    // six steps to call a function that returns 1.0 would have quietly taken that away.
    //
    // **Batch 35 is what makes this two conditions rather than one**, and the split is the
    // whole of what changed here: a deck-less frame still has terrain in it, so "no deck"
    // stopped being sufficient grounds to skip the integral. With `SPEC_TERRAIN_SHAFTS`
    // false, `terrain` folds to `false` and the pair folds back to exactly the one condition
    // this was before -- which is what makes `--no-terrain-shafts` a revert and not a flag.
    let terrain = SPEC_TERRAIN_SHAFTS && (frame.flags & FLAG_TERRAIN_SHAFTS) != 0u;
    let deck = frame.cloud_shadow > 0.0 && frame.cloud_cover > 0.0;
    if frame.godray_strength <= 0.0 {
        return 1.0;
    }
    if !deck && !terrain {
        return 1.0;
    }
    let lobe = scatter_lobe(rd);
    let imp = smoothstep(GODRAY_CUT_LO, GODRAY_CUT_HI,
                         max(max(lobe.r, lobe.g), lobe.b) * alpha);
    if imp <= 0.0 {
        return 1.0;
    }
    let d = min(dist, frame.godray_dist);
    if d <= 0.0 {
        return 1.0;
    }
    // The share of this leg's radiance that is in-scattered light, which is both the
    // normalising constant of the sampling density and the thing there is to shadow. Zero
    // means no medium -- `--fog-density 0`, or a ray high above the scale height -- and then
    // there is nothing for a shadow to fall on.
    let opac = 1.0 - transmittance_from(ro.y, d, rd);
    if opac <= 1e-6 {
        return 1.0;
    }
    let lambda = frame.fog_falloff;
    let rho0 = frame.fog_density
        * exp(-clamp(lambda * (ro.y - frame.fog_height), -60.0, 60.0));
    let k = lambda * rd.y;
    let off = dither(px, py);
    // The ray cone at the sample, in blocks: the footprint the deck's own band limit wants,
    // measured where the sample sits rather than where a view ray meets the plane.
    // `mip_bias` rides along for the reason it rides along everywhere else -- at
    // `--scale 0.5` the footprint has to stay the one the window resolves.
    let px_angle = 2.0 * frame.tan_half_fov / f32(frame.res.y) * exp2(frame.mip_bias);
    var sum = 0.0;
    // **The `v` this accumulates was the deck's alone until batch 35**, and the reason it was
    // is worth keeping: one `shadow_ray` per step measured **+10.57 ms** of `resolve` at
    // 1920x1080 looking into a low sun, against a whole frame that costs 4, and the cheapest
    // corner of that sweep -- four steps looking 128 blocks -- finds no occluder at all,
    // because the ridge is 300 to 600 blocks away. The budget that makes it affordable is the
    // budget that makes it useless. The sweep survives only in git, at `1f6c4a6:PERF.md`; the
    // rejection is in `docs/errors.md`, which is where this comment used to point at two files
    // that no longer say it.
    //
    // `terrain_shade` is the same question answered by a projection rather than a march, so
    // the cost stopped scaling with the reach. The two occluders **multiply**, which is what
    // independent transmissions do: `cloud_shade` returns a fraction and not a bit, and a
    // ridge under a cloud has to take both.
    for (var i = 0u; i < frame.godray_steps; i = i + 1u) {
        let u = (f32(i) + off) / f32(frame.godray_steps);
        let path = -log(1.0 - u * opac) / rho0;
        var s = path;
        let kp = k * path;
        if abs(kp) > 1e-4 {
            s = -log(max(1.0 - kp, 1e-6)) / k;
        }
        s = clamp(s, 0.0, d);
        let p = ro + rd * s;
        var v = cloud_shade(p, s * px_angle);
        if terrain {
            v = v * terrain_shade(p);
        }
        sum += v;
    }
    let vis = sum / f32(frame.godray_steps);
    // **What fraction of the path's whole in-scattering the sampled reach actually held.**
    // `integral(T * rho) ds` over `0..D` is exactly `1 - T(0, D)`, which is the one identity
    // this function rests on -- so the ratio of that closed form at `d` and at the full path
    // is the share of the lobe these samples were entitled to shadow. Without it a
    // horizon-grazing ray, whose optical depth keeps accumulating for kilometres past a short
    // reach, would have its entire lobe darkened on the evidence of the near field.
    var covered = 1.0;
    if d < dist {
        let full = 1.0 - transmittance_from(ro.y, dist, rd);
        if full > 1e-6 {
            covered = min(opac / full, 1.0);
        }
    }
    // `imp`, the strength and the coverage multiply into one weight: at full importance,
    // strength 1 and a fully sampled path the shaft is the mean itself, above strength 1 it
    // deepens past physical, and the clamp is for that case alone.
    return max(mix(1.0, vis, imp * frame.godray_strength * covered), 0.0);
}




// Opt-in schedule experiment. Not a lighting approximation.
override SPEC_COMPACT_SHADE_HIT: bool = false;

// Independent opt-in architecture switches. All consumers share the same Frame/vis.
override SPEC_ISOLATE_GLASS: bool = false;
override SPEC_SHADOW_PASS: bool = true;
// A constant per pipeline: main/opaque, glass-only, water-only.
override RESOLVE_LANE: u32 = 0u;
// Buffer roles are per-dispatch; no sampled/storage texture alias is introduced.
@group(3) @binding(0) var primary_sun: texture_2d<f32>;
