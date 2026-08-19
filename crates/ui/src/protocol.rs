use std::path::PathBuf;

use pdf_engine::{DocumentMetadata, EditCommand, FormField, PageMetadata, RenderedPage};

#[derive(Debug)]
pub enum EditorRequest {
    Open(PathBuf),
    LoadPage {
        path: PathBuf,
        page_index: usize,
    },
    SaveAs {
        source: PathBuf,
        destination: PathBuf,
        page_index: usize,
        edits: Vec<EditCommand>,
    },
}

#[derive(Debug)]
pub enum EditorUpdate {
    Loaded {
        path: PathBuf,
        document: DocumentMetadata,
        page: PageMetadata,
        rendered: RenderedPage,
        text: String,
        forms: Vec<FormField>,
    },
    Saved(PathBuf),
    Failed(String),
}
