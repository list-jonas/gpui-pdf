# gpui-pdf

Local-first PDF application built with Rust, GPUI, gpui-component, and a replaceable PDF engine.

Phase 0 provides an engine probe: it opens a PDF from the command line, parses and renders the first page on a document worker, then displays the page, metadata, and extracted text in a GPUI `Root` window.

## Run

```sh
cargo run -p gpui-pdf -- /absolute/path/to/document.pdf
```

## Verify

```sh
./scripts/verify.sh
```

Current product scope and delivery phases live in [`plans/001-adobe-acrobat-alternative.md`](plans/001-adobe-acrobat-alternative.md).

