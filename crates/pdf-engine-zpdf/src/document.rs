use pdf_engine::{
    DocumentMetadata, EditCommand, EngineError, EngineErrorKind, FormField, FormFieldKind,
    PageMetadata, PdfEditor, PdfReader, PdfRenderer, RenderRequest, RenderedPage,
};
use std::io::Cursor;
use zpdf::{ContentInterpreter, ImageCache, RenderBackend, TextSpan};
use zpdf_writer::{
    FormFiller, IncrementalWriter, RedactOptions, RewriteOptions, StampItem, rewrite_pdf,
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
            .map(|form| form.fields.iter().map(convert_field).collect())
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

fn convert_field(field: &zpdf::FormField) -> FormField {
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
            Some(zpdf::FieldValue::Text(value) | zpdf::FieldValue::Name(value)) => value.clone(),
            Some(zpdf::FieldValue::List(values)) => values.join("\n"),
            None => String::new(),
        },
        options: field.options.clone(),
        read_only: field.flags & FF_READ_ONLY != 0,
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
    }
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
        let page = self.page(page_index)?;
        let mut fonts = self.inner.load_page_fonts(&page);
        let mut images = ImageCache::new();
        let content = self
            .inner
            .page_content_bytes(&page)
            .map_err(|error| map_engine_error(&error))?;
        let mut spans: Vec<TextSpan> = Vec::new();
        ContentInterpreter::new(page.effective_box())
            .with_page_rotation(page.rotate)
            .with_fonts(&mut fonts)
            .with_document(self.inner.file(), &page.resources)
            .with_images(&mut images)
            .with_text_sink(&mut spans)
            .interpret(&content);
        Ok(zpdf::spans_to_text(spans, 2.0))
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
        let display_list = ContentInterpreter::new(page.effective_box())
            .with_page_rotation(page.rotate)
            .with_fonts(&mut fonts)
            .with_document(self.inner.file(), &page.resources)
            .with_images(&mut images)
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
