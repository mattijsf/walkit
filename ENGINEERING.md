# walkit engineering notes

walkit is the engine of a modern Winamp skin (`.wal`): the skin's XML and images, its
compiled MAKI scripts, the layout of its windows, and a draw list per frame that any
renderer can draw. The engine draws nothing, reads no file and plays no sound. A host does
that: it gives the engine the skin's files, measures text, decodes images, feeds touch
input, answers the player's questions, and draws the draw list.

## Commands

```bash
tools/gate.sh                               # the push gate: fmt, clippy -D warnings, tests
cargo test --workspace                      # the tests
cargo run -p walkit-trace -- <scenarios.json> traces --skins <dir> --defaults <zip>   # record the reference scenarios
tools/trace-diff.sh                         # diff traces/ against the reference engine's
cargo build --profile small --target wasm32-unknown-unknown -p walkit   # the web build
```

## Layout

- `crates/walkit`: the engine, a library (`rlib` for Rust hosts, `cdylib` for Android and
  the web). No unsafe code (`#![forbid(unsafe_code)]`); every public item documented. Its
  modules follow the reference engine's files: `files` (the skin's files and how a host stacks
  them), `xml`, `draw` (the draw list), `host` (what the engine asks of its host), `player`,
  `gui` (what each kind of object keeps, in one arena the runtime reaches by index), `text`,
  `runtime` (loading, layout, drawing, the tick, touches and actions) and `maki` (the scripts).
- `crates/walkit-trace`: a binary that runs a set of scenarios through the engine on a
  stepped clock and writes each run's trace. Not published.
- `tools/`: the gate and the trace diff.

## The trace

The engine is ported from the Kotlin engine in Andamp's `:modernskin` module, which stays
the reference. Both engines write the same text for the same scripted run: `trace v1`, the
skin and the playlist, the clock, each step (load, settle, play, stop, tap, theme, view),
the skin's log lines, menus and host actions, each tick's work, and the full draw list of
every frame that changed. The reference engine's traces are written by its
`EngineTraceTest` into `modernskin/build/traces` of the Andamp checkout; walkit-trace writes
the same files into `traces/`; `tools/trace-diff.sh` prints the first line where each pair
parts. A scenario passes when its traces are identical.

The format is fixed by the reference engine's `EngineTrace` fixture (the Kotlin side); a
change there is a version bump (`trace v2`) on both sides.

The scenarios and the skins they run are a local set (`scenarios.json`, a folder of `.wal`
files given with `--skins`, plus Winamp's default element set as a zip given with
`--defaults`; `--fonts` names a folder with the free fonts that stand in for Windows' system
fonts). A scenario names its skin's file, its folder name, the songs, and what the run does;
the other `.wal` files in the folder are the skin's siblings. They are not in this repository
and are never committed: the skins are other people's work. Traces are not committed either
(tens of megabytes per run).

## Rules

- Every source file starts with `// SPDX-License-Identifier: GPL-3.0-or-later`.
- No unsafe code. Scripts are untrusted input: every loop, stack and allocation the VM does
  has a limit, and a skin that breaks a limit stops cleanly.
- The engine stays platform-free: no file, screen, font, clock or thread inside the crate.
  Those come in through the host trait and the clock the host passes.
- `tools/gate.sh` before every push; the lefthook pre-push hook runs it. CI
  (`.github/workflows/ci.yml`) is manual-only, to keep Actions minutes near zero.
- Commit subjects are Conventional Commits, written for someone who reads release notes.
