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
}
