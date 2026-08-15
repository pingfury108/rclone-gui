//! 文件浏览器：本地 ↔ 远端双栏

use crate::app::{PaneLoad, RemotesState, RootView};
use crate::rclone::client::DirEntry;
use crate::stext::st;
use crate::theme::*;
use crate::util;
use gpui::{prelude::*, *};

pub fn render(root: &RootView, cx: &mut Context<RootView>) -> Div {
    // h_full + min_h_0：把页面高度钉在可视区内，文件列表内部滚动而非整页滚动
    let mut page = div()
        .h_full()
        .min_h(px(0.))
        .flex()
        .flex_col()
        .gap_3()
        .child(page_title("浏 览"))
        .child(render_remote_selector(root, cx))
        .child(
            div()
                .flex_1()
                .flex()
                .gap_3()
                .min_h(px(0.))
                .child(pane(
                    true,
                    &root.browser.local_path,
                    &root.browser.local_entries,
                    &root.browser.local_selected,
                    root.browser.remote_name.as_deref(),
                    cx,
                ))
                .child(render_toolbar(root, cx))
                .child(pane(
                    false,
                    &format!(
                        "{}:{}",
                        root.browser.remote_name.as_deref().unwrap_or("?"),
                        root.browser.remote_path
                    ),
                    &root.browser.remote_entries,
                    &root.browser.remote_selected,
                    root.browser.remote_name.as_deref(),
                    cx,
                )),
        );
    // 操作提示（如“请先选择文件”）
    if let Some(notice) = &root.browser.notice {
        page = page.child(
            div()
                .text_sm()
                .text_color(color(ACCENT))
                .child(st(notice.clone())),
        );
    }
    page
}

fn render_remote_selector(root: &RootView, cx: &mut Context<RootView>) -> Div {
    let names = match &root.remotes {
        RemotesState::Loaded(list) => list.iter().map(|r| r.name.clone()).collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    div()
        .flex()
        .items_center()
        .gap_2()
        .flex_wrap()
        .child(muted("远端："))
        .children(names.iter().map(|name| {
            let active = root.browser.remote_name.as_deref() == Some(name.as_str());
            let name = name.clone();
            let mut chip = div()
                .id(ElementId::Name(format!("remote-chip-{name}").into()))
                .px_2()
                .py_1()
                .rounded_md()
                .text_sm()
                .cursor_pointer();
            if active {
                chip = chip.bg(color(ACCENT)).text_color(color(0xffffff));
            } else {
                chip = chip
                    .border_1()
                    .border_color(color(BORDER))
                    .text_color(color(TEXT_MUTED))
                    .hover(|s| s.bg(color(HOVER)));
            }
            chip.on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.browser_remote_name(Some(name.clone()), cx)
                }),
            )
        }))
        .child(
            secondary_button("btn-browser-up-remote", "↑ 上级").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.browser_up_remote(cx)),
            ),
        )
}

fn render_toolbar(_root: &RootView, cx: &mut Context<RootView>) -> Div {
    div()
        .flex()
        .flex_col()
        .justify_center()
        .gap_2()
        .w(px(64.))
        .child(
            secondary_button("btn-xfer-copy-l2r", "复制 →").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.browser_transfer("copy", true, cx)),
            ),
        )
        .child(
            secondary_button("btn-xfer-move-l2r", "移动 →").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.browser_transfer("move", true, cx)),
            ),
        )
        .child(
            secondary_button("btn-xfer-copy-r2l", "← 复制").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.browser_transfer("copy", false, cx)),
            ),
        )
        .child(
            secondary_button("btn-xfer-move-r2l", "← 移动").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.browser_transfer("move", false, cx)),
            ),
        )
        .child(
            div()
                .text_xs()
                .text_color(color(TEXT_MUTED))
                .text_center()
                .child("选中文件后\n点击方向"),
        )
}

fn pane(
    is_local: bool,
    title: &str,
    entries: &PaneLoad,
    selected: &Option<String>,
    root_selected_remote: Option<&str>,
    cx: &mut Context<RootView>,
) -> Div {
    let up_id = if is_local {
        "btn-browser-up-local"
    } else {
        "btn-browser-up-remote-top"
    };
    let pane = div()
        .flex_1()
        .flex()
        .flex_col()
        .min_h(px(0.))
        .rounded_md()
        .border_1()
        .border_color(color(BORDER))
        .bg(color(0x151517))
        .overflow_hidden()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(color(BORDER))
                .child(
                    div()
                        .flex_1()
                        .text_xs()
                        .text_color(color(TEXT_MUTED))
                        .child(st(title.to_string())),
                )
                .child(
                    secondary_button(up_id, "↑").on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            if is_local {
                                this.browser_up_local(cx);
                            } else {
                                this.browser_up_remote(cx);
                            }
                        }),
                    ),
                ),
        )
        .child(match entries {
            PaneLoad::Loading => {
                div().p_4().child(muted("加载中…")).into_any_element()
            }
            PaneLoad::Failed(e) => div()
                .p_4()
                .text_sm()
                .text_color(color(ERROR))
                .child(e.clone())
                .into_any_element(),
            PaneLoad::Loaded(list) if list.is_empty() => {
                let hint = if !is_local && root_selected_remote.is_none() {
                    "请先在上方选择远端存储"
                } else {
                    "（空目录）"
                };
                div().p_4().child(muted(hint)).into_any_element()
            }
            PaneLoad::Loaded(list) => entry_list(is_local, list, selected, cx).into_any_element(),
        });

    pane
}

fn entry_list(
    is_local: bool,
    entries: &[DirEntry],
    selected: &Option<String>,
    cx: &mut Context<RootView>,
) -> Stateful<Div> {
    let scroll_id = if is_local {
        "list-local-scroll"
    } else {
        "list-remote-scroll"
    };
    div()
        .id(scroll_id)
        .flex_1()
        .min_h(px(0.))
        .overflow_y_scroll()
        .children(entries.iter().map(|e| {
            let is_sel = selected.as_deref() == Some(e.name.as_str());
            let name = e.name.clone();
            let is_dir = e.is_dir;
            let mut row = div()
                .id(ElementId::Name(format!("{scroll_id}-{}", e.name).into()))
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_1()
                .text_sm()
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        let was_selected = if is_local {
                            this.browser.local_selected.as_deref() == Some(name.as_str())
                        } else {
                            this.browser.remote_selected.as_deref() == Some(name.as_str())
                        };
                        // 目录：再次点击进入；文件：选中
                        if is_dir && was_selected {
                            if is_local {
                                this.browser_enter_local(name.clone(), cx);
                            } else {
                                this.browser_enter_remote(name.clone(), cx);
                            }
                        } else {
                            if is_local {
                                this.browser.local_selected = Some(name.clone());
                            } else {
                                this.browser.remote_selected = Some(name.clone());
                            }
                            cx.notify();
                        }
                    }),
                );
            row = row
                .child(
                    div()
                        .text_xs()
                        .child(if is_dir { "📁 " } else { "📄 " }),
                )
                .child(
                    div()
                        .flex_1()
                        .child(st(e.name.clone()))
                        .when(is_sel, |d| d.text_color(color(ACCENT))),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(color(TEXT_MUTED))
                        .child(st(if is_dir {
                            String::new()
                        } else {
                            util::fmt_size(e.size as f64)
                        })),
                );
            if is_sel {
                row = row.bg(color(0x1e293b));
            } else {
                row = row.hover(|s| s.bg(color(HOVER)));
            }
            row
        }))
}
