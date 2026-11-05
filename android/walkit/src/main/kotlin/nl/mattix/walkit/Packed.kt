// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * Reads the little-endian buffers the engine hands over: ints, floats, and strings as a length
 * and UTF-8 bytes, or as an index into a table read first.
 */
internal class PackedReader(
    bytes: ByteArray,
) {
    private val buf: ByteBuffer = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)
    private var strings: Array<String> = emptyArray()

    fun u8(): Int = buf.get().toInt() and 0xff

    fun i32(): Int = buf.getInt()

    fun u32(): Long = buf.getInt().toLong() and 0xffffffffL

    fun f32(): Float = buf.getFloat()

    fun bool(): Boolean = u8() != 0

    fun rect(): Rect = Rect(i32(), i32(), i32(), i32())

    /** A string written inline: its byte length, then its UTF-8 bytes. */
    fun inlineString(): String {
        val n = i32()
        val b = ByteArray(n)
        buf.get(b)
        return String(b, Charsets.UTF_8)
    }

    /** The magic, the version and the string table of a frame or a menu. */
    fun header() {
        val magic = ByteArray(4)
        buf.get(magic)
        require(String(magic, Charsets.US_ASCII) == MAGIC) { "not a walkit packed buffer" }
        val version = i32()
        require(version == VERSION) { "walkit packed buffer version $version, expected $VERSION" }
        strings = Array(i32()) { inlineString() }
    }

    /** A string of the table, by index. */
    fun string(): String = strings[i32()]

    /** A string of the table, or null for the none index. */
    fun stringOrNull(): String? {
        val i = i32()
        return if (i == -1) null else strings[i]
    }

    companion object {
        const val MAGIC = "WKDL"
        const val VERSION = 1
    }
}

/**
 * Writes the little-endian buffers the engine takes as arguments in the inline form: ints, and
 * strings as a length and UTF-8 bytes.
 */
internal class PackedOut {
    private val body = java.io.ByteArrayOutputStream()
    private val le = ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN)

    fun i32(v: Int) {
        le.clear()
        le.putInt(v)
        body.write(le.array(), 0, 4)
    }

    fun inlineString(s: String) {
        val b = s.toByteArray(Charsets.UTF_8)
        i32(b.size)
        body.write(b)
    }

    fun bytes(): ByteArray = body.toByteArray()
}

/** A frame as the engine packs it: every window with its drawing. */
internal object PackedFrame {
    /** One window of a frame. */
    class Window(
        val id: WindowId,
        val container: String,
        val layout: String,
        val rect: Rect,
        val cmds: List<DrawCmd>,
    )

    fun decode(bytes: ByteArray): List<Window> {
        val r = PackedReader(bytes)
        r.header()
        return List(r.i32()) {
            val id = WindowId(r.i32())
            val container = r.string()
            val layout = r.string()
            val rect = r.rect()
            val cmds = List(r.i32()) { cmd(r) }
            Window(id, container, layout, rect, cmds)
        }
    }

    private fun bitmap(r: PackedReader): BitmapDef {
        val id = r.string()
        val file = r.string()
        val x = r.i32()
        val y = r.i32()
        val w = r.i32().takeIf { it >= 0 }
        val h = r.i32().takeIf { it >= 0 }
        val gammaGroup = r.stringOrNull()
        return BitmapDef(id, file, x, y, w, h, gammaGroup)
    }

    private fun gamma(r: PackedReader): Gamma? = if (r.bool()) Gamma(r.i32(), r.i32(), r.i32(), r.i32(), r.i32()) else null

    private fun image(r: PackedReader): DrawCmd.Image {
        val bitmap = bitmap(r)
        val dst = r.rect()
        val alpha = r.i32()
        val gamma = gamma(r)
        val smooth = r.bool()
        return DrawCmd.Image(bitmap, dst, alpha, gamma, smooth)
    }

    private fun region(r: PackedReader): Region =
        when (val kind = r.u8()) {
            1 -> {
                Region.Pic(image(r))
            }

            2 -> {
                Region.Box(r.rect())
            }

            3 -> {
                val base = if (r.bool()) region(r) else null
                val add = List(r.i32()) { region(r) }
                val keep = List(r.i32()) { region(r) }
                val cut = List(r.i32()) { region(r) }
                Region.Op(base, add, keep, cut)
            }

            else -> {
                throw IllegalArgumentException("walkit packed frame: region kind $kind")
            }
        }

    private fun cmd(r: PackedReader): DrawCmd =
        when (val tag = r.u8()) {
            1 -> {
                image(r)
            }

            2 -> {
                DrawCmd.Fill(r.rect(), Rgb(r.i32()), r.i32())
            }

            3 -> {
                DrawCmd.WindowStart(region(r))
            }

            4 -> {
                DrawCmd.WindowEnd
            }

            5 -> {
                val text = r.string()
                val font = if (r.bool()) FontDef(r.string(), r.string()) else null
                val size = r.i32()
                val color = Rgb(r.i32())
                val rect = r.rect()
                val alpha = r.i32()
                val bold = r.bool()
                var shadow: Rgb? = null
                var dx = 1
                var dy = 1
                if (r.bool()) {
                    shadow = Rgb(r.i32())
                    dx = r.i32()
                    dy = r.i32()
                }
                val x = r.f32()
                val baseline = r.f32()
                DrawCmd.Text(text, font, size, color, rect, alpha, bold, shadow, dx, dy, x, baseline)
            }

            6 -> {
                DrawCmd.BitmapText(r.string(), r.rect())
            }

            7 -> {
                DrawCmd.Clip(r.rect())
            }

            8 -> {
                DrawCmd.Unclip
            }

            9 -> {
                val rect = r.rect()
                val w = r.i32()
                val h = r.i32()
                val alpha = r.i32()
                val stable = r.bool()
                val argb = IntArray(w * h) { r.i32() }
                DrawCmd.Pixels(rect, w, h, argb, alpha, stable)
            }

            else -> {
                throw IllegalArgumentException("walkit packed frame: command tag $tag")
            }
        }
}

/** A menu as it crosses the boundary: the engine's form read, and a host's menu written the same way. */
internal object PackedMenu {
    fun decode(bytes: ByteArray): SkinMenu {
        val r = PackedReader(bytes)
        r.header()
        return menu(r)
    }

    private fun menu(r: PackedReader): SkinMenu =
        SkinMenu(
            List(r.i32()) {
                when (val kind = r.u8()) {
                    1 -> {
                        SkinMenu.Command(r.i32(), r.string(), r.bool(), r.bool())
                    }

                    2 -> {
                        SkinMenu.Separator
                    }

                    3 -> {
                        SkinMenu.Submenu(r.string(), menu(r))
                    }

                    else -> {
                        throw IllegalArgumentException("walkit packed menu: item kind $kind")
                    }
                }
            },
        )

    /** [menu] in the form [decode] reads, for passing one back to the engine. */
    fun encode(menu: SkinMenu): ByteArray {
        val strings = ArrayList<String>()
        val index = HashMap<String, Int>()
        val body = java.io.ByteArrayOutputStream()
        val le = ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN)

        fun i32(v: Int) {
            le.clear()
            le.putInt(v)
            body.write(le.array(), 0, 4)
        }

        fun string(s: String) =
            i32(
                index.getOrPut(s) {
                    strings.add(s)
                    strings.size - 1
                },
            )

        fun write(m: SkinMenu) {
            i32(m.items.size)
            for (item in m.items) {
                when (item) {
                    is SkinMenu.Command -> {
                        body.write(1)
                        i32(item.id)
                        string(item.text)
                        body.write(if (item.checked) 1 else 0)
                        body.write(if (item.disabled) 1 else 0)
                    }

                    SkinMenu.Separator -> {
                        body.write(2)
                    }

                    is SkinMenu.Submenu -> {
                        body.write(3)
                        string(item.text)
                        write(item.menu)
                    }
                }
            }
        }
        write(menu)
        val out = java.io.ByteArrayOutputStream()
        out.write(PackedReader.MAGIC.toByteArray(Charsets.US_ASCII))
        le.clear()
        le.putInt(PackedReader.VERSION)
        out.write(le.array(), 0, 4)
        le.clear()
        le.putInt(strings.size)
        out.write(le.array(), 0, 4)
        for (s in strings) {
            val b = s.toByteArray(Charsets.UTF_8)
            le.clear()
            le.putInt(b.size)
            out.write(le.array(), 0, 4)
            out.write(b)
        }
        out.write(body.toByteArray())
        return out.toByteArray()
    }
}
