# AGENTS.md

The canonical agent briefing for this repository. `CLAUDE.md` is a symlink to
this file — edit this one, never a second copy. It is a pointer: the detail
lives in the files it names, and a line the agent could not get wrong without
gets cut.

## What this is

A Minecraft-style voxel engine in Rust and WGSL (wgpu 30): ray tracing sparse
trees directly on the GPU — 64³ chunks in three-level 4³ trees, with geometry
(64-bit occupancy masks) strictly decoupled from attributes (palette-compressed
IDs, light bricks). Frame order: light envelope → tile select → ray march
(64-bit visibility) → Hi-Z → recovery → resolve → temporal resolve. This
branch adds the settings **menu** on top of it —
[docs/menu.md](docs/menu.md). Headless modes are a first-class measurement
interface, not a convenience.

## Commands

```bash
cargo run --release                # windowed game (default-run = voxelcraft)
cargo test --release               # the invariant suite; needs no GPU
cargo bench --bench terrain        # worldgen timing
cargo run --release -- --screenshot out.png        # one frame, time pinned to 0
cargo run --release -- --diff a.png b.png --grid 8,8
python3 validation/check_menu.py   # menu source guards (tree-sitter + python wgpu)
python3 run_audit_suite.py         # flag-by-flag regression & diff audit -> uploadme.txt
```

- `--help` is the complete flag list.
- The measurement fixture's subcommands (list, capture, validate, implies,
  bitexact, bench, march, reference, compare) are documented in
  [docs/harness.md](docs/harness.md); this tree does not ship the `src/bin`
  drivers, so run that vocabulary from a checkout that has them.
- A measurement without the vantage that produced it is a lie: every number
  carries the capture recipe — [PERF.md](PERF.md),
  [docs/harness.md](docs/harness.md).

## Architecture map

- `src/lib.rs` — the engine crate. `src/main.rs` dispatches six modes (run /
  screenshot / lookbook / bench-terrain / bench-frames / diff) and splits on
  *who calls what*: `scene` (shared), `headless` (measurement), `app` (the
  window loop).
- `src/voxel/` — world, chunks, geometry, the sparse tree; `lod.rs` and
  `stream.rs` stream it.
- `src/render/` — the frame: march → resolve, the decoupled shadow pass
  (`shadow.rs`), shafts, TAA, UI, and the `.wgsl` shaders.
- `src/harness/` — vantages, crops, metrics, recipes. It drives the
  *executable*, never the library.
- `src/menu.rs` + `src/settings.rs` — the menu state machine and the persisted
  `Preferences` (windowed only).
- `src/journal.rs` — the world edit journal, a separate file from
  preferences.

Details: [docs/reference.md](docs/reference.md).

## Hard rules

1. **Bit-exact is the house standard.** A capture is either bit-identical to
   the control or it moved; "looks the same" is not a result. New parameters
   default to their zero control, and zero must early-return to the pre-batch
   frame by construction — guarded in the shader, not in Rust.
2. **Host-device constant sync.** Shader constants in
   `src/render/shaders/*.wgsl` must exactly mirror their Rust equivalents in
   `src/render/mod.rs` and `src/config.rs`; the suite asserts this.
3. **Headless is isolated from personal preferences.** `headless.rs`,
   `config.rs` and `scene.rs` must not contain `settings::load`;
   `validation/check_menu.py` asserts it.
4. `--water-mottle` is accepted and **ignored on purpose** (WA01 fixes it at
   0.0 to keep the atlas layer flat). The warning line is the documentation;
   do not "fix" it.
5. **DS01**: primary shadows are one geometric ray + screen-space
   reconstruction through the three-stage decoupled pass (R16F blocker
   distance → RG16F occlusion → R8 visibility; 7 bytes per pixel). Keep the
   blocker map `R16Float`; `--shadow-pass` is the default and
   `--no-shadow-pass` is retired.
6. **The `ALBEDO` table stays linear in Rec.709 luminance** for all 26
   blocks; never sRGB-encode it.
7. **Atmospheric coupling**: fog `height` is
   `crate::worldgen::SEA_LEVEL as f32`, not an authored constant.
8. **Line endings are part of the bytes.** This tree is deliberately mixed —
   `Cargo.toml`, `PERF.md` and `README.md` are CRLF, most of the rest is LF,
   with three named CRLF exceptions. Check the file before anchoring an edit
   on a newline rather than trusting a rule; `.gitattributes` (`* -text`) is
   why a Windows checkout must not "help".
9. **WGSL uniform buffers are 16-byte aligned.** Do not reorder frame
   uniform fields without re-checking the padding; the suite asserts
   offsets.
10. **The 64-bit visibility key is densely packed.** Do not insert a bit
    without updating the unpacking in `common.wgsl`.
11. The settings file is **versioned, strict key=value** (version 3). A new
    key requires a version bump and a parse test; a malformed file is reported
    and never overwritten.
12. **Menu experiments are title-only.** The seven experimental switches are
    editable before Play and locked during a session.
13. No new crate dependency without the reason stated in the commit.
14. Where you fumble, one bullet in [docs/lessons.md](docs/lessons.md): the
    trap in bold, what it cost.
15. Before deleting a document, grep the tree for what references it by name;
    before pushing, fetch the actual remote ref — the origin refspec here
    tracks `main` only.

## The control table

The `--no-*` arms are the ledger's controls; each one is the pre-batch world,
and each row's reasoning lives in [docs/ledger.md](docs/ledger.md), not here.

| Control | Arm |
|---|---|
| water | `--no-water` (pre-8) |
| biomes | `--no-biomes` (pre-9) |
| foliage tufts | `--no-foliage` (pre-14) |
| meadow | `--no-meadow` (pre-18) |
| water mottle | fixed 0.0; `--water-mottle 0.2` = pre-26 |
| demo structures | `--demo-edits`, `--demo-glass` (38b), `--demo-lamps` (59) |
| probe bounce/sun | `--no-probe-bounce`, `--no-probe-shadow`, `--no-probe-sun`, `--probe-sun-high` |
| cloud banks/relief | `--cloud-patch F`, `--cloud-relief F` (pre-95 at 0) |
| the seven experiments | `--no-compact-shade-hit`, `--no-glass-reflect`, `--no-water-look`, `--no-wind-sway`, `--no-soft-shadows` |
| surface repairs | `--legacy-lighting` / `--repaired-lighting`, `--legacy-water` / `--repaired-water` |
| LOD fade (D2b) | `--no-fade`, `--fade-schedule`, `--fade-band F` |

## Where things live

| Topic | File |
|---|---|
| vantage, crop, metric, recipe | [docs/harness.md](docs/harness.md) |
| every measured number, with its vantage | [PERF.md](PERF.md) |
| lighting, shadows, probes, albedo | [docs/shading.md](docs/shading.md) |
| water: absorption, mottle, variance | [docs/water.md](docs/water.md) |
| fog + cloud deck | [docs/sky.md](docs/sky.md) |
| blocks, `BlockDef`, the 26-row albedo | [docs/reference.md](docs/reference.md) |
| the edit journal, worlds on disk | [docs/persistence.md](docs/persistence.md) |
| the menu, `Preferences`, the settings file | [docs/menu.md](docs/menu.md) |
| batch rows, one per control | [docs/ledger.md](docs/ledger.md) |
| failure modes that look like success | [docs/errors.md](docs/errors.md) |
| traps, before you type a command | [docs/pitfalls.md](docs/pitfalls.md) |
| fumble log, before you plan a batch | [docs/lessons.md](docs/lessons.md) |
| the open work | [docs/roadmap.md](docs/roadmap.md) |
| test suite and invariants | [docs/validation-and-invariants.md](docs/validation-and-invariants.md) |
| decisions and why (append-only) | [docs/adr/](docs/adr/) |

## Out of scope

- `validation/history/` — archived results; read, never rewrite.
- `validation/check_split.py` and kin — marked *historical harness*; port
  before use.
- `target/` — build output; never ours.
- `benches/terrain.rs` — measures the hardware baseline; its output is a
  measurement and belongs in PERF.md with its vantage.
- The procedural noise cores in `src/biome.rs` and `src/worldgen.rs` — touch
  only with a proven math defect.
- `docs/adr/` — append-only; ADR numbers are never reused.
