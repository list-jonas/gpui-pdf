# PDF compatibility

Phase 0 corpus is generated in `test-support` and checked through the public engine adapter.

| Fixture | Current evidence |
|---|---|
| Text | Opens, extracts expected text, renders valid RGBA |
| Image/scanned-style | Opens and renders expected dimensions |
| Rotated | Reports 90° and renders swapped dimensions |
| Encrypted | Requires password, rejects wrong password, accepts correct password |
| Malformed | Returns structured failure without panic |
| AcroForm | Enumerates and round-trips text-field values |
| Added text | Saves a page-space text overlay and reopens with extractable text |
| Redaction | Removes intersecting fixture text, fresh-rewrites, reopens, and renders |

Known limits:

- zpdf 0.12.1 does not expose page `/UserUnit` through `PdfPage`; the adapter currently reports `1.0`.
- Text extraction evidence does not prove stable source-object mapping or selection quads.
- Text, button, and choice fields use a common string editor. Signature fields are read-only.
- Added text is a flattened overlay, not arbitrary editing of existing PDF text objects.
- Redaction removes matching page operators and annotations, but does not descend into every Form XObject or sanitize unrelated metadata, attachments, and alternate content. Verify security-sensitive output independently.
- Redaction of encrypted PDFs is rejected instead of silently removing encryption.
- Cross-platform GPUI builds and an independent-reader render comparison remain pending.
- Password entry is engine-tested but not exposed in the editor.
