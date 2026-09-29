pub mod algo;
pub mod animate;
pub mod commands;
pub mod config;
pub mod error;
pub mod exec;
pub mod ffmpeg;
pub mod imaging;
pub mod incremental;
pub mod logging;
pub mod meta;
pub mod model;
pub mod presets;
pub mod scan;
pub mod svg;

use std::path::PathBuf;

pub fn run() {
    // 配置目录在启动时解析一次，决定 Portable / 安装版 / 只读降级（规范 4.1）
    let location = config::resolve_config_dir();
    let state = commands::AppState {
        config_dir: PathBuf::from(&location.dir),
        config_mode: location.mode,
        cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::scan_folder,
            commands::group_entries,
            commands::preview_plan,
            commands::execute_plan,
            commands::load_settings,
            commands::save_settings,
            commands::touch_recent_folder,
            commands::read_background_image,
            commands::ffmpeg_self_check,
            commands::cancel_execute,
            commands::disk_free_space,
            commands::load_presets,
            commands::add_preset,
            commands::rename_preset,
            commands::remove_preset,
            commands::export_presets,
            commands::import_presets,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
