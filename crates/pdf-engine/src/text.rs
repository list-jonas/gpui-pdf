use document_core::PdfRect;

#[derive(Clone, Debug, PartialEq)]
pub struct TextFragment {
    pub text: String,
    pub rect: PdfRect,
}
