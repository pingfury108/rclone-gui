//! 模态对话框：确认操作 + 连接表单

use crate::app::{ModalState, RootView};
use crate::theme::*;
use gpui::{prelude::*, *};

pub fn render(root: &RootView, cx: &mut Context<RootView>) -> Option<Stateful<Div>> {
    match &root.modal {
        ModalState::None => None,
        ModalState::ConfirmDeleteRemote(name) => {
            let name = name.clone();
            Some(confirm_overlay(
                "modal-del-remote",
                "删除存储",
                format!("确定删除存储「{name}」？此操作仅删除本地配置，不影响远端数据。"),
                move |this, cx| {
                    this.delete_remote(name.clone(), cx);
                },
                cx,
            ))
        }
        ModalState::ConfirmDeleteConn(index) => {
            let index = *index;
            let name = root
                .connections
                .get(index)
                .map(|c| c.name.clone())
                .unwrap_or_default();
            Some(confirm_overlay(
                "modal-del-conn",
                "删除连接",
                format!("确定删除连接「{name}」？"),
                move |this, cx| {
                    this.delete_conn(index, cx);
                },
                cx,
            ))
        }
        ModalState::ConnForm => Some(modal_overlay(
            "modal-conn-form",
            modal_panel(super::settings::conn_form_content(root, cx)),
        )),
    }
}

/// 通用确认对话框
fn confirm_overlay(
    id: &'static str,
    title: &str,
    message: String,
    on_confirm: impl Fn(&mut RootView, &mut Context<RootView>) + 'static,
    cx: &mut Context<RootView>,
) -> Stateful<Div> {
    modal_overlay(
        id,
        modal_panel(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(modal_title(title.to_string()))
                .child(muted(message))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .justify_end()
                        .child(
                            secondary_button("btn-modal-cancel", "取消").on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.modal = ModalState::None;
                                    cx.notify();
                                }),
                            ),
                        )
                        .child(
                            danger_button("btn-modal-confirm", "确认删除").on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    on_confirm(this, cx);
                                    cx.notify();
                                }),
                            ),
                        ),
                ),
        ),
    )
}
