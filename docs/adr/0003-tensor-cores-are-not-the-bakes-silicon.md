# ADR 0003 — The tensor cores stay idle: the bake's gather is not its bottleneck

**Status: decided without a build, on the entry's own kill wire.** Roadmap item L6 (*the 64
tensor cores as a bake GEMM or not at all*) was written with two questions that byte math in
the sandbox could answer and a kill clause that fired. This file is the record, so the idea
is not re-planned a third time by a scarcity table that keeps listing 64 idle cores.

---

## The decision

The probe bake's final accumulation is not GEMM-shaped work worth silicon, because it is
**not the bake**. The tensor cores stay idle. Not "until measured": the fraction the kill
clause asked about is lower by *orders* than the 25% that would have kept the door open.

## The arithmetic, in full

The bake's last stage is a gather: for each probe, the horizon march's per-azimuth results
accumulate into the six axis entries with cosine weights. That is the only tensor-shaped
structure in the batch.

- **MACs per probe, generously framed.** 6 axes x 16 azimuths of accumulation, say two
  FMAs apiece per direction term: **on the order of 2 x 10^2 fused multiply-adds**. As a
  GEMM it is a 6 x 16 matrix times a 16-wide sample column -- a problem of 96 outputs and
  ~1.9K MACs *before* exaggeration.
- **Per chunk.** At post-L5 spacing, 16^3 = 4,096 probes: **~8 x 10^6 MACs** against one
  filled 16x16x16 cooperation tile's 4,096 MACs -- the tile question answers **yes** with
  three orders of magnitude to spare (the kill wire asked "fewer MACs than one tile").
- **The kill wire: fraction of the 3,125 us chunk line.** The bake's cost is the **march
  that feeds the gather**: per probe, 16 azimuths x 20 geometric samples out to 192 blocks
  of `WorldGen::height` evaluations -- biome noise octaves per sample -- plus the horizon
  maximisation. The gather is what the march pours into; its ~2K MACs per probe sit at
  **~1-3% of the line** by construction counting, nowhere near 25%.

So the only GEMM in sight would accelerate ~2% of the bake while the march stayed serial.
That is Amdahl with the accelerator on the wrong side of the ratio -- the same shape ADR
0002 recorded one rung up: real hardware, real reachability, irrelevant placement.

## What would make it live again

A bake whose *samples* are precomputed rather than marched for -- i.e. a gather over a
world representation that already exists as samples per direction. In this tree, nothing
like that exists for the probe bake, and building one *to feed* a GEMM is the tail wagging
the Amdahl ratio. If a future rework of the bake (L2's BLAS work, if it passes) makes the
march free and leaves only consolidation, this entry becomes re-rankable -- and the first
cross-check is to recompute the two numbers above against the new bake profile, not to
revive this ADR by sentiment.

## What was not tried

No build, no bench: the entry's kill clause fires on counting alone, and the rules for
these four resource rows require the arithmetic to be done in the sandbox before a GPU is
asked anything. See [`docs/roadmap.md`](../roadmap.md), *the four idle resources*.

