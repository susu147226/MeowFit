//! 动图处理（规范 5.2、6.10、12.2、12.3）。
//!
//! 覆盖 GIF、动态 WebP、APNG。要求保留帧数、每帧延迟、循环次数、透明通道与
//! 帧偏移，并且**每帧等尺寸**、调色板不串色。
//!
//! 关键点：不能只看扩展名。`.webp` 与 `.png` 既可能是静态也可能是动态，
//! 必须读容器内容判定，否则动态 WebP 会被当成静态图处理——只写出第一帧。

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::ffmpeg::FfmpegPaths;

/// 动图信息。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationInfo {
    pub width: u32,
    pub height: u32,
    /// 帧数；`None` 表示无法确定
    pub frames: Option<usize>,
    /// 循环次数；`0` 表示无限循环
    pub loop_count: u32,
    pub has_alpha: bool,
}

/// 判断该文件是否真的是动图。`ext` 为小写扩展名。
///
/// - `gif` / `apng` 直接视为动图；
/// - `webp` 看 RIFF 里有没有 `ANIM` 块；
/// - `png` 看有没有 `acTL` 块（APNG 的动画控制块）。
///
/// 容器标识块一定出现在图像数据之前，因此只读文件开头即可判定。
pub fn is_animated(path: &Path, ext: &str) -> bool {
    match ext {
        "gif" | "apng" => true,
        "webp" => head(path, 64 * 1024).map(|b| contains(&b, b"ANIM")).unwrap_or(false),
        "png" => head(path, 64 * 1024).map(|b| contains(&b, b"acTL")).unwrap_or(false),
        _ => false,
    }
}

fn head(path: &Path, limit: usize) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    let mut buf = vec![0u8; limit];
    let read = file.read(&mut buf).ok()?;
    buf.truncate(read);
    Some(buf)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// 从容器里读出循环次数。
///
/// - GIF：`NETSCAPE2.0` 应用扩展块里的子块 `03 01 <loop_lo> <loop_hi>`，0 为无限；
/// - 动态 WebP：`ANIM` 块的第 5、6 字节（背景色之后的循环次数）；
/// - APNG：`acTL` 块的第 2 个 4 字节（`num_plays`）。
fn read_loop_count(path: &Path, ext: &str) -> u32 {
    let Some(bytes) = head(path, 256 * 1024) else {
        return 0;
    };
    loop_from_bytes(&bytes, ext)
}

/// 从容器字节里解析循环次数（拆出来是为了能单独测，不必造真实文件）。
fn loop_from_bytes(bytes: &[u8], ext: &str) -> u32 {
    match ext {
        "gif" => {
            let Some(at) = bytes.windows(11).position(|w| w == b"NETSCAPE2.0") else {
                return 0;
            };
            // 结构：0x21 0xFF 0x0B "NETSCAPE2.0" 0x03 0x01 <loop_lo> <loop_hi> 0x00
            // 标识之后是 2 字节子块头（03 01），紧跟 2 字节循环次数
            let value_at = at + 11 + 2;
            if value_at + 2 > bytes.len() {
                return 0;
            }
            u16::from_le_bytes([bytes[value_at], bytes[value_at + 1]]) as u32
        }
        "webp" => {
            let Some(at) = bytes.windows(4).position(|w| w == b"ANIM") else {
                return 0;
            };
            // ANIM 块：4 字节背景色 + 2 字节循环次数
            if at + 10 > bytes.len() {
                return 0;
            }
            u16::from_le_bytes([bytes[at + 8], bytes[at + 9]]) as u32
        }
        "apng" | "png" => {
            let Some(at) = bytes.windows(4).position(|w| w == b"acTL") else {
                return 0;
            };
            // acTL：4 字节帧数 + 4 字节循环次数
            if at + 12 > bytes.len() {
                return 0;
            }
            u32::from_be_bytes([bytes[at + 8], bytes[at + 9], bytes[at + 10], bytes[at + 11]])
        }
        _ => 0,
    }
}

/// 探测动图信息。尺寸读容器头，帧数交给 ffprobe。
pub fn probe(paths: &FfmpegPaths, path: &Path, ext: &str) -> Result<AnimationInfo, AppError> {
    let (width, height) = image::image_dimensions(path)
        .map_err(|e| AppError::decode_failed(format!("无法读取动图尺寸：{e}")))?;

    let frames = crate::ffmpeg::probe(paths, path)
        .ok()
        .and_then(|result| result.video_stream().and_then(|s| s.nb_frames.clone()))
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|n| *n > 0);

    Ok(AnimationInfo {
        width,
        height,
        frames,
        loop_count: read_loop_count(path, ext),
        has_alpha: matches!(ext, "gif" | "apng" | "png" | "webp"),
    })
}

/// 动图缩放参数（规范 6.10 / 12.2）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationJob {
    pub input: String,
    pub output: String,
    pub width: u32,
    pub height: u32,
    /// 调色板颜色数：256 / 128 / 64
    pub colors: u32,
    /// 抖动开关
    pub dither: bool,
    /// 循环次数，0 为无限循环
    pub loop_count: u32,
}

const DITHER_ON: &str = "bayer";
const DITHER_OFF: &str = "none";

/// 构造动图缩放的 ffmpeg 参数（规范 12.2）。
///
/// GIF 用「单次解码 + 分色调色板」：先 `palettegen` 生成调色板，再 `paletteuse`
/// 应用，避免逐帧各自量化造成的串色与二次量化。
pub fn build_resize_args(job: &AnimationJob) -> Vec<String> {
    let mut args: Vec<String> = ["-hide_banner", "-nostdin", "-y"]
        .iter()
        .map(|s| (*s).to_string())
        .collect();

    args.push("-i".into());
    args.push(job.input.clone());

    let ext = std::path::Path::new(&job.output)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let scale = format!("scale={}:{}:flags=lanczos", job.width, job.height);

    if ext == "gif" {
        let dither = if job.dither { DITHER_ON } else { DITHER_OFF };
        args.push("-filter_complex".into());
        // reserve_transparent + alpha_threshold 才能保住透明通道，
        // 否则透明区域会被量化成某个不透明颜色
        args.push(format!(
            "[0:v]{scale},split[a][b];[a]palettegen=reserve_transparent=1:max_colors={}[p];[b][p]paletteuse=dither={}:alpha_threshold=128",
            job.colors.clamp(2, 256),
            dither
        ));
        args.push("-loop".into());
        args.push(job.loop_count.to_string());
    } else {
        // WebP / APNG：保留 alpha，直接等比缩放即可，无需求色板
        args.push("-vf".into());
        args.push(scale);
        if ext == "webp" {
            args.push("-c:v".into());
            args.push("libwebp_anim".into());
            args.push("-loop".into());
            args.push(job.loop_count.to_string());
        } else if ext == "apng" || ext == "png" {
            args.push("-f".into());
            args.push("apng".into());
            args.push("-plays".into());
            args.push(job.loop_count.to_string());
        }
    }

    // 这里**不能**加 `-map 0`：filter_complex 已经产出了要用的流，
    // 再显式映射原始流会把滤镜链的结果挤掉，导致「没有任何包写入输出文件」。
    // 动图只有一个视频流，默认流选择即可（规范 12.2 的模板也没有 -map）。
    args.push(job.output.clone());
    args
}

/// 动图转视频的参数（规范 12.3）。
pub fn build_to_video_args(job: &AnimationJob, video: &crate::ffmpeg::VideoJob) -> Vec<String> {
    // 从头构造，而不是在动图参数上删删补补：删标志位时很容易把它的值落下，
    // 那个孤立的值会被 ffmpeg 当成输出文件名。
    let mut args: Vec<String> = ["-hide_banner", "-nostdin", "-y"]
        .iter()
        .map(|s| (*s).to_string())
        .collect();

    args.push("-i".into());
    args.push(job.input.clone());
    args.push("-vf".into());
    args.push(format!("scale={}:{}:flags=lanczos", job.width, job.height));
    args.push("-c:v".into());
    args.push(video.encoder.clone());
    if video.hardware {
        args.push("-cq".into());
    } else {
        args.push("-crf".into());
    }
    args.push(video.crf.to_string());
    args.push("-pix_fmt".into());
    args.push("yuv420p".into());

    // +faststart 只对 MP4 / MOV 合法，WebM 加上会直接报错
    let out_ext = std::path::Path::new(&job.output)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(out_ext.as_str(), "mp4" | "mov" | "m4v") {
        args.push("-movflags".into());
        args.push("+faststart".into());
    }
    // 动图没有音轨
    args.push("-an".into());
    args.push(job.output.clone());
    args
}

/// 体积阶梯的一档（规范 6.10：降尺寸 + 减色数 + 丢帧三档策略逼近）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LadderStep {
    /// 档位说明，如「减色数 128」「降尺寸 90%」「丢帧 1/2」
    pub label: String,
    pub colors: u32,
    pub scale: f64,
    /// 帧率上限；`None` 表示保持原帧率
    pub max_fps: Option<f64>,
}

/// 生成体积阶梯。按规范 6.10 的三档策略依次加码：先减色数，再降尺寸，最后丢帧。
///
/// 档位总数有上限（规范 10.7：不得无限循环）：减色数最多 2 档、降尺寸最多 8 级、
/// 丢帧 1 档，合计 11 档。
pub fn build_ladder(colors: u32, fps: f64) -> Vec<LadderStep> {
    let mut steps = Vec::new();
    let mut current = colors.clamp(2, 256);

    // 第一档：减色数 256 → 128 → 64
    for next in [128u32, 64] {
        if current > next {
            current = next;
            steps.push(LadderStep {
                label: format!("减色数 {next}"),
                colors: next,
                scale: 1.0,
                max_fps: None,
            });
        }
    }

    // 第二档：按 0.9 逐级降尺寸，最多 8 级，下限 64px
    let mut scale = 1.0f64;
    for level in 1..=8 {
        scale *= 0.9;
        // 以 256×256 为参考估算下限，避免降到 64px 以下
        if 256.0 * scale < 64.0 {
            break;
        }
        steps.push(LadderStep {
            label: format!("降尺寸 {:.0}%（第 {level} 级）", scale * 100.0),
            colors: current,
            scale,
            max_fps: None,
        });
    }

    // 第三档：丢帧，最多降到原帧率的一半（规范 10.7）
    if fps > 0.0 {
        steps.push(LadderStep {
            label: format!("丢帧 1/2（{:.0} → {:.0} fps）", fps, fps / 2.0),
            colors: current,
            scale,
            max_fps: Some(fps / 2.0),
        });
    }

    steps
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(ext: &str) -> AnimationJob {
        AnimationJob {
            input: "in.gif".into(),
            output: format!("out.{ext}"),
            width: 250,
            height: 250,
            colors: 256,
            dither: false,
            loop_count: 0,
        }
    }

    #[test]
    fn common_args_and_single_decode_palette_chain() {
        let args = build_resize_args(&job("gif"));
        assert_eq!(&args[0..3], &["-hide_banner", "-nostdin", "-y"]);
        let text = args.join(" ");
        // 单次解码 + 分色调色板：一次 split，palettegen 与 paletteuse 各一次
        assert!(text.contains("palettegen=reserve_transparent=1:max_colors=256"));
        assert!(text.contains("paletteuse=dither=none"));
        assert_eq!(text.matches("split").count(), 1);
        assert!(text.contains("scale=250:250:flags=lanczos"));
        assert!(text.contains("-loop 0"));
    }

    #[test]
    fn colors_are_clamped_and_dither_switch_works() {
        let mut j = job("gif");
        j.colors = 64;
        j.dither = true;
        let text = build_resize_args(&j).join(" ");
        assert!(text.contains("max_colors=64"));
        assert!(text.contains("dither=bayer"));

        j.colors = 9999;
        assert!(build_resize_args(&j).join(" ").contains("max_colors=256"));
    }

    #[test]
    fn animated_webp_keeps_alpha_and_loop() {
        let mut j = job("webp");
        j.loop_count = 3;
        let text = build_resize_args(&j).join(" ");
        assert!(text.contains("libwebp_anim"));
        assert!(text.contains("-loop 3"));
        // 动图 WebP 不应走调色板链
        assert!(!text.contains("palettegen"));
    }

    #[test]
    fn apng_uses_plays_not_loop() {
        let mut j = job("apng");
        j.loop_count = 2;
        let text = build_resize_args(&j).join(" ");
        assert!(text.contains("-f apng"));
        assert!(text.contains("-plays 2"));
    }

    #[test]
    fn gif_loop_count_is_read_at_the_right_offset() {
        // 0x21 0xFF 0x0B "NETSCAPE2.0" 0x03 0x01 <loop> 0x00
        let mut bytes = vec![0x47, 0x49, 0x46, 0x38, 0x39, 0x61];
        bytes.extend_from_slice(&[0x21, 0xFF, 0x0B]);
        bytes.extend_from_slice(b"NETSCAPE2.0");
        bytes.extend_from_slice(&[0x03, 0x01, 0x03, 0x00, 0x00]);
        assert_eq!(loop_from_bytes(&bytes, "gif"), 3, "应读到 3 次循环");

        // 0 表示无限循环
        let mut infinite = bytes.clone();
        let at = infinite.len() - 3;
        infinite[at] = 0x00;
        infinite[at + 1] = 0x00;
        assert_eq!(loop_from_bytes(&infinite, "gif"), 0);

        // 没有该块时退回 0，而不是 panic
        assert_eq!(loop_from_bytes(b"GIF89a", "gif"), 0);
    }

    #[test]
    fn webp_and_apng_loop_count_offsets() {
        // RIFF....WEBP + ANIM 块：4 字节背景色 + 2 字节循环次数
        let mut webp = Vec::new();
        webp.extend_from_slice(b"RIFF");
        webp.extend_from_slice(&[0, 0, 0, 0]);
        webp.extend_from_slice(b"WEBPANIM");
        webp.extend_from_slice(&[0, 0, 0, 0]); // 背景色
        webp.extend_from_slice(&[0x02, 0x00]); // 循环 2 次
        assert_eq!(loop_from_bytes(&webp, "webp"), 2);

        // acTL 块：4 字节帧数 + 4 字节 num_plays（大端）
        let mut apng = Vec::new();
        apng.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        apng.extend_from_slice(b"acTL");
        apng.extend_from_slice(&5u32.to_be_bytes()); // 帧数
        apng.extend_from_slice(&7u32.to_be_bytes()); // 循环 7 次
        assert_eq!(loop_from_bytes(&apng, "apng"), 7);
    }

    #[test]
    fn ladder_escalates_colors_then_size_then_fps_and_is_bounded() {
        let steps = build_ladder(256, 10.0);
        // 第一步必然是减色数
        assert!(steps[0].label.contains("减色数 128"), "实际：{}", steps[0].label);
        // 减色数只走 128、64 两档
        assert_eq!(steps.iter().filter(|s| s.label.contains("减色数")).count(), 2);
        // 随后是降尺寸，且不出现降帧率之外的档位
        assert!(steps.iter().any(|s| s.label.contains("降尺寸")));
        // 最后一档是丢帧
        assert!(steps.last().unwrap().label.contains("丢帧"));
        // 总档位数有上限，不会无限循环
        assert!(steps.len() <= 11, "档位数应有限，实际 {}", steps.len());
        // 丢帧档不得超过原帧率的一半
        assert_eq!(steps.last().unwrap().max_fps, Some(5.0));
    }

    #[test]
    fn ladder_starts_from_the_current_color_count() {
        let steps = build_ladder(64, 0.0);
        // 已经是 64 色，不应再出现减色数档
        assert!(!steps.iter().any(|s| s.label.contains("减色数")));
        // 源帧率未知时不安排丢帧
        assert!(!steps.iter().any(|s| s.label.contains("丢帧")));
        assert!(steps.iter().any(|s| s.label.contains("降尺寸")));
    }

    #[test]
    fn webm_output_does_not_get_movflags() {
        let mut j = job("gif");
        j.output = "out.webm".into();
        let video = crate::ffmpeg::VideoJob {
            input: j.input.clone(),
            output: j.output.clone(),
            width: 250,
            height: 250,
            encoder: "libvpx-vp9".into(),
            crf: 32,
            preset: "medium".into(),
            hardware: false,
            tonemap_to_sdr: false,
            container: crate::ffmpeg::Container::Mkv,
            video_bitrate_k: None,
            fps: None,
        };
        let text = build_to_video_args(&j, &video).join(" ");
        assert!(!text.contains("-movflags"), "WebM 不能带 +faststart");
        assert!(text.contains("libvpx-vp9"));
    }

    #[test]
    fn to_video_replaces_animation_flags_with_encoder_settings() {
        let mut j = job("gif");
        j.output = "out.mp4".into();
        j.width = 250;
        j.height = 250;
        let video = crate::ffmpeg::VideoJob {
            input: j.input.clone(),
            output: j.output.clone(),
            width: 250,
            height: 250,
            encoder: "libx264".into(),
            crf: 23,
            preset: "medium".into(),
            hardware: false,
            tonemap_to_sdr: false,
            container: crate::ffmpeg::Container::Mp4,
            video_bitrate_k: None,
            fps: None,
        };
        let args = build_to_video_args(&j, &video);
        let text = args.join(" ");
        assert!(!text.contains("-loop"), "转视频不应保留动图循环参数");
        assert!(!text.contains("palettegen"), "转视频不应保留调色板链");
        assert!(text.contains("-c:v libx264"));
        assert!(text.contains("-crf 23"));
        assert!(text.contains("-pix_fmt yuv420p"));
        assert!(text.contains("-movflags +faststart"));
        // 输出文件必须仍在最后
        assert_eq!(args.last().unwrap(), "out.mp4");
    }
}
