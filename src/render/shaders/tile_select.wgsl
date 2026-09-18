fn box_outside(bmin: vec3<f32>, bmax: vec3<f32>, n: vec3<f32>, cam: vec3<f32>) -> bool {

    let p = select(bmin, bmax, n > vec3<f32>(0.0));
    return dot(n, p - cam) < 0.0;
}

fn box_distance(p: vec3<f32>, bmin: vec3<f32>, bmax: vec3<f32>) -> f32 {
    return length(max(max(bmin - p, p - bmax), vec3<f32>(0.0)));
}

@compute @workgroup_size(8, 8, 1)
fn tile_select(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= frame.tiles.x || gid.y >= frame.tiles.y {
        return;
    }
    let tile_id = gid.y * frame.tiles.x + gid.x;
    let x0 = f32(gid.x * 8u);
    let y0 = f32(gid.y * 8u);
    let x1 = x0 + 8.0;
    let y1 = y0 + 8.0;
    let d00 = ray_dir(x0, y0);
    let d10 = ray_dir(x1, y0);
    let d11 = ray_dir(x1, y1);
    let d01 = ray_dir(x0, y1);
    let center = ray_dir((x0 + x1) * 0.5, (y0 + y1) * 0.5);
    var n0 = cross(d00, d10);
    var n1 = cross(d10, d11);
    var n2 = cross(d11, d01);
    var n3 = cross(d01, d00);

    n0 = select(-n0, n0, dot(n0, center) >= 0.0);
    n1 = select(-n1, n1, dot(n1, center) >= 0.0);
    n2 = select(-n2, n2, dot(n2, center) >= 0.0);
    n3 = select(-n3, n3, dot(n3, center) >= 0.0);
    let cam = frame.cam_pos;
    let use_hiz = (frame.flags & FLAG_HIZ) != 0u;
    let tile_far = hiz[tile_id];

    var reach = frame.far;
    if (frame.flags & (FLAG_UNDERWATER | FLAG_WATER_FAR)) == (FLAG_UNDERWATER | FLAG_WATER_FAR) {
        let up = max(max(d00.y, d10.y), max(d11.y, d01.y));
        let submerged = max(frame.sea_level - cam.y, 0.0);

        var escape = 1e30;
        if up > 0.0 {
            escape = submerged / up;
        }
        if escape >= WATER_FAR_DIST {
            reach = min(frame.far, WATER_FAR_DIST);

            let dep_far = submerged - up * WATER_FAR_DIST;
            if (frame.flags & FLAG_WATER_DARK) != 0u
                && min(submerged, dep_far) >= WATER_DARK_DEPTH {
                reach = min(reach, WATER_DARK_DIST);
            }
        }
    }

    for (var ci = 0u; ci < frame.chunk_count; ci = ci + 1u) {
        let c = chunks[ci];

        if ci == frame.camera_chunk {
            let i = atomicAdd(&counters[0], 1u);
            if i < frame.max_pairs {
                pairs[i] = (tile_id << 13u) | ci;
            }
            continue;
        }

        if box_outside(c.aabb_min, c.aabb_max, frame.cam_fwd, cam) { continue; }
        if box_outside(c.aabb_min, c.aabb_max, n0, cam) { continue; }
        if box_outside(c.aabb_min, c.aabb_max, n1, cam) { continue; }
        if box_outside(c.aabb_min, c.aabb_max, n2, cam) { continue; }
        if box_outside(c.aabb_min, c.aabb_max, n3, cam) { continue; }
        let dmin = box_distance(cam, c.aabb_min, c.aabb_max);
        if dmin >= reach {
            continue;
        }
        if use_hiz && depth_key(dmin) < tile_far {

            let j = atomicAdd(&counters[1], 1u);
            if j < frame.max_pairs {
                pairs[frame.max_pairs + j] = (tile_id << 13u) | ci;
            }
            continue;
        }
        let i = atomicAdd(&counters[0], 1u);
        if i < frame.max_pairs {
            pairs[i] = (tile_id << 13u) | ci;
        }
    }
}

@compute @workgroup_size(64, 1, 1)
fn recover_select(@builtin(global_invocation_id) gid: vec3<u32>) {
    let n = min(atomicLoad(&counters[1]), frame.max_pairs);
    if gid.x >= n {
        return;
    }
    let pair = pairs[frame.max_pairs + gid.x];
    let ci = pair & 0x1FFFu;
    let c = chunks[ci];
    let dmin = box_distance(frame.cam_pos, c.aabb_min, c.aabb_max);
    if depth_key(dmin) >= hiz[pair >> 13u] {
        let k = atomicAdd(&counters[2], 1u);
        if k < frame.max_pairs {
            pairs[k] = pair;
        }
    }
}

@compute @workgroup_size(1, 1, 1)
fn finalize() {

    let raw = atomicLoad(&counters[0]);
    atomicStore(&counters[4], raw);
    let n = min(raw, frame.max_pairs);
    atomicStore(&counters[3], n);
    indirect[0] = min(n, 65535u);
    indirect[1] = 1u;
    indirect[2] = 1u;
}

@compute @workgroup_size(1, 1, 1)
fn finalize_recover() {
    let raw_recover = atomicLoad(&counters[2]);
    atomicStore(&counters[5], raw_recover);
    let n = min(raw_recover, frame.max_pairs);
    atomicStore(&counters[0], n);
    indirect[0] = min(n, 65535u);
    indirect[1] = 1u;
    indirect[2] = 1u;
}

@compute @workgroup_size(1, 1, 1)
fn finalize_deferred() {
    let n = min(atomicLoad(&counters[1]), frame.max_pairs);
    indirect[3] = (n + 63u) / 64u;
    indirect[4] = 1u;
    indirect[5] = 1u;
}
