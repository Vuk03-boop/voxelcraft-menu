> **Latest handoff:** Read [BATCH-LOG.md](BATCH-LOG.md) first. It records the decoupled-shadow overhaul, changed controls, superseded behavior, and unresolved work. Earlier validation/performance notes below describe their original batches, not acceptance of the latest code.

# Surface repairs — first implementation and acceptance notes

This is the visual-repair pass requested after the September 18 gameplay screenshots. **The changes are implemented, but their final appearance and cost still need a native gameplay retest.** No screenshot from the user's exact shoreline has been reproduced here.

The three performance experiments remain independently selectable and off by default. The ordinary lighting/water repairs below are enabled by default; existing saved experimental preferences are preserved. No save/journal format or Frame/bind-group layout migration is needed.

## Implemented

### Stronger, tighter soft shadows

- Normal soft-shadow aperture is now **1.5 times the existing sun-angle constant**, instead of 6 times. This is an initial artistic calibration, not a new physical sun-size measurement.
- Repaired soft shadows trace every surrounding sample to the requested local shadow distance. The old blocker/miss-based shortened distance is kept only in the legacy-lighting comparison path.
- Standard quality is still **four samples total** (centre + three). Optional high quality is **eight total** (centre + seven).
- Hard and soft rays use the same material policy. Primary cached visibility and secondary water/glass shading still use their own correct hit positions.
- Fully covered synthetic receivers remain visibility zero at both quality levels; a soft edge no longer requires weakening the fully occluded interior.

### More readable block lighting

All four lighting schedules—legacy, compact and their cached counterparts—use the same new composition helpers:

- Reduce **sky fill only**, with a hemisphere-dependent factor: `0.60 * mix(0.35, 1.0, clamp(normal.y*0.5+0.5,0,1))`.
- Increase the direct-sun coefficient from 0.52 to 0.78. Surface orientation still comes from `N·L`.
- Stop applying the fixed axis-based face multiplier to the entire lighting result.
- Apply corner AO and the existing probe-cube occlusion to indirect ambient, not as a second sun-shadow mask.
- Preserve the separate ambient/bounce floor and coloured block-light gathering. Ambient Light = 0 is still not the same as disabling skylight.

This is a deliberately limited recalibration of the existing lighting model, not a new GI solver or an independently stored decomposition of every probe contribution. The optional experimental replacement-probe-ambient path remains its own model and is not claimed visually calibrated by this pass.

### Glowstone no longer blocks sunlight

- `block::casts_sun_shadow` separates the sunlight rule from visible opacity and collision. Glowstone remains an opaque, solid, warm emitter; other lamp materials retain their previous casting rule.
- Both GPU shadow tracers request the independent sunlight traversal policy. Mixed nodes descend to the real material: stone behind glowstone is still hit. FULL-root shortcuts cannot bypass this check.
- Existing exact emitter metadata is uploaded in attribute flag bit 32. The existing resident-emitter specialization folds the new material filter out of no-emitter worlds.
- CPU **skylight** pour/flood also passes through glowstone, otherwise the skylight gate could leave a dark patch after the ray shadow was removed. RGB block-light obstruction is unchanged.
- Glowstone has a viewable emissive surface contribution independent of incoming sunlight/AO. It still has ordinary solid geometry and may participate in contact AO.
- The distant terrain envelope is generated from terrain heights, not player-placed glowstone; no glowstone silhouette is added to that envelope. General edit-aware distant terrain shadows are not introduced here.

### Water-interface and shared-sample protection

- Actual water-to-water interior interfaces are rejected during traversal, including chunk boundaries. Occupancy is checked separately from material identity: a uniform-water attribute is **not** evidence that an empty cell contains water.
- Genuine exposed water side faces remain visible. This does not flatten all placed water into a global sea plane or delete waterfalls/edges.
- Shared secondary samples are accepted only for compatible top-water pixels: no sky/land silhouette, mismatched face, different surface height, or excessive depth separation within the donor patch.
- Incompatible pixels evaluate their own existing reflection/refraction legs. This is a correctness fallback, **not free**: it adds donor checks and can increase local ray work and compilation/register pressure.
- Visibility encoding, hit reconstruction, TAA reconstruction and the actual water surface height are unchanged. The new traversal still produces the same kind of visibility key.

These are fixes for concrete interface/sharing defects, but they are **not yet proof that the reported one-block shimmering strip is gone**. A legitimate exposed side or another view-dependent reflection issue can look similar in a still image.

### Less regular ocean detail, same main waves

- `wave_field` and `wave_octave` are unchanged, including amplitudes, direction schedules, timing and distance/grazing filtering. The original shoaling/depth modulation is retained too.
- Only the **additional Water Look ripple layer** receives smooth world-space phase variation, breaking up its two globally coherent interference trains without per-voxel random seams.
- The water body's tint samples the average/coarsest existing atlas mip rather than repeating a visible tile across every block face.
- Existing fine-ripple distance filtering remains active.

If the remaining pattern comes from shoaling or the main wave spectrum, it needs a separate targeted change after comparing captures. No speculative four-extra-depth-rays-per-pixel smoothing or wholesale wave replacement was added.

## Controls

The existing Settings → Render lab **Soft shadows** checkbox still enables soft shadows. The new quality, diagnostic and comparison controls are **CLI-only startup options**, not additional persisted menu settings:

```bash
# Normal repaired appearance; retain your existing scene/journal arguments.
cargo run --release

# Choose softness independently of sample count.
cargo run --release -- --soft-shadows --sun-softness narrow
cargo run --release -- --soft-shadows --sun-softness normal
cargo run --release -- --soft-shadows --sun-softness wide

# Eight samples instead of four. This can cost substantially more.
cargo run --release -- --soft-shadows --soft-shadow-hq
# --no-soft-shadow-hq restores four samples.

# Visual comparisons, independently reversible.
cargo run --release -- --legacy-lighting
cargo run --release -- --legacy-water
# --repaired-lighting / --repaired-water restore the repairs.
```

The softness multipliers are narrow=1, normal=1.5, wide=3. The legacy-lighting path uses the old 6x aperture and shortened sample budgets. **Legacy flags are visual comparison modes, not old-binary bit-exact reverts**: the glowstone casting/skylight policy is a shared material change.

Diagnostics:

```bash
cargo run --release -- --surface-debug normals
cargo run --release -- --surface-debug shadow
cargo run --release -- --surface-debug direct
cargo run --release -- --surface-debug indirect
cargo run --release -- --surface-debug water-faces
cargo run --release -- --surface-debug reflection
cargo run --release -- --surface-debug refraction
```

Use `--surface-debug off` to restore normal rendering. Normals/shadow/direct/indirect inspect surface lighting; shadow shows the computed sun visibility (including existing cloud/distant composition where applicable), not an independent local-only mask. The indirect diagnostic is before the final probe-cube ambient multiplier. Water-faces colours the water's geometric normal; reflection/refraction isolate the respective composed water contribution. Air fog and underwater post-composition are bypassed for diagnostics, but presentation tone mapping/grade can still affect the displayed colours. Use a natural grade while inspecting them. Debug views are not performance benchmarks.

## Validation on the delivered code

Python wgpu 0.32, Vulkan **llvmpipe CPU**, not the native Rust wgpu/Naga 30 game:

- **87 Rust files syntax-parsed**. No Rust type checking or native unit execution.
- All 11 default shader entry-point pipelines compiled; additional wind/water-look/glass-reflection march and resolve pipelines compiled.
- **2,560 helper fixtures** matched repaired legacy vs repaired compact lighting, maximum channel error 0.0. The compact positive-control marker still reached all 128 samples.
- **54 uninstrumented image comparisons** passed for shared-water + compact shading across glass-only, shadow-only and both architectures, at 8×8 and 17×9, maximum HDR channel difference 0.0.
- **54 additional instrumented comparisons** passed across the same architecture choices with full-resolution water: every pixel had one writer, split opaque resolve called neither local shadow tracer, and unsplit controls reached tracing.
- Image cases include mixed materials, sky, glass, underwater, heatmaps, shadows disabled, night, occluded data and top-water surfaces. Independent processes use recorded controls stamped with shader/fixture hashes and adapter identity; records are temporary and are regenerated before comparison.
- Targeted real WGSL tests passed for visible/non-casting glowstone, stone behind mixed glowstone, FULL-root policy, 4/8-sample full occlusion, sky directionality, unoccluded direct-light composition, emissive glowstone, exposed water sides, interior/chunk-boundary water rejection, and empty cells with uniform-water attributes.
- A separate donor test **accepts** coplanar water and **rejects** sky, side faces, different heights and stone; safety is not implemented by disabling all reuse.
- All seven diagnostic shader specializations compiled individually. UI/blit pipelines and source guards passed. The main wave functions match the preceding delivered source.
- Four new native contracts are included in `tests/surface_repairs.rs`; related source-contract tests were updated for the intended changes.

### Resource-limit failures and remaining acceptance

Keeping many LLVM-compiled pipelines alive in one process exceeded the sandbox's process memory allowance. The image harness now runs one architecture per process. **The repaired soft-shadow + compact + shared-water combination still exceeded that limit while compiling its control's second pipeline.** Its complete image comparison and the entire simultaneously enabled effect stack are not claimed validated. Soft-shadow helpers/material rules passed separately, including high quality; that is not a substitute for the full combination.

Earlier bring-up and pre-repair logs are archived under `validation/history/`. They are historical evidence, not successful tests of the latest shader. Current failure logs are explicitly marked incomplete.

Native compilation, real-window interaction, target-GPU performance, temporal stability at the reported shoreline and final artistic calibration remain outstanding. The known wind intersection/reconstruction discrepancy still reproduces and is outside this pass.

## Local acceptance checklist

Use a copied test journal. Shader compilation and GPU allocation still follow the renderer's existing synchronous/fatal-error policy.

```bash
cargo check --all-targets
cargo test --release --test surface_repairs --test experiments --test split_passes --test spec_hi
cargo test --release --test light --test glass --test water --test water_sec --test batch102 --test batch97
```

1. Test isolated stone blocks at low and high sun angles; compare hard shadows, normal soft shadows and high quality.
2. Place floating glowstone above stone, then stone behind glowstone along the sun direction. Recheck after removing/replacing blocks and after restart.
3. Visit the original shoreline. Keep main waves on; compare repaired water vs `--legacy-water`, Water Look on/off, shared water vs `--no-water-sec`, and TAA on/off while moving slowly.
4. Check legitimate placed-water sides, underwater transitions and chunk/LOD boundaries. If the strip persists, capture a short clip plus water-faces/reflection/refraction views of that same spot.
5. Recheck your preferred ACES + Cinema grade after establishing neutral lighting.
6. Compare performance experiment combinations within this same repaired build, with all other settings fixed. Measure full resolve-family time, GPU total and wall time—not an idle-repaint FPS reading or only the opaque dispatch.

Python reproduction (`wgpu`, `tree-sitter`, `tree-sitter-rust`):

```bash
python3 validation/check_menu.py
python3 validation/check_shaders.py
python3 validation/check_compact.py
python3 validation/check_surface.py
python3 validation/check_diagnostics.py
# Run separately to bound compiler memory. Repeat for ownership.
python3 validation/check_split.py shared control
python3 validation/check_split.py shared glass
python3 validation/check_split.py shared shadow
python3 validation/check_split.py shared both
```

For memory-limited Mesa runs, prefix with `MESA_SHADER_CACHE_DISABLE=true`. The harness writes temporary `.surface-reference-*.json` controls; remove them after testing.

## Delivery

`voxelcraft-menu.patch` remains cumulative against the newer original ledger-102a export. It includes menus, the three performance experiments and this repair pass. **Do not apply it on top of an older menu patch.** Replace with the delivered source tree, or apply it to the matching original baseline. Patch forward application is verified byte-for-byte before cleanup.
