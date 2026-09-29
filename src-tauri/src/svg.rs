//! SVG 处理（规范 5.4 / 10.5）。
//!
//! 两条路径：
//! - **输出仍为 SVG**：不重采样，仅改写根元素 `width` / `height`，并同步 `viewBox`
//!   以保持画面内容不变。
//! - **输出为位图**：用 `resvg` 按**指定像素尺寸**光栅化，不做模糊的倍率处理。

use std::sync::Arc;

use resvg::{tiny_skia, usvg};

use crate::error::{AppError, ErrorCode};

/// SVG 的基准尺寸。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SvgSize {
    pub width: f64,
    pub height: f64,
    /// 根元素是否**声明**了 width / height（规范 10.5：未声明时由用户选择换算方式）
    pub declared: bool,
}

/// 从属性值中取出数值，忽略 `px` 等单位；百分比与非法值返回 `None`。
fn parse_length(raw: &str) -> Option<f64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.ends_with('%') {
        return None;
    }
    let numeric: String = trimmed
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '+')
        .collect();
    let value = numeric.parse::<f64>().ok()?;
    (value > 0.0).then_some(value)
}

/// 定位根 `<svg …>` 标签的字节区间（含 `<` 与 `>`）。
fn root_tag_range(src: &str) -> Option<(usize, usize)> {
    let start = src.find("<svg")?;
    let next = src[start + 4..].chars().next()?;
    // 排除 `<svgfoo>` / `<svg:svg` 这类同名前缀元素
    if !(next.is_whitespace() || next == '>' || next == '/') {
        return None;
    }
    let end = src[start..].find('>')? + start;
    Some((start, end))
}

/// 读取根元素上某个属性的原始值。
fn attr_value(tag: &str, name: &str) -> Option<String> {
    let bytes = tag.as_bytes();
    let needle = name.as_bytes();
    let mut i = 0usize;
    while let Some(pos) = find_from(bytes, needle, i) {
        // 属性名左侧必须是空白，避免匹配到 `stroke-width` 这类后缀
        let before_ok = pos == 0 || bytes[pos - 1].is_ascii_whitespace();
        let after = pos + needle.len();
        let mut j = after;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if before_ok && j < bytes.len() && bytes[j] == b'=' {
            let mut k = j + 1;
            while k < bytes.len() && bytes[k].is_ascii_whitespace() {
                k += 1;
            }
            let quote = *bytes.get(k)?;
            if quote == b'"' || quote == b'\'' {
                let value_start = k + 1;
                let value_end = value_start + bytes[value_start..].iter().position(|b| *b == quote)?;
                return Some(tag[value_start..value_end].to_string());
            }
        }
        i = after;
    }
    None
}

fn find_from(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from >= haystack.len() {
        return None;
    }
    haystack[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// 设置根元素属性：已存在则替换，不存在则插到标签结尾之前。
fn set_attr(tag: &str, name: &str, value: &str) -> String {
    let bytes = tag.as_bytes();
    let needle = name.as_bytes();
    let mut i = 0usize;
    while let Some(pos) = find_from(bytes, needle, i) {
        let before_ok = pos == 0 || bytes[pos - 1].is_ascii_whitespace();
        let after = pos + needle.len();
        let mut j = after;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if before_ok && j < bytes.len() && bytes[j] == b'=' {
            let mut k = j + 1;
            while k < bytes.len() && bytes[k].is_ascii_whitespace() {
                k += 1;
            }
            if let Some(quote) = bytes.get(k).copied().filter(|q| *q == b'"' || *q == b'\'') {
                let value_start = k + 1;
                let value_len = bytes[value_start..]
                    .iter()
                    .position(|b| *b == quote)
                    .unwrap_or(0);
                return format!(
                    "{}{}{}",
                    &tag[..value_start],
                    value,
                    &tag[value_start + value_len..]
                );
            }
        }
        i = after;
    }

    // 未找到则插入：`<svg foo="1">` → `<svg foo="1" name="value">`
    let insert_at = tag
        .rfind('>')
        .map(|pos| {
            // 自闭合标签 `<svg …/>` 要插在 `/` 之前
            if tag[..pos].trim_end().ends_with('/') {
                tag[..pos].trim_end().len() - 1
            } else {
                pos
            }
        })
        .unwrap_or(tag.len());
    format!("{} {}=\"{}\"{}", &tag[..insert_at], name, value, &tag[insert_at..])
}

/// 解析 SVG 基准尺寸（规范 10.5）。
pub fn base_size(src: &str) -> Option<SvgSize> {
    let (start, end) = root_tag_range(src)?;
    let tag = &src[start..=end];

    let declared_w = attr_value(tag, "width").as_deref().and_then(parse_length);
    let declared_h = attr_value(tag, "height").as_deref().and_then(parse_length);
    if let (Some(width), Some(height)) = (declared_w, declared_h) {
        return Some(SvgSize {
            width,
            height,
            declared: true,
        });
    }

    // 回退到 viewBox 的后两个数值
    let viewbox = attr_value(tag, "viewBox").or_else(|| attr_value(tag, "viewbox"))?;
    let nums: Vec<f64> = viewbox
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<f64>().ok())
        .collect();
    if nums.len() == 4 && nums[2] > 0.0 && nums[3] > 0.0 {
        return Some(SvgSize {
            width: nums[2],
            height: nums[3],
            declared: false,
        });
    }
    None
}

/// SVG → SVG：改写根元素尺寸并同步 viewBox（规范 10.5）。
pub fn rewrite_size(src: &[u8], width: u32, height: u32) -> Result<Vec<u8>, AppError> {
    let text = std::str::from_utf8(src).map_err(|_| {
        AppError::new(ErrorCode::DecodeFailed, "SVG 不是合法的 UTF-8 文本".to_string())
    })?;

    let (start, end) = root_tag_range(text)
        .ok_or_else(|| AppError::new(ErrorCode::DecodeFailed, "找不到 SVG 根元素".to_string()))?;

    let original = base_size(text).ok_or_else(|| {
        AppError::new(ErrorCode::DecodeFailed, "无法解析 SVG 的尺寸".to_string())
    })?;

    let mut tag = text[start..=end].to_string();
    tag = set_attr(&tag, "width", &width.to_string());
    tag = set_attr(&tag, "height", &height.to_string());

    // viewBox 取原值；原文件没有 viewBox 时由原 width / height 生成
    if attr_value(&tag, "viewBox").is_none() {
        let viewbox = format!("0 0 {} {}", original.width, original.height);
        tag = set_attr(&tag, "viewBox", &viewbox);
    }

    let mut out = String::with_capacity(text.len() + 32);
    out.push_str(&text[..start]);
    out.push_str(&tag);
    out.push_str(&text[end + 1..]);
    Ok(out.into_bytes())
}

/// SVG → 位图：按指定像素尺寸光栅化（规范 10.5）。
pub fn rasterize(src: &[u8], width: u32, height: u32) -> Result<image::RgbaImage, AppError> {
    let mut options = usvg::Options::default();
    let mut fontdb = fontdb::Database::new();
    fontdb.load_system_fonts();
    options.fontdb = Arc::new(fontdb);

    let tree = usvg::Tree::from_data(src, &options).map_err(|e| {
        AppError::decode_failed(format!("无法解析 SVG：{e}"))
    })?;

    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or_else(|| {
        AppError::decode_failed("无法为该尺寸创建 SVG 画布".to_string())
    })?;

    let size = tree.size();
    if size.width() <= 0.0 || size.height() <= 0.0 {
        return Err(AppError::decode_failed("SVG 的尺寸无效".to_string()));
    }
    let scale_x = width as f32 / size.width();
    let scale_y = height as f32 / size.height();

    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale_x, scale_y),
        &mut pixmap.as_mut(),
    );

    let mut out = image::RgbaImage::new(width, height);
    for (x, y, pixel) in out.enumerate_pixels_mut() {
        let src_px = pixmap.pixel(x, y).ok_or_else(|| {
            AppError::decode_failed("SVG 光栅化结果尺寸不符".to_string())
        })?;
        *pixel = image::Rgba([src_px.red(), src_px.green(), src_px.blue(), src_px.alpha()]);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DECLARED: &str =
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80"><rect width="120" height="80" fill="#c08040"/></svg>"##;
    const VIEWBOX_ONLY: &str =
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 300 150"><rect width="300" height="150" fill="#4080c0"/></svg>"##;

    #[test]
    fn reads_declared_size() {
        let size = base_size(DECLARED).unwrap();
        assert_eq!((size.width, size.height, size.declared), (120.0, 80.0, true));
    }

    #[test]
    fn reads_viewbox_when_size_absent() {
        let size = base_size(VIEWBOX_ONLY).unwrap();
        assert_eq!((size.width, size.height, size.declared), (300.0, 150.0, false));
    }

    #[test]
    fn attribute_lookup_ignores_suffixed_names() {
        let tag = r#"<svg stroke-width="4" width="10" height="20">"#;
        assert_eq!(attr_value(tag, "width").as_deref(), Some("10"));
        assert_eq!(attr_value(tag, "height").as_deref(), Some("20"));
        assert_eq!(attr_value(tag, "viewBox"), None);
    }

    #[test]
    fn rewrite_keeps_viewbox_and_updates_size() {
        let out = rewrite_size(DECLARED.as_bytes(), 240, 160).unwrap();
        let text = String::from_utf8(out).unwrap();
        let tag = &text[..text.find('>').unwrap() + 1];
        assert_eq!(attr_value(tag, "width").as_deref(), Some("240"));
        assert_eq!(attr_value(tag, "height").as_deref(), Some("160"));
        // viewBox 由原 width/height 生成，保证画面内容不变
        assert_eq!(attr_value(tag, "viewBox").as_deref(), Some("0 0 120 80"));
        // 其余内容原样保留
        assert!(text.contains(r##"fill="#c08040""##));
    }

    #[test]
    fn rewrite_syncs_existing_viewbox() {
        let out = rewrite_size(VIEWBOX_ONLY.as_bytes(), 600, 300).unwrap();
        let text = String::from_utf8(out).unwrap();
        let tag = &text[..text.find('>').unwrap() + 1];
        assert_eq!(attr_value(tag, "viewBox").as_deref(), Some("0 0 300 150"));
        assert_eq!(attr_value(tag, "width").as_deref(), Some("600"));
    }

    #[test]
    fn rasterizes_to_requested_pixel_size() {
        let img = rasterize(DECLARED.as_bytes(), 60, 40).unwrap();
        assert_eq!((img.width(), img.height()), (60, 40));
        // 图形铺满画布，中心应是不透明的内容色
        let center = img.get_pixel(30, 20);
        assert_eq!(center[3], 255);
        assert!(center[0] > center[2], "应渲染出填充色，实际 {center:?}");
    }
}
