//! 处理报告（规范 11.3 / 6.13）。
//!
//! 字段与顺序固定：源文件路径、输出文件路径、类型、所属分组、原宽度、原高度、
//! 目标宽度、目标高度、原体积、新体积、使用方式、状态、错误原因、耗时ms。
//! 状态取值为「成功 / 未改动 / 已跳过 / 失败」四类。

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// CSV 表头，顺序即规范 11.3 规定的字段顺序。
pub const HEADERS: [&str; 14] = [
    "源文件路径",
    "输出文件路径",
    "类型",
    "所属分组",
    "原宽度",
    "原高度",
    "目标宽度",
    "目标高度",
    "原体积",
    "新体积",
    "使用方式",
    "状态",
    "错误原因",
    "耗时ms",
];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportRow {
    pub source_path: String,
    pub output_path: String,
    pub kind: String,
    pub group: String,
    pub original_width: String,
    pub original_height: String,
    pub target_width: String,
    pub target_height: String,
    pub original_size: String,
    pub new_size: String,
    pub mode: String,
    pub status: String,
    pub error: String,
    pub elapsed_ms: String,
}

impl ReportRow {
    fn cells(&self) -> [&str; 14] {
        [
            &self.source_path,
            &self.output_path,
            &self.kind,
            &self.group,
            &self.original_width,
            &self.original_height,
            &self.target_width,
            &self.target_height,
            &self.original_size,
            &self.new_size,
            &self.mode,
            &self.status,
            &self.error,
            &self.elapsed_ms,
        ]
    }
}

/// 转义 CSV 字段：含逗号、引号或换行时用双引号包起来并转义内部引号。
fn escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

pub fn to_csv(rows: &[ReportRow]) -> String {
    let mut out = String::new();
    // 带 BOM，Excel 打开中文表头才不乱码
    out.push('\u{feff}');
    out.push_str(&HEADERS.join(","));
    out.push_str("\r\n");
    for row in rows {
        let line: Vec<String> = row.cells().iter().map(|c| escape(c)).collect();
        out.push_str(&line.join(","));
        out.push_str("\r\n");
    }
    out
}

pub fn to_json(rows: &[ReportRow]) -> Result<String, AppError> {
    serde_json::to_string_pretty(rows)
        .map_err(|e| AppError::write_failed(format!("报告序列化失败：{e}")))
}

/// 报告文件名：`meowfit-report-YYYYMMDD-HHmmss.{csv,json}`（规范 11.3）。
pub fn report_file_name(extension: &str) -> String {
    let now = chrono::Local::now();
    format!("meowfit-report-{}.{}", now.format("%Y%m%d-%H%M%S"), extension)
}

/// 把报告写到输出目录，返回两个文件的路径。
pub fn write_reports(
    output_dir: &Path,
    rows: &[ReportRow],
) -> Result<(String, String), AppError> {
    let csv_path = output_dir.join(report_file_name("csv"));
    let json_path = output_dir.join(report_file_name("json"));

    std::fs::write(&csv_path, to_csv(rows))
        .map_err(|e| AppError::write_failed(format!("写出 CSV 报告失败：{e}")))?;
    std::fs::write(
        &json_path,
        to_json(rows)?,
    )
    .map_err(|e| AppError::write_failed(format!("写出 JSON 报告失败：{e}")))?;

    Ok((
        csv_path.to_string_lossy().into_owned(),
        json_path.to_string_lossy().into_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> ReportRow {
        ReportRow {
            source_path: r"D:\素材\a.png".into(),
            output_path: r"D:\output\a.png".into(),
            kind: "静态图片".into(),
            group: "icon".into(),
            original_width: "64".into(),
            original_height: "64".into(),
            target_width: "128".into(),
            target_height: "128".into(),
            original_size: "1024".into(),
            new_size: "2048".into(),
            mode: "D".into(),
            status: "成功".into(),
            error: String::new(),
            elapsed_ms: "12".into(),
        }
    }

    #[test]
    fn csv_has_the_specified_header_order() {
        let csv = to_csv(&[row()]);
        let header = csv.lines().next().unwrap().trim_start_matches('\u{feff}');
        assert_eq!(header, HEADERS.join(","));
        // 字段顺序与规范 11.3 一致
        assert_eq!(
            header,
            "源文件路径,输出文件路径,类型,所属分组,原宽度,原高度,目标宽度,目标高度,原体积,新体积,使用方式,状态,错误原因,耗时ms"
        );
    }

    #[test]
    fn csv_escapes_separators_and_quotes() {
        let mut r = row();
        r.group = "含,逗号".into();
        r.error = "他说\"失败\"了".into();
        let csv = to_csv(&[r]);
        let line = csv.lines().nth(1).unwrap();
        assert!(line.contains("\"含,逗号\""), "含逗号的字段应被引号包住");
        assert!(line.contains("\"他说\"\"失败\"\"了\""), "内部引号应被转义");
    }

    #[test]
    fn json_uses_camel_case_keys() {
        let json = to_json(&[row()]).unwrap();
        assert!(json.contains("\"sourcePath\""));
        assert!(json.contains("\"originalWidth\""));
        assert!(json.contains("\"elapsedMs\""));
        // 能被解析回来
        let back: Vec<ReportRow> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].status, "成功");
    }

    #[test]
    fn report_file_name_follows_the_spec_pattern() {
        let name = report_file_name("csv");
        assert!(name.starts_with("meowfit-report-"), "实际：{name}");
        assert!(name.ends_with(".csv"));
        // YYYYMMDD-HHmmss 共 15 个字符
        let stamp = name
            .trim_start_matches("meowfit-report-")
            .trim_end_matches(".csv");
        assert_eq!(stamp.len(), 15, "实际：{stamp}");
    }
}
