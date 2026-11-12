# walkit engineering notes

## Commands

```bash
tools/gate.sh            # the push gate: fmt, clippy -D warnings, tests
cargo test --workspace   # the tests alone
tools/web-build.sh       # the wasm modules into web/dist/, with their sizes
tools/trace-diff.sh      # compare traces/ with the last release's traces
```

The Android library builds with Gradle; [android/README.md](android/README.md) lists the
toolchain. The web build needs the `wasm32-unknown-unknown` target from rustup (`rustup target
add wasm32-unknown-unknown`). `wasm-opt` (from binaryen) is optional.

## Layout

| Path | What it is |
| --- | --- |
| `crates/walkit` | The engine. Modern skins in `runtime`, `gui` and `maki`; classic skins in `classic`; the draw list in `draw`, `plan` and `packed`; behind the `truetype` feature, TrueType letters drawn as Winamp draws them, for hosts that draw into pixels. |
| `crates/walkit-trace` | Runs a scenario on a stepped clock and writes its trace. |
| `crates/walkit-web` | The engine as a wasm module with `wk_*` exports and `host_*` imports. |
| `crates/walkit-android` | The engine as `libwalkit.so`, the native methods of `nl.mattix.walkit.Native`. |
| `android/` | The AAR `nl.mattix.walkit:walkit` and its Kotlin API. |
| `web/` | The JS host: the embedding package, the player page and the trace runners. |
| `examples/` | A small app per host: web, Android and macOS. Each builds on its own, outside the workspace and the gate. |
| `tools/` | The gate, the trace diff and the web build. |
| `docs/` | The documentation site: a Starlight site, published with the API reference to GitHub Pages. |

## Traces

`walkit-trace` writes a text trace of a scripted run: each step, the skin's log lines, menus
and host actions, each tick's work, and the draw list of every frame that changed. Text is
measured by a fixed rule and the clock is stepped, so a trace depends on no platform. A change
to the trace format changes the `trace v1` header.

How a skin should look and answer is Winamp 5.666's: a rule is settled by a screenshot or a
probe of Winamp in a virtual machine, and a test then holds it. The traces catch everything
else a change moves. Before a release:

1. Record the scenarios at the release's commit:

   ```bash
   cargo run --release -p walkit-trace -- <scenarios.json> traces \
     --skins <skins dir> --wasabi <wasabi-base.zip> --fonts <fonts dir>
   ```

2. Compare them with the traces of the last release:
   `WALKIT_REFERENCE_TRACES=<last release's traces> tools/trace-diff.sh`. Every scenario that
   differs must differ because of a change the release means to make.
3. Keep the new traces as the set the next release is compared with.

The inputs stay outside the repository: the skins are other people's work, and the traces are
tens of megabytes per run.

`--wasabi` takes a zip of Winamp's `wasabi` folder, made as the guide's
[wasabi base](https://mattijsf.github.io/walkit/wasabi-base/) page shows. `--fonts` takes
`examples/skins/fonts`: DejaVu Sans Condensed and Liberation Sans, which stand in for Windows'
system fonts. Without Liberation Sans, text set in Arial lands on different baselines.

[web/README.md](web/README.md) runs the same scenarios through the wasm module in node and in a
browser, and [android/README.md](android/README.md) runs them through the AAR on a device.

## Releases

[release-please](https://github.com/googleapis/release-please) makes the releases
(`.github/workflows/release.yml`). On every push to `main` it opens or updates a release pull
request from the Conventional Commits since the last tag: before 1.0 a `fix` or `feat` raises the
patch version and a breaking change (`feat!:`) the minor one. The pull request sets the next
version everywhere it is written (the `extra-files` in `release-please-config.json`): the
workspace `version` in `Cargo.toml`, both `Cargo.lock`s, the AAR's version and its
`nl.mattix.walkit:walkit` coordinates, and the macOS example's `MARKETING_VERSION`. It also
writes `CHANGELOG.md`. Merging it tags `vX.Y.Z` and makes the GitHub release. Before merging, run
the trace comparison above.

The lines release-please bumps by text carry an `x-release-please-version` marker (in
`android/README.md`, a start/end block). `xcodegen generate` drops the two in `project.pbxproj`:
put them back. `tools/check-release-versions.sh`, part of the gate, fails when a marker is
missing or a version differs from `.release-please-manifest.json`.

### Publishing the crate

The `walkit` crate on crates.io carries the release's version. It holds only the engine's
sources, its README and the license (`include` in `crates/walkit/Cargo.toml`). The tests' fonts
stay in the repository. When the repository has a `CARGO_REGISTRY_TOKEN` secret, the release
workflow publishes the crate from the new tag. Without it, publish by hand at the tag:

```bash
cargo publish --dry-run -p walkit
cargo publish -p walkit
```

A published version cannot be replaced, only yanked. The other crates are `publish = false`.

## Rules

- Every source file starts with `// SPDX-License-Identifier: GPL-3.0-or-later`.
- The engine crate forbids unsafe code. Only `walkit-web` and `walkit-android` use it, for
  their exports, imports and handles.
- Scripts are untrusted input. Every loop, stack and allocation in the VM has a limit. A
  handler that runs too long or too deep is stopped and logged, and a script that fails 20
  times is switched off.
- The engine has no file, screen, font, clock or thread access. Those come from the host.
- Every public item has a doc comment (`#![warn(missing_docs)]`), and clippy's pedantic set is
  on for the workspace (`[workspace.lints]` in `Cargo.toml`).
- Run `tools/gate.sh` before every push. The lefthook pre-push hook runs it, and CI
  (`.github/workflows/ci.yml`) runs it only when started by hand.
- Commit subjects follow Conventional Commits; the lefthook commit-msg hook and the Commits
  workflow check them (`tools/check-commit-message.sh`).
