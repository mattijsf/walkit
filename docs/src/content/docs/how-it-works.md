---
title: How it works
---

```text
 skin files ──► walkit engine ──► draw list ──► host renderer
                  ▲      │
 host services ───┘      └──► player commands, host actions
 (pictures, fonts,
  clock, input, player)
```

## The engine

`crates/walkit` holds the engine:

- `runtime`: a loaded skin, its windows, its timers and the glue between the player and the
  skin's objects.
- `gui`: the skin's objects (layers, buttons, sliders, text, lists and the rest).
- `maki`: the MAKI virtual machine that runs the skin's compiled scripts.
- `classic`: the generated layout around a classic skin's bitmaps.
- `draw`, `plan` and `packed`: the draw list, the per-frame plan of what changes, and the packed
  form that crosses a language boundary in one buffer.

## The host

The engine has no file, screen, font, clock or thread access. The host supplies them:

- **Files.** The host opens the skin archive and hands the engine its files. A modern skin sits
  over Winamp's [wasabi base](/walkit/wasabi-base/) when the host has it, a classic skin over a
  [base skin](/walkit/base-skin/) that gives the bitmaps it leaves out.
- **Pictures and text.** The engine asks for a picture's size and pixels, and for a text's width
  and line metrics. The host decodes and measures with whatever libraries it has; a Rust
  host can take the engine's own [TrueType letters](/walkit/rust/#text), drawn as Winamp draws
  them.
- **The clock.** Every timer, animation and script reads one clock, in milliseconds. A host
  passes the wall clock; a test steps one so that every run lands on the same frames.
- **The player.** The engine reads the player's state (status, position, title, playlist,
  equalizer, visualizer data) and sends it commands (play, pause, seek, volume and the rest).
- **Input.** The host passes pointer down, move and up, and double taps, in viewport pixels.

## A frame

The host calls `tick` regularly. When something changed, `tick` returns true and the host asks
for the frame: each window with its rect and the draw commands inside it. The host draws them
and waits for the next tick.

The engine runs on the thread that calls it, one call at a time.

## The wasabi base

Winamp keeps a folder of shared skin parts, the wasabi base, that modern skins borrow frames,
scrollbars and lists from. A host that has it lays it under the skin. walkit doesn't ship it:
[The wasabi base](/walkit/wasabi-base/) says when a skin needs it and how to prepare it.
