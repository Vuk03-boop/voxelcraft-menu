//! # Batch 73's emitter counter and batch 74's waterline packing
//!
//! Both are world facts that key pictures, and both are cheap enough to prove exactly, so
//! they get the *proof* test shape: a claim about a counter whose every transition points
//! at the line that made it is stronger than any image comparison could be -- a pixel
//! that is the same by chance and a counter that is right by construction are different
//! kinds of evidence, and only the second ships blind.
//!
//! - `tests/emitter_gate.rs` (this file): the lamp count's transitions, and the
//!   waterline's nibble as the interned `Inner` actually carries it.

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

// ---------------------------------------------------------------------
// 1. the lamp count's four transitions, walked one at a time
// ---------------------------------------------------------------------

/// Insert -> set a lamp -> clear the lamp -> unload. The count is the contract the
/// `SPEC_EMITTER_GATHER` key is computed from, so the right assertion shape is every edge
/// of it: the bake sets it, an edit introducing the world's first lamp sets it, removing
/// that lamp clears it again on the same frame (D5's exact counter replaced the first
/// build's conservative upward arming -- the edit already knows both ids, so a scan was
/// never necessary), and the unload clears whatever is left.
#[test]
fn the_counter_has_the_transitions_the_feature_states() {
    let mut w = World::new();
    assert!(!w.any_emitter_resident(), "a fresh world has no lamps");

    // A chunk with a lamp lights the world from insertion.
    let lit = dense_from(|x, y, z| if x == 3 && y == 4 && z == 5 { GLOWSTONE } else { STONE });
    w.insert_local(key(0, 0, 0), build_local(&lit));
    assert!(w.any_emitter_resident(), "a lamp in the bake is a resident emitter");

    // One without does not, and introducing the first by edit does -- that is the whole
    // point of the set_block hook, the frame the gate is paid for.
    let dark = dense_from(|_, _, _| STONE);
    let mut w2 = World::new();
    w2.insert_local(key(7, 0, -3), build_local(&dark));
    assert!(!w2.any_emitter_resident());
    let p = IVec3::new(7 * 64 + 10, 20, -3 * 64 + 11);
    assert!(w2.set_block(p, GLOWSTONE), "resident chunk takes the lamp");
    assert!(w2.any_emitter_resident(), "the first placed lamp lights the world");

    // Removing it again DOES clear the world, on the same frame -- D5's answer: the
    // edit sees the old texel and the new id, so the per-chunk count is exact and
    // zero-crossings are known without a scan.
    assert!(w2.set_block(p, STONE));
    assert!(
        !w2.any_emitter_resident(),
        "exact: breaking the chunk's one lamp disarms the gather on the frame, not at unload"
    );
    assert_eq!(w2.emitter_chunks, 0);

    // Unloading is the other side of the counter.
    assert!(w.remove_chunk(key(0, 0, 0)));
    assert!(!w.any_emitter_resident(), "last lamp-chunk's unload darkens the world");
    assert!(w2.remove_chunk(key(7, 0, -3)));
    assert!(!w2.any_emitter_resident());
}

/// The intermediate values are exact too, not just the zero crossings: one lamp placed,
/// removed, placed again must read 1, 0, 1 with the world's own counter.
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

/// Two emitter chunks, one unload: the count is a count, not a boolean.
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

/// Re-inserting a chunk over itself first removes the old one -- the replacement path
/// must not double-count.
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

// ---------------------------------------------------------------------
// 2. the waterline nibble rides the interned Inner
// ---------------------------------------------------------------------

/// The packing claim behind every `l1.w` the shader reads: `PREFIX_MASK` covers exactly
/// the count and stops below the nibble. A chunk's brick count -- 64 nodes of at most 64
/// bricks each -- reaches 4096 and stops six bits shy of the nibble's start at 28, so the
/// two fields the shader masks apart can never meet.
#[test]
fn the_nibble_and_the_count_do_not_collide() {
    assert!(PREFIX_MASK > 64 * 64, "a chunk's brick count cannot leave the mask");
    assert_eq!(PREFIX_MASK & 0xF000_0000, 0, "the mask must stop below the nibble");
}

/// A seabed slab and the sea over it, air above 40: band 0 (local y 0..15) carries the
/// slab's top -- 8, the highest non-water y in it; bands 1 and 2 are all water and carry
/// 15, "no bound, walk as before"; band 3 is air and exists as nothing at all.
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
        let yn = (l1b >> 2) & 3; // cell_bit's own packing: x | y<<2 | z<<4
        match yn {
            // One node per band-column: the whole chunk is column-identical, so every
            // cell in band 0 carries the same line and every cell above it the same
            // "no bound".
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

/// The count below the nibble is the same operation on both sides: the shader masks
/// `l1.w & 0x0FFFFFFF` by hand, the CPU masks with `PREFIX_MASK`, and this is the test
/// that keeps the two copies one copy.
#[test]
fn the_two_masks_are_one_mask() {
    assert_eq!(
        PREFIX_MASK, 0x0FFF_FFFF,
        "the Rust constant the WGSL comment quotes"
    );
}

/// An edit that *widens* a node into a full one deliberately carries 15: no scan runs on
/// the edit path, and "walk as before" is what the shader does with a disabled bound.
#[test]
fn an_edit_produced_full_node_disables_the_line() {
    let mut w = World::new();
    // One air pocket in band 0 and one in band 3, so filling the first fills a node
    // without filling the root.
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
    // And the pocket-keeper node in band 3 kept its measured line from the bake: it is
    // full of stone -- all non-water -- so its line is 15 already, which is the same
    // number for the same reason and the two had better not drift apart silently.
    // The pocket at voxel (5, 50, 5) sits in 16-block cell (0, 3, 0) -- cell bit 12.
    let k = w.l1_node(rec.root, cell_bit_helper(0, 3, 0)).expect("band 3 exists");
    assert!(!k.is_empty(), "band 3's cell is nearly full");
}

/// `cell_bit` for the l1 scale, transliterated so the test reads like the march that
/// consumes its answer: l1b = x | y<<2 | z<<4 with the cell coords here, not voxel ones.
fn cell_bit_helper(x: u32, y: u32, z: u32) -> u32 {
    x | (y << 2) | (z << 4)
}

