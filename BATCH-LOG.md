# Batch log and code handoff

**Current handoff: Decoupled Shadow Architecture, Phase 1/2.**

These are local handoff labels, **not invented upstream batch numbers**. Earlier upstream batch names in comments retain their historical meaning.

At your request, testing/build attempts stopped during this handoff. The code is being delivered for you to build and finish on the GA107. **This is not a release-certified or performance-accepted build.** Known incomplete work is listed below rather than hidden behind old passing logs.

## Quick status

| Area | Code status | What is NOT established |
|---|---|---|
| One-ray primary shadows | Implemented in `shadow.wgsl`, with host resources/dispatch | Native build, register count, occupancy or <0.8 ms |
| Bilateral penumbrae | Horizontal + vertical, normal/plane-depth rejection, bounded radius | Final quality in your world; exact MSSSS/paper equivalence |
| Water variance | Optical normals and reflection composition now use removed variance | GA107 sparkle/shimmer eliminated |
| Ripple cutoff / flat atlas | Implemented | Native temporal stability |
| Glass | Transport corrections implemented; initial checklist was already satisfied | **Glass repair remains unconfirmed; new fixture failed** |
| Emergency section | Deleted from `CLAUDE.md` | Nothing else in that guidance was followed as a new task |
| Attribute lookup / subgroups | Deliberately untouched | No optimization claim |
| Frame / visibility ABI | No layout change intended | No native ABI/full-suite execution |

## H00 — Inherited menu and experiment work

**What was already in the starting tree**
- Title/pause/settings UI, draft Apply/Cancel/Defaults, persisted preferences and title-only experimental switches.
- Compact material scheduling A/B.
- Separate glass/water material dispatches under the earlier shadow-pass option.
- An f32-buffer primary-shadow cache. The earlier soft mode still fired multiple geometric rays.
- Resolve-family timing that includes moved work rather than reporting only opaque shading.

**Not done in this handoff**
- No menu redesign, world browser, save-system rewrite, grass/wind reconstruction fix or attribute-lookup redesign.
- No claim that the previous performance experiments achieved a register or FPS win.

**Status**
- Retained where compatible, but the old optional shadow architecture is superseded below.

## SR01 — Previous surface-repair pass, now partly superseded

**What that delivery changed**
- Reduced the soft-shadow aperture and extended surrounding rays to the requested shadow distance.
- Offered four/eight geometric samples.
- Rebalanced sky fill versus direct sunlight and limited AO/probe occlusion to indirect lighting.
- Added a non-sun-shadow-casting glowstone rule without making the block invisible or non-solid.
- Added water/water internal-interface rejection and a compatibility guard for shared water-secondary samples.
- Modified the extra ripple phase and averaged the body tint, while retaining the main wave field.

**What it did NOT solve**
- The cost of firing multiple shadow rays.
- Confirmed native glass transport defects.
- Confirmed distant water/shoreline sparkle.

**Hardware result supplied by you**
- RTX 3050 Laptop / GA107: soft shadows added approximately **4.55 ms**, with FPS falling **128 → 81**.
- Glass remained broken; water shimmer persisted.

**Disposition**
- Treat that hardware report as the acceptance result. Its multi-ray soft-shadow method is **replaced**, not described as successful because earlier synthetic checks passed.
- Lighting calibration, glowstone policy and water-interface protections remain unless explicitly changed below.

## DS01 — One geometric ray and a sampled primary mask

**Files**
- New: `src/render/shaders/shadow.wgsl`
- New: `src/render/shadow.rs`
- Updated: `src/render/mod.rs`, `common.wgsl`, `resolve.wgsl`

**Implemented**
- `primary_shadow`, a 16×8 screen dispatch reading final visibility.
- Sky, nonreceivers, sun-backfacing surfaces, night, disabled shadows and heatmaps produce zero blocker distance without firing a ray.
- Each eligible primary opaque receiver issues **one `shadow_ray_t` query**.
- R16Float stores positive receiver-to-blocker distance along that ray; zero is the miss/nonreceiver sentinel. Contact hits are clamped to a small positive representable value so they are not mistaken for misses.
- This distance is **not camera-space receiver depth minus an unrelated ray parameter**. `shadow_ray_t` already measures the receiver-origin-to-blocker separation needed here.
- Removed the `sun_vis_soft` multi-ray function and its calls.
- Primary opaque resolve now selects the cached lighting schedule and reads an **R8Unorm visibility texture**.
- Secondary water/glass hits retain one ordinary hard shadow query at their own position. They do not borrow the primary screen-space mask or fire the old soft loop.
- The shadow family runs after final recovered marching, before water-secondary work and resolve. Putting it before recovered visibility would read incomplete hits.

**Resource changes**
- Raw distance: R16Float, 2 bytes/pixel.
- Horizontal intermediate: RG16Float, 4 bytes/pixel, containing occlusion and propagated filter support.
- Final visibility: R8Unorm, 1 byte/pixel.
- Total nominal storage: **7 bytes/internal pixel**, approximately **13.84 MiB at 1920×1080**, excluding driver alignment.
- Separate sampled/storage bind groups for each role. No texture is simultaneously bound for reading and writing in its own pass.
- Resources are recreated when internal dimensions change. VRAM accounting includes them.
- The device now requests native adapter-specific texture-format features and checks sampled/storage support for these formats. There is **no R32 fallback**: unsupported adapters receive an explicit initialization error.

**Architecture/UI consequence**
- The dedicated primary shadow path is now mandatory, including hard-shadow mode.
- Saved `shadow_pass=false` is normalized to true. `--no-shadow-pass` warns and does not restore primary inline tracing.
- Separate glass/water lanes are mandatory to keep secondary shading out of opaque resolve.
- The obsolete glass-split and dedicated-shadow checkboxes were removed from Render lab. That page now contains **Compact shade_hit** and **Soft shadows**. Old preference/CLI keys remain readable for compatibility; they do not disable mandatory isolation.
- The ordinary **Sun shadows** control still disables shadow queries.

**Not done**
- No measured register reduction, occupancy proof, or under-0.8-ms result.
- No claim of near-100% occupancy: lack of material shading does not prove that hardware property.
- No improvement to `shadow_ray_t`'s existing coarse/chunk-order blocker-distance approximation.

## DS02 — Screen-space bilateral penumbra reconstruction

**Implemented**
- Horizontal then vertical filtering.
- Radius derived from blocker separation, projection scale and receiver depth, bounded to **4 pixels normally / 8 in HQ**.
- Subpixel contact radii stay sharp.
- Rejection at sky/nonreceiver pixels, discontinuous normals and incompatible receiver planes.
- Plane-distance rejection, rather than raw camera-depth equality, so a sloped coplanar surface is not inherently rejected.
- Both centre and neighbouring blocker support contribute. Lit receivers can receive penumbrae; the algorithm does not only lighten already-shadowed pixels.
- The intermediate's second channel preserves horizontal support for the vertical stage.
- Hard-shadow mode converts raw hits to binary visibility without soft reconstruction.
- `--sun-softness narrow|normal|wide` scales the reconstruction aperture.
- **`--soft-shadow-hq` now increases the screen-space kernel bound. It does not select eight geometric rays.** The old `SOFT_TAPS` compatibility override may still be present in the source/table but no longer drives a ray loop.

**Timing changes**
- Query capacity expanded from 16 to 22.
- Added pairs 16/17, 18/19, 20/21 for raw shadow, horizontal and vertical stages.
- HUD adds shadow-ray and filter milliseconds.
- Those stages remain inside the existing resolve-family bracket. GPU total must not add them a second time.
- Timestamp storage remains below the existing 256-byte counter offset; no `GpuFrame` expansion.

**Not done**
- No temporal shadow history, checkerboard tracing, disocclusion reconstruction, blocker search beyond the existing one-ray result, shared-memory tiling or dynamic quality controller.
- This is a bounded bilateral Phase 1/2 approximation, **not a claim of a complete/reference MSSSS implementation**.
- Thin/off-screen blockers, layer boundaries and separable-filter artifacts still require native inspection.

**Checks completed BEFORE your stop-testing request**
- Python wgpu / Vulkan llvmpipe compiled the new formats and pipelines.
- Diagnostic-only counters observed one blocker ray per eligible pixel and zero local shadow-tracer calls from opaque resolve.
- Checked 8×8 and 17×9 sizes, complete writes, disabled/night/sky/backface/heatmap cases.
- Controlled filter fixtures checked contact/full-umbra preservation, penumbra extension onto lit receivers, and sky/depth/90-degree edge rejection.
- See `validation/decoupled-results.txt`.
- These are not native Rust or GA107 results.

## WA01 — Water variance and ripple filtering

**Files**
- `src/render/shaders/resolve.wgsl`
- `src/render/mod.rs`, `src/config.rs`

**Implemented**
- Preserve `wave_field`'s removed slope variance and carry a roughness value in the internal shader `WaterCtx` structure.
- Smoothly blend optical and traced-reflection normals toward the geometric normal as unresolved variance grows.
- Include lost extra-ripple energy in the roughness estimate.
- Extra ripple amplitude fades over 48–96 blocks, is zero at/after 96, and is zero at grazing `abs(rd.y) <= 0.08`, ramping to full eligibility at 0.20. Existing wavelength/pixel-footprint fades remain.
- Blend unresolved composed reflection toward a broad sky term, affecting the traced contribution as well as analytic sky. This is a cheap approximation, not a full rough-reflection convolution.
- Force the runtime atlas build's water mottle to 0.0. A nonzero `--water-mottle` request is consumed and warned about, not applied.

**Preserved / not done**
- Main wave spectrum, phase clock and `wave_field`/`wave_octave` definitions were not rewritten.
- No mesh/water-height displacement or stochastic multi-ray water sampling.
- No screen-space reflection history or temporal specular accumulation.
- No native footage proving the shimmer is gone. The variance response and sky-blend strength remain calibration candidates.

## GL01 — Glass diagnosis and transport corrections

**What was already correct on inspection**
- Transmission used `skip_water=false`.
- The water-hit branch used the requested analytic `WATER_BODY` / medium-light / water-Fresnel composition.
- Secondary traversal already requested `skip_glass=true`.

Simply rewriting those lines would not have diagnosed your remaining native defect.

**Changed**
- Transmission reach now covers at least the frame's far range instead of treating every miss beyond the fixed 192-block glass limit as sky.
- Glass reflection no longer borrows `water_reflect_leg`, whose water-skipping policy is wrong for a pane reflecting an ocean.
- Added a glass-specific reflected trace that sees water and uses the analytic water-interface branch for it.
- Oriented the pane normal against the incoming view direction for back-side viewing.
- Kept double-pane traversal and the analytic water-through-glass model; no recursive full water shader was introduced.
- Corrected the known source-test scanner distinction: `skip_glass` is penultimate in `march_chunk`, while `skip_water` is last in `trace_world`.

**What did NOT complete**
- `cargo test --release --test glass` could not run: **cargo is not installed in this workspace**. See `validation/native-glass-results.txt`.
- A new GPU sparse-tree fixture failed its first transport expectation. It observed `(primary_t=3.5, transmitted_material=0, transmitted_t=512, water_leg_hit=0)` where a water hit was expected.
- One untested lead: the fixture uses an exactly axis-aligned ray with zero Y/Z components; inspect the existing DDA reciprocal/step behavior before assuming the new glass branch is at fault. No traversal change was made on that hypothesis.
- I did **not** establish whether that failure comes from fixture construction or production traversal. I stopped rather than claiming a glass pass.
- `validation/check_glass_transport.py` and its failing log are retained as debugging inputs.
- **Glass is therefore changed but unconfirmed, not declared fixed.** The later far-target and full reflection assertions did not complete in that failed run.

## HK01 — Housekeeping and invariants

**Done**
- Deleted the entire stale `## EMERGENCY QUEST -- do this one batch, then delete this section` block, through the next top-level section.
- Did not implement subgroup/wave material serialization or bindless indexing changes.
- Did not replace attribute lookup, Dolonius leaf prefixes, `mask_below`, geometry layout or the visibility key.
- `WaterCtx` is an internal WGSL helper struct; extending it does not widen the uploaded `GpuFrame`.
- Added this log, updated handoff pointers, and regenerated one cumulative patch.
- Touched `src`/`tests` Rust/WGSL files and `Cargo.toml` as requested. A Git patch does not preserve mtimes, so repeat the touch command after applying it locally if your build environment needs it.

**Testing state**
- Native compilation, full native tests, register statistics and target-GPU timing were not completed.
- Some old multi-ray/buffer-based test contracts were updated without rerunning them after your stop request. Older Python harnesses can still assume the previous architecture; they are not current acceptance evidence.
- Earlier successful surface-repair logs are historical and are not proof for this overhaul.
- Final packaging is mechanical file/diff work, not a test pass.

## Suggested next work on your machine

1. Build the delivered tree; resolve any Rust/wgpu API or layout issues first.
2. Run the glass suite and investigate the retained sparse-tree fixture failure before claiming the optical bug fixed.
3. Compare hard and reconstructed soft shadows at an identical moving/stationary camera. Use the new **shadow ray + filter** HUD times, not just FPS or opaque resolve time.
4. Check glass over water, glass reflected in water, two panes, back-side viewing and distant geometry beyond 192 blocks.
5. Film the same shoreline/aerial ocean with Water Look on/off and motion stopped/running. Tune variance filtering from that evidence.
6. Only then investigate shared-memory filtering or other measured shadow optimizations. Do not add the rejected wave scalarization or attribute hashing.

## Delivery / application

- Updated tree: `voxelcraft-menu/`.
- Patch: `voxelcraft-menu.patch`, cumulative against the newer original ledger-102a export.
- **Do not apply this cumulative patch on top of an earlier menu/surface-repair patch.** Use the updated tree, or apply to the matching original baseline.
- No build, benchmark or passing-glass claim is implied by delivery.

## CL01 — Final closeout (no execution)

- Added `HANDOFF.md` with per-batch touched areas, implementation methods, expected results and incomplete work.
- Added `validation/README.md` distinguishing current diagnostic inputs, the failed glass fixture and historical evidence.
- Removed `rust-env.sh`, the sandbox-only bootstrap that installed a toolchain under `/tmp`; the owner uses their own toolchain. Kept the original project export utility.
- Retained Rust test sources, optional diagnostic scripts and relevant failure logs. No further feature changes.
- Refreshed source timestamps and mechanically regenerated the cumulative patch and complete source ZIP. Temporary baseline copies were deleted after packaging.
- **No build, test, benchmark, diagnostic or validation run was performed during closeout.** Expected outcomes are not claimed as observed results.
