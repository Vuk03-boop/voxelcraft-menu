# Architecture

One crate, one binary (`voxelcraft`, `default-run`). `src/main.rs` parses
`Config` via `config::parse_args` and dispatches a `Mode`: `Run` (windowed),
`Screenshot`, `Lookbook`, `BenchFrames`, `BenchTerrain`, `Diff`.

## Module map

| Module | Role |
| --- | --- |
| `app.rs` | Windowed mode: winit event loop, surface, input. The window must outlive the surface built from it. |
| `headless.rs` | Offscreen entry points for the screenshot/bench/lookbook/diff modes. |
| `config.rs` | Every CLI flag -> `Config`; owns the `--help` text (the authoritative flag reference). |
| `worldgen.rs` | Height, caves, water, trees from `fastnoise-lite`; `SEA_LEVEL` lives here. |
| `biome.rs` | Temperature/humidity fields via in-repo `simplex2` (pinned against `fastnoise-lite` by `tests/biome.rs`). |
| `voxel/tree.rs` | Sparse tree: node/brick layout, `local_block`, `dense_index`, `DIM`, `VOL`. |
| `voxel/chunk.rs` | Chunk records, key types, attribute refs shared with light. |
| `voxel/world.rs` | The `World`: chunk store + query/edit API. |
| `voxel/geometry.rs` | Occupancy bitmasks (`below`, `has`, `Inner`, runs, `FULL_FLAG`). |
| `voxel/attributes.rs` | Per-leaf attribute storage. |
| `stream.rs` / `lod.rs` | Chunk streaming and LOD subdivision/cross-fade (`--max-lod`, `--streaming-factor`, `--fade-band`, `--fade-schedule`). |
| `light.rs` | Sky + 3-channel block-light flood fill; per-chunk cell table with brick indirection. See `rendering.md`. |
| `block.rs` | `BlockDef` table (26 blocks) + linear albedo table. See `world.md`. |
| `scene.rs` | Assembles the camera/world/lamps for a frame; demo structures via the edit path (`--demo-edits`, `--demo-glass`, `--demo-lamps`). |
| `render/mod.rs` | wgpu device/pipelines, uniform packing, `Fog`/`Clouds`/`SkyLook` look structs, screenshot path. |
| `render/shadow.rs` | The decoupled soft-shadow pass (DS01). See `rendering.md` + `docs/adr/0001`. |
| `render/blit.rs` | Present pass: tone map (`knee|aces`), grade (`none|warm|cine`). |
| `render/ui.rs`, `render/font.rs` | HUD/stats overlay (`--hud`). |
| `render/timer.rs`, `render/pools.rs` | GPU timing, buffer pools. |
| `shaft.rs` | Light shafts (volumetric beams). |
| `textures.rs` | Texture atlas generation/sampling rules. |
| `player.rs`, `math.rs` | Movement/collision physics, shared math. |
| `menu.rs`, `settings.rs`, `idle.rs` | Front-end menu flow, preferences, idle/demo attract mode. |
| `journal.rs` | Edit journal: world edits recorded/replayed, exit save, resume. |
| `lookbook.rs` | `--lookbook`: 3 views × 12 settings composed into one labeled PNG. |
| `stats.rs` | Frame stats plumbing for the HUD. |
| `harness/` | `metric.rs` (capture + compare metrics), `bench.rs` (canonical bench: 1920×1080), `vantage.rs` (camera recipes). |
| `probe.rs` | Light-probe bounce experiment (albedo stride 2). Not shipped in the windowed loop. |

## Data flow of a frame

`worldgen` -> `voxel::World` (chunks) -> `stream`/`lod` (resident set +
LOD) -> `scene` (camera, lamps, look structs) -> `render` (march `march.wgsl`
against the tree; shade in `resolve.wgsl` using `common.wgsl` shared
helpers) -> TAA accumulation (`taa.wgsl`, Halton jitter by default) ->
`blit.wgsl` present with tone map + grade. Shadowing is resolved by
`shadow.wgsl` + `shadow.rs`; light shafts by `shaft.wgsl`.

World edits go through the edit path only and are journalized; loading,
appending and saving journals (`--load-edits`, `--save-edits`, `--edits`,
`--resume`, `--no-exit-save`) is how worlds persist on disk.

## WGSL inventory (`src/render/shaders/`)

| File | Purpose |
| --- | --- |
| `common.wgsl` | Shared helpers: occupancy masks, `light_at`, `SURFACE_EPS` (its only definition). Parsed by tests for pinned constants. |
| `march.wgsl` | Primary ray march through the voxel tree. |
| `resolve.wgsl` | Shading of a hit: cones, Fresnel/mirror terms, water legs. |
| `taa.wgsl` | Temporal accumulation/resolve. |
| `shaft.wgsl` | Light shafts. |
| `shadow.wgsl` | The DS01 blocker/shadow pass companion. |
| `tile_select.wgsl` | Atlas tile selection. |
| `blit.wgsl` | Present: tone map + grade + sRGB. |
| `ui.wgsl` | HUD overlay. |
