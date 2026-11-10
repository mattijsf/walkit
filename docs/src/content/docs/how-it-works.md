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
  over Winamp's wasabi base, which the host supplies too (see below).
- **Pictures and text.** The engine asks for a picture's size and pixels, and for a text's width
  and line metrics. The host decodes and measures with whatever libraries it has.
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

## Scripts are untrusted

Every loop, stack and allocation in the VM has a limit. A handler that runs too long or too deep
is stopped and logged, and a script that fails 20 times is switched off. The engine crate
forbids unsafe code.

## The wasabi base

Winamp ships a folder of shared skin parts, `Plugins/freeform/xml/wasabi`: window frames,
scrollbars, buttons, lists and the standard groups behind tags such as `Wasabi:Button`. Modern
skins lean on it instead of drawing every part themselves, and Winamp lays it under every
modern skin it loads. walkit calls that folder the wasabi base.

A host that has it puts it under the skin, where the engine reads it at paths starting with
`wasabi-base/`: [`LayeredFiles`](/walkit/api/walkit/files/struct.LayeredFiles.html) in Rust,
`LayeredSkinFiles` on Android, the `wasabi` option of `openSkin` on the web, and `--wasabi`
on the command line tools. It is Winamp's art, so walkit doesn't ship it.

A classic skin never needs it, and many modern skins draw fine without it: Winamp Modern does.
A skin that uses a part only the wasabi base has shows nothing in that part when it is missing.

