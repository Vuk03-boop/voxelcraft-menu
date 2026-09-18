//! Runtime configuration and command-line parsing.

use crate::lod::LodConfig;
use crate::render::{Clouds, Fog, GodRays, SkyLook};
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Config {
    pub seed: i32,
    /// Water fills every column from its terrain height up to here, in **internal** world
    /// Y. `--no-water` sets it to zero: the identical terrain with no water in it, which
    /// is the control for everything in batch 8.
    pub sea_level: i32,
    /// Multiplier on water's per-channel extinction. 1.0 is the tuned value; 0 is
    /// perfectly clear water, which is the A/B partner for every claim about depth colour.
    pub water_absorb: f32,
    /// Amplitude of the water layer's per-block mottling. **Ships at 0.0 since batch 26**;
    /// `--water-mottle 0.2` is the control and reproduces the pre-batch-26 build bit for bit.
    /// **Not a correction of batch 21b, which chose 0.2 on purpose.** 21b built this
    /// flattening as its candidate fix and measured it moving **0 pixels at fourteen
    /// vantages** -- the null that proved the sea was reading the *glass* layer and broke
    /// that batch open. Having disproved it as the fix, 21b put the default back and left
    /// flattening in the roadmap as batch 21a's free half-measure. What it did not retract
    /// were three artifacts of the abandoned fix claiming 0.0 already shipped. Batch 26 took
    /// the lever on its own measurements. It stays a flag because it is the knob that says
    /// how much of the sea's texture is the mottle: an atlas constant and not a shader one,
    /// so it costs the frame nothing at any value.
    pub water_mottle: f32,
    /// Trace a reflection ray off the water surface. Off leaves the Fresnel weight on the
    /// sky alone.
    pub water_reflect: bool,
    /// Trace the refracted ray for what is under the surface. Off leaves the fully
    /// absorbed body colour, which is the documented opaque approximation.
    pub water_refract: bool,
    /// Roadmap A3, land half: wet-darkened sand within `WET_BAND` of the waterline
    /// (batch 75). Default on; `--no-shore-wet` reverts the band exactly.
    pub shore_wet: bool,
    /// Roadmap A3, water half: animated foam over the shallow column the traced
    /// refraction already measured (batch 75). `--no-shore-foam` reverts it.
    pub shore_foam: bool,
    /// Batch 81, roadmap P12's diagnostic arm: trace the two secondary water legs once
    /// per 2x2 block and let the owner's pixels read them. Deliberately dumb -- the
    /// `--reference` MAE at the three water vantages decides what the guided pass owes.
    pub water_sec: bool,
    /// Batch 81c: **which** NxN the legs are traced for. The diagnostic measured P12 gone
    /// at scale 2; the dumb upsampling is what the fraction sweep holds constant while
    /// 1/2/4 prices resolution against picture, which is the sweep the entry asks for.
    pub water_sec_scale: u32,
    /// A10's retuning batch, presentation side: which filmic shoulder the blit rolls the
    /// traced HDR onto. `knee` ships -- it is the hyperbola every exposure and brightness
    /// constant in the engine was tuned against. `aces` is Narkowicz's 2015 fit, a *curve
    /// arm* per the entry's prescription: one knob moved, all else held, so the by-eye
    /// pass reads what a different shoulder does to the picture and nothing else. AgX
    /// (the hue-preserving, per-channel-matrix prescription) is the follow-up this arm's
    /// reading is meant to decide, not part of it.
    pub tone_map_aces: bool,
    /// Batch 97a's cool midday sky (`--sky-cool`): the gradient and deck constants swap
    /// toward the reference frames' cornflower -- off is the pre-97 sky bit for bit.
    pub sky_cool: bool,
    /// G1's warm grade (`--grade warm`, batch 95): the presentation cast chosen by eye
    /// against the two reference frames in `docs/look-reference-*.png`. Luma-preserving by
    /// construction, so it cannot re-tune a lighting constant; default off, so the control
    /// is the bit-exact pre-95 frame.
    pub grade_warm: bool,
    /// Batch 97b's filmic print (`--grade cine`): blit selector value 2 -- S-curve value,
    /// desaturated mids, cool shadows against warm highlights, all on the luma pivot, so
    /// the same luma law as the warm cast holds for it.
    pub grade_cine: bool,
    /// Batch 101's strength dial (`--grade-strength f`): the second round's verdict was
    /// that cine works at sunset but crushes an already-dark midday hillside -- this is
    /// the dial between those two readings, lerping the graded pixel back toward the
    /// pre-grade pixel. Default 1.0 keeps the batch-97 grade exact (mix at 1.0 returns
    /// its second argument bit-for-bit), and the dial ends the debates-specific case:
    /// one flag, applied after both grade casts, no selector change.
    pub grade_strength: f32,
    /// Batch 97c's water look (`--water-look`): animated metre-scale ripples on the
    /// sky-facing normal, murk absorption growing with the traced column, sunlit
    /// turquoise at the bank. Default off -- the unflagged sea is the control.
    pub water_look: bool,
    /// Batch 97d's ground-cover richness (`--foliage-rich`): per-voxel hashed species
    /// variety on tufts and grass/meadow tops -- straw stands, value jitter, a rare
    /// flower. Shader-side albedo only; worldgen geometry never moves.
    pub foliage_rich: bool,
    /// Batch 97e's canopy grain (`--canopy-relief`): clump-scale brightness relief and
    /// sun-facing modulation on leaves. Same default-off law, same albedo argument.
    pub canopy_relief: bool,
    /// A9 (`--caustics`): woven sun sheets on the flooded floor, read off the wave
    /// field's own first two octaves at the sun-projected waterline point. Default off --
    /// every open roadmap look entry builds dark until the by-eye pass verdict, so a flag
    /// not given renders the shipping frame bit-exactly.
    pub caustics: bool,
    /// A4 (`--glass-reflect`): trace a pane reflection with one real march instead of
    /// assuming a sky. Same default-off law: the arm costs one `water_reflect_leg` per
    /// glass pixel when on, nothing when off, and the verdict belongs to the eyes.
    pub glass_reflect: bool,
    /// A1 (`--canopy-lift <blocks>`): raise the shaft field's height read inside
    /// tree-holding columns by this many blocks, hiding the canopy the coarse fill saw
    /// through. Pure data: the field rebuilds on the same dirty path, no shader moves.
    pub canopy_lift: f32,
    /// A5 (`--tree-blue-noise`): thin the coarse canopy proxies to LOD 0's transmittance
    /// with a blue-noise rank field instead of stamping them solid, closing the cross-fade
    /// partition the LOD cliff comes from. Generator-side only; default off.
    pub tree_blue_noise: bool,
    /// A8 piece 4 (`--meadow-side`): `MEADOW_SIDE` at the coarse repaint instead of
    /// `MEADOW`. Worldgen-content class, same as A5 -- no pipeline, no flag bit.
    pub meadow_side: bool,
    /// G2 (`--grass-dense`): dense two-height tufts at LOD 0, blue-noise tuft proxies on
    /// the coarse shell, reeds around the wet banks; two new foliage ids (`GRASS_TALL`,
    /// `REEDS`) ride the tuft's `is_tuft` contract. Worldgen-content class, same as A5;
    /// default off, and the unarmed world is bit-identical by construction.
    pub grass_dense: bool,
    /// G2 (`--wind-sway`): the cross-quad planes lean with a two-sine traveling gust
    /// field, so the references' grass *moves*. Render-side spec bit, default off;
    /// counting as a G1 look-arm because it is one, but it was always destined to be
    /// judged on G2's geometry.
    pub wind_sway: bool,
    /// Sun disc (`--soft-shadows`): penumbra -- three cone taps around the centre
    /// shadow tap soften the edge the one-bit `shadow_ray` leaves razor-hard, with the
    /// tap budget scaled by blocker distance so contact stays hard and only the fringe
    /// pays. Render-side spec bit, default off; batch 102a.
    pub soft_shadows: bool,
    /// Opt-in material-after-lighting schedule; requires GPU A/B measurement.
    pub compact_shade_hit: bool,
    /// Opt-in architecture prototypes, configured before Play.
    pub isolate_glass: bool,
    pub shadow_pass: bool,
    pub legacy_lighting: bool,
    pub legacy_water: bool,
    pub soft_shadow_hq: bool,
    /// Diagnostic: 0 off, 1 normals, 2 visibility, 3 direct, 4 indirect, 5 water faces, 6 reflection, 7 refraction.
    pub surface_debug: u32,
    /// 0 default, 1 narrow, 2 wide. CLI-only startup specialization.
    pub sun_softness: u32,
    /// A2c (`--snell-bend`): inside Snell's window, trace the world the ray actually
    /// finds -- two segments at dispatch, the transmitted half refracted at the sea
    /// plane. Default off; `resolve` is untouched by construction.
    pub snell_bend: bool,
    /// A8 (`--shaft-texel <blocks>`): the envelope's quantisation. Shipped at 4 and never
    /// swept; finer is a sharper shadow edge AND a shorter window (the field is 512 texels a
    /// side either way), which is the trade `--reference` exists to read.
    pub shaft_texel: i32,
    /// A8 (`--tint-balance`): the balanced tint table and its atlas masks. Two of the six
    /// biomes' surfaces carry alpha-0 layers (desert's sand, tundra's lichen) and a third's
    /// multiplier is identity by construction (plains), so batch 12's ground tint reaches
    /// half the palette. Off like the rest of the batch-90 family: the by-eye pass sets it.
    pub tint_balance: bool,
    /// The biome field. `--no-biomes` collapses it to one entry -- plains everywhere, no
    /// snow line, no tree line, mountain amplitude 88 -- which is the pre-batch-9 world
    /// bit for bit, and the control for everything in batch 9.
    pub biomes: bool,
    /// Batch 14's ground cover. `--no-foliage` places none, which is the pre-batch-14 world
    /// bit for bit and the control for everything in batch 14.
    ///
    /// `--no-biomes` clears it too, for the same reason it clears the tint: that flag has to
    /// reproduce a build from before the biome field existed, and ground cover is scaled by a
    /// per-biome field. `WorldGen::with_options` is where the two are ANDed.
    pub foliage: bool,
    /// Batch 18's coarse meadow. `--no-meadow` paints none, which is the pre-batch-18 world
    /// bit for bit -- and unlike every control since batch 11 it is free by construction
    /// rather than by measurement, because the batch adds no shader code at all.
    ///
    /// ANDed with `foliage` in `WorldGen::with_options`: a world with no ground cover has
    /// no LOD 0 boundary for a meadow to hide.
    pub meadow: bool,
    /// Batch 60, roadmap R3. The probe bake weights each ground sample it gathers by the sky
    /// that sample can see, so ground in a ravine bounces less than the same ground in the
    /// open. `--no-probe-shadow` clears it and the bake produces batch 58's field exactly.
    ///
    /// **Free, and free by construction rather than by measurement**: the term is a multiply
    /// by a number that is 1.0 over flat ground, computed from height samples the horizon
    /// march has already taken, and it is paid in `stream::build` on the rayon workers rather
    /// than anywhere the frame can see.
    pub bounce_shadow: bool,
    /// Tint vegetation by the biome under it. `--no-tint` switches it off and is the
    /// control for everything in batch 12: the shader returns the atlas albedo untouched
    /// before the biome field is ever sampled, so the frame is the pre-batch-12 one bit for
    /// bit and costs what it cost then.
    ///
    /// `--no-biomes` clears it too. That flag's job is to reproduce a build from before the
    /// biome field existed, and a tint derived from that field is part of what it has to
    /// take away -- which is also what keeps it a whole-frame control and not just a
    /// worldgen one.
    pub tint: bool,
    /// How far the tint is taken, 0..1. A look knob, **not** the control -- zero still pays
    /// for the biome field on every tinted pixel.
    pub tint_strength: f32,
    /// Peak slope of the water micro-normals, dimensionless (batch 16). 0 is exactly the
    /// batch-8 sheet of glass, and unlike `tint_strength` it really **is** the control:
    /// `flags_from` clears `FLAG_WAVES` with it, so the field leaves `resolve` altogether
    /// rather than evaluating to nothing.
    pub wave_amp: f32,
    /// Blocks per period of the base wave octave; the other three are harmonics of it.
    pub wave_scale: f32,
    /// Periods per second the wave phases advance. It multiplies `frame.time`, which every
    /// headless mode pins to zero, so this changes the game and never a capture.
    pub wave_speed: f32,
    /// Largest slope the *traced* reflection's normal may take, as a tangent. Zero by
    /// measurement -- see `render::Waves::reflect_slope`. `--wave-clamp 0.02` is the A/B
    /// partner that shows why.
    pub wave_reflect_slope: f32,
    /// Whether the wave band limit reads the grazing angle (batch 19). `false` is
    /// `--no-wave-aniso`, the pre-batch-19 isotropic footprint, and it is the control: the
    /// field is filtered against the pixel's width across the view ray rather than against
    /// the much longer footprint the ray actually covers on a near-horizontal sea.
    pub wave_aniso: bool,
    /// Whether the wave amplitude ramps with water depth (batch 20). `false` is
    /// `--no-wave-shoal`, the pre-batch-20 sea, which gives a two-block lagoon the same swell
    /// as open ocean. Keyed to *depth* and never to camera distance -- see
    /// `render::FLAG_WAVE_SHOAL` for why that distinction is the whole design.
    pub wave_shoal: bool,
    /// Eight wave octaves subdividing the shipped band instead of four (batch 25).
    /// `--no-wave-fill` is the control and reproduces the pre-batch-25 sea bit for bit; see
    /// `render::FLAG_WAVE_FILL` for why the count is not the variable the name suggests.
    pub wave_fill: bool,
    /// Terrain-cast crepuscular rays (batch 35). `--no-terrain-shafts` is the control; being
    /// in `render::SPEC_MASK` it is free. **It is not a pre-batch-35 revert on its own since
    /// batch 37** -- pair it with `--no-distant-shadows`, the envelope's other reader. See
    /// `render::FLAG_TERRAIN_SHAFTS` for that and for what it changes about the two cloud
    /// controls, which is the half a bench here has to know.
    pub terrain_shafts: bool,
    /// Per-block texture permutation (batch 36). `--no-tex-variation` is the control and
    /// reproduces the pre-batch-36 frame bit for bit; being in `render::SPEC_MASK` it does
    /// so for free. Which layers may turn at all is `block::PERM_CLASS`, not this flag --
    /// this only decides whether the class is consulted.
    pub tex_variation: bool,
    /// Leaf cutouts (batch 38). `--no-leaf-cutout` is the control and reproduces the
    /// pre-batch-38 frame bit for bit; being in `render::SPEC_MASK` it does so for free.
    /// *Which* blocks are carved is `BlockDef::cutout` and not this flag -- this only decides
    /// whether the marcher looks.
    pub leaf_cutout: bool,
    /// Flat lighting for hits reached through water or glass (batch 45).
    /// `--no-flat-secondary` is the control and reproduces the pre-batch-45 frame bit for
    /// bit -- see `render::FLAG_FLAT_SECONDARY`.
    pub flat_secondary: bool,
    /// The submerged reach cut (batch 48). `--no-water-far` is the control and reproduces the
    /// pre-batch-48 frame bit for bit -- see `render::FLAG_WATER_FAR`.
    ///
    /// Nothing above water reads it: the flag it sets is ANDed with `FLAG_UNDERWATER`, so a
    /// run that never submerges cannot tell this from the control.
    pub water_far: bool,
    /// The dark-water rung of that cut (batch 53). `--no-water-dark` is the control and
    /// reproduces the pre-batch-53 frame bit for bit -- see `render::FLAG_WATER_DARK`.
    ///
    /// Nothing above water reads it either, and nothing *shallow* does: the tile has to still
    /// be under the sky flood's reach at `WATER_FAR_DIST`, which needs a camera far enough
    /// down or aimed far enough down for that to be true.
    pub water_dark: bool,
    /// Total internal reflection at the water surface, seen from below (batch 54).
    /// `--no-snell` is the control and reproduces the pre-batch-54 frame bit for bit -- see
    /// `render::FLAG_SNELL`.
    ///
    /// Nothing above water reads it: the term is only reached with `FLAG_UNDERWATER` set.
    pub snell: bool,
    /// Batch 55's probe tap: one hardware trilinear tap into the probe field, taken once per
    /// shaded surface and shading nothing. `--probe-tap` switches it on and it ships **off**.
    ///
    /// **It implies `--probe-fill 1.0` since batch 57**, which is what keeps it meaning what it
    /// meant: the texture holds a real bake in the shipping build, and pinning it back to a
    /// constant is the whole of why `amb * tap` is bit-exact.
    ///
    /// It exists to price roadmap L1's read side before L1 is designed, and it is the one
    /// switch here whose *off* state is the shipping build rather than the control -- see
    /// `render::FLAG_PROBE_TAP` for why a diagnostic that ships on would tax every later
    /// measurement.
    pub probe_tap: bool,
    /// Batch 56's removal (`--probe-ambient`), which **implies `--probe-tap`**.
    ///
    /// The probe field replaces `shade_hit`'s ambient rather than multiplying into it, and the
    /// terms a pre-composed directional field would make redundant leave the module. **The frame
    /// it renders is wrong on purpose** -- the field holds a constant -- so this is a build to
    /// measure and throw away, not a control and not a look.
    pub probe_ambient: bool,
    /// Batch 57's ambient cube: the baked probe lattice supplies `shade_hit`'s directional
    /// factor instead of `face_shade`'s per-normal constants. **Ships on**, and
    /// `--no-probe-cube` is the control -- see `render::FLAG_PROBE_CUBE`.
    pub probe_cube: bool,
    /// Batch 58's ground bounce: the ambient floor's colour and magnitude come from the baked
    /// probe lattice rather than from one authored constant. `--no-probe-bounce` is the
    /// control -- see `render::FLAG_PROBE_BOUNCE`.
    pub probe_bounce: bool,
    /// Batch 60's bounced sunlight. `--no-probe-sun` clears it and the ambient floor is the
    /// authored constant again -- see `render::FLAG_PROBE_SUN`, which carries why this is the
    /// answer to ADR 0001 rather than a new term.
    pub probe_sun: bool,
    /// Batch 60's louder bounce gain, `--probe-sun-high`. **Defaults off**, unlike every other
    /// probe flag here: it is a sweep rung and not a control -- see `render::FLAG_PROBE_SUN_HIGH`.
    pub probe_sun_high: bool,
    /// Batch 59's coloured block light: the flood carries a level per channel and `shade_hit`
    /// derives the tint from what reached the cell, instead of one channel wearing
    /// `BLOCK_TINT`. **Ships on**, and `--no-light-rgb` is the control -- see
    /// `render::FLAG_LIGHT_RGB`.
    pub light_rgb: bool,
    /// Batch 63's derived sky hue: the ambient's sky colour is integrated out of the sky model
    /// the renderer already draws, per face, instead of one authored constant. **Ships on**, and
    /// `--no-sky-tint` is the control -- see `render::FLAG_SKY_TINT`.
    pub sky_tint: bool,
    /// Batch 65's Fresnel-weighted sky specular: every opaque surface reflects the sky,
    /// weighted by Schlick at `GROUND_F0` and gated by the flood's own sky level. **Ships on**,
    /// and `--no-sky-specular` is the control -- see `render::FLAG_HI_SKY_SPECULAR`. It is the
    /// first control driven by the second specialization word rather than by `frame.flags`,
    /// which batch 63 filled.
    pub sky_specular: bool,
    /// Batch 72's full-chunk march: a ray that ignores water now walks a mixed root-FULL
    /// chunk's water to the first dry voxel instead of stopping at the chunk's face --
    /// roadmap P10's defect, 31,815 pixels at `coastline` measured at batch 53. **Ships
    /// on**, and `--no-full-march` is the control, reproducing the pre-batch-72 frame bit
    /// for bit; see `render::FLAG_HI_FULL_MARCH`.
    pub full_march: bool,
    /// What the probe field is filled with. 1.0 ships, and at 1.0 the tap is bit-exact --
    /// `amb * 1.0` is `amb` for every finite `amb` -- which is what makes the cost the only
    /// thing the tap reports. **Any other value is the paired positive claim**: bit-exact
    /// alone cannot be told from a tap that was never wired up, so `--probe-fill 0.5` has to
    /// move the frame or the measurement means nothing.
    /// What the probe field is filled with, when it is filled with anything at all.
    ///
    /// **`None` ships and means the field holds the bake**, which is batch 57. `Some(f)` pins
    /// every texel to `f` and switches the bake's uploads off, which is what keeps batch 55's
    /// and batch 56's diagnostics measuring the field their numbers were read off: at
    /// `Some(1.0)` `--probe-tap` is bit-exact again -- `amb * 1.0` is `amb` for every finite
    /// `amb` -- and any other value is that batch's paired positive claim, because bit-exact
    /// alone cannot be told from a tap that was never wired up.
    pub probe_fill: Option<f32>,
    /// Fill the probe field per texel instead of uniformly (`--probe-noise`). The build that
    /// rules out a driver compressing a constant texture into a cost no real field would pay.
    pub probe_noise: bool,
    /// Count `march`'s work instead of timing it (batch 53, `--march-stats`).
    ///
    /// Turns `render::FLAG_HEATMAP` on for a `--screenshot` run and prints what
    /// `render::MarchStats` read back afterwards. **The capture it writes is the heatmap**,
    /// not the frame, and the `atomicAdd` the flag enables is real work -- so this mode
    /// measures a traversal and never a millisecond, which is exactly what it is for: a
    /// count is the one reading a heat-soaked laptop cannot move.
    pub march_stats: bool,
    /// Glass transmits (batch 38b). `--no-glass` is the control and reproduces the
    /// pre-batch-38b *shading* bit for bit; being in `render::SPEC_MASK` it does so for free.
    ///
    /// **It is not a pure revert, and it is the only control in the table that is not.** The
    /// batch also made `block::GLASS` `opaque: false`, which is a world change `light.rs`
    /// bakes into a chunk and no pipeline override can undo, so at a vantage holding glass
    /// this reproduces the old shading over the new lighting. Where no glass has been placed
    /// the two coincide exactly, which is every vantage the fixture had before this batch.
    /// See `render::FLAG_GLASS` for the whole argument.
    pub glass: bool,
    /// Distant terrain shadows (batch 37). `--no-distant-shadows` is the control and
    /// reproduces the pre-batch-37 frame bit for bit; being in `render::SPEC_MASK` it does so
    /// for free. See `render::FLAG_DISTANT_SHADOWS` for what it also changes about
    /// `--no-terrain-shafts`, which is the half a bench here has to know: the envelope has two
    /// readers now and that control speaks for only one of them.
    pub distant_shadows: bool,
    /// The short shadow march under water (batch 41). `--no-water-shadow-cut` is the control
    /// and reproduces the pre-batch-41 frame bit for bit; being in `render::SPEC_MASK` it does
    /// so for free.
    ///
    /// **It is the one control whose look cost is paid by a different batch's feature**, which
    /// is the half a bench here has to know: `shade_hit` hands the interval past this budget
    /// to batch 37's light envelope, so with `--no-distant-shadows` also set the truncation
    /// stops being nearly invisible and starts deleting a real shadow. The two compose, and
    /// `crate::harness::IMPLIES` carries that as a claim rather than a sentence. See
    /// `render::WATER_SHADOW_DIST`.
    pub water_shadow_cut: bool,
    /// A thinner canopy (batch 43). `--no-leaf-thin` is the control and reproduces the
    /// pre-batch-43 frame bit for bit; being in `render::SPEC_MASK` it does so for free.
    ///
    /// *How much* of a leaf block survives the carve is `render::LEAF_FILL`, swept by eye
    /// against a wooded crest and a canopy seen from above; this only chooses between that
    /// number and the one batch 38 authored. **Deliberately not nested under `leaf_cutout`**
    /// -- see `render::FLAG_LEAF_THIN`.
    pub leaf_thin: bool,
    pub lod: LodConfig,
    pub width: u32,
    pub height: u32,
    /// Internal render resolution as a fraction of the window.
    pub render_scale: f32,
    /// Static sub-pixel offset for every primary ray, in pixels. `None` lets the temporal
    /// pass walk its own eight-phase sequence; setting it pins every frame to one offset,
    /// which is what makes a jittered capture comparable to an unjittered one.
    pub jitter: Option<(f32, f32)>,
    /// Temporal anti-aliasing.
    pub taa: bool,
    /// Frames a `--screenshot` accumulates before it captures. The whole verification story
    /// in the docs is deterministic single-frame A/B, and an accumulating resolve only stays
    /// deterministic if the frame count is fixed and stated.
    pub taa_frames: usize,
    /// Extra mip bias in `resolve`, on top of the render-scale one the renderer derives.
    pub mip_bias: f32,
    /// Side of the sub-pixel jitter grid a `--screenshot` converges a **reference** over; 0 is
    /// off and captures one frame the ordinary way. `--reference 16` renders 256 frames, one
    /// per grid cell, averages them in linear and writes the mean plus the two half-averages
    /// the noise floor is computed from.
    ///
    /// A reference is the only thing in this project allowed to rank a look change, and batch
    /// 21 is why the halves are not optional: a 64-sample reference scored a sweep cleanly and
    /// confidently while 95% of its signal was its own sampling noise. A reference that cannot
    /// state its own floor should not be allowed to rank anything, so this mode always states
    /// it.
    pub reference: u32,
    pub fov_degrees: f32,
    pub vsync: bool,
    pub shadows: bool,
    pub hiz: bool,
    pub hud: bool,
    /// Batch 70's idle repaint: when a presented frame asks nothing new of the renderer --
    /// camera, `World::version`, the pipeline words and the sun's tick all unchanged -- the
    /// window loop presents the last converged frame again instead of rendering it again,
    /// at roughly the blit's cost. **`--no-idle-repaint` restores the pre-67 cadence where
    /// every presented frame is rendered, and is the control**: the two arms draw the same
    /// world from the same buffers, one of them orders of magnitude cheaper while the
    /// player stands still. A headless mode never engages it -- `app.rs` is its only
    /// caller, which is what keeps every capture and bench in the fixture out of it. The
    /// rule is in `src/idle.rs`; the budget and the rejected alternatives are the batch's
    /// row in `docs/ledger.md`.
    pub idle_repaint: bool,
    /// The hotbar slot the player starts on, and therefore the one `--hud` draws highlighted.
    ///
    /// **Four, because that is the slot every `--hud` capture has shown since the HUD
    /// existed** -- it was a literal `4` passed to `draw_hud` from the capture loop, which is
    /// to say a picture claiming a selection no player held. Routing it through the player
    /// makes the picture and the journal agree about one value, and keeping the number at 4
    /// is what makes batch 32's control exactly zero rather than nearly zero. `--resume`
    /// overrides it, the way a loaded journal overrides everything else about the player.
    pub hotbar: usize,
    /// `--bar`'s ten block ids, still as text. Batch 34.
    ///
    /// **Unparsed here on purpose.** Every refusal it can earn -- a bad id, the wrong count,
    /// a block no bar may hold, a journal that already carries a bar -- is a sentence, and
    /// `journal::open` is where this crate says such sentences. Parsing it here would put
    /// half the rule in a parser with no error channel and the other half one file over.
    pub bar: Option<String>,
    pub demo_edits: bool,
    /// Build batch 38b's glasshouse instead of the hut. A *second* structure and not a
    /// variant of the first, because `--demo-edits` is the `edits` vantage and half a dozen
    /// batches are measured against that reference -- see `scene::build_glass_structure`.
    pub demo_glass: bool,
    /// Build batch 59's lamp chamber instead of either of the above. A *third* structure for
    /// `demo_glass`'s reason one line up, and one sharper: the other two are references half a
    /// dozen batches are measured against, and this one has to hold blocks whose ids did not
    /// exist before this batch -- so it is the one structure no earlier binary can be handed.
    /// See `scene::build_lamp_chamber` and `docs/harness.md` on what that costs a sweep.
    pub demo_lamps: bool,
    /// Replay a saved edit journal after worldgen (batch 29). `None` is a world that is
    /// exactly what the seed says it is, which is every capture taken before this batch and
    /// every one taken since without the flag.
    ///
    /// A **missing file is an error**, not an empty journal, for the same reason the seed
    /// stored in the header is checked: a mistyped path would otherwise render an unedited
    /// world and say nothing about it, and in a sweep whose pass condition is `0 pixels
    /// differing` that is indistinguishable from success.
    pub load_edits: Option<String>,
    /// Write the journal -- the loaded entries and this session's alike -- when the mode
    /// ends. Playing a persistent world is both flags at one path.
    ///
    /// Two flags rather than one because each half has to be separately runnable: the A/B
    /// that proves a reloaded world is the world you left is `--demo-edits --save-edits f`
    /// against `--load-edits f`, and a single flag that did both could only ever compare a
    /// build against itself.
    pub save_edits: Option<String>,
    /// Start the player where the loaded journal says, rather than at the spawn column or
    /// the vantage the camera flags describe. Batch 32.
    ///
    /// **The window does this without being asked and a headless mode does not**, and that
    /// boundary is drawn at the mode rather than per field, because it is the mode that
    /// decides who owns the camera. A window has nothing else that claims one, so a loaded
    /// journal is the whole answer to where the player is. A capture has `--cam-height`,
    /// `--cam-yaw`, `--cam-pitch` and `--cam-submerge`, all measured from `spawn_position`,
    /// and all fourteen vantages of the measurement fixture are those flags: a journal that
    /// silently moved the camera would turn every named vantage into "wherever the file
    /// says", which is not a vantage at all. So a capture asks.
    ///
    /// It is refused when there is nothing to resume from -- no `--load-edits`, or a journal
    /// with no player block in it -- rather than falling back to the default camera, which
    /// would be a frame that looks exactly like a frame.
    pub resume: bool,
    /// A world on disk rather than a flag: load `PATH` if it is there, **append each edit to
    /// it as the edit lands**, and write it back compacted when the mode ends. Batch 33.
    ///
    /// It is not `--load-edits PATH --save-edits PATH` spelled shorter, and the difference is
    /// the whole feature. Those two are a *measurement* pair -- each half separately runnable
    /// so that one binary can render `--demo-edits --save-edits f` against `--load-edits f`,
    /// which is why a missing file is an error there. This is a *world*, and a world you have
    /// not played yet is a file that is not there, so a missing one starts a new world and
    /// says so out loud. **The sentence is the only guard left against a mistyped path**: the
    /// two outcomes of this flag are "loaded your world" and "started a different one", and
    /// nothing in the file system can tell them apart on your behalf.
    ///
    /// The append log hangs off this flag and not off `--save-edits`, so every A/B taken
    /// through those two writes the same bytes it wrote in batch 29.
    pub edits: Option<String>,
    /// End the mode without the exit save -- which is exactly what a killed session looks
    /// like to the file. Batch 33's measurement machinery, and the only way a fixture can
    /// see the append log at all.
    ///
    /// A run that ends by writing the whole journal leaves the same file whether it appended
    /// as it went or not, so without this flag the crash-safety half of `--edits` is
    /// unobservable: `lessons.md`'s "a fix nothing exercises is not a fix", aimed at a
    /// feature whose entire purpose is what happens when the ordinary path does not run.
    pub no_exit_save: bool,
    /// Light floor in linear radiance, so unlit caves are dim rather than pure black.
    pub ambient: f32,
    /// Exponential height fog and the scattering lobe it shares with the sky.
    pub fog: Fog,
    /// The cloud deck. `--cloud-cover 0` switches it off and is the control for everything
    /// in batch 10: the sky function returns early, so the frame is the pre-batch-10 one
    /// bit for bit and costs what it cost then.
    pub clouds: Clouds,
    /// G1's two sky levers (batch 95, roadmap G1). Both default to 0, so the pre-95 sky
    /// is the control and is bit-exact; parse-clamped 0..1 like every other knob.
    pub sky: SkyLook,
    /// Crepuscular rays. `--godray-strength 0` switches the shadowing of the haze's
    /// scattering lobe off and `--cloud-shadow 0` switches the deck's occlusion of the
    /// ground off; together they are the control for everything in batch 11, and each is
    /// bit-exact on its own.
    pub godrays: GodRays,
    pub ao: bool,
    pub physics: bool,
    /// Fraction of the day elapsed; 0.25 is morning, 0.5 noon.
    pub time_of_day: f32,
    pub day_length_seconds: f32,
    pub freeze_time: bool,
    pub stream_budget_ms: f32,
    /// Screenshot camera: height above the spawn column, and look angles in degrees.
    pub cam_height: f32,
    pub cam_yaw: f32,
    pub cam_pitch: f32,
    /// Degrees of yaw per accumulated frame during a `--screenshot`, ending on `cam_yaw`.
    /// Zero is a still camera. Non-zero is the only way to put real motion behind the
    /// captured frame, which is the only way to see what the temporal pass does with it.
    pub cam_spin: f32,
    /// Blocks of forward travel per accumulated frame during a `--screenshot`, ending on
    /// the configured position. Rotation reprojects almost perfectly and disoccludes only
    /// at the screen edge; translation disoccludes at every silhouette in the frame, which
    /// is the case the temporal pass actually has to survive.
    pub cam_dolly: f32,
    /// Put the screenshot camera this many blocks below the surface of the nearest water,
    /// which is the only way to reach the underwater medium headlessly. Zero is off.
    ///
    /// **Negative floats the camera that far *above* the surface instead**, which is the only
    /// way to reach the one vantage that checks a water reflection: from just above the
    /// water the reflection is very nearly the mirror image of the sky about the horizon, so
    /// the frame's own symmetry is the test. Batch 10 needed it and there was no way to get
    /// there.
    pub cam_submerge: f32,
    /// The value of `frame.time` at the **captured** frame, in seconds. Zero is the default
    /// and every capture taken before batch 23 is at zero, so the whole existing corpus is
    /// unchanged by this field existing.
    ///
    /// `wave_speed` and `cloud_speed` multiply `frame.time` and nothing else does, so this is
    /// the only knob that moves the sea's phase or the deck's drift in a headless render.
    /// Until batch 23 both headless paths hardcoded `0.0`, which meant the *moving* sea and
    /// the *drifting* deck were invisible to every capture, test and hash in the tree -- the
    /// only way to look at either was to run the game, which is how batch 10's parallax-free
    /// reflection shipped and why batches 10 and 16 both had to leave ghosting unmeasured.
    ///
    /// It is a still capture at a chosen moment. Two captures at two phases are two seas;
    /// neither has a *moving* field behind it, which is what `anim_rate` is for.
    pub anim_time: f32,
    /// Seconds of `frame.time` per accumulated frame during a `--screenshot`, ending on
    /// `anim_time`. Zero is a frozen field.
    ///
    /// The exact shape of `cam_spin` and `cam_dolly` and for the exact reason: the temporal
    /// pass can only be asked what it does with motion if there is real motion behind the
    /// captured frame. The difference is *what* moves. `cam_dolly` moves the camera, which
    /// disoccludes at every silhouette; this moves the field under a camera that is standing
    /// still, which disoccludes nothing and reprojects perfectly -- so anything it ghosts is
    /// the field's own history and not a reprojection failure. That is the case batches 10
    /// and 16 could not reach, and the two are separable only because they are two flags.
    ///
    /// Read by `--screenshot` alone. `--reference` converges a *still* and so holds the phase
    /// at `anim_time`, and `--bench-frames` has never swept `cam_spin` either: a bench is a
    /// fixed configuration, and `--anim-time` is how it picks which one.
    pub anim_rate: f32,
}

impl Config {
    /// The journal to read, and whether a file that is not there is a refusal.
    ///
    /// Three flags reach one question, and the boolean is the only thing that separates
    /// them. `--load-edits` is a *measurement* input and a missing file is a typo, which has
    /// to be an error: `errors.md`'s rule is that a sweep whose pass condition is `0 pixels
    /// differing` cannot tell an unedited world from a passing one. `--edits` is a *world*
    /// and a missing file is a world nobody has played, which is the ordinary first run.
    ///
    /// `--edits` wins when both are given rather than being refused, and the refusal lives
    /// at `journal::open` instead -- one place that can say why, in a sentence, on the way
    /// out. A precedence rule here would be a second answer to the same question.
    pub fn journal_in(&self) -> Option<(&Path, bool)> {
        match (&self.edits, &self.load_edits) {
            (Some(p), _) => Some((Path::new(p), false)),
            (None, Some(p)) => Some((Path::new(p), true)),
            (None, None) => None,
        }
    }

    /// Where the whole journal is written when the mode ends, compacted and sorted.
    ///
    /// `--no-exit-save` reads as `None` **here**, at the one place every mode already asks
    /// this question, rather than as a branch at each of the three call sites -- which is
    /// the argument `journal::save_if_asked` makes about its own signature, one file over.
    pub fn journal_out(&self) -> Option<&Path> {
        if self.no_exit_save {
            return None;
        }
        self.edits
            .as_deref()
            .or(self.save_edits.as_deref())
            .map(Path::new)
    }
}


impl Config {
    /// The generator this configuration asks for.
    ///
    /// **It exists because there were four copies of it and a fifth was about to be written.**
    /// `app`, `screenshot`, `bench` and `bench_terrain` each built the same five-argument
    /// `with_options` call, so batch 60 adding one more generator option would have had to
    /// touch all four and would have been one edit away from a build where the control worked
    /// in a screenshot and not in a bench -- which is exactly the class of defect
    /// `docs/harness.md` calls indistinguishable from success.
    pub fn worldgen(&self) -> crate::worldgen::WorldGen {
        let mut gen = crate::worldgen::WorldGen::with_options(
            self.seed,
            self.sea_level,
            self.biomes,
            self.foliage,
            self.meadow,
        );
        // Not a `with_options` parameter: it changes the probe bake rather than the world, and
        // every one of the test suite's own construction sites wants the default.
        gen.bounce_shadow = self.bounce_shadow;
        // Same reason a second time: A5's arm changes the world's shape rather than its
        // compile, so it rides the setter lane rather than growing `with_options` a param.
        gen.tree_blue_noise = self.tree_blue_noise;
        // And a third: A8's piece 4 is one id inside `coarse_meadow`, nothing else.
        gen.meadow_side = self.meadow_side;
        // And a fourth: batch 101's G2 -- the setter lane the A5 lineage already cut.
        gen.grass_dense = self.grass_dense;
        gen
    }
}


impl Config {
    /// The blit selector word: 0 none, 1 warm, 2 cine (97b). One word, the A10
    /// one-uniform rule -- the three writers hand `blit.rs` this and nothing else, so an
    /// unknown combination cannot spell a fourth cast at a call site.
    pub fn grade_value(&self) -> u32 {
        self.grade_warm as u32 | u32::from(self.grade_cine) << 1
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            seed: 1337,
            sea_level: crate::worldgen::SEA_LEVEL,
            water_absorb: 1.0,
            // **0.0 ships since batch 26**, and 0.2 before it was batch 21b's deliberate
            // choice rather than its oversight: 21b built this flattening first, found it
            // moved 0 pixels at fourteen vantages -- the null that revealed the glass-layer
            // stride -- and left the lever in the roadmap for 21a rather than spending it on
            // a batch that had just disproved it. Batch 26 measured the lever and took it:
            // a periodic stipple worth 27-34% of `speckle` at `coastline` and `open-sea`
            // against a smooth signal of under half a code value, and the whole of batch
            // 21's diamond weave at `lattice`. Reasoning in
            // `docs/water.md`; `--water-mottle 0.2` reproduces the old sea.
            water_mottle: 0.0,
            water_reflect: true,
            water_refract: true,
            shore_wet: true,
            shore_foam: true,
            water_sec: true,
            water_sec_scale: 2,
            tone_map_aces: false,
            grade_warm: false,
            sky_cool: false,
            grade_cine: false,
            grade_strength: 1.0,
            water_look: false,
            foliage_rich: false,
            canopy_relief: false,
            caustics: false,
            glass_reflect: false,
            canopy_lift: 0.0,
            tree_blue_noise: false,
            meadow_side: false,
            grass_dense: false,
            wind_sway: false,
            soft_shadows: false,
            compact_shade_hit: false,
            isolate_glass: false,
            shadow_pass: true,
            legacy_lighting: false, legacy_water: false, soft_shadow_hq: false,
            surface_debug: 0, sun_softness: 0,
            snell_bend: false,
            shaft_texel: 4,
            tint_balance: false,
            biomes: true,
            foliage: true,
            meadow: true,
            bounce_shadow: true,
            tint: true,
            tint_strength: 1.0,
            wave_amp: 0.12,
            wave_scale: 32.0,
            wave_speed: 0.32,
            wave_reflect_slope: 0.0,
            wave_aniso: true,
            wave_shoal: true,
            wave_fill: true,
            terrain_shafts: true,
            tex_variation: true,
            leaf_cutout: true,
            flat_secondary: true,
            water_far: true,
            water_dark: true,
            snell: true,
            probe_tap: false,
            probe_ambient: false,
            probe_cube: true,
            probe_bounce: true,
            probe_sun: true,
            probe_sun_high: false,
            light_rgb: true,
            sky_tint: true,
            sky_specular: true,
            full_march: true,
            probe_fill: None,
            probe_noise: false,
            march_stats: false,
            glass: true,
            distant_shadows: true,
            water_shadow_cut: true,
            leaf_thin: true,
            lod: LodConfig::default(),
            width: 1600,
            height: 900,
            render_scale: 1.0,
            jitter: None,
            taa: true,
            taa_frames: 32,
            mip_bias: 0.0,
            reference: 0,
            fov_degrees: 70.0,
            vsync: true,
            shadows: true,
            hiz: true,
            hud: false,
            idle_repaint: true,
            hotbar: 4,
            bar: None,
            demo_edits: false,
            demo_glass: false,
            demo_lamps: false,
            load_edits: None,
            save_edits: None,
            resume: false,
            edits: None,
            no_exit_save: false,
            ambient: 0.08,
            fog: Fog::default(),
            clouds: Clouds::default(),
            sky: SkyLook::default(),
            godrays: GodRays::default(),
            ao: true,
            physics: true,
            time_of_day: 0.30,
            day_length_seconds: 600.0,
            freeze_time: false,
            stream_budget_ms: 2.0,
            cam_height: 40.0,
            cam_yaw: 40.0,
            cam_pitch: -14.0,
            cam_spin: 0.0,
            cam_dolly: 0.0,
            cam_submerge: 0.0,
            anim_time: 0.0,
            anim_rate: 0.0,
        }
    }
}

pub enum Mode {
    Run,
    Screenshot { path: String },
    /// Batch 98: the lookbook -- `lookbook::VIEWS` x `lookbook::COMBOS` through the
    /// screenshot core, composed into one labeled PNG so the five arms' by-eye verdict
    /// can be assigned without a human keeping score. One file, flags already baked
    /// into their own tiles.
    Lookbook { dir: String },
    BenchTerrain { chunks: usize },
    BenchFrames { frames: usize },
    /// Compare two saved frames without rendering anything: the difference numbers, the
    /// bbox every differing pixel sits in, and optionally a per-cell grid so a pop says
    /// *where* it is, not just that it is. Roadmap D2b's missing instrument: `lod` has no
    /// declared crops, and the crop a spike needs is the one this prints.
    Diff {
        a: String,
        b: String,
        grid: Option<(u32, u32)>,
        crop: Option<[f32; 4]>,
    },
}

/// `0.25,-0.5`, or one number for both axes.
fn parse_pair(s: &str) -> Option<(f32, f32)> {
    let (a, b) = s.split_once(',').unwrap_or((s, s));
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

/// What [`parse_args`] prints for an argument it does not recognise.
///
/// Named once and shared with the measurement fixture, the way
/// [`crate::render::TRUNCATION_MARK`] is, and for a sharper reason than that one. The
/// parser **ignores** what it cannot read and the process still exits 0, so a control
/// whose name is misspelled produces a capture identical to one with no flag at all --
/// and in a bit-exactness sweep, where the expected answer *is* `0 pixels differing`,
/// that is the one failure mode which looks exactly like success. Every claim tested
/// that way would pass, and pass vacuously. `harness::render` refuses such a capture,
/// which keeps working only while both sides of the message are this constant.
pub const UNKNOWN_ARG_MARK: &str = "ignoring unknown argument";

pub fn parse_args() -> (Config, Mode) {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (cfg, mode, unknown) = parse_from(&args);
    for a in unknown {
        eprintln!("{UNKNOWN_ARG_MARK}: {a}");
    }
    (cfg, mode)
}

/// The parser proper, with the process's environment factored out and the unrecognised
/// arguments handed back rather than printed.
///
/// Both halves of that are for `tests/harness.rs`, which runs every argument list in the
/// vantage table through here and fails on anything this does not recognise. That test is what
/// stops the fixture from outliving a flag: before batch 22 a vantage was a sentence in a
/// document, and nothing anywhere could notice when it stopped being runnable.
pub fn parse_from(args: &[String]) -> (Config, Mode, Vec<String>) {
    let mut cfg = Config::default();
    let mut mode = Mode::Run;
    let mut unknown = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let next = |i: &mut usize| -> String {
            *i += 1;
            args.get(*i).cloned().unwrap_or_default()
        };
        match a {
            "--screenshot" => mode = Mode::Screenshot { path: next(&mut i) },
            "--lookbook" => mode = Mode::Lookbook { dir: next(&mut i) },
            "--bench-frames" => {
                mode = Mode::BenchFrames {
                    frames: next(&mut i).parse().unwrap_or(120),
                }
            }
            "--diff" => {
                // The pair first, then any of its options; anything unrecognised takes the
                // same `unknown` path as a top-level flag, because `UNKNOWN_ARG_MARK` is
                // the load-bearing half of `harness::render`'s bargain -- a misspelled
                // option silently dropping to its default is the one failure that passes.
                let (a, b) = (next(&mut i), next(&mut i));
                let (mut grid, mut crop) = (None, None);
                while args.get(i + 1).is_some_and(|v| v.starts_with("--")) {
                    i += 1;
                    match args[i].as_str() {
                        "--grid" => {
                            let v = next(&mut i);
                            if let Some((gx, gy)) = parse_pair(&v) {
                                grid = Some((gx.max(1.0) as u32, gy.max(1.0) as u32));
                            }
                        }
                        "--crop" => {
                            let v = next(&mut i);
                            let q: Vec<f32> =
                                v.split(',').filter_map(|t| t.trim().parse().ok()).collect();
                            if q.len() == 4 {
                                crop = Some([q[0], q[1], q[2], q[3]]);
                            }
                        }
                        _ => {
                            i -= 1;
                            break;
                        }
                    }
                }
                mode = Mode::Diff { a, b, grid, crop };
            }
            "--bench-terrain" => {
                mode = Mode::BenchTerrain {
                    chunks: next(&mut i).parse().unwrap_or(64),
                }
            }
            "--seed" => cfg.seed = next(&mut i).parse().unwrap_or(cfg.seed),
            "--sea-level" => cfg.sea_level = next(&mut i).parse().unwrap_or(cfg.sea_level),
            "--no-water" => cfg.sea_level = 0,
            "--water-absorb" => cfg.water_absorb = next(&mut i).parse().unwrap_or(cfg.water_absorb),
            "--water-mottle" => { let _ = next(&mut i); cfg.water_mottle=0.0; eprintln!("water mottle is fixed at zero to keep the atlas layer flat"); },
            "--no-water-reflect" => cfg.water_reflect = false,
            "--no-water-refract" => cfg.water_refract = false,
            "--no-shore-wet" => cfg.shore_wet = false,
            "--no-shore-foam" => cfg.shore_foam = false,
            "--no-water-sec" => cfg.water_sec = false,
            "--water-sec-scale" => {
                let s: u32 = next(&mut i).parse().unwrap_or(cfg.water_sec_scale);
                cfg.water_sec_scale = match s {
                    1 | 2 | 4 => s,
                    _ => cfg.water_sec_scale,
                };
            }
            // A10: unknown shoulders fall to `knee` instead of rejecting -- a "tone map"
            // the engine won't run silently is worse than the spelling it did not get.
            "--tone-map" => {
                // Unknown shoulders fall to `knee`: a tone map the engine won't run
                // silently is worse than the spelling it did not get.
                cfg.tone_map_aces = next(&mut i) == "aces";
            }
            // G1: same spelling philosophy as `--tone-map` -- an unknown cast falls to the
            // one the engine has always shipped, loudly parseable rather than rejected.
            "--grade" => {
                // 95's warm cast and 97b's cine print are one selector word; `none`
                // turns both off and an unknown spelling falls to the shipped cast for
                // the parser's standing reason.
                let v = next(&mut i);
                cfg.grade_warm = v == "warm";
                cfg.grade_cine = v == "cine";
            }
            // 101's dial (the cine sweep verdict's fix): 1.0 keeps the grade exact by
            // construction; below it the graded pixel lerps back toward the ungraded
            // one. An unparseable value falls to 1.0 like the other float knobs.
            "--grade-strength" => {
                cfg.grade_strength = next(&mut i).parse().unwrap_or(1.0);
            }
            "--sky-cool" => cfg.sky_cool = true,
            "--water-look" => cfg.water_look = true,
            "--foliage-rich" => cfg.foliage_rich = true,
            "--canopy-relief" => cfg.canopy_relief = true,
            "--caustics" => cfg.caustics = true,
            "--glass-reflect" => cfg.glass_reflect = true,
            "--canopy-lift" => {
                let v: f32 = next(&mut i).parse().unwrap_or(0.0);
                cfg.canopy_lift = v.max(0.0);
            }
            "--tree-blue-noise" => cfg.tree_blue_noise = true,
            "--meadow-side" => cfg.meadow_side = true,
            "--grass-dense" => cfg.grass_dense = true,
            "--wind-sway" => cfg.wind_sway = true,
            "--soft-shadows" => cfg.soft_shadows = true,
            "--no-soft-shadows" => cfg.soft_shadows = false,
            "--no-wind-sway" => cfg.wind_sway = false,
            "--no-water-look" => cfg.water_look = false,
            "--no-glass-reflect" => cfg.glass_reflect = false,
            "--isolate-glass" => cfg.isolate_glass = true,
            "--no-isolate-glass" => cfg.isolate_glass = false,
            "--legacy-lighting" => cfg.legacy_lighting = true,
            "--repaired-lighting" => cfg.legacy_lighting = false,
            "--legacy-water" => cfg.legacy_water = true,
            "--repaired-water" => cfg.legacy_water = false,
            "--soft-shadow-hq" => cfg.soft_shadow_hq = true,
            "--no-soft-shadow-hq" => cfg.soft_shadow_hq = false,
            "--sun-softness" => {
                cfg.sun_softness = match next(&mut i).as_str() {
                    "narrow" => 1, "wide" => 2, "normal" => 0,
                    _ => { eprintln!("--sun-softness expects narrow, normal or wide; using normal"); 0 }
                };
            }
            "--surface-debug" => {
                cfg.surface_debug = match next(&mut i).as_str() {
                    "off" => 0, "normals" => 1, "shadow" => 2, "direct" => 3,
                    "indirect" => 4, "water-faces" => 5, "reflection" => 6, "refraction" => 7,
                    _ => { eprintln!("Unknown --surface-debug mode; using off"); 0 }
                };
            }
            "--shadow-pass" => cfg.shadow_pass = true,
            "--no-shadow-pass" => { cfg.shadow_pass = true; eprintln!("--no-shadow-pass retired: primary shadows now always use the dedicated mask"); },
            "--compact-shade-hit" => cfg.compact_shade_hit = true,
            "--no-compact-shade-hit" => cfg.compact_shade_hit = false,
            "--snell-bend" => cfg.snell_bend = true,
            "--shaft-texel" => {
                let v: i32 = next(&mut i).parse().unwrap_or(4);
                cfg.shaft_texel = v.clamp(1, 64);
            }
            // G1 (batch 95): four data-only sky knobs, same uniform class as A1's lift.
            "--cloud-patch" => {
                let v: f32 = next(&mut i).parse().unwrap_or(0.0);
                cfg.clouds.patch = v.clamp(0.0, 1.0);
            }
            "--cloud-relief" => {
                let v: f32 = next(&mut i).parse().unwrap_or(0.0);
                cfg.clouds.relief = v.clamp(0.0, 1.0);
            }
            "--haze-warm" => {
                let v: f32 = next(&mut i).parse().unwrap_or(0.0);
                cfg.sky.haze_warm = v.clamp(0.0, 1.0);
            }
            "--zenith-deep" => {
                let v: f32 = next(&mut i).parse().unwrap_or(0.0);
                cfg.sky.zenith_deep = v.clamp(0.0, 1.0);
            }
            "--tint-balance" => cfg.tint_balance = true,
            "--no-biomes" => cfg.biomes = false,
            "--no-foliage" => cfg.foliage = false,
            "--no-meadow" => cfg.meadow = false,
            "--no-probe-shadow" => cfg.bounce_shadow = false,
            "--no-tint" => cfg.tint = false,
            "--tint-strength" => {
                cfg.tint_strength = next(&mut i).parse().unwrap_or(cfg.tint_strength)
            }
            "--wave-amp" => cfg.wave_amp = next(&mut i).parse().unwrap_or(cfg.wave_amp),
            "--no-waves" => cfg.wave_amp = 0.0,
            "--wave-scale" => cfg.wave_scale = next(&mut i).parse().unwrap_or(cfg.wave_scale),
            "--wave-speed" => cfg.wave_speed = next(&mut i).parse().unwrap_or(cfg.wave_speed),
            "--no-wave-aniso" => cfg.wave_aniso = false,
            "--no-wave-shoal" => cfg.wave_shoal = false,
            "--no-wave-fill" => cfg.wave_fill = false,
            "--no-terrain-shafts" => cfg.terrain_shafts = false,
            "--no-tex-variation" => cfg.tex_variation = false,
            "--no-leaf-cutout" => cfg.leaf_cutout = false,
            "--no-flat-secondary" => cfg.flat_secondary = false,
            "--no-water-far" => cfg.water_far = false,
            "--no-water-dark" => cfg.water_dark = false,
            "--no-snell" => cfg.snell = false,
            "--probe-tap" => cfg.probe_tap = true,
            // Implies `--probe-tap`: the ambient has to come from somewhere, and the removal
            // without the tap measures a shader that reads no light at all.
            "--probe-ambient" => {
                cfg.probe_ambient = true;
                cfg.probe_tap = true;
            }
            "--no-probe-cube" => cfg.probe_cube = false,
            "--no-probe-bounce" => cfg.probe_bounce = false,
            "--no-probe-sun" => cfg.probe_sun = false,
            "--probe-sun-high" => cfg.probe_sun_high = true,
            "--no-light-rgb" => cfg.light_rgb = false,
            "--no-sky-tint" => cfg.sky_tint = false,
            "--no-sky-specular" => cfg.sky_specular = false,
            "--no-full-march" => cfg.full_march = false,
            "--probe-fill" => cfg.probe_fill = next(&mut i).parse().ok(),
            "--probe-noise" => cfg.probe_noise = true,
            "--march-stats" => cfg.march_stats = true,
            "--no-glass" => cfg.glass = false,
            "--no-distant-shadows" => cfg.distant_shadows = false,
            "--no-water-shadow-cut" => cfg.water_shadow_cut = false,
            "--no-leaf-thin" => cfg.leaf_thin = false,
            "--wave-clamp" => {
                cfg.wave_reflect_slope = next(&mut i).parse().unwrap_or(cfg.wave_reflect_slope)
            }
            "--width" => cfg.width = next(&mut i).parse().unwrap_or(cfg.width),
            "--height" => cfg.height = next(&mut i).parse().unwrap_or(cfg.height),
            "--scale" => cfg.render_scale = next(&mut i).parse().unwrap_or(cfg.render_scale),
            "--jitter" => cfg.jitter = parse_pair(&next(&mut i)).or(cfg.jitter),
            "--taa-frames" => cfg.taa_frames = next(&mut i).parse().unwrap_or(cfg.taa_frames),
            "--mip-bias" => cfg.mip_bias = next(&mut i).parse().unwrap_or(cfg.mip_bias),
            "--reference" => cfg.reference = next(&mut i).parse().unwrap_or(8),
            "--no-taa" => cfg.taa = false,
            "--fov" => cfg.fov_degrees = next(&mut i).parse().unwrap_or(cfg.fov_degrees),
            "--view-distance" => {
                cfg.lod.view_distance = next(&mut i).parse().unwrap_or(cfg.lod.view_distance)
            }
            "--max-lod" => cfg.lod.max_lod = next(&mut i).parse().unwrap_or(cfg.lod.max_lod),
            "--streaming-factor" => cfg.lod.factor = next(&mut i).parse().unwrap_or(cfg.lod.factor),
            "--fade-band" => cfg.lod.fade_band = next(&mut i).parse().unwrap_or(cfg.lod.fade_band),
            "--fade-schedule" => {
                cfg.lod.fade_schedule = match next(&mut i).as_str() {
                    "smoothstep" => crate::lod::FadeSchedule::Smoothstep,
                    "smootherstep" => crate::lod::FadeSchedule::Smootherstep,
                    _ => cfg.lod.fade_schedule,
                }
            }
            "--no-fade" => cfg.lod.cross_fade = false,
            "--no-shadow-share" => cfg.lod.shadow_share = false,
            "--no-offscreen-shadows" => cfg.lod.offscreen_shadows = false,
            "--time" => {
                cfg.time_of_day = next(&mut i).parse().unwrap_or(cfg.time_of_day);
                cfg.freeze_time = true;
            }
            "--cam-height" => cfg.cam_height = next(&mut i).parse().unwrap_or(cfg.cam_height),
            "--cam-yaw" => cfg.cam_yaw = next(&mut i).parse().unwrap_or(cfg.cam_yaw),
            "--cam-pitch" => cfg.cam_pitch = next(&mut i).parse().unwrap_or(cfg.cam_pitch),
            "--cam-spin" => cfg.cam_spin = next(&mut i).parse().unwrap_or(cfg.cam_spin),
            "--cam-dolly" => cfg.cam_dolly = next(&mut i).parse().unwrap_or(cfg.cam_dolly),
            "--anim-time" => cfg.anim_time = next(&mut i).parse().unwrap_or(cfg.anim_time),
            "--anim-rate" => cfg.anim_rate = next(&mut i).parse().unwrap_or(cfg.anim_rate),
            "--cam-submerge" => cfg.cam_submerge = next(&mut i).parse().unwrap_or(3.0),
            "--no-vsync" => cfg.vsync = false,
            "--no-idle-repaint" => cfg.idle_repaint = false,
            "--no-shadows" => cfg.shadows = false,
            "--no-hiz" => cfg.hiz = false,
            "--hud" => cfg.hud = true,
            "--demo-edits" => cfg.demo_edits = true,
            "--demo-glass" => cfg.demo_glass = true,
            "--demo-lamps" => cfg.demo_lamps = true,
            "--load-edits" => cfg.load_edits = Some(next(&mut i)),
            "--save-edits" => cfg.save_edits = Some(next(&mut i)),
            "--resume" => cfg.resume = true,
            "--edits" => cfg.edits = Some(next(&mut i)),
            "--no-exit-save" => cfg.no_exit_save = true,
            // Clamped rather than refused: it indexes a fixed array three call sites away,
            // and unlike a slot read out of a *file* there is a human at the other end of
            // this one who can see which hotbar cell they got.
            "--bar" => cfg.bar = Some(next(&mut i)),
            "--hotbar" => {
                cfg.hotbar = next(&mut i)
                    .parse::<usize>()
                    .unwrap_or(cfg.hotbar)
                    .min(crate::block::HOTBAR.len() - 1)
            }
            "--ambient" => cfg.ambient = next(&mut i).parse().unwrap_or(cfg.ambient),
            "--fog-density" => cfg.fog.density = next(&mut i).parse().unwrap_or(cfg.fog.density),
            "--fog-falloff" => cfg.fog.falloff = next(&mut i).parse().unwrap_or(cfg.fog.falloff),
            "--fog-scatter" => cfg.fog.scatter = next(&mut i).parse().unwrap_or(cfg.fog.scatter),
            "--fog-g" => cfg.fog.g = next(&mut i).parse().unwrap_or(cfg.fog.g),
            "--cloud-cover" => cfg.clouds.cover = next(&mut i).parse().unwrap_or(cfg.clouds.cover),
            "--cloud-height" => {
                cfg.clouds.height = next(&mut i).parse().unwrap_or(cfg.clouds.height)
            }
            "--cloud-scale" => cfg.clouds.scale = next(&mut i).parse().unwrap_or(cfg.clouds.scale),
            "--cloud-speed" => cfg.clouds.speed = next(&mut i).parse().unwrap_or(cfg.clouds.speed),
            "--godray-strength" => {
                cfg.godrays.strength = next(&mut i).parse().unwrap_or(cfg.godrays.strength)
            }
            "--godray-steps" => {
                cfg.godrays.steps = next(&mut i).parse().unwrap_or(cfg.godrays.steps)
            }
            "--godray-dist" => cfg.godrays.dist = next(&mut i).parse().unwrap_or(cfg.godrays.dist),
            "--cloud-shadow" => {
                cfg.godrays.cloud_shadow = next(&mut i).parse().unwrap_or(cfg.godrays.cloud_shadow)
            }
            "--no-ao" => cfg.ao = false,
            "--no-physics" => cfg.physics = false,
            "--help" | "-h" => {
                println!("Surface repair: --legacy-lighting / --repaired-lighting; --legacy-water / --repaired-water.\n--sun-softness narrow|normal|wide; --soft-shadow-hq / --no-soft-shadow-hq (requires soft shadows).\n--surface-debug off|normals|shadow|direct|indirect|water-faces|reflection|refraction");
                println!("Primary shadows always use one ray + optional screen-space reconstruction; --shadow-pass is default, --no-shadow-pass is retired.\nSecondary water/glass shading remains isolated (--isolate-glass, --no-isolate-glass compatibility); timings include the full resolve family.");
                println!("Experimental: --compact-shade-hit / --no-compact-shade-hit (unmeasured schedule A/B).\n\
                     Existing arms also accept --no-soft-shadows, --no-wind-sway, --no-water-look, --no-glass-reflect.");
                println!(
                    "voxelcraft\n\
                     \n\
                     --screenshot PATH        render one frame headless to a PNG and exit\n\
                     --bench-terrain N        generate N chunks, print timings, and exit\n\
                     --bench-frames N         render N frames, print per-pass timings, and exit\n\
                     --seed N                 world seed (default 1337)\n\
                     --sea-level N            internal Y the basins flood to (default 128)\n\
                     --no-water               same terrain, no water in it\n\
                     --no-biomes              one biome everywhere: the pre-batch-9 world\n\
                     --no-foliage             no grass tufts: the pre-batch-14 world\n\
                     --no-meadow              no coarse meadow: the pre-batch-18 world\n\
                     --no-probe-shadow        probe bounce ignores whether the ground\n\
                                              it gathers is itself lit\n\
                     --no-probe-sun           ambient floor is the authored constant\n\
                     --probe-sun-high         the louder bounce gain (0.25 not 0.10)\n\
                     --no-tint                no per-biome vegetation tint\n\
                     --tint-strength F        how far the biome tint is taken (default 1)\n\
                     --water-absorb F         scale water's extinction (0 = perfectly clear)\n\
                     --water-mottle F         water layer mottling (ships 0; 0.2 = pre-batch-26)\n\
                     --cam-height F           camera height in internal Y (default 40)\n\
                     --cam-yaw D              camera yaw in degrees (default 40)\n\
                     --cam-pitch D            camera pitch in degrees (default -14)\n\
                     --cam-submerge F         capture F blocks under water (negative: above)\n\
                     --hud                    draw the stats overlay in a screenshot too\n\
                     --demo-edits             build the demo structures through the edit path\n\
                     --demo-glass             build a glasshouse through the edit path (batch 38b)\n\
                     --demo-lamps             build a chamber of coloured lamps (batch 59)\n\
                     --load-edits PATH        replay a saved edit journal after worldgen\n\
                     --save-edits PATH        write the edit journal to PATH when the mode ends\n\
                     --edits PATH             a world on disk: load if there, append, save\n\
                     --no-exit-save           end without the exit save, the way a kill does\n\
                     --resume                 start where the loaded journal's player was\n\
                     --hotbar N               hotbar slot to start on, 0-9 (default 4)\n\
                     --bar A,B,..            the ten block ids in the bar (default 1,2,3,4,8,6,7,9,10,11)\n\
                     --width N --height N     window size\n\
                     --scale F                internal render scale (default 1.0)\n\
                     --jitter X[,Y]           pin the sub-pixel ray offset (default: Halton)\n\
                     --taa-frames N           frames a screenshot converges over (default 32)\n\
                     --reference N            converge a screenshot over an NxN sub-pixel grid\n\
                     --mip-bias F             extra texture mip bias in resolve\n\
                     --cam-spin D             degrees of yaw per converged frame\n\
                     --cam-dolly F            blocks of forward travel per converged frame\n\
                     --anim-time F            seconds of wave/cloud phase at the captured frame\n\
                     --anim-rate F            seconds of that phase per converged frame\n\
                     --fov D                  vertical field of view in degrees\n\
                     --view-distance F        far plane in blocks (default 2600)\n\
                     --max-lod N              coarsest LOD level (default 4)\n\
                     --streaming-factor F     LOD subdivision aggressiveness (default 2.0)\n\
                     --fade-band F            outer edge of the LOD cross-fade (default 1.25)\n\
\n\
                     --fade-schedule NAME     LOD cross-fade curve: smoothstep (default) or\n\
                                              smootherstep (D2b's measurement arm)\n\
                     --diff A.png B.png       compare two captures: MAE, differ count, bbox as\n\
                                              crop fractions; --grid GX,GY adds a per-cell table\n\
                                              with the peak cell marked (needs no renderer)\n\
                     --crop X0,Y0,X1,Y1        with --diff: fractions of the image to compare
                     --no-shore-wet           dry sand at the waterline (reverts batch 75's band)\n\
                     --no-shore-foam          no foam over shallow columns (reverts batch 75's mottle)\n\
                     --no-water-sec           full-res traced water legs (reverts batch 81's
                                              half-res diagnostic: P12's measurement arm)\n\
                     --water-sec-scale N      water legs per NxN block: 1, 2 (default) or 4\n\
                     --cloud-patch P         G1: cumulus arrive in banks; P swings the coverage\n\
                                               threshold by up to the full noise range (default 0)\n\
                     --cloud-relief R        G1: sun-side cap vs belly shading on the deck,\n\
                                               shading only -- never adds density (default 0)\n\
                     --haze-warm W           G1: warm band hugging the horizon treeline\n\
                                               (default 0 = the pre-95 sky)\n\
                     --zenith-deep Z         G1: how much bluer the zenith runs at day\n\
                                               (default 0 = the pre-95 sky)\n\
                     --lookbook DIR          batch 98: 3 views x 12 settings composed into\n\
                                              one labeled PNG (lookbook.png) for comparison\n\
                     --grade none|warm|cine  warm luma-preserved cast (batch 95) or the\n\
                                              filmic cine print (batch 97b); default none\n\
                                              is the pre-95 presentation bit for bit\n\
                     --grade-strength F       batch 101: the grade dial; 1.0 keeps the\n\
                                              selected cast exact, below it lerps back\n\
                     --sky-cool               the reference frames' cornflower midday sky:\n\
                                              constant swap on gradient and deck (batch 97a)\n\
                     --water-look              ripples, murk absorption and a turquoise bank\n\
                                              band -- the references' water (batch 97c)\n\
                     --foliage-rich            ground-cover species variety on tufts and\n\
                                              grass/meadow tops (batch 97d)\n\
                     --canopy-relief           canopy grain: clump brightness relief and\n\
                                              sun-facing modulation on leaves (batch 97e)\n\
                     --tone-map knee|aces     filmic shoulder at the blit (batch 90's A10 arm:\n\
                                              knee is what every constant was tuned against)\n\
                     --caustics               sun sheets on the flooded floor, from the wave\n\
                                              field's own octaves (batch 90, roadmap A9)\n\
                     --glass-reflect          panes reflect the traced world, not the sky\n\
                                              (batch 90, roadmap A4)\n\
                     --canopy-lift F          hide forest canopy from the shaft envelope by\n\
                                              F blocks (batch 90, roadmap A1)\n\
                     --tree-blue-noise        thin coarse canopy proxies to LOD 0's fill\n\
                                              (batch 91, roadmap A5's LOD-cliff arm)\n\
                     --grass-dense            dense two-height tufts at LOD 0, blue-noise\n\
                                              tuft proxies on the coarse shell, reeds on\n\
                                              the wet banks (batch 101, roadmap G2)\n\
                     --wind-sway              gust shears on the grass cross-quads,\n\
                                              traveling sines in world x/z (batch 101,\n\
                                              roadmap G2's movement half)\n\
                     --soft-shadows           sun-disc penumbra: three cone taps around\n\
                                              the centre tap, budget by blocker distance\n\
                                              (batch 102a)\n\
                     --meadow-side            coarse meadow's side faces repainted too\n\
                                              (batch 92, roadmap A8's one atlas slice)\n\
                     --snell-bend             refract the primary march at the sea plane\n\
                                              (batch 91, roadmap A2c's window arm)\n\
                     --shaft-texel N          envelope blocks per texel: finer edge, shorter\n\
                                              window (batch 92, roadmap A8's swept knob)\n\
                     --tint-balance             balanced biome tints, masks on sand and lichen\n\
                                              (batch 92, roadmap A8's palette arm)\n\
                     --time F                 fix time of day, 0..1 (0.5 = noon)\n\
                     --ambient F              linear light floor (default 0.08)\n\
                     --fog-density F          haze extinction per block at sea level\n\
                     --fog-falloff F          reciprocal fog scale height in blocks\n\
                     --fog-scatter F          strength of the sun-scattering lobe\n\
                     --fog-g F                lobe asymmetry, 0 isotropic to 1 forward\n\
                     --cloud-cover F          cloud coverage 0..1 (0 = no deck at all)\n\
                     --cloud-height F         internal Y of the deck (default 448)\n\
                     --cloud-scale F          blocks per base cloud octave (default 320)\n\
                     --cloud-speed F          blocks per second of drift (default 3)\n\
                     --godray-strength F      shadowing of the haze's sun lobe (0 = off)
\
                     --godray-steps N         samples along the view ray (default 4)
\
                     --godray-dist F          how far the shaft samples spread (default 4096)
\
                     --cloud-shadow F         sun the deck takes off the ground (0 = off)
\
                     --no-terrain-shafts      no terrain-cast shafts: pair with the next for pre-batch-35
\
                     --no-tex-variation       no per-block texture permutation: the pre-batch-36 frame
\
                     --no-distant-shadows     no terrain shadow past the march: the pre-batch-37 frame
                     --no-leaf-cutout         leaves are solid cubes again: the pre-batch-38 frame
                     --no-flat-secondary      hits through water or glass take the nine-cell gather
                                              again: the pre-batch-45 frame
                     --no-water-far           a submerged camera tests chunks to the full view
                                              distance again: the pre-batch-48 frame
                     --no-water-dark          a submerged tile below the sky flood's reach looks
                                              as far as --no-water-far lets it: the pre-batch-53 frame
                     --no-snell               a submerged eye sees through the water surface at every
                                              angle again: the pre-batch-54 frame
                     --probe-tap              take one trilinear tap into the probe field per shaded
                                              surface, on top of the shading. Implies --probe-fill 1.0,
                                              so it is bit-exact and reports only a millisecond
                     --probe-ambient          that field REPLACES the ambient, and the terms it
                                              makes redundant leave the shader. Implies --probe-tap.
                                              The frame is wrong on purpose: a build to measure
                     --no-probe-cube          take the directional shading factor from face_shade's
                                              per-normal constants again, not from the baked cube
                     --no-probe-bounce        take the ambient floor from one authored constant
                                              again, not from the baked ground bounce
                     --no-light-rgb           read block light as one level wearing one warm
                                              tint again: the pre-batch-59 frame
                     --no-sky-tint            take the ambient's sky colour from one authored
                                              constant again, not from the sky model: the
                                              pre-batch-63 frame
                     --no-sky-specular        make every opaque surface perfectly diffuse
                                              again, with no Fresnel-weighted reflection of
                                              the sky: the pre-batch-65 frame
                     --no-full-march          stop every ray at a mixed-full chunk's face
                                              again, water-ignoring legs included: the
                                              pre-batch-72 frame
                     --probe-fill F           pin the probe field to the constant F and switch the
                                              bake off. At 1.0 the cube is a no-op; at anything
                                              else the frame must move, or nothing is wired up
                     --probe-noise            fill the probe field per texel instead of uniformly
                     --march-stats            print march's DDA steps and pair counts, and write
                                              the heatmap rather than the frame. Not a timing mode
                     --no-glass               glass is an opaque cube again. NOT a pure revert: the
                                              light flood still runs through a pane
                     --no-water-shadow-cut    a sea floor marches its own shadow 48 blocks again
                                              instead of 16: the pre-batch-41 frame
                     --no-leaf-thin           a canopy keeps 0.72 of each leaf block instead of
                                              0.62: the pre-batch-43 frame
\
                     --no-vsync --no-shadows --no-ao --no-physics --no-hiz --no-taa\n\
                     --no-idle-repaint        render every frame even when nothing moved\n\
                     --no-fade                pop between LOD levels instead of dissolving\n\
                     --no-shadow-share        shadow grid takes the fine LOD half, not the majority\n\
                     --no-offscreen-shadows   only chunks the frustum keeps cast shadows\n\
                     --no-water-reflect       drop the traced reflection off the water\n\
                     --no-water-refract       drop the traced refraction under the water\n\
                     --no-waves               mirror-flat water: the pre-batch-16 sea\n\
                     --wave-amp F             peak slope of the water micro-normals\n\
                     --wave-scale F           blocks per base wave octave (default 32)\n\
                     --wave-speed F           wave periods per second (0 = frozen)\n\
                     --wave-clamp F           slope cap on the traced reflection (default 0)\n\
                     --no-wave-aniso          wave detail ignores the grazing angle (pre-batch-19)\n\
                     --no-wave-shoal          full swell even in shallow water (pre-batch-20)\n\
                     --no-wave-fill           four wave octaves instead of eight (pre-batch-25)\n\
                     \n\
                     In game: WASD move, mouse look, Space/Shift up-down, F fly, Ctrl fast,\n\
                     LMB break, RMB place, 1-0 or scroll pick block, F3 stats, F4 render scale,\n\
                     F5 iteration heatmap, F6 Hi-Z culling, F7 TAA, Escape release cursor."
                );
                std::process::exit(0);
            }
            _ => unknown.push(a.to_string()),
        }
        i += 1;
    }
    // Batch 55's and batch 56's diagnostics measured a field holding a *constant*, and batch 57
    // filled the same texture with a real bake. Pinning it here -- after the loop, so an
    // explicit `--probe-fill` in either order still wins -- is what keeps their two rows in
    // `CLAUDE.md`'s control table true verbatim rather than true with a new caveat attached:
    // `--probe-tap` alone is still bit-exact, because the field it taps is still 1.0.
    if (cfg.probe_tap || cfg.probe_ambient) && cfg.probe_fill.is_none() {
        cfg.probe_fill = Some(1.0);
    }
    (cfg, mode, unknown)
}



