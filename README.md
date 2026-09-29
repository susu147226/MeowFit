# 喵尺 MeowFit

> 一款仅面向 **Windows** 的本地媒体尺寸批量调整工具。

喵尺 MeowFit 用于批量调整图片、动图与视频的尺寸。全部处理在本机完成，
**程序不发起任何网络请求**：不上传素材、不收集遥测、不检查更新、不需账号。

本仓库为**源码可见的专有软件**。许可条款见 [LICENSE](LICENSE)。

> **当前状态：功能已全部实现（P0–P6 完成）。**
> 已可用：素材导入与扫描（递归 / 仅当前层、包含排除通配符、最近文件夹）、三种自动分组与手动修正、
> 三级参数优先级、尺寸计算（模式 A–G）、实时预览；静态位图处理（重采样算法可选、EXIF 方向校正、
> ICC 保留、元数据开关、格式转换、SVG 改写属性或按像素尺寸光栅化）；视频处理（H.264 / H.265 /
> VP9 / AV1、CRF、音频直接复制、字幕与章节保留、旋转元数据、HDR 传递或色调映射、硬件加速与回退）；
> 外观设置（跟随系统、配色、背景图、拖拽自定义布局）。
> 动图处理（GIF / 动态 WebP / APNG：帧与调色板保留、颜色数 256/128/64、抖动、转 MP4/WebM、按目标体积三档逼近）；
> 输出设置（两种输出目录方式、覆盖源文件二次确认、干跑、备份、冲突策略）、处理报告（CSV / JSON）、
> 日志落盘、增量处理、体积目标阶梯、预设系统。
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

产出两种分发形态：

| 形态 | 产物 | 说明 |
| --- | --- | --- |
| 安装版 | `src-tauri/target/release/bundle/nsis/MeowFit_<版本>_x64-setup.exe` | NSIS 安装程序，当前用户安装，无需管理员权限运行 |
| Portable 绿色版 | `artifacts/MeowFit-portable-v<版本>.zip` | 解压即用；由 `scripts/package-portable.ps1` 生成 |

```bash
powershell -ExecutionPolicy Bypass -File scripts/package-portable.ps1
```

Portable 包的目录结构为 `MeowFit.exe`、`resources/ffmpeg/win-x64/`、`config/`、
`LICENSE`、`README.md`。它在只读介质上启动时会自动把配置降级到系统临时目录，
并在界面顶部提示「配置未能持久化」。

**打包前**请先完成 FFmpeg 的 12.5 自检；程序启动时也会自动执行这份自检。

## 测试

```bash
npm test
```

```bash
cd src-tauri && cargo test
```

Rust 侧包含规范 16.2 全部 33 条验收场景中可在本机自动化的部分，
以及尺寸计算、分组、优先级、动图、元数据、报告、预设等单元测试。

## 输出与执行

| 能力 | 说明 |
| --- | --- |
| 输出目录 | 两种方式并存：**默认**在源文件夹的同级建立 `output/`（输入 `D:\素材\图标` → 输出 `D:\素材\output`）；也可每次任务单独指定。两者都不可写时报 `E_OUTPUT_UNWRITABLE` 并引导改用指定目录 |
| 覆盖源文件 | 默认**关闭**；开启后写回原路径，且执行前弹出二次确认，列出将被覆盖的文件数量与路径，取消则不做任何操作 |
| 备份 | 可选「处理前备份源文件」，复制到 `output/.meowfit-backup/`，保留相对结构 |
| 同名冲突 | 跳过（默认）／覆盖／自动重命名（追加 `-1`） |
| 干跑模式 | 只计算与预览，**不写出任何文件**，但仍给出完整结论 |
| 输出校验 | 写出后按**输出格式**判定能否正常解码，不合格的输出会删除并计入「失败」 |
| 增量处理 | 记录上次结果，可跳过未变化的素材；判定依据为**体积 + 修改时间 + 设置指纹**三者全一致 |
| 进度与取消 | 处理过程实时显示进度，可取消；取消后已完成的保留、未开始的停止 |
| 磁盘预检 | 执行前读取目标磁盘剩余空间，供与预计输出体积比对 |

写出时一律先落临时文件再改名，因此开启「覆盖源文件」也不会边读边写。

## 报告与日志

- 每次任务在输出目录生成 `meowfit-report-YYYYMMDD-HHmmss.csv` 与同名 `.json`，
  字段与顺序固定为：源文件路径、输出文件路径、类型、所属分组、原宽度、原高度、
  目标宽度、目标高度、原体积、新体积、使用方式、状态、错误原因、耗时ms。
  CSV 带 BOM，Excel 打开中文表头不乱码。
- 状态只有四类：**成功 / 未改动 / 已跳过 / 失败**。扫描阶段就不支持的格式也会
  出现在「已跳过」清单里。
- 日志写入 `config/../logs/meowfit-YYYY-MM-DD.log`，格式 `[时间] [级别] 消息`，
  按日期滚动，**保留 30 天**后自动清理。

## 体积目标

以「目标体积」为约束时按阶梯逐级逼近，**绝不无限循环**：

- 图片：降质量（每次 −10，下限 40）→ 降尺寸（按 0.9 逐级，最多 8 级，下限 64px）；
- 动图：减色数（256 → 128 → 64）→ 降尺寸 → 丢帧（最多降至原帧率的一半）；
- 视频：降质量 → 降尺寸 → 降帧率。

每一档的尝试结果都会记录下来并展示；若档位耗尽仍超标，保留其中体积最小的一次输出，
并在报告中明确标注「未达标」。

## 预设

- 内置预设 8 个：1080P、720P、4K、1080×1920、1080×1080、1080×2640、2640×2640、640×640。
- 内置预设**可隐藏但不可删除**；自定义预设可新增、重命名、删除。
- 以 JSON 保存在 `config/presets.json`，便于自行编辑；支持导出与导入。

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
