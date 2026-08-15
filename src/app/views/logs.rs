//! 日志页：应用日志（检测/下载/rcd 生命周期）+ rcd 日志

use crate::app::RootView;
use crate::config::data_dir;
use crate::stext::st;
use crate::theme::*;
use gpui::{prelude::*, *};

pub fn render(root: &RootView, cx: &mut Context<RootView>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(page_title("日 志"))
        .child(
            div()
                .text_xs()
                .text_color(color(TEXT_MUTED))
                .child(format!(
                    "日志目录：{}（每 1.5 秒自动刷新）",
                    data_dir().join("logs").display()
                )),
        )
        .child(log_block(
            "app-log-scroll",
            "应用日志（app.log）",
            crate::applog::log_path().display().to_string(),
            &root.app_logs,
        ))
        .child(log_block(
            "rcd-log-scroll",
            "rclone 服务日志（rcd.log）",
            data_dir().join("logs").join("rcd.log").display().to_string(),
            &root.logs.lines,
        ))
        .child(
            secondary_button("btn-log-refresh", "立即刷新").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.logs.lines = crate::util::read_log_tail(
                        &crate::config::data_dir().join("logs").join("rcd.log"),
                    );
                    this.app_logs = crate::util::read_log_tail(&crate::applog::log_path());
                    this.start_logs_stream(cx);
                    cx.notify();
                }),
            ),
        )
}

fn log_block(id: &'static str, title: &str, path: String, lines: &[String]) -> Div {
    let body: Vec<Div> = if lines.is_empty() {
        vec![div()
            .text_xs()
            .text_color(color(TEXT_MUTED))
            .child(st("（暂无日志）"))]
    } else {
        lines
            .iter()
            .map(|line| {
                div()
                    .text_xs()
                    .text_color(color(0xcbd5e1))
                    .child(st(line.clone()))
            })
            .collect()
    };
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .flex()
                .gap_2()
                .items_center()
                .child(div().text_xs().child(title.to_string()))
                .child(
                    div()
                        .text_xs()
                        .text_color(color(TEXT_MUTED))
                        .child(path),
                )
                .child(div().flex_1()),
        )
        .child(
            div()
                .id(id)
                .h(px(160.))
                .overflow_y_scroll()
                .p_2()
                .rounded_md()
                .border_1()
                .border_color(color(BORDER))
                .bg(color(0x0d0d0f))
                .children(body),
        )
}
