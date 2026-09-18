
# CLAUDE.md

Guidance for Claude Code working in this repository.

## What this is

`voxelcraft` — a Minecraft-style voxel engine in Rust that renders by ray tracing sparse
trees in compute shaders. **There is no mesher and no triangle geometry anywhere.** If you
find yourself reaching for greedy meshing, vertex buffers, or a depth buffer, you have
misread the architecture.

## Where everything is

**This file is the whole briefing.** Everything else is read on demand, and the table says
when. Nothing here is organised by batch any more — it is organised by what you are working
on, because a decision filed under a batch number is a decision nobody can find.

| file | read it when |
|---|---|
| [`docs/roadmap.md`](docs/roadmap.md) | **once, at the start** — what to do next, and nothing else. **Every entry in it is open**; a shipped one leaves the file entirely and becomes a [`ledger.md`](docs/ledger.md) row. It was rewritten after batch 56 around the user's own statement of the goal — *a path-tracing-esque appearance with maximal performance, reached by spending the resources this machine is not using* — replacing the three ranked goals of batch 47, and the reason is recorded at the top of the file rather than applied quietly: that split put the lighting rebuild behind a performance queue meant to fund it, and batches 55 and 56 measured its read side at **net negative cost**, so it never needed funding. **The main sequence completed at batch 65, and batch 68 drained the file from 1,119 lines to 845** — what is open is D2b, D1, the P entries, the appearance list and the game. That drain is also the standing warning: **the roadmap is the file you are already in when a batch ends, so it is where everything lands**, and ten batches of shipped-work retrospective had accreted inside it while an *open* entry, P11, had been deleted outright with four documents still pointing at it. If a batch produces a finding rather than an entry, file it in [`lessons.md`](docs/lessons.md) the same session. **A reference anywhere in this tree to a roadmap designation no longer in the file resolves through *Where a closed designation went*, one table near the top** — and that includes `L1`, which is `R1` and shipped as batch 57. Six documents still say *roadmap L1* and were deliberately not rewritten: batches 55 and 56 genuinely referred to *L1*, and **falsifying six documents to tidy a name is how a claim comes to exist in two versions.** The table is the one hop instead. **And the converse, batch 93, after a queue lived three designations in chat and one in the tree: a designation must not be used in a status report until it exists as an entry in that file** -- a queue that lives only in the context window is a queue the next session inherits as code without reasons, worse than the batch number above because a chat log is not even grep. The filing is cheap: P-E is an entry now, P-A and P-D are rows in the closed-designation table, and P-B is burnt. **And the goal that file is ranked around restated itself on 2026-09-16: "lean with that look", two modded-client reference frames filed in [`docs/`](docs/) as `look-reference-*.png`, look-first with richer *procedural* textures, the G-series in the appearance list (G1 built dark at batch 95) is the queue it opened** |
| [`docs/pitfalls.md`](docs/pitfalls.md) | **before you type a command** — traps that fire at the moment of an action |
| [`docs/errors.md`](docs/errors.md) | **before you design anything** — known defects, and what has been tried and lost, with the number it lost by |
| [`docs/lessons.md`](docs/lessons.md) | **when you plan a batch**, keyed on the kind of batch. It also holds the two instruments a *look* batch needs before it starts: the **MAE calibration ladder** — 0.372 invisible, 2.87 rejected by eye, 5.98 shipped — and the three questions to ask a rung in order (*what share of the frame's light, what share of the frame's pixels, which resource pays*) |
| [`docs/harness.md`](docs/harness.md) | **before you measure anything** |
| `PERF.md` | the current frame cost, memory, worldgen |
| [`docs/adr/`](docs/adr/) | **when an entry looks unfinished rather than decided** -- one file per design that was built, measured and *reverted*, so it is in no commit and no ledger row. `0001` is roadmap R2b and `0002` is roadmap R5, and they exist because a reverted experiment leaves nothing behind for the next session to find. **Both were the next batch in the roadmap's own header when they were built**, which is what the directory is really for |
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
cargo clippy --release --all-targets     # silent since the audit-triage batch cleared 15 `x =!y` instances; keep it that way, and re-run rather than quoting
```

Headless modes need no display and are the primary way to verify changes:

```bash
cargo run --release -- --screenshot out.png --width 1280 --height 720 --hud
```

```bash
cargo run --release -- --bench-frames 140 --width 1920 --height 1080
```

```bash
cargo run --release -- --bench-terrain 96
```

```bash
cargo run --release -- --march-stats --screenshot out.png --width 1280 --height 720
```

**`--march-stats` is a count and never a timing sample** -- it switches the heatmap on, so the
marcher pays an `atomicAdd` per hit and the capture it writes is the heatmap rather than the
frame. `harness march` drives it across the set; see [`docs/harness.md`](docs/harness.md).

```bash
cargo run --release --bin probe          # adapter features and limits
cargo run --release --bin shaderstats    # registers, spills, binary size per pass
```

**Clippy is clean and every `allow` carries a reason at its own site. Do not run `clippy --fix`
blind here** — three of its suggestions would cost real meaning; see
[`docs/pitfalls.md`](docs/pitfalls.md).

**The measurement fixture is `--bin harness`, and it is where a batch starts and ends.**
`capture` and `validate` at the start, `implies` and `bitexact` at the end. **`march` is the
fourth, added in batch 53, and it is the only one of them a hot or busy machine cannot
corrupt** -- it counts the marcher's DDA steps rather than timing them, so a traversal change
is rankable before a bench exists and a *bit-exact* one still gets a positive reading. Do not hand-roll a
vantage, a crop, a metric, a hash loop or a bench A/B — [`docs/harness.md`](docs/harness.md)
explains why each of those is a trap, and nineteen sessions proved it.

**Ranking a repeat is a third thing again, and neither a picture nor `coherence` is it** --
that row has fallen from 1.2053x to **1.0594x** across two batches that removed canopy without
touching the metric, because **a known-positive is only as durable as the frame it was read
off**. `tests/repeat.rs` ranks a repeat on the atlas instead, with no frame in it at all.
[`docs/harness.md`](docs/harness.md) has both and the numbers.

## The controls

**Each control is the A/B partner for its feature, and a pure control reproduces the previous
build *exactly*** — so it can pin a regression rather than merely resemble one. **Free** means
the control costs nothing to switch on, which matters because a control that costs a
millisecond quietly taxes every measurement taken with it.

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
| `--no-tex-variation` | the pre-batch-36 frame: every block samples its layer in phase again, so a grassy slope reads as corduroy. **32% of the default frame moves**; `coherence` is **1.0594x** at `default/slope` and has fallen twice as canopy left the frame — [`harness`](docs/harness.md) before you trust that row | free (in `SPEC_MASK`) |
| `--no-leaf-cutout` | the pre-batch-38 frame: leaves and pine leaves are solid cubes again rather than a 4^3 carved mask, so a canopy is a green box. **10,272 pixels of 230,400 move at `default`**, 2,508 of 921,600 at `lod` and all of them in the near field, because the cutout is LOD 0 only | free (in `SPEC_MASK`) |
| `--no-glass` | the pre-batch-38b frame: a pane is an opaque cube again. **The one control here that is not a pure revert** — glass is `opaque: false`, a *world* change `light.rs` bakes into a chunk that no pipeline override can undo, so at a vantage holding glass it is the old shading over the new lighting. Every vantage that predates the batch is bit-exact. [`shading`](docs/shading.md) | free, and **recovers 0.346 ms** at `default` and 0.520 at `terraces`; the shipping build pays +1.036 at `coastline` for glass it never draws |
| `--no-water-shadow-cut` | the pre-batch-41 sea: a surface reached *through* the water marches its shadow 48 blocks again rather than 16. Worth **1,309 pixels of 921,600 at max delta 1** at its worst, because batch 37's envelope picks up where the march stops. [`water`](docs/water.md) | free; **the one control here that costs time to switch *on*** — +1.672 ms of `resolve` at `coastline`, +0.531 at `default` |
| `--no-leaf-thin` | the pre-batch-43 canopy: a leaf block keeps **0.72** of its 4^3 cells instead of 0.62, so a wooded crest is the blocky edge of a cube again. Swept by eye in batch 43 — the silhouette wants it low, the crown's interior wants it high. [`foliage`](docs/foliage.md) | free |
| `--no-flat-secondary` | the pre-batch-45 frame: a hit reached *through* water or glass takes the nine-cell light gather again rather than one lookup, so a submerged step gets its contact shading back. **230,613 pixels of 921,600 move at `terraces`** and 0 at `underwater`, and it is the largest saving here since Hi-Z: **-1.68 ms at `shore`, -1.58 at `terraces`, -0.50 at `default`**. The gate is the override *alone* -- adding the usual `frame.flags` test costs +1.42 instead, see [`gpu`](docs/gpu.md) | free |
| `--no-water-far` | the pre-batch-48 frame: a submerged camera tests every resident chunk out to the full view distance again, rather than stopping a tile whose every ray is still under water at `WATER_FAR_DIST`. **Bit-exact at sixteen of seventeen vantages, and the seventeenth is the point** -- batch 52 built `sea-horizon` because "bit-exact everywhere" was a statement about the *cameras*, and this control moves **67,621 pixels of 921,600 at max delta 23** there. Above water it is unreachable -- the bit is only ever read ANDed with `FLAG_UNDERWATER`, which is why it is the one control flag deliberately *outside* `SPEC_MASK`. [`water`](docs/water.md) | free; **-2.776 ms at `sea-horizon`** and **-1.125 at `underwater`** to switch on, at the 128-block reach batch 52 shipped. It was -0.835 and -0.291 at batch 48's 256 -- **the control's own cost tripled when the constant moved**, which is the clearest statement on this table that what a control is worth is a property of the build and not of the flag |
| `--no-water-dark` | the pre-batch-53 frame: a submerged tile still under the sky flood's reach looks as far as `--no-water-far` lets it rather than stopping at 64 blocks. **The depth is derived rather than authored** -- `light.rs` takes one sky level off per block of water and floods one per step in every direction, so a cell 15 blocks down holds sky level exactly 0 however the light got there, and past that reach the content is unlit *and* absorbed with no gap between the two covers. **Nested inside `--no-water-far`, which therefore clears it**; the only control here that is cleared by another one rather than clearing others. **Bit-exact at seventeen of eighteen vantages, and that is a fact about the cameras rather than about the feature** -- the rung tests the *eye's* own depth, and `deep-water` at 24 blocks down is the only camera in the set that can trigger it. There it moves **19,576 pixels of 921,600 at max delta 3**, against batch 52's 67,621 at 23 for a band twice as far out, removes **6.8% of the frame's DDA steps**, and is worth **-0.513 ms of an 8.71 ms frame** there. 96 is the retreat rung at 1,993 pixels; there is no rung below 64, because the cull is per chunk AABB and 48, 32, 24 and 16 render one identical image. [`water`](docs/water.md) | free; **-0.513 ms at `deep-water`** to switch on -- -0.278 of `march` and -0.215 of `resolve`, the second half because `resolve` reaches `march_chunk` through its own secondary rays -- and nothing anywhere else, since no other camera in the set is deep enough to fire it
| `--no-snell` | the pre-batch-54 frame: a submerged eye sees straight through the water surface at every angle, which is what it did from batch 8 until a player 32 blocks down reported a mountain drawn above the horizon. **The first defect in this project found by playing rather than by a metric.** With it on, the surface exists from below: outside the critical angle -- 41.4 degrees above horizontal, and `asin(1 / WATER_IOR)` decides it, there is **no angle constant in the tree** -- it is a mirror, and inside it Fresnel's curve. **What the mirror shows is `underwater_body()` and not a traced ray**: the traced version was built, cost 4 ms of `resolve`, and differs from the free one by a max channel delta of 3 to 7, because absorption converges a mirrored ray to exactly that colour on the way back. **The ramp is the user's own**, two corrections to the first build turned into one `smoothstep` over the *eye's* depth: no mirror at all in the top 3 blocks, full by `WATER_DARK_DEPTH`'s 15, with the surface light fading out over the same interval to `SNELL_DIM_MAX`. **17 of 18 vantages bit-exact, and that is a statement about the cameras** -- fifteen are dry and the two submerged ones sit exactly at the ramp's zero, so `deep-water` is the whole of the fixture's ability to see this. There it moves **415,642 pixels of 921,600 at max delta 136**. [`water`](docs/water.md) | free; **+0.262 ms of `resolve` at `deep-water`** to switch on, and nothing measurable anywhere else -- `coastline` +0.044 +/- 0.087 and `terraces` +0.042 +/- 0.057, both inside their own error, so the 15,232 bytes it adds to `resolve` cost nothing at the fifteen cameras that never run it. **The first build cost 6.49 ms** and traced a ray for the mirror; that ray was worth a max delta of 3 to 7 and was removed
| `--cloud-cover 0` | no cloud deck. **No longer skips the shaft integral** — batch 35 gave it a second occluder | free |
| `--no-terrain-shafts` | no ridge casts into the haze. **10 of 14 vantages move**, 258,498 pixels at `low-sun`. **No longer the pre-batch-35 frame on its own** -- batch 37 gave the envelope a second reader | free (in `SPEC_MASK`) |
| `--no-distant-shadows` | the pre-batch-37 frame: the sun term stops at `shadow_dist` and nothing shadows anything past 220 blocks. **7 of 14 vantages move**, 18,062 pixels at `lod`, 9,411 at `lattice`. Pair it with `--no-terrain-shafts` for pre-batch-35 | free (in `SPEC_MASK`) |
| `--cloud-shadow 0` | **the deeper of the two god-ray controls, and since batch 35 not a pre-batch-11 revert on its own** — pair it with `--no-terrain-shafts` | **+0.09-0.11 ms** |
| `--godray-strength 0` | keeps the ground shadow, so **not** a pre-batch-11 revert on its own. It is the one control that switches off *both* occluders | — |
| `--no-fade` | the old hysteresis deadband instead of the dissolve | — |
| **`--no-shadow-share`** | the pre-batch-62 grid: across an LOD cross-fade the chunk index every *secondary* ray reads resolves to the **fine** half unconditionally, rather than to the half carrying most of the rays. Roadmap D2, and **the first entry in this table the user found by playing** -- they ran batch 61's rejected soft-sun diagnostic and reported that a large mountain's shadow popped without it and not with it, which no still in that batch could have shown. `fade` is *a pure function of distance*, so the fine half entered `visible` the instant a chunk crossed the band's outer edge, at share ~0 -- the surface still crisply coarse while its shadow had already switched silhouette **in one frame**. The two representations differ by **31,530 pixels of 921,600 at max delta 34** at `lod`, 66x the whole of the soft sun that was masking it. **It is not a `SPEC_MASK` bit and not a `frame.flags` bit** -- no shader reads it, the change is which index the CPU writes into `grid` -- which is `--no-fade`'s shape. **It is not that there was no bit left** -- an earlier draft of this row said batch 61 had spent bit 31 and it had not: that batch was *reverted*, so `FLAG_PROBE_SUN_HIGH` at bit 30 is still the highest and bit 31 is free. The reason is the honest one, that no shader reads this. **10 of 19 vantages move with it off**: `open-sea` 13,853 px at max 132, `lattice` 9,547, `lod` 4,208, and single digits elsewhere. **`open-sea` leading rather than `lod` is the thing to know** -- that grid is read by the nine-cell gather's cross-chunk lookups as well as by shadow rays, so this is not a shadow-only control. [`lod`](docs/lod.md) | free, and **switching it on costs 0.084 ms at `lod`** -- the control is the *slower* arm, because resolving a fade to the fine half marches a more expensive tree. Indistinguishable from zero at `open-sea` (+0.044 +/- 0.031) |
| **`--no-offscreen-shadows`** | the pre-batch-64 renderer: a chunk the view frustum rejects is not uploaded, so it is in no `grid` cell, so **it casts no shadow**. Roadmap D3, and **the second entry in this table the user found by playing** -- they reported that *"the shadow should not depend on me the player but it does depending on my position and angle ... the large mountain shadow becomes shorter or larger like a weird pop in pop out"*, and the mechanism was one grep: `build_chunk_list` frustum-culled into `visible` and `visible` was the only thing that filled the grid. **Turn until a mountain leaves the frustum and its shadow leaves ground that is still on screen.** It is `--no-shadow-share`'s shape one level up -- not a `SPEC_MASK` bit and not a `frame.flags` bit, because no shader reads it: what changes is which chunks the CPU uploads. **The two sets are concatenated rather than merged, which is why `tile_select` pays nothing** -- `frame.chunk_count` is the frustum-culled length and that pass is its only reader anywhere, so the tail is reachable only through `grid`, which is indexed and carries no count. **The tail is bounded by the lattice and no new constant appears**: a chunk covering none of the 32x8x32 grid can never be named by it. At `default` that is **157 chunks drawn and 346 in the grid** against `MAX_CHUNKS` = 8191. **4 of 20 vantages move with it off** -- `offscreen-shadow` 18,468 pixels of 921,600 at max delta 31 (**MAE 2.6165** over its `shadowed-slope` crop), `terraces` 7,843 at 11, `lattice` 32 at 15, `open-sea` **1 at 1**. **That last one is D1's exact signature and is not D1**: A-against-A there is clean 8 of 8 and the A/B reproduces the pixel 3 of 3. **Sixteen reading zero is not margin, it is the band being narrow** -- an occluder has to be more than **52 degrees** off the view axis before the frustum culls it at all, *and* within `shadow_dist`'s **220 blocks** of the ground it darkens, past which batch 37's envelope carries the shadow off a height window no frustum touches. That is why a low sun reads **0**: at `--time 0.26` every shadow runs past 220 blocks. **And the defect it is named for is still not something one frame can show** -- at `--time 0.28 --cam-height 80` the control moves **29,240 pixels at yaw 265 and 0 at yaw 275**, ten degrees apart with the same slope in both. `tests/shadow_grid.rs` is where that lives as an equality. [`lod`](docs/lod.md), [`harness`](docs/harness.md) | free, and **switching it on costs 0.100 ms of `resolve` at `terraces`** (+/- 0.020, t = -4.98, and -0.105 +/- 0.023 in the Hi-Z-off arm, which is the same number twice). Nothing distinguishable at `default` (-0.050 +/- 0.042) or at `offscreen-shadow` itself (-0.014 +/- 0.047) -- **so the cost does not track the pixels that moved, and it should not**: what is paid is `march_chunk` called on grid cells that used to read `NO_CHUNK`, and a ray that now *hits* terminates early while one that still misses has paid for a traversal. `tile_select` is **+0.000 to +0.001 at all three vantages and in both Hi-Z arms**, which is the concatenation doing exactly what it is for |
| `--no-taa` | the single-frame image, bit-identical to pre-batch-6 | — |
| `--jitter X[,Y]` | pins the sample offset; `--jitter 1,0` reproduces an unjittered capture translated one pixel | — |
| `--anim-time F`, `--anim-rate F` | both ship at 0. **The rate is the load-bearing half** — with a pinned phase nothing can ghost | — |
| `--wave-clamp F` | how far the traced reflection may tilt. Ships at **0** by measurement | — |
| `--reference N` | converges over an N x N jitter grid and prints its own noise floor. **The only thing allowed to rank a look change** | — |
| `--scale F` | trace at F times the presented resolution. **Not a supersampler** — exact only at F = 2 | — |
| `--no-full-march` | the pre-batch-72 frame: every ray stops at a mixed root-FULL chunk's face, `resolve`'s water-ignoring legs included. Roadmap **P10** -- a chunk wholly under the sea surface and holding the floor is `FULL_BIT` at the root, and the pre-batch fast path returned a hit there without asking whether the ray ignores water: **31,815 pixels at `coastline`, max delta 95**, measured at batch 53, all belonging to the refracted, reflected and shadow legs. The fix marches the chunk as if its dense tree existed -- occupancy is all ones for free, and the attribute run is the dense one `leaf_attr` has always read. Ships on; the control keys the pipeline through the second specialization word (`FLAG_HI_FULL_MARCH`) and is bit-exact to the pre-batch frame by construction. [`tests/full_march.rs`](tests/full_march.rs) | **cost class: traversal, not regulation** -- the fix only moves rays that were *wrong*, which means the substitution is a per-cell march where a face hit stood, and only in mixed full chunks. Primary and water-seeing rays still stop in the entry cell because a full chunk is solid from its face to them |
| `--no-idle-repaint` | the pre-batch-70 cadence: the window renders every presented frame however still the world is. When the camera, the world version, the pipeline words and the sun's tick have not moved, the shipped build instead **presents the last converged frame again** — bit-identical to re-rendering it, because the blit re-reads the texture slot the last render presented and no pass runs between the two presents; only the HUD is rebuilt. The rule is data in `src/idle.rs`, the driver is one call in `app.rs`, and no headless path reaches it — every capture, bench and journal in the fixture still renders all seven passes per frame. [`ledger`](docs/ledger.md) batch 70 | the ~10 ms a still frame owes at `shore`, restored. The claim is verified by construction and by `tests/idle.rs`, not by a vantage: the control reinstates the cost, not a different picture |
| **`--no-probe-cube`** | the pre-batch-57 frame: the directional shading factor is `face_shade`'s hardcoded 1.00/0.80/0.62/0.50 per normal again, rather than that table modulated by a **baked six-axis sky-occlusion cube**. Roadmap R1's first rung, and the first time a wall's orientation *and position* decide its ambient rather than a table. The field is one 3D texture of six stacked slabs, 4 blocks per probe, world-space and toroidal, baked in `stream::build` off `WorldGen::height` on the idle rayon threads and read as the one trilinear tap batch 55 priced. **A pure revert at all 18 vantages and by construction rather than by luck**: the field stores *occlusion*, so an unbaked texel, a coarse LOD and a pinned field are all `face_shade(id) * 1.0`. **18 of 18 vantages move with it on**, max delta 13 to 62; `cave` is 30,300 pixels at max delta **1**, which is the no-sky rule holding rather than the feature reaching. [`shading`](docs/shading.md), [`src/probe.rs`](src/probe.rs) | free; **costs 0.02 to 0.04 ms of `resolve` at `cave`** to switch on -- the most surface-dense frame, and batch 55's -0.040 for one tap arriving on schedule. **Two paired readings, quoted as a range because they disagree on magnitude and not on sign**: -0.042 +/- 0.002 with the machine hot and -0.020 +/- 0.009 an hour later with it cool, the same vantage's absolute `resolve` moving 2.73 to 2.36 between them, which is `PERF.md`'s power-state caveat and not the shader. Nothing distinguishable from zero at `default` (-0.050 +/- 0.054) or `terraces` (+0.046 +/- 0.029). `resolve` 582,400 to 584,832 bytes at **96 registers either way**. The real price is off-frame: **1912 us per chunk** of bake, beside the flood's 1344, on sixteen threads that are idle once the world is resident |
| **`--no-probe-bounce`** | the pre-batch-58 frame: the ambient floor is `(0.85, 0.90, 1.00) * frame.ambient` again, one authored constant, rather than that constant modulated by a **baked ground bounce**. Roadmap R2's first rung, and **the constant it retires names itself** -- `floor_rgb`'s comment has read *"it is what stands in for the bounce light the two-source model never simulates"* since batch 3, and that comment was the acceptance test. `probe::bake` gathers the cosine-weighted mean albedo of the ground each axis can see, off the horizon march batch 57 already pays for, and stores a per-channel multiplier in the **`.rgb` of the texel whose `.a` is batch 57's occlusion** -- so the two features are four channels of *one* `textureSampleLevel` and a coloured directional field costs what the scalar one cost. **A pure revert at all 18 vantages and by construction**: the field holds a multiplier and the lattice is created holding 1.0. **18 of 18 move with it on**, max delta 1 to 46; `cave` is 27,415 pixels at max delta **1** (the no-sky rule holding) and `night` is 757,159 at 46, which is the largest in the set **because `floor_rgb` does not scale with `frame.daylight`** and predates this batch. **Two identities and only one is per channel, which a draft of this batch's own prose got wrong**: an unbaked texel is 1.0 on every channel, where *reference ground* is 1.0 in luminance alone -- over grass the multiplier is `(0.416, 1.259, 0.156)` and the floor turns green at exactly the brightness it had, which is what keeps the pair rankable. [`shading`](docs/shading.md), [`src/probe.rs`](src/probe.rs) | free, and **nothing measurable to switch on**: `resolve` -0.001 +/- 0.005 at `cave` and +0.008 +/- 0.032 at `default`, both inside their own error against a real revert -- so what is claimed is *below about 3 se*, 0.015 ms and 0.096. What raises that above "unresolved" is the mechanism rather than more rounds: **+128 bytes** (584,832 to 584,960) at **96 registers either way**, on the same tap. The price is off-frame: the bake goes **1868 to 2549 us** per chunk, +681 and **1.36x** against a roadmap that expected several times -- the gather rides the march that already exists |
| **`--no-light-rgb`** | the pre-batch-59 frame: block light is one 4-bit level wearing one authored warm constant again, rather than three levels whose ratio *is* the colour. Roadmap R4's first rung, and **the constant it retires had been the colour of every emitter in the world** -- `BLOCK_TINT = (1.0, 0.65, 0.32)` in `resolve.wgsl`, in a world batch 55 found had **exactly one emitter in it**. So the batch is content as much as code: three lamps, and the light cell widened from one byte to two, `sky:4 \| r:4 \| g:4 \| b:4`. **The channel had to widen rather than the block table, and the flood is why** -- a scalar level with a per-block tint applied at read time cannot mix, because the cell between an amber lamp and an azure one holds one number and no memory of which block put it there. **A pure revert at every world that predates the batch and not at one that does not**, which is `--no-glass`'s shape rather than `--no-probe-cube`'s: the new ids are a *world* change no pipeline override can undo. 18 of 18 old vantages bit-exact with it on. **2 of those 18 move with it off** -- `edits` 313,302 pixels at max delta **3**, `glass` 86,481 at 2 -- because glowstone's emission is now the levels whose `light_curve` values come nearest the retired constant, and 4 bits reach (1.0000, 0.7111, 0.3024) rather than (1.0, 0.65, 0.32); green is 9.4% out and 12 is 9.4% out the other way, which `tests/light.rs` asserts is the closest the channel can get. **The `lamps` vantage is the whole of the positive claim and cannot be a `bitexact` row at all** -- its chamber holds block ids the parent binary does not have, and `block::def` clamps an unknown id rather than refusing, so the journal substitution [`harness`](docs/harness.md) records for `--demo-glass` would replay it as *meadow*. There the control moves **919,746 pixels of 921,600** and an MAE of **13.10** over the `mixing-wall` crop. [`shading`](docs/shading.md), [`src/light.rs`](src/light.rs) | **not free, and it is the first control here whose picture reverts exactly and whose *time* cannot.** Switching it on **recovers 0.202 ms of `resolve` at `cave`** (+/- 0.014, one binary, eight rounds). What it cannot recover is the other **0.110** -- against a real revert the control is still that much slower, because the cell it reads is two bytes whichever arm compiles, and no override can shrink a brick. Shipping against `voxelcraft-pre59.exe`: **+0.321 ms at `cave`, +0.340 at `default`, +0.150 at `sky`**. **The 0.202 had no mechanism and now does: it is the gate itself.** Forcing `any_blk` to a literal costs three builds and settles it -- at `cave` both literals render a frame bit-exact with the shipping build, so the A/B is pure timing over one image. `0u` is **-0.351 ms** and `1u` is **+0.327**, and `0.678 - 0.351 = 0.327` closes the arithmetic. So the branch costs 0.351 to save 0.678, and the control arm wins by having no data-dependent branch at all rather than by doing less -- it does strictly more, exactly as this row said. **Ask what a gate costs before crediting it with what it skips**; roadmap P11 has the table and the open decision. `resolve` 586,624 bytes against the control's 588,928, **96 registers and 15,872 bytes of shared memory either way** -- and that last number is the batch's other finding, below |
| **`--no-sky-tint`** | the pre-batch-63 frame: the ambient's sky colour is `SKY_TINT`'s `vec3(0.6038, 0.7084, 1.0000)` again -- one authored constant multiplied into `amb` on **every shaded pixel in the world** and scaled only by `frame.daylight` -- rather than the sky model's own gradient integrated over the hemisphere each face can see. Roadmap R7, the sixth rung of the main sequence, and **the cheapest of them: no uniform, no CPU integral and no `GpuFrame` seam**. `sky_base`'s four endpoints were four literals with one reader until this batch and are now named constants in `common.wgsl` with two, which is batch 26's lesson applied before it could fire rather than after. **The integral collapses because the gradient is linear in its endpoints and `n.y` is exactly +1, 0 or -1 for every normal state this renderer has** -- a cube face is axis-aligned and `normal_of` returns `(0.707, 0, +/-0.707)` for both cross-quad planes -- so two weights cover the world: **0.78431373** for an up face, which is the closed form `2 / (2 + SKY_GRAD_EXP)` and is what says the quadrature is right, and **0.58578900** for a side face, which has no closed form. **What ships is a departure from `SKY_TINT` and not a replacement of it**, and that is the batch: the real sky is far darker than the constant standing in for it -- day zenith `(0.0637, 0.2140, 0.8276)` -- so a naive substitution would darken every shadow in the world, straight into the standing judgement that it already reads too dark and into batch 57's ambiguity between an exposure shift and a feature arriving. Two identities disarm it, both exact and both holding at every rung of the strength ladder: at the reference condition (up face, full daylight, no halo, which `--fog-scatter 0` reaches) the function returns `SKY_TINT` **to the bit**, and everywhere else the result carries `SKY_TINT`'s **luminance** by construction, so **only the hue moves**. `SKY_TINT` is now the *calibration* of a derived hue, which is what `face_shade` became at 57 and `floor_rgb` at 58. **The batch's most useful finding is that the physical answer is nearly invisible, and the user found it with an image slider before any number on file said so** -- at `SKY_TINT_SAT` 1.0 the `default` pair is **MAE 1.2185**, under the **2.87** at which [`ADR 0001`](docs/adr/0001-no-elevation-form-factor.md) rejected a change by eye. **759,140 pixels differing and MAE 1.22 are the same frame described two ways, and only one of them says whether anybody can see it**: a count answers *did the feature fire* and cannot answer *is it worth having*, and this batch read the first as the second for its whole length. `SKY_TINT_SAT` is the one authored number in it, swept by eye and **shipped at 2.5** (MAE 3.0379) over 1.0 and 4.0 -- `LEAF_FILL`'s shape at batch 43, and the same restraint the user showed choosing batch 60's quieter gain. **The sunset half is included and the entry's headline about it was wrong** -- at low sun `frame.daylight` falls and *both* gradient endpoints lerp toward the night constants, so the gradient alone gives dark blue and no orange at all; the warmth lives in `scatter_lobe`, added here as energy rather than sampled as a direction (`SKY_HALO_ENERGY` is 4, derived from `hg_gain`'s own normalisation). **`--fog-scatter 0` ranks the two halves apart without a second bit**, which matters because this is **bit 31 and `frame.flags` has no more**. **And the halo is worth most at midday and least at the two low-sun cameras, the opposite of what was promised**: the sky ambient is `sk * frame.daylight`, so at a sunset there is almost no ambient left to re-colour -- batch 60's *what share of the frame's light can this rung reach* arriving twice in one batch, once as a ceiling on the sunset and once on the whole feature. **15 of 19 vantages move**, max delta 14 to 42: `terraces` 910,104 of 921,600, `edits` 895,445, `glass` 889,047, `default` 759,958, `low-sun` 589,337, `canopy` 412,430 at the highest delta. **The four that do not are the four with no sky in them** -- `cave`, `lamps`, `night`, `deep-water` -- each by mechanism rather than luck, and **`cave` reading 0 is the batch-60 failure not repeating**. [`shading`](docs/shading.md), [`tests/sky_tint.rs`](tests/sky_tint.rs) | **0.02 to 0.045 ms of `resolve` at `cave` to switch on, where 0 pixels move** -- `sky_ambient_tint` is evaluated per shaded pixel and then multiplied by an `sk` of zero. **Quoted as a range because the two sweeps bracketing it were an hour apart on a machine that heat-soaked between them**: -0.045 +/- 0.014 (t = -3.21, a real difference) cool, -0.024 +/- 0.017 warm, with the same vantage's absolute `resolve` moving 2.804 to 2.839 and `default`'s 6.140 to 6.520 -- `PERF.md`'s power-state caveat and not the shader. Nothing distinguishable at `default` (+0.007 +/- 0.020) or `sky` (+0.015 +/- 0.017). **What survives both readings is that the cost tracks surface density**, which is batch 56's own discriminator for work against code. `resolve` 587,264 to 592,512 bytes, **96 registers either way**, and **shared memory 15,872 to 14,848, -1,024, which nothing in the batch aimed at and for which no mechanism is claimed** |
| **`--no-sky-specular`** | the pre-batch-65 frame: every opaque surface is perfectly diffuse again, with no Fresnel-weighted reflection of the sky. Roadmap R6, the last rung of the main sequence, and **the first term in this renderer added *outside* `albedo`** -- which is why it was the one rung nothing had capped. A specular reflection off a dielectric is not tinted by the diffuse albedo and is not a redistribution of the ambient: R3's rung modulated a floor `Config::ambient` pins at ~7% of a lit surface, R7's carried `SKY_TINT`'s luminance by construction so only hue could move, and both found that cap only after being built. Fresnel bounds this one instead -- `GROUND_F0`'s 4% at normal incidence but **20% at cos 0.3 and 61% at cos 0.1**. **The gate is `sky_sm` and batch 60 is why it is not `probe.a`**: that batch gated a bounce on the baked occlusion and lit a sealed chamber with 921,600 pixels, because the field stores occlusion *normalised to 1 in the open*, so a buried probe returns 1.0, which is unoccluded and not dark. **`sky_base` and not `sky_color`** -- the invariant is that one takes a ray origin and the other must not -- and it already mixes toward `horizon * 0.26` below the horizon, so an overhang reflects dark ground with no special case. **The first build was wrong in the way worth carrying**: it had no roughness term, Schlick reaches 1.0 at grazing for *every* F0, and a hillside seen from above came back a blue-white haze at **MAE 15.65, max delta 202**. What a bare Fresnel is missing is the geometric shadowing-masking term -- the microfacets that would mirror a grazing ray are occluded by their neighbours at exactly the angle Schlick says reflectance is highest. `GROUND_ROUGHNESS` caps the climb at `1 - roughness` = 0.10. **A gain would not have fixed it**: the roughness ladder 0.80 to 0.97 moved MAE only 11.50 to 10.35, so the fault was the curve's *shape*, not its scale. **It is the first control here driven by the second specialization word rather than by `frame.flags`**, which batch 63 filled at bit 31 -- see `SPEC_HI_MASK`. **16 of 20 vantages move**, max delta 10 to 62; **the four that do not are `cave`, `night`, `lamps` and `deep-water`, each by mechanism** -- `sky_sm` is zero underground, zero in a sealed room, carries `frame.daylight`, and is exhausted 24 blocks down -- and they are **the same four batch 63 found**. `SKY_SPEC_GAIN` ships at **0.5**, chosen by the user from images over 0.25 and the physical 1.0. [`shading`](docs/shading.md), [`tests/sky_specular.rs`](tests/sky_specular.rs) | **+0.051 ms of `resolve` at `cave`, +0.065 at `default` (indistinguishable from zero) and +0.044 at `sky`** against a real revert binary, **after batch 66**. As batch 65 shipped it was **+0.295 / +0.516 / +0.271** -- four times as much against a real revert binary, t = -28.0, -10.6 and -14.6. **The bill was register pressure rather than arithmetic, and batch 66 removed most of it by folding the arm away in the three inlined copies that are secondary hits** -- so a floor seen through water or glass no longer takes a sheen, which is batch 45's rule one rung later. Originally -- `resolve` was **96 registers without the term and 128 with it**, 592,512 bytes to 592,256, so the code got *smaller* while occupancy fell, and replacing `sky_base` with a constant still read 128. It is **96 again since batch 66**. **A `sky_sm > 0.0` early-out was built and rejected**: nothing at `cave` (0.295 to 0.282, inside error) and *worse* at `default` and `sky` (0.516 to 0.594, 0.271 to 0.305) -- roadmap P11 again. Batch 66 is where the ceiling was measured and P13 closed |
| **`--no-probe-shadow`** | the pre-batch-60 bake: the bounce gathers the albedo of the ground a probe can see and never asks whether that ground is itself lit, so a valley floor and an open field of the same grass bounce identically. Roadmap R3's first rung, transport half. **Unlike every other probe control this one changes what is *in* the field rather than whether the shader reads it**, so it lives on `WorldGen` and no pipeline override can undo it -- `--no-glass`'s shape without `--no-glass`'s caveat, because an exposure of 1.0 is the identity of the only operation it takes part in and the revert is therefore total. `ray_exposure` is a **maximum of slopes** over the `STEPS` column heights the horizon march already took, so the chunk's **81,920 height samples stay 81,920**. It sees one azimuth and its reverse, so a ravine running *along* a ray reads as open -- biased toward the old answer, which is the one it has to reduce to. **On its own it is invisible and that is the finding**: 644,121 pixels at `default` but an **MAE of 0.372**, against the **2.87** at which [`ADR 0001`](docs/adr/0001-no-elevation-form-factor.md) was rejected by eye. [`shading`](docs/shading.md), [`src/probe.rs`](src/probe.rs) | free on the frame; **+671 us of bake per chunk, 1.24x**, 2743 to 3414 |
| **`--no-probe-sun`** | the pre-batch-60 floor: `floor_rgb` is `(0.85, 0.90, 1.00) * frame.ambient` again, one flat authored constant, rather than that constant **plus** `probe.rgb * PROBE_SUN_GAIN * sky_sm`. Roadmap R3's first rung, energy half, and **it exists because ADR 0001 said it had to**: that ADR rejected the elevation form factor on a ceiling rather than an implementation -- `frame.ambient` is 0.08, so the floor is about **7% of the light on a lit surface** and no *redistribution* of it can be worth more. The only exit it names is a build where the floor carries materially more energy. **Added beside the authored floor and not folded into it**, so a cave keeps its sole light. **The gate is `sky_sm` and the first build got it wrong in the instructive direction**: it gated on `probe.a`, but the field stores *occlusion normalised to 1 in the open* so every absence of it is exact -- a buried probe returns **1.0, which is unoccluded and not dark** -- and the build lit a sealed chamber with **921,600 pixels** at `cave` and at `lamps`. [`shading`](docs/shading.md) | free; nothing measurable on the frame -- `resolve` +0.001 +/- 0.027 at `default`, -0.018 +/- 0.014 at `cave`, +0.025 +/- 0.015 at `sky` against a real pre-60 binary, all inside their own error |
| **`--probe-sun-high`** | **the third row here that is not a control, and the only `SPEC_MASK` bit in this project whose default is *off*.** It reproduces no earlier build; it selects `PROBE_SUN_GAIN_HIGH`'s 0.25 over the shipping 0.10, so the strength of one bounce can be ranked by eye without a rebuild. **Two constants behind an override rather than a uniform**, which is `SPEC_LEAF_FILL`'s shape and is chosen for `GpuFrame`'s sake -- that struct has no pads left and a value with two settings does not need one. `default` is MAE **5.7074** at max delta 59 against the shipping 2.2248 at 27. **The user played all three arms and chose 0.10**, with the standing note that the world already reads dark; the ladder is kept rather than deleted so the choice is recorded rather than re-derivable. **`cave` is 0 pixels between the two gains**, which is the `sky_sm` gate proving no gain can leak underground. [`shading`](docs/shading.md) | free |
| **`--probe-tap`** | **the one row here that is not a control, and the inversion is the point**: the feature ships *off* and this switches it *on*. Batch 55's diagnostic -- one hardware trilinear tap into the probe field per shaded surface, multiplied into `amb`, **on top of** batch 57's cube rather than instead of it. It implies `--probe-fill 1.0`, so the field holds 1.0, so the frame is **bit-exact at all 18 vantages** and the whole output is a millisecond. It exists to price roadmap L1's read side before L1 has a format. `--probe-fill F` is the paired positive -- at anything but 1.0 the frame *must* move, because bit-exact alone cannot be told from never-wired-up -- and `--probe-noise` fills per texel, which rules out the driver compressing a constant fill. **The field was 128^3 when this was measured and is 128 x 768 x 128 since batch L5**, so re-measure before quoting the cost column against a later build. [`shading`](docs/shading.md) | **costs 0.01 to 0.055 ms of `resolve` to switch on**, and the finding is that **three taps cost the same as one** (-0.040 at `cave` at both): the code exists, the work does not scale. 96 registers with it and without |
| **`--probe-ambient`** | batch 56, and **the second row here that is not a control**: it makes the probe field *replace* `shade_hit`'s ambient rather than multiply into it, so the terms a pre-composed directional field makes redundant leave the module -- nine `blk_n` `light_curve` evaluations, four per-corner `bk/(sk+bk)` divisions with their tint `mix`, the `amb` bilinear, `face_shade` and the ambient floor. **The frame it renders is wrong on purpose** and is thrown away; it implies `--probe-tap`, so what it reports is the architecture's *net*. [`shading`](docs/shading.md) | **saves 0.03 to 0.13 ms of `resolve`**, net of the tap -- +0.130 at `cave`, +0.091 at `default`, +0.030 at `sky`, two of five inside their own error. **The saving tracks surface density where batch 55's cost did not**, which is what says one is work and the other is code. `resolve` 582,400 bytes to 575,360, 96 registers either way |
| **`--soft-shadows`** | **the third row here that is not a control, and the one the bench refused to ship.** Batch 102a's sun-disc penumbra: three cone taps around the centre shadow ray, the tap budget anchored by the blocker distance the centre tap reports. The plumbing measured clean -- 21-vantage control exact, fold by construction, the 101i mirror rule carried -- and the near-field effect is real (**MAE 2.6480 over its strongest 160x90 crop, against the 2.87 rejected-by-eye rung**). What failed is everything else: whole-frame reach stays under the 0.372 invisible rung everywhere (0.1802 at its own tribunal view; the aperture ladder 6/10/24/48 gives 0.180/0.250/0.398/0.546 with max delta pinned at 26 -- **no setting is both visible and honest, do not tune it**), and the sign split is 86% lighter, a shadow-lifter where a penumbra must sit near 50/50. ADR 0002's wall met from the effect side; roadmap G5's boundary gate is the second attempt | **did not ship: +4.545 +/- 0.021 ms of a 7.825 ms frame at `--grass-dense` 1920x1080 (128 -> 81 fps), 13x its bar.** The decomposition is the finding -- scaffolding 0.073 ms, first tap +1.824, next two +1.33 each, an 8x march shortening worth 7% -- and lives in [`lessons`](docs/lessons.md): **a ray's bill is that it was fired, not how far it marched** |
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
`src/render/shaders/common.wgsl` with the pass files, and `Renderer::new` creates one pipeline
per entry point in `render::ENTRY_POINTS`. **`march` and `resolve` are the exceptions and get
one pipeline per specialization** — see `SPEC_MASK` in [`docs/gpu.md`](docs/gpu.md).

1. **shaft_scan** — the light envelope: a max-plus prefix scan over a 512x512 terrain height
   window, eight dispatches deep, one pipeline per doubling. **The only pass whose cost does not
   depend on the window size, on how much geometry is on screen, or on where the camera looks.**
   `resolve` reads what it writes **twice since batch 37** — once for the haze's
   shaft and once for the ground past `shadow_dist`, which is why `--no-terrain-shafts`
   no longer reverts the field on its own. See [`docs/sky.md`](docs/sky.md).
2. **tile_select** — screen in 8x8 tiles; each tile tests every chunk's **bounding box**, never
   its tree. Emits (tile, chunk) pairs into an append buffer. This is where the performance
   comes from: box tests are nearly free, so the expensive march runs only on survivors.
   **It reads a *prefix* of the chunk array since batch 64, not all of it.** `frame.chunk_count`
   is the frustum-culled length and this pass is the only reader of that field anywhere; the
   entries past it are the ones the frustum rejected, uploaded so that `grid` can name them and
   they can cast a shadow. That is why decoupling the two sets cost this pass nothing -- it
   box-tests exactly the set it box-tested before. [`lod`](docs/lod.md), roadmap D3.
3. **march** — one 8x8 workgroup per pair, dispatched indirectly. Writes hits to a 64-bit
   visibility buffer with `atomicMax`, so depth testing falls out of the atomic. **It is not
   free, and batch 49 is where that was measured**: the result is discarded, so `march` never
   waits on one -- and **most of what suppressing those writes saves is billed to `resolve`**,
   not to the pass that issued them: 91/9 at `underwater` (batch 49) and 68/31 at `sea-horizon`
   (batch 52). **The direction transfers and the ratio does not**, so bench a march-side cull on
   `resolve`'s row or you will see a fraction of it. A chunk behind a nearer one pays a
   read-modify-write to change nothing, which is roadmap P3c. [`gpu`](docs/gpu.md).
   **Batch 53 gave it the mask it should have been walking**: water is *in* the occupancy
   tree, so an ocean interior reads as solid at every level to a ray that ignores water --
   the submerged primary ray and every secondary ray in the frame. `GpuChunk::dry_mask` is
   `root_mask` with the water-only 16^3 cells cleared, and `march_chunk` traverses it
   instead whenever `skip_water` is set. **Traversal only** -- `mask_below` still indexes off
   the real mask, and using the dry one there reads a neighbouring node, which is a plausible
   picture rather than a crash. [`gpu`](docs/gpu.md).
4. **build_hiz** — per-tile farthest visible depth, consumed by the *next* frame.
5. **recover_select + march** — re-tests everything the stale Hi-Z rejected against the fresh
   Hi-Z. This is what makes Hi-Z lossless under fast camera motion. **Do not remove it.**
6. **resolve** — decodes the visibility buffer and shades, and is **two thirds of the frame**.
   Shadow rays, the nine-cell gather and water's secondary rays are paid per *visible* pixel,
   not per traced pixel. **Three materials since batch 38b** — `shade_water`, `shade_glass`,
   `shade_hit` — chosen on the block id this pass reads back out of the attributes, which is
   why neither transmissive material needed a bit in the key.
   **One material, two light models, since batch 45.** `shade_hit` is called four times in the
   worst case — the primary surface, water's refracted and reflected legs, glass's transmitted
   leg — and the last three take one light lookup where the primary keeps the nine-cell gather.
   That is a *light model* and not a material: the older rule that a secondary hit shaded by a
   cheaper **material** reads wrong still stands, and is why all four still go through
   `shade_hit`. What the three transports destroy is the contact cue the extra eight lookups
   buy, which is why dropping it there is invisible and dropping it on the primary would not
   be. [`docs/shading.md`](docs/shading.md).
   **Block light has been three channels since batch 59**, so the tint `shade_hit` mixes
   toward is derived from what reached the cell rather than read off one constant -- and the
   nine-cell gather's block half is skipped outright, exactly, wherever all nine cells have
   every block channel at zero, which is eighteen of the nineteen vantages. See
   [`docs/shading.md`](docs/shading.md) and roadmap R4.
   **Since batch 57 the directional half of the ambient is baked rather than tabulated.**
   `face_shade`'s 1.00/0.80/0.62/0.50 per normal is still there and still decides what an
   *unoccluded* face looks like, but it is multiplied by one trilinear tap into a six-axis
   occlusion field -- so the table is now the open-field calibration of a derived number
   rather than the number. See [`docs/shading.md`](docs/shading.md) and roadmap R1.
   **And since batch 60 the ambient floor is no longer flat.** `floor_rgb`'s authored constant
   is still there and still the sole light in an unlit cave, but a second term is *added*
   beside it -- the baked bounce times a gain times `sky_sm` -- so a lit surface's floor now
   carries the colour of the ground near it and goes to zero underground and at night through
   the same per-voxel term that gates the sun. **The batch's own finding is the ceiling, not
   the term**: that floor is about 7% of the light on a lit surface, so no refinement of it is
   worth more than 7%, which is why roadmap R3's remaining rungs have to touch the direct term
   or the gather instead. See [`docs/shading.md`](docs/shading.md) and roadmap R3.
   **And since batch 65 a surface is not purely diffuse.** Everything that is not water or glass
   was perfectly Lambertian from batch 1 until roadmap R6; now a Schlick weight at `GROUND_F0`
   carries `sky_base` in the mirror direction, **added outside `albedo`** rather than folded into
   any of the terms above -- a specular reflection is not tinted by the diffuse albedo and is not
   a share of the ambient, which is the whole reason that rung was worth opening after two that
   turned out to be capped. It is gated by `sky_sm`, so it is exactly zero in a cave and at night.
   Its cost is not its arithmetic: it takes `resolve` from 96 registers to 128, and that occupancy
   loss is why a frame with no changed pixels still pays 0.295 ms. See
   [`docs/shading.md`](docs/shading.md) and roadmap R6 and P13.
   **And since batch 63 the sky half of the ambient has the sky's own colour.** `SKY_TINT` is
   still there and still decides what the ambient looks like at one reference condition, but it
   is multiplied by the ratio between the sky gradient this face sees and the gradient at that
   reference -- so the constant is the *calibration* of a derived hue rather than the hue, which
   is the third time the same move has been made in seven batches. **The result carries
   `SKY_TINT`'s luminance by construction, so only the hue ever moves**, which is what keeps the
   before/after pair a ranking of colour rather than of exposure. See
   [`docs/shading.md`](docs/shading.md) and roadmap R7.
   **And since batch 54 one *boundary* as well as three materials.** A submerged ray does not
   hit the water surface -- `FLAG_UNDERWATER` makes water non-blocking for the march -- so
   `surface_from_below` reconstructs the crossing analytically and applies Snell's window:
   past the critical angle the surface is a mirror onto the medium's own colour. **A material is what a
   ray hits and this is what it passes through**, which is why it is a term in `resolve`'s
   body rather than a fourth arm of the `block_at` compare. [`water`](docs/water.md).
7. **taa** — reprojects the previous frame, clips to the 3x3 neighbourhood and blends. Runs
   **in linear, before the tone map**, and is the only pass reading a texture the previous
   frame wrote.

### Invariants

These will bite you. They are load-bearing, not stylistic. Subsystem-specific ones live with
their subsystem; these are the ones that cross files. **Every one is a decision *this engine*
made that binds somewhere else.** The wgpu and WGSL traps that used to sit in this list are in
[`docs/pitfalls.md`](docs/pitfalls.md) now: they fire identically in any project that uses those
APIs, so they are things to know, not things this engine chose.

- **The camera's own chunk is exempt from the box test.** From inside a box, ray-box
  intersection returns the *exit* distance, which would cull the chunk you are standing in.
- **Bind group 0 and the `@group(0)` declarations at the top of `common.wgsl` are one list
  kept in two places.** The entry count lives in `render::BIND_GROUP_0_ENTRIES` and in no
  sentence anywhere; `make_layout`'s array annotation pins the Rust half and
  `bind_group_0_matches_the_shader` pins it against the shader.
- **The engine requires `SHADER_INT64` + `SHADER_INT64_ATOMIC_MIN_MAX`** and refuses to start
  without them rather than falling back silently.
- **Visibility key layout is `depth:24 | normal:3 | chunk_id:13 | voxel_xyz:24`.** The 13-bit
  chunk id caps the per-frame list at `MAX_CHUNKS = 8191`. **The key is now full.** The voxel
  packs as 8 bits per axis where 64^3 needs 6, and batch 38 spent bits 6 and 7 of each byte on
  the 4^3 micro-cell of a carved leaf -- six needed, six free, one pair an axis. `normal:3` has
  been full since batch 14 took states 6 and 7 for the cross-quad. **A fourth primitive shape
  has nowhere left to go** and has to widen the key or take depth precision.
- **A new *material* is not a new primitive shape, and batch 38b is the proof.** That entry's
  design said one free bit would tag a glass hit; batch 38 had spent the last six and there was
  no bit to take. It turned out never to have been needed: `resolve` already calls
  `block_at(ci, v)` to tell water from everything else, so the id *is* the tag and glass is one
  more arm of that compare. **Reach for `block_at` before reaching for the key** — a shape the
  marcher has to resolve differently needs bits, and a surface `resolve` has to shade
  differently does not.
- **`hit_t` reconstructs the hit plane in micro units unconditionally, and must not branch on
  `SPEC_LEAF_CUTOUT`.** It reduces to the cube's own face exactly, because `march_chunk`
  records the *entry* micro-cell on every solid hit. A branch there -- in any of the three
  shapes tried -- makes the driver rebuild the folded function one ULP away and costs the
  control its bit-exactness for 201 pixels. `hit_t_does_not_branch_on_the_cutout_override`
  is the guard; the reasoning is at the function.
- **`shade_hit`'s `shadow_dist` parameter is a boundary between two occluders, and both sides
  of it have to be handed the same number.** The march covers `[0, shadow_dist)` and batch 37's
  light envelope covers `[shadow_dist, reach)`, and the cancellation that makes that a partition
  rather than a blend is *only* valid when `terrain_shade_beyond` is sampled at the distance the
  march just used. It is what lets a budget be shortened cheaply -- and a later edit that passed
  either reader a constant of its own would open a gap or double-count, with nothing in any
  frame to say so. `the_envelope_resumes_where_the_water_shadow_march_stops` is the guard.
- **The visibility buffer holds one hit per pixel, so exactly one chunk may march a given ray.**
  That is why the LOD cross-fade is a *partition* of the rays and not a blend weight — see
  [`docs/lod.md`](docs/lod.md).
- **Everything the compute passes compute is linear radiance.** `blit.wgsl` is the one place
  that tone-maps and encodes. **A `pow()` anywhere in `resolve`, or a display-referred constant
  in `common.wgsl`, puts the pipeline back where batch 2 found it** — see
  [`docs/shading.md`](docs/shading.md).
- **`sky_base` is the sky without clouds, `sky_color` is the sky with them, and `sky_color`
  takes a ray origin while `sky_base` must not.** Getting either wrong is invisible in a still
  capture — see [`docs/sky.md`](docs/sky.md).
- **`block_faces` strides by eight, and every WGSL call site has to say so.** A wrong stride
  returns a *plausible* layer rather than garbage, which is why this is an invariant and not a
  bug report. `faces_per_block_matches_the_shader` holds every call site to it.
- **A `block_faces` word is not a layer.** Since batch 36 it is the atlas layer with
  `block::PERM_CLASS` packed above it, so a raw word used as a layer indexes the atlas far past
  its end. `face_layer` and `face_perm` are the only two ways in, and
  `face_layer_at_every_call_site` holds every reader to them -- including the one local that is
  allowed to hold a raw word, which has to be named `*_word` to say so.
- **New block ids are appended, never inserted -- and an id and its table row are one list
  kept in two places.** `const _: () = assert!(block::WATER == 13)` is the only thing tying
  that id to its literal in `common.wgsl`. **The same rule binds the `BLOCKS` row, which is
  what batch 94 learned at 865,982 pixels of `lamps`**: an id declared 23 whose def row sat
  at position 20 re-named every batch-59 emitter by one id, every new flag off. The invariant
  is now enforced in `block.rs`'s const asserts (the tail id, its row, and the lamps' light
  channels above it), not by care with the wording.
- **A block id reaching the hotbar is checked at three edges, because the `const` assert that
  used to be enough can only speak for a `const` array.** Batch 34 made the bar's contents
  runtime state, so `block::bar_slot_refusal` is the rule and the journal, `--bar` and
  pick-block are the three callers. `TALL_GRASS` is the one that matters: `SPEC_FOLIAGE`
  compiles the marcher unable to see a tuft, so a placed one is invisible rather than wrong.
- **`GpuFrame` has no pads left, and the last two batches that wanted a number did not take
  one.** Five batches appended **four** words before `prev_view_proj` -- four is the only count
  that keeps the mat4x4 16-byte aligned without inventing a pad -- and five `offset_of!` asserts
  pin the seams, because a size assert alone cannot catch two swapped fields. **Batches 41 and 43
  both needed a tunable constant and both used an `f32` `override` in `SPEC_MASK` instead**: the
  value folds into the pipeline, the control costs nothing, and the struct does not grow. Reach
  for that before reaching for a field — a uniform is for something that changes *during* a run.

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
  counts, not from `old_l1.leaf_prefix`** — an absent L1 node carries prefix 0 and would insert
  at the front of the attribute table, silently corrupting an unrelated block.
  `tests/edit_regression.rs` guards this.
- `light.rs` (at `src/light.rs`, not under `src/voxel/` -- it shares this section's
  subject and not its directory) — sky/block flood fill into 4³ bricks. **Light lives in *air* voxels**, which have
  no leaf index, so it cannot share the attribute indexing. **A cell is two bytes since batch
  59** -- `sky:4 | r:4 | g:4 | b:4`, so a brick is 32 words rather than 16 and
  `attributes + light` went **4.2 MB to 5.5 MB over 96 chunks**, 0.416 to 0.541 bytes per solid
  voxel. **The propagation rule did not change**: `flood` is called three times on three arrays
  rather than taught about colour, which is what makes `max(r, g, b)` the level the single
  pre-59 flood held and therefore makes `--no-light-rgb` exact rather than close.
- **Three separate questions that used to be one:** does it occupy a voxel (the tree), does it
  block light (`BlockDef::opaque`), does it block the player (`BlockDef::solid`). Water is
  yes/no/no, foliage is yes/no/no, and **glass is yes/no/yes since batch 38b** — the first
  block that stops the player and not the light, which is what makes the three genuinely
  independent rather than two labels for one answer.
- **`set_block` records every accepted edit into a journal, and `stream::build` replays it
  into the dense array before the tree, the attributes or the lighting read it.** One `Arc`,
  shared by the world that records and the streamer that replays, empty unless `--edits` or
  `--load-edits` filled it. It is why a chunk that unloads and regenerates comes back edited,
  and why a reloaded world is bit-identical to the one edited live. **`--edits` appends every
  record to the file as it lands**, so the file is the world at every instant rather than only
  after a clean exit, and what makes that safe is the order of the writes rather than the
  appending. **The coarse levels aggregate
  the deltas under each of their voxels rather than taking an edit**, and an edit marks
  every stand-in over it stale, because `apply` reads the journal once, at build time. **The
  file also carries the player and, since batch 34, the bar's contents**, each as an optional
  block announced by its own bit of `flags`, which is what lets three file versions load
  through one reader with no branch on the version number --
  [`persistence`](docs/persistence.md).

- `probe.rs` (at `src/probe.rs`, beside `light.rs` and for the same reason) -- batch 57's
  six-axis sky-occlusion cube, one probe per 4 blocks, baked in `stream::build`. **It reads
  `WorldGen::height` and never the dense array, which is the whole of its design**: that makes
  it a pure function of world position, so the lattice two neighbouring chunks write into has
  no seam to interpolate across and needs no apron, no residency dependency and no re-bake.
  What it pays for that is trees, caves, overhangs and edits, none of which the height field
  carries -- the flood still does. **Unlike the flood it is baked for *every* LOD-0 chunk,
  empty ones included**, because a surface in one chunk taps probes in the next.

Worldgen, biomes and the coarse levels: [`docs/terrain.md`](docs/terrain.md). Streaming and
the LOD planner: [`docs/lod.md`](docs/lod.md).

## Working on this

**One batch per session, and that rule outranks curiosity.**

**A roadmap entry marked RESEARCH FIRST owes the user three prompts before it owes anyone
code, and that is the first thing that session does.** Write them out, hand them over, and
stop there — the user runs them through a deep-research tool and comes back with the answers,
which is a separate session. The marker exists because an appearance batch was ranked on how
wrong it looks in a screenshot rather than on what anybody had read about it, and shipping
into that gap is how a design gets re-derived badly. **Treat every number a research document
returns as an argument about mechanism and never as a measurement** — all seven on file quote
projections, several of them quote *our* real figures back at us to lend the invented ones
credibility, and [`docs/research-review.md`](docs/research-review.md) is the standing record of
that. Price it here.

**The marker served a *performance* batch as well — P1, priced at batch 40 and shipped at 41 — and the suspicion
had to go up rather than down for that, which is the half worth keeping.** An appearance document can be wrong about a mechanism and
still be argued with from a picture; a performance document's headline is always a *projection*,
and [`docs/lessons.md`](docs/lessons.md) records this project's own estimates missing in both
directions by factors of four. **Ask what was measured, on what hardware, against what baseline,
and say in the prompt that projections are not wanted.** The only thing that settles a cost here
is `harness bench`, paired and alternated, against a binary built without the feature in it.

**Two things P1's set proved, and both are about the prompt rather than the answer.** Asking for
formulas as text **does not work** — three sets of documents have now delivered every figure as
an embedded PNG and one of them was told not to, so budget the image extraction instead of
asking. And **a document quoting our own numbers is not automatically the failure mode**: it is
the failure when it multiplies them into an invented total, and it is the most valuable
paragraph in the set when it checks two of them against each other. See
[`docs/research-review.md`](docs/research-review.md).

If a batch finishes and the *numbers* look wrong but the *code* checks out — a delta with no
mechanism, or a pass that moved when nothing touched it
— **do not start investigating.** Finish and document the batch you are on, then add an entry
at the front of the queue in [`docs/roadmap.md`](docs/roadmap.md) saying what the number was,
what was expected, and what has been ruled out. Then stop.

The reason is budget, not discipline for its own sake: a mid-session investigation is
unbounded, it competes with the batch that is already half-written, and it usually ends with
two half-finished things and no measurement worth keeping.

**This does not apply to a broken build, a failing test, a capture that differs when the
control says it must not, or a control that should be free and is not.** Those are the batch,
and you fix them now.

The last one was on the defer list until batch 28 moved it, and the reason it does not belong
there is that it is not one bad number. A control that costs something taxes **every**
measurement taken through it until someone reaches the roadmap entry -- which is the same
argument the cost column of the control table makes. `--no-foliage` is the precedent: **+1.24 to
+2.07 ms**, the largest regression this project has shipped, and it was in a control.

**End a batch with `harness bitexact --exe-b` against a binary built from the parent
commit.** A flag that switches a feature off is **not** a build without the feature in it:
batch 14 measured +0.24 ms through the flag and **+1.85 ms against a real revert**, an
eight-fold difference, which is the whole reason this rule exists.

Build that binary whenever you need it: check out the parent commit in a worktree, build, and
copy `target/release/voxelcraft.exe` to `voxelcraft-preNN.exe`, a name `.gitignore` covers.
**It used to say "before you write any of the batch"**, which was the only option during the
five batches with no `.git` -- forget the copy and the working tree had already moved past the
thing you needed to build. Since batch 27 the parent commit is always reachable, so the
ordering requirement is gone, and so is the way it used to fail.

**Textures are generated procedurally in `textures.rs`. No Minecraft assets are used or may be
added.**

### The hardware round, and what a sandbox session may claim (batch 94, the five asks ruled on)

- **A green line in a sandbox chat is not a compile.** Batch 91 shipped four `rustc` errors
  (two in `src/`, two in its own test files) under the sentence "all suites compile and pass" --
  this sandbox's toolchain gets SIGKILLed before the crates graph links, so the maximum evidence
  class here is *written against the tree, parsed, not compiled*. Say which class a claim is.
- **A hardware round starts by defeating the mtime trap, not by reading numbers.** Synced trees
  arrive with their timestamps flattened (1980); `cargo` then does no work and smoke-tests run
  the *previous* binary. The gate is `find src tests -type f \( -name '*.rs' -o -name '*.wgsl'
  \) -exec touch {} + && touch Cargo.toml` before the first build of any round.
- **One runnable commit at a time** for anything touching WGSL, a binding, a pipeline layout, or
  the block table; a commit's gate is: build (after the `touch`), one screenshot, the 21-vantage
  control against the parent, `cargo test --no-fail-fast`.
- **An arm is built only when it moves pixels** at a named vantage by more than max delta 1.
  Compile-clean and test-green is eligibility, not arrival: P-A measured zero, A9 measured zero,
  and A8/A5's own premise tests said the same of them before this rule was written down. One
  `bitexact` run is the check.
- **A census guard asserts a property, not an integer.** Stale counts went red five times across
  four rounds without meaning anything; guard *shape* (every secondary ray passes `true`) and
  let the arithmetic fall out.
- **A hardware round hears the cumulative story, not the last batch's.** A "docs-only" batch can
  be true of one commit while it hides nine others; flag the tree's state since the last
  hardware round, every round.

## The machine, and what is actually scarce

**One resource is saturated and every other one is idle.** Measured on the target machine (RTX
3050 Laptop, GA107) in the batch-51 session and **re-measured in batch 57**, which is the first
batch to spend any of it. Rows marked *(57)* are the fresh ones; the rest are batch 51's and
still carried. Re-measure before quoting a figure in a design -- but the *ranking* is the point,
and it has never been close.

| resource | capacity | in use | verdict |
|---|---|---|---|
| **GPU shader ALU** | 5.501 TFLOP32 | `resolve` is 44-81% of frame, **instruction-bound at 80 registers since batch 67 and at 96 from batch 51 to 66**. Batch 66 measured what a step is worth -- at `cave`, where the feature draws nothing, **16 registers cost 0.15 ms and 32 cost 0.44**, the same number for two unrelated features -- and batch 67 spent that finding: 96 to 80 bought **+0.103 ms at `cave` and +0.399 at `coastline`**. **The lever was *where* a term is written, not what it does**: `shade_hit` is inlined four times, and a value live across the gather costs more than the arithmetic that needs it. **And batch 69 adds a second lever on the same wall**: `f16` packs two values into one 32-bit register on this driver, measured at **32 live scalars costing 96 registers against 128** -- two steps -- so a live range can be made cheaper without being made shorter. Roadmap P14 | **the wall** |
| bandwidth | 192 GB/s | `build_hiz` alone reads 16.6 MB in 0.110 ms -- **151 GB/s** | contended in bursts |
| VRAM | 4 GB | screen buffers 66.4 MB, pairs 8.4, shaft 3, world data ~50, **probe field 100.7** -- **~229 MB**. The probe field is the one row here that is *derived rather than sampled* and it drifted: 128 x 768 x 128 `Rgba16Float` is exactly 100,663,296 bytes, and batch 58's bounce took three channels the cube was not using. **Every other figure is batch 51's and has not been re-measured since**; the world-data row in particular predates batch 59 widening the light cell, so treat ~50 as a floor | **about 94% free** |
| CPU workers | 16 rayon threads | busy only while streaming; **idle once resident**. Chunk build **7250 us** on the main thread *(67)*, of which **probe bake 3125, worldgen 1896, lighting 1229, tree build 929, intern + upload 71**. The bake is **43% of chunk build and the largest single line in it**, which is the trade this table exists to make: batch 57 put it there and the frame paid 0.02 to 0.04 ms for the read | free |
| host RAM | system | **6.8 MB per 96 chunks** *(67, re-measured and unchanged since 59)* -- geometry **1.3** at 0.130 bytes/solid voxel and attributes + light **5.5** at 0.541, over 10.6M solid voxels. Plus **24 KB per chunk of probes (57)**. Dedup is **1.00x** on both run tables, which is expected and documented in `PERF.md` -- **do not "fix" it** | free |
| storage | -- | append-only journal | free |
| tensor cores | 64 (22 TFLOP16, 44 TOPS INT8) | **nothing** | untouched, and **reachable** -- `EXPERIMENTAL_COOPERATIVE_MATRIX` **true**, re-confirmed by `probe` at batch 67. **And naga's WGSL front end does express them, asked at batch 69 and no longer open** -- `enable wgpu_cooperative_matrix;`, `coop_mat16x16<f16, A>`, `coopLoad` / `coopMultiplyAdd` / `coopStore`, a 16x16 `f16` multiply into an `f32` accumulator, compiled on this device. **That is reachability and not a use**: every candidate anybody has named -- a learned denoiser, a neural radiance cache -- runs *per frame*, which aims at the one resource that is full, so this row stays "nothing" for a reason that is now about value rather than about the toolchain |
| RT cores | 16 | **nothing** | untouched and **reachable end to end, measured at batch 69 rather than asserted** -- the ray query shader compiles (`enable wgpu_ray_query;`, `acceleration_structure`, `RayDesc`, `rayQueryInitialize` / `Proceed` / `GetCommittedIntersection`) *and* wgpu 30 built a real BLAS from **AABBs** with a TLAS instance over it and submitted it on this driver. **AABBs are the whole point**: there is no mesher here, so procedural primitives are the only shape an RT core could ever accelerate for this engine, and that path is open. **R2's first rung shipped at batch 58 without them, and that weakens the case rather than making it** -- the bounce that mattered was off the height field, which is a heightfield step and not a ray. What still wants them is bouncing off canopy, cave wall and overhang, which is R3's and R4's geometry |

**Re-measured on 2026-09-15 at batch 67, and three rows moved.** Chunk build is **7250 us**, not
7947, of which the probe bake is **3125** rather than 3414 -- both fell without anyone touching
them, which is this file's own power-state caveat and the reason the *ranking* is what this table
is for and not the absolute figures. Host RAM re-measured to **6.8 MB per 96 chunks**, unchanged
since batch 59. Frame cost at 1920x1080 with hi-z and TAA on: **9.19 ms of GPU**, `resolve`
**5.85** of it -- **64%** -- against `march`'s 2.37, `taa` 0.50, `select` 0.16, `shaft` 0.11,
`hiz` 0.10. **Every capability row was re-probed and every one is unchanged.** The VRAM row is the
stale one and says so in the table.

**And batch 67 adds a second question beside the one below, which had been the only one since
batch 51.** Asking "can this be moved off frame ALU" ranks *work*. It cannot rank *code*, and
batches 65 to 67 spent three sessions on a cost that was neither: a term whose arithmetic was
nearly free cost **0.3 to 0.5 ms** by holding two vectors live across the gather, and moving where
it was written -- not what it did -- gave that back. **So ask "where does this value die" as well
as "which resource pays".** `resolve` is 80 registers and the next 16 are a whole occupancy step,
so the question has teeth: a rung that adds one more live `vec3` across the gather can cost a step,
and a rung that adds real arithmetic where nothing else is live can cost nothing.
**And there is a third form of the same question since batch 69, which is a type rather than a
place: what is this value *while it waits*.** `f16` packs two to a register and was measured at
two occupancy steps -- but only for storage across a live range, because a transcendental widens
back to `f32` and takes everything feeding it along. So the three questions are which resource
pays, where the value dies, and what it is in the meantime.

**So the question is never "can this machine afford X" -- it is "can X be moved off frame ALU."**
Worldgen already is: **7.5 ms per chunk on rayon against 84 us on the main thread**, and the
lighting flood is **1229 us** of chunk build *(67; it read 2312 when this line was written, and
nothing in between aimed at it)*. **Batch 57 put a second bake beside it** -- the ambient
cube's 1927 us, 31% of a 6141 us chunk build on the main thread -- **and the frame paid 0.02 to
0.04 ms for the read.** **Batch 58 widened that same bake rather than adding a third** -- the
ground bounce is +681 us of it, 1.36x, and the frame paid **nothing measurable**, because the
bounce and the cube are four channels of one texel and therefore one tap. That trade, in those
proportions, is what this table is for.

**Batch 60 is the cleanest reading of that trade the sequence has, because it moved both sides
at once and neither side was ambiguous.** Roadmap R3's first rung weights the bounce by the sky
the *gathered ground* can see, and it takes **zero new height samples** to do it -- the exposure
is a maximum of slopes over the `STEPS` column heights the horizon march had already taken and
was throwing away, so the chunk's 81,920 samples stay 81,920. The bake paid **+671 us, 1.24x**;
the frame paid **nothing measurable at three vantages against a real revert binary**, `sky`
included, which is the row that prices compiled code rather than executed work. **And the
batch's own negative result is the more useful half**: the term it improves modulates the
ambient floor, `Config::ambient` is 0.08, and no amount of transport accuracy beats a 7%
ceiling -- so ask what *share of the frame's light* a rung can reach before asking which
resource pays for it. That question would have ranked this rung before it was built.

**Batch 59 is the rung that could not take it, and it belongs on this table as the exception
rather than as a failure.** Roadmap R4 gave block light three channels, and there was nowhere
off-frame to put them: what varies is not the value at a probe but the value at every *cell*, and
the flood is already the cheapest possible store of that. The chunk build did not move -- the
flood reads **1348 us against 1402**, because three arrays allocated only when a chunk has an
emitter beat one allocated always -- and **the frame paid +0.321 ms of `resolve` at `cave`,
+0.340 at `default`, +0.150 at `sky`**, the largest bill the main sequence has run up. **So the
sequence has two kinds of rung and this table had named only one**: a rung that replaces a
constant with a *field* reads one tap and can be free or better, and a rung that replaces a
constant with *content* has to widen a per-cell store, where every reader pays. Ask which kind a
rung is before asking which resource pays for it. **Work added to chunk build is paid out of a resource nothing
is using; work added to `resolve` is paid out of the only one that is full.** That asymmetry is
why an appearance idea should be priced at its *stage* before it is priced in milliseconds.

**Batch 61 is the sequence's first rung rejected on *effect* rather than on cost, and it adds a second question to that one.** Roadmap R5 aimed at the direct term -- the biggest term in the frame, exactly where batch 60 said the remaining energy was -- and it was built, measured at **+2.639 ms of a 6.165 ms `resolve`** and reverted, because it moved **471 pixels of 921,600**. A soft shadow only ever reaches the pixels at a shadow's *edge*, and in a world of 1-block voxels two rays across the sun's disc do not separate by a whole voxel until the occluder is **108 blocks** away, so below that the shadow is exactly hard rather than approximately hard. **Share of the frame's light and share of the frame's pixels are different questions and a rung has to pass both** -- see [`ADR 0002`](docs/adr/0002-no-disc-sampled-sun.md).

**And the stage is the first of four instruments, each cheaper than a bench and each of which
settled something a bench could not.** Reach for them in this order before reaching for
`harness bench`:

1. **The stage.** Ask which resource pays before asking how many milliseconds. The table above
   is the whole argument.
2. **A crop.** Batch 52: a constant nothing in the fixture could see became rankable in one
   command by building the camera that could see it. **A blind vantage is not a conservative
   one** -- it cannot argue in either direction, and the crop bought the right to decide rather
   than caution.
3. **A count.** `harness march` counts DDA steps instead of timing them, so a traversal change
   ranks on a hot or busy machine and **a bit-exact one still reads positive**. Batch 53 ranked
   three candidates in four minutes with a *deliberately incorrect* build: patch one literal to
   a wrong value, count, throw the picture away.
4. **Playing it.** Batch 54 is the first defect this project found that way, and no metric on
   file had it. Then the expensive fix was measured and discarded -- a traced mirror cost 4 ms
   and differed from the free one by a max channel delta of 3 to 7.

**What they have in common is that each answers a question a timing could not have answered**,
and three of the four cost minutes rather than a session.

**Five facts this file got wrong about its own machine, recorded so they are not re-derived. The third and fourth are the instructive ones, because it was this file's *own* prose that was wrong rather than a document's -- and the fourth happened after the third was written down as a warning:**

- **The card is 5.501 TFLOP32, not 4.** Three research documents quote 4 because the prompts said
  so and none of them checked -- `research-review.md`'s failure mode, live and small.
- **wgpu 30 does expose ray tracing.** `EXPERIMENTAL_RAY_QUERY` on Vulkan **and** DX12 (Tier 1.1 +
  SM 6.5), and `BlasGeometrySizeDescriptors::AABBs` carries custom primitives, so **no mesher is
  required**. Unmeasured here, and the honest prior is low: an RT core accelerates a *broad
  phase*, and `tile_select` already does that for 0.13-0.17 ms. The case that is not ruled out is
  incoherent secondary rays, which have no tile coherence to amortise. **`--bin probe` tests for it
  since batch 55 and the adapter says `EXPERIMENTAL_RAY_QUERY` is true**, so the claim is measured
  rather than asserted -- it had stood unchallenged for four batches because nothing asked.
  **The strongest case for it is the one this file did not make**: an RT core is the right tool for
  a *bake*, not for the frame. `tile_select` already broad-phases the frame's coherent rays for
  0.13-0.17 ms, where a probe bake casts millions of incoherent rays into **static** geometry and
  amortises the acceleration structure over a chunk's whole residency -- and it is paid out of the
  idle resource rather than the full one. See roadmap L1.
- **The tensor cores are reachable, and this session is what got that wrong.** Designing L1 it argued
  they were probably shut because wgpu likely did not expose cooperative matrix, and the user asked
  for the one `probe` line instead of the belief. **`EXPERIMENTAL_COOPERATIVE_MATRIX` is true** on
  this adapter, as are `EXPERIMENTAL_RAY_HIT_VERTEX_RETURN` and `EXPERIMENTAL_RAY_TRACING_PIPELINES`.
  What is confirmed is that the *adapter* reports them; whether naga's WGSL front end can express the
  operations is a further question nobody has asked. **The general form is the point and it is the
  same failure as the row above it**: a belief about a capability is one line of `probe` away from
  being a fact, and this file has now produced that failure twice.
- **A binding array of textures is available, and the 3D texture limit is 16384 -- both from the
  batch-57 `probe` run, and this is the third time.** `TEXTURE_BINDING_ARRAY` reports **true**.
  Roadmap R1's storage section had asserted *"there is no bindless here, so a texture per chunk is
  not available"* and used it to rule out an alternative; that text left the file when R1 shipped,
  so nothing stale survives, but **the belief shaped a shipped design and was never checked**.
  `max_texture_dimension_3d` is **16384** against the 2048 the same session assumed, so the probe
  lattice -- 768 in its largest axis -- has twenty-one times the headroom in every dimension before any
  *limit* binds, and the only constraint on its resolution or its toroidal span is memory, which is
  96.5% free. **What makes this the instructive one is the timing**: the bullet above it was
  written as a standing warning that a capability belief is one `probe` line from being a fact,
  and the very next batch produced the same failure twice more in one session. The lesson is not
  to be more careful -- it is that **a design note asserting what the hardware cannot do should be
  written as a `probe` row or not written at all.**

  *(The design it produced was right anyway, and for an unrelated reason: one world-space lattice
  is seamless by construction where a per-chunk slot atlas needs an apron. Being wrong about the
  premise and right about the conclusion is not a defence of the premise.)*

- **`SHADER_F16` and `SUBGROUP` are both available, both lower to real SPIR-V, and this table
  had never listed either.** Measured 2026-09-15; `probe` reports them and compiles a shader for
  each rather than printing a bit. Subgroup size is **fixed at 32** and
  `max_compute_workgroup_storage_size` is **49152**. **`f16` is the row that matters**, because it
  is the only idle capability on this machine that reaches the register wall rather than routing
  around it: measured at a real live range, **32 live scalars cost 16 registers as `f16` against
  32 as `f32`** -- two occupancy steps, which batch 66 priced at 0.44 ms. **This is the fifth
  entry in this list and the first that is an omission rather than an error**: nothing here ever
  asserted `f16` was unavailable, which is worse, because a belief that is never written down
  cannot be checked. The ALU row above says the next 16 registers are a whole occupancy step and
  said nothing about the one instruction-set feature that halves what a live value costs. See
  roadmap P14 for the sweep, the two arms that measured nothing, and what is still open.

## The environment

**This tree is under git as of the batch-27 documentation pass.** Before that it had no `.git`
for five batches, which is why several "reproduces the pre-batch-N build" claims in the ledger
are historical assertions rather than live measurements — the revert binaries for batches 1
through 20 do not exist and cannot be rebuilt.

**`.gitattributes` is `* -text` and has to stay.** Git for Windows sets `core.autocrlf=true` at
*system* level on this machine, which would rewrite most of `src/` on the next checkout;
`-text` plus a local `core.autocrlf=false` turns that off. The tree's line endings are
genuinely mixed — **6 CRLF files against 76 LF ones**, listed in
[`docs/pitfalls.md`](docs/pitfalls.md), which also carries every other trap this environment
produces: the heredocs that fail, the `tail` that eats a cargo run, the backslash mangling, the
`newline=''` every Python write needs, and the `grep` that reports the whole tree as CRLF.
**Open it before you start editing, not after.**


================================================================================
