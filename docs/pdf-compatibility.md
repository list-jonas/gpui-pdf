# PDF compatibility

Phase 0 corpus is generated in `test-support` and checked through the public engine adapter.

| Fixture | Current evidence |
|---|---|
| Text | Opens, extracts expected text, renders valid RGBA |
| Image/scanned-style | Opens and renders expected dimensions |
| Rotated | Reports 90° and renders swapped dimensions |
| Encrypted | Requires password, rejects wrong password, accepts correct password |
| Malformed | Returns structured failure without panic |

Known limits:

- zpdf 0.12.1 does not expose page `/UserUnit` through `PdfPage`; the adapter currently reports `1.0`.
- Text extraction evidence does not prove stable source-object mapping or selection quads.
- Cross-platform GPUI builds and an independent-reader render comparison remain pending.
- Password entry is engine-tested but not exposed in the Phase 0 window.

