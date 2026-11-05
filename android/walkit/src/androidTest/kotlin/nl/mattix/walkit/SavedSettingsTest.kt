// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith

/**
 * What a skin keeps between runs moves in and out as one value through the binding:
 * [SkinRuntime.restoreSettings] before the skin loads, [SkinRuntime.savedSettings] whenever the
 * host saves.
 */
@RunWith(AndroidJUnit4::class)
class SavedSettingsTest {
    private val files =
        InlineSkinFiles(
            "player.xml" to
                """<container id="main" name="t" default_visible="1"><layout id="normal" w="300" h="40"/></container>""",
        )

    private fun runtime() = SkinRuntime(files, TraceHost(files, StringBuilder()), FakePlayer())

    private val saved =
        SavedSettings(
            attributes = mapOf("Animate Config Drawer" to "0"),
            strings = mapOf("bento/component" to "Video"),
            ints = mapOf("bento/width" to 412),
        )

    @Test
    fun aNewSkinKeepsNothing() {
        runtime().use { rt ->
            assertEquals(SavedSettings(), rt.savedSettings())
        }
    }

    @Test
    fun whatIsPutBackBeforeLoadComesOutAgainAndCountsAsNoChange() {
        runtime().use { rt ->
            rt.restoreSettings(saved)
            rt.load()
            assertEquals(saved, rt.savedSettings())
            assertEquals(0, rt.settingsVersion)
        }
    }

    @Test
    fun whatIsPutBackIsACopy() {
        runtime().use { rt ->
            rt.restoreSettings(saved)
            rt.load()
            val before = rt.savedSettings()
            rt.restoreSettings(SavedSettings(strings = mapOf("bento/component" to "Playlist")))
            assertEquals("Video", before.strings["bento/component"])
            assertEquals("Playlist", rt.savedSettings().strings["bento/component"])
        }
    }
}
