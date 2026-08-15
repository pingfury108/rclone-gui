//! 存储管理：列表 + 测试 + 删除（添加走向导）

use crate::app::{ModalState, RemotesState, RootView};
use crate::stext::st;
use crate::theme::*;
use gpui::{prelude::*, *};

pub fn render(root: &RootView, cx: &mut Context<RootView>) -> Div {
    let header = div()
        .flex()
        .items_center()
        .gap_3()
        .child(page_title("存 储").mb_0())
        .child(
            primary_button("btn-add-remote", "＋ 添加存储").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.open_wizard(cx)),
            ),
        )
        .child(
            secondary_button("btn-refresh-remotes", "刷新").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.reload_remotes(cx)),
            ),
        );

    let body = match &root.remotes {
        RemotesState::Idle | RemotesState::Loading => {
            muted("加载中…").into_any_element()
        }
        RemotesState::Failed(e) => div()
            .text_sm()
            .text_color(color(ERROR))
            .child(st(e.clone()))
            .into_any_element(),
        RemotesState::Loaded(list) if list.is_empty() => {
            muted("还没有配置任何存储，点击「添加存储」开始。").into_any_element()
        }
        RemotesState::Loaded(list) => div()
            .flex()
            .flex_col()
            .gap_3()
            .children(list.iter().map(|r| remote_row(r, root, cx)))
            .into_any_element(),
    };

    div().flex().flex_col().gap_4().child(header).child(body)
}

fn remote_row(
    remote: &crate::app::RemoteInfo,
    root: &RootView,
    cx: &mut Context<RootView>,
) -> Div {
    let name = remote.name.clone();
    let name_test = name.clone();
    let name_del = name.clone();
    let actions = div()
        .flex()
        .gap_2()
        .items_center()
        .child(
            secondary_button("btn-test-remote", "测试").on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| this.test_remote(name_test.clone(), cx)),
            ),
        )
        .child(
            secondary_button("btn-del-remote", "删除").on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.modal = ModalState::ConfirmDeleteRemote(name_del.clone());
                    cx.notify();
                }),
            ),
        );

    // 测试结果
    let mut test_result = None;
    if let Some((n, res)) = &root.remote_test {
        if n == &name {
            test_result = Some(match res {
                Ok(text) => (color(SUCCESS), text.clone()),
                Err(e) => (color(ERROR), e.clone()),
            });
        }
    }

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
                .child(div().text_sm().child(st(remote.name.clone())))
                .child(badge(&remote.kind))
                .child(div().flex_1())
                .child(actions),
        );
    if let Some((color, text)) = test_result {
        card = card.child(div().text_xs().text_color(color).child(st(text)));
    }
    card
}

/// 类型徽标
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
