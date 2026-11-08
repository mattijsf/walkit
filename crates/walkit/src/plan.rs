// SPDX-License-Identifier: GPL-3.0-or-later

//! What a renderer may keep of each window between frames: the objects that change from frame to frame, kept apart as live runs, and the
//! rest of the window, each with a version that moves when its drawing does.
//!
//! Each object's drawing is compared, command by command ([`DrawCmd::draws_same_as`]), with
//! what it drew in the frame before, so a change shows whether or not the object asked for a
//! frame. An object that changes again within [`LIVE_GAP_MS`] of its last change, or that the
//! runtime's tick reported moving, is live until [`LIVE_HOLD_MS`] after its last change.
//! Neighboring live objects form one run, unless they lie far apart. A change outside the runs
//! moves the window's version; a change inside one moves that run's.
//!
//! A window is drawn in one piece (no runs) while its shape, the clips of its groups or its
//! resting objects keep changing (`FLAT_MOVES` of the last four frames), until they rest for
//! `FLAT_EXIT_MS`; also when it would have more than `MAX_RUNS` runs, or runs over more than
//! half of it, when its shape opens anywhere but at its ends, and while it has no width or
//! height.
//!
//! A window laid out as in the frame before, with only its objects' drawing changed and no
//! object turning live or resting, is compared command by command with that frame and keeps
//! its runs (the quick look); any other is planned from the start (the full plan).

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::draw::{DrawCmd, Rect};
use crate::gui::ObjId;
use crate::host::WindowId;
use crate::runtime::DrawnWindow;

pub mod check;

/// A change this soon after an object's last one makes it live: the time once a second, and a
/// seek thumb that moves a pixel every few seconds on a long track.
pub const LIVE_GAP_MS: i64 = 5_000;

/// How long a live object stays live after its last change: through a ticker's pause at the
/// end of its text, so the window is not recorded again as it stops and starts.
pub const LIVE_HOLD_MS: i64 = 10_000;

/// A window is drawn in one piece once its shape, clips or resting objects changed in this
/// many of its last four frames.
const FLAT_MOVES: u32 = 3;

/// A window drawn in one piece for that is kept apart again once those rested this long.
const FLAT_EXIT_MS: i64 = 1_000;

/// More runs than this, and a window is drawn in one piece.
pub const MAX_RUNS: usize = 12;

/// Objects and windows not drawn for this many frames are forgotten.
const PRUNE_EVERY: u64 = 64;

const NEVER: i64 = i64::MIN;

static IDS: AtomicU64 = AtomicU64::new(0);

/// An id or version no planner in the process has handed out.
fn next_id() -> u64 {
    IDS.fetch_add(1, Ordering::Relaxed) + 1
}

/// How a renderer may keep one window's drawing between frames: the stretches of the window's
/// commands that change from frame to frame (`runs`), each kept apart, and a `version` for the
/// rest. A renderer that kept what it drew of the window for this `id` and `version` may show
/// it again; the same goes for each run by its id and version. Ids and versions are never
/// handed out twice in a process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowPlan {
    /// The same while the same window layout is drawn.
    pub id: u64,
    /// Moves when anything of the window outside its runs draws differently, or its runs are others.
    pub version: u64,
    /// The changing stretches, in drawing order; none when the window is best drawn in one piece.
    pub runs: Vec<LiveRun>,
}

/// A stretch of a window's commands that changes from frame to frame, such as a visualizer, a
/// ticker or the time: commands `from` (inclusive) to `to` (exclusive) of the window's list. It
/// holds whole objects' drawing and no window structure, so its clips open and close inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveRun {
    /// The same while the stretch starts with the same object.
    pub id: u64,
    /// Moves when any of its commands draws differently, when it holds other objects, or when `bounds` grew.
    pub version: u64,
    /// The first command.
    pub from: usize,
    /// Past the last command.
    pub to: usize,
    /// In window pixels: every pixel its commands can change that the clips in force there let
    /// through lies inside. It never shrinks while the run keeps its id.
    pub bounds: Rect,
}

/// One window of a planned frame: as [`crate::SkinRuntime::draw_windows`] draws it, with its plan.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedWindow {
    /// The window.
    pub id: WindowId,
    /// Where it sits in the stack.
    pub rect: Rect,
    /// Its drawing.
    pub cmds: Rc<Vec<DrawCmd>>,
    /// What a renderer may keep of it.
    pub plan: WindowPlan,
}

/// One stretch of an object's drawing in a window, as the frame before drew it.
#[derive(Default)]
struct Atom {
    cmds: Vec<DrawCmd>,
    /// The run this stretch started in the frame before, if it started one.
    run: Option<u64>,
}

/// What the planner knows of one object that draws: its drawing in the frame before, and when it changed.
struct Owned {
    /// Tells one object's record from another's, also from one made again after a prune.
    key: u64,
    atoms: Vec<Atom>,
    /// Stretches of its drawing seen in the frame being planned from the start.
    used: usize,
    seen: u64,
    last_change_at: i64,
    live_until: i64,
    /// The frame whose tick reported it moving.
    hinted: u64,
}

impl Owned {
    fn new() -> Owned {
        Owned {
            key: next_id(),
            atoms: Vec::new(),
            used: 0,
            seen: 0,
            last_change_at: NEVER,
            live_until: NEVER,
            hinted: 0,
        }
    }

    /// When it stops being live if it changes at `now`, in frame `frame`.
    fn live_after(&self, now: i64, frame: u64) -> i64 {
        let soon = self.last_change_at != NEVER && now - self.last_change_at <= LIVE_GAP_MS;
        if self.hinted == frame || soon || now < self.live_until {
            now + LIVE_HOLD_MS
        } else {
            self.live_until
        }
    }

    /// It changes at `now`, in frame `frame`.
    fn changed(&mut self, now: i64, frame: u64) {
        self.live_until = self.live_after(now, frame);
        self.last_change_at = now;
    }
}

/// A live run as the planner keeps it: its version and bounds, and the objects it held last.
struct RunState {
    version: u64,
    bounds: Option<Rect>,
    /// The keys of the objects it holds.
    owners: Vec<u64>,
}

impl RunState {
    /// Sets what it holds to the objects with `keys`; true when they are not the ones it held.
    fn hold(&mut self, keys: &[u64]) -> bool {
        let moved = self.owners != keys;
        if moved {
            self.owners.clear();
            self.owners.extend_from_slice(keys);
        }
        moved
    }
}

/// What a window draws outside its runs goes by: a structure command, an object, or a run.
enum Seq {
    Cmd(DrawCmd),
    Owner(u64),
    Run(u64),
}

/// What the planner keeps of one window between frames.
struct WindowState {
    id: u64,
    version: u64,
    w: i32,
    h: i32,
    seq: Vec<Seq>,
    structure: Vec<DrawCmd>,
    /// One bit for each of the last four frames: whether its shape, clips or resting objects changed.
    moves: u32,
    flat: bool,
    last_move_at: i64,
    seen: u64,
    // the frame before, as planned, for the quick look: its commands (none when its structure did
    // not scan), its plan, and its entries with the clip in force at each
    cmds: Option<Rc<Vec<DrawCmd>>>,
    plan: Option<WindowPlan>,
    count: usize,
    from: Vec<usize>,
    to: Vec<usize>,
    owned: Vec<Option<ObjId>>,
    atoms: Vec<usize>,
    live: Vec<bool>,
    run_of: Vec<i32>,
    clip: Vec<i32>,
    run_states: Vec<u64>,
}

impl WindowState {
    fn new() -> WindowState {
        WindowState {
            id: next_id(),
            version: 0,
            w: -1,
            h: -1,
            seq: Vec::new(),
            structure: Vec::new(),
            moves: 0,
            flat: false,
            last_move_at: NEVER,
            seen: 0,
            cmds: None,
            plan: None,
            count: 0,
            from: Vec::new(),
            to: Vec::new(),
            owned: Vec::new(),
            atoms: Vec::new(),
            live: Vec::new(),
            run_of: Vec::new(),
            clip: Vec::new(),
            run_states: Vec::new(),
        }
    }

    /// Room for `n` entries.
    fn ensure(&mut self, n: usize) {
        if self.from.len() >= n {
            return;
        }
        let size = n.max(self.from.len() * 2);
        self.from.resize(size, 0);
        self.to.resize(size, 0);
        self.owned.resize(size, None);
        self.atoms.resize(size, 0);
        self.live.resize(size, false);
        self.run_of.resize(size, -1);
        self.clip.resize(size * 4, 0);
    }

    /// Whether it is drawn in one piece now: it enters at `FLAT_MOVES` moves, and leaves after resting.
    fn flat_at(&mut self, now: i64) -> bool {
        if self.flat && now - self.last_move_at >= FLAT_EXIT_MS {
            self.flat = false;
            self.moves = 0;
        }
        if !self.flat && self.moves.count_ones() >= FLAT_MOVES {
            self.flat = true;
        }
        self.flat
    }

    /// Whether `flat_at` would change its mind at `now`.
    fn flat_turns_at(&self, now: i64) -> bool {
        if self.flat {
            now - self.last_move_at >= FLAT_EXIT_MS
        } else {
            self.moves.count_ones() >= FLAT_MOVES
        }
    }

    /// One more frame: whether its shape, clips or resting objects changed in it, at `now`.
    fn moved(&mut self, moved: bool, now: i64) {
        self.moves = ((self.moves << 1) | (moved as u32)) & MOVES_MASK;
        if moved {
            self.last_move_at = now;
        }
    }
}

const MOVES_MASK: u32 = 0xF;

/// The box a stretch of commands can draw on, worked out into `l`, `t`, `r`, `b` (empty when
/// `r <= l`), and boxes joined with it.
#[derive(Default)]
struct Extent {
    l: i32,
    t: i32,
    r: i32,
    b: i32,
    clips: Vec<i32>,
}

impl Extent {
    fn empty(&self) -> bool {
        self.r <= self.l || self.b <= self.t
    }

    /// What commands `from` to `to` of `cmds` can draw on, through the clip held in `clip` at
    /// `at` and the clips they open themselves. A picture is drawn at its destination or
    /// smaller: its piece is never wider than the part it was cut for; text and bitmap text are
    /// drawn clipped to their box. A box or clip whose right or bottom lies before its left or
    /// top covers what lies between them, as the platform's canvas draws it; a picture or
    /// pixels with such a destination draw nothing.
    fn of(&mut self, cmds: &[DrawCmd], from: usize, to: usize, clip: &[i32], at: usize) {
        self.clips.clear();
        self.clips.extend_from_slice(&clip[at..at + 4]);
        let mut depth = 0usize;
        self.l = 0;
        self.t = 0;
        self.r = 0;
        self.b = 0;
        for c in &cmds[from..to] {
            match c {
                DrawCmd::Image { dst, .. } => self.join_box(dst, depth, false),
                DrawCmd::Text(t) => self.join_box(&t.rect, depth, true),
                DrawCmd::BitmapText(t) => self.join_box(&t.rect, depth, true),
                DrawCmd::Pixels { rect, .. } => self.join_box(rect, depth, false),
                DrawCmd::Fill { rect, .. } => self.join_box(rect, depth, true),
                DrawCmd::Clip(rect) => {
                    depth += 1;
                    self.open(rect, depth);
                }
                DrawCmd::Unclip => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }

    fn open(&mut self, rect: &Rect, depth: usize) {
        let d = depth * 4;
        if self.clips.len() < d + 4 {
            self.clips.resize(d + 4, 0);
        }
        self.clips[d] = rect.left.min(rect.right()).max(self.clips[d - 4]);
        self.clips[d + 1] = rect.top.min(rect.bottom()).max(self.clips[d - 3]);
        self.clips[d + 2] = rect.left.max(rect.right()).min(self.clips[d - 2]);
        self.clips[d + 3] = rect.top.max(rect.bottom()).min(self.clips[d - 1]);
    }

    /// Joins what `rect` covers through the clip at `depth`; `sorted` when it covers between swapped corners too.
    fn join_box(&mut self, rect: &Rect, depth: usize, sorted: bool) {
        let d = depth * 4;
        let (l, t, r, b) = if sorted {
            (
                rect.left.min(rect.right()),
                rect.top.min(rect.bottom()),
                rect.left.max(rect.right()),
                rect.top.max(rect.bottom()),
            )
        } else {
            (rect.left, rect.top, rect.right(), rect.bottom())
        };
        self.join(
            l.max(self.clips[d]),
            t.max(self.clips[d + 1]),
            r.min(self.clips[d + 2]),
            b.min(self.clips[d + 3]),
        );
    }

    /// Joins the box `bl`, `bt`, `br`, `bb`; an empty one adds nothing.
    fn join(&mut self, bl: i32, bt: i32, br: i32, bb: i32) {
        if br <= bl || bb <= bt {
            return;
        }
        if self.empty() {
            self.l = bl;
            self.t = bt;
            self.r = br;
            self.b = bb;
            return;
        }
        self.l = self.l.min(bl);
        self.t = self.t.min(bt);
        self.r = self.r.max(br);
        self.b = self.b.max(bb);
    }

    fn clear(&mut self) {
        self.l = 0;
        self.t = 0;
        self.r = 0;
        self.b = 0;
    }

    /// `old` grown by this box, inside a `w` x `h` window; `None` while neither holds anything.
    fn grow(&mut self, old: Option<Rect>, w: i32, h: i32) -> Option<Rect> {
        if let Some(o) = old {
            self.join(o.left, o.top, o.right(), o.bottom());
        }
        let nl = self.l.clamp(0, w.max(0));
        let nt = self.t.clamp(0, h.max(0));
        let nr = self.r.clamp(nl, w.max(nl));
        let nb = self.b.clamp(nt, h.max(nt));
        if self.empty() || nr <= nl || nb <= nt {
            None
        } else {
            Some(Rect::new(nl, nt, nr - nl, nb - nt))
        }
    }
}

/// One entry of a window in the full plan: a structure command (no `owned`) or a stretch of one object's drawing.
#[derive(Default, Clone)]
struct Item {
    from: usize,
    to: usize,
    owned: Option<ObjId>,
    atom: usize,
    /// Draws differently from the frame before, or was not there.
    changed: bool,
    /// Draws differently from the frame before, where it was.
    moved: bool,
    live: bool,
    /// The run it is in, or -1, and whether it starts that run.
    run_index: i32,
    starts: bool,
    // the clip in force, and the box it draws on inside it
    clip: [i32; 4],
    l: i32,
    t: i32,
    r: i32,
    b: i32,
}

impl Item {
    fn area(&self) -> i64 {
        if self.r > self.l && self.b > self.t {
            (self.r - self.l) as i64 * (self.b - self.t) as i64
        } else {
            0
        }
    }

    fn reset(&mut self, from: usize) {
        *self = Item {
            from,
            ..Item::default()
        };
        self.run_index = -1;
    }
}

/// A run being gathered: items `first` to `last`, the box around them and the sum of their areas.
#[derive(Default)]
struct RunBuild {
    first: usize,
    last: usize,
    extent: Extent,
    areas: i64,
}

impl RunBuild {
    fn area(&self) -> i64 {
        if self.extent.empty() {
            0
        } else {
            (self.extent.r - self.extent.l) as i64 * (self.extent.b - self.extent.t) as i64
        }
    }
}

const SAME: u8 = 0;
const CHANGED: u8 = 1;
const KIND: u8 = 2;

/// Plans every window of a frame ([`DrawPlanner::plan`]); one planner for one runtime.
pub struct DrawPlanner {
    owners: HashMap<ObjId, Owned>,
    windows: HashMap<WindowId, WindowState>,
    runs: HashMap<u64, RunState>,
    frames: u64,
    full_plans: u64,
    quick_plans: u64,
    // the full plan's working room
    items: Vec<Item>,
    item_count: usize,
    builds: Vec<RunBuild>,
    build_count: usize,
    block_end: Vec<i32>,
    owner_of: Vec<Option<ObjId>>,
    open_at: Vec<usize>,
    open_whole: Vec<bool>,
    clips: Vec<i32>,
    clip_depth: usize,
    next_seq: Vec<Seq>,
    next_structure: Vec<DrawCmd>,
    extent: Extent,
    // the quick look's
    changed_at: Vec<usize>,
    changed_flag: Vec<bool>,
    run_moved: Vec<bool>,
    boxes: Vec<Extent>,
}

impl Default for DrawPlanner {
    fn default() -> Self {
        DrawPlanner::new()
    }
}

impl DrawPlanner {
    /// A planner that has seen no frame.
    pub fn new() -> DrawPlanner {
        DrawPlanner {
            owners: HashMap::new(),
            windows: HashMap::new(),
            runs: HashMap::new(),
            frames: 0,
            full_plans: 0,
            quick_plans: 0,
            items: Vec::new(),
            item_count: 0,
            builds: Vec::new(),
            build_count: 0,
            block_end: Vec::new(),
            owner_of: Vec::new(),
            open_at: Vec::new(),
            open_whole: Vec::new(),
            clips: Vec::new(),
            clip_depth: 0,
            next_seq: Vec::new(),
            next_structure: Vec::new(),
            extent: Extent::default(),
            changed_at: Vec::new(),
            changed_flag: Vec::new(),
            run_moved: Vec::new(),
            boxes: Vec::new(),
        }
    }

    /// Windows planned from the start, so far; for tests and timing runs.
    pub fn full_plans(&self) -> u64 {
        self.full_plans
    }

    /// Windows that took the quick look, so far; for tests and timing runs.
    pub fn quick_plans(&self) -> u64 {
        self.quick_plans
    }

    /// The frame's windows (`ids` and `drawn` in the same order, as
    /// [`crate::SkinRuntime::draw_windows_owned`] gives them) planned at `now`, in any clock that
    /// only moves forward, with `moving` the objects the last tick found moving by themselves
    /// ([`crate::SkinRuntime::take_tick_changed`]).
    pub fn plan(
        &mut self,
        ids: &[WindowId],
        drawn: Vec<DrawnWindow>,
        moving: &[ObjId],
        now: i64,
    ) -> Vec<PlannedWindow> {
        self.frames += 1;
        let frame = self.frames;
        for o in moving {
            if let Some(rec) = self.owners.get_mut(o) {
                rec.hinted = frame;
            }
        }
        let mut out = Vec::with_capacity(drawn.len());
        for (id, win) in ids.iter().zip(drawn) {
            let plan = self.plan_window_now(*id, win.rect, &win.cmds, &win.owners, now);
            out.push(PlannedWindow {
                id: *id,
                rect: win.rect,
                cmds: win.cmds,
                plan,
            });
        }
        if self.frames % PRUNE_EVERY == 0 {
            self.prune();
        }
        out
    }

    /// One window's plan as [`DrawPlanner::plan`] works it out, as a frame by itself at `now`:
    /// `key` stands for its layout, `owners` for who drew each command and `moving` for the
    /// objects the tick reported moving. For tests.
    pub fn plan_window(
        &mut self,
        key: WindowId,
        rect: Rect,
        cmds: &[DrawCmd],
        owners: &[Option<ObjId>],
        now: i64,
        moving: &[ObjId],
    ) -> WindowPlan {
        self.frames += 1;
        let frame = self.frames;
        for o in moving {
            if let Some(rec) = self.owners.get_mut(o) {
                rec.hinted = frame;
            }
        }
        self.plan_window_now(key, rect, &Rc::new(cmds.to_vec()), owners, now)
    }

    fn plan_window_now(
        &mut self,
        key: WindowId,
        rect: Rect,
        cmds: &Rc<Vec<DrawCmd>>,
        owners: &[Option<ObjId>],
        now: i64,
    ) -> WindowPlan {
        let frame = self.frames;
        let mut ws = self.windows.remove(&key).unwrap_or_else(WindowState::new);
        ws.seen = frame;
        let plan = if let Some(p) = self.quick(&mut ws, rect, cmds, owners, now, frame) {
            self.quick_plans += 1;
            p
        } else {
            self.full_plans += 1;
            self.full(&mut ws, rect, cmds, owners, now, frame)
        };
        self.windows.insert(key, ws);
        plan
    }

    /// Forgets windows not drawn for `PRUNE_EVERY` frames, and objects no window drew in that time.
    fn prune(&mut self) {
        let before = self.frames - PRUNE_EVERY;
        self.windows.retain(|_, ws| ws.seen >= before);
        // the quick look leaves the objects of a window it planned unvisited
        for ws in self.windows.values() {
            for j in 0..ws.count {
                if let Some(o) = ws.owned[j] {
                    if let Some(rec) = self.owners.get_mut(&o) {
                        rec.seen = self.frames;
                    }
                }
            }
        }
        let runs = &mut self.runs;
        self.owners.retain(|_, rec| {
            let keep = rec.seen >= before;
            if !keep {
                for a in &rec.atoms {
                    if let Some(r) = a.run {
                        runs.remove(&r);
                    }
                }
            }
            keep
        });
        for rec in self.owners.values_mut() {
            while rec.atoms.len() > rec.used.max(1) {
                if let Some(a) = rec.atoms.pop() {
                    if let Some(r) = a.run {
                        runs.remove(&r);
                    }
                }
            }
        }
    }

    // ---- the full plan ------------------------------------------------------------------------

    /// `cmds`, the window `ws` keeps of `rect`'s size, planned from the start at `now` in frame `frame`.
    fn full(
        &mut self,
        ws: &mut WindowState,
        rect: Rect,
        cmds: &Rc<Vec<DrawCmd>>,
        owners: &[Option<ObjId>],
        now: i64,
        frame: u64,
    ) -> WindowPlan {
        self.item_count = 0;
        self.build_count = 0;
        let parsed = self.scan(cmds, owners);
        if parsed {
            self.gather(cmds, rect.width, rect.height, now, frame);
        }
        // a window with no width or height is drawn in one piece
        let apart = parsed && !ws.flat_at(now) && rect.width > 0 && rect.height > 0;
        let runs = if apart && self.gather_runs(rect.width, rect.height) {
            self.finish_runs(ws, rect.width, rect.height)
        } else {
            Vec::new()
        };
        for i in 0..self.item_count {
            let item = &self.items[i];
            if let Some(o) = item.owned {
                if !item.starts {
                    let atom =
                        &mut self.owners.get_mut(&o).expect("an object seen").atoms[item.atom];
                    if let Some(r) = atom.run.take() {
                        self.runs.remove(&r);
                    }
                }
            }
        }
        self.version(ws, rect, cmds, parsed, now);
        let plan = WindowPlan {
            id: ws.id,
            version: ws.version,
            runs,
        };
        self.keep(ws, if parsed { Some(cmds) } else { None }, &plan);
        plan
    }

    /// Moves `ws`'s version when what the window draws outside its runs changed, and notes its moves for the flat rule.
    fn version(
        &mut self,
        ws: &mut WindowState,
        rect: Rect,
        cmds: &[DrawCmd],
        parsed: bool,
        now: i64,
    ) {
        let sized = rect.width != ws.w || rect.height != ws.h;
        let shaped = self.fill_sequence(ws, cmds, parsed);
        let mut resting = false;
        let mut changed = false;
        for i in 0..self.item_count {
            let item = &self.items[i];
            if item.owned.is_none() || item.run_index >= 0 {
                continue;
            }
            if item.changed {
                changed = true;
            }
            if item.moved && !item.live {
                resting = true;
            }
        }
        let moved = ws.version == 0 || sized || changed || !same_sequence(&self.next_seq, &ws.seq);
        ws.moved(sized || resting || shaped, now);
        if moved {
            ws.version = next_id();
        }
        ws.w = rect.width;
        ws.h = rect.height;
        std::mem::swap(&mut ws.seq, &mut self.next_seq);
        std::mem::swap(&mut ws.structure, &mut self.next_structure);
    }

    /// What the quick look of the next frame needs of this one.
    fn keep(&mut self, ws: &mut WindowState, cmds: Option<&Rc<Vec<DrawCmd>>>, plan: &WindowPlan) {
        ws.cmds = cmds.cloned();
        ws.plan = Some(plan.clone());
        ws.count = self.item_count;
        ws.ensure(self.item_count);
        for i in 0..self.item_count {
            let item = &self.items[i];
            ws.from[i] = item.from;
            ws.to[i] = item.to;
            ws.owned[i] = item.owned;
            ws.atoms[i] = item.atom;
            ws.live[i] = item.live;
            ws.run_of[i] = item.run_index;
            ws.clip[i * 4..i * 4 + 4].copy_from_slice(&item.clip);
        }
        for i in self.item_count..ws.owned.len() {
            ws.owned[i] = None;
        }
    }

    /// Checks the window's structure and finds each clip that opens and closes inside one object's
    /// drawing (`block_end` then holds where it closes, else -1). False when a run could not be kept
    /// apart: clips that do not pair up, or a shape anywhere but around the whole window.
    fn scan(&mut self, cmds: &[DrawCmd], owners: &[Option<ObjId>]) -> bool {
        let n = cmds.len();
        if self.block_end.len() < n {
            let size = n.max(self.block_end.len() * 2);
            self.block_end.resize(size, -1);
            self.owner_of.resize(size, None);
        }
        let mut depth = 0usize;
        for (i, c) in cmds.iter().enumerate() {
            self.block_end[i] = -1;
            self.owner_of[i] = None;
            let owner = owners.get(i).copied().flatten();
            let ok = match c {
                DrawCmd::WindowStart(_) => i == 0 && matches!(cmds[n - 1], DrawCmd::WindowEnd),
                DrawCmd::WindowEnd => i == n - 1 && matches!(cmds[0], DrawCmd::WindowStart(_)),
                DrawCmd::Clip(_) => {
                    self.open(i, owner, depth);
                    depth += 1;
                    true
                }
                DrawCmd::Unclip => {
                    if depth == 0 {
                        false
                    } else {
                        depth -= 1;
                        self.close(i, depth);
                        true
                    }
                }
                _ => {
                    self.drawn(i, owner, depth);
                    true
                }
            };
            if !ok {
                return false;
            }
        }
        depth == 0
    }

    fn open(&mut self, i: usize, owner: Option<ObjId>, depth: usize) {
        if depth == self.open_at.len() {
            self.open_at.push(0);
            self.open_whole.push(false);
        }
        self.owner_of[i] = owner;
        self.open_at[depth] = i;
        self.open_whole[depth] = owner.is_some();
    }

    /// The clip opened at depth `depth` closes at `i`: whole when all inside it is its owner's drawing.
    fn close(&mut self, i: usize, depth: usize) {
        let start = self.open_at[depth];
        let whole = self.open_whole[depth];
        if whole {
            self.block_end[start] = i as i32;
        }
        if depth > 0 && (!whole || self.owner_of[start] != self.owner_of[self.open_at[depth - 1]]) {
            self.open_whole[depth - 1] = false;
        }
    }

    fn drawn(&mut self, i: usize, owner: Option<ObjId>, depth: usize) {
        self.owner_of[i] = owner;
        if depth > 0 && owner != self.owner_of[self.open_at[depth - 1]] {
            self.open_whole[depth - 1] = false;
        }
    }

    /// The object a stretch of drawing starting at `i` belongs to, or `None` for structure.
    fn owner_at(&self, cmds: &[DrawCmd], i: usize) -> Option<ObjId> {
        if matches!(cmds[i], DrawCmd::Clip(_)) && self.block_end[i] < 0 {
            None
        } else {
            self.owner_of[i]
        }
    }

    /// Splits the window into structure and stretches of one object's drawing, each compared with the frame before.
    fn gather(&mut self, cmds: &[DrawCmd], w: i32, h: i32, now: i64, frame: u64) {
        self.clip_depth = 0;
        self.push_clip(0, 0, w, h);
        let n = cmds.len();
        let mut i = 0;
        while i < n {
            let owner = self.owner_at(cmds, i);
            if self.item_count == self.items.len() {
                self.items.push(Item::default());
            }
            let index = self.item_count;
            self.item_count += 1;
            self.items[index].reset(i);
            let at = (self.clip_depth - 1) * 4;
            let clip: [i32; 4] = self.clips[at..at + 4].try_into().expect("four edges");
            self.items[index].clip = clip;
            let Some(owner) = owner else {
                self.structure(&cmds[i]);
                i += 1;
                self.items[index].to = i;
                continue;
            };
            while i < n && self.owner_at(cmds, i) == Some(owner) {
                i = if matches!(cmds[i], DrawCmd::Clip(_)) {
                    self.block_end[i] as usize + 1
                } else {
                    i + 1
                };
            }
            self.items[index].to = i;
            let from = self.items[index].from;
            self.extent.of(cmds, from, i, &clip, 0);
            let item = &mut self.items[index];
            item.l = self.extent.l;
            item.t = self.extent.t;
            item.r = self.extent.r;
            item.b = self.extent.b;
            self.atom(index, owner, cmds, now, frame);
        }
    }

    /// Item `index` as the drawing of `owner`, compared with what that drew in the frame before; its liveness follows.
    fn atom(&mut self, index: usize, owner: ObjId, cmds: &[DrawCmd], now: i64, frame: u64) {
        let (from, to) = (self.items[index].from, self.items[index].to);
        let rec = self.owners.entry(owner).or_insert_with(Owned::new);
        if rec.seen != frame {
            rec.seen = frame;
            rec.used = 0;
        }
        let k = rec.used;
        rec.used += 1;
        let fresh = k >= rec.atoms.len();
        if fresh {
            rec.atoms.push(Atom::default());
        }
        let same = !fresh && same_cmds(&rec.atoms[k].cmds, &cmds[from..to]);
        if !same {
            let atom = &mut rec.atoms[k];
            atom.cmds.clear();
            atom.cmds.extend_from_slice(&cmds[from..to]);
            if !fresh {
                rec.changed(now, frame);
            }
        }
        let live = now < rec.live_until;
        let item = &mut self.items[index];
        item.owned = Some(owner);
        item.atom = k;
        item.changed = !same;
        item.moved = !same && !fresh;
        item.live = live;
    }

    /// Gathers the live stretches into runs; false when there would be too many or they would cover too much.
    fn gather_runs(&mut self, w: i32, h: i32) -> bool {
        let mut open: Option<usize> = None;
        for i in 0..self.item_count {
            let item = self.items[i].clone();
            if item.owned.is_none() || !item.live {
                open = None;
                continue;
            }
            if let Some(b) = open {
                if apart(&self.builds[b], &item) {
                    open = None;
                }
            }
            let b = if let Some(b) = open {
                b
            } else {
                let b = self.next_build(i);
                open = Some(b);
                b
            };
            let build = &mut self.builds[b];
            build.last = i;
            build.areas += item.area();
            build.extent.join(item.l, item.t, item.r, item.b);
        }
        let area: i64 = self.builds[..self.build_count]
            .iter()
            .map(|b| b.area())
            .sum();
        self.build_count <= MAX_RUNS && area * 2 <= w as i64 * h as i64
    }

    fn next_build(&mut self, first: usize) -> usize {
        if self.build_count == self.builds.len() {
            self.builds.push(RunBuild::default());
        }
        let b = self.build_count;
        self.build_count += 1;
        let build = &mut self.builds[b];
        build.first = first;
        build.last = first;
        build.extent.clear();
        build.areas = 0;
        b
    }

    /// The runs gathered, each with its id, version and bounds; the items they hold are marked.
    fn finish_runs(&mut self, ws: &mut WindowState, w: i32, h: i32) -> Vec<LiveRun> {
        let mut out = Vec::with_capacity(self.build_count);
        ws.run_states.clear();
        let mut keys = Vec::new();
        for j in 0..self.build_count {
            let (first, last) = (self.builds[j].first, self.builds[j].last);
            let first_item = &self.items[first];
            let owner = first_item.owned.expect("a run starts with an object");
            let atom_index = first_item.atom;
            let run_id = {
                let atom =
                    &mut self.owners.get_mut(&owner).expect("an object seen").atoms[atom_index];
                match atom.run {
                    Some(r) if self.runs.contains_key(&r) => r,
                    _ => {
                        let r = next_id();
                        atom.run = Some(r);
                        self.runs.insert(
                            r,
                            RunState {
                                version: 0,
                                bounds: None,
                                owners: Vec::new(),
                            },
                        );
                        r
                    }
                }
            };
            let mut moved = self.runs[&run_id].version == 0;
            keys.clear();
            for i in first..=last {
                self.items[i].run_index = j as i32;
                if self.items[i].changed {
                    moved = true;
                }
                let o = self.items[i].owned.expect("a live item is an object's");
                keys.push(self.owners[&o].key);
            }
            let state = self
                .runs
                .get_mut(&run_id)
                .expect("the run just made or kept");
            if state.hold(&keys) {
                moved = true;
            }
            self.items[first].starts = true;
            let bounds = self.builds[j].extent.grow(state.bounds, w, h);
            if bounds != state.bounds {
                moved = true;
            }
            state.bounds = bounds;
            if moved {
                state.version = next_id();
            }
            ws.run_states.push(run_id);
            let at = bounds.unwrap_or_else(|| Rect::new(0, 0, w.min(1), h.min(1)));
            out.push(LiveRun {
                id: run_id,
                version: state.version,
                from: self.items[first].from,
                to: self.items[last].to,
                bounds: at,
            });
        }
        out
    }

    /// What the window draws outside its runs goes by, in order, into `next_seq`: its structure
    /// commands, the objects drawn outside runs and the runs; and its structure alone into
    /// `next_structure`. A window whose structure did not scan is all structure. True when the
    /// structure differs from the frame before.
    fn fill_sequence(&mut self, ws: &WindowState, cmds: &[DrawCmd], parsed: bool) -> bool {
        self.next_seq.clear();
        self.next_structure.clear();
        if !parsed {
            self.next_seq.extend(cmds.iter().cloned().map(Seq::Cmd));
            self.next_structure.extend_from_slice(cmds);
            return !same_cmds(&self.next_structure, &ws.structure);
        }
        for i in 0..self.item_count {
            let item = &self.items[i];
            match item.owned {
                None => {
                    let c = &cmds[item.from];
                    self.next_seq.push(Seq::Cmd(c.clone()));
                    self.next_structure.push(c.clone());
                }
                Some(o) => {
                    if item.starts {
                        self.next_seq
                            .push(Seq::Run(ws.run_states[item.run_index as usize]));
                    } else if item.run_index < 0 {
                        self.next_seq.push(Seq::Owner(self.owners[&o].key));
                    }
                }
            }
        }
        !same_cmds(&self.next_structure, &ws.structure)
    }

    /// A structure command: what it does to the clip in force.
    fn structure(&mut self, c: &DrawCmd) {
        match c {
            DrawCmd::WindowStart(region) => {
                // its content is drawn into a layer as large as the shape's box from the corner
                let b = region.bounds().unwrap_or(Rect::new(0, 0, 1, 1));
                self.push_clip(0, 0, b.right(), b.bottom());
            }
            DrawCmd::Clip(rect) => {
                self.push_clip(rect.left, rect.top, rect.right(), rect.bottom());
            }
            DrawCmd::WindowEnd | DrawCmd::Unclip => {
                self.clip_depth = self.clip_depth.saturating_sub(1).max(1);
            }
            _ => {}
        }
    }

    /// Opens a clip: the box from (`x0`, `y0`) to (`x1`, `y1`), inside the clip in force. A corner
    /// given before the other clips between them, as the platform's canvas clips.
    fn push_clip(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        let at = self.clip_depth * 4;
        if self.clips.len() < at + 4 {
            self.clips.resize(at + 4, 0);
        }
        let l = x0.min(x1);
        let t = y0.min(y1);
        let r = x0.max(x1);
        let b = y0.max(y1);
        if self.clip_depth == 0 {
            self.clips[0] = l;
            self.clips[1] = t;
            self.clips[2] = r;
            self.clips[3] = b;
        } else {
            self.clips[at] = l.max(self.clips[at - 4]);
            self.clips[at + 1] = t.max(self.clips[at - 3]);
            self.clips[at + 2] = r.min(self.clips[at - 2]);
            self.clips[at + 3] = b.min(self.clips[at - 1]);
        }
        self.clip_depth += 1;
    }

    // ---- the quick look -----------------------------------------------------------------------

    /// `cmds`, the window `ws` keeps of `rect`'s size, at `now` in frame `frame`; `None` when it must be planned from the start.
    fn quick(
        &mut self,
        ws: &mut WindowState,
        rect: Rect,
        cmds: &Rc<Vec<DrawCmd>>,
        owners: &[Option<ObjId>],
        now: i64,
        frame: u64,
    ) -> Option<WindowPlan> {
        let laid_out = match (&ws.cmds, &ws.plan) {
            (Some(prev), Some(_)) => {
                prev.len() == cmds.len() && rect.width == ws.w && rect.height == ws.h
            }
            _ => false,
        };
        if !laid_out || ws.flat_turns_at(now) {
            return None;
        }
        let prev = ws.cmds.take().expect("the frame before");
        // the same list as the frame before changed nowhere
        let changes = if Rc::ptr_eq(&prev, cmds) {
            Some(0)
        } else {
            self.changes(ws, &prev, cmds)
        };
        let result = changes.filter(|changes| self.steady(ws, owners, *changes, now, frame));
        if let Some(changes) = result {
            Some(self.commit(ws, cmds, now, frame, changes))
        } else {
            ws.cmds = Some(prev);
            None
        }
    }

    /// How many stretches of `ws` draw differently in `cmds` than in `prev`, noted in `changed_at`; `None` when the layout differs.
    fn changes(&mut self, ws: &WindowState, prev: &[DrawCmd], cmds: &[DrawCmd]) -> Option<usize> {
        if self.changed_at.len() < ws.count {
            let size = ws.count.max(self.changed_at.len() * 2);
            self.changed_at.resize(size, 0);
            self.changed_flag.resize(size, false);
        }
        let mut changes = 0;
        for j in 0..ws.count {
            let from = ws.from[j];
            let result = if ws.owned[j].is_none() {
                if cmds[from].draws_same_as(&prev[from]) {
                    SAME
                } else {
                    KIND
                }
            } else {
                compare(prev, cmds, from, ws.to[j])
            };
            if result == KIND {
                return None;
            }
            if result == CHANGED {
                self.changed_at[changes] = j;
                changes += 1;
            }
        }
        Some(changes)
    }

    /// Whether the `changes` found leave every object as live or as resting as it was, and drawn by
    /// the same object: in a window kept apart, that keeps its runs as they are.
    fn steady(
        &mut self,
        ws: &WindowState,
        owners: &[Option<ObjId>],
        changes: usize,
        now: i64,
        frame: u64,
    ) -> bool {
        let mut steady = true;
        for c in 0..changes {
            let j = self.changed_at[c];
            let from = ws.from[j];
            if owners.get(from).copied().flatten() != ws.owned[j] {
                steady = false;
            }
            if !ws.flat {
                let rec = &self.owners[&ws.owned[j].expect("a changed stretch is an object's")];
                if (now < rec.live_after(now, frame)) != ws.live[j] {
                    steady = false;
                }
            }
        }
        if !steady || ws.flat {
            return steady;
        }
        for c in 0..changes {
            self.changed_flag[self.changed_at[c]] = true;
        }
        for j in 0..ws.count {
            let Some(o) = ws.owned[j] else { continue };
            let rec = &self.owners[&o];
            if ws.live[j] && !self.changed_flag[j] && now >= rec.live_until {
                steady = false;
            }
        }
        for c in 0..changes {
            self.changed_flag[self.changed_at[c]] = false;
        }
        steady
    }

    /// The plan once nothing calls for planning from the start: the `changes` found applied.
    fn commit(
        &mut self,
        ws: &mut WindowState,
        cmds: &Rc<Vec<DrawCmd>>,
        now: i64,
        frame: u64,
        changes: usize,
    ) -> WindowPlan {
        let plan = ws.plan.take().expect("the plan before");
        let run_count = plan.runs.len();
        if self.run_moved.len() < run_count {
            self.run_moved.resize(run_count * 2, false);
            self.boxes.resize_with(run_count * 2, Extent::default);
        }
        for r in 0..run_count {
            self.run_moved[r] = false;
        }
        let mut outside = false;
        let mut resting = false;
        for c in 0..changes {
            let j = self.changed_at[c];
            let o = ws.owned[j].expect("a changed stretch is an object's");
            let rec = self.owners.get_mut(&o).expect("an object seen");
            rec.changed(now, frame);
            let live = now < rec.live_until;
            ws.live[j] = live;
            let atom = &mut rec.atoms[ws.atoms[j]];
            atom.cmds.clear();
            atom.cmds.extend_from_slice(&cmds[ws.from[j]..ws.to[j]]);
            let run = ws.run_of[j];
            if run < 0 {
                outside = true;
                if !live {
                    resting = true;
                }
            } else {
                let run = run as usize;
                self.extent.of(cmds, ws.from[j], ws.to[j], &ws.clip, j * 4);
                if !self.run_moved[run] {
                    self.boxes[run].clear();
                }
                let (l, t, r, b) = (self.extent.l, self.extent.t, self.extent.r, self.extent.b);
                self.boxes[run].join(l, t, r, b);
                self.run_moved[run] = true;
            }
        }
        ws.moved(resting, now);
        if outside {
            ws.version = next_id();
        }
        ws.cmds = Some(cmds.clone());
        let runs = self.moved_runs(ws, &plan);
        let next = match runs {
            Some(runs) => WindowPlan {
                id: ws.id,
                version: ws.version,
                runs,
            },
            None if outside => WindowPlan {
                id: ws.id,
                version: ws.version,
                runs: plan.runs,
            },
            None => plan,
        };
        ws.plan = Some(next.clone());
        next
    }

    /// `plan`'s runs with those that changed at new versions, grown by what changed in them; `None` when none did.
    fn moved_runs(&mut self, ws: &WindowState, plan: &WindowPlan) -> Option<Vec<LiveRun>> {
        let mut runs: Option<Vec<LiveRun>> = None;
        for r in 0..plan.runs.len() {
            if !self.run_moved[r] {
                continue;
            }
            let state = self
                .runs
                .get_mut(&ws.run_states[r])
                .expect("a run of the plan before");
            let old = &plan.runs[r];
            let bounds = self.boxes[r].grow(state.bounds, ws.w, ws.h);
            state.bounds = bounds;
            state.version = next_id();
            let list = runs.get_or_insert_with(|| plan.runs.clone());
            list[r] = LiveRun {
                id: old.id,
                version: state.version,
                from: old.from,
                to: old.to,
                bounds: bounds.unwrap_or(old.bounds),
            };
        }
        runs
    }
}

/// Whether `item` lies so far from the run gathered so far that the box around both would be mostly empty.
fn apart(run: &RunBuild, item: &Item) -> bool {
    if item.area() == 0 || run.area() == 0 {
        return false;
    }
    let e = &run.extent;
    let w = e.r.max(item.r) - e.l.min(item.l);
    let h = e.b.max(item.b) - e.t.min(item.t);
    w as i64 * h as i64 > 2 * (run.areas + item.area())
}

/// Whether commands `from` to `to` draw the same in `prev` and `cmds`, differ, or differ in kind.
fn compare(prev: &[DrawCmd], cmds: &[DrawCmd], from: usize, to: usize) -> u8 {
    let mut k = from;
    while k < to && cmds[k].draws_same_as(&prev[k]) {
        k += 1;
    }
    if k == to {
        return SAME;
    }
    while k < to {
        if std::mem::discriminant(&cmds[k]) != std::mem::discriminant(&prev[k]) {
            return KIND;
        }
        k += 1;
    }
    CHANGED
}

fn same_cmds(kept: &[DrawCmd], cmds: &[DrawCmd]) -> bool {
    kept.len() == cmds.len() && kept.iter().zip(cmds).all(|(a, b)| a.draws_same_as(b))
}

fn same_sequence(a: &[Seq], b: &[Seq]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| match (x, y) {
            (Seq::Cmd(c), Seq::Cmd(d)) => c.draws_same_as(d),
            (Seq::Owner(c), Seq::Owner(d)) => c == d,
            (Seq::Run(c), Seq::Run(d)) => c == d,
            _ => false,
        })
}

#[cfg(test)]
mod tests;
