const TAA_CLIP_SIGMA: f32 = 1.0;

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

    return max(c / sum, vec3<f32>(0.0));
}

fn reproject(px: u32, py: u32, rd: vec3<f32>) -> vec3<f32> {
    let key = atomicLoad(&vis[py * frame.res.x + px]);
    var clip: vec4<f32>;
    if key == 0lu {

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

var<workgroup> tile: array<vec3<f32>, 100>;

@compute @workgroup_size(8, 8, 1)
fn taa(@builtin(global_invocation_id) gid: vec3<u32>,
       @builtin(workgroup_id) wg: vec3<u32>,
       @builtin(local_invocation_id) lid: vec3<u32>,
       @builtin(local_invocation_index) li: u32) {
    let last = vec2<i32>(i32(frame.res.x) - 1, i32(frame.res.y) - 1);
    let origin = vec2<i32>(i32(wg.x) * 8 - 1, i32(wg.y) * 8 - 1);

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

    let heat = (frame.flags & FLAG_HEATMAP) != 0u;
    if frame.taa_feedback < 1.0 && !heat {
        let reprojected = reproject(px, py, ray_dir(f32(px) + 0.5, f32(py) + 0.5));
        if reprojected.z > 0.0 {

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
