//! Decoupled per-voxel attributes (block ids) and the shared brick/word pool.
//!
//! Each leaf (4^3 cell) owns one 32-bit table entry: either an inline uniform block id
//! (bit 31 set) or a word offset to a palette block in the pool. Light bricks use the
//! same pool.

use crate::block::BlockId;

pub const UNIFORM_BIT: u32 = 1 << 31;
const MIN_ALLOC: usize = 4;

/// Word pool with power-of-two size classes and free lists.
pub struct WordPool {
    data: Vec<u32>,
    free: Vec<Vec<u32>>,
    dirty: Vec<(u32, u32)>,
    pub used_words: usize,
}

impl Default for WordPool {
    fn default() -> Self {
        Self::new()
    }
}

fn class_of(len: usize) -> (usize, usize) {
    let size = len.max(MIN_ALLOC).next_power_of_two();
    (size.trailing_zeros() as usize, size)
}

impl WordPool {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            free: (0..32).map(|_| Vec::new()).collect(),
            dirty: Vec::new(),
            used_words: 0,
        }
    }

    #[inline]
    pub fn data(&self) -> &[u32] {
        &self.data
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn bytes(&self) -> usize {
        self.data.len() * 4
    }

    #[inline]
    pub fn get(&self, off: u32, len: usize) -> &[u32] {
        &self.data[off as usize..off as usize + len]
    }

    pub fn alloc(&mut self, len: usize) -> u32 {
        let (cls, size) = class_of(len);
        self.used_words += size;
        if let Some(off) = self.free[cls].pop() {
            return off;
        }
        let off = self.data.len();
        self.data.resize(off + size, 0);
        off as u32
    }

    pub fn free(&mut self, off: u32, len: usize) {
        let (cls, size) = class_of(len);
        self.used_words -= size;
        self.free[cls].push(off);
    }

    pub fn write(&mut self, off: u32, words: &[u32]) {
        let o = off as usize;
        self.data[o..o + words.len()].copy_from_slice(words);
        self.dirty.push((off, words.len() as u32));
    }

    pub fn alloc_write(&mut self, words: &[u32]) -> u32 {
        let off = self.alloc(words.len());
        self.write(off, words);
        off
    }

    pub fn take_dirty(&mut self) -> Vec<(u32, u32)> {
        std::mem::take(&mut self.dirty)
    }
}

fn bits_for(n: usize) -> u32 {
    match n {
        0..=2 => 1,
        3..=4 => 2,
        5..=16 => 4,
        _ => 8,
    }
}

/// Encode the block ids of one leaf. `present` marks which of the 64 voxels exist.
/// Returns an inline uniform entry, or the offset (relative to `out`) of a palette block
/// appended to `out`.
// `i` indexes `ids` and is *also* the voxel's bit position in `present` and its slot in the
// packed index array. Iterating the slice instead, as clippy suggests, throws that away.
#[allow(clippy::needless_range_loop)]
pub fn encode_leaf(ids: &[BlockId; 64], present: u64, out: &mut Vec<u32>) -> u32 {
    let mut pal: [BlockId; 64] = [0; 64];
    let mut n = 0usize;
    for i in 0..64 {
        if (present >> i) & 1 != 0 {
            let id = ids[i];
            if !pal[..n].contains(&id) {
                pal[n] = id;
                n += 1;
            }
        }
    }
    if n <= 1 {
        return UNIFORM_BIT | pal[0] as u32;
    }
    let bits = bits_for(n);
    let off = out.len() as u32;
    out.push(n as u32 | (bits << 8));
    for w in 0..n.div_ceil(2) {
        let lo = pal[2 * w] as u32;
        let hi = if 2 * w + 1 < n {
            pal[2 * w + 1] as u32
        } else {
            0
        };
        out.push(lo | (hi << 16));
    }
    let idx_words = (64 * bits as usize) / 32;
    let idx_base = out.len();
    out.resize(idx_base + idx_words, 0);
    for i in 0..64 {
        if (present >> i) & 1 == 0 {
            continue;
        }
        let idx = pal[..n].iter().position(|&p| p == ids[i]).unwrap() as u32;
        let bitpos = i as u32 * bits;
        out[idx_base + (bitpos >> 5) as usize] |= idx << (bitpos & 31);
    }
    off
}

/// Decode one voxel from a table entry.
#[inline]
pub fn decode_leaf(words: &[u32], entry: u32, voxel_bit: u32) -> BlockId {
    if entry & UNIFORM_BIT != 0 {
        return entry as BlockId;
    }
    let base = entry as usize;
    let hdr = words[base];
    let n = (hdr & 0xFF) as usize;
    let bits = (hdr >> 8) & 0xFF;
    let idx_base = base + 1 + n.div_ceil(2);
    let bitpos = voxel_bit * bits;
    let w = words[idx_base + (bitpos >> 5) as usize];
    let idx = ((w >> (bitpos & 31)) & ((1u32 << bits) - 1)) as usize;
    let pw = words[base + 1 + idx / 2];
    (pw >> ((idx & 1) * 16)) as BlockId
}

/// Number of words a palette block occupies (0 for inline entries).
pub fn block_words(words: &[u32], entry: u32) -> usize {
    if entry & UNIFORM_BIT != 0 {
        return 0;
    }
    let hdr = words[entry as usize];
    let n = (hdr & 0xFF) as usize;
    let bits = ((hdr >> 8) & 0xFF) as usize;
    1 + n.div_ceil(2) + (64 * bits) / 32
}



