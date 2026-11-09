// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

/**
 * A mark a host puts on classic playlist rows: 1-bit pixel art, one string per row of pixels with
 * `#` for ink, drawn a pixel to the skin's pixel in the row's color; or, without any, [text] in the
 * row's letters.
 */
public data class RowMark(
    val pixels: List<String> = emptyList(),
    val text: String = "",
)

/** Where a row's mark goes: between its number and its title, or right ahead of its length. */
public enum class MarkPlace { BEFORE, AFTER }

/**
 * The marks on a classic playlist's rows ([SkinRuntime.setClassicRowMarks]): every mark there is,
 * each row's mark by its place in the playlist (an index into [marks], or null for none), and
 * where they go. Each mark sits in a cell as wide as the widest of [marks], so titles line up.
 */
public data class RowMarks(
    val marks: List<RowMark>,
    val rows: List<Int?>,
    val place: MarkPlace = MarkPlace.AFTER,
) {
    public companion object {
        /** No marks. */
        public val NONE: RowMarks = RowMarks(emptyList(), emptyList())
    }
}
