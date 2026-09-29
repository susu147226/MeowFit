//! 暴露给前端的 Tauri 命令。
//!
//! 尺寸计算、分组、计划构建全部落在 [`crate::algo`]，前端预览与执行写出共用同一套实现
//! （规范 20.4：不得有第二份实现）。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::Emitter;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::algo::grouping::{group_files, Group, GroupableFile};
use crate::algo::plan::build_plan;
use crate::config::{self, ConfigMode, Settings};
use crate::exec::{self, ExecOptions, ExecReport, SourceRef};
use crate::model::{Grouping, Plan, PlanRequest, Setting};
use crate::presets::{self, PresetStore};
use crate::scan::{self, ScanOptions, ScanResult};

pub const APP_NAME: &str = "喵尺 MeowFit";
pub const COPYRIGHT: &str = "© 2026 云舒眠眠";
pub const LICENSE_NAME: &str = "喵尺 MeowFit 许可证（私有，禁止再分发）";

pub struct AppState {
    pub config_dir: PathBuf,
    pub config_mode: ConfigMode,
    /// 执行中可置位以取消（规范 6.7）
    pub cancel: Arc<AtomicBool>,
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

/// 执行进度事件（规范 6.7）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub done: usize,
    pub total: usize,
    pub current: String,
}

/// 取消当前执行：已完成的保留，未开始的停止（规范 6.7）。
#[tauri::command]
pub fn cancel_execute(state: State<'_, AppState>) {
    state.cancel.store(true, Ordering::Relaxed);
}

/// 执行计划并写出。
///
/// 调用方必须先确认 `plan.ok == true`；此处再做一次兜底，防止界面漏判后写出半套结果
/// （规范 13.1：`VALIDATION_FAILED` 时不得执行任何写出）。
#[tauri::command]
pub fn execute_plan(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
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

    state.cancel.store(false, Ordering::Relaxed);

    let handle = app.clone();
    let mut progress = |done: usize, total: usize, current: &str| {
        let _ = handle.emit(
            "meowfit://progress",
            ProgressEvent {
                done,
                total,
                current: current.to_string(),
            },
        );
    };

    run_plan(
        &plan,
        &sources,
        &root,
        options,
        Some(state.config_dir.to_string_lossy().into_owned()),
        Some(&mut progress),
        Some(&state.cancel),
    )
}

/// 执行的实际逻辑。
///
/// 抽出来是为了能在没有 Tauri 运行时的测试里直接调用——命令层只是包了一层
/// 事件发射与状态读取。
pub fn run_plan(
    plan: &Plan,
    sources: &[SourceRef],
    root: &str,
    options: Option<ExecOptions>,
    config_dir: Option<String>,
    progress: Option<&mut dyn FnMut(usize, usize, &str)>,
    cancel: Option<&AtomicBool>,
) -> Result<ExecReport, String> {
    // 兜底再查一次：校验失败时不得写出任何文件（规范 13.1）
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

    // 配置目录用于写日志与增量索引
    let mut options = options.unwrap_or_default();
    options.config_dir = config_dir;

    exec::execute_with(
        plan,
        sources,
        Path::new(root),
        &options,
        progress,
        cancel,
    )
    .map_err(|e| format!("{} {}", e.code.as_str(), e.message))
}

/// 读取预设（内置预设始终齐全，规范 6.12）。
#[tauri::command]
pub fn load_presets(state: State<'_, AppState>) -> PresetStore {
    presets::load(&state.config_dir)
}

/// 新增一个用户预设。
#[tauri::command]
pub fn add_preset(
    state: State<'_, AppState>,
    name: String,
    setting: Setting,
) -> Result<PresetStore, String> {
    let mut store = presets::load(&state.config_dir);
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    presets::add_user(&mut store, &name, setting, seed);
    presets::save(&state.config_dir, &store)?;
    Ok(store)
}

/// 重命名预设。
#[tauri::command]
pub fn rename_preset(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<PresetStore, String> {
    let mut store = presets::load(&state.config_dir);
    match store.presets.iter_mut().find(|p| p.id == id) {
        Some(preset) => preset.name = name,
        None => return Err("找不到该预设".into()),
    }
    presets::save(&state.config_dir, &store)?;
    Ok(store)
}

/// 删除预设：内置预设只能隐藏，不能删除（规范 6.12）。
#[tauri::command]
pub fn remove_preset(state: State<'_, AppState>, id: String) -> Result<PresetStore, String> {
    let mut store = presets::load(&state.config_dir);
    presets::remove_or_hide(&mut store, &id)?;
    presets::save(&state.config_dir, &store)?;
    Ok(store)
}

/// 导出预设为 JSON 文本（自带校验，便于用户自行编辑与传递）。
#[tauri::command]
pub fn export_presets(state: State<'_, AppState>) -> Result<String, String> {
    serde_json::to_string_pretty(&presets::load(&state.config_dir))
        .map_err(|e| format!("导出失败：{e}"))
}

/// 从 JSON 文本导入预设，与现有预设合并（同 id 覆盖）。
#[tauri::command]
pub fn import_presets(state: State<'_, AppState>, json: String) -> Result<PresetStore, String> {
    let incoming: PresetStore =
        serde_json::from_str(&json).map_err(|e| format!("预设文件无法解析：{e}"))?;
    let mut store = presets::load(&state.config_dir);
    for preset in incoming.presets {
        match store.presets.iter_mut().find(|p| p.id == preset.id) {
            Some(existing) => *existing = preset,
            None => store.presets.push(preset),
        }
    }
    store = store.normalize();
    presets::save(&state.config_dir, &store)?;
    Ok(store)
}

/// 目标路径所在磁盘的可用空间（字节），供磁盘空间预检使用（规范 6.6）。
#[tauri::command]
pub fn disk_free_space(path: String) -> Result<u64, String> {
    let mut probe = PathBuf::from(&path);
    // 目录可能还不存在，往上找到最近一个存在的祖先再问
    while !probe.exists() {
        match probe.parent() {
            Some(parent) if parent != probe => probe = parent.to_path_buf(),
            _ => return Err(format!("无法确定可用空间：{path}")),
        }
    }
    fs2::available_space(&probe).map_err(|e| format!("读取磁盘可用空间失败：{e}"))
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

/// FFmpeg 可用性与构建自检（规范 12.5）。
///
/// 找不到二进制时如实返回 `ffmpegFound: false` 与缺失清单，不抛错——
/// 界面需要据此提示用户，而不是让整个程序不可用。
#[tauri::command]
pub fn ffmpeg_self_check() -> crate::ffmpeg::SelfCheckReport {
    match crate::ffmpeg::resolve_paths() {
        Ok(paths) => crate::ffmpeg::self_check(&paths),
        Err(err) => crate::ffmpeg::SelfCheckReport {
            ffmpeg_found: false,
            missing_encoders: crate::ffmpeg::REQUIRED_ENCODERS
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            missing_filters: crate::ffmpeg::REQUIRED_FILTERS
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            missing_hevc_decoder: true,
            summary: err.message,
        },
    }
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
