
# Measured performance

**Everything here was measured on this machine, and the frame numbers in the first two tables
were re-measured in one sitting so they are comparable with each other.** Nothing else is.

Three caveats that have each cost a session:

- **Absolute milliseconds drift by up to 1.8x with the laptop's power state.** A number from one
  session cannot be compared with a number from another. Only pairs benched back to back mean
  anything.
- **A per-feature cost is meaningless without a vantage.** Water is bounded below at exactly zero
  and above by how much sea is on screen.
- **How to bench so the result is real** is [`docs/harness.md`](docs/harness.md), and it is not
  optional -- the obvious arrangement manufactures +0.099 ms out of a null.

GPU: **NVIDIA GeForce RTX 3050 Laptop**. Per-pass times come from `--bench-frames`.

---

## Where the frame and the chunk stand (re-measured 2026-09-15, batch 67)

**1920x1080, hi-z on, TAA on, 128 chunks** -- `--bench-frames 140`:

| pass | ms | | pass | ms |
|---|---|---|---|---|
| `resolve` | **5.85** | | `taa` | 0.50 |
| `march` | 2.37 | | `select` | 0.16 |
| `march2` (recover) | 0.07 | | `shaft` | 0.11 |
| `build_hiz` | 0.10 | | **GPU total** | **9.19** |

`resolve` is **64% of the frame**. Wall median 10.01 ms (100 fps), p99 17.30. With hi-z **off** the
same frame is 11.77 ms of GPU and `march` alone is 5.17 -- **hi-z takes 2.80 ms out of `march` for
0.10 + 0.07**, which is the clearest single statement of what that pass is worth. Pairs marched
92,924 with hi-z against 266,205 without, 17% of the buffer either way.

**Chunk build, `--bench-terrain 96`** -- 96 chunks, 10.6M solid voxels:

| stage | us/chunk | share |
|---|---|---|
| **probe bake** | **3125** | **43%** |
| worldgen | 1896 | 26% |
| lighting flood | 1229 | 17% |
| tree build | 929 | 13% |
| intern + upload | 71 | 1% |
| **total** | **7250** | |

Memory: geometry **1.3 MB** (0.130 bytes per solid voxel), attributes + light **5.5 MB** (0.541),
**6.8 MB** over 96 chunks. Dedup **1.00x** on both run tables -- expected on procedural terrain,
and **not a defect to fix**.

**Two rows moved since they were last written and nothing aimed at either.** Chunk build was 7947
us and the probe bake 3414 at batch 60; the flood read 2312 when `CLAUDE.md`'s worldgen paragraph
was written. Both fell. That is the power-state caveat at the top of this file doing exactly what
it warns about, and it is why the *ranking* of these rows is what they are for.

---

## Both register ceilings down: `resolve` is 80 (batch 67)

Roadmap P8 closed, open since batch 38e. **96 registers to 80**, 595,712 bytes to 396,544.
Against `voxelcraft-pre67.exe`, a real revert binary, eight paired rounds:

| vantage | `resolve` | 1 se | t | has a pane in it? |
|---|---|---|---|---|
| `coastline` | **+0.399 ms faster** | 0.080 | +4.96 | no |
| `cave` | **+0.103 ms faster** | 0.024 | +4.30 | no |
| `default` | -0.029, indistinguishable | 0.037 | -0.78 | no |
| `glass` | -0.052, indistinguishable | 0.042 | -1.25 | **yes** |

**The two vantages that gain have no glass in them and the one with a glasshouse gains nothing.**
That is occupancy behaving exactly as batch 66 said it does, and it is why this was never
findable by profiling the feature.

### Both ceilings had to fall in one batch, and the order was forced

The count is a `max()`, so glass and batch 65's specular each held 96 **independently**. Fixing
either alone leaves the shipping build at 96 and pays **nothing**. Specular went first because it
was the one that collapsed glass's dual ceiling to a single one: the hoist lowered `shade_hit`'s
demand, `shade_hit` is one of `shade_glass`'s two arms, and stubbing the other arm then read 80
where before it read 96.

### What moved the specular was position, not arithmetic

Every stub of its three operations read 80 on its own -- dropping `sky_base`, calling it on `n`
instead of the reflected direction, replacing Schlick with a constant -- so it was none of them.
Folding the weight into one scalar changed nothing, because the compiler already schedules that.
Written beside the return, `n` and `rd` stay live across the nine-cell gather, the probe tap and
the shadow march. Hoisted, a single `vec3` crosses instead. **Ask where a value dies before
asking what an expression costs.**

### And the allocator works in blocks

Hoisting glass's own reflection above its dispatch traded one live set for another, about five
registers, and read **96 unchanged**. Shedding five does not cross a sixteen-wide step: a register
fix is worth a whole step or it is worth nothing.

---

## What `resolve` is bound by, measured at last (batch 66)

Roadmap P13 closed and P8 answered. **The test is `cave`, where glass draws nothing and batch
65's specular changes zero pixels**, so both features are pure dead weight and any cost is the
compiled code alone. All four register counts come from one binary -- both are specialization
bits, so a flag selects a different pipeline. `resolve`, hi-z on, eight paired rounds:

| build | registers | vs the 80-register build | 1 se | t |
|---|---|---|---|---|
| neither | **80** | -- | -- | -- |
| glass only | **96** | **+0.156 ms** | 0.005 | +29.4 |
| specular only | **96** | **+0.151 ms** | 0.018 | +8.3 |
| both | **128** | **+0.441 ms** | 0.014 | +31.1 |

**The same number twice, from two unrelated features.** 0.156 against 0.151 is a difference of
0.005 against an se of 0.018. A transmissive material with a traced reflection and a Fresnel sky
lobe are indistinguishable when neither draws anything, which is what says the cost is the
**register count** and not the feature.

**32 registers costs 1.41x two lots of 16** -- 0.441 against 0.312 -- so an occupancy step lies
between 96 and 128. **Do not price a register budget by interpolation.**

**Size is ruled out, now three times.** Batch 42 removed 271,872 bytes of glass and the frame got
*slower*. Batch 65's term made the module smaller while costing 0.3 to 0.5 ms. Batch 66 stubbed
glass's `shade_water` dispatch, removing **200,000 bytes**, and the register count did not move.

### And the fix, which is about inlining rather than about the line

`shade_hit` is inlined **four** times -- the primary surface, water's refracted and reflected
legs, glass's transmitted leg -- so a term written in its body exists in four copies and the
allocator budgets for all of them. Passing a literal `false` for `do_spec` at the three secondary
sites folds the arm away there:

| | registers | `cave` | `default` | `sky` |
|---|---|---|---|---|
| batch 65 as shipped | 128 | +0.295 ms | +0.516 | +0.271 |
| batch 66 | **96** | **+0.051** | **+0.065** (indistinguishable) | **+0.044** |

**83-87% recovered**, for 1,664 bytes of extra code. **Ask how many times a function is inlined
before asking what a line in it costs.**

---

## The specular term, and the 96-register ceiling it broke (batch 65)

Roadmap R6. `resolve`, hi-z on, paired against `voxelcraft-pre65.exe`, eight rounds, alternated:

| vantage | `resolve` cost | 1 se | t | pixels moved |
|---|---|---|---|---|
| `cave` | **+0.295 ms** | 0.011 | -28.04 | **0** |
| `default` | **+0.516 ms** | 0.049 | -10.63 | 760,651 |
| `sky` | **+0.271 ms** | 0.019 | -14.62 | 249,100 |

**This is the largest bill the main sequence ran up**, past batch 59's +0.321 / +0.340 / +0.150,
and unlike that one it is not the work. **`cave` pays 0.295 ms to render zero changed pixels.**

`shaderstats`, three builds in one sitting, is the whole explanation:

| build | registers | bytes | shared |
|---|---|---|---|
| term removed | **96** | 592,512 | 14,848 |
| `sky_base` replaced by a constant | **128** | 588,544 | 2,560 |
| shipping | **128** | 592,256 | 4,608 |

**The code got smaller and the register count rose by a third.** 96 is what this file and
`CLAUDE.md` have said `resolve` is instruction-bound at since batch 51, and every batch through
63 measured it; this is the first to move it. The middle row rules out the sky lookup as the
cause. Roadmap P13 carries what has not been tried -- chiefly that `shade_hit` is inlined **four**
times, so the term's live values exist in four copies where only the primary surface plausibly
needs a view-dependent sheen.

**A `sky_sm > 0.0` early-out was built and rejected**, which is roadmap P11's finding a second
time: it saved nothing at `cave` (0.295 to 0.282, inside its own error) and made `default` and
`sky` worse -- 0.516 to 0.594 and 0.271 to 0.305. Ask what a gate costs before crediting it with
what it skips.

---

## Coloured block light, which is the one rung of the main sequence that costs (batch 59)

Roadmap R4. `resolve`, hi-z on, paired against `voxelcraft-pre59.exe`, eight rounds, alternated:

| vantage | shipping vs parent | 1 se | t |
|---|---|---|---|
| `cave` | **+0.321 ms** | 0.032 | -10.15 |
| `default` | **+0.340** | 0.049 | -6.99 |
| `sky` | **+0.150** | 0.000 | -inf |

**The largest frame cost any rung of the main sequence has carried, and it is the rung that
could not be moved off frame.** R1 and R2 both put their work in the bake and paid 0.02-0.04 ms
and nothing for the read. What colour the light at a cell is has to be stored and read per pixel,
and there is nowhere else to put it.

**Two numbers split it, and only one of them has a mechanism.**

| | `cave`, `resolve` |
|---|---|
| the widened cell alone (`--no-light-rgb` vs the parent) | **+0.110** |
| the coloured read (shipping vs `--no-light-rgb`, **one binary**) | **+0.202** +/- 0.014, t = -14.48 |
| shipping vs the parent | **+0.321** |

The first is the storage: the cell is `sky:4 | r:4 | g:4 | b:4` and a brick is 32 words instead
of 16, which both arms read because no pipeline override can shrink a brick. **That is why
`--no-light-rgb` is the first control in this project whose picture reverts exactly and whose
time cannot.**

**The second has no mechanism and is at the front of the roadmap queue.** At `cave` every block
channel is zero, the neighbourhood gate below skips all twenty-seven `light_curve` evaluations,
and the shipping arm therefore executes *strictly fewer* instructions than the arm it is losing
to. `resolve` is **586,624 bytes against the control's 588,928**, **96 registers** and **15,872
bytes of shared memory** either way -- so it is not size, not occupancy and not registers.

**And the spread says less than a first draft of this section claimed.** It read that 0.321 /
0.340 / 0.150 was *batch 56's own ordering by surface density*, and therefore that the cost
was work rather than code. It is not that ordering: `cave` at +/- 0.032 and `default` at +/-
0.049 are indistinguishable from each other, where batch 56's saving had `cave` clearly ahead.
What the readings support is only this: two surface-dense frames cost about the same and a
frame with almost no shaded surfaces costs about half, so the term is not *purely* code size
-- which is weaker than the inference that was drawn from it, and is the whole of it.

**`cave` is also a tiny unlit pocket, and the user said so before this paragraph did.** That
is exactly why it is the right frame for a `resolve` timing -- every pixel is a shaded hit, so
per-surface cost is maximised, which is why batch 47 measured the gather at 72% of the pass
there. It is also why it is worth nothing as a *look* vantage: the capture is black, and no
reading taken there says anything about whether a shading change was worth its milliseconds.

### Two builds were measured and thrown away on the way here, and both are findings

**The ungated build cost +0.835 ms at `cave`** -- 31% of a 2.655 ms pass, at a vantage with no
emitter in it, where all twenty-seven evaluations return zero. The gate is an OR of the nine
gathered cells' block nibbles and is **exact rather than an approximation**: with all nine zero
the block term reduces to `SKY_TINT * sk` to the bit. It is tested on the cells rather than on
the chunk because the gather crosses chunk boundaries and the flood does not -- a chunk-level bit
would drop a lamp standing just across the seam. Batch 51's rejected chunk-flag gate is the same
lesson from the other side: exact, and it never fired.

**Caching the nine gathered curves in an `array<vec3<f32>, 9>` cost 0.147 ms**, and
`shaderstats` says why in one row: `resolve`'s shared memory goes **15,872 to 16,896 bytes**,
which is what decides how many workgroups an SM holds at once. Evaluating the twelve corner
curves inline instead -- **thirteen evaluations per pixel rather than nine** -- puts shared
memory back where it was and is faster. **On this pass occupancy is worth more than four
`light_curve` calls**, which is batch 47's finding from the other end: perfect sharing of the
nine gather reads recovered a tenth of what removing them did.

## Batch 60: the second bounce is paid entirely in the bake

**Roadmap R3's first rung, and the cleanest example so far of this project's own scarcity
rule.** The frame pays nothing measurable and the bake pays 1.24x, which is the trade
`CLAUDE.md`'s resource table exists to make obvious.

`harness bench --vantage V --exe-b voxelcraft-pre60.exe`, 8 paired alternated rounds, `resolve`
with hi-z on. Delta is `B - A` with A the shipping build, so a **positive** delta is what the
feature costs:

| vantage | A (batch 60) | B (pre-60) | paired delta | 1 se | t | verdict |
|---|---|---|---|---|---|---|
| `default` | 6.102 | 6.104 | +0.001 | 0.027 | +0.05 | indistinguishable from zero |
| `cave` | 2.854 | 2.836 | -0.018 | 0.014 | -1.25 | indistinguishable from zero |
| `sky` | 3.225 | 3.250 | +0.025 | 0.015 | +1.67 | indistinguishable from zero |

**All three are inside their own error, so what is claimed is below about 3 se** -- 0.081 ms at
`default`, 0.042 at `cave`, 0.045 at `sky`. `sky` is the row that matters for P8's reason: it
has almost no shaded surface, so it prices *compiled code* rather than executed work, and batch
56 moved it +0.030 on a pure code-size change. This batch does not move it.

`--bench-terrain 96`, main thread, per chunk, one sitting, the flag as the only difference:

| stage | `--no-probe-shadow` | batch 60 |
|---|---|---|
| worldgen | 2029.7 | 2076.1 |
| tree build | 985.1 | 995.0 |
| lighting flood | 1358.4 | 1383.3 |
| **probe bake** | **2743.1** | **3413.8** |
| total | 7195.5 | 7947.2 |

**+671 us and 1.24x on the bake, for zero new height samples.** That pairing is the whole
design: `ray_exposure` reads the `STEPS` column heights the horizon march has already taken and
computes a maximum of slopes over them, so the chunk's **20,480 height samples stay 20,480** and
what is added is arithmetic over an array already in cache. Every other row moves by about 2%,
which is run-to-run noise on this fixture and not attributable to the batch.

**The comparison worth drawing is with batch 59 two rows down.** That rung replaced a constant
with *content*, had nowhere off-frame to put it, and cost the frame +0.321 ms at `cave`. This one
replaces a constant with a *field*, reads through a tap that already existed, and costs the frame
nothing. `CLAUDE.md` names those as the sequence's two kinds of rung; batch 60 is the first to be
measured cleanly as the cheap kind on both sides at once.

### The chunk build did not pay for it, and slightly gained

`--bench-terrain 96`, main thread, per chunk, both builds in one sitting, two rounds each:

| stage | pre-59 | batch 59 |
|---|---|---|
| worldgen | 2071.1 / 2046.5 | 2083.3 / 2027.7 |
| tree build | 1039.9 / 978.0 | 1041.9 / 994.1 |
| **lighting flood** | **1402.3 / 1383.6** | **1348.3 / 1354.6** |
| probe bake | 2808.8 / 2759.2 | 2762.2 / 2742.3 |
| total | 7386.6 / 7233.8 | 7314.7 / 7188.8 |

**Three floods are not three times one flood, and the reason is that almost no chunk has an
emitter.** `light::compute` allocates the three block arrays only when one does, where the pre-59
build zeroed 256 KB unconditionally -- so in a world with no lamp in it the widening *removes*
an allocation and pays for a two-byte pack. -40 us, consistent in sign across both rounds, and
small enough that the honest claim is "no change" rather than a saving.

**Memory is where the widening shows**: `attributes + light` **4.2 MB to 5.5 MB** over 96 chunks,
**0.416 to 0.541 bytes per solid voxel**. A light brick is 32 words instead of 16.

## The ambient cube's bake, which is not a frame cost at all (batch 57)

`--bench-terrain 96`, main thread, per chunk:

| stage | us |
|---|---|
| worldgen | 1959.0 |
| tree build | 957.7 |
| lighting flood | 1344.5 |
| **probe bake** | **1912.4** |
| intern + upload | 60.1 |
| **total** | **6233.8** |

**A second run an hour later reproduced it**: 1927.1 us of bake in a 6141.4 us build, against
1912.4 in 6233.8. **Worker microseconds reproduce where frame milliseconds do not** -- the same
session's two `cave` bench readings disagreed by a factor of two on the same paired A/B. That is
worth knowing when choosing what to price a bake in.

**A 44% increase in chunk build, on the resource that is idle.** The scarcity table at the top
of `CLAUDE.md` is the whole argument: sixteen rayon threads do nothing once the world is
resident, and `resolve` is the only thing that is full. At the 160 blocks LOD 0 reaches, the
startup bill is roughly 200 chunks x 1.9 ms over sixteen threads -- under a tenth of a second,
once.

**The lever, if it ever needs one, is azimuths and not probes.** The bake is 64 probe columns x
16 azimuths x 20 geometric steps = 20,480 `WorldGen::height` evaluations per chunk, and that
call is essentially the whole of the 1912 us; the per-probe integration is arithmetic on
tangents already in hand. Cutting azimuths is linear and the basis has only four horizontal
lobes to resolve.

**VRAM: 12 MB**, one 64 x 384 x 64 `Rgba16Float` 3D texture, against the ~130 MB the rest of
the engine uses and the 4 GB the card has. 24 KB of it per resident chunk.

## The ground bounce rides that bake rather than adding one (batch 58)

`--bench-terrain 96`, main thread, per chunk, both sides in one sitting:

| stage | batch 57 | batch 58 |
|---|---|---|
| worldgen | 1913.5 | 1888.4 |
| tree build | 927.9 | 918.0 |
| lighting flood | 1296.8 | 1289.5 |
| **probe bake** | **1867.8** | **2548.9** |
| intern + upload | 57.2 | 58.4 |
| **total** | **6063.1** | **6703.3** |

**+681 us, 1.36x the bake and +10.6% of the chunk build** -- against a roadmap entry that said
to expect "several times that, because a DDA through voxels is not a heightfield step". It is
not several times because it is not a DDA: the gather rides the same 20,480 samples the horizon
march already takes and adds one `column_biome` lookup on every *other* radius plus a form
factor. **The lever named in the section above is still the lever** -- azimuths, linear -- and
`probe::ALBEDO_STRIDE` is a second one that touches only the bounce.

**Read side: nothing measurable, and the mechanism is why that is believable.**

| | `resolve` | 1 se | t |
|---|---|---|---|
| `cave` | **-0.001** | 0.005 | -0.24 |
| `default` | **+0.008** | 0.032 | +0.23 |

Paired against `voxelcraft-pre58.exe` -- a real revert and not a flag -- eight rounds each,
alternated. **Quote the detection threshold and not the word "free"**: about 3 se is 0.015 ms at
`cave` and 0.096 at `default`, so what is claimed is *below those*. What raises it above "we
could not resolve it" is that the bounce and batch 57's occlusion are **four channels of one
`textureSampleLevel`** -- `resolve` 584,832 -> **584,960 bytes, +128, at 96 registers either
way**. Batch 55 measured three taps at the price of one and concluded the cost was the code
existing rather than the work; this is the first batch to *spend* that finding, and it spent it
by widening what a tap carries instead of taking another.

**VRAM is unchanged at 12 MB.** The field was `Rgba16Float` from batch 55 and batch 57 used one
channel of it; this batch uses the other three. There is no fifth channel, which is the next
storage question a rung here will ask.


## The frame today: sixteen of the eighteen vantages, 1920x1080, TAA on

**Re-measured after batch 47.** The first version of this table was taken at batch 44 and
went stale two batches later: batch 45 took 0.5 to 1.7 ms out of `resolve`, which is the
column this table exists for. One sitting, `--bench-frames 140` at every vantage, hi-z on,
so every row is comparable with every other row and with nothing else in this file. Sorted
by GPU total, which is not the order they were run in. Each vantage's arguments were read
out of the fixture rather than retyped. **The wall column is load-bearing**: `tests/docs.rs`
scans this file for frame rates to check the README quotes none, and a rewrite that drops
every `N fps` fails that test with *the scan is broken*. Batch 44 dropped them and the pass
after 44-47 dropped them again -- per row, they cannot vanish with one table.

**Check that no `voxelcraft.exe` is running before re-taking this.** The first attempt read
`resolve` **17.83 ms at `default` against a true 5.68** because a game window was open and
competing for the GPU -- internally consistent, three times too large, and nothing in the
output says so. [`docs/pitfalls.md`](docs/pitfalls.md).

| vantage | chunks | `tile_select` | `march` | `build_hiz` | `recover` | `march` (rec) | `resolve` | `taa` | `shaft_scan` | GPU total | wall median | `resolve` share |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `terraces` | 126 | 0.15 | 1.85 | 0.11 | 0.03 | 0.05 | **8.43** | 0.50 | 0.11 | 11.23 | 12.00 ms (83 fps) | 75% |
| `coastline` | 106 | 0.13 | 1.17 | 0.10 | 0.02 | 0.05 | **8.80** | 0.48 | 0.12 | 10.88 | 11.70 ms (85 fps) | 81% |
| `overcast` | 128 | 0.17 | 2.60 | 0.11 | 0.02 | 0.08 | **6.64** | 0.52 | 0.11 | 10.25 | 11.09 ms (90 fps) | 65% |
| `lattice` | 139 | 0.15 | 2.45 | 0.10 | 0.02 | 0.04 | **6.78** | 0.49 | 0.11 | 10.15 | 10.92 ms (92 fps) | 67% |
| `open-sea` | 104 | 0.13 | 1.23 | 0.10 | 0.02 | 0.05 | **7.61** | 0.48 | 0.12 | 9.75 | 10.48 ms (95 fps) | 78% |
| `edits` | 134 | 0.17 | 2.56 | 0.11 | 0.03 | 0.07 | **6.10** | 0.53 | 0.12 | 9.68 | 10.62 ms (94 fps) | 63% |
| `glass` | 134 | 0.17 | 2.54 | 0.11 | 0.03 | 0.07 | **6.06** | 0.53 | 0.12 | 9.62 | 10.30 ms (97 fps) | 63% |
| `shore` | 108 | 0.13 | 1.40 | 0.10 | 0.02 | 0.06 | **7.24** | 0.48 | 0.11 | 9.54 | 10.36 ms (97 fps) | 76% |
| `default` | 128 | 0.16 | 2.51 | 0.11 | 0.02 | 0.07 | **5.68** | 0.50 | 0.11 | 9.18 | 10.06 ms (99 fps) | 62% |
| `underwater` | 128 | 0.16 | 3.80 | 0.11 | 0.02 | 0.11 | **3.86** | 0.51 | 0.13 | 8.69 | 9.41 ms (106 fps) | 44% |
| `lod` | 88 | 0.12 | 2.21 | 0.11 | 0.01 | 0.05 | **5.49** | 0.49 | 0.12 | 8.60 | 9.39 ms (106 fps) | 64% |
| `canopy` | 143 | 0.16 | 3.30 | 0.11 | 0.02 | 0.04 | **3.50** | 0.52 | 0.12 | 7.77 | 8.40 ms (119 fps) | 45% |
| `low-sun` | 120 | 0.16 | 2.44 | 0.10 | 0.02 | 0.06 | **4.24** | 0.50 | 0.12 | 7.66 | 8.42 ms (119 fps) | 55% |
| `night` | 128 | 0.17 | 2.69 | 0.11 | 0.02 | 0.08 | **3.65** | 0.53 | 0.00 | 7.26 | 7.84 ms (128 fps) | 50% |
| `sky` | 88 | 0.13 | 1.67 | 0.09 | 0.01 | 0.07 | **2.87** | 0.48 | 0.12 | 5.45 | 6.09 ms (164 fps) | 53% |
| `cave` | 134 | 0.16 | 0.43 | 0.10 | 0.03 | 0.01 | **2.35** | 0.26 | 0.11 | 3.46 | 4.11 ms (243 fps) | 68% |

| pass | ms over the set | share |
|---|---|---|
| `resolve` | 89.30 | 64.2% |
| `march` | 34.85 | 25.0% |
| `taa` | 7.80 | 5.6% |
| `tile_select` | 2.42 | 1.7% |
| `shaft_scan` | 1.75 | 1.3% |
| `build_hiz` | 1.68 | 1.2% |
| `march` (recover) | 0.96 | 0.7% |
| `recover_select` | 0.34 | 0.2% |
| **the sixteen frames together** | **139.17** | |

**The `underwater` row predates batch 48 and is now stale by -1.125 ms** -- -0.291 of it batch 48's and the rest batch 52's shortening of the reach to 128. It is left in rather than corrected: every row here was taken in one sitting and the file's own claim is that they are comparable to each other. Patching one of them from a later bench would buy an accurate cell at the cost of the property the table exists for. Re-take the set, or read the batch-48 and batch-52 sections.

**And the map is sixteen vantages where the fixture now has eighteen.** `sea-horizon` was added by batch 52 and `deep-water` by batch 53, and neither is spliced in here, for the same reason: a row taken in a different sitting is not comparable to these. What is worth knowing before the set is re-taken is that **`sea-horizon` would be the most expensive water vantage in it and the only one where `march` is outright the larger pass** -- 4.15 ms against `resolve`'s 3.74, where `underwater` reads 3.80 against 3.86 -- and that **both of those figures predate batches 53 and 54**, which took roughly 0.4 ms out of `march` at `sea-horizon` and put 0.26 into `resolve` at `deep-water`. The whole table is pre-53 and a re-take is owed; the two batches' own sections below carry paired deltas, which is what a comparison needs and a stale absolute is not.

### What the map says

- **`resolve` is 64% of the set and `march` 26%**, and those two are 90% of everything. The
  spread behind the `resolve` headline is **44% at `underwater` to 81% at `coastline`** -- it is a
  statement about what is on screen, not about the pass. [`docs/gpu.md`](docs/gpu.md) has the
  three pixel kinds that explain the whole table.
- **`terraces` is the most expensive frame in the fixture**, as it was at batch 44, and it had
  never been benched before that. Every cost table here from batch 8 to batch 43 was taken at
  `default` or `coastline`, and the standing sentence *the coastline is the expensive vantage*
  was true only of the two cameras anybody had measured.
- **`march` is very nearly the larger pass at `underwater`** (3.80 against 3.86 of `resolve`, on
  **161554 pairs**, the highest count in the set) **and at `canopy`** (3.30 against 3.50). Every
  `march` figure this project has quoted came from a camera where it is a quarter of the frame.
  **Still unexamined.**
- **`shaft_scan` is 0.11 to 0.12 ms at fifteen of the sixteen** and 0.00 at `night`, where the
  sun is down and `scene::shaft_scan` declines to dispatch it -- batch 35's constant-cost claim
  holding across a whole vantage set rather than across two cameras.

**Absolute milliseconds cross no sitting**, which is this file's first caveat: these are lower
than the batch-44 table partly because batch 45 shipped and partly because the machine was in a
different state. What is comparable is every row above against every other row above -- and as
a cross-check, `terraces` moved 9.88 -> 8.43 against batch 45's separately measured -1.584, and
`default` 6.09 -> 5.68 against its -0.502.

---

## Where `resolve` goes at `terraces`, the most expensive frame

**Batch 44, eight paired rounds per row**, `harness bench --vantage terraces --extra-b <control>`,
so each row is one free control against the same build. This is the decomposition the frame map
above asks for, taken at the vantage it named.

| control | `resolve` delta | 1 se | t | rounds positive |
|---|---|---|---|---|
| `--no-water-reflect` | **-4.178 ms** | 0.034 | -122.85 | 0/8 |
| `--no-water-refract` | **-4.031 ms** | 0.010 | -386.65 | 0/8 |
| `--no-waves` | -1.358 | 0.055 | -24.86 | 0/8 |
| `--no-foliage` | -1.235 | 0.043 | -28.55 | 0/8 |
| `--no-glass` | **-0.520** | 0.025 | -20.68 | 0/8 |
| `--fog-density 0` | -0.221 | 0.040 | -5.55 | 0/8 |
| `--no-terrain-shafts` | -0.084 | 0.068 | -1.23 | 3/8 |
| `--no-distant-shadows` | -0.016 | 0.071 | -0.23 | 5/8 |

The rows do not sum to the pass and are not meant to: switching off the refraction also removes
the `shade_hit` it calls at the far end, and the last two are inside their own standard error,
which is the two envelope readers costing what [`docs/sky.md`](docs/sky.md) says they cost.

**Two things worth carrying out of it.** Water's two secondary legs are **8.2 ms of a 10 ms**
`resolve` at this vantage, so anything aimed at `resolve` that is not aimed at those two is
aimed at a fifth of the pass. And **`--no-glass` recovers 0.520 ms here** -- roadmap 38e's
shipped regression showing up at a third vantage, in a world that again holds no glass.

## What each feature costs

**These were each measured in their own session against a revert binary, so they are internally
honest and not comparable with each other or with the tables above.** The reasoning for each is in
the subsystem document.

| feature | cost | vantage | where |
|---|---|---|---|
| water | **+0.16 ms** | default | [`water.md`](docs/water.md) |
| water | **+1.27 ms** | coastline | |
| foliage | **+1.85 / +2.13 ms** | default / coastline | [`foliage.md`](docs/foliage.md) |
| leaf cutouts | **not benched** -- folded size only, see below | -- | [`foliage.md`](docs/foliage.md) |
| glass | **+1.544 ms** | a glasshouse filling the frame | [`shading.md`](docs/shading.md) |
| glass, existing | **+0.573 ms**, of which the override buys back 0.346 | default, no glass in the world | |
| glass, existing | **+1.036 ms -- a shipped regression, roadmap 38e** | coastline, no glass in the world | |
| god rays + cloud shadow | +0.86 / **+1.47 ms** | into the sun / coastline | [`sky.md`](docs/sky.md) |
| terrain shafts | **+0.17 to +0.30 ms** | low-sun and coastline, four sessions | [`sky.md`](docs/sky.md) |
| distant terrain shadows | **+0.06 / +0.02 ms**, and zero at the coastline | lattice / lod | [`sky.md`](docs/sky.md) |
| clouds | +0.13 to +0.47 ms | four vantages; **0.00 with `--no-water`** | [`sky.md`](docs/sky.md) |
| shallow-water damping | +0.26 / +0.46 / **+0.79 ms** | default / coastline / shore | [`water.md`](docs/water.md) |
| the wave fill schedule | +0.11 ms | coastline and shore; 0 at default | [`water.md`](docs/water.md) |
| the temporal pass | **0.48 ms** | any | [`temporal.md`](docs/temporal.md) |
| wave micro-normals | +0.01 / +0.03 ms | default / coastline | [`water.md`](docs/water.md) |
| the biome tint, running | +0.01 to +0.05 ms | default | [`shading.md`](docs/shading.md) |
| biomes | +0.06 ms GPU, +15% worldgen | | [`terrain.md`](docs/terrain.md) |
| the cross-fade | +4.5% | the LOD vantage | [`lod.md`](docs/lod.md) |
| the coarse meadow | free by construction | | [`terrain.md`](docs/terrain.md) |
| **water's shadow budget, 48 -> 16** | **-1.672 / -0.531 / -0.465 ms** | coastline / default / shore | [`water.md`](docs/water.md) |
| the water mottle | **zero** -- an atlas constant with no shader path | | [`water.md`](docs/water.md) |
| **off-screen shadow casters** | **+0.100 ms** (+/- 0.020, t = -4.98); nothing distinguishable at `default` (-0.050 +/- 0.042) or `offscreen-shadow` (-0.014 +/- 0.047) | terraces | [`lod.md`](docs/lod.md) |

**Batch 64's row is the only one here whose cost is paid by a pass that got no new code**, and it is worth the sentence. Off-screen chunks are now uploaded and written into `grid` so they can cast shadows, and what that costs is `march_chunk` called on grid cells that used to read `NO_CHUNK` -- a ray that now *hits* terminates early, and one that still misses has paid for a traversal it did not do before. So the cost tracks how much geometry a secondary ray crosses and **not** how many pixels changed colour: `terraces` is +0.100 for 7,843 pixels moved, and `offscreen-shadow` is indistinguishable from zero for 18,468. **`tile_select` is +0.000 to +0.001 at all three vantages in both Hi-Z arms** with the chunk array more than doubled -- 157 entries to 346 at `default` -- because that pass loops `frame.chunk_count`, which stayed the frustum-culled length.

**The terrain shaft is the one feature here whose cost is a constant.** Its scan pass reads
**0.12 ms at both of those vantages, to the digit** -- a fixed 512x512 grid scanned eight
times, with nothing in it that depends on the window size, on how much geometry is on screen
or on where the camera points. The rest of the row is `resolve`'s one tap, and it is **never
separable from zero** in a paired A/B: four sessions put the frame total between +0.17 and
+0.30 with `t` from -2.8 to -14.0, and `resolve`'s own split between -0.045 and -0.149 with
`t` from -1.3 to -5.8, which does not settle even at sixteen rounds. **That is the shape a
fixed pass plus a small tap makes**, and it is the opposite of the shape the rejected version
had: +1.08 to +10.57 ms depending entirely on reach and vantage.

**Batch 37's row is the second tap into the same field and prices like one.** `resolve` takes
**+0.062 ms at `lattice`** (t = -9.65) and **+0.019 at `lod`** (t = -15.00) -- small, and unlike
the shaft's tap they *are* separable from zero, because this one is paid per visible pixel rather
than four times per view ray under an importance ramp. At the **coastline it is +0.045 +/- 0.042,
t = +1.07, indistinguishable from zero**, which is the vantage [`errors.md`](docs/errors.md)
insists anything new in `resolve` be priced at first: it is a low sun down a water-filled frame,
so almost no pixel reaches the tap. The scan pass is not charged twice -- batch 35 already runs
it, and batch 37 only widened the condition deciding whether it runs at all.

**Batch 38b's rows are the widest gap in this table between a feature's cost and its batch's**,
and the last of them is a regression rather than a price. Four paired benches in one session, all
four 0/8 rounds positive.

- With a glasshouse filling a 1920x1080 frame, `resolve` is **+1.544 ms +/- 0.047, t = -33.10**.
- At `default`, where **no glass exists anywhere and the two builds render bit-identical frames**,
  it is still **+0.573 +/- 0.043**, and `--no-glass` recovers **0.346 +/- 0.035** of that.
- **At `coastline` it is +1.036 +/- 0.088, t = -11.80 -- 13.324 against 12.288, an 8.4% regression
  on the most water-heavy vantage in the set, for code that frame never executes.** That is the same
  order as `--no-foliage`'s +1.24 to +2.07 below, which this file calls the largest regression the
  project has shipped, and which batch 15 fixed rather than tolerated. It is roadmap entry 38e, at
  the front of the queue.

**`shaderstats` is what says where it comes from.** `resolve` is **615,424 bytes** with `SPEC_GLASS=1`
and **293,888** with it off, and the register count moves **80 to 96** -- the first time that pass has
left 80 since batch 13, and the number the occupancy entry in [`errors.md`](docs/errors.md) has
quoted as fixed ever since. `march` is byte-identical either way. **Binary size and register count
are both too coarse to finish the diagnosis** -- two builds once reported identical figures while their
SPIR-V was 708 bytes apart -- so 38e's first move is a diff of the module, not another stopwatch.

**Two costs that are about a *control* rather than a feature**, and both are worth knowing before
you take a measurement with them on:

- `--cloud-shadow 0` costs **+0.09 to +0.11 ms** switched on, because the shaft factor rides
  through `sky_base`, which every fogged pixel evaluates.
- `--no-foliage` cost **+1.24 to +2.07 ms** before batch 15's `override` folded it out -- the
  largest regression this project has shipped, and it was in a control.

Every other control is free, and `--no-distant-shadows` is the most recent one measured rather
than assumed: against `voxelcraft-pre37.exe` at `lod` with hi-z on it is **resolve +0.000 +/-
0.000 and total +0.001 +/- 0.004**, which is the `SPEC_MASK` fold arriving as a stopwatch reading
and not only as an argument. **A control that costs a millisecond quietly taxes every measurement
taken with it.**

---

## The water sweep, batch 40 -- where water's cost actually is

**Eight paired benches in one sitting, so these are comparable with each other and with nothing
else in this file.** Each is `harness bench --vantage coastline --exe voxelcraft-pre40.exe
--exe-b <a build one constant away from it>`, 1920x1080, `--bench-frames 140`, 8 paired rounds,
order alternating. Every row moved `resolve` and **nothing else**: `march`, `tile_select` and
`taa` are flat to within their own standard errors in all eight.

`resolve` read 11.3 to 13.5 ms across the sitting depending on which bench, which is the power
drift this file's first caveat describes. **Only the paired deltas below mean anything.**

### The three families P1's research proposed, priced

| one-constant build | `resolve` delta | 1 se | t | rounds positive |
|---|---|---|---|---|
| `WATER_SHADOW_DIST` 48 -> 0 | **-4.100 ms** | 0.068 | -60.59 | 0/8 |
| `WATER_REFLECT_DIST` 768 -> 384, fade 256-384 | **-1.476 ms** | 0.061 | -24.34 | 0/8 |
| `CLOUD_OCTAVES` 4 -> 1 | **-0.594 ms** | 0.016 | -37.84 | 0/8 |
| `CLOUD_OCTAVES` 4 -> 1, **`--no-water` on both sides** | **-0.619 ms** | 0.028 | -22.10 | 0/8 |

**The fourth row is the one that settles the sky question, and it settles it against the
research.** The octave saving is the *same size with the water taken out of the frame*, so
water's own `sky_color` calls contribute nothing measurable and the whole saving is primary sky
pixels. All three documents ranked a sky-term fix first and one priced a planar cloud buffer at
"at least 3.0 ms"; **the entire cloud FBM at a sea-filling vantage is worth 0.6 ms, and none of
it is water's.** A cloud cache is a *sky* optimisation and belongs to whatever batch wants the
sky faster.

**`WATER_SHADOW_DIST` is the finding, and nobody proposed it.** All three documents modelled a
secondary ray as a traversal loop. It is not: a refraction ray that hits calls `shade_hit`, which
casts a **third** ray -- and that third ray is **4.100 ms of a 13.080 ms coastal `resolve`,
31% of the pass**, against 1.476 for the reflection reach the research did rank and 0.0 for the
sky term it ranked first.

### The shadow ray's cost curve

| `WATER_SHADOW_DIST` | delta vs the shipped 48 | 1 se | cost above the 0 build | per block, that segment |
|---|---|---|---|---|
| 48 (shipped) | -- | -- | 4.100 | -- |
| 32 | **-0.702** | 0.003 | 3.398 | 0.044 |
| 16 | **-1.707** | 0.023 | 2.393 | 0.063 |
| 8 | **-2.303** | 0.093 | 1.797 | 0.075 |
| 0 | **-4.100** | 0.068 | 0 | 0.225 |

**The first eight blocks cost three to five times per block what the last sixteen do**, which is
the signature of a ray that usually terminates early: the near blocks are paid by every ray and
the far ones only by the rays that found nothing. So the curve is steep where the look lives and
shallow where the savings are -- **truncating 48 -> 24 buys roughly 1.2 ms and can only affect
the pixels whose occluder is 24 to 48 blocks away**, which is the distant shore's shadow on a sea
bed that [`water.md`](docs/water.md) already names as what this constant gives up.

`WATER_SHADOW_DIST` 48 -> 0 at **`shore`** is **-2.096 ms +/- 0.002**, so the effect is not one
vantage's.

**None of these is a shipped change.** `WATER_SHADOW_DIST` 0 deletes the sea floor's sun shadow
outright and the reach truncation moves distant reflections; both need `--reference` to rank the
look, which is the batch after this one. What this sitting bought is that the batch is now a
*look* decision with its cost already known, rather than a cost discovery.


## Batch 41: water's shadow ray, 48 blocks to 16

**The look decision batch 40 left, ranked and shipped.** Everything here was measured against
`voxelcraft-pre41.exe` (`c4021d6`), which is the same binary as `pre40` plus batch 40's
documentation; the bench is one sitting of three vantages, and the look sweep is deterministic
single-frame captures at the fixture's own 1280x720 with the temporal pass off.

### What it costs, paired and alternated

`harness bench --exe voxelcraft-pre41.exe --exe-b <the shipping build> --rounds 8`, 1920x1080,
`--bench-frames 140`. **A is 48 blocks, B is 16**, so a negative delta is the saving.

| vantage | hi-z | pass | A | B | paired delta | 1 se | t | rounds B > A |
|---|---|---|---|---|---|---|---|---|
| `coastline` | on | `resolve` | 13.482 | 11.810 | **-1.672** | 0.007 | -223.00 | 0/8 |
| `coastline` | on | total | 15.931 | 14.255 | **-1.676** | 0.010 | -174.16 | 0/8 |
| `coastline` | off | `resolve` | 13.477 | 11.837 | -1.640 | 0.050 | -32.80 | 0/8 |
| `shore` | on | `resolve` | 10.856 | 10.391 | **-0.465** | 0.002 | -246.07 | 0/8 |
| `shore` | on | total | 13.586 | 13.121 | -0.465 | 0.002 | -246.05 | 0/8 |
| `default` | on | `resolve` | 7.901 | 7.370 | **-0.531** | 0.001 | -425.01 | 0/8 |
| `default` | on | total | 12.026 | 11.491 | -0.535 | 0.003 | -200.18 | 0/8 |

**`default` saves 0.531 ms with the sea confined to the right edge of the frame**, which is
the shape water's cost has always had here -- bounded below at exactly zero by the chunk's
has-water bit and above by how much sea is on screen. As a share of the frame it is 10.5% at
`coastline`, 4.4% at `default` and 3.4% at `shore`.

**Only `resolve` moved.** `tile_select`, `march`, `build_hiz`, `recover` and `taa` are flat to
+/-0.012 ms at every vantage and every hi-z setting, which is what a change to one constant in
the deferred pass should look like.

**It corroborates batch 40 across a day and a power state.** That sitting measured the same
constant at **-1.707 +/- 0.023** from the other direction -- as the 32 -> 16 step of a curve --
against **-1.672 +/- 0.007** here, a 2% disagreement between two independent pairings.

### What it costs to look at

Seven one-constant diagnostic builds against the same parent, six vantages, whole frame, as
`pixels differing / max channel delta` out of 921,600:

| build | `coastline` | `shore` | `terraces` | `lattice` | `open-sea` | `default` |
|---|---|---|---|---|---|---|
| shadow 48 -> 32 | 80 / 1 | 3 / 1 | **0** | 57 / 1 | 201 / 1 | **0** |
| shadow 48 -> 24 | 201 / 1 | 4 / 1 | **0** | 62 / 1 | 428 / 1 | 159 / 11 |
| **shadow 48 -> 16 (shipped)** | **784 / 1** | **19 / 1** | **0** | **488 / 1** | **1,309 / 1** | **247 / 10** |
| shadow 48 -> 8 | 17,462 / 2 | 34 / 1 | **0** | 4,866 / 1 | 10,520 / 1 | 253 / 9 |
| shadow 48 -> 0 | 50,054 / 3 | 92 / 1 | **0** | 19,883 / 2 | 33,610 / 3 | 554 / 6 |
| reflect 768 -> 512 | 13,842 / **145** | 5,691 / **117** | **0** | 20,332 / **109** | 12,128 / **161** | 730 / **98** |
| reflect 768 -> 384 | 18,214 / **146** | 8,702 / **120** | **0** | 22,183 / **109** | 13,947 / **162** | 887 / **105** |

**The amplitude column is the decision and the pixel count is not.** The two families overlap
on how much of the frame they move and are two orders of magnitude apart on how far they move
it: the shadow truncation is a shading residual of a code value or two, the reflection
truncation replaces a reflected shoreline with sky. `errors.md` carries the rejection.

**`terraces` reads exactly zero in every row**, which is the sun elevation rather than the
water: at `--time 0.30` over that basin nothing up-sun is high enough to shadow the floor, and
no reflection in the crop finds a hit past 384 blocks.

### What the light envelope was covering for

The same sweep with `--no-distant-shadows` on **both** sides, which switches off batch 37's
envelope and leaves the march as the only occluder:

| truncation | with the envelope | envelope off on both sides |
|---|---|---|
| 48 -> 16 at `shore` | 19 / 1 | **1,084 / 2** |
| 48 -> 16 at `lattice` | 488 / 1 | 5,233 / 4 |
| 48 -> 0 at `shore` | 92 / 1 | **369,628 / 14** |
| 48 -> 0 at `lattice` | 19,883 / 2 | 39,478 / 4 |

**Two different facts, and the pair is why 16 ships rather than 0.** The 16-to-48 band is
nearly empty with or without a backstop -- 1,084 pixels at max delta 2 is the whole content of
32 blocks of shadow march. The 0-to-16 band is where the shadow actually is: 369,628 pixels,
40% of the frame, and the envelope stands in for it so well that switching it off reads as 92
pixels. **A cheap-looking truncation is either a small effect or a good stand-in**, and only
this table tells the two apart; the near band is a stand-in, and the stand-in is
`WorldGen::height`, which knows nothing about a tree, a wall or an edit.

### Against a converged reference, it is three orders of magnitude under the floor

`CLAUDE.md` says `--reference` is the only thing allowed to rank a look change, so the two
builds were converged over an 8x8 jitter grid in one process and one world and compared to
each other. **The instrument prints its own half-split noise floor**, which is what makes the
reading mean anything:

| vantage | crop | MAE between the builds | max | px differing | the reference's own floor |
|---|---|---|---|---|---|
| `default` | full | **0.0013** | 10 | 281 | **0.7472** |
| `default` | slope (land) | **0.0000** | 0 | **0** | 0.7193 |
| `open-sea` | full | **0.0005** | 1 | 1,356 | **0.3025** |
| `open-sea` | water | **0.0009** | 1 | 287 | 0.0811 |

**575x under the floor at `default` and 90x at `open-sea`'s water crop.** `speckle`,
`coherence`, `rg_std` and `rg_speckle` all agree to four decimals on both frames. An effect
smaller than the floor is not being measured -- which here is the answer rather than a
caveat: this is the project's own ground truth saying the truncation is not resolvable.

**The land crop reading exactly zero is the second half of it.** `--no-water-shadow-cut` is a
water control and the slope crop is dry, so a non-zero there would have meant the budget had
reached a surface that is not seen through the sea.
### The binary did not move

| override | `resolve` instructions | bytes | registers | `march` |
|---|---|---|---|---|
| `SPEC_WATER_SHADOW_DIST=16` | 28,151 | 615,424 | 96 | 21,504 / 72 |
| `SPEC_WATER_SHADOW_DIST=48` | 28,151 | 615,424 | 96 | 21,504 / 72 |

**Identical, which makes this the first saving here that is neither occupancy nor code size.**
Both of `gpu.md`'s mechanisms are about instructions that *exist*; this one is about loop
iterations that *run*, so the register count and the binary size have nothing to say about it
and only the bench does. It is also what makes the control free by construction: the two
pipelines differ in one immediate operand.


## Batch 42: two candidates measured, both killed, nothing shipped

**The first session here whose whole output is two hypotheses failing with numbers attached.**
Everything below is against `voxelcraft-pre42.exe` (`278e7e0`), paired and alternated,
1920x1080, `--bench-frames 140`. A negative delta is B being faster.

### The primary shadow march: real money, and none of it in the reach

`frame.shadow_dist` is 220 blocks, and batch 37's light envelope already covers everything past
it -- the same partition that let batch 41 cut water's budget from 48 to 16 for 1.68 ms. So the
question was what the same cut is worth on the air budget, which is paid by **every lit pixel**
rather than only by pixels with sea in front of them.

| one-constant build | `default` | `low-sun` |
|---|---|---|
| `shadow_dist` 220 -> 0 (delete the march) | **-1.168 ms +/- 0.073** (t = -15.93) | **-0.643 +/- 0.002** (t = -305.15) |
| `shadow_dist` 220 -> 64 (truncate the reach) | **-0.040 +/- 0.000** | **-0.042 +/- 0.002** |

**The march is 16% of `resolve` at `default` -- 1.168 of 7.37 -- and 97% of that is inside the
first 64 blocks.** The reason is the medium and not the budget: a shadow ray on land either hits
terrain at once or escapes to open sky at once, while water's grinds for hundreds of blocks
because water is transparent to a shadow ray. **Batch 41's curve was shaped the other way for a
reason that does not transfer**, and no truncation reaches a land ray's cost without deleting
the shadow it draws.

### Roadmap 38e's first fix: 271,872 bytes out, and slower

`shade_glass` called `shade_water` itself, and the driver answered by inlining a second whole
copy of that tree. Batch 42 built the hoist the entry asked for -- the pane's transmitted leg
substitutes into the primary dispatch, so the module holds one call site instead of two:

| `resolve`, `SPEC_GLASS=1` | bytes | registers | shared mem |
|---|---|---|---|
| batch 38b's shape | 615,424 | 96 | 14,848 |
| batch 42's hoist | **343,552** | 96 | 10,752 |
| for reference, `SPEC_GLASS=0` | 293,888 | 80 | 14,848 |

**85% of everything glass adds to the pass, gone** -- and the frame:

| vantage | `resolve` A -> B | paired delta | 1 se | t | rounds B > A |
|---|---|---|---|---|---|
| `coastline` | 11.81 -> 11.90 | **+0.092** | 0.002 | +56.52 | **8/8** |
| `glass` | 7.60 -> 7.71 | **+0.110** | 0.019 | +5.76 | 7/8 |
| `default` | 7.37 -> 7.47 | **+0.065** | 0.026 | +2.54 | 7/8 |

**`glass` is the row that settles it.** That vantage is a glasshouse filling the frame --
the one place the hoisted code actually *executes* rather than merely existing -- and it is
slower there too. So this is not a frame paying for code it never reaches; the merged
dispatch is simply worse, in the common case and in the rare one alike.

**Bit-exact at all fifteen vantages, `glass` included** -- 0 pixels differing, identical file
hashes -- so this is the same picture drawn by two thirds less machine code, half a per cent
slower. **It was not shipped.**

**What separates the two builds after the hoist is the register count**, 96 against 80, which is
the one number the restructure could not move and the one thing 38e now has left. The mechanism
for the +0.09 is visible in the source rather than in a counter: after the merge the primary
`shade_hit` call takes variables a branch may have overwritten, where before it took values the
compiler could see through, so the *common* path pays for the *rare* one being folded into it.

## Batch 45: flat lighting for secondary hits, and the gate that decided the sign

**The largest single saving this project has shipped since Hi-Z.** A hit reached *through*
water or glass takes one light lookup where the primary hit keeps the nine-cell gather.
`harness bench --exe voxelcraft-pre45.exe --exe-b <the shipping build> --rounds 8`, one
sitting, `resolve` with hi-z on.

| vantage | A (gather everywhere) | B (flat secondary) | paired delta | 1 se | t | rounds B > A | frame total |
|---|---|---|---|---|---|---|---|
| `shore` | 9.890 | 8.206 | **-1.684 ms** | 0.045 | -37.52 | 0/8 | 12.491 -> 10.807 |
| `terraces` | 10.227 | 8.644 | **-1.584** | 0.026 | -61.79 | 0/8 | 13.097 -> 11.510 |
| `coastline` | 10.799 | 9.613 | **-1.186** | 0.022 | -55.07 | 0/8 | 13.046 -> 11.868 |
| `lattice` | 8.288 | 7.561 | **-0.726** | 0.037 | -19.85 | 0/8 | 12.022 -> 11.304 |
| `glass` | 7.240 | 6.557 | **-0.682** | 0.031 | -21.79 | 0/8 | 11.052 -> 10.414 |
| `default` | 6.634 | 6.131 | **-0.502** | 0.037 | -13.56 | 0/8 | 10.374 -> 9.885 |

Every vantage benched is faster and every round of every pair is faster. `march`,
`tile_select` and `taa` are flat to within their standard errors in all six, which is what a
change confined to `resolve`'s shading has to read.

### The same feature, written two ways, differing by 2.58 ms

**This is the finding, and it is not about lighting at all.** The first build gated the arm
the way every *switch* override before batch 41 does -- the compile-time override ANDed with
its own `frame.flags` bit. Measured against the same baseline, same pixels out:

| gate | `terraces` | `default` | `lattice` | `glass` | `coastline` | `shore` |
|---|---|---|---|---|---|---|
| `SPEC_FLAT_SECONDARY && (frame.flags & FLAG_FLAT_SECONDARY) != 0u` | **+1.000** | **+1.420** | **+1.465** | **+1.391** | +0.651 | +0.000 |
| `SPEC_FLAT_SECONDARY` alone | **-1.584** | **-0.502** | **-0.726** | **-0.682** | -1.186 | -1.684 |

**`default` is the row that identifies the mechanism.** It is a frame with water only at its
right edge, so it runs the new arm on almost no pixels -- and it paid the *largest* regression
of the six. A cost that lands hardest where the feature does least work is code existing, not
work running: `frame.flags` is a run-time value, so the driver cannot prove which arm
`shade_hit` takes and keeps **both** the flat lookup and the nine-cell gather in each of the
three secondary copies it inlines, plus the branch.

`shaderstats` agrees and is the cheap check:

| `resolve`, `SPEC_GLASS=1` | bytes | registers |
|---|---|---|
| gather everywhere (the control, and the pre-batch-45 module) | 615,424 | 96 |
| flat secondary, folded | **554,496** | 96 |

**60,928 bytes out and 1.584 ms in.** Set that against batch 42, which took **271,872 bytes**
out of the same pass and came back **+0.09 ms slower**: four times the code for the opposite
sign. Size is not a proxy for time in either direction -- what moved here is the *work*, and
the size is only the evidence that the fold happened. See [`docs/gpu.md`](docs/gpu.md).

### What it costs to look at

`bitexact` against `voxelcraft-pre45.exe`, 1280x720, temporal pass off. This is a deliberate
look change and not an artefact, so the numbers are what it moved rather than a floor it had
to stay under.

| vantage | pixels of 921,600 | max channel delta |
|---|---|---|
| `terraces` | 230,613 | 39 |
| `shore` | 193,680 | 35 |
| `glass` | 115,460 | 39 |
| `lattice` | 49,580 | 24 |
| `coastline` | 49,006 | 27 |
| `open-sea` | 27,007 | 22 |
| `canopy` | 20,427 | 27 |
| `sky` | 3,255 | 10 |
| `overcast` / `default` / `edits` / `lod` / `low-sun` | 972 / 964 / 803 / 731 / 513 | 20 / 20 / 39 / 4 / 14 |
| `underwater`, `cave`, `night` | **0** | 0 |

**Shallow clear water is where it shows and deep or hazy water is where it does not.** What
changes is contact shading on a surface seen through the transport: at `terraces`, two blocks
above a clear lagoon looking down, the submerged steps lose the darkening at their inner
corners and read flatter. At `shore` the same change is very hard to find by eye, because the
absorption over that path has already crushed the cue. `underwater` reads exactly zero because
the camera is below the surface and no water surface is in frame.

**`--no-flat-secondary` is bit-exact against `voxelcraft-pre45.exe` at all sixteen vantages
with identical file hashes**, so the control is a true revert and, being in `SPEC_MASK`, free.

### A footnote the next batch to fold something should have

The two *gate spellings* do not agree to the bit: `terraces` reads 230,613 pixels moved for the
folded build against 230,612 for the run-time one, and one pixel at `shore`. Same arm, same
inputs, one ULP -- the fold rearranges the expression. It does not touch the claim that
matters, which is the control against the parent binary, and that one is exact.

---

## Batch 46: the other eight gates, folded and thrown away

**Batch 45's rewrite does not generalise, and this is the measurement that bounds it.** Eight
switch overrides are gated `SPEC_X && (frame.flags & FLAG_X) != 0u`; all eight were folded to
the override alone, which is the change that was worth 2.58 ms in batch 45.

**The screen came first and predicted the whole result in two minutes.** `shaderstats` with
each gate folded one at a time, no GPU and no bench:

| gate | `resolve` bytes | delta |
|---|---|---|
| baseline | 554,496 | -- |
| `SPEC_WAVE_FILL` | 549,376 | **-5,120** |
| `SPEC_WAVE_ANISO` | 549,504 | **-4,992** |
| `SPEC_WAVE_SHOAL` | 553,984 | -512 |
| `SPEC_WAVES` | 554,112 | -384 |
| `SPEC_TEX_VARIATION` | 553,216 | -1,280 |
| `SPEC_TINT` | 553,216 | -1,280 |
| `SPEC_DISTANT_SHADOWS` | 554,240 | -256 |
| `SPEC_TERRAIN_SHAFTS` | 554,240 | -256 |
| **all eight together** | **541,952** | **-12,544** |

Every one folds further, so none could be ruled out on the screen -- but **12,544 bytes
against batch 45's 60,928** is a fifth of the code change, and `march` is byte-identical
throughout because none of these arms is reachable from it.

### What it buys, paired against `voxelcraft-pre46.exe`

| build | vantage | `resolve` delta | 1 se | t | rounds positive |
|---|---|---|---|---|---|
| all eight | `terraces` | **-0.119 ms** | 0.029 | -4.09 | 1/8 |
| all eight | `shore` | -0.065 | 0.040 | -1.62 | 2/8 |
| all eight | `default` | -0.060 | 0.044 | -1.36 | 2/8 |
| all eight | `coastline` | -0.049 | 0.088 | -0.55 | 3/8 |
| all eight | `low-sun` | -0.020 | 0.034 | -0.59 | 3/8 |
| the two largest movers only | `terraces` | -0.058 | 0.039 | -1.49 | 2/8 |
| the two largest movers only | `shore` | -0.053 | 0.035 | -1.48 | 2/8 |
| the two largest movers only | `open-sea` | **+0.001** | 0.021 | +0.06 | 5/8 |

**One row of eight clears its own standard error.** Every delta points the right way, which is
why this is a small real effect rather than noise -- but it is **-0.119 ms at the most
expensive vantage in the fixture and nothing measurable at four others**, and no subset is
individually significant, so there is no defensible partial ship either. The whole eight are
bit-exact at all sixteen vantages with identical file hashes, as a pure fold has to be.

### Why batch 45 was different, which is the finding worth keeping

**The gate shape is not what costs; the gate shape times the arm behind it times how many
copies the pass inlines is what costs.** Batch 45's arm was the **nine-cell light gather**
inside `shade_hit`, which `resolve` inlines **four** times -- so the run-time test kept three
redundant copies of nine memory lookups. These eight guard small arms: a uv permutation, a
simplex tint, a shaft sample, four wave-field branches. Same transformation, a twentieth of
the result.

**And the `frame.flags` half is not free to remove.** It is what keeps the flag word the thing
that decides, so a pipeline key that ever disagreed with `frame.flags` would still draw the
picture the flag asked for. Batch 45 spent that property once for 1.5 ms. A tenth of a
millisecond at one vantage does not buy it eight more times, so batch 46 shipped the
measurement and not the code.

---

## Batch 47: the smooth-lighting gather priced, and what `resolve` is bound by

**The first time this project has separated memory from instruction count on any part of
`resolve`**, which [`docs/gpu.md`](docs/gpu.md) has listed as unknown since batch 13. Two
diagnostic builds, twenty seconds each, and no code shipped.

### What the nine-cell gather costs

The primary hit's smooth lighting replaced by the one-lookup flat model batch 45 gave
secondary hits. It deletes the gather's work *and* its code, so it is the ceiling on anything
aimed here. `harness bench --exe voxelcraft-pre47.exe --exe-b <the diagnostic> --rounds 8`.

| vantage | `resolve` A | B | delta | 1 se | t | share of the pass |
|---|---|---|---|---|---|---|
| `cave` | 2.455 | 0.691 | **-1.764 ms** | 0.005 | -331.26 | **72%** |
| `default` | 5.764 | 4.847 | **-0.916** | 0.022 | -41.43 | 16% |
| `edits` | 6.090 | 5.186 | **-0.904** | 0.034 | -26.74 | 15% |
| `canopy` | 3.530 | 2.631 | **-0.899** | 0.012 | -77.73 | 25% |
| `lod` | 5.787 | 5.100 | **-0.687** | 0.026 | -26.48 | 12% |
| `terraces` | 8.901 | 8.303 | **-0.599** | 0.028 | -21.59 | 7% |
| `coastline` | 9.546 | 9.481 | -0.065 | 0.058 | -1.11 | 1% |

**It is the largest single identifiable block of `resolve` on a land frame**, and `cave` is the
clean reading: no sky, no water, every pixel a near primary hit, and **72% of the pass is this
one block**. `coastline` is the other end -- a frame of water and sky barely reaches it.

### It is not memory, and that is the finding

**The nine samples depend on `(ci, v, normal_id)` and not on where in the face the pixel
landed** -- only the two bilinear weights are per-pixel. So every pixel covering one block face
recomputes them identically, and at the bottom of the `default` frame a block is about twenty
pixels across: up to four hundred pixels asking one question. That is the case for caching the
gather per face in workgroup memory.

**One diagnostic retired it before anything was built.** Point all nine reads at the *same
cell*: identical instructions, `resolve` moves 640 bytes, and the memory system sees one
address instead of nine -- perfect sharing, with none of a cache's tag compares or barriers.

| vantage | perfect sharing | the ceiling above | share recovered |
|---|---|---|---|
| `cave` | **-0.191 ms** +/- 0.004 | 1.764 | **11%** |
| `default` | **-0.123** +/- 0.014 | 0.916 | **13%** |
| `canopy` | **-0.106** +/- 0.008 | 0.899 | **12%** |
| `edits` | **-0.092** +/- 0.013 | 0.904 | **10%** |

**Collapse the memory traffic nine to one and 87-90% of the cost stays.** So the gather is
instruction-bound, not memory-bound, and a real cache -- which pays a tag compare, a barrier
and an imperfect hit rate -- would recover less than these rows, for a substantial change to a
pass that declares no workgroup memory and no barriers today. **Not built.**

**Where the instructions are, for whoever comes back to this**: eighteen `light_curve`
evaluations (each three multiplies and an add -- no transcendental, so a 16-entry table for the
4-bit levels buys nothing), four corner reciprocals, the Minecraft AO rule and three bilinear
mixes. No hotspot; the cost is spread. **Making this cheaper means doing less of the
arithmetic, which means changing the look** -- which is exactly what batch 45 did for the three
hits nobody looks at directly, and why it stopped there.

---

## Batch 48: the submerged reach cut, and a constant the fixture cannot rank

**The first batch aimed at `march`**, which roadmap P4 had called 23% of the fixture with zero
batches against it. What shipped is a `tile_select` change; what is worth more than the
milliseconds is the last section, because the sweep that was supposed to author the constant
turned out to be measuring nothing.

### The roadmap's own shape was wrong, and reading the constant is what said so

P4 asked for a clamp on `frame.far` when the camera is submerged -- "a uniform change, no
shader edit" -- on the premise that *absorption kills the image within tens of blocks*.
`WATER_EXT` is `(0.280, 0.055, 0.020)` per block and its own comment says the rest: **red is
gone within a few blocks and blue survives tens.** "Tens of blocks" is red's number. Blue still
transmits **7.7% at 128 blocks** and 0.6% at 256.

**And a ray's water path is not its length.** `water_path` is analytic against the sea plane,
so a ray leaves the medium at `(sea_level - cam.y) / rd.y` and is unattenuated every block
after. At the fixture's own submerged eye -- three blocks down, pitched -14 degrees, 70-degree
field of view -- the frame reaches **+21 degrees**, where a ray is out of the water within
**nine blocks**. A uniform clamp deletes the world above the waterline for the top third of the
frame.

So the shipped rule is per *tile* and tests the water path: the most upward of the four corner
rays `tile_select` already builds decides, because it escapes soonest. If even that ray is
still submerged at `WATER_FAR_DIST`, every ray the tile marches is. `frame.far` is untouched,
which also leaves `depth_key`'s quantisation and the Hi-Z scale alone -- a hazard the uniform
version would have introduced the moment the camera entered water.

### What it buys, paired against `voxelcraft-pre48.exe` at `underwater`

Eight rounds, alternated, Hi-Z on. The naive ceiling is the uniform clamp; the other two are
the shipped directional rule at two reaches.

| build | `select` | `march` | `march2` | `resolve` | **total** | pixels moved |
|---|---|---|---|---|---|---|
| uniform clamp, 128 | +0.000 | **-0.499** | -0.040 | **-0.814** | **-1.371** | 1 at delta 8 |
| directional, 128 | -0.010 | **-0.334** | -0.030 | **-0.755** | **-1.135** | **0** |
| directional, 256 (shipped) | -0.006 | **-0.053** | -0.016 | **-0.214** | **-0.291** | **0** |

Every row is 8/8 rounds in the same direction; the shipped row is t = +16.58 on the total and
t = +28.33 on `resolve`. With Hi-Z **off** the 128 rows are -4.236 and -3.121, and `march` alone
is -3.428 and -2.358 -- so most of what this removes at the shipped settings, Hi-Z was already
removing, and the clamp collects the remainder that a stale Hi-Z cannot.

**Absolute milliseconds cross no sitting here either**: `voxelcraft-pre48.exe` read 8.386 ms in
one run of this table and 8.654 in another. Only the paired deltas are comparable, which is the
whole reason `harness bench` exists.

### `resolve` moved 0.755 ms and nothing in it was touched

**A delta with no mechanism, recorded rather than chased**, which is what `CLAUDE.md` says to do
with one. The directional build changes exactly one expression -- `tile_select`'s cull compares
`dmin` against a per-tile reach instead of `frame.far` -- and `frame.far` itself, every reader
of it in `resolve`, and the final image are all unchanged. `resolve` is decoding the same
visibility buffer and shading the same pixels, bit for bit, 0.755 ms faster.

Ruled out so far:

- **`frame.far`'s other readers in `resolve`.** The uniform ceiling changed the uniform and
  could have moved them; the directional build does not touch it and `resolve` still moves.
  The two builds' `resolve` deltas are within 8% of each other.
- **Pixels changing.** 16 vantages, 0 differing, identical file hashes.
- **`MAX_PAIRS` truncation.** 161,554 pairs against a `1 << 20` cap, and
  `warn_if_truncated` is silent. (That cap is now the *floor* of a screen-sized budget -- batch 53 -- and is unchanged at this resolution.)

What is left is cache residency in the visibility buffer -- `march` writes it far fewer times,
so `resolve` reads it warmer -- or a GPU power-state effect, which this tree has already
attributed a 1.8x to once. Neither has been tested. The roadmap carries it.

### The constant is authored, not measured, and that is the finding

The sweep that was meant to rank `WATER_FAR_DIST` **cannot see it.** Every reach from 32 blocks
upward is bit-exact at `underwater`, and so is every extinction:

| test at reach 128 | blue transmittance at 128 | pixels moved |
|---|---|---|
| `--water-absorb 1.0` (shipped) | 7.7% | **0** |
| `--water-absorb 0.5` | 28% | **0** |
| `--water-absorb 0.25` | 53% | **0** |
| `--water-absorb 0.10` | 77% | **0** |
| `--water-absorb 0.05` | **88%** | **0** |

Water barely absorbing at all, and nothing moves. The probe that explains it is a reach of
**32**, which moves **one pixel at max delta 4**: `underwater`'s submerged content ends within
32 blocks, so there is nothing out there to cut at any reach above it. **The zero-pixel readings
are the vantage being blind, not the constant having margin** -- and banking them as margin is
exactly the error `harness.md` records about a known-positive read off the wrong frame.

So **256 ships and 128 does not, at four times the cost**, on the medium's own constant rather
than on a capture: blue is under 1% there, against a 1/255 bound of 276 blocks. The argument
that would justify 128 -- the sky-light flood loses a level per block of water, so anything deep
is dark before absorption touches it -- **fails for the case this is aimed at**, because a
horizontal ray at three blocks' depth stays near the surface the whole way, where the flood has
darkened nothing.

**What would settle it is a crop, and this is batch 38d's position exactly**: a number on file,
unrankable by any existing vantage, waiting for the session that builds the camera. There it was
`LEAF_FILL`, deferred five batches until batch 43 built `canopy` and collected it. The crop here
is a submerged eye looking along the horizontal across open water with lit geometry 150 to 400
blocks out, and **0.844 ms at `underwater` is what it is worth.**

---

## Batch 49: what `march` writes is billed to `resolve`

**Roadmap P3b, and it is answered.** Batch 48 changed one expression in `tile_select` and
`resolve` came back **-0.755 ms at `underwater` on a bit-identical frame**, with `frame.far`
untouched and every reader of it in `resolve` untouched. The two candidates left were a warmer
visibility buffer and a GPU power state, and the second mattered far beyond the number: **if
`march` doing less work raises the clock `resolve` runs at, the per-pass split of every paired
bench in this project is soft**, including batch 45's 2.58 ms swing and batch 47's whole table.

**It is neither.** It is the visibility buffer's **write traffic**, and `march` doing less
*work* moves `resolve` by nothing at all.

### The build that separates them

Batch 48's change moves two things at once -- how many pairs `march` is handed, and how many
`atomicMax` writes it issues into the visibility buffer. The discriminator is a third build
that moves one without the other: `tile_select` emits the pair batch 48 would have culled,
**tagged in bit 31 of the pair word**, and `march` does the full traversal and skips only the
write. Call it the **ghost** build.

| build | pairs `march` is handed | writes issued | how it is run |
|---|---|---|---|
| **A** | pre-batch-48 | pre-batch-48 | `--no-water-far` |
| **G** | **pre-batch-48** | **batch 48's** | the ghost binary |
| **S** | batch 48's | batch 48's | the shipping rule |

**The pair counts are the check that G is what it claims**, taken at the bench's own 140 frames
and 1920x1080 rather than at a snapshot, because the bench yaws 1.4 radians over its run:

| hi-z | A | G | S |
|---|---|---|---|
| off, pairs marched | 372,335 | **372,335** | 221,963 |
| on, pairs marched | 161,554 | 183,808 | 150,950 |
| on, deferred | 210,898 | 188,644 | 70,890 |

**Hi-z off is the clean half and the row that matters: G marches A's pair set exactly**, so
A -> G varies the writes and nothing else, and G -> S varies the work with the writes already
gone. A's 161,554 is the same count batch 48 quoted, so this is the same experiment.

**Hi-z on is confounded and the counts say why.** Suppressing a write changes what `build_hiz`
finds in the visibility buffer, which changes how the *next* frame routes pairs between `march`
and the deferred list -- G marches 183,808 where A marches 161,554. That feedback is real and
is why the decisive readings below are quoted with hi-z off; the hi-z-on block is reported
beside them and closes just as well, but it is not a one-variable experiment.

**The two facts look contradictory and are not, which is worth a sentence because the next
reader will stop here.** If the frame is bit-exact, how can `build_hiz` see anything different?
Because **a bit-exact frame is not a bit-exact visibility buffer**: a far chunk that is the only
hit at its pixel leaves a key where suppressing it leaves sky, and the two *render* the same
8-bit colour because the medium has absorbed everything over that path -- which is batch 48's
own finding. `resolve` cannot tell them apart and `build_hiz`, which reads the keys, can. It is
also why the general form of this in roadmap P3c is cleaner than the ghost build rather than the
same thing: reading before writing removes only atomics that would not have changed the buffer,
so nothing reroutes.

### The decomposition, at `underwater`, 8 rounds, alternated

**Hi-z off**, where the pair counts say each experiment moves one thing:

| experiment | what varies | `march` | `resolve` | total |
|---|---|---|---|---|
| **A -> G** | **the writes alone** | -0.077 | **-0.770** | -0.850 |
| **G -> S** | **the work alone** | **-2.380** | **-0.002** | -2.386 |
| A -> S | batch 48, both at once | -2.312 | -0.740 | -3.056 |

**Hi-z on**, the shipping configuration:

| experiment | `march` | `resolve` | total |
|---|---|---|---|
| A -> G | +0.312 | **-0.756** | -0.464 |
| G -> S | -0.653 | **+0.009** | -0.664 |
| A -> S | -0.328 | -0.740 | -1.109 |

**It closes on every pass in both blocks**, which is the property that makes it a decomposition
rather than three separate numbers: hi-z on, `march` +0.312 - 0.653 = -0.341 against -0.328
measured, `resolve` -0.756 + 0.009 = -0.747 against -0.740, total -0.464 - 0.664 = -1.128
against -1.109.

**Hi-z off closes less tightly and the reason is the sitting rather than the arithmetic**:
`march` -2.457 against -2.312, `resolve` -0.772 against -0.740, total -3.236 against -3.056 --
a 6% residual, against 1-4% with hi-z on. The three experiments were three separate sittings, and
**the A side of the first two is the same behaviour measured twice** -- pre-batch-48, in two
binaries -- reading `resolve` 3.678 and 3.757, a 2% drift with a true difference of nothing.
That is the drift this file's first caveat is about. **The paired deltas are what cross a sitting and the absolute
levels are not**, so a residual of that size is the expected cost of comparing three of them
rather than evidence of a fourth term.

**The row that answers P3b is `G -> S`: `march` does 2.380 ms less work and `resolve` moves
0.002 ms**, t = -0.39, 2/8 rounds. Not a power state, not a clock, not a scheduler. Whatever
`march` does with its own instruction stream, `resolve` cannot tell.

**The row that explains batch 48 is `A -> G`: 150,372 (tile, chunk) pairs' writes suppressed --
at most 9.6 million rays' worth of `atomicMax` -- is worth 0.850 ms, and 91% of it is billed to
`resolve` rather than to the pass that issued it.** `march` keeps 9%.

### The decisive measurement is a same-binary A/B, and here that is correct

[`errors.md`](docs/errors.md) rejects a same-binary A/B for a *feature's* cost, because both sides
carry the feature's code -- batch 14 read +0.24 ms that way against a true +1.85. **A -> G is
the case that rule does not cover.** Both sides are the same executable with the same shader,
the same 372,335 pairs and the same final image; the only thing that differs is the run-time
value of one bit, which decides whether a write is issued. Nothing is being priced by its
absence from a binary, so a revert build would add a confound rather than remove one.

**The ghost build's own validity has two checks and they point in different directions**, which
is what [`lessons.md`](docs/lessons.md) asks for when a no-op claim cannot be told from a
not-wired-up one:

- **The negative**: G against S is **0 pixels differing and identical file hashes** at
  `underwater`, `coastline`, `terraces`, `shore` and `default`. A suppressed write either loses
  its `atomicMax` outright or lands at a pixel the medium renders the same colour either way,
  which is batch 48's own finding, so the frame cannot move -- and that is exactly why this zero
  on its own proves nothing, since a gate that never fired would read the same.
- **The positive**: the pair counts above. G marches 372,335 where S marches 221,963, so the
  build really is doing pre-48's work, and it really is skipping 150,372 pairs' writes.

And the **`march` column of A -> G is the check that the driver did not sink the traversal into
the branch**: -0.077 ms of 7.436, 1%. Had the compiler moved `march_chunk` inside `if h.hit &&
!ghost`, G's `march` would have collapsed toward S's and the experiment would have measured
nothing. It did not.

### What is ruled out, and what survives

**A GPU power or clock state -- ruled out twice, by two independent readings.**

- `G -> S` removes 2.380 ms of `march` and `resolve` does not move. A clock that responded to
  how hard the previous pass worked would have to respond to that.
- In `A -> G`, where `resolve` moves **-20.5%**, `select` reads 0.155 against 0.155 and `taa`
  reads 0.502 against 0.501. **A clock change is global**; this one lands on one pass.

**`resolve` reading a warmer visibility buffer -- ruled out.** That was the roadmap's own first
candidate, and the hi-z-on block refuses it: with hi-z on, `build_hiz` runs between `march` and
`resolve` and **streams the entire buffer** -- 16.6 MB at 1920x1080 -- in 0.110 ms identical on
both sides, which is **151 GB/s and therefore a read from memory rather than from cache**.
Whatever `march` left resident is gone before `resolve` starts, and `resolve` still moves -0.756. The effect is
not residency of what was written.

**What survives is narrower and more useful than either candidate**: an `atomicMax` that
`march` issues is *fire and forget* -- the returned value is discarded, so the shader never
waits -- and its cost is paid by `resolve` rather than by `march`. Naming the microarchitecture
under that is beyond what this fixture can see, and the operational statement does not need it.

### What this means for every other number in this file

**The per-pass split is sound for work and for instructions, which is the half that was at
risk.** Batch 45's 2.58 ms gate swing and batch 47's gather table are both arguments about
which pass's *instruction stream* moved, and `G -> S` is the direct test of exactly that
channel: 2.380 ms of one pass's work, 0.002 ms in another. Those results stand as written.

**What is not sound is reading a pass's timestamp as the cost of the writes it issues.** Only
`march` issues fire-and-forget writes in this frame -- `tile_select`'s appends are a handful per
thread and `taa`'s output is read by nothing in the same submission -- so this is a statement
about `march` and `resolve` specifically, and no other row in this file is affected. **A batch
that cuts `march`'s write traffic should expect its saving to show up in `resolve`, and should
not read that as a mystery.**

---

## Batch 50: read before you write

**Roadmap P3c, taken the session after batch 49 filed it.** `march` issues
`atomicMax(&vis[pix], key)` on every hit and discards the result, so a chunk behind a nearer one
pays a full read-modify-write to change nothing -- and batch 49 measured that this is not
`march`'s own problem, because **91% of what suppressing such a write saves is billed to
`resolve`**. The change is the test every depth-buffered renderer has: load the word, and issue
the atomic only if the key can win.

### It is bit-exact by an argument, and that is the awkward part

`atomicMax` only ever *raises* `vis[pix]`. A key that does not beat what is there at the moment
of the load cannot beat what is there at any later moment either, so the atomic it skips is
provably a no-op -- including against a write that races in between, which can only raise the
value further. Nothing to re-check, no lost update.

**So both directions of the control render the same frame, and `bitexact` reads zero either
way**: 16 vantages, 0 pixels differing, identical file hashes for the shipping build against
`voxelcraft-pre50.exe`, and the same 16 zeros for `--no-vis-pretest`. That is not evidence the
feature works. It is not evidence it is wired up. A capture cannot tell a correct pretest from
an inverted one from one that was never reached, because all three draw the identical picture --
which is [`docs/harness.md`](docs/harness.md)'s own warning in its sharpest form, and the reason
the checks that matter here are five source-level guards in `tests/pretest.rs` and one paired
bench.

### The screen came first, and it says where this can pay at all

**Two minutes, no GPU time worth the name, and it predicted the shape of the answer** -- batch
46's method. The saving is one skipped atomic per *losing* write, and a write can only lose if
some other chunk hit the same pixel nearer. So the quantity that decides is how many chunks a
pixel is tested against, which `--bench-frames` already prints as `pairs marched` and which is
that number over the 32,400 tiles of a 1920x1080 frame:

| vantage | pairs marched, hi-z on | chunk tests per pixel |
|---|---|---|
| `underwater` | 159,726 | **4.93** |
| `default` | 92,924 | 2.87 |
| `terraces` | 88,969 | 2.75 |
| `coastline` | 76,012 | 2.35 |
| `cave` | 32,400 | **1.00** |

**`cave` is exactly one pair per tile, and that is structural rather than a coincidence**: it is
the camera's own chunk, which `tile_select` exempts from the box test because from inside a box
the ray-box intersection returns the *exit* distance. Nothing else survives, so nothing ever
competes for a pixel, so **no write can ever lose and the load is pure overhead**. That makes
`cave` the clean worst case, and it is in the bench for that reason rather than for its size.

### The gate spelling was screened offline before anything was benched

Batch 45 found the house `SPEC_X && (frame.flags & FLAG_X) != 0u` spelling worth **2.58 ms**
against the override alone, and batch 46 bounded that to nothing for eight small gates. The rule
those two leave is that the cost is the gate shape *times the arm behind it times how many
copies the pass inlines* -- so this one was compiled both ways and read off `shaderstats`,
which costs two builds and no GPU:

| `march` | override alone | house spelling |
|---|---|---|
| `SPEC_VIS_PRETEST=1` | 21,632 bytes | **21,888 bytes** |
| `SPEC_VIS_PRETEST=0` | 21,504 | 21,504 |
| registers, either | 72 | 72 |

**256 bytes and no registers**, against batch 45's 60,928 and batch 46's 12,544. The arm is one
load and one compare inlined once, not the nine-cell gather inlined four times, so the house
spelling was kept and the flag word stays the thing that decides. Both spellings fold to the
identical 21,504 with the override false, which is what makes `--no-vis-pretest` free:
`march` is one of the two passes `SpecPipes` builds per specialization, so the arm leaves the
module rather than being branched over. `resolve` and `tile_select` are byte-identical
throughout -- this touches one pass.

### What it buys, against a build with the feature compiled out

`voxelcraft-pre50.exe` is the parent commit built, so this is a real revert and not a flag.
8 rounds, alternated, at `underwater` -- which the screen above says should suit it best.

| hi-z | pass | with | without | paired delta | 1 se | t |
|---|---|---|---|---|---|---|
| on | `march` | 4.061 | 4.005 | -0.056 | 0.024 | -2.30 |
| on | `resolve` | 3.925 | 3.896 | **-0.029** | 0.021 | **-1.35** |
| on | total | 9.069 | 8.977 | -0.091 | 0.052 | -1.77 |

**Indistinguishable from zero**, and `default` reads **exactly +0.000** on six consecutive
rounds. The sign is such that a negative delta favours the build *without* the feature; nothing
here clears the fixture's own |t| >= 2 bar in either direction.

**The bound is looser than this project's benches usually give, and the reason is thermal.**
The standard error is **0.021** against the 0.004-0.008 the same vantage returned two hours
earlier in batch 49, so what is actually established is *no effect larger than about 0.06 ms*.
See the heat-soak entry in [`docs/pitfalls.md`](docs/pitfalls.md), which this batch is the
measurement behind. The `default` rows are the trustworthy ones: by then the machine had reached
equilibrium, its rounds read 6.78 / 6.78 / 6.78 / 6.79, and a level that pairing cancels is not
a ramp that it only partly cancels.

### Hi-Z had already taken the win, and the count is the whole explanation

**The null needed a mechanism, and a *count* gets one where a timing could not -- which is also
why it was trustworthy on a hot machine.** A diagnostic build was patched to tally, per frame,
how many of `march`'s hits carry a key that cannot win:

| vantage | hits | would lose, hi-z **off** | hi-z **on** | Hi-Z removes |
|---|---|---|---|---|
| `underwater` | 10.6M / 2.53M | **58.0%** | **12.8%** | 94.7% of the losers |
| `default` | 11.6M / 2.35M | **72.5%** | **21.5%** | 94.0% |
| `cave` | 14.2M / 2.07M | **69.8%** | **0.0%** | 100% |

**An occlusion cull removes precisely the chunks whose writes would lose**, which is what Hi-Z
*is*, so by the time a pair reaches the write there is very little left to skip. The pretest
pays one load on **every** hit to save an atomic on one in five to one in eight of them -- and
at `cave` on none at all. That is a losing trade by construction, and the bench above is what it
looks like.

**`cave` is the cleanest single number in this batch**: 2,073,600 hits at 1920x1080, which is
exactly one per pixel. Every tile marches only the camera's own chunk -- the one `tile_select`
exempts from the box test, because from inside a box the ray-box intersection returns the *exit*
distance -- so nothing ever competes for a pixel and nothing can ever lose.

**The fact worth keeping is the one about Hi-Z rather than the one about the pretest.** This
file has always described Hi-Z as a cull that saves marching. It is also, and by the same
action, a **write cull**: it takes 94% to 100% of the losing `atomicMax` traffic out of the
frame, and batch 49 measured that such traffic is billed at 91% to `resolve`. So part of what
Hi-Z has been buying all along was never `march`'s time at all, and nobody had connected the
two passes that way.

### Why batch 49's number was not a bound on this, and where the entry was wrong

**Batch 49's ghost build suppressed a *superset*, and the overlap with this is small.** It
removed every write from a chunk past 128 blocks: the ones that lose, and the ones that **win**
at a pixel nothing nearer reached -- which is exactly why suppressing them changed what
`build_hiz` saw and rerouted the next frame's pairs. Read-before-write removes only the first
kind, and Hi-Z has already removed most of those. The two sets barely meet.

**The roadmap entry said this in one paragraph and contradicted it in another**, and the
contradiction is worth recording because it is the kind that arrives dressed as caution. It
noted the superset, and then argued the ghost's hi-z-on figure was *pessimistic* for a shipping
version on the grounds that read-before-write causes no Hi-Z feedback. That second claim is true
and irrelevant: the superset is the far larger effect and points the other way. **A measurement
of a superset is an upper bound on the subset and nothing more.**

### Not shipped

The feature is bit-exact, screened at 256 bytes and no registers, and buys nothing measurable.
It would have cost a `frame.flags` bit -- bit 22 of a word this engine keeps spending -- an entry
in `SPEC_MASK` that doubles the pipeline keyspace, a control in the table, and 384 bytes of
`march`. Batch 46's precedent is the rule: *a tenth of a millisecond at one vantage does not buy
that property*, and this is not even a tenth. What lands instead is a comment block at the write
site carrying the counts and the retry condition, so the next session finds the measurement
rather than the idea.

---

## Batch 52: the crop that could rank the reach

**Roadmap P4's third sub-lead, and it had been in batch 38d's position since batch 48** -- a
constant nothing in the fixture could see. `WATER_FAR_DIST` was authored at 256 off blue's
extinction because `underwater`'s submerged content ends within 32 blocks, so every reach above
that read zero pixels at every extinction down to `--water-absorb 0.05`. Those zeros were the
camera.

### The vantage

`sea-horizon`, the seventeenth: `--time 0.35 --cam-submerge 3 --cam-yaw 215 --cam-pitch 0` at the
default seed. A submerged eye along the horizontal at a chain of wooded islands, with open water
in front of them and a submerged shelf in the near right corner. Every argument predates the
batch, so it is a `bitexact` row from the day it was added.

**`--cam-pitch 0` is what makes it work**, and the obvious submerged camera -- pointed down --
is blind by construction. The clamp fires only for a tile whose most *upward* corner ray is still
under water at the reach, which three blocks down is `atan(3/256)` = 0.67 degrees. So the clamped
region is everything at or below the horizon and nothing above it, and the frame has to be aimed
*at* the horizon rather than down at the floor. Measured by band, for the shipped 128 against an
unlimited reach:

| band of the frame | pixels moved | max delta |
|---|---|---|
| above y = 0.43 | **0** | 0 |
| 0.43 to 0.52 (`horizon`) | 5,032 | **23** |
| 0.54 to 0.74 (`deep`) | 62,589 | 2 |
| below y = 0.75 | **0** | 0 |

The camera was found by search rather than by hoping: ten seeds by eight yaws screened against a
reach-0 build, the best six carried to full resolution against a reach-128 build, then eight yaws
of the default seed refined. The default seed won on merit and on being the world every other
vantage lives in.

### The ladder, and the constant

Pixels of 921,600 against an **unlimited** reach at 1280x720; milliseconds paired 8 rounds,
alternated, against the 256 build at 1920x1080 with Hi-Z on.

| reach | pixels | max delta | `march` | `resolve` | **total** |
|---|---|---|---|---|---|
| 264 and up | **0** | 0 | -- | -- | -- |
| 256 (batch 48) | 316 | 1 | -- | -- | baseline |
| 224 | 1,640 | 3 | -0.081 | -0.084 | **-0.168 +/- 0.014** |
| 192 | 24,179 | 6 | -0.352 | -0.244 | **-0.604 +/- 0.041** |
| **128 (shipped)** | 67,621 | 23 | -0.815 | -1.100 | **-1.930 +/- 0.016** |
| 64 | 175,890 | 50 | -- | -- | unpriced |

**Blue's extinction was right.** The frame goes exact at 264 and the derived 1/255 bound is
`ln(255)/0.020` = 277 -- 5% apart -- so batch 48's authored 256 sat 8 blocks under exact and
threw away 316 pixels at one code value for the privilege.

**128 ships, decided by the user against the picture.** It is the first distance cut in this
engine bought at a *visible* price: batch 41's canonical row is 1.672 ms for 1,309 pixels at max
delta 1, and this is 51 times the pixels at 23 times the delta. What carried it is the size
against the frame -- 1.930 ms of a 10.6 ms `sea-horizon`, **18%**.

**What the seam is**, because the count is misleading on its own: 62,589 of the 67,621 pixels are
open water moving by at most 2. The damage is the other 5,032, where a distant shoreline's
*underwater* silhouette is deleted and the brighter open-water haze behind it shows through.

**What was not measured**: the seam sits at a fixed distance from the *camera*, so it slides
across the water as the player swims rather than staying with the shoreline. No still frame can
price that. If it reads as a moving band in play, 224 costs 1,640 pixels at max delta 3 and 192
keeps a third of the money.

### Two numbers nobody was looking at

**The control's own cost tripled when the constant moved.** `--no-water-far` switched on, paired
8 rounds, Hi-Z on:

| vantage | reach | `select` | `march` | `march2` | `resolve` | **total** | frame |
|---|---|---|---|---|---|---|---|
| `sea-horizon` | 256 | +0.001 | -0.039 | -0.011 | **-0.769** | **-0.835 +/- 0.006** | 11.44 |
| `sea-horizon` | **128** | -- | -0.859 | -0.021 | **-1.880** | **-2.776 +/- 0.020** | 11.57 |
| `underwater` | **128** | -- | -0.317 | -0.029 | **-0.765** | **-1.125 +/- 0.032** | 8.72 |

The `underwater` row confirms batch 48's own -1.135 for a 128-block reach to within 1%, on a
different session and a different binary.

**Batch 49's split held on a fresh vantage.** Of the 2.776 ms the clamp is worth at
`sea-horizon`, **1.880 is billed to `resolve` and 0.859 to `march`** -- 68/31, against the 91/9
batch 49 measured at `underwater`. The direction is the same and the reason is the same: a chunk
culled in `tile_select` saves the march *and* the writes, and the writes are billed to the pass
that reads them. **A march-side cull benched on `march`'s row alone hides two thirds of itself.**

**And `march` is outright the larger pass here**, 4.15 ms against `resolve`'s 3.74 at the shipped
reach -- the first vantage in the set where that is true. `underwater` was the previous record at
3.80 against 3.86.
---

## Batch 51: the gather's early-out, and the condition that decides it

**Not shipped.** Two conditions were built for the same arm and neither can be taken: one is fast
and damages the picture, the other is exact and fires almost nowhere. Both numbers are here so
the next attempt starts from them. The idea and the design that follows from them is roadmap P5.

**The premise, which survives both failures.** The nine-cell gather's whole output reaches the
pixel through `(ambient_rgb + sun_rgb) * ao`. Every term is proportional to the light it just
gathered, and `ao` multiplies that product and nothing else -- the `frame.ambient` floor sits
outside it, a batch-2 decision made for an unrelated reason. **On an unlit face the nine lookups
are already being multiplied by zero**, so removing them is not an approximation and needs no
look constant authored. Batch 47 priced the whole block at **-1.764 ms at `cave`**; this is the
part of that ceiling which costs no picture.

### The two conditions, against `voxelcraft-pre51.exe`

| condition | `cave`, `resolve` | rounds | pixels moved |
|---|---|---|---|
| light at the face's own air cell below `DIM_CUT_LIGHT` | **-1.579 ms** of 2.284 | 8/8, t = +272 | **10,150 at max delta 66** (`overcast`), 10,338 at 46 (`canopy`), 10,150 at 42 (`default`), 6,953 at 31 (`low-sun`) |
| the chunk's `light_flags` uniform bit, plus the span test | **-0.017, sign mixed** | 6 of 8 rounds, stopped early | **0 at all sixteen** |

**The first collects 89% of batch 47's ceiling.** At `cave` the whole frame goes 3.364 -> 1.787
ms with hi-z on, a 47% faster frame, and **every other pass moves by at most 0.001 ms** --
`select` +0.000, `march` -0.000, `build_hiz` +0.000, `taa` -0.001 -- which is batch 49's check
that a per-pass split is real and not a clock change lifting the whole frame.

### Why no threshold saves the first one

**The decisive measurement is that lowering the threshold four-fold changed nothing.** 0.0117 and
0.00139 -- level 2 and level 0 of the flood's own scale -- produced **identical file hashes at
all sixteen vantages**. So not one of those 10,150 pixels was a face at a middling light level,
and the constant was never the variable.

`light.rs` floods by decrementing one per step over the six face neighbours, so it bounds every
cell it can *reach*: a neighbour one level above the sampled cell, a diagonal two, which works
out at about a code value against the 0.08 ambient floor. **What it cannot bound is a diagonal whose two in-plane connectors are solid**,
where the light arrived from in front of the face and never through the plane. That is an inside
corner, voxel terrain is made of them, and it is the corner the AO rule shuts hardest -- so the
cut removes contact shading at exactly the corners roadmap A7 was declined for losing.

**A chunk-boundary guard was suspected first and priced at 132 pixels.** The 3x3 at a boundary
face is served by `world_sample`, which crosses into a chunk flooded independently; refusing
those faces took `default` from **10,150 to 10,018**, and `canopy` from 10,338 to 10,327. Real,
1.3% of the effect -- **and that build was never re-benched**, so every millisecond in this
section belongs to the unguarded one, which is also the build the pixel counts describe. The seam
itself is **roadmap R1's apron** -- it was entry P5b until the batch-56 reorganisation folded it
into the rung that makes it matter, because a trilinear probe tap interpolates across the seam
where this gather moves one cell of nine. A property of `light::flood` that no document recorded.

### Why the exact one is a no-op, and how that was nearly missed

**The six rounds read -0.020, -0.020, +0.020, -0.060, -0.010, -0.010** -- mean -0.017, sign
mixed, and the run was stopped before the fixture printed its own standard error, so that mean is the only
statistic here and it is not one `harness bench` vouched for. It does not need to be: **five
rounds negative, one positive, every magnitude at or near the reading's own 0.01 ms quantum**,
against the 1.579 ms the other condition moved at the same vantage.

Bit-exact at sixteen vantages is what an exact rewrite should read. It was also what a feature
that never runs reads. **Only the bench separated them**: a chunk is 64 blocks tall, so the chunk
holding a cave holds the lit surface above it, `chunk_uniform` is false, and the gate rejects.

This is `lessons.md`'s *a no-op claim cannot be told apart from a not-wired-up claim* met in the
wild, and the sharper form of it: **a capture cannot tell a correct early-out from one that never
fires, because both draw the identical picture.** The positive half has to be the clock.

### What the shapes cost in code, which predicted neither result

| build | `resolve` bytes | registers |
|---|---|---|
| parent | 554,496 | 96 |
| threshold, `SPEC_DIM_CUT=0` | 555,392 | 96 |
| threshold, `SPEC_DIM_CUT=1` | 565,120 | 96 |
| chunk flag, `SPEC_DIM_CUT=1` | 561,024 | 96 |

**The register count never moved, and the fast build was 10,624 bytes *larger* than the parent**
-- 9,728 of that against its own folded-out control. Batch 42 and
batch 45's pair restated a third time: bytes predict nothing in either direction.

### The control, which both builds had and which worked

`--no-dim-cut`, a bit in `SPEC_MASK` gating `SPEC_DIM_CUT`, was **16 vantages, 0 pixels, identical
hashes** against the parent binary in both builds. The gate is the override alone with
`!smooth_light` first in the `||`, so the three secondary copies of `shade_hit` short-circuit and
never compile the test in -- which is what keeps batch 45's fold intact. It went with the feature;
roadmap P5 says where to find it.

---

## Batch 53: `march` counted, and the pair list resized

**The first numbers in this file that a hot machine cannot move.** Everything above is
milliseconds, and the three caveats this file opens with are all about milliseconds. `harness
march` renders each vantage with `--march-stats`, which reads back the loop counter
`march_chunk` has been accumulating into `dbg` under `FLAG_HEATMAP` since the heatmap existed
and which nothing on the CPU had ever looked at. The output is **integers**: two runs an hour
apart, on a machine doing something else, agree to the unit.

**It adds no shader code.** A per-workgroup reduction would have meant giving `march` the
workgroup memory and barrier it does not have, and the pass being counted would not have been
the pass that ships. The flag's own `atomicAdd` is real work, so a `--march-stats` run is a
count and never a timing sample.

### Three ceilings, four minutes, no GPU timing

Each row is a **deliberately incorrect** build: one literal patched to a value that is simply
wrong, built, counted, thrown away. The pictures were nonsense and nobody looked at them.

| an incorrect skip | `deep-water` | `sea-horizon` | `default` |
|---|---|---|---|
| a uniform-water **leaf** steps 16 instead of 4 | **-68%** | **-57%** | 0% |
| a missing **root** cell steps 64 instead of 16 | 0% | -0.5% | **-62%** |
| the coalesced sub-leaf test steps 4 instead of 2 | 0% | -0.4% | -10% |

The third was dropped on that row alone. The other two are complementary -- one is water, one is
air -- and the batch built the exact half of each. **Price a traversal candidate this way before
opening a format question**: the third row cost twenty seconds and saved a design.

### What the two exact halves are worth, in steps

`harness march --exe-b`, 1280x720, whole set. Pair counts are identical on both sides of both,
which is what says these are traversal changes and not culls.

| | what it is | measured |
|---|---|---|
| the coalesced root test | the leaf level's 2x2x2 emptiness test lifted to the root, four lines | **-4.90% over 18 vantages**; -15.58% `low-sun`, -12.15% `default`, 0% `deep-water`, 0% `cave` |
| `dry_mask` | `root_mask` minus the water-only 16^3 cells, traversed by any ray that ignores water | **-25.17% `deep-water`, -16.64% `sea-horizon`**, -0.26% `underwater`, -1.40% `coastline` |

**Both are bit-exact, and each number above is why that is worth anything.** A pure traversal
optimisation is exactly what `bitexact` cannot tell from a feature that never fires -- the
reading batch 51 shipped nothing over -- so a zero here is only evidence when it is quoted
beside a step count that moved.

### The distribution, which contradicts every heatmap in the set

| vantage | steps | per ray | p50 | p90 | p99 | top 10% | past `MAX_ITERS` |
|---|---|---|---|---|---|---|---|
| `sea-horizon` | 19.4M | 5.8 | 17 | 47 | 73 | 28.6% | 18 |
| `deep-water` | 18.3M | 7.5 | 15 | 43 | 68 | 27.3% | 6 |
| `default` | 13.3M | 6.0 | 12 | 24 | 52 | 25.0% | 3 |
| `terraces` | 7.3M | 2.6 | 6 | 16 | 43 | 34.9% | 0 |
| `underwater` | 6.3M | 1.7 | 3 | 17 | 50 | 46.9% | 1 |
| `cave` | 3.7M | 4.0 | 4 | 5 | 7 | 16.3% | 1 |

Every heatmap puts the cost in a thin bright band of rays grazing water. Read before the batch,
the top 1% of pixels held **3.0 to 9.5%** of the steps and the top 10% held 16 to 47%; the
**median** pixel is the frame. *A picture of a distribution is not the distribution.* The last
column is an upper bound rather than a count -- `dbg` sums across every pair covering a pixel --
and it retires `MAX_ITERS` as a lever either way.

**Set-wide, the two traversal changes take the eighteen vantages from 196,470,005 DDA steps to
176,826,842: -10.0%.**

### The module, before and after

| pass | words | bytes | registers |
|---|---|---|---|
| `tile_select` | 3,422 -> 3,434 | 8,832 -> 8,832 | 35 -> 35 |
| `march` | 10,169 -> 10,395 | 21,504 -> 21,632 | **72 -> 72** |
| `resolve` | 28,484 -> 28,710 | 554,496 -> 566,016 | **96 -> 96** |

**No register moved**, and that reading also retired the first hypothesis anyone has about this
loop -- that `cell[axis]`'s dynamic vector index forces the DDA's state into scratch. `march`
reports no spill, no stack and no local memory on both sides. `resolve` grew 11,520 bytes
because it inlines `march_chunk` through its three water-ignoring legs, which is the same reason
it stands to gain from `dry_mask` with no line of its own changing.

### The milliseconds, seven paired benches, hi-z on

**The null pair is the first row for a reason, and this batch has two of them.** Run under load,
the same build against itself read **-0.085 +/- 0.040** on the hi-z-off total; run on an idle
machine twenty minutes later it read **+0.002 +/- 0.007**. Same binary, same vantage, same
command. **Pairing and alternation cancelled the contention in both cases -- what load cost was
resolution, a factor of five of it**, which is `pitfalls.md`'s *bench late in a long session*
demonstrated on demand rather than inferred. Quote the floor, not the word: on the idle run
3 se is about **0.02 ms**, and nothing below that is claimed here.

Savings are negative. A = this build, B = the named partner, 1920x1080, 6 paired rounds,
order alternating.

| vantage | partner | `march` | `resolve` | total | of a frame |
|---|---|---|---|---|---|
| `deep-water` | **null (itself)** | -0.003 +/- 0.002 | +0.007 +/- 0.004 | **+0.002 +/- 0.007** | -- |
| `deep-water` | `pre53` (whole batch) | **-1.170 +/- 0.030** | **-0.708 +/- 0.023** | **-1.903 +/- 0.058** | 9.667 -> **19.7%** |
| `deep-water` | `pre53-noroot` (traversal only) | -0.905 +/- 0.025 | -0.433 +/- 0.019 | **-1.337 +/- 0.049** | |
| `deep-water` | `--no-water-dark` (the rung only) | -0.278 +/- 0.020 | -0.215 +/- 0.016 | **-0.513 +/- 0.038** | |
| `sea-horizon` | `pre53` | **-0.400 +/- 0.018** | **-0.133 +/- 0.015** | **-0.540 +/- 0.035** | 9.218 -> 5.9% |
| `coastline` | `pre53` | +0.012 +/- 0.004 | **-0.503 +/- 0.021** | **-0.497 +/- 0.024** | 11.095 -> 4.5% |
| `default` | `pre53` | **-0.172 +/- 0.031** | -0.057 +/- 0.070 | -0.243 +/- 0.112 | 10.392 -> 2.3% |
| `underwater` | `pre53` | -0.077 +/- 0.036 | +0.005 +/- 0.030 | -0.078 +/- 0.076 | not separable |

**The two halves add up, which is the internal check.** 1.337 traversal plus 0.513 rung is
1.850 against a measured 1.903 for both together -- the 0.053 is the rung culling chunks the
traversal changes would otherwise have walked cheaply, and it is the right sign for that.

**`coastline` is the finding, and it was the one claim this batch had left hedged.** The camera
sits 24 blocks *above* the sea, so its primary ray sees water as geometry and `march` gains
nothing at all -- it is **0.012 ms slower**, which is the new code costing slightly more than the
1.4% of steps the root skip saves there. And `resolve` is **half a millisecond faster**.

**That is not batch 49's billing effect and it should not be read as one.** Batch 49 measured
`march`'s fire-and-forget `atomicMax` traffic landing on `resolve`'s clock; nothing here changes
a write. This is simpler and was sitting in plain sight: **`resolve` calls `march_chunk` itself**,
through the refracted, reflected and shadow legs, and every one of them passes
`skip_water = true`. `dry_mask` is *their* mask as much as the submerged primary ray's. A
traversal change in this engine has two customers and only one of them is the pass it lives in.

**`underwater` reads nothing and the count said so first.** -0.26% of its DDA steps, -0.078 ms
at 1 se of 0.076. Its rays meet the sea floor within tens of blocks, so there is no ocean
interior to skip; `deep-water` exists because `underwater` cannot see this.

### The pair list, which truncated two rungs below where two documents said

Counted at `default`, against the old fixed cap of 1,048,576:

| resolution | tiles | select + deferred | share of the old cap |
|---|---|---|---|
| 1280x720 | 14,400 | 150,667 | 14% |
| 1920x1080 | 32,400 | 333,692 | 32% |
| 3840x2160 | 129,600 | 1,313,456 | **80.5%** |
| 5120x2880 | 230,400 | 2,325,513 | **145.9% -- truncating** |

The roadmap and `gpu.md` both said "reachable at 8K". A 4K capture was one busier camera or one
`--scale 2` from losing geometry. The list is screen-sized and is now sized from the screen like
`vis`, `hiz` and `dbg`, with `MAX_PAIRS` kept as the **floor** so 1280x720 and 1920x1080
allocate exactly what every number in this file was measured at. 7680x4320 now renders with no
warning of either kind.

**Behind it sat a narrower limit that had no guard at all.** The pair word spends 19 bits on the
tile index, so a screen holds 524,288 tiles; 7680x4320 is **518,400, or 98.9%**. Past it the
shift drops the high bits and a tile marches another tile's chunks -- a frame that is lit,
shaded, temporally stable and wrong. `Renderer::warn_if_oversized` is the guard, thirty-one
batches after the field.

## Batch 54: Snell's window, and the four milliseconds that were not needed

**The feature costs +0.262 ms and the first build of it cost +6.49.** The gap is one traced ray
that turned out to be worth a max channel delta of 3, and the whole of this section is about how
that was found rather than about the feature.

Savings negative, hi-z on, 1920x1080, 5 paired rounds against `voxelcraft-pre54.exe`.

| vantage | what it is | `resolve` | total |
|---|---|---|---|
| `coastline` | dry -- the term never runs | +0.044 +/- 0.087 | +0.054 +/- 0.109 |
| `terraces` | dry, and the most expensive `resolve` in the set | +0.042 +/- 0.057 | +0.044 +/- 0.073 |
| `underwater` | submerged, but at the ramp's zero -- reached and returns | -0.002 +/- 0.015 | -0.000 +/- 0.036 |
| `deep-water` | the only camera that runs it | **-0.262 +/- 0.049** | **-0.262 +/- 0.111** |

**The two dry rows are the answer to batch 12's standing question and the answer is no.**
`resolve` gained **15,232 bytes** and the fifteen cameras that never reach the term paid nothing
they could distinguish from zero. Code size costs time in this pass at *some* scale -- batch 12
found 20 KB doing it -- and 15 KB of a cold branch does not.

### The first build, and the three diagnostic builds that took it apart

`resolve` at `deep-water` went **3.54 -> 10.03 ms**, and the frame 8.75 -> 15.25. That is more
than three times what batch 53 *bought* at the same vantage, so it was not shippable and the
question was which third to remove.

| build | what it removes | `resolve` delta |
|---|---|---|
| full feature | -- | **+6.49** |
| the mirrored hit is not shaded | `shade_hit` | +3.62 |
| the wave normal is flat | `wave_field` | +5.37 |

which separates as **wave normal 1.12, trace 2.50, shading the hit 2.87**. All three are paid on
every pixel whose view ray crosses the surface -- at a level camera 24 blocks down, the whole
upper half of the frame.

### Then the measurement that removed two of the three

**What does the traced mirror actually show, against one that is simply `underwater_body()`?**

| pitch | pixels differing | max delta |
|---|---|---|
| 0 | 227,842 | **3** |
| +20 | 386,082 | **3** |
| +45 | 474,544 | **7** |

**Nothing.** A mirrored ray at these angles travels a long way through water and `water_medium`
converges it to the body colour on the way back -- blue's transmittance over 275 blocks is
0.004 -- so **the free answer is the limit the expensive one was walking towards.** The trace and
its shading went; the feature kept its picture to within a delta of 7 and lost 4 ms.

**A distance fade was tried first and is the lesser half.** The refracted leg's own
`WATER_REFRACT_NEAR`/`FAR` ramp, applied to the crossing distance, recovers **1.34 ms** -- because
`t_surf` is `depth / sin(elevation)` and only the very grazing rays fade out, while the band that
actually pays is 7 to 21 degrees. It stayed, in front of the wave normal, and it is not what
made this affordable.

**The general form, which is batch 51's lesson from the other side.** That batch said: before
pricing a way to make an expensive block cheaper, check whether there is a case where its result
is *discarded*. This one adds: check whether something already computes what it *converges to*.
Both are the same question -- does the expensive thing change the answer -- asked at opposite ends
of a ramp.

## Batch 55: the probe tap, and a cost that does not scale with it

**Roadmap L1 wants a directional coloured irradiance field baked at chunk build and read once per
shaded surface, and its stated premise-risk was the read side**: `resolve` is instruction-bound at
96 registers, and batch 47 measured the nine-cell gather inside it as instruction-bound too and
recovered only 10-13% of its ceiling by giving the memory system a perfect case. The honest prior
was that anything added there costs.

**The field was 128^3 when everything below was measured.** Batch 57 made it 64 x 384 x 64 --
six stacked slabs of the ambient cube -- and gave `--probe-tap` an implied `--probe-fill 1.0` so
the diagnostic still samples a constant. The numbers here are the ones that were read; re-measure
before quoting them against a later build.

**`--probe-tap` prices exactly one thing**: one hardware trilinear tap into a 128^3 RGBA16F field
per shaded surface, `amb = amb * probe_tap(hit)`. The field holds a constant 1.0, so the multiply
is bit-exact for every finite `amb`, the picture cannot move, and the entire output is a
millisecond. No bake, no format decision, no picture.

### The reading

`resolve` at 1920x1080. **`harness bench` prints `B - A`** (`bench.rs`, `d = y - x`), so with A the
`--probe-tap` side a **negative** number is what the tap **costs**:

| taps | fill | `default` | `coastline` | `terraces` | `cave` | `sky` |
|---|---|---|---|---|---|---|
| 1 | 1.0 | **-0.055** +/- 0.027 | **-0.010** +/- 0.000 | +0.017 +/- 0.002 | **-0.040** +/- 0.000 | **-0.020** +/- 0.000 |
| 1 | noise | -- | -- | +0.019 +/- 0.001 | **-0.040** +/- 0.000 | -- |
| 3 | 1.0 | -- | -- | +0.048 +/- 0.048 | **-0.040** +/- 0.000 | -- |

**Every figure is inside 0.06 ms of a 3.3 to 9.9 ms pass**, and three of the rows above say
something the headline does not.

**Three taps cost exactly what one tap costs.** `cave` reads **-0.040 +/- 0.000** at both, with
every one of eight rounds returning the identical delta -- and `cave` is the most surface-dense
frame in the set, the vantage where batch 47 measured the gather at **72% of the pass**. Tripling
the taps moved it by nothing. **So the cost is not the tap**; `shaderstats` says where it is
instead: `resolve` is **582,400 bytes with the arm compiled out, 583,680 with one tap and 584,960
with three**, at **96 registers in all three**. The code exists; the work does not scale.

**`sky` is the row that proves the same thing from the other end.** A tap taken once per *shaded
surface* can do almost no work in a frame that has almost none, and `sky` still reads -0.020 --
half of `cave`'s, in a frame with a fraction of the surfaces. **A cost that is present where there
is no work and flat when the work triples is code existing**, which is batch 12's mechanism and
batch 45's rule read from the other end: that batch said *a regression largest where the feature is
least used cannot be the work*, and this one adds *a cost that does not move when the work triples
is the same statement*.

**`terraces` is the one vantage that reads positive, and its own row says how much to trust it.**
At one tap it is +0.017 +/- 0.002; at three it is **+0.048 +/- 0.048**, a standard error
**twenty-four times larger** for the same quantity. That is the laptop, not the shader: the
three-tap pair ran about ninety minutes into continuous sweeping, which is the regime batch 50
documented at **+19% over seven rounds with the standard error at 0.021 against 0.004-0.008 two
hours earlier**. Pairing and alternation keep the verdict honest and what is lost is resolution --
so `terraces` at three taps is quoted as *indistinguishable from zero* and nothing is built on it.
**`cave`'s row is the trustworthy one at both settings**, because a standard error of zero means
every round returned the same integer of the timer's own quantization.

**And the constant fill is not being compressed into a cost nobody would pay.** A field filled per
texel from a hash reads **+0.019 against +0.017** at `terraces` and **identical** at `cave`. That
was the one hole a bit-exact measurement leaves open -- a texture whose every texel agrees is
exactly what memory compression is best at -- and it is closed.

### What L1 gets from this

**The read side of a probe field is affordable, and it is affordable in the way that matters.** Not
"one tap is cheap", which would have left the format question open, but **the cost does not scale
with the number of coefficients**: a 6-axis ambient cube or an SH L1 field, which needs three taps,
reads for what a DC-only field reads. The pass is instruction-bound, the texture unit is a
different pipeline, and a tap that adds no register pressure hides in the shadow of ALU work that
is already there.

**What is still unpriced is most of L1**, and the next session should not quote the table above as
though it covered any of it:

- **The saving side is not measured at all**, and it is the half that makes the architecture worth
  building: a pre-composed field deletes `light_curve`'s eighteen evaluations, the four per-corner
  `bk/(sk+bk)` divisions, the `mix(SKY_TINT, BLOCK_TINT, ..)`, `face_shade` and `floor_rgb`. That
  cannot be priced until the format is chosen. **Price the removal before designing the bake**,
  which is batch 40's rule.
- **Nothing about the bake.** Worker microseconds per chunk, VRAM per chunk and streaming latency
  are the units there, and none of them is a frame millisecond.
- **Nothing about the picture.** No metric in this fixture ranks *looks better*.

### Three readings, because bit-exact cannot speak for itself

A sweep whose pass condition is *0 pixels differing* cannot tell a free tap from an absent one --
`lessons.md` has it as *a no-op claim cannot be told apart from a not-wired-up claim*, and this is
the batch where that hazard is the design rather than a footnote.

| reading | source | says |
|---|---|---|
| the frame moves at `--probe-fill 0.5` | the fixture | the tap is **read** |
| `resolve` +1280 bytes per compiled arm, 96 registers throughout | `shaderstats` | the tap is **compiled in**, at no register cost |
| 18 vantages, 0 pixels, identical hashes | `bitexact` | the tap is **invisible** at 1.0 |

`bitexact` also ran against `voxelcraft-pre55.exe` with the tap off: **18 vantages, 0 pixels,
identical hashes**, so the shipping build is unchanged by the batch that added the diagnostic.
`tests/probe.rs` holds the two claims neither the fixture nor the compiler can make -- that the
gate is the override alone, and that the combiner is a multiply.

### The adapter, asked rather than assumed

`--bin probe` gained four feature rows and a 3D limit, because `CLAUDE.md` had been carrying *"wgpu
30 does expose ray tracing"* as an unchecked claim for four batches and this batch was about to add
a second one. On the RTX 3050 Laptop, driver 610.88:

| feature | |
|---|---|
| `EXPERIMENTAL_RAY_QUERY` | **true** |
| `EXPERIMENTAL_RAY_HIT_VERTEX_RETURN` | **true** |
| `EXPERIMENTAL_RAY_TRACING_PIPELINES` | **true** |
| `EXPERIMENTAL_COOPERATIVE_MATRIX` | **true** |
| `max_texture_dimension_3d` | 16384 |

**The cooperative-matrix row is the one that was a belief.** This session's own design argument
said the tensor cores were probably unreachable because wgpu likely did not expose the feature;
the user asked for the line rather than the belief, and the belief was wrong. **What is confirmed
is that the adapter reports it** -- whether naga's WGSL front end can express the operations is a
further question and is not answered here. `CLAUDE.md`'s scarcity table is updated accordingly:
tensor cores are idle *and reachable*, which is a different entry from idle and shut.

## Batch 56: the removal priced, and L1's read side is net negative

**Batch 55 priced what a probe tap adds and found the cost does not scale with the coefficient
count. That is half a decision.** The architecture only pays if what a pre-composed field
*removes* is larger than what the tap costs, and nothing had measured the removal -- roadmap L1
said so in its own text, and `lessons.md` has it as batch 40's rule: **price the removal before
designing the bake.**

**`--probe-ambient` is that build, and it is deliberately incorrect.** The field replaces
`shade_hit`'s ambient instead of multiplying into it, and the terms a real directional field
would make redundant leave the module. The field holds a constant, so the frame is wrong and is
thrown away -- batch 53's method, one flag instead of one patched literal. **It implies
`--probe-tap`**, so every figure below is the **net**: the tap's cost is already inside it.

### What leaves, and what stays

| leaves | stays |
|---|---|
| nine `blk_n` `light_curve` evaluations | the nine gather lookups |
| four per-corner `bk/(sk+bk)` divisions and their `mix(SKY_TINT, BLOCK_TINT, ..)` | `sky_n`, and its nine `light_curve` |
| the `amb` bilinear | the AO rule and its bilinear |
| `face_shade`, the per-normal constant standing in for directional occlusion | `sky_sm`, which gates the sun |
| the ambient floor, whose own comment calls it the stand-in for bounce | |

**The two columns are a claim about spatial frequency, not about cost.** AO is a contact cue and
`sky_sm` gates a hard shadow; both are high-frequency and a coarse probe cannot carry them. Every
term in the left column is low-frequency or a constant, which is exactly what a probe field is
for.

### The reading

`resolve` at 1920x1080. `harness bench` prints `B - A` with A the `--probe-ambient` side, so a
**positive** number is what the architecture **saves**:

| | `default` | `coastline` | `terraces` | `cave` | `sky` |
|---|---|---|---|---|---|
| **batch 56, net saving** | **+0.091** +/- 0.011 | +0.033 +/- 0.085 | -0.044 +/- 0.047 | **+0.130** +/- 0.000 | **+0.030** +/- 0.000 |
| rounds positive | 8/8 | 4/8 | 2/8 | 8/8 | 8/8 |
| batch 55, tap cost alone | -0.055 | -0.010 | +0.017 | -0.040 | -0.020 |

**Three of five are real and all three are positive; two are indistinguishable from zero.** The
saving is **0.03 to 0.13 ms of a 3.3 to 9.9 ms pass**, net of the tap.

**And unlike batch 55's cost, it tracks the work.** `cave` is the largest at +0.130 and is the
most surface-dense frame in the set, the vantage where batch 47 measured the gather at **72% of
the pass**; `default` is next at +0.091; `sky`, with almost no shaded surfaces, is the smallest
real row at +0.030 and is the code-size floor reading in the other direction -- `shaderstats`
puts `resolve` at **582,400 bytes shipping, 583,680 with the tap and 575,360 with the removal**,
**96 registers in all three**. **That ordering is the mechanism check passing**: batch 55's cost
did *not* grow with surface density and was therefore code; this saving does, and is therefore
work.

**`terraces` is the one negative and it is the session's noisy vantage.** -0.044 +/- 0.047, 2/8
rounds, indistinguishable from zero -- and it is the same vantage that read +0.017 +/- 0.002 early
in batch 55 and +0.048 +/- **0.048** ninety minutes later. Nothing is built on it in either batch.

### What this settles, and what it does not

**Settled: L1's read side is net negative cost.** A directional coloured field is not a tax on
`resolve` to be paid out of the appearance budget -- it **buys** 0.03 to 0.13 ms while replacing
two constants with something derived. Taken with batch 55's finding that the cost does not scale
with coefficient count, the read side of the architecture is no longer a risk to the design and
does not constrain the format.

**Not settled, and none of it is small:**

- **The field is a constant, not irradiance.** The *addresses* are what a real field would
  produce and the read cost transfers; what does not transfer is the assumption that one tap can
  carry sky-indirect, block light and bounce together. If the format needs the sun's own gate or
  a second lookup for block light, part of the left column comes back.
- **The bake is entirely unpriced.** Worker microseconds per chunk, VRAM per chunk, streaming
  latency -- none of them a frame millisecond, and all of them the actual work of L1.
- **The picture is not ranked at all.** This build's frame is wrong on purpose. Nothing here says
  a directional field *looks* better, and no metric in this fixture could; that is batch 54's
  instrument -- playing it.

**The honest one-line summary for the next session**: *the read side costs nothing and probably
pays; go and build the bake, and price it in worker microseconds.*

## World generation, per chunk (64^3, single thread, 96 chunks)

| stage | time |
|---|---|
| worldgen (noise, caves, trees) | 3540 us |
| tree build (dense to 4^3 tree) | 1553 us |
| lighting (sky + block flood fill) | 2312 us |
| **intern + upload** | **84 us** |
| total | 7490 us |

**Only the intern step runs on the main thread**; the rest is on rayon workers. 904 chunks stream
in about 0.4 s across 16 threads.

An earlier session measured the same command at 1624 / 726 / 1065 / 45 us -- a uniform ~2x on the
worker stages and nothing on the main thread, which reads as the power state rather than a code
change. **The ratios and the main-thread number are what carry over; the absolutes are not.**

Two fixes during development cut this from 7601 us at the time:

- **Worldgen 4441 to 1624 us.** Gravel was calling 3D noise per solid voxel (~110k calls per
  chunk). It now samples the same 17^3 coarse lattice as the caves and interpolates.
- **Lighting 3075 to 1065 us.** Two early-outs skip the flood fill entirely: a chunk wholly above
  the terrain is uniformly lit, and a chunk with no sky access and no emitters is uniformly dark.
  That covers most of a world.

---

## Memory

Measured over 96 chunks holding 10.6M solid voxels.

| | bytes | per solid voxel |
|---|---|---|
| geometry (leaf masks + inner nodes) | 1.3 MB | **0.130** |
| attributes + light (bricks) | 4.2 MB | 0.416 |

**Geometry beats the reference targets**, which quote ~0.19 bytes/voxel for 64-trees against
ESVO's ~0.57. The 0.130 comes from the **full-subtree flag**, not from interning: a solid 16^3 or
64^3 region stores no child run at all, and underground chunks are mostly solid.

**Attributes cost 3x what geometry costs**, which is the split Aokana reports for its
Minecraft-derived scene. Palette-compressing each leaf -- 1/2/4/8 bits against a per-leaf palette,
with an inline uniform entry for single-block leaves -- keeps it to 0.416 rather than the 2
bytes/voxel a raw `u16` array would need.

---

## Results that did not pay off

**Kept deliberately. A file that only records wins is a file that invites re-deriving the losses.**

**Run deduplication measures 1.00x** -- 2881 leaf runs and 66 inner runs live, none shared. The
reference document warned that wide nodes have high entropy and dedupe poorly, and on procedural
terrain no two surface chunks share a run. What actually compresses the world is the
uniform/full-subtree path, which is free. The interning machinery is still correct and
load-bearing for repetitive content -- `identical_chunks_share_runs` in `tests/tree.rs` proves two
identical chunks share every run -- it just does not help *this* worldgen. **This is expected; do
not "fix" it.**

**Ancestor memoisation failed to pay.** Three levels is not deep enough for it to matter.

**Occupancy work has twice bought nothing**, and **16x8 / 16x16 workgroup shapes were built,
measured and thrown away**. Both are in [`docs/errors.md`](docs/errors.md) with their numbers,
because they are the kind of thing a session will otherwise try again.

**Batch 38's leaf cutout has a size, not a time.** `shaderstats` folds `march` 21,504 -> 17,536
bytes and `resolve` 291,200 -> 269,440 between `SPEC_LEAF_CUTOUT` 1 and 0, registers 72 -> 58 and
80 -> 72 -- so the *control* is free by the fold, and 14 vantages came back bit-exact against
`voxelcraft-pre38.exe`. **What the batch did not do is bench the feature itself.** The session
spent its measurement budget establishing where the override may sit -- three shapes tried and
failed, recorded in [`docs/errors.md`](docs/errors.md) -- and a paired `bench` at `default` and at
a forested vantage is the number still missing. **Do not quote a cost for this feature until it
exists**; a folded size is not a millisecond, which is the whole argument this file makes.

**The world shadow ray for terrain crepuscular rays measured +10.57 ms** of `resolve` against a
frame that costs 4, and its cheapest useful corner finds no occluder at all. It is priced in
[`docs/errors.md`](docs/errors.md), and it stays here because it is still the wrong shape:
**batch 35 shipped the terrain shafts without it**, by projecting the heightfield along the sun
once per frame instead of searching along it once per sample. The sweep's own table did not
survive this file's consolidation and lives at `1f6c4a6:PERF.md`, which a comment in
`common.wgsl` went on pointing here for eight batches.

---

## Where the rest of this file went

Until this pass PERF.md was **2,813 lines**: a section per batch, each holding that batch's sweeps,
its A/B tables and its own re-derivations, accumulated across twenty-six batches on a machine whose
absolute numbers drift by 1.8x between sessions. Most of it could not be compared with any other
part of it.

What survived is above. The rest went where it is actually reachable: **per-feature reasoning to
the subsystem documents, rejected approaches and their numbers to
[`docs/errors.md`](docs/errors.md), and how to measure to
[`docs/harness.md`](docs/harness.md)**.


================================================================================
