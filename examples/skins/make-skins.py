# SPDX-License-Identifier: GPL-3.0-or-later
"""Writes walkit's two example skins, walkit-classic.wsz and walkit-modern.wal.

Every picture is drawn here, pixel by pixel, with the Python standard library alone:
the bitmaps are written as 24-bit BMP or RGBA PNG by hand and zipped with fixed times
and sorted names, so a run gives the same bytes each time.

    python3 examples/skins/make-skins.py
"""

import os
import struct
import zipfile
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))

# ---------------------------------------------------------------------------
# palette: one dark theme, one accent

BG = (30, 33, 38)
BG_HI = (50, 55, 63)
BG_LO = (17, 19, 22)
EDGE = (8, 9, 11)
TITLE = (40, 44, 51)
TITLE_OFF = (32, 35, 40)
PANEL = (12, 14, 17)
GROOVE = (38, 42, 49)
ACCENT = (45, 212, 191)
ACCENT_MID = (32, 150, 136)
ACCENT_DIM = (22, 96, 88)
ACCENT_DARK = (14, 52, 49)
INK = (216, 222, 228)
INK_DIM = (112, 120, 131)
INK_FAINT = (70, 76, 86)
BTN = (48, 53, 61)
BTN_HI = (66, 72, 82)
BTN_LO = (22, 24, 28)
BTN_DOWN = (24, 27, 31)


# ---------------------------------------------------------------------------
# a canvas of RGBA pixels


class Canvas:
    def __init__(self, w, h, color=BG):
        self.w = w
        self.h = h
        self.px = [rgba(color)] * (w * h)

    def put(self, x, y, c):
        if 0 <= x < self.w and 0 <= y < self.h:
            self.px[y * self.w + x] = rgba(c)

    def get(self, x, y):
        return self.px[y * self.w + x]

    def fill(self, x, y, w, h, c):
        c = rgba(c)
        for yy in range(max(y, 0), min(y + h, self.h)):
            row = yy * self.w
            for xx in range(max(x, 0), min(x + w, self.w)):
                self.px[row + xx] = c

    def hline(self, x, y, w, c):
        self.fill(x, y, w, 1, c)

    def vline(self, x, y, h, c):
        self.fill(x, y, 1, h, c)

    def edges(self, x, y, w, h, top_left, bottom_right):
        self.hline(x, y, w, top_left)
        self.vline(x, y, h, top_left)
        self.hline(x, y + h - 1, w, bottom_right)
        self.vline(x + w - 1, y, h, bottom_right)

    def raised(self, x, y, w, h, face=BG, hi=BG_HI, lo=BG_LO):
        self.fill(x, y, w, h, face)
        self.edges(x, y, w, h, hi, lo)

    def sunken(self, x, y, w, h, face=PANEL):
        """A display well: a dark rim on top and left, a light one under and right."""
        self.fill(x, y, w, h, face)
        self.edges(x, y, w, h, BG_LO, BG_HI)

    def copy(self, src, sx, sy, w, h, dx, dy):
        for yy in range(h):
            for xx in range(w):
                self.put(dx + xx, dy + yy, src.get(sx + xx, sy + yy))

    def round_rect(self, x, y, w, h, c, r=1):
        """A box with its corners cut by `r` pixels."""
        for yy in range(h):
            cut = max(0, r - yy, r - (h - 1 - yy))
            self.fill(x + cut, y + yy, w - 2 * cut, 1, c)


def rgba(c):
    return c if len(c) == 4 else (c[0], c[1], c[2], 255)


def mix(a, b, t):
    return tuple(round(a[i] + (b[i] - a[i]) * t) for i in range(3))


# ---------------------------------------------------------------------------
# a small pixel font: 4 x 5 letters (case folded), 3 x 5 digits

GLYPHS = {
    "A": [".##.", "#..#", "####", "#..#", "#..#"],
    "B": ["###.", "#..#", "###.", "#..#", "###."],
    "C": [".###", "#...", "#...", "#...", ".###"],
    "D": ["###.", "#..#", "#..#", "#..#", "###."],
    "E": ["####", "#...", "###.", "#...", "####"],
    "F": ["####", "#...", "###.", "#...", "#..."],
    "G": [".###", "#...", "#.##", "#..#", ".###"],
    "H": ["#..#", "#..#", "####", "#..#", "#..#"],
    "I": ["###", ".#.", ".#.", ".#.", "###"],
    "J": ["..##", "...#", "...#", "#..#", ".##."],
    "K": ["#..#", "#.#.", "##..", "#.#.", "#..#"],
    "L": ["#...", "#...", "#...", "#...", "####"],
    "M": ["#..#", "####", "####", "#..#", "#..#"],
    "N": ["#..#", "##.#", "#.##", "#..#", "#..#"],
    "O": [".##.", "#..#", "#..#", "#..#", ".##."],
    "P": ["###.", "#..#", "###.", "#...", "#..."],
    "Q": [".##.", "#..#", "#..#", "#.#.", ".#.#"],
    "R": ["###.", "#..#", "###.", "#.#.", "#..#"],
    "S": [".###", "#...", ".##.", "...#", "###."],
    "T": ["###", ".#.", ".#.", ".#.", ".#."],
    "U": ["#..#", "#..#", "#..#", "#..#", ".##."],
    "V": ["#.#", "#.#", "#.#", "#.#", ".#."],
    "W": ["#..#", "#..#", "####", "####", "#..#"],
    "X": ["#..#", "#..#", ".##.", "#..#", "#..#"],
    "Y": ["#.#", "#.#", ".#.", ".#.", ".#."],
    "Z": ["####", "...#", ".##.", "#...", "####"],
    "0": ["###", "#.#", "#.#", "#.#", "###"],
    "1": [".#.", "##.", ".#.", ".#.", "###"],
    "2": ["###", "..#", "###", "#..", "###"],
    "3": ["###", "..#", ".##", "..#", "###"],
    "4": ["#.#", "#.#", "###", "..#", "..#"],
    "5": ["###", "#..", "###", "..#", "###"],
    "6": ["###", "#..", "###", "#.#", "###"],
    "7": ["###", "..#", "..#", ".#.", ".#."],
    "8": ["###", "#.#", "###", "#.#", "###"],
    "9": ["###", "#.#", "###", "..#", "###"],
    '"': ["#.#", "#.#", "...", "...", "..."],
    "@": [".##.", "#.##", "#.##", "#...", ".###"],
    "…": [".....", ".....", ".....", ".....", "#.#.#"],
    ".": [".", ".", ".", ".", "#"],
    ":": [".", "#", ".", "#", "."],
    "(": [".#", "#.", "#.", "#.", ".#"],
    ")": ["#.", ".#", ".#", ".#", "#."],
    "-": ["...", "...", "###", "...", "..."],
    "'": ["#", "#", ".", ".", "."],
    "!": ["#", "#", "#", ".", "#"],
    "_": ["....", "....", "....", "....", "####"],
    "+": ["...", ".#.", "###", ".#.", "..."],
    "\\": ["#..", "#..", ".#.", "..#", "..#"],
    "/": ["..#", "..#", ".#.", "#..", "#.."],
    "[": ["##", "#.", "#.", "#.", "##"],
    "]": ["##", ".#", ".#", ".#", "##"],
    "^": [".#.", "#.#", "...", "...", "..."],
    "&": [".#..", "#.#.", ".#..", "#.#.", ".#.#"],
    "%": ["#..#", "..#.", ".#..", "#...", "#..#"],
    ",": ["..", "..", "..", ".#", "#."],
    "=": ["...", "###", "...", "###", "..."],
    "$": [".###", "##..", ".##.", "..##", "###."],
    "#": [".#.#", "####", ".#.#", "####", ".#.#"],
    "Å": [".##.", ".##.", "#..#", "####", "#..#"],
    "Ö": ["#..#", ".##.", "#..#", "#..#", ".##."],
    "Ä": ["#..#", ".##.", "#..#", "####", "#..#"],
    "?": ["###.", "...#", ".##.", "....", ".#.."],
    "*": ["...", "#.#", ".#.", "#.#", "..."],
    " ": ["..", "..", "..", "..", ".."],
}


def glyph(c):
    return GLYPHS.get(c.upper(), GLYPHS["?"])


def draw_glyph(img, x, y, c, color):
    for r, row in enumerate(glyph(c)):
        for k, ch in enumerate(row):
            if ch == "#":
                img.put(x + k, y + r, color)


def text_width(s):
    return sum(len(glyph(c)[0]) + 1 for c in s) - 1 if s else 0


def draw_text(img, x, y, s, color):
    """`s` in the pixel font, each letter as wide as it is drawn and one pixel apart."""
    for c in s:
        draw_glyph(img, x, y, c, color)
        x += len(glyph(c)[0]) + 1


def draw_text_centered(img, x, y, w, s, color):
    draw_text(img, x + (w - text_width(s)) // 2, y, s, color)


# ---------------------------------------------------------------------------
# icons, drawn into a box and centered on it


def icon_triangle_right(img, x, y, color, h=9):
    for c in range((h + 1) // 2):
        img.vline(x + c, y + c, h - 2 * c, color)


def icon_triangle_left(img, x, y, color, h=9):
    half = (h + 1) // 2
    for c in range(half):
        img.vline(x + half - 1 - c, y + c, h - 2 * c, color)


def icon(img, kind, cx, cy, color):
    """Transport icon `kind` centered on (`cx`, `cy`)."""
    if kind == "play":
        icon_triangle_right(img, cx - 2, cy - 4, color)
    elif kind == "pause":
        img.fill(cx - 3, cy - 4, 2, 9, color)
        img.fill(cx + 1, cy - 4, 2, 9, color)
    elif kind == "stop":
        img.fill(cx - 3, cy - 3, 7, 7, color)
    elif kind == "prev":
        img.fill(cx - 5, cy - 3, 2, 7, color)
        icon_triangle_left(img, cx - 3, cy - 3, color, 7)
        icon_triangle_left(img, cx + 1, cy - 3, color, 7)
    elif kind == "next":
        icon_triangle_right(img, cx - 4, cy - 3, color, 7)
        icon_triangle_right(img, cx, cy - 3, color, 7)
        img.fill(cx + 4, cy - 3, 2, 7, color)
    elif kind == "eject":
        for r in range(4):
            img.hline(cx - r - 1, cy - 4 + r, 2 * r + 3, color)
        img.fill(cx - 4, cy + 1, 9, 2, color)
    elif kind == "close":
        for i in range(5):
            img.put(cx - 2 + i, cy - 2 + i, color)
            img.put(cx + 2 - i, cy - 2 + i, color)
    elif kind == "min":
        img.hline(cx - 2, cy + 2, 5, color)
    elif kind == "shade":
        img.hline(cx - 2, cy - 2, 5, color)
        img.hline(cx - 2, cy - 1, 5, color)
    elif kind == "unshade":
        img.edges(cx - 2, cy - 2, 5, 5, color, color)
    elif kind == "menu":
        for r in (-2, 0, 2):
            img.hline(cx - 2, cy + r, 5, color)
    elif kind == "up":
        for r in range(3):
            img.hline(cx - r, cy - 1 + r, 2 * r + 1, color)
    elif kind == "down":
        for r in range(3):
            img.hline(cx - 2 + r, cy - 1 + r, 5 - 2 * r, color)


def button(img, x, y, w, h, kind=None, label=None, down=False, lit=False):
    """A flat button with a one-pixel bevel; pressed, it sinks and its mark turns accent."""
    if down:
        img.raised(x, y, w, h, BTN_DOWN, BTN_LO, BTN_HI)
    else:
        img.raised(x, y, w, h, BTN, BTN_HI, BTN_LO)
    color = ACCENT if (down or lit) else INK
    cx = x + w // 2
    cy = y + h // 2
    if kind:
        icon(img, kind, cx, cy, color)
    if label:
        draw_text_centered(img, x, y + (h - 5) // 2, w, label, color)


def small_button(img, x, y, kind, down=False, face=TITLE):
    """A 9 x 9 title bar button."""
    if down:
        img.round_rect(x, y, 9, 9, ACCENT_DARK)
        icon(img, kind, x + 4, y + 4, ACCENT)
    else:
        img.round_rect(x, y, 9, 9, BTN_HI if face == TITLE else BTN)
        icon(img, kind, x + 4, y + 4, INK if face == TITLE else INK_DIM)


def grip(img, x, y, w, color):
    """Two dotted lines across a title bar."""
    for xx in range(x, x + w, 2):
        img.put(xx, y, color)
        img.put(xx + 1, y + 2, color)


def title_strip(img, x, y, w, h, name, active):
    """A title bar: a flat strip with a grip either side of the centered name."""
    face = TITLE if active else TITLE_OFF
    img.fill(x, y, w, h, face)
    img.hline(x, y, w, BG_HI if active else BG)
    img.hline(x, y + h - 1, w, EDGE)
    tw = text_width(name)
    tx = x + (w - tw) // 2
    ty = y + (h - 5) // 2
    line = ACCENT_DIM if active else INK_FAINT
    gy = ty + 1
    grip(img, x + 20, gy, tx - x - 26, line)
    grip(img, tx + tw + 6, gy, x + w - 46 - (tx + tw + 6), line)
    draw_text(img, tx, ty, name, ACCENT if active else INK_DIM)


# ---------------------------------------------------------------------------
# file writers


def bmp(img):
    """`img` as a 24-bit BMP, rows bottom first."""
    pad = (4 - img.w * 3 % 4) % 4
    rows = []
    for y in range(img.h - 1, -1, -1):
        row = bytearray()
        for x in range(img.w):
            r, g, b, _ = img.get(x, y)
            row += bytes((b, g, r))
        row += b"\0" * pad
        rows.append(bytes(row))
    data = b"".join(rows)
    header = struct.pack("<2sIHHI", b"BM", 54 + len(data), 0, 0, 54)
    info = struct.pack("<IiiHHIIiiII", 40, img.w, img.h, 1, 24, 0, len(data), 2835, 2835, 0, 0)
    return header + info + data


def png(img):
    """`img` as an RGBA PNG."""
    raw = bytearray()
    for y in range(img.h):
        raw.append(0)
        for x in range(img.w):
            raw += bytes(img.get(x, y))

    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    head = struct.pack(">IIBBBBB", img.w, img.h, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", head)
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )


def write_zip(path, files):
    """`files` (name to bytes) as a zip with sorted names and a fixed time."""
    with zipfile.ZipFile(path, "w") as z:
        for name in sorted(files):
            info = zipfile.ZipInfo(name, date_time=(2024, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.create_system = 0
            info.external_attr = 0
            z.writestr(info, files[name], compresslevel=9)


# ---------------------------------------------------------------------------
# the classic skin


def text_bmp():
    """The 5 x 6 cell font: letters, then digits and marks, then a few more; the rest blank."""
    img = Canvas(155, 18, PANEL)
    rows = [
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ\"@",
        "0123456789….:()-'!_+\\/[]^&%,=$#",
        "ÅÖÄ?*",
    ]
    for r, chars in enumerate(rows):
        for i, c in enumerate(chars):
            draw_glyph(img, i * 5, r * 6, c, ACCENT)
    return img


SEGMENTS = {
    "a": (1, 1, 7, 2),
    "b": (6, 1, 2, 6),
    "c": (6, 6, 2, 6),
    "d": (1, 10, 7, 2),
    "e": (1, 6, 2, 6),
    "f": (1, 1, 2, 6),
    "g": (1, 6, 7, 1),
}
DIGIT_SEGMENTS = [
    "abcdef", "bc", "abdeg", "abcdg", "bcfg", "acdfg", "acdefg", "abc", "abcdefg", "abcdfg",
]


def numbers_bmp(extended):
    """The 9 x 13 time digits in seven segments, then a blank cell (and a minus when
    `extended`). A plain sheet keeps its minus in the middle of the 2."""
    img = Canvas(108 if extended else 99, 13, PANEL)
    for n, segs in enumerate(DIGIT_SEGMENTS):
        for s in segs:
            x, y, w, h = SEGMENTS[s]
            img.fill(n * 9 + x, y, w, h, ACCENT)
    if extended:
        img.fill(99 + 2, 6, 5, 1, ACCENT)
    return img


def titlebar_bmp():
    img = Canvas(344, 87, TITLE)
    # the buttons at rest and pressed: menu, minimize, close; shade and its pressed look
    small_button(img, 0, 0, "menu")
    small_button(img, 0, 9, "menu", True)
    small_button(img, 9, 0, "min")
    small_button(img, 9, 9, "min", True)
    small_button(img, 18, 0, "close")
    small_button(img, 18, 9, "close", True)
    small_button(img, 0, 18, "shade")
    small_button(img, 9, 18, "shade", True)
    small_button(img, 0, 27, "unshade")
    small_button(img, 9, 27, "unshade", True)
    # the bar, active and not, with its buttons in place
    for y, active in ((0, True), (15, False)):
        title_strip(img, 27, y, 275, 14, "WALKIT", active)
        face = TITLE if active else TITLE_OFF
        small_button(img, 27 + 6, y + 3, "menu", face=face)
        small_button(img, 27 + 244, y + 3, "min", face=face)
        small_button(img, 27 + 254, y + 3, "shade", face=face)
        small_button(img, 27 + 264, y + 3, "close", face=face)
    # the shade bar: name, mini vis, transport, time and position, buttons
    for y, active in ((29, True), (42, False)):
        face = TITLE if active else TITLE_OFF
        x0 = 27
        img.fill(x0, y, 275, 14, face)
        img.hline(x0, y, 275, BG_HI if active else BG)
        img.hline(x0, y + 13, 275, EDGE)
        small_button(img, x0 + 6, y + 3, "menu", face=face)
        draw_text(img, x0 + 20, y + 5, "WALKIT", ACCENT if active else INK_DIM)
        img.sunken(x0 + 77, y + 3, 41, 9)
        for kind, bx, bw in (("prev", 168, 8), ("play", 176, 10), ("pause", 186, 9),
                             ("stop", 195, 9), ("next", 204, 11), ("eject", 215, 10)):
            mini_icon(img, kind, x0 + bx + bw // 2, y + 7, INK if active else INK_DIM)
        img.sunken(x0 + 124, y + 3, 35, 8)
        img.put(x0 + 145, y + 5, ACCENT)
        img.put(x0 + 145, y + 7, ACCENT)
        small_button(img, x0 + 244, y + 3, "min", face=face)
        small_button(img, x0 + 254, y + 3, "unshade", face=face)
        small_button(img, x0 + 264, y + 3, "close", face=face)
    # the mini position bar and its thumb in three steps
    img.fill(0, 36, 17, 7, TITLE)
    img.fill(0, 39, 17, 1, GROOVE)
    for i, c in enumerate((ACCENT_MID, ACCENT, INK)):
        img.fill(17 + 3 * i, 36, 3, 7, TITLE)
        img.fill(17 + 3 * i, 37, 3, 5, c)
    # the clutter bar: at rest, off, and one column per held letter
    clutter(img, 304, 0, None, False)
    clutter(img, 312, 0, None, True)
    for k in range(5):
        clutter(img, 304 + 8 * k, 44, k, False)
    return img


CLUTTER = [("O", 2), ("A", 11), ("I", 19), ("D", 27), ("V", 34)]


def clutter(img, x, y, held, disabled):
    """The clutter bar's column: O, A, I, D and V down a strip 8 x 43, `held` lit."""
    img.fill(x, y, 8, 43, BG)
    img.vline(x, y, 43, BG_LO)
    img.vline(x + 7, y, 43, BG_HI)
    for k, (letter, top) in enumerate(CLUTTER):
        if k == held:
            img.fill(x + 1, y + top - 1, 6, 7, ACCENT_DARK)
        color = INK_FAINT if disabled else (ACCENT if k == held else INK_DIM)
        g = glyph(letter)
        draw_glyph(img, x + 1 + (6 - len(g[0])) // 2, y + top, letter, color)


def mini_icon(img, kind, cx, cy, color):
    """A transport icon 5 high for the shade bars."""
    if kind == "play":
        icon_triangle_right(img, cx - 1, cy - 2, color, 5)
    elif kind == "pause":
        img.fill(cx - 2, cy - 2, 2, 5, color)
        img.fill(cx + 1, cy - 2, 2, 5, color)
    elif kind == "stop":
        img.fill(cx - 2, cy - 2, 5, 5, color)
    elif kind == "prev":
        img.vline(cx - 2, cy - 2, 5, color)
        icon_triangle_left(img, cx - 1, cy - 2, color, 5)
    elif kind == "next":
        icon_triangle_right(img, cx - 1, cy - 2, color, 5)
        img.vline(cx + 2, cy - 2, 5, color)
    elif kind == "eject":
        for r in range(3):
            img.hline(cx - r, cy - 2 + r, 2 * r + 1, color)
        img.hline(cx - 2, cy + 2, 5, color)


def cbuttons_bmp():
    img = Canvas(136, 36, BG)
    spans = [("prev", 0, 23), ("play", 23, 23), ("pause", 46, 23), ("stop", 69, 23),
             ("next", 92, 22)]
    for kind, x, w in spans:
        button(img, x, 0, w, 18, kind)
        button(img, x, 18, w, 18, kind, down=True)
    button(img, 114, 0, 22, 16, "eject")
    button(img, 114, 16, 22, 16, "eject", down=True)
    return img


def playpaus_bmp():
    img = Canvas(42, 9, PANEL)
    icon_triangle_right(img, 2, 1, ACCENT, 7)
    img.fill(10, 1, 2, 7, ACCENT)
    img.fill(14, 1, 2, 7, ACCENT)
    img.fill(20, 2, 5, 5, INK_DIM)
    return img


def monoster_bmp():
    img = Canvas(56, 24, PANEL)
    for y, on in ((0, True), (12, False)):
        color = ACCENT if on else INK_FAINT
        draw_text_centered(img, 0, y + 4, 29, "STEREO", color)
        draw_text_centered(img, 29, y + 4, 27, "MONO", color)
    return img


def posbar_bmp():
    img = Canvas(307, 10, BG)
    posbar_track(img, 0, 0)
    for x, c in ((248, ACCENT), (278, INK)):
        img.fill(x, 0, 29, 10, BG)
        img.round_rect(x, 1, 29, 8, c, 2)
        img.fill(x + 13, 3, 1, 4, ACCENT_DARK)
        img.fill(x + 15, 3, 1, 4, ACCENT_DARK)
    return img


def posbar_track(img, x, y):
    img.fill(x, y, 248, 10, BG)
    img.sunken(x, y + 3, 248, 4, GROOVE)


def slider_sheet(width, track_x, track_w, frames_fill):
    """A volume or balance sheet: 28 frames 15 apart, then the thumbs at y 422."""
    img = Canvas(width, 433, BG)
    for k in range(28):
        y = k * 15
        img.fill(track_x, y, track_w, 13, BG)
        img.sunken(track_x, y + 5, track_w, 3, GROOVE)
        for x0, w in frames_fill(k):
            img.fill(track_x + x0, y + 6, w, 1, mix(ACCENT_DIM, ACCENT, k / 27))
    for x, c in ((0, INK), (15, ACCENT)):
        img.fill(x, 422, 14, 11, BG)
        img.round_rect(x + 1, 422, 12, 11, c, 2)
        img.fill(x + 6, 425, 2, 5, BG_LO)
    return img


def volume_bmp():
    return slider_sheet(68, 0, 68, lambda k: [(1, round(k * 51 / 27) + 6)])


def balance_bmp():
    def fill(k):
        half = round(k * 12 / 27)
        return [(19 - half, 2 * half + 1)] if k else [(19, 1)]

    return slider_sheet(47, 9, 38, fill)


def shufrep_bmp():
    img = Canvas(92, 85, BG)
    # off, off pressed, on, on pressed
    for i, (down, lit) in enumerate(((False, False), (True, False), (False, True), (True, True))):
        y = i * 15
        button(img, 0, y, 28, 15, down=down)
        repeat_icon(img, 9, y + 4, ACCENT if (down or lit) else INK)
        if lit:
            img.fill(3, y + 6, 2, 3, ACCENT)
        button(img, 28, y, 47, 15, label="SHUFFLE", down=down, lit=lit)
    # equalizer and playlist toggles: off, on; pressed off, pressed on
    for x, label in ((0, "EQ"), (23, "PL")):
        button(img, x, 61, 23, 12, label=label)
        button(img, x, 73, 23, 12, label=label, lit=True)
        button(img, x + 46, 61, 23, 12, label=label, down=True)
        button(img, x + 46, 73, 23, 12, label=label, down=True, lit=True)
    return img


def repeat_icon(img, x, y, color):
    img.hline(x + 1, y, 9, color)
    img.vline(x, y + 1, 4, color)
    img.hline(x + 1, y + 5, 9, color)
    img.vline(x + 10, y + 1, 4, color)
    img.put(x + 8, y - 1, color)
    img.put(x + 8, y + 1, color)


def main_bmp():
    img = Canvas(275, 116, BG)
    img.edges(0, 0, 275, 116, BG_HI, EDGE)
    img.edges(1, 1, 273, 114, BG, BG_LO)
    # the time and vis display, the clutter bar beside it
    img.sunken(20, 22, 85, 40)
    img.fill(72, 30, 2, 2, ACCENT)
    img.fill(72, 34, 2, 2, ACCENT)
    # the song display: title, rates, channels
    img.sunken(108, 22, 161, 33)
    img.hline(111, 37, 154, GROOVE)
    draw_text(img, 128, 43, "KBPS", INK_DIM)
    draw_text(img, 168, 43, "KHZ", INK_DIM)
    # volume and balance land on the background; the position bar's track shows stopped
    posbar_track(img, 16, 72)
    # a separator over the buttons and the logo at the bottom right
    img.hline(10, 84, 255, BG_LO)
    img.hline(10, 85, 255, BG_HI)
    logo(img, 253, 92)
    return img


def logo(img, x, y):
    """A small mark: three bars rising."""
    for i, h in enumerate((5, 9, 13)):
        img.fill(x + i * 4 + 1, y + 13 - h, 3, h, mix(ACCENT_DIM, ACCENT, i / 2))


def eqmain_bmp():
    img = Canvas(275, 315, BG)
    # the window: border, the sliders' well, labels
    img.edges(0, 0, 275, 116, BG_HI, EDGE)
    img.edges(1, 1, 273, 114, BG, BG_LO)
    img.sunken(16, 36, 247, 67)
    img.vline(74, 40, 59, GROOVE)
    for label, y in (("+12", 37), ("0", 67), ("-12", 95)):
        draw_text(img, 66 - text_width(label), y, label, INK_DIM)
    draw_text_centered(img, 21, 106, 14, "PRE", INK_DIM)
    for i, label in enumerate(("60", "170", "310", "600", "1K", "3K", "6K", "12K", "14K", "16K")):
        draw_text_centered(img, 78 + 18 * i, 106, 14, label, INK_DIM)
    # the close button at rest and pressed
    small_button(img, 0, 116, "close")
    small_button(img, 0, 125, "close", True)
    # the title bar, active and not
    for y, active in ((134, True), (149, False)):
        title_strip(img, 0, y, 275, 14, "EQUALIZER", active)
        face = TITLE if active else TITLE_OFF
        small_button(img, 254, y + 3, "shade", face=face)
        small_button(img, 264, y + 3, "close", face=face)
    # on and auto: off, on, pressed off, pressed on
    for i, (down, lit) in enumerate(((False, False), (False, True), (True, False), (True, True))):
        x = 10 + 59 * i
        led_button(img, x, 119, 25, "ON", down, lit)
        led_button(img, x + 25, 119, 33, "AUTO", down, lit)
    # the slider frames, 14 x 63, from the bottom of the range up; the thumbs; presets
    for k in range(28):
        x, y = (13 + 15 * k, 164) if k < 14 else (13 + 15 * (k - 14), 229)
        eq_slider_frame(img, x, y, k)
    for y, c in ((164, INK), (176, ACCENT)):
        img.fill(0, y, 11, 11, PANEL)
        img.round_rect(0, y + 2, 11, 7, c, 2)
        img.hline(2, y + 5, 7, BG_LO)
    button(img, 224, 164, 44, 12, label="PRESETS")
    button(img, 224, 176, 44, 12, label="PRESETS", down=True)
    # the graph's background, its line colors top to bottom, the preamp line
    img.fill(0, 294, 113, 19, PANEL)
    img.edges(0, 294, 113, 19, BG_LO, BG_HI)
    for x in range(2, 112, 4):
        img.put(x, 294 + 9, GROOVE)
    for r in range(19):
        img.put(115, 294 + r, mix(ACCENT, ACCENT_MID, abs(r - 9) / 9))
    img.hline(0, 314, 113, INK_FAINT)
    return img


def led_button(img, x, y, w, label, down, lit):
    if down:
        img.raised(x, y, w, 12, BTN_DOWN, BTN_LO, BTN_HI)
    else:
        img.raised(x, y, w, 12, BTN, BTN_HI, BTN_LO)
    img.fill(x + 3, y + 4, 3, 4, ACCENT if lit else BTN_LO)
    draw_text(img, x + 8, y + 4, label, ACCENT if lit else INK)


def eq_slider_frame(img, x, y, k):
    """Frame `k` of a band slider: 0 at the bottom of the range, 27 at the top, with the
    fill from the middle to where the thumb sits for it."""
    img.fill(x, y, 14, 63, PANEL)
    img.fill(x + 5, y + 2, 3, 59, GROOVE)
    img.hline(x + 2, y + 30, 10, INK_FAINT)
    # the thumb's middle for the values that pick this frame
    centers = [(v - 63) * 52 // 64 + 51 + 5 if v < 63 else 56 for v in range(64)
               if 27 - v * 28 // 64 == k]
    if centers:
        mid = round(sum(centers) / len(centers))
        top, bottom = sorted((30, mid))
        img.fill(x + 6, y + top, 1, bottom - top + 1, ACCENT)


def eq_ex_bmp():
    img = Canvas(275, 56, BG)
    for y, active in ((0, True), (15, False)):
        face = TITLE if active else TITLE_OFF
        img.fill(0, y, 275, 14, face)
        img.hline(0, y, 275, BG_HI if active else BG)
        img.hline(0, y + 13, 275, EDGE)
        draw_text(img, 8, y + 5, "EQ", ACCENT if active else INK_DIM)
        img.sunken(61, y + 6, 101, 3, GROOVE)
        img.sunken(164, y + 6, 45, 3, GROOVE)
        small_button(img, 254, y + 3, "unshade", face=face)
        small_button(img, 264, y + 3, "close", face=face)
    for i in range(3):
        img.fill(1 + 3 * i, 30, 3, 7, mix(ACCENT_MID, ACCENT, i / 2))
        img.fill(11 + 3 * i, 30, 3, 7, mix(ACCENT_MID, ACCENT, i / 2))
    small_button(img, 1, 38, "unshade", True)
    small_button(img, 1, 47, "shade", True)
    small_button(img, 11, 38, "close")
    small_button(img, 11, 47, "close", True)
    return img


PL_MENUS = [
    # menu, picture column, bar column, labels from y 111 down
    ("add", 0, 48, ["URL", "DIR", "FILE"]),
    ("rem", 54, 100, ["ALL", "CROP", "SEL", "MISC"]),
    ("sel", 104, 150, ["INV", "NONE", "ALL"]),
    ("misc", 154, 200, ["SORT", "INFO", "OPTS"]),
    ("list", 204, 250, ["NEW", "SAVE", "LOAD"]),
]


def pledit_bmp():
    img = Canvas(280, 186, BG)
    # the title bar: corners, the title, a tile, active (y 0) and not (y 21)
    for y, active in ((0, True), (21, False)):
        face = TITLE if active else TITLE_OFF
        for x, w in ((0, 25), (26, 100), (127, 25), (153, 25)):
            img.fill(x, y, w, 20, face)
            img.hline(x, y, w, BG_HI if active else BG)
            img.hline(x, y + 19, w, EDGE)
        line = ACCENT_DIM if active else INK_FAINT
        grip(img, 127, y + 8, 25, line)
        grip(img, 0, y + 8, 25, line)
        draw_text_centered(img, 26, y + 8, 100, "PLAYLIST", ACCENT if active else INK_DIM)
        grip(img, 26, y + 8, 18, line)
        grip(img, 108, y + 8, 18, line)
        grip(img, 153, y + 8, 4, line)
        small_button(img, 153 + 5, y + 3, "shade", face=face)
        small_button(img, 153 + 15, y + 3, "close", face=face)
    # the sides: left 12, right 5 + track 8 + 7
    img.fill(0, 42, 12, 29, BG)
    img.vline(0, 42, 29, BG_HI)
    img.vline(11, 42, 29, BG_LO)
    img.fill(31, 42, 20, 29, BG)
    img.vline(35, 42, 29, BG_LO)
    img.fill(36, 42, 8, 29, PANEL)
    img.vline(50, 42, 29, EDGE)
    # the scrollbar thumb, at rest and held
    for x, c in ((52, BTN_HI), (61, ACCENT_MID)):
        img.fill(x, 53, 8, 18, PANEL)
        img.round_rect(x + 1, 53, 6, 18, c, 1)
        for r in (59, 62, 65):
            img.hline(x + 2, r, 4, BG_LO)
    small_button(img, 52, 42, "close", True)
    small_button(img, 62, 42, "shade", True)
    small_button(img, 150, 42, "unshade", True)
    # the shade bar: left piece and tile (active, inactive), right piece with its buttons
    for y, active in ((42, True), (57, False)):
        face = TITLE if active else TITLE_OFF
        for x, w in ((72, 25), (99, 50)):
            img.fill(x, y, w, 14, face)
            img.hline(x, y, w, BG_HI if active else BG)
            img.hline(x, y + 13, w, EDGE)
            img.fill(x, y + 3, w, 8, PANEL)
        img.fill(99 + 21, y + 3, 29, 8, face)
        small_button(img, 99 + 30, y + 3, "unshade", face=face)
        small_button(img, 99 + 40, y + 3, "close", face=face)
    img.fill(72, 57, 25, 14, TITLE_OFF)
    img.hline(72, 57, 25, BG)
    img.hline(72, 70, 25, EDGE)
    img.fill(72, 60, 25, 8, PANEL)
    # the bottom bar: the left part with its four buttons
    bottom_base(img, 0, 72, 125)
    for x, label in ((14, "ADD"), (43, "REM"), (72, "SEL"), (101, "MISC")):
        button(img, x, 72 + 8, 22, 18, label=label)
    # the right part: the two time lines, the mini transport, list, the scroll arrows, a grip
    bottom_base(img, 126, 72, 150)
    bx = 126
    img.sunken(bx + 5, 72 + 8, 94, 10)
    img.sunken(bx + 62, 72 + 21, 37, 10)
    img.put(bx + 84, 72 + 24, ACCENT)
    img.put(bx + 84, 72 + 26, ACCENT)
    for i, kind in enumerate(("prev", "play", "pause", "stop", "next", "eject")):
        mini_icon(img, kind, bx + 6 + 9 * i + 4, 72 + 26, INK_DIM)
    button(img, bx + 106, 72 + 8, 22, 18, label="LIST")
    icon(img, "up", bx + 139, 72 + 4, INK_DIM)
    icon(img, "down", bx + 139, 72 + 9, INK_DIM)
    for i in range(4):
        for j in range(4 - i):
            img.put(bx + 145 - 3 * j, 72 + 34 - 3 * i, INK_FAINT)
    # the tile between them and the visualizer piece
    bottom_base(img, 179, 0, 25)
    bottom_base(img, 205, 0, 75)
    img.sunken(205 + 6, 8, 63, 22)
    # the menus: pictures at rest and under the finger, the bar beside them
    for _, col, bar, labels in PL_MENUS:
        for i, label in enumerate(labels):
            y = 111 + 19 * i
            img.fill(col, y, 22, 18, BTN)
            img.edges(col, y, 22, 18, BTN_HI, BTN_LO)
            draw_text_centered(img, col, y + 7, 22, label, INK)
            img.fill(col + 23, y, 22, 18, ACCENT_MID)
            img.edges(col + 23, y, 22, 18, ACCENT, ACCENT_DARK)
            draw_text_centered(img, col + 23, y + 7, 22, label, PANEL)
        img.fill(bar, 111, 3, 18 * len(labels), ACCENT)
    return img


def bottom_base(img, x, y, w):
    img.fill(x, y, w, 38, BG)
    img.hline(x, y, w, BG_LO)
    img.hline(x, y + 1, w, BG_HI)
    img.hline(x, y + 37, w, EDGE)


def viscolor_txt():
    colors = [PANEL, (28, 32, 38)]
    # the analyzer from the top down: light to dark
    for i in range(16):
        colors.append(mix((200, 255, 245), ACCENT_DIM, i / 15))
    for i in range(5):
        colors.append(mix(ACCENT, ACCENT_DIM, i / 4))
    colors.append(INK)
    names = ["background", "dots"] + [f"analyzer {i + 1}" for i in range(16)] + [
        f"scope {i + 1}" for i in range(5)] + ["peaks"]
    return "".join(f"{r},{g},{b}, // {n}\r\n" for (r, g, b), n in zip(colors, names)).encode()


def pledit_txt():
    return (
        "[Text]\r\n"
        "Normal=#AEBAC4\r\n"
        "Current=#2DD4BF\r\n"
        "NormalBG=#0C0E11\r\n"
        "SelectedBG=#1E4C47\r\n"
        "Font=Arial\r\n"
    ).encode()


def classic_files():
    files = {
        "main.bmp": bmp(main_bmp()),
        "titlebar.bmp": bmp(titlebar_bmp()),
        "cbuttons.bmp": bmp(cbuttons_bmp()),
        "numbers.bmp": bmp(numbers_bmp(False)),
        "nums_ex.bmp": bmp(numbers_bmp(True)),
        "text.bmp": bmp(text_bmp()),
        "playpaus.bmp": bmp(playpaus_bmp()),
        "monoster.bmp": bmp(monoster_bmp()),
        "posbar.bmp": bmp(posbar_bmp()),
        "volume.bmp": bmp(volume_bmp()),
        "balance.bmp": bmp(balance_bmp()),
        "shufrep.bmp": bmp(shufrep_bmp()),
        "eqmain.bmp": bmp(eqmain_bmp()),
        "eq_ex.bmp": bmp(eq_ex_bmp()),
        "pledit.bmp": bmp(pledit_bmp()),
        "pledit.txt": pledit_txt(),
        "viscolor.txt": viscolor_txt(),
    }
    return files


# ---------------------------------------------------------------------------
# the modern skin

CLEAR = (0, 0, 0, 0)
MAIN_W, MAIN_H = 300, 112
PL_W, PL_H = 300, 180


def modern_main_bg():
    img = Canvas(MAIN_W, MAIN_H, BG)
    img.edges(0, 0, MAIN_W, MAIN_H, BG_HI, EDGE)
    title_strip(img, 1, 1, MAIN_W - 2, 15, "WALKIT", True)
    # the display: title, time, vis
    img.sunken(8, 22, 284, 44)
    img.hline(12, 41, 276, GROOVE)
    # the seek track
    img.sunken(8, 72, 284, 4, GROOVE)
    # under the buttons, the volume track
    img.sunken(204, 92, 88, 4, GROOVE)
    vol_icon(img, 192, 89)
    return img


def vol_icon(img, x, y):
    img.fill(x, y + 3, 2, 4, INK_DIM)
    for c in range(3):
        img.vline(x + 2 + c, y + 2 - c, 6 + 2 * c, INK_DIM)


def modern_pl_bg():
    img = Canvas(PL_W, PL_H, BG)
    img.edges(0, 0, PL_W, PL_H, BG_HI, EDGE)
    title_strip(img, 1, 1, PL_W - 2, 15, "PLAYLIST", True)
    img.sunken(7, 21, PL_W - 14, PL_H - 28)
    return img


def modern_button(kind, w, h, down):
    img = Canvas(w, h, CLEAR)
    button(img, 0, 0, w, h, kind, down=down)
    return img


def modern_text_button(label, w, h, down):
    img = Canvas(w, h, CLEAR)
    button(img, 0, 0, w, h, label=label, down=down)
    return img


def modern_close(down):
    img = Canvas(9, 9, CLEAR)
    small_button(img, 0, 0, "close", down)
    return img


def modern_thumb(w, h, color):
    img = Canvas(w, h, CLEAR)
    img.round_rect(0, 0, w, h, color, 2)
    return img


SKIN_XML = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- walkit's own modern example skin: one main window and a playlist window. -->
<WinampAbstractionLayer version="0.8">
  <skininfo>
    <version>1.0</version>
    <name>walkit modern</name>
    <comment>A flat dark example skin for walkit</comment>
    <author>walkit</author>
  </skininfo>

  <elements>
    <bitmap id="main.bg" file="img/main-bg.png"/>
    <bitmap id="pl.bg" file="img/pl-bg.png"/>
    <bitmap id="close" file="img/close.png"/>
    <bitmap id="close.down" file="img/close-down.png"/>
    <bitmap id="prev" file="img/prev.png"/>
    <bitmap id="prev.down" file="img/prev-down.png"/>
    <bitmap id="play" file="img/play.png"/>
    <bitmap id="play.down" file="img/play-down.png"/>
    <bitmap id="pause" file="img/pause.png"/>
    <bitmap id="pause.down" file="img/pause-down.png"/>
    <bitmap id="stop" file="img/stop.png"/>
    <bitmap id="stop.down" file="img/stop-down.png"/>
    <bitmap id="next" file="img/next.png"/>
    <bitmap id="next.down" file="img/next-down.png"/>
    <bitmap id="pl" file="img/pl.png"/>
    <bitmap id="pl.down" file="img/pl-down.png"/>
    <bitmap id="seek.thumb" file="img/seek-thumb.png"/>
    <bitmap id="seek.thumb.down" file="img/seek-thumb-down.png"/>
    <bitmap id="volume.thumb" file="img/volume-thumb.png"/>
    <bitmap id="volume.thumb.down" file="img/volume-thumb-down.png"/>
    <color id="wasabi.window.background" value="12,14,17"/>
    <color id="wasabi.list.background" value="12,14,17"/>
    <color id="wasabi.list.text" value="174,186,196"/>
    <color id="wasabi.list.text.current" value="45,212,191"/>
    <color id="wasabi.list.text.selected" value="216,222,228"/>
    <color id="wasabi.list.text.selected.background" value="30,76,71"/>
    <color id="wasabi.border.sunken" value="38,42,49"/>
    <color id="wasabi.scrollbar.background" value="30,33,38"/>
  </elements>

  <container id="main" name="Main Window" default_x="0" default_y="0" default_visible="1">
    <layout id="normal" background="main.bg" w="300" h="112">
      <layer id="titlebar" x="0" y="0" w="280" h="17" move="1"/>
      <button id="close" action="CLOSE" x="286" y="4" image="close" downImage="close.down" tooltip="Close"/>

      <text id="songname" display="songname" ticker="1" x="14" y="25" w="272" h="14"
        fontsize="15" color="216,222,228" align="left" default="walkit"/>
      <text id="time" display="time" timeroffstyle="1" x="14" y="44" w="80" h="20"
        fontsize="24" bold="1" color="45,212,191" align="left"/>
      <text id="info" display="songinfo" x="80" y="48" w="118" h="12"
        fontsize="11" color="112,120,131" align="right"/>
      <vis id="vis" x="204" y="44" w="82" h="20" mode="1" peaks="1"
        colorallbands="45,212,191" colorbandpeak="216,222,228" colorallosc="45,212,191"/>

      <slider id="seek" action="SEEK" x="8" y="69" w="284" h="10"
        thumb="seek.thumb" downThumb="seek.thumb.down"/>

      <button id="prev" action="PREV" x="8" y="84" image="prev" downImage="prev.down" tooltip="Previous"/>
      <button id="play" action="PLAY" x="34" y="84" image="play" downImage="play.down" tooltip="Play"/>
      <button id="pause" action="PAUSE" x="60" y="84" image="pause" downImage="pause.down" tooltip="Pause"/>
      <button id="stop" action="STOP" x="86" y="84" image="stop" downImage="stop.down" tooltip="Stop"/>
      <button id="next" action="NEXT" x="112" y="84" image="next" downImage="next.down" tooltip="Next"/>
      <button id="pltoggle" action="TOGGLE" param="pledit" x="146" y="84" image="pl" downImage="pl.down" tooltip="Playlist"/>

      <slider id="volume" action="VOLUME" x="204" y="88" w="88" h="12"
        thumb="volume.thumb" downThumb="volume.thumb.down"/>
    </layout>
  </container>

  <container id="pledit" name="Playlist Editor" default_x="0" default_y="112" default_visible="1">
    <layout id="normal" background="pl.bg" w="300" h="180">
      <layer id="pl.titlebar" x="0" y="0" w="280" h="17" move="1"/>
      <button id="pl.close" action="CLOSE" x="286" y="4" image="close" downImage="close.down" tooltip="Close"/>
      <component id="pl.list" x="8" y="22" w="284" h="150"
        param="guid:{45F3F7C1-A6F3-4ee6-A15E-125E92FC3F8D}"/>
    </layout>
  </container>
</WinampAbstractionLayer>
"""


def modern_files():
    files = {
        "skin.xml": SKIN_XML.encode(),
        "img/main-bg.png": png(modern_main_bg()),
        "img/pl-bg.png": png(modern_pl_bg()),
        "img/close.png": png(modern_close(False)),
        "img/close-down.png": png(modern_close(True)),
        "img/pl.png": png(modern_text_button("PL", 24, 18, False)),
        "img/pl-down.png": png(modern_text_button("PL", 24, 18, True)),
        "img/seek-thumb.png": png(modern_thumb(24, 10, ACCENT)),
        "img/seek-thumb-down.png": png(modern_thumb(24, 10, INK)),
        "img/volume-thumb.png": png(modern_thumb(10, 12, ACCENT)),
        "img/volume-thumb-down.png": png(modern_thumb(10, 12, INK)),
    }
    for kind in ("prev", "play", "pause", "stop", "next"):
        files[f"img/{kind}.png"] = png(modern_button(kind, 24, 18, False))
        files[f"img/{kind}-down.png"] = png(modern_button(kind, 24, 18, True))
    return files


def main():
    write_zip(os.path.join(HERE, "walkit-classic.wsz"), classic_files())
    write_zip(os.path.join(HERE, "walkit-modern.wal"), modern_files())


if __name__ == "__main__":
    main()
