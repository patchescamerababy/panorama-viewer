// main.rs — 完整的 Rust 实现，包含菜单、状态栏和 3D 交互

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // 在 Release 模式下隐藏控制台窗口

mod panorama;
mod renderer;
mod i18n;

use panorama::{PanoramaViewer3D, ProjectionMode};
use renderer::Renderer;

use winit::{
    dpi::{LogicalSize, PhysicalPosition},
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Fullscreen, WindowBuilder},
};

use image::io::Reader as ImageReader;
use image::GenericImageView;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

fn main() {
    // env_logger::init(); // 在 Windows Subsystem 下标准输出不可见，可以考虑写入文件日志

    // i18n
    let mut current_lang = crate::i18n::resolve_lang_from_args();
    crate::i18n::init(current_lang.clone());

    let event_loop = EventLoop::new();
    let window = Arc::new(
        WindowBuilder::new()
            .with_title(&crate::i18n::tr("app.title"))
            .with_inner_size(LogicalSize::new(1280, 720))
            .build(&event_loop)
            .unwrap(),
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
                                        .add_filter(
                                            &crate::i18n::tr("file.filter.images"),
                                            &["jpg", "jpeg", "png", "bmp"],
                                        )
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

                                let width = renderer.size.width as f32;
                                let height = renderer.size.height as f32;

                                if width > 0.0 && height > 0.0 {
                                    let v_f = viewer.fov.to_radians();
                                    let aspect = width / height;
                                    let h_f = 2.0 * ((v_f / 2.0).tan() * aspect).atan();

                                    let yaw_per_px_deg = (h_f / width).to_degrees();
                                    let pitch_per_px_deg = (v_f / height).to_degrees();

                                    viewer.yaw -= dx * yaw_per_px_deg * viewer.sensitivity_scale;
                                    viewer.pitch = (viewer.pitch
                                        - dy * pitch_per_px_deg * viewer.sensitivity_scale)
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

                        let min_fov = if viewer.projection_mode == ProjectionMode::Stereographic {
                            10.0
                        } else {
                            5.0
                        };

                        let max_fov = match viewer.projection_mode {
                            ProjectionMode::Rectilinear
                            | ProjectionMode::Pannini
                            | ProjectionMode::Architectural => 179.9,
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
                    draw_ui(
                        ctx,
                        &mut viewer,
                        &mut next_image,
                        &mut show_fps,
                        &mut vsync_enabled,
                        fps,
                        is_loading,
                        &window,
                        &mut current_lang,
                    );
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
        println!(
            "{}",
            crate::i18n::tr_with("log.loading_image_bg", &[("path", format!("{:?}", path))])
        );

        let file = match File::open(&path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!(
                    "{}",
                    crate::i18n::tr_with("error.open_file", &[("err", format!("{}", e))])
                );
                return;
            }
        };
        let reader = BufReader::new(file);

        let img_result = ImageReader::new(reader)
            .with_guessed_format()
            .map_err(image::ImageError::IoError)
            .and_then(|mut r| {
                r.no_limits();
                r.decode()
            });

        match img_result {
            Ok(img) => {
                let (w, h) = img.dimensions();
                println!(
                    "{}",
                    crate::i18n::tr_with(
                        "log.image_loaded_size",
                        &[("w", w.to_string()), ("h", h.to_string())]
                    )
                );

                let rgba = img.to_rgba8();
                if tx.send(rgba).is_err() {
                    eprintln!("{}", crate::i18n::tr("error.send_to_main_failed"));
                }
            }
            Err(e) => eprintln!(
                "{}",
                crate::i18n::tr_with("error.decode_image", &[("err", format!("{}", e))])
            ),
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
    current_lang: &mut String,
) {
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            // File
            ui.menu_button(&crate::i18n::tr("menu.file"), |ui| {
                if ui.button(&crate::i18n::tr("menu.open_image")).clicked() {
                    ui.close_menu();
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter(&crate::i18n::tr("file.filter.images"), &["jpg", "jpeg", "png", "bmp"])
                        .pick_file()
                    {
                        *next_image = Some(path);
                    }
                }
                if ui.button(&crate::i18n::tr("menu.exit")).clicked() {
                    std::process::exit(0);
                }
            });

            // View
            ui.menu_button(&crate::i18n::tr("menu.view"), |ui| {
                if ui.button(&crate::i18n::tr("view.reset")).clicked() {
                    viewer.yaw = 0.0;
                    viewer.pitch = 0.0;
                    viewer.fov = 46.8;
                    ui.close_menu();
                }

                if ui
                    .button(if viewer.is_fullscreen {
                        crate::i18n::tr("view.fullscreen.exit")
                    } else {
                        crate::i18n::tr("view.fullscreen.enter")
                    })
                    .clicked()
                {
                    viewer.is_fullscreen = !viewer.is_fullscreen;
                    if viewer.is_fullscreen {
                        window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                    } else {
                        window.set_fullscreen(None);
                    }
                    ui.close_menu();
                }

                ui.separator();
                ui.menu_button(&crate::i18n::tr("view.projection_mode"), |ui| {
                    if ui
                        .radio_value(
                            &mut viewer.projection_mode,
                            ProjectionMode::Rectilinear,
                            crate::i18n::tr("projection.rectilinear"),
                        )
                        .clicked()
                    {
                        ui.close_menu();
                    }
                    if ui
                        .radio_value(
                            &mut viewer.projection_mode,
                            ProjectionMode::Equidistant,
                            crate::i18n::tr("projection.equidistant"),
                        )
                        .clicked()
                    {
                        ui.close_menu();
                    }
                    if ui
                        .radio_value(
                            &mut viewer.projection_mode,
                            ProjectionMode::Stereographic,
                            crate::i18n::tr("projection.stereographic"),
                        )
                        .clicked()
                    {
                        ui.close_menu();
                    }
                    if ui
                        .radio_value(
                            &mut viewer.projection_mode,
                            ProjectionMode::Pannini,
                            crate::i18n::tr("projection.pannini"),
                        )
                        .clicked()
                    {
                        ui.close_menu();
                    }
                    if ui
                        .radio_value(
                            &mut viewer.projection_mode,
                            ProjectionMode::Architectural,
                            crate::i18n::tr("projection.architectural"),
                        )
                        .clicked()
                    {
                        ui.close_menu();
                    }
                    if ui
                        .radio_value(
                            &mut viewer.projection_mode,
                            ProjectionMode::Equirectangular,
                            crate::i18n::tr("projection.equirectangular"),
                        )
                        .clicked()
                    {
                        ui.close_menu();
                    }
                });

                ui.separator();
                ui.menu_button(&crate::i18n::tr("view.input_sensitivity"), |ui| {
                    ui.add(
                        egui::Slider::new(&mut viewer.sensitivity_scale, 0.1..=5.0)
                            .text(crate::i18n::tr("view.multiplier")),
                    );
                    if ui.button(&crate::i18n::tr("view.reset_1_0")).clicked() {
                        viewer.sensitivity_scale = 1.0;
                    }
                });

                ui.separator();
                if ui.checkbox(show_fps, crate::i18n::tr("view.show_fps")).clicked() {
                    ui.close_menu();
                }
                if ui
                    .checkbox(vsync_enabled, crate::i18n::tr("view.enable_vsync"))
                    .clicked()
                {
                    // TODO: Reconfigure
                }
            });

            // Language
            ui.menu_button(&crate::i18n::tr("menu.language"), |ui| {
                let langs: [(&str, &str); 8] = [
                    ("zh-Hans", "简体中文"),
                    ("zh-Hant", "繁體中文"),
                    ("en", "English"),
                    ("ja", "日本語"),
                    ("ko", "한국어"),
                    ("fr", "Français"),
                    ("ru", "Русский"),
                    ("ar", "العربية"),
                ];

                for (code, name) in langs {
                    if ui.radio_value(current_lang, code.to_string(), name).clicked() {
                        crate::i18n::init(current_lang.clone());
                        window.set_title(&crate::i18n::tr("app.title"));
                        ui.close_menu();
                    }
                }
            });
        });
    });

    egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            if is_loading {
                ui.label(
                    egui::RichText::new(crate::i18n::tr("status.loading_image"))
                        .color(egui::Color32::YELLOW),
                );
                ui.label("|");
            }

            ui.label(format!(
                "{} {:?}",
                crate::i18n::tr("status.mode_prefix"),
                viewer.projection_mode
            ));
            ui.label("|");
            ui.label(format!("FOV: {:.1}°", viewer.fov));
            ui.label("|");

            {
                let fov_deg = viewer.fov.clamp(0.01, 179.9);
                let fov_rad = fov_deg.to_radians();
                let full_frame_diag = ((36.0f32 * 36.0f32) + (24.0f32 * 24.0f32)).sqrt();
                let equiv_focal = full_frame_diag / (2.0 * (fov_rad * 0.5).tan());
                ui.label(format!(
                    "{} {:.1}mm",
                    crate::i18n::tr("status.equiv_focal_prefix"),
                    equiv_focal
                ));
            }

            ui.label("|");
            ui.label(format!("Yaw: {:.1}°", viewer.yaw));
            ui.label("|");
            ui.label(format!("Pitch: {:.1}°", viewer.pitch));

            if *show_fps {
                ui.label("|");
                ui.label(
                    egui::RichText::new(format!("FPS: {:.1}", fps)).color(egui::Color32::GREEN),
                );
            }
        });
    });
}
