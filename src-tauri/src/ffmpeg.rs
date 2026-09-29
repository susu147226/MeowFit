//! FFmpeg 调用层（规范第十二节）。
//!
//! 两条铁律：
//! 1. 一律以**独立进程**调用，参数以数组形式传递，**不拼接 shell 字符串**；
//! 2. 尺度、容器、色彩相关的命令构造都是**纯函数**，可以在没有 FFmpeg 的机器上
//!    完整单测（见本文件末尾的测试）。

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, ErrorCode};

/// 统一附加的通用参数（规范 12 开头）
const COMMON_ARGS: [&str; 3] = ["-hide_banner", "-nostdin", "-y"];

/// 12.5 自检所需的编码器与滤镜
pub const REQUIRED_ENCODERS: [&str; 4] = ["libx264", "libx265", "libvpx-vp9", "libaom-av1"];
pub const REQUIRED_FILTERS: [&str; 4] = ["palettegen", "paletteuse", "zscale", "tonemap"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegPaths {
    pub ffmpeg: String,
    pub ffprobe: String,
    /// 是否为开发期从 PATH 找到的二进制（打包产物不应出现这种情况）
    pub from_path: bool,
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
}

/// 解析 FFmpeg 二进制位置（规范 9：Portable 取程序目录相对路径，安装版取安装目录下资源路径）。
///
/// 另外两条仅用于**开发期**：源码树的 `resources/ffmpeg/win-x64/`，以及在
/// debug 构建下回退到 PATH 上的 ffmpeg——后者会打日志说明，避免掩盖打包漏带二进制的问题。
pub fn resolve_paths() -> Result<FfmpegPaths, AppError> {
    let candidates: Vec<PathBuf> = [
        exe_dir().map(|d| d.join("resources/ffmpeg/win-x64")),
        exe_dir().map(|d| d.join("ffmpeg")),
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/ffmpeg/win-x64")),
    ]
    .into_iter()
    .flatten()
    .collect();

    for dir in candidates {
        let ffmpeg = dir.join("ffmpeg.exe");
        let ffprobe = dir.join("ffprobe.exe");
        if ffmpeg.is_file() && ffprobe.is_file() {
            return Ok(FfmpegPaths {
                ffmpeg: ffmpeg.to_string_lossy().into_owned(),
                ffprobe: ffprobe.to_string_lossy().into_owned(),
                from_path: false,
            });
        }
    }

    if cfg!(debug_assertions) && on_path("ffmpeg.exe") && on_path("ffprobe.exe") {
        return Ok(FfmpegPaths {
            ffmpeg: "ffmpeg.exe".into(),
            ffprobe: "ffprobe.exe".into(),
            from_path: true,
        });
    }

    Err(AppError::new(
        ErrorCode::FfmpegMissing,
        "找不到 FFmpeg。请把 GPL 构建的 ffmpeg.exe 与 ffprobe.exe 放到 \
         src-tauri/resources/ffmpeg/win-x64/（打包后为 程序目录/resources/ffmpeg/win-x64/）。",
    ))
}

fn on_path(name: &str) -> bool {
    Command::new(name)
        .arg("-version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// 探测（规范 12.4）
// ---------------------------------------------------------------------------

/// ffprobe 的 `streams[]` 元素。
///
/// 字段名与 ffprobe 自身的输出保持一致（snake_case），因此**不能**加 camelCase 重命名。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProbeStream {
    pub index: u32,
    #[serde(default)]
    pub codec_type: Option<String>,
    #[serde(default)]
    pub codec_name: Option<String>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub r_frame_rate: Option<String>,
    #[serde(default)]
    pub color_transfer: Option<String>,
    #[serde(default)]
    pub color_space: Option<String>,
    #[serde(default)]
    pub pix_fmt: Option<String>,
    #[serde(default)]
    pub nb_frames: Option<String>,
    #[serde(default)]
    pub tags: Option<serde_json::Value>,
    #[serde(default)]
    pub side_data_list: Option<Vec<serde_json::Value>>,
}

/// ffprobe 的 `format` 段；字段名同 ffprobe 输出（snake_case）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProbeFormat {
    #[serde(default)]
    pub duration: Option<String>,
    #[serde(default)]
    pub format_name: Option<String>,
    #[serde(default)]
    pub tags: Option<serde_json::Value>,
}

/// ffprobe 的完整输出；字段名同 ffprobe 输出（snake_case）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProbeResult {
    #[serde(default)]
    pub streams: Vec<ProbeStream>,
    #[serde(default)]
    pub format: ProbeFormat,
    #[serde(default)]
    pub chapters: Vec<serde_json::Value>,
}

/// 从探测结果中提炼出计划与界面需要的视频信息。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoInfo {
    /// 显示方向的宽高（已按旋转元数据换算）
    pub width: u32,
    pub height: u32,
    pub duration_sec: f64,
    pub fps: f64,
    /// 显示矩阵旋转角度
    pub rotation: i32,
    pub is_hdr: bool,
    pub has_audio: bool,
    pub has_subtitle: bool,
    pub chapter_count: usize,
    /// 是否含可变帧率等需要额外注意的特征
    pub pix_fmt: Option<String>,
}

impl ProbeResult {
    /// 视频流；没有视频流时返回 `None`。
    pub fn video_stream(&self) -> Option<&ProbeStream> {
        self.streams
            .iter()
            .find(|s| s.codec_type.as_deref() == Some("video"))
    }

    pub fn has_audio(&self) -> bool {
        self.streams
            .iter()
            .any(|s| s.codec_type.as_deref() == Some("audio"))
    }

    pub fn has_subtitle(&self) -> bool {
        self.streams
            .iter()
            .any(|s| s.codec_type.as_deref() == Some("subtitle"))
    }

    /// 旋转角度：优先取 `side_data_list` 里的 display matrix，其次取 `tags.rotate`。
    pub fn rotation(&self) -> i32 {
        let Some(video) = self.video_stream() else {
            return 0;
        };

        if let Some(list) = &video.side_data_list {
            for item in list {
                if let Some(rotation) = item.get("rotation").and_then(|v| v.as_f64()) {
                    // ffprobe 给的是逆时针角度，换算成惯用的顺时针角度并归一到 (-180, 180]
                    let normalized = (-rotation).round() as i32 % 360;
                    return if normalized > 180 {
                        normalized - 360
                    } else if normalized <= -180 {
                        normalized + 360
                    } else {
                        normalized
                    };
                }
            }
        }

        video
            .tags
            .as_ref()
            .and_then(|t| t.get("rotate"))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(0)
    }

    /// HDR 判定：PQ（smpte2084）或 HLG（arib-std-b67）传输特性即视为 HDR。
    pub fn is_hdr(&self) -> bool {
        self.video_stream()
            .and_then(|s| s.color_transfer.as_deref())
            .map(|t| matches!(t, "smpte2084" | "arib-std-b67"))
            .unwrap_or(false)
    }

    /// 旋转 90 / 270 度时显示宽高与编码宽高互换。
    pub fn swaps_axes(&self) -> bool {
        matches!(self.rotation().abs(), 90 | 270)
    }

    pub fn fps(&self) -> f64 {
        self.video_stream()
            .and_then(|s| s.r_frame_rate.as_deref())
            .and_then(parse_rational)
            .unwrap_or(0.0)
    }

    pub fn duration_sec(&self) -> f64 {
        self.format
            .duration
            .as_deref()
            .and_then(|d| d.parse::<f64>().ok())
            .unwrap_or(0.0)
    }

    pub fn to_video_info(&self) -> Option<VideoInfo> {
        let video = self.video_stream()?;
        let (mut w, mut h) = (video.width.unwrap_or(0), video.height.unwrap_or(0));
        if self.swaps_axes() {
            std::mem::swap(&mut w, &mut h);
        }
        Some(VideoInfo {
            width: w,
            height: h,
            duration_sec: self.duration_sec(),
            fps: self.fps(),
            rotation: self.rotation(),
            is_hdr: self.is_hdr(),
            has_audio: self.has_audio(),
            has_subtitle: self.has_subtitle(),
            chapter_count: self.chapters.len(),
            pix_fmt: video.pix_fmt.clone(),
        })
    }
}

fn parse_rational(value: &str) -> Option<f64> {
    match value.split_once('/') {
        Some((num, den)) => {
            let n = num.parse::<f64>().ok()?;
            let d = den.parse::<f64>().ok()?;
            (d != 0.0).then_some(n / d)
        }
        None => value.parse::<f64>().ok(),
    }
}

/// 调用 ffprobe 探测（规范 12.4）。
pub fn probe(paths: &FfmpegPaths, file: &Path) -> Result<ProbeResult, AppError> {
    let output = Command::new(&paths.ffprobe)
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_streams",
            "-show_format",
            "-show_chapters",
        ])
        .arg(file)
        .output()
        .map_err(|e| AppError::new(ErrorCode::DecodeFailed, format!("无法运行 ffprobe：{e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::new(
            ErrorCode::DecodeFailed,
            format!("ffprobe 无法解析该文件：{}", stderr.trim()),
        ));
    }

    serde_json::from_slice(&output.stdout).map_err(|e| {
        AppError::new(ErrorCode::DecodeFailed, format!("ffprobe 输出无法解析：{e}"))
    })
}

// ---------------------------------------------------------------------------
// 命令构建（规范 12.1，纯函数，便于单测）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Container {
    Mp4,
    Mkv,
    Other,
}

impl Container {
    pub fn from_ext(ext: &str) -> Self {
        match ext.to_ascii_lowercase().as_str() {
            "mp4" | "m4v" | "mov" => Self::Mp4,
            "mkv" | "webm" => Self::Mkv,
            _ => Self::Other,
        }
    }
}

/// 软件编码器标识（规范 6.9：四种编码器全部提供）
pub fn software_encoder(name: &str) -> &'static str {
    match name {
        "h265" | "hevc" => "libx265",
        "vp9" => "libvpx-vp9",
        "av1" => "libaom-av1",
        _ => "libx264",
    }
}

/// 硬件编码器标识；返回 `None` 表示该编码器没有对应的硬件实现。
pub fn hardware_encoder(codec: &str, accel: &str) -> Option<&'static str> {
    match (codec, accel) {
        ("h264", "nvenc") => Some("h264_nvenc"),
        ("h265" | "hevc", "nvenc") => Some("hevc_nvenc"),
        ("h264", "qsv") => Some("h264_qsv"),
        ("h265" | "hevc", "qsv") => Some("hevc_qsv"),
        ("h264", "amf") => Some("h264_amf"),
        ("h265" | "hevc", "amf") => Some("hevc_amf"),
        _ => None,
    }
}

/// HDR 色调映射到 SDR 的滤镜链（规范 12.1）
const TONEMAP_CHAIN: &str = "zscale=t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,tonemap=tonemap=hable:desat=0,zscale=t=bt709:m=bt709:r=tv,format=yuv420p";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoJob {
    pub input: String,
    pub output: String,
    pub width: u32,
    pub height: u32,
    /// libx264 / libx265 / libvpx-vp9 / libaom-av1 或对应的硬件编码器
    pub encoder: String,
    pub crf: u32,
    pub preset: String,
    /// 硬件编码时质量参数改用 `-cq`
    pub hardware: bool,
    /// 色调映射到 SDR；默认不加任何色彩滤镜，保持 HDR 原样传递
    pub tonemap_to_sdr: bool,
    pub container: Container,
    /// 目标码率（kbps）；给定时替代 CRF
    #[serde(default)]
    pub video_bitrate_k: Option<u32>,
    /// 目标帧率；给定且低于源帧率时用于降帧
    #[serde(default)]
    pub fps: Option<f64>,
}

impl VideoJob {
    /// 滤镜链：宽高已由 `compute_target` 保证为偶数，取偶不交给滤镜（规范 12.1）。
    fn filter(&self) -> String {
        let scale = format!("scale={}:{}:flags=lanczos", self.width, self.height);
        if self.tonemap_to_sdr {
            format!("{TONEMAP_CHAIN},{scale}")
        } else {
            scale
        }
    }
}

/// 构造 ffmpeg 参数数组（规范 12.1）。
pub fn build_args(job: &VideoJob) -> Vec<String> {
    let mut args: Vec<String> = COMMON_ARGS.iter().map(|s| (*s).to_string()).collect();

    args.push("-i".into());
    args.push(job.input.clone());

    // 旋转元数据：不使用 -noautorotate，交由 ffmpeg 按显示矩阵把画面摆正，
    // 滤镜因此作用在摆正后的帧上，输出观感方向与原视频一致。
    args.push("-vf".into());
    args.push(job.filter());

    args.push("-c:v".into());
    args.push(job.encoder.clone());

    if job.hardware {
        args.push("-cq".into());
        args.push(job.crf.to_string());
    } else {
        args.push("-crf".into());
        args.push(job.crf.to_string());
    }
    args.push("-preset".into());
    args.push(job.preset.clone());

    if let Some(kbps) = job.video_bitrate_k {
        args.push("-b:v".into());
        args.push(format!("{kbps}k"));
    }
    if let Some(fps) = job.fps {
        args.push("-r".into());
        args.push(format!("{fps}"));
    }

    // 音频默认直接复制，不重新编码（规范 6.9）
    args.push("-c:a".into());
    args.push("copy".into());

    // 保留全部流与元数据（含章节）
    args.push("-map".into());
    args.push("0".into());
    args.push("-map_metadata".into());
    args.push("0".into());

    // 容器专属参数
    match job.container {
        Container::Mp4 => {
            args.push("-movflags".into());
            args.push("+faststart".into());
            // MP4 承载不了原字幕格式，转 mov_text
            args.push("-c:s".into());
            args.push("mov_text".into());
        }
        Container::Mkv => {
            args.push("-c:s".into());
            args.push("copy".into());
        }
        Container::Other => {}
    }

    args.push(job.output.clone());
    args
}

/// 执行一次 ffmpeg 调用；失败时返回 stderr 摘要。
pub fn run_ffmpeg(paths: &FfmpegPaths, args: &[String]) -> Result<(), AppError> {
    let output = Command::new(&paths.ffmpeg)
        .args(args)
        .output()
        .map_err(|e| {
            AppError::new(ErrorCode::FfmpegEncodeFailed, format!("无法运行 ffmpeg：{e}"))
        })?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let tail: String = stderr
        .lines()
        .filter(|l| !l.trim().is_empty())
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" / ");

    Err(AppError::new(
        ErrorCode::FfmpegEncodeFailed,
        format!("编码失败：{tail}"),
    ))
}

/// 用给定的软件编码器重试一次（硬件编码不可用时的回退，规范 6.9）。
pub fn fallback_job(job: &VideoJob, codec: &str) -> VideoJob {
    VideoJob {
        encoder: software_encoder(codec).to_string(),
        hardware: false,
        ..job.clone()
    }
}

// ---------------------------------------------------------------------------
// 构建自检（规范 12.5）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfCheckReport {
    pub ffmpeg_found: bool,
    pub missing_encoders: Vec<String>,
    pub missing_filters: Vec<String>,
    pub missing_hevc_decoder: bool,
    /// 自检输出摘要，供写入构建日志
    pub summary: String,
}

impl SelfCheckReport {
    pub fn all_present(&self) -> bool {
        self.ffmpeg_found
            && self.missing_encoders.is_empty()
            && self.missing_filters.is_empty()
            && !self.missing_hevc_decoder
    }
}

fn list_missing(paths: &FfmpegPaths, flag: &str, required: &[&str]) -> (Vec<String>, Vec<String>) {
    let output = match Command::new(&paths.ffmpeg).arg(flag).output() {
        Ok(o) => o,
        Err(e) => return (required.iter().map(|s| (*s).to_string()).collect(), vec![e.to_string()]),
    };
    let text = String::from_utf8_lossy(&output.stdout).to_lowercase();
    let missing = required
        .iter()
        .filter(|name| !text.contains(&name.to_lowercase()))
        .map(|s| (*s).to_string())
        .collect();
    (missing, Vec::new())
}

/// 打包前自检：确认所用构建包含所需组件（规范 12.5）。
///
/// 文档给的是 `ffmpeg -encoders | findstr …` 这类管道；这里改为在 Rust 侧过滤输出，
/// 同样是那三项检查，但不拼接 shell 字符串（规范第十二节的要求）。
pub fn self_check(paths: &FfmpegPaths) -> SelfCheckReport {
    let (missing_encoders, _) = list_missing(paths, "-encoders", &REQUIRED_ENCODERS);
    let (missing_filters, _) = list_missing(paths, "-filters", &REQUIRED_FILTERS);
    let (missing_decoders, _) = list_missing(paths, "-decoders", &["hevc"]);

    let lines = [
        format!("encoders 缺失：{:?}", missing_encoders),
        format!("filters 缺失：{:?}", missing_filters),
        format!("hevc 解码器缺失：{}", !missing_decoders.is_empty()),
        format!("ffmpeg：{}", paths.ffmpeg),
    ];

    SelfCheckReport {
        ffmpeg_found: true,
        missing_encoders,
        missing_filters,
        missing_hevc_decoder: !missing_decoders.is_empty(),
        summary: lines.join("；"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(encoder: &str, container: Container) -> VideoJob {
        VideoJob {
            input: "in.mp4".into(),
            output: "out.mp4".into(),
            width: 1280,
            height: 720,
            encoder: encoder.into(),
            crf: 23,
            preset: "medium".into(),
            hardware: false,
            tonemap_to_sdr: false,
            container,
            video_bitrate_k: None,
            fps: None,
        }
    }

    fn joined(args: &[String]) -> String {
        args.join(" ")
    }

    #[test]
    fn common_args_are_always_prepended() {
        // 规范第十二节开头要求统一附加 -hide_banner -nostdin -y
        let args = build_args(&job("libx264", Container::Mp4));
        assert_eq!(&args[0..3], &["-hide_banner", "-nostdin", "-y"]);
    }

    #[test]
    fn scale_filter_uses_even_dimensions_verbatim() {
        let args = build_args(&job("libx264", Container::Mp4));
        let idx = args.iter().position(|a| a == "-vf").unwrap();
        // 宽高已由 compute_target 取偶，滤镜里不再做取偶处理
        assert_eq!(args[idx + 1], "scale=1280:720:flags=lanczos");
    }

    #[test]
    fn keeps_audio_untouched_and_maps_everything() {
        let args = build_args(&job("libx264", Container::Mp4));
        let text = joined(&args);
        // 音频直接复制、不重新编码
        assert!(text.contains("-c:a copy"));
        // 保留全部流与元数据（含章节）
        assert!(text.contains("-map 0"));
        assert!(text.contains("-map_metadata 0"));
        // 不得出现会破坏显示方向的 -noautorotate
        assert!(!text.contains("-noautorotate"));
    }

    #[test]
    fn mp4_and_mkv_get_their_container_specific_flags() {
        let mp4 = joined(&build_args(&job("libx264", Container::Mp4)));
        assert!(mp4.contains("-movflags +faststart"));
        assert!(mp4.contains("-c:s mov_text"));

        let mkv = joined(&build_args(&job("libx264", Container::Mkv)));
        assert!(!mkv.contains("-movflags"));
        assert!(mkv.contains("-c:s copy"));
    }

    #[test]
    fn hardware_uses_cq_instead_of_crf() {
        let mut hw = job("h264_nvenc", Container::Mp4);
        hw.hardware = true;
        let text = joined(&build_args(&hw));
        assert!(text.contains("-cq 23"));
        assert!(!text.contains("-crf"));
    }

    #[test]
    fn hdr_is_passed_through_by_default() {
        // 默认不加任何色彩滤镜，HDR 原样传递
        let text = joined(&build_args(&job("libx264", Container::Mp4)));
        assert!(!text.contains("zscale"));
        assert!(!text.contains("tonemap"));
    }

    #[test]
    fn tonemap_chain_replaces_plain_scale() {
        let mut sdr = job("libx264", Container::Mp4);
        sdr.tonemap_to_sdr = true;
        let args = build_args(&sdr);
        let idx = args.iter().position(|a| a == "-vf").unwrap();
        let filter = &args[idx + 1];
        assert!(filter.contains("tonemap=tonemap=hable"));
        assert!(filter.contains("zscale=t=bt709"));
        // 缩放仍然按目标宽高
        assert!(filter.ends_with("scale=1280:720:flags=lanczos"));
    }

    #[test]
    fn bitrate_and_fps_modes_add_their_flags() {
        let mut paced = job("libx264", Container::Mp4);
        paced.video_bitrate_k = Some(2500);
        paced.fps = Some(24.0);
        let text = joined(&build_args(&paced));
        assert!(text.contains("-b:v 2500k"));
        assert!(text.contains("-r 24"));
    }

    #[test]
    fn software_encoder_mapping_covers_all_four() {
        assert_eq!(software_encoder("h264"), "libx264");
        assert_eq!(software_encoder("h265"), "libx265");
        assert_eq!(software_encoder("vp9"), "libvpx-vp9");
        assert_eq!(software_encoder("av1"), "libaom-av1");
        assert_eq!(software_encoder("未知"), "libx264");
    }

    #[test]
    fn hardware_encoder_mapping_and_gaps() {
        assert_eq!(hardware_encoder("h264", "nvenc"), Some("h264_nvenc"));
        assert_eq!(hardware_encoder("h265", "qsv"), Some("hevc_qsv"));
        assert_eq!(hardware_encoder("h264", "amf"), Some("h264_amf"));
        // VP9 / AV1 没有对应的硬件实现，应回退软件编码
        assert_eq!(hardware_encoder("vp9", "nvenc"), None);
        assert_eq!(hardware_encoder("av1", "amf"), None);
    }

    #[test]
    fn fallback_job_switches_to_software_encoder() {
        let mut hw = job("h264_nvenc", Container::Mp4);
        hw.hardware = true;
        let fb = fallback_job(&hw, "h265");
        assert_eq!(fb.encoder, "libx265");
        assert!(!fb.hardware);
        // 其余参数原样保留
        assert_eq!((fb.width, fb.height, fb.crf), (1280, 720, 23));
    }

    #[test]
    fn container_detection_by_extension() {
        assert_eq!(Container::from_ext("MP4"), Container::Mp4);
        assert_eq!(Container::from_ext("mov"), Container::Mp4);
        assert_eq!(Container::from_ext("mkv"), Container::Mkv);
        assert_eq!(Container::from_ext("webm"), Container::Mkv);
        assert_eq!(Container::from_ext("avi"), Container::Other);
    }

    // ---------- 探测结果解析 ----------

    fn probe_from(json_text: &str) -> ProbeResult {
        serde_json::from_str(json_text).unwrap()
    }

    const PLAIN: &str = r#"{
      "streams": [
        {"index":0,"codec_type":"video","codec_name":"h264","width":1920,"height":1080,
         "r_frame_rate":"30000/1001","pix_fmt":"yuv420p","color_transfer":"bt709"},
        {"index":1,"codec_type":"audio","codec_name":"aac"}
      ],
      "format": {"duration":"12.345000","format_name":"mov,mp4,m4a,3gp,3g2,mj2"},
      "chapters": [{"id":0},{"id":1}]
    }"#;

    #[test]
    fn parses_plain_mp4() {
        let probe = probe_from(PLAIN);
        let info = probe.to_video_info().unwrap();
        assert_eq!((info.width, info.height), (1920, 1080));
        assert!((info.duration_sec - 12.345).abs() < 1e-6);
        assert!((info.fps - 29.97).abs() < 0.01);
        assert!(!info.is_hdr);
        assert!(info.has_audio);
        assert!(!info.has_subtitle);
        assert_eq!(info.chapter_count, 2);
        assert_eq!(info.rotation, 0);
    }

    #[test]
    fn rotation_swaps_reported_dimensions() {
        // 手机竖拍：编码 1920×1080，display matrix 旋转 -90 → 观感为 1080×1920
        let json = r#"{
          "streams": [{"index":0,"codec_type":"video","width":1920,"height":1080,
            "side_data_list":[{"side_data_type":"Display Matrix","rotation":-90}]}],
          "format": {"duration":"5.0"}
        }"#;
        let probe = probe_from(json);
        assert_eq!(probe.rotation(), 90);
        assert!(probe.swaps_axes());
        let info = probe.to_video_info().unwrap();
        assert_eq!((info.width, info.height), (1080, 1920), "应按观感方向报告宽高");
    }

    #[test]
    fn rotation_from_tags_is_understood() {
        let json = r#"{
          "streams": [{"index":0,"codec_type":"video","width":1080,"height":1920,
            "tags":{"rotate":"270"}}],
          "format": {"duration":"1.0"}
        }"#;
        let probe = probe_from(json);
        assert_eq!(probe.rotation(), 270);
        let info = probe.to_video_info().unwrap();
        assert_eq!((info.width, info.height), (1920, 1080));
    }

    #[test]
    fn detects_hdr_transfer_characteristics() {
        for transfer in ["smpte2084", "arib-std-b67"] {
            let json = format!(
                r#"{{"streams":[{{"index":0,"codec_type":"video","width":3840,"height":2160,
                   "color_transfer":"{transfer}","color_space":"bt2020nc"}}],
                   "format":{{"duration":"1.0"}}}}"#
            );
            assert!(probe_from(&json).is_hdr(), "{transfer} 应判定为 HDR");
        }
        assert!(!probe_from(PLAIN).is_hdr(), "bt709 不是 HDR");
    }

    #[test]
    fn subtitle_presence_is_reported() {
        let json = r#"{
          "streams": [
            {"index":0,"codec_type":"video","width":640,"height":360},
            {"index":1,"codec_type":"subtitle","codec_name":"subrip"}
          ],
          "format": {"duration":"1.0"}
        }"#;
        let probe = probe_from(json);
        assert!(probe.has_subtitle());
        assert!(!probe.has_audio());
    }

    #[test]
    fn missing_video_stream_yields_no_info() {
        let json = r#"{"streams":[{"index":0,"codec_type":"audio"}],"format":{}}"#;
        let probe = probe_from(json);
        assert!(probe.to_video_info().is_none());
    }

    #[test]
    fn fps_parsing_handles_rational_and_plain() {
        assert!((parse_rational("25/1").unwrap() - 25.0).abs() < 1e-9);
        assert!((parse_rational("30000/1001").unwrap() - 29.97).abs() < 0.01);
        assert!((parse_rational("24").unwrap() - 24.0).abs() < 1e-9);
        assert!(parse_rational("0/0").is_none());
        assert!(parse_rational("abc").is_none());
    }

    #[test]
    fn self_check_reports_missing_binaries_instead_of_panicking() {
        // 指向不存在的二进制：应如实报告，而不是崩溃
        let paths = FfmpegPaths {
            ffmpeg: "definitely-not-a-real-ffmpeg.exe".into(),
            ffprobe: "definitely-not-a-real-ffprobe.exe".into(),
            from_path: false,
        };
        let report = self_check(&paths);
        assert!(!report.all_present());
        assert_eq!(report.missing_encoders.len(), REQUIRED_ENCODERS.len());
        assert!(report.missing_hevc_decoder);
    }

    #[test]
    fn resolve_paths_reports_missing_ffmpeg_clearly() {
        // 本机当前没有 FFmpeg；解析应给出可操作的中文提示而不是 panic。
        // 若开发机上装了 FFmpeg，则这里应当解析成功——两种结果都算通过。
        match resolve_paths() {
            Ok(paths) => assert!(!paths.ffmpeg.is_empty()),
            Err(err) => {
                assert_eq!(err.code, ErrorCode::FfmpegMissing);
                assert!(err.message.contains("ffmpeg.exe"), "实际：{}", err.message);
            }
        }
    }
}
