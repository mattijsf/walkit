// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.random.Random

/** How the planner keeps a window's moving objects apart from the rest, and when each version moves. */
class DrawPlannerTest {
    private val planner = DrawPlanner()
    private val window = Rect(0, 0, 200, 100)
    private val key = Any()
    private var now = 1_000_000L

    /** The objects of the window, named for the test's messages. */
    private class Obj(
        val name: String,
    ) {
        override fun toString() = name
    }

    private val layout = Obj("layout")
    private val group = Obj("group")
    private val button = Obj("button")
    private val time = Obj("time")
    private val vis = Obj("vis")
    private val label = Obj("label")
    private val ticker = Obj("ticker")
    private val list = Obj("list")
    private val region = Region.Box(Rect(0, 0, 200, 100))

    /** What the window shows; each frame draws it with fresh commands. */
    private var timeText = "0:00"
    private var visPixels = intArrayOf(1, 2, 3, 4)
    private var labelText = "label"
    private var tickerScroll = 0
    private var listRow = 0
    private var shape: Region = region

    private fun <T : DrawCmd> T.by(owner: Any): T = also { it.ownedBy = owner }

    /** A left-aligned run of text in [r], shifted left by [scroll] as a ticker is, placed as the engine places it. */
    private fun text(
        s: String,
        r: Rect,
        scroll: Int = 0,
    ) = DrawCmd.Text(s, null, 10, Rgb(0xFFFFFF), r, 255, false, null, 1, 1, (r.left - scroll).toFloat(), r.top + 8f)

    private fun image(r: Rect) = DrawCmd.Image(BitmapDef("bg", "bg.png", 0, 0, r.width, r.height), r, 255)

    /** The window's commands as the runtime would draw it now. */
    private fun cmds(): List<DrawCmd> =
        listOf(
            DrawCmd.WindowStart(shape).by(layout),
            image(Rect(0, 0, 200, 100)).by(layout),
            DrawCmd.Clip(Rect(0, 0, 200, 100)).by(group),
            image(Rect(10, 10, 20, 10)).by(button),
            text(timeText, Rect(50, 10, 40, 10)).by(time),
            DrawCmd.Pixels(Rect(100, 10, 60, 20), 2, 2, visPixels.copyOf(), 255).by(vis),
            text(labelText, Rect(10, 50, 30, 10)).by(label),
            // a list clips its rows inside the list's drawing
            DrawCmd.Clip(Rect(10, 62, 100, 12)).by(list),
            text("row $listRow", Rect(10, 62 - listRow, 100, 10)).by(list),
            text("row ${listRow + 1}", Rect(10, 72 - listRow, 100, 10)).by(list),
            DrawCmd.Unclip,
            text("ticker", Rect(50, 80, 140, 10), tickerScroll).by(ticker),
            DrawCmd.Unclip,
            DrawCmd.WindowEnd,
        )

    private val cache = PlanCache()

    /** Plans the window as it is now, [ms] after the frame before, and checks the plan. */
    private fun frame(
        ms: Long = 16,
        moving: Collection<Any> = emptyList(),
    ): WindowPlan {
        now += ms
        val c = cmds()
        val plan = planner.planWindow(key, window, c, now, moving)
        cache.frame(listOf(window to c), listOf(plan), "at $now")
        return plan
    }

    /** The objects each run holds, by the commands it spans. */
    private fun WindowPlan.held(): List<Set<Any?>> {
        val c = cmds()
        return runs.map { r -> (r.from until r.to).mapNotNull { c[it].ownedBy }.toSet() }
    }

    @Test
    fun `an unchanged frame keeps every version`() {
        val first = frame()
        repeat(10) {
            val plan = frame()
            assertEquals(first.id, plan.id)
            assertEquals(first.version, plan.version)
            assertTrue(plan.runs.isEmpty())
        }
        assertEquals(1, cache.windowRecords)
    }

    @Test
    fun `a ticker that steps every frame gets a separate run, and only that run moves`() {
        frame()
        tickerScroll++
        frame()
        tickerScroll++
        val live = frame()
        assertEquals(listOf(setOf<Any?>(ticker)), live.held())
        val records = cache.windowRecords
        repeat(20) {
            tickerScroll++
            val plan = frame()
            assertEquals(live.version, plan.version)
            assertEquals(live.runs.single().id, plan.runs.single().id)
            assertNotEquals(live.runs.single().version, plan.runs.single().version)
        }
        assertEquals("the window is not recorded again", records, cache.windowRecords)
    }

    @Test
    fun `a one-off change moves the window's version and makes nothing live`() {
        val before = frame()
        labelText = "other"
        val changed = frame()
        assertNotEquals(before.version, changed.version)
        assertTrue(changed.runs.isEmpty())
        repeat(5) { assertEquals(changed.version, frame().version) }
        assertTrue(frame().runs.isEmpty())
    }

    @Test
    fun `a second change within the gap makes an object live, and it rests after the hold`() {
        frame()
        timeText = "0:01"
        assertTrue(frame(ms = 1_000).runs.isEmpty())
        timeText = "0:02"
        val live = frame(ms = DrawPlanner.LIVE_GAP_MS)
        assertEquals(listOf(setOf<Any?>(time)), live.held())
        // still live while it holds, though it did not change
        assertEquals(live.runs.single().id, frame(ms = DrawPlanner.LIVE_HOLD_MS - 100).runs.single().id)
        val rested = frame(ms = 200)
        assertTrue(rested.runs.isEmpty())
        assertNotEquals(live.version, rested.version)
    }

    @Test
    fun `changes further apart than the gap never make an object live`() {
        frame()
        repeat(4) {
            timeText = "0:0$it"
            assertTrue(frame(ms = DrawPlanner.LIVE_GAP_MS + 1).runs.isEmpty())
        }
    }

    @Test
    fun `an object the tick reports moving is live from its first change`() {
        frame()
        visPixels = intArrayOf(5, 6, 7, 8)
        assertEquals(listOf(setOf<Any?>(vis)), frame(moving = listOf(vis)).held())
    }

    @Test
    fun `pixels with the same content are no change`() {
        val before = frame()
        visPixels = visPixels.copyOf()
        val after = frame()
        assertEquals(before.version, after.version)
        assertTrue(after.runs.isEmpty())
    }

    @Test
    fun `neighboring live objects share a run and one far away gets a separate one`() {
        frame()
        repeat(3) {
            timeText = "0:1$it"
            visPixels = intArrayOf(it, 0, 0, 0)
            tickerScroll++
            frame()
        }
        timeText = "0:20"
        visPixels = intArrayOf(9, 0, 0, 0)
        tickerScroll++
        val plan = frame()
        // the time and the visualizer sit side by side; the label between them and the ticker rests
        assertEquals(listOf(setOf<Any?>(time, vis), setOf<Any?>(ticker)), plan.held())
    }

    @Test
    fun `a list that clips its rows is kept apart with its clip`() {
        frame()
        repeat(4) {
            listRow++
            frame()
        }
        val plan = frame()
        assertEquals(listOf(setOf<Any?>(list)), plan.held())
        val c = cmds()
        assertTrue(c[plan.runs.single().from] is DrawCmd.Clip)
        assertEquals(DrawCmd.Unclip, c[plan.runs.single().to - 1])
    }

    @Test
    fun `a run's bounds hold what it draws and only grow`() {
        frame()
        val bounds = ArrayList<Rect>()
        repeat(6) {
            listRow++
            bounds += frame().runs.map { it.bounds }
        }
        for (i in 1 until bounds.size) {
            val a = bounds[i - 1]
            val b = bounds[i]
            assertTrue("$b holds $a", b.left <= a.left && b.top <= a.top && b.right >= a.right && b.bottom >= a.bottom)
        }
        // the rows scroll past the list's clip; the bounds stop at it
        assertTrue(bounds.last().top >= 62)
    }

    @Test
    fun `a window whose shape keeps changing is drawn in one piece until it rests`() {
        frame()
        tickerScroll++
        frame()
        tickerScroll++
        assertEquals(1, frame().runs.size)
        repeat(3) {
            shape = Region.Box(Rect(0, 0, 200, 90 + it))
            tickerScroll++
            frame()
        }
        tickerScroll++
        assertTrue("flat", frame().runs.isEmpty())
        repeat(10) {
            tickerScroll++
            assertTrue("still flat", frame(ms = 50).runs.isEmpty())
        }
        tickerScroll++
        assertEquals("kept apart again after a second at rest", 1, frame(ms = 600).runs.size)
    }

    @Test
    fun `more runs than allowed, or runs over half the window, draw the window in one piece`() {
        val many = (0..DrawPlanner.MAX_RUNS).map { Obj("live$it") }
        val still = (0..DrawPlanner.MAX_RUNS).map { Obj("still$it") }

        fun cmds(step: Int) =
            listOf(image(Rect(0, 0, 200, 100)).by(layout)) +
                many.indices.flatMap { i ->
                    listOf(
                        text("$step", Rect(i * 14, 0, 7, 7)).by(many[i]),
                        text("x", Rect(i * 14 + 7, 0, 7, 7)).by(still[i]),
                    )
                }
        var plan: WindowPlan? = null
        repeat(4) { plan = planner.planWindow(key, window, cmds(it), now + it * 16L) }
        assertTrue("one run each would be ${many.size}", plan!!.runs.isEmpty())

        val big = Obj("big")
        val other = Any()
        repeat(4) {
            plan = planner.planWindow(other, window, listOf(text("$it", Rect(0, 0, 150, 80)).by(big)), now + it * 16L)
        }
        assertTrue("one run over 60 % of the window", plan!!.runs.isEmpty())
    }

    @Test
    fun `boxes and clips with swapped corners count where the canvas draws them`() {
        var step = 0

        // a group squeezed to a negative height clips from its bottom to its top, and its live
        // children draw a box and text whose corners are swapped too
        fun cmds() =
            listOf(
                image(Rect(0, 0, 200, 100)).by(layout),
                DrawCmd.Clip(Rect(10, 80, 100, -40)).by(group),
                image(Rect(120, 10, 20, 10)).by(button),
                DrawCmd.Fill(Rect(10, 70, 50, -20), Rgb(step), 255).by(time),
                text("$step", Rect(60, 70, -20, -10)).by(ticker),
                DrawCmd.Unclip,
            )
        val other = Any()
        var plan = planner.planWindow(other, window, cmds(), now)
        repeat(3) {
            step++
            now += 16
            val c = cmds()
            plan = planner.planWindow(other, window, c, now, moving = listOf(time, ticker))
            cache.frame(listOf(window to c), listOf(plan), "step $step")
        }
        assertEquals("the box and the text, inside the group's clip", Rect(10, 50, 50, 20), plan.runs.single().bounds)
    }

    @Test
    fun `a window of negative size is drawn in one piece`() {
        val flat = Rect(0, 0, -200, -100)
        val other = Any()
        repeat(4) {
            tickerScroll++
            now += 16
            val c = cmds()
            val plan = planner.planWindow(other, flat, c, now, moving = listOf(ticker))
            assertTrue(plan.runs.isEmpty())
        }
    }

    @Test
    fun `a window that changes size moves its version`() {
        val before = frame()
        val plan = planner.planWindow(key, Rect(0, 0, 200, 120), cmds(), now + 16)
        assertNotEquals(before.version, plan.version)
    }

    @Test
    fun `a renderer that sees only some frames never shows a kept drawing that changed`() {
        val random = Random(7)
        val skipping = PlanCache()
        repeat(600) {
            if (random.nextInt(4) == 0) timeText = "0:${random.nextInt(60)}"
            if (random.nextInt(2) == 0) visPixels = intArrayOf(random.nextInt(3), 0, 0, 0)
            if (random.nextInt(3) == 0) tickerScroll++
            if (random.nextInt(40) == 0) labelText = "label ${random.nextInt(3)}"
            if (random.nextInt(25) == 0) listRow = random.nextInt(4)
            if (random.nextInt(90) == 0) shape = Region.Box(Rect(0, 0, 200, 90 + random.nextInt(10)))
            now += 16L * (1 + random.nextInt(8))
            val c = cmds()
            val plan = planner.planWindow(key, window, c, now, if (random.nextBoolean()) listOf(vis) else emptyList())
            if (random.nextInt(3) != 0) skipping.frame(listOf(window to c), listOf(plan), "frame $it")
        }
        assertTrue(skipping.runRecords > 0)
    }
}
