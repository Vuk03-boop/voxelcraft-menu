pub const SUN_TICK_DEG: f32 = 0.024;

pub const LIVE_ANIM_EVERY: u32 = 2;

pub fn sun_tick(time_of_day: f32) -> u32 {
    (time_of_day.rem_euclid(1.0) * 360.0 / SUN_TICK_DEG) as u32
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaticFrame {
    pub cam: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub world: u64,
    pub probes_pending: bool,
    pub relight_pending: bool,
    pub flags: u32,
    pub spec_hi: u32,
    pub taa: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameKind {

    Render,

    Repaint,
}

#[derive(Default)]
pub struct IdleRepaint {
    last: Option<(StaticFrame, u32)>,
    repaints: u32,
}

impl IdleRepaint {
    pub fn new() -> Self {
        Self {
            last: None,
            repaints: 0,
        }
    }

    pub fn invalidate(&mut self) {
        self.last = None;
        self.repaints = 0;
    }

    pub fn classify(&mut self, frame: StaticFrame, animated: bool, sun: u32) -> FrameKind {
        let stale = match &self.last {
            None => true,
            Some((f, t)) => *f != frame || *t != sun,
        };
        if stale {
            self.last = Some((frame, sun));
            self.repaints = 0;
            return FrameKind::Render;
        }
        if animated && self.repaints + 1 >= LIVE_ANIM_EVERY {

            self.repaints = 0;
            return FrameKind::Render;
        }
        self.repaints += 1;
        FrameKind::Repaint
    }
}
