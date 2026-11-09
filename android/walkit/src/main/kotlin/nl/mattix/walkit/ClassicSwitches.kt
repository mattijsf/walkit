// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

/** What a classic skin's main window vis shows, in the order a press on it steps through. */
public enum class ClassicVisMode { ANALYZER, SCOPE, OFF }

/**
 * The switches of a classic skin a host keeps in step with its own settings
 * ([SkinRuntime.classicSwitches]): whether the time displays count down, whether the equalizer's
 * auto button is on, and what the main window's vis shows.
 */
public data class ClassicSwitches(
    val timeRemaining: Boolean = false,
    val eqAuto: Boolean = false,
    val visMode: ClassicVisMode = ClassicVisMode.ANALYZER,
)
