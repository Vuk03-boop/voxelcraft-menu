//! The edit journal (batch 29), and the one claim it stands on.
//!
//! **A chunk built from `worldgen + deltas` has to be the chunk you edited**, block for block
//! and light for light. Everything else here -- the file round trip, the refusals -- is
//! bookkeeping around that. The pixel-level version of the same claim is the fourteen-vantage
//! sweep in `docs/persistence.md`; this file is the version that runs in a second and says
//! *which* block disagreed, which is why it exists rather than leaving the sweep to find out.
//!
//! Nothing here touches wgpu, by the same rule as the rest of `tests/`.

use glam::IVec3;
use std::sync::Arc;
use voxelcraft::block;
use voxelcraft::config::Config;
use voxelcraft::journal::{self, EditJournal, PlayerState};
use voxelcraft::player::Player;
use voxelcraft::light;
use voxelcraft::stream::{self, Streamer};
use voxelcraft::voxel::{dense_index, ChunkKey, World, VOL};
use voxelcraft::worldgen::{WorldGen, SEA_LEVEL};

const SEED: i32 = 1337;

fn generator(seed: i32) -> Arc<WorldGen> {
    Arc::new(WorldGen::with_options(seed, SEA_LEVEL, true, true, true))
}

/// A LOD-0 chunk with terrain in it: the sea-level band under the spawn column, which is
/// where `--demo-edits` builds and where every vantage but `cave` looks.
fn terrain_chunk() -> ChunkKey {
    ChunkKey::new(0, IVec3::new(0, SEA_LEVEL / 64, 0))
}

/// Local coordinates of a dense index. The inverse of [`dense_index`], spelled out here
/// rather than imported because `journal`'s copy is private and a second reader of a layout
/// is exactly the thing these tests exist to catch.
fn local_of(i: usize) -> (usize, usize, usize) {
    (i & 63, (i >> 12) & 63, (i >> 6) & 63)
}

/// A deterministic spread of edits over whatever is actually in this chunk: blocks broken,
/// blocks placed into air, and one emitter, because a change that no lighting pass can see
/// would leave half of the claim untested.
fn edits_for(dense: &[u16], origin: IVec3) -> Vec<(IVec3, u16)> {
    let at = |i: usize| {
        let (x, y, z) = local_of(i);
        origin + IVec3::new(x as i32, y as i32, z as i32)
    };
    let solid: Vec<usize> = (0..VOL).filter(|&i| dense[i] != block::AIR).collect();
    let air: Vec<usize> = (0..VOL).filter(|&i| dense[i] == block::AIR).collect();
    assert!(
        solid.len() > 64 && air.len() > 64,
        "fixture chunk is {} solid / {} air -- it has to hold both to test both",
        solid.len(),
        air.len()
    );

    // Spread rather than clustered, so the edits land in many different leaves: a leaf that
    // was uniform has a different attribute path from one that was already a palette.
    let mut out = Vec::new();
    for (list, id) in [(&solid, block::AIR), (&air, block::COBBLE)] {
        let step = (list.len() / 40).max(1);
        out.extend(list.iter().step_by(step).take(40).map(|&i| (at(i), id)));
    }
    // One emitter, because a change no lighting pass can see would leave half the claim
    // untested -- the light comparison below is the half that catches a wrong `open` column.
    // The last solid voxel, which the stride above cannot have taken: with more than 64 in
    // the list, `step_by(step).take(40)` stops short of the end. One position, one entry.
    out.push((at(solid[solid.len() - 1]), block::GLOWSTONE));
    out
}

/// `stream::relight` fills its column heights from `gen.height(x, z)` while
/// `WorldGen::generate` fills them from `column_field` at stride 1. **The two lighting routes
/// agree only if those two agree**, and nothing else in the tree asks them to -- a chunk
/// built by the streamer and a chunk relit after an edit would simply disagree about sky
/// access, silently, in whichever direction the discrepancy went.
#[test]
fn relight_and_worldgen_derive_the_same_column_heights() {
    let gen = generator(SEED);
    let key = terrain_chunk();
    let o = key.origin();
    let mut dense = vec![0u16; VOL];
    let mut heights = Box::new([0i32; 64 * 64]);
    gen.generate(key, &mut dense, &mut heights);
    for z in 0..64usize {
        for x in 0..64usize {
            assert_eq!(
                heights[x + z * 64],
                gen.height(o.x + x as i32, o.z + z as i32),
                "column ({x}, {z})"
            );
        }
    }
}

/// The load-bearing test: worldgen plus the journal is the same chunk as worldgen followed by
/// the edits, in blocks and in light.
///
/// The `assert_ne!` in the middle is not decoration. `docs/errors.md` records that a no-op
/// claim standing alone cannot be told apart from a not-wired-up claim: without it, a
/// `record` that dropped every edit and an `apply` that applied none would pass this test
/// perfectly.
#[test]
fn a_replayed_chunk_matches_one_edited_live() {
    let gen = generator(SEED);
    let key = terrain_chunk();
    let o = key.origin();

    // Side A: the chunk as the streamer builds it today, then edited through `set_block`.
    let journal = EditJournal::empty();
    let plain = Streamer::new(gen.clone());
    let built = plain.build_now(key);
    let mut live = World::new();
    live.attach_journal(journal.clone());
    live.insert_local(key, built.tree);
    if let Some(l) = built.light {
        live.set_light(key, l);
    }

    let mut before = vec![0u16; VOL];
    assert!(live.extract_dense(key, &mut before));
    for (p, id) in edits_for(&before, o) {
        assert!(live.set_block(p, id), "chunk is resident, so this must apply");
    }
    stream::relight(&mut live, &gen, key);

    let mut edited = vec![0u16; VOL];
    assert!(live.extract_dense(key, &mut edited));
    assert_ne!(
        edited, before,
        "the edits have to change the chunk, or everything below is vacuous"
    );
    assert_eq!(journal.len(), journal_count(&before, &edited));

    // Side B: the same journal replayed into the dense array at generation.
    let mut replaying = Streamer::new(gen.clone());
    replaying.set_journal(journal.clone());
    let rebuilt = replaying.build_now(key);
    let mut world = World::new();
    world.insert_local(key, rebuilt.tree);
    let mut replayed = vec![0u16; VOL];
    assert!(world.extract_dense(key, &mut replayed));

    let first = edited
        .iter()
        .zip(&replayed)
        .position(|(a, b)| a != b)
        .map(|i| {
            let (x, y, z) = local_of(i);
            format!("at local ({x}, {y}, {z}): live {} replayed {}", edited[i], replayed[i])
        });
    assert!(first.is_none(), "{}", first.unwrap_or_default());

    // And the light, which is the half a block comparison cannot reach: side B computed it
    // during the build, side A would have computed it in `relight`.
    let mut heights = Box::new([0i32; 64 * 64]);
    for z in 0..64usize {
        for x in 0..64usize {
            heights[x + z * 64] = gen.height(o.x + x as i32, o.z + z as i32);
        }
    }
    let open = light::open_columns(&heights, o.y + 64);
    let a_light = light::compute(&edited, &open);
    let b_light = rebuilt.light.expect("an edited chunk is not empty, so it carries light");
    assert_eq!(a_light.uniform, b_light.uniform, "light uniformity");
    assert_eq!(a_light.cells, b_light.cells, "light cell table");
    assert_eq!(a_light.bricks, b_light.bricks, "light bricks");
}

/// How many voxels actually changed, which is what the journal should hold: `set_block`
/// records past both of its redundant-write returns, so a write that changed nothing is not
/// an entry.
fn journal_count(before: &[u16], after: &[u16]) -> usize {
    before.iter().zip(after).filter(|(a, b)| a != b).count()
}

/// The mid-session half of the fix, which no capture can see: a chunk unloads after
/// `unload_grace_frames` and regenerates from noise, and before this batch that took the
/// player's edits with it.
#[test]
fn edits_survive_a_chunk_unload() {
    let gen = generator(SEED);
    let key = terrain_chunk();
    let journal = EditJournal::empty();
    let mut streamer = Streamer::new(gen.clone());
    streamer.set_journal(journal.clone());

    let mut world = World::new();
    world.attach_journal(journal.clone());
    let built = streamer.build_now(key);
    world.insert_local(key, built.tree);

    let p = key.origin() + IVec3::new(31, 40, 31);
    let was = world.get_block(p);
    assert!(world.set_block(p, block::GLOWSTONE));
    assert_ne!(was, block::GLOWSTONE, "pick a voxel the edit actually changes");

    assert!(world.remove_chunk(key));
    let again = streamer.build_now(key);
    world.insert_local(key, again.tree);
    assert_eq!(world.get_block(p), block::GLOWSTONE);
}

fn temp_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("voxelcraft-{name}.journal"))
}

/// A round trip through the file, including the two cases the arithmetic gets wrong:
/// **negative chunk coordinates** (`of_block` is an arithmetic shift, and a journal that used
/// division would put these one chunk off) and a **break**, which is an edit whose id is AIR
/// and which a "record what was placed" journal would drop.
///
/// It also pins `journal`'s private inverse of `dense_index`: save writes a world position
/// derived through it and load reads that position back through `dense_index` itself, so a
/// wrong inverse lands the block somewhere else and `apply` puts it there.
#[test]
fn a_saved_journal_reloads_to_the_same_edits() {
    let path = temp_path("roundtrip");
    let placed = [
        (IVec3::new(5, 130, 7), block::COBBLE),
        (IVec3::new(-1, 200, -1), block::PLANKS),
        (IVec3::new(-70, 64, -130), block::GLOWSTONE),
        (IVec3::new(63, 129, 63), block::AIR),
        (IVec3::new(1000, 300, -1000), block::COBBLE),
    ];
    let written = EditJournal::empty();
    for (p, id) in placed {
        written.record(p, id);
    }
    written.save(&path, SEED, SEA_LEVEL).expect("saved");
    let read = EditJournal::load(&path, SEED, SEA_LEVEL).expect("loaded");

    assert_eq!(read.len(), placed.len());
    assert_eq!(read.chunks(), written.chunks());
    for (p, id) in placed {
        let key = ChunkKey::of_block(p);
        let l = p & 63;
        // Every position here is a LOD-0 chunk, which is the path that reads neither the
        // generator nor the heights: they are the coarse aggregate's inputs.
        let mut dense = vec![block::AIR; VOL];
        let gen = generator(SEED);
        assert_eq!(
            read.apply(key, &mut dense, &gen, &[0i32; 64 * 64]),
            written.apply(key, &mut dense, &gen, &[0i32; 64 * 64])
        );
        assert_eq!(
            dense[dense_index(l.x as usize, l.y as usize, l.z as usize)],
            id,
            "edit at {p} came back in the wrong place"
        );
    }
    let _ = std::fs::remove_file(&path);
}

/// Last write wins, which is what makes replay independent of the order the map hands the
/// edits back -- a player who places a block and breaks it again leaves one entry, not two.
#[test]
fn the_last_write_at_a_position_is_the_one_kept() {
    let j = EditJournal::empty();
    let p = IVec3::new(3, 140, 4);
    j.record(p, block::COBBLE);
    j.record(p, block::PLANKS);
    j.record(p, block::AIR);
    assert_eq!(j.len(), 1);
    let mut dense = vec![block::COBBLE; VOL];
    j.apply(ChunkKey::of_block(p), &mut dense, &generator(SEED), &[0i32; 64 * 64]);
    let l = p & 63;
    assert_eq!(
        dense[dense_index(l.x as usize, l.y as usize, l.z as usize)],
        block::AIR
    );
}

/// The guard that matters most, because its failure is invisible. A journal replayed into a
/// world generated from another seed lands every edit on terrain that is not there, and the
/// capture of it still looks like a capture.
#[test]
fn a_journal_from_another_world_is_refused() {
    let path = temp_path("wrong-seed");
    let j = EditJournal::empty();
    j.record(IVec3::new(1, 140, 1), block::COBBLE);
    j.save(&path, SEED, SEA_LEVEL).expect("saved");

    assert!(EditJournal::load(&path, SEED + 1, SEA_LEVEL).is_err());
    assert!(EditJournal::load(&path, SEED, SEA_LEVEL - 1).is_err());
    assert!(EditJournal::load(&path, SEED, SEA_LEVEL).is_ok());
    let _ = std::fs::remove_file(&path);
}

/// A half-written file is the other way a journal can be silently wrong: the header still
/// parses, and the records it promises are not all there.
#[test]
fn a_truncated_journal_is_refused() {
    let path = temp_path("truncated");
    let j = EditJournal::empty();
    for i in 0..8 {
        j.record(IVec3::new(i, 140, 1), block::COBBLE);
    }
    j.save(&path, SEED, SEA_LEVEL).expect("saved");
    let mut bytes = std::fs::read(&path).expect("read");
    bytes.truncate(bytes.len() - 8);
    std::fs::write(&path, &bytes).expect("write");
    assert!(EditJournal::load(&path, SEED, SEA_LEVEL).is_err());

    std::fs::write(&path, b"not a journal at all").expect("write");
    assert!(EditJournal::load(&path, SEED, SEA_LEVEL).is_err());
    let _ = std::fs::remove_file(&path);
}

// ---------------------------------------------------------------------------------------
// Batch 30: the same deltas, aggregated into the coarse levels.
//
// Batch 29 skipped them on the grounds that "there is no honest answer to which of a LOD-2
// voxel's 512 blocks an edit sets". The answer it was looking for does not exist, and the
// question was the wrong one: a coarse voxel does not *take* an edit, it **stands for** the
// blocks under it, exactly as `coarse_trees` stands for a tree it cannot draw. What follows
// is the three rules in `EditJournal::apply`, one test each, plus the two properties that
// make them safe to ship -- order independence, and costing nothing where there are no edits.
// ---------------------------------------------------------------------------------------

/// The chunk at `lod` holding a block position.
fn key_of(lod: u8, p: IVec3) -> ChunkKey {
    ChunkKey::new(lod, p >> (6 + lod as i32))
}

/// Build one chunk the way the streamer does, and hand back what the journal changed.
fn build(gen: &WorldGen, j: &EditJournal, key: ChunkKey) -> (Vec<u16>, Vec<u16>, usize) {
    let mut dense = vec![0u16; VOL];
    let mut heights = Box::new([0i32; 64 * 64]);
    gen.generate(key, &mut dense, &mut heights);
    let before = dense.clone();
    let n = j.apply(key, &mut dense, gen, &heights);
    (before, dense, n)
}

/// Where a world position lands in a chunk's dense array at that chunk's stride.
fn voxel_of(key: ChunkKey, p: IVec3) -> usize {
    let v = (p - key.origin()) / key.voxel_size();
    dense_index(v.x as usize, v.y as usize, v.z as usize)
}

/// **The headline.** A structure standing in open air is there at every level, which is the
/// whole of "a structure no longer vanishes as you walk away from it".
///
/// Built above the terrain rather than on it so the claim is provable and not merely
/// observed: nothing the generator puts in those chunks can compete for the voxel, so
/// `filled` is 0 at every level and rule 1 reduces to "the edit wins". A tower on a hillside
/// is the same code with a terrain top to clear, and that is what the `edits` vantage renders.
#[test]
fn a_structure_in_open_air_reaches_every_coarse_level() {
    let gen = generator(SEED);
    let journal = EditJournal::empty();
    // Well above the tallest terrain this generator produces (295 internal) and well below
    // `WORLD_HEIGHT`, so every chunk below is pure air before the journal touches it.
    let foot = IVec3::new(20, 400, 20);
    let tower: Vec<IVec3> = (0..40).map(|dy| foot + IVec3::Y * dy).collect();
    for &p in &tower {
        journal.record(p, block::COBBLE);
    }

    for lod in 0..=4u8 {
        let key = key_of(lod, foot);
        let (before, after, n) = build(&gen, &journal, key);
        assert!(
            tower.iter().all(|&p| before[voxel_of(key, p)] == block::AIR),
            "lod {lod}: the fixture wants the tower standing in empty sky, and the generator \
             put terrain in it"
        );
        assert!(n > 0, "lod {lod} took none of the {} edits", tower.len());
        // Every voxel the tower passes through reads as the tower, top to bottom: at stride
        // 16 that is three voxels standing for forty blocks, which is the point.
        for &p in &tower {
            assert_eq!(
                after[voxel_of(key, p)],
                block::COBBLE,
                "lod {lod}: {p} is inside the tower and its voxel is empty"
            );
        }
    }
}

/// Rule 1's other half. An edit under the coarse surface is behind an opaque face, so it must
/// not repaint the voxel standing over it -- otherwise one block placed in a cave turns an
/// eight-block cube of hillside into planks, visible from across the world.
#[test]
fn an_edit_below_the_coarse_surface_does_not_repaint_the_voxel_over_it() {
    let gen = generator(SEED);
    // A LOD-1 chunk entirely below sea level, so `column_top` puts terrain or water in every
    // one of its voxels and each stands for a full 2x2x2.
    let key = ChunkKey::new(1, IVec3::new(0, 0, 0));
    let box_min = IVec3::new(20, 60, 20);
    assert_eq!(box_min % 2, IVec3::ZERO, "the fixture wants a voxel-aligned box");

    let buried = EditJournal::empty();
    buried.record(box_min, block::PLANKS);
    let (before, after, n) = build(&gen, &buried, key);
    let di = voxel_of(key, box_min);
    assert_ne!(before[di], block::AIR, "the fixture wants a filled voxel");
    assert_eq!(n, 0, "an edit under the surface changes nothing");
    assert_eq!(after[di], before[di]);

    // The top block of the same box is the terrain's own top here, so it wins the voxel.
    let top = EditJournal::empty();
    top.record(box_min + IVec3::Y, block::PLANKS);
    let (_, after, n) = build(&gen, &top, key);
    assert_eq!(n, 1);
    assert_eq!(after[di], block::PLANKS);
}

/// Rule 2. One block broken out of eight is not a hole, and pretending otherwise would punch
/// a two-block window through a hillside seen from a kilometre away.
#[test]
fn a_coarse_voxel_clears_only_when_every_block_it_stands_for_is_gone() {
    let gen = generator(SEED);
    let key = ChunkKey::new(1, IVec3::new(0, 0, 0));
    let box_min = IVec3::new(20, 60, 20);
    let di = voxel_of(key, box_min);
    // The eight fine blocks this one voxel stands for.
    let fine: Vec<IVec3> = (0..8)
        .map(|i| box_min + IVec3::new(i & 1, (i >> 1) & 1, (i >> 2) & 1))
        .collect();

    let partial = EditJournal::empty();
    for &p in &fine[..7] {
        partial.record(p, block::AIR);
    }
    let (before, after, n) = build(&gen, &partial, key);
    assert_ne!(before[di], block::AIR, "the fixture wants a filled voxel");
    assert_eq!(n, 0, "seven of eight is not a hole");
    assert_eq!(after[di], before[di]);

    let whole = EditJournal::empty();
    for &p in &fine {
        whole.record(p, block::AIR);
    }
    let (_, after, n) = build(&gen, &whole, key);
    assert_eq!(n, 1);
    assert_eq!(after[di], block::AIR);

    // And a block broken in mid-air above the surface clears nothing, which is the half of
    // rule 2 that a count of raw AIR edits would get wrong: eight of those would empty a
    // voxel whose ground nobody touched.
    let sky = EditJournal::empty();
    let above = IVec3::new(20, 400, 20);
    for i in 0..8 {
        sky.record(above + IVec3::new(i & 1, (i >> 1) & 1, (i >> 2) & 1), block::AIR);
    }
    let sky_key = key_of(1, above);
    let (before, after, n) = build(&gen, &sky, sky_key);
    assert_eq!(n, 0);
    assert_eq!(before, after);
}

/// The map hands its entries back in no particular order, so the aggregate has to pick the
/// same winner whichever way round it sees them. Without the total order in `Footprint::best`
/// this is a picture that changes between two runs of the same world -- and a capture that
/// differs run to run is one nothing can diff, which is the argument `save`'s sort makes.
#[test]
fn the_coarse_aggregate_does_not_depend_on_the_maps_order() {
    let gen = generator(SEED);
    let above = IVec3::new(20, 400, 20);
    // Four different ids inside one stride-4 voxel, at three different heights.
    let placed = [
        (above + IVec3::new(0, 0, 0), block::COBBLE),
        (above + IVec3::new(1, 2, 3), block::PLANKS),
        (above + IVec3::new(3, 2, 1), block::GLOWSTONE),
        (above + IVec3::new(2, 1, 2), block::SNOW),
    ];
    let key = key_of(2, above);

    let forward = EditJournal::empty();
    for (p, id) in placed {
        forward.record(p, id);
    }
    let backward = EditJournal::empty();
    for (p, id) in placed.iter().rev() {
        backward.record(*p, *id);
    }
    let (_, a, _) = build(&gen, &forward, key);
    let (_, b, _) = build(&gen, &backward, key);
    assert_eq!(a, b);
    // Topmost first, then lowest z, then lowest x. Two blocks share the top height, so z
    // decides: `(3, 2, 1)` beats `(1, 2, 3)` and the voxel reads glowstone.
    assert_eq!(a[voxel_of(key, above)], block::GLOWSTONE);
}

/// The cost claim, and the reason every capture ever taken is unaffected: a coarse chunk with
/// no edits under it comes out of `apply` byte for byte as the generator left it. Stated as a
/// test because `docs/errors.md` will not accept "it is free" from a flag -- only from a term
/// that is unreachable, and this is the shape that makes it unreachable.
#[test]
fn a_coarse_chunk_with_no_edits_under_it_is_untouched() {
    let gen = generator(SEED);
    let empty = EditJournal::empty();
    let elsewhere = EditJournal::empty();
    elsewhere.record(IVec3::new(-5000, 140, -5000), block::COBBLE);

    for lod in 1..=4u8 {
        let key = key_of(lod, IVec3::new(20, 140, 20));
        let (before, after, n) = build(&gen, &empty, key);
        assert_eq!((n, before.clone()), (0, after), "lod {lod}, empty journal");
        let (_, after, n) = build(&gen, &elsewhere, key);
        assert_eq!((n, before), (0, after), "lod {lod}, edits somewhere else");
    }
}

/// The other half of the fix, and the one no single chunk build can show: a coarse stand-in
/// interned *before* an edit has no route to it, because `apply` reads the journal once. The
/// journal is the only place that sees every edit, so it is what says which chunks are stale.
#[test]
fn an_edit_names_the_chunk_the_coarse_levels_have_to_rebuild() {
    let gen = generator(SEED);
    let key = terrain_chunk();
    let journal = EditJournal::empty();
    let mut streamer = Streamer::new(gen.clone());
    streamer.set_journal(journal.clone());
    let mut world = World::new();
    world.attach_journal(journal.clone());
    let built = streamer.build_now(key);
    world.insert_local(key, built.tree);
    assert!(journal.take_dirty().is_empty(), "building dirties nothing");

    let p = key.origin() + IVec3::new(31, 40, 31);
    assert!(world.set_block(p, block::GLOWSTONE));
    assert_eq!(journal.take_dirty(), vec![key]);
    assert!(
        journal.take_dirty().is_empty(),
        "drained, so the rebuild is asked for once and not every frame after"
    );

    // A redundant write is not an edit, so it does not dirty anything either -- `set_block`
    // records past both of its early returns and this follows it for free.
    assert!(world.set_block(p, block::GLOWSTONE));
    assert!(journal.take_dirty().is_empty());
}

// ---------------------------------------------------------------------------------------
// Batch 32: the player in the header.
//
// The pixel-level version of this claim is one line of `docs/persistence.md`: a capture at a
// named camera, saved, and re-rendered with `--resume`, which has to come back **bit
// identical**. What follows is the version that runs in a millisecond and says which field
// disagreed -- and the four refusals, which are the half no image can show, because every one
// of them is a file that would otherwise render a frame that looks exactly like a frame.
// ---------------------------------------------------------------------------------------

/// A player who is nowhere near the origin on any axis, at angles that are not round
/// numbers, so a field silently swapped with its neighbour cannot pass.
fn a_player() -> PlayerState {
    PlayerState::new([-1234.5, 187.25, 9876.125], 2.318_7, -0.4213, 7, true)
}

/// Every field, through the file, exactly. `f32` equality on purpose: this is a byte-for-byte
/// format and a value that came back *nearly* right is a format that has been rounded
/// somewhere it should not have been.
#[test]
fn the_player_round_trips_through_the_file() {
    let path = temp_path("player-roundtrip");
    let written = EditJournal::empty();
    written.record(IVec3::new(5, 130, 7), block::COBBLE);
    written.set_player(a_player());
    written.save(&path, SEED, SEA_LEVEL).expect("saved");

    let read = EditJournal::load(&path, SEED, SEA_LEVEL).expect("loaded");
    assert_eq!(read.player(), Some(a_player()));
    // Written twice, so the bytes are the bytes: the same argument `save`'s sort makes about
    // the records, one field up.
    let again = temp_path("player-roundtrip-2");
    read.save(&again, SEED, SEA_LEVEL).expect("saved");
    assert_eq!(
        std::fs::read(&path).unwrap(),
        std::fs::read(&again).unwrap()
    );
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&again);
}

/// A journal nobody stamped says so, and does not invent a spawn point.
///
/// "The file does not say" and "the file says the spawn column" are different claims, and
/// only the first may fall back to `spawn_position`. This is also the case every journal
/// batches 29 and 30 ever wrote falls into.
#[test]
fn a_journal_with_no_player_says_so_rather_than_guessing() {
    let path = temp_path("no-player");
    let j = EditJournal::empty();
    j.record(IVec3::new(5, 130, 7), block::COBBLE);
    j.save(&path, SEED, SEA_LEVEL).expect("saved");
    let read = EditJournal::load(&path, SEED, SEA_LEVEL).expect("loaded");
    assert_eq!(read.player(), None);
    assert_eq!(read.len(), 1);
    let _ = std::fs::remove_file(&path);
}

/// The records are where the header says they are, with a player block in front of them.
///
/// The offset arithmetic in `load` is the one thing the player block can break in a way that
/// still parses -- a length check that forgot the block, or a slice that started in the
/// middle of it, would hand `bytemuck` 32 bytes of player and call them two edits. So: the
/// same edits, saved twice, once with a player and once without, have to replay identically.
#[test]
fn the_player_block_does_not_disturb_the_records() {
    let placed = [
        (IVec3::new(5, 130, 7), block::COBBLE),
        (IVec3::new(-70, 64, -130), block::GLOWSTONE),
        (IVec3::new(63, 129, 63), block::AIR),
    ];
    let mut replayed = Vec::new();
    for stamped in [false, true] {
        let path = temp_path(if stamped { "with-player" } else { "without-player" });
        let j = EditJournal::empty();
        for (p, id) in placed {
            j.record(p, id);
        }
        if stamped {
            j.set_player(a_player());
        }
        j.save(&path, SEED, SEA_LEVEL).expect("saved");
        let read = EditJournal::load(&path, SEED, SEA_LEVEL).expect("loaded");
        let key = ChunkKey::of_block(placed[0].0);
        let mut dense = vec![block::STONE; VOL];
        let n = read.apply(key, &mut dense, &generator(SEED), &[0i32; 64 * 64]);
        replayed.push((n, dense));
        let _ = std::fs::remove_file(&path);
    }
    assert_eq!(replayed[0].0, replayed[1].0);
    assert!(
        replayed[0].1 == replayed[1].1,
        "the player block moved the edit records"
    );
}

/// A version-1 file still loads, and the reason it can is that there is no compatibility
/// branch anywhere: it wrote its spare word as zero, and zero is what "no player block"
/// looks like in a version-2 file. Two versions, one reader.
#[test]
fn a_version_1_journal_still_loads() {
    let path = temp_path("v1");
    let j = EditJournal::empty();
    j.record(IVec3::new(5, 130, 7), block::COBBLE);
    j.save(&path, SEED, SEA_LEVEL).expect("saved");
    let mut bytes = std::fs::read(&path).expect("read");
    // `version` is the second word of the header, and the first is the magic.
    bytes[4..8].copy_from_slice(&1u32.to_le_bytes());
    std::fs::write(&path, &bytes).expect("write");

    let read = EditJournal::load(&path, SEED, SEA_LEVEL).expect("a version-1 journal must load");
    assert_eq!(read.len(), 1);
    assert_eq!(read.player(), None);

    // And a version this build has never heard of is still refused, which is the half that
    // makes the acceptance above a rule rather than an absence of checking.
    bytes[4..8].copy_from_slice(&99u32.to_le_bytes());
    std::fs::write(&path, &bytes).expect("write");
    assert!(EditJournal::load(&path, SEED, SEA_LEVEL).is_err());
    let _ = std::fs::remove_file(&path);
}

/// Two player blocks a file must not be allowed to hand back, both for the reason the seed
/// guard exists: neither announces itself in a picture.
///
/// A hotbar slot past the end of `block::HOTBAR` panics in `selected_block`, three call sites
/// from here and only when the player right-clicks. A non-finite position renders a black
/// frame, and a black frame is a frame.
#[test]
fn a_player_block_the_build_cannot_honour_is_refused() {
    let path = temp_path("bad-player");
    let slots = block::HOTBAR.len() as u32;
    for bad in [
        PlayerState::new([0.0, 140.0, 0.0], 0.0, 0.0, slots, false),
        PlayerState::new([f32::NAN, 140.0, 0.0], 0.0, 0.0, 0, false),
        PlayerState::new([0.0, f32::INFINITY, 0.0], 0.0, 0.0, 0, false),
        PlayerState::new([0.0, 140.0, 0.0], f32::NAN, 0.0, 0, false),
        PlayerState::new([0.0, 140.0, 0.0], 0.0, f32::NAN, 0, false),
    ] {
        let j = EditJournal::empty();
        j.set_player(bad);
        j.save(&path, SEED, SEA_LEVEL).expect("saved");
        assert!(
            EditJournal::load(&path, SEED, SEA_LEVEL).is_err(),
            "{bad:?} loaded when it should have been refused"
        );
    }
    // The last slot is fine, which is what says the check above is a bound and not a ban.
    let j = EditJournal::empty();
    j.set_player(PlayerState::new(
        [0.0, 140.0, 0.0],
        0.0,
        0.0,
        slots - 1,
        false,
    ));
    j.save(&path, SEED, SEA_LEVEL).expect("saved");
    assert!(EditJournal::load(&path, SEED, SEA_LEVEL).is_ok());
    let _ = std::fs::remove_file(&path);
}

/// The trip the game actually makes: a `Player`, through the file, back into a `Player`.
///
/// The two ends are `Player::state` and `Player::restore`, and the field this pins that the
/// file round trip above cannot is the pair of unit conversions -- `hotbar` crosses
/// `usize`/`u32` and `fly` crosses `bool`/`u32`, and a swapped pair of those is a save file
/// that puts you on the wrong block, walking.
#[test]
fn a_player_survives_the_trip_out_and_back() {
    let path = temp_path("player-struct");
    let mut before = Player::new(glam::Vec3::new(-1234.5, 187.25, 9876.125));
    before.yaw = 2.318_7;
    before.pitch = -0.4213;
    before.hotbar = 7;
    before.fly = false;
    before.vel = glam::Vec3::new(3.0, -9.0, 1.0);

    let j = EditJournal::empty();
    j.set_player(before.state());
    j.save(&path, SEED, SEA_LEVEL).expect("saved");
    let read = EditJournal::load(&path, SEED, SEA_LEVEL).expect("loaded");

    let mut after = Player::new(glam::Vec3::ZERO);
    after.restore(read.player().expect("the file carries a player"));
    assert_eq!(after.pos, before.pos);
    assert_eq!(after.yaw, before.yaw);
    assert_eq!(after.pitch, before.pitch);
    assert_eq!(after.hotbar, before.hotbar);
    assert_eq!(after.fly, before.fly);
    // Velocity is deliberately not in the file: a save says where you are, not how fast you
    // were falling, and a restored fall would drop the player through a world that has not
    // streamed in yet.
    assert_eq!(after.vel, glam::Vec3::ZERO);
    let _ = std::fs::remove_file(&path);
}

// ---------------------------------------------------------------------------------------
// Batch 33: a world on disk, and a file that grows as the edits land.
//
// Batches 29 to 32 wrote the journal once, at the end of the mode, so a session killed
// rather than closed threw all of it away. `--edits PATH` is the world-shaped flag -- load
// it if it is there, append every edit as it lands, write it back compacted on the way out
// -- and the claim the tests below stand on is **the file on disk is the world at every
// instant**, not only after a clean exit.
//
// The pixel-level version is the table in `docs/persistence.md`: the same 393-block hut,
// left behind by a killed session, replays to a frame identical to the one that built it
// -- and the frame a killed session used to leave is 67,983 pixels away from that.
// ---------------------------------------------------------------------------------------

/// A config that treats `path` as a world on disk, and asks for nothing else. The seed and
/// sea level come from `Config::default`, which is where this file's `SEED` also comes from.
fn world_on_disk(path: &std::path::Path) -> Config {
    Config {
        edits: Some(path.display().to_string()),
        ..Default::default()
    }
}

/// The header's `count`: the fifth word of the file, and the one the append log rewrites in
/// place. Read here from the literal offset rather than through the type, because a test
/// that shares the arithmetic it is checking checks nothing.
fn count_of(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes[16..20].try_into().unwrap())
}

fn with_count(bytes: &[u8], n: u32) -> Vec<u8> {
    let mut out = bytes.to_vec();
    out[16..20].copy_from_slice(&n.to_le_bytes());
    out
}

/// The records in the file, from its length, which is the quantity the count is a claim
/// about. The header is 24 bytes, a player block is 32 more, a bar block 24 more, and an edit
/// is 16.
///
/// **A deliberate transliteration of `journal::extras_bytes`, not a call to it**, which is
/// the same bargain four other test files in this tree strike: a helper that read the real
/// arithmetic could not notice the real arithmetic changing. Batch 34 is what that is worth
/// -- this said 24 + player and the bar block made it wrong by a record and a half, which is
/// a failing assert rather than a silently different number.
fn records_in(bytes: &[u8]) -> usize {
    let flags = u32::from_le_bytes(bytes[20..24].try_into().unwrap());
    let extras = if flags & 1 != 0 { 32 } else { 0 } + if flags & 4 != 0 { 24 } else { 0 };
    (bytes.len() - 24 - extras) / 16
}

/// A path with no file at it, which is what a world nobody has played looks like.
fn unplayed(name: &str) -> std::path::PathBuf {
    let p = temp_path(name);
    let _ = std::fs::remove_file(&p);
    p
}

/// The flag's own premise: a path that is not there yet is the ordinary first run.
///
/// **And the half that makes it safe is that `--load-edits` still refuses one.** The two
/// flags reach the same loader and differ only in this, because they answer different
/// questions: `--load-edits` is a measurement input, where a missing file is a typo and a
/// typo renders an unedited world that looks exactly like a passing one; `--edits` is a
/// world, and there is no other way to start one.
#[test]
fn edits_on_a_path_nobody_has_played_starts_a_new_world() {
    let path = unplayed("new-world");
    let j = journal::open(&world_on_disk(&path)).expect("a missing world file is a new world");
    assert!(j.is_empty());
    assert!(
        path.exists(),
        "--edits has to create the file it was handed, or the first kill has nothing to have \
         appended to"
    );
    assert_eq!(count_of(&std::fs::read(&path).unwrap()), 0);

    let missing = unplayed("new-world-strict");
    let strict = Config {
        load_edits: Some(missing.display().to_string()),
        ..Default::default()
    };
    assert!(
        journal::open(&strict).is_err(),
        "--load-edits on a missing file must stay an error"
    );
    let _ = std::fs::remove_file(&path);
}

/// **The batch, in one test.** Edits recorded and the process then simply ending -- no
/// `save_if_asked`, no clean exit, nothing written at the end -- and every block still
/// there when the file is read back.
///
/// Before batch 33 this test could not be written: the only writer ran at the end of the
/// mode, so "the session did not reach the end" and "the file is empty" were the same
/// sentence.
#[test]
fn a_killed_session_keeps_the_blocks_it_placed() {
    let path = unplayed("killed");
    let placed = [
        (IVec3::new(5, 130, 7), block::COBBLE),
        (IVec3::new(-1, 200, -1), block::PLANKS),
        (IVec3::new(-70, 64, -130), block::GLOWSTONE),
        (IVec3::new(63, 129, 63), block::AIR),
    ];
    {
        let j = journal::open(&world_on_disk(&path)).expect("new world");
        for (p, id) in placed {
            j.record(p, id);
        }
        // And here the process dies. Deliberately no `journal::save_if_asked`.
    }
    let read = EditJournal::load(&path, SEED, SEA_LEVEL).expect("the file the kill left behind");
    assert_eq!(read.len(), placed.len());
    let gen = generator(SEED);
    for (p, id) in placed {
        let mut dense = vec![block::STONE; VOL];
        read.apply(ChunkKey::of_block(p), &mut dense, &gen, &[0i32; 64 * 64]);
        let l = p & 63;
        assert_eq!(
            dense[dense_index(l.x as usize, l.y as usize, l.z as usize)],
            id,
            "the edit at {p} did not survive the kill"
        );
    }
    let _ = std::fs::remove_file(&path);
}

/// The kill landing *between* the record and the count, which is the one window the append
/// log has and the reason it writes them in that order.
///
/// The file then holds whole records the header does not claim. Recovering them is the
/// feature: a reader that trusted the count would drop exactly the edits this exists to
/// keep. Doctored here rather than raced, because a two-instruction window is not a thing a
/// test can hit on purpose.
#[test]
fn the_records_a_kill_left_uncounted_are_recovered() {
    let path = unplayed("uncounted");
    {
        let j = journal::open(&world_on_disk(&path)).expect("new world");
        for i in 0..20 {
            j.record(IVec3::new(i, 140, 1), block::COBBLE);
        }
    }
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(count_of(&bytes), 20);
    assert_eq!(records_in(&bytes), 20);
    // The last five never reached the count.
    std::fs::write(&path, with_count(&bytes, 15)).unwrap();
    let read = EditJournal::load(&path, SEED, SEA_LEVEL).expect("loaded");
    assert_eq!(read.len(), 20, "five uncounted edits were thrown away");
    let _ = std::fs::remove_file(&path);
}

/// The other shape of the same window: killed *inside* a record's own 16 bytes.
///
/// A partial record cannot be replayed and is dropped, and the whole ones in front of it are
/// kept -- which is the difference between losing one block and losing the session.
#[test]
fn a_half_written_edit_at_the_end_is_dropped_and_the_rest_kept() {
    let path = unplayed("torn");
    {
        let j = journal::open(&world_on_disk(&path)).expect("new world");
        for i in 0..20 {
            j.record(IVec3::new(i, 140, 1), block::COBBLE);
        }
    }
    let bytes = std::fs::read(&path).unwrap();
    let mut torn = with_count(&bytes, 15);
    torn.extend_from_slice(&[0xAB; 7]);
    std::fs::write(&path, &torn).unwrap();
    let read = EditJournal::load(&path, SEED, SEA_LEVEL).expect("loaded");
    assert_eq!(read.len(), 20);

    // And the next session appends *after* the whole records, not on top of the torn tail:
    // opening the log truncates it, which is what keeps every later record in phase.
    {
        let j = journal::open(&world_on_disk(&path)).expect("reopened");
        assert_eq!(j.len(), 20);
        j.record(IVec3::new(99, 140, 1), block::PLANKS);
    }
    let after = std::fs::read(&path).unwrap();
    assert_eq!(count_of(&after), 21);
    assert_eq!(records_in(&after), 21);
    assert_eq!(EditJournal::load(&path, SEED, SEA_LEVEL).unwrap().len(), 21);
    let _ = std::fs::remove_file(&path);
}

/// **The relaxation is scoped by the flag bit, and this is the test that says so.**
///
/// A file the append log never touched is held to exactly the check batch 29 shipped: a
/// length that disagrees with the count is a corrupt file and is refused. Without this the
/// batch would have quietly turned the truncation guard off for every journal in the tree --
/// and the compacted files are what every A/B in `docs/persistence.md` is measured on.
#[test]
fn a_file_that_never_appended_is_still_held_to_its_count() {
    let path = temp_path("no-append-bit");
    let j = EditJournal::empty();
    for i in 0..8 {
        j.record(IVec3::new(i, 140, 1), block::COBBLE);
    }
    j.save(&path, SEED, SEA_LEVEL).expect("saved");
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(
        u32::from_le_bytes(bytes[20..24].try_into().unwrap()) & 2,
        0,
        "`save` writes a compacted file and must not claim to be an append log"
    );

    let mut longer = bytes.clone();
    longer.extend_from_slice(&[0u8; 16]);
    std::fs::write(&path, &longer).unwrap();
    assert!(
        EditJournal::load(&path, SEED, SEA_LEVEL).is_err(),
        "a compacted file longer than its count is corrupt, not recoverable"
    );

    // The same 16 bytes, on a file that *is* an append log, are an edit. One byte of `flags`
    // is the whole difference, which is what makes the refusal above a rule.
    let mut appended = with_count(&bytes, count_of(&bytes));
    appended[20..24].copy_from_slice(&2u32.to_le_bytes());
    appended.extend_from_slice(&[0u8; 16]);
    std::fs::write(&path, &appended).unwrap();
    assert_eq!(
        EditJournal::load(&path, SEED, SEA_LEVEL)
            .expect("an append log may carry an uncounted record")
            .len(),
        9
    );
    let _ = std::fs::remove_file(&path);
}

/// The two writers agree about the world, and disagree about the bytes on purpose.
///
/// The log is a **log**: place, break, place again is three records at one position, in the
/// order they happened, and the loader's last-write-wins is that order. The exit save is a
/// **compaction**: one record per position, sorted, so the same world writes the same file.
/// Both have to replay to the same blocks, and the second has to be the smaller file.
#[test]
fn the_log_and_the_exit_save_agree_about_the_world() {
    let path = unplayed("log-vs-save");
    let p = IVec3::new(3, 140, 4);
    let j = journal::open(&world_on_disk(&path)).expect("new world");
    j.record(p, block::COBBLE);
    j.record(p, block::PLANKS);
    j.record(p, block::AIR);
    j.record(IVec3::new(4, 140, 4), block::GLOWSTONE);

    let logged = std::fs::read(&path).unwrap();
    assert_eq!(records_in(&logged), 4, "the log keeps every edit, in order");
    let from_log = EditJournal::load(&path, SEED, SEA_LEVEL).expect("loaded");
    assert_eq!(from_log.len(), 2, "two positions, whatever the record count");

    journal::save_if_asked(&world_on_disk(&path), &j, a_player(), block::HOTBAR).expect("saved");
    let saved = std::fs::read(&path).unwrap();
    assert_eq!(records_in(&saved), 2, "the exit save compacts");
    let from_save = EditJournal::load(&path, SEED, SEA_LEVEL).expect("loaded");

    let gen = generator(SEED);
    let key = ChunkKey::of_block(p);
    let mut a = vec![block::STONE; VOL];
    let mut b = vec![block::STONE; VOL];
    from_log.apply(key, &mut a, &gen, &[0i32; 64 * 64]);
    from_save.apply(key, &mut b, &gen, &[0i32; 64 * 64]);
    assert!(a == b, "the log and the compaction replay different worlds");
    let l = p & 63;
    assert_eq!(
        a[dense_index(l.x as usize, l.y as usize, l.z as usize)],
        block::AIR,
        "the last write at a position has to win through the file's own order"
    );
    let _ = std::fs::remove_file(&path);
}

/// `--no-exit-save` is what lets a fixture see any of this, so it has to actually do
/// nothing -- and, in a pair with it, the ordinary exit has to actually write.
///
/// `lessons.md`'s rule: a no-op claim standing alone cannot be told apart from a claim that
/// was never wired up, so both halves read the same file from the same session.
#[test]
fn no_exit_save_leaves_the_file_exactly_as_the_log_left_it() {
    let path = unplayed("no-exit-save");
    let j = journal::open(&world_on_disk(&path)).expect("new world");
    j.record(IVec3::new(3, 140, 4), block::COBBLE);
    j.record(IVec3::new(3, 140, 4), block::PLANKS);
    let logged = std::fs::read(&path).unwrap();

    let killed = Config {
        no_exit_save: true,
        ..world_on_disk(&path)
    };
    journal::save_if_asked(&killed, &j, a_player(), block::HOTBAR).expect("no-op");
    assert_eq!(
        std::fs::read(&path).unwrap(),
        logged,
        "--no-exit-save wrote something"
    );
    assert_eq!(
        EditJournal::load(&path, SEED, SEA_LEVEL).unwrap().player(),
        None,
        "a killed session's file carries no player, because nothing stamped one"
    );

    journal::save_if_asked(&world_on_disk(&path), &j, a_player(), block::HOTBAR).expect("saved");
    let closed = std::fs::read(&path).unwrap();
    assert_ne!(closed, logged, "the ordinary exit wrote nothing either");
    assert_eq!(
        EditJournal::load(&path, SEED, SEA_LEVEL).unwrap().player(),
        Some(a_player())
    );
    let _ = std::fs::remove_file(&path);
}

/// `--edits` is both halves at one path, so sharing it with either half is refused rather
/// than resolved by a precedence rule nobody could read off the command line.
#[test]
fn edits_will_not_share_its_path_with_the_flags_it_replaces() {
    let path = unplayed("clash");
    let other = temp_path("clash-other");
    for clash in [
        Config {
            load_edits: Some(other.display().to_string()),
            ..world_on_disk(&path)
        },
        Config {
            save_edits: Some(other.display().to_string()),
            ..world_on_disk(&path)
        },
    ] {
        assert!(journal::open(&clash).is_err());
    }
    assert!(
        !path.exists(),
        "a refused start must not create the world file it refused"
    );
    let _ = std::fs::remove_file(&path);
}

// ---------------------------------------------------------------------------------------
// Batch 34: the bar in the file. The claim is that a world comes back with the bar it was
// left with, and the negative half of it is that a file written before this batch comes back
// with the bar it has always had -- which is the same reader, taking a different branch of
// one `flags` word and no branch at all on the version number.
// ---------------------------------------------------------------------------------------

/// A bar nothing would produce by accident: every slot different from `block::HOTBAR`'s, and
/// every one of them legal.
fn a_bar() -> block::Bar {
    [
        block::SNOW,
        block::PODZOL,
        block::GRAVEL,
        block::BEDROCK,
        block::LICHEN,
        block::PINE_LEAVES,
        block::MEADOW,
        block::STONE,
        block::DIRT,
        block::GRASS,
    ]
}

/// The bar through the file, byte for byte, **in a pair with the claim that says the file is
/// where it went**. A test that only checked the round trip could not tell a working format
/// from a journal that had quietly kept the value in memory.
#[test]
fn the_bar_round_trips_through_the_file() {
    let path = temp_path("bar-roundtrip");
    let written = EditJournal::empty();
    written.record(IVec3::new(5, 130, 7), block::COBBLE);
    written.set_player(a_player());
    written.set_bar(a_bar());
    written.save(&path, SEED, SEA_LEVEL).expect("saved");

    let read = EditJournal::load(&path, SEED, SEA_LEVEL).expect("loaded");
    assert_eq!(read.bar(), Some(a_bar()), "the bar did not survive the file");
    assert_eq!(
        read.player(),
        Some(a_player()),
        "the bar block displaced the player block"
    );
    assert_eq!(read.len(), 1, "the bar block displaced the records");
    // The positive claim's partner: the bytes really are 24 longer, and the records really
    // do start after them. `records_in` is the transliterated arithmetic, so this reads the
    // quantity from the file's length rather than from the loader that just parsed it.
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(records_in(&bytes), 1, "the records moved without the reader");
    assert_eq!(
        u32::from_le_bytes(bytes[20..24].try_into().unwrap()) & 4,
        4,
        "FLAG_BAR is what says the block is there"
    );
}

/// **A file with a player and no bar is a version-2 file, and it still loads.** This is the
/// claim `VERSION`'s comment makes -- three versions, one reader, no branch on the number --
/// and it is checked by building the older layout by hand rather than by finding one, because
/// no file in the tree is older than the tree.
#[test]
fn a_file_with_no_bar_block_loads_and_means_the_default_bar() {
    let path = temp_path("bar-absent");
    let j = EditJournal::empty();
    j.record(IVec3::new(5, 130, 7), block::COBBLE);
    j.set_player(a_player());
    j.save(&path, SEED, SEA_LEVEL).expect("saved");
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(
        u32::from_le_bytes(bytes[20..24].try_into().unwrap()) & 4,
        0,
        "a journal nobody gave a bar must not claim one"
    );

    let read = EditJournal::load(&path, SEED, SEA_LEVEL).expect("loaded");
    assert_eq!(read.bar(), None, "the file says nothing about a bar");
    assert_eq!(read.player(), Some(a_player()), "and everything about a player");
    // And "says nothing" resolves to the bar every build has always had, which is the one
    // line that keeps every pre-34 capture identical.
    assert_eq!(journal::bar_for(&read), block::HOTBAR);
}

/// Every block a bar may not hold, refused at the file's edge with the reason.
///
/// **The list is `block::bar_slot_refusal`'s and the argument is `render::FLAG_FOLIAGE`'s**:
/// the `const` assert beside that flag can only speak for the compile-time array, and a bar
/// that arrives from a file is the case it cannot see. A `TALL_GRASS` slot loading here would
/// be a tuft placed into a `--no-foliage` build that was compiled unable to draw one.
#[test]
fn a_bar_block_the_build_cannot_honour_is_refused() {
    let path = temp_path("bad-bar");
    for bad in [
        block::TALL_GRASS,
        block::WATER,
        block::AIR,
        block::BLOCK_COUNT as block::BlockId,
        9999,
    ] {
        let mut bar = block::HOTBAR;
        bar[3] = bad;
        let j = EditJournal::empty();
        j.set_bar(bar);
        j.save(&path, SEED, SEA_LEVEL).expect("saved");
        assert!(
            EditJournal::load(&path, SEED, SEA_LEVEL).is_err(),
            "block {bad} loaded into a bar slot when it should have been refused"
        );
    }
    // The negative test of the test: the same slot with a legal block loads, which is what
    // says the check above is a rule about blocks and not a rule about slot 3.
    let mut bar = block::HOTBAR;
    bar[3] = block::MEADOW;
    let j = EditJournal::empty();
    j.set_bar(bar);
    j.save(&path, SEED, SEA_LEVEL).expect("saved");
    assert_eq!(
        EditJournal::load(&path, SEED, SEA_LEVEL)
            .expect("a legal bar must load")
            .bar(),
        Some(bar)
    );
}

/// `--bar`, through the one function that parses it, including every way it can be wrong.
///
/// Refused as a whole rather than clamped, which is the one place this flag deliberately
/// differs from `--hotbar`: a slot number out of range has a nearest legal answer and a block
/// id does not.
#[test]
fn the_bar_flag_is_refused_as_a_whole_or_not_at_all() {
    let path = unplayed("bar-flag");
    let with = |spec: &str| Config {
        bar: Some(spec.to_string()),
        ..world_on_disk(&path)
    };
    for bad in [
        "1,2,3",                          // too few
        "1,2,3,4,5,6,7,8,9,10,11",        // too many
        "1,2,3,4,5,6,7,8,9,dirt",         // not a number
        "18,2,3,4,5,6,7,8,9,10",          // tall grass
        "13,2,3,4,5,6,7,8,9,10",          // water
        "0,2,3,4,5,6,7,8,9,10",           // air
    ] {
        let _ = std::fs::remove_file(&path);
        assert!(
            journal::open(&with(bad)).is_err(),
            "--bar {bad:?} was accepted"
        );
    }
    // **Derived and not a literal, because the literal went stale the first time it could.**
    // This case read `"20,..."` until batch 59 appended three emissive blocks, at which
    // point 20 became `AMBER_LAMP` and the assertion was that a perfectly legal bar is
    // refused. Every other entry above names a rule that will outlive the table; this one
    // names the end of it, so it has to be computed from where the end actually is.
    let past_the_end = format!("{},2,3,4,5,6,7,8,9,10", block::BLOCK_COUNT);
    let _ = std::fs::remove_file(&path);
    assert!(
        journal::open(&with(&past_the_end)).is_err(),
        "--bar {past_the_end:?} names no block and was accepted"
    );
    let _ = std::fs::remove_file(&path);
    let j = journal::open(&with("14,15,5,12,17,16,19,1,2,3")).expect("a legal bar");
    assert_eq!(j.bar(), Some(a_bar()), "--bar has to reach the journal");
    assert_eq!(journal::bar_for(&j), a_bar());
}

/// `--bar` against a world that already carries one is refused rather than silently losing.
///
/// The same shape as `--edits` alongside `--load-edits`: two readable answers, and the one a
/// reader would have to guess is the one this module refuses to have.
#[test]
fn the_bar_flag_and_a_world_that_already_has_a_bar_is_refused() {
    let path = unplayed("bar-conflict");
    let j = EditJournal::empty();
    j.set_bar(a_bar());
    j.save(&path, SEED, SEA_LEVEL).expect("saved");

    // Without the flag the world's own bar loads, which is the ordinary case.
    let played = journal::open(&world_on_disk(&path)).expect("loaded");
    assert_eq!(played.bar(), Some(a_bar()));

    let clashing = Config {
        bar: Some("1,2,3,4,5,6,7,8,9,10".into()),
        ..world_on_disk(&path)
    };
    assert!(
        journal::open(&clashing).is_err(),
        "--bar has to lose loudly or win loudly, not quietly"
    );
}

/// Picking applies the identical rule to the identical list, which is the half of batch 34
/// that has nothing to do with files: the world is full of blocks a bar may not hold, and
/// the crosshair can be pointed at any of them.
#[test]
fn picking_refuses_exactly_what_a_file_is_refused() {
    let mut p = Player::new(glam::Vec3::new(0.0, 140.0, 0.0));
    p.hotbar = 2;
    let was = p.bar[2];
    for bad in [block::TALL_GRASS, block::WATER, block::AIR] {
        assert!(!p.pick_into_bar(bad), "block {bad} was picked up");
        assert_eq!(p.bar[2], was, "a refused pick still moved the slot");
    }
    assert!(p.pick_into_bar(block::MEADOW), "a legal block must pick up");
    assert_eq!(p.bar[2], block::MEADOW);
    assert_eq!(p.selected_block(), block::MEADOW, "and be what gets placed");
}



