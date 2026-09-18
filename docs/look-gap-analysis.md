# Look gap analysis — current engine vs the two reference frames

Source material: user-attached engine captures `uploads/Screenshot 2026-09-16 084217.png`
(coastal shallows), `230709.png` (lake + green terrace), `230721.png` (sandy delta +
stone peak), `230730.png` (treeline + peak across water); references
`docs/look-reference-1.png` (marshy river, fishing rod) and `docs/look-reference-2.png`
(lakeside tall-grass meadow, cumulus).

The references are a **proprietary modpack's custom texture pack** (user-owned design
reference, 2026-09-17: *"the textures aren't Minecraft, they are custom made... still
proprietary, I have them as reference for design"*); ours is vanilla-adjacent voxel art
with a purely procedural atlas. The modpack's painted art is therefore both off-limits
for copying AND outside what the engine's texture model is by design — the design
question here is which *properties* of the references to land procedurally, not any
subset of their art. The goal is not pixel parity; it is closing the *vibe gap*: which
single properties most separate the two looks, and which are cheapest to land inside the
G1 look-first contract (every arm carries its measured number for the later reclaim
pass; richer procedural textures are sanctioned).

## Ranked easiest → hardest to close

1. **Sky and cloud tonality.** Ours reads lilac/washed near the horizon; the references
   read cooler clean blue with whiter, heavier clouds. Pure parameter work: the 95d
   `SPEC_SKY_LOOK` override exists, the haze/zenith knobs exist, cloud patch span/relief
   are constants in `common.wgsl`. Cost: near zero. Almost closes shot E1/E3's sky row
   alone.

2. **Cinematic grade in the blit.** The references share a filmic print: gentle S-curve
   contrast, mild green desaturation, warm highlights / slightly cool shadows. The grade
   selector + `grade_warm` plumbing landed this batch — adding a `cine` preset is a
   blit-shader change with no worldgen or marching surface. Cheap, and it shifts every
   screenshot at once.

3. **Water: animated ripple normals + absorption tint.** The references' water is the
   single loudest difference: animated normal-mapped micro-waves everywhere, olive-brown
   absorption where it is deep over vegetation, sky reflection only at grazing angles,
   slight brightening where water meets banks. Ours is static, uniformly cyan-blue, flat
   specular. This is shader-only work on the water path (perf thread is paused, not
   forbidden; the number gets ledgered). Highest vibe-per-change outside the grass.

4. **Ground-cover texture richness.** R2's foreground is 3+ grass expressions — green
   blades, dry tan tips, occasional flower pink — where ours is one green per biome.
   Sanctioned rich-procedural work: hue/value jitter + a 2–3 species tint ramp on the
   carpet/surface palette, plus per-voxel tint mixing on grass block tops. No density
   change in worldgen, so the marcher sees identical geometry.

5. **Tree canopy brightness relief.** Reference canopies show sunlit yellow-green clumps
   floating on dark olive interior. Ours is one value (E3 leaves). Clump-modulated
   brightness + hue jitter + extra darkening in canopy interior (existing attr+light
   channel can carry a cheap interior mask) gets most of it. Medium cost, large effect on
   every forest shot.

6. **Distance haze layering.** R2's horizon has a blue-lilac airlight band; R1's
   background trees dissolve into warm haze. Closest hook is the 95d haze knob, but the
   terrain airlight ramp (colour + height falloff) likely needs its own small entry.
   Cheap-ish, high realism payoff on E2/E4-style vistas.

7. **Cloud form: puffy cumulus volume.** Ours are flat wispy patches; the references are
   bulbous cumulus with shaded undersides. Needs domain-warped noise + edge falloff +
   under-shading in the cloud patch shader. Still shader-only, but the biggest
   sculpting lift on this list.

8. **Soft contact shadow under canopies.** The dark pool under reference trees does
   heavy lifting. Real AO/shadow is a user-stopped perf topic; a look-only stand-in is
   darkening the carpet/ground under canopies in the attr+light attribute at worldgen
   time (bake, not trace). Medium-hard, watch the attribute budget.

9. **Natural relief on bare stone.** E2's peak is uniform grey slabs. Strata tint +
   slope-dependent rock noise would break it up. Not in the references' composition,
   but cheap and it sweetens every mountain shot.

Explicitly out of scope: fauna, held-props, weather dynamics — the gap items the
references carry that do not map to the G-look entries.

## Suggested next arm order

Sky tonality (1) + cine grade (2) as one cheap batch ("lastly, the grade"), then water
(3) alone with its measured number, then foliage richness (4) + canopy relief (5)
together. 6 and 7 whenever the earlier arms have earned their keep.

---

# The batch-100 sheet read — where we actually are, read off the twelve-tile matrix

Read of `out/lookbook.png` at HEAD `6642ee6` (36 captures on the fixed engine, `put_px`
clip confirmed: no label ink in any tile). This section supersedes the *guesses* in the
ranked list above wherever they became *measurements*.

## What the arms actually did, seen rather than counted

- **`--water-look` is the one arm a blinded reader picks out of the sheet.** At
  SEA-SUNSET it takes the water from static cyan-blue to deep teal, softens the sun
  lane, and banks the shore — the direction of reference R1's water. **Verdict holds:
  ship.** Two stains against it, both new from the full-res read: *(i)* the ripple
  field **moirés into concentric rings** at every range shown — SEA near/mid band,
  the island ring fields of SUNRISE-MOUNTAINS — so "distance fade" alone is not the
  cure, the ripple *period vs pixel footprint* is half the problem (the fork's
  per-arm table: `--water-look` max Δ161 at `lod`, 531,585 px at `lattice`); *(ii)*
  the per-arm gate reads exactly **0 at `glass-sea`** while `coastline` reads 408,940 —
  sea through glass takes the one-lookup path and gets no ripple normals. Both are
  named gates before any default-on talk.
- **`--grade cine` splits exactly as predicted.** In the two golden-hour groups it is
  the second-most-visible arm (richer sun lanes, snow pops at altitude) and reads
  *toward* the references' print. At MIDDAY it crushes an already-dark hillside into
  near-black steps — same frame, worse than base. **Verdict holds: sweep, and the
  strength knob is the difference between these two readings.**
- **`--sky-cool` is un-judgeable on this sheet.** It swaps the *day* gradient, and none
  of the three views spends <10% of its frame on midday blue — the golden-hour groups
  mix the swap into the sun palette, and MIDDAY's sky is a top-edge sliver. MAE 1.07
  is consistent with a working arm photographed where its subject isn't. Its per-arm
  zeros decompose as expected (night/deep-water exact, by construction of the
  day-half-only gate; `cave`/`lamps` moved 1 Δ through the ambient — documented, not a
  leak). Verdict revised: **"nearly invisible" was partly a framing artifact** — the
  reach question re-opens only under a sky-bearing midday view (below).
- **The two foliage arms are absent from the sheet.** Row by row, FOLIAGE, CANOPY and
  the tile this round called G2-PAIR are optical twins of BASE at all three views.
  (Designation repaired at batch 102: that tile armed exactly FOLIAGE + CANOPY under
  G2's name since 658e31d; it now arms G2's real pair, `--grass-dense --wind-sway`, and
  this bullet's "G2-PAIR" means the albedo pair it photographed.) The measured 0.42/0.18
  MAE is what invisibility looks like in numbers; the sheet shows *why* — there is
  nothing for the albedo to ride on: MIDDAY's near field carries ~2 tufts per terrain
  step. **Scrap-or-rebuild confirmed visually, not just numerically.**
- **FOG-DENSE is the sheet's quiet feature.** At SUNRISE-MOUNTAINS it layers the
  horizon exactly like reference R2 (islands dissolving into warm-lilac air), and it
  costs no arm. FOG-LOW is subtle; keep both knobs for the final tune.
- **GODRAYS-HALF/OFF read as no-ops in these three framings** — there is no
  low-sun-through-canopy view where shafts live. Not evidence about the arm; list it
  under framing debt.

## Where we need to be — the remaining gap, re-ranked from the sheet

1. **Vegetation density and shape (the generator, verdict of both forks).** The
   hillside is a terraced dirt stair with dark speckle where the references are
   continuous multi-height grass with reeds at banks. Every later G knock-on (dapple,
   shafts, contact shadow, canopy relief) multiplies by this. **Batch 101's work.**
2. **The lookbook cannot photograph its own target.** All three views are elevated
   (beachfront, aerial, drone-over-hillside) while both references are shot at *waist
   height* inside the world. A waist-height-in-grass view is required to rank G2 and
   to ever judge the midday blue `--sky-cool` buys. **Part of batch 101's bill.**
3. **Water's two gates above** (moiré cure via period/footprint shaping + distance
   fade; ripple through glass). Small, bounded, shader-only.
4. **Cine strength knob.** One uniform and a parse line; converts the sweep verdict.
5. **Sky-cool re-read under the new midday framing** — no code until judged.
6. Then the deferred list as it stood: cumulus form (7 above — the deck's streaks are
   the sheet's weakest golden-hour element), canopy/under-canopy relief **on real
   canopy** (5), contact shadow (8), haze layering tune (6), strata relief (9).

## What the sheet says we already have right

Sunset palette + sun lane against the island silhouettes; the aerial vista's
composition; horizon haze layering accessible via the existing fog knob; and — via the
fork's sha256 reconciliation — the whole 97–100 package is a *no-pixel-moved* edit to
the shipping build, so nothing here is defended at the control's expense.

