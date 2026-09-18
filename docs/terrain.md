# Terrain, biomes and the coarse levels

Worldgen, the biome field, the altitude rules, and the proxies the coarse LODs stand in with.
**Consolidated from batches 9 and 18.** The *rendering* side of biomes (the tint) is in
[`shading.md`](shading.md); defects are in [`errors.md`](errors.md).

---

## The palette snaps and the shape blends

That split is the whole design. `biome.rs` is one 3x3 grid of temperature x humidity read two
ways:

- **`band()`** thresholds it into a `BiomeId` -- the **palette**: surface block, subsurface, leaf
  id. It changes at a line.
- **`band_weights()`** blends the same grid into three floats -- the **shape**: `mountain_amp`,
  `snow_line`, `tree_line`. It changes over a couple of hundred blocks.

A palette seam is a coastline; a height seam is a cliff running the length of a border. Measured
across 17,874 borders, the amplitude steps 1.844 of a 165 spread and the terrain steps **3
blocks -- less than the worst step anywhere in the world**, which is the calibration that makes
that a statement about the blend rather than about the amplitudes.

**`BAND` 0.22, `BLEND` 0.18, and `BLEND < BAND` is load-bearing.** The blend is two smoothsteps
centred on the same thresholds the palette snaps at, so a border column is exactly half of each
neighbour's shape while already wearing one of the two palettes. If the smoothsteps overlapped,
the three weights would stop summing to one and the blend would no longer be an average of the
table. `const _: () = assert!(BLEND < BAND)` pins it.

**The blend is centred on the boundary, not on the band interiors.** Interpolating between band
*centres* leaves a desert only fractionally desert-shaped, because typical hot-band noise sits
well short of the extreme -- the amplitude would never actually reach 45.

---

## The field's frequency is a constraint, not taste

**0.00055 and 0.0007 -- wavelengths near 1800 and 1400 blocks.** Coarse chunks resample the field
at stride 2^n, so a biome much smaller than a few hundred blocks breaks into speckle at stride 8,
exactly the way tree placement did before `coarse_trees` existed.

What says the frequency is right is that the boundary fraction scales **linearly with the
stride** -- 1.09% at stride 1, 5.56% at stride 8, 5.1x for an 8x stride. A boundary is a curve,
so doubling the spacing doubles the band of cells straddling it; a field too fine has no curve
left and saturates instead.

**Speckle is 0.0054% and deliberately not zero.** Where both thresholds cross, four cells meet at
a point and a lattice sample inside one can disagree with all four neighbours. That is a corner
of the partition, not a feature too small to resolve.

**Two of the six biomes are ribbons and cannot be otherwise.** Plains and savanna live in the
middle temperature and humidity band, and the middle band of a threshold partition of smooth
noise is a strip around an isoline. Any test wanting "a patch of biome X" has to attribute
columns rather than look for a uniform square.

**`biome::simplex2` is `fastnoise-lite`'s OpenSimplex2 transliterated and pinned bit-exact
against it.** It exists so that the WGSL copy is a transliteration of something *tested*, and
`WorldGen` calls it -- it is the field, not a reference beside the field.

---

## The three altitude rules

Sitting on a column's height, in this order, all keyed to the column rather than to any
configured value:

1. **the tide line** gives `SAND`
2. **the snow line** gives `SNOW` over `STONE`
3. **the tree line** gives bare `STONE` -- above it nothing grows

Below all three, the biome's own pair.

**Snow is an altitude rule, never a biome's palette.** The first table gave the cold dry biome a
`SNOW` surface and 14% of the world then read as a single white sheet from the tide line to the
peak -- an alp rather than a mountain. So the tundra wears lichen and the snow line does the
work. Three bands, one profile for every biome: ground, scree, white cap.

**Both lines read the column's own height and biome**, the way the shore rule reads the
`SEA_LEVEL` *constant*, so LOD 0 and the coarse levels cannot reach different answers.

**Both tree placers gate on `b.surface`, never on `GRASS`.** That one token is the difference
between a desert with palms and a desert silently bald at every LOD. What makes it *checkable* is
that the desert has palms standing on sand -- with no treed non-grass biome the gate guards
nothing and no test can tell. `trees_grow_on_every_biomes_own_surface` normalises trunk rate by
each biome's own `tree_scale`: a broken gate reads exactly **zero**, where *coverage* would move
by hundredths of a per cent.

**Gravel is excluded from the surface voxel now that a surface can be stone**, because the coarse
levels have no gravel pass and a mottled rock band at LOD 0 would disagree with its own proxy.

**`mountain_amp` runs 45 to 210 and plains keeps 88.** 88 was the old global constant, so plains
is what `--no-biomes` collapses to and the control is a table row rather than a magic number. The
old world topped out at 209 internal, the new one at 295. The ceiling is set by
`slopes_stay_walkable`: at 210 the worst adjacent step is 4 against a bound of 8.

**`WorldGen::column_field` samples the field once per column into a `[ColumnBiome; 4096]`** and
threads it through the height, the palette and both tree placers -- for LOD 0 and the coarse
levels through the **identical code path**, with only the stride differing. That shared path is
what makes the altitude rules agree across a transition rather than merely agree on average.
Asking three times would triple the field's cost.

**`--no-biomes` switches off the whole layer, not just the field.** It returns plains with
`i32::MAX` for both altitude lines, in one early return in one function. Gating only the field
would leave the snow line free to fire on a tall plains peak, and the control would stop being
exact for a reason that has nothing to do with biomes.

**Internal world Y is 0..512; displayed Y is internal minus `math::Y_OFFSET` (64).**
`worldgen::SEA_LEVEL` is 128 internal (displayed 64); every column below it floods to it.

---

## The coarse levels stand in with proxies

Coarse chunks **resample the same noise at stride 2^n** rather than aggregating their children.
For terrain that is not a compromise but the mechanism: `column_field` below is one code path
and only the stride differs, so the snow line and the tree line cannot reach different answers
at two levels. **The player's edits are the one thing it cannot produce**, and they are
aggregated separately -- [`persistence.md`](persistence.md).

### Proxy trees

A tree is 7 blocks tall and 5 wide, so at stride 2+ it cannot be generated the way LOD 0 does it,
and sampling the placement hash on the coarse lattice would miss all but 1 column in `s*s`.

Each coarse voxel column instead asks whether **any** fine column it covers would hold a tree,
and the probability is scaled by the proxy's own footprint so the **canopy area** matches LOD 0
rather than the tree *count* -- one coarse voxel already shadows `s*s` columns, so scaling by
count carpets the world in leaves at stride 8.

`coarse_canopy_density_matches_lod0` pins aggregate coverage to within **1.6x** of LOD 0 at every
level; `per_biome_canopy_matches_lod0` bounds each biome separately and is looser at **2.0x**.
The gap between those two bounds is why the 1.25-1.7x per-biome drift is a documented consequence
and not a failing test.

**The tree shape is shared across biomes and only the leaf id changes**, because `coarse_trees`
scales proxy density by one `LOD0_CANOPY_COLUMNS` constant and a second canopy shape would need a
second one to keep the parity measurement meaningful.

### The coarse meadow

The same idea for ground cover, and far tighter, because a meadow stands for one voxel's worth of
*colour* rather than for a canopy's area.

It stamps at `min(1, ground_cover(..) * MEADOW_SPREAD)` -- **the same function LOD 0's tuft
placer calls**, so the two densities cannot drift -- and `coarse_meadow_density_matches_lod0`
bounds the ratio to `1.0..=MEADOW_SPREAD`, a band derived from the constant rather than fitted.

- **`MEADOW_SHADE = 0.59`** (`textures.rs`) -- how far the coarse surface is pushed toward the
  carpet's colour.
- **`MEADOW_SPREAD = 1.75`** (`worldgen.rs`) -- the stamp rate. A blade is vertical and hides the
  columns *behind* it too, so the fraction of columns carrying a tuft understates what the eye
  sees by exactly this factor. Batch 18's first sweep improved monotonically with no minimum
  because the paint was being asked to make up a factor that belonged in the placement instead.

**It runs *after* `coarse_trees`, and that is not optional**: the proxy placer tests for
`b.surface` and would stop seeing a column this had already repainted.

**Mean error against a LOD 0 ground truth fell from 15.03 code values to 1.02.** Three things it
does not fix are in [`errors.md`](errors.md), and batch 92 took the one that is one atlas slice.

#### The side face, armed (batch 92, roadmap A8 piece 4)

`--meadow-side` stamps `MEADOW_SIDE` instead of `MEADOW` -- **one id in one `if`, the gates,
the cover curve and the coin shared** -- so a steep coarse slope shows the carpet's edge on
its side faces instead of LOD 0's bright grass fringe: the new atlas slice is the grass-side
tile under the same `MEADOW_SHADE` scalar, its fringe drawn with the carpet's own recipe
and tint mask -- the *recipe* is shared, the mottle is not: `hash` mixes the layer into every
`n`, so no two slices are byte-equal by construction (batch 94 corrects this sentence, which
first claimed they were), and the structural pin lives in `tests/batch92.rs` -- dirt below
untinted by the grass-side rule. The id is **appended** per the table rule (23, after the
lamps), so `def`'s clamp on an unknown id lands on a grassy proxy in either generation of
the table -- which is the property scene's `lamps` comment records for the last row.
Worldgen-content class, same as A5: no flag bit, no shader, and the unarmed world is the
pre-92 world block for block (`tests/batch92.rs` sweeps four coarse chunks for exactly the
id swap and nothing else). The entry's two siblings stay open on purpose: the albedo cannot
know the view angle, and the residual is almost entirely the proxy trees, which is its own
batch.

### Measuring a coarse level at all

**`--streaming-factor 8` is the ground truth**, and it is the only honest target: it draws the
same ground at LOD 0 out to 512 blocks, which is exactly what the coarse band is imitating. It
streams 4000-14000 chunks in 1.7-6.6 s. **`--max-lod 0` does not work** -- it starves the
interning budget and returns a disc of terrain in an empty sky.

**Price a compensation through the medium it will be seen through.** The carpet reads as a *hue*
change against bare ground (0.741/0.745/0.820 far, 0.632/0.663/0.706 close), and a per-channel fit
measured **worse** than a single scalar -- the channels separate because the haze lifts blue
toward the sky colour faster than red or green, not because the carpet is a different colour.

---

## New block ids are appended, never inserted

`const _: () = assert!(block::WATER == 13)` in `render/mod.rs` is the only thing tying that id to
its literal in `common.wgsl`. Batch 9 added snow, podzol, pine leaves and lichen as 14-17 and four
atlas layers as 15-18; layers are independent slices, so every existing block renders identically.
Batch 14 added tall grass as 18 and batch 18 the meadow as 19.

---

## Controls

| flag | reproduces | cost |
|---|---|---|
| `--no-biomes` | the pre-batch-9 rule set: plains everywhere, amplitude 88, no snow or tree line. Pinned by a hash in `tests/biome.rs` taken before the batch was written. Clears `--no-tint`, `--no-foliage`, `--no-meadow` | free |
| `--no-meadow` | the pre-batch-18 coarse ground: bare grass where a carpet is missing | free **by construction** -- batch 18 adds no shader code |
| `--streaming-factor F` | not a control -- a LOD ground truth | -- |

**Batch 9 costs +0.06 ms of GPU and +15% worldgen per chunk**, of which only 175 us of 277 is the
biome field itself; the rest is 7.3% more terrain to fill.



