// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

/** One stretch of an object's drawing in a window, as the frame before drew it ([DrawPlanner]). */
internal class Atom {
    val cmds = ArrayList<DrawCmd>()

    /** The run this stretch started in the frame before, if it started one. */
    var run: RunState? = null
}

/** What [DrawPlanner] knows of one object that draws: its drawing in the frame before, and when it changed. */
internal class Owned {
    val atoms = ArrayList<Atom>()

    /** Stretches of its drawing seen in the frame being planned from the start. */
    var used = 0
    var seen = -1L
    var lastChangeAt = NEVER
    var liveUntil = NEVER

    /** The frame whose tick reported it moving. */
    var hinted = -1L

    /** When it stops being live if it changes at [now], in frame [frame]. */
    fun liveAfter(
        now: Long,
        frame: Long,
    ): Long {
        val soon = lastChangeAt != NEVER && now - lastChangeAt <= DrawPlanner.LIVE_GAP_MS
        return if (hinted == frame || soon || now < liveUntil) now + DrawPlanner.LIVE_HOLD_MS else liveUntil
    }

    /** It changes at [now], in frame [frame]. */
    fun changed(
        now: Long,
        frame: Long,
    ) {
        liveUntil = liveAfter(now, frame)
        lastChangeAt = now
    }
}

/** A live run as [DrawPlanner] keeps it: its id, its version and bounds, and the objects it held last. */
internal class RunState(
    val id: Long,
) {
    var version = 0L
    var bounds: Rect? = null
    val owners = ArrayList<Owned>()

    /** Sets what it holds to [count] objects from [ownerAt]; true when they are not the ones it held. */
    inline fun hold(
        count: Int,
        ownerAt: (Int) -> Owned,
    ): Boolean {
        var moved = owners.size != count
        for (k in 0 until count) {
            val owned = ownerAt(k)
            if (k >= owners.size) {
                owners.add(owned)
            } else if (owners[k] !== owned) {
                owners[k] = owned
                moved = true
            }
        }
        while (owners.size > count) owners.removeAt(owners.size - 1)
        return moved
    }
}

/** What [DrawPlanner] keeps of one window between frames. */
internal class WindowState(
    val id: Long,
) {
    var version = NEVER
    var w = -1
    var h = -1
    var seq = ArrayList<Any>()
    var structure = ArrayList<DrawCmd>()

    /** One bit for each of the last four frames: whether its shape, clips or resting objects changed. */
    var moves = 0
    var flat = false
    var lastMoveAt = NEVER
    var seen = -1L

    // the frame before, as planned, for the quick look: its commands (null when its structure did
    // not scan), its plan, and its entries with the clip in force at each
    var cmds: List<DrawCmd>? = null
    var plan: WindowPlan? = null
    var count = 0
    var from = IntArray(0)
    var to = IntArray(0)
    var owned = arrayOfNulls<Owned>(0)
    var atoms = arrayOfNulls<Atom>(0)
    var live = BooleanArray(0)
    var runOf = IntArray(0)
    var clip = IntArray(0)
    val runStates = ArrayList<RunState>()

    /** Room for [n] entries. */
    fun ensure(n: Int) {
        if (from.size >= n) return
        val size = maxOf(n, from.size * 2)
        from = from.copyOf(size)
        to = to.copyOf(size)
        owned = owned.copyOf(size)
        atoms = atoms.copyOf(size)
        live = live.copyOf(size)
        runOf = runOf.copyOf(size)
        clip = clip.copyOf(size * 4)
    }

    /** Whether it is drawn in one piece now: it enters at [DrawPlanner.FLAT_MOVES] moves, and leaves after resting. */
    fun flatAt(now: Long): Boolean {
        if (flat && now - lastMoveAt >= DrawPlanner.FLAT_EXIT_MS) {
            flat = false
            moves = 0
        }
        if (!flat && Integer.bitCount(moves) >= DrawPlanner.FLAT_MOVES) flat = true
        return flat
    }

    /** Whether [flatAt] would change its mind at [now]. */
    fun flatTurnsAt(now: Long): Boolean =
        if (flat) now - lastMoveAt >= DrawPlanner.FLAT_EXIT_MS else Integer.bitCount(moves) >= DrawPlanner.FLAT_MOVES

    /** One more frame: whether its shape, clips or resting objects changed in it, at [now]. */
    fun moved(
        static: Boolean,
        now: Long,
    ) {
        moves = ((moves shl 1) or (if (static) 1 else 0)) and MOVES_MASK
        if (static) lastMoveAt = now
    }

    private companion object {
        const val MOVES_MASK = 0xF
    }
}

/**
 * The box a stretch of commands can draw on ([of]), worked out into [l], [t], [r], [b] (empty when
 * [r] <= [l]), and boxes joined with it.
 */
internal class Extent {
    var l = 0
    var t = 0
    var r = 0
    var b = 0
    private var clips = IntArray(DEPTH * 4)

    val empty: Boolean get() = r <= l || b <= t

    /**
     * What commands [from] to [to] of [cmds] can draw on, through the clip held in [clip] at [at] and
     * the clips they open themselves. A picture is drawn at its destination or smaller: its piece is
     * never wider than the part it was cut for; text and bitmap text are drawn clipped to their box.
     * A box or clip whose right or bottom lies before its left or top covers what lies between them,
     * as the platform's canvas draws it; a picture or pixels with such a destination draw nothing.
     */
    fun of(
        cmds: List<DrawCmd>,
        from: Int,
        to: Int,
        clip: IntArray,
        at: Int,
    ) {
        clip.copyInto(clips, 0, at, at + 4)
        var depth = 0
        l = 0
        t = 0
        r = 0
        b = 0
        for (k in from until to) {
            when (val c = cmds[k]) {
                is DrawCmd.Image -> join(c.dst, depth, sorted = false)
                is DrawCmd.Text -> join(c.rect, depth, sorted = true)
                is DrawCmd.BitmapText -> join(c.rect, depth, sorted = true)
                is DrawCmd.Pixels -> join(c.rect, depth, sorted = false)
                is DrawCmd.Fill -> join(c.rect, depth, sorted = true)
                is DrawCmd.Clip -> open(c.rect, ++depth)
                DrawCmd.Unclip -> depth = maxOf(0, depth - 1)
                else -> Unit
            }
        }
    }

    private fun open(
        rect: Rect,
        depth: Int,
    ) {
        val d = depth * 4
        if (d + 4 > clips.size) clips = clips.copyOf(clips.size * 2)
        clips[d] = maxOf(minOf(rect.left, rect.right), clips[d - 4])
        clips[d + 1] = maxOf(minOf(rect.top, rect.bottom), clips[d - 3])
        clips[d + 2] = minOf(maxOf(rect.left, rect.right), clips[d - 2])
        clips[d + 3] = minOf(maxOf(rect.top, rect.bottom), clips[d - 1])
    }

    /** Joins what [box] covers through the clip at [depth]; [sorted] when it covers between swapped corners too. */
    private fun join(
        box: Rect,
        depth: Int,
        sorted: Boolean,
    ) {
        val d = depth * 4
        val l = if (sorted) minOf(box.left, box.right) else box.left
        val t = if (sorted) minOf(box.top, box.bottom) else box.top
        val r = if (sorted) maxOf(box.left, box.right) else box.right
        val b = if (sorted) maxOf(box.top, box.bottom) else box.bottom
        join(maxOf(l, clips[d]), maxOf(t, clips[d + 1]), minOf(r, clips[d + 2]), minOf(b, clips[d + 3]))
    }

    /** Joins the box [bl], [bt], [br], [bb]; an empty one adds nothing. */
    fun join(
        bl: Int,
        bt: Int,
        br: Int,
        bb: Int,
    ) {
        if (br <= bl || bb <= bt) return
        if (empty) {
            l = bl
            t = bt
            r = br
            b = bb
            return
        }
        l = minOf(l, bl)
        t = minOf(t, bt)
        r = maxOf(r, br)
        b = maxOf(b, bb)
    }

    /** Nothing. */
    fun clear() {
        l = 0
        t = 0
        r = 0
        b = 0
    }

    /** [old] grown by this box, inside a [w] x [h] window; null while neither holds anything. */
    fun grow(
        old: Rect?,
        w: Int,
        h: Int,
    ): Rect? {
        if (old != null) join(old.left, old.top, old.right, old.bottom)
        val nl = l.coerceIn(0, w)
        val nt = t.coerceIn(0, h)
        val nr = r.coerceIn(nl, w)
        val nb = b.coerceIn(nt, h)
        return when {
            empty || nr <= nl || nb <= nt -> null
            old != null && old.left == nl && old.top == nt && old.width == nr - nl && old.height == nb - nt -> old
            else -> Rect(nl, nt, nr - nl, nb - nt)
        }
    }

    private companion object {
        const val DEPTH = 16
    }
}

internal const val NEVER = Long.MIN_VALUE
