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


def title_strip(img, x, y, w, h, name, active, classic=False):
    """A title bar: a flat strip with a grip either side of the centered name.

    Winamp puts a classic title bar's buttons at y 3, 9 high: `classic` runs the strip to
    row 13 with no dark line under it, which the window draws on row 14, and centers the name
    and grip on row 7 with the buttons."""
    face = TITLE if active else TITLE_OFF
    img.fill(x, y, w, h, face)
    img.hline(x, y, w, BG_HI if active else BG)
    if not classic:
        img.hline(x, y + h - 1, w, EDGE)
    tw = text_width(name)
    tx = x + (w - tw) // 2
    ty = y + 5 if classic else y + (h - 5) // 2
    line = ACCENT_DIM if active else INK_FAINT
    gy = ty + 1
    grip(img, x + 20, gy, tx - x - 26, line)
    grip(img, tx + tw + 6, gy, x + w - 46 - (tx + tw + 6), line)
    draw_text(img, tx, ty, name, ACCENT if active else INK_DIM)


def frame_title_piece(img, x, y, w, active):
    """A 20-high title piece of a framed window (playlist, gen): the 14-high bar as
    `title_strip` draws it, the line under it, the frame and the top rim of the well."""
    img.fill(x, y, w, 14, TITLE if active else TITLE_OFF)
    img.hline(x, y, w, BG_HI if active else BG)
    img.hline(x, y + 14, w, EDGE)
    img.fill(x, y + 15, w, 5, BG)
    img.hline(x, y + 19, w, BG_LO)


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
        title_strip(img, 27, y, 275, 14, "WALKIT", active, classic=True)
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
    # the line under the title bar
    img.hline(0, 14, 275, EDGE)
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
    img.hline(0, 14, 275, EDGE)
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
        title_strip(img, 0, y, 275, 14, "EQUALIZER", active, classic=True)
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
            frame_title_piece(img, x, y, w, active)
        # the frame's sides start under the bar: the left one's edge and rim, the right
        # one's rim over the list and the scroll bar, and its edge
        img.vline(0, y + 15, 5, BG_HI)
        img.hline(0, y + 19, 11, BG)
        img.put(0, y + 19, BG_HI)
        img.hline(153 + 18, y + 19, 7, BG)
        img.vline(153 + 24, y + 15, 5, EDGE)
        line = ACCENT_DIM if active else INK_FAINT
        grip(img, 127, y + 6, 24, line)
        grip(img, 6, y + 6, 18, line)
        draw_text_centered(img, 26, y + 5, 100, "PLAYLIST", ACCENT if active else INK_DIM)
        grip(img, 26, y + 6, 18, line)
        grip(img, 108, y + 6, 18, line)
        grip(img, 153, y + 6, 4, line)
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


def gen_font_row(img, y, chars, active):
    """`chars` as gen.bmp's font: each glyph in a cell one pixel wider than it, on the title's
    face, between separator columns that the engine scans row y + 2 for."""
    face = TITLE if active else TITLE_OFF
    x = 0
    img.vline(x, y, 7, EDGE)
    for c in chars:
        w = len(glyph(c)[0]) + 1
        img.fill(x + 1, y, w, 7, face)
        draw_glyph(img, x + 1, y + 1, c, ACCENT if active else INK_DIM)
        x += w + 1
        img.vline(x, y, 7, EDGE)


def gen_bmp():
    """gen.bmp: the frame of the visualizer, media library and plug-in windows, each piece
    where Winamp 5.666 reads it."""
    img = Canvas(194, 109, BG)
    # the title, active (y 0) and not (y 21): left corner, left of the name, under the name,
    # right of the name, the fill between, right corner with the close button
    for y, active in ((0, True), (21, False)):
        face = TITLE if active else TITLE_OFF
        line = ACCENT_DIM if active else INK_FAINT
        for x in (0, 26, 52, 78, 104, 130):
            frame_title_piece(img, x, y, 25, active)
        grip(img, 6, y + 6, 18, line)
        grip(img, 26, y + 6, 18, line)
        grip(img, 78 + 6, y + 6, 18, line)
        grip(img, 104, y + 6, 24, line)
        grip(img, 130, y + 6, 10, line)
        small_button(img, 130 + 14, y + 3, "close", face=face)
        # the left side's edge and rim start under the bar; so do the right side's
        img.vline(0, y + 15, 5, BG_HI)
        img.hline(0, y + 19, 10, BG)
        img.put(0, y + 19, BG_HI)
        img.hline(130 + 17, y + 19, 8, BG)
        img.put(130 + 17, y + 19, BG_HI)
        img.vline(130 + 24, y + 15, 5, EDGE)
    # the bottom bar's left and right ends: the well's bottom rim, the frame, the edge
    img.fill(0, 42, 125, 14, BG)
    img.vline(0, 42, 13, BG_HI)
    img.hline(11, 42, 114, BG_HI)
    img.put(10, 42, BG_LO)
    img.hline(0, 55, 125, EDGE)
    img.fill(0, 57, 125, 14, BG)
    img.hline(0, 57, 118, BG_HI)
    img.vline(124, 57, 14, EDGE)
    img.hline(0, 70, 125, EDGE)
    # the sides, 29 high, and the same again 24 high above the bottom bar
    for x, h in ((127, 29), (158, 24)):
        img.fill(x, 42, 11, h, BG)
        img.vline(x, 42, h, BG_HI)
        img.vline(x + 10, 42, h, BG_LO)
    for x, h in ((139, 29), (170, 24)):
        img.fill(x, 42, 8, h, BG)
        img.vline(x, 42, h, BG_HI)
        img.vline(x + 7, 42, h, EDGE)
    # the close button held
    small_button(img, 148, 42, "close", True)
    # the bottom bar's fill and the size grip
    img.fill(127, 72, 25, 14, BG)
    img.hline(127, 72, 25, BG_HI)
    img.hline(127, 85, 25, EDGE)
    img.fill(118, 72, 7, 7, BG)
    for r in range(7):
        for c in range(7):
            if r + c >= 6 and (r + c) % 2 == 0 and r % 2 == 0:
                img.put(118 + c, 72 + r, INK_FAINT)
    # the font: digits on rows 72 and 80, letters on rows 88 and 96
    digits = "0123456789-:"
    letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZ"
    gen_font_row(img, 72, digits, True)
    gen_font_row(img, 80, digits, False)
    gen_font_row(img, 88, letters, True)
    gen_font_row(img, 96, letters, False)
    return img


def genex_bmp():
    """genex.bmp: the button, scroll bar arrows and thumbs the media library and plug-ins
    use inside a gen window, and from x 48 of row 0 one color per two pixels, in wa_dlg.h's
    order. Unused space is cyan 00c6ff, as in Winamp's base skin; pixel 111 stays cyan, since
    a color equal to it counts as unset."""
    img = Canvas(130, 75, (0, 198, 255))
    # the button, at rest (y 0) and pressed (y 15), without a label
    img.raised(0, 0, 47, 15, BTN, BTN_HI, BTN_LO)
    img.raised(0, 15, 47, 15, BTN_DOWN, BTN_LO, BTN_HI)
    # the scroll bar's arrows, 14 x 14: up and down on row 31, left and right on row 45,
    # at rest from x 0 and pressed from x 28
    for x0, down in ((0, False), (28, True)):
        for i, kind in enumerate(("up", "down", "left", "right")):
            x, y = x0 + 14 * (i % 2), 31 + 14 * (i // 2)
            if down:
                img.raised(x, y, 14, 14, BTN_DOWN, BTN_LO, BTN_HI)
            else:
                img.raised(x, y, 14, 14, BTN, BTN_HI, BTN_LO)
            color = ACCENT if down else INK
            if kind in ("up", "down"):
                icon(img, kind, x + 7, y + 7, color)
            elif kind == "left":
                icon_triangle_left(img, x + 5, y + 4, color, 5)
            else:
                icon_triangle_right(img, x + 6, y + 4, color, 5)
    # the thumbs, at rest and held: vertical 14 x 28 at x 56 and 70, horizontal 28 x 14 at
    # rows 31 and 45
    for x, held in ((56, False), (70, True)):
        img.raised(x, 31, 14, 28, BTN_DOWN if held else BTN, BTN_HI, BTN_LO)
        for r in (41, 44, 47):
            img.hline(x + 4, r, 6, ACCENT if held else BTN_HI)
    for y, held in ((31, False), (45, True)):
        img.raised(84, y, 28, 14, BTN_DOWN if held else BTN, BTN_HI, BTN_LO)
        for c in (94, 97, 100):
            img.vline(c, y + 4, 6, ACCENT if held else BTN_HI)
    selected = (30, 76, 71)
    colors = (
        PANEL,  # item background
        INK,  # item text
        BG,  # window background
        INK,  # button text
        INK,  # window text
        BTN_HI,  # highlight: the frames round lists
        selected,  # selected item
        BTN,  # list header background
        INK,  # list header text
        BTN_HI,  # list header frame top
        BTN,  # list header frame middle
        BTN_LO,  # list header frame bottom
        BG,  # header past the last column
        BTN_HI,  # scroll bar thumb
        PANEL,  # scroll bar track
        BTN,  # inactive scroll bar thumb
        PANEL,  # inactive scroll bar track
        BG,  # corner between scroll bars
        INK,  # selection bar text
        selected,  # selection bar
        INK_DIM,  # inactive selection bar text
        BG_HI,  # inactive selection bar
        (16, 18, 22),  # every second row
        INK,  # every second row's text
    )
    for i, c in enumerate(colors):
        img.fill(48 + 2 * i, 0, 2, 1, c)
    return img


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
        "gen.bmp": bmp(gen_bmp()),
        "genex.bmp": bmp(genex_bmp()),
        "pledit.txt": pledit_txt(),
        "viscolor.txt": viscolor_txt(),
    }
    return files


# ---------------------------------------------------------------------------
# the modern skin

CLEAR = (0, 0, 0, 0)
MAIN_W, MAIN_H = 300, 112


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


# The playlist, visualizer and media library resize: a grid of frame pieces, the title in two
# halves either side of the middle, grips tiled out to them, and a size grip. At 300 wide they
# match the fixed windows pixel for pixel.
FRAMED = [
    # prefix, container's title, well color, default size
    ("pl", "PLAYLIST", "panel", 300, 180),
    ("vis", "VISUALIZATION", "black", 300, 180),
    ("ml", "MEDIA LIBRARY", "panel", 300, 232),
]
FRAME = {
    "frame.tl": (0, 0, 25, 22),
    "frame.t": (25, 0, 25, 22),
    "frame.tr": (50, 0, 25, 22),
    "frame.l": (75, 0, 8, 1),
    "frame.r": (83, 0, 8, 1),
    "frame.bl": (0, 22, 25, 8),
    "frame.b": (25, 22, 25, 8),
    "frame.br": (50, 22, 25, 8),
    "frame.m.panel": (91, 0, 1, 1),
    "frame.m.black": (92, 0, 1, 1),
    "frame.grip": (93, 0, 2, 3),
    "frame.resizer": (95, 0, 8, 8),
}


def frame_png():
    """The window as modern_window_bg draws it, cut into grid pieces: the border, the title
    bar, the frame and the rims of the well."""
    img = Canvas(103, 30, CLEAR)
    # the top: border, the title bar's highlight, face and line, the frame, the well's rim
    img.fill(0, 0, 75, 22, BG)
    img.hline(0, 0, 75, BG_HI)
    img.hline(0, 1, 75, BG_HI)
    img.fill(0, 2, 75, 13, TITLE)
    img.hline(0, 15, 75, EDGE)
    img.vline(0, 0, 22, BG_HI)
    img.vline(74, 0, 22, EDGE)
    img.hline(7, 21, 61, BG_LO)
    img.put(67, 21, BG_HI)
    # the sides
    img.fill(75, 0, 16, 1, BG)
    img.put(75, 0, BG_HI)
    img.put(82, 0, BG_LO)
    img.put(83, 0, BG_HI)
    img.put(90, 0, EDGE)
    # the bottom: the well's rim, the frame, the border
    img.fill(0, 22, 75, 8, BG)
    img.hline(7, 22, 61, BG_HI)
    img.vline(0, 22, 7, BG_HI)
    img.vline(74, 22, 8, EDGE)
    img.hline(0, 29, 75, EDGE)
    # the wells, the grip tile, the size grip
    img.put(91, 0, PANEL)
    img.put(92, 0, (0, 0, 0))
    img.put(93, 0, ACCENT_DIM)
    img.put(94, 2, ACCENT_DIM)
    # the size grip: the frame's bottom right corner, opaque so all of it takes the drag, with
    # three rows of dots
    img.fill(95, 0, 8, 8, BG)
    img.put(95, 0, BG_HI)
    img.vline(102, 0, 8, EDGE)
    img.hline(95, 7, 8, EDGE)
    for r, c in ((6, 6), (6, 4), (4, 6), (6, 2), (4, 4), (2, 6)):
        img.put(95 + c, r - 1, INK_FAINT)
    return img


def titles_png():
    """Each framed window's title, one per row of 5."""
    img = Canvas(max(text_width(t) for _, t, _, _, _ in FRAMED), 5 * len(FRAMED), CLEAR)
    for i, (_, title, _, _, _) in enumerate(FRAMED):
        draw_text(img, 0, 5 * i, title, ACCENT)
    return img


def framed_xml():
    """The elements, the title groups and the layouts of the framed windows."""
    elements = [
        f'    <bitmap id="{id_}" file="img/frame.png" x="{x}" y="{y}" w="{w}" h="{h}"/>'
        for id_, (x, y, w, h) in FRAME.items()
    ]
    groups, layouts = [], {}
    for i, (prefix, title, well, w, h) in enumerate(FRAMED):
        tw = text_width(title)
        # as title_strip centers the name in a strip from x 1 to w - 2
        left = (tw + 1) // 2
        right = tw - left
        elements.append(f'    <bitmap id="{prefix}.title.l" file="img/titles.png" x="0" y="{5 * i}"'
                        f' w="{left}" h="5"/>')
        elements.append(f'    <bitmap id="{prefix}.title.r" file="img/titles.png" x="{left}"'
                        f' y="{5 * i}" w="{right}" h="5"/>')
        groups.append(f'''  <groupdef id="{prefix}.title.left">
    <layer id="{prefix}.grip.left" x="21" y="7" w="-{21 + left + 6}" relatw="1" h="3" image="frame.grip" tile="1"/>
    <layer id="{prefix}.title.l" x="-{left}" relatx="1" y="6" image="{prefix}.title.l"/>
  </groupdef>
  <groupdef id="{prefix}.title.right">
    <layer id="{prefix}.title.r" x="0" y="6" image="{prefix}.title.r"/>
    <layer id="{prefix}.grip.right" x="{right + 6}" y="7" w="-{right + 6 + 47}" relatw="1" h="3" image="frame.grip" tile="1"/>
  </groupdef>''')
        layouts[prefix] = f'''    <layout id="normal" background="frame.m.{well}" w="{w}" h="{h}" minimum_w="300" minimum_h="116">
      <grid id="{prefix}.frame" x="0" y="0" w="0" h="0" relatw="1" relath="1"
        topleft="frame.tl" top="frame.t" topright="frame.tr" left="frame.l" middle="frame.m.{well}"
        right="frame.r" bottomleft="frame.bl" bottom="frame.b" bottomright="frame.br"/>
      <group id="{prefix}.title.left" x="0" y="0" w="50" relatw="%" h="17"/>
      <group id="{prefix}.title.right" x="50" relatx="%" y="0" w="50" relatw="%" h="17"/>
      <layer id="{prefix}.titlebar" x="0" y="0" w="-20" relatw="1" h="17" move="1"/>
      <button id="{prefix}.close" action="CLOSE" x="-14" relatx="1" y="4" image="close" downImage="close.down" tooltip="Close"/>
CONTENT
      <layer id="{prefix}.resize" x="-8" relatx="1" y="-8" relaty="1" image="frame.resizer" resize="bottomright"/>
    </layout>'''
    return "\n".join(elements), "\n".join(groups), layouts


EQ_W, EQ_H = 300, 112
# the sliders: preamp, then the ten bands, each 14 wide and 52 high; with the labels under
# them they leave 3 rows free at the top and the bottom of the well
EQ_SLIDER_Y, EQ_SLIDER_H = 42, 52
EQ_LABEL_Y = 96
# 15 pixels free left of the preamp and right of the last band
EQ_PREAMP_X = 23
EQ_BAND_X = [65 + 22 * i for i in range(10)]
EQ_LABELS = ["60", "170", "310", "600", "1K", "3K", "6K", "12K", "14K", "16K"]


def modern_eq_bg():
    img = Canvas(EQ_W, EQ_H, BG)
    img.edges(0, 0, EQ_W, EQ_H, BG_HI, EDGE)
    title_strip(img, 1, 1, EQ_W - 2, 15, "EQUALIZER", True)
    # the curve's well, then the sliders' well with a groove under each slider
    img.sunken(72, 20, 220, 14)
    img.sunken(7, 38, 286, 67)
    for x in [EQ_PREAMP_X] + EQ_BAND_X:
        img.vline(x + 7, EQ_SLIDER_Y + 2, EQ_SLIDER_H - 4, GROOVE)
    # +12, 0 and -12 centered on the thumb at the top, middle and bottom of its travel
    travel = EQ_SLIDER_H - 7
    for label, at in (("+12", 0), ("0", travel // 2), ("-12", travel)):
        y = EQ_SLIDER_Y + at + 3 - 2
        draw_text(img, 57 - text_width(label), y, label, INK_DIM)
    draw_text_centered(img, EQ_PREAMP_X, EQ_LABEL_Y, 14, "PRE", INK_DIM)
    for x, label in zip(EQ_BAND_X, EQ_LABELS):
        draw_text_centered(img, x, EQ_LABEL_Y, 14, label, INK_DIM)
    return img


def modern_led_button(label, w, h, state):
    """The equalizer's on and auto buttons: off, on (`lit`), or held."""
    img = Canvas(w, h, CLEAR)
    button(img, 0, 0, w, h, label=label, down=state == "down", lit=state == "lit")
    return img


# Where each wasabi base element sits on img/wasabi.png: (x, y, w, h). The skin lays them over
# the base's own, so the windows Winamp frames itself (plug-ins, the base's dialogs) and the
# lists, buttons and scroll bars inside them match the skin.
WASABI = {
    "wasabi.frame.top.left": (0, 0, 15, 15),
    "wasabi.frame.top": (15, 0, 15, 15),
    "wasabi.frame.top.right": (30, 0, 15, 15),
    "wasabi.frame.left": (45, 0, 5, 15),
    "wasabi.frame.right": (50, 0, 5, 15),
    "wasabi.frame.bottom.left.top": (0, 15, 15, 2),
    "wasabi.frame.bottom.left": (0, 17, 15, 1),
    "wasabi.frame.bottom.left.bottom": (0, 18, 15, 2),
    "wasabi.frame.bottom.top": (15, 15, 15, 2),
    "wasabi.frame.bottom": (15, 17, 15, 1),
    "wasabi.frame.bottom.bottom": (15, 18, 15, 2),
    "wasabi.frame.bottom.right.top": (30, 15, 15, 2),
    "wasabi.frame.bottom.right": (30, 17, 15, 1),
    "wasabi.frame.bottom.right.bottom": (30, 18, 15, 2),
    "wasabi.frame.resizer": (55, 0, 12, 12),
    "wasabi.titlebar.left.active": (0, 20, 7, 9),
    "wasabi.titlebar.center.active": (7, 20, 7, 9),
    "wasabi.titlebar.right.active": (14, 20, 7, 9),
    "wasabi.titlebar.left.inactive": (0, 29, 7, 9),
    "wasabi.titlebar.center.inactive": (7, 29, 7, 9),
    "wasabi.titlebar.right.inactive": (14, 29, 7, 9),
    "wasabi.button.sysmenu": (21, 20, 14, 9),
    "wasabi.button.sysmenu.pressed": (21, 29, 14, 9),
    "wasabi.button.close": (35, 20, 14, 9),
    "wasabi.button.close.pressed": (35, 29, 14, 9),
    "wasabi.scrollbar.vertical.grip": (153, 40, 6, 8),
    "wasabi.scrollbar.horizontal.grip": (160, 40, 8, 6),
    "wasabi.scrollbar.horizontal.background.left": (153, 50, 5, 5),
    "wasabi.scrollbar.horizontal.background.middle": (158, 50, 5, 5),
    "wasabi.scrollbar.horizontal.background.right": (163, 50, 5, 5),
    "wasabi.scrollbar.vertical.background.top": (153, 55, 5, 5),
    "wasabi.scrollbar.vertical.background.middle": (158, 55, 5, 5),
    "wasabi.scrollbar.vertical.background.bottom": (163, 55, 5, 5),
    "wasabi.list.background": (80, 80, 17, 17),
    "wasabi.tree.background": (80, 80, 17, 17),
    "wasabi.window.top.left": (100, 80, 1, 1),
    "wasabi.window.top": (101, 80, 17, 1),
    "wasabi.window.top.right": (118, 80, 1, 1),
    "wasabi.window.left": (100, 81, 1, 17),
    "wasabi.window.center": (101, 81, 17, 17),
    "wasabi.window.right": (118, 81, 1, 17),
    "wasabi.window.bottom.left": (100, 98, 1, 1),
    "wasabi.window.bottom": (101, 98, 17, 1),
    "wasabi.window.bottom.right": (118, 98, 1, 1),
}
# the scroll bar's arrows and thumbs, each at rest, under the mouse and held
for i, state in enumerate(("", ".hover", ".pressed")):
    WASABI[f"wasabi.scrollbar.horizontal.left{state}"] = (0, 40 + 13 * i, 17, 13)
    WASABI[f"wasabi.scrollbar.horizontal.right{state}"] = (17, 40 + 13 * i, 17, 13)
    WASABI[f"wasabi.scrollbar.horizontal.button{state}"] = (34, 40 + 13 * i, 41, 13)
    WASABI[f"wasabi.scrollbar.vertical.left{state}"] = (75 + 13 * i, 40, 13, 17)
    WASABI[f"wasabi.scrollbar.vertical.right{state}"] = (75 + 13 * i, 57, 13, 17)
    WASABI[f"wasabi.scrollbar.vertical.button{state}"] = (114 + 13 * i, 40, 13, 41)
# the framed button, at rest and pressed: 4 pixel corners round a 25 pixel middle
for prefix, x0 in (("wasabi.button.", 0), ("wasabi.button.pressed.", 40)):
    for name, (x, y, w, h) in {
        "top.left": (0, 0, 4, 4), "top": (4, 0, 25, 4), "top.right": (29, 0, 4, 4),
        "left": (0, 4, 4, 25), "middle": (4, 4, 25, 25), "right": (29, 4, 4, 25),
        "bottom.left": (0, 29, 4, 4), "bottom": (4, 29, 25, 4), "bottom.right": (29, 29, 4, 4),
    }.items():
        WASABI[prefix + name] = (x0 + x, 80 + y, w, h)
WASABI["wasabi.button.pressed.center"] = WASABI["wasabi.button.pressed.middle"]


def wasabi_png():
    img = Canvas(170, 115, CLEAR)
    # the standard frame: the title bar's 15 rows, the sides, the bottom in three bands
    for x in (0, 15, 30):
        img.fill(x, 0, 15, 14, TITLE)
        img.hline(x, 0, 15, BG_HI)
        img.hline(x, 14, 15, EDGE)
        img.fill(x, 15, 15, 5, BG)
    img.vline(0, 0, 14, BG_HI)
    img.vline(44, 0, 15, EDGE)
    img.fill(45, 0, 10, 15, BG)
    img.vline(45, 0, 15, BG_HI)
    img.vline(49, 0, 15, BG_LO)
    img.vline(50, 0, 15, BG_HI)
    img.vline(54, 0, 15, EDGE)
    img.hline(5, 15, 25, BG_HI)
    img.hline(30, 15, 11, BG_HI)
    img.vline(0, 15, 5, BG_HI)
    img.vline(44, 15, 5, EDGE)
    img.hline(0, 19, 45, EDGE)
    img.put(4, 15, BG_LO)
    # the size grip
    for r in range(12):
        for c in range(12):
            if r + c >= 11 and (r + c) % 3 == 0 and r % 3 == 2:
                img.put(55 + c, r, INK_FAINT)
    # the title's streaks either side of the name, active and not
    for y, line in ((20, ACCENT_DIM), (29, INK_FAINT)):
        img.fill(0, y, 21, 9, CLEAR)
        img.hline(1, y + 3, 20, line)
        img.hline(1, y + 5, 20, line)
    # the window menu and close buttons
    for y, down in ((20, False), (29, True)):
        for x, kind in ((21, "menu"), (35, "close")):
            img.round_rect(x, y, 14, 9, ACCENT_DARK if down else BTN_HI)
            icon(img, kind, x + 7, y + 4, ACCENT if down else INK)
    # the scroll bars
    faces = ((BTN, BTN_HI, BTN_LO, INK), (BTN_HI, BTN_HI, BTN_LO, INK),
             (BTN_DOWN, BTN_LO, BTN_HI, ACCENT))
    for i, (face, hi, lo, mark) in enumerate(faces):
        y = 40 + 13 * i
        for x, w, kind in ((0, 17, "left"), (17, 17, "right"), (34, 41, None)):
            img.raised(x, y, w, 13, face, hi, lo)
            if kind == "left":
                icon_triangle_left(img, x + 7, y + 4, mark, 5)
            elif kind == "right":
                icon_triangle_right(img, x + 8, y + 4, mark, 5)
        x = 75 + 13 * i
        for y, kind in ((40, "up"), (57, "down")):
            img.raised(x, y, 13, 17, face, hi, lo)
            icon(img, kind, x + 6, y + 8, mark)
        img.raised(114 + 13 * i, 40, 13, 41, face, hi, lo)
    for r in (1, 4, 7):
        img.hline(153, 40 + r, 6, INK_DIM)
    for c in (1, 4, 7):
        img.vline(160 + c, 40, 6, INK_DIM)
    img.fill(153, 50, 15, 10, PANEL)
    # the framed button
    for x, face, hi, lo in ((0, BTN, BTN_HI, BTN_LO), (40, BTN_DOWN, BTN_LO, BTN_HI)):
        img.fill(x, 80, 33, 33, face)
        img.edges(x, 80, 33, 33, hi, lo)
        for cx, cy in ((x, 80), (x + 32, 80), (x, 112), (x + 32, 112)):
            img.put(cx, cy, CLEAR)
    # a list's background, and a window's: a dark line round the window's color
    img.fill(80, 80, 17, 17, PANEL)
    img.fill(100, 80, 19, 19, BG_LO)
    img.fill(101, 81, 17, 17, BG)
    return img


def title_font_png():
    """The standard frame's title font, as Winamp's text.bmp lays out its letters: cells 5 wide
    and 11 high, the letter on rows 3 to 7 so it centers on the title bar's buttons."""
    img = Canvas(155, 33, CLEAR)
    rows = [
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ\"@",
        "0123456789….:()-'!_+\\/[]^&%,=$#",
        "ÅÖÄ?*",
    ]
    for r, chars in enumerate(rows):
        for i, c in enumerate(chars):
            draw_glyph(img, i * 5, r * 11 + 3, c, ACCENT)
    return img


def wasabi_elements_xml():
    lines = [
        '    <bitmapfont id="studio.component.title" file="img/title-font.png" charwidth="5"'
        ' charheight="11" hspacing="0" vspacing="0"/>'
    ]
    for id_, (x, y, w, h) in WASABI.items():
        lines.append(
            f'    <bitmap id="{id_}" file="img/wasabi.png" x="{x}" y="{y}" w="{w}" h="{h}"/>'
        )
    return "\n".join(lines)


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
<!-- walkit's modern example skin: the main window, equalizer, playlist, visualizer and
     media library, 300 pixels wide. -->
<WinampAbstractionLayer version="0.8">
  <skininfo>
    <version>1.0</version>
    <name>walkit modern</name>
    <comment>A flat dark example skin for walkit</comment>
    <author>walkit</author>
  </skininfo>

  <elements>
    <bitmap id="main.bg" file="img/main-bg.png"/>
FRAME_ELEMENTS
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
    <bitmap id="eq" file="img/eq.png"/>
    <bitmap id="eq.down" file="img/eq-down.png"/>
    <bitmap id="eq.bg" file="img/eq-bg.png"/>
    <bitmap id="eq.on" file="img/eq-on.png"/>
    <bitmap id="eq.on.lit" file="img/eq-on-lit.png"/>
    <bitmap id="eq.on.down" file="img/eq-on-down.png"/>
    <bitmap id="eq.auto" file="img/eq-auto.png"/>
    <bitmap id="eq.auto.lit" file="img/eq-auto-lit.png"/>
    <bitmap id="eq.auto.down" file="img/eq-auto-down.png"/>
    <bitmap id="eq.thumb" file="img/eq-thumb.png"/>
    <bitmap id="eq.thumb.down" file="img/eq-thumb-down.png"/>
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
    <color id="wasabi.list.item.selected" value="30,76,71"/>
    <color id="wasabi.list.item.focused" value="30,76,71"/>
    <color id="wasabi.list.item.selected.fg" value="216,222,228"/>
    <color id="wasabi.list.column.background" value="48,53,61"/>
    <color id="wasabi.list.column.text" value="216,222,228"/>
    <color id="wasabi.list.column.separator" value="22,24,28"/>
    <color id="wasabi.tree.text" value="174,186,196"/>
    <color id="wasabi.tree.selected" value="30,76,71"/>
    <color id="wasabi.tree.hiliteddrop" value="22,96,88"/>
    <color id="wasabi.itemlist.outline.current" value="45,212,191"/>
    <color id="wasabi.itemlist.outline.focus" value="22,96,88"/>
    <color id="wasabi.itemlist.selborder" value="22,96,88"/>
    <color id="wasabi.button.text" value="216,222,228"/>
    <color id="wasabi.button.hiliteText" value="45,212,191"/>
    <color id="wasabi.button.dimmedText" value="112,120,131"/>
    <color id="wasabi.edit.text" value="216,222,228"/>
    <color id="wasabi.edit.background" value="12,14,17"/>
    <color id="wasabi.edit.selection" value="30,76,71"/>
    <color id="wasabi.text.color" value="216,222,228"/>
    <color id="wasabi.labelwnd.foreground" value="216,222,228"/>
    <color id="wasabi.labelwnd.background" value="30,33,38"/>
    <color id="wasabi.component.title.foreground" value="45,212,191"/>
    <color id="wasabi.component.title.border" value="30,33,38"/>
    <!-- the wasabi base's frame, title bar, buttons and scroll bars, in this skin's look -->
WASABI_ELEMENTS
  </elements>

FRAME_GROUPS

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
      <button id="eqtoggle" action="TOGGLE" param="equalizer" x="144" y="84" image="eq" downImage="eq.down" tooltip="Equalizer"/>
      <button id="pltoggle" action="TOGGLE" param="pledit" x="166" y="84" image="pl" downImage="pl.down" tooltip="Playlist"/>

      <slider id="volume" action="VOLUME" x="204" y="88" w="88" h="12"
        thumb="volume.thumb" downThumb="volume.thumb.down"/>
    </layout>
  </container>

  <container id="equalizer" name="Equalizer" default_x="0" default_y="112" default_visible="1">
    <layout id="normal" background="eq.bg" w="300" h="112">
      <layer id="eq.titlebar" x="0" y="0" w="280" h="17" move="1"/>
      <button id="eq.close" action="CLOSE" x="286" y="4" image="close" downImage="close.down" tooltip="Close"/>
      <button id="eq.on" action="EQ_TOGGLE" x="8" y="21" image="eq.on" activeImage="eq.on.lit"
        downImage="eq.on.down" tooltip="Equalizer on/off"/>
      <button id="eq.auto" action="EQ_AUTO" x="36" y="21" image="eq.auto" activeImage="eq.auto.lit"
        downImage="eq.auto.down" tooltip="Load presets with each song"/>
      <eqvis id="eq.curve" x="74" y="22" w="216" h="10" colortop="200,255,245"
        colormiddle="45,212,191" colorbottom="22,96,88" colorpreamp="112,120,131"/>
      <slider id="eq.preamp" action="EQ_BAND" param="preamp" x="23" y="42" w="14" h="52"
        orientation="vertical" thumb="eq.thumb" downThumb="eq.thumb.down" tooltip="Preamp"/>
      <slider id="eq.1" action="EQ_BAND" param="1" x="65" y="42" w="14" h="52"
        orientation="vertical" thumb="eq.thumb" downThumb="eq.thumb.down" tooltip="60 Hz"/>
      <slider id="eq.2" action="EQ_BAND" param="2" x="87" y="42" w="14" h="52"
        orientation="vertical" thumb="eq.thumb" downThumb="eq.thumb.down" tooltip="170 Hz"/>
      <slider id="eq.3" action="EQ_BAND" param="3" x="109" y="42" w="14" h="52"
        orientation="vertical" thumb="eq.thumb" downThumb="eq.thumb.down" tooltip="310 Hz"/>
      <slider id="eq.4" action="EQ_BAND" param="4" x="131" y="42" w="14" h="52"
        orientation="vertical" thumb="eq.thumb" downThumb="eq.thumb.down" tooltip="600 Hz"/>
      <slider id="eq.5" action="EQ_BAND" param="5" x="153" y="42" w="14" h="52"
        orientation="vertical" thumb="eq.thumb" downThumb="eq.thumb.down" tooltip="1 kHz"/>
      <slider id="eq.6" action="EQ_BAND" param="6" x="175" y="42" w="14" h="52"
        orientation="vertical" thumb="eq.thumb" downThumb="eq.thumb.down" tooltip="3 kHz"/>
      <slider id="eq.7" action="EQ_BAND" param="7" x="197" y="42" w="14" h="52"
        orientation="vertical" thumb="eq.thumb" downThumb="eq.thumb.down" tooltip="6 kHz"/>
      <slider id="eq.8" action="EQ_BAND" param="8" x="219" y="42" w="14" h="52"
        orientation="vertical" thumb="eq.thumb" downThumb="eq.thumb.down" tooltip="12 kHz"/>
      <slider id="eq.9" action="EQ_BAND" param="9" x="241" y="42" w="14" h="52"
        orientation="vertical" thumb="eq.thumb" downThumb="eq.thumb.down" tooltip="14 kHz"/>
      <slider id="eq.10" action="EQ_BAND" param="10" x="263" y="42" w="14" h="52"
        orientation="vertical" thumb="eq.thumb" downThumb="eq.thumb.down" tooltip="16 kHz"/>
    </layout>
  </container>

  <container id="pledit" name="Playlist Editor" default_x="0" default_y="224" default_visible="1">
LAYOUT_PL
      <component id="pl.list" x="8" y="22" w="-16" h="-30" relatw="1" relath="1"
        param="guid:{45F3F7C1-A6F3-4ee6-A15E-125E92FC3F8D}"/>
    </layout>
  </container>
  <container id="avs" name="Visualization" dynamic="1" component="guid:{0000000A-000C-0010-FF7B-01014263450C}"
    default_x="0" default_y="404" default_visible="0">
LAYOUT_VIS
      <component id="vis.holder" x="8" y="22" w="-16" h="-30" relatw="1" relath="1" noshowcmdbar="1" autoopen="1" autoclose="1"
        param="guid:{0000000A-000C-0010-FF7B-01014263450C}"/>
    </layout>
  </container>

  <container id="ml" name="Media Library" dynamic="1" component="guid:{6B0EDF80-C9A5-11D3-9F26-00C04F39FFC6}"
    default_x="300" default_y="0" default_visible="0">
LAYOUT_ML
      <component id="ml.list" x="8" y="22" w="-16" h="-30" relatw="1" relath="1" noshowcmdbar="1" autoopen="1" autoclose="1"
        param="guid:{6B0EDF80-C9A5-11D3-9F26-00C04F39FFC6}"/>
    </layout>
  </container>
</WinampAbstractionLayer>
"""


def skin_xml():
    """SKIN_XML with the wasabi elements and the framed windows filled in: each LAYOUT_<PREFIX>
    line and the content under it, up to its </layout>, becomes that window's layout."""
    elements, groups, layouts = framed_xml()
    xml = SKIN_XML.replace("WASABI_ELEMENTS", wasabi_elements_xml())
    xml = xml.replace("FRAME_ELEMENTS", elements).replace("FRAME_GROUPS", groups)
    for prefix, layout in layouts.items():
        start = xml.index(f"LAYOUT_{prefix.upper()}\n")
        end = xml.index("    </layout>\n", start) + len("    </layout>\n")
        content = xml[start + len(f"LAYOUT_{prefix.upper()}\n"):end - len("    </layout>\n")]
        xml = xml[:start] + layout.replace("CONTENT\n", content) + "\n" + xml[end:]
    return xml


def modern_files():
    files = {
        "skin.xml": skin_xml().encode(),
        "img/main-bg.png": png(modern_main_bg()),
        "img/eq-bg.png": png(modern_eq_bg()),
        "img/frame.png": png(frame_png()),
        "img/titles.png": png(titles_png()),
        "img/wasabi.png": png(wasabi_png()),
        "img/title-font.png": png(title_font_png()),
        "img/eq.png": png(modern_text_button("EQ", 20, 18, False)),
        "img/eq-down.png": png(modern_text_button("EQ", 20, 18, True)),
        "img/eq-thumb.png": png(modern_thumb(14, 7, ACCENT)),
        "img/eq-thumb-down.png": png(modern_thumb(14, 7, INK)),
        "img/close.png": png(modern_close(False)),
        "img/close-down.png": png(modern_close(True)),
        "img/pl.png": png(modern_text_button("PL", 20, 18, False)),
        "img/pl-down.png": png(modern_text_button("PL", 20, 18, True)),
        "img/seek-thumb.png": png(modern_thumb(24, 10, ACCENT)),
        "img/seek-thumb-down.png": png(modern_thumb(24, 10, INK)),
        "img/volume-thumb.png": png(modern_thumb(10, 12, ACCENT)),
        "img/volume-thumb-down.png": png(modern_thumb(10, 12, INK)),
    }
    for label, name, w in (("ON", "eq-on", 26), ("AUTO", "eq-auto", 34)):
        files[f"img/{name}.png"] = png(modern_led_button(label, w, 12, "off"))
        files[f"img/{name}-lit.png"] = png(modern_led_button(label, w, 12, "lit"))
        files[f"img/{name}-down.png"] = png(modern_led_button(label, w, 12, "down"))
    for kind in ("prev", "play", "pause", "stop", "next"):
        files[f"img/{kind}.png"] = png(modern_button(kind, 24, 18, False))
        files[f"img/{kind}-down.png"] = png(modern_button(kind, 24, 18, True))
    return files


def main():
    write_zip(os.path.join(HERE, "walkit-classic.wsz"), classic_files())
    write_zip(os.path.join(HERE, "walkit-modern.wal"), modern_files())


if __name__ == "__main__":
    main()
