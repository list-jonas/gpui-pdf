//! Measures the cost of a whole-document text selection:
//! `cargo run --release -p pdf-engine-zpdf --example select_all -- file.pdf`.
use std::time::Instant;

use pdf_engine::{OpenRequest, PdfEngine};
use pdf_engine_zpdf::ZpdfEngine;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: select_all <file.pdf>");
    let bytes = std::fs::read(&path).expect("read pdf");
    let mut document = ZpdfEngine.open(OpenRequest::new(bytes)).expect("open");
    let page_count = document.metadata().page_count;

    let mut fragments = Vec::new();
    for index in 0..page_count {
        fragments.extend(document.text_fragments(index).expect("fragments"));
    }
    println!("pages={page_count} fragments={}", fragments.len());

    // What select-all does today: clone every fragment into a run, then join.
    let start = Instant::now();
    let runs: Vec<(usize, String)> = fragments
        .iter()
        .enumerate()
        .map(|(index, fragment)| (index, fragment.text.clone()))
        .collect();
    let clone_time = start.elapsed();

    let start = Instant::now();
    let joined: String = runs.iter().map(|(_, text)| text.as_str()).collect();
    let join_time = start.elapsed();

    println!("clone runs   {clone_time:>10.2?}");
    println!(
        "join text    {join_time:>10.2?}  ({} chars)",
        joined.chars().count()
    );
}
