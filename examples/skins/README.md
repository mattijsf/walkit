# Example skins

Two small skins the example apps show when they cannot download a skin:

- `walkit-classic.wsz`: a classic skin with the main window, equalizer and playlist editor,
  their shade bars, `pledit.txt` and `viscolor.txt`.
- `walkit-modern.wal`: a modern skin (`skin.xml` and PNGs, no scripts) with a main window
  (song title, time, song info, vis, seek and volume sliders, transport buttons, a playlist
  toggle) and a playlist window.

Both share one flat dark look with a teal accent. Every picture is original: `make-skins.py`
draws them pixel by pixel, so the skins are under the repository's license,
GPL-3.0-or-later.

To write the two archives again (Python 3, standard library only):

```sh
python3 examples/skins/make-skins.py
```

The output is the same on every run: entries are sorted and carry a fixed time.
