mod commands;
mod context_menus;
mod document_io;
mod document_page;
mod edits;
mod form_choice;
mod geometry;
mod gestures;
mod history;
mod interaction;
mod layout;
mod model;
mod page_canvas;
mod properties;
mod schedule;
mod search;
mod selection_paint;

use std::path::PathBuf;
use std::time::Duration;

use async_channel::{Receiver, Sender};
use gpui::{
    AppContext, Context, Entity, FocusHandle, ScrollHandle, SharedString, Subscription, Task,
    Window,
};
use gpui_component::input::InputState;
use pdf_engine::{EditCommand, FormField, FormValidation, ShapeKind};

use crate::field_input::FieldInput;
use crate::page_image::render_image;
use crate::{EditorRequest, EditorUpdate};

use self::commands::MenuTarget;
use self::document_io::file_name;
use self::document_page::DocumentPage;
use self::history::EditHistory;
use self::model::{
    ChoiceMenuState, DragState, InlineNote, InlineText, PanelVisibility, SearchMatch, SelectedRun,
    Tool,
};
use self::selection_paint::SelectionOverlays;

/// How a transient message is presented in the status bar.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Severity {
    Info,
    Error,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum DocumentSecurity {
    Unknown,
    Unencrypted,
    Encrypted,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum CloseState {
    Uninstalled,
    Ready,
    AfterSave,
}

pub struct EditorView {
    requests: Sender<EditorRequest>,
    path: Option<PathBuf>,
    page_index: usize,
    page_count: usize,
    status: SharedString,
    severity: Severity,
    busy: bool,
    detail: Option<SharedString>,
    extracted_text: Option<SharedString>,
    pdf_version: (u8, u8),
    security: DocumentSecurity,
    pages: Vec<DocumentPage>,
    loaded_pages: usize,
    /// Identifies the open document, so late results from a previous file are
    /// discarded instead of painted over the current one.
    token: u64,
    /// Set once the viewport is satisfied and the rest of the document is
    /// being filled in behind the scenes.
    background_requested: bool,
    /// Visible page range the scheduler last acted on, so a settled layout is
    /// only re-checked when it actually changed.
    settled_viewport: Option<(usize, usize)>,
    focus_handle: FocusHandle,
    scroll: ScrollHandle,
    forms: Vec<FieldInput>,
    choice_menu: Option<ChoiceMenuState>,
    history: EditHistory,
    tool: Tool,
    zoom: f32,
    drag: Option<DragState>,
    selection: Vec<SelectedRun>,
    /// Merged, per-page selection geometry used for painting.
    selection_overlays: SelectionOverlays,
    selected_text: SharedString,
    /// Truncated copy of the selection shown in the properties panel.
    selected_preview: SharedString,
    /// Index into the edit history of the annotation the user clicked, so it
    /// can be deleted with the keyboard.
    selected_edit: Option<usize>,
    inline_text: Option<InlineText>,
    inline_note: Option<InlineNote>,
    highlight_color: (f64, f64, f64),
    annotation_color: (f64, f64, f64),
    shape_kind: ShapeKind,
    search_input: Entity<InputState>,
    _search_subscription: Subscription,
    _search_enter_subscription: Subscription,
    search_query: String,
    search_matches: Vec<SearchMatch>,
    search_index: usize,
    page_input: Entity<InputState>,
    _page_subscription: Subscription,
    thumbnail_scroll: ScrollHandle,
    panels: PanelVisibility,
    reading_mode: bool,
    close_state: CloseState,
    status_reset: Option<Task<()>>,
    /// Page position the last context menu was opened at, so its commands act
    /// where the user right-clicked rather than on the current page origin.
    menu_target: Option<MenuTarget>,
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
        let search_subscription = cx.observe_in(&search_input, window, |view, _, window, cx| {
            view.refresh_search(cx, false);
            view.sync_page_input(window, cx);
        });
        let search_enter = cx.subscribe_in(
            &search_input,
            window,
            |view, _, event: &gpui_component::input::InputEvent, window, cx| {
                if let gpui_component::input::InputEvent::PressEnter { secondary } = event {
                    if *secondary {
                        view.previous_search_result(
                            &crate::actions::PreviousSearchResult,
                            window,
                            cx,
                        );
                    } else {
                        view.next_search_result(&crate::actions::NextSearchResult, window, cx);
                    }
                }
            },
        );
        let page_input = input("Page", "", window, cx);
        let page_subscription = cx.subscribe_in(
            &page_input,
            window,
            |view, _, event: &gpui_component::input::InputEvent, window, cx| {
                if matches!(event, gpui_component::input::InputEvent::PressEnter { .. }) {
                    view.commit_page_input(window, cx);
                }
            },
        );

        Self {
            requests,
            path: None,
            page_index: 0,
            page_count: 0,
            status: "Open a PDF to begin".into(),
            severity: Severity::Info,
            busy: false,
            detail: None,
            extracted_text: None,
            pdf_version: (0, 0),
            security: DocumentSecurity::Unknown,
            pages: Vec::new(),
            loaded_pages: 0,
            token: 0,
            background_requested: false,
            settled_viewport: None,
            focus_handle,
            scroll: ScrollHandle::new(),
            forms: Vec::new(),
            choice_menu: None,
            history: EditHistory::default(),
            tool: Tool::Select,
            zoom: 1.0,
            drag: None,
            selection: Vec::new(),
            selection_overlays: SelectionOverlays::default(),
            selected_text: "No text selected".into(),
            selected_preview: "No text selected".into(),
            selected_edit: None,
            inline_text: None,
            inline_note: None,
            highlight_color: (1.0, 0.86, 0.2),
            annotation_color: (0.23, 0.51, 0.96),
            shape_kind: ShapeKind::Rectangle,
            search_input,
            _search_subscription: search_subscription,
            _search_enter_subscription: search_enter,
            search_query: String::new(),
            search_matches: Vec::new(),
            search_index: 0,
            page_input,
            _page_subscription: page_subscription,
            thumbnail_scroll: ScrollHandle::new(),
            panels: PanelVisibility::default(),
            reading_mode: false,
            close_state: CloseState::Uninstalled,
            status_reset: None,
            menu_target: None,
        }
    }

    fn apply(&mut self, update: EditorUpdate, window: &mut Window, cx: &mut Context<Self>) {
        match update {
            EditorUpdate::Opened(opened) => self.open_document(*opened, window, cx),
            EditorUpdate::PageRendered {
                token,
                page_index,
                scale,
                kind,
                rendered,
            } => {
                if token != self.token {
                    return;
                }
                let preview = kind == crate::PageKind::Preview;
                let bytes = u64::from(rendered.width) * u64::from(rendered.height) * 4;
                if let Some(target) = self.pages.get_mut(page_index) {
                    let was_loaded = target.image.is_some();
                    target.set_rendered_image(render_image(rendered), scale, preview, bytes);
                    if !was_loaded && target.image.is_some() {
                        self.loaded_pages += 1;
                    }
                }
                if self.loaded_pages >= self.page_count {
                    self.busy = false;
                }
                self.evict_distant_pages();
                self.refresh_active_page();
            }
            EditorUpdate::PageText {
                token,
                page_index,
                text,
                fragments,
            } => {
                if token != self.token {
                    return;
                }
                if let Some(target) = self.pages.get_mut(page_index) {
                    target.load_text(text, fragments);
                }
                self.refresh_search(cx, true);
                self.refresh_active_page();
            }
            EditorUpdate::Idle { token } => {
                if token != self.token {
                    return;
                }
                // The viewport is satisfied, so fill in the rest of the document
                // for search, thumbnails and fast jumps.
                self.busy = false;
                self.request_remaining_pages();
            }
            EditorUpdate::Saved { token, path } => {
                self.history.clear();
                self.busy = false;
                window.set_window_edited(false);
                self.path = Some(path.clone());
                window.set_window_title(&file_name(&path));
                // The saved bytes are now the render source, so existing rasters
                // are stale: drop them and redraw from the current viewport
                // without disturbing scroll position, zoom or selection.
                self.token = token;
                self.invalidate_rasters();
                self.flash(format!("Saved {}", file_name(&path)), Severity::Info, cx);
                if self.close_state == CloseState::AfterSave {
                    window.remove_window();
                }
            }
            EditorUpdate::Failed(message) => {
                self.busy = false;
                if self.close_state == CloseState::AfterSave {
                    self.close_state = CloseState::Ready;
                }
                self.flash(message, Severity::Error, cx);
            }
        }
        cx.notify();
    }

    /// Resets every per-document piece of state and starts loading the pages
    /// the reader will see first.
    fn open_document(
        &mut self,
        opened: crate::OpenedDocument,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let crate::OpenedDocument {
            token,
            path,
            document,
            pages,
            forms,
            initial_page,
        } = opened;
        self.token = token;
        if self.close_state == CloseState::AfterSave {
            self.close_state = CloseState::Ready;
        }
        self.background_requested = false;
        self.settled_viewport = None;
        self.path = Some(path.clone());
        self.page_index = initial_page;
        self.page_count = document.page_count;
        self.pdf_version = document.pdf_version;
        self.security = if document.encrypted {
            DocumentSecurity::Encrypted
        } else {
            DocumentSecurity::Unencrypted
        };
        self.pages = pages.into_iter().map(DocumentPage::placeholder).collect();
        self.loaded_pages = 0;
        self.busy = true;
        self.menu_target = None;
        self.severity = Severity::Info;
        self.status = format!("Loading {} pages…", document.page_count).into();
        self.detail = None;
        self.extracted_text = None;
        self.drag = None;
        self.clear_selection();
        self.inline_text = None;
        self.inline_note = None;
        self.search_query.clear();
        self.search_matches.clear();
        self.search_index = 0;
        self.search_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.sync_page_input(window, cx);
        window.set_window_title(&file_name(&path));
        window.set_window_edited(false);
        self.forms = forms
            .into_iter()
            .map(|field| {
                let value = self.pending_form_value(&field);
                let input = form_input(&field, &value, window, cx);
                let field_name = field.name.clone();
                let subscription = cx.subscribe_in(
                    &input,
                    window,
                    move |view, _, event: &gpui_component::input::InputEvent, window, cx| {
                        if matches!(event, gpui_component::input::InputEvent::Blur) {
                            view.validate_form_field(&field_name, window, cx);
                        }
                    },
                );
                FieldInput {
                    input,
                    field,
                    _subscription: subscription,
                }
            })
            .collect();
        self.scroll.scroll_to_top_of_item(initial_page);
        self.refresh_active_page();
        self.request_visible_pages();
    }

    /// Shows a message in the status bar and restores the document summary
    /// afterwards, so feedback is visible but never permanently replaces state.
    pub(super) fn flash(
        &mut self,
        message: impl Into<SharedString>,
        severity: Severity,
        cx: &mut Context<Self>,
    ) {
        self.status = message.into();
        self.severity = severity;
        let delay = if severity == Severity::Error {
            Duration::from_secs(8)
        } else {
            Duration::from_secs(4)
        };
        self.status_reset = Some(cx.spawn(async move |view, cx| {
            cx.background_executor().timer(delay).await;
            let _ = view.update(cx, |view, cx| {
                view.severity = Severity::Info;
                view.refresh_active_page();
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn refresh_active_page(&mut self) {
        let Some(page) = self.pages.get(self.page_index) else {
            return;
        };
        let marker = if self.history.is_empty() {
            String::new()
        } else {
            format!(" · {} unsaved edit(s)", self.history.len())
        };
        let loading = if self.loaded_pages < self.page_count {
            format!(" · loading {}/{}", self.loaded_pages, self.page_count)
        } else {
            String::new()
        };
        let name = self.path.as_deref().map_or_else(|| "PDF".into(), file_name);
        self.status = format!(
            "{name} · page {} of {}{loading}{marker}",
            self.page_index + 1,
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
        self.history
            .iter()
            .rev()
            .find_map(|edit| match edit {
                EditCommand::FillForm { name, value } if name == &field.name => Some(value.clone()),
                _ => None,
            })
            .unwrap_or_else(|| field.value.clone())
    }

    fn validate_form_field(
        &mut self,
        field_name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(item) = self.forms.iter().find(|item| item.field.name == field_name) else {
            return;
        };
        let field = item.field.clone();
        let value = item.value(cx);
        let original = field.value.clone();
        let input = item.input.clone();
        if field.required && value.trim().is_empty() {
            input.update(cx, |state, cx| state.set_value(original, window, cx));
            self.form_alert("Please fill out this field.".to_owned(), window, cx);
            return;
        }
        let Some(validation) = field.validation.clone() else {
            return;
        };
        let result = match &validation {
            FormValidation::Date {
                display_format,
                example,
                reject_future,
                minimum,
                maximum,
                ..
            } => validate_date_value(
                &value,
                display_format,
                example,
                *reject_future,
                minimum,
                maximum,
            ),
            FormValidation::AustrianInsuranceDate => validate_insurance_date(&value),
        };
        match result {
            Ok(normalized) if normalized != value => {
                input.update(cx, |state, cx| state.set_value(normalized, window, cx));
            }
            Ok(_) => {}
            Err(message) => {
                let replacement = if matches!(validation, FormValidation::Date { .. }) {
                    String::new()
                } else {
                    original
                };
                input.update(cx, |state, cx| state.set_value(replacement, window, cx));
                self.form_alert(message, window, cx);
            }
        }
    }

    fn form_alert(&mut self, message: String, window: &mut Window, cx: &mut Context<Self>) {
        use gpui::{PromptButton, PromptLevel};
        let answer = window.prompt(
            PromptLevel::Warning,
            &message,
            None,
            &[PromptButton::ok("OK")],
            cx,
        );
        self.status = message.into();
        self.severity = Severity::Error;
        cx.spawn(async move |_, _| {
            let _ = answer.await;
        })
        .detach();
        cx.notify();
    }

    /// Keeps the page box in sync with the current page unless the user is
    /// typing in it. Called from render, so every path that changes the page
    /// updates the field, including ones without a `Window` at hand.
    pub(super) fn sync_page_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use gpui::Focusable;
        if self.page_input.focus_handle(cx).is_focused(window) {
            return;
        }
        let value = if self.page_count == 0 {
            String::new()
        } else {
            (self.page_index + 1).to_string()
        };
        if self.page_input.read(cx).value() == value.as_str() {
            return;
        }
        self.page_input
            .update(cx, |input, cx| input.set_value(value, window, cx));
    }

    fn commit_page_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let raw = self.page_input.read(cx).value().trim().to_owned();
        match raw.parse::<usize>() {
            Ok(number) if number >= 1 && number <= self.page_count => {
                self.jump_to_page(number - 1, cx);
                self.focus_handle.focus(window);
            }
            _ => {
                self.flash(
                    format!("Enter a page between 1 and {}", self.page_count.max(1)),
                    Severity::Error,
                    cx,
                );
            }
        }
        self.sync_page_input(window, cx);
    }
}

fn validate_date_value(
    value: &str,
    display_format: &str,
    example: &str,
    reject_future: bool,
    minimum: &str,
    maximum: &str,
) -> Result<String, String> {
    use chrono::{Local, NaiveDate};

    if value.is_empty() {
        return Ok(String::new());
    }
    let normalized_input = if value.len() == 9 {
        format!("0{value}")
    } else {
        value.to_owned()
    };
    let date = NaiveDate::parse_from_str(&normalized_input, "%d.%m.%Y").map_err(|_| {
        format!("Bitte geben Sie das Datum im Format {display_format} ein. (Beispiel:{example})")
    })?;
    if reject_future && date > Local::now().date_naive() {
        return Err("Das Datum darf nicht nach dem akutellen Datum liegen!".to_owned());
    }
    let minimum_date = NaiveDate::parse_from_str(minimum, "%d.%m.%Y")
        .expect("PDF date minimum is normalized by adapter");
    if date < minimum_date {
        return Err(format!("Das Datum muss nach {minimum} liegen."));
    }
    let maximum_date = NaiveDate::parse_from_str(maximum, "%d.%m.%Y")
        .expect("PDF date maximum is normalized by adapter");
    if date > maximum_date {
        return Err(format!("Das Datum muss vor {maximum} liegen."));
    }
    Ok(date.format("%d.%m.%Y").to_string())
}

fn validate_insurance_date(value: &str) -> Result<String, String> {
    if value.is_empty() {
        return Ok(String::new());
    }
    if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(
            "Der zweite Teil der Versicherungsnummer muss aus 6 Zeichen bestehen".to_owned(),
        );
    }
    let day: u8 = value[..2].parse().unwrap_or_default();
    if day > 31 {
        return Err("Der Tag darf nicht größer als 31 sein".to_owned());
    }
    let month: u8 = value[2..4].parse().unwrap_or_default();
    if month > 15 {
        return Err("Falsche Monatsangabe!".to_owned());
    }
    Ok(value.to_owned())
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

pub(super) fn inline_text_input(
    placeholder: &str,
    default: &str,
    window: &mut Window,
    cx: &mut Context<EditorView>,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .multi_line(true)
            .placeholder(placeholder.to_owned())
            .default_value(default.to_owned())
    })
}

fn form_input(
    field: &FormField,
    default: &str,
    window: &mut Window,
    cx: &mut Context<EditorView>,
) -> Entity<InputState> {
    let multiline = field.multiline;
    let password = field.password && !multiline;
    let max_len = field.max_len;
    cx.new(|cx| {
        let mut state = InputState::new(window, cx)
            .placeholder(String::new())
            .default_value(default.to_owned())
            .multi_line(multiline);
        if password {
            state = state.masked(true);
        }
        if let Some(max_len) = max_len {
            state = state.validate(move |value, _| value.chars().count() <= max_len);
        }
        state
    })
}

#[cfg(test)]
mod form_validation_tests {
    use super::{validate_date_value, validate_insurance_date};

    #[test]
    fn date_validation_matches_form_messages_and_normalization() {
        let validate = |value| {
            validate_date_value(
                value,
                "TT.MM.JJJJ",
                "11.03.2007",
                true,
                "01.01.1850",
                "31.12.2200",
            )
        };

        assert_eq!(validate("1.02.2020").unwrap(), "01.02.2020");
        assert_eq!(
            validate("not-a-date").unwrap_err(),
            "Bitte geben Sie das Datum im Format TT.MM.JJJJ ein. (Beispiel:11.03.2007)"
        );
        assert_eq!(
            validate("31.12.1849").unwrap_err(),
            "Das Datum muss nach 01.01.1850 liegen."
        );
    }

    #[test]
    fn insurance_validation_matches_form_messages() {
        assert_eq!(validate_insurance_date("010120").unwrap(), "010120");
        assert_eq!(
            validate_insurance_date("123").unwrap_err(),
            "Der zweite Teil der Versicherungsnummer muss aus 6 Zeichen bestehen"
        );
        assert_eq!(
            validate_insurance_date("320120").unwrap_err(),
            "Der Tag darf nicht größer als 31 sein"
        );
        assert_eq!(
            validate_insurance_date("011620").unwrap_err(),
            "Falsche Monatsangabe!"
        );
    }
}
