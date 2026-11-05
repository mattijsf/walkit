// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The packed buffers as the engine writes them (`walkit::packed`): a frame decodes into the draw
 * commands it holds, and a menu goes round trip.
 */
class PackedTest {
    /** A frame with one window and one of every command, as `walkit::packed`'s test writes it. */
    private val frame: ByteArray by lazy { hex(FRAME_HEX) }

    @Test
    fun `a frame decodes into its windows and commands`() {
        val windows = PackedFrame.decode(frame)
        assertEquals(1, windows.size)
        val w = windows[0]
        assertEquals(7, w.id.raw)
        assertEquals("main", w.container)
        assertEquals("normal", w.layout)
        assertEquals(Rect(10, 20, 300, 100), w.rect)
        val kinds = w.cmds.map { it::class.simpleName }
        assertEquals(
            listOf("WindowStart", "Image", "Image", "Fill", "Text", "Clip", "Pixels", "Unclip", "WindowEnd"),
            kinds,
        )
        val shape = w.cmds[0] as DrawCmd.WindowStart
        val op = shape.region as Region.Op
        assertTrue(op.base is Region.Box)
        assertEquals(1, op.cut.size)
        val image = w.cmds[1] as DrawCmd.Image
        assertEquals("main.png", image.bitmap.file)
        assertEquals(10, image.bitmap.w)
        assertEquals(Rect(1, 2, 10, 5), image.dst)
        assertEquals(255, image.alpha)
        assertNull(image.gamma)
        val fill = w.cmds[3] as DrawCmd.Fill
        assertEquals(0xff00ff, fill.color.value)
        assertEquals(200, fill.alpha)
        val text = w.cmds[4] as DrawCmd.Text
        assertEquals("Andamp", text.text)
        assertEquals("font.ttf", text.font?.file)
        assertEquals(12, text.size)
        assertTrue(text.bold)
        assertEquals(0, text.shadow?.value)
        assertEquals(2.5f, text.x)
        assertEquals(10.25f, text.baseline)
        val pixels = w.cmds[6] as DrawCmd.Pixels
        assertEquals(2, pixels.w)
        assertEquals(1, pixels.h)
        assertTrue(pixels.stable)
        assertEquals(0xff112233.toInt(), pixels.argb[0])
        assertEquals(0x80445566.toInt(), pixels.argb[1])
    }

    @Test
    fun `a menu goes round trip`() {
        val menu =
            SkinMenu(
                listOf(
                    SkinMenu.Command(1, "Open", checked = false, disabled = false),
                    SkinMenu.Separator,
                    SkinMenu.Submenu("More", SkinMenu(listOf(SkinMenu.Command(2, "Ticked", checked = true, disabled = true)))),
                ),
            )
        val back = PackedMenu.decode(PackedMenu.encode(menu))
        assertEquals(3, back.items.size)
        val open = back.items[0] as SkinMenu.Command
        assertEquals(1, open.id)
        assertEquals("Open", open.text)
        assertEquals(SkinMenu.Separator, back.items[1])
        val more = back.items[2] as SkinMenu.Submenu
        assertEquals("More", more.text)
        val ticked = more.menu.items[0] as SkinMenu.Command
        assertTrue(ticked.checked && ticked.disabled)
    }

    private fun hex(s: String): ByteArray =
        s
            .filter { !it.isWhitespace() }
            .chunked(2)
            .map { it.toInt(16).toByte() }
            .toByteArray()

    private companion object {
        /** `walkit::packed::tests::a_frame_packs_with_its_strings_once`'s buffer, printed by that test. */
        val FRAME_HEX =
            "574b444c0100000007000000040000006d61696e060000006e6f726d616c070000006d61696e2e6267080000006d6169" +
                "6e2e706e6706000000416e64616d70010000006608000000666f6e742e74746601000000070000000000000001000000" +
                "0a000000140000002c01000064000000090000000303010200000000000000000a000000050000000000000000000000" +
                "0100000001020000000300000000000000000000000a00000005000000ffffffff00000000000000000a000000050000" +
                "00ff000000000001020000000300000000000000000000000a00000005000000ffffffff01000000020000000a000000" +
                "05000000ff000000000001020000000300000000000000000000000a00000005000000ffffffff03000000040000000a" +
                "000000050000008000000000000200000000000000000100000001000000ff00ff00c800000005040000000105000000" +
                "060000000c000000ffffff0000000000000000003200000014000000ff00000001010000000001000000010000000000" +
                "204000002441070000000000000000020000000200000009000000000000000002000000010000000200000001000000" +
                "ff00000001332211ff665544800804"
    }
}
