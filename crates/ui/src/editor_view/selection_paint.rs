use document_core::PdfRect;

use super::model::SelectedRun;

/// Painted selection geometry, grouped by page.
///
/// Selecting a whole document produces hundreds of thousands of text runs.
/// Painting one rectangle per run, and rescanning the whole selection for
/// every page, is what makes a large select-all crawl. Runs sharing a line are
/// merged into a single rectangle up front and stored per page, so painting a
/// page touches only that page's geometry.
#[derive(Default)]
pub struct SelectionOverlays {
    /// One entry per page index, each holding that page's merged rectangles.
    by_page: Vec<Vec<PdfRect>>,
}

impl SelectionOverlays {
    pub fn build(runs: &[SelectedRun], page_count: usize) -> Self {
        let mut by_page: Vec<Vec<PdfRect>> = vec![Vec::new(); page_count];
        for run in runs {
            let Some(page) = by_page.get_mut(run.page_index) else {
                continue;
            };
            match page.last_mut() {
                Some(last) if same_line(*last, run.rect) => *last = union(*last, run.rect),
                _ => page.push(run.rect),
            }
        }
        Self { by_page }
    }

    pub fn for_page(&self, page_index: usize) -> &[PdfRect] {
        self.by_page.get(page_index).map_or(&[], Vec::as_slice)
    }

    pub fn clear(&mut self) {
        self.by_page.clear();
    }
}

/// Two runs belong to the same painted rectangle when they share a baseline
/// and sit next to each other, which is the common case within a line.
fn same_line(left: PdfRect, right: PdfRect) -> bool {
    let height = (left.y_max - left.y_min).max(1.0);
    if (left.y_min - right.y_min).abs() > height * 0.5 {
        return false;
    }
    // Allow a word-sized gap so spaces between runs do not split the rectangle,
    // but never bridge a column break.
    let gap = right.x_min - left.x_max;
    gap >= -height && gap <= height
}

fn union(left: PdfRect, right: PdfRect) -> PdfRect {
    PdfRect::new(
        left.x_min.min(right.x_min),
        left.y_min.min(right.y_min),
        left.x_max.max(right.x_max),
        left.y_max.max(right.y_max),
    )
    .unwrap_or(left)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(page_index: usize, x_min: f64, x_max: f64, y_min: f64) -> SelectedRun {
        SelectedRun {
            page_index,
            rect: PdfRect::new(x_min, y_min, x_max, y_min + 10.0).unwrap(),
            text: "x".to_owned(),
        }
    }

    #[test]
    fn adjacent_runs_on_a_line_merge_into_one_rectangle() {
        let overlays =
            SelectionOverlays::build(&[run(0, 0.0, 30.0, 100.0), run(0, 32.0, 60.0, 100.0)], 1);

        let rects = overlays.for_page(0);
        assert_eq!(rects.len(), 1);
        assert!((rects[0].x_max - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn separate_lines_stay_separate() {
        let overlays =
            SelectionOverlays::build(&[run(0, 0.0, 30.0, 100.0), run(0, 0.0, 30.0, 80.0)], 1);

        assert_eq!(overlays.for_page(0).len(), 2);
    }

    #[test]
    fn runs_are_grouped_by_their_own_page() {
        let overlays =
            SelectionOverlays::build(&[run(0, 0.0, 30.0, 100.0), run(2, 0.0, 30.0, 100.0)], 3);

        assert_eq!(overlays.for_page(0).len(), 1);
        assert!(overlays.for_page(1).is_empty());
        assert_eq!(overlays.for_page(2).len(), 1);
    }
}
