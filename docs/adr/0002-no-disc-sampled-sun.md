# ADR 0002 — The sun stays a single ray, because its penumbra is finer than a voxel

**Status: decided and reverted.** Built in full on 2026-09-15 against batch 60, measured, and
removed. **This is a closed door, not unfinished work.** Roadmap `R5` is deleted; it had stood as
*"soft sun, from visibility the bake already has"* since the batch-56 reorganisation and was
named the next batch by the file's own header.

The code is not in git history under a commit of its own — it was reverted from the working tree
— so **this file is the only record, which is why it exists.** It is ADR 0001's shape exactly,
one rung later.

---

## The decision

`shade_hit` fires one `shadow_ray` at the centre of the sun and gets one bit, so every sun shadow
in the world has a hard edge. **That is very nearly correct and it stays.** A real sun is a disc
0.533 degrees across and its shadows do have a penumbra — but in a world of 1-block voxels that
penumbra is smaller than the world's own quantum everywhere it would be seen.

Disc sampling costs **+2.639 ms of a 6.165 ms `resolve`** and moves **471 pixels of 921,600 at a
maximum channel delta of 6**. There is no setting of its one constant that changes that verdict.

## What was built

Four stratified shadow rays across the sun's disc instead of one at its centre, with the disc
rotated per pixel and per frame so the quantisation dithers instead of banding.

- `SUN_ANGULAR_RADIUS = 0.00465` — the physical value, since the disc is 0.533 degrees across.
- `SUN_TAPS = 4u`, placed on a Vogel spiral: `sqrt((k + 0.5) / N)` for the radius so equal-area
  annuli get equal counts, the golden angle for the azimuth so any *prefix* of the sequence stays
  well spread.
- The spiral rotated by `dither(px, py)` — batch 7's interleaved gradient noise, walked one step
  per frame — so the temporal pass averages the five levels into a ramp. This was the part with a
  precedent: `sun_shaft` stratifies its own samples the same way against the same pass, and batch
  7 measured that arrangement removing **90% of what the sampler adds**.
- `FLAG_SOFT_SUN` in `SPEC_MASK`, `--no-soft-sun` the control, gated a second time on
  `smooth_light` so `resolve`'s three secondary copies of `shade_hit` folded back to one ray, as
  batch 45 established for the light model.

**The one part that worked exactly as designed was the control**, and it is worth keeping the
reason. The `else` arm was the pre-61 expression *unedited* — `hit + n * (0.02 * vs)` left inline
rather than hoisted into a name the other arm could share — so `--no-soft-sun` measured **0 pixels
at `default` against a real `voxelcraft-pre61.exe`**, bit-exact by construction rather than by
claim. That is `SPEC_LIGHT_RGB`'s shape and `hit_t`'s invariant applied ahead of time, and it cost
nothing to do.

## What it measured

Against `voxelcraft-pre61.exe`, 1280x720, TAA converged over 32 frames. The radius was swept by
rebuild — batch 48's shape, one constant patched over `target/release/voxelcraft.exe`:

| sun radius | x physical | `default` | `low-sun` | `edits` |
|---|---|---|---|---|
| 0.00465 | **1x, physical** | 471 px, max 6 | 86 px, max 14 | **0 px** |
| 0.01860 | 4x | 1,077 px, max 11 | 279 px, max 14 | — |
| 0.03720 | 8x | 1,758 px, max 12 | 482 px, max 14 | — |
| 0.07440 | 16x | 2,978 px, max 17 | 762 px, max 14 | 24 px, max 5 |

**A sun sixteen times too wide — 8.5 degrees across, the size of a fist at arm's length — moves
0.3% of one frame.** The response is linear in the radius and there is nothing at the end of it.

The cost, paired and alternated, 8 rounds, A = the four-tap build and B = the real revert:

| vantage | pass | A | B | delta | 1 se | t | rounds B > A |
|---|---|---|---|---|---|---|---|
| `default` | resolve (hi-z on) | 8.804 | 6.165 | **-2.639** | 0.043 | -60.97 | 0/8 |
| `default` | resolve (hi-z off) | 8.875 | 6.234 | -2.641 | 0.040 | -65.93 | 0/8 |

**+2.639 ms, or +43% of the pass**, for 471 pixels.

## Why — and this is the half worth carrying

**The disc's rays do not diverge by one voxel until the occluder is 108 blocks away.** Two rays at
opposite edges of the disc separate by `2 * SUN_ANGULAR_RADIUS * d` after `d` blocks, which
reaches 1.0 at `d = 1 / (2 * 0.00465) = 107.5`. Below that every ray in the disc walks **identical
cells** through `march_chunk`'s DDA and returns an identical bit, so the four taps are four copies
of one answer and the shadow is *exactly* hard.

**`edits` reading 0 pixels is that sentence measured rather than argued.** It is the one vantage
built around a structure the player placed, its occluders are tens of blocks away, and the feature
changes nothing there at all. At 16x the radius the threshold falls to about 7 blocks and the same
vantage moves 24 pixels — the mechanism confirmed from the other side, and still nothing worth
paying for.

**Above 108 blocks the penumbra does resolve, and something else already covers it.** `shadow_dist`
is 220, and past 220 the batch-37 light envelope takes over with `shaft::SOFT = 4.0` blocks of
softness — which is *wider* than the physical penumbra out there, since 220 blocks of occluder
distance buys only 1.02 blocks of it. So the band where a disc sun could add anything at all is
roughly **108 to 220 blocks**, and it is narrow at both ends.

**A fourth finding, unasked for and worth one line: four taps cost 6.1x one, not 4x.** The
arithmetic, since the bench measures a *delta* and the ratio is what generalises: `resolve` is
6.165 ms with one march and the march itself is 0.516 (below), so the pass without any sun
shadow at all is **5.649**. The four-tap build is 8.804, so its shadow term is **3.155 ms** --
**6.11x** a single march, and **1.53x** the 2.064 a linear budget would have predicted. Rays a
fraction of a degree apart still diverge enough through the tree to hurt. **Budget a ray count
superlinearly**, and read a paired delta as a delta: 2.639 / 0.516 is 5.1 and is the cost of the
*three extra* taps in units of the first, which is not the same statement.

## Three measurements taken on the way, which the roadmap should not lose

All at `default` unless named, all against `voxelcraft-pre61.exe`:

1. **The whole sun shadow march is 0.516 ms +/- 0.050** of a 6.5 ms `resolve` (t = 10.28, 8/8),
   measured by patching `shadow_dist` to 1.0 so the envelope covers everything and the march
   covers nothing. That is the ceiling on anything aimed at the sun's visibility term.
2. **Shortening the march from 220 blocks to 24 saves only 0.077 ms +/- 0.039** — so **85% of the
   ray's cost is its first 24 blocks**, and a truncation here is not the lever it is elsewhere in
   this tree. It is also nearly invisible: **1,080 px at `default`**, 194 at `low-sun`, but
   **14,348 at `canopy`**, where the trees the heightfield envelope cannot see stop casting.
3. **The shaft envelope is already a better horizon map than a probe bake would be**, which is
   what killed this entry's *original* plan before the disc was built. Both read
   `WorldGen::height`; the envelope won on every axis — `TEXEL = 4` blocks against the `SPACING = 8` the lattice
   then used (L5 later halved the spacing to 4, so this particular axis is now a tie; the
   azimuth, reach and softness points stand),
   the sun's exact azimuth against 16 bins of 22.5 degrees, 1024 blocks of reach against
   `REACH = 192`, and it is already soft. R5 had proposed storing `probe.rs`'s discarded
   `tan_h[16]` and reading it for the sun; that would have been a coarser second copy of something
   the frame recomputes every frame at a cost independent of the view.

## What this ADR did not measure, and what the user found by playing

**Every number above was read off a TAA-converged still.** The user then ran the two diagnostic
binaries and reported something no still could contain:

> *"Soft sun does look better but the main thing i loved about it is that there was 0 pop in with
> soft with the large mountain shadow but for the non soft there was pop in with it although the
> performance hit was large."*

**That is a second defect found by playing, and it is not a fact about soft sun.** It is a fact
about a shadow that pops, which disc sampling happens to *mask*: several rays spread over a few
blocks turn a hard shadow edge into a graded one, so a discrete jump lands on a soft edge instead
of a hard one and the temporal pass has partial values to blend. **At +2.639 ms that is an
expensive accidental fix for a defect that almost certainly has a cheap direct one**, which is why
this ADR still stands and why the finding is filed as `D2` in the roadmap rather than as a reason
to reopen R5.

**The mechanism is one grep and it is in the roadmap entry**: `ray_skips` -- the stochastic LOD
cross-fade -- is called in `march.wgsl` and nowhere else. The primary surface dissolves between
levels; the shadow ray reads whatever single chunk `grid` holds and flips with it.

**The methodological half, and it is this file's most useful paragraph.** The rejection above is
sound on what it measured and was **structurally blind** to what the user saw: `docs/lessons.md`
has warned since batch 23 to point `--anim-rate` at anything that moves on its own, and
`CLAUDE.md` lists *playing it* as the fourth instrument on the strength of batch 54. This session
used the first three and skipped the fourth, then handed the build to the user, who used it
immediately. **A look batch should reach for the fourth instrument before writing the verdict, not
after** -- and a rung whose whole effect is at a *moving* edge cannot be ranked on a still at all.

## What this leaves

**Hard sun shadows are not the gap between this renderer and a path-traced one.** What reads as a
soft shadow in a path tracer is sky light — an area source covering 2π steradians — and that is
the term R1, R2 and R3 have been rebuilding since batch 57, not the sun's half-degree disc.

The next question in that line is the one this session found while looking for R5's premise and
did not build: **`SKY_TINT` in `resolve.wgsl` is a single authored constant**, and nothing in the
frame couples the ambient's *colour* to the sky the renderer actually drew. `GpuFrame` carries
`daylight` and `ambient` as scalars and no sky colour at all, so at sunset the world's shadows
stay blue under an orange sky. It multiplies `amb` on every shaded pixel in the world, it is a
constant-to-derived replacement of exactly the kind the main sequence admits, and the derivation
is one hemispherical integral per frame on a CPU that is idle. It is filed in the roadmap.



