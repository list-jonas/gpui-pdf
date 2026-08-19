# Implementation Plans

Greenfield product plan generated on 2026-08-19. Workspace had no source files,
Cargo manifest, tests, or Git history when this plan was written.

The requested `plan/xxx.md` location is represented by the repository-standard
`plans/` directory required by the planning workflow.

## Execution order & status

| Plan | Title | Priority | Effort | Depends on | Status |
|------|-------|----------|--------|------------|--------|
| 001 | Build local-first Acrobat alternative with Rust + GPUI | P1 | XL | — | TODO |

Status values: TODO, IN PROGRESS, DONE, BLOCKED, REJECTED.

## Dependency notes

- Plan 001 is greenfield. Its Phase 0 spike must finish before committing to a
  PDF-engine implementation or broad feature work.
- Plan 001 uses `gpui-component` for styled shell controls, dock layout, themes,
  dialogs, lists, and tables; PDF rendering remains a custom product-owned
  canvas.

## Findings considered and rejected

- Starting with true arbitrary-PDF text editing: deferred. It is the highest
  semantic risk because PDFs commonly store positioned drawing instructions,
  not editable paragraphs. Start with reader, annotations, page operations,
  forms, and safe save; design text editing as a later capability-gated slice.
- Copying the existing `gpui-pdf` component directly: rejected for the default
  product because it is GPL-3.0-or-later. Use its behavior as a reference only
  unless project licensing explicitly accepts GPL.
