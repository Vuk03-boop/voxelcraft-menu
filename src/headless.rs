//! The three modes that render without a window, and the reference that scores them.
//!
//! `--bench-terrain`, `--screenshot` and `--bench-frames`, plus `--reference`, which is a
//! `--screenshot` that converges. Split out of `main.rs` by batch 22b.
//!
//! **This is the file the measurement fixture drives**, so it is the one place in the tree
//! where a refactor cannot be checked by reading it: every number in `PERF.md` and every
//! bit-exactness claim in `CLAUDE.md` comes out of these functions. The relocation was
//! verified the only way it can be -- `harness bitexact` against a binary built before it,
//! 0 pixels at fourteen vantages.
//!
//! `shot_frame` stays shared between `screenshot` and `converge_reference` for the reason
//! its own comment gives: two copies of it, differing by a field nobody compared, is how a
//! reference stops being a reference for the build it is scoring.

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
    // Batch 57. **The bake's only price is here**, which is the point of it: it is worker
    // microseconds beside the flood's own, paid out of sixteen rayon threads that are idle once
    // the world is resident, and it never touches the frame. Roadmap R1 says to price it in
    // this column and not in milliseconds, and this is the column.
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

/// Drive the streamer until nothing is in flight and every selected chunk is resident.
///
/// **The camera it is given is the camera it plans for, and that is the whole point of the
/// second call each headless path makes.** `find_cave` and `find_water` cannot run until the
/// world around the spawn column exists, so the first settle has to happen at the spawn
/// camera -- and the `render_set` it leaves behind was chosen for an eye the capture then
/// moves away from. Nothing corrects it afterwards: `shot_frame` takes the manager by
/// reference and only reads the plan, so one stale plan serves every accumulated frame.
///
/// Measured at 1280x720 with the temporal pass off: planning at the spawn camera instead of
/// the capture camera is worth **52 pixels at `open-sea` and 56 at `shore`**, and nothing at
/// the other twelve vantages. It is not non-determinism -- four extra `update` calls at an
/// *unmoved* camera are bit-exact at all fourteen, which is the measurement that says
/// `is_settled` is a fixed point for everything a capture can see.
/// Batch 98's lookbook driver (`--lookbook DIR`): `lookbook::VIEWS` x `lookbook::COMBOS`,
/// each through `capture` -- the same pipeline the fixture's controls run, not a cheaper
/// one, because a tile drawn by anything else would carry doubt into exactly the verdict
/// pass the sheet exists for. Thirty-six draws repeated the settle thirty-six times; the
/// composition and the labels are the module's, GPU-free and pinned by batch98's tests.
pub(crate) fn lookbook(cfg: &Config, dir: &str) -> render::anyhow_lite::Result<()> {
    // headless.rs is a module of the *binary* crate (mod headless in main.rs), so the
    // library's name is the path here — `crate::` is the bin root and knows no lookbook.
    use voxelcraft::lookbook as lb;
    std::fs::create_dir_all(dir)?;
    let mut groups = Vec::new();
    for view in &lb::VIEWS {
        let mut tiles = Vec::new();
        for combo in &lb::COMBOS {
            let mut c = cfg.clone();
            // A misspelled view argument is the fixture's worst failure mode wearing a
            // friendly face: assert, verbally, rather than shoot the default camera.
            assert!(
                lb::apply_view(&mut c, view),
                "lookbook view `{}` uses arguments the parser no longer knows",
                view.name
            );
            (combo.apply)(&mut c);
            // The sheet labels its own tiles, so the HUD would only lie beside them.
            c.hud = false;
            // A lookbook is never a reference run -- `capture`'s None branch is the
            // reference one, and there is no verdict in comparing a sheet against itself.
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

// Batch 98: the capture core is now pixels-out, because the lookbook wants the same
// frames in a contact sheet rather than on disk. **`Some(pixels)` is the ordinary
// branch, `None` is the reference branch** (which writes its own files, as it always
// did, and the lookbook is never a reference run). The save itself stayed a wrapper's
// job, so `--screenshot` reads back bit-identical bytes over the same draw calls --
// the fixture's 21-vantage control rides this path, and the refactor is a move,
// not a change.
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

    // A vantage point above the terrain, so one frame exercises LOD, fog and shadows.
    let ground = spawn_position(&gen);
    let mut player = Player::new(ground + Vec3::new(0.0, cfg.cam_height, 0.0));
    player.yaw = cfg.cam_yaw.to_radians();
    player.pitch = cfg.cam_pitch.to_radians();
    start_player(cfg, &journal, &mut player);
    let cam = player.eye();

    // Stream everything in, with no frame budget, before capturing.
    println!("generating world...");
    let start = Instant::now();
    settle(&mut manager, &mut world, cam);
    println!(
        "  {} chunks in {:.1}s",
        world.chunks.len(),
        start.elapsed().as_secs_f32()
    );

    // The eye the world above streamed at. Both searches below may move the camera, and the
    // plan left behind belongs to this one until something settles again at the new one.
    let streamed_from = cam;

    // A negative camera height means "show me a cave": drop into a real air pocket
    // rather than ending up embedded in solid stone.
    // Both searches are off under `--resume`: the journal's eye is the camera, and a search
    // that moved it would be a capture of somewhere else that still printed the resumed
    // position. `--cam-height -N` and `--cam-submerge` are the only two ways the camera moves
    // after the settle, so these two conditions are the whole of it.
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

    // `--cam-submerge` is the same idea for the other medium: drop the eye under the
    // surface of the nearest water, because a capture is the only way to A/B it.
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

    // Plan at the camera that renders. Guarded on the camera having actually moved so the
    // twelve vantages that never move one are untouched *by construction* rather than by
    // measurement -- `--cam-submerge` and a `--cam-height -N` cave are the only two ways in.
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
        // The coarse stand-ins over the structure were interned during the settle above and
        // show a world without it, because `journal::apply` reads the journal once, at build
        // time. A window would let `manager.update` replace them over the next few frames;
        // a capture is one frame, so it does the same work here and now. Deliberately *not*
        // another settle, which would spend its rebuild budget a few chunks per frame over
        // frames this path does not have -- `rebuild_dirty_now` carries the rest of that
        // argument, including the half batch 31 had to take back.
        let n = manager.rebuild_dirty_now(&mut world);
        println!("  coarse stand-ins rebuilt: {n}");
    }
    // After the edits and before the capture, so `--demo-edits --save-edits f` writes exactly
    // the structure this frame shows -- which is how the round-trip A/B gets its journal
    // without a line of code that exists only for the test.
    journal::save_if_asked(cfg, &journal, player.state(), player.bar)?;

    renderer.sync_world(&mut world, cam);
    let (sun_dir, daylight) = sun(cfg.time_of_day);
    // Present into an offscreen target so the blit and overlay run exactly as they do
    // on screen. Hi-Z reads the previous frame, so warm it up before capturing.
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
    // `--scale` was read only by the windowed F4 handler until batch 22, so a headless capture
    // ignored it in silence -- batch 19 took three renders at `--scale 3` that came back
    // identical to three decimal places. Going through `resize` rather than setting
    // `surface_size` by hand is what makes it work: the renderer derives its own mip bias from
    // the ratio of the two sizes, so a supersampled capture picks the mips the native one
    // picks, and `--mip-bias log2(factor)` is no longer something the caller has to remember.
    let internal = (
        ((cfg.width as f32 * cfg.render_scale) as u32).max(8),
        ((cfg.height as f32 * cfg.render_scale) as u32).max(8),
    );
    renderer.resize(internal, (cfg.width, cfg.height));
    // Hi-Z consumes the previous frame's depth, so it wants a few frames of warmup either
    // way; the temporal pass wants however many the caller asked it to converge over. The
    // camera never moves and the jitter cycle is fixed, so the result is still one
    // deterministic image -- which is the only reason an accumulating resolve is allowed
    // anywhere near the A/B captures the rest of these docs are built on.
    let frames = if cfg.taa { cfg.taa_frames.max(5) } else { 5 };
    // One per capture path, and it fills on the first frame it is asked for: the vantages
    // never move, so every later frame's `update` is a compare and a return.
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
            // Sweep in to the configured angle, so the captured frame has as much real motion
            // behind it as `--cam-spin` asks for. At the default of zero the camera never moves
            // and every frame reprojects onto itself.
            player.yaw = end_yaw - cfg.cam_spin.to_radians() * (frames - 1 - i) as f32;
            player.pos = end_pos - player.look_dir() * (cfg.cam_dolly * (frames - 1 - i) as f32);
            // And sweep the wave and cloud phase in the same way, for the same reason and on
            // the other half of the problem: `cam_dolly` gives the temporal pass a moving
            // *camera*, which disoccludes at every silhouette, where this gives it a moving
            // *field* under a camera that is standing still. Nothing disoccludes, every pixel
            // reprojects onto itself, and so whatever ghosts is the sea's or the deck's own
            // history rather than a reprojection failure. Batches 10 and 16 both had to leave
            // that unmeasured because `frame.time` was pinned to zero here.
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
        // One line per quantity and every one an integer, so two runs diff cleanly. The
        // per-pixel mean is over pixels a marcher *reached*, not over the frame: a sky pixel
        // that no pair covers is not a cheap march, it is no march, and averaging it in would
        // make a frame look cheaper for having more sky in it.
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

/// One frame at a pinned sub-pixel offset, into `view`.
///
/// Factored out of the capture loop so that the reference mode below renders through exactly
/// the same code as an ordinary `--screenshot`. Two copies of this, differing by a field
/// nobody compared, is how a reference stops being a reference for the build it is scoring.
#[allow(clippy::too_many_arguments)]
fn shot_frame(
    renderer: &mut Renderer,
    world: &World,
    manager: &ChunkManager,
    player: &Player,
    cfg: &Config,
    sun_dir: Vec3,
    daylight: f32,
    // `frame.time` for this frame, in seconds. Swept by the capture loop so the temporal
    // pass sees a field that is actually moving; held constant by the reference mode,
    // which converges a still.
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
    // The capture camera does not move, so this fills on the first accumulated frame and
    // early-outs on the other thirty-one.
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

/// Converge a ground-truth reference over an NxN sub-pixel grid and report its noise floor.
///
/// The recipe is batch 19's, with batch 21's correction to it: N x N captures at **native**
/// resolution, one per grid cell, averaged in linear. Native rather than supersampled because
/// the mip footprint divides by `res.y`, so a bigger render is a different signal and not a
/// better sample of the same one -- and because above 5120x2880 it silently loses geometry.
///
/// The halves are not optional and are why this is a mode rather than a shell loop. Batch 21's
/// first reference was 64 samples, ranked a sweep cleanly, and had a half-split floor of 1.434
/// against a signal of 1.515 -- 95% noise, and worse than useless, because an under-converged
/// reference aliases *in sympathy* with an unfiltered build and scores the bug as accuracy.
/// Nothing here is allowed to hand back a reference without the number that says whether it
/// can be believed.
///
/// **How the halves are split changes the floor by about 3x, and neither obvious rule is
/// right.** A checkerboard, `(i + j) % 2`, makes each half a quincunx -- itself a well
/// stratified sampling of the pixel -- so the two halves agree far better than two independent
/// 128-sample sets would, and the floor comes out optimistic: 0.197 where batch 21's 256-sample
/// reference reported 0.549. Splitting on the sample index is worse in the other direction,
/// since one half then holds every offset from the top of the grid and the other every offset
/// from the bottom, and their difference charges the reference for a systematic bias the full
/// mean does not have. So the split is a fixed hash of the grid cell: deterministic, so the
/// floor is reproducible, and uncorrelated with sub-pixel position, so it estimates sampling
/// error and not grid geometry.
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

    // Which half each grid cell falls in: rank the cells by a fixed integer hash and cut the
    // ranking in two. Deterministic, so the floor is reproducible; uncorrelated with sub-pixel
    // position, so it estimates sampling error rather than grid geometry; and exactly balanced,
    // which taking the hash's low bit is not -- that gave 39 against 25 at `--reference 8`, and
    // a lopsided split reports the noise of its smaller half.
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

    // Hi-Z consumes the previous frame's depth, so it wants warming -- once for the whole run
    // and not once per sample, since the camera never moves between them.
    //
    // **Four and not five, and that is an invariant rather than a tuning.** `frame_index`
    // walks the LOD dither and the god-ray sampler's stratification offset one step per frame,
    // so a headless capture's pixels depend on how many frames preceded it. An ordinary
    // `--no-taa --screenshot` renders five and captures the one at index 4; warming four here
    // puts the first sample at index 4 as well, which makes `--reference 1` **bit-identical**
    // to `--no-taa --jitter 0,0`. That equality is the self-test that says this path is the
    // same renderer and not a second one that resembles it. At five it differed on 3,671
    // pixels of 921,600 -- all of them in the shaft and the LOD band, none in water, which is
    // exactly the fingerprint of a dither phase and was how the cause was found.
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
            // Centres of the grid cells over [-0.5, 0.5), which is the range `halton_jitter`
            // walks and the range `ray_dir`'s `+ 0.5` puts at the pixel centre.
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
                // Alpha is linear already; it is 255 everywhere, so this only matters for
                // being right rather than for the pixels.
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
        // One sample cannot be split, and a floor computed against a black half would be a
        // large confident number meaning nothing -- which is the exact failure mode this
        // whole mode exists to prevent.
        println!("  noise floor: not available below 2 samples");
    } else {
        let floor = metric::mae(&a, &b, Rect::full(w, h));
        println!("  noise floor (half-split MAE, whole frame): {floor:.4}");
        println!("  an effect smaller than the floor is not measurable against this reference.");
    }
    Ok(())
}

/// Build a world around a fixed vantage point and hand back everything needed to render.
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
    // Off under `--resume`, for the reason the capture path's copy gives.
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
    // Same re-plan the capture path makes, for the same reason: a bench that prices the
    // frame from a submerged eye against a plan built for the eye above it is pricing a
    // frame nothing draws. The bench rotates `yaw` per frame and never moves the position,
    // so this is the only place the position can change under the plan.
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
    // Same `--scale` fix as the capture path: the bench measured the window size and ignored
    // the flag that says what is actually traced.
    let internal = (
        ((cfg.width as f32 * cfg.render_scale) as u32).max(8),
        ((cfg.height as f32 * cfg.render_scale) as u32).max(8),
    );
    renderer.resize(internal, (cfg.width, cfg.height));
    println!("gpu: {}", renderer.gpu.adapter.get_info().name);
    println!("generating world...");
    let journal = journal::open(cfg)?;
    let (mut world, manager, mut player) = headless_scene(cfg, journal.clone());
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
            // Rotate slowly so the measurement covers many view directions.
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
            // The bench yaws in place, so this fills once per process and then early-outs --
            // which is deliberate, because a bench that refilled the window every frame would
            // be pricing a CPU path the game only pays when the player walks.
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
                // A bench is one configuration measured many times, which is why it has never
                // swept `cam_spin` either. `--anim-time` is how it picks *which* moment of the
                // sea and the deck it is pricing; `--anim-rate` is not read here.
                time: cfg.anim_time,
                flags,
                spec_hi: spec_hi_from(cfg)
                    | if world.any_emitter_resident() {
                        render::FLAG_HI_EMITTER_WORLD
                    } else {
                        0
                    },
                ambient: cfg.ambient,
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
            // Skip warmup frames: pipelines compile and the readback ring fills.
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

/// Compare two already-captured frames. **This mode renders nothing** -- the sweep that
/// uses it (`docs/roadmap.md` D2b) captures against every flag combination it wants, and a
/// differ that could accidentally render would be a second instrument pretending to be
/// the first. What it reports is biased toward finding: the whole-frame numbers for
/// continuity with every table that already quotes them, then the bbox of the disagreement
/// and, when `--grid` is passed, a per-cell MAE table so that a spike in one band of the
/// frame says which cells to name as crops. Cell rows are printed only where they differ,
/// zero elsewhere; the peak cell gets a marker so a fat report still reads at a glance.
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
            // Fractions, the convention `harness::vantage::Crop` declares, so the rect a
            // spike needs can be pasted into that table unconverted.
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
                // Cell rect: the split integer-divides, so the last cell in a row or
                // column lands the remainder rather than leaving a strip unviewed.
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

