// SPDX-License-Identifier: GPL-3.0-or-later

//! The player as the Kotlin side holds it: every question and command goes to the
//! `PlayerBridge`, whose methods take and give primitives, strings and arrays (the status as
//! 0 stopped, 1 playing, 2 paused; the tags as a string array of name, value pairs; the
//! playlist as a string array of titles and an int array of lengths).
//!
//! The player is read once per native call: the first number asked for in a call takes a
//! snapshot of every number the bridge gives at once (`snapshot`), the title, the playlist and
//! the sound data are each read on their first use, and every later read in the call comes
//! from those. A command or a setting clears them, so a script that sets the volume reads
//! the volume it set. A frame's drawing asks the player a hundred times over; this way it
//! crosses into Kotlin a few times.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use jni::objects::JValue;
use jni::{jni_sig, jni_str};
use walkit::player::{Player, Status, Track};

use crate::bridge::{floats_of, ints_of, string_of, strings_of, Bridge};

/// Where each number sits in the bridge's snapshot; the band count and the bands follow.
const STATUS: usize = 0;
const POSITION_MS: usize = 1;
const LENGTH_MS: usize = 2;
const BITRATE_KBPS: usize = 3;
const SAMPLE_RATE_KHZ: usize = 4;
const CHANNELS: usize = 5;
const VOLUME: usize = 6;
const SHUFFLE: usize = 7;
const REPEAT: usize = 8;
const CROSSFADE: usize = 9;
const EQ_ENABLED: usize = 10;
const EQ_PREAMP: usize = 11;
const PAN: usize = 12;
const TRACK_INDEX: usize = 13;
const EQ_BANDS: usize = 14;

/// What a native call has read of the player so far; cleared as the next call starts.
#[derive(Default)]
pub struct PlayerCache {
    numbers: RefCell<Option<Vec<i32>>>,
    title: RefCell<Option<String>>,
    tracks: RefCell<Option<Vec<Track>>>,
    spectrum: RefCell<Option<Vec<f32>>>,
    waveform: RefCell<Option<Vec<f32>>>,
    vu: RefCell<Option<(i32, i32)>>,
}

impl PlayerCache {
    /// Forgets everything read, so the next read asks the player again.
    pub fn clear(&self) {
        self.numbers.borrow_mut().take();
        self.title.borrow_mut().take();
        self.tracks.borrow_mut().take();
        self.spectrum.borrow_mut().take();
        self.waveform.borrow_mut().take();
        self.vu.borrow_mut().take();
    }
}

/// The player bridge as the engine drives the player, with what the call in progress read of it.
pub struct JniPlayer {
    bridge: Bridge,
    cache: Rc<PlayerCache>,
}

impl JniPlayer {
    pub fn new(bridge: Bridge, cache: Rc<PlayerCache>) -> JniPlayer {
        JniPlayer { bridge, cache }
    }

    fn string(&self, name: &'static jni::strings::JNIStr) -> String {
        self.bridge
            .with(|env, obj| {
                let r = env
                    .call_method(obj, name, jni_sig!("()Ljava/lang/String;"), &[])?
                    .l()?;
                string_of(env, r)
            })
            .unwrap_or_default()
    }

    fn floats(&self, name: &'static jni::strings::JNIStr) -> Vec<f32> {
        self.bridge
            .with(|env, obj| {
                let r = env.call_method(obj, name, jni_sig!("()[F"), &[])?.l()?;
                floats_of(env, r)
            })
            .flatten()
            .unwrap_or_default()
    }

    fn ints(&self, name: &'static jni::strings::JNIStr) -> Vec<i32> {
        self.bridge
            .with(|env, obj| {
                let r = env.call_method(obj, name, jni_sig!("()[I"), &[])?.l()?;
                ints_of(env, r)
            })
            .flatten()
            .unwrap_or_default()
    }

    fn strings(&self, name: &'static jni::strings::JNIStr) -> Vec<String> {
        self.bridge
            .with(|env, obj| {
                let r = env
                    .call_method(obj, name, jni_sig!("()[Ljava/lang/String;"), &[])?
                    .l()?;
                strings_of(env, r)
            })
            .flatten()
            .unwrap_or_default()
    }

    /// Runs a command: the player may be another afterwards, so the call reads it afresh.
    fn run(&self, name: &'static jni::strings::JNIStr) {
        self.cache.clear();
        self.bridge.with(|env, obj| {
            env.call_method(obj, name, jni_sig!("()V"), &[])?;
            Ok(())
        });
    }

    fn set_int(&self, name: &'static jni::strings::JNIStr, v: i32) {
        self.cache.clear();
        self.bridge.with(|env, obj| {
            env.call_method(obj, name, jni_sig!("(I)V"), &[JValue::Int(v)])?;
            Ok(())
        });
    }

    fn set_bool(&self, name: &'static jni::strings::JNIStr, v: bool) {
        self.cache.clear();
        self.bridge.with(|env, obj| {
            env.call_method(obj, name, jni_sig!("(Z)V"), &[JValue::Bool(v)])?;
            Ok(())
        });
    }

    /// The number at `i` of the snapshot, taken now if the call has none yet; 0 past its end.
    fn number(&self, i: usize) -> i32 {
        let mut numbers = self.cache.numbers.borrow_mut();
        let snapshot = numbers.get_or_insert_with(|| self.ints(jni_str!("snapshot")));
        snapshot.get(i).copied().unwrap_or(0)
    }

    fn flag(&self, i: usize) -> bool {
        self.number(i) != 0
    }
}

impl Player for JniPlayer {
    fn status(&self) -> Status {
        match self.number(STATUS) {
            1 => Status::Playing,
            2 => Status::Paused,
            _ => Status::Stopped,
        }
    }

    fn position_ms(&self) -> i32 {
        self.number(POSITION_MS)
    }

    fn length_ms(&self) -> i32 {
        self.number(LENGTH_MS)
    }

    fn title(&self) -> String {
        self.cache
            .title
            .borrow_mut()
            .get_or_insert_with(|| self.string(jni_str!("title")))
            .clone()
    }

    fn bitrate_kbps(&self) -> i32 {
        self.number(BITRATE_KBPS)
    }

    fn sample_rate_khz(&self) -> i32 {
        self.number(SAMPLE_RATE_KHZ)
    }

    fn channels(&self) -> i32 {
        self.number(CHANNELS)
    }

    fn tags(&self) -> BTreeMap<String, String> {
        let pairs = self.strings(jni_str!("tags"));
        pairs
            .chunks_exact(2)
            .map(|kv| (kv[0].clone(), kv[1].clone()))
            .collect()
    }

    fn decoder_name(&self) -> String {
        self.string(jni_str!("decoderName"))
    }

    fn volume(&self) -> i32 {
        self.number(VOLUME)
    }

    fn set_volume(&mut self, v: i32) {
        self.set_int(jni_str!("setVolume"), v)
    }

    fn shuffle(&self) -> bool {
        self.flag(SHUFFLE)
    }

    fn set_shuffle(&mut self, on: bool) {
        self.set_bool(jni_str!("setShuffle"), on)
    }

    fn repeat(&self) -> bool {
        self.flag(REPEAT)
    }

    fn set_repeat(&mut self, on: bool) {
        self.set_bool(jni_str!("setRepeat"), on)
    }

    fn crossfade(&self) -> bool {
        self.flag(CROSSFADE)
    }

    fn set_crossfade(&mut self, on: bool) {
        self.set_bool(jni_str!("setCrossfade"), on)
    }

    fn eq_enabled(&self) -> bool {
        self.flag(EQ_ENABLED)
    }

    fn set_eq_enabled(&mut self, on: bool) {
        self.set_bool(jni_str!("setEqEnabled"), on)
    }

    fn eq_bands(&self) -> Vec<i32> {
        let n = self.number(EQ_BANDS).max(0) as usize;
        (0..n).map(|i| self.number(EQ_BANDS + 1 + i)).collect()
    }

    fn eq_preamp(&self) -> i32 {
        self.number(EQ_PREAMP)
    }

    fn set_eq_preamp(&mut self, v: i32) {
        self.set_int(jni_str!("setEqPreamp"), v)
    }

    fn pan(&self) -> i32 {
        self.number(PAN)
    }

    fn set_pan(&mut self, v: i32) {
        self.set_int(jni_str!("setPan"), v)
    }

    fn play(&mut self) {
        self.run(jni_str!("play"))
    }

    fn pause(&mut self) {
        self.run(jni_str!("pause"))
    }

    fn stop(&mut self) {
        self.run(jni_str!("stop"))
    }

    fn next(&mut self) {
        self.run(jni_str!("next"))
    }

    fn previous(&mut self) {
        self.run(jni_str!("previous"))
    }

    fn eject(&mut self) {
        self.run(jni_str!("eject"))
    }

    fn seek_to(&mut self, ms: i32) {
        self.set_int(jni_str!("seekTo"), ms)
    }

    fn set_eq_band(&mut self, band: i32, value: i32) {
        self.cache.clear();
        self.bridge.with(|env, obj| {
            env.call_method(
                obj,
                jni_str!("setEqBand"),
                jni_sig!("(II)V"),
                &[JValue::Int(band), JValue::Int(value)],
            )?;
            Ok(())
        });
    }

    fn tracks(&self) -> Vec<Track> {
        self.cache
            .tracks
            .borrow_mut()
            .get_or_insert_with(|| {
                let titles = self.strings(jni_str!("trackTitles"));
                let lengths = self.ints(jni_str!("trackLengths"));
                titles
                    .into_iter()
                    .zip(lengths)
                    .map(|(title, length_ms)| Track { title, length_ms })
                    .collect()
            })
            .clone()
    }

    fn track_index(&self) -> i32 {
        self.number(TRACK_INDEX)
    }

    fn play_track(&mut self, index: i32) {
        self.set_int(jni_str!("playTrack"), index)
    }

    fn spectrum(&self) -> Vec<f32> {
        self.cache
            .spectrum
            .borrow_mut()
            .get_or_insert_with(|| self.floats(jni_str!("spectrum")))
            .clone()
    }

    fn waveform(&self) -> Vec<f32> {
        self.cache
            .waveform
            .borrow_mut()
            .get_or_insert_with(|| self.floats(jni_str!("waveform")))
            .clone()
    }

    fn vu(&self) -> (i32, i32) {
        *self.cache.vu.borrow_mut().get_or_insert_with(|| {
            let v = self.ints(jni_str!("vu"));
            if v.len() == 2 {
                (v[0], v[1])
            } else {
                (0, 0)
            }
        })
    }
}
