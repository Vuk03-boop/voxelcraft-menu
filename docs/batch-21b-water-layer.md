# Batch 21b — the sea samples the glass layer

> **Reconstructed after the 2026-09-18 documentation cleanup.** The original
> note did not survive the text export; the surviving evidence is the comment
> in `src/render/mod.rs` that names the batch: 21b "made the sea sample the
> glass layer."

The water's secondary leg stopped re-tracing the glass transport and instead
reads the glass layer's result, so a submerged view of glass pays the glass
transport once rather than once per leg.

The arms that still exercise the layer (wording from `--help`):

- `--no-water-reflect`, `--no-water-refract` — disable the two transport legs.
- `--no-water-sec` — "full-res traced water legs (reverts batch 81's
  half-res diagnostic: P12's measurement arm)".
- `--water-sec-scale N` — "water legs per NxN block: 1, 2 (default) or 4".

The layer's current role: [water.md](water.md). The transport cost the
sharing avoids: [shading.md](shading.md).
