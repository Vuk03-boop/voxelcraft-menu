> **Latest handoff:** Read [BATCH-LOG.md](BATCH-LOG.md) first. It records the decoupled-shadow overhaul, changed controls, superseded behavior, and unresolved work. Earlier validation/performance notes below describe their original batches, not acceptance of the latest code.

# Menu and preferences — current usage

This source tree includes the menu implementation plus all three opt-in rendering experiments. See [EXPERIMENTS.md](EXPERIMENTS.md) for the code review, exact scope, test results and the implemented glass/shadow pass architecture.

The latest default lighting/water repairs and CLI-only softness, quality and diagnostic controls are described in [SURFACE-REPAIRS.md](SURFACE-REPAIRS.md). They do not add persisted menu fields or change the seven experimental toggles.

## Menus

- **Title:** Play, Settings, Quit. Full voxel Renderer creation and streaming updates are deferred until Play; the title itself still needs a supported GPU for presentation.
- **Pause:** Resume, Settings, Quit. Escape pauses; Escape from the pause page resumes. Focus loss or minimization also pauses.
- **Settings:** draft values are separate from applied preferences. Apply and save commits; Cancel / Back discards unapplied edits. Restore Defaults edits the draft, not the applied state.
- **Render lab / Effects lab:** five off-by-default shader switches (two in Render lab, three in Effects lab), editable **only on the title before Play**. The dedicated primary-shadow pass is not among them: it is mandatory and always on. During a session the experiments are locked; ordinary settings remain available. Restore Defaults during a session retains locked shader choices. Restart the application to change them on its title screen.
- **Quit:** attempts the configured world-journal save first. Missing destination or a save failure presents Back and explicit Quit without saving. Preferences and world journals are separate files.

Mouse targets and drawing use the same layout. Arrows/Tab navigate; Enter/Space activate; Left/Right adjust. Escape goes back. The menu consumes input before player movement, mouse look, hotbar or block-edit handling. Held input is cleared at pause/focus transitions and on resume.

## Controls

| Page | Controls |
|---|---|
| Display | Render scale 25–100%, FOV 30–120°, VSync, TAA |
| Graphics | Sun shadows, AO, water reflections, water transparency/refraction |
| Appearance | Original/ACES tone curve, Natural/Warm/Cinema grade, grade strength, fog density, ambient light |
| Render lab | Compact shade_hit schedule, soft shadows |
| Effects lab | Existing glass reflection, water look, wind sway |

There are **13 ordinary settings and five title-only experiments**. Scale steps by 5 percentage points; FOV by 5°; grade strength by 10%; fog by 0.0001; ambient by 0.01. Fog is bounded to 0–0.01 and ambient to 0–0.5. Valid arbitrary startup scales such as 0.8 are retained until adjusted.

The experimental glass/water/wind switches are **not fixes** for their known rendering defects. Existing soft shadows are costly. None of the three performance experiments has a measured target-GPU register/FPS improvement. The lab pages state these caveats; see the experiment notes before enabling them. Shadow-pass mode also isolates glass and water internally, without changing the separate-glass checkbox. Resolve timing includes the complete new pass family.

No world browser, save-path editor, fullscreen/display-mode controls or automatic world-recipe changes were added. Other advanced controls remain on the CLI.

## Application and pause behavior

`Preferences` owns the menu-applied state; `Config` is the derived renderer-facing snapshot. F4/F7 use the same validation/application path. Hotkey changes are session-only until Apply and save is chosen.

- Scale rebuilds screen-sized resources and invalidates history.
- VSync reconfigures the surface with automatic supported present-mode fallback.
- Tone/grade/strength update the 16-byte presentation uniform without discarding HDR history.
- FOV, lighting, fog and TAA changes invalidate history without reallocating it.
- Experimental changes are rejected after a session starts, before config, resources or the preference file are changed. On the title they update startup configuration; Play compiles the selected variants.

Paused frames re-present the last HDR image plus UI. After a resize, UI-only presentation avoids reading uninitialized history. Gameplay, sun time and the animation/weather clock stop; the frame-time baseline is reset on resume. Already-running chunk worker jobs can finish, but menus neither integrate streaming updates nor advance simulation. Menu redraws normally run near 30 Hz and stop while unfocused/minimized.

## Persistence

Windowed startup layers **defaults → saved preferences → explicit CLI options**. Explicit values win even when equal to defaults. Headless modes do not load personal preferences. Invalid final window settings fall back together to safe menu defaults with a warning; world recipes are not reset.

Location precedence:

1. `VOXELCRAFT_SETTINGS` environment override.
2. Windows `%APPDATA%/voxelcraft/settings.conf`.
3. `$XDG_CONFIG_HOME/voxelcraft/settings.conf`.
4. `$HOME/.config/voxelcraft/settings.conf`.
5. Local `.voxelcraft-settings.conf` fallback.

Version-3 `key=value` files reject unknown/duplicate keys, unsupported versions, invalid ranges and non-finite values. Reads are bounded to 64 KiB. Versions 1 and 2 remain readable. Version 1 loads with experiments off; version 2 preserves its previous experiments, with the glass split off and the (now mandatory) shadow pass forced on. The new `isolate_glass` and `shadow_pass` keys require version 3 — and a version-3 file that says `shadow_pass=false` is still overridden to on at load, because the dedicated pass is mandatory (`Preferences::decode` forces it; see BATCH-LOG.md DS01). A malformed file is reported and is not overwritten automatically; an explicit Apply and save can replace it.

Saving writes and synchronizes a sibling temporary file, then renames it. Errors leave preferences applied only for the session and display a warning with full details on stderr. This is not power-loss-proof: the parent directory is not fsynced, and concurrent processes use last-successful-writer behavior. Existing world-journal writes remain separate and are not made atomic by this change. `--no-exit-save` concerns the final snapshot; existing append-journal behavior remains unchanged.

Example persistent world:

```bash
cargo run --release -- --edits my-world.journal
```

## Verification limits and native acceptance

Syntax parsing and Python-wgpu checks passed; **Rust compilation, Rust tests, windowed input behavior and target-GPU performance have not been verified here**. Thirty-three menu/experiment/architecture Rust tests are included. The text export contains no Cargo.lock or recoverable original image/PDF assets. Use the full repository's originals where available.

Before shipping, build and run the tests listed in [EXPERIMENTS.md](EXPERIMENTS.md), then check:

1. Title settings before Play; draft Cancel; defaults; Apply; restart persistence; explicit CLI overrides, including negative experimental flags.
2. Holding keys/buttons while pausing and resuming; no stuck input or stray block edit. Focus loss, minimization and DPI changes.
3. No gameplay/weather catch-up after waiting in a menu.
4. Paused resize, scale/TAA/VSync changes and no uninitialized background.
5. Locked experimental adjustments and defaults after Play; title-only variant changes after a restart.
6. Corrupt/oversized preferences, unwritable preference paths, absent world-save destinations and world-save failures; both Back and explicit discard must work.
7. Ordinary headless captures/benchmarks, then controlled A/B captures and target-GPU profiling of compact shading and all four glass/shadow architecture combinations. Check glass, water, foliage, odd-size resize and final recovery visibility, including shadows disabled/night/heatmap.

GPU initialization and compilation remain synchronous. A loading frame is submitted, but there is no asynchronous progress/cancellation system. Basic settings-driven size limits are checked; allocation/device-loss handling still follows the existing renderer's fatal-error policy. Transactional GPU-resource rollback is not implemented.
