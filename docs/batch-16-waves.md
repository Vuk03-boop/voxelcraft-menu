# Batch 16 — waves

> **Reconstructed after the 2026-09-18 documentation cleanup.** The original
> batch note did not survive the text export. Everything below is checked
> against the current flags and shader comments; where the original recorded a
> number, [../PERF.md](../PERF.md) and the comments in
> `src/render/shaders/common.wgsl` and `resolve.wgsl` are the surviving
> source.

The ocean got animated waves: the field's slope and normal are computed in
`common.wgsl`, and `resolve` shades the water interface from the resulting
optical normal. The slope-variance figure the comments point at is recorded in
[../PERF.md](../PERF.md); the wave sum has no window, no padding and no
variance normalisation (see [water.md](water.md)).

## Live controls

- `--no-waves` — the pre-batch-16 sea: flat.
- `--wave-amp`, `--wave-scale`, `--wave-speed`, `--wave-clamp` — amplitude,
  spatial scale, drift speed, clamp.
- `--no-wave-aniso`, `--no-wave-fill`, `--no-wave-shoal` — the later
  corrections to the base field.

## The consequence this batch forced

High-frequency slope noise aliases in the reflected and refracted samples. The
current response is documented in [water.md](water.md): the removed variance is
handed to the glint lobe as roughness rather than smoothed out of the surface,
and the mottle term was fixed at zero (accepted, ignored) instead of re-tuned
against it.
