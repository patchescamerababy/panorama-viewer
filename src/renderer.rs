// renderer.rs — 核心渲染器 (Ray Casting / Fullscreen Quad)

use crate::panorama::ProjectionMode;
use image::{GenericImage, Rgba, RgbaImage};
use wgpu::util::DeviceExt;
use winit::window::Window;

fn setup_egui_ui_fonts(ctx: &egui::Context) {
    // UI 字体加载策略（多语言）：
    // - 运行时动态搜索：系统字体目录 +（可选）exe 同目录/工作目录的 ./assets
    // - 尽量覆盖：中文/繁中/日文/韩文/西里尔/阿拉伯语等
    //
    // 说明：ab_glyph 对 .ttc 支持不稳定，因此优先 .ttf/.otf；.ttc 仍会尝试，失败会自动跳过。

    fn try_parse_owned(bytes: &Vec<u8>) -> bool {
        ab_glyph::FontArc::try_from_vec(bytes.clone()).is_ok()
    }

    fn try_load_font_from_path(path: &std::path::Path) -> Option<Vec<u8>> {
        let bytes = std::fs::read(path).ok()?;
        if try_parse_owned(&bytes) {
            Some(bytes)
        } else {
            None
        }
    }

    // 运行时搜索候选字体（位置无关：基于 current_exe / 相对路径 / 系统字体目录）
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();

    // 1) 优先系统字体目录（跨平台）
    if cfg!(windows) {
        let win_fonts = std::path::PathBuf::from(r"C:\Windows\Fonts");
        // CJK
        candidates.push(win_fonts.join("msyh.ttf")); // Microsoft YaHei (简中)
        candidates.push(win_fonts.join("msyhbd.ttf"));
        candidates.push(win_fonts.join("simhei.ttf")); // SimHei
        candidates.push(win_fonts.join("simsun.ttf")); // SimSun
        candidates.push(win_fonts.join("Deng.ttf")); // DengXian
        candidates.push(win_fonts.join("Dengb.ttf"));
        candidates.push(win_fonts.join("Dengl.ttf"));
        // Japanese
        candidates.push(win_fonts.join("meiryo.ttc"));
        candidates.push(win_fonts.join("meiryo.ttf"));
        candidates.push(win_fonts.join("msgothic.ttc"));
        candidates.push(win_fonts.join("msmincho.ttc"));
        candidates.push(win_fonts.join("yugothm.ttc"));
        candidates.push(win_fonts.join("yumin.ttf"));
        // Korean
        candidates.push(win_fonts.join("malgun.ttf"));
        candidates.push(win_fonts.join("malgunbd.ttf"));
        // Arabic (often covered by Segoe UI / Tahoma / Arial)
        candidates.push(win_fonts.join("segoeui.ttf"));
        candidates.push(win_fonts.join("tahoma.ttf"));
        candidates.push(win_fonts.join("arial.ttf"));
        candidates.push(win_fonts.join("arialuni.ttf")); // Arial Unicode (may exist)
    } else if cfg!(target_os = "macos") {
        // macOS
        candidates.push(std::path::PathBuf::from("/System/Library/Fonts/PingFang.ttc"));
        candidates.push(std::path::PathBuf::from("/System/Library/Fonts/STHeiti Light.ttc"));
        candidates.push(std::path::PathBuf::from("/System/Library/Fonts/STHeiti Medium.ttc"));
        candidates.push(std::path::PathBuf::from("/System/Library/Fonts/Hiragino Sans GB.ttc"));
        candidates.push(std::path::PathBuf::from("/System/Library/Fonts/AppleSDGothicNeo.ttc"));
        candidates.push(std::path::PathBuf::from("/System/Library/Fonts/Helvetica.ttc"));
        candidates.push(std::path::PathBuf::from("/System/Library/Fonts/Supplemental/Arial Unicode.ttf"));
        candidates.push(std::path::PathBuf::from("/System/Library/Fonts/Supplemental/Arial.ttf"));
        // Noto (if installed)
        candidates.push(std::path::PathBuf::from("/Library/Fonts/NotoSansCJK-Regular.ttc"));
        candidates.push(std::path::PathBuf::from("/Library/Fonts/NotoSansSC-Regular.otf"));
        candidates.push(std::path::PathBuf::from("/Library/Fonts/NotoSansTC-Regular.otf"));
        candidates.push(std::path::PathBuf::from("/Library/Fonts/NotoSansJP-Regular.otf"));
        candidates.push(std::path::PathBuf::from("/Library/Fonts/NotoSansKR-Regular.otf"));
        candidates.push(std::path::PathBuf::from("/Library/Fonts/NotoSansArabic-Regular.ttf"));
        // user fonts
        if let Ok(home) = std::env::var("HOME") {
            let home = std::path::PathBuf::from(home);
            candidates.push(home.join("Library/Fonts/PingFang.ttc"));
            candidates.push(home.join("Library/Fonts/NotoSansCJK-Regular.ttc"));
            candidates.push(home.join("Library/Fonts/NotoSansSC-Regular.otf"));
            candidates.push(home.join("Library/Fonts/NotoSansJP-Regular.otf"));
            candidates.push(home.join("Library/Fonts/NotoSansKR-Regular.otf"));
            candidates.push(home.join("Library/Fonts/NotoSansArabic-Regular.ttf"));
        }
    } else if cfg!(unix) {
        // Linux (common distro locations; may vary)
        let linux_candidates = [
            // Noto CJK
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansSC-Regular.otf",
            "/usr/share/fonts/opentype/noto/NotoSansTC-Regular.otf",
            "/usr/share/fonts/opentype/noto/NotoSansJP-Regular.otf",
            "/usr/share/fonts/opentype/noto/NotoSansKR-Regular.otf",
            "/usr/share/fonts/truetype/noto/NotoSansSC-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansTC-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansJP-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansKR-Regular.ttf",
            // Arabic
            "/usr/share/fonts/truetype/noto/NotoSansArabic-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoNaskhArabic-Regular.ttf",
            "/usr/share/fonts/opentype/noto/NotoSansArabic-Regular.otf",
            "/usr/share/fonts/opentype/noto/NotoNaskhArabic-Regular.otf",
            // WenQuanYi (CJK fallback)
            "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        ];
        for p in linux_candidates {
            candidates.push(std::path::PathBuf::from(p));
        }

        if let Ok(home) = std::env::var("HOME") {
            let home = std::path::PathBuf::from(home);
            let user_candidates = [
                ".local/share/fonts/NotoSansCJK-Regular.ttc",
                ".local/share/fonts/NotoSansSC-Regular.ttf",
                ".local/share/fonts/NotoSansTC-Regular.ttf",
                ".local/share/fonts/NotoSansJP-Regular.ttf",
                ".local/share/fonts/NotoSansKR-Regular.ttf",
                ".local/share/fonts/NotoSansArabic-Regular.ttf",
                ".fonts/NotoSansCJK-Regular.ttc",
                ".fonts/NotoSansSC-Regular.ttf",
                ".fonts/NotoSansTC-Regular.ttf",
                ".fonts/NotoSansJP-Regular.ttf",
                ".fonts/NotoSansKR-Regular.ttf",
                ".fonts/NotoSansArabic-Regular.ttf",
            ];
            for p in user_candidates {
                candidates.push(home.join(p));
            }
        }
    }

    // 2) 再尝试 assets（用户可自行放置 Noto 字体；便于打包/跨机器）
    let asset_files = [
        // CJK 通用/简中
        "NotoSansCJK-Regular.ttc",
        "NotoSansSC-Regular.otf",
        "NotoSansSC-Regular.ttf",
        "NotoSansSC-Regular-Regular.ttf",
        // 繁中
        "NotoSansTC-Regular.otf",
        "NotoSansTC-Regular.ttf",
        // 日文
        "NotoSansJP-Regular.otf",
        "NotoSansJP-Regular.ttf",
        // 韩文
        "NotoSansKR-Regular.otf",
        "NotoSansKR-Regular.ttf",
        // 俄语/西里尔/拉丁通常已覆盖
        "NotoSans-Regular.ttf",
        "NotoSans-Regular.otf",
        // 阿拉伯语
        "NotoSansArabic-Regular.ttf",
        "NotoSansArabic-Regular.otf",
        "NotoNaskhArabic-Regular.ttf",
        "NotoNaskhArabic-Regular.otf",
    ];

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for f in asset_files {
                candidates.push(dir.join("assets").join(f));
            }
        }
    }
    for f in asset_files {
        candidates.push(std::path::PathBuf::from("assets").join(f));
    }

    let mut chosen: Option<(std::path::PathBuf, Vec<u8>)> = None;
    for p in candidates {
        if let Some(bytes) = try_load_font_from_path(&p) {
            chosen = Some((p, bytes));
            break;
        }
    }

    let Some((font_path, font_bytes)) = chosen else {
        eprintln!("{}", crate::i18n::tr("font.not_found"));
        return;
    };

    eprintln!(
        "{}",
        crate::i18n::tr_with(
            "font.using",
            &[("path", font_path.display().to_string())]
        )
    );

    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "ui".to_owned(),
        egui::FontData::from_owned(font_bytes),
    );
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        family.insert(0, "ui".to_owned());
    }
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        family.insert(0, "ui".to_owned());
    }
    ctx.set_fonts(fonts);
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    aspect: f32,
    fov_rad: f32,
    yaw: f32,
    pitch: f32,
    mode: u32, // 0=Rect, 1=Equidist, 2=Stereo, 3=Pannini, 4=Equirect, 5=Arch
    pad1: f32,
    pad2: f32,
    pad3: f32,
}

pub struct Renderer {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    render_pipeline: wgpu::RenderPipeline,
    
    // 纹理资源
    texture_bind_group_layout: wgpu::BindGroupLayout,
    diffuse_bind_group: wgpu::BindGroup,
    texture: wgpu::Texture,
    sampler: wgpu::Sampler,
    
    // Uniform 资源
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,

    // UI
    pub egui_ctx: egui::Context,
    pub egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
}

impl Renderer {
    pub async fn new(window: std::sync::Arc<Window>) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        
        let surface = unsafe { instance.create_surface(window.as_ref()) }.unwrap();
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }).await.unwrap();

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                features: wgpu::Features::empty(),
                limits: if cfg!(target_arch = "wasm32") {
                    wgpu::Limits::downlevel_webgl2_defaults()
                } else {
                    wgpu::Limits::default().using_resolution(adapter.limits())
                },
                label: None,
            },
            None,
        ).await.unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats.iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
            
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo, // VSync on
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        // --- 1. Texture Setup (Default Checkerboard) ---
        let texture_size = wgpu::Extent3d { width: 2, height: 2, depth_or_array_layers: 1 };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: Some("diffuse_texture"),
            view_formats: &[],
        });
        
        // 初始写入一些数据防止全黑
        queue.write_texture(
            wgpu::ImageCopyTexture { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255],
            wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(8), rows_per_image: Some(2) },
            texture_size,
        );

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat, // 全景图通常需要水平循环
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // --- 2. Uniform Setup ---
        let camera_uniform = CameraUniform {
            aspect: size.width as f32 / size.height as f32,
            fov_rad: 46.8f32.to_radians(),
            yaw: 0.0,
            pitch: 0.0,
            mode: 0,
            pad1: 0.0, pad2: 0.0, pad3: 0.0,
        };

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry { // Camera Uniform
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT, // Used in Fragment
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { // Texture
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { // Sampler
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("texture_bind_group_layout"),
        });

        let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: camera_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&texture_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
            label: Some("diffuse_bind_group"),
        });

        // --- 3. Pipeline Setup ---
        let shader = device.create_shader_module(wgpu::include_wgsl!("shader_equirect.wgsl"));
        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[], // 无顶点缓冲，Shader 自生成
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // 不要剔除，因为我们要画一个覆盖全屏的三角形
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None, // 不需要深度缓冲，全屏绘制
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        // --- 4. Egui Setup ---
        let egui_ctx = egui::Context::default();
        setup_egui_ui_fonts(&egui_ctx);
        
        // 修复 macOS 高分屏问题：使用正确的 API (egui-winit 0.23)
        let mut egui_state = egui_winit::State::new(window.as_ref());
        // 显式设置 pixels_per_point 以处理高 DPI 显示器
        egui_state.set_pixels_per_point(window.scale_factor() as f32);
        
        let egui_renderer = egui_wgpu::Renderer::new(&device, config.format, None, 1);

        Self {
            surface, device, queue, config, size,
            render_pipeline,
            texture_bind_group_layout, diffuse_bind_group,
            texture, sampler,
            camera_uniform, camera_buffer,
            egui_ctx, egui_state, egui_renderer,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.camera_uniform.aspect = new_size.width as f32 / new_size.height as f32;
        }
    }

    pub fn update_camera(&mut self, yaw: f32, pitch: f32, fov: f32, mode: ProjectionMode) {
        // 重要：部分投影（Rectilinear/Pannini/Architectural）在 shader 内部会用到 tan(fov/2)。
        // 当 fov == 180° 时 tan(90°) 落在奇点，会导致 Inf/NaN，最终画面全黑或闪烁。
        // 这里做一次“安全夹取”，并保持 UI 层仍可显示 180°。
        let safe_fov_deg = match mode {
            ProjectionMode::Rectilinear | ProjectionMode::Pannini | ProjectionMode::Architectural => {
                fov.clamp(1.0, 179.9)
            }
            _ => fov.clamp(1.0, 180.0),
        };

        // 同理：pitch 若到达 ±90°，Architectural 模式里 tan(pitch) 也会爆。
        let safe_pitch_deg = pitch.clamp(-89.9, 89.9);

        self.camera_uniform.yaw = yaw.to_radians();
        self.camera_uniform.pitch = safe_pitch_deg.to_radians();
        self.camera_uniform.fov_rad = safe_fov_deg.to_radians();

        self.camera_uniform.mode = match mode {
            ProjectionMode::Rectilinear => 0,
            ProjectionMode::Equidistant => 1,
            ProjectionMode::Stereographic => 2,
            ProjectionMode::Pannini => 3,
            ProjectionMode::Equirectangular => 4,
            ProjectionMode::Architectural => 5,
        };

        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[self.camera_uniform]));
    }

    pub fn load_panorama(&mut self, img: RgbaImage) {
        // 获取 GPU 纹理尺寸限制
        let max_texture_dimension = self.device.limits().max_texture_dimension_2d;
        
        let (src_w, src_h) = img.dimensions();
        
        // 如果图片超过 GPU 限制，则缩放到限制内
        let img = if src_w > max_texture_dimension || src_h > max_texture_dimension {
            let scale = (max_texture_dimension as f32 / src_w.max(src_h) as f32).min(1.0);
            let new_w = (src_w as f32 * scale) as u32;
            let new_h = (src_h as f32 * scale) as u32;
            eprintln!(
                "{}",
                crate::i18n::tr_with(
                    "gpu.image_too_large_scaled",
                    &[
                        ("src_w", src_w.to_string()),
                        ("src_h", src_h.to_string()),
                        ("max", max_texture_dimension.to_string()),
                        ("new_w", new_w.to_string()),
                        ("new_h", new_h.to_string())
                    ]
                )
            );
            image::DynamicImage::ImageRgba8(img).resize(
                new_w,
                new_h,
                image::imageops::FilterType::Lanczos3
            ).to_rgba8()
        } else {
            img
        };
        
        // 兼容非 2:1 纹理：
        // - 以"宽度"为基准计算目标等矩形高度 target_h = width / 2
        // - 如果原图高度 < target_h：在顶部补黑，把原图贴到底部（上方空置）
        // 这样 shader 在采样 v=0..1 时，上半部分自然是黑色。
        let (src_w, src_h) = img.dimensions();
        let target_h = src_w / 2;

        let img = if target_h > 0 && src_h < target_h {
            let mut canvas = RgbaImage::from_pixel(src_w, target_h, Rgba([0, 0, 0, 255]));
            let y_offset = target_h - src_h;
            // 把原图贴到底部
            // copy_from 在越界时会返回 Err，这里 y_offset 已保证不会越界
            let _ = canvas.copy_from(&img, 0, y_offset);
            canvas
        } else {
            img
        };

        let (width, height) = img.dimensions();
        let texture_size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        self.texture = self.device.create_texture(&wgpu::TextureDescriptor {
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: Some("panorama_texture"),
            view_formats: &[],
        });

        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &img,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            texture_size,
        );

        let texture_view = self.texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Recreate bind group with new texture view
        self.diffuse_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
            label: Some("diffuse_bind_group"),
        });
    }

    

    pub fn render_with_ui(
        &mut self, 
        window: &Window, 
        run_ui: impl FnOnce(&egui::Context)
    ) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        // 1. Render Scene (Fullscreen Quad)
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.1, g: 0.1, b: 0.1, a: 1.0 }),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.diffuse_bind_group, &[]);
            render_pass.draw(0..3, 0..1); // Draw 3 vertices for fullscreen coverage
        }
        
        // 2. Render UI
        let raw_input = self.egui_state.take_egui_input(window);
        let full_output = self.egui_ctx.run(raw_input, run_ui);
        
        self.egui_state.handle_platform_output(window, &self.egui_ctx, full_output.platform_output);
        let clipped_primitives = self.egui_ctx.tessellate(full_output.shapes);
        
        let screen_descriptor = egui_wgpu::renderer::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: window.scale_factor() as f32,
        };

        for (id, delta) in &full_output.textures_delta.set {
            self.egui_renderer.update_texture(&self.device, &self.queue, *id, delta);
        }
        
        self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &clipped_primitives,
            &screen_descriptor,
        );

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Egui Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: true },
                })],
                depth_stencil_attachment: None,
            });
            self.egui_renderer.render(&mut render_pass, &clipped_primitives, &screen_descriptor);
        }
        
        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}
