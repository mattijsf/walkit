// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

/** One playlist entry. */
public data class Track(
    val title: String,
    val lengthMs: Int,
)

/** The player as a modern skin sees it. Positions and lengths are milliseconds; volume is 0..255. */
public interface Player {
    /** What the transport is doing. */
    public enum class Status { STOPPED, PLAYING, PAUSED }

    public val status: Status
    public val positionMs: Int
    public val lengthMs: Int
    public val title: String
    public val bitrateKbps: Int
    public val sampleRateKhz: Int
    public val channels: Int

    /** The current file's tags by lowercase field name (artist, album, ...); a file without tags has none. */
    public val tags: Map<String, String> get() = emptyMap()

    /** The input plug-in's name for the current file, as Winamp's `getDecoderName` reports it; empty when unknown. */
    public val decoderName: String get() = ""
    public var volume: Int
    public var shuffle: Boolean
    public var repeat: Boolean
    public var crossfade: Boolean
    public var eqEnabled: Boolean
    public val eqBands: IntArray
    public var eqPreamp: Int

    /** The balance: -127 hard left, 0 the middle, 127 hard right. */
    public var pan: Int

    public fun play()

    public fun pause()

    public fun stop()

    public fun next()

    public fun previous()

    public fun eject()

    public fun seekTo(ms: Int)

    public fun setEqBand(
        band: Int,
        value: Int,
    )

    /** The playlist, and which entry is current; a player without one has just the current song. */
    public val tracks: List<Track> get() = listOf(Track(title, lengthMs))
    public val trackIndex: Int get() = 0

    /** Makes entry [index] current and plays it. */
    public fun playTrack(index: Int): Unit = play()

    /** Spectrum levels 0..1, 75 bands like Winamp's vis, or empty when nothing plays. */
    public fun spectrum(): FloatArray

    /**
     * The waveform being heard: samples -1..1, oldest first, 576 of them as Winamp hands a
     * visualizer, or empty when nothing plays.
     */
    public fun waveform(): FloatArray = FloatArray(0)

    /** Left and right VU 0..255. */
    public fun vu(): Pair<Int, Int>
}

/** A player that plays nothing, for tests and for a first look at a skin. */
public class FakePlayer : Player {
    /** The clock positions, the demo spectrum and the demo wave follow; a test gives it the runtime's. */
    public var clock: () -> Long = System::currentTimeMillis

    override var status: Player.Status = Player.Status.STOPPED
    private var startedAt = 0L
    private var pausedAt = 0
    override val positionMs: Int
        get() =
            when (status) {
                Player.Status.PLAYING -> ((clock() - startedAt).toInt() + pausedAt).coerceAtMost(lengthMs)
                Player.Status.PAUSED -> pausedAt
                Player.Status.STOPPED -> 0
            }

    /** Set to show several entries; empty means the playlist is the current song alone. */
    public var playlist: List<Track> = emptyList()
    private var index = 0
    override val tracks: List<Track> get() = playlist.ifEmpty { listOf(Track(title, lengthMs)) }
    override val trackIndex: Int get() = index.coerceIn(0, tracks.size - 1)

    override fun playTrack(index: Int) {
        val t = tracks.getOrNull(index) ?: return
        this.index = index
        title = t.title
        lengthMs = t.lengthMs
        status = Player.Status.STOPPED
        play()
    }

    override var lengthMs: Int = 215_000
    override var title: String = "Andamp - Modern skin demo"
    override var decoderName: String = ""
    override var bitrateKbps: Int = 192
    override var sampleRateKhz: Int = 44
    override var channels: Int = 2
    override var volume: Int = 200
    override var shuffle: Boolean = false
    override var repeat: Boolean = false
    override var crossfade: Boolean = false
    override var eqEnabled: Boolean = false
    override val eqBands: IntArray = IntArray(10) { 128 }
    override var eqPreamp: Int = 128
    override var pan: Int = 0

    override fun play() {
        if (status == Player.Status.PLAYING) return
        if (status == Player.Status.STOPPED) pausedAt = 0
        startedAt = clock()
        status = Player.Status.PLAYING
    }

    override fun pause() {
        when (status) {
            Player.Status.PLAYING -> {
                pausedAt = positionMs
                status = Player.Status.PAUSED
            }

            Player.Status.PAUSED -> {
                play()
            }

            Player.Status.STOPPED -> {
                Unit
            }
        }
    }

    override fun stop() {
        status = Player.Status.STOPPED
        pausedAt = 0
    }

    override fun next() {
        seekTo(0)
    }

    override fun previous() {
        seekTo(0)
    }

    override fun eject(): Unit = Unit

    override fun seekTo(ms: Int) {
        pausedAt = ms.coerceIn(0, lengthMs)
        startedAt = clock()
    }

    override fun setEqBand(
        band: Int,
        value: Int,
    ) {
        if (band in eqBands.indices) eqBands[band] = value
    }

    override fun spectrum(): FloatArray {
        if (status != Player.Status.PLAYING) return FloatArray(0)
        val t = clock() / 60.0
        return FloatArray(75) { i -> (0.5 + 0.5 * Math.sin(t + i * 0.3)).toFloat() * (1f - i / 90f) }
    }

    /** Two tones drifting with the clock, a slow one and one three times as fast, while playing. */
    override fun waveform(): FloatArray {
        if (status != Player.Status.PLAYING) return FloatArray(0)
        val t = clock() / 60.0
        return FloatArray(576) { i -> (0.35 * Math.sin(t + i * 0.05) + 0.15 * Math.sin(3 * t + i * 0.27)).toFloat() }
    }

    override fun vu(): Pair<Int, Int> = if (status == Player.Status.PLAYING) 160 to 150 else 0 to 0
}
