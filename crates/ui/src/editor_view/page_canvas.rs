use gpui::prelude::FluentBuilder;
use gpui::{
    Context, CursorStyle, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, Window, canvas, div, img, px, rgb, rgba,
};
use gpui_component::button::Button;
use gpui_component::input::Input;
use gpui_component::{Disableable, Sizable, Size};
use pdf_engine::{EditCommand, FormFieldKind, ShapeKind};

use crate::EditorView;

use super::geometry::{RENDER_SCALE, overlay_point, overlay_rect, ui_f32};
use super::model::{DragState, OverlayRect, Tool};

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
        element = element.child(
            div()
                .absolute()
                .top(px(0.0))
                .left(px(0.0))
                .size_full()
                .cursor(self.page_cursor())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |view, event, window, cx| {
                        view.page_mouse_down(page_index, event, window, cx);
                    }),
                )
                .on_mouse_move(cx.listener(Self::page_mouse_move))
                .on_mouse_up(MouseButton::Left, cx.listener(Self::page_mouse_up)),
        );
        element = self.add_search_overlays(element, page_index);
        element = self.add_edit_overlays(element, page_index);
        element = self.add_selection_overlays(element, page_index);
        element = self.add_form_overlays(element, page_index, cx);
        element = self.add_inline_text(element, page_index, cx);
        element = self.add_inline_note(element, page_index, cx);
        element
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
                            .when(item.field.read_only, |button| button.cursor_not_allowed())
                            .when(!item.field.read_only, |button| button.cursor_pointer())
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
                .child(positioned(overlay_rect(*rect, geometry, self.zoom)).bg(rgba(0x3b82_f655)));
        }
        if let Some(DragState::Region {
            page_index: drag_page,
            ..
        }) = &self.drag
            && *drag_page == page_index
            && let Some(rect) = self.drag.as_ref().and_then(DragState::rect)
            && !matches!(
                self.tool,
                super::model::Tool::Select
                    | super::model::Tool::Highlight
                    | super::model::Tool::Underline
                    | super::model::Tool::Strikeout
            )
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

    fn add_search_overlays(&self, mut page: gpui::Div, page_index: usize) -> gpui::Div {
        let geometry = self.pages[page_index].metadata.geometry;
        for (index, result) in self.search_matches.iter().enumerate() {
            if result.page_index != page_index {
                continue;
            }
            let color = if index == self.search_index {
                rgba(0xffa9_005f)
            } else {
                rgba(0xffe0_7a45)
            };
            page = page.child(positioned(overlay_rect(result.rect, geometry, self.zoom)).bg(color));
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
                EditCommand::Underline {
                    page_index: target,
                    rects,
                    color,
                } if *target == page_index => {
                    for rect in rects {
                        let rect = overlay_rect(*rect, geometry, self.zoom);
                        page = page.child(
                            div()
                                .absolute()
                                .left(px(rect.left))
                                .top(px(rect.top + rect.height - 2.0))
                                .w(px(rect.width.max(1.0)))
                                .h(px(2.0))
                                .bg(rgb(color_to_u32(*color))),
                        );
                    }
                }
                EditCommand::StrikeOut {
                    page_index: target,
                    rects,
                    color,
                } if *target == page_index => {
                    for rect in rects {
                        let rect = overlay_rect(*rect, geometry, self.zoom);
                        page = page.child(
                            div()
                                .absolute()
                                .left(px(rect.left))
                                .top(px(rect.top + rect.height / 2.0 - 1.0))
                                .w(px(rect.width.max(1.0)))
                                .h(px(2.0))
                                .bg(rgb(color_to_u32(*color))),
                        );
                    }
                }
                EditCommand::Note {
                    page_index: target,
                    x,
                    y,
                    contents: _,
                    color,
                } if *target == page_index => {
                    let (left, top) =
                        overlay_point(document_core::PdfPoint::new(*x, *y), geometry, self.zoom);
                    page = page.child(
                        div()
                            .absolute()
                            .left(px(left))
                            .top(px(top - 18.0))
                            .size_5()
                            .rounded_full()
                            .bg(rgb(color_to_u32(*color)))
                            .text_xs()
                            .text_color(rgb(0x00ff_ffff))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child("•"),
                    );
                }
                EditCommand::Shape {
                    page_index: target,
                    kind,
                    rect,
                    color,
                    ..
                } if *target == page_index => {
                    let element = positioned(overlay_rect(*rect, geometry, self.zoom))
                        .border_2()
                        .border_color(rgb(color_to_u32(*color)));
                    page = page.child(match kind {
                        ShapeKind::Rectangle => element,
                        ShapeKind::Ellipse => element.rounded_full(),
                    });
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
                            .top(px(top - display_text_size(stamp.size, self.zoom)))
                            .text_size(px(display_text_size(stamp.size, self.zoom)))
                            .text_color(rgb(0x0000_0000))
                            .child(stamp.text.clone()),
                    );
                }
                _ => {}
            }
        }
        page
    }

    fn add_inline_text(
        &self,
        page: gpui::Div,
        page_index: usize,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let Some(inline) = self
            .inline_text
            .as_ref()
            .filter(|inline| inline.page_index == page_index)
        else {
            return page;
        };
        let geometry = self.pages[page_index].metadata.geometry;
        let (left, top) = overlay_point(inline.point, geometry, self.zoom);
        let zoom = self.zoom;
        let scale = ui_f32(RENDER_SCALE) * zoom;
        let font_size = display_text_size(14.0, zoom);
        page.child(
            div()
                .absolute()
                .left(px(left))
                .top(px(top - 20.0 * scale))
                .w(px(160.0 * scale))
                .h(px(20.0 * scale))
                .border_1()
                .border_color(rgb(0x003b_82f6))
                .occlude()
                .on_mouse_move(cx.listener(Self::inline_text_mouse_move))
                .on_mouse_up(MouseButton::Left, cx.listener(Self::inline_text_mouse_up))
                .child(
                    div()
                        .absolute()
                        .left(px(0.0))
                        .top(px(0.0))
                        .w(px(12.0 * scale))
                        .h_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor(if matches!(self.drag, Some(DragState::InlineText { .. })) {
                            CursorStyle::ClosedHand
                        } else {
                            CursorStyle::OpenHand
                        })
                        .text_size(px(10.0 * scale))
                        .text_color(rgb(0x003b_82f6))
                        .child("⠿")
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |view, event, window, cx| {
                                view.inline_text_mouse_down(page_index, event, window, cx);
                            }),
                        ),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(12.0 * scale))
                        .top(px(0.0))
                        .right(px(0.0))
                        .h_full()
                        .child(
                            Input::new(&inline.input)
                                .with_size(Size::Size(px(font_size / 0.875)))
                                .h_full()
                                .px(px(2.0 * scale))
                                .py(px(scale))
                                .appearance(false)
                                .bordered(false)
                                .focus_bordered(false)
                                .text_size(px(font_size)),
                        ),
                ),
        )
    }

    fn add_inline_note(
        &self,
        page: gpui::Div,
        page_index: usize,
        _cx: &mut Context<Self>,
    ) -> gpui::Div {
        let Some(note) = self
            .inline_note
            .as_ref()
            .filter(|note| note.page_index == page_index)
        else {
            return page;
        };
        let geometry = self.pages[page_index].metadata.geometry;
        let (left, top) = overlay_point(note.point, geometry, self.zoom);
        let scale = ui_f32(RENDER_SCALE) * self.zoom;
        page.child(
            div()
                .absolute()
                .left(px(left))
                .top(px(top - 72.0 * scale))
                .w(px(160.0 * scale))
                .h(px(72.0 * scale))
                .p(px(4.0 * scale))
                .rounded_md()
                .border_1()
                .border_color(rgb(0x00f5_b942))
                .bg(rgb(0x00ff_f8dc))
                .occlude()
                .child(
                    Input::new(&note.input)
                        .with_size(Size::Size(px(12.0 * scale / 0.875)))
                        .size_full()
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false)
                        .text_color(rgb(0x001f_2937)),
                ),
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

    fn page_cursor(&self) -> CursorStyle {
        cursor_for_tool(self.tool, matches!(self.drag, Some(DragState::Pan { .. })))
    }
}

fn cursor_for_tool(tool: Tool, panning: bool) -> CursorStyle {
    if panning {
        return CursorStyle::ClosedHand;
    }

    match tool {
        Tool::Select
        | Tool::Highlight
        | Tool::Underline
        | Tool::Strikeout
        | Tool::AddText
        | Tool::Note => CursorStyle::IBeam,
        Tool::Hand => CursorStyle::OpenHand,
        Tool::Shape | Tool::Redact => CursorStyle::Crosshair,
    }
}

fn display_text_size(size: f64, zoom: f32) -> f32 {
    ui_f32(size) * ui_f32(RENDER_SCALE) * zoom
}

#[cfg(test)]
mod tests {
    use super::{CursorStyle, Tool, cursor_for_tool, display_text_size};

    #[test]
    fn text_size_uses_raster_scale_and_zoom() {
        assert!((display_text_size(14.0, 1.0) - 21.0).abs() < f32::EPSILON);
        assert!((display_text_size(14.0, 2.0) - 42.0).abs() < f32::EPSILON);
    }

    #[test]
    fn page_cursor_matches_each_tool_and_pan_state() {
        for tool in [
            Tool::Select,
            Tool::Highlight,
            Tool::Underline,
            Tool::Strikeout,
            Tool::AddText,
            Tool::Note,
        ] {
            assert_eq!(cursor_for_tool(tool, false), CursorStyle::IBeam);
        }

        assert_eq!(cursor_for_tool(Tool::Hand, false), CursorStyle::OpenHand);
        assert_eq!(cursor_for_tool(Tool::Hand, true), CursorStyle::ClosedHand);

        for tool in [Tool::Shape, Tool::Redact] {
            assert_eq!(cursor_for_tool(tool, false), CursorStyle::Crosshair);
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
    gpui::Rgba {
        r: color.0 as f32,
        g: color.1 as f32,
        b: color.2 as f32,
        a: 0.35,
    }
}

fn color_to_u32(color: (f64, f64, f64)) -> u32 {
    let channel = |value: f64| (value.clamp(0.0, 1.0) * 255.0).round() as u32;
    (channel(color.0) << 16) | (channel(color.1) << 8) | channel(color.2)
}
