// Pass 2: one 8x8 workgroup per (tile, chunk) pair. Hits resolve against each other
// through a 64-bit visibility buffer, so atomicMax does depth testing for free.

@compute @workgroup_size(8, 8, 1)
fn march(@builtin(workgroup_id) wg: vec3<u32>, @builtin(local_invocation_id) lid: vec3<u32>) {
    let count = min(atomicLoad(&counters[0]), frame.max_pairs);
    // Dispatch is capped at 65535 workgroups, so each one may own several pairs.
    for (var p = wg.x; p < count; p = p + 65535u) {
        let pair = pairs[p];
        let tile = pair >> 13u;
        let ci = pair & 0x1FFFu;
        let px = (tile % frame.tiles.x) * 8u + lid.x;
        let py = (tile / frame.tiles.x) * 8u + lid.y;
        if px >= frame.res.x || py >= frame.res.y {
            continue;
        }
        // Mid-transition this chunk owns only some of the rays and its partner owns the
        // rest, so each ray still marches exactly one representation and per-ray traversal
        // stays 1.0x. The *frame* does not: both sides are emitted as pairs and each 8x8
        // workgroup runs at roughly the share it owns.
        if ray_skips(chunks[ci].fade, px, py) {
            continue;
        }
        let rd = ray_dir(f32(px) + 0.5, f32(py) + 0.5);
        // With the eye under the surface, water stops being geometry: marching from inside
        // a water voxel would hit it at t = 0 and fill the screen with a face pressed
        // against the lens. The medium is then applied by `resolve` over the analytic
        // distance to the sea plane instead.
        let underwater = (frame.flags & FLAG_UNDERWATER) != 0u;
        var h = march_chunk(ci, frame.cam_pos, rd, frame.far, underwater, false, false, false);
        // Batch 91, roadmap A2c (`--snell-bend`): inside Snell's window, march the world the
        // ray actually finds. What ships since batch 54 computes the window's weight exactly
        // (Fresnel from the wave normal in `resolve`) but colours it with whatever a straight
        // line found on the far side of the sea plane. This arm splits the submerged primary
        // path where the straight line crosses that plane and re-marches the transmitted
        // half along its refraction, so the compressed hemisphere above the surface is real
        // geometry and not a hole in it.
        //
        // **What it deliberately does not touch is `resolve`.** Everything the window needs
        // beyond the geometry -- the TIR cone, the Fresnel split, the mirror substitution --
        // is derived analytically there from the *original* `rd` and from `dist`, and the
        // bend preserves both as `t_surf + t2`, so the weight is unchanged and only which
        // geometry it lands on moves. A missed re-march or a TIR ray keeps `h` exactly as
        // the straight march left it, which is precisely the pre-arm picture.
        //
        // **The boundary facet here is flat, and that is a stated approximation to a stated
        // successor.** The wave normal lives in `resolve.wgsl` (batch 54's facet reading);
        // bringing it here means moving the wave field to `common.wgsl`, a splice worth its
        // own batch rather than a side effect of this one. The error this leaves -- a bend
        // computed against the mean surface while the Fresnel weight reads the perturbed one
        // -- is confined to the window's rim and is *strictly smaller* than what ships, which
        // is the same mismatch with `h` straight everywhere. It is the by-eye verdict's data.
        if SPEC_SNELL_BEND && underwater && !h.hit && rd.y > 0.0 {
            let depth = frame.sea_level - frame.cam_pos.y;
            if depth > 0.0 {
                let t_surf = depth / rd.y;
                if t_surf < frame.far {
                    let surf = frame.cam_pos + rd * t_surf;
                    // `refract` zero-encodes total internal reflection, which lands the TIR
                    // half of the cone on the straight march the mirror term already owns.
                    // eta is denser-to-lighter: WATER_IOR, not its reciprocal -- the same
                    // constant `shade_water` bends into the denser side with, read the way
                    // `refract` defines the ratio.
                    let rd2 = refract(rd, vec3<f32>(0.0, -1.0, 0.0), WATER_IOR);
                    if dot(rd2, rd2) > 0.0 {
                        let h2 = march_chunk(ci, surf + rd2 * SURFACE_EPS, rd2,
                                             frame.far - t_surf, true, false, false, false);
                        if h2.hit {
                            // Re-key as one path: `t` is distance from the camera along
                            // the bent line, so the atomic still resolves it against every
                            // chunk's straight hypotheses on one scale. The rest of the key
                            // (normal, cid, voxel) is segment 2's own -- the surface the
                            // ray *found*, which is the point. The iteration count is BOTH
                            // segments' -- this engine counts iterations in a debug view and
                            // a bend that ate segment 1's grind would lie there.
                            let straight_iters = h.iters;
                            h = h2;
                            h.t = t_surf + h2.t;
                            h.iters = straight_iters + h2.iters;
                        }
                        // A bent miss is the same sky the straight miss already encoded:
                        // both leave `.hit` clear, which is why this branch writes `h`
                        // and never mutates the miss.
                    }
                }
            }
        }
        let pix = py * frame.res.x + px;
        // The write is unconditional on a hit, and reading the word first to skip the ones
        // that cannot win was built, measured and thrown away in batch 50. Do not rebuild it.
        //
        // The idea is sound and bit-exact: `atomicMax` only ever raises this word, so a key
        // that loses a load would have lost the max -- including against a write racing in
        // between, which can only raise it further. Batch 49 had also measured that a
        // suppressed write here is worth real time, 91% of it billed to `resolve` rather than
        // to this pass. It still buys nothing: `resolve` **-0.029 +/- 0.021 at `underwater`**
        // (t = -1.35) and **exactly +0.000 at `default`**, against a build with the pretest
        // compiled out.
        //
        // **Hi-Z is why, and the count is the whole explanation.** An occlusion cull removes
        // precisely the chunks whose writes would lose, so by the time a pair reaches this
        // line the losers are already gone. Writes that would lose, hi-z off against hi-z on:
        // **58.0% -> 12.8% at `underwater`, 72.5% -> 21.5% at `default`, 69.8% -> 0.0% at
        // `cave`** -- Hi-Z removing 94.7%, 94.0% and 100% of them. `cave` reads 2,073,600 hits
        // for a 1920x1080 frame, exactly one per pixel: every tile marches only the camera's
        // own chunk, which the box test exempts, so nothing ever competes for a pixel.
        //
        // So a pretest pays one load on **every** hit to skip an atomic on **one in five to
        // one in eight**, and on none at all at `cave`. Retry only with a *cheaper read of the
        // same word* -- never with `atomicLoad`, which is what lost here -- and only if
        // something ever makes Hi-Z stop collecting this first. See `PERF.md`.
        if h.hit {
            atomicMax(&vis[pix], make_key(h.t, h.normal, ci, h.voxel, h.micro));
        }
        if (frame.flags & FLAG_HEATMAP) != 0u {
            atomicAdd(&dbg[pix], h.iters);
        }
    }
}

// Pass 3: per-tile farthest visible depth. Key 0 means sky, which disables culling
// for that tile, so this stays conservative.
var<workgroup> tile_min: atomic<u32>;

@compute @workgroup_size(8, 8, 1)
fn build_hiz(@builtin(workgroup_id) wg: vec3<u32>, @builtin(local_invocation_id) lid: vec3<u32>) {
    if lid.x == 0u && lid.y == 0u {
        atomicStore(&tile_min, 0xFFFFFFFFu);
    }
    workgroupBarrier();
    let px = wg.x * 8u + lid.x;
    let py = wg.y * 8u + lid.y;
    var d: u32 = 0u;
    if px < frame.res.x && py < frame.res.y {
        d = u32(atomicLoad(&vis[py * frame.res.x + px]) >> 40u);
    }
    atomicMin(&tile_min, d);
    workgroupBarrier();
    if lid.x == 0u && lid.y == 0u {
        hiz[wg.y * frame.tiles.x + wg.x] = atomicLoad(&tile_min);
    }
}



