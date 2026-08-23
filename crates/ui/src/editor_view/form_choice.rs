use gpui::{Context, InteractiveElement, ParentElement, Styled, px};
use gpui_component::menu::{ContextMenuExt, PopupMenuItem};

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
    cx: &mut Context<EditorView>,
) -> gpui::Div {
    let rect = overlay_rect(widget_rect, geometry, zoom);
    let field = &item.field;
    let options = field.options.clone();
    let current = item.value(cx);
    let field_name = field.name.clone();
    let view = view.clone();
    page.child(
        positioned(rect)
            .id(("form-choice", field_index * 1000 + widget_index))
            .occlude()
            .flex()
            .items_center()
            .justify_between()
            .px(px(2.0))
            .bg(solid(PAGE_TEXT))
            .text_size(px(form_text_size(rect.height)))
            .text_color(solid(0x00ff_ffff))
            .child(display_choice_label(&options, &current))
            .child("\u{25be}")
            .context_menu_with_id(
                ("form-choice-menu", field_index * 1000 + widget_index),
                move |mut menu, _, _cx| {
                    for (export, label) in &options {
                        let selected = *export == current;
                        let export = export.clone();
                        let field_name = field_name.clone();
                        let view = view.clone();
                        menu = menu.item(
                            PopupMenuItem::new(label.clone())
                                .checked(selected)
                                .on_click(move |_, window, cx| {
                                    view.update(cx, |view, cx| {
                                        view.set_form_value(&field_name, &export, window, cx);
                                        view.capture_form_edits(cx);
                                    });
                                }),
                        );
                    }
                    menu
                },
            ),
    )
}

fn display_choice_label(options: &[(String, String)], value: &str) -> String {
    options
        .iter()
        .find(|(export, _)| export == value)
        .map_or_else(|| value.to_owned(), |(_, label)| label.clone())
}
