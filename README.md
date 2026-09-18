# voxelcraft

A Minecraft-style voxel engine in Rust that renders in real time by **ray
tracing sparse trees directly on the GPU** — no polygon meshing, no triangle
rasterization. Compute shaders in WGSL, driven through `wgpu`; the frame is
structured after [Aokana](https://arxiv.org/html/2505.02017v1) (Fang, Wang &
Wang, PACMCGIT 2025), built from the techniques catalogued in
[`docs/reference.md`](docs/reference.md).

This branch adds the **settings menu** — title, pause and settings screens
with draft/apply semantics, plus seven title-only rendering experiments.
[docs/menu.md](docs/menu.md) is the current behavior;
[docs/roadmap.md](docs/roadmap.md) is what is next.

## Architectural highlights

- **Sparse tree GPU ray marching**: 64³ chunks in three-level 4³ trees;
  geometry (64-bit occupancy masks) strictly decoupled from attributes
  (palette-compressed IDs, light bricks).
- **Atmosphere**: volumetric fog anchored to worldgen sea level
  (`SEA_LEVEL = 128`; density 5.0e-4, falloff 1/64, forward-scattering
  `g = 0.80`); procedural cloud deck at internal Y 448 (cover 0.45, drift
  3.0 blocks/s). The parameters and their zero controls:
  [docs/sky.md](docs/sky.md),
  [ADR](docs/adr/0004-atmospheric-fog-worldgen-integration.md).
- **DS01, decoupled shadows**: one geometric primary ray pass storing the
  nearest blocker distance in an `R16Float` buffer (7 bytes per pixel, about
  13.84 MiB at 1920×1080) plus screen-space reconstruction — no multi-ray PCSS
  pass. [ADR](docs/adr/0005-decoupled-shadow-architecture-ds01.md).
- **WA01, water variance filtering**: wave slope variance the filter removes
  is handed to the glint as roughness instead of being smoothed out of the
  surface; the mottle term is fixed at zero by runtime enforcement.
  [ADR](docs/adr/0006-water-variance-filtering-wa01.md).
- **BlockDef + linear Rec.709 albedo**: one definition per block carries
  solidity, opacity, cutout, foliage and RGB emission; a 26-row `ALBEDO`
  table supplies linear reflectance for bounce lighting.
  [ADR](docs/adr/0007-blockdef-and-rec709-linear-albedo.md).

## Requirements

A GPU with 64-bit shader atomics (`SHADER_INT64` +
`SHADER_INT64_ATOMIC_MIN_MAX`) — any modern discrete card. The engine refuses
to start otherwise rather than silently falling back.

## Running it

```bash
cargo run --release        # windowed game
cargo test --release       # the invariant suite; needs no GPU
cargo bench --bench terrain
```

| Input | Action |
|---|---|
| **WASD** | Move |
| **Mouse look** | Camera yaw/pitch |
| **Space / Shift** | Up / down (swim up / dive in water) |
| **F** | Toggle fly |
| **Ctrl** | Sprint |
| **LMB / RMB** | Break / place |
| **1–0 / wheel** | Pick hotbar block |
| **F3** | Stats |
| **F4** | Render scale |
| **F5** | Iteration heatmap |
| **F6** | Hi-Z culling |
| **F7** | Temporal anti-aliasing |
| **Esc** | Pause menu / resume |

Choose **Play** on the title screen to begin.

Headless modes are a first-class measurement interface: a single frame to
PNG, terrain and frame timing, a labeled lookbook, and a diff of two saved
captures:

```bash
cargo run --release -- --screenshot shot.png --width 1920 --height 1080 --hud
cargo run --release -- --bench-frames 140 --width 1920 --height 1080
cargo run --release -- --bench-terrain 96
```

The flag reference is the `--help` text, and it is the complete list. A
persistent world is one file:

```bash
cargo run --release -- --edits my-world.journal
```

## Repository map

- `src/` — the engine crate (`lib.rs`) and the game binary (`main.rs`, which
  splits into `scene` / `headless` / `app`). Rendering in `src/render/` with
  the `.wgsl` shaders beside it, the sparse-voxel world in `src/voxel/`, the
  measurement fixture in `src/harness/`, the menu and preferences in
  `src/menu.rs` and `src/settings.rs`, the world journal in `src/journal.rs`.
- `tests/` — the invariant suite: bit-exactness against the pre-feature
  control, CPU/GPU shader-constant sync, and the mathematics of the core
  algorithms (refraction in `tests/snell.rs`, wave statistics in
  `tests/waves.rs`, the light flood in `tests/light.rs`).
- `benches/` — worldgen timing.
- `validation/` — historical Python checks; read its
  [README](validation/README.md) before running any of them.
- [`PERF.md`](PERF.md) — the only place that quotes a measurement, and every
  number there carries the vantage and recipe it was taken at.

Test-suite invariants in detail:
[docs/validation-and-invariants.md](docs/validation-and-invariants.md).

## Documentation

[`CLAUDE.md`](CLAUDE.md) is the briefing — a symlink to the canonical
[`AGENTS.md`](AGENTS.md), kept short on purpose, with the hard rules and the
batch control table. Each subsystem keeps its own document —
[shading](docs/shading.md), [water](docs/water.md), [sky and
fog](docs/sky.md), [blocks and the albedo table](docs/reference.md),
[persistence](docs/persistence.md), [the measurement
harness](docs/harness.md), [failure modes that look
like success](docs/errors.md), [pitfalls](docs/pitfalls.md),
[lessons](docs/lessons.md), the [batch ledger](docs/ledger.md) — and the
decisions and their reasoning live in [docs/adr/](docs/adr/).

The screenshots and PDF reports from the original handoff did not survive the
text export. Where a measurement used to sit beside an image, the capture
recipe stands in for it; the recipes are in
[docs/harness.md](docs/harness.md).
