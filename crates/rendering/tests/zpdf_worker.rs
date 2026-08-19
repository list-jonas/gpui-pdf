use std::sync::Arc;
use std::time::Duration;

use pdf_engine::{OpenRequest, RenderRequest};
use pdf_engine_zpdf::ZpdfEngine;
use rendering::{DocumentCommand, DocumentEvent, DocumentWorker};
use test_support::text_pdf;

#[test]
fn zpdf_work_stays_on_document_worker() {
    let worker = DocumentWorker::spawn(Arc::new(ZpdfEngine), OpenRequest::new(text_pdf()));
    assert!(matches!(receive(&worker), DocumentEvent::Opened(_)));

    worker
        .send(DocumentCommand::PageMetadata { page_index: 0 })
        .unwrap();
    assert!(matches!(receive(&worker), DocumentEvent::PageMetadata(_)));

    worker
        .send(DocumentCommand::Render {
            request: RenderRequest {
                page_index: 0,
                scale: 1.0,
            },
            generation: worker.current_generation(),
        })
        .unwrap();
    assert!(matches!(
        receive(&worker),
        DocumentEvent::PageRendered { .. }
    ));

    worker
        .send(DocumentCommand::ExtractText { page_index: 0 })
        .unwrap();
    assert!(matches!(
        receive(&worker),
        DocumentEvent::TextReady { text, .. } if text.contains("Phase zero")
    ));

    worker.shutdown().unwrap();
}

fn receive(worker: &DocumentWorker) -> DocumentEvent {
    worker
        .events()
        .recv_timeout(Duration::from_secs(3))
        .unwrap()
}
