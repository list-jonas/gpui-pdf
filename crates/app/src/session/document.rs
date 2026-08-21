use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_channel::{Receiver, Sender};
use pdf_engine::{DocumentMetadata, OpenRequest, PageMetadata, PdfEngine};
use pdf_engine_zpdf::ZpdfEngine;
use rendering::{
    DocumentCommand, DocumentEvent, DocumentWorker, JobKind, PoolEvent, RenderJob, RenderPool,
};
use ui::{EditorRequest, EditorUpdate, OpenedDocument, PageKind, PageRequest};

/// Rasterising is CPU-bound, so a few workers hide per-page latency without
/// starving the UI thread or the machine.
const MAX_RENDER_WORKERS: usize = 4;

pub fn start(requests: Receiver<EditorRequest>, updates: Sender<EditorUpdate>) {
    std::thread::spawn(move || {
        let mut open: Option<OpenDocumentSession> = None;
        let mut token = 0_u64;
        while let Ok(request) = requests.recv_blocking() {
            let result = match request {
                EditorRequest::Open(path) => {
                    token += 1;
                    // Dropping the old session stops its workers before the new
                    // document starts competing for cores.
                    open = None;
                    load(&path, 0, token, &updates).map(|session| {
                        open = Some(session);
                    })
                }
                EditorRequest::Render { replace, jobs } => {
                    render(open.as_ref(), replace, &jobs);
                    Ok(())
                }
                EditorRequest::SaveAs {
                    source,
                    destination,
                    edits,
                } => {
                    token += 1;
                    save(&source, &destination, token, edits, &updates).map(|session| {
                        open = Some(session);
                    })
                }
            };
            if let Err(error) = result {
                let _ = updates.send_blocking(EditorUpdate::Failed(error));
            }
        }
    });
}

/// Keeps the render pool alive so pages can be rasterised at new scales
/// without reparsing the file on every zoom or scroll.
pub struct OpenDocumentSession {
    pool: RenderPool,
    pages: Vec<PageMetadata>,
}

fn render(session: Option<&OpenDocumentSession>, replace: bool, jobs: &[PageRequest]) {
    let Some(session) = session else {
        return;
    };
    if replace {
        session.pool.cancel_pending_renders();
    }
    let jobs: Vec<_> = jobs
        .iter()
        .filter(|job| job.page_index < session.pages.len())
        .map(|job| RenderJob {
            page_index: job.page_index,
            scale: job.scale,
            kind: match job.kind {
                PageKind::Preview => JobKind::Preview,
                PageKind::Sharp => JobKind::Sharp,
                PageKind::Text => JobKind::Text,
            },
            priority: job.priority,
        })
        .collect();
    session.pool.submit(&jobs);
}

/// Opens the document and reports its structure immediately. Page rasters and
/// text arrive later, driven by what the reader is actually looking at.
fn load(
    path: &Path,
    page_index: usize,
    token: u64,
    updates: &Sender<EditorUpdate>,
) -> Result<OpenDocumentSession, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let bytes: Arc<[u8]> = Arc::from(bytes);
    let engine: Arc<dyn PdfEngine> = Arc::new(ZpdfEngine);
    let worker = DocumentWorker::spawn(Arc::clone(&engine), OpenRequest::new(Arc::clone(&bytes)));
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
    worker
        .shutdown()
        .map_err(|_| "document worker panicked during shutdown".to_owned())?;

    updates
        .send_blocking(EditorUpdate::Opened(Box::new(OpenedDocument {
            token,
            path: path.to_path_buf(),
            document,
            pages: pages.clone(),
            forms,
            initial_page: page_index,
        })))
        .map_err(|error| error.to_string())?;

    let pool = spawn_pool(&engine, &bytes, page_count, token, updates.clone());
    Ok(OpenDocumentSession { pool, pages })
}

fn spawn_pool(
    engine: &Arc<dyn PdfEngine>,
    bytes: &Arc<[u8]>,
    page_count: usize,
    token: u64,
    updates: Sender<EditorUpdate>,
) -> RenderPool {
    let workers = std::thread::available_parallelism()
        .map_or(2, std::num::NonZero::get)
        .saturating_sub(1)
        .clamp(1, MAX_RENDER_WORKERS)
        .min(page_count.max(1));
    RenderPool::spawn(engine, bytes, None, workers, move |event| {
        let update = match event {
            PoolEvent::Rendered {
                page_index,
                scale,
                kind,
                page,
            } => EditorUpdate::PageRendered {
                token,
                page_index,
                scale,
                kind: match kind {
                    JobKind::Sharp => PageKind::Sharp,
                    JobKind::Preview | JobKind::Text => PageKind::Preview,
                },
                rendered: page,
            },
            PoolEvent::Text {
                page_index,
                text,
                fragments,
            } => EditorUpdate::PageText {
                token,
                page_index,
                text,
                fragments,
            },
            PoolEvent::Idle => EditorUpdate::Idle { token },
            PoolEvent::Failed { error, .. } => EditorUpdate::Failed(error.to_string()),
        };
        let _ = updates.send_blocking(update);
    })
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

/// Writes the edited document and re-points the render pool at the saved
/// bytes, so annotations become part of the page rasters. The document is not
/// reopened, so the reader keeps their position, zoom and selection.
fn save(
    source: &Path,
    destination: &Path,
    token: u64,
    edits: Vec<pdf_engine::EditCommand>,
    updates: &Sender<EditorUpdate>,
) -> Result<OpenDocumentSession, String> {
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

    let saved: Arc<[u8]> = Arc::from(output);
    let engine: Arc<dyn PdfEngine> = Arc::new(ZpdfEngine);
    let worker = DocumentWorker::spawn(Arc::clone(&engine), OpenRequest::new(Arc::clone(&saved)));
    let metadata = expect_opened(receive(&worker)?)?;
    let pages = load_page_metadata(&worker, metadata.page_count)?;
    worker
        .shutdown()
        .map_err(|_| "document worker panicked during shutdown".to_owned())?;
    let pool = spawn_pool(&engine, &saved, metadata.page_count, token, updates.clone());

    updates
        .send_blocking(EditorUpdate::Saved {
            token,
            path: destination.to_path_buf(),
        })
        .map_err(|error| error.to_string())?;
    Ok(OpenDocumentSession { pool, pages })
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

fn expect_forms(event: DocumentEvent) -> Result<Vec<pdf_engine::FormField>, String> {
    match event {
        DocumentEvent::FormFields(forms) => Ok(forms),
        DocumentEvent::Failed { error, .. } => Err(error.to_string()),
        event => Err(format!("unexpected forms event: {event:?}")),
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
    use pdf_engine::RenderRequest;
    use std::time::Instant;
    use ui::PageRequest;

    fn preview(page_index: usize) -> PageRequest {
        PageRequest {
            page_index,
            scale: 0.35,
            kind: PageKind::Preview,
            priority: 0,
        }
    }

    #[test]
    fn opening_reports_structure_before_any_page_is_rendered() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sample.pdf");
        std::fs::write(&path, test_support::form_pdf()).unwrap();
        let (sender, receiver) = async_channel::unbounded();

        let session = load(&path, 0, 1, &sender).unwrap();

        assert!(matches!(
            receiver.recv_blocking().unwrap(),
            EditorUpdate::Opened(opened)
                if opened.forms.len() == 3 && opened.pages.len() == 1 && opened.token == 1
        ));
        assert!(receiver.is_empty());
        drop(session);
    }

    #[test]
    fn requested_pages_are_rendered_and_text_extracted() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("multi-page.pdf");
        std::fs::write(&path, test_support::multi_page_pdf()).unwrap();
        let (sender, receiver) = async_channel::unbounded();
        let session = load(&path, 0, 7, &sender).unwrap();
        let _ = receiver.recv_blocking().unwrap();

        let jobs: Vec<_> = (0..test_support::MULTI_PAGE_COUNT).map(preview).collect();
        render(Some(&session), true, &jobs);
        render(
            Some(&session),
            false,
            &[PageRequest {
                page_index: 0,
                scale: 1.0,
                kind: PageKind::Text,
                priority: 5,
            }],
        );

        let mut rendered = 0;
        let mut text = None;
        let deadline = Instant::now() + Duration::from_secs(10);
        while (rendered < test_support::MULTI_PAGE_COUNT || text.is_none())
            && Instant::now() < deadline
        {
            match receiver.recv_blocking().unwrap() {
                EditorUpdate::PageRendered {
                    token,
                    rendered: page,
                    ..
                } => {
                    assert_eq!(token, 7);
                    assert!(page.is_valid());
                    rendered += 1;
                }
                EditorUpdate::PageText {
                    page_index,
                    text: page_text,
                    ..
                } => {
                    assert_eq!(page_index, 0);
                    text = Some(page_text);
                }
                EditorUpdate::Failed(error) => panic!("{error}"),
                _ => {}
            }
        }

        assert_eq!(rendered, test_support::MULTI_PAGE_COUNT);
        assert!(text.unwrap().contains("Fixture page 1"));
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
        let session = save(&path, &path, 2, edits, &sender).unwrap();

        assert!(
            receiver.recv_blocking().is_ok_and(
                |update| matches!(update, EditorUpdate::Saved { token, .. } if token == 2)
            )
        );
        let reopened = ZpdfEngine.open(OpenRequest::new(std::fs::read(&path).unwrap()));
        assert!(reopened.is_ok());
        drop(session);
    }

    #[test]
    fn saving_does_not_reopen_the_document() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sample.pdf");
        std::fs::write(&path, test_support::form_pdf()).unwrap();
        let (sender, receiver) = async_channel::unbounded();

        let session = save(&path, &path, 3, Vec::new(), &sender).unwrap();

        // Only a save confirmation: an `Opened` update here would reset the
        // reader's scroll position, zoom and selection.
        let update = receiver.recv_blocking().unwrap();
        assert!(matches!(update, EditorUpdate::Saved { .. }));
        assert!(
            !matches!(update, EditorUpdate::Opened(_)),
            "saving must not reopen the document"
        );
        assert!(receiver.is_empty());
        drop(session);
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
}
