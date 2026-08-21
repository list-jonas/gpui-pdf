//! Writes the large stress-test document used for manual performance checks.
//!
//! `cargo run -p test-support --bin generate-stress-pdf -- [path] [pages]`
use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("stress.pdf"), PathBuf::from);
    let pages = std::env::args()
        .nth(2)
        .and_then(|value| value.parse().ok())
        .unwrap_or(test_support::STRESS_PAGE_COUNT);
    let pdf = test_support::stress_pdf(pages);
    println!(
        "{} pages, {} KiB -> {}",
        pages,
        pdf.len() / 1024,
        path.display()
    );
    std::fs::write(path, pdf)
}
