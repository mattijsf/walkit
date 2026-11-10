# Web example

A page that shows a skin with the `walkit-web` package: one canvas per window, laid out apart as
on a desktop, a small player of its own, and the pointer passed to the engine. It follows the
[Web](https://mattijsf.github.io/walkit/web/) guide.

```bash
tools/web-pack.sh examples/web/walkit-web   # from the repository root
python3 -m http.server -d examples 8000
# then open http://127.0.0.1:8000/web/
```

The page opens Winamp's base skin, downloaded from the Winamp Skin Museum; "Winamp Modern"
downloads that one. When a download fails, the page shows the original skin of the same kind
in [`../skins`](../skins); "walkit classic" and "walkit modern" open those directly. The page
also hands the engine the stand-in fonts in `../skins/fonts`, which is why the server serves
`examples/` as a whole.

"Skin" opens a `.wsz` or `.wal` of your own, as does dropping one on the page. "Default set"
takes a zip of Winamp's `wasabi` folder, for a modern skin that draws lists or frames from it.

Any static server works; the page needs no build step.
