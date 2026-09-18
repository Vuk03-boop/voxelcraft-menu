# CLAUDE.md

Guidance for Claude Code working in this repository.

## What this is

`voxelcraft` — a Minecraft-style voxel engine in Rust that renders by ray tracing sparse
trees in compute shaders. **There is no mesher and no triangle geometry anywhere.** If you
find yourself reaching for greedy meshing, vertex buffers, or a depth buffer, you have
misread the architecture.

## Where everything is

**This file is the whole briefing.** Everything else is read on demand, and the table says
when. Nothing here is organised by batch — it is organised by what you are working on,
because a decision filed under a batch number is a decision nobody can find.

| file | read it when |
|---|---|
| [`docs/roadmap.md`](docs/roadmap.md) | **once, at the start** — what to do next, and nothing else. Every entry in it is open; a shipped one leaves the file entirely and becomes a ledger row. **A designation must not be used in a status report until it exists as an entry in that file** — a queue that lives only in the context window is a queue the next session inherits as code without reasons. A reference to a designation no longer in the file resolves through *Where a closed designation went*, the one table near the top — including `L1`, which is `R1` and shipped as batch 57; **falsifying old documents to tidy a name is how a claim comes to exist in two versions, and the table is the one hop instead.** If a batch produces a finding rather than an entry, file it in [`docs/lessons.md`](docs/lessons.md) the same session |
| [`docs/pitfalls.md`](docs/pitfalls.md) | **before you type a command** — traps that fire at the moment of an action |
| [`docs/errors.md`](docs/errors.md) | **before you design anything** — known defects, and what has been tried and lost, with the number it lost by |
| [`docs/lessons.md`](docs/lessons.md) | **when you plan a batch**, keyed on the kind of batch. It also holds the two instruments a *look* batch needs before it starts: the **MAE calibration ladder** — 0.372 invisible, 2.87 rejected by eye, 5.98 shipped — and the three questions to ask a rung in order (*what share of the frame's light, what share of the frame's pixels, which resource pays*) |
| [`docs/harness.md`](docs/harness.md) | **before you measure anything** |
| `PERF.md` | the current frame cost, memory, worldgen |
| [`docs/adr/`](docs/adr/) | **when an entry looks unfinished rather than decided** — one file per design that was built, measured and *reverted*, so it is in no commit and no ledger row. `0001` is roadmap R2b, `0002` is roadmap R5 |
| [`docs/ledger.md`](docs/ledger.md) | what each shipped batch did — history, one row each |

The subsystems, each holding its own constants and the reason for each:
[`water`](docs/water.md) · [`sky`](docs/sky.md) · [`shading`](docs/shading.md) ·
[`terrain`](docs/terrain.md) · [`foliage`](docs/foliage.md) · [`lod`](docs/lod.md) ·
[`temporal`](docs/temporal.md) · [`gpu`](docs/gpu.md) ·
[`persistence`](docs/persistence.md)

**Every constant also carries its "why" in a comment at its own definition site, and that is
the copy that cannot drift — read the comment before you change the number.** Batch 26 is the
proof: four places described one amplitude and the one the renderer read was the odd one out.

**Never open anything in `docs/research/`** — 6.4 MB of PDF that
[`docs/research-review.md`](docs/research-review.md) already summarises, including what is
wrong with each. `docs/reference.md` is external design input, not engine documentation.

## Commands

```bash
cargo test --release          # --release matters, debug is ~30x slower
```

```bash
cargo run --release           # the game
```

```bash
cargo clippy --release --all-targets     # NOT silent: an external audit counted 26 warnings (suspicious_assignment_formatting x13 in
# src/menu.rs, the known 12-arg constructor, and test-side field_reassign_with_default among
# them) -- clear them, then keep it at zero
```

Headless modes need no display and are the primary way to verify changes:

```bash
cargo run --release -- --screenshot out.png --width 1280 --height 720 --hud
cargo run --release -- --bench-frames 140 --width 1920 --height 1080
cargo run --release -- --bench-terrain 96
cargo run --release -- --march-stats --screenshot out.png --width 1280 --height 720
```

**`--march-stats` is a count and never a timing sample** -- it switches the heatmap on, so the
marcher pays an `atomicAdd` per hit and the capture it writes is the heatmap rather than the
frame. `harness march` drives it across the set; see [`docs/harness.md`](docs/harness.md).

```bash
cargo run --release --bin probe          # adapter features and limits
cargo run --release --bin shaderstats    # registers, spills, binary size per pass
```

**Every `allow` carries a reason at its own site, and clippy is not currently clean -- do not
run `clippy --fix` blind here** — three of its suggestions would cost real meaning; see
[`docs/pitfalls.md`](docs/pitfalls.md).

**The measurement fixture is `--bin harness`, and it is where a batch starts and ends.**
`capture` and `validate` at the start, `implies` and `bitexact` at the end. **`march` is the
fourth, and it is the only one of them a hot or busy machine cannot corrupt** -- it counts the
marcher's DDA steps rather than timing them, so a traversal change is rankable before a bench
exists and a *bit-exact* one still gets a positive reading. Do not hand-roll a vantage, a
crop, a metric, a hash loop or a bench A/B — [`docs/harness.md`](docs/harness.md) explains
why each of those is a trap, and nineteen sessions proved it.

**Ranking a repeat is a third thing again, and neither a picture nor `coherence` is it** --
that row has fallen from 1.2053x to **1.0594x** across two batches that removed canopy without
touching the metric, because **a known-positive is only as durable as the frame it was read
off**. `tests/repeat.rs` ranks a repeat on the atlas instead, with no frame in it at all.

## The controls

**Each control is the A/B partner for its feature, and a pure control reproduces the previous
build *exactly*** — so it can pin a regression rather than merely resemble one. **Free** means
the control costs nothing to switch on, which matters because a control that costs a
millisecond quietly taxes every measurement taken with it. Full per-batch stories, vantage
tallies and measurement caveats live in [`docs/ledger.md`](docs/ledger.md) and
[`docs/harness.md`](docs/harness.md); this table carries what each control does, what it
clears, and what it costs.

| flag | reproduces | cost |
|---|---|---|
| `--no-biomes` | the pre-batch-9 rule set. Clears `--no-tint`, `--no-foliage`, `--no-meadow` | free |
| `--no-water` | the same heightfield with air where water was | free |
| `--no-waves` | mirror-flat sea. Clears the three wave controls below | free |
| `--no-wave-aniso` | the footprint measured across the ray, understated 15.9x at the horizon | free |
| `--no-wave-shoal` | full swell in a two-block lagoon. Composes with `--no-wave-aniso` | free |
| `--no-wave-fill` | four wave octaves spanning the band instead of eight subdividing it | free |
| `--water-mottle 0.2` | the pre-batch-26 sea | free (atlas constant) |
| `--no-tint` | atlas albedo before the biome field | free |
| `--no-foliage` | the pre-batch-14 world, no grass tufts | free |
| `--no-meadow` | the pre-batch-18 coarse ground | free by construction |
| `--no-tex-variation` | the pre-batch-36 frame: every block samples its layer in phase again, so a grassy slope reads as corduroy. **32% of the default frame moves**; `coherence` has fallen twice as canopy left the frame — check [`harness`](docs/harness.md) before trusting that row | free (in `SPEC_MASK`) |
| `--no-leaf-cutout` | the pre-batch-38 frame: leaves are solid cubes again rather than a 4^3 carved mask. **10,272 pixels move at `default`**, all in the near field — the cutout is LOD 0 only | free (in `SPEC_MASK`) |
| `--no-glass` | the pre-batch-38b frame: a pane is an opaque cube again. **The one control here that is not a pure revert** — glass is `opaque: false`, a *world* change `light.rs` bakes into a chunk that no pipeline override can undo; every vantage that predates the batch is bit-exact. [`shading`](docs/shading.md) | free, and **recovers 0.346 ms at `default`**, 0.520 at `terraces`; the shipping build pays +1.036 at `coastline` for glass it never draws |
| `--no-water-shadow-cut` | the pre-batch-41 sea: a surface reached *through* the water marches its shadow 48 blocks again rather than 16. [`water`](docs/water.md) | free; **the one control here that costs time to switch *on*** — +1.672 ms of `resolve` at `coastline`, +0.531 at `default` |
| `--no-leaf-thin` | the pre-batch-43 canopy: a leaf block keeps **0.72** of its 4^3 cells instead of 0.62, swept by eye. [`foliage`](docs/foliage.md) | free |
| `--no-flat-secondary` | the pre-batch-45 frame: a hit reached *through* water or glass takes the nine-cell light gather again rather than one lookup. The largest saving here since Hi-Z: **-1.68 ms at `shore`**. The gate is the override *alone* — the usual `frame.flags` test costs +1.42 instead; see [`gpu`](docs/gpu.md) | free |
| `--no-water-far` | the pre-batch-48 frame: a submerged camera tests every resident chunk out to the full view distance again rather than stopping a tile whose every ray is still under water at `WATER_FAR_DIST`. Bit-exact at sixteen of seventeen vantages, and the seventeenth (`sea-horizon`, built because "bit-exact everywhere" was a statement about the *cameras*) is the point. The one control flag deliberately *outside* `SPEC_MASK`. [`water`](docs/water.md) | free; **-2.776 ms at `sea-horizon`**, -1.125 at `underwater`. **The control's own cost tripled when the constant moved** — what a control is worth is a property of the build and not of the flag |
| `--no-water-dark` | the pre-batch-53 frame: a submerged tile still under the sky flood's reach looks as far as `--no-water-far` lets it rather than stopping at 64 blocks. **Nested inside `--no-water-far`, which therefore clears it** — the only control here cleared by another rather than clearing others. The depth is derived rather than authored (`light.rs` takes one sky level off per block of water). [`water`](docs/water.md) | free; **-0.513 ms at `deep-water`** — the only camera in the set deep enough to fire it |
| `--no-snell` | the pre-batch-54 frame: a submerged eye sees straight through the water surface at every angle. **The first defect in this project found by playing rather than by a metric.** Outside the critical angle (`asin(1 / WATER_IOR)` — there is **no angle constant in the tree**) the surface is a mirror showing `underwater_body()` and not a traced ray: the traced version was built, cost 4 ms of `resolve`, and differed from the free one by a max channel delta of 3 to 7. [`water`](docs/water.md) | free; **+0.262 ms of `resolve` at `deep-water`** to switch on, nothing measurable anywhere else |
| `--cloud-cover 0` | no cloud deck. **No longer skips the shaft integral** — batch 35 gave it a second occluder | free |
| `--no-terrain-shafts` | no ridge casts into the haze. **10 of 14 vantages move**. **No longer the pre-batch-35 frame on its own** — batch 37 gave the envelope a second reader | free (in `SPEC_MASK`) |
| `--no-distant-shadows` | the pre-batch-37 frame: the sun term stops at `shadow_dist` and nothing shadows anything past 220 blocks. Pair it with `--no-terrain-shafts` for pre-batch-35 | free (in `SPEC_MASK`) |
| `--cloud-shadow 0` | **the deeper of the two god-ray controls, and since batch 35 not a pre-batch-11 revert on its own** — pair it with `--no-terrain-shafts` | **+0.09-0.11 ms** |
| `--godray-strength 0` | keeps the ground shadow, so **not** a pre-batch-11 revert on its own. It is the one control that switches off *both* occluders | — |
| `--no-fade` | the old hysteresis deadband instead of the dissolve | — |
| `--no-taa` | the single-frame image, bit-identical to pre-batch-6 | — |
| `--jitter X[,Y]` | pins the sample offset; `--jitter 1,0` reproduces an unjittered capture translated one pixel | — |
| `--anim-time F`, `--anim-rate F` | both ship at 0. **The rate is the load-bearing half** — with a pinned phase nothing can ghost | — |
| `--wave-clamp F` | how far the traced reflection may tilt. Ships at **0** by measurement | — |
| `--reference N` | converges over an N x N jitter grid and prints its own noise floor. **The only thing allowed to rank a look change** | — |
| `--no-idle-repaint` | the pre-batch-70 cadence: the window renders every presented frame however still the world is. The shipped build presents the last converged frame again when camera, world version, pipeline words and sun tick are unchanged — bit-identical to re-rendering, only the HUD rebuilt. The rule is data in `src/idle.rs`; no headless path reaches it | free |
| `--no-full-march` | the pre-batch-72 frame: every ray stops at a mixed root-FULL chunk's face, `resolve`'s water-ignoring legs included — **31,815 pixels at `coastline`, max delta 95**. Roadmap P10; the fix marches the chunk as if its dense tree existed. [`tests/full_march.rs`](tests/full_march.rs) | cost class: traversal, not regulation — only moves rays that were *wrong*, and only in mixed full chunks |
| **`--no-shadow-share`** | the pre-batch-62 grid: across an LOD cross-fade the chunk index every *secondary* ray reads resolves to the **fine** half unconditionally. Roadmap D2, and **the first entry in this table the user found by playing**. **It is not a `SPEC_MASK` bit and not a `frame.flags` bit** — no shader reads it; what changes is which index the CPU writes into `grid`, which is `--no-fade`'s shape. `open-sea` leading rather than `lod` is the thing to know: the grid is read by the nine-cell gather's cross-chunk lookups as well as by shadow rays. [`lod`](docs/lod.md) | free, and **switching it on costs 0.084 ms at `lod`** — the control is the *slower* arm, because resolving to the fine half marches a more expensive tree |
| **`--no-offscreen-shadows`** | the pre-batch-64 renderer: a chunk the view frustum rejects is not uploaded, so it is in no `grid` cell, so **it casts no shadow**. Roadmap D3, **the second entry found by playing** — *"the large mountain shadow becomes shorter or larger like a weird pop in pop out"*. Same shape as `--no-shadow-share` one level up. The two sets are concatenated rather than merged, which is why `tile_select` pays nothing; the tail is bounded by the lattice. `tests/shadow_grid.rs` holds the ten-degrees-apart case. [`lod`](docs/lod.md), [`harness`](docs/harness.md) | free, and **switching it on costs 0.100 ms of `resolve` at `terraces`** — the cost does not track the pixels that moved, and it should not: a ray that now *hits* terminates early while one that still misses has paid for a traversal |
| **`--no-probe-cube`** | the pre-batch-57 frame: the directional shading factor is `face_shade`'s hardcoded 1.00/0.80/0.62/0.50 per normal again, rather than that table modulated by a **baked six-axis sky-occlusion cube**. Roadmap R1's first rung. The field is one 3D texture of six stacked slabs, 4 blocks per probe, world-space and toroidal, baked in `stream::build` off `WorldGen::height` and read as the one trilinear tap batch 55 priced. **A pure revert at all 18 vantages and by construction rather than by luck**: the field stores *occlusion*, so an unbaked texel, a coarse LOD and a pinned field are all `face_shade(id) * 1.0`. [`shading`](docs/shading.md), [`src/probe.rs`](src/probe.rs) | free; **costs 0.02 to 0.04 ms of `resolve` at `cave`** to switch on. The real price is off-frame: **1912 us per chunk** of bake on threads that are idle once the world is resident |
| **`--no-probe-bounce`** | the pre-batch-58 frame: the ambient floor is `(0.85, 0.90, 1.00) * frame.ambient` again, one authored constant, rather than that constant modulated by a **baked ground bounce** — `probe::bake` gathers the cosine-weighted mean albedo of the ground each axis can see, stored in the `.rgb` of the texel whose `.a` is batch 57's occlusion, so the two features are four channels of *one* tap. **A pure revert by construction**: the field holds a multiplier and the lattice is created holding 1.0. [`shading`](docs/shading.md), [`src/probe.rs`](src/probe.rs) | free, **nothing measurable to switch on** (+128 bytes at 96 registers). The price is off-frame: the bake goes **1868 to 2549 us** per chunk — the gather rides the march that already exists |
| **`--no-light-rgb`** | the pre-batch-59 frame: block light is one 4-bit level wearing one authored warm constant again, rather than three levels whose ratio *is* the colour. Roadmap R4's first rung. **The channel had to widen rather than the block table, and the flood is why** — the light cell went from one byte to two, `sky:4 \| r:4 \| g:4 \| b:4`, and the propagation rule did not change, which is what makes the revert exact. **Not a pure time revert**: the cell it reads is two bytes whichever arm compiles. [`shading`](docs/shading.md) | **recovers 0.202 ms of `resolve` at `cave`**; the remaining 0.110 against a real revert is the cell size itself |
| **`--no-sky-tint`** | the pre-batch-63 frame: the ambient's sky colour is `SKY_TINT`'s `vec3(0.6038, 0.7084, 1.0000)` again — one authored constant on every shaded pixel — rather than the sky model's own gradient integrated over the hemisphere each face can see. Roadmap R7. **What ships is a departure from `SKY_TINT` and not a replacement of it**: at the reference condition the function returns `SKY_TINT` to the bit, and everywhere else the result carries its **luminance** by construction, so only the hue moves. `SKY_TINT_SAT` is the one authored number, shipped at **2.5** by eye. **`--fog-scatter 0` ranks the two halves apart without a second bit, which matters because this is bit 31 and `frame.flags` has no more.** [`shading`](docs/shading.md), [`tests/sky_tint.rs`](tests/sky_tint.rs) | **0.02 to 0.045 ms of `resolve` at `cave` to switch on, where 0 pixels move** — the cost tracks surface density |
| **`--no-sky-specular`** | the pre-batch-65 frame: every opaque surface is perfectly diffuse again. Roadmap R6, and **the first term in this renderer added *outside* `albedo`** — a specular reflection is not tinted by the diffuse albedo and is not a redistribution of the ambient. **The gate is `sky_sm` and batch 60 is why it is not `probe.a`**: the field stores occlusion normalised to 1 in the open, so a buried probe returns 1.0, which is unoccluded and not dark. `GROUND_ROUGHNESS` caps the Fresnel climb (the first build without it was a blue-white haze at **MAE 15.65**). **It is the first control here driven by the second specialization word (`SPEC_HI_MASK`) rather than by `frame.flags`.** `SKY_SPEC_GAIN` ships at **0.5**, chosen by the user from images. [`shading`](docs/shading.md), [`tests/sky_specular.rs`](tests/sky_specular.rs) | **+0.051 ms of `resolve` at `cave`** after batch 66 folded the arm away in secondary hits (it was +0.295 as batch 65 shipped it — the bill was register pressure, not arithmetic; **96 registers again since batch 66**) |
| **`--no-probe-shadow`** | the pre-batch-60 bake: the bounce gathers the albedo of the ground a probe can see and never asks whether that ground is itself lit. Roadmap R3's first rung, transport half. **Unlike every other probe control this one changes what is *in* the field rather than whether the shader reads it**, so it lives on `WorldGen` and no pipeline override can undo it — `--no-glass`'s shape, but the revert is total. `ray_exposure` is a **maximum of slopes** over the `STEPS` column heights the horizon march already took, so the chunk's **81,920 height samples stay 81,920**. **On its own it is invisible and that is the finding**: 644,121 pixels at `default` but an **MAE of 0.372**, against the **2.87** at which ADR 0001 was rejected by eye. [`shading`](docs/shading.md), [`src/probe.rs`](src/probe.rs) | free on the frame; **+671 us of bake per chunk, 1.24x** |
| **`--no-probe-sun`** | the pre-batch-60 floor: `floor_rgb` is `(0.85, 0.90, 1.00) * frame.ambient` again, one flat authored constant, rather than that constant **plus** `probe.rgb * PROBE_SUN_GAIN * sky_sm`. Roadmap R3's first rung, energy half, and **it exists because ADR 0001 said it had to**: `frame.ambient` is 0.08, so the floor is about **7% of the light on a lit surface** and no redistribution of it can be worth more. Added beside the authored floor, so a cave keeps its sole light. [`shading`](docs/shading.md) | free; nothing measurable on the frame against a real pre-60 binary |
| **`--probe-sun-high`** | **not a control — the only `SPEC_MASK` bit in this project whose default is *off*.** It reproduces no earlier build; it selects `PROBE_SUN_GAIN_HIGH`'s 0.25 over the shipping 0.10, so the strength of one bounce can be ranked by eye without a rebuild. **Two constants behind an override rather than a uniform** — `GpuFrame` has no pads left and a value with two settings does not need one. **The user played all three arms and chose 0.10**; the ladder is kept so the choice is recorded rather than re-derivable. `cave` is 0 pixels between the two gains — the `sky_sm` gate proving no gain can leak underground. [`shading`](docs/shading.md) | free |
| **`--probe-tap`** | **not a control — the inversion is the point**: the feature ships *off* and this switches it *on*. Batch 55's diagnostic — one hardware trilinear tap into the probe field per shaded surface, multiplied into `amb`, **on top of** batch 57's cube. It implies `--probe-fill 1.0`, so the frame is **bit-exact at all 18 vantages** and the whole output is a millisecond. `--probe-fill F` is the paired positive — at anything but 1.0 the frame *must* move — and `--probe-noise` rules out driver compression of a constant fill. **The field was 128^3 when this was measured and is 128 x 768 x 128 since L5 halved the spacing**, so re-measure before quoting the cost column against a later build. [`shading`](docs/shading.md) | **costs 0.01 to 0.055 ms of `resolve` to switch on**, and the finding is that **three taps cost the same as one**: the code exists, the work does not scale |
| **`--probe-ambient`** | **not a control**: it makes the probe field *replace* `shade_hit`'s ambient rather than multiply into it, so the terms a pre-composed directional field makes redundant leave the module. **The frame it renders is wrong on purpose** and is thrown away; it implies `--probe-tap`, so what it reports is the architecture's *net*. [`shading`](docs/shading.md) | **saves 0.03 to 0.13 ms of `resolve`**, net of the tap — the saving tracks surface density where batch 55's cost did not, which is what says one is work and the other is code |
| **`--soft-shadows`** | the DS01/DS02 arm: the dedicated primary-shadow pass (**one geometric ray per eligible receiver**, always on) plus, with this flag, the **screen-space bilateral penumbra** over its blocker distances. `--sun-softness narrow\|normal\|wide` scales the aperture; `--soft-shadow-hq` raises the kernel bound (4 -> 8 pixels) and **does not select extra geometric rays**. The batch-102a three-tap cone this flag originally armed was refused by the bench (**+4.545 ms of a 7.825 ms frame, 13x its bar**; whole-frame MAE under the 0.372 invisible rung everywhere, sign split 86% lighter) and deleted — ADR 0002's wall met from the effect side, roadmap G5's boundary gate the second attempt. [EXPERIMENTS](EXPERIMENTS.md), [`shading`](docs/shading.md) | see [EXPERIMENTS](EXPERIMENTS.md) for the pass family's resources and timings; the lesson from the refused arm lives in [`lessons`](docs/lessons.md): **a ray's bill is that it was fired, not how far it marched** |
| `--no-hiz`, `--ambient`, `--no-water-refract`, `--no-water-reflect`, `--fog-density 0`, `--fog-falloff 0`, `--water-absorb F` | one term each | — |

`--help` lists every flag with a one-line description. **This table is the only place that
carries what a control costs and which other controls it clears**, which is why `README.md`
points here rather than describing the set a second time.

**`--screenshot` presents through an offscreen render target**, so it exercises the compute
passes, the blit and the HUD exactly as the window does. It accumulates `--taa-frames`
(default 32) and captures the last — still one deterministic image.

## Architecture

### The frame (src/render/)

Seven compute passes, all sharing one WGSL module. `render::shader_source()` concatenates
`src/render/shaders/common.wgsl` with the six pass files, and `Renderer::new` creates one
pipeline per entry point in `render::ENTRY_POINTS`. **`march` and `resolve` are the exceptions
and get one pipeline per specialization** — see `SPEC_MASK` in [`docs/gpu.md`](docs/gpu.md).

1. **shaft_scan** — the light envelope: a max-plus prefix scan over a 512x512 terrain height
   window, eight dispatches deep. **The only pass whose cost does not depend on the window
   size, on how much geometry is on screen, or on where the camera looks.** `resolve` reads
   what it writes **twice since batch 37** — the haze's shaft and the ground past
   `shadow_dist`. See [`docs/sky.md`](docs/sky.md).
2. **tile_select** — screen in 8x8 tiles; each tile tests every chunk's **bounding box**, never
   its tree. This is where the performance comes from: box tests are nearly free, so the
   expensive march runs only on survivors. **It reads a *prefix* of the chunk array since
   batch 64** — `frame.chunk_count` is the frustum-culled length and this pass is its only
   reader; the entries past it are the chunks the frustum rejected, uploaded so `grid` can
   name them and they can cast a shadow. [`lod`](docs/lod.md), roadmap D3.
3. **march** — one 8x8 workgroup per pair, dispatched indirectly. Writes hits to a 64-bit
   visibility buffer with `atomicMax`, so depth testing falls out of the atomic. **It is not
   free, and most of what suppressing its writes saves is billed to `resolve`, not `march`**
   (91/9 at `underwater`, 68/31 at `sea-horizon`) — the direction transfers and the ratio
   does not, so bench a march-side cull on `resolve`'s row. Since batch 53 it walks
   `GpuChunk::dry_mask` whenever `skip_water` is set — **traversal only**: `mask_below` still
   indexes off the real mask, and using the dry one there reads a neighbouring node, which is
   a plausible picture rather than a crash. [`gpu`](docs/gpu.md).
4. **build_hiz** — per-tile farthest visible depth, consumed by the *next* frame.
5. **recover_select + march** — re-tests everything the stale Hi-Z rejected against the fresh
   Hi-Z. This is what makes Hi-Z lossless under fast camera motion. **Do not remove it.**
6. **resolve** — decodes the visibility buffer and shades, and is **two thirds of the frame**.
   Shadow rays, the nine-cell gather and water's secondary rays are paid per *visible* pixel,
   not per traced pixel. **Three materials since batch 38b** — `shade_water`, `shade_glass`,
   `shade_hit` — chosen on the block id the pass reads back out of the attributes, which is
   why neither transmissive material needed a bit in the key. **One material, two light
   models, since batch 45**: `shade_hit` is called four times in the worst case, and the three
   secondary legs take one light lookup where the primary keeps the nine-cell gather — a
   *light model*, not a material; what the transports destroy is the contact cue the extra
   eight lookups buy. **Block light has been three channels since batch 59**, derived from
   what reached the cell. **Since batch 57 the directional half of the ambient is baked rather
   than tabulated** — `face_shade`'s table is now the open-field calibration of a derived
   number. **Since batch 60 the ambient floor is no longer flat** — the baked bounce is added
   beside the authored constant and goes to zero underground through the same `sky_sm` gate
   as the sun; the batch's own finding is the ceiling: that floor is about 7% of the light on
   a lit surface. **Since batch 65 a surface is not purely diffuse** — a Schlick weight at
   `GROUND_F0` carries `sky_base` in the mirror direction, **added outside `albedo`**, gated
   by `sky_sm`. **Since batch 63 the sky half of the ambient has the sky's own colour** —
   `SKY_TINT` is now the calibration of a derived hue. **And since batch 54 one *boundary* as
   well as three materials**: `surface_from_below` reconstructs the water crossing
   analytically and applies Snell's window — a material is what a ray hits and this is what
   it passes through. [`docs/shading.md`](docs/shading.md), [`water`](docs/water.md).
7. **taa** — reprojects the previous frame, clips to the 3x3 neighbourhood and blends. Runs
   **in linear, before the tone map**, and is the only pass reading a texture the previous
   frame wrote.

### Invariants

These will bite you. They are load-bearing, not stylistic. Subsystem-specific ones live with
their subsystem; these are the ones that cross files. **Every one is a decision *this engine*
made that binds somewhere else.** The wgpu and WGSL traps that used to sit in this list are in
[`docs/pitfalls.md`](docs/pitfalls.md) now.

- **The camera's own chunk is exempt from the box test.** From inside a box, ray-box
  intersection returns the *exit* distance, which would cull the chunk you are standing in.
- **Bind group 0 and the `@group(0)` declarations at the top of `common.wgsl` are one list
  kept in two places.** The entry count lives in `render::BIND_GROUP_0_ENTRIES`;
  `make_layout`'s array annotation pins the Rust half and `bind_group_0_matches_the_shader`
  pins it against the shader.
- **The engine requires `SHADER_INT64` + `SHADER_INT64_ATOMIC_MIN_MAX`** and refuses to start
  without them rather than falling back silently.
- **Visibility key layout is `depth:24 | normal:3 | chunk_id:13 | voxel_xyz:24`.** The 13-bit
  chunk id caps the per-frame list at `MAX_CHUNKS = 8191`. **The key is now full** — batch 38
  spent bits 6 and 7 of each byte on the 4^3 micro-cell of a carved leaf, and `normal:3` has
  been full since batch 14. **A fourth primitive shape has nowhere left to go.**
- **A new *material* is not a new primitive shape.** `resolve` already calls
  `block_at(ci, v)` to tell water from everything else, so the id *is* the tag — **reach for
  `block_at` before reaching for the key**: a shape the marcher has to resolve differently
  needs bits, and a surface `resolve` has to shade differently does not.
- **`hit_t` reconstructs the hit plane in micro units unconditionally, and must not branch on
  `SPEC_LEAF_CUTOUT`.** A branch there makes the driver rebuild the folded function one ULP
  away and costs the control its bit-exactness. `hit_t_does_not_branch_on_the_cutout_override`
  is the guard.
- **`shade_hit`'s `shadow_dist` parameter is a boundary between two occluders, and both sides
  of it have to be handed the same number.** The march covers `[0, shadow_dist)` and the
  light envelope covers `[shadow_dist, reach)`, and the cancellation that makes that a
  partition is only valid when `terrain_shade_beyond` is sampled at the distance the march
  just used. `the_envelope_resumes_where_the_water_shadow_march_stops` is the guard.
- **The visibility buffer holds one hit per pixel, so exactly one chunk may march a given
  ray.** That is why the LOD cross-fade is a *partition* of the rays and not a blend weight —
  see [`docs/lod.md`](docs/lod.md).
- **Everything the compute passes compute is linear radiance.** `blit.wgsl` is the one place
  that tone-maps and encodes. **A `pow()` anywhere in `resolve`, or a display-referred
  constant in `common.wgsl`, puts the pipeline back where batch 2 found it.**
- **`sky_base` is the sky without clouds, `sky_color` is the sky with them, and `sky_color`
  takes a ray origin while `sky_base` must not.** Getting either wrong is invisible in a still
  capture — see [`docs/sky.md`](docs/sky.md).
- **`block_faces` strides by eight, and every WGSL call site has to say so.** A wrong stride
  returns a *plausible* layer rather than garbage. `faces_per_block_matches_the_shader` holds
  every call site to it.
- **A `block_faces` word is not a layer.** Since batch 36 it is the atlas layer with
  `block::PERM_CLASS` packed above it; `face_layer` and `face_perm` are the only two ways in,
  and `face_layer_at_every_call_site` holds every reader to them — including the one local
  allowed to hold a raw word, which has to be named `*_word` to say so.
- **New block ids are appended, never inserted — and an id and its table row are one list
  kept in two places.** `const _: () = assert!(block::WATER == 13)` ties the id to its
  literal in `common.wgsl`; the `BLOCKS` row is enforced in `block.rs`'s const asserts (the
  tail id, its row, and the lamps' light channels above it), because batch 94 re-named every
  batch-59 emitter when an id and its row disagreed.
- **A block id reaching the hotbar is checked at three edges** — the journal, `--bar` and
  pick-block all go through `block::bar_slot_refusal`, because the `const` assert can only
  speak for a `const` array. `TALL_GRASS` is the one that matters: `SPEC_FOLIAGE` compiles
  the marcher unable to see a tuft, so a placed one is invisible rather than wrong.
- **`GpuFrame` has no pads left.** Six batches appended **four** words before
  `prev_view_proj` — four is the only count that keeps the mat4x4 16-byte aligned without
  inventing a pad — and seven `offset_of!` asserts pin the seams, because a size assert alone
  cannot catch two swapped fields. **Batches 41 and 43 both needed a tunable constant and
  both used an `f32` `override` in `SPEC_MASK` instead**: the value folds into the pipeline,
  the control costs nothing, and the struct does not grow. **Reach for that before reaching
  for a field** — a uniform is for something that changes *during* a run.

### World storage (src/voxel/)

A chunk is 64³ voxels in a 3-level 4³-branching tree. **Geometry and attributes are
deliberately decoupled** — DAGs deduplicate geometry well and attributes badly.

- `geometry.rs` — leaves are bare `u64` occupancy masks; `Inner` nodes are 16 bytes. Child runs
  are content-hashed, interned and reference counted. `Inner::leaf_prefix` is the Dolonius-style
  leaf count that makes attribute lookup a single load.
- **The full-subtree flag (`FULL_FLAG`, bit 31 of `run_ptr`) is what actually compresses the
  world**, not the interning. Measured dedup on procedural terrain is **1.00x** — expected,
  documented in `PERF.md`, **do not "fix" it**.
- `attributes.rs` — block ids per chunk, each leaf palette-compressed to 1/2/4/8 bits with an
  inline uniform entry for single-block leaves.
- `world.rs` — `set_block` is a bottom-up path edit. **`leaf_pos` must be derived from sibling
  counts, not from `old_l1.leaf_prefix`** — an absent L1 node carries prefix 0 and would
  silently corrupt an unrelated block. `tests/edit_regression.rs` guards this.
- `light.rs` (at `src/light.rs`, not under `src/voxel/` — it shares this section's subject and
  not its directory) — sky/block flood fill into 4³ bricks. **Light lives in *air* voxels**,
  which have no leaf index, so it cannot share the attribute indexing. **A cell is two bytes
  since batch 59** — `sky:4 | r:4 | g:4 | b:4` — and **the propagation rule did not change**:
  `flood` is called three times on three arrays rather than taught about colour, which is
  what makes `max(r, g, b)` the level the single pre-59 flood held and therefore makes
  `--no-light-rgb` exact rather than close.
- **Three separate questions that used to be one:** does it occupy a voxel (the tree), does it
  block light (`BlockDef::opaque`), does it block the player (`BlockDef::solid`). Water is
  yes/no/no, foliage is yes/no/no, and **glass is yes/no/yes since batch 38b** — the first
  block that stops the player and not the light.
- **`set_block` records every accepted edit into a journal, and `stream::build` replays it
  into the dense array before the tree, the attributes or the lighting read it.** It is why a
  chunk that unloads and regenerates comes back edited, and why a reloaded world is
  bit-identical to the one edited live. **`--edits` appends every record to the file as it
  lands**, so the file is the world at every instant; what makes that safe is the order of
  the writes, not the appending. **The coarse levels aggregate the deltas under each of their
  voxels rather than taking an edit**, and an edit marks every stand-in over it stale. **The
  file also carries the player and the bar's contents**, each as an optional block announced
  by its own bit of `flags`, which is what lets three file versions load through one reader
  with no branch on the version number — [`persistence`](docs/persistence.md).
- `probe.rs` (at `src/probe.rs`, beside `light.rs` and for the same reason) — batch 57's
  six-axis sky-occlusion cube, one probe per 4 blocks, baked in `stream::build`. **It reads
  `WorldGen::height` and never the dense array, which is the whole of its design**: a pure
  function of world position, so the lattice two neighbouring chunks write into has no seam
  and needs no apron, no residency dependency and no re-bake. What it pays for that is trees,
  caves, overhangs and edits, none of which the height field carries — the flood still does.
  **Unlike the flood it is baked for *every* LOD-0 chunk, empty ones included**, because a
  surface in one chunk taps probes in the next.

Worldgen, biomes and the coarse levels: [`docs/terrain.md`](docs/terrain.md). Streaming and
the LOD planner: [`docs/lod.md`](docs/lod.md).

## Working on this

**One batch per session, and that rule outranks curiosity.**

**A roadmap entry marked RESEARCH FIRST owes the user three prompts before it owes anyone
code, and that is the first thing that session does.** Write them out, hand them over, and
stop there — the user runs them through a deep-research tool and comes back with the answers,
which is a separate session. **Treat every number a research document returns as an argument
about mechanism and never as a measurement** — all seven on file quote projections, several
quote *our* real figures back at us to lend the invented ones credibility, and
[`docs/research-review.md`](docs/research-review.md) is the standing record of that.
**Ask what was measured, on what hardware, against what baseline, and say in the prompt that
projections are not wanted.** Asking for formulas as text **does not work** — budget the
image extraction instead. And a document quoting our own numbers is not automatically the
failure mode: it is the failure when it multiplies them into an invented total, and the most
valuable paragraph in the set when it checks two of them against each other. The only thing
that settles a cost here is `harness bench`, paired and alternated, against a binary built
without the feature in it.

If a batch finishes and the *numbers* look wrong but the *code* checks out — a delta with no
mechanism, or a pass that moved when nothing touched it — **do not start investigating.**
Finish and document the batch you are on, then add an entry at the front of the queue in
[`docs/roadmap.md`](docs/roadmap.md) saying what the number was, what was expected, and what
has been ruled out. Then stop. The reason is budget: a mid-session investigation is unbounded
and usually ends with two half-finished things and no measurement worth keeping. **This does
not apply to a broken build, a failing test, a capture that differs when the control says it
must not, or a control that should be free and is not.** Those are the batch, and you fix
them now — a control that costs something taxes **every** measurement taken through it
(`--no-foliage` is the precedent: **+1.24 to +2.07 ms**, the largest regression this project
has shipped, and it was in a control).

**End a batch with `harness bitexact --exe-b` against a binary built from the parent
commit.** A flag that switches a feature off is **not** a build without the feature in it:
batch 14 measured +0.24 ms through the flag and **+1.85 ms against a real revert**, an
eight-fold difference, which is the whole reason this rule exists. Build that binary whenever
you need it: check out the parent commit in a worktree, build, and copy
`target/release/voxelcraft.exe` to `voxelcraft-preNN.exe`, a name `.gitignore` covers.

**Textures are generated procedurally in `textures.rs`. No Minecraft assets are used or may be
added.**

### The hardware round, and what a sandbox session may claim (batch 94, the five asks ruled on)

- **A green line in a sandbox chat is not a compile.** Batch 91 shipped four `rustc` errors
  under the sentence "all suites compile and pass" — this sandbox's toolchain gets SIGKILLed
  before the crates graph links, so the maximum evidence class here is *written against the
  tree, parsed, not compiled*. Say which class a claim is.
- **A hardware round starts by defeating the mtime trap, not by reading numbers.** Synced trees
  arrive with their timestamps flattened (1980); `cargo` then does no work and smoke-tests run
  the *previous* binary. The gate is `find src tests -type f \( -name '*.rs' -o -name '*.wgsl'
  \) -exec touch {} + && touch Cargo.toml` before the first build of any round.
- **One runnable commit at a time** for anything touching WGSL, a binding, a pipeline layout, or
  the block table; a commit's gate is: build (after the `touch`), one screenshot, the 21-vantage
  control against the parent, `cargo test --no-fail-fast`.
- **An arm is built only when it moves pixels** at a named vantage by more than max delta 1.
  Compile-clean and test-green is eligibility, not arrival. One `bitexact` run is the check.
- **A census guard asserts a property, not an integer.** Stale counts went red five times
  across four rounds without meaning anything; guard *shape* and let the arithmetic fall out.
- **A hardware round hears the cumulative story, not the last batch's.** A "docs-only" batch
  can be true of one commit while it hides nine others; flag the tree's state since the last
  hardware round, every round.

## The machine, and what is actually scarce

**One resource is saturated and every other one is idle.** Measured on the target machine (RTX
3050 Laptop, GA107) in the batch-51 session and re-measured in batch 57 and at batch 67.
Re-measure before quoting a figure in a design — but the *ranking* is the point, and it has
never been close.

| resource | capacity | in use | verdict |
|---|---|---|---|
| **GPU shader ALU** | 5.501 TFLOP32 | `resolve` is 44-81% of frame, **instruction-bound at 80 registers since batch 67** (96 from batch 51 to 66). **16 registers cost 0.15 ms and 32 cost 0.44** — the same number for two unrelated features — and `f16` packs two values into one register on this driver (**32 live scalars: 96 registers against 128**). The lever is *where* a term is written and *what type it waits as*, not what it does. Roadmap P14 | **the wall** |
| bandwidth | 192 GB/s | `build_hiz` alone reads 16.6 MB in 0.110 ms — **151 GB/s** | contended in bursts |
| VRAM | 4 GB | screen buffers 66.4 MB, pairs 8.4, shaft 3, world data ~50, **probe field ~100.7** -- **~229 MB**. The probe field is the one row here that is *derived rather than sampled* and so cannot drift -- a claim this row itself disproved for two releases by quoting 12.0 MB after L5 grew the field: the derivation was right and the transcription was not. 128 x 768 x 128 `Rgba16Float` is exactly 100,663,296 bytes. **Every other figure is batch 51's and has not been re-measured since**; the world-data row predates batch 59 widening the light cell, so treat ~50 as a floor | **~94% free** |
| CPU workers | 16 rayon threads | busy only while streaming; **idle once resident**. Chunk build **7250 us** *(67)*, of which **probe bake 3125, worldgen 1896, lighting 1229, tree build 929, intern + upload 71** — the bake is **43% of chunk build and the largest single line in it**, which is the trade this table exists to make | free |
| host RAM | system | **6.8 MB per 96 chunks** *(67, unchanged since 59)* — geometry **1.3**, attributes + light **5.5**, over 10.6M solid voxels, plus **24 KB per chunk of probes**. Dedup is **1.00x** on both run tables, which is expected and documented in `PERF.md` — **do not "fix" it** | free |
| storage | -- | append-only journal | free |
| tensor cores | 64 (22 TFLOP16, 44 TOPS INT8) | **nothing** | untouched, and **reachable** — `EXPERIMENTAL_COOPERATIVE_MATRIX` **true**, re-confirmed by `probe` at batch 67, and naga's WGSL front end does express them (`enable wgpu_cooperative_matrix;`). See [`gpu`](docs/gpu.md) |
| RT cores | 16 | **nothing** | untouched and **reachable end to end, measured at batch 69 rather than asserted** — the ray query shader compiles and wgpu 30 built a real BLAS from AABBs with a TLAS over it. The honest prior is low: an RT core accelerates a *broad phase*, and `tile_select` already does that for 0.13-0.17 ms; the case not ruled out is incoherent secondary rays, and **the strongest case is a *bake*, not the frame** |

Frame cost at 1920x1080 with hi-z and TAA on (batch 67): **9.19 ms of GPU**, `resolve`
**5.85** of it — **64%** — against `march`'s 2.37, `taa` 0.50, `select` 0.16, `shaft` 0.11,
`hiz` 0.10. **The VRAM row is the stale one and says so in the table.**

**Three questions, in this order, before any bench.** *Which resource pays* — work added to
chunk build is paid out of a resource nothing is using; work added to `resolve` is paid out
of the only one that is full, so an appearance idea should be priced at its *stage* before it
is priced in milliseconds. *Where does this value die* — a term whose arithmetic is nearly
free can cost 0.3 to 0.5 ms by holding two vectors live across the gather, and `resolve` at
80 registers means the next 16 are a whole occupancy step. *What is this value while it
waits* — `f16` packs two to a register, but only for storage across a live range: a
transcendental widens back to `f32` and takes everything feeding it along.

**So the question is never "can this machine afford X" — it is "can X be moved off frame
ALU."** Worldgen already is (**7.5 ms per chunk on rayon against 84 us on the main
thread**), and batches 57, 58 and 60 put three bakes beside it — the cube, the bounce, the
exposure — each riding the march that already existed, each reading one tap on the frame,
each paying nothing measurable. **Batch 59 is the exception that names the rule**: block
light's three channels had nowhere off-frame to go, because what varies is the value at
every *cell* — a rung that replaces a constant with a *field* reads one tap and can be free
or better; a rung that replaces a constant with *content* has to widen a per-cell store,
where every reader pays. **Ask which kind a rung is before asking which resource pays for
it.** Batch 61 adds the last question: it aimed at the direct term, was built, measured at
**+2.639 ms**, and reverted for moving **471 pixels of 921,600** — *what share of the
frame's light, and what share of its pixels, can this rung reach.*

**Four instruments, each cheaper than a bench and each of which settled something a bench
could not.** Reach for them in this order:

1. **The stage.** Ask which resource pays before asking how many milliseconds. The table above
   is the whole argument.
2. **A crop.** Batch 52: a constant nothing in the fixture could see became rankable in one
   command by building the camera that could see it. **A blind vantage is not a conservative
   one** — it cannot argue in either direction.
3. **A count.** `harness march` counts DDA steps instead of timing them, so a traversal change
   ranks on a hot or busy machine and **a bit-exact one still reads positive**. Batch 53
   ranked three candidates in four minutes with a *deliberately incorrect* build: patch one
   literal to a wrong value, count, throw the picture away.
4. **Playing it.** Batch 54 is the first defect this project found that way, and no metric on
   file had it.

**Five capability beliefs this file got wrong about its own machine, recorded so they are not
re-derived — the card is 5.501 TFLOP32 not 4; wgpu 30 does expose ray tracing (Vulkan and
DX12, AABBs, no mesher required); the tensor cores are reachable; a binding array of textures
is available and the 3D texture limit is 16384, so the probe lattice — 768 in its largest
axis — has twenty-one times the headroom in every dimension before any *limit* binds; and
`SHADER_F16` + `SUBGROUP` are available and lower to real SPIR-V, `f16` being the only idle
capability that reaches the register wall rather than routing around it. The general lesson,
produced three times in a row: **a design note asserting what the hardware cannot do should
be written as a `probe` row or not written at all** — a belief about a capability is one line
of `probe` away from being a fact.** The full stories live in
[`docs/gpu.md`](docs/gpu.md) and [`docs/research-review.md`](docs/research-review.md).

## The environment

**This tree is under git as of the batch-27 documentation pass.** Before that it had no `.git`
for five batches, which is why several "reproduces the pre-batch-N build" claims in the ledger
are historical assertions rather than live measurements — the revert binaries for batches 1
through 20 do not exist and cannot be rebuilt.

**`.gitattributes` is `* -text` and has to stay.** Git for Windows sets `core.autocrlf=true`
at *system* level on this machine, which would rewrite much of the source on the next
checkout; `-text` plus a local `core.autocrlf=false` turns that off. **The tree's line endings
are genuinely mixed and follow no rule you can rely on**: the post-export census counts
**117 CRLF-only files against 53 LF-only** — most of `src/` and `tests/` arrived CRLF from
the export, while `src/menu.rs`, `src/settings.rs`, `Cargo.lock` and the newer `.md` docs
are LF. Check the file before anchoring an edit on a newline.
[`docs/pitfalls.md`](docs/pitfalls.md) carries every other trap this environment produces:
the heredocs that fail, the `tail` that eats a cargo run, the backslash mangling, the
`newline=''` every Python write needs, and the `grep` that reports the whole tree as CRLF.
**Open it before you start editing, not after.**

================================================================================
