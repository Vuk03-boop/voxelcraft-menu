use std::sync::Arc;
use std::time::{Duration, Instant};

use glam::Vec2;
use voxelcraft::config::Config;
use voxelcraft::menu::{self, Action, Menu, Screen};
use voxelcraft::settings::{self, Preferences};
use std::path::PathBuf;
use voxelcraft::idle::{self, FrameKind};
use voxelcraft::journal::{self, EditJournal};
use voxelcraft::player::{Input, Player};
use voxelcraft::render::{self, FrameParams, Renderer};
use voxelcraft::shaft::ShaftField;
use voxelcraft::stats::{fmt_bytes, Stats};
use voxelcraft::stream::ChunkManager;
use voxelcraft::voxel::{ChunkKey, World};
use voxelcraft::{block, lod, math};
use winit::application::ApplicationHandler;
use winit::event::{
    DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

use crate::scene;
use crate::scene::{
    camera_basis, draw_hud, flags_from, on, spawn_position, spec_hi_from, sun, underwater_flag,
    waves_from,
};

struct Surface {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
}

struct App {
    cfg: Config,
    prefs: Preferences,
    preference_path: PathBuf,
    menu: Menu,
    menu_gpu: Option<render::Gpu>,
    menu_ui: Option<render::ui::UiRenderer>,
    pointer: [f32; 2],
    focused: bool,
    minimized: bool,
    next_menu_redraw: Instant,
    loading_shown: bool,
    session_started: bool,
    saved_on_exit: bool,

    _instance: Option<wgpu::Instance>,
    window: Option<Arc<Window>>,
    surface: Option<Surface>,
    renderer: Option<Renderer>,
    world: World,
    manager: ChunkManager,
    player: Player,
    input: Input,
    stats: Stats,
    relight_queue: rustc_hash::FxHashSet<ChunkKey>,
    grabbed: bool,
    show_stats: bool,
    heatmap: bool,
    hiz: bool,
    time_of_day: f32,

    shaft: ShaftField,
    animation_time: f32,

    idle: idle::IdleRepaint,

    idle_animated: bool,

    rendered_once: bool,

    repainted_last: bool,
}

impl App {
    fn new(cfg: Config, journal: Arc<EditJournal>, preference_path: PathBuf, status: String) -> Self {
        let gen = Arc::new(cfg.worldgen());
        let mut player = Player::new(spawn_position(&gen));
        player.hotbar = cfg.hotbar;

        if let Some(s) = journal.player() {
            player.restore(s);
        }

        player.bar = voxelcraft::journal::bar_for(&journal);
        let mut manager = ChunkManager::new(cfg.lod, gen.clone());

        manager.attach_journal(journal.clone());
        let mut world = World::new();
        world.attach_journal(journal);
        let idle_animated = cfg.wave_speed != 0.0 || cfg.clouds.speed != 0.0 || cfg.wind_sway;

        let shaft_texel = cfg.shaft_texel;
        let prefs = Preferences::from_config(&cfg);
        let mut menu = Menu::new(prefs.clone());
        menu.status = status;
        menu.save_hint = if cfg.journal_out().is_some() { "World saving enabled for this session".into() }
            else { "World not saved: launch with --edits PATH to keep your world".into() };
        Self {
            prefs, preference_path, menu, menu_gpu: None, menu_ui: None,
            pointer: [0.0; 2], focused: true, minimized: false,
            next_menu_redraw: Instant::now(), loading_shown: false,
            session_started: false, saved_on_exit: false,
            time_of_day: cfg.time_of_day,
            cfg,
            _instance: None,
            window: None,
            surface: None,
            renderer: None,
            world,
            manager,
            player,
            input: Input::default(),
            stats: Stats::new(),
            relight_queue: Default::default(),
            grabbed: false,
            show_stats: true,
            heatmap: false,
            hiz: true,
            shaft: ShaftField::with_texel(shaft_texel),
            animation_time: 0.0,
            idle: idle::IdleRepaint::new(),
            idle_animated,
            rendered_once: false,
            repainted_last: false,
        }
    }

    fn internal_size(&self) -> (u32, u32) {
        let s = self.prefs.scale;
        let (w, h) = match &self.surface {
            Some(s) => (s.config.width, s.config.height),
            None => (self.cfg.width, self.cfg.height),
        };
        (
            ((w as f32 * s) as u32).max(8),
            ((h as f32 * s) as u32).max(8),
        )
    }

    fn set_grab(&mut self, grab: bool) {
        let Some(window) = &self.window else { return };
        if grab {
            let ok = window
                .set_cursor_grab(CursorGrabMode::Confined)
                .or_else(|_| window.set_cursor_grab(CursorGrabMode::Locked))
                .is_ok();
            if ok {
                window.set_cursor_visible(false);
                self.grabbed = true;
            }
        } else {
            let _ = window.set_cursor_grab(CursorGrabMode::None);
            window.set_cursor_visible(true);
            self.grabbed = false;
        }
    }

    fn reconfigure(&mut self, size: (u32, u32)) {
        let Some(s) = &mut self.surface else { return };
        if size.0 == 0 || size.1 == 0 {
            return;
        }
        s.config.width = size.0;
        s.config.height = size.1;
        if let Some(r) = &self.renderer {
            s.surface.configure(&r.gpu.device, &s.config);
        } else if let Some(gpu) = &self.menu_gpu {
            s.surface.configure(&gpu.device, &s.config);
        }
        let internal = self.internal_size();
        if let Some(r) = &mut self.renderer {
            r.resize(internal, size);
            self.rendered_once = false;
        }

        self.idle.invalidate();
    }

    fn handle_edits(&mut self) {
        if self.input.lmb {
            if let Some(pick) = self.player.pick(&self.world) {
                if self.world.set_block(pick.block, block::AIR) {
                    self.relight_queue.insert(ChunkKey::of_block(pick.block));
                }
            }
        }
        if self.input.rmb {
            if let Some(pick) = self.player.pick(&self.world) {
                let target = pick.adjacent;
                if !self.player.intersects_block(target)
                    && self.world.get_block(target) == block::AIR
                    && self.world.set_block(target, self.player.selected_block())
                {
                    self.relight_queue.insert(ChunkKey::of_block(target));
                }
            }
        }

        if self.input.mmb {
            if let Some(pick) = self.player.pick(&self.world) {
                let id = self.world.get_block(pick.block);
                self.player.pick_into_bar(id);
            }
        }

        if let Some(&key) = self.relight_queue.iter().next() {
            self.relight_queue.remove(&key);
            let gen = self.manager.generator().clone();
            voxelcraft::stream::relight(&mut self.world, &gen, key);
        }
    }

    fn draw_overlay(&mut self) {
        let (sw, sh) = match &self.surface {
            Some(s) => (s.config.width as f32, s.config.height as f32),
            None => return,
        };
        let Some(r) = &mut self.renderer else { return };
        let ui = &mut r.ui;

        draw_hud(ui, sw, sh, self.player.hotbar, &self.player.bar);

        if !self.show_stats {
            return;
        }
        let s = &self.stats;
        let t = &r.timer;
        let p = self.player.pos;
        let mut lines = vec![
            format!("FPS {:.0}   frame {:.2}ms   cpu {:.2}ms (max {:.2}){}", s.fps(), s.frame_ms(), s.cpu_ms(), s.cpu_ms_max(),
                if self.repainted_last { "   idle repaint" } else { "" }),
            format!("GPU {:.2}ms  select {:.2}  march {:.2}  hiz {:.2}  recover {:.2}  resolve {:.2}  taa {:.2}  shaft {:.2}{}",
                t.total_ms(), t.ms(0), t.ms(2), t.ms(4), t.ms(6), t.ms(8), t.ms(12), t.ms(14),
                if self.repainted_last { "   (last full render)" } else { "" }),
            format!("pos {:.1} {:.1} {:.1}   yaw {:.0}   {}", p.x, p.y - math::Y_OFFSET as f32, p.z, self.player.yaw.to_degrees(), if self.player.fly { "fly" } else { "walk" }),
            format!("chunks drawn {} / loaded {} / pending {}", s.chunks_drawn, s.chunks_loaded, s.chunks_in_flight),
            format!("pairs marched {} deferred {}", s.pairs_marched, s.pairs_deferred),
            format!("geometry {} ({:.3} b/voxel)   attr+light {} ({:.3} b/voxel)",
                fmt_bytes(s.geometry_bytes as u64), s.bytes_per_voxel(), fmt_bytes(s.attribute_bytes as u64), s.attr_bytes_per_voxel()),
            format!("dedup {:.2}x   solid voxels {}   vram {}", s.dedup_ratio, s.solid_voxels, fmt_bytes(s.vram_bytes)),
            format!("lod {:?}   scale {:.2}   hiz {}   taa {}   heat {}", s.lod_histogram, self.prefs.scale, on(self.hiz), on(self.prefs.taa), on(self.heatmap)),
        ];
        if self.prefs.isolate_glass || self.prefs.shadow_pass {
            lines.push(format!("resolve = full pass family   glass split {}   shadow pass {}",
                on(self.prefs.isolate_glass || self.prefs.shadow_pass), on(self.prefs.shadow_pass)));
        }
        lines.push(format!("shadow ray {:.2}ms  filter {:.2}ms (inside resolve family)",t.ms(16),t.ms(18)+t.ms(20)));
        if !self.grabbed {
            lines.push("click to capture mouse   esc to pause".into());
        }
        let scale = 2.0;
        let lh = voxelcraft::render::font::CELL_H as f32 * scale + 4.0;
        r.ui.rect(
            6.0,
            6.0,
            760.0,
            lh * lines.len() as f32 + 8.0,
            [0.0, 0.0, 0.0, 0.45],
        );
        for (i, line) in lines.iter().enumerate() {
            r.ui.text_shadow(
                12.0,
                10.0 + i as f32 * lh,
                scale,
                line,
                [0.92, 0.95, 1.0, 1.0],
            );
        }
    }
}

impl App {
    fn window_size(&self) -> (u32, u32) {
        self.surface.as_ref().map(|s| (s.config.width, s.config.height))
            .unwrap_or((self.cfg.width, self.cfg.height))
    }

    fn pause(&mut self) {
        self.menu.pause();
        self.input = Input::default();
        self.set_grab(false);
        self.stats.reset_clock();
    }

    fn resume(&mut self) {
        self.input = Input::default();
        self.stats.reset_clock();
        self.idle.invalidate();
        self.menu.screen = Screen::Playing;
        self.set_grab(true);
        if !self.grabbed {
            self.menu.pause();
            self.menu.status = "Mouse capture unavailable. Try Resume again.".into();
        }
    }

    fn start_session(&mut self) {
        let Some(gpu) = self.menu_gpu.take() else { return; };
        let size = self.window_size();
        let Some(surface) = &self.surface else { self.menu_gpu = Some(gpu); return; };
        let format = surface.config.format;
        let mut renderer = Renderer::new(
            gpu, self.internal_size(), format,
            self.cfg.water_mottle, self.cfg.tint_balance,
            self.cfg.probe_fill, self.cfg.probe_noise, self.cfg.lod,
            self.cfg.water_sec_scale.trailing_zeros(),
            self.cfg.tone_map_aces as u32, self.cfg.grade_value(), self.cfg.grade_strength,
        );
        renderer.surface_size = size;
        println!("gpu: {}", renderer.gpu.adapter.get_info().name);
        self.renderer = Some(renderer);
        self.menu_ui = None;
        self.session_started = true;
        self.menu.experiments_locked = true;
        self.resume();
    }

    fn apply_preferences(&mut self, next: Preferences, persist: bool) {
        if let Err(e) = next.validate() { self.menu.status = e; return; }
        let plan = self.prefs.plan(&next);

        if self.session_started && plan.experiments {
            self.menu.status = "Experimental switches require a restart. Change them on the title.".into();
            return;
        }
        let size = self.window_size();
        let internal = (((size.0 as f32 * next.scale) as u32).max(8),
                        ((size.1 as f32 * next.scale) as u32).max(8));

        let gpu = self.renderer.as_ref().map(|r| &r.gpu).or(self.menu_gpu.as_ref());
        if let Some(gpu) = gpu {
            let limits = gpu.device.limits();
            if internal.0 > limits.max_texture_dimension_2d || internal.1 > limits.max_texture_dimension_2d
                || u64::from(internal.0) * u64::from(internal.1) * 8 > limits.max_storage_buffer_binding_size {
                self.menu.status = "Requested resolution exceeds this GPU's limits.".into(); return;
            }
        }
        if let Some(r) = &mut self.renderer {
            if plan.resize { r.resize(internal, size); self.rendered_once = false; }
            if plan.presentation { r.set_presentation(next.aces as u32, next.grade.index(), next.grade_strength); }
            if plan.history { r.invalidate_history(); }
        }
        if plan.surface {
            if let Some(surface) = &mut self.surface {
                surface.config.present_mode = if next.vsync { wgpu::PresentMode::AutoVsync } else { wgpu::PresentMode::AutoNoVsync };
                let gpu = self.renderer.as_ref().map(|r| &r.gpu).or(self.menu_gpu.as_ref());
                if let Some(gpu) = gpu { surface.surface.configure(&gpu.device, &surface.config); }
            }
        }
        next.write_config(&mut self.cfg);
        self.prefs = next;
        self.menu.draft = self.prefs.clone();
        self.idle.invalidate();
        self.idle_animated = self.cfg.wave_speed != 0.0 || self.cfg.clouds.speed != 0.0 || self.cfg.wind_sway;
        self.menu.status = if self.session_started { "Applied. Resume to see scene changes.".into() }
            else { "Applied. Ready to play.".into() };
        if persist {
            match settings::save(&self.preference_path, &self.prefs) {
                Ok(()) => self.menu.status.push_str(" Preferences saved."),
                Err(e) => {
                    eprintln!("settings save: {e}");
                    self.menu.status = format!("Applied this session, NOT saved: {e}");
                }
            }
        }
    }

    fn request_quit(&mut self, event_loop: &ActiveEventLoop, discard: bool) {
        self.input = Input::default();
        self.set_grab(false);
        if self.session_started && !discard {
            if self.cfg.journal_out().is_none() {
                if self.menu.screen == Screen::Playing { self.menu.pause(); }
                self.menu.status.clear();
                self.menu.confirm_quit();
                return;
            }
            if let Err(e) = journal::save_if_asked(&self.cfg, &self.world.journal, self.player.state(), self.player.bar) {
                eprintln!("world save: {e}");
                if self.menu.screen == Screen::Playing { self.menu.pause(); }
                self.menu.status = format!("World NOT saved: {e}");
                self.menu.confirm_quit();
                return;
            }
        }
        self.saved_on_exit = true;
        event_loop.exit();
    }

    fn menu_action(&mut self, event_loop: &ActiveEventLoop, action: Action) {
        match action {
            Action::Play => { self.menu.screen = Screen::Loading; self.loading_shown = false; self.menu.status.clear(); }
            Action::Resume => self.resume(),
            Action::Settings => self.menu.open_settings(&self.prefs),
            Action::Quit => self.request_quit(event_loop, false),
            Action::QuitAnyway => self.request_quit(event_loop, true),
            Action::Back => self.menu.dismiss_quit(),
            Action::Cancel => self.menu.cancel(),
            Action::Tab(page) => { self.menu.page = page; self.menu.focus = page as usize; }
            Action::Adjust(setting, direction) => self.menu.adjust(setting, direction),
            Action::Apply => self.apply_preferences(self.menu.draft.clone(), true),
            Action::Defaults => self.menu.restore_defaults(),
        }
    }

    fn menu_frame(&mut self) {
        let size = self.window_size();
        let Some(surface) = &self.surface else { return; };
        let frame = match surface.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => { self.reconfigure(size); return; }
            _ => return,
        };
        let view = frame.texture.create_view(&Default::default());
        if let Some(r) = &mut self.renderer {
            self.menu.draw(&mut r.ui, size, self.rendered_once);
            if self.rendered_once { r.repaint(&view); }
            else { menu::present_ui(&r.gpu, &mut r.ui, &view, size); }
            r.gpu.queue.present(frame);
        } else if let (Some(gpu), Some(ui)) = (&self.menu_gpu, &mut self.menu_ui) {
            self.menu.draw(ui, size, false);
            menu::present_ui(gpu, ui, &view, size);
            gpu.queue.present(frame);
        }
        if self.menu.screen == Screen::Loading { self.loading_shown = true; }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("voxelcraft")
            .with_inner_size(winit::dpi::PhysicalSize::new(
                self.cfg.width,
                self.cfg.height,
            ));
        let window = Arc::new(event_loop.create_window(attrs).expect("window"));

        let instance = render::make_instance();
        let surface = instance.create_surface(window.clone()).expect("surface");

        let gpu = match pollster::block_on(render::init_gpu(&instance, Some(&surface))) {
            Ok(gpu) => gpu,
            Err(e) => {
                eprintln!("{e}");
                event_loop.exit();
                return;
            }
        };
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1))
            .expect("surface not supported by adapter");
        config.present_mode = if self.cfg.vsync {
            wgpu::PresentMode::AutoVsync
        } else {
            wgpu::PresentMode::AutoNoVsync
        };
        config.usage |= wgpu::TextureUsages::RENDER_ATTACHMENT;
        surface.configure(&gpu.device, &config);

        let format = config.format;
        self.cfg.width = config.width;
        self.cfg.height = config.height;
        self._instance = Some(instance);
        self.surface = Some(Surface { surface, config });

        self.menu_ui = Some(render::ui::UiRenderer::new(&gpu.device, &gpu.queue, format));
        self.menu_gpu = Some(gpu);
        self.window = Some(window);
    }

    fn device_event(&mut self, _e: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            if self.grabbed && self.menu.screen == Screen::Playing {
                self.input.mouse_dx += delta.0 as f32;
                self.input.mouse_dy += delta.1 as f32;
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {

        match &event {
            WindowEvent::CloseRequested => { self.request_quit(event_loop, false); return; }
            WindowEvent::Focused(focus) => {
                self.focused = *focus;
                self.input = Input::default();
                if !*focus && self.menu.screen == Screen::Playing { self.pause(); }
                if *focus { self.stats.reset_clock(); }
                return;
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = [position.x as f32, position.y as f32];
                let size = self.window_size();
                if self.menu.screen != Screen::Playing { self.menu.hover(self.pointer, size); }
            }
            WindowEvent::Resized(size) => {
                self.minimized = size.width == 0 || size.height == 0;
                if self.minimized && self.menu.screen == Screen::Playing { self.pause(); }
                self.reconfigure((size.width, size.height));
                return;
            }
            _ => {}
        }
        if self.menu.screen != Screen::Playing {
            let action = match &event {
                WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed && !event.repeat => {
                    if let PhysicalKey::Code(key) = event.physical_key { self.menu.key(key) } else { None }
                }
                WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                    self.menu.click(self.pointer, self.window_size())
                }
                _ => None,
            };
            if let Some(action) = action { self.menu_action(event_loop, action); }
            let redraw = matches!(&event, WindowEvent::RedrawRequested);
            if redraw { self.frame(); }
            if !redraw { if let Some(w) = &self.window { w.request_redraw(); } }
            return;
        }
        match event {
            WindowEvent::MouseWheel { delta, .. } => {
                self.input.scroll += match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let down = state == ElementState::Pressed;
                if !self.grabbed {
                    if down {
                        self.set_grab(true);
                    }
                    return;
                }
                match button {
                    MouseButton::Left => {
                        self.input.lmb = down && !self.input.lmb_down;
                        self.input.lmb_down = down;
                    }
                    MouseButton::Right => {
                        self.input.rmb = down && !self.input.rmb_down;
                        self.input.rmb_down = down;
                    }
                    MouseButton::Middle => {
                        self.input.mmb = down && !self.input.mmb_down;
                        self.input.mmb_down = down;
                    }
                    _ => {}
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };
                if event.state == ElementState::Pressed {
                    if !event.repeat {
                        self.input.pressed.insert(code);
                    }
                    self.input.down.insert(code);
                    match code {
                        KeyCode::Escape => self.pause(),
                        KeyCode::F3 => self.show_stats = !self.show_stats,
                        KeyCode::F4 => {
                            let mut p = self.prefs.clone();
                            p.scale = [1.0, 0.75, 0.6, 0.5].into_iter()
                                .find(|s| *s < p.scale - 0.001).unwrap_or(1.0);
                            self.apply_preferences(p, false);
                        }
                        KeyCode::F5 => self.heatmap = !self.heatmap,
                        KeyCode::F6 => self.hiz = !self.hiz,
                        KeyCode::F7 => {
                            let mut p = self.prefs.clone(); p.taa = !p.taa;
                            self.apply_preferences(p, false);
                        }
                        _ => {}
                    }
                } else {
                    self.input.down.remove(&code);
                }
            }
            WindowEvent::RedrawRequested => self.frame(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.minimized || !self.focused {
            event_loop.set_control_flow(ControlFlow::Wait);
        } else if self.menu.screen == Screen::Playing {
            event_loop.set_control_flow(ControlFlow::Poll);
            if let Some(w) = &self.window { w.request_redraw(); }
        } else {
            let now = Instant::now();
            if now >= self.next_menu_redraw {
                if let Some(w) = &self.window { w.request_redraw(); }
                self.next_menu_redraw = now + Duration::from_millis(33);
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_menu_redraw));
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if !self.session_started || self.saved_on_exit { return; }
        if let Err(e) =
            journal::save_if_asked(
            &self.cfg,
            &self.world.journal,
            self.player.state(),
            self.player.bar,
        )
        {
            eprintln!("{e}");
        }
    }
}

impl App {
    fn frame(&mut self) {
        if self.minimized || !self.focused { return; }
        if self.menu.screen != Screen::Playing {
            if self.menu.screen == Screen::Loading && self.loading_shown { self.start_session(); }
            if self.menu.screen != Screen::Playing { self.menu_frame(); return; }
        }
        let dt = self.stats.tick();
        self.animation_time += dt;
        let cpu_start = Instant::now();

        if !self.cfg.freeze_time {
            self.time_of_day = (self.time_of_day + dt / self.cfg.day_length_seconds).fract();
        }
        self.player
            .update(dt, &self.input, &self.world, self.cfg.physics);
        self.handle_edits();
        self.input.end_frame();

        let cam = self.player.eye();
        self.manager.update(
            cam,
            &mut self.world,
            Duration::from_secs_f32(self.cfg.stream_budget_ms / 1000.0),
        );

        let (fwd, right, up) = camera_basis(&self.player);
        let (sw, sh) = match &self.surface {
            Some(s) => (s.config.width, s.config.height),
            None => return,
        };
        let aspect = sw as f32 / sh.max(1) as f32;
        let frustum = math::Frustum::from_camera(
            cam,
            fwd,
            right,
            up,
            (self.cfg.fov_degrees.to_radians() * 0.5).tan(),
            aspect,
            self.cfg.lod.view_distance,
        );

        let probes_pending = self
            .world
            .probe_dirty
            .keys()
            .any(|&k| voxelcraft::probe::uploadable(k.origin(), cam));
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        renderer.sync_world(&mut self.world, cam);
        renderer.build_chunk_list(&self.manager.render_set, &self.world, cam, &frustum);

        self.stats.chunks_drawn = renderer.last_chunk_count;
        self.stats.chunks_loaded = self.world.chunks.len();
        self.stats.chunks_in_flight = self.manager.pending();
        self.stats.lod_histogram = lod::histogram(&self.manager.render_set, self.cfg.lod.max_lod);
        self.stats.geometry_bytes = self.world.geometry_bytes();
        self.stats.attribute_bytes = self.world.bricks.bytes();
        self.stats.solid_voxels = self.world.total_solid_voxels();
        self.stats.dedup_ratio = self.world.leaves.dedup_ratio();
        self.stats.vram_bytes = renderer.vram_estimate();
        self.stats.pairs_marched = renderer.timer.counters[3];
        self.stats.pairs_deferred = renderer.timer.counters[1];

        self.draw_overlay();

        let (sun_dir, daylight) = sun(self.time_of_day);

        if let Some(r) = &self.renderer {
            self.shaft
                .update(self.manager.generator(), cam.x, cam.z, self.cfg.canopy_lift);
            if self.shaft.dirty() {
                r.upload_shaft(self.shaft.heights());
            }
        }
        let flags =
            flags_from(&self.cfg, self.heatmap, self.hiz) | underwater_flag(&self.world, cam);
        let params = FrameParams {
            cam_pos: cam,
            cam_fwd: fwd,
            cam_right: right,
            cam_up: up,
            fov_y: self.cfg.fov_degrees.to_radians(),
            far: self.cfg.lod.view_distance,
            sun_dir,
            daylight,
            time: self.animation_time,
            flags,
            spec_hi: spec_hi_from(&self.cfg)

                | if self.world.any_emitter_resident() {
                    render::FLAG_HI_EMITTER_WORLD
                } else {
                    0
                },
            ambient: self.cfg.ambient,
            shadow_dist: self.cfg.shadow_dist,
            world_epoch: self.world.version as u32,
            exp_shadow_repro: self.cfg.exp_shadow_repro,
            fog: self.cfg.fog,
            clouds: self.cfg.clouds,
            sky: self.cfg.sky,
            godrays: self.cfg.godrays,
            sea_level: self.cfg.sea_level as f32,
            water_absorb: self.cfg.water_absorb,
            jitter: self.cfg.jitter.map(Vec2::from),
            taa: self.prefs.taa,
            mip_bias: self.cfg.mip_bias,
            tint: render::Tint {
                strength: self.cfg.tint_strength,
                seed: self.cfg.seed,
            },
            waves: waves_from(&self.cfg),
            shaft_origin: self.shaft.origin_world(),
            shaft_texel: self.shaft.texel() as f32,
            shaft_scan: scene::shaft_scan(&self.cfg, flags, sun_dir),
        };

        let still = idle::StaticFrame {
            cam: cam.to_array(),
            yaw: self.player.yaw,
            pitch: self.player.pitch,
            world: self.world.version,
            probes_pending,
            relight_pending: !self.relight_queue.is_empty(),
            flags,
            spec_hi: params.spec_hi,
            taa: self.prefs.taa,
        };
        let kind = if self.cfg.idle_repaint {
            self.idle
                .classify(still, self.idle_animated, idle::sun_tick(self.time_of_day))
        } else {
            FrameKind::Render
        };

        let Some(surf) = &self.surface else { return };
        let frame = match surf.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f)
            | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                let size = (surf.config.width, surf.config.height);
                self.reconfigure(size);
                return;
            }
            _ => return,
        };
        let view = frame.texture.create_view(&Default::default());
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        match kind {

            FrameKind::Repaint if self.rendered_once => {
                renderer.repaint(&view);
                self.repainted_last = true;
            }
            _ => {
                renderer.render(&params, Some(&view));
                self.repainted_last = false;
                self.rendered_once = true;
            }
        }
        renderer.gpu.queue.present(frame);

        self.stats
            .record_cpu((Instant::now() - cpu_start).as_secs_f32() * 1000.0);
    }
}

pub(crate) fn run(mut cfg: Config) {

    let path = settings::preference_path();
    let mut status = String::new();
    let mut prefs = match settings::load(&path) {
        Ok(Some(p)) => p,
        Ok(None) => Preferences::from_config(&cfg),
        Err(e) => { eprintln!("settings: {e}"); status = format!("Settings not loaded: {e}"); Preferences::from_config(&cfg) }
    };
    prefs.overlay_cli(&cfg, &std::env::args().skip(1).collect::<Vec<_>>());
    if let Err(e) = prefs.validate() {
        eprintln!("settings: {e}; using safe menu defaults");
        status = format!("Invalid window settings: {e}"); prefs = Preferences::default();
    }
    prefs.write_config(&mut cfg);

    let journal = match journal::open(&cfg) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new(cfg, journal, path, status);
    event_loop.run_app(&mut app).expect("run");
}
