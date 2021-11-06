// Vertex shader

[[block]]
struct ScalingUniform {
    window_width: f32;
    window_height: f32;
    scale: f32;
};

[[group(1), binding(0)]]
var<uniform> scaling: ScalingUniform;

struct VertexInput {
    [[location(0)]] position: vec2<f32>;
    [[location(1)]] tex_coords: vec2<f32>;
    [[location(2)]] tint: vec4<f32>;
};

struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(0)]] tex_coords: vec2<f32>;
    [[location(1)]] tint: vec4<f32>;
};

[[stage(vertex)]]
fn main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coords = model.tex_coords;
    out.tint = model.tint;

    var x: f32 = 2.0 * (model.position.x + 0.5) / scaling.window_width;
    var y: f32 = 2.0 * (model.position.y + 0.5) / scaling.window_height - 0.5;

    out.clip_position = vec4<f32>(x * scaling.scale, y * scaling.scale, 0.0, 1.0);
    return out;
}

// Fragment shader

[[group(0), binding(0)]]
var t_diffuse: texture_2d<f32>;
[[group(0), binding(1)]]
var s_diffuse: sampler;

[[stage(fragment)]]
fn main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
   return textureSample(t_diffuse, s_diffuse, in.tex_coords) * in.tint;
}