// The light envelope's scan (batch 35). One pass of a max-plus prefix scan over the terrain
// height window, run `SHAFT_STEPS` times with `SHAFT_STEP` folded in as an override.
//
// **The recurrence.** A point is in terrain shadow if anything along the ray toward the sun is
// above that ray. Walking that ray horizontally by `d` raises it by `d * tan(elevation)`, so
// the lowest lit altitude at a column is
//
//     E(q) = max over d >= 0 of [ h(q + shat * d) - d * tan(elevation) ]
//
// with `shat` the horizontal direction toward the sun. `d = 0` is in the maximum on purpose:
// it is what makes a point *below* the ground come out shadowed without a second test.
//
// **Why a scan and not a gather.** Written as a gather that is 256 taps per texel. Written as
// a doubling scan it is one tap per texel per pass and eight passes, because `max` with an
// additive offset is an associative operator -- a semiring -- and Hillis-Steele applies to it
// unchanged. After `k` passes the window is `2^k` texels, so the reach is the whole of the
// budget rather than a truncation of it.
//
// **It is near the gather rather than equal to it**, and the difference is the bilinear tap
// below: exactness holds over an integer lattice, and the sun's azimuth is not axis-aligned,
// so a value carried three texels by a one-step and a two-step pass has been interpolated
// twice where a gather interpolates it once. Measured at 0.6 blocks against the real
// generator, which is a seventh of `shaft_soft` -- inside the ramp the edge is blurred over,
// and pinned there rather than at a float epsilon by `the_scan_is_the_envelope_it_claims_to_be`.
//
// **Why it ping-pongs rather than running in place.** In place, a thread reading a neighbour
// another thread has already advanced would take a *longer* reach than the pass is entitled
// to -- which is harmless for the answer, since a longer reach is more correct, and fatal for
// this project, because the result would then depend on scheduling. Every measurement here
// rests on two runs of the same arguments producing the same file, and `harness validate`
// checks exactly that before it checks anything else.

// Which doubling this dispatch is, 0-based. An `override` rather than a uniform because the
// value is fixed per pipeline and known at compile time: `1u << k` and the two slice bases
// all fold, so the pass is a load, a lerp, a subtract and a max.
override SHAFT_STEP: f32 = 0.0;

@compute @workgroup_size(8, 8, 1)
fn shaft_scan(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= SHAFT_DIM || gid.y >= SHAFT_DIM {
        return;
    }
    let k = u32(SHAFT_STEP);
    let idx = gid.y * SHAFT_DIM + gid.x;
    // Pass 0 reads the static heights; every later pass reads what the one before it wrote.
    // `src` and `dst` are never the same slice, which is what makes the pass race-free.
    var src = SHAFT_HEIGHTS;
    if k > 0u {
        src = ((k - 1u) & 1u) * SHAFT_SLICE;
    }
    let dst = (k & 1u) * SHAFT_SLICE;
    let span = f32(1u << k);
    // A sun at the zenith has no horizontal direction to sweep along, and the clamp is what
    // stops the division rather than a branch: a tiny `horiz` makes `climb` enormous, the
    // carried term collapses to minus infinity, and the envelope falls back to the terrain
    // itself -- which is the right answer for an overhead sun. At night `sun_dir.y` is
    // negative and the envelope grows without bound, also right, and never read: the lobe is
    // zero, so `sun_shaft` has already returned.
    let horiz = max(length(frame.sun_dir.xz), 1e-3);
    let dir = frame.sun_dir.xz / horiz;
    let climb = span * frame.shaft_texel * (frame.sun_dir.y / horiz);
    let here = shaft[src + idx];
    let carried = shaft_sample(src, vec2<f32>(f32(gid.x), f32(gid.y)) + dir * span) - climb;
    shaft[dst + idx] = max(here, carried);
}



