//! Batch 70: the idle-repaint decision, as data rather than as a branch in the app.
//!
//! **What it is deciding.** From the first batch through batch 66 the window loop encoded
//! all seven passes for every presented frame. That is the right default for a renderer,
//! and it is a waste of every joule in the situation a game is in most of the time: the
//! player is standing still, the world is resident, and the only things that move are the
//! ones authored to move on their own -- the wave phase, the deck's wind, the sun. When
//! that is true, the frame a fresh render would produce is the frame already converged in
//! the history texture, and the honest way to show it at the display's rate is to present
//! that texture again rather than to compute it again.
//!
//! The rule lives in the library and not in `app.rs` for the same reason the vantage set
//! is data in `harness/vantage.rs`: `app.rs` is the one module no capture can reach, so
//! the decision is made here where `tests/idle.rs` can see it, and the app keeps only the
//! plumbing. The budget this was bought against, and every candidate that lost, is the
//! batch's row in `docs/ledger.md`.

/// The sun's tick: how far the day may advance before a still frame owes the sky and the
/// shadows a fresh render even with nothing else moving.
///
/// **Derived from the shadow run, not from the refresh rate.** The longest shadow run in
/// the frame is the batch-37 envelope's, which carries terrain shadows out past 600
/// blocks; at that range one tick moves a shadow edge by `600 * tan(0.024 deg)` = 0.25
/// blocks, sub-pixel at any camera that can see a run that long, and `the_tick_is_sub_
/// pixel_by_construction` in `tests/idle.rs` pins the bound to the constant. It is a tick
/// and not a freeze: with the sea and the deck frozen, an idle frame re-renders at every
/// tick boundary, which at the default 600-second day (0.6 deg/s) is every 40 ms. The
/// creep that replaces is what the tick was sized below.
pub const SUN_TICK_DEG: f32 = 0.024;

/// The cadence a live field caps an idle run at. The sea's phase and the deck's wind are
/// authored motion at up to 60 Hz and more: repainting more than one vblank in a row
/// turns that into motion at half the display rate or below, which reads on the water.
/// So with either speed non-zero, at most every second presented frame may be a repaint.
/// The cadence exits the frame any token changes -- camera motion included -- because a
/// full render is what a changed token is for.
pub const LIVE_ANIM_EVERY: u32 = 2;

/// Which [`SUN_TICK_DEG`] tick a time-of-day falls in. `time_of_day` is a fraction of a
/// day, so the wrap from the last tick back to tick 0 is a change like any other and
/// forces a frame at midnight rather than trusting a modulo to be continuous.
pub fn sun_tick(time_of_day: f32) -> u32 {
    (time_of_day.rem_euclid(1.0) * 360.0 / SUN_TICK_DEG) as u32
}

/// Everything two presented frames may differ by while still being obliged to show the
/// same world, reduced to one comparable token.
///
/// **A field belongs here only under the argument that a difference in it changes
/// pixels**, and the set is deliberately small enough to audit:
///
/// - `cam`, `yaw`, `pitch` -- the view. Motion-vector TAA, the shaft window's anchor and
///   every ray origin downstream of them. A stationary player yields these bit-stable:
///   zero input integrates to zero displacement, so no epsilon is wanted and none exists.
/// - `world` -- `World::version`, bumped by chunk interning, edits and relights exactly
///   when the render list can change, which is the one place a changed world announces
///   it. `probes_pending` covers the bake a chunk carries before `sync_world` drains it,
///   which changes the probe lattice without touching `version`. `relight_pending` is
///   the app's own queue between `set_block` and `stream::relight`, which would otherwise
///   be a one-frame hole between two version bumps.
/// - `flags`, `spec_hi`, `taa` -- the pipeline words and the history mode. F5/F6/F7 and
///   stepping into water all land here, so a toggle renders before it can repaint.
///
/// Not here, deliberately: the HUD (rebuilt every repaint, which is what keeps the
/// counters live), the shaft window (a function of `cam`, already covered), and
/// `frame.time`, which no field reads when the wave and cloud speeds are zero -- and when
/// either is not, `animated` caps the cadence instead of adding a field.
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

/// What one presented frame owes the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameKind {
    /// Encode all seven passes. The presented image is new work.
    Render,
    /// Present the last converged frame again. **Bit-identical to re-presenting the frame
    /// it came from**, because no pass runs between the two presents and the blit reads
    /// the same texture slot the last render presented -- the frame the display shows is
    /// the frame that was rendered; only the HUD changed, and the HUD is rebuilt first.
    Repaint,
}

/// The idle-repaint driver. Owns the last rendered token and the cadence counter, and
/// nothing else: the app feeds it one [`StaticFrame`] per presented frame.
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

    /// Forget the last rendered token, forcing the next `classify` to render. Called on
    /// resize and on a lost surface: the history textures were recreated, so the frame a
    /// repaint would re-present is an empty one.
    pub fn invalidate(&mut self) {
        self.last = None;
        self.repaints = 0;
    }

    /// What this vblank owes, and the whole of the rule.
    ///
    /// A token the previous render did not see -- including "there was no previous
    /// render", which is what the first frame and every invalidation produce -- forces a
    /// render, and is recorded against it. So does the sun crossing a tick. With the
    /// authored fields live, at most [`LIVE_ANIM_EVERY`] - 1 repaints may follow a
    /// render; with them frozen, a still world repaints until a token or the sun says
    /// otherwise, which is the regime a `--wave-speed 0 --cloud-speed 0 --freeze-time`
    /// run lives in.
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
            // The cadence render keeps its token: the world it redraws is the one already
            // recorded, so nothing is stored and the next cadence starts from zero.
            self.repaints = 0;
            return FrameKind::Render;
        }
        self.repaints += 1;
        FrameKind::Repaint
    }
}



