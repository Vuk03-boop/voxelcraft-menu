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

## Protocol

1. Red-green-refactor: pin the expectation (failing test or captured
   reference) *before* the behavior change; keep the pin while refactoring.
2. `cargo test` must pass before and after every change; the relevant
   `validation/check_*.py` should agree.
3. Changing a shader constant? Grep `tests/` for its name first.
4. A new control that replaces an old behavior ships as a `--no-*`/arm pair
   with its bit-exactness story spelled out in the flag's `--help` line.
