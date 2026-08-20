use std::path::PathBuf;

use pdf_engine::{
    DocumentMetadata, EditCommand, FormField, PageMetadata, RenderedPage, TextFragment,
};

#[derive(Debug)]
pub enum EditorRequest {
    Open(PathBuf),
    /// Re-render a single page at a higher raster scale for crisp zooming.
    RenderPage {
        page_index: usize,
        scale: f32,
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
    Opened(Box<OpenedDocument>),
    PageLoaded(Box<LoadedPage>),
    PageRerendered {
        page_index: usize,
        scale: f32,
        rendered: RenderedPage,
    },
    Saved(PathBuf),
    Failed(String),
}

#[derive(Debug)]
pub struct OpenedDocument {
    pub path: PathBuf,
    pub document: DocumentMetadata,
    pub pages: Vec<PageMetadata>,
    pub forms: Vec<FormField>,
    pub initial_page: usize,
}

#[derive(Debug)]
pub struct LoadedPage {
    pub page: PageMetadata,
    pub rendered: RenderedPage,
    pub text: String,
    pub fragments: Vec<TextFragment>,
}
