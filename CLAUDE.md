# CLAUDE.md — Agent Behavioral Contract & Architecture Pointer

## 1. Project Overview & Architectural Map
VoxelCraft is a real-time GPU ray-traced voxel engine written in Rust and WGSL (`wgpu`).
- **Core Principle**: GPU sparse trees (4³-branching nodes, 64³ chunks), zero mesh generation, zero triangles.
- **Compute Pipeline**: Light envelope -> Tile select -> Ray march (64-bit visibility buffer) -> Hi-Z build -> Recovery -> Resolve (shading) -> Temporal resolve (TAA).
- **Decoupled Architecture**: Geometry (64-bit occupancy masks) and attributes (palette-compressed IDs, lighting bricks) are strictly decoupled.

## 2. Essential Commands
- `cargo run --release`: Start interactive application.
- `cargo test --release`: Run full test suite.
- `cargo run --release -- --bench-frames 140 --width 1920 --height 1080`: Headless frame benchmark.
- `cargo run --release -- --screenshot shot.png --width 1920 --height 1080 --hud`: Headless capture.
- `cargo run --release --bin harness -- capture`: Standardized metric suite.

## 3. Strict Behavioral Rules & Invariants
Every automated modification must respect these non-negotiable invariants:
1. **Bit-Exact Determinism**: Control flags reproducing prior baselines (`--no-water`, `--no-biomes`, `--no-waves`, `--no-taa`) must produce bit-exact renders down to the pixel.
2. **Host-Device Constant Synchronization**: Shader constants in `src/render/shaders/*.wgsl` must exactly mirror Rust struct equivalents in `src/render/mod.rs` and `src/config.rs`.
3. **Decoupled Shadow (DS01)**: Single geometric ray pass; blocker depth must remain stored in `R16Float` format (~13.84 MiB at 1080p).
4. **Water Variance Filtering (WA01)**: Wave normal variance stabilization blends to flat geometric normal; `water_mottle` is locked to `0.0` at runtime (cli flag override ignored).
5. **Rec.709 Linear Radiance**: The `ALBEDO` table (26 blocks) must stay linear in Rec.709 luminance space; never sRGB.
6. **Atmospheric Coupling**: Fog height is dynamically linked to `crate::worldgen::SEA_LEVEL as f32`.

## 4. Out-of-Scope Files (Do Not Touch Without Explicit Request)
- `target/`: Build artifacts.
- `benches/terrain.rs`: Hardware-baseline performance harness.
- Procedural noise cores in `src/biome.rs` and `src/worldgen.rs` (unless fixing proven math defects).

## 5. Agent Struggle & Fumble Log [CLAUD-FUMBLE]
To prevent catastrophic remembering and repeated traps, respect documented failure modes:
- `[CLAUD-FUMBLE: LINE-ENDINGS]` `Cargo.toml` and `.gitattributes` require CRLF; `.rs` and `.wgsl` require LF. Do not introduce mixed line endings.
- `[CLAUD-FUMBLE: WGSL-ALIGNMENT]` Uniform buffers in WGSL enforce 16-byte alignment. Do not reorder fields in `GpuFrame` without padding.
- `[CLAUD-FUMBLE: WATER-MOTTLE]` Do not attempt to re-enable `--water-mottle` in the pipeline; WA01 intentionally overrides it to `0.0`.
- `[CLAUD-FUMBLE: RAY-PACKING]` Ray marching 64-bit visibility keys are densely packed; do not insert bitfields without updating unpack shaders.

## 6. Architecture Documentation Pointers
- Atmospheric Fog & Clouds: [docs/adr/0001-atmospheric-fog-worldgen-integration.md](docs/adr/0001-atmospheric-fog-worldgen-integration.md)
- Decoupled Shadows (DS01): [docs/adr/0002-decoupled-shadow-architecture-ds01.md](docs/adr/0002-decoupled-shadow-architecture-ds01.md)
- Water Variance Filtering (WA01): [docs/adr/0003-water-variance-filtering-wa01.md](docs/adr/0003-water-variance-filtering-wa01.md)
- BlockDef & Linear Albedo: [docs/adr/0004-blockdef-and-rec709-linear-albedo.md](docs/adr/0004-blockdef-and-rec709-linear-albedo.md)
- Test Suite & Invariants: [docs/validation-and-invariants.md](docs/validation-and-invariants.md)
