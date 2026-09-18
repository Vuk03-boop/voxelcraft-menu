# Sky, haze and god rays

The atmosphere, the cloud deck and the crepuscular shaft, in one file. **Consolidated from
batches 5, 10 and 11.** Rejections -- the world shadow ray, the "bounded to rays that miss
geometry" claim, the camera-origin reflection bug -- are in [`errors.md`](errors.md).

---

## The split that everything else depends on

**`sky_base` is the sky without clouds; `sky_color` is the sky with them. Which one a call site
wants is not a style choice.**

| call site | function | why |
|---|---|---|
| the sky miss in `resolve` | `sky_color` | the ray reaches the sky |
| the water reflection's missed ray | `sky_color` | same |
| both fog targets | **`sky_base`** | the haze in front of a ridge is lit by the sky *behind* the ridge, not by a deck eight kilometres past it |

Mixing a surface toward a cloud-bearing sky prints the deck's brightness onto the hillside,
brightest exactly where the haze is thickest -- a light with no source in the frame.

**`sky_color` takes a ray origin and `sky_base` cannot and must not.** The gradient is at
infinity and has no parallax, so a direction is all it needs. The deck is at a finite altitude,
so the origin decides which piece of it a ray hits: the sky miss starts at the **camera**, the
water reflection at the **surface**.

---

## Haze

**A gradient with distance, not a curtain at the far plane.**

- **`fog_density` 5.0e-4 per block at sea level** -- an e-folding distance of 2000 blocks,
  quoted at `fog_height = SEA_LEVEL` so a valley floor is the reference. 7.0e-4 washes the
  mid-ground out; 3.5e-4 leaves far ridges reading as unfogged geometry.
  **Re-judged at dawn in batch 42 and it holds**, which is worth a line because the original
  tuning was done at a high sun and this constant is most visible at a low one. Fifty-four
  renders over a `--time` x `--fog-density` grid at three cameras -- the horizon crop at
  `default`, the archipelago from 240 blocks up, and into the sun at the horizon -- put the
  usable band at **3.5e-4 to 5.0e-4** and the best of it at the shipped 5.0e-4: below that a far
  headland keeps a saturated tree line and reads as a cut-out pasted at the horizon, and by
  6.0e-4 the sea has stopped reading as sea at range. **`--time 0.26` is the dawn**, and it is
  where the whole band is easiest to judge: the sun is at the horizon, the cloud deck is lit
  warm against a graded sky, and the terrain is still bright enough to carry its own texture.
  The one thing those pictures argue with is `Config::time_of_day`, which ships at **0.30** --
  a bright morning rather than a dawn. That is a preference about where a session *starts*, not
  a defect, and batch 42 left it alone.
- **Scale height 64 blocks** (`fog_falloff` = 1/64), close to the terrain's own relief, which is
  what makes the altitude term legible rather than academic. Half the sea-level density is
  reached 44 blocks up. Standing higher genuinely clears the air: **1.51x** the horizon-band
  contrast from 120 blocks up, **1.89x** from 240.
- **The optical depth is written `s * (1 - exp(-x)) / x` with `x = lambda * s * D_y`**, not in
  the `(lambda * D_y)`-denominator form. Same expression, but the horizontal-ray singularity
  then lives in one place with an obvious limit (`x -> 0` gives `s`) instead of needing a
  separate branch. `--fog-falloff 0` degrades exactly to distance fog by that same limit.
- **The ±60 clamps are overflow guards, not tuning.** They only fire where the result has
  already saturated. `--fog-density 1e9`, `--fog-falloff 5` and a negative falloff all render
  without a NaN.
- **`fog_scatter` 0.020, `g` 0.80.** HG is normalised so isotropic reads 1.0 rather than
  1/(4pi) -- the tint is a look constant, not an irradiance. The peak is 0.90 against the old
  `cos^16` term's 0.25, but the peak is not the point: at 40 degrees off the sun `cos^16` is at
  1.4% of its own peak and gone where HG still carries 3%, so the skirt is **7.7x brighter** and
  the glow spreads to the horizon instead of ringing the disc.
- **The lobe *replaced* `pow(sd, 16) * 0.25`; it was not added beside it.** One lobe lives in
  `sky_color`, and the fog mixes toward it, so the halo is counted once and a hazed ridge meets
  the sky above it without a seam. Adding a second doubles the horizon.
- **`pow(d, 1.5)` in `hg_gain` is the phase function's own exponent, not a gamma curve.**
  Everything here is linear radiance.

---

## The cloud deck

One analytic plane, evaluated in the sky miss. **`--cloud-cover 0` returns before a deck exists
and is bit-exact at eight vantages, free.**

**Batch 35 changed what that control means and not what it costs, and the difference was worth a
bench to be sure of.** It used to skip the shaft integral outright, the deck being the shaft's
only occluder; now the integral still runs for the terrain. Measured at `low-sun` against the
same build, switching it on takes `resolve` from 4.130 to **3.526 ms** -- it *saves* 0.60 ms
rather than costing anything, so "free" in the sense the control table means it (nothing is
added, so nothing is taxed) survives intact. What does not survive is the implication: a
deck-less frame still has terrain in it.

- **The haze *is* the horizon fade.** The deck's alpha is `density * transmittance` over the
  real distance to the plane, through the haze's own closed form rather than a second falloff
  invented for the sky. It costs nothing -- the function already existed -- and it makes
  `--fog-density 0` a second control: crisp cloud to the horizon.
- **The band limit renormalises by the surviving weights, falling back to the running *mean*
  and not to zero.** A dropped octave must leave a smooth sheet; clear sky at the horizon would
  be a hole with a cloud edge around it. Measured at a sea horizon with haze off, the 100 rows
  above the horizon shift **3.3x less** under a half-pixel offset with the band limit than
  without. **It costs 0.00 ms** -- forcing every octave to survive moves `resolve` not at all.
  It is an image-quality device; do not "optimise" it away.
- **The footprint is `resolve`'s mip footprint with a plane in place of a voxel face**, and it
  carries `mip_bias` for the same reason. **The distance is the whole path from the eye, not
  this leg's `t`** -- the cone spreads from the eye, so a reflection off water 600 blocks out
  arrives with a cone 600 blocks wider.
- **`CLOUD_SOFT` 0.14 and `CLOUD_DEEP` 0.55 are two ramps off one noise field, deliberately
  different widths.** One ramp for both was the first version and every cloud was a flat grey
  blob, because the pixel that just turned opaque is also the pixel that just turned black.
  Split, a cloud is opaque a little way in from a wispy rim while its underside darkens only
  deep inside a large one.
- **Thickness is the only shading variable a flat layer has.** No normal, so `dens` does the
  whole job: a thin edge passes sunlight and reads bright, a thick core reads as its own
  shadowed underside. Seen from above that inverts, and one `select` is the whole of that case.
- **The sun's reddening is `SCATTER_TINT`, not a new constant** -- a cloud lit by a different
  red than the air under it reads as a decal pasted on the sky. **`CLOUD_WARMTH` 0.72** rather
  than 1.0 because the rim is lit *through* the reddening air and is not made of it; at 1.0 a
  sunset cloud is the colour of the lobe itself, which is a coal.
- **`CLOUD_G` 0.82 and `CLOUD_SCATTER` 0.055** -- a cloud scatters harder forward than air, and
  the scatter constant is small because it multiplies a gain peaking near 30.
- **The night colour is above the night zenith and below the night horizon.** Not hedging: a
  cloud at night is dark against a bright horizon and light against a dark zenith.
- **An integer hash, not `fract(sin(dot(...)))`** -- cheaper, and `sin` of a large argument is
  the one thing here whose result genuinely differs between drivers, which would put a
  driver-dependent image behind every A/B in this directory.
- **Quintic interpolation, not cubic smoothstep.** Its second derivative vanishes at the lattice
  too; a coverage threshold on a merely C1 field prints cell edges as creases across every cloud.
- **`CLOUD_WIND` is not axis-aligned** -- a pure +X drift slides the field over its own lattice
  once per `cloud_scale` and the deck visibly repeats.
- **`cloud_cover` is a threshold on a bell-shaped field, not a sky fraction.** Four octaves
  summed are Gaussian-ish, so the response is steeply S-shaped: at the zenith 0.35 covers 2% of
  the sky and 0.70 covers 68%. The default 0.45 gives 6.5% at the zenith and 12.8% at a shallow
  vantage. Renaming it to something that sounds like a fraction would only hide the mismatch.

---

### The look variants (batch 95, roadmap G1 -- banks, caps, a warmer sky)

The goal moved on 2026-09-16: lean the whole frame toward the two reference frames saved at
[`look-reference-river.png`](look-reference-river.png) and
[`look-reference-bank.png`](look-reference-bank.png), and the sky's share of that is four
knobs plus one cast, all default 0, all folding branches in the shader. The first hardware
round priced the subtler class as well: a *uniform* guard (`if frame.knob > 0.0`) still lets
the compiler reassociate the arithmetic around a branch it cannot remove, and the
21-vantage control moved seven vantages at max delta 1 for it. Batch 95d therefore hides
all four behind one `SPEC_SKY_LOOK` override -- the tint arm's folding class, bit-exact by
construction -- with the knobs staying uniforms past the fold so the ladders sweep
one-build cheap.

- **`--cloud-patch P`** -- cumulus arrive in *banks*: one coarse tap at `CLOUD_PATCH_SPAN`
  (6x the deck's own span, so bank wavelengths run about two kilometres and read as
  geography rather than weather noise) swings the coverage threshold by up to the full
  noise range. The deck's own `drift` keeps banks and deck glued to one geography, the band
  limit follows the span (`fp / CLOUD_PATCH_SPAN` -- batch 10's own octave argument), and
  the clamp holds the `dens` contract: density still follows its threshold per texel, it
  is only *where the texels are* that moved.
- **`--cloud-relief R`** -- side-lighting that fakes verticality on a flat plane: one second
  tap displaced sunward prices the field's slope into the underside ramp (the lee reads
  belly, the leading edge reads cap). It is *shading only* -- `dens` is final before the
  block runs and `tests/batch95.rs` pins the ordering, because a shading term that could add
  density would grow clouds the control cannot see.
- **`--haze-warm W`** -- the warm band the references wear over the treeline, strongest
  toward the sun, dead at the zenith. It lives in `sky_base` so the fog path picks it up
  through the air that is genuinely in front of the ridge -- the batch-10 argument, one
  level down: a `sky_base` edit *is* a haze edit, for free and for the same reason.
- **`--zenith-deep Z`** -- a multiply-only deeper blue upstairs, weighted by the gradient's
  own exponent; at 0 the factor is 1.0 everywhere, so nothing the gradient was tuned to
  moves.
- **`--grade none|warm`** -- one pass away, in the blit: a luma-preserving warm balance and
  a gentle saturation lift on the selector's spent padding word, exactly as A10 taught the
  presentation to take a new law. It cannot re-tune a lighting constant because brightness
  is not what it moves.

**Acceptance, pre-written for the hardware round:** all five default off, so the control is
construction; each knob must move pixels at a sky-bearing vantage past max delta 1 or it is
not built (A1's rule). The knobs sweep 0.3/0.5/0.7 ladders at `sky`, `default` and
`terraces`, sun low to midday, against the references; what ships from the ladders is the
by-eye pass's call, not a number's.

---

## God rays

**The shaft is a weight on the lobe that is already there, not a beam beside it.** `sky_base`
gained one parameter, `shaft`, and it multiplies `scatter_lobe` **and nothing else** -- the
gradient is the sky itself, and a ridge does not shadow the sky. Callers with no shaft to offer
(both of `shade_water`'s) pass a literal 1.0, and that is not laziness: it would be eight more
deck lookups on every water pixel.

**There are two occluders since batch 35, and they multiply.** `cloud_shade` answers for the
deck and `terrain_shade` for the ground, and a product is what independent transmissions do --
`cloud_shade` returns a fraction rather than a bit, so a ridge standing under a cloud takes
both. The consequence to know before benching anything here is that **`--cloud-cover 0` and
`--cloud-shadow 0` stopped being able to skip the integral**: a deck-less frame still has
terrain in it. `--cloud-shadow 0 --no-terrain-shafts` is the pre-batch-11 revert now, and
`harness implies` carries that as a pair of claims rather than one.

**`cloud_shade` is the only definition of how much sun reaches a point, and it has exactly two
callers on purpose** -- the shaft integral in `sun_shaft` and the sun term in `shade_hit`. If the
deck occludes a beam it has to occlude the ground under the beam, and one function is what keeps
them from disagreeing. It reads **`CLOUD_SOFT`, not `CLOUD_DEEP`**: what stops sunlight is how
opaque the cloud is, not how far into a thick one its underside goes dark.

**`cloud_shadow` 0.85, not 1.0**, because a cloud scatters light through rather than stopping it
dead. At 1.0 an overcast landscape reads as night.

**Importance sampling, because the weight has an invertible CDF.** The estimator is a
`T(0,s) * rho(s)`-weighted mean of sun visibility, and the integral of that weight over `0..D` is
exactly `1 - T(0, D)` -- the very alpha `resolve` fogs with. Drawing `s` with `T(0,s)` stratified
uniformly turns the estimator into a plain mean, with no weights and no denominator:
`s = -log(1 - k * path) / k`, with the `k -> 0` limit covering the horizontal ray in the same
place `transmittance_from` covers it.

**It came out better *and* cheaper per sample, which is not the usual trade.** Against a
32-sample reference, **four** importance-placed samples leave 0.17% of the frame off by more than
one code value where **six** evenly spaced ones left 1.8% for the same money, and single-frame
noise drops 3x. Each step is two `log`s instead of three `exp`s. Four is the knee.

**The sun's angular size sets the penumbra and the deck's band limit is already the right tool.**
A shadow cast from `t` blocks away is blurred by `t * SUN_ANGLE` -- 26 blocks for a deck 2.8 km
along a five-degree sun. Handing that to `cloud_fbm` as a footprint gives the soft edge for free,
and because it drops octaves the penumbra has already erased, it is *also* what makes the lookup
cheap enough to sample four times a pixel. **`SUN_ANGLE` 0.0093 rad**, the real 0.53 degrees, the
same disc `sky_base` draws.

**The importance ramp had to be tightened after the first measurement, and that is the lesson.**
`max(scatter_lobe(rd)) * alpha` bounds what the shaft could move at a pixel, and below
`GODRAY_CUT_LO` the integral is skipped. The first values (0.0015-0.0060) cut only past 109
degrees off the sun -- almost nothing -- so a vantage where shafts moved **zero pixels** still
paid a full millisecond, because a smooth ramp at weight 0.07 costs what it costs at 1.0. At
**0.0040-0.0120** the cut is near 55 degrees and looking away from the sun costs +0.04 ms instead
of +1.0. **Tune a ramp to where the work stops being worth doing, then check the vantages you do
not care about actually reach zero.**

**`godray_dist` is a resolution knob, not a cost one**, which is why it defaults past the far
plane: the occluder is analytic, so a point ten kilometres out costs what a near one costs, and
truncating a horizon ray measurably biases it.

**Three early exits, and the third is not decoration.** No strength, no cloud shadow, or no deck
all return 1.0 before any work -- a loop that walked four steps to call a function returning 1.0
would have quietly taken away `--cloud-cover 0`'s measured freeness. **Batch 35 split the second
and third off into their own condition**, because they now have to be paired with "and no
terrain occluder either"; the strength check stayed alone.

---

## The light envelope: terrain as an occluder

**Batch 35, and the whole of it is one substitution.** The shaft asks, four times per view ray,
whether the sun reaches a point in mid-air. Batch 11 answered with a `shadow_ray` and measured
**+10.57 ms** of `resolve` against a frame that costs 4 -- and the cheapest corner of that sweep
finds nothing, because the ridge is 300 to 600 blocks away. The budget that makes it affordable
is the budget that makes it useless. The full sweep is in [`errors.md`](errors.md); it survives
in the tree only at `1f6c4a6:PERF.md`.

**What replaces the march is a projection, and the reason it works is that it stores an altitude
rather than an angle.** For a directional light the shadow of a heightfield is itself a
heightfield: at each column there is one altitude below which the sun is blocked. Then `lit(p)`
is `p.y > E(p.xz)`, one tap, and the search along the third dimension has been done once for the
whole world instead of once per sample.

That distinction is load-bearing. **Horizon mapping -- the obvious neighbour of this idea -- is
the version that cannot work**, because the maximum elevation *angle* around a column answers a
query on the surface and not one in mid-air: the angle to the ridge that set it depends on the
distance to that ridge, and a horizon map has thrown the distance away. An envelope has already
combined the occluder's height and its range into the one number the comparison needs.

- **`shaft::DIM` 512 texels at `shaft::TEXEL` 4 blocks** is a 2048-block window, which puts the
  camera 1024 blocks from every edge -- and 1024 is exactly `shaft::STEPS`' reach, so a sample
  anywhere in the window can see an occluder as far away as the scan can carry one. Sizing the
  two independently would mean one of them was paid for and unused.
- **`shaft::STEPS` 8, and the doubling is what makes the reach affordable.** `max` with an
  additive offset is a semiring, so Hillis-Steele applies unchanged and the window after `k`
  passes is `2^k` texels. Written as a gather that is 256 taps per texel; written as a doubling
  scan it is one tap per texel per pass.
- **The scan is near the gather rather than equal to it, and the bound is `SOFT`.** The window
  is exact over an integer lattice; the sun's azimuth is not axis-aligned, so each pass
  resamples bilinearly and a value carried three texels has been interpolated twice where a
  gather interpolates it once. Measured over the real generator at a low sun, the worst
  disagreement is **0.6 blocks** -- a seventh of the ramp the edge is already blurred over, so
  it cannot move a pixel. `the_scan_is_the_envelope_it_claims_to_be` asserts against `SOFT`
  rather than against a float epsilon for exactly that reason, and **the first version of that
  test asserted the epsilon and caught this prose rather than the code.**
- **It ping-pongs rather than running in place, and that is a determinism decision and not a
  correctness one.** In place, a thread reading a neighbour another thread had already advanced
  would take a *longer* reach -- harmless for the answer, since more reach is more correct, and
  fatal here, because the result would depend on scheduling. `harness validate` runs two renders
  of the same arguments and demands the same file before it checks anything else.
- **The scan costs 0.12 ms and does not care where the camera points.** Measured directly off
  its own timestamp at `low-sun` and at the coastline: **0.12 at both, to the digit**, because a
  fixed 512x512 grid scanned eight times has nothing in it that depends on the window size, on
  how much geometry is on screen or on the view direction. That is the whole difference in shape
  from what was rejected, which ran +1.08 to +10.57 depending entirely on reach and vantage. The
  rest of the batch's cost is `resolve`'s one tap and it is **never separable from zero** in a
  paired A/B -- four sessions put the frame total at +0.17 to +0.30 ms.
- **`shaft::SOFT` 4 blocks does two jobs with one number.** A shadow cast from `d` blocks away is
  blurred by `d * SUN_ANGLE`, about 3.7 blocks at 400, which is a penumbra a hard edge would be
  wrong to omit; and the envelope is quantised at `TEXEL`, so the same ramp is what keeps the
  cell grid from reading as a staircase.
- **The heights are filled on the CPU from `WorldGen::height`, and two properties of this engine
  are why that is cheap here and would not be in general.** The field is a pure function of the
  column, so the envelope covers ground **no chunk has been streamed for** -- a structure built
  by projecting resident geometry loses the ridge exactly when a low sun wants it. And the field
  is static per column, so only the *sweep* depends on the sun: the window scrolls under the
  camera and a move of one texel exposes two strips of 512.

**`shade_hit` still casts a real `shadow_ray`, and that asymmetry with `cloud_shade` is
deliberate.** `cloud_shade` has two callers on purpose; `terrain_shade` has one. The ground
shadow is paid per *visible pixel*, where an exact march is affordable, and replacing it with a
quantised envelope would trade a correct shadow for a coarse one to buy nothing. So the ground
gets the march and the volume gets the envelope, and where they disagree it is by `SOFT` at
ridge range, under a lobe that is a glow rather than an edge.

**What it approximates away** -- exact for a world that *is* a heightfield, wrong exactly where
ours is not -- is four entries in [`errors.md`](errors.md): overhangs, caves, tree canopies and
player edits. The canopy is the one with a one-line fix and it was deliberately not taken,
because it is a look decision with its own constant.

### The canopy lift, behind a flag (batch 90, roadmap A1)

**That one line was written at batch 90, as an arm rather than a shipping default.**
`--canopy-lift F` adds F blocks to the fill's height read inside exactly the columns the
tree placer gates admit -- biome tree scale, above the beach, under the tree line -- so
beams stop passing through canopy the camera renders. It changes one expression in
`ShaftField::update`, no shader anywhere, and at **0.0 the field is the pre-arm one bit
for bit** (`tests/batch90.rs` pins both facts, scrolled fills included). What the lift
should ship at is the by-eye pass's, not this file's.

### The texel, unlatched (batch 92, roadmap A8)

**`shaft::TEXEL` was a constant for a slope argument, never a measurement, and roadmap A8 asked for
the sweep.** Batch 92 wired the knob: `--shaft-texel N` (1..=64, clamped so a sweeping script cannot
trip `with_texel`'s assert) and the constant stays as the default, so the shipping picture does not
move. **The arm is one CPU struct field because batch 35 pre-designed it** -- the scan, the sampler
and the upload have all read `frame.shaft_texel` since the field existed, and only the fill, the
anchor arithmetic and the `envelope_reference` partner knew the constant. What changes with the knob
is the *trade*, and the window is where to read it: `DIM` stays 512, so a texel of 2 is a sharper
shadow edge AND a window half the span -- the camera then sits 512 blocks from each edge instead of
1024, inside which long shadows the old window carried stop existing. `tests/batch91.rs` pins that
the fill's centres and the gather's climb follow the knob and not the constant that shipped, because
`envelope_reference` is what the GPU scan is validated against and a stale climb in it sweeps the
wrong function green. The sweep itself -- `--reference` across the ladder, the by-eye verdict where
1 lands -- is the acceptance pass this wire exists for.


---

## The envelope's second reader: the ground past the march

**Batch 37, and it is one call rather than a mechanism.** `shade_hit` marches a real
`shadow_ray` to `shadow_dist` -- a literal **220 blocks** -- and nothing shadowed anything past
it. The envelope already reaches 1024, so the batch is a second tap into a field that was
already being filled and scanned.

**The offset is the whole design, and it makes the handoff a partition rather than a blend.**
The obvious form, `lit * terrain_shade(hit)`, is wrong in the way a screenshot flatters:
`E(hit.xz)` is a maximum over *all* occluders up-sun, including every one the march had just
answered exactly, so the envelope's 4-block quantisation gets a second vote on near geometry and
paints soft false shadows over ground the marcher had resolved to the voxel. Sampling the
envelope `d` blocks **up the sun ray** instead removes exactly the near ones:

```text
E(q.xz) = D*tan + max over s >= D of ( h(p.xz + s*dir) - s*tan )      q = p + sun_dir * d
q.y     = p.y + d*sun_dir.y = p.y + D*tan
```

The two `D*tan` cancel, so `q.y > E(q.xz)` is exactly `p.y > max over s >= D`. **Sampling the
envelope `d` blocks up-sun is the envelope at the surface with every occluder nearer than `d`
struck out of the maximum**, which is why `shade_hit` can hand the march `[0, shadow_dist]` and
this the rest with no gap between them and nothing counted twice -- and why the batch authored
**no crossover distance and no blend width**. There was nothing left to tune once the terms
cancelled. `the_offset_is_the_envelope_with_the_near_occluders_struck_out` asserts the identity
against the reference field to 1e-2 blocks rather than to `SOFT`, because the interpolation
slack the scan is allowed is a different claim and this one must not inherit it.

**The restricted maximum is never above the full one**, which is what makes it safe to multiply
into a march rather than mix with: a maximum over a subset cannot exceed the maximum, so the
distant term can only ever darken a pixel the whole envelope would also have darkened. That is
the test's second assertion, and its third is that the handoff actually removes an occluder at a
quarter of the columns -- without which it would pass on an offset of zero, which is the version
that puts the quantisation back where it does harm.

### What it is worth, which is less than the roadmap claimed

**The entry that asked for this said "at a low sun the whole far field is flat, evenly lit
terrain". That is overstated, and the number that says so is an exact march rather than an
opinion.** A diagnostic build with `shadow_dist` at **1500** blocks -- the honest ground truth,
and the thing the roadmap should have been priced against before the entry was written -- moves
**15,348 pixels of 921,600** at the `lod` camera and **0 to 1,328** everywhere else tried,
across four world seeds. At most **1.7% of a frame**, and usually under 0.1%.

**The mechanism is a squeeze between two conditions, and it is geometry rather than tuning.** A
distant occluder can only shadow a receiver if it clears the sun ray, which by 220 blocks has
already climbed `220 * sin(elev)`. With this generator's relief of 60 to 90 blocks above a
valley floor that needs **elev below about 20 degrees**. But the ground's own `ndl` is
`sin(elev)`, so below about 5 degrees there is almost no direct term left for a shadow to
remove. The window where a distant terrain shadow both exists and is visible is roughly **3 to
20 degrees of sun elevation**, and it is small at both ends of it. Swept over the sun's own
parameter at a fixed camera: **18,062 pixels at `--time 0.26`, 1,104 at 0.30, and exactly 0 at
0.33 and above** -- where 220 blocks of ray has climbed over everything there is.

The second reason is this world: an archipelago has little uninterrupted land fetch up-sun, and
the sea casts nothing. Four seeds at one mid vantage put the exact march at **759, 616, 0 and
602** pixels.

### How close it gets, and the one place it exceeds the march

Against that same 1500-block march, per pixel and by luminance:

| | `lod` (1 degree sun) | mid camera (5 degree sun) |
|---|---|---|
| both move the pixel, **same direction** | 12,955 of 12,955 | 1,121 of 1,121 |
| the envelope darkens it *further* than the march | **0** | **0** |
| the march shadows it and the envelope does not | 559 | 207 |
| the envelope shadows it and the march does not | 2,805 | **1** |

**The envelope never over-darkens**, at either sun, which is the restricted-maximum property
arriving as a measurement rather than as an argument. The last row is the one worth reading:
**one pixel** at a 5-degree sun, where every occluder is close enough for the march to reach,
and 2,805 at a 1-degree sun, where a shadow is kilometres long and `shadow_ray` **runs out of
world** -- it returns `false` the moment the ray leaves the chunk grid, which is `GRID_X` 32
chunks and so half-width 1024 blocks. **So the 1500-block march is a ground truth for in-grid
occluders and not for the rest**, and the envelope's extra pixels at a horizon sun are mostly it
seeing further rather than it being wrong. Anyone repricing this should not quote the 15,348 as
a ceiling without that sentence attached.

The 559 and 207 the other way are the envelope's genuine misses, and they are not separated
here. Two candidates, both already on record as [`errors.md`](errors.md) defects of the same
field: **the canopy**, since `ShaftField` fills from `WorldGen::height` and a tree is a real
occluder to a real march; and the **4-block quantisation**. The canopy lift queued in
[`roadmap.md`](roadmap.md) would move both readers of the envelope at once, and this is the
first measurement that says the ground term wants it too.

### What it costs, and the control

**Both envelope readers were priced again at `terraces` in batch 44, and both read zero.**
`--no-terrain-shafts` is **-0.084 ms +/- 0.068 (t = -1.23, 3/8 rounds positive)** and
`--no-distant-shadows` **-0.016 +/- 0.071 (t = -0.23, 5/8)** of a 10.4 ms `resolve` -- both
inside their own standard error, at the most expensive vantage in the fixture. That is the
shape a fixed scan pass plus a small tap is supposed to have, now confirmed at a water frame
rather than only at the land ones these were measured on.

**`--no-distant-shadows` is the control and it is in `SPEC_MASK`**, so the offset, the bilinear
tap and the four loads leave `resolve` rather than being branched past -- the argument every
override in that list makes, and the one `shade_hit` most deserves, since it runs per visible
pixel and is most of what the pass costs. The tap is also under `lit > 0.0`, so a face the march
has already shadowed does not pay for it, which is `cloud_shade`'s arrangement one line below.

**It is what `--no-terrain-shafts` stopped being able to say on its own.** Batch 35's control
reproduced the pre-batch-35 build because the envelope had exactly one reader. It has two now,
and `--no-terrain-shafts --no-distant-shadows` is the pre-batch-35 revert -- the same shape of
change `--cloud-shadow 0` went through one batch earlier, and `harness implies` carries it as
three claims rather than one.

---

## Controls

| flag | reproduces | cost |
|---|---|---|
| `--fog-density 0` | no haze | -- |
| `--fog-falloff 0` | plain distance fog, the height term's A/B partner | -- |
| `--cloud-cover 0` | no deck, pre-batch-10. Bit-exact at eight | free |
| `--no-terrain-shafts` | no ridge casts into the haze. **No longer pre-batch-35 on its own** -- batch 37 gave the envelope a second reader; pair it with `--no-distant-shadows` | free -- it is in `SPEC_MASK`, so the tap leaves `resolve` rather than being branched past |
| `--no-distant-shadows` | pre-batch-37: the sun term stops at `shadow_dist` and nothing shadows anything past 220 blocks. **14 vantages, 0 pixels, identical hashes** against a real revert; **7 of 14 move** with it on | free -- in `SPEC_MASK` for the same reason |
| `--cloud-shadow 0` | **the deeper of the two, and no longer pre-batch-11 on its own** -- the deck stopped being the shaft's only occluder at batch 35. Bit-exact at nine | **+0.09-0.11 ms** -- the shaft rides through `sky_base`, which every fogged pixel evaluates |
| `--godray-strength 0` | keeps the ground shadow, so it does **not** reproduce pre-batch-11 alone. Since batch 35 it is the only control that switches off *both* occluders | -- |

**`--cloud-shadow 0 --no-terrain-shafts` absorbs `--godray-strength 0` at 0 pixels; the reverse
moves 890 to 4,269.** That asymmetry is deliberate and was verified in both directions -- and
**batch 35 is what put the second flag in the first half of it**, which is the shape of change a
composition table exists to force into the open. The pair is completed by the other direction:
with the deck's shadow already off, adding `--no-terrain-shafts` still moves **393,912 pixels at
`sky`**, which is what says the first claim is a claim about composition and not about a control
that was never wired up.

**Every sky-adjacent feature is most expensive down a coastline, not in a sky-filling frame** --
see [`water.md`](water.md) and [`errors.md`](errors.md). Clouds cost +0.16 ms at a vantage with
almost no sky in it and **exactly zero** at the same vantage with `--no-water`.



