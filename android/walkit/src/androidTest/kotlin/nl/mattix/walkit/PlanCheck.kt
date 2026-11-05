// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue

/**
 * What every plan must hold for a window of [rect]'s size drawing [cmds]: runs in order, apart and
 * inside the list; no window shape in a run and its clips paired inside it; bounds inside the window
 * that hold every pixel a run's commands can draw through the clips in force there. Boxes and clips
 * are taken as the platform's canvas takes them: with a right or bottom before the left or top, they
 * cover what lies between; a picture or pixels with such a destination draw nothing.
 */
internal fun checkPlan(
    cmds: List<DrawCmd>,
    rect: Rect,
    plan: WindowPlan,
    what: String = "",
) {
    var prev = 0
    for (run in plan.runs) {
        assertTrue(
            "$what: run ${run.from}..${run.to} after $prev in ${cmds.size}",
            run.from >= prev && run.from < run.to && run.to <= cmds.size,
        )
        prev = run.to
        val b = run.bounds
        assertTrue(
            "$what: bounds $b in ${rect.width}x${rect.height}",
            b.left >= 0 && b.top >= 0 && b.right <= rect.width && b.bottom <= rect.height,
        )
        assertTrue("$what: bounds $b empty", b.width > 0 && b.height > 0)
        checkPaired(cmds, run, what)
    }
    // the clip in force at each command outside the runs, and each run's commands against its bounds
    val clips = ArrayDeque<IntArray>()
    clips.addLast(sorted(0, 0, rect.width, rect.height))
    var i = 0
    var next = 0
    while (i < cmds.size) {
        val run = plan.runs.getOrNull(next)
        if (run != null && i == run.from) {
            checkBounds(cmds, run, clips.last(), what)
            i = run.to
            next++
            continue
        }
        when (val c = cmds[i]) {
            is DrawCmd.WindowStart -> {
                val box = c.region.bounds() ?: Rect(0, 0, 1, 1)
                clips.addLast(intersect(clips.last(), 0, 0, box.right, box.bottom))
            }

            is DrawCmd.Clip -> {
                clips.addLast(intersect(clips.last(), c.rect.left, c.rect.top, c.rect.right, c.rect.bottom))
            }

            DrawCmd.WindowEnd, DrawCmd.Unclip -> {
                clips.removeLast()
            }

            else -> {
                Unit
            }
        }
        i++
    }
}

/** A run holds no window shape, and closes every clip it opens. */
private fun checkPaired(
    cmds: List<DrawCmd>,
    run: LiveRun,
    what: String,
) {
    var depth = 0
    for (i in run.from until run.to) {
        when (cmds[i]) {
            is DrawCmd.WindowStart, DrawCmd.WindowEnd -> throw AssertionError("$what: shape in run at $i")
            is DrawCmd.Clip -> depth++
            DrawCmd.Unclip -> depth--
            else -> Unit
        }
        assertTrue("$what: clip closed before it opened in run at $i", depth >= 0)
    }
    assertEquals("$what: clips open at the end of the run", 0, depth)
}

/** Every pixel [run]'s commands draw through [outer] and the clips they open lies inside its bounds. */
private fun checkBounds(
    cmds: List<DrawCmd>,
    run: LiveRun,
    outer: IntArray,
    what: String,
) {
    val inner = ArrayDeque<IntArray>()
    inner.addLast(outer)
    for (k in run.from until run.to) {
        val cmd = cmds[k]
        if (cmd is DrawCmd.Clip) {
            inner.addLast(
                intersect(inner.last(), cmd.rect.left, cmd.rect.top, cmd.rect.right, cmd.rect.bottom),
            )
        }
        if (cmd === DrawCmd.Unclip) inner.removeLast()
        val e = extentOf(cmd) ?: continue
        val d = intersect(inner.last(), e.left, e.top, e.right, e.bottom)
        if (d[2] <= d[0] || d[3] <= d[1]) continue
        val b = run.bounds
        assertTrue(
            "$what: command $k draws on ${d.toList()} outside $b",
            d[0] >= b.left && d[1] >= b.top && d[2] <= b.right && d[3] <= b.bottom,
        )
    }
}

/** The box from ([x0], [y0]) to ([x1], [y1]) as left, top, right, bottom, whichever corner comes first. */
private fun sorted(
    x0: Int,
    y0: Int,
    x1: Int,
    y1: Int,
) = intArrayOf(minOf(x0, x1), minOf(y0, y1), maxOf(x0, x1), maxOf(y0, y1))

/** Clip [c] cut down by the box from ([x0], [y0]) to ([x1], [y1]), whichever corner comes first. */
private fun intersect(
    c: IntArray,
    x0: Int,
    y0: Int,
    x1: Int,
    y1: Int,
): IntArray {
    val s = sorted(x0, y0, x1, y1)
    return intArrayOf(maxOf(c[0], s[0]), maxOf(c[1], s[1]), minOf(c[2], s[2]), minOf(c[3], s[3]))
}

private fun extentOf(c: DrawCmd): Rect? =
    when (c) {
        is DrawCmd.Image -> c.dst.takeIf { it.width > 0 && it.height > 0 }
        is DrawCmd.Pixels -> c.rect.takeIf { it.width > 0 && it.height > 0 }
        is DrawCmd.Fill -> c.rect
        is DrawCmd.Text -> c.rect
        is DrawCmd.BitmapText -> c.rect
        else -> null
    }

/**
 * A renderer that keeps each window and each run while its version holds, as the app's layers do,
 * and checks that what it keeps draws the same as the frame it is handed: the same version must
 * always mean the same drawing, also across frames it never saw.
 */
internal class PlanCache {
    private val windows = HashMap<Long, Pair<Long, List<Any>>>()
    private val runs = HashMap<Long, Pair<Long, List<DrawCmd>>>()

    /** Windows and runs recorded again, over all frames handed in. */
    var windowRecords = 0
        private set
    var runRecords = 0
        private set

    fun frame(
        stack: List<Pair<Rect, List<DrawCmd>>>,
        plans: List<WindowPlan>,
        what: String = "",
    ) {
        assertEquals("$what: a plan for each window", stack.size, plans.size)
        for ((i, plan) in plans.withIndex()) {
            val (rect, cmds) = stack[i]
            checkPlan(cmds, rect, plan, "$what window $i")
            for (run in plan.runs) {
                val drawn = cmds.subList(run.from, run.to)
                val kept = runs[run.id]
                if (kept != null && kept.first == run.version) {
                    assertTrue("$what: run ${run.id} kept at version ${run.version} draws differently", same(kept.second, drawn))
                } else {
                    runs[run.id] = run.version to drawn.toList()
                    runRecords++
                }
            }
            val content = windowContent(rect, cmds, plan)
            val kept = windows[plan.id]
            if (kept != null && kept.first == plan.version) {
                assertTrue(
                    "$what: window ${plan.id} kept at version ${plan.version} draws differently",
                    same(kept.second, content),
                )
            } else {
                windows[plan.id] = plan.version to content
                windowRecords++
            }
        }
    }

    /** What a window's layer draws: its size, its commands outside runs and, in their places, the runs' ids. */
    private fun windowContent(
        rect: Rect,
        cmds: List<DrawCmd>,
        plan: WindowPlan,
    ): List<Any> {
        val out = ArrayList<Any>()
        out.add(rect.width to rect.height)
        var i = 0
        var next = 0
        while (i < cmds.size) {
            val run = plan.runs.getOrNull(next)
            if (run != null && i == run.from) {
                out.add(run.id)
                i = run.to
                next++
            } else {
                out.add(cmds[i++])
            }
        }
        return out
    }

    private fun same(
        a: List<Any>,
        b: List<Any>,
    ): Boolean =
        a.size == b.size &&
            a.indices.all { k ->
                a[k].let { x -> if (x is DrawCmd) (b[k] as? DrawCmd)?.let(x::drawsSameAs) == true else x == b[k] }
            }
}
