# LOD, streaming and the cross-fade

How the octree picks what to draw, how it streams, and how a transition dissolves instead of
popping. **Consolidated from batch 7** and the streaming architecture. Defects are in
[`errors.md`](errors.md); what a *coarse level looks like* is [`terrain.md`](terrain.md).

---

## Two lists, and the difference between them is the whole behaviour

`lod::Planner::plan` walks the implicit chunk octree once per frame and produces:

- **`selected`** -- what the camera wants, ignoring residency. It drives streaming requests.
- **`render_set`** -- what can actually be drawn now, as `RenderItem { key, fade }`.

`LODError = size * 2.0 - distance(centre, camera)`.

**`render_set` tiles its volume exactly once in expectation.** Outside a transition band each
entry owns its volume outright; inside one a coarse entry and its fine subtree overlap while
**splitting the rays between them**. `tests/lod_stream.rs` asserts the sum of shares at every
probe is 1.0 and that the only overlaps are complementary cross-fade partners.

**Two chunks covering the same volume with the *same* rays is not harmless overdraw.** The
visibility buffer holds one hit per pixel, so both get marched and it keeps whichever surface is
nearer -- a half-refined node shows coarse geometry punching *through* the fine children it
stands in for. A gap is a hole. That is why the fade is a **partition of the rays** and not a
blend weight.

---

## The cross-fade

**`fade` is a signed threshold, not a share.** `+f` marches the rays whose dither falls below
`f` and `-f` the rays at or above it, so a parent and its subtree partition every pixel exactly
once **by construction**. Storing each entry's own share would make the partition depend on two
floats agreeing.

**The schedule is a smoothstep of distance with no history**, and that is load-bearing. The old
deadband needed last frame's answer to produce this frame's, which is what made approach and
retreat pop at different distances; a schedule with no history cannot flutter, because the same
camera position always gives the same plan. Flown both ways in one test run, the deadband
measures 554/659 (**1.19x asymmetry**) and the fade 840/840 (**1.00x**). Worst per-frame LOD step
went **1.000 to 0.062**.

**Smoothstep rather than a linear ramp, for the band edges.** The draw list changes at both
edges and smoothstep is flat at both, so an entry joins and leaves at zero share *and* zero rate
of change -- a camera crossing the outer edge in a single frame cannot make the list change
visible. The cost is that it lingers at tiny `f`, where the fine side is a sparse speckle the
clip mostly pulls back to the coarse mean, which is the right answer at 2% weight.

**`fade_band` is 1.25**: a node is entirely its children inside `size * factor` and entirely
itself outside `size * factor * fade_band`.

**The dither is interleaved gradient noise, not a blue-noise texture.** Void-and-cluster would
want a twentieth binding, and IGN already has the property that matters -- neighbours get
well-separated values, so a threshold cuts an 8x8 tile into a fine interleaved pattern and the
3x3 neighbourhood the temporal clip builds contains **both representations**. That neighbourhood
is the entire mechanism the dissolve resolves by.

**The animation offsets the coordinates, not the value.** Adding a per-frame constant to the
noise output moves every pixel the same way at once and sweeps the dissolve across the screen
like a wipe. Offsetting the sample point keeps the spatial spectrum and gives each pixel its own
sequence over the ~7 frames the accumulator averages.

**`TAA_CLIP_SIGMA` stays at 1.0, and the expectation that it should open was backwards.**
Residual grain goes 1.40 at sigma 1.0, 1.66 at 1.5, 1.89 at 4.0 -- monotonically *worse* as the
box opens. A tight box pulls each history sample toward the neighbourhood mean, and that mean is
the local blend of the two representations, **so the clip is not rejecting the dissolve, it is
performing most of it.**

**The chunk grid resolves a fade to its fine side and secondary rays are not dithered.**
`build_chunk_list` fills the grid coarse-first so finer entries overwrite, which means shadow
rays and cross-chunk AO see one representation. Dithering them too would add noise the temporal
pass has no motion vector for.

**A cross-fade needs both sides to have geometry**, so `Residency` distinguishes `Empty` from
`Solid`. A resident-but-empty node covers a volume fine as a stand-in but as a fade *partner*
would dissolve half the rays into sky. `dissolve` bails to children-only there, and in two other
cases -- an empty subtree, and a subtree already containing a transition. **All three fall back
to a pop, never a hole.**

**Nested bands cannot happen at the default settings and the code does not rely on it.** A
node's band starts at `2 S` and its children's ends at `1.25 S`, with a child centre at most
`0.43 S` off its parent's, so the gap is a factor of 1.6. `--streaming-factor 1.0` closes it,
which is why `dissolve` checks rather than assumes.

---

## Residency: down is free, up is a step

A node whose subtree has not fully streamed draws **itself** as a single stand-in rather than
drawing over its ready children. A selected node that is not resident is covered by its
**resident descendants** first (`cover_depth` levels down) and only then by an ancestor.

That asymmetry is deliberate: on a merge the fine children are still loaded, so descending is
free and keeps the picture identical, where climbing to a grandparent -- or finding nothing at
all -- is a large visible step.

**Do not prefetch the whole ancestor chain of a selected chunk.** The cross-fade adds exactly one
level to `selected` -- the node a subtree is dissolving out of, which *is* drawn -- and it must
not add a second. See [`errors.md`](errors.md) for what happened when it did.

---

## Streaming

Implicit chunk octree. rayon workers generate and build; **the main thread interns under a time
budget (~45 us/chunk)**, so keep main-thread work out of the hot path.

**Coarse LODs resample the same noise at stride 2^n rather than aggregating children**, and
that part is right: `column_field` is one code path with only the stride differing, so the
altitude rules agree across a transition by construction. What no coarse level had a route to
was the player's *deltas*, and batch 30 gave it one -- `journal::apply` aggregates the edits in
each coarse voxel's footprint. See [`persistence.md`](persistence.md).

**An edit does not reach a stand-in that is already resident**, because `apply` reads the
journal once, at build time. The journal names every chunk an edit lands in;
`ChunkManager::absorb_dirty` walks their ancestors and `update` rebuilds the resident ones a few
per frame, against the same `max_in_flight` budget as a missing chunk and **before** them -- an
absent chunk shows nothing, a stale one shows something wrong. Nothing is ever removed to do it:
`insert_local` swaps a rebuilt chunk in when it lands, so no frame has a hole in it.
`rebuild_dirty_now` is the same work on the calling thread, for a capture that has no next
frame to be corrected in.

After an edit, `stream::relight` reads the chunk back out of the tree with
`World::extract_dense` and recomputes lighting; the game loop does at most one chunk per frame.

**A plan belongs to the camera it was planned at, and the headless paths have two cameras.**
`find_cave` and `find_water` cannot choose an eye until the world around the spawn column has
streamed, so the settle that streams it runs at the *spawn* camera -- and `shot_frame` takes
the manager by reference and re-reads that one `render_set` for every accumulated frame, so
nothing downstream can notice the eye has moved since. Both headless paths therefore settle a
second time once the eye is placed, guarded on the eye having actually moved so the vantages
that never move one are untouched by construction. Measured at 1280x720 with the temporal pass
off, the stale plan was worth **52 pixels at `open-sea` and 56 at `shore`** and nothing at the
other twelve. It is not a determinism problem, and batch 30 filed it as one: four extra
`update` calls at an unmoved camera are **0 pixels at all fourteen**.
`re_settling_at_a_moved_camera_gives_that_camera_s_plan` holds both halves -- that the plan
changes, and that it changes *to* the plan a world streamed at that camera has.

**`frame_index` wraps at 64.** Clear of f32 precision in the dither's multiply, and clear of both
the eight jitter phases and the 32 frames a `--screenshot` converges over, so a capture never
sees the pattern repeat and stays deterministic.

---

## `dry_mask` is per chunk, so a coarse chunk carries its own

Batch 53's water-skipping mask (see [`gpu.md`](gpu.md)) is computed in `build_local`, which every
chunk goes through -- coarse ones included, since a coarse chunk is `build_local` over a
downsampled dense array. So a stride-2 chunk's mask describes the stride-2 world, which is the
only world a ray marching that chunk ever sees: the cross-fade **partitions** rays, so no ray
meets both representations and the two masks never have to agree.

That is the same argument the cutout's `vs == 1.0` gate does *not* get to make, and the reason
this one needed no LOD gate at all: a 16^3 cell of water is a 16^3 cell of water at any stride,
where a 4^3 micro-cell inside a stride-2 voxel stands for eight blocks and means nothing.

## The canopy's cliff, and the blue-noise arm that closes it (batch 91, roadmap A5)

**Built, default off: `--tree-blue-noise`.** The partition the cross-fade makes elsewhere is
aided here by one inversion: a near canopy *is* porous (the batch-43 carve keeps `LEAF_FILL`
of a leaf block's 4^3 cells), while the same canopy one level out was an opaque proxy (batch
38c's (2r+1)-squared stamp), so a cross-fade between them partitions rays across a hard
permeability step rather than a shape difference. The measured still is small and bounded
(2,508 px of 921,600 at `lod`, max delta 16, one bbox) because the fixture's cameras never
cross the band during a capture -- motion is where the clamp and the Hi-Z step get their
chance, and A5's capture protocol owns that number before the arm can call itself built.

**The arm's shape is the research's winner and it is priced**: a fractional-opacity scalar
would cost +180% to +250% per coarse ray; thinning a strictly binary mask buys -40%, since a
thinned cell is skipped rather than marched. The rank field is the 3D R3 recurrence
(`phi_3` = 1.4656) evaluated on the voxel lattice with a seed phase: the published-name
blue-noise class, chosen over a hash because white noise keeps the same 62% in *clumps* and
a clumped hole is the same cliff at finer grain. `COARSE_CANOPY_KEEP` mirrors
`render::LEAF_FILL` bit for bit (compile-time pinned, because the claim is mean transmittance
matched to LOD 0), trunks are thinned never, and the off world is the legacy world exactly.

**Batch-90b's acceptance rule binds first**: the arm counts as built when it moves pixels at
`lod` -- and at a camera crossing the band -- past max delta 1, not before; the -40% bench at
a coarse-heavy vantage is then the research's price tag being read back.

## Measuring any of this

**LOD transitions are invisible at eye level and obvious from height.** A band is a shell of
nodes at one distance, and at the default `--cam-height 40` almost all of it is behind a ridge --
a fade-on/fade-off A/B there moves **0.05% of pixels**. `--cam-height 240 --cam-pitch -20` puts
**13%** of the frame in a transition, and that is the vantage every batch-7 number came from.

---

## Controls

| flag | reproduces | cost |
|---|---|---|
| `--no-fade` | the old hysteresis deadband instead of the dissolve. `boundary_flutter_is_damped_without_the_fade` still guards it there | -- |
| `--streaming-factor F` | not a control -- a LOD ground truth at 8 | -- |
| `--no-hiz` | the Hi-Z rejection, isolated | -- |
| `--no-shadow-share` | the pre-62 grid: a fade resolves to the fine half unconditionally | free; the control is the **slower** arm, +0.084 ms at `lod` |
| `--no-offscreen-shadows` | the pre-64 grid: only chunks the frustum keeps are uploaded, so nothing off screen casts a shadow | free; the **feature** costs 0.100 ms of `resolve` at `terraces` and nothing measurable at `default`. `tile_select` is flat |

## The grid every secondary ray reads, and which half of a fade it resolves to

**The cross-fade is a partition of the *primary* rays and there is exactly one place the split
cannot be expressed: `grid`.** It holds one chunk index per LOD-0 cell, it is what `shadow_ray`
walks and what the nine-cell gather reaches through when its neighbourhood crosses a chunk
boundary, and one cell cannot hold two representations. So a secondary ray sees one of the two
halves of a transition, chosen once, on the CPU.

**Until batch 62 it was the fine half, unconditionally, and that was a pop.** `RenderItem::fade`
is a pure function of distance, so the fine half enters `visible` the *instant* a chunk crosses
the band's outer edge -- when its share of the rays is still about zero. The surface was
therefore still ~100% coarse, crisp and stable, while the shadow it cast had already switched to
the fine silhouette in a single frame.

**How much there is to snap**: forcing the grid to the coarse half instead moves **31,530 pixels
of 921,600 at max delta 34** at `lod`. That is the full difference between the two
representations, and it is 66 times the entire effect of the soft sun batch 61 measured and
rejected.

**Batch 62 sorts the grid writes by `RenderItem::share()`**, so the half carrying most of the
rays wins the cell. The discontinuity still exists -- one cell, one chunk -- but it lands in the
*middle* of the fade, the one moment the frame is already in flux and the temporal pass is
already blending two representations of the surface. Outside a transition every share is 1.0, the
tie-break decides, and it is the old rule exactly: coarser written first, finer overwrites. That
is what keeps every vantage with no fading chunk in it bit-exact.

**It is also cheaper**, which was not the goal: `resolve` is **0.084 ms +/- 0.032 faster at
`lod`**, because resolving a fade to the coarse half marches a coarser tree.

### How to measure a pop, since neither a still nor a bench can

**This is the part to reuse.** Batch 61 rejected a feature on still captures and the user then
found this defect by playing the rejected build -- a pop is a *frame-to-frame* discontinuity and
has to be measured as one.

- **What works**: sweep `--streaming-factor` in 0.025 steps with `--no-taa` at `lod`, and diff
  **consecutive** captures. The factor scales the LOD distance thresholds, so it walks the ring
  across a fixed scene exactly as camera motion does with nothing else changing. A pop is a spike
  in one step. Two of four crossings fell from **45,174 to 4,723** and **35,753 to 4,882**.
- **What does not work**: `--cam-dolly` against a static capture. Both arms read **1.1176 MAE**,
  because a moving capture's residual is dominated by disocclusion at every silhouette and a pop
  is buried in it. It was the obvious instrument and it discriminated nothing.

## What may cast a shadow, and why the frustum must not decide it

**Batch 62 decided *which representation* of a chunk the grid holds. Batch 64 decided whether
the chunk is in the grid at all**, and it is the same question one level up: both are answers to
*what may a secondary ray see*, and neither is anything the marcher knows about.

**Until batch 64 there was one chunk list.** `Renderer::build_chunk_list` frustum-culled
`render_set` and the survivors were both what `tile_select` box-tests and what fills `grid` --
so a chunk the frustum rejected was never uploaded, was never named by a grid cell, and
**occluded nothing**. Turn until a mountain leaves the frustum and its shadow leaves the ground
it was falling on, while that ground is still on screen.

**The user found it by playing**, and reported it as *"the shadow should not depend on me the
player but it does depending on my position and angle ... the large mountain shadow becomes
shorter or larger like a weird pop in pop out"*. It is the third defect in this project found
that way and the second in three batches -- see [`harness.md`](harness.md) on why a fixture of
single stills cannot see either of them.

**The two lists are concatenated rather than merged, and the order is what makes that free.**
`partition_for_grid` returns the frustum-culled entries first and the shadow-only entries after
them; `frame.chunk_count` carries the length of the *first* list, and `tile_select` -- the only
reader of that field anywhere -- loops `0 .. chunk_count`. So an entry in the tail is
unreachable from the pass whose cost scales with the list, and the per-tile box test sees
exactly the set it saw before the batch. The tail is reachable only through `grid`, which is
indexed and carries no count.

**The tail is bounded by the lattice and not by a distance of its own.** A chunk covering no
cell of the 32x8x32 grid can never be named by it, so uploading one would be uploading something
nothing can read -- which is why no new constant appears in this batch. That box is 2048 x 512 x
2048 blocks around the camera, already far wider than the frustum and wider than any ray that
reads it. Measured at the `default` camera: **157 chunks drawn and 346 in the grid**, against
`MAX_CHUNKS` = 8191, so the 13-bit chunk id has 23x headroom over the whole set.

**What it costs is `march_chunk` called on grid cells that used to read `NO_CHUNK`**, and the
sign is worth getting right: a ray that now *hits* terminates **early** and is cheaper, so what
is actually paid is every ray that crosses a newly-present chunk and finds nothing. **So the cost
tracks how much geometry a secondary ray traverses and not how many pixels changed colour** --
`terraces` is **0.100 ms of `resolve`** (+/- 0.020, t = -4.98) for 7,843 pixels moved, and
`offscreen-shadow` is indistinguishable from zero (-0.014 +/- 0.047) for 18,468. `tile_select` is
**+0.000 to +0.001 ms** at three vantages in both Hi-Z arms, which is the concatenation above
doing what it is for.

**The pair, at the `offscreen-shadow` vantage** -- `--time 0.32 --cam-yaw 265 --cam-height 80`,
where the slope on the right is shadowed by a ridge past the right edge of the frame:
[before](../screenshots/batch64_offscreen_shadow_before.png) ·
[after](../screenshots/batch64_offscreen_shadow_after.png) ·
[the difference at 8x](../screenshots/batch64_offscreen_shadow_diff.png). **18,468 pixels of
921,600 at max delta 31**, MAE **2.6165** over the `shadowed-slope` crop -- and the whole frame
is MAE 0.2552, which is why the third image exists.

### The band the defect lives in is narrower than it sounds

Two conditions have to hold at once before a frame can show this at all, and knowing both is
what makes a vantage findable instead of lucky:

- **The occluder has to be more than 52 degrees off the view axis** -- the frustum's own
  horizontal half-angle at `--fov 70` and 16:9 -- or it is not culled in the first place.
- **It has to be within `shadow_dist`'s 220 blocks of the ground it darkens.** Past that it is
  batch 37's light envelope that carries the shadow, off a 512x512 terrain height window that no
  frustum touches, so the grid is not what is answering.

The second is what makes a low sun read **zero**: at `--time 0.26` the sun is 5 degrees up, every
shadow runs past 220 blocks, and the envelope has the whole of it. `offscreen-shadow` sits at
`--time 0.32`, where the sun is 44 degrees up and a 100-block ridge casts about 100 blocks.

### The grid is not shadow-only, and that is why this is not a shadow-only fix

`grid` is also what the nine-cell gather reaches through when its neighbourhood crosses a chunk
boundary -- batch 62's own control row says so. Before this batch a cross-chunk lookup into an
off-screen neighbour returned `NO_CHUNK`, which `world_sample` reads as **open sky and not
solid**: the ambient at the very edge of the frame was computed against a world that stopped at
the frustum. That is repaired by the same change and is not separable from it.



