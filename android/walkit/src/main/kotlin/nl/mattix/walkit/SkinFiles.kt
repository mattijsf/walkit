// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

import java.io.ByteArrayOutputStream
import java.io.Closeable
import java.io.File
import java.util.zip.CRC32
import java.util.zip.Deflater
import java.util.zip.ZipFile

/**
 * The files of one skin, looked up by path without regard to case or slash direction. The engine
 * reads through this from one thread, one read at a time; a stack of files (the skin, its
 * siblings, the default set) is composed here and copied nowhere.
 */
public interface SkinFiles {
    public fun read(path: String): ByteArray?

    public fun exists(path: String): Boolean = read(path) != null

    public fun names(): List<String>

    public companion object {
        public fun normalize(path: String): String =
            path
                .replace(
                    '\\',
                    '/',
                ).trimStart('/')
                .lowercase()
                .split('/')
                .filter { it.isNotEmpty() && it != "." }
                .joinToString("/")
    }
}

/** A `.wal`, which is a zip. The whole archive is read into memory once; skins are a few megabytes. */
public class ZipSkinFiles(
    file: File,
) : SkinFiles {
    private val entries: Map<String, ByteArray> =
        ZipFile(file).use { zip ->
            // sizes the archive claims can lie (a zip bomb): count what actually comes out
            var total = 0L
            val all = LinkedHashMap<String, ByteArray>()
            for (e in zip
                .entries()
                .asSequence()
                .filter { !it.isDirectory }
                .take(MAX_ENTRIES)) {
                val bytes = zip.getInputStream(e).use { readAtMost(it, MAX_ENTRY) } ?: continue
                total += bytes.size
                if (total > MAX_TOTAL) break
                all[SkinFiles.normalize(e.name)] = bytes
            }
            // some archives hold the skin in one folder: strip it
            val prefix = WalArchive.folderOf(all.keys)
            if (prefix.isNullOrEmpty()) {
                all
            } else {
                all.filterKeys { it.startsWith(prefix) }.mapKeys { it.key.removePrefix(prefix) }
            }
        }

    override fun read(path: String): ByteArray? = entries[SkinFiles.normalize(path)]

    override fun names(): List<String> = entries.keys.toList()

    /**
     * A `.wal` read entry by entry as paths ask for it, for a skin another skin reads a few files
     * from (Bento reads Big Bento's scripts and XML): the list of entries is read as it opens, and
     * only the bytes of the entries read are kept, within the same limits as a whole archive. The
     * archive stays open until [close], so a file deleted meanwhile reads on as it was; after it,
     * an entry not read before reads as missing. Two threads may read at once.
     */
    public class OnDemand(
        file: File,
    ) : SkinFiles,
        Closeable {
        private val zip = ZipFile(file)

        /** Entry names by path, as [ZipSkinFiles] would name them. */
        private val index: Map<String, String> = indexOf(zip)

        /** The entries read so far, null for one that did not read; guarded by itself. */
        private val kept = HashMap<String, ByteArray?>()

        /** The bytes [kept] holds. */
        public var heldBytes: Long = 0L
            private set

        /** How many reads went to the archive. */
        public var archiveReads: Int = 0
            private set

        override fun read(path: String): ByteArray? {
            val n = SkinFiles.normalize(path)
            synchronized(kept) { if (n in kept) return kept[n] }
            val name = index[n] ?: return null
            // read outside the lock, so one thread does not wait on another's entry
            val bytes =
                runCatching {
                    zip.getEntry(name)?.let { e -> zip.getInputStream(e).use { readAtMost(it, MAX_ENTRY) } }
                }.getOrNull()
            return synchronized(kept) {
                archiveReads++
                if (n !in kept) {
                    val held = bytes?.takeIf { heldBytes + it.size <= MAX_TOTAL }
                    kept[n] = held
                    heldBytes += held?.size ?: 0
                }
                kept[n]
            }
        }

        override fun names(): List<String> = index.keys.toList()

        override fun close(): Unit = zip.close()

        private companion object {
            /** [zip]'s entry names by path, as [ZipSkinFiles] would name them; [zip] is closed when they do not read. */
            fun indexOf(zip: ZipFile): Map<String, String> {
                var index: Map<String, String>? = null
                try {
                    val all = LinkedHashMap<String, String>()
                    for (e in zip
                        .entries()
                        .asSequence()
                        .filter { !it.isDirectory }
                        .take(MAX_ENTRIES)) {
                        all[SkinFiles.normalize(e.name)] = e.name
                    }
                    val prefix = WalArchive.folderOf(all.keys)
                    index =
                        if (prefix.isNullOrEmpty()) {
                            all
                        } else {
                            all.filterKeys { it.startsWith(prefix) }.mapKeys { it.key.removePrefix(prefix) }
                        }
                    return index
                } finally {
                    if (index == null) zip.close()
                }
            }
        }
    }

    private companion object {
        const val MAX_ENTRY = 32 * 1024 * 1024
        const val MAX_TOTAL = 128L * 1024 * 1024
        const val MAX_ENTRIES = 8_192

        /** The stream's bytes, or null when there are more than [limit]. */
        fun readAtMost(
            input: java.io.InputStream,
            limit: Int,
        ): ByteArray? {
            val out = ByteArrayOutputStream()
            val buf = ByteArray(64 * 1024)
            while (true) {
                val n = input.read(buf)
                if (n < 0) return out.toByteArray()
                if (out.size() + n > limit) return null
                out.write(buf, 0, n)
            }
        }
    }
}

/**
 * A skin's files with Winamp's default element set behind them: paths under `wasabi-base/` read
 * from [base] (the `Plugins\freeform\xml\wasabi` folder, zipped), everything else from [skin].
 * Winamp draws lists, scrollbars and frames a skin does not define from that set.
 */
public class LayeredSkinFiles(
    private val skin: SkinFiles,
    private val base: SkinFiles,
) : SkinFiles {
    override fun read(path: String): ByteArray? {
        val n = SkinFiles.normalize(path)
        return if (n.startsWith(BASE_PREFIX)) base.read(n.removePrefix(BASE_PREFIX)) else skin.read(path)
    }

    override fun names(): List<String> = skin.names() + base.names().map { BASE_PREFIX + it }

    internal companion object {
        internal const val BASE_PREFIX = "wasabi-base/"
    }
}

/**
 * A skin beside the other skins in a Skins folder: a path `../<name>/...` or `@SKINSPATH@/<name>/...`
 * reads from that sibling (Bento takes its scripts and most of its XML from "Big Bento").
 * Siblings are named by their folder name lowercased without spaces ("bigbento"); [siblings] gives
 * the one by that name, or null, so a host can open a sibling only when a path names it.
 */
public class SkinsFolderFiles(
    private val skin: SkinFiles,
    private val siblings: (String) -> SkinFiles?,
) : SkinFiles {
    /** Siblings opened beforehand, by folder name. */
    public constructor(skin: SkinFiles, siblings: Map<String, SkinFiles>) : this(skin, siblings::get)

    private fun sibling(path: String): Pair<SkinFiles, String>? {
        val n = SkinFiles.normalize(path).removePrefix("@skinspath@/")
        val parts = n.split('/')
        val start =
            when {
                parts.firstOrNull() == ".." -> 1
                n != SkinFiles.normalize(path) -> 0
                else -> return null
            }
        val name = parts.getOrNull(start)?.replace(" ", "") ?: return null
        return siblings(name)?.let { files -> files to parts.drop(start + 1).joinToString("/") }
    }

    override fun read(path: String): ByteArray? = sibling(path)?.let { (f, p) -> f.read(p) } ?: skin.read(path)

    override fun names(): List<String> = skin.names()
}

/**
 * How a zip is told to be a modern skin: it holds a skin.xml. Classic skins (.wsz) never do.
 * The answer needs only the names in the zip's directory, so nothing is unpacked to give it.
 */
public object WalArchive {
    /**
     * The folder the skin sits in, as a normalized prefix ending in "/", or "" when skin.xml sits
     * at the root. Null when the zip holds no skin.xml, so it is not a modern skin. Some archives
     * hold the whole skin in one folder; the first skin.xml found decides which.
     */
    public fun skinFolder(names: Iterable<String>): String? {
        val normalized = names.map { SkinFiles.normalize(it) }
        if ("skin.xml" in normalized) return ""
        return normalized.firstOrNull { it.endsWith("/skin.xml") }?.removeSuffix("skin.xml")
    }

    /**
     * The folder a skin of either kind sits in, as [skinFolder] gives it for a modern skin; for a
     * classic skin, the folder that holds main.bmp ("" at the root). Null when there is neither.
     * The engine strips the same folder (`files::skin_folder`).
     */
    public fun folderOf(names: Iterable<String>): String? {
        skinFolder(names)?.let { return it }
        val normalized = names.map { SkinFiles.normalize(it) }
        if ("main.bmp" in normalized) return ""
        return normalized.firstOrNull { it.endsWith("/main.bmp") }?.removeSuffix("main.bmp")
    }

    /** Whether a zip with these entry names is a modern skin. */
    public fun isModern(names: Iterable<String>): Boolean = skinFolder(names) != null
}

/**
 * Whether a picture's header asks for more pixels than a skin picture can need, read before
 * decoding: a file that claims 50000 x 50000 pixels would take gigabytes. PNG, BMP, GIF and JPEG
 * headers are read; anything else passes and is left to the decoder.
 */
public object ImageLimits {
    internal const val MAX_SIDE = 8_192
    public const val MAX_PIXELS: Long = 16L * 1024 * 1024

    public fun tooLarge(bytes: ByteArray): Boolean {
        val (w, h) = size(bytes) ?: return false
        return w <= 0 || h <= 0 || w > MAX_SIDE || h > MAX_SIDE || w.toLong() * h > MAX_PIXELS
    }

    /** Width and height from the header, or null when the format is not one of the four. */
    internal fun size(b: ByteArray): Pair<Int, Int>? =
        when {
            b.size >= 24 && b.u8(0) == 0x89 && b.u8(1) == 'P'.code && b.u8(2) == 'N'.code && b.u8(3) == 'G'.code -> {
                b.be32(16) to b.be32(20)
            }

            b.size >= 26 && b.u8(0) == 'B'.code && b.u8(1) == 'M'.code -> {
                b.le32(18) to kotlin.math.abs(b.le32(22))
            }

            b.size >= 10 && b.u8(0) == 'G'.code && b.u8(1) == 'I'.code && b.u8(2) == 'F'.code -> {
                b.le16(6) to b.le16(8)
            }

            b.size >= 4 && b.u8(0) == 0xff && b.u8(1) == 0xd8 -> {
                jpegSize(b)
            }

            else -> {
                null
            }
        }

    /** The byte at [i] as 0..255, or -1 past the end. */
    private fun ByteArray.u8(i: Int) = if (i < size) this[i].toInt() and 0xff else -1

    private fun ByteArray.be16(i: Int) = (u8(i) shl 8) or u8(i + 1)

    private fun ByteArray.be32(i: Int) = (be16(i) shl 16) or be16(i + 2)

    private fun ByteArray.le16(i: Int) = u8(i) or (u8(i + 1) shl 8)

    private fun ByteArray.le32(i: Int) = le16(i) or (le16(i + 2) shl 16)

    /** A JPEG's size from its first start-of-frame marker; null at the first byte that is not a marker. */
    private fun jpegSize(b: ByteArray): Pair<Int, Int>? {
        var i = 2
        while (i + 9 < b.size && (b[i].toInt() and 0xff) == 0xff) {
            val marker = b[i + 1].toInt() and 0xff
            val len = ((b[i + 2].toInt() and 0xff) shl 8) or (b[i + 3].toInt() and 0xff)
            if (isStartOfFrame(marker)) {
                val h = ((b[i + 5].toInt() and 0xff) shl 8) or (b[i + 6].toInt() and 0xff)
                val w = ((b[i + 7].toInt() and 0xff) shl 8) or (b[i + 8].toInt() and 0xff)
                return w to h
            }
            if (len < 2) return null
            i += 2 + len
        }
        return null
    }

    /** SOF0 to SOF15, less DHT (C4), JPG (C8) and DAC (CC), which share the range. */
    private fun isStartOfFrame(marker: Int) = marker in 0xc0..0xcf && marker != 0xc4 && marker != 0xc8 && marker != 0xcc
}

/**
 * Pictures Winamp makes instead of reading a file. `<bitmap file="$solid" w h color alpha>` is a
 * w x h picture of one color (DeClassified's playlist middle and frame texture). The engine gives
 * the bitmap element a made-up file name that holds everything needed
 * (`$solid/<rrggbb><aa>/<w>x<h>.png`), and [read] answers it with PNG bytes, so every renderer
 * loads it like any skin file.
 */
internal object GeneratedImages {
    private const val SOLID = "\$solid/"

    /** PNG bytes for a made-up name, or null for a real file. */
    internal fun read(path: String): ByteArray? {
        if (!path.startsWith(SOLID)) return null
        val (spec, size) =
            path
                .removePrefix(SOLID)
                .removeSuffix(".png")
                .split('/')
                .takeIf { it.size == 2 } ?: return null
        val argb = spec.toLongOrNull(16)?.toInt() ?: return null
        val (w, h) = size.split('x').map { (it.toIntOrNull() ?: 1).coerceIn(1, ImageLimits.MAX_SIDE) }
        if (w.toLong() * h > ImageLimits.MAX_PIXELS) return null
        val rgb = argb ushr 8
        val a = argb and 0xff
        return png(w, h, (a shl 24) or rgb)
    }

    private fun png(
        w: Int,
        h: Int,
        argb: Int,
    ): ByteArray {
        val raw = ByteArray((w * 4 + 1) * h)
        for (y in 0 until h) {
            var o = y * (w * 4 + 1) + 1
            repeat(w) {
                raw[o++] = (argb ushr 16).toByte()
                raw[o++] = (argb ushr 8).toByte()
                raw[o++] = argb.toByte()
                raw[o++] = (argb ushr 24).toByte()
            }
        }
        val out = ByteArrayOutputStream()
        out.write(byteArrayOf(0x89.toByte(), 'P'.code.toByte(), 'N'.code.toByte(), 'G'.code.toByte(), 13, 10, 26, 10))

        fun int(v: Int) = byteArrayOf((v ushr 24).toByte(), (v ushr 16).toByte(), (v ushr 8).toByte(), v.toByte())

        fun chunk(
            type: String,
            data: ByteArray,
        ) {
            out.write(int(data.size))
            val td = type.toByteArray() + data
            out.write(td)
            out.write(int(CRC32().apply { update(td) }.value.toInt()))
        }
        chunk("IHDR", int(w) + int(h) + byteArrayOf(8, 6, 0, 0, 0))
        val d = Deflater()
        d.setInput(raw)
        d.finish()
        val buf = ByteArray(raw.size + 64)
        val z = ByteArrayOutputStream()
        while (!d.finished()) z.write(buf, 0, d.deflate(buf))
        d.end()
        chunk("IDAT", z.toByteArray())
        chunk("IEND", ByteArray(0))
        return out.toByteArray()
    }
}

/** [files] that also answer [GeneratedImages]' made-up names. */
public class WithGeneratedImages(
    private val files: SkinFiles,
) : SkinFiles by files {
    override fun read(path: String): ByteArray? = GeneratedImages.read(path) ?: files.read(path)

    override fun exists(path: String): Boolean = GeneratedImages.read(path) != null || files.exists(path)
}
