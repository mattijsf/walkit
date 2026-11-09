# Contributing

## Commands

```bash
tools/gate.sh            # the push gate: fmt, clippy -D warnings, tests
cargo test --workspace   # the tests alone
tools/web-build.sh       # the wasm modules into web/dist/, with their sizes
tools/trace-diff.sh      # compare traces/ with the reference engine's traces
```

## Layout

| Path | What it is |
| --- | --- |
| `crates/walkit` | The engine. |
| `crates/walkit-trace` | Runs a scenario on a stepped clock and writes its trace. |
| `crates/walkit-web` | The engine as a wasm module with `wk_*` exports and `host_*` imports. |
| `crates/walkit-android` | The engine as `libwalkit.so`, the native methods of `nl.mattix.walkit.Native`. |
| `android/` | The AAR `nl.mattix.walkit:walkit` and its Kotlin API. |
| `web/` | The JS host: the embedding package, the player page and the trace runners. |
| `tools/` | The gate, the trace diff and the web build. |
| `docs/` | This site. |

## Rules

- Every source file starts with `// SPDX-License-Identifier: GPL-3.0-or-later`.
- The engine crate forbids unsafe code. Only `walkit-web` and `walkit-android` use it, for
  their exports, imports and handles.
- Scripts are untrusted input. Every loop, stack and allocation in the VM has a limit.
- The engine has no file, screen, font, clock or thread access. Those come from the host.
- Every public item has a doc comment (`#![warn(missing_docs)]`), and clippy's pedantic set is
  on for the workspace.
- Run `tools/gate.sh` before every push. The lefthook pre-push hook runs it.
- Commit subjects follow Conventional Commits.

## This site

The guide is an [mdBook](https://rust-lang.github.io/mdBook/) in `docs/`, and the API
reference is rustdoc. To build both:

```bash
mdbook build docs
cargo doc -p walkit --no-deps
cp -r target/doc docs/book/api
```

A push to `main` that touches `docs/` or the engine publishes them to GitHub Pages.
