# GPU shape: what actually costs time in this frame

**Consolidated from batches 13 and 17**, the two measurement batches that moved zero pixels.
Between them they rule out most of what a reader would guess. Everything ruled out is in
[`errors.md`](errors.md) with its number; this file is what remains true.

---

## What a pixel costs, which is the whole performance character of this engine

**Added after batches 44-47, because it is the shortest true thing anyone can carry.** The
frame's cost is not geometry and it is barely traversal. It is **what kind of pixel is on
screen**, and there are three kinds:

| pixel | what `resolve` does for it | roughly |
|---|---|---|
| **sky** | one `sky_color` -- the gradient, the scatter lobe, the cloud FBM. No rays, no lighting, no texture | **1 unit** |
| **land** | the nine-cell light gather and its AO, one shadow ray, a texture tap, the envelope and cloud taps | **~3 units** |
| **water** | all of the above **three times over** -- the surface itself, then a refraction ray and a full `shade_hit` on what it finds, then a reflection ray and a full `shade_hit` on what *that* finds -- plus a `sky_color` every water pixel pays unconditionally for the Fresnel mix | **~9 units** |

**Everything measured in batches 44 to 47 is a consequence of that table.** The frame map's
2.4x spread across the vantage set is the mix of the three. `terraces` and `coastline` are the
expensive frames because they are nearly all *near* water; `sky` and `cave` are cheap and
expensive respectively for the same reason from opposite ends -- `cave` has no sky pixels at
all and every one of its pixels is a close land hit paying the full gather, which is why that
one block is **72% of `resolve`** there.

**Two ramps already bend the water row down with distance**, which is why a frame full of
*distant* sea is far cheaper than one full of near sea: the refraction fades out over
96-192 blocks and the traced reflection over 512-1024, so past a few hundred blocks a water
pixel drops from about nine units to about four. That is the single biggest reason pitch
changes the frame rate so much at a fixed position.

**What it tells a batch**: anything aimed at `resolve` that is not aimed at water's two
secondary legs or at the nine-cell gather is aimed at a small fraction of the pass. Those two
are the pass. See `PERF.md` for the frame map and the decomposition behind this.

## The headline: occupancy has never once predicted a time here

**`resolve` is not register-starved and not occupancy-limited.** 80 registers at 8x8, **no
spill, no stack anywhere in the frame**.

> **The 80 is stale as of batch 38b, and it is now a property of the pipeline key rather than of
> the pass.** `resolve` reads **96 registers with `SPEC_GLASS=1`** and 80 with it off -- the
> first time it has left 80 since batch 13, and the shipping build is the 96 one. Every figure
> below was computed at the old shape. **Re-read it with `shaderstats --override SPEC_GLASS=0/1`
> before quoting any of it**, because a table computed at one specialization describes a shader
> the other one is not -- which is the same trap batch 17 recorded when widening a block changed
> the register count out from under an occupancy table.

- Batch 13 took `resolve` from 50% to 66.7% theoretical occupancy -- eight more resident warps
  -- and the pass moved **-0.01 ms**.
- Batch 17 raised `tile_select` from 67% to 100% with nothing else in the change and bought
  **exactly zero**.

**The arithmetic is worth doing because it says what *cannot* be the problem. It has never said
what is. Only the bench does.**

## What does pay is warp fill

`tile_select` at 4x4 ran **16 threads against a 32-lane warp** -- half the lanes idle by
construction. Filling the warp bought **2.3x of the pass** at three vantages.

Half-masked lanes are a *throughput* defect and show up on a clock; occupancy is a latency-hiding
*ceiling* and twice has not. When both are wrong at once, fixing either fixes both, so **the
sweep that separates them is the only thing that says which one paid**.

**Count the lanes against the subgroup width before you open the occupancy table.**

## The register budget is not independent of the workgroup shape

Widen a block from 64 threads to 128 and the driver answers by spending **fewer registers per
thread** -- 80 down to 64, with a 16-byte spill to get there. So "more threads per block" and
"the same code per thread" are not both true, and an occupancy table computed from a register
count measured at the old shape is a table about a shader that no longer exists.

## A folded gate and a run-time gate are different programs, and batch 45 measured the gap

**The sharpest number this file has, and it is not about lighting.** Batch 45 gave secondary
hits a one-lookup light model instead of the nine-cell gather, and built it twice. The two
builds differ in **one expression** -- how the arm is gated -- and take the same arm, on the
same pixels, for the same output:

| gate | `terraces` | `default` | `lattice` | `glass` |
|---|---|---|---|---|
| `SPEC_FLAT_SECONDARY && (frame.flags & FLAG_FLAT_SECONDARY) != 0u` | **+1.000 ms** | **+1.420** | **+1.465** | **+1.391** |
| `SPEC_FLAT_SECONDARY` alone | **-1.584 ms** | **-0.502** | **-0.726** | **-0.682** |

A **2.58 ms swing at one vantage** between two spellings of the same decision.

**The first form is the house style, which is what makes this worth a section.** Every *switch*
override from batch 13 to batch 38b is written `SPEC_X && (frame.flags & FLAG_X) != 0u`, and
the reason is good: the flag word stays the thing that decides, so a pipeline key disagreeing
with `frame.flags` still draws the picture the flag asked for. That belt-and-braces is free
when the arm it guards is small. It is **not** free when the arm lives inside a function
`resolve` inlines four times: `frame.flags` is a run-time value, the driver cannot prove which
arm is taken, and every inlined copy carries both arms and the branch.

**`default` is how you tell which mechanism you have, and it costs nothing to look.** It is a
frame with water only at its right edge -- so it runs the new arm on almost no pixels, and it
paid the *largest* regression of the six. **A cost that lands hardest where the feature does
least work is code, not work.** That is batch 12's finding restated as a diagnostic rather than
as a fact, and it is the cheapest way this project has found to separate the two.

**What to take from it**: when an override gates something inside a hot inlined call tree,
write the override alone and let the pipeline key be the only copy of the decision -- which is
exactly what `FLAG_WATER_SHADOW_CUT` and `FLAG_LEAF_THIN` already do, for a different reason
(a distance does not fit in a bit). Batch 45 is the first time a *switch* has had to do it, and
the licence is the same one: every bit in `SPEC_MASK` is fixed for the life of the process.
`tests/secondary.rs` holds the guard, because the wrong form is the form a tidy-up would reach
for and nothing in any image shows it.

**Batch 46 bounded that advice and it is the half a reader needs second.** The same fold applied
to the *other eight* switch gates is **-0.119 ms at `terraces`** and inside its own standard
error everywhere else -- a twentieth of batch 45's result, for a fifth of the code change
(12,544 bytes against 60,928). **The quantity that matters is the gate shape times the arm
behind it times how many copies the pass inlines.** Batch 45's arm was the nine-cell gather in a
function inlined four times; these eight guard a uv permutation, a simplex tint, a shaft sample
and four wave branches. So the rule is not *fold every gate* -- it is *fold a gate whose arm is
large and multiply inlined*, and **`shaderstats` tells you which before a single bench**: eight
one-line edits and eight offline compiles cost two minutes and predicted the whole answer. The
`frame.flags` half stays everywhere else, because it is what keeps the flag word the thing that
decides. See `PERF.md` and [`errors.md`](errors.md).

## Code size is a hypothesis about time, and batch 42 is the first test of it that failed

**Batch 12 measured a control costing +0.07 ms while executing nothing and found 20 KB of
machine code rather than a register, and every batch since has read that as *size costs time*.**
Batch 42 removed 271,872 bytes of `resolve` -- 85% of everything `SPEC_GLASS` adds to it, taking
the pass from 615,424 bytes back to 343,552 -- and the frame got **+0.09 ms slower** at
`coastline`, bit-exact at all fifteen vantages.

| `resolve`, `SPEC_GLASS=1` | bytes | registers | `coastline` |
|---|---|---|---|
| batch 38b's shape (two inlined copies of `shade_water`) | 615,424 | 96 | -- |
| batch 42's hoist (one shared dispatch) | **343,552** | 96 | **+0.09 ms** |
| for reference, `SPEC_GLASS=0` | 293,888 | 80 | -- |

**Two things that leaves standing, and they are what the next session should read.**

- **The +1.036 ms that roadmap entry 38e was opened for is not instruction-cache pressure from
  glass's code.** Four fifths of that code is gone and the millisecond is not. What did *not*
  move is the **register count**, 96 with glass compiled in against 80 without -- the one
  number batch 38b changed that this batch could not. That is the hypothesis 38e now carries.
- **A shared call site is paid for in constant propagation.** The merge made the primary
  `shade_hit` call take variables a branch may have overwritten where it used to take values
  the compiler could see through, and the pass that pays for that is the one running on every
  pixel. The +0.09 is that trade, and it is the cost side of every "call one function from two
  places" tidy-up in a shader this size.

**So size and time are two measurements here, not one.** Batch 12's finding was that size is
what *differs* when a folded control costs something -- which is still true and still worth
checking first. It was never evidence that shrinking the pass makes it faster.

**Batch 45 is the other half of that pair and it points the other way.** Folding the gate took
`resolve` from **615,424 to 554,496 bytes** -- 60,928, a *fifth* of what batch 42 removed --
and bought **-1.584 ms** where batch 42's 271,872 bytes bought +0.09. Four and a half times the
code, opposite sign. **The two together are the statement: bytes removed predicts nothing at
all, in either direction.** What batch 45 removed was three inlined copies of nine memory
lookups, and the size is the evidence that the fold happened rather than the reason it paid.

## "It costs X while executing nothing" has two mechanisms wanting opposite fixes

Batch 12 called its control's +0.07 ms occupancy. Measured, the register count was **identical**
with the feature compiled in and out; what differed was **20 KB of machine code** in the pass
that is 58% of the frame -- **64% as of batch 47's re-measure**, which does not change the argument and is recorded so the figure is not quoted forward as current. Batch 14 measured +1.14 ms and it is both.

An occupancy problem wants fewer registers; a code-size problem wants the code *gone*, which is
an `override`. **Only a register count tells you which one you have.**

---

## The shapes, and why they are what they are

| pass | shape | why |
|---|---|---|
| `tile_select` | 8x8 | a full warp; 4x4 was half-masked. 16x8 and 16x16 were built, measured and thrown away |
| `march` | 8x8 | one workgroup per (tile, chunk) pair, dispatched indirectly |
| `resolve` | 8x8 | 16x8 raised occupancy a third and bought -0.01 ms |
| `recover_select` | 64x1x1 | 16 registers, one thread per deferred pair over a 1-D list; 64 is already two full warps and it has neither defect |
| the three `finalize` entries | 1x1x1 | genuinely one thread of work each -- read a counter, write three words. 1/32 of a warp is as wasteful as it looks and it is four instructions, three times a frame; `tile_select` at 0.16 ms is 16000x their combined cost |

**`tile_select` appends through an atomic counter, so its dispatch shape decides the pair
buffer's *order*.** Order is unobservable today for two reasons and one measurement: `march`
resolves by `atomicMax` so pair order cannot decide a pixel; the pair *set* does not depend on
the shape; and the one order-sensitive step -- truncation at `MAX_PAIRS` -- is not reached at any
sane capture size (334k at 1920x1080 against a floor of 1,048,576). **That headroom is what makes
batch 17's reshape bit-exact.**

> **Two numbers in the paragraph above were wrong until batch 53 measured them, and they were
> wrong in the dangerous direction.** This file said the cap was "not reached at any sane
> capture size" and that 7680x4320 asks for 3,088,248 -- an arithmetic projection nobody had
> checked. Counted at `default`: **1,529,818 pairs at 5120x2880**, 46% over the old fixed cap
> and truncating, and **80.5% of it at plain 3840x2160**. The list is now sized from the screen
> like every other screen-sized buffer, with `MAX_PAIRS` kept as the floor so the two
> resolutions this project measures at allocate exactly what they always did. `errors.md` and
> `tests/pairs.rs` carry it.
>
> **And the cap was never the first limit.** The pair word spends 19 bits on the tile index, so
> a screen may hold 524,288 tiles; **7680x4320 is 518,400, which is 98.9%**. Past it a tile
> marches another tile's chunks -- not missing geometry, *wrong* geometry, in a frame with
> nothing in it to say so. `Renderer::warn_if_oversized` is the guard, added thirty-one batches
> after the field.

**If the list ever overflows, which subset survives is a function of the workgroup shape.** It is
reachable from a flag, and since batch 22 it is no longer silent: `finalize` and `finalize_recover` store the raw demands into `counters[4]`/`[5]` before
the clamp, `Renderer::warn_if_truncated` prints from both headless modes, and the fixture refuses
to score a capture that warned.

---

## Specialization: `SPEC_MASK`

**`march` and `resolve` are the only passes with more than one pipeline.** Anything in
`SPEC_MASK` is folded into the shader as a WGSL `override` at pipeline-compile time instead of
branched on per pixel, which takes the dead feature's **code** out of the module rather than
merely skipping it.

`ensure_spec` keys the cache on `frame.flags & SPEC_MASK` and builds **both** passes from one
constant list, because `foliage_aware` lives in `common.wgsl` -- a run where the two disagreed
would march a tuft the shading pass cannot see.

**Everything in the mask must be fixed for the life of the process.** `FLAG_HIZ` and
`FLAG_HEATMAP` are on function keys and `FLAG_UNDERWATER` depends on where the camera is
standing; all three must stay out of it.

**Batch 48 is the first flag deliberately built to stay out, and it is worth knowing why a
feature bit would want to.** `FLAG_WATER_FAR` gates a cull that only runs when the camera is
submerged, so it is only ever read ANDed with `FLAG_UNDERWATER` -- and a mask bit exists to let
the driver fold an arm away at pipeline-compile time, which is impossible when the other half of
the condition changes per frame. `const _: () = assert!(SPEC_MASK & FLAG_WATER_FAR == 0)` pins
it, the inverse of the four asserts beside it.

**The sharper half is structural: the pass could not have taken an override anyway.**
`SpecPipes` holds `march` and `resolve` and nothing else, so `tile_select` is built once with no
constants -- which means a *number* for it has nowhere to live but a `GpuFrame` field (and that
struct has no pads left) or a plain WGSL `const`, which is what `WATER_FAR_DIST` is. **Before
reaching for an `f32` override the way batches 41 and 43 did, check that the pass reading it is
one of the two that get more than one pipeline.**

**Batch 55 added the first bit whose feature ships *off*, and the asymmetry is worth naming.**
`FLAG_PROBE_TAP` gates a diagnostic -- one trilinear tap per shaded surface, priced for roadmap L1
-- and every other bit in the mask gates something the shipping build wants. It is in the mask for
`FLAG_WATER_SHADOW_CUT`'s reason rather than `FLAG_TINT`'s: **the shader never tests the bit**,
because `SPEC_PROBE_TAP` is deliberately the whole gate, so there is no run-time test that could
disagree with a stale pipeline key and a bit outside the mask would leave `ensure_spec` unable to
tell the two builds apart -- `--probe-tap` would then render the shipping frame, and a sweep whose
pass condition is *0 pixels differing* would call that a pass. **The measurement it produced is
about this section rather than about the tap**: `resolve` is 582,400 bytes with the arm compiled
out, 583,680 with one tap and 584,960 with three, at **96 registers in all three** -- and the frame
cost is the same at one tap and at three. So the fold works, the arm leaves the module, and what is
left costs 0.01 to 0.055 ms *whether or not it runs*, which is this pass's standing defect (P8)
showing up in the smallest feature anyone has added to it.

**A fold is a property of the pass, not of the source shape, and neither result predicts the
next.** `SPEC_TINT`'s shape lost `resolve`'s fold for batch 15 and kept it for batch 16 -- same
three lines, two passes, opposite answers, because the driver stops propagating a run-time value
through a large enough inlined call tree. **Check every pass the constant reaches with
`shaderstats`, in both directions, before believing a fold.** See [`foliage.md`](foliage.md) for
the one case where the AND had to be dropped.

---

## Reading the driver

**`render::shader_source()` and `render::ENTRY_POINTS` are the only copies of what the module is
and what is in it.** `--bin shaderstats` compiles that same string offline with
`wgpu_hal::vulkan`'s own naga options. A second concatenation over there would be a second
shader, and a register count for a shader nobody runs is worse than no register count; a pass
added without an entry in `ENTRY_POINTS` gets **no** number rather than a wrong one.

**Binary Size does not identify a program.** Two builds both reported 240128 bytes and 72
registers and their SPIR-V was **177 words and 708 bytes apart** -- the override fold leaves
machinery behind that deleting the code does not. The size is a quantised *allocation* figure and
the register count a *maximum*, so two different programs land on the same pair routinely.
`--wgsl` dumps the module and `--source` reads one back, so diffing the bytes costs no rebuild.

**Two driver-side numbers that carry no information.** `resolve` reports shared memory it does
not declare (2816 bytes per block at 8x8, against zero `var<workgroup>` and zero barriers) -- the
statistic is trustworthy where it can be checked, so this is the driver reserving something of
its own, and it does not bind. And "Local Memory Size" carries a constant **2^36** on every pass;
only the low bits move, and they move exactly once, to 16, at the same moment the register count
drops to 64 -- which is what a spill looks like.

---

## A pass is billed for the work it does and not for the writes it issues (batch 49)

**Batch 48 left this file an open measurement-integrity question and batch 49 closed it.** The
question was worth a session rather than a footnote: batch 48 changed one expression in
`tile_select` and `resolve` came back **-0.755 ms at `underwater` on a bit-identical frame**,
and if the cause was `march` doing less work raising the clock `resolve` runs at, **the per-pass
split of every paired bench here would be soft** -- including batch 45's 2.58 ms gate swing and
batch 47's gather table, both of which are arguments about *which pass* moved.

**The discriminator was a third build**, because batch 48's change moves two things at once --
the pairs `march` is handed and the `atomicMax` writes it issues. The **ghost** build emits the
culled pair *tagged* and skips only the write, so `march` does pre-48's work with batch 48's
write traffic. At `underwater`, hi-z off, where the pair counts confirm each row varies one
thing:

| experiment | what varies | `march` | `resolve` |
|---|---|---|---|
| writes alone | 150,372 pairs' writes suppressed, **same 372,335 pairs marched** | -0.077 | **-0.770** |
| work alone | 372,335 -> 221,963 pairs, writes already suppressed | **-2.380** | **-0.002** |

**`march` doing 2.380 ms less work moves `resolve` by 0.002 ms.** That is the direct test of the
channel that was in doubt, and it says the split is sound: whatever a pass does with its own
instruction stream, the next pass cannot tell. **Batch 45's and batch 47's results stand as
written.**

**What the split does not survive is a pass's fire-and-forget writes.** Suppressing those
150,372 pairs' `atomicMax` -- at most 9.6 million rays' worth -- is worth 0.850 ms of frame, and
**91% of it is billed to `resolve` and 9% to `march`**. The returned value of an `atomicMax` is
discarded here, so `march` never waits on one; something downstream does.

**Batch 52 read it a second time, at a vantage batch 49 did not have, and the split is real but
not a constant.** At `sea-horizon` the submerged reach clamp is worth **-2.776 ms**, of which
**-1.880 is `resolve` and -0.859 `march`** -- 68/31 against `underwater`'s 91/9. So the *sign* and
the *mechanism* transfer and the *ratio* does not, which is what you would expect of a quantity
that depends on how much of the culled work was marching and how much was writing. **Quote the
direction, not the number**: a march-side cull benched on `march`'s row alone hid two thirds of
itself here and nine tenths of itself there, and in neither case would `march` have said so.

**Two candidates were ruled out by measurement rather than by argument.**

- **A power or clock state**, twice. The work row above is the first: 2.380 ms removed, nothing
  moves. The second is cheaper and worth copying -- in the writes row, where `resolve` moves
  **-20.5%**, `select` reads 0.155 against 0.155 and `taa` 0.502 against 0.501. **A clock change
  is global, and this one lands on one pass.** Check the short passes before reaching for a
  power explanation; they cost nothing to read and they are already in the table.
- **`resolve` reading a warmer visibility buffer**, which was the leading candidate. With hi-z
  **on**, `build_hiz` runs between `march` and `resolve` and streams the whole buffer -- 16.6 MB
  at 1920x1080 -- in 0.110 ms identical on both sides. That is **151 GB/s, so it is being read
  from memory and not from cache**, and `resolve` still moves -0.756. Nothing `march` left
  resident survives to be read.

**The reusable half is the build, and it is batch 47's technique aimed at a different channel.**
Batch 47 held a block's instructions fixed and moved its memory behaviour. This holds a pass's
work fixed and moves its *write* behaviour, by tagging the pair rather than dropping it. Both
are the same move: **when one change varies two quantities, the discriminator is the build that
varies one of them, not an argument about which is larger.**

**Two cautions for whoever builds the next one.**

- **Hi-z on is not a one-variable experiment for a write change.** Suppressing a write changes
  what `build_hiz` finds, which changes how the *next* frame routes pairs between `march` and the
  deferred list: the ghost build marches 183,808 pairs where its own control marches 161,554.
  The hi-z-off block is the clean reading and the hi-z-on one closes just as well without being
  clean.
- **The gate has to be checked against the driver sinking the traversal into it.** `if h.hit &&
  !ghost` sits after `march_chunk`, and a compiler that moved the call inside the branch would
  have made the experiment measure nothing while still rendering the right picture. The check is
  free and it is already in the table: `march` reads -0.077 of 7.436 in the writes row, so the
  work really was held fixed.

---

## Removing work whose result is discarded -- the idea, and why batch 51 could not spend it

**Batch 47 asked what the gather is bound by and answered *instructions*. Batch 51 tried to
spend that answer and came back with nothing**, which is worth a section because the idea is
sound and it is the *condition* that is hard.

**The idea.** The gather's whole output reaches the pixel through `(ambient_rgb + sun_rgb) * ao`,
and every term is proportional to the light it just gathered -- `ao` multiplies that product and
nothing else, because the `frame.ambient` floor sits outside it. So on an unlit face the nine
lookups are **already** being multiplied by zero. Removing them is not an approximation and needs
no look constant authored. **Before pricing a way to make an expensive block cheaper, check
whether there is a case where its result is discarded.**

**The two shapes built, and the gap between them is the whole finding:**

| condition | `cave` | picture |
|---|---|---|
| light at the face's own cell is below a threshold | **-1.579 ms** of a 2.284 ms `resolve`, 8/8, t = +272 | **10,150 px at max delta 66** at `overcast` |
| the *chunk's* `light_flags` uniform bit, plus the span test | **-0.017, sign mixed over six rounds** | bit-exact, 16 of 16 |

**The first is fast and wrong.** A face can be dark while a *diagonal* of its 3x3 is lit,
whenever that diagonal's two in-plane connectors are solid and the light arrived from in front of
the face rather than through the plane. `light.rs` bounds every cell it can reach and says
nothing about one it cannot. That is an inside corner, voxel terrain is made of them, and it is
the corner AO shuts hardest -- so it removes contact shading at exactly the corners roadmap A7
was declined for losing. **No threshold rescues it**: a four-fold lower one gives a
*bit-identical* image, which is the measurement that killed the whole family.

**The second is exact and never fires.** A chunk is 64 blocks tall, so the chunk holding a cave
also holds the lit surface above it and the uniform bit is clear. **Bit-exact at sixteen
vantages was the feature doing nothing**, and only the bench separated the two readings --
`lessons.md`'s *a no-op claim cannot be told apart from a not-wired-up claim*, met in the wild.

**What the condition has to be is the 4^3 light brick, and that is unbuilt.** Bricks carry their
own uniform bit, a dark region is uniformly dark at brick scale, and a face's 3x3 lies in one
plane so it spans at most **2x2 = 4 bricks**. Four loads against nine cell decodes, eighteen
`light_curve` evaluations, four reciprocals and the AO rule. Roadmap P5.

**Neither build moved the register count** -- 96 with the cut compiled in and 96 without -- and
the fast one *added* 10,624 bytes to `resolve` while making it much faster. Batch 42 and batch 45
restated a third time: **bytes predict nothing in either direction; what predicts is whether the
work runs.**

## Hi-Z is a write cull as well as a march cull (batch 50)

**Nobody had connected the two passes this way, and batch 49 is what makes it matter.** Hi-Z is
described everywhere in this project as an occlusion cull that saves *marching*. It is also, by
the same action, a cull of the `atomicMax` traffic `march` issues -- because the chunks it
rejects are exactly the chunks whose writes would have lost.

Counted per frame at 1920x1080, as the share of `march`'s hits whose key cannot win:

| vantage | hi-z off | hi-z on | Hi-Z removes |
|---|---|---|---|
| `underwater` | 58.0% | **12.8%** | 94.7% of the losers |
| `default` | 72.5% | **21.5%** | 94.0% |
| `cave` | 69.8% | **0.0%** | 100% |

**Batch 49 measured that such a write is billed at 91% to `resolve`**, so part of what Hi-Z has
been buying since it shipped was never `march`'s time at all. That also retires the obvious
follow-up: batch 50 built read-before-write, and it buys nothing precisely because this table
leaves it nothing to collect. See [`errors.md`](errors.md).

**`cave` is the structural case and the cleanest number here**: 2,073,600 hits for a 1920x1080
frame is exactly one per pixel. Every tile marches only the camera's own chunk -- the one
`tile_select` exempts from the box test, because from inside a box the ray-box intersection
returns the *exit* distance -- so nothing ever competes for a pixel and nothing can ever lose.

---

## What `resolve` is bound by -- one block answered, the rest open

`resolve` is two thirds of the frame, spilled nothing at the shape it ran for twelve batches,
and did not care about a third more occupancy. For twelve batches **nothing said what it *is*
bound by**: memory latency, texture throughput and plain instruction count were all standing.

**Batch 47 separated two of those for the largest identifiable block of the pass, and the answer
is instruction count.** The nine-cell smooth-lighting gather is **72% of `resolve` at `cave`**,
16% at `default` and 1% at `coastline` -- the ceiling measured by replacing it with batch 45's
one-lookup flat model. Then the decisive build: point all nine reads at the **same cell**, so the
instructions are identical (`resolve` moves 640 bytes) and the memory system sees one address
instead of nine.

| vantage | nine reads -> one address | the whole block | share recovered |
|---|---|---|---|
| `cave` | -0.191 ms | 1.764 | **11%** |
| `default` | -0.123 | 0.916 | **13%** |
| `canopy` | -0.106 | 0.899 | **12%** |
| `edits` | -0.092 | 0.904 | **10%** |

**Nine-to-one on the memory traffic and 87-90% of the cost stays.** This block is ALU- and
issue-bound, not latency-bound -- which is consistent with every occupancy result above, because
occupancy buys latency hiding and there is little latency here to hide. **That is the first
positive statement this file has been able to make about what the pass is limited by**, rather
than another thing ruled out.

**What it retires**: caching the gather per block face in workgroup memory. The nine samples
depend only on `(ci, v, normal_id)`, so every pixel on one face recomputes them identically --
up to four hundred at the bottom of the `default` frame -- and that redundancy is real and worth
about a tenth of a millisecond, because it was never the expensive part. A cache would pay tag
compares and a barrier, in a pass that declares no workgroup memory and no barriers, to chase
less than the rows above.

**What it does not settle**: texture throughput, and whether the *rest* of `resolve` behaves like
this block. The profiler-free protocol in [`research-review.md`](research-review.md) is still
never run, and it is still the way to ask about the pass as a whole. But the technique batch 47
used is cheaper and worked: **take one block, build a variant that changes only its memory
behaviour while holding its instructions fixed, and bench the pair.** That is available for any
other block anyone suspects.

---

## What `march` is doing, counted (batch 53)

**`march` had never been looked at.** Fifty-two batches aimed at `resolve`, and the one pass
whose work is *exactly countable* was ranked with a stopwatch or not at all. It accumulates its
loop counter into `dbg` under `FLAG_HEATMAP` and has since the heatmap existed; nothing on the
CPU had ever read it. `--march-stats` is the reader, `harness march` the sweep, and the whole
instrument adds **no shader code** -- which is the point, because a per-workgroup reduction
would have meant giving `march` the workgroup memory and barrier it does not have, and the
pass being counted would not have been the pass that ships.

**An integer is the reading a laptop cannot move.** `PERF.md` opens with three caveats and all
three are about milliseconds; `pitfalls.md` records `resolve`'s A side climbing 19% over seven
rounds of heat soak. Two `harness march` runs an hour apart agree to the unit.

### The distribution, which is flatter than the heatmap looks

Steps per pixel at 1280x720 on the shipping build, summed over every `(tile, chunk)` pair that
reached the marcher. `per ray` divides by `(select + recover) * 64`, which is one `march_chunk`
call and not one pixel -- a pixel covered by four chunks pays four walks:

| vantage | steps | per ray | p50 | p90 | p99 | top 10% | past `MAX_ITERS` |
|---|---|---|---|---|---|---|---|
| `sea-horizon` | 19.4M | 5.8 | 17 | 47 | 73 | 28.6% | 18 |
| `deep-water` | 18.3M | 7.5 | 15 | 43 | 68 | 27.3% | 6 |
| `default` | 13.3M | 6.0 | 12 | 24 | 52 | 25.0% | 3 |
| `terraces` | 7.3M | 2.6 | 6 | 16 | 43 | 34.9% | 0 |
| `underwater` | 6.3M | 1.7 | 3 | 17 | 50 | 46.9% | 1 |
| `cave` | 3.7M | 4.0 | 4 | 5 | 7 | 16.3% | 1 |

**Every heatmap in the set puts the cost in a thin bright band of rays grazing water, and the
percentiles say that band is not where the money is.** Read before the batch, the top 1% of
pixels held **3.0 to 9.5%** of the steps and the top 10% held 16 to 47%; the **median** pixel,
at 12 to 17 steps on the shipping build, is the frame. An optimisation aimed at what the
heatmap draws the eye to would have been aimed at a twentieth of the pass. *A picture of a
distribution is not the distribution.*

**`MAX_ITERS` is not a lever and now has a reading.** The last column is at most **18 pixels of
921,600**, and it is an upper bound rather than a count: `dbg` sums across every pair covering a
pixel, so four chunks at 140 steps read 560 without any ray having given up.

### Three ceilings, priced in four minutes with no GPU timing

**The technique is `lessons.md`'s *price the ceiling before designing the recovery*, aimed at a
traversal**: patch one literal to a value that is **deliberately incorrect**, build, and count.
The picture is wrong and nobody cares, because a step count is the only output being read.

| an incorrect skip | `deep-water` | `sea-horizon` | `default` |
|---|---|---|---|
| a uniform-water **leaf** steps 16 instead of 4 | **-68%** | **-57%** | 0% |
| a missing **root** cell steps 64 instead of 16 | 0% | -0.5% | **-62%** |
| the coalesced sub-leaf test steps 4 instead of 2 | 0% | -0.4% | -10% |

The third was dropped on that row alone. The other two are complementary -- one is water, the
other is air -- and batch 53 built the exact half of each.

### The coalesced test lifts from the leaf to the root verbatim

`cell_bit` is `x | y<<2 | z<<4` at **both** levels and the root mask is the same `vec2<u32>` a
leaf is, so the four lines the leaf level has run since the tree was written apply one level up
with nothing changed but which mask they read. An empty 32^3 costs one step instead of two to
eight. **-4.90% of DDA steps over the eighteen vantages**, -15.58% at `low-sun`, -12.15% at
`default`, zero at `deep-water` and `cave`, pair counts identical, bit-exact.

There is no rung above it: a 4x4x4 group is the whole root mask, and `root_mask == 0` returns
before the loop starts.

### `dry_mask`: water is in the occupancy tree, and that cuts both ways

`water.md` calls putting water in the tree the load-bearing choice, and it is -- in both
directions. A ray that ignores water is looking at a **different world** from the one the tree
describes: an ocean interior reads as occupied at every level and is empty to it. That is the
submerged primary ray and **every** secondary ray in the frame, since refraction, reflection and
shadow all pass `skip_water = true`.

`dry_mask` is `root_mask` with every 16^3 cell holding nothing but water cleared -- 64 bits per
chunk, built in `build_local`'s existing loop, `GpuChunk` from 80 bytes to 96. `march_chunk`
traverses it instead whenever the ray ignores water, so a water-only cell falls into the
coalesced arm above and costs one step per 16 blocks, or one per 32 when its neighbours are
water too.

**-25.17% at `deep-water` and -16.64% at `sea-horizon`**, pair counts identical, bit-exact.
The root skip is worth **0.00% and -0.04%** at those two, so both figures are this mask's
alone -- and it is worth **nothing** at `underwater` and `coastline`, where the rays hit the
sea floor within tens of blocks and where the camera is dry. The two changes together take the
eighteen-vantage total from **196,470,005 DDA steps to 176,826,842, -10.0%**.

**Three things about it that a later batch needs.**

- **Traversal only.** `mask_below(root_mask, l1b)` still indexes the interned `inners` run,
  which is laid out against the real mask. Using `dry_mask` there reads a *neighbouring node*,
  and a neighbouring node is a plausible picture rather than a crash.
- **Glass and foliage count as dry**, so one mask serves every ray class. A secondary ray
  ignores those too and could have had a looser mask; three masks that are each right for one
  ray are worth less than one that is right for all of them, and no generator places glass in
  open water.
- **Its second customer is `resolve`, and that is where the surprise is.** `resolve` calls
  `march_chunk` itself through the refracted, reflected and shadow legs, and all three pass
  `skip_water = true` -- so `dry_mask` is their mask too. At `coastline`, a camera 24 blocks
  **above** the sea where `march` gains nothing and is 0.012 ms *slower*, `resolve` is
  **-0.503 +/- 0.021 ms**. **A traversal change in this engine has two customers and only one of
  them is the pass it lives in.** Not batch 49's billing effect -- nothing here changes a write.
- **It is root-level, and the ceiling above says that is a quarter of what is there.** The rest
  needs the same summary on `Inner`, which is 16 bytes with no room -- roadmap P9, where the
  cheap shape (a 4-bit *y* bound in `leaf_prefix`'s twenty spare bits) is written down.

### What it cost the module

`shaderstats`, before and after the whole batch:

| pass | words | bytes | registers |
|---|---|---|---|
| `tile_select` | 3,422 -> 3,434 | 8,832 -> 8,832 | 35 -> 35 |
| `march` | 10,169 -> 10,395 | 21,504 -> 21,632 | **72 -> 72** |
| `resolve` | 28,484 -> 28,710 | 554,496 -> 566,016 | **96 -> 96** |

**No register moved anywhere**, which is worth stating because it is the first thing this file
would have reached for. `march` was 72 registers with no spill, no stack and no local memory
before the batch and after it -- and that reading also retired, before a single bench, the
hypothesis that `cell[axis]`'s dynamic vector index was forcing the DDA's state into scratch.
`resolve` grew 11,520 bytes because it inlines `march_chunk` through its three water-ignoring
legs, which is the same reason it stands to *gain* from `dry_mask` without a line of its own
changing.



