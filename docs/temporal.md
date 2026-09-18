# The temporal pass, and the animation clock

TAA, and the two flags that let a capture see a *moving* world. **Consolidated from batches 6 and
23.** Ghosting defects are in [`errors.md`](errors.md); the LOD dissolve the clip resolves is in
[`lod.md`](lod.md).

---

## The pass

`taa` reprojects the previous frame through `prev_view_proj`, clips it to the current 3x3
neighbourhood and blends. **It runs in linear, before the tone map**, and it is the only pass
that reads a texture the previous frame wrote.

It costs **0.48 ms** and leaves `resolve` where it found it. With `--no-taa` the engine is
bit-identical to the pre-batch-6 build.

**Measured against a 16x supersampled ground truth, not by eye**: the same frame at 5120x2880
box-averaged 4x4 down to 1280x720 **in radiance** -- sRGB decoded and the tone-map shoulder
inverted before averaging, because averaging display values is the gamma-space mistake batch 1
fixed. Rendered with `--mip-bias 2` so texture LOD matches, leaving only geometric aliasing in
the error. **RMSE 3.084 to 1.234, -60%** on a still camera.

---

## Why the constants are what they are

- **`hit_t` is shared by `resolve` and `taa`, and that sharing is the point.** The key's 24-bit
  depth field exists to make `atomicMax` do the depth test and is far too coarse to reconstruct
  a position from; both passes re-derive `t` from the stored face. If they ever disagreed, a
  surface would sample its history from a slightly wrong place -- a permanent sub-pixel smear no
  clip would catch, because the colour it reads is plausible.
- **The reprojected position has this frame's `jitter` subtracted.** The history grid is indexed
  by pixel centres; tracking a point sampled a third of a pixel off centre and resampling the
  history *there* would resample by the jitter every frame, and at feedback 0.15 a per-frame
  filter compounds about sevenfold. Subtracting it makes a still camera land on the exact texel
  centre, where the Catmull-Rom weights collapse to (0,1,0,0) and the filter is the identity.
  `tests/camera.rs` pins that for all eight phases.
- **The sky is reprojected as a direction, with `w = 0`** -- a miss has no position, and a zero
  w drops the translation out of the matrix. One line instead of a special case.
- **`TAA_FEEDBACK` is 0.15, not the 0.1 that was guessed.** At 0.10 the still image is
  marginally better (1.192 vs 1.234) but the fastest camera measured comes out **worse than no
  temporal pass at all** (3.126 vs 3.084): under motion the clip fires on most pixels, and
  holding 90% of a clipped value converges to the edge of the box rather than to anything true.
  0.15 keeps 97% of the still-camera gain and is never worse than no TAA at any speed measured.
  0.20 and 0.30 are better under motion and materially worse standing still (1.319, 1.590) -- a
  real knee, not a plateau.
- **`TAA_CLIP_SIGMA` is 1.0 and wider is not better.** 1.5 or 2.0 buys 8-11% on a still camera
  and costs 5-9% under motion; by 1.5 the fast-motion error has passed no-TAA. **1.0 is the only
  setting measured that never loses to switching the pass off.** This is the voxel-content
  problem: every voxel edge is a real discontinuity, so a loose box keeps samples from the far
  side of one. (It is also doing most of the LOD dissolve -- see [`lod.md`](lod.md).)
- **The clip is in YCoCg and pulls along the line to the box centre.** Clamping each channel
  independently invents a colour that was never in the neighbourhood. YCoCg because an edge
  between two colours of similar brightness has a box thin across chroma and wide along luma,
  which an RGB box cannot express.
- **The history resample is Catmull-Rom, not bilinear.** A bilinear resample is a low-pass filter
  applied once per frame, which at this feedback is roughly seven blurs deep -- the image
  dissolves. The five-tap form drops the four corners of the 3x3 and renormalises: they carry
  1.6% of the weight at the worst offset and 44% of the taps.
- **The 3x3 neighbourhood comes out of workgroup memory and the barrier is unconditional.** 100
  loads per 8x8 tile instead of 576. It must run **before** the bounds check, because a barrier
  only some of the workgroup reaches is undefined behaviour and the edge workgroups are exactly
  the ones with threads past the end of the image. Worth 0.02 ms of 0.50.
- **`mip_bias` is derived by the renderer, not asked of the caller** -- `log2(render height /
  window height)` plus whatever `--mip-bias` adds. The footprint in `resolve` divides by
  `frame.res.y`, so `--scale 0.5` picks a blurrier mip and leaves the reconstructor nothing to
  work with. At scale 1.0 it is exactly zero.
- **Feedback is forced to 1.0 whenever the history is worthless** -- the first frame, after a
  resize, and after the pass was toggled with F7. That is the reset; there is no separate flag.

**`taa` gets its own bind groups (`taa_bind_group[parity]`) rather than an extra binding**,
because a texture cannot be a write storage binding and a sampled binding of the same bind group
-- wgpu merges every resource of a bind group into the pass's usage scope. Binding 11 is whatever
that pass writes; 16/17 are the two it only reads.

---

## Jitter

**`jitter` is a pixel offset applied inside `ray_dir`**, not a skew of the projection, so every
pass shifts together -- `tile_select` builds its tile frustum from the same function and still
bounds exactly the rays its own tile marches. Verified at whole-pixel scale: **`--jitter 1,0`
reproduces the unjittered capture translated one pixel, bit for bit.**

**A `--screenshot` accumulates `--taa-frames` (default 32) and captures the last.** It is still
one deterministic image -- the camera and the eight-phase jitter cycle are fixed. 32 against 48
differ by RMSE 0.048 with max delta 2/255; 32 against 96, the same RMSE with max 4.

**But a capture's pixels depend on how many frames preceded it.** `frame_index` walks the
stochastic LOD dither and the god-ray sampler's stratification one step per frame, so a
`--no-taa` capture (5 frames, captured at index 4) and a `--taa-frames 32` capture are at
different dither phases. It has never made an A/B wrong, because both sides render the same
count -- but anything that renders its own frames before capturing has to land on index 4 to
match a plain capture, which is why `--reference`'s warmup is four and not five.

---

## The animation clock

Until batch 23 every headless mode pinned `frame.time` to zero, so no capture, test or hash could
say anything about a *moving* world. `--anim-time F` and `--anim-rate F` are that instrument.

**Both ship at 0**, which is what every capture before batch 23 was taken at, so the pair is
bit-exact at all fourteen vantages.

**`frame.time` has exactly two readers -- `wave_speed` and `cloud_speed` -- and that is measured
rather than asserted.** `--no-waves --cloud-cover 0 --anim-time 4` against the same two flags
alone is **0 pixels at fourteen vantages** with the water and the world intact, and
`frame_time_is_read_only_by_the_two_animation_phases` in `tests/textures.rs` parses
`shader_source()` to keep it that way.

**The rate is the load-bearing half and was not in the brief.** With a pinned *phase* all 32
accumulated frames show the identical sea, so the temporal pass reprojects a still image onto
itself and **nothing can ghost**. Only a non-zero rate moves the field while the frame
accumulates.

**`--anim-rate` and `--cam-dolly` measure different things and are deliberately separate flags.**
The dolly moves the *camera* and disoccludes at every silhouette; the rate moves the *field*
under a camera standing still and disoccludes nothing, so whatever ghosts under it is the sea's
or the deck's own history.

Measured once: **the deck does not ghost at all** (0.018 of a code value, max 2), and **the
moving sea ghosts 0.34-0.66 and is 0.84x *quieter* than the still one for it** -- motion is a net
anti-aliaser here. That last number resized batch 19's speckle item without touching it: a batch
that drives still-frame speckle to 1.00x will be over-filtering the moving sea.

**A rotating camera cannot measure ghosting and a translating one can.** `--cam-spin` alone
barely separated the variants (0.7% across a 3x range of sigma) because pure rotation reprojects
almost perfectly and disoccludes only at the screen edge. `--cam-dolly` spreads the same sweep
over 14%. **Any future claim about ghosting needs the dolly.**

---

## Controls

| flag | reproduces | cost |
|---|---|---|
| `--no-taa` | the single-frame image, bit-identical to pre-batch-6 | -- |
| `--jitter X[,Y]` | pins the sample offset | -- |
| `--anim-time F`, `--anim-rate F` | both ship at 0; bit-exact at all fourteen | -- |
| `--taa-frames N` | how many frames a capture accumulates (default 32) | -- |
| `--scale F` | trace at F times the presented resolution, routed through `resize` so the renderer derives its own mip bias. **Not a general supersampler** -- the blit resolves with one bilinear tap, an exact box only at F = 2 | -- |



