use std::path::PathBuf;

use pdf_engine::{
    DocumentMetadata, EditCommand, FormField, PageMetadata, RenderedPage, TextFragment,
};

/// What a queued page job should produce.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageKind {
    /// A cheap low-resolution raster, shown until the sharp one arrives.
    Preview,
    /// A raster matching the on-screen size.
    Sharp,
    /// Page text and its geometry, used by search and selection.
    Text,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageRequest {
    pub page_index: usize,
    pub scale: f32,
    pub kind: PageKind,
    /// Lower runs first.
    pub priority: u32,
}

#[derive(Debug)]
pub enum EditorRequest {
    Open(PathBuf),
    /// The pages worth working on right now, ordered by priority. When
    /// `replace` is set, queued rasterisation for other pages is dropped.
    Render {
        replace: bool,
        jobs: Vec<PageRequest>,
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
    PageRendered {
        token: u64,
        page_index: usize,
        scale: f32,
        kind: PageKind,
        rendered: RenderedPage,
    },
    PageText {
        token: u64,
        page_index: usize,
        text: String,
        fragments: Vec<TextFragment>,
    },
    /// Every queued job for this document finished.
    Idle {
        token: u64,
    },
    Saved(PathBuf),
    Failed(String),
}

#[derive(Debug)]
pub struct OpenedDocument {
    /// Identifies the document instance, so results from a previously open
    /// file are ignored instead of painted over the new one.
    pub token: u64,
    pub path: PathBuf,
    pub document: DocumentMetadata,
    pub pages: Vec<PageMetadata>,
    pub forms: Vec<FormField>,
    pub initial_page: usize,
}
