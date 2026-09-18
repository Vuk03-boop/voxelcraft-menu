# ADR 0003: Water Variance Filtering (WA01) and Mottle Suppression

## Status
Accepted

## Context
Procedurally generated water surfaces driven by micro-normal wave functions introduce high-frequency slope variance. At grazing angles or distance, this slope variance induces severe specular aliasing, crawling highlights, and unstable reflection/refraction vectors. Conventional filtering dampens wave amplitude, destroying realistic crisp highlights.

## Decision
Implement **Water Variance Filtering (WA01)**:
1. **Variance Preservation via Normal Blending**: Dynamically preserve unresolved slope variance by blending the final perturbed wave normal towards the flat geometric normal (`vec3<f32>(0.0, 1.0, 0.0)`) proportionally as distance/footprint variance increases.
2. **Hard-Coded Zero Mottle Enforcement**: Force `water_mottle = 0.0` inside configuration parsing (`src/config.rs`) and texture atlas generation (`src/textures.rs`), ignoring any user-specified `--water-mottle` CLI flags.

## Consequences
- **Positive**: Eliminates specular flickering and temporal instability over open water surfaces while preserving animated wave geometry.
- **Positive**: Eliminates high-frequency noise in secondary traced rays.
- **Negative**: User attempts to customize water mottle via command-line flags are intentionally disregarded to protect pipeline rendering stability.
