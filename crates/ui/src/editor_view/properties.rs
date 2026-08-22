use gpui::{
    Context, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::menu::ContextMenuExt;
use gpui_component::scroll::ScrollableElement;

use super::model::{Tool, shape_label};
use crate::EditorView;
use crate::theme::{
    BORDER, BORDER_STRONG, DANGER_TEXT, DANGER_TINT, PANEL_TINT, TEXT_FAINT, TEXT_MUTED, WELL_TINT,
    solid, tint,
};
use pdf_engine::{EditCommand, ShapeKind};

impl EditorView {
    pub(super) fn render_properties(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let panel = div().w_72().h_full().p_4().flex().flex_col().gap_4().child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(self.tool.label()),
                )
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(tint(WELL_TINT))
                        .text_xs()
                        .text_color(tint(TEXT_MUTED))
                        .child(self.tool.shortcut()),
                ),
        );

        let content = self
            .add_tool_properties(panel, cx)
            .child(self.render_edit_list(cx))
            .when_some(self.detail.clone(), |panel, detail| {
                panel.child(
                    div()
                        .pt_4()
                        .border_t_1()
                        .border_color(tint(BORDER))
                        .text_xs()
                        .text_color(tint(TEXT_MUTED))
                        .child(detail),
                )
            })
            .overflow_y_scrollbar();

        // The scrollbar wrapper is not a parent element, so the menu lives on
        // an outer container that also carries the panel's chrome.
        div()
            .id("properties-panel")
            .flex_shrink_0()
            .h_full()
            .bg(tint(PANEL_TINT))
            .border_l_1()
            .border_color(tint(BORDER))
            .child(content)
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|view, event, _, _| view.clear_menu_target(event)),
            )
            .context_menu({
                let view = cx.entity();
                move |menu, _, cx| view.read(cx).view_menu(menu)
            })
    }

    /// Lists queued edits so users can see and remove pending changes.
    fn render_edit_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = div().flex().flex_col().gap_2().pt_3().border_t_1();
        list = list.border_color(tint(BORDER)).child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(tint(TEXT_FAINT))
                .child(if self.history.is_empty() {
                    "NO PENDING EDITS".to_owned()
                } else {
                    format!("PENDING EDITS ({})", self.history.len())
                }),
        );

        for (index, edit) in self.history.iter().enumerate().take(12) {
            list = list.child(
                div()
                    .id(("edit-row", index))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(tint(WELL_TINT))
                    .text_xs()
                    .child(div().flex_1().truncate().child(edit_label(edit)))
                    .child(
                        Button::new(("remove-edit", index))
                            .label("✕")
                            .ghost()
                            .cursor_pointer()
                            .tooltip("Remove this edit")
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.history.remove(index);
                                view.mark_edited(window, cx);
                            })),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |view, event, _, cx| {
                            view.prepare_edit_row_menu(index, event, cx);
                        }),
                    )
                    .context_menu({
                        let view = cx.entity();
                        move |menu, _, cx| view.read(cx).edit_row_menu(index, menu)
                    }),
            );
        }
        list.min_h(px(0.0))
    }

    #[allow(clippy::too_many_lines)]
    fn add_tool_properties(&self, panel: gpui::Div, cx: &mut Context<Self>) -> gpui::Div {
        match self.tool {
            Tool::Select => panel.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Selected text"),
                    )
                    .when_some(self.selection_summary(), |panel, summary| {
                        panel.child(
                            div()
                                .text_xs()
                                .text_color(tint(TEXT_MUTED))
                                .child(summary),
                        )
                    })
                    .child(
                        div()
                            .p_3()
                            .rounded_md()
                            .bg(tint(WELL_TINT))
                            .text_sm()
                            .child(self.selected_preview.clone()),
                    ),
            ),
            Tool::Edit => panel.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Edit"))
                    .child("Click text or comments you added to edit and move them. PDF form fields remain directly editable."),
            ),
            Tool::Hand => panel.child("Drag page to pan. Scroll and trackpad also work."),
            Tool::Highlight => panel
                .child("Drag across text. Highlight follows extracted text runs.")
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(self.color_button(
                            "yellow",
                            0x00ff_dc33,
                            (1.0, 0.86, 0.2),
                            cx,
                        ))
                        .child(self.color_button(
                            "green",
                            0x0078_d98b,
                            (0.35, 0.85, 0.45),
                            cx,
                        ))
                        .child(self.color_button(
                            "pink",
                            0x00ff_7bab,
                            (1.0, 0.35, 0.6),
                            cx,
                        )),
                ),
            Tool::Underline | Tool::Strikeout => panel
                .child("Drag across text. Markup follows extracted text runs.")
                .child(self.annotation_color_buttons(cx)),
            Tool::AddText => {
                panel.child("Click page and type. Click a saved text overlay to edit it.")
            }
            Tool::Note => panel
                .child("Click page and type. Click a saved comment to edit it.")
                .child(self.annotation_color_buttons(cx)),
            Tool::Signature => panel.child(
                "Click page and type a visual signature. This is not a certificate-backed digital signature.",
            ),
            Tool::Shape => panel
                .child(format!("Draw {} on page.", shape_label(self.shape_kind)))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(Self::shape_button(
                            "rectangle",
                            "Rectangle",
                            ShapeKind::Rectangle,
                            cx,
                        ))
                        .child(Self::shape_button(
                            "ellipse",
                            "Ellipse",
                            ShapeKind::Ellipse,
                            cx,
                        )),
                )
                .child(self.annotation_color_buttons(cx)),
            Tool::Redact => panel
                .child("Drag a rectangle over content. Saving applies permanent redaction.")
                .child(
                    div()
                        .p_3()
                        .rounded_md()
                        .bg(tint(DANGER_TINT))
                        .text_color(solid(DANGER_TEXT))
                        .child("Verify saved output before distributing."),
                ),
        }
    }

    fn color_button(
        &self,
        id: &'static str,
        color: u32,
        value: (f64, f64, f64),
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        swatch(id, color, same_color(self.highlight_color, value))
            .on_click(cx.listener(move |view, _, _, cx| view.set_highlight_color(value, cx)))
    }

    fn annotation_color_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .gap_2()
            .child(self.annotation_color_button(
                "annotation-blue",
                0x003b_82f6,
                (0.23, 0.51, 0.96),
                cx,
            ))
            .child(self.annotation_color_button(
                "annotation-red",
                0x00ef_4444,
                (0.94, 0.27, 0.27),
                cx,
            ))
            .child(self.annotation_color_button(
                "annotation-green",
                0x0022_c55e,
                (0.13, 0.65, 0.32),
                cx,
            ))
    }

    fn annotation_color_button(
        &self,
        id: &'static str,
        color: u32,
        value: (f64, f64, f64),
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        swatch(id, color, same_color(self.annotation_color, value))
            .on_click(cx.listener(move |view, _, _, cx| view.set_annotation_color(value, cx)))
    }

    fn shape_button(
        id: &'static str,
        label: &'static str,
        kind: ShapeKind,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        Button::new(id)
            .label(label)
            .on_click(cx.listener(move |view, _, _, cx| view.set_shape_kind(kind, cx)))
    }
}

/// Tolerant comparison for the small set of preset colors.
fn same_color(left: (f64, f64, f64), right: (f64, f64, f64)) -> bool {
    (left.0 - right.0).abs() < 0.01
        && (left.1 - right.1).abs() < 0.01
        && (left.2 - right.2).abs() < 0.01
}

fn swatch(id: &'static str, color: u32, selected: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .size_8()
        .rounded_full()
        .bg(solid(color))
        .border_2()
        .border_color(if selected {
            solid(0x00ff_ffff)
        } else {
            tint(BORDER_STRONG)
        })
        .cursor_pointer()
}

fn edit_label(edit: &EditCommand) -> String {
    match edit {
        EditCommand::Highlight { page_index, .. } => format!("Highlight · page {}", page_index + 1),
        EditCommand::Underline { page_index, .. } => format!("Underline · page {}", page_index + 1),
        EditCommand::StrikeOut { page_index, .. } => format!("Strikeout · page {}", page_index + 1),
        EditCommand::Redact { page_index, .. } => format!("Redaction · page {}", page_index + 1),
        EditCommand::Shape { page_index, .. } => format!("Shape · page {}", page_index + 1),
        EditCommand::Note { page_index, .. } => format!("Comment · page {}", page_index + 1),
        EditCommand::AddText(stamp) => {
            format!("Text · page {}", stamp.page_index + 1)
        }
        EditCommand::FillForm { name, .. } => format!("Form · {name}"),
    }
}
