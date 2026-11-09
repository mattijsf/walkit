# Trace parity

walkit is a port of the Kotlin engine in the `:modernskin` module of
[Andamp](https://github.com/mattijsf/andamp). Both engines write a text trace of a scripted
run: each step, the skin's log lines, menus and host actions, each tick's work, and the draw list
of every frame that changed.

A scenario passes when both traces are identical. A change to the trace format changes the
`trace v1` header on both sides.

## Checking parity

1. In an Andamp checkout, run `EngineTraceTest`. It writes the reference traces to
   `modernskin/build/traces`.
2. Record the same scenarios with walkit:

   ```bash
   cargo run --release -p walkit-trace -- <scenarios.json> traces \
     --skins <skins dir> --defaults <wasabi-defaults.zip> --fonts <fonts dir>
   ```

3. Compare them:

   ```bash
   WALKIT_REFERENCE_TRACES=<andamp>/modernskin/build/traces tools/trace-diff.sh
   ```

## Inputs

The inputs stay outside the repository: the skins are other people's work, and the traces are
tens of megabytes per run.

`--defaults` takes a zip of Winamp's `wasabi` folder. `--fonts` takes Andamp's default-set fonts
folder (`modernskin/src/main/resources/nl/mattix/andamp/modernskin/defaultset/fonts`). It holds
DejaVu Sans Condensed and Liberation Sans, which stand in for Windows' system fonts. Without
Liberation Sans, text set in Arial lands on different baselines.

## Other targets

The same scenarios run through the wasm module in node and in a browser, and through the AAR on
an Android device. Their traces must match the desktop's byte for byte. The repository's
[web/README.md](https://github.com/mattijsf/walkit/blob/main/web/README.md) and
[android/README.md](https://github.com/mattijsf/walkit/blob/main/android/README.md) show how.
