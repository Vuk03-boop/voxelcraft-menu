override SHAFT_STEP: f32 = 0.0;

@compute @workgroup_size(8, 8, 1)
fn shaft_scan(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= SHAFT_DIM || gid.y >= SHAFT_DIM {
        return;
    }
    let k = u32(SHAFT_STEP);
    let idx = gid.y * SHAFT_DIM + gid.x;

    var src = SHAFT_HEIGHTS;
    if k > 0u {
        src = ((k - 1u) & 1u) * SHAFT_SLICE;
    }
    let dst = (k & 1u) * SHAFT_SLICE;
    let span = f32(1u << k);

    let horiz = max(length(frame.sun_dir.xz), 1e-3);
    let dir = frame.sun_dir.xz / horiz;
    let climb = span * frame.shaft_texel * (frame.sun_dir.y / horiz);
    let here = shaft[src + idx];
    let carried = shaft_sample(src, vec2<f32>(f32(gid.x), f32(gid.y)) + dir * span) - climb;
    shaft[dst + idx] = max(here, carried);
}
