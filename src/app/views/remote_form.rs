//! 存储添加向导：模板卡片 + 动态表单（config/providers 驱动）

use crate::app::{ProvidersLoad, RootView};
use crate::rclone::client::Provider;
use crate::stext::st;
use crate::theme::*;
use gpui::{prelude::*, *};

/// 常用模板
const TEMPLATES: &[(&str, &str, &str)] = &[
    ("http", "HTTP", "只读下载静态文件服务"),
    ("webdav", "WebDAV", "坚果云 / Nextcloud 等"),
    ("s3", "S3 对象存储", "AWS / MinIO / 国内对象存储"),
    ("sftp", "SFTP", "SSH 文件传输"),
    ("ftp", "FTP", "传统 FTP 服务"),
    ("local", "本地目录", "本机磁盘目录"),
];

pub fn render(root: &RootView, cx: &mut Context<RootView>) -> Div {
    let header = div()
        .flex()
        .items_center()
        .gap_3()
        .child(page_title("添加存储").mb_0())
        .child(
            secondary_button("btn-wizard-close", "关闭").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.close_wizard(cx)),
            ),
        );

    let Some(wizard) = &root.wizard else {
        return div().child(header);
    };
    let body = match &wizard.providers {
        ProvidersLoad::Loading => muted("正在加载存储类型列表…").into_any_element(),
        ProvidersLoad::Failed(e) => div()
            .text_sm()
            .text_color(color(ERROR))
            .child(st(e.clone()))
            .into_any_element(),
        ProvidersLoad::Loaded(providers) => {
            if wizard.selected.is_some() {
                render_form(root, providers, cx).into_any_element()
            } else if wizard.show_all {
                render_all(providers, cx).into_any_element()
            } else {
                render_templates(providers, cx).into_any_element()
            }
        }
    };

    div().flex().flex_col().gap_4().child(header).child(body)
}

/// 模板卡片选择页
fn render_templates(providers: &[Provider], cx: &mut Context<RootView>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(muted("选择一个存储类型开始。常用类型如下，其他类型可查看全部。"))
        .child(
            div()
                .flex()
                .gap_3()
                .flex_wrap()
                .children(TEMPLATES.iter().map(|(kind, name, desc)| {
                    let kind = kind.to_string();
                    let available = providers.iter().any(|p| p.name == kind);
                    template_card(name, desc, available, kind.clone(), cx)
                })),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .child(
                    secondary_button("btn-all-providers", "查看全部类型").on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            if let Some(w) = this.wizard.as_mut() {
                                w.show_all = true;
                            }
                            cx.notify();
                        }),
                    ),
                ),
        )
}

fn template_card(
    name: &str,
    desc: &str,
    available: bool,
    kind: String,
    cx: &mut Context<RootView>,
) -> Stateful<Div> {
    let id = format!("tpl-{kind}");
    div()
        .id(ElementId::Name(id.into()))
        .w(px(180.))
        .flex()
        .flex_col()
        .gap_1()
        .p_4()
        .rounded_md()
        .border_1()
        .border_color(color(BORDER))
        .cursor_pointer()
        .hover(|s| s.bg(color(HOVER)))
        .when(!available, |d| d.opacity(0.4))
        .child(div().text_sm().child(st(name.to_string())))
        .child(div().text_xs().text_color(color(TEXT_MUTED)).child(st(desc.to_string())))
        .when(available, |d| {
            d.on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.wizard_select_provider(kind.clone(), cx)
                }),
            )
        })
}

/// 全部类型选择页
fn render_all(providers: &[Provider], cx: &mut Context<RootView>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(muted("共 {} 种后端，点击选择".replace("{}", &providers.len().to_string())))
        .child(
            div()
                .id("providers-scroll")
                .flex()
                .flex_wrap()
                .gap_2()
                .children(providers.iter().map(|p| {
                    let name = p.name.clone();
                    let desc = p.description.clone();
                    div()
                        .id(ElementId::Name(format!("pv-{}", p.name).into()))
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .border_1()
                        .border_color(color(BORDER))
                        .cursor_pointer()
                        .hover(|s| s.bg(color(HOVER)))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.wizard_select_provider(name.clone(), cx)
                            }),
                        )
                        .child(div().text_sm().child(st(p.name.clone())))
                        .child(
                            div()
                                .text_xs()
                                .text_color(color(TEXT_MUTED))
                                .child(st(desc)),
                        )
                })),
        )
}

/// 表单页（动态字段）
fn render_form(
    root: &RootView,
    providers: &[Provider],
    cx: &mut Context<RootView>,
) -> Div {
    let wizard = root.wizard.as_ref().unwrap();
    let Some(provider) = providers.iter().find(|p| {
        wizard.selected.as_deref() == Some(p.name.as_str())
    }) else {
        return muted("未找到所选类型").into();
    };

    let mut form = div().flex().flex_col().gap_3().child(
        div()
            .text_sm()
            .text_color(color(TEXT_MUTED))
            .child(st(format!("类型：{} · {}", provider.name, provider.description))),
    );

    // 名称
    form = form.child(
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(crate::text_input::form_label("存储名称 *"))
            .child(wizard.name_input.clone()),
    );

    // 字段（必填 & 基础字段在前；高级选项可展开）
    let visible: Vec<_> = wizard
        .fields
        .iter()
        .filter(|f| !f.option.advanced || wizard.show_advanced)
        .collect();
    for f in &visible {
        let required_mark = if f.option.required { " *" } else { "" };
        form = form.child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(crate::text_input::form_label(format!(
                    "{}{required_mark}",
                    f.option.name
                )))
                .child(f.input.clone())
                .child(
                    div()
                        .text_xs()
                        .text_color(color(TEXT_MUTED))
                        .child(st(f.option.help_summary())),
                ),
        );
    }

    let has_advanced = wizard.fields.iter().any(|f| f.option.advanced);
    if has_advanced {
        form = form.child(
            secondary_button("btn-toggle-advanced", {
                if wizard.show_advanced {
                    "收起高级选项"
                } else {
                    "显示高级选项"
                }
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.wizard_toggle_advanced(cx)),
            ),
        );
    }

    // 提示与操作
    if let Some((ok, text)) = &wizard.message {
        form = form.child(
            div()
                .text_sm()
                .text_color(color(if *ok { SUCCESS } else { ERROR }))
                .child(st(text.clone())),
        );
    }
    form = form.child(
        div()
            .flex()
            .gap_2()
            .child(
                primary_button("btn-create", {
                    if wizard.busy { "创建中…" } else { "创建并测试连接" }
                })
                .when(wizard.busy, |d| d.opacity(0.6))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| this.wizard_create(cx)),
                ),
            )
            .child(
                secondary_button("btn-wizard-back", "返回类型选择").on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        if let Some(w) = this.wizard.as_mut() {
                            w.selected = None;
                            w.fields.clear();
                            w.message = None;
                        }
                        cx.notify();
                    }),
                ),
            ),
    );

    form
}
