// SPDX-License-Identifier: GPL-3.0-or-later

//! A loaded modern skin: its resources, its containers, its scripts, and the glue between the
//! player and the objects. Everything runs on one thread; the app calls `tick` regularly and
//! `draw_windows` when something changed.

mod animated;
mod build;
mod buttons;
mod calls;
mod draw;
mod draw_kinds;
mod events;
mod frames;
mod free;
pub(crate) mod layout;
mod lists;
mod maki;
mod objects;
mod options;
mod pointer;
mod resources;
pub mod scripted;
mod shapes;
mod sizes;
mod sliders;
mod system;
#[cfg(test)]
mod tests;
mod tick;
mod touches;
mod tree;
mod vis;
mod vm;
mod windows;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::rc::Rc;

use crate::draw::{BitmapDef, BitmapFontDef, DrawCmd, FontDef, Gamma, Rect, Region, Rgb};
use crate::files::SkinFiles;
use crate::gui::{Container, ContainerId, Kind, Obj, ObjId};
use crate::host::{
    Clock, EngineStats, ObjectInfo, Presentation, SavedSettings, SkinHost, SkinSetup, WindowId,
    WindowInfo,
};
use crate::maki::{Handle, Script, SystemObject, Value};
use crate::player::{Player, Status};
use crate::xml::XmlNode;

pub(crate) const MAX_LAYOUT_PASSES: i32 = 4;
pub(crate) const MAX_EVENT_DEPTH: i32 = 48;
pub(crate) const MAX_INCLUDE_DEPTH: usize = 32;
pub(crate) const MAX_XML_DEPTH: usize = 256;
pub(crate) const MAX_NODES: i64 = 200_000;
pub(crate) const MAX_SETTINGS: usize = 4_096;
pub(crate) const MAX_SETTING_LENGTH: usize = 65_536;
pub(crate) const MAX_OBJECTS: i64 = 100_000;
pub(crate) const MAX_ALIAS_DEPTH: i32 = 8;
pub(crate) const EQ_FLAT: i32 = 128;
pub(crate) const PLAYLIST_GUID: &str = "45f3f7c1-a6f3-4ee6-a15e-125e92fc3f8d";
pub(crate) const LIBRARY_GUID: &str = "6b0edf80-c9a5-11d3-9f26-00c04f39ffc6";
pub(crate) const AVS_GUID: &str = "0000000a-000c-0010-ff7b-01014263450c";
pub(crate) const VIDEO_GUID: &str = "f0816d7b-fffc-4343-80f2-e8199aa15cc3";
/// The windows a holder shows the visualizer in: Winamp's visualizer, and its video.
pub(crate) const VIS_HOLDS: [&str; 2] = [AVS_GUID, VIDEO_GUID];
/// Winamp's windows that show a list: the playlist editor and the media library.
pub(crate) const LIST_GUIDS: [&str; 2] = [PLAYLIST_GUID, LIBRARY_GUID];
/// The tags that embed one of Winamp's built-in windows in a skin window.
pub(crate) const HOLDER_TAGS: [&str; 2] = ["component", "windowholder"];
/// The windows the app shows when a host answers host actions.
pub(crate) const HOST_WINDOWS: [&str; 3] = ["ml", "avs", "video"];
/// The most pixels `opaque_right` reads of one picture; a larger one counts as showing to its right edge.
const MAX_OPAQUE_SCAN: i64 = 512 * 512;
/// A line's ascent and descent as shares of the em, for a host that measures no line.
const DEFAULT_ASCENT: f32 = 0.8;
const DEFAULT_DESCENT: f32 = 0.2;

/// Winamp's GUID aliases, and the short names the host knows windows by.
pub(crate) const TOGGLE_GUIDS: [(&str, &str); 7] = [
    ("pl", PLAYLIST_GUID),
    ("playlist", PLAYLIST_GUID),
    ("ml", LIBRARY_GUID),
    ("musiclibrary", LIBRARY_GUID),
    ("library", LIBRARY_GUID),
    ("avs", AVS_GUID),
    ("video", VIDEO_GUID),
];

pub(crate) fn toggle_guid(alias: &str) -> Option<&'static str> {
    TOGGLE_GUIDS
        .iter()
        .find(|(a, _)| *a == alias)
        .map(|(_, g)| *g)
}

/// How much of a bitmap part lies on its sheet; see `fit_to_sheet`.
#[derive(Debug, Clone)]
pub(crate) enum Fit {
    /// The file holds all of it at the size it names.
    Whole,
    /// The part the file holds, and the size asked for.
    Cut {
        part: BitmapDef,
        asked_w: i32,
        asked_h: i32,
    },
    /// Nothing of it lies on the file.
    None,
}

/// A loaded modern skin.
pub struct SkinRuntime {
    pub(crate) files: Box<dyn SkinFiles>,
    /// How far a classic title has scrolled, in pixels.
    /// A classic skin's `region.txt` polygons by section.
    pub(crate) classic_regions: HashMap<String, Vec<Vec<(i32, i32)>>>,
    /// A classic skin's playlist look (`pledit.txt`).
    pub(crate) classic_pledit: crate::classic::pledit::PleditStyle,
    /// `gen.bmp`'s font, once read.
    pub(crate) classic_gen_font: Option<Rc<crate::classic::gen::GenFont>>,
    /// The skin's cursor files, decoded as they are first asked for.
    pub(crate) classic_cursors:
        HashMap<&'static str, Option<Rc<crate::classic::cursor::SkinCursor>>>,
    /// A classic skin's vis palette (`viscolor.txt`).
    pub(crate) classic_palette: crate::classic::viscolor::Palette,
    /// The classic analyzer's state.
    pub(crate) classic_vis: crate::classic::vis::ClassicVis,
    /// Whether a classic skin's equalizer shows AUTO lit (presets loaded per song, the host's
    /// business).
    pub(crate) classic_eq_auto: bool,
    /// The first row a classic skin's playlist shows.
    pub(crate) classic_pl_scroll: i32,
    /// What a classic skin's vis area shows.
    pub(crate) classic_vis_mode: crate::classic::vis::VisMode,
    /// The vis mode before the last quick run of taps on the vis, and when that run began.
    pub(crate) classic_vis_before: crate::classic::vis::VisMode,
    pub(crate) classic_vis_tapped_at: i64,
    /// What the title row shows in place of the song while a slider is dragged.
    pub(crate) classic_status: Option<String>,
    /// The title's scroll in pixels when it last stopped or started, the clock time from which
    /// it moves on (later than now while held), and the title it belongs to.
    pub(crate) classic_title_base: i32,
    pub(crate) classic_title_epoch: i64,
    pub(crate) classic_title_key: String,
    /// How a classic skin's vis area draws: Winamp's visualization options.
    pub(crate) classic_vis_style: crate::classic::vis::VisStyle,
    /// A press on a classic window that has not ended.
    pub(crate) classic_press: Option<crate::classic::touch::ClassicPress>,
    /// A classic playlist's bottom-button menu a tap left open, under touch: the window and the
    /// button.
    pub(crate) classic_open_menu: Option<(ObjId, crate::classic::touch::PlMenu)>,
    /// Whether the classic time display counts down.
    pub(crate) classic_remaining: bool,
    /// Whether the classic playlist's rows are in the skin's letters (`Presentation::skin_list_font`).
    pub(crate) skin_list_font: bool,
    /// Where the thing sending the next host action is in the stack, when a classic window's
    /// control or a long press set it; else the action's source object's own rectangle.
    pub(crate) action_from: Option<Rect>,
    /// Pixels the classic playlist adds to a row (`Presentation::list_row_extra`).
    pub(crate) list_row_extra: i32,
    /// The ink of the skin's `text.bmp` for those rows, `None` when it has none, and whether it
    /// was read: once a skin.
    pub(crate) classic_list_ink: Option<std::rc::Rc<crate::classic::listfont::ListInk>>,
    pub(crate) classic_list_ink_read: bool,
    /// The classic playlist's selected row.
    pub(crate) classic_selected_row: Option<i32>,
    pub(crate) host: Box<dyn SkinHost>,
    pub(crate) player: Box<dyn Player>,
    clock: Clock,
    // ---- resources
    pub(crate) bitmaps: HashMap<String, BitmapDef>,
    bitmap_fonts: HashMap<String, BitmapFontDef>,
    fonts: HashMap<String, FontDef>,
    colors: HashMap<String, Rgb>,
    color_aliases: HashMap<String, String>,
    color_groups: HashMap<String, String>,
    aliases: HashMap<String, String>,
    pub(crate) group_defs: HashMap<String, Rc<XmlNode>>,
    /// Groupdef ids by their `windowtype`, in the order the skin defined them.
    pub(crate) window_types: HashMap<String, Vec<String>>,
    /// A groupdef that redefines an id, to the one it replaced (by the node's address).
    earlier_defs: HashMap<usize, Rc<XmlNode>>,
    xui_tags: HashMap<String, String>,
    gamma_sets: Vec<(String, HashMap<String, Gamma>)>,
    current_gamma_set: Option<String>,
    gammas: HashMap<String, Option<Gamma>>,
    base_bitmaps: HashSet<String>,
    base_group_defs: HashSet<usize>,
    base_colors: HashSet<String>,
    collecting_base: bool,
    /// What of the default element set this skin used so far.
    pub(crate) base_usage: BTreeSet<String>,
    resolved_fonts: HashMap<String, Option<Rc<BitmapFontDef>>>,
    fits: HashMap<BitmapDef, Fit>,
    opaque_columns: HashMap<String, i32>,
    gdi_lines: HashMap<String, Option<crate::gdi::GdiLineMetrics>>,
    /// A window's shape as `window_start` last gave it: the change it was worked out at, and the command.
    shaped: HashMap<ObjId, (i64, DrawCmd)>,
    // ---- objects
    pub(crate) objects: Vec<Obj>,
    pub(crate) containers: Vec<Container>,
    /// Windows a skin button opened, in the order they opened.
    opened: Vec<ContainerId>,
    windows_now: Vec<ObjId>,
    windows_at: i64,
    window_ids_now: Vec<WindowId>,
    window_ids_at: i64,
    /// Layouts by window id (index = id - 1).
    layouts_by_id: Vec<ObjId>,
    placed: HashMap<ObjId, Rect>,
    /// The windows set out apart, when the host asks for that.
    pub(crate) free: free::FreeState,
    focused: Option<ObjId>,
    shown_before: HashSet<ContainerId>,
    pub(crate) scripts: Vec<Script>,
    pub(crate) systems: Vec<SystemObject>,
    programs: HashMap<String, Option<Rc<crate::maki::Program>>>,
    pub(crate) made: Vec<maki::Made>,
    config_items: Vec<(String, usize)>,
    created: i64,
    random_state: u64,
    /// How far the local time is from UTC at a moment of the clock, in ms.
    zone_offset: Box<dyn Fn(i64) -> i32>,
    /// The windows the app opened for a script's `showWindow` and has not closed since.
    shown_for_skin: Vec<String>,
    /// An error a method call raised for the script running it.
    pub(crate) script_error: Option<String>,
    pending_starts: Vec<usize>,
    pub(crate) timers: Vec<usize>,
    /// Component buckets that have yet to hear onStartup, which they do once a script listens to them.
    starting_buckets: Vec<ObjId>,
    pub(crate) animations: Vec<ObjId>,
    /// Objects on their way to a target set with `setTarget*` and `gotoTarget`.
    pub(crate) moving: Vec<ObjId>,
    /// The events fired while a test records them; see `scripted`.
    pub(crate) recorded: Option<Vec<scripted::RecordedEvent>>,
    pub(crate) private_ints: HashMap<String, i32>,
    pub(crate) private_strings: HashMap<String, String>,
    config_flags: HashMap<String, bool>,
    config_values: HashMap<String, i32>,
    /// Skin settings by attribute name, as Winamp keeps them in studio.xnf.
    pub(crate) stored_settings: BTreeMap<String, String>,
    settings_version: i32,
    config_version: i32,
    /// XML attributes no object handled, as "tag attribute" with a count.
    pub(crate) unhandled_params: BTreeMap<String, i32>,
    // ---- state
    pub(crate) skin_name: String,
    pub(crate) have_library: bool,
    host_actions: bool,
    pub(crate) viewport_w: i32,
    pub(crate) viewport_h: i32,
    view_size: Option<(i32, i32)>,
    columns: i32,
    column_width: i32,
    stretch_last: bool,
    pub(crate) touch_slop: i32,
    /// Whether a finger drives the skin (`Presentation::touch`).
    pub(crate) touch: bool,
    pub(crate) last_pointer: (i32, i32),
    dirty: bool,
    /// The clock alone moved a classic window (its title, time, vis): the next draw redraws only
    /// the windows that read the clock and keeps the others' last drawing.
    clock_dirty: bool,
    /// When a classic window next changes by itself; 0 for at once.
    classic_next_at: i64,
    /// The windows the clock alone moved since the last draw.
    clock_windows: HashSet<ObjId>,
    /// The second a classic playlist's time showed when it last drew, and whether it counted down.
    clock_second: (i32, bool),
    /// Each window's last drawing, for the windows a clock-only draw keeps.
    window_drawings: HashMap<ObjId, (Vec<DrawCmd>, Vec<Option<ObjId>>)>,
    needs_layout: bool,
    scripts_started: bool,
    last_status: Status,
    last_title: String,
    last_volume: i32,
    last_switches: i32,
    last_tracks: Vec<crate::player::Track>,
    last_track_index: i32,
    last_eq_on: bool,
    last_preamp: i32,
    last_bands: Vec<i32>,
    last_pan: i32,
    last_format: (i32, i32, i32),
    last_position: i32,
    /// Every object of a window, in drawing order, from the last layout pass.
    pub(crate) all_objects: Vec<ObjId>,
    sliders: Vec<ObjId>,
    texts: Vec<ObjId>,
    vis_objects: Vec<ObjId>,
    progress_grids: Vec<ObjId>,
    buttons: Vec<ObjId>,
    captured: Option<ObjId>,
    pub(crate) wal_version: f64,
    pub(crate) change_count: i64,
    pub(crate) shapes_worked_out: i64,
    layout_passes: i64,
    timer_fires: i64,
    last_rects: HashMap<ObjId, Rect>,
    told_windows: Vec<WindowId>,
    /// The objects the last tick found moving by themselves.
    pub(crate) tick_changed: Vec<ObjId>,
    /// The object that drew each command of the window being drawn, in the order drawn.
    pub(crate) drawn_owners: Vec<Option<ObjId>>,
    /// Whether a window's drawing is under way, so its groups note who drew what.
    pub(crate) tracking_owners: bool,
    slide_lead: i64,
    expanded_nodes: i64,
    built: i64,
    event_depth: i32,
    firing: Vec<String>,
    edge_placements: i32,
    /// Total length of the playlist entries selected in the playlist editor, for `display="pe_info"`.
    pub(crate) playlist_selected_ms: i32,
    pointer: pointer::PointerState,
}

impl SkinRuntime {
    /// A runtime for the skin in `files`, with `host` and `player`, set up as `setup` says, on `clock`.
    pub fn new(
        files: Box<dyn SkinFiles>,
        host: Box<dyn SkinHost>,
        player: Box<dyn Player>,
        setup: SkinSetup,
        clock: Clock,
    ) -> Self {
        let last_status = player.status();
        let last_title = player.title();
        let last_volume = player.volume();
        let last_switches = switches_of(&*player);
        let last_tracks = player.tracks();
        let last_track_index = player.track_index();
        let last_eq_on = player.eq_enabled();
        let last_preamp = player.eq_preamp();
        let last_bands = player.eq_bands();
        let last_pan = player.pan();
        let last_format = (
            player.bitrate_kbps(),
            player.sample_rate_khz(),
            player.channels(),
        );
        let last_position = player.position_ms();
        SkinRuntime {
            files,
            classic_regions: HashMap::new(),
            classic_pledit: crate::classic::pledit::PleditStyle::default(),
            classic_gen_font: None,
            classic_cursors: HashMap::new(),
            classic_palette: crate::classic::viscolor::BASE,
            classic_vis: crate::classic::vis::ClassicVis::default(),
            classic_vis_mode: crate::classic::vis::VisMode::default(),
            classic_vis_before: crate::classic::vis::VisMode::default(),
            classic_vis_tapped_at: i64::MIN / 2,
            classic_status: None,
            classic_title_base: 0,
            classic_title_epoch: 0,
            classic_title_key: String::new(),
            classic_vis_style: crate::classic::vis::VisStyle::default(),
            classic_pl_scroll: 0,
            classic_eq_auto: false,
            classic_press: None,
            classic_open_menu: None,
            classic_remaining: false,
            classic_selected_row: None,
            host,
            player,
            clock,
            bitmaps: HashMap::new(),
            bitmap_fonts: HashMap::new(),
            fonts: HashMap::new(),
            colors: HashMap::new(),
            color_aliases: HashMap::new(),
            color_groups: HashMap::new(),
            aliases: HashMap::new(),
            group_defs: HashMap::new(),
            window_types: HashMap::new(),
            earlier_defs: HashMap::new(),
            xui_tags: HashMap::new(),
            gamma_sets: Vec::new(),
            current_gamma_set: None,
            gammas: HashMap::new(),
            base_bitmaps: HashSet::new(),
            base_group_defs: HashSet::new(),
            base_colors: HashSet::new(),
            collecting_base: false,
            base_usage: BTreeSet::new(),
            resolved_fonts: HashMap::new(),
            fits: HashMap::new(),
            opaque_columns: HashMap::new(),
            gdi_lines: HashMap::new(),
            shaped: HashMap::new(),
            objects: Vec::new(),
            containers: Vec::new(),
            opened: Vec::new(),
            windows_now: Vec::new(),
            windows_at: -1,
            window_ids_now: Vec::new(),
            window_ids_at: -1,
            layouts_by_id: Vec::new(),
            placed: HashMap::new(),
            free: free::FreeState::default(),
            focused: None,
            shown_before: HashSet::new(),
            scripts: Vec::new(),
            systems: Vec::new(),
            programs: HashMap::new(),
            made: Vec::new(),
            config_items: Vec::new(),
            created: 0,
            random_state: 0x9E37_79B9_7F4A_7C15,
            zone_offset: Box::new(|_| 0),
            shown_for_skin: Vec::new(),
            script_error: None,
            pending_starts: Vec::new(),
            timers: Vec::new(),
            starting_buckets: Vec::new(),
            animations: Vec::new(),
            moving: Vec::new(),
            recorded: None,
            private_ints: HashMap::new(),
            private_strings: HashMap::new(),
            config_flags: HashMap::new(),
            config_values: HashMap::new(),
            stored_settings: BTreeMap::new(),
            settings_version: 0,
            config_version: 0,
            unhandled_params: BTreeMap::new(),
            skin_name: setup.skin_name,
            have_library: setup.have_library,
            host_actions: setup.host_actions,
            viewport_w: 1080,
            viewport_h: 1920,
            view_size: None,
            columns: 1,
            column_width: 0,
            stretch_last: true,
            touch_slop: 6,
            touch: false,
            skin_list_font: false,
            list_row_extra: 0,
            action_from: None,
            classic_list_ink: None,
            classic_list_ink_read: false,
            last_pointer: (0, 0),
            dirty: true,
            clock_dirty: false,
            classic_next_at: 0,
            clock_windows: HashSet::new(),
            clock_second: (-1, false),
            window_drawings: HashMap::new(),
            needs_layout: true,
            scripts_started: false,
            last_status,
            last_title,
            last_volume,
            last_switches,
            last_tracks,
            last_track_index,
            last_eq_on,
            last_preamp,
            last_bands,
            last_pan,
            last_format,
            last_position,
            all_objects: Vec::new(),
            sliders: Vec::new(),
            texts: Vec::new(),
            vis_objects: Vec::new(),
            progress_grids: Vec::new(),
            buttons: Vec::new(),
            captured: None,
            wal_version: 1.0,
            change_count: 0,
            shapes_worked_out: 0,
            layout_passes: 0,
            timer_fires: 0,
            last_rects: HashMap::new(),
            told_windows: Vec::new(),
            tick_changed: Vec::new(),
            drawn_owners: Vec::new(),
            tracking_owners: false,
            slide_lead: 0,
            expanded_nodes: 0,
            built: 0,
            event_depth: 0,
            firing: Vec::new(),
            edge_placements: 0,
            playlist_selected_ms: 0,
            pointer: pointer::PointerState::default(),
        }
    }

    /// The host, for what it keeps of a run.
    pub fn host(&mut self) -> &mut dyn SkinHost {
        &mut *self.host
    }

    /// The player, for a host that drives it.
    pub fn player(&mut self) -> &mut dyn Player {
        &mut *self.player
    }

    /// Sets how a classic skin's vis area draws (Winamp's visualization options).
    pub fn set_classic_vis_style(&mut self, style: crate::classic::vis::VisStyle) {
        self.classic_vis_style = style;
        self.repaint();
    }

    /// The clock's reading now, in ms.
    pub fn now(&self) -> i64 {
        (self.clock)()
    }

    /// The screen a skin may place windows on before the host sets a view size, in layout pixels.
    pub fn set_viewport(&mut self, w: i32, h: i32) {
        self.viewport_w = w;
        self.viewport_h = h;
    }

    /// How far the local time is from UTC at a moment of the clock, in ms: the time of day
    /// scripts read is local. 0 unless the host says.
    pub fn set_zone_offset(&mut self, f: Box<dyn Fn(i64) -> i32>) {
        self.zone_offset = f;
    }

    /// The screen as scripts ask for it (`getViewportWidth`): the phone's view in layout pixels once known.
    pub fn viewport(&self) -> (i32, i32) {
        self.view_size.unwrap_or((self.viewport_w, self.viewport_h))
    }

    /// What the engine has done so far; see [`EngineStats`].
    pub fn stats(&self) -> EngineStats {
        EngineStats {
            script_steps: self.scripts.iter().map(|s| s.steps_since_refill()).sum(),
            timer_fires: self.timer_fires,
            layout_passes: self.layout_passes,
            shapes_worked_out: self.shapes_worked_out,
        }
    }

    /// The skin's color themes (gammasets), in the order the skin defines them.
    pub fn color_themes(&self) -> Vec<String> {
        self.gamma_sets.iter().map(|(n, _)| n.clone()).collect()
    }

    /// The color theme in use.
    pub fn color_theme(&self) -> Option<&str> {
        self.current_gamma_set.as_deref()
    }

    /// Wears the color theme named `name`, matched ignoring case; an unknown name changes nothing.
    pub fn select_color_theme(&mut self, name: &str) {
        let Some(m) = self
            .gamma_sets
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(n, _)| n.clone())
        else {
            return;
        };
        if self.current_gamma_set.as_deref() != Some(m.as_str()) {
            self.settings_changed();
        }
        self.current_gamma_set = Some(m);
        self.gammas.clear();
        self.invalidate();
    }

    /// Counts changes to what a host keeps of the skin between runs.
    pub fn settings_version(&self) -> i32 {
        self.settings_version
    }

    /// What the host keeps of the skin between runs, as it is now; the color theme is
    /// [`SkinRuntime::color_theme`].
    pub fn saved_settings(&self) -> SavedSettings {
        SavedSettings {
            attributes: self.stored_settings.clone(),
            strings: self
                .private_strings
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            ints: self
                .private_ints
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect(),
        }
    }

    /// Puts back what [`SkinRuntime::saved_settings`] gave in an earlier run, before `load`, so
    /// the scripts read it as they start. It counts as no change ([`SkinRuntime::settings_version`]).
    pub fn restore_settings(&mut self, saved: &SavedSettings) {
        self.stored_settings
            .extend(saved.attributes.iter().map(|(k, v)| (k.clone(), v.clone())));
        self.private_strings
            .extend(saved.strings.iter().map(|(k, v)| (k.clone(), v.clone())));
        self.private_ints
            .extend(saved.ints.iter().map(|(k, v)| (k.clone(), *v)));
    }

    pub(crate) fn settings_changed(&mut self) {
        self.settings_version += 1;
    }

    pub(crate) fn config_changed(&mut self) {
        self.config_version += 1;
    }

    /// Keeps a script's private setting, within bounds.
    pub(crate) fn store_private_int(&mut self, key: &str, value: i32) {
        let k = key.to_lowercase();
        if k.len() > MAX_SETTING_LENGTH {
            self.host.log(&format!("setting {k} too long, not kept"));
            return;
        }
        if !self.private_ints.contains_key(&k) && self.private_ints.len() >= MAX_SETTINGS {
            self.host.log(&format!("too many settings, {k} not kept"));
            return;
        }
        if self.private_ints.insert(k, value) != Some(value) {
            self.settings_changed();
        }
    }

    /// Keeps a script's private string, within bounds.
    pub(crate) fn store_private_string(&mut self, key: &str, value: &str) {
        let k = key.to_lowercase();
        if k.len() > MAX_SETTING_LENGTH || value.len() > MAX_SETTING_LENGTH {
            self.host.log(&format!("setting {k} too long, not kept"));
            return;
        }
        if !self.private_strings.contains_key(&k) && self.private_strings.len() >= MAX_SETTINGS {
            self.host.log(&format!("too many settings, {k} not kept"));
            return;
        }
        if self.private_strings.insert(k, value.to_string()).as_deref() != Some(value) {
            self.settings_changed();
        }
    }
}

/// One window's drawing with the object that drew each command ([`SkinRuntime::draw_windows_owned`]):
/// `owners[i]` is the object (an id into the skin's objects, the same from frame to frame while
/// the skin is loaded) whose drawing `cmds[i]` is, or `None` for a command that is the window's
/// structure (the end of a shape or a clip).
#[derive(Debug, Clone, PartialEq)]
pub struct DrawnWindow {
    /// Where the window sits in the stack.
    pub rect: Rect,
    /// Its drawing.
    pub cmds: Vec<DrawCmd>,
    /// Who drew each command.
    pub owners: Vec<Option<ObjId>>,
}

/// The player's shuffle, repeat and crossfade switches as bits 1, 2 and 4.
pub(crate) fn switches_of(p: &dyn Player) -> i32 {
    (p.shuffle() as i32) | ((p.repeat() as i32) << 1) | ((p.crossfade() as i32) << 2)
}

/// An element id and its old-style twin: Winamp's own objects ask for `wasabi.list.text`, the
/// default set aliases `studio.list.text` to it, and an older skin (MMD3) defines the `studio.`
/// name itself, which Winamp still finds.
pub(crate) fn names_for(id: &str) -> Vec<String> {
    let k = id.to_lowercase();
    if let Some(rest) = k.strip_prefix("wasabi.") {
        vec![k.clone(), format!("studio.{rest}")]
    } else if let Some(rest) = k.strip_prefix("studio.") {
        vec![k.clone(), format!("wasabi.{rest}")]
    } else {
        vec![k]
    }
}

/// Whether `k` is one of the kinds that holds children.
pub(crate) fn is_grouped(k: &Kind) -> bool {
    matches!(
        k,
        Kind::Group | Kind::Layout(_) | Kind::Frame(_) | Kind::Bucket(_) | Kind::XFade(_)
    )
}
