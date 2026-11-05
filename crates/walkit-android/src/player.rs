// SPDX-License-Identifier: GPL-3.0-or-later

//! The player as the Kotlin side holds it: every question and command goes to the
//! `PlayerBridge`, whose methods take and give primitives, strings and arrays (the status as
//! 0 stopped, 1 playing, 2 paused; the tags as a string array of name, value pairs; the
//! playlist as a string array of titles and an int array of lengths).

use std::collections::BTreeMap;

use jni::objects::JValue;
use jni::{jni_sig, jni_str};
use walkit::player::{Player, Status, Track};

use crate::bridge::{floats_of, ints_of, string_of, strings_of, Bridge};

/// The player bridge as the engine drives the player.
pub struct JniPlayer(pub Bridge);

impl JniPlayer {
    fn int(&self, name: &'static jni::strings::JNIStr) -> i32 {
        self.0
            .with(|env, obj| env.call_method(obj, name, jni_sig!("()I"), &[])?.i())
            .unwrap_or(0)
    }

    fn bool(&self, name: &'static jni::strings::JNIStr) -> bool {
        self.0
            .with(|env, obj| env.call_method(obj, name, jni_sig!("()Z"), &[])?.z())
            .unwrap_or(false)
    }

    fn string(&self, name: &'static jni::strings::JNIStr) -> String {
        self.0
            .with(|env, obj| {
                let r = env
                    .call_method(obj, name, jni_sig!("()Ljava/lang/String;"), &[])?
                    .l()?;
                string_of(env, r)
            })
            .unwrap_or_default()
    }

    fn floats(&self, name: &'static jni::strings::JNIStr) -> Vec<f32> {
        self.0
            .with(|env, obj| {
                let r = env.call_method(obj, name, jni_sig!("()[F"), &[])?.l()?;
                floats_of(env, r)
            })
            .flatten()
            .unwrap_or_default()
    }

    fn ints(&self, name: &'static jni::strings::JNIStr) -> Vec<i32> {
        self.0
            .with(|env, obj| {
                let r = env.call_method(obj, name, jni_sig!("()[I"), &[])?.l()?;
                ints_of(env, r)
            })
            .flatten()
            .unwrap_or_default()
    }

    fn strings(&self, name: &'static jni::strings::JNIStr) -> Vec<String> {
        self.0
            .with(|env, obj| {
                let r = env
                    .call_method(obj, name, jni_sig!("()[Ljava/lang/String;"), &[])?
                    .l()?;
                strings_of(env, r)
            })
            .flatten()
            .unwrap_or_default()
    }

    fn run(&self, name: &'static jni::strings::JNIStr) {
        self.0.with(|env, obj| {
            env.call_method(obj, name, jni_sig!("()V"), &[])?;
            Ok(())
        });
    }

    fn set_int(&self, name: &'static jni::strings::JNIStr, v: i32) {
        self.0.with(|env, obj| {
            env.call_method(obj, name, jni_sig!("(I)V"), &[JValue::Int(v)])?;
            Ok(())
        });
    }

    fn set_bool(&self, name: &'static jni::strings::JNIStr, v: bool) {
        self.0.with(|env, obj| {
            env.call_method(obj, name, jni_sig!("(Z)V"), &[JValue::Bool(v)])?;
            Ok(())
        });
    }
}

impl Player for JniPlayer {
    fn status(&self) -> Status {
        match self.int(jni_str!("status")) {
            1 => Status::Playing,
            2 => Status::Paused,
            _ => Status::Stopped,
        }
    }

    fn position_ms(&self) -> i32 {
        self.int(jni_str!("positionMs"))
    }

    fn length_ms(&self) -> i32 {
        self.int(jni_str!("lengthMs"))
    }

    fn title(&self) -> String {
        self.string(jni_str!("title"))
    }

    fn bitrate_kbps(&self) -> i32 {
        self.int(jni_str!("bitrateKbps"))
    }

    fn sample_rate_khz(&self) -> i32 {
        self.int(jni_str!("sampleRateKhz"))
    }

    fn channels(&self) -> i32 {
        self.int(jni_str!("channels"))
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
        self.int(jni_str!("volume"))
    }

    fn set_volume(&mut self, v: i32) {
        self.set_int(jni_str!("setVolume"), v)
    }

    fn shuffle(&self) -> bool {
        self.bool(jni_str!("shuffle"))
    }

    fn set_shuffle(&mut self, on: bool) {
        self.set_bool(jni_str!("setShuffle"), on)
    }

    fn repeat(&self) -> bool {
        self.bool(jni_str!("repeat"))
    }

    fn set_repeat(&mut self, on: bool) {
        self.set_bool(jni_str!("setRepeat"), on)
    }

    fn crossfade(&self) -> bool {
        self.bool(jni_str!("crossfade"))
    }

    fn set_crossfade(&mut self, on: bool) {
        self.set_bool(jni_str!("setCrossfade"), on)
    }

    fn eq_enabled(&self) -> bool {
        self.bool(jni_str!("eqEnabled"))
    }

    fn set_eq_enabled(&mut self, on: bool) {
        self.set_bool(jni_str!("setEqEnabled"), on)
    }

    fn eq_bands(&self) -> Vec<i32> {
        self.ints(jni_str!("eqBands"))
    }

    fn eq_preamp(&self) -> i32 {
        self.int(jni_str!("eqPreamp"))
    }

    fn set_eq_preamp(&mut self, v: i32) {
        self.set_int(jni_str!("setEqPreamp"), v)
    }

    fn pan(&self) -> i32 {
        self.int(jni_str!("pan"))
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
        self.0.with(|env, obj| {
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
        let titles = self.strings(jni_str!("trackTitles"));
        let lengths = self.ints(jni_str!("trackLengths"));
        titles
            .into_iter()
            .zip(lengths)
            .map(|(title, length_ms)| Track { title, length_ms })
            .collect()
    }

    fn track_index(&self) -> i32 {
        self.int(jni_str!("trackIndex"))
    }

    fn play_track(&mut self, index: i32) {
        self.set_int(jni_str!("playTrack"), index)
    }

    fn spectrum(&self) -> Vec<f32> {
        self.floats(jni_str!("spectrum"))
    }

    fn waveform(&self) -> Vec<f32> {
        self.floats(jni_str!("waveform"))
    }

    fn vu(&self) -> (i32, i32) {
        let v = self.ints(jni_str!("vu"));
        if v.len() == 2 {
            (v[0], v[1])
        } else {
            (0, 0)
        }
    }
}
