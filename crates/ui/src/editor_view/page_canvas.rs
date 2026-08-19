use gpui::{
    Context, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, Window, canvas, div, img, px, rgb, rgba,
};
use gpui_component::Disableable;
use gpui_component::button::Button;
use gpui_component::input::Input;
use pdf_engine::{EditCommand, FormFieldKind};

use crate::EditorView;

use super::geometry::{overlay_point, overlay_rect, ui_f32};
use super::model::{DragState, OverlayRect};

impl EditorView {
    pub(super) fn render_document(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        if self.pages.is_empty() {
            return div()
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .text_color(rgb(0x006b_7076))
                .child("Open a PDF with Cmd+O or from Finder")
                .into_any_element();
        }

        let mut scroll = div()
            .id("document-scroll")
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .items_center()
            .gap_6()
            .p_8()
            .overflow_scroll()
            .track_scroll(&self.scroll)
            .on_scroll_wheel(cx.listener(Self::document_scroll_wheel))
            .on_pinch(cx.listener(Self::document_pinch));
        scroll.style().allow_concurrent_scroll = Some(true);
        for page_index in 0..self.pages.len() {
            scroll = scroll.child(self.render_page(page_index, cx));
        }
        scroll.into_any_element()
    }

    fn render_page(&self, page_index: usize, cx: &mut Context<Self>) -> gpui::Div {
        let page = &self.pages[page_index];
        let width = page.image_size.0 * self.zoom;
        let height = page.image_size.1 * self.zoom;
        let bounds = page.bounds.clone();
        let mut element = div()
            .relative()
            .w(px(width))
            .h(px(height))
            .flex_none()
            .bg(rgb(0x00ff_ffff))
            .shadow_xl()
            .child(
                canvas(
                    move |page_bounds, _, _| bounds.set(page_bounds),
                    |_, (), _, _| {},
                )
                .absolute()
                .top(px(0.0))
                .left(px(0.0))
                .size_full(),
            );
        if let Some(image) = page.image.clone() {
            element = element.child(img(image).size_full());
        } else {
            element = element.child(
                div()
                    .flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(0x0080_858b))
                    .child(format!("Loading page {}…", page_index + 1)),
            );
        }
        element = self.add_edit_overlays(element, page_index);
        element = self.add_selection_overlays(element, page_index);
        element = self.add_form_overlays(element, page_index, cx);
        element = self.add_inline_text(element, page_index);
        element
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, event, window, cx| {
                    view.page_mouse_down(page_index, event, window, cx);
                }),
            )
            .on_mouse_move(cx.listener(Self::page_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::page_mouse_up))
    }

    fn add_form_overlays(
        &self,
        mut page: gpui::Div,
        page_index: usize,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let geometry = self.pages[page_index].metadata.geometry;
        for (field_index, item) in self.forms.iter().enumerate() {
            for (widget_index, widget) in item
                .field
                .widgets
                .iter()
                .filter(|widget| widget.page_index == page_index)
                .enumerate()
            {
                let rect = overlay_rect(widget.rect, geometry, self.zoom);
                if item.field.kind == FormFieldKind::Button {
                    let name = item.field.name.clone();
                    let checked = !matches!(item.value(cx).as_str(), "Off" | "false" | "");
                    page = page.child(
                        positioned(rect).child(
                            Button::new((
                                "form-button",
                                field_index.saturating_mul(1000) + widget_index,
                            ))
                            .label(if checked { "✓" } else { "" })
                            .disabled(item.field.read_only)
                            .on_click(cx.listener(
                                move |view, _, window, cx| {
                                    view.toggle_form_button(&name, window, cx);
                                },
                            )),
                        ),
                    );
                } else {
                    page = page.child(positioned(rect).child(Input::new(&item.input).disabled(
                        item.field.read_only || item.field.kind == FormFieldKind::Signature,
                    )));
                }
            }
        }
        page
    }

    fn add_selection_overlays(&self, mut page: gpui::Div, page_index: usize) -> gpui::Div {
        if page_index != self.page_index {
            return page;
        }
        let geometry = self.pages[page_index].metadata.geometry;
        for rect in &self.selected_rects {
            page = page
                .child(positioned(overlay_rect(*rect, geometry, self.zoom)).bg(rgba(0x553b_82f6)));
        }
        if let Some(DragState::Region {
            page_index: drag_page,
            ..
        }) = &self.drag
            && *drag_page == page_index
            && let Some(rect) = self.drag.as_ref().and_then(DragState::rect)
        {
            page = page.child(
                positioned(overlay_rect(rect, geometry, self.zoom))
                    .border_2()
                    .border_color(rgb(0x003b_82f6))
                    .bg(rgba(0x223b_82f6)),
            );
        }
        page
    }

    fn add_edit_overlays(&self, mut page: gpui::Div, page_index: usize) -> gpui::Div {
        let geometry = self.pages[page_index].metadata.geometry;
        for edit in &self.edits {
            match edit {
                EditCommand::Highlight {
                    page_index: target,
                    rects,
                    color,
                } if *target == page_index => {
                    for rect in rects {
                        page = page.child(
                            positioned(overlay_rect(*rect, geometry, self.zoom))
                                .bg(highlight_preview(*color)),
                        );
                    }
                }
                EditCommand::Redact {
                    page_index: target,
                    rect,
                } if *target == page_index => {
                    page = page.child(
                        positioned(overlay_rect(*rect, geometry, self.zoom)).bg(rgba(0xcc11_1827)),
                    );
                }
                EditCommand::AddText(stamp) if stamp.page_index == page_index => {
                    let (left, top) = overlay_point(
                        document_core::PdfPoint::new(stamp.x, stamp.y),
                        geometry,
                        self.zoom,
                    );
                    page = page.child(
                        div()
                            .absolute()
                            .left(px(left))
                            .top(px(top - ui_f32(stamp.size) * self.zoom))
                            .text_size(px(ui_f32(stamp.size) * self.zoom))
                            .text_color(rgb(0x0000_0000))
                            .child(stamp.text.clone()),
                    );
                }
                _ => {}
            }
        }
        page
    }

    fn add_inline_text(&self, page: gpui::Div, page_index: usize) -> gpui::Div {
        let Some(inline) = self
            .inline_text
            .as_ref()
            .filter(|inline| inline.page_index == page_index)
        else {
            return page;
        };
        let geometry = self.pages[page_index].metadata.geometry;
        let (left, top) = overlay_point(inline.point, geometry, self.zoom);
        page.child(
            div()
                .absolute()
                .left(px(left))
                .top(px(top - 24.0))
                .w_56()
                .h_9()
                .bg(rgb(0x00ff_ffff))
                .border_2()
                .border_color(rgb(0x003b_82f6))
                .child(Input::new(&inline.input)),
        )
    }

    fn toggle_form_button(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(item) = self.forms.iter().find(|item| item.field.name == name) {
            let next = if matches!(item.value(cx).as_str(), "Off" | "false" | "") {
                "true"
            } else {
                "false"
            };
            item.input
                .update(cx, |state, cx| state.set_value(next, window, cx));
            self.status = format!("Updated {name}").into();
            cx.notify();
        }
    }
}

fn positioned(rect: OverlayRect) -> gpui::Div {
    div()
        .absolute()
        .left(px(rect.left))
        .top(px(rect.top))
        .w(px(rect.width.max(1.0)))
        .h(px(rect.height.max(1.0)))
}

fn highlight_preview(color: (f64, f64, f64)) -> gpui::Rgba {
    if color.0 < 0.5 {
        rgba(0x6678_d98b)
    } else if color.1 < 0.6 {
        rgba(0x66ff_7bab)
    } else {
        rgba(0x66ff_dc33)
    }
}
