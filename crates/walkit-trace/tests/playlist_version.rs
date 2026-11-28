// SPDX-License-Identifier: GPL-3.0-or-later

//! A player that numbers its playlist has it read once per change, not on every use: in a tick,
//! in the playlist's drawing and in a press on it. One that does not is read as before.

#![cfg(feature = "native")]

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;

use walkit::files::SkinFiles;
use walkit::host::{Clock, SkinSetup};
use walkit::player::{FakePlayer, Player, Status, Track};
use walkit::SkinRuntime;
use walkit_trace::archive::{decoder, fonts, read_wal};
use walkit_trace::TraceHost;

struct Shared(Rc<dyn SkinFiles>);

impl SkinFiles for Shared {
    fn read(&self, path: &str) -> Option<Rc<[u8]>> {
        self.0.read(path)
    }

    fn names(&self) -> Vec<String> {
        self.0.names()
    }
}

/// A fake player that counts the reads of its playlist.
struct Counted {
    inner: FakePlayer,
    reads: Rc<Cell<usize>>,
}

impl Player for Counted {
    fn tracks(&self) -> Vec<Track> {
        self.reads.set(self.reads.get() + 1);
        self.inner.tracks()
    }
    fn playlist_version(&self) -> Option<u64> {
        self.inner.playlist_version()
    }
    fn track_index(&self) -> i32 {
        self.inner.track_index()
    }
    fn play_track(&mut self, index: i32) {
        self.inner.play_track(index);
    }
    fn status(&self) -> Status {
        self.inner.status()
    }
    fn position_ms(&self) -> i32 {
        self.inner.position_ms()
    }
    fn length_ms(&self) -> i32 {
        self.inner.length_ms()
    }
    fn title(&self) -> String {
        self.inner.title()
    }
    fn set_track(&mut self, title: &str, length_ms: i32) {
        self.inner.set_track(title, length_ms);
    }
    fn set_silent(&mut self, silent: bool) {
        self.inner.set_silent(silent);
    }
    fn set_format(&mut self, kbps: i32, khz: i32, channels: i32) {
        self.inner.set_format(kbps, khz, channels);
    }
    fn bitrate_kbps(&self) -> i32 {
        self.inner.bitrate_kbps()
    }
    fn sample_rate_khz(&self) -> i32 {
        self.inner.sample_rate_khz()
    }
    fn channels(&self) -> i32 {
        self.inner.channels()
    }
    fn tags(&self) -> BTreeMap<String, String> {
        self.inner.tags()
    }
    fn decoder_name(&self) -> String {
        self.inner.decoder_name()
    }
    fn volume(&self) -> i32 {
        self.inner.volume()
    }
    fn set_volume(&mut self, v: i32) {
        self.inner.set_volume(v);
    }
    fn shuffle(&self) -> bool {
        self.inner.shuffle()
    }
    fn set_shuffle(&mut self, on: bool) {
        self.inner.set_shuffle(on);
    }
    fn repeat(&self) -> bool {
        self.inner.repeat()
    }
    fn set_repeat(&mut self, on: bool) {
        self.inner.set_repeat(on);
    }
    fn crossfade(&self) -> bool {
        self.inner.crossfade()
    }
    fn set_crossfade(&mut self, on: bool) {
        self.inner.set_crossfade(on);
    }
    fn eq_enabled(&self) -> bool {
        self.inner.eq_enabled()
    }
    fn set_eq_enabled(&mut self, on: bool) {
        self.inner.set_eq_enabled(on);
    }
    fn eq_bands(&self) -> Vec<i32> {
        self.inner.eq_bands()
    }
    fn eq_preamp(&self) -> i32 {
        self.inner.eq_preamp()
    }
    fn set_eq_preamp(&mut self, v: i32) {
        self.inner.set_eq_preamp(v);
    }
    fn pan(&self) -> i32 {
        self.inner.pan()
    }
    fn set_pan(&mut self, v: i32) {
        self.inner.set_pan(v);
    }
    fn play(&mut self) {
        self.inner.play();
    }
    fn pause(&mut self) {
        self.inner.pause();
    }
    fn stop(&mut self) {
        self.inner.stop();
    }
    fn next(&mut self) {
        self.inner.next();
    }
    fn previous(&mut self) {
        self.inner.previous();
    }
    fn eject(&mut self) {
        self.inner.eject();
    }
    fn seek_to(&mut self, ms: i32) {
        self.inner.seek_to(ms);
    }
    fn set_eq_band(&mut self, band: i32, value: i32) {
        self.inner.set_eq_band(band, value);
    }

    fn spectrum(&self) -> Vec<f32> {
        self.inner.spectrum()
    }
    fn waveform(&self) -> Vec<f32> {
        self.inner.waveform()
    }
    fn vu(&self) -> (i32, i32) {
        self.inner.vu()
    }
}

/// The example classic skin with its playlist open, playing 40 songs; the reads counter.
fn runtime(numbered: bool, clock: &Clock) -> (SkinRuntime, Rc<Cell<usize>>) {
    let skins = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/skins");
    let files: Rc<dyn SkinFiles> =
        Rc::new(read_wal(&skins.join("walkit-classic.wsz")).expect("the skin"));
    let host = TraceHost::new(
        files.clone(),
        Rc::new(RefCell::new(String::new())),
        decoder(),
        fonts(&skins.join("fonts")),
    );
    let mut inner = FakePlayer::new(clock.clone());
    inner.numbered = numbered;
    inner.playlist = (0..40)
        .map(|i| Track {
            title: format!("Song {i}"),
            length_ms: 200_000,
        })
        .collect();
    let reads = Rc::new(Cell::new(0));
    let player = Counted {
        inner,
        reads: reads.clone(),
    };
    let mut rt = SkinRuntime::new(
        Box::new(Shared(files)),
        Box::new(host),
        Box::new(player),
        SkinSetup::default(),
        clock.clone(),
    );
    rt.set_viewport(1200, 900);
    rt.load();
    rt.open_window("pledit");
    rt.tick();
    rt.draw_windows_owned();
    rt.player().play();
    (rt, reads)
}

/// The text the windows draw, all of it.
fn drawn_text(rt: &mut SkinRuntime) -> String {
    rt.draw_windows()
        .iter()
        .flat_map(|(_, cmds)| cmds.iter())
        .map(walkit::listing::line)
        .collect::<Vec<_>>()
        .join("\n")
}

/// 40 ticks of 25 ms, each drawn, with a scroll of the playlist among them.
fn play(rt: &mut SkinRuntime, now: &Cell<i64>) {
    for step in 0..40 {
        now.set(now.get() + 25);
        if step == 20 {
            rt.scroll_classic_playlist(3);
        }
        if rt.tick() {
            rt.draw_windows_owned();
        }
    }
}

#[test]
fn a_numbered_playlist_is_read_once_per_change() {
    let now = Rc::new(Cell::new(0));
    let clock: Clock = {
        let now = now.clone();
        Rc::new(move || now.get())
    };
    let (mut rt, reads) = runtime(true, &clock);
    let before = drawn_text(&mut rt);
    reads.set(0);
    play(&mut rt, &now);
    assert_eq!(reads.get(), 0, "the list kept while its number holds");
    // another title for the song playing: the number moves, the list is read again once and
    // the playlist shows it
    rt.player().set_track("Another song", 123_000);
    assert!(rt.tick());
    let after = drawn_text(&mut rt);
    assert_eq!(reads.get(), 1);
    assert_ne!(before, after);
    play(&mut rt, &now);
    assert_eq!(reads.get(), 1);
}

#[test]
fn an_unnumbered_playlist_is_read_whenever_it_is_used() {
    let now = Rc::new(Cell::new(0));
    let clock: Clock = {
        let now = now.clone();
        Rc::new(move || now.get())
    };
    let (mut rt, reads) = runtime(false, &clock);
    reads.set(0);
    play(&mut rt, &now);
    assert!(reads.get() >= 40, "{} reads", reads.get());
    rt.player().set_track("Another song", 123_000);
    assert!(rt.tick());
}

#[test]
fn both_draw_the_same() {
    let now = Rc::new(Cell::new(0));
    let clock: Clock = {
        let now = now.clone();
        Rc::new(move || now.get())
    };
    let (mut a, _) = runtime(true, &clock);
    let (mut b, _) = runtime(false, &clock);
    for step in 0..80 {
        now.set(now.get() + 25);
        for rt in [&mut a, &mut b] {
            match step {
                20 => rt.scroll_classic_playlist(5),
                40 => rt.player().set_track("Another song", 123_000),
                60 => rt.player().stop(),
                _ => {}
            }
        }
        assert_eq!(a.tick(), b.tick(), "step {step}");
        assert_eq!(a.draw_windows(), b.draw_windows(), "step {step}");
    }
}
