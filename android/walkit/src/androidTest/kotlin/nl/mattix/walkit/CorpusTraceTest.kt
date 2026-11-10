// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.json.JSONArray
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File

/**
 * Every trace scenario, run through the AAR on the device and written as a trace,
 * for the byte compare with the traces the same commit writes on a desktop: the proof that the
 * binding carries everything the engine does. The inputs are pushed to `/data/local/tmp/walkit` beforehand
 * (`scenarios.json`, the corpus skins in `skins/`, the small set in `small/` with
 * `scenarios-small.json`, `wasabi-base.zip`, the stand-in fonts in `fonts/`; see README.md);
 * the traces land in the test app's external files dir, to be pulled and diffed with
 * `tools/trace-diff.sh`. Skipped without the inputs.
 */
@RunWith(AndroidJUnit4::class)
class CorpusTraceTest {
    private val inputs = File("/data/local/tmp/walkit")

    @Test
    fun theSmallSkinRecordsTheSameTwice() {
        val small = File(inputs, "small/small.wal")
        assumeTrue("$small is pushed", small.exists())
        val sc = TraceScenario.winampReference("small", stack(ZipSkinFiles(small), File(inputs, "small")), "small")
        val trace = EngineTrace.record(sc)
        assertEquals(trace, EngineTrace.record(sc))
        assertTrue(trace.lines().contains("step load"))
        assertTrue(trace.lines().any { it.startsWith("frame t=0") })
    }

    @Test
    fun corpus() {
        run(File(inputs, "scenarios.json"), File(inputs, "skins"))
    }

    @Test
    fun smallSet() {
        run(File(inputs, "scenarios-small.json"), File(inputs, "small"))
    }

    private fun run(
        scenarios: File,
        skins: File,
    ) {
        assumeTrue("$scenarios is pushed", scenarios.exists())
        val out =
            File(
                InstrumentationRegistry.getInstrumentation().targetContext.getExternalFilesDir(null),
                "traces",
            ).apply { mkdirs() }
        val arr = JSONArray(scenarios.readText())
        var frames = 0
        for (i in 0 until arr.length()) {
            val o = arr.getJSONObject(i)
            val name = o.getString("name")
            val wal = File(skins, o.getString("wal"))
            if (!wal.exists()) {
                println("$name: no ${wal.name}")
                continue
            }

            fun strings(key: String) = o.optJSONArray(key)?.let { a -> (0 until a.length()).map { a.getString(it) } }

            fun pair(key: String) = o.optJSONArray(key)?.takeIf { it.length() == 2 }?.let { it.getInt(0) to it.getInt(1) }
            val sc =
                TraceScenario.winampReference(
                    name,
                    stack(ZipSkinFiles(wal), skins, o.optBoolean("noSiblings", false), o.optBoolean("noDefaults", false)),
                    o.getString("skin").removeSuffix(".wal"),
                    strings("playlist") ?: listOf("Andamp"),
                    theme = o.optString("theme", null).takeIf { !o.isNull("theme") },
                    view = pair("view"),
                    show = strings("show").orEmpty(),
                    mainSize = pair("mainsize"),
                    taps = strings("taps").orEmpty(),
                    haveLibrary = !o.optBoolean("noLibrary", false),
                )
            val started = System.nanoTime()
            val trace = EngineTrace.record(sc)
            val ms = (System.nanoTime() - started) / 1_000_000
            File(out, "$name.trace").writeText(trace)
            val n = trace.lines().count { it.startsWith("frame t=") }
            frames += n
            println("$name: ${trace.lines().size} lines, $n frames, $ms ms")
        }
        assertTrue(frames > 0)
    }

    /**
     * The skin with the other `.wal` files of its folder as siblings (by folder name, "BigBento.wal"
     * is "Big Bento"), Winamp's wasabi base under it, and the stand-in fonts in `fonts/` as the
     * engine asks for them.
     */
    private fun stack(
        skin: SkinFiles,
        folder: File,
        noSiblings: Boolean = false,
        noWasabi: Boolean = false,
    ): SkinFiles {
        var files = skin
        if (!noSiblings) {
            val siblings =
                folder
                    .listFiles { f -> f.name.endsWith(".wal") }
                    .orEmpty()
                    .associate { f -> f.nameWithoutExtension.replace(" ", "").lowercase() to ZipSkinFiles.OnDemand(f) }
            files = SkinsFolderFiles(files, siblings)
        }
        val wasabi = File(inputs, "wasabi-base.zip")
        if (!noWasabi && wasabi.exists()) files = LayeredSkinFiles(files, ZipSkinFiles(wasabi))
        return WithFonts(files, File(inputs, "fonts"))
    }

    /** [files] with the stand-in fonts of [dir] at `fonts/<name>`, as a host's wasabi base carries them. */
    private class WithFonts(
        private val files: SkinFiles,
        private val dir: File,
    ) : SkinFiles by files {
        override fun read(path: String): ByteArray? {
            val n = SkinFiles.normalize(path)
            if (n.startsWith("fonts/")) {
                val f = dir.listFiles()?.firstOrNull { it.name.lowercase() == n.removePrefix("fonts/") }
                if (f != null) return f.readBytes()
            }
            return files.read(path)
        }
    }
}
