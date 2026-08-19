use document_core::{PageGeometry, PdfRect, Rotation};
use pdf_engine::{EngineError, EngineErrorKind};

pub fn map_open_error(error: &zpdf::Error, had_password: bool) -> EngineError {
    let kind = match error {
        zpdf::Error::WrongPassword if had_password => EngineErrorKind::IncorrectPassword,
        zpdf::Error::WrongPassword => EngineErrorKind::PasswordRequired,
        zpdf::Error::UnsupportedVersion(_, _) => EngineErrorKind::Unsupported,
        zpdf::Error::RecursionLimit(_)
        | zpdf::Error::StreamSizeLimit(_)
        | zpdf::Error::StringLengthLimit(_) => EngineErrorKind::ResourceLimit,
        _ => EngineErrorKind::InvalidDocument,
    };
    EngineError::new(kind, error.to_string())
}

pub fn map_engine_error(error: &impl ToString) -> EngineError {
    EngineError::new(EngineErrorKind::Internal, error.to_string())
}

pub fn map_render_error(error: &impl ToString) -> EngineError {
    EngineError::new(EngineErrorKind::Rendering, error.to_string())
}

pub fn page_geometry(page: &zpdf::PdfPage) -> Result<PageGeometry, EngineError> {
    let media = PdfRect::new(
        page.media_box.x0,
        page.media_box.y0,
        page.media_box.x1,
        page.media_box.y1,
    )
    .map_err(|error| map_engine_error(&error))?;
    let crop = PdfRect::new(
        page.crop_box.x0,
        page.crop_box.y0,
        page.crop_box.x1,
        page.crop_box.y1,
    )
    .unwrap_or(media);
    let rotation = Rotation::from_degrees(page.rotate).map_err(|error| map_engine_error(&error))?;
    PageGeometry::new(media, crop, rotation, 1.0).map_err(|error| map_engine_error(&error))
}
