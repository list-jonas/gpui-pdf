use zpdf::{DisplayList, FontCache, ImageCache, TextSpan};

/// How many pages keep their interpreted display list (and decoded images) in
/// memory. Re-rasterising a cached list skips parsing, font loading, image
/// decoding and interpretation, which is what makes zooming feel instant.
const RENDER_CACHE_PAGES: usize = 4;
/// Text spans are small compared to display lists, so more pages fit.
const SPAN_CACHE_PAGES: usize = 32;

pub struct RenderedContent {
    pub fonts: FontCache,
    pub images: ImageCache,
    pub display_list: DisplayList,
}

/// Small most-recently-used caches keyed by page index.
pub struct PageCache {
    content: Vec<(usize, RenderedContent)>,
    spans: Vec<(usize, Vec<TextSpan>)>,
}

impl PageCache {
    pub const fn new() -> Self {
        Self {
            content: Vec::new(),
            spans: Vec::new(),
        }
    }

    pub fn content(&mut self, page_index: usize) -> Option<&RenderedContent> {
        let position = self
            .content
            .iter()
            .position(|(index, _)| *index == page_index)?;
        let entry = self.content.remove(position);
        self.content.push(entry);
        self.content.last().map(|(_, content)| content)
    }

    pub fn insert_content(
        &mut self,
        page_index: usize,
        content: RenderedContent,
    ) -> &RenderedContent {
        self.content.retain(|(index, _)| *index != page_index);
        while self.content.len() >= RENDER_CACHE_PAGES {
            self.content.remove(0);
        }
        self.content.push((page_index, content));
        &self.content.last().expect("just inserted").1
    }

    pub fn spans(&mut self, page_index: usize) -> Option<&[TextSpan]> {
        let position = self
            .spans
            .iter()
            .position(|(index, _)| *index == page_index)?;
        let entry = self.spans.remove(position);
        self.spans.push(entry);
        self.spans.last().map(|(_, spans)| spans.as_slice())
    }

    pub fn insert_spans(&mut self, page_index: usize, spans: Vec<TextSpan>) -> &[TextSpan] {
        self.spans.retain(|(index, _)| *index != page_index);
        while self.spans.len() >= SPAN_CACHE_PAGES {
            self.spans.remove(0);
        }
        self.spans.push((page_index, spans));
        &self.spans.last().expect("just inserted").1
    }

    /// Drops everything derived from the document, used after edits change it.
    pub fn clear(&mut self) {
        self.content.clear();
        self.spans.clear();
    }
}
