# ADR 0001 — The bounce field carries no elevation form factor

**Status: decided and reverted.** Built in full on 2026-09-15 against batch 58, measured, and
removed. **This is a closed door, not unfinished work.** Roadmap `R2b` is deleted; the entry that
used to say *"the elevation half of the form factor, which is what batch 58 left on the table"*
was written by the session that then built it and found the table was the right place for it.

The code is not in git history under a commit of its own — it was reverted from the working tree
— so **this file is the only record, which is why it exists.**

---

## The decision

The baked ground bounce (batch 58, roadmap R2) stores a per-channel multiplier on `resolve`'s
`floor_rgb`. Its per-axis variation is **azimuthal only**: a +X face is coloured by the ground on
its +X side, and a +Y and a -Y face at one probe read the same value.

**That is physically wrong and it stays.** A ceiling receives nothing but bounce and an open floor
receives almost none. Correcting it is about forty lines and they were written. It is not being
shipped because **the term it redistributes is 7% of the light, and this world has almost no
ceilings to redistribute it to.**

## What was built

`cube_from_horizon` already computes `up`, the cosine-weighted sky fraction, and discards
everything but the cube and `believe`. The build returned a `Horizon { cube, believe, ground }`
and multiplied the bounce's mean by `ground[axis]`, where `ground` is the complement of the
*absolute* sky fraction, anchored by `GROUND_REF = 0.5` so that a wall in an open field keeps the
floor it was authored with:

| axis | absolute sky fraction | ground, after `/ GROUND_REF` |
|---|---|---|
| **-Y** (ceiling) | 0 — no sky lies below the horizon | **2.0** |
| **sideways** | `0.5 * rel` — half the hemisphere is above the horizontal | **1.0** |
| **+Y** (open floor) | `up`, which is 1 over flat open ground | **0.0** |

**Three design facts worth keeping even though the code is gone**, because each cost a wrong turn:

1. **It is not a lobe change.** `bounce_from_albedo` computes a weighted *mean*, so every lobe
   weight divides straight back out of it — a flat vertical lobe and a cosine one give the same
   magnitude and differ only in tint. The elevation term has to be a **factor on the mean**. The
   roadmap entry that proposed R2b said "replace batch 58's flat vertical lobes" and that phrasing
   was wrong.
2. **It has to be faded by `believe`, or it doubles the only light a cave has.** `up` is ~0 under
   rock, so an unfaded ground fraction reads **2.0 on every axis** exactly where `floor_rgb`'s own
   comment calls it *"the sole light in an unlit cave"*.
3. **Its control has to be a data control, not a `SPEC_` override.** The term folds into the
   multiplier the tap already reads, so the shader is byte-identical either way; `--no-probe-
   elevation` was a bake-time flag threaded through `Streamer`, and the purity claim is that the
   two *bakes* agree rather than that the two pipelines do.

## Why it was rejected — the three measurements

### 1. The energy budget: the floor is 7% of the light

`Config::ambient` is **0.08**, so `floor_rgb` is `(0.85, 0.90, 1.00) * 0.08` ≈ `(0.068, 0.072,
0.080)` in linear. Against `ambient_rgb = amb * 0.50` reaching ~0.5 and `sun_rgb` reaching ~0.52,
the floor is about **7% of the light on a lit surface**. Removing it entirely costs ~3% after the
tone map — three or four code values on a mid grey.

**So the whole of what this rung redistributes is 7%**, and redistributing 7% correctly cannot buy
more than 7%. That number should have been computed before the code was written; it is the
cheapest of the three measurements here and the only one that was available for free.

### 2. The ceiling census: 3.6% of the best frame, and 0 in three of eight

A diagnostic build in which **only the -Y axis** takes the elevation term — so any pixel that
moves is a downward-facing face — screened eight upward-looking cameras at 640x360 (230,400 px),
all at `--time 0.35`:

| camera | px moved | max Δ | share of frame |
|---|---|---|---|
| `--cam-height 4 --cam-pitch 35 --cam-yaw 300` | **8,304** | 9 | **3.6%** |
| `--cam-height 2 --cam-pitch 60 --cam-yaw 300` | 8,192 | 8 | 3.6% |
| `--cam-height 2 --cam-pitch 45 --cam-yaw 37` | 7,521 | 8 | 3.3% |
| `--cam-height 2 --cam-pitch 40 --cam-yaw 0` | 7,033 | 7 | 3.1% |
| `--cam-height 3 --cam-pitch 50 --cam-yaw 96` | 3,542 | 8 | 1.5% |
| `--cam-height 3 --cam-pitch 40 --cam-yaw 210` | **0** | 0 | 0% |
| `--cam-height 2 --cam-pitch 70 --cam-yaw 180` | **0** | 0 | 0% |
| `--cam-height 2 --cam-pitch 55 --cam-yaw 143` | **0** | 0 | 0% |

**A heightfield world has no ceilings by construction.** What exists is tree-canopy undersides,
and cave roofs — and a cave roof is where `believe` is 0, so the term is switched off there
anyway. At the camera *chosen to maximise the gain*, the full term moved 51,500 px of which the
ceiling half was 8,304: **16% gain, 84% loss.**

### 3. The amplified diff: 2–5 code values, almost all of it darker

Per-pixel signed difference between the term on and off, at 960x540. "Moved" is any channel
differing by at least one code value; the mean is over moved pixels only.

| camera | moved | mean abs Δ | p95 | max | darker | brighter |
|---|---|---|---|---|---|---|
| `default` | 58.6% | **2.87** | 7 | 28 | 44.6% | 13.3% |
| `lod` | 29.0% | **5.49** | 19 | 41 | 27.0% | **2.0%** |
| ceiling camera | 22.2% | **1.87** | 4 | 9 | 12.7% | 9.4% |

**At `default` the average changed pixel moves under three code values** — below what anyone can
see flipping between two stills, which is how the rejection actually happened: the user looked at
the pair and said *"I do not see the difference?"*, and they were right.

**At `lod` it is visible and it is the worst case**: a ×10-amplified diff is almost pure red over
every sunlit slope, 27% of the frame darker against 2% brighter. A uniform dimming with no
compensating gain anywhere in frame.

## The mistake this ADR also records

The session first reported this as *"84% a darkening of upward faces"* from **pixel counts**, and
called the trade bad. That was the wrong instrument: 136,869 pixels moving by 2.87 code values is
not a darkening anybody sees. `lessons.md` already says to *rank a truncation by what is in the
band, not by the size of the thing it truncates*, and batch 41 already has the paired example —
1,309 px at max delta 1 against 5,442 px at delta 86. **A pixel count is a reach, not a
magnitude.** Quote a magnitude beside every count, or the count argues for whatever you want.

## What would reopen this

**One thing, and it is not a better implementation.** If `Config::ambient` — or whatever term
stands in for indirect light at the time — ever becomes a materially larger share of a lit
surface, the 7% ceiling on this whole rung moves and the arithmetic has to be redone. R3's
multi-bounce field is the most likely cause, because a field that *replaces* the ambient rather
than multiplying into it (batch 56's shape) puts far more energy through this term.

**Nothing else reopens it.** Not a finer probe grid, not more azimuths, not a better form factor —
the term was derived correctly and the tests below proved it was. The limit is the energy budget
and the shape of the world, and neither is a code problem.

## What the build left behind that was kept

The verification, because it outlived the feature. `tests/probe.rs` keeps the three batch-58
discriminators the ceiling work was written against, and the break-checks that calibrated them:
flattening the azimuthal lobes fails two of three, and **mirroring** them fails only
`the_brighter_ground_is_on_the_brighter_axis` — a world where every wall takes its colour from the
ground behind it, which no picture reads as wrong.

The two elevation tests were reverted with the code. Their shape is recorded here in case the
retry condition above ever fires: an open field must read **2 / 1 / 0** down / sideways / up, and
a buried probe must read **1 on every axis**, because the second is what keeps a cave lit and an
inversion that returned the sky fraction where it meant the ground fraction passes every
batch-58 test.

---

*Measured on the target machine (RTX 3050 Laptop, GA107) against `84fca9e`, batch 58 as shipped.*



