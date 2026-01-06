# 全景照片查看器（Rust / wgpu / egui）

[English](./README.md)

这是一个使用 Rust 编写、由 **wgpu + winit + egui** 构建的 GPU 加速全景照片查看器。  
支持多种投影模式（标准透视 / 鱼眼 / 小行星 / 帕尼尼 / 建筑校正 / 等矩形展开），并提供鼠标交互控制。

## 功能特性

- 使用 `wgpu` **GPU 渲染**（Fragment Shader 全屏 Ray Casting）
- 基于 egui 的 UI：**菜单栏 + 状态栏**
- **异步加载图片**（后台线程），避免卡顿
- 支持 **拖拽文件** 到窗口加载
- **多投影模式**
  - 标准透视（Rectilinear）
  - 等距鱼眼（Equidistant / Fisheye）
  - 小行星（Stereographic / Little Planet）
  - 帕尼尼（Pannini）
  - 建筑校正（Architectural）
  - 等矩形展开（Equirectangular / 原图展开）
- **视图交互**
  - 鼠标左键拖拽：旋转（Yaw/Pitch）
  - 鼠标滚轮：缩放（FOV）
  - 重置视角 / 全屏切换
- **大图处理**
  - 当图片尺寸超过 GPU 最大纹理限制时会自动缩放
  - 对非 2:1 的图片：会补黑到 2:1 画布，以兼容等矩形采样

## 截图

> 建议补充项目截图：
>
> `![screenshot](./docs/screenshot.png)`

## 环境要求

- Rust 工具链（edition 2021）
- 支持 `wgpu` 的显卡与驱动（不同系统对应 Vulkan/DirectX/Metal 等后端）

## 构建与运行

```bash
# 编译
cargo build

# 运行
cargo run

# Release 编译
cargo build --release
```

## 使用说明

### 打开图片

- 菜单：**文件 → 打开图片 (O)...**
- 快捷键：按 **O**
- 或者：将图片文件 **拖拽到窗口**

支持格式：`jpg/jpeg/png/bmp`（由 `image` crate 提供解码）

### 操作方式

- **旋转**：按住 **鼠标左键** 拖拽
- **缩放（调整 FOV）**：滚轮
- **全屏**：**F11**
- **重置视角**：视图 → 重置视图

### 投影模式切换

在菜单：**视图 → 投影模式**。

## 字体与多语言（i18n）

本项目 UI 使用 **egui**，文字渲染效果取决于 egui 可用的字体：

- **拉丁字母 / 基础 ASCII**：通常可直接使用 egui 默认字体正常显示
- **CJK（中/日/韩）及其它文字系统**：一般需要提供包含对应字形（glyph）的字体文件
- **Emoji**：通常需要专门的 emoji 字体（本项目默认未专门处理）

### 运行时字体加载策略（当前实现）

渲染器会在运行时尝试查找字体，并将其注册为 egui 的最高优先级字体：

- 优先尝试 `./assets/`（推荐，保证跨平台一致性）
  - `assets/NotoSansSC-Regular.ttf`
  - `assets/NotoSansSC-Regular.otf`
- 若未找到，再尝试系统字体目录（Windows/macOS/Linux 常见路径）中的少量常见文件名

说明：

- `ab_glyph` 对 `.ttc` 字体集合的解析不一定稳定，因此优先推荐 **`.ttf/.otf`**。
- 如果找不到可用字体，非拉丁字符可能会显示为方块（□）。

### 推荐做法（跨平台）

建议将支持 Unicode 的字体放在 `assets/` 目录（开发时放在项目根目录的 `assets/`，发布时放在 exe 同目录的 `assets/`），例如：

- **中文**：`NotoSansSC-Regular.ttf` / `SimHei.ttf` / `Microsoft YaHei`（如能获得 `.ttf`）
- **日文/韩文**：`NotoSansCJK-Regular.ttf`（或任何包含 JP/KR 字形的 `.ttf/.otf`）
- **多语言（体积较大）**：Noto CJK 或其它泛 Unicode 字体

如需更完整的多语言覆盖，可在 `src/renderer.rs` 中扩展字体搜索列表，或加载多个字体并按顺序注册到 egui（当前函数名为 `setup_egui_chinese_fonts`，但你可以按需要改造成通用字体加载器）。

## 项目结构

- `src/main.rs` — 窗口/事件循环、输入交互、菜单/状态栏、异步加载图片
- `src/panorama.rs` — 相机参数与 `ProjectionMode`
- `src/renderer.rs` — wgpu 渲染器 + egui 集成 + 纹理上传
- `src/shader_equirect.wgsl` — 投影 shader（全屏 ray casting）

## License / 许可证

当前仓库尚未包含 `LICENSE` 文件。

如果计划发布到 GitHub，建议添加 `LICENSE`（如 MIT / Apache-2.0 等），并在此处更新许可证说明。
