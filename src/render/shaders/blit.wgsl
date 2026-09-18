// Present the linear HDR trace output: tone-map the highlights, then encode for display.

struct BlitUniform {
    // 1 when the target format has no hardware sRGB encode and this shader must do it.
    encode: u32,
    // A10 (`--tone-map`): 0 is the knee hyperbola every constant in the engine was tuned
    // against, 1 is the Narkowicz ACES fit. A *uniform*, not two pipelines: the selector
    // only chooses which shoulder the highlight rolls onto, and the rule that no constant
    // moves lives above this file at the entry, where it belongs.
    tone: u32,
    // G1's warm grade (`--grade`, batch 95): 0 is the pre-95 presentation bit for bit, 1 is
    // the cast the reference frames in `docs/look-reference-*.png` wear. Same one-uniform
    // rule as `tone` above it: a selector, never a second pipeline. Batch 97b spends value
    // 2 on `cine` -- the filmic print the same references carry -- under the same law.
    grade: u32,
    // Batch 101 (`--grade-strength`): the dial between the two readings the second round
    // gave the cine cast, lerping the graded pixel back toward the pre-grade one. Spends
    // the last padding word, so the uniform stays 16 bytes; a strength of exactly 1.0
    // makes `mix(before, graded, 1.0)` return `graded` bit-for-bit, and the batch-97 grade
    // frame is unchanged by construction. Both grade branches apply the dial the same way.
    strength: f32,
}

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var src_samp: sampler;
@group(0) @binding(2) var<uniform> blit: BlitUniform;

// Highlight shoulder, and nothing else. Below the knee this is exactly the identity, so
// every constant tuned against the linear image stays where it was put; above it, a
// hyperbola that is C1 at the knee and asymptotes to 1.0, so the sun disc and glowstone
// roll off instead of clipping to a flat white blob.
const KNEE: f32 = 0.75;

fn tonemap(x: vec3<f32>) -> vec3<f32> {
    let s = (1.0 - KNEE) * (1.0 - KNEE);
    let shoulder = 1.0 - s / (max(x, vec3<f32>(KNEE)) + (1.0 - 2.0 * KNEE));
    return select(shoulder, x, x < vec3<f32>(KNEE));
}

// Narkowicz's fit of the ACES reference filmic curve, `x(a x + b) / (x(c x + d) + e)` --
// the 2015 constants unmodified, because a fit re-tuned here would be its own batch and
// this file exists to make the law swappable. Per-channel means a hue skew at the bright
// shoulder some prescriptions kill with a matrix; the by-eye pass reads that against the
// reference, which is the entry's whole instrument and the reason this is a *curve arm*
// and not a shader change.
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
    // Batch 95's G1 arm: a warm white balance plus a gentle saturation lift, both
    // *luma-preserving* -- what moves is colour, not brightness, so this cannot re-tune a
    // single constant the lighting engine was authored against (their whole world stays
    // where the curve left it, which A10's entry demands of anything that touches every
    // pixel). The numbers are chosen by eye against the two reference frames: daylight in
    // them runs golden even at noon, mids carry slightly more chroma than this engine
    // ships. Off the flag the branch never runs and the frame is the pre-95 frame.
    if blit.grade == 1u {
        let before = c;
        let luma = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
        let balanced = clamp(c * vec3<f32>(1.05, 1.0, 0.93), vec3<f32>(0.0), vec3<f32>(1.0));
        c = clamp(vec3<f32>(luma) + (balanced - vec3<f32>(luma)) * 1.06,
                  vec3<f32>(0.0), vec3<f32>(1.0));
        // 101's dial as a guarded lerp: at the default the branch never runs and the
        // cast is the batch-95 instruction stream, bit for bit -- no mix-formula
        // reassociation is trusted with a control wager.
        if blit.strength < 1.0 {
            c = mix(before, c, clamp(blit.strength, 0.0, 1.0));
        }
    }
    // Batch 97b (`--grade cine`), the references' filmic print: an S-curve in value,
    // mids slightly desaturated, shadows cooled against warm highlights. Same law as the
    // warm cast one block up -- everything rides the luma pivot, so brightness stays
    // where the curve left it and no lighting constant re-tunes; what moves is hue and
    // micro-contrast. Selector, spare word, no pipeline; off the value the branch never
    // runs and the frame is the pre-97 frame.
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



