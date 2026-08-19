use gpui::{
    Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Render, Styled, Window,
    div, img, prelude::FluentBuilder, px, rgb,
};
use gpui_component::button::Button;
use gpui_component::{Disableable, Selectable};

use crate::EditorView;
use crate::actions::{
    AddTextTool, FitPage, HandTool, HighlightTool, NextPage, OpenDocument, PreviousPage,
    RedactTool, SaveDocument, SelectTool, ZoomIn, ZoomOut,
};

use super::geometry::raster_f32;
use super::model::Tool;

impl EditorView {
    fn render_title_bar(&self) -> impl IntoElement {
        div()
            .h_12()
            .flex()
            .items_center()
            .gap_3()
            .px_4()
            .bg(rgb(0x0021_2428))
            .text_color(rgb(0x00ff_ffff))
            .child(
                div()
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(rgb(0x00e6_2020))
                    .font_weight(FontWeight::BOLD)
                    .child("PDF"),
            )
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.status.clone()),
            )
            .child(
                div()
                    .ml_auto()
                    .text_xs()
                    .text_color(rgb(0x00c8_cbd0))
                    .child(format!("{} pending edits", self.edits.len())),
            )
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h(px(56.0))
            .flex()
            .items_center()
            .gap_1()
            .px_3()
            .bg(rgb(0x00f7_f7f8))
            .border_b_1()
            .border_color(rgb(0x00d8_dadd))
            .child(Button::new("open").label("Open").on_click(
                cx.listener(|view, _, window, cx| view.open_picker(&OpenDocument, window, cx)),
            ))
            .child(Button::new("save").label("Save As…").on_click(
                cx.listener(|view, _, window, cx| view.save_picker(&SaveDocument, window, cx)),
            ))
            .child(div().h_7().w_px().mx_2().bg(rgb(0x00d3_d5d8)))
            .child(self.tool_button("select", "Select", Tool::Select, SelectTool, cx))
            .child(self.tool_button("hand", "Hand", Tool::Hand, HandTool, cx))
            .child(self.tool_button("highlight", "Highlight", Tool::Highlight, HighlightTool, cx))
            .child(self.tool_button("add-text", "Add text", Tool::AddText, AddTextTool, cx))
            .child(self.tool_button("redact", "Redact", Tool::Redact, RedactTool, cx))
            .child(div().h_7().w_px().mx_2().bg(rgb(0x00d3_d5d8)))
            .child(
                Button::new("previous")
                    .label("‹")
                    .disabled(self.page_index == 0)
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.previous_page(&PreviousPage, window, cx);
                    })),
            )
            .child(div().min_w_20().text_center().child(format!(
                "{} / {}",
                self.page_index + 1,
                self.page_count.max(1)
            )))
            .child(
                Button::new("next")
                    .label("›")
                    .disabled(self.page_count == 0 || self.page_index + 1 >= self.page_count)
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.next_page(&NextPage, window, cx);
                    })),
            )
            .child(div().h_7().w_px().mx_2().bg(rgb(0x00d3_d5d8)))
            .child(
                Button::new("zoom-out").label("−").on_click(
                    cx.listener(|view, _, window, cx| view.zoom_out(&ZoomOut, window, cx)),
                ),
            )
            .child(
                div()
                    .min_w(px(56.0))
                    .text_center()
                    .child(format!("{}%", (self.zoom * 100.0).round())),
            )
            .child(
                Button::new("zoom-in")
                    .label("+")
                    .on_click(cx.listener(|view, _, window, cx| view.zoom_in(&ZoomIn, window, cx))),
            )
            .child(
                Button::new("fit").label("Fit").on_click(
                    cx.listener(|view, _, window, cx| view.fit_page(&FitPage, window, cx)),
                ),
            )
    }

    fn tool_button<A: gpui::Action>(
        &self,
        id: &'static str,
        label: &'static str,
        tool: Tool,
        action: A,
        cx: &mut Context<Self>,
    ) -> Button {
        Button::new(id)
            .label(label)
            .selected(self.tool == tool)
            .on_click(cx.listener(move |_, _, window, cx| {
                window.dispatch_action(action.boxed_clone(), cx);
            }))
    }

    fn render_left_panel(&self) -> impl IntoElement {
        div()
            .w_40()
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .gap_3()
            .p_3()
            .bg(rgb(0x00f4_f5f6))
            .border_r_1()
            .border_color(rgb(0x00d8_dadd))
            .child(
                div()
                    .w_full()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Pages"),
            )
            .when_some(self.image.clone(), |panel, image| {
                let ratio = raster_f32(self.image_size.1) / raster_f32(self.image_size.0.max(1));
                panel.child(
                    div()
                        .w(px(112.0))
                        .h(px(112.0 * ratio))
                        .p_1()
                        .bg(rgb(0x00ff_ffff))
                        .border_2()
                        .border_color(rgb(0x003b_82f6))
                        .shadow_sm()
                        .child(img(image).size_full()),
                )
            })
            .child(
                div()
                    .text_sm()
                    .child(format!("Page {}", self.page_index + 1)),
            )
    }
}

impl Render for EditorView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context("PdfEditor")
            .on_action(cx.listener(Self::open_picker))
            .on_action(cx.listener(Self::save_picker))
            .on_action(cx.listener(Self::previous_page))
            .on_action(cx.listener(Self::next_page))
            .on_action(cx.listener(Self::select_tool))
            .on_action(cx.listener(Self::hand_tool))
            .on_action(cx.listener(Self::highlight_tool))
            .on_action(cx.listener(Self::add_text_tool))
            .on_action(cx.listener(Self::redact_tool))
            .on_action(cx.listener(Self::zoom_in))
            .on_action(cx.listener(Self::zoom_out))
            .on_action(cx.listener(Self::actual_size))
            .on_action(cx.listener(Self::fit_page))
            .on_action(cx.listener(Self::commit_text))
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x00dd_dfe2))
            .text_color(rgb(0x0020_2124))
            .child(self.render_title_bar())
            .child(self.render_toolbar(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.render_left_panel())
                    .child(self.render_document(cx))
                    .child(self.render_properties(cx)),
            )
            .child(
                div()
                    .h_7()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_3()
                    .text_xs()
                    .bg(rgb(0x00f7_f7f8))
                    .border_t_1()
                    .border_color(rgb(0x00d8_dadd))
                    .child(format!("{} tool", self.tool.label()))
                    .child(format!("{}%", (self.zoom * 100.0).round()))
                    .when_some(self.extracted_text.clone(), |bar, text| {
                        bar.child(div().ml_auto().max_w_96().truncate().child(text))
                    }),
            )
    }
}
