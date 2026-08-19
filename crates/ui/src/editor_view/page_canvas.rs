use gpui::{
    Context, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, Window, canvas, div, img, px, rgb, rgba,
};
use gpui_component::Disableable;
use gpui_component::button::Button;
use gpui_component::input::Input;
use gpui_component::scroll::ScrollableElement;
use pdf_engine::{EditCommand, FormFieldKind};

use crate::EditorView;

use super::geometry::{overlay_point, overlay_rect, raster_f32, ui_f32};
use super::model::OverlayRect;

impl EditorView {
    pub(super) fn render_document(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some(image) = self.image.clone() else {
            return div()
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .text_color(rgb(0x006b_7076))
                .child("Open a PDF with Cmd+O or from Finder")
                .into_any_element();
        };
        let width = raster_f32(self.image_size.0) * self.zoom;
        let height = raster_f32(self.image_size.1) * self.zoom;
        let page = self.render_page(image, width, height, cx);
        div()
            .id("document-scroll")
            .flex()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_scroll()
            .track_scroll(&self.scroll)
            .vertical_scrollbar(&self.scroll)
            .horizontal_scrollbar(&self.scroll)
            .child(
                div()
                    .flex()
                    .min_w_full()
                    .min_h_full()
                    .p_8()
                    .items_center()
                    .justify_center()
                    .child(page),
            )
            .into_any_element()
    }

    fn render_page(
        &self,
        image: std::sync::Arc<gpui::RenderImage>,
        width: f32,
        height: f32,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let bounds = self.page_bounds.clone();
        let mut page = div()
            .relative()
            .w(px(width))
            .h(px(height))
            .flex_none()
            .bg(rgb(0x00ff_ffff))
            .shadow_xl()
            .child(img(image).size_full())
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
        page = self.add_edit_overlays(page);
        page = self.add_selection_overlays(page);
        page = self.add_form_overlays(page, cx);
        page = self.add_inline_text(page);
        page.on_mouse_down(MouseButton::Left, cx.listener(Self::page_mouse_down))
            .on_mouse_move(cx.listener(Self::page_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::page_mouse_up))
    }

    fn add_form_overlays(&self, mut page: gpui::Div, cx: &mut Context<Self>) -> gpui::Div {
        let Some(geometry) = self.page_geometry else {
            return page;
        };
        for (field_index, item) in self.forms.iter().enumerate() {
            for (widget_index, widget) in item
                .field
                .widgets
                .iter()
                .filter(|widget| widget.page_index == self.page_index)
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

    fn add_selection_overlays(&self, mut page: gpui::Div) -> gpui::Div {
        let Some(geometry) = self.page_geometry else {
            return page;
        };
        for rect in &self.selected_rects {
            page = page
                .child(positioned(overlay_rect(*rect, geometry, self.zoom)).bg(rgba(0x553b_82f6)));
        }
        if let Some(rect) = self.drag.as_ref().and_then(super::model::DragState::rect) {
            page = page.child(
                positioned(overlay_rect(rect, geometry, self.zoom))
                    .border_2()
                    .border_color(rgb(0x003b_82f6))
                    .bg(rgba(0x223b_82f6)),
            );
        }
        page
    }

    fn add_edit_overlays(&self, mut page: gpui::Div) -> gpui::Div {
        let Some(geometry) = self.page_geometry else {
            return page;
        };
        for edit in &self.edits {
            match edit {
                EditCommand::Highlight {
                    page_index,
                    rects,
                    color,
                } if *page_index == self.page_index => {
                    for rect in rects {
                        page = page.child(
                            positioned(overlay_rect(*rect, geometry, self.zoom))
                                .bg(highlight_preview(*color)),
                        );
                    }
                }
                EditCommand::Redact { page_index, rect } if *page_index == self.page_index => {
                    page = page.child(
                        positioned(overlay_rect(*rect, geometry, self.zoom)).bg(rgba(0xcc11_1827)),
                    );
                }
                EditCommand::AddText(stamp) if stamp.page_index == self.page_index => {
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

    fn add_inline_text(&self, page: gpui::Div) -> gpui::Div {
        let (Some(geometry), Some(inline)) = (self.page_geometry, &self.inline_text) else {
            return page;
        };
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
