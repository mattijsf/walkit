// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The native side reads the player once per call (its numbers as one snapshot) and keeps what the
 * host answers for good (a text's width), so a frame crosses into Kotlin a few times, not a
 * hundred.
 */
@RunWith(AndroidJUnit4::class)
class BoundaryReadsTest {
    private val files =
        InlineSkinFiles(
            "player.xml" to
                """<container id="main" name="t" default_visible="1"><layout id="normal" w="200" h="100">""" +
                """<text id="title" display="songtitle" x="10" y="10" w="180" h="20" color="255,255,255"/>""" +
                """<text id="time" display="time" x="10" y="40" w="80" h="20" color="0,255,0"/>""" +
                """</layout></container>""",
        )

    /** The fake player, counting how often the runtime reads its position and its title. */
    private class CountingPlayer(
        private val inner: FakePlayer = FakePlayer().apply { clock = { 0L } },
    ) : Player by inner {
        var positions = 0
        var titles = 0
        override val positionMs: Int get() = inner.positionMs.also { positions++ }
        override val title: String get() = inner.title.also { titles++ }
    }

    /** The trace host, counting the texts it is asked to measure. */
    private class CountingHost(
        private val inner: TraceHost,
    ) : SkinHost by inner {
        constructor(files: SkinFiles) : this(TraceHost(files, StringBuilder()))

        var widthCalls = 0
        val widthKeys = HashSet<String>()

        override fun textWidth(
            text: String,
            font: FontDef?,
            size: Int,
            bold: Boolean,
        ): Int {
            widthCalls++
            widthKeys += "w/$text/${font?.id}/$size/$bold"
            return inner.textWidth(text, font, size, bold)
        }

        override fun textAdvance(
            text: String,
            font: FontDef?,
            size: Int,
            bold: Boolean,
        ): Float {
            widthCalls++
            widthKeys += "a/$text/${font?.id}/$size/$bold"
            return inner.textAdvance(text, font, size, bold)
        }
    }

    @Test
    fun aCallReadsThePlayersNumbersOnce() {
        val player = CountingPlayer()
        SkinRuntime(files, CountingHost(files), player, SkinSetup(), clock = { 0L }).use { rt ->
            rt.load()
            player.positions = 0
            rt.tick()
            assertEquals("one snapshot in a tick", 1, player.positions)
            rt.drawWindows()
            assertTrue("at most one more in the draw", player.positions <= 2)
        }
    }

    @Test
    fun aCallReadsTheTitleOnce() {
        val player = CountingPlayer()
        SkinRuntime(files, CountingHost(files), player, SkinSetup(), clock = { 0L }).use { rt ->
            rt.load()
            player.titles = 0
            rt.tick()
            rt.drawWindows()
            assertTrue("a title read per call at most, got ${player.titles}", player.titles <= 2)
        }
    }

    @Test
    fun aTextIsMeasuredOnceForGood() {
        val host = CountingHost(files)
        SkinRuntime(files, host, FakePlayer().apply { clock = { 0L } }, SkinSetup(), clock = { 0L }).use { rt ->
            rt.load()
            repeat(20) {
                rt.tick()
                rt.drawWindows()
            }
            assertTrue("texts were measured", host.widthCalls > 0)
            assertEquals("each text once", host.widthKeys.size, host.widthCalls)
        }
    }
}
