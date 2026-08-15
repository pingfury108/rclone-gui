// Windows 下作为 GUI 应用运行（不弹控制台窗口）
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod applog;
mod config;
mod rclone;
mod stext;
mod text_input;
mod theme;
mod util;

use app::RootView;
use gpui::*;
use std::borrow::Cow;
use text_input::{
    Backspace, Copy, Cut, Delete, End, Home, Left, Paste, Right, SelectAll, SelectLeft,
    SelectRight, ShowCharacterPalette,
};

fn main() {
    Application::new().run(|cx: &mut App| {
        // 注册内嵌中文字体；失败时回退系统字体（macOS/Windows 自带 CJK 字体）
        let _ = cx
            .text_system()
            .add_fonts(vec![Cow::Borrowed(
                include_bytes!("../assets/fonts/NotoSansSC-Regular.otf") as &[u8],
            )]);

        // 文本输入框快捷键
        cx.bind_keys([
            KeyBinding::new("backspace", Backspace, None),
            KeyBinding::new("delete", Delete, None),
            KeyBinding::new("left", Left, None),
            KeyBinding::new("right", Right, None),
            KeyBinding::new("shift-left", SelectLeft, None),
            KeyBinding::new("shift-right", SelectRight, None),
            KeyBinding::new("cmd-a", SelectAll, None),
            KeyBinding::new("ctrl-a", SelectAll, None),
            KeyBinding::new("cmd-v", Paste, None),
            KeyBinding::new("ctrl-v", Paste, None),
            KeyBinding::new("cmd-c", Copy, None),
            KeyBinding::new("ctrl-c", Copy, None),
            KeyBinding::new("cmd-x", Cut, None),
            KeyBinding::new("ctrl-x", Cut, None),
            KeyBinding::new("home", Home, None),
            KeyBinding::new("end", End, None),
            KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, None),
            KeyBinding::new("escape", stext::ClearSelection, None),
        ]);

        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1180.), px(760.)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Rclone GUI".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |_, cx| cx.new(|cx| RootView::new(cx)),
            )
            .unwrap();
        // 初始焦点给到根视图，使全局选择快捷键可用
        window
            .update(cx, |view, window, cx| {
                let handle = view.focus_handle(cx);
                window.focus(&handle);
            })
            .unwrap();
        cx.activate(true);
    });
}
