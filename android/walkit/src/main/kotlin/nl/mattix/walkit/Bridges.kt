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

    /**
     * The menu the runtime is asking the listener's pick from through [SkinRuntime.menuChoice],
     * while that call runs: the host gets that same object back, as a host that waits on a menu
     * by identity expects, rather than a copy read from the packed menu.
     */
    var offered: SkinMenu? = null

    /** `Int.MIN_VALUE` for null. */
    fun chooseFromMenu(packed: ByteArray): Int {
        val decoded = PackedMenu.decode(packed)
        val menu = offered?.takeIf { it.sameAs(decoded) } ?: decoded
        return host.chooseFromMenu(menu) ?: Int.MIN_VALUE
    }

    /** Whether two menus list the same items. */
    private fun SkinMenu.sameAs(o: SkinMenu): Boolean =
        items.size == o.items.size &&
            items.zip(o.items).all { (a, b) ->
                when (a) {
                    is SkinMenu.Command -> {
                        b is SkinMenu.Command && a.id == b.id && a.text == b.text && a.checked == b.checked &&
                            a.disabled == b.disabled
                    }

                    SkinMenu.Separator -> {
                        b === SkinMenu.Separator
                    }

                    is SkinMenu.Submenu -> {
                        b is SkinMenu.Submenu && a.text == b.text && a.menu.sameAs(b.menu)
                    }
                }
            }

    fun windowsChanged(ids: IntArray): Unit = host.windowsChanged(ids.map { WindowId(it) })

    /** [from] is [x, y, w, h, px, py], the point Int.MIN_VALUE when no touch sent it; null for none. */
    fun hostAction(
        action: String,
        param: String?,
        from: IntArray?,
    ): Unit =
        host.hostAction(
            action,
            param,
            from?.let {
                ActionFrom(Rect(it[0], it[1], it[2], it[3]), (it[4] to it[5]).takeIf { p -> p.first != Int.MIN_VALUE })
            },
        )
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
     * crossfade, equalizer on, preamp, pan and track index, whether the playlist is numbered and
     * its number's low and high halves, then the band count and the bands.
     */
    fun snapshot(): IntArray {
        val bands = player.eqBands
        val playlist = player.playlistVersion
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
                if (playlist == null) 0 else 1,
                (playlist ?: 0L).toInt(),
                ((playlist ?: 0L) ushr Int.SIZE_BITS).toInt(),
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

    /** The playlist as last read for its titles, so the lengths that follow are of the same list. */
    private var read: List<Track> = emptyList()

    fun trackTitles(): Array<String> =
        player.tracks
            .also { read = it }
            .map { it.title }
            .toTypedArray()

    fun trackLengths(): IntArray = read.map { it.lengthMs }.toIntArray()

    fun trackIndex(): Int = player.trackIndex

    fun playTrack(index: Int): Unit = player.playTrack(index)

    fun spectrum(): FloatArray = player.spectrum()

    fun waveform(): FloatArray = player.waveform()

    fun vu(): IntArray = player.vu().let { (l, r) -> intArrayOf(l, r) }
}
