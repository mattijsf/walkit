---
title: Traces
---

`walkit-trace` runs a scenario on a stepped clock and writes a text trace of the run: each step,
the skin's log lines, menus and host actions, each tick's work, and the draw list of every frame
that changed. Text is measured by a fixed rule and the clock is stepped, so a trace depends on no
platform. A change to the trace format changes the `trace v1` header.

## What a skin should do

How a skin should look and answer is Winamp 5.666's. A rule is settled by a screenshot or a
probe of Winamp in a virtual machine, and a test then holds it. The traces catch everything else
a change moves.

## Before a release

1. Record the scenarios at the release's commit:

   ```bash
   cargo run --release -p walkit-trace -- <scenarios.json> traces \
     --skins <skins dir> --wasabi <wasabi-base.zip> --fonts <fonts dir>
   ```

2. Compare them with the traces of the last release:

   ```bash
   WALKIT_REFERENCE_TRACES=<last release's traces> tools/trace-diff.sh
   ```

   Every scenario that differs must differ because of a change the release means to make.

3. Keep the new traces as the set the next release is compared with.

## Inputs

The inputs stay outside the repository: the skins are other people's work, and the traces are
tens of megabytes per run.

`--wasabi` takes a zip of Winamp's `wasabi` folder, made as
[The wasabi base](/walkit/wasabi-base/#make-the-zip) shows. `--fonts` takes
`examples/skins/fonts` from the repository: DejaVu Sans Condensed and Liberation Sans, which
stand in for Windows' system fonts. Without Liberation Sans, text set in Arial lands on
different baselines.

## Other targets

The same scenarios run through the wasm module in node and in a browser, and through the AAR on
an Android device. Their traces must match the desktop's byte for byte at the same commit. The
repository's [web/README.md](https://github.com/mattijsf/walkit/blob/main/web/README.md) and
[android/README.md](https://github.com/mattijsf/walkit/blob/main/android/README.md) show how.
