// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

/**
 * A popup menu a skin's script built and popped (`PopupMenu.popAtMouse`): what the host shows for
 * the listener to pick from. The script hears the picked command's id, or 0 for none.
 */
public class SkinMenu(
    public val items: List<Item>,
) {
    /** One line of the menu. */
    public sealed interface Item

    /** A command the script tells apart by its [id]; [text] reads as the listener sees it. */
    public class Command(
        public val id: Int,
        public val text: String,
        public val checked: Boolean,
        public val disabled: Boolean,
    ) : Item

    /** A line between groups of commands. */
    public data object Separator : Item

    /** [text] opening [menu]. */
    public class Submenu(
        public val text: String,
        public val menu: SkinMenu,
    ) : Item
}
