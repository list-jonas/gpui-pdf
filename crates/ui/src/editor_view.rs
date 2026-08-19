use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_channel::{Receiver, Sender};
use document_core::PdfRect;
use gpui::{
    App, AppContext, Context, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement,
    PathPromptOptions, Render, RenderImage, SharedString, Styled, Window, div, img,
    prelude::FluentBuilder,
};
use gpui_component::Disableable;
use gpui_component::button::Button;
use gpui_component::input::{Input, InputState};
use gpui_component::scroll::ScrollableElement;
use pdf_engine::{EditCommand, FormField, FormFieldKind, TextStamp};

use crate::actions::{NextPage, OpenDocument, PreviousPage, SaveDocument};
use crate::controls::{labeled_input, section};
use crate::field_input::FieldInput;
use crate::page_image::render_image;
use crate::{EditorRequest, EditorUpdate};

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
                    .update_in(cx, |view, window, cx| {
                        view.apply(update, window, cx);
                    })
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
            redact_x0: input("Left", "40", window, cx),
            redact_y0: input("Bottom", "40", window, cx),
            redact_x1: input("Right", "200", window, cx),
            redact_y1: input("Top", "80", window, cx),
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
                self.status = format!(
                    "{} · page {} of {}{}",
                    file_name(&path),
                    page.index + 1,
                    document.page_count,
                    if self.edits.is_empty() {
                        ""
                    } else {
                        " · Edited"
                    }
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

    fn open_picker(&mut self, _: &OpenDocument, _: &mut Window, cx: &mut Context<Self>) {
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

    fn save_picker(&mut self, _: &SaveDocument, _: &mut Window, cx: &mut Context<Self>) {
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

    fn add_text(&mut self, cx: &mut Context<Self>) {
        let result = (|| {
            let text = value(&self.add_text, cx);
            if text.trim().is_empty() {
                return Err("Enter text first".to_owned());
            }
            Ok(EditCommand::AddText(TextStamp {
                page_index: self.page_index,
                text,
                x: number(&self.text_x, cx, "X")?,
                y: number(&self.text_y, cx, "Y")?,
                size: positive_number(&self.text_size, cx, "font size")?,
            }))
        })();
        self.push_edit(result, "Text queued; use Save As to write it", cx);
    }

    fn add_redaction(&mut self, cx: &mut Context<Self>) {
        let result = (|| {
            let rect = PdfRect::new(
                number(&self.redact_x0, cx, "left")?,
                number(&self.redact_y0, cx, "bottom")?,
                number(&self.redact_x1, cx, "right")?,
                number(&self.redact_y1, cx, "top")?,
            )
            .map_err(|error| error.to_string())?;
            Ok(EditCommand::Redact {
                page_index: self.page_index,
                rect,
            })
        })();
        self.push_edit(
            result,
            "Redaction queued; Save As permanently removes content",
            cx,
        );
    }

    fn push_edit(
        &mut self,
        edit: Result<EditCommand, String>,
        message: &'static str,
        cx: &mut Context<Self>,
    ) {
        match edit {
            Ok(edit) => {
                self.edits.push(edit);
                self.status = message.into();
            }
            Err(error) => self.status = error.into(),
        }
        cx.notify();
    }

    fn previous_page(&mut self, _: &PreviousPage, _: &mut Window, cx: &mut Context<Self>) {
        if self.page_index > 0 {
            self.load_page(self.page_index - 1, cx);
        }
    }

    fn next_page(&mut self, _: &NextPage, _: &mut Window, cx: &mut Context<Self>) {
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

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .p_3()
            .bg(gpui::rgb(0x00ff_ffff))
            .border_b_1()
            .border_color(gpui::rgb(0x00df_e1e5))
            .child(Button::new("open").label("Open PDF").on_click(
                cx.listener(|view, _, window, cx| view.open_picker(&OpenDocument, window, cx)),
            ))
            .child(Button::new("save").label("Save As…").on_click(
                cx.listener(|view, _, window, cx| view.save_picker(&SaveDocument, window, cx)),
            ))
            .child(
                Button::new("previous")
                    .label("Previous")
                    .disabled(self.page_index == 0)
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.previous_page(&PreviousPage, window, cx)
                    })),
            )
            .child(
                Button::new("next")
                    .label("Next")
                    .disabled(self.page_count == 0 || self.page_index + 1 >= self.page_count)
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.next_page(&NextPage, window, cx);
                    })),
            )
            .child(
                div()
                    .ml_auto()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.status.clone()),
            )
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let forms = self.forms.iter().fold(
            section(format!("Form fields ({})", self.forms.len())),
            |section, item| {
                section.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .child(format!("{} · {:?}", item.field.name, item.field.kind)),
                        )
                        .child(Input::new(&item.input).disabled(
                            item.field.read_only || item.field.kind == FormFieldKind::Signature,
                        )),
                )
            },
        );

        div()
            .w_80()
            .flex_shrink_0()
            .h_full()
            .overflow_y_scrollbar()
            .p_3()
            .flex()
            .flex_col()
            .gap_3()
            .bg(gpui::rgb(0x00f7_f8fa))
            .border_l_1()
            .border_color(gpui::rgb(0x00df_e1e5))
            .child(forms)
            .child(
                section("Add text")
                    .child(labeled_input("Text", &self.add_text))
                    .child(
                        div()
                            .grid()
                            .grid_cols(3)
                            .gap_2()
                            .child(labeled_input("X", &self.text_x))
                            .child(labeled_input("Y", &self.text_y))
                            .child(labeled_input("Size", &self.text_size)),
                    )
                    .child(
                        Button::new("queue-text")
                            .label("Queue text")
                            .on_click(cx.listener(|view, _, _, cx| view.add_text(cx))),
                    ),
            )
            .child(
                section("Permanent redaction")
                    .child(
                        div()
                            .text_xs()
                            .child("PDF coordinates: left, bottom, right, top"),
                    )
                    .child(
                        div()
                            .grid()
                            .grid_cols(2)
                            .gap_2()
                            .child(labeled_input("Left", &self.redact_x0))
                            .child(labeled_input("Bottom", &self.redact_y0))
                            .child(labeled_input("Right", &self.redact_x1))
                            .child(labeled_input("Top", &self.redact_y1)),
                    )
                    .child(
                        Button::new("queue-redaction")
                            .label("Queue redaction")
                            .on_click(cx.listener(|view, _, _, cx| view.add_redaction(cx))),
                    ),
            )
    }
}

impl Render for EditorView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context("PdfEditor")
            .on_action(cx.listener(Self::open_picker))
            .on_action(cx.listener(Self::save_picker))
            .on_action(cx.listener(Self::previous_page))
            .on_action(cx.listener(Self::next_page))
            .flex()
            .flex_col()
            .size_full()
            .bg(gpui::rgb(0x00e9_ebef))
            .text_color(gpui::rgb(0x0020_2124))
            .child(self.render_toolbar(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_w_0()
                            .p_5()
                            .items_center()
                            .justify_center()
                            .when_some(self.image.clone(), |view, image| {
                                view.child(
                                    div()
                                        .size_full()
                                        .bg(gpui::rgb(0x00ff_ffff))
                                        .shadow_lg()
                                        .child(img(image).size_full()),
                                )
                            })
                            .when(self.image.is_none(), |view| {
                                view.child("Open a PDF with Cmd+O or from Finder")
                            }),
                    )
                    .child(self.render_sidebar(cx)),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .bg(gpui::rgb(0x00ff_ffff))
                    .when_some(self.detail.clone(), ParentElement::child)
                    .when_some(self.extracted_text.clone(), |view, text| {
                        view.child(div().truncate().child(text))
                    }),
            )
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

fn value(input: &Entity<InputState>, cx: &App) -> String {
    input.read(cx).value().to_string()
}

fn number(input: &Entity<InputState>, cx: &App, label: &str) -> Result<f64, String> {
    value(input, cx)
        .parse::<f64>()
        .map_err(|_| format!("{label} must be a number"))
        .and_then(|number| {
            number
                .is_finite()
                .then_some(number)
                .ok_or_else(|| format!("{label} must be finite"))
        })
}

fn positive_number(input: &Entity<InputState>, cx: &App, label: &str) -> Result<f64, String> {
    let number = number(input, cx, label)?;
    (number > 0.0)
        .then_some(number)
        .ok_or_else(|| format!("{label} must be positive"))
}

fn file_name(path: &Path) -> String {
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
