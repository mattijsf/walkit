// SPDX-License-Identifier: GPL-3.0-or-later

//! The player as a modern skin sees it, and a player that plays nothing.

use std::collections::BTreeMap;

use crate::host::Clock;

/// One playlist entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    /// The title shown.
    pub title: String,
    /// The length in milliseconds.
    pub length_ms: i32,
}

/// What the transport is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    /// Nothing plays.
    Stopped,
    /// The song plays.
    Playing,
    /// The song is paused.
    Paused,
}

/// The player as a modern skin sees it. Positions and lengths are milliseconds; volume is 0..255.
pub trait Player {
    /// What the transport is doing.
    fn status(&self) -> Status;
    /// Where the song is, in milliseconds.
    fn position_ms(&self) -> i32;
    /// The song's length, in milliseconds.
    fn length_ms(&self) -> i32;
    /// The song's title.
    fn title(&self) -> String;

    /// Makes `title`, `length_ms` long, the current song, where the player allows a host to
    /// stage one (a check page's fake player); nothing elsewhere.
    fn set_track(&mut self, title: &str, length_ms: i32) {
        let _ = (title, length_ms);
    }

    /// Makes the player hand out silence (an empty-ish spectrum and a flat wave) where it allows
    /// a host to stage one (a check page's fake player); nothing elsewhere.
    fn set_silent(&mut self, silent: bool) {
        let _ = silent;
    }

    /// Stages the song's format where the player allows it (a check page's fake player):
    /// `kbps`, `khz` and `channels`; nothing elsewhere.
    fn set_format(&mut self, kbps: i32, khz: i32, channels: i32) {
        let _ = (kbps, khz, channels);
    }
    /// The bitrate.
    fn bitrate_kbps(&self) -> i32;
    /// The sample rate.
    fn sample_rate_khz(&self) -> i32;
    /// The channel count.
    fn channels(&self) -> i32;
    /// The current file's tags by lowercase field name (artist, album, ...); a file without tags has none.
    fn tags(&self) -> BTreeMap<String, String> {
        BTreeMap::new()
    }
    /// The input plug-in's name for the current file, as Winamp's `getDecoderName` reports it; empty when unknown.
    fn decoder_name(&self) -> String {
        String::new()
    }
    /// The volume, 0..255.
    fn volume(&self) -> i32;
    /// Sets the volume.
    fn set_volume(&mut self, v: i32);
    /// Shuffle on.
    fn shuffle(&self) -> bool;
    /// Sets shuffle.
    fn set_shuffle(&mut self, on: bool);
    /// Repeat on.
    fn repeat(&self) -> bool;
    /// Sets repeat.
    fn set_repeat(&mut self, on: bool);
    /// Crossfade on.
    fn crossfade(&self) -> bool;
    /// Sets crossfade.
    fn set_crossfade(&mut self, on: bool);
    /// The equalizer on.
    fn eq_enabled(&self) -> bool;
    /// Sets the equalizer on or off.
    fn set_eq_enabled(&mut self, on: bool);
    /// The ten band values, 0..255.
    fn eq_bands(&self) -> Vec<i32>;
    /// The preamp, 0..255.
    fn eq_preamp(&self) -> i32;
    /// Sets the preamp.
    fn set_eq_preamp(&mut self, v: i32);
    /// The balance: -127 hard left, 0 the middle, 127 hard right.
    fn pan(&self) -> i32;
    /// Sets the balance.
    fn set_pan(&mut self, v: i32);
    /// Starts playing.
    fn play(&mut self);
    /// Pauses, or resumes when paused.
    fn pause(&mut self);
    /// Stops.
    fn stop(&mut self);
    /// The next song.
    fn next(&mut self);
    /// The previous song.
    fn previous(&mut self);
    /// Ejects: opens a file.
    fn eject(&mut self);
    /// Seeks to `ms`.
    fn seek_to(&mut self, ms: i32);
    /// Sets band `band` to `value`.
    fn set_eq_band(&mut self, band: i32, value: i32);
    /// The playlist; a player without one has just the current song.
    fn tracks(&self) -> Vec<Track> {
        vec![Track {
            title: self.title(),
            length_ms: self.length_ms(),
        }]
    }
    /// Which entry is current.
    fn track_index(&self) -> i32 {
        0
    }
    /// Makes entry `index` current and plays it.
    fn play_track(&mut self, _index: i32) {
        self.play();
    }
    /// Spectrum levels 0..1, 75 bands like Winamp's vis, or empty when nothing plays.
    fn spectrum(&self) -> Vec<f32>;
    /// The waveform being heard: samples -1..1, oldest first, 576 of them as Winamp hands a
    /// visualizer, or empty when nothing plays.
    fn waveform(&self) -> Vec<f32> {
        Vec::new()
    }
    /// Left and right VU 0..255.
    fn vu(&self) -> (i32, i32);
}

/// A player that plays nothing, for tests and for a first look at a skin. The clock positions,
/// the demo spectrum and the demo wave follow; a test gives it the runtime's.
pub struct FakePlayer {
    clock: Clock,
    status: Status,
    started_at: i64,
    paused_at: i32,
    /// Set to show several entries; empty means the playlist is the current song alone.
    pub playlist: Vec<Track>,
    index: i32,
    /// The song's length.
    pub length_ms: i32,
    /// The song's title.
    pub title: String,
    /// What `getDecoderName` answers.
    pub decoder_name: String,
    /// The bitrate.
    pub bitrate_kbps: i32,
    /// The sample rate.
    pub sample_rate_khz: i32,
    /// The channel count.
    pub channels: i32,
    /// Whether it hands out silence.
    pub silent: bool,
    volume: i32,
    shuffle: bool,
    repeat: bool,
    crossfade: bool,
    eq_enabled: bool,
    eq_bands: Vec<i32>,
    eq_preamp: i32,
    pan: i32,
}

impl FakePlayer {
    /// A stopped player reading `clock`.
    pub fn new(clock: Clock) -> Self {
        FakePlayer {
            clock,
            status: Status::Stopped,
            started_at: 0,
            paused_at: 0,
            playlist: Vec::new(),
            index: 0,
            length_ms: 215_000,
            title: "Andamp - Modern skin demo".to_string(),
            decoder_name: String::new(),
            bitrate_kbps: 192,
            sample_rate_khz: 44,
            channels: 2,
            silent: false,
            volume: 200,
            shuffle: false,
            repeat: false,
            crossfade: false,
            eq_enabled: false,
            eq_bands: vec![128; 10],
            eq_preamp: 128,
            pan: 0,
        }
    }

    fn now(&self) -> i64 {
        (self.clock)()
    }
}

impl Player for FakePlayer {
    fn set_silent(&mut self, silent: bool) {
        self.silent = silent;
    }

    fn set_format(&mut self, kbps: i32, khz: i32, channels: i32) {
        self.bitrate_kbps = kbps;
        self.sample_rate_khz = khz;
        self.channels = channels;
    }

    fn set_track(&mut self, title: &str, length_ms: i32) {
        self.title = title.to_string();
        self.length_ms = length_ms;
        if let Some(t) = self.playlist.get_mut(self.index.max(0) as usize) {
            t.title = title.to_string();
            t.length_ms = length_ms;
        }
    }

    fn status(&self) -> Status {
        self.status
    }

    fn position_ms(&self) -> i32 {
        match self.status {
            Status::Playing => (((self.now() - self.started_at) as i32)
                .wrapping_add(self.paused_at))
            .min(self.length_ms),
            Status::Paused => self.paused_at,
            Status::Stopped => 0,
        }
    }

    fn length_ms(&self) -> i32 {
        self.length_ms
    }

    fn title(&self) -> String {
        self.title.clone()
    }

    fn bitrate_kbps(&self) -> i32 {
        self.bitrate_kbps
    }

    fn sample_rate_khz(&self) -> i32 {
        self.sample_rate_khz
    }

    fn channels(&self) -> i32 {
        self.channels
    }

    fn decoder_name(&self) -> String {
        self.decoder_name.clone()
    }

    fn volume(&self) -> i32 {
        self.volume
    }

    fn set_volume(&mut self, v: i32) {
        self.volume = v;
    }

    fn shuffle(&self) -> bool {
        self.shuffle
    }

    fn set_shuffle(&mut self, on: bool) {
        self.shuffle = on;
    }

    fn repeat(&self) -> bool {
        self.repeat
    }

    fn set_repeat(&mut self, on: bool) {
        self.repeat = on;
    }

    fn crossfade(&self) -> bool {
        self.crossfade
    }

    fn set_crossfade(&mut self, on: bool) {
        self.crossfade = on;
    }

    fn eq_enabled(&self) -> bool {
        self.eq_enabled
    }

    fn set_eq_enabled(&mut self, on: bool) {
        self.eq_enabled = on;
    }

    fn eq_bands(&self) -> Vec<i32> {
        self.eq_bands.clone()
    }

    fn eq_preamp(&self) -> i32 {
        self.eq_preamp
    }

    fn set_eq_preamp(&mut self, v: i32) {
        self.eq_preamp = v;
    }

    fn pan(&self) -> i32 {
        self.pan
    }

    fn set_pan(&mut self, v: i32) {
        self.pan = v;
    }

    fn play(&mut self) {
        if self.status == Status::Playing {
            return;
        }
        if self.status == Status::Stopped {
            self.paused_at = 0;
        }
        self.started_at = self.now();
        self.status = Status::Playing;
    }

    fn pause(&mut self) {
        match self.status {
            Status::Playing => {
                self.paused_at = self.position_ms();
                self.status = Status::Paused;
            }
            Status::Paused => self.play(),
            Status::Stopped => {}
        }
    }

    fn stop(&mut self) {
        self.status = Status::Stopped;
        self.paused_at = 0;
    }

    fn next(&mut self) {
        self.seek_to(0);
    }

    fn previous(&mut self) {
        self.seek_to(0);
    }

    fn eject(&mut self) {}

    fn seek_to(&mut self, ms: i32) {
        self.paused_at = ms.clamp(0, self.length_ms.max(0));
        self.started_at = self.now();
    }

    fn set_eq_band(&mut self, band: i32, value: i32) {
        if let Some(b) = usize::try_from(band)
            .ok()
            .and_then(|i| self.eq_bands.get_mut(i))
        {
            *b = value;
        }
    }

    fn tracks(&self) -> Vec<Track> {
        if self.playlist.is_empty() {
            vec![Track {
                title: self.title.clone(),
                length_ms: self.length_ms,
            }]
        } else {
            self.playlist.clone()
        }
    }

    fn track_index(&self) -> i32 {
        self.index.clamp(0, self.tracks().len() as i32 - 1)
    }

    fn play_track(&mut self, index: i32) {
        let tracks = self.tracks();
        let Some(t) = usize::try_from(index).ok().and_then(|i| tracks.get(i)) else {
            return;
        };
        self.index = index;
        self.title.clone_from(&t.title);
        self.length_ms = t.length_ms;
        self.status = Status::Stopped;
        self.play();
    }

    fn spectrum(&self) -> Vec<f32> {
        if self.status != Status::Playing {
            return Vec::new();
        }
        if self.silent {
            return vec![0.0; 75];
        }
        let t = self.now() as f64 / 60.0;
        (0..75)
            .map(|i| ((0.5 + 0.5 * (t + i as f64 * 0.3).sin()) as f32) * (1.0 - i as f32 / 90.0))
            .collect()
    }

    /// Two tones drifting with the clock, a slow one and one three times as fast, while playing.
    fn waveform(&self) -> Vec<f32> {
        if self.status != Status::Playing {
            return Vec::new();
        }
        let t = self.now() as f64 / 60.0;
        (0..576)
            .map(|i| {
                (0.35 * (t + i as f64 * 0.05).sin() + 0.15 * (3.0 * t + i as f64 * 0.27).sin())
                    as f32
            })
            .collect()
    }

    fn vu(&self) -> (i32, i32) {
        if self.status == Status::Playing {
            (160, 150)
        } else {
            (0, 0)
        }
    }
}
