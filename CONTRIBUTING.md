# Contributing

Use stable Rust as pinned in `rust-toolchain.toml`. The engine integration tests are in `crates/engine/tests`. Larger fixture sources and their build commands live in `tests/fixtures`. Extend the engine first, then expose the behavior through the CLI and desktop where applicable.

Run the checks below before submitting changes. In a test, identify the actual bytes or source program, expected behavior, and failure condition. For a research-derived change, record an observation or link in [the reference inventory](docs/REFERENCE_INVENTORY.md). See [SECURITY.md](SECURITY.md) for private vulnerability reporting.

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --release --locked
```
