// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

/**
 * What the host tells a skin before it loads ([SkinRuntime]'s constructor). [skinName] is what
 * getSkinName() answers: the skin's folder name, which scripts key what they remember by.
 * [haveLibrary] says whether Winamp's Media Library lives inside the skin (`@HAVE_LIBRARY@`).
 * With [hostActions] the host answers the button actions Winamp answered outside the skin
 * ([SkinHost.hostAction]) and shows the library, the visualizer and video itself, so the skin's
 * frames for them stay shut.
 */
public data class SkinSetup(
    val skinName: String = "",
    val haveLibrary: Boolean = true,
    val hostActions: Boolean = false,
)

/**
 * How the host shows a skin's windows ([SkinRuntime.presentation]), in layout pixels:
 * - [viewSize]: the space the host has for the stack; null leaves each window at the size it has;
 * - [viewport]: the screen scripts may place windows on before there is a [viewSize];
 * - [columns]: 2 sets the windows that show a list out right of the main window, anything else docks every window under it;
 * - [columnWidth]: how wide that right column is, 0 for as wide as what shows of the main window;
 * - [stretchLast]: whether the last docked list window takes the height left under the others;
 * - [touchSlop]: how far beside an object a finger may land and still mean it;
 * - [touch]: whether a finger drives the skin rather than a mouse.
 */
public data class Presentation(
    val viewSize: Pair<Int, Int>? = null,
    val viewport: Pair<Int, Int> = 1080 to 1920,
    val columns: Int = 1,
    val columnWidth: Int = 0,
    val stretchLast: Boolean = true,
    val touchSlop: Int = 6,
    /**
     * Whether the windows are set out apart, each where it is, moved by a drag on its move areas
     * and docked and snapped as on Winamp's desktop (see [SkinRuntime.placeWindow]); false for one
     * stack.
     */
    val freeWindows: Boolean = false,
    /**
     * Whether a finger drives the skin rather than a mouse: a tap on a classic playlist's bottom
     * button leaves its menu open for a second tap, where a mouse holds the button and lets go
     * over a picture.
     */
    val touch: Boolean = false,
)

/**
 * What a skin keeps between runs ([SkinRuntime.savedSettings], [SkinRuntime.restoreSettings]):
 * - [attributes]: its settings by attribute name, as Winamp keeps them in studio.xnf;
 * - [strings], [ints]: its scripts' private and public values (setPrivateString, setPublicInt and
 *   the like), by lowercase "section/item" or "public/item".
 */
public data class SavedSettings(
    val attributes: Map<String, String> = emptyMap(),
    val strings: Map<String, String> = emptyMap(),
    val ints: Map<String, Int> = emptyMap(),
)

/**
 * What the engine has done so far ([SkinRuntime.stats]), for a host's timing log or a probe:
 * - [scriptSteps]: script steps since the last tick began, across every script;
 * - [timerFires]: timer events fired;
 * - [layoutPasses]: layout passes run;
 * - [shapesWorkedOut]: window shapes worked out.
 */
public data class EngineStats(
    val scriptSteps: Long = 0,
    val timerFires: Long = 0,
    val layoutPasses: Long = 0,
    val shapesWorkedOut: Long = 0,
)
