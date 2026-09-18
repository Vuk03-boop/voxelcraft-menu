# Validation Framework and Architectural Invariants

## 1. Quality Assurance Philosophy
The test suite in `tests/` is organized modularly by functional discipline, enforcing strict architectural and mathematical invariants rather than subjective visual approximations.

## 2. Core Invariants

### 2.1 Bit-Exact Reproducibility
- Core rendering passes support baseline feature deactivation flags (`--no-water`, `--no-biomes`, `--no-waves`, `--no-taa`, `--no-fade`, `--cloud-cover 0`).
- A feature toggle must reproduce the exact pre-feature baseline commit down to the bit-level output of the frame. This determinism prevents floating-point drift and cross-platform regressions.

### 2.2 Host-Device Constant Synchronization
- Numerical parameters defined in Rust data structures (`src/render/mod.rs`, `src/config.rs`) must remain strictly synchronized with WGSL uniform definitions (`src/render/shaders/*.wgsl`).
- The test suite validates memory layout offsets, field ordering, and constant equality before pipeline execution.

### 2.3 Algorithmic Verification
The test suite performs formal mathematical proofs on foundational algorithms:
- **Snell's Law & Refraction**: Validated in `tests/snell.rs` across air/water interfaces.
- **Wave Autocorrelation**: Tested in `tests/waves.rs` to guarantee statistical surface stationarity.
- **Light Envelope Scan**: Tested in `tests/light.rs` to confirm monotonic heightfield occlusion properties.
- **Glass & Cutout Boundaries**: Validated in `tests/glass.rs` and `tests/cutout.rs` ensuring sub-voxel ray transmission correctness.

## 3. Test Suite Organization
| Test Module | Coverage Scope |
|---|---|
| `tests/water.rs`, `tests/waves.rs` | Wave equations, depth absorption, water surface invariants |
| `tests/snell.rs`, `tests/glass.rs` | Refraction, Fresnel reflection, pane transmission |
| `tests/light.rs`, `tests/probe.rs` | Sky/block light flood fill, Rec.709 bounce lighting |
| `tests/worldgen.rs`, `tests/biome.rs` | Procedural terrain, biome interpolation, `SEA_LEVEL` constraints |
| `tests/physics.rs`, `tests/camera.rs` | Player collision, non-tunneling bounds, camera projection |
