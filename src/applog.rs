//! 极简应用日志：写入 {data_dir}/logs/app.log
//! 低频打点用（检测/下载/守护进程生命周期），不引入外部日志框架

use std::io::Write;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);

pub fn log_path() -> std::path::PathBuf {
    crate::config::data_dir().join("logs").join("app.log")
}

/// 追加一条日志（带 UTC 时间戳）
pub fn info(msg: &str) {
    let line = format!("[{}] {msg}\n", timestamp());
    let path = log_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let result = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut f| f.write_all(line.as_bytes()));
    if let Err(e) = result {
        // 日志失败不应影响主流程，但保留最后一次错误便于排查
        if let Ok(mut guard) = LAST_ERROR.lock() {
            *guard = Some(e.to_string());
        }
    }
    #[cfg(debug_assertions)]
    eprint!("{line}");
}

/// 当前 UTC 时间戳（RFC3339 简化版，无需 chrono）
fn timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86400) as i64;
    let secs_of_day = secs % 86400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60
    )
}

/// Howard Hinnant 的 civil_from_days（days 为 1970-01-01 起的天数）
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
