use crate::block::{self, BlockId, AIR, WATER};
use crate::voxel::tree::{cell_bit, dense_index};
use crate::voxel::VOL;

pub const CELLS: usize = 4096;

pub const BRICK_WORDS: usize = 32;
pub const MAX_LEVEL: u8 = 15;

#[inline]
pub fn pack(sky: u8, blk: [u8; 3]) -> u16 {
    ((sky as u16) << 12)
        | ((blk[0] as u16 & 0xF) << 8)
        | ((blk[1] as u16 & 0xF) << 4)
        | (blk[2] as u16 & 0xF)
}

#[inline]
pub fn sky_of(b: u16) -> u8 {
    (b >> 12) as u8
}

#[inline]
pub fn block_of(b: u16) -> [u8; 3] {
    [
        ((b >> 8) & 0xF) as u8,
        ((b >> 4) & 0xF) as u8,
        (b & 0xF) as u8,
    ]
}

#[inline]
pub fn block_level(b: u16) -> u8 {
    let c = block_of(b);
    c[0].max(c[1]).max(c[2])
}

pub struct LightData {

    pub cells: Vec<u32>,
    pub bricks: Vec<[u32; BRICK_WORDS]>,

    pub uniform: Option<u16>,
}

const UNIFORM_BIT: u32 = 1 << 31;

pub fn compute(dense: &[BlockId], above_open: &[bool]) -> LightData {
    debug_assert_eq!(dense.len(), VOL);

    let any_emitter = dense.iter().any(|&b| block::def(b).light != [0, 0, 0]);
    let open_count = above_open.iter().filter(|&&o| o).count();

    if open_count == above_open.len() && !any_emitter && dense.iter().all(|&b| b == AIR) {
        return LightData {
            cells: Vec::new(),
            bricks: Vec::new(),
            uniform: Some(pack(MAX_LEVEL, [0, 0, 0])),
        };
    }
    if open_count == 0 && !any_emitter {
        return LightData {
            cells: Vec::new(),
            bricks: Vec::new(),
            uniform: Some(0),
        };
    }

    let mut sky = vec![0u8; VOL];

    let mut blk: [Vec<u8>; 3] = if any_emitter {
        [vec![0u8; VOL], vec![0u8; VOL], vec![0u8; VOL]]
    } else {
        [Vec::new(), Vec::new(), Vec::new()]
    };
    let mut queue: Vec<u32> = Vec::with_capacity(4096);

    for z in 0..64usize {
        for x in 0..64usize {
            if !above_open[x + z * 64] {
                continue;
            }
            let mut level = MAX_LEVEL;
            for y in (0..64usize).rev() {
                let i = dense_index(x, y, z);
                if block::casts_sun_shadow(dense[i]) {
                    break;
                }
                sky[i] = level;
                queue.push(i as u32);
                if dense[i] == WATER {
                    if level == 0 {
                        break;
                    }
                    level -= 1;
                }
            }
        }
    }
    flood(&mut sky, dense, &mut queue, true);

    if any_emitter {
        let emitters: Vec<u32> = dense
            .iter()
            .enumerate()
            .filter(|(_, &id)| block::def(id).light != [0, 0, 0])
            .map(|(i, _)| i as u32)
            .collect();
        for (c, level) in blk.iter_mut().enumerate() {
            queue.clear();
            for &i in &emitters {
                let e = block::def(dense[i as usize]).light[c];
                if e > 0 {
                    level[i as usize] = e;
                    queue.push(i);
                }
            }
            flood(level, dense, &mut queue, false);
        }
    }

    pack_bricks(&sky, &blk)
}

#[inline]
fn opaque(id: BlockId) -> bool {
    block::def(id).opaque
}

fn flood(level: &mut [u8], dense: &[BlockId], queue: &mut Vec<u32>, sunlight: bool) {
    let mut head = 0;
    while head < queue.len() {
        let i = queue[head] as usize;
        head += 1;
        let l = level[i];
        if l <= 1 {
            continue;
        }
        let x = i & 63;
        let z = (i >> 6) & 63;
        let y = i >> 12;
        let next = l - 1;
        let mut visit = |nx: usize, ny: usize, nz: usize, q: &mut Vec<u32>| {
            let ni = dense_index(nx, ny, nz);
            let blocked = if sunlight { block::casts_sun_shadow(dense[ni]) } else { opaque(dense[ni]) };
            if blocked || level[ni] >= next {
                return;
            }
            level[ni] = next;
            q.push(ni as u32);
        };
        if x > 0 {
            visit(x - 1, y, z, queue);
        }
        if x < 63 {
            visit(x + 1, y, z, queue);
        }
        if y > 0 {
            visit(x, y - 1, z, queue);
        }
        if y < 63 {
            visit(x, y + 1, z, queue);
        }
        if z > 0 {
            visit(x, y, z - 1, queue);
        }
        if z < 63 {
            visit(x, y, z + 1, queue);
        }

        if head > VOL * 2 {
            break;
        }
    }
}

fn pack_bricks(sky: &[u8], blk: &[Vec<u8>; 3]) -> LightData {
    let lit = !blk[0].is_empty();
    let at = |i: usize| -> [u8; 3] {
        if lit {
            [blk[0][i], blk[1][i], blk[2][i]]
        } else {
            [0, 0, 0]
        }
    };
    let mut cells = vec![0u32; CELLS];
    let mut bricks: Vec<[u32; BRICK_WORDS]> = Vec::new();
    let first = pack(sky[0], at(0));
    let mut chunk_uniform = true;
    for cz in 0..16usize {
        for cy in 0..16usize {
            for cx in 0..16usize {
                let mut brick = [0u32; BRICK_WORDS];
                let mut uniform = true;
                let mut val = 0u16;
                for vz in 0..4usize {
                    for vy in 0..4usize {
                        for vx in 0..4usize {
                            let i = dense_index(cx * 4 + vx, cy * 4 + vy, cz * 4 + vz);
                            let b = pack(sky[i], at(i));
                            let bit = cell_bit(vx as u32, vy as u32, vz as u32) as usize;
                            if bit == 0 {
                                val = b;
                            } else if b != val {
                                uniform = false;
                            }

                            brick[bit >> 1] |= (b as u32) << ((bit & 1) * 16);
                        }
                    }
                }
                let ci = cx | (cy << 4) | (cz << 8);
                if uniform {
                    cells[ci] = UNIFORM_BIT | val as u32;
                    if val != first {
                        chunk_uniform = false;
                    }
                } else {
                    cells[ci] = bricks.len() as u32;
                    bricks.push(brick);
                    chunk_uniform = false;
                }
            }
        }
    }
    if chunk_uniform {
        LightData {
            cells: Vec::new(),
            bricks: Vec::new(),
            uniform: Some(first),
        }
    } else {
        LightData {
            cells,
            bricks,
            uniform: None,
        }
    }
}

pub fn open_columns(heights: &[i32; 64 * 64], chunk_top: i32) -> Vec<bool> {
    heights.iter().map(|&h| h <= chunk_top).collect()
}
