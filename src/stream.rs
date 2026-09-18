//! Chunk streaming: worker generation, budgeted main-thread interning, LOD selection,
//! fallback-to-ancestor rendering, and unloading.

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
    /// Batch 57's ambient cube. Present for every LOD-0 chunk, **including an empty one**,
    /// which is where it differs from `light` -- a probe four blocks inside a chunk of pure
    /// air is still read by the trilinear tap of a surface in the *neighbouring* chunk, so
    /// leaving it unbaked would leave that surface reading whatever the lattice held before.
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
            // Replay the player's edits before anything reads the array, so the tree, the
            // attributes and the lighting below all see one world. Doing it here rather than
            // through `set_block` after interning is what makes a reloaded chunk identical to
            // the one that was edited live -- see `docs/persistence.md`.
            //
            // `heights` is passed rather than re-derived because the coarse path aggregates
            // against the surface *this* chunk was filled up to, and a second sample of the
            // field is a second answer waiting to disagree with the first.
            journal.apply(key, &mut dense, gen, &heights);
            let tree = build_local(&dense);
            // Only LOD 0 carries real lighting; coarse chunks read as open sky.
            let light = (key.lod == 0 && !tree.is_empty()).then(|| {
                let open = light::open_columns(&heights, key.origin().y + 64);
                light::compute(&dense, &open)
            });
            // The bake reads `heights` and the generator and never touches `dense`, which is
            // what makes it a pure function of world position and the field seamless across
            // chunk boundaries -- see `probe`'s module note. It is deliberately *not* gated on
            // `tree.is_empty()` the way the flood above is: an empty chunk's probes are read
            // by its neighbours' surfaces.
            // Batch 79 (P-D): the 13 ms line, paid of the idle VRAM's mirror on the CPU
            // side. The bake is a pure function of position (heights + generator, never
            // `dense` or `journal`), so a stored answer *is* the recomputed one to the
            // bit, and an edit-revisited chunk answers from the cache the same frame it
            // would have without it. Get-then-insert with the 13 ms between them OUTSIDE
            // the lock: the threads this loop exists to feed would otherwise queue behind
            // one another's bakes, which is the thing the workers were hired to prevent.
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
    /// Shared with the `World` that records into it, and with every worker that replays from
    /// it. Empty until `set_journal`, which is the whole of "persistence is off".
    journal: Arc<EditJournal>,
    /// Batch 79 (P-D): remembered bakes of previously streamed chunks. `Mutex` rather
    /// than per-thread clones (the `DENSE`/`HEIGHTS` pattern): one roll-up of memory
    /// shared where it is read, with the 13 ms between `get` and `insert` outside it.
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

    /// Build synchronously on the calling thread (tests, benches, headless capture).
    pub fn build_now(&self, key: ChunkKey) -> ChunkBuild {
        build(&self.gen, &self.journal, &self.probe_cache, key)
    }

    /// Intern finished chunks until the time budget is spent. Returns how many landed.
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

/// Recompute lighting for a chunk after an edit, reading its current contents back out
/// of the tree. `heights` comes from worldgen, so sky access matches generation.
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
    /// What the camera wants, regardless of what has streamed in. Drives requests.
    pub selected: Vec<ChunkKey>,
    /// Chunks to draw this frame, using coarser or finer stand-ins wherever the selected
    /// level is not resident yet. It tiles the selection's volume once *in expectation*:
    /// outside a transition band each entry owns its volume outright, and inside one a
    /// coarse entry and its fine subtree overlap while splitting the rays between them.
    pub render_set: Vec<RenderItem>,
    last_needed: FxHashMap<ChunkKey, u64>,
    /// Coarse stand-ins interned before an edit landed under them, and therefore showing a
    /// world without it. `journal::apply` reads the journal once, at build time, so the only
    /// way an edit reaches a distance is to build the chunk again.
    dirty_coarse: FxHashSet<ChunkKey>,
    frame: u64,
    pub max_in_flight: usize,
    pub unload_grace_frames: u64,
    pub last_drained: usize,
}

impl ChunkManager {
    /// Frames between retention sweeps. Anything well under `unload_grace_frames`.
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

    /// Wire in the journal the workers replay from. The caller attaches the same `Arc` to the
    /// `World`, which is the recording half; one object, so what is written is what comes
    /// back.
    pub fn attach_journal(&mut self, journal: Arc<EditJournal>) {
        self.streamer.set_journal(journal);
    }

    pub fn pending(&self) -> usize {
        self.streamer.in_flight()
    }

    /// Take the chunks the journal has recorded edits in and mark every coarse ancestor over
    /// them stale.
    ///
    /// An edit reaches LOD 0 through `set_block` and every other level through a rebuild,
    /// because `journal::apply` reads the journal once, at build time: a stand-in already
    /// resident has no route to an edit that lands afterwards, and it is exactly the one the
    /// player sees the moment they walk away. Absorbed into a set rather than acted on as it
    /// arrives, because a player laying blocks dirties the same handful of ancestors every
    /// frame.
    ///
    /// One function because there are two schedules for the same work and only one statement
    /// of what an edit invalidates -- see [`Self::rebuild_dirty_now`].
    fn absorb_dirty(&mut self) {
        for k in self.streamer.journal().take_dirty() {
            let mut key = k;
            while key.lod < self.cfg.max_lod {
                key = key.parent();
                self.dirty_coarse.insert(key);
            }
        }
    }

    /// Rebuild every stale coarse stand-in now, on this thread, and return how many.
    ///
    /// [`Self::update`] does the same work against the streaming budget, a few chunks a
    /// frame, which is right for a session and useless for a capture: a headless frame is
    /// rendered once, straight after the edits, and has no later frame to be corrected in.
    /// It is the same split `Streamer::build_now` already makes against `request`.
    ///
    /// **Building here rather than settling again is not a convenience**, though batch 30
    /// gave the wrong reason for it and batch 31 had to take that reason back. The claim was
    /// that `update` is not idempotent on a settled world. It is: four extra calls at an
    /// unmoved camera are bit-exact at all fourteen vantages. What moved batch 30's two
    /// captures was that a second `settle()` runs at the camera the capture is *about to*
    /// render from, and re-planning there was the correct thing to do -- worth 52 pixels at
    /// `open-sea` and 56 at `shore`, which the capture path now takes deliberately. The
    /// surviving reason is the plain one: a settle spends its rebuild budget a few chunks
    /// per frame, and this call site has no next frame to spend it over.
    pub fn rebuild_dirty_now(&mut self, world: &mut World) -> usize {
        self.absorb_dirty();
        let mut n = 0;
        for k in std::mem::take(&mut self.dirty_coarse) {
            if self.streamer.is_in_flight(&k) {
                self.dirty_coarse.insert(k);
            } else if world.contains(&k) {
                // `absorb_dirty` only ever inserts a `parent()`, so this is never LOD 0 and
                // `build` never returns light for it -- which is why only the tree is taken.
                debug_assert!(k.lod > 0, "a coarse rebuild would drop LOD 0's light");
                let b = self.streamer.build_now(k);
                world.insert_local(b.key, b.tree);
                n += 1;
            }
        }
        n
    }

    /// True once nothing is in flight and every selected chunk is resident.
    pub fn is_settled(&self, world: &World) -> bool {
        self.streamer.in_flight() == 0 && self.selected.iter().all(|k| world.contains(k))
    }

    pub fn update(&mut self, cam: Vec3, world: &mut World, budget: Duration) {
        self.frame += 1;
        self.last_drained = self.streamer.drain(world, budget);

        // Plan against what is resident *after* draining, so a chunk that landed this
        // frame is drawn this frame rather than one frame later.
        // `Empty` and `Solid` both cover a volume, so a stand-in is chosen exactly as it
        // was; the distinction only matters to `dissolve`, which will not fade a chunk
        // into a partner that has no geometry to fade from.
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

        // Request what is missing, coarse levels first so the far view fills in
        // immediately, then nearest first so detail arrives where the player looks.
        // Walking up stops at the first resident ancestor: anything above it is already
        // stood in for, and requesting it anyway used to generate hundreds of chunks a
        // second that the unload pass deleted on arrival.
        let mut free_slots = self.max_in_flight.saturating_sub(self.streamer.in_flight());

        // Stale stand-ins before missing ones: a chunk that is merely absent shows nothing,
        // where a stale one shows something wrong, and the wrong thing is the defect this
        // batch exists to remove. A rebuild replaces the resident chunk when it lands
        // (`insert_local` swaps it), so nothing is ever removed and there is no frame with a
        // hole in it.
        if !self.dirty_coarse.is_empty() {
            for k in std::mem::take(&mut self.dirty_coarse) {
                if self.streamer.is_in_flight(&k) {
                    // It may have been generated before the edit was recorded, so it stays
                    // dirty and is asked again once it lands. Worst case that is one wasted
                    // rebuild of a chunk that already had the edit in it.
                    self.dirty_coarse.insert(k);
                } else if !world.contains(&k) {
                    // Nothing to correct: whenever it is next built it reads the journal.
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

        // Retention bookkeeping. The grace period is hundreds of frames, so touching it
        // every frame only costs two passes over every loaded chunk for nothing.
        let frame = self.frame;
        if frame.is_multiple_of(Self::BOOKKEEP_EVERY) {
            let drawn = self.render_set.iter().map(|it| it.key);
            for k in drawn.chain(self.selected.iter().copied()) {
                self.last_needed.insert(k, frame);
            }
            // A chunk nobody has asked for yet still gets the full grace period from the
            // moment it lands, so a stand-in is never deleted before it gets used.
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



