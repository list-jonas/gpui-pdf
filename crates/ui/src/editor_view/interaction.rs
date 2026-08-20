use gpui::{
    App, ClipboardItem, Context, Focusable, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    PinchEvent, ScrollWheelEvent, Window, point, px,
};
use pdf_engine::{EditCommand, TextStamp};

use crate::actions::{
    ActualSize, AddTextTool, Cancel, CopySelection, EditTool, FitPage, FitWidth, HandTool,
    HighlightTool, NoteTool, RedactTool, Redo, SelectAllText, SelectTool, ShapeTool, SignatureTool,
    StrikeoutTool, UnderlineTool, Undo, ZoomIn, ZoomOut,
};
use crate::editor_view::inline_text_input;

use super::EditorView;
use super::Severity;
use super::geometry::page_point;
use super::gestures::{AnchorContext, DocumentMetrics, anchored_document_offset, pinch_zoom};
use super::model::{DragState, InlineNote, InlineText, Tool};
use super::schedule::{self, PageState, Viewport};

pub(super) const MIN_ZOOM: f32 = 0.25;
pub(super) const MAX_ZOOM: f32 = 8.0;
const DOCUMENT_INSET: f32 = 64.0;
const ZOOM_STOPS: [f32; 12] = [
    0.25, 0.33, 0.5, 0.67, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0, 8.0,
];
/// Minimum drag size (PDF points) before a region edit is created.
const MIN_REGION: f64 = 4.0;
/// How far (PDF points) a pointer may sit from a text run and still select it.
const SELECTION_TOLERANCE: f64 = 24.0;
/// Soft budget for decoded page rasters held in memory (about 512 MiB).
const MAX_IMAGE_BYTES: u64 = 512 * 1024 * 1024;
/// Pages within this distance of the viewport are never evicted.
const KEEP_RESIDENT_MARGIN: u32 = 8;

/// Steps to the next preset zoom stop so repeated presses feel predictable
/// instead of drifting off the presets after a fit or pinch.
fn next_zoom_step(zoom: f32, zoom_in: bool) -> f32 {
    if zoom_in {
        ZOOM_STOPS
            .iter()
            .copied()
            .find(|stop| *stop > zoom + 0.001)
            .unwrap_or(MAX_ZOOM)
    } else {
        ZOOM_STOPS
            .iter()
            .rev()
            .copied()
            .find(|stop| *stop < zoom - 0.001)
            .unwrap_or(MIN_ZOOM)
    }
}

impl EditorView {
    pub(super) fn select_tool(
        &mut self,
        _: &SelectTool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activate_tool(Tool::Select, window, cx);
    }

    pub(super) fn hand_tool(&mut self, _: &HandTool, window: &mut Window, cx: &mut Context<Self>) {
        self.activate_tool(Tool::Hand, window, cx);
    }

    pub(super) fn edit_tool(&mut self, _: &EditTool, window: &mut Window, cx: &mut Context<Self>) {
        self.activate_tool(Tool::Edit, window, cx);
    }

    pub(super) fn highlight_tool(
        &mut self,
        _: &HighlightTool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activate_tool(Tool::Highlight, window, cx);
    }

    pub(super) fn add_text_tool(
        &mut self,
        _: &AddTextTool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activate_tool(Tool::AddText, window, cx);
    }

    pub(super) fn underline_tool(
        &mut self,
        _: &UnderlineTool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activate_tool(Tool::Underline, window, cx);
    }

    pub(super) fn strikeout_tool(
        &mut self,
        _: &StrikeoutTool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activate_tool(Tool::Strikeout, window, cx);
    }

    pub(super) fn note_tool(&mut self, _: &NoteTool, window: &mut Window, cx: &mut Context<Self>) {
        self.activate_tool(Tool::Note, window, cx);
    }

    pub(super) fn signature_tool(
        &mut self,
        _: &SignatureTool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activate_tool(Tool::Signature, window, cx);
    }

    pub(super) fn shape_tool(
        &mut self,
        _: &ShapeTool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activate_tool(Tool::Shape, window, cx);
    }

    pub(super) fn redact_tool(
        &mut self,
        _: &RedactTool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activate_tool(Tool::Redact, window, cx);
    }

    fn activate_tool(&mut self, tool: Tool, window: &mut Window, cx: &mut Context<Self>) {
        if self.input_has_focus(window, cx) {
            return;
        }
        if self.tool == tool {
            return;
        }
        self.tool = tool;
        self.drag = None;
        self.materialize_inline_edits(cx);
        if !matches!(tool, Tool::Select | Tool::Hand) {
            self.selected_rects.clear();
            self.selected_text = "No text selected".into();
        }
        self.flash(format!("{} tool", tool.label()), Severity::Info, cx);
    }

    pub(super) fn toggle_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.panels.search {
            self.close_search(window, cx);
        } else {
            self.open_search(&crate::actions::Search, window, cx);
        }
    }

    pub(super) fn input_has_focus(&self, window: &Window, cx: &App) -> bool {
        self.search_input.focus_handle(cx).is_focused(window)
            || self.page_input.focus_handle(cx).is_focused(window)
            // A newly created overlay receives focus before its first render. Keep tool
            // shortcuts out of the way during that short hand-off too, otherwise the
            // first typed `t`/`n` can switch tools and discard an empty overlay.
            || self.inline_text.is_some()
            || self.inline_note.is_some()
            || self
                .forms
                .iter()
                .any(|field| field.input.focus_handle(cx).is_focused(window))
    }

    pub(super) fn zoom_in(&mut self, _: &ZoomIn, _: &mut Window, cx: &mut Context<Self>) {
        self.set_zoom(next_zoom_step(self.zoom, true), cx);
    }

    pub(super) fn zoom_out(&mut self, _: &ZoomOut, _: &mut Window, cx: &mut Context<Self>) {
        self.set_zoom(next_zoom_step(self.zoom, false), cx);
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
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        let x = (f32::from(viewport.width) - DOCUMENT_INSET) / width;
        let y = (f32::from(viewport.height) - DOCUMENT_INSET) / height;
        self.set_zoom(x.min(y), cx);
    }

    pub(super) fn fit_width(&mut self, _: &FitWidth, _: &mut Window, cx: &mut Context<Self>) {
        let viewport = self.scroll.bounds().size;
        let Some(page) = self.pages.get(self.page_index) else {
            return;
        };
        let width = page.image_size.0;
        if width <= 0.0 {
            return;
        }
        self.set_zoom((f32::from(viewport.width) - DOCUMENT_INSET) / width, cx);
    }

    pub(super) fn undo(&mut self, _: &Undo, window: &mut Window, cx: &mut Context<Self>) {
        if self.input_has_focus(window, cx) {
            return;
        }
        if self.history.undo().is_some() {
            self.mark_edited(window, cx);
            self.flash("Undo", Severity::Info, cx);
        } else {
            self.flash("Nothing to undo", Severity::Info, cx);
        }
    }

    pub(super) fn redo(&mut self, _: &Redo, window: &mut Window, cx: &mut Context<Self>) {
        if self.input_has_focus(window, cx) {
            return;
        }
        if self.history.redo().is_some() {
            self.mark_edited(window, cx);
            self.flash("Redo", Severity::Info, cx);
        } else {
            self.flash("Nothing to redo", Severity::Info, cx);
        }
    }

    /// Escape backs out of whatever is currently in progress, innermost first.
    pub(super) fn cancel(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<Self>) {
        if self.drag.take().is_some() {
            self.flash("Cancelled", Severity::Info, cx);
            return;
        }
        if self.inline_text.is_some() || self.inline_note.is_some() {
            self.inline_text = None;
            self.inline_note = None;
            self.focus_handle.focus(window);
            self.flash("Discarded draft", Severity::Info, cx);
            return;
        }
        if self.panels.search {
            self.close_search(window, cx);
            return;
        }
        if !self.selected_rects.is_empty() {
            self.selected_rects.clear();
            self.selected_text = "No text selected".into();
            self.flash("Selection cleared", Severity::Info, cx);
            return;
        }
        if self.tool != Tool::Select {
            self.activate_tool(Tool::Select, window, cx);
        }
    }

    pub(super) fn select_all_text(
        &mut self,
        _: &SelectAllText,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.input_has_focus(window, cx) {
            return;
        }
        let Some(page) = self.pages.get(self.page_index) else {
            return;
        };
        if page.fragments.is_empty() {
            self.flash("No selectable text on this page", Severity::Info, cx);
            return;
        }
        self.selected_rects = page
            .fragments
            .iter()
            .map(|fragment| fragment.rect)
            .collect();
        self.selected_text = page
            .fragments
            .iter()
            .map(|fragment| fragment.text.as_str())
            .collect::<String>()
            .into();
        self.flash(
            format!("Selected {} characters", self.selected_text.chars().count()),
            Severity::Info,
            cx,
        );
    }

    /// Keeps panning inside the scrollable content instead of letting the
    /// document drift into empty space.
    fn clamp_offset(&self, x: f32, y: f32) -> gpui::Point<gpui::Pixels> {
        let max = self.scroll.max_offset();
        point(
            px(x.clamp(-f32::from(max.width).max(0.0), 0.0)),
            px(y.clamp(-f32::from(max.height).max(0.0), 0.0)),
        )
    }

    /// Marks the window dirty so macOS shows the unsaved-changes indicator.
    pub(super) fn mark_edited(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.set_window_edited(!self.history.is_empty());
        self.refresh_active_page();
        cx.notify();
    }

    fn set_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        let clamped = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        if (clamped - self.zoom).abs() < f32::EPSILON {
            return;
        }
        self.zoom = clamped;
        self.request_visible_pages();
        self.flash(
            format!("Zoom {}%", (self.zoom * 100.0).round()),
            Severity::Info,
            cx,
        );
    }

    /// Queues the work the current viewport needs: full-quality rasters for
    /// what is on screen, cheap previews just outside it, and page text for
    /// search and selection.
    pub(super) fn request_visible_pages(&mut self) {
        let viewport = self.viewport();
        let states = self.page_states();
        let jobs = schedule::plan(viewport, &states);
        self.background_requested = false;
        self.dispatch(jobs, true);
    }

    /// Fills in pages outside the viewport once the visible ones are done.
    pub(super) fn request_remaining_pages(&mut self) {
        if self.background_requested {
            return;
        }
        let viewport = self.viewport();
        let states = self.page_states();
        let jobs = schedule::plan_background(viewport, &states);
        if jobs.is_empty() {
            return;
        }
        self.background_requested = true;
        self.dispatch(jobs, false);
    }

    /// Frees rasters far from the viewport so a long document does not grow
    /// without bound while the background pass fills it in.
    pub(super) fn evict_distant_pages(&mut self) {
        let total: u64 = self.pages.iter().map(|page| page.image_bytes).sum();
        if total <= MAX_IMAGE_BYTES {
            return;
        }
        let viewport = self.viewport();
        let mut candidates: Vec<_> = self
            .pages
            .iter()
            .enumerate()
            .filter(|(index, page)| {
                page.image.is_some() && distance(viewport, *index) > KEEP_RESIDENT_MARGIN
            })
            .map(|(index, page)| (distance(viewport, index), index, page.image_bytes))
            .collect();
        candidates.sort_unstable_by_key(|(distance, _, _)| std::cmp::Reverse(*distance));

        let mut freed = 0_u64;
        for (_, index, bytes) in candidates {
            if total.saturating_sub(freed) <= MAX_IMAGE_BYTES {
                break;
            }
            if let Some(page) = self.pages.get_mut(index) {
                page.release_image();
                self.loaded_pages = self.loaded_pages.saturating_sub(1);
                freed += bytes;
            }
        }
        // Evicted pages may need to come back, so allow another background pass.
        self.background_requested = false;
    }

    fn dispatch(&mut self, jobs: Vec<crate::PageRequest>, replace: bool) {
        if jobs.is_empty() {
            return;
        }
        for job in &jobs {
            let Some(page) = self.pages.get_mut(job.page_index) else {
                continue;
            };
            match job.kind {
                crate::PageKind::Text => page.text_requested = true,
                crate::PageKind::Preview | crate::PageKind::Sharp => {
                    page.requested_scale = page.requested_scale.max(job.scale);
                }
            }
        }
        let _ = self
            .requests
            .try_send(crate::EditorRequest::Render { replace, jobs });
    }

    fn viewport(&self) -> Viewport {
        let last = self.pages.len().saturating_sub(1);
        let (first_visible, last_visible) = if self.scroll.children_count() == 0 {
            (self.page_index, self.page_index)
        } else {
            (
                self.scroll.top_item().min(last),
                self.scroll.bottom_item().min(last),
            )
        };
        Viewport {
            first_visible: first_visible.min(self.page_index),
            last_visible: last_visible.max(self.page_index).min(last),
            target_scale: super::geometry::ui_f32(super::geometry::RENDER_SCALE) * self.zoom,
        }
    }

    fn page_states(&self) -> Vec<PageState> {
        self.pages
            .iter()
            .map(|page| PageState {
                render_scale: page.render_scale,
                requested_scale: page.requested_scale,
                text_loaded: page.text_loaded,
                text_requested: page.text_requested,
            })
            .collect()
    }

    pub(super) fn document_scroll_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Cmd + wheel is the conventional zoom gesture on a mouse.
        if event.modifiers.platform {
            let delta = event.delta.pixel_delta(px(1.0)).y;
            let factor = 1.0 + f32::from(delta) * 0.01;
            self.set_zoom(self.zoom * factor.clamp(0.5, 1.5), cx);
            return;
        }
        self.sync_current_page_from_scroll();
        self.refresh_active_page();
        self.request_visible_pages();
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
        if self.tool == Tool::Hand {
            self.set_current_page(page_index);
            self.drag = Some(DragState::Pan {
                start: event.position,
                offset: self.scroll.offset(),
            });
            cx.notify();
            return;
        }
        self.set_current_page(page_index);
        let Some(point) = self.pdf_point(page_index, event.position) else {
            return;
        };
        if matches!(self.tool, Tool::AddText | Tool::Signature) {
            self.materialize_inline_text(cx);
            let placeholder = if self.tool == Tool::Signature {
                "Type signature"
            } else {
                "Type on page"
            };
            let text = inline_text_input(placeholder, "", window, cx);
            self.inline_text = Some(InlineText {
                page_index,
                point,
                input: text.clone(),
            });
            window.defer(cx, move |window, cx| {
                text.update(cx, |state, cx| state.focus(window, cx));
            });
        } else if self.tool == Tool::Note {
            self.materialize_inline_note(cx);
            let note = inline_text_input("Write comment", "", window, cx);
            self.inline_note = Some(InlineNote {
                page_index,
                point,
                input: note.clone(),
            });
            window.defer(cx, move |window, cx| {
                note.update(cx, |state, cx| state.focus(window, cx));
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

    pub(super) fn edit_text_overlay(
        &mut self,
        edit_index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(EditCommand::AddText(stamp)) = self.history.edits().get(edit_index).cloned()
        else {
            return;
        };
        self.history.remove(edit_index);
        let input = inline_text_input("Type on page", &stamp.text, window, cx);
        self.inline_text = Some(InlineText {
            page_index: stamp.page_index,
            point: document_core::PdfPoint::new(stamp.x, stamp.y),
            input: input.clone(),
        });
        window.defer(cx, move |window, cx| {
            input.update(cx, |state, cx| state.focus(window, cx));
        });
        self.tool = Tool::AddText;
        cx.stop_propagation();
        cx.notify();
    }

    pub(super) fn edit_note_overlay(
        &mut self,
        edit_index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(EditCommand::Note {
            page_index,
            x,
            y,
            contents,
            ..
        }) = self.history.edits().get(edit_index).cloned()
        else {
            return;
        };
        self.history.remove(edit_index);
        let input = inline_text_input("Write comment", &contents, window, cx);
        self.inline_note = Some(InlineNote {
            page_index,
            point: document_core::PdfPoint::new(x, y),
            input: input.clone(),
        });
        window.defer(cx, move |window, cx| {
            input.update(cx, |state, cx| state.focus(window, cx));
        });
        self.tool = Tool::Note;
        cx.stop_propagation();
        cx.notify();
    }

    pub(super) fn materialize_inline_edits(&mut self, cx: &App) {
        self.materialize_inline_text(cx);
        self.materialize_inline_note(cx);
    }

    fn materialize_inline_text(&mut self, cx: &App) {
        let Some(inline) = self.inline_text.take() else {
            return;
        };
        let text = inline.input.read(cx).value().to_string();
        if text.trim().is_empty() {
            return;
        }
        self.history.push(EditCommand::AddText(TextStamp {
            page_index: inline.page_index,
            text,
            x: inline.point.x,
            y: inline.point.y,
            size: 14.0,
        }));
    }

    fn materialize_inline_note(&mut self, cx: &App) {
        let Some(note) = self.inline_note.take() else {
            return;
        };
        let contents = note.input.read(cx).value().trim().to_owned();
        if contents.is_empty() {
            return;
        }
        self.history.push(EditCommand::Note {
            page_index: note.page_index,
            x: note.point.x,
            y: note.point.y,
            contents,
            color: self.annotation_color,
        });
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

    pub(super) fn inline_note_mouse_down(
        &mut self,
        page_index: usize,
        event: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(point) = self.pdf_point(page_index, event.position) else {
            return;
        };
        let Some(note) = self
            .inline_note
            .as_ref()
            .filter(|note| note.page_index == page_index)
        else {
            return;
        };
        self.drag = Some(DragState::InlineNote {
            page_index,
            start: point,
            point: note.point,
        });
        cx.stop_propagation();
        cx.notify();
    }

    pub(super) fn inline_note_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.dragging() {
            self.move_inline_note(event.position, cx);
        }
    }

    pub(super) fn inline_note_mouse_up(
        &mut self,
        _: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.drag, Some(DragState::InlineNote { .. })) {
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
            Some(
                DragState::Region { page_index, .. }
                | DragState::InlineText { page_index, .. }
                | DragState::InlineNote { page_index, .. },
            ) => self.pdf_point(*page_index, event.position),
            _ => None,
        };
        match &mut self.drag {
            Some(DragState::Pan { start, offset }) => {
                let x = f32::from(offset.x) + f32::from(event.position.x - start.x);
                let y = f32::from(offset.y) + f32::from(event.position.y - start.y);
                self.scroll.set_offset(self.clamp_offset(x, y));
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
            Some(DragState::InlineNote { .. }) => {
                self.move_inline_note(event.position, cx);
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.drag, Some(DragState::Pan { .. })) {
            self.drag = None;
            cx.notify();
            return;
        }
        if matches!(
            self.drag,
            Some(DragState::InlineText { .. } | DragState::InlineNote { .. })
        ) {
            self.drag = None;
            cx.notify();
            return;
        }
        let Some(mut drag) = self.drag.take() else {
            return;
        };
        let page_index = match &drag {
            DragState::Region { page_index, .. } => *page_index,
            DragState::Pan { .. } | DragState::InlineText { .. } | DragState::InlineNote { .. } => {
                return;
            }
        };
        if let Some(point) = self.pdf_point(page_index, event.position)
            && let DragState::Region { current, .. } = &mut drag
        {
            *current = point;
        }
        let DragState::Region { start, current, .. } = drag else {
            return;
        };
        self.finish_region(page_index, start, current, window, cx);
        cx.notify();
    }

    /// A mouse-up anywhere in the window must end the drag; otherwise releasing
    /// outside the page leaves the tool stuck in a dragging state.
    pub(super) fn window_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.drag.is_some() {
            self.page_mouse_up(event, window, cx);
        }
    }

    fn finish_region(
        &mut self,
        page_index: usize,
        start: document_core::PdfPoint,
        current: document_core::PdfPoint,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.tool {
            Tool::Select | Tool::Edit => {
                self.select_text_range(page_index, start, current);
            }
            Tool::Highlight => {
                self.select_text_range(page_index, start, current);
                self.highlight_selection(page_index, window, cx);
            }
            Tool::Underline => {
                self.select_text_range(page_index, start, current);
                self.markup_selection(page_index, Markup::Underline, window, cx);
            }
            Tool::Strikeout => {
                self.select_text_range(page_index, start, current);
                self.markup_selection(page_index, Markup::Strikeout, window, cx);
            }
            Tool::Shape => {
                if let Some(rect) = (DragState::Region {
                    page_index,
                    start,
                    current,
                })
                .rect()
                {
                    if rect.width() < MIN_REGION || rect.height() < MIN_REGION {
                        self.flash("Drag a larger area to draw a shape", Severity::Info, cx);
                        return;
                    }
                    self.history.push(EditCommand::Shape {
                        page_index,
                        kind: self.shape_kind,
                        rect,
                        color: self.annotation_color,
                        width: 2.0,
                    });
                    self.mark_edited(window, cx);
                    self.flash("Shape added", Severity::Info, cx);
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
                    if rect.width() < MIN_REGION || rect.height() < MIN_REGION {
                        self.flash("Drag a larger area to redact", Severity::Info, cx);
                        return;
                    }
                    self.history.push(EditCommand::Redact { page_index, rect });
                    self.mark_edited(window, cx);
                    self.flash("Redaction added; applied on save", Severity::Info, cx);
                }
            }
            Tool::Hand | Tool::AddText | Tool::Note | Tool::Signature => {}
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
        if self.selected_rects.is_empty() {
            self.selected_text = "No text selected".into();
        }
    }

    fn highlight_selection(
        &mut self,
        page_index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.markup_selection(page_index, Markup::Highlight, window, cx);
    }

    fn markup_selection(
        &mut self,
        page_index: usize,
        markup: Markup,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_rects.is_empty() {
            self.flash(
                format!(
                    "No text under the {} selection",
                    markup.label().to_lowercase()
                ),
                Severity::Info,
                cx,
            );
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
        self.history.push(edit);
        self.selected_rects.clear();
        self.selected_text = "No text selected".into();
        self.mark_edited(window, cx);
        self.flash(format!("{} added", markup.label()), Severity::Info, cx);
    }

    pub(super) fn copy_selection(
        &mut self,
        _: &CopySelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.input_has_focus(window, cx) {
            return;
        }
        if self.selected_text.is_empty() || self.selected_text == "No text selected" {
            self.flash("No text selected to copy", Severity::Info, cx);
        } else {
            cx.write_to_clipboard(ClipboardItem::new_string(self.selected_text.to_string()));
            self.flash(
                format!("Copied {} characters", self.selected_text.chars().count()),
                Severity::Info,
                cx,
            );
        }
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

    fn move_inline_note(&mut self, position: gpui::Point<gpui::Pixels>, cx: &mut Context<Self>) {
        let Some((page_index, start, original)) = self.drag.as_ref().and_then(|drag| match drag {
            DragState::InlineNote {
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
        if let Some(note) = self.inline_note.as_mut() {
            note.point = document_core::PdfPoint::new(
                original.x + current.x - start.x,
                original.y + current.y - start.y,
            );
            self.status = "Move comment".into();
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

/// Finds the fragment under (or nearest to) the point, but ignores fragments
/// that are far away so a stray click on blank space selects nothing.
fn nearest_fragment(
    fragments: &[pdf_engine::TextFragment],
    point: document_core::PdfPoint,
) -> Option<usize> {
    let (index, distance) = fragments
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            point_distance(left.rect, point).total_cmp(&point_distance(right.rect, point))
        })
        .map(|(index, fragment)| (index, point_distance(fragment.rect, point)))?;
    (distance <= SELECTION_TOLERANCE * SELECTION_TOLERANCE).then_some(index)
}

fn point_distance(rect: document_core::PdfRect, point: document_core::PdfPoint) -> f64 {
    let x = point.x.clamp(rect.x_min, rect.x_max);
    let y = point.y.clamp(rect.y_min, rect.y_max);
    (point.x - x).powi(2) + (point.y - y).powi(2)
}

/// Pages away from the visible range, used for eviction order.
fn distance(viewport: Viewport, page_index: usize) -> u32 {
    let before = viewport.first_visible.saturating_sub(page_index);
    let after = page_index.saturating_sub(viewport.last_visible);
    u32::try_from(before.max(after)).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use document_core::{PdfPoint, PdfRect};
    use pdf_engine::TextFragment;

    use super::{MAX_ZOOM, MIN_ZOOM, next_zoom_step, text_selection};

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

    #[test]
    fn zoom_steps_move_between_presets_and_stop_at_limits() {
        assert!((next_zoom_step(1.0, true) - 1.25).abs() < f32::EPSILON);
        assert!((next_zoom_step(1.0, false) - 0.75).abs() < f32::EPSILON);
        assert!((next_zoom_step(0.4, true) - 0.5).abs() < f32::EPSILON);
        assert!((next_zoom_step(MAX_ZOOM, true) - MAX_ZOOM).abs() < f32::EPSILON);
        assert!((next_zoom_step(MIN_ZOOM, false) - MIN_ZOOM).abs() < f32::EPSILON);
    }

    #[test]
    fn clicking_far_from_text_selects_nothing() {
        let fragments = vec![TextFragment {
            text: "Hello".into(),
            rect: PdfRect::new(0.0, 0.0, 50.0, 10.0).unwrap(),
        }];

        let far = text_selection(
            &fragments,
            PdfPoint::new(0.0, 600.0),
            PdfPoint::new(10.0, 600.0),
        );
        assert!(far.is_empty());

        let near = text_selection(
            &fragments,
            PdfPoint::new(5.0, 5.0),
            PdfPoint::new(40.0, 5.0),
        );
        assert_eq!(near.len(), 1);
    }
}
