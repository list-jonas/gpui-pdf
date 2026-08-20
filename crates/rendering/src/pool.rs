use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};

use pdf_engine::{
    EngineError, OpenRequest, Password, PdfDocument, PdfEngine, RenderRequest, RenderedPage,
    TextFragment,
};

/// What a queued job produces. Previews are cheap, full-quality rasters are
/// not, and text extraction only feeds search and selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobKind {
    Preview,
    Sharp,
    Text,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderJob {
    pub page_index: usize,
    pub scale: f32,
    pub kind: JobKind,
    /// Lower runs first. Callers use viewport distance so the pages a reader is
    /// actually looking at never queue behind the rest of the document.
    pub priority: u32,
}

#[derive(Debug)]
pub enum PoolEvent {
    Rendered {
        page_index: usize,
        scale: f32,
        kind: JobKind,
        page: RenderedPage,
    },
    Text {
        page_index: usize,
        text: String,
        fragments: Vec<TextFragment>,
    },
    Failed {
        page_index: usize,
        error: EngineError,
    },
    /// No jobs left for the current document generation.
    Idle,
}

struct Queue {
    jobs: Vec<RenderJob>,
    generation: u64,
    active: usize,
}

struct Shared {
    queue: Mutex<Queue>,
    ready: Condvar,
    shutdown: AtomicBool,
    generation: AtomicU64,
}

/// A small pool of independent documents that rasterise pages in parallel.
///
/// Each worker parses its own copy of the file (sharing the underlying bytes),
/// because a parsed document owns interior-mutable caches and cannot be shared
/// across threads.
pub struct RenderPool {
    shared: Arc<Shared>,
    threads: Vec<JoinHandle<()>>,
}

impl RenderPool {
    pub fn spawn<F>(
        engine: &Arc<dyn PdfEngine>,
        bytes: &Arc<[u8]>,
        password: Option<&[u8]>,
        workers: usize,
        sink: F,
    ) -> Self
    where
        F: Fn(PoolEvent) + Send + Sync + 'static,
    {
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue {
                jobs: Vec::new(),
                generation: 0,
                active: 0,
            }),
            ready: Condvar::new(),
            shutdown: AtomicBool::new(false),
            generation: AtomicU64::new(0),
        });
        let sink = Arc::new(sink);
        let threads = (0..workers.max(1))
            .map(|_| {
                let shared = Arc::clone(&shared);
                let engine = Arc::clone(engine);
                let bytes = Arc::clone(bytes);
                let password = password.map(<[u8]>::to_vec);
                let sink = Arc::clone(&sink);
                thread::spawn(move || {
                    let mut request = OpenRequest::new(bytes);
                    if let Some(password) = password {
                        request = request.with_password(Password::new(password));
                    }
                    match engine.open(request) {
                        Ok(document) => run(&shared, document, sink.as_ref()),
                        Err(error) => sink(PoolEvent::Failed {
                            page_index: 0,
                            error,
                        }),
                    }
                })
            })
            .collect();
        Self { shared, threads }
    }

    /// Replaces any queued work whose page is also in `jobs`, keeping the
    /// queue small when a reader scrolls or zooms faster than pages render.
    pub fn submit(&self, jobs: &[RenderJob]) {
        if jobs.is_empty() {
            return;
        }
        let mut queue = lock(&self.shared.queue);
        for job in jobs {
            queue
                .jobs
                .retain(|queued| queued.page_index != job.page_index || queued.kind != job.kind);
            queue.jobs.push(*job);
        }
        drop(queue);
        self.shared.ready.notify_all();
    }

    /// Drops every queued job. In-flight jobs finish but their results carry the
    /// previous generation, so callers can ignore them.
    pub fn cancel_pending(&self) {
        let mut queue = lock(&self.shared.queue);
        queue.jobs.clear();
        queue.generation = self.shared.generation.fetch_add(1, Ordering::AcqRel) + 1;
    }

    /// Drops queued rasterisation work but keeps background text extraction,
    /// so scrolling never restarts the whole-document text pass that search
    /// and selection depend on.
    pub fn cancel_pending_renders(&self) {
        let mut queue = lock(&self.shared.queue);
        queue.jobs.retain(|job| job.kind == JobKind::Text);
    }

    pub fn pending(&self) -> usize {
        let queue = lock(&self.shared.queue);
        queue.jobs.len() + queue.active
    }
}

/// A panicking worker must not wedge the queue, so poisoning is recovered from.
fn lock(queue: &Mutex<Queue>) -> MutexGuard<'_, Queue> {
    queue.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Drop for RenderPool {
    fn drop(&mut self) {
        self.shared.shutdown.store(true, Ordering::Release);
        self.shared.ready.notify_all();
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

fn run(
    shared: &Arc<Shared>,
    mut document: Box<dyn PdfDocument>,
    sink: &(impl Fn(PoolEvent) + ?Sized),
) {
    loop {
        let Some(job) = next_job(shared) else {
            return;
        };
        let event = match job.kind {
            JobKind::Text => match text_for(document.as_mut(), job.page_index) {
                Ok(event) => event,
                Err(error) => PoolEvent::Failed {
                    page_index: job.page_index,
                    error,
                },
            },
            JobKind::Preview | JobKind::Sharp => match document.render_page(RenderRequest {
                page_index: job.page_index,
                scale: job.scale,
            }) {
                Ok(page) => PoolEvent::Rendered {
                    page_index: job.page_index,
                    scale: job.scale,
                    kind: job.kind,
                    page,
                },
                Err(error) => PoolEvent::Failed {
                    page_index: job.page_index,
                    error,
                },
            },
        };
        sink(event);
        let drained = finish_job(shared);
        if drained {
            sink(PoolEvent::Idle);
        }
    }
}

fn text_for(document: &mut dyn PdfDocument, page_index: usize) -> Result<PoolEvent, EngineError> {
    let text = document.extract_text(page_index)?;
    let fragments = document.text_fragments(page_index)?;
    Ok(PoolEvent::Text {
        page_index,
        text,
        fragments,
    })
}

fn next_job(shared: &Arc<Shared>) -> Option<RenderJob> {
    let mut queue = lock(&shared.queue);
    loop {
        if shared.shutdown.load(Ordering::Acquire) {
            return None;
        }
        if let Some(position) = best_job(&queue.jobs) {
            let job = queue.jobs.swap_remove(position);
            queue.active += 1;
            return Some(job);
        }
        queue = shared
            .ready
            .wait(queue)
            .unwrap_or_else(PoisonError::into_inner);
    }
}

/// True when this was the last outstanding job.
fn finish_job(shared: &Arc<Shared>) -> bool {
    let mut queue = lock(&shared.queue);
    queue.active = queue.active.saturating_sub(1);
    queue.active == 0 && queue.jobs.is_empty()
}

fn best_job(jobs: &[RenderJob]) -> Option<usize> {
    jobs.iter()
        .enumerate()
        .min_by_key(|(_, job)| (job.priority, job.page_index))
        .map(|(position, _)| position)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highest_priority_job_runs_first() {
        let jobs = vec![
            RenderJob {
                page_index: 9,
                scale: 1.0,
                kind: JobKind::Preview,
                priority: 30,
            },
            RenderJob {
                page_index: 2,
                scale: 1.0,
                kind: JobKind::Sharp,
                priority: 1,
            },
        ];

        assert_eq!(best_job(&jobs), Some(1));
    }
}
