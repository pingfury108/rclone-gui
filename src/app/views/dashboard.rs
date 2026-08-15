//! 仪表盘：连接与运行状态总览

use crate::app::{ActiveConn, DaemonState, RemotesState, RootView, SetupState};
use crate::stext::st;
use crate::theme::*;
use gpui::{prelude::*, *};

pub fn render(root: &RootView, cx: &mut Context<RootView>) -> Div {
    let (version, bin) = match &root.setup {
        SetupState::Ready { bin, version } => (version.clone(), bin.display().to_string()),
        _ => ("unknown".to_string(), "-".to_string()),
    };
    let (rcd_status, rcd_err) = match &root.daemon {
        Some(DaemonState::Running(d)) => (format!("运行中 · {}", d.conn.addr), None),
        Some(DaemonState::Starting) => ("启动中…".to_string(), None),
        Some(DaemonState::Failed(e)) => ("未运行".to_string(), Some(e.clone())),
        None => ("未启动".to_string(), None),
    };
    let remote_count = match &root.remotes {
        RemotesState::Loaded(list) => list.len().to_string(),
        _ => "-".to_string(),
    };
    let conn_label = root.active_label();

    let mut body = div()
        .flex()
        .gap_4()
        .flex_wrap()
        .child(card("当前连接", conn_label))
        .child(card("rclone 版本", version))
        .child(card("rclone 路径", bin))
        .child(card("rcd 状态", rcd_status))
        .child(card("存储数量", remote_count));

    if let Some(err) = rcd_err {
        body = body.child(
            div()
                .flex()
                .gap_3()
                .items_center()
                .child(div().text_sm().text_color(color(ERROR)).child(err))
                .child(
                    secondary_button("btn-rcd-retry", "重试启动").on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| this.start_daemon(cx)),
                    ),
                ),
        );
    }

    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(page_title("仪表盘"))
        .child(body)
        .child(
            div()
                .text_xs()
                .text_color(color(TEXT_MUTED))
                .child(if matches!(root.active, ActiveConn::Local) {
                    "当前使用应用自动托管的本地 rclone 服务；也可在左下角添加外部服务器连接。"
                } else {
                    "当前连接的是外部 rclone 服务器，文件操作将直接在该服务器上执行。"
                }),
        )
}

fn card(title: &str, value: String) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .p_4()
        .rounded_md()
        .border_1()
        .border_color(color(BORDER))
        .min_w(px(200.))
        .child(muted(title.to_string()))
        .child(div().text_sm().child(st(value)))
}
