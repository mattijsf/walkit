// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

/**
 * What a press at a point of a modern skin would take hold of ([SkinRuntime.pointerShapeAt]), for
 * the pointer icon a host shows there: nothing that is sized, a divider or a window's edge
 * dragged left and right or up and down, or a window's corner.
 */
public enum class PointerShape { ARROW, LEFT_RIGHT, UP_DOWN, DIAGONAL, OTHER_DIAGONAL }
