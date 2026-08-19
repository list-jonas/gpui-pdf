const DOCUMENT_PADDING: f32 = 64.0;
const PAGE_GAP: f32 = 24.0;

#[derive(Clone, Copy)]
pub(super) struct DocumentMetrics {
    pub page_count: usize,
    pub max_page_width: f32,
    pub total_page_height: f32,
    pub prior_page_height: f32,
    pub page_size: (f32, f32),
}

#[derive(Clone, Copy)]
pub(super) struct AnchorContext {
    pub page_index: usize,
    pub zoom: f32,
    pub ratio: f32,
    pub viewport_origin: (f32, f32),
    pub viewport_size: (f32, f32),
    pub pointer: (f32, f32),
    pub page_origin: (f32, f32),
}

pub(super) fn pinch_zoom(zoom: f32, delta: f32) -> f32 {
    (zoom * (1.0 + delta.clamp(-0.5, 0.5))).clamp(0.25, 4.0)
}

pub(super) fn anchored_document_offset(
    metrics: DocumentMetrics,
    context: AnchorContext,
) -> (f32, f32) {
    let gaps = usize_f32(metrics.page_count.saturating_sub(1)) * PAGE_GAP;
    let content_width =
        (metrics.max_page_width * context.zoom + DOCUMENT_PADDING).max(context.viewport_size.0);
    let content_height = (metrics.total_page_height * context.zoom + gaps + DOCUMENT_PADDING)
        .max(context.viewport_size.1);
    let page_left = (content_width - metrics.page_size.0 * context.zoom) / 2.0;
    let page_top = DOCUMENT_PADDING / 2.0
        + metrics.prior_page_height * context.zoom
        + usize_f32(context.page_index) * PAGE_GAP;
    let local_x = (context.pointer.0 - context.page_origin.0) * context.ratio;
    let local_y = (context.pointer.1 - context.page_origin.1) * context.ratio;
    let x = context.pointer.0 - context.viewport_origin.0 - page_left - local_x;
    let y = context.pointer.1 - context.viewport_origin.1 - page_top - local_y;

    (
        x.clamp((context.viewport_size.0 - content_width).min(0.0), 0.0),
        y.clamp((context.viewport_size.1 - content_height).min(0.0), 0.0),
    )
}

fn usize_f32(value: usize) -> f32 {
    f32::from(u16::try_from(value).unwrap_or(u16::MAX))
}

#[cfg(test)]
mod tests {
    use super::{AnchorContext, DocumentMetrics, anchored_document_offset, pinch_zoom};

    #[test]
    fn applies_and_clamps_pinch_delta() {
        assert!((pinch_zoom(1.0, 0.12) - 1.12).abs() < f32::EPSILON);
        assert!((pinch_zoom(1.0, -2.0) - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn keeps_anchor_when_page_grows_past_viewport() {
        let offset = anchored_document_offset(
            DocumentMetrics {
                page_count: 1,
                max_page_width: 500.0,
                total_page_height: 800.0,
                prior_page_height: 0.0,
                page_size: (500.0, 800.0),
            },
            AnchorContext {
                page_index: 0,
                zoom: 2.0,
                ratio: 2.0,
                viewport_origin: (0.0, 0.0),
                viewport_size: (1000.0, 900.0),
                pointer: (500.0, 432.0),
                page_origin: (250.0, 32.0),
            },
        );

        assert!((offset.0 + 32.0).abs() < f32::EPSILON);
        assert!((offset.1 + 400.0).abs() < f32::EPSILON);
    }

    #[test]
    fn includes_prior_pages_in_vertical_anchor() {
        let offset = anchored_document_offset(
            DocumentMetrics {
                page_count: 2,
                max_page_width: 500.0,
                total_page_height: 800.0,
                prior_page_height: 400.0,
                page_size: (500.0, 400.0),
            },
            AnchorContext {
                page_index: 1,
                zoom: 2.0,
                ratio: 2.0,
                viewport_origin: (0.0, 0.0),
                viewport_size: (1000.0, 900.0),
                pointer: (500.0, 556.0),
                page_origin: (250.0, 456.0),
            },
        );

        assert!((offset.1 + 500.0).abs() < f32::EPSILON);
    }
}
