// SPDX-License-Identifier: GPL-3.0-or-later

package nl.mattix.walkit

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Test

class WalArchiveTest {
    @Test
    fun `a classic skin zipped inside a folder has that folder stripped but is not modern`() {
        val names = listOf("Win-98_Ruby/", "Win-98_Ruby/Main.bmp", "Win-98_Ruby/pledit.txt")
        assertEquals("win-98_ruby/", WalArchive.folderOf(names))
        assertNull(WalArchive.skinFolder(names))
        assertFalse(WalArchive.isModern(names))
    }

    @Test
    fun `skin xml decides the folder over main bmp`() {
        assertEquals("new/", WalArchive.folderOf(listOf("old/main.bmp", "new/skin.xml")))
        assertEquals("", WalArchive.folderOf(listOf("main.bmp", "extra/main.bmp")))
        assertNull(WalArchive.folderOf(listOf("a.png")))
    }
}
