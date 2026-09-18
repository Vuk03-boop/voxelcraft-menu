# World

## Sea level

`worldgen.rs` owns `pub const SEA_LEVEL: i32 = 128`. It is the single source
for every waterline-adjacent decision: fog height (`rendering.md`), beach
band (`<= SEA_LEVEL + 2`), reed placement, tree and grass altitude cut-offs,
and the fog default. Never add a competing constant; derive.

## Generation

- Height and cave noise come from `fastnoise-lite` (`worldgen.rs`).
- Temperature/humidity biome fields come from the in-repo `biome::simplex2`;
  `tests/biome.rs` (`noise_matches_fastnoise_lite`) pins it against the
  reference crate, which stays a dev-dependency for exactly that cross-check.
- Biomes gate species by altitude (`tree_line`) and climate scales; columns
  near the waterline get shore treatment (wet band and foam, revertible via
  `--no-shore-wet` / `--no-shore-foam`).

## Blocks

`block.rs` defines `BLOCKS: [BlockDef; 26]` (air through reeds;
`REEDS == BLOCK_COUNT - 1` is a compile-time assert).

| `BlockDef` field | Meaning |
| --- | --- |
| `name` | Stable string id (used in menus, journals, tests). |
| `faces: [u32; 6]` | Atlas tile per face; helpers `all(t)` and `column(side, bottom, top)`. |
| `solid` | Collision / physics body. |
| `opaque` | Occludes light and view. Kept distinct from `solid` so glass is solid non-opaque. |
| `cutout` | Binary alpha-tested rendering (foliage-style), no blending. |
| `foliage` | Plant class: distinct lighting/culling treatment. |
| `light: [u8; 3]` | RGB emission into the light flood; `[0,0,0]` for non-emitters. |

## Albedo and bounce light

`block.rs` carries `ALBEDO: [[f32; 3]; 26]`, one row per block, **linear
Rec.709 reflectance — never gamma-encoded**. Lighting math assumes linear;
storing an sRGB value here silently corrupts bounce energy. Sample rows as
committed:

| Block (by index) | Albedo (R, G, B) |
| --- | --- |
| 0–1 (air, stone) | `[0.1738, 0.1738, 0.1738]` |
| 2 (dirt) | `[0.2066, 0.1016, 0.0470]` |
| 3 (grass top) | `[0.0920, 0.2947, 0.0407]` |
| 4 (sand) | `[0.6170, 0.5245, 0.2573]` |
| 10 (glass) | `[0.4397, 0.6282, 0.7365]` |
| 11 (amber lamp class) | `[0.8058, 0.6399, 0.2432]` |

The probe bounce (`probe.rs`) reads this table with a gather stride of 2 and
normalizes by `luma(ALBEDO[BOUNCE_REF])`. Adding a block means appending a
row **and** keeping it linear; see `docs/adr/0004-static-albedo-table.md`.

## Persistence: edit journals

World edits happen through the edit path only and are recorded as journals
(`journal.rs`). `--load-edits PATH` replays, `--save-edits PATH` writes at
mode end, `--edits PATH` loads-or-creates and appends, `--resume` restores
the player position from the journal, and `--no-exit-save` simulates a kill
(no exit save). `tests/edit_journal.rs` exercises this against temp files.
