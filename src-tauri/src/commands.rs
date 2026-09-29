//! 暴露给前端的 Tauri 命令。
//!
//! 尺寸计算、分组、计划构建全部落在 [`crate::algo`]，前端预览与执行写出共用同一套实现
//! （规范 20.4：不得有第二份实现）。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::algo::grouping::{group_files, Group, GroupableFile};
use crate::algo::plan::build_plan;
use crate::config::{self, ConfigMode, Settings};
use crate::exec::{self, ExecOptions, ExecReport, SourceRef};
use crate::model::{Grouping, Plan, PlanRequest};
use crate::scan::{self, ScanOptions, ScanResult};

pub const APP_NAME: &str = "喵尺 MeowFit";
pub const COPYRIGHT: &str = "© 2026 云舒眠眠";
pub const LICENSE_NAME: &str = "喵尺 MeowFit 许可证（私有，禁止再分发）";

pub struct AppState {
    pub config_dir: PathBuf,
    pub config_mode: ConfigMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub identifier: String,
    pub copyright: String,
    pub license_name: String,
    pub config_dir: String,
    pub config_mode: ConfigMode,
    /// 只读介质下降级到临时目录时为 false，界面须提示「配置未能持久化」
    pub config_persistent: bool,
}

#[tauri::command]
pub fn get_app_info(app: tauri::AppHandle, state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        name: APP_NAME.to_string(),
        version: app.package_info().version.to_string(),
        identifier: app.config().identifier.clone(),
        copyright: COPYRIGHT.to_string(),
        license_name: LICENSE_NAME.to_string(),
        config_dir: state.config_dir.to_string_lossy().into_owned(),
        config_mode: state.config_mode,
        config_persistent: state.config_mode != ConfigMode::Temporary,
    }
}

/// 扫描素材文件夹。扫描本身是只读的，不会创建或改动任何素材文件。
#[tauri::command]
pub fn scan_folder(root: String, options: Option<ScanOptions>) -> Result<ScanResult, String> {
    let options = options.unwrap_or_default();
    scan::scan_folder(Path::new(&root), &options)
}

/// 按指定方式自动分组（三种方式单选）。
#[tauri::command]
pub fn group_entries(files: Vec<GroupableFile>, grouping: Grouping) -> Vec<Group> {
    group_files(&files, grouping)
}

/// 构建任务计划：前端预览与执行前校验共用。
#[tauri::command]
pub fn preview_plan(request: PlanRequest) -> Plan {
    build_plan(&request)
}

/// 执行计划并写出。
///
/// 调用方必须先确认 `plan.ok == true`；此处再做一次兜底，防止界面漏判后写出半套结果
/// （规范 13.1：`VALIDATION_FAILED` 时不得执行任何写出）。
#[tauri::command]
pub fn execute_plan(
    request: PlanRequest,
    sources: Vec<SourceRef>,
    options: Option<ExecOptions>,
    root: String,
) -> Result<ExecReport, String> {
    let plan = build_plan(&request);
    if !plan.ok {
        let failed: Vec<&str> = plan
            .entries
            .iter()
            .filter(|e| e.error.is_some())
            .map(|e| e.name.as_str())
            .collect();
        return Err(format!(
            "存在校验失败的素材，已阻止执行：{}",
            failed.join("、")
        ));
    }
    exec::execute(&plan, &sources, Path::new(&root), &options.unwrap_or_default())
        .map_err(|e| format!("{} {}", e.code.as_str(), e.message))
}

#[tauri::command]
pub fn load_settings(state: State<'_, AppState>) -> Settings {
    config::load_settings(&state.config_dir)
}

#[tauri::command]
pub fn save_settings(
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<Settings, String> {
    config::save_settings(&state.config_dir, &settings)?;
    Ok(settings)
}

/// 把文件夹记入「最近使用的文件夹」（最多 10 条）并持久化。
#[tauri::command]
pub fn touch_recent_folder(state: State<'_, AppState>, folder: String) -> Result<Settings, String> {
    let mut settings = config::load_settings(&state.config_dir);
    settings.touch_recent_folder(&folder);
    config::save_settings(&state.config_dir, &settings)?;
    Ok(settings)
}

/// 背景图大小上限；超过则拒绝，避免把巨大的数据塞进界面。
pub const MAX_BACKGROUND_BYTES: u64 = 8 * 1024 * 1024;

/// 读取本地背景图并编码为 data URL。
///
/// 仅用于**界面外观**（规范 6.14）：图片不进入素材处理流程、不写入任何日志、
/// 也不会被上传——本程序不发起任何网络请求。
#[tauri::command]
pub fn read_background_image(path: String) -> Result<String, String> {
    use base64::Engine as _;

    let file = Path::new(&path);
    let metadata = std::fs::metadata(file).map_err(|e| format!("无法读取背景图：{e}"))?;
    if metadata.len() > MAX_BACKGROUND_BYTES {
        return Err(format!(
            "背景图过大（上限 {} MB），请换一张小一些的图片",
            MAX_BACKGROUND_BYTES / 1024 / 1024
        ));
    }

    let mime = match file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        _ => return Err("仅支持 PNG / JPG / WebP / GIF / BMP / AVIF 作为背景图".into()),
    };

    let bytes = std::fs::read(file).map_err(|e| format!("无法读取背景图：{e}"))?;
    Ok(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    ))
}
