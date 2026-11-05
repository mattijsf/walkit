// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.lang.reflect.Field
import java.lang.reflect.Modifier

/**
 * [DrawCmd.drawsSameAs] tells every change apart: each field of each command, and of the picture
 * part and the font they name, changed alone makes a command draw differently. A field added
 * later is found by reflection, so it cannot be left out of the comparison unnoticed.
 */
class DrawCmdSameTest {
    private val region = Region.Box(Rect(0, 0, 10, 10))

    /** One of each command, made afresh on each call with equal values. */
    private val samples: List<() -> DrawCmd> =
        listOf(
            { DrawCmd.Image(BitmapDef("a", "a.png", 1, 2, 3, 4, "g"), Rect(1, 2, 3, 4), 200, Gamma(1, 2, 3, 0, 0)) },
            { DrawCmd.Fill(Rect(1, 2, 3, 4), Rgb(0x123456), 200) },
            { DrawCmd.Text("abc", FontDef("f", "f.ttf"), 12, Rgb(1), Rect(1, 2, 3, 4), 255, false, Rgb(2), 1, 1, 2.5f, 7.5f) },
            { DrawCmd.BitmapText("abc", Rect(1, 2, 3, 4)) },
            { DrawCmd.Clip(Rect(1, 2, 3, 4)) },
            { DrawCmd.Pixels(Rect(1, 2, 3, 4), 2, 2, intArrayOf(1, 2, 3, 4), 255, false) },
            { DrawCmd.WindowStart(region) },
        )

    @Test
    fun `equal commands made apart draw the same, pixels with equal content included`() {
        for (make in samples) {
            val a = make()
            assertTrue(a.javaClass.simpleName, a.drawsSameAs(make()))
        }
        assertTrue(DrawCmd.Unclip.drawsSameAs(DrawCmd.Unclip))
        assertTrue(DrawCmd.WindowEnd.drawsSameAs(DrawCmd.WindowEnd))
        assertFalse(DrawCmd.Unclip.drawsSameAs(DrawCmd.WindowEnd))
    }

    @Test
    fun `changing any one field of a command makes it draw differently`() {
        for (make in samples) {
            val a = make()
            for (f in fieldsOf(a.javaClass)) {
                val b = make()
                f.set(b, changed(f.get(b)))
                val what = "${a.javaClass.simpleName}.${f.name}"
                assertFalse(what, a.drawsSameAs(b))
                assertFalse(what, b.drawsSameAs(a))
            }
        }
    }

    @Test
    fun `changing any one field of a picture part or a font makes the command draw differently`() {
        val image = samples[0]
        val text = samples[2]
        val nested =
            listOf(
                Triple(image, "bitmap", BitmapDef::class.java),
                Triple(text, "font", FontDef::class.java),
            )
        for ((make, holder, type) in nested) {
            val a = make()
            for (f in fieldsOf(type)) {
                val b = make()
                val held = field(b.javaClass, holder).get(b)
                f.set(held, changed(f.get(held)))
                val what = "${a.javaClass.simpleName}.$holder.${f.name}"
                assertFalse(what, a.drawsSameAs(b))
            }
        }
    }

    @Test
    fun `a window shape worked out again is the same while it is made the same way`() {
        fun shape(cut: Int) =
            Region.Op(
                Region.Pic(DrawCmd.Image(BitmapDef("a", "a.png", 0, 0, 10, 10), Rect(0, 0, 10, 10), 255)),
                listOf(Region.Box(Rect(10, 0, 5, 5))),
                emptyList(),
                listOf(Region.Box(Rect(2, 2, cut, 2))),
            )
        assertTrue(DrawCmd.WindowStart(shape(2)).drawsSameAs(DrawCmd.WindowStart(shape(2))))
        assertFalse(DrawCmd.WindowStart(shape(2)).drawsSameAs(DrawCmd.WindowStart(shape(3))))
        assertFalse(DrawCmd.WindowStart(shape(2)).drawsSameAs(DrawCmd.WindowStart(Region.Box(Rect(0, 0, 15, 10)))))
    }

    @Test
    fun `different kinds of command never draw the same`() {
        val all = samples.map { it() } + DrawCmd.Unclip + DrawCmd.WindowEnd
        for (a in all) {
            for (b in all) {
                if (a !== b) assertFalse("${a.javaClass.simpleName} ${b.javaClass.simpleName}", a.drawsSameAs(b))
            }
        }
    }

    @Test
    fun `the owner is no part of what a command draws`() {
        val a = DrawCmd.Fill(Rect(1, 2, 3, 4), Rgb(0x123456), 200).also { it.ownedBy = 3 }
        val b = DrawCmd.Fill(Rect(1, 2, 3, 4), Rgb(0x123456), 200).also { it.ownedBy = 4 }
        assertTrue(a.drawsSameAs(b))
    }

    /** The fields a command or a definition draws by: those it declares, less the owner it carries for the planner. */
    private fun fieldsOf(type: Class<*>): List<Field> =
        type.declaredFields
            .filter { !Modifier.isStatic(it.modifiers) && !it.isSynthetic && it.name != "ownedBy" }
            .onEach { it.isAccessible = true }

    private fun field(
        type: Class<*>,
        name: String,
    ): Field = type.getDeclaredField(name).also { it.isAccessible = true }

    /** A value of the same type, unequal to [v]. */
    private fun changed(v: Any?): Any =
        when (v) {
            is Int -> v + 1
            is Float -> v + 0.5f
            is Boolean -> !v
            is String -> v + "x"
            is Rect -> Rect(v.left + 1, v.top, v.width, v.height)
            is Gamma -> v.copy(r = v.r + 1)
            is Rgb -> Rgb(v.value + 1)
            is IntArray -> v.copyOf().also { it[0]++ }
            is Region -> Region.Box(Rect(1, 0, 10, 10))
            is BitmapDef -> BitmapDef(v.id, v.file, v.x + 1, v.y, v.w, v.h, v.gammaGroup)
            is FontDef -> FontDef(v.id + "x", v.file)
            else -> error("no changed value for $v")
        }
}
