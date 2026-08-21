# GPUI PDF

Local-first PDF application built with Rust, GPUI, gpui-component, and a replaceable PDF engine.

The editor opens and navigates PDFs, fills existing form fields, adds text
overlays, queues content redactions, and atomically saves an edited copy.

## Run

```sh
cargo run -p gpui-pdf -- /absolute/path/to/document.pdf
```

Use `Cmd+O` to open a document, `Cmd+S` to save in place, and `Cmd+Shift+S`
for Save As.

## Shortcuts

| Action | Shortcut |
| --- | --- |
| Open / Save / Save As | `Cmd+O` / `Cmd+S` / `Cmd+Shift+S` |
| Undo / Redo | `Cmd+Z` / `Cmd+Shift+Z` |
| Find / Next / Previous | `Cmd+F` / `Cmd+G` / `Cmd+Shift+G` |
| Scroll | `↑` / `↓`, `Space` / `Shift+Space`, `PageUp` / `PageDown` |
| Document start / end | `Home` / `End` |
| Page back / forward | `←` / `→` (or `Cmd+←` / `Cmd+→`) |
| First / Last / Go to page | `Cmd+↑` / `Cmd+↓` / `Cmd+J` |
| Zoom in / out | `Cmd+=` / `Cmd+-` |
| Actual size / Fit page / Fit width | `Cmd+0` / `Cmd+1` / `Cmd+2` |
| Copy / Select all text | `Cmd+C` / `Cmd+A` |
| Delete selected annotation | `Delete` / `Backspace` |
| Toggle thumbnails / properties | `Cmd+Ctrl+S` / `Cmd+Alt+0` |
| Cancel current action | `Esc` |
| Tools | `V` select, `H` hand, `E` edit, `U` highlight, `L` underline, `K` strike, `T` text, `N` comment, `S` sign, `G` shape, `R` redact |

Editing and navigation keys are ignored while a text field has focus, so typing
never scrolls the page or deletes an annotation. `Esc` always works: it leaves
the field first, then clears the selection, then returns to the select tool.

Dragging with the select, highlight, underline or strike tools continues across
page boundaries, and `Cmd+A` selects the whole document.

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
