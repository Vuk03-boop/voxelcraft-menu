> **Latest handoff:** Read [BATCH-LOG.md](BATCH-LOG.md) first. It records the decoupled-shadow overhaul, changed controls, superseded behavior, and unresolved work. Earlier validation/performance notes below describe their original batches, not acceptance of the latest code.

# voxelcraft

A Minecraft-style voxel engine in Rust that renders by **ray tracing sparse trees on the
GPU** — no meshing, no triangles anywhere in the pipeline.

Built from the techniques catalogued in [`docs/reference.md`](docs/reference.md),
taking its "aggressive" frontier path (§4.1 + §4.2) and structuring the frame after
[Aokana](https://arxiv.org/html/2505.02017v1) (Fang, Wang & Wang, PACMCGIT 2025).

Real-time at 1080p on an RTX 3050 Laptop — sun shadows, smooth per-corner lighting,
temporal anti-aliasing, water that reflects and refracts, six biomes, a cloud deck,
crepuscular rays, per-biome tint and animated waves. The cost here is shading rather than
traversal, which is why a frame rate is meaningless without the vantage attached to it.
[`PERF.md`](PERF.md) is the only place that quotes a measurement, and every number in it
carries the vantage it was taken at.

![Landscape](screenshots/landscape.png)

![Biomes](screenshots/biomes.png)

![Water](screenshots/water.png)

| | |
|---|---|
| ![Building](screenshots/building.png) | ![Night](screenshots/night.png) |
| Structures built through the edit path | Glowstone lighting at night |

<!-- Regenerate these six with (each accumulates 32 frames and captures the last):
     --screenshot screenshots/landscape.png --width 1280 --height 720 --hud --time 0.38 --cam-height 55
     --screenshot screenshots/biomes.png    --width 1280 --height 720 --time 0.30 --cam-height 240 --cam-pitch -20 --cam-yaw 40
     --screenshot screenshots/water.png     --width 1280 --height 720 --time 0.74 --cam-height 6 --cam-pitch -3 --cam-yaw 143
     --screenshot screenshots/building.png  --width 1280 --height 720 --hud --demo-edits --cam-height 12 --cam-pitch -24
     --screenshot screenshots/night.png     --width 1024 --height 576 --demo-edits --time 0.92 --cam-height 12 --cam-pitch -24
     --screenshot screenshots/cave.png      --width 1024 --height 576 --demo-edits --cam-height -1                        -->

## Current surface-repair update

The lighting/water repair layer is enabled by default: tighter soft shadows, stronger
sun-facing contrast, non-sun-shadow-casting glowstone, water-interface protection and
safer water sample sharing. Main wave functions are preserved. See
[SURFACE-REPAIRS.md](SURFACE-REPAIRS.md) for controls, validation limits and the native
shoreline acceptance checklist. These repairs have no measured target-GPU performance claim.

## Running it

Needs a GPU with 64-bit shader atomics (`SHADER_INT64` + `SHADER_INT64_ATOMIC_MIN_MAX`) —
any modern discrete card. The engine refuses to start otherwise rather than silently
falling back.

```bash
cargo run --release
```

Controls: **WASD** move, mouse look, **Space/Shift** up-down, **F** toggle fly, **Ctrl**
sprint, **LMB** break, **RMB** place, **1-0** or scroll to pick a block, **F3** stats,
**F4** render scale, **F5** iteration heatmap, **F6** Hi-Z culling, **F7** temporal
anti-aliasing, **Esc** pause/resume. Choose **Play** on the title screen to begin. In water,
**Space** swims up and **Shift** dives.

The menu has **Display, Graphics, Appearance, Render lab and Effects lab** pages with Apply,
Cancel and Restore Defaults. Gameplay and weather pause while menus are open.
Five shader experiments are **off by default and title-only**; they lock after
Play. Compact shade_hit and the soft-shadow switch are unmeasured performance A/Bs;
the glass-reflection/water-look/wind switches expose existing experimental paths,
not new fixes. The dedicated primary-shadow pass is no longer an experiment: it is
mandatory and always on, and it isolates glass and water with it (resolve timing
includes the whole family). `--no-isolate-glass` reverts the glass split;
`--no-shadow-pass` is retired — it warns and leaves the dedicated mask on.
See [MENU-NOTES.md](MENU-NOTES.md) for usage and [EXPERIMENTS.md](EXPERIMENTS.md)
for the architecture, measured correctness checks and native-validation limits.
F4/F7 share the applied settings state; hotkey changes are session-only until you
choose **Apply and save** in Settings.

Headless modes, all useful without a display:

```bash
cargo run --release -- --screenshot shot.png --width 1920 --height 1080 --hud
```

```bash
cargo run --release -- --bench-frames 140 --width 1920 --height 1080
```

```bash
cargo run --release -- --bench-terrain 96
```

```bash
cargo run --release --bin harness -- capture
```

The last one is the measurement fixture. It holds the named vantages the documentation
quotes, the crops scored at each, and the metrics, so a capture taken today is comparable to
one taken six months ago. `harness list` prints the set and `harness --help` the subcommands.

> **Repository caveat:** the `harness`, `probe` and `shaderstats` drivers (`src/bin/`)
> are documented but **absent from version control** — the export pipeline behind the
> initial commit pruned every directory named `bin`. Restore them from the original
> sources before running the commands above; `tests/bins.rs` fails loudly until then.

**How to measure so the result means anything** -- why a bench has to be paired and
order-alternating, what each metric is sensitive to, and how the converged reference reports
its own noise floor -- is [`docs/harness.md`](docs/harness.md). It is not optional reading:
the obvious way to arrange an A/B charges one side the machine's warm-up ramp and manufactures
a tenth of a millisecond that is not there.

## Flags

**Each one is the A/B partner for a single feature** — most reproduce the
build from before that feature shipped, bit for bit, so a regression can be pinned rather
than merely described. `--no-water`, `--no-biomes`, `--no-waves`, `--no-taa`, `--no-fade`,
`--cloud-cover 0` and `--no-tint` are the ones reached for most.

```bash
cargo run --release -- --help
```

`--help` lists every flag with a one-line description. The table in
[`CLAUDE.md`](CLAUDE.md) is the one to read when it matters what a control *costs* and which
other controls it clears — a control that costs a millisecond quietly taxes every measurement
taken with it.

## How it renders

A chunk is 64³ voxels held in a 3-level 4³-branching tree — the wide-node structure the
reference document recommends over octrees, which benchmark *worse* than plain hierarchical
grids. Each frame runs seven compute passes: the **light envelope** (a prefix scan that turns terrain
occlusion into a heightfield lookup), **tile selection** (screen in 8×8 tiles, each
testing chunk bounding boxes rather than trees), **ray march** (one workgroup per surviving
pair, into a 64-bit visibility buffer), **Hi-Z build**, **recovery** (which is what makes the
Hi-Z lossless under fast camera motion), **resolve** (shading, paid per visible pixel rather
than per traced pixel), and **temporal resolve**.

Geometry and attributes are **decoupled**, which is the single most important structural
decision here: sparse voxel DAGs deduplicate geometry well and attributes badly. Leaves are
bare 64-bit occupancy masks; block ids live in a separate table, palette-compressed per leaf;
light lives in 4³ bricks, because it occupies air voxels that have no leaf index at all. Edits
are bottom-up path updates — rewrite the leaf, intern it, rebuild the parent run, rebuild the
root. Nothing is remeshed, because there is no mesh.

**The passes, the invariants that hold them together and the reasoning behind every constant
are in [`CLAUDE.md`](CLAUDE.md) and the subsystem documents it indexes** —
[`water`](docs/water.md), [`sky`](docs/sky.md), [`shading`](docs/shading.md),
[`terrain`](docs/terrain.md), [`foliage`](docs/foliage.md), [`lod`](docs/lod.md),
[`temporal`](docs/temporal.md) and [`gpu`](docs/gpu.md). This section is a map, not a second
copy of them.

## What it does and doesn't do

Terrain with caves, trees and six biomes, infinite in X and Z, 512 tall. Streaming with LOD out past
2500 blocks. Walking, jumping, flying, block-accurate collision. Break and place from a
10-block hotbar. Procedurally generated textures (no Minecraft assets anywhere). Sky and
block-light flood fill, sun shadows, smooth per-corner light and AO, a day/night cycle,
exponential height fog with a sun-scattering lobe, temporal anti-aliasing, dissolved rather
than popped LOD transitions, oceans with traced reflection, refraction, depth absorption and
swimming, a temperature/humidity biome field with per-biome terrain, snow and tree lines, and
a drifting cloud deck that reflects in the water, shadows the ground beneath it and takes the
sun's glow out of the air it stands in front of.

Not implemented: inventory, crafting, mobs, multiplayer. Crepuscular rays are cast by the
cloud deck and, since batch 35, by the terrain — a ridge with a low sun behind it takes the
glow out of the haze on its dark face. It does that by reading a light-envelope heightfield
rather than by marching, because the honest march measured ten times what the rest of the
feature costs; what the heightfield cannot cast are overhangs, tree canopies and player-built
structures. Water does not flow, and its surface
is geometrically flat at a fixed sea level: the waves shipped in batch 16 are **micro-normals**
that perturb only the terms which never trace, so both traced rays keep the flat normal and a
placed block of water just sits there. **Leaves and glass stopped being opaque cubes in batches
38 and 38b** -- a canopy is a 4^3 sub-voxel mask at LOD 0 and a pane transmits under a Fresnel
weight -- but a pane's reflection is the sky and never a traced ray, glass is invisible to every
secondary ray including its own, and the coarse levels still stamp leaves as solid cubes. Apart
from the canopy's mask and foliage's cross-quad there is no primitive other than the cube. The roadmap in
[`docs/roadmap.md`](docs/roadmap.md) covers what is scheduled and what is only listed, and
`docs/` holds the reasoning behind each shipped batch's constants.

## Tests

```bash
cargo test --release
```

Covering the voxel core (dense↔tree round-trips, run sharing, reference-count
release, attribute recovery, 2000 randomised edits against a dense mirror), worldgen
invariants (caves are carved, slopes stay walkable across biome borders, coarse LODs track the
same surface, and every biome grows trees on its **own** surface block rather than on grass),
biomes (`--no-biomes` reproduces the pre-biome world bit for bit, the field still resolves at
stride 8, the palette snaps where the shape blends, and the surface a column gets agrees at
every LOD),
ground cover (`--no-foliage` and `--no-meadow` each reproduce the world before their batch bit
for bit, and the coarse ground wears its stand-in colour at the rate the fine ground wears
tufts, per biome),
water (basins flood to sea level, `--no-water` leaves the identical terrain dry, `--no-waves`
flattens the surface back to glass, `--no-wave-aniso` restores the band limit that could not
see the grazing angle, `--no-wave-shoal` restores the swell that ignored how deep the water
was, `--no-wave-fill` halves the number of wave octaves back to four, coarse
levels cover the same area of sea, sky light dims a level per block of depth),
LOD streaming (every point is drawn exactly once even mid-cross-fade, transitions never
step a whole level in one frame, a stationary camera streams nothing),
camera algebra (a ray reconstructed and reprojected lands back on its own pixel, on its
*centre* for every jitter phase, and the eight phases cover the pixel and repeat),
player physics (landing, walls, ceilings, no tunnelling at 90 blocks/second, and swimming —
entering the water arrests a fall, sinking is slow, and holding Space surfaces), and the
documentation itself — every relative link resolves, no flag is documented in two places at
once, and no measurement is transcribed out of [`PERF.md`](PERF.md).

## Layout

| Path | |
|---|---|
| `src/voxel/` | tree, interned run pools, palette attributes, edits |
| `src/render/shaders/` | the seven compute passes in WGSL |
| `src/render/mod.rs` | pass orchestration, GPU pools, chunk list, shadow grid |
| `src/worldgen.rs` `src/biome.rs` | terrain generation and the biome table |
| `src/light.rs` | the sky and block light flood fill |
| `src/stream.rs` `src/lod.rs` | streaming, LOD selection, relighting after edits |
| `docs/` | why the constants of each shipped batch are what they are |



