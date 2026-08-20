//! Ad-hoc timing harness: `cargo run --release -p pdf-engine-zpdf --example bench -- file.pdf`.
use std::time::Instant;

use pdf_engine::{OpenRequest, PdfEngine, RenderRequest};
use pdf_engine_zpdf::ZpdfEngine;

fn main() {
    let path = std::env::args().nth(1).expect("usage: bench <file.pdf>");
    let pages: usize = std::env::args()
        .nth(2)
        .and_then(|value| value.parse().ok())
        .unwrap_or(8);
    let bytes = std::fs::read(&path).expect("read pdf");

    let start = Instant::now();
    let mut document = ZpdfEngine.open(OpenRequest::new(bytes)).expect("open");
    let open = start.elapsed();
    let page_count = document.metadata().page_count;

    let start = Instant::now();
    for index in 0..page_count {
        document.page_metadata(index).expect("metadata");
    }
    let metadata = start.elapsed();

    let start = Instant::now();
    let forms = document.form_fields().expect("forms").len();
    let form_time = start.elapsed();

    let budget = pages.min(page_count);
    let start = Instant::now();
    for index in 0..budget {
        document
            .render_page(RenderRequest {
                page_index: index,
                scale: 1.5,
            })
            .expect("render");
    }
    let render = start.elapsed();

    let start = Instant::now();
    for index in 0..budget {
        document.extract_text(index).expect("text");
        document.text_fragments(index).expect("fragments");
    }
    let text = start.elapsed();

    let start = Instant::now();
    document
        .render_page(RenderRequest {
            page_index: 0,
            scale: 3.0,
        })
        .expect("render");
    let rerender = start.elapsed();

    let mut scales = Vec::new();
    for scale in [0.2_f32, 0.35, 0.75, 1.5] {
        let start = Instant::now();
        for index in 0..budget {
            document
                .render_page(RenderRequest {
                    page_index: index,
                    scale,
                })
                .expect("render");
        }
        scales.push((scale, start.elapsed()));
    }

    println!("pages={page_count} forms={forms} sampled={budget}");
    let per_page = u32::try_from(budget.max(1)).unwrap_or(1);
    println!("open           {open:>10.2?}");
    println!("page metadata  {metadata:>10.2?}");
    println!("form fields    {form_time:>10.2?}");
    println!(
        "render @1.5    {render:>10.2?}  ({:.1?}/page)",
        render / per_page
    );
    println!(
        "text+frags     {text:>10.2?}  ({:.1?}/page)",
        text / per_page
    );
    println!("rerender @3.0  {rerender:>10.2?}");
    for (scale, elapsed) in scales {
        println!(
            "cached @{scale:<4}    {elapsed:>10.2?}  ({:.1?}/page)",
            elapsed / per_page
        );
    }
}
