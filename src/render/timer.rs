use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub const MAX_QUERIES: u32 = 22;
const TS_BYTES: u64 = MAX_QUERIES as u64 * 8;
const COUNTER_OFFSET: u64 = 256;
const COUNTER_BYTES: u64 = 64;
const TOTAL: u64 = 512;
const RING: usize = 3;

pub mod q {
    pub const TILE_SELECT: u32 = 0;
    pub const MARCH: u32 = 2;
    pub const HIZ: u32 = 4;
    pub const RECOVER: u32 = 6;
    pub const RESOLVE: u32 = 8;
    pub const MARCH2: u32 = 10;
    pub const TAA: u32 = 12;

    pub const SHAFT: u32 = 14;
    pub const SHADOW_RAY: u32 = 16;
    pub const SHADOW_H: u32 = 18;
    pub const SHADOW_V: u32 = 20;
}

struct Slot {
    buf: wgpu::Buffer,
    ready: Arc<AtomicBool>,
    pending: bool,
}

pub struct Readback {
    pub query_set: Option<wgpu::QuerySet>,
    staging: wgpu::Buffer,
    slots: Vec<Slot>,
    frame: usize,
    period: f32,
    timestamps: [u64; MAX_QUERIES as usize],

    pub counters: [u32; 16],
    pub valid: bool,
}

impl Readback {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, timestamps: bool) -> Self {
        let query_set = timestamps.then(|| {
            device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: MAX_QUERIES,
            })
        });
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("query staging"),
            size: TOTAL,
            usage: wgpu::BufferUsages::QUERY_RESOLVE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let slots = (0..RING)
            .map(|_| Slot {
                buf: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("readback"),
                    size: TOTAL,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                ready: Arc::new(AtomicBool::new(false)),
                pending: false,
            })
            .collect();
        Self {
            query_set,
            staging,
            slots,
            frame: 0,
            period: queue.get_timestamp_period(),
            timestamps: [0; MAX_QUERIES as usize],
            counters: [0; 16],
            valid: false,
        }
    }

    pub fn writes(&self, begin: u32) -> Option<wgpu::ComputePassTimestampWrites<'_>> {
        self.query_set
            .as_ref()
            .map(|qs| wgpu::ComputePassTimestampWrites {
                query_set: qs,
                beginning_of_pass_write_index: Some(begin),
                end_of_pass_write_index: Some(begin + 1),
            })
    }

    pub fn family_timestamp(&self, begin: bool) -> Option<wgpu::ComputePassTimestampWrites<'_>> {
        self.query_set.as_ref().map(|qs| wgpu::ComputePassTimestampWrites {
            query_set: qs,
            beginning_of_pass_write_index: if begin { Some(q::RESOLVE) } else { None },
            end_of_pass_write_index: if begin { None } else { Some(q::RESOLVE + 1) },
        })
    }

    pub fn encode(&mut self, encoder: &mut wgpu::CommandEncoder, counters: &wgpu::Buffer) {
        let slot = self.frame % RING;
        if self.slots[slot].pending && self.slots[slot].ready.load(Ordering::Acquire) {
            {
                let view = self.slots[slot]
                    .buf
                    .slice(..)
                    .get_mapped_range()
                    .expect("readback map");
                self.timestamps
                    .copy_from_slice(bytemuck::cast_slice(&view[..TS_BYTES as usize]));
                let c = COUNTER_OFFSET as usize;
                self.counters
                    .copy_from_slice(bytemuck::cast_slice(&view[c..c + COUNTER_BYTES as usize]));
                self.valid = true;
            }
            self.slots[slot].buf.unmap();
            self.slots[slot].pending = false;
            self.slots[slot].ready.store(false, Ordering::Release);
        }
        if self.slots[slot].pending {

            self.frame += 1;
            return;
        }
        if let Some(qs) = &self.query_set {
            encoder.resolve_query_set(qs, 0..MAX_QUERIES, &self.staging, 0);
        }
        encoder.copy_buffer_to_buffer(counters, 0, &self.staging, COUNTER_OFFSET, COUNTER_BYTES);
        encoder.copy_buffer_to_buffer(&self.staging, 0, &self.slots[slot].buf, 0, TOTAL);
        self.slots[slot].pending = true;
        self.frame += 1;
    }

    pub fn map_latest(&mut self) {
        let slot = (self.frame + RING - 1) % RING;
        let s = &self.slots[slot];
        if s.pending && !s.ready.load(Ordering::Acquire) {
            let flag = s.ready.clone();
            s.buf.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                if r.is_ok() {
                    flag.store(true, Ordering::Release);
                }
            });
        }
    }

    pub fn ms(&self, begin: u32) -> f32 {
        if !self.valid {
            return 0.0;
        }
        let a = self.timestamps[begin as usize];
        let b = self.timestamps[begin as usize + 1];
        if b <= a {
            return 0.0;
        }
        (b - a) as f32 * self.period / 1.0e6
    }

    pub fn total_ms(&self) -> f32 {
        [
            q::TILE_SELECT,
            q::MARCH,
            q::HIZ,
            q::RECOVER,
            q::MARCH2,
            q::RESOLVE,
            q::TAA,
            q::SHAFT,
        ]
        .iter()
        .map(|&b| self.ms(b))
        .sum()
    }
}
