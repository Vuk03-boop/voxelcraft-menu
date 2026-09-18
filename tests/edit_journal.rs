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

fn terrain_chunk() -> ChunkKey {
    ChunkKey::new(0, IVec3::new(0, SEA_LEVEL / 64, 0))
}

fn local_of(i: usize) -> (usize, usize, usize) {
    (i & 63, (i >> 12) & 63, (i >> 6) & 63)
}

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

    let mut out = Vec::new();
    for (list, id) in [(&solid, block::AIR), (&air, block::COBBLE)] {
        let step = (list.len() / 40).max(1);
        out.extend(list.iter().step_by(step).take(40).map(|&i| (at(i), id)));
    }

    out.push((at(solid[solid.len() - 1]), block::GLOWSTONE));
    out
}

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

#[test]
fn a_replayed_chunk_matches_one_edited_live() {
    let gen = generator(SEED);
    let key = terrain_chunk();
    let o = key.origin();

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

fn journal_count(before: &[u16], after: &[u16]) -> usize {
    before.iter().zip(after).filter(|(a, b)| a != b).count()
}

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

fn key_of(lod: u8, p: IVec3) -> ChunkKey {
    ChunkKey::new(lod, p >> (6 + lod as i32))
}

fn build(gen: &WorldGen, j: &EditJournal, key: ChunkKey) -> (Vec<u16>, Vec<u16>, usize) {
    let mut dense = vec![0u16; VOL];
    let mut heights = Box::new([0i32; 64 * 64]);
    gen.generate(key, &mut dense, &mut heights);
    let before = dense.clone();
    let n = j.apply(key, &mut dense, gen, &heights);
    (before, dense, n)
}

fn voxel_of(key: ChunkKey, p: IVec3) -> usize {
    let v = (p - key.origin()) / key.voxel_size();
    dense_index(v.x as usize, v.y as usize, v.z as usize)
}

#[test]
fn a_structure_in_open_air_reaches_every_coarse_level() {
    let gen = generator(SEED);
    let journal = EditJournal::empty();

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

        for &p in &tower {
            assert_eq!(
                after[voxel_of(key, p)],
                block::COBBLE,
                "lod {lod}: {p} is inside the tower and its voxel is empty"
            );
        }
    }
}

#[test]
fn an_edit_below_the_coarse_surface_does_not_repaint_the_voxel_over_it() {
    let gen = generator(SEED);

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

    let top = EditJournal::empty();
    top.record(box_min + IVec3::Y, block::PLANKS);
    let (_, after, n) = build(&gen, &top, key);
    assert_eq!(n, 1);
    assert_eq!(after[di], block::PLANKS);
}

#[test]
fn a_coarse_voxel_clears_only_when_every_block_it_stands_for_is_gone() {
    let gen = generator(SEED);
    let key = ChunkKey::new(1, IVec3::new(0, 0, 0));
    let box_min = IVec3::new(20, 60, 20);
    let di = voxel_of(key, box_min);

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

#[test]
fn the_coarse_aggregate_does_not_depend_on_the_maps_order() {
    let gen = generator(SEED);
    let above = IVec3::new(20, 400, 20);

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

    assert_eq!(a[voxel_of(key, above)], block::GLOWSTONE);
}

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

    assert!(world.set_block(p, block::GLOWSTONE));
    assert!(journal.take_dirty().is_empty());
}

fn a_player() -> PlayerState {
    PlayerState::new([-1234.5, 187.25, 9876.125], 2.318_7, -0.4213, 7, true)
}

#[test]
fn the_player_round_trips_through_the_file() {
    let path = temp_path("player-roundtrip");
    let written = EditJournal::empty();
    written.record(IVec3::new(5, 130, 7), block::COBBLE);
    written.set_player(a_player());
    written.save(&path, SEED, SEA_LEVEL).expect("saved");

    let read = EditJournal::load(&path, SEED, SEA_LEVEL).expect("loaded");
    assert_eq!(read.player(), Some(a_player()));

    let again = temp_path("player-roundtrip-2");
    read.save(&again, SEED, SEA_LEVEL).expect("saved");
    assert_eq!(
        std::fs::read(&path).unwrap(),
        std::fs::read(&again).unwrap()
    );
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&again);
}

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

#[test]
fn a_version_1_journal_still_loads() {
    let path = temp_path("v1");
    let j = EditJournal::empty();
    j.record(IVec3::new(5, 130, 7), block::COBBLE);
    j.save(&path, SEED, SEA_LEVEL).expect("saved");
    let mut bytes = std::fs::read(&path).expect("read");

    bytes[4..8].copy_from_slice(&1u32.to_le_bytes());
    std::fs::write(&path, &bytes).expect("write");

    let read = EditJournal::load(&path, SEED, SEA_LEVEL).expect("a version-1 journal must load");
    assert_eq!(read.len(), 1);
    assert_eq!(read.player(), None);

    bytes[4..8].copy_from_slice(&99u32.to_le_bytes());
    std::fs::write(&path, &bytes).expect("write");
    assert!(EditJournal::load(&path, SEED, SEA_LEVEL).is_err());
    let _ = std::fs::remove_file(&path);
}

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

    assert_eq!(after.vel, glam::Vec3::ZERO);
    let _ = std::fs::remove_file(&path);
}

fn world_on_disk(path: &std::path::Path) -> Config {
    Config {
        edits: Some(path.display().to_string()),
        ..Default::default()
    }
}

fn count_of(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes[16..20].try_into().unwrap())
}

fn with_count(bytes: &[u8], n: u32) -> Vec<u8> {
    let mut out = bytes.to_vec();
    out[16..20].copy_from_slice(&n.to_le_bytes());
    out
}

fn records_in(bytes: &[u8]) -> usize {
    let flags = u32::from_le_bytes(bytes[20..24].try_into().unwrap());
    let extras = if flags & 1 != 0 { 32 } else { 0 } + if flags & 4 != 0 { 24 } else { 0 };
    (bytes.len() - 24 - extras) / 16
}

fn unplayed(name: &str) -> std::path::PathBuf {
    let p = temp_path(name);
    let _ = std::fs::remove_file(&p);
    p
}

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

    std::fs::write(&path, with_count(&bytes, 15)).unwrap();
    let read = EditJournal::load(&path, SEED, SEA_LEVEL).expect("loaded");
    assert_eq!(read.len(), 20, "five uncounted edits were thrown away");
    let _ = std::fs::remove_file(&path);
}

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

    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(records_in(&bytes), 1, "the records moved without the reader");
    assert_eq!(
        u32::from_le_bytes(bytes[20..24].try_into().unwrap()) & 4,
        4,
        "FLAG_BAR is what says the block is there"
    );
}

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

    assert_eq!(journal::bar_for(&read), block::HOTBAR);
}

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

#[test]
fn the_bar_flag_is_refused_as_a_whole_or_not_at_all() {
    let path = unplayed("bar-flag");
    let with = |spec: &str| Config {
        bar: Some(spec.to_string()),
        ..world_on_disk(&path)
    };
    for bad in [
        "1,2,3",                          
        "1,2,3,4,5,6,7,8,9,10,11",        
        "1,2,3,4,5,6,7,8,9,dirt",         
        "18,2,3,4,5,6,7,8,9,10",          
        "13,2,3,4,5,6,7,8,9,10",          
        "0,2,3,4,5,6,7,8,9,10",           
    ] {
        let _ = std::fs::remove_file(&path);
        assert!(
            journal::open(&with(bad)).is_err(),
            "--bar {bad:?} was accepted"
        );
    }

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

#[test]
fn the_bar_flag_and_a_world_that_already_has_a_bar_is_refused() {
    let path = unplayed("bar-conflict");
    let j = EditJournal::empty();
    j.set_bar(a_bar());
    j.save(&path, SEED, SEA_LEVEL).expect("saved");

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
