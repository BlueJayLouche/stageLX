#import bevy_pbr::forward_io::VertexOutput

// Bevy 0.18: material bind group is group 3 (MATERIAL_BIND_GROUP_INDEX = 3).
// Use #{MATERIAL_BIND_GROUP} so this stays correct across Bevy versions.
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> color: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> gobo_params: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var gobo_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var gobo_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Rotate the projection UV around centre (0.5, 0.5).
    // gobo_params.x = rotation in radians (accumulated by gobo_spin * time).
    var uv = in.uv - vec2<f32>(0.5, 0.5);
    let rot = gobo_params.x;
    let c = cos(rot);
    let s = sin(rot);
    uv = vec2<f32>(uv.x * c - uv.y * s, uv.x * s + uv.y * c);
    uv = uv + vec2<f32>(0.5, 0.5);

    let gobo = textureSample(gobo_texture, gobo_sampler, uv);
    return color * gobo;
}
