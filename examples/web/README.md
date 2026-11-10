# Web example

A page that shows a skin with the `walkit-web` package: one canvas per window, laid out apart as
on a desktop, a small player of its own, and the pointer passed to the engine. It follows the
[Web](https://mattijsf.github.io/walkit/web/) guide.

```bash
tools/web-pack.sh examples/web/walkit-web   # from the repository root
python3 -m http.server -d examples/web 8000
# then open http://127.0.0.1:8000/
```

Pick a classic `.wsz` skin, or drop one on the page. A modern `.wal` skin also needs Winamp's
default element set: pick a zip of the `wasabi` folder under "Default set" first.

Any static server works; the page needs no build step.
