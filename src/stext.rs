//! 全局文本选择：注册表 + 窗口级选区
//!
//! 原理：Stext 元素在 paint 时把自身 (bounds, ShapedLine) 注册到全局 REGISTRY；
//! 根视图监听鼠标事件维护全局选区（窗口坐标两点），每个 Stext 自绘选区高亮；
//! Cmd/Ctrl+C 时按阅读顺序拼接所有与选区相交的文本写入剪贴板。

use gpui::{
    actions, fill, point, prelude::*, rgba, App, Bounds, ClipboardItem, Element, ElementId,
    GlobalElementId, LayoutId, Pixels, Point, ShapedLine, SharedString, Style, TextRun, Window,
};
use std::sync::Mutex;

actions!(stext, [ClearSelection]);

/// 一帧内所有可见文本区域
pub struct TextRegion {
    pub bounds: Bounds<Pixels>,
    pub line: ShapedLine,
}

pub struct SelectionState {
    pub anchor: Option<Point<Pixels>>,
    pub focus: Option<Point<Pixels>>,
}

pub static REGISTRY: Mutex<Vec<TextRegion>> = Mutex::new(Vec::new());
pub static SELECTION: Mutex<SelectionState> = Mutex::new(SelectionState {
    anchor: None,
    focus: None,
});

pub fn clear_registry() {
    if let Ok(mut g) = REGISTRY.lock() {
        g.clear();
    }
}

pub fn begin_selection(p: Point<Pixels>) {
    if let Ok(mut g) = SELECTION.lock() {
        g.anchor = Some(p);
        g.focus = Some(p);
    }
}

/// 拖动更新选区；返回是否处于选择中（用于决定是否触发重绘）
pub fn update_selection(p: Point<Pixels>) -> bool {
    if let Ok(mut g) = SELECTION.lock() {
        if g.anchor.is_some() {
            g.focus = Some(p);
            return true;
        }
    }
    false
}

/// 鼠标抬起：零长度选区（单击）视为取消选择
pub fn end_selection() {
    if let Ok(mut g) = SELECTION.lock() {
        if g.anchor.is_some() && g.anchor == g.focus {
            g.anchor = None;
            g.focus = None;
        }
    }
}

pub fn clear_selection() {
    if let Ok(mut g) = SELECTION.lock() {
        g.anchor = None;
        g.focus = None;
    }
}

/// 某文本区域与选区的交集（字节范围）
fn selection_range(bounds: &Bounds<Pixels>, line: &ShapedLine) -> Option<std::ops::Range<usize>> {
    let sel = SELECTION.lock().ok()?;
    let (a, f) = (sel.anchor?, sel.focus?);
    if a == f {
        return None;
    }
    let (start, end) = if a.y < f.y || (a.y == f.y && a.x <= f.x) {
        (a, f)
    } else {
        (f, a)
    };

    let top = bounds.top();
    let bottom = bounds.bottom();
    if end.y < top || start.y > bottom {
        return None;
    }
    let len = line.text.len();
    let left = bounds.left();
    let start_idx = if start.y < top {
        0
    } else if start.y > bottom {
        len
    } else {
        line.closest_index_for_x(start.x - left)
    };
    let end_idx = if end.y > bottom {
        len
    } else if end.y < top {
        0
    } else {
        line.closest_index_for_x(end.x - left)
    };
    (start_idx < end_idx).then_some(start_idx..end_idx)
}

/// 拼接当前选中的文本（按阅读顺序：先上后下、先左后右）
pub fn selected_text() -> String {
    let Ok(registry) = REGISTRY.lock() else {
        return String::new();
    };
    let mut parts: Vec<(Bounds<Pixels>, String)> = registry
        .iter()
        .filter_map(|r| {
            selection_range(&r.bounds, &r.line)
                .map(|range| (r.bounds, r.line.text[range].to_string()))
        })
        .collect();
    parts.sort_by(|a, b| {
        (a.0.origin.y, a.0.origin.x)
            .partial_cmp(&(b.0.origin.y, b.0.origin.x))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut out = String::new();
    let mut last_top: Option<Pixels> = None;
    for (bounds, text) in parts {
        match last_top {
            Some(t) if t == bounds.top() => {}
            _ => {
                if !out.is_empty() {
                    out.push('\n');
                }
            }
        }
        out.push_str(&text);
        last_top = Some(bounds.top());
    }
    out
}

/// 把选中文本写入剪贴板
pub fn copy_selection_to_clipboard(cx: &mut App) {
    let text = selected_text();
    if !text.is_empty() {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }
}

// ---------- Stext 元素 ----------

/// 可选择的文本元素（单行；样式继承父级 text_* 设置）
pub struct Stext {
    text: SharedString,
}

pub fn st(text: impl Into<SharedString>) -> Stext {
    Stext { text: text.into() }
}

pub struct StextElement {
    text: SharedString,
}

impl IntoElement for StextElement {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl IntoElement for Stext {
    type Element = StextElement;
    fn into_element(self) -> Self::Element {
        // shape_line 不接受换行符，统一清洗
        StextElement {
            text: self.text.replace('\n', " ").into(),
        }
    }
}

impl Element for StextElement {
    type RequestLayoutState = ShapedLine;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let text_style = window.text_style();
        let run = TextRun {
            len: self.text.len(),
            font: text_style.font(),
            color: text_style.color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(self.text.clone(), font_size, &[run], None);

        let mut style = Style::default();
        style.size.width = line.width.into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), line)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let line = request_layout.clone();

        // 选区高亮（画在文本下方）
        if let Some(range) = selection_range(&bounds, &line) {
            let x0 = line.x_for_index(range.start);
            let x1 = line.x_for_index(range.end);
            window.paint_quad(fill(
                Bounds::from_corners(
                    point(bounds.left() + x0, bounds.top()),
                    point(bounds.left() + x1, bounds.bottom()),
                ),
                rgba(0x0ea5e955),
            ));
        }

        line.paint(bounds.origin, window.line_height(), window, cx)
            .unwrap();

        if let Ok(mut g) = REGISTRY.lock() {
            g.push(TextRegion { bounds, line });
        }
    }
}

/// 用于占位符样式的半透明文本（与 Stext 相同，预留扩展）
#[allow(dead_code)]
pub fn st_placeholder(text: impl Into<SharedString>) -> Stext {
    st(text)
}
