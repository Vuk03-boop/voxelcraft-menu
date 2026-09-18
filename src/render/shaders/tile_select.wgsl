// Pass 1: for each 8x8 screen tile, list the chunks whose AABB the tile frustum touches.
// Testing the box instead of the tree is the point: box tests are almost free, and the
// expensive tree march then runs only on surviving (tile, chunk) pairs.

fn box_outside(bmin: vec3<f32>, bmax: vec3<f32>, n: vec3<f32>, cam: vec3<f32>) -> bool {
    // The most positive corner along n; if even that is behind the plane, the box is out.
    let p = select(bmin, bmax, n > vec3<f32>(0.0));
    return dot(n, p - cam) < 0.0;
}

fn box_distance(p: vec3<f32>, bmin: vec3<f32>, bmax: vec3<f32>) -> f32 {
    return length(max(max(bmin - p, p - bmax), vec3<f32>(0.0)));
}

// 8x8 and not 4x4: 4x4 is 16 threads, half a 32-lane warp, so half of every lane the
// pass schedules idled by construction, and at 34 registers it was nowhere near
// register-limited -- 16 blocks x 1 warp = 16 of 48 warp slots. One tile per thread off
// global_invocation_id, no workgroup state and no barrier, so the shape is free to change.
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
    // Orient every side plane inward.
    n0 = select(-n0, n0, dot(n0, center) >= 0.0);
    n1 = select(-n1, n1, dot(n1, center) >= 0.0);
    n2 = select(-n2, n2, dot(n2, center) >= 0.0);
    n3 = select(-n3, n3, dot(n3, center) >= 0.0);
    let cam = frame.cam_pos;
    let use_hiz = (frame.flags & FLAG_HIZ) != 0u;
    let tile_far = hiz[tile_id];

    // Batch 48: a tile every one of whose rays is still inside the water at
    // `WATER_FAR_DIST` stops there instead of at the view distance.
    //
    // **The test is the ray's water path and not its length**, and conflating the two is
    // what makes the obvious version of this wrong. `water_path` is analytic against the
    // sea plane, so a ray leaves the medium at `(sea_level - cam.y) / rd.y` and is not
    // attenuated one block further -- at the fixture's own submerged eye, three blocks
    // down, a ray only 21 degrees above the horizontal is out of the water within nine
    // blocks and can see a headland two kilometres away at full strength. Clamping
    // `frame.far` itself would delete that, and would do it to the top third of the frame.
    //
    // The most *upward* of the four corner rays is what decides, because it is the one that
    // escapes soonest: if even that ray is still submerged at `WATER_FAR_DIST`, every ray
    // the tile marches is. `rd.y <= 0` never leaves at all, which is the common case here
    // and the reason this pays -- it is the half of the frame pointing at the sea floor.
    //
    // Hoisted above the chunk loop deliberately: this is per *tile*, and the loop below runs
    // it against every resident chunk.
    var reach = frame.far;
    if (frame.flags & (FLAG_UNDERWATER | FLAG_WATER_FAR)) == (FLAG_UNDERWATER | FLAG_WATER_FAR) {
        let up = max(max(d00.y, d10.y), max(d11.y, d01.y));
        let submerged = max(frame.sea_level - cam.y, 0.0);
        // A ray heading level or down never reaches the surface; `1e30` is "never" and
        // compares the same way a real escape distance does, which keeps this one branch.
        var escape = 1e30;
        if up > 0.0 {
            escape = submerged / up;
        }
        if escape >= WATER_FAR_DIST {
            reach = min(frame.far, WATER_FAR_DIST);
            // Batch 53, and **nested rather than beside** the rung above on purpose: this
            // one covers `[WATER_DARK_DIST, WATER_FAR_DIST)` and the outer test is what
            // covers everything past that. Written as a sibling it would read as a second,
            // independent reason to stop, which it is not.
            //
            // **The interval is `[0, WATER_FAR_DIST]` and the two terms are its endpoints.**
            // Depth is linear in `t`, so a ray is under `WATER_DARK_DEPTH` throughout exactly
            // when it is under at both ends -- `submerged` is the depth at `t = 0` and
            // `dep_far` the depth where batch 48 stops. Which one binds depends on the sign
            // of `up`, and that is the whole reason both are here: a rising ray is shallowest
            // at the far end and a descending one at the eye.
            //
            // **Testing from the eye rather than from `WATER_DARK_DIST` is a decision and not
            // an oversight**, because the shorter interval is what the derivation actually
            // needs and this one is stricter. The shorter one lets a *shallow* camera in by
            // pitching down -- three blocks under, eleven degrees of tilt, and the rung fires
            // -- which is true (content that far down a steep ray really is unlit) and is the
            // wrong shape: the clamp would then be a property of where the player is
            // **looking**, so the far sea floor pops as they turn. Anchoring at the eye makes
            // it a property of where the player **is**, which changes at swimming speed and,
            // at that depth, between two frames that are both dark anyway. What the loose
            // version is worth is on file rather than guessed -- 4.7% of `sea-horizon`'s DDA
            // steps for **55,058 pixels at max delta 9** -- in `docs/errors.md`.
            //
            // The other three corner rays are at or below the most upward one at every `t`,
            // so bounding that one bounds the tile.
            let dep_far = submerged - up * WATER_FAR_DIST;
            if (frame.flags & FLAG_WATER_DARK) != 0u
                && min(submerged, dep_far) >= WATER_DARK_DEPTH {
                reach = min(reach, WATER_DARK_DIST);
            }
        }
    }

    for (var ci = 0u; ci < frame.chunk_count; ci = ci + 1u) {
        let c = chunks[ci];
        // The camera's own chunk is exempt: from inside the box, the ray-box depth is the
        // exit distance, which overestimates and would cull the chunk we stand in.
        if ci == frame.camera_chunk {
            let i = atomicAdd(&counters[0], 1u);
            if i < frame.max_pairs {
                pairs[i] = (tile_id << 13u) | ci;
            }
            continue;
        }
        // Near plane first, so boxes behind the camera cannot slip through the side planes.
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
            // Behind last frame's farthest visible surface: defer to the recovery pass.
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

// Re-test pairs the stale Hi-Z rejected, now against this frame's Hi-Z.
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
    // The raw demand, kept before the clamp takes it away. `counters[0]` is overwritten by
    // `finalize_recover` and a clamped count cannot say whether it clamped, so this is the
    // only place the number that means "geometry is missing from this frame" can live.
    // `--screenshot` above 5120x2880 reaches the cap and used to do it in silence: the near
    // water came back flat sky-miss blue and read perfectly plausibly as very smooth water.
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

// One thread per deferred pair, rounded up to whole workgroups.
@compute @workgroup_size(1, 1, 1)
fn finalize_deferred() {
    let n = min(atomicLoad(&counters[1]), frame.max_pairs);
    indirect[3] = (n + 63u) / 64u;
    indirect[4] = 1u;
    indirect[5] = 1u;
}



