use glam::IVec3;
use voxelcraft::block;
use voxelcraft::config::parse_from;
use voxelcraft::voxel::tree::dense_index;
use voxelcraft::voxel::{ChunkKey, World, VOL};

#[test]
fn a_mixed_chunk_of_sea_and_floor_reaches_the_shader_as_a_full_mixed_root() {
    let mut dense = vec![block::AIR; VOL];
    for y in 0..64 {
        for z in 0..64 {
            for x in 0..64 {
                dense[dense_index(x, y, z)] = if y < 32 { block::STONE } else { block::WATER };
            }
        }
    }
    let t = voxelcraft::voxel::build_local(&dense);
    let mut world = World::new();
    let key = ChunkKey::new(0, IVec3::new(0, 0, 0));
    world.insert_local(key, t);
    let rec = &world.chunks[&key];

    assert!(
        rec.root.is_full(),
        "water above stone, every voxel occupied: the root has to collapse"
    );
    assert!(rec.has_water, "and the chunk has to say so");
    assert_ne!(
        rec.dry_mask, u64::MAX,
        "the water-only cells have to stay out of the dry mask"
    );
    assert_ne!(
        rec.dry_mask, 0,
        "and the floor's cells have to be in it: with the fix the whole march runs on this word"
    );
    assert!(
        rec.dry_mask.count_ones() == 32,
        "the bottom 32 of the 64 16^3 cells hold dry voxels, the top 32 do not"
    );
}

#[test]
fn the_marcher_and_leaf_attr_synthesize_the_same_full_root_node() {
    let src = voxelcraft::render::shader_source();
    let synth = "vec4<u32>(0xFFFFFFFFu, 0xFFFFFFFFu, FULL_BIT, l1b * 64u";
    let count = src.matches(synth).count();
    assert!(
        count >= 2,
        "the synthesized full-root L1 node appears {count} time(s); leaf_attr and the march loop must both have it"
    );
}

#[test]
fn the_override_reaches_the_return() {
    let src = voxelcraft::render::shader_source();
    let override_decl = "override SPEC_FULL_MARCH: bool = true;";
    assert!(
        src.contains(override_decl),
        "SPEC_FULL_MARCH is not declared as an override in common.wgsl"
    );
    assert!(
        src.contains("root_full && !SPEC_FULL_MARCH"),
        "the control path has to test the override at the return itself"
    );
}

#[test]
fn the_control_parses() {
    assert!(
        voxelcraft::config::Config::default().full_march,
        "the fix ships on"
    );
    let args: Vec<String> = vec!["--no-full-march".to_string()];
    let (cfg, _mode, unknown) = parse_from(&args);
    assert!(
        unknown.is_empty(),
        "--no-full-march parsed as unknown: {unknown:?}"
    );
    assert!(!cfg.full_march, "--no-full-march has to turn the fix off");
}
