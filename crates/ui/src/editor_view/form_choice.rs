use gpui::prelude::FluentBuilder;
use gpui::{
    Context, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, px, rgba,
};
use gpui_component::tooltip::Tooltip;

use super::geometry::overlay_rect;
use super::model::{ChoiceMenuState, OverlayRect};
use super::page_canvas::{form_text_size, positioned};
use crate::EditorView;
use crate::field_input::FieldInput;
use crate::theme::{PAGE_TEXT, solid};

/// Renders a choice field and its editor-owned option menu.
#[allow(clippy::too_many_arguments)]
pub(super) fn add_form_choice_overlay(
    page: gpui::Div,
    view: &gpui::Entity<EditorView>,
    item: &FieldInput,
    widget_rect: document_core::PdfRect,
    geometry: document_core::PageGeometry,
    zoom: f32,
    field_index: usize,
    widget_index: usize,
    page_index: usize,
    hint: Option<&str>,
    cx: &mut Context<EditorView>,
) -> gpui::Div {
    let rect = overlay_rect(widget_rect, geometry, zoom);
    let field = &item.field;
    let options = field.options.clone();
    let current = item.value(cx);
    let field_name = field.name.clone();
    let multi_select = field.multi_select;
    let menu_state = ChoiceMenuState {
        field_name: field_name.clone(),
        page_index,
        rect,
        options: options.clone(),
        current: current.clone(),
        multi_select,
    };
    let open_view = view.clone();
    let mut overlay = positioned(rect)
        .id(("form-choice", field_index * 1000 + widget_index))
        .occlude()
        .flex()
        .items_center()
        .justify_between()
        .px(px(2.0))
        .bg(rgba(0x4e9c_ff2e))
        .text_size(px(form_text_size(rect.height)))
        .text_color(solid(PAGE_TEXT))
        .cursor_pointer()
        .child(display_choice_label(&options, &current))
        .child("\u{25be}")
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            open_view.update(cx, |view, cx| {
                view.choice_menu = Some(menu_state.clone());
                cx.notify();
            });
        });
    if let Some(hint) = hint {
        let hint = hint.to_owned();
        overlay = overlay.tooltip(move |window, cx| Tooltip::new(hint.clone()).build(window, cx));
    }

    page.child(overlay)
}

pub(super) fn render_choice_menu(
    menu: &ChoiceMenuState,
    view: &gpui::Entity<EditorView>,
) -> gpui::AnyElement {
    let width = menu.rect.width.max(180.0);
    let height = choice_menu_height(menu.options.len());
    let mut popup = positioned(OverlayRect {
        left: menu.rect.left,
        top: menu.rect.top + menu.rect.height + 2.0,
        width,
        height,
    })
    .id("form-choice-menu")
    .flex()
    .flex_col()
    .overflow_hidden()
    .bg(solid(0xffff_ffff))
    .border_1()
    .border_color(solid(0xff9c_a3af))
    .shadow_lg();

    for (option_index, (export, label)) in menu.options.iter().enumerate() {
        let selected = if menu.multi_select {
            menu.current.split('\u{1f}').any(|value| value == export)
        } else {
            *export == menu.current
        };
        let field_name = menu.field_name.clone();
        let current = menu.current.clone();
        let export = export.clone();
        let multi_select = menu.multi_select;
        let view = view.clone();
        popup = popup.child(
            gpui::div()
                .id(("form-choice-option", option_index))
                .h(px(24.0))
                .flex()
                .items_center()
                .px(px(6.0))
                .text_color(solid(PAGE_TEXT))
                .when(selected, |this| this.bg(rgba(0x4e9c_ff33)))
                .cursor_pointer()
                .child(label.clone())
                .on_click(move |_, window, cx| {
                    view.update(cx, |view, cx| {
                        let value = choice_value(&current, &export, multi_select);
                        view.set_form_value(&field_name, &value, window, cx);
                        view.capture_form_edits(cx);
                        view.choice_menu = None;
                        cx.notify();
                    });
                }),
        );
    }
    popup.into_any_element()
}

fn display_choice_label(options: &[(String, String)], value: &str) -> String {
    if value.contains('\u{1f}') {
        return value
            .split('\u{1f}')
            .filter(|value| !value.is_empty())
            .map(|value| display_choice_label(options, value))
            .collect::<Vec<_>>()
            .join(", ");
    }
    options
        .iter()
        .find(|(export, _)| export == value)
        .map_or_else(|| value.to_owned(), |(_, label)| label.clone())
}

fn toggle_choice_value(current: &str, export: &str) -> String {
    let mut values: Vec<_> = current
        .split('\u{1f}')
        .filter(|value| !value.is_empty() && *value != export)
        .map(str::to_owned)
        .collect();
    if !current.split('\u{1f}').any(|value| value == export) {
        values.push(export.to_owned());
    }
    values.join("\u{1f}")
}

pub(super) fn choice_value(current: &str, export: &str, multi_select: bool) -> String {
    if multi_select {
        toggle_choice_value(current, export)
    } else {
        export.to_owned()
    }
}

pub(super) fn choice_menu_height(option_count: usize) -> f32 {
    f32::from(u16::try_from(option_count).unwrap_or(u16::MAX)) * 24.0
}

#[cfg(test)]
mod tests {
    use super::{display_choice_label, toggle_choice_value};

    #[test]
    fn choice_labels_use_display_values() {
        let options = vec![("A".to_owned(), "Artist".to_owned())];
        assert_eq!(display_choice_label(&options, "A"), "Artist");
    }

    #[test]
    fn multi_choice_values_toggle() {
        assert_eq!(toggle_choice_value("A\u{1f}B", "A"), "B");
        assert_eq!(toggle_choice_value("A", "B"), "A\u{1f}B");
    }
}
