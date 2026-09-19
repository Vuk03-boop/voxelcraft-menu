pub mod blit;
pub mod font;
pub mod pools;
pub mod timer;
pub mod ui;
mod shadow;

use crate::biome;
use crate::block;
use crate::lod::RenderItem;
use crate::probe::{self, ProbeData};
use crate::math::{Aabb, Frustum};
use crate::textures;
use crate::voxel::{AttrRef, ChunkKey, LightRef, World};
use bytemuck::{Pod, Zeroable};
use glam::{IVec3, Mat4, Vec2, Vec3};
use pools::{DynBuffer, GpuMirror};
use timer::{q, Readback};

pub const TILE: u32 = 8;
pub const MAX_CHUNKS: usize = 8191;

pub const MAX_PAIRS: u32 = 1 << 20;

const PAIRS_PER_TILE: u64 = 16;

pub fn pairs_for(tiles: (u32, u32)) -> u32 {
    let want = u64::from(tiles.0) * u64::from(tiles.1) * PAIRS_PER_TILE;
    want.max(u64::from(MAX_PAIRS)).min(u64::from(u32::MAX) / 2) as u32
}

pub const TILE_ID_BITS: u32 = 32 - 13;
pub const MAX_TILES: u32 = 1 << TILE_ID_BITS;

pub const MARCH_MAX_ITERS: u32 = 512;

pub const TRUNCATION_MARK: &str = "pair list truncated";

pub const OVERSIZE_MARK: &str = "tile index overflows the pair word";

#[derive(Clone, Copy, Debug)]
pub struct PairDemand {

    pub select: u32,

    pub deferred: u32,

    pub recover: u32,
    pub cap: u32,
}

impl PairDemand {
    pub fn overflowed(&self) -> bool {
        self.select > self.cap || self.deferred > self.cap || self.recover > self.cap
    }

    pub fn worst(&self) -> u32 {
        self.select.max(self.deferred).max(self.recover)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MarchStats {
    pub pixels: u64,
    pub iters: u64,

    pub worst: u32,

    pub touched: u64,

    pub capped: u64,

    pub top_1: f64,
    pub top_10: f64,
    pub p50: u32,
    pub p90: u32,
    pub p99: u32,
    pub pairs: Option<PairDemand>,
    pub chunks: usize,
}

pub const GRID_X: u32 = 32;
pub const GRID_Y: u32 = 8;
pub const GRID_Z: u32 = 32;
const GRID_LEN: usize = (GRID_X * GRID_Y * GRID_Z) as usize;
const NO_CHUNK: u32 = 0xFFFF_FFFF;

pub fn grid_origin(cam: Vec3) -> IVec3 {
    let cc = (cam / 64.0).floor().as_ivec3();
    IVec3::new(cc.x - GRID_X as i32 / 2, 0, cc.z - GRID_Z as i32 / 2)
}

pub fn grid_span(key: ChunkKey, gmin: IVec3) -> Option<(IVec3, IVec3)> {
    let span = 1i32 << key.lod;
    let base = key.pos * span - gmin;
    let lo = base.max(IVec3::ZERO);
    let hi = (base + IVec3::splat(span)).min(IVec3::new(
        GRID_X as i32,
        GRID_Y as i32,
        GRID_Z as i32,
    ));
    if lo.cmplt(hi).all() {
        Some((lo, hi))
    } else {
        None
    }
}

pub fn partition_for_grid(
    items: &[(RenderItem, Aabb)],
    cam: Vec3,
    frustum: &Frustum,
    gmin: IVec3,
    offscreen_shadows: bool,
) -> (Vec<RenderItem>, Vec<RenderItem>) {
    let mut visible: Vec<(f32, RenderItem)> = Vec::with_capacity(items.len());
    let mut offscreen: Vec<(f32, RenderItem)> = Vec::new();
    for &(item, aabb) in items {
        if frustum.contains_aabb(&aabb) {
            visible.push((aabb.distance_to(cam), item));
        } else if offscreen_shadows && grid_span(item.key, gmin).is_some() {
            offscreen.push((aabb.distance_to(cam), item));
        }
    }

    let by_distance = |a: &(f32, RenderItem), b: &(f32, RenderItem)| {
        a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal)
    };
    visible.sort_by(by_distance);
    visible.truncate(MAX_CHUNKS);

    offscreen.sort_by(by_distance);
    offscreen.truncate(MAX_CHUNKS - visible.len());
    (
        visible.into_iter().map(|(_, it)| it).collect(),
        offscreen.into_iter().map(|(_, it)| it).collect(),
    )
}

const NEAR: f32 = 0.05;

const TAA_FEEDBACK: f32 = 0.15;

pub const FLAG_HIZ: u32 = 1;
pub const FLAG_HEATMAP: u32 = 2;
pub const FLAG_SHADOWS: u32 = 4;
pub const FLAG_AO: u32 = 8;

const _: () = assert!(block::WATER == 13);

const _: () = assert!(block::TALL_GRASS == 18);

const _: () = assert!(block::GRASS_TALL == 24);
const _: () = assert!(block::REEDS == 25);

const _: () = assert!(block::LEAVES == 7);
const _: () = assert!(block::PINE_LEAVES == 16);
const _: () = assert!(block::BLOCKS[block::LEAVES as usize].cutout);
const _: () = assert!(block::BLOCKS[block::PINE_LEAVES as usize].cutout);

const _: () = assert!(block::FACES_PER_BLOCK == 8);

pub const FLAG_WATER_REFLECT: u32 = 16;
pub const FLAG_WATER_REFRACT: u32 = 32;

pub const FLAG_UNDERWATER: u32 = 64;

pub const FLAG_TINT: u32 = 128;

pub const FLAG_FOLIAGE: u32 = 256;

const _: () = {
    let mut i = 0;
    while i < block::HOTBAR.len() {
        assert!(block::HOTBAR[i] != block::TALL_GRASS);
        i += 1;
    }
};

pub const FLAG_WAVES: u32 = 512;

pub const FLAG_WAVE_ANISO: u32 = 1024;

pub const FLAG_WAVE_SHOAL: u32 = 2048;

pub const FLAG_WAVE_FILL: u32 = 4096;

pub const FLAG_TERRAIN_SHAFTS: u32 = 8192;

pub const FLAG_TEX_VARIATION: u32 = 16384;

pub const FLAG_DISTANT_SHADOWS: u32 = 32768;

pub const FLAG_LEAF_CUTOUT: u32 = 65536;

pub const FLAG_GLASS: u32 = 131072;

pub const FLAG_WATER_SHADOW_CUT: u32 = 262144;

pub const FLAG_LEAF_THIN: u32 = 524288;

pub const FLAG_FLAT_SECONDARY: u32 = 1048576;

pub const FLAG_WATER_FAR: u32 = 2097152;

pub const FLAG_WATER_DARK: u32 = 4194304;

pub const FLAG_SNELL: u32 = 8388608;

pub const FLAG_HI_SHORE_WET: u32 = 8;

pub const FLAG_HI_SHORE_FOAM: u32 = 16;

pub const FLAG_HI_WATER_SEC: u32 = 32;

pub const FLAG_HI_CAUSTICS: u32 = 64;

pub const FLAG_HI_GLASS_REFLECT: u32 = 128;

pub const FLAG_HI_SNELL_BEND: u32 = 256;

pub const FLAG_HI_TINT_BALANCE: u32 = 512;

pub const FLAG_HI_SKY_LOOK: u32 = 1024;

pub const FLAG_HI_SKY_COOL: u32 = 2048;

pub const FLAG_HI_WATER_LOOK: u32 = 4096;

pub const FLAG_HI_FOLIAGE_RICH: u32 = 8192;

pub const FLAG_HI_CANOPY_RELIEF: u32 = 16384;

pub const FLAG_HI_WIND_SWAY: u32 = 32768;

pub const FLAG_HI_SOFT_SHADOWS: u32 = 65536;

pub const FLAG_HI_COMPACT_SHADE_HIT: u32 = 131072;
pub const FLAG_HI_ISOLATE_GLASS: u32 = 262144;
pub const FLAG_HI_SHADOW_PASS: u32 = 524288;
pub const FLAG_HI_LEGACY_LIGHTING: u32 = 1048576;
pub const FLAG_HI_LEGACY_WATER: u32 = 2097152;
pub const FLAG_HI_SOFT_HQ: u32 = 4194304;
pub const FLAG_HI_DEBUG_A: u32 = 8388608;
pub const FLAG_HI_DEBUG_B: u32 = 16777216;
pub const FLAG_HI_DEBUG_C: u32 = 33554432;
pub const FLAG_HI_SUN_NARROW: u32 = 67108864;
pub const FLAG_HI_SUN_WIDE: u32 = 134217728;

pub const FLAG_PROBE_TAP: u32 = 16777216;

pub const FLAG_PROBE_AMBIENT: u32 = 33554432;

pub const FLAG_PROBE_CUBE: u32 = 67108864;

pub const FLAG_PROBE_BOUNCE: u32 = 134217728;

pub const FLAG_LIGHT_RGB: u32 = 268435456;

pub const FLAG_PROBE_SUN: u32 = 536870912;

pub const FLAG_PROBE_SUN_HIGH: u32 = 1073741824;

pub const FLAG_SKY_TINT: u32 = 2147483648;

pub const FLAG_HI_SKY_SPECULAR: u32 = 1;

pub const FLAG_HI_FULL_MARCH: u32 = 2;

pub const FLAG_HI_EMITTER_WORLD: u32 = 4;

pub const SPEC_HI_MASK: u32 = FLAG_HI_SKY_SPECULAR
    | FLAG_HI_FULL_MARCH
    | FLAG_HI_EMITTER_WORLD
    | FLAG_HI_SHORE_WET
    | FLAG_HI_SHORE_FOAM
    | FLAG_HI_WATER_SEC
    | FLAG_HI_CAUSTICS
    | FLAG_HI_GLASS_REFLECT
    | FLAG_HI_SNELL_BEND
    | FLAG_HI_TINT_BALANCE
    | FLAG_HI_SKY_LOOK
    | FLAG_HI_SKY_COOL
    | FLAG_HI_WATER_LOOK
    | FLAG_HI_FOLIAGE_RICH
    | FLAG_HI_CANOPY_RELIEF
    | FLAG_HI_WIND_SWAY
    | FLAG_HI_SOFT_SHADOWS
    | FLAG_HI_COMPACT_SHADE_HIT
    | FLAG_HI_ISOLATE_GLASS
    | FLAG_HI_SHADOW_PASS
    | FLAG_HI_LEGACY_LIGHTING
    | FLAG_HI_LEGACY_WATER
    | FLAG_HI_SOFT_HQ
    | FLAG_HI_DEBUG_A
    | FLAG_HI_DEBUG_B
    | FLAG_HI_DEBUG_C
    | FLAG_HI_SUN_NARROW
    | FLAG_HI_SUN_WIDE;
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_SKY_SPECULAR != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_FULL_MARCH != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_EMITTER_WORLD != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_SHORE_WET != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_SHORE_FOAM != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_WATER_SEC != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_CAUSTICS != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_GLASS_REFLECT != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_SNELL_BEND != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_TINT_BALANCE != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_SKY_LOOK != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_SKY_COOL != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_WATER_LOOK != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_FOLIAGE_RICH != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_CANOPY_RELIEF != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_WIND_SWAY != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_SOFT_SHADOWS != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_COMPACT_SHADE_HIT != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_ISOLATE_GLASS != 0);
const _: () = assert!(SPEC_HI_MASK & FLAG_HI_SHADOW_PASS != 0);

pub const LEAF_FILL: f32 = 0.62;

pub const LEAF_FILL_PRE43: f32 = 0.72;

pub const WATER_SHADOW_DIST: f32 = 16.0;

pub const WATER_SHADOW_DIST_PRE41: f32 = 48.0;

const _: () = assert!(block::GLASS == 10);

pub const SPEC_MASK: u32 = FLAG_TINT
    | FLAG_FOLIAGE
    | FLAG_WAVES
    | FLAG_WAVE_ANISO
    | FLAG_WAVE_SHOAL
    | FLAG_WAVE_FILL
    | FLAG_TERRAIN_SHAFTS
    | FLAG_TEX_VARIATION
    | FLAG_DISTANT_SHADOWS
    | FLAG_LEAF_CUTOUT
    | FLAG_GLASS
    | FLAG_WATER_SHADOW_CUT
    | FLAG_LEAF_THIN
    | FLAG_FLAT_SECONDARY
    | FLAG_PROBE_TAP
    | FLAG_PROBE_AMBIENT
    | FLAG_PROBE_CUBE
    | FLAG_PROBE_BOUNCE
    | FLAG_LIGHT_RGB
    | FLAG_PROBE_SUN
    | FLAG_PROBE_SUN_HIGH
    | FLAG_SKY_TINT;

const _: () = assert!(SPEC_MASK & FLAG_WATER_SHADOW_CUT != 0);
const _: () = assert!(WATER_SHADOW_DIST < WATER_SHADOW_DIST_PRE41);
const _: () = assert!(WATER_SHADOW_DIST_PRE41 == 48.0);
const _: () = assert!(SPEC_MASK & FLAG_LEAF_THIN != 0);
const _: () = assert!(SPEC_MASK & FLAG_FLAT_SECONDARY != 0);

const _: () = assert!(SPEC_MASK & FLAG_PROBE_TAP != 0);

const _: () = assert!(SPEC_MASK & FLAG_PROBE_AMBIENT != 0);

const _: () = assert!(SPEC_MASK & FLAG_PROBE_CUBE != 0);

const _: () = assert!(SPEC_MASK & FLAG_PROBE_BOUNCE != 0);
const _: () = assert!(SPEC_MASK & FLAG_PROBE_SUN != 0);
const _: () = assert!(SPEC_MASK & FLAG_PROBE_SUN_HIGH != 0);

const _: () = assert!(SPEC_MASK & FLAG_SKY_TINT != 0);

const _: () = assert!(FLAG_SKY_TINT == 1u32 << 31);

const _: () = assert!(SPEC_MASK & FLAG_LIGHT_RGB != 0);

const _: () = assert!(block::BLOCKS[block::GLOWSTONE as usize].light[0] == 15);
const _: () = assert!(block::GLOWSTONE == 11);

const _: () = assert!(SPEC_MASK & FLAG_WATER_FAR == 0);

const _: () = assert!(SPEC_MASK & FLAG_WATER_DARK == 0);

const _: () = assert!(SPEC_MASK & FLAG_SNELL == 0);

const _: () = assert!(crate::block::SAND as u32 == 4);
const _: () = assert!(LEAF_FILL < LEAF_FILL_PRE43);
const _: () = assert!(LEAF_FILL_PRE43 == 0.72);

const _: () = assert!(biome::BAND == 0.22 && biome::BLEND == 0.18);
const _: () = assert!(biome::TEMP_SEED == 7 && biome::HUMID_SEED == 8);
#[rustfmt::skip]
const _: () = assert!(
    biome::GRID[0][0] == biome::TUNDRA && biome::GRID[0][1] == biome::PLAINS && biome::GRID[0][2] == biome::DESERT &&
    biome::GRID[1][0] == biome::TAIGA  && biome::GRID[1][1] == biome::PLAINS && biome::GRID[1][2] == biome::SAVANNA &&
    biome::GRID[2][0] == biome::TAIGA  && biome::GRID[2][1] == biome::FOREST && biome::GRID[2][2] == biome::FOREST
);

#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable)]
struct GpuFrame {
    cam_pos: [f32; 3],
    time: f32,
    cam_fwd: [f32; 3],
    tan_half_fov: f32,
    cam_right: [f32; 3],
    aspect: f32,
    cam_up: [f32; 3],
    far: f32,
    sun_dir: [f32; 3],
    daylight: f32,
    res: [u32; 2],
    tiles: [u32; 2],
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
    grid_min: [i32; 3],
    sea_level: f32,
    grid_dim: [u32; 3],
    water_absorb: f32,
    jitter: [f32; 2],
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
    shaft_origin: [f32; 2],
    shaft_texel: f32,
    shaft_soft: f32,
    cloud_patch: f32,
    cloud_relief: f32,
    haze_warm: f32,
    zenith_deep: f32,
    prev_view_proj: [[f32; 4]; 4],
    world_epoch: u32,
    epoch_pad: [u32; 3],
}

const _: () = assert!(std::mem::size_of::<GpuFrame>() == 368);

const _: () = assert!(std::mem::offset_of!(GpuFrame, cloud_cover) == 192);
const _: () = assert!(std::mem::offset_of!(GpuFrame, godray_strength) == 208);
const _: () = assert!(std::mem::offset_of!(GpuFrame, tint_strength) == 224);
const _: () = assert!(std::mem::offset_of!(GpuFrame, wave_amp) == 240);
const _: () = assert!(std::mem::offset_of!(GpuFrame, shaft_origin) == 256);
const _: () = assert!(std::mem::offset_of!(GpuFrame, cloud_patch) == 272);
const _: () = assert!(std::mem::offset_of!(GpuFrame, prev_view_proj) == 288);
const _: () = assert!(std::mem::offset_of!(GpuFrame, world_epoch) == 352);

#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable)]
struct GpuChunk {
    aabb_min: [f32; 3],
    voxel_size: f32,
    aabb_max: [f32; 3],
    lod: u32,
    origin: [f32; 3],

    fade: f32,
    root: [u32; 4],
    attr_base: u32,

    attr_flags: u32,
    light_base: u32,
    light_flags: u32,

    dry_mask: [u32; 2],

    _reserved: [u32; 2],
}

#[derive(Clone, Copy, Debug)]
pub struct Fog {

    pub density: f32,

    pub falloff: f32,

    pub scatter: f32,

    pub g: f32,

    pub height: f32,
}

impl Default for Fog {
    fn default() -> Self {
        Self {
            density: 5.0e-4,
            falloff: 1.0 / 64.0,
            scatter: 0.020,
            g: 0.80,
            height: crate::worldgen::SEA_LEVEL as f32,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Clouds {

    pub cover: f32,

    pub height: f32,

    pub scale: f32,

    pub speed: f32,

    pub patch: f32,

    pub relief: f32,
}

impl Default for Clouds {
    fn default() -> Self {
        Self {
            cover: 0.45,
            height: 448.0,
            scale: 320.0,
            speed: 3.0,
            patch: 0.0,
            relief: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SkyLook {

    pub haze_warm: f32,

    pub zenith_deep: f32,
}

impl Default for SkyLook {
    fn default() -> Self {
        Self {
            haze_warm: 0.0,
            zenith_deep: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GodRays {

    pub strength: f32,

    pub steps: u32,

    pub dist: f32,

    pub cloud_shadow: f32,
}

impl Default for GodRays {
    fn default() -> Self {
        Self {
            strength: 1.0,
            steps: 4,
            dist: 4096.0,
            cloud_shadow: 0.85,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Tint {

    pub strength: f32,

    pub seed: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct Waves {

    pub amp: f32,

    pub scale: f32,

    pub speed: f32,

    pub reflect_slope: f32,
}

impl Default for Waves {
    fn default() -> Self {
        Self {
            amp: 0.12,
            scale: 32.0,
            speed: 0.32,
            reflect_slope: 0.0,
        }
    }
}

pub struct FrameParams {
    pub cam_pos: Vec3,
    pub cam_fwd: Vec3,
    pub cam_right: Vec3,
    pub cam_up: Vec3,
    pub fov_y: f32,
    pub far: f32,
    pub sun_dir: Vec3,
    pub daylight: f32,
    pub time: f32,
    pub flags: u32,

    pub spec_hi: u32,
    pub ambient: f32,
    pub shadow_dist: f32,
    pub world_epoch: u32,
    pub exp_shadow_repro: bool,
    pub fog: Fog,

    pub clouds: Clouds,

    pub sky: SkyLook,

    pub godrays: GodRays,

    pub sea_level: f32,

    pub water_absorb: f32,

    pub jitter: Option<Vec2>,

    pub taa: bool,

    pub mip_bias: f32,

    pub tint: Tint,

    pub waves: Waves,

    pub shaft_origin: [f32; 2],

    pub shaft_texel: f32,

    pub shaft_scan: bool,
}

pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter: wgpu::Adapter,
    pub has_timestamps: bool,
}

pub fn make_instance() -> wgpu::Instance {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::VULKAN | wgpu::Backends::DX12;
    wgpu::Instance::new(desc)
}

pub async fn init_gpu(
    instance: &wgpu::Instance,
    compatible: Option<&wgpu::Surface<'static>>,
) -> anyhow_lite::Result<Gpu> {
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: compatible,
            force_fallback_adapter: false,
            ..Default::default()
        })
        .await
        .map_err(|e| format!("no suitable GPU adapter: {e}"))?;

    let required = wgpu::Features::SHADER_INT64 | wgpu::Features::SHADER_INT64_ATOMIC_MIN_MAX | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
    let available = adapter.features();
    if !available.contains(required) {
        return Err(format!(
            "GPU {} lacks required native texture-format features or 64-bit shader atomics (SHADER_INT64 | SHADER_INT64_ATOMIC_MIN_MAX), which the \
             visibility buffer needs",
            adapter.get_info().name
        )
        .into());
    }
    for format in [wgpu::TextureFormat::R16Float,wgpu::TextureFormat::Rg16Float,wgpu::TextureFormat::R8Unorm] {
        let uses=wgpu::TextureUsages::STORAGE_BINDING|wgpu::TextureUsages::TEXTURE_BINDING;
        if !adapter.get_texture_format_features(format).allowed_usages.contains(uses) {
            return Err(format!("GPU lacks sampled/storage support for shadow format {format:?}").into());
        }
    }
    let has_timestamps = available.contains(wgpu::Features::TIMESTAMP_QUERY);
    let features = required | (available & wgpu::Features::TIMESTAMP_QUERY);
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("voxelcraft"),
            required_features: features,
            required_limits: adapter.limits(),
            ..Default::default()
        })
        .await
        .map_err(|e| format!("device request failed: {e}"))?;
    Ok(Gpu {
        device,
        queue,
        adapter,
        has_timestamps,
    })
}

pub mod anyhow_lite {
    pub type Error = Box<dyn std::error::Error + Send + Sync>;
    pub type Result<T> = std::result::Result<T, Error>;
}

struct Pipelines {
    tile_select: wgpu::ComputePipeline,
    recover_select: wgpu::ComputePipeline,
    finalize: wgpu::ComputePipeline,
    finalize_recover: wgpu::ComputePipeline,
    finalize_deferred: wgpu::ComputePipeline,
    build_hiz: wgpu::ComputePipeline,
    taa: wgpu::ComputePipeline,
}

struct SpecPipes {
    primary_shadow: Option<wgpu::ComputePipeline>,
    shadow_blurs: [wgpu::ComputePipeline;2],
    glass_resolve: Option<wgpu::ComputePipeline>,
    water_resolve: Option<wgpu::ComputePipeline>,
    march: wgpu::ComputePipeline,
    resolve: wgpu::ComputePipeline,

    water_sec: wgpu::ComputePipeline,
}

pub struct Renderer {
    pub gpu: Gpu,
    layout: wgpu::BindGroupLayout,
    pipes: Pipelines,

    module: wgpu::ShaderModule,
    pipeline_layout: wgpu::PipelineLayout,

    spec_pipes: Vec<((u32, u32), SpecPipes)>,
    bind_group: wgpu::BindGroup,

    taa_bind_group: [wgpu::BindGroup; 2],
    bind_dirty: bool,

    frame_buf: wgpu::Buffer,
    chunk_buf: DynBuffer,
    grid_buf: wgpu::Buffer,
    inners: GpuMirror,
    leaves: GpuMirror,
    bricks: GpuMirror,
    faces: wgpu::Buffer,
    pairs: DynBuffer,

    max_pairs: u32,
    counters: wgpu::Buffer,
    indirect_storage: wgpu::Buffer,
    indirect: wgpu::Buffer,
    vis: DynBuffer,
    hiz: DynBuffer,
    dbg: DynBuffer,

    shaft_buf: wgpu::Buffer,

    shaft_pipes: Vec<wgpu::ComputePipeline>,

    out_tex: wgpu::Texture,
    out_view: wgpu::TextureView,

    pub water_sec_shift: u32,

    water_sec_view: (wgpu::TextureView, wgpu::TextureView),
    water_sec_tex: (wgpu::Texture, wgpu::Texture),

    water_read_group: wgpu::BindGroup,
    water_store_group: wgpu::BindGroup,
    water_read_layout: wgpu::BindGroupLayout,
    water_store_layout: wgpu::BindGroupLayout,

    water_empty_group: wgpu::BindGroup,
    shadow_targets: shadow::Targets,
    shadow_layouts: [wgpu::BindGroupLayout;3],
    shadow_pipe_layouts: [wgpu::PipelineLayout;3],
    split_layout: wgpu::BindGroupLayout,

    water_read_pipe_layout: wgpu::PipelineLayout,
    water_sec_pipe_layout: wgpu::PipelineLayout,

    hist_view: [wgpu::TextureView; 2],
    atlas_view: wgpu::TextureView,

    probe_view: wgpu::TextureView,

    probe_tex: wgpu::Texture,

    probe_sampler: wgpu::Sampler,

    probe_pinned: bool,
    atlas_sampler: wgpu::Sampler,
    linear_sampler: wgpu::Sampler,

    pub blit: blit::BlitPass,
    pub ui: ui::UiRenderer,
    pub timer: Readback,

    pub size: (u32, u32),

    pub surface_size: (u32, u32),
    tiles: (u32, u32),
    surface_format: wgpu::TextureFormat,
    grid_min: IVec3,

    chunk_scratch: Vec<GpuChunk>,
    grid_scratch: Vec<u32>,

    shadow_share: bool,

    offscreen_shadows: bool,

    pub last_chunk_count: usize,

    pub last_grid_chunks: usize,
    pub last_camera_chunk: Option<usize>,

    prev_view_proj: Option<Mat4>,
    shadow_sig: Option<u64>,

    frame_index: u64,

    history_valid: bool,

    last_blit: usize,
}

impl Renderer {

    pub fn new(
        gpu: Gpu,
        size: (u32, u32),
        surface_format: wgpu::TextureFormat,
        _water_mottle: f32,
        tint_balance: bool,
        probe_fill: Option<f32>,
        probe_noise: bool,
        lod: crate::lod::LodConfig,

        water_sec_shift: u32,

        tone: u32,

        grade: u32,

        grade_strength: f32,
    ) -> Self {
        let device = &gpu.device;
        let queue = &gpu.queue;
        let size = (size.0.max(8), size.1.max(8));
        let tiles = (size.0.div_ceil(TILE), size.1.div_ceil(TILE));

        let source = shader_source();
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("voxel passes"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });

        let layout = make_layout(device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("voxel layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });

        let wread_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("water sec read"),
            entries: &[sampled_entry(0), sampled_entry(1)],
        });
        let wstore_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("water sec store"),
            entries: &[storage_tex_entry(0), storage_tex_entry(1)],
        });
        let wempty_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("water sec group-1 placeholder"),
            entries: &[],
        });
        let split_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("primary sun mask R8"), entries: &[shadow::sampled(0)],
        });
        let shadow_layouts=shadow::layouts(device);
        let shadow_pipe_layouts=std::array::from_fn(|i|device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label:Some("shadow stage pipeline layout"),
            bind_group_layouts:&[Some(&layout),Some(&wempty_layout),Some(&wempty_layout),Some(&shadow_layouts[i])],
            immediate_size:0,
        }));
        let shadow_targets=shadow::Targets::new(device,(1,1),&split_layout,&shadow_layouts);
        let water_read_pipe_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("resolve layout"),
                bind_group_layouts: &[Some(&layout), Some(&wread_layout), Some(&wempty_layout), Some(&split_layout)],
                immediate_size: 0,
            });
        let water_sec_pipe_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("water sec layout"),
                bind_group_layouts: &[Some(&layout), Some(&wempty_layout), Some(&wstore_layout)],
                immediate_size: 0,
            });
        let wempty_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("water sec group-1 placeholder"),
            layout: &wempty_layout,
            entries: &[],
        });
        let mk = |name: &'static str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(name),
                layout: Some(&pipeline_layout),
                module: &module,
                entry_point: Some(name),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let pipes = Pipelines {
            tile_select: mk("tile_select"),
            recover_select: mk("recover_select"),
            finalize: mk("finalize"),
            finalize_recover: mk("finalize_recover"),
            finalize_deferred: mk("finalize_deferred"),
            build_hiz: mk("build_hiz"),
            taa: mk("taa"),
        };

        let shaft_pipes: Vec<wgpu::ComputePipeline> = (0..crate::shaft::STEPS)
            .map(|k| {
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("shaft scan"),
                    layout: Some(&pipeline_layout),
                    module: &module,
                    entry_point: Some("shaft_scan"),
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants: &[("SHAFT_STEP", k as f64)],
                        ..Default::default()
                    },
                    cache: None,
                })
            })
            .collect();

        let spec_pipes = Vec::new();

        use wgpu::BufferUsages as U;
        let frame_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame"),
            size: std::mem::size_of::<GpuFrame>() as u64,
            usage: U::UNIFORM | U::COPY_DST,
            mapped_at_creation: false,
        });
        let chunk_buf = DynBuffer::new(
            device,
            "chunk list",
            (MAX_CHUNKS * std::mem::size_of::<GpuChunk>()) as u64,
            U::STORAGE,
        );
        let grid_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("chunk grid"),
            size: (GRID_LEN * 4) as u64,
            usage: U::STORAGE | U::COPY_DST,
            mapped_at_creation: false,
        });
        let inners = GpuMirror::new(device, "inner nodes", U::STORAGE);
        let leaves = GpuMirror::new(device, "leaf masks", U::STORAGE);
        let bricks = GpuMirror::new(device, "bricks", U::STORAGE);

        let face_table = block::face_table();
        let faces = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("block faces"),
            size: (face_table.len() * 4) as u64,
            usage: U::STORAGE | U::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&faces, 0, bytemuck::cast_slice(&face_table));

        let max_pairs = pairs_for((size.0.div_ceil(TILE), size.1.div_ceil(TILE)));
        let pairs = DynBuffer::new(device, "pairs", (max_pairs as u64) * 2 * 4, U::STORAGE);
        let counters = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("counters"),
            size: 64,
            usage: U::STORAGE | U::COPY_DST | U::COPY_SRC,
            mapped_at_creation: false,
        });

        let indirect_storage = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("indirect storage"),
            size: 32,
            usage: U::STORAGE | U::COPY_SRC | U::COPY_DST,
            mapped_at_creation: false,
        });

        let indirect = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("indirect"),
            size: 32,
            usage: U::INDIRECT | U::COPY_DST,
            mapped_at_creation: false,
        });
        let vis = DynBuffer::new(
            device,
            "visibility",
            (size.0 as u64) * (size.1 as u64) * 8,
            U::STORAGE,
        );
        let hiz = DynBuffer::new(
            device,
            "hi-z",
            (tiles.0 as u64) * (tiles.1 as u64) * 4,
            U::STORAGE,
        );

        let dbg = DynBuffer::new(
            device,
            "debug",
            (size.0 as u64) * (size.1 as u64) * 4,
            U::STORAGE | U::COPY_SRC,
        );

        let shaft_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("light envelope"),
            size: (crate::shaft::BUFFER_FLOATS * 4) as u64,
            usage: U::STORAGE | U::COPY_DST,
            mapped_at_creation: false,
        });

        let (out_tex, out_view) = make_hdr_texture(device, size, "trace output");
        let hist_view = [
            make_hdr_texture(device, size, "taa history 0").1,
            make_hdr_texture(device, size, "taa history 1").1,
        ];

        let span = 1u32 << water_sec_shift;
        let ws = (size.0.div_ceil(span), size.1.div_ceil(span));
        let water_sec_tex = (
            make_hdr_texture(device, ws, "water sec refr").0,
            make_hdr_texture(device, ws, "water sec refl").0,
        );
        let water_sec_view = (
            water_sec_tex.0.create_view(&Default::default()),
            water_sec_tex.1.create_view(&Default::default()),
        );
        let water_read_group =
            make_water_sec_group(device, &wread_layout, (&water_sec_view.0, &water_sec_view.1), "water sec read");
        let water_store_group = make_water_sec_group(
            device,
            &wstore_layout,
            (&water_sec_view.0, &water_sec_view.1),
            "water sec store",
        );
        let (atlas_view, atlas_sampler) = make_atlas(device, queue, 0.0, tint_balance);
        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("history sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let (probe_tex, probe_view) = make_probe(device, queue, probe_fill, probe_noise);
        let probe_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("probe sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::Repeat,
            ..Default::default()
        });

        let bind_group = make_bind_group(
            device,
            &layout,
            &frame_buf,
            &chunk_buf,
            &inners,
            &leaves,
            &bricks,
            &faces,
            &pairs.buf,
            &counters,
            &indirect_storage,
            &vis,
            &hiz,
            &out_view,
            &atlas_view,
            &atlas_sampler,
            &dbg,
            &grid_buf,
            (&hist_view[0], &hist_view[1]),
            &linear_sampler,
            &shaft_buf,
            &probe_view,
            &probe_sampler,
        );
        let taa_bind_group = std::array::from_fn(|i| {
            make_bind_group(
                device,
                &layout,
                &frame_buf,
                &chunk_buf,
                &inners,
                &leaves,
                &bricks,
                &faces,
                &pairs.buf,
                &counters,
                &indirect_storage,
                &vis,
                &hiz,
                &hist_view[i],
                &atlas_view,
                &atlas_sampler,
                &dbg,
                &grid_buf,
                (&hist_view[i ^ 1], &out_view),
                &linear_sampler,
                &shaft_buf,
                &probe_view,
                &probe_sampler,
            )
        });
        let blit = blit::BlitPass::new(
            device,
            queue,
            surface_format,
            tone,
            grade,
            grade_strength,
            &[&out_view, &hist_view[0], &hist_view[1]],
        );
        let ui = ui::UiRenderer::new(device, queue, surface_format);
        let timer = Readback::new(device, queue, gpu.has_timestamps);

        Self {
            gpu,
            layout,
            pipes,
            module,
            pipeline_layout,
            spec_pipes,
            bind_group,
            taa_bind_group,
            bind_dirty: false,
            frame_buf,
            chunk_buf,
            grid_buf,
            inners,
            leaves,
            bricks,
            faces,
            pairs,
            max_pairs,
            counters,
            indirect_storage,
            indirect,
            vis,
            hiz,
            dbg,
            shaft_buf,
            shaft_pipes,
            out_tex,
            out_view,
            water_sec_shift,
            water_sec_view,
            water_sec_tex,
            water_read_group,
            water_store_group,
            water_read_layout: wread_layout,
            water_store_layout: wstore_layout,
            water_empty_group: wempty_group,
            shadow_targets, shadow_layouts, shadow_pipe_layouts, split_layout,
            water_read_pipe_layout,
            water_sec_pipe_layout,
            hist_view,
            atlas_view,
            probe_view,
            probe_tex,
            probe_sampler,
            probe_pinned: probe_fill.is_some(),
            atlas_sampler,
            linear_sampler,
            blit,
            ui,
            timer,
            size,
            surface_size: size,
            tiles,
            surface_format,
            grid_min: IVec3::ZERO,
            prev_view_proj: None,
            shadow_sig: None,
            frame_index: 0,
            history_valid: false,
            last_blit: 0,
            chunk_scratch: Vec::with_capacity(MAX_CHUNKS),
            grid_scratch: vec![NO_CHUNK; GRID_LEN],
            shadow_share: lod.shadow_share,
            offscreen_shadows: lod.offscreen_shadows,
            last_chunk_count: 0,
            last_grid_chunks: 0,
            last_camera_chunk: None,
        }
    }

    pub fn invalidate_history(&mut self) {
        self.history_valid = false;
        self.prev_view_proj = None;
    }

    pub fn set_presentation(&self, tone: u32, grade: u32, strength: f32) {
        self.blit.set_presentation(&self.gpu.queue, self.surface_format, tone, grade, strength);
    }

    pub fn resize(&mut self, size: (u32, u32), surface: (u32, u32)) {
        self.surface_size = (surface.0.max(1), surface.1.max(1));
        let size = (size.0.max(8), size.1.max(8));
        if size == self.size {
            return;
        }
        self.size = size;
        self.tiles = (size.0.div_ceil(TILE), size.1.div_ceil(TILE));
        let device = &self.gpu.device;
        self.vis
            .resize(device, (size.0 as u64) * (size.1 as u64) * 8);
        self.dbg
            .resize(device, (size.0 as u64) * (size.1 as u64) * 4);
        self.hiz
            .resize(device, (self.tiles.0 as u64) * (self.tiles.1 as u64) * 4);

        self.max_pairs = pairs_for(self.tiles);
        self.pairs
            .resize(device, (self.max_pairs as u64) * 2 * 4);
        let (t, v) = make_hdr_texture(device, size, "trace output");
        self.out_tex = t;
        self.out_view = v;
        self.hist_view = [
            make_hdr_texture(device, size, "taa history 0").1,
            make_hdr_texture(device, size, "taa history 1").1,
        ];
        let span = 1u32 << self.water_sec_shift;
        let ws = (size.0.div_ceil(span), size.1.div_ceil(span));
        self.water_sec_tex = (
            make_hdr_texture(device, ws, "water sec refr").0,
            make_hdr_texture(device, ws, "water sec refl").0,
        );
        self.water_sec_view = (
            self.water_sec_tex.0.create_view(&Default::default()),
            self.water_sec_tex.1.create_view(&Default::default()),
        );
        self.blit.set_sources(
            device,
            &[&self.out_view, &self.hist_view[0], &self.hist_view[1]],
        );

        self.history_valid = false;
        self.bind_dirty = true;
    }

    fn rebuild_bind_group(&mut self) {
        self.bind_group = make_bind_group(
            &self.gpu.device,
            &self.layout,
            &self.frame_buf,
            &self.chunk_buf,
            &self.inners,
            &self.leaves,
            &self.bricks,
            &self.faces,
            &self.pairs.buf,
            &self.counters,
            &self.indirect_storage,
            &self.vis,
            &self.hiz,
            &self.out_view,
            &self.atlas_view,
            &self.atlas_sampler,
            &self.dbg,
            &self.grid_buf,
            (&self.hist_view[0], &self.hist_view[1]),
            &self.linear_sampler,
            &self.shaft_buf,
            &self.probe_view,
            &self.probe_sampler,
        );

        self.water_read_group = make_water_sec_group(
            &self.gpu.device,
            &self.water_read_layout,
            (&self.water_sec_view.0, &self.water_sec_view.1),
            "water sec read",
        );
        self.water_store_group = make_water_sec_group(
            &self.gpu.device,
            &self.water_store_layout,
            (&self.water_sec_view.0, &self.water_sec_view.1),
            "water sec store",
        );
        self.taa_bind_group = std::array::from_fn(|i| {
            make_bind_group(
                &self.gpu.device,
                &self.layout,
                &self.frame_buf,
                &self.chunk_buf,
                &self.inners,
                &self.leaves,
                &self.bricks,
                &self.faces,
                &self.pairs.buf,
                &self.counters,
                &self.indirect_storage,
                &self.vis,
                &self.hiz,
                &self.hist_view[i],
                &self.atlas_view,
                &self.atlas_sampler,
                &self.dbg,
                &self.grid_buf,
                (&self.hist_view[i ^ 1], &self.out_view),
                &self.linear_sampler,
                &self.shaft_buf,
                &self.probe_view,
                &self.probe_sampler,
            )
        });
        self.bind_dirty = false;
    }

    pub fn upload_shaft(&self, heights: &[f32]) {
        debug_assert_eq!(heights.len(), crate::shaft::DIM * crate::shaft::DIM);
        self.gpu.queue.write_buffer(
            &self.shaft_buf,
            (crate::shaft::HEIGHTS_BASE * 4) as u64,
            bytemuck::cast_slice(heights),
        );
    }

    pub fn sync_world(&mut self, world: &mut World, cam: Vec3) {
        let device = &self.gpu.device;
        let queue = &self.gpu.queue;
        let mut dirty = world.inners.take_dirty();
        if self.inners.sync(
            device,
            queue,
            bytemuck::cast_slice(world.inners.data()),
            &mut dirty,
            16,
        ) {
            self.bind_dirty = true;
        }
        let mut dirty = world.leaves.take_dirty();
        if self.leaves.sync(
            device,
            queue,
            bytemuck::cast_slice(world.leaves.data()),
            &mut dirty,
            8,
        ) {
            self.bind_dirty = true;
        }
        let mut dirty = world.bricks.take_dirty();
        if self.bricks.sync(
            device,
            queue,
            bytemuck::cast_slice(world.bricks.data()),
            &mut dirty,
            4,
        ) {
            self.bind_dirty = true;
        }
        for (key, data) in world.take_probe_dirty() {
            if probe::uploadable(key.origin(), cam) {
                self.write_probe(key.origin(), &data);
            } else {

                world.set_probe(key, data);
            }
        }
    }

    fn write_probe(&self, origin: IVec3, data: &ProbeData) {
        if self.probe_pinned {
            return;
        }
        let sp = probe::SPACING;

        let bx = (origin.x / sp).rem_euclid(PROBE_DIM_XZ as i32) as u32;
        let bz = (origin.z / sp).rem_euclid(PROBE_DIM_XZ as i32) as u32;
        let by = (origin.y / sp).clamp(0, PROBE_DIM_Y as i32 - probe::PER_AXIS as i32) as u32;
        let n = probe::PER_AXIS as u32;
        let mut texels: Vec<u8> = Vec::with_capacity(probe::PROBES * PROBE_TEXEL as usize);
        for axis in 0..probe::AXES {

            texels.clear();
            for i in 0..probe::PROBES {
                let o = (axis * probe::PROBES + i) * 3;
                for c in 0..3 {
                    texels.extend_from_slice(&f16_bits(data.bounce[o + c]).to_le_bytes());
                }
                texels.extend_from_slice(
                    &f16_bits(data.occl[axis * probe::PROBES + i]).to_le_bytes(),
                );
            }
            self.gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.probe_tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: bx,
                        y: axis as u32 * PROBE_DIM_Y + by,
                        z: bz,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &texels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(n * PROBE_TEXEL),
                    rows_per_image: Some(n),
                },
                wgpu::Extent3d {
                    width: n,
                    height: n,
                    depth_or_array_layers: n,
                },
            );
        }
    }

    pub fn build_chunk_list(
        &mut self,
        render_set: &[RenderItem],
        world: &World,
        cam: Vec3,
        frustum: &Frustum,
    ) {
        self.chunk_scratch.clear();

        let gmin = grid_origin(cam);

        let mut plan: Vec<(RenderItem, Aabb)> = Vec::with_capacity(render_set.len());
        for &item in render_set {
            let Some(rec) = world.chunks.get(&item.key) else {
                continue;
            };
            if rec.solid_count == 0 {
                continue;
            }
            plan.push((item, rec.world_aabb()));
        }
        let (visible, offscreen) =
            partition_for_grid(&plan, cam, frustum, gmin, self.offscreen_shadows);

        let mut camera_chunk = None;
        for item in visible.iter().chain(offscreen.iter()) {
            let key = &item.key;
            let rec = &world.chunks[key];
            let aabb = rec.world_aabb();
            let idx = self.chunk_scratch.len();

            if idx < visible.len()
                && camera_chunk.is_none()
                && cam.cmpge(aabb.min).all()
                && cam.cmple(aabb.max).all()
            {
                camera_chunk = Some(idx);
            }
            let (attr_base, mut attr_flags) = match rec.attr {
                AttrRef::Uniform(id) => (id as u32, 1),
                AttrRef::Table { base, .. } => (base, 0),
            };
            if rec.has_water {
                attr_flags |= 2;
            }
            if rec.has_foliage {
                attr_flags |= 4;
            }
            if rec.has_cutout {
                attr_flags |= 8;
            }
            if rec.has_emitter { attr_flags |= 32; }
            if rec.has_glass {
                attr_flags |= 16;
            }
            let (light_base, light_flags) = match rec.light {
                LightRef::Uniform(b) => (b as u32, 1),
                LightRef::Table { base } => (base, 0),
            };
            let root: [u32; 4] = bytemuck::cast(rec.root);
            self.chunk_scratch.push(GpuChunk {
                aabb_min: aabb.min.to_array(),
                voxel_size: key.voxel_size() as f32,
                aabb_max: aabb.max.to_array(),
                lod: key.lod as u32,
                origin: key.origin().as_vec3().to_array(),
                fade: item.fade,
                root,
                attr_base,
                attr_flags,
                light_base,
                light_flags,
                dry_mask: [rec.dry_mask as u32, (rec.dry_mask >> 32) as u32],
                _reserved: [0; 2],
            });
        }

        self.last_chunk_count = visible.len();
        self.last_grid_chunks = self.chunk_scratch.len();
        self.last_camera_chunk = camera_chunk;

        self.grid_scratch.fill(NO_CHUNK);

        let grid_set: Vec<RenderItem> =
            visible.iter().chain(offscreen.iter()).copied().collect();
        let mut order: Vec<usize> = (0..grid_set.len()).collect();
        let share_match = self.shadow_share;
        order.sort_by(|&a, &b| {
            let (ia, ib) = (grid_set[a], grid_set[b]);
            let key = |it: crate::lod::RenderItem| if share_match { it.share() } else { 1.0 };
            key(ia)
                .total_cmp(&key(ib))
                .then(ib.key.lod.cmp(&ia.key.lod))
        });
        for i in order {
            let Some((lo, hi)) = grid_span(grid_set[i].key, gmin) else {
                continue;
            };
            for z in lo.z..hi.z {
                for y in lo.y..hi.y {
                    let row = (y as u32 * GRID_X + z as u32 * GRID_X * GRID_Y) as usize;
                    for x in lo.x..hi.x {
                        self.grid_scratch[row + x as usize] = i as u32;
                    }
                }
            }
        }
        let queue = &self.gpu.queue;
        queue.write_buffer(&self.grid_buf, 0, bytemuck::cast_slice(&self.grid_scratch));
        if self.chunk_buf.write(
            &self.gpu.device,
            queue,
            bytemuck::cast_slice(&self.chunk_scratch),
        ) {
            self.bind_dirty = true;
        }
        self.grid_min = gmin;
    }

    fn ensure_shadow_targets(&mut self,size:(u32,u32)) {
        if self.shadow_targets.size!=size {
            self.shadow_targets=shadow::Targets::new(&self.gpu.device,size,&self.split_layout,&self.shadow_layouts);
            self.shadow_sig = None;
        }
    }

    // Exp 1 (temporal shadow reprojection, safe-static-first): the blocker,
    // radius and visibility targets persist between frames, so when EVERY
    // input to the shadow trio is bit-identical to last frame's, skipping
    // the dispatch reuses exactly what re-tracing would compute. The
    // signature therefore FNV-mixes the camera basis, lens, sun, reach, the
    // full spec pair, the render size and the world epoch -- one bit moves,
    // the trio runs cold. Moving-camera reprojection (depth-compatible
    // sampling of a history copy) is the follow-up this signature already
    // conservatively blocks.
    fn shadow_signature(&self, p: &FrameParams, spec: (u32, u32), size: (u32, u32)) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for f in [p.cam_pos, p.cam_fwd, p.cam_right, p.cam_up, p.sun_dir] {
            for c in [f.x, f.y, f.z] {
                h = (h ^ u64::from(c.to_bits())).wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        for b in [p.fov_y.to_bits(), p.far.to_bits(), p.shadow_dist.to_bits()] {
            h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
        }
        for v in [p.world_epoch as u64, u64::from(spec.0), u64::from(spec.1), u64::from(size.0), u64::from(size.1)] {
            h = (h ^ v).wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }

    fn ensure_spec(&mut self, spec: (u32, u32)) -> usize {
        if let Some(i) = self.spec_pipes.iter().position(|(k, _)| *k == spec) {
            return i;
        }
        let pipes = make_spec(
            &self.gpu.device,
            &self.module,
            (
                &self.pipeline_layout,
                &self.water_sec_pipe_layout,
                &self.water_read_pipe_layout,
                &self.shadow_pipe_layouts[0], &self.shadow_pipe_layouts[1], &self.shadow_pipe_layouts[2],
            ),
            spec.0,
            spec.1,
            self.water_sec_shift,
        );
        self.spec_pipes.push((spec, pipes));
        self.spec_pipes.len() - 1
    }

    pub fn render(&mut self, p: &FrameParams, target: Option<&wgpu::TextureView>) {
        if self.bind_dirty {
            self.rebuild_bind_group();
        }
        let (w, h) = self.size;

        let spec_key = (p.flags & SPEC_MASK, (p.spec_hi | FLAG_HI_SHADOW_PASS) & SPEC_HI_MASK);
        let spec_idx = self.ensure_spec(spec_key);
        self.ensure_shadow_targets((w,h));

        let sibling = (spec_key.0, spec_key.1 ^ FLAG_HI_EMITTER_WORLD);
        if sibling != spec_key {
            self.ensure_spec(sibling);
        }
        let heatmap = p.flags & FLAG_HEATMAP != 0;
        let hiz_on = p.flags & FLAG_HIZ != 0;

        let taa_on = p.taa;
        let parity = (self.frame_index & 1) as usize;

        let jitter = match p.jitter {
            Some(j) => j,
            None if taa_on => crate::math::halton_jitter(self.frame_index),
            None => Vec2::ZERO,
        };
        let feedback = if taa_on && self.history_valid {
            TAA_FEEDBACK
        } else {
            1.0
        };

        let mip_bias = (h as f32 / self.surface_size.1 as f32).log2() + p.mip_bias;

        let tan_half_fov = (p.fov_y * 0.5).tan();
        let aspect = w as f32 / h as f32;

        let view_proj = crate::math::view_proj(
            p.cam_pos,
            p.cam_fwd,
            p.cam_right,
            p.cam_up,
            tan_half_fov,
            aspect,
            NEAR,
            p.far,
        );
        let prev_view_proj = self.prev_view_proj.unwrap_or(view_proj);

        let frame = GpuFrame {
            cam_pos: p.cam_pos.to_array(),
            time: p.time,
            cam_fwd: p.cam_fwd.to_array(),
            tan_half_fov,
            cam_right: p.cam_right.to_array(),
            aspect,
            cam_up: p.cam_up.to_array(),
            far: p.far,
            sun_dir: p.sun_dir.to_array(),
            daylight: p.daylight,
            res: [w, h],
            tiles: [self.tiles.0, self.tiles.1],
            chunk_count: self.last_chunk_count as u32,
            camera_chunk: self.last_camera_chunk.map_or(NO_CHUNK, |i| i as u32),
            flags: p.flags,
            max_pairs: self.max_pairs,
            fog_density: p.fog.density,
            fog_falloff: p.fog.falloff,
            shadow_dist: p.shadow_dist,
            ambient: p.ambient,
            fog_scatter: p.fog.scatter,
            fog_g: p.fog.g,
            fog_height: p.fog.height,
            mip_bias,
            grid_min: self.grid_min.to_array(),
            sea_level: p.sea_level,
            grid_dim: [GRID_X, GRID_Y, GRID_Z],
            water_absorb: p.water_absorb,
            jitter: jitter.to_array(),
            taa_feedback: feedback,

            frame_index: (self.frame_index % 64) as u32,
            cloud_cover: p.clouds.cover,
            cloud_height: p.clouds.height,

            cloud_scale: p.clouds.scale.max(1.0),
            cloud_speed: p.clouds.speed,
            godray_strength: p.godrays.strength,

            godray_steps: p.godrays.steps.clamp(1, 64),
            godray_dist: p.godrays.dist.max(0.0),

            cloud_shadow: p.godrays.cloud_shadow.clamp(0.0, 1.0),

            tint_strength: p.tint.strength.clamp(0.0, 1.0),
            tint_seed: p.tint.seed,
            tint_freq_t: biome::TEMP_FREQ,
            tint_freq_h: biome::HUMID_FREQ,
            wave_amp: p.waves.amp.max(0.0),

            wave_scale: p.waves.scale.max(1.0),
            wave_speed: p.waves.speed,
            wave_reflect_slope: p.waves.reflect_slope.max(0.0),
            shaft_origin: p.shaft_origin,
            shaft_texel: p.shaft_texel,
            shaft_soft: crate::shaft::SOFT,

            cloud_patch: p.clouds.patch,
            cloud_relief: p.clouds.relief,
            haze_warm: p.sky.haze_warm,
            zenith_deep: p.sky.zenith_deep,
            prev_view_proj: prev_view_proj.to_cols_array_2d(),
            world_epoch: p.world_epoch,
            epoch_pad: [0; 3],
        };
        self.gpu
            .queue
            .write_buffer(&self.frame_buf, 0, bytemuck::bytes_of(&frame));
        self.prev_view_proj = Some(view_proj);

        let mut enc = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        enc.clear_buffer(&self.vis.buf, 0, None);
        enc.clear_buffer(&self.counters, 0, None);
        enc.clear_buffer(&self.indirect_storage, 0, None);
        if heatmap {
            enc.clear_buffer(&self.dbg.buf, 0, None);
        }
        if !hiz_on {
            enc.clear_buffer(&self.hiz.buf, 0, None);
        }

        if p.shaft_scan {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("light envelope"),
                timestamp_writes: self.timer.writes(q::SHAFT),
            });
            pass.set_bind_group(0, &self.bind_group, &[]);
            let groups = (crate::shaft::DIM as u32).div_ceil(8);
            for pipe in &self.shaft_pipes {
                pass.set_pipeline(pipe);
                pass.dispatch_workgroups(groups, groups, 1);
            }
        }
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("tile select"),
                timestamp_writes: self.timer.writes(q::TILE_SELECT),
            });
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_pipeline(&self.pipes.tile_select);

            pass.dispatch_workgroups(self.tiles.0.div_ceil(8), self.tiles.1.div_ceil(8), 1);
            pass.set_pipeline(&self.pipes.finalize);
            pass.dispatch_workgroups(1, 1, 1);
        }
        enc.copy_buffer_to_buffer(&self.indirect_storage, 0, &self.indirect, 0, 32);
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("march"),
                timestamp_writes: self.timer.writes(q::MARCH),
            });
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_pipeline(&self.spec_pipes[spec_idx].1.march);
            pass.dispatch_workgroups_indirect(&self.indirect, 0);
        }
        if hiz_on {
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("hi-z"),
                    timestamp_writes: self.timer.writes(q::HIZ),
                });
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_pipeline(&self.pipes.build_hiz);
                pass.dispatch_workgroups(self.tiles.0, self.tiles.1, 1);
                pass.set_pipeline(&self.pipes.finalize_deferred);
                pass.dispatch_workgroups(1, 1, 1);
            }
            enc.copy_buffer_to_buffer(&self.indirect_storage, 0, &self.indirect, 0, 32);
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("recover"),
                    timestamp_writes: self.timer.writes(q::RECOVER),
                });
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_pipeline(&self.pipes.recover_select);
                pass.dispatch_workgroups_indirect(&self.indirect, 12);
                pass.set_pipeline(&self.pipes.finalize_recover);
                pass.dispatch_workgroups(1, 1, 1);
            }
            enc.copy_buffer_to_buffer(&self.indirect_storage, 0, &self.indirect, 0, 32);
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("march recovered"),
                    timestamp_writes: self.timer.writes(q::MARCH2),
                });
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_pipeline(&self.spec_pipes[spec_idx].1.march);
                pass.dispatch_workgroups_indirect(&self.indirect, 0);
            }
        }
        if spec_key.1 & (FLAG_HI_ISOLATE_GLASS | FLAG_HI_SHADOW_PASS) == 0 {
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("resolve"),
                timestamp_writes: self.timer.writes(q::RESOLVE),
            });
            pass.set_bind_group(0, &self.bind_group, &[]);

            if spec_key.1 & FLAG_HI_WATER_SEC != 0 {

                pass.set_bind_group(1, &self.water_empty_group, &[]);
                pass.set_bind_group(2, &self.water_store_group, &[]);
                pass.set_pipeline(&self.spec_pipes[spec_idx].1.water_sec);
                let span = 1u32 << self.water_sec_shift;
                let (hw, hh) = (w.div_ceil(span), h.div_ceil(span));
                pass.dispatch_workgroups(hw.div_ceil(8), hh.div_ceil(8), 1);
            }

            pass.set_bind_group(1, &self.water_read_group, &[]);
            pass.set_bind_group(2, &self.water_empty_group, &[]);
            pass.set_bind_group(3, &self.shadow_targets.read_group, &[]);
            pass.set_pipeline(&self.spec_pipes[spec_idx].1.resolve);
            pass.dispatch_workgroups(w.div_ceil(16), h.div_ceil(8), 1);
        }
        } else {

            {
                let _pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("primary shadow / split family begin"),
                    timestamp_writes: self.timer.family_timestamp(true),
                });
            }
            let shadow_sig = self.shadow_signature(p, spec_key, (w, h));
            let reuse_shadow = p.exp_shadow_repro && self.shadow_sig == Some(shadow_sig);
            if !reuse_shadow {
            for stage in 0..3 {
                let label=["shadow one ray", "shadow bilateral horizontal", "shadow bilateral vertical"][stage];
                let mut pass=enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label:Some(label), timestamp_writes:self.timer.writes(q::SHADOW_RAY+stage as u32*2),
                });
                pass.set_bind_group(0,&self.bind_group,&[]);
                pass.set_bind_group(1,&self.water_empty_group,&[]);
                pass.set_bind_group(2,&self.water_empty_group,&[]);
                pass.set_bind_group(3,&self.shadow_targets.groups[stage],&[]);
                let pipe=if stage==0 { self.spec_pipes[spec_idx].1.primary_shadow.as_ref().unwrap() }
                    else { &self.spec_pipes[spec_idx].1.shadow_blurs[stage-1] };
                pass.set_pipeline(pipe);
                pass.dispatch_workgroups(w.div_ceil(16),h.div_ceil(8),1);
            }
            self.shadow_sig = Some(shadow_sig);
            }
            if spec_key.1 & FLAG_HI_WATER_SEC != 0 {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("water secondary legs"), timestamp_writes: None,
                });
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_bind_group(1, &self.water_empty_group, &[]);
                pass.set_bind_group(2, &self.water_store_group, &[]);
                pass.set_pipeline(&self.spec_pipes[spec_idx].1.water_sec);
                let span=1u32 << self.water_sec_shift;
                pass.dispatch_workgroups(w.div_ceil(span).div_ceil(8), h.div_ceil(span).div_ceil(8), 1);
            }
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some(if spec_key.1 & FLAG_HI_SHADOW_PASS != 0 { "opaque resolve" } else { "main resolve (glass excluded)" }), timestamp_writes: None,
                });
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_bind_group(1, &self.water_read_group, &[]);
                pass.set_bind_group(2, &self.water_empty_group, &[]);
                pass.set_bind_group(3, &self.shadow_targets.read_group, &[]);
                pass.set_pipeline(&self.spec_pipes[spec_idx].1.resolve);
                pass.dispatch_workgroups(w.div_ceil(16), h.div_ceil(8), 1);
            }
            for (label, pipe) in [
                ("glass resolve", self.spec_pipes[spec_idx].1.glass_resolve.as_ref()),
                ("water resolve", self.spec_pipes[spec_idx].1.water_resolve.as_ref()),
            ] {
                if let Some(pipe) = pipe {
                    let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: Some(label), timestamp_writes: None,
                    });
                    pass.set_bind_group(0, &self.bind_group, &[]);
                    pass.set_bind_group(1, &self.water_read_group, &[]);
                    pass.set_bind_group(2, &self.water_empty_group, &[]);
                    pass.set_bind_group(3, &self.shadow_targets.read_group, &[]);
                    pass.set_pipeline(pipe);
                    pass.dispatch_workgroups(w.div_ceil(16), h.div_ceil(8), 1);
                }
            }

            let _end = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("split family end"), timestamp_writes: self.timer.family_timestamp(false),
            });
        }
        if taa_on {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("taa"),
                timestamp_writes: self.timer.writes(q::TAA),
            });
            pass.set_bind_group(0, &self.taa_bind_group[parity], &[]);
            pass.set_pipeline(&self.pipes.taa);
            pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
        }

        if let Some(view) = target {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("present"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            self.last_blit = if taa_on { 1 + parity } else { 0 };
            self.blit.draw(&mut pass, self.last_blit);
            self.ui.draw(&self.gpu.queue, &mut pass, self.surface_size);
        }

        self.timer.encode(&mut enc, &self.counters);
        self.gpu.queue.submit(Some(enc.finish()));
        self.timer.map_latest();
        self.frame_index += 1;

        self.history_valid = taa_on;
    }

    pub fn repaint(&mut self, target: &wgpu::TextureView) {
        let mut enc = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("repaint"),
            });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("present"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            self.blit.draw(&mut pass, self.last_blit);
            self.ui.draw(&self.gpu.queue, &mut pass, self.surface_size);
        }
        self.gpu.queue.submit(Some(enc.finish()));
    }

    pub fn pair_demand(&self) -> Option<PairDemand> {
        self.timer.valid.then(|| PairDemand {
            select: self.timer.counters[4],
            deferred: self.timer.counters[1],
            recover: self.timer.counters[5],
            cap: self.max_pairs,
        })
    }

    pub fn warn_if_truncated(&self) -> bool {
        let Some(d) = self.pair_demand() else {
            return false;
        };
        if !d.overflowed() {
            return false;
        }
        eprintln!(
            "warning: {TRUNCATION_MARK} -- tile_select emitted {} (tile, chunk) pairs against \
             MAX_PAIRS = {}.",
            d.worst(),
            d.cap
        );
        eprintln!(
            "         Geometry is missing from this frame and it will not look like it is. \
             Lower --width/--height/--scale, or --view-distance."
        );
        true
    }

    pub fn march_stats(&self) -> MarchStats {
        let pixels = (self.size.0 as u64) * (self.size.1 as u64);
        let bytes = pixels * 4;
        let staging = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("march stats"),
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        enc.copy_buffer_to_buffer(&self.dbg.buf, 0, &staging, 0, bytes);
        self.gpu.queue.submit(Some(enc.finish()));
        staging.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        self.gpu
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .ok();
        let view = staging.slice(..).get_mapped_range().expect("march stats map");
        let iters: &[u32] = bytemuck::cast_slice(&view);
        let mut total: u64 = 0;
        let mut worst = 0u32;
        let mut touched: u64 = 0;
        let mut capped: u64 = 0;
        let mut hot: Vec<u32> = Vec::with_capacity(iters.len());
        for &i in iters {
            total += i as u64;
            worst = worst.max(i);
            touched += u64::from(i > 0);
            capped += u64::from(i > MARCH_MAX_ITERS);
            hot.push(i);
        }
        drop(view);
        staging.unmap();

        hot.sort_unstable_by(|a, b| b.cmp(a));
        let share = |n: usize| -> f64 {
            let n = n.min(hot.len());
            let s: u64 = hot[..n].iter().map(|&i| u64::from(i)).sum();
            if total == 0 {
                0.0
            } else {
                100.0 * s as f64 / total as f64
            }
        };
        let pct = |p: f64| -> u32 {
            if hot.is_empty() {
                0
            } else {
                hot[(((100.0 - p) / 100.0) * hot.len() as f64) as usize % hot.len()]
            }
        };
        MarchStats {
            pixels,
            iters: total,
            worst,
            touched,
            capped,
            top_1: share(hot.len() / 100),
            top_10: share(hot.len() / 10),
            p50: pct(50.0),
            p90: pct(90.0),
            p99: pct(99.0),
            pairs: self.pair_demand(),
            chunks: self.last_chunk_count,
        }
    }

    pub fn warn_if_oversized(&self) -> bool {
        let n = u64::from(self.tiles.0) * u64::from(self.tiles.1);
        if n <= u64::from(MAX_TILES) {
            return false;
        }
        eprintln!(
            "warning: {OVERSIZE_MARK} -- {}x{} is {n} tiles of 8x8 against a {TILE_ID_BITS}-bit              tile field, which holds {MAX_TILES}.",
            self.size.0, self.size.1
        );
        eprintln!(
            "         Tiles past that index wrap and march another tile's chunks. The frame              will look plausible and be wrong. Lower --width/--height/--scale."
        );
        true
    }

    pub fn read_texture(&self, tex: &wgpu::Texture, size: (u32, u32)) -> Vec<u8> {
        let (w, h) = size;
        let unpadded = w * 4;
        let padded = (unpadded + 255) & !255;
        let buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("screenshot"),
            size: (padded * h) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.gpu.queue.submit(Some(enc.finish()));
        buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        self.gpu
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .ok();
        let view = buf.slice(..).get_mapped_range().expect("screenshot map");
        let mut out = Vec::with_capacity((unpadded * h) as usize);
        for y in 0..h {
            let s = (y * padded) as usize;
            out.extend_from_slice(&view[s..s + unpadded as usize]);
        }
        drop(view);
        buf.unmap();
        out
    }

    pub fn vram_estimate(&self) -> u64 {
        self.inners.capacity()
            + self.leaves.capacity()
            + self.bricks.capacity()
            + self.chunk_buf.capacity()
            + self.vis.capacity()
            + self.hiz.capacity()
            + self.dbg.capacity()
            + self.shadow_targets.bytes()
            + (self.max_pairs as u64) * 8

            + (self.size.0 as u64 * self.size.1 as u64 * 8) * 4
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.surface_format
    }
}

fn make_hdr_texture(
    device: &wgpu::Device,
    size: (u32, u32),
    label: &str,
) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,

        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&Default::default());
    (tex, view)
}

fn make_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    water_mottle: f32,
    tint_balance: bool,
) -> (wgpu::TextureView, wgpu::Sampler) {
    let layers = block::tex::COUNT;
    let size = textures::TEX_SIZE;
    let mips = textures::MIP_LEVELS;
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("block atlas"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: layers,
        },
        mip_level_count: mips,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,

        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let mut level = textures::build_atlas(water_mottle, tint_balance);
    let mut dim = size;
    for mip in 0..mips {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: mip,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &level,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(dim * 4),
                rows_per_image: Some(dim),
            },
            wgpu::Extent3d {
                width: dim,
                height: dim,
                depth_or_array_layers: layers,
            },
        );
        if mip + 1 < mips {
            level = textures::downsample(&level, dim, layers);
            dim /= 2;
        }
    }
    let view = tex.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("atlas sampler"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    });
    (view, sampler)
}

fn f16_bits(v: f32) -> u16 {
    let b = v.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let exp = ((b >> 23) & 0xFF) as i32 - 127 + 15;
    let mant = ((b >> 13) & 0x3FF) as u16;
    if exp <= 0 {
        return sign;
    }
    if exp >= 31 {
        return sign | 0x7C00;
    }
    sign | ((exp as u16) << 10) | mant
}

pub const PROBE_DIM_XZ: u32 = (512 / probe::SPACING) as u32;
const _: () = assert!(PROBE_DIM_XZ == 128);

pub const PROBE_DIM_Y: u32 = (crate::math::WORLD_HEIGHT / probe::SPACING) as u32;
const _: () = assert!(PROBE_DIM_Y == 128);

const PROBE_TEXEL: u32 = 8;

fn make_probe(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    fill: Option<f32>,
    noise: bool,
) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("probe field"),
        size: wgpu::Extent3d {
            width: PROBE_DIM_XZ,
            height: PROBE_DIM_Y * probe::AXES as u32,
            depth_or_array_layers: PROBE_DIM_XZ,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,

        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    let texels = (PROBE_DIM_XZ * PROBE_DIM_Y * PROBE_DIM_XZ) as usize * probe::AXES;
    let mut data: Vec<u8> = Vec::with_capacity(texels * PROBE_TEXEL as usize);
    for i in 0..texels {
        for c in 0..4u32 {
            let v = match (fill, noise) {
                (Some(_), true) => {

                    let h = (i as u32)
                        .wrapping_mul(2654435761)
                        .wrapping_add(c * 0x9E3779B9);
                    0.5 + (h >> 8) as f32 / (1u32 << 25) as f32
                }
                (Some(f), false) => f,
                (None, _) => 1.0,
            };
            data.extend_from_slice(&f16_bits(v).to_le_bytes());
        }
    }

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(PROBE_DIM_XZ * PROBE_TEXEL),
            rows_per_image: Some(PROBE_DIM_Y * probe::AXES as u32),
        },
        wgpu::Extent3d {
            width: PROBE_DIM_XZ,
            height: PROBE_DIM_Y * probe::AXES as u32,
            depth_or_array_layers: PROBE_DIM_XZ,
        },
    );
    let view = tex.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D3),
        ..Default::default()
    });
    (tex, view)
}

fn storage_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

pub fn shader_source() -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}",
        include_str!("shaders/common.wgsl"),
        include_str!("shaders/tile_select.wgsl"),
        include_str!("shaders/march.wgsl"),
        include_str!("shaders/resolve.wgsl"),
        include_str!("shaders/taa.wgsl"),
        include_str!("shaders/shaft.wgsl"),
        include_str!("shaders/shadow.wgsl"),
    )
}

pub const ENTRY_POINTS: [&str; 11] = [
    "shaft_scan",
    "tile_select",
    "finalize",
    "recover_select",
    "finalize_recover",
    "finalize_deferred",
    "march",
    "build_hiz",
    "water_sec",
    "resolve",
    "taa",
];

fn make_spec(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    layouts: (&wgpu::PipelineLayout, &wgpu::PipelineLayout, &wgpu::PipelineLayout, &wgpu::PipelineLayout, &wgpu::PipelineLayout, &wgpu::PipelineLayout),
    spec: u32,
    spec_hi: u32,

    water_sec_shift: u32,
) -> SpecPipes {
    let constants = [
        ("SPEC_TINT", if spec & FLAG_TINT != 0 { 1.0 } else { 0.0 }),
        (
            "SPEC_FOLIAGE",
            if spec & FLAG_FOLIAGE != 0 { 1.0 } else { 0.0 },
        ),
        ("SPEC_WAVES", if spec & FLAG_WAVES != 0 { 1.0 } else { 0.0 }),

        (
            "SPEC_WAVE_ANISO",
            if spec & FLAG_WAVE_ANISO != 0 { 1.0 } else { 0.0 },
        ),
        (
            "SPEC_WAVE_SHOAL",
            if spec & FLAG_WAVE_SHOAL != 0 { 1.0 } else { 0.0 },
        ),
        (
            "SPEC_WAVE_FILL",
            if spec & FLAG_WAVE_FILL != 0 { 1.0 } else { 0.0 },
        ),
        (
            "SPEC_TERRAIN_SHAFTS",
            if spec & FLAG_TERRAIN_SHAFTS != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_TEX_VARIATION",
            if spec & FLAG_TEX_VARIATION != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_DISTANT_SHADOWS",
            if spec & FLAG_DISTANT_SHADOWS != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_LEAF_CUTOUT",
            if spec & FLAG_LEAF_CUTOUT != 0 {
                1.0
            } else {
                0.0
            },
        ),
        ("SPEC_GLASS", if spec & FLAG_GLASS != 0 { 1.0 } else { 0.0 }),
        (
            "SPEC_FLAT_SECONDARY",
            if spec & FLAG_FLAT_SECONDARY != 0 {
                1.0
            } else {
                0.0
            },
        ),

        (
            "SPEC_WATER_SHADOW_DIST",
            if spec & FLAG_WATER_SHADOW_CUT != 0 {
                WATER_SHADOW_DIST as f64
            } else {
                WATER_SHADOW_DIST_PRE41 as f64
            },
        ),

        (
            "SPEC_PROBE_AMBIENT",
            if spec & FLAG_PROBE_AMBIENT != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_PROBE_TAP",
            if spec & FLAG_PROBE_TAP != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_PROBE_CUBE",
            if spec & FLAG_PROBE_CUBE != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_PROBE_BOUNCE",
            if spec & FLAG_PROBE_BOUNCE != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_LIGHT_RGB",
            if spec & FLAG_LIGHT_RGB != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_PROBE_SUN",
            if spec & FLAG_PROBE_SUN != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_PROBE_SUN_HIGH",
            if spec & FLAG_PROBE_SUN_HIGH != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_SKY_TINT",
            if spec & FLAG_SKY_TINT != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_LEAF_FILL",
            if spec & FLAG_LEAF_THIN != 0 {
                LEAF_FILL as f64
            } else {
                LEAF_FILL_PRE43 as f64
            },
        ),

        (
            "SPEC_SKY_SPECULAR",
            if spec_hi & FLAG_HI_SKY_SPECULAR != 0 {
                1.0
            } else {
                0.0
            },
        ),

        (
            "SPEC_FULL_MARCH",
            if spec_hi & FLAG_HI_FULL_MARCH != 0 {
                1.0
            } else {
                0.0
            },
        ),

        (
            "SPEC_EMITTER_GATHER",
            if spec_hi & FLAG_HI_EMITTER_WORLD != 0 {
                1.0
            } else {
                0.0
            },
        ),

        (
            "SPEC_SHORE_WET",
            if spec_hi & FLAG_HI_SHORE_WET != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_SHORE_FOAM",
            if spec_hi & FLAG_HI_SHORE_FOAM != 0 {
                1.0
            } else {
                0.0
            },
        ),

        (
            "SPEC_WATER_SEC",
            if spec_hi & FLAG_HI_WATER_SEC != 0 {
                1.0
            } else {
                0.0
            },
        ),

        (
            "SPEC_CAUSTICS",
            if spec_hi & FLAG_HI_CAUSTICS != 0 {
                1.0
            } else {
                0.0
            },
        ),

        (
            "SPEC_GLASS_REFLECT",
            if spec_hi & FLAG_HI_GLASS_REFLECT != 0 {
                1.0
            } else {
                0.0
            },
        ),

        (
            "SPEC_SNELL_BEND",
            if spec_hi & FLAG_HI_SNELL_BEND != 0 {
                1.0
            } else {
                0.0
            },
        ),

        (
            "SPEC_TINT_BALANCE",
            if spec_hi & FLAG_HI_TINT_BALANCE != 0 {
                1.0
            } else {
                0.0
            },
        ),

        (
            "SPEC_SKY_LOOK",
            if spec_hi & FLAG_HI_SKY_LOOK != 0 {
                1.0
            } else {
                0.0
            },
        ),

        (
            "SPEC_SKY_COOL",
            if spec_hi & FLAG_HI_SKY_COOL != 0 {
                1.0
            } else {
                0.0
            },
        ),

        (
            "SPEC_WATER_LOOK",
            if spec_hi & FLAG_HI_WATER_LOOK != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_FOLIAGE_RICH",
            if spec_hi & FLAG_HI_FOLIAGE_RICH != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_CANOPY_RELIEF",
            if spec_hi & FLAG_HI_CANOPY_RELIEF != 0 {
                1.0
            } else {
                0.0
            },
        ),
        (
            "SPEC_WIND_SWAY",
            if spec_hi & FLAG_HI_WIND_SWAY != 0 {
                1.0
            } else {
                0.0
            },
        ),

        (
            "SPEC_SOFT_SHADOWS",
            if spec_hi & FLAG_HI_SOFT_SHADOWS != 0 {
                1.0
            } else {
                0.0
            },
        ),

        ("SPEC_COMPACT_SHADE_HIT", if spec_hi & FLAG_HI_COMPACT_SHADE_HIT != 0 { 1.0 } else { 0.0 }),
        ("SPEC_ISOLATE_GLASS", if spec_hi & FLAG_HI_ISOLATE_GLASS != 0 { 1.0 } else { 0.0 }),
        ("SPEC_SHADOW_PASS", 1.0),
        ("SHADOW_RADIUS_LIMIT", if spec_hi & FLAG_HI_SOFT_HQ != 0 { 8.0 } else { 4.0 }),
        ("SPEC_LIGHTING_REPAIR", if spec_hi & FLAG_HI_LEGACY_LIGHTING == 0 { 1.0 } else { 0.0 }),
        ("SPEC_WATER_REPAIR", if spec_hi & FLAG_HI_LEGACY_WATER == 0 { 1.0 } else { 0.0 }),
        ("SOFT_TAPS", if spec_hi & FLAG_HI_SOFT_HQ != 0 { 7.0 } else { 3.0 }),
        ("SOFT_PENUMBRA", if spec_hi & FLAG_HI_SUN_NARROW != 0 { 1.0 } else if spec_hi & FLAG_HI_SUN_WIDE != 0 { 3.0 } else { 1.5 }),
        ("SURFACE_DEBUG", ((spec_hi >> 23) & 7) as f64),
        ("WATER_SEC_SHIFT", water_sec_shift as f64),
    ];
    let mk = |name: &'static str, label: &'static str, layout: &wgpu::PipelineLayout, lane: f64| {
        let mut selected = constants.to_vec();
        selected.push(("RESOLVE_LANE", lane));
        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(label), layout: Some(layout), module, entry_point: Some(name),
            compilation_options: wgpu::PipelineCompilationOptions {
                constants: &selected, zero_initialize_workgroup_memory: true,
            }, cache: None,
        })
    };
    let shadow = true;
    let glass = shadow || spec_hi & FLAG_HI_ISOLATE_GLASS != 0;
    SpecPipes {
        march: mk("march", "march", layouts.0, 0.0),
        resolve: mk("resolve", "resolve main", layouts.2, 0.0),
        water_sec: mk("water_sec", "water secondary legs", layouts.1, 0.0),
        primary_shadow: shadow.then(|| mk("primary_shadow", "primary shadow", layouts.3, 0.0)),
        shadow_blurs: [mk("shadow_horizontal", "shadow horizontal", layouts.4, 0.0),mk("shadow_vertical", "shadow vertical", layouts.5, 0.0)],
        glass_resolve: glass.then(|| mk("resolve", "glass resolve", layouts.2, 1.0)),
        water_resolve: shadow.then(|| mk("resolve", "water resolve", layouts.2, 2.0)),
    }
}

pub const BIND_GROUP_0_ENTRIES: usize = 22;

fn make_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let entries: [wgpu::BindGroupLayoutEntry; BIND_GROUP_0_ENTRIES] = [
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            storage_entry(1, true),
            storage_entry(2, true),
            storage_entry(3, true),
            storage_entry(4, true),
            storage_entry(5, true),
            storage_entry(6, false),
            storage_entry(7, false),
            storage_entry(8, false),
            storage_entry(9, false),
            storage_entry(10, false),
            wgpu::BindGroupLayoutEntry {
                binding: 11,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::Rgba16Float,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 12,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 13,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            storage_entry(14, false),
            storage_entry(15, true),
            sampled_entry(16),
            sampled_entry(17),
            wgpu::BindGroupLayoutEntry {
                binding: 18,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            storage_entry(19, false),

            wgpu::BindGroupLayoutEntry {
                binding: 20,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D3,
                    multisampled: false,
                },
                count: None,
            },

            wgpu::BindGroupLayoutEntry {
                binding: 21,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
    ];
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("voxel bgl"),
        entries: &entries,
    })
}

fn make_water_sec_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    views: (&wgpu::TextureView, &wgpu::TextureView),
    label: &str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(views.0),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(views.1),
            },
        ],
    })
}

fn storage_tex_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format: wgpu::TextureFormat::Rgba16Float,
            view_dimension: wgpu::TextureViewDimension::D2,
        },
        count: None,
    }
}

fn sampled_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn make_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    frame: &wgpu::Buffer,
    chunks: &DynBuffer,
    inners: &GpuMirror,
    leaves: &GpuMirror,
    bricks: &GpuMirror,
    faces: &wgpu::Buffer,
    pairs: &wgpu::Buffer,
    counters: &wgpu::Buffer,
    indirect: &wgpu::Buffer,
    vis: &DynBuffer,
    hiz: &DynBuffer,
    out_view: &wgpu::TextureView,
    atlas: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    dbg: &DynBuffer,
    grid: &wgpu::Buffer,

    temporal: (&wgpu::TextureView, &wgpu::TextureView),
    linear: &wgpu::Sampler,
    shaft: &wgpu::Buffer,
    probe: &wgpu::TextureView,
    probe_samp: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("voxel bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: frame.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: chunks.buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: inners.buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: leaves.buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: bricks.buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: faces.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: pairs.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: counters.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 8,
                resource: indirect.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 9,
                resource: vis.buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 10,
                resource: hiz.buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 11,
                resource: wgpu::BindingResource::TextureView(out_view),
            },
            wgpu::BindGroupEntry {
                binding: 12,
                resource: wgpu::BindingResource::TextureView(atlas),
            },
            wgpu::BindGroupEntry {
                binding: 13,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 14,
                resource: dbg.buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 15,
                resource: grid.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 16,
                resource: wgpu::BindingResource::TextureView(temporal.0),
            },
            wgpu::BindGroupEntry {
                binding: 17,
                resource: wgpu::BindingResource::TextureView(temporal.1),
            },
            wgpu::BindGroupEntry {
                binding: 18,
                resource: wgpu::BindingResource::Sampler(linear),
            },
            wgpu::BindGroupEntry {
                binding: 19,
                resource: shaft.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 20,
                resource: wgpu::BindingResource::TextureView(probe),
            },
            wgpu::BindGroupEntry {
                binding: 21,
                resource: wgpu::BindingResource::Sampler(probe_samp),
            },
        ],
    })
}
