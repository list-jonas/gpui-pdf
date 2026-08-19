use document_core::EngineCapabilities;
use pdf_engine::{EngineError, EngineErrorKind, OpenRequest, PdfDocument, PdfEngine};

use crate::convert::map_open_error;
use crate::document::ZpdfDocument;

#[derive(Clone, Copy, Debug, Default)]
pub struct ZpdfEngine;

impl PdfEngine for ZpdfEngine {
    fn capabilities(&self) -> EngineCapabilities {
        EngineCapabilities::READ
            .union(EngineCapabilities::RENDER)
            .union(EngineCapabilities::EXTRACT_TEXT)
            .union(EngineCapabilities::ENCRYPTED_DOCUMENTS)
    }

    fn open(&self, request: OpenRequest) -> Result<Box<dyn PdfDocument>, EngineError> {
        let password = request
            .password
            .as_ref()
            .map_or(b"".as_slice(), pdf_engine::Password::expose);
        let had_password = request.password.is_some();
        let document = zpdf::PdfDocument::open_with_password(request.bytes, password)
            .map_err(|error| map_open_error(&error, had_password))?;
        if document.is_encrypted() && document.file().decryptor().is_none() {
            let kind = if had_password {
                EngineErrorKind::IncorrectPassword
            } else {
                EngineErrorKind::PasswordRequired
            };
            return Err(EngineError::new(kind, "PDF password is required"));
        }
        Ok(Box::new(ZpdfDocument::new(document)))
    }
}
