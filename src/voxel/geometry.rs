use bytemuck::{Pod, Zeroable};
use hashbrown::HashTable;
use rustc_hash::FxHasher;
use std::hash::Hasher;

pub const FULL_FLAG: u32 = 1 << 31;

pub const PREFIX_MASK: u32 = 0x0FFF_FFFF;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Pod, Zeroable)]
pub struct Inner {
    pub mask: u64,

    pub run_ptr: u32,

    pub leaf_prefix: u32,
}

impl Inner {
    pub const EMPTY: Inner = Inner {
        mask: 0,
        run_ptr: 0,
        leaf_prefix: 0,
    };

    #[inline]
    pub fn full(leaf_prefix: u32) -> Inner {
        Inner {
            mask: u64::MAX,
            run_ptr: FULL_FLAG,
            leaf_prefix,
        }
    }
    #[inline]
    pub fn is_full(&self) -> bool {
        self.run_ptr & FULL_FLAG != 0
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.mask == 0
    }
    #[inline]
    pub fn run(&self) -> u32 {
        self.run_ptr & !FULL_FLAG
    }
    #[inline]
    pub fn count(&self) -> u32 {
        self.mask.count_ones()
    }
}

#[inline]
pub fn below(mask: u64, bit: u32) -> u32 {
    (mask & ((1u64 << bit) - 1)).count_ones()
}

#[inline]
pub fn has(mask: u64, bit: u32) -> bool {
    (mask >> bit) & 1 != 0
}

pub trait RunElem: Pod + Eq + std::hash::Hash + Send + Sync + 'static {}
impl RunElem for u64 {}
impl RunElem for Inner {}

fn hash_run<T: Pod>(run: &[T]) -> u64 {
    let mut h = FxHasher::default();
    h.write_usize(run.len());
    h.write(bytemuck::cast_slice::<T, u8>(run));
    h.finish()
}

pub struct RunPool<T: RunElem> {
    data: Vec<T>,
    refs: Vec<u32>,
    lens: Vec<u8>,
    table: HashTable<u32>,
    free: Vec<Vec<u32>>,
    dirty: Vec<(u32, u32)>,

    pub live_elems: usize,

    pub referenced_elems: u64,
}

impl<T: RunElem> Default for RunPool<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: RunElem> RunPool<T> {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            refs: Vec::new(),
            lens: Vec::new(),
            table: HashTable::new(),
            free: (0..=64).map(|_| Vec::new()).collect(),
            dirty: Vec::new(),
            live_elems: 0,
            referenced_elems: 0,
        }
    }

    #[inline]
    pub fn data(&self) -> &[T] {
        &self.data
    }

    #[inline]
    pub fn get(&self, off: u32, len: u32) -> &[T] {
        &self.data[off as usize..(off + len) as usize]
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn bytes(&self) -> usize {
        self.data.len() * std::mem::size_of::<T>()
    }

    pub fn live_runs(&self) -> usize {
        self.table.len()
    }

    pub fn intern(&mut self, run: &[T]) -> (u32, bool) {
        debug_assert!(!run.is_empty() && run.len() <= 64);
        let h = hash_run(run);
        let data = &self.data;
        let lens = &self.lens;
        let found = self
            .table
            .find(h, |&o| {
                let o = o as usize;

                lens[o] as usize == run.len() && &data[o..o + run.len()] == run
            })
            .copied();
        if let Some(off) = found {
            self.refs[off as usize] += 1;
            self.referenced_elems += run.len() as u64;
            return (off, false);
        }
        let off = self.alloc(run.len());
        let o = off as usize;
        self.data[o..o + run.len()].copy_from_slice(run);
        self.refs[o] = 1;
        self.lens[o] = run.len() as u8;
        let data = &self.data;
        let lens = &self.lens;
        self.table.insert_unique(h, off, |&x| {
            let x = x as usize;
            hash_run(&data[x..x + lens[x] as usize])
        });
        self.dirty.push((off, run.len() as u32));
        self.live_elems += run.len();
        self.referenced_elems += run.len() as u64;
        (off, true)
    }

    pub fn add_ref(&mut self, off: u32) {
        let o = off as usize;
        debug_assert!(self.refs[o] > 0);
        self.refs[o] += 1;
        self.referenced_elems += self.lens[o] as u64;
    }
    pub fn release(&mut self, off: u32) -> bool {
        let o = off as usize;
        let len = self.lens[o] as usize;
        debug_assert!(self.refs[o] > 0, "release of unreferenced run");
        self.refs[o] -= 1;
        self.referenced_elems -= len as u64;
        if self.refs[o] != 0 {
            return false;
        }
        let h = hash_run(&self.data[o..o + len]);
        match self.table.find_entry(h, |&x| x == off) {
            Ok(e) => {
                e.remove();
            }
            Err(_) => debug_assert!(false, "run missing from intern table"),
        }
        self.free[len].push(off);
        self.live_elems -= len;
        true
    }

    fn alloc(&mut self, len: usize) -> u32 {
        if let Some(off) = self.free[len].pop() {
            return off;
        }
        let off = self.data.len();
        self.data.resize(off + len, T::zeroed());
        self.refs.resize(off + len, 0);
        self.lens.resize(off + len, 0);
        off as u32
    }

    pub fn take_dirty(&mut self) -> Vec<(u32, u32)> {
        std::mem::take(&mut self.dirty)
    }

    pub fn dedup_ratio(&self) -> f64 {
        if self.live_elems == 0 {
            1.0
        } else {
            self.referenced_elems as f64 / self.live_elems as f64
        }
    }
}
