use crate::lod::LodConfig;
use crate::render::{Clouds, Fog, GodRays, SkyLook};
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Config {
    pub seed: i32,

    pub sea_level: i32,

    pub water_absorb: f32,

    pub water_mottle: f32,

    pub water_reflect: bool,

    pub water_refract: bool,

    pub shore_wet: bool,

    pub shore_foam: bool,

    pub water_sec: bool,

    pub water_sec_scale: u32,

    pub tone_map_aces: bool,

    pub sky_cool: bool,

    pub grade_warm: bool,

    pub grade_cine: bool,

    pub grade_strength: f32,

    pub water_look: bool,

    pub foliage_rich: bool,

    pub canopy_relief: bool,

    pub caustics: bool,

    pub glass_reflect: bool,

    pub canopy_lift: f32,

    pub tree_blue_noise: bool,

    pub meadow_side: bool,

    pub grass_dense: bool,

    pub wind_sway: bool,

    pub soft_shadows: bool,

    pub compact_shade_hit: bool,

    pub isolate_glass: bool,
    pub shadow_pass: bool,
    pub legacy_lighting: bool,
    pub legacy_water: bool,
    pub soft_shadow_hq: bool,

    pub surface_debug: u32,

    pub sun_softness: u32,
    pub shadow_dist: f32,

    pub snell_bend: bool,

    pub shaft_texel: i32,

    pub tint_balance: bool,

    pub biomes: bool,

    pub foliage: bool,

    pub meadow: bool,

    pub bounce_shadow: bool,

    pub tint: bool,

    pub tint_strength: f32,

    pub wave_amp: f32,

    pub wave_scale: f32,

    pub wave_speed: f32,

    pub wave_reflect_slope: f32,

    pub wave_aniso: bool,

    pub wave_shoal: bool,

    pub wave_fill: bool,

    pub terrain_shafts: bool,

    pub tex_variation: bool,

    pub leaf_cutout: bool,

    pub flat_secondary: bool,

    pub water_far: bool,

    pub water_dark: bool,

    pub snell: bool,

    pub probe_tap: bool,

    pub probe_ambient: bool,

    pub probe_cube: bool,

    pub probe_bounce: bool,

    pub probe_sun: bool,

    pub probe_sun_high: bool,

    pub light_rgb: bool,

    pub sky_tint: bool,

    pub sky_specular: bool,

    pub full_march: bool,

    pub probe_fill: Option<f32>,

    pub probe_noise: bool,

    pub march_stats: bool,

    pub glass: bool,

    pub distant_shadows: bool,

    pub water_shadow_cut: bool,

    pub leaf_thin: bool,
    pub lod: LodConfig,
    pub width: u32,
    pub height: u32,

    pub render_scale: f32,

    pub jitter: Option<(f32, f32)>,

    pub taa: bool,

    pub taa_frames: usize,

    pub mip_bias: f32,

    pub reference: u32,
    pub fov_degrees: f32,
    pub vsync: bool,
    pub shadows: bool,
    pub hiz: bool,
    pub hud: bool,

    pub idle_repaint: bool,

    pub hotbar: usize,

    pub bar: Option<String>,
    pub demo_edits: bool,

    pub demo_glass: bool,

    pub demo_lamps: bool,

    pub load_edits: Option<String>,

    pub save_edits: Option<String>,

    pub resume: bool,

    pub edits: Option<String>,

    pub no_exit_save: bool,

    pub ambient: f32,

    pub fog: Fog,

    pub clouds: Clouds,

    pub sky: SkyLook,

    pub godrays: GodRays,
    pub ao: bool,
    pub physics: bool,

    pub time_of_day: f32,
    pub day_length_seconds: f32,
    pub freeze_time: bool,
    pub stream_budget_ms: f32,

    pub cam_height: f32,
    pub cam_yaw: f32,
    pub cam_pitch: f32,

    pub cam_spin: f32,

    pub cam_dolly: f32,

    pub cam_submerge: f32,

    pub anim_time: f32,

    pub anim_rate: f32,
}

impl Config {

    pub fn journal_in(&self) -> Option<(&Path, bool)> {
        match (&self.edits, &self.load_edits) {
            (Some(p), _) => Some((Path::new(p), false)),
            (None, Some(p)) => Some((Path::new(p), true)),
            (None, None) => None,
        }
    }

    pub fn journal_out(&self) -> Option<&Path> {
        if self.no_exit_save {
            return None;
        }
        self.edits
            .as_deref()
            .or(self.save_edits.as_deref())
            .map(Path::new)
    }
}

impl Config {

    pub fn worldgen(&self) -> crate::worldgen::WorldGen {
        let mut gen = crate::worldgen::WorldGen::with_options(
            self.seed,
            self.sea_level,
            self.biomes,
            self.foliage,
            self.meadow,
        );

        gen.bounce_shadow = self.bounce_shadow;

        gen.tree_blue_noise = self.tree_blue_noise;

        gen.meadow_side = self.meadow_side;

        gen.grass_dense = self.grass_dense;
        gen
    }
}

impl Config {

    pub fn grade_value(&self) -> u32 {
        self.grade_warm as u32 | u32::from(self.grade_cine) << 1
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            seed: 1337,
            sea_level: crate::worldgen::SEA_LEVEL,
            water_absorb: 1.0,

            water_mottle: 0.0,
            water_reflect: true,
            water_refract: true,
            shore_wet: true,
            shore_foam: true,
            water_sec: true,
            water_sec_scale: 2,
            tone_map_aces: false,
            grade_warm: false,
            sky_cool: false,
            grade_cine: false,
            grade_strength: 1.0,
            water_look: false,
            foliage_rich: false,
            canopy_relief: false,
            caustics: false,
            glass_reflect: false,
            canopy_lift: 0.0,
            tree_blue_noise: false,
            meadow_side: false,
            grass_dense: false,
            wind_sway: false,
            soft_shadows: false,
            compact_shade_hit: false,
            isolate_glass: false,
            shadow_pass: true,
            legacy_lighting: false, legacy_water: false, soft_shadow_hq: false,
            surface_debug: 0, sun_softness: 0, shadow_dist: 220.0,
            snell_bend: false,
            shaft_texel: 4,
            tint_balance: false,
            biomes: true,
            foliage: true,
            meadow: true,
            bounce_shadow: true,
            tint: true,
            tint_strength: 1.0,
            wave_amp: 0.12,
            wave_scale: 32.0,
            wave_speed: 0.32,
            wave_reflect_slope: 0.0,
            wave_aniso: true,
            wave_shoal: true,
            wave_fill: true,
            terrain_shafts: true,
            tex_variation: true,
            leaf_cutout: true,
            flat_secondary: true,
            water_far: true,
            water_dark: true,
            snell: true,
            probe_tap: false,
            probe_ambient: false,
            probe_cube: true,
            probe_bounce: true,
            probe_sun: true,
            probe_sun_high: false,
            light_rgb: true,
            sky_tint: true,
            sky_specular: true,
            full_march: true,
            probe_fill: None,
            probe_noise: false,
            march_stats: false,
            glass: true,
            distant_shadows: true,
            water_shadow_cut: true,
            leaf_thin: true,
            lod: LodConfig::default(),
            width: 1600,
            height: 900,
            render_scale: 1.0,
            jitter: None,
            taa: true,
            taa_frames: 32,
            mip_bias: 0.0,
            reference: 0,
            fov_degrees: 70.0,
            vsync: true,
            shadows: true,
            hiz: true,
            hud: false,
            idle_repaint: true,
            hotbar: 4,
            bar: None,
            demo_edits: false,
            demo_glass: false,
            demo_lamps: false,
            load_edits: None,
            save_edits: None,
            resume: false,
            edits: None,
            no_exit_save: false,
            ambient: 0.08,
            fog: Fog::default(),
            clouds: Clouds::default(),
            sky: SkyLook::default(),
            godrays: GodRays::default(),
            ao: true,
            physics: true,
            time_of_day: 0.30,
            day_length_seconds: 600.0,
            freeze_time: false,
            stream_budget_ms: 2.0,
            cam_height: 40.0,
            cam_yaw: 40.0,
            cam_pitch: -14.0,
            cam_spin: 0.0,
            cam_dolly: 0.0,
            cam_submerge: 0.0,
            anim_time: 0.0,
            anim_rate: 0.0,
        }
    }
}

pub enum Mode {
    Run,
    Screenshot { path: String },

    Lookbook { dir: String },
    BenchTerrain { chunks: usize },
    BenchFrames { frames: usize },

    Diff {
        a: String,
        b: String,
        grid: Option<(u32, u32)>,
        crop: Option<[f32; 4]>,
    },
}

fn parse_pair(s: &str) -> Option<(f32, f32)> {
    let (a, b) = s.split_once(',').unwrap_or((s, s));
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

pub const UNKNOWN_ARG_MARK: &str = "ignoring unknown argument";

pub fn parse_args() -> (Config, Mode) {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (cfg, mode, unknown) = parse_from(&args);
    for a in unknown {
        eprintln!("{UNKNOWN_ARG_MARK}: {a}");
    }
    (cfg, mode)
}

pub fn parse_from(args: &[String]) -> (Config, Mode, Vec<String>) {
    let mut cfg = Config::default();
    let mut mode = Mode::Run;
    let mut unknown = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let next = |i: &mut usize| -> String {
            *i += 1;
            args.get(*i).cloned().unwrap_or_default()
        };
        match a {
            "--screenshot" => mode = Mode::Screenshot { path: next(&mut i) },
            "--lookbook" => mode = Mode::Lookbook { dir: next(&mut i) },
            "--bench-frames" => {
                mode = Mode::BenchFrames {
                    frames: next(&mut i).parse().unwrap_or(120),
                }
            }
            "--diff" => {

                let (a, b) = (next(&mut i), next(&mut i));
                let (mut grid, mut crop) = (None, None);
                while args.get(i + 1).is_some_and(|v| v.starts_with("--")) {
                    i += 1;
                    match args[i].as_str() {
                        "--grid" => {
                            let v = next(&mut i);
                            if let Some((gx, gy)) = parse_pair(&v) {
                                grid = Some((gx.max(1.0) as u32, gy.max(1.0) as u32));
                            }
                        }
                        "--crop" => {
                            let v = next(&mut i);
                            let q: Vec<f32> =
                                v.split(',').filter_map(|t| t.trim().parse().ok()).collect();
                            if q.len() == 4 {
                                crop = Some([q[0], q[1], q[2], q[3]]);
                            }
                        }
                        _ => {
                            i -= 1;
                            break;
                        }
                    }
                }
                mode = Mode::Diff { a, b, grid, crop };
            }
            "--bench-terrain" => {
                mode = Mode::BenchTerrain {
                    chunks: next(&mut i).parse().unwrap_or(64),
                }
            }
            "--seed" => cfg.seed = next(&mut i).parse().unwrap_or(cfg.seed),
            "--sea-level" => cfg.sea_level = next(&mut i).parse().unwrap_or(cfg.sea_level),
            "--no-water" => cfg.sea_level = 0,
            "--water-absorb" => cfg.water_absorb = next(&mut i).parse().unwrap_or(cfg.water_absorb),
            "--water-mottle" => { let _ = next(&mut i); cfg.water_mottle=0.0; eprintln!("water mottle is fixed at zero to keep the atlas layer flat"); },
            "--no-water-reflect" => cfg.water_reflect = false,
            "--no-water-refract" => cfg.water_refract = false,
            "--no-shore-wet" => cfg.shore_wet = false,
            "--no-shore-foam" => cfg.shore_foam = false,
            "--no-water-sec" => cfg.water_sec = false,
            "--water-sec-scale" => {
                let s: u32 = next(&mut i).parse().unwrap_or(cfg.water_sec_scale);
                cfg.water_sec_scale = match s {
                    1 | 2 | 4 => s,
                    _ => cfg.water_sec_scale,
                };
            }

            "--tone-map" => {

                cfg.tone_map_aces = next(&mut i) == "aces";
            }

            "--grade" => {

                let v = next(&mut i);
                cfg.grade_warm = v == "warm";
                cfg.grade_cine = v == "cine";
            }

            "--grade-strength" => {
                cfg.grade_strength = next(&mut i).parse().unwrap_or(1.0);
            }
            "--sky-cool" => cfg.sky_cool = true,
            "--water-look" => cfg.water_look = true,
            "--foliage-rich" => cfg.foliage_rich = true,
            "--canopy-relief" => cfg.canopy_relief = true,
            "--caustics" => cfg.caustics = true,
            "--glass-reflect" => cfg.glass_reflect = true,
            "--canopy-lift" => {
                let v: f32 = next(&mut i).parse().unwrap_or(0.0);
                cfg.canopy_lift = v.max(0.0);
            }
            "--tree-blue-noise" => cfg.tree_blue_noise = true,
            "--meadow-side" => cfg.meadow_side = true,
            "--grass-dense" => cfg.grass_dense = true,
            "--wind-sway" => cfg.wind_sway = true,
            "--soft-shadows" => cfg.soft_shadows = true,
            "--no-soft-shadows" => cfg.soft_shadows = false,
            "--no-wind-sway" => cfg.wind_sway = false,
            "--no-water-look" => cfg.water_look = false,
            "--no-glass-reflect" => cfg.glass_reflect = false,
            "--isolate-glass" => cfg.isolate_glass = true,
            "--no-isolate-glass" => cfg.isolate_glass = false,
            "--legacy-lighting" => cfg.legacy_lighting = true,
            "--repaired-lighting" => cfg.legacy_lighting = false,
            "--legacy-water" => cfg.legacy_water = true,
            "--repaired-water" => cfg.legacy_water = false,
            "--soft-shadow-hq" => cfg.soft_shadow_hq = true,
            "--no-soft-shadow-hq" => cfg.soft_shadow_hq = false,
            "--sun-softness" => {
                cfg.sun_softness = match next(&mut i).as_str() {
                    "narrow" => 1, "wide" => 2, "normal" => 0,
                    _ => { eprintln!("--sun-softness expects narrow, normal or wide; using normal"); 0 }
                };
            }
            "--surface-debug" => {
                cfg.surface_debug = match next(&mut i).as_str() {
                    "off" => 0, "normals" => 1, "shadow" => 2, "direct" => 3,
                    "indirect" => 4, "water-faces" => 5, "reflection" => 6, "refraction" => 7,
                    _ => { eprintln!("Unknown --surface-debug mode; using off"); 0 }
                };
            }
            "--shadow-pass" => cfg.shadow_pass = true,
            "--no-shadow-pass" => { cfg.shadow_pass = true; eprintln!("--no-shadow-pass retired: primary shadows now always use the dedicated mask"); },
            "--compact-shade-hit" => cfg.compact_shade_hit = true,
            "--no-compact-shade-hit" => cfg.compact_shade_hit = false,
            "--snell-bend" => cfg.snell_bend = true,
            "--shaft-texel" => {
                let v: i32 = next(&mut i).parse().unwrap_or(4);
                cfg.shaft_texel = v.clamp(1, 64);
            }

            "--cloud-patch" => {
                let v: f32 = next(&mut i).parse().unwrap_or(0.0);
                cfg.clouds.patch = v.clamp(0.0, 1.0);
            }
            "--cloud-relief" => {
                let v: f32 = next(&mut i).parse().unwrap_or(0.0);
                cfg.clouds.relief = v.clamp(0.0, 1.0);
            }
            "--haze-warm" => {
                let v: f32 = next(&mut i).parse().unwrap_or(0.0);
                cfg.sky.haze_warm = v.clamp(0.0, 1.0);
            }
            "--zenith-deep" => {
                let v: f32 = next(&mut i).parse().unwrap_or(0.0);
                cfg.sky.zenith_deep = v.clamp(0.0, 1.0);
            }
            "--tint-balance" => cfg.tint_balance = true,
            "--no-biomes" => cfg.biomes = false,
            "--no-foliage" => cfg.foliage = false,
            "--no-meadow" => cfg.meadow = false,
            "--no-probe-shadow" => cfg.bounce_shadow = false,
            "--no-tint" => cfg.tint = false,
            "--tint-strength" => {
                cfg.tint_strength = next(&mut i).parse().unwrap_or(cfg.tint_strength)
            }
            "--wave-amp" => cfg.wave_amp = next(&mut i).parse().unwrap_or(cfg.wave_amp),
            "--no-waves" => cfg.wave_amp = 0.0,
            "--wave-scale" => cfg.wave_scale = next(&mut i).parse().unwrap_or(cfg.wave_scale),
            "--wave-speed" => cfg.wave_speed = next(&mut i).parse().unwrap_or(cfg.wave_speed),
            "--no-wave-aniso" => cfg.wave_aniso = false,
            "--no-wave-shoal" => cfg.wave_shoal = false,
            "--no-wave-fill" => cfg.wave_fill = false,
            "--no-terrain-shafts" => cfg.terrain_shafts = false,
            "--no-tex-variation" => cfg.tex_variation = false,
            "--no-leaf-cutout" => cfg.leaf_cutout = false,
            "--no-flat-secondary" => cfg.flat_secondary = false,
            "--no-water-far" => cfg.water_far = false,
            "--no-water-dark" => cfg.water_dark = false,
            "--no-snell" => cfg.snell = false,
            "--probe-tap" => cfg.probe_tap = true,

            "--probe-ambient" => {
                cfg.probe_ambient = true;
                cfg.probe_tap = true;
            }
            "--no-probe-cube" => cfg.probe_cube = false,
            "--no-probe-bounce" => cfg.probe_bounce = false,
            "--no-probe-sun" => cfg.probe_sun = false,
            "--probe-sun-high" => cfg.probe_sun_high = true,
            "--no-light-rgb" => cfg.light_rgb = false,
            "--no-sky-tint" => cfg.sky_tint = false,
            "--no-sky-specular" => cfg.sky_specular = false,
            "--no-full-march" => cfg.full_march = false,
            "--probe-fill" => cfg.probe_fill = next(&mut i).parse().ok(),
            "--probe-noise" => cfg.probe_noise = true,
            "--march-stats" => cfg.march_stats = true,
            "--no-glass" => cfg.glass = false,
            "--shadow-dist" => cfg.shadow_dist = next(&mut i).parse().unwrap_or(cfg.shadow_dist),
            "--no-distant-shadows" => cfg.distant_shadows = false,
            "--no-water-shadow-cut" => cfg.water_shadow_cut = false,
            "--no-leaf-thin" => cfg.leaf_thin = false,
            "--wave-clamp" => {
                cfg.wave_reflect_slope = next(&mut i).parse().unwrap_or(cfg.wave_reflect_slope)
            }
            "--width" => cfg.width = next(&mut i).parse().unwrap_or(cfg.width),
            "--height" => cfg.height = next(&mut i).parse().unwrap_or(cfg.height),
            "--scale" => cfg.render_scale = next(&mut i).parse().unwrap_or(cfg.render_scale),
            "--jitter" => cfg.jitter = parse_pair(&next(&mut i)).or(cfg.jitter),
            "--taa-frames" => cfg.taa_frames = next(&mut i).parse().unwrap_or(cfg.taa_frames),
            "--mip-bias" => cfg.mip_bias = next(&mut i).parse().unwrap_or(cfg.mip_bias),
            "--reference" => cfg.reference = next(&mut i).parse().unwrap_or(8),
            "--no-taa" => cfg.taa = false,
            "--fov" => cfg.fov_degrees = next(&mut i).parse().unwrap_or(cfg.fov_degrees),
            "--view-distance" => {
                cfg.lod.view_distance = next(&mut i).parse().unwrap_or(cfg.lod.view_distance)
            }
            "--max-lod" => cfg.lod.max_lod = next(&mut i).parse().unwrap_or(cfg.lod.max_lod),
            "--streaming-factor" => cfg.lod.factor = next(&mut i).parse().unwrap_or(cfg.lod.factor),
            "--fade-band" => cfg.lod.fade_band = next(&mut i).parse().unwrap_or(cfg.lod.fade_band),
            "--fade-schedule" => {
                cfg.lod.fade_schedule = match next(&mut i).as_str() {
                    "smoothstep" => crate::lod::FadeSchedule::Smoothstep,
                    "smootherstep" => crate::lod::FadeSchedule::Smootherstep,
                    _ => cfg.lod.fade_schedule,
                }
            }
            "--no-fade" => cfg.lod.cross_fade = false,
            "--no-shadow-share" => cfg.lod.shadow_share = false,
            "--no-offscreen-shadows" => cfg.lod.offscreen_shadows = false,
            "--time" => {
                cfg.time_of_day = next(&mut i).parse().unwrap_or(cfg.time_of_day);
                cfg.freeze_time = true;
            }
            "--cam-height" => cfg.cam_height = next(&mut i).parse().unwrap_or(cfg.cam_height),
            "--cam-yaw" => cfg.cam_yaw = next(&mut i).parse().unwrap_or(cfg.cam_yaw),
            "--cam-pitch" => cfg.cam_pitch = next(&mut i).parse().unwrap_or(cfg.cam_pitch),
            "--cam-spin" => cfg.cam_spin = next(&mut i).parse().unwrap_or(cfg.cam_spin),
            "--cam-dolly" => cfg.cam_dolly = next(&mut i).parse().unwrap_or(cfg.cam_dolly),
            "--anim-time" => cfg.anim_time = next(&mut i).parse().unwrap_or(cfg.anim_time),
            "--anim-rate" => cfg.anim_rate = next(&mut i).parse().unwrap_or(cfg.anim_rate),
            "--cam-submerge" => cfg.cam_submerge = next(&mut i).parse().unwrap_or(3.0),
            "--no-vsync" => cfg.vsync = false,
            "--no-idle-repaint" => cfg.idle_repaint = false,
            "--no-shadows" => cfg.shadows = false,
            "--no-hiz" => cfg.hiz = false,
            "--hud" => cfg.hud = true,
            "--demo-edits" => cfg.demo_edits = true,
            "--demo-glass" => cfg.demo_glass = true,
            "--demo-lamps" => cfg.demo_lamps = true,
            "--load-edits" => cfg.load_edits = Some(next(&mut i)),
            "--save-edits" => cfg.save_edits = Some(next(&mut i)),
            "--resume" => cfg.resume = true,
            "--edits" => cfg.edits = Some(next(&mut i)),
            "--no-exit-save" => cfg.no_exit_save = true,

            "--bar" => cfg.bar = Some(next(&mut i)),
            "--hotbar" => {
                cfg.hotbar = next(&mut i)
                    .parse::<usize>()
                    .unwrap_or(cfg.hotbar)
                    .min(crate::block::HOTBAR.len() - 1)
            }
            "--ambient" => cfg.ambient = next(&mut i).parse().unwrap_or(cfg.ambient),
            "--fog-density" => cfg.fog.density = next(&mut i).parse().unwrap_or(cfg.fog.density),
            "--fog-falloff" => cfg.fog.falloff = next(&mut i).parse().unwrap_or(cfg.fog.falloff),
            "--fog-scatter" => cfg.fog.scatter = next(&mut i).parse().unwrap_or(cfg.fog.scatter),
            "--fog-g" => cfg.fog.g = next(&mut i).parse().unwrap_or(cfg.fog.g),
            "--cloud-cover" => cfg.clouds.cover = next(&mut i).parse().unwrap_or(cfg.clouds.cover),
            "--cloud-height" => {
                cfg.clouds.height = next(&mut i).parse().unwrap_or(cfg.clouds.height)
            }
            "--cloud-scale" => cfg.clouds.scale = next(&mut i).parse().unwrap_or(cfg.clouds.scale),
            "--cloud-speed" => cfg.clouds.speed = next(&mut i).parse().unwrap_or(cfg.clouds.speed),
            "--godray-strength" => {
                cfg.godrays.strength = next(&mut i).parse().unwrap_or(cfg.godrays.strength)
            }
            "--godray-steps" => {
                cfg.godrays.steps = next(&mut i).parse().unwrap_or(cfg.godrays.steps)
            }
            "--godray-dist" => cfg.godrays.dist = next(&mut i).parse().unwrap_or(cfg.godrays.dist),
            "--cloud-shadow" => {
                cfg.godrays.cloud_shadow = next(&mut i).parse().unwrap_or(cfg.godrays.cloud_shadow)
            }
            "--no-ao" => cfg.ao = false,
            "--no-physics" => cfg.physics = false,
            "--help" | "-h" => {
                println!("Surface repair: --legacy-lighting / --repaired-lighting; --legacy-water / --repaired-water.\n--sun-softness narrow|normal|wide; --soft-shadow-hq / --no-soft-shadow-hq (requires soft shadows).\n--surface-debug off|normals|shadow|direct|indirect|water-faces|reflection|refraction");
                println!("Primary shadows always use one ray + optional screen-space reconstruction; --shadow-pass is default, --no-shadow-pass is retired.\nSecondary water/glass shading remains isolated (--isolate-glass, --no-isolate-glass compatibility); timings include the full resolve family.");
                println!("Experimental: --compact-shade-hit / --no-compact-shade-hit (unmeasured schedule A/B).\n\
                     Existing arms also accept --no-soft-shadows, --no-wind-sway, --no-water-look, --no-glass-reflect.");
                println!(
                    "voxelcraft\n\
                     \n\
                     --screenshot PATH        render one frame headless to a PNG and exit\n\
                     --bench-terrain N        generate N chunks, print timings, and exit\n\
                     --bench-frames N         render N frames, print per-pass timings, and exit\n\
                     --seed N                 world seed (default 1337)\n\
                     --sea-level N            internal Y the basins flood to (default 128)\n\
                     --no-water               same terrain, no water in it\n\
                     --no-biomes              one biome everywhere: the pre-batch-9 world\n\
                     --no-foliage             no grass tufts: the pre-batch-14 world\n\
                     --no-meadow              no coarse meadow: the pre-batch-18 world\n\
                     --no-probe-shadow        probe bounce ignores whether the ground\n\
                                              it gathers is itself lit\n\
                     --no-probe-sun           ambient floor is the authored constant\n\
                     --probe-sun-high         the louder bounce gain (0.25 not 0.10)\n\
                     --no-tint                no per-biome vegetation tint\n\
                     --tint-strength F        how far the biome tint is taken (default 1)\n\
                     --water-absorb F         scale water's extinction (0 = perfectly clear)\n\
                     --water-mottle F         water layer mottling (retired: parsed, warned, force-zeroed)\n\
                     --cam-height F           camera height in internal Y (default 40)\n\
                     --cam-yaw D              camera yaw in degrees (default 40)\n\
                     --cam-pitch D            camera pitch in degrees (default -14)\n\
                     --cam-submerge F         capture F blocks under water (negative: above)\n\
                     --hud                    draw the stats overlay in a screenshot too\n\
                     --demo-edits             build the demo structures through the edit path\n\
                     --demo-glass             build a glasshouse through the edit path (batch 38b)\n\
                     --demo-lamps             build a chamber of coloured lamps (batch 59)\n\
                     --load-edits PATH        replay a saved edit journal after worldgen\n\
                     --save-edits PATH        write the edit journal to PATH when the mode ends\n\
                     --edits PATH             a world on disk: load if there, append, save\n\
                     --no-exit-save           end without the exit save, the way a kill does\n\
                     --resume                 start where the loaded journal's player was\n\
                     --hotbar N               hotbar slot to start on, 0-9 (default 4)\n\
                     --bar A,B,..            the ten block ids in the bar (default 1,2,3,4,8,6,7,9,10,11)\n\
                     --width N --height N     window size\n\
                     --scale F                internal render scale (default 1.0)\n\
                     --jitter X[,Y]           pin the sub-pixel ray offset (default: Halton)\n\
                     --taa-frames N           frames a screenshot converges over (default 32)\n\
                     --reference N            converge a screenshot over an NxN sub-pixel grid\n\
                     --mip-bias F             extra texture mip bias in resolve\n\
                     --cam-spin D             degrees of yaw per converged frame\n\
                     --cam-dolly F            blocks of forward travel per converged frame\n\
                     --anim-time F            seconds of wave/cloud phase at the captured frame\n\
                     --anim-rate F            seconds of that phase per converged frame\n\
                     --fov D                  vertical field of view in degrees\n\
                     --view-distance F        far plane in blocks (default 2600)\n\
                     --max-lod N              coarsest LOD level (default 4)\n\
                     --streaming-factor F     LOD subdivision aggressiveness (default 2.0)\n\
                     --fade-band F            outer edge of the LOD cross-fade (default 1.25)\n\
\n\
                     --fade-schedule NAME     LOD cross-fade curve: smoothstep (default) or\n\
                                              smootherstep (D2b's measurement arm)\n\
                     --diff A.png B.png       compare two captures: MAE, differ count, bbox as\n\
                                              crop fractions; --grid GX,GY adds a per-cell table\n\
                                              with the peak cell marked (needs no renderer)\n\
                     --crop X0,Y0,X1,Y1        with --diff: fractions of the image to compare
                     --no-shore-wet           dry sand at the waterline (reverts batch 75's band)\n\
                     --no-shore-foam          no foam over shallow columns (reverts batch 75's mottle)\n\
                     --no-water-sec           full-res traced water legs (reverts batch 81's
                                              half-res diagnostic: P12's measurement arm)\n\
                     --water-sec-scale N      water legs per NxN block: 1, 2 (default) or 4\n\
                     --cloud-patch P         G1: cumulus arrive in banks; P swings the coverage\n\
                                               threshold by up to the full noise range (default 0)\n\
                     --cloud-relief R        G1: sun-side cap vs belly shading on the deck,\n\
                                               shading only -- never adds density (default 0)\n\
                     --haze-warm W           G1: warm band hugging the horizon treeline\n\
                                               (default 0 = the pre-95 sky)\n\
                     --zenith-deep Z         G1: how much bluer the zenith runs at day\n\
                                               (default 0 = the pre-95 sky)\n\
                     --lookbook DIR          batch 98: 3 views x 12 settings composed into\n\
                                              one labeled PNG (lookbook.png) for comparison\n\
                     --grade none|warm|cine  warm luma-preserved cast (batch 95) or the\n\
                                              filmic cine print (batch 97b); default none\n\
                                              is the pre-95 presentation bit for bit\n\
                     --grade-strength F       batch 101: the grade dial; 1.0 keeps the\n\
                                              selected cast exact, below it lerps back\n\
                     --sky-cool               the reference frames' cornflower midday sky:\n\
                                              constant swap on gradient and deck (batch 97a)\n\
                     --water-look              ripples, murk absorption and a turquoise bank\n\
                                              band -- the references' water (batch 97c)\n\
                     --foliage-rich            ground-cover species variety on tufts and\n\
                                              grass/meadow tops (batch 97d)\n\
                     --canopy-relief           canopy grain: clump brightness relief and\n\
                                              sun-facing modulation on leaves (batch 97e)\n\
                     --tone-map knee|aces     filmic shoulder at the blit (batch 90's A10 arm:\n\
                                              knee is what every constant was tuned against)\n\
                     --caustics               sun sheets on the flooded floor, from the wave\n\
                                              field's own octaves (batch 90, roadmap A9)\n\
                     --glass-reflect          panes reflect the traced world, not the sky\n\
                                              (batch 90, roadmap A4)\n\
                     --canopy-lift F          hide forest canopy from the shaft envelope by\n\
                                              F blocks (batch 90, roadmap A1)\n\
                     --tree-blue-noise        thin coarse canopy proxies to LOD 0's fill\n\
                                              (batch 91, roadmap A5's LOD-cliff arm)\n\
                     --grass-dense            dense two-height tufts at LOD 0, blue-noise\n\
                                              tuft proxies on the coarse shell, reeds on\n\
                                              the wet banks (batch 101, roadmap G2)\n\
                     --wind-sway              gust shears on the grass cross-quads,\n\
                                              traveling sines in world x/z (batch 101,\n\
                                              roadmap G2's movement half)\n\
                     --soft-shadows           sun-disc penumbra: three cone taps around\n\
                                              the centre tap, budget by blocker distance\n\
                                              (batch 102a)\n\
                     --meadow-side            coarse meadow's side faces repainted too\n\
                                              (batch 92, roadmap A8's one atlas slice)\n\
                     --snell-bend             refract the primary march at the sea plane\n\
                                              (batch 91, roadmap A2c's window arm)\n\
                     --shaft-texel N          envelope blocks per texel: finer edge, shorter\n\
                                              window (batch 92, roadmap A8's swept knob)\n\
                     --tint-balance             balanced biome tints, masks on sand and lichen\n\
                                              (batch 92, roadmap A8's palette arm)\n\
                     --time F                 fix time of day, 0..1 (0.5 = noon)\n\
                     --ambient F              linear light floor (default 0.08)\n\
                     --fog-density F          haze extinction per block at sea level\n\
                     --fog-falloff F          reciprocal fog scale height in blocks\n\
                     --fog-scatter F          strength of the sun-scattering lobe\n\
                     --fog-g F                lobe asymmetry, 0 isotropic to 1 forward\n\
                     --cloud-cover F          cloud coverage 0..1 (0 = no deck at all)\n\
                     --cloud-height F         internal Y of the deck (default 448)\n\
                     --cloud-scale F          blocks per base cloud octave (default 320)\n\
                     --cloud-speed F          blocks per second of drift (default 3)\n\
                     --godray-strength F      shadowing of the haze's sun lobe (0 = off)
\
                     --godray-steps N         samples along the view ray (default 4)
\
                     --godray-dist F          how far the shaft samples spread (default 4096)
\
                     --cloud-shadow F         sun the deck takes off the ground (0 = off)
\
                     --no-terrain-shafts      no terrain-cast shafts: pair with the next for pre-batch-35
\
                     --no-tex-variation       no per-block texture permutation: the pre-batch-36 frame
\
                     --no-distant-shadows     no terrain shadow past the march: the pre-batch-37 frame
                     --shadow-dist F          cap the sun-shadow march in blocks (default 220.0)
                     --no-leaf-cutout         leaves are solid cubes again: the pre-batch-38 frame
                     --no-flat-secondary      hits through water or glass take the nine-cell gather
                                              again: the pre-batch-45 frame
                     --no-water-far           a submerged camera tests chunks to the full view
                                              distance again: the pre-batch-48 frame
                     --no-water-dark          a submerged tile below the sky flood's reach looks
                                              as far as --no-water-far lets it: the pre-batch-53 frame
                     --no-snell               a submerged eye sees through the water surface at every
                                              angle again: the pre-batch-54 frame
                     --probe-tap              take one trilinear tap into the probe field per shaded
                                              surface, on top of the shading. Implies --probe-fill 1.0,
                                              so it is bit-exact and reports only a millisecond
                     --probe-ambient          that field REPLACES the ambient, and the terms it
                                              makes redundant leave the shader. Implies --probe-tap.
                                              The frame is wrong on purpose: a build to measure
                     --no-probe-cube          take the directional shading factor from face_shade's
                                              per-normal constants again, not from the baked cube
                     --no-probe-bounce        take the ambient floor from one authored constant
                                              again, not from the baked ground bounce
                     --no-light-rgb           read block light as one level wearing one warm
                                              tint again: the pre-batch-59 frame
                     --no-sky-tint            take the ambient's sky colour from one authored
                                              constant again, not from the sky model: the
                                              pre-batch-63 frame
                     --no-sky-specular        make every opaque surface perfectly diffuse
                                              again, with no Fresnel-weighted reflection of
                                              the sky: the pre-batch-65 frame
                     --no-full-march          stop every ray at a mixed-full chunk's face
                                              again, water-ignoring legs included: the
                                              pre-batch-72 frame
                     --probe-fill F           pin the probe field to the constant F and switch the
                                              bake off. At 1.0 the cube is a no-op; at anything
                                              else the frame must move, or nothing is wired up
                     --probe-noise            fill the probe field per texel instead of uniformly
                     --march-stats            print march's DDA steps and pair counts, and write
                                              the heatmap rather than the frame. Not a timing mode
                     --no-glass               glass is an opaque cube again. NOT a pure revert: the
                                              light flood still runs through a pane
                     --no-water-shadow-cut    a sea floor marches its own shadow 48 blocks again
                                              instead of 16: the pre-batch-41 frame
                     --no-leaf-thin           a canopy keeps 0.72 of each leaf block instead of
                                              0.62: the pre-batch-43 frame
\
                     --no-vsync --no-shadows --no-ao --no-physics --no-hiz --no-taa\n\
                     --no-idle-repaint        render every frame even when nothing moved\n\
                     --no-fade                pop between LOD levels instead of dissolving\n\
                     --no-shadow-share        shadow grid takes the fine LOD half, not the majority\n\
                     --no-offscreen-shadows   only chunks the frustum keeps cast shadows\n\
                     --no-water-reflect       drop the traced reflection off the water\n\
                     --no-water-refract       drop the traced refraction under the water\n\
                     --no-waves               mirror-flat water: the pre-batch-16 sea\n\
                     --wave-amp F             peak slope of the water micro-normals\n\
                     --wave-scale F           blocks per base wave octave (default 32)\n\
                     --wave-speed F           wave periods per second (0 = frozen)\n\
                     --wave-clamp F           slope cap on the traced reflection (default 0)\n\
                     --no-wave-aniso          wave detail ignores the grazing angle (pre-batch-19)\n\
                     --no-wave-shoal          full swell even in shallow water (pre-batch-20)\n\
                     --no-wave-fill           four wave octaves instead of eight (pre-batch-25)\n\
                     \n\
                     In game: WASD move, mouse look, Space/Shift up-down, F fly, Ctrl fast,\n\
                     LMB break, RMB place, 1-0 or scroll pick block, F3 stats, F4 render scale,\n\
                     F5 iteration heatmap, F6 Hi-Z culling, F7 TAA, Escape release cursor."
                );
                std::process::exit(0);
            }
            _ => unknown.push(a.to_string()),
        }
        i += 1;
    }

    if (cfg.probe_tap || cfg.probe_ambient) && cfg.probe_fill.is_none() {
        cfg.probe_fill = Some(1.0);
    }
    (cfg, mode, unknown)
}
