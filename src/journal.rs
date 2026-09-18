//! The edit journal: the deltas `set_block` applied, replayed into the dense array after
//! worldgen so a chunk comes back the way the player left it.
//!
//! **Only the deltas are persisted, and that is a property of the world model rather than a
//! shortcut.** Terrain is a pure function of the seed, so worldgen plus the deltas *is* the
//! world; writing chunks out would be writing down something the generator already knows.
//!
//! **The coarse levels take the deltas too, and that is batch 30.** They still resample the
//! noise at stride 2^n rather than aggregating their children, and they should: the field is
//! shared code with only the stride differing, so terrain already agrees across a transition.
//! The deltas were the one thing in the world that no coarse level had any route to, which is
//! why a structure used to vanish as the player walked away from it. What aggregates is the
//! journal, not the terrain -- see [`EditJournal::apply`] for the three rules.
//!
//! Replay happens in [`crate::stream::build`], between `generate` and `build_local`, and not
//! through [`crate::voxel::World::set_block`]. Three reasons, in order of how much they
//! matter:
//!
//! 1. **Lighting falls out for free.** `light::compute` then runs on the edited dense array,
//!    so a reloaded chunk needs no relight pass and cannot disagree with a live-edited one.
//! 2. It runs on the rayon worker, so a chunk reappearing costs the main thread nothing.
//! 3. `ChunkRecord::regions` grows one entry per non-uniform-leaf edit and is freed only on
//!    unload; a `set_block` replay would pay that again on every reload.
//!
//! The claim this module lives or dies by is that those two routes agree exactly --
//! `worldgen + deltas` is pixel-identical to `worldgen` followed by the same edits. It is
//! checked block-for-block in `tests/edit_journal.rs` and pixel-for-pixel by the sweep in
//! `docs/persistence.md`.

use crate::block::{BlockId, AIR};
use crate::config::Config;
use crate::voxel::{dense_index, ChunkKey, VOL};
use crate::worldgen::WorldGen;
use glam::IVec3;
use parking_lot::{Mutex, RwLock};
use rustc_hash::{FxHashMap, FxHashSet};
use std::cmp::Reverse;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Arc;

/// `VXJ1` as little-endian bytes. A foreign or truncated file has to fail loudly: a journal
/// that silently replayed nothing would render a world that looks entirely plausible, which
/// is the failure mode `docs/errors.md` calls indistinguishable from success.
pub const MAGIC: u32 = u32::from_le_bytes(*b"VXJ1");
/// Bumped to 2 by batch 32, which added the player block, and to 3 by batch 34, which added
/// the bar block. **Every version loads**, and that is not a compatibility courtesy -- it is
/// the `flags` word below doing exactly the job its predecessor's comment promised. A
/// version-1 file wrote that word as zero, so it says "no player block" without a line of
/// code asking what version it was; a version-2 file says "no bar block" the same way, and
/// "no bar block" already means the default bar. Three versions, one reader, no branch on the
/// number -- **the second time that claim has been made and the first time it was tested by a
/// file the reader had never seen**.
///
/// What the bump buys is the *other* direction: a pre-34 binary handed a version-3 file says
/// so in one sentence, where without it the file would fail on a byte count and blame the
/// edit records. That is what `voxelcraft-pre34.exe` actually prints, and it is the reason
/// batch 32 spent a number it did not otherwise need.
pub const VERSION: u32 = 3;

/// Bit 0 of [`Header::flags`]: a [`PlayerState`] block sits between the header and the
/// records.
const FLAG_PLAYER: u32 = 1;

/// Bit 1 of [`Header::flags`]: **this file was left by the append log, so its tail may hold
/// whole records the `count` does not claim.** Batch 33.
///
/// It is a spare bit of `flags` rather than version 3, which is the thing batch 32's comment
/// on `VERSION` said the next feature should do. It buys more than tidiness: a file with this
/// bit clear keeps the exact strict check batch 29 shipped -- a size that disagrees with the
/// count is a truncation and is refused -- so relaxing that check for an append log does not
/// relax it for the compacted files every A/B in `docs/persistence.md` is measured on.
const FLAG_APPEND: u32 = 2;

/// Bit 2 of [`Header::flags`]: a [`BarState`] block sits after the player block. Batch 34.
///
/// **A second presence bit rather than a wider player block**, and the two genuinely come
/// apart: a version-2 file has a player and no bar, which is the state this bit exists to
/// describe. Folding the bar into [`PlayerState`] would have made its size depend on the
/// version number, and reading it would then be a branch on that number -- the one thing
/// `VERSION`'s comment says the `flags` word is there to avoid.
const FLAG_BAR: u32 = 4;

/// The file header. Six words, no implicit padding, so `bytemuck` can read it in place.
///
/// `seed` and `sea_level` are the load-bearing half. A journal replayed into a world
/// generated from a different seed puts every edit at a position that no longer means
/// anything -- a hut in mid-air, a tunnel through open sky -- and **every capture of it still
/// looks like a capture**. Refusing the load is the only place that can be caught.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Header {
    magic: u32,
    version: u32,
    seed: i32,
    sea_level: i32,
    count: u32,
    /// What else is in the file. This was the spare word batch 29 left "for when player
    /// state lands", and it is spent the way that comment intended: as the *presence* bit,
    /// not as the state. Twenty-four bits of it are still spare.
    flags: u32,
}

/// Where the player was standing when the journal was written, and what they had in hand.
///
/// **32 bytes, so the header plus this stays 8-byte aligned** and the record array behind it
/// is still `bytemuck`-readable in place -- the same arithmetic the header's own comment used
/// to make on its own.
///
/// `pos` is feet, in **internal** world coordinates: the frame [`Record::y`] uses and
/// [`crate::player::Player::pos`] holds, not the one the stats overlay prints. One frame for
/// the whole file, so a journal cannot disagree with itself about where the ground is.
///
/// Angles in radians rather than the degrees `--cam-yaw` takes, because this is what the
/// player *is* and not what a command line asked for; `Player` stores radians and the
/// conversion belongs at the flag, which is where it already lives.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PlayerState {
    pub pos: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub hotbar: u32,
    /// Flying rather than walking. A `u32` because `bool` is not `Pod` and a file format
    /// should not care what Rust thinks a byte of `true` looks like.
    pub fly: u32,
    _pad: u32,
}

impl PlayerState {
    pub fn new(pos: [f32; 3], yaw: f32, pitch: f32, hotbar: u32, fly: bool) -> Self {
        Self {
            pos,
            yaw,
            pitch,
            hotbar,
            fly: fly as u32,
            _pad: 0,
        }
    }

    /// Everything about a loaded player block that could make the renderer draw a frame
    /// nobody could tell was wrong.
    ///
    /// A NaN position renders a black image and a hotbar of 47 indexes off the end of
    /// [`crate::block::HOTBAR`]; the first is `docs/errors.md`'s failure mode that looks like
    /// success and the second is a panic in a getter three call sites deep. Both are checked
    /// here, at the file's edge, for the reason the seed guard is: this is the last place
    /// that still knows the value came out of a file.
    fn check(&self, path: &Path) -> Result<(), String> {
        if !self.pos.iter().all(|v| v.is_finite()) || !self.yaw.is_finite() || !self.pitch.is_finite()
        {
            return Err(format!(
                "{}: the journal's player position or look angle is not a finite number",
                path.display()
            ));
        }
        if self.hotbar as usize >= crate::block::HOTBAR.len() {
            return Err(format!(
                "{}: journal selects hotbar slot {}, this build has {}",
                path.display(),
                self.hotbar,
                crate::block::HOTBAR.len()
            ));
        }
        Ok(())
    }
}

/// What the player had to hand, one block id per slot. Batch 34.
///
/// **24 bytes, which is what keeps the record array 8-byte aligned.** Ten `u16` slots is 20,
/// and 24 + 32 + 20 leaves the records at an odd multiple of four -- so the two spare `u16`
/// are load-bearing rather than decoration, exactly as [`PlayerState`]'s own `_pad` is. They
/// are also the next feature's room: a count per slot is a `u16` per slot and would want a
/// block of its own anyway, which is the argument [`FLAG_BAR`] makes for being a bit.
///
/// **Its own block rather than five more words of [`PlayerState`]**, because a bar is not a
/// property of where the player is standing: the camera flags own the player in a capture and
/// nothing owns the bar, which is why `--resume` gates one and not the other. The boundary
/// batch 32 drew between the mode's camera and the file's player is drawn again here, one
/// field over, and it lands in a different place -- see `docs/persistence.md`.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BarState {
    pub slots: crate::block::Bar,
    _pad: [u16; 2],
}

impl BarState {
    pub fn new(slots: crate::block::Bar) -> Self {
        Self { slots, _pad: [0; 2] }
    }

    /// Refuse a bar this build cannot honour, at the file's edge and for the reason every
    /// other guard in this module is here: **this is the last place that still knows the
    /// value came out of a file**.
    ///
    /// The rule itself is `block::bar_slot_refusal`, one file over, because the flag parser
    /// has to apply the identical rule and a second copy would be a second rule. What is
    /// local to here is only that a file gets *refused* where `--hotbar` gets clamped: there
    /// is no human at the other end of a file who can see which block they got.
    fn check(&self, path: &Path) -> Result<(), String> {
        for (i, &id) in self.slots.iter().enumerate() {
            if let Some(why) = crate::block::bar_slot_refusal(id) {
                return Err(format!(
                    "{}: journal puts block {id} in bar slot {i}, and {why}",
                    path.display()
                ));
            }
        }
        Ok(())
    }
}

/// **The record array's alignment, which is the one property of this layout no reader can
/// check for itself.** `bytemuck` reads the records in place, so the bytes in front of them
/// have to come to a multiple of eight however many optional blocks are present -- and every
/// one of the four combinations is a file this build can be handed. A size assert on
/// [`BarState`] alone would not catch it: the quantity that matters is the sum.
const _: () = {
    let head = std::mem::size_of::<Header>();
    let player = std::mem::size_of::<PlayerState>();
    let bar = std::mem::size_of::<BarState>();
    assert!(head.is_multiple_of(8));
    assert!((head + player).is_multiple_of(8));
    assert!((head + bar).is_multiple_of(8));
    assert!((head + player + bar).is_multiple_of(8));
};

/// One edit as it sits in the file: 16 bytes, little-endian, both targets.
///
/// `y` is **internal** world Y -- the coordinate `set_block` takes, not the one the stats
/// overlay prints, which subtracts `math::Y_OFFSET`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Record {
    x: i32,
    y: i32,
    z: i32,
    id: u16,
    _pad: u16,
}

impl Record {
    fn at(p: IVec3, id: BlockId) -> Self {
        Self {
            x: p.x,
            y: p.y,
            z: p.z,
            id,
            _pad: 0,
        }
    }
}

/// One record's worth of file, and the offsets the append log rewrites in place. Taken from
/// the types rather than written down, because a field added to [`Header`] moves them.
const REC: u64 = std::mem::size_of::<Record>() as u64;
const HEAD: u64 = std::mem::size_of::<Header>() as u64;
const COUNT_AT: u64 = std::mem::offset_of!(Header, count) as u64;
const FLAGS_AT: u64 = std::mem::offset_of!(Header, flags) as u64;

/// How much file sits between the header and the records. **One expression, three readers**
/// -- the loader splitting the bytes, the append log seeking to the end of them and the
/// compacting save sizing its buffer -- because a file whose readers disagree about where the
/// records start is a file that replays every edit some bytes shifted and still looks like a
/// journal.
///
/// It became a sum in batch 34 and the shape is the point: each block is present or absent on
/// its own bit, so a version-2 file (player, no bar) and a version-3 one (both) are two values
/// of this function rather than two code paths.
fn extras_bytes(flags: u32) -> usize {
    let mut n = 0;
    if flags & FLAG_PLAYER != 0 {
        n += std::mem::size_of::<PlayerState>();
    }
    if flags & FLAG_BAR != 0 {
        n += std::mem::size_of::<BarState>();
    }
    n
}

/// The file `--edits` is appending to, so a session **killed** rather than closed keeps
/// every block it placed. Batch 33.
///
/// Batches 29 to 32 wrote the journal once, at the end, which means a crash, a kill or a
/// power cut threw the whole session away -- the defect `docs/errors.md` has carried since
/// 29 and the reason the roadmap called this "the one a player notices".
///
/// **The write order is the design.** One append is: mark the file, then the record, then
/// the count -- and each step leaves a file the reader can still make sense of:
///
/// 1. `FLAG_APPEND` goes in **before the first record**, not after it. A file carrying an
///    uncounted record *without* that bit is one [`EditJournal::load`] has to refuse, since
///    it is byte-for-byte what a truncation looks like; setting the bit first means the
///    permission to recover is always already on disk when there is something to recover.
/// 2. The **record before the count**, so the file never claims more than it holds. Killed
///    between the two, it holds one whole record nobody counted, which is recoverable.
///    The other order loses the same edit *and* fails the length check, blaming the file.
///
/// **No `sync_data` per edit**, which is a deliberate limit and not an oversight: the write
/// lands in the operating system's cache, which outlives the process but not the machine.
/// The roadmap item is a *killed session*; defending a power cut means a disk round trip for
/// every block placed, and that is a trade nobody has asked for.
struct AppendLog {
    file: std::fs::File,
    /// The header's `flags` as it stands on disk, so setting `FLAG_APPEND` cannot drop
    /// `FLAG_PLAYER` -- the word is rewritten whole and there is only one copy of it.
    flags: u32,
    /// Records the file physically holds, which is also what its `count` says after every
    /// completed append.
    count: u32,
}

impl AppendLog {
    /// Open `path` for appending, or create it as an empty journal for this world.
    ///
    /// **A new file gets no player block.** A world nobody has closed yet has no player to
    /// write, and `FLAG_PLAYER` is a claim that 32 bytes are there -- the exit save is what
    /// puts one in. Loading such a file back gives `None`, which is what "the file does not
    /// say" already means everywhere else in this module.
    fn open(path: &Path, seed: i32, sea_level: i32) -> Result<Self, String> {
        let io = |e: std::io::Error| format!("{}: {e}", path.display());
        if !path.exists() {
            let head = Header {
                magic: MAGIC,
                version: VERSION,
                seed,
                sea_level,
                count: 0,
                flags: 0,
            };
            std::fs::write(path, bytemuck::bytes_of(&head)).map_err(io)?;
        }
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(io)?;
        let mut bytes = [0u8; std::mem::size_of::<Header>()];
        file.read_exact(&mut bytes).map_err(io)?;
        let head: Header = *bytemuck::from_bytes(&bytes);
        // Every guard on the contents ran in `EditJournal::load` a moment ago, on the same
        // path. What is read back here is the *layout*: where the records start, and how
        // many of them the file holds -- which after a kill is not what `count` says.
        let body = HEAD + extras_bytes(head.flags) as u64;
        let len = file.metadata().map_err(io)?.len().max(body);
        let whole = (len - body) / REC;
        // Repair, in the one place that is allowed to: the reader decided how many whole
        // records there are, so the writer makes the file end on one. Without this an
        // append would land on top of half of a torn record and produce a file whose every
        // subsequent record is 16 bytes out of phase.
        if body + whole * REC != len {
            file.set_len(body + whole * REC).map_err(io)?;
        }
        let mut log = Self {
            file,
            flags: head.flags,
            count: head.count,
        };
        if u64::from(log.count) != whole {
            log.count = whole as u32;
            log.write_at(COUNT_AT, &log.count.to_le_bytes())?;
        }
        Ok(log)
    }

    fn write_at(&mut self, at: u64, bytes: &[u8]) -> Result<(), String> {
        self.file
            .seek(SeekFrom::Start(at))
            .and_then(|_| self.file.write_all(bytes))
            .map_err(|e| format!("edit journal: {e}"))
    }

    /// One edit, in the order the type's own comment argues for.
    fn append(&mut self, r: &Record) -> Result<(), String> {
        if self.flags & FLAG_APPEND == 0 {
            self.flags |= FLAG_APPEND;
            self.write_at(FLAGS_AT, &self.flags.to_le_bytes())?;
        }
        self.file
            .seek(SeekFrom::End(0))
            .and_then(|_| self.file.write_all(bytemuck::bytes_of(r)))
            .map_err(|e| format!("edit journal: {e}"))?;
        self.count += 1;
        self.write_at(COUNT_AT, &self.count.to_le_bytes())
    }
}

/// Every edit the world has taken, grouped by the chunk that will replay it.
///
/// Grouped per chunk because replay is per chunk, and **last-write-wins inside a chunk**,
/// which is what makes replay independent of the order the edits come back out of the map.
/// A journal with no ordering to preserve needs no ordering to be correct.
#[derive(Default)]
pub struct EditJournal {
    /// LOD-0 chunk -> dense voxel index -> the id last written there.
    edits: RwLock<FxHashMap<ChunkKey, FxHashMap<u32, BlockId>>>,
    /// LOD-0 chunks edited since the last drain, for [`Self::take_dirty`].
    ///
    /// Separate from `edits` because it answers a different question: `edits` is what the
    /// world *is*, this is what has changed since somebody last asked. A coarse chunk reads
    /// the journal once, at build time, so one already resident has no route to an edit that
    /// lands afterwards -- and it is the one the player is about to walk away from.
    dirty: RwLock<FxHashSet<ChunkKey>>,
    /// Where the player was, if anything has said. Batch 32.
    ///
    /// `None` is a journal that carries only deltas -- every file batch 29 or 30 wrote, and
    /// every one written by a mode that has no player to ask. It is deliberately not a
    /// default position: "the file does not say" and "the file says the spawn column" are
    /// different claims, and only the first one may fall back to `spawn_position`.
    player: RwLock<Option<PlayerState>>,
    /// What was in the player's bar, if anything has said. Batch 34. `None` means the
    /// default bar, which is the whole of what a pre-34 file not saying is.
    bar: RwLock<Option<crate::block::Bar>>,
    /// The open file `--edits` grows as edits land, or `None` -- which is every mode this
    /// engine has ever run except one flag. Batch 33.
    ///
    /// A `Mutex` rather than an `RwLock` because every user of it writes, and it is taken on
    /// the one code path in the engine that is paced by a human hand: a block placed is a
    /// mouse click, so a lock and two small writes cost nothing measurable, and holding the
    /// lock across both of them is what makes the record-then-count order an order rather
    /// than a race.
    log: Mutex<Option<AppendLog>>,
}

/// One coarse voxel's worth of edits, accumulated before anything is written.
///
/// Built per footprint rather than applied as they are read, because the three rules in
/// [`EditJournal::apply`] are all statements about the *set* of edits a coarse voxel covers:
/// which is topmost, and whether they account for every block the voxel stands for.
#[derive(Clone, Copy, Default)]
struct Footprint {
    /// The topmost solid edit, under a total order -- y descending, then z, then x.
    ///
    /// A total order rather than "whichever came back first" because the map hands its
    /// entries out in no particular order, and two runs of the same world have to paint the
    /// same voxel. It is the argument [`EditJournal::save`]'s sort makes, one level down.
    best: Option<(i32, i32, i32, BlockId)>,
    /// Edits to `AIR` at a position the coarse level's own heightfield calls solid. Edits
    /// above that surface are not counted: they clear nothing, and counting them would let a
    /// block placed and broken again in mid-air delete the ground under it.
    cleared: u32,
}

impl EditJournal {
    /// The journal a world starts with when nothing is loaded. Always present rather than an
    /// `Option`, so there is one code path and "persistence is off" is an empty map rather
    /// than a branch that can be left unwired.
    pub fn empty() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Record one accepted edit. Called from exactly one place -- the point in
    /// `World::set_block` where a write is known to change something -- because the two
    /// callers that edit (`App::handle_edits` and `build_demo_structure`) would otherwise be
    /// two places to forget, and a third would be added later.
    pub fn record(&self, p: IVec3, id: BlockId) {
        let key = ChunkKey::of_block(p);
        let l = p & 63;
        let idx = dense_index(l.x as usize, l.y as usize, l.z as usize) as u32;
        self.edits.write().entry(key).or_default().insert(idx, id);
        self.dirty.write().insert(key);
        // And onto the end of the file, if `--edits` attached a log. **Here rather than at
        // the two callers that edit**, for the same reason the map insert above is here:
        // there is one place a write is known to be real, and a second place to remember
        // would be a place to forget.
        //
        // The map has already taken the edit at this point, so a log that fails has cost the
        // session its crash-safety and nothing else -- the exit save still holds everything.
        // So it is reported once and **disarmed**, rather than reported per block placed:
        // a full disk would otherwise print one line per click for the rest of the session.
        let mut log = self.log.lock();
        if let Some(l) = log.as_mut() {
            if let Err(e) = l.append(&Record::at(p, id)) {
                eprintln!("{e} -- the append log is off; --edits still writes on exit");
                *log = None;
            }
        }
    }

    /// Hand the journal the file it appends to. `journal::open` is the only caller.
    fn attach_log(&self, log: AppendLog) {
        *self.log.lock() = Some(log);
    }

    /// Where the player was when this journal was last written, if it says.
    pub fn player(&self) -> Option<PlayerState> {
        *self.player.read()
    }

    /// Record where the player is. [`save_if_asked`] is the only caller in the binary, which
    /// is what makes the state in a file the state at the moment that file was written rather
    /// than whenever somebody last remembered to stamp it.
    pub fn set_player(&self, p: PlayerState) {
        *self.player.write() = Some(p);
    }

    /// What was in the player's bar when this journal was last written, if it says.
    ///
    /// `None` is every file written before batch 34 and every mode that has no player, and it
    /// means **the default bar** rather than an empty one -- `block::HOTBAR` is what a player
    /// who has never picked a block is holding, so "the file does not say" and "the file says
    /// the default" are the same world. That is the one place this field's story differs from
    /// [`Self::player`], where the two are different claims and only the first may fall back.
    pub fn bar(&self) -> Option<crate::block::Bar> {
        *self.bar.read()
    }

    /// Record what is in the bar. [`save_if_asked`] is the only caller, for the reason
    /// [`Self::set_player`] has only one: what lands in a file is what was true at the moment
    /// that file was written, not whenever somebody last remembered to stamp it.
    pub fn set_bar(&self, b: crate::block::Bar) {
        *self.bar.write() = Some(b);
    }

    /// The LOD-0 chunks edited since this was last called, and clear the set.
    ///
    /// Drained rather than read because the caller acts on each one exactly once.
    /// `ChunkManager::update` is the only caller: it walks each chunk's ancestors and
    /// rebuilds the coarse stand-ins that were interned before the edit existed. Loading a
    /// file does not go through `record`, so `--load-edits` dirties nothing -- correctly,
    /// since at that point nothing has been built yet.
    pub fn take_dirty(&self) -> Vec<ChunkKey> {
        let mut d = self.dirty.write();
        d.drain().collect()
    }

    /// Replay this chunk's edits into a freshly generated dense array. Returns how many
    /// voxels the journal changed, which is what the load message counts.
    ///
    /// At LOD 0 that is a straight write per edit. **At a coarse level each voxel stands for
    /// `s^3` blocks and the edits inside it have to be aggregated**, which is the same
    /// bargain `coarse_trees` and `coarse_meadow` already strike one file over: a coarse
    /// level is allowed to disagree about *which* cells, never about whether anything is
    /// there. Three rules, and every one of them is `generate_coarse`'s own rule read back:
    ///
    /// 1. **The topmost occupied block wins.** `generate_coarse` says exactly this about
    ///    terrain -- a voxel straddling a shoreline reads as water -- so a solid edit takes
    ///    the voxel when it sits at or above the terrain's own top block inside that
    ///    footprint, and is ignored below it, where it is behind an opaque face anyway.
    /// 2. **A voxel clears only when every block it stands for is gone.** One block broken
    ///    out of 512 is not a hole at stride 8, and pretending otherwise would punch through
    ///    a hillside from a distance.
    /// 3. **"Every block it stands for" is counted against the coarse heightfield**, not
    ///    against LOD 0. The coarse levels have no caves in them, so a count taken from the
    ///    fine world would be a count of blocks this representation never had -- the test
    ///    has to be consistent with the thing it is clearing.
    ///
    /// `heights` is the array `WorldGen::generate` just filled, indexed by *coarse* column,
    /// so the aggregate reads the surface this chunk was actually built from rather than
    /// sampling the field a second time and inviting the two to disagree.
    pub fn apply(
        &self,
        key: ChunkKey,
        dense: &mut [BlockId],
        gen: &WorldGen,
        heights: &[i32; 64 * 64],
    ) -> usize {
        debug_assert_eq!(dense.len(), VOL);
        if key.lod != 0 {
            return self.apply_coarse(key, dense, gen, heights);
        }
        let map = self.edits.read();
        let Some(chunk) = map.get(&key) else {
            return 0;
        };
        for (&idx, &id) in chunk {
            dense[idx as usize] = id;
        }
        chunk.len()
    }

    /// Rules 1-3 above. Two passes, because the first cannot know what it is competing with
    /// until it has seen every edit in the footprint.
    ///
    /// The scan is over the journal's own chunks rather than over the volume: a LOD-4 chunk
    /// stands over 4096 LOD-0 ones and the overwhelming majority of them are untouched. That
    /// makes the cost **`O(chunks edited)` per coarse chunk built**, which is the right shape
    /// for a player's session and the wrong one for a journal with a hundred thousand chunks
    /// in it. The empty-map early-out is what keeps every capture ever taken free of it.
    fn apply_coarse(
        &self,
        key: ChunkKey,
        dense: &mut [BlockId],
        gen: &WorldGen,
        heights: &[i32; 64 * 64],
    ) -> usize {
        let map = self.edits.read();
        if map.is_empty() {
            return 0;
        }
        let s = key.voxel_size();
        let o = key.origin();
        // The LOD-0 chunks this one stands over, in LOD-0 chunk units.
        let span = 1i32 << key.lod;
        let lo = key.pos * span;
        let hi = lo + IVec3::splat(span);

        let mut acc: FxHashMap<usize, Footprint> = FxHashMap::default();
        for (ck, chunk) in map.iter() {
            if ck.pos.cmplt(lo).any() || ck.pos.cmpge(hi).any() {
                continue;
            }
            let co = ck.origin();
            for (&idx, &id) in chunk {
                let (lx, ly, lz) = unpack_index(idx);
                let w = co + IVec3::new(lx as i32, ly as i32, lz as i32);
                // Exact, not rounded: `o` is a multiple of `s * 64` and `w` is inside the
                // chunk, so this lands in 0..64 on every axis by construction.
                let v = (w - o) / s;
                let (vx, vy, vz) = (v.x as usize, v.y as usize, v.z as usize);
                let e = acc.entry(dense_index(vx, vy, vz)).or_default();
                if id != AIR {
                    let rank = (Reverse(w.y), w.z, w.x);
                    if e.best
                        .is_none_or(|(by, bz, bx, _)| rank < (Reverse(by), bz, bx))
                    {
                        e.best = Some((w.y, w.z, w.x, id));
                    }
                } else if w.y < gen.column_top(heights[vx + vz * 64]) {
                    e.cleared += 1;
                }
            }
        }

        let mut n = 0;
        for (di, e) in acc {
            let (x, y, z) = unpack_index(di as u32);
            let y0 = o.y + y as i32 * s;
            // How much terrain this voxel stands for: the coarse column is solid from the
            // bottom of the chunk to `column_top` and empty above it, so the box holds
            // `filled` layers of it, `s * s` blocks each.
            let filled = (gen.column_top(heights[x + z * 64]) - y0).clamp(0, s);
            if let Some((by, _, _, id)) = e.best {
                // `y0 + filled - 1` is the terrain's own topmost block in this box, and
                // `y0 - 1` when there is none -- which every edit clears, so a structure
                // standing in open air always wins its voxel.
                if by >= y0 + filled - 1 {
                    dense[di] = id;
                    n += 1;
                }
            } else if e.cleared > 0 && e.cleared >= (filled * s * s) as u32 {
                dense[di] = AIR;
                n += 1;
            }
        }
        n
    }

    pub fn len(&self) -> usize {
        self.edits.read().values().map(|c| c.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.edits.read().values().all(|c| c.is_empty())
    }

    /// Chunks holding at least one edit. The half of the load message that says *where*.
    pub fn chunks(&self) -> usize {
        self.edits.read().len()
    }

    /// Read a journal, refusing anything that was not written for this world.
    pub fn load(path: &Path, seed: i32, sea_level: i32) -> Result<Arc<Self>, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if bytes.len() < std::mem::size_of::<Header>() {
            return Err(format!("{}: too short to be an edit journal", path.display()));
        }
        let (head, rest) = bytes.split_at(std::mem::size_of::<Header>());
        let head: Header = *bytemuck::from_bytes(head);
        if head.magic != MAGIC {
            return Err(format!("{}: not an edit journal", path.display()));
        }
        // Every version this build knows, not just the one it writes, and the reason is at
        // `VERSION`: a version-1 file's spare word is a zero it wrote itself, so it announces
        // "no player block" through the same field a version-2 file uses to announce one.
        // Two versions, one reader, no branch on the number.
        if head.version == 0 || head.version > VERSION {
            return Err(format!(
                "{}: journal version {}, this build reads 1..={VERSION}",
                path.display(),
                head.version
            ));
        }
        if head.seed != seed || head.sea_level != sea_level {
            return Err(format!(
                "{}: journal is for seed {} sea-level {}, this world is seed {seed} sea-level \
                 {sea_level} -- the edits would land on terrain that is not there",
                path.display(),
                head.seed,
                head.sea_level
            ));
        }
        // The player and bar blocks sit between the header and the records, so the length
        // check below cannot call a file the wrong size until it knows what is there.
        // Splitting rather than indexing also keeps the record slice's start a multiple of
        // eight by construction, which is what lets `bytemuck` read it in place.
        let extras = extras_bytes(head.flags);
        if rest.len() < extras {
            return Err(format!(
                "{}: the header claims a player or bar block the file is too short to hold",
                path.display()
            ));
        }
        let (extras, rest) = rest.split_at(extras);
        // **In the order the bits are numbered, which is the order they were added.** A
        // reader that took them in any other order would work until the day a file had one
        // block and not the other, which is every file batches 29 to 33 wrote.
        let (player, extras) = if head.flags & FLAG_PLAYER != 0 {
            let (p, r) = extras.split_at(std::mem::size_of::<PlayerState>());
            (Some(*bytemuck::from_bytes::<PlayerState>(p)), r)
        } else {
            (None, extras)
        };
        let bar = if head.flags & FLAG_BAR != 0 {
            Some(*bytemuck::from_bytes::<BarState>(extras))
        } else {
            None
        };
        if let Some(p) = &player {
            p.check(path)?;
        }
        if let Some(b) = &bar {
            b.check(path)?;
        }
        let want = head.count as usize * std::mem::size_of::<Record>();
        // **The strict check, and the one writer allowed past it.** An append log leaves a
        // file longer than its header claims and never shorter, because it writes the record
        // before the count (see [`AppendLog`]) -- so whole records past `want` are edits a
        // killed session made and nobody counted, and replaying them is the entire feature.
        // Dropping them instead would make `--edits` crash-safe in every case except the one
        // it exists for.
        //
        // Both directions of the original check survive: *fewer* bytes than the count claims
        // is still a truncation whatever the flags say, and *more* without `FLAG_APPEND` is
        // still a file this module did not write. That is what the bit is for.
        let appended = head.flags & FLAG_APPEND != 0;
        let rest = if rest.len() == want || (appended && rest.len() > want) {
            let whole = rest.len() / std::mem::size_of::<Record>();
            let recovered = whole - head.count as usize;
            let torn = rest.len() - whole * std::mem::size_of::<Record>();
            if recovered > 0 {
                println!(
                    "{}: recovered {recovered} edit(s) the header had not counted -- \
                     the session was killed rather than closed",
                    path.display()
                );
            }
            if torn > 0 {
                println!(
                    "{}: dropped {torn} trailing byte(s) of a half-written edit",
                    path.display()
                );
            }
            &rest[..whole * std::mem::size_of::<Record>()]
        } else {
            return Err(format!(
                "{}: header says {} edits ({want} bytes), file carries {}",
                path.display(),
                head.count,
                rest.len()
            ));
        };
        let journal = Self::default();
        *journal.player.write() = player;
        *journal.bar.write() = bar.map(|b| b.slots);
        {
            // **In file order, so the last record at a position wins.** For a file
            // [`Self::save`] wrote that is a statement about nothing: the map it came from
            // holds one entry per position, so the records are already unique and the sort
            // is only there to make the bytes reproducible. For an append log it is the
            // whole of the semantics -- place, break, place again is three records at one
            // position, and the file's order is the order they happened in.
            let mut map = journal.edits.write();
            for r in bytemuck::cast_slice::<u8, Record>(rest) {
                let p = IVec3::new(r.x, r.y, r.z);
                let l = p & 63;
                let idx = dense_index(l.x as usize, l.y as usize, l.z as usize) as u32;
                map.entry(ChunkKey::of_block(p))
                    .or_default()
                    .insert(idx, r.id);
            }
        }
        Ok(Arc::new(journal))
    }

    /// Write the whole journal, loaded entries and this session's alike.
    ///
    /// **Sorted, so the same world writes the same bytes.** The map's iteration order is not
    /// stable across runs, and an artifact that differs for no reason is one nothing can
    /// diff -- which is the same argument the capture manifest's hashes make.
    pub fn save(&self, path: &Path, seed: i32, sea_level: i32) -> Result<(), String> {
        let mut records: Vec<Record> = Vec::with_capacity(self.len());
        for (key, chunk) in self.edits.read().iter() {
            let o = key.origin();
            for (&idx, &id) in chunk {
                let (x, y, z) = unpack_index(idx);
                records.push(Record::at(
                    o + IVec3::new(x as i32, y as i32, z as i32),
                    id,
                ));
            }
        }
        records.sort_unstable_by_key(|r| (r.x, r.z, r.y));
        let player = *self.player.read();
        let bar = *self.bar.read();
        let head = Header {
            magic: MAGIC,
            version: VERSION,
            seed,
            sea_level,
            count: records.len() as u32,
            flags: if player.is_some() { FLAG_PLAYER } else { 0 }
                | if bar.is_some() { FLAG_BAR } else { 0 },
        };
        let mut out = Vec::with_capacity(
            std::mem::size_of::<Header>()
                + extras_bytes(head.flags)
                + records.len() * std::mem::size_of::<Record>(),
        );
        out.extend_from_slice(bytemuck::bytes_of(&head));
        // **In the order the bits are numbered**, which is the order `load` splits them in
        // and the only thing making the two halves one format.
        if let Some(p) = &player {
            out.extend_from_slice(bytemuck::bytes_of(p));
        }
        if let Some(b) = bar {
            out.extend_from_slice(bytemuck::bytes_of(&BarState::new(b)));
        }
        out.extend_from_slice(bytemuck::cast_slice(&records));
        std::fs::write(path, &out).map_err(|e| format!("{}: {e}", path.display()))?;
        // **This write is a compaction, and it moves the ground under the append log.** The
        // file just written has one record per position, an exact `count` and no
        // `FLAG_APPEND`; the log's open handle described the file that was replaced, and
        // appending against a stale length is how a save file gets a record 16 bytes out of
        // phase. So the log is re-opened onto what is now there.
        //
        // Only when there is one, so `--save-edits` writes exactly the bytes and takes
        // exactly the steps it took in batch 29. And the reason this is not merely tidy: a
        // capture calls `save_if_asked` *before* the frame it captures, so a mode saving and
        // then continuing to edit is a thing that already happens.
        let mut log = self.log.lock();
        if log.is_some() {
            *log = Some(AppendLog::open(path, seed, sea_level)?);
        }
        Ok(())
    }
}

/// The inverse of [`dense_index`], which is the only place the layout is written down.
///
/// Here rather than beside it because this is the only caller: the forward direction is on
/// every generation path and this one runs once per edit at save time.
/// `dense_index_round_trips` in `tests/edit_journal.rs` holds the two together.
fn unpack_index(i: u32) -> (usize, usize, usize) {
    (
        (i & 63) as usize,
        ((i >> 12) & 63) as usize,
        ((i >> 6) & 63) as usize,
    )
}

/// The journal a run starts from: whatever `--edits` or `--load-edits` names, or an empty
/// one -- and, under `--edits`, the open file the rest of the session appends to.
///
/// Under `--load-edits` a missing file is an error, for the same reason a seed mismatch is: a
/// typo in the path would otherwise render an unedited world and say nothing. Under `--edits`
/// a missing file is **the ordinary first run**, and the only thing standing between those
/// two readings of one path is the line this prints, which is why it prints one.
pub fn open(cfg: &Config) -> Result<Arc<EditJournal>, String> {
    // Refused rather than resolved by precedence. `--edits PATH --save-edits OTHER` has two
    // readable answers -- write both, write the later one -- and a flag combination whose
    // meaning a reader has to guess is one `Config::journal_out` would have to guess too.
    if cfg.edits.is_some() && (cfg.load_edits.is_some() || cfg.save_edits.is_some()) {
        return Err(
            "--edits is both halves at one path: drop it, or drop --load-edits/--save-edits".into(),
        );
    }
    let Some((path, must_exist)) = cfg.journal_in() else {
        if cfg.resume {
            return Err(
                "--resume has no journal to resume from: pass --load-edits PATH as well".into(),
            );
        }
        // No journal named, and `--bar` still has to be honoured and still has to be
        // refused when it is malformed. A run with no persistence is the ordinary way to
        // capture a bar, and a flag validated only when a file happened to be named would
        // be a flag nothing checked on the path the fixture actually takes.
        let j = EditJournal::empty();
        if let Some(spec) = &cfg.bar {
            j.set_bar(parse_bar(spec)?);
        }
        return Ok(j);
    };
    let j = if must_exist || path.exists() {
        // `load`'s own error carries the file-system message when the path is a typo, which
        // is the half of the guard a sentence here would have to reproduce.
        let j = EditJournal::load(path, cfg.seed, cfg.sea_level)?;
        println!(
            "loaded {} edit(s) in {} chunk(s) from {}",
            j.len(),
            j.chunks(),
            path.display()
        );
        j
    } else {
        // **Said out loud, every time.** "Loaded your world" and "started a new one" are the
        // two outcomes of `--edits`, they look identical from inside the renderer, and a
        // mistyped path picks the second silently. `errors.md`'s rule about a frame that
        // looks exactly like a frame applies to a *world* that looks exactly like a world.
        println!(
            "no journal at {} yet -- starting a new world there",
            path.display()
        );
        EditJournal::empty()
    };
    // A `--resume` with nothing to resume from is refused rather than quietly ignored, for
    // the reason a missing `--load-edits` file is an error: it would render the default
    // vantage, print nothing about it, and be a frame that looks exactly like a frame.
    if cfg.resume && j.player().is_none() {
        return Err(format!(
            "{}: --resume, but this journal carries no player -- it predates batch 32, \
             or was written by a mode that has no player in it",
            path.display()
        ));
    }
    if let Some(pl) = j.player() {
        println!(
            "  player at {:.1} {:.1} {:.1}, hotbar {}{}",
            pl.pos[0],
            pl.pos[1] - crate::math::Y_OFFSET as f32,
            pl.pos[2],
            pl.hotbar,
            if cfg.resume { " (resumed)" } else { "" }
        );
    }
    // **`--bar` is refused against a file that already carries one, rather than resolved by
    // precedence**, for the reason `--edits` alongside `--load-edits` is: the two have two
    // readable answers and the flag would lose silently -- this module's recurring failure
    // mode, a world that looks exactly like a world. A new world under `--edits` carries no
    // bar yet, so authoring one there is the ordinary case and is untouched by it.
    //
    // What an accepted `--bar` does is **set the journal's bar**, not keep a third copy: from
    // here on there is one answer to what is in the bar, `bar_for` reads it and the exit save
    // writes it back. A flag that authored a bar the file never heard about would be a bar
    // that vanished on the next load.
    if let Some(spec) = &cfg.bar {
        if j.bar().is_some() {
            return Err(format!(
                "{}: this journal already carries a bar and --bar names another -- drop the flag to play the world's own bar",
                path.display()
            ));
        }
        j.set_bar(parse_bar(spec)?);
    }
    if let Some(b) = j.bar() {
        println!("  bar: {}", bar_names(&b));
    }
    // Last, and after every refusal above: a log attached to a world the run is about to
    // reject would create the file `--edits` named on the way out of a failed start.
    if cfg.edits.is_some() {
        j.attach_log(AppendLog::open(path, cfg.seed, cfg.sea_level)?);
    }
    Ok(j)
}

/// Honour `--save-edits`, wherever a mode ends.
///
/// **Takes the player rather than reading one off the journal**, so what lands in the file is
/// where the player was at the moment of the write, and so there is no way to write a journal
/// without stamping one. Three callers -- the window's `exiting`, `--screenshot` and
/// `--bench-frames` -- and the signature is what stops a fourth from being the one that
/// forgot. That is `scene::flags_from`'s argument, one file over, and it is the same argument
/// `EditJournal::record` makes about there being exactly one recording site.
pub fn save_if_asked(
    cfg: &Config,
    j: &EditJournal,
    player: PlayerState,
    bar: crate::block::Bar,
) -> Result<(), String> {
    let Some(path) = cfg.journal_out() else {
        return Ok(());
    };
    j.set_player(player);
    // Stamped here and not read off the journal, which is the argument above applied to the
    // field batch 34 added: a bar the player changed after the load would otherwise be a bar
    // the file never hears about.
    j.set_bar(bar);
    j.save(path, cfg.seed, cfg.sea_level)?;
    println!("saved {} edit(s) to {}", j.len(), path.display());
    Ok(())
}

/// The bar a run starts with: the journal's if anything has set one, else the build's own
/// `block::HOTBAR`.
///
/// **One function because there are two callers and they must not disagree** -- the window
/// and the capture path -- which is the argument `Config::journal_in` makes about its three
/// flags. `--bar` is not consulted here because `journal::open` has already folded it into
/// the journal, so there is exactly one place that answers this question.
///
/// The precedence is the other half of batch 32's boundary, and it lands on the opposite side
/// of it. A loaded *camera* is refused unless `--resume` asks for it, because the fourteen
/// named vantages **are** the camera flags and a journal that moved one silently would turn
/// every vantage into "wherever the file says". Nothing in the fixture is a bar, so a loaded
/// bar needs no permission -- the boundary is the mode's, and a bar is not the mode's.
pub fn bar_for(j: &EditJournal) -> crate::block::Bar {
    j.bar().unwrap_or(crate::block::HOTBAR)
}

/// `--bar`'s ten block ids, refused as a whole rather than clamped.
///
/// **`--hotbar` clamps, and the difference is what the value indexes.** A slot *number* out of
/// range has an obvious nearest legal answer and a human at the other end who can see which
/// cell they got. A block *id* has none: the nearest legal id is a different block, and a bar
/// quietly holding sand where the command line said tall grass is a capture that looks exactly
/// like the capture that was asked for.
fn parse_bar(spec: &str) -> Result<crate::block::Bar, String> {
    let mut out = crate::block::HOTBAR;
    let fields: Vec<&str> = spec.split(',').collect();
    if fields.len() != crate::block::SLOTS {
        return Err(format!(
            "--bar takes {} comma-separated block ids, got {}",
            crate::block::SLOTS,
            fields.len()
        ));
    }
    for (i, f) in fields.iter().enumerate() {
        let id: crate::block::BlockId = f
            .trim()
            .parse()
            .map_err(|_| format!("--bar slot {i}: {f:?} is not a block id"))?;
        if let Some(why) = crate::block::bar_slot_refusal(id) {
            return Err(format!("--bar slot {i}: block {id}, and {why}"));
        }
        out[i] = id;
    }
    Ok(out)
}

/// The bar as block names, for the one line `journal::open` prints about it. Names rather than
/// ids because the ids are what `--bar` takes and the names are what a reader can check
/// against the world in front of them.
fn bar_names(b: &crate::block::Bar) -> String {
    b.iter()
        .map(|&id| crate::block::def(id).name)
        .collect::<Vec<_>>()
        .join(", ")
}



