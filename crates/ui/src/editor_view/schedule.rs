use crate::{PageKind, PageRequest};

/// Raster scale used for the first pass over a page. Cheap enough that a big
/// document fills in quickly, sharp enough to read headings and layout.
pub const PREVIEW_SCALE: f32 = 0.35;
/// Upper bound on raster scale, to keep memory and render time sane.
pub const MAX_RENDER_SCALE: f32 = 4.0;
/// Pages on each side of the viewport kept at full quality, so scrolling by a
/// page or two shows a sharp raster immediately.
const SHARP_MARGIN: usize = 1;
/// Pages on each side of the viewport that get a preview, so fast scrolling
/// still lands on something other than an empty rectangle.
const PREVIEW_MARGIN: usize = 6;

#[derive(Clone, Copy, Debug)]
pub struct PageState {
    /// Scale of the raster currently on screen, or 0 when the page is empty.
    pub render_scale: f32,
    /// Scale already asked for, so identical requests are not repeated.
    pub requested_scale: f32,
    pub text_loaded: bool,
    pub text_requested: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Viewport {
    pub first_visible: usize,
    pub last_visible: usize,
    /// Raster scale that matches the current on-screen size.
    pub target_scale: f32,
}

/// Picks the work worth doing for the current viewport, nearest page first.
///
/// Visible pages are rasterised sharp; nearby pages get a cheap preview so
/// scrolling is never blocked behind full-quality rendering of pages the
/// reader has already passed.
pub fn plan(viewport: Viewport, pages: &[PageState]) -> Vec<PageRequest> {
    if pages.is_empty() {
        return Vec::new();
    }
    let target = viewport.target_scale.clamp(PREVIEW_SCALE, MAX_RENDER_SCALE);
    let sharp_first = viewport.first_visible.saturating_sub(SHARP_MARGIN);
    let sharp_last = (viewport.last_visible + SHARP_MARGIN).min(pages.len() - 1);
    let preview_first = viewport.first_visible.saturating_sub(PREVIEW_MARGIN);
    let preview_last = (viewport.last_visible + PREVIEW_MARGIN).min(pages.len() - 1);

    let mut jobs = Vec::new();
    for (page_index, page) in pages.iter().enumerate() {
        let distance = distance_from_viewport(viewport, page_index);
        let sharp = (sharp_first..=sharp_last).contains(&page_index);
        let previewable = (preview_first..=preview_last).contains(&page_index);

        if sharp && needs_scale(page, target) {
            // An empty page renders a preview first so something appears while
            // the full-quality raster is still being produced.
            if page.render_scale <= 0.0 && target > PREVIEW_SCALE {
                jobs.push(PageRequest {
                    page_index,
                    scale: PREVIEW_SCALE,
                    kind: PageKind::Preview,
                    priority: distance,
                });
            }
            jobs.push(PageRequest {
                page_index,
                scale: target,
                kind: PageKind::Sharp,
                priority: 1_000 + distance,
            });
        } else if previewable && page.render_scale <= 0.0 && page.requested_scale <= 0.0 {
            jobs.push(PageRequest {
                page_index,
                scale: PREVIEW_SCALE,
                kind: PageKind::Preview,
                priority: 10_000 + distance,
            });
        }

        if !page.text_loaded && !page.text_requested && previewable {
            jobs.push(PageRequest {
                page_index,
                scale: 1.0,
                kind: PageKind::Text,
                priority: 100_000 + distance,
            });
        }
    }
    jobs
}

/// Background pass that fills in the rest of a document once the viewport is
/// satisfied, so search and thumbnails eventually cover every page.
pub fn plan_background(viewport: Viewport, pages: &[PageState]) -> Vec<PageRequest> {
    let mut jobs = Vec::new();
    for (page_index, page) in pages.iter().enumerate() {
        let distance = distance_from_viewport(viewport, page_index);
        if page.render_scale <= 0.0 && page.requested_scale <= 0.0 {
            jobs.push(PageRequest {
                page_index,
                scale: PREVIEW_SCALE,
                kind: PageKind::Preview,
                priority: 1_000_000 + distance,
            });
        }
        if !page.text_loaded && !page.text_requested {
            jobs.push(PageRequest {
                page_index,
                scale: 1.0,
                kind: PageKind::Text,
                priority: 2_000_000 + distance,
            });
        }
    }
    jobs
}

fn needs_scale(page: &PageState, target: f32) -> bool {
    page.requested_scale + 0.01 < target
}

fn distance_from_viewport(viewport: Viewport, page_index: usize) -> u32 {
    let before = viewport.first_visible.saturating_sub(page_index);
    let after = page_index.saturating_sub(viewport.last_visible);
    let distance = before.max(after);
    u32::try_from(distance).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_pages(count: usize) -> Vec<PageState> {
        vec![
            PageState {
                render_scale: 0.0,
                requested_scale: 0.0,
                text_loaded: false,
                text_requested: false,
            };
            count
        ]
    }

    fn viewport(first: usize, last: usize) -> Viewport {
        Viewport {
            first_visible: first,
            last_visible: last,
            target_scale: 1.5,
        }
    }

    #[test]
    fn visible_pages_get_a_preview_before_the_sharp_raster() {
        let jobs = plan(viewport(0, 0), &empty_pages(200));
        let first: Vec<_> = jobs.iter().filter(|job| job.page_index == 0).collect();

        assert_eq!(first[0].kind, PageKind::Preview);
        assert_eq!(first[1].kind, PageKind::Sharp);
        assert!(first[0].priority < first[1].priority);
    }

    #[test]
    fn distant_pages_are_left_to_the_background_pass() {
        let jobs = plan(viewport(0, 0), &empty_pages(200));

        assert!(jobs.iter().all(|job| job.page_index < 20));
    }

    #[test]
    fn nearer_pages_outrank_further_ones() {
        let jobs = plan(viewport(10, 12), &empty_pages(200));
        let near = jobs
            .iter()
            .find(|job| job.page_index == 13 && job.kind == PageKind::Sharp)
            .expect("near page queued");
        let far = jobs
            .iter()
            .find(|job| job.page_index == 17 && job.kind == PageKind::Preview)
            .expect("far page queued");

        assert!(near.priority < far.priority);
    }

    #[test]
    fn a_satisfied_page_queues_no_raster_work() {
        let mut pages = empty_pages(3);
        for page in &mut pages {
            page.render_scale = 1.5;
            page.requested_scale = 1.5;
            page.text_loaded = true;
        }

        assert!(plan(viewport(0, 2), &pages).is_empty());
    }
}
