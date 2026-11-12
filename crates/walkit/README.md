# walkit

A Winamp skin engine, as a library. It reads a modern skin (a `.wal` file: a zip of XML, images
and compiled MAKI scripts) or a classic skin (a `.wsz` file: the bitmaps of Winamp 2), runs the
skin's scripts, lays its windows out and hands the host a draw list for each frame.

The host draws that list on whatever it has, and supplies the images, fonts, touch input and
player. The engine draws nothing, reads no file and plays no sound.

A classic skin loads the same way as a modern one: the engine wraps its bitmaps in a generated
layout. It draws the main window, the equalizer, the playlist editor, their shade bars, the
analyzer and the oscilloscope as Winamp 5.666 or Winamp 2.91 draws them.

## Features

- `truetype`: TrueType text drawn as Windows draws it, for hosts that draw their own letters
  (uses [skrifa](https://crates.io/crates/skrifa)).

## Documentation

The guide and the API reference are at <https://mattijsf.github.io/walkit/>. The source, the
Android library, the web build and an example app for each host are at
<https://github.com/mattijsf/walkit>.

## License

GPL-3.0-or-later.
