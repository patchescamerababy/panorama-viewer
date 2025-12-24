// panorama.rs — 视角参数与投影模式

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProjectionMode {
    Rectilinear,    // 1. 标准透视 (适合正常视角，直线保持直线)
    Equidistant,    // 2. 等距鱼眼 (适合广角，边缘压缩，直线弯曲)
    Stereographic,  // 3. 小行星/立体投影 (艺术效果)
    Pannini,        // 4. 帕尼尼投影 (建筑常用，垂直线直，水平压缩)
    Equirectangular,// 5. 原图展开 (2:1 平面查看)
    Architectural,  // 6. 建筑校正 (类似 Rectilinear 但修正垂直透视)
}

pub struct PanoramaViewer3D {
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
    pub sensitivity_scale: f32,
    pub projection_mode: ProjectionMode,
    pub is_fullscreen: bool,
}

impl PanoramaViewer3D {
    pub fn new() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            fov: 46.8,
            sensitivity_scale: 1.0,
            projection_mode: ProjectionMode::Rectilinear,
            is_fullscreen: false,
        }
    }
}
