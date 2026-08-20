use gpui::{
    ClipboardItem, Context, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PinchEvent,
    ScrollWheelEvent, Window, point, px,
};
use pdf_engine::{EditCommand, TextStamp};

use crate::actions::{
    ActualSize, AddTextTool, CommitNote, CommitText, CopySelection, FitPage, HandTool,
    HighlightTool, NoteTool, RedactTool, SelectTool, ShapeTool, StrikeoutTool, UnderlineTool,
    ZoomIn, ZoomOut,
};
use crate::editor_view::inline_text_input;

use super::EditorView;
use super::geometry::page_point;
use super::gestures::{AnchorContext, DocumentMetrics, anchored_document_offset, pinch_zoom};
use super::model::{DragState, InlineNote, InlineText, Tool};

impl EditorView {
    pub(super) fn select_tool(&mut self, _: &SelectTool, _: &mut Window, cx: &mut Context<Self>) {
        self.activate_tool(Tool::Select, cx);
    }

    pub(super) fn hand_tool(&mut self, _: &HandTool, _: &mut Window, cx: &mut Context<Self>) {
        self.activate_tool(Tool::Hand, cx);
    }

    pub(super) fn highlight_tool(
        &mut self,
        _: &HighlightTool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activate_tool(Tool::Highlight, cx);
    }

    pub(super) fn add_text_tool(
        &mut self,
        _: &AddTextTool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activate_tool(Tool::AddText, cx);
    }

    pub(super) fn underline_tool(
        &mut self,
        _: &UnderlineTool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activate_tool(Tool::Underline, cx);
    }

    pub(super) fn strikeout_tool(
        &mut self,
        _: &StrikeoutTool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activate_tool(Tool::Strikeout, cx);
    }

    pub(super) fn note_tool(&mut self, _: &NoteTool, _: &mut Window, cx: &mut Context<Self>) {
        self.activate_tool(Tool::Note, cx);
    }

    pub(super) fn shape_tool(&mut self, _: &ShapeTool, _: &mut Window, cx: &mut Context<Self>) {
        self.activate_tool(Tool::Shape, cx);
    }

    pub(super) fn redact_tool(&mut self, _: &RedactTool, _: &mut Window, cx: &mut Context<Self>) {
        self.activate_tool(Tool::Redact, cx);
    }

    fn activate_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        self.tool = tool;
        self.drag = None;
        self.inline_text = None;
        self.inline_note = None;
        self.status = format!("{} tool active", tool.label()).into();
        cx.notify();
    }

    pub(super) fn zoom_in(&mut self, _: &ZoomIn, _: &mut Window, cx: &mut Context<Self>) {
        self.set_zoom(self.zoom + 0.25, cx);
    }

    pub(super) fn zoom_out(&mut self, _: &ZoomOut, _: &mut Window, cx: &mut Context<Self>) {
        self.set_zoom(self.zoom - 0.25, cx);
    }

    pub(super) fn actual_size(&mut self, _: &ActualSize, _: &mut Window, cx: &mut Context<Self>) {
        self.set_zoom(1.0, cx);
    }

    pub(super) fn fit_page(&mut self, _: &FitPage, _: &mut Window, cx: &mut Context<Self>) {
        let viewport = self.scroll.bounds().size;
        let Some(page) = self.pages.get(self.page_index) else {
            return;
        };
        let (width, height) = page.image_size;
        let x = (f32::from(viewport.width) - 64.0) / width;
        let y = (f32::from(viewport.height) - 64.0) / height;
        self.set_zoom(x.min(y), cx);
    }

    fn set_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        self.zoom = zoom.clamp(0.25, 4.0);
        self.status = format!("Zoom {}%", (self.zoom * 100.0).round()).into();
        cx.notify();
    }

    pub(super) fn document_scroll_wheel(
        &mut self,
        _: &ScrollWheelEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sync_current_page_from_scroll();
        self.refresh_active_page();
        cx.notify();
    }

    pub(super) fn document_pinch(
        &mut self,
        event: &PinchEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let old_zoom = self.zoom;
        let new_zoom = pinch_zoom(old_zoom, event.delta);
        if (new_zoom - old_zoom).abs() < f32::EPSILON {
            return;
        }

        let page_index = self
            .pages
            .iter()
            .position(|page| page.bounds.get().contains(&event.position))
            .unwrap_or(self.page_index);
        let Some(page) = self.pages.get(page_index) else {
            return;
        };
        let ratio = new_zoom / old_zoom;
        let viewport = self.scroll.bounds();
        let metrics = DocumentMetrics {
            page_count: self.pages.len(),
            max_page_width: self
                .pages
                .iter()
                .map(|page| page.image_size.0)
                .fold(0.0, f32::max),
            total_page_height: self.pages.iter().map(|page| page.image_size.1).sum(),
            prior_page_height: self.pages[..page_index]
                .iter()
                .map(|page| page.image_size.1)
                .sum(),
            page_size: page.image_size,
        };
        let (x, y) = anchored_document_offset(
            metrics,
            AnchorContext {
                page_index,
                zoom: new_zoom,
                ratio,
                viewport_origin: (f32::from(viewport.origin.x), f32::from(viewport.origin.y)),
                viewport_size: (
                    f32::from(viewport.size.width),
                    f32::from(viewport.size.height),
                ),
                pointer: (f32::from(event.position.x), f32::from(event.position.y)),
                page_origin: (
                    f32::from(page.bounds.get().origin.x),
                    f32::from(page.bounds.get().origin.y),
                ),
            },
        );
        self.scroll.set_offset(point(px(x), px(y)));
        self.set_zoom(new_zoom, cx);
    }

    pub(super) fn page_mouse_down(
        &mut self,
        page_index: usize,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .pages
            .get(page_index)
            .and_then(|page| page.image.as_ref())
            .is_none()
        {
            return;
        }
        self.set_current_page(page_index);
        if self.tool == Tool::Hand {
            self.drag = Some(DragState::Pan {
                start: event.position,
                offset: self.scroll.offset(),
            });
            cx.notify();
            return;
        }
        let Some(point) = self.pdf_point(page_index, event.position) else {
            return;
        };
        if self.tool == Tool::AddText {
            let text = inline_text_input("Type on page", "", window, cx);
            text.update(cx, |state, cx| state.focus(window, cx));
            self.inline_text = Some(InlineText {
                page_index,
                point,
                input: text,
            });
        } else if self.tool == Tool::Note {
            let note = inline_text_input("Write comment", "", window, cx);
            note.update(cx, |state, cx| state.focus(window, cx));
            self.inline_note = Some(InlineNote {
                page_index,
                point,
                input: note,
            });
        } else {
            self.drag = Some(DragState::Region {
                page_index,
                start: point,
                current: point,
            });
        }
        cx.notify();
    }

    pub(super) fn inline_text_mouse_down(
        &mut self,
        page_index: usize,
        event: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(point) = self.pdf_point(page_index, event.position) else {
            return;
        };
        let Some(inline) = self
            .inline_text
            .as_ref()
            .filter(|inline| inline.page_index == page_index)
        else {
            return;
        };
        self.drag = Some(DragState::InlineText {
            page_index,
            start: point,
            point: inline.point,
        });
        cx.stop_propagation();
        cx.notify();
    }

    pub(super) fn inline_text_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.dragging() {
            self.move_inline_text(event.position, cx);
        }
    }

    pub(super) fn inline_text_mouse_up(
        &mut self,
        _: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.drag, Some(DragState::InlineText { .. })) {
            self.drag = None;
            cx.notify();
        }
    }

    pub(super) fn page_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let pointer = match &self.drag {
            Some(DragState::Region { page_index, .. }) => {
                self.pdf_point(*page_index, event.position)
            }
            Some(DragState::InlineText { page_index, .. }) => {
                self.pdf_point(*page_index, event.position)
            }
            _ => None,
        };
        match &mut self.drag {
            Some(DragState::Pan { start, offset }) => {
                let x = f32::from(offset.x) + f32::from(event.position.x - start.x);
                let y = f32::from(offset.y) + f32::from(event.position.y - start.y);
                self.scroll.set_offset(point(px(x), px(y)));
            }
            Some(DragState::Region { current, .. }) => {
                let Some(point) = pointer else {
                    return;
                };
                *current = point;
            }
            Some(DragState::InlineText { .. }) => {
                self.move_inline_text(event.position, cx);
                return;
            }
            None => return,
        }
        if let Some(DragState::Region {
            page_index,
            start,
            current,
        }) = self.drag.as_ref()
            && matches!(
                self.tool,
                Tool::Select | Tool::Highlight | Tool::Underline | Tool::Strikeout
            )
        {
            self.select_text_range(*page_index, *start, *current);
        }
        cx.notify();
    }

    pub(super) fn page_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.drag, Some(DragState::Pan { .. })) {
            self.drag = None;
            cx.notify();
            return;
        }
        if matches!(self.drag, Some(DragState::InlineText { .. })) {
            self.drag = None;
            cx.notify();
            return;
        }
        let Some(mut drag) = self.drag.take() else {
            return;
        };
        let page_index = match &drag {
            DragState::Region { page_index, .. } => *page_index,
            DragState::Pan { .. } | DragState::InlineText { .. } => return,
        };
        if let Some(point) = self.pdf_point(page_index, event.position)
            && let DragState::Region { current, .. } = &mut drag
        {
            *current = point;
        }
        let DragState::Region { start, current, .. } = drag else {
            return;
        };
        self.finish_region(page_index, start, current);
        cx.notify();
    }

    fn finish_region(
        &mut self,
        page_index: usize,
        start: document_core::PdfPoint,
        current: document_core::PdfPoint,
    ) {
        match self.tool {
            Tool::Select => self.select_text_range(page_index, start, current),
            Tool::Highlight => {
                self.select_text_range(page_index, start, current);
                self.highlight_selection(page_index);
            }
            Tool::Underline => {
                self.select_text_range(page_index, start, current);
                self.markup_selection(page_index, Markup::Underline);
            }
            Tool::Strikeout => {
                self.select_text_range(page_index, start, current);
                self.markup_selection(page_index, Markup::Strikeout);
            }
            Tool::Shape => {
                if let Some(rect) = (DragState::Region {
                    page_index,
                    start,
                    current,
                })
                .rect()
                {
                    self.edits.push(EditCommand::Shape {
                        page_index,
                        kind: self.shape_kind,
                        rect,
                        color: self.annotation_color,
                        width: 2.0,
                    });
                    self.status = "Shape queued; Save As to write annotation".into();
                }
            }
            Tool::Redact => {
                if let Some(rect) = (DragState::Region {
                    page_index,
                    start,
                    current,
                })
                .rect()
                {
                    self.edits.push(EditCommand::Redact { page_index, rect });
                    self.status = "Redaction queued; Save As to apply".into();
                }
            }
            Tool::Hand | Tool::AddText | Tool::Note => {}
        }
    }

    fn select_text_range(
        &mut self,
        page_index: usize,
        start: document_core::PdfPoint,
        current: document_core::PdfPoint,
    ) {
        let Some(page) = self.pages.get(page_index) else {
            return;
        };
        let selection = text_selection(&page.fragments, start, current);
        self.selected_rects = selection.iter().map(|(rect, _)| *rect).collect();
        self.selected_text = selection
            .iter()
            .map(|(_, text)| text.as_str())
            .collect::<String>()
            .into();
        self.status = if self.selected_rects.is_empty() {
            "No text selected".into()
        } else {
            format!("Selected {} characters", self.selected_text.chars().count()).into()
        };
    }

    fn highlight_selection(&mut self, page_index: usize) {
        self.markup_selection(page_index, Markup::Highlight);
    }

    fn markup_selection(&mut self, page_index: usize, markup: Markup) {
        if self.selected_rects.is_empty() {
            self.status = format!("No text under {} selection", markup.label()).into();
            return;
        }
        let rects = self.selected_rects.clone();
        let edit = match markup {
            Markup::Highlight => EditCommand::Highlight {
                page_index,
                rects,
                color: self.highlight_color,
            },
            Markup::Underline => EditCommand::Underline {
                page_index,
                rects,
                color: self.annotation_color,
            },
            Markup::Strikeout => EditCommand::StrikeOut {
                page_index,
                rects,
                color: self.annotation_color,
            },
        };
        self.edits.push(edit);
        self.selected_rects.clear();
        self.selected_text = "No text selected".into();
        self.status = format!("{} queued; Save As to write annotation", markup.label()).into();
    }

    pub(super) fn copy_selection(
        &mut self,
        _: &CopySelection,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_text.is_empty() || self.selected_text == "No text selected" {
            self.status = "No text selected to copy".into();
        } else {
            cx.write_to_clipboard(ClipboardItem::new_string(self.selected_text.to_string()));
            self.status = "Copied selected text".into();
        }
        cx.notify();
    }

    pub(super) fn commit_text(&mut self, _: &CommitText, _: &mut Window, cx: &mut Context<Self>) {
        let Some(inline) = self.inline_text.take() else {
            self.status = "Click page with Add text tool first".into();
            cx.notify();
            return;
        };
        let text = inline.input.read(cx).value().to_string();
        if text.trim().is_empty() {
            self.status = "Text cannot be empty".into();
        } else {
            self.edits.push(EditCommand::AddText(TextStamp {
                page_index: inline.page_index,
                text,
                x: inline.point.x,
                y: inline.point.y,
                size: 14.0,
            }));
            self.status = "Text queued; Save As to write it".into();
        }
        cx.notify();
    }

    pub(super) fn commit_note(&mut self, _: &CommitNote, _: &mut Window, cx: &mut Context<Self>) {
        let Some(note) = self.inline_note.take() else {
            self.status = "Click page with Comment tool first".into();
            cx.notify();
            return;
        };
        let contents = note.input.read(cx).value().trim().to_owned();
        if contents.is_empty() {
            self.status = "Comment cannot be empty".into();
        } else {
            self.edits.push(EditCommand::Note {
                page_index: note.page_index,
                x: note.point.x,
                y: note.point.y,
                contents,
                color: self.annotation_color,
            });
            self.status = "Comment queued; Save As to write annotation".into();
        }
        cx.notify();
    }

    fn pdf_point(
        &self,
        page_index: usize,
        position: gpui::Point<gpui::Pixels>,
    ) -> Option<document_core::PdfPoint> {
        let page = self.pages.get(page_index)?;
        page_point(
            position,
            page.bounds.get(),
            page.metadata.geometry,
            self.zoom,
        )
    }

    fn move_inline_text(&mut self, position: gpui::Point<gpui::Pixels>, cx: &mut Context<Self>) {
        let Some((page_index, start, original)) = self.drag.as_ref().and_then(|drag| match drag {
            DragState::InlineText {
                page_index,
                start,
                point,
            } => Some((*page_index, *start, *point)),
            _ => None,
        }) else {
            return;
        };
        let Some(current) = self.pdf_point(page_index, position) else {
            return;
        };
        if let Some(inline) = self.inline_text.as_mut() {
            inline.point = document_core::PdfPoint::new(
                original.x + current.x - start.x,
                original.y + current.y - start.y,
            );
            self.status = "Move text field".into();
            cx.notify();
        }
    }
}

#[derive(Clone, Copy)]
enum Markup {
    Highlight,
    Underline,
    Strikeout,
}

impl Markup {
    const fn label(self) -> &'static str {
        match self {
            Self::Highlight => "Highlight",
            Self::Underline => "Underline",
            Self::Strikeout => "Strike out",
        }
    }
}

fn text_selection(
    fragments: &[pdf_engine::TextFragment],
    start: document_core::PdfPoint,
    current: document_core::PdfPoint,
) -> Vec<(document_core::PdfRect, String)> {
    let Some(start_index) = nearest_fragment(fragments, start) else {
        return Vec::new();
    };
    let Some(end_index) = nearest_fragment(fragments, current) else {
        return Vec::new();
    };
    let forward = start_index <= end_index;
    let (first_index, last_index) = if forward {
        (start_index, end_index)
    } else {
        (end_index, start_index)
    };

    (first_index..=last_index)
        .map(|index| {
            let fragment = &fragments[index];
            (fragment.rect, fragment.text.clone())
        })
        .collect()
}

fn nearest_fragment(
    fragments: &[pdf_engine::TextFragment],
    point: document_core::PdfPoint,
) -> Option<usize> {
    fragments
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            point_distance(left.rect, point).total_cmp(&point_distance(right.rect, point))
        })
        .map(|(index, _)| index)
}

fn point_distance(rect: document_core::PdfRect, point: document_core::PdfPoint) -> f64 {
    let x = point.x.clamp(rect.x_min, rect.x_max);
    let y = point.y.clamp(rect.y_min, rect.y_max);
    (point.x - x).powi(2) + (point.y - y).powi(2)
}

#[cfg(test)]
mod tests {
    use document_core::{PdfPoint, PdfRect};
    use pdf_engine::TextFragment;

    use super::text_selection;

    #[test]
    fn text_selection_preserves_complete_fragment_text() {
        let fragments = vec![
            TextFragment {
                text: "Hello ".into(),
                rect: PdfRect::new(0.0, 0.0, 60.0, 10.0).unwrap(),
            },
            TextFragment {
                text: "world".into(),
                rect: PdfRect::new(60.0, 0.0, 110.0, 10.0).unwrap(),
            },
        ];

        let selected = text_selection(
            &fragments,
            PdfPoint::new(20.0, 5.0),
            PdfPoint::new(90.0, 5.0),
        );

        assert_eq!(
            selected
                .iter()
                .map(|(_, text)| text.as_str())
                .collect::<String>(),
            "Hello world"
        );
        assert_eq!(selected.len(), 2);
    }
}
