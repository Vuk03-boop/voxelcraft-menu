# Batch 19 — grazing angles

> **Reconstructed after the 2026-09-18 documentation cleanup.** The original
> note — a table of grazing-angle behavior in `resolve`'s water-interface
> shading — did not survive the text export. The comments in
> `src/render/shaders/resolve.wgsl` still name what the table held; the table
> itself is gone.

What survives of the batch:

- The grazing-angle shading of the water interface in `resolve`; the comments
  around the secondary water shading point at the lost table.
- The rule that a grazing-angle change is measured at the harness vantages,
  not by eye — see [harness.md](harness.md).
- The related look knobs that outlived it: `--sun-softness narrow|normal|wide`
  and the soft-shadow arms, all in `--help`.

If a future batch needs the numbers again, re-measure at the grazing
vantages; do not transcribe a remembered value.
