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
 * Plans how a renderer keeps each window's drawing between frames: which stretches change
 * ([LiveRun]) and which stay. Not ported yet: the planner needs the engine to say which object
 * drew each command, which this binding's frames do not carry, so [draw] refuses. A renderer draws
 * every window whole from [SkinRuntime.drawWindows] until it lands.
 */
public class DrawPlanner {
    /** A frame's windows as [SkinRuntime.drawWindows] gives them, and the plan of each, in the same order. */
    public class Planned(
        public val stack: List<Pair<Rect, List<DrawCmd>>>,
        public val plans: List<WindowPlan>,
    )

    /** Nanoseconds spent planning, the drawing itself left out; for timing runs. */
    public var planNanos: Long = 0L
        private set

    /** Windows planned from the start; for tests and timing runs. */
    public var fullPlans: Long = 0L
        private set

    /** Refuses: see the class. */
    public fun draw(
        rt: SkinRuntime,
        nowMs: Long,
    ): Planned = throw UnsupportedOperationException("walkit does not port DrawPlanner yet (runtime of ${rt.skinName} at $nowMs)")

    public companion object {
        /** How long a window stays live after its last change, in ms. */
        public const val LIVE_HOLD_MS: Long = 1_000
    }
}
