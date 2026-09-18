
# Roadmap

**What to do next, and nothing else.** Read once at the start of a session, after which it is
inert. What shipped is [`ledger.md`](ledger.md); what is known-wrong is
[`errors.md`](errors.md); the briefing is [`../CLAUDE.md`](../CLAUDE.md).

**Every entry here is open.** A shipped entry leaves this file entirely and becomes a ledger
row -- that is the rule this file was rewritten around after batch 47, when open work had
scattered across six *"what batch N left"* sections and the thing `CLAUDE.md` warns about had
happened: **a decision filed under a batch number is a decision nobody can find.** If you
finish something here, delete it here.

---

## If you read nothing else: the main sequence is complete, `resolve` is 80 registers, D1 is closed, and what is open is D2b and P11 onward

**Nothing left in this file is a defect anybody has seen in the picture.** D2b is a residual found
by sweeping rather than by playing. That is why the ranking below is the user's own and not a
queue of bugs.

**The user restated the ranking on 2026-09-15 and delegated the choice: *"i want good looks then
performance ... whatever you think."*** They restated it again later the same day: ***"i can
afford to lose performance"*** -- their only real performance concern is water, which they plan to
revert separately. **So cost is not the gate on a looks rung; visible effect is**, and
[`lessons.md`](lessons.md)'s MAE ladder is what ranks one before it is built. Batch 63 is what it
looks like when a rung passes every other test and nearly fails that one.

**P14 is the newest and is a mechanism rather than a fix**: `f16` packs two values into one 32-bit register on this driver, measured, which is the only idle resource on the scarcity table that reaches `resolve`'s register wall instead of routing around it. What is unmeasured is whether this shader has a step's worth of `f16`-safe live range in it.

**The cheapest thing in this file is [P11](#p11-the-gathers-gate-costs-more-than-the-work-it-skips)**,
and it is a decision rather than an investigation: 0.351 ms of `resolve` at `cave` sitting in a
gate that is measured, closed and reproducible in three builds.

**Two ceilings from R5's session that belong to no entry**, both at `default` against a real revert
binary, both recorded in [`ADR 0002`](adr/0002-no-disc-sampled-sun.md):

- **The whole sun shadow march is 0.516 ms +/- 0.050** of a 6.5 ms `resolve`. That bounds anything
  aimed at the sun's visibility term.
- **Shortening it from 220 blocks to 24 saves only 0.077 +/- 0.039** -- **85% of the ray's cost is
  its first 24 blocks**, so truncation is not the lever here that it is elsewhere in this tree. It
  costs 1,080 px at `default` and **14,348 at `canopy`**, where trees stop casting.

**And one reading from batch 64 that closes rather than opens.** `open-sea` moves **1 pixel at max
delta 1** under `--no-offscreen-shadows`, which is D1's exact signature and is not D1: A-against-A
at that vantage came back clean **8 times of 8**, and the A/B reproduced the single pixel **3 times
of 3**. A pixel that cannot be told from a known intermittent fault by its *value* can be told from
it in eleven commands.

---

## Where a closed designation went

**Every entry in this file is open, so a reference to a closed one resolves outside it.** Six
documents and most ledger rows name work by its roadmap letter, and `CLAUDE.md` deliberately does
*not* rewrite those names when an entry ships -- falsifying six documents to tidy a name is how a
claim comes to exist in two versions. **This table is the one hop instead**, and it is the only
thing in this file that is about finished work.

| designation | where it is now |
|---|---|
| **L1** = **R1**, directional visibility | shipped, batch 57 -- [`ledger`](ledger.md), [`shading`](shading.md). The letter changed in the batch-56 reorganisation, which is why six documents still say *L1* |
| **R2**, one bounce | shipped, batch 58 -- [`ledger`](ledger.md), [`shading`](shading.md) |
| **R2b**, the elevation form factor | built, measured, **rejected** -- [`ADR 0001`](adr/0001-no-elevation-form-factor.md). In no commit and no ledger row |
| **R3**, the ground's own exposure | shipped, batch 60 -- [`ledger`](ledger.md). It **closed the ambient floor** as an avenue |
| **R4**, emissive content | shipped, batch 59 -- [`ledger`](ledger.md), [`shading`](shading.md) |
| **R5**, soft sun from the same field | built, measured, **rejected** -- [`ADR 0002`](adr/0002-no-disc-sampled-sun.md). In no commit and no ledger row |
| **R6**, specular beyond water and glass | shipped, batch 65 -- [`ledger`](ledger.md), [`shading`](shading.md) |
| **R7**, the ambient's colour | shipped, batch 63 -- [`ledger`](ledger.md). It **closed the ambient** as an avenue, for the same reason R3 closed the floor |
| **D2**, the LOD shadow pop | shipped, batch 62. The residual is **D2b**, below |
| **D3**, offscreen shadows | shipped, batch 64 -- [`ledger`](ledger.md), [`lod`](lod.md) |
| **D4**, `rg_speckle`'s failing row | fixed on 2026-09-15, commit `c4b90eb` -- [`harness.md`](harness.md) carries the rule under `MetricCheck::both` |
| **P2**, truncating the primary shadow march | **ruled out**, batch 42 -- [`errors.md`](errors.md). Land is not water: 97% of the ray's cost is inside its first 64 blocks |
| **P3b**, a power state explaining one pass moving | **ruled out**, batch 49 -- [`errors.md`](errors.md), killed twice at `underwater` |
| **P4**, `march`'s share of the fixture | shipped across batches 48, 52 and 53. **The half still open is P9**, below |
| **P8**, `resolve`'s register ceiling | shipped, batch 67 -- 96 to **80** |
| **P10**, a root-`FULL` chunk opaque to a water-ignoring ray | shipped, batch 72 -- [`ledger`](ledger.md). The mixed full chunk now marches as if its tree existed; `--no-full-march` is the control |
| **P13**, batch 65's occupancy bill | shipped, batch 66 -- 128 to 96 |
| **P5b**, the probe apron | **answered by construction** at batch 57 and never billed -- four rungs in a row rode a field that does not know which chunk is asking. The first entry that needs real voxel geometry pays it |
| **P-A**, the `f16` register park | **measured zero, closed** -- commits `f620c7e` through `4e48d71`, built and un-shipped in full. The designation never existed as an entry in this file (batch-93's steward finding); the question it answered **does** and is P14 above, where the close-out numbers are now filed |
| **P-D**, the probe-bake cache | **spent** at commit `d4701a3` -- the bake is a pure function of world position (it marches `WorldGen::height`, never the voxels), so the window-scroll cache a chat queue planned against has no invalidation to spend itself on. Same steward finding: the designation never existed here |
| **P-B**, and every further chat-queue letter | **burnt letters.** Their claims were never written into this tree, so there is nothing to point at -- and the rule that produced this row stands below: a designation must not be used in a status report before it is an entry. *Do not reuse `P-B` for something else*: a letter with two histories in chat resolves to neither |

| **D1**, the pixel that flipped at `lattice` once in fifteen runs | shipped, batch 71 -- [`ledger`](ledger.md), [`shading`](shading.md). The mechanism was the probe lattice's own toroidal alias meeting the unload grace at teleport distance; the fix is an upload window derived from the alias distance, not a sort. The entry's two killed suspects and its methodology lesson stay in the ledger row |

**What every rung had in common is the admission test for a new one**: it replaces a *constant*
with something *derived*, and it pays for the derivation somewhere other than `resolve`. **Both
halves are now known to be insufficient on their own** -- R4 passed the first and had nowhere
off-frame to go, and R3 and R7 passed both and were capped by how little of the frame's light they
could reach. [`lessons.md`](lessons.md) carries all three questions and the order to ask them in.

---

## D2b. Two of the four LOD ring crossings still pop -- and the cheap test did not close it

**This entry's own first line was run on 2026-09-15 and the spikes survive it.** Re-running batch
62's sweep with the temporal pass left on does **not** absorb them, so they are not substantially
the dissolve's dither pattern. It is a real batch.

**Read the MAE column and not the pixel column.** TAA puts a floor of 35,000 to 70,000 differing
pixels under *every* step in the sweep, and the count stops discriminating entirely -- batch 63's
lesson arriving in the instrument rather than in a result.

| step | `--no-taa` px | `--no-taa` MAE | TAA px | TAA MAE |
|---|---|---|---|---|
| 1.950->1.975 | 42,694 | **0.3742** | 96,896 | **0.3786** |
| 2.025->2.050 | 40,672 | **0.4127** | 79,366 | **0.4133** |
| 2.075->2.100 | 16,058 | 0.1613 | 41,436 | 0.1583 |
| every other step | 4,197-9,133 | 0.048-0.140 | 34,647-71,737 | 0.049-0.144 |

The two spikes move **+0.0044 and +0.0006** with the temporal pass on, the same increment every
non-spike step also shows. The dither hypothesis predicted they would fall toward the baseline;
they do not move at all.

**The mechanism says this test was decisive rather than merely suggestive.** `dither` advances its
pattern by `5.588238 * frame.frame_index` (`common.wgsl`), so it is *temporally* varying, and a
32-frame `--taa-frames` capture averages it into a genuine blend rather than a stochastic split.
The dither is therefore not merely softened in the TAA arm, it is **gone** -- and the spikes are
unchanged. Had they been the dither they would have collapsed to the baseline, not held to four
decimals.

**The `--no-taa` arm reproduces batch 62's table to within 200 pixels** -- 42,694 against 42,718,
40,672 against 40,868, 4,716 against 4,723, 4,887 against 4,882 -- so the instrument is sound and
the sweep is worth re-running rather than re-deriving. The command is one loop over
`--streaming-factor` at `lod`, 13 captures per arm.

**Two things the re-run adds that batch 62's table did not have.**

- **A third crossing at 2.075->2.100**, MAE 0.1613 against a 0.05 baseline -- three times the
  floor, a quarter of the two named spikes, and absent from batch 62's table entirely. Whether it
  is the same defect smaller or a different one is unasked.
- **Rank these against their neighbours and never against the by-eye line.** 0.41 MAE is far under
  [`ADR 0001`](adr/0001-no-elevation-form-factor.md)'s 2.87 and batch 63 shipped nothing below
  3.04 -- but those are judgements about a *still*, and `CLAUDE.md`'s own correction after batch
  64 is that an MAE cannot rank a pop. What makes these two visible is that they are **8x the step
  either side of them**, not what they measure absolutely.

**`lod` has no crops**, and that is the gap the next session here will feel first: batch 64's
lesson is that a crop MAE ranks a localised defect where a whole-frame number under-ranks it, and
every figure above is whole-frame.

**D2 shipped as batch 62** -- [`ledger.md`](ledger.md) has it -- and closed half the problem.
What is left is measured and narrow.

Batch 62's instrument is the one to reuse and is the entry's most valuable part: sweep
`--streaming-factor` in 0.025 steps at `lod` with `--no-taa`, which walks the LOD ring across a
fixed scene exactly as camera motion does, and diff **consecutive** captures. A pop is a spike in
one step. (`--cam-dolly` against a static capture does **not** work -- both arms read 1.1176 MAE,
because a moving capture's residual is dominated by disocclusion and the pop is buried in it.)

| ring crossing | before batch 62 | after |
|---|---|---|
| factor 1.975 | 42,726 px | 42,718 -- **unchanged** |
| factor 2.050 | 38,437 px | 40,868 -- **unchanged** |
| factor 2.150 | 45,174 px | **4,723** |
| factor 2.175 | 35,753 px | **4,882** |

**The two that remain are where the *surface* changes representation, not just its shadow**, so
no ordering of a grid that holds one chunk per cell can reach them. The candidates, unranked and
unmeasured:

- ~~The dissolve is per ray and `--no-taa` shows it raw.~~ **Ruled out on 2026-09-15** by the
  table at the top of this entry: with the temporal pass on, both spikes hold their MAE to within
  0.0044. This was the cheap exit and it is closed.
- **So the question is whether `fade`'s *schedule* is too fast at the band edges** -- `fade_band`
  is 1.25 -- rather than whether anything is wrong with the mechanism. This is now the only
  surviving candidate in the entry, and it is unmeasured.

**Do not reach for a two-entry grid.** Batch 62's own comment is the argument: dithering the
secondary rays too would add noise the temporal pass has no motion vector for, and the measured
win came from an ordering that costs nothing and is *faster*.

---

## The goal

**Ranked by the user on 2026-09-14 and restated by them after batch 56, in their words: a**
***path-tracing-esque appearance with maximal performance***, **reached by spending the resources
this machine is not using, and the first of those is the lighting engine.**

**This file was reorganised around that sentence rather than merely re-sorted**, because the old
three-goal split -- performance, then appearance, then the game -- put the lighting rebuild in the
*second* goal and therefore behind a performance queue that was supposed to fund it. **Batches 55
and 56 destroyed that ordering**, and the honest thing is to say so at the top rather than to
quietly move entries: the lighting rebuild does not *need* funding, because its read side is net
**negative** cost. It pays for itself and then some.

Nothing here was deleted in the reorganisation. **Performance did not stop mattering; it stopped
being a prerequisite.** What follows is one main sequence and three supporting lists.

**Restated again on 2026-09-16, the day batch 94 banked the queue head** (P12's GO; the
large-water complaint of September did not reproduce at 120 fps in play): ***"i want my game to
lean with that look"*** -- the look being two modded-client frames filed at
[`look-reference-river.png`](look-reference-river.png) and
[`look-reference-bank.png`](look-reference-bank.png). The contract was asked and answered the
same day: **look first, performance reclaimed after** -- no floor holds while arms land, but
every arm still carries its measured number, so the reclaim pass knows what each cast cost --
and textures may become *richer procedural* (more atlas slices, species-level variation; still
generated in `textures.rs`, because CLAUDE.md's asset rule stands). Those frames are four
things in this engine's vocabulary: dense ground cover, depth-tinted water with a shore band,
a sky with real cumulus, and a warm grade -- which is why the appearance list grew the
G-series filed below, and why this sentence sits here rather than the file being quietly
re-sorted a second time.

---

## Why a path-traced look is reachable here, which is the whole strategy

**Per-pixel path tracing is not, and the numbers ruling it out are ours.** Three research
documents in [`research-review.md`](research-review.md) price diffuse per-pixel GI at **9.42 to
18.62 ms** on this card, against a `resolve` that is 44-81% of a 6-10 ms frame and is the single
saturated resource in the machine. **All three are right and all three answered a question
nobody should have asked**, because all three prompts described a *frame* budget.

**The world is static.** Geometry and emitters change only when a player edits, chunks are
already rebuilt one at a time on **sixteen rayon threads that are idle once the world is
resident**, and worldgen is **7.5 ms per chunk there against 84 us on the main thread**. So the
route to the look is not to trace faster. It is:

> **Move every watt of light transport off the frame, precompute it where nothing is watching,
> and read it back for less than the term it replaces.**

**That last clause is not aspirational; it is measured.** `light.rs` is *already* a precomputed
transfer function -- the sky nibble stores how much sky a cell sees and `frame.daylight`
multiplies at read time, which is PRT with a one-coefficient, direction-free basis. Batch 55
priced what a richer field costs to read and found **the cost does not scale with the coefficient
count**; batch 56 priced what it lets `resolve` delete and found the exchange is **net +0.03 to
+0.13 ms**, at 96 registers and 7,040 fewer bytes. **A directional coloured field is faster than
the two constants it replaces.**

**The corollary, and it is the rule this file now ranks by:** an appearance idea is ranked first
by *which resource pays for it*, and only then in milliseconds. `CLAUDE.md`'s scarcity table is
the argument; the short version is that GPU shader ALU is the wall and **VRAM at 97% free,
sixteen CPU threads, 64 tensor cores and 16 RT cores are not**. Work moved to chunk build is
paid out of a resource nothing is using. Work added to `resolve` is paid out of the only one
that is full.

**Two capabilities stopped being assumptions in batch 55.** `--bin probe` now asks, and on this
adapter `EXPERIMENTAL_RAY_QUERY`, `EXPERIMENTAL_RAY_HIT_VERTEX_RETURN`,
`EXPERIMENTAL_RAY_TRACING_PIPELINES` and **`EXPERIMENTAL_COOPERATIVE_MATRIX` all report true**.
The adapter reporting a feature is not naga being able to express it, which nobody has checked --
but the RT cores are reachable, and **the case for them is a bake and not a frame**: `tile_select`
already broad-phases the frame's coherent rays for 0.13-0.17 ms, where a probe bake casts millions
of *incoherent* rays into *static* geometry and amortises its acceleration structure over a
chunk's whole residency.

---

# Performance

**Re-ranked rather than demoted at batch 56.** These were goal 1 when goal 1 was a prerequisite;
they stopped being one when the main sequence's read side turned out to pay for itself. What they
are instead is the budget the appearance work below will spend.

**P11 first if any of them, because it is the one that is finished** -- what is left on it is a
decision rather than an investigation. **P9 is the largest number.**

### P11. The gather's gate costs more than the work it skips

**Answered on 2026-09-15, and it was batch 59's "arm that does less work and is slower".** The
entry named its own cheap test -- *a build where `any_blk` is forced to a literal `0u` and one
where it is forced to `1u`, two numbers, a wrong picture thrown away* -- and the test worked
first time. **The picture was not even wrong**: at `cave` every block channel is zero, so both
diagnostics render a frame **bit-exact with the shipping build**, and the A/B is a pure timing
difference over a provably identical image.

Three paired benches, `cave`, `resolve`, hi-z on, 8 alternated rounds each, delta is `B - A`
with A the shipping build:

| B side | delta | 1 se | t | rounds B > A |
|---|---|---|---|---|
| `--no-light-rgb`, the control, as calibration | **-0.172** | 0.015 | -11.71 | 0/8 |
| `any_blk` forced literal `0u` | **-0.351** | 0.011 | -30.79 | 0/8 |
| `any_blk` forced literal `1u` | **+0.327** | 0.014 | +23.00 | 8/8 |

**The gate costs 0.351 ms and the work it skips is worth 0.678.** Those two numbers are one
reading, not two: `blk1 - blk0` is 0.678, and `0.678 - 0.351 = 0.327`, which is exactly what
`blk1` measured against the shipping build. The arithmetic closes, which is the check that the
three builds are measuring one thing.

**So the paradox dissolves in the right direction.** The control arm is not faster because it
does less -- it does strictly more, and the entry was right that it does. It is faster because
**it has no data-dependent branch in it at all**. The shipping arm pays 0.351 ms for a gate that
saves it 0.678, nets +0.327 against always working, and still loses to an arm that never asks
the question. At `cave` the cheapest build of the four is the one that is bit-exact with
shipping and cannot skip anything: **`blk0` at 2.422 ms against the control's 2.509 and
shipping's 2.774.**

**And the entry's own eliminations survive the third build.** `shaderstats` reads **96 registers
and 15,872 bytes of shared memory on all three**, so it is not occupancy and not register
pressure here either. Code size is again anti-correlated with speed: `resolve` is 587,648 bytes
on the fast `blk0`, 586,624 shipping, 586,112 on the slow `blk1`.

**What is left unexplained is narrower than what this entry started with, and it is a question
about the hardware rather than about this tree**: a *uniform-false* branch, taken identically by
every lane in every workgroup of the frame, costs 0.351 ms at the same occupancy. Latency
hiding across the neighbourhood load is the standing hypothesis and nothing here tests it.

**What to do about it is a real decision and it is not obvious**, which is why this is a P entry
and not a fix:

- A world with no emitter in the neighbourhood is **eighteen of nineteen vantages**, so the gate
  is paying 0.351 to win 0.678 almost nowhere.
- But the vantage it exists for is `lamps`, and there the ungated cost is the +0.835 ms
  [`PERF.md`](../PERF.md) already records for an ungated three-channel gather.
- **The third option is the one nobody has priced: a gate that is not data-dependent.** A
  `frame.flags` bit, or a specialization constant, set from whether the *world* has any emitter
  resident -- batch 51's rejected chunk-flag gate is not this, because it was per chunk and
  fired on the chunk holding the lit surface above a cave. Per *frame* it would be uniform, and
  a `SPEC_MASK` override costs nothing and does not grow `GpuFrame`, which is the shape batches
  41 and 43 already used.

**One thing has changed under this entry since it was written.** Batch 65 added a second
specialization word, `SPEC_HI_MASK`, so the third option above is cheaper than it was: a bit no
longer has to be found, and `frame.flags` being full at bit 31 does not block it. The measurement
stands as taken.

**Hold every future gather to this number.** Batch 59's lesson was *on this pass occupancy is worth
more than four `light_curve` calls*; this one is sharper and points the other way about branches --
**a gate around a gather on this pass costs about a third of a millisecond before it saves
anything.** Anything that writes a new gather should not add a second data-dependent one without
measuring it the same way, which is three builds and twelve minutes.

### P9. The dry mask one level down, which is the half of P4's biggest number that is still there

**Batch 53 took a quarter of a ceiling it measured at 68%, and the gap is one level of the
tree.** `dry_mask` is per *chunk*: 64 bits, one per 16^3 root cell, set when that cell holds
anything a water-ignoring ray could stop at. A cell that is **entirely** water falls out of the
traversal and costs one step, or one step per 32^3 when its neighbours are water too. A cell
that holds any dry voxel at all -- one corner of sea floor, one boulder -- stays in, and the ray
walks its water at `skip = 4` with a `bricks[]` load per step exactly as before.

**The numbers, all from `harness march` and all integers:**

| | `deep-water` | `sea-horizon` |
|---|---|---|
| ceiling (a uniform-water **leaf** skipping 16, incorrect) | **-68%** | **-57%** |
| shipped (`dry_mask`, root level, exact) | -25.2% | -16.6% |

The root-level coalesced skip that shipped beside it is worth 0.00% and -0.04% at these two
vantages, so the shipped row is this mask's alone rather than the pair's. In milliseconds the pair is
**-1.337 +/- 0.049 at `deep-water`** and **-0.497 at `coastline`**, the second of those entirely
`resolve`'s -- so whatever P9 recovers, **price it on `resolve`'s row as well as `march`'s**,
because `resolve` calls `march_chunk` through three legs that all ignore water.

**What the remaining two thirds needs is the same mask on `Inner`, and there is nowhere to put
it.** An `Inner` is 16 bytes -- `mask: u64`, `run_ptr: u32`, `leaf_prefix: u32` -- and a per-node
dry mask is another 64 bits. Widening it to 24 costs every L1 node in the world half again as
much, and the runs are **interned**, so an attribute-derived bit would break the dedup as well.
Two things make that less bad than it sounds and they should be checked rather than assumed:
measured dedup on procedural terrain is **1.00x** (`PERF.md`, and *do not "fix" it*), so the
interning is buying nothing today; and `leaf_prefix` is a count bounded by 4096, so **20 of its
32 bits are free** -- not 64, but enough for a coarser summary.

**The cheap shape to try first is not a mask at all.** A 4-bit *y* bound per node -- "every
voxel above local y = k in this node is water" -- fits in `leaf_prefix`'s spare bits, costs
nothing to intern against, and is exactly right for an ocean, which is stratified. It cannot
describe a cave under a headland, and the ray that pays here is horizontal through open sea.

**Price it with `harness march` before building any of it.** The ceiling above took four
minutes and no GPU, by patching `skip = select(1, 4, ...)` to `select(1, 16, ...)` -- a
deliberately incorrect build, counted rather than timed. A *y*-bound variant can be priced the
same way before the format question is opened at all.
### P12. Half-resolution or checkerboard secondary rays

**The redesign `A2` says is needed, and the best idea the eighth research document had** --
[`research-review.md`](research-review.md) has the verdict on the rest of it.

Water's two secondary legs are **8.2 ms of a 10 ms `resolve`** on a water-filled frame
(`--no-water-reflect` -4.178, `--no-water-refract` -4.031 at `terraces`), and every *constant*
lever on that has been measured and killed. Tracing them at half resolution or on alternating
checkerboard pixels and reconstructing with a guided cross-bilateral upsample -- the primary
surface's depth and normal as edge-stopping weights -- removes 50% to 75% of that traversal and
of the secondary `shade_hit` shading behind it.

**Verdict (2026-09-16, user's hardware, RTX 3050, Vulkan): GO.** Half-res legs, built in the
sibling fork and benched there against a converged 256-sample `--reference`: water-crop MAE
**0.75/0.25/0.09** (full-res control 0.28/0.08/0.07), worst half-vs-full pixel **0.6784 MAE /
max delta 33 at `terraces`** against a by-eye bar of 2.87 -- one fifth of the bar. Speed:
**-4.603, -4.317, -4.647, -3.863 ms** at `terraces`, `shore`, `coastline`, `open-sea`
respectively (all t >= 130), roughly **90% of the fork's entire per-vantage advantage**, with
registers unmoved (`resolve` 80, `water_sec` 72, `march` 72). The 8.2 ms figure in the opening
paragraph is superseded by this measurement: the legs re-read **5.0-6.8 ms at 720p** (66-73%
of `resolve`, ~55% of the frame) -- the ceiling is 6.8 ms and the half-res recovery band is
2.5-5.1 ms. September's standing complaint (large water = expensive case) **did not
reproduce** at 120 fps in play. The far-lever warning below stands as written; the fraction
was picked by measurement (the sibling fork's own precision experiment, worst crop +0.465 MAE
at `terraces`), not by 0.5 looking natural.

**One fraction further, measured and pre-verdict (2026-09-16).** `--water-sec-scale 4`
(quarter-res) against the shipped 2, same harness, frame total: **-1.440 +-0.031 ms at
`terraces`** (6.310 vs 7.750) and **-1.284 +-0.041 ms at `coastline`** (5.526 vs 6.810) --
~19% of the frame at water, on top of this entry's own take. Picture, water crop, quarter vs
half: MAE 1.26/0.42/0.04 at `terraces`/`shore`/`open-sea`, max delta 33/20/8 -- under the 2.87
bar everywhere, but `terraces` at 1.26 is the first number in this sweep that sits in the
bar's neighbourhood, and stacked on half-res's own 0.68 the eye is buying roughly 1.9 total
against the converged reference. **Whether 4 is the entry's fraction is an eye verdict, not a
metric**: fly `2` and `4` at a shore view on both trees; that one answer buys or forecloses
the ~1.4 ms, and the by-eye loop is the user's, so this holds at measured-not-decided until
then.

**The objection to test first, because it is the one the recommendation did not raise.** Wave
normals are high frequency by construction: `--no-wave-aniso` is worth **15.9x at the horizon**,
and eight octaves subdivide the band on purpose. A half-res trace eats exactly that detail, and
the glint lobe is where the variance already goes. **So rank this on `open-sea` and `shore` as
well as `terraces`** -- the two vantages built around distant and shallow water respectively --
and use `--reference` rather than a pair, since what is at risk is convergence and not a shift.

**`--no-water-far`'s history is the warning about the constant.** What a reach is worth is a
property of the build and not of the flag: that control's own cost *tripled* when its constant
moved. Pick the fraction by measurement, not by 0.5 looking natural.

### P14. `f16` packs on this driver, and it is the only idle resource that reaches the register wall

**Measured on 2026-09-15, and it is a mechanism with an unmeasured opportunity rather than a
fix.** Batches 65 to 67 established that `resolve` is bound by its register count and not by its
arithmetic -- 16 registers cost 0.15 ms at `cave` and 32 cost 0.44, the same number for two
unrelated features that drew nothing. Every other row of `CLAUDE.md`'s scarcity table moves
*work* off the pass. **`f16` is the only one that shrinks what the pass already holds**, because
two of them pack into one 32-bit register.

**The adapter and the toolchain both allow it, and neither had ever been asked.** `probe` now
reports `SHADER_F16`, `SUBGROUP`, `SUBGROUP_BARRIER` and `SUBGROUP_VERTEX` all **true**,
subgroup size **min 32 max 32**, and `max_compute_workgroup_storage_size` **49152** -- and it
compiles a shader for the two that matter rather than printing a bit, because *reports the
feature* and *naga can lower it* are two different questions -- a distinction `CLAUDE.md` had
left open for cooperative matrix since batch 55 and which the same session then closed for all
four capabilities. Both compile; see the closing paragraph.

**The test is synthetic pressure at a real live range.** A chain of `vec4`s born beside
`spec_pre` and consumed beside `spec_rgb` -- so it crosses the nine-cell gather, the probe tap
and the shadow march, which is the span batch 67 identified -- with the two arms structurally
identical and differing only in type. `shaderstats --source` reads the register count without
rebuilding the game. `resolve` baseline is **80**.

| live scalars across the gather | `f32` | `f16` |
|---|---|---|
| 16 | 96 | 96 |
| **32** | **128** | **96** |
| 48 | 128 | 128 |

**At 32 scalars `f16` saves 32 registers -- two occupancy steps**, which batch 66 priced at
0.44 ms at `cave`. The SPIR-V was checked rather than assumed: the `f16` arm carries
`OpTypeFloat 16` and `OpCapability Float16` with **2** `OpFConvert`, the `f32` arm carries
neither.

**Four things the sweep settles, and the second is the transferable one:**

- **It is storage that packs, not arithmetic.** The first arm chained `sin()` and **both sides
  read 128** -- a transcendental widens to `f32`, so every value feeding one is `f32` in a
  register however it is declared. The lever is what a value *is while it waits*, which is batch
  67's *ask where a value dies* pointing at a type instead of at a position.
- **A cheap chain measures nothing.** A pure multiply-add arm read **80 at every width up to 48
  scalars**, because the allocator rematerialised it past the gather rather than holding it. A
  pressure test has to make recomputation expensive -- one independent `sin` per vector did it --
  or it is inert and reads as a clean negative.
- **It is a step function and lands on the wrong side as easily as the right one.** The 48-scalar
  row is 128 on both arms. Batch 67's *a register fix is worth a whole step or it is worth
  nothing*, confirmed from the other direction.
- **Shared memory is not a resource here.** The driver reports **15,872 bytes** on `resolve`,
  which declares no `var<workgroup>` at all, and the figure swung between 2,560 and 14,848
  across these arms with no relation to anything the shader asked for. It is driver-derived.
  **Do not plan against that column**; an earlier draft of this entry called it free scratch.

**What is open is the survey, and it is the whole of the remaining work.** Nothing here shows
that `resolve` *has* 32 scalars of `f16`-safe live range to convert. `spec_pre` is the one
`vec3` known to cross the gather and is **3 scalars**, far under a step and therefore worth
nothing on its own. What a batch has to do first is enumerate what is live across that span and
which of it tolerates `f16`'s ~3 decimal digits -- colours, normals, weights and occlusion do;
world positions, `dist` and anything feeding the visibility key do not. **Price that survey
before writing any `f16`**, the way P9's ceiling was priced with a deliberately incorrect build:
convert a candidate set crudely, read `shaderstats`, throw the picture away.

**The survey has since been run and the answer is closed: no, not where it tried.** P-A --
commits `f620c7e` through `4e48d71`, built and then un-shipped in full exactly as this
paragraph prescribes -- enumerated the span and converted crude: **11 `f16`-tolerant scalars**
live across the gather, short of the 32-scalar lever the table prices, and the step table said
what happens at that size before the build did: `resolve` reads **80 with the parks folded in
and 80 with them folded out**, bit-exact at 6 of 6 vantages, bench inside error at all three
(-0.006/+0.007/-0.005 ms). That is the *never fires* signature, not a failed idea, and it is
the entry's own warning arriving on schedule: storage packs, arithmetic does not, and this
shader's live range is rematerialised or `f32`-bound. **What does *not* close is the
mechanism question the close-out commit owed**: *why* the remaining live range is
rematerialised or held `f32` -- a register-allocation read made of shaderstats beside the
disassembly, owed by anything that parks values across the gather again.

**And the rest of the table moved too, in reachability and not in value.** Subgroups are
confirmed at a fixed 32 lanes and `resolve` at 16x8 is exactly four of them with no partial
warp -- but `errors.md` already caps the obvious use, sharing the gather, at **10-13%**.
**Both of `CLAUDE.md`'s two "untouched" rows were taken as far as they can go without a
design**: naga's WGSL front end expresses ray query *and* cooperative matrix, and wgpu 30
built a real **AABB** BLAS with a TLAS instance over it and submitted it on this driver.
AABBs are the whole point -- there is no mesher here, so procedural primitives are the only
shape an RT core could accelerate for this engine, and that path is now known open rather
than assumed. **What did not change is the argument**: an RT core is still the right tool for
a *bake* and not for the frame, because `tile_select` already broad-phases the frame's
coherent rays for 0.13-0.17 ms; and every cooperative-matrix candidate anybody has named
runs per frame, which aims at the one resource that is full. **Reachability was the cheap
question and it is answered; the expensive one is whether there is a use, and nothing here
touches that.**

### What is already ruled out, so nobody re-plans it

Each of these has its number and its retry condition in [`errors.md`](errors.md). **Read it
before designing anything aimed at `resolve`.**

- **Every distance constant water's traced reflection has** -- the reach, the fade and the
  reflected hit's shadow budget. The first two are real money paid exactly where the look is;
  the third is free to look at and buys nothing.
- **Truncating the primary shadow march.** 97% of its cost is inside the first 64 blocks.
- **Folding the other eight switch gates** the way batch 45 folded its own. -0.119 ms at one
  vantage, inside its standard error at four.
- **Caching the gather per block face.** The premise is exact and the block is instruction-bound,
  so perfect sharing recovers 10-13%.
- **Occupancy, twice.** And code size, once, in the wrong direction.
- **Four more, all batch 53's and all aimed at `march` rather than `resolve`.** The dynamic
  vector index `cell[axis]` forcing the DDA into scratch -- 72 registers, no spill, no local
  memory, before and after, read off `shaderstats` for free. `MAX_ITERS` as a lever, at 19
  pixels of 921,600 and that an upper bound. Widening the coalesced sub-leaf test to a 4x4x4
  group, at -10% of one vantage's steps and no exact version to build. And letting the
  dark-water rung fire on a shallow camera pitched down, which is the one of the four that is
  a **live decision rather than a dead end** -- see Parked.

---

# Appearance: what the main sequence did not cover

**Appearance work that is not light transport.** Each is annotated where a shipped rung changed its
status. **The main sequence is complete, so nothing here is queued behind a rung any more** -- rank
these against each other, and against [`lessons.md`](lessons.md)'s MAE ladder before building
one.

### A1. The canopy lift

**Built at batch 90 behind `--canopy-lift F`, default off.** The fill in `ShaftField::
update` adds the authored lift on columns passing the tree placer's own three gates; at
0.0 the field is the pre-arm one bit for bit (pinned in `tests/batch90.rs`). **Hardware,
first pass: fires but sub-visible** -- lift 8 moves 21,682 pixels at `canopy` (max delta
24) for MAE 0.054 against the 0.372 "invisible" rung, so the mechanism works at too small
a dose; the lift sweep is the by-eye's next reading before this entry calls itself done.

**`errors.md` calls it the cheapest visible win left, and batch 37 doubled what it is worth.**
`ShaftField` fills from `WorldGen::height`, so a forested crest casts the shadow of its
*ground* and light slices straight through the trees standing on it. `coarse_trees` already
aggregates canopy height for the coarse LODs and the fill is one line from reading it.

**What makes it a batch rather than a line is that a canopy height is a look constant** -- how
far a stand of trees lifts the occluder has to be authored, asserted and priced like
`MEADOW_SHADE` was. And since batch 37 the envelope shades the **ground as well as the haze**,
so one fill line improves two readers at once: against an exact march the envelope currently
misses **559 pixels at `lod` and 207 at a mid camera**, unattributed between the canopy and the
4-block quantisation and not separable without doing this.

### A2. Water at scale -- the user's own report, and it spans both goals

> *"It looks divine on the shore, but where there are large bodies of water it breaks and looks
> like shit -- and it also tanks performance."*

**Recorded verbatim on 2026-09-14 because it is the one defect on this file that came from
playing the game rather than from a metric**, and because it is the only entry that is
simultaneously the largest performance cost in the engine and an appearance complaint.

**What the measurements already say about the second half.** Water is the most expensive pixel
kind by a factor of nine, and on a water-filled frame its two secondary legs are **8.2 ms of a
10 ms `resolve`** -- `--no-water-reflect` -4.178 and `--no-water-refract` -4.031 at `terraces`.
Every *cheap* lever on that has now been measured and killed, which means **a real improvement
here is a redesign and not a constant.**

**What is on file about the first half**, and it is more than it looks:

- **Distant water carries excess speckle that no band limit reaches** -- 1.43x a converged
  reference, and the headline is stale twice over. `wave_octave` spends the variance it removes
  in the glint lobe alone, while the Fresnel weight, `sky_color(mirror)` and the traced
  reflection all read a normal flattened without being widened. **`gloss.w.z` already carries
  the removed variance and three readers ignore it.** [`errors.md`](errors.md),
  [`water.md`](water.md).
- **Reflected and refracted content ghosts under fast motion**, because `taa` reprojects the
  *surface*. Every screen-space reflection has this; the fix needs a second motion vector and
  **there is nowhere in the visibility key to put one.**
- **The wave field is four octaves spanning the band, eight subdividing it since batch 25**, and
  the grazing footprint correction is worth 15.9x at the horizon -- both aimed at exactly the
  large-body case that is being complained about, which means the existing machinery is
  *trying* and falling short rather than absent.

**Scope this before taking it.** A full water rewrite is several batches, and the honest first
one is a *diagnosis* batch: render `open-sea`, `lattice` and the horizon at `--reference` and
say in numbers and pictures what "breaks" is -- speckle, the wave period reading as plaid, the
reflection's grazing behaviour, or the LOD of the sea surface itself. **Do not redesign before
that**, because three of the four candidates above already have failed fixes on record.

**Batch 52 added a fifth candidate to that list, deliberately.** `WATER_FAR_DIST` at 128 deletes
a distant shoreline's *underwater* silhouette -- **5,032 pixels at max delta 23** of the 67,621
it moves -- accepted against the picture for 1.930 ms. The seam sits at a fixed distance from
the *camera*, so it slides as the player swims, and **no still frame priced that**. If a
diagnosis batch finds "breaks" at the horizon, check this before redesigning anything: the
retreat values are 224 for 1,640 pixels at max delta 3, and 192 for a third of the money.
`ledger.md` row 52. **Batch 53 shipped the depth sub-lead and it did *not* retire this** --
the rung tests the eye's own depth, so at `sea-horizon`'s three blocks it never fires and batch
52's seam is exactly where it was. What batch 53 adds to a diagnosis batch is the camera: a
frame 24 blocks down is a different complaint from a frame three blocks down, and `deep-water`
is now in the set to tell them apart.

**Batch 54 changed a large part of what a *submerged* frame looks like, and a diagnosis batch
has to account for it.** The surface now exists from below: past the critical angle it is a
mirror, and the transmitted light fades with the eye's depth. So *"where there are large bodies
of water it breaks"* was made from a build that saw straight through the surface at every angle,
and half of what it described may no longer be there. **Re-take the complaint before redesigning
anything** -- and take it at more than one depth, since the ramp makes three blocks down and
twenty-four down genuinely different frames now.

### A2c. Inside Snell's window, the world above is not compressed

**Built at batch 91 behind `--snell-bend`, default off.** Two segments at dispatch: the
submerged primary path splits analytically at the sea plane and the transmitted half
re-marches refracted; `resolve` is untouched *by construction* (every window term it owns
reads `rd` and `dist`, and the bend preserves both as `t_surf + t2`). Stated approximation:
the bend facet is flat where the Fresnel weight is waved, a strictly-smaller error than what
ships, with the wave-field-to-`common.wgsl` migration as the named successor. The owed
measurements -- march-row bench at the submerged vantages, `march` register read against
the spec, and the acceptance-rule pixel threshold -- are laid out in [water.md](water.md).

**What batch 54 left, and it is architectural rather than a scope cut.** That batch put the
boundary back: outside the critical angle a submerged eye now sees a mirror, and inside it the
Fresnel curve. What it could not do is **bend** the transmitted ray. Physically the whole
hemisphere above the surface is squeezed into a 41.4-degree cone, so what is inside the window
should be a compressed image of the sky and the shoreline; here it is an undistorted one, seen
through a hole.

**Why `resolve` cannot fix it alone**: `march` has already traced a straight line and written its
hit into the visibility buffer, so the pass that shades can change *what the surface does* but not
*which geometry the ray found*. Bending at the boundary means the marcher has to know about the
boundary -- which is a second march segment for every submerged ray that crosses it, or a
refraction applied before the primary march is dispatched.

**Rank it honestly before taking it.** What is wrong now is a distortion and never a presence or
absence: outside the window the mirror takes weight 1 and the transmitted leg is not consulted at
all, so the error is confined to the cone and is largest at its rim. That is a much smaller thing
than what batch 54 fixed, and it costs a march-side change to get.

**The cheaper half that is not this**: `SNELL_DIM_MAX` is capped at 0.9 because
`underwater_body()` bottoms out at `frame.ambient` once the sky flood is zero, which is the 4-bit
light nibble rather than physics. **R2 shipped and did not lift it**: batch 58 multiplies
`floor_rgb` and `underwater_body()` bottoms out on `frame.ambient`, which is a different term.
**So the cap is R3's to lift** -- see
[`water.md`](water.md).

### A3. Shoreline foam and wet sand

**Added at the user's request, and unpriced.** Water currently meets land at a hard line. Real
shorelines have a wet-darkened band and foam where the two meet, and it would be one of the
most visible improvements available on a frame this project screenshots constantly.

**Structurally cheap, which is why it ranks above the harder appearance items**: the engine
already knows water depth at every refracted sample and sea level is a uniform, so a darkening
ramp on land within a block or two of the waterline is an albedo term and not a ray. Foam is a
second term on the same quantity.

**The trap to avoid is the one `--no-meadow` hit**: a compensation authored as an albedo cannot
know the view angle, and it has to be priced through the medium it will be seen through -- see
[`lessons.md`](lessons.md) under *Planning a look change*.

### A4. Glass's traced reflection

**Built at batch 90 behind `--glass-reflect`, default off.** `shade_glass`'s sky-only
`refl` is replaced by one `water_reflect_leg` march (the function keeps its name; the
recursion-legality note at the call site is why that is safe). On a miss the leg's `w`
is 0 and the old sky stands. **Hardware, first pass: the round's win** -- MAE **5.47** at
`glass` (148,496 px, max 146) for **+0.043 +/- 0.020 ms of `resolve`**, just under the
5.98 rung this project's shipped changes at, at the one vantage full of panes.

**A pane's Fresnel weight goes to the sky and never to a traced reflection**, so glass facing a
hillside shows sky where it should show hill, worst at grazing where the weight is largest.
Batch 8's own split -- refraction was 8b and the traced reflection 8c -- and **the shape of the
fix already exists as `--no-water-reflect`'s partner.** A batch, not a line.

Its neighbour, and cheaper: **glass is invisible to every secondary ray**, so a glass house
reflected in water shows the world behind its walls. That one is deliberate -- it is water's
own `skip_water` rule reaching a second material, and it is what caps depth complexity at two
interfaces with no counter anywhere. Changing it needs the counter.

### A5. 38c -- the canopy's LOD cliff

**Built at batch 91 behind `--tree-blue-noise`, default off.** The coarse proxies in
`coarse_trees` are thinned cell-by-cell where an R3 rank field falls under
`COARSE_CANOPY_KEEP == LEAF_FILL` -- mean transmittance matched to LOD 0 by value, the
mask still strictly binary, trunks untinned because LOD 0's are untinned. The -40% per
coarse ray and the pixel footprint batch 38 measured are now properties a bench can
read; what the sweep should prove first is the batch-90b acceptance rule, at `lod`.

A 4^3 cell inside a stride-2 voxel means nothing, so the cutout is LOD 0 only -- but
`coarse_trees` stamps `b.leaf` at every stride up to 8, so a porous near canopy meets a solid
far one across a cross-fade that **partitions rays rather than blending samples**.

The still-frame half is measured and bounded: **2,508 pixels of 921,600 at `lod`, max delta 16,
all inside one bbox in the near field.** What that does not cover is motion -- the TAA-clamp and
Hi-Z halves need a camera crossing the band and **no vantage in the fixture has one**, so this
batch builds its capture with it.

**The fix is generator-side and the research priced it cheaper than doing nothing**: decimate
the coarse occupancy mask with a 3D blue-noise rank field so its mean transmittance matches
LOD 0 while the coarse traversal stays strictly binary -- **-40% per coarse ray**, because a
thinned mask skips more. The alternative it rules out is a fractional-opacity scalar at +180%
to +250%.

### A9. Caustics, which this renderer has none of

**Built at batch 90 behind `--caustics`, default off -- and filed as DIAGNOSED, NOT
BUILT after the hardware pass** (the batch-90b acceptance rule in lessons.md: an arm
must move pixels at a named vantage past max delta 1 to count as built). The arm fires
(115 of 921,600 moved at `coastline`, max delta 1; zero at `terraces`/`shore`) yet reads
as nothing, and the diagnosis is form, not plumbing: the carrier is quartic in `wave_amp`
(product of two slope-squares, <=1.07% of direct sun at shipping defaults) **and** the
wave trains are the wrong scale for ribs regardless of form. The full arithmetic and the
next build's two owed decisions (an authored high-frequency caustic field AND the
curvature-focus form) are in [water.md](water.md). **Not to be tuned in place** -- a
gain sweep against a quartic carrier is the P-A failure shape a second time.

**`caustic` appears zero times in this tree**, and dancing light on a submerged bed is one of the
strongest cues a path tracer gives that water is being simulated rather than drawn. The cheap
form is not photon tracing: evaluate two overlapping animated 2D wave functions along the sun's
direction onto submerged surfaces and modulate by `exp(-depth * k)`, so deep water stays dark and
a shallow shore gets sharp ribbons.

**It is pure ALU in `shade_hit`**, which is the saturated resource, so it has to be priced rather
than assumed -- and it is gated by the same `sky_sm` and depth terms batch 53 and 60 already
compute, so it costs nothing where there is no sun and no water.

**The honest prior on the look is good and the honest prior on the *derivation* is not.** Nothing
here is derived from the wave field the surface actually uses, so a second animated function is a
second thing to keep in phase with `wave_octave` -- and `--anim-rate` is the control that would
expose them disagreeing. Prefer deriving the pattern from the existing wave normal if that can be
made to work, and say so in the batch if it cannot.

### A10. AgX or a modern neutral tone curve -- and it is a *retuning* batch, not a shader change

**The curve arm is built at batch 90 behind `--tone-map knee|aces`** (default `knee`,
the hyperbola every constant was tuned against). Implemented curve: Narkowicz's 2015
ACES fit, per-channel -- deliberately *not* AgX proper, whose per-channel matrix is the
follow-up this arm's reading decides. A runtime uniform at `BlitPass`, not a second
pipeline, per the entry's own rule. **Hardware, first pass: full-frame at every vantage
(all 921,600 px at `sky`, max delta 29) for zero frame cost** -- blit uniform, as the
entry prescribed. What remains is the eye verdict itself, weighed against the standing
note that the world already reads too dark (the ACES shoulder pulls highlights *down*),
and then the retuning batch, which stays open under the same name.

[`blit.wgsl`](../src/render/shaders/blit.wgsl) is the one place that tone-maps. It is **not** a
naive Reinhard, whatever a document says: it is a deliberate knee hyperbola, C1 at `KNEE = 0.75`,
asymptoting to 1.0 so the sun disc and glowstone roll off instead of clipping.

**And its comment is the entry.** *"Below the knee this is exactly the identity, so every constant
tuned against the linear image stays where it was put."* **AgX is identity nowhere.** Swapping it
in invalidates every constant in this engine authored by eye -- `floor_rgb`, `SKY_TINT`,
`PROBE_SUN_GAIN`, `AO_STRENGTH`, the biome tints, `LEAF_FILL`, the water mottle -- each of which
carries a comment explaining a number chosen against the *current* curve.

**So the cost is not the shader and quoting "0.00 ms on the 3D pipeline" is true and useless.**
The cost is a retune, every reference capture in the fixture moves at once, and `bitexact` stops
being able to say anything until the new baseline is captured. **Budget it as a batch whose
deliverable is a re-tuned constant set**, take it when the main sequence is quiet, and do not
take it in the same session as a lighting rung -- two things moving every pixel at once is the
one arrangement in which neither can be ranked.

### A8. The smaller ones, each already carrying its number

- **`shaft::TEXEL` has never been swept.** It ships at 4 blocks on an argument about worldgen
  slopes rather than on a measurement, and it is the knob between a blocky shadow edge and four
  times the fill. `--reference` is the tool. **Armed in batch 92** as `--shaft-texel N`: the
  shader read `frame.shaft_texel` from the start, so the arm is one struct field, three call
  sites and a clamp -- the sweep itself is the acceptance pass this batch only wires.
- **Edits do not cast a shaft.** A tower on a ridge gets a correct ground shadow and no shaft,
  because `ShaftField` fills from `WorldGen::height`. `journal::apply` already aggregates deltas
  per coarse voxel, which is the shape the fill would need. **No vantage in the fixture can see
  it**, so this one builds its capture with it. **Triaged in batch 92, not built**: it needs
  journal deltas joined into the field's fill *and* a capture built to see the result, so it
  is a feature with a fixture dependency, not a flag the family pattern covers.
- **Two of the six biomes take almost no tint and one takes none at all.** **Armed in batch
  92** as `--tint-balance`: sand and lichen gain masks at `build_atlas` time (content, not a
  pipeline key), and the plains row swaps for a balanced one through `SPEC_TINT_BALANCE`. The
  accepted spill -- every biome's beach sand takes its local tint -- is the by-eye pass's.
- **The meadow's three loose ends.** The compensation is an albedo and cannot know the view
  angle; only the top face is repainted, so a steep coarse slope shows bare grass on its sides
  (a `MEADOW_SIDE` layer is one atlas slice); and the residual is almost entirely the proxy
  trees. **Do not take all three in one session -- the third is its own batch.** **The middle
  one is armed in batch 92** as `--meadow-side`: one id, one slice, gates and coin shared, the
  other two left exactly as open as they were.
- **Ground cover stops at the LOD 0 boundary**, because a tuft is one voxel and cannot exist at
  stride 2. **Triaged in batch 92 as an accepted limit, not a flag**: the approximation the
  meadow compensates is acknowledged rather than armable -- what there is to paint at stride 2
  is a colour question for the by-eye pass, not a mechanism.
- **The envelope is a heightfield**, so an overhang casts a solid column of shadow and a
  mountain is solid from the inside. Both are right from outside and wrong from within.
  **Triaged in batch 92 as a redesign, not an arm**: batch 35's ledger row rejected the exact
  march as +10.57 ms at a frame that costs 4, and an arm that re-introduces it behind a flag
  would re-legislate that finding rather than answer it.

---

### G1. Sky, deck, horizon and grade -- the look goal's first face

**Built at batch 95, dark behind five doors, aimed at the two reference frames saved beside
this file.** What batch 94's stop-order closed under it: water is no longer the expensive case
(P12's GO is filed above), so the look goal opens upstream of the water, at the face the
references spend their vista on. The arms, all guarded-branch knobs defaulting to the
bit-exact pre-95 sky: `--cloud-patch` (cumulus arrive in *banks* -- a coarse tap at
`CLOUD_PATCH_SPAN`, 6x the deck's own span, swings the coverage threshold by up to the full
noise range; banks ride the deck's drift so geography stays one field), `--cloud-relief`
(sun-side cap-vs-belly shading on a flat plane -- *shading only*, `dens` is final before the
block runs and the batch-95 test pins the ordering), `--haze-warm` (the warm band over the
treeline, added in `sky_base` so the fog path picks it up for free), `--zenith-deep`
(multiply-only, weighted by the gradient's own exponent), and `--grade none|warm` (the
luma-preserving cast, one presentation pass later, on the blit selector's spent pad --
brightness is not what it moves, so no lighting constant re-tunes). **Acceptance, pre-written
per the batch-90 law**: each knob moves pixels past max delta 1 at a sky-bearing vantage or it
is not built; ladders 0.3/0.5/0.7 at `sky`, `default` and `terraces`, sun low to midday;
the shipped set is the by-eye pass's read of the references, not a metric's. **The cost is
bounded the batch-10 way**: deck work happens only on rays that reach the deck, the grade is
one selector word of presentation -- and the look-first contract means a real bill, if one
arrives, is *recorded and paid later*, not rejected unread.

**First hardware round (2026-09-16): builds, renders, moves pixels at all four knobs -- and
the control miss was real, then repaired.** The uniform-guard class (`if frame.knob > 0.0`)
moved seven of twenty-one control vantages 1-4 pixels at max delta 1: pure reassociation in
the arithmetic *around* branches the compiler cannot remove, batch 14's law one level down.
Batch 95d folds all four guards behind one `SPEC_SKY_LOOK` override (bit 1024, the tint
arm's class; the knobs stay uniforms past the fold), so the control claim is true by
construction again and the re-gate is owed. The arms at deliberately top-end settings, all
at once, against the pre-95 frame: MAE 18.41 at `sky`, 11.11 `canopy`, 8.88 `coastline`,
8.09 `low-sun`, 3.51 `default`, max delta 100-141, ~921k px everywhere -- two to three
times the 5.98 shipped rung, which is what a sweep's top end is *for*. The frames'
reading: `coastline` and `canopy` sit closest to the references, and the knobs doing the
vista work are `--haze-warm` and `--cloud-patch` -- the ladder sweep and the by-eye verdict
are next, and the honest note of the round stands: the references' other half is geometry
and texture density, which is G2's sentence and A5's open wound, not a sky problem.

**Second hardware round (2026-09-17), the five batch-97 arms by their measured numbers,
with the testing AI's verdict in place of the by-eye pass.** `--sky-cool`: **sweep** --
fires, but MAE 1.07 / max delta 9 is nearly invisible, so the arm needs reach (stronger
swap constants or a dial) before a ship verdict can be argued; a comment-mention posed as
a sixth swap site in its own census, repaired at 99c by counting select-site shapes
instead of the token (comments are text too). `--grade cine`: **sweep** -- MAE 9.54 /
max delta 19, works at the sunset view but darkens an already-dark MIDDAY hillside; the
fix is a **strength knob on the blit grade**, filed as G1's next small rung before this
grade can ship. **Knob landed (batch 101c's `--grade-strength`), and the batch-102
hardware round re-took the verdict on a land-filling frame: full cine crushes (MAE
9.1930, mid-tone separation flat against the standing too-dark judgement), half keeps
the base's brightness with the S-curve (MAE 4.3052) -- ship `--grade-strength 0.5`,
full cine does not; the dial was the right thing to build.** ALL-FIVE reads as cine +
water-look dominating (MAE 12.87), so no
interference finding -- the arms compose the way their classes predicted.

### G2. Ground cover: the density the references read as alive

**STATUS, second hardware round (2026-09-17): the species-albedo half is SCRAP-OR-REBUILD
-- both arms fired and neither is visible: `--foliage-rich` MAE 0.42 / max delta 18,
`--canopy-relief` MAE 0.18 / max delta 15.** The testing AI's read of its own lookbook
sheet is the finding this entry was built to buy: the gap against the references is
vegetation **density and shape**, not albedo, so the next batch is a **generator-side
rebuild** (denser/multiple-height grass stamps and reed stands on the coarse path), not
another shading dial -- and a shader-side albedo arm cannot rank these rebuilds either,
because the lookbook's MIDDAY view fills its frame with a near hillside while the
reference frames sit waist-height among the reeds. **The lookbook therefore needs a
waist-height-in-grass tile** before any foliage rebuild can be judged, filed with it.
The density half's other input stands discharged head-on: at batch 99b both the tests in the A5 set
are repaired in-tree (the leaf-hole predicate learned PINE_LEAVES; the white-noise
test's y pin moved off the one-voxel LOD-2 shell onto the rank field where 94 measured
it, with the shell bound pinned as the pixels' structural property) -- the plan's demand
that G2 first say what A5 got wrong now has its answer in code: two test-geography
assumptions (a species list that forgot the biome leaf id, and a property measured on
the wrong object), no generator defect left standing in A5's name.

**The biggest pixel share of both frames, and the one that walks straight into A5's open
wound.** Every near face in the references carries tall grass in more than one height and
tint, and the banks carry reed stands; this engine has tall-grass stamps, the meadow carpet
and A8's `meadow-side`, and at stride 2 the cover falls off the canopy's LOD cliff -- which
is A5's entry, and **A5's own tests said at batch 94 that A5 is not built**. Two of the
goal's sanctions apply here and nowhere above it: *richer procedural textures* (more slices
in the generated atlas, species-level variation) and no fps floor while it lands. Both come
with the batch-94b rule in their path -- every new atlas slice and block id is an append,
id and row kept one list, and an entry this size will spend both. **G2's plan must first
say what A5 got wrong, not just build past it**: a batch whose premise is a measured
population-recovery at the cliff is a different batch from "more stamps."

### G3. Water's next rung: depth colour and the shore band

**STATUS, second hardware round (2026-09-17): `--water-look` is the round's winner --
SHIP.** MAE 4.69 with max delta **156**, the biggest measured movement of the five arms,
and the testing AI's read of the lookbook tiles is *moves toward the references*, not
merely *moves*: murk graded by the traced column, the turquoise bank band, and the
metre-scale ripple all read as the A2/A3 cues rather than as a discoloring pass. Open
question carried forward, not answered silently: whether and when it becomes a default --
the flag stays dark pending the user's own eye, the ship verdict being about the look,
not about flipping the vanilla build. `--sky-cool` also touches this face (cornflower
midday) but its entry is G1's as the gradient's owner, and its verdict is sweep.
Second-round addendum from the fork's own shots: at aerial distances the metre-scale
ripple moires into concentric rings (visible in the medium/maxed sea and aerial
frames) -- the arm wants a **distance or pixel-footprint fade on the ripple term**
before the default-on question opens; the murk and bank band read clean at every
range shown. Batch-100 sheet zooms sharpen this: the **pre-existing** static wave
normals already alias into parallel banding at grazing range, and the ripple term's
period builds the new chevron rings on the same substrate -- so the footprint cure is
one fix with two beneficiaries, and "aerial-only" was wrong on both counts.

**A2 and A3 have owned these questions for months; this is the goal's re-read of them now
that the cost side is closed.** What the references show past what ships: absorption *graded
by depth* (murky green-brown at the bank, glass-clear further out) and a shore interaction
band (wet sand, a soft waterline, A3's foam lineage). With P12's GO the legs are the cheap
half of the frame at water and the quarter-res fraction sits filed awaiting its eye verdict
-- so G3's candidates are colour and content, the rungs this queue had been skipping while
it paid for the trace. **A9's measured zero stays out of the plan**: the references do not
lean on caustics either, and the no-op read and the reference agree, twice.

### G4. The canopy: density, dapple, and the lift that measured invisible

A1's canopy lift fired and moved almost nothing -- MAE 0.054 against its own 0.372 rung --
and the references' canopy is *density plus dapple*, so the sweep is two dials at once: the
lift ladder A1's entry already names, and the shaft field's texel knob A8 piece 1 unlatched
at batch 92 (dapple is an edge *and* an intensity; a low-sun canopy vantage is where both
read). The richer-texture sanction reaches here as species-level leaf variation. Ranked
behind G1 only because its by-eye pass needs the references' *season* pinned first -- the
frames read late-summer, and a canopy verdict taken against the wrong season is A1's
knob-sweep failure wearing a calendar.

### G5. The sun disc: the penumbra, gated -- second attempt

**STATUS: first design built, measured, and not shipped (batch 102a, third hardware
round, 2026-09-17).** The references' shadows are soft-edged *and* deeper; this engine's
are razor and, on the user's standing judgement, the world already reads too dark. 102a's
cone-tap penumbra answered the first half and failed the round on both axes: **+4.545 ms
against a +0.35 ms bar** -- the cost is the taps themselves, 1.3-1.8 ms apiece however
short their march (scaffolding 0.073 ms; the finding is written up in
[`lessons.md`](lessons.md)'s "a ray's bill") -- and **86% of moved pixels lighten**, a
shadow-lifter where a penumbra must split ~50/50. Local reach is real -- MAE 2.6480 over
its strongest 160x90 crop, against the 2.87 rung -- so the verdict is fix, not revert,
and ADR 0002's wall was this time met from the effect side.

The second attempt fires the cone only where an edge can be: **gate on the blocker
distance `shadow_ray_t` already returns for free** -- a blocker nearer than `1 / aper`
(~18 blocks at the shipping aperture) cannot cast a fringe wider than a voxel, and those
near-field pixels are exactly what the first build lifted. The detector is arithmetic,
not a ray, which is the round's whole lesson. **Acceptance, both ends: cost inside +0.35
ms at `--grass-dense` 1920x1080 paired-and-alternated, AND a sign split near 50/50 with
mean near zero over the moved set; a gate that makes the arm cheap by making it
invisible fails the second end.** The fallback worth pricing first if the gate misses is
*contact darkening, not penumbra* -- deeper shadows bought out of the probe field's idle
resource (L5's neighbour in the scarcity table) rather than softer ones bought out of
the resource that is full. A screen-space filter stays unpriced at the stage level:
`build_hiz` already reads per-tile depth and the visibility buffer holds 24-bit depth
and a normal, and nobody has yet asked what those already hold.

---

# The game, still last

**Explicitly last as of 2026-09-14, and unchanged by the batch-56 reorganisation.** It was the stated goal from batch 29 and shipped its
file-format half in batches 29-34 -- the edit journal, edits at a distance, the plan and the
camera that reads it, the player in the header, a world on disk, and the bar in the file. All
six are ledger rows; [`persistence.md`](persistence.md) is the subsystem.

- [ ] **Stacks, and whether they mean survival.** A count per slot, placing that decrements,
  breaking that credits, and **an empty slot that cannot place** -- that last clause is the
  whole weight of it, because it would be the first thing in this engine to make an edit
  *refusable*, so it reaches `set_block`'s callers where neither batch 32 nor 34 did. Drop rules
  per block type are a third question and do not have to be answered with it. The bar block has
  two spare `u16` and a count per slot needs ten, so the format work is a third block.
- [ ] **Entities, and the wall in front of them.** **Know this before designing anything that
  wants a third shape**: a mob needs a new primitive in the visibility key, `normal:3` has been
  full since batch 14 took states 6 and 7 for the cross-quad, and batch 38 spent the voxel
  field's six free bits on the leaf micro-cell. **There is nowhere left to put one** without
  widening the key or taking depth precision. Persistence, inventory and survival do not touch
  this; mobs do.

---

---

## The four idle resources, examined 2026-09-16

**The scarcity table's four untouched rows and one runtime audit, taken as a set.** Two are
answered by byte math in the sandbox (P15, L6), one is decided and pending its instrument
(L2), one shipped as constants (L5), and the audit found dormant debt and closed it (D5).
The rule for every one of them: the acceptance instrument is named *here*, because that is
the location it is checked against, not retrofitted to a diff.

### L2. The probe bake on the RT cores -- hybrid, and the decision is the entry

**Does the bake stop reading `WorldGen::height`?** 3,125 us per chunk, 43% of the chunk
build, and blind to trees, caves, overhangs and edits.

**Decision: hybrid -- height field far, ray query near (batch-69b BLAS), with the near
reach at 64 blocks.** The reasoning, resolved here at the wording the handoff demanded
rather than rediscovered in code:

- **What the height field buys today and why it is not nothing.** The bake is a *pure
  function of world position*: no apron, no residency coupling, no re-bake ever, and an
  unbaked texel holding 1.0 is *identity*, not wrong. D1/UPLOAD_REACH's whole determinism
  argument -- "the lattice is a pure function of the resident set at the capture" -- stands
  on exactly this, and `make_probe`'s comment says so verbatim.
- **What full ray-query costs, priced in its own units.** Probes near a chunk border march
  out to `REACH` = 192: a full-geometry bake needs up to 7^3 chunks resident to answer one
  chunk's field, so the bake starts *defending* the streamer instead of being carried by
  it; an unbaked texel in a world with ray queries is not the 1.0 identity but *open sky in
  a cave mouth*. And an edit under full costing is unaffordable horror: `set_block` at `p`
  invalidates every probe whose fan crosses it within 192 -- worst case the 384-block cube,
  216 chunk re-bakes, ~0.7 s of worker time for one placed block. That number alone closes
  option (b).
- **Why 64 blocks of near reach and not another number.** One chunk ring: an edit's
  re-bake set is at most 27 chunks (84 ms, bounded and queueable, and only for edits inside
  the D1 upload window -- outside it no probe is baking *at all*). Beyond it the height
  field stays *bit-identical to today* (the far function is exactly the function the bake
  has always had), so the only pixels allowed to move are inside the instrument's reach.
  The pure function still decides *what neighbours share*: the far value is
  position-invariant, so a cross-chunk seam can only be authored inside 64 blocks, which is
  exactly the region the acceptance checks.

**Acceptance:** `--bench-terrain 96` per-chunk line against the parent binary, then
`bitexact` at all vantages *except* `canopy` and `cave`, which **must** move (they are the
two cameras standing under coverage the height field cannot see; if they don't move, the
geometry is not in the bake). **Kill:** BLAS build over 3,125 us at equal-or-worse MAE --
a bake twice the price for the same picture -- or any visible chunk seam anywhere in the
near reach (the seam is the hybrid's characteristic failure, so that is where it is read).

### L5. The probe lattice went to 4-block spacing

**Shipped as the constants that define it** (commit `589093a`): `SPACING` 8 -> 4,
`PROBE_DIM_XZ` 64 -> 128 holding the 512-block toroidal period *unchanged* (every
`UPLOAD_REACH` claim is period arithmetic and stands unedited, D1b's window included), the
two mirrored WGSL numbers with their `const _` pins. The tap is still one trilinear read;
the bill is 8x the worker bake and 12.6 MB -> ~100.7 MB of the 3.9 GB of VRAM, which is
96.5% free. Max 3D texture axis on this hardware is 16,384 against the 768-texel slab stack
-- probed, not assumed, because this tree once rejected a design on a limit it had never
queried.

**Acceptance:** `resolve` must bench flat (within 1 se of zero) at every vantage -- no live
registers were added -- then `--reference` swept 8/4/2 at `canopy` and `cave`.
**Kill:** 8 -> 4 under MAE 0.372 at `canopy` is a clean no and closes the entry in one
session.

**The hardware re-read (2026-09-16) filed, with its costs finally named in one table.** The
frame itself pays nothing at either spacing (one tap, inside measurement error) and the
picture benefit stays under the by-eye bar at every camera (UA-0.61 MAE at `canopy`); what
spacing 4 spends is all upstream of the frame:

| | spacing 8 | spacing 4 |
|---|---|---|
| probe bake | 3,256 us/chunk | **13,915 us/chunk** |
| chunk build total | 7,824 us | **18,514 us** |
| probe VRAM | 12 MiB | **96 MiB** |
| process RSS | ~217 MB | **501 MB** |

The keep case is motion stability only -- the MAE numbers above are what a still camera can
say, and the eye verdict this entry owes is still **pending**. If the verdict kills spacing 4,
reverting to 8 pays back 10.7 ms per chunk build and 84 MiB of VRAM, and takes P-D and P-E
down with it (both are entered conditioned on spacing 4 surviving).

### P-E. The probe bake, parallel: per-slab across the rayon workers, edits as the first consumer

**This designation existed only in a chat queue until batch 93 filed it** -- see the roster in
*Where a closed designation went* -- and it is entered here exactly once so that the queue's
claim survives a session boundary: what it is, what it costs, and why it waits.

**The claim.** The probe bake is **13.9 ms of an 18.5 ms chunk build**, single-threaded on the
main thread, so a chunk walked toward appears 2.4 times later than it appears when the bake is
not in the way. The bake is embarrasingly parallel -- every probe is a pure function of world
position (P-D's close-out is what proves that from the tree's own constants) -- so splitting
the lattice **per slab** across the idle rayon workers would mostly erase that. **The first
consumer is not streaming but the edit rebake**: `journal::apply` already aggregates deltas
per coarse voxel, and a split bake is what rebakes the slabs an edit touched in session time
rather than in frame time.

**Ranked down, on the queue's own agreement, because it is the expensive answer to a question
with a one-constant answer.** The 13.9 ms is a consequence of L5's `SPACING` 4. At 8 the same
bake costs **3,256 us** and there is no latency problem to parallelize away. What spacing 4
buys the picture was measured on the user's hardware and reported 2026-09-16: **MAE 0.61 at
`canopy`, under the by-eye bar at every camera.** So this entry exists only in the world where
L5 survives its eye verdict -- if the verdict kills spacing 4, the chunk build returns to
~4 ms and P-E closes unbuilt with it, the one-constant alternative having already shipped in
`589093a`'s control.

**The blockers, named so the unblock is a sentence and not a session.** (i) The 13.9/18.5 ms
pair is a sibling-fork measurement; the next session re-reads it on this tree's bench before
designing against it. (ii) `UPLOAD_REACH`, D1b's window and L5's period arithmetic are all
written against the 512-block toroidal period of the *filled* field -- a lower-latency fill
does not change the field's shape, so none of those entries' pins moves, but the chunk-build
bench protocol from batch 57 (1912 us/chunk at spacing 8) is the comparison baseline. (iii)
The edit-rebake consumer wants the journal-delta-to-slab join that roadmap A8-2 names; the two
entries are siblings and neither is cheaper alone.

### P15. build_hiz reads 16.6 MB; the byte math says it has to

**The entry the pass's whole bandwidth apparently invites -- and doesn't.** It runs at 151
GB/s of the adapter's 192, so it *looks* like there should be free bytes: it reads one
64-bit word per pixel to take a 24-bit lane, and it reads every pixel to produce a
conservative per-tile bound. Both half-thoughts were taken to their conclusions:

- **"Does it need all 64 bits?"** The 64 bits are march's visibility key
  (normal:3 | ci:13 | voxel:24 | depth:24) produced by one `atomicMax(u64)`. A narrower
  read exists only from a separate 4-byte depth mirror that march would have to write *and
  clear*: one extra `atomicMax<u32>` per winning hit (+8.3 MB of stores per frame) and an
  8.3 MB clear -- **16.6 MB moved into the frame-wide contention pool against 8.3 MB saved
  in one pass**. Negative before latency is even asked.
- **"Does it need every pixel?"** No still can answer that safely, and that is the trap the
  entry carries with it: Hi-Z is lossless only because `recover_select` re-tests what the
  bound rejects. Sub-sampling a tile raises its minimum depth in the *false-reject*
  direction, and a rejection that is wrong only while the camera moves is geometry that
  vanishes under motion -- the one defect the fixture cannot see. There is no licence to
  change the picture, and there is no picture-safe version of this read.

**Acceptance: it has already been run and it is the bench row itself** -- build_hiz must
stay at 0.110 ms, bitexact 0 pixels everywhere, which it does because nothing changed.
**Kill was by construction**, which is the cheaper of the two; the trap named the shape of
the kill in advance.

### L6. The tensor cores as a bake GEMM -- decided, not built

Kill-clause arithmetic in full is [`ADR 0003`](adr/0003-tensor-cores-are-not-the-bakes-silicon.md):
a 2% slice of a 3,125 us line is not a place to spend 64 cores, and the gather would fill a
tile but cannot move the march that feeds it. Closed door.

### D5. The pipeline variant the P11 gate flips at runtime

**Audit results, all landed in `e96c11d`:** `ensure_spec` *was* lazy -- the first lamp
placed into a lamp-less world would have compiled a shader mid-play, the batch-54 hitch
recreated by design. Both emitter arms of any spec word are now pre-warmed at first use.
The "conservative-upward" arming is gone entirely: with the edit already knowing both block
ids, `LocalTree`/`ChunkRecord` keep an exact per-voxel emitter count, and the gate disarms
on the frame the last lamp breaks -- nothing can hold a session permanently armed.
`PREFIX_MASK`'s audit covered *every* Rust reader of `leaf_prefix` (grep-driven, not
recall-driven): the three arithmetic sites mask, the set_block repack preserves the
nibble, and `tree.rs` reads `LocalL1`'s unpaced prefix, which never carries one.

---

---

## Parked, each with the number it lost by

**Kept as one line apiece so they are not re-planned.** The arguments are in
[`errors.md`](errors.md) and [`research-review.md`](research-review.md).

- **P5, the gather's early-out.** Built twice in batch 51 and neither shipped. The cheap
  condition -- threshold the light at the face's own cell -- is **-1.579 ms of a 2.284 ms
  `resolve` at `cave`** and moves **10,150 pixels at max delta 66**, and no threshold saves it:
  a four-fold lower one gave *identical file hashes at all sixteen vantages*, so the damage is
  inside corners rather than dim faces. The exact condition -- the chunk's `light_flags` bit
  plus a span test -- is **-0.017 ms, sign mixed over six rounds**, because a chunk is 64 blocks
  tall and the one holding a cave holds the lit surface above it. The untried design is the 4^3
  light brick; **retry only after R3**, which is now the rung that would rewrite the brick this
  entry is written against -- R2 shipped at batch 58 and left the nine-cell gather untouched, for
  batch 57's reason exactly. **R1 shipped without touching it** -- batch 57 put the cube in a
  separate 3D texture rather than widening the nibbles, precisely so the decode stayed out of
  the nine-cell gather, so this entry is live again and unblocked rather than waiting.
- **P6, what the whole pass is bound by.** Answered for one block by batch 47 -- collapse the
  gather's nine reads to one address and 87-90% of its cost stays, so it is instruction count
  and not memory. The unrun half is P1's profiler-free protocol, and its only purpose was to
  rank P7.
- **P7, split `resolve` by material.** The largest structural idea the frame-time queue had, and
  conditional on P6. Occupancy has been chased twice here and paid nothing; code size once and
  paid **less** than nothing (-271,872 bytes, **+0.09 ms**). Nothing says the split would relieve
  what the pass is actually limited by.
- **A6, the per-block repeat on land.** Same field as 36c, which measured variants asymptoting at
  **0.64** against a 1/N floor with four buying 78% of everything they can ever deliver.
  `GRASS_SIDE` sits at **0.8365** under `FLIP_U` and a band-depth variant reaches **0.5090**,
  carrying a blotchiness risk `repeat_score` structurally cannot see.
- **The dark-water rung on a shallow camera.** Batch 53's rung tests the eye's own depth as
  well as the tile's rays, which is stricter than its derivation needs. Dropping `submerged`
  from one `min` in `tile_select` lets a camera three blocks down clamp whenever it pitches
  eleven degrees under the horizontal: **4.7% of `sea-horizon`'s DDA steps for 55,058 pixels at
  max delta 9**, against the 6.8% for 19,576 at max delta 3 the shipped rung buys at
  `deep-water`. **Declined on a motion argument rather than on the physics** -- anchored at the
  reach the clamp is a property of where the player is *looking*, so the far sea floor pops as
  they turn, which is batch 52's sliding seam one step worse and the half no still frame prices.
  Live rather than dead: it is one line, and the user took batch 52's trade on numbers of the
  same shape. [`errors.md`](errors.md).
- **A7, the gather trade.** **-1.764 ms at `cave`, -0.916 at `default`** for dropping the primary
  hit's nine-cell gather. **Declined by the user on 2026-09-14, against the picture**: without it
  a terraced hillside loses each step's inner corner and stops reading as steps. Live rather than
  dead -- the number is on file and the decision costs one build -- but moot until R3 settles
  what the gather is gathering. **R2 did not settle it either**: batch 58 multiplied the floor
  and left all nine lookups where batch 57 left them. **Neither R1 nor R2 settled it**: batch 57 left all nine lookups in
  place, because a field carrying sky occlusion alone cannot replace the contact cue they buy.

- **P3c, read-before-write.** `march`'s `atomicMax` on a hit that loses is a read-modify-write
  changing nothing, and batch 49 measured 0.850 ms of frame riding on such writes. Batch 50
  built the skip -- bit-exact by the monotonicity of `atomicMax`, 256 bytes, no registers -- and
  it is **indistinguishable from zero**: `resolve` -0.029 +/- 0.021 at `underwater`, +0.000 at
  `default`. **Hi-Z already takes 94% to 100% of the losing writes**, so there is nothing left to
  collect. Retry only with a *cheaper read of the same word*, never `atomicLoad`.
  [`errors.md`](errors.md) and [`gpu.md`](gpu.md).
- **36c, the rest of the repetition.** The stone seam is a **no-op** -- `blob()` is exactly
  periodic with period 16 and sampling world coordinates is the identity map on this geometry,
  correlation **+1.0000**. Variants asymptote at **0.64** against a 1/N floor, and four buy 78%
  of everything variants can ever deliver here.
- **Traversal micro-optimisations from the 64-tree article** -- octant mirroring, mantissa-XOR
  inversion, ancestor detection via `firstbithigh`. Unexploited and low priority; ancestor
  memoisation already failed because three levels is not deep enough.
- **DLSS.** wgpu does not expose it, it is vendor-locked, and we are not fill-rate limited. The
  mode actually wanted is DLAA, which is what batch 6 amounts to.
- **Additive dual radiance.** The corner magnitude is `max(sky, block)`, Minecraft's rule. A look
  decision, not a correctness fix; leave it unless the caves read wrong.
- **Raising `TEX_SIZE` above 16.** The ceiling on how fine a blade can read, and it touches every
  texture, every mip and the alpha tint mask. Its own batch, and not a small one.
- **A froxel volume** and **a sun-space depth buffer splatted with `atomicMin`** -- the two
  structures batch 35's research turned up and the batch did not need. Each answers a question
  batch 35 dodged rather than solved: per-pixel evaluation becoming the cost, and casting from
  geometry rather than from a heightfield.
- **`coherence` as a ranking instrument.** It reads **1.0594x against a 1.037 floor** and catches
  total blindness and very little else. Only worth revisiting if an appearance batch again needs
  to rank a repeat *in image space*, and what it would need is a vantage that is mostly one
  permutable layer at a resolvable block size. `tests/repeat.rs` ranks a repeat with no frame in
  it at all.

