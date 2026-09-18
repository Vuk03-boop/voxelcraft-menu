# The menu and preferences

The menu state, layout and navigation shared by hit testing, keyboard
navigation and drawing live in `src/menu.rs`; the persisted windowed
preferences live in `src/settings.rs`. Draft edits never mutate applied
preferences until Apply commits.

> This file absorbs `MENU-NOTES.md` from the original handoff, folded in the
> 2026-09-18 documentation pass; the handoff's other companion notes
> (BATCH-LOG, EXPERIMENTS, SURFACE-REPAIRS) no longer exist in this tree.

## Screens and pages

`Screen`: Title, Playing, Pause, Settings, Loading, ConfirmQuit. `Page`
(settings): Display, Graphics, Appearance, Experimental, Effects. `Setting`:
the nineteen adjustable values in the table below. `Action`: what an item
does — navigation, `Adjust(Setting, i32)`, `Apply`, `Cancel`, `Defaults`.

- **Title:** Play, Settings, Quit. Full renderer creation and streaming
  updates are deferred until Play; the title still needs a supported GPU for
  presentation.
- **Pause:** Resume, Settings, Quit. Escape pauses; Escape from the pause page
  resumes. Focus loss or minimization also pauses.
- **Settings:** draft values are separate from applied preferences. Apply
  saves; Cancel / Back discards unapplied edits. Restore Defaults edits the
  draft, not the applied state.
- **Render lab / Effects lab:** seven off-by-default shader switches, editable
  **only on the title before Play**. During a session they are locked;
  ordinary settings remain available, and Restore Defaults during a session
  retains the locked shader choices.
- **Quit:** attempts the configured world-journal save first. A missing
  destination or a save failure presents Back and an explicit quit without
  saving. Preferences and world journals are separate files.

Mouse targets and drawing use the same layout: an `Item` is a rect plus a
label plus an action, and hit testing is its `contains` check. Arrows/Tab
navigate, Enter/Space activate, Left/Right adjust, Escape goes back. The menu
consumes input before player movement, mouse look, hotbar or block-edit
handling, and held input is cleared at pause/focus transitions and on resume.

## Controls

| Page | Controls |
|---|---|
| Display | Render scale 25–100%, FOV 30–120°, VSync, TAA |
| Graphics | Sun shadows, AO, water reflections, water transparency/refraction |
| Appearance | Original/ACES tone curve, Natural/Warm/Cinema grade, grade strength, fog density, ambient light |
| Render lab | Compact shade_hit schedule, separate glass pass, dedicated shadow pass, soft shadows |
| Effects lab | Existing glass reflection, water look, wind sway |

**13 ordinary settings and 7 title-only experiments.** Scale steps by 5
percentage points; FOV by 5°; grade strength by 10%; fog by 0.0001; ambient by
0.01. Fog is bounded to 0–0.01 and ambient to 0–0.5. A valid arbitrary
startup scale such as 0.8 is retained until adjusted.

The experimental switches are **not fixes** for their known rendering
defects; soft shadows are costly; none of the performance experiments has a
measured target-GPU improvement. The lab pages state these caveats.
Shadow-pass mode also isolates glass and water internally, without changing
the separate-glass checkbox; resolve timing includes the complete pass
family.

No world browser, save-path editor, fullscreen/display-mode controls or
automatic world-recipe changes were added. The remaining knobs stay on the
CLI (`--help` is the complete list), including the latest lighting/water
repair defaults and their `--legacy-*` / `--repaired-*` arms.

## Application and pause behavior

`Preferences` owns the menu-applied state; `Config` is the derived
renderer-facing snapshot. F4/F7 use the same validation/application path;
hotkey changes are session-only until Apply and save is chosen.

- Scale rebuilds screen-sized resources and invalidates history.
- VSync reconfigures the surface with automatic present-mode fallback.
- Tone/grade/strength update the presentation uniform without discarding HDR
  history.
- FOV, lighting, fog and TAA invalidate history without reallocating it.
- Experimental changes after a session starts are rejected before config,
  resources or the preference file are changed; on the title they update the
  startup configuration, and Play compiles the selected variants.

Paused frames re-present the last HDR image plus UI; after a resize,
UI-only presentation avoids reading uninitialized history. Gameplay, sun time
and the animation/weather clock stop, and the frame-time baseline resets on
resume. Running chunk worker jobs may finish, but the menu neither integrates
streaming updates nor advances simulation. Redraws run near 30 Hz and stop
while unfocused or minimized.

## Persistence

Windowed startup layers **defaults → saved preferences → explicit CLI
options**; explicit values win even when equal to defaults. Headless modes do
not load personal preferences (`validation/check_menu.py` asserts the
isolation). Invalid final window settings fall back together to safe menu
defaults with a warning; world recipes are not reset.

Location precedence:

1. `VOXELCRAFT_SETTINGS` environment override.
2. `%APPDATA%/voxelcraft/settings.conf` on Windows.
3. `$XDG_CONFIG_HOME/voxelcraft/settings.conf`.
4. `$HOME/.config/voxelcraft/settings.conf`.
5. Local `.voxelcraft-settings.conf` fallback.

Version-3 `key=value` files reject unknown/duplicate keys, unsupported
versions, invalid ranges and non-finite values; reads are bounded to 64 KiB.
Versions 1 and 2 remain readable: version 1 loads with experiments off, and
version 2 preserves its experiments while the two architecture options stay
off. `isolate_glass` and `shadow_pass` require version 3. A malformed file is
reported and not overwritten automatically; an explicit Apply and save can
replace it.

Saving writes and synchronizes a sibling temporary file, then renames it.
Errors leave preferences applied for the session and print a warning with
full details on stderr. This is not power-loss-proof: the parent directory is
not fsynced, and concurrent processes are last-successful-writer. World
journal writes remain separate and are not made atomic by this.
`--no-exit-save` concerns the final snapshot only.

```bash
cargo run --release -- --edits my-world.journal   # a persistent world
```

## Verification

The menu, experiment and architecture tests live in `tests/settings_menu.rs`
and `tests/experiments.rs`. The source-level guards live in
`validation/check_menu.py`: a tree-sitter parse of every `.rs` file, the
deferred-renderer / pause-before-simulation / frozen-clock / unified
scale-TAA / headless-isolation invariants, and the claim that every
preference CLI alias is a real parser arm. Windowed input behavior and
target-GPU performance are covered by neither and still need a human at the
machine.
