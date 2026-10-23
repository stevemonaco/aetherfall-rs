// Multiplied over the world (the pipeline's blend state does the multiply): the area's light map.
struct FragmentInput {
    @builtin(position) position: vec4<f32>,
    @location(2) uv: vec2<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var light_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var light_sampler: sampler;

@fragment
fn fragment(in: FragmentInput) -> @location(0) vec4<f32> {
    return textureSample(light_texture, light_sampler, in.uv);
}
