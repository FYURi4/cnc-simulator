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
    @location(3) color:    vec3<f32>,
}

struct VsOut {
    @builtin(position) clip_pos:  vec4<f32>,
    @location(0)       world_pos: vec3<f32>,
    @location(1)       normal:    vec3<f32>,
    @location(2)       color:     vec3<f32>,
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let world = object.model * vec4<f32>(in.position, 1.0);
    out.clip_pos  = camera.view_proj * world;
    out.world_pos = world.xyz;
    out.normal    = normalize(in.normal);
    out.color     = in.color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let cell = 1.0;
    let coord = in.world_pos.xz / cell;
    let d = abs(fract(coord - 0.5) - 0.5) / fwidth(coord);
    let minor = 1.0 - clamp(min(d.x, d.y), 0.0, 1.0);

    let coord10 = in.world_pos.xz / (cell * 10.0);
    let d10 = abs(fract(coord10 - 0.5) - 0.5) / fwidth(coord10);
    let major = 1.0 - clamp(min(d10.x, d10.y), 0.0, 1.0);

    let minor_color = vec3<f32>(0.30, 0.30, 0.34);
    let major_color = vec3<f32>(0.60, 0.60, 0.65);

    var color = vec3<f32>(0.0);
    var alpha = 0.0;

    if (minor > 0.0) {
        color = mix(color, minor_color, minor);
        alpha = max(alpha, minor * 0.8);
    }
    if (major > 0.0) {
        color = mix(color, major_color, major);
        alpha = max(alpha, major);
    }

    let axis_x = 1.0 - clamp(abs(in.world_pos.z) / (fwidth(in.world_pos.z) * 1.5), 0.0, 1.0);
    let axis_z = 1.0 - clamp(abs(in.world_pos.x) / (fwidth(in.world_pos.x) * 1.5), 0.0, 1.0);

    if (axis_x > 0.0) {
        color = mix(color, vec3<f32>(0.90, 0.20, 0.20), axis_x);
        alpha = max(alpha, axis_x);
    }
    if (axis_z > 0.0) {
        color = mix(color, vec3<f32>(0.20, 0.45, 0.95), axis_z);
        alpha = max(alpha, axis_z);
    }

    let dist = length(in.world_pos.xz);
    let fade = 1.0 - smoothstep(8.0, 14.0, dist);
    alpha *= fade;

    if (alpha < 0.01) {
        discard;
    }

    return vec4<f32>(color * object.tint.rgb * in.color, alpha);
}