// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

/**
 * What the engine calls back into, one object per side of the host: the methods the native
 * library looks up by name and signature, taking and giving only primitives, strings and
 * arrays. A method that throws ends the engine call that led to it with a `RuntimeException`
 * naming the throwable; the engine itself hears the method's default.
 */
internal class FilesBridge(
    private val files: SkinFiles,
) {
    fun read(path: String): ByteArray? = files.read(path)

    fun names(): Array<String> = files.names().toTypedArray()
}

/** [SkinHost] as the engine asks it; the stand-in fonts come from the skin's [files]. */
internal class HostBridge(
    private val host: SkinHost,
    private val files: SkinFiles,
) {
    private fun font(
        id: String?,
        file: String?,
    ): FontDef? = if (id == null) null else FontDef(id, file ?: "")

    fun imageSize(file: String): IntArray? = host.imageSize(file)?.let { (w, h) -> intArrayOf(w, h) }

    fun pixels(
        file: String,
        x: Int,
        y: Int,
        w: Int,
        h: Int,
    ): IntArray? = host.pixels(file, x, y, w, h)

    fun textWidth(
        text: String,
        fontId: String?,
        fontFile: String?,
        size: Int,
        bold: Boolean,
    ): Int = host.textWidth(text, font(fontId, fontFile), size, bold)

    fun textAdvance(
        text: String,
        fontId: String?,
        fontFile: String?,
        size: Int,
        bold: Boolean,
    ): Float = host.textAdvance(text, font(fontId, fontFile), size, bold)

    fun lineMetrics(
        fontId: String?,
        fontFile: String?,
        size: Int,
        bold: Boolean,
    ): FloatArray? = host.lineMetrics(font(fontId, fontFile), size, bold)?.let { (a, d) -> floatArrayOf(a, d) }

    /** The bytes of a stand-in font (`fonts/LiberationSans.ttf`), from the skin's files. */
    fun fontFile(file: String): ByteArray? = files.read(file)

    fun log(message: String): Unit = host.log(message)

    fun invalidate(): Unit = host.invalidate()

    /** -1 for null, else 0 or 1. */
    fun windowOpen(kind: String): Int =
        when (host.windowOpen(kind)) {
            null -> -1
            false -> 0
            true -> 1
        }

    /** `Int.MIN_VALUE` for null. */
    fun chooseFromMenu(packed: ByteArray): Int = host.chooseFromMenu(PackedMenu.decode(packed)) ?: Int.MIN_VALUE

    fun windowsChanged(ids: IntArray): Unit = host.windowsChanged(ids.map { WindowId(it) })

    fun hostAction(
        action: String,
        param: String?,
    ): Unit = host.hostAction(action, param)
}

/** [Player] as the engine drives it: the status as 0 stopped, 1 playing, 2 paused. */
internal class PlayerBridge(
    private val player: Player,
) {
    fun status(): Int =
        when (player.status) {
            Player.Status.STOPPED -> 0
            Player.Status.PLAYING -> 1
            Player.Status.PAUSED -> 2
        }

    /**
     * Every number of the player at once, as the native side reads them once per call: the
     * status, position, length, bitrate, sample rate, channels, volume, shuffle, repeat,
     * crossfade, equalizer on, preamp, pan and track index, then the band count and the bands.
     */
    fun snapshot(): IntArray {
        val bands = player.eqBands
        val numbers =
            intArrayOf(
                status(),
                player.positionMs,
                player.lengthMs,
                player.bitrateKbps,
                player.sampleRateKhz,
                player.channels,
                player.volume,
                if (player.shuffle) 1 else 0,
                if (player.repeat) 1 else 0,
                if (player.crossfade) 1 else 0,
                if (player.eqEnabled) 1 else 0,
                player.eqPreamp,
                player.pan,
                player.trackIndex,
                bands.size,
            )
        return numbers + bands
    }

    fun positionMs(): Int = player.positionMs

    fun lengthMs(): Int = player.lengthMs

    fun title(): String = player.title

    fun bitrateKbps(): Int = player.bitrateKbps

    fun sampleRateKhz(): Int = player.sampleRateKhz

    fun channels(): Int = player.channels

    /** Name, value, name, value, ... */
    fun tags(): Array<String> = player.tags.flatMap { (k, v) -> listOf(k, v) }.toTypedArray()

    fun decoderName(): String = player.decoderName

    fun volume(): Int = player.volume

    fun setVolume(v: Int) {
        player.volume = v
    }

    fun shuffle(): Boolean = player.shuffle

    fun setShuffle(on: Boolean) {
        player.shuffle = on
    }

    fun repeat(): Boolean = player.repeat

    fun setRepeat(on: Boolean) {
        player.repeat = on
    }

    fun crossfade(): Boolean = player.crossfade

    fun setCrossfade(on: Boolean) {
        player.crossfade = on
    }

    fun eqEnabled(): Boolean = player.eqEnabled

    fun setEqEnabled(on: Boolean) {
        player.eqEnabled = on
    }

    fun eqBands(): IntArray = player.eqBands

    fun eqPreamp(): Int = player.eqPreamp

    fun setEqPreamp(v: Int) {
        player.eqPreamp = v
    }

    fun pan(): Int = player.pan

    fun setPan(v: Int) {
        player.pan = v
    }

    fun play(): Unit = player.play()

    fun pause(): Unit = player.pause()

    fun stop(): Unit = player.stop()

    fun next(): Unit = player.next()

    fun previous(): Unit = player.previous()

    fun eject(): Unit = player.eject()

    fun seekTo(ms: Int): Unit = player.seekTo(ms)

    fun setEqBand(
        band: Int,
        value: Int,
    ): Unit = player.setEqBand(band, value)

    fun trackTitles(): Array<String> = player.tracks.map { it.title }.toTypedArray()

    fun trackLengths(): IntArray = player.tracks.map { it.lengthMs }.toIntArray()

    fun trackIndex(): Int = player.trackIndex

    fun playTrack(index: Int): Unit = player.playTrack(index)

    fun spectrum(): FloatArray = player.spectrum()

    fun waveform(): FloatArray = player.waveform()

    fun vu(): IntArray = player.vu().let { (l, r) -> intArrayOf(l, r) }
}
