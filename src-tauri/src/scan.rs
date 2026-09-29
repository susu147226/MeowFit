//! 素材扫描（规范 5.5、6.1）。
//!
//! 扫描必须是**只读**的：本模块不创建、不移动、不修改任何素材文件，
//! 唯一例外是在素材文件夹的父目录下解析输出目录时**不**创建它。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::model::MediaKind;
use crate::svg;

/// 静态位图：由 Rust `image` crate 处理
const RASTER_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "bmp", "tif", "tiff", "ico", "avif", "heic", "heif",
];
/// 需要 FFmpeg 解码的静态位图（P1 尚未接入）
const FFMPEG_RASTER_EXTS: &[&str] = &["avif", "heic", "heif"];
const SVG_EXTS: &[&str] = &["svg"];
const ANIMATED_EXTS: &[&str] = &["gif", "apng"];
const VIDEO_EXTS: &[&str] = &[
    "mp4", "mov", "mkv", "avi", "webm", "m4v", "flv", "wmv", "mpeg", "mpg", "ts",
];

/// 系统文件，一律忽略（规范 5.5）
const SYSTEM_FILES: &[&str] = &["thumbs.db", ".ds_store", "desktop.ini"];

/// 按扩展名判定素材大类，大小写不敏感（规范 5.5、5.4）。
pub fn classify(ext: &str) -> Option<MediaKind> {
    let e = ext.to_ascii_lowercase();
    if SVG_EXTS.contains(&e.as_str()) {
        Some(MediaKind::Svg)
    } else if RASTER_EXTS.contains(&e.as_str()) {
        Some(MediaKind::Raster)
    } else if ANIMATED_EXTS.contains(&e.as_str()) {
        Some(MediaKind::Animated)
    } else if VIDEO_EXTS.contains(&e.as_str()) {
        Some(MediaKind::Video)
    } else {
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanOptions {
    /// 递归子文件夹；`false` 表示「仅当前层」
    #[serde(default = "default_true")]
    pub recursive: bool,
    /// 包含规则通配符；非空时只保留命中的文件
    #[serde(default)]
    pub include: Vec<String>,
    /// 排除规则通配符，如 `*_thumb.*`、`@2x`
    #[serde(default)]
    pub exclude: Vec<String>,
}

fn default_true() -> bool {
    true
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            recursive: true,
            include: Vec::new(),
            exclude: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannedFile {
    /// 相对路径，作稳定 id
    pub id: String,
    pub path: String,
    /// 相对输入根目录，分隔符统一为 `/`
    pub relative_path: String,
    /// 相对输入根目录的父路径；位于根目录下时为空字符串
    pub relative_parent: String,
    pub name: String,
    pub ext: String,
    /// `None` 表示扩展名不在支持列表内
    pub kind: Option<MediaKind>,
    pub size: u64,
    pub mtime_ms: i64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// 仅对 SVG 有意义：根元素是否声明了 width / height（规范 10.5）。
    /// 未声明时界面须在「直接填像素」与「按 DPI 换算」之间提供切换。
    pub svg_declared: bool,
    /// 视频附加信息（时长、帧率、旋转、HDR 等），由 ffprobe 得到
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video: Option<crate::ffmpeg::VideoInfo>,
    /// 动图附加信息（帧数、循环次数），仅动图有
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animation: Option<crate::animate::AnimationInfo>,
    /// 已跳过的原因；`None` 表示该文件可参与处理
    pub skip_reason: Option<String>,
}

impl ScannedFile {
    pub fn is_processable(&self) -> bool {
        self.skip_reason.is_none() && self.width.is_some() && self.height.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub root: String,
    /// 本次扫描解析出的输出目录（默认「源文件夹同级 output/」，规范 6.5）
    pub output_dir: String,
    pub files: Vec<ScannedFile>,
}

fn build_globset(patterns: &[String]) -> Result<GlobSet, String> {
    let mut builder = GlobSetBuilder::new();
    for p in patterns {
        let p = p.trim();
        if p.is_empty() {
            continue;
        }
        let glob = Glob::new(p).map_err(|e| format!("通配符 `{p}` 无法解析：{e}"))?;
        builder.add(glob);
    }
    builder.build().map_err(|e| format!("通配符构建失败：{e}"))
}

/// 默认输出目录：输入文件夹的**同级**目录下建立 `output/`（规范 6.5）。
pub fn default_output_dir(input_dir: &Path) -> PathBuf {
    match input_dir.parent() {
        Some(parent) => parent.join("output"),
        None => input_dir.join("output"),
    }
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// 该目录名是否要整体剪枝。
fn is_pruned_dir_name(name: &str) -> bool {
    // 隐藏目录与程序自身的运行期目录（规范 5.5）
    name.starts_with('.') || name.eq_ignore_ascii_case("config") || name.eq_ignore_ascii_case("logs")
}

fn to_relative(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// 扫描文件夹（规范 5.5、6.1）。
pub fn scan_folder(root: &Path, options: &ScanOptions) -> Result<ScanResult, String> {
    if !root.is_dir() {
        return Err(format!("不是有效的文件夹：{}", root.display()));
    }

    let output_dir = default_output_dir(root);
    let include = build_globset(&options.include)?;
    let exclude = build_globset(&options.exclude)?;
    // FFmpeg 路径解析一次即可，供本次扫描中所有视频探测复用
    let ffmpeg_paths = crate::ffmpeg::resolve_paths().ok();
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));

    let mut files: Vec<ScannedFile> = Vec::new();

    let mut walker = WalkDir::new(root).follow_links(false);
    if !options.recursive {
        walker = walker.max_depth(1);
    }

    for entry in walker.into_iter().filter_entry(|e| {
        if e.depth() == 0 {
            return true; // 输入根目录自身永不剪枝
        }
        if !e.file_type().is_dir() {
            return true;
        }
        let name = e.file_name().to_string_lossy().to_string();
        if is_pruned_dir_name(&name) {
            return false;
        }
        let path = e.path();
        // 输出目录自身：既排除本次解析出的输出目录，也排除根目录下直属的 `output/`。
        // 后者是「上次以父目录为输入跑过一次」留下的产物，不排除就会被当成新素材再处理一遍。
        if e.depth() == 1 && name.eq_ignore_ascii_case("output") {
            return false;
        }
        if same_dir(path, &output_dir) {
            return false;
        }
        if let Some(exe) = &exe_dir {
            if same_dir(path, exe) {
                return false;
            }
        }
        true
    }) {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue, // 无权限等：跳过，不中断整体扫描
        };
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // 隐藏文件、以 . 开头的文件、系统文件
        if name.starts_with('.') || SYSTEM_FILES.contains(&name.to_ascii_lowercase().as_str()) {
            continue;
        }

        let relative_path = to_relative(path, root);
        let relative_parent = Path::new(&relative_path)
            .parent()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();

        // 包含 / 排除通配符：同时匹配完整相对路径与文件名
        let candidates = [relative_path.as_str(), name.as_str()];
        if !options.include.is_empty() && !candidates.iter().any(|c| include.is_match(c)) {
            continue;
        }
        if candidates.iter().any(|c| exclude.is_match(c)) {
            continue;
        }

        let metadata = match fs::metadata(path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let mtime_ms = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        let ext = Path::new(&name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        // .webp 与 .png 既可能是静态也可能是动态，必须看容器内容再定性，
        // 否则动态 WebP 会被当成静态图处理、只写出第一帧
        let kind = match classify(&ext) {
            Some(MediaKind::Raster) if crate::animate::is_animated(path, &ext) => {
                Some(MediaKind::Animated)
            }
            other => other,
        };

        let probed = probe(path, &ext, kind, ffmpeg_paths.as_ref());
        let (width, height, skip_reason, svg_declared, video, animation) = (
            probed.width,
            probed.height,
            probed.skip_reason,
            probed.svg_declared,
            probed.video,
            probed.animation,
        );

        files.push(ScannedFile {
            id: relative_path.clone(),
            path: path.to_string_lossy().into_owned(),
            relative_path,
            relative_parent,
            name,
            ext,
            kind,
            size: metadata.len(),
            mtime_ms,
            width,
            height,
            svg_declared,
            video,
            animation,
            skip_reason,
        });
    }

    // 目录遍历顺序在各平台不一致，固定按相对路径排序以保证结果稳定
    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    Ok(ScanResult {
        root: root.to_string_lossy().into_owned(),
        output_dir: output_dir.to_string_lossy().into_owned(),
        files,
    })
}

/// 探测结果：尺寸、SVG 是否声明尺寸、视频附加信息、以及「已跳过」原因。
struct Probe {
    width: Option<u32>,
    height: Option<u32>,
    svg_declared: bool,
    video: Option<crate::ffmpeg::VideoInfo>,
    animation: Option<crate::animate::AnimationInfo>,
    skip_reason: Option<String>,
}

/// 探测尺寸并给出「已跳过」原因。
fn probe(
    path: &Path,
    ext: &str,
    kind: Option<MediaKind>,
    ffmpeg_paths: Option<&crate::ffmpeg::FfmpegPaths>,
) -> Probe {
    let skipped = |reason: String| Probe {
        width: None,
        height: None,
        svg_declared: false,
        video: None,
        animation: None,
        skip_reason: Some(reason),
    };

    match kind {
        None => skipped(format!("不支持的格式：.{ext}")),
        Some(MediaKind::Raster) => {
            // AVIF / HEIC / HEIF 由 FFmpeg 处理（规范 5.4：image crate 的 avif 特性未启用时回退 FFmpeg）
            if FFMPEG_RASTER_EXTS.contains(&ext) {
                let Some(paths) = ffmpeg_paths else {
                    return skipped(format!("缺少 FFmpeg，无法探测 .{ext}（需 ffmpeg.exe 与 ffprobe.exe）"));
                };
                return match crate::ffmpeg::probe(paths, path)
                    .ok()
                    .and_then(|result| result.to_video_info())
                {
                    Some(info) => Probe {
                        width: Some(info.width),
                        height: Some(info.height),
                        svg_declared: false,
                        video: None,
                        animation: None,
                        skip_reason: None,
                    },
                    None => skipped(format!("无法从 .{ext} 中解析出图像尺寸")),
                };
            }
            match image::image_dimensions(path) {
                Ok((mut w, mut h)) => {
                    // 带 EXIF 方向的竖拍图：实际画面宽高与原始像素宽高相反（规范 6.8）
                    if matches!(ext, "jpg" | "jpeg") && crate::imaging::swaps_axes(path) {
                        std::mem::swap(&mut w, &mut h);
                    }
                    Probe {
                        width: Some(w),
                        height: Some(h),
                        svg_declared: false,
                        video: None,
                        animation: None,
                        skip_reason: None,
                    }
                }
                Err(_) => skipped("无法解码该文件".to_string()),
            }
        }
        Some(MediaKind::Svg) => match fs::read_to_string(path).ok().and_then(|text| svg::base_size(&text)) {
            Some(size) => Probe {
                width: Some(size.width.round() as u32),
                height: Some(size.height.round() as u32),
                svg_declared: size.declared,
                video: None,
                animation: None,
                skip_reason: None,
            },
            None => skipped("无法从 SVG 中解析出尺寸".to_string()),
        },
        Some(MediaKind::Animated) => {
            let Some(paths) = ffmpeg_paths else {
                return skipped("缺少 FFmpeg，无法处理动图（需 ffmpeg.exe 与 ffprobe.exe）".to_string());
            };
            match crate::animate::probe(paths, path, ext) {
                Ok(info) => Probe {
                    width: Some(info.width),
                    height: Some(info.height),
                    svg_declared: false,
                    video: None,
                    animation: Some(info),
                    skip_reason: None,
                },
                Err(err) => skipped(err.message),
            }
        }
        Some(MediaKind::Video) => {
            // 视频尺寸必须由 ffprobe 得到（规范 12.4）；找不到 FFmpeg 时如实跳过
            let Some(paths) = ffmpeg_paths else {
                return skipped(
                    "缺少 FFmpeg，无法探测视频（需 ffmpeg.exe 与 ffprobe.exe）".to_string(),
                );
            };
            match crate::ffmpeg::probe(paths, path) {
                Ok(result) => match result.to_video_info() {
                    Some(info) => Probe {
                        width: Some(info.width),
                        height: Some(info.height),
                        svg_declared: false,
                        video: Some(info),
                        animation: None,
                        skip_reason: None,
                    },
                    None => skipped("该文件里没有视频流".to_string()),
                },
                Err(err) => skipped(err.message),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    fn write_png(path: &Path, w: u32, h: u32) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let img = RgbImage::from_pixel(w, h, Rgb([200, 120, 40]));
        img.save(path).unwrap();
    }

    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("meowfit-scan-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn classify_is_case_insensitive() {
        assert_eq!(classify("PNG"), Some(MediaKind::Raster));
        assert_eq!(classify("JpG"), Some(MediaKind::Raster));
        assert_eq!(classify("SVG"), Some(MediaKind::Svg));
        assert_eq!(classify("GIF"), Some(MediaKind::Animated));
        assert_eq!(classify("MP4"), Some(MediaKind::Video));
        assert_eq!(classify("TS"), Some(MediaKind::Video));
        assert_eq!(classify("xyz"), None);
        assert_eq!(classify("zip"), None);
    }

    #[test]
    fn scan_reads_dimensions_and_skips_unsupported() {
        let root = temp_root("basic");
        write_png(&root.join("icon_01.png"), 64, 32);
        write_png(&root.join("icon_02.png"), 16, 16);

        let result = scan_folder(&root, &ScanOptions::default()).unwrap();
        assert_eq!(result.files.len(), 2);
        let f = result.files.iter().find(|f| f.name == "icon_01.png").unwrap();
        assert_eq!((f.width, f.height), (Some(64), Some(32)));
        assert_eq!(f.kind, Some(MediaKind::Raster));
        assert!(f.is_processable());
        assert_eq!(f.relative_parent, "");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_skips_hidden_and_system_files() {
        let root = temp_root("hidden");
        write_png(&root.join("a.png"), 8, 8);
        write_png(&root.join(".hidden.png"), 8, 8);
        fs::write(root.join("Thumbs.db"), b"x").unwrap();
        write_png(&root.join(".secret/b.png"), 8, 8);

        let result = scan_folder(&root, &ScanOptions::default()).unwrap();
        let names: Vec<&str> = result.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["a.png"]);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_records_skip_reason_for_unsupported_files() {
        let root = temp_root("unsupported");
        write_png(&root.join("ok.png"), 8, 8);
        fs::write(root.join("data.xyz"), b"x").unwrap();
        fs::write(root.join("archive.zip"), b"x").unwrap();

        // 场景 8：不支持格式被跳过并列入「已跳过」，不中断其他文件
        let result = scan_folder(&root, &ScanOptions::default()).unwrap();
        assert_eq!(result.files.len(), 3);
        let skipped: Vec<&str> = result
            .files
            .iter()
            .filter(|f| !f.is_processable())
            .map(|f| f.name.as_str())
            .collect();
        assert_eq!(skipped, vec!["archive.zip", "data.xyz"]);
        assert!(result
            .files
            .iter()
            .find(|f| f.name == "ok.png")
            .unwrap()
            .is_processable());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn non_recursive_stops_at_current_level() {
        let root = temp_root("top");
        write_png(&root.join("top.png"), 8, 8);
        write_png(&root.join("sub/deep.png"), 8, 8);

        let all = scan_folder(&root, &ScanOptions::default()).unwrap();
        assert_eq!(all.files.len(), 2);
        let deep = all.files.iter().find(|f| f.name == "deep.png").unwrap();
        assert_eq!(deep.relative_parent, "sub");

        let top_only = scan_folder(
            &root,
            &ScanOptions {
                recursive: false,
                ..ScanOptions::default()
            },
        )
        .unwrap();
        assert_eq!(top_only.files.len(), 1);
        assert_eq!(top_only.files[0].name, "top.png");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn include_and_exclude_globs() {
        let root = temp_root("glob");
        write_png(&root.join("icon_01.png"), 8, 8);
        write_png(&root.join("icon_01_thumb.png"), 8, 8);
        write_png(&root.join("logo@2x.png"), 8, 8);

        let result = scan_folder(
            &root,
            &ScanOptions {
                include: vec![],
                exclude: vec!["*_thumb.*".into(), "*@2x*".into()],
                ..ScanOptions::default()
            },
        )
        .unwrap();
        let names: Vec<&str> = result.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["icon_01.png"]);

        let only_icon = scan_folder(
            &root,
            &ScanOptions {
                include: vec!["icon_*".into()],
                exclude: vec![],
                ..ScanOptions::default()
            },
        )
        .unwrap();
        let names: Vec<&str> = only_icon.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["icon_01.png", "icon_01_thumb.png"]);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_excludes_previous_output_dir() {
        // 规范 6.5：以父目录为输入时，上次跑出来的 `output/` 不能被当成新素材再处理一遍
        let root = temp_root("outdir");
        write_png(&root.join("a.png"), 8, 8);
        write_png(&root.join("output/old.png"), 8, 8);
        write_png(&root.join("sub/output/nested.png"), 8, 8);

        let result = scan_folder(&root, &ScanOptions::default()).unwrap();
        let names: Vec<&str> = result.files.iter().map(|f| f.name.as_str()).collect();
        // 根目录直属的 output/ 被排除；深层同名目录不受影响（那不是输出落点）
        assert_eq!(names, vec!["a.png", "nested.png"]);
        assert_eq!(result.output_dir, default_output_dir(&root).to_string_lossy());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn svg_dimensions_from_width_height_and_viewbox() {
        let root = temp_root("svg");
        fs::write(
            root.join("a.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80"><rect/></svg>"#,
        )
        .unwrap();
        fs::write(
            root.join("b.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 300 150"><rect/></svg>"#,
        )
        .unwrap();

        let result = scan_folder(&root, &ScanOptions::default()).unwrap();
        let a = result.files.iter().find(|f| f.name == "a.svg").unwrap();
        let b = result.files.iter().find(|f| f.name == "b.svg").unwrap();
        assert_eq!((a.width, a.height), (Some(120), Some(80)));
        assert_eq!((b.width, b.height), (Some(300), Some(150)));
        assert_eq!(a.kind, Some(MediaKind::Svg));

        let _ = fs::remove_dir_all(&root);
    }
}
