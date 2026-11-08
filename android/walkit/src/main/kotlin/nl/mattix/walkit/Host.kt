// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

/** The pictures of a skin as the host decodes them: sizes and pixels, for shapes and taps. */
public interface ImageHost {
    public fun imageSize(file: String): Pair<Int, Int>?

    /** ARGB of one pixel of a skin image, or null when it cannot be read. */
    public fun pixel(
        file: String,
        x: Int,
        y: Int,
    ): Int? = null

    /**
     * ARGB of the [w] x [h] block of a skin image at [x], [y], row by row, or null when a pixel of
     * it cannot be read; one [pixel] at a time unless the host reads a block at once.
     */
    public fun pixels(
        file: String,
        x: Int,
        y: Int,
        w: Int,
        h: Int,
    ): IntArray? = IntArray(w * h) { i -> pixel(file, x + i % w, y + i / w) ?: return null }
}

/** Text as the host's renderer draws it: widths, and the font's line where its file gives none. */
public interface TextHost {
    public fun textWidth(
        text: String,
        font: FontDef?,
        size: Int,
    ): Int

    public fun textWidth(
        text: String,
        font: FontDef?,
        size: Int,
        bold: Boolean,
    ): Int = textWidth(text, font, size)

    /**
     * How wide [text] draws, in fractional layout pixels, as this host's renderer draws it: the
     * engine places text by it. The whole-pixel [textWidth] by default.
     */
    public fun textAdvance(
        text: String,
        font: FontDef?,
        size: Int,
        bold: Boolean,
    ): Float = textWidth(text, font, size, bold).toFloat()

    /**
     * Ascent and descent in layout pixels as this host's renderer lays the font out, for a font
     * whose file gives Windows' line no figures; null leaves it to a share of the em.
     */
    public fun lineMetrics(
        font: FontDef?,
        size: Int,
        bold: Boolean,
    ): Pair<Float, Float>? = null
}

/** The rest of what the engine asks of the host: the log, redraws, the app's windows, menus and actions. */
public interface WindowHost {
    public fun log(message: String)

    public fun invalidate()

    /**
     * Whether one of the app's windows is open: "pl" (playlist), "ml" (library), "eq",
     * "video" or "vis". Null leaves it to the skin's container. Buttons that toggle these light up.
     */
    public fun windowOpen(kind: String): Boolean? = null

    /**
     * Shows a skin's popup menu and returns the id of the command the listener picked, or 0 for
     * none. The script waits for it, so a host that shows menus blocks here until the answer. Null
     * means the menu closed without the listener (the app went away or the wait ran out): the
     * script hears 0, and the touch shows no more menus.
     */
    public fun chooseFromMenu(menu: SkinMenu): Int? = 0

    /**
     * The windows on screen changed ([SkinRuntime.windowIds], main first): one opened, closed or
     * switched layout (a shade). A host that shows each window apart makes or drops its windows here.
     */
    public fun windowsChanged(windows: List<WindowId>): Unit = Unit

    /**
     * A button action the host answers, as Winamp answered it outside the skin, when the skin was
     * set up with [SkinSetup.hostActions]: the action uppercase, its param or window name, and
     * where on screen it came from ([ActionFrom]) for a menu it opens; null when nothing on screen
     * sent it.
     */
    public fun hostAction(
        action: String,
        param: String?,
        from: ActionFrom?,
    ): Unit = Unit
}

/** Services the app supplies to the skin: its pictures, its text and its windows. */
public interface SkinHost :
    ImageHost,
    TextHost,
    WindowHost

/** A control a finger can press, as a host's magnifier lists them ([SkinRuntime.pressControls]). */
public data class TouchControl(
    /** Where it answers, in stack pixels. */
    val rect: Rect,
    /** Whether a hold on it asks for the magnifier whatever its size: one of a stack of look-alike pictures, such as a classic playlist menu's. */
    val magnify: Boolean,
)

/**
 * Where a host action came from on screen, in the stack's layout pixels (as pointer events take
 * them): a menu it opens can go against the thing that sent it, or at the finger.
 */
public data class ActionFrom(
    /** The thing that sent it: the button, a classic window's control, or the point a long press held as a 1 x 1 box. */
    val rect: Rect,
    /** Where the finger went down for it, x to y; null when no touch sent it (a script's timer). */
    val point: Pair<Int, Int>?,
)
