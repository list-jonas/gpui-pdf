use std::path::{Path, PathBuf};

use gpui::{App, Context, PathPromptOptions, Window};
use pdf_engine::{EditCommand, FormFieldKind};

use crate::actions::{
    FirstPage, GoToPage, LastPage, NextPage, OpenDocument, PreviousPage, SaveDocument,
    SaveDocumentAs,
};
use crate::{EditorRequest, EditorView};

use super::Severity;

impl EditorView {
    pub(super) fn open_picker(&mut self, _: &OpenDocument, _: &mut Window, cx: &mut Context<Self>) {
        self.busy = false;
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open PDF".into()),
        });
        cx.spawn(async move |view, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = view.update(cx, |view, cx| {
                    view.open_path(path);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn open_path(&mut self, path: PathBuf) {
        self.status = "Loading PDF…".into();
        self.busy = true;
        self.detail = Some(path.display().to_string().into());
        self.history.clear();
        let _ = self.requests.try_send(EditorRequest::Open(path));
    }

    /// Save writes back to the open file; Save As always prompts.
    pub(super) fn save_document(
        &mut self,
        _: &SaveDocument,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(source) = self.path.clone() else {
            self.flash("Open a PDF before saving", Severity::Error, cx);
            return;
        };
        self.capture_form_edits(cx);
        self.materialize_inline_edits(cx);
        if self.history.is_empty() {
            self.flash("No changes to save", Severity::Info, cx);
            return;
        }
        let edits = self.history.to_vec();
        self.busy = true;
        self.flash("Saving…", Severity::Info, cx);
        let _ = self.requests.try_send(EditorRequest::SaveAs {
            source: source.clone(),
            destination: source,
            page_index: self.page_index,
            edits,
        });
        let _ = window;
    }

    pub(super) fn save_picker(
        &mut self,
        _: &SaveDocumentAs,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(source) = self.path.clone() else {
            self.flash("Open a PDF before saving", Severity::Error, cx);
            return;
        };
        self.capture_form_edits(cx);
        self.materialize_inline_edits(cx);
        let edits = self.history.to_vec();
        let directory = source.parent().unwrap_or_else(|| Path::new("/"));
        let suggestion = format!("{}-edited.pdf", file_stem(&source));
        let receiver = cx.prompt_for_new_path(directory, Some(&suggestion));
        cx.spawn(async move |view, cx| {
            if let Ok(Ok(Some(destination))) = receiver.await {
                let _ = view.update(cx, |view, cx| {
                    view.busy = true;
                    view.status = "Saving edited PDF…".into();
                    let _ = view.requests.try_send(EditorRequest::SaveAs {
                        source,
                        destination,
                        page_index: view.page_index,
                        edits,
                    });
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn capture_form_edits(&mut self, cx: &App) {
        let values: Vec<_> = self
            .forms
            .iter()
            .filter(|item| !item.field.read_only && item.field.kind != FormFieldKind::Signature)
            .map(|item| {
                (
                    item.field.name.clone(),
                    item.field.value.clone(),
                    item.value(cx),
                )
            })
            .collect();
        for (name, original, value) in values {
            self.history.retain(|edit| {
                !matches!(edit, EditCommand::FillForm { name: existing, .. } if existing == &name)
            });
            if value != original {
                self.history.push(EditCommand::FillForm { name, value });
            }
        }
    }

    pub(super) fn previous_page(
        &mut self,
        _: &PreviousPage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.input_has_focus(window, cx) && self.page_index > 0 {
            self.jump_to_page(self.page_index - 1, cx);
        }
    }

    pub(super) fn next_page(&mut self, _: &NextPage, window: &mut Window, cx: &mut Context<Self>) {
        if !self.input_has_focus(window, cx) && self.page_index + 1 < self.page_count {
            self.jump_to_page(self.page_index + 1, cx);
        }
    }

    pub(super) fn first_page(
        &mut self,
        _: &FirstPage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.input_has_focus(window, cx) {
            self.jump_to_page(0, cx);
        }
    }

    pub(super) fn last_page(&mut self, _: &LastPage, window: &mut Window, cx: &mut Context<Self>) {
        if !self.input_has_focus(window, cx) && self.page_count > 0 {
            self.jump_to_page(self.page_count - 1, cx);
        }
    }

    pub(super) fn go_to_page(&mut self, _: &GoToPage, window: &mut Window, cx: &mut Context<Self>) {
        let page_input = self.page_input.clone();
        window.defer(cx, move |window, cx| {
            page_input.update(cx, |input, cx| input.focus(window, cx));
        });
    }

    pub(super) fn jump_to_page(&mut self, page_index: usize, cx: &mut Context<Self>) {
        if page_index >= self.pages.len() {
            return;
        }
        self.capture_form_edits(cx);
        self.set_current_page(page_index);
        self.scroll.scroll_to_top_of_item(page_index);
        self.thumbnail_scroll.scroll_to_item(page_index);
        self.request_visible_pages();
        cx.notify();
    }

    pub(super) fn set_current_page(&mut self, page_index: usize) {
        if page_index < self.pages.len() {
            self.page_index = page_index;
            self.refresh_active_page();
        }
    }

    pub(super) fn sync_current_page_from_scroll(&mut self) {
        if self.pages.is_empty() || self.scroll.children_count() == 0 {
            return;
        }
        self.set_current_page(self.scroll.top_item().min(self.pages.len() - 1));
    }
}

pub(super) fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("PDF")
        .to_owned()
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("document")
        .to_owned()
}
