//! 传输任务：进度条 + 速度 + ETA + 取消

use crate::app::{JobUiStatus, RootView, TransferJob};
use crate::stext::st;
use crate::theme::*;
use crate::util;
use gpui::{prelude::*, *};

pub fn render(root: &RootView, cx: &mut Context<RootView>) -> Div {
    if root.transfers.is_empty() {
        return div()
            .flex()
            .flex_col()
            .gap_3()
            .child(page_title("传 输"))
            .child(muted("暂无传输任务。在「浏览」页选中文件后使用复制/移动按钮发起任务。"));
    }

    let has_finished = root
        .transfers
        .iter()
        .any(|j| j.status != JobUiStatus::Running);
    let mut header = div().flex().items_center().gap_3().child(page_title("传 输").mb_0());
    if has_finished {
        header = header.child(
            secondary_button("btn-clear-finished", "清空已完成").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.clear_finished_transfers(cx)),
            ),
        );
    }

    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(header)
        .children(
            root.transfers
                .iter()
                .rev()
                .map(|job| job_row(job, cx)),
        )
}

fn job_row(job: &TransferJob, cx: &mut Context<RootView>) -> Div {
    let status_text = match job.status {
        JobUiStatus::Running => "进行中",
        JobUiStatus::Done => "已完成",
        JobUiStatus::Stopped => "已取消",
        JobUiStatus::Failed => "失败",
    };
    let status_color = match job.status {
        JobUiStatus::Running => ACCENT,
        JobUiStatus::Done => SUCCESS,
        JobUiStatus::Stopped => TEXT_MUTED,
        JobUiStatus::Failed => ERROR,
    };

    let percent = progress_percent(job.stats.bytes, job.stats.total_bytes);
    let mut card = div()
        .flex()
        .flex_col()
        .gap_2()
        .px_4()
        .py_3()
        .rounded_md()
        .border_1()
        .border_color(color(BORDER))
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(badge(&job.kind))
                .child(
                    div()
                        .flex_1()
                        .text_sm()
                        .child(st(format!("{}  →  {}", job.src, job.dst))),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(color(status_color))
                        .child(st(status_text.to_string())),
                ),
        )
        .child(progress_bar(percent));

    // 进度详情
    let detail = match job.status {
        JobUiStatus::Running => format!(
            "{} / {} · {} · ETA {}",
            util::fmt_size(job.stats.bytes as f64),
            util::fmt_size(job.stats.total_bytes as f64),
            util::fmt_speed(job.stats.speed),
            util::fmt_eta(job.stats.eta),
        ),
        _ => job.error.clone(),
    };
    card = card.child(
        div()
            .flex()
            .items_center()
            .gap_3()
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .text_color(color(TEXT_MUTED))
                    .child(st(detail)),
            )
            .when(job.status == JobUiStatus::Running, |d| {
                d.child({
                    let conn = job.conn.clone();
                    let jobid = job.jobid;
                    secondary_button("btn-cancel-job", "取消").on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            if root_client_matches(this, conn.clone()) {
                                this.cancel_job(jobid, cx);
                            }
                        }),
                    )
                })
            }),
    );
    card
}

/// 仅当任务属于当前连接时才显示取消按钮（避免误操作）
fn root_client_matches(_this: &RootView, job_conn: String) -> bool {
    _this.active_label() == job_conn
}

fn progress_percent(done: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        (done as f32 / total as f32).clamp(0.0, 1.0)
    }
}

fn progress_bar(percent: f32) -> Div {
    div()
        .h(px(6.))
        .w_full()
        .rounded_full()
        .bg(color(0x27272a))
        .child(
            div()
                .h_full()
                .rounded_full()
                .bg(color(ACCENT))
                .w(relative(percent)),
        )
}

fn badge(kind: &str) -> Div {
    div()
        .px_2()
        .py_0p5()
        .rounded_full()
        .bg(color(0x1e3a5f))
        .text_xs()
        .text_color(color(0x93c5fd))
        .child(st(kind.to_string()))
}
