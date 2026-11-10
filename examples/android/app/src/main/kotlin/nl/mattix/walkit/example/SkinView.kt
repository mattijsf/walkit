// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit.example

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.PorterDuff
import android.graphics.PorterDuffXfermode
import android.graphics.RectF
import android.graphics.Typeface
import android.util.Log
import android.view.Choreographer
import android.view.MotionEvent
import android.view.View
import nl.mattix.walkit.DrawCmd
import nl.mattix.walkit.FakePlayer
import nl.mattix.walkit.FontDef
import nl.mattix.walkit.FontFace
import nl.mattix.walkit.Gamma
import nl.mattix.walkit.ImageLimits
import nl.mattix.walkit.Rect
import nl.mattix.walkit.Region
import nl.mattix.walkit.SkinFiles
import nl.mattix.walkit.SkinHost
import nl.mattix.walkit.SkinRuntime
import nl.mattix.walkit.SkinSetup
import nl.mattix.walkit.Track
import java.io.File
import java.util.WeakHashMap
import kotlin.math.ceil

/**
 * Shows one skin: owns its [SkinRuntime], answers the engine as its [SkinHost], ticks it on every
 * display frame and draws its windows on the view's canvas, the stack scaled to fit the view.
 * Everything runs on the main thread, which is the thread the engine runs on.
 */
class SkinView(
    context: Context,
) : View(context),
    SkinHost,
    Choreographer.FrameCallback {
    /** The player the skin drives: the library's demo player, which plays nothing. */
    val player =
        FakePlayer().apply {
            playlist = List(8) { Track("walkit example - track ${it + 1}", 180_000 + it * 15_000) }
            playTrack(0)
        }

    private var files: SkinFiles? = null
    private var runtime: SkinRuntime? = null

    /** The windows of the last frame, each with its place in the stack. */
    private var frame: List<Pair<Rect, List<DrawCmd>>> = emptyList()

    /** Set when the engine asks for a redraw; the next display frame draws. */
    private var dirty = false

    // where the stack sits on the view: stack pixel x shows at offsetX + x * scale
    private var scale = 1f
    private var offsetX = 0f
    private var offsetY = 0f
    private var touchSeq = 0

    // ---- the skin ----------------------------------------------------------------------------

    /** Loads the skin in [skinFiles], named [name] for its scripts, in place of the one shown. */
    fun show(
        skinFiles: SkinFiles,
        name: String,
    ) {
        close()
        files = skinFiles
        val rt = SkinRuntime(skinFiles, this, player, SkinSetup(skinName = name))
        runtime = rt
        rt.presentation = rt.presentation.copy(touch = true)
        rt.load()
        dirty = true
    }

    /** Frees the engine's side and every picture made from the skin. */
    fun close() {
        runtime?.close()
        runtime = null
        files = null
        frame = emptyList()
        pictures.clear()
        tinted.clear()
        cutMasks.clear()
        typefaces.clear()
        invalidate()
    }

    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        Choreographer.getInstance().postFrameCallback(this)
    }

    override fun onDetachedFromWindow() {
        Choreographer.getInstance().removeFrameCallback(this)
        super.onDetachedFromWindow()
    }

    /** Ticks the engine once per display frame, and draws the engine's frame when something changed. */
    override fun doFrame(frameTimeNanos: Long) {
        val rt = runtime
        if (rt != null && (rt.tick() || dirty)) {
            dirty = false
            frame = rt.drawWindows()
            // View's own invalidate: this one asks the engine for no frame
            super.invalidate()
        }
        Choreographer.getInstance().postFrameCallback(this)
    }

    // ---- SkinHost: pictures ------------------------------------------------------------------

    /** The skin's pictures as decoded, by normalized path; null for one that does not decode. */
    private val pictures = HashMap<String, Bitmap?>()

    private fun picture(file: String): Bitmap? =
        pictures.getOrPut(SkinFiles.normalize(file)) {
            val bytes = files?.read(file)
            if (bytes == null || ImageLimits.tooLarge(bytes)) {
                null
            } else {
                val opts = BitmapFactory.Options().apply { inPreferredConfig = Bitmap.Config.ARGB_8888 }
                BitmapFactory.decodeByteArray(bytes, 0, bytes.size, opts)
            }
        }

    override fun imageSize(file: String): Pair<Int, Int>? = picture(file)?.let { it.width to it.height }

    override fun pixel(
        file: String,
        x: Int,
        y: Int,
    ): Int? {
        val b = picture(file) ?: return null
        return if (x in 0 until b.width && y in 0 until b.height) b.getPixel(x, y) else 0
    }

    override fun pixels(
        file: String,
        x: Int,
        y: Int,
        w: Int,
        h: Int,
    ): IntArray? {
        val b = picture(file) ?: return null
        val out = IntArray(w * h)
        // the part inside the picture; pixels outside it read as clear
        val x0 = x.coerceIn(0, b.width)
        val y0 = y.coerceIn(0, b.height)
        val x1 = (x + w).coerceIn(0, b.width)
        val y1 = (y + h).coerceIn(0, b.height)
        if (x1 > x0 && y1 > y0) b.getPixels(out, (y0 - y) * w + (x0 - x), w, x0, y0, x1 - x0, y1 - y0)
        return out
    }

    /** A picture in a color theme's tint, made once per picture and tint. */
    private val tinted = HashMap<Pair<String, Gamma>, Bitmap>()

    private fun tinted(
        file: String,
        gamma: Gamma?,
    ): Bitmap? {
        val b = picture(file) ?: return null
        if (gamma == null || gamma.isIdentity) return b
        return tinted.getOrPut(SkinFiles.normalize(file) to gamma) {
            val px = IntArray(b.width * b.height)
            b.getPixels(px, 0, b.width, 0, 0, b.width, b.height)
            for (i in px.indices) px[i] = gamma.apply(px[i])
            Bitmap.createBitmap(px, b.width, b.height, Bitmap.Config.ARGB_8888)
        }
    }

    /** A picture with every pixel that marks a cut made opaque and the rest clear, for shapes. */
    private val cutMasks = HashMap<String, Bitmap>()

    private fun cutMask(file: String): Bitmap? {
        val b = picture(file) ?: return null
        return cutMasks.getOrPut(SkinFiles.normalize(file)) {
            val px = IntArray(b.width * b.height)
            b.getPixels(px, 0, b.width, 0, 0, b.width, b.height)
            for (i in px.indices) px[i] = if (DrawCmd.isCutPixel(px[i])) -0x1000000 else 0
            Bitmap.createBitmap(px, b.width, b.height, Bitmap.Config.ARGB_8888)
        }
    }

    // ---- SkinHost: text ----------------------------------------------------------------------

    /** Typefaces made from font files of the skin (or of the default set under it), by path. */
    private val typefaces = HashMap<String, Typeface>()

    private fun typefaceFromFile(path: String): Typeface =
        typefaces.getOrPut(SkinFiles.normalize(path)) {
            val bytes = files?.read(path) ?: return@getOrPut Typeface.SANS_SERIF
            // Typeface reads a file, so the font goes to the cache folder first
            val f = File(context.cacheDir, "font-${SkinFiles.normalize(path).hashCode()}.ttf")
            f.writeBytes(bytes)
            runCatching { Typeface.Builder(f).build() }.getOrNull() ?: Typeface.SANS_SERIF
        }

    private val textPaint = Paint(Paint.ANTI_ALIAS_FLAG)

    /** [textPaint] set up for [font] at [size] as the engine chooses its face ([FontFace.of]). */
    private fun textPaint(
        font: FontDef?,
        size: Int,
        bold: Boolean,
    ): Paint {
        val p = textPaint
        when (val face = FontFace.of(font, bold)) {
            is FontFace.SkinFile -> {
                p.typeface = typefaceFromFile(face.font.file)
                p.isFakeBoldText = face.fakeBold
            }

            is FontFace.StandIn -> {
                // the free fonts for Windows' system fonts, when the default set carries them
                p.typeface = typefaceFromFile(face.file)
                p.isFakeBoldText = false
            }

            is FontFace.Default -> {
                p.typeface = Typeface.SANS_SERIF
                p.isFakeBoldText = face.fakeBold
            }
        }
        p.textSize = FontFace.emSize(size)
        return p
    }

    override fun textWidth(
        text: String,
        font: FontDef?,
        size: Int,
    ): Int = textWidth(text, font, size, false)

    override fun textWidth(
        text: String,
        font: FontDef?,
        size: Int,
        bold: Boolean,
    ): Int = ceil(textAdvance(text, font, size, bold)).toInt()

    override fun textAdvance(
        text: String,
        font: FontDef?,
        size: Int,
        bold: Boolean,
    ): Float = textPaint(font, size, bold).measureText(text)

    // ---- SkinHost: the rest ------------------------------------------------------------------

    override fun log(message: String) {
        Log.i(TAG, message)
    }

    /** The engine (or Android) asks for a redraw: the next display frame asks the engine for its frame. */
    override fun invalidate() {
        dirty = true
        super.invalidate()
    }

    // ---- drawing -----------------------------------------------------------------------------

    private val paint = Paint().apply { isFilterBitmap = false }
    private val src = android.graphics.Rect()
    private val dst = RectF()

    /** The bitmaps made from stable pixel pictures, kept as long as the command is. */
    private val stablePixels = WeakHashMap<DrawCmd.Pixels, Bitmap>()

    override fun onDraw(canvas: Canvas) {
        val rt = runtime ?: return
        if (frame.isEmpty()) return
        // the part of the stack its windows show, from its top-left corner, scaled to fit the
        // view and centered across
        val (w, h) = rt.drawnSize
        if (w <= 0 || h <= 0) return
        scale = minOf(width.toFloat() / w, height.toFloat() / h)
        offsetX = (width - w * scale) / 2
        offsetY = 0f
        canvas.save()
        canvas.translate(offsetX, offsetY)
        canvas.scale(scale, scale)
        for ((rect, cmds) in frame) {
            if (rect.width <= 0 || rect.height <= 0) continue
            val count = canvas.save()
            canvas.translate(rect.left.toFloat(), rect.top.toFloat())
            canvas.clipRect(0, 0, rect.width, rect.height)
            drawWindow(canvas, rect, cmds)
            canvas.restoreToCount(count)
        }
        canvas.restore()
    }

    /** One window's commands, drawn at its own top-left corner. */
    private fun drawWindow(
        canvas: Canvas,
        rect: Rect,
        cmds: List<DrawCmd>,
    ) {
        // the save counts of open clips, and of the open shape with its region
        val clips = ArrayDeque<Int>()
        val shapes = ArrayDeque<Pair<Int, Region>>()
        for (cmd in cmds) {
            when (cmd) {
                is DrawCmd.Image -> drawImage(canvas, cmd)

                is DrawCmd.Fill -> {
                    paint.color = cmd.color.value or -0x1000000
                    paint.alpha = cmd.alpha
                    canvas.drawRect(rectF(cmd.rect), paint)
                    paint.alpha = 255
                }

                is DrawCmd.Text -> drawText(canvas, cmd)

                is DrawCmd.Pixels -> drawPixels(canvas, cmd)

                is DrawCmd.Clip -> {
                    clips.addLast(canvas.save())
                    canvas.clipRect(rectF(cmd.rect))
                }

                DrawCmd.Unclip -> clips.removeLastOrNull()?.let(canvas::restoreToCount)

                // A shaped window: what follows draws into a layer of its own, and at its end
                // the layer keeps only the pixels inside the shape's mask (DST_IN).
                is DrawCmd.WindowStart -> {
                    val count = canvas.saveLayer(0f, 0f, rect.width.toFloat(), rect.height.toFloat(), null)
                    shapes.addLast(count to cmd.region)
                }

                DrawCmd.WindowEnd -> {
                    val (count, region) = shapes.removeLastOrNull() ?: continue
                    // close the clips opened inside the shape, keeping the layer itself
                    canvas.restoreToCount(count + 1)
                    while (clips.isNotEmpty() && clips.last() > count) clips.removeLast()
                    canvas.saveLayer(0f, 0f, rect.width.toFloat(), rect.height.toFloat(), blendPaint(PorterDuff.Mode.DST_IN))
                    region.eval(MaskCanvas(canvas))
                    canvas.restoreToCount(count)
                }

                is DrawCmd.BitmapText -> Unit // the engine spells it out as clips and images
            }
        }
    }

    private fun drawImage(
        canvas: Canvas,
        cmd: DrawCmd.Image,
    ) {
        val b = cmd.bitmap
        val bitmap = tinted(b.file, cmd.gamma) ?: return
        val sw = b.w ?: (bitmap.width - b.x)
        val sh = b.h ?: (bitmap.height - b.y)
        if (sw <= 0 || sh <= 0 || cmd.dst.width == 0 || cmd.dst.height == 0) return
        src.set(b.x, b.y, b.x + sw, b.y + sh)
        paint.alpha = cmd.alpha
        paint.isFilterBitmap = cmd.smooth
        drawFlippable(canvas, bitmap, cmd.dst, paint)
        paint.isFilterBitmap = false
        paint.alpha = 255
    }

    /** [bitmap]'s [src] part over [r]; a negative width or height mirrors it. */
    private fun drawFlippable(
        canvas: Canvas,
        bitmap: Bitmap,
        r: Rect,
        p: Paint,
    ) {
        dst.set(rectF(r))
        if (r.width >= 0 && r.height >= 0) {
            canvas.drawBitmap(bitmap, src, dst, p)
            return
        }
        dst.sort()
        canvas.save()
        canvas.scale(if (r.width < 0) -1f else 1f, if (r.height < 0) -1f else 1f, dst.centerX(), dst.centerY())
        canvas.drawBitmap(bitmap, src, dst, p)
        canvas.restore()
    }

    private fun drawText(
        canvas: Canvas,
        cmd: DrawCmd.Text,
    ) {
        val p = textPaint(cmd.font, cmd.size, cmd.bold)
        canvas.save()
        canvas.clipRect(rectF(cmd.rect))
        cmd.shadow?.let {
            p.color = it.value or -0x1000000
            p.alpha = cmd.alpha
            canvas.drawText(cmd.text, cmd.x + cmd.shadowDx, cmd.baseline + cmd.shadowDy, p)
        }
        p.color = cmd.color.value or -0x1000000
        p.alpha = cmd.alpha
        canvas.drawText(cmd.text, cmd.x, cmd.baseline, p)
        canvas.restore()
    }

    private fun drawPixels(
        canvas: Canvas,
        cmd: DrawCmd.Pixels,
    ) {
        if (cmd.w <= 0 || cmd.h <= 0) return
        val make = { Bitmap.createBitmap(cmd.argb, cmd.w, cmd.h, Bitmap.Config.ARGB_8888) }
        val bitmap = if (cmd.stable) stablePixels.getOrPut(cmd, make) else make()
        src.set(0, 0, cmd.w, cmd.h)
        paint.alpha = cmd.alpha
        drawFlippable(canvas, bitmap, cmd.rect, paint)
        paint.alpha = 255
    }

    /**
     * What [Region.eval] draws a window's mask through: pictures and black boxes, and each layer
     * merged with the blend it names. Opaque pixels show the window; clear ones hide it.
     */
    private inner class MaskCanvas(
        private val canvas: Canvas,
    ) : Region.Canvas {
        override fun picture(
            image: DrawCmd.Image,
            hard: Boolean,
        ) {
            val b = image.bitmap
            val bitmap = (if (hard) cutMask(b.file) else picture(b.file)) ?: return
            val sw = b.w ?: (bitmap.width - b.x)
            val sh = b.h ?: (bitmap.height - b.y)
            if (sw <= 0 || sh <= 0 || image.dst.width == 0 || image.dst.height == 0) return
            src.set(b.x, b.y, b.x + sw, b.y + sh)
            drawFlippable(canvas, bitmap, image.dst, paint)
        }

        override fun box(rect: Rect) {
            paint.color = -0x1000000
            canvas.drawRect(rectF(rect), paint)
        }

        override fun layer(
            blend: Region.Blend,
            body: () -> Unit,
        ) {
            val mode =
                when (blend) {
                    Region.Blend.ADD -> PorterDuff.Mode.SRC_OVER
                    Region.Blend.KEEP -> PorterDuff.Mode.DST_IN
                    Region.Blend.CUT -> PorterDuff.Mode.DST_OUT
                }
            val count = canvas.saveLayer(null, blendPaint(mode))
            body()
            canvas.restoreToCount(count)
        }
    }

    private fun blendPaint(mode: PorterDuff.Mode) = Paint().apply { xfermode = PorterDuffXfermode(mode) }

    private fun rectF(r: Rect) = RectF(r.left.toFloat(), r.top.toFloat(), r.right.toFloat(), r.bottom.toFloat())

    // ---- touch -------------------------------------------------------------------------------

    /** A finger on the view, handed to the engine in stack pixels. */
    override fun onTouchEvent(event: MotionEvent): Boolean {
        val rt = runtime ?: return false
        val x = ((event.x - offsetX) / scale).toInt()
        val y = ((event.y - offsetY) / scale).toInt()
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> rt.pointerDown(x, y, ++touchSeq)
            MotionEvent.ACTION_MOVE -> rt.pointerMove(x, y)
            MotionEvent.ACTION_UP -> rt.pointerUp(x, y)
            MotionEvent.ACTION_CANCEL -> rt.pointerCancel()
            else -> return true
        }
        dirty = true
        return true
    }

    private companion object {
        const val TAG = "walkit"
    }
}
