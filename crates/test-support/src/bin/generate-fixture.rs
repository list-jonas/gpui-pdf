use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("form-fixture.pdf"), PathBuf::from);
    std::fs::write(path, test_support::form_pdf())
}
