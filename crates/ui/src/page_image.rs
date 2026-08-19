use std::sync::Arc;

use gpui::RenderImage;
use pdf_engine::RenderedPage;

pub fn render_image(mut page: RenderedPage) -> Option<Arc<RenderImage>> {
    for pixel in page.rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let image = image::RgbaImage::from_raw(page.width, page.height, page.rgba)?;
    Some(Arc::new(RenderImage::new([image::Frame::new(image)])))
}
