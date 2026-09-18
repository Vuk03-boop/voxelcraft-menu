use crate::journal::EditJournal;
use crate::light::{self, LightData};
use crate::lod::{LodConfig, Planner, RenderItem, Residency};
use crate::probe::{self, ProbeData};
use crate::voxel::{build_local, ChunkKey, LocalTree, World, VOL};
use crate::worldgen::WorldGen;
use crossbeam_channel::{unbounded, Receiver, Sender};
use glam::Vec3;
use rustc_hash::{FxHashMap, FxHashSet};
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct ChunkBuild {
    pub key: ChunkKey,
    pub tree: LocalTree,
    pub light: Option<LightData>,

    pub probe: Option<ProbeData>,
}

thread_local! {
    static DENSE: RefCell<Vec<u16>> = RefCell::new(vec![0; VOL]);
    static HEIGHTS: RefCell<Box<[i32; 64 * 64]>> = RefCell::new(Box::new([0; 64 * 64]));
}

fn build(
    gen: &WorldGen,
    journal: &EditJournal,
    cache: &Mutex<probe::BakeCache>,
    key: ChunkKey,
) -> ChunkBuild {
    DENSE.with(|d| {
        HEIGHTS.with(|hs| {
            let mut dense = d.borrow_mut();
            let mut heights = hs.borrow_mut();
            gen.generate(key, &mut dense, &mut heights);

            journal.apply(key, &mut dense, gen, &heights);
            let tree = build_local(&dense);

            let light = (key.lod == 0 && !tree.is_empty()).then(|| {
                let open = light::open_columns(&heights, key.origin().y + 64);
                light::compute(&dense, &open)
            });

            let probe = (key.lod == 0).then(|| {
                if let Some(hit) = cache.lock().unwrap().get(&key) {
                    return hit;
                }
                let d = probe::bake(gen, &heights, key.origin(), gen.bounce_shadow);
                cache.lock().unwrap().insert(
                    key,
                    ProbeData {
                        occl: d.occl.clone(),
                        bounce: d.bounce.clone(),
                    },
                );
                d
            });
            ChunkBuild {
                key,
                tree,
                light,
                probe,
            }
        })
    })
}

pub struct Streamer {
    pub(crate) gen: Arc<WorldGen>,

    journal: Arc<EditJournal>,

    probe_cache: Arc<Mutex<probe::BakeCache>>,
    tx: Sender<ChunkBuild>,
    rx: Receiver<ChunkBuild>,
    in_flight: FxHashSet<ChunkKey>,
}

impl Streamer {
    pub fn new(gen: Arc<WorldGen>) -> Self {
        let (tx, rx) = unbounded();
        Self {
            gen,
            journal: EditJournal::empty(),
            tx,
            rx,
            in_flight: FxHashSet::default(),
            probe_cache: Arc::new(Mutex::new(probe::BakeCache::default())),
        }
    }

    pub fn set_journal(&mut self, journal: Arc<EditJournal>) {
        self.journal = journal;
    }

    pub fn journal(&self) -> &Arc<EditJournal> {
        &self.journal
    }

    pub fn in_flight(&self) -> usize {
        self.in_flight.len()
    }

    pub fn is_in_flight(&self, key: &ChunkKey) -> bool {
        self.in_flight.contains(key)
    }

    pub fn request(&mut self, key: ChunkKey) {
        if !self.in_flight.insert(key) {
            return;
        }
        let gen = self.gen.clone();
        let journal = self.journal.clone();
        let cache = self.probe_cache.clone();
        let tx = self.tx.clone();
        rayon::spawn(move || {
            let _ = tx.send(build(&gen, &journal, &cache, key));
        });
    }

    pub fn build_now(&self, key: ChunkKey) -> ChunkBuild {
        build(&self.gen, &self.journal, &self.probe_cache, key)
    }

    pub fn drain(&mut self, world: &mut World, budget: Duration) -> usize {
        let start = Instant::now();
        let mut n = 0;
        while let Ok(b) = self.rx.try_recv() {
            self.in_flight.remove(&b.key);
            world.insert_local(b.key, b.tree);
            if let Some(l) = b.light {
                world.set_light(b.key, l);
            }
            if let Some(p) = b.probe {
                world.set_probe(b.key, p);
            }
            n += 1;
            if start.elapsed() > budget {
                break;
            }
        }
        n
    }
}

pub fn relight(world: &mut World, gen: &WorldGen, key: ChunkKey) {
    if key.lod != 0 {
        return;
    }
    DENSE.with(|d| {
        HEIGHTS.with(|hs| {
            let mut dense = d.borrow_mut();
            if !world.extract_dense(key, &mut dense) {
                return;
            }
            let mut heights = hs.borrow_mut();
            let o = key.origin();
            for z in 0..64 {
                for x in 0..64 {
                    heights[x + z * 64] = gen.height(o.x + x as i32, o.z + z as i32);
                }
            }
            let open = light::open_columns(&heights, o.y + 64);
            world.set_light(key, light::compute(&dense, &open));
        })
    });
}

pub struct ChunkManager {
    pub cfg: LodConfig,
    pub streamer: Streamer,
    planner: Planner,

    pub selected: Vec<ChunkKey>,

    pub render_set: Vec<RenderItem>,
    last_needed: FxHashMap<ChunkKey, u64>,

    dirty_coarse: FxHashSet<ChunkKey>,
    frame: u64,
    pub max_in_flight: usize,
    pub unload_grace_frames: u64,
    pub last_drained: usize,
}

impl ChunkManager {

    const BOOKKEEP_EVERY: u64 = 16;

    pub fn new(cfg: LodConfig, gen: Arc<WorldGen>) -> Self {
        Self {
            cfg,
            streamer: Streamer::new(gen),
            planner: Planner::default(),
            selected: Vec::new(),
            render_set: Vec::new(),
            last_needed: FxHashMap::default(),
            dirty_coarse: FxHashSet::default(),
            frame: 0,
            max_in_flight: 64,
            unload_grace_frames: 240,
            last_drained: 0,
        }
    }

    pub fn generator(&self) -> &Arc<WorldGen> {
        &self.streamer.gen
    }

    pub fn attach_journal(&mut self, journal: Arc<EditJournal>) {
        self.streamer.set_journal(journal);
    }

    pub fn pending(&self) -> usize {
        self.streamer.in_flight()
    }

    fn absorb_dirty(&mut self) {
        for k in self.streamer.journal().take_dirty() {
            let mut key = k;
            while key.lod < self.cfg.max_lod {
                key = key.parent();
                self.dirty_coarse.insert(key);
            }
        }
    }

    pub fn rebuild_dirty_now(&mut self, world: &mut World) -> usize {
        self.absorb_dirty();
        let mut n = 0;
        for k in std::mem::take(&mut self.dirty_coarse) {
            if self.streamer.is_in_flight(&k) {
                self.dirty_coarse.insert(k);
            } else if world.contains(&k) {

                debug_assert!(k.lod > 0, "a coarse rebuild would drop LOD 0's light");
                let b = self.streamer.build_now(k);
                world.insert_local(b.key, b.tree);
                n += 1;
            }
        }
        n
    }

    pub fn is_settled(&self, world: &World) -> bool {
        self.streamer.in_flight() == 0 && self.selected.iter().all(|k| world.contains(k))
    }

    pub fn update(&mut self, cam: Vec3, world: &mut World, budget: Duration) {
        self.frame += 1;
        self.last_drained = self.streamer.drain(world, budget);

        self.planner.plan(
            cam,
            &self.cfg,
            &|k: ChunkKey| match world.chunks.get(&k) {
                None => Residency::Absent,
                Some(r) if r.solid_count == 0 => Residency::Empty,
                Some(_) => Residency::Solid,
            },
            &mut self.selected,
            &mut self.render_set,
        );

        self.absorb_dirty();

        let mut free_slots = self.max_in_flight.saturating_sub(self.streamer.in_flight());

        if !self.dirty_coarse.is_empty() {
            for k in std::mem::take(&mut self.dirty_coarse) {
                if self.streamer.is_in_flight(&k) {

                    self.dirty_coarse.insert(k);
                } else if !world.contains(&k) {

                } else if free_slots > 0 {
                    self.streamer.request(k);
                    free_slots -= 1;
                } else {
                    self.dirty_coarse.insert(k);
                }
            }
        }

        if free_slots > 0 {
            let mut missing: Vec<(ChunkKey, f32)> = Vec::new();
            let mut seen = FxHashSet::default();
            for &k in &self.selected {
                let mut key = k;
                loop {
                    if world.contains(&key) {
                        break;
                    }
                    if !self.streamer.is_in_flight(&key) && seen.insert(key) {
                        missing.push((key, key.bounds().distance_to(cam)));
                    }
                    if key.lod >= self.cfg.max_lod {
                        break;
                    }
                    key = key.parent();
                }
            }
            missing.sort_by(|a, b| {
                b.0.lod
                    .cmp(&a.0.lod)
                    .then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            });
            for (key, _) in missing.into_iter().take(free_slots) {
                self.streamer.request(key);
            }
        }

        let frame = self.frame;
        if frame.is_multiple_of(Self::BOOKKEEP_EVERY) {
            let drawn = self.render_set.iter().map(|it| it.key);
            for k in drawn.chain(self.selected.iter().copied()) {
                self.last_needed.insert(k, frame);
            }

            for k in world.chunks.keys() {
                self.last_needed.entry(*k).or_insert(frame);
            }
            let grace = self.unload_grace_frames;
            let stale: Vec<ChunkKey> = world
                .chunks
                .keys()
                .filter(|k| frame.saturating_sub(self.last_needed[k]) > grace)
                .copied()
                .collect();
            for k in stale {
                world.remove_chunk(k);
                self.last_needed.remove(&k);
            }
            self.last_needed
                .retain(|_, &mut seen| frame.saturating_sub(seen) <= grace * 2);
        }
    }
}
