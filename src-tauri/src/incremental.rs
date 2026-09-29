//! 增量处理索引（规范 6.13 / 11.4）。
//!
//! 记录上次处理结果，下次可选择「跳过未变化的素材」。
//! 判定必须包含源文件的**修改时间与体积**，避免误跳过同名已替换的文件。

use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::Computed;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexEntry {
    pub size: u64,
    pub mtime_ms: i64,
    pub target_width: u32,
    pub target_height: u32,
    pub setting_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    pub version: u32,
    #[serde(default)]
    pub entries: HashMap<String, IndexEntry>,
}

impl Default for Index {
    fn default() -> Self {
        Self {
            version: 1,
            entries: HashMap::new(),
        }
    }
}

/// 索引文件名（规范 11.4）
pub const INDEX_FILE: &str = "incremental-index.json";

pub fn index_path(config_dir: &Path) -> PathBuf {
    config_dir.join(INDEX_FILE)
}

/// 读取索引；缺失或损坏时退回空索引，不阻断执行。
pub fn load(config_dir: &Path) -> Index {
    fs::read_to_string(index_path(config_dir))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save(config_dir: &Path, index: &Index) -> Result<(), String> {
    fs::create_dir_all(config_dir).map_err(|e| format!("创建配置目录失败：{e}"))?;
    let text = serde_json::to_string_pretty(index).map_err(|e| format!("序列化索引失败：{e}"))?;
    fs::write(index_path(config_dir), text).map_err(|e| format!("写入索引失败：{e}"))
}

/// 由生效设置算出指纹。设置一变就重新处理，与是否同名无关。
pub fn setting_hash(setting_json: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    setting_json.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// 是否跳过该素材（规范 11.4：体积、修改时间、设置指纹三者全部一致才跳过）。
pub fn should_skip(
    index: &Index,
    source_path: &str,
    size: u64,
    mtime_ms: i64,
    hash: &str,
) -> bool {
    match index.entries.get(source_path) {
        Some(entry) => {
            entry.size == size && entry.mtime_ms == mtime_ms && entry.setting_hash == hash
        }
        None => false,
    }
}

/// 记录一次成功处理的结果。
pub fn record(
    index: &mut Index,
    source_path: &str,
    size: u64,
    mtime_ms: i64,
    target: Option<&Computed>,
    hash: &str,
) {
    index.entries.insert(
        source_path.to_string(),
        IndexEntry {
            size,
            mtime_ms,
            target_width: target.map(|t| t.width).unwrap_or(0),
            target_height: target.map(|t| t.height).unwrap_or(0),
            setting_hash: hash.to_string(),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_config(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "meowfit-inc-{tag}-{}-{:?}",
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
    fn skip_requires_size_mtime_and_hash_to_all_match() {
        let mut index = Index::default();
        record(&mut index, "a.png", 1000, 111, None, "hash1");

        assert!(should_skip(&index, "a.png", 1000, 111, "hash1"));
        // 体积变了 → 重新处理
        assert!(!should_skip(&index, "a.png", 2000, 111, "hash1"));
        // 修改时间变了 → 重新处理（同名但被替换过的文件靠这个兜住）
        assert!(!should_skip(&index, "a.png", 1000, 222, "hash1"));
        // 设置变了 → 重新处理
        assert!(!should_skip(&index, "a.png", 1000, 111, "hash2"));
        // 没记录过的文件 → 不跳过
        assert!(!should_skip(&index, "b.png", 1000, 111, "hash1"));
    }

    #[test]
    fn setting_hash_is_stable_and_distinguishes_inputs() {
        assert_eq!(setting_hash("{\"mode\":\"A\",\"scale\":2}"), setting_hash("{\"mode\":\"A\",\"scale\":2}"));
        assert_ne!(setting_hash("{\"mode\":\"A\",\"scale\":2}"), setting_hash("{\"mode\":\"A\",\"scale\":3}"));
    }

    #[test]
    fn index_round_trips_and_survives_corruption() {
        let dir = temp_config("round");
        let mut index = Index::default();
        record(&mut index, "a.png", 1000, 111, None, "h");
        save(&dir, &index).unwrap();

        let back = load(&dir);
        assert_eq!(back.version, 1);
        assert!(back.entries.contains_key("a.png"));

        // 损坏的索引不应让程序起不来
        fs::write(index_path(&dir), "{ 坏掉的 json").unwrap();
        let fallback = load(&dir);
        assert!(fallback.entries.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn index_file_name_matches_spec() {
        assert_eq!(index_path(Path::new(r"D:\app\config")), PathBuf::from(r"D:\app\config\incremental-index.json"));
    }
}
