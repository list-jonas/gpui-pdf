mod document_io;
mod document_page;
mod edits;
mod geometry;
mod gestures;
mod interaction;
mod layout;
mod model;
mod page_canvas;
mod properties;
mod search;

use std::path::PathBuf;

use async_channel::{Receiver, Sender};
use document_core::PdfRect;
use gpui::{
    AppContext, Context, Entity, FocusHandle, ScrollHandle, SharedString, Subscription, Window,
};
use gpui_component::input::InputState;
use pdf_engine::{EditCommand, FormField};

use crate::field_input::FieldInput;
use crate::page_image::render_image;
use crate::{EditorRequest, EditorUpdate};

use self::document_io::file_name;
use self::document_page::DocumentPage;
use self::model::{DragState, InlineText, SearchMatch, Tool};

pub struct EditorView {
    requests: Sender<EditorRequest>,
    path: Option<PathBuf>,
    page_index: usize,
    page_count: usize,
    status: SharedString,
    detail: Option<SharedString>,
    extracted_text: Option<SharedString>,
    pdf_version: (u8, u8),
    pages: Vec<DocumentPage>,
    loaded_pages: usize,
    focus_handle: FocusHandle,
    scroll: ScrollHandle,
    forms: Vec<FieldInput>,
    edits: Vec<EditCommand>,
    tool: Tool,
    zoom: f32,
    drag: Option<DragState>,
    selected_rects: Vec<PdfRect>,
    selected_text: SharedString,
    inline_text: Option<InlineText>,
    highlight_color: (f64, f64, f64),
    search_input: Entity<InputState>,
    _search_subscription: Subscription,
    search_query: String,
    search_matches: Vec<SearchMatch>,
    search_index: usize,
}

impl EditorView {
    pub fn new(
        requests: Sender<EditorRequest>,
        updates: Receiver<EditorUpdate>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window);
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

        let search_input = input("Search", "", window, cx);
        let search_subscription = cx.observe(&search_input, |view, _, cx| {
            view.refresh_search(cx, false);
        });

        Self {
            requests,
            path: None,
            page_index: 0,
            page_count: 0,
            status: "Open a PDF to begin".into(),
            detail: None,
            extracted_text: None,
            pdf_version: (0, 0),
            pages: Vec::new(),
            loaded_pages: 0,
            focus_handle,
            scroll: ScrollHandle::new(),
            forms: Vec::new(),
            edits: Vec::new(),
            tool: Tool::Select,
            zoom: 1.0,
            drag: None,
            selected_rects: Vec::new(),
            selected_text: "No text selected".into(),
            inline_text: None,
            highlight_color: (1.0, 0.86, 0.2),
            search_input,
            _search_subscription: search_subscription,
            search_query: String::new(),
            search_matches: Vec::new(),
            search_index: 0,
        }
    }

    fn apply(&mut self, update: EditorUpdate, window: &mut Window, cx: &mut Context<Self>) {
        match update {
            EditorUpdate::Opened(opened) => {
                let crate::OpenedDocument {
                    path,
                    document,
                    pages,
                    forms,
                    initial_page,
                } = *opened;
                self.path = Some(path.clone());
                self.page_index = initial_page;
                self.page_count = document.page_count;
                self.pdf_version = document.pdf_version;
                self.pages = pages.into_iter().map(DocumentPage::placeholder).collect();
                self.loaded_pages = 0;
                self.status = format!("Loading {} pages…", document.page_count).into();
                self.detail = None;
                self.extracted_text = None;
                self.drag = None;
                self.selected_rects.clear();
                self.inline_text = None;
                self.search_query.clear();
                self.search_matches.clear();
                self.search_index = 0;
                self.search_input
                    .update(cx, |input, cx| input.set_value("", window, cx));
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
                self.scroll.scroll_to_top_of_item(initial_page);
                self.refresh_active_page();
            }
            EditorUpdate::PageLoaded(loaded) => {
                let crate::LoadedPage {
                    page,
                    rendered,
                    text,
                    fragments,
                } = *loaded;
                let image_size = (rendered.width, rendered.height);
                if let Some(target) = self.pages.get_mut(page.index) {
                    let was_loaded = target.image.is_some();
                    target.load(render_image(rendered), image_size, text, fragments);
                    if !was_loaded {
                        self.loaded_pages += 1;
                    }
                }
                self.refresh_search(cx, true);
                self.refresh_active_page();
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

    fn refresh_active_page(&mut self) {
        let Some(page) = self.pages.get(self.page_index) else {
            return;
        };
        let marker = if self.edits.is_empty() {
            ""
        } else {
            " · Edited"
        };
        let name = self.path.as_deref().map_or_else(|| "PDF".into(), file_name);
        self.status = format!(
            "{name} · page {} of {} · loaded {}/{}{marker}",
            self.page_index + 1,
            self.page_count,
            self.loaded_pages,
            self.page_count,
        )
        .into();
        self.detail = Some(
            format!(
                "{:.0} × {:.0} pt · {}° · PDF {}.{}",
                page.metadata.geometry.crop_box.width(),
                page.metadata.geometry.crop_box.height(),
                page.metadata.geometry.rotation.degrees(),
                self.pdf_version.0,
                self.pdf_version.1
            )
            .into(),
        );
        self.extracted_text = (!page.text.is_empty()).then(|| page.text.clone());
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
