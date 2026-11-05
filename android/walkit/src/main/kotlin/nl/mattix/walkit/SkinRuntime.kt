// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

import java.io.Closeable
import java.util.TimeZone

/**
 * A loaded modern skin: its resources, its containers, its scripts, and the glue between the
 * player and the objects, run by the native engine (`libwalkit.so`). Everything runs on one
 * thread; the app calls [tick] regularly and [drawWindows] when something changed.
 *
 * The engine reads the skin's [files], asks the [host] and drives the [player] from the thread
 * that calls it, through callbacks. The [clock] is read once at the start of each call and
 * handed to the engine, which reads it from there. [close] frees the engine's side; a runtime
 * that is garbage collected unclosed is freed then.
 *
 * What the native engine does not port yet is marked on each member and refuses with
 * [UnsupportedOperationException], or answers empty where the reference engine answers nothing
 * of note: the stack arrangement in columns ([presentation], [contentRight], [shownWidth],
 * [naturalStackSize], [leftColumn], [listWindowOpen]), and the visualizer window ([visRect],
 * [visualizerWindowClosed]).
 */
@Suppress("TooManyFunctions") // the one host every object and script of a skin calls into
public class SkinRuntime(
    files: SkinFiles,
    public val host: SkinHost,
    public val player: Player,
    setup: SkinSetup = SkinSetup(),
    clock: () -> Long = System::currentTimeMillis,
) : Closeable {
    /**
     * The clock every timer, animation and script reads, in ms. The app leaves the wall clock; a
     * test steps its own so animations and timers land on the same frame every run.
     */
    public var clock: () -> Long = clock

    public fun now(): Long = clock()

    // the bridges stay referenced here for as long as the engine holds them
    private val filesBridge = FilesBridge(files)
    private val hostBridge = HostBridge(host, files)
    private val playerBridge = PlayerBridge(player)

    private var handle: Long =
        Native.rtNew(
            filesBridge,
            hostBridge,
            playerBridge,
            setup.skinName,
            setup.haveLibrary,
            setup.hostActions,
            VIEWPORT_W,
            VIEWPORT_H,
            clock(),
        )

    private var loaded = false

    /** The engine's side, with the clock set to now; a closed runtime refuses. */
    private fun h(): Long {
        check(handle != 0L) { "the runtime is closed" }
        val now = clock()
        Native.rtSetNow(handle, now, TimeZone.getDefault().getOffset(now))
        return handle
    }

    /** Frees the engine's side; every call after refuses. */
    override fun close() {
        if (handle != 0L) {
            Native.rtFree(handle)
            handle = 0L
        }
    }

    @Suppress("ProtectedMemberInFinalClass") // the JVM's finalizer hook
    protected fun finalize() {
        close()
    }

    /** What getSkinName() answers: the skin's folder name, set by the host before [load]. */
    public var skinName: String = setup.skinName
        set(value) {
            if (loaded && value != field) throw UnsupportedOperationException("walkit takes the skin name before load")
            field = value
        }

    // ---- what the host keeps between runs ------------------------------------------------------

    /**
     * Counts changes to what a host keeps of the skin between runs: the scripts' private and public
     * values, the config attributes and the color theme.
     */
    public val settingsVersion: Int get() = Native.rtSettingsVersion(h())

    /** What the host keeps of the skin between runs, as it is now; the color theme is [colorTheme]. */
    public fun savedSettings(): SavedSettings {
        val r = PackedReader(Native.rtSavedSettings(h()))
        val attributes = LinkedHashMap<String, String>()
        repeat(r.i32()) {
            val k = r.inlineString()
            attributes[k] = r.inlineString()
        }
        val strings = LinkedHashMap<String, String>()
        repeat(r.i32()) {
            val k = r.inlineString()
            strings[k] = r.inlineString()
        }
        val ints = LinkedHashMap<String, Int>()
        repeat(r.i32()) {
            val k = r.inlineString()
            ints[k] = r.i32()
        }
        return SavedSettings(attributes, strings, ints)
    }

    /**
     * Puts back what [savedSettings] gave in an earlier run, before [load], so the scripts read it
     * as they start. It counts as no change ([settingsVersion]).
     */
    public fun restoreSettings(saved: SavedSettings) {
        val out = PackedOut()
        out.i32(saved.attributes.size)
        for ((k, v) in saved.attributes) {
            out.inlineString(k)
            out.inlineString(v)
        }
        out.i32(saved.strings.size)
        for ((k, v) in saved.strings) {
            out.inlineString(k)
            out.inlineString(v)
        }
        out.i32(saved.ints.size)
        for ((k, v) in saved.ints) {
            out.inlineString(k)
            out.i32(v)
        }
        Native.rtRestoreSettings(h(), out.bytes())
    }

    /**
     * Counts changes to the skin's Config items: an item or attribute made, an attribute made by
     * newAttribute, or a value changed. A host compares it to [SkinOptions.version] to know whether
     * [skinOptions] would read differently.
     */
    public val configVersion: Int get() = Native.rtConfigVersion(h())

    /**
     * The options the skin's scripts list for Winamp's Options menu, read from the Config item
     * Winamp lists there only. An attribute whose value is another item's GUID is a submenu named
     * by the attribute, "-" is a separator, and any other is a toggle. Attributes only read with
     * getAttribute are left out. Reading makes no item. A toggle is a radio button when a toggle
     * button of the skin set to that attribute draws a picture whose id says "radio".
     */
    public fun skinOptions(): SkinOptions {
        val r = PackedReader(Native.rtSkinOptions(h()))
        val version = r.i32()
        return SkinOptions(version, optionEntries(r))
    }

    private fun optionEntries(r: PackedReader): List<SkinOptions.Entry> =
        List(r.i32()) {
            when (val kind = r.i32()) {
                1 -> {
                    val key = SkinOptions.Key(r.inlineString(), r.inlineString())
                    SkinOptions.Toggle(key, r.inlineString(), r.i32() != 0, r.i32() != 0)
                }

                2 -> {
                    SkinOptions.Separator
                }

                3 -> {
                    SkinOptions.Submenu(r.inlineString(), optionEntries(r))
                }

                else -> {
                    throw IllegalStateException("walkit packed options: entry kind $kind")
                }
            }
        }

    /**
     * Flips the option [key] names as a pick from Winamp's menu does: "1" over a value that reads
     * as 0, "0" over any other. It is set with setData, so every script hears onDataChanged and the
     * value is kept with the skin's settings. False for an unknown key, a separator, or a value
     * that is a GUID, a submenu's or not.
     */
    public fun toggleSkinOption(key: SkinOptions.Key): Boolean = Native.rtToggleSkinOption(h(), key.item, key.attribute)

    // ---- the screen --------------------------------------------------------------------------

    /** The screen as scripts ask for it (`getViewportWidth`): the phone's view in layout pixels once known. */
    public val viewport: Pair<Int, Int> get() = Native.rtViewport(h()).let { it[0] to it[1] }

    /**
     * The space the app has for the stack, in layout pixels; null leaves each window at the size it
     * has. A new one puts each window where the phone draws it in the new viewport: against the
     * edges a script moved it to, or in the middle.
     */
    public var viewSize: Pair<Int, Int>?
        get() = Native.rtViewSize(h()).takeIf { it.size == 2 }?.let { it[0] to it[1] }
        set(value) = Native.rtSetViewSize(h(), value?.first ?: 0, value?.second ?: 0)

    private var presentationNow = Presentation()

    /**
     * How the host shows the windows, as one value ([Presentation]). The engine takes
     * [Presentation.viewSize] and [Presentation.viewport]; columns, the column width, stretching
     * the last window and the touch slop are not ported yet, and a value that sets them away from
     * their defaults refuses.
     */
    public var presentation: Presentation
        get() = presentationNow.copy(viewSize = viewSize, viewport = viewport)
        set(value) {
            val defaults = Presentation()
            if (value.columns != defaults.columns ||
                value.columnWidth != defaults.columnWidth ||
                value.stretchLast != defaults.stretchLast ||
                value.touchSlop != defaults.touchSlop
            ) {
                throw UnsupportedOperationException("walkit does not port columns, column width, stretchLast or touchSlop yet")
            }
            presentationNow = value
            Native.rtSetViewport(h(), value.viewport.first, value.viewport.second)
            viewSize = value.viewSize
        }

    // ---- loading and themes --------------------------------------------------------------------

    /** Loads `skin.xml` and everything it includes, builds the windows and starts the scripts. */
    public fun load() {
        Native.rtLoad(h())
        loaded = true
    }

    /** The skin's color themes (gammasets), in the order the skin defines them. */
    public val colorThemes: List<String> get() = Native.rtColorThemes(h()).toList()

    /** The color theme in use. */
    public val colorTheme: String? get() = Native.rtColorTheme(h())

    /** Wears the color theme named [name], matched ignoring case; an unknown name changes nothing. */
    public fun selectColorTheme(name: String): Unit = Native.rtSelectColorTheme(h(), name)

    // ---- windows -------------------------------------------------------------------------------

    /** The windows on screen, top to bottom: the main one first. */
    public val windowIds: List<WindowId> get() = Native.rtWindowIds(h()).map { WindowId(it) }

    /** The main window as it shows now (its normal layout or its shade), or null when the skin has none. */
    public val mainWindow: WindowId? get() = Native.rtMainWindow(h()).takeIf { it >= 0 }?.let { WindowId(it) }

    /** What [id] is like now; see [WindowInfo]. */
    public fun windowInfo(id: WindowId): WindowInfo {
        val r = PackedReader(Native.rtWindowInfo(h(), id.raw))
        val wid = WindowId(r.i32())
        val size = r.i32() to r.i32()
        val minW = r.i32()
        val minH = r.i32()
        val maxW = r.i32()
        val maxH = r.i32()
        val resizable = r.i32() != 0
        val edgeX = edge(r.i32())
        val edgeY = edge(r.i32())
        val container = r.inlineString()
        val layout = r.inlineString()
        return WindowInfo(wid, size, minW, minH, maxW, maxH, resizable, edgeX, edgeY, container, layout)
    }

    private fun edge(v: Int): Edge? =
        when (v) {
            0 -> Edge.START
            1 -> Edge.END
            else -> null
        }

    /**
     * The object with id [id] in the windows on screen, the top one first, as a host or a check
     * finds what to tap; null when none has it. See [ObjectInfo].
     */
    public fun objectInfo(id: String): ObjectInfo? {
        val r = PackedReader(Native.rtObjectInfo(h(), id) ?: return null)
        val window = WindowId(r.i32())
        val rect = r.rect()
        val stackRect = r.rect()
        val visible = r.i32() != 0
        val className = r.inlineString()
        return ObjectInfo(window, rect, stackRect, visible, className)
    }

    /** The window container [container] shows now (its current layout), or null when the skin has no such container. */
    public fun windowOf(container: String): WindowId? = Native.rtWindowOf(h(), container).takeIf { it >= 0 }?.let { WindowId(it) }

    /** Where [id] sits in the stack, in layout pixels. */
    public fun windowRect(id: WindowId): Rect = Native.rtWindowRect(h(), id.raw).let { Rect(it[0], it[1], it[2], it[3]) }

    /** Opens the window of [container] in the stack; false when the skin has no such container. */
    public fun openWindow(container: String): Boolean = Native.rtOpenWindow(h(), container)

    /** Closes the window of [container]; false when the skin has no such container. */
    public fun closeWindow(container: String): Boolean = Native.rtCloseWindow(h(), container)

    /** [id]'s size as it shows now, in layout pixels. */
    public fun windowSize(id: WindowId): Pair<Int, Int> = Native.rtWindowSize(h(), id.raw).let { it[0] to it[1] }

    /** Sizes [id] to [width] by [height] where it is, as the host sizes a window to its box. */
    public fun resizeWindow(
        id: WindowId,
        width: Int,
        height: Int,
    ): Unit = Native.rtResizeWindow(h(), id.raw, width, height)

    /** Where [id] visibly ends on the right. Not ported yet: refuses. */
    public fun contentRight(
        id: WindowId,
        layOutFirst: Boolean = false,
    ): Int =
        throw UnsupportedOperationException("walkit does not port contentRight yet (window ${id.raw}, layOutFirst=$layOutFirst)")

    /** Whether a window docked under the main one shows a list. Not ported yet: refuses. */
    public val listWindowOpen: Boolean get() = throw UnsupportedOperationException("walkit does not port listWindowOpen yet")

    /** How far right the stack shows in one column. Not ported yet: refuses. */
    public fun shownWidth(width: Int = 0): Int =
        throw UnsupportedOperationException("walkit does not port shownWidth yet ($width)")

    /** How large the stack is in columns. Not ported yet: refuses. */
    public fun naturalStackSize(
        columns: Int,
        columnWidth: Int = 0,
    ): Pair<Int, Int>? = throw UnsupportedOperationException("walkit does not port naturalStackSize yet ($columns, $columnWidth)")

    /** The left column of the stack in two columns. Not ported yet: refuses. */
    public fun leftColumn(): Rect? = throw UnsupportedOperationException("walkit does not port leftColumn yet")

    /** Whether a window besides the main one is open, which Back would put away. */
    public val windowOpen: Boolean get() = Native.rtWindowOpen(h())

    /**
     * How many times a script moved the main window against an edge of the viewport after the
     * listener picked from one of its menus in the same touch.
     */
    public val edgePlacements: Int get() = Native.rtEdgePlacements(h())

    /** Where the visualizer shows in the stack. Not ported yet: refuses. */
    public val visRect: Rect? get() = throw UnsupportedOperationException("walkit does not port visRect yet")

    /** The app closed its visualizer window. Not ported yet: refuses. */
    public fun visualizerWindowClosed(): Unit =
        throw UnsupportedOperationException("walkit does not port visualizerWindowClosed yet")

    /** Closes the window opened last, as Back does. */
    public fun closeTopWindow(): Unit = Native.rtCloseTopWindow(h())

    /** The stack's size, in layout pixels. */
    public val stackSize: Pair<Int, Int> get() = Native.rtStackSize(h()).let { it[0] to it[1] }

    /**
     * The part of the stack its windows show, in layout pixels: [stackSize] with each window cut on
     * its right to where it shows and on its bottom to the part Winamp docks and snaps to.
     */
    public val drawnSize: Pair<Int, Int> get() = Native.rtDrawnSize(h()).let { it[0] to it[1] }

    /** The engine asks for a redraw. */
    public fun invalidate(): Unit = Native.rtInvalidate(h())

    /** Lays the main window out now, for a script that asks where something is. */
    public fun ensureLayout(): Unit = Native.rtEnsureLayout(h())

    // ---- per frame -----------------------------------------------------------------------------

    /** Whether a target slide runs now. */
    public val sliding: Boolean get() = Native.rtSliding(h())

    /** Whether something moves by itself right now: a target slide, or an animation playing and not paused. */
    public val animating: Boolean get() = Native.rtAnimating(h())

    /**
     * Whether something on the skin moves smoothly by itself now and wants a frame on every display
     * frame: a target slide, a visualizer while the music plays, an animation playing.
     */
    public val framesWanted: Boolean get() = Native.rtFramesWanted(h())

    /**
     * When, in [clock] milliseconds, something on the skin next changes by itself: a running timer
     * firing, an animation's next frame, a target slide, a ticker's next step. Null when nothing
     * will. A time due already comes back as it is.
     */
    public fun nextDueAt(): Long? = Native.rtNextDueAt(h()).takeIf { it != Long.MIN_VALUE }

    /** What the engine has done so far, for a host's timing log or probe; see [EngineStats]. */
    public val stats: EngineStats get() = Native.rtStats(h()).let { EngineStats(it[0], it[1], it[2], it[3]) }

    /** [tick] with the next target slide placed [slideLead] ms ahead of now. */
    public fun tick(slideLead: Long): Boolean = Native.rtTick(h(), slideLead.coerceAtLeast(0))

    /**
     * Advances timers, tickers and player-driven objects. Returns true when a redraw is due: when
     * something that shows changed, a visualizer that moves only once its 25 ms step has passed.
     */
    public fun tick(): Boolean = Native.rtTick(h(), -1)

    /** Every window's drawing with its place in the stack; draw each translated to its rectangle. */
    public fun drawWindows(): List<Pair<Rect, List<DrawCmd>>> =
        PackedFrame.decode(Native.rtDrawWindows(h())).map {
            it.rect to
                it.cmds
        }

    // ---- touches -------------------------------------------------------------------------------

    /**
     * Pointer events take points of the stack (see [windowRect]); objects hear points of their
     * window. [seq] names this touch for [menuTarget], [moveTarget] and [rightClick].
     */
    public fun pointerDown(
        x: Int,
        y: Int,
        seq: Int = 0,
    ): Unit = Native.rtPointerDown(h(), x, y, seq)

    /** [pointerDown] at a point of [window], in its own pixels. */
    public fun pointerDown(
        window: WindowId,
        x: Int,
        y: Int,
        seq: Int = 0,
    ): Unit = Native.rtPointerDownIn(h(), window.raw, x, y, seq)

    public fun pointerMove(
        x: Int,
        y: Int,
    ): Unit = Native.rtPointerMove(h(), x, y)

    /** [pointerMove] at a point of [window], in its own pixels. */
    public fun pointerMove(
        window: WindowId,
        x: Int,
        y: Int,
    ): Unit = Native.rtPointerMoveIn(h(), window.raw, x, y)

    public fun pointerUp(
        x: Int,
        y: Int,
    ): Unit = Native.rtPointerUp(h(), x, y)

    /** [pointerUp] at a point of [window], in its own pixels. */
    public fun pointerUp(
        window: WindowId,
        x: Int,
        y: Int,
    ): Unit = Native.rtPointerUpIn(h(), window.raw, x, y)

    /** A second tap of a double tap, at a point of the stack; [seq] names it as [pointerDown]'s does. */
    public fun doubleTap(
        x: Int,
        y: Int,
        seq: Int,
    ): Unit = Native.rtDoubleTap(h(), x, y, seq)

    /** [doubleTap] at a point of [window], in its own pixels. */
    public fun doubleTap(
        window: WindowId,
        x: Int,
        y: Int,
        seq: Int,
    ): Unit = Native.rtDoubleTapIn(h(), window.raw, x, y, seq)

    /**
     * The seq of the last [pointerDown] or [doubleTap] when it landed on an object with a skin menu,
     * else null: [rightClick] with that seq opens the menu.
     */
    public val menuTarget: Int? get() = Native.rtMenuTarget(h()).takeIf { it != Int.MIN_VALUE }

    /**
     * The seq of the last [pointerDown] when it landed on no object, or the object it pressed moves
     * the window, else null; a [doubleTap] clears it. The host moves the skin when such a press
     * turns into a drag.
     */
    public val moveTarget: Int? get() = Native.rtMoveTarget(h()).takeIf { it != Int.MIN_VALUE }

    /**
     * A right click on the object the touch [seq] landed on, at the point it landed (see
     * [menuTarget]), as a separate touch. Nothing happens for an older touch's seq, or when the
     * object is no longer on screen.
     */
    public fun rightClick(seq: Int): Unit = Native.rtRightClick(h(), seq)

    /**
     * Runs [block] as a touch the listener made, from which a script may pop a menu. The pointer
     * calls run in it; a host that opens the skin's menu itself wraps that call in it too. [fresh]
     * starts a new touch, whose menu count starts over.
     */
    public fun <T> gesture(
        fresh: Boolean = true,
        block: () -> T,
    ): T {
        Native.rtGestureBegin(h(), fresh)
        try {
            return block()
        } finally {
            Native.rtGestureEnd(h())
        }
    }

    /**
     * The listener's pick from [menu], a menu a script popped, by command id, or 0: shown through
     * [SkinHost.chooseFromMenu] when a touch is in progress ([gesture]), at most a few times per
     * touch, and not after one closed without the listener.
     */
    public fun menuChoice(menu: SkinMenu): Int = Native.rtMenuChoice(h(), PackedMenu.encode(menu))

    /**
     * Ends the press under the finger without a release: a button no longer shows pressed and gives
     * no click, a slider stops dragging without a final position. For a host that takes the touch,
     * such as a long press that opens the app's menu.
     */
    public fun pointerCancel(): Unit = Native.rtPointerCancel(h())

    private companion object {
        /** The screen a skin may place windows on before the host sets a view size: a Full HD portrait phone. */
        const val VIEWPORT_W = 1080
        const val VIEWPORT_H = 1920
    }
}
