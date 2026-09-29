//! 日志落盘（规范 6.13 / 11.5）。
//!
//! 纯文本、UTF-8，每行 `[时间] [级别] 消息`，按日期滚动，保留 30 天。

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// 日志保留天数（规范 11.5）
pub const RETAIN_DAYS: u64 = 30;

/// 日志目录：`config/../logs/`（规范 6.13 的写法）。
pub fn log_dir(config_dir: &Path) -> PathBuf {
    match config_dir.parent() {
        Some(parent) => parent.join("logs"),
        None => config_dir.join("logs"),
    }
}

fn file_for(dir: &Path, date: chrono::NaiveDate) -> PathBuf {
    dir.join(format!("meowfit-{}.log", date.format("%Y-%m-%d")))
}

/// 追加一行日志；写不进去也不影响主流程（返回 `Err` 由调用方决定是否提示）。
pub fn append(config_dir: &Path, level: &str, message: &str) -> Result<(), String> {
    let dir = log_dir(config_dir);
    fs::create_dir_all(&dir).map_err(|e| format!("创建日志目录失败：{e}"))?;

    let now = chrono::Local::now();
    let path = file_for(&dir, now.date_naive());
    let line = format!("[{}] [{}] {}\n", now.format("%Y-%m-%d %H:%M:%S"), level, message);

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("打开日志文件失败：{e}"))?;
    file.write_all(line.as_bytes())
        .map_err(|e| format!("写入日志失败：{e}"))?;
    Ok(())
}

/// 清理超过保留期的日志，返回删除的文件数。
pub fn purge_old(config_dir: &Path) -> usize {
    let dir = log_dir(config_dir);
    let Ok(entries) = fs::read_dir(&dir) else {
        return 0;
    };
    let cutoff = chrono::Local::now().date_naive() - chrono::Days::new(RETAIN_DAYS);
    let mut removed = 0;

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(stamp) = name
            .strip_prefix("meowfit-")
            .and_then(|rest| rest.strip_suffix(".log"))
        else {
            continue;
        };
        let Ok(date) = chrono::NaiveDate::parse_from_str(stamp, "%Y-%m-%d") else {
            continue;
        };
        if date < cutoff && fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_config(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "meowfit-log-{tag}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("config")).unwrap();
        dir.join("config")
    }

    #[test]
    fn log_dir_is_a_sibling_of_config() {
        // 规范 6.13：日志写入 config/../logs/
        let config = Path::new(r"D:\app\config");
        assert_eq!(log_dir(config), PathBuf::from(r"D:\app\logs"));
    }

    #[test]
    fn append_creates_dated_file_with_spec_format() {
        let config = temp_config("append");
        append(&config, "INFO", "启动完成").unwrap();
        append(&config, "ERROR", "有文件失败").unwrap();

        let dir = log_dir(&config);
        let files: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(files.len(), 1, "同一天只应有一个文件");
        assert!(files[0].starts_with("meowfit-") && files[0].ends_with(".log"));

        let text = fs::read_to_string(dir.join(&files[0])).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        // 格式：[时间] [级别] 消息
        assert!(lines[0].starts_with('['), "实际：{}", lines[0]);
        assert!(lines[0].contains("] [INFO] "), "实际：{}", lines[0]);
        assert!(lines[0].ends_with("启动完成"));
        assert!(lines[1].contains("] [ERROR] "), "实际：{}", lines[1]);

        let _ = fs::remove_dir_all(config.parent().unwrap());
    }

    #[test]
    fn purge_removes_only_expired_logs() {
        let config = temp_config("purge");
        let dir = log_dir(&config);
        fs::create_dir_all(&dir).unwrap();
        let today = chrono::Local::now().date_naive();

        let write = |date: chrono::NaiveDate| {
            fs::write(file_for(&dir, date), "x").unwrap();
        };
        write(today);
        write(today - chrono::Days::new(29)); // 保留期内
        write(today - chrono::Days::new(31)); // 超期

        let removed = purge_old(&config);
        assert_eq!(removed, 1, "只应删掉超期的那一个");
        assert!(file_for(&dir, today).exists());
        assert!(file_for(&dir, today - chrono::Days::new(29)).exists());
        assert!(!file_for(&dir, today - chrono::Days::new(31)).exists());

        let _ = fs::remove_dir_all(config.parent().unwrap());
    }

    #[test]
    fn append_is_resilient_when_the_directory_is_unwritable() {
        // 目录不可创建时返回 Err 而不是 panic，主流程不应因此中断
        let missing = Path::new(r"Z:\definitely\not\here");
        assert!(append(missing, "INFO", "x").is_err());
    }
}
