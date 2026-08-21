# Test fixtures

## `stress.pdf`

A 250-page, text-heavy document used for manual performance checks: load time,
scrolling, zoom and select-all. It contains roughly 1.3 million text runs, which
is what makes whole-document work visible.

It is generated, not downloaded, so this repository carries no third-party PDF
and everything here is ours to redistribute under the project's own licenses.
Regenerate it with:

```sh
cargo run -p test-support --bin generate-stress-pdf -- tests/fixtures/stress.pdf
```

Pass a page count as a second argument to build a larger or smaller document.

Real-world PDFs are useful for spot checks, but keep them out of the repository
unless their license clearly permits redistribution. Papers from arXiv, for
example, are usually covered by a licence that grants distribution rights to
arXiv only.
