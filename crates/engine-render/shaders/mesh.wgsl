struct Camera {
    view_proj: mat4x4<f32>,
    eye: vec3<f32>,
    grid_extent: f32,
    grid_color: vec4<f32>,
    grid_type: f32,
    grid_visible: f32,
    grid_step_x: f32,
    grid_step_y: f32,
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
    if (camera.grid_visible < 0.5) {
        discard;
    }
    let step_x = max(camera.grid_step_x, 0.001);
    let step_y = max(camera.grid_step_y, 0.001);
    
    // Minor grid (step)
    let coord_x = in.world_pos.x / step_x;
    let coord_y = in.world_pos.z / step_y;
    let d = min(
        abs(fract(coord_x - 0.5) - 0.5) / fwidth(coord_x),
        abs(fract(coord_y - 0.5) - 0.5) / fwidth(coord_y)
    );
    let minor = 1.0 - clamp(d, 0.0, 1.0);

    // Major lines every five minor grid cells (25 mm at the default 5 mm step).
    let major_step_x = step_x * 5.0;
    let major_step_y = step_y * 5.0;
    let coord10_x = in.world_pos.x / major_step_x;
    let coord10_y = in.world_pos.z / major_step_y;
    let d10 = min(
        abs(fract(coord10_x - 0.5) - 0.5) / fwidth(coord10_x),
        abs(fract(coord10_y - 0.5) - 0.5) / fwidth(coord10_y)
    );
    let major = 1.0 - clamp(d10, 0.0, 1.0);

    var color = camera.grid_color.rgb * vec3<f32>(0.2);
    var alpha = 0.0;

    let is_grid = camera.grid_type < 0.5;
    let is_dots = camera.grid_type > 0.5 && camera.grid_type < 1.5;
    let is_dots_major = camera.grid_type > 1.5 && camera.grid_type < 2.5;
    let is_dots_only = camera.grid_type > 2.5;

    if (is_grid) {
        // Full grid with lines
        if (minor > 0.0) {
            color = mix(color, camera.grid_color.rgb * vec3<f32>(0.6), minor);
            alpha = max(alpha, minor * 0.8);
        }
        if (major > 0.0) {
            color = mix(color, camera.grid_color.rgb, major);
            alpha = max(alpha, major);
        }

        // Axis lines X (red) and Y (blue)
        let axis_x = 1.0 - clamp(abs(in.world_pos.z) / (fwidth(in.world_pos.z) * 1.5), 0.0, 1.0);
        let axis_z = 1.0 - clamp(abs(in.world_pos.x) / (fwidth(in.world_pos.x) * 1.5), 0.0, 1.0);

        if (axis_x > 0.0) {
            color = mix(color, camera.grid_color.rgb * vec3<f32>(1.5, 0.3, 0.3), axis_x);
            alpha = max(alpha, axis_x);
        }
        if (axis_z > 0.0) {
            color = mix(color, camera.grid_color.rgb * vec3<f32>(0.3, 0.6, 1.5), axis_z);
            alpha = max(alpha, axis_z);
        }
    } else if (is_dots) {
        // Dots: minor dots at step, major dots at 15mm
        if (minor > 0.7) {
            color = mix(color, camera.grid_color.rgb * vec3<f32>(0.6), minor);
            alpha = max(alpha, minor * 0.6);
        }
        if (major > 0.7) {
            color = mix(color, camera.grid_color.rgb, major);
            alpha = max(alpha, major * 0.8);
        }
    } else if (is_dots_major) {
        // Only major dots at 15mm
        if (major > 0.7) {
            color = mix(color, camera.grid_color.rgb, major);
            alpha = max(alpha, major * 0.8);
        }
    } else if (is_dots_only) {
        // Only major dots, no minor
        if (major > 0.7) {
            color = mix(color, camera.grid_color.rgb, major);
            alpha = max(alpha, major * 0.9);
        }
    }

    let dist = length(in.world_pos.xz);
    let fade_start = camera.grid_extent * 0.6;
    let fade_end   = camera.grid_extent;
    let fade = 1.0 - smoothstep(fade_start, fade_end, dist);
    alpha *= fade;

    if (alpha < 0.01) {
        discard;
    }

    return vec4<f32>(color * in.color, alpha * camera.grid_color.a);
}
