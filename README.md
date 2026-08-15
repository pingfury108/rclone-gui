# rclone-gui

跨平台 rclone 图形界面，基于 GPUI（Zed 编辑器的 UI 框架）构建。
所有与 rclone 的交互都通过官方 Remote Control (rc) HTTP API 完成，不解析命令行输出。

## 功能

- **零配置起步**：未安装 rclone 时可一键自动下载安装（多下载源自动切换，国内可用）；本地 `rclone rcd` 自动托管，用户无感知
- **存储管理**：常用类型模板（HTTP / WebDAV / S3 / SFTP / FTP / 本地）+ 全部 69 种后端的动态表单（由 `config/providers` 驱动），创建后自动测试连接
- **文件浏览器**：本地 ↔ 远端双栏，复制 / 移动传输
- **传输任务**：实时进度条、速度、ETA、取消
- **外部连接**：添加运行 `rclone rcd` 的服务器（NAS 等），一套 UI 管理多台机器
- **日志**：应用日志 + rcd 日志，自动刷新
- **全局文本选择**：任意文本可拖选，Cmd/Ctrl+C 复制
- 深色 Zed 风格 UI，内嵌中文字体（Noto Sans SC），支持中文输入法

设计文档见 [SPEC.md](SPEC.md)。

## 安装

### 下载预编译包

从 [Releases](../../releases) 下载对应平台压缩包，解压即用：

| 平台 | 文件 |
|---|---|
| macOS (Apple Silicon) | `rclone-gui-macos-arm64.tar.gz` |
| macOS (Intel) | `rclone-gui-macos-x86_64.tar.gz` |
| Windows | `rclone-gui-windows-x86_64.zip` |
| Linux | `rclone-gui-linux-x86_64.tar.gz` |

首次启动会自动下载安装 rclone（也可选择已有的 rclone）。

### 从源码构建

```bash
cargo build --release
```

**系统要求**：macOS 11+ / Windows 10+ / 主流 Linux 桌面（X11 或 Wayland）

- macOS 需要 Metal Toolchain：`xcodebuild -downloadComponent MetalToolchain`
- Linux 需要系统依赖：
  ```bash
  sudo apt install libxcb1-dev libxcb-xkb-dev libxkbcommon-dev libxkbcommon-x11-dev \
    libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
    libwayland-dev libfontconfig-dev libfreetype-dev pkg-config clang
  ```

## 使用：管理远程服务器上的 rclone

1. 在服务器上运行：
   ```bash
   rclone rcd --rc-addr :5572 --rc-user admin --rc-pass 你的密码
   ```
2. 本应用侧边栏「＋ 添加连接」，填入地址 `服务器IP:5572` 与账号密码
3. 点击连接名即可在 本地/多台服务器 之间切换管理

## 发布流程（维护者）

推送 tag 触发 GitHub Actions，自动构建四个平台包并附加到 Release：

```bash
git tag v0.1.0 && git push origin v0.1.0
```

## 已知限制

- OAuth 类后端（Google Drive、OneDrive 等）暂需先用命令行 `rclone config` 完成授权
- Remote 暂不支持编辑，仅创建/删除
- Linux arm64 暂无预编译包（可自行 `cargo build --release`）

## License

见 [LICENSE](LICENSE)。
