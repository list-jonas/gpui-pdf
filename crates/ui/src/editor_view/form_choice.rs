use gpui::{
    Context, InteractiveElement, ParentElement, StatefulInteractiveElement, Styled, px, rgba,
};
use gpui_component::menu::{ContextMenuExt, PopupMenuItem};
use gpui_component::tooltip::Tooltip;

use super::geometry::overlay_rect;
use super::page_canvas::form_text_size;
use super::page_canvas::positioned;
use crate::EditorView;
use crate::field_input::FieldInput;
use crate::theme::{PAGE_TEXT, solid};

/// Renders a choice field as a dropdown whose menu lists every export value.
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
    hint: Option<&str>,
    cx: &mut Context<EditorView>,
) -> gpui::Div {
    let rect = overlay_rect(widget_rect, geometry, zoom);
    let field = &item.field;
    let options = field.options.clone();
    let current = item.value(cx);
    let multi_select = field.multi_select;
    let field_name = field.name.clone();
    let view = view.clone();
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
        .child(display_choice_label(&options, &current))
        .child("\u{25be}");
    if let Some(hint) = hint {
        let hint = hint.to_owned();
        overlay = overlay.tooltip(move |window, cx| Tooltip::new(hint.clone()).build(window, cx));
    }
    page.child(overlay.context_menu_with_id(
        ("form-choice-menu", field_index * 1000 + widget_index),
        move |mut menu, _, _cx| {
            for (export, label) in &options {
                let selected = if multi_select {
                    current.split('\u{1f}').any(|value| value == export)
                } else {
                    *export == current
                };
                let export = export.clone();
                let current = current.clone();
                let field_name = field_name.clone();
                let view = view.clone();
                menu = menu.item(
                    PopupMenuItem::new(label.clone())
                        .checked(selected)
                        .on_click(move |_, window, cx| {
                            view.update(cx, |view, cx| {
                                let value = if multi_select {
                                    toggle_choice_value(&current, &export)
                                } else {
                                    export.clone()
                                };
                                view.set_form_value(&field_name, &value, window, cx);
                                view.capture_form_edits(cx);
                            });
                        }),
                );
            }
            menu
        },
    ))
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
