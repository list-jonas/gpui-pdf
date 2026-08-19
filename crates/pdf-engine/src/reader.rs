use document_core::EngineCapabilities;

use crate::{DocumentMetadata, EngineError, OpenRequest, PageMetadata, PdfRenderer};

pub trait PdfReader {
    fn metadata(&self) -> DocumentMetadata;
    fn page_metadata(&self, page_index: usize) -> Result<PageMetadata, EngineError>;
    fn extract_text(&mut self, page_index: usize) -> Result<String, EngineError>;
}

pub trait PdfDocument: PdfReader + PdfRenderer + Send {}

impl<T> PdfDocument for T where T: PdfReader + PdfRenderer + Send {}

pub trait PdfEngine: Send + Sync {
    fn capabilities(&self) -> EngineCapabilities;
    fn open(&self, request: OpenRequest) -> Result<Box<dyn PdfDocument>, EngineError>;
}
