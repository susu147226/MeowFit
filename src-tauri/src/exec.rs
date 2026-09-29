//! 任务执行：解析输出目录、映射输出路径、冲突处理、逐文件写出。
//!
//! P1 阶段只覆盖使验收场景 1–8 可实测所需的基础路径：
//! 默认「源文件夹同级 output/」、保留相对目录结构、输出保持原名、默认不覆盖源文件。
//! 干跑、备份、磁盘预检、输出校验、报告导出、增量处理见 P5。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, ErrorCode};
use crate::imaging;
use crate::model::{Action, Plan, Status};
use crate::scan;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRef {
    pub id: String,
    /// 源文件绝对路径
    pub path: String,
    pub kind: crate::model::MediaKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecOptions {
    /// `None` 表示使用默认的「源文件夹同级 output/」
    #[serde(default)]
    pub output_dir: Option<String>,
    #[serde(default = "default_true")]
    pub keep_structure: bool,
    /// skip | overwrite | rename
    #[serde(default = "default_conflict")]
    pub on_conflict: String,
    #[serde(default = "default_fill")]
    pub background_fill_color: String,
}

fn default_true() -> bool {
    true
}
fn default_conflict() -> String {
    "skip".into()
}
fn default_fill() -> String {
    "#FFFFFF".into()
}

impl Default for ExecOptions {
    fn default() -> Self {
        Self {
            output_dir: None,
            keep_structure: true,
            on_conflict: default_conflict(),
            background_fill_color: default_fill(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileOutcome {
    pub id: String,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    pub original_size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_size: Option<u64>,
    /// 已跳过 / 失败的原因
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeCounts {
    pub total: usize,
    pub success: usize,
    pub unchanged: usize,
    pub skipped: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecReport {
    pub output_dir: String,
    pub outcomes: Vec<FileOutcome>,
    pub counts: OutcomeCounts,
}

/// 探测目录可写性：创建目录并写入再删除一个临时文件（规范 4.1 的判定方式）。
fn ensure_writable_dir(dir: &Path) -> Result<(), AppError> {
    fs::create_dir_all(dir).map_err(|e| {
        AppError::output_unwritable(format!(
            "输出目录不可写：{}（{e}）。请改用「用户指定输出目录」。",
            dir.display()
        ))
    })?;

    let probe = dir.join(".meowfit-write-probe");
    match fs::write(&probe, b"") {
        Ok(()) => {
            let _ = fs::remove_file(&probe);
            Ok(())
        }
        Err(e) => Err(AppError::output_unwritable(format!(
            "输出目录不可写：{}（{e}）。请改用「用户指定输出目录」。",
            dir.display()
        ))),
    }
}

/// 解析输出目录（规范 6.5）。
pub fn resolve_output_dir(root: &Path, opts: &ExecOptions) -> Result<PathBuf, AppError> {
    let dir = match &opts.output_dir {
        Some(d) if !d.trim().is_empty() => PathBuf::from(d),
        _ => scan::default_output_dir(root),
    };
    ensure_writable_dir(&dir)?;
    Ok(dir)
}

/// 处理同名冲突，返回最终输出路径；`None` 表示按策略跳过。
fn resolve_conflict(path: PathBuf, on_conflict: &str) -> Option<PathBuf> {
    if !path.exists() {
        return Some(path);
    }
    match on_conflict {
        "overwrite" => Some(path),
        "rename" => {
            let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let ext = path
                .extension()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            for n in 1..10_000 {
                let name = if ext.is_empty() {
                    format!("{stem}-{n}")
                } else {
                    format!("{stem}-{n}.{ext}")
                };
                let candidate = parent.join(name);
                if !candidate.exists() {
                    return Some(candidate);
                }
            }
            None
        }
        // 默认跳过（规范 6.5 / 附录 B 第 5 项）
        _ => None,
    }
}

fn output_path_for(output_root: &Path, source_id: &str, keep_structure: bool) -> PathBuf {
    let relative = source_id.replace('\\', "/");
    let relative = relative.trim_start_matches('/');
    if keep_structure {
        let mut path = output_root.to_path_buf();
        for segment in relative.split('/').filter(|s| !s.is_empty() && *s != "..") {
            path.push(segment);
        }
        path
    } else {
        let name = relative.rsplit('/').next().unwrap_or(relative);
        output_root.join(name)
    }
}

/// 执行计划（规范 6.7 的单文件粒度部分）。
///
/// `plan` 必须来自 [`crate::algo::plan::build_plan`]，且 `plan.ok == true`；
/// 调用方在 `ok == false` 时不得调用本函数（规范 13.1 的 `VALIDATION_FAILED`）。
pub fn execute(
    plan: &Plan,
    sources: &[SourceRef],
    root: &Path,
    opts: &ExecOptions,
) -> Result<ExecReport, AppError> {
    let output_root = resolve_output_dir(root, opts)?;

    let lookup: std::collections::HashMap<&str, &SourceRef> =
        sources.iter().map(|s| (s.id.as_str(), s)).collect();

    let mut outcomes: Vec<FileOutcome> = Vec::with_capacity(plan.entries.len());

    for entry in &plan.entries {
        let source = match lookup.get(entry.id.as_str()) {
            Some(s) => *s,
            None => {
                outcomes.push(FileOutcome {
                    id: entry.id.clone(),
                    status: Status::Failed,
                    output_path: None,
                    original_size: 0,
                    new_size: None,
                    reason: Some("找不到对应的源文件".into()),
                });
                continue;
            }
        };

        let original_size = fs::metadata(&source.path).map(|m| m.len()).unwrap_or(0);

        if let Some(err) = &entry.error {
            outcomes.push(FileOutcome {
                id: entry.id.clone(),
                status: Status::Failed,
                output_path: None,
                original_size,
                new_size: None,
                reason: Some(format!("{} {}", err.code.as_str(), err.message)),
            });
            continue;
        }

        // 未改动：不写出、不复制、不转码（规范 6.4 / 10.4）
        if entry.action == Action::Unchanged {
            outcomes.push(FileOutcome {
                id: entry.id.clone(),
                status: Status::Unchanged,
                output_path: None,
                original_size,
                new_size: None,
                reason: None,
            });
            continue;
        }

        let target = match entry.target {
            Some(t) => t,
            None => continue,
        };
        let mode = match entry.mode {
            Some(m) => m,
            None => continue,
        };

        if !imaging::is_writable_by_this_stage(source.kind) {
            outcomes.push(FileOutcome {
                id: entry.id.clone(),
                status: Status::Skipped,
                output_path: None,
                original_size,
                new_size: None,
                reason: Some("该类型处理将在后续阶段接入".into()),
            });
            continue;
        }

        let wanted = output_path_for(&output_root, &entry.id, opts.keep_structure);

        // 绝不覆盖源文件（规范第八节）
        if wanted == PathBuf::from(&source.path) {
            outcomes.push(FileOutcome {
                id: entry.id.clone(),
                status: Status::Failed,
                output_path: None,
                original_size,
                new_size: None,
                reason: Some("输出路径与源文件相同，已阻止覆盖源文件".into()),
            });
            continue;
        }

        let final_path = match resolve_conflict(wanted, &opts.on_conflict) {
            Some(p) => p,
            None => {
                outcomes.push(FileOutcome {
                    id: entry.id.clone(),
                    status: Status::Skipped,
                    output_path: None,
                    original_size,
                    new_size: None,
                    reason: Some("输出目录中已存在同名文件".into()),
                });
                continue;
            }
        };

        match imaging::render_and_write(
            Path::new(&source.path),
            &final_path,
            &target,
            mode,
            &opts.background_fill_color,
        ) {
            Ok(new_size) => outcomes.push(FileOutcome {
                id: entry.id.clone(),
                status: Status::Success,
                output_path: Some(final_path.to_string_lossy().into_owned()),
                original_size,
                new_size: Some(new_size),
                reason: None,
            }),
            Err(err) => outcomes.push(FileOutcome {
                id: entry.id.clone(),
                status: Status::Failed,
                output_path: None,
                original_size,
                new_size: None,
                reason: Some(format!("{} {}", err.code.as_str(), err.message)),
            }),
        }
    }

    let mut counts = OutcomeCounts {
        total: outcomes.len(),
        ..Default::default()
    };
    for o in &outcomes {
        match o.status {
            Status::Success => counts.success += 1,
            Status::Unchanged => counts.unchanged += 1,
            Status::Skipped => counts.skipped += 1,
            Status::Failed => counts.failed += 1,
        }
    }

    Ok(ExecReport {
        output_dir: output_root.to_string_lossy().into_owned(),
        outcomes,
        counts,
    })
}

/// 只读介质场景：不可写时给出 `E_OUTPUT_UNWRITABLE`（规范 13.2）
pub fn unwritable_error(dir: &Path) -> AppError {
    AppError::new(
        ErrorCode::OutputUnwritable,
        format!("输出目录不可写：{}", dir.display()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algo::plan::build_plan;
    use crate::model::{
        GroupSetting, MediaKind, Mode, PlanFileInput, PlanGroupInput, PlanRequest, Setting,
    };
    use image::{Rgb, RgbImage};

    fn write_png(path: &Path, w: u32, h: u32) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        RgbImage::from_pixel(w, h, Rgb([10, 20, 30])).save(path).unwrap();
    }

    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "meowfit-exec-{tag}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn plan_for(files: Vec<PlanFileInput>, groups: Vec<PlanGroupInput>, global: Option<Setting>) -> Plan {
        build_plan(&PlanRequest {
            files,
            groups,
            global,
        })
    }

    fn dims_of(path: &Path) -> (u32, u32) {
        image::image_dimensions(path).unwrap()
    }

    #[test]
    fn writes_halved_images_to_sibling_output_and_leaves_source_untouched() {
        // 验收场景 1
        let root = temp_root("halve").join("素材");
        write_png(&root.join("a.png"), 200, 100);
        write_png(&root.join("sub/b.png"), 80, 40);
        let before = fs::metadata(root.join("a.png")).unwrap().len();

        let plan = plan_for(
            vec![
                PlanFileInput {
                    id: "a.png".into(),
                    name: "a.png".into(),
                    width: 200,
                    height: 100,
                    is_video: false,
                    group: "g".into(),
                    setting: None,
                },
                PlanFileInput {
                    id: "sub/b.png".into(),
                    name: "b.png".into(),
                    width: 80,
                    height: 40,
                    is_video: false,
                    group: "g".into(),
                    setting: None,
                },
            ],
            vec![],
            Some(Setting::scale(Mode::A, 0.5)),
        );
        assert!(plan.ok);

        let sources = vec![
            SourceRef {
                id: "a.png".into(),
                path: root.join("a.png").to_string_lossy().into_owned(),
                kind: MediaKind::Raster,
            },
            SourceRef {
                id: "sub/b.png".into(),
                path: root.join("sub/b.png").to_string_lossy().into_owned(),
                kind: MediaKind::Raster,
            },
        ];

        let report = execute(&plan, &sources, &root, &ExecOptions::default()).unwrap();
        assert_eq!(report.counts.success, 2);

        // 输出到 源文件夹的同级目录/output/
        let out = root.parent().unwrap().join("output");
        assert_eq!(report.output_dir, out.to_string_lossy());
        assert_eq!(dims_of(&out.join("a.png")), (100, 50));
        // 保留相对目录结构、输出文件名保持原名
        assert_eq!(dims_of(&out.join("sub/b.png")), (40, 20));

        // 源文件未被改动
        assert_eq!(fs::metadata(root.join("a.png")).unwrap().len(), before);
        assert_eq!(dims_of(&root.join("a.png")), (200, 100));

        let _ = fs::remove_dir_all(root.parent().unwrap());
    }

    #[test]
    fn unchanged_files_are_not_copied_to_output() {
        // 验收场景 2 的关键行为：未改动素材原地不动，输出目录中不存在它
        let root = temp_root("unchanged").join("素材");
        write_png(&root.join("icon_01.png"), 100, 100);
        write_png(&root.join("bg_01.png"), 100, 100);

        let plan = plan_for(
            vec![
                PlanFileInput {
                    id: "icon_01.png".into(),
                    name: "icon_01.png".into(),
                    width: 100,
                    height: 100,
                    is_video: false,
                    group: "icon".into(),
                    setting: None,
                },
                PlanFileInput {
                    id: "bg_01.png".into(),
                    name: "bg_01.png".into(),
                    width: 100,
                    height: 100,
                    is_video: false,
                    group: "bg".into(),
                    setting: None,
                },
            ],
            vec![PlanGroupInput {
                name: "icon".into(),
                setting: Some(GroupSetting::Explicit(Setting::width_height(
                    Mode::D, 128.0, 128.0,
                ))),
            }],
            None,
        );

        let sources = vec![
            SourceRef {
                id: "icon_01.png".into(),
                path: root.join("icon_01.png").to_string_lossy().into_owned(),
                kind: MediaKind::Raster,
            },
            SourceRef {
                id: "bg_01.png".into(),
                path: root.join("bg_01.png").to_string_lossy().into_owned(),
                kind: MediaKind::Raster,
            },
        ];

        let report = execute(&plan, &sources, &root, &ExecOptions::default()).unwrap();
        assert_eq!(report.counts.success, 1);
        assert_eq!(report.counts.unchanged, 1);

        let out = PathBuf::from(&report.output_dir);
        assert!(out.join("icon_01.png").exists());
        assert!(!out.join("bg_01.png").exists(), "未改动素材不得复制到输出目录");

        let _ = fs::remove_dir_all(root.parent().unwrap());
    }

    #[test]
    fn conflict_policy_skip_and_rename() {
        let root = temp_root("conflict").join("素材");
        write_png(&root.join("a.png"), 100, 100);

        let plan = plan_for(
            vec![PlanFileInput {
                id: "a.png".into(),
                name: "a.png".into(),
                width: 100,
                height: 100,
                is_video: false,
                group: "g".into(),
                setting: None,
            }],
            vec![],
            Some(Setting::scale(Mode::A, 0.5)),
        );
        let sources = vec![SourceRef {
            id: "a.png".into(),
            path: root.join("a.png").to_string_lossy().into_owned(),
            kind: MediaKind::Raster,
        }];

        // 先跑一次产生输出
        execute(&plan, &sources, &root, &ExecOptions::default()).unwrap();
        let out = root.parent().unwrap().join("output");

        // 默认策略：跳过
        let again = execute(&plan, &sources, &root, &ExecOptions::default()).unwrap();
        assert_eq!(again.counts.skipped, 1);

        // 自动重命名：追加 -1
        let renamed = execute(
            &plan,
            &sources,
            &root,
            &ExecOptions {
                on_conflict: "rename".into(),
                ..ExecOptions::default()
            },
        )
        .unwrap();
        assert_eq!(renamed.counts.success, 1);
        assert!(out.join("a-1.png").exists());

        let _ = fs::remove_dir_all(root.parent().unwrap());
    }

    #[test]
    fn unwritable_output_dir_reports_e_output_unwritable() {
        let root = temp_root("ro").join("素材");
        write_png(&root.join("a.png"), 100, 100);

        // 用一个「位于普通文件之下」的路径，必然无法创建
        let blocker = root.parent().unwrap().join("blocker");
        fs::write(&blocker, b"x").unwrap();

        let opts = ExecOptions {
            output_dir: Some(blocker.join("sub").to_string_lossy().into_owned()),
            ..ExecOptions::default()
        };
        let err = resolve_output_dir(&root, &opts).unwrap_err();
        assert_eq!(err.code, ErrorCode::OutputUnwritable);

        let _ = fs::remove_dir_all(root.parent().unwrap());
    }

    #[test]
    fn keep_structure_off_flattens_output() {
        let root = temp_root("flat").join("素材");
        write_png(&root.join("sub/a.png"), 100, 100);

        let plan = plan_for(
            vec![PlanFileInput {
                id: "sub/a.png".into(),
                name: "a.png".into(),
                width: 100,
                height: 100,
                is_video: false,
                group: "g".into(),
                setting: None,
            }],
            vec![],
            Some(Setting::scale(Mode::A, 0.5)),
        );
        let sources = vec![SourceRef {
            id: "sub/a.png".into(),
            path: root.join("sub/a.png").to_string_lossy().into_owned(),
            kind: MediaKind::Raster,
        }];

        let report = execute(
            &plan,
            &sources,
            &root,
            &ExecOptions {
                keep_structure: false,
                ..ExecOptions::default()
            },
        )
        .unwrap();
        let out = PathBuf::from(&report.output_dir);
        assert!(out.join("a.png").exists());
        assert!(!out.join("sub/a.png").exists());

        let _ = fs::remove_dir_all(root.parent().unwrap());
    }

    #[test]
    fn mode_b_pads_transparent_for_png_and_background_color_for_jpeg() {
        let root = temp_root("pad").join("素材");
        write_png(&root.join("wide.png"), 200, 100);
        // 同一张图另存为 JPG，用于验证不支持 alpha 的输出格式改用背景色填充
        image::open(root.join("wide.png"))
            .unwrap()
            .to_rgb8()
            .save(root.join("wide.jpg"))
            .unwrap();

        let plan = plan_for(
            vec![
                PlanFileInput {
                    id: "wide.png".into(),
                    name: "wide.png".into(),
                    width: 200,
                    height: 100,
                    is_video: false,
                    group: "g".into(),
                    setting: None,
                },
                PlanFileInput {
                    id: "wide.jpg".into(),
                    name: "wide.jpg".into(),
                    width: 200,
                    height: 100,
                    is_video: false,
                    group: "g".into(),
                    setting: None,
                },
            ],
            vec![],
            Some(Setting::width_height(Mode::B, 200.0, 200.0)),
        );

        let sources = vec![
            SourceRef {
                id: "wide.png".into(),
                path: root.join("wide.png").to_string_lossy().into_owned(),
                kind: MediaKind::Raster,
            },
            SourceRef {
                id: "wide.jpg".into(),
                path: root.join("wide.jpg").to_string_lossy().into_owned(),
                kind: MediaKind::Raster,
            },
        ];

        let report = execute(&plan, &sources, &root, &ExecOptions::default()).unwrap();
        assert_eq!(report.counts.success, 2);
        let out = PathBuf::from(&report.output_dir);

        // PNG：输出 200×200 画布，内容 200×100 居中，上下各 50px 透明
        let png_out = out.join("wide.png");
        assert_eq!(dims_of(&png_out), (200, 200));
        let img = image::open(&png_out).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(100, 10)[3], 0, "空余区域默认应为透明");
        assert_eq!(img.get_pixel(100, 100)[3], 255, "内容区域应不透明");

        // JPG：不支持 alpha，空余区域以背景色（默认白）填充
        let jpg_out = out.join("wide.jpg");
        assert_eq!(dims_of(&jpg_out), (200, 200));
        let img = image::open(&jpg_out).unwrap().to_rgba8();
        let pad = img.get_pixel(100, 10);
        assert!(
            pad[0] > 240 && pad[1] > 240 && pad[2] > 240,
            "JPG 空余区域应为白色，实际 {pad:?}"
        );

        let _ = fs::remove_dir_all(root.parent().unwrap());
    }
}
