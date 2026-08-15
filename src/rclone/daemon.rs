//! 本地托管 rcd 守护进程生命周期

use super::client::{Connection, RcClient};
use super::manager::prepare_command;
use crate::config::data_dir;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub struct Daemon {
    child: Child,
    pub conn: Connection,
}

impl Daemon {
    pub fn client(&self) -> RcClient {
        RcClient::new(&self.conn)
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// 启动本地 rcd 并等待就绪（阻塞，请在后台线程调用）
pub fn start_and_wait(bin: &Path) -> Result<Daemon, String> {
    let port = pick_free_port()?;
    let conn = Connection {
        addr: format!("127.0.0.1:{port}"),
        user: "rclone-gui".to_string(),
        pass: random_token(),
    };

    let log_file = data_dir().join("logs").join("rcd.log");
    if let Some(dir) = log_file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let log_arg = log_file.to_string_lossy().into_owned();

    let mut cmd = Command::new(bin);
    cmd.args([
        "rcd",
        "--rc-addr",
        &conn.addr,
        "--rc-user",
        &conn.user,
        "--rc-pass",
        &conn.pass,
        "--log-level",
        "NOTICE",
        "--log-file",
        &log_arg,
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null());
    prepare_command(&mut cmd);

    let child = cmd.spawn().map_err(|e| format!("启动 rcd 失败: {e}"))?;
    crate::applog::info(&format!(
        "rcd 已启动: addr={} pid={} log={log_arg}",
        conn.addr,
        child.id()
    ));
    let mut daemon = Daemon { child, conn };
    let client = daemon.client();

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut last_err = String::new();
    while Instant::now() < deadline {
        match client.core_version() {
            Ok(_) => {
                crate::applog::info("rcd 就绪探测成功");
                return Ok(daemon);
            }
            Err(e) => last_err = e,
        }
        if let Ok(Some(status)) = daemon.child.try_wait() {
            crate::applog::info(&format!("rcd 启动后立即退出: {status}"));
            return Err(format!("rcd 启动后立即退出（{status}），日志见 {log_file:?}"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    crate::applog::info(&format!("等待 rcd 就绪超时，最后错误: {last_err}"));
    Err(format!("等待 rcd 就绪超时（{last_err}），日志见 {log_file:?}"))
}

/// 取一个本机空闲端口（释放后交给 rclone，存在极小竞争窗口）
fn pick_free_port() -> Result<u16, String> {
    let listener =
        TcpListener::bind("127.0.0.1:0").map_err(|e| format!("分配端口失败: {e}"))?;
    listener
        .local_addr()
        .map(|a| a.port())
        .map_err(|e| format!("分配端口失败: {e}"))
}

/// 本机随机 token（非密码学场景，防本机其他进程误连即可）
fn random_token() -> String {
    let mut h = DefaultHasher::new();
    h.write_u32(std::process::id());
    if let Ok(d) = SystemTime::now().duration_since(UNIX_EPOCH) {
        h.write_u128(d.as_nanos());
    }
    format!("{:016x}", h.finish())
}
