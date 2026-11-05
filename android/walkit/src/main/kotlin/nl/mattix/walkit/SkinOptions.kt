// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

/**
 * The options a skin's scripts list for Winamp's Options menu, as of [version]
 * ([SkinRuntime.configVersion] when the list was read). Two are equal when they list the same
 * entries, whatever their versions.
 */
public class SkinOptions(
    public val version: Int,
    public val entries: List<Entry>,
) {
    override fun equals(other: Any?): Boolean = other is SkinOptions && other.entries == entries

    override fun hashCode(): Int = entries.hashCode()

    override fun toString(): String = "SkinOptions(version=$version, entries=$entries)"

    /** One line of the skin's options menu. */
    public sealed interface Entry

    /**
     * A checkable option; [checked] as Winamp ticks it: the value read as a number is not 0.
     * [radio] when the skin's page shows it as one of a set of radio buttons.
     */
    public data class Toggle(
        val key: Key,
        val text: String,
        val checked: Boolean,
        val radio: Boolean = false,
    ) : Entry

    /** A line between groups of options. */
    public data object Separator : Entry

    /** An attribute whose value is another item's GUID; [text] is the attribute's name. */
    public data class Submenu(
        val text: String,
        val entries: List<Entry>,
    ) : Entry

    /** An option by item GUID and lowercased attribute name; stays valid across snapshots. */
    public data class Key(
        val item: String,
        val attribute: String,
    )

    public companion object {
        public val EMPTY: SkinOptions = SkinOptions(0, emptyList())
    }
}
