// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

/**
 * The native methods of `libwalkit.so` (`crates/walkit-android`), one per engine operation. A
 * runtime is a handle made by [rtNew] and dropped by [rtFree]; every other method takes it
 * first. "None" answers are -1 for ids, `Int.MIN_VALUE` for seqs, `Long.MIN_VALUE` for a time,
 * an empty array for a size. A frame is the packed buffer [PackedFrame] reads, planned at the
 * time given; a window's and an object's description a small buffer of ints and strings
 * ([PackedReader]).
 */
internal object Native {
    init {
        HostLibrary.load()
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

    @JvmStatic external fun hfNew(data: ByteArray): Long

    @JvmStatic external fun hfFree(h: Long)

    @JvmStatic external fun hfCovers(
        h: Long,
        text: String,
    ): Boolean

    @JvmStatic external fun hfWidth(
        h: Long,
        text: String,
        em: Float,
    ): Int

    @Suppress("LongParameterList") // the text, its color and the block it goes in
    @JvmStatic
    external fun hfDraw(
        h: Long,
        text: String,
        em: Float,
        argb: Int,
        w: Int,
        hgt: Int,
        x: Int,
        baseline: Int,
    ): IntArray

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

    @JvmStatic
    external fun rtDrawWindows(
        h: Long,
        nowMs: Long,
    ): ByteArray

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

    @JvmStatic
    external fun rtPlaceWindow(
        h: Long,
        id: Int,
        x: Int,
        y: Int,
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

    @JvmStatic external fun rtSkinName(h: Long): String

    @JvmStatic
    external fun rtSetSkinName(
        h: Long,
        name: String,
    )

    @JvmStatic external fun rtPresentation(h: Long): IntArray

    @Suppress("LongParameterList") // one call for every window setting
    @JvmStatic
    external fun rtSetPresentation(
        h: Long,
        hasView: Boolean,
        viewW: Int,
        viewH: Int,
        viewportW: Int,
        viewportH: Int,
        columns: Int,
        columnWidth: Int,
        stretchLast: Boolean,
        touchSlop: Int,
        freeWindows: Boolean,
        touch: Boolean,
        skinListFont: Boolean,
        listRowExtra: Int,
    )

    @JvmStatic
    external fun rtContentRight(
        h: Long,
        id: Int,
        layOutFirst: Boolean,
    ): Int

    @JvmStatic external fun rtListWindowOpen(h: Long): Boolean

    @JvmStatic
    external fun rtShownWidth(
        h: Long,
        width: Int,
    ): Int

    @JvmStatic
    external fun rtNaturalStackSize(
        h: Long,
        columns: Int,
        columnWidth: Int,
    ): IntArray

    @JvmStatic external fun rtLeftColumn(h: Long): IntArray

    @JvmStatic external fun rtVisRect(h: Long): IntArray

    @JvmStatic external fun rtVisualizerWindowClosed(h: Long)

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

    // ---- what a test does as a script would (the test fixtures) ----------------------------
    // A handle is a kind (0 object, 1 container, 2 System, 3 made) and an index, -1 for none; a
    // value is packed as a tag (0 null, 1 int, 2 string, 3 handle) and what follows it.

    @JvmStatic
    external fun rtScriptObject(
        h: Long,
        id: String,
    ): Int

    @JvmStatic
    external fun rtScriptLayout(
        h: Long,
        window: Int,
    ): Int

    @JvmStatic
    external fun rtScriptContainer(
        h: Long,
        id: String,
    ): Int

    @JvmStatic
    external fun rtScriptMake(
        h: Long,
        className: String,
    ): Int

    @JvmStatic
    external fun rtScriptConfigItem(
        h: Long,
        name: String,
        guid: String,
    ): Int

    @JvmStatic
    external fun rtScriptCall(
        h: Long,
        kind: Int,
        index: Int,
        method: String,
        args: ByteArray,
    ): ByteArray?

    @JvmStatic
    external fun rtScriptBeginSlide(
        h: Long,
        index: Int,
    )

    @JvmStatic external fun rtScriptEndSlides(h: Long)

    @JvmStatic external fun rtScriptNextTimerAt(h: Long): Long

    @JvmStatic
    external fun rtScriptRecordEvents(
        h: Long,
        on: Boolean,
    )

    @JvmStatic external fun rtScriptTakeEvents(h: Long): ByteArray
}
