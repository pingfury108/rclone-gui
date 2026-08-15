//! rclone 二进制管理：检测、版本查询、托管下载

use crate::config::{data_dir, AppConfig};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct FoundBinary {
    pub path: PathBuf,
    pub version: String,
}

pub fn binary_name() -> &'static str {
    if cfg!(windows) {
        "rclone.exe"
    } else {
        "rclone"
    }
}

/// 托管安装路径：{data_dir}/rclone-gui/bin/rclone[.exe]
pub fn managed_bin_path() -> PathBuf {
    data_dir().join("bin").join(binary_name())
}

/// 按优先级查找可用的 rclone：用户指定 > 托管 > PATH
pub fn detect() -> Option<FoundBinary> {
    if let Some(custom) = AppConfig::load().rclone_path {
        match version_of(&custom) {
            Some(version) => {
                crate::applog::info(&format!("rclone 检测：使用用户指定路径 {custom:?} ({version})"));
                return Some(FoundBinary {
                    path: custom,
                    version,
                });
            }
            None => crate::applog::info(&format!("rclone 检测：用户指定路径不可用 {custom:?}")),
        }
    }

    let managed = managed_bin_path();
    if let Some(version) = version_of(&managed) {
        crate::applog::info(&format!("rclone 检测：使用托管二进制 {managed:?} ({version})"));
        return Some(FoundBinary {
            path: managed,
            version,
        });
    }

    // PATH：直接用命令名，由操作系统解析
    if let Some(version) = version_of(Path::new(binary_name())) {
        crate::applog::info(&format!("rclone 检测：使用 PATH 中的 rclone ({version})"));
        return Some(FoundBinary {
            path: PathBuf::from(binary_name()),
            version,
        });
    }

    crate::applog::info("rclone 检测：未找到可用的 rclone");
    None
}

/// 运行 `rclone version` 并解析首行 `rclone vX.Y.Z`
pub fn version_of(bin: &Path) -> Option<String> {
    let mut cmd = Command::new(bin);
    cmd.arg("version");
    prepare_command(&mut cmd);
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first = stdout.lines().next()?;
    first
        .strip_prefix("rclone ")
        .map(|v| v.trim().to_string())
        .filter(|v| v.starts_with('v'))
}

/// 当前平台的 (os, arch)
fn os_arch() -> (&'static str, &'static str) {
    let os = if cfg!(target_os = "macos") {
        "osx"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    };
    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "amd64"
    };
    (os, arch)
}

/// 无法动态获取最新版本时的回退版本（GitHub 保留历史版本，不影响下载）
const FALLBACK_VERSION: &str = "1.75.0";

/// 下载源列表（按优先级排序，失败自动切换下一个）
fn candidate_urls() -> Vec<String> {
    let (os, arch) = os_arch();
    let version = latest_version().unwrap_or_else(|| FALLBACK_VERSION.to_string());
    let gh_file = format!("rclone-v{version}-{os}-{arch}.zip");
    let gh_url =
        format!("https://github.com/rclone/rclone/releases/download/v{version}/{gh_file}");
    vec![
        format!("https://downloads.rclone.org/rclone-current-{os}-{arch}.zip"),
        format!("https://gh-proxy.com/{gh_url}"),
        format!("https://ghfast.top/{gh_url}"),
        format!("https://mirror.ghproxy.com/{gh_url}"),
        gh_url,
    ]
}

/// 从官方小文件获取最新版本号（如 "rclone v1.71.0" → "1.71.0"）
fn latest_version() -> Option<String> {
    let agent = download_agent();
    let mut resp = agent
        .get("https://downloads.rclone.org/version.txt")
        .call()
        .ok()?;
    let text = resp.body_mut().read_to_string().ok()?;
    let version = text.split_whitespace().nth(1)?.trim_start_matches('v');
    Some(version.to_string())
}

/// 带超时的下载 Agent（避免慢源无限挂起）
fn download_agent() -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_connect(Some(std::time::Duration::from_secs(10)))
        .timeout_recv_response(Some(std::time::Duration::from_secs(15)))
        .timeout_recv_body(Some(std::time::Duration::from_secs(120)))
        .build();
    ureq::Agent::new_with_config(config)
}

/// 下载进度事件
pub enum DownloadEvent {
    /// 开始尝试某个源（index/total/url）
    Source { index: usize, total: usize, url: String },
    /// 当前源下载进度（0.0 ~ 1.0）
    Progress(f32),
}

/// 下载最新版 rclone 并安装到托管路径，返回安装位置
pub fn download_latest(mut progress: impl FnMut(DownloadEvent)) -> Result<PathBuf, String> {
    let urls = candidate_urls();
    crate::applog::info(&format!("开始下载 rclone，候选源 {} 个", urls.len()));
    let total = urls.len();
    let mut last_err = String::new();
    for (index, url) in urls.iter().enumerate() {
        crate::applog::info(&format!("尝试下载源: {url}"));
        progress(DownloadEvent::Source {
            index: index + 1,
            total,
            url: url.clone(),
        });
        match try_download(url, &mut progress) {
            Ok(bytes) => {
                crate::applog::info(&format!("下载成功，共 {} 字节，开始解压", bytes.len()));
                let dest = extract_binary(&bytes)?;
                crate::applog::info(&format!("rclone 安装完成: {dest:?}"));
                return Ok(dest);
            }
            Err(e) => {
                crate::applog::info(&format!("下载源失败: {url} → {e}"));
                last_err = e;
            }
        }
    }
    Err(format!("所有下载源均失败。最后错误: {last_err}\n可前往 https://rclone.org 手动安装后重启应用"))
}

fn try_download(
    url: &str,
    progress: &mut impl FnMut(DownloadEvent),
) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let agent = download_agent();
    let mut resp = agent
        .get(url)
        .call()
        .map_err(|e| format!("请求失败: {e}"))?;
    if resp.status().as_u16() != 200 {
        return Err(format!("HTTP {}", resp.status()));
    }
    let total: Option<u64> = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok());

    let mut reader = resp.body_mut().as_reader();
    let mut data = Vec::with_capacity(total.unwrap_or(24 * 1024 * 1024) as usize);
    let mut buf = [0u8; 64 * 1024];
    let mut last_reported = 0.0f32;
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("读取下载内容失败: {e}"))?;
        if n == 0 {
            break;
        }
        if data.len() + n > 300 * 1024 * 1024 {
            return Err("下载内容超出大小限制".to_string());
        }
        data.extend_from_slice(&buf[..n]);
        if let Some(total) = total {
            let fraction = (data.len() as f32 / total as f32).min(1.0);
            // 每 1% 上报一次，避免通道洪泛
            if fraction - last_reported >= 0.01 {
                last_reported = fraction;
                progress(DownloadEvent::Progress(fraction));
            }
        }
    }
    progress(DownloadEvent::Progress(1.0));
    Ok(data)
}

/// 从官方 zip 包中解压 rclone 可执行文件到托管路径
fn extract_binary(zip_bytes: &[u8]) -> Result<PathBuf, String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes))
        .map_err(|e| format!("解压失败: {e}"))?;

    let mut data = Vec::new();
    let mut found = false;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("读取压缩包失败: {e}"))?;
        let name = entry.name().to_string();
        if name.ends_with("/rclone") || name.ends_with("/rclone.exe") {
            entry
                .read_to_end(&mut data)
                .map_err(|e| format!("解压失败: {e}"))?;
            found = true;
            break;
        }
    }
    if !found {
        return Err("压缩包中未找到 rclone 可执行文件".to_string());
    }

    let dest = managed_bin_path();
    if let Some(dir) = dest.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut file = std::fs::File::create(&dest).map_err(|e| format!("写入失败: {e}"))?;
    file.write_all(&data).map_err(|e| format!("写入失败: {e}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755));
    }

    Ok(dest)
}

/// Windows 下隐藏子进程控制台窗口
pub fn prepare_command(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}
