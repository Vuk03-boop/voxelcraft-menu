> **Latest handoff:** Read [BATCH-LOG.md](BATCH-LOG.md) first. It records the decoupled-shadow overhaul, changed controls, superseded behavior, and unresolved work. Earlier validation/performance notes below describe their original batches, not acceptance of the latest code.

# Rendering experiments — all three tiers now implemented

**Current update:** [SURFACE-REPAIRS.md](SURFACE-REPAIRS.md) documents the subsequent authorized lighting/water fixes, current validation and new CLI controls. Repairs are on by default; the performance experiments below remain off by default. The original appearance is no longer the default control.

**Experimental source, not a release-certified build.** The compact schedule, separate glass pass and dedicated primary-shadow pass are implemented, independently selectable and off by default -- except the dedicated primary-shadow pass, which ships on and is forced on at every load. No register-count or FPS improvement has been established on the target GPU.

## How to enable them

Start the application, then open **Settings → Render lab** before Play:

| Menu option | CLI enable | CLI revert |
|---|---|---|
| Compact shade_hit / A-B | `--compact-shade-hit` | `--no-compact-shade-hit` |
| Separate glass pass | `--isolate-glass` | `--no-isolate-glass` |
| Dedicated shadow pass | `--shadow-pass` (default) | none: it warns and leaves the pass on |
| Soft shadows / high cost | `--soft-shadows` | `--no-soft-shadows` |

Use **Apply and save**, return to the title, then Play. Shader options lock after a session starts; restart to change them. Ordinary display/appearance settings remain available while paused. Restore Defaults during gameplay preserves locked shader options; on the title it turns all experiments off.

**Effects lab** contains the existing glass-reflection, water-look and wind-sway effects. These remain optional effects, separate from the default surface-repair layer. In particular, **Separate glass pass is different from Glass reflection**: separation changes where existing glass shading executes, not its optical model.

The shadow option necessarily separates secondary shading too:

| Separate glass | Shadow pass | Effective architecture |
|---|---|---|
| Off | Off | Combined resolve architecture, with the existing optional water-secondary prepass |
| On | Off | Main resolve excludes glass; a separate dispatch shades glass |
| Off | On | Primary shadow visibility → opaque/sky resolve → glass and water material dispatches |
| On | On | Same pass topology as shadow-only; glass is not dispatched twice |

These are full-screen **masked dispatch prototypes**, not sparse work lists. Empty-material scenes still pay classification/dispatch overhead. A scene without glass is not a meaningful glass-shading speed test.

## Tier 1 — compact material schedule

`SPEC_COMPACT_SHADE_HIT` retains the previous A/B arrangement: original `shade_hit_legacy` and a late-material `shade_hit_compact` schedule. The legacy material-first schedule is retained; both schedules now share the authorized surface-repair composition helpers. The experiment finishes gather/shadows/probes before material UVs, atlas sampling, tint, wetness and foliage variation. The batch-67 specular hoist and early LOD calculation remain early, and final multiplication order is preserved.

UV/texel values already stop being used before the original gather; the main carried material result is its three-component albedo. Moving work therefore does **not** promise the quoted four-to-six-register saving. The driver can reschedule, rematerialize or normalize both arms, or make the new one worse.

## Tier 2 — separate glass shading

`SPEC_ISOLATE_GLASS` removes the glass-shading branch from the **main resolve specialization** and creates a separate glass pipeline/compute dispatch. It is not merely a helper rename.

The shader uses the same `resolve` entry point with a **compile-time `RESOLVE_LANE` override**: 0 for main, 1 for glass-only, 2 for water-only. Distinct pipelines and dispatches are created for these lanes. A lane is not a runtime per-pixel rendering-quality choice.

Glass still calls its original `shade_glass`, preserving pane tint, straight thin-pane transmission, the existing optional reflection leg and the existing through-pane water approximation. The existing air/fog and underwater-medium tail executes once in that material's lane. Glass writes its final HDR pixels directly into the shared output texture; no extra intermediate HDR image or additional half-float round trip is introduced.

With only glass separation enabled, main resolve **still contains water's secondary shading**. Glass separation alone is not a promise of a lower general register ceiling.

## Tier 3 — dedicated primary-shadow visibility

`SPEC_SHADOW_PASS` introduces a real `primary_shadow` compute dispatch **after all primary visibility work, including recovered marching**. It reconstructs the same hit point and foliage-facing normal, applies the same `0.02 * voxel_size` bias and primary shadow distance, and invokes the existing hard or soft local sun-shadow code.

Visibility is stored in an **R8Unorm write-only storage texture**, distance map and an RG16Float blur target. Each in-bounds pixel is initialized, including sky, transparent pixels, heatmaps, unlit faces, disabled shadows and night. Fractional soft-shadow visibility is reconstructed in screen space, and `validation/check_decoupled.py` asserts the mask carries only 0 and 255.

Cached primary lighting schedules are derived from their original inline schedules. Their only lighting substitution is `lit = primary_sun[pix]` in place of the local hard/soft ray evaluation. Distant-shadow handoff, cloud shading, skylight gating, probes and material calculations remain in their original relative places.

**Secondary positions must not read a primary screen-space shadow.** Consequently, shadow-pass mode also sends water and glass pixels to their own material dispatches. Their secondary hits retain their own original ray budgets and local shadows. This makes local `shadow_ray`/`shadow_ray_t` calls unreachable from the opaque main lane, instead of simply relocating its first call while leaving secondary tracing there.

The new sequence is:

```
light envelope → tile select → march
  → optional Hi-Z / recovery selection / recovered march
  → primary local-shadow visibility (when enabled)
  → existing water-secondary legs (when enabled)
  → main resolve
  → separate glass resolve (when required)
  → separate water resolve (shadow-pass mode)
  → TAA → presentation
```

Sky and heatmap ownership remain with main resolve. Material writers finish before TAA. Both architecture options can be enabled together without duplicate glass writes.

### Resources, startup and timings

- New specialization bits: `FLAG_HI_ISOLATE_GLASS = 262144` and `FLAG_HI_SHADOW_PASS = 524288`; both participate in the cache mask and CLI/windowed frame mapping.
- The three shadow targets occupy **7 bytes per internal pixel** between them: approximately **13.84 MiB at 1920x1080**, plus write/read traffic.
- Group 3 carries that buffer. Existing group-0 layout and Frame ABI are unchanged; water's sampled/storage roles remain separated.
- A four-byte placeholder is used while disabled. Full storage is allocated lazily when rendering requires it, recreated/rebound for changed internal dimensions, and released back to a placeholder when no longer requested. VRAM estimates include it.
- Only requested glass/water/shadow pipelines are created. Specialized startup pipelines now derive from actual FrameParams, not an all-bits mask that would compile unrequested effects. The existing emitter-sibling warmup remains.
- **The existing `resolve` timing column measures the entire family**, not just the cheaper opaque dispatch. Query indices 8/9 bracket the new passes and their intervening work. Existing GPU-total and benchmark columns therefore do not hide work moved elsewhere. Debug labels distinguish the passes for vendor profilers; separate numerical per-pass timing columns were not added.

Allocation/device errors retain the renderer's existing fatal policy; no transactional GPU rollback is implemented. Compilation is synchronous and selected combinations can take time or exhaust resources. **Use a copied test world.** If a saved combination cannot compile, restart to the lightweight title and restore defaults before Play, or use the negative CLI flags.

## Historical tier validation (before the surface-repair pass)

The results in this section describe the preceding prototype delivery, not the final repair build. See SURFACE-REPAIRS.md for current results and limits. Pre-repair image logs are archived under validation/history/.

Python wgpu 0.32 on Vulkan **llvmpipe (CPU)**, not the native Rust wgpu/Naga 30 application:

- **86 Rust files parsed** without syntax errors; this is not type checking.
- Default and existing experimental compute-pipeline checks passed.
- The previous **2,560 production-helper fixtures** still match compact vs legacy and legacy vs original baseline with maximum channel error **0.0**. An in-memory positive-control marker confirms the compact selector is active.
- **240 uninstrumented split-vs-unsplit HDR image comparisons** passed with maximum channel difference **0.0** across separate basic, shared-water/compact, soft-shadow, existing-effects and compact-plus-soft profiles. Each profile compares all four architecture combinations at 8×8 and 17×9. Cases include sky, glass, mixed opaque/water/foliage, underwater, heatmaps, disabled shadows and night and occupied shadow-control data.
- **48 additional instrumented comparisons** passed. Atomic counters verified exactly one final HDR writer per pixel and **zero local shadow-tracer calls from split opaque resolve**, while unsplit positive controls did invoke local shadow tracing. Instrumentation exists only in the test's in-memory shader, never in the saved game WGSL.
- Pixel sentinels and visibility readbacks checked complete output/initialization, bounds and resize/rebinding. The soft-shadow occupied fixture exercises fractional visibility; no new soft-shadow algorithm was substituted.
- The cross-compute-pass timestamp descriptor pattern and original query indices 8/9 passed a separate GPU API check.
- **33 menu/experiment/architecture Rust tests are included but not executed.** Existing specialization inventory and probe-call assertions were updated for the cached schedules. Source guards verify title-only ownership, CLI spellings and unchanged original control code.

The first attempt to accumulate every existing effect plus all new pipelines in one test process was killed during compilation. Validation was then run in smaller fresh processes. The three proposed improvements **together with soft shadows** were validated in the combined profile; the entire effect stack with every visual experiment simultaneously enabled is **not** claimed validated. The incomplete log is retained and marked as such.

Synthetic final visibility is intentionally used here. These tests exercise real shading entry points and resource transitions, but are **not** complete world renders, target-driver image validation, native input tests, Rust compilation, register statistics or FPS measurements. The existing wind reconstruction discrepancy still reproduces; glass/water/wind appearance defects are not fixed by these performance switches.

## Run locally

With the project's normal Rust toolchain and sufficient build space:

```bash
cargo check --all-targets
cargo test --release --test settings_menu --test experiments --test split_passes --test spec_hi
cargo test --release --test batch102 --test water --test probe
cargo run --release
```

Headless flags work independently of saved preferences. Keep seed, camera, time, resolution, edits and all unrelated settings fixed. Use representative scenes containing glass and water, not only the default terrain view. Example comparison commands (add identical scene/journal options to each) -- the dedicated shadow pass is on in every arm below, so they isolate the glass split

```bash
cargo run --release -- --bench-frames 240 --no-isolate-glass
cargo run --release -- --bench-frames 240 --isolate-glass
cargo run --release -- --bench-frames 240 --shadow-pass
cargo run --release -- --bench-frames 240 --compact-shade-hit --shadow-pass --soft-shadows

cargo run --release -- --screenshot control.png --no-isolate-glass
cargo run --release -- --screenshot split.png --isolate-glass --shadow-pass
```

Alternate warmed A/B runs; compare **resolve-family time, GPU total and wall time**. For the soft-shadow comparison, enable soft shadows on both control and split runs. Use a vendor profiler to inspect the labeled main/shadow/glass/water pipelines, registers, spills, shared memory and occupancy. The full repository's `shaderstats` tool would need matching new layouts/overrides; its source was not included in this text export.

Current Python reproduction commands are in [SURFACE-REPAIRS.md](SURFACE-REPAIRS.md). The split harness now records a control first and runs one architecture per process to bound compiler memory. Original-baseline equality is retired for intentional visual repairs; current tests compare the repaired schedules and architectures with one another.

**No claim of 64 registers, four-to-six saved registers, a “huge” improvement or an affordable G5 budget is made.** These options now let you measure those hypotheses rather than assuming them.

## Delivery

The workspace retains only `voxelcraft-menu/` and `voxelcraft-menu.patch`; temporary tool installations, baseline copies and staging scripts are removed after validation.

The patch is **cumulative against the newer original ledger-102a export**, including the menu and all three experiments. Do not apply it on top of an earlier menu patch; use the updated tree or revert the earlier menu commit first. On the matching original baseline:

```bash
git apply --check /path/to/voxelcraft-menu.patch
git apply /path/to/voxelcraft-menu.patch
```

The export lacked Cargo.lock, original binary reference assets and some tool sources. Use your full repository's copies where available. Native build and target-GPU acceptance remain outstanding.
