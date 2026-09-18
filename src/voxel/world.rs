//! Global world: shared pools, chunk records, block queries and edits.

use super::attributes::{decode_leaf, encode_leaf, WordPool, UNIFORM_BIT};
use super::chunk::{AttrRef, ChunkKey, ChunkRecord, LightRef};
use super::geometry::{below, has, Inner, RunPool};
use super::tree::{l1_bit, leaf_bit, voxel_bit, LocalTree};
use crate::block::{self, BlockId, AIR};
use crate::journal::EditJournal;
use crate::math::WORLD_HEIGHT;
use crate::probe::ProbeData;
use glam::IVec3;
use rustc_hash::FxHashMap;
use std::sync::Arc;

pub const LIGHT_TABLE_WORDS: usize = 4096;
pub const LIGHT_BRICK_WORDS: usize = 32;

pub struct World {
    pub leaves: RunPool<u64>,
    pub inners: RunPool<Inner>,
    pub bricks: WordPool,
    pub chunks: FxHashMap<ChunkKey, ChunkRecord>,
    /// Incremented on any change that affects the render list.
    pub version: u64,
    /// Batch 57's ambient cubes, baked and not yet written into the renderer's lattice.
    ///
    /// **A map rather than a queue, keyed on the chunk**, because a chunk can be rebuilt
    /// before the renderer has drained the previous bake and two entries for one chunk would
    /// write the same texels twice. It is drained whole by `Renderer::sync_world`; a `World`
    /// nobody renders -- `--bench-terrain`, the tests -- simply accumulates it and drops it,
    /// which is bounded by the chunk count and is why there is no cap here.
    pub probe_dirty: FxHashMap<ChunkKey, ProbeData>,
    /// Resident chunks whose `has_emitter` is set (roadmap P11). A count rather than a
    /// scan, because the question is asked once per frame: whether the world holds any
    /// emitter at all is what keys `SPEC_EMITTER_GATHER`. **Exact on the edit path** (D5):
    /// placing the first lamp of a chunk bumps it, breaking that chunk's last lamp bumps
    /// it back, so the gate disarms on the frame the last lamp out is removed rather than
    /// when its chunk someday streams out.
    pub emitter_chunks: u32,
    /// Where accepted edits are recorded, so a chunk that unloads and regenerates comes back
    /// the way it was left. Empty until `attach_journal` shares the streamer's own, and a
    /// world that never attaches one still records -- into a map nothing reads, which is what
    /// keeps `set_block` to a single path rather than an `Option` it could forget to check.
    pub journal: Arc<EditJournal>,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    pub fn new() -> Self {
        Self {
            leaves: RunPool::new(),
            inners: RunPool::new(),
            bricks: WordPool::new(),
            chunks: FxHashMap::default(),
            version: 0,
            probe_dirty: FxHashMap::default(),
            emitter_chunks: 0,
            journal: EditJournal::empty(),
        }
    }

    /// Share the streamer's journal, so what `set_block` records is what replay reads back.
    pub fn attach_journal(&mut self, journal: Arc<EditJournal>) {
        self.journal = journal;
    }

    pub fn contains(&self, key: &ChunkKey) -> bool {
        self.chunks.contains_key(key)
    }

    /// Whether any resident chunk holds an emitter (roadmap P11): the uniform answer
    /// the block-light gather's gate was *meant* to ask. True whenever one is, so the
    /// only cost of a stale bit is the gated arm staying on a world whose lamp was
    /// removed somewhere behind the camera.
    pub fn any_emitter_resident(&self) -> bool {
        self.emitter_chunks != 0
    }

    /// Intern a worker-built tree into the global pools.
    pub fn insert_local(&mut self, key: ChunkKey, t: LocalTree) {
        if self.chunks.contains_key(&key) {
            self.remove_chunk(key);
        }
        let mut run: Vec<Inner> = Vec::with_capacity(t.l1.len());
        let mut temp: Vec<u32> = Vec::new();
        for n in &t.l1 {
            if n.full {
                run.push(Inner::full(n.leaf_prefix | ((n.water_line as u32) << 28)));
            } else {
                let cnt = n.mask.count_ones() as usize;
                let s = n.leaf_start as usize;
                let (off, _) = self.leaves.intern(&t.leaves[s..s + cnt]);
                temp.push(off);
                run.push(Inner {
                    mask: n.mask,
                    run_ptr: off,
                    // P9: the waterline rides the free top nibble; the count fits in 12.
                    leaf_prefix: n.leaf_prefix | ((n.water_line as u32) << 28),
                });
            }
        }
        let root = self.make_root(t.root_mask, &run);
        for off in temp {
            self.leaves.release(off);
        }

        let mut regions = Vec::new();
        let attr = match t.uniform {
            Some(u) => AttrRef::Uniform(u),
            None if t.attrs.is_empty() => AttrRef::Uniform(AIR),
            None => {
                let pal_base = if t.palette.is_empty() {
                    0
                } else {
                    let b = self.bricks.alloc_write(&t.palette);
                    regions.push((b, t.palette.len()));
                    b
                };
                let table: Vec<u32> = t
                    .attrs
                    .iter()
                    .map(|&e| {
                        if e & UNIFORM_BIT != 0 {
                            e
                        } else {
                            e + pal_base
                        }
                    })
                    .collect();
                let base = self.bricks.alloc_write(&table);
                AttrRef::Table {
                    base,
                    len: table.len() as u32,
                }
            }
        };
        let rec = ChunkRecord {
            key,
            root,
            attr,
            regions,
            light: LightRef::Uniform(0xF000),
            light_region: None,
            aabb_min: IVec3::new(
                t.aabb_min[0] as i32,
                t.aabb_min[1] as i32,
                t.aabb_min[2] as i32,
            ),
            aabb_max: IVec3::new(
                t.aabb_max[0] as i32,
                t.aabb_max[1] as i32,
                t.aabb_max[2] as i32,
            ),
            solid_count: t.solid_count,
            leaf_count: t.leaf_count,
            has_water: t.has_water,
            dry_mask: t.dry_mask,
            has_foliage: t.has_foliage,
            has_cutout: t.has_cutout,
            has_glass: t.has_glass,
            has_emitter: t.has_emitter,
            emitters: t.emitters,
            version: 0,
        };
        self.chunks.insert(key, rec);
        if self.chunks[&key].emitters != 0 {
            self.emitter_chunks += 1;
        }
        self.version += 1;
    }

    /// Intern an L1 run (taking references on its leaf runs) and produce the root node.
    fn make_root(&mut self, root_mask: u64, run: &[Inner]) -> Inner {
        if run.is_empty() {
            return Inner::EMPTY;
        }
        if run.len() == 64 && run.iter().all(|n| n.is_full()) {
            return Inner::full(0);
        }
        let (off, is_new) = self.inners.intern(run);
        if is_new {
            for n in run {
                if !n.is_full() {
                    self.leaves.add_ref(n.run());
                }
            }
        }
        Inner {
            mask: root_mask,
            run_ptr: off,
            leaf_prefix: 0,
        }
    }

    fn release_root(&mut self, root: Inner) {
        if root.is_full() || root.is_empty() {
            return;
        }
        let nodes: Vec<Inner> = self.inners.get(root.run(), root.count()).to_vec();
        if self.inners.release(root.run()) {
            for n in nodes {
                if !n.is_full() {
                    self.leaves.release(n.run());
                }
            }
        }
    }

    pub fn remove_chunk(&mut self, key: ChunkKey) -> bool {
        let Some(rec) = self.chunks.remove(&key) else {
            return false;
        };
        if rec.has_emitter {
            self.emitter_chunks -= 1;
        }
        self.release_root(rec.root);
        if let AttrRef::Table { base, len } = rec.attr {
            self.bricks.free(base, len as usize);
        }
        for (off, len) in rec.regions {
            self.bricks.free(off, len);
        }
        if let LightRef::Table { base } = rec.light {
            self.bricks.free(base, LIGHT_TABLE_WORDS);
        }
        if let Some((off, len)) = rec.light_region {
            self.bricks.free(off, len);
        }
        // A bake nobody has uploaded yet is for a chunk that is gone. The lattice keeps
        // whatever it already held there, which is either an older bake of the same world
        // position -- the bake is a pure function of one, so it is the *same* answer -- or
        // the `face_shade` fill the field is created with. Neither can be read by a surface,
        // because there is no longer a chunk here to shade.
        self.probe_dirty.remove(&key);
        self.version += 1;
        true
    }

    /// Hand a freshly baked ambient cube to the renderer's lattice.
    ///
    /// Unlike [`Self::set_light`] this does not need the chunk to be resident and does not
    /// touch the brick pool: the field is world-space and per-chunk only in who bakes it.
    pub fn set_probe(&mut self, key: ChunkKey, data: ProbeData) {
        self.probe_dirty.insert(key, data);
    }

    /// Take everything baked since the last call, for upload.
    pub fn take_probe_dirty(&mut self) -> FxHashMap<ChunkKey, ProbeData> {
        std::mem::take(&mut self.probe_dirty)
    }

    /// L1 node for a cell bit of a root (synthesizes nodes of full roots).
    #[inline]
    pub fn l1_node(&self, root: Inner, l1b: u32) -> Option<Inner> {
        if !has(root.mask, l1b) {
            return None;
        }
        if root.is_full() {
            return Some(Inner::full(l1b * 64));
        }
        Some(self.inners.get(root.run(), root.count())[below(root.mask, l1b) as usize])
    }

    #[inline]
    pub fn leaf_mask(&self, l1: Inner, lfb: u32) -> u64 {
        if !has(l1.mask, lfb) {
            return 0;
        }
        if l1.is_full() {
            return u64::MAX;
        }
        self.leaves.get(l1.run(), l1.count())[below(l1.mask, lfb) as usize]
    }

    #[inline]
    fn leaf_block(&self, attr: AttrRef, leaf_index: u32, vb: u32) -> BlockId {
        match attr {
            AttrRef::Uniform(u) => u,
            AttrRef::Table { base, .. } => {
                let words = self.bricks.data();
                decode_leaf(words, words[(base + leaf_index) as usize], vb)
            }
        }
    }

    pub fn get_block(&self, p: IVec3) -> BlockId {
        if p.y < 0 || p.y >= WORLD_HEIGHT {
            return AIR;
        }
        let Some(rec) = self.chunks.get(&ChunkKey::of_block(p)) else {
            return AIR;
        };
        let l = p & 63;
        let (x, y, z) = (l.x as u32, l.y as u32, l.z as u32);
        let Some(l1) = self.l1_node(rec.root, l1_bit(x, y, z)) else {
            return AIR;
        };
        let lfb = leaf_bit(x, y, z);
        let m = self.leaf_mask(l1, lfb);
        let vb = voxel_bit(x, y, z);
        if !has(m, vb) {
            return AIR;
        }
        self.leaf_block(rec.attr, (l1.leaf_prefix & crate::voxel::PREFIX_MASK) + below(l1.mask, lfb), vb)
    }

    #[inline]
    pub fn is_solid(&self, p: IVec3) -> bool {
        block::def(self.get_block(p)).solid
    }

    pub fn is_loaded_at(&self, p: IVec3) -> bool {
        self.chunks.contains_key(&ChunkKey::of_block(p))
    }

    /// Set a block at LOD 0 with a bottom-up path edit. Returns false if the chunk is
    /// not loaded.
    pub fn set_block(&mut self, p: IVec3, id: BlockId) -> bool {
        if p.y < 0 || p.y >= WORLD_HEIGHT {
            return false;
        }
        let key = ChunkKey::of_block(p);
        let (old_root, attr, leaf_count) = match self.chunks.get(&key) {
            Some(r) => (r.root, r.attr, r.leaf_count),
            None => return false,
        };
        let l = p & 63;
        let (x, y, z) = (l.x as u32, l.y as u32, l.z as u32);
        let l1b = l1_bit(x, y, z);
        let lfb = leaf_bit(x, y, z);
        let vb = voxel_bit(x, y, z);

        // Current L1 run, dense by cell.
        let mut l1_nodes = [Inner::EMPTY; 64];
        if old_root.is_full() {
            for (b, n) in l1_nodes.iter_mut().enumerate() {
                // P9: a synthetic node has no measured waterline; 0xF disables the
                // ray-side skip, which is the conservative direction (walk as before).
                *n = Inner::full(((b as u32) * 64) | 0xF000_0000);
            }
        } else if !old_root.is_empty() {
            let run = self.inners.get(old_root.run(), old_root.count());
            let mut k = 0;
            for b in 0..64u32 {
                if has(old_root.mask, b) {
                    l1_nodes[b as usize] = run[k];
                    k += 1;
                }
            }
        }
        let old_l1_nodes = l1_nodes;
        let old_l1 = l1_nodes[l1b as usize];

        // Current leaf run of that L1, dense by cell.
        let mut leafs = [0u64; 64];
        if old_l1.is_full() {
            leafs = [u64::MAX; 64];
        } else if !old_l1.is_empty() {
            let run = self.leaves.get(old_l1.run(), old_l1.count());
            let mut k = 0;
            for b in 0..64u32 {
                if has(old_l1.mask, b) {
                    leafs[b as usize] = run[k];
                    k += 1;
                }
            }
        }
        let old_leaf = leafs[lfb as usize];
        let was_solid = has(old_leaf, vb);
        let now_solid = id != AIR;
        if !was_solid && !now_solid {
            return true;
        }
        // Table position of this leaf; also the insertion point when it did not exist.
        // Derived from the dense node array rather than `old_l1.leaf_prefix`, because an
        // absent L1 cell carries a prefix of 0 and would insert at the front of the table.
        let mut base_prefix = 0u32;
        for n in l1_nodes.iter().take(l1b as usize) {
            base_prefix += n.count();
        }
        debug_assert!(old_l1.is_empty() || (old_l1.leaf_prefix & crate::voxel::PREFIX_MASK) == base_prefix);
        let leaf_pos = (base_prefix + below(old_l1.mask, lfb)) as usize;

        let mut ids = [AIR; 64];
        if old_leaf != 0 {
            for b in 0..64u32 {
                if has(old_leaf, b) {
                    ids[b as usize] = self.leaf_block(attr, leaf_pos as u32, b);
                }
            }
        }
        if was_solid && now_solid && ids[vb as usize] == id {
            return true;
        }
        // The one place an edit is known to be real: past the height bound, past the
        // residency check, and past both redundant-write returns. Recording here rather than
        // at the two call sites that edit is what stops a third from being added without one
        // -- the argument `scene.rs` makes about `flags_from`. See `docs/persistence.md`.
        self.journal.record(p, id);
        let new_leaf = if now_solid {
            old_leaf | (1u64 << vb)
        } else {
            old_leaf & !(1u64 << vb)
        };
        let old_id = ids[vb as usize];
        ids[vb as usize] = id;

        // ---- attribute table
        let mut table: Vec<u32> = match attr {
            AttrRef::Uniform(u) => vec![UNIFORM_BIT | u as u32; leaf_count as usize],
            AttrRef::Table { base, len } => self.bricks.get(base, len as usize).to_vec(),
        };
        let mut new_regions: Vec<(u32, usize)> = Vec::new();
        if new_leaf != 0 {
            let mut pw = Vec::new();
            let e = encode_leaf(&ids, new_leaf, &mut pw);
            let entry = if e & UNIFORM_BIT != 0 {
                e
            } else {
                let off = self.bricks.alloc_write(&pw);
                new_regions.push((off, pw.len()));
                off
            };
            if old_leaf != 0 {
                table[leaf_pos] = entry;
            } else {
                table.insert(leaf_pos, entry);
            }
        } else {
            table.remove(leaf_pos);
        }

        // ---- new L1 node
        leafs[lfb as usize] = new_leaf;
        let mut new_mask = 0u64;
        for (b, m) in leafs.iter().enumerate() {
            if *m != 0 {
                new_mask |= 1u64 << b;
            }
        }
        let new_l1 = if new_mask == 0 {
            Inner::EMPTY
        } else if new_mask == u64::MAX && leafs.iter().all(|&m| m == u64::MAX) {
            // P9: the edit path does not re-scan for a waterline; disable the bound.
            Inner::full(0xF000_0000)
        } else {
            let run: Vec<u64> = leafs.iter().copied().filter(|&m| m != 0).collect();
            let (off, _) = self.leaves.intern(&run);
            Inner {
                mask: new_mask,
                run_ptr: off,
                leaf_prefix: 0,
            }
        };
        let temp_leaf = if !new_l1.is_full() && !new_l1.is_empty() {
            Some(new_l1.run())
        } else {
            None
        };
        l1_nodes[l1b as usize] = new_l1;

        // ---- new L1 run with recomputed prefixes
        let mut run: Vec<Inner> = Vec::with_capacity(64);
        let mut prefix = 0u32;
        let mut root_mask = 0u64;
        for (b, n) in l1_nodes.iter().enumerate() {
            if n.is_empty() {
                continue;
            }
            let mut n = *n;
            n.leaf_prefix = prefix | (n.leaf_prefix & !crate::voxel::PREFIX_MASK);
            prefix += n.count();
            run.push(n);
            root_mask |= 1u64 << b;
        }
        let new_root = self.make_root(root_mask, &run);
        if let Some(off) = temp_leaf {
            self.leaves.release(off);
        }
        if !old_root.is_full() && !old_root.is_empty() && self.inners.release(old_root.run()) {
            for n in old_l1_nodes.iter() {
                if !n.is_empty() && !n.is_full() {
                    self.leaves.release(n.run());
                }
            }
        }

        // ---- write back attributes
        let uniform_all = match table.first() {
            None => true,
            Some(&f) => f & UNIFORM_BIT != 0 && table.iter().all(|&e| e == f),
        };
        let new_attr = if uniform_all {
            if let AttrRef::Table { base, len } = attr {
                self.bricks.free(base, len as usize);
            }
            AttrRef::Uniform(table.first().map_or(AIR, |&f| f as BlockId))
        } else {
            match attr {
                AttrRef::Table { base, len } if len as usize == table.len() => {
                    self.bricks.write(base, &table);
                    AttrRef::Table { base, len }
                }
                AttrRef::Table { base, len } => {
                    self.bricks.free(base, len as usize);
                    let b = self.bricks.alloc_write(&table);
                    AttrRef::Table {
                        base: b,
                        len: table.len() as u32,
                    }
                }
                AttrRef::Uniform(_) => {
                    let b = self.bricks.alloc_write(&table);
                    AttrRef::Table {
                        base: b,
                        len: table.len() as u32,
                    }
                }
            }
        };

        let rec = self.chunks.get_mut(&key).unwrap();
        rec.root = new_root;
        rec.attr = new_attr;
        rec.regions.extend(new_regions);
        rec.leaf_count = prefix;
        if now_solid && !was_solid {
            rec.solid_count += 1;
            let v = IVec3::new(x as i32, y as i32, z as i32);
            if rec.solid_count == 1 {
                rec.aabb_min = v;
                rec.aabb_max = v + IVec3::ONE;
            } else {
                rec.aabb_min = rec.aabb_min.min(v);
                rec.aabb_max = rec.aabb_max.max(v + IVec3::ONE);
            }
        } else if !now_solid && was_solid {
            rec.solid_count -= 1;
        }
        rec.has_water |= id == crate::block::WATER;
        // Batch 53, and **set-only for the reason the four flags around it are**: a placed
        // block that is not water makes its 16^3 cell opaque to a ray that ignores water, and
        // failing to record that would let the marcher step straight through it. Removing the
        // last dry block in a cell does not clear the bit, because clearing means a scan of
        // 4,096 voxels and the only cost of not clearing is the skip this chunk does not get.
        if id != crate::block::WATER && id != crate::block::AIR {
            rec.dry_mask |= 1u64 << crate::voxel::tree::l1_bit(x, y, z);
        }
        rec.has_foliage |= crate::block::is_foliage(id);
        rec.has_cutout |= crate::block::is_cutout(id);
        rec.has_glass |= crate::block::is_glass(id);
        // P11, exact (D5): the edit knows both ids -- `old_id` is the block that was
        // there -- so the count moves on both transitions, and the gate it feeds arms and
        // disarms on the frame either happens. No scan, anywhere: the counter below is the
        // scan of every edit this chunk has ever taken, paid two compares at a time.
        let now_e = crate::block::is_emitter(id);
        let was_e = crate::block::is_emitter(old_id);
        match (was_e, now_e) {
            (false, true) => {
                rec.emitters += 1;
                if rec.emitters == 1 {
                    rec.has_emitter = true;
                    self.emitter_chunks += 1;
                }
            }
            (true, false) => {
                rec.emitters -= 1;
                if rec.emitters == 0 {
                    rec.has_emitter = false;
                    self.emitter_chunks -= 1;
                }
            }
            _ => {}
        }
        rec.version += 1;
        self.version += 1;
        true
    }

    /// Attach computed lighting to a chunk, replacing whatever it had.
    pub fn set_light(&mut self, key: ChunkKey, data: crate::light::LightData) {
        let Some(rec) = self.chunks.get(&key) else {
            return;
        };
        let old = rec.light;
        let old_bricks = rec.light_region;
        let new = match data.uniform {
            Some(b) => {
                if let LightRef::Table { base } = old {
                    self.bricks.free(base, LIGHT_TABLE_WORDS);
                }
                if let Some((off, len)) = old_bricks {
                    self.bricks.free(off, len);
                }
                let rec = self.chunks.get_mut(&key).unwrap();
                rec.light_region = None;
                LightRef::Uniform(b)
            }
            None => {
                let words = data.bricks.len() * LIGHT_BRICK_WORDS;
                let brick_base = if words > 0 {
                    let off = self.bricks.alloc(words);
                    let flat: Vec<u32> =
                        data.bricks.iter().flat_map(|b| b.iter().copied()).collect();
                    self.bricks.write(off, &flat);
                    Some((off, words))
                } else {
                    None
                };
                // Cell entries hold absolute pool offsets, so rebase the brick indices.
                let base_off = brick_base.map_or(0, |(o, _)| o);
                let cells: Vec<u32> = data
                    .cells
                    .iter()
                    .map(|&e| {
                        if e & crate::voxel::attributes::UNIFORM_BIT != 0 {
                            e
                        } else {
                            base_off + e * LIGHT_BRICK_WORDS as u32
                        }
                    })
                    .collect();
                let table = match old {
                    LightRef::Table { base } => {
                        self.bricks.write(base, &cells);
                        base
                    }
                    LightRef::Uniform(_) => self.bricks.alloc_write(&cells),
                };
                if let Some((off, len)) = old_bricks {
                    self.bricks.free(off, len);
                }
                let rec = self.chunks.get_mut(&key).unwrap();
                rec.light_region = brick_base;
                LightRef::Table { base: table }
            }
        };
        let rec = self.chunks.get_mut(&key).unwrap();
        rec.light = new;
        rec.version += 1;
        self.version += 1;
    }

    /// Expand a chunk back into a dense block array by walking the tree, which is far
    /// cheaper than 262144 `get_block` calls. Used to relight after edits.
    pub fn extract_dense(&self, key: ChunkKey, out: &mut [BlockId]) -> bool {
        use super::tree::{bit_xyz, dense_index};
        let Some(rec) = self.chunks.get(&key) else {
            return false;
        };
        out.fill(AIR);
        let root = rec.root;
        if root.is_empty() {
            return true;
        }
        for l1b in 0..64u32 {
            let Some(l1) = self.l1_node(root, l1b) else {
                continue;
            };
            let (l1x, l1y, l1z) = bit_xyz(l1b);
            for lfb in 0..64u32 {
                let mask = self.leaf_mask(l1, lfb);
                if mask == 0 {
                    continue;
                }
                let leaf_index = (l1.leaf_prefix & crate::voxel::PREFIX_MASK) + below(l1.mask, lfb);
                let (lfx, lfy, lfz) = bit_xyz(lfb);
                for vb in 0..64u32 {
                    if !has(mask, vb) {
                        continue;
                    }
                    let (vx, vy, vz) = bit_xyz(vb);
                    let x = (l1x * 16 + lfx * 4 + vx) as usize;
                    let y = (l1y * 16 + lfy * 4 + vy) as usize;
                    let z = (l1z * 16 + lfz * 4 + vz) as usize;
                    out[dense_index(x, y, z)] = self.leaf_block(rec.attr, leaf_index, vb);
                }
            }
        }
        true
    }

    /// Bytes held by the geometry pools.
    pub fn geometry_bytes(&self) -> usize {
        self.leaves.bytes() + self.inners.bytes()
    }

    pub fn total_solid_voxels(&self) -> u64 {
        self.chunks.values().map(|c| c.solid_count as u64).sum()
    }
}



