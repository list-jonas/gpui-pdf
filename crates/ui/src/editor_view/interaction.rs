use gpui::{
    Context, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PinchEvent, ScrollWheelEvent, Window,
    point, px,
};
use pdf_engine::{EditCommand, TextStamp};

use crate::actions::{
    ActualSize, AddTextTool, CommitText, FitPage, HandTool, HighlightTool, RedactTool, SelectTool,
    ZoomIn, ZoomOut,
};
use crate::editor_view::input;

use super::EditorView;
use super::geometry::page_point;
use super::gestures::{AnchorContext, DocumentMetrics, anchored_document_offset, pinch_zoom};
use super::model::{DragState, InlineText, Tool};

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

    pub(super) fn redact_tool(&mut self, _: &RedactTool, _: &mut Window, cx: &mut Context<Self>) {
        self.activate_tool(Tool::Redact, cx);
    }

    fn activate_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        self.tool = tool;
        self.drag = None;
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
            return;
        }
        let Some(point) = self.pdf_point(page_index, event.position) else {
            return;
        };
        if self.tool == Tool::AddText {
            let text = input("Type on page", "", window, cx);
            text.update(cx, |state, cx| state.focus(window, cx));
            self.inline_text = Some(InlineText {
                page_index,
                point,
                input: text,
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
            None => return,
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
            return;
        }
        let Some(mut drag) = self.drag.take() else {
            return;
        };
        let page_index = match &drag {
            DragState::Region { page_index, .. } => *page_index,
            DragState::Pan { .. } => return,
        };
        if let Some(point) = self.pdf_point(page_index, event.position)
            && let DragState::Region { current, .. } = &mut drag
        {
            *current = point;
        }
        let Some(rect) = drag.rect() else {
            cx.notify();
            return;
        };
        self.finish_region(page_index, rect);
        cx.notify();
    }

    fn finish_region(&mut self, page_index: usize, rect: document_core::PdfRect) {
        match self.tool {
            Tool::Select => self.select_fragments(page_index, rect),
            Tool::Highlight => self.highlight_fragments(page_index, rect),
            Tool::Redact => {
                self.edits.push(EditCommand::Redact { page_index, rect });
                self.status = "Redaction queued; Save As to apply".into();
            }
            Tool::Hand | Tool::AddText => {}
        }
    }

    fn select_fragments(&mut self, page_index: usize, rect: document_core::PdfRect) {
        let Some(page) = self.pages.get(page_index) else {
            return;
        };
        let selected: Vec<_> = page
            .fragments
            .iter()
            .filter(|fragment| fragment.rect.intersection(rect).is_some())
            .collect();
        self.selected_rects = selected.iter().map(|fragment| fragment.rect).collect();
        self.selected_text = selected
            .iter()
            .map(|fragment| fragment.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
            .into();
        self.status = format!("Selected {} text runs", selected.len()).into();
    }

    fn highlight_fragments(&mut self, page_index: usize, rect: document_core::PdfRect) {
        let Some(page) = self.pages.get(page_index) else {
            return;
        };
        let rects: Vec<_> = page
            .fragments
            .iter()
            .filter(|fragment| fragment.rect.intersection(rect).is_some())
            .map(|fragment| fragment.rect)
            .collect();
        if rects.is_empty() {
            self.status = "No text under highlight selection".into();
            return;
        }
        self.selected_rects.clone_from(&rects);
        self.edits.push(EditCommand::Highlight {
            page_index,
            rects,
            color: self.highlight_color,
        });
        self.status = "Highlight queued; Save As to write annotation".into();
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
}
