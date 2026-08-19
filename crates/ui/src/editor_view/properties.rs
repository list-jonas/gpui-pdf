use gpui::{
    Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, rgb,
};
use gpui_component::Disableable;
use gpui_component::button::Button;
use gpui_component::scroll::ScrollableElement;

use crate::EditorView;
use crate::actions::CommitText;

use super::model::Tool;

impl EditorView {
    pub(super) fn render_properties(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let panel = div()
            .w_72()
            .flex_shrink_0()
            .h_full()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .bg(rgb(0x0011_1418))
            .border_l_1()
            .border_color(rgb(0x0025_292f))
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Properties"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x009c_a3ad))
                    .child(format!("{} tool", self.tool.label())),
            );

        self.add_tool_properties(panel, cx)
            .when_some(self.detail.clone(), |panel, detail| {
                panel.child(
                    div()
                        .pt_4()
                        .border_t_1()
                        .border_color(rgb(0x0025_292f))
                        .text_xs()
                        .text_color(rgb(0x009c_a3ad))
                        .child(detail),
                )
            })
            .overflow_y_scrollbar()
    }

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
                    .child(
                        div()
                            .p_3()
                            .rounded_md()
                            .bg(rgb(0x001c_2026))
                            .text_sm()
                            .child(self.selected_text.clone()),
                    ),
            ),
            Tool::Hand => panel.child("Drag page to pan. Scroll and trackpad also work."),
            Tool::Highlight => panel
                .child("Drag across text. Highlight follows extracted text runs.")
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(Self::color_button(
                            "yellow",
                            0x00ff_dc33,
                            (1.0, 0.86, 0.2),
                            cx,
                        ))
                        .child(Self::color_button(
                            "green",
                            0x0078_d98b,
                            (0.35, 0.85, 0.45),
                            cx,
                        ))
                        .child(Self::color_button(
                            "pink",
                            0x00ff_7bab,
                            (1.0, 0.35, 0.6),
                            cx,
                        )),
                ),
            Tool::AddText => panel
                .child("Click page, type directly over PDF, then commit.")
                .child(
                    Button::new("commit-text")
                        .label("Commit text")
                        .disabled(self.inline_text.is_none())
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.commit_text(&CommitText, window, cx);
                        })),
                ),
            Tool::Redact => panel
                .child("Drag a rectangle over content. Save As applies permanent redaction.")
                .child(
                    div()
                        .p_3()
                        .rounded_md()
                        .bg(rgb(0x0038_1719))
                        .text_color(rgb(0x00ff_a4a4))
                        .child("Verify saved output before distributing."),
                ),
        }
    }

    fn color_button(
        id: &'static str,
        color: u32,
        value: (f64, f64, f64),
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(id)
            .size_8()
            .rounded_full()
            .bg(rgb(color))
            .border_1()
            .border_color(rgb(0x0090_9499))
            .cursor_pointer()
            .on_click(cx.listener(move |view, _, _, cx| view.set_highlight_color(value, cx)))
    }
}
