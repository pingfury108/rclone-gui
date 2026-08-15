//! 设置：rclone 路径 + 外部连接管理

use crate::app::{ActiveConn, ModalState, RootView, SetupState};
use crate::config::data_dir;
use crate::theme::*;
use gpui::{prelude::*, *};

pub fn render(root: &RootView, cx: &mut Context<RootView>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_6()
        .child(render_rclone_section(root, cx))
        .child(render_connections_section(root, cx))
}

fn render_rclone_section(root: &RootView, cx: &mut Context<RootView>) -> Div {
    let (version, bin) = match &root.setup {
        SetupState::Ready { bin, version } => (version.clone(), bin.display().to_string()),
        _ => ("unknown".to_string(), "-".to_string()),
    };
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(page_title("rclone").mb_0())
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(muted(format!("版本：{version}")))
                .child(muted(format!("路径：{bin}"))),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .child(
                    secondary_button("btn-repick", "重新选择 rclone…").on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| this.pick_existing(cx)),
                    ),
                )
                .child(
                    secondary_button("btn-redetect", "重新检测").on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.setup = SetupState::Checking;
                            this.detect(cx);
                        }),
                    ),
                ),
        )
        .child(
            muted(format!(
                "托管二进制与日志目录：{}",
                data_dir().display()
            )),
        )
}

fn render_connections_section(root: &RootView, cx: &mut Context<RootView>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(page_title("外部连接").mb_0())
        .child(
            muted(
                "可添加已运行 `rclone rcd` 的服务器（如 NAS），用本应用管理那台机器的 rclone。",
            ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .children(
                    root.connections
                        .iter()
                        .enumerate()
                        .map(|(i, c)| connection_row(i, c, root, cx)),
                ),
        )
        .child(
            secondary_button("btn-add-conn-form", "＋ 添加连接").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.open_conn_form(None, cx)),
            ),
        )
}

fn connection_row(
    index: usize,
    conn: &crate::config::ConnectionProfile,
    root: &RootView,
    cx: &mut Context<RootView>,
) -> Div {
    let active = root.active == ActiveConn::External(conn.name.clone());
    let name = conn.name.clone();

    // 测试结果展示
    let mut test_result = None;
    if let Some((n, res)) = &root.conn_test {
        if n == &name {
            test_result = Some(match res {
                Ok(text) => (color(SUCCESS), text.clone()),
                Err(e) => (color(ERROR), e.clone()),
            });
        }
    }

    let mut row = div()
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
                .child(
                    div()
                        .text_sm()
                        .text_color(color(if active { ACCENT } else { TEXT }))
                        .child(format!("{} · {}", conn.name, conn.addr)),
                )
                .child(if active {
                    div()
                        .text_xs()
                        .text_color(color(ACCENT))
                        .child("（使用中）")
                } else {
                    div()
                })
                .child(div().flex_1())
                .child(
                    secondary_button("btn-conn-use", "使用").on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.set_active(ActiveConn::External(name.clone()), cx);
                        }),
                    ),
                )
                .child(
                    secondary_button("btn-conn-test", "测试").on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| this.test_conn(index, cx)),
                    ),
                )
                .child(
                    secondary_button("btn-conn-edit", "编辑").on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| this.open_conn_form(Some(index), cx)),
                    ),
                )
                .child(
                    secondary_button("btn-conn-del", "删除").on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.modal = ModalState::ConfirmDeleteConn(index);
                            cx.notify();
                        }),
                    ),
                ),
        );
    if let Some((c, text)) = test_result {
        row = row.child(div().text_xs().text_color(c).child(text));
    }
    row
}

/// 连接表单内容（由模态面板包装）
pub fn conn_form_content(root: &RootView, cx: &mut Context<RootView>) -> Div {
    let Some(form) = &root.conn_form else {
        return div();
    };
    let mut panel = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(modal_title(if form.editing.is_some() {
            "编辑连接"
        } else {
            "添加连接"
        }))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(crate::text_input::form_label("名称"))
                .child(form.name.clone()),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(crate::text_input::form_label("地址"))
                .child(form.addr.clone()),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(crate::text_input::form_label("用户名"))
                .child(form.user.clone()),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(crate::text_input::form_label("密码"))
                .child(form.pass.clone()),
        );

    if let Some((ok, text)) = &form.message {
        panel = panel.child(
            div()
                .text_sm()
                .text_color(color(if *ok { SUCCESS } else { ERROR }))
                .child(text.clone()),
        );
    }
    panel = panel.child(
        div()
            .flex()
            .gap_2()
            .child(
                primary_button("btn-conn-save", "保存").on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| this.save_conn(cx)),
                ),
            )
            .child(
                secondary_button("btn-conn-cancel", "取消").on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.conn_form = None;
                        this.modal = ModalState::None;
                        cx.notify();
                    }),
                ),
            ),
    );
    panel
}
