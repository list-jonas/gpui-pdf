use document_core::PdfRect;
use gpui::{Context, Window};

use crate::actions::{NextSearchResult, PreviousSearchResult, Search};

use super::EditorView;
use super::model::SearchMatch;

impl EditorView {
    pub(super) fn open_search(&mut self, _: &Search, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh_search(cx, false);
        self.search_input
            .update(cx, |input, cx| input.focus(window, cx));
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
        self.search_query = query.clone();
        self.search_matches.clear();
        self.search_index = 0;
        if query.is_empty() {
            cx.notify();
            return;
        }

        for (page_index, page) in self.pages.iter().enumerate() {
            for fragment in &page.fragments {
                self.search_matches.extend(fragment_matches(
                    page_index,
                    fragment.rect,
                    &fragment.text,
                    &query,
                ));
            }
        }
        if let Some(result) = self.search_matches.first().copied() {
            self.jump_to_page(result.page_index, cx);
        }
        self.status = if self.search_matches.is_empty() {
            format!("No results for \"{query}\"").into()
        } else {
            format!("{} results for \"{query}\"", self.search_matches.len()).into()
        };
        cx.notify();
    }

    fn move_search_result(&mut self, direction: isize, cx: &mut Context<Self>) {
        self.refresh_search(cx, false);
        if self.search_matches.is_empty() {
            self.status = "No search results".into();
            cx.notify();
            return;
        }
        let count = self.search_matches.len() as isize;
        self.search_index = (self.search_index as isize + direction).rem_euclid(count) as usize;
        let result = self.search_matches[self.search_index];
        self.jump_to_page(result.page_index, cx);
        self.status = format!(
            "Result {} of {}",
            self.search_index + 1,
            self.search_matches.len()
        )
        .into();
    }
}

fn fragment_matches(page_index: usize, rect: PdfRect, text: &str, query: &str) -> Vec<SearchMatch> {
    let (haystack, needle) = if query.is_ascii() {
        (text.to_ascii_lowercase(), query.to_ascii_lowercase())
    } else {
        (text.to_owned(), query.to_owned())
    };
    let char_count = text.chars().count();
    if char_count == 0 || needle.is_empty() {
        return Vec::new();
    }
    haystack
        .match_indices(&needle)
        .filter_map(|(byte_start, _)| {
            let start = haystack[..byte_start].chars().count();
            let end = start + needle.chars().count();
            let width = rect.width() / char_count as f64;
            PdfRect::new(
                rect.x_min + width * start as f64,
                rect.y_min,
                rect.x_min + width * end as f64,
                rect.y_max,
            )
            .ok()
            .map(|rect| SearchMatch { page_index, rect })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use document_core::PdfRect;

    use super::fragment_matches;

    #[test]
    fn finds_each_case_insensitive_match_with_its_own_rect() {
        let rect = PdfRect::new(0.0, 0.0, 110.0, 10.0).unwrap();
        let matches = fragment_matches(2, rect, "Find find!", "find");

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].page_index, 2);
        assert!(matches[0].rect.x_max < matches[1].rect.x_min);
    }
}
