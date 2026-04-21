# Contributing to 8v

Thank you for your interest in contributing to 8v.

## How to contribute

1. Open an issue describing what you want to change and why
2. Fork the repository and create a branch
3. Make your changes — requires Rust 1.82.0 or later (`rustup update stable`)
4. Run `cargo test --workspace && cargo clippy --workspace && cargo fmt --all --check`
5. Open a pull request

## Rules

- Every warning is an error. `cargo clippy -- -D warnings` must pass.
- Tests are production code. Same quality standards as application crates.
- No silent fallbacks. Every error must be visible.
- Run `8v check .` before submitting. It must pass on itself.

## What we accept

- Bug fixes with regression tests
- Parser improvements for supported tools
- New stack support with tests and fixtures
- Documentation corrections

## What we don't accept

- Features without a corresponding issue
- Changes that add complexity without proven user need
- Suppressions (`#[allow(...)]`, `allow-file()`) to hide problems

## License

By contributing, you agree that your contributions will be licensed under the same license as the crate you're modifying:

- **o8v-fs** and **o8v-process**: MIT License
- **All other crates**: Business Source License 1.1 (BSL-1.1)
