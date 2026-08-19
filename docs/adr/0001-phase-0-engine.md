# ADR 0001: Phase 0 engine and UI pins

- Status: provisionally accepted for Phase 0
- Date: 2026-08-19

## Decision

Use GPUI 0.2.2 with gpui-component 0.5.1. Enable GPUI's official `runtime_shaders` feature so local macOS builds do not require the optional Xcode Metal Toolchain. Cargo resolves one GPUI package.

Use zpdf 0.12.1 at release commit `9537f457b15e22c7c9c21827ec12f254b7644c22`, CPU rendering only. Keep it behind `pdf-engine` and inside one document worker.

## Evidence

- All required format, check, test, and strict Clippy commands pass.
- Text, image, rotation, encryption, malformed-input, worker, and app-probe tests pass.
- GPUI and gpui-component declare Apache-2.0; zpdf declares MIT.
- GPUI has no duplicate package or mixed source in the dependency graph.

## Consequences

zpdf is accepted for continued reader work, not yet for public-alpha save, redaction, signature, or arbitrary text-editing claims. Those capabilities need round-trip and independent-reader evidence.
