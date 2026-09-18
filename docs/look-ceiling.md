# The ceiling — what this engine can and cannot look like on today's hardware

Written at the user's direct question (batch-101 preface): *"you know my engine — what is
its limit visually to achieving [the modpack references] without destroying fps?"*

Measured frame (fork, RTX 3050 Laptop GA107, 1920×1080, hi-z + TAA, 128 chunks):
**GPU 7.36–7.62 ms** (march 2.42–2.51 · resolve 3.98–4.12 · taa 0.50 · select 0.16 ·
hiz 0.11 · shaft 0.11), wall median 8.2–8.4 ms ≈ 119–122 fps, hi-z off → GPU ~10.1–10.4
(march ~5.4). All five 97-arms together measured **+0.077 ms resolve (t = 1.61,
indistinguishable from zero)** at the worst look vantage. resolve is ~54% of the frame;
march is ~1/3. The standing contract (look-first, no floor): *every arm carries its
measured number; the reclaim pass is later*. So "without destroying fps" reads as: keep
the frame inside ~+2 ms, i.e. **≈ 9.5–10.5 ms ≈ 95–110 fps on this GPU at 1080p**.

## The honest matrix: reference property vs. engine

### Tier 0 — already paid for (≈ 0 ms, sanctioned)

- **Richer procedural texture variety** (species ramps, per-face value/hue jitter,
  dither-pattern sides/bottoms of dirt): select-only shading terms in the class batch 97
  measured free. The modpack's painted *art* is untouched; the *variety* is reachable.
- **Wind sway on cross-quads** (tufts/reeds oscillate by hashed time-offset): a UV-time
  term. This is the single highest vibe-per-byte motion cue the references have and we
  lack, at ~zero cost.
- **Cloud banks/relief tuning already shipped dark** (batch-95 arms) + fog layering knobs
  (FOG-DENSE proved on the sheet).

### Tier 1 — in budget (estimates, each to be measured per the contract)

| Item | Mechanic | Estimate at 1080p |
|---|---|---|
| **G2 density: multi-height tufts, 3–5× population** | more cross-quad cells in near field; A5-style coarse shell so mid-range doesn't go bald | +0.3…+1.0 ms on grass-heavy view (waist view will prove it) |
| **Reed stands along banks** | waterline cross-quad columns, biome-gated | negligible–0.1 (sparse) |
| **Ripple period/footprint cure** (old wave field + new ripple) | footprint-shaped amplitude/period — likely nets *negative* (less near-field spec work) | ~0 or better |
| **Water reflections, quarter-res** (reuse deferred secondaries, P12's machinery) | one deferred secondary ray class on water hits | +0.2…0.5 on water-heavy |
| **2.5D cumulus** (deck lobe warp, under-shade, edge falloff) | screen-space, sky pixels only | +0.3…+0.8 |
| **Tree-aware probe bake → contact pools + dapple** | bake-time only (trees added to the occlusion march; shafts texel knob A8 unlatched at 92) | **0 GPU**; bake +200…400 µs/chunk on the idle threads |
| **Micro-relief normal variation on dirt/grass/stone** | face-normal jitter, shader-only | ~0.1–0.2 |

Sum of the Tier-1 middle estimates: ≈ +1.0…+2.6 ms → ~9.4–10.9 ms ≈ 92–117 fps. **Not
destroyed**; measured item-by-item, anything landing catastrophically is shelved with
its number recorded (the contract's own rule).

### Tier 2 — the true walls (what this stack can only fake, stated so nobody re-plans them)

1. **Hand-painted texture art and authored models.** The modpack's hero assets —
   painterly grass blades, curved reed geometry, overlapping latin-plane foliage —
   are hand-authored. Engine-defining wall: 1-block voxels + procedural atlas; the
   3-normal visibility key is **full** (states 6/7 taken by the cross-quad at batch 14),
   and the voxel field's 6 spare bits were spent at batch 38 (leaf micro-cell). New
   *shape classes* need a key widening — engine surgery, not a batch. Cross-quads can
   *suggest* the blades; they cannot be their reference models.
2. **Soft shadow rays at full res.** The reference's washed-soft sun shading is traced.
   A second march per visible pixel at 1080p on a GA107 is not +2 ms, it is the march
   again at its floor (+3…+5 in grass-heavy views) — that *is* destroying fps on this
   hardware. The reachable 80%: canopy/terrain sun-shadowing at **quarter-res deferred**
   (the P12 machinery, perf thread currently user-paused — it becomes a look candidate
   only if dapple proves the value cannot live in the probe bake).
3. **Screen-space sheen/bloom and motion-blur sheens.** Cheap to add, but they are not
   "the reference's lighting", they are its post. A small bloom in the blit is a taste
   call parked for the user — named so it does not accrete silently.
4. **Caustics.** A9 measured zero and dropped it; the references do not lean on it.
5. **Anything proportional to view distance** (more chunks, denser far foliage).
   Chunk budget is the frame's biggest dial; 128 is the measured working point.

## The answer in one paragraph

This engine can reach, at ~95–110 fps on your hardware: **density** (the real gap),
**water character** (teal/absorption/bank, once the ripple is footprint-shaped),
**horizon layering**, **dapple and contact pools** (via the bake, free at runtime),
**motion** (sway), **tenor** (grade/sky knobs with strength dials), and **variety**
(richer procedural faces). It cannot reach, in this architecture at this fps class:
the modpack's *hand-painted art per se*, curved authored plant models, and full-res
traced soft shadows — the soft-shadow look is approximable at quarter-res if the probe
bake's version proves insufficient. Those are the three walls; everything else on your
reference sheets is on the achievable side of the arithmetic and is now batch 101+'s
planning input.

## Batch 101 scope, locked by this ceiling

1. **G2 generator rebuild** — multi-height tuft stamping (2–3 expressions in taller
   bands), 3–5× density in the near field, a coarse tuft shell at LOD 1/2 in A5's
   lineage (rank-thinned proxies) so mid-range doesn't go bald; reeds on banks.
   Flagged dark by default; the 21-vantage control stays bit-exact while armed off.
2. **Waist-height-in-grass lookbook view** (the MIDDAY group's successor/4th view:
   eye height ~2 blocks inside a grased bank of the coastline) — without it, G2 cannot
   be ranked, and `--sky-cool`'s midday blue cannot even be seen.
3. Adjacent smalls queued after Tier-1 evidence: ripple footprint cure; cine strength
   knob; sky-cool re-read.

