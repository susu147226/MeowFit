use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// 缩放方式编号（规范 6.2）。
/// F「仅放大 / 仅缩小」不是独立模式，而是 A–D 的附加开关，见 `Setting::only_down` / `only_up`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    A,
    B,
    C,
    D,
    E,
    G,
}

/// 倍数锚点缩放（模式 G）的锚点位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Anchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl Default for Anchor {
    fn default() -> Self {
        Self::Center
    }
}

/// 自动分组方式，三种单选（规范 6.4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Grouping {
    Prefix,
    Extension,
    Folder,
}

impl Default for Grouping {
    fn default() -> Self {
        Self::Prefix
    }
}

/// 素材大类，决定处理引擎（规范 5.4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaKind {
    /// 静态位图：image crate
    Raster,
    /// SVG：resvg crate
    Svg,
    /// GIF / 动态 WebP / APNG：FFmpeg
    Animated,
    /// 视频：FFmpeg
    Video,
}

impl MediaKind {
    pub fn is_video(self) -> bool {
        matches!(self, Self::Video)
    }
}

/// 三态参数的「已设置」内容：null / 跟随整体 / 具体设置（规范 10.2）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "setting")]
pub enum GroupSetting {
    /// 显式跟随整体
    FollowGlobal,
    /// 本层生效
    Explicit(Setting),
}

/// 一层缩放参数。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Setting {
    pub mode: Mode,
    /// 模式 A / G：缩放倍率
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
    /// 模式 B / C / D：目标宽
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    /// 模式 B / C / D：目标高
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    /// 模式 B：「不补边」开关，打开后输出内容实际尺寸
    #[serde(default)]
    pub no_pad: bool,
    /// 模式 E：长边上限
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<f64>,
    /// F 附加开关：仅缩小
    #[serde(default)]
    pub only_down: bool,
    /// F 附加开关：仅放大
    #[serde(default)]
    pub only_up: bool,
    /// 模式 G：锚点
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<Anchor>,
}

impl Setting {
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            scale: None,
            width: None,
            height: None,
            no_pad: false,
            limit: None,
            only_down: false,
            only_up: false,
            anchor: None,
        }
    }

    pub fn scale(mode: Mode, scale: f64) -> Self {
        Self {
            scale: Some(scale),
            ..Self::new(mode)
        }
    }

    pub fn width_height(mode: Mode, width: f64, height: f64) -> Self {
        Self {
            width: Some(width),
            height: Some(height),
            ..Self::new(mode)
        }
    }
}

/// 解析出的目标尺寸结果（规范 10.1）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Computed {
    /// 输出画布宽
    pub width: u32,
    /// 输出画布高
    pub height: u32,
    /// 实际内容宽（模式 B 补边时小于画布宽，其余等于画布宽）
    pub content_width: u32,
    /// 实际内容高
    pub content_height: u32,
    /// 内容在画布中的锚点
    pub anchor: Anchor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    /// 不做任何改动：不缩放、不转码、不写出、不复制
    Unchanged,
    /// 需要缩放并写出
    Resize,
}

/// 处理报告中的状态枚举（规范 11.3），取值固定为四类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Success,
    Unchanged,
    Skipped,
    Failed,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Success => "成功",
            Self::Unchanged => "未改动",
            Self::Skipped => "已跳过",
            Self::Failed => "失败",
        }
    }
}

/// 生效设置的来源，用于界面上一眼看出「改成什么尺寸、为什么」。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SettingSource {
    /// 单文件设置
    File,
    /// 分组设置
    Group,
    /// 整体设置
    Global,
    /// 三层都未设置 —— 保持原样
    None,
}

pub const SOURCE_FILE: &str = "单文件";
pub const SOURCE_GROUP: &str = "分组";
pub const SOURCE_GLOBAL: &str = "整体";
pub const SOURCE_NONE: &str = "不变";

impl SettingSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::File => SOURCE_FILE,
            Self::Group => SOURCE_GROUP,
            Self::Global => SOURCE_GLOBAL,
            Self::None => SOURCE_NONE,
        }
    }
}

/// 构建计划时每个文件的输入。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanFileInput {
    pub id: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub is_video: bool,
    /// 该文件当前所属分组名
    pub group: String,
    /// 单文件设置；null 表示未设置
    #[serde(default)]
    pub setting: Option<Setting>,
}

/// 构建计划时每个分组的输入。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanGroupInput {
    pub name: String,
    /// null 表示未设置；否则为「跟随整体」或具体设置
    #[serde(default)]
    pub setting: Option<GroupSetting>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanRequest {
    pub files: Vec<PlanFileInput>,
    #[serde(default)]
    pub groups: Vec<PlanGroupInput>,
    /// 整体设置；null 表示未设置
    #[serde(default)]
    pub global: Option<Setting>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanEntry {
    pub id: String,
    pub name: String,
    pub group: String,
    pub source: SettingSource,
    pub action: Action,
    pub original_width: u32,
    pub original_height: u32,
    /// 目标尺寸；`action == Unchanged` 时为 null
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<Computed>,
    /// 该条目被使用的方式编号（A–G）；未改动时为 null
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<Mode>,
    /// 生效的完整设置；增量处理据此计算指纹（规范 11.4）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setting: Option<Setting>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<AppError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub entries: Vec<PlanEntry>,
    /// 全部条目均可执行（无校验错误）
    pub ok: bool,
}
