use document_core::PdfRect;

use crate::{EngineError, FormField};

#[derive(Clone, Debug, PartialEq)]
pub struct TextStamp {
    pub page_index: usize,
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub size: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EditCommand {
    FillForm {
        name: String,
        value: String,
    },
    AddText(TextStamp),
    Redact {
        page_index: usize,
        rect: PdfRect,
    },
    Highlight {
        page_index: usize,
        rects: Vec<PdfRect>,
        color: (f64, f64, f64),
    },
}

pub trait PdfEditor {
    fn form_fields(&self) -> Result<Vec<FormField>, EngineError>;
    fn export(&mut self, edits: &[EditCommand]) -> Result<Vec<u8>, EngineError>;
}
