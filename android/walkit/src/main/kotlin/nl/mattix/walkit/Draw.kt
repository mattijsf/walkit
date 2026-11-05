// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

/** A rectangle in layout pixels. */
public data class Rect(
    val left: Int,
    val top: Int,
    val width: Int,
    val height: Int,
) {
    val right: Int get() = left + width
    val bottom: Int get() = top + height
}

/**
 * A part of a skin image: the element [id] (lowercase) in the skin's XML, the [file] and the source
 * rectangle inside it, or null size for the whole file, and the gamma group the color theme tints it by.
 */
public class BitmapDef(
    public val id: String,
    public val file: String,
    public val x: Int,
    public val y: Int,
    public val w: Int?,
    public val h: Int?,
    public val gammaGroup: String? = null,
)

/**
 * A color theme's tint for one gamma group: per-channel factors in 1/4096 steps around 1, an
 * optional grey conversion first (1 = brightest channel, 2 = average). `boost` brightens before
 * the tint (see [apply]), with or without a tint.
 */
public data class Gamma(
    val r: Int,
    val g: Int,
    val b: Int,
    val boost: Int,
    val gray: Int,
) {
    // a boost brightens even with no tint: MMD3's "Thinger" group is 0,0,0 with boost 1 in its
    // default theme, and Winamp 5.666 draws the component bucket lifted there
    val isIdentity: Boolean get() = r == 0 && g == 0 && b == 0 && gray == 0 && boost == 0

    /** [argb] tinted, as the engine tints a picture of this group. */
    public fun apply(argb: Int): Int {
        var red = (argb shr 16) and 0xff
        var green = (argb shr 8) and 0xff
        var blue = argb and 0xff
        if (gray != 0) {
            red = if (gray == 2) (red + green + blue) / 3 else maxOf(red, green, blue)
            green = red
            blue = red
        }

        // boost brightens before the tint. 1 squeezes the pixel into the upper half (c / 2 + 128:
        // Winamp 5.666's black menu text under MonoDark and MMD3's green labels under "silver1 |
        // xblue" both fit); 2 adds 32 and quadruples, as Webamp Modern does (only seen with
        // near-black tints, where it matches)
        fun lifted(c: Int): Double =
            when (boost) {
                1 -> c / 2.0 + 128
                2 -> (c + 32) * 4.0
                else -> c.toDouble()
            }

        fun ch(
            c: Int,
            f: Int,
        ) = (lifted(c) * (1.0 + f / 4096.0)).toInt().coerceIn(0, 255)
        return (argb and 0xff000000.toInt()) or (ch(red, r) shl 16) or (ch(green, g) shl 8) or ch(blue, b)
    }
}

/** A font a skin declares: its [id], and the TrueType [file] in the skin, empty for a system font. */
public class FontDef(
    public val id: String,
    public val file: String,
)

/** RGB color as 0xRRGGBB. */
@JvmInline
public value class Rgb(
    public val value: Int,
)

/** What the renderer draws, in order, in layout pixels; the engine hands a frame of these per window. */
public sealed class DrawCmd {
    /** A bitmap, or the part of it [bitmap] names, stretched to [dst]. */
    public class Image(
        public val bitmap: BitmapDef,
        public val dst: Rect,
        public val alpha: Int,
        /** The color theme's tint for this bitmap, filled in when the frame is drawn. */
        public val gamma: Gamma? = null,
        /** Scale with filtering instead of nearest pixels: Winamp shrinks a list's thumb picture that way. */
        public val smooth: Boolean = false,
    ) : DrawCmd()

    /** A [rect] filled with [color] at [alpha], 0..255. */
    public class Fill(
        public val rect: Rect,
        public val color: Rgb,
        public val alpha: Int,
    ) : DrawCmd()

    /**
     * Start of a shaped window: what follows until [WindowEnd] shows only inside [region]. Renderers
     * evaluate it with [Region.eval].
     */
    public class WindowStart(
        public val region: Region,
    ) : DrawCmd()

    /** End of the shaped window [WindowStart] began: one shared object. */
    public object WindowEnd : DrawCmd()

    /**
     * Whether [other] draws exactly what this draws: the same kind of command with the same values,
     * the same part of the same picture in the same tint, the same font, and pixels with the same
     * content. A renderer may then keep what it drew for one and show it for the other. A window's
     * shape is the same when it is made of the same pictures, boxes and operations, whether or not
     * the engine worked it out again.
     */
    internal fun drawsSameAs(other: DrawCmd): Boolean =
        when (this) {
            is Image -> other is Image && sameAs(other)
            is Fill -> other is Fill && rect == other.rect && color == other.color && alpha == other.alpha
            is WindowStart -> other is WindowStart && region.sameShapeAs(other.region)
            WindowEnd -> other === WindowEnd
            is Text -> other is Text && sameAs(other)
            is BitmapText -> other is BitmapText && text == other.text && rect == other.rect
            is Clip -> other is Clip && rect == other.rect
            Unclip -> other === Unclip
            is Pixels -> other is Pixels && sameAs(other)
        }

    /**
     * A run of [text] in [font] at [size], drawn at [x] and [baseline] in [color] at [alpha], with a
     * [shadow] when set. [rect] is the box it is clipped to.
     */
    @Suppress("LongParameterList") // a run of text: its words, font, color, box and shadow
    public class Text internal constructor(
        public val text: String,
        public val font: FontDef?,
        public val size: Int,
        public val color: Rgb,
        public val rect: Rect,
        public val alpha: Int,
        public val bold: Boolean,
        public val shadow: Rgb?,
        /** Where the shadow sits from the text. */
        public val shadowDx: Int,
        public val shadowDy: Int,
        /** Where the text starts across, in layout pixels: aligned in [rect] by its measured width, less a ticker's scroll. */
        public val x: Float,
        /** The baseline, in layout pixels: placed in [rect] by the font's line. */
        public val baseline: Float,
    ) : DrawCmd()

    /**
     * Text in a skin's bitmap font. The engine spells it out as [Clip] and [Image] commands before a
     * host sees the list, so a renderer draws nothing for it.
     */
    public class BitmapText internal constructor(
        internal val text: String,
        internal val rect: Rect,
    ) : DrawCmd()

    /** Clip the commands that follow, until the matching [Unclip]. */
    public class Clip(
        public val rect: Rect,
    ) : DrawCmd()

    /** The end of the last [Clip]; like [WindowEnd], one shared object. */
    public object Unclip : DrawCmd()

    /**
     * A small ARGB picture ([w] x [h], rows top first) stretched over [rect] with nearest pixels: the
     * vis, the EQ graph and gradients. [stable] means the same picture comes back unchanged in
     * later frames until the picture changes, so a host may keep what it made from it.
     */
    public class Pixels(
        public val rect: Rect,
        public val w: Int,
        public val h: Int,
        public val argb: IntArray,
        public val alpha: Int,
        public val stable: Boolean = false,
    ) : DrawCmd()

    public companion object {
        /**
         * Whether a pixel of a region picture marks a cut: opaque, whatever its color. Winamp
         * Modern's main window marks its corners in black, its shaded playlist in white.
         */
        public fun isCutPixel(argb: Int): Boolean = (argb ushr 24) >= 128
    }
}

/**
 * A window's shape as Winamp composes it: a picture's opaque pixels, a rectangle, or an
 * object's own region with its children's added, intersected and cut away.
 */
public sealed class Region {
    /** The opaque pixels of [image]. */
    public class Pic(
        public val image: DrawCmd.Image,
    ) : Region()

    /** The whole of [rect]. */
    public class Box(
        public val rect: Rect,
    ) : Region()

    /** [base] (nothing when null), kept only where [keep] is when that is not empty, plus [add], less [cut]. */
    public class Op(
        public val base: Region?,
        public val add: List<Region>,
        public val keep: List<Region>,
        public val cut: List<Region>,
    ) : Region()

    /** Every picture in it. */
    internal fun pics(): List<DrawCmd.Image> =
        when (this) {
            is Pic -> listOf(image)
            is Box -> emptyList()
            is Op -> (listOfNotNull(base) + add + keep + cut).flatMap { it.pics() }
        }

    /** The box around it, or null when it holds nothing. */
    public fun bounds(): Rect? =
        when (this) {
            is Pic -> {
                image.dst
            }

            is Box -> {
                rect
            }

            is Op -> {
                (listOfNotNull(base?.bounds()) + add.mapNotNull { it.bounds() }).reduceOrNull { a, b ->
                    val l = minOf(a.left, b.left)
                    val t = minOf(a.top, b.top)
                    Rect(l, t, maxOf(a.right, b.right) - l, maxOf(a.bottom, b.bottom) - t)
                }
            }
        }

    /**
     * Renders the region as a mask through a renderer's [Canvas]: each operand in a layer of its
     * own, composed with the matching blend. A picture that cuts counts every opaque pixel
     * ([DrawCmd.isCutPixel]), one that adds keeps its alpha.
     */
    public fun eval(
        c: Canvas,
        hard: Boolean = false,
    ) {
        when (this) {
            is Pic -> {
                c.picture(image, hard)
            }

            is Box -> {
                c.box(rect)
            }

            is Op -> {
                if (keep.isEmpty() && cut.isEmpty()) {
                    // a plain union draws straight into the layer it is in, with no extra layer
                    base?.eval(c, hard)
                    add.forEach { it.eval(c, hard) }
                    return
                }
                base?.eval(c, hard)
                if (keep.isNotEmpty()) c.layer(Blend.KEEP) { keep.forEach { it.eval(c, true) } }
                add.forEach { a ->
                    if (a is Op &&
                        (a.keep.isNotEmpty() || a.cut.isNotEmpty())
                    ) {
                        c.layer(Blend.ADD) { a.eval(c, hard) }
                    } else {
                        a.eval(c, hard)
                    }
                }
                // the cuts together in one layer
                if (cut.isNotEmpty()) c.layer(Blend.CUT) { cut.forEach { it.eval(c, true) } }
            }
        }
    }

    /** How a layer merges into what is under it: added, kept only where both are, or cut away. */
    public enum class Blend { ADD, KEEP, CUT }

    /** What a renderer gives [eval]: draw a picture or a black box, and run [body] in a layer merged with a blend. */
    public interface Canvas {
        public fun picture(
            image: DrawCmd.Image,
            hard: Boolean,
        )

        public fun box(rect: Rect)

        public fun layer(
            blend: Blend,
            body: () -> Unit,
        )
    }
}

private fun DrawCmd.Image.sameAs(o: DrawCmd.Image) =
    dst == o.dst && alpha == o.alpha && smooth == o.smooth && gamma == o.gamma && bitmap.sameAs(o.bitmap)

private fun DrawCmd.Text.sameAs(o: DrawCmd.Text) =
    text == o.text && size == o.size && color == o.color && rect == o.rect && alpha == o.alpha && bold == o.bold &&
        shadow == o.shadow && shadowDx == o.shadowDx && shadowDy == o.shadowDy && font.sameAs(o.font) &&
        x == o.x && baseline == o.baseline

private fun DrawCmd.Pixels.sameAs(o: DrawCmd.Pixels) =
    rect == o.rect && w == o.w && h == o.h && alpha == o.alpha && stable == o.stable &&
        (argb === o.argb || argb.contentEquals(o.argb))

/** The same part of the same file: a part made again each frame (an animation's frame) draws the same. */
private fun BitmapDef.sameAs(o: BitmapDef) =
    this === o || (x == o.x && y == o.y && w == o.w && h == o.h && id == o.id && file == o.file && gammaGroup == o.gammaGroup)

/** The same font: a system font a text names is made again each frame. */
private fun FontDef?.sameAs(o: FontDef?) = this === o || (this != null && o != null && id == o.id && file == o.file)

/** The same shape: the same pictures in the same places, the same boxes, combined the same way. */
private fun Region.sameShapeAs(o: Region): Boolean =
    this === o ||
        when (this) {
            is Region.Pic -> o is Region.Pic && image.drawsSameAs(o.image)
            is Region.Box -> o is Region.Box && rect == o.rect
            is Region.Op -> o is Region.Op && sameShapeAs(o)
        }

private fun Region.Op.sameShapeAs(o: Region.Op): Boolean {
    val b = base
    val ob = o.base
    val sameBase = if (b == null || ob == null) b === ob else b.sameShapeAs(ob)
    return sameBase && add.sameShapesAs(o.add) && keep.sameShapesAs(o.keep) && cut.sameShapesAs(o.cut)
}

private fun List<Region>.sameShapesAs(o: List<Region>) = size == o.size && indices.all { this[it].sameShapeAs(o[it]) }
