// SPDX-License-Identifier: GPL-3.0-or-later

//! How the planner keeps a window's moving objects apart from the rest, and when each version
//! moves; the reference engine's planner tests, on frames written in place.

use std::rc::Rc;

use super::check::PlanCache;
use super::*;
use crate::draw::{Align, BitmapDef, Region, Rgb, TextCmd, VAlign};

const LAYOUT: ObjId = 1;
const GROUP: ObjId = 2;
const BUTTON: ObjId = 3;
const TIME: ObjId = 4;
const VIS: ObjId = 5;
const LABEL: ObjId = 6;
const TICKER: ObjId = 7;
const LIST: ObjId = 8;
const KEY: WindowId = WindowId(1);
const OTHER: WindowId = WindowId(2);
const WINDOW: Rect = Rect::new(0, 0, 200, 100);

/// A left-aligned run of text in `r`, shifted left by `scroll` as a ticker is, placed as the engine places it.
fn text(s: &str, r: Rect, scroll: i32) -> DrawCmd {
    DrawCmd::Text(Box::new(TextCmd {
        text: s.to_string(),
        font: None,
        size: 10,
        color: Rgb(0xFFFFFF),
        rect: r,
        align: Align::Left,
        scroll,
        alpha: 255,
        valign: VAlign::Top,
        bold: false,
        shadow: None,
        shadow_dx: 1,
        shadow_dy: 1,
        x: (r.left - scroll) as f32,
        baseline: r.top as f32 + 8.0,
    }))
}

fn image(r: Rect) -> DrawCmd {
    DrawCmd::image(
        BitmapDef::new("bg", "bg.png", 0, 0, Some(r.width), Some(r.height), None),
        r,
        255,
    )
}

fn pixels(r: Rect, argb: &[u32]) -> DrawCmd {
    DrawCmd::Pixels {
        rect: r,
        w: 2,
        h: 2,
        argb: Rc::from(argb),
        alpha: 255,
        stable: false,
    }
}

/// A window as a test shows it, each frame drawn with fresh commands; planned and checked.
struct Window {
    planner: DrawPlanner,
    cache: PlanCache,
    now: i64,
    time_text: String,
    vis_pixels: Vec<u32>,
    label_text: String,
    ticker_scroll: i32,
    list_row: i32,
    shape: Region,
}

impl Window {
    fn new() -> Window {
        Window {
            planner: DrawPlanner::new(),
            cache: PlanCache::default(),
            now: 1_000_000,
            time_text: "0:00".into(),
            vis_pixels: vec![1, 2, 3, 4],
            label_text: "label".into(),
            ticker_scroll: 0,
            list_row: 0,
            shape: Region::Box(Rect::new(0, 0, 200, 100)),
        }
    }

    /// The window's commands as the runtime would draw it now, with who drew each.
    fn cmds(&self) -> (Vec<DrawCmd>, Vec<Option<ObjId>>) {
        let row = self.list_row;
        let list: Vec<(DrawCmd, Option<ObjId>)> = vec![
            (DrawCmd::WindowStart(self.shape.clone()), Some(LAYOUT)),
            (image(Rect::new(0, 0, 200, 100)), Some(LAYOUT)),
            (DrawCmd::Clip(Rect::new(0, 0, 200, 100)), Some(GROUP)),
            (image(Rect::new(10, 10, 20, 10)), Some(BUTTON)),
            (
                text(&self.time_text, Rect::new(50, 10, 40, 10), 0),
                Some(TIME),
            ),
            (
                pixels(Rect::new(100, 10, 60, 20), &self.vis_pixels),
                Some(VIS),
            ),
            (
                text(&self.label_text, Rect::new(10, 50, 30, 10), 0),
                Some(LABEL),
            ),
            // a list clips its rows inside the list's drawing
            (DrawCmd::Clip(Rect::new(10, 62, 100, 12)), Some(LIST)),
            (
                text(&format!("row {row}"), Rect::new(10, 62 - row, 100, 10), 0),
                Some(LIST),
            ),
            (
                text(
                    &format!("row {}", row + 1),
                    Rect::new(10, 72 - row, 100, 10),
                    0,
                ),
                Some(LIST),
            ),
            (DrawCmd::Unclip, None),
            (
                text("ticker", Rect::new(50, 80, 140, 10), self.ticker_scroll),
                Some(TICKER),
            ),
            (DrawCmd::Unclip, None),
            (DrawCmd::WindowEnd, None),
        ];
        list.into_iter().unzip()
    }

    /// Plans the window as it is now, `ms` after the frame before, and checks the plan.
    fn frame_moving(&mut self, ms: i64, moving: &[ObjId]) -> WindowPlan {
        self.now += ms;
        let (cmds, owners) = self.cmds();
        let plan = self
            .planner
            .plan_window(KEY, WINDOW, &cmds, &owners, self.now, moving);
        let what = format!("at {}", self.now);
        self.cache
            .frame(&[(WINDOW, &cmds)], std::slice::from_ref(&plan), &what);
        plan
    }

    fn frame(&mut self) -> WindowPlan {
        self.frame_moving(16, &[])
    }

    fn frame_after(&mut self, ms: i64) -> WindowPlan {
        self.frame_moving(ms, &[])
    }

    /// The objects each run holds, by the commands it spans.
    fn held(&self, plan: &WindowPlan) -> Vec<Vec<ObjId>> {
        let (_, owners) = self.cmds();
        plan.runs
            .iter()
            .map(|r| {
                let mut v: Vec<ObjId> = owners[r.from..r.to].iter().flatten().copied().collect();
                v.dedup();
                v
            })
            .collect()
    }
}

#[test]
fn an_unchanged_frame_keeps_every_version() {
    let mut w = Window::new();
    let first = w.frame();
    for _ in 0..10 {
        let plan = w.frame();
        assert_eq!(first.id, plan.id);
        assert_eq!(first.version, plan.version);
        assert!(plan.runs.is_empty());
    }
    assert_eq!(1, w.cache.window_records);
}

#[test]
fn a_ticker_that_steps_every_frame_gets_a_separate_run_and_only_that_run_moves() {
    let mut w = Window::new();
    w.frame();
    w.ticker_scroll += 1;
    w.frame();
    w.ticker_scroll += 1;
    let live = w.frame();
    assert_eq!(vec![vec![TICKER]], w.held(&live));
    let records = w.cache.window_records;
    for _ in 0..20 {
        w.ticker_scroll += 1;
        let plan = w.frame();
        assert_eq!(live.version, plan.version);
        assert_eq!(live.runs[0].id, plan.runs[0].id);
        assert_ne!(live.runs[0].version, plan.runs[0].version);
    }
    assert_eq!(
        records, w.cache.window_records,
        "the window is not recorded again"
    );
}

#[test]
fn a_one_off_change_moves_the_windows_version_and_makes_nothing_live() {
    let mut w = Window::new();
    let before = w.frame();
    w.label_text = "other".into();
    let changed = w.frame();
    assert_ne!(before.version, changed.version);
    assert!(changed.runs.is_empty());
    for _ in 0..5 {
        assert_eq!(changed.version, w.frame().version);
    }
    assert!(w.frame().runs.is_empty());
}

#[test]
fn a_second_change_within_the_gap_makes_an_object_live_and_it_rests_after_the_hold() {
    let mut w = Window::new();
    w.frame();
    w.time_text = "0:01".into();
    assert!(w.frame_after(1_000).runs.is_empty());
    w.time_text = "0:02".into();
    let live = w.frame_after(LIVE_GAP_MS);
    assert_eq!(vec![vec![TIME]], w.held(&live));
    // still live while it holds, though it did not change
    assert_eq!(
        live.runs[0].id,
        w.frame_after(LIVE_HOLD_MS - 100).runs[0].id
    );
    let rested = w.frame_after(200);
    assert!(rested.runs.is_empty());
    assert_ne!(live.version, rested.version);
}

#[test]
fn changes_further_apart_than_the_gap_never_make_an_object_live() {
    let mut w = Window::new();
    w.frame();
    for i in 0..4 {
        w.time_text = format!("0:0{i}");
        assert!(w.frame_after(LIVE_GAP_MS + 1).runs.is_empty());
    }
}

#[test]
fn an_object_the_tick_reports_moving_is_live_from_its_first_change() {
    let mut w = Window::new();
    w.frame();
    w.vis_pixels = vec![5, 6, 7, 8];
    let plan = w.frame_moving(16, &[VIS]);
    assert_eq!(vec![vec![VIS]], w.held(&plan));
}

#[test]
fn pixels_with_the_same_content_are_no_change() {
    let mut w = Window::new();
    let before = w.frame();
    w.vis_pixels = w.vis_pixels.clone();
    let after = w.frame();
    assert_eq!(before.version, after.version);
    assert!(after.runs.is_empty());
}

#[test]
fn neighboring_live_objects_share_a_run_and_one_far_away_gets_a_separate_one() {
    let mut w = Window::new();
    w.frame();
    for i in 0..3 {
        w.time_text = format!("0:1{i}");
        w.vis_pixels = vec![i, 0, 0, 0];
        w.ticker_scroll += 1;
        w.frame();
    }
    w.time_text = "0:20".into();
    w.vis_pixels = vec![9, 0, 0, 0];
    w.ticker_scroll += 1;
    let plan = w.frame();
    // the time and the visualizer sit side by side; the label between them and the ticker rests
    assert_eq!(vec![vec![TIME, VIS], vec![TICKER]], w.held(&plan));
}

#[test]
fn a_list_that_clips_its_rows_is_kept_apart_with_its_clip() {
    let mut w = Window::new();
    w.frame();
    for _ in 0..4 {
        w.list_row += 1;
        w.frame();
    }
    let plan = w.frame();
    assert_eq!(vec![vec![LIST]], w.held(&plan));
    let (c, _) = w.cmds();
    assert!(matches!(c[plan.runs[0].from], DrawCmd::Clip(_)));
    assert!(matches!(c[plan.runs[0].to - 1], DrawCmd::Unclip));
}

#[test]
fn a_runs_bounds_hold_what_it_draws_and_only_grow() {
    let mut w = Window::new();
    w.frame();
    let mut bounds = Vec::new();
    for _ in 0..6 {
        w.list_row += 1;
        bounds.extend(w.frame().runs.iter().map(|r| r.bounds));
    }
    for i in 1..bounds.len() {
        let a = bounds[i - 1];
        let b = bounds[i];
        assert!(
            b.left <= a.left
                && b.top <= a.top
                && b.right() >= a.right()
                && b.bottom() >= a.bottom(),
            "{b:?} holds {a:?}"
        );
    }
    // the rows scroll past the list's clip; the bounds stop at it
    assert!(bounds.last().expect("runs").top >= 62);
}

#[test]
fn a_window_whose_shape_keeps_changing_is_drawn_in_one_piece_until_it_rests() {
    let mut w = Window::new();
    w.frame();
    w.ticker_scroll += 1;
    w.frame();
    w.ticker_scroll += 1;
    assert_eq!(1, w.frame().runs.len());
    for i in 0..3 {
        w.shape = Region::Box(Rect::new(0, 0, 200, 90 + i));
        w.ticker_scroll += 1;
        w.frame();
    }
    w.ticker_scroll += 1;
    assert!(w.frame().runs.is_empty(), "flat");
    for _ in 0..10 {
        w.ticker_scroll += 1;
        assert!(w.frame_after(50).runs.is_empty(), "still flat");
    }
    w.ticker_scroll += 1;
    assert_eq!(
        1,
        w.frame_after(600).runs.len(),
        "kept apart again after a second at rest"
    );
}

#[test]
fn more_runs_than_allowed_or_runs_over_half_the_window_draw_the_window_in_one_piece() {
    let mut planner = DrawPlanner::new();
    let now = 1_000_000;
    let many: Vec<ObjId> = (100..=100 + MAX_RUNS).collect();
    let still: Vec<ObjId> = (200..=200 + MAX_RUNS).collect();
    let cmds = |step: i32| -> (Vec<DrawCmd>, Vec<Option<ObjId>>) {
        let mut list = vec![(image(Rect::new(0, 0, 200, 100)), Some(LAYOUT))];
        for i in 0..many.len() {
            let x = i as i32 * 14;
            list.push((
                text(&step.to_string(), Rect::new(x, 0, 7, 7), 0),
                Some(many[i]),
            ));
            list.push((text("x", Rect::new(x + 7, 0, 7, 7), 0), Some(still[i])));
        }
        list.into_iter().unzip()
    };
    let mut plan = None;
    for i in 0..4 {
        let (c, o) = cmds(i);
        plan = Some(planner.plan_window(KEY, WINDOW, &c, &o, now + i as i64 * 16, &[]));
    }
    assert!(
        plan.expect("a plan").runs.is_empty(),
        "one run each would be {}",
        many.len()
    );

    let big: ObjId = 300;
    let mut plan = None;
    for i in 0..4 {
        let c = vec![text(&i.to_string(), Rect::new(0, 0, 150, 80), 0)];
        plan = Some(planner.plan_window(OTHER, WINDOW, &c, &[Some(big)], now + i as i64 * 16, &[]));
    }
    assert!(
        plan.expect("a plan").runs.is_empty(),
        "one run over 60 % of the window"
    );
}

#[test]
fn boxes_and_clips_with_swapped_corners_count_where_the_canvas_draws_them() {
    let mut planner = DrawPlanner::new();
    let mut cache = PlanCache::default();
    let mut now = 1_000_000;
    let mut step = 0;
    // a group squeezed to a negative height clips from its bottom to its top, and its live
    // children draw a box and text whose corners are swapped too
    let cmds = |step: i32| -> (Vec<DrawCmd>, Vec<Option<ObjId>>) {
        vec![
            (image(Rect::new(0, 0, 200, 100)), Some(LAYOUT)),
            (DrawCmd::Clip(Rect::new(10, 80, 100, -40)), Some(GROUP)),
            (image(Rect::new(120, 10, 20, 10)), Some(BUTTON)),
            (
                DrawCmd::Fill {
                    rect: Rect::new(10, 70, 50, -20),
                    color: Rgb(step as u32),
                    alpha: 255,
                },
                Some(TIME),
            ),
            (
                text(&step.to_string(), Rect::new(60, 70, -20, -10), 0),
                Some(TICKER),
            ),
            (DrawCmd::Unclip, None),
        ]
        .into_iter()
        .unzip()
    };
    let (c, o) = cmds(step);
    let mut plan = planner.plan_window(OTHER, WINDOW, &c, &o, now, &[]);
    for _ in 0..3 {
        step += 1;
        now += 16;
        let (c, o) = cmds(step);
        plan = planner.plan_window(OTHER, WINDOW, &c, &o, now, &[TIME, TICKER]);
        cache.frame(
            &[(WINDOW, &c)],
            std::slice::from_ref(&plan),
            &format!("step {step}"),
        );
    }
    assert_eq!(
        Rect::new(10, 50, 50, 20),
        plan.runs[0].bounds,
        "the box and the text, inside the group's clip"
    );
}

#[test]
fn a_window_of_negative_size_is_drawn_in_one_piece() {
    let mut w = Window::new();
    let flat = Rect::new(0, 0, -200, -100);
    for _ in 0..4 {
        w.ticker_scroll += 1;
        w.now += 16;
        let (c, o) = w.cmds();
        let plan = w.planner.plan_window(OTHER, flat, &c, &o, w.now, &[TICKER]);
        assert!(plan.runs.is_empty());
    }
}

#[test]
fn a_window_that_changes_size_moves_its_version() {
    let mut w = Window::new();
    let before = w.frame();
    let (c, o) = w.cmds();
    let plan = w
        .planner
        .plan_window(KEY, Rect::new(0, 0, 200, 120), &c, &o, w.now + 16, &[]);
    assert_ne!(before.version, plan.version);
}

#[test]
fn a_renderer_that_sees_only_some_frames_never_shows_a_kept_drawing_that_changed() {
    let mut w = Window::new();
    let mut skipping = PlanCache::default();
    // a small linear congruential generator, so the run is the same each time
    let mut seed: u64 = 7;
    let mut next = |n: u64| -> u64 {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) % n
    };
    for it in 0..600 {
        if next(4) == 0 {
            w.time_text = format!("0:{}", next(60));
        }
        if next(2) == 0 {
            w.vis_pixels = vec![next(3) as u32, 0, 0, 0];
        }
        if next(3) == 0 {
            w.ticker_scroll += 1;
        }
        if next(40) == 0 {
            w.label_text = format!("label {}", next(3));
        }
        if next(25) == 0 {
            w.list_row = next(4) as i32;
        }
        if next(90) == 0 {
            w.shape = Region::Box(Rect::new(0, 0, 200, 90 + next(10) as i32));
        }
        w.now += 16 * (1 + next(8) as i64);
        let (c, o) = w.cmds();
        let moving: &[ObjId] = if next(2) == 0 { &[VIS] } else { &[] };
        let plan = w.planner.plan_window(KEY, WINDOW, &c, &o, w.now, moving);
        if next(3) != 0 {
            skipping.frame(
                &[(WINDOW, &c)],
                std::slice::from_ref(&plan),
                &format!("frame {it}"),
            );
        }
    }
    assert!(skipping.run_records > 0);
}
