//! GPU buffers that mirror CPU-side pools, with dirty-range uploads and growth.

/// A storage buffer that grows and can be updated by dirty element ranges.
pub struct GpuMirror {
    pub buf: wgpu::Buffer,
    cap: u64,
    usage: wgpu::BufferUsages,
    label: &'static str,
    /// Bumped whenever the buffer is recreated, so bind groups can be rebuilt.
    pub generation: u32,
    pub uploaded_bytes: u64,
}

const MIN_CAP: u64 = 1 << 20;

fn make_buffer(
    device: &wgpu::Device,
    label: &'static str,
    size: u64,
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage,
        mapped_at_creation: false,
    })
}

impl GpuMirror {
    pub fn new(device: &wgpu::Device, label: &'static str, usage: wgpu::BufferUsages) -> Self {
        let usage = usage | wgpu::BufferUsages::COPY_DST;
        Self {
            buf: make_buffer(device, label, MIN_CAP, usage),
            cap: MIN_CAP,
            usage,
            label,
            generation: 0,
            uploaded_bytes: 0,
        }
    }

    pub fn capacity(&self) -> u64 {
        self.cap
    }

    /// Bring the GPU copy up to date. `dirty` holds (offset, len) in elements of
    /// `elem` bytes. Returns true if the buffer was recreated.
    pub fn sync(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: &[u8],
        dirty: &mut Vec<(u32, u32)>,
        elem: usize,
    ) -> bool {
        let needed = data.len() as u64;
        if needed > self.cap {
            let mut cap = self.cap;
            while cap < needed {
                cap *= 2;
            }
            self.buf = make_buffer(device, self.label, cap, self.usage);
            self.cap = cap;
            self.generation += 1;
            queue.write_buffer(&self.buf, 0, data);
            self.uploaded_bytes += needed;
            dirty.clear();
            return true;
        }
        if dirty.is_empty() {
            return false;
        }
        dirty.sort_unstable();
        let mut merged: Vec<(u64, u64)> = Vec::with_capacity(dirty.len());
        for &(off, len) in dirty.iter() {
            let s = off as u64 * elem as u64;
            let e = s + len as u64 * elem as u64;
            match merged.last_mut() {
                // Merge ranges that are adjacent or nearly so; one bigger write beats two.
                Some(last) if s <= last.1 + 4096 => last.1 = last.1.max(e),
                _ => merged.push((s, e)),
            }
        }
        dirty.clear();
        if merged.len() > 512 {
            queue.write_buffer(&self.buf, 0, data);
            self.uploaded_bytes += needed;
            return false;
        }
        for (s, e) in merged {
            let e = e.min(needed);
            if e > s {
                // write_buffer requires a 4-byte aligned offset and size.
                let s = s & !3;
                let e = (e + 3) & !3;
                let e = e.min(needed);
                queue.write_buffer(&self.buf, s, &data[s as usize..e as usize]);
                self.uploaded_bytes += e - s;
            }
        }
        false
    }
}

/// A storage buffer rewritten wholesale each frame (the per-frame chunk list).
pub struct DynBuffer {
    pub buf: wgpu::Buffer,
    cap: u64,
    usage: wgpu::BufferUsages,
    label: &'static str,
    pub generation: u32,
}

impl DynBuffer {
    pub fn new(
        device: &wgpu::Device,
        label: &'static str,
        size: u64,
        usage: wgpu::BufferUsages,
    ) -> Self {
        let usage = usage | wgpu::BufferUsages::COPY_DST;
        let cap = size.max(256);
        Self {
            buf: make_buffer(device, label, cap, usage),
            cap,
            usage,
            label,
            generation: 0,
        }
    }

    pub fn write(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[u8]) -> bool {
        let needed = (data.len() as u64).max(256);
        let mut grew = false;
        if needed > self.cap {
            let mut cap = self.cap;
            while cap < needed {
                cap *= 2;
            }
            self.buf = make_buffer(device, self.label, cap, self.usage);
            self.cap = cap;
            self.generation += 1;
            grew = true;
        }
        if !data.is_empty() {
            queue.write_buffer(&self.buf, 0, data);
        }
        grew
    }

    /// Resize without preserving contents. Returns true if recreated.
    pub fn resize(&mut self, device: &wgpu::Device, size: u64) -> bool {
        let size = size.max(256);
        if size == self.cap {
            return false;
        }
        self.buf = make_buffer(device, self.label, size, self.usage);
        self.cap = size;
        self.generation += 1;
        true
    }

    pub fn capacity(&self) -> u64 {
        self.cap
    }
}



