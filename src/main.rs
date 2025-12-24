// main.rs — 完整的 Rust 实现，包含菜单、状态栏和 3D 交互

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // 在 Release 模式下隐藏控制台窗口

mod panorama;
mod renderer;

use panorama::{PanoramaViewer3D, ProjectionMode};
use renderer::Renderer;

use winit::{
    dpi::{LogicalSize, PhysicalPosition},
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{WindowBuilder, Fullscreen},
};

use std::sync::Arc;
use std::path::PathBuf;
use std::time::Instant;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use image::io::Reader as ImageReader;
use image::GenericImageView;
use std::fs::File;
use std::io::BufReader;

fn main() {
    // env_logger::init(); // 在 Windows Subsystem 下标准输出不可见，可以考虑写入文件日志
    
    let event_loop = EventLoop::new();
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("全景照片查看器 - Panorama Viewer (Rust GPU)")
            .with_inner_size(LogicalSize::new(1280, 720))
            .build(&event_loop)
            .unwrap()
    ); 

    // Renderer 初始化不再需要 Mesh，改用全屏 Ray Casting
    let mut renderer = pollster::block_on(Renderer::new(window.clone()));
    let mut viewer = PanoramaViewer3D::new();
    
    // 交互状态
    let mut mouse_pressed = false;
    let mut last_mouse_pos: Option<PhysicalPosition<f64>> = None;
    
    // FPS 计算
    let mut last_frame_time = Instant::now();
    let mut frame_count = 0;
    let mut fps = 0.0;
    let mut show_fps = false;

    // UI 状态
    let mut vsync_enabled = true;
    let mut is_loading = false;

    // 异步加载通道
    let (tx, rx): (Sender<image::RgbaImage>, Receiver<image::RgbaImage>) = channel();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        
        // 检查是否有新加载的图片
        if let Ok(rgba) = rx.try_recv() {
            renderer.load_panorama(rgba);
            is_loading = false;
        }

        match event {
            Event::WindowEvent { event, .. } => {
                // 先让 egui 处理事件
                let response = renderer.egui_state.on_event(&renderer.egui_ctx, &event);
                if response.consumed {
                    return;
                }

                match event {
                    WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit;
                    }

                    WindowEvent::Resized(new_size) => {
                        renderer.resize(new_size);
                    }

                    // 键盘快捷键
                    WindowEvent::KeyboardInput { input, .. } => {
                        if input.state == ElementState::Pressed {
                            match input.virtual_keycode {
                                Some(VirtualKeyCode::O) => {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("图片", &["jpg", "jpeg", "png", "bmp"])
                                    .pick_file()
                                {
                                    is_loading = true;
                                    start_load_image(path, tx.clone());
                                }
                                }
                                Some(VirtualKeyCode::F11) => {
                                    viewer.is_fullscreen = !viewer.is_fullscreen;
                                    if viewer.is_fullscreen {
                                        window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                                    } else {
                                        window.set_fullscreen(None);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }

                    // 鼠标交互
                    WindowEvent::MouseInput { state, button, .. } => {
                        if button == MouseButton::Left {
                            mouse_pressed = state == ElementState::Pressed;
                            if !mouse_pressed {
                                last_mouse_pos = None;
                            }
                        }
                    }

                    WindowEvent::CursorMoved { position, .. } => {
                        if mouse_pressed {
                            if let Some(last_pos) = last_mouse_pos {
                                let dx = (position.x - last_pos.x) as f32;
                                let dy = (position.y - last_pos.y) as f32;
                                
                                // 旋转逻辑
                                let width = renderer.size.width as f32;
                                let height = renderer.size.height as f32;
                                
                                if width > 0.0 && height > 0.0 {
                                    // 对齐 JavaFX 版本的拖拽映射：
                                    // vF = fov(垂直)；hF = 2 * atan(tan(vF/2) * aspect)
                                    // yaw += dx/width * hF；pitch -= dy/height * vF
                                    //
                                    // 说明：这里保留当前 Rust 的方向约定（yaw -=、pitch -=），
                                    // 只把“每像素角度”改成和 Java 一致的计算方式，保证不同 FOV 下手感稳定。
                                    let v_f = viewer.fov.to_radians();
                                    let aspect = width / height;
                                    let h_f = 2.0 * ((v_f / 2.0).tan() * aspect).atan();

                                    let yaw_per_px_deg = (h_f / width).to_degrees();
                                    let pitch_per_px_deg = (v_f / height).to_degrees();

                                    viewer.yaw -= dx * yaw_per_px_deg * viewer.sensitivity_scale;
                                    viewer.pitch = (viewer.pitch - dy * pitch_per_px_deg * viewer.sensitivity_scale)
                                        .clamp(-90.0, 90.0);
                                }
                            }
                            last_mouse_pos = Some(position);
                        }
                    }

                    WindowEvent::MouseWheel { delta, .. } => {
                        let scroll = match delta {
                            MouseScrollDelta::LineDelta(_, y) => y,
                            MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 20.0,
                        };
                        // 不同的投影模式可能有不同的 FOV 限制
                        let min_fov =
                            if viewer.projection_mode == ProjectionMode::Stereographic { 10.0 } else { 5.0 };

                        // 允许更大的 FOV（最大可到 180°）。注意：Rectilinear/Pannini/Architectural 在 shader 里用到了
                        // tan(fov/2)，严格的 180° 会落在 tan(90°) 奇点；因此这里只把它们放宽到 179.9°，
                        // 其它模式可到 180°。
                        let max_fov = match viewer.projection_mode {
                            ProjectionMode::Rectilinear | ProjectionMode::Pannini | ProjectionMode::Architectural => 179.9,
                            _ => 180.0,
                        };

                        viewer.fov = (viewer.fov - scroll * 2.5).clamp(min_fov, max_fov);
                    }

                    WindowEvent::DroppedFile(path) => {
                        is_loading = true;
                        start_load_image(path, tx.clone());
                    }

                    _ => {}
                }
            }

            Event::RedrawRequested(_) => {
                // FPS 统计
                frame_count += 1;
                let now = Instant::now();
                if now.duration_since(last_frame_time).as_secs_f32() >= 1.0 {
                    fps = frame_count as f32 / now.duration_since(last_frame_time).as_secs_f32();
                    frame_count = 0;
                    last_frame_time = now;
                }

                // 更新相机矩阵和投影模式
                renderer.update_camera(viewer.yaw, viewer.pitch, viewer.fov, viewer.projection_mode);

                // 渲染 UI 和 场景
                let mut next_image = None;
                let render_result = renderer.render_with_ui(&window, |ctx| {
                    draw_ui(ctx, &mut viewer, &mut next_image, &mut show_fps, &mut vsync_enabled, fps, is_loading, &window);
                });

                if let Some(path) = next_image {
                    is_loading = true;
                    start_load_image(path, tx.clone());
                }

                match render_result {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => renderer.resize(renderer.size),
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    Err(e) => eprintln!("Render error: {:?}", e),
                }
            }

            Event::MainEventsCleared => {
                window.request_redraw();
            }

            _ => {}
        }
    });
}

fn start_load_image(path: PathBuf, tx: Sender<image::RgbaImage>) {
    thread::spawn(move || {
        println!("后台加载图片: {:?}", path);
        
        // 使用 BufReader 优化 IO 读取性能
        let file = match File::open(&path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("无法打开文件: {}", e);
                return;
            }
        };
        let reader = BufReader::new(file);

        let img_result = ImageReader::new(reader)
            .with_guessed_format()
            .map_err(image::ImageError::IoError)
            .and_then(|mut r| {
                // 移除图片尺寸限制，允许加载任意大小的图片到内存
                r.no_limits();
                r.decode()
            });

        match img_result {
            Ok(img) => {
                let (w, h) = img.dimensions();
                println!("加载图片完成，尺寸: {}x{}", w, h);
                
                // 直接转换为 RGBA8 格式，不做任何缩放
                let rgba = img.to_rgba8();
                if tx.send(rgba).is_err() {
                    eprintln!("发送图片到主线程失败（主线程可能已退出）");
                }
            },
            Err(e) => eprintln!("无法解码图片: {}", e),
        }
    });
}

fn draw_ui(
    ctx: &egui::Context, 
    viewer: &mut PanoramaViewer3D, 
    next_image: &mut Option<PathBuf>,
    show_fps: &mut bool,
    vsync_enabled: &mut bool,
    fps: f32,
    is_loading: bool,
    window: &winit::window::Window,
) {
    // 自动隐藏 UI 逻辑可以这里添加，这里先保持常驻
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("文件", |ui| {
                if ui.button("打开图片 (O)...").clicked() {
                    ui.close_menu();
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("图片", &["jpg", "jpeg", "png", "bmp"])
                        .pick_file() 
                    {
                        *next_image = Some(path);
                    }
                }
                if ui.button("退出").clicked() {
                    std::process::exit(0);
                }
            });

            ui.menu_button("视图", |ui| {
                if ui.button("重置视图").clicked() {
                    viewer.yaw = 0.0;
                    viewer.pitch = 0.0;
                    viewer.fov = 46.8; // 50mm 默认视角（与 Java 版一致）
                    ui.close_menu();
                }
                
                // 全屏切换
                if ui.button(if viewer.is_fullscreen { "退出全屏 (F11)" } else { "全屏显示 (F11)" }).clicked() {
                    viewer.is_fullscreen = !viewer.is_fullscreen;
                    if viewer.is_fullscreen {
                        window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                    } else {
                        window.set_fullscreen(None);
                    }
                    ui.close_menu();
                }

                ui.separator();
                ui.menu_button("投影模式", |ui| {
                    if ui.radio_value(&mut viewer.projection_mode, ProjectionMode::Rectilinear, "标准透视 (Rectilinear)").clicked() { ui.close_menu(); }
                    if ui.radio_value(&mut viewer.projection_mode, ProjectionMode::Equidistant, "等距鱼眼 (Fisheye)").clicked() { ui.close_menu(); }
                    if ui.radio_value(&mut viewer.projection_mode, ProjectionMode::Stereographic, "小行星 (Little Planet)").clicked() { ui.close_menu(); }
                    if ui.radio_value(&mut viewer.projection_mode, ProjectionMode::Pannini, "帕尼尼 (Pannini)").clicked() { ui.close_menu(); }
                    if ui.radio_value(&mut viewer.projection_mode, ProjectionMode::Architectural, "建筑校正 (Architectural)").clicked() { ui.close_menu(); }
                    if ui.radio_value(&mut viewer.projection_mode, ProjectionMode::Equirectangular, "平面展开 (Equirectangular)").clicked() { ui.close_menu(); }
                });

                ui.separator();
                ui.menu_button("输入灵敏度", |ui| {
                     ui.add(egui::Slider::new(&mut viewer.sensitivity_scale, 0.1..=5.0).text("倍率"));
                     if ui.button("重置 (1.0)").clicked() {
                         viewer.sensitivity_scale = 1.0;
                     }
                });
                ui.separator();
                if ui.checkbox(show_fps, "显示帧率 (FPS)").clicked() {
                    ui.close_menu();
                }
                if ui.checkbox(vsync_enabled, "启用垂直同步 (VSync)").clicked() {
                     // TODO: Reconfigure
                }
            });
        });
    });

    egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            if is_loading {
                ui.label(egui::RichText::new("正在加载图片...").color(egui::Color32::YELLOW));
                ui.label("|");
            }

            ui.label(format!("模式: {:?}", viewer.projection_mode));
            ui.label("|");
            ui.label(format!("FOV: {:.1}°", viewer.fov));
            ui.label("|");
            // 计算 35mm 全画幅等效焦距（假设 viewer.fov 表示对角视角）
            {
                let fov_deg = viewer.fov.clamp(0.01, 179.9);
                let fov_rad = fov_deg.to_radians();
                // 35mm 全画幅对角线 (36x24 mm)
                let full_frame_diag = ((36.0f32 * 36.0f32) + (24.0f32 * 24.0f32)).sqrt(); // ≈ 43.2666 mm
                let equiv_focal = full_frame_diag / (2.0 * (fov_rad * 0.5).tan());
                ui.label(format!("35mm 等效焦距: {:.1}mm", equiv_focal));
            }
            ui.label("|");
            ui.label(format!("Yaw: {:.1}°", viewer.yaw));
            ui.label("|");
            ui.label(format!("Pitch: {:.1}°", viewer.pitch));
            
            if *show_fps {
                ui.label("|");
                ui.label(egui::RichText::new(format!("FPS: {:.1}", fps)).color(egui::Color32::GREEN));
            }
        });
    });
}
