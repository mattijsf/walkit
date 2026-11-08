// SPDX-License-Identifier: GPL-3.0-or-later

package nl.mattix.walkit

/**
 * A TrueType font drawn as Windows draws a classic skin's text: outlines hinted to the pixel grid,
 * each pixel either ink or clear, advances in whole pixels and no kerning, as GDI's `DrawText`
 * lays out a line. Sizes are ems in pixels; a host enlarges what [draw] gives unfiltered.
 *
 * Made from a font file's bytes; null from [of] when they are not a font. One thread at a time.
 */
public class HintedFont private constructor(
    private var handle: Long,
) : AutoCloseable {
    /** Whether the font has a glyph for every character of [text] but spaces. */
    public fun covers(text: String): Boolean = Native.hfCovers(h(), text)

    /** How wide [text] draws at an em of [em] pixels, in whole pixels. */
    public fun width(
        text: String,
        em: Float,
    ): Int = Native.hfWidth(h(), text, em)

    /**
     * [text] at an em of [em] pixels in [argb] on clear pixels: a block [w] x [h] pixels, rows top
     * first, the line starting at [x] with its baseline at [baseline].
     */
    @Suppress("LongParameterList") // the text, its color and the block it goes in
    public fun draw(
        text: String,
        em: Float,
        argb: Int,
        w: Int,
        h: Int,
        x: Int,
        baseline: Int,
    ): IntArray = Native.hfDraw(h(), text, em, argb, w, h, x, baseline)

    override fun close() {
        if (handle != 0L) Native.hfFree(handle)
        handle = 0
    }

    private fun h(): Long = handle.also { check(it != 0L) { "font closed" } }

    public companion object {
        /** The font in [data], a TrueType or OpenType file; null when it is not one. */
        public fun of(data: ByteArray): HintedFont? = Native.hfNew(data).takeIf { it != 0L }?.let(::HintedFont)
    }
}
