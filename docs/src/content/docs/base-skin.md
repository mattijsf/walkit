---
title: "Classic skins: a base skin"
description: Why a classic skin may need a base skin under it, and how each host passes one.
---

A classic skin (`.wsz`) is a zip of bitmaps: `main.bmp` for the main window, `eqmain.bmp` for
the equalizer, `pledit.bmp` for the playlist, `text.bmp` for the letters, and so on. Many skins
leave some of them out. Plenty skin only the main window.

Winamp fills every gap from its **base skin**, the classic look it ships with. A skin without
`eqmain.bmp` shows Winamp's own equalizer; a skin without cursor files shows Winamp's own
cursors. walkit does the same when the host puts a base skin under the classic skin.

## Do you need it?

Only for classic skins that leave bitmaps out. Without a base skin, the parts drawn from a
missing bitmap stay empty: the rest of the skin still shows.

Any complete classic skin works as the base. Winamp's own base skin is in the
[Winamp Skin Museum](https://skins.webamp.org/); walkit doesn't ship it, since it is Winamp's
art. The original classic skin in
[`examples/skins`](https://github.com/mattijsf/walkit/tree/main/examples/skins) is complete
and works as a base too.

The engine takes a bitmap from the base only when the skin itself lacks it. Winamp's own
stand-ins come first: a skin without `balance.bmp` shows its `volume.bmp` there, and a skin
without `nums_ex.bmp` shows its `numbers.bmp`.

The text files are never taken from the base. A skin without `pledit.txt`, `viscolor.txt` or
`region.txt` gets Winamp's playlist colors, Winamp's vis palette and plain rectangles, even
when the base skin has those files, as in Winamp. `genex.bmp` is a bitmap like the others: a
skin without it gives the media library and plug-in windows the base skin's colors
([`classic_genex_colors`](/walkit/api/walkit/struct.SkinRuntime.html#method.classic_genex_colors)).

## Use it

The base skin goes in the same place as the [wasabi base](/walkit/wasabi-base/) for a modern
skin: under the skin, where the engine reads it at paths starting with `wasabi-base/`. A host
decides by the skin's own files: a classic skin (`main.bmp` and no `skin.xml`) gets the base
skin, a modern one the wasabi base.

| Host | How |
| --- | --- |
| Rust | Unzip the base skin into a [`MemoryFiles`](/walkit/api/walkit/files/struct.MemoryFiles.html) and pass it as the base of [`LayeredFiles`](/walkit/api/walkit/files/struct.LayeredFiles.html) ([guide](/walkit/rust/#files)). |
| Web | `walkit.openSkin(bytes, { base, wasabi })`: `base` lies under a classic skin, `wasabi` under a modern one ([guide](/walkit/web/#embedding)). |
| Android | `LayeredSkinFiles(skinFiles, ZipSkinFiles(File("Winamp Base.wsz")))` ([guide](/walkit/android/#using-it)). |
| Examples | Each one downloads Winamp's base skin and puts it under every classic skin it opens ([examples](/walkit/examples/)). |

The host decodes pictures through the same layered files, so a path such as
`wasabi-base/eqmain.bmp` in the draw list reads from the base skin.
