---
title: The draw list
---

Each frame is a list of windows. Each window has a rect in viewport pixels and a list of draw
commands in the window's layout pixels, drawn in order.

| Command | What the host draws |
| --- | --- |
| `Image` | A bitmap, or the named part of it, stretched to `dst` at an opacity, with the color theme's tint when the skin has one. Nearest pixels, or filtered when `smooth` is set. |
| `Fill` | A box filled with a color at an opacity. |
| `Text` | A run of text in a font, size, color and alignment, clipped to its box. The engine places it: `x` and `baseline` say where it starts. `smooth` says to draw it smoothed, as Winamp draws a modern skin's text (see [Text](/walkit/rust/#text)). |
| `BitmapText` | Text in a skin's bitmap font, spelled out as glyphs. |
| `Clip`, `Unclip` | Clip the commands in between to a rect. |
| `WindowStart`, `WindowEnd` | A shaped window: the commands in between show only inside its region. |
| `Pixels` | A small ARGB picture stretched over a rect with nearest pixels: the visualizer, the equalizer graph, gradients. |

The [API reference](/walkit/api/walkit/draw/enum.DrawCmd.html) lists every field.

## The plan and the packed frame

A host that redraws only what changed asks for a planned frame. The plan says which stretches
of a window change from frame to frame, such as a ticker or a visualizer, and gives a version
for the rest, so a host can cache the still parts.

Across a language boundary (wasm, JNI) a frame travels as one packed buffer that carries only the
commands that changed since the previous frame. The web and Android hosts decode it into
command objects.

## The text form

The engine also writes the draw list as text, one command per line, the format of a
[trace](/walkit/traces/):

```text
fill 0,0,864,1632 #000000 a=255
image window/window.png 100,23,5,5 -> 0,18,5,5 a=255
text "A song title (3:45)" font=- size=13 #93afb9 a=255 88,24,136,20 x=88.00 base=34.40
clip 0,0,864,1632
pixels 22,52,72,16 72x16 a=255 crc=d22b684a
unclip
```

Rects are `x,y,width,height`. An image names its file and the part of it, then where it goes.
`pixels` gives a checksum of the picture in place of its pixels.
