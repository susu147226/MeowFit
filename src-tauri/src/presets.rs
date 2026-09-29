//! 预设（规范 6.12）。
//!
//! - 内置预设 8 个，可隐藏但不可删除；
//! - 用户可新增、重命名、删除自定义预设；
//! - 以 JSON 保存（`config/presets.json`），便于用户自行编辑。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::{Mode, Setting};

pub const PRESETS_FILE: &str = "presets.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    /// 内置为 `builtin-<slug>`，用户预设为 `user-<uuid>`
    pub id: String,
    pub name: String,
    pub builtin: bool,
    /// 内置预设可隐藏但不可删除
    #[serde(default)]
    pub hidden: bool,
    pub setting: Setting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetStore {
    pub version: u32,
    #[serde(default)]
    pub presets: Vec<Preset>,
}

/// 内置预设的目标宽高（规范 6.12）。
/// 都按「等比适配进目标框」处理（模式 B），与预设的用途一致。
const BUILTIN_SIZES: [(&str, &str, u32, u32); 8] = [
    ("builtin-1080p", "1080P", 1920, 1080),
    ("builtin-720p", "720P", 1280, 720),
    ("builtin-4k", "4K", 3840, 2160),
    ("builtin-1080x1920", "1080×1920", 1080, 1920),
    ("builtin-1080x1080", "1080×1080", 1080, 1080),
    ("builtin-1080x2640", "1080×2640", 1080, 2640),
    ("builtin-2640x2640", "2640×2640", 2640, 2640),
    ("builtin-640x640", "640×640", 640, 640),
];

pub fn builtin_presets() -> Vec<Preset> {
    BUILTIN_SIZES
        .iter()
        .map(|(id, name, width, height)| Preset {
            id: (*id).to_string(),
            name: (*name).to_string(),
            builtin: true,
            hidden: false,
            setting: Setting::width_height(Mode::B, *width as f64, *height as f64),
        })
        .collect()
}

impl Default for PresetStore {
    fn default() -> Self {
        Self {
            version: 1,
            presets: builtin_presets(),
        }
    }
}

impl PresetStore {
    /// 保证内置预设始终齐全：缺的补上，已隐藏的状态保留。
    pub fn normalize(mut self) -> Self {
        for builtin in builtin_presets() {
            match self.presets.iter_mut().find(|p| p.id == builtin.id) {
                Some(existing) => {
                    // 内置预设的名称与设置以程序内置为准，隐藏状态尊重用户的
                    let hidden = existing.hidden;
                    let id = existing.id.clone();
                    *existing = Preset { hidden, id, ..builtin };
                }
                None => self.presets.push(builtin),
            }
        }
        // 内置在前，用户预设在后
        self.presets.sort_by_key(|p| !p.builtin);
        self
    }

    pub fn user_presets(&self) -> impl Iterator<Item = &Preset> {
        self.presets.iter().filter(|p| !p.builtin)
    }
}

pub fn presets_path(config_dir: &Path) -> PathBuf {
    config_dir.join(PRESETS_FILE)
}

/// 读取预设；文件缺失或损坏时退回内置预设。
pub fn load(config_dir: &Path) -> PresetStore {
    fs::read_to_string(presets_path(config_dir))
        .ok()
        .and_then(|text| serde_json::from_str::<PresetStore>(&text).ok())
        .unwrap_or_default()
        .normalize()
}

pub fn save(config_dir: &Path, store: &PresetStore) -> Result<(), String> {
    fs::create_dir_all(config_dir).map_err(|e| format!("创建配置目录失败：{e}"))?;
    let text =
        serde_json::to_string_pretty(store).map_err(|e| format!("序列化预设失败：{e}"))?;
    fs::write(presets_path(config_dir), text).map_err(|e| format!("写入预设失败：{e}"))
}

/// 新增用户预设。
pub fn add_user(store: &mut PresetStore, name: &str, setting: Setting, seed: u64) -> Preset {
    let preset = Preset {
        id: format!("user-{seed:016x}"),
        name: name.to_string(),
        builtin: false,
        hidden: false,
        setting,
    };
    store.presets.push(preset.clone());
    preset
}

/// 删除：内置预设只能隐藏，不能删除。
pub fn remove_or_hide(store: &mut PresetStore, id: &str) -> Result<(), String> {
    let Some(index) = store.presets.iter().position(|p| p.id == id) else {
        return Err("找不到该预设".into());
    };
    if store.presets[index].builtin {
        store.presets[index].hidden = true;
        Ok(())
    } else {
        store.presets.remove(index);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_config(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "meowfit-preset-{tag}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn builtin_presets_match_the_spec_list() {
        let store = PresetStore::default();
        let names: Vec<&str> = store.presets.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "1080P",
                "720P",
                "4K",
                "1080×1920",
                "1080×1080",
                "1080×2640",
                "2640×2640",
                "640×640"
            ]
        );
        assert!(store.presets.iter().all(|p| p.builtin));
        assert!(store.presets.iter().all(|p| p.id.starts_with("builtin-")));
        // 1080P 的尺寸要对得上
        let p1080 = store.presets.iter().find(|p| p.id == "builtin-1080p").unwrap();
        assert_eq!(p1080.setting.width, Some(1920.0));
        assert_eq!(p1080.setting.height, Some(1080.0));
    }

    #[test]
    fn user_preset_round_trips_across_restarts() {
        let config = temp_config("round");
        let mut store = PresetStore::default();
        add_user(&mut store, "我的 800×600", Setting::width_height(Mode::B, 800.0, 600.0), 42);
        save(&config, &store).unwrap();

        // 重新读取（等价于重启程序）
        let back = load(&config);
        let custom = back.user_presets().find(|p| p.name == "我的 800×600");
        assert!(custom.is_some(), "自定义预设应保留");
        assert_eq!(custom.unwrap().setting.width, Some(800.0));
        assert_eq!(custom.unwrap().setting.height, Some(600.0));
        // 内置的也还在
        assert_eq!(back.presets.iter().filter(|p| p.builtin).count(), 8);

        let _ = fs::remove_dir_all(&config);
    }

    #[test]
    fn builtin_cannot_be_deleted_but_can_be_hidden() {
        let mut store = PresetStore::default();
        remove_or_hide(&mut store, "builtin-720p").unwrap();

        // 没有被删掉，只是隐藏了
        let p = store.presets.iter().find(|p| p.id == "builtin-720p").unwrap();
        assert!(p.hidden);
        assert_eq!(store.presets.iter().filter(|p| p.builtin).count(), 8);

        // 重新加载后仍然存在（隐藏状态保留）
        assert!(store.normalize().presets.iter().any(|p| p.id == "builtin-720p"));
    }

    #[test]
    fn user_preset_can_be_deleted() {
        let mut store = PresetStore::default();
        let preset = add_user(&mut store, "临时", Setting::scale(Mode::A, 2.0), 7);
        assert_eq!(store.user_presets().count(), 1);
        remove_or_hide(&mut store, &preset.id).unwrap();
        assert_eq!(store.user_presets().count(), 0, "用户预设应被真正删除");
    }

    #[test]
    fn missing_or_corrupt_file_falls_back_to_builtins() {
        let config = temp_config("corrupt");
        assert_eq!(load(&config).presets.len(), 8, "没有文件时应给出内置预设");

        fs::write(presets_path(&config), "{ 坏 json").unwrap();
        assert_eq!(load(&config).presets.len(), 8, "文件损坏时也应退回内置预设");

        let _ = fs::remove_dir_all(&config);
    }

    #[test]
    fn deleted_builtin_is_restored_on_load() {
        let config = temp_config("restore");
        // 手工造一个只剩一个内置预设的文件
        let partial = PresetStore {
            version: 1,
            presets: builtin_presets().into_iter().take(1).collect(),
        };
        save(&config, &partial).unwrap();
        let back = load(&config);
        assert_eq!(back.presets.len(), 8, "缺失的内置预设应被补回");

        let _ = fs::remove_dir_all(&config);
    }
}
