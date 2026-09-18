
# Water

Everything the sea is, in one file: the medium, the surface, the wave field, and the constants
behind each. **Consolidated from batches 8, 16, 19, 20, 21, 21b, 25 and 26**, which were eight
separate documents and are why a decision made in one of them could not be found from another.

What has been **tried and rejected** here -- perturbing the traced reflection, re-spreading the
wave directions, correcting the water's mip footprint, filtering the lattice -- is in
[`errors.md`](errors.md) with the numbers it lost by. Read that before designing anything.

**Batch 38b generalized most of this file to glass**, which is the same optics with the medium
taken out -- a surface the primary ray stops at, one refracted ray, a Fresnel weight toward a
reflection, and no Beer-Lambert because a pane is one block thick. It is written up in
[`shading.md`](shading.md) rather than here, because this file is about the *sea*: the waves,
the absorption curve and the shoaling ramp are all things a window does not have. What the two
share -- and what a batch touching either must not break -- is `march_chunk`'s pair of `skip_`
bools and the rule that **every secondary ray skips glass**. It is *not* true of water any
more: `trace_world` carried a hard-coded `skip_water = true` from batch 8 until 38b's follow-up,
which was right for water's own three callers and wrong for the first one glass added -- the sea
was invisible through a pane. It is a parameter now, the three water-side sites pass the `true`
they always had, and [`errors.md`](errors.md) has the number and the lesson.

---

## Water is in the occupancy tree, and that is the load-bearing choice

It fills its voxel, so the marcher stops on it, the visibility buffer gets a real hit at a real
depth, `resolve` has a surface to shade and `taa` has a point to reproject. Everything else
follows. Three questions that used to be one:

| question | answer for water | where |
|---|---|---|
| does it occupy a voxel? | **yes** | the occupancy tree -- everything but air |
| does it block light? | **no** | `BlockDef::opaque` |
| does it block the player? | **no** | `BlockDef::solid` |

`BlockDef::solid` is **only** about collision. `voxel_occludes` (not `voxel_solid`) is the
rendering question.

**`ATTR_HAS_WATER` is why the feature costs nothing inland.** `march_chunk` ANDs `skip_water`
with the chunk's own bit, and `gather_face` / `voxel_occludes` gate their lookups the same way,
so a chunk with no water executes the instructions it did before batch 8. A dry-world capture is
bit-identical. Without that bit the cost would land on every shadow ray in the world.

**It cuts the other way too, and batch 53 is what that cost (and what it was worth).** A ray
that *ignores* water -- the submerged primary ray, and every refracted, reflected and shadow ray
in the frame -- is looking at a different world from the one this tree describes: an ocean
interior is occupied at every level and is empty to it, so it walked open sea four voxels at a
time with an attribute load per step. `GpuChunk::dry_mask` is `root_mask` with the water-only
16^3 cells cleared, and `march_chunk` traverses **that** whenever `skip_water` is set. Worth
**-25.2% of `deep-water`'s DDA steps and -16.6% of `sea-horizon`'s**, bit-exact, and it is
root-level -- the rest of a 68% ceiling is roadmap P9. [`gpu.md`](gpu.md).

**Water does not occlude for AO or the light gather.** A sea floor's face has three water
neighbours in its 3x3; counted as solid they would put it on the fully-occluded corner and render
a lit sandy bottom as black.

**`SPEC_WATER_SHADOW_DIST` is 16 blocks since batch 41, against 220 through air.** A sea floor
under a low sun aims its shadow ray nearly along the surface and, with water transparent, grinds
through hundreds of blocks of it: `resolve` read **7.62 ms** on a sea-filling frame uncapped,
against 4.93 with water opaque to shadows -- which loses the lit floor entirely and reads like a
cave. Batch 8 capped it at 48 and landed at **5.58 ms**.

**Batch 40 then measured what the capped ray still costs and it is the largest single number in
this engine: 4.100 ms +/- 0.068 of a 13.080 ms `resolve` at `coastline`, 31% of the pass.** Not
the traversal -- a refracted ray that *hits* calls `shade_hit`, and `shade_hit` casts a shadow
ray of its own, so this is a third-generation ray paid on every pixel where the sea has a floor
under it. All three of P1's research documents modelled the secondary rays as traversal loops
and none of them proposed this constant; see [`research-review.md`](research-review.md).

**Batch 41 took it to 16, and the reason that is nearly invisible is batch 37 rather than
anything about water.** `shade_hit` passes the same budget to `terrain_shade_beyond`, so the
light envelope resumes exactly where the march stops and the two partition the ray with no gap
-- shortening the budget moves the handoff instead of deleting the shadow. Measured at fifteen
vantages the whole change is **1,309 pixels of 921,600 at max channel delta 1** at its worst,
and **0 pixels at ten of them**. It is worth **-1.672 ms +/- 0.007 of `resolve` at
`coastline`** over 8 paired rounds, -0.531 at the default camera and -0.465 at `shore`, and
`--no-water-shadow-cut` restores 48 bit-exactly against `voxelcraft-pre41.exe` at all fifteen.

**Converged over an 8x8 jitter grid, the two builds differ by an MAE of 0.0013 at `default`
against that reference's own half-split noise floor of 0.7472** -- 575x under it, and 90x under
at `open-sea`'s water crop, with `speckle`, `coherence`, `rg_std` and `rg_speckle` agreeing to
four decimals. The project's own ground truth cannot resolve the change. `PERF.md` has both
tables.

**What is given up is the 16-to-48-block band's worth of occluders the envelope cannot
represent**, and that is why the batch stopped at 16 rather than at 0 where another 2.4 ms was
sitting. The envelope is `WorldGen::height`: it knows the terrain, and it knows nothing about a
tree, a player's wall or anything the edit journal placed. Switched off on both sides of the
A/B, the 16-to-48 band is still nearly empty -- 1,084 pixels at `shore` against 369,628 for the
first sixteen blocks -- so **the near march is where the shadow is, and a 16-block exact march
still catches a tree-height occluder at a mid sun.** No vantage in the fixture holds a tree
shadowing water, which is a reason to keep the margin rather than evidence there is nothing to
keep it for.

**Light: water is transparent to the flood and attenuates only the vertical sky pour**, one level
per block, which is Minecraft's own rule and why a sea floor darkens with depth. The *colour* of
that depth is the renderer's per-channel absorption -- fifteen integer levels cannot carry a
curve.

---

## The medium

**`WATER_EXT = (0.280, 0.055, 0.020)` per block**, roughly the measured coefficients for clear sea
water: red gone within a few blocks, blue surviving tens. Applied to the **view leg only** -- the
downward leg is the sky flood's job. That split keeps the chromatic half on the GPU as a
per-channel exponential and the scalar half in the light bricks. `--water-absorb` scales it.

**Absorption converges toward the medium's colour, not toward black.** A pure `exp()` makes deep
water a hole in the world. It converges to the atlas layer, which is the water's *scattering*
colour rather than an albedo. `WATER_BODY` in `resolve.wgsl` is the same authored value decoded,
for the underwater case with no surface texel to sample --
`water_body_matches_the_atlas_layer` parses it out of the module and holds it to the layer's
linear mean at 4%, because for eight batches the comment claiming they agreed was the only thing
that did.

**The water layer ships flat (`--water-mottle` 0) since batch 26.** Its `0.20 * blob` mottling
repeated once per block via `face_uv` and read as a diamond weave at grazing angles -- under half
a code value of MAE and **27-34% of `speckle`** at `coastline` and `open-sea`. `--water-mottle
0.2` reproduces the pre-batch-26 sea bit for bit at fourteen vantages.

**`block_faces` strides by eight and `shade_water` must say so.** It strode by **6** from batch 8
to 21b and read index 81 -- block 10 face 1, `tex::GLASS` -- so the sea's medium colour was a
glass pane's pale blue-white for eight batches. 81 is a valid index into a table of 160, so a
wrong stride returns a *plausible* layer rather than garbage and the world merely looks a little
wrong. `FACES_PER_BLOCK` names it, a `const _: () = assert!` pins it, and
`faces_per_block_matches_the_shader` holds every WGSL call site to it.

---

## The surface

**`WATER_F0 = 0.02`, and the smallness is the point.** Schlick climbs to 1 at grazing however
small the normal-incidence term is, so one constant gives both halves of what water looks like:
your feet at your feet, the sky at the horizon.

**Both legs are shaded with the flat light model since batch 45, and only the primary hit keeps
the nine-cell gather.** A hit reached *through* the water -- the sea floor under the refraction,
the hillside in the reflection -- takes one light lookup at the air cell in front of its face,
with no AO and no bilinear. It is worth **-1.684 ms of `resolve` at `shore`, -1.584 at
`terraces`, -1.186 at `coastline`**, the largest saving here since Hi-Z, and what it costs is
the contact shading on a submerged step: **230,613 pixels at `terraces`** at max channel delta
39, and nothing at all at `underwater`, where no water surface is in frame. **Shallow clear
water looking down is where it shows and a deep or hazy path is where it does not**, because
`exp(-extinction * depth)` has already taken that cue by the time the light comes back.
`--no-flat-secondary` restores it; [`shading.md`](shading.md) has the model and
[`gpu.md`](gpu.md) has the thing that decided the sign, which was how the gate was written
rather than anything about lighting.

**The refracted ray bends; the traced reflection does not perturb.** `refract` at IOR 1.333
displaces the bottom, which is most of what says "seen through water" up close. The reflection
keeps the flat normal -- see [`errors.md`](errors.md), it was measured and it is expensive and
unfilterable.

**The glint is analytic, and only on a *missed* reflection.** The sun disc in `sky_color` is 1.5
degrees wide; a flat sheet reflects it into a spot a pixel or two across, so a calm lake under a
low sun would carry no sun at all. `GLINT_POWER 320` and `GLINT_GAIN 2.5` add a wide lobe with no
second ray and no divergence, and terrain in the mirror direction occludes it for free because a
hit replaces the whole term.

**The two traces have different budgets because they cost and cover different amounts.** On a
sea-filling frame refraction changes **35%** of pixels and costs **4.16 ms**; reflection changes
**2.2%** and costs **1.02 ms**. So refraction fades over **96-192 blocks** and reflection reaches
**768** and fades over **512-1024**. What fades is the *hit*, never the trace -- a reflection that
misses is `sky_color`, exact at every distance, so only a hit could seam.

**The reach was re-priced in batch 40, re-ranked in batch 41 and swept in batch 44, and it stays
at 768.** The interval it gives up is handed to nothing -- unlike the shadow budget above, which
hands its own to the light envelope -- and that is why every value below 768 costs a picture.

**Batch 44 separated the two constants batch 40 had moved in one build, and the separation
changes what this paragraph used to claim.** That build took the reach 768 -> 384 *and* the fade
512/1024 -> 256/384 together, so its 1.476 ms belonged to both; measured one at a time at
`coastline` the reach is **-0.746 ms** and the fade **-0.553**. The 22,183 pixels at delta 109
this paragraph used to attribute to halving the reach are **the fade's, at `lattice`** -- the
reach alone moves 7,615 there.

| lever | `coastline` `resolve` | what it costs to look at |
|---|---|---|
| reach 768 -> 512 | **-0.446 ms** | 5,442 px at delta 86, and the headland is already gone |
| reach 768 -> 384 | **-0.746** | 8,918 at delta 86 |
| reach 768 -> 256 | **-1.164** | 10,656 at delta 95 |
| reach 768 -> 128 | **-1.646** | 12,951 at delta 144 |
| fade 512/1024 -> 256/384 | **-0.553** | 68,661 over the sixteen vantages at delta 146, 13 of 16 moving |
| the reflected hit's shadow budget, 220 -> 16 | **-0.044 +/- 0.029, zero** | 273 over the set at delta 8 |

**What the reach actually buys is the headland**, and that is the sentence the cost curve could
not supply. At `coastline` the dark crest at the right edge is mirrored in the sea beneath it;
the reflected ray that finds it is longer than 512 blocks, so **the reflection is gone at 512**
and at every value below. It is the most prominent reflection in the fixture, in the frame this
project screenshots, and 0.446 ms is not what it is worth. **The band under the water was nearly
empty and this one is not**, which is the whole difference between batch 41 and this.

**The third lever is free to look at and buys nothing**, which is the finding worth keeping.
A reflected hit calls `shade_hit` with `frame.shadow_dist`'s 220 where a refracted one gets
`SPEC_WATER_SHADOW_DIST`'s 16, and cutting it to 16 -- or to 0 -- is inside its own standard
error at `coastline` and at `terraces`. **The medium is why**: a refracted ray's hit is under
the sea and its shadow ray sets off through water, which is transparent to it and lets it grind
for hundreds of blocks; a reflected ray's hit is a hillside in air, whose shadow ray stops at
once. Batch 41's 1.68 ms was never *a secondary leg's shadow ray is expensive* -- it was
water's transparency, and it does not transfer even to the other leg of the same function.
The numbers and the retry conditions are in [`errors.md`](errors.md).

**`shade_water` calls `sky_color(mirror)` unconditionally**, before and regardless of
`FLAG_WATER_REFLECT`, because the Fresnel mix needs a sky term either way. Every water pixel pays
for whatever is in `sky_color`. This is the single most-repeated cost surprise in the project --
see [`errors.md`](errors.md).

**`sky_color` takes a ray origin and `sky_base` must not.** The gradient is at infinity; the deck
is at a finite altitude, so the origin decides which piece of it a ray hits. The reflection starts
at the **surface**, not at the camera.

### Underwater

A frame flag, and the surface is an analytic plane. From inside the medium the marcher cannot find
the boundary -- a water-to-air transition is not an occupancy edge -- so `FLAG_UNDERWATER` turns
water non-blocking for the primary march and `water_path` intersects the ray with
`frame.sea_level`. **The flag is read off the block at the eye, not from the plane**, because a
cave below sea level is dry.

Two deliberate approximations: a horizontal ray inside a small pond never reaches the far bank and
stays fogged as though it were the sea. **Snell's window was the second and is no longer
absent -- batch 54, at the end of this file.**


**A submerged camera stops testing chunks at `WATER_FAR_DIST` (batch 48), and the test is the
ray's water path rather than its length.** `tile_select` takes the most upward of a tile's four
corner rays: if even that one is still under the plane at that range, every ray the tile marches
is, and the tile stops there instead of at `frame.far`'s 2600 blocks. At the 128-block reach
batch 52 shipped it is worth **-2.776 ms of an 11.6 ms frame at `sea-horizon`** and **-1.125 of
8.7 at `underwater`**; at batch 48's 256 the same control was -0.835 and -0.291.
`--no-water-far` is the control, and it is free in both directions.

**The distinction is the whole feature.** The obvious version -- clamp `frame.far` itself when
submerged -- is what the roadmap asked for, and it deletes everything above the waterline: at
three blocks' depth a ray only 21 degrees above the horizontal leaves the water within nine
blocks and then sees a headland two kilometres off at full strength. Leaving `frame.far` alone
also leaves `depth_key`'s quantisation and the Hi-Z scale fixed, which a per-frame clamp would
have shifted the moment the camera submerged.

**The constant was authored blind and is now measured, and the measurement moved it the other
way (batch 52).** Batch 48 wrote 256 off blue's extinction -- 0.020 per block, the only channel
that survives this far, putting transmittance under 1% there -- because `underwater`'s submerged
content ends within 32 blocks and every reach above that read zero pixels at every extinction
down to `--water-absorb 0.05`. Those zeros were the vantage having nothing to cut. `sea-horizon`
is the camera that does: a submerged eye along the horizontal at islands 264 blocks out. The
whole ladder is in `PERF.md` and at the constant itself; what it decided is that the content
ends at **264 blocks**, that 256 was throwing away 316 pixels at one code value, and that
**128 costs 67,621 pixels at max delta 23 and buys 1.930 ms** -- 18% of the frame there.

**Blue's extinction was right about where the content ends and wrong about nothing.**
`ln(255)/0.020` = 277 blocks against a measured 264 -- 5% apart, and batch 48's 256 sat 8 blocks
under exact. What the capture added is not a correction to that reasoning but the *price of
ignoring it*: 128 is half the derived number and costs a visible seam, and it was taken anyway,
by the user and against the picture, because 1.930 ms is 18% of the frame at that vantage. The
seam and what was **not** measured about it -- it sits at a fixed distance from the camera, so it
slides as the player swims -- are in [`errors.md`](errors.md) with the retreat rows.

**And it stops at a shorter reach again when the eye itself is below the sky flood (batch 53).**
`WATER_DARK_DIST` is 64 blocks, against `WATER_FAR_DIST`'s 128, and the rung that picks it is
nested inside batch 48's -- so `--no-water-far` clears both and `--no-water-dark` clears only
this one.

**The depth is derived, which makes it the one constant in this subsystem that was not
authored.** `light.rs` pours sky light down an open column at `MAX_LEVEL` = 15 and takes one
level off per block of *water*, and `flood` decrements one per step in **every** direction -- so
any path from the surface to a cell `d` blocks down is at least `d` steps long and that cell's
sky light is bounded by `max(15 - d, 0)` however the light reached it. At `d = 15` the bound is
exactly zero. `WATER_DARK_DEPTH` is that 15, held to `light::MAX_LEVEL` by
`dark_depth_matches_the_light_flood`.

**What the pair buys is an interval covered twice over.** Past `WATER_FAR_DIST` content is
absorbed, which is batch 48's whole argument. Below `WATER_DARK_DEPTH` it is unlit. A tile under
that depth for the *whole* of `[0, WATER_FAR_DIST]` has the two covers meeting with no gap
between them, so everything past `WATER_DARK_DIST` is one or the other and the tile can stop
there.

**Both ends of that interval have to be tested, and the first cut of this tested one.** Depth is
linear in `t`, so "under the flood's reach throughout" is "under it at both ends" -- and which
end binds flips with the sign of the ray's elevation. A descending ray is shallowest at the eye;
a rising one at the far end. The far-end-only version admits every steeply-downward tile at any
camera depth at all, which is true about the *content* and wrong about the *feature*: it moved
**98,323 pixels at max delta 9 at `sea-horizon`**, a camera three blocks down where nothing is
dark. Anchoring at the eye as well is stricter than the physics needs and is deliberate -- the
clamp is then a property of where the player **is** rather than of where they are **looking**, so
the far sea floor cannot pop as they turn. [`errors.md`](errors.md) carries what the loose
version was worth.

**`WATER_DARK_DIST` = 64 because that is where the ladder stops moving, and the floor is
structural.** A reach culls whole chunks on `box_distance` and an LOD 0 chunk is 64 blocks on a
side, so under the chunk edge there is nothing left to remove. Measured at `deep-water`, against
an unlimited reach:

| reach | pixels of 921,600 | max delta | note |
|---|---|---|---|
| 128 (off) | 34,909 | 28 | batch 52's rung alone -- the parent frame |
| 96 | 36,902 | 28 | +1,993 over the parent at max delta 2; the retreat rung |
| **64** | **54,485** | 28 | **+19,576 over the parent at max delta 3** |
| 48, 32, 24, 16 | identical to 64 -- one sha256 across four rungs | | |

**Ranked against batch 52's, this is the cheap kind of truncation**: 19,576 pixels at max delta
3 against 67,621 at 23, for a band twice as near. What the depth buys is that an unlit surface
carries no *detail* to lose -- it is a silhouette against the open-water haze, not an image --
which is exactly what a shallow camera at the same distance does lose, and why lowering
`WATER_FAR_DIST` instead would not have been the same trade.

**Seventeen of the eighteen vantages are bit-exact under the control**, because no other camera
in the set is deep: `underwater` and `sea-horizon` both sit three blocks down. That is a
statement about the fixture and not about the feature, which is why `deep-water` exists and why
`harness::CHECKS` carries a row policing its depth.

**What it is worth in work**, counted rather than timed: **1,788,193 of 26,269,424 DDA steps at
`deep-water` (6.8%)**, and 14,203 of 24,134 deferred pairs. Zero at every other vantage.

**And in milliseconds, which the count predicted**: **-0.513 +/- 0.038 of an 8.71 ms frame** at
`deep-water`, split -0.278 `march` and -0.215 `resolve` -- the second half because `resolve`
reaches `march_chunk` through its own secondary rays. Zero at every other vantage, and the null
pair taken the same sitting reads +0.002 +/- 0.007, so the floor is about 0.02 ms.

---

## The wave field

**Slopes, never displaced.** The visibility buffer holds one flat voxel face per pixel and `taa`
reprojects that point; a displaced surface would have to be traced, stored and reprojected, and
**there is nowhere in the visibility key to put it**. So the field perturbs shading and nothing
else.

**Eight octaves subdividing 32 to 3.14 blocks** (batch 25), against the four that spanned the same
band before it. Same total slope variance -- the sea is equally rough and only the spectrum
changed. Peak slope-field autocorrelation went **0.9988 to 0.7314**. Six octaves scores 0.8777 and
is free; eight costs **+0.11 ms at coastline and shore** and was chosen because six still shows a
regular lozenge structure in the sun glitter at the far grazing band.

The frequency ratios are irrational-ish so the sum has no period a still frame reads as a grid,
and the phase rates are their `sqrt` -- deep-water dispersion, so short waves travel slower and the
sum never repeats in time either.

**The autocorrelation is a closed form and does not need an estimator.** Summing
`v_k cos(w_k . L)` over the octaves has no window, no padding and no variance normalisation. It
lives in `tests/waves.rs`, parsed out of `render::shader_source()`, so it cannot drift from the
shader. Three documents once disagreed about this number because each ran an FFT over a window
nobody recorded.

### Three normals, because the terms want different ones

| term | normal | why |
|---|---|---|
| Fresnel | full perturbed | no ray, and it carries most of the look |
| analytic glint | full perturbed | no ray |
| `sky_color` lookup | full perturbed | a ray, but not a traced one |
| traced reflection | **flat** | measured -- see [`errors.md`](errors.md) |
| traced refraction | **flat** | the expensive trace, paid on 35% of a sea-filling frame |

Schlick is steep near grazing, so a few degrees of tilt is a large swing in how much sky a facet
shows -- which is why a **6.8-degree peak slope** turns sheet glass into water without touching a
ray.

Two grazing failure modes are handled by code that already existed: a facet tilting past the view
ray is clamped by `schlick`'s own 1e-3, and a mirror direction below the horizon is faded by
`sky_base`'s existing `rd.y` rolloff. Neither needed a guard.

### The band limit, and the footprint it is evaluated against

The normal is evaluated once per pixel with no mip chain under it. At 600 blocks its finest octave
is a quarter of a pixel across; left alone it aliases into white sparkle the temporal pass answers
by smearing.

Each octave fades as its wavelength approaches the pixel footprint -- full weight at four pixels
per period, gone at two -- and **the slope variance the fade removes is handed to the glint lobe
as roughness** rather than dropped. A cosine of peak slope `a` has variance `a*a/2`, a Phong
exponent is about `2 / sigma2`, so the two compose as reciprocals and the gain follows the
exponent: a wider glitter path is not also a brighter one.

**The footprint is an ellipse and the filter has to be anisotropic** (batch 19). A pixel's
footprint on the water plane is stretched *along* the view direction by `k = 1 / |rd.y|`, and
filtering every octave at the long axis turns a distant sea back into sheet glass. Each octave is
filtered against the ellipse's extent along its **own** direction:

```
ext = px * sqrt(1 + kk*d*d)     // d = dot(dir, vh), kk = k*k - 1
```

One dot, one fma, one sqrt per octave. `kk == 0` gives exactly 1.0 in IEEE, so `ext == px` bit for
bit and `--no-wave-aniso` is a revert rather than an equivalent. `vh` uses
`inverseSqrt(max(dot(rd.xz, rd.xz), 1e-12))` rather than `normalize`, because looking straight
down gives a zero `rd.xz`.

Before this the correction was **understated 15.9x at the horizon**: at 428 blocks the true
footprint is 5.94 blocks against a finest octave of 3.14 -- four times past Nyquist -- while the
fade returned 0.98. Beyond ~500 blocks the wave field was contributing more error than signal, and
`--no-waves` scored *closer to the truth* than leaving it on.

**A rolloff and its footprint are one decision.** The original `smoothstep(2.0, 4.0, ...)` reached
zero at Nyquist itself and had **never once fired**, because the footprint was up to 16x too small.
Correcting only the footprint measured *worse than shipping nothing*.

### Shoaling

`shoal = smoothstep(0.0, SHOAL_DEPTH, depth)`, scaling the **slope going in**, not the fade coming
out. The difference is not cosmetic: a band limit removes detail the pixel cannot resolve and hands
the variance to the glint, because the waves are still there. Here they genuinely are not, so the
variance goes with them -- putting this in the fade would have made a calm lagoon a *glittery* calm
lagoon.

**The depth is its own ray**, straight down from the surface voxel's centre, `SHOAL_MAX` blocks
through `trace_world`. It is deliberately **not** hoisted off the refraction, which already traces
to the floor with the flat normal and would have cost nothing: that would make
`--no-water-refract` change the waves, and it is one of the flags promised to isolate one term. A
0.7 ms ray was the right price for a control that still means what it says.

**Tilted by a thousandth, which is load-bearing.** `trace_world` builds chunk-exit times from
`1.0 / rd`, so an exactly axis-aligned ray puts `inf` on the dead axes, the first `t_exit` comes
back `-inf`, and the walk steps sideways sixteen times and misses. It fails **silently** -- a miss
reads as deep water, so the feature does nothing and looks like a constant needing tuning. This is
the only axis-aligned ray in the engine.

**`SHOAL_DEPTH = 6`**, swept by eye: 3 is too subtle, 16 flattens the near field to a mirror.
`SHOAL_MAX` is *defined as* `SHOAL_DEPTH` so the ray is exactly as long as the constant it feeds.
No ground truth exists for it -- unlike batch 19's filter, nothing says how calm a lagoon should be.

---

## Controls

All four wave controls are free (`SPEC_MASK` folds them out of the module rather than branching).

| flag | reproduces | cost of the control |
|---|---|---|
| `--no-water` | same heightfield, air where water was. Shore and tree-line rules read the `SEA_LEVEL` *constant*, not the configured level, exactly so this stays a control and not a second world | free |
| `--no-waves` | mirror-flat, pre-batch-16. Clears the three below | free |
| `--no-wave-aniso` | pre-batch-19: footprint measured across the ray instead of on the water | free |
| `--no-wave-shoal` | pre-batch-20: full swell in a two-block lagoon. Composes with `--no-wave-aniso` to reproduce the pre-19 hash exactly | free |
| `--no-wave-fill` | pre-batch-25: four octaves spanning the band instead of eight subdividing it | free |
| `--water-mottle 0.2` | pre-batch-26 sea | free (atlas constant) |
| `--wave-clamp F` | how far the *traced* reflection may tilt. Ships at **0** by measurement | -- |
| `--no-water-shadow-cut` | pre-batch-41: a surface reached through the water marches its own shadow 48 blocks again instead of 16. **The only control here that costs time to switch on** -- it is the batch's saving, handed back | free (a pipeline key; `shaderstats` is identical in both directions) |
| `--no-water-far` | pre-batch-48: a submerged camera tests chunks to the full view distance again, rather than stopping a water-locked tile at `WATER_FAR_DIST`. Above water it is unreachable -- the flag is only ever read ANDed with `FLAG_UNDERWATER`. **Bit-exact at sixteen of seventeen vantages; `sea-horizon` is the one that can see it** (67,621 pixels at max delta 23) | free; **-2.776 ms at `sea-horizon`** and **-1.125 at `underwater`** to switch on at the shipped 128, against -0.835 and -0.291 at batch 48's 256; `tile_select` reads 0.150 against 0.160 either way |
| `--no-water-refract`, `--no-water-reflect`, `--water-absorb F` | one term each | -- |

**The feature itself is not free and its cost depends entirely on the vantage.** Water is bounded
below at exactly zero by `ATTR_HAS_WATER` and above by how much sea is on screen: **+0.16 ms** at
the default camera, **+1.27 ms** down a coastline. Quote both or neither.
---

## The surface from below (batch 54)

**The one defect in this subsystem that was reported from play rather than found by a metric.**
A player 32 blocks down, looking a few degrees above the horizontal, saw a mountain:
*"mountain is still visible when deep inside water at this i dont think it should be"*. They
were right, and nothing here modelled the boundary at all -- `FLAG_UNDERWATER` makes water
non-blocking for the primary march, so a submerged ray crossed the surface as though it were not
there. A miss went to `sky_color`; a hit above the waterline was shaded in full.

**The critical angle is the whole of the physics and it is not a constant here.**
`asin(1 / WATER_IOR)` is 48.6 degrees from the normal, so a submerged eye sees the world above
the surface only inside a cone **41.4 degrees above horizontal**, and outside it the surface is a
mirror. `surface_from_below` writes Schlick against the **transmitted** angle, which is what
makes one expression do both jobs: `sin2_t >= 1` *is* the total-internal-reflection test, and the
same `sin2_t` gives the Fresnel curve inside the window. There is no
degree literal and no cosine of one anywhere in the term --
`the_critical_angle_is_derived_from_the_index_and_not_written_down` refuses them by name, because
`refract` in `shade_water` already reads `WATER_IOR` and two copies could drift.

**Why no reach constant could have fixed it**, which is the half worth carrying forward. A
`tile_select` clamp fires only when *every* ray in a tile is still submerged at the clamp
distance, and a ray aimed at a peak that breaks the surface leaves the water. The escape boundary
is `atan(depth / WATER_FAR_DIST)` -- 14.0 degrees at that player's depth -- and above it a
submerged ray is never clamped, because clamping it would delete the **sky**. Batch 53 made deep
water cheaper and could not make it look right; those are different entries and this is the one
the complaint belonged to.

### The ramp, which is the user's own correction and is two asks in one constant

Shown the first build, they gave two: *"the first 3 blocks of water should not have a mirror, the
mirror should be below them"*, and *"it should be dark at depths ... midway to the bottom you
dont really see the surface light that much if at all"*. Those are the ends of one
`smoothstep(SNELL_MIN_DEPTH, WATER_DARK_DEPTH, depth)` over the **eye's** depth:

| eye depth | the mirror | the surface light |
|---|---|---|
| 0-3 | none at all; the frame is **bit-identical** to the build without the feature | untouched |
| 3-15 | fades in | fades out |
| 15+ | full, below the critical angle | down to `SNELL_DIM_MAX` of the way to the body colour |

**The far end is derived and the near end is authored.** `WATER_DARK_DEPTH` is
`light::MAX_LEVEL`, which batch 53 established is where the sky flood reaches exactly zero -- so
ending the surface light there makes the two halves of the engine agree instead of inventing a
second horizon. Three is a look call: the critical angle is 48.6 degrees at any depth, and what
supports the exception is that the flat-surface model is worst exactly there, a few blocks under
a wave field where the facet slope is large against the solid angle a pixel covers.

**It reads the eye and not the ray, for the reason batch 53's dark rung does.** Driven from each
ray's own crossing, the effect would come and go as the player *turned*; driven from the camera
it changes as they *swim*. `the_ramp_reads_the_eye_and_not_the_ray` pins it.

**`SNELL_DIM_MAX` is 0.9 and it exists because of a limitation somewhere else.** Fading the
transmitted leg all the way to `underwater_body()` makes a frame 24 blocks down uniformly
near-black -- that colour is `WATER_BODY * medium_light(light at the eye)`, and with the sky
flood at zero the only thing left in it is `frame.ambient`'s 0.08. **The engine's deep water is
far darker than clear water is, and that is the 4-bit light nibble showing through rather than
physics.** Swept by eye at `deep-water`: 0.75 keeps a clearly readable window, 0.9 dims it to a
deep blue that still carries the cloud shapes, 1.0 is the flat near-black that started this. **If
roadmap L1 ever gives deep water real in-scattered light, this cap is the first thing that should
go back up**, and `the_surface_light_fade_is_capped_below_one` is where a reader finds out it was
ever held down.

### What the mirror shows, and the four milliseconds that answer cost

**`underwater_body()`, and the traced version was built, measured and thrown away.** The obvious
build casts a ray down into the water from the crossing point and shades what it finds. It is
**invisible**: against a mirror that is simply the medium's own colour it differs by a max
channel delta of **3 at a level camera, 3 pitched up 20 degrees and 7 at 45**. And it cost
**4 ms of `resolve` at `deep-water`** -- the pass went 3.28 to 9.77 -- because it runs on every
pixel whose view ray crosses the surface, which at a level camera is the whole upper half of the
frame.

**The reason it is invisible is the reason it should not have been built.** A mirrored ray at
these angles travels a long way through water, and `water_medium` converges it to
`underwater_body()` on the way back. The expensive answer and the free one agree *because the
free one is the limit the expensive one was walking towards*.

The split, measured with three diagnostic builds at `deep-water`: the **wave normal 1.12 ms**,
the **trace 2.50**, **shading what it found 2.87**. Dropping the last two took the whole feature
from **+6.49 ms to +0.262**, and `the_mirror_does_not_trace` is what stops it coming back. If it
ever should, the case is a *shallow* camera over a bright floor, where the return path is short
enough for the trace and the limit to disagree -- and it needs a distance gate that keeps it off
a deep one.

### What it does not do

**Inside the window the world above is not compressed**, and that is architectural rather than a
scope cut: the transmitted ray should bend at the boundary, and `march` has already traced a
straight line and put its hit in the visibility buffer, so `resolve` cannot change which geometry
the primary ray found. What is left undone is therefore a **distortion inside the window** and
never a presence or absence -- outside the window the mirror takes weight 1 and the transmitted
leg is not consulted at all.

### What it costs to look at

`--no-snell` is the control and it is a **pure revert**: the gate is the first statement of the
term and returns the caller's colour untouched. **Seventeen of the eighteen vantages are
bit-exact under it**, and that is a fact about the cameras rather than the feature -- fifteen are
dry, and `underwater` and `sea-horizon` are both exactly three blocks down, where the ramp is
zero. `deep-water` is the whole of the fixture's ability to see this, which is why
`harness::CHECKS` polices its depth and `IMPLIES` carries the positive row.

## Water seen through a pane is analytic (batch 67)

**`shade_glass` dispatched a water hit to `shade_water` from batch 38b until batch 67**, inlining
a second complete copy of it -- two `trace_world` marches and two `shade_hit` calls -- inside a
function that already had a march live. That was the last thing holding `resolve` at 96 registers,
worth **+0.103 ms at `cave` and +0.399 at `coastline`**, neither of which contains a pane.

It is now the body colour, water's own Fresnel and a sky reflection off the face normal.

**Batch 54 is the precedent and an exact one.** It built Snell's mirror by tracing a real ray,
measured **4 ms** of `resolve`, and found it differed from `underwater_body()`'s analytic colour
by a **max channel delta of 3 to 7** -- because absorption converges a ray to that colour anyway.
Batch 45 supplies the other half: a transport destroys the detail a secondary ray buys, and this
is water behind *one more* transport than that rule was written for.

**What it costs is the wave normal.** The analytic arm uses the flat face normal, so the sea
through a pane has no wave detail. That is the one fidelity loss in the batch, and restoring it is
not cheap: `wave_field` needs a shoaling `trace_world` march of its own, which is most of what was
removed.

**It is ranked rather than asserted, and the fixture could not do it.** The change moves **0 pixels
at all twenty vantages that predate it**, `glass` included -- whose crop comment says a glasshouse
*"that ever landed on a shoreline would fail"* its `Land` tag, so the set had deliberately excluded
the only configuration that could see this. `glass-sea` is the twenty-first, and there it moves
**123,079 pixels at max delta 37, MAE 0.5990**, under [`ADR 0001`](adr/0001-no-elevation-form-factor.md)'s
2.87 rejected-by-eye line. Three neighbouring cameras read 0 before one worked; `--cam-submerge 1`
is what puts the house in the shallows, because `build_glass_structure` drops it at whatever
surface the camera faces.

**The guard is `tests/glass.rs`'s `water_seen_through_a_pane_is_shaded_as_water`**, which fired on
this change and was right to. Its claim is unchanged -- the sea through a pane must be *water* and
must never reach `shade_hit`, which would draw it as a flat blue cube face -- so it now pins
`WATER_BODY`, `schlick` and the absence of `shade_hit` in that arm rather than the literal call.

## Inside Snell's window, the bent world instead of the hole (batch 91, roadmap A2c)

**Built, default off: `--snell-bend`, the marcher's arm.** Batch 54's note on this page has
said for nine batches that the window's *weight* is exact and its filling is a hole: the cone
is computed in `resolve` (Fresnel from the wave normal, TIR falling out of the same line), but
the geometry inside it is whatever a straight line from the camera found on the far side of
the sea plane. The arm is the half batch 54 called architectural and left: `march` splits the
submerged primary path at the plane -- analytically, `t_surf = (sea_level - cam.y) / rd.y`,
no extra traversal to find it -- and re-marches the transmitted half refracted, so the
hemisphere above the surface is present in the cone compressed, as physics packs it, rather
than absent-to-distorted.

**The entry's first design rule and the arm's best property are the same line: `resolve`
gains nothing.** Every term past the geometry -- the cone, the split, the mirror, the dimming
-- reads the original `rd` and `dist`, and the bend preserves both as `t_surf + t2`. Tests pin
that purity in code (`tests/batch91.rs`), the way a census turns an architecture claim into a
build failure for whoever breaks it.

**The worn approximation is labelled**: the boundary facet the bend refracts against is
*flat*, where `resolve`'s Fresnel reads the wave normal. That mismatch already ships --
everywhere, on straight rays -- so the arm leaves only the rim of the window inside it. The
successor (wave field moved to `common.wgsl`, the bend phase-locked to the weight) is an
entry-shaped splice, not a side effect; the by-eye pass gets to say whether the rim earns it.
`SURFACE_EPS` becoming shared (one home in `common.wgsl`, both files pinned to it) is the
smallest consequence of the same migration.

**What the hardware pass measures, written down beforehand**: the march-row bench at
`underwater` / `deep-water`, and *this entry's register read, the one batch 54 said the
march-side version would owe* -- register counts of `march` on and off the spec, since a
second `march_chunk` inline is one of the two shapes register allocation loses hardest on.
The batch-90b acceptance rule binds as usual (a named vantage, past max delta 1), and the
known-good reference read may *rise* by design here: the reference converges the old,
straight-through geometry, exactly as batch 72's mixed-full chunks did.

## Sun caustics on the flooded floor, behind a flag (batch 90, roadmap A9)

**Built, default off: `--caustics`.** The sheets are the wave field's own -- the same first two
deep-water octaves the surface shades (their anchors, their slope schedule, `frame.wave_scale`,
`frame.wave_amp`, the phase `frame.time * frame.wave_speed`), read once at the point where the
sun ray through this pixel's floor meets the sea plane, their slope magnitudes multiplied and
squared once so the crossings stand and the middles fall. Two octaves, not the schedule,
because a caustic *is* a woven interference pattern by physics and the third train's wavelength
is already below the filter. **A flat sea makes the whole expression exactly zero** -- zero
amplitude is zero slope is the identity multiply -- which is the standing rule applied to a new
place: the flag gates a build, the constants gate a look, and neither has to pretend a still sea
carries a ribbed floor that a `--anim-rate 0` capture would freeze mid-ripple.

The gates are the waterline's own facts: the depth window `CAUSTIC_DEPTH` (8 blocks), the
`sky_sm`-carrying `direct` already computed, and the bit nested under `sea_level > 0` beside
`FLAG_HI_WATER_SEC` in `spec_hi_from`. The arm is pure ALU in `shade_hit` and folds to nothing
when the override is off, so the shipping pipeline contains exactly the pre-arm code -- the same
batch-53/73 construction the shore band wears. `CAUSTIC_GAIN` is the taste constant and is named
as such in the shader; what it ships at is the by-eye pass's.

### The first build reads zero on hardware, and the arithmetic says why rather than which knob

**Filed as diagnosed-not-built under the batch-90 acceptance rule** (lessons.md: an arm counts
as built when it moves pixels at a named vantage by more than max delta 1). On the user's GPU
the arm reads **0 pixels at `terraces` and `shore`, 115 of 921,600 at `coastline`, every one
at max delta 1** -- and `--anim-time 3.0` makes it *worse*, folding the pinned-clock escape
hatch. Three of the named suspects fall there and are hereby eliminated: the `sea_level > 0`
nesting, the pinned clock, and the plumbing itself all fall because the arm **fires** --
115 pixels *moved*. What moves nothing is written into the carrier, not the gate.

**The mechanism, at shipping defaults and in one line: the term is quartic in `wave_amp`.**
`wave_amp` 0.12, `wave_scale` 32: peak slopes of the two caustic octaves are 0.0504 and 0.0324,
so the squared product the shader multiplies by `CAUSTIC_GAIN` tops at 2.67e-6, and the whole
term at **1.07% of direct sun** before the depth fade, 0.79% at depth 1, 0.10% at depth 8 --
per pixel, only where both cosines happen to peak together, which is why the 115 pixels are
those pixels. A gain that would lift it to a life-change (+30%) is 28x the current one -- and
the same gain at double the wave amplitude escalates the term 16x, so no `CAUSTIC_GAIN` value
is both visible here and sane there. **The gain is a suspect by name and exonerated by form:**
no scalar survives this term's amplitude responsiveness.

**Deeper, and the reason this is filed as form-and-not-value**: the focusing argument caustics
actually run on is `1/|1 - eta * d * laplacian(h)|` -- *curvature*, linear in depth, with a
fold singularity the sheet structure comes from. Folded correctly at this world's lambda-32
trains it still reads only ~0.4-3.7% pre-sharpening at depths 1-8: **the wave field's two
deep-water octaves are simply the wrong carrier for meter-scale ribs, whatever the form**. The
next build owes these two decisions *together*: an authored high-frequency caustic field (its
own lambda, its own amplitude schedule, sun-projected exactly as this one) AND the curvature
form -- either one alone stays sub-visible or amplitude-fragile at the shipping sea. That is
the diagnosis handed to the tuning step, and the sweep to confirm it is one binary per
candidate away, on hardware.

