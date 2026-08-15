//! rclone Remote Control (rc) HTTP API 客户端

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct Connection {
    /// 形如 "127.0.0.1:5572" 或 "http://nas:5572"
    pub addr: String,
    pub user: String,
    pub pass: String,
}

#[derive(Clone)]
pub struct RcClient {
    base_url: String,
    auth_header: String,
    agent: ureq::Agent,
}

// ---------- rc 返回类型 ----------

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // os/arch 后续里程碑用于展示
pub struct CoreVersion {
    pub version: String,
    #[serde(default)]
    pub os: String,
    #[serde(default)]
    pub arch: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RemoteEntry {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(flatten)]
    #[allow(dead_code)]
    pub params: BTreeMap<String, serde_json::Value>,
}

// 注意：config/providers 直接序列化 Go 结构体，字段为 PascalCase
#[derive(Debug, Clone, Deserialize)]
pub struct Provider {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(default, rename = "Description")]
    pub description: String,
    #[serde(default, rename = "Options")]
    pub options: Vec<ProviderOption>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // 字段来自 schema，部分暂未在 UI 使用
pub struct ProviderOption {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(default, rename = "Help")]
    pub help: String,
    #[serde(default, rename = "Required")]
    pub required: bool,
    #[serde(default, rename = "IsPassword")]
    pub is_password: bool,
    #[serde(default, rename = "Sensitive")]
    pub sensitive: bool,
    #[serde(default, rename = "Advanced")]
    pub advanced: bool,
    #[serde(default, rename = "Type")]
    pub kind: String,
    #[serde(default, rename = "Default")]
    pub default: serde_json::Value,
    #[serde(default, rename = "Examples")]
    pub examples: Vec<ProviderExample>,
}

impl ProviderOption {
    /// 用帮助文本的首行作为输入框占位符
    pub fn help_or_placeholder(&self) -> String {
        if !self.help.is_empty() {
            self.help.lines().next().unwrap_or("").trim().to_string()
        } else if self.default.is_string() {
            self.default.as_str().unwrap().to_string()
        } else {
            String::new()
        }
    }

    /// 用于表单展示的帮助文本（只取第一段，避免过长）
    pub fn help_summary(&self) -> String {
        self.help
            .split("\n\n")
            .next()
            .unwrap_or("")
            .trim()
            .to_string()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ProviderExample {
    #[serde(default, rename = "Value")]
    pub value: serde_json::Value,
    #[serde(default, rename = "Help")]
    pub help: String,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct DirEntry {
    #[serde(rename = "Path", default)]
    pub path: String,
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "Size", default)]
    pub size: i64,
    #[serde(rename = "MimeType", default)]
    pub mime_type: String,
    #[serde(rename = "ModTime", default)]
    pub mod_time: String,
    #[serde(rename = "IsDir", default)]
    pub is_dir: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[allow(dead_code)]
pub struct JobStatus {
    #[serde(default)]
    pub finished: bool,
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub duration: f64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[allow(dead_code)]
pub struct CoreStats {
    #[serde(default)]
    pub bytes: u64,
    #[serde(default, rename = "totalBytes")]
    pub total_bytes: u64,
    #[serde(default)]
    pub speed: f64,
    #[serde(default)]
    pub eta: Option<f64>,
    #[serde(default)]
    pub transfers: u64,
    #[serde(default, rename = "totalTransfers")]
    pub total_transfers: u64,
    #[serde(default)]
    pub errors: u64,
    #[serde(default)]
    pub transferring: Vec<Transferring>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Transferring {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub bytes: u64,
    #[serde(default)]
    pub percentage: u64,
}

#[derive(Debug, Deserialize)]
struct JobIdResponse {
    #[serde(default)]
    jobid: u64,
}

#[derive(Debug, Deserialize)]
struct ListRemotesResponse {
    #[serde(default)]
    remotes: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ProvidersResponse {
    #[serde(default)]
    providers: Vec<Provider>,
}

#[derive(Debug, Deserialize)]
struct OperationsListResponse {
    #[serde(default)]
    list: Vec<DirEntry>,
}

#[derive(Debug, Deserialize)]
struct RcError {
    error: Option<String>,
}

// ---------- 客户端 ----------

impl RcClient {
    pub fn new(conn: &Connection) -> Self {
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            // rc 地址是本机/内网，必须绕过系统代理，否则会被代理服务器拦截导致超时
            .proxy(None)
            .build();
        let token = STANDARD.encode(format!("{}:{}", conn.user, conn.pass));
        let base_url = if conn.addr.starts_with("http://") || conn.addr.starts_with("https://") {
            conn.addr.trim_end_matches('/').to_string()
        } else {
            format!("http://{}", conn.addr)
        };
        Self {
            base_url,
            auth_header: format!("Basic {token}"),
            agent: ureq::Agent::new_with_config(config),
        }
    }

    /// 调用任意 rc 端点，如 `core/version`
    pub fn call<Req: Serialize, Resp: DeserializeOwned>(
        &self,
        method: &str,
        req: &Req,
    ) -> Result<Resp, String> {
        let url = format!("{}/{}", self.base_url, method);
        let mut resp = self
            .agent
            .post(&url)
            .header("Authorization", &self.auth_header)
            .header("Content-Type", "application/json")
            .send_json(req)
            .map_err(|e| format!("连接 rcd 失败: {e}"))?;

        if resp.status().as_u16() != 200 {
            let body = resp.body_mut().read_to_string().unwrap_or_default();
            let detail = serde_json::from_str::<RcError>(&body)
                .ok()
                .and_then(|e| e.error)
                .unwrap_or(body);
            return Err(format!("rc 接口 {method} 错误: {detail}"));
        }

        resp.body_mut()
            .read_json::<Resp>()
            .map_err(|e| format!("解析 rc 响应失败: {e}"))
    }

    pub fn core_version(&self) -> Result<CoreVersion, String> {
        self.call("core/version", &serde_json::json!({}))
    }

    /// config/listremotes
    pub fn list_remotes(&self) -> Result<Vec<String>, String> {
        let resp: ListRemotesResponse = self.call("config/listremotes", &serde_json::json!({}))?;
        Ok(resp.remotes)
    }

    /// config/dump：所有 remote 的完整配置（含 type）
    pub fn config_dump(&self) -> Result<BTreeMap<String, RemoteEntry>, String> {
        self.call("config/dump", &serde_json::json!({}))
    }

    /// config/providers：所有后端 schema
    pub fn config_providers(&self) -> Result<Vec<Provider>, String> {
        let resp: ProvidersResponse = self.call("config/providers", &serde_json::json!({}))?;
        Ok(resp.providers)
    }

    /// config/create：创建 remote
    pub fn config_create(
        &self,
        name: &str,
        kind: &str,
        params: &BTreeMap<String, String>,
    ) -> Result<(), String> {
        let body = serde_json::json!({
            "name": name,
            "type": kind,
            "parameters": params,
            "obscure": true,
        });
        self.call::<_, serde_json::Value>("config/create", &body)?;
        Ok(())
    }

    /// config/update：更新 remote 参数（不传的值保留）
    #[allow(dead_code)] // 编辑 remote 功能后续启用
    pub fn config_update(
        &self,
        name: &str,
        params: &BTreeMap<String, String>,
    ) -> Result<(), String> {
        let body = serde_json::json!({
            "name": name,
            "parameters": params,
            "obscure": true,
        });
        self.call::<_, serde_json::Value>("config/update", &body)?;
        Ok(())
    }

    /// config/delete：删除 remote
    pub fn config_delete(&self, name: &str) -> Result<(), String> {
        let body = serde_json::json!({ "name": name });
        self.call::<_, serde_json::Value>("config/delete", &body)?;
        Ok(())
    }

    /// operations/list：列出 fs 下 remote 路径的内容
    pub fn operations_list(&self, fs: &str, remote: &str) -> Result<Vec<DirEntry>, String> {
        let body = serde_json::json!({ "fs": fs, "remote": remote });
        let resp: OperationsListResponse = self.call("operations/list", &body)?;
        Ok(resp.list)
    }

    /// sync/copy（异步），返回 jobid
    pub fn start_copy(&self, src_fs: &str, dst_fs: &str) -> Result<u64, String> {
        let body = serde_json::json!({
            "srcFs": src_fs,
            "dstFs": dst_fs,
            "_async": true,
        });
        let resp: JobIdResponse = self.call("sync/copy", &body)?;
        Ok(resp.jobid)
    }

    /// sync/move（异步），返回 jobid
    pub fn start_move(&self, src_fs: &str, dst_fs: &str) -> Result<u64, String> {
        let body = serde_json::json!({
            "srcFs": src_fs,
            "dstFs": dst_fs,
            "_async": true,
        });
        let resp: JobIdResponse = self.call("sync/move", &body)?;
        Ok(resp.jobid)
    }

    /// sync/sync（异步），返回 jobid
    pub fn start_sync(&self, src_fs: &str, dst_fs: &str) -> Result<u64, String> {
        let body = serde_json::json!({
            "srcFs": src_fs,
            "dstFs": dst_fs,
            "_async": true,
        });
        let resp: JobIdResponse = self.call("sync/sync", &body)?;
        Ok(resp.jobid)
    }

    /// job/status
    pub fn job_status(&self, jobid: u64) -> Result<JobStatus, String> {
        let body = serde_json::json!({ "jobid": jobid });
        self.call("job/status", &body)
    }

    /// job/stop：取消任务
    pub fn job_stop(&self, jobid: u64) -> Result<(), String> {
        let body = serde_json::json!({ "jobid": jobid });
        self.call::<_, serde_json::Value>("job/stop", &body)?;
        Ok(())
    }

    /// core/stats：统计（group 形如 "job/3"）
    pub fn core_stats(&self, group: &str) -> Result<CoreStats, String> {
        let body = serde_json::json!({ "group": group });
        self.call("core/stats", &body)
    }
}
