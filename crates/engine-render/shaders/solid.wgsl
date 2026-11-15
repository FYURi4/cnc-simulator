struct Camera {
    view_proj: mat4x4<f32>,
    eye: vec3<f32>,
}
struct Object {
    model: mat4x4<f32>,
    tint: vec4<f32>,
}
@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var<uniform> object: Object;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal:   vec3<f32>,
    @location(2) uv:       vec2<f32>,
}

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0)       normal:   vec3<f32>,
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let world = object.model * vec4<f32>(in.position, 1.0);
    out.clip_pos = camera.view_proj * world;
    out.normal   = normalize((object.model * vec4<f32>(in.normal, 0.0)).xyz);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let l = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let d = max(dot(n, l), 0.0);
    let base = vec3<f32>(0.85, 0.55, 0.30) * object.tint.rgb;
    return vec4<f32>(base * (0.3 + 0.7 * d), object.tint.a);
}