//! 配置目录解析与持久化。
//!
//! 目录解析遵循规范 4.1 的三级回退：
//! 程序目录/config → %APPDATA%/MeowFit → 系统临时目录/MeowFit-<随机串>。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// 配置落点模式，对应规范第四节的两种形态与只读降级。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfigMode {
    /// 程序目录下 ./config —— Portable 绿色版
    Portable,
    /// %APPDATA%/MeowFit —— 安装版
    Installed,
    /// 只读环境下的降级落点，配置无法持久化
    Temporary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigLocation {
    pub dir: String,
    pub mode: ConfigMode,
}

/// 在目标目录下创建并删除一个临时文件，以探测可写性（规范 4.1 的判定方式）。
fn is_writable_dir(dir: &Path) -> bool {
    if fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".meowfit-write-probe");
    match fs::write(&probe, b"") {
        Ok(()) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

fn random_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}{:x}", nanos, std::process::id())
}

/// 按三级回退解析配置目录（规范 4.1）。
pub fn resolve_config_dir() -> ConfigLocation {
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
    {
        let candidate = exe_dir.join("config");
        if is_writable_dir(&candidate) {
            return ConfigLocation {
                dir: candidate.to_string_lossy().into_owned(),
                mode: ConfigMode::Portable,
            };
        }
    }

    if let Some(appdata) = std::env::var_os("APPDATA") {
        let candidate = PathBuf::from(appdata).join("MeowFit");
        if is_writable_dir(&candidate) {
            return ConfigLocation {
                dir: candidate.to_string_lossy().into_owned(),
                mode: ConfigMode::Installed,
            };
        }
    }

    let mut dir = std::env::temp_dir();
    dir.push(format!("MeowFit-{}", random_suffix()));
    let _ = fs::create_dir_all(&dir);
    ConfigLocation {
        dir: dir.to_string_lossy().into_owned(),
        mode: ConfigMode::Temporary,
    }
}

// ---------------------------------------------------------------------------
// settings.json —— 结构见规范 11.1
// ---------------------------------------------------------------------------

/// 界面外观（规范 6.14 / 第七节）。背景图仅服务界面外观，不进入素材处理流程。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeConfig {
    /// system | light | dark；默认跟随系统外观
    pub mode: String,
    /// 背景图绝对路径或 null（本地导入，不上传）
    pub background_image: Option<String>,
    /// 背景图透明度 0–100
    pub background_opacity: u32,
    /// 背景图导致对比度不足时自动加蒙层
    #[serde(default = "default_true")]
    pub auto_scrim: bool,
    /// 主题色 #RRGGBB
    #[serde(default = "default_accent")]
    pub accent: String,
    /// 界面密度：compact | standard | relaxed
    #[serde(default = "default_density")]
    pub density: String,
    /// 右侧栏宽度（px）
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: u32,
    /// 预览与执行列宽度（px）
    #[serde(default = "default_sidebar_width")]
    pub preview_width: u32,
}

fn default_true() -> bool {
    true
}

fn default_accent() -> String {
    "#A8763A".into()
}

fn default_density() -> String {
    "standard".into()
}

fn default_sidebar_width() -> u32 {
    350
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            mode: "system".into(),
            background_image: None,
            background_opacity: 100,
            auto_scrim: default_true(),
            accent: default_accent(),
            density: default_density(),
            sidebar_width: default_sidebar_width(),
            preview_width: default_sidebar_width(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputConfig {
    pub directory: String,
    pub overwrite_source: bool,
    pub keep_structure: bool,
    pub on_conflict: String,
    pub background_fill_color: String,
    /// `true` = 剥离缩略图等冗余元数据（规范 11.1 的默认值）
    pub strip_redundant_metadata: bool,
    /// 输出格式：keep | png | jpeg | webp（规范 6.8 的格式转换）
    #[serde(default = "default_output_format")]
    pub output_format: String,
}

fn default_output_format() -> String {
    "keep".into()
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            directory: "./output".into(),
            overwrite_source: false,
            keep_structure: true,
            on_conflict: "skip".into(),
            background_fill_color: "#FFFFFF".into(),
            strip_redundant_metadata: true,
            output_format: default_output_format(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessingConfig {
    pub concurrency: u32,
    pub resample: String,
    pub jpg_quality: u32,
    pub video_crf: u32,
    pub video_encoder: String,
    pub hardware_accel: bool,
    pub hdr_tonemap_to_sdr: bool,
    pub gif_colors: u32,
    pub gif_dither: bool,
    pub upscale_warn_threshold: f64,
    pub svg_dpi: u32,
    /// 未声明尺寸的 SVG 如何取得基准：pixel（直接填像素）| dpi（按 DPI 换算，规范 10.5）
    #[serde(default = "default_svg_size_mode")]
    pub svg_size_mode: String,
}

fn default_svg_size_mode() -> String {
    "pixel".into()
}

impl Default for ProcessingConfig {
    fn default() -> Self {
        Self {
            concurrency: 8,
            resample: "lanczos3".into(),
            jpg_quality: 85,
            video_crf: 23,
            video_encoder: "h264".into(),
            hardware_accel: false,
            hdr_tonemap_to_sdr: false,
            gif_colors: 256,
            gif_dither: false,
            upscale_warn_threshold: 4.0,
            svg_dpi: 96,
            svg_size_mode: default_svg_size_mode(),
        }
    }
}

/// 配置文件 `config/settings.json`（规范 11.1）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub version: u32,
    pub language: String,
    #[serde(default)]
    pub theme: ThemeConfig,
    #[serde(default)]
    pub output: OutputConfig,
    #[serde(default)]
    pub processing: ProcessingConfig,
    /// prefix | extension | folder
    pub grouping: String,
    #[serde(default)]
    pub recent_folders: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            language: "zh-CN".into(),
            theme: ThemeConfig::default(),
            output: OutputConfig::default(),
            processing: ProcessingConfig::default(),
            grouping: "prefix".into(),
            recent_folders: Vec::new(),
        }
    }
}

/// 最近使用的文件夹最多保留 10 条（规范 6.1）。
pub const MAX_RECENT_FOLDERS: usize = 10;

impl Settings {
    /// 把某个文件夹挪到最近列表首位，去重并截断到 10 条。
    pub fn touch_recent_folder(&mut self, folder: &str) {
        self.recent_folders.retain(|f| f != folder);
        self.recent_folders.insert(0, folder.to_string());
        self.recent_folders.truncate(MAX_RECENT_FOLDERS);
    }
}

fn settings_path(config_dir: &Path) -> PathBuf {
    config_dir.join("settings.json")
}

/// 读取配置。文件缺失或损坏时回退到默认值，不阻断启动。
pub fn load_settings(config_dir: &Path) -> Settings {
    let path = settings_path(config_dir);
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

pub fn save_settings(config_dir: &Path, settings: &Settings) -> Result<(), String> {
    fs::create_dir_all(config_dir).map_err(|e| format!("创建配置目录失败：{e}"))?;
    let text = serde_json::to_string_pretty(settings).map_err(|e| format!("序列化配置失败：{e}"))?;
    fs::write(settings_path(config_dir), text).map_err(|e| format!("写入配置失败：{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_dir_is_always_resolved_and_writable() {
        let loc = resolve_config_dir();
        assert!(Path::new(&loc.dir).is_dir(), "配置目录必须真实存在");
        // 任何一种模式都必须给出可写目录（只读环境下会落到临时目录）
        assert!(is_writable_dir(Path::new(&loc.dir)));
    }

    #[test]
    fn recent_folders_dedupe_and_truncate() {
        let mut s = Settings::default();
        for i in 0..15 {
            s.touch_recent_folder(&format!("D:/f{i}"));
        }
        assert_eq!(s.recent_folders.len(), MAX_RECENT_FOLDERS);
        assert_eq!(s.recent_folders[0], "D:/f14");

        // 已存在的项被移到首位而不是重复追加
        s.touch_recent_folder("D:/f10");
        assert_eq!(s.recent_folders.len(), MAX_RECENT_FOLDERS);
        assert_eq!(s.recent_folders[0], "D:/f10");
        assert_eq!(
            s.recent_folders.iter().filter(|f| *f == "D:/f10").count(),
            1
        );
    }

    #[test]
    fn settings_round_trip_matches_spec_shape() {
        let mut s = Settings::default();
        s.grouping = "folder".into();
        s.touch_recent_folder("D:/素材/图标");

        let text = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&text).unwrap();
        assert_eq!(back.version, 1);
        assert_eq!(back.language, "zh-CN");
        assert_eq!(back.grouping, "folder");
        assert_eq!(back.output.on_conflict, "skip");
        assert_eq!(back.output.overwrite_source, false);
        assert_eq!(back.processing.resample, "lanczos3");
        assert_eq!(back.processing.jpg_quality, 85);
        assert_eq!(back.recent_folders, vec!["D:/素材/图标"]);
    }

    #[test]
    fn corrupt_config_falls_back_to_default() {
        let dir = std::env::temp_dir().join(format!("meowfit-test-{}", random_suffix()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(settings_path(&dir), "{ not json").unwrap();
        let s = load_settings(&dir);
        assert_eq!(s.grouping, "prefix");
        let _ = fs::remove_dir_all(&dir);
    }
}
