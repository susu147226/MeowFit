//! 任务执行：解析输出目录、映射输出路径、冲突处理、逐文件写出。
//!
//! P1 阶段只覆盖使验收场景 1–8 可实测所需的基础路径：
//! 默认「源文件夹同级 output/」、保留相对目录结构、输出保持原名、默认不覆盖源文件。
//! 干跑、备份、磁盘预检、输出校验、报告导出、增量处理见 P5。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, ErrorCode};
use crate::ffmpeg;
use crate::imaging;
use crate::model::{Action, Plan, Status};
use crate::scan;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRef {
    pub id: String,
    /// 源文件绝对路径
    pub path: String,
    pub kind: crate::model::MediaKind,
    /// 动图的循环次数，执行时原样写回；非动图为 None
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_count: Option<u32>,
    /// 动图源扩展名（转视频时要看原格式决定滤镜）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<String>,
    /// 扫描阶段就判定要跳过的原因（规范 5.5）；这类素材不进入计划，
    /// 但必须出现在报告的「已跳过」清单里（规范 11.3 / 场景 27）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_reason: Option<String>,
}

/// 视频处理参数（规范 6.9）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoOptions {
    /// h264 | h265 | vp9 | av1（规范 6.9：四种编码器全部提供）
    #[serde(default = "default_codec")]
    pub codec: String,
    #[serde(default = "default_crf")]
    pub crf: u32,
    #[serde(default = "default_preset")]
    pub preset: String,
    #[serde(default)]
    pub hardware: bool,
    /// nvenc | qsv | amf
    #[serde(default = "default_accel")]
    pub accel: String,
    /// 默认保持 HDR 原样传递；开启后色调映射到 SDR
    #[serde(default)]
    pub tonemap_to_sdr: bool,
}

fn default_codec() -> String {
    "h264".into()
}
fn default_crf() -> u32 {
    23
}
fn default_preset() -> String {
    "medium".into()
}
fn default_accel() -> String {
    "nvenc".into()
}

impl Default for VideoOptions {
    fn default() -> Self {
        Self {
            codec: default_codec(),
            crf: default_crf(),
            preset: default_preset(),
            hardware: false,
            accel: default_accel(),
            tonemap_to_sdr: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecOptions {
    /// `None` 表示使用默认的「源文件夹同级 output/」
    #[serde(default)]
    pub output_dir: Option<String>,
    #[serde(default = "default_true")]
    pub keep_structure: bool,
    /// skip | overwrite | rename
    #[serde(default = "default_conflict")]
    pub on_conflict: String,
    /// 图片处理参数：重采样、质量、输出格式、元数据开关、背景色
    #[serde(default)]
    pub image: imaging::ImageOptions,
    /// 视频处理参数：编码器、质量、硬件加速、HDR 策略
    #[serde(default)]
    pub video: VideoOptions,
    /// 动图处理参数：颜色数、抖动、是否转视频（规范 6.10）
    #[serde(default)]
    pub animation: AnimationOptions,
    /// 干跑：只计算与预览，不写出任何文件（规范 6.5）
    #[serde(default)]
    pub dry_run: bool,
    /// 处理前先把源文件备份到 output/.meowfit-backup/（规范 6.5）
    #[serde(default)]
    pub backup: bool,
    /// 覆盖源文件（规范 6.5；默认关闭，界面须二次确认）
    #[serde(default)]
    pub overwrite_source: bool,
    /// 写出后自动校验能否正常解码，失败则删除输出（规范 6.7）
    #[serde(default = "default_true")]
    pub verify_output: bool,
    /// 增量处理：跳过未变化的素材（规范 6.13）
    #[serde(default)]
    pub skip_unchanged: bool,
    /// 配置目录：用于写日志与增量索引；为 None 时不做这两件事
    #[serde(default)]
    pub config_dir: Option<String>,
}

/// 动图处理参数（规范 6.10）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationOptions {
    /// 调色板颜色数：256 / 128 / 64
    #[serde(default = "default_gif_colors")]
    pub colors: u32,
    #[serde(default)]
    pub dither: bool,
    /// none | mp4 | webm —— 「GIF 转 MP4 / WebM」
    #[serde(default = "default_to_video")]
    pub to_video: String,
    /// 目标体积（字节）；给定时按规范 6.10 的三档策略逐级逼近
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_bytes: Option<u64>,
}

fn default_gif_colors() -> u32 {
    256
}

fn default_to_video() -> String {
    "none".into()
}

impl Default for AnimationOptions {
    fn default() -> Self {
        Self {
            colors: default_gif_colors(),
            dither: false,
            to_video: default_to_video(),
            target_bytes: None,
        }
    }
}

fn default_true() -> bool {
    true
}
fn default_conflict() -> String {
    "skip".into()
}

impl Default for ExecOptions {
    fn default() -> Self {
        Self {
            output_dir: None,
            keep_structure: true,
            on_conflict: default_conflict(),
            image: imaging::ImageOptions::default(),
            video: VideoOptions::default(),
            animation: AnimationOptions::default(),
            dry_run: false,
            backup: false,
            overwrite_source: false,
            verify_output: true,
            skip_unchanged: false,
            config_dir: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileOutcome {
    pub id: String,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    pub original_size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_size: Option<u64>,
    /// 已跳过 / 失败的原因
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeCounts {
    pub total: usize,
    pub success: usize,
    pub unchanged: usize,
    pub skipped: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecReport {
    pub output_dir: String,
    pub outcomes: Vec<FileOutcome>,
    pub counts: OutcomeCounts,
    /// 运行级说明（例如硬件编码回退），供界面写入日志
    #[serde(default)]
    pub notes: Vec<String>,
    /// 本次是否为干跑（未写出任何文件）
    #[serde(default)]
    pub dry_run: bool,
    /// 报告文件路径（成功写出时给出）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_csv: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_json: Option<String>,
}

/// 本阶段能够写出的素材类型（动图见 P4）。
fn supported_kind(kind: crate::model::MediaKind) -> bool {
    matches!(
        kind,
        crate::model::MediaKind::Raster
            | crate::model::MediaKind::Svg
            | crate::model::MediaKind::Video
            | crate::model::MediaKind::Animated
    )
}

/// 用 FFmpeg 处理动图（规范 6.10 / 12.2 / 12.3）。
///
/// 输出仍是动图时走调色板链（GIF）或直接缩放（WebP / APNG）；
/// 输出 mp4 / webm 时按 12.3 转成视频。
fn encode_animation(
    paths: Option<&ffmpeg::FfmpegPaths>,
    source: &SourceRef,
    out: &Path,
    target: &crate::model::Computed,
    opts: &ExecOptions,
    notes: &mut Vec<String>,
) -> Result<u64, AppError> {
    let paths = paths.ok_or_else(|| {
        AppError::new(
            ErrorCode::FfmpegMissing,
            "找不到 FFmpeg，无法处理动图。请把 ffmpeg.exe 与 ffprobe.exe 放到              src-tauri/resources/ffmpeg/win-x64/。"
                .to_string(),
        )
    })?;

    let out_ext = out
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let job = crate::animate::AnimationJob {
        input: source.path.clone(),
        output: out.to_string_lossy().into_owned(),
        width: target.width,
        height: target.height,
        colors: opts.animation.colors,
        dither: opts.animation.dither,
        // 循环次数原样写回，保证动画行为不变
        loop_count: source.loop_count.unwrap_or(0),
    };

    // 目标体积：按规范 6.10 的三档策略逐级逼近，不得无限循环
    if let Some(target_bytes) = opts.animation.target_bytes {
        return shrink_to_target(paths, source, out, target, opts, target_bytes, notes);
    }

    let args = if matches!(out_ext.as_str(), "mp4" | "mov" | "mkv" | "webm" | "m4v") {
        let video = ffmpeg::VideoJob {
            input: job.input.clone(),
            output: job.output.clone(),
            width: job.width,
            height: job.height,
            encoder: ffmpeg::software_encoder(&opts.video.codec).to_string(),
            crf: opts.video.crf,
            preset: opts.video.preset.clone(),
            hardware: false,
            tonemap_to_sdr: false,
            container: ffmpeg::Container::from_ext(&out_ext),
            video_bitrate_k: None,
            fps: None,
        };
        crate::animate::build_to_video_args(&job, &video)
    } else {
        crate::animate::build_resize_args(&job)
    };

    ffmpeg::run_ffmpeg(paths, &args)?;
    Ok(fs::metadata(out).map(|m| m.len()).unwrap_or(0))
}

/// 源文件的修改时间（毫秒），增量判定要用（规范 11.4）。
fn mtime_of(path: &str) -> i64 {
    fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 把源文件复制到备份目录，保留相对结构。
fn backup_source(backup_root: &Path, id: &str, source: &Path) -> Result<(), AppError> {
    let target = output_path_for(backup_root, id, true, "");
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AppError::write_failed(format!("创建备份目录失败：{e}")))?;
    }
    fs::copy(source, &target).map_err(|e| AppError::write_failed(format!("备份源文件失败：{e}")))?;
    Ok(())
}

/// 临时输出路径：与最终文件同目录，保证改名是同盘操作。
fn temp_for(final_path: &Path, out_ext: &str) -> PathBuf {
    let stem = final_path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "out".into());
    let dir = final_path.parent().map(Path::to_path_buf).unwrap_or_default();
    dir.join(format!("{stem}.meowfit-tmp.{out_ext}"))
}

/// 图片按目标体积逐级逼近（规范 6.11 / 10.7）。
///
/// 阶梯：先降质量（每次 -10，下限 40），再按 0.9 逐级降尺寸（最多 8 级，下限 64px）。
/// 总尝试次数 = 阶梯档位数，绝不无限循环；全部档位仍超标时保留最小体积并标注未达标。
#[allow(clippy::too_many_arguments)]
fn shrink_image_to_target(
    src: &Path,
    out: &Path,
    target: &crate::model::Computed,
    mode: crate::model::Mode,
    opts: &imaging::ImageOptions,
    target_bytes: u64,
    notes: &mut Vec<String>,
) -> Result<u64, AppError> {
    // 第 0 档：原参数
    let mut attempts: Vec<(String, imaging::ImageOptions, f64)> =
        vec![("原参数".into(), opts.clone(), 1.0)];

    let mut quality = opts.quality;
    while quality > 40 {
        quality = quality.saturating_sub(10).max(40);
        attempts.push((format!("质量降至 {quality}"), imaging::ImageOptions { quality, ..opts.clone() }, 1.0));
        if quality == 40 {
            break;
        }
    }

    let mut scale = 1.0f64;
    for level in 1..=8 {
        scale *= 0.9;
        let w = (target.width as f64 * scale).round() as u32;
        let h = (target.height as f64 * scale).round() as u32;
        if w.min(h) < 64 {
            break;
        }
        attempts.push((
            format!("降尺寸 {:.0}%（{w}×{h}）", scale * 100.0),
            imaging::ImageOptions { quality, ..opts.clone() },
            scale,
        ));
        if level == 8 {
            break;
        }
    }

    let mut smallest: Option<(u64, usize)> = None;
    for (index, (label, options, factor)) in attempts.iter().enumerate() {
        let mut scaled = target.to_owned();
        scaled.width = ((target.width as f64 * factor).round().max(1.0)) as u32;
        scaled.height = ((target.height as f64 * factor).round().max(1.0)) as u32;
        scaled.content_width = ((target.content_width as f64 * factor).round().max(1.0)) as u32;
        scaled.content_height = ((target.content_height as f64 * factor).round().max(1.0)) as u32;

        let bytes = match imaging::write_image(src, out, &scaled, mode, options) {
            Ok(size) => size,
            Err(_) => continue,
        };
        notes.push(format!("体积阶梯「{label}」→ {bytes}"));
        if smallest.map(|(b, _)| bytes < b).unwrap_or(true) {
            smallest = Some((bytes, index));
        }
        if bytes <= target_bytes {
            notes.push(format!("已在「{label}」档达标（{bytes} ≤ {target_bytes}）"));
            return Ok(bytes);
        }
    }

    let Some((bytes, index)) = smallest else {
        return Err(AppError::new(
            ErrorCode::WriteFailed,
            "体积阶梯的每一档都写不出结果".to_string(),
        ));
    };
    // 保留体积最小的一档输出，并明确标注未达标
    let (label, options, factor) = &attempts[index];
    let mut scaled = target.to_owned();
    scaled.width = ((target.width as f64 * factor).round().max(1.0)) as u32;
    scaled.height = ((target.height as f64 * factor).round().max(1.0)) as u32;
    scaled.content_width = ((target.content_width as f64 * factor).round().max(1.0)) as u32;
    scaled.content_height = ((target.content_height as f64 * factor).round().max(1.0)) as u32;
    imaging::write_image(src, out, &scaled, mode, options)?;
    notes.push(format!(
        "无法达标：最小体积为 {bytes} 字节（档位「{label}」），目标 {target_bytes} 字节"
    ));
    Ok(bytes)
}

/// 写出后校验输出能否正常解码（规范 6.7）。校验不通过的文件必须删除。
///
/// 判定依据是**输出文件的格式**，不是源素材类型：源是 SVG、输出是 PNG 时，
/// 后者要用位图解码去验，否则会把正确的输出误判为损坏。
fn verify_output(
    path: &Path,
    out_ext: &str,
    ffmpeg_paths: Option<&ffmpeg::FfmpegPaths>,
) -> Result<(), AppError> {
    let ext = out_ext.to_ascii_lowercase();
    let ok = match ext.as_str() {
        "svg" => std::fs::read_to_string(path)
            .ok()
            .and_then(|text| crate::svg::base_size(&text))
            .is_some(),
        // image crate 自己能解的位图
        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "tif" | "tiff" | "ico" => {
            image::image_dimensions(path).is_ok()
        }
        // 动图、视频、HEIC，以及只由 FFmpeg 处理的 AVIF 都交给 ffprobe
        _ => ffmpeg_paths
            .map(|paths| ffmpeg::probe(paths, path).is_ok())
            .unwrap_or(true),
    };

    if ok {
        Ok(())
    } else {
        Err(AppError::new(
            ErrorCode::OutputVerifyFailed,
            format!(
                "输出校验未通过：{}",
                path.file_name().unwrap_or_default().to_string_lossy()
            ),
        ))
    }
}

/// 按规范 11.3 的字段与顺序组装报告行。
fn build_report_rows(
    entries: &[crate::model::PlanEntry],
    sources: &[SourceRef],
    output_root: &Path,
    outcomes: &[FileOutcome],
) -> Vec<crate::report::ReportRow> {
    let outcome_of: std::collections::HashMap<&str, &FileOutcome> =
        outcomes.iter().map(|o| (o.id.as_str(), o)).collect();
    let source_of: std::collections::HashMap<&str, &SourceRef> =
        sources.iter().map(|s| (s.id.as_str(), s)).collect();

    entries
        .iter()
        .map(|entry| (entry.id.as_str(), Some(entry)))
        .chain(
            outcomes
                .iter()
                .filter(|o| !entries.iter().any(|e| e.id == o.id))
                .map(|o| (o.id.as_str(), None)),
        )
        .map(|(id, entry)| {
            let outcome = outcome_of.get(id);
            let source = source_of.get(id);
            let target = entry.and_then(|e| e.target);

            crate::report::ReportRow {
                source_path: source.map(|s| s.path.clone()).unwrap_or_default(),
                output_path: outcome.and_then(|o| o.output_path.clone()).unwrap_or_else(|| {
                    output_root.join(id).to_string_lossy().into_owned()
                }),
                kind: source.map(|s| kind_label(s.kind)).unwrap_or("未知").to_string(),
                group: entry.map(|e| e.group.clone()).unwrap_or_default(),
                original_width: entry.map(|e| e.original_width.to_string()).unwrap_or_default(),
                original_height: entry.map(|e| e.original_height.to_string()).unwrap_or_default(),
                target_width: target.map(|t| t.width.to_string()).unwrap_or_default(),
                target_height: target.map(|t| t.height.to_string()).unwrap_or_default(),
                original_size: outcome.map(|o| o.original_size).unwrap_or(0).to_string(),
                new_size: outcome
                    .and_then(|o| o.new_size)
                    .map(|n| n.to_string())
                    .unwrap_or_default(),
                mode: entry
                    .and_then(|e| e.mode)
                    .map(|m| format!("{m:?}"))
                    .unwrap_or_else(|| "不变".into()),
                status: outcome
                    .map(|o| o.status.label().to_string())
                    .unwrap_or_else(|| "未改动".into()),
                error: outcome.and_then(|o| o.reason.clone()).unwrap_or_default(),
                elapsed_ms: String::new(),
            }
        })
        .collect()
}

fn kind_label(kind: crate::model::MediaKind) -> &'static str {
    match kind {
        crate::model::MediaKind::Raster => "静态图片",
        crate::model::MediaKind::Svg => "SVG",
        crate::model::MediaKind::Animated => "动图",
        crate::model::MediaKind::Video => "视频",
    }
}

/// 处理只能交给 FFmpeg 的位图（AVIF / HEIC / HEIF，规范 5.4）。
///
/// 这些格式 image crate 解不了，统一走 FFmpeg 的 scale + 对应编码器。
/// 其中 HEIC / HEIF **没有可用的 muxer**，只能引导用户改选输出格式。
fn encode_ffmpeg_image(
    paths: Option<&ffmpeg::FfmpegPaths>,
    src: &Path,
    out: &Path,
    target: &crate::model::Computed,
    opts: &imaging::ImageOptions,
) -> Result<u64, AppError> {
    let paths = paths.ok_or_else(|| {
        AppError::new(
            ErrorCode::FfmpegMissing,
            "找不到 FFmpeg，无法处理 AVIF / HEIC。请把 ffmpeg.exe 与 ffprobe.exe 放到              src-tauri/resources/ffmpeg/win-x64/。"
                .to_string(),
        )
    })?;

    let out_ext = out
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if matches!(out_ext.as_str(), "heic" | "heif") {
        return Err(AppError::new(
            ErrorCode::WriteFailed,
            "当前 FFmpeg 构建无法写出 HEIC / HEIF。请在「图片处理 → 输出格式」里选择 PNG / JPEG / WebP 后重试。"
                .to_string(),
        ));
    }

    let flag = imaging::ffmpeg_scale_flag(opts.resample);
    let mut args: Vec<String> = ["-hide_banner", "-nostdin", "-y"]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    args.push("-i".into());
    args.push(src.to_string_lossy().into_owned());
    args.push("-vf".into());
    args.push(format!("scale={}:{}:flags={flag}", target.width, target.height));
    args.push("-frames:v".into());
    args.push("1".into());

    match out_ext.as_str() {
        "avif" => {
            args.push("-c:v".into());
            args.push("libaom-av1".into());
            args.push("-crf".into());
            // 质量 1–100 映射到 AV1 的 CRF（0–63，越小越好）
            let crf = (63.0 - (opts.quality.clamp(1, 100) as f64 / 100.0) * 63.0).round() as u32;
            args.push(crf.min(63).to_string());
            args.push("-cpu-used".into());
            args.push("8".into());
            args.push("-f".into());
            args.push("avif".into());
        }
        "png" => {
            args.push("-c:v".into());
            args.push("png".into());
        }
        "webp" => {
            args.push("-c:v".into());
            args.push("libwebp".into());
            args.push("-quality".into());
            args.push(opts.quality.clamp(1, 100).to_string());
        }
        "jpg" | "jpeg" => {
            args.push("-c:v".into());
            args.push("mjpeg".into());
            args.push("-q:v".into());
            args.push(imaging::jpeg_qscale(opts.quality).to_string());
        }
        _ => {}
    }

    args.push(out.to_string_lossy().into_owned());
    ffmpeg::run_ffmpeg(paths, &args)?;
    Ok(fs::metadata(out).map(|m| m.len()).unwrap_or(0))
}

/// 动图按目标体积逐级逼近（规范 6.10 / 10.7）。
///
/// 依次尝试「原参数 → 减色数 → 降尺寸 → 丢帧」，每一档写出并测量实际体积，
/// 一旦达标就采用该档；全部档位耗尽仍超标时，保留其中体积最小的一次输出，
/// 并在说明中标注「未达标」——绝不无限循环。
fn shrink_to_target(
    paths: &ffmpeg::FfmpegPaths,
    source: &SourceRef,
    out: &Path,
    target: &crate::model::Computed,
    opts: &ExecOptions,
    target_bytes: u64,
    notes: &mut Vec<String>,
) -> Result<u64, AppError> {
    let fps = ffmpeg::probe(paths, Path::new(&source.path))
        .ok()
        .and_then(|p| p.to_video_info())
        .map(|i| i.fps)
        .unwrap_or(0.0);

    let base = crate::animate::AnimationJob {
        input: source.path.clone(),
        output: out.to_string_lossy().into_owned(),
        width: target.width,
        height: target.height,
        colors: opts.animation.colors,
        dither: opts.animation.dither,
        loop_count: source.loop_count.unwrap_or(0),
    };

    // 第 0 档是原参数，其后才是阶梯
    let mut attempts: Vec<(String, crate::animate::AnimationJob)> =
        vec![("原参数".to_string(), base.clone())];
    for step in crate::animate::build_ladder(base.colors, fps) {
        attempts.push((
            step.label,
            crate::animate::AnimationJob {
                width: ((target.width as f64 * step.scale).round() as u32).max(2),
                height: ((target.height as f64 * step.scale).round() as u32).max(2),
                colors: step.colors,
                ..base.clone()
            },
        ));
    }

    let scratch = out.with_extension(format!("meowfit-tmp.{}", out_ext_of(out)));
    let mut smallest: Option<(u64, String)> = None;

    for (label, job) in &attempts {
        let job = crate::animate::AnimationJob {
            output: scratch.to_string_lossy().into_owned(),
            ..job.clone()
        };
        if ffmpeg::run_ffmpeg(paths, &crate::animate::build_resize_args(&job)).is_err() {
            continue;
        }
        let bytes = fs::metadata(&scratch).map(|m| m.len()).unwrap_or(0);
        notes.push(format!("体积阶梯「{label}」→ {}", bytes));
        if smallest.as_ref().map(|(b, _)| bytes < *b).unwrap_or(true) {
            smallest = Some((bytes, label.clone()));
        }
        if bytes <= target_bytes {
            fs::rename(&scratch, out).map_err(|e| AppError::write_failed(format!("写出失败：{e}")))?;
            notes.push(format!("已在「{label}」档达标（{bytes} ≤ {target_bytes}）"));
            return Ok(bytes);
        }
    }
    let _ = fs::remove_file(&scratch);

    // 全部档位都不达标：用最小体积的那一档重放一次，并标注未达标
    let Some((bytes, label)) = smallest else {
        return Err(AppError::new(
            ErrorCode::WriteFailed,
            "体积阶梯的每一档都写不出结果".to_string(),
        ));
    };
    let step = attempts.iter().find(|(l, _)| *l == label).map(|(_, j)| j.clone());
    if let Some(job) = step {
        ffmpeg::run_ffmpeg(paths, &crate::animate::build_resize_args(&job))?;
    }
    notes.push(format!(
        "无法达标：最小体积为 {bytes} 字节（档位「{label}」），目标 {target_bytes} 字节"
    ));
    Ok(bytes)
}

fn out_ext_of(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("gif")
        .to_string()
}

/// 用 FFmpeg 编码一个视频；硬件编码失败时自动回退软件编码（规范 6.9）。
fn encode_video(
    paths: Option<&ffmpeg::FfmpegPaths>,
    source: &str,
    out: &Path,
    target: &crate::model::Computed,
    opts: &VideoOptions,
    notes: &mut Vec<String>,
) -> Result<u64, AppError> {
    let paths = paths.ok_or_else(|| {
        AppError::new(
            ErrorCode::FfmpegMissing,
            "找不到 FFmpeg，无法处理视频。请把 ffmpeg.exe 与 ffprobe.exe 放到 \
             src-tauri/resources/ffmpeg/win-x64/（打包后为 程序目录/resources/ffmpeg/win-x64/）。"
                .to_string(),
        )
    })?;

    let out_ext = out
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_string();

    let hardware = if opts.hardware {
        ffmpeg::hardware_encoder(&opts.codec, &opts.accel)
    } else {
        None
    };
    if opts.hardware && hardware.is_none() {
        notes.push(format!(
            "{} 没有 {} 硬件实现，已直接使用软件编码 {}",
            opts.codec.to_uppercase(),
            opts.accel.to_uppercase(),
            ffmpeg::software_encoder(&opts.codec)
        ));
    }

    let mut job = ffmpeg::VideoJob {
        input: source.to_string(),
        output: out.to_string_lossy().into_owned(),
        width: target.width,
        height: target.height,
        encoder: hardware
            .unwrap_or_else(|| ffmpeg::software_encoder(&opts.codec))
            .to_string(),
        crf: opts.crf,
        preset: opts.preset.clone(),
        hardware: hardware.is_some(),
        tonemap_to_sdr: opts.tonemap_to_sdr,
        container: ffmpeg::Container::from_ext(&out_ext),
        video_bitrate_k: None,
        fps: None,
    };

    match ffmpeg::run_ffmpeg(paths, &ffmpeg::build_args(&job)) {
        Ok(()) => Ok(fs::metadata(out).map(|m| m.len()).unwrap_or(0)),
        Err(err) if job.hardware => {
            // 编码失败必须自动回退软件编码，并在日志中留下回退记录
            let software = ffmpeg::software_encoder(&opts.codec);
            notes.push(format!(
                "硬件编码 {} 失败（{}），已回退到软件编码 {}",
                job.encoder, err.message, software
            ));
            job = ffmpeg::fallback_job(&job, &opts.codec);
            ffmpeg::run_ffmpeg(paths, &ffmpeg::build_args(&job))?;
            Ok(fs::metadata(out).map(|m| m.len()).unwrap_or(0))
        }
        Err(err) => Err(err),
    }
}

/// 探测目录可写性：创建目录并写入再删除一个临时文件（规范 4.1 的判定方式）。
fn ensure_writable_dir(dir: &Path) -> Result<(), AppError> {
    fs::create_dir_all(dir).map_err(|e| {
        AppError::output_unwritable(format!(
            "输出目录不可写：{}（{e}）。请改用「用户指定输出目录」。",
            dir.display()
        ))
    })?;

    let probe = dir.join(".meowfit-write-probe");
    match fs::write(&probe, b"") {
        Ok(()) => {
            let _ = fs::remove_file(&probe);
            Ok(())
        }
        Err(e) => Err(AppError::output_unwritable(format!(
            "输出目录不可写：{}（{e}）。请改用「用户指定输出目录」。",
            dir.display()
        ))),
    }
}

/// 解析输出目录（规范 6.5）。
pub fn resolve_output_dir(root: &Path, opts: &ExecOptions) -> Result<PathBuf, AppError> {
    let dir = match &opts.output_dir {
        Some(d) if !d.trim().is_empty() => PathBuf::from(d),
        _ => scan::default_output_dir(root),
    };
    ensure_writable_dir(&dir)?;
    Ok(dir)
}

/// 处理同名冲突，返回最终输出路径；`None` 表示按策略跳过。
fn resolve_conflict(path: PathBuf, on_conflict: &str) -> Option<PathBuf> {
    if !path.exists() {
        return Some(path);
    }
    match on_conflict {
        "overwrite" => Some(path),
        "rename" => {
            let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let ext = path
                .extension()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            for n in 1..10_000 {
                let name = if ext.is_empty() {
                    format!("{stem}-{n}")
                } else {
                    format!("{stem}-{n}.{ext}")
                };
                let candidate = parent.join(name);
                if !candidate.exists() {
                    return Some(candidate);
                }
            }
            None
        }
        // 默认跳过（规范 6.5 / 附录 B 第 5 项）
        _ => None,
    }
}

fn output_path_for(
    output_root: &Path,
    source_id: &str,
    keep_structure: bool,
    new_ext: &str,
) -> PathBuf {
    let relative = source_id.replace('\\', "/");
    let relative = relative.trim_start_matches('/');
    let mut path = if keep_structure {
        let mut path = output_root.to_path_buf();
        for segment in relative.split('/').filter(|s| !s.is_empty() && *s != "..") {
            path.push(segment);
        }
        path
    } else {
        let name = relative.rsplit('/').next().unwrap_or(relative);
        output_root.join(name)
    };

    // 统一转换格式时扩展名随目标格式变化（规范 6.5 的例外条款）
    if !new_ext.is_empty() {
        path.set_extension(new_ext);
    }
    path
}

/// 执行计划（规范 6.7 的单文件粒度部分）。
///
/// `plan` 必须来自 [`crate::algo::plan::build_plan`]，且 `plan.ok == true`；
/// 调用方在 `ok == false` 时不得调用本函数（规范 13.1 的 `VALIDATION_FAILED`）。
pub fn execute(
    plan: &Plan,
    sources: &[SourceRef],
    root: &Path,
    opts: &ExecOptions,
) -> Result<ExecReport, AppError> {
    execute_with(plan, sources, root, opts, None, None)
}

/// 带进度回调与取消标志的执行入口（规范 6.7）。
pub fn execute_with(
    plan: &Plan,
    sources: &[SourceRef],
    root: &Path,
    opts: &ExecOptions,
    mut progress: Option<&mut dyn FnMut(usize, usize, &str)>,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<ExecReport, AppError> {
    use std::sync::atomic::Ordering;

    let output_root = resolve_output_dir(root, opts)?;

    // FFmpeg 路径解析一次，供本次执行中所有视频复用
    let ffmpeg_paths = ffmpeg::resolve_paths().ok();
    let mut notes: Vec<String> = Vec::new();

    // 增量处理：读上次的索引，处理完再写回（规范 6.13）
    let config_dir = opts.config_dir.as_ref().map(PathBuf::from);
    let mut index = config_dir
        .as_ref()
        .map(|dir| crate::incremental::load(dir))
        .unwrap_or_default();

    // 备份目录：处理前先把源文件复制一份，便于回滚（规范 6.5）
    let backup_root = output_root.join(".meowfit-backup");
    if opts.dry_run {
        notes.push("干跑模式：只计算与预览，不写出任何文件".into());
    }

    let lookup: std::collections::HashMap<&str, &SourceRef> =
        sources.iter().map(|s| (s.id.as_str(), s)).collect();

    let mut outcomes: Vec<FileOutcome> = Vec::with_capacity(plan.entries.len());

    // 扫描阶段就不支持的素材：不进入计划，但要出现在报告的「已跳过」清单里
    for source in sources.iter().filter(|s| s.skip_reason.is_some()) {
        outcomes.push(FileOutcome {
            id: source.id.clone(),
            status: Status::Skipped,
            output_path: None,
            original_size: fs::metadata(&source.path).map(|m| m.len()).unwrap_or(0),
            new_size: None,
            reason: source.skip_reason.clone(),
        });
    }

    for entry in &plan.entries {
        // 取消：已完成的保留，未开始的停止（规范 6.7）
        if let Some(flag) = cancel {
            if flag.load(Ordering::Relaxed) {
                notes.push("任务已取消：已完成的保留，未开始的停止".into());
                break;
            }
        }

        let source = match lookup.get(entry.id.as_str()) {
            Some(s) => *s,
            None => {
                outcomes.push(FileOutcome {
                    id: entry.id.clone(),
                    status: Status::Failed,
                    output_path: None,
                    original_size: 0,
                    new_size: None,
                    reason: Some("找不到对应的源文件".into()),
                });
                continue;
            }
        };

        let original_size = fs::metadata(&source.path).map(|m| m.len()).unwrap_or(0);

        if let Some(err) = &entry.error {
            outcomes.push(FileOutcome {
                id: entry.id.clone(),
                status: Status::Failed,
                output_path: None,
                original_size,
                new_size: None,
                reason: Some(format!("{} {}", err.code.as_str(), err.message)),
            });
            continue;
        }

        // 未改动：不写出、不复制、不转码（规范 6.4 / 10.4）
        if entry.action == Action::Unchanged {
            outcomes.push(FileOutcome {
                id: entry.id.clone(),
                status: Status::Unchanged,
                output_path: None,
                original_size,
                new_size: None,
                reason: None,
            });
            continue;
        }

        let target = match entry.target {
            Some(t) => t,
            None => continue,
        };
        let mode = match entry.mode {
            Some(m) => m,
            None => continue,
        };

        if !supported_kind(source.kind) {
            outcomes.push(FileOutcome {
                id: entry.id.clone(),
                status: Status::Skipped,
                output_path: None,
                original_size,
                new_size: None,
                reason: Some("该类型处理将在后续阶段接入".into()),
            });
            continue;
        }

        let src_ext = Path::new(&source.path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        // 视频保持原容器；动图转视频时按用户选择换成 mp4 / webm；
        // 图片与 SVG 按格式设置转换
        let out_ext = match source.kind {
            crate::model::MediaKind::Video => src_ext.clone(),
            crate::model::MediaKind::Animated if opts.animation.to_video != "none" => {
                opts.animation.to_video.clone()
            }
            crate::model::MediaKind::Animated => src_ext.clone(),
            _ => imaging::resolve_output_ext(&src_ext, opts.image.format),
        };
        let wanted = if opts.overwrite_source {
            // 用户显式开启「覆盖源文件」时才写回原路径（界面须二次确认）
            PathBuf::from(&source.path)
        } else {
            output_path_for(&output_root, &entry.id, opts.keep_structure, &out_ext)
        };

        // 默认绝不覆盖源文件（规范第八节）
        if !opts.overwrite_source && wanted == PathBuf::from(&source.path) {
            outcomes.push(FileOutcome {
                id: entry.id.clone(),
                status: Status::Failed,
                output_path: None,
                original_size,
                new_size: None,
                reason: Some("输出路径与源文件相同，已阻止覆盖源文件".into()),
            });
            continue;
        }

        // 开启「覆盖源文件」时目标就是源文件本身，不存在「同名冲突」一说
        let final_path = if opts.overwrite_source {
            Some(wanted)
        } else {
            resolve_conflict(wanted, &opts.on_conflict)
        };
        let final_path = match final_path {
            Some(p) => p,
            None => {
                outcomes.push(FileOutcome {
                    id: entry.id.clone(),
                    status: Status::Skipped,
                    output_path: None,
                    original_size,
                    new_size: None,
                    reason: Some("输出目录中已存在同名文件".into()),
                });
                continue;
            }
        };

        // 增量处理：体积、修改时间、设置指纹三者全一致才跳过（规范 11.4）
        let hash = crate::incremental::setting_hash(
            &serde_json::to_string(&entry.setting).unwrap_or_default(),
        );
        if opts.skip_unchanged
            && crate::incremental::should_skip(
                &index,
                &source.path,
                original_size,
                mtime_of(&source.path),
                &hash,
            )
        {
            outcomes.push(FileOutcome {
                id: entry.id.clone(),
                status: Status::Skipped,
                output_path: None,
                original_size,
                new_size: None,
                reason: Some("未发生变化，按增量设置跳过".into()),
            });
            continue;
        }

        if let Some(f) = progress.as_mut() {
            f(outcomes.len() + 1, plan.entries.len(), &entry.name);
        }

        // 先把源文件备份一份，便于回滚
        if opts.backup && !opts.dry_run {
            let _ = backup_source(&backup_root, &entry.id, Path::new(&source.path));
        }

        // 干跑：不写出任何文件，但仍按「会成功」计入报告
        if opts.dry_run {
            outcomes.push(FileOutcome {
                id: entry.id.clone(),
                status: Status::Success,
                output_path: Some(final_path.to_string_lossy().into_owned()),
                original_size,
                new_size: None,
                reason: Some("干跑模式，未写出".into()),
            });
            continue;
        }

        // 一律先写到临时文件再改名：这样「覆盖源文件」时也不会边读边写
        let temp_path = temp_for(&final_path, &out_ext);

        let result = if source.kind == crate::model::MediaKind::Animated {
            encode_animation(ffmpeg_paths.as_ref(), source, &temp_path, &target, opts, &mut notes)
        } else if imaging::is_ffmpeg_only_raster(&src_ext) {
            encode_ffmpeg_image(ffmpeg_paths.as_ref(), Path::new(&source.path), &temp_path, &target, &opts.image)
        } else if source.kind == crate::model::MediaKind::Video {
            encode_video(ffmpeg_paths.as_ref(), &source.path, &temp_path, &target, &opts.video, &mut notes)
        } else if let Some(target_bytes) = opts.image.target_bytes {
            shrink_image_to_target(
                Path::new(&source.path),
                &temp_path,
                &target,
                mode,
                &opts.image,
                target_bytes,
                &mut notes,
            )
        } else {
            imaging::write_image(Path::new(&source.path), &temp_path, &target, mode, &opts.image)
        };

        // 写出后校验：解不出来的输出不许留在输出目录（规范 6.7）
        let result = result.and_then(|size| {
            if !opts.verify_output {
                return Ok(size);
            }
            match verify_output(&temp_path, &out_ext, ffmpeg_paths.as_ref()) {
                Ok(()) => Ok(size),
                Err(err) => {
                    let _ = fs::remove_file(&temp_path);
                    Err(err)
                }
            }
        });

        let result = result.and_then(|size| {
            fs::rename(&temp_path, &final_path)
                .map_err(|e| AppError::write_failed(format!("输出改名失败：{e}")))?;
            Ok(size)
        });

        match result {
            Ok(new_size) => {
                crate::incremental::record(
                    &mut index,
                    &source.path,
                    original_size,
                    mtime_of(&source.path),
                    Some(&target),
                    &hash,
                );
                outcomes.push(FileOutcome {
                    id: entry.id.clone(),
                    status: Status::Success,
                    output_path: Some(final_path.to_string_lossy().into_owned()),
                    original_size,
                    new_size: Some(new_size),
                    reason: None,
                })
            }
            Err(err) => outcomes.push(FileOutcome {
                id: entry.id.clone(),
                status: Status::Failed,
                output_path: None,
                original_size,
                new_size: None,
                reason: Some(format!("{} {}", err.code.as_str(), err.message)),
            }),
        }
    }

    let mut counts = OutcomeCounts {
        total: outcomes.len(),
        ..Default::default()
    };
    for o in &outcomes {
        match o.status {
            Status::Success => counts.success += 1,
            Status::Unchanged => counts.unchanged += 1,
            Status::Skipped => counts.skipped += 1,
            Status::Failed => counts.failed += 1,
        }
    }

    // 增量索引写回
    if let Some(dir) = &config_dir {
        if !opts.dry_run {
            if let Err(err) = crate::incremental::save(dir, &index) {
                notes.push(format!("增量索引保存失败：{err}"));
            }
        }
        let _ = crate::logging::purge_old(dir);
        for line in &notes {
            let _ = crate::logging::append(dir, "WARN", line);
        }
        // 每次任务都留一条小结，保证日志有据可查
        let _ = crate::logging::append(
            dir,
            "INFO",
            &format!(
                "任务{}：成功 {}、未改动 {}、已跳过 {}、失败 {}；输出目录 {}",
                if opts.dry_run { "（干跑）" } else { "" },
                counts.success,
                counts.unchanged,
                counts.skipped,
                counts.failed,
                output_root.display()
            ),
        );
    }

    // 报告：干跑也要完整出具（规范 6.5 / 场景 22）
    let rows = build_report_rows(&plan.entries, sources, &output_root, &outcomes);
    let (report_csv, report_json) = if opts.dry_run {
        (None, None)
    } else {
        match crate::report::write_reports(&output_root, &rows) {
            Ok((csv, json)) => (Some(csv), Some(json)),
            Err(err) => {
                notes.push(format!("报告写出失败：{}", err.message));
                (None, None)
            }
        }
    };

    Ok(ExecReport {
        output_dir: output_root.to_string_lossy().into_owned(),
        outcomes,
        counts,
        notes,
        dry_run: opts.dry_run,
        report_csv,
        report_json,
    })
}

/// 只读介质场景：不可写时给出 `E_OUTPUT_UNWRITABLE`（规范 13.2）
pub fn unwritable_error(dir: &Path) -> AppError {
    AppError::new(
        ErrorCode::OutputUnwritable,
        format!("输出目录不可写：{}", dir.display()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algo::plan::build_plan;
    use crate::model::{
        GroupSetting, MediaKind, Mode, PlanFileInput, PlanGroupInput, PlanRequest, Setting,
    };
    use image::{Rgb, RgbImage};

    fn write_png(path: &Path, w: u32, h: u32) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        RgbImage::from_pixel(w, h, Rgb([10, 20, 30])).save(path).unwrap();
    }

    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "meowfit-exec-{tag}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn plan_for(files: Vec<PlanFileInput>, groups: Vec<PlanGroupInput>, global: Option<Setting>) -> Plan {
        build_plan(&PlanRequest {
            files,
            groups,
            global,
        })
    }

    fn dims_of(path: &Path) -> (u32, u32) {
        image::image_dimensions(path).unwrap()
    }

    #[test]
    fn writes_halved_images_to_sibling_output_and_leaves_source_untouched() {
        // 验收场景 1
        let root = temp_root("halve").join("素材");
        write_png(&root.join("a.png"), 200, 100);
        write_png(&root.join("sub/b.png"), 80, 40);
        let before = fs::metadata(root.join("a.png")).unwrap().len();

        let plan = plan_for(
            vec![
                PlanFileInput {
                    id: "a.png".into(),
                    name: "a.png".into(),
                    width: 200,
                    height: 100,
                    is_video: false,
                    group: "g".into(),
                    setting: None,
                },
                PlanFileInput {
                    id: "sub/b.png".into(),
                    name: "b.png".into(),
                    width: 80,
                    height: 40,
                    is_video: false,
                    group: "g".into(),
                    setting: None,
                },
            ],
            vec![],
            Some(Setting::scale(Mode::A, 0.5)),
        );
        assert!(plan.ok);

        let sources = vec![
            SourceRef {
                id: "a.png".into(),
                path: root.join("a.png").to_string_lossy().into_owned(),
                kind: MediaKind::Raster,
                loop_count: None,
                ext: None,
                skip_reason: None,
            },
            SourceRef {
                id: "sub/b.png".into(),
                path: root.join("sub/b.png").to_string_lossy().into_owned(),
                kind: MediaKind::Raster,
                loop_count: None,
                ext: None,
                skip_reason: None,
            },
        ];

        let report = execute(&plan, &sources, &root, &ExecOptions::default()).unwrap();
        assert_eq!(report.counts.success, 2);

        // 输出到 源文件夹的同级目录/output/
        let out = root.parent().unwrap().join("output");
        assert_eq!(report.output_dir, out.to_string_lossy());
        assert_eq!(dims_of(&out.join("a.png")), (100, 50));
        // 保留相对目录结构、输出文件名保持原名
        assert_eq!(dims_of(&out.join("sub/b.png")), (40, 20));

        // 源文件未被改动
        assert_eq!(fs::metadata(root.join("a.png")).unwrap().len(), before);
        assert_eq!(dims_of(&root.join("a.png")), (200, 100));

        let _ = fs::remove_dir_all(root.parent().unwrap());
    }

    #[test]
    fn unchanged_files_are_not_copied_to_output() {
        // 验收场景 2 的关键行为：未改动素材原地不动，输出目录中不存在它
        let root = temp_root("unchanged").join("素材");
        write_png(&root.join("icon_01.png"), 100, 100);
        write_png(&root.join("bg_01.png"), 100, 100);

        let plan = plan_for(
            vec![
                PlanFileInput {
                    id: "icon_01.png".into(),
                    name: "icon_01.png".into(),
                    width: 100,
                    height: 100,
                    is_video: false,
                    group: "icon".into(),
                    setting: None,
                },
                PlanFileInput {
                    id: "bg_01.png".into(),
                    name: "bg_01.png".into(),
                    width: 100,
                    height: 100,
                    is_video: false,
                    group: "bg".into(),
                    setting: None,
                },
            ],
            vec![PlanGroupInput {
                name: "icon".into(),
                setting: Some(GroupSetting::Explicit(Setting::width_height(
                    Mode::D, 128.0, 128.0,
                ))),
            }],
            None,
        );

        let sources = vec![
            SourceRef {
                id: "icon_01.png".into(),
                path: root.join("icon_01.png").to_string_lossy().into_owned(),
                kind: MediaKind::Raster,
                loop_count: None,
                ext: None,
                skip_reason: None,
            },
            SourceRef {
                id: "bg_01.png".into(),
                path: root.join("bg_01.png").to_string_lossy().into_owned(),
                kind: MediaKind::Raster,
                loop_count: None,
                ext: None,
                skip_reason: None,
            },
        ];

        let report = execute(&plan, &sources, &root, &ExecOptions::default()).unwrap();
        assert_eq!(report.counts.success, 1);
        assert_eq!(report.counts.unchanged, 1);

        let out = PathBuf::from(&report.output_dir);
        assert!(out.join("icon_01.png").exists());
        assert!(!out.join("bg_01.png").exists(), "未改动素材不得复制到输出目录");

        let _ = fs::remove_dir_all(root.parent().unwrap());
    }

    #[test]
    fn conflict_policy_skip_and_rename() {
        let root = temp_root("conflict").join("素材");
        write_png(&root.join("a.png"), 100, 100);

        let plan = plan_for(
            vec![PlanFileInput {
                id: "a.png".into(),
                name: "a.png".into(),
                width: 100,
                height: 100,
                is_video: false,
                group: "g".into(),
                setting: None,
            }],
            vec![],
            Some(Setting::scale(Mode::A, 0.5)),
        );
        let sources = vec![SourceRef {
            id: "a.png".into(),
            path: root.join("a.png").to_string_lossy().into_owned(),
            kind: MediaKind::Raster,
                loop_count: None,
                ext: None,
                skip_reason: None,
            }];

        // 先跑一次产生输出
        execute(&plan, &sources, &root, &ExecOptions::default()).unwrap();
        let out = root.parent().unwrap().join("output");

        // 默认策略：跳过
        let again = execute(&plan, &sources, &root, &ExecOptions::default()).unwrap();
        assert_eq!(again.counts.skipped, 1);

        // 自动重命名：追加 -1
        let renamed = execute(
            &plan,
            &sources,
            &root,
            &ExecOptions {
                on_conflict: "rename".into(),
                ..ExecOptions::default()
            },
        )
        .unwrap();
        assert_eq!(renamed.counts.success, 1);
        assert!(out.join("a-1.png").exists());

        let _ = fs::remove_dir_all(root.parent().unwrap());
    }

    #[test]
    fn unwritable_output_dir_reports_e_output_unwritable() {
        let root = temp_root("ro").join("素材");
        write_png(&root.join("a.png"), 100, 100);

        // 用一个「位于普通文件之下」的路径，必然无法创建
        let blocker = root.parent().unwrap().join("blocker");
        fs::write(&blocker, b"x").unwrap();

        let opts = ExecOptions {
            output_dir: Some(blocker.join("sub").to_string_lossy().into_owned()),
            ..ExecOptions::default()
        };
        let err = resolve_output_dir(&root, &opts).unwrap_err();
        assert_eq!(err.code, ErrorCode::OutputUnwritable);

        let _ = fs::remove_dir_all(root.parent().unwrap());
    }

    #[test]
    fn keep_structure_off_flattens_output() {
        let root = temp_root("flat").join("素材");
        write_png(&root.join("sub/a.png"), 100, 100);

        let plan = plan_for(
            vec![PlanFileInput {
                id: "sub/a.png".into(),
                name: "a.png".into(),
                width: 100,
                height: 100,
                is_video: false,
                group: "g".into(),
                setting: None,
            }],
            vec![],
            Some(Setting::scale(Mode::A, 0.5)),
        );
        let sources = vec![SourceRef {
            id: "sub/a.png".into(),
            path: root.join("sub/a.png").to_string_lossy().into_owned(),
            kind: MediaKind::Raster,
                loop_count: None,
                ext: None,
                skip_reason: None,
            }];

        let report = execute(
            &plan,
            &sources,
            &root,
            &ExecOptions {
                keep_structure: false,
                ..ExecOptions::default()
            },
        )
        .unwrap();
        let out = PathBuf::from(&report.output_dir);
        assert!(out.join("a.png").exists());
        assert!(!out.join("sub/a.png").exists());

        let _ = fs::remove_dir_all(root.parent().unwrap());
    }

    #[test]
    fn mode_b_pads_transparent_for_png_and_background_color_for_jpeg() {
        let root = temp_root("pad").join("素材");
        write_png(&root.join("wide.png"), 200, 100);
        // 同一张图另存为 JPG，用于验证不支持 alpha 的输出格式改用背景色填充
        image::open(root.join("wide.png"))
            .unwrap()
            .to_rgb8()
            .save(root.join("wide.jpg"))
            .unwrap();

        let plan = plan_for(
            vec![
                PlanFileInput {
                    id: "wide.png".into(),
                    name: "wide.png".into(),
                    width: 200,
                    height: 100,
                    is_video: false,
                    group: "g".into(),
                    setting: None,
                },
                PlanFileInput {
                    id: "wide.jpg".into(),
                    name: "wide.jpg".into(),
                    width: 200,
                    height: 100,
                    is_video: false,
                    group: "g".into(),
                    setting: None,
                },
            ],
            vec![],
            Some(Setting::width_height(Mode::B, 200.0, 200.0)),
        );

        let sources = vec![
            SourceRef {
                id: "wide.png".into(),
                path: root.join("wide.png").to_string_lossy().into_owned(),
                kind: MediaKind::Raster,
                loop_count: None,
                ext: None,
                skip_reason: None,
            },
            SourceRef {
                id: "wide.jpg".into(),
                path: root.join("wide.jpg").to_string_lossy().into_owned(),
                kind: MediaKind::Raster,
                loop_count: None,
                ext: None,
                skip_reason: None,
            },
        ];

        let report = execute(&plan, &sources, &root, &ExecOptions::default()).unwrap();
        assert_eq!(report.counts.success, 2);
        let out = PathBuf::from(&report.output_dir);

        // PNG：输出 200×200 画布，内容 200×100 居中，上下各 50px 透明
        let png_out = out.join("wide.png");
        assert_eq!(dims_of(&png_out), (200, 200));
        let img = image::open(&png_out).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(100, 10)[3], 0, "空余区域默认应为透明");
        assert_eq!(img.get_pixel(100, 100)[3], 255, "内容区域应不透明");

        // JPG：不支持 alpha，空余区域以背景色（默认白）填充
        let jpg_out = out.join("wide.jpg");
        assert_eq!(dims_of(&jpg_out), (200, 200));
        let img = image::open(&jpg_out).unwrap().to_rgba8();
        let pad = img.get_pixel(100, 10);
        assert!(
            pad[0] > 240 && pad[1] > 240 && pad[2] > 240,
            "JPG 空余区域应为白色，实际 {pad:?}"
        );

        let _ = fs::remove_dir_all(root.parent().unwrap());
    }
}
