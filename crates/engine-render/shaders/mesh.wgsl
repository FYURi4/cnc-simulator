struct Camera {
    view_proj: mat4x4<f32>,
    eye: vec3<f32>,
}
@group(0) @binding(0) var<uniform> camera: Camera;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal:   vec3<f32>,
    @location(2) uv:       vec2<f32>,
}

struct VsOut {
    @builtin(position) clip_pos:  vec4<f32>,
    @location(0)       world_pos: vec3<f32>,
    @location(1)       normal:    vec3<f32>,
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip_pos  = camera.view_proj * vec4<f32>(in.position, 1.0);
    out.world_pos = in.position;
    out.normal    = normalize(in.normal);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let l = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let diffuse = abs(dot(n, l));

    let cell = 1.0;
    let coord = in.world_pos.xz / cell;
    let d = abs(fract(coord - 0.5) - 0.5) / fwidth(coord);
    let minor = 1.0 - clamp(min(d.x, d.y), 0.0, 1.0);

    let coord10 = in.world_pos.xz / (cell * 10.0);
    let d10 = abs(fract(coord10 - 0.5) - 0.5) / fwidth(coord10);
    let major = 1.0 - clamp(min(d10.x, d10.y), 0.0, 1.0);

    let base_color  = vec3<f32>(0.14, 0.14, 0.17);
    let minor_color = vec3<f32>(0.30, 0.30, 0.34);
    let major_color = vec3<f32>(0.60, 0.60, 0.65);

    var color = base_color * (0.5 + 0.5 * diffuse);
    color = mix(color, minor_color, minor * 0.7);
    color = mix(color, major_color, major);

    let axis_x = 1.0 - clamp(abs(in.world_pos.z) / (fwidth(in.world_pos.z) * 1.5), 0.0, 1.0);
    let axis_z = 1.0 - clamp(abs(in.world_pos.x) / (fwidth(in.world_pos.x) * 1.5), 0.0, 1.0);

    color = mix(color, vec3<f32>(0.90, 0.20, 0.20), axis_x);
    color = mix(color, vec3<f32>(0.20, 0.45, 0.95), axis_z);

    return vec4<f32>(color, 1.0);
}