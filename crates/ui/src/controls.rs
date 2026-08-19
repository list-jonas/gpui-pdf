use gpui::{Div, ParentElement, SharedString, Styled, div};
use gpui::{Entity, IntoElement};
use gpui_component::input::{Input, InputState};

pub fn section(title: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .rounded_md()
        .bg(gpui::rgb(0x00ff_ffff))
        .border_1()
        .border_color(gpui::rgb(0x00df_e1e5))
        .child(
            div()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(title.into()),
        )
}

pub fn labeled_input(
    label: impl Into<SharedString>,
    input: &Entity<InputState>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_xs()
                .text_color(gpui::rgb(0x005f_6368))
                .child(label.into()),
        )
        .child(Input::new(input))
}
