use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{Bounds, Pixels, RenderImage, SharedString};
use pdf_engine::{PageMetadata, TextFragment};

use super::geometry::page_raster_size;

pub struct DocumentPage {
    pub metadata: PageMetadata,
    pub image: Option<Arc<RenderImage>>,
    pub image_size: (f32, f32),
    pub text: SharedString,
    pub fragments: Vec<TextFragment>,
    pub bounds: Rc<Cell<Bounds<Pixels>>>,
    /// Raster scale of the current image, relative to PDF points.
    pub render_scale: f32,
    /// Scale already requested, so identical requests are not repeated.
    pub requested_scale: f32,
}

impl DocumentPage {
    pub fn placeholder(metadata: PageMetadata) -> Self {
        Self {
            metadata,
            image: None,
            image_size: page_raster_size(metadata.geometry),
            text: SharedString::default(),
            fragments: Vec::new(),
            bounds: Rc::new(Cell::new(Bounds::default())),
            render_scale: super::geometry::ui_f32(super::geometry::RENDER_SCALE),
            requested_scale: super::geometry::ui_f32(super::geometry::RENDER_SCALE),
        }
    }

    pub fn load(
        &mut self,
        image: Option<Arc<RenderImage>>,
        image_size: (u32, u32),
        text: String,
        fragments: Vec<TextFragment>,
    ) {
        self.image = image;
        self.image_size = (
            super::geometry::raster_f32(image_size.0),
            super::geometry::raster_f32(image_size.1),
        );
        self.text = text.into();
        self.fragments = fragments;
    }

    /// Swaps in a sharper raster without disturbing layout or text geometry.
    pub fn set_rendered_image(&mut self, image: Option<Arc<RenderImage>>, scale: f32) {
        if image.is_some() {
            self.image = image;
            self.render_scale = scale;
        }
    }
}
