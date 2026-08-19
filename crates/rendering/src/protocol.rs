use pdf_engine::{
    DocumentMetadata, EditCommand, EngineError, FormField, PageMetadata, RenderRequest,
    RenderedPage,
};

use crate::Generation;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    Open,
    PageMetadata,
    Render,
    ExtractText,
    Forms,
    Export,
}

#[derive(Clone, Debug, PartialEq)]
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
    FormFields,
    Export {
        edits: Vec<EditCommand>,
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
    FormFields(Vec<FormField>),
    Exported(Vec<u8>),
    Failed {
        operation: Operation,
        error: EngineError,
    },
    Closed,
}
