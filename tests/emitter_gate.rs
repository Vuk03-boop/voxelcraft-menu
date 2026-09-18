use glam::IVec3;
use voxelcraft::block::*;
use voxelcraft::voxel::*;

fn dense_from(f: impl Fn(u32, u32, u32) -> BlockId) -> Vec<BlockId> {
    let mut d = vec![AIR; VOL];
    for y in 0..64u32 {
        for z in 0..64u32 {
            for x in 0..64u32 {
                d[dense_index(x as usize, y as usize, z as usize)] = f(x, y, z);
            }
        }
    }
    d
}

fn key(x: i32, y: i32, z: i32) -> ChunkKey {
    ChunkKey::new(0, IVec3::new(x, y, z))
}

#[test]
fn the_counter_has_the_transitions_the_feature_states() {
    let mut w = World::new();
    assert!(!w.any_emitter_resident(), "a fresh world has no lamps");

    let lit = dense_from(|x, y, z| if x == 3 && y == 4 && z == 5 { GLOWSTONE } else { STONE });
    w.insert_local(key(0, 0, 0), build_local(&lit));
    assert!(w.any_emitter_resident(), "a lamp in the bake is a resident emitter");

    let dark = dense_from(|_, _, _| STONE);
    let mut w2 = World::new();
    w2.insert_local(key(7, 0, -3), build_local(&dark));
    assert!(!w2.any_emitter_resident());
    let p = IVec3::new(7 * 64 + 10, 20, -3 * 64 + 11);
    assert!(w2.set_block(p, GLOWSTONE), "resident chunk takes the lamp");
    assert!(w2.any_emitter_resident(), "the first placed lamp lights the world");

    assert!(w2.set_block(p, STONE));
    assert!(
        !w2.any_emitter_resident(),
        "exact: breaking the chunk's one lamp disarms the gather on the frame, not at unload"
    );
    assert_eq!(w2.emitter_chunks, 0);

    assert!(w.remove_chunk(key(0, 0, 0)));
    assert!(!w.any_emitter_resident(), "last lamp-chunk's unload darkens the world");
    assert!(w2.remove_chunk(key(7, 0, -3)));
    assert!(!w2.any_emitter_resident());
}

#[test]
fn the_edit_path_counts_exactly_both_ways() {
    let dark = dense_from(|_, _, _| STONE);
    let mut w = World::new();
    w.insert_local(key(0, 0, 0), build_local(&dark));
    let a = IVec3::new(4, 4, 4);
    let b = IVec3::new(8, 8, 8);
    assert!(w.set_block(a, GLOWSTONE));
    assert!(w.set_block(b, GLOWSTONE));
    assert_eq!(w.emitter_chunks, 1);
    assert_eq!(w.chunks[&key(0, 0, 0)].emitters, 2);
    assert!(w.set_block(a, AIR), "a lamp can be mined back to air");
    assert_eq!(w.chunks[&key(0, 0, 0)].emitters, 1, "one of two lamps down");
    assert!(w.any_emitter_resident(), "the other lamp still arms the gate");
    assert!(w.set_block(b, GLOWSTONE), "re-writing the same id changes nothing...");
    assert_eq!(w.chunks[&key(0, 0, 0)].emitters, 1, "...and the count agrees");
    assert!(w.set_block(b, AIR));
    assert_eq!(w.chunks[&key(0, 0, 0)].emitters, 0);
    assert!(!w.any_emitter_resident());
}

#[test]
fn the_counter_is_a_count_and_decrements_to_zero_only_at_zero() {
    let lit = dense_from(|x, y, z| if (x + y + z) == 3 { GLOWSTONE } else { STONE });
    let mut w = World::new();
    w.insert_local(key(0, 0, 0), build_local(&lit));
    w.insert_local(key(1, 0, 0), build_local(&lit));
    assert_eq!(w.emitter_chunks, 2);
    assert!(w.remove_chunk(key(0, 0, 0)));
    assert_eq!(w.emitter_chunks, 1);
    assert!(w.any_emitter_resident(), "one lamp-chunk still lights the world");
    assert!(w.remove_chunk(key(1, 0, 0)));
    assert_eq!(w.emitter_chunks, 0);
    assert!(!w.any_emitter_resident());
}

#[test]
fn replacement_unloads_before_it_inserts() {
    let lit = dense_from(|x, y, z| if x == 0 && y == 0 && z == 0 { GLOWSTONE } else { STONE });
    let dark = dense_from(|_, _, _| STONE);
    let mut w = World::new();
    w.insert_local(key(0, 0, 0), build_local(&lit));
    assert_eq!(w.emitter_chunks, 1);
    w.insert_local(key(0, 0, 0), build_local(&dark));
    assert_eq!(
        w.emitter_chunks, 0,
        "a re-bake of the same key pays the old chunk's lamp back"
    );
    assert!(!w.any_emitter_resident());
}

#[test]
fn the_nibble_and_the_count_do_not_collide() {
    assert!(PREFIX_MASK > 64 * 64, "a chunk's brick count cannot leave the mask");
    assert_eq!(PREFIX_MASK & 0xF000_0000, 0, "the mask must stop below the nibble");
}

#[test]
fn seabed_sets_its_line_and_open_sea_carries_fifteen() {
    let dense = dense_from(|_, y, _| {
        if y <= 8 {
            STONE
        } else if y <= 40 {
            WATER
        } else {
            AIR
        }
    });
    let mut w = World::new();
    w.insert_local(key(0, 0, 0), build_local(&dense));
    let rec = &w.chunks[&key(0, 0, 0)];
    assert!(!rec.root.is_full(), "the air cap keeps the root non-full");
    for l1b in 0..64u32 {
        let yn = (l1b >> 2) & 3;
        match yn {

            0 => assert_eq!(
                w.l1_node(rec.root, l1b).expect("band 0 is solid").leaf_prefix >> 28,
                8,
                "band 0 carries the slab's top"
            ),
            3 => assert!(
                w.l1_node(rec.root, l1b).is_none(),
                "the air band has no node to bound"
            ),
            _ => assert_eq!(
                w.l1_node(rec.root, l1b).expect("the sea band is water").leaf_prefix >> 28,
                15,
                "all-water bands carry 'no bound'"
            ),
        }
    }
}

#[test]
fn the_two_masks_are_one_mask() {
    assert_eq!(
        PREFIX_MASK, 0x0FFF_FFFF,
        "the Rust constant the WGSL comment quotes"
    );
}

#[test]
fn an_edit_produced_full_node_disables_the_line() {
    let mut w = World::new();

    let dense = dense_from(|x, y, z| {
        if (x == 1 && y == 2 && z == 3) || (x == 5 && y == 50 && z == 5) {
            AIR
        } else {
            STONE
        }
    });
    w.insert_local(key(0, 0, 0), build_local(&dense));
    let p = IVec3::new(1, 2, 3);
    assert!(w.set_block(p, STONE));
    let rec = &w.chunks[&key(0, 0, 0)];
    assert!(!rec.root.is_full(), "band 3's pocket keeps the root partial");
    let n = w.l1_node(rec.root, 0).expect("band 0 exists");
    assert!(n.is_full(), "filling the pocket fills the node");
    assert_eq!(
        n.leaf_prefix >> 28,
        0xF,
        "the edit path has no waterline scan; 15 disables the bound conservatively"
    );

    let k = w.l1_node(rec.root, cell_bit_helper(0, 3, 0)).expect("band 3 exists");
    assert!(!k.is_empty(), "band 3's cell is nearly full");
}

fn cell_bit_helper(x: u32, y: u32, z: u32) -> u32 {
    x | (y << 2) | (z << 4)
}
