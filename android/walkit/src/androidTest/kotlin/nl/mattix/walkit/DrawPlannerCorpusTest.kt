// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File

/**
 * The engine's plans over the binding on the corpus skins pushed to `/data/local/tmp/walkit/skins`
 * (see [CorpusTraceTest]; skipped without them), playing on a stepped clock, then stopped, left
 * resting past the live hold and played again: every plan holds its promises, a renderer keeping
 * windows and runs by version never shows a stale picture, and a command unchanged since the frame
 * before is the same object. Once a skin has played a while no window is recorded again; the skins
 * that draw the most keep their moving parts apart, and those runs end once their objects rest.
 */
@RunWith(AndroidJUnit4::class)
class DrawPlannerCorpusTest {
    private val skins = File("/data/local/tmp/walkit/skins")

    @Test
    fun theCorpusSkinsKeepThePlansPromisesWhilePlaying() {
        assumeTrue("$skins is pushed", File(skins, SKINS[0]).exists())
        for (name in SKINS) play(name)
    }

    private fun files(name: String): SkinFiles {
        val siblings =
            skins
                .listFiles { f -> f.name.endsWith(".wal") }
                .orEmpty()
                .associate { f -> f.nameWithoutExtension.replace(" ", "").lowercase() to ZipSkinFiles.OnDemand(f) }
        return WithGeneratedImages(SkinsFolderFiles(ZipSkinFiles(File(skins, name)), siblings))
    }

    /** A runtime on [name] at the phone's size, its player on the stepped clock [now]. */
    private fun runtime(
        name: String,
        now: () -> Long,
    ): Pair<SkinRuntime, FakePlayer> {
        val files = files(name)
        val player =
            FakePlayer().apply {
                clock = now
                lengthMs = 600_000
                title = "Andamp - a title long enough to scroll in every ticker"
            }
        val rt = SkinRuntime(files, TraceHost(files, StringBuilder()), player, clock = now)
        rt.load()
        // a window that resizes both ways fills the phone, as the app fits Bento
        if (name.contains("Bento")) rt.mainWindow?.let { rt.resizeWindow(it, W, H) }
        rt.viewSize = W to H
        return rt to player
    }

    /** One skin playing: [PLAY_MS] of play, then Stop, a rest past the live hold, and play again. */
    private fun play(name: String) {
        var t = START
        val (rt, player) = runtime(name) { t }
        val run = Run(name, rt)
        repeat(SETTLE_FRAMES) {
            t += FRAME
            rt.tick()
        }
        player.play()
        val playing = t
        var steadyRecords = -1
        var steadyRuns = 0
        while (t < playing + PLAY_MS) {
            t += FRAME
            // the last seconds of play: objects that move turned live long before
            val steady = t >= playing + PLAY_MS - STEADY_MS
            if (steady && steadyRecords < 0) steadyRecords = run.cache.windowRecords
            val plans = run.frame(t) ?: continue
            if (steady) steadyRuns = maxOf(steadyRuns, plans.sumOf { it.runs.size })
        }
        assertEquals(
            "$name: no window recorded again in the last ${STEADY_MS / 1000} s of play",
            steadyRecords,
            run.cache.windowRecords,
        )
        if (name in KEPT_APART) assertTrue("$name keeps moving parts apart while it plays", steadyRuns > 0)
        player.stop()
        val stopped = t
        while (t < stopped + STOP_MS) {
            t += FRAME
            run.frame(t)
        }
        // nothing drawn for longer than the live hold, then a frame, then play again
        t += LIVE_HOLD_MS + FRAME
        run.frame(t, force = true)
        player.play()
        val again = t
        while (t < again + STOP_MS) {
            t += FRAME
            run.frame(t)
        }
        if (name in KEPT_APART) assertTrue("$name: runs end once their objects rest", run.runsEnded > 0)
        assertTrue("$name: commands unchanged between frames are the same objects", run.sameObjects > 0)
        rt.close()
    }

    /** A skin's runtime planned frame by frame, checked each frame. */
    private inner class Run(
        val name: String,
        val rt: SkinRuntime,
    ) {
        val cache = PlanCache()
        var runsEnded = 0
        var sameObjects = 0
        private var frames = 0
        private var lastRuns = emptySet<Long>()
        private var last: List<List<DrawCmd>> = emptyList()

        /** Ticks the runtime at [t]; when it changed, or with [force], plans and checks a frame and gives its plans. */
        fun frame(
            t: Long,
            force: Boolean = false,
        ): List<WindowPlan>? {
            val at = frames++
            val changed = rt.tick()
            if (!changed && !force && at > 0) return null
            val planned = rt.drawPlanned(t)
            cache.frame(planned.stack, planned.plans, "$name frame $at")
            val ids = planned.plans.flatMap { p -> p.runs.map { it.id } }.toSet()
            runsEnded += lastRuns.count { it !in ids }
            lastRuns = ids
            // what did not change crosses as the object of the frame before
            val lists = planned.stack.map { it.second }
            if (lists.size == last.size) {
                for (i in lists.indices) {
                    val now = lists[i]
                    val before = last[i]
                    if (now.size != before.size) continue
                    for (k in now.indices) if (now[k] === before[k]) sameObjects++
                }
            }
            last = lists
            return planned.plans
        }
    }

    private companion object {
        val SKINS =
            listOf(
                "WinampModern566.wal",
                "Bento.wal",
                "BigBento.wal",
                "CornerAmp_Redux.wal",
                "MMD3.wal",
                "DeClassified.wal",
                "Winamp3x.wal",
                "Winamp2000SP4.wal",
            )

        /** The skins whose moving parts are kept apart while they play: the three that draw the most. */
        val KEPT_APART = setOf("Bento.wal", "BigBento.wal", "MMD3.wal")
        const val START = 1_792_324_800_000L

        /** How long a live object stays live after its last change, as the engine has it. */
        const val LIVE_HOLD_MS = 10_000L

        /** The phone's size in skin pixels, as a full-screen Bento has it. */
        const val W = 633
        const val H = 1258
        const val FRAME = 16L
        const val SETTLE_FRAMES = 120
        const val PLAY_MS = 40_000L
        const val STEADY_MS = 15_000L
        const val STOP_MS = 2_000L
    }
}
