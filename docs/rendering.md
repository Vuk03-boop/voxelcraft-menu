# Rendering

## Atmosphere: fog and clouds

Two cheap, separate systems rather than one volumetric model. Both default
looks live in `render/mod.rs` (`Fog`, `Clouds`, `SkyLook`), animated free of
the march pass.

| Parameter | `Fog` | `Clouds` |
| --- | --- | --- |
| Density / cover | `5.0e-4` | `0.45` |
| Falloff | `1.0/64.0` | — |
| Scatter coefficient | `0.020` | — |
| Phase `g` | `0.80` (strong forward scatter) | — |
| Height | `worldgen::SEA_LEVEL` (128) | `448.0` |
| Scale | — | `320.0` |
| Speed | — | `3.0` |
| Patch / relief | — | `0.0` / `0.0` (`--cloud-patch`, `--cloud-relief`) |

Decisions encoded here:

- **Fog height derives from sea level**, so low terrain is shrouded and high
  terrain is clear without per-biome tuning (ADR 0003).
- High `g` gives a sun-forward haze rather than an even murk; density stays
  low enough to read as aerial perspective, not occlusion.
- Clouds sit at a fixed deck above all geometry; `patch` and `relief` default
  to the pre-G1 sky and only shade — relief never adds density.
- `SkyLook` (`--haze-warm`, `--zenith-deep`) defaults to 0/0, the pre-95 sky.

## Shadows: decoupled architecture (DS01)

One geometric-ray primary shadow pass instead of a multi-tap soft shadow +
blur. The primary pass stores the distance to the nearest blocker behind the
pixel in a dedicated `R16Float` target (half-precision: ≈4 MiB at
1920×1080 instead of ≈8 MiB for R32); the companion targets use
`Rg16Float`/`R8Unorm`. Soft penumbras are reconstructed later from that
blocker distance rather than re-traced or blurred at full resolution.

Trade-off, recorded in `docs/adr/0001-decoupled-shadow-architecture.md`:
visually plausible soft shadows at a fraction of the bandwidth of
PCSS-style pipelines, at some cost to maximal theoretical softness.

## Water: variance-preserving filtering (WA01)

Unresolved wave-slope variance aliases into sparkle. Naive variance removal
smooths that away along with the detail that carries reflection and normal
response; WA01 instead **preserves** that variance and blends the final
normal toward the geometric normal as the unresolved variance grows. Detail
survives, extreme reflection vectors do not.

Consequences:

- `--water-mottle` is pinned to `0.0`: the flag still parses (see the
  `--help` line) but the value is discarded with a stderr notice
  ("water mottle is fixed at zero to keep the atlas layer flat").
  ADR 0002.
- Secondary (screen-space-traced) water legs run at half resolution by
  default: `--water-sec-scale N` = legs per N×N block (`1|2|4`, default 2);
  `--no-water-sec` reverts to the full-res path as the measurement arm.
- `--water-look` composes ripples + murk absorption + turquoise shallows.

## Light transport

`light.rs` runs a 15-level flood fill (`MAX_LEVEL`) per chunk over a
4096-entry cell table (`CELLS`); a cell is either an inline uniform
half-word or an index into a brick pool (`BRICK_WORDS = 32`).

- Block light is **three channels** (`sky:4 | r:4 | g:4 | b:4`, two bytes per
  cell). One propagation rule applied to three arrays, so a white emitter's
  channels stay equal everywhere and `max(r, g, b)` reproduces the mono
  result — this is what makes `--no-light-rgb` a bit-exact control.
- `common.wgsl` `light_at` must mirror the two-byte layout
  (`(idx & 1) * 16` half-word select). `tests/light.rs` pins both copies.
- Bounce lighting in `probe.rs` gathers from a static per-block albedo table
  (see `world.md`), stride-2 over the column.

## Presentation

- TAA: Halton sub-pixel jitter by default; `--jitter X[,Y]` pins the offset;
  `--taa-frames N` sets convergence length for captures (default 32), and
  `--reference N` converges an N×N sub-pixel grid as ground truth.
- Tone map: `--tone-map knee|aces`, tuned against `knee`.
- Grade: `--grade none|warm|cine` with `--grade-strength` (1.0 = the cast
  exactly, below lerps back). `none` is the pre-95 presentation bit for bit.
- `--scale` renders internally at a fraction; `--mip-bias` biases the atlas
  mip in resolve.
