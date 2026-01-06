# Panorama Viewer (Rust / wgpu / egui)

[中文](./README-zh_cn.md)

A GPU-accelerated panorama photo viewer written in Rust, built on **wgpu + winit + egui**.  
Supports multiple projection modes (rectilinear / fisheye / little planet / pannini / architectural correction / equirectangular) with interactive mouse controls.

## Features

- **GPU rendering** via `wgpu` (fullscreen ray-casting in fragment shader)
- **Egui UI** menu bar + status bar
- **Async image loading** (background thread) to avoid UI stalls
- **Drag & drop** to load images
- **Projection modes**
  - Rectilinear (standard perspective)
  - Equidistant (fisheye)
  - Stereographic (little planet)
  - Pannini
  - Architectural correction
  - Equirectangular (flat view)
- **View controls**
  - Mouse drag to rotate (yaw/pitch)
  - Mouse wheel to zoom (FOV)
  - Reset view / fullscreen toggle
- **Large image handling**
  - Auto downscale if texture size exceeds GPU limits
  - Non-2:1 textures are padded to a 2:1 canvas for equirectangular sampling

## Screenshot

> Add your screenshot here (recommended):
>
> `![screenshot](./docs/screenshot.png)`

## Requirements

- Rust toolchain (edition 2021)
- A GPU/driver that supports `wgpu` (Vulkan/DirectX/Metal depending on OS)

## Build & Run

```bash
# build
cargo build

# run
cargo run

# release build
cargo build --release
```

## How to Use

### Open an image

- Menu: **File → Open (O)...**
- Shortcut: press **O**
- Or **drag & drop** an image file into the window

Supported formats: `jpg/jpeg/png/bmp` (via the `image` crate)

### Controls

- **Rotate**: hold **Left Mouse Button** and drag
- **Zoom (FOV)**: mouse wheel
- **Fullscreen**: **F11**
- **Reset view**: View → Reset

### Projection Modes

In the menu: **View → Projection Mode**.

## Fonts / Internationalization (i18n)

This project uses **egui** for the UI. Text rendering depends on fonts available to egui:

- **Latin / basic ASCII**: usually works out-of-the-box with egui defaults.
- **CJK (Chinese/Japanese/Korean) and other scripts**: may require providing a font that contains the needed glyphs.
- **Emoji**: may require a dedicated emoji font (not handled by this project by default).

### Runtime font loading strategy (current implementation)

The renderer currently tries to locate a font at runtime and registers it into egui as the highest-priority font:

- First try `./assets/` (recommended for consistent cross-platform rendering)
  - `assets/NotoSansSC-Regular.ttf`
  - `assets/NotoSansSC-Regular.otf`
- Then try common system font locations (Windows/macOS/Linux) with a small set of well-known filenames.

Notes:

- `ab_glyph` parsing for `.ttc` collections can be unreliable, so **`.ttf/.otf`** are preferred.
- If no usable font is found, non-Latin characters may render as tofu (□).

### Recommended setup (cross-platform)

Place a Unicode font in `assets/` next to the executable (or in the project root during development). For example:

- **Chinese**: `NotoSansSC-Regular.ttf` / `SimHei.ttf` / `Microsoft YaHei` (exported as `.ttf` if available)
- **Japanese/Korean**: `NotoSansCJK-Regular.ttf` (or other `.ttf/.otf` with JP/KR glyphs)
- **Multi-language (large file)**: a Noto CJK or other pan-Unicode font

If you need full multi-language support, adjust the search list and/or load multiple fonts in `src/renderer.rs` (`setup_egui_chinese_fonts`) and register them into egui font families.

## Project Structure

- `src/main.rs` — window/event loop, input handling, menus/status bar, async image loading
- `src/panorama.rs` — camera parameters and `ProjectionMode`
- `src/renderer.rs` — wgpu renderer + egui integration + texture upload
- `src/shader_equirect.wgsl` — projection shader (fullscreen ray-casting)

## License

No license file is included in this repository yet.

If you plan to publish this project on GitHub, add a `LICENSE` (MIT/Apache-2.0/etc.) and update this section accordingly.
