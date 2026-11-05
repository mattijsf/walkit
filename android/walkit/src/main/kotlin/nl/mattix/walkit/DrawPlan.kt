// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

/**
 * How a renderer may keep one window's drawing between frames, as [DrawPlanner] works it out: the
 * stretches of the window's commands that change from frame to frame ([runs]), each kept apart,
 * and a [version] for the rest. A renderer that kept what it drew of the window for this [id] and
 * [version] may show it again; the same goes for each run by its id and version. Ids and versions
 * are never handed out twice in a process.
 */
public class WindowPlan(
    /** The same while the same window layout is drawn. */
    public val id: Long,
    /** Moves when anything of the window outside its runs draws differently, or its runs are others. */
    public val version: Long,
    /** The changing stretches, in drawing order; none when the window is best drawn in one piece. */
    public val runs: List<LiveRun>,
)

/**
 * A stretch of a window's commands that changes from frame to frame, such as a visualizer, a
 * ticker or the time: commands [from] (inclusive) to [to] (exclusive) of the window's list. It
 * holds whole objects' drawing and no window structure, so its clips open and close inside it.
 */
public class LiveRun(
    /** The same while the stretch starts with the same object. */
    public val id: Long,
    /** Moves when any of its commands draws differently, when it holds other objects, or when [bounds] grew. */
    public val version: Long,
    public val from: Int,
    public val to: Int,
    /**
     * In window pixels: every pixel its commands can change that the clips in force there let
     * through lies inside. It never shrinks while the run keeps its id.
     */
    public val bounds: Rect,
)

/**
 * Draws a runtime's windows as [SkinRuntime.drawWindows] does, and works out for each window what a
 * renderer may keep from the frames before ([WindowPlan]): the objects that change from frame to
 * frame, kept apart as live runs, and the rest of the window.
 *
 * Each object's drawing is compared, command by command ([DrawCmd.drawsSameAs]), with what it drew
 * in the frame before, so a change shows whether or not the object asked for a frame. An object
 * that changes again within [LIVE_GAP_MS] of its last change, or that the runtime's tick reported
 * moving, is live until [LIVE_HOLD_MS] after its last change. Neighboring live objects form one
 * run, unless they lie far apart. A change outside the runs moves the window's version; a change
 * inside one moves that run's.
 *
 * A window is drawn in one piece (no runs) while its shape, the clips of its groups or its resting
 * objects keep changing ([FLAT_MOVES] of the last four frames), until they rest for
 * [FLAT_EXIT_MS]; also when it would have more than [MAX_RUNS] runs, or runs over more than half
 * of it, when its shape opens anywhere but at its ends, and while it has no width or height.
 *
 * A window laid out as in the frame before, with only its objects' drawing changed and no object
 * turning live or resting, is compared command by command with that frame and keeps its runs
 * ([QuickLook]); any other is planned from the start ([FullPlan]).
 *
 * Used on one thread (the engine's), one planner for one runtime.
 */
public class DrawPlanner {
    /** A frame's windows as [SkinRuntime.drawWindows] gives them, and the plan of each, in the same order. */
    public class Planned(
        public val stack: List<Pair<Rect, List<DrawCmd>>>,
        public val plans: List<WindowPlan>,
    )

    // objects and windows are keyed by what the engine names them with: an id for a frame handed
    // over the boundary, any object in a test
    private val owners = HashMap<Any, Owned>()
    private val windowStates = HashMap<Any, WindowState>()
    private var frames = 0L
    private val full = FullPlan(owners)
    private val quick = QuickLook()

    /** Nanoseconds spent planning, the drawing itself left out; for timing runs. */
    public var planNanos: Long = 0L
        private set

    /** Windows planned from the start, and windows that took the quick look; for tests and timing runs. */
    public var fullPlans: Long = 0L
        private set
    internal var quickPlans = 0L
        private set

    /**
     * [rt]'s windows, drawn as [SkinRuntime.drawWindows] draws them, with a plan for each, at
     * [nowMs] in any clock that only moves forward.
     */
    public fun draw(
        rt: SkinRuntime,
        nowMs: Long,
    ): Planned {
        rt.ensureLayout()
        frames++
        val frame = rt.drawPlanned()
        for (o in frame.changed) owners[o]?.hinted = frames
        val stack = ArrayList<Pair<Rect, List<DrawCmd>>>(frame.windows.size)
        val plans = ArrayList<WindowPlan>(frame.windows.size)
        for (w in frame.windows) {
            stack.add(w.rect to w.cmds)
            val t0 = System.nanoTime()
            plans.add(plan(w.id.raw, w.rect, w.cmds, nowMs))
            planNanos += System.nanoTime() - t0
        }
        if (frames % PRUNE_EVERY == 0L) prune()
        return Planned(stack, plans)
    }

    /**
     * One window's plan as [draw] works it out, as a frame by itself at [nowMs]: [key] stands for
     * its layout and [moving] for the objects the tick reported moving. For tests.
     */
    internal fun planWindow(
        key: Any,
        rect: Rect,
        cmds: List<DrawCmd>,
        nowMs: Long,
        moving: Collection<Any> = emptyList(),
    ): WindowPlan {
        frames++
        for (o in moving) owners[o]?.hinted = frames
        return plan(key, rect, cmds, nowMs)
    }

    private fun plan(
        layout: Any,
        rect: Rect,
        cmds: List<DrawCmd>,
        now: Long,
    ): WindowPlan {
        val ws = windowStates.getOrPut(layout) { WindowState(nextId()) }
        ws.seen = frames
        quick.plan(ws, rect, cmds, now, frames)?.let {
            quickPlans++
            return it
        }
        fullPlans++
        return full.plan(ws, rect, cmds, now, frames)
    }

    /** Forgets windows not drawn for [PRUNE_EVERY] frames, and objects no window drew in that time. */
    private fun prune() {
        val before = frames - PRUNE_EVERY
        windowStates.values.removeIf { it.seen < before }
        // the quick look leaves the objects of a window it planned unvisited
        for (ws in windowStates.values) for (j in 0 until ws.count) ws.owned[j]?.seen = frames
        owners.values.removeIf { it.seen < before }
        for (rec in owners.values) while (rec.atoms.size > maxOf(1, rec.used)) rec.atoms.removeAt(rec.atoms.size - 1)
    }

    public companion object {
        /**
         * A change this soon after an object's last one makes it live: the time once a second, and
         * a seek thumb that moves a pixel every few seconds on a long track.
         */
        internal const val LIVE_GAP_MS = 5_000L

        /**
         * How long a live object stays live after its last change: through a ticker's pause at
         * the end of its text, so the window is not recorded again as it stops and starts.
         */
        public const val LIVE_HOLD_MS: Long = 10_000L

        /**
         * A window is drawn in one piece once its shape, clips or resting objects changed in this
         * many of its last four frames.
         */
        internal const val FLAT_MOVES = 3

        /** A window drawn in one piece for that is kept apart again once those rested this long. */
        internal const val FLAT_EXIT_MS = 1_000L

        /** More runs than this, and a window is drawn in one piece. */
        internal const val MAX_RUNS = 12

        /** Objects and windows not drawn for this many frames are forgotten. */
        internal const val PRUNE_EVERY = 64L

        private val ids =
            java.util.concurrent.atomic
                .AtomicLong()

        /** An id or version no planner in the process has handed out. */
        internal fun nextId(): Long = ids.incrementAndGet()
    }
}
