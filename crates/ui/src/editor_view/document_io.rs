use std::path::{Path, PathBuf};

use gpui::{App, Context, PathPromptOptions, Window};
use pdf_engine::{EditCommand, FormFieldKind};

use crate::actions::{NextPage, OpenDocument, PreviousPage, SaveDocument};
use crate::{EditorRequest, EditorView};

impl EditorView {
    pub(super) fn open_picker(&mut self, _: &OpenDocument, _: &mut Window, cx: &mut Context<Self>) {
        self.status = "Choose a PDF…".into();
        cx.notify();
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
        self.detail = Some(path.display().to_string().into());
        self.edits.clear();
        let _ = self.requests.try_send(EditorRequest::Open(path));
    }

    pub(super) fn save_picker(&mut self, _: &SaveDocument, _: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = self.path.clone() else {
            self.status = "Open a PDF before saving".into();
            cx.notify();
            return;
        };
        self.capture_form_edits(cx);
        let edits = self.edits.clone();
        let directory = source.parent().unwrap_or_else(|| Path::new("/"));
        let suggestion = format!("{}-edited.pdf", file_stem(&source));
        let receiver = cx.prompt_for_new_path(directory, Some(&suggestion));
        cx.spawn(async move |view, cx| {
            if let Ok(Ok(Some(destination))) = receiver.await {
                let _ = view.update(cx, |view, cx| {
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
            self.edits.retain(|edit| {
                !matches!(edit, EditCommand::FillForm { name: existing, .. } if existing == &name)
            });
            if value != original {
                self.edits.push(EditCommand::FillForm { name, value });
            }
        }
    }

    pub(super) fn previous_page(
        &mut self,
        _: &PreviousPage,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.page_index > 0 {
            self.load_page(self.page_index - 1, cx);
        }
    }

    pub(super) fn next_page(&mut self, _: &NextPage, _: &mut Window, cx: &mut Context<Self>) {
        if self.page_index + 1 < self.page_count {
            self.load_page(self.page_index + 1, cx);
        }
    }

    fn load_page(&mut self, page_index: usize, cx: &mut Context<Self>) {
        let Some(path) = self.path.clone() else {
            return;
        };
        self.capture_form_edits(cx);
        self.status = format!("Loading page {}…", page_index + 1).into();
        let _ = self
            .requests
            .try_send(EditorRequest::LoadPage { path, page_index });
        cx.notify();
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
