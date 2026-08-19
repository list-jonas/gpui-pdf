#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormFieldKind {
    Text,
    Button,
    Choice,
    Signature,
    Unknown,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FormField {
    pub name: String,
    pub kind: FormFieldKind,
    pub value: String,
    pub options: Vec<(String, String)>,
    pub read_only: bool,
    pub widgets: Vec<FormWidget>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FormWidget {
    pub page_index: usize,
    pub rect: PdfRect,
}
use document_core::PdfRect;
