use std::sync::Arc;

use gpui::RenderImage;
use pdf_engine::RenderedPage;

pub fn render_image(mut page: RenderedPage) -> Option<Arc<RenderImage>> {
    for pixel in page.rgba.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    let image = image::RgbaImage::from_raw(page.width, page.height, page.rgba)?;
    Some(Arc::new(RenderImage::new([image::Frame::new(image)])))
}
