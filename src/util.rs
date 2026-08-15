//! 通用格式化工具

/// 人类可读的大小（字节 → "12.3 MB"）
pub fn fmt_size(bytes: f64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// 人类可读的速度（字节/秒 → "12.3 MB/s"）
pub fn fmt_speed(speed: f64) -> String {
    format!("{}/s", fmt_size(speed))
}

/// 人类可读的 ETA（秒 → "1h 2m 3s"）
pub fn fmt_eta(seconds: Option<f64>) -> String {
    match seconds {
        Some(s) if s.is_finite() && s >= 0.0 => {
            let s = s as u64;
            let (h, m, s) = (s / 3600, (s % 3600) / 60, s % 60);
            if h > 0 {
                format!("{h}h {m}m {s}s")
            } else if m > 0 {
                format!("{m}m {s}s")
            } else {
                format!("{s}s")
            }
        }
        _ => "--".to_string(),
    }
}

/// 路径拼接（处理空段）
pub fn join_path(base: &str, name: &str) -> String {
    if base.is_empty() {
        name.to_string()
    } else if base.ends_with('/') {
        format!("{base}{name}")
    } else {
        format!("{base}/{name}")
    }
}

/// 读取日志文件尾部（最近 64KB，最多 400 行）
pub fn read_log_tail(path: &std::path::Path) -> Vec<String> {
    let Ok(meta) = std::fs::metadata(path) else {
        return Vec::new();
    };
    let len = meta.len();
    let read_from = len.saturating_sub(64 * 1024);
    let Ok(mut file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    use std::io::{Read, Seek, SeekFrom};
    let _ = file.seek(SeekFrom::Start(read_from));
    let mut buf = String::new();
    let _ = file.read_to_string(&mut buf);
    buf.lines().rev().take(400).map(|s| s.to_string()).collect::<Vec<_>>().into_iter().rev().collect()
}
