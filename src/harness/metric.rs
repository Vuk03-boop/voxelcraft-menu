//! Image metrics, and the hash the bit-exactness sweep prints.
//!
//! Every metric here takes a [`Rect`], because a metric over a whole frame is a metric over
//! whatever happened to be in the frame. Batch 21 built three look-metrics in one session and
//! all three were confounded -- one of them by a shoreline edge sitting inside an ad-hoc crop,
//! which pinned an FFT band peak to a near-constant value across builds differing by MAE 18.8.
//! A crop that is a named constant cannot drift between two numbers in the same table.
//!
//! The definitions are spelled out rather than left to the caller's idea of them, because the
//! previous arrangement -- a metric written fresh in whatever scratch script the session
//! happened to need -- is the reason two "speckle" numbers in two batch docs are not
//! necessarily the same quantity.

use std::path::Path;

/// An 8-bit RGBA frame exactly as `--screenshot` writes it.
///
/// The values are **sRGB-encoded**: `resolve` works in linear and `blit.wgsl` is the one
/// place that tone-maps and encodes. Anything that averages frames has to decode first --
/// see [`srgb_to_linear`].
#[derive(Clone)]
pub struct Img {
    pub w: u32,
    pub h: u32,
    /// RGBA, four bytes per pixel, row major.
    pub px: Vec<u8>,
}

impl Img {
    pub fn new(w: u32, h: u32) -> Img {
        Img {
            w,
            h,
            px: vec![0; (w as usize) * (h as usize) * 4],
        }
    }

    pub fn load(path: &Path) -> Result<Img, String> {
        let img = image::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let rgba = img.to_rgba8();
        Ok(Img {
            w: rgba.width(),
            h: rgba.height(),
            px: rgba.into_raw(),
        })
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        image::save_buffer(
            path,
            &self.px,
            self.w,
            self.h,
            image::ColorType::Rgba8,
        )
        .map_err(|e| format!("{}: {e}", path.display()))
    }

    #[inline]
    pub fn get(&self, x: u32, y: u32, c: usize) -> u8 {
        let x = x.min(self.w - 1) as usize;
        let y = y.min(self.h - 1) as usize;
        self.px[(y * self.w as usize + x) * 4 + c]
    }

    pub fn same_size(&self, other: &Img) -> bool {
        self.w == other.w && self.h == other.h
    }
}

/// A pixel rectangle, `x1`/`y1` exclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}

impl Rect {
    pub fn full(w: u32, h: u32) -> Rect {
        Rect {
            x0: 0,
            y0: 0,
            x1: w,
            y1: h,
        }
    }

    pub fn pixels(&self) -> u64 {
        (self.x1.saturating_sub(self.x0) as u64) * (self.y1.saturating_sub(self.y0) as u64)
    }

    pub fn is_empty(&self) -> bool {
        self.x1 <= self.x0 || self.y1 <= self.y0
    }

    fn clamped(&self, img: &Img) -> Rect {
        Rect {
            x0: self.x0.min(img.w),
            y0: self.y0.min(img.h),
            x1: self.x1.min(img.w),
            y1: self.y1.min(img.h),
        }
    }
}

impl std::fmt::Display for Rect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}x{}+{}+{}",
            self.x1 - self.x0,
            self.y1 - self.y0,
            self.x0,
            self.y0
        )
    }
}

// ------------------------------------------------------------------ difference metrics

/// Mean absolute difference over R, G and B, in code values (0..255).
///
/// Alpha is ignored: `--screenshot` writes it as 255 everywhere, and the atlas's alpha is a
/// tint mask that never reaches the frame.
pub fn mae(a: &Img, b: &Img, r: Rect) -> f64 {
    let r = r.clamped(a);
    let mut sum = 0.0f64;
    for y in r.y0..r.y1 {
        for x in r.x0..r.x1 {
            for c in 0..3 {
                sum += (a.get(x, y, c) as f64 - b.get(x, y, c) as f64).abs();
            }
        }
    }
    let n = r.pixels().max(1) as f64 * 3.0;
    sum / n
}

/// Largest single-channel difference anywhere in the rect, in code values.
pub fn max_delta(a: &Img, b: &Img, r: Rect) -> u32 {
    let r = r.clamped(a);
    let mut m = 0u32;
    for y in r.y0..r.y1 {
        for x in r.x0..r.x1 {
            for c in 0..3 {
                let d = (a.get(x, y, c) as i32 - b.get(x, y, c) as i32).unsigned_abs();
                m = m.max(d);
            }
        }
    }
    m
}

/// The bounding box of every pixel that differs, or `None` when nothing does.
///
/// **A pop is a place before it is a number.** D2b's two surviving spikes are 8x their
/// step *neighbours* while sitting far under the by-eye line absolutely, so neither a
/// whole-frame MAE nor a still rated them correctly; what pins one is where it lives, and
/// batch 64's lesson is that the crop has to reach the defect before MAE can rank it. This
/// is the cheap half of that: one scan, and the `--diff` mode reports the region the next
/// run should be quoting.
pub fn diff_bbox(a: &Img, b: &Img) -> Option<Rect> {
    if a.w != b.w || a.h != b.h {
        return None;
    }
    let mut lo = [u32::MAX; 2];
    let mut hi = [0u32; 2];
    for y in 0..a.h {
        for x in 0..a.w {
            if (0..3).any(|c| a.get(x, y, c) != b.get(x, y, c)) {
                lo[0] = lo[0].min(x);
                lo[1] = lo[1].min(y);
                hi[0] = hi[0].max(x + 1);
                hi[1] = hi[1].max(y + 1);
            }
        }
    }
    (hi[0] > 0).then(|| Rect {
        x0: lo[0],
        y0: lo[1],
        x1: hi[0],
        y1: hi[1],
    })
}

/// Pixels differing in any of R, G, B. This is the number every "0 pixels differing" claim
/// in `PERF.md` is quoting.
pub fn pixels_differing(a: &Img, b: &Img, r: Rect) -> u64 {
    let r = r.clamped(a);
    let mut n = 0u64;
    for y in r.y0..r.y1 {
        for x in r.x0..r.x1 {
            if (0..3).any(|c| a.get(x, y, c) != b.get(x, y, c)) {
                n += 1;
            }
        }
    }
    n
}

// ------------------------------------------------------------------ single-image metrics

/// High-frequency residual: the mean of `|c - mean3x3(c)|` over R, G and B, in code values.
///
/// This is the "speckle" of batches 19 and 21. Two things about it are worth stating because
/// neither was written down when the number was first quoted. The 3x3 mean **includes the
/// centre pixel**, and neighbours are read from the whole image with coordinates clamped to
/// its bounds, so a crop touching the frame edge is not penalised for it.
///
/// It is blind to coherence by construction -- it charges a lattice and a field of unrelated
/// noise the same amount -- which is why batch 21 could not settle a question about a
/// *pattern* with it. Use it for "how much detail is here", never for "is this detail
/// regular".
pub fn speckle(a: &Img, r: Rect) -> f64 {
    let r = r.clamped(a);
    let mut sum = 0.0f64;
    for y in r.y0..r.y1 {
        for x in r.x0..r.x1 {
            for c in 0..3 {
                let mut m = 0.0f64;
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let sx = (x as i32 + dx).clamp(0, a.w as i32 - 1) as u32;
                        let sy = (y as i32 + dy).clamp(0, a.h as i32 - 1) as u32;
                        m += a.get(sx, sy, c) as f64;
                    }
                }
                sum += (a.get(x, y, c) as f64 - m / 9.0).abs();
            }
        }
    }
    sum / (r.pixels().max(1) as f64 * 3.0)
}

/// Standard deviation of `R - G` over the rect, in code values.
///
/// Batch 21b's absorption metric. Water's per-channel extinction takes red out fastest, so a
/// variation in **path length** shows up as a variation in `R - G` while a variation in
/// *material* mostly does not: SAND against GRASS moves green by about 70 and moves `R - G`
/// much less than it moves either channel.
pub fn rg_std(a: &Img, r: Rect) -> f64 {
    let r = r.clamped(a);
    let n = r.pixels().max(1) as f64;
    let mut sum = 0.0f64;
    let mut sum2 = 0.0f64;
    for y in r.y0..r.y1 {
        for x in r.x0..r.x1 {
            let d = a.get(x, y, 0) as f64 - a.get(x, y, 1) as f64;
            sum += d;
            sum2 += d * d;
        }
    }
    (sum2 / n - (sum / n) * (sum / n)).max(0.0).sqrt()
}

/// High-frequency residual of `R - G`: the mean of `|(r - g) - mean3x3(r - g)|` over the rect,
/// in code values. The 3x3 convention is [`speckle`]'s exactly -- the centre pixel is included
/// and neighbours are read from the whole image with coordinates clamped to its bounds.
///
/// Batch 21b's acceptance metric, and it exists because [`rg_std`] cannot be one. `rg_std`
/// charges the whole spread of `R - G`, and on water that is two quantities at once: the
/// low-frequency depth cue, which is what the absorption term is *for*, and the per-voxel
/// path-length lattice, which is the defect. A change that deleted both would read as a
/// triumph on `rg_std`. This one sees only what varies between neighbouring pixels, so the
/// pair separates them -- a fix has to drop this while leaving `rg_std` standing.
///
/// Like `speckle` it is blind to coherence, and here that costs nothing: the question is how
/// much per-pixel `R - G` structure a *terraced* surface carries, and `terraces/dry` is the
/// same geometry with no water in front of it and so is the level to read this against.
pub fn rg_speckle(a: &Img, r: Rect) -> f64 {
    let r = r.clamped(a);
    let mut sum = 0.0f64;
    for y in r.y0..r.y1 {
        for x in r.x0..r.x1 {
            let mut m = 0.0f64;
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let sx = (x as i32 + dx).clamp(0, a.w as i32 - 1) as u32;
                    let sy = (y as i32 + dy).clamp(0, a.h as i32 - 1) as u32;
                    m += a.get(sx, sy, 0) as f64 - a.get(sx, sy, 1) as f64;
                }
            }
            let d = a.get(x, y, 0) as f64 - a.get(x, y, 1) as f64;
            sum += (d - m / 9.0).abs();
        }
    }
    sum / r.pixels().max(1) as f64
}

/// Mean R, G and B over the rect, in code values.
// `c` is a channel number: it indexes the accumulator and is also an argument to `get`.
#[allow(clippy::needless_range_loop)]
pub fn mean_rgb(a: &Img, r: Rect) -> [f64; 3] {
    let r = r.clamped(a);
    let n = r.pixels().max(1) as f64;
    let mut s = [0.0f64; 3];
    for y in r.y0..r.y1 {
        for x in r.x0..r.x1 {
            for c in 0..3 {
                s[c] += a.get(x, y, c) as f64;
            }
        }
    }
    [s[0] / n, s[1] / n, s[2] / n]
}

// ------------------------------------------------------------------ coherence

/// The side of the box whose mean is subtracted before anything is correlated, in pixels.
///
/// **Three, which is [`speckle`]'s own 3x3 and not a coincidence** -- the two metrics
/// high-pass the frame identically and differ only in what they do next, which is what makes
/// them a pair rather than two opinions. `speckle` averages the residual's magnitude and
/// answers *how much* detail is here; this one correlates the residual against itself and
/// answers *how regular* it is.
///
/// It was swept, and the sweep is the argument. Against `--no-tex-variation` at
/// `default/slope` the response falls monotonically as the box widens -- **1.20x at 3, 1.18x
/// at 5, 1.12x at 9, 1.08x at 15** -- because a wider box keeps more of the frame's own shape,
/// and the slope's shape is the same in both builds. At 129 the metric is blind: every lag in
/// the annulus reads 0.4 to 0.7 on both sides and the two differ in the fourth decimal. The
/// defect lives at the texel scale, so the band has to.
///
/// **Re-swept against the carved-canopy frame in batch 39 and it did not move**: 1.0755x at
/// 3, 1.0715 at 5, 1.0643 at 7, 1.0584 at 9, 1.0468 at 13 -- the same monotone fall from the
/// same winner, on a response a third the size. The constant was never the thing that changed.
pub const COH_HIGHPASS: u32 = 3;

/// The side of the window a peak is taken in, in pixels at [`vantage::STD_HEIGHT`], stepped
/// by half of itself across the crop.
///
/// **A whole-crop correlation cannot see this defect and the reason is perspective, not
/// tuning.** A block projects to about 20 pixels near the bottom of the `default` frame and to
/// about 3 at the crest, so the lattice the eye reads as one repeating surface has no single
/// period in the image -- correlate the whole crop and every depth's peak lands at a different
/// lag, each carrying a few per cent of the area, and the sum of them is a smooth decay with
/// no peak in it at all. Inside 64 pixels the depth range is small enough that there is one
/// period, and the peak comes back.
///
/// Swept the same way: **1.18x at 48, 1.20x at 64, 1.19x at 96, 1.08x at 128**. It has an
/// optimum rather than a direction, which is what says the parameter is the right one.
///
/// **Re-swept in batch 39 and the optimum is still 64**: 1.0218x at 32, 1.0678 at 48,
/// **1.0755 at 64**, 1.0671 at 96, 1.0210 at 128, and 0.9696 at 192 -- where the tile is wide
/// enough to hold several depths again and the reading inverts, which is the whole-crop
/// failure this constant exists to avoid, arriving gradually.
pub const COH_TILE: u32 = 64;

/// The inner edge of the lag annulus, in pixels at [`vantage::STD_HEIGHT`].
///
/// Below it every image scores near 1 trivially -- that is the central lobe of whatever
/// detail survived the high-pass, which is the picture being *smooth* rather than the picture
/// *repeating*. The same shape of constant, and the same argument, as `LAG_MIN` in
/// `tests/waves.rs`: the other place in this project that takes a peak over an annulus rather
/// than over everything.
///
/// At [`COH_HIGHPASS`] of 3 the lobe is two pixels wide, so this barely binds -- 4, 6 and 8
/// score 1.202, 1.205 and 1.208. It is 6 for the margin and not for the third decimal.
pub const COH_LAG_MIN: u32 = 6;

/// The outer edge. **It may not exceed half of [`COH_TILE`], and it is exactly half.**
///
/// A lag of `L` inside a tile of side `T` leaves `(T - L)^2` pixel pairs to correlate, so the
/// estimate gets noisier as the lag grows -- and a *maximum* taken over some fifteen hundred
/// noisy estimates is biased upward by however noisy the worst of them is. Half the tile is
/// where that bias is still a quarter of the tile's pixels rather than a handful of them.
///
/// It also has to reach the period that carries the defect, and at `default/slope` that is 21
/// to 24 pixels: an annulus stopping at 20 scores **1.07x** where one reaching 24 scores
/// **1.21x**. 32 rather than 24 so that a vantage with nearer ground than this one still has
/// its period inside the annulus.
///
/// **Re-swept in batch 39**: 0.9943x at 12, 1.0215 at 16, 1.0462 at 20, **1.0853 at 24**,
/// 1.0809 at 28, 1.0755 at 32. 24 now edges 32 by 0.01 -- a third decimal on a 7.55% response,
/// against a forward-looking reason for 32 that still holds, so it stays. Recorded because a
/// sweep whose winner moved inside the noise is a sweep that has to say so.
pub const COH_LAG_MAX: u32 = 32;

/// How strongly a crop repeats, and at what lag -- the answer to "is this detail *regular*",
/// which is the one question [`speckle`] is built not to answer.
#[derive(Clone, Copy, Debug)]
pub struct Coherence {
    /// Mean over the crop's tiles of each tile's peak correlation. 1 is a picture that
    /// reproduces itself exactly one lag away; 0 is a picture whose detail at that distance
    /// is unrelated to itself.
    pub peak: f64,
    /// The lag of the *strongest* tile, in pixels of this image. Two builds agreeing on
    /// `peak` and disagreeing on `lag` are not measuring the same structure, and a batch
    /// chasing a repeat needs to know which period it caught.
    pub lag: (i32, i32),
    /// How many tiles were averaged. A crop too small to hold one is 0, and a 0 here is the
    /// reading that says so rather than a number.
    pub tiles: u32,
}

/// Mean over [`COH_TILE`]-sided tiles of the peak normalised autocorrelation inside the lag
/// annulus \[[`COH_LAG_MIN`], [`COH_LAG_MAX`]\], after [`COH_HIGHPASS`]'s local mean is
/// subtracted.
///
/// **This is the metric [`speckle`] is blind to, and the two are a pair in the same way
/// [`rg_std`] and [`rg_speckle`] are.** `speckle` charges a lattice and a field of unrelated
/// noise the same amount; batch 36 is the demonstration, moving 32% of the frame's pixels
/// while `speckle` went 4.0573 to 4.0533. This one is the other way round: it is near-blind to
/// how *much* detail a crop carries and reads only how much of that detail is a copy of
/// itself.
///
/// **Four things are pinned here that an FFT taken in a scratch script leaves loose**, which is
/// the whole reason this is a fixture function and not a session's own code. Three documents
/// once disagreed about the wave field's autocorrelation because each ran an FFT over a window
/// nobody wrote down, and batch 21's band peak read near-constant across builds differing by
/// MAE 18.8 because the crop it ran over held a shoreline edge.
///
/// - **The window is the tile**, and there is no padding: only pixel pairs with *both* ends
///   inside it contribute. A wrapped or zero-padded correlation would score the tile's border.
/// - **The high-pass reads the whole image**, with the box clipped to the image bounds and the
///   mean taken over what is left -- [`speckle`]'s convention, so a crop at the frame edge is
///   not penalised for being there.
/// - **The normalisation is Pearson over the overlap**, recomputed per lag from that lag's own
///   two windows. The overlap shrinks as the lag grows, and dividing by a fixed zero-lag
///   energy would quietly make long lags score low for being long.
/// - **Every constant scales with the image height**, so a tile and an annulus mean the same
///   world distance at any capture size, the way a [`Rect`] from a `Crop` means the same
///   region.
///
/// **What it is worth, measured rather than claimed, and it is worth less than it was.**
/// Against `--no-tex-variation` at `default/slope` it read **0.1510 to 0.1819** when batch 36b
/// fitted it -- a 20.53% response -- and reads **0.1420 to 0.1527** now, 7.55%. **The
/// instrument did not change and neither did its four constants; the frame did.** Batch 38
/// carved the tree canopy, and [`coherence_tiles`] says where the loss went: 35 of the 190
/// tiles carried 79% of the old response, those 35 are the canopy tiles, and every
/// canopy-free tile is unchanged to four decimals. The in-phase lattice this control was
/// mostly moving was the `LEAVES` layer on solid leaf cubes rather than `GRASS_SIDE` on the
/// slope the crop is named for -- which nothing written at the time suspected, this doc
/// included.
///
/// At every other crop in the set it moves by under 2%, at `lod` by nothing at all because a
/// frame 240 blocks up has no block wide enough to carry a period, and at `shore`, `terraces`
/// and `underwater` it now moves the *wrong way*. **A coherence claim at a vantage this metric
/// has not been shown to respond at is not a measurement**, which is the same rule the
/// `water`/`land` crop tags already encode -- and after batch 38 that is very nearly every
/// vantage.
///
/// Two things it charges that are not texture, both real and neither a defect to fix:
/// **geometry**, because a staircase of voxel steps repeats at the block period exactly as an
/// in-phase texture does -- batch 36's own picture says a large part of the `default` slope's
/// banding is the staircase, and this scores the total; and **quantisation**, which is why the
/// `coastline` sky crop reads 0.72, the highest number in the set, off nothing but the sRGB
/// contours of a smooth gradient. Ranking the halves apart needs two controls, not a second
/// metric.
pub fn coherence(a: &Img, r: Rect) -> Coherence {
    coherence_with(a, r, COH_PARAMS)
}

/// The four constants of [`coherence`], together, so that a sweep of them is a call rather
/// than an edit.
///
/// **Every one was fitted against a frame, and a frame is not a fixed thing.** Batch 36b swept
/// all four at `default/slope` and wrote the winners into the constants below; batch 38 then
/// carved the canopy that turned out to be carrying four fifths of what the sweep was
/// measuring. A fixture that can only be re-fitted by editing it is a fixture nobody re-fits.
#[derive(Clone, Copy, Debug)]
pub struct CohParams {
    pub highpass: u32,
    pub tile: u32,
    pub lag_min: u32,
    pub lag_max: u32,
}

/// The shipping constants, in the one place a sweep can pass something else.
pub const COH_PARAMS: CohParams = CohParams {
    highpass: COH_HIGHPASS,
    tile: COH_TILE,
    lag_min: COH_LAG_MIN,
    lag_max: COH_LAG_MAX,
};

/// [`coherence`] at parameters of the caller's choosing. See [`CohParams`].
pub fn coherence_with(a: &Img, r: Rect, p: CohParams) -> Coherence {
    let tiles = coherence_tiles_with(a, r, p);
    let n = tiles.len() as u32;
    let best = tiles
        .iter()
        .copied()
        .fold((0.0f64, (0, 0)), |b, t| if t.0 > b.0 { t } else { b });
    Coherence {
        peak: summarise_tiles(&tiles),
        lag: best.1,
        tiles: n,
    }
}

/// Every tile's own peak and the lag it was found at, in the order the sweep visits them.
///
/// **Exposed because the summary over tiles is a choice and not an implementation detail**,
/// and because the crop-wide [`coherence_grid`] below cannot stand in for it: that one
/// correlates the whole crop, which is the reading the tile exists to replace. Scoring a
/// candidate summary means scoring it against this list, on images already on disk.
pub fn coherence_tiles(a: &Img, r: Rect) -> Vec<(f64, (i32, i32))> {
    coherence_tiles_with(a, r, COH_PARAMS)
}

/// [`coherence_tiles`] at parameters of the caller's choosing.
pub fn coherence_tiles_with(a: &Img, r: Rect, p: CohParams) -> Vec<(f64, (i32, i32))> {
    let s = scale(a);
    let lo = lag_px(p.lag_min, s);
    let hi = lag_px(p.lag_max, s);
    let side = lag_px(p.tile, s);
    let field = Field::new(a, odd_win(p.highpass, s));
    let r = r.clamped(a);
    let mut out: Vec<(f64, (i32, i32))> = Vec::new();
    let mut y = r.y0 as i32;
    while y + side <= r.y1 as i32 {
        let mut x = r.x0 as i32;
        while x + side <= r.x1 as i32 {
            let tile = Rect {
                x0: x as u32,
                y0: y as u32,
                x1: (x + side) as u32,
                y1: (y + side) as u32,
            };
            let mut peak = 0.0f64;
            let mut at = (0, 0);
            // The half-plane only: the pair set at `-L` is the pair set at `L` with its two
            // ends swapped, so `rho(-L) == rho(L)` and scoring both is scoring each twice.
            for dy in 0..=hi {
                for dx in -hi..=hi {
                    if dy == 0 && dx <= 0 {
                        continue;
                    }
                    let d2 = dx * dx + dy * dy;
                    if d2 < lo * lo || d2 > hi * hi {
                        continue;
                    }
                    let rho = field.rho(tile, dx, dy);
                    if rho > peak {
                        peak = rho;
                        at = (dx, dy);
                    }
                }
            }
            out.push((peak, at));
            x += side / 2;
        }
        y += side / 2;
    }
    out
}

/// Reduce the per-tile peaks of [`coherence_tiles`] to the one number the metric reports.
///
/// **The mean, and batch 39 measured that no other statistic is better.** When the
/// `--no-tex-variation` row fell under its floor the first hypothesis was dilution -- that a
/// minority of tiles holding no lattice were dragging an otherwise healthy mean down -- which
/// would have been answered by a median or a trimmed mean. Scored on the same four images, the
/// carved frame reads 1.0755x under the mean, 1.0836 under the median, 1.0714 over the top
/// three quarters, 1.0683 over the top quarter and 1.0717 over the top tenth. **They agree to
/// the second decimal, which is what says the tiles did not stop contributing -- the thing
/// they were contributing stopped existing.** A summary cannot recover a signal that is not
/// in the image.
fn summarise_tiles(tiles: &[(f64, (i32, i32))]) -> f64 {
    if tiles.is_empty() {
        return 0.0;
    }
    tiles.iter().map(|t| t.0).sum::<f64>() / tiles.len() as f64
}

/// Every lag in the half-plane `|dx| <= max`, `0 <= dy <= max`, row major from `dy = 0`, with
/// the annulus *not* applied -- which is how the annulus gets placed.
///
/// `compare --profile` prints this. It stands in the same relation to [`COH_LAG_MIN`] and
/// [`COH_LAG_MAX`] as `--rect` does to a [`Crop`](crate::harness::Crop): the way a candidate
/// gets scored before it is written into the table, so that neither is a number somebody typed.
///
/// **It correlates the whole crop, and [`coherence`] does not** -- that one takes a peak
/// inside each [`COH_TILE`] tile and means them, for the perspective reason at that constant.
/// So this grid is a *diagnosis of the crop*, useful for asking where the lag annulus has to
/// reach, and it is **not** the surface the metric maximises over: the two disagree by
/// construction, and the disagreement is the entire argument for tiling. For where a response
/// lives rather than where a lag does, the reading is `compare --tiles`, over
/// [`coherence_tiles`].
pub fn coherence_grid(a: &Img, r: Rect, max: i32) -> Vec<f64> {
    let field = Field::new(a, highpass_win(scale(a)));
    let rect = r.clamped(a);
    let mut out = Vec::with_capacity(((2 * max + 1) * (max + 1)) as usize);
    for dy in 0..=max {
        for dx in -max..=max {
            out.push(if dy == 0 && dx == 0 {
                1.0
            } else {
                field.rho(rect, dx, dy)
            });
        }
    }
    out
}

/// The capture's size against the one every constant here was authored at.
fn scale(a: &Img) -> f64 {
    a.h as f64 / super::vantage::STD_HEIGHT as f64
}

fn lag_px(v: u32, s: f64) -> i32 {
    ((v as f64 * s).round() as i32).max(1)
}

/// Odd, so the box has a centre pixel.
fn highpass_win(s: f64) -> u32 {
    odd_win(COH_HIGHPASS, s)
}

fn odd_win(v: u32, s: f64) -> u32 {
    ((v as f64 * s).round() as u32).max(3) | 1
}

/// The high-passed field, with the prefix sums that make a per-lag Pearson normalisation O(1).
///
/// Only the cross term needs to touch pixels. Every other quantity a correlation coefficient
/// wants -- the count, both means, both variances -- is a difference of four numbers, which is
/// what keeps a 2D lag sweep affordable enough to be run on every crop of every compare.
struct Field {
    w: i32,
    f: Vec<f64>,
    /// Prefix sums of `f` and of `f * f`, both `(w + 1) * (h + 1)`.
    sat: Vec<f64>,
    sat2: Vec<f64>,
}

impl Field {
    fn new(a: &Img, win: u32) -> Field {
        let (w, h) = (a.w as i32, a.h as i32);
        // Integer prefix sums of R + G + B, so the local mean is exact rather than an
        // accumulation of rounding over a 129-wide box.
        let mut isum = vec![0u64; ((w + 1) * (h + 1)) as usize];
        for y in 0..h {
            let mut row = 0u64;
            for x in 0..w {
                let p = ((y * w + x) * 4) as usize;
                row += (a.px[p] as u64) + (a.px[p + 1] as u64) + (a.px[p + 2] as u64);
                isum[((y + 1) * (w + 1) + x + 1) as usize] =
                    isum[(y * (w + 1) + x + 1) as usize] + row;
            }
        }
        let rad = (win / 2) as i32;
        let mut f = vec![0.0f64; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let x0 = (x - rad).max(0);
                let y0 = (y - rad).max(0);
                let x1 = (x + rad + 1).min(w);
                let y1 = (y + rad + 1).min(h);
                let s = isum[(y1 * (w + 1) + x1) as usize] + isum[(y0 * (w + 1) + x0) as usize]
                    - isum[(y0 * (w + 1) + x1) as usize]
                    - isum[(y1 * (w + 1) + x0) as usize];
                let n = ((x1 - x0) * (y1 - y0)) as f64 * 3.0;
                let p = ((y * w + x) * 4) as usize;
                let v = (a.px[p] as f64 + a.px[p + 1] as f64 + a.px[p + 2] as f64) / 3.0;
                f[(y * w + x) as usize] = v - s as f64 / n;
            }
        }
        let mut sat = vec![0.0f64; ((w + 1) * (h + 1)) as usize];
        let mut sat2 = vec![0.0f64; ((w + 1) * (h + 1)) as usize];
        for y in 0..h {
            let (mut r1, mut r2) = (0.0f64, 0.0f64);
            for x in 0..w {
                let v = f[(y * w + x) as usize];
                r1 += v;
                r2 += v * v;
                let i = ((y + 1) * (w + 1) + x + 1) as usize;
                sat[i] = sat[(y * (w + 1) + x + 1) as usize] + r1;
                sat2[i] = sat2[(y * (w + 1) + x + 1) as usize] + r2;
            }
        }
        Field {
            w,
            f,
            sat,
            sat2,
        }
    }

    fn box_sum(&self, s: &[f64], x0: i32, y0: i32, x1: i32, y1: i32) -> f64 {
        let i = |x: i32, y: i32| s[(y * (self.w + 1) + x) as usize];
        i(x1, y1) + i(x0, y0) - i(x0, y1) - i(x1, y0)
    }

    /// Pearson correlation between the crop and the crop shifted by `(dx, dy)`, over the pixels
    /// where both lie inside it.
    fn rho(&self, r: Rect, dx: i32, dy: i32) -> f64 {
        let x0 = (r.x0 as i32).max(r.x0 as i32 - dx);
        let y0 = (r.y0 as i32).max(r.y0 as i32 - dy);
        let x1 = (r.x1 as i32).min(r.x1 as i32 - dx);
        let y1 = (r.y1 as i32).min(r.y1 as i32 - dy);
        if x1 <= x0 || y1 <= y0 {
            return 0.0;
        }
        let n = ((x1 - x0) * (y1 - y0)) as f64;
        let sx = self.box_sum(&self.sat, x0, y0, x1, y1);
        let sxx = self.box_sum(&self.sat2, x0, y0, x1, y1);
        let sy = self.box_sum(&self.sat, x0 + dx, y0 + dy, x1 + dx, y1 + dy);
        let syy = self.box_sum(&self.sat2, x0 + dx, y0 + dy, x1 + dx, y1 + dy);
        let mut sxy = 0.0f64;
        for y in y0..y1 {
            let a = (y * self.w + x0) as usize;
            let b = ((y + dy) * self.w + x0 + dx) as usize;
            for i in 0..(x1 - x0) as usize {
                sxy += self.f[a + i] * self.f[b + i];
            }
        }
        let cov = sxy - sx * sy / n;
        let vx = sxx - sx * sx / n;
        let vy = syy - sy * sy / n;
        // A crop with no detail left after the high-pass has no structure to be regular, and
        // 0 is the honest reading of it -- not a division that would return whatever the
        // rounding left behind.
        if vx <= 1e-9 || vy <= 1e-9 {
            return 0.0;
        }
        cov / (vx * vy).sqrt()
    }
}

// ------------------------------------------------------------------ sRGB

/// The sRGB EOTF. `--screenshot` writes encoded values and anything that averages frames has
/// to come back here first -- averaging encoded values is a different operation and a wrong
/// one, which is why every reference in this project is described as "averaged in linear".
pub fn srgb_to_linear(v: u8) -> f32 {
    let c = v as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub fn linear_to_srgb(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let v = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (v * 255.0 + 0.5).clamp(0.0, 255.0) as u8
}

// ------------------------------------------------------------------ sha256

/// SHA-256 of a byte slice, lower-case hex.
///
/// Here rather than from a crate because the only thing this project hashes is a PNG it just
/// wrote, and `docs/sky.md` already records nine of them in this format -- a
/// dependency for sixty lines of fixed arithmetic would be a worse trade than the sixty lines.
pub fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let mut msg = data.to_vec();
    let bits = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_be_bytes());

    for block in msg.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d) = (h[0], h[1], h[2], h[3]);
        let (mut e, mut f, mut g, mut hh) = (h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    h.iter().map(|v| format!("{v:08x}")).collect()
}

pub fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(sha256_hex(&bytes))
}



