use crate::{EngineError, RenderRequest, RenderedPage};

pub trait PdfRenderer {
    fn render_page(&mut self, request: RenderRequest) -> Result<RenderedPage, EngineError>;
}
