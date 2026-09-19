fn surface_sky_fill(n: vec3<f32>) -> vec3<f32> {
    let sky = sky_ambient_tint(n);
    if !SPEC_LIGHTING_REPAIR { return sky; }

    return sky * (0.60 * mix(0.35, 1.0, clamp(n.y * 0.5 + 0.5, 0.0, 1.0)));
}
fn surface_light(ambient: vec3<f32>, sun: vec3<f32>, ao: f32, floor_light: vec3<f32>, face: f32) -> vec3<f32> {
    if !SPEC_LIGHTING_REPAIR { return ((ambient + sun) * ao + floor_light) * face; }

    return ambient * ao * face + sun + floor_light;
}
fn surface_radiance(albedo: vec3<f32>, light: vec3<f32>, spec: vec3<f32>, id: u32) -> vec3<f32> {
    if SPEC_LIGHTING_REPAIR && id == GLOWSTONE_ID {
        return albedo * (vec3<f32>(1.0, 0.7111, 0.3024) * 1.5 + light * 0.15);
    }
    return albedo * light + spec;
}

fn face_shade(normal_id: u32) -> f32 {
    switch normal_id {
        case 3u: { return 1.00; }
        case 2u: { return 0.50; }
        case 4u, 5u: { return 0.80; }

        case 6u, 7u: { return 0.90; }
        default: { return 0.62; }
    }
}

fn block_ambient(blk: vec3<f32>, sk: f32, sky: vec3<f32>) -> vec3<f32> {
    let bk = max(blk.r, max(blk.g, blk.b));
    let tint = blk / max(bk, 1e-4);
    return mix(sky, tint, bk / max(sk + bk, 1e-4)) * max(sk, bk);
}

fn heat(v: f32) -> vec3<f32> {
    let t = clamp(v, 0.0, 1.0);
    let c = vec3<f32>(
        smoothstep(0.35, 0.85, t),
        smoothstep(0.0, 0.45, t) - smoothstep(0.65, 1.0, t),
        1.0 - smoothstep(0.0, 0.45, t),
    );
    return pow(c, vec3<f32>(2.2));
}

fn light_curve(level: f32) -> f32 {
    let l = level / 15.0;
    return l * l * (0.6 + 0.4 * l);
}

fn light_curve3(level: vec3<f32>) -> vec3<f32> {
    let l = level / 15.0;
    return l * l * (vec3<f32>(0.6) + 0.4 * l);
}

const SKY_TINT: vec3<f32> = vec3<f32>(0.6038, 0.7084, 1.0000);

fn luma3(c: vec3<f32>) -> f32 {
    return 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
}

const SKY_W_UP: f32 = 0.78431373;
const SKY_W_SIDE: f32 = 0.58578900;

const SKY_AMBIENT_REF: vec3<f32> = vec3<f32>(0.14658824, 0.30178431, 0.84105882);

const SKY_TINT_LUMA: f32 = 0.70721556;

const SKY_HALO_ENERGY: f32 = 4.0;

const SKY_TINT_SAT: f32 = 2.5;

fn sky_ambient_tint(n: vec3<f32>) -> vec3<f32> {
    if !SPEC_SKY_TINT {
        return SKY_TINT;
    }
    let day = frame.daylight;

    let horizon = mix(SKY_NIGHT_HORIZON, SKY_DAY_HORIZON, day);
    let zenith = mix(SKY_NIGHT_ZENITH, SKY_DAY_ZENITH, day);
    let w = select(select(SKY_W_SIDE, SKY_W_UP, n.y > 0.5), 0.0, n.y < -0.5);
    var c = mix(horizon, zenith, w);

    c = c + SCATTER_TINT
        * (frame.fog_scatter * day * SKY_HALO_ENERGY * max(dot(n, frame.sun_dir), 0.0));

    let t = SKY_TINT * pow(c / SKY_AMBIENT_REF, vec3<f32>(SKY_TINT_SAT));
    return t * (SKY_TINT_LUMA / max(luma3(t), 1e-6));
}

const BLOCK_TINT: vec3<f32> = vec3<f32>(1.0000, 0.6500, 0.3200);

const AO_STRENGTH: f32 = 0.25;

const PROBE_DIM: f32 = 128.0;
const PROBE_SPACING: f32 = 4.0;
const PROBE_INV_SPAN: f32 = 1.0 / (PROBE_DIM * PROBE_SPACING);
const PROBE_TAPS: u32 = 1u;

const PROBE_TAP_STRIDE: f32 = 0.37;

const PROBE_SLABS: f32 = 6.0;

const PROBE_NORMAL_BIAS: f32 = PROBE_SPACING * 0.5;

fn probe_field(p: vec3<f32>, normal_id: u32) -> vec4<f32> {
    let cross = normal_id >= NORMAL_CROSS_A;
    let slab = select(normal_id, 3u, cross);

    let t = p * (1.0 / PROBE_SPACING);
    let y = clamp(t.y, 0.5, PROBE_DIM - 0.5);
    let uvw = vec3<f32>(
        t.x * (1.0 / PROBE_DIM),
        (f32(slab) * PROBE_DIM + y) * (1.0 / (PROBE_DIM * PROBE_SLABS)),
        t.z * (1.0 / PROBE_DIM),
    );
    return textureSampleLevel(probe_tex, probe_samp, uvw, 0.0);
}

fn probe_tap(p: vec3<f32>) -> vec3<f32> {
    var acc = vec3<f32>(1.0);
    for (var i = 0u; i < PROBE_TAPS; i = i + 1u) {
        let uvw = fract(p * PROBE_INV_SPAN + vec3<f32>(0.0, 0.0, f32(i) * PROBE_TAP_STRIDE));
        acc = acc * textureSampleLevel(probe_tex, linear_samp, uvw, 0.0).rgb;
    }
    return acc;
}

fn face_uv(normal_id: u32, local: vec3<f32>) -> vec2<f32> {
    if normal_id >= NORMAL_CROSS_A {

        return vec2<f32>(local.x, 1.0 - local.y);
    }
    let axis = normal_id >> 1u;
    var uv: vec2<f32>;
    if axis == 0u {
        uv = vec2<f32>(local.z, 1.0 - local.y);
    } else if axis == 1u {
        uv = vec2<f32>(local.x, local.z);
    } else {
        uv = vec2<f32>(local.x, 1.0 - local.y);
    }
    if normal_id == 1u || normal_id == 4u {
        uv.x = 1.0 - uv.x;
    }
    return uv;
}

fn perm_key(box_min: vec3<f32>, normal_id: u32) -> u32 {
    let c = vec3<i32>(floor(box_min));
    return hash_u32(bitcast<u32>(c.x) * 0x9e3779b9u
        ^ bitcast<u32>(c.y) * 0x85ebca6bu
        ^ bitcast<u32>(c.z) * 0xc2b2ae35u
        ^ normal_id * 0x27d4eb2fu);
}

fn permute_uv(uv: vec2<f32>, pclass: u32, h: u32) -> vec2<f32> {
    let d4 = pclass == PERM_D4;

    var p = select(uv, vec2<f32>(uv.y, uv.x), d4 && (h & 1u) != 0u);
    p.x = select(p.x, 1.0 - p.x, pclass != PERM_NONE && (h & 2u) != 0u);
    p.y = select(p.y, 1.0 - p.y, d4 && (h & 4u) != 0u);
    return p;
}

const BIOME_BAND: f32 = 0.22;
const BIOME_BLEND: f32 = 0.18;

const BIOME_TEMP_SEED: i32 = 7;
const BIOME_HUMID_SEED: i32 = 8;

const BIOME_PRIME_X: u32 = 501125321u;
const BIOME_PRIME_Y: u32 = 1136930381u;

const GRAD24_FIRST: f32 = 1.4398966328953218;
const GRAD24_STEP: f32 = 0.26179938779914946;
const GRAD8_FIRST: f32 = 1.1780972450961724;
const GRAD8_STEP: f32 = 0.7853981633974483;

fn simplex_grad(seed: u32, x_primed: u32, y_primed: u32, xd: f32, yd: f32) -> f32 {
    let h = (seed ^ x_primed ^ y_primed) * 0x27d4eb2du;
    let hs = bitcast<i32>(h);

    let idx = u32((hs ^ (hs >> 15u)) & 254i);
    var angle: f32;
    if idx < 240u {
        angle = GRAD24_FIRST - GRAD24_STEP * f32((idx % 48u) >> 1u);
    } else {
        angle = GRAD8_FIRST - GRAD8_STEP * f32((idx - 240u) >> 1u);
    }
    return xd * cos(angle) + yd * sin(angle);
}

// Exp 3: lazily-refined biome tint cache. 128x128 cells, 256-block period,
// zero-sentinel start; cells refine in place with the IDENTICAL expression the
// uncached path evaluates, so values (and pixels) are bit-exact. Races between
// threads write the same value.
override SPEC_BIOME_BAKE: bool = false;
@group(0) @binding(22) var<storage, read_write> biome_bake: array<vec3<f32>, 16384>;

fn biome_fast_floor(f: f32) -> i32 {
    if f >= 0.0 {
        return i32(f);
    }
    return i32(f) - 1;
}

fn biome_noise(seed_i: i32, freq: f32, p: vec2<f32>) -> f32 {
    let seed = bitcast<u32>(seed_i);
    let sqrt3 = 1.7320508075688772;
    let g2 = (3.0 - sqrt3) / 6.0;

    let f2 = 0.5 * (sqrt3 - 1.0);
    var xy = p * freq;
    let s = (xy.x + xy.y) * f2;
    xy = xy + vec2<f32>(s, s);

    let i0 = biome_fast_floor(xy.x);
    let j0 = biome_fast_floor(xy.y);
    let xi = xy.x - f32(i0);
    let yi = xy.y - f32(j0);

    let t = (xi + yi) * g2;
    let x0 = xi - t;
    let y0 = yi - t;

    let i = bitcast<u32>(i0) * BIOME_PRIME_X;
    let j = bitcast<u32>(j0) * BIOME_PRIME_Y;

    let a = 0.5 - x0 * x0 - y0 * y0;
    var n0 = 0.0;
    if a > 0.0 {
        n0 = (a * a) * (a * a) * simplex_grad(seed, i, j, x0, y0);
    }

    let c = (2.0 * (1.0 - 2.0 * g2) * (1.0 / g2 - 2.0)) * t
        + ((-2.0 * (1.0 - 2.0 * g2) * (1.0 - 2.0 * g2)) + a);
    var n2 = 0.0;
    if c > 0.0 {
        let x2 = x0 + (2.0 * g2 - 1.0);
        let y2 = y0 + (2.0 * g2 - 1.0);
        n2 = (c * c) * (c * c)
            * simplex_grad(seed, i + BIOME_PRIME_X, j + BIOME_PRIME_Y, x2, y2);
    }

    var n1 = 0.0;
    if y0 > x0 {
        let x1 = x0 + g2;
        let y1 = y0 + (g2 - 1.0);
        let b = 0.5 - x1 * x1 - y1 * y1;
        if b > 0.0 {
            n1 = (b * b) * (b * b) * simplex_grad(seed, i, j + BIOME_PRIME_Y, x1, y1);
        }
    } else {
        let x1 = x0 + (g2 - 1.0);
        let y1 = y0 + g2;
        let b = 0.5 - x1 * x1 - y1 * y1;
        if b > 0.0 {
            n1 = (b * b) * (b * b) * simplex_grad(seed, i + BIOME_PRIME_X, j, x1, y1);
        }
    }

    return (n0 + n1 + n2) * 99.83685446303647;
}

fn biome_band_weights(n: f32) -> vec3<f32> {
    let hot = smoothstep(BIOME_BAND - BIOME_BLEND, BIOME_BAND + BIOME_BLEND, n);
    let cold = 1.0 - smoothstep(-BIOME_BAND - BIOME_BLEND, -BIOME_BAND + BIOME_BLEND, n);
    return vec3<f32>(cold, 1.0 - cold - hot, hot);
}

const TINT_PLAINS:  vec3<f32> = vec3<f32>(1.000, 1.000, 1.000);

const TINT_PLAINS_BALANCED: vec3<f32> = vec3<f32>(0.935, 1.067, 0.892);
const TINT_FOREST:  vec3<f32> = vec3<f32>(0.664, 1.045, 1.000);
const TINT_SAVANNA: vec3<f32> = vec3<f32>(1.842, 0.935, 0.793);
const TINT_DESERT:  vec3<f32> = vec3<f32>(2.096, 0.957, 0.699);
const TINT_TAIGA:   vec3<f32> = vec3<f32>(0.832, 0.935, 2.096);
const TINT_TUNDRA:  vec3<f32> = vec3<f32>(0.755, 0.893, 2.812);

fn biome_tint(world_xz: vec2<f32>) -> vec3<f32> {
    let temperature = biome_noise(frame.tint_seed + BIOME_TEMP_SEED, frame.tint_freq_t, world_xz);
    let humidity = biome_noise(frame.tint_seed + BIOME_HUMID_SEED, frame.tint_freq_h, world_xz);
    let wt = biome_band_weights(temperature);
    let wh = biome_band_weights(humidity);

    let plains = select(TINT_PLAINS, TINT_PLAINS_BALANCED, SPEC_TINT_BALANCE);
    let dry = wt.x * TINT_TUNDRA + wt.y * plains + wt.z * TINT_DESERT;
    let mid = wt.x * TINT_TAIGA  + wt.y * plains + wt.z * TINT_SAVANNA;
    let wet = wt.x * TINT_TAIGA  + wt.y * TINT_FOREST + wt.z * TINT_FOREST;
    return wh.x * dry + wh.y * mid + wh.z * wet;
}

fn biome_tint_cached(xz: vec2<f32>) -> vec3<f32> {
    let q = vec2<i32>(floor(xz * (1.0f / 256.0))) & vec2<i32>(127);
    let idx = u32(q.y) * 128u + u32(q.x);
    let hit = biome_bake[idx];
    if !all(hit == vec3<f32>(0.0)) { return hit; }
    let v = biome_tint(xz);
    biome_bake[idx] = v;
    return v;
}

fn biome_tint_use(xz: vec2<f32>) -> vec3<f32> {
    if SPEC_BIOME_BAKE { return biome_tint_cached(xz); }
    return biome_tint(xz);
}

const SKY_SPEC_GAIN: f32 = 0.5;

fn shade_hit_legacy(ci: u32, v: vec3<u32>, id: u32, normal_id: u32, hit: vec3<f32>, rd: vec3<f32>, dist: f32,
             shadow_dist: f32, smooth_light: bool, do_spec: bool) -> vec3<f32> {
        let c = chunks[ci];
        let vs = c.voxel_size;

        let foliage = normal_id >= NORMAL_CROSS_A;
        var n = normal_of(normal_id);
        if foliage && dot(n, rd) > 0.0 {
            n = -n;
        }

        var spec_pre = vec3<f32>(0.0);
        if SPEC_SKY_SPECULAR && do_spec {
            spec_pre = sky_base(reflect(rd, n), 1.0)
                * schlick_ground(clamp(dot(n, -rd), 0.0, 1.0));
        }
        let axis = normal_id >> 1u;
        let box_min = c.origin + vec3<f32>(v) * vs;
        let local = clamp((hit - box_min) / vs, vec3<f32>(0.0), vec3<f32>(1.0));

        let face_word = block_faces[id * 8u + normal_id];
        let layer = face_layer(face_word);

        var uv = face_uv(normal_id, local);
        if SPEC_TEX_VARIATION && (frame.flags & FLAG_TEX_VARIATION) != 0u {
            uv = permute_uv(uv, face_perm(face_word), perm_key(box_min, normal_id));
        }
        var fa: f32;
        var fb: f32;
        var ta: vec3<i32>;
        var tb: vec3<i32>;
        if axis == 0u {
            fa = local.y; fb = local.z;
            ta = vec3<i32>(0, 1, 0); tb = vec3<i32>(0, 0, 1);
        } else if axis == 1u {
            fa = local.x; fb = local.z;
            ta = vec3<i32>(1, 0, 0); tb = vec3<i32>(0, 0, 1);
        } else {
            fa = local.x; fb = local.y;
            ta = vec3<i32>(1, 0, 0); tb = vec3<i32>(0, 1, 0);
        }

        let grazing = max(abs(dot(n, rd)), 0.25);
        let footprint = max(dist, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y) * 16.0 / vs / grazing;
        let lod = clamp(log2(max(footprint, 1e-4)) + frame.mip_bias, 0.0, 3.0);

        let texel = textureSampleLevel(atlas, atlas_samp, uv, i32(layer), lod);
        var albedo = texel.rgb;

        let tint_mask = select(texel.a, 1.0, foliage);
        if SPEC_TINT && (frame.flags & FLAG_TINT) != 0u && tint_mask > 0.0 {

            let k = tint_mask * frame.tint_strength;
            albedo = albedo * mix(vec3<f32>(1.0), biome_tint_use(hit.xz), k);
        }

        if SPEC_SHORE_WET && id == SAND_ID {
            let wet = 1.0 - smoothstep(frame.sea_level, frame.sea_level + WET_BAND, hit.y);
            albedo = albedo * mix(1.0, WET_DARK, wet);
        }

        if SPEC_FOLIAGE_RICH
            && (foliage || ((id == GRASS_ID || id == MEADOW_ID) && normal_id == NORMAL_PY))
        {
            let hr = hash_alpha(box_min);
            let hv = hash_alpha(box_min + vec3<f32>(17.31, 7.77, 3.03));
            var rich = vec3<f32>(1.0);
            if hr < 0.16 {

                rich = vec3<f32>(1.30, 1.10, 0.55);
            } else if hr > 0.97 {

                rich = vec3<f32>(1.35, 0.72, 0.95);
            }
            albedo = albedo * rich * mix(0.88, 1.10, hv);
        }

        if SPEC_CANOPY_RELIEF && is_cutout_id(id) {
            let clump = cloud_noise(box_min.xz * 0.21
                + vec2<f32>(box_min.y * 0.313, box_min.y * 0.173));
            let relief = mix(0.72, 1.34, clump * clump);
            let micro = mix(0.92, 1.08, hash_alpha(box_min));
            let toward_sun = clamp(dot(n, frame.sun_dir) * 0.5 + 0.5, 0.0, 1.0);
            albedo = albedo * relief * micro * mix(0.84, 1.0, toward_sun);
        }

        let sky_tint = surface_sky_fill(n);
        var ao: f32 = 1.0;
        var sky_sm: f32 = 0.0;
        var amb: vec3<f32> = vec3<f32>(0.0);
        if foliage {
            let packed = light_at(ci, vec3<i32>(v));
            let sk = light_curve(light_sky(packed)) * frame.daylight;
            sky_sm = sk;
            if !SPEC_PROBE_AMBIENT {
                if SPEC_LIGHT_RGB {
                    amb = block_ambient(light_curve3(light_block_rgb(packed)), sk, sky_tint);
                } else {
                    let bk = light_curve(light_block_level(packed));
                    amb = mix(sky_tint, BLOCK_TINT, bk / max(sk + bk, 1e-4)) * max(sk, bk);
                }
            }
        } else if !smooth_light {

            let air_v1 = vec3<i32>(v) + vec3<i32>(n);
            var packed1: u32;
            if all(air_v1 >= vec3<i32>(0)) && all(air_v1 <= vec3<i32>(63)) {
                packed1 = light_at(ci, air_v1);
            } else {
                packed1 = world_sample(box_min + vec3<f32>(0.5 * vs) + n * vs).x;
            }
            let sk1 = light_curve(light_sky(packed1)) * frame.daylight;
            sky_sm = sk1;
            if !SPEC_PROBE_AMBIENT {
                if SPEC_LIGHT_RGB {
                    amb = block_ambient(light_curve3(light_block_rgb(packed1)), sk1, sky_tint);
                } else {
                    let bk1 = light_curve(light_block_level(packed1));
                    amb = mix(sky_tint, BLOCK_TINT, bk1 / max(sk1 + bk1, 1e-4)) * max(sk1, bk1);
                }
            }

        } else {

            let air_c = box_min + vec3<f32>(0.5 * vs) + n * vs;
            let step_a = vec3<f32>(ta) * vs;
            let step_b = vec3<f32>(tb) * vs;

            let air_v = vec3<i32>(v) + vec3<i32>(n);
            let span = abs(ta) + abs(tb);
            let local_only = all(air_v - span >= vec3<i32>(0)) && all(air_v + span <= vec3<i32>(63));
            var nb: array<vec2<u32>, 9>;
            if local_only {
                gather_face(ci, air_v, ta, tb, &nb);
            } else {
                for (var i = 0u; i < 9u; i = i + 1u) {
                    nb[i] = world_sample(air_c + step_a * (f32(i % 3u) - 1.0) + step_b * (f32(i / 3u) - 1.0));
                }
            }
            var sky_n: array<f32, 9>;

            var blk_n: array<f32, 9>;
            var open_n: array<f32, 9>;

            var any_blk = 0u;
            for (var i = 0u; i < 9u; i = i + 1u) {
                sky_n[i] = light_curve(light_sky(nb[i].x)) * frame.daylight;

                if !SPEC_PROBE_AMBIENT {
                    if SPEC_LIGHT_RGB {

                        if SPEC_EMITTER_GATHER {
                            any_blk = any_blk | (nb[i].x & 0xFFFu);
                        }
                    } else {
                        blk_n[i] = light_curve(light_block_level(nb[i].x));
                    }
                }
                open_n[i] = 1.0 - f32(nb[i].y);
            }

            var blk4 = vec3<f32>(0.0);
            if SPEC_EMITTER_GATHER && SPEC_LIGHT_RGB && !SPEC_PROBE_AMBIENT && any_blk != 0u {
                blk4 = light_curve3(light_block_rgb(nb[4].x));
            }

            let ao_on = (frame.flags & FLAG_AO) != 0u;
            var amb_c: array<vec3<f32>, 4>;
            var sky_c: array<f32, 4>;
            var ao_c: array<f32, 4>;
            for (var k = 0u; k < 4u; k = k + 1u) {
                let ia = u32(4 + (i32(k & 1u) * 2 - 1));
                let ib = u32(4 + (i32(k >> 1u) * 2 - 1) * 3);
                let ic = ia + ib - 4u;

                let inv = 1.0 / (1.0 + open_n[ia] + open_n[ib] + open_n[ic]);
                let sk = (sky_n[4] + sky_n[ia] * open_n[ia] + sky_n[ib] * open_n[ib] + sky_n[ic] * open_n[ic]) * inv;
                sky_c[k] = sk;

                if !SPEC_PROBE_AMBIENT {

                    if SPEC_LIGHT_RGB {
                        if SPEC_EMITTER_GATHER && any_blk != 0u {
                            let ba = light_curve3(light_block_rgb(nb[ia].x));
                            let bb = light_curve3(light_block_rgb(nb[ib].x));
                            let bc = light_curve3(light_block_rgb(nb[ic].x));
                            let bkv = (blk4 + ba * open_n[ia] + bb * open_n[ib] + bc * open_n[ic]) * inv;
                            amb_c[k] = block_ambient(bkv, sk, sky_tint);
                        } else {

                            amb_c[k] = sky_tint * sk;
                        }
                    } else {
                        let bk = (blk_n[4] + blk_n[ia] * open_n[ia] + blk_n[ib] * open_n[ib] + blk_n[ic] * open_n[ic]) * inv;
                        amb_c[k] = mix(sky_tint, BLOCK_TINT, bk / max(sk + bk, 1e-4)) * max(sk, bk);
                    }
                }

                let s1 = 1.0 - open_n[ia];
                let s2 = 1.0 - open_n[ib];
                let occ = select(s1 + s2 + 1.0 - open_n[ic], 3.0, s1 > 0.5 && s2 > 0.5);
                ao_c[k] = select(1.0, 1.0 - occ * AO_STRENGTH, ao_on);
            }

            ao = mix(mix(ao_c[0], ao_c[1], fa), mix(ao_c[2], ao_c[3], fa), fb);
            sky_sm = mix(mix(sky_c[0], sky_c[1], fa), mix(sky_c[2], sky_c[3], fa), fb);
            if !SPEC_PROBE_AMBIENT {
                amb = mix(mix(amb_c[0], amb_c[1], fa), mix(amb_c[2], amb_c[3], fa), fb);
            }
        }

        if SPEC_PROBE_AMBIENT {

            amb = probe_tap(hit);
        } else if SPEC_PROBE_TAP {
            amb = amb * probe_tap(hit);
        }

        var visibility = 1.0;
        var direct = 0.0;
        let ndl = dot(n, frame.sun_dir);
        if ndl > 0.0 && frame.daylight > 0.01 {
            var lit = 1.0;
            if (frame.flags & FLAG_SHADOWS) != 0u {

                lit = select(1.0, 0.0, shadow_ray(hit + n * (0.02 * vs), frame.sun_dir, shadow_dist));

                if SPEC_DISTANT_SHADOWS && (frame.flags & FLAG_DISTANT_SHADOWS) != 0u && lit > 0.0 {
                    lit = lit * terrain_shade_beyond(hit, shadow_dist);
                }
            }

            if lit > 0.0 {
                let cloud_fp = max(dist, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y)
                    * exp2(frame.mip_bias);
                lit = lit * cloud_shade(hit, cloud_fp);
            }

            visibility = lit;
            direct = lit * ndl * sky_sm;

            if SPEC_CAUSTICS {
                let cdepth = frame.sea_level - hit.y;
                if cdepth > 0.0 && cdepth < CAUSTIC_DEPTH && direct > 0.0 {

                    let sp = hit.xz - frame.sun_dir.xz * (cdepth / max(frame.sun_dir.y, 1e-3));
                    let a = frame.wave_amp;
                    let l0 = frame.wave_scale;
                    let ph = frame.time * frame.wave_speed * WAVE_TAU;

                    let g0 = wave_octave(sp, WAVE_D0, l0, a * 0.42, ph, CAUSTIC_PX,
                                         vec2<f32>(0.0), 0.0, false);
                    let g1 = wave_octave(sp, WAVE_D1, l0 / 2.17, a * 0.27, ph * 1.4731,
                                         CAUSTIC_PX, vec2<f32>(0.0), 0.0, false);

                    let r0 = g0.x * g0.x + g0.y * g0.y;
                    let r1 = g1.x * g1.x + g1.y * g1.y;
                    let sheets = r0 * r1 * CAUSTIC_GAIN;
                    direct = direct * (1.0 + sheets * exp(-cdepth * CAUSTIC_FADE));
                }
            }
        }

        var probe = vec4<f32>(1.0);
        if !SPEC_PROBE_AMBIENT && (SPEC_PROBE_CUBE || SPEC_PROBE_BOUNCE) {
            probe = probe_field(hit + n * PROBE_NORMAL_BIAS, normal_id);
        }
        var shade = 1.0;
        if !SPEC_PROBE_AMBIENT {
            shade = select(face_shade(normal_id), 1.0, SPEC_LIGHTING_REPAIR);
            if SPEC_PROBE_CUBE {
                shade = shade * probe.a;
            }
        }
        let ambient_rgb = amb * 0.50;
        let sun_rgb = vec3<f32>(1.0000, 0.8469, 0.5647) * (direct * select(0.52, 0.78, SPEC_LIGHTING_REPAIR));

        var floor_rgb = vec3<f32>(0.0);
        if !SPEC_PROBE_AMBIENT {
            floor_rgb = vec3<f32>(0.85, 0.90, 1.00) * frame.ambient;
            if SPEC_PROBE_BOUNCE {
                floor_rgb = floor_rgb * probe.rgb;
            }

            if SPEC_PROBE_SUN {
                floor_rgb = floor_rgb
                    + vec3<f32>(0.85, 0.90, 1.00) * probe.rgb * (probe_sun_gain() * sky_sm);
            }
        }

        if SURFACE_DEBUG == 1u { return n * 0.5 + vec3<f32>(0.5); }
        if SURFACE_DEBUG == 2u { return vec3<f32>(visibility); }
        if SURFACE_DEBUG == 3u { return sun_rgb; }
        if SURFACE_DEBUG == 4u { return ambient_rgb * ao + floor_rgb; }
        let spec_rgb = spec_pre * (sky_sm * SKY_SPEC_GAIN);
        return surface_radiance(albedo, surface_light(ambient_rgb, sun_rgb, ao, floor_rgb, shade), spec_rgb, id);
}

fn shade_hit_compact(ci: u32, v: vec3<u32>, id: u32, normal_id: u32, hit: vec3<f32>, rd: vec3<f32>, dist: f32,
             shadow_dist: f32, smooth_light: bool, do_spec: bool) -> vec3<f32> {
        let c = chunks[ci];
        let vs = c.voxel_size;
        let foliage = normal_id >= NORMAL_CROSS_A;
        var n = normal_of(normal_id);
        if foliage && dot(n, rd) > 0.0 {
            n = -n;
        }
        var spec_pre = vec3<f32>(0.0);
        if SPEC_SKY_SPECULAR && do_spec {
            spec_pre = sky_base(reflect(rd, n), 1.0)
                * schlick_ground(clamp(dot(n, -rd), 0.0, 1.0));
        }
        let grazing = max(abs(dot(n, rd)), 0.25);
        let footprint = max(dist, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y) * 16.0 / vs / grazing;
        let lod = clamp(log2(max(footprint, 1e-4)) + frame.mip_bias, 0.0, 3.0);
        var light_rgb: vec3<f32>;
        var face_scale: f32;
        var spec_value: vec3<f32>;
        var material_ndl: f32;
        {
        let axis = normal_id >> 1u;
        let box_min = c.origin + vec3<f32>(v) * vs;
        let local = clamp((hit - box_min) / vs, vec3<f32>(0.0), vec3<f32>(1.0));
        var fa: f32;
        var fb: f32;
        var ta: vec3<i32>;
        var tb: vec3<i32>;
        if axis == 0u {
            fa = local.y; fb = local.z;
            ta = vec3<i32>(0, 1, 0); tb = vec3<i32>(0, 0, 1);
        } else if axis == 1u {
            fa = local.x; fb = local.z;
            ta = vec3<i32>(1, 0, 0); tb = vec3<i32>(0, 0, 1);
        } else {
            fa = local.x; fb = local.y;
            ta = vec3<i32>(1, 0, 0); tb = vec3<i32>(0, 1, 0);
        }
        let sky_tint = surface_sky_fill(n);
        var ao: f32 = 1.0;
        var sky_sm: f32 = 0.0;
        var amb: vec3<f32> = vec3<f32>(0.0);
        if foliage {
            let packed = light_at(ci, vec3<i32>(v));
            let sk = light_curve(light_sky(packed)) * frame.daylight;
            sky_sm = sk;
            if !SPEC_PROBE_AMBIENT {
                if SPEC_LIGHT_RGB {
                    amb = block_ambient(light_curve3(light_block_rgb(packed)), sk, sky_tint);
                } else {
                    let bk = light_curve(light_block_level(packed));
                    amb = mix(sky_tint, BLOCK_TINT, bk / max(sk + bk, 1e-4)) * max(sk, bk);
                }
            }
        } else if !smooth_light {
            let air_v1 = vec3<i32>(v) + vec3<i32>(n);
            var packed1: u32;
            if all(air_v1 >= vec3<i32>(0)) && all(air_v1 <= vec3<i32>(63)) {
                packed1 = light_at(ci, air_v1);
            } else {
                packed1 = world_sample(box_min + vec3<f32>(0.5 * vs) + n * vs).x;
            }
            let sk1 = light_curve(light_sky(packed1)) * frame.daylight;
            sky_sm = sk1;
            if !SPEC_PROBE_AMBIENT {
                if SPEC_LIGHT_RGB {
                    amb = block_ambient(light_curve3(light_block_rgb(packed1)), sk1, sky_tint);
                } else {
                    let bk1 = light_curve(light_block_level(packed1));
                    amb = mix(sky_tint, BLOCK_TINT, bk1 / max(sk1 + bk1, 1e-4)) * max(sk1, bk1);
                }
            }
        } else {
            let air_c = box_min + vec3<f32>(0.5 * vs) + n * vs;
            let step_a = vec3<f32>(ta) * vs;
            let step_b = vec3<f32>(tb) * vs;
            let air_v = vec3<i32>(v) + vec3<i32>(n);
            let span = abs(ta) + abs(tb);
            let local_only = all(air_v - span >= vec3<i32>(0)) && all(air_v + span <= vec3<i32>(63));
            var nb: array<vec2<u32>, 9>;
            if local_only {
                gather_face(ci, air_v, ta, tb, &nb);
            } else {
                for (var i = 0u; i < 9u; i = i + 1u) {
                    nb[i] = world_sample(air_c + step_a * (f32(i % 3u) - 1.0) + step_b * (f32(i / 3u) - 1.0));
                }
            }
            var sky_n: array<f32, 9>;
            var blk_n: array<f32, 9>;
            var open_n: array<f32, 9>;
            var any_blk = 0u;
            for (var i = 0u; i < 9u; i = i + 1u) {
                sky_n[i] = light_curve(light_sky(nb[i].x)) * frame.daylight;
                if !SPEC_PROBE_AMBIENT {
                    if SPEC_LIGHT_RGB {
                        if SPEC_EMITTER_GATHER {
                            any_blk = any_blk | (nb[i].x & 0xFFFu);
                        }
                    } else {
                        blk_n[i] = light_curve(light_block_level(nb[i].x));
                    }
                }
                open_n[i] = 1.0 - f32(nb[i].y);
            }
            var blk4 = vec3<f32>(0.0);
            if SPEC_EMITTER_GATHER && SPEC_LIGHT_RGB && !SPEC_PROBE_AMBIENT && any_blk != 0u {
                blk4 = light_curve3(light_block_rgb(nb[4].x));
            }
            let ao_on = (frame.flags & FLAG_AO) != 0u;
            var amb_c: array<vec3<f32>, 4>;
            var sky_c: array<f32, 4>;
            var ao_c: array<f32, 4>;
            for (var k = 0u; k < 4u; k = k + 1u) {
                let ia = u32(4 + (i32(k & 1u) * 2 - 1));
                let ib = u32(4 + (i32(k >> 1u) * 2 - 1) * 3);
                let ic = ia + ib - 4u;
                let inv = 1.0 / (1.0 + open_n[ia] + open_n[ib] + open_n[ic]);
                let sk = (sky_n[4] + sky_n[ia] * open_n[ia] + sky_n[ib] * open_n[ib] + sky_n[ic] * open_n[ic]) * inv;
                sky_c[k] = sk;
                if !SPEC_PROBE_AMBIENT {
                    if SPEC_LIGHT_RGB {
                        if SPEC_EMITTER_GATHER && any_blk != 0u {
                            let ba = light_curve3(light_block_rgb(nb[ia].x));
                            let bb = light_curve3(light_block_rgb(nb[ib].x));
                            let bc = light_curve3(light_block_rgb(nb[ic].x));
                            let bkv = (blk4 + ba * open_n[ia] + bb * open_n[ib] + bc * open_n[ic]) * inv;
                            amb_c[k] = block_ambient(bkv, sk, sky_tint);
                        } else {
                            amb_c[k] = sky_tint * sk;
                        }
                    } else {
                        let bk = (blk_n[4] + blk_n[ia] * open_n[ia] + blk_n[ib] * open_n[ib] + blk_n[ic] * open_n[ic]) * inv;
                        amb_c[k] = mix(sky_tint, BLOCK_TINT, bk / max(sk + bk, 1e-4)) * max(sk, bk);
                    }
                }
                let s1 = 1.0 - open_n[ia];
                let s2 = 1.0 - open_n[ib];
                let occ = select(s1 + s2 + 1.0 - open_n[ic], 3.0, s1 > 0.5 && s2 > 0.5);
                ao_c[k] = select(1.0, 1.0 - occ * AO_STRENGTH, ao_on);
            }
            ao = mix(mix(ao_c[0], ao_c[1], fa), mix(ao_c[2], ao_c[3], fa), fb);
            sky_sm = mix(mix(sky_c[0], sky_c[1], fa), mix(sky_c[2], sky_c[3], fa), fb);
            if !SPEC_PROBE_AMBIENT {
                amb = mix(mix(amb_c[0], amb_c[1], fa), mix(amb_c[2], amb_c[3], fa), fb);
            }
        }
        if SPEC_PROBE_AMBIENT {
            amb = probe_tap(hit);
        } else if SPEC_PROBE_TAP {
            amb = amb * probe_tap(hit);
        }
        var visibility = 1.0;
        var direct = 0.0;
        let ndl = dot(n, frame.sun_dir);
        if ndl > 0.0 && frame.daylight > 0.01 {
            var lit = 1.0;
            if (frame.flags & FLAG_SHADOWS) != 0u {
                lit = select(1.0, 0.0, shadow_ray(hit + n * (0.02 * vs), frame.sun_dir, shadow_dist));
                if SPEC_DISTANT_SHADOWS && (frame.flags & FLAG_DISTANT_SHADOWS) != 0u && lit > 0.0 {
                    lit = lit * terrain_shade_beyond(hit, shadow_dist);
                }
            }
            if lit > 0.0 {
                let cloud_fp = max(dist, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y)
                    * exp2(frame.mip_bias);
                lit = lit * cloud_shade(hit, cloud_fp);
            }
            visibility = lit;
            direct = lit * ndl * sky_sm;
            if SPEC_CAUSTICS {
                let cdepth = frame.sea_level - hit.y;
                if cdepth > 0.0 && cdepth < CAUSTIC_DEPTH && direct > 0.0 {
                    let sp = hit.xz - frame.sun_dir.xz * (cdepth / max(frame.sun_dir.y, 1e-3));
                    let a = frame.wave_amp;
                    let l0 = frame.wave_scale;
                    let ph = frame.time * frame.wave_speed * WAVE_TAU;
                    let g0 = wave_octave(sp, WAVE_D0, l0, a * 0.42, ph, CAUSTIC_PX,
                                         vec2<f32>(0.0), 0.0, false);
                    let g1 = wave_octave(sp, WAVE_D1, l0 / 2.17, a * 0.27, ph * 1.4731,
                                         CAUSTIC_PX, vec2<f32>(0.0), 0.0, false);
                    let r0 = g0.x * g0.x + g0.y * g0.y;
                    let r1 = g1.x * g1.x + g1.y * g1.y;
                    let sheets = r0 * r1 * CAUSTIC_GAIN;
                    direct = direct * (1.0 + sheets * exp(-cdepth * CAUSTIC_FADE));
                }
            }
        }
        var probe = vec4<f32>(1.0);
        if !SPEC_PROBE_AMBIENT && (SPEC_PROBE_CUBE || SPEC_PROBE_BOUNCE) {
            probe = probe_field(hit + n * PROBE_NORMAL_BIAS, normal_id);
        }
        var shade = 1.0;
        if !SPEC_PROBE_AMBIENT {
            shade = select(face_shade(normal_id), 1.0, SPEC_LIGHTING_REPAIR);
            if SPEC_PROBE_CUBE {
                shade = shade * probe.a;
            }
        }
        let ambient_rgb = amb * 0.50;
        let sun_rgb = vec3<f32>(1.0000, 0.8469, 0.5647) * (direct * select(0.52, 0.78, SPEC_LIGHTING_REPAIR));
        var floor_rgb = vec3<f32>(0.0);
        if !SPEC_PROBE_AMBIENT {
            floor_rgb = vec3<f32>(0.85, 0.90, 1.00) * frame.ambient;
            if SPEC_PROBE_BOUNCE {
                floor_rgb = floor_rgb * probe.rgb;
            }
            if SPEC_PROBE_SUN {
                floor_rgb = floor_rgb
                    + vec3<f32>(0.85, 0.90, 1.00) * probe.rgb * (probe_sun_gain() * sky_sm);
            }
        }
        if SURFACE_DEBUG == 1u { return n * 0.5 + vec3<f32>(0.5); }
        if SURFACE_DEBUG == 2u { return vec3<f32>(visibility); }
        if SURFACE_DEBUG == 3u { return sun_rgb; }
        if SURFACE_DEBUG == 4u { return ambient_rgb * ao + floor_rgb; }
        let spec_rgb = spec_pre * (sky_sm * SKY_SPEC_GAIN);
        light_rgb = surface_light(ambient_rgb, sun_rgb, ao, floor_rgb, shade);
        face_scale = 1.0;
        spec_value = spec_rgb;
        material_ndl = ndl;
        }
        let material_chunk = chunks[ci];
        let box_min = material_chunk.origin + vec3<f32>(v) * material_chunk.voxel_size;
        let local = clamp((hit - box_min) / material_chunk.voxel_size, vec3<f32>(0.0), vec3<f32>(1.0));
        let face_word = block_faces[id * 8u + normal_id];
        let layer = face_layer(face_word);
        var uv = face_uv(normal_id, local);
        if SPEC_TEX_VARIATION && (frame.flags & FLAG_TEX_VARIATION) != 0u {
            uv = permute_uv(uv, face_perm(face_word), perm_key(box_min, normal_id));
        }
        let texel = textureSampleLevel(atlas, atlas_samp, uv, i32(layer), lod);
        var albedo = texel.rgb;
        let tint_mask = select(texel.a, 1.0, foliage);
        if SPEC_TINT && (frame.flags & FLAG_TINT) != 0u && tint_mask > 0.0 {
            let k = tint_mask * frame.tint_strength;
            albedo = albedo * mix(vec3<f32>(1.0), biome_tint_use(hit.xz), k);
        }
        if SPEC_SHORE_WET && id == SAND_ID {
            let wet = 1.0 - smoothstep(frame.sea_level, frame.sea_level + WET_BAND, hit.y);
            albedo = albedo * mix(1.0, WET_DARK, wet);
        }
        if SPEC_FOLIAGE_RICH
            && (foliage || ((id == GRASS_ID || id == MEADOW_ID) && normal_id == NORMAL_PY))
        {
            let hr = hash_alpha(box_min);
            let hv = hash_alpha(box_min + vec3<f32>(17.31, 7.77, 3.03));
            var rich = vec3<f32>(1.0);
            if hr < 0.16 {
                rich = vec3<f32>(1.30, 1.10, 0.55);
            } else if hr > 0.97 {
                rich = vec3<f32>(1.35, 0.72, 0.95);
            }
            albedo = albedo * rich * mix(0.88, 1.10, hv);
        }
        if SPEC_CANOPY_RELIEF && is_cutout_id(id) {
            let clump = cloud_noise(box_min.xz * 0.21
                + vec2<f32>(box_min.y * 0.313, box_min.y * 0.173));
            let relief = mix(0.72, 1.34, clump * clump);
            let micro = mix(0.92, 1.08, hash_alpha(box_min));
            let toward_sun = clamp(material_ndl * 0.5 + 0.5, 0.0, 1.0);
            albedo = albedo * relief * micro * mix(0.84, 1.0, toward_sun);
        }
        return surface_radiance(albedo, light_rgb * face_scale, spec_value, id);
}

fn shade_hit(ci: u32, v: vec3<u32>, id: u32, normal_id: u32, hit: vec3<f32>, rd: vec3<f32>, dist: f32,
             shadow_dist: f32, smooth_light: bool, do_spec: bool) -> vec3<f32> {
    if SPEC_COMPACT_SHADE_HIT {
        return shade_hit_compact(ci, v, id, normal_id, hit, rd, dist, shadow_dist, smooth_light, do_spec);
    }
    return shade_hit_legacy(ci, v, id, normal_id, hit, rd, dist, shadow_dist, smooth_light, do_spec);
}

const PROBE_SUN_GAIN: f32 = 0.10;

const PROBE_SUN_GAIN_HIGH: f32 = 0.25;

fn probe_sun_gain() -> f32 {
    var g = PROBE_SUN_GAIN;
    if SPEC_PROBE_SUN_HIGH {
        g = PROBE_SUN_GAIN_HIGH;
    }
    return g;
}

const WATER_REFRACT_NEAR: f32 = 96.0;
const WATER_REFRACT_FAR: f32 = 192.0;

const WATER_REFLECT_NEAR: f32 = 512.0;
const WATER_REFLECT_FAR: f32 = 1024.0;

const WATER_REFRACT_DIST: f32 = 96.0;
const WATER_REFLECT_DIST: f32 = 768.0;

fn secondary_smooth() -> bool {
    return !SPEC_FLAT_SECONDARY;
}

const GLASS_TRANSMIT_DIST: f32 = 192.0;

const GLASS_SHADOW_DIST: f32 = 48.0;

const GLINT_POWER: f32 = 320.0;
const GLINT_GAIN: f32 = 2.5;
const SUN_TINT: vec3<f32> = vec3<f32>(1.0000, 0.8469, 0.5647);

const WATER_BODY: vec3<f32> = vec3<f32>(0.0100, 0.0947, 0.1474);

fn medium_light(packed: u32) -> f32 {
    let sky_l = light_curve(light_sky(packed)) * frame.daylight;
    let blk_l = light_curve(light_block_level(packed));
    return max(sky_l, blk_l) * 0.50 + frame.ambient;
}

const WAVE_TAU: f32 = 6.28318531;

const WAVE_GRAZE_MIN: f32 = 1e-3;

const SHOAL_DEPTH: f32 = 6.0;

const SHOAL_MAX: f32 = SHOAL_DEPTH;

const SHOAL_DIR: vec3<f32> = vec3<f32>(0.001, -0.999999, 0.001);

const WAVE_FILTER_K: f32 = 1.0;

const WAVE_D0: vec2<f32> = vec2<f32>( 0.913089,  0.407760);
const WAVE_D1: vec2<f32> = vec2<f32>(-0.388685,  0.921371);
const WAVE_D2: vec2<f32> = vec2<f32>(-0.980382, -0.197108);
const WAVE_D3: vec2<f32> = vec2<f32>( 0.477328, -0.878725);

const WAVE_F0: vec2<f32> = vec2<f32>(-0.438371,  0.898794);
const WAVE_F1: vec2<f32> = vec2<f32>(-0.927184,  0.374607);
const WAVE_F2: vec2<f32> = vec2<f32>(-0.190809,  0.981627);
const WAVE_F3: vec2<f32> = vec2<f32>(-0.669131,  0.743145);
const WAVE_F4: vec2<f32> = vec2<f32>( 0.121869,  0.992546);
const WAVE_F5: vec2<f32> = vec2<f32>( 0.694658,  0.719340);
const WAVE_F6: vec2<f32> = vec2<f32>( 0.970296,  0.241922);
const WAVE_F7: vec2<f32> = vec2<f32>( 0.438371,  0.898794);

fn wave_octave(p: vec2<f32>, dir: vec2<f32>, lambda: f32, slope: f32, phase: f32, px: f32,
               vh: vec2<f32>, kk: f32, aniso: bool) -> vec3<f32> {
    var fade = 0.0;
    if aniso {

        let d = dot(dir, vh);
        let ext = px * sqrt(1.0 + kk * d * d);

        let u = ext / lambda;
        fade = exp(-WAVE_FILTER_K * u * u);
    } else {

        fade = smoothstep(2.0, 4.0, lambda / max(px, 1e-4));
    }
    let g = dir * (slope * fade * cos(WAVE_TAU / lambda * dot(p, dir) + phase));

    return vec3<f32>(g, 0.5 * slope * slope * (1.0 - fade * fade));
}

fn wave_field(p: vec2<f32>, px: f32, vh: vec2<f32>, kk: f32, aniso: bool, shoal: f32) -> vec3<f32> {
    let l0 = frame.wave_scale;

    let a = frame.wave_amp * shoal;
    let ph = frame.time * frame.wave_speed * WAVE_TAU;

    if SPEC_WAVE_FILL && (frame.flags & FLAG_WAVE_FILL) != 0u {
        return wave_octave(p, WAVE_F0, l0          , a * 0.3086, ph * 1.0000, px, vh, kk, aniso)
             + wave_octave(p, WAVE_F1, l0 /  1.3934, a * 0.2584, ph * 1.1804, px, vh, kk, aniso)
             + wave_octave(p, WAVE_F2, l0 /  1.9417, a * 0.2163, ph * 1.3934, px, vh, kk, aniso)
             + wave_octave(p, WAVE_F3, l0 /  2.7056, a * 0.1810, ph * 1.6449, px, vh, kk, aniso)
             + wave_octave(p, WAVE_F4, l0 /  3.7700, a * 0.1515, ph * 1.9417, px, vh, kk, aniso)
             + wave_octave(p, WAVE_F5, l0 /  5.2533, a * 0.1269, ph * 2.2920, px, vh, kk, aniso)
             + wave_octave(p, WAVE_F6, l0 /  7.3201, a * 0.1062, ph * 2.7056, px, vh, kk, aniso)
             + wave_octave(p, WAVE_F7, l0 / 10.2000, a * 0.0889, ph * 3.1937, px, vh, kk, aniso);
    }
    return wave_octave(p, WAVE_D0, l0,         a * 0.42, ph * 1.0000, px, vh, kk, aniso)
         + wave_octave(p, WAVE_D1, l0 /  2.17, a * 0.27, ph * 1.4731, px, vh, kk, aniso)
         + wave_octave(p, WAVE_D2, l0 /  4.71, a * 0.19, ph * 2.1703, px, vh, kk, aniso)
         + wave_octave(p, WAVE_D3, l0 / 10.20, a * 0.12, ph * 3.1937, px, vh, kk, aniso);
}

fn wave_normal(grad: vec2<f32>) -> vec3<f32> {
    return normalize(vec3<f32>(-grad.x, 1.0, -grad.y));
}

const WET_BAND: f32 = 1.5;
const WET_DARK: f32 = 0.62;
const FOAM_DEPTH: f32 = 2.5;

const CAUSTIC_DEPTH: f32 = 8.0;
const CAUSTIC_FADE: f32 = 0.30;
const CAUSTIC_PX: f32 = 0.35;

const CAUSTIC_GAIN: f32 = 4000.0;
const FOAM_LO: f32 = 0.45;
const FOAM_HI: f32 = 0.75;
const FOAM_MAX: f32 = 0.85;
const FOAM_RATE: f32 = 0.55;

const FOAM_RGB: vec3<f32> = vec3<f32>(0.92, 0.97, 1.0);

const WATER_LOOK_MURK: vec3<f32> = vec3<f32>(0.82, 0.90, 0.70);
const WATER_LOOK_MURK_DIST: f32 = 7.0;
const WATER_LOOK_SHALLOW: vec3<f32> = vec3<f32>(0.55, 0.74, 0.68);
const WATER_LOOK_SHORE_DEPTH: f32 = 2.0;
const WATER_LOOK_RIPPLE_AMP: f32 = 0.16;

fn look_ripple(p: vec2<f32>, time: f32, w1: f32, w2: f32) -> vec2<f32> {
    let d1 = vec2<f32>(0.86, 0.51);
    let d2 = vec2<f32>(-0.42, 0.91);
    let k1 = 6.2832 / 1.15;
    let k2 = 6.2832 / 0.62;
    var phase = vec2<f32>(0.0);
    if SPEC_WATER_REPAIR {

        phase = vec2<f32>(sin(dot(p, vec2<f32>(0.071, 0.113))),
                          sin(dot(p, vec2<f32>(-0.097, 0.053)) + 1.9)) * 2.0;
    }
    let g1 = d1 * (cos(dot(p, d1) * k1 + time * 1.7 + phase.x) * w1);
    let g2 = d2 * (cos(dot(p, d2) * k2 - time * 2.3 + phase.y) * w2);
    return g1 + g2;
}

fn foam_hash(i: vec2<f32>) -> f32 {
    let q = vec2<i32>(i);
    return f32((hash_u32(u32(q.x) * 374761393u + u32(q.y) * 668265263u) >> 8u) & 0xFFFFFFu) / 16777216.0;
}

fn foam_mottle(xz: vec2<f32>, time: f32) -> f32 {
    let p = xz * (1.0 / 0.7);
    let i = floor(p);
    let w = fract(p);
    let u = w * w * (3.0 - 2.0 * w);
    let m0_ = mix(foam_hash(i), foam_hash(i + vec2<f32>(1.0, 0.0)), u.x);
    let m1_ = mix(foam_hash(i + vec2<f32>(0.0, 1.0)), foam_hash(i + vec2<f32>(1.0, 1.0)), u.x);
    let m = mix(m0_, m1_, u.y);

    let ph = time * FOAM_RATE * 4.0 + foam_hash(i * vec2<f32>(2.23, 2.71)) * 6.28318;
    return m * (0.5 + 0.5 * sin(ph));
}

fn water_refract_leg(hit: vec3<f32>, n: vec3<f32>, rd: vec3<f32>, t: f32) -> vec4<f32> {
    let rdir = refract(rd, n, 1.0 / WATER_IOR);
    let ro2 = hit + rdir * SURFACE_EPS;
    let h2 = trace_world(ro2, rdir, WATER_REFRACT_DIST, true);
    var under = vec3<f32>(0.0);
    var depth = WATER_REFRACT_DIST;
    if h2.hit {
        let p2 = ro2 + rdir * h2.t;
        under = shade_hit(h2.ci, h2.voxel, block_at(h2.ci, h2.voxel), h2.normal, p2, rdir, t + h2.t,
                          SPEC_WATER_SHADOW_DIST, secondary_smooth(), false);
        depth = h2.t;
    }
    return vec4<f32>(under, depth);
}

fn water_reflect_leg(hit: vec3<f32>, n: vec3<f32>, mirror_t: vec3<f32>, t: f32) -> vec4<f32> {

    let ro3 = hit + n * SURFACE_EPS;
    let h3 = trace_world(ro3, mirror_t, WATER_REFLECT_DIST, true);
    if !h3.hit {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let p3 = ro3 + mirror_t * h3.t;

    var lit = shade_hit(h3.ci, h3.voxel, block_at(h3.ci, h3.voxel), h3.normal, p3, mirror_t, t + h3.t,
                        frame.shadow_dist, secondary_smooth(), false);

    lit = mix(lit, sky_base(mirror_t, 1.0), 1.0 - transmittance_from(hit.y, h3.t, mirror_t));
    return vec4<f32>(lit, 1.0);
}

struct WaterCtx {
    body: vec3<f32>,
    ml: f32,
    n: vec3<f32>,
    nw: vec3<f32>,
    nt: vec3<f32>,
    gloss: f32,
    waved: bool,
    refract_detail: f32,
    reflect_detail: f32,
    mirror: vec3<f32>,
    mirror_t: vec3<f32>,
    hit: vec3<f32>,
    rd: vec3<f32>,
    t: f32,
    roughness: f32,
}

fn water_ctx(ci: u32, v: vec3<u32>, normal_id: u32, hit: vec3<f32>, rd: vec3<f32>, t: f32) -> WaterCtx {
    let c = chunks[ci];
    let vs = c.voxel_size;
    let n = normal_of(normal_id);
    let box_min = c.origin + vec3<f32>(v) * vs;
    let local = clamp((hit - box_min) / vs, vec3<f32>(0.0), vec3<f32>(1.0));

    let footprint = max(t, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y) * 16.0 / vs;
    let lod = clamp(log2(max(footprint, 1e-4)) + frame.mip_bias, 0.0, 3.0);

    let layer = face_layer(block_faces[WATER_ID * 8u + normal_id]);
    var tint = textureSampleLevel(atlas, atlas_samp, face_uv(normal_id, local), i32(layer), lod).rgb;
    if SPEC_WATER_REPAIR {

        tint = textureSampleLevel(atlas, atlas_samp, vec2<f32>(0.5), i32(layer), 3.0).rgb;
    }

    let ml = medium_light(light_at(ci, vec3<i32>(v)));
    let body = tint * ml;

    let refract_detail = 1.0 - smoothstep(WATER_REFRACT_NEAR, WATER_REFRACT_FAR, t);
    let reflect_detail = 1.0 - smoothstep(WATER_REFLECT_NEAR, WATER_REFLECT_FAR, t);

    let px_world = max(t, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y);
    var nw = n;
    var nt = n;

    var gloss = 1.0;
    var unresolved = 0.0;
    let waved = SPEC_WAVES && (frame.flags & FLAG_WAVES) != 0u && normal_id == NORMAL_PY;
    if waved {

        var kk = 0.0;
        var vh = vec2<f32>(0.0, 0.0);
        let aniso = SPEC_WAVE_ANISO && (frame.flags & FLAG_WAVE_ANISO) != 0u;
        if aniso {
            let k = 1.0 / max(abs(rd.y), WAVE_GRAZE_MIN);
            kk = k * k - 1.0;
            vh = rd.xz * inverseSqrt(max(dot(rd.xz, rd.xz), 1e-12));
        }

        var shoal = 1.0;
        if SPEC_WAVE_SHOAL && (frame.flags & FLAG_WAVE_SHOAL) != 0u {
            let dro = vec3<f32>(floor(hit.x) + 0.5, hit.y - SURFACE_EPS, floor(hit.z) + 0.5);
            let dh = trace_world(dro, SHOAL_DIR, SHOAL_MAX, true);
            var depth = SHOAL_MAX;
            if dh.hit {
                depth = dh.t;
            }
            shoal = smoothstep(0.0, SHOAL_DEPTH, depth);
        }
        let w = wave_field(hit.xz, px_world, vh, kk, aniso, shoal);
        unresolved = max(w.z,0.0);
        nw = wave_normal(w.xy);

        nt = wave_normal(w.xy * min(1.0, frame.wave_reflect_slope / max(length(w.xy), 1e-6)));

        gloss = 1.0 / (1.0 + 0.5 * GLINT_POWER * w.z);
    }

    if SPEC_WATER_LOOK && normal_id == NORMAL_PY {

        let pix = px_world / max(abs(rd.y), WAVE_GRAZE_MIN);
        let range_gate = (1.0-smoothstep(48.0,96.0,t))*smoothstep(0.08,0.20,abs(rd.y));
        let fade_115 = range_gate*smoothstep(2.0, 4.0, 1.15 / max(pix, 1e-4));
        let fade_62 = range_gate*smoothstep(2.0, 4.0, 0.62 / max(pix, 1e-4));
        unresolved += WATER_LOOK_RIPPLE_AMP*WATER_LOOK_RIPPLE_AMP*0.5*
            (0.36*(1.0-fade_115*fade_115)+0.16*(1.0-fade_62*fade_62));
        let rp = look_ripple(
            hit.xz,
            frame.time * frame.wave_speed,
            0.60 * fade_115,
            0.40 * fade_62,
        );
        nw = normalize(nw + vec3<f32>(rp.x, 0.0, rp.y) * WATER_LOOK_RIPPLE_AMP);
    }

    let optical_roughness=clamp(1.0-exp(-8.0*unresolved),0.0,1.0);
    nw=normalize(mix(nw,n,optical_roughness));
    nt=normalize(mix(nt,n,optical_roughness));

    let mirror = reflect(rd, nw);
    var mirror_t = reflect(rd, nt);
    if waved {
        mirror_t = normalize(vec3<f32>(mirror_t.x, max(mirror_t.y, 0.0), mirror_t.z));
    }

    return WaterCtx(body, ml, n, nw, nt, gloss, waved, refract_detail, reflect_detail,
                    mirror, mirror_t, hit, rd, t, optical_roughness);
}

fn water_compose(ctx: WaterCtx, leg_r: vec4<f32>, leg_m: vec4<f32>) -> vec3<f32> {

    var refr = ctx.body;
    var column_depth = WATER_REFRACT_DIST;
    if (frame.flags & FLAG_WATER_REFRACT) != 0u && ctx.refract_detail > 0.0 {
        refr = mix(refr, water_medium(leg_r.rgb, leg_r.a, ctx.body), ctx.refract_detail);
        column_depth = leg_r.a;
    }

    if SPEC_WATER_LOOK && ctx.refract_detail > 0.0 {
        let depth_f = clamp(column_depth / WATER_LOOK_MURK_DIST, 0.0, 1.0);
        refr = refr * mix(vec3<f32>(1.0), WATER_LOOK_MURK, depth_f);
        let shall = 1.0 - smoothstep(0.0, WATER_LOOK_SHORE_DEPTH, column_depth);
        refr = mix(refr, WATER_LOOK_SHALLOW * ctx.ml, shall * 0.45 * ctx.refract_detail);
    }

    if SPEC_SHORE_FOAM && ctx.refract_detail > 0.0 && column_depth < FOAM_DEPTH {
        let shall = 1.0 - smoothstep(0.0, FOAM_DEPTH, column_depth);

        let m = foam_mottle(ctx.hit.xz, frame.time * frame.wave_speed);
        let foam = shall * smoothstep(FOAM_LO, FOAM_HI, m) * ctx.refract_detail;
        refr = mix(refr, FOAM_RGB * ctx.ml, clamp(foam, 0.0, 1.0) * FOAM_MAX);
    }

    var refl = sky_color(ctx.hit, ctx.mirror, ctx.t, 1.0)
        + SUN_TINT * (pow(max(dot(ctx.mirror, frame.sun_dir), 0.0), GLINT_POWER * ctx.gloss)
                      * GLINT_GAIN * ctx.gloss * frame.daylight);
    if (frame.flags & FLAG_WATER_REFLECT) != 0u && ctx.reflect_detail > 0.0 && leg_m.a > 0.0 {
        refl = mix(refl, leg_m.rgb, ctx.reflect_detail);
    }

    if SURFACE_DEBUG == 5u { return ctx.n * 0.5 + vec3<f32>(0.5); }
    if SURFACE_DEBUG == 6u { return refl; }
    if SURFACE_DEBUG == 7u { return refr; }

    refl=mix(refl,sky_base(ctx.n,1.0),ctx.roughness*0.65);

    return mix(refr, refl, schlick(max(-dot(ctx.rd, ctx.nw), 1e-3)));
}

fn shade_water(ci: u32, v: vec3<u32>, normal_id: u32, hit: vec3<f32>, rd: vec3<f32>, t: f32) -> vec3<f32> {
    let ctx = water_ctx(ci, v, normal_id, hit, rd, t);

    var leg_r = vec4<f32>(0.0, 0.0, 0.0, WATER_REFRACT_DIST);
    if (frame.flags & FLAG_WATER_REFRACT) != 0u && ctx.refract_detail > 0.0 {
        leg_r = water_refract_leg(hit, ctx.n, rd, t);
    }
    var leg_m = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    if (frame.flags & FLAG_WATER_REFLECT) != 0u && ctx.reflect_detail > 0.0 {
        leg_m = water_reflect_leg(hit, ctx.n, ctx.mirror_t, t);
    }
    return water_compose(ctx, leg_r, leg_m);
}

fn water_share_safe(px: u32, py: u32, normal_id: u32, hit: vec3<f32>, t: f32) -> bool {
    if normal_id != NORMAL_PY { return false; }
    let span = 1u << WATER_SEC_SHIFT;
    let base = vec2<u32>((px >> WATER_SEC_SHIFT) * span, (py >> WATER_SEC_SHIFT) * span);
    for (var y=0u; y<span; y++) { for (var x=0u; x<span; x++) {
        let p=base+vec2<u32>(x,y);
        if any(p >= frame.res) { continue; }
        let key=atomicLoad(&vis[p.y*frame.res.x+p.x]);
        if key==0lu { return false; }
        let nid=u32(key>>37u)&7u;
        if nid!=normal_id { return false; }
        let ci=u32(key>>24u)&0x1FFFu;
        let low=u32(key&0xFFFFFFlu);
        let v=vec3<u32>((low>>16u)&VOXEL_MASK,(low>>8u)&VOXEL_MASK,low&VOXEL_MASK);
        if block_at(ci,v)!=WATER_ID { return false; }
        let micro=vec3<u32>((low>>22u)&3u,(low>>14u)&3u,(low>>6u)&3u);
        let rd=ray_dir(f32(p.x)+0.5,f32(p.y)+0.5);
        let td=hit_t(ci,v,micro,nid,rd);
        let hd=frame.cam_pos+rd*td;
        if abs(hd.y-hit.y)>0.01 || abs(td-t)>max(0.1,min(1.0,t*0.02)) { return false; }
    }}
    return true;
}

fn shade_water_sec(ci: u32, v: vec3<u32>, normal_id: u32, hit: vec3<f32>, rd: vec3<f32>, t: f32,
                   px: u32, py: u32) -> vec3<f32> {
    if SPEC_WATER_REPAIR && !water_share_safe(px, py, normal_id, hit, t) {
        return shade_water(ci, v, normal_id, hit, rd, t);
    }
    let ctx = water_ctx(ci, v, normal_id, hit, rd, t);
    var leg_r = vec4<f32>(0.0, 0.0, 0.0, WATER_REFRACT_DIST);
    var leg_m = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    let hr = vec2<i32>(i32(px >> WATER_SEC_SHIFT), i32(py >> WATER_SEC_SHIFT));
    leg_r = textureLoad(water_refr_sample, hr, 0);
    leg_m = textureLoad(water_refl_sample, hr, 0);
    return water_compose(ctx, leg_r, leg_m);
}

// Exp 5 (glass SSR): screen-space substitute for the glass trace_world
// legs. The primary visibility buffer already carries (ci, v, micro, normal)
// per pixel; marching the reflect/transmit rays against it keeps the glass
// legs O(steps) instead of a whole-scene DDA per leg. Off-screen rays, misses
// and > SSR_MAX_DIST falls back to the exact sky fallback shade the DDA path
// used. CHANGE class: leg radiance legitimately differs from the full march.
override SPEC_GLASS_SSR: bool = false;
const SSR_STEPS: u32 = 24u;
const SSR_MAX_DIST: f32 = 192.0;

struct SsrHit {
    ok: bool,
    ci: u32,
    v: vec3<u32>,
    normal: u32,
    t: f32,
};

fn ssr_seed(hit: vec3<f32>) -> u32 {
    return u32(abs(hit.x * 57.0 + hit.z * 127.0 + hit.y * 31.0)) + 61u;
}

fn ssr_project(w: vec3<f32>) -> vec3<f32> { // (px, py, view_z)
    let d = w - frame.cam_pos;
    let z = dot(d, frame.cam_fwd);
    let x = dot(d, frame.cam_right);
    let y = dot(d, frame.cam_up);
    let ndc_x = x / max(z * frame.tan_half_fov * frame.aspect, 1e-4);
    let ndc_y = y / max(z * frame.tan_half_fov, 1e-4);
    return vec3<f32>((ndc_x * 0.5 + 0.5) * f32(frame.res.x), (0.5 - ndc_y * 0.5) * f32(frame.res.y), z);
}

fn ssr_trace(origin: vec3<f32>, rdir: vec3<f32>, seed: u32) -> SsrHit {
    var none: SsrHit;
    let j = f32(hash_u32(seed) & 0xFFFFu) * (1.0f / 65536.0);
    let px_scale = max(frame.tan_half_fov * 2.0 / f32(frame.res.y), 1e-4);
    var reach = 0.75 + j * 0.5;
    for (var i = 0u; i < SSR_STEPS; i = i + 1u) {
        if reach > SSR_MAX_DIST { break; }
        let w = origin + rdir * reach;
        let pr = ssr_project(w);
        if pr.z <= 0.0 || pr.x < 0.0 || pr.y < 0.0 || pr.x >= f32(frame.res.x) || pr.y >= f32(frame.res.y) { break; }
        let spx = u32(pr.x);
        let spy = u32(pr.y);
        let key = atomicLoad(&vis[spy * frame.res.x + spx]);
        if key != 0lu {
            let ci = u32(key >> 24u) & 0x1FFFu;
            let low = u32(key & 0xFFFFFFlu);
            let v = vec3<u32>((low >> 16u) & VOXEL_MASK,(low >> 8u) & VOXEL_MASK, low & VOXEL_MASK);
            let micro = vec3<u32>((low >> 22u) & 3u, (low >> 14u) & 3u, (low >> 6u) & 3u);
            let nid = u32(key >> 37u) & 7u;
            let rd_s = ray_dir(f32(spx) + 0.5, f32(spy) + 0.5);
            let ts = hit_t(ci, v, micro, nid, rd_s);
            if abs(ts - reach) <= max(0.5, reach * 0.02) {
                var out: SsrHit;
                out.ok = true;
                out.ci = ci;
                out.v = v;
                out.normal = nid;
                out.t = reach;
                return out;
            }
        }
        reach = reach * 1.12 + max(pr.z * px_scale * 0.5, 0.25);
    }
    return none;
}

fn shade_glass(ci: u32, v: vec3<u32>, normal_id: u32, hit: vec3<f32>, rd: vec3<f32>, t: f32) -> vec3<f32> {
    let c = chunks[ci];
    let vs = c.voxel_size;
    var n = normal_of(normal_id);
    if dot(n,rd)>0.0 { n=-n; }
    let box_min = c.origin + vec3<f32>(v) * vs;
    let local = clamp((hit - box_min) / vs, vec3<f32>(0.0), vec3<f32>(1.0));

    let footprint = max(t, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y) * 16.0 / vs;
    let lod = clamp(log2(max(footprint, 1e-4)) + frame.mip_bias, 0.0, 3.0);

    let layer = face_layer(block_faces[GLASS_ID * 8u + normal_id]);
    let texel = textureSampleLevel(atlas, atlas_samp, face_uv(normal_id, local), i32(layer), lod);

    let tint = texel.rgb;

    let rdir = rd;
    let ro2 = hit + rdir * SURFACE_EPS;

    var through = sky_color(ro2, rdir, t, 1.0);
    if SPEC_GLASS_SSR {
        let ssr_t = ssr_trace(ro2, rdir, ssr_seed(hit));
        if ssr_t.ok {
            let p2 = ro2 + rdir * ssr_t.t;
            let id2 = block_at(ssr_t.ci, ssr_t.v);
            if id2 == WATER_ID {
                through = shade_water(ssr_t.ci, ssr_t.v, ssr_t.normal, p2, rdir, t + ssr_t.t);
            } else {
                through = shade_hit(ssr_t.ci, ssr_t.v, id2, ssr_t.normal, p2, rdir,
                                    t + ssr_t.t, GLASS_SHADOW_DIST, secondary_smooth(), false);
            }
            through = mix(through, sky_base(rdir, 1.0), 1.0 - transmittance_from(hit.y, ssr_t.t, rdir));
        }
    } else {
        let h2 = trace_world(ro2, rdir, max(GLASS_TRANSMIT_DIST,frame.far), false);
        if h2.hit {
            let p2 = ro2 + rdir * h2.t;
            let id2 = block_at(h2.ci, h2.voxel);
            if id2 == WATER_ID {
                through = shade_water(h2.ci, h2.voxel, h2.normal, p2, rdir, t + h2.t);
            } else {
                through = shade_hit(h2.ci, h2.voxel, id2, h2.normal, p2, rdir,
                                    t + h2.t, GLASS_SHADOW_DIST, secondary_smooth(), false);
            }
            through = mix(through, sky_base(rdir, 1.0), 1.0 - transmittance_from(hit.y, h2.t, rdir));
        }
    }

    let mirror = reflect(rd, n);
    var refl = sky_color(hit, mirror, t, 1.0);

    if SPEC_GLASS_REFLECT {
        let origin=hit+n*SURFACE_EPS;
        if SPEC_GLASS_SSR {
            let ssr_r = ssr_trace(origin, mirror, ssr_seed(hit) ^ 57u);
            if ssr_r.ok {
                let point=origin+mirror*ssr_r.t;
                let material=block_at(ssr_r.ci,ssr_r.v);
                if material==WATER_ID {
                    refl=shade_water(ssr_r.ci,ssr_r.v,ssr_r.normal,point,mirror,t+ssr_r.t);
                } else {
                    refl=shade_hit(ssr_r.ci,ssr_r.v,material,ssr_r.normal,point,mirror,
                        t+ssr_r.t,GLASS_SHADOW_DIST,secondary_smooth(),false);
                }
                refl=mix(refl,sky_base(mirror,1.0),1.0-transmittance_from(hit.y,ssr_r.t,mirror));
            }
        } else {
            let reflected=trace_world(origin,mirror,max(GLASS_TRANSMIT_DIST,frame.far),false);
            if reflected.hit {
                let point=origin+mirror*reflected.t;
                let material=block_at(reflected.ci,reflected.voxel);
                if material==WATER_ID {
                    refl=shade_water(reflected.ci,reflected.voxel,reflected.normal,point,mirror,t+reflected.t);
                } else {
                    refl=shade_hit(reflected.ci,reflected.voxel,material,reflected.normal,point,mirror,
                        t+reflected.t,GLASS_SHADOW_DIST,secondary_smooth(),false);
                }
                refl=mix(refl,sky_base(mirror,1.0),1.0-transmittance_from(hit.y,reflected.t,mirror));
            }
        }
    }

    return mix(through * tint, refl, schlick_glass(max(-dot(rd, n), 1e-3)));
}

// Exp 2 (half-res glass legs): quadrant threads share one shade_glass. The two
// trace_world legs are the cost the bench prices (glass-reflect-b +41% wall);
// across a 2x2 tile those radiance fields are smooth, so legs run once per
// quadrant while the per-pixel tail (shaft, transmittance, water path) keeps
// its own inputs -- flat leg radiance, crisp silhouettes.
var<private> quad_glass: vec3<f32>;
var<private> quad_glass_ok: bool = false;
fn shade_glass_shared(ci: u32, v: vec3<u32>, normal_id: u32, hit: vec3<f32>, rd: vec3<f32>, t: f32) -> vec3<f32> {
    if !quad_glass_ok {
        quad_glass = shade_glass(ci, v, normal_id, hit, rd, t);
        quad_glass_ok = true;
    }
    return quad_glass;
}

fn underwater_body() -> vec3<f32> {
    return WATER_BODY * medium_light(world_sample(frame.cam_pos).x);
}

fn surface_from_below(above: vec3<f32>, rd: vec3<f32>, dist: f32) -> vec3<f32> {

    if (frame.flags & FLAG_SNELL) == 0u {
        return above;
    }

    let depth = frame.sea_level - frame.cam_pos.y;
    let s = smoothstep(SNELL_MIN_DEPTH, WATER_DARK_DEPTH, depth);
    if s <= 0.0 {
        return above;
    }

    if rd.y <= 0.0 {
        return above;
    }
    let t_surf = (frame.sea_level - frame.cam_pos.y) / rd.y;

    if t_surf >= dist || t_surf <= 0.0 {
        return above;
    }
    let surf = frame.cam_pos + rd * t_surf;

    let mirror_detail = 1.0 - smoothstep(WATER_REFRACT_NEAR, WATER_REFRACT_FAR, t_surf);

    var nup = vec3<f32>(0.0, 1.0, 0.0);

    if SPEC_WAVES && (frame.flags & FLAG_WAVES) != 0u && mirror_detail > 0.0 {
        let px_world = max(t_surf, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y);
        var kk = 0.0;
        var vh = vec2<f32>(0.0, 0.0);
        let aniso = SPEC_WAVE_ANISO && (frame.flags & FLAG_WAVE_ANISO) != 0u;
        if aniso {
            let k = 1.0 / max(abs(rd.y), WAVE_GRAZE_MIN);
            kk = k * k - 1.0;
            vh = rd.xz * inverseSqrt(max(dot(rd.xz, rd.xz), 1e-12));
        }

        let w = wave_field(surf.xz, px_world, vh, kk, aniso, 1.0);
        nup = wave_normal(w.xy);
    }

    let cos_i = clamp(dot(rd, nup), 1e-3, 1.0);
    let sin2_t = WATER_IOR * WATER_IOR * (1.0 - cos_i * cos_i);
    var f = 1.0;
    if sin2_t < 1.0 {
        f = schlick(sqrt(1.0 - sin2_t));
    }

    let below = underwater_body();
    let transmitted = mix(above, underwater_body(), s * SNELL_DIM_MAX);
    return mix(transmitted, below, f * s);
}

@compute @workgroup_size(16, 8, 1)
fn resolve(
@builtin(global_invocation_id) gid: vec3<u32>) {
    if SPEC_GLASS_QUAD && RESOLVE_LANE == 1u {
        let bx = gid.x * 2u;
        let by = gid.y * 2u;
        resolve_pixel(bx, by);
        resolve_pixel(bx + 1u, by);
        resolve_pixel(bx, by + 1u);
        resolve_pixel(bx + 1u, by + 1u);
        return;
    }
    resolve_pixel(gid.x, gid.y);
}

fn resolve_pixel(px: u32, py: u32) {
    if px >= frame.res.x || py >= frame.res.y {
        return;
    }
    let pix = py * frame.res.x + px;
    let rd = ray_dir(f32(px) + 0.5, f32(py) + 0.5);

    if RESOLVE_LANE != 0u && (frame.flags & FLAG_HEATMAP) != 0u { return; }
    if (frame.flags & FLAG_HEATMAP) != 0u {
        let it = f32(atomicLoad(&dbg[pix]));
        textureStore(out_tex, vec2<i32>(i32(px), i32(py)), vec4<f32>(heat(it / 300.0), 1.0));
        return;
    }

    let underwater = (frame.flags & FLAG_UNDERWATER) != 0u;
    let key = atomicLoad(&vis[pix]);
    if RESOLVE_LANE != 0u {
        if key == 0lu { return; }
        let filter_ci = u32(key >> 24u) & 0x1FFFu;
        let filter_low = u32(key & 0xFFFFFFlu);
        let filter_v = vec3<u32>((filter_low >> 16u)&VOXEL_MASK,(filter_low >> 8u)&VOXEL_MASK,filter_low&VOXEL_MASK);
        let filter_id = block_at(filter_ci,filter_v);
        if RESOLVE_LANE == 1u && (!SPEC_GLASS || filter_id != GLASS_ID) { return; }
        if RESOLVE_LANE == 2u && filter_id != WATER_ID { return; }
    }
    var color: vec3<f32>;
    var dist = frame.far;
    if key == 0lu {

        var shaft = 1.0;
        if !underwater {
            shaft = sun_shaft(frame.cam_pos, rd, frame.far, 1.0, px, py);
        }
        color = sky_color(frame.cam_pos, rd, 0.0, shaft);
    } else {
        let normal_id = u32(key >> 37u) & 7u;
        let ci = u32(key >> 24u) & 0x1FFFu;
        let low = u32(key & 0xFFFFFFlu);
        let v = vec3<u32>((low >> 16u) & VOXEL_MASK, (low >> 8u) & VOXEL_MASK, low & VOXEL_MASK);
        let micro = vec3<u32>((low >> 22u) & 3u, (low >> 14u) & 3u, (low >> 6u) & 3u);

        let t = hit_t(ci, v, micro, normal_id, rd);
        let hit = frame.cam_pos + rd * t;
        let id = block_at(ci, v);
        dist = t;

        if RESOLVE_LANE == 1u {
            color=shade_glass_shared(ci,v,normal_id,hit,rd,t);
        } else if RESOLVE_LANE == 2u {
            if SPEC_WATER_SEC { color=shade_water_sec(ci,v,normal_id,hit,rd,t,px,py); }
            else { color=shade_water(ci,v,normal_id,hit,rd,t); }
        } else {
            if id == WATER_ID {
                if RESOLVE_LANE == 0u { return; }
                else {
            if SPEC_WATER_SEC { color=shade_water_sec(ci,v,normal_id,hit,rd,t,px,py); }
            else { color=shade_water(ci,v,normal_id,hit,rd,t); }
                }
            } else if SPEC_GLASS && id == GLASS_ID {
                if RESOLVE_LANE == 0u { return; }
                else { color=shade_glass_shared(ci,v,normal_id,hit,rd,t); }
            } else {
                color=shade_hit_cached(ci,v,id,normal_id,hit,rd,t,pix);
            }
        }

        var wet = 0.0;
        if underwater && SURFACE_DEBUG == 0u {
            wet = water_path(t, rd);
        }

        let alpha = 1.0 - transmittance_from(frame.cam_pos.y + wet * rd.y, t - wet, rd);
        var shaft = 1.0;
        if !underwater {
            shaft = sun_shaft(frame.cam_pos, rd, t, alpha, px, py);
        }
        if SURFACE_DEBUG == 0u { color = mix(color, sky_base(rd, shaft), alpha); }
    }
    if underwater && SURFACE_DEBUG == 0u {

        color = surface_from_below(color, rd, dist);
        color = water_medium(color, water_path(dist, rd), underwater_body());
    }
    textureStore(out_tex, vec2<i32>(i32(px), i32(py)), vec4<f32>(color, 1.0));
}

@compute @workgroup_size(8, 8, 1)
fn water_sec(@builtin(global_invocation_id) gid: vec3<u32>) {
    let hx = gid.x;
    let hy = gid.y;
    let span = 1u << WATER_SEC_SHIFT;
    let hw = (frame.res.x + span - 1u) >> WATER_SEC_SHIFT;
    let hh = (frame.res.y + span - 1u) >> WATER_SEC_SHIFT;
    if hx >= hw || hy >= hh {
        return;
    }
    var leg_r = vec4<f32>(0.0, 0.0, 0.0, WATER_REFRACT_DIST);
    var leg_m = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    var best_t = 3.4e38;
    var has = false;
    var bci = 0u;
    var bv = vec3<u32>(0u, 0u, 0u);
    var bmicro = vec3<u32>(0u, 0u, 0u);
    var bnid = 0u;
    var bpx = 0u;
    var bpy = 0u;
    for (var i = 0u; i < span; i = i + 1u) {
        for (var j = 0u; j < span; j = j + 1u) {
            let pxs = span * hx + i;
            let pys = span * hy + j;
            if pxs >= frame.res.x || pys >= frame.res.y {
                continue;
            }
            let key = atomicLoad(&vis[pys * frame.res.x + pxs]);
            if key == 0lu {
                continue;
            }
            let nids = u32(key >> 37u) & 7u;
            let cis = u32(key >> 24u) & 0x1FFFu;
            let low = u32(key & 0xFFFFFFlu);
            let vs_ = vec3<u32>((low >> 16u) & VOXEL_MASK, (low >> 8u) & VOXEL_MASK, low & VOXEL_MASK);
            let micros = vec3<u32>((low >> 22u) & 3u, (low >> 14u) & 3u, (low >> 6u) & 3u);
            if block_at(cis, vs_) != WATER_ID {
                continue;
            }
            let rd0 = ray_dir(f32(pxs) + 0.5, f32(pys) + 0.5);
            let ts = hit_t(cis, vs_, micros, nids, rd0);
            if ts < best_t {
                best_t = ts;
                bci = cis;
                bv = vs_;
                bmicro = micros;
                bnid = nids;
                bpx = pxs;
                bpy = pys;
                has = true;
            }
        }
    }
    if has {
        let rd_d = ray_dir(f32(bpx) + 0.5, f32(bpy) + 0.5);
        let hit_d = frame.cam_pos + rd_d * best_t;
        let ctx = water_ctx(bci, bv, bnid, hit_d, rd_d, best_t);
        if (frame.flags & FLAG_WATER_REFRACT) != 0u && ctx.refract_detail > 0.0 {
            leg_r = water_refract_leg(hit_d, ctx.n, rd_d, best_t);
        }
        if (frame.flags & FLAG_WATER_REFLECT) != 0u && ctx.reflect_detail > 0.0 {
            leg_m = water_reflect_leg(hit_d, ctx.n, ctx.mirror_t, best_t);
        }
    }
    let hpos = vec2<i32>(i32(hx), i32(hy));
    textureStore(water_refr_tex, hpos, leg_r);
    textureStore(water_refl_tex, hpos, leg_m);
}

fn shade_hit_legacy_cached(ci: u32, v: vec3<u32>, id: u32, normal_id: u32, hit: vec3<f32>, rd: vec3<f32>, dist: f32,
             shadow_dist: f32, smooth_light: bool, do_spec: bool, pix: u32) -> vec3<f32> {
        let c = chunks[ci];
        let vs = c.voxel_size;
        let foliage = normal_id >= NORMAL_CROSS_A;
        var n = normal_of(normal_id);
        if foliage && dot(n, rd) > 0.0 {
            n = -n;
        }
        var spec_pre = vec3<f32>(0.0);
        if SPEC_SKY_SPECULAR && do_spec {
            spec_pre = sky_base(reflect(rd, n), 1.0)
                * schlick_ground(clamp(dot(n, -rd), 0.0, 1.0));
        }
        let axis = normal_id >> 1u;
        let box_min = c.origin + vec3<f32>(v) * vs;
        let local = clamp((hit - box_min) / vs, vec3<f32>(0.0), vec3<f32>(1.0));
        let face_word = block_faces[id * 8u + normal_id];
        let layer = face_layer(face_word);
        var uv = face_uv(normal_id, local);
        if SPEC_TEX_VARIATION && (frame.flags & FLAG_TEX_VARIATION) != 0u {
            uv = permute_uv(uv, face_perm(face_word), perm_key(box_min, normal_id));
        }
        var fa: f32;
        var fb: f32;
        var ta: vec3<i32>;
        var tb: vec3<i32>;
        if axis == 0u {
            fa = local.y; fb = local.z;
            ta = vec3<i32>(0, 1, 0); tb = vec3<i32>(0, 0, 1);
        } else if axis == 1u {
            fa = local.x; fb = local.z;
            ta = vec3<i32>(1, 0, 0); tb = vec3<i32>(0, 0, 1);
        } else {
            fa = local.x; fb = local.y;
            ta = vec3<i32>(1, 0, 0); tb = vec3<i32>(0, 1, 0);
        }
        let grazing = max(abs(dot(n, rd)), 0.25);
        let footprint = max(dist, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y) * 16.0 / vs / grazing;
        let lod = clamp(log2(max(footprint, 1e-4)) + frame.mip_bias, 0.0, 3.0);
        let texel = textureSampleLevel(atlas, atlas_samp, uv, i32(layer), lod);
        var albedo = texel.rgb;
        let tint_mask = select(texel.a, 1.0, foliage);
        if SPEC_TINT && (frame.flags & FLAG_TINT) != 0u && tint_mask > 0.0 {
            let k = tint_mask * frame.tint_strength;
            albedo = albedo * mix(vec3<f32>(1.0), biome_tint_use(hit.xz), k);
        }
        if SPEC_SHORE_WET && id == SAND_ID {
            let wet = 1.0 - smoothstep(frame.sea_level, frame.sea_level + WET_BAND, hit.y);
            albedo = albedo * mix(1.0, WET_DARK, wet);
        }
        if SPEC_FOLIAGE_RICH
            && (foliage || ((id == GRASS_ID || id == MEADOW_ID) && normal_id == NORMAL_PY))
        {
            let hr = hash_alpha(box_min);
            let hv = hash_alpha(box_min + vec3<f32>(17.31, 7.77, 3.03));
            var rich = vec3<f32>(1.0);
            if hr < 0.16 {
                rich = vec3<f32>(1.30, 1.10, 0.55);
            } else if hr > 0.97 {
                rich = vec3<f32>(1.35, 0.72, 0.95);
            }
            albedo = albedo * rich * mix(0.88, 1.10, hv);
        }
        if SPEC_CANOPY_RELIEF && is_cutout_id(id) {
            let clump = cloud_noise(box_min.xz * 0.21
                + vec2<f32>(box_min.y * 0.313, box_min.y * 0.173));
            let relief = mix(0.72, 1.34, clump * clump);
            let micro = mix(0.92, 1.08, hash_alpha(box_min));
            let toward_sun = clamp(dot(n, frame.sun_dir) * 0.5 + 0.5, 0.0, 1.0);
            albedo = albedo * relief * micro * mix(0.84, 1.0, toward_sun);
        }
        let sky_tint = surface_sky_fill(n);
        var ao: f32 = 1.0;
        var sky_sm: f32 = 0.0;
        var amb: vec3<f32> = vec3<f32>(0.0);
        if foliage {
            let packed = light_at(ci, vec3<i32>(v));
            let sk = light_curve(light_sky(packed)) * frame.daylight;
            sky_sm = sk;
            if !SPEC_PROBE_AMBIENT {
                if SPEC_LIGHT_RGB {
                    amb = block_ambient(light_curve3(light_block_rgb(packed)), sk, sky_tint);
                } else {
                    let bk = light_curve(light_block_level(packed));
                    amb = mix(sky_tint, BLOCK_TINT, bk / max(sk + bk, 1e-4)) * max(sk, bk);
                }
            }
        } else if !smooth_light {
            let air_v1 = vec3<i32>(v) + vec3<i32>(n);
            var packed1: u32;
            if all(air_v1 >= vec3<i32>(0)) && all(air_v1 <= vec3<i32>(63)) {
                packed1 = light_at(ci, air_v1);
            } else {
                packed1 = world_sample(box_min + vec3<f32>(0.5 * vs) + n * vs).x;
            }
            let sk1 = light_curve(light_sky(packed1)) * frame.daylight;
            sky_sm = sk1;
            if !SPEC_PROBE_AMBIENT {
                if SPEC_LIGHT_RGB {
                    amb = block_ambient(light_curve3(light_block_rgb(packed1)), sk1, sky_tint);
                } else {
                    let bk1 = light_curve(light_block_level(packed1));
                    amb = mix(sky_tint, BLOCK_TINT, bk1 / max(sk1 + bk1, 1e-4)) * max(sk1, bk1);
                }
            }
        } else {
            let air_c = box_min + vec3<f32>(0.5 * vs) + n * vs;
            let step_a = vec3<f32>(ta) * vs;
            let step_b = vec3<f32>(tb) * vs;
            let air_v = vec3<i32>(v) + vec3<i32>(n);
            let span = abs(ta) + abs(tb);
            let local_only = all(air_v - span >= vec3<i32>(0)) && all(air_v + span <= vec3<i32>(63));
            var nb: array<vec2<u32>, 9>;
            if local_only {
                gather_face(ci, air_v, ta, tb, &nb);
            } else {
                for (var i = 0u; i < 9u; i = i + 1u) {
                    nb[i] = world_sample(air_c + step_a * (f32(i % 3u) - 1.0) + step_b * (f32(i / 3u) - 1.0));
                }
            }
            var sky_n: array<f32, 9>;
            var blk_n: array<f32, 9>;
            var open_n: array<f32, 9>;
            var any_blk = 0u;
            for (var i = 0u; i < 9u; i = i + 1u) {
                sky_n[i] = light_curve(light_sky(nb[i].x)) * frame.daylight;
                if !SPEC_PROBE_AMBIENT {
                    if SPEC_LIGHT_RGB {
                        if SPEC_EMITTER_GATHER {
                            any_blk = any_blk | (nb[i].x & 0xFFFu);
                        }
                    } else {
                        blk_n[i] = light_curve(light_block_level(nb[i].x));
                    }
                }
                open_n[i] = 1.0 - f32(nb[i].y);
            }
            var blk4 = vec3<f32>(0.0);
            if SPEC_EMITTER_GATHER && SPEC_LIGHT_RGB && !SPEC_PROBE_AMBIENT && any_blk != 0u {
                blk4 = light_curve3(light_block_rgb(nb[4].x));
            }
            let ao_on = (frame.flags & FLAG_AO) != 0u;
            var amb_c: array<vec3<f32>, 4>;
            var sky_c: array<f32, 4>;
            var ao_c: array<f32, 4>;
            for (var k = 0u; k < 4u; k = k + 1u) {
                let ia = u32(4 + (i32(k & 1u) * 2 - 1));
                let ib = u32(4 + (i32(k >> 1u) * 2 - 1) * 3);
                let ic = ia + ib - 4u;
                let inv = 1.0 / (1.0 + open_n[ia] + open_n[ib] + open_n[ic]);
                let sk = (sky_n[4] + sky_n[ia] * open_n[ia] + sky_n[ib] * open_n[ib] + sky_n[ic] * open_n[ic]) * inv;
                sky_c[k] = sk;
                if !SPEC_PROBE_AMBIENT {
                    if SPEC_LIGHT_RGB {
                        if SPEC_EMITTER_GATHER && any_blk != 0u {
                            let ba = light_curve3(light_block_rgb(nb[ia].x));
                            let bb = light_curve3(light_block_rgb(nb[ib].x));
                            let bc = light_curve3(light_block_rgb(nb[ic].x));
                            let bkv = (blk4 + ba * open_n[ia] + bb * open_n[ib] + bc * open_n[ic]) * inv;
                            amb_c[k] = block_ambient(bkv, sk, sky_tint);
                        } else {
                            amb_c[k] = sky_tint * sk;
                        }
                    } else {
                        let bk = (blk_n[4] + blk_n[ia] * open_n[ia] + blk_n[ib] * open_n[ib] + blk_n[ic] * open_n[ic]) * inv;
                        amb_c[k] = mix(sky_tint, BLOCK_TINT, bk / max(sk + bk, 1e-4)) * max(sk, bk);
                    }
                }
                let s1 = 1.0 - open_n[ia];
                let s2 = 1.0 - open_n[ib];
                let occ = select(s1 + s2 + 1.0 - open_n[ic], 3.0, s1 > 0.5 && s2 > 0.5);
                ao_c[k] = select(1.0, 1.0 - occ * AO_STRENGTH, ao_on);
            }
            ao = mix(mix(ao_c[0], ao_c[1], fa), mix(ao_c[2], ao_c[3], fa), fb);
            sky_sm = mix(mix(sky_c[0], sky_c[1], fa), mix(sky_c[2], sky_c[3], fa), fb);
            if !SPEC_PROBE_AMBIENT {
                amb = mix(mix(amb_c[0], amb_c[1], fa), mix(amb_c[2], amb_c[3], fa), fb);
            }
        }
        if SPEC_PROBE_AMBIENT {
            amb = probe_tap(hit);
        } else if SPEC_PROBE_TAP {
            amb = amb * probe_tap(hit);
        }
        var visibility = 1.0;
        var direct = 0.0;
        let ndl = dot(n, frame.sun_dir);
        if ndl > 0.0 && frame.daylight > 0.01 {
            var lit = 1.0;
            if (frame.flags & FLAG_SHADOWS) != 0u {
                lit = textureLoad(primary_sun,vec2<i32>(i32(pix%frame.res.x),i32(pix/frame.res.x)),0).x;
                if SPEC_DISTANT_SHADOWS && (frame.flags & FLAG_DISTANT_SHADOWS) != 0u && lit > 0.0 {
                    lit = lit * terrain_shade_beyond(hit, shadow_dist);
                }
            }
            if lit > 0.0 {
                let cloud_fp = max(dist, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y)
                    * exp2(frame.mip_bias);
                lit = lit * cloud_shade(hit, cloud_fp);
            }
            visibility = lit;
            direct = lit * ndl * sky_sm;
            if SPEC_CAUSTICS {
                let cdepth = frame.sea_level - hit.y;
                if cdepth > 0.0 && cdepth < CAUSTIC_DEPTH && direct > 0.0 {
                    let sp = hit.xz - frame.sun_dir.xz * (cdepth / max(frame.sun_dir.y, 1e-3));
                    let a = frame.wave_amp;
                    let l0 = frame.wave_scale;
                    let ph = frame.time * frame.wave_speed * WAVE_TAU;
                    let g0 = wave_octave(sp, WAVE_D0, l0, a * 0.42, ph, CAUSTIC_PX,
                                         vec2<f32>(0.0), 0.0, false);
                    let g1 = wave_octave(sp, WAVE_D1, l0 / 2.17, a * 0.27, ph * 1.4731,
                                         CAUSTIC_PX, vec2<f32>(0.0), 0.0, false);
                    let r0 = g0.x * g0.x + g0.y * g0.y;
                    let r1 = g1.x * g1.x + g1.y * g1.y;
                    let sheets = r0 * r1 * CAUSTIC_GAIN;
                    direct = direct * (1.0 + sheets * exp(-cdepth * CAUSTIC_FADE));
                }
            }
        }
        var probe = vec4<f32>(1.0);
        if !SPEC_PROBE_AMBIENT && (SPEC_PROBE_CUBE || SPEC_PROBE_BOUNCE) {
            probe = probe_field(hit + n * PROBE_NORMAL_BIAS, normal_id);
        }
        var shade = 1.0;
        if !SPEC_PROBE_AMBIENT {
            shade = select(face_shade(normal_id), 1.0, SPEC_LIGHTING_REPAIR);
            if SPEC_PROBE_CUBE {
                shade = shade * probe.a;
            }
        }
        let ambient_rgb = amb * 0.50;
        let sun_rgb = vec3<f32>(1.0000, 0.8469, 0.5647) * (direct * select(0.52, 0.78, SPEC_LIGHTING_REPAIR));
        var floor_rgb = vec3<f32>(0.0);
        if !SPEC_PROBE_AMBIENT {
            floor_rgb = vec3<f32>(0.85, 0.90, 1.00) * frame.ambient;
            if SPEC_PROBE_BOUNCE {
                floor_rgb = floor_rgb * probe.rgb;
            }
            if SPEC_PROBE_SUN {
                floor_rgb = floor_rgb
                    + vec3<f32>(0.85, 0.90, 1.00) * probe.rgb * (probe_sun_gain() * sky_sm);
            }
        }
        if SURFACE_DEBUG == 1u { return n * 0.5 + vec3<f32>(0.5); }
        if SURFACE_DEBUG == 2u { return vec3<f32>(visibility); }
        if SURFACE_DEBUG == 3u { return sun_rgb; }
        if SURFACE_DEBUG == 4u { return ambient_rgb * ao + floor_rgb; }
        let spec_rgb = spec_pre * (sky_sm * SKY_SPEC_GAIN);
        return surface_radiance(albedo, surface_light(ambient_rgb, sun_rgb, ao, floor_rgb, shade), spec_rgb, id);
}

fn shade_hit_compact_cached(ci: u32, v: vec3<u32>, id: u32, normal_id: u32, hit: vec3<f32>, rd: vec3<f32>, dist: f32,
             shadow_dist: f32, smooth_light: bool, do_spec: bool, pix: u32) -> vec3<f32> {
        let c = chunks[ci];
        let vs = c.voxel_size;
        let foliage = normal_id >= NORMAL_CROSS_A;
        var n = normal_of(normal_id);
        if foliage && dot(n, rd) > 0.0 {
            n = -n;
        }
        var spec_pre = vec3<f32>(0.0);
        if SPEC_SKY_SPECULAR && do_spec {
            spec_pre = sky_base(reflect(rd, n), 1.0)
                * schlick_ground(clamp(dot(n, -rd), 0.0, 1.0));
        }
        let grazing = max(abs(dot(n, rd)), 0.25);
        let footprint = max(dist, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y) * 16.0 / vs / grazing;
        let lod = clamp(log2(max(footprint, 1e-4)) + frame.mip_bias, 0.0, 3.0);
        var light_rgb: vec3<f32>;
        var face_scale: f32;
        var spec_value: vec3<f32>;
        var material_ndl: f32;
        {
        let axis = normal_id >> 1u;
        let box_min = c.origin + vec3<f32>(v) * vs;
        let local = clamp((hit - box_min) / vs, vec3<f32>(0.0), vec3<f32>(1.0));
        var fa: f32;
        var fb: f32;
        var ta: vec3<i32>;
        var tb: vec3<i32>;
        if axis == 0u {
            fa = local.y; fb = local.z;
            ta = vec3<i32>(0, 1, 0); tb = vec3<i32>(0, 0, 1);
        } else if axis == 1u {
            fa = local.x; fb = local.z;
            ta = vec3<i32>(1, 0, 0); tb = vec3<i32>(0, 0, 1);
        } else {
            fa = local.x; fb = local.y;
            ta = vec3<i32>(1, 0, 0); tb = vec3<i32>(0, 1, 0);
        }
        let sky_tint = surface_sky_fill(n);
        var ao: f32 = 1.0;
        var sky_sm: f32 = 0.0;
        var amb: vec3<f32> = vec3<f32>(0.0);
        if foliage {
            let packed = light_at(ci, vec3<i32>(v));
            let sk = light_curve(light_sky(packed)) * frame.daylight;
            sky_sm = sk;
            if !SPEC_PROBE_AMBIENT {
                if SPEC_LIGHT_RGB {
                    amb = block_ambient(light_curve3(light_block_rgb(packed)), sk, sky_tint);
                } else {
                    let bk = light_curve(light_block_level(packed));
                    amb = mix(sky_tint, BLOCK_TINT, bk / max(sk + bk, 1e-4)) * max(sk, bk);
                }
            }
        } else if !smooth_light {
            let air_v1 = vec3<i32>(v) + vec3<i32>(n);
            var packed1: u32;
            if all(air_v1 >= vec3<i32>(0)) && all(air_v1 <= vec3<i32>(63)) {
                packed1 = light_at(ci, air_v1);
            } else {
                packed1 = world_sample(box_min + vec3<f32>(0.5 * vs) + n * vs).x;
            }
            let sk1 = light_curve(light_sky(packed1)) * frame.daylight;
            sky_sm = sk1;
            if !SPEC_PROBE_AMBIENT {
                if SPEC_LIGHT_RGB {
                    amb = block_ambient(light_curve3(light_block_rgb(packed1)), sk1, sky_tint);
                } else {
                    let bk1 = light_curve(light_block_level(packed1));
                    amb = mix(sky_tint, BLOCK_TINT, bk1 / max(sk1 + bk1, 1e-4)) * max(sk1, bk1);
                }
            }
        } else {
            let air_c = box_min + vec3<f32>(0.5 * vs) + n * vs;
            let step_a = vec3<f32>(ta) * vs;
            let step_b = vec3<f32>(tb) * vs;
            let air_v = vec3<i32>(v) + vec3<i32>(n);
            let span = abs(ta) + abs(tb);
            let local_only = all(air_v - span >= vec3<i32>(0)) && all(air_v + span <= vec3<i32>(63));
            var nb: array<vec2<u32>, 9>;
            if local_only {
                gather_face(ci, air_v, ta, tb, &nb);
            } else {
                for (var i = 0u; i < 9u; i = i + 1u) {
                    nb[i] = world_sample(air_c + step_a * (f32(i % 3u) - 1.0) + step_b * (f32(i / 3u) - 1.0));
                }
            }
            var sky_n: array<f32, 9>;
            var blk_n: array<f32, 9>;
            var open_n: array<f32, 9>;
            var any_blk = 0u;
            for (var i = 0u; i < 9u; i = i + 1u) {
                sky_n[i] = light_curve(light_sky(nb[i].x)) * frame.daylight;
                if !SPEC_PROBE_AMBIENT {
                    if SPEC_LIGHT_RGB {
                        if SPEC_EMITTER_GATHER {
                            any_blk = any_blk | (nb[i].x & 0xFFFu);
                        }
                    } else {
                        blk_n[i] = light_curve(light_block_level(nb[i].x));
                    }
                }
                open_n[i] = 1.0 - f32(nb[i].y);
            }
            var blk4 = vec3<f32>(0.0);
            if SPEC_EMITTER_GATHER && SPEC_LIGHT_RGB && !SPEC_PROBE_AMBIENT && any_blk != 0u {
                blk4 = light_curve3(light_block_rgb(nb[4].x));
            }
            let ao_on = (frame.flags & FLAG_AO) != 0u;
            var amb_c: array<vec3<f32>, 4>;
            var sky_c: array<f32, 4>;
            var ao_c: array<f32, 4>;
            for (var k = 0u; k < 4u; k = k + 1u) {
                let ia = u32(4 + (i32(k & 1u) * 2 - 1));
                let ib = u32(4 + (i32(k >> 1u) * 2 - 1) * 3);
                let ic = ia + ib - 4u;
                let inv = 1.0 / (1.0 + open_n[ia] + open_n[ib] + open_n[ic]);
                let sk = (sky_n[4] + sky_n[ia] * open_n[ia] + sky_n[ib] * open_n[ib] + sky_n[ic] * open_n[ic]) * inv;
                sky_c[k] = sk;
                if !SPEC_PROBE_AMBIENT {
                    if SPEC_LIGHT_RGB {
                        if SPEC_EMITTER_GATHER && any_blk != 0u {
                            let ba = light_curve3(light_block_rgb(nb[ia].x));
                            let bb = light_curve3(light_block_rgb(nb[ib].x));
                            let bc = light_curve3(light_block_rgb(nb[ic].x));
                            let bkv = (blk4 + ba * open_n[ia] + bb * open_n[ib] + bc * open_n[ic]) * inv;
                            amb_c[k] = block_ambient(bkv, sk, sky_tint);
                        } else {
                            amb_c[k] = sky_tint * sk;
                        }
                    } else {
                        let bk = (blk_n[4] + blk_n[ia] * open_n[ia] + blk_n[ib] * open_n[ib] + blk_n[ic] * open_n[ic]) * inv;
                        amb_c[k] = mix(sky_tint, BLOCK_TINT, bk / max(sk + bk, 1e-4)) * max(sk, bk);
                    }
                }
                let s1 = 1.0 - open_n[ia];
                let s2 = 1.0 - open_n[ib];
                let occ = select(s1 + s2 + 1.0 - open_n[ic], 3.0, s1 > 0.5 && s2 > 0.5);
                ao_c[k] = select(1.0, 1.0 - occ * AO_STRENGTH, ao_on);
            }
            ao = mix(mix(ao_c[0], ao_c[1], fa), mix(ao_c[2], ao_c[3], fa), fb);
            sky_sm = mix(mix(sky_c[0], sky_c[1], fa), mix(sky_c[2], sky_c[3], fa), fb);
            if !SPEC_PROBE_AMBIENT {
                amb = mix(mix(amb_c[0], amb_c[1], fa), mix(amb_c[2], amb_c[3], fa), fb);
            }
        }
        if SPEC_PROBE_AMBIENT {
            amb = probe_tap(hit);
        } else if SPEC_PROBE_TAP {
            amb = amb * probe_tap(hit);
        }
        var visibility = 1.0;
        var direct = 0.0;
        let ndl = dot(n, frame.sun_dir);
        if ndl > 0.0 && frame.daylight > 0.01 {
            var lit = 1.0;
            if (frame.flags & FLAG_SHADOWS) != 0u {
                lit = textureLoad(primary_sun,vec2<i32>(i32(pix%frame.res.x),i32(pix/frame.res.x)),0).x;
                if SPEC_DISTANT_SHADOWS && (frame.flags & FLAG_DISTANT_SHADOWS) != 0u && lit > 0.0 {
                    lit = lit * terrain_shade_beyond(hit, shadow_dist);
                }
            }
            if lit > 0.0 {
                let cloud_fp = max(dist, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y)
                    * exp2(frame.mip_bias);
                lit = lit * cloud_shade(hit, cloud_fp);
            }
            visibility = lit;
            direct = lit * ndl * sky_sm;
            if SPEC_CAUSTICS {
                let cdepth = frame.sea_level - hit.y;
                if cdepth > 0.0 && cdepth < CAUSTIC_DEPTH && direct > 0.0 {
                    let sp = hit.xz - frame.sun_dir.xz * (cdepth / max(frame.sun_dir.y, 1e-3));
                    let a = frame.wave_amp;
                    let l0 = frame.wave_scale;
                    let ph = frame.time * frame.wave_speed * WAVE_TAU;
                    let g0 = wave_octave(sp, WAVE_D0, l0, a * 0.42, ph, CAUSTIC_PX,
                                         vec2<f32>(0.0), 0.0, false);
                    let g1 = wave_octave(sp, WAVE_D1, l0 / 2.17, a * 0.27, ph * 1.4731,
                                         CAUSTIC_PX, vec2<f32>(0.0), 0.0, false);
                    let r0 = g0.x * g0.x + g0.y * g0.y;
                    let r1 = g1.x * g1.x + g1.y * g1.y;
                    let sheets = r0 * r1 * CAUSTIC_GAIN;
                    direct = direct * (1.0 + sheets * exp(-cdepth * CAUSTIC_FADE));
                }
            }
        }
        var probe = vec4<f32>(1.0);
        if !SPEC_PROBE_AMBIENT && (SPEC_PROBE_CUBE || SPEC_PROBE_BOUNCE) {
            probe = probe_field(hit + n * PROBE_NORMAL_BIAS, normal_id);
        }
        var shade = 1.0;
        if !SPEC_PROBE_AMBIENT {
            shade = select(face_shade(normal_id), 1.0, SPEC_LIGHTING_REPAIR);
            if SPEC_PROBE_CUBE {
                shade = shade * probe.a;
            }
        }
        let ambient_rgb = amb * 0.50;
        let sun_rgb = vec3<f32>(1.0000, 0.8469, 0.5647) * (direct * select(0.52, 0.78, SPEC_LIGHTING_REPAIR));
        var floor_rgb = vec3<f32>(0.0);
        if !SPEC_PROBE_AMBIENT {
            floor_rgb = vec3<f32>(0.85, 0.90, 1.00) * frame.ambient;
            if SPEC_PROBE_BOUNCE {
                floor_rgb = floor_rgb * probe.rgb;
            }
            if SPEC_PROBE_SUN {
                floor_rgb = floor_rgb
                    + vec3<f32>(0.85, 0.90, 1.00) * probe.rgb * (probe_sun_gain() * sky_sm);
            }
        }
        if SURFACE_DEBUG == 1u { return n * 0.5 + vec3<f32>(0.5); }
        if SURFACE_DEBUG == 2u { return vec3<f32>(visibility); }
        if SURFACE_DEBUG == 3u { return sun_rgb; }
        if SURFACE_DEBUG == 4u { return ambient_rgb * ao + floor_rgb; }
        let spec_rgb = spec_pre * (sky_sm * SKY_SPEC_GAIN);
        light_rgb = surface_light(ambient_rgb, sun_rgb, ao, floor_rgb, shade);
        face_scale = 1.0;
        spec_value = spec_rgb;
        material_ndl = ndl;
        }
        let material_chunk = chunks[ci];
        let box_min = material_chunk.origin + vec3<f32>(v) * material_chunk.voxel_size;
        let local = clamp((hit - box_min) / material_chunk.voxel_size, vec3<f32>(0.0), vec3<f32>(1.0));
        let face_word = block_faces[id * 8u + normal_id];
        let layer = face_layer(face_word);
        var uv = face_uv(normal_id, local);
        if SPEC_TEX_VARIATION && (frame.flags & FLAG_TEX_VARIATION) != 0u {
            uv = permute_uv(uv, face_perm(face_word), perm_key(box_min, normal_id));
        }
        let texel = textureSampleLevel(atlas, atlas_samp, uv, i32(layer), lod);
        var albedo = texel.rgb;
        let tint_mask = select(texel.a, 1.0, foliage);
        if SPEC_TINT && (frame.flags & FLAG_TINT) != 0u && tint_mask > 0.0 {
            let k = tint_mask * frame.tint_strength;
            albedo = albedo * mix(vec3<f32>(1.0), biome_tint_use(hit.xz), k);
        }
        if SPEC_SHORE_WET && id == SAND_ID {
            let wet = 1.0 - smoothstep(frame.sea_level, frame.sea_level + WET_BAND, hit.y);
            albedo = albedo * mix(1.0, WET_DARK, wet);
        }
        if SPEC_FOLIAGE_RICH
            && (foliage || ((id == GRASS_ID || id == MEADOW_ID) && normal_id == NORMAL_PY))
        {
            let hr = hash_alpha(box_min);
            let hv = hash_alpha(box_min + vec3<f32>(17.31, 7.77, 3.03));
            var rich = vec3<f32>(1.0);
            if hr < 0.16 {
                rich = vec3<f32>(1.30, 1.10, 0.55);
            } else if hr > 0.97 {
                rich = vec3<f32>(1.35, 0.72, 0.95);
            }
            albedo = albedo * rich * mix(0.88, 1.10, hv);
        }
        if SPEC_CANOPY_RELIEF && is_cutout_id(id) {
            let clump = cloud_noise(box_min.xz * 0.21
                + vec2<f32>(box_min.y * 0.313, box_min.y * 0.173));
            let relief = mix(0.72, 1.34, clump * clump);
            let micro = mix(0.92, 1.08, hash_alpha(box_min));
            let toward_sun = clamp(material_ndl * 0.5 + 0.5, 0.0, 1.0);
            albedo = albedo * relief * micro * mix(0.84, 1.0, toward_sun);
        }
        return surface_radiance(albedo, light_rgb * face_scale, spec_value, id);
}
fn shade_hit_cached(ci: u32, v: vec3<u32>, id: u32, normal_id: u32, hit: vec3<f32>, rd: vec3<f32>, dist: f32, pix: u32) -> vec3<f32> {
    if SPEC_COMPACT_SHADE_HIT {
        return shade_hit_compact_cached(ci,v,id,normal_id,hit,rd,dist,frame.shadow_dist,true,true,pix);
    }
    return shade_hit_legacy_cached(ci,v,id,normal_id,hit,rd,dist,frame.shadow_dist,true,true,pix);
}
