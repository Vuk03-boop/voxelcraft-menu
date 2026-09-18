// Pass 6: temporal accumulation. Runs between `resolve` and the blit, so it works in the
// linear HDR the rest of the pipeline speaks and never sees the tone map -- resolving
// display-referred colour would put back the accumulation error batch 2 removed.
//
// **Motion vectors are synthesised, never stored.** `march` writes a visibility key and
// nothing else; `hit_t` turns that key back into the exact world-space point the pixel saw,
// and one multiply by last frame's matrix says where that point was. A velocity buffer
// would cost a write and a read per pixel for something two lines of algebra already have.
//
// The reprojected position has this frame's `jitter` subtracted off it. The history grid is
// indexed by pixel centres, so tracking a point that was sampled a third of a pixel off
// centre and then resampling the history there would smear the accumulator by the jitter
// every single frame. Subtracting it makes a still camera land on the exact texel centre,
// where the Catmull-Rom weights below collapse to the identity.

// Half-width of the neighbourhood colour box, in standard deviations. Voxel content is the
// hard case for this number: every voxel edge is a real discontinuity, so a wide box lets a
// stale sample from the far side of an edge survive and read as a smear. 1.0 keeps the clip
// tight enough that a moving camera does not trail, and the eight-phase jitter still
// converges because the samples a still pixel accumulates all lie inside its own box.
const TAA_CLIP_SIGMA: f32 = 1.0;

// YCoCg, so the clip box is aligned to luma and chroma rather than to the three primaries.
// An edge between two colours of similar brightness has a box that is thin across chroma
// and wide along luma, which an RGB box cannot express.
fn rgb_to_ycocg(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        0.25 * c.r + 0.5 * c.g + 0.25 * c.b,
        0.5 * c.r - 0.5 * c.b,
        -0.25 * c.r + 0.5 * c.g - 0.25 * c.b,
    );
}

fn ycocg_to_rgb(c: vec3<f32>) -> vec3<f32> {
    let t = c.x - c.z;
    return vec3<f32>(t + c.y, c.x + c.z, t - c.y);
}

// Pull the history back to the surface of the box along the line to its centre, rather than
// clamping each channel independently. A per-channel clamp invents a colour that was never
// in the neighbourhood; this one lands on a point of the segment between the two.
fn clip_to_box(lo: vec3<f32>, hi: vec3<f32>, c: vec3<f32>) -> vec3<f32> {
    let centre = 0.5 * (hi + lo);
    let extent = max(0.5 * (hi - lo), vec3<f32>(1e-5));
    let v = c - centre;
    let n = abs(v / extent);
    let m = max(n.x, max(n.y, n.z));
    if m > 1.0 {
        return centre + v / m;
    }
    return c;
}

// Catmull-Rom through five bilinear taps. Reprojection lands between texels almost always,
// and a plain bilinear resample of the history is a low-pass filter applied once per frame
// -- at feedback 0.1 that is ten blurs deep and the image dissolves. The cubic's negative
// lobes put the sharpening back.
//
// The four corner taps of the 3x3 are dropped and the weights renormalised, which is the
// standard trade: they carry a few percent of the weight and cost 44% of the bandwidth.
// At a whole-texel offset the weights collapse to (0, 1, 0, 0) and the sum to exactly 1, so
// a still camera reads its history back bit for bit instead of slowly grinding it down.
fn history_bicubic(p: vec2<f32>) -> vec3<f32> {
    let res = vec2<f32>(frame.res);
    let tex1 = floor(p - 0.5) + 0.5;
    let f = p - tex1;
    let f2 = f * f;
    let f3 = f2 * f;
    let w0 = -0.5 * f3 + f2 - 0.5 * f;
    let w1 = 1.5 * f3 - 2.5 * f2 + 1.0;
    let w2 = -1.5 * f3 + 2.0 * f2 + 0.5 * f;
    let w3 = 0.5 * f3 - 0.5 * f2;
    // w1 + w2 = 1 + 0.5 * f * (1 - f), which never drops below 1 on [0, 1].
    let w12 = w1 + w2;
    let t0 = (tex1 - 1.0) / res;
    let t3 = (tex1 + 2.0) / res;
    let t12 = (tex1 + w2 / w12) / res;
    var c = vec3<f32>(0.0);
    c += textureSampleLevel(taa_hist, linear_samp, vec2<f32>(t12.x, t0.y), 0.0).rgb * (w12.x * w0.y);
    c += textureSampleLevel(taa_hist, linear_samp, vec2<f32>(t0.x, t12.y), 0.0).rgb * (w0.x * w12.y);
    c += textureSampleLevel(taa_hist, linear_samp, vec2<f32>(t12.x, t12.y), 0.0).rgb * (w12.x * w12.y);
    c += textureSampleLevel(taa_hist, linear_samp, vec2<f32>(t3.x, t12.y), 0.0).rgb * (w3.x * w12.y);
    c += textureSampleLevel(taa_hist, linear_samp, vec2<f32>(t12.x, t3.y), 0.0).rgb * (w12.x * w3.y);
    let sum = w12.x * w0.y + w0.x * w12.y + w12.x * w12.y + w3.x * w12.y + w12.x * w3.y;
    // The cubic overshoots, and a negative radiance is not a colour.
    return max(c / sum, vec3<f32>(0.0));
}

// Where the point this pixel sees was on last frame's screen, in pixels. `.z` is 0 when
// there is no answer -- the point was behind the previous camera, or off its screen.
fn reproject(px: u32, py: u32, rd: vec3<f32>) -> vec3<f32> {
    let key = atomicLoad(&vis[py * frame.res.x + px]);
    var clip: vec4<f32>;
    if key == 0lu {
        // The sky has no position, only a direction, so reproject it as one: a direction is
        // a point at infinity, which is exactly what a zero w does to the translation.
        clip = frame.prev_view_proj * vec4<f32>(rd, 0.0);
    } else {
        let normal_id = u32(key >> 37u) & 7u;
        let ci = u32(key >> 24u) & 0x1FFFu;
        let low = u32(key & 0xFFFFFFlu);
        let v = vec3<u32>((low >> 16u) & VOXEL_MASK, (low >> 8u) & VOXEL_MASK, low & VOXEL_MASK);
        let micro = vec3<u32>((low >> 22u) & 3u, (low >> 14u) & 3u, (low >> 6u) & 3u);
        let hit = frame.cam_pos + rd * hit_t(ci, v, micro, normal_id, rd);
        clip = frame.prev_view_proj * vec4<f32>(hit, 1.0);
    }
    if clip.w <= 1e-6 {
        return vec3<f32>(0.0);
    }
    let ndc = clip.xy / clip.w;
    let res = vec2<f32>(frame.res);
    let p = vec2<f32>((ndc.x * 0.5 + 0.5) * res.x, (0.5 - ndc.y * 0.5) * res.y) - frame.jitter;
    if any(p < vec2<f32>(0.0)) || any(p > res) {
        return vec3<f32>(0.0);
    }
    return vec3<f32>(p, 1.0);
}

// The 8x8 workgroup plus its one-pixel halo. Every pixel wants the 3x3 around it, and
// eight of those nine cells belong to a neighbour in the same workgroup: 100 loads for the
// tile instead of 576.
var<workgroup> tile: array<vec3<f32>, 100>;

@compute @workgroup_size(8, 8, 1)
fn taa(@builtin(global_invocation_id) gid: vec3<u32>,
       @builtin(workgroup_id) wg: vec3<u32>,
       @builtin(local_invocation_id) lid: vec3<u32>,
       @builtin(local_invocation_index) li: u32) {
    let last = vec2<i32>(i32(frame.res.x) - 1, i32(frame.res.y) - 1);
    let origin = vec2<i32>(i32(wg.x) * 8 - 1, i32(wg.y) * 8 - 1);
    // Unconditional, and before the bounds check: a barrier that only some of the
    // workgroup reaches is undefined behaviour, and the edge workgroups are exactly the
    // ones with threads past the end of the image.
    for (var k = li; k < 100u; k = k + 64u) {
        let q = clamp(origin + vec2<i32>(i32(k % 10u), i32(k / 10u)), vec2<i32>(0), last);
        tile[k] = textureLoad(taa_src, q, 0).rgb;
    }
    workgroupBarrier();

    let px = gid.x;
    let py = gid.y;
    if px >= frame.res.x || py >= frame.res.y {
        return;
    }
    let coord = vec2<i32>(i32(px), i32(py));
    let cur = tile[(lid.y + 1u) * 10u + lid.x + 1u];
    var out_c = cur;

    // The heatmap is a per-frame iteration counter, not radiance. Blending it across frames
    // would only make it lie, so it passes straight through.
    let heat = (frame.flags & FLAG_HEATMAP) != 0u;
    if frame.taa_feedback < 1.0 && !heat {
        let reprojected = reproject(px, py, ray_dir(f32(px) + 0.5, f32(py) + 0.5));
        if reprojected.z > 0.0 {
            // The 3x3 of *this* frame around the pixel, as the range of colour the surface
            // plausibly has here. Mean and variance come out of one pass over the nine.
            var m1 = vec3<f32>(0.0);
            var m2 = vec3<f32>(0.0);
            for (var i = 0u; i < 9u; i = i + 1u) {
                let c = rgb_to_ycocg(tile[(lid.y + i / 3u) * 10u + lid.x + i % 3u]);
                m1 += c;
                m2 += c * c;
            }
            let mean = m1 / 9.0;
            let sigma = sqrt(max(m2 / 9.0 - mean * mean, vec3<f32>(0.0)));

            let hist = rgb_to_ycocg(history_bicubic(reprojected.xy));
            let clipped = clip_to_box(mean - TAA_CLIP_SIGMA * sigma, mean + TAA_CLIP_SIGMA * sigma, hist);
            out_c = mix(max(ycocg_to_rgb(clipped), vec3<f32>(0.0)), cur, frame.taa_feedback);
        }
    }
    textureStore(out_tex, coord, vec4<f32>(out_c, 1.0));
}



