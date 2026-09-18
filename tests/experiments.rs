mod support;
use voxelcraft::config::{Config, parse_from};
use voxelcraft::menu::{Menu, Page, Screen, Setting};
use voxelcraft::settings::Preferences;
use voxelcraft::render::{FLAG_HI_COMPACT_SHADE_HIT, SPEC_HI_MASK};

#[test]
fn every_experiment_defaults_off_and_old_preferences_migrate_off() {
    let p=Preferences::decode("version=1\nscale=0.8").unwrap();
    assert_eq!(p.scale,0.8);
    assert!(!p.compact_shade_hit && !p.glass_reflect && !p.water_look && !p.wind_sway && !p.soft_shadows);
    let c=Config::default();assert!(!c.compact_shade_hit);
    assert!(Preferences::decode("version=1\ncompact_shade_hit=true").is_err());
}
#[test]
fn current_version_round_trips_all_experiments() {
    let mut p=Preferences::default();
    p.compact_shade_hit=true;p.glass_reflect=true;p.water_look=true;p.wind_sway=true;p.soft_shadows=true;
    let encoded=p.encode();assert!(encoded.starts_with("version=3\n"));
    assert_eq!(Preferences::decode(&encoded).unwrap(),p);
}
#[test]
fn explicit_disable_flags_override_saved_experiments() {
    let mut p=Preferences::default();p.compact_shade_hit=true;p.glass_reflect=true;p.water_look=true;p.wind_sway=true;p.soft_shadows=true;
    let args:Vec<String>=["--no-compact-shade-hit","--no-glass-reflect","--no-water-look","--no-wind-sway","--no-soft-shadows"].iter().map(|s|s.to_string()).collect();
    let (c,_,unknown)=parse_from(&args);assert!(unknown.is_empty());p.overlay_cli(&c,&args);
    assert!(!p.experiments_differ(&Preferences::default()));
}
#[test]
fn new_cli_switch_has_a_revert_and_real_key() {
    let args=vec!["--compact-shade-hit".into()];let (c,_,unknown)=parse_from(&args);
    assert!(unknown.is_empty());assert!(c.compact_shade_hit);
    assert!(FLAG_HI_COMPACT_SHADE_HIT.is_power_of_two());
    assert_eq!(SPEC_HI_MASK & FLAG_HI_COMPACT_SHADE_HIT,FLAG_HI_COMPACT_SHADE_HIT);
    assert!(include_str!("../src/scene.rs").contains("if cfg.compact_shade_hit { f |= render::FLAG_HI_COMPACT_SHADE_HIT; }"));
    assert!(include_str!("../src/render/mod.rs").contains("\"SPEC_COMPACT_SHADE_HIT\", if spec_hi & FLAG_HI_COMPACT_SHADE_HIT"));
}
#[test]
fn experimental_plan_is_not_a_uniform_only_update() {
    let a=Preferences::default();let mut b=a.clone();b.compact_shade_hit=true;
    let plan=a.plan(&b);assert!(plan.experiments && plan.history);
    assert!(!plan.resize && !plan.surface && !plan.presentation);
}
#[test]
fn active_session_locks_shader_switches_and_defaults_preserve_them() {
    let mut p=Preferences::default();p.wind_sway=true;
    let mut m=Menu::new(p.clone());m.pause();m.open_settings(&p);m.experiments_locked=true;
    for s in [Setting::IsolateGlass,Setting::ShadowPass,Setting::CompactShade,Setting::GlassReflect,Setting::WaterLook,Setting::WindSway,Setting::SoftShadows] {m.adjust(s,1);assert_eq!(m.draft,p);}
    m.adjust(Setting::Scale,-1);assert_ne!(m.draft.scale,p.scale);
    m.restore_defaults();assert!(m.draft.wind_sway);assert_eq!(m.draft.scale,Preferences::default().scale);
    m.cancel();assert_eq!(m.screen,Screen::Pause);
}
#[test]
fn title_can_edit_and_reset_every_experimental_control() {
    let p=Preferences::default();let mut m=Menu::new(p.clone());m.open_settings(&p);m.page=Page::Experimental;
    assert_eq!(m.rows().len(),2);
    for page in [Page::Experimental, Page::Effects] {
        m.page=page;
        for (s,_,_) in m.rows() {assert!(s.experimental());m.adjust(s,1);}
    }
    assert!(m.draft.compact_shade_hit && m.draft.glass_reflect && m.draft.water_look && m.draft.wind_sway && m.draft.soft_shadows);
    m.restore_defaults();assert!(!m.draft.experiments_differ(&p));
}
#[test]
fn compact_arm_moves_material_after_lighting_without_moving_specular_late() {
    let src=include_str!("../src/render/shaders/resolve.wgsl");
    let a=support::wgsl_function(src,"shade_hit_legacy");let b=support::wgsl_function(src,"shade_hit_compact");
    assert!(a.find("textureSampleLevel(atlas").unwrap()<a.find("gather_face(").unwrap());
    assert!(b.find("gather_face(").unwrap()<b.find("textureSampleLevel(atlas").unwrap());
    assert!(b.find("shadow_ray(").unwrap()<b.find("textureSampleLevel(atlas").unwrap());
    assert!(b.find("probe_field(").unwrap()<b.find("textureSampleLevel(atlas").unwrap());
    assert!(b.find("sky_base(reflect(rd, n)").unwrap()<b.find("gather_face(").unwrap());
    assert!(b.contains("return surface_radiance(albedo, light_rgb * face_scale, spec_value, id);"));
    let wrapper=support::wgsl_function(src,"shade_hit");
    assert!(wrapper.contains("if SPEC_COMPACT_SHADE_HIT"));assert!(wrapper.contains("return shade_hit_legacy("));
}
