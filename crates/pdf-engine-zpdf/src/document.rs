use pdf_engine::{
    DocumentMetadata, EditCommand, EngineError, EngineErrorKind, FormAction, FormButtonKind,
    FormField, FormFieldKind, FormValidation, FormWidget, PageMetadata, PdfEditor, PdfReader,
    PdfRenderer, RenderRequest, RenderedPage, ShapeKind, TextFragment,
};
use std::io::Cursor;
use zpdf::{ContentInterpreter, ImageCache, RenderBackend, TextSpan};
use zpdf_writer::{
    AnnotationSpec, FormFiller, IncrementalWriter, MarkupKind, RedactOptions, RewriteOptions,
    StampItem, rewrite_pdf,
};

use crate::cache::{PageCache, RenderedContent};
use crate::convert::{map_engine_error, map_render_error, page_geometry};

const FF_READ_ONLY: i64 = 1;
const FF_REQUIRED: i64 = 1 << 1;
const FF_MULTILINE: i64 = 1 << 12;
const FF_RADIO: i64 = 1 << 15;
const FF_PUSH_BUTTON: i64 = 1 << 16;
const FF_MULTI_SELECT: i64 = 1 << 21;

pub struct ZpdfDocument {
    inner: zpdf::PdfDocument,
    source: Vec<u8>,
    password: Vec<u8>,
    cache: PageCache,
}

impl ZpdfDocument {
    pub const fn new(inner: zpdf::PdfDocument, source: Vec<u8>, password: Vec<u8>) -> Self {
        Self {
            inner,
            source,
            password,
            cache: PageCache::new(),
        }
    }

    fn page(&self, index: usize) -> Result<zpdf::PdfPage, EngineError> {
        if index >= self.inner.page_count() {
            return Err(EngineError::new(
                EngineErrorKind::InvalidDocument,
                format!("page index {index} is out of bounds"),
            ));
        }
        self.inner
            .page(index)
            .map_err(|error| map_engine_error(&error))
    }
}

impl Drop for ZpdfDocument {
    fn drop(&mut self) {
        self.password.fill(0);
    }
}

impl PdfEditor for ZpdfDocument {
    fn form_fields(&self) -> Result<Vec<FormField>, EngineError> {
        Ok(self
            .inner
            .acro_form()
            .map(|form| {
                form.fields
                    .iter()
                    .map(|field| self.convert_field(field))
                    .collect()
            })
            .unwrap_or_default())
    }

    fn export(&mut self, edits: &[EditCommand]) -> Result<Vec<u8>, EngineError> {
        self.cache.clear();
        let has_redactions = edits
            .iter()
            .any(|edit| matches!(edit, EditCommand::Redact { .. }));
        if has_redactions && self.inner.is_encrypted() {
            return Err(EngineError::new(
                EngineErrorKind::Unsupported,
                "redacting encrypted PDFs is not supported",
            ));
        }
        let mut writer = if self.password.is_empty() {
            IncrementalWriter::new(self.source.clone())
        } else {
            IncrementalWriter::new_with_password(self.source.clone(), &self.password)
        }
        .map_err(map_write_error)?;

        for edit in edits {
            apply_edit(&mut writer, edit)?;
        }
        let mut output = Cursor::new(Vec::new());
        writer.write(&mut output).map_err(map_write_error)?;
        let output = output.into_inner();
        if !has_redactions {
            return Ok(output);
        }
        let redacted = zpdf::PdfDocument::open(output).map_err(map_write_error)?;
        rewrite_pdf(redacted.file(), &RewriteOptions::default()).map_err(map_write_error)
    }
}

impl ZpdfDocument {
    fn convert_field(&self, field: &zpdf::FormField) -> FormField {
        let kind = match field.kind {
            zpdf::FieldKind::Text => FormFieldKind::Text,
            zpdf::FieldKind::Button => FormFieldKind::Button,
            zpdf::FieldKind::Choice => FormFieldKind::Choice,
            zpdf::FieldKind::Signature => FormFieldKind::Signature,
            zpdf::FieldKind::Unknown => FormFieldKind::Unknown,
        };
        FormField {
            name: field.name.clone(),
            kind,
            value: match &field.value {
                Some(zpdf::FieldValue::Text(value) | zpdf::FieldValue::Name(value)) => {
                    value.clone()
                }
                Some(zpdf::FieldValue::List(values)) => values.join("\u{1f}"),
                None => String::new(),
            },
            options: field.options.clone(),
            max_len: field.max_len.and_then(|len| usize::try_from(len).ok()),
            multiline: field.flags & FF_MULTILINE != 0,
            password: field.is_password(),
            comb: field.is_comb(),
            multi_select: field.kind == zpdf::FieldKind::Choice
                && field.flags & FF_MULTI_SELECT != 0,
            required: field.flags & FF_REQUIRED != 0,
            read_only: field.flags & FF_READ_ONLY != 0,
            button_kind: (kind == FormFieldKind::Button).then_some({
                if field.flags & FF_PUSH_BUTTON != 0 {
                    FormButtonKind::Push
                } else if field.flags & FF_RADIO != 0 {
                    FormButtonKind::Radio
                } else {
                    FormButtonKind::CheckBox
                }
            }),
            validation: self.field_validation(field),
            widgets: self.field_widgets(field),
        }
    }

    fn field_widgets(&self, field: &zpdf::FormField) -> Vec<FormWidget> {
        let mut widgets = Vec::new();
        let field_hint = self
            .object_dictionary(field.field_id)
            .and_then(|dictionary| dictionary.get("TU").cloned())
            .and_then(|object| resolve_object(self.inner.file(), &object))
            .and_then(|object| pdf_string(&object));
        for page_index in 0..self.inner.page_count() {
            let Ok(page) = self.inner.page(page_index) else {
                continue;
            };
            for id in page.annots.iter().filter(|id| field.widgets.contains(id)) {
                let Some(dictionary) = self.object_dictionary(*id) else {
                    continue;
                };
                if !widget_is_visible(&dictionary) {
                    continue;
                }
                let Ok(raw_rect) = dictionary.get_rect("Rect") else {
                    continue;
                };
                let Ok(rect) =
                    document_core::PdfRect::new(raw_rect.x0, raw_rect.y0, raw_rect.x1, raw_rect.y1)
                else {
                    continue;
                };
                widgets.push(FormWidget {
                    page_index,
                    rect,
                    visible: true,
                    hint: dictionary
                        .get("TU")
                        .cloned()
                        .and_then(|object| resolve_object(self.inner.file(), &object))
                        .and_then(|object| pdf_string(&object))
                        .or_else(|| field_hint.clone()),
                    on_value: (field.kind == zpdf::FieldKind::Button)
                        .then(|| button_on_value(self.inner.file(), &dictionary))
                        .flatten(),
                    action: (field.kind == zpdf::FieldKind::Button)
                        .then(|| button_action(self.inner.file(), &dictionary))
                        .flatten()
                        .or_else(|| {
                            self.object_dictionary(field.field_id)
                                .and_then(|field| button_action(self.inner.file(), &field))
                        }),
                });
            }
        }
        widgets
    }

    fn field_validation(&self, field: &zpdf::FormField) -> Option<FormValidation> {
        let dictionary = self.object_dictionary(field.field_id)?;
        let script = additional_action_script(self.inner.file(), &dictionary, "V")?;
        if script.contains("std_format_date") && script.contains("dd.mm.yyyy") {
            return Some(FormValidation::Date {
                format: "dd.mm.yyyy".to_owned(),
                display_format: "TT.MM.JJJJ".to_owned(),
                example: "11.03.2007".to_owned(),
                reject_future: script.contains(",\"true\"") || script.contains(", \"true\""),
                minimum: "01.01.1850".to_owned(),
                maximum: "31.12.2200".to_owned(),
            });
        }
        script
            .contains("std_enCheck")
            .then_some(FormValidation::AustrianInsuranceDate)
    }

    fn object_dictionary(&self, id: zpdf::ObjectId) -> Option<zpdf::PdfDict> {
        self.inner.file().resolve(id).ok()?.as_dict().ok().cloned()
    }
}

fn resolve_object(file: &zpdf::PdfFile, object: &zpdf::PdfObject) -> Option<zpdf::PdfObject> {
    match object {
        zpdf::PdfObject::Ref(id) => file.resolve(*id).ok(),
        direct => Some(direct.clone()),
    }
}

fn resolve_dictionary(file: &zpdf::PdfFile, object: &zpdf::PdfObject) -> Option<zpdf::PdfDict> {
    resolve_object(file, object)?.as_dict().ok().cloned()
}

fn pdf_string(object: &zpdf::PdfObject) -> Option<String> {
    let bytes = object.as_str().ok()?.as_bytes();
    if let Some(body) = bytes.strip_prefix(&[0xfe, 0xff]) {
        let (pairs, _) = body.as_chunks::<2>();
        let units = pairs.iter().map(|pair| u16::from_be_bytes(*pair));
        return Some(
            char::decode_utf16(units)
                .map(|c| c.unwrap_or('\u{fffd}'))
                .collect(),
        );
    }
    // PDF text strings without a UTF-16 BOM use PDFDocEncoding. Its common
    // Latin range is byte-compatible with Unicode; mapping bytes directly
    // preserves legacy form hints such as `0xfc` (`ü`) that UTF-8 decoding
    // would replace with U+FFFD.
    Some(bytes.iter().map(|&byte| char::from(byte)).collect())
}

fn action_script(file: &zpdf::PdfFile, object: &zpdf::PdfObject) -> Option<String> {
    let action = resolve_dictionary(file, object)?;
    (action.get_name("S").ok()? == "JavaScript")
        .then(|| action.get("JS"))
        .flatten()
        .and_then(|script| resolve_object(file, script))
        .as_ref()
        .and_then(pdf_string)
}

fn additional_action_script(
    file: &zpdf::PdfFile,
    dictionary: &zpdf::PdfDict,
    event: &str,
) -> Option<String> {
    let actions = resolve_dictionary(file, dictionary.get("AA")?)?;
    action_script(file, actions.get(event)?)
}

fn button_on_value(file: &zpdf::PdfFile, widget: &zpdf::PdfDict) -> Option<String> {
    let appearances = widget
        .get("AP")
        .and_then(|object| resolve_dictionary(file, object));
    let normal = appearances
        .as_ref()
        .and_then(|appearances| appearances.get("N"))
        .and_then(|object| resolve_dictionary(file, object));
    normal
        .as_ref()
        .and_then(|normal| {
            normal
                .0
                .keys()
                .find(|name| name.as_str() != "Off")
                .map(|name| name.as_str().to_owned())
        })
        .or_else(|| {
            widget
                .get_name("AS")
                .ok()
                .filter(|name| *name != "Off")
                .map(std::borrow::ToOwned::to_owned)
        })
}

fn widget_is_visible(widget: &zpdf::PdfDict) -> bool {
    widget.get_i64("F").unwrap_or(0) & (1 | 2 | 32) == 0
}

fn button_action(file: &zpdf::PdfFile, dictionary: &zpdf::PdfDict) -> Option<FormAction> {
    let action_object = dictionary
        .get("AA")
        .and_then(|additional| resolve_dictionary(file, additional))
        .and_then(|actions| actions.get("U").or_else(|| actions.get("D")).cloned())
        .or_else(|| dictionary.get("A").cloned())?;
    let action = resolve_dictionary(file, &action_object)?;
    if action.get_name("S").ok() == Some("ResetForm") {
        return Some(FormAction::ResetForm);
    }
    let script = action_script(file, &action_object)?;
    parse_javascript_button_action(&script)
}

fn parse_javascript_button_action(script: &str) -> Option<FormAction> {
    if let Some(url) = quoted_after(script, "app.launchURL(") {
        return Some(FormAction::OpenUrl { url });
    }

    if script.contains("util.printd") && script.contains("new Date") {
        let field_name = quoted_after(script, "getField(")?;
        let format = quoted_after(script, "util.printd(")?;
        return Some(FormAction::SetToday { field_name, format });
    }

    if !script.contains("==\"Off\"") && !script.contains("== \"Off\"") {
        return None;
    }
    let else_index = script.find("else")?;
    let (off_branch, checked_branch) = script.split_at(else_index);
    let field_name = quoted_after(script, "gf(")?;
    let when_unchecked = assigned_value(off_branch);
    let when_checked = assigned_value(checked_branch);
    (when_checked.is_some() || when_unchecked.is_some()).then_some(FormAction::SetButtonValue {
        field_name,
        when_checked,
        when_unchecked,
    })
}

fn quoted_after(text: &str, marker: &str) -> Option<String> {
    let remainder = text.get(text.find(marker)? + marker.len()..)?.trim_start();
    let quote = remainder.chars().next()?;
    if !matches!(quote, '\'' | '"') {
        return None;
    }
    let body = remainder.get(quote.len_utf8()..)?;
    Some(body.get(..body.find(quote)?)?.to_owned())
}

fn assigned_value(branch: &str) -> Option<String> {
    quoted_after(branch, ".value=").or_else(|| quoted_after(branch, ".value ="))
}

#[allow(clippy::too_many_lines)]
fn apply_edit(writer: &mut IncrementalWriter, edit: &EditCommand) -> Result<(), EngineError> {
    match edit {
        EditCommand::FillForm { name, value } => {
            let field = writer
                .document()
                .acro_form()
                .and_then(|form| form.fields.iter().find(|field| field.name == *name))
                .cloned();
            let mut filler = FormFiller::new(writer).map_err(map_write_error)?;
            filler.set(name, value).map_err(map_write_error)?;
            filler.finish().map_err(map_write_error)?;
            if let Some(field) = field.filter(|field| {
                matches!(field.kind, zpdf::FieldKind::Text | zpdf::FieldKind::Choice)
            }) {
                let mut dictionary = writer
                    .resolve_current(field.field_id)
                    .map_err(map_write_error)?
                    .as_dict()
                    .map_err(map_write_error)?
                    .clone();
                dictionary.insert(
                    zpdf::PdfName::new("V"),
                    zpdf::PdfObject::String(zpdf::PdfString(encode_pdf_string(value))),
                );
                writer.overwrite_object(field.field_id, zpdf::PdfObject::Dict(dictionary));
            }
            Ok(())
        }
        EditCommand::AddText(stamp) => writer
            .stamp_page(
                stamp.page_index,
                &[StampItem::Text {
                    text: stamp.text.clone(),
                    x: stamp.x,
                    y: stamp.y,
                    font: "Helvetica".to_owned(),
                    size: stamp.size,
                    color: (0.0, 0.0, 0.0),
                }],
            )
            .map_err(map_write_error),
        EditCommand::Redact { page_index, rect } => writer
            .redact_page(
                *page_index,
                &[zpdf::Rect::new(
                    rect.x_min, rect.y_min, rect.x_max, rect.y_max,
                )],
                &RedactOptions::default(),
            )
            .map_err(map_write_error),
        EditCommand::Highlight {
            page_index,
            rects,
            color,
        } => {
            let rects: Vec<_> = rects
                .iter()
                .map(|rect| zpdf::Rect::new(rect.x_min, rect.y_min, rect.x_max, rect.y_max))
                .collect();
            writer
                .add_annotation(
                    *page_index,
                    &AnnotationSpec::markup_from_rects(MarkupKind::Highlight, &rects, *color, None),
                )
                .map(|_| ())
                .map_err(map_write_error)
        }
        EditCommand::Underline {
            page_index,
            rects,
            color,
        } => add_markup(writer, *page_index, rects, MarkupKind::Underline, *color),
        EditCommand::StrikeOut {
            page_index,
            rects,
            color,
        } => add_markup(writer, *page_index, rects, MarkupKind::StrikeOut, *color),
        EditCommand::Note {
            page_index,
            x,
            y,
            contents,
            color,
        } => writer
            .add_annotation(
                *page_index,
                &AnnotationSpec::Note {
                    x: *x,
                    y: *y,
                    contents: contents.clone(),
                    color: Some(*color),
                    icon: Some("Comment".to_owned()),
                },
            )
            .map(|_| ())
            .map_err(map_write_error),
        EditCommand::Shape {
            page_index,
            kind,
            rect,
            color,
            width,
        } => {
            let rect = zpdf::Rect::new(rect.x_min, rect.y_min, rect.x_max, rect.y_max);
            let spec = match kind {
                ShapeKind::Rectangle => AnnotationSpec::Square {
                    rect,
                    color: *color,
                    interior: None,
                    width: *width,
                },
                ShapeKind::Ellipse => AnnotationSpec::Circle {
                    rect,
                    color: *color,
                    interior: None,
                    width: *width,
                },
            };
            writer
                .add_annotation(*page_index, &spec)
                .map(|_| ())
                .map_err(map_write_error)
        }
    }
}

fn add_markup(
    writer: &mut IncrementalWriter,
    page_index: usize,
    rects: &[document_core::PdfRect],
    kind: MarkupKind,
    color: (f64, f64, f64),
) -> Result<(), EngineError> {
    let rects: Vec<_> = rects
        .iter()
        .map(|rect| zpdf::Rect::new(rect.x_min, rect.y_min, rect.x_max, rect.y_max))
        .collect();
    writer
        .add_annotation(
            page_index,
            &AnnotationSpec::markup_from_rects(kind, &rects, color, None),
        )
        .map(|_| ())
        .map_err(map_write_error)
}

fn encode_pdf_string(value: &str) -> Vec<u8> {
    if value.is_ascii() {
        return value.as_bytes().to_vec();
    }
    let mut encoded = vec![0xfe, 0xff];
    encoded.extend(value.encode_utf16().flat_map(u16::to_be_bytes));
    encoded
}

fn map_write_error(error: impl std::fmt::Display) -> EngineError {
    EngineError::new(EngineErrorKind::Internal, error.to_string())
}

impl PdfReader for ZpdfDocument {
    fn metadata(&self) -> DocumentMetadata {
        DocumentMetadata {
            page_count: self.inner.page_count(),
            pdf_version: self.inner.version(),
            encrypted: self.inner.is_encrypted(),
        }
    }

    fn page_metadata(&self, page_index: usize) -> Result<PageMetadata, EngineError> {
        let page = self.page(page_index)?;
        Ok(PageMetadata {
            index: page_index,
            geometry: page_geometry(&page)?,
        })
    }

    fn extract_text(&mut self, page_index: usize) -> Result<String, EngineError> {
        let spans = self.page_spans(page_index)?.to_vec();
        Ok(zpdf::spans_to_text(spans, 2.0))
    }

    fn text_fragments(&mut self, page_index: usize) -> Result<Vec<TextFragment>, EngineError> {
        Ok(self
            .page_spans(page_index)?
            .iter()
            .flat_map(character_fragments)
            .collect())
    }
}

/// Turns one PDF show-text operation into independently selectable characters.
/// zpdf exposes a single advance for the operation rather than glyph advances,
/// so each character receives an equal share of that advance. This keeps the
/// hit-test and annotation geometry precise enough for character and word
/// selection without discarding whitespace inside a text operation.
fn character_fragments(span: &TextSpan) -> Vec<TextFragment> {
    let character_count = span.text.chars().count();
    let Ok(character_count) = u32::try_from(character_count) else {
        return Vec::new();
    };
    if character_count == 0 {
        return Vec::new();
    }

    let count = f64::from(character_count);
    let size = f64::from(span.size).abs().max(1.0);
    span.text
        .chars()
        .enumerate()
        .filter_map(|(index, character)| {
            let index = u32::try_from(index).ok()?;
            let start = f64::from(index) / count;
            let end = f64::from(index.saturating_add(1)) / count;
            let x0 = span.x + span.advance * start;
            let x1 = span.x + span.advance * end;
            document_core::PdfRect::new(
                x0.min(x1),
                span.y - size * 0.25,
                x0.max(x1),
                span.y + size * 0.8,
            )
            .ok()
            .map(|rect| TextFragment {
                text: character.to_string(),
                rect,
            })
        })
        .collect()
}

impl ZpdfDocument {
    /// Text spans for a page, interpreted once and reused by both the plain-text
    /// and geometry-carrying extractors.
    fn page_spans(&mut self, page_index: usize) -> Result<&[TextSpan], EngineError> {
        if self.cache.spans(page_index).is_some() {
            return Ok(self.cache.spans(page_index).expect("cached above"));
        }
        let spans = self.interpret_spans(page_index)?;
        Ok(self.cache.insert_spans(page_index, spans))
    }

    fn interpret_spans(&mut self, page_index: usize) -> Result<Vec<TextSpan>, EngineError> {
        let page = self.page(page_index)?;
        let mut fonts = self.inner.load_page_fonts(&page);
        let mut images = ImageCache::new();
        let content = self
            .inner
            .page_content_bytes(&page)
            .map_err(|error| map_engine_error(&error))?;
        let mut spans: Vec<TextSpan> = Vec::new();
        ContentInterpreter::new(page.effective_box())
            .with_fonts(&mut fonts)
            .with_document(self.inner.file(), &page.resources)
            .with_images(&mut images)
            .with_text_sink(&mut spans)
            .interpret(&content);
        Ok(spans)
    }
}

impl PdfRenderer for ZpdfDocument {
    fn render_page(&mut self, request: RenderRequest) -> Result<RenderedPage, EngineError> {
        if !request.scale.is_finite() || request.scale <= 0.0 {
            return Err(EngineError::new(
                EngineErrorKind::Rendering,
                "render scale must be finite and positive",
            ));
        }
        let limits = self.inner.file().limits().clone();
        let content = self.page_content(request.page_index)?;
        let rendered = zpdf::cpu::CpuRenderer::new()
            .with_limits(&limits)
            .with_fonts(&content.fonts)
            .with_images(&content.images)
            .render_display_list(&content.display_list, request.scale)
            .map_err(|error| map_render_error(&error))?;
        let page = RenderedPage::new(rendered.width, rendered.height, rendered.data);
        if !page.is_valid() {
            return Err(EngineError::new(
                EngineErrorKind::Rendering,
                "renderer returned an invalid RGBA buffer",
            ));
        }
        Ok(page)
    }
}

impl ZpdfDocument {
    /// Interpreted page content, cached so re-rendering at a new scale only
    /// re-rasterises instead of reparsing fonts, images and operators.
    fn page_content(&mut self, page_index: usize) -> Result<&RenderedContent, EngineError> {
        if self.cache.content(page_index).is_some() {
            return Ok(self.cache.content(page_index).expect("cached above"));
        }
        let content = self.interpret_content(page_index)?;
        Ok(self.cache.insert_content(page_index, content))
    }

    fn interpret_content(&mut self, page_index: usize) -> Result<RenderedContent, EngineError> {
        let page = self.page(page_index)?;
        let mut fonts = self.inner.load_page_fonts(&page);
        let mut images = ImageCache::new();
        let content = self
            .inner
            .page_content_bytes(&page)
            .map_err(|error| map_engine_error(&error))?;
        let annotations = self.inner.page_annotations(&page);
        let display_list = ContentInterpreter::new(page.effective_box())
            .with_page_rotation(page.rotate)
            .with_fonts(&mut fonts)
            .with_document(self.inner.file(), &page.resources)
            .with_images(&mut images)
            .with_annotations(&annotations)
            .interpret(&content);
        Ok(RenderedContent {
            fonts,
            images,
            display_list,
        })
    }
}
