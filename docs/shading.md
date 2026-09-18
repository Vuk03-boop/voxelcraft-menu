
# Shading: light, AO, the tone map and the biome tint

**Consolidated from batches 1-4 and 12.** How a voxel face gets its colour, and the biome tint
that modulates it. Rejections are in [`errors.md`](errors.md).

---

## Everything is linear radiance, end to end

`out_tex` is `rgba16float` because sRGB is not a legal storage format, so `resolve` **cannot**
encode; `blit.wgsl` is the one place that tone-maps and encodes, and it only encodes by hand
when the target format has no hardware sRGB (`BlitUniform::encode`).

**A `pow()` anywhere in `resolve`, or a display-referred constant in `common.wgsl`, puts the
pipeline back where batch 2 found it.** This is the single most load-bearing rule in the
renderer, and two later batches have had to route around it rather than break it -- the biome
tint's constants are pre-raised (below) and the meadow's compensation is an albedo.

**Where it landed.** Sunlit grass top against shadowed dirt side went **35.9:1 to 11.6:1**, of
which 2.0:1 is the textures' own albedo -- a **5.9:1 lighting ratio, inside the 3-6:1 daylight
band**. Crushed pixels (max channel < 16) went 56.6% to 1.9% in a cave and 38.3% to 0.14% at
night. Distant terrain trends toward sky (mean luminance 0.021 to 0.045) instead of collapsing
to black.

---

## Light

- **`face_shade` and `light_curve` were left alone.** Read as linear they are already what they
  claimed to be: 1.00/0.80/0.62/0.50 is Minecraft's own face split, and `l*l*(0.6+0.4l)` tracks
  Minecraft's `0.8^(15-level)` to within 10%. Gamma space had been silently squaring both.
- **The sky and sun tints are decoded display colours** (`0.80,0.86,1.00` becomes
  `0.6038,0.7084,1.0`). In linear they still sum to near-neutral under full sun.
- **`ambient` is 0.08, not 0.20, and it lives *outside* the sky-tinted term.** As an encoded
  value 0.20 contributed `0.20^2.2` = 0.03 of actual light; carried into linear it lit caves and
  midnight like an overcast noon. It is also the only light in an unlit cave, and wearing the
  sky's tint it turned stone navy.
- **The ambient floor is outside the AO product too.** At `AO_STRENGTH` 0.25 it has to be: 0.08
  times a fully occluded corner lands back under the display's first code values. It is also
  right regardless -- the floor stands in for bounce light the two-source model never simulates,
  not for a hemisphere a corner can occlude.
- **Block light carries its own tint, mixed per corner -- and since batch 59 that tint is
  *derived* rather than authored.** One scalar `max(sky, block)` wearing the sky tint lit caves
  blue under glowstone, which is what put a second tint here in the first place. Per corner
  rather than per face also stops a torch beside a face warming the far side of it, and daylight
  captures are bit-identical across that split. What batch 59 changed is where the colour comes
  from: three flood channels, and the tint is what reached the cell. See below.
- **Corner light is averaged *after* `light_curve`, not before.** Averaging levels then curving
  puts the blend back into a nonlinear space, which is the one thing batch 2 was about. Solid
  cells are left out of the mean; the centre cell is the air the ray arrived through, so there
  is always at least one sample.
- **Direct sun is gated by the *smoothed* sky term.** The gate stops sun leaking onto a face the
  shadow ray missed; taking it from the flat sample would put a per-voxel step back into an
  otherwise smooth face.
- **The corner magnitude is `max(sky, block)`, Minecraft's rule.** Summing the two tinted terms
  instead would brighten where both overlap -- a look decision, parked, not a correctness fix.

**`world_light` and `world_solid` are gone.** `world_sample` returns both from one grid walk and
`gather_face` does the nine in-chunk cells with the tree descent shared between neighbours.
Sampling the shaded chunk directly also fixed a quiet wrong: the chunk grid only spans a window
around the camera, so chunks past it used to sample as full sky and never occluded.

---

## Two light models, and which surface gets which

**Batch 45.** `shade_hit` is called four times per frame in the worst case -- once for the
surface a primary ray lands on, and once each for water's refracted leg, water's reflected leg
and glass's transmitted leg -- and since batch 45 the primary keeps the nine-cell gather while
the other three take one lookup.

**What the gather buys is a cue about where a face meets its neighbours**: per-corner smoothing
of the flood's levels, and the AO below. **All three secondary transports destroy that cue
before the pixel is written** -- a reflection is Fresnel-weighted at a grazing angle off a
wave-perturbed normal, a refracted hit is multiplied by `exp(-extinction * depth)` on the way
back to the eye, and a transmitted one is dimmed by the pane's own texel. So the cheaper model
is not an approximation of the expensive one at these hits; it is the same information after
the transport has removed the part the extra eight lookups paid for.

**It is worth -0.5 to -1.68 ms of `resolve`** at every vantage benched, which is the largest
saving here since Hi-Z, and it moves **230,613 pixels at `terraces`** and **0 at `underwater`**.
Shallow clear water looking straight down is where it shows -- the submerged steps lose the
darkening at their inner corners -- and a hazy or deep path is where it does not. `PERF.md` has
both tables and `--no-flat-secondary` is the control, bit-exact against `voxelcraft-pre45.exe`.

**The flat arm is the cross-quad's own model, one block over.** A tuft is shaded flat because
it has no face to gather across; a secondary hit is shaded flat because nobody can tell. The
one difference is *where* the light is read: a tuft takes `light_at(ci, v)` at the voxel it
stands in, because the flood runs through a foliage voxel, and a solid block's own cell holds
no light -- so this reads the air cell in front of the face, `v + n`, and falls back to
`world_sample` on the thin border where that crosses out of the chunk.

**How it is gated is a `gpu.md` entry rather than a shading one**, and it is worth following
the link before touching this: the arm has to be selected by the override *alone*, with no
`frame.flags` test ANDed to it, or the feature costs a millisecond instead of saving one.

## Ambient occlusion

**`AO_STRENGTH` is 0.25**, so three occluding neighbours leave exactly a quarter of the light --
Minecraft's own vertex step read as linear radiance. Restoring the old *screen* value would mean
0.113 and that is the wrong target: **bilerp spreads each corner's darkening over a quarter of
the face where the nearest-corner version confined it to the rim**, so the same number reads
heavier and the peak is reached over area rather than at a seam. 0.32 was tried and turns the
dirt murky.

AO now touches **90.1% of shaded pixels instead of 49.5%** at the same peak contact darkness.

**Water does not occlude for AO or the light gather** -- a sea floor face has three water
neighbours in its 3x3 and counting them as solid renders a lit sandy bottom black. See
[`water.md`](water.md).

---

## The tone map and the mip footprint

**The tone map is a shoulder and nothing else.** `KNEE = 0.75`, C1 at the knee, asymptotic to
1.0, **exact identity below it**. Nothing tuned against the linear image is moved by it, and the
sun disc rolls off instead of clipping to a flat white blob.

**The mip footprint's grazing stretch is clamped to 4x.** The real footprint on ground at a
grazing angle is `1/|dot(n, rd)|` larger, which is where the mid-ground moire came from;
unclamped it turns the ground to mush. (`shade_water` deliberately does *not* carry this
correction -- see [`errors.md`](errors.md), correcting it measured monotonically worse.)

### The shoulder is swappable, behind a flag (batch 90, roadmap A10)

**Built, default off: `--tone-map knee|aces`.** The knee above is this file's position -- it is
the hyperbola every exposure and brightness constant in the engine was tuned against, and it
stays the shipping law until a stocktake says otherwise. Batch 90 added the thing roadmap A10
actually asked to *build*: a uniform selector at the blit, one word of `BlitUniform`, with
Narkowicz's 2015 ACES fit (`x(a x + b) / (x(c x + d) + e)`) behind value 1. **Not a pipeline
variant** -- the selector chooses only which shoulder the highlight rolls onto, and per the
research review's own warning, the curve is the cheap part. Also not AgX: the hue-preserving,
per-channel-matrix prescription is deferred, and the by-eye verdict on this arm is the data that
picks whether that follow-up exists. A guardsman detail with a rule of its own: an unrecognised
shoulder spelling falls back to `knee`, because a tone map the engine will not run is worse
silently than the spelling it did not get.)

---

## The biome tint

A per-pixel multiply on albedo, evaluated from the shaded point's world XZ, storing **nothing**.

### The field exists twice, and only one copy generates the world

`biome_noise` in `resolve.wgsl` is a transliteration of `biome::simplex2`, which is what
`WorldGen` actually calls. They must agree about **where a biome is**, not bit for bit: the
palette snaps at `BAND` while the tint blends across `BLEND` either side, so there is no
threshold for a last-bit difference to fall the wrong side of. A *scrambled* field is the only
failure a transliteration typo actually produces.

| link | what keeps it honest |
|---|---|
| `fastnoise-lite` to `biome::simplex2` | `noise_matches_fastnoise_lite`, bit-exact over 570k columns, three frequency regimes |
| `biome::simplex2` to `biome_noise` | line-for-line transliteration, one documented departure |
| `BAND`, `BLEND`, seed offsets, the 3x3 of ids | `const _: () = assert!` in `render/mod.rs` |
| the two frequencies | not mirrored -- they travel in `GpuFrame` |
| the six colours | not mirrored -- they exist only in WGSL |

**A transliteration source that nothing uses would have rotted; this one cannot, because the
world is generated from it.**

### The atlas alpha channel was already free

`textures.rs` wrote 255 into channel 3 of every texel and `resolve` took `.rgb`. Nothing had
ever read it -- a spare per-texel channel, linear even in an sRGB format, already carried down
the mip chain by `downsample` averaged separately from the colour.

It is now a **tint mask**: `px` writes 0, `pxt` writes 255. **The split is per *texel*, not per
layer, and that is the whole reason it works.** Grass's side texture is one layer holding both
the green fringe and the dirt under it; a per-layer flag would tint the dirt, which is exactly
what Minecraft's separate side *overlay* exists to avoid. The trilinear filter then fades the
tint across the boundary instead of stepping -- including at distant mips where the two have
blurred together and the mask has blurred with them.

### The multipliers are linear, which is why they had to be authored squared

`albedo` is what the hardware handed back from an sRGB atlas, so it is **already linear**, and a
multiplier there reads on screen at roughly its 1/2.2 power. The first set was authored as if it
were sRGB and moved **a maximum of 12 code values across an entire frame**.

| biome | linear | apparent |
|---|---|---|
| plains | 1.000, 1.000, 1.000 | identity, by construction |
| forest | 0.664, 1.045, 1.000 | 0.83, 1.02, 1.00 |
| savanna | 1.842, 0.935, 0.793 | 1.32, 0.97, 0.90 |
| desert | 2.096, 0.957, 0.699 | 1.40, 0.98, 0.85 |
| taiga | 0.832, 0.935, 2.096 | 0.92, 0.97, 1.40 |
| tundra | 0.755, 0.893, 2.812 | 0.88, 0.95, 1.60 |

**The correction lives in how the six constants were chosen, once, and not in a `pow` in
`resolve`.**

The *shape* follows Minecraft's own grass palette: what separates its biomes is mostly red and
blue with green nearly fixed. **Plains is exactly (1,1,1) by construction, not taste** -- it
makes the tint a departure from the atlas rather than a wash over it, so a diff against
`--no-tint` shows the biomes and not a global multiply.

**Two of six biomes take almost no tint and one takes none** -- the desert and the tundra
because their surface layers carry alpha 0, the plains because its multiplier is the identity
by construction. The defect is now **armed rather than open**: batch 92's `--tint-balance`,
see below; the pre-arm complaint stays on file in [`errors.md`](errors.md).

### The balance, behind a flag (batch 92, roadmap A8)

`--tint-balance` (FLAG_HI_TINT_BALANCE 512, `SPEC_TINT_BALANCE`) moves three of six biomes
toward readable without touching the default, and it is *two halves under one bit*:

- **The shader half is the first constant *swap* the second word has keyed, not new code.**
  A `select` on the compile-time override picks `TINT_PLAINS_BALANCED` (0.935, 1.067, 0.892
  linear; apparent 0.97, 1.03, 0.95, a warm-green lean) where the batch-12 row sat, and off,
  naga folds the select to the identity row -- the off pipeline is bit-identical and the on
  one is the same instructions with other immediates. It costs no registers in a pass whose
  batch-67 entry is exactly about register reads, precisely because nothing branches or adds.
- **The atlas half is content, and that is why it is a `build_atlas` argument and not the
  override's.** No pipeline key can put alpha into a file written without it: armed, the sand
  and lichen layers gain mask 255 (lichen's stone blobs stay bare, the grass-side fringe's
  rule), and `tests/batch92.rs` pins the contract -- colour bytes identical at both arms,
  alpha differing on exactly two layers and nowhere else.

**The known spill, the entry's own sentence, is every non-desert beach.** A mask is not
biome-aware: sand masked for the desert also takes taiga blue on a taiga shore. Whether that
reads as detail or as staining is the verdict the by-eye pass is for; under the batch-90b
acceptance rule the arm is not built until it moves pixels at a named vantage past max delta
1, and which picture is the beach is a picture question.

### The blend was not the problem

The obvious suspect for a weak tint was the blend, since `band_weights` only saturates past
`BAND + BLEND`. Measured over 95k columns it was innocent: **60.7% of columns are more than 90%
one biome**. Widening `BLEND` would have changed the *terrain's shape*, which reads the same
constant, to fix six numbers the terrain does not read at all. **Measure the suspect before
widening a constant something else reads.**

### Cost

**`--no-tint` is free since batch 13's `SPEC_TINT` fold.** It cost **+0.07 to +0.11 ms** before
that while executing nothing -- not occupancy but code size, from `biome_noise` being compiled
into `resolve` at all, which made the fixed cost of the feature existing *larger* than the
variable cost of it running. That finding generalised and is in [`errors.md`](errors.md).

**The coastline is the tint's *cheapest* vantage**, inverting the pattern batches 10 and 11 hit.
`shade_water` calls `shade_hit` twice more, but those hits are sea floor and sky, which carry a
zero mask -- the tint is bounded by *mask coverage*, not by how much sky a ray reaches. The
expensive vantages are the ones with vegetation filling the screen: the default camera and the
middle of the night.

---

## Glass, which is the water model with the medium taken out

**Batch 38b.** `block::GLASS` had shipped since the first batch as an opaque cube with a pale
blue texture on it, so a window was a wall you could see the bricks of. It is now the third
material `resolve` knows: `shade_water` for the sea, `shade_glass` for a pane, `shade_hit` for
everything else, chosen on the block id the deferred pass reads back out of the attributes.

**Water and glass are the same optics with the medium removed**, which is why the design is a
generalization rather than a second feature. Both are a surface the primary ray stops at, one
secondary ray for what is behind it, and a Fresnel weight toward a reflection. What water has
and glass does not is a *body*: the sea's transmitted leg travels through an absorbing medium
and converges to `body` with depth, and a pane is one block thick. So `shade_glass` has no
Beer-Lambert, no `water_medium`, no depth and no near/far ramp -- the transmitted colour is
the hit behind the pane times the pane's own texel, and that absence is most of why the
function is a third of `shade_water`'s length.

Two constants, and only one of them is authored:

- **`GLASS_F0 = 0.0426`**, which is `((n-1)/(n+1))^2` at soda-lime glass's n = 1.52. Double
  water's 0.02 -- which is why a pane catches the sky at angles a lake still looks through,
  although both curves reach 1 at grazing and it is the *shape* that makes either read as a
  surface.
- **`GLASS_TRANSMIT_DIST = 192`**, against water's 96, and the one number with a judgement in
  it. A transmitted water ray is looking for a sea floor a few blocks down inside a medium that
  has converged long before the cap; a window is looking at a landscape, and a greenhouse
  whose far wall dissolves at ninety-six blocks is a worse artefact than the cap prevents. It
  is a *reach* rather than a look constant and has not been swept, because the batch's own
  vantage holds nothing 192 blocks behind its glass and a sweep would have measured zero.

### The transmitted ray does not bend, and that is the one place the generalization stops

**The batch shipped `refract()` here first, copied from `shade_water` line for line, and it put
a double image inside every glasshouse**: looking into a corner, the plinth appeared once
through each wall, displaced symmetrically outward, and the hillside seen through a pane was
visibly offset from the same hillside beside it.

A slab refracts **twice** -- once going in and once coming out -- and for a pane one block thick
the two very nearly cancel, leaving a lateral offset of a fraction of a block. `skip_glass`
means the transmitted ray never finds the exit face, so only the entry half was ever applied and
the bend was never undone. A ray that carries straight on is therefore *more* physical than the
bend it replaces, not a simplification of it.

**The medium is exactly why water can do what glass cannot.** A refracted water ray really does
continue inside the water, all the way down to the sea floor, so its single refraction is the
whole story and the cancellation never arises. That is the same sentence as "water has a body
and glass does not", read one term further along, and it is why `GLASS_IOR` is not a constant in
this engine while `WATER_IOR` is: the reflectance depends on the index, and the path no longer
does.

### `schlick_glass` is a second function on purpose

Adding an `f0` parameter to `schlick` and passing `WATER_F0` at the old call sites is the same
value through a different expression, and [`errors.md`](errors.md)'s batch-38 entry is what
that costs here: a folded-away difference of one ULP took a control's bit-exactness for 201
pixels. Water's Fresnel is read by every sea pixel in the fixture. Five lines of duplication
is a cheap way to guarantee it is byte-for-byte the function batch 8 shipped, and the
fourteen-vantage sweep is what says it worked.

### The transmitted ray is the one `trace_world` caller that looks for water

**It shipped skipping it, and the sea was invisible through a pane.** `trace_world` hard-coded
`skip_water = true` from batch 8 until a user looked through a window at a bay and got the
unlit seabed back. The literal was right for all three callers water had and wrong for the
first one glass added; it is a parameter now, and the three water-side sites pass the `true`
they always had.

Stopping at water is only half of it. **A water hit has to be shaded by `shade_water`**, the
same dispatch the primary ray makes -- handing it to `shade_hit` would draw the sea through a
window as a flat blue cube face, which is worse than the bug because it looks deliberate. So a
glass pixel over water spends three secondary rays: one through the pane, and water's own two.
Bounded, and paid only where a pane covers a sea.

`only_the_transmitted_ray_looks_for_water` and `water_seen_through_a_pane_is_shaded_as_water`
pin both halves, and the first was checked by reinstating the bug and watching it fail.

### The cap at two interfaces is an asymmetry and not a counter

The research asked for glass hard-capped at two interfaces so a double-pane greenhouse
resolves. There is no counter anywhere. `march_chunk` grew a `skip_glass` bool beside
`skip_water`, the primary ray passes `false`, and **every secondary ray passes `true`** -- so
the primary hit is interface one and every pane behind it is air to the ray that goes looking.
That is water's own answer to the same question, unchanged since batch 8, and it means a
greenhouse resolves to what is behind *both* walls rather than to a second wall.

`only_the_primary_ray_stops_at_a_pane` in `tests/glass.rs` is what holds the four argument
lists to it, because the rule is stated nowhere else.

### What it does not do, and each is a batch rather than an oversight

- **No traced reflection.** The Fresnel weight goes to `sky_color` alone, which is batch 8's
  own split -- refraction was 8b and the traced reflection 8c. A pane facing a hillside shows
  sky where it should show hill, worst at grazing where Fresnel is largest.
- **Glass is invisible to every secondary ray**, which is the same `skip_glass` rule read from
  the other end: a glass house reflected in a lake shows the world behind its walls. Water has
  been exactly this since batch 8 and no defect has ever been filed against it.
- **The second pane's Fresnel and tint are lost**, which is the price of the paragraph above.
- **A pane is not lit.** Neither term is an albedo under a light, so there is no gather, no
  shadow ray and no `light_at` on this path.

### The light half, and why no control can revert it

`BlockDef::opaque` went to `false`, so the sky flood runs through a pane and a glass house is
lit rather than being a black box with a window in it. **This is a property of the world, not
of a pipeline**: `light.rs` reads `opaque` while a chunk is being built, long before a
`SPEC_GLASS` override exists. So `--no-glass` reproduces the pre-38b *shading* over post-38b
*lighting*, and it is the first control in this project that is not a pure revert.

Measured rather than asserted: `--no-glass` against `voxelcraft-pre38b.exe`, with the same
glasshouse in both, is **131,689 pixels of 921,600 at a max channel delta of 17** -- and
**exactly 0 pixels** in two patches of hillside away from the structure, so the impurity is
bounded to the glass itself and its own faces. Where no glass has been placed the two builds
coincide, which is every vantage the fixture had before this batch.

The flood is also *clear* rather than attenuating: water costs a level a block on the vertical
pour and glass costs nothing. `a_glass_roof_lets_the_sky_through_and_planks_do_not` is the
check, and the plank half is what makes it a measurement -- same room, one block id different,
fifteen against zero.

### The pane's reflection traced, behind a flag (batch 90, roadmap A4)

**Built, default off: `--glass-reflect`.** Batch 38b's `refl` asks the sky what a pane returns;
this arm asks the world -- one `water_reflect_leg` march, which despite its name marches terrain
and never re-enters either transmissive shader (the recursion note at `shade_glass` is the
reason that is legal, and the function keeps its name on purpose). On a miss the leg's alpha
is zero and the sky lookup the arm already paid for stands, so a high window's fallback row has
no hole in it. The arm folds out of the pipeline when off, so the shipping pane keeps the sky
assumption until the by-eye pass says whether the traced world beats it -- the batch-54 finding
about transports and secondary rays is exactly the opposing bet, and the two now share one flag.

---

## The probe tap, which shades nothing (batch 55)

**`--probe-tap` is the only thing in `resolve` that is not there to make a picture**, and the one
switch in the tree whose *off* state is the shipping build. It takes one hardware trilinear tap
into an RGBA16F field per shaded surface and multiplies `amb` by the result; the field holds a
constant 1.0, so `amb * tap` is bit-exact for every finite `amb` and the whole output is a
millisecond.

**Batch 57 refilled that texture with a real bake and this row still holds**, because `--probe-tap` now implies `--probe-fill 1.0`: the field it samples is pinned back to a constant, so `amb * 1.0` is bit-exact for the reason it always was. The field itself is no longer 128^3 -- it was 64 x 384 x 64 at batch 57 and is 128 x 768 x 128 since L5 halved the spacing, six stacked slabs of the ambient cube -- and the tap's address arithmetic changed with it, so **re-measure before quoting this row's milliseconds against a future build.**

**What it is for.** Roadmap L1 wants to replace the ambient floor and `face_shade` with a real
directional, coloured, bounced irradiance field, baked at chunk build on sixteen idle cores. That
design lives or dies on whether `resolve` can afford to *read* such a field, and the honest prior
was that it could not: the pass is instruction-bound at 96 registers, and batch 47 measured the
nine-cell gather inside it as instruction-bound too and failed to make it cheaper. **One tap,
priced before any format exists, is the cheapest question that settles the premise** -- and it
needs no bake, no storage decision and no picture.

**Three readings, because bit-exact is the one result that cannot speak for itself.**
`lessons.md` puts it as *a no-op claim cannot be told apart from a not-wired-up claim*, and a
sweep whose pass condition is **0 pixels differing** cannot tell a free tap from an absent one:

| reading | says |
|---|---|
| `--probe-fill 0.5` moves the frame | the tap is read |
| `shaderstats`: `resolve` **+1280 bytes**, **96 registers both sides** | the tap is compiled in, and costs no register pressure |
| `bitexact`, 18 vantages, 0 pixels, identical hashes | the tap is invisible at 1.0 |

`tests/probe.rs` holds the two source claims the fixture cannot: that the gate is the override
**alone** -- batch 45's rule, and here a run-time gate would make the shipping build pay for a tap
it never takes and the batch's whole reading would be of itself -- and that the combiner is a
multiply, since an add or a `mix` is defensible on its face and destroys the bit-exactness the
measurement rests on.

**And batch 56 priced the other side**, which is what turns the tap from an interesting number
into a decision. `--probe-ambient` makes the field *replace* `amb` rather than multiply into it,
and the terms it makes redundant leave the module: the nine `blk_n` `light_curve` evaluations, the
four per-corner `bk/(sk+bk)` divisions with their tint `mix`, the `amb` bilinear, **`face_shade`**
and **`floor_rgb`**. What stays is what a coarse probe could not carry -- the nine gather lookups,
`sky_n`, the AO rule and `sky_sm` -- because AO is a contact cue and `sky_sm` gates a hard shadow,
and both are high-frequency where indirect light is not.

**It implies `--probe-tap`, so the figure is net, and it is a saving**: **+0.130 ms at `cave`,
+0.091 at `default`, +0.030 at `sky`**, two of five vantages inside their own error, `resolve`
582,400 bytes down to 575,360 at 96 registers either way. **The saving tracks surface density
where the tap's cost did not**, which is the mechanism check: one is work and the other was code.
So the two constants this document has described since batch 2 -- the per-normal `face_shade`
split and the ambient floor that "stands in for the bounce light the two-source model never
simulates" -- can be replaced by something derived **and** the pass gets faster.

**What neither batch priced**: the field held a constant, not irradiance, so what transfers is
the read cost and not the assumption that one tap carries sky-indirect, block light and bounce
together. And nothing about the bake, which was all of R1's actual work -- **batch 57 is where
that happened**, and the section below it is what it found. `PERF.md` has the numbers.

**Both diagnostics still work and still mean what they meant**, because `--probe-tap` and
`--probe-ambient` now imply `--probe-fill 1.0`: the texture they sample holds real shading
factors in the shipping build, and pinning it back to a constant is what keeps `x * 1.0` the
reason the tap is bit-exact.

---

## The ambient cube, which is what the tap was pricing (batch 57)

**`face_shade` is no longer the answer; it is the open-field calibration of one.** A baked
six-axis field says how much of an open field's sky each position still sees along each axis,
`shade_hit` multiplies the table by it, and the two constants that used to be the whole of
directional occlusion now only decide what an *unoccluded* surface looks like. A wall at the
foot of a cliff is darker than the same wall in a field; a valley floor is darker than a ridge;
a ridge is exactly what it was.

The field, the bake and every constant in it are in [`src/probe.rs`](../src/probe.rs) with the
reasoning at each definition. What belongs here is the part that is about `resolve`.

### Six axes is exact here, and that is why the read is one tap

Every cube face in this world has an axis-aligned normal. A six-entry cube -- one value per
`normal_id` -- is therefore not a low-order fit to a hemisphere the way it is in a triangle
renderer: for a cube face it is **exact**, and `shade_hit` evaluates it by *selecting one entry*
rather than reconstructing an SH basis. SH L1 is the textbook choice and stores 12 floats
against 18; here it would buy smaller storage in exchange for ringing, possible negatives and a
reconstruction nothing would use.

The six entries are six slabs stacked along the field's Y, and a face samples only the slab its
normal names -- so the operation count is **one** hardware trilinear tap, which is the number
batch 55 measured. Picking the slab is a switch on `normal_id`, which `face_shade` already is.

### It stores occlusion, not shading, and that is what makes every absence exact

The value in the field is 1 for an open field. `shade_hit` computes `face_shade(id) * tap`, so
an unbaked texel, a chunk at a coarse LOD, a pinned `--probe-fill 1.0` and a probe buried in
rock all reduce to `face_shade(id) * 1.0` -- the pre-batch-57 frame **to the bit**.

**The first build of this batch baked the product instead and the sweep caught it**: `cave`
moved 367,222 pixels at max delta 1, because 0.62 and 0.80 do not survive a round trip through
`f16` and 1.0 does. A whole vantage of noise, from storing a number the shader already had.

The shipping build leaves `cave` at **30,300 pixels, still at max delta 1** -- not zero, and
the residue is real: the pocket is shallow enough in places that its probes do see a little
sky, so the no-sky ramp is part-way rather than off. What removed the other 339,000 was making
that ramp a `smoothstep` instead of a linear one.

### The normal bias, which is the difference between the feature working and half working

The tap is taken at `hit + n * PROBE_SPACING * 0.5`, two blocks along the surface normal. A
surface sits on the boundary its own occluder defines -- the face of a cliff is the plane where
the height field steps -- so the probe nearest an unbiased `hit` is as likely to be *inside* the
terrain, where the cube has nothing to say, as in the air the face looks out into. Half a probe
rather than a block, because the lattice is 4 blocks coarse and a one-block nudge leaves three
quarters of the weight where it was.

### The X/Z asymmetry stays, and the batch that tried to delete it is why

`face_shade` gives X sides 0.62 and Z sides 0.80, which is a readability convention Minecraft
invented and this engine inherited: in an open field the sky is azimuthally symmetric, so two
side faces of one cube genuinely do receive the same ambient. **The first build of this batch
stored the absolute visibility and mapped an open side face onto 0.75 between them.** It is the
more honest physics and it made the picture worse: `default` came back **brighter and flatter**,
because that frame is mostly X-facing steps going 0.62 to 0.75, and the occlusion the batch
exists to add was buried under an exposure shift nobody had asked for.

That is `lessons.md`'s *change one thing per build* meeting this batch's instrument. The look
change is ranked by a **before/after screenshot pair**, and a pair cannot separate two changes.
So the cube normalises against the open field, the image moves only where the terrain actually
occludes, and the asymmetry is left for roadmap R2 and R5 to retire -- a real bounce and a real
sky integral give the absolute number a reference that is not an authored constant.

### What it cost, and where

| reading | value |
|---|---|
| `resolve`, `cave` (most surface-dense), hot | **-0.042 ms +/- 0.002**, t = -26 |
| `resolve`, `cave`, an hour later, cool | **-0.020 +/- 0.009**, t = -2.2 |
| `resolve`, `default` | -0.050 +/- 0.054, indistinguishable from zero |
| `resolve`, `terraces` | +0.046 +/- 0.029, indistinguishable from zero |
| `shaderstats` | 582,400 -> **584,832 bytes**, **96 registers either way** |
| the bake | **1912 us per chunk**, beside the flood's 1344 -- `PERF.md` |

**Quote the range and not either number.** The two `cave` readings are the same paired A/B an
hour apart and they disagree by a factor of two on magnitude while agreeing on sign; the
vantage's own absolute `resolve` moved **2.73 to 2.36 ms** between them, which is `PERF.md`'s
power-state caveat rather than anything about this feature. The honest figure is **0.02 to
0.04 ms at the most surface-dense vantage in the set**.

**And it is batch 55's number arriving on schedule.** That batch priced one tap at `cave` at
-0.040 +/- 0.000, which sits inside this range. What
this batch does **not** bank is batch 56's saving: that figure is what a pre-composed field lets
`resolve` *delete*, and this rung deletes nothing -- the nine `light_curve` evaluations, the
per-corner divisions, the `amb` bilinear and `floor_rgb` are all still there, because a field
carrying sky occlusion alone cannot replace them. **R2 is the batch that collects it.**

### What the field cannot see, and why that was the right trade

The occluder is the height field and nothing else -- no trees, no caves, no overhangs, no edits.
In exchange the bake is a **pure function of world position**, which buys three things the
roadmap's planned 10^3 apron could not: the lattice has no seams at all (roadmap P5b, answered
by construction rather than by an invalidation graph), a probe is occluded by terrain 190 blocks
away rather than one chunk away, and a chunk never needs re-baking because a neighbour arrived.
The flood still carries the canopy and the cave -- what is lost is their contribution to
*directionality*, not to level. R2 gathers a bounce at the probe and does need real geometry;
that is the batch that pays for the apron. **Batch 58 gathered it and did not need to** --
see the next section, which is where that expectation is corrected.

---

## The ground bounce, which is what the floor was standing in for (batch 58)

**Roadmap R2's first rung, and the constant it retires names itself.** `floor_rgb` is
`(0.85, 0.90, 1.00) * frame.ambient`, kept outside the tinted term and outside the AO product,
and the comment at its own definition site has read *"it is what stands in for the bounce light
the two-source model never simulates"* since batch 3. This batch simulates one. The comment was
the acceptance test and it is the thing that passed.

**What changed is the bake and one multiply.** `probe::bake` gathers the albedo of the ground
each axis can see -- off the horizon march batch 57 already pays for -- and stores a per-channel
multiplier on that constant in the `.rgb` of the texel whose `.a` is batch 57's occlusion.
`shade_hit` multiplies. There is no new pass, no new resource and **no second tap**: one
`textureSampleLevel` returns a `vec4` and the two features read four channels of it.

### The two identities, which are different claims and only one is per channel

**This is the thing to get right before reading anything below**, and a draft of it was wrong
inside this session.

- **An unbaked texel returns 1.0 on every channel.** So do a coarse LOD, a pinned
  `--probe-fill 1.0`, a probe with no sky over it, and `--no-probe-bounce`. `floor_rgb * 1.0`
  is `floor_rgb` to the bit, which is what makes the control a **pure revert -- 18 of 18
  vantages, 0 pixels, identical file hashes** -- by construction rather than by luck. It is
  batch 57's own argument for storing occlusion rather than shading, one rung on.
- **Reference ground returns 1.0 in *luminance*, and not per channel.** Over grass the
  multiplier is about `(0.416, 1.259, 0.156)`: the floor turns green at *exactly* the
  brightness it had. That is what stops this batch doing what batch 57's first build did --
  burying the feature under an exposure shift a before/after pair cannot separate from it.

`tests/probe.rs::reference_ground_leaves_the_exposure_alone` asserts the second; the first is
what `bitexact` measures.

### How the bounce is gathered

Per probe, per axis, the cosine-weighted mean albedo of the ground that axis can see:

- **The albedo** is `WorldGen::surface_block_at` at each march sample -- the generator's own
  palette rule rather than a second copy of it, so a column under the sea gathers *water's*
  albedo and not the sea floor's, with no test here saying so. `block::ALBEDO` is the mean
  linear albedo of each block's **+Y face**, a `const` table pinned against the generated atlas
  by `albedo_matches_the_atlas` -- a hand-written table is exactly the thing that drifts, and it
  drifts in a way no capture would attribute to this.
- **The radial weight** is the solid angle an annulus of ground at radius `r` and drop `d`
  subtends at the probe, `r*dr*d^2/(r^2+d^2)^2`, derived rather than authored. **It peaks at
  `r = d`**, which is the whole of what makes the tint local: a probe four blocks up is
  coloured by the ground four blocks away and one sixty blocks up by the ground sixty blocks
  away.
- **The azimuthal weight** is batch 57's own cosine lobe for the four sideways axes, and flat
  for the two vertical ones -- a horizontal face has no azimuth to prefer. **So this field's
  per-axis variation is azimuthal**: a +X face is coloured by the ground on its +X side. A +Y
  and a -Y face at one probe read the same value, because the same ground surrounds them.
- **`believe` is batch 57's weight, reused and not recomputed.** Where a probe sees no sky
  there is no sky-lit bounce either and the authored floor -- *"the sole light in an unlit
  cave"* -- is still the best answer. One weight for both fields is also what keeps a cave from
  acquiring a green cast off the grass above it, and `cave` moving **27,415 pixels at max delta
  1** is that rule holding rather than the feature reaching.

### What it costs, and the honest form of the claim

| | |
|---|---|
| bake, per chunk | **1868 -> 2549 us**, +681 |
| `resolve` at `cave` | **-0.001 ms +/- 0.005**, t = -0.24 |
| `resolve` at `default` | **+0.008 ms +/- 0.032**, t = +0.23 |
| `resolve` code size | 584,832 -> **584,960 bytes**, +128 |
| registers | **96 either way** |

**The bake is 1.36x and not the "several times" the roadmap expected**, because the gather rides
the march that already exists: what it adds per sample is one biome lookup on every other radius
and a form factor, and it cannot add a sample the horizon does not already take. Chunk build
goes 6063 -> 6703 us, paid on sixteen rayon threads that are idle once the world is resident.

**Neither frame reading is distinguishable from zero, and the detection threshold is about
3 se -- 0.015 ms at `cave` and 0.096 at `default`.** That is the honest statement and it is
weaker than "free". What makes it believable rather than merely unresolved is the mechanism:
this is the *same* `textureSampleLevel` batch 55 priced and batch 57 shipped, +128 bytes of
multiply, at an unchanged register count. `one_tap_serves_both_fields` in `tests/probe.rs` is
what stops a later edit splitting it in two -- that would double the read cost while leaving
every picture identical, so no capture, no control and no `bitexact` row could catch it.

### "18 of 18 move" is the number that looks like proof and is not

**A global tint would also move 18 of 18 vantages**, and nothing in the sweep separates the two.
`lessons.md` records the negative form of this -- *a no-op claim cannot be told apart from a
not-wired-up claim* -- and this is its positive form: **a feature that moved the whole frame
cannot be told from a constant that moved the whole frame.** So the three things the field
claims to be are each read somewhere the sweep cannot reach, in `tests/probe.rs`, off a real
`WorldGen` at a chunk *searched* for two surface types rather than hardcoded:

| test | what it catches | margin |
|---|---|---|
| `the_bounce_varies_across_a_chunk` | a global constant | min to max spans > 1.5x over mixed ground |
| `the_bounce_varies_between_axes` | the azimuthal lobe dropped -- six slabs holding one tint five times over | widest four-axis disagreement > 0.05 |
| `the_brighter_ground_is_on_the_brighter_axis` | the lobe **mirrored**, which is a sign error | 0.4252 against 0.1115 at the probe it picks |

**Each was checked against a build that breaks it, and the third is the one worth having.**
Flattening every lobe fails the second and third and **passes the first**; swapping the -X and
+X lobes fails only the third and **passes the other two** -- a world where every wall takes its
colour from the ground behind it, which no picture reads as wrong. The third's expectation is
recomputed by a **different route** on purpose -- a plain unweighted box mean of the surface
albedo in each half-plane, not the lobe-and-form-factor gather -- because a transliteration of
the gather would agree with a mirrored table as happily as with a correct one.

**The chunk is found with an assert rather than written down.** A hardcoded shoreline that
stopped being a shoreline would leave all three passing vacuously over uniform grass, which is
`lessons.md`'s *put the assert in the tool, not in the plan*.

### And the one claim that failed

**Batch 57 has an `implies` row saying `--probe-fill 1.0` already switches the ambient cube off,
and the twin of it for the bounce does not hold**: 0 pixels at `default`, `lod` and `canopy`, and
**2 at `terraces` at max delta 1, reproducibly**. The cube's tap multiplies `shade`, a scalar
factor at the end of `shade_hit`'s expression; the bounce's multiplies `floor_rgb`, an addend
*inside* the parenthesised sum -- and reassociating a sum is where a ULP goes. It is `hit_t`'s
invariant in a new place, it says nothing against the control (which is exact against a real
revert at 18 of 18), and the row was removed rather than relaxed.
[`errors.md`](errors.md) has the whole of it, including why no fix is wanted.

### What it cannot see, and what that leaves R2

**The same limit batch 57 has, and for the same reason it was worth taking.** The gather marches
`WorldGen::height`, so it cannot bounce off a tree, a cave wall, an overhang or an edit -- and in
exchange the field stays a **pure function of world position**, with no seam, no apron, no
residency dependency and no re-bake when a neighbour lands.

**So the apron is still unpaid, and this rung is the reason it did not have to be.** The roadmap
read R2 as the batch that buys the apron, because it assumed a bounce needs a voxel DDA. The
dominant term outdoors is sunlight off the ground, the ground is exactly what the height field
describes, and the sky level at an exposed column top is 15 by construction -- so the flood is
not needed either. What a voxel gather would add is bounce off canopy and cave wall, which is
where R3's second bounce and R4's emitters both point.

**And one thing is not carried that a form factor would carry.** The magnitude is the authored
floor's; only its colour and the ground's own brightness move it. A ceiling receives nothing but
bounce and a floor receives almost none, and this field says they receive the same -- that
elevation term **was** the next thing to add here. It was built on 2026-09-15, measured and
rejected: `floor_rgb` is 7% of the light on a lit surface and **ceilings are 3.6% of the best of
eight upward-looking cameras**, so redistributing that 7% correctly moves `default` by a mean of
2.87 code values. [`ADR 0001`](adr/0001-no-elevation-form-factor.md) is the whole record -- it was
reverted rather than committed, so nothing in git history shows it happened.

### Where it is most visible, which is not where the work is

**At night.** `floor_rgb` does not scale with `frame.daylight` -- it is `frame.ambient`, a
constant -- so at `--time 0.05` the floor is nearly the whole of the light and the bounce's
colour dominates: `night` moves **757,159 pixels of 921,600 at max delta 46**, the largest in
the set, against `cave`'s 27,415 at 1. **That is a property of the floor being daylight-
independent, which predates this batch and which this batch did not change.** Worth knowing
before reading the sweep as a statement about where the gather does its work: the gather does
identical work at every vantage.


---

## Coloured block light, which is what `BLOCK_TINT` was standing in for (batch 59)

**Roadmap R4, and it is the one rung of the main sequence whose cost lands on the frame.** R1
and R2 moved work to the bake and paid 0.02-0.04 ms and nothing for the read. This one cannot:
what colour the light at a cell is has to be *stored* somewhere and *read* per pixel, and there
is no off-frame place to put it. The numbers are at the bottom of this section and they are the
largest any rung of the sequence has cost.

### The constant it retires, and why this one could not survive as a calibration

`BLOCK_TINT = (1.0, 0.65, 0.32)` was **the colour of every emitter in the world** -- one
authored display colour, decoded, applied to whatever the block flood happened to hold. That is
exactly the shape of thing this ladder retires, and batch 55 found the sharper half of it while
pricing the probe tap: `block::def(..).light > 0` was true of **exactly one block**. The engine
had one emitter, it was white, and the constant was its colour.

**Batches 57 and 58 both kept the constant they retired, as the calibration of the derived
thing.** `face_shade`'s table is still what an *unoccluded* face looks like and the cube
multiplies into it; `floor_rgb` is still the floor's brightness and the bounce multiplies into
it. Both multipliers have identity 1, which is what makes both controls exact by construction.

**The same shape was available here and was rejected.** Keep `BLOCK_TINT`, derive a chroma
normalised so a white emitter returns `(1, 1, 1)`, multiply: glowstone stays bit-exact and the
control is free. What it costs is the **gamut**. Every channel of the product is capped by the
constant, so blue tops out at 0.32 of what red can reach and a blue lamp is three times dimmer
than a white one at the same level -- a cap that is a property of one colour nobody would author
twice. Luminance-renormalising the product removes the cap and puts `luma(BLOCK_TINT) /
luma(BLOCK_TINT * chroma)` in the inner loop, where the numerator is a constant fold and the
denominator is not; batch 58 lost two pixels at `terraces` to exactly that kind of asymmetry
([`errors.md`](errors.md)), and here it would be sitting inside the nine-cell gather.

So the constant moved into the block table as glowstone's own row, and the read is
`vec3(light_curve(r), light_curve(g), light_curve(b))` with no tint constant in it at all.

### What that cost glowstone, which is the batch's one look regression

`light_curve` is `l^2 (0.6 + 0.4 l)` over `l = level / 15`, so authoring a hue means authoring
the levels whose *curved* values carry it. Four bits reach:

| channel | target | nearest level | reached | error |
|---|---|---|---|---|
| red | 1.00 | 15 | 1.0000 | exact |
| green | 0.65 | 13 | 0.7111 | **+9.4%** |
| blue | 0.32 | 9 | 0.3024 | -5.5% |

**Green is the interesting one: 12 is 0.5888, which is 9.4% out the other way.** The target sits
almost exactly between two levels and there is no better answer at this width;
`tests/light.rs::glowstone_is_the_retired_block_tint` asserts that no level lands nearer, so the
row is a fit rather than a preference. 13 takes the tie because glowstone's own texture is
greener than the constant was -- `ALBEDO` normalises to (1.0, 0.794, 0.302) -- so the lamp and
its light agree slightly better than they did.

**What that costs, measured**: `edits` moves **313,302 pixels at max delta 3** and `glass`
**86,481 at max delta 2** against the parent build. Three code values, on the two vantages in the
fixture that hold a glowstone.

### Why the channel widened and the block table did not

The obvious cheaper design is a scalar level with a per-block tint applied at read time, and it
**cannot mix**. A cell between an amber lamp and an azure one holds one number and no memory of
which block put it there, so the wall between them takes one hue or the other. Three independent
floods hold both, and `tests/light.rs::two_lamps_of_different_colours_mix` asserts the midpoint
carries red from one and blue from the other. The `lamps` vantage exists to put that on a screen.

So the cell went from one byte to two -- `sky:4 | r:4 | g:4 | b:4` -- and a light brick from 16
words to 32. [`light.rs`](../src/light.rs) calls `flood` three times on three arrays rather than
teaching it about colour, which is what keeps `max(r, g, b)` equal to the level the single pre-59
flood held at every cell it reached. That identity is the whole of why `--no-light-rgb` is exact.

### The neighbourhood gate, which is the difference between shipping this and not

Three channels means **twenty-seven `light_curve` evaluations in the nine-cell gather where the
pre-59 build had nine**, and the first build of this batch paid that at every shaded pixel in the
world: **+0.835 ms of a 2.655 ms `resolve` at `cave`**, a vantage holding no emitter at all,
where all twenty-seven return zero.

**Skipping them is exact and not an approximation.** With all nine cells' block nibbles zero,
every corner's `bkv` is exactly zero, `mix` at weight zero returns `SKY_TINT` to the bit and
`max(sk, 0.0)` is `sk` -- the whole block term reduces to `SKY_TINT * sk`. No constant is
authored and no pixel moves.

**And it is tested on the nine cells rather than on the chunk, which is what keeps it seamless.**
The flood is chunk-local but this neighbourhood is not: at a chunk boundary three of the nine
cells come from next door through `world_sample`, so a chunk-level "has an emitter" bit would
drop a lamp standing just across the seam. Batch 51's rejected gate is the other half of that
lesson from the other side -- it was exact and never fired, because a chunk 64 blocks tall holds
the lit surface above the cave.

### The array that was not worth caching, which is this batch's second finding

The natural way to write the gathered curves is the way the sky channel is written: nine values
into an `array<vec3<f32>, 9>`, read four at a time by the corner loop. **That version is slower**,
and `shaderstats` says why in one row: `resolve`'s shared memory goes **15,872 to 16,896 bytes**,
which is what decides how many workgroups an SM holds at once.

Taking the array out and evaluating the twelve corner curves inline -- **thirteen evaluations
instead of nine** -- puts shared memory back at 15,872 and recovers **0.147 ms at `cave`**. So on
this pass occupancy is worth more than four `light_curve` calls.

**Batch 47 found the same shape from the other end**: pointing all nine gather reads at one cell,
with perfect sharing as far as the memory system is concerned, recovered a tenth of what removing
them did. This block is instruction-bound and residency-bound, not memory-bound, and trading
arithmetic for residency is the trade that pays here.

### What it bought, which is the only thing that justifies the above

**The user judged the `lamps` pair and judged it a clear win** -- *"i love the lamps ... it looks a lot better as in quite a bit"* -- and that is recorded here for the reason roadmap A2 records the complaint that started the water work: **this is the only ranking a look change gets.** The batch spends +0.32 ms of the one saturated resource in the machine, and a document that carries the cost and not the verdict is telling the next session half of a trade.

**Scoped to `lamps` deliberately.** It is the only vantage where the feature does anything visible; `edits` and `glass` move by max delta 3 and 2 in daylight and neither the user nor this session could see a difference in either pair. Reproduce it with `--demo-lamps --time 0.30 --cam-height -20 --cam-yaw 0 --cam-pitch 0`, with and without `--no-light-rgb`; the frames are not tracked in `screenshots/` because one binary and a flag regenerate them, which is the standing rule there.

**One thing the pair says that the numbers do not**: the shipping frame is *dimmer* than the control, because a saturated lamp emits less than a white one at the same level, and it was preferred anyway. So the win is the colour and not the exposure -- which is the argument against raising the lamps' weak channels to brighten them, since that is the same edit as desaturating them.

### What it costs, and the part of it that has no mechanism

Paired against `voxelcraft-pre59.exe`, eight rounds, `resolve` with hi-z on:

| vantage | shipping vs parent | the control vs parent |
|---|---|---|
| `cave` | **+0.321 ms** +/- 0.032 | **+0.110** |
| `default` | **+0.340** +/- 0.049 | -- |
| `sky` | **+0.150** +/- 0.000 | -- |

The split is the point. **+0.110 is the widened cell**, paid by both arms -- the control reads a
two-byte cell out of a 32-word brick whichever way `SPEC_LIGHT_RGB` folds, and no pipeline
override can shrink a brick. That is why `--no-light-rgb` is the first control in this project
whose *picture* reverts exactly and whose *time* cannot.

**The remaining +0.202 has no mechanism and is at the front of the roadmap queue.** Measured
directly as well as by subtraction -- one binary, both arms, `cave`, **-0.202 +/- 0.014, t =
-14.48** -- and at that vantage every block channel is zero, the gate skips all twenty-seven
evaluations, and the shipping arm therefore does *strictly less* work than the control arm it is
losing to. `resolve` is **586,624 bytes against the control's 588,928**, at 96 registers and
15,872 bytes of shared memory either way, so it is not size, not occupancy and not registers.

**The spread does not order by surface density, and an earlier draft of this section said it
did.** `cave` is 0.321 +/- 0.032 and `default` is 0.340 +/- 0.049 -- indistinguishable from
each other against a combined 0.059 -- where batch 56's saving had `cave` clearly ahead of
`default`, which is the ordering that let that batch argue its number was *work* and not
*code*. **That inference is not available here.** What the three readings support is weaker
and should be stated as such: two frames full of shaded surfaces cost about the same, and a
frame with almost none costs about half. A term that were purely code size would not halve
at `sky`, so it is not *only* code -- and that is the whole of what can be claimed.

What none of it explains is how the arm that executes fewer instructions is the slower one.

## The light that reaches the ground, and the floor that stops being flat (batch 60)

**Roadmap R3's first rung, in two halves that ship together because neither is right alone.**
One is a transport term in the bake and one is an energy term in `resolve`, and the batch is
only defensible as a pair: the first without the second is invisible, and the second without
the first is wrong.

### The transport half: a valley floor is not an open field

Batch 58 gathers the albedo of the ground a probe can see, weights it by a form factor, and
**never asks whether that ground is itself lit**. Two patches of the same grass -- one at the
bottom of a ravine, one in the open -- bounce exactly the same amount into the field. That is
a first bounce pretending to be a complete one, and the missing factor is the whole of what
makes a second bounce a second bounce: light has to *reach* the ground before the ground can
send any of it on.

`probe::ray_exposure` supplies it, **and takes no new height samples doing it**. The horizon
march already walks `STEPS` columns outward along each of `AZIMUTHS` azimuths and already knows
every one of their heights; what it did not do was keep them. The step loop is split in two so
the whole profile of a ray exists before any step on it is weighted, and the sky a sample at
step `si` can see is then a **maximum of slopes** over that profile rather than a second march.
The chunk's 81,920 height samples stay 81,920; what the batch adds is arithmetic over an array
already in cache, and the bake goes **2743 to 3414 us, +671 and 1.24x**.

The per-direction factor is `1 / (1 + t^2)`, **quoted from `cube_from_horizon`'s up axis rather
than re-derived**: `sin^2(atan(t))` is `t^2/(1+t^2)`, so a cosine-weighted hemisphere cut off at
elevation `atan(t)` passes `1/(1+t^2)` of an open one. Outward and inward are kept apart and
averaged rather than maxed together, because two ridges on opposite sides cut two different
parts of the hemisphere and one maximum would charge the taller for both.

**It sees one azimuth and its reverse, and that is a real limitation stated rather than hidden.**
A ravine running *along* a ray reads as open, because the ray carries no data about the
directions perpendicular to itself. The estimate is therefore a lower bound on occlusion and an
upper bound on exposure -- biased toward batch 58's answer, which is the one it has to reduce
to. **Flat ground gives `t = 0` both ways and therefore exactly 1.0**, which is what makes the
whole half a pure revert over open country rather than a close one.

### The energy half, and the ceiling it exists to clear

**On its own the transport half is invisible, and that is measured rather than feared.** It
moves 644,121 pixels at `default` at a max channel delta of 6, for an **MAE of 0.372**.
[`ADR 0001`](adr/0001-no-elevation-form-factor.md) had already rejected the elevation form
factor at **2.87** at the same vantage, by the user looking at the pair. So the first half lands
**eight times under a term already rejected as invisible**, and it lands there for the ADR's own
reason: `Config::ambient` is 0.08, so `floor_rgb` is about **7% of the light on a lit surface**,
and no redistribution of it can be worth more than that.

That ADR names exactly one way out -- *a build where the ambient floor carries materially more
energy* -- and the second half is it. Beside the authored floor, not replacing it:

```wgsl
floor_rgb = vec3<f32>(0.85, 0.90, 1.00) * frame.ambient;   // unchanged, still the cave's light
if SPEC_PROBE_BOUNCE { floor_rgb = floor_rgb * probe.rgb; }
if SPEC_PROBE_SUN {
    floor_rgb = floor_rgb + vec3<f32>(0.85, 0.90, 1.00) * probe.rgb * (probe_sun_gain() * sky_sm);
}
```

**Added rather than folded in**, so the authored constant goes on being the sole light in an
unlit cave and the two lines stay a pair rather than a replacement. With it the batch reads
**MAE 2.2248 at `default`** and **4.8510 at `terraces`** -- past the ceiling, and past the
number the ADR was rejected at.

### The gate, which is this batch's one real defect

**The first build gated on `probe.a * frame.daylight` and was wrong in a way worth keeping**,
because the mistake is the field's central design decision read backwards.

The reasoning was: a cave sees no sky, so `probe.a` will be near zero there. But the field
**stores occlusion normalised to 1 in an open field**, precisely so that every absence of it is
exact -- an unbaked texel, a coarse LOD, a pinned `--probe-fill 1.0` and a probe the bake has no
opinion about all return **1.0**. And 1.0 is *unoccluded*, not *dark*. The bake reads
`WorldGen::height` and never the voxels, so it has no opinion about anything underground at all.
The result was **921,600 pixels at `cave` and 921,600 at `lamps`**: full sunlight bouncing around
a sealed chamber, on a build whose every other number looked right.

`sky_sm` is the correct quantity and **was already in scope two lines above**, gating `direct`
for the same reason. It is the flood's own per-voxel answer to how much sky reaches this
surface, it is zero in a cave and in a sealed room, and it already carries `frame.daylight` --
so the corrected term needs *neither* of the two factors the draft multiplied in, and goes to
zero at night through the same path the sun does. With it, `cave` is **216 pixels at max delta
1** and `lamps` **404 at 1**, which is batch 57's and 58's own signature.

**The independent confirmation is better than the fix.** `cave` reads **0 pixels between the two
gains** -- so no value of the gain can leak underground, which the `sky_sm` gate proves without
reference to the bug that motivated it.

### `PROBE_SUN_GAIN`, the one authored number, and how it was chosen

Everything else in the term is derived: the ground's albedo and the sky it sees come from the
bake, the sun's strength and the receiver's connection to it from `sky_sm`. What no measurement
in this tree fixes is **the strength of one bounce relative to the direct term**, because nothing
here has ever computed one. It is **not** an energy-conserving derivation and must not be
described as one.

**0.10 ships, and it was chosen by the user playing all three arms.** `--probe-sun-high` selects
0.25 and exists so that could happen: `default` MAE 5.7074 at max delta 59 against 2.2248 at 27.
The user's verdict was the quieter one, with the standing note that the world already reads dark
-- so the ladder is kept in the build rather than deleted, and the choice is recorded rather
than re-derivable.

**It is two constants selected by an override rather than a uniform**, which is
`SPEC_LEAF_FILL`'s shape and is chosen for `GpuFrame`'s sake: that struct has no pads left,
appending to it costs four words and seven `offset_of!` asserts, and a value with two settings
does not need one. `FLAG_PROBE_SUN_HIGH` is therefore the **only bit in `SPEC_MASK` whose
default is off** -- it is a sweep rung and not a control, because it reproduces no earlier build.

### What it closed

**The ambient floor is finished as a place to spend effort**, and both directions of evidence
point at it: the ceiling measured twice, and the user choosing the quieter of two gains and
closing the line. Roadmap R3's remaining shape has to touch the direct term or the nine-cell
gather, which is why R5 is next rather than a second bounce. **Iterating a quantity worth 7%
converges to something worth 7%.**

## The sky's own colour, which is what `SKY_TINT` was standing in for (batch 63)

**Roadmap R7, and the sixth rung of the main sequence to replace a constant with something
derived.** `SKY_TINT` was `vec3(0.6038, 0.7084, 1.0000)` multiplied into `amb` on **every shaded
pixel in the world**, scaled only by `frame.daylight`. Nothing coupled it to the sky the renderer
actually draws: `amb` never read `sky_base` or `sky_color`, which appear only in water's
reflections and in the sky path itself. The sky model and the ambient standing in for it sat in
two files and did not speak.

It is the cheapest rung the sequence has taken and it touches more pixels than any of them.

### What it is

`sky_ambient_tint(n)` integrates the gradient `sky_base` already evaluates over the hemisphere a
face can see, and returns the result at `SKY_TINT`'s own luminance. Three things make it small:

- **The gradient is two lerps of four constants against `frame.daylight`**, which `resolve`
  already has. No uniform, no CPU integral, no `GpuFrame` seam -- an earlier draft of R7 budgeted
  all three. There is also no Rust twin to maintain: `sky_base` is WGSL only, `scene::sun`
  computes a direction and a daylight and no colour at all, so a CPU-side integral would have
  meant *writing* the second copy this tree spends so much effort avoiding.
- **The hemisphere integral has a closed form in the one case that matters.** The gradient is
  `mix(horizon, zenith, pow(up, SKY_GRAD_EXP))`, which is linear in its two endpoints, so its
  cosine-weighted mean is `mix(horizon, zenith, W)` where `W` is the mean of the *parameter*
  alone. `W` depends only on `n.y`.
- **And `n.y` is exactly +1, 0 or -1 for every normal state this renderer has.** A cube face's
  normal is axis aligned and `normal_of` returns `(0.707, 0, +/-0.707)` for both cross-quad
  planes. So two constants cover every face in the world and a `select` is exact where a fitted
  curve would not be.

| face | `W` | where it comes from |
|---|---|---|
| up | **0.78431373** | closed form: `2 / (2 + SKY_GRAD_EXP)`, the whole hemisphere being sky |
| side | **0.58578900** | quadrature: 0.58584712 at 300 polar steps, 0.58580391 at 600, 0.58578915 at 1200 |
| down | **0** | no sky in the hemisphere at all, so the horizon is the stand-in |

**The up case is what says the quadrature is right**, and `tests/sky_tint.rs` asserts it against
the closed form rather than against a remembered number -- the side weight has no such form, and
the only reason to believe it is that the same routine reproduces the one that does.

### The trap, and the two identities that disarm it

**The real sky is far darker than the constant standing in for it.** Day zenith is
`(0.0637, 0.2140, 0.8276)` against `SKY_TINT`'s `(0.6038, 0.7084, 1.0000)`. Substituting radiance
naively would make every shadow in the world darker and bluer -- straight into the user's
standing judgement that the world already reads too dark, and into batch 57's own retreat, where
a before/after pair could not separate a global exposure shift from a feature arriving.

So what ships is a **departure from** `SKY_TINT` rather than a replacement of it, and it carries
two identities that are exact rather than close:

- At the reference condition -- a face pointing straight up, full daylight, no halo, which
  `--fog-scatter 0` reaches -- the function returns `SKY_TINT` **to the bit**, *and it does so at
  every rung of the strength ladder below*, because `pow(1, anything)` is 1. That is what makes
  `SKY_TINT` the *calibration* of a derived hue rather than the hue, exactly as `face_shade`
  became at batch 57 and `floor_rgb` at batch 58.
- Everywhere else, `luma3` of the result is `luma3(SKY_TINT)` **by construction**. Only the hue
  moves. No exposure anywhere in the world changes, so the before/after pair ranks a colour --
  and again this holds at every rung, because the renormalisation is applied last.

`SKY_AMBIENT_REF` and `SKY_TINT_LUMA` are results of expressions WGSL cannot fold -- there is no
const `mix` and no const `pow` -- so they ship as literals, and `tests/sky_tint.rs` recomputes
both from the endpoints beside them. **A literal that had drifted would render a plausible frame
at a slightly wrong hue and the control would still be bit-exact**, because the control reverts
the whole function rather than the constant inside it. That is the one failure mode here that no
picture and no sweep could catch.

### The physical answer is nearly invisible, and that is the batch's most useful finding

**Built at its physical strength, this rung is very close to nothing to look at.** The `default`
before/after pair is **MAE 1.2185 at max delta 24** -- against the **2.87** at which
[`ADR 0001`](adr/0001-no-elevation-form-factor.md) rejected a change *by eye*, and three times
batch 60's 0.372, which that batch itself called invisible.

**It was the user who found this, with an image slider, and the fixture then confirmed it.** The
first pair shipped to them came back *"0 difference in the pictures"* -- which was very nearly
right, and no number then on file said otherwise, because every number the batch had measured was
a pixel count. **759,140 pixels differing and MAE 1.22 are the same frame described two ways, and
only one of them says whether anybody can see it.** A count answers *did the feature fire*; it
cannot answer *is the feature worth having*, and this batch had been reading the first as the
second for its whole length.

Two causes compound, and both are design decisions rather than defects:

- **The luminance renormalisation deliberately removes the brightness difference**, which is the
  larger half of what separates a real sky from this constant. That was the point -- it is what
  disarms the trap above -- but what it leaves is only a rotation.
- **A hemisphere-averaged sky is far less saturated than the zenith** people picture when they
  say the sky is blue. Then `amb * 0.50` halves it again and a direct sun term stands beside it.

So `SKY_TINT_SAT` is an exponent on the per-channel ratio: 1.0 is the physical answer, 0 is
`SKY_TINT` exactly, and above 1.0 exaggerates the same hue rather than inventing a different one.
**It is the one authored number in this batch**, and it is swept by eye and shipped at one value,
which is `LEAF_FILL`'s shape at batch 43 and for `LEAF_FILL`'s reason: no metric in this tree can
rank a look, so the user is the instrument.

| `SKY_TINT_SAT` | `default` MAE | `canopy` MAE | reference points |
|---|---|---|---|
| 1.0 -- physical | 1.2185 | 0.6670 | under ADR 0001's 2.87 rejected-by-eye line |
| **2.5 -- ships** | **3.0379** | **1.6791** | beside `--probe-sun`'s shipping 2.2248 at `default` |
| 4.0 | 4.8419 | 2.6975 | approaching `--probe-sun-high`'s 5.7074, which the user rejected |

**The user played the ladder and chose 2.5**, the same restraint they showed choosing batch 60's
quieter gain. The ladder is kept here rather than deleted so the choice is recorded rather than
re-derivable.

### The sunset half, and the entry's own headline was wrong about it

R7's entry had long said *"at sunset the world's shadows stay blue under an orange sky"*, and the
second half was only half true. At low sun `frame.daylight` falls, so **both** gradient endpoints
lerp toward the *night* constants -- dark blue, not orange. The warm colour of a sunset lives
entirely in `scatter_lobe`, a directional term around the sun that until this batch was read by
the sky itself, the haze and the cloud deck, and by nothing else.

So the halo is added to the radiance before the ratio is taken:

```wgsl
c = c + SCATTER_TINT * (frame.fog_scatter * day * SKY_HALO_ENERGY * max(dot(n, frame.sun_dir), 0.0));
```

**`SKY_HALO_ENERGY` is 4 and is derived, not authored.** `hg_gain` is normalised so isotropic
scattering reads 1.0, so the lobe integrates over the sphere to `4 * pi`; a lobe that narrow,
averaged over a cosine-weighted hemisphere, is that energy times the cosine weight at the sun,
and the `1 / pi` of the average leaves 4.

Sampling `hg_gain` along a single representative direction was the alternative and is worse: the
lobe is about 30 degrees wide at the shipping `--fog-g 0.80` and the hemisphere is not, so one
sample reads the wrong side of a cliff whenever the sun sits off that direction but still in
view -- which at a sunset is most of the frame. The cosine falls off smoothly and has no edge.

**Both terms carry `frame.daylight`, so their ratio does not**, which is what keeps a low sun's
warmth from fading out along with the light that causes it.

### What the halo half is worth, which is less than advertised and not where expected

**`--fog-scatter 0` zeroes the halo and leaves the gradient standing**, so the two halves rank
apart through an existing control and without spending a second flag bit -- which matters,
because `FLAG_SKY_TINT` is bit 31 and `frame.flags` has no more. At the shipping strength:

| vantage | both halves | gradient alone | the halo's share |
|---|---|---|---|
| `terraces` | 910,104 @ 20 | 579,125 @ 20 | **+330,979** |
| `default` | 759,958 @ 37 | 630,439 @ 32 | +129,519, and the max delta 32 -> 37 |
| `coastline` (0.74) | 200,705 @ 21 | 122,181 @ 20 | +78,524 |
| `low-sun` (0.26) | 589,337 @ 32 | 529,672 @ 30 | +59,665 |
| `canopy` | 412,430 @ 42 | 355,460 @ 42 | +56,970 |

**The halo is largest at midday and smallest at the two low-sun cameras, which is the opposite of
what the entry promised.** The mechanism is not subtle once seen: the sky ambient is
`sk * frame.daylight`, so at a sunset there is very little ambient light left to re-colour, and a
hue applied to nearly nothing moves nearly nothing. The warmth is real and it is dim, and it is
dim for the same reason the sunset is.

**The general form is worth more than the number**, and it is the same one the section above
found from the other side: a term that re-colours light is bounded by how much of that light
there is. That is batch 60's *ask what share of the frame's light a rung can reach* arriving
twice in one batch -- once as a ceiling on the sunset, once as a ceiling on the whole feature.

### What it moves and what it costs

**15 of 19 vantages move**, max delta 14 to 42, `canopy` highest and `terraces` widest at 910,104
pixels of 921,600; `default` 759,958, `low-sun` 589,337, `edits` 895,445, `glass` 889,047.
**The four that do not are the four with no sky in them** -- `cave`, `lamps`, `night` and
`deep-water` -- and each has a mechanism rather than a coincidence: `cave` and `lamps` are sealed
so the flood's `sk` is zero, `night` has `frame.daylight` at 0.05 so the sky ambient is near zero
before any hue is applied, and `deep-water` sits 24 blocks down where `WATER_DARK_DEPTH` has
already taken the sky level to zero. **R7's entry named `cave` as the one vantage that must not
move**, and a non-zero row there would have been batch 60's own failure repeating.

**19 of 19 bit-exact with `--no-sky-tint`, against a real `voxelcraft-pre63.exe`** and not
through the flag -- checked at both the physical strength and the shipping one.

**Frame cost is at most a few hundredths of a millisecond, and the honest reading is a range
rather than a number**, because the two sweeps that bracket it were taken an hour apart on a
machine that heat-soaked in between:

| vantage | cool machine, `SKY_TINT_SAT` 1.0 | warm machine, shipping 2.5 |
|---|---|---|
| `cave` | **-0.045 +/- 0.014**, t = -3.21, a real difference | -0.024 +/- 0.017, t = -1.41, inside its error |
| `default` | -0.032 +/- 0.019 | +0.007 +/- 0.020 |
| `sky` | -0.008 +/- 0.016 | +0.015 +/- 0.017 |

**The same vantage's absolute `resolve` moved 2.804 to 2.839 at `cave` and 6.140 to 6.520 at
`default` between the two**, which is `PERF.md`'s power-state caveat and not the shader. What
survives both readings is the sign at `cave` and the ordering: **the cost tracks surface
density**, largest where there is most shaded surface and indistinguishable from zero at `sky`,
which is batch 56's own discriminator for work against code. Call it **0.02 to 0.045 ms of
`resolve` at `cave`**, where **0 pixels move** -- `sky_ambient_tint` is evaluated per shaded pixel
and then multiplied by an `sk` of zero.

`resolve` is **587,264 to 592,512 bytes, +5,248**, at **96 registers either way**.

**One number moved that nothing in this batch aimed at, and it is recorded rather than explained:
`resolve`'s shared memory fell from 15,872 bytes to 14,848, -1,024.** Shared memory decides how
many workgroups an SM can hold at once -- batch 59's own comment turns on that -- so this is a
change in the right direction, but no mechanism is claimed for it. The plausible story is that
hoisting one `vec3` out of the per-corner loop changed how the compiler laid the corner arrays
out, and a plausible story is not a measurement.

## The specular term (batch 65, roadmap R6)

**Everything that is not water or glass was perfectly Lambertian from batch 1 until this batch.**
Water and glass have traced real reflections since batches 8 and 38b; stone, soil, snow and bark
had none at all, which was the largest single gap left between this renderer and a path-traced
one once the ambient rungs had landed.

```wgsl
spec_rgb = sky_base(reflect(rd, n), 1.0) * (schlick_ground(dot(n, -rd)) * sky_sm * SKY_SPEC_GAIN);
return albedo * ((ambient_rgb + sun_rgb) * ao + floor_rgb) * shade + spec_rgb;
```

**It is added outside `albedo`, and that one fact is why R6 was worth opening when R3 and R7
were not.** A specular reflection off a dielectric is not tinted by the diffuse albedo and is not
a redistribution of the ambient -- it is new energy on top of the diffuse line. R3's rung
modulated the ambient floor, which `Config::ambient` pins at about 7% of a lit surface; R7's
carried `SKY_TINT`'s luminance by construction, so only hue could ever move. **Both found their
ceiling only after being built.** This one is bounded by Fresnel instead, and Fresnel is not a
share of anything: `GROUND_F0`'s 4% at normal incidence, **20% at cos 0.3, 61% at cos 0.1**.

### The roughness cap, which is the batch's transferable finding

**The first build had no roughness term and was wrong**, at MAE 15.65 and max delta 202 -- a
blue-white haze over the whole frame rather than a sheen. Schlick's curve reaches **1.0 at
grazing for every F0**, and a hillside seen from above is at grazing incidence over most of its
extent, so the frame reflected 60% of the sky nearly everywhere.

**What a bare Fresnel is missing is the geometric shadowing-masking term.** On a rough surface
the microfacets that would mirror a grazing ray are occluded by their own neighbours -- at
exactly the angle Schlick says reflectance is highest -- so the two effects oppose each other and
a model carrying the first without the second blows out. A full microfacet BRDF is not wanted
here; `GROUND_ROUGHNESS` caps how far the curve may climb, at `1 - roughness` = **0.10** rather
than 1.0, leaving the honest 4% at normal incidence untouched.

**Turning the gain down would not have fixed it, and the ladder is the proof.** Across roughness
0.80, 0.90, 0.95 and 0.97 -- grazing ceilings of 0.20 down to 0.03 -- MAE moved only **11.50,
10.81, 10.43, 10.35**. The grazing term was never the bulk of the error; the base 4% times a
bright sky was. So the fault was the curve's *shape*, and a scale knob cannot fix a shape.
`SKY_SPEC_GAIN` is the scale knob and is a separate axis: 0.25, 0.50 and 1.00 read **3.20, 5.98
and 10.81**. **0.5 shipped, chosen by the user from images**, with 1.0 the unscaled physical
answer -- `LEAF_FILL`'s shape at batch 43 and `SKY_TINT_SAT`'s at 63.

### The gate, and why it is `sky_sm` for the third time

**Batch 60 is why it is not `probe.a`.** That batch's first draft gated its bounce on the baked
occlusion and lit a sealed chamber with 921,600 pixels, because the field stores occlusion
*normalised to 1 in the open*: an unbaked texel, a coarse LOD and anything underground all return
1.0, which is unoccluded and not dark. `sky_sm` is the flood's own per-voxel answer to how much
sky reaches this surface, it is exactly zero in a cave and in a sealed room, and it already
carries `frame.daylight`. **The four vantages reading 0 pixels -- `cave`, `night`, `lamps`,
`deep-water` -- are the same four batch 63 found**, which is two independent gates agreeing about
which cameras have no sky in them.

**`sky_base` and not `sky_color`**, per the invariant that one takes a ray origin and the other
must not: what is wanted is radiance in a direction, with no deck intersected and no origin to
intersect it from. It also already mixes toward `horizon * 0.26` as `rd.y` goes negative, so a
ceiling, an overhang and an underside reflect dark ground with no special case in this file.

### Two things it does not do, recorded rather than hidden

- **No roughness lookup.** The sky is sampled as a mirror. What makes that tolerable is that the
  sky is low-frequency -- the same approximation roadmap R6 said a low-order probe field would be
  making, reached without the field.
- **`sky_base`'s sun disc is unshadowed**, so a slope the shadow ray rejected can still catch a
  glint. The disc is narrow (`smoothstep(0.9975, 0.9990)`) so it is small, but it is wrong, and
  separating it out needs a fourth sky function rather than a condition.

`GROUND_F0` is one constant for the whole world because **nothing here is a metal** -- a metal's
F0 *is* its albedo and would have to be per block, which is batch 59's *content* kind of rung and
would widen a per-voxel store where every reader pays. This is the *field* kind: one constant, no
new attribute, no bit in the visibility key.

### What it cost, and the finding is that it was not the arithmetic

**+0.295 ms of `resolve` at `cave` -- where 0 pixels change -- +0.516 at `default`, +0.271 at
`sky`**, against a real `voxelcraft-pre65.exe` at t = -28.0, -10.6 and -14.6. `shaderstats` in
three builds settles the mechanism: `resolve` is **96 registers with the term removed and 128
with it**, and replacing `sky_base` with a constant still reads 128, so it is not the sky lookup.
Bytes went 592,512 to 592,256 -- **the code got smaller while occupancy fell**. That is why a
camera with no changed pixels pays nearly as much as one with 760,000, and it is roadmap P13.

**A `sky_sm > 0.0` early-out was built and rejected.** It saved nothing at `cave` (0.295 to
0.282, inside its own error) and made `default` and `sky` *worse* (0.516 to 0.594, 0.271 to
0.305). Roadmap P11's lesson exactly: ask what a gate costs before crediting it with what it
skips.

### What actually fixed it, one batch later: the term is on the primary hit only

**Batch 66.** `shade_hit` is inlined **four** times -- the primary surface, water's refracted and
reflected legs, glass's transmitted leg -- so the specular arm existed in four copies and the
register allocator budgeted for all of them. It takes a `do_spec` parameter now, `true` at the
primary call site and a literal `false` at the other three, which folds the arm away there and
takes `resolve` from **128 registers back to 96**. Batch 65's bill falls from +0.295 / +0.516 /
+0.271 ms to **+0.051 / +0.065 / +0.044**, an 83-87% recovery, for 1,664 bytes of extra code.

**`do_spec` is a separate parameter and not a reuse of `smooth_light`, deliberately.** Those three
sites already pass `secondary_smooth()`, which is `!SPEC_FLAT_SECONDARY` and folds identically
today -- so reusing it would work and would quietly make `--no-flat-secondary` a second control
for this feature. `tests/sky_specular.rs` and `tests/secondary.rs` hold the two halves.

**What it costs is the picture on transmissive surfaces, and batch 45 already had the rule.** A
floor seen through water or a pane no longer takes a sky sheen. That is the same argument batch 45
made for giving secondary hits one light lookup instead of the nine-cell gather: the three
transports destroy the contact cue, so the cheap model is right there and would not be on the
primary. Measured **MAE 0.8388** at `terraces` (max delta 16) and **0.4665** at `shore` (17), both
far under [`ADR 0001`](adr/0001-no-elevation-form-factor.md)'s 2.87 rejected-by-eye line.

**The generalisation is the part worth carrying**: ask how many times a function is inlined before
asking what a line in it costs. `PERF.md` has the register table that made this measurable.

### And batch 67 finished it: the term is *hoisted*, not just restricted

Cutting the copies from four to one left the remaining copy still costing **16 registers**. Batch
67 stubbed its three operations one at a time and **every simplification read 80** -- dropping
`sky_base`, calling it on `n` rather than the reflected direction, replacing the Schlick weight
with a constant. So it was none of them, and folding the weight into one scalar first changed
nothing, because the compiler already schedules that.

**What mattered was where the term is written.** Beside the return, `n` and `rd` have to stay live
across the nine-cell gather, the probe tap and the shadow march, and `sky_base`'s own demand lands
on that peak. The direction-dependent half now sits just after `n` is finalised, so both die where
they always died and a single `vec3` crosses the gather; `sky_sm` is not known that early and does
not need to be, since it is one scalar multiplied at the return.

That took `resolve` to **80 registers** once glass's own ceiling fell in the same batch, and it is
worth **+0.103 ms at `cave` and +0.399 at `coastline`**. **Ask where a value dies before asking
what an expression costs.** The one visible consequence is a float reassociation: `sky` moves **1
pixel at max delta 1**, reproduced 3 of 3, which is D1's signature and is not D1.

## Controls

| flag | reproduces | cost |
|---|---|---|
| `--ambient F` | the ambient floor, isolated | -- |
| `--probe-tap` | **not a control and the table's one inversion**: the feature ships *off* and this switches it *on*. One trilinear tap per shaded surface into a field of 1.0, so the frame is bit-exact at all 18 vantages -- see the section above | `SPEC_PROBE_TAP`; `PERF.md` has the cost |
| `--probe-ambient` | batch 56: the field **replaces** the ambient and the redundant terms leave the module. Implies `--probe-tap`. **The frame is wrong on purpose** -- a build to measure and discard | **saves 0.03-0.13 ms net**; see above |
| `--no-probe-cube` | batch 57: the directional factor comes from `face_shade`'s per-normal constants again rather than from the baked cube. **A pure revert at all 18 vantages, by construction** -- the field holds occlusion and an unbaked texel is 1.0 | free (`SPEC_PROBE_CUBE`) |
| `--no-probe-bounce` | batch 58: the ambient floor comes from one authored constant again rather than from the baked ground bounce. **A pure revert at all 18 vantages, by construction** -- the field holds a multiplier and an unbaked texel is 1.0. Independent of `--no-probe-cube` despite sharing one texel | free (`SPEC_PROBE_BOUNCE`) |
| `--probe-fill F` | pins the field to a constant and **switches the bake's uploads off**, which is what keeps batch 55's and 56's diagnostics measuring the field their numbers came from. **The paired positive claim**: at anything but 1.0 the frame has to move, and if it does not the tap was never wired up. `--probe-tap` and `--probe-ambient` each imply `--probe-fill 1.0` | -- |
| `--probe-noise` | fills the field per texel instead of uniformly, which rules out a driver compressing a constant fill into a cost no real field would pay | -- |
| `--no-light-rgb` | batch 59: block light is one 4-bit level wearing `BLOCK_TINT` again rather than three levels whose ratio is the colour. **Bit-exact at all 18 vantages that predate the batch and not a pure revert**, which is `--no-glass`'s shape: the lamps are a world change and glowstone's row is a 4-bit re-authoring, so with the control *off* `edits` and `glass` move by max delta 3 and 2 | **not free**: switching it on recovers 0.202 ms of `resolve` at `cave` and cannot recover the other 0.110, which is the widened cell both arms read (`SPEC_LIGHT_RGB`) |
| `--no-sky-tint` | batch 63: the ambient's sky colour is one authored constant again rather than the sky model's own, integrated per face. **A pure revert at 19 of 19 vantages against a real `voxelcraft-pre63.exe`**, by construction -- the cleared arm returns `SKY_TINT` itself rather than a formula that evaluates to it, and that holds at every rung of `SKY_TINT_SAT`. **15 of 19 move with it off**, max delta 14 to 42, and the four that do not are the four with no sky in them | **0.02 to 0.045 ms of `resolve` at `cave`**, where **0 pixels move** -- quoted as a range because the two readings bracketing it were taken an hour apart on a machine that heat-soaked between them, and they agree on sign and ordering but not magnitude. Nothing distinguishable at `default` or `sky`. The cost tracks surface density, so it is work and not code (`SPEC_SKY_TINT`) |
| `--no-sky-specular` | batch 65: every opaque surface is perfectly diffuse again. **A pure revert at 20 of 20 vantages against a real `voxelcraft-pre65.exe`** -- 0 pixels, identical hashes. 16 of 20 move with it on; the four that do not are `cave`, `night`, `lamps` and `deep-water`, each because `sky_sm` is exactly zero there. MAE **5.98** at `default` | **not free: +0.295 ms of `resolve` at `cave` where 0 pixels move, +0.516 at `default`, +0.271 at `sky`** -- an occupancy loss (96 to 128 registers), not arithmetic. Roadmap P13 |
| `--demo-lamps` | builds the chamber the `lamps` vantage is taken of, through the normal edit path. **The one demo structure no earlier binary can be handed**: its block ids did not exist before batch 59 and `block::def` clamps rather than refusing | -- |
| `--no-tint` | `shade_hit` returns atlas albedo before the biome field. Bit-exact at nine | free (`SPEC_TINT`) |
| `--no-glass` | a pane is an opaque cube again. **Not a pure revert** -- the flood still runs through it, worth 131,689 pixels at max delta 17 at the one vantage holding glass, and 0 anywhere else | free (`SPEC_GLASS`) |
| `--demo-glass` | builds the glasshouse the `glass` vantage is taken of, through the normal edit path | -- |
| `--tint-strength F` | how far the tint is taken | -- |
| `--jitter X[,Y]` | pins the sample offset; `--jitter 1,0` reproduces an unjittered capture translated one pixel, bit for bit | -- |

