// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

/**
 * A window of a loaded skin, as the host names it: one layout of one container. A container that
 * switches layout (to its shade, say) shows another window. An id stays the same for as long as
 * the skin is loaded, and no other window of that skin gets it.
 */
@JvmInline
public value class WindowId(
    /** The engine's number for the window; a host that converts ids into another engine's types makes them from it. */
    public val raw: Int,
)

/** An edge of the viewport on one axis: the left or top one, or the right or bottom one. */
public enum class Edge { START, END }

/**
 * What the host reads of a window at one moment ([SkinRuntime.windowInfo]): its size as it shows
 * now, the minimum and maximum the skin sets (0 for none), whether the skin declares resize
 * handles across and up and down, the edges of the viewport a script last moved it against, and
 * the ids of its [container] and [layout] in the skin's XML. Read it on the engine thread where it
 * is needed: it does not follow later changes.
 */
public data class WindowInfo(
    val id: WindowId,
    val size: Pair<Int, Int>,
    val minW: Int,
    val minH: Int,
    val maxW: Int,
    val maxH: Int,
    val resizableBothWays: Boolean,
    val edgeX: Edge?,
    val edgeY: Edge?,
    val container: String = "",
    val layout: String = "",
)

/**
 * What the host reads of one object of a skin ([SkinRuntime.objectInfo]): the window it is in,
 * where it sits in that window ([rect]) and in the stack ([stackRect]), in layout pixels, whether
 * it shows, and its class as scripts name it (Button, Slider, Text, ...). A snapshot, like
 * [WindowInfo].
 */
public data class ObjectInfo(
    val window: WindowId,
    val rect: Rect,
    val stackRect: Rect,
    val visible: Boolean,
    val className: String,
)
