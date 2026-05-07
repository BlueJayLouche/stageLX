#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::view

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> color: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> gobo_params: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var gobo_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var gobo_sampler: sampler;
/// x=half_angle_rad  y=cone_length_m  z=scatter_k  w=extinction_k
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var<uniform> beam_params: vec4<f32>;
/// world→cone-local inverse transform, written each frame by articulate_beams
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var<uniform> world_to_cone: mat4x4<f32>;

const STEPS: i32 = 16;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // With cull_mode=None both faces rasterize.  Process only the back face
    // (where the ray exits the cone) so each screen pixel marches once.
    let view_dir = normalize(in.world_position.xyz - view.world_position);
    if dot(view_dir, normalize(in.world_normal)) < 0.0 {
        return vec4<f32>(0.0);
    }

    // Gobo UV rotation around (0.5, 0.5).
    var uv = in.uv - vec2<f32>(0.5);
    let c = cos(gobo_params.x);
    let s = sin(gobo_params.x);
    uv = vec2<f32>(uv.x*c - uv.y*s, uv.x*s + uv.y*c) + vec2<f32>(0.5);
    let gobo_mask = textureSample(gobo_texture, gobo_sampler, uv).r;
    if gobo_mask < 0.01 { return vec4<f32>(0.0); }

    let ta = tan(beam_params.x);   // tan(half_angle)
    let H  = beam_params.y;         // cone length (metres)
    let sk = beam_params.z;         // radial scatter falloff
    let ek = beam_params.w;         // depth extinction falloff

    // Transform ray into cone-local space.  t=0 = camera, t=1 = this fragment.
    let o  = (world_to_cone * vec4<f32>(view.world_position, 1.0)).xyz;
    let fp = (world_to_cone * vec4<f32>(in.world_position.xyz, 1.0)).xyz;
    let d  = fp - o;

    // Ray–cone quadratic.
    // Cone: apex=(0, H/2, 0), axis=(0,−1,0), surface: x²+z² = ((H/2−y)·ta)²
    let K   = H * 0.5 - o.y;
    let A   = d.x*d.x + d.z*d.z - ta*ta*d.y*d.y;
    let B   = 2.0*(o.x*d.x + o.z*d.z + ta*ta*K*d.y);
    let Cv  = o.x*o.x + o.z*o.z - ta*ta*K*K;
    let dsc = B*B - 4.0*A*Cv;
    if dsc < 0.0 || abs(A) < 1e-8 { return vec4<f32>(0.0); }

    let sq    = sqrt(dsc);
    let inv2A = 0.5 / A;
    var t0 = (-B - sq)*inv2A;
    var t1 = (-B + sq)*inv2A;
    if t0 > t1 { let tmp = t0; t0 = t1; t1 = tmp; }

    // Clamp to segment camera→fragment and to finite cone Y band.
    t0 = max(t0, 0.0);
    t1 = min(t1, 1.0);
    if abs(d.y) > 1e-6 {
        let tlo = (-H*0.5 - o.y) / d.y;
        let thi = ( H*0.5 - o.y) / d.y;
        t0 = max(t0, min(tlo, thi));
        t1 = min(t1, max(tlo, thi));
    }
    if t1 <= t0 + 1e-6 { return vec4<f32>(0.0); }

    // 16-step ray march accumulating volumetric density.
    var acc   = 0.0;
    let inv_n = 1.0 / f32(STEPS);
    let chord = t1 - t0;
    for (var i = 0; i < STEPS; i++) {
        let t   = t0 + (f32(i) + 0.5) * chord * inv_n;
        let p   = o + t * d;
        let r_c = max((H*0.5 - p.y) * ta, 1e-6);
        let r   = sqrt(p.x*p.x + p.z*p.z);
        if r > r_c { continue; }
        let nr  = r / r_c;                              // 0 at axis, 1 at surface
        let dn  = (t - t0) / max(chord, 1e-6);         // 0 at entry, 1 at exit
        acc += exp(-nr*nr * sk) * exp(-dn * ek);
    }

    // Average sample density, then scale for perceptible brightness.
    acc = clamp(acc * inv_n * 1.5, 0.0, 1.0);

    return vec4<f32>(color.rgb * gobo_mask, acc * color.a);
}
