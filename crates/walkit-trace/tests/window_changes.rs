// SPDX-License-Identifier: GPL-3.0-or-later

//! A host that draws only the windows `changed_windows` names ends every tick with the same
//! drawings as one that takes every window's drawing, on the example skins: playing, dragged,
//! hovered, stopped, with the playlist changed under it.

#![cfg(feature = "native")]

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use walkit::draw::{DrawCmd, Rect};
use walkit::files::SkinFiles;
use walkit::host::{Clock, SkinSetup, WindowId};
use walkit::player::{FakePlayer, Track};
use walkit::runtime::WindowChanges;
use walkit::SkinRuntime;
use walkit_trace::archive::{decoder, fonts, read_wal};
use walkit_trace::TraceHost;

const START: i64 = 1_792_324_800_000;

/// Where the main window starts down the viewport, its left at 0.
const START_Y: i32 = 0;

struct Shared(Rc<dyn SkinFiles>);

impl SkinFiles for Shared {
    fn read(&self, path: &str) -> Option<Rc<[u8]>> {
        self.0.read(path)
    }

    fn names(&self) -> Vec<String> {
        self.0.names()
    }
}

fn skins() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/skins")
}

/// The example skin `file` with a playlist of 30 on `clock`, its windows set out apart.
fn runtime(file: &str, clock: &Clock, numbered: bool) -> SkinRuntime {
    runtime_playing(file, clock, numbered, 215_000)
}

/// [`runtime`] with a song `length_ms` long.
fn runtime_playing(file: &str, clock: &Clock, numbered: bool, length_ms: i32) -> SkinRuntime {
    let files: Rc<dyn SkinFiles> = Rc::new(read_wal(&skins().join(file)).expect("the skin"));
    let host = TraceHost::new(
        files.clone(),
        Rc::new(RefCell::new(String::new())),
        decoder(),
        fonts(&skins().join("fonts")),
    );
    let mut player = FakePlayer::new(clock.clone());
    player.numbered = numbered;
    player.length_ms = length_ms;
    player.playlist = (0..30)
        .map(|i| Track {
            title: format!("Song {i}"),
            length_ms: 200_000 + i * 1000,
        })
        .collect();
    let mut rt = SkinRuntime::new(
        Box::new(Shared(files)),
        Box::new(host),
        Box::new(player),
        SkinSetup {
            host_actions: true,
            ..SkinSetup::default()
        },
        clock.clone(),
    );
    rt.set_viewport(1200, 900);
    rt.load();
    rt.open_window("equalizer");
    rt.open_window("pledit");
    let mut presentation = rt.presentation();
    presentation.free_windows = true;
    presentation.viewport = (1200, 900);
    presentation.touch_slop = 0;
    rt.set_presentation(&presentation);
    rt
}

/// What a host shows: each window's place and drawing, bottom to top.
type Shown = Vec<(WindowId, Rect, Rc<Vec<DrawCmd>>)>;

/// A host that takes every window's drawing after a tick that changed something; how many
/// of them it drew, those with another list than before.
fn draw_all(
    rt: &mut SkinRuntime,
    kept: &mut HashMap<WindowId, Rc<Vec<DrawCmd>>>,
) -> (Shown, usize) {
    let mut drawn = 0;
    if rt.take_placed_only().is_none() {
        let ids = rt.drawn_window_ids();
        for (id, d) in ids.into_iter().zip(rt.draw_windows_owned()) {
            if kept.get(&id).is_none_or(|k| !Rc::ptr_eq(k, &d.cmds)) {
                drawn += 1;
            }
            kept.insert(id, d.cmds);
        }
    }
    (shown(rt, kept), drawn)
}

/// A host that draws only the windows that changed; how many it drew.
fn draw_changed(
    rt: &mut SkinRuntime,
    kept: &mut HashMap<WindowId, Rc<Vec<DrawCmd>>>,
    counts: &mut Counts,
) -> (Shown, usize) {
    let mut drawn = 0;
    match rt.changed_windows() {
        WindowChanges::Nothing => counts.nothing += 1,
        WindowChanges::Placed => {
            counts.placed += 1;
            assert!(rt.draw_changed_windows().is_empty());
        }
        changes => {
            let windows = rt.draw_changed_windows();
            if let WindowChanges::Windows(ids) = &changes {
                counts.some += 1;
                let got: Vec<WindowId> = windows.iter().map(|(id, _)| *id).collect();
                assert_eq!(&got, ids, "the windows drawn are the ones named");
            } else {
                counts.all += 1;
                assert_eq!(windows.len(), rt.drawn_window_ids().len());
            }
            for (id, d) in windows {
                drawn += 1;
                kept.insert(id, d.cmds);
            }
        }
    }
    (shown(rt, kept), drawn)
}

fn shown(rt: &mut SkinRuntime, kept: &HashMap<WindowId, Rc<Vec<DrawCmd>>>) -> Shown {
    rt.drawn_window_ids()
        .into_iter()
        .map(|id| (id, rt.window_rect(id), kept[&id].clone()))
        .collect()
}

#[derive(Default, Debug)]
struct Counts {
    nothing: usize,
    placed: usize,
    some: usize,
    all: usize,
}

/// Runs `file` through both hosts in step and checks they show the same after every tick.
fn same_drawings(file: &str) -> Counts {
    let now = Rc::new(Cell::new(START));
    let clock: Clock = {
        let now = now.clone();
        Rc::new(move || now.get())
    };
    // the reference host's player cannot tell when its playlist changed, as before versions
    let mut a = runtime(file, &clock, false);
    let mut b = runtime(file, &clock, true);
    let mut kept_a = HashMap::new();
    let mut kept_b = HashMap::new();
    let mut counts = Counts::default();
    let (mut drawn_a, mut drawn_b) = (0, 0);
    let main = a.main_window().expect("a main window");
    a.tick();
    assert_eq!(
        a.window_rect(main).top,
        START_Y,
        "{file}: the main window's place"
    );
    for step in 0..400 {
        now.set(now.get() + 25);
        let at = a.window_rect(main);
        // pressed on the title bar, then dragged 5 across and 3 down a step: playing, and
        // again once stopped, where the windows only move
        let (x, y) = (60, START_Y + 6);
        let k = step % 100 - 60;
        for rt in [&mut a, &mut b] {
            match step {
                1 => rt.player().play(),
                60 => rt.pointer_down(x, y, 1),
                61..=70 => rt.pointer_move(x + k * 5, y + k * 3),
                71 => rt.pointer_up(x + 50, y + 30),
                360 => rt.pointer_down(x + 50, y + 30, 2),
                361..=370 => rt.pointer_move(x + 50 + k * 5, y + 30 + k * 3),
                371 => rt.pointer_up(x + 100, y + 60),
                // hovered across the window
                100..=140 => rt.pointer_move(at.left + (step - 100) * 6, at.top + at.height / 2),
                // the playlist changes under it
                200 => rt.player().set_track("Another song", 123_000),
                300 => rt.player().stop(),
                _ => {}
            }
        }
        let ticked_a = a.tick();
        let ticked_b = b.tick();
        assert_eq!(ticked_a, ticked_b, "{file} step {step}: the ticks differ");
        if step == 0 || ticked_a {
            let (shown_a, n) = draw_all(&mut a, &mut kept_a);
            drawn_a += n;
            let (shown_b, n) = draw_changed(&mut b, &mut kept_b, &mut counts);
            drawn_b += n;
            assert_eq!(shown_a.len(), shown_b.len(), "{file} step {step}");
            for ((ia, ra, ca), (ib, rb, cb)) in shown_a.iter().zip(&shown_b) {
                assert_eq!((ia, ra), (ib, rb), "{file} step {step}: places differ");
                assert_eq!(
                    ca, cb,
                    "{file} step {step}: window {} draws otherwise",
                    ia.0
                );
            }
        } else {
            assert_eq!(
                b.changed_windows(),
                WindowChanges::Nothing,
                "{file} step {step}"
            );
        }
    }
    // the same windows draw either way: the engine kept the lists of the others already
    assert_eq!(
        drawn_b, drawn_a,
        "{file}: {drawn_b} windows drawn, {drawn_a} the other way"
    );
    counts
}

#[test]
fn a_classic_skin_draws_the_same_window_by_window() {
    let counts = same_drawings("walkit-classic.wsz");
    // playing, the main window alone draws again with every step of its clock
    assert!(counts.some > 100, "{counts:?}");
    assert!(counts.all > 0, "{counts:?}");
    assert!(counts.placed > 0, "{counts:?}");
}

#[test]
fn a_modern_skin_draws_the_same_window_by_window() {
    let counts = same_drawings("walkit-modern.wal");
    assert!(counts.some > 100, "{counts:?}");
    assert!(counts.all > 0, "{counts:?}");
    assert!(counts.placed > 0, "{counts:?}");
}

#[test]
fn nothing_changed_draws_nothing() {
    let clock: Clock = Rc::new(|| START);
    let mut rt = runtime("walkit-classic.wsz", &clock, true);
    rt.tick();
    assert_eq!(rt.changed_windows(), WindowChanges::All);
    assert_eq!(rt.draw_changed_windows().len(), rt.drawn_window_ids().len());
    // stopped, on a clock that stands: nothing moves
    assert!(!rt.tick());
    assert_eq!(rt.changed_windows(), WindowChanges::Nothing);
    assert!(rt.draw_changed_windows().is_empty());
}

#[test]
fn a_seek_bar_that_follows_the_song_draws_its_own_window_again() {
    let now = Rc::new(Cell::new(START));
    let clock: Clock = {
        let now = now.clone();
        Rc::new(move || now.get())
    };
    // a song of 10 seconds: the thumb moves with nearly every tick
    let mut rt = runtime_playing("walkit-modern.wal", &clock, true, 10_000);
    rt.player().play();
    rt.tick();
    rt.draw_changed_windows();
    let main = rt.main_window().expect("a main window");
    let mut followed = 0;
    for _ in 0..200 {
        now.set(now.get() + 25);
        rt.tick();
        let changes = rt.changed_windows();
        assert_ne!(
            changes,
            WindowChanges::All,
            "nothing but moving parts changed"
        );
        if let WindowChanges::Windows(ids) = &changes {
            assert_eq!(ids, &[main], "only the main window shows the song");
            followed += 1;
        }
        rt.draw_changed_windows();
    }
    assert!(followed > 150, "{followed} ticks drew the main window");
}
