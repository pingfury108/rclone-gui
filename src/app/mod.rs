//! 根视图：状态机 + 导航 + 所有领域逻辑
//! 各页面渲染在 app/views/ 子模块中

mod views;

use crate::config::{AppConfig, ConnectionProfile};
use crate::rclone::client::{CoreStats, DirEntry, RcClient};
use crate::rclone::daemon::{self, Daemon};
use crate::rclone::manager;
use crate::stext::st;
use crate::text_input::TextInput;
use crate::theme::*;
use crate::util;
use gpui::{prelude::*, *};
use std::path::PathBuf;
use std::time::Duration;

// ---------- 页面 ----------

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Dashboard,
    Remotes,
    Browser,
    Transfers,
    Logs,
    Settings,
}

impl Page {
    pub fn label(self) -> &'static str {
        match self {
            Page::Dashboard => "仪表盘",
            Page::Remotes => "存 储",
            Page::Browser => "浏 览",
            Page::Transfers => "传 输",
            Page::Logs => "日 志",
            Page::Settings => "设 置",
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Page::Dashboard => "nav-dashboard",
            Page::Remotes => "nav-remotes",
            Page::Browser => "nav-browser",
            Page::Transfers => "nav-transfers",
            Page::Logs => "nav-logs",
            Page::Settings => "nav-settings",
        }
    }
}

// ---------- 模态对话框 ----------

/// 二级操作统一走模态对话框
pub enum ModalState {
    None,
    /// 删除存储确认（remote 名）
    ConfirmDeleteRemote(String),
    /// 删除外部连接确认（索引）
    ConfirmDeleteConn(usize),
    /// 外部连接表单（数据在 conn_form）
    ConnForm,
}

// ---------- 状态 ----------

pub enum SetupState {
    Checking,
    Missing,
    Downloading { fraction: f32, note: String },
    Ready { bin: PathBuf, version: String },
    Failed(String),
}

pub enum DaemonState {
    Starting,
    Running(Daemon),
    Failed(String),
}

#[derive(Clone, PartialEq, Eq)]
pub enum ActiveConn {
    Local,
    External(String),
}

impl ActiveConn {
    pub fn label(&self) -> String {
        match self {
            ActiveConn::Local => "本地（托管）".to_string(),
            ActiveConn::External(name) => name.clone(),
        }
    }
}

#[derive(Clone, Default)]
pub struct RemoteInfo {
    pub name: String,
    pub kind: String,
}

pub enum RemotesState {
    Idle,
    Loading,
    Loaded(Vec<RemoteInfo>),
    Failed(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum JobUiStatus {
    Running,
    Done,
    Stopped,
    Failed,
}

pub struct TransferJob {
    pub jobid: u64,
    pub kind: String,
    pub src: String,
    pub dst: String,
    pub conn: String,
    pub status: JobUiStatus,
    pub stats: CoreStats,
    pub error: String,
}

pub enum ProvidersLoad {
    Loading,
    Loaded(Vec<crate::rclone::client::Provider>),
    Failed(String),
}

pub struct FieldEntry {
    pub option: crate::rclone::client::ProviderOption,
    pub input: Entity<TextInput>,
}

pub struct WizardState {
    pub providers: ProvidersLoad,
    pub show_all: bool,
    pub selected: Option<String>,
    pub name_input: Entity<TextInput>,
    pub fields: Vec<FieldEntry>,
    pub show_advanced: bool,
    pub busy: bool,
    pub message: Option<(bool, String)>,
}

pub enum PaneLoad {
    Loading,
    Loaded(Vec<DirEntry>),
    Failed(String),
}

pub struct BrowserState {
    pub local_path: String,
    pub remote_name: Option<String>,
    pub remote_path: String,
    pub local_entries: PaneLoad,
    pub remote_entries: PaneLoad,
    pub local_selected: Option<String>,
    pub remote_selected: Option<String>,
    /// 操作提示（如“请先选择文件”）
    pub notice: Option<String>,
}

impl Default for BrowserState {
    fn default() -> Self {
        Self {
            local_path: dirs::home_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "/".to_string()),
            remote_name: None,
            remote_path: String::new(),
            local_entries: PaneLoad::Loading,
            remote_entries: PaneLoad::Loading,
            local_selected: None,
            remote_selected: None,
            notice: None,
        }
    }
}

pub struct LogsState {
    pub lines: Vec<String>,
    pub streaming: bool,
}

impl Default for LogsState {
    fn default() -> Self {
        Self {
            lines: Vec::new(),
            streaming: false,
        }
    }
}

pub struct ConnForm {
    pub name: Entity<TextInput>,
    pub addr: Entity<TextInput>,
    pub user: Entity<TextInput>,
    pub pass: Entity<TextInput>,
    pub editing: Option<usize>,
    pub message: Option<(bool, String)>,
}

pub struct RootView {
    pub setup: SetupState,
    pub daemon: Option<DaemonState>,
    pub connections: Vec<ConnectionProfile>,
    pub active: ActiveConn,
    pub remotes: RemotesState,
    pub remote_test: Option<(String, Result<String, String>)>,
    pub modal: ModalState,
    pub page: Page,
    pub wizard: Option<WizardState>,
    pub browser: BrowserState,
    pub transfers: Vec<TransferJob>,
    pub transfers_polling: bool,
    pub logs: LogsState,
    pub app_logs: Vec<String>,
    pub conn_form: Option<ConnForm>,
    pub conn_test: Option<(String, Result<String, String>)>,
    pub focus_handle: FocusHandle,
}

impl RootView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let config = AppConfig::load();
        let connections = config.connections.clone();
        let active = match &config.active_connection {
            Some(name) if connections.iter().any(|c| &c.name == name) => {
                ActiveConn::External(name.clone())
            }
            _ => ActiveConn::Local,
        };
        let view = Self {
            setup: SetupState::Checking,
            daemon: None,
            connections,
            active,
            remotes: RemotesState::Idle,
            remote_test: None,
            modal: ModalState::None,
            page: Page::Dashboard,
            wizard: None,
            browser: BrowserState::default(),
            transfers: Vec::new(),
            transfers_polling: false,
            logs: LogsState::default(),
            app_logs: Vec::new(),
            conn_form: None,
            conn_test: None,
            focus_handle: cx.focus_handle(),
        };
        view.detect(cx);
        view
    }

    // ---------- rclone 检测 / 安装 ----------

    fn detect(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            let found = cx
                .background_executor()
                .spawn(async move { manager::detect() })
                .await;
            this.update(cx, |this, cx| {
                match found {
                    Some(f) => {
                        this.setup = SetupState::Ready {
                            bin: f.path,
                            version: f.version,
                        };
                        this.start_daemon(cx);
                    }
                    None => this.setup = SetupState::Missing,
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn start_download(&mut self, cx: &mut Context<Self>) {
        self.setup = SetupState::Downloading {
            fraction: 0.0,
            note: "准备下载…".to_string(),
        };
        cx.notify();

        enum Msg {
            Event(manager::DownloadEvent),
            Finished(Result<PathBuf, String>),
        }
        let (tx, rx) = std::sync::mpsc::channel::<Msg>();
        std::thread::spawn(move || {
            let tx2 = tx.clone();
            let result = manager::download_latest(move |ev| {
                let _ = tx2.send(Msg::Event(ev));
            });
            let _ = tx.send(Msg::Finished(result));
        });

        cx.spawn(async move |this: WeakEntity<Self>, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let mut finished = None;
            let mut events = Vec::new();
            for msg in rx.try_iter() {
                match msg {
                    Msg::Event(e) => events.push(e),
                    Msg::Finished(r) => finished = Some(r),
                }
            }
            let done = finished.is_some();
            this.update(cx, |this, cx| {
                for ev in events {
                    if let SetupState::Downloading { fraction, note } = &mut this.setup {
                        match ev {
                            manager::DownloadEvent::Source { index, total, url } => {
                                *fraction = 0.0;
                                *note = format!("正在下载…（源 {index}/{total}）");
                                let _ = url; // 详细地址见 app.log
                            }
                            manager::DownloadEvent::Progress(f) => *fraction = f,
                        }
                    }
                }
                if let Some(result) = finished {
                    match result {
                        Ok(path) => {
                            let version = manager::version_of(&path)
                                .unwrap_or_else(|| "unknown".to_string());
                            this.setup = SetupState::Ready { bin: path, version };
                            this.start_daemon(cx);
                        }
                        Err(e) => this.setup = SetupState::Failed(e),
                    }
                }
                cx.notify();
            })
            .ok();
            if done {
                break;
            }
        })
        .detach();
    }

    fn pick_existing(&mut self, cx: &mut Context<Self>) {
        let picked = rfd::FileDialog::new()
            .set_title("选择 rclone 可执行文件")
            .pick_file();
        if let Some(path) = picked {
            match manager::version_of(&path) {
                Some(version) => {
                    let mut cfg = AppConfig::load();
                    cfg.rclone_path = Some(path.clone());
                    cfg.save();
                    self.setup = SetupState::Ready { bin: path, version };
                    self.start_daemon(cx);
                }
                None => {
                    self.setup = SetupState::Failed("所选文件不是有效的 rclone".to_string());
                }
            }
            cx.notify();
        }
    }

    fn start_daemon(&mut self, cx: &mut Context<Self>) {
        let SetupState::Ready { bin, .. } = &self.setup else {
            return;
        };
        let bin = bin.clone();
        self.daemon = Some(DaemonState::Starting);
        cx.notify();
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { daemon::start_and_wait(&bin) })
                .await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(d) => {
                        this.daemon = Some(DaemonState::Running(d));
                        this.reload_remotes(cx);
                    }
                    Err(e) => this.daemon = Some(DaemonState::Failed(e)),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // ---------- 连接 ----------

    fn client(&self) -> Option<RcClient> {
        match &self.active {
            ActiveConn::Local => match &self.daemon {
                Some(DaemonState::Running(d)) => Some(d.client()),
                _ => None,
            },
            ActiveConn::External(name) => self
                .connections
                .iter()
                .find(|c| &c.name == name)
                .map(|c| RcClient::new(&crate::rclone::client::Connection {
                    addr: c.addr.clone(),
                    user: c.user.clone(),
                    pass: c.pass.clone(),
                })),
        }
    }

    fn active_label(&self) -> String {
        self.active.label()
    }

    fn set_active(&mut self, active: ActiveConn, cx: &mut Context<Self>) {
        self.active = active;
        let mut cfg = AppConfig::load();
        cfg.active_connection = match &self.active {
            ActiveConn::Local => None,
            ActiveConn::External(name) => Some(name.clone()),
        };
        cfg.save();
        self.reload_remotes(cx);
        self.ensure_transfer_poll(cx);
        cx.notify();
    }

    /// 拉取 remote 列表（含类型）
    fn reload_remotes(&mut self, cx: &mut Context<Self>) {
        let Some(client) = self.client() else {
            self.remotes = RemotesState::Failed(
                "未连接 rcd（本地托管未就绪或未选择外部连接）".to_string(),
            );
            cx.notify();
            return;
        };
        self.remotes = RemotesState::Loading;
        self.remote_test = None;
        cx.notify();
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            let dump = client.config_dump();
            let result = match dump {
                Ok(map) => {
                    let mut list: Vec<RemoteInfo> = map
                        .into_iter()
                        .map(|(name, entry)| RemoteInfo {
                            name,
                            kind: entry.kind,
                        })
                        .collect();
                    list.sort_by(|a, b| a.name.cmp(&b.name));
                    Ok(list)
                }
                Err(_) => {
                    // 兜底：部分 rc 环境不支持 config/dump
                    client.list_remotes().map(|names| {
                        names
                            .into_iter()
                            .map(|name| RemoteInfo {
                                name,
                                kind: "? ".to_string(),
                            })
                            .collect()
                    })
                }
            };
            this.update(cx, |this, cx| {
                this.remotes = match result {
                    Ok(list) => RemotesState::Loaded(list),
                    Err(e) => RemotesState::Failed(e),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn delete_remote(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(client) = self.client() else {
            return;
        };
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            let _ = cx
                .background_executor()
                .spawn(async move { client.config_delete(&name) })
                .await;
            this.update(cx, |this, cx| {
                this.reload_remotes(cx);
            })
            .ok();
        })
        .detach();
    }

    fn test_remote(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(client) = self.client() else {
            return;
        };
        self.remote_test = Some((name.clone(), Ok("测试中…".to_string())));
        cx.notify();
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            let fs = format!("{name}:");
            let result = cx
                .background_executor()
                .spawn(async move {
                    match client.operations_list(&fs, "") {
                        Ok(list) => Ok(format!("连接正常，根目录 {} 项", list.len())),
                        Err(e) => Err(e),
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                this.remote_test = Some((name.clone(), result));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // ---------- 传输 ----------

    fn submit_transfer(&mut self, kind: &str, src: &str, dst: &str, cx: &mut Context<Self>) {
        let Some(client) = self.client() else {
            return;
        };
        let conn = self.active_label();
        let src = src.to_string();
        let dst = dst.to_string();
        let kind = kind.to_string();
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            let kind2 = kind.clone();
            let src2 = src.clone();
            let dst2 = dst.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    let start = match kind2.as_str() {
                        "move" => client.start_move(&src2, &dst2),
                        "sync" => client.start_sync(&src2, &dst2),
                        _ => client.start_copy(&src2, &dst2),
                    };
                    start
                })
                .await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(jobid) => {
                        this.transfers.push(TransferJob {
                            jobid,
                            kind,
                            src,
                            dst,
                            conn,
                            status: JobUiStatus::Running,
                            stats: CoreStats::default(),
                            error: String::new(),
                        });
                        this.ensure_transfer_poll(cx);
                    }
                    Err(e) => {
                        this.transfers.push(TransferJob {
                            jobid: 0,
                            kind,
                            src,
                            dst,
                            conn,
                            status: JobUiStatus::Failed,
                            stats: CoreStats::default(),
                            error: e,
                        });
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn cancel_job(&mut self, jobid: u64, cx: &mut Context<Self>) {
        if let Some(job) = self.transfers.iter_mut().find(|j| j.jobid == jobid) {
            if job.status == JobUiStatus::Running {
                job.status = JobUiStatus::Stopped;
            }
        }
        let Some(client) = self.client() else {
            cx.notify();
            return;
        };
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            let _ = cx
                .background_executor()
                .spawn(async move { client.job_stop(jobid) })
                .await;
            this.update(cx, |_, _| {}).ok();
        })
        .detach();
        cx.notify();
    }

    fn ensure_transfer_poll(&mut self, cx: &mut Context<Self>) {
        if self.transfers_polling {
            return;
        }
        self.transfers_polling = true;
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            'poll: loop {
                let snapshot = this
                    .update(cx, |this, _| {
                        (this.active_label(), this.client(), this.running_jobids())
                    })
                    .ok()
                    .map(|(l, c, ids)| (l, c, ids));
                let Some((conn, Some(client), ids)) = snapshot else {
                    break 'poll;
                };
                if ids.is_empty() {
                    break 'poll;
                }
                // 并发抓取每个运行中任务的状态
                let mut updates: Vec<(u64, Result<CoreStats, String>, Result<_, String>)> =
                    Vec::new();
                for id in ids {
                    let c = client.clone();
                    let g = format!("job/{id}");
                    let stats = cx
                        .background_executor()
                        .spawn(async move { c.core_stats(&g) })
                        .await;
                    let c = client.clone();
                    let status = cx
                        .background_executor()
                        .spawn(async move { c.job_status(id) })
                        .await;
                    updates.push((id, stats, status));
                }
                let updated = this
                    .update(cx, |this, cx| {
                        for (id, stats, status) in updates {
                            if let Some(job) =
                                this.transfers.iter_mut().find(|j| j.jobid == id)
                            {
                                if let Ok(s) = stats {
                                    job.stats = s;
                                }
                                if job.status == JobUiStatus::Running {
                                    if let Ok(s) = status {
                                        if s.finished {
                                            job.status = if s.success {
                                                JobUiStatus::Done
                                            } else {
                                                JobUiStatus::Failed
                                            };
                                            job.error = s.error;
                                        }
                                    }
                                }
                            }
                        }
                        if conn != this.active_label() {
                            // 连接已切换，暂停轮询，等待下次触发
                            this.transfers_polling = false;
                            cx.notify();
                            return;
                        }
                        cx.notify();
                    })
                    .ok();
                if updated.is_none() {
                    break 'poll;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(700))
                    .await;
            }
            // 轮询循环结束，复位标志
            this.update(cx, |this, cx| {
                this.transfers_polling = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn clear_finished_transfers(&mut self, cx: &mut Context<Self>) {
        self.transfers
            .retain(|j| j.status == JobUiStatus::Running);
        cx.notify();
    }

    fn running_jobids(&self) -> Vec<u64> {
        self.transfers
            .iter()
            .filter(|j| {
                j.status == JobUiStatus::Running && j.conn == self.active_label()
            })
            .map(|j| j.jobid)
            .collect()
    }
}

// ---------- 存储添加向导 ----------

impl RootView {
    fn open_wizard(&mut self, cx: &mut Context<Self>) {
        let Some(client) = self.client() else {
            self.remotes = RemotesState::Failed("未连接 rcd，无法打开存储向导".to_string());
            cx.notify();
            return;
        };
        let name_input = cx.new(|cx| TextInput::new(cx, "存储名称（如 myremote）"));
        self.wizard = Some(WizardState {
            providers: ProvidersLoad::Loading,
            show_all: false,
            selected: None,
            name_input,
            fields: Vec::new(),
            show_advanced: false,
            busy: false,
            message: None,
        });
        cx.notify();
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { client.config_providers() })
                .await;
            this.update(cx, |this, cx| {
                if let Some(w) = this.wizard.as_mut() {
                    w.providers = match result {
                        Ok(list) => ProvidersLoad::Loaded(list),
                        Err(e) => ProvidersLoad::Failed(e),
                    };
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn close_wizard(&mut self, cx: &mut Context<Self>) {
        self.wizard = None;
        self.reload_remotes(cx);
        cx.notify();
    }

    fn wizard_select_provider(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(provider) = self.wizard_provider(&name) else {
            return;
        };
        let fields = provider
            .options
            .iter()
            .map(|opt| FieldEntry {
                option: opt.clone(),
                input: if opt.is_password || opt.sensitive {
                    cx.new(|cx| TextInput::masked(cx, opt.help_or_placeholder()))
                } else {
                    cx.new(|cx| TextInput::new(cx, opt.help_or_placeholder()))
                },
            })
            .collect();
        self.wizard.as_mut().map(|w| {
            w.selected = Some(name);
            w.fields = fields;
            w.show_advanced = false;
            w.message = None;
        });
        cx.notify();
    }

    fn wizard_provider(&self, name: &str) -> Option<crate::rclone::client::Provider> {
        match &self.wizard {
            Some(w) => match &w.providers {
                ProvidersLoad::Loaded(list) => list.iter().find(|p| p.name == name).cloned(),
                _ => None,
            },
            None => None,
        }
    }

    fn wizard_toggle_advanced(&mut self, cx: &mut Context<Self>) {
        if let Some(w) = self.wizard.as_mut() {
            w.show_advanced = !w.show_advanced;
        }
        cx.notify();
    }

    fn wizard_create(&mut self, cx: &mut Context<Self>) {
        // 阶段 1：取数据并校验（结束后释放对 wizard 的借用）
        let Some(w) = self.wizard.as_mut() else {
            return;
        };
        if w.busy {
            return;
        }
        let name = w.name_input.read(cx).content();
        let Some(kind) = w.selected.clone() else {
            w.message = Some((false, "请先选择存储类型".to_string()));
            cx.notify();
            return;
        };
        if name.is_empty() {
            w.message = Some((false, "存储名称不能为空".to_string()));
            w.name_input.update(cx, |i, _| i.invalid = true);
            cx.notify();
            return;
        }
        // 收集已填写的参数（空值不发送，交由 rclone 用默认值）
        let params: std::collections::BTreeMap<String, String> = w
            .fields
            .iter()
            .filter_map(|f| {
                let v = f.input.read(cx).content();
                (!v.is_empty()).then_some((f.option.name.clone(), v))
            })
            .collect();

        // 阶段 2：检查连接
        let Some(client) = self.client() else {
            if let Some(w) = self.wizard.as_mut() {
                w.message = Some((false, "未连接 rcd，无法创建".to_string()));
            }
            cx.notify();
            return;
        };
        if let Some(w) = self.wizard.as_mut() {
            w.busy = true;
            w.message = None;
        }
        cx.notify();
        let name2 = name.clone();
        let kind2 = kind.clone();
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    client
                        .config_create(&name2, &kind2, &params)
                        .map_err(|e| format!("创建失败: {e}"))?;
                    // 创建成功后测试连接
                    let fs = format!("{name2}:");
                    match client.operations_list(&fs, "") {
                        Ok(list) => Ok(format!("创建成功，连接正常（根目录 {} 项）", list.len())),
                        Err(e) => Ok(format!("已创建，但连接测试失败: {e}")),
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                if let Some(w) = this.wizard.as_mut() {
                    w.busy = false;
                    w.message = Some(match result {
                        Ok(text) => (true, text),
                        Err(e) => (false, e),
                    });
                }
                this.reload_remotes(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

// ---------- 文件浏览器 ----------

impl RootView {
    fn browser_remote_name(&mut self, name: Option<String>, cx: &mut Context<Self>) {
        self.browser.remote_name = name;
        self.browser.remote_path = String::new();
        self.browser.remote_selected = None;
        self.browser.notice = None;
        self.load_remote_pane(cx);
        cx.notify();
    }

    fn load_local_pane(&mut self, cx: &mut Context<Self>) {
        let Some(client) = self.client() else {
            self.browser.local_entries = PaneLoad::Failed("未连接 rcd".to_string());
            cx.notify();
            return;
        };
        let path = self.browser.local_path.clone();
        self.browser.local_entries = PaneLoad::Loading;
        cx.notify();
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { client.operations_list(&path, "") })
                .await;
            this.update(cx, |this, cx| {
                this.browser.local_entries = match result {
                    Ok(list) => PaneLoad::Loaded(sort_entries(list)),
                    Err(e) => PaneLoad::Failed(e),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn load_remote_pane(&mut self, cx: &mut Context<Self>) {
        let Some(client) = self.client() else {
            self.browser.remote_entries = PaneLoad::Failed("未连接 rcd".to_string());
            cx.notify();
            return;
        };
        let Some(name) = self.browser.remote_name.clone() else {
            self.browser.remote_entries = PaneLoad::Loaded(Vec::new());
            cx.notify();
            return;
        };
        let fs = format!("{name}:");
        let remote = self.browser.remote_path.clone();
        self.browser.remote_entries = PaneLoad::Loading;
        cx.notify();
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { client.operations_list(&fs, &remote) })
                .await;
            this.update(cx, |this, cx| {
                this.browser.remote_entries = match result {
                    Ok(list) => PaneLoad::Loaded(sort_entries(list)),
                    Err(e) => PaneLoad::Failed(e),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn browser_enter_local(&mut self, name: String, cx: &mut Context<Self>) {
        self.browser.local_path = util::join_path(&self.browser.local_path, &name);
        self.browser.local_selected = None;
        self.load_local_pane(cx);
        cx.notify();
    }

    fn browser_up_local(&mut self, cx: &mut Context<Self>) {
        if let Some(parent) = parent_path(&self.browser.local_path) {
            self.browser.local_path = parent;
            self.browser.local_selected = None;
            self.load_local_pane(cx);
            cx.notify();
        }
    }

    fn browser_enter_remote(&mut self, name: String, cx: &mut Context<Self>) {
        self.browser.remote_path = util::join_path(&self.browser.remote_path, &name);
        self.browser.remote_selected = None;
        self.load_remote_pane(cx);
        cx.notify();
    }

    fn browser_up_remote(&mut self, cx: &mut Context<Self>) {
        if let Some(parent) = parent_path(&self.browser.remote_path) {
            self.browser.remote_path = parent;
            self.browser.remote_selected = None;
            self.load_remote_pane(cx);
            cx.notify();
        }
    }

    /// 发起浏览页传输。local_to_remote=true 时从本地→远端，反之反向。
    fn browser_transfer(&mut self, kind: &str, local_to_remote: bool, cx: &mut Context<Self>) {
        if self.client().is_none() {
            self.browser.notice = Some("未连接 rcd，无法传输".to_string());
            cx.notify();
            return;
        }
        // 选中项所在 Pane 的信息
        let (src, dst) = if local_to_remote {
            let Some(sel) = self.browser.local_selected.clone() else {
                self.browser.notice = Some("请先在左侧选择要传输的文件或目录".to_string());
                cx.notify();
                return;
            };
            let is_dir = self
                .browser
                .local_entries
                .pane_dir(&sel)
                .unwrap_or(false);
            let Some(remote) = self.browser.remote_name.clone() else {
                self.browser.notice = Some("请先在上方选择远端存储".to_string());
                cx.notify();
                return;
            };
            let src = util::join_path(&self.browser.local_path, &sel);
            let base = format!("{}:{}", remote, self.browser.remote_path);
            let dst = if is_dir {
                util::join_path(&base, &sel)
            } else {
                base
            };
            (src, dst)
        } else {
            let Some(sel) = self.browser.remote_selected.clone() else {
                self.browser.notice = Some("请先在右侧选择要传输的文件或目录".to_string());
                cx.notify();
                return;
            };
            let is_dir = self
                .browser
                .remote_entries
                .pane_dir(&sel)
                .unwrap_or(false);
            let Some(remote) = self.browser.remote_name.clone() else {
                self.browser.notice = Some("请先在上方选择远端存储".to_string());
                cx.notify();
                return;
            };
            let base = format!("{}:{}", remote, self.browser.remote_path);
            let src = util::join_path(&base, &sel);
            let dst = if is_dir {
                util::join_path(&self.browser.local_path, &sel)
            } else {
                self.browser.local_path.clone()
            };
            (src, dst)
        };
        self.browser.notice = None;
        self.submit_transfer(kind, &src, &dst, cx);
    }
}

fn sort_entries(mut list: Vec<DirEntry>) -> Vec<DirEntry> {
    list.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
    list
}

fn parent_path(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches('/');
    let (parent, _name) = trimmed.rfind('/').map(|i| {
        let (p, n) = trimmed.split_at(i);
        (p.to_string(), n.to_string())
    })?;
    if parent.is_empty() {
        Some("/".to_string())
    } else {
        Some(parent)
    }
}

impl PaneLoad {
    fn pane_dir(&self, name: &str) -> Option<bool> {
        match self {
            PaneLoad::Loaded(list) => list.iter().find(|e| e.name == name).map(|e| e.is_dir),
            _ => None,
        }
    }
}

// ---------- 日志 ----------

fn rcd_log_path() -> std::path::PathBuf {
    crate::config::data_dir().join("logs").join("rcd.log")
}

impl RootView {
    fn start_logs_stream(&mut self, cx: &mut Context<Self>) {
        if self.page != Page::Logs || self.logs.streaming {
            return;
        }
        self.logs.streaming = true;
        self.logs.lines = crate::util::read_log_tail(&rcd_log_path());
        self.app_logs = crate::util::read_log_tail(&crate::applog::log_path());
        cx.notify();
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(1500))
                    .await;
                let should_stop = this
                    .update(cx, |this, cx| {
                        if this.page != Page::Logs {
                            this.logs.streaming = false;
                            cx.notify();
                            return true;
                        }
                        this.logs.lines = crate::util::read_log_tail(&rcd_log_path());
                        this.app_logs = crate::util::read_log_tail(&crate::applog::log_path());
                        cx.notify();
                        false
                    })
                    .ok()
                    .unwrap_or(true);
                if should_stop {
                    break;
                }
            }
        })
        .detach();
    }
}

// ---------- 外部连接表单 ----------

impl RootView {
    fn open_conn_form(&mut self, editing: Option<usize>, cx: &mut Context<Self>) {
        let prefill = editing.and_then(|i| self.connections.get(i).cloned());
        let name = cx.new(|cx| TextInput::new(cx, "连接名称"));
        let addr = cx.new(|cx| TextInput::new(cx, "地址，如 192.168.1.10:5572 或 http://nas:5572"));
        let user = cx.new(|cx| TextInput::new(cx, "用户名（留空则无）"));
        let pass = cx.new(|cx| TextInput::masked(cx, "密码"));
        if let Some(c) = &prefill {
            name.update(cx, |i, cx| i.set_content(&c.name, cx));
            addr.update(cx, |i, cx| i.set_content(&c.addr, cx));
            user.update(cx, |i, cx| i.set_content(&c.user, cx));
            pass.update(cx, |i, cx| i.set_content(&c.pass, cx));
        }
        self.conn_form = Some(ConnForm {
            name,
            addr,
            user,
            pass,
            editing,
            message: None,
        });
        self.modal = ModalState::ConnForm;
        cx.notify();
    }

    fn save_conn(&mut self, cx: &mut Context<Self>) {
        let Some(form) = self.conn_form.as_mut() else {
            return;
        };
        let name = form.name.read(cx).content();
        let addr = form.addr.read(cx).content();
        let user = form.user.read(cx).content();
        let pass = form.pass.read(cx).content();
        if name.is_empty() || addr.is_empty() {
            form.message = Some((false, "名称和地址不能为空".to_string()));
            cx.notify();
            return;
        }
        let editing = form.editing;
        let mut cfg = AppConfig::load();
        match editing {
            Some(i) => {
                if let Some(c) = self.connections.get_mut(i) {
                    c.name = name.clone();
                    c.addr = addr.clone();
                    c.user = user.clone();
                    c.pass = pass.clone();
                }
            }
            None => self.connections.push(ConnectionProfile {
                name: name.clone(),
                addr: addr.clone(),
                user: user.clone(),
                pass: pass.clone(),
            }),
        }
        cfg.connections = self.connections.clone();
        cfg.save();
        self.conn_form = None;
        self.modal = ModalState::None;
        cx.notify();
    }

    fn delete_conn(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(c) = self.connections.get(index) {
            if self.active == ActiveConn::External(c.name.clone()) {
                self.active = ActiveConn::Local;
                self.reload_remotes(cx);
            }
        }
        self.connections.remove(index);
        let mut cfg = AppConfig::load();
        cfg.connections = self.connections.clone();
        cfg.save();
        cx.notify();
    }

    fn test_conn(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(c) = self.connections.get(index).cloned() else {
            return;
        };
        self.conn_test = Some((c.name.clone(), Ok("测试中…".to_string())));
        cx.notify();
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            let client = RcClient::new(&crate::rclone::client::Connection {
                addr: c.addr.clone(),
                user: c.user.clone(),
                pass: c.pass.clone(),
            });
            let result = cx
                .background_executor()
                .spawn(async move {
                    match client.core_version() {
                        Ok(v) => Ok(format!("连接正常 · rclone {}", v.version)),
                        Err(e) => Err(e),
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                this.conn_test = Some((c.name.clone(), result));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

// ---------- 渲染 ----------

impl RootView {
    fn render_onboarding(&self, cx: &mut Context<Self>) -> Div {
        // 每个状态独立的 标题/副标题/正文，避免文案与当前阶段不符
        let (title, subtitle, body) = match &self.setup {
            SetupState::Checking => (
                "欢迎使用 Rclone GUI",
                None,
                muted("正在检测本机 rclone…").into_any_element(),
            ),
            SetupState::Missing => (
                "未检测到 rclone",
                Some("Rclone GUI 依赖 rclone 工作，它是一款命令行云存储同步工具。"),
                self.onboarding_buttons(cx).into_any_element(),
            ),
            SetupState::Downloading { fraction, note } => (
                "正在安装 rclone",
                Some("首次使用需要下载 rclone，完成后将自动进入主界面。"),
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .items_center()
                    .child(muted(note.clone()))
                    .child(
                        div()
                            .w(px(360.))
                            .h(px(6.))
                            .rounded_full()
                            .bg(color(0x27272a))
                            .child(
                                div()
                                    .h_full()
                                    .rounded_full()
                                    .bg(color(ACCENT))
                                    .w(relative(*fraction)),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(color(TEXT_MUTED))
                            .child(format!("{:.0}%", fraction * 100.0)),
                    )
                    .into_any_element(),
            ),
            SetupState::Failed(e) => (
                "安装未完成",
                None,
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .text_color(color(ERROR))
                            .text_center()
                            .child(st(e.clone())),
                    )
                    .child(self.onboarding_buttons(cx))
                    .into_any_element(),
            ),
            SetupState::Ready { .. } => (
                "准备就绪",
                None,
                muted("正在启动…").into_any_element(),
            ),
        };

        let mut panel = div()
            .flex()
            .flex_col()
            .gap_3()
            .items_center()
            .max_w(px(480.))
            .child(div().text_xl().child(st(title)));
        if let Some(sub) = subtitle {
            panel = panel.child(muted(sub).text_center());
        }
        panel = panel.child(body);

        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(panel)
    }

    fn onboarding_buttons(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .items_center()
            .child(
                primary_button("btn-download", "自动下载安装（推荐）").on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| this.start_download(cx)),
                ),
            )
            .child(
                secondary_button("btn-pick", "选择已安装的 rclone").on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| this.pick_existing(cx)),
                ),
            )
            .child(
                secondary_button("btn-recheck", "重新检测").on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.setup = SetupState::Checking;
                        cx.notify();
                        this.detect(cx);
                    }),
                ),
            )
            .child(muted("也可以先自行安装 rclone（https://rclone.org），再点击重新检测。"))
    }

    fn nav_item(&self, page: Page, cx: &mut Context<Self>) -> Stateful<Div> {
        let active = self.page == page;
        let mut item = div()
            .id(page.id())
            .px_2()
            .py_1p5()
            .rounded_md()
            .text_sm()
            .cursor_pointer()
            .child(st(page.label()));
        if active {
            item = item.bg(color(HOVER)).text_color(color(TEXT));
        } else {
            item = item
                .text_color(color(TEXT_MUTED))
                .hover(|s| s.bg(color(HOVER)));
        }
        item.on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| {
                // 向导打开时点击菜单：关闭向导并切换页面
                if this.wizard.is_some() {
                    this.close_wizard(cx);
                }
                this.page = page;
                if page == Page::Logs {
                    this.start_logs_stream(cx);
                }
                if page == Page::Browser {
                    this.load_local_pane(cx);
                    this.load_remote_pane(cx);
                }
                cx.notify();
            }),
        )
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> Div {
        let mut sidebar = div()
            .w(px(200.))
            .h_full()
            .bg(color(PANEL))
            .border_r_1()
            .border_color(color(BORDER))
            .p_3()
            .flex()
            .flex_col()
            .gap_1()
            .child(div().text_lg().mb_2().child(st("Rclone GUI")));

        // 连接列表
        let local_active = self.active == ActiveConn::Local;
        sidebar = sidebar.child(
            div()
                .id("conn-local")
                .px_2()
                .py_1p5()
                .rounded_md()
                .text_sm()
                .cursor_pointer()
                .flex()
                .gap_2()
                .items_center()
                .child(status_dot(
                    matches!(self.daemon, Some(DaemonState::Running(_))),
                ))
                .child(st("本地（托管）"))
                .when(local_active, |d| d.bg(color(HOVER)).text_color(color(TEXT)))
                .when(!local_active, |d| {
                    d.text_color(color(TEXT_MUTED)).hover(|s| s.bg(color(HOVER)))
                })
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        if this.active != ActiveConn::Local {
                            this.set_active(ActiveConn::Local, cx);
                        }
                    }),
                ),
        );
        for (i, c) in self.connections.iter().enumerate() {
            let name = c.name.clone();
            let active = self.active == ActiveConn::External(name.clone());
            let mut item = div()
                .id(ElementId::Name(format!("conn-{i}").into()))
                .px_2()
                .py_1p5()
                .rounded_md()
                .text_sm()
                .cursor_pointer()
                .child(st(c.name.clone()));
            if active {
                item = item.bg(color(HOVER)).text_color(color(TEXT));
            } else {
                item = item
                    .text_color(color(TEXT_MUTED))
                    .hover(|s| s.bg(color(HOVER)));
            }
            let name2 = name.clone();
            sidebar = sidebar.child(
                item.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        if this.active != ActiveConn::External(name2.clone()) {
                            this.set_active(ActiveConn::External(name2.clone()), cx);
                        }
                    }),
                ),
            );
        }

        sidebar = sidebar
            .child(div().flex_1())
            .child(
                secondary_button("btn-add-conn", "＋ 添加连接").on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.open_conn_form(None, cx);
                        this.page = Page::Settings;
                        cx.notify();
                    }),
                ),
            )
            .child(self.nav_item(Page::Dashboard, cx))
            .child(self.nav_item(Page::Remotes, cx))
            .child(self.nav_item(Page::Browser, cx))
            .child(self.nav_item(Page::Transfers, cx))
            .child(self.nav_item(Page::Logs, cx))
            .child(self.nav_item(Page::Settings, cx));

        sidebar
    }

    fn render_statusbar(&self) -> Div {
        let version = match &self.setup {
            SetupState::Ready { version, .. } => format!("rclone {version}"),
            _ => "rclone 不可用".to_string(),
        };
        let conn = match &self.active {
            ActiveConn::Local => match &self.daemon {
                Some(DaemonState::Running(d)) => format!("本地托管 rcd · {}", d.conn.addr),
                Some(DaemonState::Starting) => "本地 rcd 启动中…".to_string(),
                Some(DaemonState::Failed(_)) => "本地 rcd 未运行".to_string(),
                None => "本地 rcd 未启动".to_string(),
            },
            ActiveConn::External(name) => format!("外部连接 · {name}"),
        };
        div()
            .w_full()
            .px_3()
            .py_1()
            .bg(color(PANEL))
            .border_t_1()
            .border_color(color(BORDER))
            .flex()
            .justify_between()
            .text_xs()
            .text_color(color(TEXT_MUTED))
            .child(st(version))
            .child(st(conn))
    }

    fn render_main(&mut self, cx: &mut Context<Self>) -> Div {
        let content: AnyElement = match self.page {
            Page::Dashboard => views::dashboard::render(self, cx).into_any_element(),
            Page::Remotes => views::remotes::render(self, cx).into_any_element(),
            Page::Browser => views::browser::render(self, cx).into_any_element(),
            Page::Transfers => views::transfers::render(self, cx).into_any_element(),
            Page::Logs => views::logs::render(self, cx).into_any_element(),
            Page::Settings => views::settings::render(self, cx).into_any_element(),
        };

        // 内容区：页面 + 可选的向导模态层
        let mut content_area = div()
            .flex_1()
            .h_full()
            .child(
                div()
                    .size_full()
                    .p_4()
                    .id("content-scroll")
                    .overflow_y_scroll()
                    .child(content),
            );
        if self.wizard.is_some() {
            content_area = content_area.child(
                div()
                    .id("wizard-backdrop") // 需要 id 以拦截对下层内容的点击
                    .absolute()
                    .inset_0()
                    .bg(rgba(0x000000_99))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .id("wizard-panel")
                            .w(px(640.))
                            .max_h_full()
                            .overflow_y_scroll()
                            .bg(color(BG))
                            .border_1()
                            .border_color(color(BORDER))
                            .rounded_lg()
                            .p_5()
                            .child(views::remote_form::render(self, cx)),
                    ),
            );
        }
        // 模态对话框（确认/表单）在最上层
        if let Some(overlay) = views::modal::render(self, cx) {
            content_area = content_area.child(overlay);
        }

        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .flex()
                    .overflow_hidden()
                    .child(self.render_sidebar(cx))
                    .child(content_area),
            )
            .child(self.render_statusbar())
    }
}

impl Focusable for RootView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for RootView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 每帧重建文本注册表（paint 阶段会重新填充）
        crate::stext::clear_registry();

        let root = div()
            .id("root")
            .track_focus(&self.focus_handle)
            .font_family(FONT_FAMILY)
            .size_full()
            .flex()
            .flex_col()
            .bg(color(BG))
            .text_color(color(TEXT))
            .text_base()
            // 全局文本选择：按下开始、拖动更新、抬起结算
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, e: &MouseDownEvent, _, cx| {
                    crate::stext::begin_selection(e.position);
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|_, e: &MouseMoveEvent, _, cx| {
                if crate::stext::update_selection(e.position) {
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| {
                    crate::stext::end_selection();
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| {
                    crate::stext::end_selection();
                    cx.notify();
                }),
            )
            // 焦点不在输入框时，Cmd/Ctrl+C 复制全局选区
            .on_action(cx.listener(|_, _: &crate::text_input::Copy, _, cx| {
                crate::stext::copy_selection_to_clipboard(cx);
            }))
            .on_action(cx.listener(|_, _: &crate::stext::ClearSelection, _, cx| {
                crate::stext::clear_selection();
                cx.notify();
            }));

        if matches!(self.setup, SetupState::Ready { .. }) {
            root.child(self.render_main(cx))
        } else {
            root.child(self.render_onboarding(cx))
        }
    }
}
