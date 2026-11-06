// SPDX-License-Identifier: GPL-3.0-or-later

//! The `System` object's script methods: the player, the clock, strings and the group the
//! script lives in.

use super::{SkinRuntime, EQ_FLAT};
use crate::gui::{Kind, ObjId};
use crate::maki::{Handle, Value};
use crate::player::Status;

const MS_PER_DAY: i64 = 24 * 60 * 60 * 1000;
const MS_PER_SECOND: i32 = 1000;

fn arg(args: &[Value], i: usize) -> Value {
    args.get(i).cloned().unwrap_or(Value::Null)
}

fn format_time(seconds: i32) -> String {
    let s = seconds.abs();
    format!(
        "{}{}:{:02}",
        if seconds < 0 { "-" } else { "" },
        s / 60,
        s % 60
    )
}

/// The first `n` UTF-16 units of `s`, as Kotlin's `take` counts them.
fn take_units(s: &str, n: usize) -> String {
    let units: Vec<u16> = s.encode_utf16().take(n).collect();
    String::from_utf16_lossy(&units)
}

fn drop_units(s: &str, n: usize) -> String {
    let units: Vec<u16> = s.encode_utf16().skip(n).collect();
    String::from_utf16_lossy(&units)
}

fn take_last_units(s: &str, n: usize) -> String {
    let all: Vec<u16> = s.encode_utf16().collect();
    let from = all.len().saturating_sub(n);
    String::from_utf16_lossy(&all[from..])
}

/// `indexOf` in UTF-16 units, as Kotlin counts them.
fn index_of_units(s: &str, sub: &str) -> i32 {
    match s.find(sub) {
        Some(byte) => s[..byte].encode_utf16().count() as i32,
        None => -1,
    }
}

impl SkinRuntime {
    /// A method of the System object of script `s`.
    pub(crate) fn system_maki_call(
        &mut self,
        s: usize,
        name: &str,
        args: &[Value],
    ) -> Option<Value> {
        let str = |v: String| Value::string(v);
        let int = Value::Int;
        let num = Value::Num;
        Some(match name {
            "getclassname" => Value::str("System"),
            "getid" => Value::str("system"),
            "getscriptgroup" => Value::of(self.systems[s].group.map(Handle::Gui)),
            "getparam" => str(self.systems[s].param.clone()),
            "getcontainer" => Value::of(
                self.container(&arg(args, 0).as_string())
                    .map(Handle::Container),
            ),
            "getnumcontainers" => int(self.containers.len() as i32),
            "enumcontainer" => Value::of(
                usize::try_from(arg(args, 0).as_int())
                    .ok()
                    .filter(|i| *i < self.containers.len())
                    .map(Handle::Container),
            ),
            "newgroup" => Value::of(
                self.instantiate_group(&arg(args, 0).as_string())
                    .map(Handle::Gui),
            ),
            "newdynamiccontainer" | "newgroupaslayout" => Value::Null,
            // the compiler-made prologue wants 2..65535 here; Winamp answers its own version
            "getruntimeversion" => num(5.666),
            "getversionstring" | "getwinampversion" => Value::str("5.66"),
            "getskinname" => str(self.skin_name.clone()),
            "getplayitemstring" | "getplayitemdisplaytitle" => str(self.player.title()),
            "getplayitemmetadatastring" | "getmetadatastring" => {
                // a file without tags answers nothing for its fields; the display title comes from the file name
                let field = args
                    .last()
                    .map(|a| a.as_string().to_lowercase())
                    .unwrap_or_default();
                let p = &*self.player;
                str(match field.as_str() {
                    "title" => p.tags().get("title").cloned().unwrap_or_default(),
                    "bitrate" => (p.bitrate_kbps() * 1000).to_string(),
                    "srate" => (p.sample_rate_khz() * 1000).to_string(),
                    "length" => p.length_ms().to_string(),
                    f => p.tags().get(f).cloned().unwrap_or_default(),
                })
            }
            "getplayitemlength" => int(self.player.length_ms()),
            "getposition" => int(self.player.position_ms()),
            "getstatus" => int(match self.player.status() {
                Status::Playing => 1,
                Status::Paused => -1,
                Status::Stopped => 0,
            }),
            "seekto" => {
                self.player.seek_to(arg(args, 0).as_int());
                Value::Null
            }
            "play" => {
                self.player.play();
                Value::Null
            }
            "stop" => {
                self.player.stop();
                Value::Null
            }
            "pause" => {
                self.player.pause();
                Value::Null
            }
            "next" => {
                self.player.next();
                Value::Null
            }
            "previous" => {
                self.player.previous();
                Value::Null
            }
            "eject" => {
                self.player.eject();
                Value::Null
            }
            "getvolume" => int(self.player.volume()),
            "setvolume" => {
                self.player.set_volume(arg(args, 0).as_int().clamp(0, 255));
                Value::Null
            }
            "getleftvumeter" => int(self.player.vu().0),
            "getrightvumeter" => int(self.player.vu().1),
            "getvisband" => {
                let band = usize::try_from(arg(args, 1).as_int()).ok();
                let level = band
                    .and_then(|b| self.player.spectrum().get(b).copied())
                    .unwrap_or(0.0);
                int((level * 255.0) as i32)
            }
            "geteq" => int(self.player.eq_enabled() as i32),
            "seteq" => {
                self.player.set_eq_enabled(arg(args, 0).truthy());
                Value::Null
            }
            // scripts see the equalizer from -127 to 127 with 0 flat; the player keeps 0-255 with 128 flat
            "geteqband" => {
                let b = usize::try_from(arg(args, 0).as_int()).ok();
                let v = b
                    .and_then(|b| self.player.eq_bands().get(b).copied())
                    .unwrap_or(EQ_FLAT);
                int((v - EQ_FLAT).clamp(-127, 127))
            }
            "seteqband" => {
                self.player.set_eq_band(
                    arg(args, 0).as_int(),
                    (arg(args, 1).as_int() + EQ_FLAT).clamp(0, 255),
                );
                Value::Null
            }
            "geteqpreamp" => int((self.player.eq_preamp() - EQ_FLAT).clamp(-127, 127)),
            "seteqpreamp" => {
                self.player
                    .set_eq_preamp((arg(args, 0).as_int() + EQ_FLAT).clamp(0, 255));
                Value::Null
            }
            "getplaylistlength" => int(self.player.tracks().len() as i32),
            "getplaylistindex" => int(self.player.track_index()),
            // the player can only make an entry current by playing it: while it is stopped or
            // paused, the current entry stays
            "setplaylistindex" => {
                let index = arg(args, 0).as_int();
                if self.player.status() == Status::Playing
                    && index >= 0
                    && (index as usize) < self.player.tracks().len()
                {
                    self.player.play_track(index);
                }
                Value::Null
            }
            // the current song's, stopped or not (Winamp3x shows 64 and 8 after Stop); nothing without one
            "getsonginfotext" | "getsonginfotexttranslated" => {
                let p = &*self.player;
                let channels = if p.channels() == 1 { "mono" } else { "stereo" };
                str(if p.title().is_empty() {
                    String::new()
                } else {
                    format!(
                        "{}kbps {channels} {}kHz",
                        p.bitrate_kbps(),
                        p.sample_rate_khz()
                    )
                })
            }
            "integertostring" => str(arg(args, 0).as_int().to_string()),
            "stringtointeger" => int(arg(args, 0).as_int()),
            "floattostring" => {
                let digits = arg(args, 1).as_int().max(0) as usize;
                str(format!("{:.*}", digits, arg(args, 0).as_double()))
            }
            "stringtofloat" => num(arg(args, 0).as_double()),
            // milliseconds, as getPosition, getPlayItemLength and getTimeOfDay give them, in whole seconds
            "integertotime" => str(format_time(arg(args, 0).as_int() / MS_PER_SECOND)),
            // hours, minutes and seconds (Wasabi SDK std.mi), from milliseconds as above
            "integertolongtime" => {
                let t = arg(args, 0).as_int() / MS_PER_SECOND;
                let a = t.abs();
                str(format!(
                    "{}{}:{:02}:{:02}",
                    if t < 0 { "-" } else { "" },
                    a / 3600,
                    a / 60 % 60,
                    a % 60
                ))
            }
            "integer" => int(crate::maki::value::double_to_int(
                arg(args, 0).as_double().round(),
            )),
            "frac" => {
                let d = arg(args, 0).as_double();
                num(d - d.floor())
            }
            "chr" => str(char::from_u32(arg(args, 0).as_int() as u32 & 0xffff)
                .map(|c| c.to_string())
                .unwrap_or_default()),
            "strlen" => int(crate::text::utf16_len(&arg(args, 0).as_string())),
            "strupper" => str(arg(args, 0).as_string().to_uppercase()),
            "strlower" => str(arg(args, 0).as_string().to_lowercase()),
            "strmid" => {
                let from = arg(args, 1).as_int().max(0) as usize;
                let n = arg(args, 2).as_int().max(0) as usize;
                str(take_units(&drop_units(&arg(args, 0).as_string(), from), n))
            }
            "strleft" => str(take_units(
                &arg(args, 0).as_string(),
                arg(args, 1).as_int().max(0) as usize,
            )),
            "strright" => str(take_last_units(
                &arg(args, 0).as_string(),
                arg(args, 1).as_int().max(0) as usize,
            )),
            "strsearch" => int(index_of_units(
                &arg(args, 0).as_string(),
                &arg(args, 1).as_string(),
            )),
            "gettoken" => {
                let s = arg(args, 0).as_string();
                let sep = arg(args, 1).as_string();
                let tokens: Vec<&str> = if sep.is_empty() {
                    vec![&s]
                } else {
                    s.split(&*sep).collect()
                };
                str(usize::try_from(arg(args, 2).as_int())
                    .ok()
                    .and_then(|i| tokens.get(i))
                    .map(|t| t.to_string())
                    .unwrap_or_default())
            }
            "getextension" => str(arg(args, 0)
                .as_string()
                .rsplit_once('.')
                .map(|(_, e)| e.to_string())
                .unwrap_or_default()),
            "removepath" => {
                let s = arg(args, 0).as_string();
                let after = s.rsplit('\\').next().unwrap_or(&s);
                str(after.rsplit('/').next().unwrap_or(after).to_string())
            }
            "getpath" => {
                let s = arg(args, 0).as_string();
                let before = s.rsplit_once('\\').map(|(b, _)| b).unwrap_or(&s);
                str(before
                    .rsplit_once('/')
                    .map(|(b, _)| b)
                    .unwrap_or(before)
                    .to_string())
            }
            "urlencode" | "urldecode" | "translate" => arg(args, 0),
            "getprivateint" => {
                let key = format!("{}/{}", arg(args, 0).as_string(), arg(args, 1).as_string())
                    .to_lowercase();
                int(self
                    .private_ints
                    .get(&key)
                    .copied()
                    .unwrap_or_else(|| arg(args, 2).as_int()))
            }
            "setprivateint" => {
                let key = format!("{}/{}", arg(args, 0).as_string(), arg(args, 1).as_string());
                self.store_private_int(&key, arg(args, 2).as_int());
                Value::Null
            }
            "getprivatestring" => {
                let key = format!("{}/{}", arg(args, 0).as_string(), arg(args, 1).as_string())
                    .to_lowercase();
                str(self
                    .private_strings
                    .get(&key)
                    .cloned()
                    .unwrap_or_else(|| arg(args, 2).as_string().to_string()))
            }
            "setprivatestring" => {
                let key = format!("{}/{}", arg(args, 0).as_string(), arg(args, 1).as_string());
                self.store_private_string(&key, &arg(args, 2).as_string());
                Value::Null
            }
            "getpublicint" => {
                let key = format!("public/{}", arg(args, 0).as_string()).to_lowercase();
                int(self
                    .private_ints
                    .get(&key)
                    .copied()
                    .unwrap_or_else(|| arg(args, 1).as_int()))
            }
            "setpublicint" => {
                self.store_private_int(
                    &format!("public/{}", arg(args, 0).as_string()),
                    arg(args, 1).as_int(),
                );
                Value::Null
            }
            "getpublicstring" => {
                let key = format!("public/{}", arg(args, 0).as_string()).to_lowercase();
                str(self
                    .private_strings
                    .get(&key)
                    .cloned()
                    .unwrap_or_else(|| arg(args, 1).as_string().to_string()))
            }
            "setpublicstring" => {
                self.store_private_string(
                    &format!("public/{}", arg(args, 0).as_string()),
                    &arg(args, 1).as_string(),
                );
                Value::Null
            }
            "messagebox" => {
                self.host.log(&format!(
                    "messagebox: {} [{}]",
                    arg(args, 0).as_string(),
                    arg(args, 1).as_string()
                ));
                int(0)
            }
            "debugstring" => {
                self.host
                    .log(&format!("debug: {}", arg(args, 0).as_string()));
                Value::Null
            }
            "getviewportwidth"
            | "getmonitorwidth"
            | "getviewportwidthfromguiobject"
            | "getviewportwidthfrompoint"
            | "getmonitorwidthfromguiobject"
            | "getmonitorwidthfrompoint" => int(self.viewport().0),
            "getviewportheight"
            | "getmonitorheight"
            | "getviewportheightfromguiobject"
            | "getviewportheightfrompoint"
            | "getmonitorheightfromguiobject"
            | "getmonitorheightfrompoint" => int(self.viewport().1),
            "getviewportleft"
            | "getviewporttop"
            | "getmonitorleft"
            | "getmonitortop"
            | "getviewportleftfromguiobject"
            | "getviewporttopfromguiobject"
            | "getmonitorleftfromguiobject"
            | "getmonitortopfromguiobject"
            | "getviewportleftfrompoint"
            | "getviewporttopfrompoint"
            | "getmonitorleftfrompoint"
            | "getmonitortopfrompoint" => int(0),
            "getcurappleft" => int(self
                .main_layout()
                .and_then(|l| self.objects[l].layout().map(|d| d.view_x))
                .unwrap_or(0)),
            "getcurapptop" => int(self
                .main_layout()
                .and_then(|l| self.objects[l].layout().map(|d| d.view_y))
                .unwrap_or(0)),
            "getcurappwidth" => int(self
                .main_layout()
                .map(|l| self.objects[l].base.rect.width)
                .unwrap_or(0)),
            "getcurappheight" => int(self
                .main_layout()
                .map(|l| self.objects[l].base.rect.height)
                .unwrap_or(0)),
            "getmouseposx" => int(self.last_pointer.0),
            "getmouseposy" => int(self.last_pointer.1),
            // milliseconds since local midnight, as Winamp gives it
            "gettimeofday" => {
                let now = self.now();
                int((now + self.zone_offset_at(now) as i64).rem_euclid(MS_PER_DAY) as i32)
            }
            "getdate" => int((self.now() / 1000) as i32),
            "random" => {
                let n = arg(args, 0).as_int();
                int(if n <= 0 {
                    0
                } else {
                    (self.random_unit() * n as f64) as i32
                })
            }
            "sin" => num(arg(args, 0).as_double().sin()),
            "cos" => num(arg(args, 0).as_double().cos()),
            "tan" => num(arg(args, 0).as_double().tan()),
            "asin" => num(arg(args, 0).as_double().asin()),
            "acos" => num(arg(args, 0).as_double().acos()),
            "atan" => num(arg(args, 0).as_double().atan()),
            "atan2" => num(arg(args, 0).as_double().atan2(arg(args, 1).as_double())),
            "pow" => num(arg(args, 0).as_double().powf(arg(args, 1).as_double())),
            "sqr" => {
                let d = arg(args, 0).as_double();
                num(d * d)
            }
            "sqrt" => num(arg(args, 0).as_double().sqrt()),
            "ln" => num(arg(args, 0).as_double().ln()),
            "log10" => num(arg(args, 0).as_double().log10()),
            "exp" => num(arg(args, 0).as_double().exp()),
            // links, downloads, the clipboard and files stay shut: a skin's script reaches only the skin
            "lockui"
            | "unlockui"
            | "ddesend"
            | "setcurrenttrackrating"
            | "navigateurl"
            | "enqueuefile"
            | "playfile"
            | "navigateurlbrowser"
            | "popmainbrowser"
            | "getmainbrowser"
            | "downloadurl"
            | "downloadmedia"
            | "setdownloadpath"
            | "hidewindow"
            | "selectfile"
            | "selectfolder"
            | "selectfilestart"
            | "selectfileend"
            | "setclipboardtext"
            | "setmenutransparency"
            | "systemmenu"
            | "windowmenu"
            | "triggeraction"
            | "popupmenu" => Value::Null,
            "isnamedwindowvisible" => {
                Value::Bool(self.named_window_visible(&arg(args, 0).as_string()))
            }
            "showwindow" => {
                self.show_host_window(&arg(args, 0).as_string());
                Value::Null
            }
            "isobjectvalid" => Value::Bool(arg(args, 0).as_object().is_some()),
            "istransparencyavailable" | "isdesktopalphaavailable" | "isproversion" => {
                Value::Bool(false)
            }
            "isvideo"
            | "isvideofullscreen"
            | "iskeydown"
            | "isminimized"
            | "getcurrenttrackrating"
            | "isappactive"
            | "isloadingskin"
            | "getplaylistcount"
            | "getfilesize" => int(0),
            "getwac" => Value::Null,
            "hasvideosupport" => int(1),
            "getdownloadpath" => Value::str(""),
            "getextfamily" => Value::str("Audio"),
            "getdecodername" => str(self.player.decoder_name()),
            "getalbumart" => int(0),
            "getskinpath" | "getskinpathinfo" => Value::str(""),
            _ => return None,
        })
    }

    /// A number in 0..1 for `random`: a plain generator stepped once per call.
    fn random_unit(&mut self) -> f64 {
        self.random_state ^= self.random_state << 13;
        self.random_state ^= self.random_state >> 7;
        self.random_state ^= self.random_state << 17;
        (self.random_state >> 11) as f64 / (1u64 << 53) as f64
    }

    /// How far the local time is from UTC at `at`, in ms: what the host told, else 0.
    fn zone_offset_at(&self, at: i64) -> i32 {
        (self.zone_offset)(at)
    }

    /// A script's `isNamedWindowVisible` of the window `id` names by GUID or alias.
    pub(crate) fn named_window_visible(&mut self, id: &str) -> bool {
        let bare = id.to_lowercase();
        let bare = bare.strip_prefix("guid:").unwrap_or(&bare).to_string();
        let kind = Self::window_kind(&bare);
        if kind == "avs" && self.vis_rect().is_some() {
            return true;
        }
        if let Some(open) = self.host.window_open(&kind) {
            return open;
        }
        self.toggle_target(&bare).is_some_and(|c| self.is_open(c))
    }

    /// The host closed its visualizer window, which stands in for the skin's own visualizer and
    /// video windows: each of those a script had it open goes back into the skin, as the skin's
    /// own window's Reattach button puts it (Winamp Modern's `button.vis.reattach`), the button
    /// in the skin's window for it whose id says "attach". The skin's window stays shut.
    pub fn visualizer_window_closed(&mut self) {
        for (kind, guid) in [("avs", super::AVS_GUID), ("video", super::VIDEO_GUID)] {
            let Some(at) = self.shown_for_skin.iter().position(|k| k == kind) else {
                continue;
            };
            self.shown_for_skin.remove(at);
            let Some(window) = self
                .containers
                .iter()
                .position(|c| SkinRuntime::holds(&c.attrs, guid))
            else {
                continue;
            };
            let layouts = self.containers[window].layouts.clone();
            if let Some(button) = layouts.iter().find_map(|l| self.attach_button(*l)) {
                self.button_click(button);
            }
        }
        self.invalidate();
    }

    /// The button under `o` whose id says "attach", if any.
    fn attach_button(&self, o: ObjId) -> Option<ObjId> {
        let obj = &self.objects[o];
        if matches!(obj.kind, Kind::Button(_)) && obj.base.id.to_lowercase().contains("attach") {
            return Some(o);
        }
        if obj.group.is_some() {
            return obj.children().iter().find_map(|c| self.attach_button(*c));
        }
        None
    }

    /// A script's `showWindow` of a window the app shows, named by GUID or alias: the app opens it.
    pub(crate) fn show_host_window(&mut self, id: &str) {
        let bare = id.to_lowercase();
        let bare = bare.strip_prefix("guid:").unwrap_or(&bare).to_string();
        let kind = Self::window_kind(&bare);
        if !super::HOST_WINDOWS.contains(&kind.as_str()) {
            return;
        }
        if self.host_actions {
            self.shown_for_skin.push(kind.clone());
            self.host.host_action("SHOW", Some(&kind));
        }
    }
}
