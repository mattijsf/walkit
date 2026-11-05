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
 * The planner over the binding on the corpus skins pushed to `/data/local/tmp/walkit/skins` (see
 * [CorpusTraceTest]; skipped without them), playing on a stepped clock, then stopped, left resting
 * past the live hold and played again: it draws exactly what [SkinRuntime.drawWindows] draws, every
 * plan holds its promises, and a renderer keeping windows and runs by version never shows a stale
 * picture. Once a skin has played a while no window is recorded again; the skins that draw the most
 * keep their moving parts apart, and those runs end once their objects rest.
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
        val (twin, twinPlayer) = runtime(name) { t }
        val run = Run(name, rt, twin)
        repeat(SETTLE_FRAMES) {
            t += FRAME
            rt.tick()
            twin.tick()
        }
        player.play()
        twinPlayer.play()
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
        twinPlayer.stop()
        val stopped = t
        while (t < stopped + STOP_MS) {
            t += FRAME
            run.frame(t)
        }
        // nothing drawn for longer than the live hold, then a frame, then play again
        t += DrawPlanner.LIVE_HOLD_MS + FRAME
        run.frame(t, force = true)
        player.play()
        twinPlayer.play()
        val again = t
        while (t < again + STOP_MS) {
            t += FRAME
            run.frame(t)
        }
        if (name in KEPT_APART) assertTrue("$name: runs end once their objects rest", run.runsEnded > 0)
        rt.close()
        twin.close()
    }

    /** A skin's runtime planned frame by frame beside a [twin] that draws as drawWindows draws, checked each frame. */
    private inner class Run(
        val name: String,
        val rt: SkinRuntime,
        val twin: SkinRuntime,
    ) {
        val planner = DrawPlanner()
        val cache = PlanCache()
        var runsEnded = 0
        private var frames = 0
        private var lastRuns = emptySet<Long>()

        /** Ticks both runtimes at [t]; when they changed, or with [force], plans and checks a frame and gives its plans. */
        fun frame(
            t: Long,
            force: Boolean = false,
        ): List<WindowPlan>? {
            val at = frames++
            val changed = rt.tick()
            assertEquals("$name frame $at: both runtimes tick alike", changed, twin.tick())
            if (!changed && !force && at > 0) return null
            val planned = planner.draw(rt, t)
            val stack = twin.drawWindows()
            assertEquals("$name frame $at: windows", stack.size, planned.stack.size)
            for (i in stack.indices) {
                val (r, cmds) = stack[i]
                val (pr, pcmds) = planned.stack[i]
                assertEquals("$name frame $at window $i", r, pr)
                assertEquals("$name frame $at window $i: commands", cmds.size, pcmds.size)
                for (k in cmds.indices) assertTrue("$name frame $at window $i command $k", alike(cmds[k], pcmds[k]))
            }
            cache.frame(planned.stack, planned.plans, "$name frame $at")
            val ids = planned.plans.flatMap { p -> p.runs.map { it.id } }.toSet()
            runsEnded += lastRuns.count { it !in ids }
            lastRuns = ids
            return planned.plans
        }
    }

    /** The same drawing from two runtimes: a window's shape is another object in each, so it is compared by what it holds. */
    private fun alike(
        a: DrawCmd,
        b: DrawCmd,
    ): Boolean {
        if (a !is DrawCmd.WindowStart || b !is DrawCmd.WindowStart) return a.drawsSameAs(b)
        val pa = a.region.pics()
        val pb = b.region.pics()
        return a.region.bounds() == b.region.bounds() && pa.size == pb.size && pa.indices.all { pa[it].drawsSameAs(pb[it]) }
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
