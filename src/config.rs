//! 应用配置持久化（{config_dir}/rclone-gui/config.json）

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConnectionProfile {
    pub name: String,
    /// 形如 "127.0.0.1:5572" 或 "http://nas:5572"
    pub addr: String,
    pub user: String,
    pub pass: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    /// 用户手动指定的 rclone 可执行文件路径
    #[serde(default)]
    pub rclone_path: Option<PathBuf>,
    /// 外部 rcd 连接
    #[serde(default)]
    pub connections: Vec<ConnectionProfile>,
    /// 当前激活的外部连接名（None 表示本地托管）
    #[serde(default)]
    pub active_connection: Option<String>,
}

impl AppConfig {
    pub fn load() -> Self {
        let path = config_file();
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = config_file();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }
}

fn config_file() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rclone-gui")
        .join("config.json")
}

/// 应用数据目录（托管二进制、日志）
pub fn data_dir() -> PathBuf {
    let dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rclone-gui");
    let _ = std::fs::create_dir_all(&dir);
    dir
}
