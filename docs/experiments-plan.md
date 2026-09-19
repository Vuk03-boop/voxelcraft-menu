# Experiments Lane — roadmap ideas shipped as off-by-default flags

Every idea from the review lands as an `--exp-*` flag: **off by default**, so
`main` behavior (and every existing EXACT suite pin) is untouched, and each
landed experiment gets its own suite row. One experiment per sitting; after
the last one, a single full `upload_report.py` run compares everything and
produces the eyeball PNGs.

## Convention

- Flag name: `--exp-<idea>` → `Config.exp_<idea>: bool`, `Default` false.
- Suite row in `upload_report.py` CHANGE class until measured, plus a
  `perf` arm `<name>-b` (bench with the flag) so the end run prices it.
- Output-preserving experiments (octree skip, shadow reprojection) must
  land in the **EXACT** class: any pixel movement with the flag ON vs its
  own OFF baseline is a regression, not a surprise.
- Anything that adds a specialization override follows the spec_hi guard:
  `SPEC_EXP_*` override + `FLAG_HI_EXP_*` sibling, wired in `render/mod.rs`
  — the wiring pin (`tests/spec_hi.rs`) fails the moment one side exists
  without the other.

## Order (readiness x payoff)

| # | Flag | Idea | Prereqs | Class |
|---|------|------|---------|-------|
| 1 | `--exp-shadow-repro` | Temporal shadow reprojection (safe-static-first) | world epoch on GPU (landed) + signature-gated dispatch skip; moving-camera repro is the follow-up | EXACT |
| 2 | `--exp-halfres-glass` | Half-res glass legs, quadrant sharing (shipped) | halved-grid dispatch, shade_glass cached per 2x2 thread via private vars; per-pixel tail stays per-pixel | CHANGE + perf arm |
| 3 | `--exp-biome-bake` | Lazily-refined biome tint cache (shipped) | 128² vec3 storage cells (256-block period), refined in place with identical f32 math, spec-key zeroing | EXACT |
| 4 | `--exp-temporal-hiz` | min-blend last frame's Hi-Z before finalize | world epoch on GPU | EXACT ~ (defer ratio only) |

Execution order (readiness x payoff, re-checked as machinery appeared):
**shipped** = 0) world epoch on GPU `731cec6`, 1) `--exp-shadow-repro` `7cc068d`,
then 4) `--exp-temporal-hiz` was pulled ahead of 2/3 because it rides the SAME
signature machinery rendered-only (renamed `shadow_signature` ->
`steady_signature` to reflect the shared use). Both skips are
safe-static-first: exact by construction on steady frames, deliberately inert
while the camera or the world moves. Remaining: 2) half-res glass, 3) biome
bake, 5) glass SSR, 6) octree skip -- the plumbing-heavy four each get a
focused sitting.
| 5 | `--exp-glass-ssr` | Screen-space marching for glass legs, sky fallback | visibility always-on (today: isolate-lane only, mod.rs:1021) | CHANGE + perf arm (the +41% row) |
| 6 | `--exp-octree-skip` | Subtree-level skip in DDA | dda.rs pins exist | EXACT |

## The shared hook: world epoch on GPU

`World.version` already bumps on every mutation (`voxel/world.rs:152,
213, 526, 590`) and is already read by the app (`app.rs:749`). Experiments
1 and 4 only need it uploaded:

1. Append `world_epoch: u32` + pad to the **end** of `GpuFrame`
   (offset pins at `render/mod.rs:466` keep the existing layout honest;
   the new field extends, never shifts).
2. Fill it in app upload alongside `prev_view_proj` (:1640) and in the
   headless FrameParams literals (bins.rs pins the pattern).
3. Mirror the field at the end of the WGSL `Frame` struct so offsets
   line up.

Experiments that reuse history consult `world_epoch`: when it changes,
reprojection/blend is disabled for that frame (full re-trace). Static
camera + static world = the 90%-reuse case the roadmap promised.

## End-of-lane ritual (after experiment 6)

One full `python validation/upload_report.py` (no `--fast`) at the user's
single-slot cadence: all six `--exp-*` rows measured, all baseline EXACT
rows still 0 px, plus lookbook + ctx PNGs for the eyeball pass on any
CHANGE-class arm (glass especially).
