# 0001. Decoupled shadow architecture (DS01)

Status: Accepted

## Context

Soft shadows classically cost multi-tap depth sampling plus full-resolution
blur passes (PCSS-style): several extra taps per pixel, an intermediate depth
pyramid, and a separable blur chain — memory bandwidth and VRAM on a frame
that already marches a voxel tree. The naive alternative, a single hard
shadow ray, is cheap but visibly wrong under a large sun.

## Decision

Resolve soft shadows in a decoupled pass driven by one geometric-ray primary
shadow. The primary pass stores the distance to the nearest blocker behind
each pixel in a dedicated `R16Float` render target; companion targets use
`Rg16Float`/`R8Unorm`. Penumbra size is reconstructed from that blocker
distance downstream instead of being re-traced or blurred at full resolution.
Implementation: `src/render/shadow.rs` (pass + target formats) and
`src/render/shaders/shadow.wgsl`.

## Alternatives considered

- **PCSS-style multi-tap + blur** — rejected: multiple full-res depth maps
  and blur intermediates; the cost profile this engine was built to avoid.
- **Hard single ray, no blocker storage** — rejected: no penumbra response
  at all; unacceptable under the sky/sun model.
- **`R32Float` blocker map** — rejected: `R16Float` halves the target
  (≈4 MiB vs ≈8 MiB at 1920×1080) at a precision the penumbra estimate
  tolerates.

## Consequences

- Visually plausible soft shadows at a fraction of the bandwidth; some
  theoretical softness ceiling is traded away deliberately.
- The blocker map's format and layout are a cross-file contract (Rust pass,
  WGSL consumers); `tests/shadow_grid.rs` pins behavior and
  `validation/check_shadow`-family history tracks it.
- Any future change to shadow quality must start from the blocker-distance
  mechanism, not from reintroducing a blur chain.
