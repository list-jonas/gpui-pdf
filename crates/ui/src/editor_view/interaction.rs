use gpui::{
    App, ClipboardItem, Context, Focusable, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    PinchEvent, ScrollWheelEvent, Window, point, px,
};
use pdf_engine::{EditCommand, TextStamp};

use crate::actions::{
    ActualSize, AddTextTool, Cancel, CopySelection, DeleteSelection, EditTool, FitPage, FitWidth,
    HandTool, HighlightTool, NoteTool, RedactTool, Redo, ScrollDown, ScrollPageDown, ScrollPageUp,
    ScrollToBottom, ScrollToTop, ScrollUp, SelectAllText, SelectTool, ShapeTool, SignatureTool,
    StrikeoutTool, TogglePropertiesPanel, ToggleSidebar, UnderlineTool, Undo, ZoomIn, ZoomOut,
};
use crate::editor_view::inline_text_input;

use super::EditorView;
use super::Severity;
use super::document_page::DocumentPage;
use super::geometry::page_point;
use super::gestures::{AnchorContext, DocumentMetrics, anchored_document_offset, pinch_zoom};
use super::model::{DragState, InlineNote, InlineText, SelectedRun, Tool};
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
/// Pixels moved by a single arrow-key scroll.
const SCROLL_STEP: f32 = 80.0;
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
            self.clear_selection();
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
        self.cancel_innermost(window, cx);
    }

    /// Escape reaches the editor even while a text field has focus, because
    /// inputs bind Escape themselves and would otherwise swallow it.
    pub(super) fn capture_escape(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key == "escape" && !event.keystroke.modifiers.modified() {
            self.cancel_innermost(window, cx);
            cx.stop_propagation();
        }
    }

    fn cancel_innermost(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Text fields swallow Escape, so a field with focus is handled first:
        // Escape returns focus to the page rather than doing nothing.
        if self.search_input.focus_handle(cx).is_focused(window) {
            self.close_search(window, cx);
            return;
        }
        if self.page_input.focus_handle(cx).is_focused(window) {
            self.focus_handle.focus(window);
            self.sync_page_input(window, cx);
            cx.notify();
            return;
        }
        if self
            .forms
            .iter()
            .any(|field| field.input.focus_handle(cx).is_focused(window))
        {
            self.focus_handle.focus(window);
            cx.notify();
            return;
        }
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
        if self.has_selection() || self.selected_edit.is_some() {
            self.clear_selection();
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
        // Select every page whose text has been extracted so far. Pages still
        // loading are reported rather than silently omitted.
        let runs: Vec<SelectedRun> = self
            .pages
            .iter()
            .enumerate()
            .flat_map(|(page_index, page)| {
                page.fragments.iter().map(move |fragment| SelectedRun {
                    page_index,
                    rect: fragment.rect,
                    text: fragment.text.clone(),
                })
            })
            .collect();
        if runs.is_empty() {
            self.flash("No selectable text yet", Severity::Info, cx);
            return;
        }
        let pending = self.pages.iter().filter(|page| !page.text_loaded).count();
        self.set_selection(runs);
        let note = if pending > 0 {
            format!("; {pending} page(s) still loading")
        } else {
            String::new()
        };
        self.flash(
            format!(
                "Selected {} characters{note}",
                self.selected_text.chars().count()
            ),
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
                current_page: page_index,
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
        // A text drag may leave the page it started on, so the pointer is
        // resolved against whichever page it is currently over.
        let hovered = self.page_at(event.position);
        let pointer = match &self.drag {
            Some(DragState::Region { page_index, .. }) => hovered
                .and_then(|page| {
                    self.pdf_point(page, event.position)
                        .map(|point| (page, point))
                })
                .or_else(|| {
                    self.pdf_point(*page_index, event.position)
                        .map(|point| (*page_index, point))
                }),
            Some(
                DragState::InlineText { page_index, .. } | DragState::InlineNote { page_index, .. },
            ) => self
                .pdf_point(*page_index, event.position)
                .map(|point| (*page_index, point)),
            _ => None,
        };
        match &mut self.drag {
            Some(DragState::Pan { start, offset }) => {
                let x = f32::from(offset.x) + f32::from(event.position.x - start.x);
                let y = f32::from(offset.y) + f32::from(event.position.y - start.y);
                self.scroll.set_offset(self.clamp_offset(x, y));
            }
            Some(DragState::Region {
                current,
                current_page,
                ..
            }) => {
                let Some((page, point)) = pointer else {
                    return;
                };
                *current = point;
                *current_page = page;
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
            current_page,
        }) = self.drag.as_ref()
            && matches!(
                self.tool,
                Tool::Select | Tool::Highlight | Tool::Underline | Tool::Strikeout
            )
        {
            self.select_text_range(*page_index, *start, *current_page, *current);
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
        let hovered = self.page_at(event.position).unwrap_or(page_index);
        if let Some(point) = self.pdf_point(hovered, event.position)
            && let DragState::Region {
                current,
                current_page,
                ..
            } = &mut drag
        {
            *current = point;
            *current_page = hovered;
        }
        let DragState::Region {
            start,
            current,
            current_page,
            ..
        } = drag
        else {
            return;
        };
        self.finish_region(page_index, start, current_page, current, window, cx);
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
        current_page: usize,
        current: document_core::PdfPoint,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.tool {
            Tool::Select | Tool::Edit => {
                self.select_text_range(page_index, start, current_page, current);
            }
            Tool::Highlight => {
                self.select_text_range(page_index, start, current_page, current);
                self.markup_selection(Markup::Highlight, window, cx);
            }
            Tool::Underline => {
                self.select_text_range(page_index, start, current_page, current);
                self.markup_selection(Markup::Underline, window, cx);
            }
            Tool::Strikeout => {
                self.select_text_range(page_index, start, current_page, current);
                self.markup_selection(Markup::Strikeout, window, cx);
            }
            Tool::Shape => {
                if let Some(rect) = (DragState::Region {
                    page_index,
                    start,
                    current,
                    current_page,
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
                    current_page,
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

    /// Selects every text run between the drag anchor and the pointer, across
    /// page boundaries when the drag leaves the starting page.
    fn select_text_range(
        &mut self,
        anchor_page: usize,
        anchor: document_core::PdfPoint,
        head_page: usize,
        head: document_core::PdfPoint,
    ) {
        let Some(anchor_edge) = self.selection_edge(anchor_page, anchor, head_page > anchor_page)
        else {
            return;
        };
        let Some(head_edge) = self.selection_edge(head_page, head, head_page < anchor_page) else {
            return;
        };
        self.set_selection(text_selection(&self.pages, anchor_edge, head_edge));
    }

    /// Resolves a pointer position to a text run. `edge_of_page` snaps to the
    /// first or last run when the pointer sits past the text on that page,
    /// which is what happens while dragging through a page break.
    fn selection_edge(
        &self,
        page_index: usize,
        point: document_core::PdfPoint,
        toward_end: bool,
    ) -> Option<SelectionEdge> {
        let page = self.pages.get(page_index)?;
        if page.fragments.is_empty() {
            return None;
        }
        let fragment = nearest_fragment(&page.fragments, point).unwrap_or(if toward_end {
            page.fragments.len() - 1
        } else {
            0
        });
        Some(SelectionEdge {
            page_index,
            fragment,
        })
    }

    pub(super) fn set_selection(&mut self, runs: Vec<SelectedRun>) {
        self.selected_text = if runs.is_empty() {
            "No text selected".into()
        } else {
            join_selection(&runs).into()
        };
        self.selection = runs;
    }

    pub(super) fn clear_selection(&mut self) {
        self.selection.clear();
        self.selected_text = "No text selected".into();
        self.selected_edit = None;
    }

    /// The page whose painted bounds contain the pointer, if any.
    fn page_at(&self, position: gpui::Point<gpui::Pixels>) -> Option<usize> {
        self.pages
            .iter()
            .position(|page| page.bounds.get().contains(&position))
    }

    /// Deletes whatever is selected: a clicked annotation, otherwise the last
    /// edit that touched the text selection.
    pub(super) fn delete_selection(
        &mut self,
        _: &DeleteSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.input_has_focus(window, cx) {
            return;
        }
        if let Some(index) = self.selected_edit.take()
            && self.history.remove(index).is_some()
        {
            self.mark_edited(window, cx);
            self.flash("Annotation deleted", Severity::Info, cx);
            return;
        }
        if self.has_selection() {
            self.clear_selection();
            self.flash("Selection cleared", Severity::Info, cx);
            return;
        }
        self.flash("Select an annotation to delete", Severity::Info, cx);
    }

    /// Marks a placed annotation as the target of keyboard actions.
    pub(super) fn select_edit(&mut self, edit_index: usize, cx: &mut Context<Self>) {
        self.selection.clear();
        self.selected_text = "No text selected".into();
        self.selected_edit = Some(edit_index);
        cx.notify();
    }

    pub(super) fn scroll_up(&mut self, _: &ScrollUp, _: &mut Window, cx: &mut Context<Self>) {
        self.scroll_by(-SCROLL_STEP, cx);
    }

    pub(super) fn scroll_down(&mut self, _: &ScrollDown, _: &mut Window, cx: &mut Context<Self>) {
        self.scroll_by(SCROLL_STEP, cx);
    }

    pub(super) fn scroll_page_up(
        &mut self,
        _: &ScrollPageUp,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.scroll_by(-self.viewport_step(), cx);
    }

    pub(super) fn scroll_page_down(
        &mut self,
        _: &ScrollPageDown,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.scroll_by(self.viewport_step(), cx);
    }

    pub(super) fn scroll_to_top(
        &mut self,
        _: &ScrollToTop,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.scroll.set_offset(point(px(0.0), px(0.0)));
        self.sync_current_page_from_scroll();
        cx.notify();
    }

    pub(super) fn scroll_to_bottom(
        &mut self,
        _: &ScrollToBottom,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let max = self.scroll.max_offset();
        self.scroll
            .set_offset(point(px(0.0), px(-f32::from(max.height).max(0.0))));
        self.sync_current_page_from_scroll();
        cx.notify();
    }

    fn viewport_step(&self) -> f32 {
        // Overlap slightly so no line is skipped between screenfuls.
        (f32::from(self.scroll.bounds().size.height) * 0.9).max(SCROLL_STEP)
    }

    fn scroll_by(&mut self, delta: f32, cx: &mut Context<Self>) {
        let offset = self.scroll.offset();
        self.scroll
            .set_offset(self.clamp_offset(f32::from(offset.x), f32::from(offset.y) - delta));
        self.sync_current_page_from_scroll();
        self.request_visible_pages();
        cx.notify();
    }

    pub(super) fn toggle_sidebar(
        &mut self,
        _: &ToggleSidebar,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.panels.sidebar = !self.panels.sidebar;
        cx.notify();
    }

    pub(super) fn toggle_properties_panel(
        &mut self,
        _: &TogglePropertiesPanel,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.panels.properties = !self.panels.properties;
        cx.notify();
    }

    pub(super) fn has_selection(&self) -> bool {
        !self.selection.is_empty()
    }

    /// Describes the size and page span of the current selection.
    pub(super) fn selection_summary(&self) -> Option<String> {
        let first = self.selection.first()?.page_index;
        let last = self.selection.last()?.page_index;
        let characters = self.selected_text.chars().count();
        Some(if first == last {
            format!("{characters} characters · page {}", first + 1)
        } else {
            format!("{characters} characters · pages {}–{}", first + 1, last + 1)
        })
    }

    /// Applies a markup annotation to the selection, emitting one edit per
    /// page so a selection spanning pages is annotated on all of them.
    pub(super) fn markup_selection(
        &mut self,
        markup: Markup,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selection.is_empty() {
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
        let mut pages: Vec<(usize, Vec<document_core::PdfRect>)> = Vec::new();
        for run in &self.selection {
            match pages.last_mut() {
                Some((page, rects)) if *page == run.page_index => rects.push(run.rect),
                _ => pages.push((run.page_index, vec![run.rect])),
            }
        }
        let page_count = pages.len();
        for (page_index, rects) in pages {
            self.history.push(match markup {
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
            });
        }
        self.clear_selection();
        self.mark_edited(window, cx);
        let scope = if page_count > 1 {
            format!(" across {page_count} pages")
        } else {
            String::new()
        };
        self.flash(
            format!("{} added{scope}", markup.label()),
            Severity::Info,
            cx,
        );
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

/// Where a selection starts or ends: a page plus an index into that page's
/// text runs.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
struct SelectionEdge {
    page_index: usize,
    fragment: usize,
}

/// Text runs between two edges, walking whole pages in between so a drag can
/// span any number of pages.
fn text_selection(
    pages: &[DocumentPage],
    anchor: SelectionEdge,
    head: SelectionEdge,
) -> Vec<SelectedRun> {
    let (first, last) = if anchor <= head {
        (anchor, head)
    } else {
        (head, anchor)
    };

    let mut runs = Vec::new();
    for page_index in first.page_index..=last.page_index.min(pages.len().saturating_sub(1)) {
        let Some(page) = pages.get(page_index) else {
            break;
        };
        if page.fragments.is_empty() {
            continue;
        }
        let start = if page_index == first.page_index {
            first.fragment
        } else {
            0
        };
        let end = if page_index == last.page_index {
            last.fragment
        } else {
            page.fragments.len() - 1
        };
        for fragment in start..=end.min(page.fragments.len() - 1) {
            let fragment = &page.fragments[fragment];
            runs.push(SelectedRun {
                page_index,
                rect: fragment.rect,
                text: fragment.text.clone(),
            });
        }
    }
    runs
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

/// PDF text runs carry no spaces, so word and line breaks are reconstructed
/// from the geometry of neighbouring runs.
fn join_selection(selection: &[SelectedRun]) -> String {
    let mut text = String::new();
    let mut previous: Option<&SelectedRun> = None;
    for run in selection {
        if let Some(last) = previous {
            if last.page_index == run.page_index {
                let line_height = (last.rect.y_max - last.rect.y_min).max(1.0);
                if (last.rect.y_min - run.rect.y_min).abs() > line_height * 0.5 {
                    text.push('\n');
                } else {
                    let gap = run.rect.x_min - last.rect.x_max;
                    let space = line_height * 0.22;
                    let ends_open = text.ends_with(|c: char| c.is_whitespace() || c == '-');
                    if gap > space && !ends_open && !run.text.starts_with(char::is_whitespace) {
                        text.push(' ');
                    }
                }
            } else {
                text.push('\n');
            }
        }
        text.push_str(&run.text);
        previous = Some(run);
    }
    text
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
    use pdf_engine::{PageMetadata, TextFragment};

    use super::{
        DocumentPage, MAX_ZOOM, MIN_ZOOM, SelectedRun, SelectionEdge, join_selection,
        next_zoom_step, text_selection,
    };

    fn page(index: usize, words: &[(&str, f64)]) -> DocumentPage {
        let box_rect = PdfRect::new(0.0, 0.0, 600.0, 800.0).unwrap();
        let geometry = document_core::PageGeometry::new(
            box_rect,
            box_rect,
            document_core::Rotation::None,
            1.0,
        )
        .unwrap();
        let mut page = DocumentPage::placeholder(PageMetadata { index, geometry });
        page.load_text(
            String::new(),
            words
                .iter()
                .map(|(text, x)| TextFragment {
                    text: (*text).to_owned(),
                    rect: PdfRect::new(*x, 100.0, x + 40.0, 110.0).unwrap(),
                })
                .collect(),
        );
        page
    }

    fn edge(page_index: usize, fragment: usize) -> SelectionEdge {
        SelectionEdge {
            page_index,
            fragment,
        }
    }

    #[test]
    fn selection_spans_whole_pages_between_its_edges() {
        let pages = vec![
            page(0, &[("one", 0.0), ("two", 50.0)]),
            page(1, &[("three", 0.0), ("four", 50.0)]),
            page(2, &[("five", 0.0), ("six", 50.0)]),
        ];

        let runs = text_selection(&pages, edge(0, 1), edge(2, 0));

        assert_eq!(
            runs.iter().map(|run| run.text.as_str()).collect::<Vec<_>>(),
            ["two", "three", "four", "five"]
        );
    }

    #[test]
    fn selection_is_the_same_when_dragged_backwards() {
        let pages = vec![
            page(0, &[("one", 0.0), ("two", 50.0)]),
            page(1, &[("three", 0.0)]),
        ];

        let forward = text_selection(&pages, edge(0, 1), edge(1, 0));
        let backward = text_selection(&pages, edge(1, 0), edge(0, 1));

        let texts =
            |runs: &[SelectedRun]| runs.iter().map(|run| run.text.clone()).collect::<Vec<_>>();
        assert_eq!(texts(&forward), texts(&backward));
        assert_eq!(texts(&forward), ["two", "three"]);
    }

    #[test]
    fn joined_selection_breaks_lines_between_pages() {
        let runs = vec![
            SelectedRun {
                page_index: 0,
                rect: PdfRect::new(0.0, 100.0, 40.0, 110.0).unwrap(),
                text: "end".to_owned(),
            },
            SelectedRun {
                page_index: 1,
                rect: PdfRect::new(0.0, 700.0, 40.0, 710.0).unwrap(),
                text: "start".to_owned(),
            },
        ];

        assert_eq!(join_selection(&runs), "end\nstart");
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
    fn selection_reconstructs_spaces_and_line_breaks() {
        let runs = vec![
            SelectedRun {
                page_index: 0,
                rect: PdfRect::new(0.0, 100.0, 40.0, 110.0).unwrap(),
                text: "Hello".to_owned(),
            },
            SelectedRun {
                page_index: 0,
                rect: PdfRect::new(46.0, 100.0, 90.0, 110.0).unwrap(),
                text: "world".to_owned(),
            },
            SelectedRun {
                page_index: 0,
                rect: PdfRect::new(0.0, 86.0, 30.0, 96.0).unwrap(),
                text: "next".to_owned(),
            },
        ];

        assert_eq!(join_selection(&runs), "Hello world\nnext");
    }

    #[test]
    fn selection_keeps_tight_runs_as_one_word() {
        let runs = vec![
            SelectedRun {
                page_index: 0,
                rect: PdfRect::new(0.0, 100.0, 20.0, 110.0).unwrap(),
                text: "Zusammen".to_owned(),
            },
            SelectedRun {
                page_index: 0,
                rect: PdfRect::new(20.2, 100.0, 40.0, 110.0).unwrap(),
                text: "fassung".to_owned(),
            },
        ];

        assert_eq!(join_selection(&runs), "Zusammenfassung");
    }

    #[test]
    fn point_far_from_any_text_matches_no_fragment() {
        let fragments = vec![TextFragment {
            text: "Hello".into(),
            rect: PdfRect::new(0.0, 0.0, 50.0, 10.0).unwrap(),
        }];

        assert!(super::nearest_fragment(&fragments, PdfPoint::new(0.0, 600.0)).is_none());
        assert_eq!(
            super::nearest_fragment(&fragments, PdfPoint::new(5.0, 5.0)),
            Some(0)
        );
    }
}
