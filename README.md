# VoxelCraft

A Minecraft-style voxel engine in Rust that renders in real time by **ray tracing sparse trees directly on the GPU** — no polygon meshing, no triangle rasterization.

Built with [`wgpu`](https://github.com/gfx-rs/wgpu) using compute shaders written in WGSL.

---

## Architectural Highlights

- **Sparse Tree GPU Ray Marching**: Chunks are 64³ voxels stored in 3-level 4³-branching sparse trees. Geometry (64-bit occupancy masks) and attributes (palette-compressed IDs and light bricks) are decoupled.
- **Atmospheric Integration**:
  - **Volumetric Fog**: Anchored dynamically to the world generation sea level (`SEA_LEVEL = 128`), with density `5.0e-4`, falloff `1/64`, and strong forward scattering (`g = 0.80`).
  - **Procedural Clouds**: High-altitude planar noise deck at height `448.0` with cover `0.45`, drifting at `3.0` blocks/sec.
- **Decoupled Shadow Architecture (DS01)**:
  - Soft shadows are computed via a single geometric primary ray pass storing nearest blocker depth in a half-precision `R16Float` buffer (~13.84 MiB at 1080p), cutting VRAM bandwidth compared to full-precision buffers and avoiding costly multi-ray PCSS passes.
- **Water Variance Filtering (WA01)**:
  - Procedural micro-normal wave slopes preserve high-frequency variance by blending the optical normal towards the geometric flat normal as variance increases.
  - Runtime enforcement suppresses `water_mottle` to `0.0` to preserve flat atlas layers and numerical stability.
- **BlockDef & Linear Rec.709 Bounce Lighting**:
  - `BlockDef` unifies solidity, opacity, cutout status (alpha testing / sub-voxel geometry), foliage flags, and RGB emission.
  - An immutable 26-block lookup table (`ALBEDO`) provides linear reflectance values in Rec.709 space, ensuring physically grounded diffuse inter-reflection.

---

## System Requirements

Requires a discrete GPU supporting 64-bit shader atomics (`SHADER_INT64` and `SHADER_INT64_ATOMIC_MIN_MAX`). The engine explicitly checks for these capabilities on initialization.

---

## Running VoxelCraft

### Interactive Mode
```bash
cargo run --release
```

#### Controls
| Input | Action |
|---|---|
| **WASD** | Movement (forward, left, back, right) |
| **Mouse Look** | Camera yaw and pitch |
| **Space / Shift** | Jump / descend (swim up / dive in water) |
| **F** | Toggle fly mode |
| **Ctrl** | Sprint |
| **LMB / RMB** | Break block / place block |
| **1 – 0 / Wheel** | Select hotbar block |
| **F3** | Toggle diagnostic stats |
| **F4** | Render scale toggle |
| **F5** | Iteration heatmap |
| **F6** | Hi-Z culling toggle |
| **F7** | Temporal Anti-Aliasing (TAA) toggle |
| **Esc** | Pause menu / resume |

---

## Headless & Benchmarking Modes

Headless execution is fully supported without an active display surface:

```bash
# Capture a 1080p frame with HUD overlay
cargo run --release -- --screenshot shot.png --width 1920 --height 1080 --hud

# Execute a 140-frame headless benchmark
cargo run --release -- --bench-frames 140 --width 1920 --height 1080

# Benchmark procedural terrain generation across 96 chunks
cargo run --release -- --bench-terrain 96

# Capture standardized regression metrics
cargo run --release --bin harness -- capture
```

---

## Testing & Verification

```bash
cargo test --release
```

The validation suite enforces strict invariants:
- **Bit-Exact Determinism**: Baseline switches (`--no-water`, `--no-biomes`, `--no-waves`, `--no-taa`) produce bit-for-bit identical frames to pre-feature commits.
- **Shader Constant Sync**: CPU-side Rust structures in `src/render/` remain strictly synchronized with GPU-side WGSL uniforms.
- **Algorithmic Correctness**: Dedicated suites test refraction (`tests/snell.rs`), wave autocorrelation (`tests/waves.rs`), and occlusion prefix scans (`tests/light.rs`).

For more details, see [docs/validation-and-invariants.md](docs/validation-and-invariants.md).

---

## Architecture Decision Records (ADRs)

Key architectural decisions are documented under `docs/adr/`:
- [ADR 0001: Atmospheric Fog and World Generation Coupling](docs/adr/0001-atmospheric-fog-worldgen-integration.md)
- [ADR 0002: Decoupled Shadow Architecture (DS01)](docs/adr/0002-decoupled-shadow-architecture-ds01.md)
- [ADR 0003: Water Variance Filtering (WA01)](docs/adr/0003-water-variance-filtering-wa01.md)
- [ADR 0004: BlockDef Unified Properties and Linear Rec.709 Albedo Model](docs/adr/0004-blockdef-and-rec709-linear-albedo.md)

For AI agent behavioral guidelines and automated workflow rules, see [CLAUDE.md](CLAUDE.md) / [AGENTS.md](AGENTS.md).
