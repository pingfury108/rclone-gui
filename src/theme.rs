//! 主题：Zed 风格深色配色 + 通用小组件

use crate::stext::st;
use gpui::{div, prelude::*, px, rgb, rgba, Div, Rgba, SharedString, Stateful};

pub const BG: u32 = 0x18181b;
pub const PANEL: u32 = 0x111113;
pub const BORDER: u32 = 0x27272a;
pub const HOVER: u32 = 0x27272a;
pub const TEXT: u32 = 0xe4e4e7;
pub const TEXT_MUTED: u32 = 0xa1a1aa;
pub const ACCENT: u32 = 0x0ea5e9;
pub const ACCENT_HOVER: u32 = 0x0284c7;
pub const SUCCESS: u32 = 0x22c55e;
pub const ERROR: u32 = 0xef4444;

pub const FONT_FAMILY: &str = "Noto Sans SC";

pub fn color(v: u32) -> Rgba {
    rgb(v)
}

/// 主按钮（强调色）
pub fn primary_button(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id.into())
        .px_3()
        .py_1p5()
        .rounded_md()
        .bg(color(ACCENT))
        .text_color(color(0xffffff))
        .text_sm()
        .cursor_pointer()
        .hover(|s| s.bg(color(ACCENT_HOVER)))
        .child(label.into())
}

/// 次按钮（边框式）
pub fn secondary_button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
) -> Stateful<Div> {
    div()
        .id(id.into())
        .px_3()
        .py_1p5()
        .rounded_md()
        .border_1()
        .border_color(color(BORDER))
        .text_color(color(TEXT))
        .text_sm()
        .cursor_pointer()
        .hover(|s| s.bg(color(HOVER)))
        .child(label.into())
}

/// 页面标题
pub fn page_title(label: impl Into<SharedString>) -> Div {
    div()
        .text_xl()
        .text_color(color(TEXT))
        .mb_4()
        .child(st(label))
}

/// 次要说明文字
pub fn muted(label: impl Into<SharedString>) -> Div {
    div()
        .text_sm()
        .text_color(color(TEXT_MUTED))
        .child(st(label))
}

/// 状态点（●）
pub fn status_dot(ok: bool) -> Div {
    div()
        .text_xs()
        .text_color(color(if ok { SUCCESS } else { ERROR }))
        .child("●")
}

/// 危险操作按钮（删除确认等）
pub fn danger_button(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id.into())
        .px_3()
        .py_1p5()
        .rounded_md()
        .bg(color(ERROR))
        .text_color(color(0xffffff))
        .text_sm()
        .cursor_pointer()
        .hover(|s| s.bg(color(0xdc2626)))
        .child(label.into())
}

/// 模态遮罩层（覆盖内容区，拦截下层点击）
pub fn modal_overlay(id: impl Into<SharedString>, panel: Div) -> Stateful<Div> {
    div()
        .id(id.into())
        .absolute()
        .inset_0()
        .bg(rgba(0x000000_99))
        .flex()
        .items_center()
        .justify_center()
        .child(panel)
}

/// 模态面板容器
pub fn modal_panel(content: Div) -> Div {
    div()
        .w(px(480.))
        .max_h_full()
        .overflow_hidden()
        .bg(color(BG))
        .border_1()
        .border_color(color(BORDER))
        .rounded_lg()
        .p_5()
        .child(content)
}

/// 对话框标题
pub fn modal_title(label: impl Into<SharedString>) -> Div {
    div().text_lg().mb_3().child(st(label))
}
