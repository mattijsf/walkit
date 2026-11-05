// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

/**
 * How a renderer may keep one window's drawing between frames, as the engine works it out: the
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

/** A frame's windows as [SkinRuntime.drawWindows] gives them, and the plan of each, in the same order. */
public class Planned(
    public val stack: List<Pair<Rect, List<DrawCmd>>>,
    public val plans: List<WindowPlan>,
)
