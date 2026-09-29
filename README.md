# 喵尺 MeowFit

> 一款仅面向 **Windows** 的本地媒体尺寸批量调整工具。

喵尺 MeowFit 用于批量调整图片、动图与视频的尺寸。全部处理在本机完成，
**程序不发起任何网络请求**：不上传素材、不收集遥测、不检查更新、不需账号。

本仓库为**源码可见的专有软件**。许可条款见 [LICENSE](LICENSE)。

> **当前状态：开发中（P4 阶段已完成，验收场景 1–19 均已实测通过）。**
> 已可用：素材导入与扫描（递归 / 仅当前层、包含排除通配符、最近文件夹）、三种自动分组与手动修正、
> 三级参数优先级、尺寸计算（模式 A–G）、实时预览；静态位图处理（重采样算法可选、EXIF 方向校正、
> ICC 保留、元数据开关、格式转换、SVG 改写属性或按像素尺寸光栅化）；视频处理（H.264 / H.265 /
> VP9 / AV1、CRF、音频直接复制、字幕与章节保留、旋转元数据、HDR 传递或色调映射、硬件加速与回退）；
> 外观设置（跟随系统、配色、背景图、拖拽自定义布局）。
> 动图处理（GIF / 动态 WebP / APNG：帧与调色板保留、颜色数 256/128/64、抖动、转 MP4/WebM、按目标体积三档逼近）。
> 尚未接入：输出设置与报告导出（P5）、打包（P6）。
>
> **视频功能需要 FFmpeg**：仓库不附带二进制，可用 `scripts/build-ffmpeg-gpl.sh` 自行构建，
> 或按下方「FFmpeg」一节放置。

---

## 目标平台

- 仅 **Windows**，64 位，Windows 10 1809 及以上。
- 不提供 macOS 版、Linux 版。
- 不提供命令行接口、右键菜单集成、文件关联。
- 提供两种分发形态，功能完全一致：**Portable 绿色版**（解压即用）与 **安装版**（NSIS 安装程序）。

## 支持的素材格式

| 类别 | 格式 |
| --- | --- |
| 静态图片 | PNG、JPG / JPEG、WebP、BMP、TIFF、AVIF、HEIC / HEIF、ICO、SVG |
| 动图 | GIF、动态 WebP、APNG |
| 视频 | MP4、MOV、MKV、AVI、WebM、M4V、FLV、WMV、MPEG / MPG、TS |

## 技术栈

| 层 | 选型 |
| --- | --- |
| 桌面框架 | Tauri 2 |
| 前端 | React + TypeScript + Vite |
| 样式 | 纯 CSS + CSS 自定义属性（浅色 / 深色两套变量） |
| 核心 | Rust（edition 2021） |
| 静态图片 | `image` crate |
| SVG | `resvg` crate |
| 视频 / 动图 | FFmpeg（GPL 构建，独立进程调用） |
| 打包 | Windows NSIS |

应用标识（bundle identifier）：`com.susu147226.meowfit`

## 开发

```bash
npm install
```

```bash
npm run tauri dev
```

## 构建

```bash
npm run tauri build
```

## 图标

图标由作者提供，已应用到窗口图标与可执行文件图标。

`src-tauri/icons/icon.ico` 直接采用作者提供的原始文件——它本身已含
规范 7.1 要求的全部七种尺寸（16、24、32、48、64、128、256）。
其余 PNG 资源（32、64、128、128@2x、各尺寸 Store 徽标）由
`npm run tauri icon` 从其中的 256×256 帧生成。

**注意**：`tauri icon` 生成出来的 `.ico` 只有 6 种尺寸（缺 128），
若日后需要重新生成，记得用 `src-tauri/icons/source.ico` 把 `icon.ico` 覆盖回来。
该命令还会顺带生成 `android/`、`ios/` 目录，本项目仅面向 Windows，可直接删除。

## FFmpeg

视频与动图处理依赖 FFmpeg。本项目采用 **GPL 构建的 FFmpeg，以独立进程方式调用**
（不链接、不修改其源码）。FFmpeg 二进制**不纳入版本控制**（体积较大）。

**放置路径：**

```
src-tauri/resources/ffmpeg/win-x64/
├── ffmpeg.exe
├── ffprobe.exe
└── libgcc_s_seh-1.dll      # 静态构建后仅剩的运行时依赖（GCC 运行时）
```

**获取方式：** 仓库不附带二进制（体积较大）。可用 `scripts/build-ffmpeg-gpl.sh` 自行构建，
该脚本在 MSYS2 MINGW64 下运行，产出 FFmpeg **9.0.2**，静态链接 libx264 / libx265 /
libvpx / libaom / zimg / libwebp。脚本头部写明了完整的依赖安装命令与注意事项。

打包前需完成 FFmpeg 构建自检，确认所用构建包含 `libx264`、`libx265`、
`libvpx-vp9`、`libaom-av1` 编码器，`palettegen`、`paletteuse`、`zscale`、
`tonemap` 滤镜，以及 `hevc` 解码器。程序启动时也会自动执行这份自检，
缺少组件会在界面与日志中给出明确提示，而不是静默禁用相关功能。

**开发期便利**：调试构建下若上述位置没有二进制，会回退到 PATH 上的
`ffmpeg.exe` / `ffprobe.exe` 并在日志中说明；发布构建**不会**使用 PATH，
以免掩盖打包漏带二进制的问题。

## 许可与致谢

**本项目代码**：喵尺 MeowFit 自身的源代码与文档适用 [LICENSE](LICENSE) 中的私有许可证，
仅允许个人非商业用途，**禁止再分发**。

**随包的 FFmpeg**：本软件包内 `resources/ffmpeg/` 目录下的 FFmpeg 可执行文件为独立的第三方程序，
适用 **GPL** 许可证，**不受本项目私有许可证约束**。GPL 赋予您对该 FFmpeg 部分的复制与再分发权利；
该权利仅作用于 FFmpeg 文件本身，不延伸至喵尺 MeowFit 的其他文件。

FFmpeg 完整源码获取地址（含随包构建所启用的全部组件）：

- FFmpeg 9.0.2 — https://ffmpeg.org/releases/
- x264 — https://code.videolan.org/videolan/x264
- x265 4.3 — https://bitbucket.org/multicoreware/x265_git
- libvpx 1.17.0 — https://chromium.googlesource.com/webm/libvpx
- libaom 3.15.1 — https://aomedia.googlesource.com/aom
- zimg 3.0.6 — https://github.com/sekrit-twc/zimg
- libwebp 1.6.0 — https://chromium.googlesource.com/webm/libwebp
- MSYS2 各依赖包的构建配方 — https://github.com/msys2/MINGW-packages

---

© 2026 云舒眠眠。保留所有权利。
