use std::sync::Arc;
use std::time::{Duration, Instant};

use glam::{Vec2, Vec3};
use voxelcraft::config::Config;
use voxelcraft::journal::{self, EditJournal};
use voxelcraft::player::Player;
use voxelcraft::render::{self, FrameParams, Renderer};
use voxelcraft::shaft::ShaftField;
use voxelcraft::stats::fmt_bytes;
use voxelcraft::stream::ChunkManager;
use voxelcraft::voxel::{ChunkKey, World};
use voxelcraft::math;

use crate::scene::{
    build_demo_structure, build_glass_structure, build_lamp_chamber, camera_basis, draw_hud, find_cave,
    find_water, flags_from, on, spec_hi_from,
    spawn_position, start_player, sun, underwater_flag, waves_from,
};

pub(crate) fn bench_terrain(cfg: &Config, chunks: usize) {
    let gen = Arc::new(cfg.worldgen());
    let mut world = World::new();
    let mut dense = vec![0u16; voxelcraft::voxel::VOL];
    let mut heights = Box::new([0i32; 64 * 64]);

    let side = (chunks as f64).cbrt().ceil() as i32;
    let mut keys = Vec::new();
    'outer: for z in 0..side * 2 {
        for x in 0..side * 2 {
            for y in 1..4 {
                keys.push(ChunkKey::new(0, glam::IVec3::new(x, y, z)));
                if keys.len() >= chunks {
                    break 'outer;
                }
            }
        }
    }

    let mut gen_ns = 0u128;
    let mut build_ns = 0u128;
    let mut intern_ns = 0u128;
    let mut light_ns = 0u128;

    let mut probe_ns = 0u128;
    for &key in &keys {
        let t0 = Instant::now();
        gen.generate(key, &mut dense, &mut heights);
        let t1 = Instant::now();
        let tree = voxelcraft::voxel::build_local(&dense);
        let t2 = Instant::now();
        let open = voxelcraft::light::open_columns(&heights, key.origin().y + 64);
        let light = voxelcraft::light::compute(&dense, &open);
        let t3 = Instant::now();
        let probe = voxelcraft::probe::bake(&gen, &heights, key.origin(), gen.bounce_shadow);
        let t4 = Instant::now();
        world.insert_local(key, tree);
        world.set_light(key, light);
        world.set_probe(key, probe);
        let t5 = Instant::now();
        gen_ns += (t1 - t0).as_nanos();
        build_ns += (t2 - t1).as_nanos();
        light_ns += (t3 - t2).as_nanos();
        probe_ns += (t4 - t3).as_nanos();
        intern_ns += (t5 - t4).as_nanos();
    }

    let n = keys.len() as f64;
    let solid = world.total_solid_voxels();
    let geom = world.geometry_bytes();
    let attr = world.bricks.bytes();
    println!("chunks:             {}", keys.len());
    println!("solid voxels:       {solid}");
    println!();
    println!("per chunk (us)");
    println!("  worldgen          {:8.1}", gen_ns as f64 / n / 1000.0);
    println!("  tree build        {:8.1}", build_ns as f64 / n / 1000.0);
    println!("  lighting          {:8.1}", light_ns as f64 / n / 1000.0);
    println!("  probe bake        {:8.1}", probe_ns as f64 / n / 1000.0);
    println!("  intern + upload   {:8.1}", intern_ns as f64 / n / 1000.0);
    println!(
        "  total             {:8.1}",
        (gen_ns + build_ns + light_ns + probe_ns + intern_ns) as f64 / n / 1000.0
    );
    println!();
    println!("memory");
    println!(
        "  geometry          {:>10}  ({:.3} bytes/voxel)",
        fmt_bytes(geom as u64),
        geom as f64 / solid.max(1) as f64
    );
    println!(
        "  attributes+light  {:>10}  ({:.3} bytes/voxel)",
        fmt_bytes(attr as u64),
        attr as f64 / solid.max(1) as f64
    );
    println!(
        "  leaf runs live    {} ({:.2}x dedup)",
        world.leaves.live_runs(),
        world.leaves.dedup_ratio()
    );
    println!(
        "  inner runs live   {} ({:.2}x dedup)",
        world.inners.live_runs(),
        world.inners.dedup_ratio()
    );
}

pub(crate) fn lookbook(cfg: &Config, dir: &str) -> render::anyhow_lite::Result<()> {

    use voxelcraft::lookbook as lb;
    std::fs::create_dir_all(dir)?;
    let mut groups = Vec::new();
    for view in &lb::VIEWS {
        let mut tiles = Vec::new();
        for combo in &lb::COMBOS {
            let mut c = cfg.clone();

            assert!(
                lb::apply_view(&mut c, view),
                "lookbook view `{}` uses arguments the parser no longer knows",
                view.name
            );
            (combo.apply)(&mut c);

            c.hud = false;

            c.reference = 0;
            println!("lookbook: {} / {}", view.name, combo.label);
            let pixels = capture(&c, "lookbook")?
                .expect("reference is clamped to 0, so capture returns pixels");
            tiles.push(lb::Tile {
                label: combo.label.to_string(),
                width: c.width,
                height: c.height,
                pixels,
            });
        }
        groups.push(lb::compose_group(view, &tiles));
    }
    let (w, h, px) = lb::compose_sheet(&groups);
    let path = format!("{dir}/lookbook.png");
    image::save_buffer(&path, &px, w, h, image::ColorType::Rgba8)?;
    println!("wrote {path} ({w}x{h})");
    Ok(())
}

fn settle(manager: &mut ChunkManager, world: &mut World, cam: Vec3) {
    for _ in 0..4000 {
        manager.update(cam, world, Duration::from_millis(50));
        if manager.is_settled(world) {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

pub(crate) fn screenshot(cfg: &Config, path: &str) -> render::anyhow_lite::Result<()> {
    if let Some(pixels) = capture(cfg, path)? {
        image::save_buffer(
            path,
            &pixels,
            cfg.width,
            cfg.height,
            image::ColorType::Rgba8,
        )?;
        println!("wrote {path}");
    }
    Ok(())
}

fn capture(cfg: &Config, path: &str) -> render::anyhow_lite::Result<Option<Vec<u8>>> {
    let instance = render::make_instance();
    let gpu = pollster::block_on(render::init_gpu(&instance, None))?;
    let mut renderer = Renderer::new(
        gpu,
        (cfg.width, cfg.height),
        wgpu::TextureFormat::Rgba8Unorm,
        cfg.water_mottle,
        cfg.tint_balance,
        cfg.probe_fill,
        cfg.probe_noise,
        cfg.lod,
        cfg.water_sec_scale.trailing_zeros(),
        cfg.tone_map_aces as u32,
        cfg.grade_value(),
        cfg.grade_strength,
    );
    let gen = Arc::new(cfg.worldgen());
    let journal = journal::open(cfg)?;
    let mut world = World::new();
    world.attach_journal(journal.clone());
    let mut manager = ChunkManager::new(cfg.lod, gen.clone());
    manager.attach_journal(journal.clone());

    let ground = spawn_position(&gen);
    let mut player = Player::new(ground + Vec3::new(0.0, cfg.cam_height, 0.0));
    player.yaw = cfg.cam_yaw.to_radians();
    player.pitch = cfg.cam_pitch.to_radians();
    start_player(cfg, &journal, &mut player);
    let cam = player.eye();

    println!("generating world...");
    let start = Instant::now();
    settle(&mut manager, &mut world, cam);
    println!(
        "  {} chunks in {:.1}s",
        world.chunks.len(),
        start.elapsed().as_secs_f32()
    );

    let streamed_from = cam;

    let cam = if cfg.cam_height < 0.0 && !cfg.resume {
        match find_cave(&world, cam) {
            Some(p) => {
                println!(
                    "  cave found at {:.0} {:.0} {:.0}",
                    p.x,
                    p.y - math::Y_OFFSET as f32,
                    p.z
                );
                player.pos = p - Vec3::new(0.0, voxelcraft::player::EYE, 0.0);
                p
            }
            None => {
                println!("  no cave found near the spawn column");
                cam
            }
        }
    } else {
        cam
    };

    let cam = if cfg.cam_submerge != 0.0 && !cfg.resume {
        match find_water(&world, cam, cfg.sea_level, cfg.cam_submerge) {
            Some(p) => {
                println!(
                    "  {} at {:.0} {:.0} {:.0}",
                    if cfg.cam_submerge > 0.0 {
                        "submerged"
                    } else {
                        "afloat"
                    },
                    p.x,
                    p.y - math::Y_OFFSET as f32,
                    p.z
                );
                player.pos = p - Vec3::new(0.0, voxelcraft::player::EYE, 0.0);
                p
            }
            None => {
                println!("  no water found near the spawn column");
                cam
            }
        }
    } else {
        cam
    };

    if cam != streamed_from {
        settle(&mut manager, &mut world, cam);
        println!(
            "  replanned at the capture camera: {} chunks",
            world.chunks.len()
        );
    }

    if cfg.demo_edits || cfg.demo_glass || cfg.demo_lamps {
        if cfg.demo_lamps {
            build_lamp_chamber(&mut world, &gen, cam, player.look_dir());
        } else if cfg.demo_glass {
            build_glass_structure(&mut world, &gen, cam, player.look_dir());
        } else {
            build_demo_structure(&mut world, &gen, cam, player.look_dir());
        }

        let n = manager.rebuild_dirty_now(&mut world);
        println!("  coarse stand-ins rebuilt: {n}");
    }

    journal::save_if_asked(cfg, &journal, player.state(), player.bar)?;

    renderer.sync_world(&mut world, cam);
    let (sun_dir, daylight) = sun(cfg.time_of_day);

    let present = renderer
        .gpu
        .device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("headless present"),
            size: wgpu::Extent3d {
                width: cfg.width,
                height: cfg.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
    let present_view = present.create_view(&Default::default());

    let internal = (
        ((cfg.width as f32 * cfg.render_scale) as u32).max(8),
        ((cfg.height as f32 * cfg.render_scale) as u32).max(8),
    );
    renderer.resize(internal, (cfg.width, cfg.height));

    let frames = if cfg.taa { cfg.taa_frames.max(5) } else { 5 };

    let mut shaft = ShaftField::with_texel(cfg.shaft_texel);

    let out = if cfg.reference > 0 {
        converge_reference(
            &mut renderer,
            &world,
            &manager,
            &player,
            cfg,
            sun_dir,
            daylight,
            &mut shaft,
            &present,
            &present_view,
            path,
        )?;
        None
    } else {
        let end_yaw = player.yaw;
        let end_pos = player.pos;
        for i in 0..frames {

            player.yaw = end_yaw - cfg.cam_spin.to_radians() * (frames - 1 - i) as f32;
            player.pos = end_pos - player.look_dir() * (cfg.cam_dolly * (frames - 1 - i) as f32);

            let time = cfg.anim_time - cfg.anim_rate * (frames - 1 - i) as f32;
            if cfg.hud && i == frames - 1 {
                draw_hud(
                    &mut renderer.ui,
                    cfg.width as f32,
                    cfg.height as f32,
                    player.hotbar,
                    &player.bar,
                );
            }
            shot_frame(
                &mut renderer,
                &world,
                &manager,
                &player,
                cfg,
                sun_dir,
                daylight,
                time,
                cfg.jitter.map(Vec2::from),
                cfg.taa,
                &mut shaft,
                &present_view,
            );
        }

        let pixels = renderer.read_texture(&present, (cfg.width, cfg.height));
        println!(
            "captured {}x{}{}, {} chunks drawn, {} in grid, {}",
            cfg.width,
            cfg.height,
            if cfg.render_scale != 1.0 {
                format!(" traced at {}x{}", internal.0, internal.1)
            } else {
                String::new()
            },
            renderer.last_chunk_count,
            renderer.last_grid_chunks,
            if cfg.taa {
                format!("taa converged over {frames} frames")
            } else {
                "taa off".into()
            }
        );
        Some(pixels)
    };

    if cfg.march_stats {
        let m = renderer.march_stats();

        println!(
            "march-stats: {} iters over {} touched pixels of {} ({:.2} per touched, worst {})",
            m.iters, m.touched, m.pixels,
            m.iters as f64 / (m.touched.max(1) as f64),
            m.worst
        );
        if let Some(d) = m.pairs {
            println!(
                "march-stats: pairs select {} deferred {} recover {} of MAX_PAIRS {} ({:.1}%)",
                d.select, d.deferred, d.recover, d.cap,
                100.0 * d.worst() as f64 / d.cap as f64
            );
        }
        println!(
            "march-stats: p50 {} p90 {} p99 {} worst {} | top 1% hold {:.1}% of steps,              top 10% hold {:.1}%",
            m.p50, m.p90, m.p99, m.worst, m.top_1, m.top_10
        );
        println!(
            "march-stats: capped {} pixels past MAX_ITERS ({}), {} chunks in the frame list",
            m.capped,
            render::MARCH_MAX_ITERS,
            m.chunks
        );
    }
    renderer.warn_if_oversized();
    renderer.warn_if_truncated();
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
fn shot_frame(
    renderer: &mut Renderer,
    world: &World,
    manager: &ChunkManager,
    player: &Player,
    cfg: &Config,
    sun_dir: Vec3,
    daylight: f32,

    time: f32,
    jitter: Option<Vec2>,
    taa: bool,
    shaft: &mut ShaftField,
    view: &wgpu::TextureView,
) {
    let cam = player.eye();
    let (fwd, right, up) = camera_basis(player);
    let frustum = math::Frustum::from_camera(
        cam,
        fwd,
        right,
        up,
        (cfg.fov_degrees.to_radians() * 0.5).tan(),
        cfg.width as f32 / cfg.height as f32,
        cfg.lod.view_distance,
    );
    renderer.build_chunk_list(&manager.render_set, world, cam, &frustum);

    shaft.update(manager.generator(), cam.x, cam.z, cfg.canopy_lift);
    if shaft.dirty() {
        renderer.upload_shaft(shaft.heights());
    }
    let flags = flags_from(cfg, cfg.march_stats, cfg.hiz) | underwater_flag(world, cam);
    let params = FrameParams {
        cam_pos: cam,
        cam_fwd: fwd,
        cam_right: right,
        cam_up: up,
        fov_y: cfg.fov_degrees.to_radians(),
        far: cfg.lod.view_distance,
        sun_dir,
        daylight,
        time,
        flags,
        spec_hi: spec_hi_from(cfg)
            | if world.any_emitter_resident() {
                render::FLAG_HI_EMITTER_WORLD
            } else {
                0
            },
        ambient: cfg.ambient,
        shadow_dist: cfg.shadow_dist,
        fog: cfg.fog,
        clouds: cfg.clouds,
        sky: cfg.sky,
        godrays: cfg.godrays,
        sea_level: cfg.sea_level as f32,
        water_absorb: cfg.water_absorb,
        jitter,
        taa,
        mip_bias: cfg.mip_bias,
        tint: render::Tint {
            strength: cfg.tint_strength,
            seed: cfg.seed,
        },
        waves: waves_from(cfg),
        shaft_origin: shaft.origin_world(),
        shaft_texel: shaft.texel() as f32,
        shaft_scan: crate::scene::shaft_scan(cfg, flags, sun_dir),
    };
    renderer.render(&params, Some(view));
    renderer
        .gpu
        .device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .ok();
}

#[allow(clippy::too_many_arguments)]
fn converge_reference(
    renderer: &mut Renderer,
    world: &World,
    manager: &ChunkManager,
    player: &Player,
    cfg: &Config,
    sun_dir: Vec3,
    daylight: f32,
    shaft: &mut ShaftField,
    present: &wgpu::Texture,
    present_view: &wgpu::TextureView,
    path: &str,
) -> render::anyhow_lite::Result<()> {
    use voxelcraft::harness::metric::{self, Img, Rect};

    let n = cfg.reference;
    let (w, h) = (cfg.width, cfg.height);
    let len = (w as usize) * (h as usize) * 4;
    let mut acc = vec![0f32; len];
    let mut halves = [vec![0f32; len], vec![0f32; len]];
    let mut half_n = [0f32; 2];

    let mut order: Vec<u32> = (0..n * n).collect();
    order.sort_by_key(|&t| {
        let mut hash = t.wrapping_mul(0x9E37_79B1);
        hash ^= hash >> 15;
        hash.wrapping_mul(0x85EB_CA6B)
    });
    let mut in_a = vec![false; (n * n) as usize];
    for &t in order.iter().take((n * n / 2) as usize) {
        in_a[t as usize] = true;
    }

    for _ in 0..4 {
        shot_frame(
            renderer,
            world,
            manager,
            player,
            cfg,
            sun_dir,
            daylight,
            cfg.anim_time,
            Some(Vec2::ZERO),
            false,
            shaft,
            present_view,
        );
    }

    for j in 0..n {
        for i in 0..n {

            let off = Vec2::new(
                (i as f32 + 0.5) / n as f32 - 0.5,
                (j as f32 + 0.5) / n as f32 - 0.5,
            );
            shot_frame(
                renderer,
                world,
                manager,
                player,
                cfg,
                sun_dir,
                daylight,
                cfg.anim_time,
                Some(off),
                false,
                shaft,
                present_view,
            );
            let bytes = renderer.read_texture(present, (w, h));
            let k = usize::from(!in_a[(j * n + i) as usize]);
            half_n[k] += 1.0;
            for (t, &b) in bytes.iter().enumerate() {

                let lin = if t % 4 == 3 {
                    b as f32 / 255.0
                } else {
                    metric::srgb_to_linear(b)
                };
                acc[t] += lin;
                halves[k][t] += lin;
            }
        }
        println!("  reference row {}/{}", j + 1, n);
    }

    let total = (n * n) as f32;
    let encode = |buf: &[f32], count: f32| -> Img {
        Img {
            w,
            h,
            px: buf
                .iter()
                .enumerate()
                .map(|(t, &v)| {
                    let mean = v / count;
                    if t % 4 == 3 {
                        (mean * 255.0 + 0.5).clamp(0.0, 255.0) as u8
                    } else {
                        metric::linear_to_srgb(mean)
                    }
                })
                .collect(),
        }
    };

    let mean = encode(&acc, total);
    let a = encode(&halves[0], half_n[0].max(1.0));
    let b = encode(&halves[1], half_n[1].max(1.0));
    let stem = path.strip_suffix(".png").unwrap_or(path);
    let (pa, pb) = (format!("{stem}.halfa.png"), format!("{stem}.halfb.png"));
    mean.save(std::path::Path::new(path))?;
    a.save(std::path::Path::new(&pa))?;
    b.save(std::path::Path::new(&pb))?;

    println!(
        "wrote {path} ({w}x{h}), {} samples on a {n}x{n} sub-pixel grid, averaged in linear",
        total as u32
    );
    println!("  halves {} + {} -> {pa}, {pb}", half_n[0], half_n[1]);
    if half_n[0] < 1.0 || half_n[1] < 1.0 {

        println!("  noise floor: not available below 2 samples");
    } else {
        let floor = metric::mae(&a, &b, Rect::full(w, h));
        println!("  noise floor (half-split MAE, whole frame): {floor:.4}");
        println!("  an effect smaller than the floor is not measurable against this reference.");
    }
    Ok(())
}

fn headless_scene(cfg: &Config, journal: Arc<EditJournal>) -> (World, ChunkManager, Player) {
    let gen = Arc::new(cfg.worldgen());
    let mut world = World::new();
    world.attach_journal(journal.clone());
    let mut manager = ChunkManager::new(cfg.lod, gen.clone());
    manager.attach_journal(journal.clone());
    let ground = spawn_position(&gen);
    let mut player = Player::new(ground + Vec3::new(0.0, cfg.cam_height, 0.0));
    player.yaw = cfg.cam_yaw.to_radians();
    player.pitch = cfg.cam_pitch.to_radians();
    start_player(cfg, &journal, &mut player);
    let cam = player.eye();
    let start = Instant::now();
    settle(&mut manager, &mut world, cam);
    println!(
        "  {} chunks in {:.1}s",
        world.chunks.len(),
        start.elapsed().as_secs_f32()
    );

    if cfg.cam_submerge != 0.0 && !cfg.resume {
        if let Some(p) = find_water(&world, cam, cfg.sea_level, cfg.cam_submerge) {
            println!(
                "  {} at {:.0} {:.0} {:.0}",
                if cfg.cam_submerge > 0.0 {
                    "submerged"
                } else {
                    "afloat"
                },
                p.x,
                p.y - math::Y_OFFSET as f32,
                p.z
            );
            player.pos = p - Vec3::new(0.0, voxelcraft::player::EYE, 0.0);
        }
    }

    if player.eye() != cam {
        settle(&mut manager, &mut world, player.eye());
        println!(
            "  replanned at the capture camera: {} chunks",
            world.chunks.len()
        );
    }
    (world, manager, player)
}

pub(crate) fn bench_frames(cfg: &Config, frames: usize) -> render::anyhow_lite::Result<()> {
    let instance = render::make_instance();
    let gpu = pollster::block_on(render::init_gpu(&instance, None))?;
    let mut renderer = Renderer::new(
        gpu,
        (cfg.width, cfg.height),
        wgpu::TextureFormat::Rgba8Unorm,
        cfg.water_mottle,
        cfg.tint_balance,
        cfg.probe_fill,
        cfg.probe_noise,
        cfg.lod,
        cfg.water_sec_scale.trailing_zeros(),
        cfg.tone_map_aces as u32,
        cfg.grade_value(),
        cfg.grade_strength,
    );

    let internal = (
        ((cfg.width as f32 * cfg.render_scale) as u32).max(8),
        ((cfg.height as f32 * cfg.render_scale) as u32).max(8),
    );
    renderer.resize(internal, (cfg.width, cfg.height));
    println!("gpu: {}", renderer.gpu.adapter.get_info().name);
    println!("generating world...");
    let journal = journal::open(cfg)?;
    let (mut world, mut manager, mut player) = headless_scene(cfg, journal.clone());
    if cfg.demo_edits || cfg.demo_glass || cfg.demo_lamps {
        let gen = Arc::new(cfg.worldgen());
        let cam = player.eye();
        if cfg.demo_lamps {
            build_lamp_chamber(&mut world, &gen, cam, player.look_dir());
        } else if cfg.demo_glass {
            build_glass_structure(&mut world, &gen, cam, player.look_dir());
        } else {
            build_demo_structure(&mut world, &gen, cam, player.look_dir());
        }
        let n = manager.rebuild_dirty_now(&mut world);
        println!("  coarse stand-ins rebuilt: {n}");
    }
    journal::save_if_asked(cfg, &journal, player.state(), player.bar)?;
    let cam = player.eye();
    renderer.sync_world(&mut world, cam);
    let (sun_dir, daylight) = sun(cfg.time_of_day);
    let aspect = cfg.width as f32 / cfg.height as f32;

    let mut shaft = ShaftField::with_texel(cfg.shaft_texel);
    for hiz in [false, true] {
        let mut wall = Vec::new();
        let mut per_pass = [0.0f32; 8];
        let mut samples = 0.0f32;
        let mut chunks = 0;
        for i in 0..frames {

            player.yaw = cfg.cam_yaw.to_radians() + i as f32 * 0.01;
            let cam = player.eye();
            let (fwd, right, up) = camera_basis(&player);
            let frustum = math::Frustum::from_camera(
                cam,
                fwd,
                right,
                up,
                (cfg.fov_degrees.to_radians() * 0.5).tan(),
                aspect,
                cfg.lod.view_distance,
            );
            renderer.build_chunk_list(&manager.render_set, &world, cam, &frustum);
            chunks = renderer.last_chunk_count;

            shaft.update(manager.generator(), cam.x, cam.z, cfg.canopy_lift);
            if shaft.dirty() {
                renderer.upload_shaft(shaft.heights());
            }
            let flags = flags_from(cfg, false, hiz) | underwater_flag(&world, cam);
            let params = FrameParams {
                cam_pos: cam,
                cam_fwd: fwd,
                cam_right: right,
                cam_up: up,
                fov_y: cfg.fov_degrees.to_radians(),
                far: cfg.lod.view_distance,
                sun_dir,
                daylight,

                time: cfg.anim_time,
                flags,
                spec_hi: spec_hi_from(cfg)
                    | if world.any_emitter_resident() {
                        render::FLAG_HI_EMITTER_WORLD
                    } else {
                        0
                    },
                ambient: cfg.ambient,
                shadow_dist: cfg.shadow_dist,
                fog: cfg.fog,
                clouds: cfg.clouds,
                sky: cfg.sky,
                godrays: cfg.godrays,
                sea_level: cfg.sea_level as f32,
                water_absorb: cfg.water_absorb,
                jitter: cfg.jitter.map(Vec2::from),
                taa: cfg.taa,
                mip_bias: cfg.mip_bias,
                tint: render::Tint {
                    strength: cfg.tint_strength,
                    seed: cfg.seed,
                },
                waves: waves_from(cfg),
                shaft_origin: shaft.origin_world(),
                shaft_texel: shaft.texel() as f32,
                shaft_scan: crate::scene::shaft_scan(cfg, flags, sun_dir),
            };
            let t0 = Instant::now();
            renderer.render(&params, None);
            renderer
                .gpu
                .device
                .poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: None,
                })
                .ok();
            let ms = (Instant::now() - t0).as_secs_f32() * 1000.0;

            if i >= 20 {
                wall.push(ms);
                let t = &renderer.timer;
                if t.valid {
                    for (k, &q) in [0u32, 2, 4, 6, 10, 8, 12, 14].iter().enumerate() {
                        per_pass[k] += t.ms(q);
                    }
                    samples += 1.0;
                }
            }
        }
        wall.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let med = wall[wall.len() / 2];
        let p99 = wall[((wall.len() as f32 * 0.99) as usize).min(wall.len() - 1)];
        let s = samples.max(1.0);
        println!();
        println!(
            "--- {}x{}  hi-z {}  taa {}  {} chunks ---",
            cfg.width,
            cfg.height,
            on(hiz),
            on(cfg.taa),
            chunks
        );
        println!(
            "  wall  median {med:.2}ms  ({:.0} fps)   p99 {p99:.2}ms",
            1000.0 / med
        );
        println!("  gpu   select {:.2}  march {:.2}  hiz {:.2}  recover {:.2}  march2 {:.2}  resolve {:.2}  taa {:.2}  shaft {:.2}",
            per_pass[0] / s, per_pass[1] / s, per_pass[2] / s, per_pass[3] / s, per_pass[4] / s, per_pass[5] / s, per_pass[6] / s, per_pass[7] / s);
        println!("  gpu   total {:.2}ms", per_pass.iter().sum::<f32>() / s);
        println!(
            "  pairs marched {}  deferred {}",
            renderer.timer.counters[3], renderer.timer.counters[1]
        );
        if let Some(d) = renderer.pair_demand() {
            println!(
                "  pair demand   select {} / {}  ({:.0}% of the buffer)",
                d.select,
                d.cap,
                100.0 * d.worst() as f32 / d.cap as f32
            );
        }
        renderer.warn_if_oversized();
    renderer.warn_if_truncated();
    }
    Ok(())
}

pub(crate) fn diff(
    a_path: &str,
    b_path: &str,
    grid: Option<(u32, u32)>,
    crop_frac: Option<[f32; 4]>,
) -> Result<(), String> {
    use voxelcraft::harness::metric::{self, Rect};
    use std::path::Path;
    let a = metric::Img::load(Path::new(a_path))?;
    let b = metric::Img::load(Path::new(b_path))?;
    if a.w != b.w || a.h != b.h {
        return Err(format!(
            "size mismatch: {} is {}x{}, {} is {}x{}",
            a_path, a.w, a.h, b_path, b.w, b.h
        ));
    }
    let full = Rect::full(a.w, a.h);
    let r = crop_frac
        .map(|q| {

            let f = |v: f32, m: u32| (v.clamp(0.0, 1.0) * m as f32).round() as u32;
            Rect {
                x0: f(q[0], a.w),
                y0: f(q[1], a.h),
                x1: f(q[2], a.w),
                y1: f(q[3], a.h),
            }
        })
        .unwrap_or(full);
    println!(
        "differing {} of {}  MAE {:.4}  max {}",
        metric::pixels_differing(&a, &b, r),
        r.pixels(),
        metric::mae(&a, &b, r),
        metric::max_delta(&a, &b, r),
    );
    match metric::diff_bbox(&a, &b) {
        Some(bx) => println!(
            "bbox {}x{} at ({}, {}) -- fractions {:.3},{:.3},{:.3},{:.3}",
            bx.x1 - bx.x0,
            bx.y1 - bx.y0,
            bx.x0,
            bx.y0,
            bx.x0 as f64 / a.w as f64,
            bx.y0 as f64 / a.h as f64,
            bx.x1 as f64 / a.w as f64,
            bx.y1 as f64 / a.h as f64,
        ),
        None => println!("bbox none"),
    }
    if let Some((gx, gy)) = grid {
        let mut cells: Vec<(u32, u32, Rect, f64, u64)> = Vec::new();
        for cy in 0..gy {
            for cx in 0..gx {

                let cell = Rect {
                    x0: r.x0 + (r.x1 - r.x0) * cx / gx,
                    y0: r.y0 + (r.y1 - r.y0) * cy / gy,
                    x1: r.x0 + (r.x1 - r.x0) * (cx + 1) / gx,
                    y1: r.y0 + (r.y1 - r.y0) * (cy + 1) / gy,
                };
                cells.push((
                    cx,
                    cy,
                    cell,
                    metric::mae(&a, &b, cell),
                    metric::pixels_differing(&a, &b, cell),
                ));
            }
        }
        let peak = cells
            .iter()
            .map(|(_, _, _, m, _)| *m)
            .fold(0.0f64, f64::max);
        for (cx, cy, cell, m, n) in &cells {
            if *n > 0 {
                println!(
                    "  cell {},{} ({}x{} @ {},{}) MAE {:.4} px {}{}",
                    cx,
                    cy,
                    cell.x1 - cell.x0,
                    cell.y1 - cell.y0,
                    cell.x0,
                    cell.y0,
                    m,
                    n,
                    if *m == peak && peak > 0.0 { "  <==" } else { "" },
                );
            }
        }
    }
    Ok(())
}
