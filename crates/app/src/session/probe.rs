use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_channel::Receiver;
use pdf_engine::{DocumentMetadata, OpenRequest, PageMetadata, RenderRequest, RenderedPage};
use pdf_engine_zpdf::ZpdfEngine;
use rendering::{DocumentCommand, DocumentEvent, DocumentWorker};
use ui::{PagePreview, ProbeUpdate};

pub fn start(path: Option<String>) -> Receiver<ProbeUpdate> {
    let (sender, receiver) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let update = path.map_or_else(
            || ProbeUpdate::Failed("Pass a PDF path as the first argument".into()),
            |path| probe(Path::new(&path)).unwrap_or_else(ProbeUpdate::Failed),
        );
        let _ = sender.send_blocking(update);
    });
    receiver
}

fn probe(path: &Path) -> Result<ProbeUpdate, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let worker = DocumentWorker::spawn(Arc::new(ZpdfEngine), OpenRequest::new(bytes));
    let document = expect_opened(receive(&worker)?)?;

    worker
        .send(DocumentCommand::PageMetadata { page_index: 0 })
        .map_err(|error| error.to_string())?;
    let page = expect_page_metadata(receive(&worker)?)?;

    worker
        .send(DocumentCommand::Render {
            request: RenderRequest {
                page_index: 0,
                scale: 1.0,
            },
            generation: worker.current_generation(),
        })
        .map_err(|error| error.to_string())?;
    let rendered = expect_rendered(receive(&worker)?)?;

    worker
        .send(DocumentCommand::ExtractText { page_index: 0 })
        .map_err(|error| error.to_string())?;
    let text = expect_text(receive(&worker)?)?;
    worker
        .shutdown()
        .map_err(|_| "document worker panicked during shutdown".to_owned())?;

    Ok(ProbeUpdate::Ready {
        summary: summary(&document, &page, &rendered),
        text,
        preview: PagePreview {
            width: rendered.width,
            height: rendered.height,
            rgba: rendered.rgba,
        },
    })
}

fn receive(worker: &DocumentWorker) -> Result<DocumentEvent, String> {
    worker
        .events()
        .recv_timeout(Duration::from_secs(10))
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

fn summary(document: &DocumentMetadata, page: &PageMetadata, rendered: &RenderedPage) -> String {
    let geometry = page.geometry;
    format!(
        "PDF {}.{} · {} page(s) · {:.0}×{:.0} pt · {}° · {}×{} px",
        document.pdf_version.0,
        document.pdf_version.1,
        document.page_count,
        geometry.crop_box.width(),
        geometry.crop_box.height(),
        geometry.rotation.degrees(),
        rendered.width,
        rendered.height,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_reports_rendered_page() {
        let path = std::env::temp_dir().join(format!("gpui-pdf-probe-{}.pdf", std::process::id()));
        std::fs::write(&path, test_support::text_pdf()).unwrap();

        let update = probe(&path).unwrap();
        std::fs::remove_file(path).unwrap();

        assert!(matches!(
            update,
            ProbeUpdate::Ready {
                summary,
                text,
                preview,
            } if summary.contains("1 page(s)")
                && text.contains("Phase zero")
                && preview.width > 0
        ));
    }
}
