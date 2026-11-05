// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

/**
 * A skin written in place for a test: `skin.xml` including `player.xml`, and the files given by
 * path and text.
 */
class InlineSkinFiles(
    vararg files: Pair<String, String>,
) : SkinFiles {
    private val m =
        (
            listOf(
                "skin.xml" to
                    """<WinampAbstractionLayer version="0.8"><include file="player.xml"/></WinampAbstractionLayer>""",
            ) + files
        ).associate { (path, text) -> SkinFiles.normalize(path) to text.toByteArray() }

    override fun read(path: String): ByteArray? = m[SkinFiles.normalize(path)]

    override fun names(): List<String> = m.keys.toList()
}
