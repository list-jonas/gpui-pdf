mod document_io;
mod edits;
mod layout;

use std::path::PathBuf;
use std::sync::Arc;

use async_channel::{Receiver, Sender};
use gpui::{AppContext, Context, Entity, RenderImage, SharedString, Window};
use gpui_component::input::InputState;
use pdf_engine::{EditCommand, FormField};

use crate::field_input::FieldInput;
use crate::page_image::render_image;
use crate::{EditorRequest, EditorUpdate};

use self::document_io::file_name;

pub struct EditorView {
    requests: Sender<EditorRequest>,
    path: Option<PathBuf>,
    page_index: usize,
    page_count: usize,
    status: SharedString,
    detail: Option<SharedString>,
    extracted_text: Option<SharedString>,
    image: Option<Arc<RenderImage>>,
    forms: Vec<FieldInput>,
    edits: Vec<EditCommand>,
    add_text: Entity<InputState>,
    text_x: Entity<InputState>,
    text_y: Entity<InputState>,
    text_size: Entity<InputState>,
    redact_x0: Entity<InputState>,
    redact_y0: Entity<InputState>,
    redact_x1: Entity<InputState>,
    redact_y1: Entity<InputState>,
}

impl EditorView {
    pub fn new(
        requests: Sender<EditorRequest>,
        updates: Receiver<EditorUpdate>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.spawn_in(window, async move |view, cx| {
            while let Ok(update) = updates.recv().await {
                if view
                    .update_in(cx, |view, window, cx| view.apply(update, window, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();

        Self {
            requests,
            path: None,
            page_index: 0,
            page_count: 0,
            status: "Open a PDF to begin".into(),
            detail: None,
            extracted_text: None,
            image: None,
            forms: Vec::new(),
            edits: Vec::new(),
            add_text: input("Text to add", "", window, cx),
            text_x: input("X", "24", window, cx),
            text_y: input("Y", "24", window, cx),
            text_size: input("Size", "14", window, cx),
            redact_x0: input("Left", "", window, cx),
            redact_y0: input("Bottom", "", window, cx),
            redact_x1: input("Right", "", window, cx),
            redact_y1: input("Top", "", window, cx),
        }
    }

    fn apply(&mut self, update: EditorUpdate, window: &mut Window, cx: &mut Context<Self>) {
        match update {
            EditorUpdate::Loaded {
                path,
                document,
                page,
                rendered,
                text,
                forms,
            } => {
                self.path = Some(path.clone());
                self.page_index = page.index;
                self.page_count = document.page_count;
                let marker = if self.edits.is_empty() {
                    ""
                } else {
                    " · Edited"
                };
                self.status = format!(
                    "{} · page {} of {}{marker}",
                    file_name(&path),
                    page.index + 1,
                    document.page_count,
                )
                .into();
                self.detail = Some(
                    format!(
                        "{:.0} × {:.0} pt · {}° · PDF {}.{}",
                        page.geometry.crop_box.width(),
                        page.geometry.crop_box.height(),
                        page.geometry.rotation.degrees(),
                        document.pdf_version.0,
                        document.pdf_version.1
                    )
                    .into(),
                );
                self.extracted_text = Some(text.into());
                self.image = render_image(rendered);
                self.forms = forms
                    .into_iter()
                    .map(|field| {
                        let value = self.pending_form_value(&field);
                        FieldInput {
                            input: input(&field.name, &value, window, cx),
                            field,
                        }
                    })
                    .collect();
            }
            EditorUpdate::Saved(path) => {
                self.edits.clear();
                self.status = format!("Saved {}", path.display()).into();
            }
            EditorUpdate::Failed(message) => {
                self.status = "Operation failed".into();
                self.detail = Some(message.into());
            }
        }
        cx.notify();
    }

    fn pending_form_value(&self, field: &FormField) -> String {
        self.edits
            .iter()
            .rev()
            .find_map(|edit| match edit {
                EditCommand::FillForm { name, value } if name == &field.name => Some(value.clone()),
                _ => None,
            })
            .unwrap_or_else(|| field.value.clone())
    }
}

fn input(
    placeholder: &str,
    default: &str,
    window: &mut Window,
    cx: &mut Context<EditorView>,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder(placeholder.to_owned())
            .default_value(default.to_owned())
    })
}
