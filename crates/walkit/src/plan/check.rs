// SPDX-License-Identifier: GPL-3.0-or-later

//! What every plan must hold, checked in tests: runs in order, apart and inside the list; no
//! window shape in a run and its clips paired inside it; bounds inside the window that hold
//! every pixel a run's commands can draw through the clips in force there. Boxes and clips are
//! taken as the platform's canvas takes them: with a right or bottom before the left or top,
//! they cover what lies between; a picture or pixels with such a destination draw nothing. And
//! a renderer that keeps each window and each run while its version holds ([`PlanCache`]) must
//! always see the same drawing for the same version, also across frames it never saw.

use std::collections::HashMap;

use crate::draw::{DrawCmd, Rect};

use super::{LiveRun, WindowPlan};

/// Panics unless `plan` holds its promises for a window of `rect`'s size drawing `cmds`.
pub fn check_plan(cmds: &[DrawCmd], rect: Rect, plan: &WindowPlan, what: &str) {
    let mut prev = 0;
    for run in &plan.runs {
        assert!(
            run.from >= prev && run.from < run.to && run.to <= cmds.len(),
            "{what}: run {}..{} after {prev} in {}",
            run.from,
            run.to,
            cmds.len()
        );
        prev = run.to;
        let b = run.bounds;
        assert!(
            b.left >= 0 && b.top >= 0 && b.right() <= rect.width && b.bottom() <= rect.height,
            "{what}: bounds {b:?} in {}x{}",
            rect.width,
            rect.height
        );
        assert!(b.width > 0 && b.height > 0, "{what}: bounds {b:?} empty");
        check_paired(cmds, run, what);
    }
    // the clip in force at each command outside the runs, and each run's commands against its bounds
    let mut clips = vec![sorted(0, 0, rect.width, rect.height)];
    let mut i = 0;
    let mut next = 0;
    while i < cmds.len() {
        if let Some(run) = plan.runs.get(next) {
            if i == run.from {
                check_bounds(cmds, run, clips.last().copied().expect("a clip"), what);
                i = run.to;
                next += 1;
                continue;
            }
        }
        match &cmds[i] {
            DrawCmd::WindowStart(region) => {
                let b = region.bounds().unwrap_or(Rect::new(0, 0, 1, 1));
                let top = clips.last().copied().expect("a clip");
                clips.push(intersect(top, 0, 0, b.right(), b.bottom()));
            }
            DrawCmd::Clip(r) => {
                let top = clips.last().copied().expect("a clip");
                clips.push(intersect(top, r.left, r.top, r.right(), r.bottom()));
            }
            DrawCmd::WindowEnd | DrawCmd::Unclip => {
                clips.pop();
            }
            _ => {}
        }
        i += 1;
    }
}

/// A run holds no window shape, and closes every clip it opens.
fn check_paired(cmds: &[DrawCmd], run: &LiveRun, what: &str) {
    let mut depth = 0i32;
    for (i, cmd) in cmds.iter().enumerate().take(run.to).skip(run.from) {
        match cmd {
            DrawCmd::WindowStart(_) | DrawCmd::WindowEnd => panic!("{what}: shape in run at {i}"),
            DrawCmd::Clip(_) => depth += 1,
            DrawCmd::Unclip => depth -= 1,
            _ => {}
        }
        assert!(
            depth >= 0,
            "{what}: clip closed before it opened in run at {i}"
        );
    }
    assert_eq!(depth, 0, "{what}: clips open at the end of the run");
}

/// Every pixel `run`'s commands draw through `outer` and the clips they open lies inside its bounds.
fn check_bounds(cmds: &[DrawCmd], run: &LiveRun, outer: [i32; 4], what: &str) {
    let mut inner = vec![outer];
    for (k, cmd) in cmds.iter().enumerate().take(run.to).skip(run.from) {
        if let DrawCmd::Clip(r) = cmd {
            let top = inner.last().copied().expect("a clip");
            inner.push(intersect(top, r.left, r.top, r.right(), r.bottom()));
        }
        if matches!(cmd, DrawCmd::Unclip) {
            inner.pop();
        }
        let Some(e) = extent_of(cmd) else { continue };
        let top = inner.last().copied().expect("a clip");
        let d = intersect(top, e.left, e.top, e.right(), e.bottom());
        if d[2] <= d[0] || d[3] <= d[1] {
            continue;
        }
        let b = run.bounds;
        assert!(
            d[0] >= b.left && d[1] >= b.top && d[2] <= b.right() && d[3] <= b.bottom(),
            "{what}: command {k} draws on {d:?} outside {b:?}"
        );
    }
}

/// The box from (`x0`, `y0`) to (`x1`, `y1`) as left, top, right, bottom, whichever corner comes first.
fn sorted(x0: i32, y0: i32, x1: i32, y1: i32) -> [i32; 4] {
    [x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)]
}

/// Clip `c` cut down by the box from (`x0`, `y0`) to (`x1`, `y1`), whichever corner comes first.
fn intersect(c: [i32; 4], x0: i32, y0: i32, x1: i32, y1: i32) -> [i32; 4] {
    let s = sorted(x0, y0, x1, y1);
    [
        c[0].max(s[0]),
        c[1].max(s[1]),
        c[2].min(s[2]),
        c[3].min(s[3]),
    ]
}

fn extent_of(c: &DrawCmd) -> Option<Rect> {
    match c {
        DrawCmd::Image { dst, .. } if dst.width > 0 && dst.height > 0 => Some(*dst),
        DrawCmd::Pixels { rect, .. } if rect.width > 0 && rect.height > 0 => Some(*rect),
        DrawCmd::Fill { rect, .. } => Some(*rect),
        DrawCmd::Text(t) => Some(t.rect),
        DrawCmd::BitmapText(t) => Some(t.rect),
        _ => None,
    }
}

/// What a window's layer draws: its size, its commands outside runs and, in their places, the runs' ids.
#[derive(Clone)]
enum Content {
    Size(i32, i32),
    Cmd(DrawCmd),
    Run(u64),
}

fn same_content(a: &[Content], b: &[Content]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| match (x, y) {
            (Content::Size(w, h), Content::Size(w2, h2)) => w == w2 && h == h2,
            (Content::Cmd(c), Content::Cmd(d)) => c.draws_same_as(d),
            (Content::Run(r), Content::Run(s)) => r == s,
            _ => false,
        })
}

/// A renderer that keeps each window and each run while its version holds, as a host's layers
/// do, and checks that what it keeps draws the same as the frame it is handed: the same version
/// must always mean the same drawing, also across frames it never saw.
#[derive(Default)]
pub struct PlanCache {
    windows: HashMap<u64, (u64, Vec<Content>)>,
    runs: HashMap<u64, (u64, Vec<DrawCmd>)>,
    /// Windows recorded again, over all frames handed in.
    pub window_records: u64,
    /// Runs recorded again, over all frames handed in.
    pub run_records: u64,
}

impl PlanCache {
    /// Checks a frame: each window's commands with its rect, and the plans in the same order.
    pub fn frame(&mut self, stack: &[(Rect, &[DrawCmd])], plans: &[WindowPlan], what: &str) {
        assert_eq!(stack.len(), plans.len(), "{what}: a plan for each window");
        for (i, plan) in plans.iter().enumerate() {
            let (rect, cmds) = stack[i];
            check_plan(cmds, rect, plan, &format!("{what} window {i}"));
            for run in &plan.runs {
                let drawn = &cmds[run.from..run.to];
                match self.runs.get(&run.id) {
                    Some((version, kept)) if *version == run.version => {
                        assert!(
                            kept.len() == drawn.len()
                                && kept.iter().zip(drawn).all(|(a, b)| a.draws_same_as(b)),
                            "{what}: run {} kept at version {} draws differently",
                            run.id,
                            run.version
                        );
                    }
                    _ => {
                        self.runs.insert(run.id, (run.version, drawn.to_vec()));
                        self.run_records += 1;
                    }
                }
            }
            let content = window_content(rect, cmds, plan);
            match self.windows.get(&plan.id) {
                Some((version, kept)) if *version == plan.version => {
                    assert!(
                        same_content(kept, &content),
                        "{what}: window {} kept at version {} draws differently",
                        plan.id,
                        plan.version
                    );
                }
                _ => {
                    self.windows.insert(plan.id, (plan.version, content));
                    self.window_records += 1;
                }
            }
        }
    }
}

fn window_content(rect: Rect, cmds: &[DrawCmd], plan: &WindowPlan) -> Vec<Content> {
    let mut out = vec![Content::Size(rect.width, rect.height)];
    let mut i = 0;
    let mut next = 0;
    while i < cmds.len() {
        if let Some(run) = plan.runs.get(next) {
            if i == run.from {
                out.push(Content::Run(run.id));
                i = run.to;
                next += 1;
                continue;
            }
        }
        out.push(Content::Cmd(cmds[i].clone()));
        i += 1;
    }
    out
}
