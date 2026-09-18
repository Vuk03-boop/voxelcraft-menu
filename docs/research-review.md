# Research review — the documents, and what each was worth

Seven deep-research documents were commissioned against the topics in
`DO-NOT-READ-CLAUDE.md` §4 (that file has since been deleted) and delivered together. The PDFs are in
[`docs/research/`](research/). **This file is the verdict, so nothing has to re-read 6.4 MB of
PDF.** **Count the entries rather than trusting a sentence about them** -- this heading said
*seven* until an eighth arrived on 2026-09-15 by a different route, which is `ledger.md`'s own
rule about its row count arriving here. Each was read in full and its premises checked against the code; every claim below that
says *verified* names the file and line it was checked at.

---

## The systemic flaw, which matters more than any individual verdict

**Every document that quotes a speedup quotes a projection, not a measurement.**

| Doc | Headline number | What it actually is |
|---|---|---|
| Wavefront | "3.97 ms net reduction, **4.28x** speedup" | modelled from assumed VGPR counts |
| Epipolar | "10.57 ms → 0.69 ms, **15.3x** throughput" | a per-pass budget table for a pipeline that does not exist |
| Ocean | "5.61 ms → approximately **1.76 ms**" | the 5.61 is ours; the 1.76 is invented |
| Radiance Cascades | "**2.45 ms**, target met" | a sum of six estimated pass times |
| Denoising | "**2.74 ms** reconstruction budget" | same shape |

None of these has been run on this engine. This is precisely the failure mode `PERF.md`'s
"Projected vs measured" section exists to warn about — the same shape as
`Voxel Ray Tracing Architecture.pdf`, whose four per-feature estimates all missed, in four
different directions. **Treat every number in all seven as an argument about mechanism, never
as a measurement.** The mechanisms are frequently right. The digits are not evidence.

A second, quieter pattern: several documents quote *our* real numbers back at us (3.98 ms
resolve, 58% of frame, +4.16 ms refraction, +10.57 ms shadow ray) and then extrapolate. The
real number lends unearned credibility to the invented one sitting beside it.

---

## Per document

### WGSL Compute Shader Optimization — **strongest, and batch 13**

Two findings, both real and both cheap:

- **`override` specialization constants.** WGSL `override` compiles to `OpSpecConstant`, and
  the driver folds it *before* dead-code elimination and register allocation. This is the
  direct answer to batch 12's measured **+0.07 ms paid while executing nothing**. wgpu exposes
  it as `PipelineCompilationOptions.constants`.
- **The 16-workgroup trap.** Ampere SMs cap at 16 resident workgroups. An 8×8 = 64-thread
  workgroup is 2 warps, so 16 × 2 = 32 of 48 warp slots — **occupancy pinned at 66.7%
  regardless of register count**. 16×8 (128 threads) reaches 48.

*Verified:* `resolve.wgsl:551` is `@workgroup_size(8,8,1)` and `resolve` contains **no
`var<workgroup>` and no `workgroupBarrier`** — its workgroup shape is arbitrary and free to
change. `taa` is not (`taa.wgsl:121`, `array<vec3<f32>, 100>` = the (8+2)² halo). `march` is not
(its workgroup *is* the 8×8 tile). `tile_select` is `@workgroup_size(4,4,1)` — 16 threads, half
a warp, with half the lanes idle by construction — but that pass is 0.37 ms, so the upside is
small.

*Measured, batch 13.* Both findings were acted on and only one of them was right.

- **The `override` is real and worth what was claimed.** −0.07 ms at the default camera and
  −0.09 down a coastline, which is to the hundredth what batch 12 measured its absence costing.
  But the *mechanism* both documents assumed is wrong: folding the tint out changes `resolve`'s
  register count by **zero** and its machine code by 20 KB. It is a code-size win, not an
  occupancy one, and that distinction is what makes `override` the right fix rather than an
  accidental one.
- **The 16-workgroup trap is wrong in both halves.** At 80 registers, 8×8 sits at **50%**
  occupancy and not 66.7% — the register count binds first and the block cap is never reached, so
  "regardless of register count" is exactly backwards. And 16×8 does not "reach 48": the driver
  answers a 128-thread block by spending 64 registers instead of 80, spilling 16 bytes per
  thread to get there, and lands on 32 warps — the number the document predicted for the shape
  it was arguing against. Shipped anyway, because a lifted ceiling costs nothing, but worth
  **−0.01 ms** at the default camera and −0.09 at the coastline.

Every register figure it gave for this engine ("if resolve requires 40 registers…") was
hypothetical. They are measured now: `PERF.md`, "Occupancy (batch 13)".

### Voxel Ray Tracer Foliage Integration — **premise verified, two corrections**

Solves the blocker `CLAUDE.md` calls "the constraint that shapes the whole batch". A 3-bit
normal field has 8 states; axis-aligned cube faces use 6; **states 6 and 7 are free**, so a
cross-quad's two diagonal planes need no bit reallocation anywhere.

*Verified:* `common.wgsl:370` and `:455` both compute
`h.normal = axis * 2u + select(1u, 0u, pos_dir[axis])` with `axis ∈ {0,1,2}`, giving `0..5`;
`make_key` packs it as `(u64(normal) << 37u)` at `:826`. States 6 and 7 are genuinely unused.

Also worth keeping: branchless two-plane intersection, ray-cone LOD for mip selection without
screen-space derivatives, hashed alpha testing anchored to 3D surface position (so TAA resolves
it rather than fighting it), and the argument for why sub-voxel data must live *in* the 64-bit
key rather than a side buffer — a side write is not atomic with the `atomicMax` and tears.

**Correction 1 — the atomics are wrong for this engine.** Its `commit_visibility` is a
compare-exchange loop over paired `u32` words. We have native `array<atomic<u64>>`
(`common.wgsl:122`) and require `SHADER_INT64_ATOMIC_MIN_MAX` to start. Adopting its version
would be a regression.

**Correction 2 — the alpha channel is already taken.** The doc assumes atlas alpha is opacity
for alpha testing. **Batch 12 made it the per-texel tint mask**: `textures.rs:25` (`px`) writes
0, `:31` (`pxt`) writes 255, and `resolve.wgsl:309` reads `texel.a > 0.0`. Grass tufts want
*both* — an alpha cutout and a biome tint. The clean resolution is the per-block-id foliage
flag batch 14 needs anyway: for a foliage block, alpha means opacity and the tint mask is
implied 1. That disambiguation costs nothing and is worth designing in from the start.

Also note its sample code hardcodes camera forward as `(0,0,1)` when computing view depth,
which is wrong for any camera.

### Real-Time Ocean Water Rendering — **best idea in the set**

Overturns a batch-8 decision that was documented as deliberate. `docs/water.md` says
planar water "keeps the secondary rays coherent across a workgroup, and the sun glint that a
wavy normal would give honestly is an analytic lobe instead."

**The coherence argument only binds the rays that traverse.** Fresnel, the GGX sun glint and
the sky reflection are pure ALU and texture work — they never touch the voxel tree, so a
perturbed normal costs them nothing in divergence. So:

- `n_micro` → Fresnel, specular lobe, sky reflection. Zero traversal cost.
- `n_geo` (flat) → the refraction ray. Full workgroup lockstep preserved; distortion
  reintroduced analytically after the march.
- `slerp(n_geo, n_micro, k ≤ 0.35)` → the traced terrain reflection, bounded divergence.

You keep every coherence property batch 8 bought *and* stop the sea reading as sheet glass.

**The hard part is not the waves, it is the filtering.** A perturbed normal at distance
aliases violently and will overwhelm TAA. Options, cheapest first: Toksvig (isotropic, forces
symmetric highlight widening), LEAN (full slope covariance, anisotropic elongation along
swells, 10 bytes/texel), or **Bruneton analytic slope variance** — zero storage, integrates
sub-pixel wave energy from the spectrum analytically, and is the one that fits an engine with
no normal maps to mip.

Gerstner vs. FFT is a later question; Gerstner needs no VRAM and no compute pass, and is where
to start.

### Voxel Ray Tracing Denoising — **one gem, the rest not applicable**

The gem: **virtual hit point reprojection.** the live caveat in [`errors.md`](errors.md) says reflected content
ghosts because "a per-pixel second motion vector is what would fix it, and there is nowhere in
the visibility key to put one." The answer is that you do not store one — you derive it:

```
P_virt = P_1 + t_2 · d
```

The virtual image sits along the *primary* view direction, extended past the surface by the
secondary hit distance. For curved surfaces this needs a curvature correction; **voxel faces
are planar, so it is analytically exact**. It needs `t_2` in a small `r16float` buffer, not a
bit of the key — which is why the caveat's framing ("nowhere in the key") was looking in the
wrong place.

Also worth keeping: for cardinal voxel normals the SVGF edge-stopping power function collapses
to an **integer equality test** (`n_p · n_q ∈ {1, 0, -1}`), and depth-based history rejection
should be a **world-space plane distance test**, since voxel terracing produces false
disocclusions at grazing angles.

**Not applicable:** the 7-pass SVGF pipeline at 2.74 ms. It solves 1-spp Monte Carlo noise.
This engine does not render at 1 spp — it converges, and TAA already removes **91%** of what
the god-ray sampler adds. Adopting it would cost roughly a third of the frame to fix a problem
we do not have.

### Wavefront Compute Ray Tracing — **right mechanism, unproven diagnosis**

The megakernel argument is real and the queue/indirect-dispatch machinery is described
correctly (LDS compaction, one global atomic per workgroup, `dispatch_workgroups_indirect`,
octahedral packing, ping-pong for multi-bounce).

**But its central causal claim is unproven, and there is a competing explanation it never
considers.** It attributes reflection costing 1.02 ms at 2.2% coverage to warp divergence
(1/32 lane efficiency). Yet `docs/water.md` records that **reflection reaches 768
blocks and fades over 512–1024, where refraction fades out over 96–192** — reflection rays are
4–8× longer. Measured cost per unit coverage is 0.46 vs 0.12 ms, about **3.9×**. Ray length
alone explains the entire gap with no divergence required.

Which explanation is right changes the fix completely: divergence wants a wavefront refactor,
ray length wants a shorter reflection reach. **Batch 13's register numbers came back and they do
not support the wavefront reading.** `resolve` uses 80 registers at the shape it shipped with,
with **zero spill and zero stack frame**, and raising its occupancy by a third moved the pass by
0.01 ms at the default camera. The megakernel is not starving the register file and the pass is
not waiting on occupancy, so a 4.28× projection modelled on assumed VGPR counts has nothing
underneath it here. **The ray-length explanation is the one left standing**, and it is also the
cheap one to act on.

Its own analysis rejects Morton/radix ray sorting as not paying for itself on a shallow tree —
that conclusion is worth keeping.

### Epipolar Crepuscular Rays — **sound technique, prerequisite we do not have**

Epipolar sampling plus a 1D min-max mipmap is the correct answer to terrain-cast shafts, and
it is the technique batch 11 was missing when it rejected the brute-force version at +10.57 ms.
Sparse boundary samples with interpolation between them, hierarchical space skipping in
O(log K) instead of O(N), cross-bilateral upsample with explicit halo prevention.

**The blocker is structural: it needs a cascaded shadow map.** The doc is explicit — slices are
sampled from CSM cascades, and it recommends 3 cascades at 128/384/768 blocks, populated by an
orthographic compute projection with `atomicMin`. This engine has no shadow map of any kind and
has never needed one; it has a visibility buffer and a DDA. It also assumes a full-resolution
linear depth texture, which we do not have either (depth lives as 24 bits inside the key).

That is a whole new subsystem standing between here and the first shaft. **Park it** — and
reconsider if anything else ever wants a CSM.

### Voxel Radiance Cascades — **correct choice, wrong scale**

The comparison is sound: RC beats VXGI for a sparse voxel tree, because VXGI needs dense
anisotropic clipmaps (250+ MB, re-filtered every frame) while RC marches the tree we already
have and reuses the coarse LODs as far-field emitters. The penumbra-theorem framing is right,
the anisotropic-vs-isotropic leaking argument is right, and the ringing fix (bilinear 3D
ray-tip interpolation) is a real detail.

**But it is a rewrite, not a batch.** It replaces `light.rs`, the 4³ light bricks, the uniform
flags, `stream::relight`, and batch 3's entire per-corner smooth lighting model — 0.61 ms that
was carefully bought and measured. Its own budget is 2.45 ms, or **+36% of the current frame**,
and that figure is projected.

The engine is also not GI-limited: `docs/shading.md` records crushed pixels at
**0.14%** in a night capture after the ambient floor moved out of the AO product. Caves read
correctly today.

Worth revisiting only as a deliberate multi-session project, never as "batch N".

---

## What this changed in the roadmap

| Batch | Was | Now |
|---|---|---|
| 13 | non-cube foliage | **occupancy: measure, then two cheap fixes** — shipped |
| 14 | — | non-cube foliage |
| 15 | — | water micro-normals |
| 16 | — | `tile_select`'s warp shape, which batch 13's numbers turned up |

Batch 13 existed because three of these seven documents rest on one unmeasured claim, that claim
was cheap to test, and it pointed at the pass that is 58% of the frame. Its control was bit-exact
*by construction* — a workgroup size and a folded specialization constant cannot change
arithmetic — and it held: eighteen captures byte-identical, plus two odd resolutions for the
dispatch rounding.

**The claim did not hold.** `resolve` is not register-starved and is not occupancy-limited, and
nothing measured says what it *is* bound by. Three of the seven documents are weaker for it, and
the honest position on the remaining four is unchanged: the mechanisms may be right, the digits
are still not evidence.

---

# The three texture-variation documents — batch 36

Commissioned against batch 36's three prompts, delivered together as `.docx`. **They are the
first set to answer the standing complaint above**, and the difference is worth naming before
anything else: where the original seven quoted projections modelled from assumed values, these
quote timings attributed to named authors, on named hardware, at a stated resolution, for a
stated workload. That is the shape a number has to have to be worth reading. It is still not a
measurement *of this engine*, and none of them was run here.

A second difference: **they disagree with each other**, which the original seven never did. That
is a feature. Two independent answers that diverge tell you where the real question is.

## The systemic note, updated

**Every formula in all three is an embedded PNG**, because the tool rendered its maths as
pictures. A naive text extraction silently drops every number and leaves the prose reading as
though the figures were never there — "Baseline standard single-tap texture lookup: ." Anything
reading these files again has to pull `word/media/*.png` and look at them. The benchmark table
below was recovered that way and not from the text.

## What they agree on, and what it bought

- **The corduroy has a shipped fix with a name.** OptiFine's `natural.properties` binds each
  texture to a symmetry group, and its shipped binding for `grass_block_side` is mode **F,
  horizontal reflection only** — rotation would swing the grass rim onto the side or the bottom
  of the face, mirroring leaves the fringe *height* fixed while alternating its profile. That is
  batch 36's `perm::FLIP_U`, arrived at by somebody else first. Minecraft proper does the same
  thing one level up, hashing integer world coordinates to pick a blockstate variant.
- **Multi-tap stochastic texturing is out, on two independent grounds**, and only one of them is
  cost. It needs footprint derivatives a compute pass does not have; and blending three taps
  interpolates texel values, which destroys nearest-filtered pixel art *by construction*. The
  second argument would hold at any price.
- **A dihedral permutation is exact on this atlas.** 16x16, power of two, `mag_filter: Nearest`:
  a mirror or transpose is a bijection of the texel grid, so no resampling and no intermediate
  colour. It commutes with box downsampling, so every mip is the permutation of the mip.
  Verified independently in `permute_uv`'s own comment and pinned by the mip chain being
  untouched.
- **The hash key must not touch a ray.** Key it on the integer voxel coordinate, never the float
  hit position, or `taa`'s jitter flips the key along face seams and the 3x3 clip reads a static
  world as a moving one. `perm_key` does this, and `box_min` was already exact.

## The benchmark table, recovered from the embedded images

| Source | Hardware / workload | Figure |
|---|---|---|
| Mikkelsen, JCGT 2022 (hex-tiling) | RTX 2080 Ti, 3840x2160, fullscreen quad | single tap **0.27 ms** |
| " | " | 3-tap stochastic triangular **1.09 ms** (4.03x) |
| " | " | 3-tap hex **0.88 ms** (3.25x) |
| " | " | 2-tap barycentric truncation **0.61 ms** (2.25x) |
| Heitz & Neyret, HPG 2018 | GTX 980, 1920x1080, one diffuse material | **1.2 ms** |

**Treat the ratios and not the milliseconds.** The ratios are the mechanism — a tap costs a tap,
and three of them cost three — and they are what rules multi-tap out of a pass that is 65% of
this frame. The absolute figures are somebody else's pass on somebody else's GPU.

## The one mechanism worth more than the batch

**"It costs milliseconds while executing nothing" finally has a named cause: the L0 instruction
cache.** Both documents that touch cost say the same thing — a compute kernel's compiled binary
competes for a per-WGP / per-SM-subpartition instruction cache of a few tens of KB, and once the
working set exceeds it a wave cannot issue *while waiting for its own next instruction*, which
defeats latency hiding across the whole kernel including the paths that are not taken.

This is the first mechanism offered for batch 12's **+0.07 ms** and batch 14's **+1.24 to +2.13
ms**, and it is consistent with what batch 13 actually measured: register count unchanged,
machine code 20 KB larger, and the fold worth exactly what the absence had cost. It also predicts
the counter that would confirm it — `Wait inst fetch` in AMD RGP, or
`smsp__warp_issue_stalled_wait_inst_fetch_percent` ("Warp Stall: No Instruction") in Nsight
Compute — neither of which this project has ever looked at. **That is a measurement nobody here
has taken and it would settle a four-batch-old question.** It is not batch 36.

## Where they disagree, and how the code settles it

**The sampler.** Document A concludes the atlas sampler "must be configured to `Repeat`";
document C concludes `ClampToEdge` "should be retained". Both are right about the hardware — all
three agree that an address mode applies *within* an array layer and cannot bleed across slices,
so the inter-layer-bleed premise is false — and document C supplies the reason a *ray-traced*
pipeline keeps clamping anyway: a DDA hit lands on a face boundary at exactly 0.0 or 1.0, and a
float drift to -0.0001 under `Repeat` wraps to the opposite edge and sparkles.

**The engine needed neither.** `shade_hit` already clamps `local` to [0, 1], so the sparkle
cannot occur here; and batch 36 chose a permutation, which stays inside 0..1. `make_atlas` is
untouched.

## What one of them got wrong about us

Document A asserts a 29 KB atlas "fits comfortably within the 32 KB L1" and that expanding to 8
variants at ~230 KB "slightly exceeds" it — then reasons at length about L1-vs-L2 latency for the
variant case. The arithmetic is the giveaway: 21 layers x 16x16 x RGBA8 with 4 mips is **~28.6
KB**, and it is not obvious that a texture cache holds a whole atlas resident against everything
else the pass is reading. The conclusion it draws — that variants are cheap — may well be right;
the confidence is unearned, and it is the same shape as the original seven. **If batch 36b ships
variants it has to measure the atlas, not quote this.**

## The claim batch 36 declined to take

All three recommend N procedural variants per material as the primary fix, with permutation as a
supplement. **Batch 36 shipped only the permutation**, on grounds none of the documents address:
the two fix the same defect, so shipping both at once means no measurement can say which paid.
The permutation is free — zero extra fetches, zero atlas bytes — and the variant half is not.
Rank the cheap one first; `docs/roadmap.md` carries what is left.
---

# The three transparency documents — batch 38

Commissioned against batch 38's three prompts, delivered together as `.docx`: one on the cutout
test inside a DDA, one on a second hit under a single-hit visibility buffer, one on what a
see-through block does to every ray that is not primary.

**The evidence standard the prompts carried was answered, and that is the first thing worth
recording.** Figures arrive with a named source, named hardware, a stated resolution and a
stated workload; every option carries its own falsification threshold with the counter to read;
two of the three close by listing which engine premise each conclusion rests on, which is
exactly what made the corrections below cheap to write. Where nothing is published, they say so
rather than inventing a number.

**The formulas are embedded PNGs again** — 79, 120 and 108 of them across the three files. A
naive text extraction silently drops every figure and leaves the prose reading as though the
numbers were never there. Everything quoted below was recovered from `word/media/*.png` and read
as images, the same way the batch-36 benchmark table was. **Anything reading these files again
has to do the same.**

---

## What all three agree on, and it is the batch

**Leaves stop being a texture test and become geometry: a 4³ sub-voxel occupancy mask, evaluated
as a fourth tree tier when the DDA steps into a voxel whose block id says cutout.** Three
documents reach it from three different directions — the first on traversal cost, the second on
what a hit is allowed to be, the third on what survives coarsening — and none of them needs the
others to get there.

The argument that matters is not the performance one. It is that **a 4³ mask dissolves the alpha
channel conflict instead of working around it**. `textures.rs:123` builds `LEAVES` with `pxt`,
so its alpha is 255 meaning *tintable*, and leaves are in the tint set; batch 14's trick — for a
foliage block alpha means opacity and the tint mask is implied 1 — cannot extend to a block that
needs both meanings at once. Put the holes in the tree and no texel is ever discarded: the atlas
alpha stays a tint mask for every block, and `make_atlas` is untouched. The same fix also
retires `Castaño` coverage renormalization and footprint-scaled thresholds before either is
built, because a cutout that is geometry has no alpha to mip.

The secondary agreements, each with its own reasoning:

- **The inline any-hit test is out**, and the grounds are the same ones that ruled out multi-tap
  in batch 36: it puts a texture fetch and a UV reconstruction inside the inner DDA loop, on the
  *failure* path, where a miss costs 50–150+ cycles against 4–8 for an empty step.
- **Hashed alpha is out for leaves**, and this is where the three are least unanimous — the first
  rejects it outright on TAA clipping, the third scores it "high" for silhouette retention and
  "poor" for temporal stability, the second keeps it as an option. All three agree it is
  *incompatible with the tint channel*, which settles it here for the same reason the inline test
  is settled.
- **`override` specialization is the right remedy for compiled-but-unexecuted code**, not dynamic
  branching, and the third document prices the alternative: a bitwise material/ray collision mask
  inside a megakernel at 22–34 KB and 88% I-cache hit rate, against 12–18 KB per entry point and
  a stated zero regression for permuted kernels. That is the shape `SPEC_MASK` already is.

## What each one adds that the others do not

**The first** supplies the mechanism and the sizing. 2³ "fails to read as foliage"; 8³ needs
eight `u64`s, up to 24 micro-steps, and projects to subpixel noise past 20 m; **4³ is 64 cells in
one native `u64`, which is the same bitmask arithmetic `march_chunk` already compiles for level
0**. 32 canonical templates is 256 bytes of constant table, and chunk VRAM does not move —
geometry and attributes are already decoupled, so a block id indexes a shared template.

**The second** is the one that changes the batch's shape: **glass and leaves are not one
feature, and unifying them is an anti-pattern.** Depth complexity K ∈ [3,10] for a canopy against
K ∈ [2,4] for glass; diffuse binary cutout against Fresnel, refraction and Beer-Lambert; and
every unified design fails in one direction or the other — secondary rays are right for a
window and cost an estimated 12–18 ms at ten per pixel in a forest, a K-buffer deep enough for a
canopy is 66.36 MB at K=4 and still cannot bend a refracted ray, and stochastic handles ten
layers of leaves but puts screen-door noise on a sharp refractive background. **Its
recommendation is that leaves keep K=1** — a sub-voxel leaf hit is an ordinary opaque hit, Hi-Z
keeps working, `resolve` shades it through the existing path — **and that glass alone gets the
water model generalized**, capped at two interfaces, tagged by one of the free bits.

Its arithmetic checks: 1920×1080×8 B is 16.59 MB, K=2 is 33.18, K=4 is 66.36, a 32-bit head
buffer is 8.29. That is the thing the batch-36 review caught document A failing, and it is worth
saying when a document gets it right.

**The third** names the risk the other two create and do not see. **The discrete LOD cliff is the
critical structural failure mode**: a porous LOD 0 canopy against a solid stride-2 cube, across a
cross-fade that partitions rays rather than blending samples, gives adjacent pixels depth
disparities of several voxels — which breaks the 3×3 neighbourhood clamp, and drives the per-tile
Hi-Z to the canopy's outer shell so that chunks behind the tree are culled from under the LOD 0
rays that can still see them. Its fix is the good part: **keep the coarse mask strictly binary
and decimate it with a 3D blue-noise rank field**, so mean transmittance matches LOD 0 without a
float, a hash or a texture tap entering the coarse traversal. It prices a fractional-opacity
scalar at +180% to +250% per coarse ray and blue-noise decimation at **−40%**, because a thinned
mask skips more.

It also settles the light flood cheaply: σ per block with `max(0, L − σ)` is Bellman-Ford over a
lattice with non-negative weights, so **adding absorption strictly reduces the iteration count**
— foliage at σ=2 reaches zero in ⌈15/2⌉ = 8 steps instead of 15 — and a per-chunk `is_foliage`
bitmask alongside the occupancy mask keeps it branchless at three ALU ops. And it offers, for
shadows, the option that costs least: **0.20× of an opaque shadow ray**, by not dispatching one
through foliage at all and letting the air-voxel grid carry the attenuation the nine-cell gather
already reads.

---

## Corrections, each checked against the code

**Correction 1 — the six free bits hold a 4³ micro-coordinate exactly, and no document saw it.**
The second states the six bits are "structurally incapable of holding secondary geometry" and
should become metadata flags. That is right for a second *hit* — 58 bits minimum, as it argues —
and wrong for a sub-voxel *coordinate*, which is what this design needs. A 4³ cell is 2 bits per
axis; the free bits are bit 6 and 7 of each of the three voxel bytes; **6 needed, 6 free, one per
axis pair.** That is what makes the first document's recommendation implementable without
touching `make_key`'s layout at all, and it is the single most load-bearing fact in the set.

*Verified:* `common.wgsl:1218` packs `(v.x << 16u) | (v.y << 8u) | v.z` with `v` in 0..63, so bits
6–7, 14–15 and 22–23 are free — the count `CLAUDE.md` already carries. **The seam it creates is on
the read side**: `resolve.wgsl:1010` decodes with `& 0xFFu` per byte, so it currently reads those
bits as part of the coordinate and would have to narrow to `& 0x3Fu` with the micro-cell pulled
out separately. One writer, one reader, and a test belongs on the pair.

**Correction 2 — the recovery pass does not rescue Hi-Z from a non-occluding hit, and the second
document assumes it does.** It reasons that a glass pane poisoning the per-tile depth is caught
by "the lossless recovery pass", leaving a performance loss rather than a correctness one.
**Recovery re-tests rejected pairs against the *fresh* Hi-Z, which is built the same frame from
the same poisoned visibility buffer**, so it fixes *staleness* and not *wrongness*; a chunk
behind the glass is rejected by both tests and the geometry genuinely drops out.

*Verified:* `tile_select.wgsl`'s `recover_select` tests `depth_key(dmin) >= hiz[pair >> 13u]`,
and `build_hiz` (`march.wgsl`) takes `atomicMin` over the same frame's `vis`. The consequence is
that the document's preferred remedy — a separate opaque-only depth buffer feeding the Hi-Z — is
**mandatory for glass rather than an optimisation**, and its cheaper alternative (clear an
`OCCLUSION_ELIGIBLE` bit and let the tile collapse to the far plane) is the conservative one.
**None of this touches leaves**: a sub-voxel leaf hit is a real opaque surface at a real depth,
so Hi-Z stays correct by construction. It is an argument for splitting the batch, not for
enlarging it.

**Correction 3 — the LOD cliff is real here, and more real than the document could know.**
The third reasons about it from the prompt's description. `worldgen.rs:668`'s `coarse_trees`
stamps `b.leaf` at every stride up to 8, so **leaves are not LOD 0 only the way ground cover is**
— the cliff is reachable at stride 2, 4 and 8, in the default frame, and `--streaming-factor`
can put a transition where a capture can see it. The premise holds and the risk ranks where the
document puts it.

**Correction 4 — the light-flood premise holds exactly.** `light.rs:140` is `let next = l - 1`
with `opaque(dense[ni])` as a hard stop, which is the relaxation the third document models, so σ
per block is a one-line generalization of a loop that already has the right shape and its
monotonicity argument transfers without qualification.

**Correction 5 — the first document's projection does not reproduce from its own figures.** It
states an inflation factor of 2.5–3.2× for an inline test, then projects `march` from 2.12 ms to
"approximately 3.8 to 4.5 ms" — which is 1.79–2.12×, not 2.5–3.2×. The arithmetic only closes if
the ratio is blended over the 40% of tiles it says carry foliage (0.6 + 0.4 × 2.5 = 1.6 → 3.4 ms;
0.6 + 0.4 × 3.2 = 1.88 → 4.0 ms), which it never says. Not fatal, and it argues *against* the
option it rejects — but it is the projection shape, and **2.12 ms and 5.48 ms are our numbers
being quoted back at us**, which is the pattern the seven original documents established.

**One attribution could not be checked.** The 1.84 ms → 6.21 ms / **3.37×** figure for a software
DDA with and without an inline alpha test is credited to "DubiousConst282, VoxelRT benchmark
repository, 2024" on an RX 6800 XT at 2560×1440. The author and the repository are real — the
64-tree article from the same source is already cited in `roadmap.md` — but whether that repo
publishes that benchmark table was not verified here. Treat the **ratio** as the argument, which
is what the prompt asked for and what the evidence standard allows.

---

## What this changed in the roadmap

| Entry | Was | Now |
|---|---|---|
| 38 | leaves and glass stop being opaque cubes | **38: leaves, as 4³ sub-voxel occupancy masks** — the cutout becomes geometry |
| — | — | **38b: glass, as a generalized water model** — secondary ray, K=2, and the Hi-Z bifurcation Correction 2 makes mandatory |

**The split is the documents' finding and not a scope cut.** The second document argues the two
are different features on optics, on depth complexity and on which pipeline can serve them; the
first and third only ever answer for leaves; and Correction 2 says the glass half drags in a
second depth buffer that the leaf half does not need. Shipping both together would also make the
measurement unreadable, which is the same argument batch 36 used to decline the variants half of
its own research.

---

# The three water documents — P1

Commissioned against P1's three prompts — one on the sky term every water pixel pays, one on
the two secondary rays, one on what a compute megakernel is actually bound by — and delivered
together as `.docx`.

**The evidence standard was answered in shape, and only partly in substance.** All three carry
named sources with hardware, resolution and workload; all three state an engine premise per
recommendation and a falsification threshold with the counter to read; and two of them write
"not published" where nothing is published rather than inventing a figure — the perceptual
question in particular comes back honestly empty three separate times. What none of them does
is keep *our* numbers out of their own arithmetic. **That is legitimate in one place and the old
failure in another**, and the difference is worth naming because it is the first time the
distinction has been visible: the second document computes 4.16/35% against 1.02/2.2% and
compares the ratio to 768/192, which is our numbers being *checked against each other* and is
the most valuable paragraph in the set; the first document turns the same habit into "if this
fails to reduce resolve by at least 3.0 ms", which is a projection wearing a threshold's
clothes.

**The formulas are embedded PNGs again — 104, 73 and 202 of them — and the prompt asked for
text in as many words.** Three sets of documents have now done this and one of them was
explicitly told not to. **Stop asking; budget the extraction instead.** Everything quoted below
came out of `word/media/*.png` read as images, and the Nsight thresholds in particular exist
*only* there — a plain text extraction of the third document yields a decision procedure with
every number missing and reads as though it never had any.

**They overlap rather than partition, and that is the second useful thing about them.** The
document named for the megakernel answers all three prompts; the one named for secondary rays
answers two. So the set contains three independent passes at the screen-space question and two
at everything else, and **where two documents reached the same conclusion from different
directions it is recorded as agreement below rather than repeated**.

---

## What they agree on, and what each agreement is worth

- **Screen space is dead for water here, and the refraction half is dead by construction.** The
  primary march stops *on* the water surface and writes that hit; the seabed is never in the
  visibility buffer, so the screen-space hit rate for refraction is **exactly 0%** — not low,
  zero. For reflection both documents that price it land in the same band from production data
  (Lumen on open water **under 15%**; Fortnite Chapter 4 water reflections toward the horizon
  **under 10%**; CryEngine 3 / Killzone measurements **under 20%** at horizontal view angles,
  falling further looking down), against **60–85%** for enclosed interiors, which is the number
  every SSR paper is quoting. **This retires the second half of prompt 2 outright** and it is
  the cleanest kill in the set.
- **Ray length, not divergence, and now with a citation.** The third prompt's suspicion is
  confirmed from Aila & Laine (2009, GTX 285) and Aila, Laine & Karras (2012, GTX 480/680):
  SIMD divergence accounts for **under 15–20%** of traversal runtime for coherent bundles, and
  their own intra-warp compaction experiment using POPC/ballot **cost more than the idle lanes
  it removed**. Laine, Karras & Aila (2013, GTX Titan) put wavefront queue management at
  **0.5–2.0 ms** of fixed overhead, paying only past ~8 distinct complex materials; Morton-code
  ray sorting is **0.6–1.5 ms** for 1–2M rays, which is more than the entire 1.02 ms reflection
  budget it would be optimising. **Every wavefront, compaction and sorting option is priced out
  of this engine by published measurement**, which is what `errors.md` had on suspicion.
- **The Fresnel early-out is out, and both documents that examine it reach the same failure.** A
  hard threshold draws a step along an iso-angle contour; the discontinuity is orders of
  magnitude above visual edge detection; the 3x3 neighbourhood clamp cannot absorb it because a
  pixel crossing the boundary has its history clipped rather than blended, so it flickers; and
  the smooth version has to evaluate the sky in order to scale it, which is the whole cost back.
  Russian roulette is named and rejected for the same reason it is rejected everywhere here —
  this pass converges, it does not denoise.
- **The cloud FBM is the sky, and the gradient is free.** Both documents that split it put the
  analytic atmosphere at **18–35 SASS instructions, zero memory traffic**, against **420–780**
  for the noise, and the published sky-LUT timings back it: a whole hemispherical scattering LUT
  is **0.036 ms on a GTX 1060** and **0.008 ms on a 4090**. **Nothing about `sky_base` is worth
  optimising.** If the sky term is expensive here it is `cloud_fbm` and nothing else.

---

## The single most valuable thing in the set: a decision procedure for what `resolve` is bound by

`gpu.md` has closed with "nothing yet says what it *is* bound by" since batch 17. **This is the
first artefact that could answer it**, and it is the third prompt's question answered exactly as
asked — thresholds and an order, not a metric list. Recovered from the images.

**The frontend, cited to Jia & Van Sandt (Citadel, GTC 2021 S33322), Accel-Sim, and *Analyzing
Modern NVIDIA GPU Cores* (arXiv 2503.20481):**

| structure | per SMSP | per SM | line | note |
|---|---|---|---|---|
| L0 instruction cache | **2–4 KB** (128–256 instructions) | 8–16 KB aggregate | 64 B (4 instructions) | private, direct-mapped or 2-way |
| L1 instruction cache | shared | **16–32 KB** | 128 B (8 instructions) | 4–8 way across the 4 SMSPs |
| instruction buffer | 3–4 entries per warp | 48–64 per SMSP | — | per-warp FIFO between fetch and issue |

An Ampere SASS instruction is **16 bytes** — 8 of opcode, 8 of control word. So `resolve` is
**~18,816 instructions at 294 KB** and **~39,360 at 615 KB**, and *both* overflow even the L1
instruction cache by more than ten times. **The document's own conclusion from that is the
important half: aggregate binary size cannot be the mechanism, because the small build overflows
too.** What it proposes instead is that the *hot loop* fits in L0 at 294 KB and stops fitting at
615 KB, once the allocator has spread basic blocks and inserted preservation code. That is a
hypothesis, not a measurement, and it is the one the procedure below exists to test.

**The order is strict and each step disqualifies the next:**

| step | metric | says yes | says no |
|---|---|---|---|
| 1 | `smsp__warp_issue_stalled_no_instruction_per_warp_active.pct` | **> 15%** with `smsp__instruction_cache_hit_rate.pct` **< 85%** → instruction-cache thrashing | < 5%, or hit rate > 95% |
| 2 | `smsp__warp_issue_stalled_long_scoreboard_per_warp_active.pct` | **> 30%** with `l1tex__average_hit_rate.pct` **< 60%** → memory latency | < 10% |
| 3 | `smsp__warp_issue_stalled_math_pipe_throttle_per_warp_active.pct` | **> 20%** with `sm__pipe_alu_cycles_active...pct_of_peak` **> 70%** → compute bound | ALU < 40% |
| 4 | `smsp__thread_inst_executed_per_inst_executed.ratio` | **< 16.0** of 32 → SIMT divergence | > 28.0 |
| — | `l1tex__tex_throughput.avg.pct_of_peak_sustained_elapsed` | > 75% → texture bound | < 25% |

Step 1 positive with step 2 negative is the conclusive reading for instruction-cache thrashing.

**And the profiler is not required.** The third prompt asked whether a controlled binary-size
sweep could locate the cliff with the tools already here, and the answer is yes, with a protocol:
inject unrolled arithmetic behind a specialization constant, made to depend on a live dynamic
input so the compiler cannot eliminate it; build variants stepping the binary in fixed
increments with the register count pinned; bench each one paired and alternated; **plot time
against size and look for a step rather than a slope.** A slope is ordinary fetch overhead; a
discontinuity across one increment is the capacity threshold. `harness bench` and `SPEC_MASK`
are both halves of that already, and it would settle a question four batches old.

**Batch 47 answered a piece of it by a cheaper route, and the protocol is still worth running
for the rest.** To ask whether the nine-cell light gather is memory-bound, it built the variant
that changes *only* that block's memory behaviour -- all nine reads pointed at one cell, so the
instructions are identical (`resolve` moves 640 bytes) and the memory system sees one address
instead of nine. **It recovered 10-13% of the block**, so that block is instruction-bound. Two
diagnostic builds, twenty seconds each, no profiler and no prototype. **The protocol above is
still the only thing on file that can ask the question about the *pass* rather than about one
block**, but the technique generalises to any block anyone suspects, and it is far cheaper.

---

## What each document adds that the others do not

**The sky document supplies the one derivation worth keeping, and it is exactly right.** For a
camera-centred directional lookup against a cloud deck at finite altitude, the world-space
displacement on the deck is

```
dP = 2 * (horizontal distance from camera to the water hit)
```

**independent of the deck's altitude entirely** — the altitude changes only the angle that
displacement subtends at the eye. *Verified here algebraically rather than taken*: with the
mirror direction `r` being `d` with its Y negated, the two intersections differ by
`t_w * (d + r)`, whose Y component cancels and whose horizontal part is twice the camera-to-hit
horizontal offset. It is a clean invariant, it is the reason every cubemap, octahedral map and
SH cache is wrong for this engine rather than merely approximate, and **no document in the
previous ten offered anything of this shape.** Its recommendation follows from it: a **top-down
2D planar cloud buffer** rendered once per frame, sampled by an exact ray-plane intersection,
preserves world-anchored parallax by construction and replaces the FBM with one bilinear fetch.

**The secondary-ray document supplies the arithmetic that closes prompt 2's central question.**
Refraction is 4.16 ms over 35% coverage, reflection 1.02 over 2.2% — unit costs of 0.1189 and
0.4636 per 1% of frame, a ratio of **3.90**, against a reach ratio of 768/192 = **4.0**. Those
agree to 2.5%, and with the reflection normal clamped flat the rays across a workgroup are a
parallel beam that steps through identical chunk strata, so there is no divergence term left to
find. Its recommendation is the cheap one: **reflection reach 768 → 384, fading 256–384**, for
about half the reflection DDA cycles, on the argument that haze has already converged distant
terrain to the horizon sky before 384 blocks and the residual contrast falls under 8-bit
quantisation.

**The megakernel document supplies the frontend numbers above, and one correction to its own
recommendation that it does not notice** — see below.

---

## Corrections, each checked against the code

**Correction 1 — `CLOUD_OCTAVES` is 4, not 5, and every instruction count in two documents is
built on the 5.** Both the sky and megakernel documents cost "a typical 5-octave FBM
configuration" at 420–780 instructions and price the remedy as "5 → 2, a 60% reduction".
*Verified:* `common.wgsl:1657` is `const CLOUD_OCTAVES: u32 = 4u`. The direction of every
conclusion survives — the noise still dominates the gradient — but the headline saving is
overstated by a fifth before anything else is checked, and "5 → 2" is really "4 → 2".

**Correction 2 — a secondary ray's cost is not its DDA steps, and both documents model it as
though it were.** Every recommendation about reach and stride treats a secondary ray as a
traversal loop. It is not: a refraction or reflection ray that *hits* calls `shade_hit`, and
`shade_hit` casts **a third ray** — `resolve.wgsl:493` is the `shadow_ray` — plus the nine-cell
`gather_face` and, past `shadow_dist`, batch 37's `terrain_shade_beyond`. The refraction leg
passes `WATER_SHADOW_DIST` and the reflection leg passes the full `frame.shadow_dist` of 220.
**`water.md` already carries the measurement that makes this decisive**: water uncapped read
7.62 ms against 4.93 with water opaque to shadows, so the shadow ray at the end of the
refraction was two thirds of the whole cost of water before `WATER_SHADOW_DIST` capped it at 48.
A refraction ray in shallow water terminates on the seabed within a few blocks and then pays a
48-block shadow ray and a nine-cell gather. **The 3.90-against-4.0 agreement in Correction-free
form is therefore weaker than it looks** — it compares *maximum reach* rather than mean path
length, and refraction's mean path is nowhere near its 192-block cap. The ray-length conclusion
for *reflection* stands, because a reflection over open water genuinely misses and runs its full
reach. The stride-2 recommendation for *refraction* rests on the half of the cost that may not
be there. **Price `shade_hit` before optimising the walk.**

**Correction 3 — the band limit already culls octaves, but only where it does not matter.**
`cloud_fbm` weights each octave by `1 - smoothstep(0.25, 0.5, fp * freq)` and skips it entirely
at zero, and `shade_water` passes `travelled = t` so the reflected cone is correctly widened.
Both documents recommend octave truncation for secondary rays without noticing that a
footprint-driven version of it is already running. Worked through at 70 degrees vertical FOV and
1080p (`px_angle` 0.0013), with the deck 320 blocks above the sea and a camera 24 up: a water pixel
at **100 blocks** reflects at about 13 degrees, reaches the deck at 1,373, and gives `fp` = 0.026 --
so every one of the four octaves clears `smoothstep(0.25, 0.5, fp * freq)` and all four are paid. At
**400 blocks** the ray is down to 3 degrees, the deck is 5,333 away, `fp` = 0.39, and only the base
octave half-survives. **The crossover sits somewhere around 200-400 blocks, so the band limit culls
octaves exactly where the pixels are already cheapest and pays all four across the near field that
fills a coastal frame.** The recommendation is therefore not redundant, and it is also not free money;
the octave sweep below prices it in one build.

**Correction 4 — the 2D planar cloud buffer has a footprint the sky document never sizes.** It
proposes "a fixed-resolution offscreen grid" centred on the camera. *Verified:* `CLOUD_FADE` is
**3.0e4** and `CLOUD_MAX` **1.0e5** (`common.wgsl:1680`), the deck sits at **448** against a sea
level of **128** (`Clouds::default`, `worldgen::SEA_LEVEL`), and `cloud_scale` is **320** blocks
per base octave so the finest of the four is 40. A grazing reflection climbs 320 blocks while
running thousands horizontally, and a primary ray to the horizon reaches the deck out to
`CLOUD_FADE`. The buffer therefore has to cover tens of thousands of blocks *and* resolve a
40-block feature near the camera, which is a mip chain rather than a grid — and a mip chain is a
different filter from the analytic band limit the FBM currently applies, so the control would
not be bit-exact and the batch would owe a look measurement. **The idea survives; "a single
bilinear sample" does not.**

**Correction 5 — occupancy is offered as a mechanism again, and this project has ruled it out
twice by measurement.** The megakernel document derives 80 → 96 registers as 6 → 5 resident
warps per SMSP and lists it among the causes of the +1.036 ms. The arithmetic is right —
16,384 registers per SMSP over 32 lanes gives 6.4 warps at 80 and 5.3 at 96 — and the
*conclusion* is the one `errors.md` and `gpu.md` both refuse: batch 13 added **eight** resident
warps to this pass and bought **−0.01 ms**, batch 17 took another pass from 67% to 100%
occupancy and bought exactly zero. **A mechanism that predicts a millisecond from one warp,
in a pass that did not move for eight, is not the mechanism.** The document's own frontend
argument does not need it.

**Correction 6 — the megakernel document's Phase 1 is already shipped, and what it actually
describes is entry 38e.** "Compile separate resolve pipelines using Vulkan specialization
constants; eliminate unexecuted material branches" is `SPEC_MASK`, which has existed since batch
13 and which `glass` is already in. The reason the shipping build pays anyway is that
`SPEC_GLASS` is **on** in it, because glass is a feature the world can contain — so the remedy
the document names as optimal is, stated precisely, *make that constant follow whether the
resident world holds any glass*, which is 38e's second candidate fix and comes with the
mid-session pipeline compile and the `SPEC_MASK` invariant break that entry already records.
**The document could not have known this and it does not change the advice; it changes which
roadmap entry the advice belongs to.**

**Correction 7 — two measurement sittings are being mixed.** The 4.16 ms and 1.02 ms figures
come from water's own batches; the 7.15 ms coastal delta was measured at the end of batch 38b.
The second document computes "refraction accounts for 58% of the total water cost delta" by
dividing one by the other. `PERF.md`'s first caveat is that absolute milliseconds drift by up to
**1.8x** with this laptop's power state and that only pairs benched back to back mean anything.
**The share is not established; it is the first thing batch 40 has to re-measure.**

---

## What this changed in the roadmap

| entry | was | now |
|---|---|---|
| P1 | water, research first, subject named and design open | **batch 40: a literal sweep that prices three families before choosing one** — three one-constant diagnostic builds, each benched paired against the shipping binary. **Batch 41 then ranked the two survivors on look and shipped the one no document named**, 48 -> 16 blocks of shadow march |
| P2 | open | unchanged, and now with a profiler-free protocol it can use |
| 38e | a candidate under P2 | **unchanged in rank, and now with an outside argument** for the pipeline-key fix it already lists second |

**The batch the set actually supports is not any of the three headline recommendations.** Each
of the three families — sky ALU, reflection reach, refraction traversal — has a **one-literal
diagnostic build** that prices it in about twenty seconds, which is batch 37's own trick
(`shadow_dist` 220 → 1500, build, copy the exe aside, restore, rebuild) and is on file in
`../CLAUDE.md`'s environment section. `CLOUD_OCTAVES` 4 → 1 prices the entire sky branch
including the 2D buffer nobody has built. `WATER_REFLECT_DIST` 768 → 384 prices the reach
recommendation exactly as shipped. `WATER_SHADOW_DIST` 48 → 0 prices Correction 2 and says
whether refraction's cost is the walk or the shading at the end of it. **Three builds, three
paired benches, and the batch that follows is chosen by a measurement rather than by a
document** — which is the standing rule for a performance entry and the thing the research was
commissioned to inform rather than to replace.

---

## Postscript: the first document set ever scored against a measurement

**Written the same session, after the sweep the entry above called for actually ran.** Eight
paired benches, `PERF.md` under *The water sweep, batch 40*. **This is the first time any
research document here has had its ranking checked rather than argued with**, and the result is
worth more than any individual verdict above.

| what the documents ranked | where it ranked | what it measured |
|---|---|---|
| a sky-term fix (2D cloud buffer / octave truncation) | **first, by all three** | **-0.594 ms, and -0.619 with the water removed** -- so water's share is zero |
| reflection reach 768 -> 384 | second or third | **-1.476 ms**, 3.3x more than the 0.45 projected |
| the refraction's shadow ray | **never mentioned by any of them** | **-4.100 ms, 31% of a coastal `resolve`** |

**The ranking is exactly inverted, and the mechanism for why is Correction 2.** All three
documents modelled a secondary ray as a traversal loop and reasoned about step counts, strides
and reach. The thing that actually costs is the `shade_hit` at the *end* of the ray, which casts
a third ray of its own -- a fact that is in this repository, in `water.md`, with a number
attached, and that no document was given and none inferred.

**Three conclusions, and the third is the one to carry forward.**

1. **The mechanisms were right and the ranking was worthless.** Every individual claim that could
   be checked held up -- screen space really is 0% for refraction, ray length really does explain
   reflection against divergence, the parallax invariant is exactly right. What failed is the
   *ordering*, every time, in both directions and by factors of three to seven.
2. **A projection was wrong even when it was hedged as a falsification threshold.** "If this
   fails to reduce resolve by at least 3.0 ms, the recommendation is falsified" reads like rigour
   and is a guess; the true figure for water was zero. **A threshold invented by the same process
   that invented the estimate is not an independent check.**
3. **The cheapest experiment beat the most expensive document.** Three one-constant diagnostic
   builds, about twenty seconds each, priced three families more decisively than seven thousand
   words and forty citations. `../CLAUDE.md` already said the only thing that settles a cost here
   is `harness bench` against a build without the feature in it. **It is now also true that the
   sweep should come before the reading, not after it** -- had these three builds been made at
   the top of the P1 session, two of the three prompts would have been aimed somewhere else.

**A fourth, added by batch 41, which ranked the two survivors on *look* rather than on time.**
The documents' ranking was inverted a second time and on a different axis: the constant none of
them named is worth **1,309 pixels of 921,600 at a max channel delta of 1**, and the one they
did rank is worth **22,183 pixels at a delta of 109** for less of the saving. **Neither answer
was available to a document, and the reason is the same both times** -- the cheap one is cheap
because batch 37 gave `shade_hit` a second occluder that resumes where the march stops, so the
truncation moves a handoff rather than deleting a shadow. That is a fact about the *interaction
of two batches in this repository*, eighteen months of nobody's literature, and no amount of
reading about ray traversal produces it. **Ask a document for mechanism; ask the tree for
ranking.**

---

# The eighth document, and the first that did its homework (2026-09-15)

**Not a PDF and not commissioned.** The user pasted a set of recommendations into chat after
batch 62 and asked for the *methodology* to be judged rather than the numbers -- explicitly:
*"ignore the potential zero frametime cost more look at the methodology."* That instruction is
this file's own thesis arriving from the other direction, so the verdict is filed here rather
than in the roadmap.

**It is the best of the eight and the gap is not close.** It read the tree. Ten of its
load-bearing quotes were checked against the code and `PERF.md` and came back correct:

| claim | verdict |
|---|---|
| instruction-bound at 96 registers | correct |
| VRAM and CPU workers idle | correct |
| probe lattice is 64 x 384 x 64 | correct |
| `shaft_scan` fixed at 0.12 ms | correct -- `PERF.md` reads 0.11-0.12 at fifteen of sixteen, 0.00 at `night` |
| secondary rays are 8.2 ms of a 10 ms `resolve` | correct, and ours |
| `EXPERIMENTAL_RAY_QUERY` supported | correct, batch 55's `probe` run |
| the penumbra is sub-voxel under 108 blocks | correct, batch 61 |
| `SKY_TINT` is a hardcoded constant | correct |
| `ShaftField` fills only from `WorldGen::height` | correct |
| `coarse_trees` exists | correct, `worldgen.rs:709` -- but see below |

**So the systemic flaw at the top of this file has a second form, and this document is the
specimen.** The first form was *invented digits beside real ones*. The second is **correct
digits attached to the wrong mechanism**, and it is harder to catch precisely because the
digits check out.

## The three failures, in rising order of how much they would have cost

**1. A measured cost rounded to zero.** *"A single hardware trilinear tap costs practically
0.00 ms."* Batch 55 measured the tap at **0.01 to 0.055 ms** and batch 57 measured the cube at
**0.02 to 0.04 at `cave`**. Cheap, not free -- and the whole argument for stacking a bounce bake
behind that tap rests on which. Its two invented figures are the ordinary kind and announce
themselves: *"~0.25-0.35 ms fixed 2D compute"*, and *"a land pixel costs ~3 work units, a water
pixel ~9"*, which is not even a unit.

**2. Two claims about our code that are wrong, and both would have been acted on.**

- *"`coarse_trees` already aggregates canopy elevation for LOD selection."* It does not.
  `worldgen.rs:709` takes `out: &mut [BlockId]` and writes **proxy trunk and canopy blocks**
  into a coarse chunk, bailing above stride 8. There is no canopy-height array, so the
  recommendation's *"adding this aggregated height to the scan buffer"* has nothing to add.
  The idea is still right -- it is roadmap `A1`, and `shaft.rs`'s own module note calls it *"a
  one-line change -- deliberately not taken here"* -- but somebody has to build the aggregate.
- *"Standard Reinhard or naive sRGB clamping."* [`blit.wgsl`](../src/render/shaders/blit.wgsl)
  is neither. It is a deliberate knee hyperbola, C1 at `KNEE = 0.75`, asymptoting to 1.0 so the
  sun disc and glowstone roll off rather than clipping. **And the misreading hides the real cost
  of the document's most interesting suggestion**: that function's comment says *"Below the knee
  this is exactly the identity, so every constant tuned against the linear image stays where it
  was put."* AgX is identity nowhere. Swapping it in invalidates every constant in this engine
  authored by eye -- `floor_rgb`, `SKY_TINT`, `PROBE_SUN_GAIN`, `AO_STRENGTH`, the biome tints,
  `LEAF_FILL`, the water mottle -- each of which carries a comment saying why it is what it is.
  **The cost of a tone-map change is not in the shader. It is in the retuning**, and "0.00 ms on
  the 3D pipeline" is true and beside the point.

**3. Three inferences borrowed from a measurement of something else.** This is the dangerous
category and `lessons.md` has carried the rule since batch 59: *a precedent is a template for an
argument, not a conclusion you inherit.*

- *"Batch 54 proved that fading to the body color converged to within 3-7 delta of a full traced
  ray"*, offered as grounds for cutting the **refracted seabed march**. Batch 54 measured that
  for the **Snell's-window mirror seen from below** -- a different ray, a different path length
  through the medium, approximating a different thing. The underlying idea may well pay; the
  evidence offered is for another transport.
- *"Massive SIMD warp divergence"* as batch 61's mechanism. Nobody measured that. What batch 61
  measured is that four taps cost **6.11x** one -- of which **4x is doing 4x the work** and the
  superlinear excess is **1.53x**. The recommendation's logic is *therefore use one ray*, which
  treats the whole 6x as a divergence problem that one ray dissolves. It is not; one ray is one
  ray's worth of work, and that was never the objection.
- **It quotes roadmap R6's own sentence** -- *screen-space "fails at exactly the grazing angles
  where specular matters most"* -- approvingly in one section, and recommends SSR for water in
  another. Distant water is nearly all grazing. The document contradicts itself using our text,
  and its fallback structure means a miss pays the screen-space march *and* the 3D march.

## What survives, and three of them were not on the roadmap

Ranked by whether the mechanism holds, not by the cost claimed:

1. **Half-resolution or checkerboard secondary rays.** The best idea in the set, built on its one
   correctly-quoted large number. `A2` already says *"a real improvement here is a redesign and
   not a constant"* and this is the redesign. The objection it does not raise: wave normals are
   high frequency and `--no-wave-aniso` is worth **15.9x at the horizon**, which is precisely the
   detail a half-res trace eats. Filed as **P12**.
2. **2D caustic projection.** Genuinely absent -- `caustic` appears **zero** times in this tree --
   cheap, and one of the strongest path-traced cues water has. Filed as **A9**.
3. **Canopy into the shaft scan.** Already `A1`; correct mechanism, missing prerequisite.
4. **Sky-coupled ambient.** Already `R7`, already ranked next.
5. **Refraction depth cutoff.** Plausible, wrongly justified, and there is better local precedent
   than the one cited -- `--no-water-dark` culls at 64 and `WATER_DARK_DEPTH` is 15. Folded into
   `A2`.
6. **AgX.** Worth doing, as a *retuning* batch. Filed as **A10** with that framing.
7. **Screen-space penumbra.** Does not engage with what batch 61 found: there is no penumbra
   below 108 blocks to reconstruct, so a blur scaled by `t_occl / t_receiver` **invents** one.
   Legitimate as style, not as physics -- and at the document's own suggested 2.5-3.5 degrees the
   effect is *smaller than the build already rejected*, since the measured sweep is linear and
   16x physical moved 2,978 px. **Not filed.**
8. **One ray plus TAA.** Does not engage with `taa.wgsl` clipping to the 3x3 neighbourhood, which
   is the mechanism that rejects exactly the noisy history a 1-spp stochastic shadow produces.
   **Not filed.**

**The RT-core suggestion has a concrete error worth keeping**: tracing *"against coarse chunk
bounding boxes"* returns box hits, not surface hits -- a 64^3 AABB says nothing about its
contents, and a real version needs a BLAS over voxel-level AABBs, which is what `CLAUDE.md`
notes `BlasGeometrySizeDescriptors::AABBs` is for. It also calls the bake *"an asynchronous
worker"*, conflating the idle **CPU** threads with the idle **GPU** RT cores. And it does not
mention what a voxel-geometry bake forfeits: three shipped rungs depend on the probe field being
a pure function of world position, with no apron, no residency dependency and no re-bake on
edit. That property is the reason `probe.rs` reads the height field in the first place.

## The rule this document adds to the top of this file

**Check the mechanism even when the numbers check out.** Six of the seven PDFs were caught by
their digits; this one's digits are almost all real, and every one of its three serious errors is
a *mechanism* attached to a correct number. The cheapest test is the one used here: for each
recommendation, name the file and line it claims to be about, and open it.



