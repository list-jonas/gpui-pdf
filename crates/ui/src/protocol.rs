use std::path::PathBuf;

use pdf_engine::{
    DocumentMetadata, EditCommand, FormField, PageMetadata, RenderedPage, TextFragment,
};

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
    Loaded(Box<LoadedDocument>),
    Saved(PathBuf),
    Failed(String),
}

#[derive(Debug)]
pub struct LoadedDocument {
    pub path: PathBuf,
    pub document: DocumentMetadata,
    pub page: PageMetadata,
    pub rendered: RenderedPage,
    pub text: String,
    pub fragments: Vec<TextFragment>,
    pub forms: Vec<FormField>,
}
