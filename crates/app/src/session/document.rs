use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_channel::{Receiver, Sender};
use pdf_engine::{DocumentMetadata, OpenRequest, PageMetadata, RenderRequest, RenderedPage};
use pdf_engine_zpdf::ZpdfEngine;
use rendering::{DocumentCommand, DocumentEvent, DocumentWorker};
use ui::{EditorRequest, EditorUpdate, LoadedDocument};

pub fn start(requests: Receiver<EditorRequest>, updates: Sender<EditorUpdate>) {
    std::thread::spawn(move || {
        while let Ok(request) = requests.recv_blocking() {
            let result = match request {
                EditorRequest::Open(path) => load(&path, 0, &updates),
                EditorRequest::LoadPage { path, page_index } => load(&path, page_index, &updates),
                EditorRequest::SaveAs {
                    source,
                    destination,
                    page_index,
                    edits,
                } => save(&source, &destination, page_index, edits, &updates),
            };
            if let Err(error) = result {
                let _ = updates.send_blocking(EditorUpdate::Failed(error));
            }
        }
    });
}

fn load(path: &Path, page_index: usize, updates: &Sender<EditorUpdate>) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let worker = DocumentWorker::spawn(Arc::new(ZpdfEngine), OpenRequest::new(bytes));
    let document = expect_opened(receive(&worker)?)?;
    if page_index >= document.page_count {
        return Err(format!("page {} does not exist", page_index + 1));
    }

    worker
        .send(DocumentCommand::PageMetadata { page_index })
        .map_err(|error| error.to_string())?;
    let page = expect_page_metadata(receive(&worker)?)?;
    worker
        .send(DocumentCommand::Render {
            request: RenderRequest {
                page_index,
                scale: 1.5,
            },
            generation: worker.current_generation(),
        })
        .map_err(|error| error.to_string())?;
    let rendered = expect_rendered(receive(&worker)?)?;
    worker
        .send(DocumentCommand::ExtractText { page_index })
        .map_err(|error| error.to_string())?;
    let text = expect_text(receive(&worker)?)?;
    worker
        .send(DocumentCommand::FormFields)
        .map_err(|error| error.to_string())?;
    let forms = expect_forms(receive(&worker)?)?;
    worker
        .send(DocumentCommand::TextFragments { page_index })
        .map_err(|error| error.to_string())?;
    let fragments = expect_fragments(receive(&worker)?)?;
    worker
        .shutdown()
        .map_err(|_| "document worker panicked during shutdown".to_owned())?;

    updates
        .send_blocking(EditorUpdate::Loaded(Box::new(LoadedDocument {
            path: path.to_path_buf(),
            document,
            page,
            rendered,
            text,
            fragments,
            forms,
        })))
        .map_err(|error| error.to_string())
}

fn save(
    source: &Path,
    destination: &Path,
    page_index: usize,
    edits: Vec<pdf_engine::EditCommand>,
    updates: &Sender<EditorUpdate>,
) -> Result<(), String> {
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
    load(destination, page_index, updates)
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

    #[test]
    fn opens_fixture_and_sends_loaded_update() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sample.pdf");
        std::fs::write(&path, test_support::form_pdf()).unwrap();
        let (sender, receiver) = async_channel::bounded(2);

        load(&path, 0, &sender).unwrap();

        assert!(matches!(
            receiver.recv_blocking().unwrap(),
            EditorUpdate::Loaded(loaded) if loaded.forms.len() == 3
        ));
    }
}
