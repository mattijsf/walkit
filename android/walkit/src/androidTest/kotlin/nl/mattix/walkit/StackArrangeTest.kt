// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * How a phone sets out a skin's windows as one stack, through the binding: each window docked
 * under what shows of the main one, the last list window filling the height upright, list windows
 * in a second column beside the main one when the host asks for two columns, and the stack's
 * measures ([SkinRuntime.naturalStackSize], [SkinRuntime.leftColumn], [SkinRuntime.shownWidth],
 * [SkinRuntime.contentRight]).
 */
@RunWith(AndroidJUnit4::class)
class StackArrangeTest {
    // a main window 300x200 whose shaped body shows 200x120 of it, an equalizer and a playlist,
    // both of which can be made taller, and a window held at 340 px wide
    private val files =
        InlineSkinFiles(
            "player.xml" to
                """<container id="main" name="t" default_visible="1"><layout id="normal" w="300" h="200">""" +
                """<group id="body" x="0" y="0" w="200" h="120" sysregion="1" rectrgn="1"/>""" +
                """</layout></container>""" +
                """<container id="eq" name="Equalizer" default_visible="0"><layout id="normal" w="300" h="60">""" +
                """<layer id="eq.sizer" resize="bottomright" x="290" y="50" w="10" h="10"/>""" +
                """</layout></container>""" +
                """<container id="pl" name="Playlist" default_visible="0"><layout id="normal" w="300" h="80">""" +
                """<component id="list" param="guid:{45F3F7C1-A6F3-4ee6-A15E-125E92FC3F8D}" """ +
                """x="0" y="0" w="290" h="70"/>""" +
                """<layer id="pl.sizer" resize="bottomright" x="290" y="70" w="10" h="10"/>""" +
                """</layout></container>""" +
                """<container id="wide" name="Wide" default_visible="0">""" +
                """<layout id="normal" w="340" h="40" minimum_w="340" maximum_w="340"/></container>""",
        )

    private fun loaded(): SkinRuntime =
        SkinRuntime(
            files,
            TraceHost(files, StringBuilder()),
            FakePlayer().apply { clock = { 0L } },
            clock = { 0L },
        ).apply { load() }

    private fun SkinRuntime.open(vararg ids: String) = ids.forEach { assertTrue("$it opens", openWindow(it)) }

    private fun SkinRuntime.rectOf(id: String): Rect {
        ensureLayout()
        return windowRect(checkNotNull(windowOf(id)))
    }

    private fun SkinRuntime.show(
        w: Int,
        h: Int,
        columns: Int = 1,
        columnWidth: Int = 0,
        stretchLast: Boolean = true,
    ) {
        presentation =
            presentation.copy(viewSize = w to h, columns = columns, columnWidth = columnWidth, stretchLast = stretchLast)
    }

    @Test
    fun thePresentationReadsAndSetsTheWindowSettingsAsOneValue() {
        loaded().use { rt ->
            assertEquals(Presentation(), rt.presentation)
            val p =
                Presentation(
                    viewSize = 200 to 400,
                    viewport = 800 to 600,
                    columns = 2,
                    columnWidth = 150,
                    stretchLast = false,
                    touchSlop = 9,
                )
            rt.presentation = p
            assertEquals(p, rt.presentation)
            assertEquals(200 to 400, rt.viewSize)
            rt.presentation = rt.presentation.copy(viewSize = null)
            assertNull(rt.viewSize)
            assertEquals("the screen scripts see before there is a view", 800 to 600, rt.viewport)
        }
    }

    @Test
    fun oneColumnDocksEachWindowUnderWhatShowsOfTheMainWindowAsWideAsIt() {
        loaded().use { rt ->
            rt.show(200, 1000, stretchLast = false)
            rt.open("eq", "pl")

            assertEquals(Rect(0, 0, 300, 200), rt.rectOf("main"))
            assertEquals("under the body, as wide as it", Rect(0, 120, 200, 60), rt.rectOf("eq"))
            assertEquals(Rect(0, 180, 200, 80), rt.rectOf("pl"))
            assertEquals(300 to 260, rt.stackSize)
            assertEquals(200, rt.shownWidth())
            assertEquals(200, rt.contentRight(checkNotNull(rt.windowOf("main"))))
            assertTrue(rt.listWindowOpen)
        }
    }

    @Test
    fun uprightTheLastListWindowTakesTheHeightLeft() {
        loaded().use { rt ->
            rt.show(200, 1000)
            rt.open("eq", "pl")

            assertEquals(Rect(0, 120, 200, 60), rt.rectOf("eq"))
            assertEquals("the playlist fills the view", Rect(0, 180, 200, 820), rt.rectOf("pl"))
        }
    }

    @Test
    fun sidewaysWithTwoColumnsTheListWindowsSitRightOfTheMainWindow() {
        loaded().use { rt ->
            rt.show(600, 400, columns = 2)
            rt.open("pl", "eq")

            assertEquals("under the body, at its height though it is the last window", Rect(0, 120, 200, 60), rt.rectOf("eq"))
            assertEquals(Rect(300, 0, 200, 400), rt.rectOf("pl"))
            assertEquals(500 to 400, rt.stackSize)
            assertEquals(Rect(0, 0, 300, 200), rt.leftColumn())
        }
    }

    @Test
    fun theRightColumnStartsPastAWindowInTheLeftColumnThatIsWiderThanTheMainOne() {
        loaded().use { rt ->
            rt.show(800, 400, columns = 2)
            rt.open("wide", "pl")

            assertEquals(Rect(0, 120, 340, 40), rt.rectOf("wide"))
            assertEquals(Rect(340, 0, 200, 400), rt.rectOf("pl"))
            assertEquals(540 to 400, rt.stackSize)
            assertEquals(Rect(0, 0, 340, 200), rt.leftColumn())
            assertEquals(540 to 200, rt.naturalStackSize(2))
            assertEquals(340, rt.shownWidth())
        }
    }

    @Test
    fun withNoListWindowOpenTwoColumnsAreOne() {
        loaded().use { rt ->
            rt.show(600, 400)
            rt.open("eq")
            val one = rt.rectOf("eq")

            rt.show(600, 400, columns = 2)
            assertEquals(one, rt.rectOf("eq"))
            assertEquals(Rect(0, 120, 200, 60), one)
            assertNull("no left column to dock against", rt.leftColumn())
            assertFalse(rt.listWindowOpen)
        }
    }

    @Test
    fun aStacksNaturalSizeInOneColumnAndInTwoLeavesEveryWindowAsItIs() {
        loaded().use { rt ->
            rt.show(200, 1000)
            rt.open("eq", "pl")
            assertEquals(Rect(0, 180, 200, 820), rt.rectOf("pl"))
            val passes = rt.stats.layoutPasses

            assertEquals("one column: the playlist at its natural height", 300 to 260, rt.naturalStackSize(1))
            assertEquals("two columns: as tall as the main window", 500 to 200, rt.naturalStackSize(2))
            assertEquals("no window changed", 200 to 820, rt.windowSize(checkNotNull(rt.windowOf("pl"))))
            assertEquals("no layout pass ran", passes, rt.stats.layoutPasses)
        }
    }
}
