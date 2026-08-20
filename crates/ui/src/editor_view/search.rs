use document_core::PdfRect;
use gpui::{Context, Window};

use crate::actions::{NextSearchResult, PreviousSearchResult, Search};

use super::EditorView;
use super::Severity;
use super::model::SearchMatch;

impl EditorView {
    pub(super) fn open_search(&mut self, _: &Search, window: &mut Window, cx: &mut Context<Self>) {
        self.panels.search = true;
        self.refresh_search(cx, false);
        let search_input = self.search_input.clone();
        window.defer(cx, move |window, cx| {
            search_input.update(cx, |input, cx| input.focus(window, cx));
        });
        cx.notify();
    }

    /// Closes the search bar, clears highlights and returns focus to the page.
    pub(super) fn close_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.panels.search = false;
        self.search_matches.clear();
        self.search_index = 0;
        self.search_query.clear();
        self.search_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.focus_handle.focus(window);
        cx.notify();
    }

    pub(super) fn next_search_result(
        &mut self,
        _: &NextSearchResult,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_search_result(1, cx);
    }

    pub(super) fn previous_search_result(
        &mut self,
        _: &PreviousSearchResult,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_search_result(-1, cx);
    }

    pub(super) fn refresh_search(&mut self, cx: &mut Context<Self>, force: bool) {
        let query = self.search_input.read(cx).value().trim().to_owned();
        if !force && query == self.search_query {
            return;
        }
        let query_changed = query != self.search_query;
        self.search_query.clone_from(&query);
        self.search_matches.clear();
        if query_changed {
            self.search_index = 0;
        }
        if query.is_empty() {
            cx.notify();
            return;
        }

        for (page_index, page) in self.pages.iter().enumerate() {
            self.search_matches
                .extend(page_matches(page_index, &page.fragments, &query));
        }
        if self.search_index >= self.search_matches.len() {
            self.search_index = 0;
        }
        if query_changed && let Some(result) = self.search_matches.first().copied() {
            self.jump_to_page(result.page_index, cx);
        }
        cx.notify();
    }

    fn move_search_result(&mut self, direction: isize, cx: &mut Context<Self>) {
        self.refresh_search(cx, false);
        if self.search_matches.is_empty() {
            let message = if self.search_query.is_empty() {
                "Type something to search".to_owned()
            } else {
                format!("No results for \"{}\"", self.search_query)
            };
            self.flash(message, Severity::Info, cx);
            return;
        }
        self.search_index = if direction.is_negative() {
            self.search_index
                .checked_sub(1)
                .unwrap_or(self.search_matches.len() - 1)
        } else {
            (self.search_index + 1) % self.search_matches.len()
        };
        let result = self.search_matches[self.search_index];
        self.jump_to_page(result.page_index, cx);
        self.flash(
            format!(
                "Result {} of {}",
                self.search_index + 1,
                self.search_matches.len()
            ),
            Severity::Info,
            cx,
        );
    }
}

fn page_matches(
    page_index: usize,
    fragments: &[pdf_engine::TextFragment],
    query: &str,
) -> Vec<SearchMatch> {
    let text: String = fragments
        .iter()
        .map(|fragment| fragment.text.as_str())
        .collect();
    let (haystack, needle) = if query.is_ascii() {
        (text.to_ascii_lowercase(), query.to_ascii_lowercase())
    } else {
        (text.clone(), query.to_owned())
    };
    if text.is_empty() || needle.is_empty() {
        return Vec::new();
    }
    haystack
        .match_indices(&needle)
        .filter_map(|(byte_start, _)| {
            let start = u32::try_from(haystack[..byte_start].chars().count()).unwrap_or(u32::MAX);
            let end =
                start.saturating_add(u32::try_from(needle.chars().count()).unwrap_or(u32::MAX));
            matching_rect(fragments, start, end).map(|rect| SearchMatch { page_index, rect })
        })
        .collect()
}

fn matching_rect(fragments: &[pdf_engine::TextFragment], start: u32, end: u32) -> Option<PdfRect> {
    let mut offset = 0_u32;
    let mut matched: Option<PdfRect> = None;
    for fragment in fragments {
        let count = u32::try_from(fragment.text.chars().count()).unwrap_or(u32::MAX);
        let fragment_end = offset.saturating_add(count);
        if start < fragment_end && end > offset {
            let left = start.saturating_sub(offset).min(count);
            let right = end.saturating_sub(offset).min(count);
            if left < right {
                let width = fragment.rect.width() / f64::from(count);
                let rect = PdfRect::new(
                    fragment.rect.x_min + width * f64::from(left),
                    fragment.rect.y_min,
                    fragment.rect.x_min + width * f64::from(right),
                    fragment.rect.y_max,
                )
                .ok()?;
                matched = Some(match matched {
                    Some(existing) => PdfRect::new(
                        existing.x_min.min(rect.x_min),
                        existing.y_min.min(rect.y_min),
                        existing.x_max.max(rect.x_max),
                        existing.y_max.max(rect.y_max),
                    )
                    .ok()?,
                    None => rect,
                });
            }
        }
        offset = fragment_end;
        if offset >= end {
            break;
        }
    }
    matched
}

#[cfg(test)]
mod tests {
    use document_core::PdfRect;

    use pdf_engine::TextFragment;

    use super::page_matches;

    #[test]
    fn finds_each_case_insensitive_match_with_its_own_rect() {
        let matches = page_matches(
            2,
            &[TextFragment {
                text: "Find find!".into(),
                rect: PdfRect::new(0.0, 0.0, 110.0, 10.0).unwrap(),
            }],
            "find",
        );

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].page_index, 2);
        assert!(matches[0].rect.x_max < matches[1].rect.x_min);
    }

    #[test]
    fn finds_text_split_across_pdf_fragments() {
        let matches = page_matches(
            0,
            &[
                TextFragment {
                    text: "Sea".into(),
                    rect: PdfRect::new(0.0, 0.0, 30.0, 10.0).unwrap(),
                },
                TextFragment {
                    text: "rch".into(),
                    rect: PdfRect::new(30.0, 0.0, 60.0, 10.0).unwrap(),
                },
            ],
            "search",
        );

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].rect, PdfRect::new(0.0, 0.0, 60.0, 10.0).unwrap());
    }
}
