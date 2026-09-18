# AGENTS.md — behavioral contract for working in this repo

This file is the canonical rule set for any agent (human or AI) working on
voxelcraft. `CLAUDE.md` is a symlink to this file; edit this one. Other
documentation lives under `docs/` and is linked from here — do not inline it.

**Keeping this file lean is a rule, not a preference.** Budget: 200 lines,
hard ceiling 300. Apply the "test for every line": if the agent would not make
a mistake without this line, delete the line. Link out instead of copying;
prune stale lines regularly ("catastrophic remembering" is the failure mode).

## Project

voxelcraft is an experimental voxel engine in Rust (wgpu 30, winit 0.30,
WGSL shaders). One binary (`voxelcraft`) runs the windowed game and all
headless harness modes. Interiors are a sparse voxel tree with LOD streaming,
flood-fill sky/RGB block light, procedural worldgen, and a ray-marched render
with soft shadows, water and atmosphere.

## Essential commands

| Task | Command |
| --- | --- |
| Run the game (windowed) | `cargo run --release` |
| Full flag reference | `cargo run --release -- --help` (authoritative; do not duplicate it in docs) |
| Test suite | `cargo test` |
| One suite | `cargo test --test glass` (any file in `tests/`) |
| Criterion bench | `cargo bench --bench terrain` |
| Frame bench | `cargo run --release -- --bench-frames N` |
| Headless capture | `cargo run --release -- --screenshot out.png` |
| Compare captures | `cargo run --release -- --diff A.png B.png --crop X0,Y0,X1,Y1 --grid 4,4` |
| Validation checks | `python3 validation/check_<area>.py` (9 scripts; see `docs/testing.md`) |

## Architecture map (details: `docs/architecture.md`)

`src/main.rs` entry + mode dispatch -> `app.rs` (window/event loop) or
`headless.rs` (offscreen). `config.rs` parses every flag into `Config`.
`worldgen.rs` + `biome.rs` build height/cave/biome fields; `voxel/` owns the
tree (`tree.rs`, `chunk.rs`, `world.rs`, `geometry.rs`, `attributes.rs`);
`stream.rs`/`lod.rs` stream and subdivide it. `scene.rs` assembles a frame;
`render/` marches it (`render/mod.rs` + `render/shaders/*.wgsl`, plus `blit`,
`shadow`, `timer`, `ui`, `pools`, `font`). `light.rs` is the 3-channel flood
fill. `block.rs` defines blocks + albedo. `player.rs`/`math.rs` physics;
`menu.rs`/`settings.rs`/`idle.rs` front-end; `journal.rs` edit persistence;
`harness/` bench/validate tooling; `probe.rs` the light-probe experiment.

## Hard rules

1. **Bit-exact controls stay bit-exact.** Flag pairs documented as "reverts
   batch N" or "the pre-N presentation bit for bit" (e.g. `--grade none`,
   `--no-light-rgb`, `--no-water`) must produce identical output to their
   reference. If you cannot keep the identity, do not change the code path.
2. **Rust/WGSL constants move together.** Examples: `BRICK_WORDS = 32` and
   the 2-byte light cell in `common.wgsl` (`light_at`, `(idx & 1) * 16`
   half-word select); `SURFACE_EPS` is defined exactly once, in
   `common.wgsl`. Rename a shared constant and you must grep the tests first
   — many pin it as source text.
3. **`--water-mottle` is fixed at 0.0.** The flag parses, prints the reason
   to stderr, and ignores the requested value (WA01). Do not plumb the value
   through; see `docs/adr/0002-water-variance-filtering.md`.
4. **Tests are headless and pure.** No GPU device, no window in `tests/` —
   they open temp files and parse source/shader text only.
5. **Sea level has one home.** `worldgen::SEA_LEVEL = 128`. Anything tied to
   the waterline (fog height, beaches, reeds, tree/grass cut-offs) derives
   from it; do not introduce a second constant.
6. **Window/surface lifetime.** The window handle must outlive the surface
   built from it (`app.rs`); keep the drop order.
7. **Cite paths and symbols, never line numbers, in docs and comments.** Line
   numbers go stale silently.

## Documentation discipline

- This file is a pointer. Detail belongs in `docs/` (index: `docs/README.md`).
- A behavior change updates the doc it contradicts **in the same commit**.
  If doc and code disagree, the code is right — fix the doc immediately.
- Any significant architectural decision gets an ADR (`docs/adr/`,
  template in `docs/adr/README.md`). ADRs are append-only; supersede, never
  rewrite history.
- When the agent fumbles (wrong assumption, wasted loop, confusing code),
  append the lesson to `docs/lessons.md` — that log is how the workflow
  improves.

## Daily pitfalls (full log: `docs/lessons.md`)

- Mixed line endings on purpose; `.gitattributes` pins `* -text`. Check bytes
  before anchoring an edit on a newline.
- Shader-source pins in `tests/` split lines on `//`; a stray comment marker
  inside a string literal breaks them.
