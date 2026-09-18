//! Native CPU contracts for architecture options; GPU integration lives in validation/check_split.py.
mod support;
use voxelcraft::config::{Config, parse_from};
use voxelcraft::settings::Preferences;
use voxelcraft::menu::{Menu, Page, Setting};
use voxelcraft::render::{SPEC_HI_MASK,FLAG_HI_ISOLATE_GLASS,FLAG_HI_SHADOW_PASS};

#[test]
fn primary_shadow_architecture_is_mandatory() {
    let d=Config::default();assert!(!d.isolate_glass && d.shadow_pass);
    let args:Vec<String>=["--isolate-glass","--shadow-pass"].iter().map(|s|s.to_string()).collect();
    let (c,_,unknown)=parse_from(&args);assert!(unknown.is_empty());assert!(c.isolate_glass && c.shadow_pass);
    let mut p=Preferences::from_config(&c);
    let args:Vec<String>=["--no-isolate-glass","--no-shadow-pass"].iter().map(|s|s.to_string()).collect();
    let (c,_,unknown)=parse_from(&args);assert!(unknown.is_empty());p.overlay_cli(&c,&args);
    assert!(!p.isolate_glass && p.shadow_pass);
    for bit in [FLAG_HI_ISOLATE_GLASS,FLAG_HI_SHADOW_PASS] {assert!(bit.is_power_of_two());assert_eq!(SPEC_HI_MASK&bit,bit);}
    assert_eq!(FLAG_HI_ISOLATE_GLASS&FLAG_HI_SHADOW_PASS,0);
}
#[test]
fn v2_migration_enables_the_required_primary_mask() {
    let p=Preferences::decode("version=2\ncompact_shade_hit=true\nsoft_shadows=true").unwrap();
    assert!(p.compact_shade_hit && p.soft_shadows);assert!(!p.isolate_glass && p.shadow_pass);
    assert!(Preferences::decode("version=2\nshadow_pass=true").is_err());
    let mut p=p;p.isolate_glass=true;p.shadow_pass=true;
    assert!(p.encode().starts_with("version=3\n"));assert_eq!(Preferences::decode(&p.encode()).unwrap(),p);
}
#[test]
fn new_options_are_title_only_and_defaults_preserve_locked_state() {
    let mut p=Preferences::default();p.shadow_pass=true;p.isolate_glass=true;
    let mut m=Menu::new(p.clone());m.open_settings(&p);m.page=Page::Experimental;
    assert_eq!(m.rows().len(),2);m.page=Page::Effects;assert_eq!(m.rows().len(),3);
    m.experiments_locked=true;m.adjust(Setting::ShadowPass,1);m.adjust(Setting::IsolateGlass,1);
    assert_eq!(m.draft,p);m.restore_defaults();assert!(m.draft.shadow_pass && m.draft.isolate_glass);
    let mut off=p.clone();off.shadow_pass=false;assert!(p.plan(&off).experiments);
}
#[test]
fn cached_schedules_do_not_trace_local_shadows() {
    let s=include_str!("../src/render/shaders/resolve.wgsl");
    for name in ["shade_hit_legacy_cached","shade_hit_compact_cached"] {
        let body=support::wgsl_function(s,name);
        assert!(body.contains("lit = textureLoad(primary_sun,"));
        assert!(!body.contains("shadow_ray(") && !body.contains("sun_vis_soft("));
        assert!(body.contains("terrain_shade_beyond(hit, shadow_dist)"));
        assert!(body.contains("cloud_shade(hit, cloud_fp)"));
        assert!(body.contains("direct = lit * ndl * sky_sm"));
    }
    let src=include_str!("../src/render/shaders/shadow.wgsl");
    let pass=support::wgsl_function(src,"primary_shadow");
    assert_eq!(pass.matches("shadow_ray_t(").count(),1);
    assert!(!pass.contains("shadow_ray("));
    assert!(pass.contains("textureStore(shadow_dist_tex"));
}
#[test]
fn material_lanes_are_compile_time_and_secondary_paths_keep_their_own_shadows() {
    let s=include_str!("../src/render/shaders/resolve.wgsl");let body=support::wgsl_function(s,"resolve");
    assert!(body.contains("RESOLVE_LANE == 1u"));assert!(body.contains("RESOLVE_LANE == 2u"));
    assert!(!body.contains("color = shade_hit("));
    assert!(body.contains("if RESOLVE_LANE == 0u { return; }"));
    assert!(body.contains("shade_hit_cached(ci,v,id,normal_id,hit,rd,t,pix)"));
    let glass=support::wgsl_function(s,"shade_glass");assert!(!glass.contains("primary_sun"));
    let water=support::wgsl_function(s,"water_reflect_leg");assert!(!water.contains("primary_sun"));
}
#[test]
fn final_visibility_precedes_all_new_passes_and_taa_follows_them() {
    let s=include_str!("../src/render/mod.rs");
    let recovery=s.find("label: Some(\"march recovered\")").unwrap();
    let shadow=s.find("label: Some(\"primary shadow / split family begin\")").unwrap();
    let opaque=s.find("\"opaque resolve\"").unwrap();
    let end=s.find("label: Some(\"split family end\")").unwrap();
    let taa=s.find("label: Some(\"taa\")").unwrap();
    assert!(recovery<shadow && shadow<opaque && opaque<end && end<taa);
    assert!(s.contains("self.ensure_shadow_targets((w,h))"));
    assert!(s.contains("let spec_pipes = Vec::new();"));
    assert!(s.contains("primary_shadow: shadow.then("));assert!(s.contains("water_resolve: shadow.then("));
}
#[test]
fn resolve_timestamps_bracket_the_family_not_just_the_cheaper_opaque_dispatch() {
    let s=include_str!("../src/render/timer.rs");
    assert!(s.contains("beginning_of_pass_write_index: if begin { Some(q::RESOLVE) } else { None }"));
    assert!(s.contains("end_of_pass_write_index: if begin { None } else { Some(q::RESOLVE + 1) }"));
    let s=include_str!("../src/render/mod.rs");
    assert!(s.contains("self.timer.family_timestamp(true)"));assert!(s.contains("self.timer.family_timestamp(false)"));
}
