// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

/**
 * The native methods of `libwalkit.so` (`crates/walkit-android`), one per engine operation. A
 * runtime is a handle made by [rtNew] and dropped by [rtFree]; every other method takes it
 * first. "None" answers are -1 for ids, `Int.MIN_VALUE` for seqs, `Long.MIN_VALUE` for a time,
 * an empty array for a size. A frame is the packed buffer [PackedFrame] reads; a window's and an
 * object's description a small buffer of ints and strings ([PackedReader]).
 */
internal object Native {
    init {
        System.loadLibrary("walkit")
    }

    @Suppress("LongParameterList") // one call for what the runtime is made of
    @JvmStatic
    external fun rtNew(
        files: FilesBridge,
        host: HostBridge,
        player: PlayerBridge,
        skinName: String,
        haveLibrary: Boolean,
        hostActions: Boolean,
        viewportW: Int,
        viewportH: Int,
        startMs: Long,
    ): Long

    @JvmStatic external fun rtFree(h: Long)

    @JvmStatic
    external fun rtSetNow(
        h: Long,
        nowMs: Long,
        zoneOffsetMs: Int,
    )

    @JvmStatic external fun rtLoad(h: Long)

    @JvmStatic
    external fun rtTick(
        h: Long,
        slideLead: Long,
    ): Boolean

    @JvmStatic external fun rtDrawWindows(h: Long): ByteArray

    @JvmStatic external fun rtWindowIds(h: Long): IntArray

    @JvmStatic external fun rtMainWindow(h: Long): Int

    @JvmStatic
    external fun rtWindowInfo(
        h: Long,
        id: Int,
    ): ByteArray

    @JvmStatic
    external fun rtObjectInfo(
        h: Long,
        id: String,
    ): ByteArray?

    @JvmStatic
    external fun rtWindowOf(
        h: Long,
        container: String,
    ): Int

    @JvmStatic
    external fun rtWindowRect(
        h: Long,
        id: Int,
    ): IntArray

    @JvmStatic
    external fun rtOpenWindow(
        h: Long,
        container: String,
    ): Boolean

    @JvmStatic
    external fun rtCloseWindow(
        h: Long,
        container: String,
    ): Boolean

    @JvmStatic
    external fun rtWindowSize(
        h: Long,
        id: Int,
    ): IntArray

    @JvmStatic
    external fun rtResizeWindow(
        h: Long,
        id: Int,
        width: Int,
        height: Int,
    )

    @JvmStatic external fun rtWindowOpen(h: Long): Boolean

    @JvmStatic external fun rtCloseTopWindow(h: Long)

    @JvmStatic
    external fun rtSetViewSize(
        h: Long,
        w: Int,
        height: Int,
    )

    @JvmStatic external fun rtViewSize(h: Long): IntArray

    @JvmStatic
    external fun rtSetViewport(
        h: Long,
        w: Int,
        height: Int,
    )

    @JvmStatic external fun rtViewport(h: Long): IntArray

    @JvmStatic external fun rtEdgePlacements(h: Long): Int

    @JvmStatic external fun rtColorThemes(h: Long): Array<String>

    @JvmStatic external fun rtColorTheme(h: Long): String?

    @JvmStatic
    external fun rtSelectColorTheme(
        h: Long,
        name: String,
    )

    @JvmStatic external fun rtSettingsVersion(h: Long): Int

    @JvmStatic external fun rtConfigVersion(h: Long): Int

    @JvmStatic external fun rtSkinOptions(h: Long): ByteArray

    @JvmStatic
    external fun rtToggleSkinOption(
        h: Long,
        item: String,
        attribute: String,
    ): Boolean

    @JvmStatic external fun rtSavedSettings(h: Long): ByteArray

    @JvmStatic
    external fun rtRestoreSettings(
        h: Long,
        saved: ByteArray,
    )

    @JvmStatic external fun rtStats(h: Long): LongArray

    @JvmStatic external fun rtEnsureLayout(h: Long)

    @JvmStatic external fun rtStackSize(h: Long): IntArray

    @JvmStatic external fun rtDrawnSize(h: Long): IntArray

    @JvmStatic external fun rtSliding(h: Long): Boolean

    @JvmStatic external fun rtAnimating(h: Long): Boolean

    @JvmStatic external fun rtFramesWanted(h: Long): Boolean

    @JvmStatic external fun rtNextDueAt(h: Long): Long

    @JvmStatic external fun rtInvalidate(h: Long)

    @JvmStatic
    external fun rtPointerDown(
        h: Long,
        x: Int,
        y: Int,
        seq: Int,
    )

    @JvmStatic
    external fun rtPointerMove(
        h: Long,
        x: Int,
        y: Int,
    )

    @JvmStatic
    external fun rtPointerUp(
        h: Long,
        x: Int,
        y: Int,
    )

    @JvmStatic
    external fun rtPointerDownIn(
        h: Long,
        window: Int,
        x: Int,
        y: Int,
        seq: Int,
    )

    @JvmStatic
    external fun rtPointerMoveIn(
        h: Long,
        window: Int,
        x: Int,
        y: Int,
    )

    @JvmStatic
    external fun rtPointerUpIn(
        h: Long,
        window: Int,
        x: Int,
        y: Int,
    )

    @JvmStatic
    external fun rtDoubleTap(
        h: Long,
        x: Int,
        y: Int,
        seq: Int,
    )

    @JvmStatic
    external fun rtDoubleTapIn(
        h: Long,
        window: Int,
        x: Int,
        y: Int,
        seq: Int,
    )

    @JvmStatic external fun rtMenuTarget(h: Long): Int

    @JvmStatic external fun rtMoveTarget(h: Long): Int

    @JvmStatic
    external fun rtRightClick(
        h: Long,
        seq: Int,
    )

    @JvmStatic external fun rtPointerCancel(h: Long)

    @JvmStatic
    external fun rtGestureBegin(
        h: Long,
        fresh: Boolean,
    )

    @JvmStatic external fun rtGestureEnd(h: Long)

    @JvmStatic
    external fun rtMenuChoice(
        h: Long,
        menu: ByteArray,
    ): Int
}
