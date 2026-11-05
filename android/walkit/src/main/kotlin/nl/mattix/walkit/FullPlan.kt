// SPDX-License-Identifier: GPL-3.0-or-later
package nl.mattix.walkit

/**
 * A window planned from the start ([DrawPlanner]): its structure checked, its drawing split into
 * stretches of one object each and compared with what each object drew in the frame before, the
 * live stretches gathered into runs, and the window's version moved when anything outside them
 * changed. Keeps what the quick look of the next frame needs ([QuickLook]).
 */
internal class FullPlan(
    private val owners: HashMap<Any, Owned>,
) {
    /** One entry of the window: a structure command (no [owned]) or a stretch of one object's drawing. */
    private class Item {
        var from = 0
        var to = 0
        var owned: Owned? = null
        var atom: Atom? = null

        /** Draws differently from the frame before, or was not there. */
        var changed = false

        /** Draws differently from the frame before, where it was. */
        var moved = false
        var live = false

        /** The run it is in, or -1, and whether it starts that run. */
        var runIndex = -1
        var starts = false

        // the clip in force, and the box it draws on inside it
        val clip = IntArray(4)
        var l = 0
        var t = 0
        var r = 0
        var b = 0

        val area: Long get() = if (r > l && b > t) (r - l).toLong() * (b - t) else 0L

        fun reset(from: Int) {
            this.from = from
            owned = null
            atom = null
            changed = false
            moved = false
            live = false
            runIndex = -1
            starts = false
            l = 0
            t = 0
            r = 0
            b = 0
        }
    }

    /** A run being gathered: items [first] to [last], the box around them and the sum of their areas. */
    private class RunBuild {
        var first = 0
        var last = 0
        val box = Extent()
        var areas = 0L

        val area: Long get() = if (box.empty) 0L else (box.r - box.l).toLong() * (box.b - box.t)
    }

    private val items = ArrayList<Item>()
    private var itemCount = 0
    private val builds = ArrayList<RunBuild>()
    private var buildCount = 0
    private var blockEnd = IntArray(INITIAL)
    private var ownerOf = arrayOfNulls<Any>(INITIAL)
    private var openAt = IntArray(DEPTH)
    private var openWhole = BooleanArray(DEPTH)
    private var clips = IntArray(DEPTH * 4)
    private var clipDepth = 0
    private var nextSeq = ArrayList<Any>()
    private var nextStructure = ArrayList<DrawCmd>()
    private val extent = Extent()

    /** [cmds], the window [ws] keeps of [rect]'s size, planned at [now] in frame [frame]. */
    fun plan(
        ws: WindowState,
        rect: Rect,
        cmds: List<DrawCmd>,
        now: Long,
        frame: Long,
    ): WindowPlan {
        itemCount = 0
        buildCount = 0
        val parsed = scan(cmds)
        if (parsed) gather(cmds, rect.width, rect.height, now, frame)
        // a window with no width or height is drawn in one piece
        val apart = parsed && !ws.flatAt(now) && rect.width > 0 && rect.height > 0
        val runs =
            if (apart && gatherRuns(rect.width, rect.height)) {
                finishRuns(ws, rect.width, rect.height)
            } else {
                emptyList()
            }
        for (i in 0 until itemCount) {
            val item = items[i]
            if (item.owned != null && !item.starts) item.atom?.run = null
        }
        version(ws, rect, cmds, parsed, now)
        val plan = WindowPlan(ws.id, ws.version, runs)
        keep(ws, cmds.takeIf { parsed }, plan)
        return plan
    }

    /** Moves [ws]'s version when what the window draws outside its runs changed, and notes its moves for the flat rule. */
    private fun version(
        ws: WindowState,
        rect: Rect,
        cmds: List<DrawCmd>,
        parsed: Boolean,
        now: Long,
    ) {
        val sized = rect.width != ws.w || rect.height != ws.h
        val shaped = fillSequence(ws, cmds, parsed)
        var resting = false
        var changed = false
        for (i in 0 until itemCount) {
            val item = items[i]
            if (item.owned == null || item.runIndex >= 0) continue
            if (item.changed) changed = true
            if (item.moved && !item.live) resting = true
        }
        val moved = ws.version == NEVER || sized || changed || !sameSequence(nextSeq, ws.seq)
        ws.moved(sized || resting || shaped, now)
        if (moved) ws.version = DrawPlanner.nextId()
        ws.w = rect.width
        ws.h = rect.height
        val oldSeq = ws.seq
        ws.seq = nextSeq
        nextSeq = oldSeq
        val oldStructure = ws.structure
        ws.structure = nextStructure
        nextStructure = oldStructure
    }

    /** What the quick look of the next frame needs of this one. */
    private fun keep(
        ws: WindowState,
        cmds: List<DrawCmd>?,
        plan: WindowPlan,
    ) {
        ws.cmds = cmds
        ws.plan = plan
        ws.count = itemCount
        ws.ensure(itemCount)
        for (i in 0 until itemCount) {
            val item = items[i]
            ws.from[i] = item.from
            ws.to[i] = item.to
            ws.owned[i] = item.owned
            ws.atoms[i] = item.atom
            ws.live[i] = item.live
            ws.runOf[i] = item.runIndex
            item.clip.copyInto(ws.clip, i * 4)
        }
        for (i in itemCount until ws.owned.size) {
            ws.owned[i] = null
            ws.atoms[i] = null
        }
    }

    /**
     * Checks the window's structure and finds each clip that opens and closes inside one object's
     * drawing ([blockEnd] then holds where it closes, else -1). False when a run could not be kept
     * apart: clips that do not pair up, or a shape anywhere but around the whole window.
     */
    private fun scan(cmds: List<DrawCmd>): Boolean {
        val n = cmds.size
        if (blockEnd.size < n) {
            blockEnd = IntArray(maxOf(n, blockEnd.size * 2))
            ownerOf = arrayOfNulls(blockEnd.size)
        }
        var depth = 0
        var ok = true
        for (i in 0 until n) {
            blockEnd[i] = -1
            ownerOf[i] = null
            when (val c = cmds[i]) {
                is DrawCmd.WindowStart -> ok = i == 0 && cmds[n - 1] === DrawCmd.WindowEnd
                DrawCmd.WindowEnd -> ok = i == n - 1 && cmds[0] is DrawCmd.WindowStart
                is DrawCmd.Clip -> open(i, c.ownedBy, depth++)
                DrawCmd.Unclip -> if (depth == 0) ok = false else close(i, --depth)
                else -> drawn(i, c.ownedBy, depth)
            }
            if (!ok) return false
        }
        return depth == 0
    }

    private fun open(
        i: Int,
        owner: Any?,
        depth: Int,
    ) {
        if (depth == openAt.size) {
            openAt = openAt.copyOf(depth * 2)
            openWhole = openWhole.copyOf(depth * 2)
        }
        ownerOf[i] = owner
        openAt[depth] = i
        openWhole[depth] = owner != null
    }

    /** The clip opened at depth [depth] closes at [i]: whole when all inside it is its owner's drawing. */
    private fun close(
        i: Int,
        depth: Int,
    ) {
        val start = openAt[depth]
        val whole = openWhole[depth]
        if (whole) blockEnd[start] = i
        if (depth > 0 && (!whole || ownerOf[start] != ownerOf[openAt[depth - 1]])) openWhole[depth - 1] = false
    }

    private fun drawn(
        i: Int,
        owner: Any?,
        depth: Int,
    ) {
        ownerOf[i] = owner
        if (depth > 0 && owner != ownerOf[openAt[depth - 1]]) openWhole[depth - 1] = false
    }

    /** The object a stretch of drawing starting at [i] belongs to, or null for structure. */
    private fun ownerAt(
        cmds: List<DrawCmd>,
        i: Int,
    ): Any? = if (cmds[i] is DrawCmd.Clip && blockEnd[i] < 0) null else ownerOf[i]

    /** Splits the window into structure and stretches of one object's drawing, each compared with the frame before. */
    private fun gather(
        cmds: List<DrawCmd>,
        w: Int,
        h: Int,
        now: Long,
        frame: Long,
    ) {
        clipDepth = 0
        pushClip(0, 0, w, h)
        val n = cmds.size
        var i = 0
        while (i < n) {
            val owner = ownerAt(cmds, i)
            if (itemCount == items.size) items.add(Item())
            val item = items[itemCount++]
            item.reset(i)
            val at = (clipDepth - 1) * 4
            clips.copyInto(item.clip, 0, at, at + 4)
            if (owner == null) {
                structure(cmds[i])
                item.to = ++i
                continue
            }
            while (i < n && ownerAt(cmds, i) == owner) i = if (cmds[i] is DrawCmd.Clip) blockEnd[i] + 1 else i + 1
            item.to = i
            extent.of(cmds, item.from, i, item.clip, 0)
            item.l = extent.l
            item.t = extent.t
            item.r = extent.r
            item.b = extent.b
            atom(item, owner, cmds, now, frame)
        }
    }

    /** [item] as the drawing of [owner], compared with what that drew in the frame before; its liveness follows. */
    private fun atom(
        item: Item,
        owner: Any,
        cmds: List<DrawCmd>,
        now: Long,
        frame: Long,
    ) {
        val rec = owners.getOrPut(owner) { Owned() }
        if (rec.seen != frame) {
            rec.seen = frame
            rec.used = 0
        }
        val k = rec.used++
        val fresh = k >= rec.atoms.size
        val atom = if (fresh) Atom().also { rec.atoms.add(it) } else rec.atoms[k]
        val same = !fresh && same(atom.cmds, cmds, item.from, item.to)
        if (!same) {
            atom.cmds.clear()
            for (j in item.from until item.to) atom.cmds.add(cmds[j])
            if (!fresh) rec.changed(now, frame)
        }
        item.owned = rec
        item.atom = atom
        item.changed = !same
        item.moved = !same && !fresh
        item.live = now < rec.liveUntil
    }

    private fun same(
        kept: List<DrawCmd>,
        cmds: List<DrawCmd>,
        from: Int,
        to: Int,
    ): Boolean {
        if (kept.size != to - from) return false
        for (j in kept.indices) if (!kept[j].drawsSameAs(cmds[from + j])) return false
        return true
    }

    /** Gathers the live stretches into runs; false when there would be too many or they would cover too much. */
    private fun gatherRuns(
        w: Int,
        h: Int,
    ): Boolean {
        var open: RunBuild? = null
        for (i in 0 until itemCount) {
            val item = items[i]
            if (item.owned == null || !item.live) {
                open = null
                continue
            }
            if (open != null && apart(open, item)) open = null
            val build = open ?: nextBuild(i).also { open = it }
            build.last = i
            build.areas += item.area
            build.box.join(item.l, item.t, item.r, item.b)
        }
        var area = 0L
        for (j in 0 until buildCount) area += builds[j].area
        return buildCount <= DrawPlanner.MAX_RUNS && area * 2 <= w.toLong() * h
    }

    /** Whether [item] lies so far from the run gathered so far that the box around both would be mostly empty. */
    private fun apart(
        run: RunBuild,
        item: Item,
    ): Boolean {
        if (item.area == 0L || run.area == 0L) return false
        val box = run.box
        val w = maxOf(box.r, item.r) - minOf(box.l, item.l)
        val h = maxOf(box.b, item.b) - minOf(box.t, item.t)
        return w.toLong() * h > 2 * (run.areas + item.area)
    }

    private fun nextBuild(first: Int): RunBuild {
        if (buildCount == builds.size) builds.add(RunBuild())
        return builds[buildCount++].also {
            it.first = first
            it.last = first
            it.box.clear()
            it.areas = 0
        }
    }

    /** The runs gathered, each with its id, version and bounds; the items they hold are marked. */
    private fun finishRuns(
        ws: WindowState,
        w: Int,
        h: Int,
    ): List<LiveRun> {
        val out = ArrayList<LiveRun>(buildCount)
        ws.runStates.clear()
        for (j in 0 until buildCount) {
            val build = builds[j]
            val first = items[build.first]
            val atom = checkNotNull(first.atom)
            val state = atom.run ?: RunState(DrawPlanner.nextId()).also { atom.run = it }
            var moved = state.version == 0L
            for (i in build.first..build.last) {
                items[i].runIndex = j
                if (items[i].changed) moved = true
            }
            if (state.hold(build.last - build.first + 1) { checkNotNull(items[build.first + it].owned) }) moved = true
            first.starts = true
            val bounds = build.box.grow(state.bounds, w, h)
            if (bounds !== state.bounds) moved = true
            state.bounds = bounds
            if (moved) state.version = DrawPlanner.nextId()
            ws.runStates.add(state)
            val at = bounds ?: Rect(0, 0, minOf(1, w), minOf(1, h))
            out.add(LiveRun(state.id, state.version, first.from, items[build.last].to, at))
        }
        return out
    }

    /**
     * What the window draws outside its runs goes by, in order, into [nextSeq]: its structure commands, the
     * objects drawn outside runs and the runs; and its structure alone into [nextStructure]. A
     * window whose structure did not scan is all structure. True when the structure differs from
     * the frame before.
     */
    private fun fillSequence(
        ws: WindowState,
        cmds: List<DrawCmd>,
        parsed: Boolean,
    ): Boolean {
        nextSeq.clear()
        nextStructure.clear()
        if (!parsed) {
            nextSeq.addAll(cmds)
            nextStructure.addAll(cmds)
            return !sameSequence(nextStructure, ws.structure)
        }
        for (i in 0 until itemCount) {
            val item = items[i]
            val owned = item.owned
            when {
                owned == null -> {
                    val c = cmds[item.from]
                    nextSeq.add(c)
                    nextStructure.add(c)
                }

                item.starts -> {
                    nextSeq.add(ws.runStates[item.runIndex])
                }

                item.runIndex < 0 -> {
                    nextSeq.add(owned)
                }
            }
        }
        return !sameSequence(nextStructure, ws.structure)
    }

    private fun sameSequence(
        a: List<Any>,
        b: List<Any>,
    ): Boolean {
        if (a.size != b.size) return false
        for (i in a.indices) {
            val x = a[i]
            val y = b[i]
            val same = if (x is DrawCmd) y is DrawCmd && x.drawsSameAs(y) else x === y
            if (!same) return false
        }
        return true
    }

    /** A structure command: what it does to the clip in force. */
    private fun structure(c: DrawCmd) {
        when (c) {
            is DrawCmd.WindowStart -> {
                // its content is drawn into a layer as large as the shape's box from the corner
                val b = c.region.bounds() ?: Rect(0, 0, 1, 1)
                pushClip(0, 0, b.right, b.bottom)
            }

            is DrawCmd.Clip -> {
                pushClip(c.rect.left, c.rect.top, c.rect.right, c.rect.bottom)
            }

            DrawCmd.WindowEnd, DrawCmd.Unclip -> {
                clipDepth = maxOf(1, clipDepth - 1)
            }

            else -> {
                Unit
            }
        }
    }

    /**
     * Opens a clip: the box from ([x0], [y0]) to ([x1], [y1]), inside the clip in force. A corner given
     * before the other clips between them, as the platform's canvas clips.
     */
    private fun pushClip(
        x0: Int,
        y0: Int,
        x1: Int,
        y1: Int,
    ) {
        if ((clipDepth + 1) * 4 > clips.size) clips = clips.copyOf(clips.size * 2)
        val at = clipDepth * 4
        val l = minOf(x0, x1)
        val t = minOf(y0, y1)
        val r = maxOf(x0, x1)
        val b = maxOf(y0, y1)
        if (clipDepth == 0) {
            clips[0] = l
            clips[1] = t
            clips[2] = r
            clips[3] = b
        } else {
            clips[at] = maxOf(l, clips[at - 4])
            clips[at + 1] = maxOf(t, clips[at - 3])
            clips[at + 2] = minOf(r, clips[at - 2])
            clips[at + 3] = minOf(b, clips[at - 1])
        }
        clipDepth++
    }

    private companion object {
        const val INITIAL = 256
        const val DEPTH = 16
    }
}
