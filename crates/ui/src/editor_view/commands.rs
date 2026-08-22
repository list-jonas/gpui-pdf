//! Commands reachable from context menus.
//!
//! Every entry here is an action so the same handler serves the right-click
//! menu, the menu bar and any key binding, and so menus can render the
//! matching shortcut next to their labels.

use gpui::{ClipboardItem, Context, Window};
use pdf_engine::{EditCommand, TextStamp};

use crate::EditorView;
use crate::actions::{
    AddNoteHere, AddTextHere, ClearEdits, CopyFilePath, CopyPageText, DeleteAnnotation, Deselect,
    EditAnnotation, FindSelection, HighlightSelection, PasteText, RedactSelection, RevealInFinder,
    StrikeoutSelection, UnderlineSelection,
};

use super::Severity;
use super::interaction::Markup;
use super::model::Tool;

/// Where a page context menu was opened, so placement commands land under the
/// pointer instead of at an arbitrary spot.
#[derive(Clone, Copy)]
pub(super) struct MenuTarget {
    pub page_index: usize,
    pub point: document_core::PdfPoint,
    /// Annotation under the pointer, when the click landed on one.
    pub edit_index: Option<usize>,
}

impl EditorView {
    pub(super) fn highlight_selection(
        &mut self,
        _: &HighlightSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.markup_selection(Markup::Highlight, window, cx);
    }

    pub(super) fn underline_selection(
        &mut self,
        _: &UnderlineSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.markup_selection(Markup::Underline, window, cx);
    }

    pub(super) fn strikeout_selection(
        &mut self,
        _: &StrikeoutSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.markup_selection(Markup::Strikeout, window, cx);
    }

    /// Redacts the bounding box of every selected run, so a text selection can
    /// be blacked out without redrawing the region by hand.
    pub(super) fn redact_selection(
        &mut self,
        _: &RedactSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selection.is_empty() {
            self.flash("Select text to redact", Severity::Info, cx);
            return;
        }
        let mut pages: Vec<(usize, document_core::PdfRect)> = Vec::new();
        for run in &self.selection {
            match pages.last_mut() {
                Some((page, rect)) if *page == run.page_index => *rect = union(*rect, run.rect),
                _ => pages.push((run.page_index, run.rect)),
            }
        }
        let count = pages.len();
        for (page_index, rect) in pages {
            self.history.push(EditCommand::Redact { page_index, rect });
        }
        self.clear_selection();
        self.mark_edited(window, cx);
        self.flash(
            format!("Redaction added on {count} page(s); applied on save"),
            Severity::Info,
            cx,
        );
    }

    /// Searches for the selected text, which is the usual "look this up"
    /// gesture in a reader.
    pub(super) fn find_selection(
        &mut self,
        _: &FindSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.has_selection() {
            self.flash("Select text to search for", Severity::Info, cx);
            return;
        }
        // A whole paragraph never matches anything, so the query is the first
        // line of the selection.
        let query: String = self
            .selected_text
            .lines()
            .next()
            .unwrap_or_default()
            .trim()
            .chars()
            .take(120)
            .collect();
        if query.is_empty() {
            self.flash("Select text to search for", Severity::Info, cx);
            return;
        }
        self.panels.search = true;
        self.search_input
            .update(cx, |input, cx| input.set_value(&query, window, cx));
        self.refresh_search(cx, true);
        self.flash(format!("Searching for \"{query}\""), Severity::Info, cx);
    }

    pub(super) fn deselect(&mut self, _: &Deselect, _: &mut Window, cx: &mut Context<Self>) {
        self.clear_selection();
        self.flash("Selection cleared", Severity::Info, cx);
        cx.notify();
    }

    /// Copies the text of the page under the pointer, which is faster than
    /// dragging across a full page.
    pub(super) fn copy_page_text(
        &mut self,
        _: &CopyPageText,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let page_index = self.menu_target.map_or(self.page_index, |t| t.page_index);
        let Some(page) = self.pages.get(page_index) else {
            return;
        };
        if !page.text_loaded {
            self.flash("Page text is still loading", Severity::Info, cx);
            return;
        }
        let text = join_fragments(&page.fragments);
        if text.trim().is_empty() {
            self.flash("This page has no extractable text", Severity::Info, cx);
            return;
        }
        let characters = text.chars().count();
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.flash(
            format!("Copied page {} ({characters} characters)", page_index + 1),
            Severity::Info,
            cx,
        );
    }

    /// Pastes clipboard text onto the page as an editable text stamp.
    pub(super) fn paste_text(
        &mut self,
        _: &PasteText,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(text) = cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .filter(|text| !text.trim().is_empty())
        else {
            self.flash("Clipboard has no text to paste", Severity::Info, cx);
            return;
        };
        let Some(target) = self.menu_target.or_else(|| self.center_target()) else {
            self.flash("Open a PDF before pasting", Severity::Info, cx);
            return;
        };
        self.history.push(EditCommand::AddText(TextStamp {
            page_index: target.page_index,
            text,
            x: target.point.x,
            y: target.point.y,
            size: 14.0,
        }));
        self.mark_edited(window, cx);
        self.flash("Pasted text onto page", Severity::Info, cx);
    }

    /// Starts a text draft where the menu was opened.
    pub(super) fn add_text_here(
        &mut self,
        _: &AddTextHere,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.menu_target.or_else(|| self.center_target()) else {
            return;
        };
        self.tool = Tool::AddText;
        self.start_inline_text(
            target.page_index,
            target.point,
            "Type on page",
            "",
            window,
            cx,
        );
    }

    /// Starts a comment draft where the menu was opened.
    pub(super) fn add_note_here(
        &mut self,
        _: &AddNoteHere,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.menu_target.or_else(|| self.center_target()) else {
            return;
        };
        self.tool = Tool::Note;
        self.start_inline_note(target.page_index, target.point, "", window, cx);
    }

    /// Reopens the annotation under the pointer for editing.
    pub(super) fn edit_annotation(
        &mut self,
        _: &EditAnnotation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(edit_index) = self
            .menu_target
            .and_then(|target| target.edit_index)
            .or(self.selected_edit)
        else {
            self.flash("Right-click an annotation to edit it", Severity::Info, cx);
            return;
        };
        match self.history.edits().get(edit_index) {
            Some(EditCommand::AddText(_)) => self.edit_text_overlay(edit_index, window, cx),
            Some(EditCommand::Note { .. }) => self.edit_note_overlay(edit_index, window, cx),
            _ => self.flash("This annotation has no editable text", Severity::Info, cx),
        }
    }

    /// Deletes the annotation the menu was opened on, falling back to the
    /// selected one.
    pub(super) fn delete_annotation(
        &mut self,
        _: &DeleteAnnotation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(edit_index) = self
            .menu_target
            .and_then(|target| target.edit_index)
            .or(self.selected_edit)
        else {
            self.flash("Right-click an annotation to delete it", Severity::Info, cx);
            return;
        };
        if self.history.remove(edit_index).is_some() {
            self.selected_edit = None;
            self.mark_edited(window, cx);
            self.flash("Annotation deleted", Severity::Info, cx);
        }
    }

    /// Drops every queued edit. The document on disk is untouched, so this is
    /// recoverable by not saving.
    pub(super) fn clear_edits(
        &mut self,
        _: &ClearEdits,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.history.is_empty() {
            self.flash("No pending edits", Severity::Info, cx);
            return;
        }
        let count = self.history.len();
        self.history.clear();
        self.clear_selection();
        self.inline_text = None;
        self.inline_note = None;
        self.mark_edited(window, cx);
        self.flash(
            format!("Discarded {count} pending edit(s)"),
            Severity::Info,
            cx,
        );
    }

    pub(super) fn reveal_in_finder(
        &mut self,
        _: &RevealInFinder,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.path.clone() else {
            self.flash("No document open", Severity::Info, cx);
            return;
        };
        cx.reveal_path(&path);
    }

    pub(super) fn copy_file_path(
        &mut self,
        _: &CopyFilePath,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.path.clone() else {
            self.flash("No document open", Severity::Info, cx);
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(path.display().to_string()));
        self.flash("Copied file path", Severity::Info, cx);
    }

    /// Fallback target for placement commands invoked without a pointer, such
    /// as from the menu bar: the top-left text origin of the current page.
    fn center_target(&self) -> Option<MenuTarget> {
        let page = self.pages.get(self.page_index)?;
        let box_rect = page.metadata.geometry.crop_box;
        Some(MenuTarget {
            page_index: self.page_index,
            point: document_core::PdfPoint::new(
                box_rect.x_min + box_rect.width() * 0.5,
                box_rect.y_min + box_rect.height() * 0.5,
            ),
            edit_index: None,
        })
    }
}

fn union(left: document_core::PdfRect, right: document_core::PdfRect) -> document_core::PdfRect {
    document_core::PdfRect::new(
        left.x_min.min(right.x_min),
        left.y_min.min(right.y_min),
        left.x_max.max(right.x_max),
        left.y_max.max(right.y_max),
    )
    .unwrap_or(left)
}

/// Reconstructs page text from its runs, which keeps copy output consistent
/// with what a selection of the same page produces.
fn join_fragments(fragments: &[pdf_engine::TextFragment]) -> String {
    let runs: Vec<super::model::SelectedRun> = fragments
        .iter()
        .map(|fragment| super::model::SelectedRun {
            page_index: 0,
            rect: fragment.rect,
            text: fragment.text.clone(),
        })
        .collect();
    super::interaction::join_selection(&runs)
}
