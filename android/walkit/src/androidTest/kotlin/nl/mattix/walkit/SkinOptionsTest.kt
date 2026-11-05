// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File

/**
 * The options a skin's scripts list for Winamp's Options menu and a pick of one, through the
 * binding, on skins of the reference corpus pushed to `/data/local/tmp/walkit/skins` (see
 * [CorpusTraceTest]); skipped without them.
 */
@RunWith(AndroidJUnit4::class)
class SkinOptionsTest {
    private val skins = File("/data/local/tmp/walkit/skins")

    /** [wal] of the corpus loaded and ticked five times on a clock stuck at 0, as the reference tests run it. */
    private fun runtime(wal: String): SkinRuntime {
        val file = File(skins, wal)
        assumeTrue("$file is pushed", file.exists())
        val files = ZipSkinFiles(file)
        val rt = SkinRuntime(files, TraceHost(files, StringBuilder()), FakePlayer().apply { clock = { 0L } }, clock = { 0L })
        rt.load()
        repeat(5) { rt.tick() }
        return rt
    }

    private fun List<SkinOptions.Entry>.toggle(text: String) = filterIsInstance<SkinOptions.Toggle>().single { it.text == text }

    private fun List<SkinOptions.Entry>.sub(text: String) =
        filterIsInstance<SkinOptions.Submenu>().single { it.text == text }.entries

    private fun SkinRuntime.drawers() =
        skinOptions()
            .entries
            .sub("Winamp Modern")
            .sub("Drawers")

    @Test
    fun winampModernListsItsOptionsAsWinampDoesAndItsScriptsKeepADrawerPairToOneChoice() {
        runtime("WinampModern566.wal").use { rt ->
            val top = rt.skinOptions().entries
            assertEquals(listOf("Winamp Modern"), top.filterIsInstance<SkinOptions.Submenu>().map { it.text })
            assertEquals(1, top.size)
            val modern = top.sub("Winamp Modern")
            assertEquals(
                listOf("Drawers", "Menus", "Main Windowshade Mode", "Vis Shortcut Button", "Notifications", "Songticker"),
                modern.filterIsInstance<SkinOptions.Submenu>().map { it.text },
            )
            assertFalse(modern.sub("Drawers").toggle("Animate Config Drawer").checked)

            val top1 = rt.drawers().toggle("Open Video/Vis from the top")
            if (top1.checked) rt.toggleSkinOption(rt.drawers().toggle("Open Video/Vis from the bottom").key)
            assertFalse(rt.drawers().toggle("Open Video/Vis from the top").checked)

            assertTrue(rt.toggleSkinOption(top1.key))
            assertTrue(rt.drawers().toggle("Open Video/Vis from the top").checked)
            assertFalse(rt.drawers().toggle("Open Video/Vis from the bottom").checked)

            rt.toggleSkinOption(top1.key)
            assertTrue(rt.drawers().toggle("Open Video/Vis from the top").checked)
        }
    }

    @Test
    fun winampModernsDrawerPairAreRadioButtonsAndItsOtherOptionsAreNot() {
        runtime("WinampModern566.wal").use { rt ->
            val drawers = rt.drawers()
            assertTrue(drawers.toggle("Open Video/Vis from the top").radio)
            assertTrue(drawers.toggle("Open Video/Vis from the bottom").radio)
            assertFalse(drawers.toggle("Animate Config Drawer").radio)
        }
    }

    @Test
    fun bigBentoListsEightEntriesAtTheTop() {
        runtime("BigBento.wal").use { rt ->
            assertEquals(8, rt.skinOptions().entries.size)
        }
    }

    @Test
    fun theVersionMovesWithAChange() {
        runtime("WinampModern566.wal").use { rt ->
            val before = rt.skinOptions()
            assertEquals(rt.configVersion, before.version)
            rt.toggleSkinOption(rt.drawers().toggle("Animate Config Drawer").key)
            assertTrue(rt.configVersion != before.version)
            assertTrue(rt.drawers().toggle("Animate Config Drawer").checked)
        }
    }
}
