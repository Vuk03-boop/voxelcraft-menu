struct UiUniform {
    screen: vec2<f32>,
    pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> ui: UiUniform;
@group(0) @binding(1) var font_tex: texture_2d<f32>;
@group(0) @binding(2) var font_samp: sampler;

struct VsIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs(in: VsIn) -> VsOut {
    var o: VsOut;
    let ndc = vec2<f32>(in.pos.x / ui.screen.x * 2.0 - 1.0, 1.0 - in.pos.y / ui.screen.y * 2.0);
    o.clip = vec4<f32>(ndc, 0.0, 1.0);
    o.uv = in.uv;
    o.color = in.color;
    return o;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    if in.uv.x < 0.0 {
        return in.color;
    }
    let a = textureSample(font_tex, font_samp, in.uv).r;
    return vec4<f32>(in.color.rgb, in.color.a * a);
}
