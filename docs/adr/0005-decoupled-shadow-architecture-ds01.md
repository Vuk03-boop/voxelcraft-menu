# ADR 0002: Decoupled Shadow Architecture (DS01)

## Status
Accepted

## Context
Soft shadow simulation in voxel ray tracing traditionally demands multi-ray sampling passes (such as Percentage-Closer Soft Shadows / PCSS) or expensive shadow map filtering. In a high-resolution deferred or compute-based pipeline, tracing multiple shadow rays per pixel overwhelms GPU memory bandwidth and ALU execution budgets.

## Decision
Adopt the **Decoupled Shadow Architecture (DS01)**:
1. **Single Geometric Ray Primary Pass**: Rather than multi-ray stochastic sampling, cast a single geometric primary shadow ray to detect blockers.
2. **Half-Precision Blocker Depth Storage**: Store nearest blocker distance in a dedicated `R16Float` render target instead of standard full-precision `R32Float`.
3. **Screen-Space VRAM Budget**: At 1920×1080 resolution, the `R16Float` blocker map consumes approximately ~13.84 MiB of VRAM (1920 * 1080 * 2 bytes = 4.15 MB base + mip/filter targets = ~13.84 MiB total working footprint).

## Consequences
- **Positive**: 50% reduction in shadow buffer memory footprint compared to `R32Float`.
- **Positive**: High frame rates sustained on discrete consumer GPUs without requiring hardware ray tracing acceleration.
- **Trade-off**: Sacrifices photorealistic penumbra accuracy under complex overlapping blocker geometries in exchange for deterministic execution and bounded memory consumption.
