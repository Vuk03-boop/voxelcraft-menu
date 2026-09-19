use std::collections::VecDeque;
use std::time::Instant;

pub struct Stats {
    frames: VecDeque<f32>,
    cpu_frames: VecDeque<f32>,
    last: Instant,
    pub frame_index: u64,

    pub chunks_drawn: usize,
    pub chunks_loaded: usize,
    pub chunks_in_flight: usize,
    pub lod_histogram: Vec<usize>,
    pub geometry_bytes: usize,
    pub attribute_bytes: usize,
    pub solid_voxels: u64,
    pub dedup_ratio: f64,
    pub pairs_marched: u32,
    pub pairs_deferred: u32,
    pub vram_bytes: u64,
}

impl Default for Stats {
    fn default() -> Self {
        Self::new()
    }
}

impl Stats {
    pub fn new() -> Self {
        Self {
            frames: VecDeque::with_capacity(120),
            cpu_frames: VecDeque::with_capacity(120),
            last: Instant::now(),
            frame_index: 0,
            chunks_drawn: 0,
            chunks_loaded: 0,
            chunks_in_flight: 0,
            lod_histogram: Vec::new(),
            geometry_bytes: 0,
            attribute_bytes: 0,
            solid_voxels: 0,
            dedup_ratio: 1.0,
            pairs_marched: 0,
            pairs_deferred: 0,
            vram_bytes: 0,
        }
    }

    pub fn reset_clock(&mut self) { self.last = Instant::now(); }

    pub fn tick(&mut self) -> f32 {
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32();
        self.last = now;
        self.frame_index += 1;
        push(&mut self.frames, dt * 1000.0);
        dt.clamp(1.0 / 1000.0, 0.1)
    }

    pub fn record_cpu(&mut self, ms: f32) {
        push(&mut self.cpu_frames, ms);
    }

    pub fn fps(&self) -> f32 {
        let m = mean(&self.frames);
        if m > 0.0 {
            1000.0 / m
        } else {
            0.0
        }
    }

    pub fn frame_ms(&self) -> f32 {
        mean(&self.frames)
    }
    pub fn cpu_ms(&self) -> f32 {
        mean(&self.cpu_frames)
    }

    pub fn cpu_ms_max(&self) -> f32 {
        self.cpu_frames.iter().copied().fold(0.0, f32::max)
    }

    pub fn bytes_per_voxel(&self) -> f64 {
        if self.solid_voxels == 0 {
            0.0
        } else {
            self.geometry_bytes as f64 / self.solid_voxels as f64
        }
    }

    pub fn attr_bytes_per_voxel(&self) -> f64 {
        if self.solid_voxels == 0 {
            0.0
        } else {
            self.attribute_bytes as f64 / self.solid_voxels as f64
        }
    }
}

fn push(q: &mut VecDeque<f32>, v: f32) {
    if q.len() >= 120 {
        q.pop_front();
    }
    q.push_back(v);
}

fn mean(q: &VecDeque<f32>) -> f32 {
    if q.is_empty() {
        0.0
    } else {
        q.iter().sum::<f32>() / q.len() as f32
    }
}

pub fn fmt_bytes(b: u64) -> String {
    const U: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < 3 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{v:.0}{}", U[i])
    } else {
        format!("{v:.1}{}", U[i])
    }
}
