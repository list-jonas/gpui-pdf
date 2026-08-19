use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

use tempfile::NamedTempFile;

pub fn write_pdf_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "save path has no parent"))?;
    let mut temporary = NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    sync_directory(parent)
}

fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_destination_with_complete_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("edited.pdf");
        std::fs::write(&path, b"old").unwrap();

        write_pdf_atomically(&path, b"new pdf").unwrap();

        assert_eq!(std::fs::read(path).unwrap(), b"new pdf");
    }
}
