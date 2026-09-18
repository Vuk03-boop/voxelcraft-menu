# Testing

## Shape of the suite

Everything lives in `tests/` as integration suites, one file per domain
(`glass.rs`, `water.rs`, `snell.rs`, `light.rs`, `shadow_grid.rs`,
`edit_journal.rs`, `menu`-adjacent: `settings_menu.rs`, `emitter_gate.rs`,
…). `tests/support/mod.rs` is the shared helper. Suites are pure and
headless: no GPU device, no window — contract checks, source/shader-text
pins, image diffs via the headless modes, and temp-file round-trips only.

Run: `cargo test`, or one suite with `cargo test --test <name>`.
Benches: `cargo bench --bench terrain` (criterion, `harness = false`) and
`cargo run --release -- --bench-frames N`.

## Enforced invariants

| Invariant | Where pinned |
| --- | --- |
| Bit-exact controls: a "reverts batch N" flag pair must produce bit-identical output | `tests/light.rs` (`--no-light-rgb`), image-diff suites |
| Rust/WGSL constant agreement (e.g. `BRICK_WORDS = 32`, 2-byte light cell) | `tests/light.rs` (`light_cell_is_two_bytes_wide`) |
| `SURFACE_EPS` defined exactly once, in `common.wgsl` | `tests/batch91.rs` |
| Water invariants: mottle pinned 0, half-res secondary legs default | `tests/water.rs`, `tests/water_sec.rs` |
| Physics, refraction and wave math | `tests/physics.rs`, `tests/snell.rs`, `tests/waves.rs` |
| Journal round-trips and edit path | `tests/edit_journal.rs`, `tests/edit_regression.rs` |

**Source-text pins.** Many suites locate behavior by reading `.rs`/`.wgsl`
source and asserting it contains (or lacks) a literal — `tests/support/mod.rs`
strips `//` comments before pinning. Renaming a shared constant, moving a
definition to another file, or introducing a stray `//` inside a string
literal will fail these suites. Treat that as the intended tripwire:
grep the tests before you refactor names.

## Validation scripts (`validation/`)

Python checkers over the tree/git history, run individually:

| Script | Checks |
| --- | --- |
| `check_menu.py` | Menu flow/settings contract |
| `check_shaders.py` | Shader pipeline wiring and pinned constants |
| `check_surface.py` | Surface repair families |
| `check_split.py` | Pass-split (march/resolve) decoupling |
| `check_compact.py` | Compaction/ownership behavior |
| `check_decoupled.py` | The decoupled-restructure contract |
| `check_diagnostics.py` | Diagnostic capture paths |
| `check_family_timestamps.py` | Result-family timestamp ordering |
| `check_glass_transport.py` | Glass light-transport transport results |
| `upload_report.py` | Full baseline-vs-flag sweep: capture, diff, and bench every flag arm, verify restated defaults are bit-identical, exercise journals/lookbook/bench-terrain/`cargo test`, and write `uploadme.txt` for upload (`--fast`, `--skip-tests`, `--skip-lookbook`, `--skip-perf`, `--cases SUBSTR`) |

## The upload report (for remote diagnosis)

`python3 validation/upload_report.py` from the repo root builds the release
binary, then writes `uploadme.txt` (plus a full command trace in
`uploadme.txt.log` and PNGs in `uploadme_runs/`):

1. **Environment + build**: host, toolchain, git HEAD, GPU adapter name,
   `cargo build --release` result and warning count.
2. **Baseline**: a pinned capture (960x540, seed 1337, uniform TAA) plus a
   *determinism twin* — two identical captures must diff to zero.
3. **Flag sweep**: every CLI arm as a screenshot diffed against the baseline
   with the binary's own `--diff`. Arms that merely restate defaults must be
   bit-identical (EXACT — `compact-shade-hit` is one: it is an alternate
   build of the same math, so bit-exactness is its contract); feature arms
   must move pixels (CHANGE), and an arm that moves nothing is reported
   SUSPECT rather than silently green. Arms whose feature the baseline
   capture has nothing for are paired with a curated vantage from
   `src/harness/vantage.rs`'s catalogue (the demo structures and the
   submerged/periscope scenes exist for exactly this), and the few that
   still cannot trigger in the spawn scene carry the INERT expectation and
   report as expected-inert with the reason spelled out.
4. **Perf sweep**: `--bench-frames` under perf-relevant flags with
   wall/gpu-total deltas vs baseline, then one baseline re-run at the end so
   thermal/order drift (laptops heat up across the sweep) can be told apart
   from a real regression.
5. **Functional**: edit-journal save/replay bit-identity, `--bench-terrain`
   timings, `--lookbook`, and the full `cargo test --release` summary.

Verdicts land at the end of the file: `FAIL` needs a fix, `SUSPECT` needs a
look, `EXPECTED-INERT` is the scene saying "nothing to see" for a reason the
report names (and a note fires if such an arm starts moving pixels — the
trigger exists after all). The file is designed to be pasted back so someone
else can diagnose or optimize from it.

Scene-sensitivity notes learned the first time this ran: `--tint-strength`
clamps to 1.0 in the CPU→GPU frame upload, so values above 1 are
bit-identical by construction; `--probe-noise` only perturbs a forced
`--probe-fill` pre-fill; `--snell`/`--snell-bend` engage only under water;
the demo builders sit 14 blocks ahead of the camera, which the default
vantage leaves buried inside the front hill.

## Protocol

1. Red-green-refactor: pin the expectation (failing test or captured
   reference) *before* the behavior change; keep the pin while refactoring.
2. `cargo test` must pass before and after every change; the relevant
   `validation/check_*.py` should agree.
3. Changing a shader constant? Grep `tests/` for its name first.
4. A new control that replaces an old behavior ships as a `--no-*`/arm pair
   with its bit-exactness story spelled out in the flag's `--help` line.
