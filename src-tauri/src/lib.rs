pub mod algo;
pub mod commands;
pub mod config;
pub mod error;
pub mod exec;
pub mod imaging;
pub mod meta;
pub mod model;
pub mod scan;
pub mod svg;

use std::path::PathBuf;

pub fn run() {
    // 配置目录在启动时解析一次，决定 Portable / 安装版 / 只读降级（规范 4.1）
    let location = config::resolve_config_dir();
    let state = commands::AppState {
        config_dir: PathBuf::from(&location.dir),
        config_mode: location.mode,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
