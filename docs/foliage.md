# Foliage

Grass tufts as cross-quads in the visibility key, and the `override` that makes switching them
off free. **Consolidated from batches 14 and 15.** The LOD 0 boundary and the failed fold shapes
are in [`errors.md`](errors.md).

---

## The mechanism

`march_chunk`'s water path is the template, and the shape was copied rather than invented: a
per-chunk flag, the leaf's `attr` entry cached in a register across the leaf's four voxels,
`leaf_block(attr, uc)` at the `solid` point, and `solid` cleared to keep the ray going.

**On a foliage voxel the marcher clears `solid` first and only then tries the blades.** That
order is a correctness fix, not a style choice -- the other way round leaves an opaque cube where
a tuft should be.

**Two normal states, not four.** `normal:3` in the visibility key took states 6 and 7 for the two
cross-quad planes, which is why the key's normal field is now **full**: a third primitive shape
must spend the six free bits in the voxel field instead (bits 6 and 7 of each byte, since the
voxel packs as 8 bits per axis where 64^3 needs 6).

A cross-quad is **double-sided** -- there is one blade and the eye may be on either side -- so
only *which plane* is stored and the facing is settled in `shade_hit` against the view ray. That
is what makes two states enough where a one-sided quad would have needed four.

**The alpha cutout is hashed and anchored to the surface point, not to the pixel** -- the same
bargain batch 7's dissolve strikes, and for the same reason: a threshold that is a pure function
of where the blade edge is gives the same answer from every camera position, so TAA integrates
the noise into coverage instead of fighting a pattern that swims.

**`face_uv` runs u along local x for both planes.** On plane A local z tracks x and on plane B it
mirrors it, so one expression covers the pair and the blade texture is not stretched differently
on the two halves of the same tuft.

---

## Scope decisions

- **No AO and no per-corner gather.** `gather_face` derives its tangents from `axis`, which a
  diagonal does not have. A tuft takes the flood's own answer at its voxel, flat -- and it can do
  that *because* it does not occlude, so what is stored there is the light arriving at the blade.
- **Transparent to the light flood and to shadow rays** (`opaque: false`), and excluded from
  `voxel_occludes` -- without which every blade would cast an AO shadow on the ground directly
  under it and darken exactly what the batch exists to decorate.
- **`solid: false`.** The third block to answer the occupies / blocks-light / blocks-player
  question as yes/no/no.
- **One voxel tall, LOD 0 only.** Both to measure before doubling.
- **A per-biome `grass_scale`, never a `GRASS` gate.** A gate on one block id leaves any biome
  not using it silently bare and nothing fails. Desert and tundra are bare here **because their
  entries say so**.

---

## The `override`, and why it has to reach `foliage_aware`

**Guarding the `cross_quad` call site on a specialized bool folds nothing, in either operand
order.** The marcher still has to recognise a tuft in order to step *past* one -- a shadow ray
passing `skip_foliage = true` needs the block-id lookup even though it never intersects a blade
-- so the driver keeps the intersection alive for a branch it cannot prove dead.

Only killing the **recognition** strips it, and that is sound exactly when the world holds
nothing to recognise. So the constant is keyed on `FLAG_FOLIAGE`, a property of the *generator*,
and never on `skip_foliage`, a property of one ray.

**Three source shapes fold identically when the override is false and differ when it is true**,
which is the shipping build:

| source shape | `resolve` bytes |
|---|---|
| batch 14, no override at all | 233344 |
| `var foliage_aware = false; if SPEC_FOLIAGE { ... }` | **234240** -- shipped |
| `SPEC_FOLIAGE && ...` | 234624 |
| `... && SPEC_FOLIAGE` | 234624 |

The `if` is 384 bytes cheaper than either `&&`. **None of the three reaches 233344** -- declaring
the override costs the shipping build 896 bytes however it is written, which is where its +0.03
ms comes from.

**`SPEC_FOLIAGE` is deliberately *not* ANDed with its flag, where `SPEC_TINT` is.** Adding
`(frame.flags & FLAG_FOLIAGE) != 0u` keeps `march`'s fold and costs `resolve` **42 KB and eight
registers**: the extra term leaves `foliage_aware` a run-time value and `resolve`'s inlined call
tree is large enough that the driver stops propagating it.

What makes the missing run-time test safe is that **`TALL_GRASS` is not in `block::HOTBAR`**,
pinned by a `const _: () = assert!` beside `FLAG_FOLIAGE`, so the generator is the only source of
ground cover and there is no third state to catch. **Do not restore it without re-measuring
`resolve`.**

**`ensure_spec` builds both `march` and `resolve` from one constant list**, because
`foliage_aware` lives in `common.wgsl` -- a run where the two disagreed would march a tuft the
shading pass cannot see.

---

## The control

**`PRE_BATCH14 = 0xbc5c5c4e9b53c3c3`**, measured through `tests/biome.rs`'s existing `world_hash`
on the untouched tree **before a line of the batch was written**. The test also asserts the
enabled path *differs*, so a placer that silently did nothing fails rather than passes. Reusing
`world_hash` rather than copying it was deliberate -- a second hash would be a second definition
of what "the world" is, and the two would drift.

**`--no-biomes` forces foliage off**, in `WorldGen::with_options`. Not a shortcut: `--no-biomes`
is *defined* as the pre-batch-9 rule set and ground cover did not exist then, so letting it
survive would break `PRE_BATCH9`.

**`--no-foliage` is free since batch 15's fold. Before it, the control cost +1.24 to +2.07 ms --
the largest regression this project has shipped.** Batch 15 recovered 1.25 ms at the default
camera and 2.08 down a coastline, **all of it in the control and none of it in the game**, which
got 0.03 ms *slower*. Still worth shipping: every future batch's A/B leans on these controls, and
one that costs a millisecond quietly taxes every measurement taken with it. It is a measurement
batch wearing an optimisation's clothes.

**Nothing here makes drawing grass cheaper; it only makes *not* drawing it free.** The shipping
build still carries the whole intersection, because it needs it.
---

# Leaf cutouts (batch 38)

**A second foliage primitive, and it shares nothing with the first but this document.** Ground
cover above is a cross-quad: two authored planes inside a voxel, alpha-tested against the atlas.
A canopy is the opposite shape of problem -- the block is *mostly* there and wants holes in it --
and the answer is not a texture test at all.

## The mechanism

A leaf block is a **4^3 sub-voxel occupancy mask** carved out of its cube. When the DDA steps
into a voxel whose id is `LEAVES` or `PINE_LEAVES`, `cutout_march` runs a second DDA through the
64 micro-cells inside it, at a quarter of the scale, with the same entry snap and the same
"which boundary is nearest" step as the outer loop. A micro-cell is solid if a hash of its
integer world position falls under `SPEC_LEAF_FILL`. Hit a solid one and the ray stops there; cross
the whole block without hitting one and the block was air to that ray, so it steps one cell and
carries on.

**Four is not a taste.** 4^3 is 64 cells, which is one native `u64` and the same branching factor
every other level of this tree uses -- so the micro-march is arithmetic the module already
compiles rather than a second kind of traversal. It is also the largest grid whose coordinate
fits the key.

**There is no mask table and no buffer.** The research recommended 32 canonical templates in a
256-byte constant table; a hash per *visited* micro-cell is cheaper than that, because the micro
DDA touches two or three cells and never sixty-four. It also costs no bind-group entry, which a
table would have, and it makes `LEAF_FILL` a real float constant that `--reference` can sweep
rather than a bit-pattern nobody can tune -- which batch 43 then did.

## The fill fraction, swept (batch 43)

**`SPEC_LEAF_FILL` is 0.62.** It shipped at **0.72** for five batches, authored against the
research's 0.45 on an argument -- our leaf blocks are a solid 5x5 stamp where the paper's
canopies were already sparse, so the same fraction on a denser arrangement reads as a tree that
has been shot at. The argument was sound and the number it produced was too high.

**The sweep needed a crop the fixture did not have**, which is why batch 43 added the
`canopy` vantage: a wooded crest at eye level against open sky, lit from the front at
`--time 0.35`. Every other vantage shows leaves against ground or against other leaves, where
the carve is a speckle in a green mass. Only a silhouette shows whether an outline breaks up at
all, and that is what this constant decides.

**Seven builds, one constant apart, and the two halves of the frame want opposite directions.**

| fill | against sky (the crest) | against ground (a crown from above) |
|---|---|---|
| solid, pre-38 | a clean blocky edge -- a green box | flat mass |
| 0.45 | strongly eroded; the crown loses body | **visibly moth-eaten**, dark speckles throughout |
| 0.55 | clearly broken, mass retained | mild speckle |
| **0.62 -- ships** | **clearly broken, mass retained** | **subtle notches only** |
| 0.72 -- shipped before | modest breakup, close to the solid edge | near-smooth |
| 0.82 | two or three notches | smooth |
| 0.90 | **indistinguishable from a solid cube** | smooth |

- **The silhouette wants it low.** At 0.82 and 0.90 a crest is the same edge a solid cube gives,
  so the whole of batch 38 is invisible where a tree is most visible. The breakup only becomes
  legible below about 0.65 -- which means the old 0.72 sat at the edge of the range where the
  feature reads at all.
- **The crown's interior wants it high.** Every carved cell that opens onto shadowed interior
  reads as a dark speckle from above, and 0.45 is exactly the "tree that has been shot at" the
  old comment was guarding against -- now seen rather than predicted, which is the difference
  between an argument and a sweep.

0.62 is where they meet: a clearly broken skyline with only a subtle interior speckle. **The
old number's reasoning was never wrong about the failure mode, only about where it starts**, and
that is worth keeping in mind before moving it again -- the floor is real and it is near 0.5.

**`--no-leaf-thin` restores 0.72**, bit-exact at all sixteen vantages against
`voxelcraft-pre43.exe`, and free: `shaderstats` reads `march` 21,504 bytes at 72 registers and
`resolve` 615,424 at 96 in **both** directions, because the two builds differ in one immediate
operand and in no code at all. That is [`gpu.md`](gpu.md)'s fourth mechanism -- a cost that is
neither occupancy nor code size -- in the direction where it costs nothing.

## What it bought that was going to be a separate problem

- **The atlas alpha conflict dissolved instead of being worked around.** `textures.rs` builds
  `LEAVES` with `pxt`, so its alpha is 255 meaning *tintable*, and leaves are in the tint set.
  Batch 14's reversal -- for a foliage block, alpha is opacity and the tint mask is implied 1 --
  could never have extended to a block that needs both meanings at once. Holes in the tree
  discard no texel, so the alpha stays the batch-12 tint mask for every block and `make_atlas` is
  untouched.
- **Nothing has to filter a cutout at range**, so `Castano` coverage renormalization and
  footprint-scaled thresholds are both retired before anyone builds them. A cutout that is
  geometry has no alpha to mip.
- **Dappled shadows fell out for free.** A shadow ray meets the same holes the camera does,
  because they are the same geometry. **This is why there is no `skip_cutout` beside `skip_water`
  and `skip_foliage`**: those two exist because a shadow ray wants to pass *through* water and
  grass, and a leaf is the one case where the ray should meet exactly what the eye meets.

## The six bits, and the key being full

A 4^3 cell needs 2 bits an axis. The voxel field packs 8 bits an axis where 64^3 needs 6, so bits
6 and 7 of each byte have been free since the key was designed -- **six needed, six free, one
pair per byte.** `make_key`'s layout did not move by one bit.

**The key is now full**, and `CLAUDE.md`'s invariant says so. `normal:3` has been full since batch
14 took states 6 and 7 for the cross-quad; the voxel field is full now. A fourth primitive shape
has to widen the key or spend depth precision.

None of the three research documents saw this. The one that answered the key question
concluded the six bits were "structurally incapable of holding secondary geometry" -- which is
right for a second *hit*, and wrong for a sub-voxel *coordinate*, and that distinction is what
made the batch implementable at all.

## Where an override may sit, which cost more than the feature

`hit_t` reconstructs the hit point from the stored face rather than from the key's 24-bit depth,
and `taa` reprojects through the same helper, so the two must agree to the bit. A carved hit
lands on a micro-cell's face, so `hit_t` had to change -- and **every shape that put
`if SPEC_LEAF_CUTOUT` on that path left the *disabled* build one ULP from the binary it is
supposed to reproduce**: 201 pixels of 230,400 at a max channel delta of 1, on terrain only, with
the sky byte-identical.

Three shapes were tried -- an early `return` inside `hit_t`, a selected offset with one `return`,
and the branch hoisted into its own dispatch function behind a plain `let` -- and all three
failed. Removing the branch while *keeping* the extra parameter and the widened decode came back
bit-exact, which is what proved neither of those was ever the problem.

**What ships has no branch there at all.** `march_chunk` records the **entry** micro-cell on every
solid hit, not only a carved one, so the micro plane reduces to the cube's own face *exactly*:
the near face is cell 0 at offset 0, the far face is cell MICRO-1 at offset MICRO, and
`MICRO * (vs / MICRO)` is `vs` on the nose for every power-of-two stride. One expression serves a
carved block and a whole one. `hit_t_does_not_branch_on_the_cutout_override` is the guard, and
deleting it costs nothing today and costs a bit-exact control the next time someone tidies.

The awareness flag was the same finding a second time. Written the way `foliage_aware` above it
is -- the second `if` outside the first -- it left **5,120 bytes of `resolve`** with the constant
false. Nesting both tests inside the override's own block took it to 1,664 and spelling out
`is_cutout_id` rather than calling it took it to 1,024. **`march` folded to byte-identical
throughout**; it is the deferred pass, which reaches `march_chunk` through water's secondary
rays, that is fussy. Same family as the foliage entry above, and the same moral: the constant has
to reach every use, not merely the expensive one.

## The control

`--no-leaf-cutout`, in `SPEC_MASK`, free by the fold: **14 vantages, 0 pixels differing,
identical file hashes** against `voxelcraft-pre38.exe`. `shaderstats` folds `march` from 21,504
to 17,536 bytes and `resolve` from 291,200 to 269,440, with registers 72 to 58 and 80 to 72.

The positive half is **34,986 pixels of 921,600 at `default`** -- 3.80% of the frame, MAE 0.1531,
max 194 -- and **2,508 of 921,600 at `lod`**, max 16, every one inside a single bbox in the near
field, because at 240 blocks up almost everything on screen is already coarse.

## What it leaves

- **`LEAF_FILL` was swept in batch 43 and is 0.62.** It shipped at 0.72, unranked, for five
  batches; the sweep is below.
- **The LOD cliff.** The cutout is gated to `vs == 1.0` because a micro-cell inside a stride-2
  voxel stands for eight blocks, but `coarse_trees` stamps `b.leaf` at every stride up to 8 -- so
  a porous near canopy meets a solid far one across a cross-fade that partitions rays rather than
  blending samples. The still-frame half is bounded (the `lod` number above); the motion half is
  unmeasured and **no vantage in the fixture crosses the band**.
- **What the cutout costs `march`, measured in batch 45 and not before.** `--no-leaf-cutout` at
  the `canopy` vantage is **-0.323 ms +/- 0.005 of a 3.138 ms `march`** (t = -65.68), with
  `resolve` -0.185 and the frame -0.515. So the 4^3 micro-march is **10% of that pass** at the
  vantage built to look at a canopy -- real, bounded, and **not** the empty-space-acceleration
  collapse it had been suggested to be. The other 2.8 ms is ordinary DDA on the most chunk-dense
  frame in the fixture (143 chunks). Worth knowing before anyone reads `canopy`'s high `march`
  figure as a leaf problem: most of it is not.
- **The light flood is untouched.** A leaf still stops it: `BlockDef::opaque` stays true, and a
  per-block attenuation scalar is a separate question with its own convergence argument. The
  *sun* dapples anyway, for free, because that is a ray and not the flood.



