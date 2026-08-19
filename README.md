# GPUI PDF

Local-first PDF application built with Rust, GPUI, gpui-component, and a replaceable PDF engine.

The editor opens and navigates PDFs, fills existing form fields, adds text
overlays, queues permanent redactions, and atomically saves an edited copy.

## Run

```sh
cargo run -p gpui-pdf -- /absolute/path/to/document.pdf
```

Use `Cmd+O` to open a document and `Cmd+S` or `Cmd+Shift+S` for Save As.

## macOS app

```sh
./scripts/package-macos.sh
open "dist/GPUI PDF.app"
```

The bundle registers as a PDF editor for Finder's Open With menu. Local builds
are ad-hoc signed; public distribution requires Developer ID signing and
notarization.

## Verify

```sh
./scripts/verify.sh
```

Current product scope and delivery phases live in [`plans/001-adobe-acrobat-alternative.md`](plans/001-adobe-acrobat-alternative.md).
