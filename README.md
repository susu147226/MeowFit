# 喵尺 MeowFit

> 一款仅面向 **Windows** 的本地媒体尺寸批量调整工具。

喵尺 MeowFit 用于批量调整图片、动图与视频的尺寸。全部处理在本机完成，
**程序不发起任何网络请求**：不上传素材、不收集遥测、不检查更新、不需账号。

本仓库为**源码可见的专有软件**。许可条款见 [LICENSE](LICENSE)。

> **当前状态：开发中（P0 阶段）。** 工程骨架已就位，功能尚未实现。

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

仓库内当前使用**占位图标**（Tauri 默认图标），**不是**最终图标。

最终图标由作者提供，设计要求为「一只奶油色小猫抱着软尺，尾巴卷成刻度」，
需打包为多尺寸 `.ico`（16、24、32、48、64、128、256 px），并应用到窗口图标、
可执行文件图标、安装程序图标、界面左上角 Logo 与 README 首图。

替换步骤：将作者提供的图标放到 `src-tauri/icons/` 覆盖现有文件，然后执行
`npm run tauri icon <源图路径>` 重新生成全部尺寸，并重新打包。

## FFmpeg

视频与动图处理依赖 FFmpeg。本项目采用 **GPL 构建的 FFmpeg，以独立进程方式调用**
（不链接、不修改其源码）。FFmpeg 二进制**不纳入版本控制**（体积较大）。

**放置路径：**

```
src-tauri/resources/ffmpeg/win-x64/
├── ffmpeg.exe
└── ffprobe.exe
```

打包前需完成 FFmpeg 构建自检，确认所用构建包含 `libx264`、`libx265`、
`libvpx-vp9`、`libaom-av1` 编码器，`palettegen`、`paletteuse`、`zscale`、
`tonemap` 滤镜，以及 `hevc` 解码器。

## 许可与致谢

**本项目代码**：喵尺 MeowFit 自身的源代码与文档适用 [LICENSE](LICENSE) 中的私有许可证，
仅允许个人非商业用途，**禁止再分发**。

**随包的 FFmpeg**：本软件包内 `resources/ffmpeg/` 目录下的 FFmpeg 可执行文件为独立的第三方程序，
适用 **GPL** 许可证，**不受本项目私有许可证约束**。GPL 赋予您对该 FFmpeg 部分的复制与再分发权利；
该权利仅作用于 FFmpeg 文件本身，不延伸至喵尺 MeowFit 的其他文件。
FFmpeg 完整源码获取地址：`【构建时填入所采用的上游发行地址】`。

---

© 2026 云舒眠眠。保留所有权利。
