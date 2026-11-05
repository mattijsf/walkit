// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertThrows
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
        val decoded = PackedFrame.decode(frame) { null }
        val windows = decoded.windows
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
        val plan = w.plan
        assertEquals(5L, plan.id)
        assertEquals(6L, plan.version)
        val run = plan.runs.single()
        assertEquals(8L, run.id)
        assertEquals(9L, run.version)
        assertEquals(4, run.from)
        assertEquals(7, run.to)
        assertEquals(Rect(0, 0, 50, 20), run.bounds)
    }

    @Test
    fun `the next frame keeps the commands that did not change, as the same objects`() {
        val first = PackedFrame.decode(frame) { null }.windows[0]
        val next = PackedFrame.decode(hex(NEXT_HEX)) { first.cmds }.windows[0]
        assertEquals(9, next.cmds.size)
        for (k in next.cmds.indices) {
            if (k == 3) continue
            assertSame("command $k kept", first.cmds[k], next.cmds[k])
        }
        val fill = next.cmds[3] as DrawCmd.Fill
        assertEquals(0x00ff00, fill.color.value)
        assertEquals(10L, next.plan.version)
        val same = PackedFrame.decode(hex(SAME_HEX)) { next.cmds }.windows[0]
        assertSame("a window as before keeps its list", next.cmds, same.cmds)
    }

    @Test
    fun `a frame that keeps what the host never had is refused`() {
        val first = PackedFrame.decode(frame) { null }.windows[0]
        assertThrows(IllegalStateException::class.java) { PackedFrame.decode(hex(NEXT_HEX)) { null } }
        assertThrows(IllegalStateException::class.java) { PackedFrame.decode(hex(SAME_HEX)) { null } }
        assertEquals(
            9,
            PackedFrame
                .decode(hex(SAME_HEX)) { first.cmds }
                .windows[0]
                .cmds.size,
        )
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
        /** `walkit::packed::tests::a_frame_packs_with_its_strings_once`'s buffers, printed by that test. */
        const val FRAME_HEX =
            "574b444c0300000007000000040000006d61696e060000006e6f726d616c070000006d61696e2e6267080000" +
                "006d61696e2e706e6706000000416e64616d70010000006608000000666f6e742e7474660100000007000000" +
                "00000000010000000a000000140000002c010000640000000500000000000000060000000000000001000000" +
                "0800000000000000090000000000000004000000070000000000000000000000320000001400000001010000" +
                "0001090000000303010200000000000000000a00000005000000000000000000000001000000010200000003" +
                "00000000000000000000000a00000005000000ffffffff00000000000000000a00000005000000ff00000000" +
                "0001020000000300000000000000000000000a00000005000000ffffffff01000000020000000a0000000500" +
                "0000ff000000000001020000000300000000000000000000000a00000005000000ffffffff03000000040000" +
                "000a000000050000008000000000000200000000000000000100000001000000ff00ff00c800000005040000" +
                "000105000000060000000c000000ffffff0000000000000000003200000014000000ff000000010100000000" +
                "0100000001000000000020400000244107000000000000000002000000020000000900000000000000000200" +
                "0000010000000200000001000000ff00000001332211ff665544800804"
        const val NEXT_HEX =
            "574b444c0300000002000000040000006d61696e060000006e6f726d616c0100000007000000000000000100" +
                "00000a000000140000002c0100006400000005000000000000000a0000000000000001000000080000000000" +
                "0000090000000000000004000000070000000000000000000000320000001400000001030000000003000000" +
                "0101000000020000000000000000010000000100000000ff0000c80000000005000000"
        const val SAME_HEX =
            "574b444c0300000002000000040000006d61696e060000006e6f726d616c0100000007000000000000000100" +
                "00000a000000140000002c0100006400000005000000000000000a0000000000000001000000080000000000" +
                "0000090000000000000004000000070000000000000000000000320000001400000000"
    }
}
