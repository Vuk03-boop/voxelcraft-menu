# 0004. Static linear-albedo table for bounce light

Status: Accepted

## Context

Believable diffuse inter-reflection needs each material's reflectance.
Dynamic global illumination (path tracing, voxel GI) was out of budget for
this engine's frame cost; constant or absent albedo reads wrong the moment
colored lamps and tinted foliage light a surface.

## Decision

Ship a static table: `ALBEDO: [[f32; 3]; 26]` in `block.rs`, one row per
`BlockDef`, holding **linear Rec.709** reflectance. The probe bounce
(`probe.rs`) gathers through it at stride 2 and normalizes by
`luma(ALBEDO[BOUNCE_REF])`. Being static keeps the bounce path a table read,
not a simulation.

## Alternatives considered

- **Dynamic/voxel GI** — rejected: frame cost and state complexity far
  beyond the budget this pipeline sets.
- **Gamma-encoded values for authoring convenience** — rejected: lighting
  math assumes linear; sRGB rows would silently corrupt bounce energy. Rows
  are authored linear and stay linear.

## Consequences

- Adding a block is a three-step contract: add the `BlockDef`, append the
  albedo row, verify `REEDS == BLOCK_COUNT - 1`-style asserts still hold.
- Indirect light is fixed per block type and immediate neighborhood; it does
  not adapt beyond that — accepted as the engine's fidelity/performance
  point.
- `tests/biome.rs`-family and probe suites exercise the table; the values
  are documented in `docs/world.md`.
