struct BlitUniform {

    encode: u32,

    tone: u32,

    grade: u32,

    strength: f32,
}

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var src_samp: sampler;
@group(0) @binding(2) var<uniform> blit: BlitUniform;

const KNEE: f32 = 0.75;

fn tonemap(x: vec3<f32>) -> vec3<f32> {
    let s = (1.0 - KNEE) * (1.0 - KNEE);
    let shoulder = 1.0 - s / (max(x, vec3<f32>(KNEE)) + (1.0 - 2.0 * KNEE));
    return select(shoulder, x, x < vec3<f32>(KNEE));
}

fn aces_fit(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(max(c, vec3<f32>(0.0031308)), vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VsOut {
    var o: VsOut;
    let x = f32((vi << 1u) & 2u);
    let y = f32(vi & 2u);
    o.clip = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    o.uv = vec2<f32>(x, y);
    return o;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let hdr = max(textureSample(src, src_samp, in.uv).rgb, vec3<f32>(0.0));
    var c = tonemap(hdr);
    if blit.tone == 1u {
        c = aces_fit(hdr);
    }

    if blit.grade == 1u {
        let before = c;
        let luma = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
        let balanced = clamp(c * vec3<f32>(1.05, 1.0, 0.93), vec3<f32>(0.0), vec3<f32>(1.0));
        c = clamp(vec3<f32>(luma) + (balanced - vec3<f32>(luma)) * 1.06,
                  vec3<f32>(0.0), vec3<f32>(1.0));

        if blit.strength < 1.0 {
            c = mix(before, c, clamp(blit.strength, 0.0, 1.0));
        }
    }

    if blit.grade == 2u {
        let before = c;
        let luma = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
        var g = mix(c, vec3<f32>(luma), 0.12);
        g = mix(g, g * g * (vec3<f32>(3.0) - 2.0 * g), 0.45);
        let shad = pow(clamp(1.0 - luma * 2.2, 0.0, 1.0), 2.0);
        let hi = pow(clamp(luma, 0.0, 1.0), 3.0);
        g *= vec3<f32>(1.0 - 0.05 * shad + 0.03 * hi,
                       1.0 - 0.02 * shad + 0.02 * hi,
                       1.0 + 0.07 * shad - 0.01 * hi);
        c = clamp(g, vec3<f32>(0.0), vec3<f32>(1.0));
        if blit.strength < 1.0 {
            c = mix(before, c, clamp(blit.strength, 0.0, 1.0));
        }
    }
    if blit.encode != 0u {
        c = linear_to_srgb(c);
    }
    return vec4<f32>(c, 1.0);
}
