// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

import org.junit.Assert.assertEquals
import org.junit.Test

/** On a plain JVM the engine comes from the test fixtures' host build: a skin loads and draws. */
class HostLibraryTest {
    private val files =
        object : SkinFiles {
            private val m =
                mapOf(
                    "skin.xml" to
                        """<WinampAbstractionLayer version="0.8"><include file="player.xml"/></WinampAbstractionLayer>""",
                    "player.xml" to
                        """<container id="main" name="t" default_visible="1"><layout id="normal" w="200" h="100">""" +
                        """<button id="play" action="PLAY" x="10" y="10" w="20" h="20"/></layout></container>""",
                ).mapValues { it.value.toByteArray() }

            override fun read(path: String): ByteArray? = m[SkinFiles.normalize(path)]

            override fun names(): List<String> = m.keys.toList()
        }

    private val host =
        object : SkinHost {
            override fun imageSize(file: String): Pair<Int, Int>? = null

            override fun textWidth(
                text: String,
                font: FontDef?,
                size: Int,
            ): Int = text.length * size / 2

            override fun log(message: String) = Unit

            override fun invalidate() = Unit
        }

    @Test
    fun `a skin loads and draws on the host build`() {
        SkinRuntime(files, host, FakePlayer(), clock = { 0L }).use { rt ->
            rt.load()
            val main = checkNotNull(rt.mainWindow)
            assertEquals(200 to 100, rt.windowSize(main))
            rt.tick()
            assertEquals(1, rt.drawWindows().size)
        }
    }

    @Test
    fun `a host sets the classic switches and reads them back`() {
        SkinRuntime(files, host, FakePlayer(), clock = { 0L }).use { rt ->
            rt.load()
            assertEquals(ClassicSwitches(), rt.classicSwitches)
            val set = ClassicSwitches(timeRemaining = true, eqAuto = true, visMode = ClassicVisMode.SCOPE)
            rt.classicSwitches = set
            assertEquals(set, rt.classicSwitches)
            rt.setClassicPlaylistSelection(listOf(0, 3))
        }
    }
}
