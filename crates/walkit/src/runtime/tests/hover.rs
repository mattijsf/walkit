// SPDX-License-Identifier: GPL-3.0-or-later

//! A mouse over a modern skin with no button held, and the app losing the keyboard.

use std::cell::Cell;
use std::rc::Rc;

use super::{runtime, QuietHost, SKIN_XML};
use crate::files::MemoryFiles;
use crate::host::{Clock, SkinSetup};
use crate::player::FakePlayer;
use crate::runtime::SkinRuntime;

const PLAYER_XML: &str = r#"
<elements>
  <bitmap id="up" file="up.png"/>
  <bitmap id="over" file="over.png"/>
  <bitmap id="down" file="down.png"/>
  <bitmap id="thumb" file="thumb.png"/>
  <bitmap id="hoverthumb" file="hoverthumb.png"/>
</elements>
<container id="main" name="t" default_visible="1"><layout id="normal" w="100" h="50">
<button id="play" x="10" y="10" w="20" h="20" action="PLAY" image="up" hoverimage="over"
  downimage="down"/>
<button id="stop" x="50" y="10" w="20" h="20" action="STOP" image="up" inactivealpha="128"
  tooltip="Stop"/>
<slider id="vol" x="10" y="35" w="80" h="10" action="volume" thumb="thumb" hoverthumb="hoverthumb"/>
</layout></container>"#;

fn loaded() -> SkinRuntime {
    let mut rt = runtime(PLAYER_XML);
    rt.load();
    rt.ensure_layout();
    rt
}

/// The pictures the main window draws, by their bitmap ids.
fn pictures(rt: &mut SkinRuntime) -> String {
    format!("{:?}", rt.draw_windows())
}

#[test]
fn a_button_shows_its_hover_picture_while_the_pointer_is_over_it() {
    let mut rt = loaded();
    assert!(!pictures(&mut rt).contains("over.png"));
    rt.pointer_hover(15, 15);
    assert!(pictures(&mut rt).contains("over.png"));
    // pressed it shows its down picture, and over it again after the release
    rt.pointer_down(15, 15, 1);
    let held = pictures(&mut rt);
    assert!(held.contains("down.png") && !held.contains("over.png"));
    rt.pointer_up(15, 15);
    assert!(pictures(&mut rt).contains("over.png"));
    // off it, onto a button with no hover picture, and out of the window
    rt.pointer_hover(55, 15);
    assert!(!pictures(&mut rt).contains("over.png"));
    rt.pointer_hover(15, 15);
    rt.pointer_leave();
    assert!(!pictures(&mut rt).contains("over.png"));
}

#[test]
fn a_release_away_from_the_button_leaves_it_unlit() {
    let mut rt = loaded();
    rt.pointer_hover(15, 15);
    rt.pointer_down(15, 15, 1);
    // held and off the button it shows itself up, as Winamp's does, and down again back on it
    rt.pointer_move(55, 15);
    let off = pictures(&mut rt);
    assert!(!off.contains("down.png") && !off.contains("over.png"));
    rt.pointer_move(15, 15);
    assert!(pictures(&mut rt).contains("down.png"));
    rt.pointer_move(55, 15);
    rt.pointer_up(55, 15);
    assert!(!pictures(&mut rt).contains("over.png"));
}

#[test]
fn the_object_the_pointer_came_onto_shows_it_from_the_next_move() {
    let mut rt = loaded();
    rt.pointer_hover(55, 15);
    // from the stop button onto play: this move is the stop button's last
    rt.pointer_hover(15, 15);
    assert!(!pictures(&mut rt).contains("over.png"));
    rt.pointer_hover(16, 15);
    assert!(pictures(&mut rt).contains("over.png"));
}

#[test]
fn a_slider_shows_its_hover_thumb_while_the_pointer_is_over_it() {
    let mut rt = loaded();
    assert!(!pictures(&mut rt).contains("hoverthumb.png"));
    rt.pointer_hover(50, 40);
    assert!(pictures(&mut rt).contains("hoverthumb.png"));
    rt.pointer_leave();
    assert!(!pictures(&mut rt).contains("hoverthumb.png"));
}

#[test]
fn a_touch_with_no_hover_shows_no_hover_picture() {
    let mut rt = loaded();
    rt.pointer_down(15, 15, 1);
    rt.pointer_up(15, 15);
    assert!(!pictures(&mut rt).contains("over.png"));
}

#[test]
fn an_object_paints_with_its_inactive_alpha_while_another_app_has_the_keys() {
    let mut rt = loaded();
    let lit = pictures(&mut rt);
    rt.set_app_active(false);
    let behind = pictures(&mut rt);
    assert_ne!(lit, behind);
    assert!(behind.contains("alpha: 128"));
    rt.set_app_active(true);
    assert_eq!(pictures(&mut rt), lit);
}

/// [`loaded`] on a clock the test moves, in ms.
fn on_clock() -> (SkinRuntime, Rc<Cell<i64>>) {
    let time = Rc::new(Cell::new(1000));
    let mut files = MemoryFiles::new();
    files.insert("skin.xml", SKIN_XML.as_bytes());
    files.insert("player.xml", PLAYER_XML.as_bytes());
    let at = time.clone();
    let clock: Clock = Rc::new(move || at.get());
    let mut rt = SkinRuntime::new(
        Box::new(files),
        Box::new(QuietHost),
        Box::new(FakePlayer::new(clock.clone())),
        SkinSetup::default(),
        clock,
    );
    rt.load();
    rt.ensure_layout();
    (rt, time)
}

#[test]
fn a_tooltip_shows_after_350_ms_over_its_object_for_3_seconds() {
    let (mut rt, time) = on_clock();
    rt.pointer_hover(55, 15);
    assert_eq!(rt.tooltip(), None);
    assert_eq!(
        rt.next_due_at(),
        Some(1350),
        "the host is woken when it shows"
    );
    time.set(1349);
    assert_eq!(rt.tooltip(), None);
    time.set(1350);
    assert_eq!(rt.tooltip().as_deref(), Some("Stop"));
    assert_eq!(rt.next_due_at(), Some(4350), "and when it goes");
    time.set(4349);
    assert_eq!(rt.tooltip().as_deref(), Some("Stop"));
    time.set(4350);
    assert_eq!(rt.tooltip(), None);
    // moving over the same object does not show it again
    rt.pointer_hover(56, 15);
    time.set(5000);
    assert_eq!(rt.tooltip(), None);
}

#[test]
fn a_press_or_leaving_hides_the_tooltip_until_the_mouse_comes_onto_an_object_again() {
    let (mut rt, time) = on_clock();
    rt.pointer_hover(55, 15);
    time.set(1500);
    assert!(rt.tooltip().is_some());
    rt.pointer_down(55, 15, 1);
    assert_eq!(rt.tooltip(), None);
    rt.pointer_up(55, 15);
    time.set(1600);
    assert_eq!(rt.tooltip(), None, "still over it after the release");
    // off and back on: shown again from 350 ms later
    rt.pointer_leave();
    rt.pointer_hover(55, 15);
    time.set(1950);
    assert_eq!(rt.tooltip().as_deref(), Some("Stop"));
    rt.pointer_leave();
    assert_eq!(rt.tooltip(), None);
    // an object with no tooltip shows none
    rt.pointer_hover(15, 15);
    time.set(3000);
    assert_eq!(rt.tooltip(), None);
}
