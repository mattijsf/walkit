// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith

/**
 * A drawer that shows Winamp's visualizer, through the binding: where the app draws its
 * visualizer ([SkinRuntime.visRect]), and the app's visualizer window closing
 * ([SkinRuntime.visualizerWindowClosed]).
 */
@RunWith(AndroidJUnit4::class)
class VisDrawerTest {
    // a main window 200 by 100 whose drawer group, 40 px high, shows 30 px of a visualizer holder
    // under a 10 px button bar
    private fun files(holderShown: Boolean) =
        InlineSkinFiles(
            "player.xml" to
                """<container id="main" name="t" default_visible="1"><layout id="normal" w="200" h="100">""" +
                """<group id="drawer" x="10" y="60" w="180" h="40">""" +
                """<windowholder id="vis" x="0" y="10" w="180" h="60" hold="guid:avs" """ +
                """visible="${if (holderShown) 1 else 0}"/>""" +
                """</group></layout></container>""",
        )

    private fun loaded(holderShown: Boolean): SkinRuntime {
        val files = files(holderShown)
        return SkinRuntime(files, TraceHost(files, StringBuilder()), FakePlayer().apply { clock = { 0L } }, clock = { 0L })
            .apply {
                viewSize = 200 to 1000
                load()
            }
    }

    @Test
    fun theVisualizerShowsWhereItsHolderShowsCutToTheDrawerAroundIt() {
        loaded(holderShown = true).use { rt ->
            assertEquals(Rect(10, 70, 180, 30), rt.visRect)
        }
    }

    @Test
    fun aHiddenHolderShowsNoVisualizer() {
        loaded(holderShown = false).use { rt ->
            assertNull(rt.visRect)
        }
    }

    @Test
    fun theVisualizerWindowClosingWithNothingShownChangesNothing() {
        loaded(holderShown = true).use { rt ->
            rt.visualizerWindowClosed()
            assertEquals(1, rt.windowIds.size)
            assertEquals(Rect(10, 70, 180, 30), rt.visRect)
        }
    }
}
