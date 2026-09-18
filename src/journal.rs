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

pub const MAGIC: u32 = u32::from_le_bytes(*b"VXJ1");

pub const VERSION: u32 = 3;

const FLAG_PLAYER: u32 = 1;

const FLAG_APPEND: u32 = 2;

const FLAG_BAR: u32 = 4;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Header {
    magic: u32,
    version: u32,
    seed: i32,
    sea_level: i32,
    count: u32,

    flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PlayerState {
    pub pos: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub hotbar: u32,

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

const _: () = {
    let head = std::mem::size_of::<Header>();
    let player = std::mem::size_of::<PlayerState>();
    let bar = std::mem::size_of::<BarState>();
    assert!(head.is_multiple_of(8));
    assert!((head + player).is_multiple_of(8));
    assert!((head + bar).is_multiple_of(8));
    assert!((head + player + bar).is_multiple_of(8));
};

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

const REC: u64 = std::mem::size_of::<Record>() as u64;
const HEAD: u64 = std::mem::size_of::<Header>() as u64;
const COUNT_AT: u64 = std::mem::offset_of!(Header, count) as u64;
const FLAGS_AT: u64 = std::mem::offset_of!(Header, flags) as u64;

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

struct AppendLog {
    file: std::fs::File,

    flags: u32,

    count: u32,
}

impl AppendLog {

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

        let body = HEAD + extras_bytes(head.flags) as u64;
        let len = file.metadata().map_err(io)?.len().max(body);
        let whole = (len - body) / REC;

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

#[derive(Default)]
pub struct EditJournal {

    edits: RwLock<FxHashMap<ChunkKey, FxHashMap<u32, BlockId>>>,

    dirty: RwLock<FxHashSet<ChunkKey>>,

    player: RwLock<Option<PlayerState>>,

    bar: RwLock<Option<crate::block::Bar>>,

    log: Mutex<Option<AppendLog>>,
}

#[derive(Clone, Copy, Default)]
struct Footprint {

    best: Option<(i32, i32, i32, BlockId)>,

    cleared: u32,
}

impl EditJournal {

    pub fn empty() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn record(&self, p: IVec3, id: BlockId) {
        let key = ChunkKey::of_block(p);
        let l = p & 63;
        let idx = dense_index(l.x as usize, l.y as usize, l.z as usize) as u32;
        self.edits.write().entry(key).or_default().insert(idx, id);
        self.dirty.write().insert(key);

        let mut log = self.log.lock();
        if let Some(l) = log.as_mut() {
            if let Err(e) = l.append(&Record::at(p, id)) {
                eprintln!("{e} -- the append log is off; --edits still writes on exit");
                *log = None;
            }
        }
    }

    fn attach_log(&self, log: AppendLog) {
        *self.log.lock() = Some(log);
    }

    pub fn player(&self) -> Option<PlayerState> {
        *self.player.read()
    }

    pub fn set_player(&self, p: PlayerState) {
        *self.player.write() = Some(p);
    }

    pub fn bar(&self) -> Option<crate::block::Bar> {
        *self.bar.read()
    }

    pub fn set_bar(&self, b: crate::block::Bar) {
        *self.bar.write() = Some(b);
    }

    pub fn take_dirty(&self) -> Vec<ChunkKey> {
        let mut d = self.dirty.write();
        d.drain().collect()
    }

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

            let filled = (gen.column_top(heights[x + z * 64]) - y0).clamp(0, s);
            if let Some((by, _, _, id)) = e.best {

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

    pub fn chunks(&self) -> usize {
        self.edits.read().len()
    }

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

        let extras = extras_bytes(head.flags);
        if rest.len() < extras {
            return Err(format!(
                "{}: the header claims a player or bar block the file is too short to hold",
                path.display()
            ));
        }
        let (extras, rest) = rest.split_at(extras);

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

        if let Some(p) = &player {
            out.extend_from_slice(bytemuck::bytes_of(p));
        }
        if let Some(b) = bar {
            out.extend_from_slice(bytemuck::bytes_of(&BarState::new(b)));
        }
        out.extend_from_slice(bytemuck::cast_slice(&records));
        std::fs::write(path, &out).map_err(|e| format!("{}: {e}", path.display()))?;

        let mut log = self.log.lock();
        if log.is_some() {
            *log = Some(AppendLog::open(path, seed, sea_level)?);
        }
        Ok(())
    }
}

fn unpack_index(i: u32) -> (usize, usize, usize) {
    (
        (i & 63) as usize,
        ((i >> 12) & 63) as usize,
        ((i >> 6) & 63) as usize,
    )
}

pub fn open(cfg: &Config) -> Result<Arc<EditJournal>, String> {

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

        let j = EditJournal::empty();
        if let Some(spec) = &cfg.bar {
            j.set_bar(parse_bar(spec)?);
        }
        return Ok(j);
    };
    let j = if must_exist || path.exists() {

        let j = EditJournal::load(path, cfg.seed, cfg.sea_level)?;
        println!(
            "loaded {} edit(s) in {} chunk(s) from {}",
            j.len(),
            j.chunks(),
            path.display()
        );
        j
    } else {

        println!(
            "no journal at {} yet -- starting a new world there",
            path.display()
        );
        EditJournal::empty()
    };

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

    if cfg.edits.is_some() {
        j.attach_log(AppendLog::open(path, cfg.seed, cfg.sea_level)?);
    }
    Ok(j)
}

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

    j.set_bar(bar);
    j.save(path, cfg.seed, cfg.sea_level)?;
    println!("saved {} edit(s) to {}", j.len(), path.display());
    Ok(())
}

pub fn bar_for(j: &EditJournal) -> crate::block::Bar {
    j.bar().unwrap_or(crate::block::HOTBAR)
}

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

fn bar_names(b: &crate::block::Bar) -> String {
    b.iter()
        .map(|&id| crate::block::def(id).name)
        .collect::<Vec<_>>()
        .join(", ")
}
