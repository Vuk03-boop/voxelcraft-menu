//! GPU-free contracts for the first menu. Full native-window acceptance is in MENU-NOTES.md.
use voxelcraft::config::{Config, parse_from};
use voxelcraft::menu::{Menu, Screen, Page, Setting, Action};
use voxelcraft::settings::{self, Preferences, Grade};
use winit::keyboard::KeyCode;

#[test]
fn preferences_round_trip() {
    let p=Preferences::default();
    assert_eq!(Preferences::decode(&p.encode()).unwrap(),p);
}
#[test]
fn refuses_nonfinite_out_of_range_and_unknown_values() {
    for text in ["version=1\nscale=NaN", "version=1\nfov=inf", "version=1\ngrade_strength=-1",
        "version=1\nscale=2", "version=1\nvsync=perhaps", "version=1\ngrade=3", "version=99", "fov=70",
        "version=1\nunknown=true", "version=1\nfov=60\nfov=70"] {
        assert!(Preferences::decode(text).is_err(),"accepted {text}");
    }
}
#[test]
fn missing_optional_fields_use_version_one_defaults() {
    assert_eq!(Preferences::decode("version=1").unwrap(),Preferences::default());
}
#[test]
fn explicit_cli_default_overrides_saved_nondefault() {
    let mut p=Preferences::default(); p.fov=100.0;p.scale=0.6;p.grade=Grade::Cine;
    let args:Vec<String>=["--fov","70","--scale","0.8","--grade","none"].into_iter().map(str::to_owned).collect();
    let (cfg,_,unknown)=parse_from(&args);assert!(unknown.is_empty());
    p.overlay_cli(&cfg,&args);
    assert_eq!(p.fov,70.0);assert_eq!(p.scale,0.8);assert_eq!(p.grade,Grade::None);
}
#[test]
fn omitted_cli_option_does_not_erase_preference() {
    let mut p=Preferences::default();p.scale=0.8;
    p.overlay_cli(&Config::default(),&[]);assert_eq!(p.scale,0.8);
}
#[test]
fn preferences_cannot_rewrite_world_recipe_or_unowned_shader_flags() {
    let mut c=Config::default();c.seed=-77;c.sea_level=96;c.grass_dense=true;c.foliage_rich=true;
    c.tree_blue_noise=true;c.edits=Some("my-world.journal".into());
    Preferences::default().write_config(&mut c);
    assert_eq!(c.seed,-77);assert_eq!(c.sea_level,96);
    assert!(c.grass_dense && c.foliage_rich && c.tree_blue_noise);
    assert_eq!(c.edits.as_deref(),Some("my-world.journal"));
}
#[test]
fn grade_is_exclusive_when_written_to_legacy_config() {
    let mut c=Config::default();let mut p=Preferences::default();
    for g in [Grade::None,Grade::Warm,Grade::Cine] {
        p.grade=g;p.write_config(&mut c);assert_eq!(c.grade_value(),g.index());
        assert!(!(c.grade_warm && c.grade_cine));
    }
}
#[test]
fn presentation_only_change_preserves_hdr_history() {
    let a=Preferences::default();let mut b=a.clone();b.grade=Grade::Warm;
    let p=a.plan(&b);assert!(p.presentation);assert!(!p.history && !p.resize && !p.surface);
}
#[test]
fn resize_and_lighting_changes_invalidate_history() {
    let a=Preferences::default();let mut b=a.clone();b.scale=0.8;
    let p=a.plan(&b);assert!(p.resize && p.history);
    b=a.clone();b.fog_density+=0.0001;assert!(a.plan(&b).history);
    b=a.clone();b.taa=!b.taa;assert!(a.plan(&b).history);
}
#[test]
fn vsync_does_not_require_world_targets() {
    let a=Preferences::default();let mut b=a.clone();b.vsync=!b.vsync;
    let p=a.plan(&b);assert!(p.surface);assert!(!p.resize && !p.history);
}
#[test]
fn cancel_does_not_mutate_applied_preferences() {
    let applied=Preferences::default();let mut m=Menu::new(applied.clone());
    m.open_settings(&applied);m.adjust(Setting::Scale,-1);assert_ne!(m.draft,applied);
    m.cancel();assert_eq!(m.screen,Screen::Title);
    m.open_settings(&applied);assert_eq!(m.draft,applied);
}
#[test]
fn settings_returns_to_pause_and_escape_resumes() {
    let p=Preferences::default();let mut m=Menu::new(p.clone());m.pause();m.open_settings(&p);
    assert_eq!(m.key(KeyCode::Escape),Some(Action::Cancel));m.cancel();assert_eq!(m.screen,Screen::Pause);
    assert_eq!(m.key(KeyCode::Escape),Some(Action::Resume));
}
#[test]
fn quit_confirmation_does_not_destroy_settings_return_state() {
    let p=Preferences::default();let mut m=Menu::new(p.clone());m.pause();m.open_settings(&p);
    m.confirm_quit();m.confirm_quit();m.dismiss_quit();assert_eq!(m.screen,Screen::Settings);
    m.cancel();assert_eq!(m.screen,Screen::Pause);
}
#[test]
fn pointer_mapping_and_hit_test_share_draw_coordinates() {
    let p=Preferences::default();let mut m=Menu::new(p.clone());m.open_settings(&p);
    for size in [(900,700),(1920,1080),(640,480),(320,240),(2560,1440)] {
        let s=(size.0 as f32/900.0).min(size.1 as f32/700.0);
        for item in m.items() {
            let [x,y,w,h]=item.rect;
            let physical=[(size.0 as f32-900.0*s)*0.5+(x+w*0.5)*s,
                          (size.1 as f32-700.0*s)*0.5+(y+h*0.5)*s];
            assert_eq!(m.click(physical,size),Some(item.action));
        }
    }
}
#[test]
fn keyboard_focus_wraps_and_loading_has_no_game_action() {
    let mut m=Menu::new(Preferences::default());m.key(KeyCode::ArrowUp);
    assert_eq!(m.focus,m.items().len()-1);
    m.key(KeyCode::ArrowDown);assert_eq!(m.focus,0);
    m.screen=Screen::Loading;assert!(m.items().is_empty());assert_eq!(m.key(KeyCode::Enter),None);
}
#[test]
fn adjustment_controls_cannot_escape_validation_ranges() {
    let p=Preferences::default();let mut m=Menu::new(p.clone());m.open_settings(&p);
    for page in [Page::Display,Page::Graphics,Page::Appearance,Page::Experimental,Page::Effects] {
        m.page=page;
        for (setting,_,_) in m.rows() {
            for _ in 0..400 {m.adjust(setting,1);m.draft.validate().unwrap();}
            for _ in 0..400 {m.adjust(setting,-1);m.draft.validate().unwrap();}
        }
    }
}
#[test]
fn file_save_replaces_complete_preferences_and_invalid_save_keeps_old() {
    let dir=std::env::temp_dir().join(format!("voxelcraft-settings-{}-{}",std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let path=dir.join("prefs.conf");let a=Preferences::default();
    assert!(settings::load(&path).unwrap().is_none());
    settings::save(&path,&a).unwrap();assert_eq!(settings::load(&path).unwrap(),Some(a.clone()));
    let mut b=a.clone();b.scale=0.8;settings::save(&path,&b).unwrap();
    assert_eq!(settings::load(&path).unwrap(),Some(b.clone()));
    b.scale=f32::NAN;assert!(settings::save(&path,&b).is_err());
    assert_eq!(settings::load(&path).unwrap().unwrap().scale,0.8);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn headless_paths_do_not_load_personal_preferences() {
    assert!(!include_str!("../src/headless.rs").contains("settings::load"));
    assert!(!include_str!("../src/config.rs").contains("settings::load"));
}
