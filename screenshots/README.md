# screenshots/

Tracked on purpose: `CLAUDE.md` says these are referenced by the batch docs, and since batch
22 that is actually true of every file here. Fourteen were not, and were linked from their
batches' own documents rather than deleted -- they are the visual record of shipped work, and
an unlinked capture is a bookkeeping defect and not clutter.

**This index is a directory listing, not an explanation.** What each figure shows and why it
was taken lives in the batch document that links it.

| files | batch | explained in |
|---|---|---|
| `biomes.png`, `building.png`, `cave.png`, `landscape.png`, `night.png`, `water.png` | the project's general figures | `README.md` |
| `lodfix/` (4) | an LOD coverage fix from before the batch ledger began; `ab.png` is the two-panel comparison | nothing -- it predates the ledger, and see below |
| `crossfade/` (3) | batch 7's stochastic LOD dissolve | `PERF.md` |
| `batch9_*` (4) | 9, biomes | [`docs/terrain.md`](../docs/terrain.md) |
| `batch10_*` (5) | 10, clouds | [`docs/sky.md`](../docs/sky.md) |
| `batch11_*` (4) | 11, god rays | [`docs/sky.md`](../docs/sky.md) |
| `batch12_*` (3) | 12, the biome tint | [`docs/shading.md`](../docs/shading.md) |
| `batch14_*` (3) | 14, foliage | [`docs/foliage.md`](../docs/foliage.md) |
| `batch18_*` (3) | 18, the coarse meadow | [`docs/terrain.md`](../docs/terrain.md) |
| `batch19_*` (4) | 19, the grazing footprint | [`docs/water.md`](../docs/water.md) |
| `batch20_*` (5) | 20, shallow-water damping | [`docs/water.md`](../docs/water.md) |
| `batch21_*` (3) | 21, the water lattice | [`docs/water.md`](../docs/water.md) |
| `batch21b_*` (5) | 21b, the sea sampling the glass layer | [`docs/water.md`](../docs/water.md) |
| `batch25_*` (2) | 25, the fill wave schedule | [`docs/water.md`](../docs/water.md) |
| `batch26_*` (3) | 26, the water mottle that never shipped | [`docs/water.md`](../docs/water.md) |
| `batch52_*` (2) | 52, the submerged reach and the seam it exposed | [`docs/water.md`](../docs/water.md) |
| `batch64_offscreen_shadow_*` (3) | 64, terrain off screen casting a shadow again | [`docs/lod.md`](../docs/lod.md) |

`lodfix/` is the one group with no document. It predates the ledger and nothing written down
says which defect it is the A/B for; the frames show the same terrain with LOD stepping
resolved differently, which is consistent with the coverage rule `CLAUDE.md` describes under
"Streaming and LOD", but that is an inference and is not recorded as fact anywhere. Kept for
that reason rather than deleted.

**New captures do not belong here by default.** `--bin harness capture` writes the reference
set to `target/harness/`, which is ignored; put a file here only when a document links it.



