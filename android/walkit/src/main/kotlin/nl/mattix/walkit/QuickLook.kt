// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

/**
 * The quick look at a window ([DrawPlanner]): when it is laid out as in the frame before (the same
 * commands in kind and object, the same structure and size), and no object turns live or rests,
 * each stretch is compared in place with that frame, the runs are kept, and the versions of the
 * window and of the runs move where something changed. Otherwise the window is planned from the
 * start ([FullPlan]).
 */
internal class QuickLook {
    private var changedAt = IntArray(INITIAL)
    private var changedFlag = BooleanArray(INITIAL)
    private var runMoved = BooleanArray(DEPTH)
    private var boxes = arrayOfNulls<Extent>(DEPTH)
    private val extent = Extent()

    /** [cmds], the window [ws] keeps of [rect]'s size, at [now] in frame [frame]; null when it must be planned from the start. */
    fun plan(
        ws: WindowState,
        rect: Rect,
        cmds: List<DrawCmd>,
        now: Long,
        frame: Long,
    ): WindowPlan? {
        val prev = ws.cmds
        val plan = ws.plan
        val laidOut = prev != null && plan != null && prev.size == cmds.size && rect.width == ws.w && rect.height == ws.h
        if (!laidOut || ws.flatTurnsAt(now)) return null
        val changes = changes(ws, checkNotNull(prev), cmds)
        if (changes < 0 || !steady(ws, prev, cmds, changes, now, frame)) return null
        return commit(ws, checkNotNull(plan), cmds, now, frame, changes)
    }

    /** How many stretches of [ws] draw differently in [cmds] than in [prev], noted in [changedAt]; -1 when the layout differs. */
    private fun changes(
        ws: WindowState,
        prev: List<DrawCmd>,
        cmds: List<DrawCmd>,
    ): Int {
        if (changedAt.size < ws.count) {
            changedAt = IntArray(maxOf(ws.count, changedAt.size * 2))
            changedFlag = BooleanArray(changedAt.size)
        }
        var changes = 0
        for (j in 0 until ws.count) {
            val from = ws.from[j]
            val result =
                if (ws.owned[j] == null) {
                    if (cmds[from].drawsSameAs(prev[from])) SAME else KIND
                } else {
                    compare(prev, cmds, from, ws.to[j])
                }
            if (result == KIND) return -1
            if (result == CHANGED) changedAt[changes++] = j
        }
        return changes
    }

    /**
     * Whether the [changes] found leave every object as live or as resting as it was, and drawn by the
     * same object: in a window kept apart, that keeps its runs as they are.
     */
    @Suppress("LongParameterList") // the window, both frames and when
    private fun steady(
        ws: WindowState,
        prev: List<DrawCmd>,
        cmds: List<DrawCmd>,
        changes: Int,
        now: Long,
        frame: Long,
    ): Boolean {
        var steady = true
        for (c in 0 until changes) {
            val j = changedAt[c]
            val from = ws.from[j]
            if (cmds[from].ownedBy != prev[from].ownedBy) steady = false
            if (!ws.flat && (now < checkNotNull(ws.owned[j]).liveAfter(now, frame)) != ws.live[j]) steady = false
        }
        if (!steady || ws.flat) return steady
        for (c in 0 until changes) changedFlag[changedAt[c]] = true
        for (j in 0 until ws.count) {
            val rec = ws.owned[j] ?: continue
            if (ws.live[j] && !changedFlag[j] && now >= rec.liveUntil) steady = false
        }
        for (c in 0 until changes) changedFlag[changedAt[c]] = false
        return steady
    }

    /** The plan once nothing calls for planning from the start: the [changes] found applied. */
    @Suppress("LongParameterList") // the window, its plan, the frame and when
    private fun commit(
        ws: WindowState,
        plan: WindowPlan,
        cmds: List<DrawCmd>,
        now: Long,
        frame: Long,
        changes: Int,
    ): WindowPlan {
        val runCount = plan.runs.size
        if (runMoved.size < runCount) {
            runMoved = BooleanArray(runCount * 2)
            boxes = boxes.copyOf(runCount * 2)
        }
        for (r in 0 until runCount) runMoved[r] = false
        var outside = false
        var resting = false
        for (c in 0 until changes) {
            val j = changedAt[c]
            val rec = checkNotNull(ws.owned[j])
            val atom = checkNotNull(ws.atoms[j])
            rec.changed(now, frame)
            val live = now < rec.liveUntil
            ws.live[j] = live
            atom.cmds.clear()
            for (k in ws.from[j] until ws.to[j]) atom.cmds.add(cmds[k])
            val run = ws.runOf[j]
            if (run < 0) {
                outside = true
                if (!live) resting = true
            } else {
                extent.of(cmds, ws.from[j], ws.to[j], ws.clip, j * 4)
                val box = boxes[run] ?: Extent().also { boxes[run] = it }
                if (!runMoved[run]) box.clear()
                box.join(extent.l, extent.t, extent.r, extent.b)
                runMoved[run] = true
            }
        }
        ws.moved(resting, now)
        if (outside) ws.version = DrawPlanner.nextId()
        ws.cmds = cmds
        val runs = movedRuns(ws, plan)
        val next = if (outside || runs !== plan.runs) WindowPlan(ws.id, ws.version, runs) else plan
        ws.plan = next
        return next
    }

    /** [plan]'s runs with those that changed at new versions, grown by what changed in them; the same list when none did. */
    private fun movedRuns(
        ws: WindowState,
        plan: WindowPlan,
    ): List<LiveRun> {
        var runs: ArrayList<LiveRun>? = null
        for (r in plan.runs.indices) {
            if (!runMoved[r]) continue
            val state = ws.runStates[r]
            val old = plan.runs[r]
            val bounds = checkNotNull(boxes[r]).grow(state.bounds, ws.w, ws.h)
            state.bounds = bounds
            state.version = DrawPlanner.nextId()
            val list = runs ?: ArrayList(plan.runs).also { runs = it }
            list[r] = LiveRun(state.id, state.version, old.from, old.to, bounds ?: old.bounds)
        }
        return runs ?: plan.runs
    }

    /** Whether commands [from] to [to] draw the same in [prev] and [cmds], differ, or differ in kind. */
    private fun compare(
        prev: List<DrawCmd>,
        cmds: List<DrawCmd>,
        from: Int,
        to: Int,
    ): Int {
        var k = from
        while (k < to && cmds[k].drawsSameAs(prev[k])) k++
        if (k == to) return SAME
        while (k < to) {
            if (cmds[k].javaClass !== prev[k].javaClass) return KIND
            k++
        }
        return CHANGED
    }

    private companion object {
        const val INITIAL = 256
        const val DEPTH = 16
        const val SAME = 0
        const val CHANGED = 1
        const val KIND = 2
    }
}
