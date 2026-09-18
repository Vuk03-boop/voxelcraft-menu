@compute @workgroup_size(8, 8, 1)
fn march(@builtin(workgroup_id) wg: vec3<u32>, @builtin(local_invocation_id) lid: vec3<u32>) {
    let count = min(atomicLoad(&counters[0]), frame.max_pairs);

    for (var p = wg.x; p < count; p = p + 65535u) {
        let pair = pairs[p];
        let tile = pair >> 13u;
        let ci = pair & 0x1FFFu;
        let px = (tile % frame.tiles.x) * 8u + lid.x;
        let py = (tile / frame.tiles.x) * 8u + lid.y;
        if px >= frame.res.x || py >= frame.res.y {
            continue;
        }

        if ray_skips(chunks[ci].fade, px, py) {
            continue;
        }
        let rd = ray_dir(f32(px) + 0.5, f32(py) + 0.5);

        let underwater = (frame.flags & FLAG_UNDERWATER) != 0u;
        var h = march_chunk(ci, frame.cam_pos, rd, frame.far, underwater, false, false, false);

        if SPEC_SNELL_BEND && underwater && !h.hit && rd.y > 0.0 {
            let depth = frame.sea_level - frame.cam_pos.y;
            if depth > 0.0 {
                let t_surf = depth / rd.y;
                if t_surf < frame.far {
                    let surf = frame.cam_pos + rd * t_surf;

                    let rd2 = refract(rd, vec3<f32>(0.0, -1.0, 0.0), WATER_IOR);
                    if dot(rd2, rd2) > 0.0 {
                        let h2 = march_chunk(ci, surf + rd2 * SURFACE_EPS, rd2,
                                             frame.far - t_surf, true, false, false, false);
                        if h2.hit {

                            let straight_iters = h.iters;
                            h = h2;
                            h.t = t_surf + h2.t;
                            h.iters = straight_iters + h2.iters;
                        }

                    }
                }
            }
        }
        let pix = py * frame.res.x + px;

        if h.hit {
            atomicMax(&vis[pix], make_key(h.t, h.normal, ci, h.voxel, h.micro));
        }
        if (frame.flags & FLAG_HEATMAP) != 0u {
            atomicAdd(&dbg[pix], h.iters);
        }
    }
}

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
