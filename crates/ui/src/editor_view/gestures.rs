const DOCUMENT_PADDING: f32 = 64.0;
const PAGE_GAP: f32 = 24.0;
/// Must match the zoom range enforced by the editor actions.
const MIN_ZOOM: f32 = 0.25;
const MAX_ZOOM: f32 = 8.0;

/// Everything needed to keep one window-space point pinned while the zoom
/// changes: where the scroll viewport sits, how far it is scrolled, and the
/// point that must not move.
#[derive(Clone, Copy)]
pub(super) struct AnchorContext {
    pub viewport_origin: (f32, f32),
    pub viewport_size: (f32, f32),
    /// Current scroll offset, negative as the document scrolls down/right.
    pub offset: (f32, f32),
    /// Window-space point to keep stationary (pointer or viewport centre).
    pub anchor: (f32, f32),
}

pub(super) fn pinch_zoom(zoom: f32, delta: f32) -> f32 {
    (zoom * (1.0 + delta.clamp(-0.5, 0.5))).clamp(MIN_ZOOM, MAX_ZOOM)
}

/// Result of an anchored zoom: the new scroll offset plus the page the anchor
/// landed on, which the caller uses to keep the page indicator honest without
/// waiting for the next layout pass.
#[derive(Clone, Copy)]
pub(super) struct AnchoredScroll {
    pub offset: (f32, f32),
    pub page_index: usize,
}

/// Scroll offset that keeps the document point under `context.anchor` under
/// that same window point after the zoom changes.
///
/// `pages` holds unscaled page sizes in layout order. Each page sits centred in
/// a full-width row, and the rows are stacked with a fixed gap inside a padded
/// column, so an anchor is described by its unscaled offset from the centre of
/// a page plus that page's index. Both are zoom independent, so the new
/// position is a straight re-projection at the new zoom.
pub(super) fn anchored_document_offset(
    pages: &[(f32, f32)],
    old_zoom: f32,
    new_zoom: f32,
    context: AnchorContext,
) -> AnchoredScroll {
    if pages.is_empty() || old_zoom <= 0.0 || new_zoom <= 0.0 {
        return AnchoredScroll {
            offset: context.offset,
            page_index: 0,
        };
    }
    let content_x = context.anchor.0 - context.viewport_origin.0 - context.offset.0;
    let content_y = context.anchor.1 - context.viewport_origin.1 - context.offset.1;

    let new_width = content_width(pages, new_zoom, context.viewport_size.0);
    let new_height = content_height(pages, new_zoom, context.viewport_size.1);

    let (page_index, document_y) = document_position(pages, old_zoom, content_y);
    let page_width = pages[page_index].0;
    let document_x =
        (content_x - page_center_x(page_width, old_zoom, context.viewport_size.0)) / old_zoom;

    let target_x =
        page_center_x(page_width, new_zoom, context.viewport_size.0) + document_x * new_zoom;
    let target_y =
        DOCUMENT_PADDING / 2.0 + usize_f32(page_index) * PAGE_GAP + document_y * new_zoom;

    let x = context.anchor.0 - context.viewport_origin.0 - target_x;
    let y = context.anchor.1 - context.viewport_origin.1 - target_y;

    AnchoredScroll {
        offset: (
            x.clamp((context.viewport_size.0 - new_width).min(0.0), 0.0),
            y.clamp((context.viewport_size.1 - new_height).min(0.0), 0.0),
        ),
        page_index,
    }
}

fn content_width(pages: &[(f32, f32)], zoom: f32, viewport_width: f32) -> f32 {
    let max_page_width = pages.iter().map(|page| page.0).fold(0.0, f32::max);
    (max_page_width * zoom + DOCUMENT_PADDING).max(viewport_width)
}

/// Horizontal centre of one page, measured from the left edge of the scrolled
/// content. A page narrower than the viewport is centred in the viewport; a
/// wider one is centred on itself, which is where the full-width row puts it.
fn page_center_x(page_width: f32, zoom: f32, viewport_width: f32) -> f32 {
    let row_width = (viewport_width - DOCUMENT_PADDING).max(page_width * zoom);
    DOCUMENT_PADDING / 2.0 + row_width / 2.0
}

fn content_height(pages: &[(f32, f32)], zoom: f32, viewport_height: f32) -> f32 {
    let gaps = usize_f32(pages.len().saturating_sub(1)) * PAGE_GAP;
    let total_page_height: f32 = pages.iter().map(|page| page.1).sum();
    (total_page_height * zoom + gaps + DOCUMENT_PADDING).max(viewport_height)
}

/// Maps a vertical content position to the page it falls on plus the unscaled
/// distance from the top of the first page. Points inside a gap clamp to the
/// nearest page edge so the anchor never lands between pages.
fn document_position(pages: &[(f32, f32)], zoom: f32, content_y: f32) -> (usize, f32) {
    let last = pages.len() - 1;
    let mut top = DOCUMENT_PADDING / 2.0;
    let mut prior = 0.0;
    for (index, (_, height)) in pages.iter().enumerate() {
        let bottom = top + height * zoom;
        if content_y <= bottom || index == last {
            let local = ((content_y - top) / zoom).clamp(0.0, *height);
            return (index, prior + local);
        }
        top = bottom + PAGE_GAP;
        prior += height;
    }
    (last, prior)
}

fn usize_f32(value: usize) -> f32 {
    f32::from(u16::try_from(value).unwrap_or(u16::MAX))
}

#[cfg(test)]
mod tests {
    use super::{AnchorContext, anchored_document_offset, pinch_zoom};

    #[test]
    fn applies_and_clamps_pinch_delta() {
        assert!((pinch_zoom(1.0, 0.12) - 1.12).abs() < f32::EPSILON);
        assert!((pinch_zoom(1.0, -2.0) - 0.5).abs() < f32::EPSILON);
    }

    /// Window position of a document point, given a scroll offset and zoom.
    fn projected(
        pages: &[(f32, f32)],
        zoom: f32,
        viewport: (f32, f32),
        offset: (f32, f32),
        page_index: usize,
        local: (f32, f32),
    ) -> (f32, f32) {
        let page_left = super::page_center_x(pages[page_index].0, zoom, viewport.0)
            - pages[page_index].0 * zoom / 2.0;
        let prior: f32 = pages[..page_index].iter().map(|page| page.1).sum();
        let page_top = super::DOCUMENT_PADDING / 2.0
            + prior * zoom
            + super::usize_f32(page_index) * super::PAGE_GAP;
        (
            offset.0 + page_left + local.0 * zoom,
            offset.1 + page_top + local.1 * zoom,
        )
    }

    #[test]
    fn keeps_pointer_over_the_same_document_point() {
        let pages = [(500.0, 800.0), (500.0, 800.0)];
        let viewport = (600.0, 900.0);
        let offset = (-200.0, -300.0);
        let anchor = projected(&pages, 1.0, viewport, offset, 1, (120.0, 250.0));
        let next = anchored_document_offset(
            &pages,
            1.0,
            2.0,
            AnchorContext {
                viewport_origin: (0.0, 0.0),
                viewport_size: viewport,
                offset,
                anchor,
            },
        );
        assert_eq!(next.page_index, 1);
        let moved = projected(&pages, 2.0, viewport, next.offset, 1, (120.0, 250.0));

        assert!((moved.0 - anchor.0).abs() < 0.01);
        assert!((moved.1 - anchor.1).abs() < 0.01);
    }

    #[test]
    fn keeps_anchor_when_viewport_is_offset_in_the_window() {
        let pages = [(500.0, 800.0)];
        let viewport_origin = (220.0, 60.0);
        let viewport = (400.0, 400.0);
        let offset = (-80.0, -100.0);
        let local = (100.0, 300.0);
        let raw = projected(&pages, 1.0, viewport, offset, 0, local);
        let anchor = (raw.0 + viewport_origin.0, raw.1 + viewport_origin.1);
        let next = anchored_document_offset(
            &pages,
            1.0,
            1.5,
            AnchorContext {
                viewport_origin,
                viewport_size: viewport,
                offset,
                anchor,
            },
        );
        let moved = projected(&pages, 1.5, viewport, next.offset, 0, local);

        assert!((moved.0 + viewport_origin.0 - anchor.0).abs() < 0.01);
        assert!((moved.1 + viewport_origin.1 - anchor.1).abs() < 0.01);
    }

    #[test]
    fn clamps_to_the_scrollable_range() {
        let pages = [(500.0, 800.0)];
        let viewport = (1000.0, 900.0);
        let next = anchored_document_offset(
            &pages,
            2.0,
            0.25,
            AnchorContext {
                viewport_origin: (0.0, 0.0),
                viewport_size: viewport,
                offset: (-100.0, -900.0),
                anchor: (500.0, 450.0),
            },
        );

        assert!(next.offset.0.abs() < f32::EPSILON);
        assert!(next.offset.1.abs() < f32::EPSILON);
    }

    /// Zooming in from a page that fits to one that overflows the viewport used
    /// to drift left, because the page kept being centred on the viewport while
    /// the content grew past it.
    #[test]
    fn keeps_pointer_anchored_when_the_page_grows_past_the_viewport() {
        let pages = [(500.0, 800.0)];
        let viewport = (600.0, 900.0);
        let offset = (0.0, 0.0);
        // Right-hand side of the page, where left drift is most visible.
        let local = (420.0, 200.0);
        let anchor = projected(&pages, 1.0, viewport, offset, 0, local);
        let next = anchored_document_offset(
            &pages,
            1.0,
            4.0,
            AnchorContext {
                viewport_origin: (0.0, 0.0),
                viewport_size: viewport,
                offset,
                anchor,
            },
        );
        let moved = projected(&pages, 4.0, viewport, next.offset, 0, local);

        assert!((moved.0 - anchor.0).abs() < 0.01);
        assert!((moved.1 - anchor.1).abs() < 0.01);
    }

    /// A page wider than the viewport must stay reachable: its left edge sits at
    /// the content's left padding, not off-screen.
    #[test]
    fn a_page_wider_than_the_viewport_starts_at_the_content_edge() {
        let pages = [(500.0, 800.0)];
        let viewport = (400.0, 900.0);
        let left = super::page_center_x(pages[0].0, 2.0, viewport.0) - pages[0].0 * 2.0 / 2.0;

        assert!((left - super::DOCUMENT_PADDING / 2.0).abs() < f32::EPSILON);
    }
}
