use pdf_engine::{DocumentMetadata, EngineError, PageMetadata, RenderRequest, RenderedPage};

use crate::Generation;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    Open,
    PageMetadata,
    Render,
    ExtractText,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DocumentCommand {
    PageMetadata {
        page_index: usize,
    },
    Render {
        request: RenderRequest,
        generation: Generation,
    },
    ExtractText {
        page_index: usize,
    },
    Shutdown,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DocumentEvent {
    Opened(DocumentMetadata),
    PageMetadata(PageMetadata),
    PageRendered {
        page_index: usize,
        generation: Generation,
        page: RenderedPage,
    },
    TextReady {
        page_index: usize,
        text: String,
    },
    Failed {
        operation: Operation,
        error: EngineError,
    },
    Closed,
}
