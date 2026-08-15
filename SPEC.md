# rclone-gui 设计规格书

> 版本: v0.1 (M0–M1 范围) · UI 框架: GPUI 0.2.2 (Zed)

## 1. 目标与非目标

### 1.1 目标
- 跨平台桌面应用（macOS / Windows / Linux），深色 Zed 风格 UI，完整中文字体显示
- **小白路径零配置**：未安装 rclone 时应用内一键下载安装；本地 rcd 自动托管，用户无感知
- **专家路径不损失能力**：可指定自有 rclone、可连接外部 rcd 服务器、动态表单支持全部 rclone 后端
- 所有与 rclone 的交互走 **Remote Control (rc) HTTP API**，不解析 CLI 文本输出

### 1.2 非目标（本期）
- 不实现 mount 管理（P2）
- 不重新实现 rclone 配置加密（`config/password` 相关）
- 不做移动平台

## 2. 总体架构

```
┌─────────────────────────────────────────────────┐
│                  GPUI App (前端)                 │
│  RootView ──┬── Onboarding（首次设置向导）        │
│             └── Main（侧边栏导航 + 页面 + 状态栏） │
│                      │ cx.spawn + background_executor
├─────────────────────▼───────────────────────────┤
│ rclone::client::RcClient  (HTTP + Basic Auth)   │
├─────────────────────────────────────────────────┤
│ rclone::daemon（本地托管 rcd 子进程）/ 外部 rcd    │
├─────────────────────────────────────────────────┤
│ rclone::manager（二进制检测 / 下载 / 版本管理）    │
└─────────────────────────────────────────────────┘
```

**数据流**：领域状态保存在 `Entity`（RootView）中；耗时操作（检测/下载/HTTP 调用）在
`cx.background_executor()` 执行，结果通过 `WeakEntity::update` 写回并 `cx.notify()` 触发重渲染。

## 3. 核心子系统规格

### 3.1 rclone 二进制管理（`rclone/manager.rs`）

**三级查找**（`detect() -> Option<FoundBinary>`）：
1. `config.rclone_path`（用户在设置中指定）
2. 托管路径：`{data_dir}/rclone-gui/bin/rclone[.exe]`
3. 系统 PATH（直接尝试执行 `rclone version`）

**版本检测**：`rclone version` 首行匹配 `rclone vX.Y.Z`；失败（不存在/无权限/非 rclone）返回 None。

**托管下载**：
- URL: `https://downloads.rclone.org/rclone-current-{os}-{arch}.zip`
  - os: `osx`（macOS）/ `windows` / `linux`；arch: `amd64` / `arm64`
- 下载到内存（上限 300MB）→ zip 中定位 `rclone-current-*/rclone[.exe]` → 写入托管路径
- Unix 下设置 `0o755`；Windows 追加 `.exe`
- M0 进度为不定态（indeterminate），M5 可加百分比

**Windows**：所有子进程调用追加 `CREATE_NO_WINDOW (0x08000000)`，避免弹控制台窗口。

### 3.2 本地 rcd 守护进程（`rclone/daemon.rs`）

```
rclone rcd --rc-addr 127.0.0.1:{随机端口} --rc-user rclone-gui --rc-pass {随机token}
           --log-level NOTICE --log-file {data_dir}/logs/rcd.log
```
- 端口：`TcpListener::bind("127.0.0.1:0")` 取空闲端口（先释放再传给 rclone，接受极小竞争窗口）
- token：进程号 + 系统时间纳秒哈希（本机随机即可，非密码学场景）
- 就绪探测：启动后轮询 `core/version`（100ms × 最多 50 次）
- 生命周期：`Daemon` 实现 `Drop`（kill 子进程）；句柄存于 RootView，应用退出时回收
- 已知限制：极端情况下进程退出未走 Drop 会留下孤儿 rcd（监听 localhost 随机端口，风险低）

### 3.3 RC 客户端（`rclone/client.rs`）

```rust
pub struct RcClient { base_url: String, auth: String /* Basic base64 */, agent: ureq::Agent }
impl RcClient {
    pub fn call<Req: Serialize, Resp: DeserializeOwned>(&self, method: &str, req: &Req) -> Result<Resp, String>;
    pub fn core_version(&self) -> Result<CoreVersion, String>;
    pub fn list_remotes(&self) -> Result<Vec<String>, String>;   // config/listremotes
}
```
- Agent 配置 `http_status_as_error(false)`，手动判断状态码，从响应体解析 rclone 错误
  （rclone rc 错误格式：`{"error": "...", "path": "...", "status": 4xx}`）
- ureq 使用 rustls（无 OpenSSL 跨平台问题）

**本期使用的 rc 端点**：

| 端点 | 用途 | 阶段 |
|---|---|---|
| `core/version` | 连通性/版本探测 | M1 |
| `config/listremotes` | remote 列表 | M1 |
| `config/providers` | 动态表单 schema | M2 |
| `config/create` `config/update` `config/delete` | remote 增改删 | M2 |
| `operations/list` | 文件浏览 | M4 |
| `sync/copy` `sync/move` `sync/sync`（`_async`） | 传输任务 | M3 |
| `core/stats` `job/status` `job/stop` | 进度/取消 | M3 |

### 3.4 配置持久化（`config.rs`）

- 路径：`{config_dir}/rclone-gui/config.json`
- 内容：`{ "rclone_path": "..." | null, "connections": [...] /* M1 外部连接预留 */ }`
- 读取失败一律回退默认配置（容忍损坏，不阻断启动）

### 3.5 中文字体

- 内嵌 `assets/fonts/NotoSansSC-Regular.otf`（`include_bytes!`）
- 启动时 `cx.text_system().add_fonts(...)` 注册，根元素 `.font_family("Noto Sans SC")`
- 失败时静默回退系统字体（macOS PingFang / Windows 雅黑均可显示中文）

## 4. UI 规格

### 4.1 窗口
- 初始 1024×680 居中；关闭最后一个窗口退出应用
- 主题（`theme.rs`）：背景 `#18181b`，侧栏/状态栏 `#111113`，边框 `#27272a`，
  正文 `#e4e4e7`，次要 `#a1a1aa`，强调 `#0ea5e9`，成功 `#22c55e`，错误 `#ef4444`

### 4.2 状态机（RootView）

```
SetupState:  Checking → Missing ⇄ Downloading → Ready{bin, version}
                  ↘ Failed(msg)（可重试）
DaemonState: Starting → Running{handle, conn, client} / Failed(msg)
RemotesState: Loading → Loaded(Vec<String>) / Failed(msg)
```

启动流程：`detect()` → 找到则 `Ready` 并自动 `start_daemon` → 就绪后 `load_remotes`；
未找到则进入 Onboarding。

### 4.3 Onboarding（首次设置向导）
- 标题 + 说明（为什么需要 rclone，一句话）
- 按钮：`自动下载安装（推荐）` / `选择已安装的 rclone`（系统文件对话框 rfd）
- 下载中显示不定态进度；失败显示错误 + 重试
- 成功后自动进入主界面

### 4.4 主界面
```
┌────────────┬──────────────────────────────┐
│ Rclone GUI │  页面内容                     │
│ ● 本地(托管)│                              │
│            │  Dashboard: rclone/rcd 信息卡 │
│ 仪表盘      │  存  储: remote 卡片列表      │
│ 存  储      │  传  输: 占位(M3)            │
│ 传  输      │  设  置: rclone 路径/重选     │
│ 设  置      │                              │
├────────────┴──────────────────────────────┤
│ rclone v1.x · rcd 运行中 127.0.0.1:PORT    │
└───────────────────────────────────────────┘
```
- 侧边栏连接切换器本期仅显示"本地（托管）"，外部连接 UI 在 M1 后续迭代加入
- 「存储」页本期只读列表 + 「添加存储」占位按钮（M2 实现向导）

## 5. 目录与数据布局

```
仓库:
  SPEC.md
  assets/fonts/NotoSansSC-Regular.otf
  src/
    main.rs            入口、字体注册、窗口
    theme.rs           颜色常量 + 通用组件（按钮/卡片/标签）
    config.rs          配置持久化
    app.rs             RootView（状态机 + 导航 + 全部页面渲染）
    rclone/mod.rs
    rclone/manager.rs  二进制检测/下载
    rclone/daemon.rs   本地 rcd 生命周期
    rclone/client.rs   RC HTTP 客户端

用户数据:
  {data_dir}/rclone-gui/bin/rclone[.exe]     托管二进制
  {data_dir}/rclone-gui/logs/rcd.log         rcd 日志
  {config_dir}/rclone-gui/config.json        应用配置
```

## 6. 依赖（pin 版本）

| crate | 版本 | 用途 |
|---|---|---|
| gpui | =0.2.2 | UI（锁精确版本，API 随 Zed 快速变动） |
| serde / serde_json | 1 | rc API 与配置序列化 |
| ureq | 3 (rustls+json) | HTTP 客户端 |
| zip | 8 (仅 deflate) | 解压 rclone 发行包 |
| dirs | 6 | 平台数据/配置目录 |
| base64 | 0.22 | rc Basic Auth 头 |
| rfd | 0.15 | 原生文件选择对话框 |
| anyhow | 1 | 内部错误传递 |

## 7. 里程碑与验收

- **M0 地基**（已完成）：窗口骨架、中文字体、主题、rclone 检测/下载、Onboarding、配置持久化
  - 验收：无 rclone 环境首次启动出现向导，可一键下载并进入主界面
- **M1 连接**（已完成）：本地 rcd 托管、就绪探测、状态栏、存储只读列表
- **M2 存储向导**（已完成）：模板卡片（HTTP/WebDAV/S3/SFTP/FTP/Local）+ `config/providers` 动态表单（必填标记/高级折叠/密码掩码）+ 创建后自动测试连接 + 删除（二次确认）
- **M3 传输**（已完成）：浏览页发起 copy/move、`core/stats?group=job/N` 实时进度/速度/ETA、`job/stop` 取消、700ms 轮询
- **M4 文件浏览器**（已完成）：本地↔远端双栏、点击选中/再点进入目录、面包屑、双方向复制/移动
- **M5 打磨**（已完成大部分）：日志页（1.5s 自动刷新 rcd.log 尾部）、外部连接管理（增删改/测试/切换，持久化）

**已知限制（后续迭代）**
- OAuth 类后端（Google Drive、OneDrive 等）暂不支持在 UI 内完成授权流程，需先用命令行 `rclone config` 配置
- rclone 下载进度为不定态，未显示百分比
- Remote 暂不支持编辑（仅创建/删除；密码类字段回读为 obscured，直接编辑会损坏）
- 目录双击进入改为「单击选中、再次单击进入」（gpui 0.2.2 无 on_double_click）

## 8. 风险与对策

| 风险 | 对策 |
|---|---|
| GPUI API 不稳定 | 锁 `=0.2.2`；升级单独评估 |
| 端口竞争（先释放再绑定） | 竞争窗口极小；失败则重试一次 |
| rcd 孤儿进程 | Drop 回收 + 仅监听 localhost 随机端口，风险可接受 |
| rclone.org 下载被墙/失败 | Onboarding 显示手动安装指引 + 自选路径兜底 |
| 字体注册失败 | 静默回退系统 CJK 字体 |
