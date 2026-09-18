//! Surface-repair contracts. GPU traversal/readback checks live in validation/check_surface.py.
mod support;
use voxelcraft::{block, config, light};
use voxelcraft::voxel::{VOL, tree::{dense_index,cell_bit}};

fn sample(d: &light::LightData,x:usize,y:usize,z:usize)->u16 {
    if let Some(v)=d.uniform { return v; }
    let cell=(x>>2)|((y>>2)<<4)|((z>>2)<<8);
    let entry=d.cells[cell];
    if entry & (1<<31)!=0 { return entry as u16; }
    let v=cell_bit((x&3) as u32,(y&3) as u32,(z&3) as u32) as usize;
    (d.bricks[entry as usize][v>>1] >> ((v&1)*16)) as u16
}
#[test]
fn glowstone_keeps_geometry_and_emission_but_does_not_block_sunlight() {
    assert!(block::def(block::GLOWSTONE).solid && block::def(block::GLOWSTONE).opaque);
    assert!(block::is_emitter(block::GLOWSTONE));
    assert!(!block::casts_sun_shadow(block::GLOWSTONE));
    assert!(block::casts_sun_shadow(block::STONE));
    assert!(block::casts_sun_shadow(block::AMBER_LAMP));
    let mut dense=vec![block::AIR;VOL];
    for z in 0..64 { for x in 0..64 { dense[dense_index(x,32,z)]=block::GLOWSTONE; } }
    let lit=light::compute(&dense,&vec![true;4096]);
    assert_eq!(light::sky_of(sample(&lit,32,31,32)),15);
    for z in 0..64 { for x in 0..64 { dense[dense_index(x,32,z)]=block::STONE; } }
    let dark=light::compute(&dense,&vec![true;4096]);
    assert_eq!(light::sky_of(sample(&dark,32,31,32)),0);
}
#[test]
fn cli_controls_are_explicit_and_do_not_enable_performance_experiments() {
    let args:Vec<String>=["--legacy-lighting","--legacy-water","--soft-shadow-hq","--sun-softness","wide","--surface-debug","water-faces"].iter().map(|s|s.to_string()).collect();
    let (c,_,unknown)=config::parse_from(&args);assert!(unknown.is_empty());
    assert!(c.legacy_lighting && c.legacy_water && c.soft_shadow_hq);
    assert_eq!(c.sun_softness,2);assert_eq!(c.surface_debug,5);
    assert!(c.shadow_pass && !c.isolate_glass && !c.compact_shade_hit);
    let d=config::Config::default();assert!(!d.legacy_lighting && !d.legacy_water && !d.soft_shadow_hq);
}
#[test]
fn shadow_material_filter_is_separate_from_camera_and_full_root_shortcut() {
    let s=include_str!("../src/render/shaders/common.wgsl");
    let body=support::wgsl_function(s,"march_chunk");
    assert!(body.contains("sun_ray && (c.attr_flags & ATTR_HAS_EMITTER) != 0u"));
    assert!(body.contains("!SPEC_FULL_MARCH && !noncaster_aware && !water_interface"));
    assert!(body.contains("if !casts_sun_shadow(id)"));
    let shadow=include_str!("../src/render/shaders/shadow.wgsl");
    assert!(shadow.contains("shadow_horizontal"));
    assert!(!s.contains("fn sun_vis_soft("));
}
#[test]
fn water_repair_checks_occupancy_and_rejects_incompatible_donors() {
    let s=include_str!("../src/render/shaders/common.wgsl");
    assert!(support::wgsl_function(s,"world_block_at").contains("!voxel_occupied(ci, v)"));
    let s=include_str!("../src/render/shaders/resolve.wgsl");
    let safe=support::wgsl_function(s,"water_share_safe");
    for text in ["normal_id != NORMAL_PY", "nid!=normal_id", "block_at(ci,v)!=WATER_ID", "abs(hd.y-hit.y)>0.01"] { assert!(safe.contains(text)); }
    assert!(support::wgsl_function(s,"shade_water_sec").contains("return shade_water(ci, v, normal_id, hit, rd, t)"));
    for name in ["shade_hit_legacy","shade_hit_compact","shade_hit_legacy_cached","shade_hit_compact_cached"] {
        let b=support::wgsl_function(s,name);assert!(b.contains("surface_sky_fill(n)"));assert!(b.contains("surface_radiance(albedo,"));
    }
}
