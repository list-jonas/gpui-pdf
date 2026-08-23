//! Right-click menus.
//!
//! Menu contents follow the convention Acrobat and other readers use: items
//! act on what was clicked, commands that cannot apply right now are shown
//! disabled rather than hidden, so the menu keeps a stable shape, and every
//! item is backed by an action so its keyboard shortcut is displayed.

use gpui::{Context, MouseButton, MouseDownEvent};
use gpui_component::menu::PopupMenu;
use pdf_engine::EditCommand;

use crate::EditorView;
use crate::actions::{
    ActualSize, AddNoteHere, AddTextHere, ClearEdits, CopyFilePath, CopyPageText, CopySelection,
    DeleteAnnotation, Deselect, DocumentProperties, EditAnnotation, FindSelection, FitPage,
    FitWidth, GoToPage, HighlightSelection, NextPage, OpenDocument, OpenInDefaultViewer, PasteText,
    PreviousPage, RedactSelection, RevealInFinder, SaveDocument, SaveDocumentAs, SelectAllText,
    StrikeoutSelection, ToggleFullScreen, TogglePropertiesPanel, ToggleReadingMode, ToggleSidebar,
    UnderlineSelection,
};

use super::commands::MenuTarget;

impl EditorView {
    /// Records what the pointer is over before a page menu opens, so the menu
    /// can be built for that spot and its commands act there.
    pub(super) fn prepare_page_menu(
        &mut self,
        page_index: usize,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Right {
            return;
        }
        let Some(point) = self.pdf_point(page_index, event.position) else {
            return;
        };
        // Right-clicking outside the selection acts on the click target, which
        // is what every other reader does. Clicking inside it keeps it.
        let edit_index = self.edit_at(page_index, point);
        if let Some(edit_index) = edit_index {
            self.select_edit(edit_index, cx);
        }
        self.set_current_page(page_index);
        self.menu_target = Some(MenuTarget {
            page_index,
            point,
            edit_index,
        });
        cx.notify();
    }

    /// The topmost annotation whose geometry contains the point, if any.
    fn edit_at(&self, page_index: usize, point: document_core::PdfPoint) -> Option<usize> {
        self.history
            .iter()
            .enumerate()
            .rev()
            .find(|(_, edit)| edit_contains(edit, page_index, point))
            .map(|(index, _)| index)
    }

    /// Page canvas menu: selection commands first, then placement, then view.
    pub(super) fn page_menu(&self, menu: PopupMenu) -> PopupMenu {
        let has_selection = self.has_selection();
        let has_annotation = self
            .menu_target
            .and_then(|target| target.edit_index)
            .is_some();
        let annotation_editable = self
            .menu_target
            .and_then(|target| target.edit_index)
            .and_then(|index| self.history.edits().get(index))
            .is_some_and(|edit| matches!(edit, EditCommand::AddText(_) | EditCommand::Note { .. }));

        // An annotation under the pointer is the most specific target, so its
        // commands replace the generic menu instead of extending it.
        if has_annotation {
            return menu
                .menu_with_enable("Edit", Box::new(EditAnnotation), annotation_editable)
                .menu("Delete", Box::new(DeleteAnnotation));
        }

        if has_selection {
            return menu
                .menu("Copy", Box::new(CopySelection))
                .menu("Search for Selection", Box::new(FindSelection))
                .separator()
                .menu("Highlight", Box::new(HighlightSelection))
                .menu("Underline", Box::new(UnderlineSelection))
                .menu("Strike Out", Box::new(StrikeoutSelection))
                .menu("Redact", Box::new(RedactSelection))
                .separator()
                .menu("Deselect", Box::new(Deselect));
        }

        menu.menu("Add Text Here", Box::new(AddTextHere))
            .menu("Add Comment Here", Box::new(AddNoteHere))
            .menu("Paste as Text", Box::new(PasteText))
            .separator()
            .menu("Select All Text", Box::new(SelectAllText))
            .menu("Copy Page Text", Box::new(CopyPageText))
            .separator()
            .menu("Fit Page", Box::new(FitPage))
            .menu("Fit Width", Box::new(FitWidth))
    }

    /// Thumbnail menu: the page commands that need a specific page.
    pub(super) fn thumbnail_menu(&self, page_index: usize, menu: PopupMenu) -> PopupMenu {
        let at_start = page_index == 0;
        let at_end = page_index + 1 >= self.page_count;
        menu.menu_with_enable("Previous Page", Box::new(PreviousPage), !at_start)
            .menu_with_enable("Next Page", Box::new(NextPage), !at_end)
            .menu("Go to Page…", Box::new(GoToPage))
            .separator()
            .menu("Copy Page Text", Box::new(CopyPageText))
            .separator()
            .menu("Hide Page Thumbnails", Box::new(ToggleSidebar))
    }

    /// Title bar menu: document-level commands, like a Finder proxy icon.
    pub(super) fn document_menu(&self, menu: PopupMenu) -> PopupMenu {
        let has_document = self.path.is_some();
        let has_edits = !self.history.is_empty();
        menu.menu_with_enable("Save", Box::new(SaveDocument), has_document && has_edits)
            .menu_with_enable("Save As…", Box::new(SaveDocumentAs), has_document)
            .menu("Open…", Box::new(OpenDocument))
            .separator()
            .menu_with_enable("Discard Pending Edits", Box::new(ClearEdits), has_edits)
            .separator()
            .menu_with_enable(
                "Document Properties",
                Box::new(DocumentProperties),
                has_document,
            )
            .menu_with_enable(
                "Open in Default Viewer",
                Box::new(OpenInDefaultViewer),
                has_document,
            )
            .menu_with_enable("Reveal in Finder", Box::new(RevealInFinder), has_document)
            .menu_with_enable("Copy File Path", Box::new(CopyFilePath), has_document)
    }

    /// Chrome menu: toggles for the panel that was clicked, plus fit.
    pub(super) fn view_menu(&self, menu: PopupMenu) -> PopupMenu {
        menu.menu_with_check(
            "Page Thumbnails",
            self.panels.sidebar,
            Box::new(ToggleSidebar),
        )
        .menu_with_check(
            "Properties Panel",
            self.panels.properties,
            Box::new(TogglePropertiesPanel),
        )
        .menu_with_check(
            "Reading Mode",
            self.reading_mode,
            Box::new(ToggleReadingMode),
        )
        .separator()
        .menu("Fit Page", Box::new(FitPage))
        .menu("Fit Width", Box::new(FitWidth))
        .menu("Actual Size", Box::new(ActualSize))
        .separator()
        .menu("Full Screen Mode", Box::new(ToggleFullScreen))
    }

    /// Pending-edit row menu.
    pub(super) fn edit_row_menu(&self, edit_index: usize, menu: PopupMenu) -> PopupMenu {
        let editable =
            self.history.edits().get(edit_index).is_some_and(|edit| {
                matches!(edit, EditCommand::AddText(_) | EditCommand::Note { .. })
            });
        menu.menu_with_enable("Edit", Box::new(EditAnnotation), editable)
            .menu("Delete", Box::new(DeleteAnnotation))
            .separator()
            .menu("Discard All Pending Edits", Box::new(ClearEdits))
    }

    /// Points the annotation commands at a pending-edit row before its menu
    /// opens, so Edit and Delete act on that row.
    pub(super) fn prepare_edit_row_menu(
        &mut self,
        edit_index: usize,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Right {
            return;
        }
        let page_index = self
            .history
            .edits()
            .get(edit_index)
            .and_then(edit_page)
            .unwrap_or(self.page_index);
        self.menu_target = Some(MenuTarget {
            page_index,
            point: document_core::PdfPoint::new(0.0, 0.0),
            edit_index: Some(edit_index),
        });
        self.selected_edit = Some(edit_index);
        cx.notify();
    }

    /// Clears the pointer target so a menu opened from chrome does not act on
    /// a stale page position.
    pub(super) fn clear_menu_target(&mut self, event: &MouseDownEvent) {
        if event.button == MouseButton::Right {
            self.menu_target = None;
        }
    }
}

fn edit_page(edit: &EditCommand) -> Option<usize> {
    match edit {
        EditCommand::AddText(stamp) => Some(stamp.page_index),
        EditCommand::Redact { page_index, .. }
        | EditCommand::Highlight { page_index, .. }
        | EditCommand::Underline { page_index, .. }
        | EditCommand::StrikeOut { page_index, .. }
        | EditCommand::Note { page_index, .. }
        | EditCommand::Shape { page_index, .. } => Some(*page_index),
        EditCommand::FillForm { .. } => None,
    }
}

/// Hit test for annotations, in PDF space. Point annotations have no rect of
/// their own, so they use the same box their overlay paints.
fn edit_contains(edit: &EditCommand, page: usize, point: document_core::PdfPoint) -> bool {
    /// Painted size of a text stamp and comment, in PDF points.
    const NOTE_WIDTH: f64 = 160.0;
    const NOTE_HEIGHT: f64 = 72.0;
    const TEXT_HEIGHT: f64 = 20.0;

    let contains = |rect: &document_core::PdfRect| {
        point.x >= rect.x_min
            && point.x <= rect.x_max
            && point.y >= rect.y_min
            && point.y <= rect.y_max
    };
    match edit {
        EditCommand::Redact { page_index, rect }
        | EditCommand::Shape {
            page_index, rect, ..
        } => *page_index == page && contains(rect),
        EditCommand::Highlight {
            page_index, rects, ..
        }
        | EditCommand::Underline {
            page_index, rects, ..
        }
        | EditCommand::StrikeOut {
            page_index, rects, ..
        } => *page_index == page && rects.iter().any(contains),
        EditCommand::Note {
            page_index, x, y, ..
        } => {
            *page_index == page
                && point.x >= *x
                && point.x <= x + NOTE_WIDTH
                && point.y >= *y
                && point.y <= y + NOTE_HEIGHT
        }
        EditCommand::AddText(stamp) => {
            let characters =
                f64::from(u32::try_from(stamp.text.chars().count()).unwrap_or(u32::MAX));
            let width = NOTE_WIDTH.min(characters * stamp.size * 0.6);
            stamp.page_index == page
                && point.x >= stamp.x
                && point.x <= stamp.x + width.max(stamp.size)
                && point.y >= stamp.y
                && point.y <= stamp.y + TEXT_HEIGHT.max(stamp.size)
        }
        EditCommand::FillForm { .. } => false,
    }
}

#[cfg(test)]
mod tests {
    use document_core::{PdfPoint, PdfRect};
    use pdf_engine::{EditCommand, TextStamp};

    use super::edit_contains;

    #[test]
    fn region_annotations_are_hit_only_on_their_own_page() {
        let edit = EditCommand::Redact {
            page_index: 1,
            rect: PdfRect::new(10.0, 10.0, 50.0, 40.0).unwrap(),
        };

        assert!(edit_contains(&edit, 1, PdfPoint::new(20.0, 20.0)));
        assert!(!edit_contains(&edit, 0, PdfPoint::new(20.0, 20.0)));
        assert!(!edit_contains(&edit, 1, PdfPoint::new(80.0, 20.0)));
    }

    #[test]
    fn markup_is_hit_through_any_of_its_rects() {
        let edit = EditCommand::Highlight {
            page_index: 0,
            rects: vec![
                PdfRect::new(0.0, 0.0, 10.0, 10.0).unwrap(),
                PdfRect::new(40.0, 0.0, 50.0, 10.0).unwrap(),
            ],
            color: (1.0, 1.0, 0.0),
        };

        assert!(edit_contains(&edit, 0, PdfPoint::new(45.0, 5.0)));
        assert!(!edit_contains(&edit, 0, PdfPoint::new(25.0, 5.0)));
    }

    /// Text stamps carry no rectangle, so the hit box is derived from the
    /// glyph size and must still cover the text that was typed.
    #[test]
    fn text_stamps_use_a_hit_box_around_their_baseline() {
        let edit = EditCommand::AddText(TextStamp {
            page_index: 0,
            text: "Hello".to_owned(),
            x: 100.0,
            y: 200.0,
            size: 14.0,
        });

        assert!(edit_contains(&edit, 0, PdfPoint::new(105.0, 205.0)));
        assert!(!edit_contains(&edit, 0, PdfPoint::new(105.0, 260.0)));
    }
}
