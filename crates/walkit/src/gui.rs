// SPDX-License-Identifier: GPL-3.0-or-later

//! The GUI objects of a loaded skin: what each kind of object keeps, held in one arena by the
//! runtime and reached by index. The runtime does the work on them (`runtime`), as every
//! object's behavior reaches into the skin around it.

use std::rc::Rc;

use crate::draw::{Rect, Rgb};
use crate::host::WindowId;
use crate::player::Status;
use crate::text::TextData;

/// The index of a GUI object in the runtime's arena.
pub type ObjId = usize;

/// The index of a container in the runtime's containers.
pub type ContainerId = usize;

/// `"1"` or `"true"` (any case).
pub fn to_bool(v: &str) -> bool {
    v == "1" || v.eq_ignore_ascii_case("true")
}

/// A number as Kotlin's `toIntOrNull() ?: toDoubleOrNull()?.toInt()` reads it, trimmed.
pub fn to_int_lenient(v: &str) -> Option<i32> {
    let t = v.trim();
    t.parse::<i32>()
        .ok()
        .or_else(|| crate::maki::value::parse_double(t).map(crate::maki::value::double_to_int))
}

/// A color as Winamp writes it: "r,g,b", "#rrggbb", or "white" / "black".
pub fn parse_rgb(v: &str) -> Option<Rgb> {
    let name = v.trim().to_lowercase();
    let parts: Vec<Option<i32>> = v.split(',').map(|p| p.trim().parse::<i32>().ok()).collect();
    if name == "white" {
        return Some(Rgb(0xffffff));
    }
    if name == "black" {
        return Some(Rgb(0));
    }
    if parts.len() >= 3 && parts.iter().all(|p| p.is_some()) {
        let r = parts[0].unwrap_or(0);
        let g = parts[1].unwrap_or(0);
        let b = parts[2].unwrap_or(0);
        return Some(Rgb(((r << 16) | (g << 8) | b) as u32));
    }
    let hex = v.trim().strip_prefix('#').unwrap_or(v.trim());
    i64::from_str_radix(hex, 16)
        .ok()
        .filter(|_| !hex.is_empty())
        .map(|n| Rgb((n & 0xffffff) as u32))
}

/// What every GUI object keeps: its place in the XML tree, its geometry attributes, its look
/// flags, its attributes as written, where it was last laid out, and a target slide.
#[derive(Debug, Clone)]
pub struct GuiBase {
    /// The XML tag it was made from.
    pub tag: String,
    /// Its id, lowercase.
    pub id: String,
    /// The group it is in.
    pub parent: Option<ObjId>,
    /// `x`, as written.
    pub x: Option<i32>,
    /// `y`.
    pub y: Option<i32>,
    /// `w`.
    pub w: Option<i32>,
    /// `h`.
    pub h: Option<i32>,
    /// `default_w` / `default_h`: the size of a group that writes none.
    pub default_w: Option<i32>,
    /// See `default_w`.
    pub default_h: Option<i32>,
    /// `relat*`: 0 the value itself, 1 the value plus the parent's size, 2 a percentage of the parent's size.
    pub relat_x: i32,
    /// See `relat_x`.
    pub relat_y: i32,
    /// See `relat_x`.
    pub relat_w: i32,
    /// See `relat_x`.
    pub relat_h: i32,
    /// Shown.
    pub visible: bool,
    /// `activealpha`: opacity while the window has focus.
    pub active_alpha: i32,
    /// `inactivealpha`: opacity while it has not.
    pub inactive_alpha: i32,
    /// Lets touches through.
    pub ghost: bool,
    /// Takes touches and draws at full strength.
    pub enabled: bool,
    /// How its region joins the window's shape.
    pub sysregion: i32,
    /// Whether the XML wrote sysregion: a group that did not takes one from its children.
    pub sysregion_set: bool,
    /// `tooltip`.
    pub tooltip: Option<String>,
    /// `dblclickaction`.
    pub dbl_click_action: Option<String>,
    /// `move`: whether a drag that starts on it moves the window; `None` when not written.
    pub move_window: Option<bool>,
    /// A title bar's insets from the frame's `padtitleleft`.
    pub pad_left: i32,
    /// See `pad_left`.
    pub pad_right: i32,
    /// Every attribute set, in the order first set; a key set again keeps its place.
    pub attrs: Vec<(String, String)>,
    /// `resize` as the skin's XML writes it, before any script changes it.
    pub declared_resize: Option<String>,
    /// Where it was last laid out, in layout pixels.
    pub rect: Rect,
    /// The alpha the ancestors leave, 0..255.
    pub effective_alpha: i32,
    /// `rectrgn="1"`: the object's region is its rectangle, for the window shape and for taps.
    pub rect_rgn: bool,
    /// Target x, y, w, h and alpha set by `setTarget*`; `None` where none was set.
    pub target: [Option<i32>; 5],
    /// `setTargetSpeed`, in seconds.
    pub target_seconds: f64,
    /// Where a target slide started from.
    pub move_from: Option<[i32; 5]>,
    /// Where it goes.
    pub move_to: Option<[Option<i32>; 5]>,
    /// When it started, or `None` while none runs.
    pub move_start: Option<i64>,
    /// How long it takes.
    pub move_millis: i64,
}

impl GuiBase {
    /// A fresh object made from `tag`.
    pub fn new(tag: &str) -> Self {
        GuiBase {
            tag: tag.to_string(),
            id: String::new(),
            parent: None,
            x: None,
            y: None,
            w: None,
            h: None,
            default_w: None,
            default_h: None,
            relat_x: 0,
            relat_y: 0,
            relat_w: 0,
            relat_h: 0,
            visible: true,
            active_alpha: 255,
            inactive_alpha: 255,
            ghost: false,
            enabled: true,
            sysregion: 0,
            sysregion_set: false,
            tooltip: None,
            dbl_click_action: None,
            move_window: None,
            pad_left: 0,
            pad_right: 0,
            attrs: Vec::new(),
            declared_resize: None,
            rect: Rect::EMPTY,
            effective_alpha: 255,
            rect_rgn: false,
            target: [None; 5],
            target_seconds: 0.0,
            move_from: None,
            move_to: None,
            move_start: None,
            move_millis: 0,
        }
    }

    /// `alpha`: the alpha with and without focus together.
    pub fn alpha(&self) -> i32 {
        self.active_alpha
    }

    /// Sets both alphas.
    pub fn set_alpha(&mut self, v: i32) {
        self.active_alpha = v;
        self.inactive_alpha = v;
    }

    /// The attribute `key` as last set.
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Sets attribute `key`, keeping its place when it is there.
    pub fn set_attr(&mut self, key: &str, value: &str) {
        match self.attrs.iter_mut().find(|(k, _)| k == key) {
            Some(entry) => entry.1 = value.to_string(),
            None => self.attrs.push((key.to_string(), value.to_string())),
        }
    }

    /// What the paint alpha reads of the object itself: its two alphas and whether it is enabled.
    pub fn paint_key(&self) -> i32 {
        (self.active_alpha << 9) | (self.inactive_alpha << 1) | (self.enabled as i32)
    }

    /// What the geometry parameters set, to tell whether one of them changed anything.
    pub fn placement(&self) -> [Option<i32>; 12] {
        [
            self.x,
            self.y,
            self.w,
            self.h,
            self.default_w,
            self.default_h,
            Some(self.relat_x),
            Some(self.relat_y),
            Some(self.relat_w),
            Some(self.relat_h),
            Some(self.pad_left),
            Some(self.pad_right),
        ]
    }
}

/// What a group keeps: its children and background, and what Wasabi's xui machinery adds.
#[derive(Debug, Clone, Default)]
pub struct GroupData {
    /// The children, in drawing order.
    pub children: Vec<ObjId>,
    /// `background`: the picture behind the children.
    pub background: Option<String>,
    /// Off until `background=` turns it on, so a later `drawbackground="0"` wins.
    pub draw_background: bool,
    /// `instance_id`.
    pub instance_id: Option<String>,
    /// `autowidthsource`.
    pub auto_width_source: Option<String>,
    /// `autoheightsource`.
    pub auto_height_source: Option<String>,
    /// The scripts that live in this group, by System object index.
    pub system_objects: Vec<usize>,
    /// What the engine does for this group where Winamp's default set runs a script.
    pub behavior: Option<Behavior>,
    /// Attributes a xui tag got that are no GUI attributes; sent to its scripts as `onSetXuiParam`.
    pub xui_params: Vec<(String, String)>,
    /// `embed_xui` on a group definition: the id of the child the xui object exposes to scripts.
    pub embed_xui: Option<String>,
}

/// What the engine does for a default-set group where Winamp's default set runs a script.
#[derive(Debug, Clone)]
pub enum Behavior {
    /// A standard frame: `content` names the group placed in its client area.
    StandardFrame {
        /// `contentrect`: x, y, w, h of the client area.
        area: [i32; 4],
    },
    /// A title bar: the window's name centered, with a streak either side.
    TitleBar {
        /// Padding on the left, added up as set.
        pad_left: i32,
        /// Padding on the right.
        pad_right: i32,
    },
}

/// What a layout (one look of a container) keeps besides its group.
#[derive(Debug, Clone)]
pub struct LayoutData {
    /// How the host names this window.
    pub window_id: WindowId,
    /// The container it belongs to.
    pub container: Option<ContainerId>,
    /// `minimum_w`.
    pub min_w: i32,
    /// `minimum_h`.
    pub min_h: i32,
    /// `maximum_w`.
    pub max_w: i32,
    /// `maximum_h`.
    pub max_h: i32,
    /// The size the listener gave the window by dragging its resize grip; `None` until then.
    pub user_size: Option<(i32, i32)>,
    /// `desktopalpha="1"`: drawn with per-pixel transparency and no region.
    pub desktop_alpha: bool,
    /// `snapadjustleft`.
    pub snap_left: i32,
    /// `snapadjusttop`.
    pub snap_top: i32,
    /// `snapadjustright`.
    pub snap_right: i32,
    /// `snapadjustbottom`.
    pub snap_bottom: i32,
    /// Where the window sits in the viewport while it shows this layout.
    pub view_x: i32,
    /// Whether the window has a spot of its own among windows set out apart (see `runtime::free`).
    pub free_placed: bool,
    /// See `view_x`.
    pub view_y: i32,
    /// Which side edge of the viewport a script last moved the window against.
    pub edge_x: Option<crate::host::Edge>,
    /// As `edge_x`, up and down.
    pub edge_y: Option<crate::host::Edge>,
    /// `linkwidth`: the layout of the same container whose width this one shares.
    pub link_width: Option<String>,
    /// `linkheight`.
    pub link_height: Option<String>,
    /// The window's composed region, worked out once per change (see `composed_at`).
    pub composed: Option<crate::draw::Region>,
    /// The whole-window cuts of `composed`.
    pub cuts: Vec<crate::draw::Region>,
    /// The change count `composed` was worked out at.
    pub composed_at: i64,
    /// The window shape, worked out once per change.
    pub shape: Option<crate::draw::Region>,
    /// The change count `shape` was worked out at.
    pub shape_at: i64,
    /// `contentRight`, worked out once per change.
    pub right_now: i32,
    /// See `right_now`.
    pub right_at: i64,
    /// `contentBottom`, worked out once per change.
    pub bottom_now: i32,
    /// See `bottom_now`.
    pub bottom_at: i64,
}

impl LayoutData {
    /// A layout with window id `window_id`.
    pub fn new(window_id: WindowId) -> Self {
        LayoutData {
            window_id,
            container: None,
            min_w: 0,
            min_h: 0,
            max_w: 0,
            max_h: 0,
            user_size: None,
            desktop_alpha: false,
            snap_left: 0,
            snap_top: 0,
            snap_right: 0,
            snap_bottom: 0,
            view_x: 0,
            free_placed: false,
            view_y: 0,
            edge_x: None,
            edge_y: None,
            link_width: None,
            link_height: None,
            composed: None,
            cuts: Vec::new(),
            composed_at: -1,
            shape: None,
            shape_at: -1,
            right_now: 0,
            right_at: -1,
            bottom_now: 0,
            bottom_at: -1,
        }
    }
}

/// A picture.
#[derive(Debug, Clone, Default)]
pub struct LayerData {
    /// `image`.
    pub image: Option<String>,
    /// `tile="1"` repeats the picture across the layer; otherwise it is stretched to fit.
    pub tile: bool,
    /// `inactiveimage`: drawn instead of the image while the window has no focus.
    pub inactive_image: Option<String>,
}

/// A strip of frames.
#[derive(Debug, Clone)]
pub struct AnimData {
    /// `framewidth`.
    pub frame_w: Option<i32>,
    /// `frameheight`.
    pub frame_h: Option<i32>,
    /// The frame shown.
    pub frame: i32,
    /// Set by frameheight (true) or framewidth (false), the last one written; else from the picture.
    pub vertical_set: Option<bool>,
    /// `start`.
    pub start_frame: i32,
    /// `end`.
    pub end_frame: Option<i32>,
    /// `speed`, ms a frame.
    pub speed: i32,
    /// `autoreplay`.
    pub auto_replay: bool,
    /// `autoplay`.
    pub auto_play: bool,
    /// Playing.
    pub playing: bool,
    /// Paused while playing.
    pub paused: bool,
    /// The step, 1 or -1.
    pub step: i32,
    /// When it steps on.
    pub next_at: i64,
}

impl Default for AnimData {
    fn default() -> Self {
        AnimData {
            frame_w: None,
            frame_h: None,
            frame: 0,
            vertical_set: None,
            start_frame: 0,
            end_frame: None,
            speed: 200,
            auto_replay: true,
            auto_play: false,
            playing: false,
            paused: false,
            step: 1,
            next_at: 0,
        }
    }
}

/// How a button toggles: a plain push, a toggle with a config attribute, or n states.
#[derive(Debug, Clone)]
pub enum ButtonKind {
    /// A push button.
    Plain,
    /// A button that stays down: shuffle, repeat, EQ on. Its state lives in a config attribute.
    Toggle {
        /// `cfgattrib`.
        cfg_attrib: Option<String>,
        /// `cfgval`: the config value that means on (default 1).
        cfg_val: i32,
        /// `autotoggle="0"`: a tap does not flip it; scripts do.
        auto_toggle: bool,
    },
    /// A button that steps through `nstates` states.
    NStates {
        /// `nstates`.
        states: i32,
        /// `cfgvals`.
        values: Option<Vec<i32>>,
        /// `cfgattrib`.
        cfg_attrib: Option<String>,
        /// The state without a config attribute or an action state.
        local_state: i32,
        /// `autoelements="0"`: the pictures keep their names instead of taking the state number.
        auto_elements: bool,
    },
}

/// A push button with an action, such as PLAY, or a script listening to `onLeftClick`.
#[derive(Debug, Clone)]
pub struct ButtonData {
    /// Which kind of button.
    pub kind: ButtonKind,
    /// `image`.
    pub image: Option<String>,
    /// `downimage`.
    pub down_image: Option<String>,
    /// `hoverimage`.
    pub hover_image: Option<String>,
    /// `activeimage`.
    pub active_image: Option<String>,
    /// `action`.
    pub action: Option<String>,
    /// `param`.
    pub param: Option<String>,
    /// `action_target`.
    pub action_target: Option<String>,
    /// Held down.
    pub pressed: bool,
    /// Activated by a script.
    pub activated: bool,
    /// `borders`: Winamp's button frame round the picture.
    pub borders: bool,
    /// `center_image`.
    pub center_image: bool,
    /// `text`: a label Winamp draws on the button.
    pub label: Option<String>,
    /// The shown state as the last frame drew it, or as last heard of; `None` before the first frame.
    pub drawn_state: Option<i32>,
}

impl ButtonData {
    /// A button of `kind`.
    pub fn new(kind: ButtonKind) -> Self {
        ButtonData {
            kind,
            image: None,
            down_image: None,
            hover_image: None,
            active_image: None,
            action: None,
            param: None,
            action_target: None,
            pressed: false,
            activated: false,
            borders: false,
            center_image: false,
            label: None,
            drawn_state: None,
        }
    }
}

/// A thumb on a bar.
#[derive(Debug, Clone)]
pub struct SliderData {
    /// `action`, lowercase.
    pub action: Option<String>,
    /// `param`.
    pub param: Option<String>,
    /// `thumb`.
    pub thumb: Option<String>,
    /// `downthumb`.
    pub down_thumb: Option<String>,
    /// `barleft`.
    pub bar_left: Option<String>,
    /// `barmiddle`.
    pub bar_middle: Option<String>,
    /// `barright`.
    pub bar_right: Option<String>,
    /// `orientation="v"`.
    pub vertical: bool,
    /// `low`.
    pub low: i32,
    /// `high`.
    pub high: i32,
    /// Whether `low` was written.
    pub low_set: bool,
    /// Whether `high` was written.
    pub high_set: bool,
    /// The position, `low..=high`.
    pub position: i32,
    /// Held by the finger.
    pub dragging: bool,
    /// `lock()`: takes no touch.
    pub locked: bool,
    /// Where in the thumb the finger holds it, along the bar.
    pub grab: i32,
    /// Whether it followed the player at least once.
    pub synced: bool,
}

impl Default for SliderData {
    fn default() -> Self {
        SliderData {
            action: None,
            param: None,
            thumb: None,
            down_thumb: None,
            bar_left: None,
            bar_middle: None,
            bar_right: None,
            vertical: false,
            low: 0,
            high: 255,
            low_set: false,
            high_set: false,
            position: 0,
            dragging: false,
            locked: false,
            grab: 0,
            synced: false,
        }
    }
}

/// The stop, play and pause light.
#[derive(Debug, Clone, Default)]
pub struct StatusData {
    /// `stopbitmap`.
    pub stop: Option<String>,
    /// `playbitmap`.
    pub play: Option<String>,
    /// `pausebitmap`.
    pub pause: Option<String>,
}

impl StatusData {
    /// The picture for `status`.
    pub fn image(&self, status: Status) -> Option<&str> {
        match status {
            Status::Stopped => self.stop.as_deref(),
            Status::Playing => self.play.as_deref(),
            Status::Paused => self.pause.as_deref(),
        }
    }
}

/// A stretchable background: corners and edges at their natural size, the middle stretched.
#[derive(Debug, Clone, Default)]
pub struct GridData {
    /// The nine parts by name.
    pub parts: Vec<(String, String)>,
}

/// `<ProgressGrid>`: a grid over the played part of its rectangle.
#[derive(Debug, Clone)]
pub struct ProgressData {
    /// `orientation`.
    pub orientation: String,
    /// The filled length as the last frame drew it.
    pub drawn_filled: i32,
    /// The play position `next_fill_at` last saw.
    pub asked_position: i32,
    /// The filled length it last saw.
    pub asked_filled: i32,
    /// The time it gave for them.
    pub asked_due: i64,
}

impl Default for ProgressData {
    fn default() -> Self {
        ProgressData {
            orientation: "right".into(),
            drawn_filled: -1,
            asked_position: -1,
            asked_filled: -1,
            asked_due: 0,
        }
    }
}

/// `<rect>`: a rectangle in one color, filled or as a 1 px outline.
#[derive(Debug, Clone, Default)]
pub struct RectData {
    /// The color written out.
    pub color: Rgb,
    /// A skin color by name.
    pub color_id: Option<String>,
    /// `filled`.
    pub filled: bool,
}

/// `<images>`: the classic skins' volume and balance bars.
#[derive(Debug, Clone, Default)]
pub struct ImagesData {
    /// `images`: the strip.
    pub image: Option<String>,
    /// `imagesspacing`.
    pub spacing: i32,
    /// `source`, lowercase.
    pub source: String,
}

/// A menu bar entry.
#[derive(Debug, Clone, Default)]
pub struct MenuData {
    /// `menu`: the menu it opens, as Winamp names it.
    pub menu: Option<String>,
    /// `normal`: the object shown while not pressed.
    pub normal: Option<String>,
    /// `down`: the object shown while pressed.
    pub down: Option<String>,
}

/// `<Wasabi:Frame>`: two groups side by side or stacked with a divider between them.
#[derive(Debug, Clone)]
pub struct FrameData {
    /// `orientation="v"`: side by side.
    pub vertical: bool,
    /// `from="right"` / `"bottom"`: the position counts from the far side.
    pub from_far: bool,
    /// `width` / `height`: where the divider sits.
    pub pos: i32,
    /// `minwidth` / `minheight`.
    pub min_pos: i32,
    /// `maxwidth` / `maxheight`; a negative maximum counts from the far edge.
    pub max_pos: Option<i32>,
    /// The first pane.
    pub first: Option<ObjId>,
    /// The second pane.
    pub second: Option<ObjId>,
    /// The divider picture ("" for none; `None` for the default set's).
    pub divider: Option<String>,
    /// The grabber picture.
    pub grabber: Option<String>,
    /// Whether there is a divider at all.
    pub resizable: bool,
    /// Where the divider was last laid out.
    pub sizer_rect: Rect,
}

impl Default for FrameData {
    fn default() -> Self {
        FrameData {
            vertical: true,
            from_far: false,
            pos: 128,
            min_pos: 0,
            max_pos: None,
            first: None,
            second: None,
            divider: None,
            grabber: None,
            resizable: true,
            sizer_rect: Rect::EMPTY,
        }
    }
}

/// `<componentbucket>`: a strip of the groups whose groupdef has its `wndtype`.
#[derive(Debug, Clone, Default)]
pub struct BucketData {
    /// `wndtype`, lowercase.
    pub window_type: Option<String>,
    /// `vertical="1"`: stacked.
    pub vertical: bool,
    /// `spacing`.
    pub spacing: i32,
    /// `leftmargin`.
    pub left_margin: i32,
    /// `rightmargin`.
    pub right_margin: i32,
    /// How far the entries are scrolled, in pixels along the strip.
    pub scroll: i32,
}

/// `<groupxfade>`: shows the group its `groupid` names, fading from the one it showed before.
#[derive(Debug, Clone)]
pub struct XFadeData {
    /// `speed`, seconds.
    pub speed: f64,
    /// The pages made so far, by lowercase group id.
    pub pages: Vec<(String, ObjId)>,
    /// The page shown.
    pub current: Option<ObjId>,
}

impl Default for XFadeData {
    fn default() -> Self {
        XFadeData {
            speed: 0.25,
            pages: Vec::new(),
            current: None,
        }
    }
}

/// How the oscilloscope draws its points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeStyle {
    /// A dot per column.
    Dots,
    /// The points joined into a line.
    Line,
    /// Filled from each point to the middle.
    Filled,
}

/// `<vis>`: Winamp's analyzer and oscilloscope.
#[derive(Debug, Clone)]
pub struct VisData {
    /// Row colors, top row first.
    pub rows: [Rgb; 16],
    /// `colorbandpeak`.
    pub peak_color: Rgb,
    /// Oscilloscope colors, colorosc1 (the middle rows) to colorosc5 (the outer rows).
    pub scope_colors: [Rgb; 5],
    /// `gammagroup`.
    pub gamma_group: Option<String>,
    /// 1 spectrum, 2 oscilloscope; any other value shows nothing.
    pub mode: i32,
    /// `oscstyle`.
    pub scope_style: ScopeStyle,
    /// `peaks`.
    pub peaks: bool,
    /// `bandwidth="thin"`.
    pub thin: bool,
    /// `flipv`.
    pub flip_v: bool,
    /// `fliph`.
    pub flip_h: bool,
    /// `falloff`, 0..4.
    pub falloff: i32,
    /// `peakfalloff`, 0..4.
    pub peak_falloff: i32,
    /// Per data column: bar height in rows.
    pub bars: [f32; 75],
    /// Per data column: peak height in 1/256 rows.
    pub peak: [f32; 75],
    /// Per data column: peak speed.
    pub speed: [f32; 75],
    /// When the bars last stepped.
    pub last_frame_at: i64,
    /// The oscilloscope's last 25 ms step.
    pub scope_at: i64,
    /// Whether the oscilloscope showed the flat line of no waveform then.
    pub scope_flat: bool,
}

/// Winamp's classic analyzer colors, top row (red) to bottom row (green).
pub const CLASSIC_BANDS: [u32; 16] = [
    0xEF3110, 0xCE2910, 0xD65A00, 0xD66600, 0xD67300, 0xC67B08, 0xDEA518, 0xD6B521, 0xBDDE29,
    0x94DE21, 0x29CE10, 0x32BE10, 0x39B510, 0x319C08, 0x299400, 0x188408,
];

/// Winamp's classic oscilloscope colors, colorosc1 (white) to colorosc5.
pub const CLASSIC_SCOPE: [u32; 5] = [0xFFFFFF, 0xD6D6DE, 0xB5BDBD, 0xA0AAAF, 0x949CA5];

impl Default for VisData {
    fn default() -> Self {
        VisData {
            rows: CLASSIC_BANDS.map(Rgb),
            peak_color: Rgb(0x959596),
            scope_colors: CLASSIC_SCOPE.map(Rgb),
            gamma_group: None,
            mode: 1,
            scope_style: ScopeStyle::Line,
            peaks: true,
            thin: false,
            flip_v: false,
            flip_h: false,
            falloff: 2,
            peak_falloff: 1,
            bars: [0.0; 75],
            peak: [0.0; 75],
            speed: [3.0; 75],
            last_frame_at: -1,
            scope_at: -1,
            scope_flat: true,
        }
    }
}

/// `<eqvis>`: the EQ curve.
#[derive(Debug, Clone)]
pub struct EqVisData {
    /// `colortop`.
    pub top: Rgb,
    /// `colormiddle`.
    pub middle: Rgb,
    /// `colorbottom`.
    pub bottom: Rgb,
    /// `colorpreamp`.
    pub preamp: Rgb,
    /// `gammagroup`.
    pub gamma_group: Option<String>,
}

impl Default for EqVisData {
    fn default() -> Self {
        EqVisData {
            top: Rgb(0xffffff),
            middle: Rgb(0xffffff),
            bottom: Rgb(0xffffff),
            preamp: Rgb(0x888888),
            gamma_group: None,
        }
    }
}

/// `<ColorThemes:List>`: the skin's color themes as a list.
#[derive(Debug, Clone)]
pub struct ThemeListData {
    /// The highlighted row's theme: the one in use until the user taps another.
    pub selected: Option<String>,
    /// Whether the column header shows.
    pub header: bool,
    /// Whether the horizontal scrollbar shows.
    pub hscroll: bool,
    /// The first visible row, in pixels from the top of the rows.
    pub scroll_px: i32,
    /// The theme the list last followed.
    pub shown_for: Option<String>,
    /// Where the finger went down.
    pub down_y: i32,
    /// The scroll then.
    pub down_scroll: i32,
    /// Whether the finger dragged.
    pub dragged: bool,
}

impl Default for ThemeListData {
    fn default() -> Self {
        ThemeListData {
            selected: None,
            header: true,
            hscroll: true,
            scroll_px: 0,
            shown_for: None,
            down_y: 0,
            down_scroll: 0,
            dragged: false,
        }
    }
}

/// `<gradient>`: a picture Winamp makes from color stops.
#[derive(Debug, Clone)]
pub struct GradientData {
    /// `gradient_x1`.
    pub x1: f32,
    /// `gradient_y1`.
    pub y1: f32,
    /// `gradient_x2`.
    pub x2: f32,
    /// `gradient_y2`.
    pub y2: f32,
    /// The stops: position 0..1 and ARGB, sorted by position.
    pub stops: Vec<(f32, u32)>,
    /// `gammagroup`.
    pub gamma_group: Option<String>,
    /// The picture last made, and the size and tint it was made for.
    pub made: Option<Rc<[u32]>>,
    /// See `made`.
    pub made_for: Option<(i32, i32, Option<crate::draw::Gamma>)>,
}

impl Default for GradientData {
    fn default() -> Self {
        GradientData {
            x1: 0.0,
            y1: 0.0,
            x2: 1.0,
            y2: 0.0,
            stops: Vec::new(),
            gamma_group: None,
            made: None,
            made_for: None,
        }
    }
}

/// The playlist editor's list, which Winamp fills with its own playlist.
#[derive(Debug, Clone)]
pub struct PlaylistData {
    /// The selected row, or -1.
    pub selected: i32,
    /// How far the rows are scrolled.
    pub scroll_px: i32,
    /// Where the finger went down.
    pub down_y: i32,
    /// The scroll then.
    pub down_scroll: i32,
    /// Whether the finger dragged.
    pub dragged: bool,
}

impl Default for PlaylistData {
    fn default() -> Self {
        PlaylistData {
            selected: -1,
            scroll_px: 0,
            down_y: 0,
            down_scroll: 0,
            dragged: false,
        }
    }
}

/// `<LayoutStatus>`: the status bar of a window with a status frame.
#[derive(Debug, Clone)]
pub struct LayoutStatusData {
    /// `includeonly`: which components' commands show.
    pub include_only: Option<Vec<String>>,
    /// `exclude`.
    pub exclude: Vec<String>,
    /// `bg="0"`: no field.
    pub background: bool,
}

impl Default for LayoutStatusData {
    fn default() -> Self {
        LayoutStatusData {
            include_only: None,
            exclude: Vec::new(),
            background: true,
        }
    }
}

/// What each kind of object keeps besides its [`GuiBase`] (and its [`GroupData`] for a group).
#[derive(Debug, Clone)]
pub enum Kind {
    /// An object the runtime does not draw, with the class scripts see.
    Unknown {
        /// Its class as scripts name it.
        class_name: String,
    },
    /// A plain group.
    Group,
    /// One look of a container.
    Layout(Box<LayoutData>),
    /// A picture.
    Layer(LayerData),
    /// A strip of frames.
    AnimatedLayer(LayerData, AnimData),
    /// A button.
    Button(ButtonData),
    /// A line of text.
    Text(Box<TextData>),
    /// A slider.
    Slider(SliderData),
    /// The play status light.
    Status(StatusData),
    /// A classic skin's window, drawn as Winamp draws it.
    ClassicWindow(crate::classic::ClassicWindow),
    /// A stretchable background.
    Grid(GridData),
    /// A progress grid.
    ProgressGrid(GridData, ProgressData),
    /// The album art.
    AlbumArt {
        /// `notfoundimage`.
        not_found: Option<String>,
    },
    /// A rectangle.
    RectObj(RectData),
    /// A strip picked by the volume, balance or seek position.
    Images(ImagesData),
    /// A menu bar entry.
    Menu(MenuData),
    /// Two panes with a divider.
    Frame(FrameData),
    /// A strip of groups of one window type.
    Bucket(BucketData),
    /// A group that fades between pages.
    XFade(XFadeData),
    /// The analyzer and oscilloscope.
    Vis(Box<VisData>),
    /// The EQ curve.
    EqVis(EqVisData),
    /// The color themes list.
    ColorThemeList(ThemeListData),
    /// A gradient picture.
    Gradient(GradientData),
    /// The playlist editor's list.
    PlaylistView(PlaylistData),
    /// A window's status bar.
    LayoutStatus(LayoutStatusData),
}

/// One GUI object: its base, its group data when it holds children, and its kind.
#[derive(Debug, Clone)]
pub struct Obj {
    /// What every object keeps.
    pub base: GuiBase,
    /// The children and the rest, for a group.
    pub group: Option<GroupData>,
    /// What its kind keeps.
    pub kind: Kind,
}

impl Obj {
    /// An object of `kind` made from `tag`, with group data when `grouped`.
    pub fn new(tag: &str, kind: Kind, grouped: bool) -> Self {
        Obj {
            base: GuiBase::new(tag),
            group: if grouped {
                Some(GroupData::default())
            } else {
                None
            },
            kind,
        }
    }

    /// The class name scripts see.
    pub fn class_name(&self) -> &str {
        match &self.kind {
            Kind::Unknown { class_name } => class_name,
            Kind::Group => "Group",
            Kind::Layout(_) => "Layout",
            Kind::Layer(_) => "Layer",
            Kind::AnimatedLayer(..) => "AnimatedLayer",
            Kind::Button(b) => match b.kind {
                ButtonKind::Toggle { .. } => "ToggleButton",
                _ => "Button",
            },
            Kind::Text(_) => "Text",
            Kind::Slider(_) => "Slider",
            Kind::Status(_) => "Status",
            Kind::Grid(_)
            | Kind::ProgressGrid(..)
            | Kind::AlbumArt { .. }
            | Kind::RectObj(_)
            | Kind::Images(_)
            | Kind::ColorThemeList(_)
            | Kind::Gradient(_)
            | Kind::PlaylistView(_) => "GuiObject",
            Kind::Menu(_) => "Menu",
            Kind::Frame(_) | Kind::XFade(_) => "Group",
            Kind::Bucket(_) => "ComponentBucket",
            Kind::Vis(_) => "Vis",
            Kind::ClassicWindow(_) => "ClassicWindow",
            Kind::EqVis(_) => "EqVis",
            Kind::LayoutStatus(_) => "LayoutStatus",
        }
    }

    /// Whether it is a layout.
    pub fn is_layout(&self) -> bool {
        matches!(self.kind, Kind::Layout(_))
    }

    /// The children, or none for an object that holds none.
    pub fn children(&self) -> &[ObjId] {
        self.group
            .as_ref()
            .map(|g| g.children.as_slice())
            .unwrap_or(&[])
    }

    /// The layout data, for a layout.
    pub fn layout(&self) -> Option<&LayoutData> {
        match &self.kind {
            Kind::Layout(l) => Some(l),
            _ => None,
        }
    }

    /// The layout data, mutable, for a layout.
    pub fn layout_mut(&mut self) -> Option<&mut LayoutData> {
        match &mut self.kind {
            Kind::Layout(l) => Some(l),
            _ => None,
        }
    }
}

/// A window: the main player, the playlist editor, the equalizer. It shows one of its layouts.
#[derive(Debug, Clone)]
pub struct Container {
    /// Its id, lowercase.
    pub id: String,
    /// Its attributes as written.
    pub attrs: Vec<(String, String)>,
    /// Its layouts.
    pub layouts: Vec<ObjId>,
    /// The layout shown.
    pub current: Option<ObjId>,
    /// Whether it shows.
    pub visible: bool,
    /// `name`, or the id.
    pub name: String,
    /// `default_x`: where each layout sits in the viewport at the start.
    pub default_x: i32,
    /// `default_y`.
    pub default_y: i32,
    /// Whether the skin gave the container a spot (`default_x` or `default_y`).
    pub has_default_spot: bool,
    /// Screen pixels per layout pixel of its windows: 2 for Winamp's double size.
    pub scale: f32,
}

impl Container {
    /// A container `id` with `attrs`.
    pub fn new(id: &str, attrs: Vec<(String, String)>) -> Self {
        let attr = |k: &str| {
            attrs
                .iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.clone())
        };
        Container {
            id: id.to_string(),
            visible: attr("default_visible").map(|v| to_bool(&v)).unwrap_or(true),
            name: attr("name").unwrap_or_else(|| id.to_string()),
            default_x: attr("default_x")
                .and_then(|v| to_int_lenient(&v))
                .unwrap_or(0),
            default_y: attr("default_y")
                .and_then(|v| to_int_lenient(&v))
                .unwrap_or(0),
            has_default_spot: attr("default_x").is_some() || attr("default_y").is_some(),
            scale: 1.0,
            attrs,
            layouts: Vec::new(),
            current: None,
        }
    }

    /// The attribute `key`.
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// `canclose="0"`: a close leaves it open.
    pub fn can_close(&self) -> bool {
        self.attr("canclose").map(to_bool).unwrap_or(true)
    }
}

/// A shared, immutable string, as ids and file names are kept.
pub type Shared = Rc<str>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_parse_as_winamp_writes_them() {
        assert_eq!(parse_rgb("255,0,128"), Some(Rgb(0xff0080)));
        assert_eq!(parse_rgb("#00ff00"), Some(Rgb(0x00ff00)));
        assert_eq!(parse_rgb("White"), Some(Rgb(0xffffff)));
        assert_eq!(parse_rgb("wasabi.text"), None);
    }

    #[test]
    fn lenient_ints() {
        assert_eq!(to_int_lenient(" 12 "), Some(12));
        assert_eq!(to_int_lenient("12.7"), Some(12));
        assert_eq!(to_int_lenient("x"), None);
    }
}
