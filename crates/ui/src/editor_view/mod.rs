mod document_io;
mod edits;
mod geometry;
mod interaction;
mod layout;
mod model;
mod page_canvas;
mod properties;

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use async_channel::{Receiver, Sender};
use document_core::{PageGeometry, PdfRect};
use gpui::{
    AppContext, Bounds, Context, Entity, Pixels, RenderImage, ScrollHandle, SharedString, Window,
};
use gpui_component::input::InputState;
use pdf_engine::{EditCommand, FormField, TextFragment};

use crate::field_input::FieldInput;
use crate::page_image::render_image;
use crate::{EditorRequest, EditorUpdate};

use self::document_io::file_name;
use self::model::{DragState, InlineText, Tool};

pub struct EditorView {
    requests: Sender<EditorRequest>,
    path: Option<PathBuf>,
    page_index: usize,
    page_count: usize,
    status: SharedString,
    detail: Option<SharedString>,
    extracted_text: Option<SharedString>,
    image: Option<Arc<RenderImage>>,
    image_size: (u32, u32),
    page_geometry: Option<PageGeometry>,
    page_bounds: Rc<Cell<Bounds<Pixels>>>,
    scroll: ScrollHandle,
    forms: Vec<FieldInput>,
    fragments: Vec<TextFragment>,
    edits: Vec<EditCommand>,
    tool: Tool,
    zoom: f32,
    drag: Option<DragState>,
    selected_rects: Vec<PdfRect>,
    selected_text: SharedString,
    inline_text: Option<InlineText>,
    highlight_color: (f64, f64, f64),
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
            image_size: (0, 0),
            page_geometry: None,
            page_bounds: Rc::new(Cell::new(Bounds::default())),
            scroll: ScrollHandle::new(),
            forms: Vec::new(),
            fragments: Vec::new(),
            edits: Vec::new(),
            tool: Tool::Select,
            zoom: 1.0,
            drag: None,
            selected_rects: Vec::new(),
            selected_text: "No text selected".into(),
            inline_text: None,
            highlight_color: (1.0, 0.86, 0.2),
        }
    }

    fn apply(&mut self, update: EditorUpdate, window: &mut Window, cx: &mut Context<Self>) {
        match update {
            EditorUpdate::Loaded(loaded) => {
                let crate::LoadedDocument {
                    path,
                    document,
                    page,
                    rendered,
                    text,
                    fragments,
                    forms,
                } = *loaded;
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
                self.image_size = (rendered.width, rendered.height);
                self.image = render_image(rendered);
                self.page_geometry = Some(page.geometry);
                self.fragments = fragments;
                self.drag = None;
                self.selected_rects.clear();
                self.inline_text = None;
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

pub(super) fn input(
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
