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
    /// True while the visible image is a cheap low-resolution stand-in.
    pub preview: bool,
    /// Set once page text has been extracted, so it is requested only once.
    pub text_loaded: bool,
    /// Set while text extraction is queued for this page.
    pub text_requested: bool,
    /// Approximate bytes held by the current raster, for the memory budget.
    pub image_bytes: u64,
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
            render_scale: 0.0,
            requested_scale: 0.0,
            preview: true,
            text_loaded: false,
            text_requested: false,
            image_bytes: 0,
        }
    }

    pub fn load_text(&mut self, text: String, fragments: Vec<TextFragment>) {
        self.text = text.into();
        self.fragments = fragments;
        self.text_loaded = true;
        self.text_requested = false;
    }

    /// Swaps in a new raster without disturbing layout or text geometry. A
    /// preview never replaces a sharper image that already arrived.
    pub fn set_rendered_image(
        &mut self,
        image: Option<Arc<RenderImage>>,
        scale: f32,
        preview: bool,
        bytes: u64,
    ) {
        if image.is_none() {
            return;
        }
        if self.image.is_some() && scale < self.render_scale {
            return;
        }
        self.image = image;
        self.render_scale = scale;
        self.preview = preview;
        self.image_bytes = bytes;
    }

    /// Frees the raster of a page far from the viewport. Layout is untouched,
    /// so nothing moves when the page is rendered again on the way back.
    pub fn release_image(&mut self) {
        self.image = None;
        self.image_bytes = 0;
        self.render_scale = 0.0;
        self.requested_scale = 0.0;
        self.preview = true;
    }
}
