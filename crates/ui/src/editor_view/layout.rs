use gpui::{
    Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Render, Styled, Window,
    div, img, prelude::FluentBuilder,
};
use gpui_component::Disableable;
use gpui_component::button::Button;
use gpui_component::input::Input;
use gpui_component::scroll::ScrollableElement;
use pdf_engine::FormFieldKind;

use crate::EditorView;
use crate::actions::{NextPage, OpenDocument, PreviousPage, SaveDocument};
use crate::controls::{labeled_input, section};

impl EditorView {
    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .p_3()
            .bg(gpui::rgb(0x00ff_ffff))
            .border_b_1()
            .border_color(gpui::rgb(0x00df_e1e5))
            .child(Button::new("open").label("Open PDF").on_click(
                cx.listener(|view, _, window, cx| view.open_picker(&OpenDocument, window, cx)),
            ))
            .child(Button::new("save").label("Save As…").on_click(
                cx.listener(|view, _, window, cx| view.save_picker(&SaveDocument, window, cx)),
            ))
            .child(
                Button::new("previous")
                    .label("Previous")
                    .disabled(self.page_index == 0)
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.previous_page(&PreviousPage, window, cx);
                    })),
            )
            .child(
                Button::new("next")
                    .label("Next")
                    .disabled(self.page_count == 0 || self.page_index + 1 >= self.page_count)
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.next_page(&NextPage, window, cx);
                    })),
            )
            .child(
                div()
                    .ml_auto()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.status.clone()),
            )
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let forms = self.forms.iter().fold(
            section(format!("Form fields ({})", self.forms.len())),
            |section, item| {
                section.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .child(format!("{} · {:?}", item.field.name, item.field.kind)),
                        )
                        .child(Input::new(&item.input).disabled(
                            item.field.read_only || item.field.kind == FormFieldKind::Signature,
                        )),
                )
            },
        );

        div()
            .w_80()
            .flex_shrink_0()
            .h_full()
            .overflow_y_scrollbar()
            .p_3()
            .flex()
            .flex_col()
            .gap_3()
            .bg(gpui::rgb(0x00f7_f8fa))
            .border_l_1()
            .border_color(gpui::rgb(0x00df_e1e5))
            .child(forms)
            .child(
                section("Add text")
                    .child(labeled_input("Text", &self.add_text))
                    .child(
                        div()
                            .grid()
                            .grid_cols(3)
                            .gap_2()
                            .child(labeled_input("X", &self.text_x))
                            .child(labeled_input("Y", &self.text_y))
                            .child(labeled_input("Size", &self.text_size)),
                    )
                    .child(
                        Button::new("queue-text")
                            .label("Queue text")
                            .on_click(cx.listener(|view, _, _, cx| view.add_text(cx))),
                    ),
            )
            .child(
                section("Content redaction (beta)")
                    .child(
                        div().text_xs().child(
                            "PDF coordinates: left, bottom, right, top. Verify saved output.",
                        ),
                    )
                    .child(
                        div()
                            .grid()
                            .grid_cols(2)
                            .gap_2()
                            .child(labeled_input("Left", &self.redact_x0))
                            .child(labeled_input("Bottom", &self.redact_y0))
                            .child(labeled_input("Right", &self.redact_x1))
                            .child(labeled_input("Top", &self.redact_y1)),
                    )
                    .child(
                        Button::new("queue-redaction")
                            .label("Queue redaction")
                            .on_click(cx.listener(|view, _, _, cx| view.add_redaction(cx))),
                    ),
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
            .flex()
            .flex_col()
            .size_full()
            .bg(gpui::rgb(0x00e9_ebef))
            .text_color(gpui::rgb(0x0020_2124))
            .child(self.render_toolbar(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_w_0()
                            .p_5()
                            .items_center()
                            .justify_center()
                            .when_some(self.image.clone(), |view, image| {
                                view.child(
                                    div()
                                        .size_full()
                                        .bg(gpui::rgb(0x00ff_ffff))
                                        .shadow_lg()
                                        .child(img(image).size_full()),
                                )
                            })
                            .when(self.image.is_none(), |view| {
                                view.child("Open a PDF with Cmd+O or from Finder")
                            }),
                    )
                    .child(self.render_sidebar(cx)),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .bg(gpui::rgb(0x00ff_ffff))
                    .when_some(self.detail.clone(), ParentElement::child)
                    .when_some(self.extracted_text.clone(), |view, text| {
                        view.child(div().truncate().child(text))
                    }),
            )
    }
}
