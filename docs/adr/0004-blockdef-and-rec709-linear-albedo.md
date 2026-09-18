# ADR 0004: BlockDef Unified Properties and Linear Rec.709 Albedo Model

## Status
Accepted

## Context
A voxel world requires physical simulation (collision, player movement), light propagation (flood fill, attenuation), and visual rendering (alpha cutout, biome tint, diffuse bounce). Storing material properties scattered across disparate arrays leads to cache misses, synchronization bugs, and non-linear lighting calculation errors.

## Decision
1. **Component-Based `BlockDef` Blueprint**: Define every block type in `src/block.rs` with unified properties:
   - `faces: [u32; 6]`: Atlas layer indices per face normal (-X, +X, -Y, +Y, -Z, +Z).
   - `solid: bool`: Governs player collision and physics obstruction.
   - `opaque: bool`: Controls light attenuation in the sky/block light flood fill.
   - `cutout: bool`: Distinguishes sub-voxel geometry (e.g. 4³ leaf canopy masks) from alpha blending.
   - `foliage: bool`: Flags in-voxel cross-quads where atlas alpha represents opacity rather than biome tint mask.
   - `light: [u8; 3]`: Emissive RGB light levels (e.g. glowstone, lamps).
2. **Linear Rec.709 Albedo Lookup Table (`ALBEDO`)**:
   - Provide an immutable static table of 26 blocks (Air through Reeds).
   - Ensure all reflectance values are strictly linearRec.709 luminance values, derived after decoding `Rgba8UnormSrgb` atlas textures to avoid gamma-biasing diffuse bounce illumination.

## Consequences
- **Positive**: Eliminates runtime texture inspection during indirect lighting passes; bounce light calculations use immediate array index lookups.
- **Positive**: Clear separation between collision (`solid`), light obstruction (`opaque`), and geometry format (`cutout`/`foliage`).
- **Negative**: Dynamic per-block albedo tinting requires modifying or recreating the global albedo table.
