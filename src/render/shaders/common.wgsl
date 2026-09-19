struct Frame {
    cam_pos: vec3<f32>,
    time: f32,
    cam_fwd: vec3<f32>,
    tan_half_fov: f32,
    cam_right: vec3<f32>,
    aspect: f32,
    cam_up: vec3<f32>,
    far: f32,
    sun_dir: vec3<f32>,
    daylight: f32,
    res: vec2<u32>,
    tiles: vec2<u32>,
    chunk_count: u32,
    camera_chunk: u32,
    flags: u32,
    max_pairs: u32,

    fog_density: f32,
    fog_falloff: f32,
    shadow_dist: f32,
    ambient: f32,
    fog_scatter: f32,
    fog_g: f32,
    fog_height: f32,

    mip_bias: f32,
    grid_min: vec3<i32>,

    sea_level: f32,
    grid_dim: vec3<u32>,

    water_absorb: f32,

    jitter: vec2<f32>,

    taa_feedback: f32,

    frame_index: u32,

    cloud_cover: f32,

    cloud_height: f32,

    cloud_scale: f32,

    cloud_speed: f32,

    godray_strength: f32,
    godray_steps: u32,
    godray_dist: f32,

    cloud_shadow: f32,

    tint_strength: f32,
    tint_seed: i32,
    tint_freq_t: f32,
    tint_freq_h: f32,

    wave_amp: f32,

    wave_scale: f32,

    wave_speed: f32,

    wave_reflect_slope: f32,

    shaft_origin: vec2<f32>,
    shaft_texel: f32,
    shaft_soft: f32,

    cloud_patch: f32,
    cloud_relief: f32,
    haze_warm: f32,
    zenith_deep: f32,

    prev_view_proj: mat4x4<f32>,
};

struct Chunk {
    aabb_min: vec3<f32>,
    voxel_size: f32,
    aabb_max: vec3<f32>,
    lod: u32,
    origin: vec3<f32>,

    fade: f32,
    root: vec4<u32>,
    attr_base: u32,
    attr_flags: u32,
    light_base: u32,
    light_flags: u32,

    dry_mask: vec2<u32>,
    reserved: vec2<u32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> chunks: array<Chunk>;
@group(0) @binding(2) var<storage, read> inners: array<vec4<u32>>;
@group(0) @binding(3) var<storage, read> leaves: array<vec2<u32>>;
@group(0) @binding(4) var<storage, read> bricks: array<u32>;
@group(0) @binding(5) var<storage, read> block_faces: array<u32>;

const FACE_LAYER_BITS: u32 = 16u;
fn face_layer(word: u32) -> u32 { return word & 0xffffu; }
fn face_perm(word: u32) -> u32 { return word >> FACE_LAYER_BITS; }

const PERM_NONE: u32 = 0u;
const PERM_FLIP_U: u32 = 1u;
const PERM_D4: u32 = 2u;
@group(0) @binding(6) var<storage, read_write> pairs: array<u32>;
@group(0) @binding(7) var<storage, read_write> counters: array<atomic<u32>>;
@group(0) @binding(8) var<storage, read_write> indirect: array<u32>;
@group(0) @binding(9) var<storage, read_write> vis: array<atomic<u64>>;
@group(0) @binding(10) var<storage, read_write> hiz: array<u32>;
@group(0) @binding(11) var out_tex: texture_storage_2d<rgba16float, write>;
@group(0) @binding(12) var atlas: texture_2d_array<f32>;
@group(0) @binding(13) var atlas_samp: sampler;
@group(0) @binding(14) var<storage, read_write> dbg: array<atomic<u32>>;
@group(0) @binding(15) var<storage, read> grid: array<u32>;

@group(0) @binding(16) var taa_hist: texture_2d<f32>;
@group(0) @binding(17) var taa_src: texture_2d<f32>;
@group(0) @binding(18) var linear_samp: sampler;

@group(0) @binding(19) var<storage, read_write> shaft: array<f32>;

@group(0) @binding(20) var probe_tex: texture_3d<f32>;

@group(0) @binding(21) var probe_samp: sampler;

@group(1) @binding(0) var water_refr_sample: texture_2d<f32>;
@group(1) @binding(1) var water_refl_sample: texture_2d<f32>;
@group(2) @binding(0) var water_refr_tex: texture_storage_2d<rgba16float, write>;
@group(2) @binding(1) var water_refl_tex: texture_storage_2d<rgba16float, write>;

const FULL_BIT: u32 = 0x80000000u;
const PTR_MASK: u32 = 0x7FFFFFFFu;
const UNIFORM_BIT: u32 = 0x80000000u;
const NO_CHUNK: u32 = 0xFFFFFFFFu;
const FLAG_HIZ: u32 = 1u;
const FLAG_HEATMAP: u32 = 2u;
const FLAG_SHADOWS: u32 = 4u;
const FLAG_AO: u32 = 8u;
const FLAG_WATER_REFLECT: u32 = 16u;
const FLAG_WATER_REFRACT: u32 = 32u;
const FLAG_UNDERWATER: u32 = 64u;
const FLAG_TINT: u32 = 128u;

const FLAG_WAVES: u32 = 512u;
const FLAG_WAVE_ANISO: u32 = 1024u;
const FLAG_WAVE_SHOAL: u32 = 2048u;
const FLAG_WAVE_FILL: u32 = 4096u;
const FLAG_TERRAIN_SHAFTS: u32 = 8192u;
const FLAG_TEX_VARIATION: u32 = 16384u;
const FLAG_DISTANT_SHADOWS: u32 = 32768u;

const FLAG_FLAT_SECONDARY: u32 = 1048576u;
const FLAG_WATER_FAR: u32 = 2097152u;
const FLAG_WATER_DARK: u32 = 4194304u;
const FLAG_SNELL: u32 = 8388608u;
const MAX_ITERS: u32 = 512u;

const WATER_FAR_DIST: f32 = 128.0;

const WATER_DARK_DEPTH: f32 = 15.0;

const SNELL_MIN_DEPTH: f32 = 3.0;

const SNELL_DIM_MAX: f32 = 0.9;
const WATER_DARK_DIST: f32 = 64.0;

const SHAFT_DIM: u32 = 512u;
const SHAFT_STEPS: u32 = 8u;
const SHAFT_SLICE: u32 = SHAFT_DIM * SHAFT_DIM;
const SHAFT_HEIGHTS: u32 = 2u * SHAFT_SLICE;
const SHAFT_RESULT: u32 = SHAFT_SLICE;

override SPEC_TINT: bool = true;

override SPEC_FOLIAGE: bool = true;

override SPEC_WAVES: bool = true;

override SPEC_WAVE_ANISO: bool = true;

override SPEC_WAVE_SHOAL: bool = true;

override SPEC_WAVE_FILL: bool = true;

override SPEC_TERRAIN_SHAFTS: bool = true;

override SPEC_TEX_VARIATION: bool = true;

override SPEC_DISTANT_SHADOWS: bool = true;

override SPEC_LEAF_CUTOUT: bool = true;

override SPEC_FULL_MARCH: bool = true;

override SPEC_SHORE_WET: bool = true;
override SPEC_SHORE_FOAM: bool = true;

override SPEC_EMITTER_GATHER: bool = true;

override SPEC_GLASS: bool = true;

override SPEC_WATER_SHADOW_DIST: f32 = 16.0;

override SPEC_LEAF_FILL: f32 = 0.62;

override SPEC_FLAT_SECONDARY: bool = true;

override SPEC_PROBE_TAP: bool = false;

override SPEC_PROBE_AMBIENT: bool = false;

override SPEC_PROBE_CUBE: bool = true;

override SPEC_PROBE_BOUNCE: bool = true;
override SPEC_PROBE_SUN: bool = true;
override SPEC_PROBE_SUN_HIGH: bool = false;

override SPEC_LIGHT_RGB: bool = true;

override SPEC_SKY_TINT: bool = true;

override SPEC_SKY_SPECULAR: bool = true;

override SPEC_WATER_SEC: bool = true;

override WATER_SEC_SHIFT: u32 = 1u;

override SPEC_CAUSTICS: bool = false;

override SPEC_GLASS_REFLECT: bool = false;

override SPEC_SNELL_BEND: bool = false;

override SPEC_TINT_BALANCE: bool = false;

override SPEC_SKY_LOOK: bool = false;

override SPEC_SKY_COOL: bool = false;

override SPEC_WATER_LOOK: bool = false;
override SPEC_FOLIAGE_RICH: bool = false;
override SPEC_CANOPY_RELIEF: bool = false;

override SPEC_WIND_SWAY: bool = false;

override SPEC_SOFT_SHADOWS: bool = false;

const WATER_ID: u32 = 13u;

const SAND_ID: u32 = 4u;

const GLASS_ID: u32 = 10u;
const GLOWSTONE_ID: u32 = 11u;
override SPEC_LIGHTING_REPAIR: bool = true;
override SPEC_WATER_REPAIR: bool = true;
override SURFACE_DEBUG: u32 = 0u;
fn casts_sun_shadow(id: u32) -> bool { return id != GLOWSTONE_ID; }

fn world_block_at(p: vec3<f32>) -> u32 {
    let cell = vec3<i32>(floor(p / 64.0)) - frame.grid_min;
    if any(cell < vec3<i32>(0)) || any(cell >= vec3<i32>(frame.grid_dim)) { return 0u; }
    let u = vec3<u32>(cell);
    let ci = grid[u.x + u.y * frame.grid_dim.x + u.z * frame.grid_dim.x * frame.grid_dim.y];
    if ci == NO_CHUNK { return 0u; }
    let c = chunks[ci];
    let v = vec3<i32>(floor((p - c.origin) / c.voxel_size));
    if any(v < vec3<i32>(0)) || any(v >= vec3<i32>(64)) { return 0u; }
    if !voxel_occupied(ci, v) { return 0u; }
    return block_at(ci, vec3<u32>(v));
}

const ATTR_UNIFORM: u32 = 1u;
const ATTR_HAS_WATER: u32 = 2u;
const ATTR_HAS_FOLIAGE: u32 = 4u;
const ATTR_HAS_CUTOUT: u32 = 8u;

const ATTR_HAS_GLASS: u32 = 16u;
const ATTR_HAS_EMITTER: u32 = 32u;

const TALL_GRASS_ID: u32 = 18u;

const GRASS_TALL_ID: u32 = 24u;
const REEDS_ID: u32 = 25u;
fn is_tuft(b: u32) -> bool {
    return b == TALL_GRASS_ID || b == GRASS_TALL_ID || b == REEDS_ID;
}

const NORMAL_PY: u32 = 3u;
const NORMAL_CROSS_A: u32 = 6u;
const NORMAL_CROSS_B: u32 = 7u;

fn mask_has(m: vec2<u32>, bit: u32) -> bool {
    if bit < 32u {
        return ((m.x >> bit) & 1u) != 0u;
    }
    return ((m.y >> (bit - 32u)) & 1u) != 0u;
}

fn mask_below(m: vec2<u32>, bit: u32) -> u32 {
    if bit < 32u {
        return countOneBits(m.x & ((1u << bit) - 1u));
    }
    if bit == 32u {
        return countOneBits(m.x);
    }
    return countOneBits(m.x) + countOneBits(m.y & ((1u << (bit - 32u)) - 1u));
}

fn cell_bit(c: vec3<u32>) -> u32 {
    return c.x | (c.y << 2u) | (c.z << 4u);
}

fn leaf_block(entry: u32, v: vec3<u32>) -> u32 {
    if (entry & UNIFORM_BIT) != 0u {
        return entry & 0xFFFFu;
    }
    let hdr = bricks[entry];
    let n = hdr & 0xFFu;
    let bits = (hdr >> 8u) & 0xFFu;
    let pal_words = (n + 1u) / 2u;
    let vb = cell_bit(v & vec3<u32>(3u));
    let bitpos = vb * bits;
    let w = bricks[entry + 1u + pal_words + (bitpos >> 5u)];
    let idx = (w >> (bitpos & 31u)) & ((1u << bits) - 1u);
    let pw = bricks[entry + 1u + idx / 2u];
    return (pw >> ((idx & 1u) * 16u)) & 0xFFFFu;
}

fn leaf_attr(ci: u32, v: vec3<u32>) -> u32 {
    let c = chunks[ci];
    let root_mask = vec2<u32>(c.root.x, c.root.y);
    let l1b = cell_bit(v >> vec3<u32>(4u));
    var l1: vec4<u32>;
    if (c.root.z & FULL_BIT) != 0u {
        l1 = vec4<u32>(0xFFFFFFFFu, 0xFFFFFFFFu, FULL_BIT, l1b * 64u);
    } else {
        l1 = inners[(c.root.z & PTR_MASK) + mask_below(root_mask, l1b)];
    }
    let lfb = cell_bit((v >> vec3<u32>(2u)) & vec3<u32>(3u));
    return bricks[c.attr_base + (l1.w & 0x0FFFFFFFu) + mask_below(vec2<u32>(l1.x, l1.y), lfb)];
}

fn block_at(ci: u32, v: vec3<u32>) -> u32 {
    let c = chunks[ci];
    if (c.attr_flags & ATTR_UNIFORM) != 0u {
        return c.attr_base;
    }
    return leaf_block(leaf_attr(ci, v), v);
}

fn ray_dir(px: f32, py: f32) -> vec3<f32> {

    let ndc_x = ((px + frame.jitter.x) / f32(frame.res.x)) * 2.0 - 1.0;
    let ndc_y = 1.0 - ((py + frame.jitter.y) / f32(frame.res.y)) * 2.0;
    var d = normalize(frame.cam_fwd
        + frame.cam_right * (ndc_x * frame.tan_half_fov * frame.aspect)
        + frame.cam_up * (ndc_y * frame.tan_half_fov));

    let tiny = abs(d) < vec3<f32>(1e-6);
    return select(d, vec3<f32>(1e-6, 1e-6, 1e-6) * sign(d + vec3<f32>(1e-9)), tiny);
}

fn dither(px: u32, py: u32) -> f32 {
    let p = vec2<f32>(f32(px) + 5.588238 * f32(frame.frame_index), f32(py));
    return fract(52.9829189 * fract(dot(p, vec2<f32>(0.06711056, 0.00583715))));
}

fn ray_skips(fade: f32, px: u32, py: u32) -> bool {
    if fade >= 1.0 {
        return false;
    }
    let d = dither(px, py);

    if fade >= 0.0 {
        return d >= fade;
    }
    return d < -fade;
}

fn normal_of(id: u32) -> vec3<f32> {
    if id >= NORMAL_CROSS_A {

        let s = select(-1.0, 1.0, id == NORMAL_CROSS_B);
        return vec3<f32>(0.70710678, 0.0, s * 0.70710678);
    }
    let axis = id >> 1u;
    let s = select(-1.0, 1.0, (id & 1u) == 1u);
    if axis == 0u { return vec3<f32>(s, 0.0, 0.0); }
    if axis == 1u { return vec3<f32>(0.0, s, 0.0); }
    return vec3<f32>(0.0, 0.0, s);
}

fn hash_alpha(p: vec3<f32>) -> f32 {
    let q = vec3<i32>(floor(p * 64.0));
    var h = (u32(q.x) * 73856093u) ^ (u32(q.y) * 19349663u) ^ (u32(q.z) * 83492791u);
    h = h ^ (h >> 13u);
    h = h * 1274126177u;
    return f32((h >> 8u) & 0xFFFFFFu) / 16777216.0;
}

struct CrossHit {
    hit: bool,
    t: f32,
    normal: u32,
};

const WIND_SWAY_T1: f32 = 1.9;
const WIND_SWAY_T2: f32 = 3.1;

const WIND_SWAY_AMP: f32 = 0.09;

fn wind_shear(wx: f32, wz: f32, time: f32) -> vec2<f32> {
    let g = 0.65 * sin(time * WIND_SWAY_T1 + wx * 0.11 + wz * 0.07)
        + 0.35 * sin(time * WIND_SWAY_T2 + wx * 0.173 + wz * 0.129 + 1.7);
    return vec2<f32>(g * 0.8, g * 0.6) * WIND_SWAY_AMP;
}

fn cross_quad(uc: vec3<u32>, ro: vec3<f32>, rd: vec3<f32>, layer: u32, vs: f32, shear: vec2<f32>) -> CrossHit {
    var r: CrossHit;
    r.hit = false;
    r.t = 0.0;
    r.normal = NORMAL_CROSS_A;

    let base = vec3<f32>(uc);

    var ts = vec2<f32>(-1.0, -1.0);
    if SPEC_WIND_SWAY {

        let dy = ro.y - base.y;
        let kA = shear.x - shear.y;
        let kB = shear.x + shear.y;
        let dA = rd.x - rd.z - kA * rd.y;
        if abs(dA) > 1e-6 {
            ts.x = ((base.x - base.z) + kA * dy - (ro.x - ro.z)) / dA;
        }
        let dB = rd.x + rd.z - kB * rd.y;
        if abs(dB) > 1e-6 {
            ts.y = ((base.x + base.z + 1.0) + kB * dy - (ro.x + ro.z)) / dB;
        }
    } else {
        let dA = rd.x - rd.z;
        if abs(dA) > 1e-6 {
            ts.x = ((base.x - base.z) - (ro.x - ro.z)) / dA;
        }
        let dB = rd.x + rd.z;
        if abs(dB) > 1e-6 {
            ts.y = ((base.x + base.z + 1.0) - (ro.x + ro.z)) / dB;
        }
    }
    var order = vec2<u32>(NORMAL_CROSS_A, NORMAL_CROSS_B);
    if ts.y < ts.x {
        ts = ts.yx;
        order = order.yx;
    }
    for (var i = 0u; i < 2u; i = i + 1u) {
        let tq = ts[i];
        if tq < 0.0 {
            continue;
        }
        let local = ro + rd * tq - base;

        if any(local < vec3<f32>(-1e-4)) || any(local > vec3<f32>(1.0 + 1e-4)) {
            continue;
        }
        let uv = vec2<f32>(local.x, 1.0 - local.y);

        let footprint = max(tq * vs, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y) * 16.0 / vs;
        let lod = clamp(log2(max(footprint, 1e-4)) + frame.mip_bias, 0.0, 3.0);

        let a = textureSampleLevel(atlas, atlas_samp, uv, i32(layer), lod).a;
        if a <= hash_alpha(ro + rd * tq) {
            continue;
        }
        r.hit = true;
        r.t = tq;
        r.normal = order[i];
        return r;
    }
    return r;
}

struct Hit {
    hit: bool,
    t: f32,
    normal: u32,
    voxel: vec3<u32>,

    micro: vec3<u32>,
    iters: u32,
};

const LEAVES_ID: u32 = 7u;
const PINE_LEAVES_ID: u32 = 16u;

const GRASS_ID: u32 = 3u;
const MEADOW_ID: u32 = 19u;

fn is_cutout_id(id: u32) -> bool {
    return id == LEAVES_ID || id == PINE_LEAVES_ID;
}

const MICRO: u32 = 4u;

const MICRO_SHIFT: u32 = 6u;
const VOXEL_MASK: u32 = 63u;

fn leaf_solid(wv: vec3<i32>, m: vec3<u32>) -> bool {
    let q = wv * i32(MICRO) + vec3<i32>(m);
    let h = hash_u32((u32(q.x) * 73856093u) ^ (u32(q.y) * 19349663u) ^ (u32(q.z) * 83492791u));
    return f32(h >> 8u) * (1.0 / 16777216.0) < SPEC_LEAF_FILL;
}

fn micro_entry(uc: vec3<u32>, ro: vec3<f32>, rd: vec3<f32>, t: f32, axis: u32,
               pos_dir: vec3<bool>, snap: bool) -> vec3<u32> {
    let p = (ro + rd * t - vec3<f32>(uc)) * f32(MICRO);
    var m = clamp(vec3<i32>(floor(p)), vec3<i32>(0), vec3<i32>(i32(MICRO) - 1));
    if snap {
        m[axis] = select(i32(MICRO) - 1, 0, pos_dir[axis]);
    }
    return vec3<u32>(m);
}

struct MicroHit {
    hit: bool,
    t: f32,
    normal: u32,
    micro: vec3<u32>,
};

fn cutout_march(wv: vec3<i32>, uc: vec3<u32>, ro: vec3<f32>, rd: vec3<f32>, t_in: f32,
                axis_in: u32, pos_dir: vec3<bool>, snap: bool) -> MicroHit {
    var r: MicroHit;
    r.hit = false;
    r.t = t_in;
    r.normal = 0u;
    r.micro = vec3<u32>(0u);
    let base = vec3<f32>(uc);
    let inv = 1.0 / rd;
    let ms = 1.0 / f32(MICRO);
    var m = vec3<i32>(micro_entry(uc, ro, rd, t_in, axis_in, pos_dir, snap));
    var t = t_in;
    var axis = axis_in;

    for (var i = 0u; i < 3u * MICRO; i = i + 1u) {
        if any(m < vec3<i32>(0)) || any(m >= vec3<i32>(i32(MICRO))) {
            return r;
        }
        if leaf_solid(wv, vec3<u32>(m)) {
            r.hit = true;
            r.t = t;
            r.normal = axis * 2u + select(1u, 0u, pos_dir[axis]);
            r.micro = vec3<u32>(m);
            return r;
        }
        let bound = vec3<f32>(select(m, m + vec3<i32>(1), pos_dir));
        let tn = (base + bound * ms - ro) * inv;
        if tn.x <= tn.y && tn.x <= tn.z {
            t = tn.x;
            axis = 0u;
        } else if tn.y <= tn.z {
            t = tn.y;
            axis = 1u;
        } else {
            t = tn.z;
            axis = 2u;
        }
        m[axis] += select(-1, 1, pos_dir[axis]);
    }
    return r;
}

fn march_chunk(ci: u32, ro_w: vec3<f32>, rd: vec3<f32>, t_limit: f32, skip_water: bool,
               skip_foliage: bool, skip_glass: bool, sun_ray: bool) -> Hit {
    var h: Hit;
    h.hit = false;
    h.iters = 0u;
    h.t = 0.0;
    h.normal = 0u;
    h.voxel = vec3<u32>(0u);
    h.micro = vec3<u32>(0u);
    let c = chunks[ci];
    let vs = c.voxel_size;
    let ro = (ro_w - c.origin) / vs;
    let rd_dda = select(rd, vec3<f32>(1e-6, 1e-6, 1e-6) * sign(rd + vec3<f32>(1e-9)), abs(rd) < vec3<f32>(1e-6));
    let inv = 1.0 / rd_dda;
    let bmin = (c.aabb_min - c.origin) / vs;
    let bmax = (c.aabb_max - c.origin) / vs;
    let t0 = (bmin - ro) * inv;
    let t1 = (bmax - ro) * inv;
    let tmin3 = min(t0, t1);
    let tmax3 = max(t0, t1);
    let t_enter = max(max(tmin3.x, tmin3.y), tmin3.z);
    let t_exit = min(min(tmax3.x, tmax3.y), tmax3.z);
    let t_end = min(t_exit, t_limit / vs);
    var t = max(t_enter, 0.0);
    if t >= t_end {
        return h;
    }
    var axis: u32 = 0u;
    if tmin3.x >= tmin3.y && tmin3.x >= tmin3.z {
        axis = 0u;
    } else if tmin3.y >= tmin3.z {
        axis = 1u;
    } else {
        axis = 2u;
    }
    let pos_dir = rd_dda > vec3<f32>(0.0);
    var pos = ro + rd * t;
    var cell = vec3<i32>(floor(pos));
    if t_enter > 0.0 {

        cell[axis] = select(i32(ceil(bmax[axis])) - 1, i32(floor(bmin[axis])), pos_dir[axis]);
    }
    cell = clamp(cell, vec3<i32>(0), vec3<i32>(63));
    let root_mask = vec2<u32>(c.root.x, c.root.y);
    let root_ptr = c.root.z & PTR_MASK;

    var water_aware = skip_water && (c.attr_flags & ATTR_HAS_WATER) != 0u;
    if water_aware && (c.attr_flags & ATTR_UNIFORM) != 0u {
        if c.attr_base == WATER_ID {
            return h;
        }
        water_aware = false;
    }

    let attr_uniform = (c.attr_flags & ATTR_UNIFORM) != 0u;

    var foliage_aware = false;
    if SPEC_FOLIAGE {
        foliage_aware = (c.attr_flags & ATTR_HAS_FOLIAGE) != 0u;
    }
    if foliage_aware && attr_uniform && !is_tuft(c.attr_base) {
        foliage_aware = false;
    }

    var cutout_aware = false;
    if SPEC_LEAF_CUTOUT {
        cutout_aware = (c.attr_flags & ATTR_HAS_CUTOUT) != 0u && vs == 1.0;
        if cutout_aware && attr_uniform
            && c.attr_base != LEAVES_ID && c.attr_base != PINE_LEAVES_ID {
            cutout_aware = false;
        }
    }

    var glass_aware = false;
    if SPEC_GLASS {
        glass_aware = skip_glass && (c.attr_flags & ATTR_HAS_GLASS) != 0u;
        if glass_aware && attr_uniform {

            if c.attr_base == GLASS_ID {
                return h;
            }
            glass_aware = false;
        }
    }
    var noncaster_aware = false;

    if SPEC_EMITTER_GATHER { noncaster_aware = sun_ray && (c.attr_flags & ATTR_HAS_EMITTER) != 0u; }
    if noncaster_aware && attr_uniform && !casts_sun_shadow(c.attr_base) { return h; }
    let water_interface = SPEC_WATER_REPAIR && !skip_water && (c.attr_flags & ATTR_HAS_WATER) != 0u;
    let attr_needed = (noncaster_aware && !attr_uniform) || (water_interface && !attr_uniform) || water_aware || glass_aware
        || ((foliage_aware || cutout_aware) && !attr_uniform);

    let foliage_trace = !skip_foliage && foliage_aware;

    var trav_mask = root_mask;
    if water_aware {
        trav_mask = c.dry_mask;
    }

    var root_full = (c.root.z & FULL_BIT) != 0u;
    if root_full && !SPEC_FULL_MARCH && !noncaster_aware && !water_interface {

        h.hit = true;
        h.t = t * vs;
        h.normal = axis * 2u + select(1u, 0u, pos_dir[axis]);
        h.voxel = vec3<u32>(cell);
        h.micro = micro_entry(vec3<u32>(cell), ro, rd, t, axis, pos_dir, t > 0.0);
        return h;
    }

    if trav_mask.x == 0u && trav_mask.y == 0u {
        return h;
    }
    var l1_cached: u32 = 0xFFFFFFFFu;
    var l1: vec4<u32> = vec4<u32>(0u);
    var leaf_cached: u32 = 0xFFFFFFFFu;
    var leaf: vec2<u32> = vec2<u32>(0u);

    var attr: u32 = 0u;
    loop {
        h.iters += 1u;
        if h.iters > MAX_ITERS {
            break;
        }
        if any(cell < vec3<i32>(0)) || any(cell > vec3<i32>(63)) {
            break;
        }
        let uc = vec3<u32>(cell);
        var skip: i32 = 16;
        var solid = false;
        let l1b = cell_bit(uc >> vec3<u32>(4u));
        if mask_has(trav_mask, l1b) {
            if l1b != l1_cached {
                l1_cached = l1b;
                if root_full {

                    l1 = vec4<u32>(0xFFFFFFFFu, 0xFFFFFFFFu, FULL_BIT, l1b * 64u | 0xF0000000u);
                } else {
                    l1 = inners[root_ptr + mask_below(root_mask, l1b)];
                }
                leaf_cached = 0xFFFFFFFFu;
            }

            var line_skip = false;
            if water_aware {
                let wl = l1.w >> 28u;
                let ly = f32(uc.y & 15u);
                if wl < 15u && ly > f32(wl) {

                    line_skip = rd.y >= 0.0;
                    if !line_skip {
                        let h = max(abs(rd.x), abs(rd.z));
                        if h > 1e-6 && (ly - f32(wl)) > 16.0 * (abs(rd.y) / h) {
                            line_skip = true;
                        }
                    }
                }
            }
            if line_skip {
                skip = 16;
            } else if (l1.z & FULL_BIT) != 0u {
                solid = true;
                if attr_needed {
                    let lfb = cell_bit((uc >> vec3<u32>(2u)) & vec3<u32>(3u));
                    if lfb != leaf_cached {
                        leaf_cached = lfb;
                        attr = bricks[c.attr_base + (l1.w & 0x0FFFFFFFu) + lfb];
                    }
                }
            } else {
                let l1m = vec2<u32>(l1.x, l1.y);
                let lfb = cell_bit((uc >> vec3<u32>(2u)) & vec3<u32>(3u));
                if mask_has(l1m, lfb) {
                    if lfb != leaf_cached {
                        leaf_cached = lfb;
                        let li = mask_below(l1m, lfb);
                        leaf = leaves[(l1.z & PTR_MASK) + li];
                        if attr_needed {
                            attr = bricks[c.attr_base + (l1.w & 0x0FFFFFFFu) + li];
                        }
                    }
                    let vb = cell_bit(uc & vec3<u32>(3u));
                    if mask_has(leaf, vb) {
                        solid = true;
                    } else {

                        let sh = vb & 42u;
                        var lo: u32;
                        if sh == 0u {
                            lo = leaf.x;
                        } else if sh < 32u {
                            lo = (leaf.x >> sh) | (leaf.y << (32u - sh));
                        } else {
                            lo = leaf.y >> (sh - 32u);
                        }
                        skip = select(1, 2, (lo & 0x00330033u) == 0u);
                    }
                } else {
                    skip = 4;
                }
            }
        } else {

            let sh = l1b & 42u;
            var rlo: u32;
            if sh == 0u {
                rlo = trav_mask.x;
            } else if sh < 32u {
                rlo = (trav_mask.x >> sh) | (trav_mask.y << (32u - sh));
            } else {
                rlo = trav_mask.y >> (sh - 32u);
            }
            skip = select(16, 32, (rlo & 0x00330033u) == 0u);
        }
        if solid && noncaster_aware {
            var id = c.attr_base;
            if !attr_uniform { id = leaf_block(attr, uc); }
            if !casts_sun_shadow(id) {
                solid = false;
                skip = 1;
            }
        }
        if solid && water_interface {
            var id = c.attr_base;
            if !attr_uniform { id = leaf_block(attr, uc); }
            if id == WATER_ID {
                let nid = axis * 2u + select(1u, 0u, pos_dir[axis]);
                let outward = normal_of(nid);
                let boundary = ro_w + rd * (t * vs);

                if world_block_at(boundary + outward * (0.002 * vs)) == WATER_ID {
                    solid = false;
                    skip = 1;
                }
            }
        }
        if solid && water_aware {
            if leaf_block(attr, uc) == WATER_ID {
                solid = false;

                skip = select(1, 4, (attr & UNIFORM_BIT) != 0u);
            }
        }

        if solid && glass_aware {
            if leaf_block(attr, uc) == GLASS_ID {
                solid = false;
                skip = select(1, 4, (attr & UNIFORM_BIT) != 0u);
            }
        }
        if solid && foliage_aware {
            var bid = c.attr_base;
            if !attr_uniform {
                bid = leaf_block(attr, uc);
            }
            if is_tuft(bid) {

                solid = false;
                skip = 1;
                if foliage_trace {

                    var sway = vec2<f32>(0.0, 0.0);
                    if SPEC_WIND_SWAY {
                        sway = wind_shear(
                            f32(c.origin.x) + f32(uc.x) * vs,
                            f32(c.origin.z) + f32(uc.z) * vs,
                            frame.time * frame.cloud_speed,
                        );
                    }
                    let q = cross_quad(uc, ro, rd, face_layer(block_faces[bid * 8u + NORMAL_CROSS_A]), vs, sway);
                    if q.hit {
                        h.hit = true;
                        h.t = q.t * vs;
                        h.normal = q.normal;
                        h.voxel = uc;
                        return h;
                    }
                }
            }
        }
        if solid && cutout_aware {
            var bid = c.attr_base;
            if !attr_uniform {
                bid = leaf_block(attr, uc);
            }
            if is_cutout_id(bid) {

                let wv = vec3<i32>(c.origin) + vec3<i32>(uc);
                let q = cutout_march(wv, uc, ro, rd, t, axis, pos_dir, t > 0.0);
                if q.hit {
                    h.hit = true;
                    h.t = q.t * vs;
                    h.normal = q.normal;
                    h.voxel = uc;
                    h.micro = q.micro;
                    return h;
                }

                solid = false;
                skip = 1;
            }
        }
        if solid {
            h.hit = true;
            h.t = t * vs;
            h.normal = axis * 2u + select(1u, 0u, pos_dir[axis]);
            h.voxel = uc;
            h.micro = micro_entry(uc, ro, rd, t, axis, pos_dir, t > 0.0);
            return h;
        }
        let bmin_i = cell - (cell & vec3<i32>(skip - 1));
        let bmax_i = bmin_i + vec3<i32>(skip);
        let bounds = select(vec3<f32>(bmin_i), vec3<f32>(bmax_i), pos_dir);
        let tn = (bounds - ro) * inv;
        if tn.x <= tn.y && tn.x <= tn.z {
            t = tn.x;
            axis = 0u;
        } else if tn.y <= tn.z {
            t = tn.y;
            axis = 1u;
        } else {
            t = tn.z;
            axis = 2u;
        }
        if t >= t_end {
            break;
        }
        pos = ro + rd * t;
        cell = vec3<i32>(floor(pos));
        cell[axis] = select(bmin_i[axis] - 1, bmax_i[axis], pos_dir[axis]);
    }
    return h;
}

fn voxel_occupied(ci: u32, v: vec3<i32>) -> bool {
    if any(v < vec3<i32>(0)) || any(v > vec3<i32>(63)) {
        return false;
    }
    let c = chunks[ci];
    let uv = vec3<u32>(v);
    let root_mask = vec2<u32>(c.root.x, c.root.y);
    let l1b = cell_bit(uv >> vec3<u32>(4u));
    if !mask_has(root_mask, l1b) {
        return false;
    }
    var occupied = false;
    if (c.root.z & FULL_BIT) != 0u {
        occupied = true;
    } else {
        let l1 = inners[(c.root.z & PTR_MASK) + mask_below(root_mask, l1b)];
        if (l1.z & FULL_BIT) != 0u {
            occupied = true;
        } else {
            let l1m = vec2<u32>(l1.x, l1.y);
            let lfb = cell_bit((uv >> vec3<u32>(2u)) & vec3<u32>(3u));
            if mask_has(l1m, lfb) {
                let leaf = leaves[(l1.z & PTR_MASK) + mask_below(l1m, lfb)];
                occupied = mask_has(leaf, cell_bit(uv & vec3<u32>(3u)));
            }
        }
    }
    return occupied;
}

fn voxel_occludes(ci: u32, v: vec3<i32>) -> bool {
    if any(v < vec3<i32>(0)) || any(v > vec3<i32>(63)) {
        return false;
    }
    let c = chunks[ci];
    let uv = vec3<u32>(v);
    let root_mask = vec2<u32>(c.root.x, c.root.y);
    let l1b = cell_bit(uv >> vec3<u32>(4u));
    if !mask_has(root_mask, l1b) {
        return false;
    }
    var occupied = false;
    if (c.root.z & FULL_BIT) != 0u {
        occupied = true;
    } else {
        let l1 = inners[(c.root.z & PTR_MASK) + mask_below(root_mask, l1b)];
        if (l1.z & FULL_BIT) != 0u {
            occupied = true;
        } else {
            let l1m = vec2<u32>(l1.x, l1.y);
            let lfb = cell_bit((uv >> vec3<u32>(2u)) & vec3<u32>(3u));
            if mask_has(l1m, lfb) {
                let leaf = leaves[(l1.z & PTR_MASK) + mask_below(l1m, lfb)];
                occupied = mask_has(leaf, cell_bit(uv & vec3<u32>(3u)));
            }
        }
    }

    if occupied && (c.attr_flags & (ATTR_HAS_WATER | ATTR_HAS_FOLIAGE)) != 0u {
        let b = block_at(ci, uv);
        return b != WATER_ID && !is_tuft(b);
    }
    return occupied;
}

const LIGHT_OPEN_SKY: u32 = 0xF000u;

fn light_sky(packed: u32) -> f32 {
    return f32(packed >> 12u);
}

fn light_block_rgb(packed: u32) -> vec3<f32> {
    return vec3<f32>(
        f32((packed >> 8u) & 0xFu),
        f32((packed >> 4u) & 0xFu),
        f32(packed & 0xFu),
    );
}

fn light_block_level(packed: u32) -> f32 {
    let c = light_block_rgb(packed);
    return max(c.r, max(c.g, c.b));
}

fn light_at(ci: u32, v: vec3<i32>) -> u32 {
    let c = chunks[ci];
    if (c.light_flags & 1u) != 0u {
        return c.light_base;
    }
    if any(v < vec3<i32>(0)) || any(v > vec3<i32>(63)) {
        return LIGHT_OPEN_SKY;
    }
    let uv = vec3<u32>(v);
    let cell = (uv.x >> 2u) | ((uv.y >> 2u) << 4u) | ((uv.z >> 2u) << 8u);
    let entry = bricks[c.light_base + cell];
    if (entry & UNIFORM_BIT) != 0u {
        return entry & 0xFFFFu;
    }
    let vb = cell_bit(uv & vec3<u32>(3u));
    let w = bricks[entry + (vb >> 1u)];
    return (w >> ((vb & 1u) * 16u)) & 0xFFFFu;
}

fn grid_lookup(p: vec3<f32>) -> u32 {
    let cc = vec3<i32>(floor(p / 64.0)) - frame.grid_min;
    if any(cc < vec3<i32>(0)) || any(cc >= vec3<i32>(frame.grid_dim)) {
        return NO_CHUNK;
    }
    let u = vec3<u32>(cc);
    return grid[u.x + u.y * frame.grid_dim.x + u.z * frame.grid_dim.x * frame.grid_dim.y];
}

fn world_sample(p: vec3<f32>) -> vec2<u32> {
    let ci = grid_lookup(p);
    if ci == NO_CHUNK {
        return vec2<u32>(LIGHT_OPEN_SKY, 0u);
    }
    let c = chunks[ci];
    let v = vec3<i32>(floor((p - c.origin) / c.voxel_size));
    return vec2<u32>(light_at(ci, v), select(0u, 1u, voxel_occludes(ci, v)));
}

fn gather_face(ci: u32, base: vec3<i32>, ta: vec3<i32>, tb: vec3<i32>,
               out: ptr<function, array<vec2<u32>, 9>>) {
    let c = chunks[ci];
    let root_mask = vec2<u32>(c.root.x, c.root.y);
    let root_full = (c.root.z & FULL_BIT) != 0u;
    let root_ptr = c.root.z & PTR_MASK;
    let light_uniform = (c.light_flags & 1u) != 0u;
    let has_water = (c.attr_flags & ATTR_HAS_WATER) != 0u;
    var l1_key: u32 = 0xFFFFFFFFu;
    var l1: vec4<u32> = vec4<u32>(0u);
    var leaf_key: u32 = 0xFFFFFFFFu;
    var leaf: vec2<u32> = vec2<u32>(0u);
    var brick_key: u32 = 0xFFFFFFFFu;
    var brick: u32 = 0u;
    for (var i = 0u; i < 9u; i = i + 1u) {
        let uv = vec3<u32>(base + ta * (i32(i % 3u) - 1) + tb * (i32(i / 3u) - 1));

        var solid = false;
        let l1b = cell_bit(uv >> vec3<u32>(4u));
        if mask_has(root_mask, l1b) {
            if root_full {
                solid = true;
            } else {
                if l1b != l1_key {
                    l1_key = l1b;
                    l1 = inners[root_ptr + mask_below(root_mask, l1b)];
                    leaf_key = 0xFFFFFFFFu;
                }
                let l1m = vec2<u32>(l1.x, l1.y);
                let lfb = cell_bit((uv >> vec3<u32>(2u)) & vec3<u32>(3u));
                if (l1.z & FULL_BIT) != 0u {
                    solid = true;
                } else if mask_has(l1m, lfb) {
                    if lfb != leaf_key {
                        leaf_key = lfb;
                        leaf = leaves[(l1.z & PTR_MASK) + mask_below(l1m, lfb)];
                    }
                    solid = mask_has(leaf, cell_bit(uv & vec3<u32>(3u)));
                }
            }
        }

        if solid && has_water {
            solid = block_at(ci, uv) != WATER_ID;
        }

        var packed = c.light_base;
        if !light_uniform {
            let cell = (uv.x >> 2u) | ((uv.y >> 2u) << 4u) | ((uv.z >> 2u) << 8u);
            if cell != brick_key {
                brick_key = cell;
                brick = bricks[c.light_base + cell];
            }
            if (brick & UNIFORM_BIT) != 0u {
                packed = brick & 0xFFFFu;
            } else {
                let vb = cell_bit(uv & vec3<u32>(3u));
                packed = (bricks[brick + (vb >> 1u)] >> ((vb & 1u) * 16u)) & 0xFFFFu;
            }
        }
        (*out)[i] = vec2<u32>(packed, select(0u, 1u, solid));
    }
}

fn shadow_ray(ro: vec3<f32>, rd: vec3<f32>, max_dist: f32) -> bool {
    var r_ro = ro;
    var r_dist = max_dist;

    if SPEC_WATER_REPAIR && rd.y > 1e-4 && ro.y < frame.sea_level {
        let t_surf = (frame.sea_level - ro.y) / rd.y;
        if t_surf >= r_dist {
            return false;
        }
        r_ro = ro + rd * t_surf;
        r_dist = r_dist - t_surf;
    }
    let rd_dda = select(rd, vec3<f32>(1e-6, 1e-6, 1e-6) * sign(rd + vec3<f32>(1e-9)), abs(rd) < vec3<f32>(1e-6));
    let inv = 1.0 / rd_dda;
    let pos_dir = rd_dda > vec3<f32>(0.0);
    var cell = vec3<i32>(floor(r_ro / 64.0));
    var t = 0.0;
    var last: u32 = NO_CHUNK;
    for (var i = 0u; i < 24u; i = i + 1u) {
        let rel = cell - frame.grid_min;
        if any(rel < vec3<i32>(0)) || any(rel >= vec3<i32>(frame.grid_dim)) {
            return false;
        }
        let u = vec3<u32>(rel);
        let ci = grid[u.x + u.y * frame.grid_dim.x + u.z * frame.grid_dim.x * frame.grid_dim.y];
        if ci != NO_CHUNK && ci != last {
            last = ci;

            if march_chunk(ci, r_ro, rd, r_dist, true, true, true, true).hit {
                return true;
            }
        }

        let bounds = vec3<f32>(select(cell, cell + vec3<i32>(1), pos_dir)) * 64.0;
        let tn = (bounds - r_ro) * inv;
        var axis = 0u;
        if tn.x <= tn.y && tn.x <= tn.z {
            t = tn.x;
        } else if tn.y <= tn.z {
            t = tn.y;
            axis = 1u;
        } else {
            t = tn.z;
            axis = 2u;
        }
        if t >= r_dist {
            return false;
        }
        cell[axis] += select(-1, 1, pos_dir[axis]);
    }
    return false;
}

override SOFT_PENUMBRA: f32 = 1.5;

override SOFT_TAPS: u32 = 3u;

const SOFT_MISS_DIST: f32 = 64.0;

fn shadow_ray_t(ro: vec3<f32>, rd: vec3<f32>, max_dist: f32) -> f32 {
    let rd_dda = select(rd, vec3<f32>(1e-6, 1e-6, 1e-6) * sign(rd + vec3<f32>(1e-9)), abs(rd) < vec3<f32>(1e-6));
    let inv = 1.0 / rd_dda;
    let pos_dir = rd_dda > vec3<f32>(0.0);
    var cell = vec3<i32>(floor(ro / 64.0));
    var t = 0.0;
    var last: u32 = NO_CHUNK;
    for (var i = 0u; i < 24u; i = i + 1u) {
        let rel = cell - frame.grid_min;
        if any(rel < vec3<i32>(0)) || any(rel >= vec3<i32>(frame.grid_dim)) {
            return -1.0;
        }
        let u = vec3<u32>(rel);
        let ci = grid[u.x + u.y * frame.grid_dim.x + u.z * frame.grid_dim.x * frame.grid_dim.y];
        if ci != NO_CHUNK && ci != last {
            last = ci;

            let m = march_chunk(ci, ro, rd, max_dist, true, true, true, true);
            if m.hit {
                return m.t;
            }
        }
        let bounds = vec3<f32>(select(cell, cell + vec3<i32>(1), pos_dir)) * 64.0;
        let tn = (bounds - ro) * inv;
        var axis = 0u;
        if tn.x <= tn.y && tn.x <= tn.z {
            t = tn.x;
        } else if tn.y <= tn.z {
            t = tn.y;
            axis = 1u;
        } else {
            t = tn.z;
            axis = 2u;
        }
        if t >= max_dist {
            return -1.0;
        }
        cell[axis] += select(-1, 1, pos_dir[axis]);
    }
    return -1.0;
}

struct WorldHit {
    hit: bool,
    t: f32,
    normal: u32,
    voxel: vec3<u32>,
    ci: u32,
};

fn trace_world(ro: vec3<f32>, rd: vec3<f32>, max_dist: f32, skip_water: bool) -> WorldHit {
    var out: WorldHit;
    out.hit = false;
    out.t = max_dist;
    out.normal = 0u;
    out.voxel = vec3<u32>(0u);
    out.ci = NO_CHUNK;
    let tiny = abs(rd) < vec3<f32>(1e-6);
    let dir = select(rd, vec3<f32>(1e-6, 1e-6, 1e-6) * sign(rd + vec3<f32>(1e-9)), tiny);
    let inv = 1.0 / dir;
    let pos_dir = dir > vec3<f32>(0.0);
    var cell = vec3<i32>(floor(ro / 64.0));
    for (var i = 0u; i < 16u; i = i + 1u) {
        let rel = cell - frame.grid_min;
        if any(rel < vec3<i32>(0)) || any(rel >= vec3<i32>(frame.grid_dim)) {
            return out;
        }
        let u = vec3<u32>(rel);
        let ci = grid[u.x + u.y * frame.grid_dim.x + u.z * frame.grid_dim.x * frame.grid_dim.y];
        let bounds = vec3<f32>(select(cell, cell + vec3<i32>(1), pos_dir)) * 64.0;
        let tn = (bounds - ro) * inv;
        var t_exit = tn.x;
        var axis = 0u;
        if tn.y < t_exit {
            t_exit = tn.y;
            axis = 1u;
        }
        if tn.z < t_exit {
            t_exit = tn.z;
            axis = 2u;
        }
        if ci != NO_CHUNK {
            let h = march_chunk(ci, ro, rd, min(t_exit, max_dist), skip_water, false, true, false);
            if h.hit {
                out.hit = true;
                out.t = h.t;
                out.normal = h.normal;
                out.voxel = h.voxel;
                out.ci = ci;
                return out;
            }
        }
        if t_exit >= max_dist {
            return out;
        }
        cell[axis] += select(-1, 1, pos_dir[axis]);
    }
    return out;
}

const WATER_EXT: vec3<f32> = vec3<f32>(0.280, 0.055, 0.020);
const WATER_IOR: f32 = 1.333;

const SURFACE_EPS: f32 = 0.02;

const WATER_F0: f32 = 0.02;

fn water_medium(c: vec3<f32>, d: f32, body: vec3<f32>) -> vec3<f32> {
    let tr = exp(-WATER_EXT * frame.water_absorb * max(d, 0.0));
    return c * tr + body * (vec3<f32>(1.0) - tr);
}

fn schlick(cos_i: f32) -> f32 {
    let m = clamp(1.0 - cos_i, 0.0, 1.0);
    let m2 = m * m;
    return WATER_F0 + (1.0 - WATER_F0) * m2 * m2 * m;
}

const GLASS_F0: f32 = 0.0426;

fn schlick_glass(cos_i: f32) -> f32 {
    let m = clamp(1.0 - cos_i, 0.0, 1.0);
    let m2 = m * m;
    return GLASS_F0 + (1.0 - GLASS_F0) * m2 * m2 * m;
}

const GROUND_F0: f32 = 0.04;

const GROUND_ROUGHNESS: f32 = 0.9;

fn schlick_ground(cos_i: f32) -> f32 {
    let m = clamp(1.0 - cos_i, 0.0, 1.0);
    let m2 = m * m;

    let top = max(1.0 - GROUND_ROUGHNESS, GROUND_F0);
    return GROUND_F0 + (top - GROUND_F0) * m2 * m2 * m;
}

fn water_path(t: f32, rd: vec3<f32>) -> f32 {
    if rd.y <= 0.0 {
        return t;
    }
    return clamp((frame.sea_level - frame.cam_pos.y) / rd.y, 0.0, t);
}

fn hit_t(ci: u32, v: vec3<u32>, micro: vec3<u32>, normal_id: u32, rd: vec3<f32>) -> f32 {
    let c = chunks[ci];
    let box_min = c.origin + vec3<f32>(v) * c.voxel_size;
    if normal_id >= NORMAL_CROSS_A {

        if normal_id == NORMAL_CROSS_A {
            return ((box_min.x - box_min.z) - (frame.cam_pos.x - frame.cam_pos.z))
                / (rd.x - rd.z);
        }
        return ((box_min.x + box_min.z + c.voxel_size) - (frame.cam_pos.x + frame.cam_pos.z))
            / (rd.x + rd.z);
    }
    let axis = normal_id >> 1u;

    let ms = c.voxel_size / f32(MICRO);
    let plane = box_min[axis]
        + f32(micro[axis] + select(0u, 1u, (normal_id & 1u) == 1u)) * ms;
    return (plane - frame.cam_pos[axis]) / rd[axis];
}

fn depth_key(t: f32) -> u32 {
    return u32(clamp(1.0 - t / frame.far, 0.0, 1.0) * 16777215.0);
}

fn make_key(t: f32, normal: u32, ci: u32, v: vec3<u32>, micro: vec3<u32>) -> u64 {
    let d = depth_key(t);

    let b = v | (micro << vec3<u32>(MICRO_SHIFT));
    return (u64(d) << 40u) | (u64(normal) << 37u) | (u64(ci) << 24u)
        | u64((b.x << 16u) | (b.y << 8u) | b.z);
}

fn transmittance_from(y0: f32, s: f32, rd: vec3<f32>) -> f32 {
    let lambda = frame.fog_falloff;
    let x = clamp(lambda * s * rd.y, -60.0, 60.0);
    var path = s;
    if abs(x) > 1e-4 {
        path = s * (1.0 - exp(-x)) / x;
    }
    let rho = frame.fog_density * exp(-clamp(lambda * (y0 - frame.fog_height), -60.0, 60.0));
    return exp(-clamp(rho * path, 0.0, 60.0));
}

fn transmittance(s: f32, rd: vec3<f32>) -> f32 {
    return transmittance_from(frame.cam_pos.y, s, rd);
}

const SCATTER_TINT: vec3<f32> = vec3<f32>(1.0000, 0.4770, 0.1474);

const SKY_NIGHT_HORIZON: vec3<f32> = vec3<f32>(0.0049, 0.0060, 0.0134);
const SKY_DAY_HORIZON: vec3<f32> = vec3<f32>(0.4480, 0.6210, 0.8900);
const SKY_NIGHT_ZENITH: vec3<f32> = vec3<f32>(0.0008, 0.0015, 0.0039);
const SKY_DAY_ZENITH: vec3<f32> = vec3<f32>(0.0637, 0.2140, 0.8276);

const SKY_COOL_DAY_HORIZON: vec3<f32> = vec3<f32>(0.4074, 0.6057, 0.9150);
const SKY_COOL_DAY_ZENITH: vec3<f32> = vec3<f32>(0.0385, 0.1892, 0.8600);

const SKY_GRAD_EXP: f32 = 0.55;

fn hg_gain(cos_t: f32, g: f32) -> f32 {
    let g2 = g * g;
    let d = 1.0 + g2 - 2.0 * g * cos_t;
    return (1.0 - g2) / pow(max(d, 1e-4), 1.5);
}

fn scatter_lobe(rd: vec3<f32>) -> vec3<f32> {
    let sd = dot(rd, frame.sun_dir);
    return SCATTER_TINT * (hg_gain(sd, frame.fog_g) * frame.fog_scatter * frame.daylight);
}

fn sky_base(rd: vec3<f32>, shaft: f32) -> vec3<f32> {
    let day = frame.daylight;
    let up = clamp(rd.y, 0.0, 1.0);
    let horizon = mix(SKY_NIGHT_HORIZON,
                      select(SKY_DAY_HORIZON, SKY_COOL_DAY_HORIZON, SPEC_SKY_COOL), day);
    let zenith = mix(SKY_NIGHT_ZENITH,
                     select(SKY_DAY_ZENITH, SKY_COOL_DAY_ZENITH, SPEC_SKY_COOL), day);
    var c = mix(horizon, zenith, pow(up, SKY_GRAD_EXP));
    let sd = dot(rd, frame.sun_dir);
    c += vec3<f32>(1.0, 0.8276, 0.5225) * smoothstep(0.9975, 0.9990, sd) * max(day, 0.2);

    c += scatter_lobe(rd) * shaft;
    if SPEC_SKY_LOOK && frame.haze_warm > 0.0 {

        let band = pow(1.0 - up, 3.0);
        let toward = 0.35 + 0.65 * pow(max(sd, 0.0), 4.0);
        c += SCATTER_TINT * (band * toward * frame.haze_warm * day * 0.22);
    }
    if SPEC_SKY_LOOK && frame.zenith_deep > 0.0 {

        let z = pow(up, SKY_GRAD_EXP);
        c *= mix(vec3<f32>(1.0), vec3<f32>(0.80, 0.90, 1.16), frame.zenith_deep * z * day);
    }
    c = mix(c, horizon * 0.26, smoothstep(0.0, -0.35, rd.y));
    return c;
}

const CLOUD_OCTAVES: u32 = 4u;

const CLOUD_SOFT: f32 = 0.14;
const CLOUD_DEEP: f32 = 0.55;

const CLOUD_PATCH_SPAN: f32 = 6.0;

const CLOUD_WIND: vec2<f32> = vec2<f32>(0.94, 0.34);

const CLOUD_FADE: f32 = 3.0e4;
const CLOUD_MAX: f32 = 1.0e5;

const CLOUD_LIT: vec3<f32> = vec3<f32>(1.0000, 0.9575, 0.8963);
const CLOUD_BASE: vec3<f32> = vec3<f32>(0.4854, 0.5312, 0.6462);

const CLOUD_COOL_LIT: vec3<f32> = vec3<f32>(1.0000, 1.0000, 1.0000);
const CLOUD_COOL_BASE: vec3<f32> = vec3<f32>(0.4420, 0.4880, 0.6010);

const CLOUD_NIGHT: vec3<f32> = vec3<f32>(0.0021, 0.0026, 0.0052);

const CLOUD_WARMTH: f32 = 0.72;

const CLOUD_G: f32 = 0.82;
const CLOUD_SCATTER: f32 = 0.055;

fn hash_u32(x: u32) -> u32 {
    var h = x;
    h ^= h >> 16u;
    h *= 0x7feb352du;
    h ^= h >> 15u;
    h *= 0x846ca68bu;
    h ^= h >> 16u;
    return h;
}

fn cloud_lattice(c: vec2<i32>) -> f32 {
    let h = hash_u32(bitcast<u32>(c.x) ^ (hash_u32(bitcast<u32>(c.y)) * 0x9e3779b9u));
    return f32(h >> 8u) * (1.0 / 16777216.0);
}

fn cloud_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = p - i;

    let w = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let c = vec2<i32>(i);
    let a = cloud_lattice(c);
    let b = cloud_lattice(c + vec2<i32>(1, 0));
    let d = cloud_lattice(c + vec2<i32>(0, 1));
    let e = cloud_lattice(c + vec2<i32>(1, 1));
    return mix(mix(a, b, w.x), mix(d, e, w.x), w.y);
}

fn cloud_fbm(p: vec2<f32>, fp: f32) -> f32 {
    var freq = 1.0;
    var amp = 1.0;
    var sum = 0.0;
    var norm = 0.0;
    for (var i = 0u; i < CLOUD_OCTAVES; i = i + 1u) {
        let w = amp * (1.0 - smoothstep(0.25, 0.5, fp * freq));
        if w > 0.0 {
            sum += w * cloud_noise(p * freq);
            norm += w;
        }
        freq *= 2.0;
        amp *= 0.5;
    }
    if norm <= 0.0 {
        return 0.5;
    }
    return sum / norm;
}

fn cloud_radiance(dens: f32, depth: f32, rd: vec3<f32>) -> vec3<f32> {
    let day = frame.daylight;
    let thick = select(depth, 0.0, rd.y < 0.0);

    let warm = 1.0 - smoothstep(0.02, 0.30, frame.sun_dir.y);
    let sun_tint = mix(vec3<f32>(1.0), SCATTER_TINT, warm * CLOUD_WARMTH);
    let day_c = mix(select(CLOUD_LIT, CLOUD_COOL_LIT, SPEC_SKY_COOL) * sun_tint,
                    select(CLOUD_BASE, CLOUD_COOL_BASE, SPEC_SKY_COOL), thick);

    let glow = SCATTER_TINT
        * (hg_gain(dot(rd, frame.sun_dir), CLOUD_G) * CLOUD_SCATTER * day * (1.0 - dens));
    return mix(CLOUD_NIGHT, day_c, day) + glow;
}

fn sky_color(ro: vec3<f32>, rd: vec3<f32>, travelled: f32, shaft: f32) -> vec3<f32> {
    let base = sky_base(rd, shaft);
    if frame.cloud_cover <= 0.0 {
        return base;
    }

    let dy = frame.cloud_height - ro.y;
    if dy * rd.y <= 0.0 {
        return base;
    }
    let t = dy / rd.y;
    if t > CLOUD_MAX {
        return base;
    }
    let p = ro.xz + rd.xz * t;

    let px_angle = 2.0 * frame.tan_half_fov / f32(frame.res.y) * exp2(frame.mip_bias);
    let fp = (travelled + t) * px_angle / (abs(rd.y) * frame.cloud_scale);

    let drift = CLOUD_WIND * (frame.time * frame.cloud_speed);
    let n = cloud_fbm((p + drift) / frame.cloud_scale, fp);
    var thr = 1.0 - frame.cloud_cover;
    if SPEC_SKY_LOOK && frame.cloud_patch > 0.0 {

        let m = cloud_fbm(
            (p + drift) / (frame.cloud_scale * CLOUD_PATCH_SPAN),
            fp / CLOUD_PATCH_SPAN
        );
        thr = clamp(thr + (m - 0.5) * frame.cloud_patch, 0.0, 0.99);
    }
    let dens = smoothstep(thr, thr + CLOUD_SOFT, n);
    if dens <= 0.0 {
        return base;
    }
    var depth = smoothstep(thr, thr + CLOUD_DEEP, n);
    if SPEC_SKY_LOOK && frame.cloud_relief > 0.0 {

        let az = normalize(frame.sun_dir.xz + vec2<f32>(1.0e-3, 0.0));
        let step = (8.0 + frame.sun_dir.y * 48.0) / frame.cloud_scale;
        let ns = cloud_fbm((p + drift) / frame.cloud_scale + az * step, fp);
        depth = clamp(depth + (ns - n) * (2.2 * frame.cloud_relief), 0.0, 1.0);
    }

    let reach = 1.0 - smoothstep(CLOUD_FADE, CLOUD_MAX, t);
    let a = dens * reach * transmittance_from(ro.y, t, rd);
    return mix(base, cloud_radiance(dens, depth, rd), a);
}

const GODRAY_CUT_LO: f32 = 0.0040;
const GODRAY_CUT_HI: f32 = 0.0120;

const SUN_ANGLE: f32 = 0.0093;

fn cloud_shade(p: vec3<f32>, fp: f32) -> f32 {
    if frame.cloud_shadow <= 0.0 || frame.cloud_cover <= 0.0 {
        return 1.0;
    }

    let dy = frame.cloud_height - p.y;
    if dy * frame.sun_dir.y <= 0.0 {
        return 1.0;
    }
    let t = dy / frame.sun_dir.y;
    if t > CLOUD_MAX {
        return 1.0;
    }
    let q = p.xz + frame.sun_dir.xz * t;
    let drift = CLOUD_WIND * (frame.time * frame.cloud_speed);
    let n = cloud_fbm((q + drift) / frame.cloud_scale, max(fp, t * SUN_ANGLE) / frame.cloud_scale);
    let thr = 1.0 - frame.cloud_cover;
    return 1.0 - smoothstep(thr, thr + CLOUD_SOFT, n) * frame.cloud_shadow;
}

fn shaft_sample(base: u32, t: vec2<f32>) -> f32 {
    let hi = f32(SHAFT_DIM) - 1.0;
    let c = clamp(t, vec2<f32>(0.0), vec2<f32>(hi));
    let f0 = floor(c);
    let fr = c - f0;
    let i0 = vec2<u32>(f0);
    let i1 = min(i0 + vec2<u32>(1u), vec2<u32>(u32(hi)));
    let a = shaft[base + i0.y * SHAFT_DIM + i0.x];
    let b = shaft[base + i0.y * SHAFT_DIM + i1.x];
    let c0 = shaft[base + i1.y * SHAFT_DIM + i0.x];
    let d = shaft[base + i1.y * SHAFT_DIM + i1.x];
    return mix(mix(a, b, fr.x), mix(c0, d, fr.x), fr.y);
}

fn terrain_shade(p: vec3<f32>) -> f32 {

    let t = (p.xz - frame.shaft_origin) / frame.shaft_texel - 0.5;
    let e = shaft_sample(SHAFT_RESULT, t);
    return smoothstep(e - frame.shaft_soft, e + frame.shaft_soft, p.y);
}

fn terrain_shade_beyond(p: vec3<f32>, d: f32) -> f32 {
    return terrain_shade(p + frame.sun_dir * d);
}

fn sun_shaft(ro: vec3<f32>, rd: vec3<f32>, dist: f32, alpha: f32, px: u32, py: u32) -> f32 {

    let terrain = SPEC_TERRAIN_SHAFTS && (frame.flags & FLAG_TERRAIN_SHAFTS) != 0u;
    let deck = frame.cloud_shadow > 0.0 && frame.cloud_cover > 0.0;
    if frame.godray_strength <= 0.0 {
        return 1.0;
    }
    if !deck && !terrain {
        return 1.0;
    }
    let lobe = scatter_lobe(rd);
    let imp = smoothstep(GODRAY_CUT_LO, GODRAY_CUT_HI,
                         max(max(lobe.r, lobe.g), lobe.b) * alpha);
    if imp <= 0.0 {
        return 1.0;
    }
    let d = min(dist, frame.godray_dist);
    if d <= 0.0 {
        return 1.0;
    }

    let opac = 1.0 - transmittance_from(ro.y, d, rd);
    if opac <= 1e-6 {
        return 1.0;
    }
    let lambda = frame.fog_falloff;
    let rho0 = frame.fog_density
        * exp(-clamp(lambda * (ro.y - frame.fog_height), -60.0, 60.0));
    let k = lambda * rd.y;
    let off = dither(px, py);

    let px_angle = 2.0 * frame.tan_half_fov / f32(frame.res.y) * exp2(frame.mip_bias);
    var sum = 0.0;

    for (var i = 0u; i < frame.godray_steps; i = i + 1u) {
        let u = (f32(i) + off) / f32(frame.godray_steps);
        let path = -log(1.0 - u * opac) / rho0;
        var s = path;
        let kp = k * path;
        if abs(kp) > 1e-4 {
            s = -log(max(1.0 - kp, 1e-6)) / k;
        }
        s = clamp(s, 0.0, d);
        let p = ro + rd * s;
        var v = cloud_shade(p, s * px_angle);
        if terrain {
            v = v * terrain_shade(p);
        }
        sum += v;
    }
    let vis = sum / f32(frame.godray_steps);

    var covered = 1.0;
    if d < dist {
        let full = 1.0 - transmittance_from(ro.y, dist, rd);
        if full > 1e-6 {
            covered = min(opac / full, 1.0);
        }
    }

    return max(mix(1.0, vis, imp * frame.godray_strength * covered), 0.0);
}

override SPEC_COMPACT_SHADE_HIT: bool = false;

override SPEC_ISOLATE_GLASS: bool = false;
override SPEC_SHADOW_PASS: bool = true;

override RESOLVE_LANE: u32 = 0u;

@group(3) @binding(0) var primary_sun: texture_2d<f32>;
