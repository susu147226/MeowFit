//! 静态位图的缩放与写出。
//!
//! P1 阶段只提供一条基础写出路径：固定 Lanczos3 重采样、保持原格式、不做
//! EXIF 方向校正、不保留 / 剥离元数据。规范 6.8 的图片专项（重采样算法可选、
//! EXIF、ICC、元数据开关、格式转换、质量参数）与 10.5 的 SVG 处理属 P2 阶段。

use std::path::Path;

use image::imageops::FilterType;
use image::{DynamicImage, Rgb, Rgba, RgbaImage};

use crate::error::AppError;
use crate::model::{Anchor, Computed, Mode};

/// 不支持 alpha 的输出格式，须以背景色填充（规范 6.8）
fn format_lacks_alpha(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "jpg" | "jpeg" | "bmp" | "ico"
    )
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
fn paste_on_canvas(content: RgbaImage, w: u32, h: u32, anchor: Anchor, fill: Rgba<u8>) -> RgbaImage {
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

    image::imageops::overlay(&mut canvas, &content, x as i64, y as i64);
    canvas
}

/// 把带 alpha 的图像按背景色压平成不透明图像。
fn flatten(img: &RgbaImage, bg: Rgba<u8>) -> RgbaImage {
    let mut out = RgbaImage::from_pixel(img.width(), img.height(), bg);
    image::imageops::overlay(&mut out, img, 0, 0);
    out
}

/// 按计划算出的目标尺寸渲染源文件。
///
/// 尺寸计算不在此处重复：`target` 必须来自 [`crate::algo::size::compute_target`]。
pub fn render(
    src: &Path,
    target: &Computed,
    mode: Mode,
    fill: Rgba<u8>,
) -> Result<RgbaImage, AppError> {
    let img = image::open(src)
        .map_err(|e| AppError::decode_failed(format!("无法解码 {}：{e}", src.display())))?;

    let filter = FilterType::Lanczos3;
    let (tw, th) = (target.width, target.height);
    let (cw, ch) = (target.content_width, target.content_height);

    let rendered = match mode {
        // 填满裁剪：等比缩放至覆盖目标框后居中裁剪
        Mode::C => img.resize_to_fill(tw, th, filter).to_rgba8(),
        // 拉伸：强制改变比例
        Mode::D => img.resize_exact(tw, th, filter).to_rgba8(),
        _ => {
            let content = img.resize_exact(cw, ch, filter).to_rgba8();
            if tw == cw && th == ch {
                content
            } else {
                // 模式 B 默认输出目标框画布，等比内容居中，空余区域填充
                paste_on_canvas(content, tw, th, target.anchor, fill)
            }
        }
    };

    Ok(rendered)
}

/// 渲染并写出。返回新文件的字节数。
pub fn render_and_write(
    src: &Path,
    out: &Path,
    target: &Computed,
    mode: Mode,
    background_fill: &str,
) -> Result<u64, AppError> {
    let ext = out
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let bg = parse_hex_color(background_fill);
    // 补边区域的填充：输出格式支持透明时默认透明，不支持时用背景色（规范 10.1 / 6.8）
    let canvas_fill = if format_lacks_alpha(&ext) {
        bg
    } else {
        Rgba([0, 0, 0, 0])
    };

    let img = render(src, target, mode, canvas_fill)?;

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::write_failed(format!("创建输出目录失败：{e}")))?;
    }

    // 输出格式不支持 alpha 时以背景色填充（P1 固定用设置中的背景色，默认白）
    let image_to_save: DynamicImage = if format_lacks_alpha(&ext) {
        let flat = flatten(&img, bg);
        let mut rgb = image::RgbImage::new(flat.width(), flat.height());
        for (x, y, p) in flat.enumerate_pixels() {
            rgb.put_pixel(x, y, Rgb([p[0], p[1], p[2]]));
        }
        DynamicImage::ImageRgb8(rgb)
    } else {
        DynamicImage::ImageRgba8(img)
    };

    image_to_save
        .save(out)
        .map_err(|e| AppError::write_failed(format!("写出 {} 失败：{e}", out.display())))?;

    std::fs::metadata(out)
        .map(|m| m.len())
        .map_err(|e| AppError::write_failed(format!("读取输出文件信息失败：{e}")))
}

/// P1 阶段只写出静态位图；SVG 与动图、视频在后续阶段接入。
pub fn is_writable_by_this_stage(kind: crate::model::MediaKind) -> bool {
    matches!(kind, crate::model::MediaKind::Raster)
}
