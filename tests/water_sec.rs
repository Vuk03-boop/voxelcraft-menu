use voxelcraft::block::{self, WATER};
use voxelcraft::render;

#[test]
fn water_is_its_own_class_only() {
    assert!(!block::is_foliage(WATER));
    assert!(!block::is_cutout(WATER));
    assert!(!block::is_glass(WATER));
    assert!(!block::is_emitter(WATER));
}

#[test]
fn entry_points_and_binding_table_carry_the_pass() {
    let eps = &render::ENTRY_POINTS;
    assert_eq!(eps.len(), 11);
    assert_eq!(eps.iter().filter(|e| **e == "water_sec").count(), 1);
    let at = eps.iter().position(|e| *e == "water_sec").unwrap();
    assert_eq!(eps[at + 1], "resolve", "the reader must follow the pass");

    assert_eq!(render::BIND_GROUP_0_ENTRIES, 22);
    let src = render::shader_source();
    assert!(src.contains("@group(1) @binding(0) var water_refr_sample: texture_2d<f32>;"));
    assert!(src.contains("@group(2) @binding(0) var water_refr_tex: texture_storage_2d<rgba16float, write>;"));
    assert!(src.contains("@group(1) @binding(1) var water_refl_sample: texture_2d<f32>;"));
    assert!(src.contains("@group(2) @binding(1) var water_refl_tex: texture_storage_2d<rgba16float, write>;"));
}

#[test]
fn water_sec_is_a_spec_word_member() {
    assert_ne!(render::SPEC_HI_MASK & render::FLAG_HI_WATER_SEC, 0);

}

#[test]
fn water_sec_scale_carries_the_fraction_sweep() {
    use voxelcraft::config::Config;
    assert_eq!(Config::default().water_sec_scale, 2);
    assert_eq!(2u32.trailing_zeros(), 1);
    assert_eq!(1u32.trailing_zeros(), 0);
    assert_eq!(4u32.trailing_zeros(), 2);
    let src = render::shader_source();
    assert!(src.contains("override WATER_SEC_SHIFT: u32 = 1u;"), "the shift override");
}
