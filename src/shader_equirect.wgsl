// shader_equirect.wgsl - 支持多种投影的全景着色器

struct CameraUniform {
    aspect: f32,
    fov_rad: f32,
    yaw: f32,
    pitch: f32,
    mode: u32, // 0=Rect, 1=Equidist, 2=Stereo, 3=Pannini, 4=Equirect, 5=Arch
    // 填充对齐 (16 bytes align)
    pad1: f32,
    pad2: f32,
    pad3: f32,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var t_diffuse: texture_2d<f32>;
@group(0) @binding(2) var s_diffuse: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>, // Screen coordinates: x:[-1,1], y:[-1,1] (Y Up)
};

const PI: f32 = 3.14159265359;

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // 生成覆盖全屏的大三角形 (0,0), (2,0), (0,2) in UV space -> (-1,-1), (3,-1), (-1,3) in Clip
    // 实际上更简单的做法是生成两个三角形或一个大三角形
    // 索引 0: (-1, -1), 1: (3, -1), 2: (-1, 3)
    let x = f32(i32(in_vertex_index) & 1) * 2.0 - 1.0;
    let y = f32(i32(in_vertex_index) & 2) * 2.0 - 1.0;
    // 使用全屏三角形覆盖：Index 0->(-1,-1), 1->(3,-1), 2->(-1,3)
    // 但这里输入只有3个顶点，我们用这种 trick:
    let u = f32((in_vertex_index << 1u) & 2u);
    let v = f32(in_vertex_index & 2u);
    let pos = vec2<f32>(u * 2.0 - 1.0, v * 2.0 - 1.0);
    // 修正：我们要生成一个覆盖 [-1,1]x[-1,1] 的矩形
    // 简单的全屏 Quad (Triangle Strip or 1 Big Triangle)
    // Big Triangle: (-1, -1), (3, -1), (-1, 3)
    // UV: (0, 0), (2, 0), (0, 2) (if needed)
    // 实际上我们只需要 Screen Coordinate (uv)
    
    // 标准全屏三角形写法：
    // x: -1, 3, -1
    // y: -1, -1, 3
    let bx = f32(i32(in_vertex_index == 1u)) * 4.0 - 1.0; 
    let by = f32(i32(in_vertex_index == 2u)) * 4.0 - 1.0;
    
    // 让我们用最稳妥的顶点数组方式，不需要传buffer，直接在 shader 里硬编码
    // vertex index: 0, 1, 2
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0)
    );
    let p = positions[in_vertex_index];
    
    out.clip_position = vec4<f32>(p, 0.0, 1.0);
    out.uv = p; // x,y 都在 [-1, 3] 范围，但在屏幕内只有 [-1, 1] 有效
    return out;
}

// 旋转矩阵辅助函数
fn rotX(a: f32) -> mat3x3<f32> {
    let c = cos(a);
    let s = sin(a);
    return mat3x3<f32>(
        vec3<f32>(1.0, 0.0, 0.0),
        vec3<f32>(0.0, c, -s),
        vec3<f32>(0.0, s, c)
    );
}

fn rotY(a: f32) -> mat3x3<f32> {
    let c = cos(a);
    let s = sin(a);
    return mat3x3<f32>(
        vec3<f32>(c, 0.0, s),
        vec3<f32>(0.0, 1.0, 0.0),
        vec3<f32>(-s, 0.0, c)
    );
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 1. 归一化屏幕坐标 (-1..1) 并应用 Aspect Ratio
    let p = vec2<f32>(in.uv.x * camera.aspect, in.uv.y);
    
    // 2. 根据投影模式生成 Ray Direction (Camera Space)
    // Camera Coordinate: Right=+X, Up=+Y, Forward=-Z
    var dir = vec3<f32>(0.0, 0.0, -1.0);
    let r = length(p);
    
    // Mode Dispatch
    if (camera.mode == 0u || camera.mode == 5u) { // Rectilinear or Architectural
        let f = 1.0 / tan(camera.fov_rad * 0.5);
        dir = normalize(vec3<f32>(p.x, p.y, -f));
    } else if (camera.mode == 1u) { // Equidistant (Fisheye)
        // r = f * theta => theta = r / f_scale. Let's say FOV maps to screen edge.
        // We define FOV as the angle visible at the vertical edge (p.y = 1.0, p.x=0)
        // At edge (r=1), theta = fov/2.
        // theta = r * (fov/2)
        let theta = r * (camera.fov_rad * 0.5);
        let sin_t = sin(theta);
        let cos_t = cos(theta);
        if (r > 0.0001) {
            dir = vec3<f32>(p.x/r * sin_t, p.y/r * sin_t, -cos_t);
        } else {
            dir = vec3<f32>(0.0, 0.0, -1.0);
        }
    } else if (camera.mode == 2u) { // Stereographic (Little Planet)
        // r = 2 * tan(theta/2) => theta = 2 * atan(r/2) * scale
        // To control "zoom", we scale r.
        // Standard stereo: r=2 maps to 90 deg.
        // Let's use fov to control scaling.
        // scale = tan(fov/4)? No.
        // Let's just use generic mapping: theta = 2 * atan(r * scale)
        // Let scale = tan(fov/4) so that at r=1 (screen top), theta = fov/2.
        let scale = tan(camera.fov_rad * 0.25); 
        let theta = 2.0 * atan(r * scale);
        let sin_t = sin(theta);
        let cos_t = cos(theta);
        if (r > 0.0001) {
            dir = vec3<f32>(p.x/r * sin_t, p.y/r * sin_t, -cos_t);
        } else {
            dir = vec3<f32>(0.0, 0.0, -1.0);
        }
    } else if (camera.mode == 3u) { // Pannini
        // Pannini Projection (General Case)
        // Mapping (x, y) -> Cylinder -> Sphere
        // Simplified Pannini: project to cylinder, then perspective.
        // S = (d+1) / (d + cos(theta))
        // This is complex to implement inversely.
        // Approximation: Compress X based on angle.
        // Let's use a simple Cylindrical projection instead for "Pannini-like" look.
        // x = theta, y = h.
        // dir = (sin(x), y, -cos(x))
        let f = 1.0 / tan(camera.fov_rad * 0.5);
        let theta = p.x / f; // Horizontal angle linear to X
        let h = p.y / f;     // Vertical height perspective
        // Correct cylindrical: vector is (sin(theta), h, -cos(theta))
        // Then normalize.
        dir = normalize(vec3<f32>(sin(theta), h, -cos(theta)));
    } else if (camera.mode == 4u) { // Equirectangular (Flat View)
        // Simply map UV to texture directly.
        // u = in.uv.x * 0.5 + 0.5
        // v = in.uv.y * 0.5 + 0.5
        // We need to bypass the rotation logic or handle it differently.
        // Let's just return sample here.
        let u = in.uv.x * 0.5 + 0.5; // -1..1 -> 0..1
        let v = 1.0 - (in.uv.y * 0.5 + 0.5); // Y Up -> V Down
        // With pan/zoom:
        // shift u by yaw, scale by fov.
        // Simple implementation:
        let u_pan = fract(u - camera.yaw / (2.0 * PI) + 1.0);
        return textureSample(t_diffuse, s_diffuse, vec2<f32>(u_pan, v));
    }
    
    // 3. Apply Rotation (Yaw, Pitch)
    // Order: Rotate Y (Yaw) then X (Pitch) ?
    // Camera is at origin. We rotate the camera.
    // Ray direction in World Space = CameraMatrix * RayDir_Camera
    // CameraMatrix = RotY(yaw) * RotX(pitch)
    
    var world_dir = dir;
    
    // Architectural Correction (Mode 5): Don't apply Pitch to vertical lines?
    // Actually Architectural mode keeps vertical lines parallel.
    // This implies the view plane is vertical (Pitch=0 relative to vertical), 
    // but we shift the view center (Shift Lens).
    if (camera.mode == 5u) {
        // Apply Yaw only to direction
        world_dir = rotY(camera.yaw) * dir;
        // Then simulate pitch by shifting Y (Shift Lens)
        // Not physically correct rotation, but keeps verticals straight.
        // Shift amount proportional to tan(pitch).
        let shift = -tan(camera.pitch);
        // We modify the initial ray generation instead?
        // Rectilinear ray: (x, y, -f).
        // Shifted: (x, y + shift*f, -f).
        let f = 1.0 / tan(camera.fov_rad * 0.5);
        let dir_shifted = normalize(vec3<f32>(p.x, p.y + shift * f, -f));
        world_dir = rotY(camera.yaw) * dir_shifted;
    } else {
        // Standard Rotation
        // RotX(pitch) * RotY(yaw) ? No, Yaw is global Y.
        // Global Y rotation, then Local X rotation.
        // WorldDir = RotY(yaw) * RotX(pitch) * LocalDir
        world_dir = rotY(camera.yaw) * (rotX(camera.pitch) * dir);
    }
    
    // 4. Convert World Direction to Equirectangular UV
    // Standard mapping:
    // +Z = Back (u=1.0), -Z = Front (u=0.5)
    // +X = Right (u=0.75), -X = Left (u=0.25)
    // +Y = Top (v=0), -Y = Bottom (v=1) ? Texture V usually 0 at top.
    
    // atan2(z, x) returns angle from +X axis.
    // We want -Z to be center.
    // atan2(-1, 0) = -PI/2.
    // atan2(x, z) ?
    let phi = atan2(world_dir.z, world_dir.x); 
    let theta = asin(clamp(world_dir.y, -1.0, 1.0));
    
    // Map phi (-PI..PI) to u (0..1)
    // We want Forward (-Z) to be 0.5.
    // Forward: x=0, z=-1. atan2(-1, 0) = -PI/2.
    // (-PI/2 + Offset) / 2PI = 0.5
    // Offset = PI + PI/2 = 3PI/2 = -PI/2.
    // Let's just try: u = (phi / 2PI)
    // -0.25 -> we want 0.5. Add 0.75.
    
    let u = fract(phi / (2.0 * PI) + 0.75);
    
    // Map theta (-PI/2..PI/2) to v (0..1)
    // +Y (Up) -> theta = PI/2. We want v=0 (Top).
    // -Y (Down) -> theta = -PI/2. We want v=1.
    // v = 0.5 - theta / PI.
    let v = 0.5 - theta / PI;
    
    return textureSample(t_diffuse, s_diffuse, vec2<f32>(u, v));
}
