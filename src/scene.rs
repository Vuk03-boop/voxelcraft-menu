//! The pieces both the window and the headless modes need: where the sun is, where the
//! camera starts, what medium the eye is in, and which render flags a config implies.
//!
//! Split out of `main.rs` by batch 22b. The line the split follows is **who calls it**, not
//! what it is about: everything here has at least one caller in [`crate::headless`] and one
//! in [`crate::app`], which is why it can live in neither. `flags_from` is the reason the
//! module exists -- three call sites build a `FrameParams` and a fourth would be the one
//! that forgot, and that argument stops holding the moment the three are in different files
//! with a copy each.
//!
//! Nothing here was renamed or rewritten in the move. `flags_from` is named in prose by
//! `config.rs`, `harness/mod.rs`, `render/mod.rs` and `common.wgsl`, and `tests/camera.rs`
//! transliterates `camera_basis` on purpose; a rename would have falsified all five with
//! nothing in the tree able to notice.

use glam::Vec3;
use voxelcraft::config::Config;
use voxelcraft::journal::EditJournal;
use voxelcraft::player::Player;
use voxelcraft::render;
use voxelcraft::voxel::{ChunkKey, World};
use voxelcraft::worldgen::WorldGen;
use voxelcraft::{block, math};

/// Sun direction and daylight factor for a fraction of the day.
pub(crate) fn sun(t: f32) -> (Vec3, f32) {
    let a = (t - 0.25) * std::f32::consts::TAU;
    let dir = Vec3::new(a.cos() * 0.55, a.sin(), 0.42).normalize();
    let daylight = ((dir.y + 0.12) / 0.32).clamp(0.0, 1.0);
    (dir, daylight)
}

pub(crate) fn spawn_position(gen: &WorldGen) -> Vec3 {
    // Above the water, not above the ground: the spawn column is as likely as any other to
    // sit in a basin, and `--cam-height` is measured from here.
    let h = gen.height(8, 8).max(gen.sea_level) as f32;
    Vec3::new(8.5, h + 2.0, 8.5)
}

/// Where a headless player starts: the vantage the camera flags describe, or -- under
/// `--resume` -- wherever the loaded journal says. Batch 32, and the bar in batch 34.
///
/// Both headless paths call it and a third would be the one that forgot, which is the
/// argument this module exists to make. **The boundary it draws is the mode's and not the
/// field's**: under `--resume` the journal supplies the position, the look angles and the
/// hotbar together, and without it none of them. Splitting it per field -- letting the file
/// pick the hotbar while the camera flags keep the camera -- reads perfectly reasonable and
/// would mean a `--hud` capture quietly changed the moment a journal was loaded.
///
/// **The bar is the one field outside that boundary, and it is outside because it is not a
/// camera.** `--resume` exists because the fourteen named vantages *are* the camera flags, so
/// a journal that moved one would turn every vantage into "wherever the file says". No
/// vantage names a bar, so there is nothing for a loaded bar to overrule. What makes that
/// safe rather than merely arguable is arithmetic: a journal written by a player who never
/// picked a block carries `block::HOTBAR`, which is what the bar would have been anyway, so
/// **every capture ever taken is bit-identical either way** -- and batch 34's `bitexact` is
/// the row that says so.
///
/// `journal::open` refuses a `--resume` with nothing to resume from, so the branch below
/// cannot silently do nothing.
pub(crate) fn start_player(cfg: &Config, journal: &EditJournal, player: &mut Player) {
    player.hotbar = cfg.hotbar;
    player.bar = voxelcraft::journal::bar_for(journal);
    if cfg.resume {
        if let Some(s) = journal.player() {
            player.restore(s);
        }
    }
}

/// Eye position `depth` blocks below the surface of the nearest water to `near`, if there
/// is any within a chunk or so. The only way to reach the underwater medium headlessly,
/// which is the only way to A/B it -- the same job `find_cave` does for `--cam-height -N`.
pub(crate) fn find_water(world: &World, near: Vec3, sea: i32, depth: f32) -> Option<Vec3> {
    let start = near.floor().as_ivec3();
    let below = (depth.ceil() as i32).max(1);
    for r in 0..64i32 {
        for dz in -r..=r {
            for dx in -r..=r {
                if dx.abs().max(dz.abs()) != r {
                    continue;
                }
                let p = glam::IVec3::new(start.x + dx, sea - 1, start.z + dz);
                if !world.is_loaded_at(p) {
                    continue;
                }
                // Water at the surface, far enough down to hold the camera, and open for a
                // few blocks around it -- otherwise the search settles on the first shore
                // tile it meets and the capture is a sand wall pressed against the lens.
                let eye = glam::IVec3::new(p.x, sea - below, p.z);
                let open = (-3..=3).all(|d| {
                    world.get_block(eye + glam::IVec3::new(d, 0, 0)) == block::WATER
                        && world.get_block(eye + glam::IVec3::new(0, 0, d)) == block::WATER
                });
                if world.get_block(p) == block::WATER && open {
                    return Some(Vec3::new(
                        p.x as f32 + 0.5,
                        sea as f32 - depth,
                        p.z as f32 + 0.5,
                    ));
                }
            }
        }
    }
    None
}


/// Search below and around a point for an open air pocket with headroom: a cave.
pub(crate) fn find_cave(world: &World, near: Vec3) -> Option<Vec3> {
    let start = near.floor().as_ivec3();
    let clear = |p: glam::IVec3| {
        // Needs standing room and a solid floor, so we are in a chamber not a crack.
        // Water is not solid, so a flooded basin would otherwise read as a fine cave.
        world.get_block(p) != block::WATER
            && !world.is_solid(p)
            && !world.is_solid(p + glam::IVec3::Y)
            && world.is_solid(p - glam::IVec3::Y)
            && (1..=3).any(|d| !world.is_solid(p + glam::IVec3::new(d, 0, 0)))
            && (1..=3).any(|d| !world.is_solid(p + glam::IVec3::new(0, 0, d)))
    };
    let mut best: Option<(i32, glam::IVec3)> = None;
    for dy in -60..=10i32 {
        for r in 0..14i32 {
            for dz in -r..=r {
                for dx in -r..=r {
                    if dx.abs().max(dz.abs()) != r {
                        continue;
                    }
                    let p = start + glam::IVec3::new(dx, dy, dz);
                    if p.y < 6 || !world.is_loaded_at(p) || !clear(p) {
                        continue;
                    }
                    // Prefer the roomiest pocket found.
                    let room = (1..=6)
                        .take_while(|&d| !world.is_solid(p + glam::IVec3::new(0, d, 0)))
                        .count() as i32;
                    if best.is_none_or(|(b, _)| room > b) {
                        best = Some((room, p));
                    }
                }
            }
        }
        if let Some((room, p)) = best {
            if room >= 3 {
                return Some(p.as_vec3() + Vec3::new(0.5, voxelcraft::player::EYE, 0.5));
            }
        }
    }
    best.map(|(_, p)| p.as_vec3() + Vec3::new(0.5, voxelcraft::player::EYE, 0.5))
}

/// Place a lit shelter in front of the camera through the normal edit path, so a capture
/// exercises set_block, the dirty-range upload and block lighting.
pub(crate) fn build_demo_structure(world: &mut World, gen: &WorldGen, cam: Vec3, dir: Vec3) {
    // Underground, a hut makes no sense: light the chamber instead.
    if world.is_solid(cam.floor().as_ivec3() + glam::IVec3::new(0, 3, 0)) {
        let mut keys = rustc_hash::FxHashSet::default();
        for (dx, dz) in [(2, 0), (-2, 0), (0, 2), (0, -2)] {
            let p = cam.floor().as_ivec3() + glam::IVec3::new(dx, -1, dz);
            if world.get_block(p) != block::AIR && world.set_block(p, block::GLOWSTONE) {
                keys.insert(ChunkKey::of_block(p));
            }
        }
        println!("  cave lamps: {} chunks edited", keys.len());
        for key in keys {
            voxelcraft::stream::relight(world, gen, key);
        }
        return;
    }
    let flat = Vec3::new(dir.x, 0.0, dir.z).normalize_or(Vec3::X);
    let centre = cam + flat * 14.0;
    let base = glam::IVec3::new(centre.x as i32, 0, centre.z as i32);
    // Drop to the surface under that point.
    let mut y = cam.y as i32;
    while y > 4 && !world.is_solid(glam::IVec3::new(base.x, y - 1, base.z)) {
        y -= 1;
    }
    let touched: Vec<_> = {
        let mut edits: Vec<(glam::IVec3, u16)> = Vec::new();
        for dz in -5..=5i32 {
            for dx in -5..=5i32 {
                // Floor of planks.
                edits.push((glam::IVec3::new(base.x + dx, y, base.z + dz), block::PLANKS));
                let edge = dx.abs() == 5 || dz.abs() == 5;
                if edge {
                    for h in 1..=4i32 {
                        let corner = dx.abs() == 5 && dz.abs() == 5;
                        let id = if corner && h == 4 {
                            block::GLOWSTONE
                        } else {
                            block::COBBLE
                        };
                        edits.push((glam::IVec3::new(base.x + dx, y + h, base.z + dz), id));
                    }
                }
            }
        }
        // A roof, leaving a hole so sky light still reaches the inside.
        for dz in -5..=5i32 {
            for dx in -5..=5i32 {
                if dx.abs() <= 1 && dz.abs() <= 1 {
                    continue;
                }
                edits.push((
                    glam::IVec3::new(base.x + dx, y + 5, base.z + dz),
                    block::PLANKS,
                ));
            }
        }
        let mut keys = rustc_hash::FxHashSet::default();
        for (p, id) in edits {
            if world.set_block(p, id) {
                keys.insert(ChunkKey::of_block(p));
            }
        }
        keys.into_iter().collect()
    };
    println!("  demo structure: {} chunks edited", touched.len());
    for key in touched {
        voxelcraft::stream::relight(world, gen, key);
    }
}

/// Place a chamber of coloured lamps under the camera through the normal edit path, so a
/// capture has an emitter in it that is not white. Batch 59.
///
/// **A third structure rather than lamps added to either of the other two**, for the reason
/// `build_glass_structure` gives -- `edits` and `glass` are references half a dozen batches are
/// measured against -- and for one that is sharper here and belongs to this batch alone:
/// **these blocks did not exist before batch 59**, so no earlier binary can be handed this
/// world at all. `--save-edits` out of this build and `--load-edits` into `voxelcraft-pre59.exe`
/// is the trick `docs/harness.md` records for a vantage whose flag the parent cannot parse, and
/// it does **not** work here: the journal carries block ids, `block::def` clamps an unknown id
/// to the last row of a shorter table, and the old binary would render a chamber of meadow.
/// So the `lamps` vantage is not a `bitexact` row against the parent and its positive claim
/// comes from `--no-light-rgb` inside this binary, which is what a control is for.
///
/// What it is shaped to exercise, in the order the batch reads them:
///
/// - **One lamp alone on a wall.** The ordinary case: a colour arriving on stone, which is the
///   whole of what a per-channel flood buys over a scalar one, and the only thing in the
///   fixture that can tell a tint applied at read time from light that is actually coloured.
/// - **Two lamps facing each other across the room.** The case that *requires* three floods:
///   the wall between amber and azure receives both, and the cell holding their overlap has to
///   remember which channel came from where. A scalar level with a per-block tint cannot, and
///   this is the pixel that says so.
/// - **A roof with no hole in it.** Sky light must not reach the room, or the block channels
///   are a correction to a term twenty times their size and nothing is legible. That is
///   `build_glass_structure`'s closed roof used for the opposite purpose.
/// - **Glowstone in the same room**, so the retired `BLOCK_TINT`'s own colour is in the frame
///   beside the three that replace it, and the 4-bit re-authoring of it can be judged against
///   neighbours rather than against memory.
///
/// **Underground and not a hut, unlike both of the others.** A lit room with a daylit exterior
/// spends most of its pixels on the exterior; the `cave` vantage's own argument is that the
/// most surface-dense frame is the one where a shading term is visible, and an enclosed chamber
/// is that frame with something in it to look at.
pub(crate) fn build_lamp_chamber(world: &mut World, gen: &WorldGen, cam: Vec3, dir: Vec3) {
    // Centred on the eye, because the camera stands *inside* this structure rather than in
    // front of it -- there is no surface to drop to and nothing to build 14 blocks away.
    let base = cam.floor().as_ivec3() - glam::IVec3::new(0, 2, 0);
    const R: i32 = 6;
    const H: i32 = 5;
    let mut edits: Vec<(glam::IVec3, u16)> = Vec::new();
    for dz in -R..=R {
        for dx in -R..=R {
            for dy in 0..=H {
                let p = base + glam::IVec3::new(dx, dy, dz);
                let shell = dx.abs() == R || dz.abs() == R || dy == 0 || dy == H;
                // Hollow inside, cobble shell. Cobble rather than stone because its texture is
                // the flattest grey in the atlas -- what has to be readable is the light, and a
                // wall whose own albedo is already mottled hides a gradient across it.
                edits.push((p, if shell { block::COBBLE } else { block::AIR }));
            }
        }
    }
    // **All four emitters in one wall, and the wall is the one the camera is looking at.**
    // Two earlier arrangements were built and rejected by the picture, which is worth a line
    // each because both are the obvious thing to try. Four lamps on four *different* walls put
    // two of them behind the eye and left the frame one mixing seam; the same four in the wall
    // *behind* the camera lit only the wall twelve blocks opposite, and `light::flood`
    // decrements one level per block, so at level 3 that wall is `light_curve(3)` = 0.027 and
    // the capture is black. Light in this engine is local, and a demonstration of it has to be
    // built where the eye already is.
    //
    // So: the lamps are in view, their own faces bright, and the wall between two of them is
    // the pixel that matters -- a seam is where two floods overlap, and no per-block tint
    // applied at read time can produce one however the tint is chosen.
    //
    // The room is axis-aligned and the look direction is not, so the wall is the dominant
    // horizontal axis of `dir` and the lamps run along the other one. A `dir` with both
    // horizontal components exactly zero -- straight up or straight down -- falls to the -X
    // wall rather than to the middle of the room, which is the only thing that matters about
    // the tie-break, and the vantage looks at neither.
    let (front, along) = if dir.x.abs() >= dir.z.abs() {
        (
            glam::IVec3::new(if dir.x > 0.0 { R } else { -R }, 0, 0),
            glam::IVec3::Z,
        )
    } else {
        (
            glam::IVec3::new(0, 0, if dir.z > 0.0 { R } else { -R }),
            glam::IVec3::X,
        )
    };
    // Eye height in the room, and four positions across it: spacing 3, 4, 3 in a wall spanning
    // -6..=6, so the outer two keep a block of margin from the corners. Glowstone is one of the
    // four deliberately -- the constant batch 59 retired is `BLOCK_TINT`, glowstone's row is
    // the 4-bit re-authoring of it, and the only way to judge that is beside the colours that
    // replaced it rather than against memory.
    let eye = 3;
    for (k, id) in [
        (-5, block::AMBER_LAMP),
        (-2, block::VERDANT_LAMP),
        (2, block::AZURE_LAMP),
        (5, block::GLOWSTONE),
    ] {
        edits.push((base + front + along * k + glam::IVec3::new(0, eye, 0), id));
    }
    let mut keys = rustc_hash::FxHashSet::default();
    for (p, id) in edits {
        if world.set_block(p, id) {
            keys.insert(ChunkKey::of_block(p));
        }
    }
    println!("  lamp chamber: {} chunks edited", keys.len());
    for key in keys {
        voxelcraft::stream::relight(world, gen, key);
    }
}

/// Place a glasshouse in front of the camera through the normal edit path, so a capture has
/// glass in it at all. Batch 38b.
///
/// **A second structure rather than glass added to [`build_demo_structure`], and that is the
/// whole reason this function exists.** The `edits` vantage has been one of the fixture's
/// fourteen since batch 29 and half a dozen batches have measured against it; putting a pane
/// in that hut would move a reference every one of them is quoted from, and would spend the
/// batch's cleanest claim -- that fourteen vantages are bit-exact against the parent build --
/// to save one function.
///
/// What it is shaped to exercise, in the order the batch measures them:
///
/// - **A single pane with terrain behind it.** The refracted leg's ordinary case, and most of
///   what the A/B moves.
/// - **A double pane**, one wall inset one block from another along the near side, so a ray
///   crosses two interfaces. It is the only thing in the fixture that can tell "the second
///   pane is skipped" from "the second pane is a wall", which is the claim `skip_glass` makes.
/// - **A solid roof and no light hole.** [`build_demo_structure`]'s hut leaves a 3x3 gap in
///   its planks "so sky light still reaches the inside" -- it has to, because planks are
///   `opaque: true`. This roof is glass and is closed, so the interior is lit *through* it or
///   not at all. That is the batch's world half, the half `--no-glass` cannot revert, and it
///   is deliberately built as the one thing a capture can see: with `opaque: true` restored,
///   this room is black.
/// - **Contents worth seeing through a window**: a plinth of stone and a glowstone, so the
///   transmitted image carries both a sky-lit and a block-lit surface.
///
/// **No underground branch, unlike [`build_demo_structure`].** That one lights a chamber
/// instead of building a hut when the camera is buried, because `--demo-edits` is reachable
/// from a cave camera; this is reached only from `--demo-glass`, whose one caller is a vantage
/// 14 blocks above the surface. Built underground it would be a glass box inside rock, which is
/// odd rather than wrong -- and a batch that wants a *buried* glasshouse should say so in a
/// vantage rather than get one by accident here.
pub(crate) fn build_glass_structure(world: &mut World, gen: &WorldGen, cam: Vec3, dir: Vec3) {
    let flat = Vec3::new(dir.x, 0.0, dir.z).normalize_or(Vec3::X);
    let centre = cam + flat * 14.0;
    let base = glam::IVec3::new(centre.x as i32, 0, centre.z as i32);
    let mut y = cam.y as i32;
    while y > 4 && !world.is_solid(glam::IVec3::new(base.x, y - 1, base.z)) {
        y -= 1;
    }
    let touched: Vec<_> = {
        let mut edits: Vec<(glam::IVec3, u16)> = Vec::new();
        for dz in -5..=5i32 {
            for dx in -5..=5i32 {
                edits.push((glam::IVec3::new(base.x + dx, y, base.z + dz), block::PLANKS));
                let edge = dx.abs() == 5 || dz.abs() == 5;
                if edge {
                    for h in 1..=4i32 {
                        edits.push((glam::IVec3::new(base.x + dx, y + h, base.z + dz), block::GLASS));
                    }
                }
                // The roof, and **no hole in it**: glass is `opaque: false`, so the flood
                // comes through. This is the one edit in the fixture whose point is what
                // happens to the light rather than what happens to a ray.
                edits.push((
                    glam::IVec3::new(base.x + dx, y + 5, base.z + dz),
                    block::GLASS,
                ));
            }
        }
        // The second pane: one wall inset a block from the -Z face, so a ray entering there
        // crosses two interfaces a block apart. Left open at the ends so the room is one
        // room and the inset is a pane rather than a partition.
        for dx in -3..=3i32 {
            for h in 1..=4i32 {
                edits.push((glam::IVec3::new(base.x + dx, y + h, base.z - 4), block::GLASS));
            }
        }
        // Something to look at through it, lit two ways.
        for h in 1..=2i32 {
            edits.push((glam::IVec3::new(base.x, y + h, base.z + 2), block::STONE));
        }
        edits.push((glam::IVec3::new(base.x, y + 3, base.z + 2), block::GLOWSTONE));

        let mut keys = rustc_hash::FxHashSet::default();
        for (p, id) in edits {
            if world.set_block(p, id) {
                keys.insert(ChunkKey::of_block(p));
            }
        }
        keys.into_iter().collect()
    };
    println!("  glass structure: {} chunks edited", touched.len());
    for key in touched {
        voxelcraft::stream::relight(world, gen, key);
    }
}

/// The crosshair and the hotbar. `selected` is [`Player::hotbar`] at both call sites --
/// **the capture path used to pass a literal 4**, which is a picture claiming a selection no
/// player held, and `--hotbar` now defaults to that same 4 so every `--hud` capture ever
/// taken still reads the same.
///
/// `bar` is [`Player::bar`] at both call sites and is batch 34's half of the same sentence:
/// this drew `block::HOTBAR` until then, which was a picture claiming contents no player
/// held the moment a player could hold different ones. Both arguments are the same argument,
/// and the reason the whole `--hud` corpus still reads identically is that a player who has
/// picked nothing holds exactly `block::HOTBAR`.
pub(crate) fn draw_hud(
    ui: &mut voxelcraft::render::ui::UiRenderer,
    sw: f32,
    sh: f32,
    selected: usize,
    bar: &block::Bar,
) {
    let (cx, cy) = (sw * 0.5, sh * 0.5);
    let c = [1.0, 1.0, 1.0, 0.75];
    ui.rect(cx - 9.0, cy - 1.0, 18.0, 2.0, c);
    ui.rect(cx - 1.0, cy - 9.0, 2.0, 18.0, c);

    let cell = 46.0;
    let bar_w = block::SLOTS as f32 * cell;
    let x0 = cx - bar_w * 0.5;
    let y0 = sh - cell - 14.0;
    ui.rect(
        x0 - 3.0,
        y0 - 3.0,
        bar_w + 6.0,
        cell + 6.0,
        [0.0, 0.0, 0.0, 0.45],
    );
    for (i, &id) in bar.iter().enumerate() {
        let x = x0 + i as f32 * cell;
        let sel = i == selected;
        ui.rect(
            x + 2.0,
            y0 + 2.0,
            cell - 4.0,
            cell - 4.0,
            if sel {
                [1.0, 1.0, 1.0, 0.30]
            } else {
                [0.0, 0.0, 0.0, 0.30]
            },
        );
        let name = block::def(id).name;
        ui.text_shadow(
            x + 5.0,
            y0 + cell * 0.5 - 5.0,
            2.0,
            &name[..name.len().min(5)],
            [0.95, 0.95, 0.95, 1.0],
        );
        ui.text_shadow(
            x + 5.0,
            y0 + 5.0,
            2.0,
            &format!("{}", (i + 1) % 10),
            [0.8, 0.8, 0.8, 1.0],
        );
    }
}

pub(crate) fn camera_basis(p: &Player) -> (Vec3, Vec3, Vec3) {
    let fwd = p.look_dir();
    let right = fwd.cross(Vec3::Y).normalize_or(Vec3::X);
    let up = right.cross(fwd).normalize_or(Vec3::Y);
    (fwd, right, up)
}

/// The second specialization word -- see `render::SPEC_HI_MASK`. Separate from `flags_from`
/// rather than folded into it because the two words have different destinations: that one is
/// uploaded into `GpuFrame` and this one never leaves the CPU, and a single function returning
/// both would make it easy to write a bit into the half that cannot carry it.
pub(crate) fn spec_hi_from(cfg: &Config) -> u32 {
    let mut f = 0u32;
    if cfg.isolate_glass { f |= render::FLAG_HI_ISOLATE_GLASS; }
    f |= render::FLAG_HI_SHADOW_PASS;
    if cfg.legacy_lighting { f |= render::FLAG_HI_LEGACY_LIGHTING; }
    if cfg.legacy_water { f |= render::FLAG_HI_LEGACY_WATER; }
    if cfg.soft_shadow_hq { f |= render::FLAG_HI_SOFT_HQ; }
    f |= cfg.surface_debug.min(7) << 23;
    if cfg.sun_softness == 1 { f |= render::FLAG_HI_SUN_NARROW; }
    if cfg.sun_softness == 2 { f |= render::FLAG_HI_SUN_WIDE; }
    if cfg.compact_shade_hit { f |= render::FLAG_HI_COMPACT_SHADE_HIT; }
    // Batch 65, unconditional on the world for `FLAG_SKY_TINT`'s reason exactly: every opaque
    // surface in the world reads it, so there is no world state that could make the arm
    // unreachable and nothing to nest it under.
    if cfg.sky_specular {
        f |= render::FLAG_HI_SKY_SPECULAR;
    }
    // Batch 72 (roadmap P10), and again unconditional on the world: whether a ray that
    // ignores water should walk a mixed-full chunk's water instead of stopping at its
    // face depends on no chunk's content, so there is nothing to nest it under.
    if cfg.full_march {
        f |= render::FLAG_HI_FULL_MARCH;
    }
    // Batch 75 (roadmap A3), nested exactly where `FLAG_WAVES` is nested: zeroed sea level
    // floods nothing, so there is no waterline for the band to lean on, no surface for the
    // foam to sit on, and the field could never be reached. **Second word**: the first
    // build put these in `flags_from` on bits that collided with `FLAG_PROBE_BOUNCE` and
    // `FLAG_LIGHT_RGB` -- the verification pass moved them here, and the lesson is written
    // at the constants.
    if cfg.sea_level > 0 {
        if cfg.shore_wet {
            f |= render::FLAG_HI_SHORE_WET;
        }
        if cfg.shore_foam {
            f |= render::FLAG_HI_SHORE_FOAM;
        }
        // Batch 81 (roadmap P12), nested on water existing for the same reason the shore
        // pair is: a zeroed sea level holds no water pixel for the pass to shade; the
        // `--no-water-sec` revert is a pipeline fold, free for the same `SPEC_` reason.
        if cfg.water_sec {
            f |= render::FLAG_HI_WATER_SEC;
        }
        // Batch 90 (roadmap A9), nested beside `water_sec` for its reason: no sea, no
        // surface, nothing to refract through. The `--caustics` build stays off by
        // default -- the by-eye pass sets the bit it reads.
        if cfg.caustics {
            f |= render::FLAG_HI_CAUSTICS;
        }
        // Batch 91 (roadmap A2c), nested beside `water_sec` for its reason: the bend needs
        // a boundary to exist -- no sea level, nothing refracts.
        if cfg.snell_bend {
            f |= render::FLAG_HI_SNELL_BEND;
        }
        // Batch 97c (roadmap G3's water half), nested beside the shore family for its
        // reason: a waterless world holds no surface to ripple and no column to murk.
        if cfg.water_look {
            f |= render::FLAG_HI_WATER_LOOK;
        }
    }
    // Batch 92 (roadmap A8), OUTSIDE the water trio's block above: ground tint is on land
    // and a zeroed sea level must not fold it away. Same shipping default as the family.
    if cfg.tint_balance {
        f |= render::FLAG_HI_TINT_BALANCE;
    }
    // Batch 101f (`--wind-sway`, roadmap G2's movement half), OUTSIDE any nesting for
    // the 97d/97e reason: with the arm off the cross-quads keep their planes, with the
    // world holding no tufts the spec bit is simply one more line the shader never
    // reaches -- the same honest shape `foliage_rich` uses, not a second `SPEC_FOLIAGE`
    // to keep in agreement.
    if cfg.wind_sway {
        f |= render::FLAG_HI_WIND_SWAY;
    }
    // Batch 102a (`--soft-shadows`), OUTSIDE any nesting for `FLAG_HI_SKY_SPECULAR`'s
    // reason exactly: the sun's disc reaches every surface the point sun does, so no
    // world state could make the arm unreachable. The shader pays the extra taps only
    // inside `FLAG_SHADOWS`, so `--ambient` keeps its no-shadows story whole.
    if cfg.soft_shadows {
        f |= render::FLAG_HI_SOFT_SHADOWS;
    }
    if cfg.foliage_rich {
        f |= render::FLAG_HI_FOLIAGE_RICH;
    }
    if cfg.canopy_relief {
        f |= render::FLAG_HI_CANOPY_RELIEF;
    }
    // Batch 90 (roadmap A4). Unconditional on the world for `FLAG_HI_SKY_SPECULAR`'s
    // reason: glass shades everywhere a pane can stand. Default off, per the batch's
    // standing rule that every open roadmap look entry builds dark.
    if cfg.glass_reflect {
        f |= render::FLAG_HI_GLASS_REFLECT;
    }
    // Batch 95 (roadmap G1): one bit for the four sky-look knobs. Unconditional on the
    // world for `FLAG_HI_SKY_SPECULAR`'s reason: the sky is behind everything there is.
    // The bit reads the *clamped* parse values, so an explicit 0.0 is the same build as
    // no flag -- the property the control's bit-exactness claim leans on.
    if cfg.clouds.patch > 0.0
        || cfg.clouds.relief > 0.0
        || cfg.sky.haze_warm > 0.0
        || cfg.sky.zenith_deep > 0.0
    {
        f |= render::FLAG_HI_SKY_LOOK;
    }
    // Batch 97a (roadmap G3's sky half). Unconditional on the world for
    // `FLAG_HI_SKY_LOOK`'s G1 reason exactly: the sky is behind everything there is.
    if cfg.sky_cool {
        f |= render::FLAG_HI_SKY_COOL;
    }
    f
}

pub(crate) fn flags_from(cfg: &Config, heatmap: bool, hiz: bool) -> u32 {
    let hiz = hiz && cfg.hiz;
    let mut f = 0;
    if cfg.shadows {
        f |= render::FLAG_SHADOWS;
    }
    if cfg.ao {
        f |= render::FLAG_AO;
    }
    if heatmap {
        f |= render::FLAG_HEATMAP;
    }
    if hiz {
        f |= render::FLAG_HIZ;
    }
    if cfg.water_reflect {
        f |= render::FLAG_WATER_REFLECT;
    }
    if cfg.water_refract {
        f |= render::FLAG_WATER_REFRACT;
    }
    // `--no-biomes` takes the tint with it: it reproduces a build from before there was a
    // biome field to tint from, and leaving a tint on a one-biome world would make it a
    // uniform multiply on every leaf rather than nothing at all.
    if cfg.tint && cfg.biomes {
        f |= render::FLAG_TINT;
    }
    // Straight off the generator's own answer rather than a second copy of `foliage &&
    // biomes`, because batch 15 folds this one into the shader: a flag that denied foliage a
    // `--no-biomes`-style rule had actually placed would draw every tuft as an opaque cube.
    if voxelcraft::worldgen::has_foliage(cfg.foliage, cfg.biomes) {
        f |= render::FLAG_FOLIAGE;
    }
    // `--no-water` takes the waves with it, and that is the same argument `FLAG_FOLIAGE`
    // makes rather than the one `FLAG_TINT` makes: a zeroed sea level means the generator
    // floods nothing and `block::WATER` is not in `block::HOTBAR`, so a run with no water
    // in it cannot acquire a water pixel later and the field could never be reached. What
    // makes it safe where the foliage version needed an assert to be safe is that the
    // failure mode is milder anyway -- a water block that somehow existed here would shade
    // as batch 8's flat surface, not as the wrong primitive.
    if cfg.wave_amp > 0.0 && cfg.sea_level > 0 {
        f |= render::FLAG_WAVES;
        // Nested rather than a second `&&` chain, so the grazing footprint cannot be asked
        // for in a run that has no wave field to filter: `SPEC_WAVE_ANISO` would then be a
        // live pipeline key selecting between two identical shaders.
        if cfg.wave_aniso {
            f |= render::FLAG_WAVE_ANISO;
        }
        if cfg.wave_shoal {
            f |= render::FLAG_WAVE_SHOAL;
        }
        if cfg.wave_fill {
            f |= render::FLAG_WAVE_FILL;
        }
    }
    // Batch 35. Unconditional on the world the way `FLAG_TINT` is not and `FLAG_WAVES` is
    // not: there is no configuration that removes the terrain, so unlike water and foliage
    // there is no "the field could never be reached" case to nest this under. `--no-biomes`
    // changes the heights and does not delete them, and `ShaftField` fills from the same
    // `WorldGen` the chunks came from, so the envelope follows a control rather than needing
    // to be cleared by one.
    if cfg.terrain_shafts {
        f |= render::FLAG_TERRAIN_SHAFTS;
    }
    // Batch 36, and unconditional for the same reason batch 35's is: no control removes the
    // ground, so unlike water and foliage there is no world in which the permutation could
    // never be reached. `--no-biomes` changes *which* blocks the generator places and cannot
    // take `GRASS_SIDE` out of it, so the class table follows a control rather than needing
    // to be cleared by one.
    if cfg.tex_variation {
        f |= render::FLAG_TEX_VARIATION;
    }
    // Batch 38, unconditional for the reason the permutation above it is: no control removes
    // the trees, and `--no-biomes` changes which leaf a biome plants rather than whether it
    // plants one. It is *not* nested under `foliage` -- ground cover and canopies are two
    // different blocks and two different primitives, and nesting them would make
    // `--no-foliage` quietly a leaf control as well.
    if cfg.leaf_cutout {
        f |= render::FLAG_LEAF_CUTOUT;
    }
    // Batch 45, unconditional for `glass`'s reason below rather than `leaf_cutout`'s above:
    // the arm it gates is reached from `shade_water` and `shade_glass`, and a player can build
    // a pane on any frame, so there is no world in which a secondary hit is impossible. A dry
    // world with no glass in it simply never calls either function, which costs nothing and is
    // the has-water bit of `GpuChunk::attr_flags` doing its job rather than this flag's.
    if cfg.flat_secondary {
        f |= render::FLAG_FLAT_SECONDARY;
    }
    // Batch 48, and unconditional for a reason none of the others here has: this bit is only
    // ever *read* ANDed with `FLAG_UNDERWATER`, which `underwater_flag` sets per frame from
    // where the camera is standing. So the nesting every other water flag has to spell out --
    // "a run with no sea can never reach it" -- is already enforced at the use, and a
    // condition here would be a second copy of it that could drift.
    if cfg.water_far {
        f |= render::FLAG_WATER_FAR;
    }
    // Batch 53, unconditional for the same reason one line up, and with a second nesting on
    // top of it that is also at the use rather than here: the rung asks the tile to still be
    // under the sky flood's reach where batch 48 stops, which no shallow camera satisfies.
    if cfg.water_dark {
        f |= render::FLAG_WATER_DARK;
    }
    // Batch 54, unconditional for the same reason again: the term is only reached with
    // `FLAG_UNDERWATER` set, so a dry run cannot tell this from the control by construction.
    if cfg.snell {
        f |= render::FLAG_SNELL;
    }
    // Batch 55, and the one entry here that is *false* by default. Unconditional on the world
    // for a reason none of the others has: the arm it gates reads a texture that exists in every
    // run and shades nothing, so there is no world in which it could not be reached and nothing
    // for a control to clear. What decides it is only ever `--probe-tap`.
    if cfg.probe_tap {
        f |= render::FLAG_PROBE_TAP;
    }
    // Batch 56, and the implication is enforced in `config.rs` where the flag is parsed rather
    // than here, so that `Config::probe_ambient` can never be true with `probe_tap` false in any
    // consumer -- `tests/probe.rs` holds it. Unconditional on the world for the tap's reason.
    if cfg.probe_ambient {
        f |= render::FLAG_PROBE_AMBIENT;
    }
    // Batch 57, unconditional on the world for a reason of its own: the lattice exists in
    // every run and is created holding the constants the control restores, so a world nobody
    // has baked a probe for renders the same frame either way. There is nothing here for a
    // control to clear and nothing about the world that could make the arm unreachable.
    if cfg.probe_cube {
        f |= render::FLAG_PROBE_CUBE;
    }
    // Batch 58, unconditional for batch 57's reason exactly -- the lattice exists in every run
    // and is created holding the identity, so a world nobody has baked a probe for renders the
    // same frame either way. It is also the same texel, so there is not even a second resource
    // whose absence a control could stand in for.
    if cfg.probe_bounce {
        f |= render::FLAG_PROBE_BOUNCE;
    }
    // Batch 60. Deliberately *not* nested under `probe_bounce`, and the reason is that the two
    // are one feature read from opposite ends: the bounce decides the floor's *colour* and this
    // decides how much energy that colour carries. Nesting would make `--no-probe-bounce` a
    // control for both and leave the more interesting half with no control of its own.
    if cfg.probe_sun {
        f |= render::FLAG_PROBE_SUN;
    }
    // The only `SPEC_MASK` bit here whose default is *off*: it raises the bounce gain rather
    // than reproducing anything, so the shipping build must not set it.
    if cfg.probe_sun_high {
        f |= render::FLAG_PROBE_SUN_HIGH;
    }
    // Batch 59, unconditional, and for `FLAG_GLASS`'s reason rather than batch 57's. The three
    // above it are unconditional because nothing about a world can make their arm unreachable;
    // this one is unconditional because an emitter can be *placed*. Nesting it under "does the
    // world hold a lamp" would compile the shipping build unable to read a channel the player
    // is about to fill, which is the `FLAG_FOLIAGE` hazard -- and unlike `TALL_GRASS` there is
    // no `bar_slot_refusal` available to close it, because a lamp in the bar is the point.
    if cfg.light_rgb {
        f |= render::FLAG_LIGHT_RGB;
    }
    // Batch 63, and unconditional for the plainest reason on this list: every shaded pixel in
    // the world reads the sky's hue, so there is no world state that could make the arm
    // unreachable and nothing to nest it under.
    if cfg.sky_tint {
        f |= render::FLAG_SKY_TINT;
    }
    // Batch 38b, unconditional -- and the one flag here for which "unconditional" is a
    // *requirement* rather than an observation. The three above it are unconditional because
    // no control can remove the terrain or the trees; this one is unconditional because
    // `block::GLASS` is in `block::HOTBAR`, so a world with no glass in it at load time can
    // acquire a pane on any frame. Nesting it under "does the world hold glass" would be the
    // `FLAG_FOLIAGE` hazard with no assert available to close it: the marcher would have been
    // compiled unable to see through a window the player is about to build.
    if cfg.glass {
        f |= render::FLAG_GLASS;
    }
    // Batch 37, and unconditional for the same reason the two above it are: no control removes
    // the terrain, so there is no world in which the envelope could never be reached. It is
    // deliberately *not* nested under `terrain_shafts` -- the two are independent readers of
    // one field, one shading the haze and one shading the ground, and nesting them would make
    // `--no-terrain-shafts` silently a second control for this as well.
    if cfg.distant_shadows {
        f |= render::FLAG_DISTANT_SHADOWS;
    }
    // Batch 41, and unconditional for a reason none of the four above it has: this one does
    // not gate a feature that a world might not contain, it selects the *value* of a constant
    // `shade_water`'s refraction leg reads every time it runs. Nesting it under "does this
    // world hold water" would be `ATTR_HAS_WATER`'s job done a second time and worse -- that
    // bit is per chunk and this would be per process.
    if cfg.water_shadow_cut {
        f |= render::FLAG_WATER_SHADOW_CUT;
    }
    // Batch 43, and unconditional for the reason the one above it is: it selects the *value*
    // of a constant rather than gating a feature. Not nested under `leaf_cutout` even though a
    // build with the cutout off never reads the fraction -- nesting would make
    // `--no-leaf-cutout` a second control for this, and one control per claim is the rule
    // `FLAG_DISTANT_SHADOWS` had to learn against `FLAG_TERRAIN_SHAFTS`.
    if cfg.leaf_thin {
        f |= render::FLAG_LEAF_THIN;
    }
    f
}

/// Whether this frame has to run the light envelope's scan.
///
/// Three ways for the answer to be no, and each one is eight dispatches saved: the feature is
/// off, no shadowing was asked for, or the sun is below the horizon. The third is the only one
/// that moves during a run, and it is what keeps a night frame from scanning a field whose
/// every value would be "shadowed" for a shaft that returns before it looks.
///
/// Here rather than at the three `FrameParams` call sites for this module's stated reason: a
/// fourth call site would be the one that forgot.
pub(crate) fn shaft_scan(cfg: &Config, flags: u32, sun_dir: Vec3) -> bool {
    // Batch 37 gave the field a second reader and this is where that lands: the scan has to
    // run if *either* wants it. The shaft's own condition keeps `--godray-strength 0`, which
    // is a control over the haze and has never been one over the ground -- `shade_hit`'s sun
    // term is not gated by it, so the distant half must not be either.
    let shaft = flags & render::FLAG_TERRAIN_SHAFTS != 0 && cfg.godrays.strength > 0.0;
    let ground = flags & render::FLAG_DISTANT_SHADOWS != 0;
    (shaft || ground) && sun_dir.y > 0.0
}

/// The wave parameters, straight off the config. Split out because three call sites build
/// `FrameParams` and a fourth would be the one that forgot.
pub(crate) fn waves_from(cfg: &Config) -> render::Waves {
    render::Waves {
        amp: cfg.wave_amp,
        scale: cfg.wave_scale,
        speed: cfg.wave_speed,
        reflect_slope: cfg.wave_reflect_slope,
    }
}

/// The medium the eye is in. Read off the actual block and not from the sea level, because
/// a cave below sea level is dry and the plane would say otherwise.
pub(crate) fn underwater_flag(world: &World, cam: Vec3) -> u32 {
    if world.get_block(math::block_of(cam)) == block::WATER {
        render::FLAG_UNDERWATER
    } else {
        0
    }
}

/// How a boolean setting reads in a status line.
///
/// Here rather than beside either caller because there are two -- the window's stats
/// overlay and `--bench-frames`'s summary -- and a second copy is how the two stop
/// agreeing about what `off` looks like.
pub(crate) fn on(b: bool) -> &'static str {
    if b {
        "on"
    } else {
        "off"
    }
}



