// SPDX-License-Identifier: GPL-3.0-or-later

//! A player the page owns: the engine reads the state the page last pushed
//! (`wk_rt_host_player_state`, `wk_rt_host_player_vis`) and sends every command to the page
//! (`host_player`), which plays the sound itself (webamp's audio graph, say).

use std::cell::RefCell;
use std::rc::Rc;

use walkit::player::{Player, Status};

use crate::imports;

/// What the page last said about its player.
#[derive(Debug, Clone)]
pub struct HostState {
    pub status: Status,
    pub position_ms: i32,
    pub length_ms: i32,
    pub title: String,
    pub kbps: i32,
    pub khz: i32,
    pub channels: i32,
    pub volume: i32,
    pub pan: i32,
    pub shuffle: bool,
    pub repeat: bool,
    pub eq_on: bool,
    pub preamp: i32,
    pub bands: Vec<i32>,
    pub spectrum: Vec<f32>,
    pub waveform: Vec<f32>,
    pub vu: (i32, i32),
    pub track_index: i32,
}

impl Default for HostState {
    fn default() -> Self {
        HostState {
            status: Status::Stopped,
            position_ms: 0,
            length_ms: 0,
            title: String::new(),
            kbps: 0,
            khz: 0,
            channels: 2,
            volume: 200,
            pan: 0,
            shuffle: false,
            repeat: false,
            eq_on: false,
            preamp: 128,
            bands: vec![128; 10],
            spectrum: Vec::new(),
            waveform: Vec::new(),
            vu: (0, 0),
            track_index: 0,
        }
    }
}

/// The ints `wk_rt_host_player_state` takes, in order.
pub const STATE_INTS: usize = 23;

impl HostState {
    /// Takes the ints of `wk_rt_host_player_state`: status (0 stopped, 1 playing, 2 paused),
    /// position and length in ms, kbps, kHz, channels, volume 0..255, pan -127..127, shuffle,
    /// repeat, eq on, preamp 0..255, the ten bands 0..255, then the playlist entry playing (from 0).
    pub fn set_ints(&mut self, v: &[i32]) {
        if v.len() < STATE_INTS {
            return;
        }
        self.status = match v[0] {
            1 => Status::Playing,
            2 => Status::Paused,
            _ => Status::Stopped,
        };
        self.position_ms = v[1];
        self.length_ms = v[2];
        self.kbps = v[3];
        self.khz = v[4];
        self.channels = v[5];
        self.volume = v[6];
        self.pan = v[7];
        self.shuffle = v[8] != 0;
        self.repeat = v[9] != 0;
        self.eq_on = v[10] != 0;
        self.preamp = v[11];
        self.bands = v[12..22].to_vec();
        self.track_index = v[22];
    }
}

/// The engine's side of the page's player.
pub struct HostPlayer(pub Rc<RefCell<HostState>>);

impl Player for HostPlayer {
    fn status(&self) -> Status {
        self.0.borrow().status
    }
    fn position_ms(&self) -> i32 {
        self.0.borrow().position_ms
    }
    fn length_ms(&self) -> i32 {
        self.0.borrow().length_ms
    }
    fn title(&self) -> String {
        self.0.borrow().title.clone()
    }
    fn bitrate_kbps(&self) -> i32 {
        self.0.borrow().kbps
    }
    fn sample_rate_khz(&self) -> i32 {
        self.0.borrow().khz
    }
    fn channels(&self) -> i32 {
        self.0.borrow().channels
    }
    fn volume(&self) -> i32 {
        self.0.borrow().volume
    }
    // each setter keeps the value at once, so a slider being dragged does not jump back before
    // the page pushes its state again
    fn set_volume(&mut self, v: i32) {
        self.0.borrow_mut().volume = v;
        imports::player("volume", v);
    }
    fn shuffle(&self) -> bool {
        self.0.borrow().shuffle
    }
    fn set_shuffle(&mut self, on: bool) {
        self.0.borrow_mut().shuffle = on;
        imports::player("shuffle", on as i32);
    }
    fn repeat(&self) -> bool {
        self.0.borrow().repeat
    }
    fn set_repeat(&mut self, on: bool) {
        self.0.borrow_mut().repeat = on;
        imports::player("repeat", on as i32);
    }
    fn crossfade(&self) -> bool {
        false
    }
    fn set_crossfade(&mut self, _on: bool) {}
    fn eq_enabled(&self) -> bool {
        self.0.borrow().eq_on
    }
    fn set_eq_enabled(&mut self, on: bool) {
        self.0.borrow_mut().eq_on = on;
        imports::player("eq", on as i32);
    }
    fn eq_bands(&self) -> Vec<i32> {
        self.0.borrow().bands.clone()
    }
    fn eq_preamp(&self) -> i32 {
        self.0.borrow().preamp
    }
    fn set_eq_preamp(&mut self, v: i32) {
        self.0.borrow_mut().preamp = v;
        imports::player("preamp", v);
    }
    fn pan(&self) -> i32 {
        self.0.borrow().pan
    }
    fn set_pan(&mut self, v: i32) {
        self.0.borrow_mut().pan = v;
        imports::player("pan", v);
    }
    fn play(&mut self) {
        imports::player("play", 0);
    }
    fn pause(&mut self) {
        imports::player("pause", 0);
    }
    fn stop(&mut self) {
        imports::player("stop", 0);
    }
    fn next(&mut self) {
        imports::player("next", 0);
    }
    fn previous(&mut self) {
        imports::player("previous", 0);
    }
    fn eject(&mut self) {
        imports::player("eject", 0);
    }
    fn seek_to(&mut self, ms: i32) {
        self.0.borrow_mut().position_ms = ms;
        imports::player("seek", ms);
    }
    fn set_eq_band(&mut self, band: i32, value: i32) {
        if let Some(b) = self.0.borrow_mut().bands.get_mut(band as usize) {
            *b = value;
        }
        imports::player(&format!("band{band}"), value);
    }
    fn spectrum(&self) -> Vec<f32> {
        self.0.borrow().spectrum.clone()
    }
    fn waveform(&self) -> Vec<f32> {
        self.0.borrow().waveform.clone()
    }
    fn vu(&self) -> (i32, i32) {
        self.0.borrow().vu
    }
    fn track_index(&self) -> i32 {
        self.0.borrow().track_index
    }
}
