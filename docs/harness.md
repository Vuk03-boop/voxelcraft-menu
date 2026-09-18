
# The measurement fixture

**Read the whole of this before you measure anything.** It is short on purpose.
**Consolidated from batches 22, 22b and 24.** Rejected measurement methods -- same-binary A/B,
first-half-vs-second-half benching, supersampled ground truths, checkerboard noise floors -- are
in [`errors.md`](errors.md) with the numbers they lost by.

`src/harness/` is the data and the definitions; `src/bin/harness.rs` is the driver. **It renders
nothing itself -- it drives `voxelcraft.exe` by path**, which is a design decision and not a
convenience: a fixture that could only measure itself could not do the job the ledger most
depends on, because *a flag that switches a feature off is not a build without the feature in it*.

---

## The commands

```bash
cargo run --release --bin harness -- list        # the vantage set and every crop, as data
cargo run --release --bin harness -- capture     # the whole reference set, one command
cargo run --release --bin harness -- validate    # every metric against a control known to move it
cargo run --release --bin harness -- implies     # every "clears X too" claim in the control table
cargo run --release --bin harness -- bitexact --exe-b ./voxelcraft-preNN.exe
cargo run --release --bin harness -- bench --vantage coastline --exe-b ./voxelcraft-preNN.exe
cargo run --release --bin harness -- march --exe-b ./voxelcraft-preNN.exe
cargo run --release --bin harness -- reference --vantage lattice --samples 16 --mip0
cargo run --release --bin harness -- compare a.png b.png --vantage terraces
cargo run --release --bin harness -- compare a.png b.png --rect 0.2,0.86,0.3,0.99 --profile 32
cargo run --release --bin harness -- compare a.png b.png --vantage default --crop slope --tiles
```

**`capture` and `validate` at the start of a batch; `implies` and `bitexact` at the end.**

**Do not hand-roll a vantage, a crop, a metric or a hash loop -- nineteen sessions did.** And do
not hand-roll a bench A/B either: `bench` is paired, alternates which side runs first, and prints
a standard error.

---

## A number is a vantage, a crop and a metric definition

Missing any one of the three, it is not a measurement. The same `--no-water-refract` control is
worth MAE **7.47, 15.77, 14.84 or 38.43** depending on which of two vantages and which of two
crops.

**The set runs at 1280x720 with the temporal pass off.** 1280x720 because six batches used it;
TAA off because a temporal pass hides exactly the high-frequency residual the look-metrics
exist to measure.

**How many vantages there are is not written down here any more, and that is deliberate.**
This sentence read *fourteen*, then *seventeen*, then *eighteen*, and was wrong on the day
each of the last three was added -- batch 59 makes it **nineteen** with `lamps`. Run
`harness list`. **Every count in this file and in the ledger is historical**: *bit-exact at
all sixteen vantages* is a true statement about the sweep that batch ran and is not a claim
about today's set, so do not read one as the other and do not update them -- a batch that
rewrote its predecessors' counts would be falsifying measurements to tidy a number, which is
the thing `CLAUDE.md`'s note about *roadmap L1* exists to refuse.

**Batch 64 added `offscreen-shadow`, and it is `sea-horizon`'s reason with the geometry turned
from a depth into an angle.** Roadmap D3 is terrain outside the view frustum casting no shadow,
and the camera that can see that has to satisfy two conditions at once: the occluder more than
**52 degrees** off the view axis -- the frustum's own horizontal half-angle, below which nothing
is culled -- and within **220 blocks** of the ground it darkens, past which batch 37's light
envelope carries the shadow off a height window no frustum touches. **Sixteen of the nineteen
cameras that predate it satisfy neither and are bit-exact under `--no-offscreen-shadows`.**

**The other three are why this vantage's `CHECKS` row is an MAE and not a count, and the three
rows above it in that table do not transfer.** `sea-horizon`, `deep-water` under the dark rung
and `deep-water` under Snell's window are each *the whole* of the fixture's ability to see their
feature, so a count over the frame is the right guard there. This one is not: `terraces` moves
7,843 pixels and `lattice` 32. What it is alone in is **rank** -- MAE 2.6165 over its own crop
against a whole-frame 0.2552 -- so a count here would be satisfied by a camera that had drifted
onto somebody else's response.

**Its two numbers disagree about which camera is better and the disagreement is the point.** The
same control at `--time 0.28` moves **32,937** pixels against 0.32's **18,468**, and over the
`shadowed-slope` crop 0.28 is **MAE 0.22 at max delta 9** against 0.32's **2.6165 at max delta
27**. A count answers *did the feature fire*; it cannot answer *can anybody see it*, which is
what batch 63 spent a whole batch learning. The vantage ships at 0.32 and its `CHECKS` row is an
**MAE over the crop** rather than a pixel count over the frame, so a build whose shadow had gone
faint would fail it.

**And the defect it is named for is still not something one frame can show.** What the user
reported is a shadow that changes as they turn: at this camera's position the control moves
29,240 pixels at yaw 265 and **0 at yaw 275**, ten degrees away, with the same slope in both
frames. `tests/shadow_grid.rs` is where that claim lives as an equality -- the grid-eligible set
is identical at eight yaws from one position -- because a capture cannot hold a disagreement
between two cameras.

**Batch 52 added the seventeenth, `sea-horizon`, and it is the first one added to make a
*control* visible rather than a constant.** A submerged eye along the horizontal at a chain of
wooded islands 264 blocks out -- the camera roadmap P4 specified and no vantage had. Until it
existed, `--no-water-far` was bit-exact at every vantage in the set, which read as a property of
the feature and was a property of the cameras: `underwater` is the only other submerged one and
its content ends within 32 blocks. It now moves **67,621 pixels of 921,600 at max delta 23**
there, and the whole `WATER_FAR_DIST` ladder is rankable in one command.

**`--cam-pitch 0` is the argument that makes it work, and it is worth knowing why**, because the
obvious submerged camera points down and that one is blind by construction. The clamp fires only
for a tile whose most *upward* corner ray is still under water at the reach -- three blocks down
that is `atan(3/256)` = 0.67 degrees -- so the clamped region is everything at or below the
horizon and nothing above it. Above y = 0.43 of this frame the reach moves **zero** pixels at
any value. A vantage that drifted back to the default -14 degree pitch would put the whole frame
inside the clamped region and no distance in it at all, which is `underwater` again;
`tests/submerged.rs` asserts the pitch for exactly that reason.

**Batch 53 added the eighteenth, `deep-water`, and it is `sea-horizon`'s lesson applied one
batch early rather than one batch late.** An eye **24 blocks down** over open sea floor, along
the horizontal. Roadmap P4's depth sub-lead is the user's own report -- *"the only time it looks
odd from inside is when I go too deep"* -- and **no vantage in the set was deep**: `underwater`
and `sea-horizon` both sit three blocks under, which is less than a quarter of the depth at
which the sky flood runs out. The rung it ranks cannot fire at either, so both would have
returned bit-exact and agreed with anything.

**24 is the deepest `find_water` reaches near the spawn column**; 30 finds nothing at all, and
the search wanders 54 blocks to manage 24. What that depth buys is the only property that
matters here -- it is past `WATER_DARK_DEPTH`'s 15, so the rung fires on the **camera** rather
than on the pitch, which is what `the_dark_rung_still_has_a_deep_vantage_under_it` pins.

**`--cam-pitch 0` again, and for `sea-horizon`'s reason turned inside out.** There the clamped
region was everything at or below the horizon. Here the camera is deep enough that *below* the
horizon is near sea floor within a chunk or two, and the content a reach can delete sits in a
narrow band of near-horizontal rays just **above** it -- the ones that graze forward for
hundreds of blocks while rising a few. Measured before the batch was written, and the reason
this camera was chosen over yaw 180: against an unlimited reach the *existing* 128-block clamp
moves 36,820 pixels here, **every one of them between y = 0.33 and y = 0.54** and none at all
below. A pitched-down camera would have had the near floor in front of every ray and nothing to
cut.

Its `floor` crop is the half `sea-horizon` does not carry: a region that must **not** move,
where both of that vantage's crops respond. It reads **1 pixel** under the shipped rung.

**Batch 43 added the sixteenth, `canopy`, and it exists for a crop rather than for a camera.**
A wooded crest at eye level against open sky: the only vantage where a canopy is a *silhouette*
rather than a mass, which is what `SPEC_LEAF_FILL` decides and what roadmap 38d had nothing to
sweep against. Every one of its arguments predates the batch, so unlike `glass` it can be a
`bitexact` row against the parent binary from the day it was added.

**Batch 38b added the fifteenth, `glass`, and it is the only one holding a transmissive block
that is not water.** It is the `edits` camera exactly, with `--demo-glass` in place of
`--demo-edits`, so the two differ in the structure and in nothing else. A second structure
rather than glass added to the hut, because `edits` has been a reference since batch 29 and
half a dozen batches quote numbers off it -- putting a pane in it would have moved every one of
them to save a function.

**They are data, not prose.** `harness list` prints them; `tests/harness.rs` runs **every argument
list through the real command-line parser** and fails on anything it does not recognise. Before
that, a vantage that had stopped being runnable would silently produce a capture from the default
camera -- which is a frame, and looks like one, and is not the vantage anybody asked for. **Quote
a vantage by name and take its arguments from `list` rather than retyping them.**

**A crop is a rectangle in fractions of the frame**, so it means the same region at any capture
size, and it carries a **checkable tag**. `validate` holds it: a `water` crop must move on at
least **99%** of its pixels and a `land` or `sky` crop on **exactly none**, against
`--no-water-refract --no-water-reflect --no-waves --water-absorb 0`. Not against `--no-water`:
that drops the sea level to zero, so at any `--cam-submerge` vantage the camera search finds no
water and the whole frame moves. `--rect x0,y0,x1,y1` on `compare` scores an ad-hoc region, which
is how a candidate crop gets placed before it becomes one.

---

## Seven metrics, and the rule they have to pass

`mae`, `max`, `pixels` (differences), `speckle`, `coherence`, `rg_std` and `rg_speckle` (single
image). Definitions live at the definition site -- `speckle`'s 3x3 mean includes the centre
pixel and reads neighbours from the whole image with coordinates clamped, so a crop at the frame
edge is not penalised for being there.

**The rule, stated as code in `harness::CHECKS`: a measure that cannot see a known-positive is
not fit to rule anything out.** Each metric is paired with a control known to move it and a
floor at roughly half the measured response -- a floor rather than the value, because a floor
catches a metric that has gone blind while an equality would just be a second copy of the number.

**`rg_speckle` exists because a metric can be unfit as an acceptance test while being a perfectly
good diagnostic.** `rg_std` charges the smooth depth cue and the per-voxel lattice together, so a
change deleting both would score as a triumph; `rg_speckle` is the same quantity high-passed by
`speckle`'s own 3x3, and the pair separates them.

**A fixture's known-positives can be calibrated against a bug, and only fixing the bug reveals
it.** Two of the original six were.

### `coherence`, and what it is and is not worth

**`speckle` is blind to *coherence* by construction** -- it charges a lattice and a field of
unrelated noise the same amount. Batch 36 is the demonstration: 32% of the frame's pixels moved
and `speckle` went 4.0573 to 4.0533. `coherence` is the other half of that pair, the way
`rg_speckle` is `rg_std`'s: the **mean over 64-pixel tiles of the peak normalised
autocorrelation** inside a 6-to-32-pixel lag annulus, after `speckle`'s own 3x3 high-pass.
`speckle` answers *how much* detail is here; this answers *how regular* it is.

**Three things about it were measured rather than chosen, and the numbers are at each constant's
own definition in `metric.rs`.** All three were re-swept in batch 39 against the frame batch 38
left, and **none of them moved** -- the numbers below are the originals, and each constant's own
comment carries the re-sweep beside them.

- **A whole-crop correlation cannot see this defect, and the reason is perspective rather than
  tuning.** A block spans about 20 pixels at the bottom of the `default` frame and about 3 at
  the crest, so the lattice has no single period in the image: correlate the whole crop and
  every depth's peak lands at a different lag carrying a few per cent of the area, and the sum
  is a smooth decay with no peak in it. Both builds then read 0.4 to 0.7 at every lag and
  **differ in the fourth decimal**. The tile is what fixes it, and 64 is an optimum rather than a
  direction: 1.18x at 48, **1.20x at 64**, 1.19x at 96, 1.08x at 128.
- **The band has to be the texel scale.** Against `--no-tex-variation` the response falls
  monotonically as the high-pass box widens -- 1.20x at 3, 1.18x at 5, 1.12x at 9, 1.08x at 15,
  blind at 129 -- because a wider box keeps more of the frame's own shape, and the slope's shape
  is the same in both builds.
- **The annulus has to reach the period.** Stopping at 20 pixels scores 1.07x where reaching 24
  scores 1.21x: at `default/slope` the responding lattice is 21 to 24 pixels. It ships at 32, and
  may not exceed half the tile, because a lag of `L` in a tile of `T` leaves `(T - L)^2` pairs
  and a maximum over fifteen hundred noisy estimates is biased upward by the noisiest of them.

**It was worth 1.2053x and is worth 1.0755x, and batch 39 is the whole reason to read this
section rather than quote it.** `0.1510 to 0.1819` at `default/slope` became `0.1420 to 0.1527`
with **no line of the metric changing and no constant moving** -- batch 38 carved the tree
canopy, and four fifths of what this control had been moving was leaf cubes sampling their
layer in phase. It is now the weakest row in `CHECKS` by a factor of five, at a floor of
**1.037** that can catch total blindness and very little else. Under 2% at every other crop in
the set, **exactly nothing at `lod`**, where a frame 240 blocks up has no block wide enough to
carry a period, and the *wrong way* at `shore`, `terraces` and `underwater`. A coherence claim
at a vantage this metric has not been shown to respond at is not a measurement, and after batch
38 that is very nearly every vantage.

### A known-positive is only as durable as the frame it was read off

**This is the lesson, and it cost a batch.** `CHECKS` pairs each metric with a control known to
move it and a floor at half the measured response, so a metric that goes blind is caught. What
it cannot catch on its own is the *third* possibility: the instrument is fine, the control is
fine, and the **content the pair was reading has been removed by an unrelated feature**. That is
what batch 38 did, and the floor fired as designed without anyone being able to say which of the
three had happened.

Telling them apart needed a reading this fixture did not have, which is why `compare --tiles`
now exists beside `compare --profile`. `coherence` is a *mean over tiles*, so a response is a
sum of 190-odd contributions and the metric reports only the total. The breakdown is decisive in
one command:

| tiles of 190 | share of the `--no-tex-variation` response at `default/slope` |
|---|---|
| top 1 | 7.6% |
| top 10 | 50.4% |
| top 20 | 74.6% |
| top 35 | 86.2% |

and the 35 tiles holding tree canopy carried **79%** on their own, falling from `+0.1330` to
`+0.0266` when the canopy was carved, while **every canopy-free tile is unchanged to four
decimals**. So the in-phase lattice this control was mostly moving was never `GRASS_SIDE`'s
corduroy on the slope the crop is named for -- it was the `LEAVES` layer, `D4`-permuted, on
solid leaf cubes. **Batch 38 was accidentally the larger share of a texture-repetition fix**, and
nothing written at the time -- this file included -- suspected it.

Three repairs were measured and two were declined, which is what stops them being re-proposed:

- **A more robust summary over tiles.** The first hypothesis was dilution -- a minority of dead
  tiles dragging a healthy mean down -- which a median or a trimmed mean would fix. Scored on
  the same four images the carved frame reads **1.0755x under the mean, 1.0836 median, 1.0714
  top-three-quarters, 1.0683 top-quarter, 1.0717 top-tenth**. They agree to the second decimal.
  The tiles did not stop contributing; the thing they were contributing stopped existing, and no
  summary recovers a signal that is not in the image.
- **Re-fitting the four constants.** All four were re-swept against the carved frame and the
  optimum did not move: highpass still 3, tile still 64 (1.0218 at 32, 1.0671 at 96, and
  **0.9696 at 192**, where the tile is wide enough to hold several depths and the reading
  inverts). The one wanderer is `COH_LAG_MAX`, where 24 now edges 32 by **1.0853 against
  1.0755** -- a third decimal against a forward-looking reason for 32 that still holds, so it
  stays, and it is recorded because a sweep whose winner moved inside the noise has to say so.
- **Re-pointing the row somewhere else.** There is nowhere: `--no-tex-variation` is worth
  **1.0736x** at `coastline/water` -- a texture control read off a `water` crop, which the crop
  tags exist to refuse -- and under 1.02x everywhere else.
- **Propping it up with `--no-leaf-cutout` on both sides**, which restores 1.2053x exactly and
  was **declined**: the row would then speak for a build nobody ships, and it would have to be
  propped again the next time leaves change.

What was done instead is to re-fit the floor to half the response, say all of the above at the
row itself, and **move the fitness claim to where no frame can reach it** -- the synthetic
lattice pair in `tests/harness.rs`, which reads 12.7x off two images differing in exactly one
thing. That test carried the claim all along; batch 39 is what made the division of labour
explicit.

**Batch 43 moved it a second time, for the same reason and by less.** Thinning `SPEC_LEAF_FILL`
from 0.72 to 0.62 takes roughly a tenth more leaf surface out of the frame, and the
`--no-tex-variation` row at `default/slope` went **1.0755x to 1.0594x** -- 0.1404 against 0.1487
where batch 39 read 0.1420 against 0.1527. It passes, with the floor at 1.037, and **the floor
was deliberately not re-fitted**: batch 39 set it at half the response it measured, half of this
response would be about 1.030, and lowering a floor because the batch doing the lowering just
made it harder to meet is the shape of self-dealing that section exists to refuse. What it costs
is margin -- the row has about 2.2 points of it now against 3.9 before -- so **the next batch
that removes canopy owes this row a reading before it believes a pass**, and if it goes under,
the answer is the synthetic lattice pair in `tests/harness.rs` carrying the claim rather than a
third re-fit.

**And it is not `pixels` wearing a different hat.** The same control moves **597,986 pixels of
921,600** at `terraces` -- twice what it moves at `default` -- and `coherence` there reads
**0.9957x**, which is nothing. How much of a frame changed and how regular that frame is are
different questions, and the pair of numbers at `terraces` is the cleanest statement of it
this fixture has.

The reason it is weak is worth more than the number: **the peak it finds is a lattice, and the
lattice is the terrain's staircase and the texture's phase together.** `--no-tex-variation` can
only move the share that is texture, and that share is a fifth. Batch 36's picture said this in
words; this is the number under it. It also charges **quantisation** -- the `coastline` sky crop
reads 0.72, the highest in the set, off nothing but the sRGB contours of a smooth gradient.

**Its known-positive is synthetic and lives in `tests/harness.rs`**, not in a render, and that is
deliberate: the same noise latticed at 20 pixels and unlatticed scores **1.0000 against 0.0785**,
a separation of 12.7x at the correct lag, while `speckle` cannot tell the pair apart at all. The
`CHECKS` row says what the metric is worth on a frame; the test says the instrument is not blind.
A frame conflates them and could never say which had failed.

**`compare --profile N` prints the whole lag grid instead of the metric row**, which is how the
annulus was placed and how the next batch to move it should do the same. It stands to
`COH_LAG_MIN`/`COH_LAG_MAX` exactly as `--rect` stands to a crop.

**It correlates the whole crop, and `coherence` does not** -- that one takes a peak inside each
64-pixel tile and means them, for the perspective reason above. The two disagree by
construction, and the disagreement *is* the argument for tiling, so `--profile` is a diagnosis
of the crop and not the surface the metric maximises over. Batch 39 found that distinction
undocumented, in a file that called `--profile` "how the annulus gets placed". For where a
response lives rather than where a lag does, the reading is **`compare --tiles`**.

### And a second time, where the fixture had excluded the case on purpose (batch 67)

**Batch 52's lesson repeated, with a sharper edge: the blind spot was documented and nobody read
it as one.** Batch 67 replaced how water is shaded when seen *through a pane* and the change moved
**0 pixels at all twenty vantages**, `glass` included -- the only one holding a pane at all. Its
own crop comment explains why: the crop is tagged `Land`, and a glasshouse *"that ever landed on a
shoreline would fail it rather than quietly scoring a water crop as a glass one."* That tag is
right, and it means the set had deliberately excluded the one configuration that could see this.

**Twenty rows of zero could not argue the change was safe, only that nobody was looking.**
`glass-sea` is the twenty-first vantage: `--demo-glass` with `--cam-submerge 1`, which matters
because `build_glass_structure` drops the house at whatever surface the camera faces, so an eye one
block above the sea puts panes over water. Three neighbouring cameras -- submerge -1, 0 and -2 --
read **0**, because the house lands on the beach. There the change reads **123,079 pixels at max
delta 37, MAE 0.5990**, under [`ADR 0001`](adr/0001-no-elevation-form-factor.md)'s 2.87.

**The generalisation: a crop tag that excludes a configuration is a statement about the fixture's
reach, not only about the crop.** When a change moves zero everywhere, read the tags before
concluding it is safe.

### A zero is only as meaningful as the frame's ability to show one (batch 48)

**The mirror image of the section above, and it is the more dangerous of the two**, because a
known-positive going quiet announces itself against its floor and a *control check* reading zero
looks exactly like success. `bitexact`'s pass condition is **0 pixels differing**. So is the
reading you get from a vantage that has nothing to differ about.

Batch 48 clamped how far a submerged camera tests chunks and got 16 vantages, 0 pixels, identical
hashes -- correct, and nearly banked as evidence the constant had margin. It has none. The
constant was then stressed by putting `--water-absorb` on **both** sides (the technique in the
section below) and walked down to **0.05**, where water barely absorbs at all and blue still
transmits 88% at the clamp distance. Still zero. The probe that explained it was shortening the
clamp to 32 blocks, which moved **one pixel at max delta 4**: `underwater`'s submerged content
ends within 32 blocks, so *every* setting above that is bit-exact and the sweep was measuring
nothing.

**The rule: before reading a zero as a property of the change, establish that the vantage can
produce a non-zero at all.** The cheapest way is to push the constant past any plausible value
until the frame *does* move -- if it never does, the vantage is blind and the zeros are about the
camera. A control-purity check is exempt, because there the zero is the whole claim and the
positive half is measured separately (batch 45's pairing); it is **authored look constants** that
this catches, where a clean sweep is otherwise taken as permission to ship the aggressive value.

**Batch 52 closed this one, and the ending is not the one the rule predicts.** It built the
camera -- `sea-horizon`, the seventeenth vantage -- and the reach became rankable. So the blind
sweep had indeed been hiding something. **What it was hiding is that the aggressive value was
affordable**: 128 blocks costs a visible seam and buys **1.930 ms of a 10.6 ms frame**, and it
shipped. The rule above is written as though the crop exists to *stop* the aggressive value, and
here it licensed it. **A blind vantage is not a conservative one** -- it is one that cannot argue
in either direction, and a batch that treats "I could not see it" as "therefore be careful" is
guessing with a confident tone. What the crop bought was not caution; it was the right to decide.

**And the guard that keeps it from going blind again is a `CHECKS` row, not prose.**
`--no-water-far` at `sea-horizon` has to move at least 150 pixels. Its floor is fitted to the
*weakest defensible reach* (316 pixels at 256) rather than to the shipped one (67,621 at 128),
because a floor at half the shipped response would be an assertion about `WATER_FAR_DIST` and
would fail the day anybody legitimately put the reach back up. **It is the only row in `CHECKS`
that polices a vantage rather than a metric**, and it is there because every other kind of check
this fixture has would have passed throughout batch 48.

### Ranking a repeat is not `coherence`'s job -- `tests/repeat.rs` is

**Nothing can be ranked at 1.20x** -- nor at the 1.0755x it has since fallen to -- and both
remaining halves of roadmap item 36b are ranking questions. So the quantity is also computed where it has no estimator in it at all: on the atlas.
Two blocks show the same layer under two draws of the same hash, and the mean correlation between
what they show is a closed form in the texels and `block::PERM_CLASS` -- no crop, no band, no
tile, no annulus, no perspective and no geometry. `tests/waves.rs` is the precedent and the shape
is deliberately its.

| what | repeat score |
|---|---|
| the thirteen layers still at `perm::NONE` | **1.0000**, by construction |
| `GRASS_SIDE`, the layer batch 36 was aimed at, under `FLIP_U` | **0.8365** |
| the seven isotropic layers under `D4`, mean | **0.1395**, against a 1/8 floor of 0.1250 |

The dihedral group is within sampling error of the best eight states can do; the mirror that
`GRASS_SIDE` takes removes about a sixth of its repeat, because mirroring u leaves every row
where it was -- the green band stays at the top, the soil at the bottom, and only the fringe
profile between them alternates. The batch took seven layers to the floor and left its own
subject at 0.84.

**This table used to be read as explaining the 1.20x, and batch 39 measured that the reading
was backwards.** The story was that `GRASS_SIDE`'s stubborn 0.8365 is what *limits* the frame
number, "and a frame full of grass slope is what you see that in". But the swing a frame control
produces is the distance from a layer's permuted score **to 1.0**, so a `D4` layer at 0.14 swings
six times as far as a `FLIP_U` layer at 0.84 -- and on the `default` frame the `D4` layer with
the area behind it was `LEAVES`. The tiles agree: 79% of the response came from the canopy.
**The score here times the area the layer covers is the prediction; the score alone is not**, and
that is the sentence this table was missing rather than a number in it being wrong.

**A variant set is a set of images exactly as a permutation orbit is**, so the same function
scores it and the two land on one axis -- which is what lets N variants be ranked as a *marginal*
gain over the permutation rather than an absolute one. `N` uncorrelated variants floor at `1/N`,
which is the number to compare against rather than against zero.

Two things that file pins that an image cannot: the states are the shader's own three bits,
asserted against `render::shader_source()`; and the closed form assumes neighbours draw
independently and uniformly, which is **measured** rather than assumed -- adjacent blocks share a
state 0.1253 of the time against an independent 0.1250, and the empirical score over real
adjacent pairs agrees with the closed form to 0.0002.

`validate` runs the negative first: two renders of the same arguments must be the same file.
Everything else is a claim about a difference, and a difference measured on a non-deterministic
renderer is not a measurement.

---

### Putting a control on *both* sides, to ask what a second feature is covering for

**`--extra` applies to both sides, and that is a measurement rather than a convenience.**
`--extra-b` exists so the two sides can differ; `--extra` exists so they can be made to agree
about something the question is not about, and batch 41 is what that is for.

The batch truncated water's shadow march from 48 blocks to 16 and measured **1,309 pixels of
921,600 at max channel delta 1**, which reads as "the band was empty". It was not: batch 37's
light envelope resumes exactly where the march stops, so the reading was the *envelope's*
agreement with the march and not the band's emptiness. Re-running the identical sweep with
`--extra "--no-distant-shadows"` -- the envelope switched off on **both** sides -- separates
them in one command:

| truncation | with the envelope | with `--no-distant-shadows` on both sides |
|---|---|---|
| 48 -> 16 | 19 pixels at `shore`, max delta 1 | **1,084**, max delta 2 |
| 48 -> 0 | 92 pixels at `shore`, max delta 1 | **369,628**, max delta 14 |

The first row says the band really is nearly empty and the envelope is not what makes it look
that way. The second says the near sixteen blocks hold essentially the whole shadow, and that
switching *them* off would have been hidden almost perfectly by the envelope -- 92 pixels
standing in for 369,628. **A cheap-looking number is either a small effect or a good stand-in,
and putting the stand-in's own control on both sides is what tells you which**, at the cost of
one extra sweep and no new code.

### And the same thing as a durable property of a `CHECKS` row (`MetricCheck::both`)

**The section above is a sweep somebody runs once. `both` is that arrangement kept**, and there
is exactly one row in the table using it. It is deliberately hard to reach for, because the
obvious use of it is the thing batch 39 refused.

**Batch 39's refusal stands.** That batch was offered `--no-leaf-cutout` on both sides of
`--no-tex-variation`, which restores its response from 1.0755x to 1.2053x exactly, and declined
it *"because the row would then speak for a build nobody ships"*. `both` does not repeal that.

**What it is for is a different failure, and roadmap D4 is the case that produced it: a metric
whose *premise* has been violated, as against one whose *response* has shrunk.** Batch 39's row
had honestly got weaker -- the frame really did hold less in-phase lattice after batch 38 carved
the canopy -- so propping it up would have certified the instrument against a frame that no
longer existed. A confound is the other thing. `rg_speckle`'s row asserts that **absorption** is
what carries a high-frequency R-G signal in water; batch 63 gave every face in the world an R-G
signal out of the *sky's hue*, which is not absorption and was never what the row was pointed at.
The ratio, which has to *fall*, rose instead -- **0.4993x to 1.2722x** on the binary built from
batch 63's own commit, and it stayed failing for two batches. Cancelling the hue on both arms
changes nothing about what the row certifies for absorption; it is the only way to certify it.

**Two conditions, both measured rather than argued, and neither automatic:**

- **The confound must be one flag, not one per batch.** A prop that has to grow every time an
  appearance rung lands is batch 39's objection arriving slowly. This was tested rather than
  hoped: **batch 65 put a Fresnel reflection of the sky on every opaque surface in the world and
  moved this row by 0.008** -- 0.5156x to 0.5073x -- where batch 63 alone moved it by 0.77.
- **The companion row stays unpropped.** `rg_std` measures the same control on the same crop with
  `both` empty and passes on the shipping frame at **0.3427x**. Batch 21b built the pair so that
  neither half could be satisfied by deleting both signals; a pair with both halves isolated
  would lose the half that speaks for the build that ships.

**The printed table names it** -- `--water-absorb 0 (both: --no-sky-tint)` -- because a row
measured against an isolated baseline and a row measured against the shipping frame are different
claims, and a table that printed them identically would be the quiet version of the thing batch
39 refused out loud.

## The converged reference, and its floor

`--reference N` renders an N x N sub-pixel jitter grid **in one process and one world**, averages
in linear, and writes the mean plus the two half-averages. **It always prints its own half-split
noise floor**, and nothing here can hand back a reference without the number that says whether it
can be believed.

The grid is at **native resolution**, because the mip footprint divides by `res.y` -- a
supersampled render is a different signal, not a better sample of the same one.

**The split rule moves the floor by 3x and neither obvious rule is right.** Checkerboarding
`(i + j) % 2` makes each half a **quincunx**, itself well stratified, so the halves agree far
better than independent sets and the floor reads 3x optimistic. Splitting on sample index is
biased the other way. The shipped rule is a **rank-balanced hash**, and **anything quoting a
floor has to say how it split**.

---

## Benching: three rules, and the middle one is the one nobody had

`resolve` walks monotonically from **9.25 to 9.43 ms over sixteen consecutive runs of the
identical command** -- a +0.18 warm-up ramp. Any arrangement that puts one side's runs
systematically later charges it that.

1. **Pair inside a round.**
2. **Alternate which side runs first** -- the second process of a pair starts warmer, worth +0.03.
3. **Report a standard error**, because `--bench-frames` prints a mean with no dispersion and the
   differences argued about here are 1% of a pass.

`harness bench` is all three, and it exists so no session re-derives them. A **null** experiment
analysed the old way reports **+0.099 ms**, which is exactly the phantom that carried a
four-batch emergency.

---

## Two fixture defects worth knowing, both fixed

- **`bitexact` could not express a same-binary A/B**, which is *why* no composition claim had
  ever been checked -- both halves of "`--no-biomes` clears `--no-tint`" live in one executable.
  **`--extra-b` is side B's argument set**; with no `--exe-b` it means this build twice with
  different flags. `--extra` alone applies to both sides.
- **An unrecognised argument printed to stderr and exited 0**, so a misspelled control rendered
  the same frame as no control -- and in a sweep whose pass condition *is* `0 pixels differing`,
  that is the one failure mode indistinguishable from success. `harness::render` now refuses such
  a capture, keyed on `config::UNKNOWN_ARG_MARK`.

**`harness`'s own parser is now strict** -- the wart this paragraph used to record is closed by
batch 102's in-tree driver. An unknown `--flag` is a usage error naming the vocabulary, and a
misspelled `--vantage` fails fast in `vantage::select` instead of falling through to the whole
set and announcing itself by taking ten minutes. The guard now protects both calls: the inner
one (`harness::render` refusing a capture the engine ran with an ignored flag) and the outer
one (`Opts::parse` refusing a flag the driver itself does not know).

**`compare` takes two `.png` paths** where every sibling takes `--vantage`, and **`bitexact`
exits non-zero whenever images differ** -- right for a control check, awkward when measuring how
far a feature moves.

---

## Building the revert binary

**Every A/B claim needs a build without the feature in it, and since batch 27 the parent commit
is always there to build one from.** Check it out in a worktree, build, copy
`target/release/voxelcraft.exe` to `voxelcraft-preNN.exe` in the repository root -- `.gitignore`
covers that name -- and end with `bitexact --exe-b ./voxelcraft-preNN.exe`. The `./` is not style: Rust's `Command::new` does not search the current directory (Windows per the standard library, POSIX by PATH without the cwd), so a bare `voxelcraft-preNN.exe` in the repository root reports *program not found* from a file that is right there.

Copying the binary before you start is still the cheapest moment to get one. **It is no longer
the only one**, and this section used to say it was: during the five batches with no `.git`, a
forgotten copy could not be reconstructed, because the working tree had already moved past the
build you needed.

If you need one mid-batch, the cheap fake is to force the feature's own gate to a literal `false`
in the shader and build that -- the driver dead-strips the rest. Check it against `shaderstats`.

### A vantage whose flag the parent binary cannot parse cannot be a `bitexact` row

**Batch 38b hit this on its last command of the session, and the next batch to add a vantage
with a new flag will hit it too.** The `glass` vantage passes `--demo-glass`, which exists only
in the build that added it, so `bitexact --exe-b voxelcraft-pre38b.exe` **refused the row** --
batch 22b's `config::UNKNOWN_ARG_MARK` guard caught the old binary ignoring the argument. That
is the guard doing exactly its job: without it the capture would have been the *default camera*
wearing the vantage's name, which is the one failure mode indistinguishable from success.

So the sweep is the fourteen that predate the batch, and the new vantage's positive claim has to
come from somewhere else. **The way round is batch 33's, and it makes the A/B a real revert
rather than a flag:** `--save-edits` the structure out of the new binary and `--load-edits` it
into the old one, which knows the journal format because the batch did not change it. Check the
substitution before you use it -- the replayed capture against the `--demo-glass` one, same
binary, which read **0 pixels of 921,600** -- and then both binaries are rendering the same
world and the difference is the feature.

**And the trick does not always exist, which batch 59 is the case for.** Its `lamps`
vantage builds a chamber out of block ids that *did not exist* in the parent binary, and
`block::def` clamps an unknown id to the last row of a shorter table rather than refusing
-- so a replayed journal would come back as a room made of **meadow**, rendered without
complaint, which is the one failure mode indistinguishable from success. `--demo-glass`
escaped this only because glass was id 10 in both builds. **A vantage that introduces a
block id cannot be a `bitexact` row against any earlier build at all**, and its positive
claim has to come from a control inside one binary: for batch 59 that is `--no-light-rgb`,
as a `CHECKS` row and two `IMPLIES` rows rather than as a sweep.

The same trick is what lets `bench` price such a feature at all: a vantage's own arguments are
baked, so bench a *plain-camera* vantage with `--extra "--cam-... --load-edits PATH"`, which
both binaries can parse. **Pass an absolute path**: a spawned renderer does not resolve Git
Bash's `/tmp` the way the shell that launched it does, which cost this batch one run.

---

## Scale and screenshots

**`--screenshot` presents through an offscreen render target**, so it exercises the compute
passes, the blit and the HUD exactly as the window does.

**`--scale F` is not a general supersampler.** It works and is routed through `resize` so the
renderer derives its own mip bias, but the blit resolves with one bilinear tap -- an exact box
only at F = 2. Use `--reference` for a ground truth.

---

## `march`: the one reading heat cannot move (batch 53)

```bash
cargo run --release --bin harness -- march
cargo run --release --bin harness -- march --exe-b voxelcraft-pre53-noroot.exe
cargo run --release --bin harness -- march --only deep-water,sea-horizon
```

**Everything else in this fixture measures either a picture or a millisecond, and this measures
*work*.** It renders each vantage with `--march-stats`, which turns `FLAG_HEATMAP` on and reads
back the loop counter `march_chunk` has been accumulating into `dbg` since the heatmap existed.
The output is integers: DDA steps, their distribution across the frame, the pair demand against
the buffer's capacity, and how many pixels ran past `MAX_ITERS`.

**Why that matters more than it sounds.** `PERF.md` opens with three caveats and every one is
about milliseconds -- power state drifts absolute times by 1.8x, a heat-soaked session triples
the standard error, a stray game window inflates a pass threefold. **None of them can move an
integer.** Two `march` runs an hour apart on a machine doing anything else agree to the unit, so
a traversal change can be ranked **before** a bench exists and a bench that later disagrees is
the thing under suspicion.

**It adds no shader code, and that was the constraint.** The tidy version -- a per-workgroup
reduction and one atomic per 64 pixels -- would have meant giving `march` the `var<workgroup>`
and the barrier it does not have, and the pass being counted would not have been the pass that
ships. The flag's own `atomicAdd` is real work, so **a `--march-stats` run is not a timing
sample** and the mode says so where it prints.

**The capture it writes is the heatmap rather than the frame.** Look at them: they are what said
the cost is rays grazing water and not the terrain, at every vantage in the set. They are also
what the percentile row exists to contradict -- the bright band is 3 to 9.5% of the steps, and
**a picture of a distribution is not the distribution.**

**With `--exe-b` or `--extra-b` it prints the delta, and that pairing is the whole point for a
traversal change.** A pure optimisation is bit-exact by construction, which is exactly what
`lessons.md` calls *a no-op claim cannot be told apart from a not-wired-up claim* -- the reading
batch 51 shipped nothing over. Batch 53's two traversal halves are each quoted as **0 pixels at
n vantages** *beside* a step count that moved: -4.90% set-wide for the coalesced root test,
-25.17% at `deep-water` for `dry_mask`. Either number alone would have been worth nothing.

**Price a candidate with a deliberately incorrect build.** Batch 53 ranked three in four minutes
by patching one literal each to a value that is simply wrong -- a uniform-water leaf stepping 16
instead of 4, a missing root cell stepping 64 instead of 16, the coalesced sub-leaf test stepping
4 instead of 2 -- and counting. The pictures were nonsense and no one looked at them. Two of the
three turned into shipped code and the third was dropped on its ceiling alone. [`gpu.md`](gpu.md)
has the table.

**It is a subcommand and not a shell loop because of this file's oldest rule.** Batch 53
hand-typed four vantages' arguments while measuring exactly this, rendered four cameras nobody
had asked for, and read the difference as a defect in the feature under test. `vantage::select`
is the only thing in this tree that gets a vantage right.

