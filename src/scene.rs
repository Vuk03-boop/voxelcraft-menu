use glam::Vec3;
use voxelcraft::config::Config;
use voxelcraft::journal::EditJournal;
use voxelcraft::player::Player;
use voxelcraft::render;
use voxelcraft::voxel::{ChunkKey, World};
use voxelcraft::worldgen::WorldGen;
use voxelcraft::{block, math};

pub(crate) fn sun(t: f32) -> (Vec3, f32) {
    let a = (t - 0.25) * std::f32::consts::TAU;
    let dir = Vec3::new(a.cos() * 0.55, a.sin(), 0.42).normalize();
    let daylight = ((dir.y + 0.12) / 0.32).clamp(0.0, 1.0);
    (dir, daylight)
}

pub(crate) fn spawn_position(gen: &WorldGen) -> Vec3 {

    let h = gen.height(8, 8).max(gen.sea_level) as f32;
    Vec3::new(8.5, h + 2.0, 8.5)
}

pub(crate) fn start_player(cfg: &Config, journal: &EditJournal, player: &mut Player) {
    player.hotbar = cfg.hotbar;
    player.bar = voxelcraft::journal::bar_for(journal);
    if cfg.resume {
        if let Some(s) = journal.player() {
            player.restore(s);
        }
    }
}

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

pub(crate) fn find_cave(world: &World, near: Vec3) -> Option<Vec3> {
    let start = near.floor().as_ivec3();
    let clear = |p: glam::IVec3| {

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

pub(crate) fn build_demo_structure(world: &mut World, gen: &WorldGen, cam: Vec3, dir: Vec3) {

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

pub(crate) fn build_lamp_chamber(world: &mut World, gen: &WorldGen, cam: Vec3, dir: Vec3) {

    let base = cam.floor().as_ivec3() - glam::IVec3::new(0, 2, 0);
    const R: i32 = 6;
    const H: i32 = 5;
    let mut edits: Vec<(glam::IVec3, u16)> = Vec::new();
    for dz in -R..=R {
        for dx in -R..=R {
            for dy in 0..=H {
                let p = base + glam::IVec3::new(dx, dy, dz);
                let shell = dx.abs() == R || dz.abs() == R || dy == 0 || dy == H;

                edits.push((p, if shell { block::COBBLE } else { block::AIR }));
            }
        }
    }

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

                edits.push((
                    glam::IVec3::new(base.x + dx, y + 5, base.z + dz),
                    block::GLASS,
                ));
            }
        }

        for dx in -3..=3i32 {
            for h in 1..=4i32 {
                edits.push((glam::IVec3::new(base.x + dx, y + h, base.z - 4), block::GLASS));
            }
        }

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

pub(crate) fn spec_hi_from(cfg: &Config) -> u32 {
    let mut f = 0u32;
    if cfg.isolate_glass { f |= render::FLAG_HI_ISOLATE_GLASS; }
    if cfg.exp_halfres_glass { f |= render::FLAG_HI_GLASS_QUAD; }
    f |= render::FLAG_HI_SHADOW_PASS;
    if cfg.legacy_lighting { f |= render::FLAG_HI_LEGACY_LIGHTING; }
    if cfg.legacy_water { f |= render::FLAG_HI_LEGACY_WATER; }
    if cfg.soft_shadow_hq { f |= render::FLAG_HI_SOFT_HQ; }
    f |= cfg.surface_debug.min(7) << 23;
    if cfg.sun_softness == 1 { f |= render::FLAG_HI_SUN_NARROW; }
    if cfg.sun_softness == 2 { f |= render::FLAG_HI_SUN_WIDE; }
    if cfg.compact_shade_hit { f |= render::FLAG_HI_COMPACT_SHADE_HIT; }

    if cfg.sky_specular {
        f |= render::FLAG_HI_SKY_SPECULAR;
    }

    if cfg.full_march {
        f |= render::FLAG_HI_FULL_MARCH;
    }

    if cfg.sea_level > 0 {
        if cfg.shore_wet {
            f |= render::FLAG_HI_SHORE_WET;
        }
        if cfg.shore_foam {
            f |= render::FLAG_HI_SHORE_FOAM;
        }

        if cfg.water_sec {
            f |= render::FLAG_HI_WATER_SEC;
        }

        if cfg.caustics {
            f |= render::FLAG_HI_CAUSTICS;
        }

        if cfg.snell_bend {
            f |= render::FLAG_HI_SNELL_BEND;
        }

        if cfg.water_look {
            f |= render::FLAG_HI_WATER_LOOK;
        }
    }

    if cfg.tint_balance {
        f |= render::FLAG_HI_TINT_BALANCE;
    }

    if cfg.wind_sway {
        f |= render::FLAG_HI_WIND_SWAY;
    }

    if cfg.soft_shadows {
        f |= render::FLAG_HI_SOFT_SHADOWS;
    }
    if cfg.foliage_rich {
        f |= render::FLAG_HI_FOLIAGE_RICH;
    }
    if cfg.canopy_relief {
        f |= render::FLAG_HI_CANOPY_RELIEF;
    }

    if cfg.glass_reflect {
        f |= render::FLAG_HI_GLASS_REFLECT;
    }

    if cfg.clouds.patch > 0.0
        || cfg.clouds.relief > 0.0
        || cfg.sky.haze_warm > 0.0
        || cfg.sky.zenith_deep > 0.0
    {
        f |= render::FLAG_HI_SKY_LOOK;
    }

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

    if cfg.tint && cfg.biomes {
        f |= render::FLAG_TINT;
    }

    if voxelcraft::worldgen::has_foliage(cfg.foliage, cfg.biomes) {
        f |= render::FLAG_FOLIAGE;
    }

    if cfg.wave_amp > 0.0 && cfg.sea_level > 0 {
        f |= render::FLAG_WAVES;

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

    if cfg.terrain_shafts {
        f |= render::FLAG_TERRAIN_SHAFTS;
    }

    if cfg.tex_variation {
        f |= render::FLAG_TEX_VARIATION;
    }

    if cfg.leaf_cutout {
        f |= render::FLAG_LEAF_CUTOUT;
    }

    if cfg.flat_secondary {
        f |= render::FLAG_FLAT_SECONDARY;
    }

    if cfg.water_far {
        f |= render::FLAG_WATER_FAR;
    }

    if cfg.water_dark {
        f |= render::FLAG_WATER_DARK;
    }

    if cfg.snell {
        f |= render::FLAG_SNELL;
    }

    if cfg.probe_tap {
        f |= render::FLAG_PROBE_TAP;
    }

    if cfg.probe_ambient {
        f |= render::FLAG_PROBE_AMBIENT;
    }

    if cfg.probe_cube {
        f |= render::FLAG_PROBE_CUBE;
    }

    if cfg.probe_bounce {
        f |= render::FLAG_PROBE_BOUNCE;
    }

    if cfg.probe_sun {
        f |= render::FLAG_PROBE_SUN;
    }

    if cfg.probe_sun_high {
        f |= render::FLAG_PROBE_SUN_HIGH;
    }

    if cfg.light_rgb {
        f |= render::FLAG_LIGHT_RGB;
    }

    if cfg.sky_tint {
        f |= render::FLAG_SKY_TINT;
    }

    if cfg.glass {
        f |= render::FLAG_GLASS;
    }

    if cfg.distant_shadows {
        f |= render::FLAG_DISTANT_SHADOWS;
    }

    if cfg.water_shadow_cut {
        f |= render::FLAG_WATER_SHADOW_CUT;
    }

    if cfg.leaf_thin {
        f |= render::FLAG_LEAF_THIN;
    }
    f
}

pub(crate) fn shaft_scan(cfg: &Config, flags: u32, sun_dir: Vec3) -> bool {

    let shaft = flags & render::FLAG_TERRAIN_SHAFTS != 0 && cfg.godrays.strength > 0.0;
    let ground = flags & render::FLAG_DISTANT_SHADOWS != 0;
    (shaft || ground) && sun_dir.y > 0.0
}

pub(crate) fn waves_from(cfg: &Config) -> render::Waves {
    render::Waves {
        amp: cfg.wave_amp,
        scale: cfg.wave_scale,
        speed: cfg.wave_speed,
        reflect_slope: cfg.wave_reflect_slope,
    }
}

pub(crate) fn underwater_flag(world: &World, cam: Vec3) -> u32 {
    if world.get_block(math::block_of(cam)) == block::WATER {
        render::FLAG_UNDERWATER
    } else {
        0
    }
}

pub(crate) fn on(b: bool) -> &'static str {
    if b {
        "on"
    } else {
        "off"
    }
}
