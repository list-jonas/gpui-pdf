# Agent Requirements

- Follow implementation plans in `plans/` in dependency order.
- Start with code when the Rust toolchain is still being installed; run checks as soon as it becomes available.
- Keep code maintainable and split responsibilities across small, focused files.
- Follow Rust API design, ownership, error-handling, testing, formatting, and linting best practices.
- Keep comments minimal. Add them only when they explain non-obvious intent or safety constraints.
- Use Luna subagents for independent parallel work when doing so speeds up development.
- Make small, cohesive commits during implementation.
- Push completed commits when a Git remote is configured and the requested checks pass.
- Ship a valid macOS application bundle that registers as a PDF viewer and opens Finder PDF events.
- Keep common file, page, view, and editing commands in the native menu bar with keyboard shortcuts.
- Present an Acrobat-like workspace with page navigation, document canvas, tool controls, and properties.
- Support zoom, fit-page, native trackpad pinch zoom, smooth two-axis trackpad panning, and direct hand-tool panning.
- Place editable AcroForm controls over their real PDF widget rectangles.
- Perform text selection and colored highlighting against extracted PDF text geometry.
- Add text directly on the page and preview it at its saved PDF position.
- Select redaction regions directly on the page and permanently apply them during export.
- Verify saved PDFs by reopening them and testing edited content and annotations.
