# Agent Requirements

## Workflow

- Follow implementation plans in `plans/` in dependency order.
- Run `./scripts/verify.sh` (fmt, check, test, clippy) before every commit; fix failures, never silence them.
- Make small, cohesive commits. Push when a remote exists and checks pass.
- Use subagents for independent parallel work when it actually speeds things up.

## Code standards

- Adhere to Rust API Guidelines and idiomatic Rust: clear ownership and borrowing, no needless `clone`/allocation, `impl Trait`/generics over boxing on hot paths, derive standard traits where sensible.
- Errors: `anyhow` at application boundaries, typed errors (`thiserror`-style enums) in library crates. No `unwrap`/`expect`/`panic!` outside tests and documented invariants.
- Workspace lints are authoritative: `unsafe_code` forbidden, clippy `all` + `pedantic` denied. No blanket `allow` without a justifying comment.
- Keep modules small and single-purpose; split files before they sprawl.
- Comments only for non-obvious intent, invariants, or PDF-spec quirks. No narration of the code.
- Tests next to the behavior they cover, plus integration tests in `tests/` for cross-crate flows; use `test-support` fixtures instead of ad-hoc PDFs.

## Architecture

- Keep engine types behind `pdf-engine`; keep GPUI out of engine and model crates.
- Document any structural decision that is hard to reverse in `docs/adr/`.

## Product requirements

- Ship a valid macOS app bundle that registers as a PDF viewer and handles Finder PDF open events.
- Keep common file, page, view, and editing commands in the native menu bar with keyboard shortcuts.
- Present an Acrobat-like workspace: page navigation, document canvas, tool controls, properties.
- Support zoom, fit-page, native trackpad pinch zoom, smooth two-axis panning, and hand-tool panning.
- Place editable AcroForm controls over their real PDF widget rectangles.
- Do text selection and colored highlighting against extracted PDF text geometry.
- Add text on the page and preview it at its saved PDF position.
- Select redaction regions on the page and apply them permanently during export.
- Verify saved PDFs by reopening them and testing edited content and annotations.
