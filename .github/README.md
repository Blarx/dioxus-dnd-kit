# CI and crate publishing

`workflows/rust.yml` runs on pushes and pull requests to `main`, and supports
manual runs. `workflows/publish.yml` runs when a `v*` tag is pushed.

Both call `workflows/checks.yml`, which checks:

- GitHub Actions syntax with actionlint and Rust formatting;
- Clippy for all targets, with warnings treated as errors;
- tests for all targets and the rustdoc examples;
- rustdoc with missing documentation, broken links, and warnings treated as errors;
- the packaged crate through `cargo publish --dry-run`;
- release Web builds of all four examples using Dioxus CLI `0.7.10`.

Clippy includes the compilation checks previously performed by `cargo check`.
No registry token is passed to the shared checks.

The library does not track `Cargo.lock`. The Rust job generates it once and
uploads it as `checked-cargo-lock`. The Web and publish jobs restore that file
and use `--locked`, so they use the dependency versions that passed the checks.

Before a release, set the package version in `Cargo.toml`, finish the changelog,
and commit the changes. Push a tag matching the version exactly, such as
`v0.3.0` for version `0.3.0`. A mismatched tag fails before the release checks.
The publish job runs only after all shared checks succeed, and uses the existing
`CARGO_REGISTRY_TOKEN` repository secret. Updating the workflow files alone does
not publish a crate.

When adding an example, add it to the Web build loop in `checks.yml`. Keep the
Dioxus CLI version there aligned with the Dioxus version used to verify the examples.
