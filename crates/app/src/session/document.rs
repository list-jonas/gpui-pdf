use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_channel::{Receiver, Sender};
use pdf_engine::{DocumentMetadata, OpenRequest, PageMetadata, RenderRequest, RenderedPage};
use pdf_engine_zpdf::ZpdfEngine;
use rendering::{DocumentCommand, DocumentEvent, DocumentWorker};
use ui::{EditorRequest, EditorUpdate, LoadedPage, OpenedDocument};

pub fn start(requests: Receiver<EditorRequest>, updates: Sender<EditorUpdate>) {
    std::thread::spawn(move || {
        let mut open: Option<OpenDocumentSession> = None;
        while let Ok(request) = requests.recv_blocking() {
            let result = match request {
                EditorRequest::Open(path) => load(&path, 0, &updates).map(|session| {
                    open = Some(session);
                }),
                EditorRequest::RenderPage { page_index, scale } => {
                    rerender(open.as_ref(), page_index, scale, &updates)
                }
                EditorRequest::SaveAs {
                    source,
                    destination,
                    page_index,
                    edits,
                } => save(&source, &destination, page_index, edits, &updates).map(|session| {
                    open = session.or(open.take());
                }),
            };
            if let Err(error) = result {
                let _ = updates.send_blocking(EditorUpdate::Failed(error));
            }
        }
    });
}

/// Keeps the opened document alive so pages can be re-rendered at higher
/// raster scales without reparsing the file on every zoom change.
pub struct OpenDocumentSession {
    worker: DocumentWorker,
    pages: Vec<PageMetadata>,
}

fn rerender(
    session: Option<&OpenDocumentSession>,
    page_index: usize,
    scale: f32,
    updates: &Sender<EditorUpdate>,
) -> Result<(), String> {
    let Some(session) = session else {
        return Ok(());
    };
    if session.pages.get(page_index).is_none() {
        return Ok(());
    }
    session
        .worker
        .send(DocumentCommand::Render {
            request: RenderRequest { page_index, scale },
            generation: session.worker.current_generation(),
        })
        .map_err(|error| error.to_string())?;
    let rendered = expect_rendered(receive(&session.worker)?)?;
    updates
        .send_blocking(EditorUpdate::PageRerendered {
            page_index,
            scale,
            rendered,
        })
        .map_err(|error| error.to_string())
}

fn load(
    path: &Path,
    page_index: usize,
    updates: &Sender<EditorUpdate>,
) -> Result<OpenDocumentSession, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let worker = DocumentWorker::spawn(Arc::new(ZpdfEngine), OpenRequest::new(bytes));
    let document = expect_opened(receive(&worker)?)?;
    let page_count = document.page_count;
    if page_index >= page_count {
        return Err(format!("page {} does not exist", page_index + 1));
    }

    let pages = load_page_metadata(&worker, page_count)?;
    worker
        .send(DocumentCommand::FormFields)
        .map_err(|error| error.to_string())?;
    let forms = expect_forms(receive(&worker)?)?;

    updates
        .send_blocking(EditorUpdate::Opened(Box::new(OpenedDocument {
            path: path.to_path_buf(),
            document,
            pages: pages.clone(),
            forms,
            initial_page: page_index,
        })))
        .map_err(|error| error.to_string())?;

    for index in page_load_order(page_index, page_count) {
        let page = load_page(&worker, pages[index])?;
        updates
            .send_blocking(EditorUpdate::PageLoaded(Box::new(page)))
            .map_err(|error| error.to_string())?;
    }
    Ok(OpenDocumentSession { worker, pages })
}

fn load_page_metadata(
    worker: &DocumentWorker,
    page_count: usize,
) -> Result<Vec<PageMetadata>, String> {
    (0..page_count)
        .map(|page_index| {
            worker
                .send(DocumentCommand::PageMetadata { page_index })
                .map_err(|error| error.to_string())?;
            expect_page_metadata(receive(worker)?)
        })
        .collect()
}

fn load_page(worker: &DocumentWorker, page: PageMetadata) -> Result<LoadedPage, String> {
    let page_index = page.index;
    worker
        .send(DocumentCommand::Render {
            request: RenderRequest {
                page_index,
                scale: 1.5,
            },
            generation: worker.current_generation(),
        })
        .map_err(|error| error.to_string())?;
    let rendered = expect_rendered(receive(worker)?)?;
    worker
        .send(DocumentCommand::ExtractText { page_index })
        .map_err(|error| error.to_string())?;
    let text = expect_text(receive(worker)?)?;
    worker
        .send(DocumentCommand::TextFragments { page_index })
        .map_err(|error| error.to_string())?;
    let fragments = expect_fragments(receive(worker)?)?;
    Ok(LoadedPage {
        page,
        rendered,
        text,
        fragments,
    })
}

fn page_load_order(initial_page: usize, page_count: usize) -> impl Iterator<Item = usize> {
    (initial_page..page_count).chain(0..initial_page)
}

fn save(
    source: &Path,
    destination: &Path,
    page_index: usize,
    edits: Vec<pdf_engine::EditCommand>,
    updates: &Sender<EditorUpdate>,
) -> Result<Option<OpenDocumentSession>, String> {
    let bytes = std::fs::read(source).map_err(|error| format!("{}: {error}", source.display()))?;
    let worker = DocumentWorker::spawn(Arc::new(ZpdfEngine), OpenRequest::new(bytes));
    expect_opened(receive(&worker)?)?;
    worker
        .send(DocumentCommand::Export { edits })
        .map_err(|error| error.to_string())?;
    let output = expect_exported(receive(&worker)?)?;
    worker
        .shutdown()
        .map_err(|_| "document worker panicked during shutdown".to_owned())?;
    persistence::write_pdf_atomically(destination, &output)
        .map_err(|error| format!("{}: {error}", destination.display()))?;
    updates
        .send_blocking(EditorUpdate::Saved(destination.to_path_buf()))
        .map_err(|error| error.to_string())?;
    load(destination, page_index, updates).map(Some)
}

fn receive(worker: &DocumentWorker) -> Result<DocumentEvent, String> {
    worker
        .events()
        .recv_timeout(Duration::from_secs(30))
        .map_err(|error| format!("document worker timeout: {error}"))
}

fn expect_opened(event: DocumentEvent) -> Result<DocumentMetadata, String> {
    match event {
        DocumentEvent::Opened(metadata) => Ok(metadata),
        DocumentEvent::Failed { error, .. } => Err(error.to_string()),
        event => Err(format!("unexpected open event: {event:?}")),
    }
}

fn expect_page_metadata(event: DocumentEvent) -> Result<PageMetadata, String> {
    match event {
        DocumentEvent::PageMetadata(metadata) => Ok(metadata),
        DocumentEvent::Failed { error, .. } => Err(error.to_string()),
        event => Err(format!("unexpected metadata event: {event:?}")),
    }
}

fn expect_rendered(event: DocumentEvent) -> Result<RenderedPage, String> {
    match event {
        DocumentEvent::PageRendered { page, .. } => Ok(page),
        DocumentEvent::Failed { error, .. } => Err(error.to_string()),
        event => Err(format!("unexpected render event: {event:?}")),
    }
}

fn expect_text(event: DocumentEvent) -> Result<String, String> {
    match event {
        DocumentEvent::TextReady { text, .. } => Ok(text),
        DocumentEvent::Failed { error, .. } => Err(error.to_string()),
        event => Err(format!("unexpected text event: {event:?}")),
    }
}

fn expect_forms(event: DocumentEvent) -> Result<Vec<pdf_engine::FormField>, String> {
    match event {
        DocumentEvent::FormFields(forms) => Ok(forms),
        DocumentEvent::Failed { error, .. } => Err(error.to_string()),
        event => Err(format!("unexpected forms event: {event:?}")),
    }
}

fn expect_fragments(event: DocumentEvent) -> Result<Vec<pdf_engine::TextFragment>, String> {
    match event {
        DocumentEvent::TextFragments { fragments, .. } => Ok(fragments),
        DocumentEvent::Failed { error, .. } => Err(error.to_string()),
        event => Err(format!("unexpected text fragments event: {event:?}")),
    }
}

fn expect_exported(event: DocumentEvent) -> Result<Vec<u8>, String> {
    match event {
        DocumentEvent::Exported(bytes) => Ok(bytes),
        DocumentEvent::Failed { error, .. } => Err(error.to_string()),
        event => Err(format!("unexpected export event: {event:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdf_engine::PdfEngine;

    #[test]
    fn opens_fixture_and_sends_loaded_update() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sample.pdf");
        std::fs::write(&path, test_support::form_pdf()).unwrap();
        let (sender, receiver) = async_channel::unbounded();

        load(&path, 0, &sender).unwrap();

        assert!(matches!(
            receiver.recv_blocking().unwrap(),
            EditorUpdate::Opened(opened) if opened.forms.len() == 3 && opened.pages.len() == 1
        ));
        assert!(matches!(
            receiver.recv_blocking().unwrap(),
            EditorUpdate::PageLoaded(page) if page.page.index == 0
        ));
    }

    #[test]
    fn saving_over_the_source_file_keeps_a_readable_pdf() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sample.pdf");
        std::fs::write(&path, test_support::form_pdf()).unwrap();
        let (sender, receiver) = async_channel::unbounded();

        let edits = vec![pdf_engine::EditCommand::FillForm {
            name: "customer.name".to_owned(),
            value: "Ada".to_owned(),
        }];
        save(&path, &path, 0, edits, &sender).unwrap();

        assert!(
            receiver
                .recv_blocking()
                .is_ok_and(|update| matches!(update, EditorUpdate::Saved(_)))
        );
        let reopened = ZpdfEngine.open(OpenRequest::new(std::fs::read(&path).unwrap()));
        assert!(reopened.is_ok());
    }

    #[test]
    fn engine_iterates_and_renders_every_fixture_page() {
        let mut document = ZpdfEngine
            .open(OpenRequest::new(test_support::multi_page_pdf()))
            .unwrap();

        assert_eq!(
            document.metadata().page_count,
            test_support::MULTI_PAGE_COUNT
        );

        for page_index in 0..document.metadata().page_count {
            assert_eq!(
                document.page_metadata(page_index).unwrap().index,
                page_index
            );
            assert!(
                document
                    .render_page(RenderRequest {
                        page_index,
                        scale: 1.0,
                    })
                    .unwrap()
                    .is_valid()
            );
        }
    }

    #[test]
    fn session_loads_every_fixture_page() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("multi-page.pdf");
        std::fs::write(&path, test_support::multi_page_pdf()).unwrap();

        let (sender, receiver) = async_channel::unbounded();
        load(&path, 1, &sender).unwrap();

        assert!(matches!(
            receiver.recv_blocking().unwrap(),
            EditorUpdate::Opened(opened)
                if opened.pages.len() == test_support::MULTI_PAGE_COUNT
                    && opened.initial_page == 1
        ));
        let pages: Vec<_> = (0..test_support::MULTI_PAGE_COUNT)
            .map(|_| receiver.recv_blocking().unwrap())
            .collect();
        assert_eq!(
            pages
                .iter()
                .filter(|update| matches!(update, EditorUpdate::PageLoaded(_)))
                .count(),
            test_support::MULTI_PAGE_COUNT
        );
        assert!(matches!(
            &pages[0],
            EditorUpdate::PageLoaded(page)
                if page.page.index == 1
                    && page.rendered.is_valid()
                    && page.text.contains("Fixture page 2")
        ));
    }
}
