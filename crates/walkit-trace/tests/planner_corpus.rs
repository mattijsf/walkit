// SPDX-License-Identifier: GPL-3.0-or-later

//! The planner on the corpus skins (see `named_skins.rs` for where they are; without them
//! nothing is checked), playing on a stepped clock, then stopped, left resting past the live
//! hold and played again: every plan holds its promises, and a renderer keeping windows and
//! runs by version never shows a stale picture. Once a skin has played a while no window is
//! recorded again; the skins that draw the most keep their moving parts apart, and those runs
//! end once their objects rest.

#![cfg(feature = "native")]

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use walkit::draw::{DrawCmd, Rect};
use walkit::files::SkinFiles;
use walkit::host::{Clock, SkinSetup};
use walkit::plan::check::PlanCache;
use walkit::plan::{DrawPlanner, LIVE_HOLD_MS};
use walkit::player::FakePlayer;
use walkit::SkinRuntime;
use walkit_trace::archive::{decoder, read_wal};
use walkit_trace::TraceHost;

const SKINS: [&str; 8] = [
    "WinampModern566.wal",
    "Bento.wal",
    "BigBento.wal",
    "CornerAmp_Redux.wal",
    "MMD3.wal",
    "DeClassified.wal",
    "Winamp3x.wal",
    "Winamp2000SP4.wal",
];

/// The skins whose moving parts are kept apart while they play: the three that draw the most.
const KEPT_APART: [&str; 3] = ["Bento.wal", "BigBento.wal", "MMD3.wal"];
const START: i64 = 1_792_324_800_000;

/// The phone's size in skin pixels, as a full-screen Bento has it.
const W: i32 = 633;
const H: i32 = 1258;
const FRAME: i64 = 16;
const SETTLE_FRAMES: usize = 120;
const PLAY_MS: i64 = 40_000;
const STEADY_MS: i64 = 15_000;
const STOP_MS: i64 = 2_000;

/// The skin's files, read by the host and the engine both.
struct Shared(Rc<dyn SkinFiles>);

impl SkinFiles for Shared {
    fn read(&self, path: &str) -> Option<Rc<[u8]>> {
        self.0.read(path)
    }

    fn names(&self) -> Vec<String> {
        self.0.names()
    }
}

fn corpus() -> Option<PathBuf> {
    let dir = match std::env::var_os("WALKIT_CORPUS_SKINS") {
        Some(d) => PathBuf::from(d),
        None => {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../andamp/app/src/test/corpus")
        }
    };
    if dir.is_dir() {
        Some(dir)
    } else {
        eprintln!("no corpus at {}: nothing checked", dir.display());
        None
    }
}

/// The stepped clock every runtime and player of a run reads.
type Now = Rc<RefCell<i64>>;

/// A runtime on `name` at the phone's size, its player on the clock `now`, loaded.
fn runtime(dir: &Path, name: &str, now: &Now) -> SkinRuntime {
    let path = dir.join(name);
    let files: Rc<dyn SkinFiles> =
        Rc::new(read_wal(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())));
    let tick = now.clone();
    let clock: Clock = Rc::new(move || *tick.borrow());
    let host = TraceHost::new(
        files.clone(),
        Rc::new(RefCell::new(String::new())),
        decoder(),
        None,
    );
    let mut player = FakePlayer::new(clock.clone());
    player.length_ms = 600_000;
    player.title = "Andamp - a title long enough to scroll in every ticker".into();
    let mut rt = SkinRuntime::new(
        Box::new(Shared(files)),
        Box::new(host),
        Box::new(player),
        SkinSetup::default(),
        clock,
    );
    rt.load();
    // a window that resizes both ways fills the phone, as the app fits Bento
    if name.contains("Bento") {
        if let Some(main) = rt.main_window() {
            rt.resize_window(main, W, H);
        }
    }
    rt.set_view_size(Some((W, H)));
    rt
}

/// A skin's runtime planned frame by frame, checked each frame.
struct Run {
    name: String,
    rt: SkinRuntime,
    planner: DrawPlanner,
    cache: PlanCache,
    runs_ended: u64,
    frames: u64,
    last_runs: HashSet<u64>,
}

impl Run {
    /// Ticks the runtime at `t`; when it changed, or with `force`, plans and checks a frame
    /// and gives the number of runs in it.
    fn frame(&mut self, t: i64, force: bool) -> Option<usize> {
        let at = self.frames;
        self.frames += 1;
        let changed = self.rt.tick();
        if !changed && !force && at > 0 {
            return None;
        }
        let ids = self.rt.window_ids();
        let moving = self.rt.take_tick_changed();
        let drawn = self.rt.draw_windows_owned();
        let planned = self.planner.plan(&ids, drawn, &moving, t);
        let stack: Vec<(Rect, &[DrawCmd])> =
            planned.iter().map(|w| (w.rect, &w.cmds[..])).collect();
        let plans: Vec<_> = planned.iter().map(|w| w.plan.clone()).collect();
        self.cache
            .frame(&stack, &plans, &format!("{} frame {at}", self.name));
        let ids: HashSet<u64> = plans
            .iter()
            .flat_map(|p| p.runs.iter().map(|r| r.id))
            .collect();
        self.runs_ended += self.last_runs.iter().filter(|r| !ids.contains(r)).count() as u64;
        self.last_runs = ids;
        Some(plans.iter().map(|p| p.runs.len()).sum())
    }
}

/// One skin playing: `PLAY_MS` of play, then Stop, a rest past the live hold, and play again.
fn play(dir: &Path, name: &str) {
    let now: Now = Rc::new(RefCell::new(START));
    let rt = runtime(dir, name, &now);
    let mut run = Run {
        name: name.to_string(),
        rt,
        planner: DrawPlanner::new(),
        cache: PlanCache::default(),
        runs_ended: 0,
        frames: 0,
        last_runs: HashSet::new(),
    };
    let step = |now: &Now| {
        *now.borrow_mut() += FRAME;
        *now.borrow()
    };
    for _ in 0..SETTLE_FRAMES {
        step(&now);
        run.rt.tick();
    }
    run.rt.player().play();
    let playing = *now.borrow();
    let mut steady_records = None;
    let mut steady_runs = 0;
    while *now.borrow() < playing + PLAY_MS {
        let t = step(&now);
        // the last seconds of play: objects that move turned live long before
        let steady = t >= playing + PLAY_MS - STEADY_MS;
        if steady && steady_records.is_none() {
            steady_records = Some(run.cache.window_records);
        }
        let Some(runs) = run.frame(t, false) else {
            continue;
        };
        if steady {
            steady_runs = steady_runs.max(runs);
        }
    }
    assert_eq!(
        steady_records,
        Some(run.cache.window_records),
        "{name}: no window recorded again in the last {} s of play",
        STEADY_MS / 1000
    );
    if KEPT_APART.contains(&name) {
        assert!(
            steady_runs > 0,
            "{name} keeps moving parts apart while it plays"
        );
    }
    run.rt.player().stop();
    let stopped = *now.borrow();
    while *now.borrow() < stopped + STOP_MS {
        let t = step(&now);
        run.frame(t, false);
    }
    // nothing drawn for longer than the live hold, then a frame, then play again
    *now.borrow_mut() += LIVE_HOLD_MS + FRAME;
    let t = *now.borrow();
    run.frame(t, true);
    run.rt.player().play();
    let again = *now.borrow();
    while *now.borrow() < again + STOP_MS {
        let t = step(&now);
        run.frame(t, false);
    }
    if KEPT_APART.contains(&name) {
        assert!(
            run.runs_ended > 0,
            "{name}: runs end once their objects rest"
        );
    }
}

#[test]
fn the_corpus_skins_keep_the_plans_promises_while_playing() {
    let Some(dir) = corpus() else { return };
    for name in SKINS {
        play(&dir, name);
    }
}
