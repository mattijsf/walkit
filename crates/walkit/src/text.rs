// SPDX-License-Identifier: GPL-3.0-or-later

//! A line of text: a fixed string, or a player display such as the time or the song name. With
//! `ticker="1"` a string wider than the box scrolls.

use std::rc::Rc;

use crate::draw::{
    Align, BitmapFontDef, BitmapTextCmd, DrawCmd, FontDef, Rect, Rgb, TextCmd, VAlign,
};
use crate::gui::{to_bool, to_int_lenient, ObjId};
use crate::maki::Value;
use crate::player::Status;
use crate::runtime::SkinRuntime;

/// A text that names no size.
const DEFAULT_SIZE: i32 = 14;
const PAD: i32 = 2;
/// A text ticker moves on every third poll tick: the default "Text Ticker Speed" of 1/3.
pub(crate) const TEXT_SKIP: i32 = 2;
/// Strings whose width a text keeps.
const MAX_WIDTHS: usize = 4;
pub(crate) const ALTERNATE_MS: i64 = 1000;
pub(crate) const EMPTY_WIDTH: i32 = 32;
/// A songticker's step: "Text Ticker Increment" 1 x 13 / speed 1/3 ms.
pub(crate) const SONG_STEP_MS: i64 = 39;
pub(crate) const BOUNCE_PAUSE_MS: i32 = 2000;
pub(crate) const LOOP_GAP: &str = " *** ";

/// What a text object keeps.
#[derive(Debug, Clone)]
pub struct TextData {
    /// `text` / `default`.
    pub static_text: Option<String>,
    /// `display`, lowercase.
    pub display: Option<String>,
    /// `font`.
    pub font: Option<String>,
    /// `fontsize`.
    pub font_size: i32,
    /// The color; a text that names no color draws white.
    pub color: Rgb,
    /// A skin color by name.
    pub color_id: Option<String>,
    /// Winamp centers text vertically unless the skin says otherwise.
    pub valign: VAlign,
    /// `bold`.
    pub bold: bool,
    /// `shadowcolor`.
    pub shadow: Option<Rgb>,
    /// `shadowx`: a shadow is drawn only when one is not 0, at that offset.
    pub shadow_x: i32,
    /// `shadowy`.
    pub shadow_y: i32,
    /// `offsetx`: added to where the text is drawn.
    pub offset_x: i32,
    /// `offsety`.
    pub offset_y: i32,
    /// `forcelowercase`.
    pub force_lower: bool,
    /// `forcefixed`: a time's pieces are as wide as their characters times the width of "0".
    pub force_fixed: bool,
    /// `leftpadding`.
    pub left_padding: Option<i32>,
    /// `rightpadding`.
    pub right_padding: i32,
    /// `shadowcolor` as a skin color name.
    pub shadow_id: Option<String>,
    /// `timeroffstyle`.
    pub timer_off_style: i32,
    /// `align`.
    pub align: Align,
    /// `ticker`.
    pub ticker: bool,
    /// A `<songticker>`.
    pub song_ticker: bool,
    /// A songticker's `ticker="bounce"`.
    pub bounce: bool,
    /// `tickerstep`.
    pub ticker_step: i32,
    /// `timecolonwidth`.
    pub time_colon_width: Option<i32>,
    /// `showlen`.
    pub show_len: bool,
    /// `forceupcase`.
    pub force_upper: bool,
    /// A temporary message (setAlternateText).
    pub alternate_text: Option<String>,
    /// Text a script set, shown instead of the display until it sets "" again.
    pub script_text: Option<String>,
    /// The ticker's scroll.
    pub scroll: i32,
    /// The ticker's direction.
    pub scroll_dir: i32,
    /// Poll ticks the ticker pauses for.
    pub scroll_pause: i32,
    /// The text at the last tick.
    pub last_text: String,
    /// What a text that follows the player showed at the last tick.
    pub last_shown: Option<String>,
    /// When a temporary message clears.
    pub alternate_until: i64,
    /// When the ticker last stepped.
    pub ticked_at: i64,
    /// Poll ticks since the last move.
    pub skipped: i32,
    /// Whether the ticker holds still.
    pub held: bool,
    /// Widths measured for the last few strings, in the font as set now.
    pub widths: Vec<(String, i32)>,
    /// The text `loop_is` was measured for.
    pub loop_of: Option<String>,
    /// How far a looping songticker scrolls before its text starts over.
    pub loop_is: i32,
}

impl Default for TextData {
    fn default() -> Self {
        TextData {
            static_text: None,
            display: None,
            font: None,
            font_size: 0,
            color: Rgb(0xffffff),
            color_id: None,
            valign: VAlign::Center,
            bold: false,
            shadow: None,
            shadow_x: 0,
            shadow_y: 0,
            offset_x: 0,
            offset_y: 0,
            force_lower: false,
            force_fixed: false,
            left_padding: None,
            right_padding: 0,
            shadow_id: None,
            timer_off_style: 0,
            align: Align::Left,
            ticker: false,
            song_ticker: false,
            bounce: false,
            ticker_step: 1,
            time_colon_width: None,
            show_len: false,
            force_upper: false,
            alternate_text: None,
            script_text: None,
            scroll: 0,
            scroll_dir: 1,
            scroll_pause: 0,
            last_text: String::new(),
            last_shown: None,
            alternate_until: 0,
            ticked_at: -1,
            skipped: 0,
            held: false,
            widths: Vec::new(),
            loop_of: None,
            loop_is: 0,
        }
    }
}

impl TextData {
    /// A songticker: a text that shows the song name and its length unless told otherwise.
    pub fn song_ticker() -> Self {
        TextData {
            song_ticker: true,
            valign: VAlign::Top,
            display: Some("songname".into()),
            ticker: true,
            show_len: true,
            ..Default::default()
        }
    }

    fn is_time(&self) -> bool {
        matches!(
            self.display.as_deref(),
            Some("time") | Some("timeelapsed") | Some("timeremaining")
        )
    }

    fn effective_size(&self) -> i32 {
        if self.font_size > 0 {
            self.font_size
        } else {
            DEFAULT_SIZE
        }
    }

    fn colon_width(&self) -> Option<i32> {
        if self.is_time() {
            self.time_colon_width.filter(|w| *w >= 0)
        } else {
            None
        }
    }

    /// forceupcase, then forcelowercase: lower wins when both are set.
    fn cased(&self, t: String) -> String {
        let t = if self.force_upper {
            t.to_uppercase()
        } else {
            t
        };
        if self.force_lower {
            t.to_lowercase()
        } else {
            t
        }
    }

    /// A stopped time display by timeroffstyle.
    fn off_text(&self, remaining: bool) -> String {
        let minus = if remaining { "-" } else { "" };
        match self.timer_off_style {
            1 => format!("{minus}00:00"),
            2 => String::new(),
            3 => format!("{minus}0:00:00"),
            4 => " :  :  ".to_string(),
            _ => "  :  ".to_string(),
        }
    }

    /// The poll timer a text ticks on, in ms, and the ticks it pauses at each end; `None` for a
    /// text that never ticks: static texts and text feeds, `ticker` or not.
    fn poll_timer(&self) -> Option<(i32, i32)> {
        match self.display.as_deref() {
            Some(
                "songname" | "songinfo" | "songtitle" | "songartist" | "songalbum" | "songlength"
                | "songinfo_localise",
            ) => Some((25, 160)),
            Some("time" | "timeelapsed" | "timeremaining") => Some((250, 16)),
            Some("componentbucket") => Some((50, 80)),
            _ => None,
        }
    }

    /// Sets the attribute `key`; false for one a text does not take. Measures anew.
    pub(crate) fn set_param(&mut self, key: &str, value: &str) -> bool {
        self.widths.clear();
        self.loop_of = None;
        match key {
            "text" | "default" => self.static_text = Some(value.to_string()),
            "display" => self.display = Some(value.to_lowercase()),
            "font" => self.font = Some(value.to_string()),
            "fontsize" => self.font_size = to_int_lenient(value).unwrap_or(0),
            "color" | "textcolor" => match crate::gui::parse_rgb(value) {
                Some(c) => self.color = c,
                None => self.color_id = Some(value.to_string()),
            },
            "leftpadding" => self.left_padding = value.parse().ok(),
            "rightpadding" => self.right_padding = value.parse().unwrap_or(0),
            "shadowcolor" => match crate::gui::parse_rgb(value) {
                Some(c) => self.shadow = Some(c),
                None => self.shadow_id = Some(value.to_string()),
            },
            // Winamp knows top and center only: "bottom" and "middle" draw at the top
            // a songticker has no valign: it draws at the top of its box (winamp1)
            "valign" => {
                if !self.song_ticker {
                    self.valign = if value.eq_ignore_ascii_case("center") {
                        VAlign::Center
                    } else {
                        VAlign::Top
                    };
                }
            }
            "bold" => self.bold = to_bool(value),
            "timeroffstyle" => self.timer_off_style = to_int_lenient(value).unwrap_or(0),
            "align" => {
                self.align = match value.to_lowercase().as_str() {
                    "center" => Align::Center,
                    "right" => Align::Right,
                    _ => Align::Left,
                }
            }
            // a text reads the number only ("scroll" is off); a songticker reads the words
            "ticker" => {
                if self.song_ticker {
                    self.ticker = !value.eq_ignore_ascii_case("off");
                    self.bounce = value.eq_ignore_ascii_case("bounce");
                } else {
                    self.ticker = to_int_lenient(value).unwrap_or(0) != 0;
                }
            }
            "tickerstep" => self.ticker_step = to_int_lenient(value).unwrap_or(1),
            "timecolonwidth" => self.time_colon_width = to_int_lenient(value),
            "showlen" => self.show_len = to_bool(value),
            "forceupcase" | "forceuppercase" => self.force_upper = to_bool(value),
            "shadowx" => self.shadow_x = to_int_lenient(value).unwrap_or(0),
            "shadowy" => self.shadow_y = to_int_lenient(value).unwrap_or(0),
            "offsetx" => self.offset_x = to_int_lenient(value).unwrap_or(0),
            "offsety" => self.offset_y = to_int_lenient(value).unwrap_or(0),
            "forcelocase" | "forcelowercase" => self.force_lower = to_bool(value),
            "antialias" | "italic" | "underline" | "altfont" | "forcefixed" => {
                self.force_fixed = to_bool(value)
            }
            "wrap" | "nograb" | "fontbgcolor" | "ghost" | "timeroffset" | "dblclickaction"
            | "image" => {}
            _ => return false,
        }
        true
    }
}

fn long_time(ms: i32) -> String {
    let s = ms / 1000;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        time(ms)
    }
}

/// `m:ss` of `ms`.
pub fn time(ms: i32) -> String {
    let s = ms / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The length of `s` as Kotlin counts it: in UTF-16 units.
pub fn utf16_len(s: &str) -> i32 {
    s.encode_utf16().count() as i32
}

impl SkinRuntime {
    fn text(&self, id: ObjId) -> &TextData {
        match &self.objects[id].kind {
            crate::gui::Kind::Text(t) => t,
            _ => panic!("not a text"),
        }
    }

    fn text_mut(&mut self, id: ObjId) -> &mut TextData {
        match &mut self.objects[id].kind {
            crate::gui::Kind::Text(t) => t,
            _ => panic!("not a text"),
        }
    }

    /// What the text `id` shows now.
    pub(crate) fn current_text(&self, id: ObjId) -> String {
        let t = self.text(id);
        if let Some(a) = t.alternate_text.as_ref().filter(|a| !a.is_empty()) {
            return a.clone();
        }
        if let Some(s) = t.script_text.as_ref().filter(|s| !s.is_empty()) {
            return t.cased(s.clone());
        }
        let p = &*self.player;
        let stopped = p.status() == Status::Stopped;
        let text = match t.display.as_deref() {
            None | Some("") => t.static_text.clone().unwrap_or_default(),
            // the three are one display; stopped, they show the timeroffstyle
            Some("time" | "timeelapsed") => {
                if stopped {
                    t.off_text(false)
                } else {
                    time(p.position_ms())
                }
            }
            Some("timeremaining") => {
                if stopped {
                    t.off_text(true)
                } else {
                    format!("-{}", time(p.length_ms() - p.position_ms()))
                }
            }
            Some("songname" | "songtitle") => {
                let title = p.title();
                if stopped && title.is_empty() {
                    t.static_text.clone().unwrap_or_default()
                } else if t.show_len {
                    format!("{title} ({})", time(p.length_ms()))
                } else {
                    title
                }
            }
            Some("songlength") => time(p.length_ms()),
            Some("songinfo") => {
                if stopped {
                    String::new()
                } else {
                    format!(
                        "{}kbps {} {:.1}kHz",
                        p.bitrate_kbps(),
                        if p.channels() > 1 { "stereo" } else { "mono" },
                        p.sample_rate_khz() as f64
                    )
                }
            }
            Some("songartist") => p.tags().get("artist").cloned().unwrap_or_default(),
            Some("songalbum") => p.tags().get("album").cloned().unwrap_or_default(),
            Some("songbitrate") => {
                if stopped {
                    String::new()
                } else {
                    p.bitrate_kbps().to_string()
                }
            }
            Some("songsamplerate") => {
                if stopped {
                    String::new()
                } else {
                    p.sample_rate_khz().to_string()
                }
            }
            // the playlist editor's "selected / total" lengths
            Some("pe_info") => {
                let total: i32 = p
                    .tracks()
                    .iter()
                    .fold(0i32, |a, t| a.wrapping_add(t.length_ms));
                format!(
                    "{}/{}",
                    long_time(self.playlist_selected_ms),
                    long_time(total)
                )
            }
            Some("vid_info" | "componentbucket") => String::new(),
            // any other display is a text feed: its text, else the text attribute, never the name
            _ => t.static_text.clone().unwrap_or_default(),
        };
        let text = if text == ":componentname" {
            self.layout_object(id)
                .and_then(|l| self.objects[l].layout().and_then(|d| d.container))
                .map(|c| self.containers[c].name.clone())
                .unwrap_or_default()
        } else {
            text
        };
        t.cased(text)
    }

    fn text_bitmap_font(&mut self, id: ObjId) -> Option<Rc<BitmapFontDef>> {
        let font = self.text(id).font.clone()?;
        self.bitmap_font(&font)
    }

    /// A font the skin does not define is one installed on the system, named as is.
    fn text_ttf(&self, id: ObjId) -> Option<FontDef> {
        let font = self.text(id).font.as_deref()?;
        Some(
            self.font(font)
                .unwrap_or_else(|| FontDef::new(&font.to_lowercase(), "")),
        )
    }

    /// The width of `text` in the text's font, kept for the last few strings.
    pub(crate) fn text_width_of(&mut self, id: ObjId, text: &str) -> i32 {
        if let Some((_, w)) = self.text(id).widths.iter().find(|(s, _)| s == text) {
            return *w;
        }
        let w = self.measure_text(id, text);
        let t = self.text_mut(id);
        if t.widths.len() >= MAX_WIDTHS {
            t.widths.remove(0);
        }
        t.widths.push((text.to_string(), w));
        w
    }

    /// The width of the text shown now.
    pub(crate) fn text_width(&mut self, id: ObjId) -> i32 {
        let text = self.current_text(id);
        self.text_width_of(id, &text)
    }

    fn measure_text(&mut self, id: ObjId, text: &str) -> i32 {
        if let Some(f) = self.text_bitmap_font(id) {
            let advance = f.char_width + f.h_spacing;
            let colon = self.text(id).colon_width().unwrap_or(advance);
            // as measured (getTextWidth, auto width): the last glyph in full, no spacing after it. The
            // Winamp3 base skin's title bar script centers its title from this and lands 1 px off
            // otherwise; drawing a centered text still centers the sum of the advances (MMD3)
            let sum: i32 = text
                .chars()
                .map(|c| if c == ':' { colon } else { advance })
                .sum();
            return sum - if text.is_empty() { 0 } else { f.h_spacing };
        }
        let ttf = self.text_ttf(id);
        let t = self.text(id);
        let (size, bold) = (t.effective_size(), t.bold);
        self.host.text_width(text, ttf.as_ref(), size, bold)
    }

    /// A time's pieces between its colons, their widths and the colon slot.
    fn time_pieces(&mut self, id: ObjId, text: &str) -> (Vec<i32>, i32) {
        let ttf = self.text_ttf(id);
        let (size, bold, fixed, colon) = {
            let t = self.text(id);
            (t.effective_size(), t.bold, t.force_fixed, t.colon_width())
        };
        let mut w = |s: &str| self.host.text_width(s, ttf.as_ref(), size, bold);
        let zero = w("0");
        let widths = text
            .split(':')
            .map(|p| if fixed { utf16_len(p) * zero } else { w(p) })
            .collect();
        let cw = colon.unwrap_or_else(|| w(":"));
        (widths, cw)
    }

    /// The width a text asks for: the text plus 4, plus both paddings; a time laid out by its
    /// colon slot without the 4; 32 for no text. The height is the font's.
    pub(crate) fn text_intrinsic_size(&mut self, id: ObjId) -> (i32, i32) {
        let text = self.current_text(id);
        let bf = self.text_bitmap_font(id);
        let (pads, is_time) = {
            let t = self.text(id);
            (t.left_padding.unwrap_or(0) + t.right_padding, t.is_time())
        };
        let w = if text.is_empty() {
            EMPTY_WIDTH
        } else if is_time && text.contains(':') && bf.is_none() {
            let (widths, cw) = self.time_pieces(id, &text);
            widths.iter().sum::<i32>() + cw * (widths.len() as i32 - 1) + pads
        } else {
            self.text_width_of(id, &text) + 2 * PAD + pads
        };
        let h = bf
            .map(|f| f.char_height)
            .unwrap_or_else(|| self.text(id).effective_size());
        (w, h)
    }

    /// Moves a wide ticker by the time passed since `now` last came; true when something changed.
    pub(crate) fn text_tick(&mut self, id: ObjId, now: i64) -> bool {
        if self.text(id).alternate_text.is_some() && now >= self.text(id).alternate_until {
            self.text_mut(id).alternate_text = None;
            self.repaint();
        }
        let rect_w = self.objects[id].base.rect.width;
        if !self.text(id).ticker || rect_w <= 0 {
            return self.text_shown_changed(id);
        }
        let text = self.current_text(id);
        if text != self.text(id).last_text {
            let t = self.text_mut(id);
            let was_time = t.is_time() && !t.last_text.is_empty();
            t.last_text = text;
            t.ticked_at = now;
            // a time that counts on keeps its place; a new title starts over
            if !was_time || t.song_ticker {
                t.scroll = 0;
                t.scroll_dir = 1;
                t.scroll_pause = if t.song_ticker {
                    0
                } else {
                    t.poll_timer().map(|p| p.1).unwrap_or(0)
                };
                t.skipped = 0;
            }
            return true;
        }
        if self.text(id).ticked_at < 0 {
            self.text_mut(id).ticked_at = now;
        }
        let overflow = self.text_width_of(id, &text) - rect_w;
        if overflow <= 0 {
            self.text_hold(id, now);
            let t = self.text_mut(id);
            if t.scroll != 0 {
                t.scroll = 0;
                true
            } else {
                false
            }
        } else if self
            .text(id)
            .alternate_text
            .as_ref()
            .is_some_and(|a| !a.is_empty())
        {
            // a temporary message holds the ticker still
            self.text_hold(id, now)
        } else if self.text(id).song_ticker {
            self.song_step(id, now, &text)
        } else {
            self.text_step(id, now, overflow)
        }
    }

    /// Whether a text that follows the player now shows something else: a new second, a new bitrate.
    fn text_shown_changed(&mut self, id: ObjId) -> bool {
        if self.text(id).display.is_none() {
            return false;
        }
        let text = self.current_text(id);
        if self.text(id).last_shown.as_deref() == Some(text.as_str()) {
            return false;
        }
        self.text_mut(id).last_shown = Some(text);
        true
    }

    /// A text's ticker: on every third poll tick it moves `tickerstep` px, pausing 4 s at each
    /// end. While the music is stopped it holds at the start of the text; when it plays, it
    /// waits there its 4 s and scrolls on.
    fn text_step(&mut self, id: ObjId, now: i64, overflow: i32) -> bool {
        let Some((period, pause)) = self.text(id).poll_timer() else {
            return false;
        };
        if self.player.status() == Status::Stopped {
            return self.text_hold_at_start(id, now, pause);
        }
        self.text_resume(id, now, period as i64);
        let t = self.text_mut(id);
        let mut changed = false;
        // ticks Windows could not deliver in time are caught up
        while now - t.ticked_at >= period as i64 {
            t.ticked_at += period as i64;
            if t.scroll_pause > 0 {
                t.scroll_pause -= 1;
                continue;
            }
            let skipped = t.skipped;
            t.skipped += 1;
            if skipped < TEXT_SKIP {
                continue;
            }
            t.skipped = 0;
            t.scroll = (t.scroll + t.scroll_dir * t.ticker_step).clamp(0, overflow);
            changed = true;
            if t.scroll >= overflow || t.scroll <= 0 {
                t.scroll_dir = -t.scroll_dir;
                t.scroll_pause = pause;
            }
        }
        changed
    }

    /// A songticker: a loop with no pause, or a bounce with 2 s at each end, 1 px every 39 ms
    /// while playing; it holds where it is while paused or stopped.
    fn song_step(&mut self, id: ObjId, now: i64, text: &str) -> bool {
        if self.player.status() != Status::Playing {
            return self.text_hold(id, now);
        }
        self.text_resume(id, now, SONG_STEP_MS);
        let mut changed = false;
        while now - self.text(id).ticked_at >= SONG_STEP_MS {
            self.text_mut(id).ticked_at += SONG_STEP_MS;
            if !self.text(id).bounce {
                let period = self.loop_period(id, text);
                let t = self.text_mut(id);
                t.scroll = if period > 0 {
                    (t.scroll + 1) % period
                } else {
                    0
                };
                changed = true;
                continue;
            }
            if self.text(id).scroll_pause > 0 {
                self.text_mut(id).scroll_pause -= 1;
                continue;
            }
            let overflow = self.text_width_of(id, text) - self.objects[id].base.rect.width;
            let t = self.text_mut(id);
            t.scroll = (t.scroll + t.scroll_dir).clamp(0, overflow.max(0));
            changed = true;
            if t.scroll >= overflow || t.scroll <= 0 {
                t.scroll_dir = -t.scroll_dir;
                t.scroll_pause = BOUNCE_PAUSE_MS / SONG_STEP_MS as i32;
            }
        }
        changed
    }

    /// Holds the ticker where it is; false, as nothing moved.
    fn text_hold(&mut self, id: ObjId, now: i64) -> bool {
        let t = self.text_mut(id);
        t.held = true;
        t.ticked_at = now;
        false
    }

    /// Holds the ticker at the start of its text, to wait `start_pause` poll ticks there once it
    /// moves again. True when that moved it back.
    fn text_hold_at_start(&mut self, id: ObjId, now: i64, start_pause: i32) -> bool {
        self.text_hold(id, now);
        let t = self.text_mut(id);
        let moved = t.scroll != 0;
        t.scroll = 0;
        t.scroll_dir = 1;
        t.scroll_pause = start_pause;
        t.skipped = 0;
        moved
    }

    /// Ends a hold at `now`: the ticker's clock goes on from at most one `period` back.
    fn text_resume(&mut self, id: ObjId, now: i64, period: i64) {
        let t = self.text_mut(id);
        if !t.held {
            return;
        }
        t.held = false;
        t.ticked_at = t.ticked_at.max(now - period);
    }

    /// How far a looping songticker scrolls before `text` starts over: the text and the gap after it.
    fn loop_period(&mut self, id: ObjId, text: &str) -> i32 {
        if self.text(id).loop_of.as_deref() != Some(text) {
            let both = format!("{text}{LOOP_GAP}{text}");
            let w = self.text_width_of(id, &both) - self.text_width_of(id, text);
            let t = self.text_mut(id);
            t.loop_is = w;
            t.loop_of = Some(text.to_string());
        }
        self.text(id).loop_is
    }

    /// When, in the runtime's clock at `now`, the text next changes by itself with the player in
    /// `status`: a temporary message going back, or the ticker's next step. `None` when neither will.
    pub(crate) fn text_next_due_at(&mut self, id: ObjId, now: i64, status: Status) -> Option<i64> {
        let back = self
            .text(id)
            .alternate_text
            .as_ref()
            .map(|_| self.text(id).alternate_until);
        let Some(step) = self.text_next_step_at(id, now, status) else {
            return back;
        };
        Some(back.map_or(step, |b| b.min(step)))
    }

    fn text_next_step_at(&mut self, id: ObjId, now: i64, status: Status) -> Option<i64> {
        let rect_w = self.objects[id].base.rect.width;
        let moves = {
            let t = self.text(id);
            t.ticker
                && rect_w > 0
                && status != Status::Stopped
                && (!t.song_ticker || status == Status::Playing)
        };
        if !moves {
            return None;
        }
        let text = self.current_text(id);
        let overflow = self.text_width_of(id, &text) - rect_w;
        let t = self.text(id);
        if text != t.last_text || t.ticked_at < 0 {
            Some(now)
        } else if t.alternate_text.as_ref().is_some_and(|a| !a.is_empty()) {
            None
        } else if overflow <= 0 {
            if t.scroll != 0 {
                Some(now)
            } else {
                None
            }
        } else if t.held {
            // the first tick after a hold ends it
            Some(now)
        } else if t.song_ticker {
            Some(
                t.ticked_at
                    + SONG_STEP_MS
                        * if t.bounce {
                            t.scroll_pause as i64 + 1
                        } else {
                            1
                        },
            )
        } else {
            // the polls an end pause waits, then the polls skipped before a move
            t.poll_timer().map(|(period, _)| {
                t.ticked_at
                    + period as i64
                        * (t.scroll_pause as i64 + (TEXT_SKIP - t.skipped).max(0) as i64 + 1)
            })
        }
    }

    /// The text's drawing.
    pub(crate) fn draw_text(&mut self, id: ObjId, out: &mut Vec<DrawCmd>) {
        let rect = self.objects[id].base.rect;
        if !self.objects[id].base.visible || rect.width <= 0 || rect.height <= 0 {
            return;
        }
        let text = self.current_text(id);
        if text.is_empty() {
            return;
        }
        let wide = self.text(id).ticker && self.text_width_of(id, &text) > rect.width;
        let alpha = self.objects[id].base.effective_alpha;
        let t = self.text(id);
        let a = if wide { Align::Left } else { t.align };
        // a looping songticker draws the title twice, the second after " *** "
        let shown = if wide && t.song_ticker && !t.bounce {
            format!("{text}{LOOP_GAP}{text}")
        } else {
            text.clone()
        };
        if let Some(bf) = self.text_bitmap_font(id) {
            let t = self.text(id);
            // Winamp 5.666 keeps 2 px free left of bitmap text (timers and drawer tabs); a
            // songticker's leftpadding="5" rightpadding="-5" moves both edges instead, and
            // without padding it starts at the edge
            let lp = t
                .left_padding
                .unwrap_or(if t.song_ticker { 0 } else { PAD });
            let bx = Rect::new(
                rect.left + lp + t.offset_x,
                rect.top + t.offset_y,
                (rect.width - lp + t.right_padding).max(0),
                rect.height,
            );
            out.push(DrawCmd::BitmapText(Box::new(BitmapTextCmd {
                text: shown,
                font: bf,
                rect: bx,
                align: a,
                alpha,
                valign: t.valign,
                colon_width: t.colon_width(),
                scroll: if wide { t.scroll } else { 0 },
            })));
        } else {
            self.draw_true_type(id, out, &text, shown, wide, a, alpha);
        }
    }

    /// `shown` in a TrueType font, aligned by `a`: a time by its pieces, anything else in one run.
    #[allow(clippy::too_many_arguments)]
    fn draw_true_type(
        &mut self,
        id: ObjId,
        out: &mut Vec<DrawCmd>,
        text: &str,
        shown: String,
        wide: bool,
        a: Align,
        alpha: i32,
    ) {
        let rect = self.objects[id].base.rect;
        let color_id = self.text(id).color_id.clone();
        let c = color_id
            .and_then(|cid| self.color(&cid))
            .unwrap_or(self.text(id).color);
        // a shadow only with an offset, black unless shadowcolor says otherwise
        let sh = {
            let t = self.text(id);
            if t.shadow_x != 0 || t.shadow_y != 0 {
                let shadow_id = t.shadow_id.clone();
                Some(
                    t.shadow
                        .or_else(|| shadow_id.and_then(|s| self.color(&s)))
                        .unwrap_or(Rgb(0)),
                )
            } else {
                None
            }
        };
        let ttf = self.text_ttf(id);
        let t = self.text(id);
        // 2 px in, plus leftpadding unless a centered text fits; the right edge stays at the
        // box less 2; a songticker starts at its padding
        let text_rect = if t.song_ticker {
            let lp = t.left_padding.unwrap_or(0);
            Rect::new(
                rect.left + lp + t.offset_x,
                rect.top + t.offset_y,
                (rect.width - lp + t.right_padding).max(0),
                rect.height,
            )
        } else {
            let lp = if a == Align::Center && !wide {
                0
            } else {
                t.left_padding.unwrap_or(0)
            };
            Rect::new(
                rect.left + PAD + lp + t.offset_x,
                rect.top + t.offset_y,
                (rect.width - PAD).max(0),
                rect.height,
            )
        };
        let is_time = t.is_time();
        let make = |t: &TextData, text: String, rect: Rect, align: Align, scroll: i32| -> DrawCmd {
            DrawCmd::Text(Box::new(TextCmd {
                text,
                font: ttf.clone(),
                size: t.effective_size(),
                color: c,
                rect,
                align,
                scroll,
                alpha,
                valign: t.valign,
                bold: t.bold,
                shadow: sh,
                shadow_dx: t.shadow_x,
                shadow_dy: t.shadow_y,
                x: 0.0,
                baseline: 0.0,
            }))
        };
        if is_time && !wide && text.contains(':') {
            let pieces: Vec<String> = text.split(':').map(|s| s.to_string()).collect();
            let (widths, cw) = self.time_pieces(id, text);
            let t = self.text(id);
            let total: i32 = widths.iter().sum::<i32>() + cw * (pieces.len() as i32 - 1);
            let mut x = match a {
                Align::Left => rect.left + PAD + t.left_padding.unwrap_or(0),
                Align::Center => rect.left + (rect.width - total) / 2,
                Align::Right => rect.right() - t.right_padding - PAD - total,
            } + t.offset_x;
            for (i, piece) in pieces.iter().enumerate() {
                if !piece.is_empty() {
                    out.push(make(
                        t,
                        piece.clone(),
                        Rect::new(x, text_rect.top, widths[i], text_rect.height),
                        Align::Left,
                        0,
                    ));
                }
                x += widths[i];
                if i < pieces.len() - 1 {
                    out.push(make(
                        t,
                        ":".to_string(),
                        Rect::new(x, text_rect.top, cw, text_rect.height),
                        Align::Left,
                        0,
                    ));
                    x += cw;
                }
            }
        } else {
            let scroll = if wide { t.scroll } else { 0 };
            out.push(make(t, shown, text_rect, a, scroll));
        }
    }

    /// Runs `change` on what the text holds. It asks for a frame only when the text shown is now
    /// different, so a script that writes the same text again on a timer asks for none. Scripts
    /// hear `onTextChanged` with the text now shown either way.
    pub(crate) fn change_text(&mut self, id: ObjId, change: impl FnOnce(&mut TextData)) {
        let before = self.current_text(id);
        change(self.text_mut(id));
        let now = self.current_text(id);
        if now != before {
            self.repaint();
        }
        self.dispatch(
            crate::maki::Handle::Gui(id),
            "ontextchanged",
            &[Value::string(now)],
        );
    }

    /// A text's `setParam`, from the XML or a script: true when the text took it.
    pub(crate) fn text_set_param(&mut self, id: ObjId, key: &str, value: &str) -> bool {
        if key == "text" || key == "default" {
            self.text_mut(id).widths.clear();
            self.text_mut(id).loop_of = None;
            let v = value.to_string();
            self.change_text(id, |t| t.static_text = Some(v));
            return true;
        }
        self.text_mut(id).set_param(key, value)
    }

    /// A script setting `text` sets what shows, as setText does: on a display text it takes the
    /// display's place (Winamp3x's shaded timer writes "  :  " over its TIME display). True when handled here.
    pub(crate) fn text_set_xml_param(&mut self, id: ObjId, key: &str, value: &str) -> bool {
        if (key == "text" || key == "default")
            && self
                .text(id)
                .display
                .as_ref()
                .is_some_and(|d| !d.is_empty())
        {
            self.objects[id].base.set_attr(key, value);
            let v = value.to_string();
            self.change_text(id, |t| {
                t.script_text = Some(v);
                t.alternate_text = None;
            });
            return true;
        }
        false
    }

    /// The script methods of a text; `None` for one a text does not answer.
    pub(crate) fn text_maki_call(
        &mut self,
        id: ObjId,
        name: &str,
        args: &[Value],
    ) -> Option<Value> {
        Some(match name {
            "settext" => {
                let v = args
                    .first()
                    .map(|a| a.as_string().to_string())
                    .unwrap_or_default();
                self.change_text(id, |t| {
                    if t.display.as_ref().is_none_or(|d| d.is_empty()) {
                        t.static_text = Some(v);
                    } else {
                        t.script_text = Some(v);
                    }
                    // new text also ends a temporary message
                    t.alternate_text = None;
                });
                Value::Null
            }
            // a temporary message: it clears itself after a second; the same text again keeps that second
            "setalternatetext" => {
                let v = args
                    .first()
                    .map(|a| a.as_string().to_string())
                    .unwrap_or_default();
                let now = self.now();
                let t = self.text_mut(id);
                if t.alternate_text.as_deref() != Some(v.as_str()) {
                    t.alternate_until = now + ALTERNATE_MS;
                }
                t.alternate_text = Some(v);
                self.repaint();
                Value::Null
            }
            "gettext" => Value::string(self.current_text(id)),
            // the width the text asks for, its 2 px padding on both sides included
            "gettextwidth" => Value::Int(self.text_width(id) + 2 * PAD),
            _ => return None,
        })
    }
}
