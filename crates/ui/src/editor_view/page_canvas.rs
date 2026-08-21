use gpui::prelude::FluentBuilder;
use gpui::{
    Context, CursorStyle, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, Window, canvas, div, img, px, rgb, rgba,
};
use gpui_component::Disableable;
use gpui_component::button::Button;
use gpui_component::input::Input;
use pdf_engine::{EditCommand, FormFieldKind, ShapeKind};

use crate::EditorView;

use super::geometry::{RENDER_SCALE, overlay_point, overlay_rect, ui_f32};
use super::model::{DragState, OverlayRect, Tool};
use crate::theme::{PAGE_TEXT, TEXT_FAINT, TEXT_MUTED, solid, tint};

impl EditorView {
    pub(super) fn render_document(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        if self.pages.is_empty() {
            return div()
                .flex()
                .flex_1()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_2()
                .child(div().text_color(tint(TEXT_MUTED)).child("No document open"))
                .child(
                    div()
                        .text_sm()
                        .text_color(tint(TEXT_FAINT))
                        .child("Press Cmd+O, or open a PDF from Finder"),
                )
                .into_any_element();
        }

        let mut scroll = div()
            .id("document-scroll")
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            // Pages centre themselves inside a full-width row instead of being
            // centred by the column. Cross-axis centring would push a page that
            // is wider than the viewport half-way off the left edge, where the
            // scroll offset cannot reach it.
            .items_start()
            .gap_6()
            .p_8()
            .overflow_scroll()
            .track_scroll(&self.scroll)
            .on_scroll_wheel(cx.listener(Self::document_scroll_wheel))
            .on_pinch(cx.listener(Self::document_pinch));
        scroll.style().allow_concurrent_scroll = Some(true);
        for page_index in 0..self.pages.len() {
            scroll = scroll.child(
                div()
                    .flex()
                    .flex_row()
                    .flex_none()
                    .justify_center()
                    .min_w_full()
                    .child(self.render_page(page_index, cx)),
            );
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
            .bg(solid(0x00ff_ffff))
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
                    .text_color(tint(TEXT_MUTED))
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
        element = self.add_edit_overlays(element, page_index, cx);
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
                            .when(item.field.read_only, gpui::Styled::cursor_not_allowed)
                            .when(!item.field.read_only, gpui::Styled::cursor_pointer)
                            .on_click(cx.listener(
                                move |view, _, window, cx| {
                                    view.toggle_form_button(&name, window, cx);
                                },
                            )),
                        ),
                    );
                } else {
                    let disabled =
                        item.field.read_only || item.field.kind == FormFieldKind::Signature;
                    // Field text has to scale with the page, otherwise values
                    // overflow their widget rect at anything but 100% zoom.
                    let font_size = (rect.height * 0.6).clamp(6.0, 40.0);
                    page = page.child(
                        positioned(rect)
                            .rounded_sm()
                            .bg(if disabled {
                                rgba(0x9ca3_ad1a)
                            } else {
                                rgba(0x4e9c_ff2e)
                            })
                            .child(
                                Input::new(&item.input)
                                    .disabled(disabled)
                                    .size_full()
                                    .appearance(false)
                                    .bordered(false)
                                    .focus_bordered(false)
                                    .px(px(2.0))
                                    .py(px(0.0))
                                    .text_color(solid(PAGE_TEXT))
                                    .text_size(px(font_size)),
                            ),
                    );
                }
            }
        }
        page
    }

    fn add_selection_overlays(&self, mut page: gpui::Div, page_index: usize) -> gpui::Div {
        let geometry = self.pages[page_index].metadata.geometry;
        // Selection can span pages, so each page paints its own runs.
        for run in self
            .selection
            .iter()
            .filter(|run| run.page_index == page_index)
        {
            page = page.child(
                positioned(overlay_rect(run.rect, geometry, self.zoom)).bg(rgba(0x3b82_f655)),
            );
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
                    .bg(rgba(0x3b82_f622)),
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

    #[allow(clippy::too_many_lines)]
    fn add_edit_overlays(
        &self,
        mut page: gpui::Div,
        page_index: usize,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let geometry = self.pages[page_index].metadata.geometry;
        for (edit_index, edit) in self.history.iter().enumerate() {
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
                        positioned(overlay_rect(*rect, geometry, self.zoom))
                            .id(("redact-edit", edit_index))
                            .bg(rgba(0x1118_27cc))
                            .cursor_pointer()
                            .when(self.selected_edit == Some(edit_index), |redaction| {
                                redaction.border_2().border_color(rgb(0x003b_82f6))
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |view, _, _, cx| {
                                    view.select_edit(edit_index, cx);
                                    cx.stop_propagation();
                                }),
                            ),
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
                    contents,
                    color,
                } if *target == page_index => {
                    let (left, top) =
                        overlay_point(document_core::PdfPoint::new(*x, *y), geometry, self.zoom);
                    let scale = ui_f32(RENDER_SCALE) * self.zoom;
                    page = page.child(
                        div()
                            .absolute()
                            .left(px(left))
                            .top(px(top - 72.0 * scale))
                            .w(px(160.0 * scale))
                            .h(px(72.0 * scale))
                            .p(px(4.0 * scale))
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(color_to_u32(*color)))
                            .bg(rgb(0x00ff_f8dc))
                            .text_size(px(display_text_size(9.0, self.zoom)))
                            .text_color(rgb(0x001f_2937))
                            .flex()
                            .items_start()
                            .cursor_pointer()
                            .child(contents.clone())
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |view, _, window, cx| {
                                    view.edit_note_overlay(edit_index, window, cx);
                                }),
                            ),
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
                        .id(("shape-edit", edit_index))
                        .border_2()
                        .border_color(rgb(color_to_u32(*color)))
                        .cursor_pointer()
                        .when(self.selected_edit == Some(edit_index), |shape| {
                            shape.bg(rgba(0x3b82_f622))
                        })
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |view, _, _, cx| {
                                view.select_edit(edit_index, cx);
                                cx.stop_propagation();
                            }),
                        );
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
                            .cursor_pointer()
                            .child(stamp.text.clone())
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |view, _, window, cx| {
                                    view.edit_text_overlay(edit_index, window, cx);
                                }),
                            ),
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
                                .h_full()
                                .px(px(2.0 * scale))
                                .py(px(0.0))
                                .appearance(false)
                                .bordered(false)
                                .focus_bordered(false)
                                .text_color(solid(PAGE_TEXT))
                                .text_size(px(font_size)),
                        ),
                ),
        )
    }

    fn add_inline_note(
        &self,
        page: gpui::Div,
        page_index: usize,
        cx: &mut Context<Self>,
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
                .on_mouse_move(cx.listener(Self::inline_note_mouse_move))
                .on_mouse_up(MouseButton::Left, cx.listener(Self::inline_note_mouse_up))
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
                        .cursor(if matches!(self.drag, Some(DragState::InlineNote { .. })) {
                            CursorStyle::ClosedHand
                        } else {
                            CursorStyle::OpenHand
                        })
                        .text_size(px(10.0 * scale))
                        .text_color(rgb(0x00b4_6d00))
                        .child("⠿")
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |view, event, window, cx| {
                                view.inline_note_mouse_down(page_index, event, window, cx);
                            }),
                        ),
                )
                .child(
                    Input::new(&note.input)
                        .size_full()
                        .pl(px(14.0 * scale))
                        .py(px(0.0))
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false)
                        .text_color(solid(PAGE_TEXT))
                        .text_size(px(display_text_size(9.0, self.zoom))),
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
        | Tool::Edit
        | Tool::Highlight
        | Tool::Underline
        | Tool::Strikeout
        | Tool::AddText
        | Tool::Note
        | Tool::Signature => CursorStyle::IBeam,
        Tool::Hand => CursorStyle::OpenHand,
        Tool::Shape | Tool::Redact => CursorStyle::Crosshair,
    }
}

fn display_text_size(size: f64, zoom: f32) -> f32 {
    ui_f32(size) * ui_f32(RENDER_SCALE) * zoom
}

fn positioned(rect: OverlayRect) -> gpui::Div {
    div()
        .absolute()
        .left(px(rect.left))
        .top(px(rect.top))
        .w(px(rect.width.max(1.0)))
        .h(px(rect.height.max(1.0)))
}

#[allow(clippy::cast_possible_truncation)]
fn highlight_preview(color: (f64, f64, f64)) -> gpui::Rgba {
    gpui::Rgba {
        r: color.0 as f32,
        g: color.1 as f32,
        b: color.2 as f32,
        a: 0.35,
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn color_to_u32(color: (f64, f64, f64)) -> u32 {
    let channel = |value: f64| (value.clamp(0.0, 1.0) * 255.0).round() as u32;
    (channel(color.0) << 16) | (channel(color.1) << 8) | channel(color.2)
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
