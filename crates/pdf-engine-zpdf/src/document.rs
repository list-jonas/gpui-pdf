use pdf_engine::{
    DocumentMetadata, EngineError, EngineErrorKind, PageMetadata, PdfReader, PdfRenderer,
    RenderRequest, RenderedPage,
};
use zpdf::{ContentInterpreter, ImageCache, RenderBackend, TextSpan};

use crate::convert::{map_engine_error, map_render_error, page_geometry};

pub struct ZpdfDocument {
    inner: zpdf::PdfDocument,
}

impl ZpdfDocument {
    pub const fn new(inner: zpdf::PdfDocument) -> Self {
        Self { inner }
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
