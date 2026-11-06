// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit.fixtures

import nl.mattix.walkit.Native
import nl.mattix.walkit.PackedOut
import nl.mattix.walkit.PackedReader
import nl.mattix.walkit.SkinMenu
import nl.mattix.walkit.SkinRuntime
import nl.mattix.walkit.WindowId

// What a host's tests do to a loaded skin the way its scripts would. The host itself goes through
// SkinRuntime's API only; these stand in for a skin's scripts where a test has none.

/**
 * Something a script holds: an object of the skin, a container, a script's System, or an
 * object made with `new`. [call] runs one of its methods as `std.mi` names them, any case.
 */
public class ScriptObject internal constructor(
    private val rt: SkinRuntime,
    internal val kind: Int,
    internal val index: Int,
) {
    /**
     * Runs [method] with [args] (ints, strings and script objects) and gives what it answered: an
     * Int, a String, a [ScriptObject] or null. Throws when the receiver has no such method.
     */
    public fun call(
        method: String,
        vararg args: Any?,
    ): Any? {
        val out = PackedOut()
        out.i32(args.size)
        for (a in args) out.value(a)
        val answer = Native.rtScriptCall(rt.h(), kind, index, method, out.bytes()) ?: error("no method $method")
        return PackedReader(answer).value(rt)
    }

    private fun PackedOut.value(v: Any?) {
        when (v) {
            null -> {
                i32(0)
            }

            is Int -> {
                i32(1)
                i32(v)
            }

            is Boolean -> {
                i32(1)
                i32(if (v) 1 else 0)
            }

            is String -> {
                i32(2)
                inlineString(v)
            }

            is ScriptObject -> {
                i32(3)
                i32(v.kind)
                i32(v.index)
            }

            else -> {
                error("not a script value: $v")
            }
        }
    }

    internal companion object {
        const val OBJECT = 0
        const val CONTAINER = 1
        const val SYSTEM = 2
        const val MADE = 3
    }
}

private fun PackedReader.value(rt: SkinRuntime): Any? =
    when (val tag = i32()) {
        0 -> null
        1 -> i32()
        2 -> inlineString()
        3 -> ScriptObject(rt, i32(), i32())
        else -> error("script value tag $tag")
    }

/** The object with id [id] in the main window, as a script's `getObject` finds it. */
public fun SkinRuntime.scriptObject(id: String): ScriptObject {
    val i = Native.rtScriptObject(h(), id)
    check(i >= 0) { "no object $id" }
    return ScriptObject(this, ScriptObject.OBJECT, i)
}

/** The layout [window] shows, as a script holds a `Layout`. */
public fun SkinRuntime.scriptLayout(window: WindowId): ScriptObject {
    val i = Native.rtScriptLayout(h(), window.raw)
    check(i >= 0) { "window ${window.raw} not on screen" }
    return ScriptObject(this, ScriptObject.OBJECT, i)
}

/** The container with id [id], as a script's `getContainer` finds it. */
public fun SkinRuntime.scriptContainer(id: String): ScriptObject {
    val i = Native.rtScriptContainer(h(), id)
    check(i >= 0) { "no container $id" }
    return ScriptObject(this, ScriptObject.CONTAINER, i)
}

/** An object of class [className] (Timer, PopupMenu, ...), as a script's `new` makes it. */
public fun SkinRuntime.scriptMake(className: String): ScriptObject {
    val i = Native.rtScriptMake(h(), className)
    check(i >= 0) { "no class $className" }
    return ScriptObject(this, ScriptObject.MADE, i)
}

/**
 * Makes a popup menu with [commands] (text to id) and shows it under the finger, as a script's
 * `PopupMenu.popAtMouse()` does, inside a fresh gesture; returns the id picked, 0 for none.
 */
public fun SkinRuntime.popScriptMenu(vararg commands: Pair<String, Int>): Int {
    val menu = SkinMenu(commands.map { (text, id) -> SkinMenu.Command(id, text, checked = false, disabled = false) })
    return gesture(fresh = true) { menuChoice(menu) }
}

/** Starts a timer firing every [delayMs], as a script's `new Timer; setDelay; start` does. */
public fun SkinRuntime.startScriptTimer(delayMs: Int) {
    val timer = scriptMake("Timer")
    timer.call("setDelay", delayMs)
    timer.call("start")
}

/** When the next of the skin's timers fires, in [SkinRuntime.now]'s ms; null when none runs. */
public val SkinRuntime.nextTimerAt: Long? get() = Native.rtScriptNextTimerAt(h()).takeIf { it != Long.MIN_VALUE }

/** Moves and sizes [window] as a script's `Layout.resize(x, y, w, h)` does. */
public fun SkinRuntime.resizeAsScript(
    window: WindowId,
    x: Int,
    y: Int,
    w: Int,
    h: Int,
) {
    scriptLayout(window).call("resize", x, y, w, h)
}

/** Where a script reads [window] to be (`getLeft`, `getTop`). */
public fun SkinRuntime.placeAsScript(window: WindowId): Pair<Int, Int> {
    val l = scriptLayout(window)
    return l.call("getLeft") as Int to l.call("getTop") as Int
}

/** Switches [container] to its layout [layout] (a shade and back), as a script's `switchToLayout` does. */
public fun SkinRuntime.switchLayoutAsScript(
    container: String,
    layout: String,
) {
    scriptContainer(container).call("switchToLayout", layout)
}

/** Sets attribute [key] of object [id] in the main window to [value], as a script's `setXmlParam` does. */
public fun SkinRuntime.setParamAsScript(
    id: String,
    key: String,
    value: String,
) {
    scriptObject(id).call("setXmlParam", key, value)
}

/**
 * Puts object [id] of the main window on its way to a target, as a script's `gotoTarget` does:
 * the windows count as sliding until [endSlidesAsScript].
 */
public fun SkinRuntime.beginSlideAsScript(id: String) {
    Native.rtScriptBeginSlide(h(), scriptObject(id).index)
}

/** Ends every slide [beginSlideAsScript] began, as the slides reaching their targets do. */
public fun SkinRuntime.endSlidesAsScript() {
    Native.rtScriptEndSlides(h())
}

/** The step slider [id] of the main window is at, as a script's `getPosition` reads it. */
public fun SkinRuntime.sliderPosition(id: String): Int = scriptObject(id).call("getPosition") as Int

/** A Config item, as a script reaches it with getItem or newItem. */
public class ScriptConfigItem internal constructor(
    private val item: ScriptObject,
) {
    /** Makes attribute [name] with [default], as `newAttribute` does; a stored value wins over the default. */
    public fun option(
        name: String,
        default: String,
    ) {
        item.call("newAttribute", name, default)
    }

    /** Reads attribute [name], as `getAttribute` does, without making it. */
    public fun read(name: String) {
        item.call("getAttribute", name)
    }
}

/** The Config item [name] with [guid], as `getItem` or `newItem` reaches it. */
private fun SkinRuntime.configItem(
    name: String,
    guid: String,
): ScriptConfigItem {
    val i = Native.rtScriptConfigItem(h(), name, guid)
    check(i >= 0) { "no config item $name" }
    return ScriptConfigItem(ScriptObject(this, ScriptObject.MADE, i))
}

/** The Config item Winamp lists in its Options menu, by the GUID Winamp gives it. */
private const val OPTIONS_MENU = "{1828D28F-78DD-4647-8532-EBA504B8FC04}"

/** The Config item a skin lists its options for Winamp's Options menu in, made as getItem makes it. */
public fun SkinRuntime.optionsItem(): ScriptConfigItem = configItem(OPTIONS_MENU, OPTIONS_MENU)

/** A Config item made as newItem([name], [guid]) makes it. */
public fun SkinRuntime.newConfigItem(
    name: String,
    guid: String,
): ScriptConfigItem = configItem(name, guid)

/**
 * One event a test listens for: [event] on object [id] of the main window, or on System when
 * [id] is null, written down as [name] with its first [args] arguments.
 */
public class Listen(
    public val id: String?,
    public val event: String,
    public val name: String,
    public val args: Int,
)

/** From now on the engine writes down every event it fires, for [heard]. */
public fun SkinRuntime.recordEvents() {
    Native.rtScriptRecordEvents(h(), true)
}

/**
 * What a script listening as [listens] say would have heard since the last call, in order: each
 * event's name as the listen names it, with its first arguments as ints.
 */
public fun SkinRuntime.heard(listens: List<Listen>): List<Pair<String, List<Int>>> {
    val r = PackedReader(Native.rtScriptTakeEvents(h()))
    val objects = listens.mapNotNull { it.id }.distinct().associateWith { Native.rtScriptObject(h(), it) }
    val out = ArrayList<Pair<String, List<Int>>>()
    repeat(r.i32()) {
        val kind = r.i32()
        val index = r.i32()
        val event = r.inlineString()
        val args = List(r.i32()) { r.value(this) }
        for (l in listens) {
            if (!l.event.equals(event, ignoreCase = true)) continue
            val on = if (l.id == null) kind == ScriptObject.SYSTEM else kind == ScriptObject.OBJECT && objects[l.id] == index
            if (on) out += l.name to args.take(l.args).map { it as? Int ?: 0 }
        }
    }
    return out
}
