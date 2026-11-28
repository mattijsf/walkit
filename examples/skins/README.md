# Example skins

Two small skins the example apps show when they cannot download a skin:

- `walkit-classic.wsz`: a classic skin with the main window, equalizer and playlist editor,
  their shade bars, the `gen.bmp` frame of the visualizer, library and plug-in windows,
  `genex.bmp`, `pledit.txt` and `viscolor.txt`.
- `walkit-modern.wal`: a modern skin (`skin.xml`, PNGs and one script), every window 300
  pixels wide for phone screens: a main window (song title, time, song info, vis, seek and
  volume sliders, transport buttons, equalizer and playlist toggles), an equalizer (with a
  presets button, and +12, 0 and -12 labels that set every band), a playlist,
  a visualizer window that holds the host's visualizer, and a media library window. The
  playlist, visualizer and media library resize from their bottom right corner. It also
  replaces the wasabi base's frame, title bar, buttons, scroll bars and list colors, so the
  windows Winamp frames itself match the skin.

Both share one flat dark look with a teal accent. Every picture is original: `make-skins.py`
draws them pixel by pixel, so the skins are under the repository's license,
GPL-3.0-or-later.

`fonts/` holds the free fonts that stand in for Windows' Arial and Tahoma: Liberation Sans
(metric-compatible with Arial, SIL Open Font License) and DejaVu Sans Condensed (Bitstream
Vera license), with their license texts. The engine reads them from a skin's files as
`fonts/LiberationSans.ttf` and the like, so every example puts them under every skin: a classic
playlist then sets its rows in Arial's widths, as Winamp does.

To write the two archives again (Python 3, standard library only):

```sh
python3 examples/skins/make-skins.py
```

The output is the same on every run: entries are sorted and carry a fixed time.

`eq.m` is the modern equalizer's script. `eq.maki` is the same script compiled with `mc.exe`
from the Wasabi SDK (`mc.exe eq.m`, on Windows), and `make-skins.py` puts that file in the
skin as `scripts/eq.maki`. Compile it again after a change to `eq.m`.
