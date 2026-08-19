# Contributing

Use Rust 1.97.1 through the checked-in toolchain file. Keep engine types behind `pdf-engine`, keep GPUI out of engine and model crates, and keep changes small enough to review independently.

Run before committing:

```sh
./scripts/verify.sh
```

For dependency changes, also run:

```sh
cargo metadata --locked --format-version 1
cargo tree -d
cargo deny check licenses advisories bans sources
```

`cargo-deny` is required before a distributable build but is not installed automatically by this repository.

