
# Errors: what is broken, and what has already been tried and lost

**Read the section for the thing you are about to work on. That is the whole point of this
file.** It replaces `live-caveats.md` and absorbs the "what was not built" / "what this does
not fix" / "do not re-derive this" sections that were spread across twenty-one batch
documents, where nothing could find them.

Two kinds of entry, deliberately in one file because a session needs both before it plans:

- **DEFECT** -- known wrong, measured, priced, and deliberately not fixed. None is a
  regression.
- **REJECTED** -- tried, measured, and lost. Each carries the number it lost by and the
  condition under which it would be worth retrying. **If you are about to build one of these,
  stop.**

Operational traps that fire while you type a command are [`pitfalls.md`](pitfalls.md), which
is a different file on purpose. What shipped is [`ledger.md`](ledger.md); what is next is
[`roadmap.md`](roadmap.md).

---

## Water and waves

**REJECTED, batch 48 -- clamping `frame.far` itself when the camera is submerged.** This was the
roadmap's own prescription for P4's cheapest sub-lead, described there as "a uniform change, no
shader edit", on the premise that *absorption kills the image within tens of blocks*. Both halves
are wrong and the constant says so at its own definition site: `WATER_EXT` is
`(0.280, 0.055, 0.020)` per block and its comment reads **red is gone within a few blocks and blue
survives tens**. "Tens of blocks" is red. Blue still transmits **7.7% at 128 blocks**.

**The load-bearing error is not the number, it is that a ray's water path is not its length.**
`water_path` is analytic against the sea plane, so a ray leaves the medium at
`(sea_level - cam.y) / rd.y` and is unattenuated every block after that. At the fixture's own
submerged eye -- three blocks down, pitched -14 degrees, 70-degree field of view -- the frame
reaches **+21 degrees**, and a ray there is out of the water within **nine blocks**. A uniform
clamp deletes the world above the waterline for the top third of the frame, and it does it to the
half of the image that is *not* attenuated.

Built anyway as a **ceiling**, which is what it is good for: at 128 blocks it is **-1.371 ms** at
`underwater` against **-1.135** for the per-tile rule that shipped, and it moves one pixel at max
delta 8 where the shipped rule moves none. So the uniform version is 20% faster, worse-looking by
construction, and carries a second defect the shipped one does not -- it changes `depth_key`'s
quantisation scale, which the **previous** frame's Hi-Z was written in, at the instant the camera
crosses the surface. **Retry condition: none.** The per-tile test costs about ten ALU ops per tile
and `tile_select` reads 0.150 ms against 0.160 either way.

**DEFECT, batch 52 -- the submerged reach seam, bought deliberately for 1.930 ms.** Batch 48
declined a 128-block reach because the fixture could not see what it deleted; batch 52 built
`sea-horizon` -- a submerged eye along the horizontal at islands 264 blocks out -- and the user
took 128 against the picture. **What it costs is 67,621 pixels of 921,600 at max delta 23**, and
the shape matters more than the count: 62,589 of them are open water moving by **at most 2**, and
the damage is the other 5,032, a pale seam where a distant shoreline's *underwater* silhouette is
deleted and the brighter haze behind it shows through. The full ladder is in `PERF.md` and at
the constant's own definition; the three numbers that matter here are that **the content ends at
264 blocks**, that **128 buys -1.930 ms**, and that the two retreats below are cheap.

**This is the first distance cut in this engine bought at a visible price**, and it is worth
knowing why it was allowed: every previous one truncated an *empty* band -- batch 41's 1.672 ms
for 1,309 pixels at max delta 1 is the canonical row -- and this one is 51 times those pixels at
23 times the delta for a comparable saving. What carried it is the size against the frame: 18% of
a 10.6 ms `sea-horizon`.

**`screenshots/batch52_seam.png` is the seam at 3x against both older builds**, and it is tracked
for this entry's sake: every other number here can be re-measured from the tree, and this one
needs two binaries that no longer exist to see. `screenshots/batch52_sea_horizon.png` is the whole
frame the crops are cut from.

**Two things were measured and one was not.** Blue's extinction and the frame agree about where
the content ends -- `ln(255)/0.020` = 277 against a measured 264, 5% apart, so batch 48's authored
256 was 8 blocks under exact and *right*. What nobody ranked is **motion**: the seam sits at a
fixed distance from the camera, so it slides across the water as the player swims rather than
staying with the shoreline, and no still frame can price that. **Retry condition, and it is a
retreat rather than an advance: if the seam reads as a moving band in play, 224 costs almost
nothing (1,640 pixels at max delta 3) and 192 keeps a third of the money.**

**REJECTED, batch 44 -- every distance constant water's traced reflection has.** The frame map
put `terraces` at the top of the fixture and its decomposition put `--no-water-reflect` at the
top of `terraces` (**-4.178 ms +/- 0.034** of a 10.1 ms `resolve`), so the leg was worth taking
apart. It has three distance levers and **all three were built, measured and thrown away**:

| lever | what it buys | what it costs |
|---|---|---|
| the **reach**, `WATER_REFLECT_DIST` 768 -> 512 / 384 / 256 / 128 | -0.446 / -0.746 / -1.164 / -1.646 ms at `coastline` | 5,442 / 8,918 / 10,656 / 12,951 px at delta 86-144 -- and **the headland is gone by 512** |
| the **fade**, `WATER_REFLECT_NEAR/FAR` 512/1024 -> 256/384 | -0.553 at `coastline`, -0.870 at `open-sea`, **zero at `terraces`** | **68,661 px over the sixteen vantages at delta 146**, 13 of 16 moving |
| the reflected hit's **shadow budget**, `frame.shadow_dist` 220 -> 48 / 16 / 0 | **-0.026 / -0.044 / -0.019 ms, every one inside its own standard error** | 167 / 273 / 21,427 px over the whole set |

**The reach and the fade are real money paid exactly where the look is.** At `coastline` the
crest at the right edge is mirrored in the sea beneath it and the ray that finds it is longer
than 512 blocks, so *every* value below 768 deletes the most prominent reflection in the
fixture for under half a millisecond. This is the exact inverse of batch 41's band, which held
1,309 pixels at delta 1.

**The shadow budget is free to look at and buys nothing, and that is the entry worth keeping.**
A reflected hit marches 220 blocks where batch 41 cut a refracted hit to 16, and the asymmetry
looked like 1.68 ms lying on the floor. It is not there: cutting to 16 is **-0.044 +/- 0.029**
and *deleting the shadow outright* is **-0.019 +/- 0.056**. **The medium is why, for the third
time in three batches.** A refracted hit is under the sea, so its shadow ray sets off through
water -- transparent to it -- and grinds for hundreds of blocks. A reflected hit is a hillside
in air, so its shadow ray hits terrain at once or escapes at once, which is precisely what
batch 42 found for the *primary* march on land. Batch 41's saving was never "a secondary leg's
shadow ray is expensive"; it was water's transparency, and it does not transfer to the other
leg of the same function.

**Retry condition for all three: only with something that makes a reflected ray's near field or
its shading cheaper.** At `terraces` the whole reach below 512 is bit-exact and still buys only
-0.256 ms, so 79% of that leg's 4.178 is the traversal's first blocks and the `shade_hit` at
the end of it -- neither of which any of these constants reaches. And **batch 40's -1.476 ms
for "the reach" was the reach and the fade in one build**; `PERF.md` and [`water.md`](water.md)
carry the corrected pair.

**REJECTED -- perturbing the traced reflection ray.** Bending it by 2.6 degrees of normal tilt
cost **+0.22 ms at the default camera and +0.84 down a coastline**, four to five times the
whole wave field, and put salt-and-pepper along every reflected shoreline. It is visible at
`--wave-clamp 0.008` and ugly at `0.020`. **It cannot be filtered**: a traced reflection is a
hit or a miss, one bit per pixel, and there is no mip chain over a ray cast. The band limit has
nothing to say about it because the normal it perturbs is perfectly smooth and the
discontinuity is in what the ray *finds*. `--wave-clamp` ships at 0 and stays as the A/B
partner. Retry only with a reflection that can be pre-filtered -- a cone trace or a blurred
reflection buffer -- which is its own batch.

**REJECTED -- re-spreading the wave directions to kill the plaid.** Scored numerically before
it was ever put in a shader, which is why it cost minutes: the best of **4000 random direction
sets** moves peak slope-field autocorrelation from 0.839 only to **0.737**, and an evenly
spread set to 0.801. Four cosines are near-periodic whatever their orientation. The clustering
is real -- the shipped four were 11.4, 24.1, 112.9 and 118.5 degrees, two tight clusters 90
degrees apart -- and it is **not the cause**. Only the octave *count* moves it, which batch 25
then shipped (eight octaves, 0.9988 to 0.7314).

**REJECTED -- correcting the water's mip footprint for the grazing angle.** `shade_hit` divides
its footprint by the grazing cosine and `shade_water` does not; that asymmetry is real and
fixing it scores **monotonically worse at every strength** (MAE 1.2161 to 1.4907, detail 1.01x
of truth down to 0.66x), saturating at 0.35 because `lod` clamps to 3. Do not re-derive it; the
table is in the batch 21 record.

**REJECTED -- filtering the water lattice at all.** A 256-sample, mip-0, correctly
area-averaged ground truth **still has the weave at full strength**. Supersampling converges
*to* it, so it is content and no footprint correction, mip bias or band limit can remove it
without removing signal. An analytic argument that the layer is "pure high-frequency noise
whose correct area-average is its mean" was persuasive and wrong: most of the variation at that
vantage is the refracted sea floor and the reflected deck, both real, and `shade_hit` already
carries the grazing correction for the floor.

**REJECTED -- a hashed per-block uv *offset* to break texture repetition.** What survives of
that idea is per-block **rotation and flip**, which stays inside 0..1, and it must be opt-in per
texture: rotating isotropic noise is free, rotating `LOG_SIDE`'s grain or a cross-quad's blade is
a new defect. That is batch 36, and `block::PERM_CLASS` is the opt-in.

**The reason recorded here for three batches was the wrong one, and it mattered.** It said the
offset cannot wrap because the sampler is `ClampToEdge` -- but an address mode applies *within*
an array layer, each atlas layer is an independent 16x16 image, and `AddressMode::Repeat` would
wrap correctly. `face_uv` already clamps `local` to [0, 1], so today the two modes are
behaviour-identical and the switch would move no pixel. **The sampler was never the constraint;
it was a setting.** The argument that actually kills the offset is about *content*: shifting a
bounded tile moves the feature out of the place the block boundary put it, so `GRASS_SIDE`'s
fringe lands across the middle of a face and stone's mortar line splits between two blocks. That
argument holds at any address mode, which is why the conclusion survived its reason being wrong.
Batch 36's research is the standing record -- see [`research-review.md`](research-review.md).

**REJECTED -- shortening the traced reflection's reach to buy time.** The other half of what
batch 41 had to choose between, and it lost by two orders of magnitude on the only axis that
separated them. `WATER_REFLECT_DIST` 768 -> 384 with the fade moved to 256-384 is **-1.476 ms
+/- 0.061** of a coastal `resolve` (batch 40) and moves **22,183 pixels of 921,600 at max
channel delta 109** at `lattice`, 18,214 at 146 at `coastline`, 13,947 at 162 at `open-sea`.
Halving the truncation barely helps: 768 -> 512, fade 384-512, still moves 20,332 pixels at 109.
Against that, `WATER_SHADOW_DIST` 48 -> 16 bought **more** time -- 1.707 ms -- for **1,309
pixels at max delta 1**, so the reflection reach is 17x the pixels at 100x the amplitude for
less of the saving.

**The amplitude is the finding and the pixel count is not.** A truncated shadow ray hands its
interval to batch 37's light envelope and the sea floor stays shadowed to within a code value;
a truncated reflection hands its interval to nothing, so a reflected shoreline is replaced by
sky. Those are a shading residual and a missing feature, and no ranking that reads only "how
much of the frame moved" can tell them apart -- which is the same thing
[`harness.md`](harness.md) says about `pixels` against `coherence`. Retry only if something
other than time wants the reach shortened, or if a pre-filtered reflection ever exists to make
the far band cheap rather than absent.

**REJECTED -- keying shallow-water damping to camera distance instead of depth.** Far cheaper
and the wrong feature: it would calm as you approached and build as you retreated, `taa` would
fight it the whole way, and no still capture could show it. Depth is a property of the world,
so a patch of sea looks the same from everywhere and a still A/B is a valid test.

**DEFECT -- distant water carries excess speckle that no band limit reaches.** Batch 19 took it
from 1.62x a converged reference to **1.43x** and **no value of `WAVE_FILTER_K` reaches
1.00x**; pushing harder trades MAE for a shrinking speckle gain. Split by four renders, a third
of the residual is the cloud deck sampled through the perturbed normal and two thirds is
Fresnel and the glint. All of it is one gap: **`wave_octave` spends the variance it removes in
the glint lobe alone**, and the Fresnel weight, `sky_color(mirror)` and the traced reflection
all read a normal that has been flattened without being widened. `gloss.w.z` already carries
the removed variance and three readers ignore it.
**The 1.43x is stale twice over** and re-measuring it is the first thing any batch here owes:
it was taken on a still frame (a moving sea is 0.84x as speckly), and batch 25 dropped raw
`speckle` at `open-sea/water` by a further 22-24%.

**DEFECT -- reflected and refracted content ghosts under fast motion.** `taa` reprojects the
water *surface*, which is the right motion vector for the surface and the wrong one for what is
seen in it. Every screen-space reflection has this. The fix is a per-pixel second motion vector
and **there is nowhere in the visibility key to put one** -- see the key layout in `CLAUDE.md`.
**REJECTED, batch 54 -- tracing the mirror a submerged eye sees on the underside of the water
surface.** The obvious build for Snell's window: cast a ray down into the water from the crossing
point, shade what it finds, absorb it back. It works, it is what the physics says, and it is
**invisible**. Against a mirror that is simply `underwater_body()` it differs by a max channel
delta of **3 at a level camera, 3 pitched up 20 degrees and 7 at 45** -- and it cost **4 ms of
`resolve` at `deep-water`**, taking the pass from 3.28 to 9.77 and the whole feature from
+0.262 ms to **+6.49**, more than three times what batch 53 had bought at that vantage.

**The reason is the reason it should not have been built, and it is a general one.** A mirrored
ray at these angles travels a long way through water, and `water_medium` converges it to
`underwater_body()` on the way back: blue's transmittance over 275 blocks is 0.004. **The
expensive answer and the free one agree because the free one is the limit the expensive one was
walking towards.** That is batch 51's lesson -- *before pricing a way to make an expensive block
cheaper, check whether its result is discarded* -- in its other form: check whether something
already computes what it converges to.

**The cost split, from three diagnostic builds at `deep-water`**: the wave normal **1.12 ms**,
the trace **2.50**, shading the hit **2.87**. A distance fade on the crossing (the refracted
leg's own `WATER_REFRACT_NEAR`/`FAR` ramp, applied to `t_surf`) recovered only **1.34 ms** of it,
because at a level camera the band that pays is 7 to 21 degrees of elevation and only the very
grazing rays fade out. The fade stayed, in front of the wave normal, and the trace went.

**Retry condition**: a *shallow* camera over a bright floor, where the return path is short
enough for the trace and the limit to disagree. It would need a distance gate that keeps it off a
deep camera, and its own numbers. `tests/snell.rs`'s `the_mirror_does_not_trace` is what it has
to argue with.

**REJECTED, batch 53 -- letting the dark-water rung fire on a shallow camera pitched down.**
The rung stops a submerged tile at `WATER_DARK_DIST` when its rays are under `WATER_DARK_DEPTH`
for the whole of `[0, WATER_FAR_DIST]`, and the *derivation* only needs the interval
`[WATER_DARK_DIST, WATER_FAR_DIST]` -- content nearer than the reach is never culled by it. The
looser test is true about the content and wrong about the feature. At three blocks down a ray
eleven degrees below the horizontal is past 15 blocks deep within 64, so **`sea-horizon` starts
clamping**: 55,058 pixels at max delta 9 with both ends tested against the loose interval,
98,323 at max delta 9 with only the far end. What it buys is **4.7% of `sea-horizon`'s DDA
steps**, against the 6.8% the shipped rung buys at `deep-water` for 19,576 pixels at max delta 3.

**The argument for refusing it is about motion and not about the physics.** Anchored at
`WATER_DARK_DIST`, the clamp is a property of where the player is **looking**: tilt down eleven
degrees and the far sea floor disappears, tilt back and it returns. Anchored at the eye it is a
property of where the player **is**, which changes at swimming speed and, at that depth, between
two frames that are both dark anyway. Batch 52 shipped a seam that *slides* and flagged that no
still frame had priced it; this is the same risk one step worse, and it was declined rather than
measured badly.

**Retry condition**: the user deciding the shallow case is acceptable, the way batch 52's was
decided. It is one line -- drop `submerged` from the `min` in `tile_select` -- and the numbers
above are what it costs and buys. `harness march --only sea-horizon --extra-b ""` re-reads the
work half in one command.

---

## Glass

**REJECTED, batch 38b, by measurement -- the separate opaque-only depth buffer the research
called *mandatory*.** Correction 2 of [`research-review.md`](research-review.md) verified the
mechanism and was right about it: `recover_select` re-tests rejected pairs against the *fresh*
Hi-Z, which `build_hiz` took `atomicMin` over the same frame's poisoned `vis`, so recovery
fixes staleness and not wrongness. The document concluded that a glass pane poisoning a tile's
depth would therefore cull the geometry behind it, and that a second depth buffer was not an
optimisation but a requirement. **The consequence does not follow for this engine**, because
`trace_world` walks the chunk grid and never reads `hiz` or `pairs` -- so nothing Hi-Z rejects
can be missing from the *secondary* ray, and the secondary ray is where the water model puts
the content behind the surface.

Batch 39 argued this from the code and said to price it anyway. Priced: **`--no-hiz` against
the default build is 0 pixels and an identical file hash at all fifteen vantages, `glass`
included.** A whole buffer and a pass, retired by one A/B, and the entry is here rather than in
the ledger because the *shape* is reusable -- a correct mechanism does not carry its
consequence across an architecture it was not derived on.

**FOUND, batch 38b follow-up, by the user in the window -- the sea was invisible through
glass, and no vantage in the fixture could have seen it.** `trace_world` hard-coded
`skip_water = true` from batch 8 until this was found. That was right for all three callers
water had -- its own refraction starts *inside* the medium, its reflection wants the world
above the surface, and a shadow ray must not be stopped by a sea -- so the literal read as a
property of tracing the world when it was really water's own policy. `shade_glass` was the
first caller that wanted the opposite, and inherited the wrong answer: a window onto a bay came
back with the **unlit seabed**, no surface, no medium and the horizon at the wrong height.
Measured at a glass wall above a shoreline, **21,441 pixels of 518,400 at a max channel delta
of 134**.

The fix is a parameter, and it is *two* changes rather than one -- stopping at water is only
half. A water hit handed to `shade_hit` draws the sea through a window as a flat blue cube
face, which is **worse than the bug**, because it looks deliberate. `shade_glass` dispatches on
the block id exactly as the primary ray does, so a glass pixel over water spends three secondary
rays. Bounded, and paid only where a pane actually covers a sea.

**The reusable half is the fixture's blind spot, and it is a composition gap rather than a
coverage one.** Every vantage holding water holds no glass, and the one vantage holding glass
holds no water -- so the *fourteen bit-exact* rows and the *fifteenth moving 168,456 pixels*
were both true, both green, and both structurally incapable of seeing this. **A feature whose
bug only appears where two features overlap is invisible to a fixture that has no vantage
holding both**, and adding a fifteenth vantage does not fix that; adding one where the new
feature meets the *old* ones does. `only_the_transmitted_ray_looks_for_water` is what closes it
here without a capture, and it was checked by reinstating the bug and watching it fail.

**And the general rule: a default that is right for every caller you have is still a default.**
`skip_water` had one correct value for eight batches, which is exactly how long it took to look
like a fact about the function.

**The fix is correct and it is not cheap, and that is its own entry.** Pulling `shade_water`
into `shade_glass`'s call tree took `resolve` from 394,624 bytes to **615,424** and its register
count from **80 to 96** -- the first time that pass has left 80 since batch 13, and the figure
the occupancy entry below has quoted as fixed ever since. The frame cost is **+1.036 ms at
`coastline` with no glass anywhere in that world**, which is roadmap entry 38e and is at the
front of the queue. The correctness was not in question -- an invisible ocean is worse than a
millisecond -- but the pair is the clearest case this project has of the oldest rule about this
pass: **`resolve` bills for code that exists, and a correctness fix pays that bill too.**

**DEFECT -- glass is invisible to every secondary ray, so a glass house reflected in water
shows the world behind its walls.** Deliberate, and it is water's own rule rather than a new
one: `trace_world` has passed `skip_water = true` unconditionally since batch 8, so water is
already invisible in its own reflection and refraction, and no defect has ever been filed
against that. The same `skip_glass = true` is what caps depth complexity at two interfaces
with no counter anywhere. Its other faces: **a second pane's Fresnel and tint are lost** (a
double-pane greenhouse resolves to what is behind both walls, which is the intended behaviour
and a real approximation), and **a pane casts no shadow at all** rather than a tinted one,
because a shadow ray is one bit and the only alternative was full shadow -- which would also
have contradicted `opaque: false` in the flood.

**FOUND, batch 38b -- refracting a thin slab once is worse than not refracting it at all, and
it shows up as a double image rather than as a wrong number.** `shade_glass` was written by
copying `shade_water`'s refraction line for line, `refract(rd, n, 1.0 / GLASS_IOR)` included.
The result put **two copies of everything inside a glasshouse** on the screen -- looking into a
corner, the plinth appeared once through each wall, displaced symmetrically outward -- and
offset the hillside seen through a pane from the same hillside beside it.

A slab refracts **twice**, and for a pane one block thick the entry and exit bends very nearly
cancel, leaving a lateral offset of a fraction of a block. `skip_glass` means the transmitted
ray never finds the exit face, so only the entry half was ever applied and nothing undid it. The
fix is `let rdir = rd;` and it is **more** physical than the line it replaces.

**The reusable half is which term stopped generalizing and why.** Water's single refraction is
correct precisely because water has a *body*: the ray really does continue inside the medium all
the way to the sea floor, so there is no second interface to cancel against. Every other term
carried across to glass untouched. **When generalizing a medium's optics to a surface, the terms
that depend on the path are the ones that break, and the ones that depend on the index are
not** -- `F0` survived, the bend did not.

And it was found by **looking at a picture**, after every number in the batch had already gone
green: fourteen vantages bit-exact, the fifteenth moving 168,000-odd pixels, the crop tag clean
and `validate` passing. Not one of them can see a displaced image, because every one scores a
frame against another frame and both frames had the bend in them. **A fixture built entirely
out of differences is blind to an error that is in both sides of every difference it takes.**

The correction is also *cheaper*, which was not the reason for it and is worth knowing anyway:
`resolve` at the glasshouse vantage went **+1.910 ms to +1.497** against the same revert binary --
both measured before the water fix below, which took the same row to +1.544.
Parallel transmitted rays are more coherent than diverging ones -- they enter the same chunks in
the same order -- which is batch 8's own argument for why water's *reflection* is traced off a
slope-clamped normal.

**DEFECT -- a pane's Fresnel weight goes to the sky and never to a traced reflection**, so
glass facing a hillside shows sky where it should show hill, worst at grazing where the weight
is largest. Batch 8's own split: refraction was 8b and the traced reflection 8c. The shape of
the fix already exists as `--no-water-reflect`'s partner and is a batch, not a line.

**A rule rather than a rejection -- a new material does not need a bit in the visibility key,
and reaching for one is the mistake to catch.** The 38b roadmap entry's design said "one free
bit tags it", written before batch 38 spent the voxel field's last six on the micro-cell. There
was no bit, and the feature shipped without needing one: `resolve` already calls
`block_at(ci, v)` to tell water from everything else, so the block id *is* the tag. **A
primitive the marcher must resolve differently needs key bits; a surface `resolve` must shade
differently does not**, and the two look identical until you ask which pass has to know.

---

## Textures and tiling

**PARTLY FIXED, batch 36 -- every block samples the identical 0..1 of its layer, in phase,
forever.** `face_uv` takes the position *inside* the voxel, so a texture repeats once per block
for water and for **every other block in the world**. A grazing eye compresses many blocks into
few pixels and identical neighbours read as a periodic signal.

Batch 36 broke the phase for the eight layers that can survive being turned -- `GRASS_SIDE` by a
hashed mirror of u, seven isotropic layers by the full dihedral group -- keyed on the voxel's
integer world position. **What is still in phase is the other thirteen**, and the reason is
recorded rather than accidental: `blob()` wraps with `rem_euclid(16)`, so `STONE`, `GRAVEL`,
`COBBLE`, `GLOWSTONE`, `BEDROCK`, `SNOW` and half of `LICHEN` currently tile *seamlessly* across
block boundaries and a permutation would break that join. Whether the variety is worth the seam
is a look question, it wants a picture, and batch 36 declined to answer it.

**A thing the batch's own picture said, which nothing had said before.** With the texture phase
broken, the default vantage's slope still reads as regular banding -- because a large part of
that banding is the *terrain's staircase*, not the texture's phase. The defect was filed for
several batches as wholly textural. It is not, and a batch that goes after the rest of it should
know which half it is attacking before it starts.

**FIXED, batch 36b -- the fixture could not see this defect.** `speckle` moved from 4.0573 to
4.0533 across a change that moved 32% of the frame's pixels, blind exactly as
[`harness.md`](harness.md) warns a coherence complaint will be. `coherence` and
`tests/repeat.rs` are the two halves of the instrument that was missing; what each is worth, and
why it took two, is in [`harness.md`](harness.md).

**REJECTED -- a whole-crop autocorrelation of the frame, at any band.** The obvious form of the
metric above, and it is blind: correlate a crop against itself over a lag annulus and both builds
read 0.4 to 0.7 at every lag and **differ in the fourth decimal**, at high-pass windows of 3, 7,
15, 33 and 129 alike. **The reason is perspective and no amount of tuning reaches it.** A block
spans about 20 pixels at the bottom of the `default` frame and about 3 at the crest, so the
lattice the eye reads as one repeating surface has no single period in the image -- every depth's
peak lands at a different lag carrying a few per cent of the crop's area, and the sum of them is
a smooth monotone decay with no peak in it at all. Inside a 64-pixel tile there is one period and
the peak comes back. **Do not re-derive this**: the whole-crop form is what anyone would write
first, it looks right, and its output is a plausible number that means nothing.

**And a caution the same session paid for twice.** The first version of the texture-space score
correlated the three channels against one global mean, so most of the variance in the vector was
the *palette* -- dirt's red sitting 50 code values above its blue -- which no permutation touches.
Every `D4` layer scored 0.88 to 0.94 and the table said the dihedral group buys almost nothing.
It buys nearly everything: **0.1395 against a 1/8 floor of 0.1250** once each channel is centred
on its own mean. That is `rg_std`'s defect exactly, two quantities charged at once and the one
that moves being the smaller, and it was caught by an assertion that turned out to be a
prediction rather than a check.

**DEFECT -- `FLIP_U` removes about a sixth of `GRASS_SIDE`'s repeat, and `GRASS_SIDE` is what
batch 36 was aimed at.** The seven isotropic layers sit at **0.1395**, within sampling error of
the floor eight states can reach. The layer the batch was actually about sits at **0.8365**,
because mirroring u leaves every row where it was: the green band stays at the top, the soil
stays at the bottom, and only the fringe profile between them alternates. Nothing here is wrong
-- `FLIP_U` is the only class a bounded texture can take, and batch 36's research is right that
rotation would swing the rim onto the wrong edge -- but **the grass slope is most of what a
default frame shows, so most of the world's remaining repeat is this one number.** A batch
wanting the rest of it has to change the *content* rather than the orientation, which is what N
variants means. This is the measurement the roadmap's "price the atlas rather than quoting the
research at it" was asking for, taken before the atlas was priced.

**DEFECT -- `shade_hit`'s tiling of the sea floor seen *through* the water is unmeasured.** The
claim that it is "the larger half of the problem" dates from before the glass-layer fix, was
never scored, and is not obviously true in a current capture with the mottle off. Treat it as an
open question, not a finding.

---

## Sky, clouds and god rays

**REJECTED -- a world shadow ray for terrain crepuscular rays.** One `shadow_ray` per step
measured **+10.57 ms of `resolve`** at 1920x1080 into a low sun, against a whole frame that
costs 4. The cheapest corner of the sweep -- four steps looking only 128 blocks -- is +1.08 ms
and **finds nothing**, because at that vantage the occluding mountain is 300 to 600 blocks
away. *The budget that makes it affordable is the budget that makes it useless.* A step budget
is not the answer; a different algorithm would be.

**Batch 35 built the different algorithm and this entry stands unchanged** -- a per-sample march
is still the wrong shape, and what replaced it is not a cheaper march but a projection, which is
why the reach stopped being a cost at all. The sweep's full table survives only in git, at
**`1f6c4a6:PERF.md` lines 1112-1135**; the consolidation that cut `PERF.md` from 2,813 lines to
187 took it, and a comment in `common.wgsl` went on pointing at it for eight batches. The
vantage, if anyone reprices this: `--time 0.26 --cam-yaw 37 --cam-pitch 2 --cam-height 120
--cloud-cover 0.6`, whose control is 4.14 ms where the coastline's is 8.96 -- so a number taken
only there is taken at the cheap vantage.

**FIXED in batch 35 -- terrain cast no crepuscular ray.** Kept as a heading because it was "the
last visible hole in the stated cinematic goal" for twenty-four batches and that is where a
session will look for it. The deck shadowed the haze's sun lobe and the ground; a ridge did not,
so a mountain with a low sun behind it carried an unshadowed patch of haze glow on its dark
face. `terrain_shade` reads a light-envelope heightfield instead of marching --
[`sky.md`](sky.md) has the mechanism and the constants. Worth **258,498 pixels at `low-sun` and
202,276 at `sky`** of 921,600, **10 of 14 vantages**, against `--no-terrain-shafts`. The four
that do not move are the bounds already working rather than the feature failing: `night` has no
lobe, `lattice` looks away from the sun so the importance ramp cuts it, and `cave` and
`underwater` have no haze along the view ray to shadow. The residuals it left are the four
entries below.

**DEFECT -- the envelope is a heightfield, so an overhang casts a solid column of shadow.** A
single-valued height per column cannot represent an arch or a cliff overhang, so air that should
be lit beneath one is shaded. Ours come only from a cave breaking the surface, which is rare and
sub-pixel at the 300-to-600 blocks the shafts are cast from. Nobody has seen it.

**DEFECT -- the envelope calls a mountain solid, which is right from outside and wrong from
inside.** Standing in a cave looking out through a mouth, the exterior heightfield says the rock
above is solid and no shaft enters. It is unobservable today for a second reason as well as the
first: at the `cave` vantage the view ray hits rock within a few blocks, so the haze opacity over
that path is below the `1e-6` early exit and `sun_shaft` returns before it looks at anything.

**MEASURED, and it retires a roadmap premise -- distant terrain shadows are worth at most 1.7%
of a frame.** Roadmap item 37 said "at a low sun the whole far field is flat, evenly lit
terrain", and batch 37 priced that against an exact march before believing it: a diagnostic
build with `shadow_dist` at **1500** blocks instead of 220 moves **15,348 pixels of 921,600** at
the `lod` camera, **1,328** at a mid camera, and **759, 616, 0 and 602** across four seeds at one
vantage. The premise is geometry, and the geometry squeezes it from both sides: an occluder past
220 blocks has to clear a ray that has already climbed `220 * sin(elev)`, which needs elevation
below about 20 degrees against this generator's relief; and the ground's own `ndl` *is*
`sin(elev)`, so below about 5 degrees there is no direct term left to remove. Swept on the sun
alone: **18,062 pixels at `--time 0.26`, 1,104 at 0.30, 0 at 0.33 and above**. The feature
shipped anyway -- it is free, it is exact where it applies, and 7 of 14 vantages move -- but
**an entry that says "the whole far field" should be priced before it is planned**, and this is
the second time a shaft entry has been written from a screenshot rather than from a march.

**REJECTED as a ground truth -- a long `shadow_ray` past the chunk grid.** The 1500-block march
above is a ceiling for *in-grid* occluders and for nothing else. `shadow_ray` returns `false` the
moment the ray leaves the chunk grid, which is `GRID_X` 32 chunks and so 1024 blocks of
half-width, and at a 1-degree sun a shadow is kilometres long. Measured against it per pixel,
batch 37's envelope shadows **2,805** pixels the march does not at the horizon-sun vantage and
**1** at a 5-degree one -- the difference is the march running out of world, not the envelope
being wrong. Where both move a pixel they move it in the same direction **12,955 times out of
12,955**, and the envelope darkens one *further* than the march **zero** times, which is the
restricted-maximum property of [`sky.md`](sky.md)'s offset identity arriving as a measurement.

**DEFECT -- tree canopies do not cast a shaft, so light slices through a forested crest.** The
canopy is a known lift over the terrain and folding it into the fill is close to a one-line
change; it was deliberately not taken with batch 35, because a canopy height is a look decision
with its own constant and the batch was the mechanism. `coarse_trees` already aggregates exactly
this quantity for the coarse LODs. **Batch 37 gave it a second symptom and the first number
either has had**: the envelope now shades the ground as well as the haze, and against an exact
march it misses **559 pixels at `lod` and 207 at a mid camera** -- unattributed between the
canopy and the 4-block quantisation, because separating them needs the canopy lift itself. One
fill line would move both readers at once, which is what makes it the cheapest item left here.

**DEFECT -- player edits do not cast a shaft.** A tower built on a ridge gets a correct ground
shadow from `shade_hit`'s march and no shaft from the envelope, because `ShaftField` fills from
`WorldGen::height` and the generator does not know about the journal. `journal::apply` already
aggregates deltas per coarse voxel, which is the same shape the fill would need. Cheap, and no
capture in the fixture can see it: `--demo-edits` builds at the camera, where the importance ramp
has already cut the shaft.

**REJECTED -- "the deck's cost is bounded to rays that miss geometry."** False, and it has
surprised two batches. `shade_water` calls `sky_color(mirror)` **unconditionally**, before and
regardless of `FLAG_WATER_REFLECT`, because the Fresnel mix needs a sky term either way. So a
water pixel is a ray that *hit* geometry and still evaluates the sky, and it is a second, third
and fourth door into whatever the sky costs. The most expensive vantage for anything
sky-adjacent is the coastline, not the sky-filling frame. **Before pricing anything new in
`resolve`, bench `--time 0.74 --cam-height 24 --cam-pitch -6 --cam-yaw 143`.**

**REJECTED -- passing `frame.cam_pos` as the water reflection's sky origin.** It shipped once.
The deck is at a finite altitude, so the origin decides which piece of it a ray hits; one origin
for every water pixel turns the reflected deck into a parallax-free sky that slides with the
camera. **Every still capture stayed plausible and the bit-exact control could not see it** --
it switches the deck off. Found by flying. When a batch adds something a *secondary* ray reads,
point `--cam-dolly` at the secondary ray.

**DEFECT -- a clouded sky ghosts under camera translation where a clear one does not.** `taa`
reprojects a sky pixel as a direction, exact for a gradient at infinity and wrong for a deck 258
blocks up. At `--cam-dolly 2` it is a sixth of the residual the voxel world already carries.
Same missing second motion vector as the water reflection.

---

## Foliage, ground cover and biomes

**REJECTED -- folding the foliage intersection out of `resolve` by hoisting the test.** Two
shapes were tried, in both operand orders -- `let foliage_trace = !skip_foliage && foliage_aware`
and guarding `cross_quad` with it -- and the driver removed the code **only** when the constant
reached `foliage_aware` itself. **Do not re-derive this.** The working tool was an `override` in
`SPEC_MASK`, which batch 15 then shipped.

**DEFECT -- ground cover stops at the LOD 0 boundary.** A tuft is one voxel and cannot exist at
stride 2 or more, so foliage is LOD 0 only and that will not change. Batch 18 removed the
visible *step* (mean error against a LOD 0 ground truth fell 15.03 code values to 1.02) and left
three things: the compensation is an albedo so **it cannot know the view angle** (0.59 is right
at a grazing -20 degrees, too dark from straight down); the stamp is two-tone at the coarse
voxel's scale and reads as dapple at stride 8; and **only the top face is repainted**, so a steep
coarse slope still shows bare grass on its sides.

**DEFECT -- the coarse canopy runs 1.25-1.7x LOD 0's, worst in the densest biome.** Proxy
density matches canopy *area*, and LOD-0 canopies overlap where two trees stand close while a
proxy stamp does not. It is the largest residual left in the coarse ground after batch 18 --
2.14 of the remaining 2.14 / 0.01 / 0.90 error is red, and that is the canopy.

**DEFECT -- two of the six biomes take almost no tint and one takes none at all.** The tint set
is grass, leaves and pine leaves. Tundra's surface is `LICHEN` with `tree_scale` 0, so the
coldest biome in the world is the one the cold tint never reaches; the desert is nearly the same
with `SAND` and sparse palms. A look decision rather than a bug -- tinting lichen would put a
biome's colour on something the eye reads as rock.

**DEFECT, batch 38 -- a carved canopy is LOD 0 only, so there is a cliff at every transition.**
A 4^3 micro-cell inside a stride-2 voxel stands for eight blocks and means nothing, so the
cutout is gated to `vs == 1.0` -- the same argument that keeps ground cover at LOD 0. But
`worldgen.rs`'s `coarse_trees` stamps `b.leaf` at every stride up to 8, so unlike ground cover
the *block* is still there at coarse levels, as a solid cube. A porous near canopy therefore
meets a solid far one across a cross-fade that **partitions rays rather than blending samples**.
The research ranked this the critical structural risk of the batch; the first measurement says
it is real and bounded -- at the `lod` vantage, 240 blocks up, the cutout moves **2,508 pixels
of 921,600 at a max channel delta of 16, every one of them inside one bbox in the near field**,
because everything else on screen is already coarse. What that number does *not* cover is
motion: the TAA and Hi-Z halves of the risk need a camera crossing the band, and no vantage in
the fixture does. The proposed fix is generator-side -- blue-noise decimation of the coarse mask
so its mean transmittance matches LOD 0 with the coarse traversal still strictly binary -- and
it is the front of [`roadmap.md`](roadmap.md).

**FIXED in batch 43 -- `LEAF_FILL` shipped unswept for five batches.** 0.72, authored against
the research's 0.45 on the argument that our canopies are a solid 5x5 stamp where theirs were
already sparse. The argument was reasonable and the number was too high: swept over seven builds
against a wooded crest, **0.82 and 0.90 give the same blocky edge a solid cube does**, so the
whole of batch 38 was invisible in a silhouette, and the shipped 0.72 was only just inside the
range where the carve reads at all. It is **0.62** now, and the sweep is in
[`foliage.md`](foliage.md) with the two crops that bound it from opposite sides.

**DEFECT -- the 16x16 atlas is the ceiling on how fine a blade can read.** Raising `TEX_SIZE`
touches every texture, every mip and the alpha tint mask. Its own batch.

---

## LOD and streaming

**FIXED in batch 30 -- coarse LODs do not reflect player edits.** Kept as a heading because two
sessions looked for it here. Coarse chunks still resample the noise at stride 2^n, and still
should: the field is one code path with only the stride differing, so terrain agrees across a
transition already. **What was missing was never the terrain, it was the deltas**, and
`journal::apply` now aggregates those over each coarse voxel's footprint -- the same bargain
`coarse_trees` strikes for a tree it cannot draw. [`persistence.md`](persistence.md) has the
three rules and what they were measured at. The three residuals it left are below.

**DEFECT -- an edit buried under terrain the player has partly dug out stays buried.** Rule 1
gives a coarse voxel to the topmost *occupied* block, and asks the heightfield -- not the edits
-- where the terrain's own top is. So a player who breaks the surface above a block they placed
sees the block at LOD 0 and the bare hillside behind it at every coarse level. Resolving it
exactly means walking all `s^3` positions of a footprint rather than the edits in it, which is
512 at stride 8 against a handful today. **Nobody has seen this**: it needs an edit under an
edit, in one footprint, in the same session. Worth fixing when a second thing wants the full
footprint walk, and not before.

**DEFECT -- the coarse aggregate scans the journal per chunk built, so its cost is
`O(chunks edited)`.** The scan is over the journal's own chunks rather than the volume, because
a LOD-4 chunk stands over 4096 LOD-0 ones; the price of that is a journal with many *chunks* in
it, not many edits. Measured: a settle of **752 chunks takes 0.2 s** with no journal and with a
393-edit one in a single chunk, and **0.3 s** against a synthetic journal touching **10,000
chunks** -- three runs each, `--cam-height 350 --cam-pitch -88`. So it is free at a session's
scale and +50% of worldgen at ten thousand edited chunks. The fix when that day comes is an
index keyed on a coarse chunk, not a different rule; the empty-map early-out is what keeps every
capture in the project clear of it entirely.

**DEFECT -- coarse chunks still carry no lighting**, edited or not, and read as open sky. Batch
30 changed nothing here: `stream::build` computes light for LOD 0 only. A glowstone placed in a
cave lights nothing at a distance, which is invisible in practice because the cave is not
visible either.

**REJECTED -- clearing a coarse voxel by counting raw `AIR` edits in its footprint.** Eight
blocks placed and broken again in mid-air would then delete the ground beneath them, because
nothing in the count knows which positions held anything. The shipped rule counts only edits the
coarse heightfield calls solid, and `a_coarse_voxel_clears_only_when_every_block_it_stands_for_is_gone`
carries that case as its third assertion.


**FIXED in batch 31 -- the headless capture planned LOD at one camera and rendered from
another.** Kept as a heading because the defect was on record for a batch as something else
entirely ("`update` is not idempotent on a settled world"). `find_cave` and `find_water` need a
streamed world before they can choose an eye, so the settle that streams it runs at the spawn
camera; both headless paths now settle again once the eye is placed. Worth **52 pixels at
`open-sea` and 56 at `shore`**, nothing at the other twelve, and the two captures moved in the
manifest for it. The misattribution is under *Measurement method* below, because that is the
part worth not repeating. [`lod.md`](lod.md).

**DEFECT, recorded and not chased -- a settled world keeps unloading for hundreds of frames.**
The retention sweep runs at `frame % BOOKKEEP_EVERY` against a grace period of
`unload_grace_frames`, both frame-counted, so a world that `is_settled` already returned true
for goes on reclaiming coarse stand-ins nothing ever asked to draw -- 3 to 7 chunks at a time,
out past frame 600 in a test that settles at 372. **No capture can see it and none ever could**:
the sweep can only take a chunk the plan has not named for 240 frames, so it cannot take one out
from under a frame. `a_settled_plan_survives_further_updates` pins exactly that -- the plan and
the chunks it names are a fixed point, the resident set is deliberately not.

**DEFECT, recorded and not chased -- retreat unloads pop up to four times as hard as approach.**
`cross_fade_removes_the_pop` measures flips per crossing in both directions, and ledger
rows 100-101 recorded the asymmetry variance at 3.23x-3.67x before the bound was widened.
`tests/lod_stream.rs` now asserts only `ratio < 4.0`, which is a widened bound standing in
front of an unexplained mechanism, not a fix: nothing in the fade schedule explains why a
retreating camera should lose uncovered subtrees faster than an approaching one gains them.
The bound keeps the test green on the recorded variance; the ratio itself is the open
question. Worth chasing only with a capture that shows *which* subtree pops on retreat.

**REJECTED -- prefetching the whole ancestor chain of a selected chunk.** It generates chunks
nothing draws, and since `last_needed` only tracks what the plan asked for, the unload pass
deleted them on arrival -- a stationary camera interned ~16 chunks a frame forever, starving the
budget the moving camera needed. The request walk stops at the first resident ancestor.
`tests/lod_stream.rs` pins this at zero.

**REJECTED -- `--max-lod 0` as a LOD ground truth.** It starves the interning budget and returns
a disc of terrain in an empty sky. Use **`--streaming-factor 8`**, which draws the same ground
at LOD 0 out to 512 blocks and streams in seconds.

---

## Persistence

**DEFECT -- a headless `--save-edits` writes the *capture* camera as the player, and a capture
camera is not somewhere a player would stand.** `--screenshot --cam-height 350 --cam-pitch -88
--save-edits w.journal` records a player 350 blocks in the air looking straight down, and
`--cam-submerge 3` records one underwater. It is not wrong -- that is where the camera was, and
`--resume` reproducing it exactly is the batch-32 measurement -- but a journal produced by a
capture and then *played* starts somewhere surprising. **Nobody has been bitten by this**: the
flags that reach a strange camera are the fixture's, and the fixture never plays. Fixing it
means deciding whether a capture has a player at all, which is a bigger question than the
symptom.

**DEFECT -- the window's restore and the window's save are the two lines of batch 32 no capture
can reach.** `App::new`'s `if let Some(s) = journal.player()` and the `save_if_asked` in
`exiting` are wgpu-and-winit code, and nothing in `tests/` opens a window. Everything behind
them is reached: `--resume` drives `Player::restore` and the whole file path through a capture,
and `save_if_asked` is exercised fourteen times by every sweep. This is `app.rs` saying the same
thing about itself that it has said since batch 22b, and batch 29 said about `exiting` before
it -- listed here so the next session does not have to rediscover which half is covered.

**DEFECT -- a session killed rather than closed loses the player's position and, since batch
34, the bar, though no longer its edits.** Batch 33 appends each edit as it lands under
`--edits`, which fixes the deltas exactly; the player block and the bar block are still written
by the exit save alone, so a killed session comes back where the last clean exit left it,
holding what it held then. **This entry said the position and the edits were one defect with
one fix, and that was wrong**: an edit is discrete and a position is continuous, so appending
can only ever fix the first. Under `--load-edits`/`--save-edits` a kill still costs everything,
which is now a property of that measurement pair rather than of the engine.

The fix is cheap and nobody has taken it: both blocks are fixed-size at fixed offsets, so
stamping them is a 56-byte write in place -- what has to be decided is *when*, and neither
obvious answer is free of a cost. At each edit, the file says where you were when you last
changed the world, which is better than the last clean exit and still not where you died. Per
frame, it is a syscall a frame for a value nobody reads until the process is gone. It is a
schedule, and a schedule needs a constant with a reason at its definition site.

**Batch 34 widened the symptom and did not widen the fix**, which is the thing to know before
pricing it: one decision about *when* now recovers two fields, and the bar is the easier of the
two to argue about -- it changes only when a block is picked, so "at each edit" would leave it
stale in a way a position never is. A schedule that stamps on any change to either block costs
the same syscall and has no such gap.

**DEFECT -- a headless `--edits` run records the capture camera as the player, exactly as
`--save-edits` does.** The entry above about capture cameras applies unchanged; `--edits` adds
no new symptom, and the fixture still never plays.

## GPU, occupancy and shader cost

**REJECTED -- truncating the *primary* shadow march the way batch 41 truncated water's.** The
idea was the front candidate of roadmap P2 and it was measured and killed in batch 42.
`frame.shadow_dist` is 220 blocks and batch 37's light envelope already covers everything past
it, so the same argument seemed to apply -- and it does not, because **land is not water**.
Deleting the march entirely is worth **-1.168 ms +/- 0.073** of `resolve` at `default` and
**-0.643 +/- 0.002** at `low-sun`, so the ray is real money; truncating its *reach* to 64 blocks
is worth **-0.040 +/- 0.000** and **-0.042 +/- 0.002**. **97% of the cost is inside the first 64
blocks.** A shadow ray on land either hits terrain immediately or escapes to open sky
immediately; water's grinds for hundreds of blocks because water is transparent to it, which is
the whole of why batch 41's curve was shaped the other way. Retry only with something that makes
the *near* blocks cheaper -- a coarser first stride, an occupancy skip, or the light brick's own
sky level as an early-out -- and not with a shorter reach.

**REJECTED -- removing glass's duplicated `shade_water` to buy back 38e's millisecond.** Batch
42 built it: the pane's transmitted leg substitutes into the primary dispatch rather than making
its own call, so the module holds one copy of that tree instead of two. It works exactly as
designed -- `resolve` goes **615,424 -> 343,552 bytes**, 85% of everything `SPEC_GLASS` adds --
and the frame is **+0.092 ms +/- 0.002 slower** at `coastline`, 8/8 rounds positive, bit-exact
at all fifteen vantages. **Code size was a hypothesis about time and this is the measurement
that separated them**; see [`gpu.md`](gpu.md) for what it leaves standing and roadmap 38e for
what to try instead. Retry only alongside a register-count fix, since 96-against-80 is the one
difference the hoist could not reach.

**REJECTED, batch 47 -- caching the nine-cell smooth-lighting gather per block face.** The
premise is exact: the nine samples depend on `(ci, v, normal_id)` and not on where in the face
the pixel landed, so every pixel covering one face recomputes them identically, and a block is
about twenty pixels across at the bottom of the `default` frame. Up to four hundred pixels, one
answer. The block is worth **-1.764 ms at `cave` (72% of `resolve`), -0.916 at `default`, -0.899
at `canopy`, -0.687 at `lod`, -0.599 at `terraces`**, so the target was real.

**It is not memory-bound, and one diagnostic said so before anything was built.** Pointing all
nine reads at the same cell -- identical instructions, 640 bytes of `resolve`, perfect sharing
with none of a cache's overhead -- recovers **10% to 13%** of that: -0.191 at `cave`, -0.123 at
`default`. So ~88% of the block is its instruction stream, and a real cache paying a tag compare
and a barrier, in a pass that declares no workgroup memory and no barriers today, would recover
less than a tenth of a millisecond.

**Retry only if the arithmetic gets cheaper, not the fetching.** Eighteen `light_curve`
evaluations (three multiplies and an add each, so a 16-entry table for the 4-bit levels buys
nothing), four corner reciprocals, the AO rule and three bilinear mixes, with no hotspot among
them. Making this cheaper means doing less of it, which means changing the look -- which is what
batch 45 did for the three hits nobody looks at directly, and why it stopped at the primary.

**REJECTED, batch 51 -- cutting the gather on a threshold at the face's own light cell.** The
shape roadmap P5 asked for, built first, and wrong for a reason no threshold reaches. It read
`light_at(ci, v + n)` -- the air cell in front of the face, which is the cell the flat arm
already takes -- and dropped the nine-cell gather when `max(sk, bk)` fell below a constant. It
moved **10,150 pixels at max delta 66 at `overcast`, 10,338 at 46 at `canopy`, 10,150 at 42 at
`default`, 6,953 at 31 at `low-sun`** -- one build, and the same one the millisecond was read off.

**The decisive measurement is that lowering the threshold four-fold gave a bit-identical image**
-- 0.0117 and 0.00139 produced the same hash at all sixteen vantages -- so not one of those
pixels was a face at a middling light level, and no value of the constant was ever going to
help. **A face can be dark while a *diagonal* of its 3x3 is lit**, whenever that diagonal's two
in-plane connectors are solid and the light arrived from in front of the face rather than through
the plane. `light.rs` floods by decrementing one per step over the six face neighbours, so it
bounds every cell it can *reach* and says nothing about one it cannot. That is an inside corner,
voxel terrain is made of them, and it is the corner the AO rule shuts hardest -- so the cut was
removing contact shading at exactly the corners roadmap A7 was declined for losing.

**A chunk-boundary guard was suspected first and priced at 132 pixels.** The 3x3 at a boundary
face is served by `world_sample`, which crosses into a neighbouring chunk flooded independently;
refusing those faces took `default` from 10,150 to **10,018** and `canopy` from 10,338 to 10,327.
Real, 1.3% of the effect, and that build was never re-benched -- every millisecond above belongs
to the unguarded one. The seam itself is **roadmap R1's apron**, which was entry P5b until batch 56 folded it there.

**REJECTED by the same batch -- the exact version, which is a no-op.** The second build tested
the *chunk* instead: `light_flags` bit 0 plus a span test, which together say the nine cells
**are** one byte rather than that this one is dark. Bit-exact at all sixteen vantages, and
**-0.017 ms at `cave`, sign mixed over six rounds** -- because a chunk is 64 blocks tall, so the
chunk holding a cave holds the lit surface above it, `chunk_uniform` is false, and the gate
rejects. It fires almost nowhere.

**The pair is the entry.** One condition is cheap and admits faces it must not; the other is
exact and admits almost nothing. **Neither shipped**, and the tree is byte-identical to its
parent apart from documentation.

**Two lessons, and the second is the one that nearly got away.** Being dark and having a dark
neighbourhood are two claims, and a sample of the centre is only ever the first. And **bit-exact
at sixteen vantages was the feature doing nothing** -- `lessons.md`'s *a no-op claim cannot be
told apart from a not-wired-up claim*, met in the wild, with only the bench able to separate
them. A capture cannot tell a correct early-out from one that never fires.

**Retry at the 4^3 light brick**, which is neither the face nor the chunk: bricks carry their own
uniform bit, a dark region is uniformly dark at brick scale, and a face's 3x3 spans at most four
of them. Roadmap P5 has the design and the numbers both shapes cost.

**REJECTED, batch 46 -- folding the *other* eight gates the way batch 45 folded its own.**
Batch 45's rewrite was worth 2.58 ms, so the obvious follow-up was the eight switch overrides
still written `SPEC_X && (frame.flags & FLAG_X) != 0u`. All eight were folded and measured:
**-0.119 ms +/- 0.029 at `terraces`** and inside their own standard error at `coastline`,
`default`, `shore` and `low-sun`; the two largest movers alone are -0.058 at `terraces` and
**+0.001 at `open-sea`**. Bit-exact at all sixteen vantages, as a pure fold must be, and
`resolve` drops 12,544 bytes against batch 45's 60,928.

**Why it does not transfer**: the cost is the gate shape *times the arm behind it times how
many copies the pass inlines*. Batch 45's arm was the nine-cell light gather inside `shade_hit`,
which `resolve` inlines four times, so the run-time test kept three redundant copies of nine
memory lookups. These eight guard a uv permutation, a simplex tint, a shaft sample and four
wave-field branches. **Not shipped**, because the `frame.flags` half is what keeps the flag word
the thing that decides -- a pipeline key that ever disagreed with it would still draw the
picture the flag asked for -- and a tenth of a millisecond at one vantage does not buy that
property eight more times.

**Retry only for an arm that is large *and* sits in a function the pass inlines several times,
and screen with `shaderstats` first.** The byte delta predicted this entire result in two
minutes: 12,544 against 60,928 is the whole story, and no bench was needed to see the ratio.
`SPEC_FOLIAGE` was already folded (batch 14 found 384 bytes in it) and `SPEC_LEAF_CUTOUT` and
`SPEC_GLASS` are gated differently again, so the list really is eight and not the nine an
earlier count of the grep suggested -- two of those hits were comments.

**REJECTED, batch 45 -- gating a hot inlined arm with the house style's `frame.flags` test.**
The batch's own first build, kept here because the wrong form is the form a tidy-up reaches
for. Secondary hits took a one-lookup light model instead of the nine-cell gather, gated
`SPEC_FLAT_SECONDARY && (frame.flags & FLAG_FLAT_SECONDARY) != 0u` -- which is how every switch
override from batch 13 to 38b is written. It was **uniformly slower**: +1.000 ms of `resolve`
at `terraces`, **+1.420 at `default`**, +1.465 at `lattice`, +1.391 at `glass`, 8/8 rounds
positive at five of six vantages. Rewritten as `!SPEC_FLAT_SECONDARY` the identical feature is
**-1.584 / -0.502 / -0.726 / -0.682**, a 2.58 ms swing at one vantage with the same arm taken
and the same pixels out. `frame.flags` is a run-time value, so the driver keeps both arms in
each of the three copies of `shade_hit` it inlines. **`default` is what identifies it**: a
frame with water only at its right edge paid the largest regression, and a cost that lands
hardest where a feature does least work is code existing rather than work running. See
[`gpu.md`](gpu.md); `tests/secondary.rs` is the guard.

**REJECTED -- chasing occupancy. Twice, deliberately, both empty.** Batch 13 took `resolve` from
50% to 66.7% theoretical occupancy -- eight more resident warps -- and the pass moved
**-0.01 ms**. Batch 17's sweep raised `tile_select` from 67% to 100% with nothing else in it and
bought **exactly zero**. The occupancy arithmetic says what *cannot* be the problem and has
never once said what is. **`resolve` is not register-starved and not occupancy-limited**: 80
registers at 8x8, no spill, no stack anywhere in the frame.

**And the conclusion is overturned as well as the number, which batches 66 and 67 settled.**
`resolve` **is** occupancy-limited, and the two rejections above are still correct about what
they measured -- raising *theoretical* occupancy bought nothing, twice. What pays is the register
count itself: at `cave`, where the feature under test draws nothing, **16 registers cost 0.156 ms
and 32 cost 0.441**, the same figure for two unrelated features, and batch 67 spent that to take
the pass **96 to 80** for +0.103 ms at `cave` and +0.399 at `coastline`. So read the bolded claim
above as *the occupancy arithmetic never found it* and not as *the pass does not care* -- it
cares, and `shaderstats` is what shows it. [`lessons.md`](lessons.md) has the law.

**That 80 is stale as of batch 38b and the number now depends on a pipeline key**, which is the
first thing a performance batch here has to know. `resolve` reads **96 registers with
`SPEC_GLASS=1`** and **80 with it off** -- the first time this pass has left 80 since batch 13,
and every occupancy figure above was computed at the old shape. Re-read it with `shaderstats
--override` before quoting any of it; a table computed at one specialization describes a shader
the other one is not.

**What does pay is warp *fill*.** `tile_select` at 4x4 ran 16 threads against a 32-lane warp,
and filling it bought **2.3x of the pass** at three vantages. Count the lanes against the
subgroup width before you open the occupancy table. **The subgroup width is 32, measured at batch
69 and no longer something to look up** -- `probe` prints `subgroup size: min 32 max 32`, so
`resolve` at 16x8 is exactly four full subgroups and `tile_select` at 8x8 is two.

**REJECTED -- 16x8 and 16x16 for `tile_select`.** Built, measured, thrown away. **This entry
also said "and 16x8 for `resolve`" until batch 69, and three things contradicted it**: the
shipping shader is `@compute @workgroup_size(16, 8, 1)` with a comment explaining why, ledger row
13 says that batch *shipped* `resolve` reshaped to 16x8, and git has it at 16x8 in the first
commit there is. **The code is the copy that cannot drift**, which is batch 26's lesson and the
reason this half of the sentence went rather than the ledger's. What was rejected for `resolve`
and is not in dispute is that the *reshape bought no time*: its own comment says it is the
ceiling being lifted off rather than a speedup. Also: widening a block from 64 to 128 threads makes the driver spend **fewer
registers per thread** (80 down to 64, with a 16-byte spill), so "more threads" and "the same
code per thread" are not both true, and an occupancy table computed at the old shape describes a
shader that no longer exists.

**REJECTED -- run dedup and ancestor memoisation.** Both failed to pay; measured dedup on
procedural terrain is **1.00x**, which is expected. Three levels is not deep enough for
memoisation to matter. Recorded in `PERF.md`; do not re-attempt without a different tree shape.

**DEFECT -- code compiled into `resolve` costs about a millisecond whether or not it runs.**
Batch 12 measured +0.07 ms and called it occupancy; it was code size. Batch 14 measured +1.14 and
it is both. Anything added to that pass, **or reachable from it through `trace_world`**, is
billed for existing -- batch 14's cost was `cross_quad` being pulled into `resolve`'s call tree
by water's secondary rays, not the branch added to `shade_hit`. `SPEC_MASK` is the remedy and not
a cure: it removes the code for runs that do not need the feature, and the shipping build still
carries every byte.

**FOUND, batch 38 -- a folded-away branch is not free, and where you put it decides the frame.**
Batch 38 added a 4^3 micro-cell to the visibility key, which meant `hit_t` had to reconstruct a
different plane. Every shape that put `if SPEC_LEAF_CUTOUT` on that path left the **disabled**
build one ULP away from the binary it was supposed to reproduce -- **201 pixels of 230,400 at a
max channel delta of 1**, on terrain only, sky byte-identical. Three shapes were tried and all
three failed: an early `return` inside `hit_t`, a selected offset with one `return`, and the
branch hoisted into its own dispatch function behind a plain `let`. Removing the branch and
leaving the parameter made it bit-exact, which is what proved the parameter and the widened
decode were never the problem.

**What shipped is no branch at all**: `march_chunk` records the entry micro-cell on *every*
solid hit, so the micro plane reduces to the cube's own face exactly -- cell 0 at offset 0, cell
MICRO-1 at offset MICRO, and `MICRO * (vs / MICRO)` is `vs` on the nose for every power-of-two
stride. One expression serves both. `hit_t_does_not_branch_on_the_cutout_override` is the guard.

The separate half of the same hunt: the *awareness* flag cost **5,120 bytes of `resolve`** with
the constant false, written the way the foliage flag above it is -- the second `if` outside the
first. Nesting both tests inside the override's own block took it to 1,664 and spelling out
`is_cutout_id` took it to 1,024. **`march` folded to byte-identical throughout**; it is the
deferred pass, which reaches `march_chunk` through water's secondary rays, that is fussy. Same
family as the foliage entry above, and the same moral: the constant has to reach every use, not
merely the expensive one, and an expression with the same value is not the same expression.

**DEFECT, recorded and not chased -- `resolve` reports 32 bytes of Local Memory Size** where
every other pass reports none, registers unchanged at 72, stack 0, and it disappears with
`SPEC_WAVES=0`. Ruled out: returning a `vec3` from `wave_octave` instead of a two-field struct
(saved 129 SPIR-V words, moved the local memory not at all), and inlining the four direction
constants. No measurable cost attached.

**REJECTED -- reading the visibility buffer before writing it, to skip the `atomicMax` that
loses.** Roadmap P3c, built and measured in batch 50 and **not shipped**. `march` issues
`atomicMax(&vis[pix], key)` on every hit and discards the result, so a chunk behind a nearer one
pays a read-modify-write to change nothing; batch 49 had measured that suppressing such writes
is worth 0.850 ms of frame, 91% of it billed to `resolve`. The skip is one `atomicLoad` and a
compare, **bit-exact by the monotonicity of `atomicMax`** rather than by a capture, and it buys
**nothing at the vantage that should suit it best**: `resolve` -0.029 +/- 0.021 at `underwater`
(t = -1.35) and exactly +0.000 at `default`, against a build with the feature compiled out.

**Hi-Z is why, and a count says so where a timing could not.** An occlusion cull removes
precisely the chunks whose writes would lose, so the losers are gone before `march` sees them.
Writes that would lose, hi-z off against hi-z on: **58.0% -> 12.8% at `underwater`, 72.5% ->
21.5% at `default`, 69.8% -> 0.0% at `cave`** -- Hi-Z taking 94.7%, 94.0% and **100%** of them.
`cave` reads 2,073,600 hits for a 1920x1080 frame, exactly one per pixel, because every tile
marches only the camera's own chunk. So the pretest pays a load on **every** hit to skip an
atomic on one in five to one in eight, and at `cave` on none.

**Batch 49's number was never a bound on this, and the roadmap entry quoting it contradicted
itself.** The ghost build suppressed a *superset* -- writes that lose, and writes that **win**
at a pixel nothing nearer reached, which is why it rerouted the next frame's pairs.
Read-before-write removes only the first kind and Hi-Z has already removed most of those. The
entry noted the superset and then also argued the ghost figure was "pessimistic" for a shipping
version; that is true of the Hi-Z feedback, false overall, and the superset is much the larger
effect. **A measurement of a superset is an upper bound on the subset and nothing more** -- an
error that arrives dressed as caution.

**Retry condition**: a *cheaper read of the same word* -- never `atomicLoad`, which is what lost
here -- and only if something makes Hi-Z stop collecting this first.

**RULED OUT -- a GPU power or clock state as the explanation for one pass moving when another
pass's work changed.** This was half of roadmap P3b and the dangerous half, because it would
have made the per-pass split of every paired bench in this project soft. Batch 49 killed it
twice at `underwater`. **A build that marches 372,335 pairs against one that marches 221,963 --
2.380 ms of `march` -- moves `resolve` by -0.002 ms**, t = -0.39, 2/8 rounds; a clock that
answered to how hard the previous pass worked would have answered to that. And in the build
where `resolve` *does* move, it moves **-20.5%** while `select` reads 0.155 against 0.155 and
`taa` 0.502 against 0.501. **A clock change is global and would show in the short passes, which
are already printed in every bench table.** Read them before reaching for a power explanation;
the 1.8x drift `PERF.md` warns about is between *sittings*, not between two passes of one frame.

**RULED OUT -- `resolve` reading a warmer visibility buffer.** The other half of P3b, and the
one the roadmap led with. With hi-z **on**, `build_hiz` runs between `march` and `resolve` and
streams the whole buffer -- 16.6 MB at 1920x1080 -- in 0.110 ms identical on both sides, which is
**151 GB/s and so a read from memory rather than from cache**: nothing `march` left resident
survives to be read, and `resolve`
still moves **-0.756**. What does explain it is the write traffic itself rather than its
residue, and **91% of what suppressing it saves is billed to `resolve` and 9% to `march`** --
see `PERF.md` and [`gpu.md`](gpu.md). The retry condition is a pass that issues fire-and-forget
writes; there is exactly one in this frame.
**RULED OUT, batch 53 -- `cell[axis]` forcing the DDA's state into scratch memory.** The first
hypothesis a reader reaches for on this loop, because `march_chunk` and `cutout_march` both
index vectors with a run-time `axis` and a dynamic index into a function-local vector is the
classic way to lose a register allocation. **`shaderstats` answered it for free and before any
bench**: `march` reads **72 registers, no spill, no stack and no local memory**, and it read the
same after the batch added two traversal features to it. `gpu.md`'s note that the driver reports
a constant 2^36 of "Local Memory Size" on every pass is what makes that readable at all -- the
low bits move exactly once in this project, to 16, at the moment a spill happens.

**RULED OUT, batch 53 -- `MAX_ITERS` as either a cost lever or a correctness defect.** A ray that
runs past 512 steps returns `hit = false`, which is a hole in the world, and until this batch
nothing anywhere could say whether it ever happened. `--march-stats` counts it: **at most 19
pixels of 921,600** at the worst vantage in the set, 1 to 6 at the rest. And that is an **upper
bound rather than a count** -- `dbg` sums across every pair covering a pixel, so four chunks at
140 steps read 560 with no ray having given up. The guard rail is not being touched.

**REJECTED, batch 53 -- widening the coalesced sub-leaf test from a 2x2x2 to a 4x4x4 group.**
Priced as a ceiling with a deliberately incorrect `skip = select(1, 4, ...)` and counted: **-10%
of `default`'s DDA steps, -0.4% at `sea-horizon`, 0% at `deep-water`**. Dropped on that row
alone, and there is no exact version to build anyway -- a 4x4x4 group is the whole leaf, and an
empty leaf is already `skip = 4` from the level above. The same technique applied one level up
**did** pay and shipped; see `gpu.md`.

**FOUND and FIXED, batch 53 -- the pair list truncates at 5K, not at 8K, and 4K sits at 80% of
it.** Roadmap P4 and `gpu.md` both carried "reachable at 8K" from batch 22's arithmetic. Measured
at `default` against the fixed `MAX_PAIRS` of 1,048,576: **1,529,818 pairs at 5120x2880**, 46%
over and truncating, and **80.5% of the cap at plain 3840x2160**. A 4K capture was one busier
camera or one `--scale 2` away from silently losing geometry, and `--screenshot` offers both.
The list is screen-sized and is now sized from the screen, like `vis`, `hiz` and `dbg`, with
`MAX_PAIRS` kept as the **floor** so 1280x720 and 1920x1080 allocate exactly what every number
in this tree was measured at. `tests/pairs.rs` pins that.

**FOUND batch 53, FIXED in batch 72 -- a root-`FULL` chunk was opaque to a ray that ignores water.**
`march_chunk`'s fast path is `if (c.root.z & FULL_BIT) != 0u { h.hit = true; ... }` and it does
not consult `skip_water`. A chunk whose 262,144 voxels are **all** occupied -- water above, stone
below, no air, which is any 64^3 region wholly under the sea surface holding the floor -- returns
a hit at its own face to every refracted, reflected and shadow ray. The uniform case is handled
three lines earlier and is why this has never shown; the **mixed** case is not.

Measured by the one build that bounds it (return no hit when `water_aware`, wrong in the other
direction): **31,815 pixels at max delta 95 at `coastline`**, 2 at `sea-horizon`, and **0 at
`deep-water`, `underwater` and `terraces`**. The camera that shows it is the one that is *not*
submerged, which is why thirty-one batches missed it: `coastline`'s primary ray sees water as
geometry, and the pixels belong to `resolve`'s three secondary legs.

**Not fixed in batch 53**, which had already shipped three sub-leads, and because neither build
is correct -- the right answer is the first dry voxel inside the chunk and the loop cannot
produce one: `Inner::full` carries `run_ptr = FULL_FLAG`, so `root_ptr` is 0 and
`inners[0 + mask_below(...)]` reads another chunk's node. The fast path is load-bearing.

**Fixed in batch 72 (roadmap P10)**: the loop now walks a full root's cells with the node
synthesized exactly as `leaf_attr` builds it -- occupancy is all ones for free, and the
attribute run went in dense at 64 entries per cell the day the tree was written, so only the
index was ever missing. `--no-full-march` keeps the face-hit as the control.

**FOUND and GUARDED, batch 53 -- the pair word's tile field, unprotected for thirty-one
batches.** `tile_select` packs `(tile_id << 13) | chunk_id` into a `u32`, so a screen may hold
2^19 = 524,288 tiles. **7680x4320 is 518,400 -- 98.9%** -- and 8192x4320 is over, at which point
the shift drops the high bits and a tile marches the chunks some *other* tile selected. **It
does not lose geometry; it draws the wrong geometry**, in a frame that is lit, shaded,
temporally stable and wrong, which is this project's own definition of the failure mode worth
fixing first. `Renderer::warn_if_oversized` is the guard and `OVERSIZE_MARK` the phrase, kept
separate from `TRUNCATION_MARK` because the two are different defects with different fixes.
`--scale 2` is the argument most likely to reach it, since it multiplies the *internal*
resolution and nothing in the flag's name says so.

---

### The bounce's elevation form factor, built and rejected (batch 58b, 2026-09-15)

**The full record is [`ADR 0001`](adr/0001-no-elevation-form-factor.md)** -- it was reverted from
the working tree rather than committed, so there is no diff to find and that file is the only
trace. The short form, so nobody has to open it to know the shape:

- **What**: scale the baked bounce by how much of each axis's hemisphere is ground rather than
  sky -- ceiling 2.0, wall 1.0, open floor 0.0, anchored on the wall.
- **Why not**: `floor_rgb` is **7% of the light on a lit surface** (`Config::ambient` is 0.08), and
  **ceilings are 3.6% of the best of eight upward-looking cameras and 0% in three of them**. It
  moved `default` by a mean of **2.87 code values** and `lod` by 5.49, the latter almost entirely
  *darker* -- 27% of the frame against 2% brighter.
- **Retry condition**: only if the ambient floor ever carries materially more energy. Not a finer
  grid, not more azimuths, not a better form factor -- the derivation was correct and tested.


### A pinned-field A/B is not bit-exact where the multiply lands inside a sum (batch 58)

**The claim that failed, and it was this batch's own.** Batch 57 has an `implies` row saying
that `--probe-fill 1.0` already switches the ambient cube off, because what `shade_hit` does
with the tap is multiply `face_shade` by it -- so naming `--no-probe-cube` on top is a no-op.
It passes at all four vantages. Batch 58 added the twin of it for the ground bounce and **it
fails at `terraces`: 2 pixels at max delta 1, and 0 at `default`, `lod` and `canopy`.**

**It reproduces.** Three consecutive runs, identical file hashes each time, so this is not the
non-reproducing `implies` reading batch 57 filed at the front of the roadmap -- that one is
`--no-water` against the wave flags at `lattice`, it is 1 pixel, and it fired again in the same
sweep. Two small numbers in one output that are **not** the same phenomenon, which is worth
saying because they look identical in the table.

**The mechanism is `hit_t`'s invariant in a new place.** `CLAUDE.md` records that branching
inside `hit_t` on `SPEC_LEAF_CUTOUT` "makes the driver rebuild the folded function one ULP away
and costs the control its bit-exactness for 201 pixels". The same thing here:

```
albedo * ((ambient_rgb + sun_rgb) * ao + floor_rgb) * shade
```

Batch 57's tap multiplies **`shade`** -- a scalar factor at the end of the expression, which the
driver folds identically whether or not the multiply is there. Batch 58's multiplies
**`floor_rgb`**, an addend *inside* the parenthesised sum, and reassociating a sum is where a
ULP goes. So the two rows differ by where in one expression the term sits, and nothing else.

**What this does and does not say:**

- **It says nothing against the control.** `--no-probe-bounce` is bit-exact against
  `voxelcraft-pre58.exe` -- a real revert, not a flag -- at **18 of 18 vantages, 0 pixels,
  identical file hashes**. The control build folds the way the parent build folds, because it is
  the same code. What is one ULP away is the *feature's* pipeline with the field pinned.
- **Batch 60 added a third arm at the same expression site and it is exact, which sharpens the
  rule rather than softening it.** `--no-probe-sun` implies `--probe-sun-high` changes nothing:
  4 vantages, 0 pixels, identical file hashes, across two different pipeline keys, on a term
  that is an addend into `floor_rgb` exactly as batch 58's is. **That is not a counter-example
  and must not be read as one.** Batch 58's two arms differ in *live* code at the site -- one
  multiplies `floor_rgb` and the other does not, so the sum is reassociated. Batch 60's differ
  only in an override that feeds `probe_sun_gain`, inside a block `SPEC_PROBE_SUN` has already
  dead-stripped in **both** arms, so the driver is handed identical code and there is nothing
  to fold differently. **The distinction to carry is live versus dead, not site versus site**:
  a `SPEC_MASK` bit whose only consumer is dead is free of this hazard, and a bit that changes
  which terms reach a sum is not, however small the change looks.

- **It says the pinned-field question cannot always be asked this way**, which is what the row
  existed for: batch 55 invented `--probe-fill` to separate "the field's contents moved the
  frame" from "the code path around it did", and that separation is only available where the
  fold happens to survive. **Whether it survives is a property of the expression site, not of
  the design**, so check it rather than assume it -- and do not read a row like batch 57's as
  licence to add its twin.
- **No fix was attempted and one is not wanted.** Making the control build carry the multiply
  too -- `floor_rgb * select(vec3(1.0), probe.rgb, SPEC_PROBE_BOUNCE)` -- would fold the two
  diagnostics together and **break the 18-of-18 claim against the parent**, which is the one
  that matters. That is the trade `hit_t` already made in the other direction, and it goes the
  same way here.

**Retry condition:** none. The row was removed rather than relaxed, because `Composes` has only
`Nothing` and `Something` and a third "almost nothing" variant would weaken every existing row
to buy one.


## Measurement method

**REJECTED -- a same-binary A/B for a feature's cost.** Batch 14 measured foliage on against
`--no-foliage` in one executable, got **+0.24 ms**, and found the coastline looked *cheaper*
with the feature on. Both sides were carrying 55 KB of shader the feature had added. Against a
real revert the same two vantages read **+1.85 and +2.13**. A flag that switches a feature off is
not a build without the feature in it. Build `voxelcraft-preNN.exe` before you start.

**REJECTED -- analysing a bench as first-half against second-half.** `resolve` walks
monotonically from 9.25 to 9.43 ms over sixteen consecutive runs of the *identical* command, so
any arrangement that puts one side systematically later charges it that: a **null** experiment
analysed that way reports **+0.099 ms**, which is exactly the phantom cost a four-batch emergency
was built on. Pair inside a round, alternate which side runs first (the second process starts
warmer, +0.03), and report a standard error. `harness bench` is all three.

**REJECTED -- `shaderstats`' binary size as proof two builds are the same program.** Two builds
both reported 240128 bytes and 72 registers and their SPIR-V was **177 words and 708 bytes
apart**; the size is a quantised allocation figure and the register count a maximum. Diff the
bytes: `--wgsl` dumps the module and `--source` reads one back, so it costs no rebuild.

**REJECTED -- supersampling as a ground truth.** Mip selection divides by `res.y`, so a
supersampled render picks finer mips and is a *different signal*; and above 5120x2880 the frame
silently drops geometry through `MAX_PAIRS` truncation. The near water came back a flat blue
that read perfectly plausibly as smooth water. Use `--reference N`: a sub-pixel jitter grid at
native resolution, averaged in linear.

**REJECTED -- checkerboarding a jitter grid to get a noise floor.** `(i + j) % 2` makes each half
a quincunx, itself well stratified, so the halves agree far better than independent sets and the
floor reads **3x optimistic**. Splitting on sample index is biased the other way. The shipped rule
is a rank-balanced hash, and anything quoting a floor must say how it split.

**REJECTED -- trusting an under-converged reference.** 64 samples gave a half-split floor of
**1.434 against a signal of 1.515** -- 95% noise -- and ranked a sweep cleanly and confidently.
An under-converged reference aliases *in sympathy* with an unfiltered build, so the bug scores as
accuracy. 256 samples put the floor at 0.55 and the table changed meaning.

**REJECTED -- a no-op claim standing alone in `harness implies`.** An entry asserting that a
control does nothing **cannot be told apart from one asserting the control is not wired up** --
both render two identical frames and `Nothing` is the pass condition for each. `--water-mottle
0.2 -> Nothing` passed every run for six batches while being true only of a build whose default
was wrong. Keep no-op claims in a *pair* with a positive one, reading the quantity from two
different places.

**REJECTED -- inferring a cause from a colour-channel signature.** "+32.5 red, +6.8 green, +1.0
blue, so it is path length" reads like a fingerprint and is a property of the sRGB transfer plus
whatever made red dark. Absorption makes red dark, a dark channel sits in the steep part of the
transfer, so **every** brightness variation under water comes out red-dominant. A monotone
response to `--water-absorb` is the same error again: absorption is the amplifier, so anything
routed through it is monotone in it.

**REJECTED -- changing two things in one diagnostic build.** The shader-side test that appeared
to confirm the water layer set the uv to `(0.5, 0.5)` **and** the mip to 3. It proved "not this
layer *as currently sampled*", which is much weaker than "not this layer", and cost three builds.

**REJECTED -- "three causes eliminated, so it is the fourth."** Batch 30 saw two captures move
under a second `settle()`, eliminated the aggregate, the rebuild and the journal -- each
soundly, each by stubbing it out -- and concluded `update` was not idempotent on a settled
world. It is: four extra calls at an **unmoved** camera are 0 pixels at all fourteen vantages.
Every one of those experiments varied the *edit* path and none varied the *camera*, and a
settle plans at whatever camera it is handed, which at that point in the capture path is the
one `find_water` had just chosen. The extra call was correcting a stale plan, which is why the
fix reproduces the same 52 and 56 exactly. **An elimination sweep names a cause only if the
surviving candidate was in it**, and the cheapest test -- the same extra call with nothing else
changed -- was the one not run.

**REJECTED -- an A/B whose side B falls back to side A's input by default.** Batch 32's first
`--resume` test rendered a capture at the default camera, saved the journal, and rendered it
back with `--resume` and no camera flags: **0 pixels, MAE 0.0000**, and completely worthless,
because side B's *default* camera is the one side A was given. A `--resume` that did nothing at
all scores exactly the same. This is the vacuous vantage in its other costume -- the nine
out-of-frame vantages of batch 29's round trip are a claim about *content* the camera cannot
see, and this is a claim about a *flag* whose absence is indistinguishable from its presence.
The fix is one line: give side A a camera the defaults cannot reach, which read **921,600
pixels and MAE 42.18** against the default vantage and 0 against its own source. **When the
thing under test is a flag, check what the run does with the flag removed, not only what it does
with the flag on.**

**FOUND, batch 39 -- a known-positive is only as durable as the frame it was read off, and
this one was reading the tree canopy.** `CHECKS`'s `coherence` row fell from **1.2053x to
1.0755x** and under its 1.10 floor with no line of the metric changed and no constant moved.
The cause is batch 38: carving the canopy into a 4^3 mask removed four fifths of what
`--no-tex-variation` had been moving at `default/slope`. **35 of the 190 tiles carried 79% of
the response, those 35 are the canopy tiles, and every canopy-free tile is unchanged to four
decimals** -- so the in-phase lattice the control was mostly moving was the `LEAVES` layer,
`D4`-permuted, on solid leaf cubes, and never `GRASS_SIDE`'s corduroy on the slope the crop is
named for. Nothing written at the time suspected it, `metric.rs`, `harness.md`, `tests/repeat.rs`
and the parked 36c entry all included, and `tests/repeat.rs` had the causality backwards: it
credited `GRASS_SIDE`'s 0.8365 as the *limiter* on the frame reading, where the swing a control
produces is the distance from a layer's permuted score **to 1.0**, so a `D4` layer at 0.14
swings six times as far as a `FLIP_U` one at 0.84. **The score times the area the layer covers
is the prediction; the score alone is not.**

Three things this rules out, each measured rather than argued, so none is re-proposed:

- **A more robust summary over tiles does not help.** The dilution hypothesis -- a minority of
  dead tiles dragging a healthy mean down -- predicts a median or a trimmed mean recovers it.
  Mean **1.0755x**, median 1.0836, top three quarters 1.0714, top quarter 1.0683, top tenth
  1.0717. They agree to the second decimal. The tiles did not stop contributing; what they were
  contributing stopped existing.
- **Re-fitting the four constants does not help.** All four re-swept against the carved frame:
  highpass still 3, tile still 64, `lag_min` flat, and `COH_LAG_MAX` 24 edging 32 by 1.0853
  against 1.0755 -- a third decimal against a forward-looking reason for 32 that still holds.
- **There is nowhere to re-point the row.** `--no-tex-variation` is worth 1.0736x at
  `coastline/water`, which is a texture control read off a `water` crop, and under 1.02x at
  every other crop in the fixture -- **the wrong way** at `shore`, `terraces` and `underwater`.

**REJECTED, batch 39 -- propping the row up with `--no-leaf-cutout` on both sides.** It restores
**1.2053x exactly**, which is what makes it tempting and what makes it wrong: the row would then
certify the instrument against a build nobody ships, and would need propping again the next time
leaves change. The row was re-fitted to 1.037 instead, and **the claim that the instrument is
not blind was moved to where no frame can reach it** -- the synthetic lattice pair in
`tests/harness.rs`, at 12.7x off two images differing in exactly one thing. It had carried that
claim all along; batch 39 is what made the division of labour explicit.

**DEFECT, batch 39 -- `compare --profile` measures a different quantity from the metric it is
documented as placing.** `coherence_grid` correlates the **whole crop**; `coherence` takes a
peak inside each 64-pixel tile and means them. The two disagree by construction and the
disagreement *is* the argument for tiling -- `COH_TILE`'s own comment says a whole-crop
correlation has "no peak in it at all" -- yet `harness.md` called `--profile` "how the annulus
gets placed". Both readings are useful and neither is wrong; what was missing is the sentence
saying they are not the same surface. Not chased further because `--profile` answers the
question it is actually asked (where does a lag sit), and `compare --tiles` now answers the one
it was being credited with (where does a response live).

**A rule rather than a rejection -- when a change that must move the image moves nothing, believe
the null.** Flattening the water layer came back **0 pixels at fourteen vantages**. That is not a
fix that failed to help; it is the frame saying it never read that layer, and it is what found
the real bug. When a sweep prints hashes, look for two rows that match.

