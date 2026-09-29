use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::model::Grouping;

/// 前缀匹配：取第一个分隔符（`_`、`-`、空格、`.`）之前的部分。
/// 分隔符集合与规范 10.3.1 一致，`\d` 按 JS 语义限定为 `[0-9]`。
static PREFIX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([^_\-\s.]+)").expect("前缀正则为常量"));

/// 剥离结尾数字。
static TRAILING_DIGITS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[0-9]+$").expect("尾数字正则为常量"));

pub const NO_PREFIX: &str = "<无前缀>";
pub const NO_EXTENSION: &str = "<无扩展名>";
pub const ROOT_FOLDER: &str = "<根目录>";

/// 去掉最后一级扩展名。`my.icon.png` → `my.icon`
fn stem_of(filename: &str) -> &str {
    Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename)
}

/// 按命名前缀分组（规范 10.3.1）。
pub fn prefix_group(filename: &str) -> String {
    let stem = stem_of(filename);
    let prefix = match PREFIX_RE.find(stem) {
        Some(m) => m.as_str(),
        None => stem,
    };
    let stripped = TRAILING_DIGITS_RE.replace(prefix, "");
    if stripped.is_empty() {
        NO_PREFIX.to_string()
    } else {
        stripped.to_lowercase()
    }
}

/// 按扩展名分组（规范 10.3.2）。扩展名判定大小写不敏感。
pub fn extension_group(filename: &str) -> String {
    match Path::new(filename).extension().and_then(|e| e.to_str()) {
        Some(ext) if !ext.is_empty() => ext.to_lowercase(),
        _ => NO_EXTENSION.to_string(),
    }
}

/// 按所在子文件夹分组（规范 10.3.3）。`relative_parent` 为相对输入根目录的父路径。
pub fn folder_group(relative_parent: &str) -> String {
    let normalized = relative_parent.replace('\\', "/");
    let trimmed = normalized.trim_matches('/');
    if trimmed.is_empty() {
        ROOT_FOLDER.to_string()
    } else {
        trimmed.to_string()
    }
}

/// 参与分组的最小文件信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupableFile {
    pub id: String,
    pub name: String,
    /// 相对输入根目录的父路径，分隔符 `/`；位于根目录下时为空字符串
    #[serde(default)]
    pub relative_parent: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub name: String,
    pub file_ids: Vec<String>,
}

/// 按指定方式分组（规范 10.3）。三种方式单选，不叠加。
pub fn group_files(files: &[GroupableFile], grouping: Grouping) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();

    for file in files {
        let name = match grouping {
            Grouping::Prefix => prefix_group(&file.name),
            Grouping::Extension => extension_group(&file.name),
            Grouping::Folder => folder_group(&file.relative_parent),
        };

        match groups.iter_mut().find(|g| g.name == name) {
            Some(g) => g.file_ids.push(file.id.clone()),
            None => groups.push(Group {
                name,
                file_ids: vec![file.id.clone()],
            }),
        }
    }

    // 组名排序，保证界面顺序稳定
    groups.sort_by(|a, b| a.name.cmp(&b.name));
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 规范 10.3.1 分组用例表，12 行全部覆盖。
    #[test]
    fn spec_table_10_3_1() {
        for (input, expected) in [
            ("icon_01.png", "icon"),
            ("icon-2.png", "icon"),
            ("icon 3.png", "icon"),
            ("icon4.png", "icon"),
            ("Icon_05.PNG", "icon"),
            ("bg001.jpg", "bg"),
            ("bg_001.jpg", "bg"),
            ("logo.png", "logo"),
            ("001.jpg", NO_PREFIX),
            ("头像_01.png", "头像"),
            ("my.icon.png", "my"),
            ("icon.gif", "icon"),
        ] {
            assert_eq!(prefix_group(input), expected, "输入：{input}");
        }
    }

    #[test]
    fn prefix_edges() {
        // 空文件名
        assert_eq!(prefix_group(""), NO_PREFIX);
        // 纯数字
        assert_eq!(prefix_group("123.png"), NO_PREFIX);
        assert_eq!(prefix_group("0001"), NO_PREFIX);
        // 多分隔符
        assert_eq!(prefix_group("a__b.png"), "a");
        assert_eq!(prefix_group("a-b-c.png"), "a");
        assert_eq!(prefix_group("a.b.c.png"), "a");
        // 中文名
        assert_eq!(prefix_group("素材0001.png"), "素材");
        assert_eq!(prefix_group("背景图.png"), "背景图");
        // 大小写不敏感
        assert_eq!(prefix_group("ICON_01.PNG"), "icon");
        // 无扩展名
        assert_eq!(prefix_group("icon"), "icon");
    }

    #[test]
    fn prefix_keyword_is_not_treated_as_extension() {
        // my.icon.png 的 stem 为 my.icon，不是 my
        assert_eq!(stem_of("my.icon.png"), "my.icon");
        assert_eq!(prefix_group("my.icon.png"), "my");
    }

    #[test]
    fn extension_grouping() {
        assert_eq!(extension_group("a.PNG"), "png");
        assert_eq!(extension_group("a.JpEg"), "jpeg");
        assert_eq!(extension_group("noext"), NO_EXTENSION);
        assert_eq!(extension_group("a."), NO_EXTENSION);
        assert_eq!(extension_group(".gitignore"), NO_EXTENSION);
        assert_eq!(extension_group("中文.图片"), "图片");
    }

    #[test]
    fn folder_grouping() {
        assert_eq!(folder_group(""), ROOT_FOLDER);
        assert_eq!(folder_group("/"), ROOT_FOLDER);
        assert_eq!(folder_group("ui"), "ui");
        assert_eq!(folder_group("ui/icons"), "ui/icons");
        assert_eq!(folder_group(r"ui\icons"), "ui/icons");
        assert_eq!(folder_group("/ui/icons/"), "ui/icons");
    }

    #[test]
    fn grouping_does_not_split_on_extension() {
        // icon.gif 与 icon_01.png 必须同组（分组不区分扩展名）
        let files = vec![
            GroupableFile {
                id: "1".into(),
                name: "icon.gif".into(),
                relative_parent: String::new(),
            },
            GroupableFile {
                id: "2".into(),
                name: "icon_01.png".into(),
                relative_parent: String::new(),
            },
        ];
        let groups = group_files(&files, Grouping::Prefix);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "icon");
        assert_eq!(groups[0].file_ids, vec!["1", "2"]);
    }

    #[test]
    fn group_files_by_extension_and_folder() {
        let files = vec![
            GroupableFile {
                id: "1".into(),
                name: "a.png".into(),
                relative_parent: "ui".into(),
            },
            GroupableFile {
                id: "2".into(),
                name: "b.jpg".into(),
                relative_parent: "ui".into(),
            },
            GroupableFile {
                id: "3".into(),
                name: "c.png".into(),
                relative_parent: String::new(),
            },
        ];

        let by_ext = group_files(&files, Grouping::Extension);
        assert_eq!(by_ext.len(), 2);
        assert_eq!(by_ext[0].name, "jpg");
        assert_eq!(by_ext[1].name, "png");
        assert_eq!(by_ext[1].file_ids, vec!["1", "3"]);

        let by_folder = group_files(&files, Grouping::Folder);
        assert_eq!(by_folder.len(), 2);
        assert_eq!(by_folder[0].name, ROOT_FOLDER);
        assert_eq!(by_folder[1].name, "ui");
        assert_eq!(by_folder[1].file_ids, vec!["1", "2"]);
    }

    #[test]
    fn empty_input_yields_no_groups() {
        assert!(group_files(&[], Grouping::Prefix).is_empty());
    }
}
