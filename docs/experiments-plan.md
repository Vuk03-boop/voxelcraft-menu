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
| 5 | `--exp-glass-ssr` | Screen-space marching for glass legs, sky fallback (shipped) | vis buffer is always-on at resolve; 24-step jittered march, 1.12 geometric growth, DDA kept as fallback branch | CHANGE + perf arm |
| 6 | *octree-level skip* | Subtree-level run skipping in the DDA | **deferred with reasons (see below)** | EXACT |

## Exp 6 deferred: why the marcher did not get touched blind

The plan's honest read of `trace_world` + `march_chunk`: the DDA ALREADY
implements most of what the roadmap asked for -- absent L1 lines skip at 16
blocks, their pair-runs at 32 (`rlo & 0x00330033u` position-rotation test),
absent 4³ leaf quadrants at 4, and voxel spans at 1-2. The residual win
(runs of 4+ empty L1 nodes, "hierarchical cone skip to next set bit")
requires extending the rotated-mask encoding whose bit-pair selection
(`sh = l1b & 42u`) is position-dependent magic that the current pins do not
cover: dda.rs freezes reciprocal guards, pos_dir provenance, and raw-ray
position/normal math -- precisely the invariants that stay true for a WRONG
run-skip. The EXACT suite would catch visual drift, but only after the user
got a build back in exchange for the most delicate function in the renderer.
The lane's discipline (pin before promise) says no.

The day this goes in, the honest path is: (1) write the run-skip as a Rust
emulation in a dda.rs test against the existing synthetic chunk fixtures,
asserting bit-identical HITS on the corpus, not just inspectable strings;
(2) only then the wgsl mirror; (3) MAX_ITERS headroom monitored via the
existing iteration heatmap (dbg) before/after. Instrumentation for its value
already exists: the per-pixel iteration heatmap and the no-full-march perf
arm tell you exactly where the 15->8 step theorem would pay.


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

## Validation round 2 — what the first user-side sweep caught

The first full `upload_report.py` run after exp 1/4/3 landed found four
issues; all are fixed in this tree:

- **exp-shadow-repro / exp-temporal-hiz (FAIL)**: the steady-state signature
  lacked the TAA jitter and `time`, so a screenshot re-tracing every frame
  (its own halton-jittered, time-advancing rays) was replayed from the
  previous frame's targets. Time and both jitter components now hash into
  the signature; any animated parameter invalidates reuse and the arms are
  EXACT again (the bench loop keeps its steady skip because time is frozen
  there).
- **exp-biome-bake (FAIL)**: the bake table is plain storage; a triple being
  written by one invocation can tear while another reads it, and a torn mix
  can clear the "non-zero means ready" sentinel. The cache now returns a
  value only after two consecutive identical non-zero reads, else it computes
  the tint on the spot (same value, no tearing on the critical path).
- **exp-halfres-glass / exp-glass-ssr (SUSPECT, zero pixels moved)**: the
  CHANGE-class evidence was measured at the default vantage, where every
  glass pane faces parked sky and the changes honestly move nothing below
  the differ threshold. Both rows now compare inside the glass-house context
  (`--demo-glass` vantage, shared with the no-glass row) where the panes sit
  in front of terrain.
- **tests/cutout.rs (FAIL)**: the SSR step legitimately decodes the
  visibility payload for shading — a fourth `(low >> 16u) & VOXEL_MASK` site.
  The pinned count rose to 4 with the new site named in the message; the
  audit (every site must mask with VOXEL_MASK, never 0xFF) still passes.

Also fixed along the way: `upload_report.py` mixed a `Path` with a string on
the default log path (`TypeError` when no `--out` was given), and exp 3's
plumbing had bind-group call sites missing the bake buffer.
