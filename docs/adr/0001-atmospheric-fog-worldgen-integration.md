# ADR 0001: Atmospheric Fog and World Generation Coupling

## Status
Accepted

## Context
Atmospheric effects in voxel terrains often suffer from disconnected manual tuning: fog placed at arbitrary absolute altitudes either drowns elevated mountains or fails to blanket coastlines and oceans. Furthermore, intersecting cloud geometry with high mountain terrain causes visual clipping artifacts and requires expensive distance-field culling.

## Decision
1. **Procedural Fog Anchoring**: Dynamically couple the vertical origin of the fog system (`Fog` struct) to the world's topographical sea level constant (`crate::worldgen::SEA_LEVEL as f32`).
2. **Volumetric Fog Parameters**:
   - `density`: `5.0e-4` (subtle ambient atmospheric perspective without obstructing navigation).
   - `falloff`: `1.0 / 64.0` (gradual attenuation over 64 vertical units).
   - `scatter`: `0.020` with Henyey-Greenstein phase function parameter `g = 0.80` (strong forward scattering for solar illumination).
3. **Decoupled Cloud Layer**: Place the procedural cloud deck (`Clouds` struct) at a fixed high altitude (`448.0` units), well above maximum terrain generation elevation:
   - `cover`: `0.45` (partly cloudy default).
   - `scale`: `320.0` (base noise frequency).
   - `speed`: `3.0` blocks/sec drift.
   - `relief` / `patch`: `0.0` (simplified planar coverage ensuring minimal instruction overhead).

## Consequences
- **Positive**: Coastal areas and water basins naturally inherit dense atmospheric haze, while higher elevations remain clear, without manual per-biome volume placement.
- **Positive**: High-altitude cloud positioning eliminates geometry clipping and ray marching terrain collision tests.
- **Negative**: Cloud deck cannot dip below mountain summits for dense ground-level overcast weather without dynamic altitude scaling.
