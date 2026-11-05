// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith

/**
 * A skin names itself in `<skininfo><name>`, which the app falls back on when it has no name for
 * it, and the app may name it before or after it loads, through the binding.
 */
@RunWith(AndroidJUnit4::class)
class SkinNameTest {
    private val files =
        InlineSkinFiles(
            "skin.xml" to
                """<WinampAbstractionLayer version="0.8">""" +
                """<skininfo><version>1.0</version><name>Big Bento</name><author>Victor</author></skininfo>""" +
                """<include file="player.xml"/></WinampAbstractionLayer>""",
            "player.xml" to
                """<container id="main" name="t" default_visible="1"><layout id="normal" w="200" h="100"/></container>""",
        )

    private fun runtime(setup: SkinSetup = SkinSetup()) =
        SkinRuntime(files, TraceHost(files, StringBuilder()), FakePlayer(), setup)

    @Test
    fun aSkinTheAppHasNoNameForTakesTheOneInSkininfo() {
        runtime().use { rt ->
            rt.load()
            assertEquals("Big Bento", rt.skinName)
        }
    }

    @Test
    fun aNameTheAppGaveStays() {
        runtime(SkinSetup(skinName = "Bento")).use { rt ->
            rt.load()
            assertEquals("Bento", rt.skinName)
        }
    }

    @Test
    fun aNameGivenAfterLoadIsWhatScriptsReadFromThenOn() {
        runtime().use { rt ->
            rt.load()
            rt.skinName = "Other"
            assertEquals("Other", rt.skinName)
        }
    }
}
