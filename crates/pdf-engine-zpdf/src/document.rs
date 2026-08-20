use pdf_engine::{
    DocumentMetadata, EditCommand, EngineError, EngineErrorKind, FormField, FormFieldKind,
    FormWidget, PageMetadata, PdfEditor, PdfReader, PdfRenderer, RenderRequest, RenderedPage,
    ShapeKind, TextFragment,
};
use std::io::Cursor;
use zpdf::{ContentInterpreter, ImageCache, RenderBackend, TextSpan};
use zpdf_writer::{
    AnnotationSpec, FormFiller, IncrementalWriter, MarkupKind, RedactOptions, RewriteOptions,
    StampItem, rewrite_pdf,
};

use crate::convert::{map_engine_error, map_render_error, page_geometry};

const FF_READ_ONLY: i64 = 1;

pub struct ZpdfDocument {
    inner: zpdf::PdfDocument,
    source: Vec<u8>,
    password: Vec<u8>,
}

impl ZpdfDocument {
    pub const fn new(inner: zpdf::PdfDocument, source: Vec<u8>, password: Vec<u8>) -> Self {
        Self {
            inner,
            source,
            password,
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
                Some(zpdf::FieldValue::List(values)) => values.join("\n"),
                None => String::new(),
            },
            options: field.options.clone(),
            read_only: field.flags & FF_READ_ONLY != 0,
            widgets: self.field_widgets(field),
        }
    }

    fn field_widgets(&self, field: &zpdf::FormField) -> Vec<FormWidget> {
        let mut widgets = Vec::new();
        for page_index in 0..self.inner.page_count() {
            let Ok(page) = self.inner.page(page_index) else {
                continue;
            };
            for (id, annotation) in page.annots.iter().zip(self.inner.page_annotations(&page)) {
                if field.widgets.contains(id)
                    && let Ok(rect) = document_core::PdfRect::new(
                        annotation.rect.x0,
                        annotation.rect.y0,
                        annotation.rect.x1,
                        annotation.rect.y1,
                    )
                {
                    widgets.push(FormWidget { page_index, rect });
                }
            }
        }
        widgets
    }
}

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
        Ok(zpdf::spans_to_text(self.extract_spans(page_index)?, 2.0))
    }

    fn text_fragments(&mut self, page_index: usize) -> Result<Vec<TextFragment>, EngineError> {
        Ok(self
            .extract_spans(page_index)?
            .into_iter()
            .filter(|span| !span.text.trim().is_empty())
            .filter_map(|span| {
                let x0 = span.x.min(span.x + span.advance);
                let x1 = span.x.max(span.x + span.advance);
                let size = f64::from(span.size).abs().max(1.0);
                document_core::PdfRect::new(x0, span.y - size * 0.25, x1, span.y + size * 0.8)
                    .ok()
                    .map(|rect| TextFragment {
                        text: span.text,
                        rect,
                    })
            })
            .collect())
    }
}

impl ZpdfDocument {
    fn extract_spans(&mut self, page_index: usize) -> Result<Vec<TextSpan>, EngineError> {
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
        let page = self.page(request.page_index)?;
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
        let rendered = zpdf::cpu::CpuRenderer::new()
            .with_limits(self.inner.file().limits())
            .with_fonts(&fonts)
            .with_images(&images)
            .render_display_list(&display_list, request.scale)
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
