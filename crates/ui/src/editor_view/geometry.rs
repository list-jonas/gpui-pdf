use document_core::{PageGeometry, PdfPoint, PdfRect, ViewportPoint, ViewportTransform};
use gpui::{Bounds, Pixels, Point};

use super::model::OverlayRect;

pub const RENDER_SCALE: f64 = 1.5;

pub fn page_raster_size(geometry: PageGeometry) -> (f32, f32) {
    let (width, height) = transform(geometry, 1.0).viewport_size();
    (ui_f32(width), ui_f32(height))
}

/// Scale a delivered raster was really painted at, derived from its own pixel
/// size instead of the request. Render backends may clamp very large rasters to
/// a pixel budget, so the bitmap that arrives can cover less than the scale
/// that was asked for. Painting it at the requested scale would stretch it and
/// cut off whatever lands outside the page box, which looks like disappearing
/// content after zooming in.
#[must_use]
pub fn delivered_render_scale(
    geometry: PageGeometry,
    requested_scale: f32,
    delivered_width: u32,
) -> f32 {
    let base_width = page_raster_size(geometry).0;
    if base_width <= 0.0 || requested_scale <= 0.0 {
        return requested_scale;
    }
    // Ratio of two positive pixel-derived widths, far inside f32 range.
    let ratio = f64::from(delivered_width) / f64::from(base_width);
    #[allow(clippy::cast_possible_truncation)]
    let ratio = ratio as f32;
    ratio
}

pub fn page_point(
    position: Point<Pixels>,
    bounds: Bounds<Pixels>,
    geometry: PageGeometry,
    zoom: f32,
) -> Option<PdfPoint> {
    if !bounds.contains(&position) {
        return None;
    }
    let local = ViewportPoint::new(
        f64::from(position.x - bounds.origin.x),
        f64::from(position.y - bounds.origin.y),
    );
    Some(transform(geometry, zoom).viewport_to_pdf(local))
}

pub fn overlay_rect(rect: PdfRect, geometry: PageGeometry, zoom: f32) -> OverlayRect {
    let transform = transform(geometry, zoom);
    let first = transform.pdf_to_viewport(PdfPoint::new(rect.x_min, rect.y_min));
    let second = transform.pdf_to_viewport(PdfPoint::new(rect.x_max, rect.y_max));
    OverlayRect {
        left: ui_f32(first.x.min(second.x)),
        top: ui_f32(first.y.min(second.y)),
        width: ui_f32((first.x - second.x).abs()),
        height: ui_f32((first.y - second.y).abs()),
    }
}

pub fn overlay_point(point: PdfPoint, geometry: PageGeometry, zoom: f32) -> (f32, f32) {
    let point = transform(geometry, zoom).pdf_to_viewport(point);
    (ui_f32(point.x), ui_f32(point.y))
}

#[allow(clippy::cast_possible_truncation)]
pub fn ui_f32(value: f64) -> f32 {
    value as f32
}

fn transform(geometry: PageGeometry, zoom: f32) -> ViewportTransform {
    ViewportTransform::new(geometry, f64::from(zoom), RENDER_SCALE)
        .expect("validated viewport scale")
}

#[cfg(test)]
mod tests {
    use document_core::{PageGeometry, PdfRect, Rotation};

    use super::*;

    #[test]
    fn page_rect_maps_to_raster_overlay() {
        let page = PdfRect::new(0.0, 0.0, 200.0, 100.0).unwrap();
        let geometry = PageGeometry::new(page, page, Rotation::None, 1.0).unwrap();
        let overlay = overlay_rect(PdfRect::new(20.0, 30.0, 80.0, 50.0).unwrap(), geometry, 2.0);

        assert!((overlay.left - 60.0).abs() < f32::EPSILON);
        assert!((overlay.top - 150.0).abs() < f32::EPSILON);
        assert!((overlay.width - 180.0).abs() < f32::EPSILON);
        assert!((overlay.height - 60.0).abs() < f32::EPSILON);
    }

    #[test]
    fn delivered_scale_matches_request_when_raster_is_unchanged() {
        let page = PdfRect::new(0.0, 0.0, 200.0, 100.0).unwrap();
        let geometry = PageGeometry::new(page, page, Rotation::None, 1.0).unwrap();
        let (width, _) = page_raster_size(geometry);

        let scale = delivered_render_scale(geometry, 2.0, (width * 2.0) as u32);

        assert!((scale - 2.0).abs() < 0.01);
    }

    #[test]
    fn clamped_raster_reports_the_scale_it_really_used() {
        let page = PdfRect::new(0.0, 0.0, 200.0, 100.0).unwrap();
        let geometry = PageGeometry::new(page, page, Rotation::None, 1.0).unwrap();
        let (width, _) = page_raster_size(geometry);

        // Asked for 2x, but a pixel-budget clamp delivered the 1x bitmap.
        let clamped_width = width as u32;
        let scale = delivered_render_scale(geometry, 2.0, clamped_width);

        assert!((scale - 1.0).abs() < 0.01);
    }
}
