# 0003. Fog height derives from `worldgen::SEA_LEVEL`

Status: Accepted

## Context

Ground fog needs a vertical anchor. A free-floating fog height invites
per-biome tuning and drift: every new world tweak silently invalidates the
hand-set value, and mismatches read instantly (fog floating over ridges or
missing from the sea).

## Decision

`Fog::default().height` is `crate::worldgen::SEA_LEVEL as f32` — the one
waterline constant (128) owned by `worldgen.rs`. Low terrain and the sea
surface sit inside the fog layer; elevations climb out of it procedurally,
with no per-biome override.

## Alternatives considered

- **Authored constant independent of sea level** — rejected: two sources of
  truth that must be kept consistent by hand.
- **Per-biome fog heights** — rejected: tuning burden and seams; the
  sea-level rule reads naturally in every tested biome.

## Consequences

- Changing `SEA_LEVEL` retunes fog everywhere at once — intended, and the
  reason `AGENTS.md` forbids a second waterline constant (hard rule 5).
- Biome/elevation work inherits correct atmosphere for free.
- The remaining atmosphere knobs (`density`, `falloff`, `scatter`, `g`) are
  documented in `docs/rendering.md`; screenshot arms pin the look
  (`--reference`, `--taa-frames`).
