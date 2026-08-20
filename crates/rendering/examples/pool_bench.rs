//! End-to-end timing for the render pool:
//! `cargo run --release -p rendering --example pool_bench -- file.pdf`.
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Instant;

use pdf_engine::{OpenRequest, PdfEngine};
use pdf_engine_zpdf::ZpdfEngine;
use rendering::{JobKind, PoolEvent, RenderJob, RenderPool};

const PREVIEW_SCALE: f32 = 0.35;
const SHARP_SCALE: f32 = 1.5;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: pool_bench <file.pdf>");
    let bytes: Arc<[u8]> = Arc::from(std::fs::read(&path).expect("read pdf"));
    let engine: Arc<dyn PdfEngine> = Arc::new(ZpdfEngine);
    let page_count = engine
        .open(OpenRequest::new(Arc::clone(&bytes)))
        .expect("open")
        .metadata()
        .page_count;

    let workers = std::thread::available_parallelism()
        .map_or(2, std::num::NonZero::get)
        .saturating_sub(1)
        .clamp(1, 4);

    let (sender, receiver) = mpsc::channel();
    let start = Instant::now();
    let pool = RenderPool::spawn(&engine, &bytes, None, workers, move |event| {
        let _ = sender.send(event);
    });

    // What the editor asks for when a document opens on page 1.
    let mut jobs = vec![RenderJob {
        page_index: 0,
        scale: PREVIEW_SCALE,
        kind: JobKind::Preview,
        priority: 0,
    }];
    jobs.push(RenderJob {
        page_index: 0,
        scale: SHARP_SCALE,
        kind: JobKind::Sharp,
        priority: 1_000,
    });
    for page_index in 1..page_count.min(7) {
        jobs.push(RenderJob {
            page_index,
            scale: PREVIEW_SCALE,
            kind: JobKind::Preview,
            priority: 10_000 + u32::try_from(page_index).unwrap_or(u32::MAX),
        });
    }
    pool.submit(&jobs);

    let mut first_pixel = None;
    let mut first_sharp = None;
    let mut rendered = 0;
    while rendered < jobs.len() {
        match receiver.recv().expect("pool event") {
            PoolEvent::Rendered { kind, .. } => {
                rendered += 1;
                if first_pixel.is_none() {
                    first_pixel = Some(start.elapsed());
                }
                if kind == JobKind::Sharp && first_sharp.is_none() {
                    first_sharp = Some(start.elapsed());
                }
            }
            PoolEvent::Failed { error, .. } => panic!("render failed: {error}"),
            PoolEvent::Idle | PoolEvent::Text { .. } => {}
        }
    }
    let all = start.elapsed();

    println!("pages={page_count} workers={workers}");
    println!(
        "first pixels    {:>10.2?}",
        first_pixel.expect("a page rendered")
    );
    println!(
        "first sharp     {:>10.2?}",
        first_sharp.expect("a sharp page")
    );
    println!("first screenful {all:>10.2?}");
}
