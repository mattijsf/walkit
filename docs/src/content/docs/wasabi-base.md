---
title: The wasabi base
description: What Winamp's wasabi folder is, when a skin needs it, and how to make the zip walkit reads.
---

Winamp keeps a folder of shared skin parts next to its own files: window frames, scrollbars,
buttons, list styles, and the standard groups behind tags such as `Wasabi:Button`. Modern skins
borrow from it instead of drawing every part themselves, and Winamp lays it under every modern
skin it loads. walkit calls this folder the **wasabi base**.

It is Winamp's art, so walkit doesn't ship it. A host that wants it supplies it as a zip.

## Do you need it?

Often not.

- **Classic skins** (`.wsz`) never use it. They take what they leave out from a classic
  [base skin](/walkit/base-skin/) instead.
- **Many modern skins** (`.wal`) draw everything themselves. Winamp Modern does, and so do the
  skins in the [examples](/walkit/examples/).
- **Some modern skins** borrow parts from it, such as a list's scrollbar or a window's frame.
  Without the wasabi base, those parts are simply missing: the rest of the skin still shows.

When a skin looks incomplete, try it again with the wasabi base.

## Get the folder

The folder comes with every Winamp 5 install, at:

```text
Winamp\Plugins\Freeform\xml\wasabi
```

On Windows, Winamp usually sits in `C:\Program Files (x86)\Winamp`. The folder holds a file
`wasabi.xml` and subfolders such as `xml`, `window` and `Scripts`.

Copy the whole `wasabi` folder to the machine you build on. Nothing else from Winamp is needed.

## Make the zip

walkit reads the folder's **contents** from the top of the zip. Zip what is inside `wasabi`,
not the folder itself: a zip whose first level is a single `wasabi` folder doesn't work, and
gives no error.

**macOS and Linux**, in a terminal:

```bash
cd path/to/wasabi
zip -r ../wasabi-base.zip .
```

**Windows**, in PowerShell:

```powershell
Compress-Archive -Path path\to\wasabi\* -DestinationPath wasabi-base.zip
```

**In Finder or Explorer**, open the `wasabi` folder, select everything in it, and compress the
selection. Compressing the folder itself wraps it one level too deep.

### Check it

Open the zip, or list it:

```bash
unzip -l wasabi-base.zip | grep " xml/system-elements.xml"
```

The file `xml/system-elements.xml` must sit at the top of the zip, not under `wasabi/`. The name
of the zip doesn't matter.

## Use it

Each host lays the zip's files under the skin. The engine then finds them at paths starting
with `wasabi-base/`.

| Host | How |
| --- | --- |
| Rust | Unzip into a [`MemoryFiles`](/walkit/api/walkit/files/struct.MemoryFiles.html) and pass it as the base of [`LayeredFiles`](/walkit/api/walkit/files/struct.LayeredFiles.html) ([guide](/walkit/rust/#files)). |
| Web | `walkit.openSkin(bytes, { wasabi })`, with the zip's bytes or its URL ([guide](/walkit/web/#embedding)). |
| Android | `LayeredSkinFiles(skinFiles, ZipSkinFiles(File("wasabi-base.zip")))` ([guide](/walkit/android/#using-it)). |
| Command line | `--wasabi wasabi-base.zip` on `walkit-trace`, `web/node.mjs` and `web/serve.mjs` ([traces](/walkit/traces/)). |
| Examples | The "Wasabi base" button on the web and Android, `--wasabi` on macOS ([examples](/walkit/examples/)). |

## Fonts are separate

The wasabi base holds no fonts. Skins that name Arial or Tahoma get free stand-ins that walkit
reads from `fonts/` in the skin's files: Liberation Sans for Arial and DejaVu Sans Condensed for
Tahoma. The repository has them, with their licenses, in
[`examples/skins/fonts`](https://github.com/mattijsf/walkit/tree/main/examples/skins/fonts).
A host puts them under the skin the same way, at `fonts/LiberationSans.ttf` and the like.
