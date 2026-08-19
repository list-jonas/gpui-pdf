use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};

use pdf_engine::{OpenRequest, PdfDocument, PdfEngine};

use crate::{DocumentCommand, DocumentEvent, Generation, Operation};

pub struct DocumentWorker {
    commands: mpsc::Sender<DocumentCommand>,
    events: mpsc::Receiver<DocumentEvent>,
    generation: Arc<AtomicU64>,
    thread: Option<JoinHandle<()>>,
}

impl DocumentWorker {
    pub fn spawn(engine: Arc<dyn PdfEngine>, request: OpenRequest) -> Self {
        let (command_sender, command_receiver) = mpsc::channel();
        let (event_sender, event_receiver) = mpsc::channel();
        let generation = Arc::new(AtomicU64::new(0));
        let worker_generation = Arc::clone(&generation);
        let thread = thread::spawn(move || {
            let document = engine.open(request);
            match document {
                Ok(document) => run(
                    document,
                    &command_receiver,
                    &event_sender,
                    &worker_generation,
                ),
                Err(error) => {
                    let _ = event_sender.send(DocumentEvent::Failed {
                        operation: Operation::Open,
                        error,
                    });
                    let _ = event_sender.send(DocumentEvent::Closed);
                }
            }
        });
        Self {
            commands: command_sender,
            events: event_receiver,
            generation,
            thread: Some(thread),
        }
    }

    pub fn events(&self) -> &mpsc::Receiver<DocumentEvent> {
        &self.events
    }

    pub fn send(&self, command: DocumentCommand) -> Result<(), mpsc::SendError<DocumentCommand>> {
        self.commands.send(command)
    }

    pub fn current_generation(&self) -> Generation {
        Generation::new(self.generation.load(Ordering::Acquire))
    }

    pub fn invalidate(&self) -> Generation {
        Generation::new(self.generation.fetch_add(1, Ordering::AcqRel) + 1)
    }

    pub fn shutdown(mut self) -> thread::Result<()> {
        let _ = self.commands.send(DocumentCommand::Shutdown);
        self.join()
    }

    fn join(&mut self) -> thread::Result<()> {
        self.thread.take().map_or(Ok(()), JoinHandle::join)
    }
}

impl Drop for DocumentWorker {
    fn drop(&mut self) {
        let _ = self.commands.send(DocumentCommand::Shutdown);
    }
}

fn run(
    mut document: Box<dyn PdfDocument>,
    commands: &mpsc::Receiver<DocumentCommand>,
    events: &mpsc::Sender<DocumentEvent>,
    current_generation: &AtomicU64,
) {
    if events
        .send(DocumentEvent::Opened(document.metadata()))
        .is_err()
    {
        return;
    }
    while let Ok(command) = commands.recv() {
        match command {
            DocumentCommand::PageMetadata { page_index } => {
                let event = match document.page_metadata(page_index) {
                    Ok(metadata) => DocumentEvent::PageMetadata(metadata),
                    Err(error) => DocumentEvent::Failed {
                        operation: Operation::PageMetadata,
                        error,
                    },
                };
                if events.send(event).is_err() {
                    return;
                }
            }
            DocumentCommand::Render {
                request,
                generation,
            } => {
                if generation.get() != current_generation.load(Ordering::Acquire) {
                    continue;
                }
                let result = document.render_page(request);
                if generation.get() != current_generation.load(Ordering::Acquire) {
                    continue;
                }
                let event = match result {
                    Ok(page) => DocumentEvent::PageRendered {
                        page_index: request.page_index,
                        generation,
                        page,
                    },
                    Err(error) => DocumentEvent::Failed {
                        operation: Operation::Render,
                        error,
                    },
                };
                if events.send(event).is_err() {
                    return;
                }
            }
            DocumentCommand::ExtractText { page_index } => {
                let event = match document.extract_text(page_index) {
                    Ok(text) => DocumentEvent::TextReady { page_index, text },
                    Err(error) => DocumentEvent::Failed {
                        operation: Operation::ExtractText,
                        error,
                    },
                };
                if events.send(event).is_err() {
                    return;
                }
            }
            DocumentCommand::FormFields => {
                let event = match document.form_fields() {
                    Ok(fields) => DocumentEvent::FormFields(fields),
                    Err(error) => DocumentEvent::Failed {
                        operation: Operation::Forms,
                        error,
                    },
                };
                if events.send(event).is_err() {
                    return;
                }
            }
            DocumentCommand::Export { edits } => {
                let event = match document.export(&edits) {
                    Ok(bytes) => DocumentEvent::Exported(bytes),
                    Err(error) => DocumentEvent::Failed {
                        operation: Operation::Export,
                        error,
                    },
                };
                if events.send(event).is_err() {
                    return;
                }
            }
            DocumentCommand::Shutdown => break,
        }
    }
    let _ = events.send(DocumentEvent::Closed);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use document_core::{EngineCapabilities, PageGeometry, PdfRect, Rotation};
    use pdf_engine::{
        DocumentMetadata, EditCommand, EngineError, FormField, OpenRequest, PageMetadata,
        PdfEditor, PdfEngine, PdfReader, PdfRenderer, RenderRequest, RenderedPage,
    };

    use super::*;

    struct FakeEngine;
    struct FakeDocument;

    impl PdfEngine for FakeEngine {
        fn capabilities(&self) -> EngineCapabilities {
            EngineCapabilities::READ.union(EngineCapabilities::RENDER)
        }

        fn open(&self, _: OpenRequest) -> Result<Box<dyn PdfDocument>, EngineError> {
            Ok(Box::new(FakeDocument))
        }
    }

    impl PdfReader for FakeDocument {
        fn metadata(&self) -> DocumentMetadata {
            DocumentMetadata {
                page_count: 1,
                pdf_version: (1, 7),
                encrypted: false,
            }
        }

        fn page_metadata(&self, page_index: usize) -> Result<PageMetadata, EngineError> {
            let rect = PdfRect::new(0.0, 0.0, 10.0, 20.0).unwrap();
            Ok(PageMetadata {
                index: page_index,
                geometry: PageGeometry::new(rect, rect, Rotation::None, 1.0).unwrap(),
            })
        }

        fn extract_text(&mut self, _: usize) -> Result<String, EngineError> {
            Ok("text".into())
        }
    }

    impl PdfRenderer for FakeDocument {
        fn render_page(&mut self, _: RenderRequest) -> Result<RenderedPage, EngineError> {
            Ok(RenderedPage::new(1, 1, vec![0, 0, 0, 255]))
        }
    }

    impl PdfEditor for FakeDocument {
        fn form_fields(&self) -> Result<Vec<FormField>, EngineError> {
            Ok(Vec::new())
        }

        fn export(&mut self, _: &[EditCommand]) -> Result<Vec<u8>, EngineError> {
            Ok(b"pdf".to_vec())
        }
    }

    #[test]
    fn worker_opens_renders_and_closes_cleanly() {
        let worker = DocumentWorker::spawn(Arc::new(FakeEngine), OpenRequest::new(vec![]));
        assert!(matches!(recv(&worker), DocumentEvent::Opened(_)));

        worker
            .send(DocumentCommand::Render {
                request: RenderRequest {
                    page_index: 0,
                    scale: 1.0,
                },
                generation: worker.current_generation(),
            })
            .unwrap();
        assert!(matches!(recv(&worker), DocumentEvent::PageRendered { .. }));

        worker.shutdown().unwrap();
    }

    #[test]
    fn stale_render_requests_are_discarded() {
        let worker = DocumentWorker::spawn(Arc::new(FakeEngine), OpenRequest::new(vec![]));
        assert!(matches!(recv(&worker), DocumentEvent::Opened(_)));
        let stale = worker.current_generation();
        worker.invalidate();

        worker
            .send(DocumentCommand::Render {
                request: RenderRequest {
                    page_index: 0,
                    scale: 1.0,
                },
                generation: stale,
            })
            .unwrap();
        worker.send(DocumentCommand::Shutdown).unwrap();

        assert!(matches!(recv(&worker), DocumentEvent::Closed));
    }

    fn recv(worker: &DocumentWorker) -> DocumentEvent {
        worker
            .events()
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
    }
}
