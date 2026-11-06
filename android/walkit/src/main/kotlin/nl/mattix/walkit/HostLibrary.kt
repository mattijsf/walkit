// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

import java.nio.file.Files

/**
 * Loads the engine's native library: `libwalkit.so` from the app on Android, or, on a plain JVM
 * (a host's unit tests), the build for the machine that the library's test fixtures carry as a
 * resource under `nl/mattix/walkit/host/<os>-<cpu>/`, copied to a temporary file first.
 */
internal object HostLibrary {
    fun load() {
        try {
            System.loadLibrary("walkit")
        } catch (e: UnsatisfiedLinkError) {
            if (!fromResources()) throw e
        }
    }

    /** Whether a build for this machine was found among the resources and loaded. */
    private fun fromResources(): Boolean {
        val os = System.getProperty("os.name").orEmpty().lowercase()
        val (folder, extension) =
            when {
                os.startsWith("mac") -> "darwin" to "dylib"
                os.startsWith("linux") -> "linux" to "so"
                else -> return false
            }
        val cpu =
            when (System.getProperty("os.arch").orEmpty().lowercase()) {
                "aarch64", "arm64" -> "aarch64"
                "amd64", "x86_64" -> "x86_64"
                else -> return false
            }
        val path = "/nl/mattix/walkit/host/$folder-$cpu/libwalkit.$extension"
        val bytes = HostLibrary::class.java.getResourceAsStream(path)?.use { it.readBytes() } ?: return false
        val file = Files.createTempFile("libwalkit", ".$extension").toFile()
        file.deleteOnExit()
        file.writeBytes(bytes)
        System.load(file.absolutePath)
        return true
    }
}
