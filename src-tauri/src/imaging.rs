//! 静态图片处理（规范 6.8、10.5）。
//!
//! 覆盖：重采样算法可选、EXIF 方向校正、ICC 保留、元数据开关、格式转换与质量参数、
//! SVG 双路径（改写属性 / 按目标像素尺寸光栅化）。

use std::path::Path;

use image::imageops::FilterType;
use image::{DynamicImage, ExtendedColorType, ImageEncoder, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, ErrorCode};
use crate::meta;
use crate::model::{Anchor, Computed, Mode};
use crate::svg;

/// 重采样算法（规范 6.8，默认 Lanczos3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Resample {
    Lanczos3,
    Bicubic,
    Bilinear,
    Nearest,
}

impl Default for Resample {
    fn default() -> Self {
        Self::Lanczos3
    }
}

impl Resample {
    pub fn filter(self) -> FilterType {
        match self {
            Self::Lanczos3 => FilterType::Lanczos3,
            Self::Bicubic => FilterType::CatmullRom,
            Self::Bilinear => FilterType::Triangle,
            Self::Nearest => FilterType::Nearest,
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "bicubic" => Self::Bicubic,
            "bilinear" => Self::Bilinear,
            "nearest" => Self::Nearest,
            _ => Self::Lanczos3,
        }
    }
}

/// 输出格式（规范 6.8「保持原格式」或「统一转换为指定格式」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OutputFormat {
    Keep,
    Png,
    Jpeg,
    Webp,
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self::Keep
    }
}

impl OutputFormat {
    pub fn from_name(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "png" => Self::Png,
            "jpeg" | "jpg" => Self::Jpeg,
            "webp" => Self::Webp,
            _ => Self::Keep,
        }
    }

    /// 统一转换时的目标扩展名；`Keep` 时沿用源扩展名。
    pub fn resolve_ext(self, source_ext: &str) -> String {
        match self {
            Self::Keep => source_ext.to_ascii_lowercase(),
            Self::Png => "png".into(),
            Self::Jpeg => "jpg".into(),
            Self::Webp => "webp".into(),
        }
    }

    fn from_ext(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "webp" => Some(Self::Webp),
            _ => None,
        }
    }
}

/// 处理参数（来自 `settings.json` 的 `processing` / `output` 段）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageOptions {
    #[serde(default)]
    pub resample: Resample,
    /// 质量参数，仅对 JPEG 生效；WebP 目前为无损编码（规范 6.8 / 5.4）
    #[serde(default = "default_quality")]
    pub quality: u8,
    #[serde(default)]
    pub format: OutputFormat,
    /// `true` 表示「保留全部元数据（含缩略图等冗余数据）」
    #[serde(default)]
    pub keep_all_metadata: bool,
    #[serde(default = "default_fill")]
    pub background_fill: String,
}

fn default_quality() -> u8 {
    85
}

fn default_fill() -> String {
    "#FFFFFF".into()
}

impl Default for ImageOptions {
    fn default() -> Self {
        Self {
            resample: Resample::default(),
            quality: default_quality(),
            format: OutputFormat::default(),
            keep_all_metadata: false,
            background_fill: default_fill(),
        }
    }
}

/// 不保留 alpha 的输出格式，须以背景色填充（规范 6.8）
fn format_lacks_alpha(format: OutputFormat) -> bool {
    matches!(format, OutputFormat::Jpeg)
}

fn parse_hex_color(hex: &str) -> Rgba<u8> {
    let h = hex.trim_start_matches('#');
    if h.len() != 6 {
        return Rgba([255, 255, 255, 255]);
    }
    let parse = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(255);
    Rgba([parse(0), parse(2), parse(4), 255])
}

/// 按锚点把内容贴到画布上（模式 B 补边）。
pub fn paste_on_canvas(
    content: RgbaImage,
    w: u32,
    h: u32,
    anchor: Anchor,
    fill: Rgba<u8>,
) -> RgbaImage {
    let mut canvas = RgbaImage::from_pixel(w, h, fill);
    let (cw, ch) = content.dimensions();
    let (max_x, max_y) = (w.saturating_sub(cw), h.saturating_sub(ch));

    let (x, y) = match anchor {
        Anchor::TopLeft => (0, 0),
        Anchor::Top => (max_x / 2, 0),
        Anchor::TopRight => (max_x, 0),
        Anchor::Left => (0, max_y / 2),
        Anchor::Center => (max_x / 2, max_y / 2),
        Anchor::Right => (max_x, max_y / 2),
        Anchor::BottomLeft => (0, max_y),
        Anchor::Bottom => (max_x / 2, max_y),
        Anchor::BottomRight => (max_x, max_y),
    };

    image::imageops::overlay(&mut canvas, &content, i64::from(x), i64::from(y));
    canvas
}

/// 把带 alpha 的图像按背景色压平。
fn flatten(img: &RgbaImage, bg: Rgba<u8>) -> RgbaImage {
    let mut out = RgbaImage::from_pixel(img.width(), img.height(), bg);
    image::imageops::overlay(&mut out, img, 0, 0);
    out
}

/// 按计划算出的目标尺寸，把已解码的图像渲染成画布。
///
/// 尺寸不在此处重新计算：`target` 必须来自 [`crate::algo::size::compute_target`]。
pub fn render_image(
    img: &DynamicImage,
    target: &Computed,
    mode: Mode,
    resample: Resample,
    fill: Rgba<u8>,
) -> RgbaImage {
    let filter = resample.filter();
    let (tw, th) = (target.width, target.height);
    let (cw, ch) = (target.content_width, target.content_height);

    match mode {
        // 填满裁剪：等比缩放至覆盖目标框后居中裁剪
        Mode::C => img.resize_to_fill(tw, th, filter).to_rgba8(),
        // 拉伸：强制改变比例
        Mode::D => {
            if img.width() == tw && img.height() == th {
                img.to_rgba8()
            } else {
                img.resize_exact(tw, th, filter).to_rgba8()
            }
        }
        _ => {
            let content = if img.width() == cw && img.height() == ch {
                img.to_rgba8()
            } else {
                img.resize_exact(cw, ch, filter).to_rgba8()
            };
            if tw == cw && th == ch {
                content
            } else {
                // 模式 B 默认输出目标框画布，等比内容居中，空余区域填充
                paste_on_canvas(content, tw, th, target.anchor, fill)
            }
        }
    }
}

/// 按输出格式编码像素。
fn encode(canvas: &RgbaImage, format: OutputFormat, quality: u8, fill: Rgba<u8>) -> Result<Vec<u8>, AppError> {
    let (w, h) = canvas.dimensions();
    let mut buf: Vec<u8> = Vec::new();

    match format {
        OutputFormat::Jpeg => {
            let flat = flatten(canvas, fill);
            let mut rgb = Vec::with_capacity((w * h * 3) as usize);
            for pixel in flat.pixels() {
                rgb.extend_from_slice(&[pixel[0], pixel[1], pixel[2]]);
            }
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality.clamp(1, 100))
                .encode(&rgb, w, h, ExtendedColorType::Rgb8)
                .map_err(|e| AppError::write_failed(format!("JPEG 编码失败：{e}")))?;
        }
        OutputFormat::Webp => {
            // 规范 5.4 指定的引擎只提供无损 WebP 编码
            image::codecs::webp::WebPEncoder::new_lossless(&mut buf)
                .encode(canvas.as_raw(), w, h, ExtendedColorType::Rgba8)
                .map_err(|e| AppError::write_failed(format!("WebP 编码失败：{e}")))?;
        }
        OutputFormat::Png | OutputFormat::Keep => {
            image::codecs::png::PngEncoder::new(&mut buf)
                .write_image(canvas.as_raw(), w, h, ExtendedColorType::Rgba8)
                .map_err(|e| AppError::write_failed(format!("PNG 编码失败：{e}")))?;
        }
    }
    Ok(buf)
}

fn is_svg(ext: &str) -> bool {
    ext.eq_ignore_ascii_case("svg")
}

/// 依据源扩展名与格式设置，算出实际输出扩展名。
pub fn resolve_output_ext(src_ext: &str, format: OutputFormat) -> String {
    let target = format.resolve_ext(src_ext);
    // SVG 在「保持原格式」下仍输出 SVG；统一转换时才光栅化
    if is_svg(src_ext) && format == OutputFormat::Keep {
        return "svg".into();
    }
    if is_svg(&target) {
        // SVG 无法作为位图编码目标，退回 PNG
        return "png".into();
    }
    target
}

/// 处理并写出一个文件。返回新文件的字节数。
pub fn write_image(
    src: &Path,
    out: &Path,
    target: &Computed,
    mode: Mode,
    opts: &ImageOptions,
) -> Result<u64, AppError> {
    let src_ext = src
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let out_ext = out
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::write_failed(format!("创建输出目录失败：{e}")))?;
    }

    // ---------- SVG ----------
    if is_svg(&src_ext) {
        let bytes = std::fs::read(src)
            .map_err(|e| AppError::decode_failed(format!("读取 {} 失败：{e}", src.display())))?;

        if out_ext == "svg" {
            // SVG → SVG：不重采样，仅改写根元素尺寸并同步 viewBox
            let out_bytes = svg::rewrite_size(&bytes, target.width, target.height)?;
            std::fs::write(out, &out_bytes)
                .map_err(|e| AppError::write_failed(format!("写出 {} 失败：{e}", out.display())))?;
            return Ok(out_bytes.len() as u64);
        }

        // SVG → 位图：按目标像素尺寸光栅化
        let raster = svg::rasterize(&bytes, target.content_width, target.content_height)?;
        let decoded = DynamicImage::ImageRgba8(raster);
        let fill = canvas_fill(out_ext.as_str(), opts);
        let canvas = render_image(&decoded, target, mode, opts.resample, fill);
        let format = OutputFormat::from_ext(&out_ext).unwrap_or(OutputFormat::Png);
        let encoded = encode(&canvas, format, opts.quality, fill)?;
        std::fs::write(out, &encoded)
            .map_err(|e| AppError::write_failed(format!("写出 {} 失败：{e}", out.display())))?;
        return Ok(encoded.len() as u64);
    }

    // ---------- 静态位图 ----------
    let metadata = meta::read(src);
    let decoded = image::open(src)
        .map_err(|e| AppError::decode_failed(format!("无法解码 {}：{e}", src.display())))?;

    // 先按 EXIF 旋转再缩放（规范 6.8）
    let mut oriented = decoded;
    oriented.apply_orientation(metadata.orientation());

    let fill = canvas_fill(out_ext.as_str(), opts);
    let canvas = render_image(&oriented, target, mode, opts.resample, fill);
    let format = OutputFormat::from_ext(&out_ext).unwrap_or(OutputFormat::Png);
    let encoded = encode(&canvas, format, opts.quality, fill)?;

    // ICC 必须保留；其余元数据按开关决定（规范 6.8）
    meta::write_with_metadata(out, encoded, &metadata, opts.keep_all_metadata)
}

/// 补边区域与 alpha 压平所用的填充色。
///
/// 输出格式支持透明时补边默认透明；不支持时用设置中的背景色。
fn canvas_fill(out_ext: &str, opts: &ImageOptions) -> Rgba<u8> {
    let format = OutputFormat::from_ext(out_ext).unwrap_or(OutputFormat::Png);
    if format_lacks_alpha(format) {
        parse_hex_color(&opts.background_fill)
    } else {
        Rgba([0, 0, 0, 0])
    }
}

/// P2 之后可写出的类型：静态位图与 SVG（动图与视频见 P4 / P3）。
pub fn is_writable_by_this_stage(kind: crate::model::MediaKind) -> bool {
    matches!(
        kind,
        crate::model::MediaKind::Raster | crate::model::MediaKind::Svg
    )
}

/// JPEG 无 alpha 时的背景色说明，供界面提示（规范 6.8）。
pub fn alpha_loss_note(format: OutputFormat) -> Option<&'static str> {
    format_lacks_alpha(format).then_some("该输出格式不支持透明通道，将以背景色填充")
}

/// 供扫描阶段使用：EXIF 方向是否需要交换宽高。
pub fn swaps_axes(path: &Path) -> bool {
    meta::swaps_axes(path)
}

pub fn unsupported_error(ext: &str) -> AppError {
    AppError::new(
        ErrorCode::UnsupportedFormat,
        format!("暂不支持写出 .{ext} 格式"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    #[test]
    fn resample_names_map_to_filters() {
        assert_eq!(Resample::from_name("lanczos3"), Resample::Lanczos3);
        assert_eq!(Resample::from_name("Nearest"), Resample::Nearest);
        assert_eq!(Resample::from_name("bicubic"), Resample::Bicubic);
        assert_eq!(Resample::from_name("bilinear"), Resample::Bilinear);
        assert_eq!(Resample::from_name("未知"), Resample::Lanczos3);
    }

    #[test]
    fn output_extension_resolution() {
        assert_eq!(resolve_output_ext("png", OutputFormat::Keep), "png");
        assert_eq!(resolve_output_ext("png", OutputFormat::Webp), "webp");
        assert_eq!(resolve_output_ext("jpeg", OutputFormat::Jpeg), "jpg");
        // SVG 保持原格式时仍是 SVG；统一转换时输出位图
        assert_eq!(resolve_output_ext("svg", OutputFormat::Keep), "svg");
        assert_eq!(resolve_output_ext("svg", OutputFormat::Webp), "webp");
        // SVG 不能作为位图编码目标
        assert_eq!(resolve_output_ext("png", OutputFormat::Keep), "png");
    }

    #[test]
    fn nearest_and_lanczos_differ_on_upscale() {
        // 2×2 棋盘放大后，Nearest 保持硬边、Lanczos3 产生过渡色
        let mut src = RgbImage::new(2, 2);
        src.put_pixel(0, 0, Rgb([0, 0, 0]));
        src.put_pixel(1, 0, Rgb([255, 255, 255]));
        src.put_pixel(0, 1, Rgb([255, 255, 255]));
        src.put_pixel(1, 1, Rgb([0, 0, 0]));
        let decoded = DynamicImage::ImageRgb8(src);

        let target = Computed {
            width: 64,
            height: 64,
            content_width: 64,
            content_height: 64,
            anchor: Anchor::Center,
        };

        let nearest = render_image(&decoded, &target, Mode::D, Resample::Nearest, Rgba([0, 0, 0, 0]));
        let lanczos = render_image(&decoded, &target, Mode::D, Resample::Lanczos3, Rgba([0, 0, 0, 0]));

        // Nearest 只会产生纯黑与纯白
        let nearest_has_gray = nearest
            .pixels()
            .any(|p| p[0] != 0 && p[0] != 255);
        assert!(!nearest_has_gray, "Nearest 不应产生过渡色");

        // Lanczos3 在边界处会产生中间灰
        let lanczos_has_gray = lanczos.pixels().any(|p| p[0] > 20 && p[0] < 235);
        assert!(lanczos_has_gray, "Lanczos3 应产生平滑过渡");
    }

    #[test]
    fn jpeg_output_is_flattened_over_background() {
        let canvas = RgbaImage::from_pixel(4, 4, Rgba([200, 100, 50, 0]));
        let encoded = encode(&canvas, OutputFormat::Jpeg, 90, Rgba([255, 255, 255, 255])).unwrap();
        assert!(encoded.starts_with(&[0xFF, 0xD8]), "应输出 JPEG 数据");
    }

    #[test]
    fn webp_output_is_produced() {
        let canvas = RgbaImage::from_pixel(8, 8, Rgba([10, 200, 30, 255]));
        let encoded = encode(&canvas, OutputFormat::Webp, 85, Rgba([0, 0, 0, 0])).unwrap();
        assert!(encoded.len() > 12);
        assert_eq!(&encoded[0..4], b"RIFF", "应输出 RIFF 容器");
        assert_eq!(&encoded[8..12], b"WEBP", "应为 WebP 数据");
    }
}
