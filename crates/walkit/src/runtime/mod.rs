// SPDX-License-Identifier: GPL-3.0-or-later

//! A loaded modern skin: its resources, its containers, its scripts, and the glue between the
//! player and the objects. Everything runs on one thread; the app calls `tick` regularly and
//! `draw_windows` when something changed.

mod build;
mod calls;
mod draw;
mod frames;
pub(crate) mod layout;
mod lists;
mod maki;
mod objects;
mod options;
mod pointer;
pub mod scripted;
mod system;
#[cfg(test)]
mod tests;
mod tick;
mod vis;
mod vm;

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
    pub(crate) last_pointer: (i32, i32),
    dirty: bool,
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
        SkinRuntime {
            files,
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
            last_pointer: (0, 0),
            dirty: true,
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

    // ---- resources -------------------------------------------------------------------------

    /// The bitmap element `id` names, through aliases; a plain file name used as an image.
    pub(crate) fn bitmap(&mut self, id: &str) -> Option<BitmapDef> {
        let mut from_base: Option<BitmapDef> = None;
        for name in names_for(id) {
            let Some((key, def)) = self.aliased_bitmap(&name) else {
                continue;
            };
            if !self.base_bitmaps.contains(&key) {
                return Some(def);
            }
            if from_base.is_none() {
                from_base = Some(def);
            }
        }
        if let Some(def) = from_base {
            self.base_usage.insert(format!("bitmap:{}", def.id));
            return Some(def);
        }
        // a plain file name used as an image
        if self.files.exists(id) {
            let key = id.to_lowercase();
            Some(
                self.bitmaps
                    .entry(key.clone())
                    .or_insert_with(|| BitmapDef::new(&key, id, 0, 0, None, None, None))
                    .clone(),
            )
        } else {
            None
        }
    }

    /// The bitmap element `name` defines, or names through element aliases (at most 8 hops),
    /// with the id it is defined under; `None` when the chain ends without one.
    fn aliased_bitmap(&self, name: &str) -> Option<(String, BitmapDef)> {
        let mut key = name.to_string();
        let mut hops = 0;
        while hops < 8 {
            hops += 1;
            if let Some(def) = self.bitmaps.get(&key) {
                return Some((key, def.clone()));
            }
            key = self.aliases.get(&key)?.clone();
        }
        None
    }

    /// The size of bitmap `id`: as its element says, else from its file.
    pub(crate) fn bitmap_size(&mut self, id: &str) -> Option<(i32, i32)> {
        let def = self.bitmap(id)?;
        if let (Some(w), Some(h)) = (def.w, def.h) {
            return Some((w, h));
        }
        let full = self.host.image_size(&def.file)?;
        Some((
            def.w.unwrap_or(full.0 - def.x),
            def.h.unwrap_or(full.1 - def.y),
        ))
    }

    /// Whether the bitmap drawn into `rect` is not fully transparent at a point. Unknown
    /// pixels count as opaque, so a host that cannot read pixels keeps rectangle hit testing.
    pub(crate) fn opaque_at(&mut self, image: Option<&str>, rect: Rect, px: i32, py: i32) -> bool {
        let Some(def) = image.and_then(|i| self.bitmap(i)) else {
            return true;
        };
        let Some((w, h)) = self.bitmap_size(&def.id) else {
            return true;
        };
        if rect.width <= 0 || rect.height <= 0 {
            return true;
        }
        let sx = def.x + (px - rect.left) * w / rect.width;
        let sy = def.y + (py - rect.top) * h / rect.height;
        let Some(p) = self.host.pixel(&def.file, sx, sy) else {
            return true;
        };
        (p >> 24) != 0
    }

    /// Where `image` drawn into `dst` stops showing on the right: the right edge of its last
    /// column with a pixel that is not fully transparent, in layout pixels, or `dst`'s left
    /// edge when none shows. The column is looked up once per bitmap part.
    pub(crate) fn opaque_right(&mut self, image: &BitmapDef, dst: Rect) -> i32 {
        if dst.width <= 0 {
            return dst.left;
        }
        let Some(full) = self.host.image_size(&image.file) else {
            return dst.right();
        };
        let sw = image.w.unwrap_or(full.0 - image.x);
        let sh = image.h.unwrap_or(full.1 - image.y);
        // only the part the sheet holds is drawn
        let cw = sw.min(full.0 - image.x);
        let ch = sh.min(full.1 - image.y);
        let no_source = cw <= 0 || ch <= 0;
        if image.x < 0 || image.y < 0 || no_source {
            return dst.left;
        }
        let key = format!(
            "{}|{}|{}|{}|{}",
            image.file.to_lowercase(),
            image.x,
            image.y,
            sw,
            sh
        );
        let column = match self.opaque_columns.get(&key) {
            Some(c) => *c,
            None => {
                let c = if cw as i64 * ch as i64 > MAX_OPAQUE_SCAN {
                    sw - 1
                } else {
                    match self.host.pixels(&image.file, image.x, image.y, cw, ch) {
                        None => sw - 1,
                        Some(px) => (0..cw)
                            .rev()
                            .find(|c| (0..ch).any(|r| (px[(r * cw + c) as usize] >> 24) != 0))
                            .unwrap_or(-1),
                    }
                };
                self.opaque_columns.insert(key, c);
                c
            }
        };
        if column < 0 {
            return dst.left;
        }
        // stretched to dst, source column c covers dst pixels up to (c + 1) * dst / source, rounded up
        dst.right()
            .min(dst.left + ((column + 1) * dst.width + sw - 1) / sw)
    }

    /// A bitmap font; its `file` may name a bitmap element, which then gives the file and the offset.
    pub(crate) fn bitmap_font(&mut self, id: &str) -> Option<Rc<BitmapFontDef>> {
        let key = id.to_lowercase();
        if let Some(f) = self.resolved_fonts.get(&key) {
            return f.clone();
        }
        let resolved = self.bitmap_fonts.get(&key).map(|def| {
            let element = self.bitmaps.get(&def.file.to_lowercase());
            // the color theme reaches a bitmap font only through the bitmap element it draws from;
            // Winamp 5.666 ignores gammagroup on <bitmapfont> itself
            match element {
                Some(e) => Rc::new(BitmapFontDef {
                    id: def.id.clone(),
                    file: e.file.clone(),
                    char_width: def.char_width,
                    char_height: def.char_height,
                    h_spacing: def.h_spacing,
                    v_spacing: def.v_spacing,
                    ox: e.x,
                    oy: e.y,
                    gamma_group: e.gamma_group.clone(),
                }),
                None => Rc::new(BitmapFontDef {
                    ox: 0,
                    oy: 0,
                    gamma_group: None,
                    ..def.clone()
                }),
            }
        });
        self.resolved_fonts.insert(key, resolved.clone());
        resolved
    }

    /// The TrueType font `id`.
    pub(crate) fn font(&self, id: &str) -> Option<FontDef> {
        self.fonts.get(&id.to_lowercase()).cloned()
    }

    /// The tint of gamma group `group` in the color theme worn now.
    pub(crate) fn gamma(&mut self, group: Option<&str>) -> Option<Gamma> {
        let group = group?;
        if let Some(g) = self.gammas.get(group) {
            return *g;
        }
        let g = self
            .current_gamma_set
            .as_ref()
            .and_then(|set| self.gamma_sets.iter().find(|(n, _)| n == set))
            .and_then(|(_, groups)| groups.get(&group.to_lowercase()).copied())
            .filter(|g| !g.is_identity());
        self.gammas.insert(group.to_string(), g);
        g
    }

    /// The color `id`, tinted by the theme.
    pub(crate) fn color(&mut self, id: &str) -> Option<Rgb> {
        self.color_at_depth(id, 0)
    }

    fn color_at_depth(&mut self, id: &str, depth: i32) -> Option<Rgb> {
        let key = self.color_key(id)?;
        if self.base_colors.contains(&key) {
            self.base_usage.insert(format!("color:{key}"));
        }
        // an alias takes the other color's RGB, tinted by its own group (or the other's when it has none)
        let c = match self.colors.get(&key) {
            Some(c) => *c,
            None => {
                let alias = self
                    .color_aliases
                    .get(&key)
                    .filter(|_| depth < MAX_ALIAS_DEPTH)?
                    .clone();
                self.plain_color(&alias, depth + 1)?
            }
        };
        // a color that names no gammagroup takes the theme's "Text" group (Wasabi SDK skinclr)
        let group = self
            .color_groups
            .get(&key)
            .cloned()
            .or_else(|| {
                self.color_aliases
                    .get(&key)
                    .and_then(|a| self.color_groups.get(a))
                    .cloned()
            })
            .unwrap_or_else(|| "text".to_string());
        let Some(g) = self.gamma(Some(&group)) else {
            return Some(c);
        };
        Some(Rgb(g.apply(c.0 | 0xff00_0000) & 0xffffff))
    }

    /// A color's RGB before any tint, following aliases.
    fn plain_color(&self, id: &str, depth: i32) -> Option<Rgb> {
        let key = self.color_key(id)?;
        match self.colors.get(&key) {
            Some(c) => Some(*c),
            None => {
                let alias = self
                    .color_aliases
                    .get(&key)
                    .filter(|_| depth < MAX_ALIAS_DEPTH)?;
                self.plain_color(alias, depth + 1)
            }
        }
    }

    /// The name a color is defined under, as an RGB or an alias: `id` or its old-style twin, a
    /// skin's definition before the default set's. `None` when neither is defined.
    fn color_key(&self, id: &str) -> Option<String> {
        let names: Vec<String> = names_for(id)
            .into_iter()
            .filter(|n| self.colors.contains_key(n) || self.color_aliases.contains_key(n))
            .collect();
        names
            .iter()
            .find(|n| !self.base_colors.contains(*n))
            .or(names.first())
            .cloned()
    }

    // ---- containers and windows ----------------------------------------------------------

    /// The container `id`, matched ignoring case.
    pub(crate) fn container(&self, id: &str) -> Option<ContainerId> {
        self.containers
            .iter()
            .position(|c| c.id.eq_ignore_ascii_case(id))
    }

    pub(crate) fn main_container(&self) -> Option<ContainerId> {
        self.container("main").or(if self.containers.is_empty() {
            None
        } else {
            Some(0)
        })
    }

    pub(crate) fn main_layout(&self) -> Option<ObjId> {
        self.main_container()
            .and_then(|c| self.containers[c].current)
    }

    /// The windows on screen, top to bottom: the main one first.
    pub(crate) fn windows(&mut self) -> Vec<ObjId> {
        if self.windows_at != self.change_count {
            let mut w: Vec<ObjId> = self.main_layout().into_iter().collect();
            w.extend(
                self.opened
                    .iter()
                    .filter_map(|c| self.containers[*c].current),
            );
            self.windows_now = w;
            self.windows_at = self.change_count;
        }
        self.windows_now.clone()
    }

    /// A new id for `layout`, which keeps it; see [`WindowId`].
    pub(crate) fn new_window_id(&mut self) -> WindowId {
        WindowId(self.layouts_by_id.len() as u32 + 1)
    }

    pub(crate) fn register_layout(&mut self, layout: ObjId) {
        self.layouts_by_id.push(layout);
    }

    fn layout_of(&self, id: WindowId) -> ObjId {
        self.layouts_by_id[(id.0 as usize).wrapping_sub(1)]
    }

    pub(crate) fn window_id_of(&self, layout: ObjId) -> WindowId {
        self.objects[layout]
            .layout()
            .map(|l| l.window_id)
            .unwrap_or(WindowId(0))
    }

    /// The windows on screen, top to bottom, as `windows` has them: the main one first.
    pub fn window_ids(&mut self) -> Vec<WindowId> {
        let now = self.windows();
        if self.window_ids_at != self.change_count {
            self.window_ids_now = now.iter().map(|l| self.window_id_of(*l)).collect();
            self.window_ids_at = self.change_count;
        }
        self.window_ids_now.clone()
    }

    /// The main window as it shows now (its normal layout or its shade), or `None` when the skin has none.
    pub fn main_window(&self) -> Option<WindowId> {
        self.main_layout().map(|l| self.window_id_of(l))
    }

    /// What `id` is like now; see [`WindowInfo`].
    pub fn window_info(&mut self, id: WindowId) -> WindowInfo {
        let l = self.layout_of(id);
        let size = self.layout_size(l);
        let both = self.declared_resizable_both_ways(l);
        let d = self.objects[l].layout().expect("a layout");
        WindowInfo {
            id,
            size,
            min_w: d.min_w,
            min_h: d.min_h,
            max_w: d.max_w,
            max_h: d.max_h,
            resizable_both_ways: both,
            edge_x: d.edge_x,
            edge_y: d.edge_y,
            container: d
                .container
                .map(|c| self.containers[c].id.clone())
                .unwrap_or_default(),
            layout: self.objects[l].base.id.clone(),
        }
    }

    /// The object with id `id` in the windows on screen, the top one first; `None` when none has it.
    pub fn object_info(&mut self, id: &str) -> Option<ObjectInfo> {
        let windows = self.windows();
        let o = windows
            .iter()
            .rev()
            .find_map(|w| self.find_object(*w, id))?;
        let layout = self.layout_object(o)?;
        let origin = self.window_rect_of(layout);
        let r = self.objects[o].base.rect;
        Some(ObjectInfo {
            window: self.window_id_of(layout),
            rect: r,
            stack_rect: Rect::new(origin.left + r.left, origin.top + r.top, r.width, r.height),
            visible: self.objects[o].base.visible,
            class_name: self.objects[o].class_name().to_string(),
        })
    }

    /// The window container `container` shows now, or `None` when the skin has no such container.
    pub fn window_of(&self, container: &str) -> Option<WindowId> {
        self.container(container)
            .and_then(|c| self.containers[c].current)
            .map(|l| self.window_id_of(l))
    }

    /// Where `id` sits in the stack, in layout pixels.
    pub fn window_rect(&mut self, id: WindowId) -> Rect {
        self.window_rect_of(self.layout_of(id))
    }

    /// Where `layout` sits in the stack, in layout pixels.
    pub(crate) fn window_rect_of(&mut self, layout: ObjId) -> Rect {
        match self.placed.get(&layout) {
            Some(r) => *r,
            None => {
                let (w, h) = self.layout_size(layout);
                Rect::new(0, 0, w, h)
            }
        }
    }

    /// Opens the window of `container` in the stack; false when the skin has no such container.
    pub fn open_window(&mut self, container: &str) -> bool {
        match self.container(container) {
            Some(c) => {
                self.open_container(c);
                true
            }
            None => false,
        }
    }

    /// Closes the window of `container`; false when the skin has no such container.
    pub fn close_window(&mut self, container: &str) -> bool {
        match self.container(container) {
            Some(c) => {
                self.close_container(c);
                true
            }
            None => false,
        }
    }

    /// `id`'s size as it shows now, in layout pixels.
    pub fn window_size(&mut self, id: WindowId) -> (i32, i32) {
        self.layout_size(self.layout_of(id))
    }

    /// Sizes `id` to `width` by `height` where it is, as the host sizes a window to its box.
    pub fn resize_window(&mut self, id: WindowId, width: i32, height: i32) {
        let l = self.layout_of(id);
        self.resize_layout_to(l, width, height);
    }

    /// Whether a window besides the main one is open, which Back would put away.
    pub fn window_open(&self) -> bool {
        !self.opened.is_empty()
    }

    /// The space the app has for the stack, in layout pixels; `None` leaves each window at the size it has.
    pub fn set_view_size(&mut self, value: Option<(i32, i32)>) {
        if value != self.view_size {
            self.view_size = value;
            for c in 0..self.containers.len() {
                self.fit_viewport(c);
            }
            self.relayout();
        }
    }

    /// The space the app has for the stack.
    pub fn view_size(&self) -> Option<(i32, i32)> {
        self.view_size
    }

    /// What getSkinName() answers: the skin's folder name, which scripts key what they remember
    /// by; the name in the skin's `<skininfo>` when the host gave none before `load`.
    pub fn skin_name(&self) -> &str {
        &self.skin_name
    }

    /// Names the skin for its scripts, before or after `load`; a name given stays through `load`.
    pub fn set_skin_name(&mut self, name: &str) {
        self.skin_name = name.to_string();
    }

    /// How the host shows the windows, as one value.
    pub fn presentation(&self) -> Presentation {
        Presentation {
            view_size: self.view_size,
            viewport: (self.viewport_w, self.viewport_h),
            columns: self.columns,
            column_width: self.column_width,
            stretch_last: self.stretch_last,
            touch_slop: self.touch_slop,
        }
    }

    /// Sets how the host shows the windows; what differs changes, as setting the viewport, the
    /// columns, the column width, the stretch, the touch slop and the view size one by one does.
    pub fn set_presentation(&mut self, p: &Presentation) {
        self.set_viewport(p.viewport.0, p.viewport.1);
        self.set_columns(p.columns);
        self.set_column_width(p.column_width);
        self.set_stretch_last(p.stretch_last);
        self.touch_slop = p.touch_slop;
        self.set_view_size(p.view_size);
    }

    /// How many columns the stack is set out in once the host gives it a view size: at 2 the
    /// windows that show a list go in a column right of the main window, else every window
    /// docks under the main one. A change lays the windows out again.
    pub fn set_columns(&mut self, columns: i32) {
        if columns != self.columns {
            self.columns = columns;
            self.relayout();
        }
    }

    /// How wide the windows in the right column of two columns are, in layout pixels; 0 for as
    /// wide as what shows of the main window. A change lays the windows out again.
    pub fn set_column_width(&mut self, width: i32) {
        if width != self.column_width {
            self.column_width = width;
            self.relayout();
        }
    }

    /// Whether the last docked window takes the height the view leaves under the others, when
    /// it can be made taller and shows a list. Off, every window keeps the height the skin gives it.
    pub fn set_stretch_last(&mut self, stretch: bool) {
        if stretch != self.stretch_last {
            self.stretch_last = stretch;
            self.relayout();
        }
    }

    /// How far beside an object a finger may land and still mean it, in layout pixels.
    pub fn set_touch_slop(&mut self, slop: i32) {
        self.touch_slop = slop;
    }

    /// Where `id` visibly ends on the right; with `lay_out_first` after laying it out at its
    /// size, as the stack is measured.
    pub fn content_right_of(&mut self, id: WindowId, lay_out_first: bool) -> i32 {
        let l = self.layout_of(id);
        if lay_out_first {
            self.layout_root(l);
        }
        self.content_right(l)
    }

    /// Whether a window docked under the main one shows a list.
    pub fn list_window_open(&mut self) -> bool {
        let others: Vec<ObjId> = self.windows().into_iter().skip(1).collect();
        others.iter().any(|l| self.shows_list(*l))
    }

    /// How far right the stack shows in one column while the main window shows `width` across
    /// (0 for what it shows now): that width, or a docked window its minimum width makes wider.
    /// It changes no window's size and runs no layout pass. 0 without a main window.
    pub fn shown_width(&mut self, width: i32) -> i32 {
        let Some(main) = self.main_layout() else {
            return 0;
        };
        let others: Vec<ObjId> = self.windows().into_iter().skip(1).collect();
        self.layout_root(main);
        let across = if width > 0 {
            width
        } else {
            self.content_right(main)
        };
        let widest = others
            .iter()
            .map(|l| self.size_at(*l, across).0)
            .max()
            .unwrap_or(0);
        across.max(widest)
    }

    /// How large the stack is in `columns` with a view and every window at its natural height,
    /// with a right column `column_width` wide, in layout pixels. It changes no window's size
    /// and runs no layout pass, so the skin hears nothing: the host weighs one column against
    /// two with it. `None` without a main window.
    pub fn natural_stack_size(&mut self, columns: i32, column_width: i32) -> Option<(i32, i32)> {
        let main = self.main_layout()?;
        let others: Vec<ObjId> = self.windows().into_iter().skip(1).collect();
        let (mut right, mut bottom) = self.layout_size(main);
        self.layout_root(main);
        for column in self.columns_of(main, &others, columns, column_width) {
            let mut y = column.top;
            for l in column.windows {
                let (w, h) = self.size_at(l, column.width);
                right = right.max(column.x + w);
                y += h;
            }
            bottom = bottom.max(y);
        }
        Some((right, bottom))
    }

    /// The left column of the stack in two columns, in layout pixels from the stack's corner:
    /// from the left edge to where the right column starts, and from the top to where the lowest
    /// window in it ends as Winamp docks to it. `None` for a stack in one column: the columns are
    /// not 2, or no list window is open.
    pub fn left_column(&mut self) -> Option<Rect> {
        let main = self.main_layout()?;
        let others: Vec<ObjId> = self.windows().into_iter().skip(1).collect();
        if self.columns != 2 || !others.iter().any(|l| self.shows_list(*l)) {
            return None;
        }
        let mut column = vec![main];
        column.extend(others.iter().copied().filter(|l| !self.shows_list(*l)));
        let mut right = 0;
        let mut bottom = 0;
        for l in column {
            let r = self.window_rect_of(l);
            right = right.max(r.right());
            bottom = bottom.max(r.top + self.snapped_size(l).1);
        }
        Some(Rect::new(0, 0, right, bottom))
    }

    /// How many times a script moved the main window against an edge of the viewport after the
    /// listener picked from one of its menus in the same touch.
    pub fn edge_placements(&self) -> i32 {
        self.edge_placements
    }

    pub(crate) fn main_placed_at_edge(&mut self) {
        if self.pointer.gesture_depth > 0 && self.pointer.picked_this_gesture {
            self.edge_placements += 1;
        }
    }

    /// Whether a skin window is open: on a phone (a view size), when it is in the stack,
    /// whatever its own visibility says; without one, as the window itself says.
    pub(crate) fn is_open(&self, c: ContainerId) -> bool {
        if self.view_size.is_none() {
            self.containers[c].visible
        } else {
            Some(c) == self.main_container() || self.opened.contains(&c)
        }
    }

    pub(crate) fn open_container(&mut self, c: ContainerId) {
        if Some(c) == self.main_container() || self.opened.contains(&c) {
            return;
        }
        self.opened.push(c);
        self.shown_before.insert(c);
        self.container_show(c);
        self.relayout();
    }

    pub(crate) fn close_container(&mut self, c: ContainerId) {
        let Some(at) = self.opened.iter().position(|o| *o == c) else {
            return;
        };
        self.opened.remove(at);
        self.container_hide(c);
        self.relayout();
    }

    /// Gives `l` the focus: a layout switched to takes it.
    pub(crate) fn focus(&mut self, l: ObjId) {
        self.focused = Some(l);
    }

    pub(crate) fn has_focus(&mut self, l: Option<ObjId>) -> bool {
        let Some(l) = l else { return false };
        let windows = self.windows();
        let focused = self
            .focused
            .filter(|f| windows.contains(f))
            .or_else(|| self.main_layout());
        Some(l) == focused
    }

    /// Gives `l` the focus as a touch does: the window losing it and `l` show their alphas and
    /// pictures for it on the next frame, with nothing moved or resized.
    pub(crate) fn take_focus(&mut self, l: ObjId) {
        if Some(l) == self.focused {
            return;
        }
        let windows = self.windows();
        let had = self
            .focused
            .filter(|f| windows.contains(f))
            .or_else(|| self.main_layout());
        self.focused = Some(l);
        if had == Some(l) {
            return;
        }
        if let Some(h) = had {
            self.refresh_alphas(h);
        }
        self.refresh_alphas(l);
        self.invalidate();
    }

    /// A script's Container.show / hide: the same as a button's, into the phone's stack and out.
    pub(crate) fn show_container(&mut self, c: ContainerId, show: bool) {
        if !show && !self.containers[c].can_close() {
            return;
        }
        if self.view_size.is_none() {
            if show {
                self.container_show(c)
            } else {
                self.container_hide(c)
            }
        } else if show {
            self.open_container(c)
        } else {
            self.close_container(c)
        }
    }

    /// Closes the window opened last.
    pub fn close_top_window(&mut self) {
        if let Some(c) = self.opened.last().copied() {
            self.close_container(c);
        }
    }

    pub(crate) fn container_show(&mut self, c: ContainerId) {
        if !self.containers[c].visible {
            self.containers[c].visible = true;
            self.invalidate();
            let cur = self.containers[c].current;
            self.dispatch(
                Handle::Container(c),
                "onshowlayout",
                &[Value::of(cur.map(Handle::Gui))],
            );
        }
    }

    pub(crate) fn container_hide(&mut self, c: ContainerId) {
        if self.containers[c].visible {
            self.containers[c].visible = false;
            self.invalidate();
            let cur = self.containers[c].current;
            self.dispatch(
                Handle::Container(c),
                "onhidelayout",
                &[Value::of(cur.map(Handle::Gui))],
            );
        }
    }

    /// Shows layout `id` of container `c`, or the first one when there is none by that id; the
    /// new layout hears onSetVisible(1) before the container's onSwitchToLayout, and takes the focus.
    pub(crate) fn switch_to(&mut self, c: ContainerId, id: &str) {
        let target = self
            .container_layout(c, id)
            .or_else(|| self.containers[c].layouts.first().copied());
        let Some(target) = target else { return };
        let current = self.containers[c].current;
        if current == Some(target) {
            return;
        }
        self.dispatch(
            Handle::Container(c),
            "onbeforeswitchtolayout",
            &[
                Value::of(current.map(Handle::Gui)),
                Value::Obj(Handle::Gui(target)),
            ],
        );
        if let Some(cur) = current {
            self.announce_visible(cur, false);
        }
        self.containers[c].current = Some(target);
        self.relayout();
        self.announce_visible(target, true);
        self.focus(target);
        self.dispatch(
            Handle::Container(c),
            "onswitchtolayout",
            &[Value::Obj(Handle::Gui(target))],
        );
    }

    /// The layout `id` of container `c`.
    pub(crate) fn container_layout(&self, c: ContainerId, id: &str) -> Option<ObjId> {
        self.containers[c]
            .layouts
            .iter()
            .copied()
            .find(|l| self.objects[*l].base.id.eq_ignore_ascii_case(id))
    }

    /// Puts each layout of container `c` where a phone draws it in a new viewport.
    pub(crate) fn fit_viewport(&mut self, c: ContainerId) {
        let middle = Some(c) == self.main_container();
        for l in self.containers[c].layouts.clone() {
            self.layout_fit_viewport(l, middle);
        }
    }

    // ---- change tracking ---------------------------------------------------------------------

    /// Asks for a frame and a new window shape.
    pub fn invalidate(&mut self) {
        self.dirty = true;
        self.change_count += 1;
    }

    /// Asks for a frame for a change that moves, resizes, shows, hides and reshapes nothing.
    pub(crate) fn repaint(&mut self) {
        self.dirty = true;
    }

    pub(crate) fn relayout(&mut self) {
        self.needs_layout = true;
        self.dirty = true;
        self.change_count += 1;
    }

    /// Whether the skin's scripts have started: what is set from here on is set by them.
    pub(crate) fn scripts_running(&self) -> bool {
        self.scripts_started
    }

    // ---- events ------------------------------------------------------------------------------

    /// Whether another event may fire inside the ones already firing.
    fn enter_event(&mut self, event: &str) -> bool {
        if self.event_depth >= MAX_EVENT_DEPTH {
            self.host.log(&format!(
                "event {event} not fired: events nested {} deep",
                self.event_depth
            ));
            return false;
        }
        self.event_depth += 1;
        self.firing.push(event.to_string());
        true
    }

    fn leave_event(&mut self) {
        self.event_depth -= 1;
        self.firing.pop();
    }

    /// The event firing right now, innermost.
    pub(crate) fn firing_now(&self) -> Option<&str> {
        self.firing.last().map(|s| s.as_str())
    }

    /// Fires `event` on `target` in every script; how many handlers ran.
    pub(crate) fn dispatch(&mut self, target: Handle, event: &str, args: &[Value]) -> i32 {
        if !self.enter_event(event) {
            return 0;
        }
        self.record(target, event, args);
        let mut n = 0;
        for s in 0..self.scripts.len() {
            if s < self.scripts.len() && self.scripts[s].hears(event) {
                n += self.script_dispatch(s, target, event, args).0;
            }
        }
        self.leave_event();
        // what happens to the child a xui group exposes happens to the group
        if let Handle::Gui(o) = target {
            if let Some(p) = self.xui_group_exposing(o) {
                n += self.dispatch(Handle::Gui(p), event, args);
            }
        }
        n
    }

    /// Fires `event` and gives back what the last handler that ran returned (Null when none ran).
    pub(crate) fn dispatch_for_result(
        &mut self,
        target: Handle,
        event: &str,
        args: &[Value],
    ) -> Value {
        if !self.enter_event(event) {
            return Value::Null;
        }
        self.record(target, event, args);
        let mut result = Value::Null;
        for s in 0..self.scripts.len() {
            if s < self.scripts.len() && self.scripts[s].hears(event) {
                let (n, last) = self.script_dispatch(s, target, event, args);
                if n > 0 {
                    result = last;
                }
            }
        }
        self.leave_event();
        if let Handle::Gui(o) = target {
            if let Some(p) = self.xui_group_exposing(o) {
                let r = self.dispatch_for_result(Handle::Gui(p), event, args);
                if r != Value::Null {
                    result = r;
                }
            }
        }
        result
    }

    /// The group that exposes `o` as its xui child, if any.
    fn xui_group_exposing(&self, o: ObjId) -> Option<ObjId> {
        let p = self.objects[o].base.parent?;
        let g = self.objects[p].group.as_ref()?;
        if g.embed_xui.as_deref() == Some(self.objects[o].base.id.as_str()) {
            Some(p)
        } else {
            None
        }
    }

    /// A script hears System events only once it has run onScriptLoaded (Wasabi SDK objcontroller).
    pub(crate) fn dispatch_system(&mut self, event: &str, args: &[Value]) {
        for s in 0..self.systems.len() {
            if self.systems[s].started {
                self.dispatch(Handle::System(s), event, args);
            }
        }
    }

    /// Runs a script's onScriptLoaded; its group's xui parameters follow at once.
    pub(crate) fn start_script(&mut self, s: usize) {
        self.systems[s].started = true;
        self.dispatch(Handle::System(s), "onscriptloaded", &[]);
        if let Some(g) = self.systems[s].group {
            self.deliver_xui_params(g);
        }
    }

    /// A xui element's parameters reach its scripts once they all ran onScriptLoaded.
    pub(crate) fn deliver_xui_params(&mut self, g: ObjId) {
        let Some(group) = self.objects[g].group.as_ref() else {
            return;
        };
        if group.xui_params.is_empty() {
            return;
        }
        // a default-set group the engine runs hears its parameters at once
        if group.behavior.is_some() {
            let params =
                std::mem::take(&mut self.objects[g].group.as_mut().expect("a group").xui_params);
            for (k, v) in params {
                self.behavior_xui_param(g, &k, &v);
            }
            return;
        }
        let systems = group.system_objects.clone();
        if systems.is_empty() || !systems.iter().all(|s| self.systems[*s].started) {
            return;
        }
        let params =
            std::mem::take(&mut self.objects[g].group.as_mut().expect("a group").xui_params);
        for (k, v) in params {
            for s in &systems {
                self.dispatch(
                    Handle::System(*s),
                    "onsetxuiparam",
                    &[Value::str(&k), Value::str(&v)],
                );
            }
        }
    }

    // ---- text placement --------------------------------------------------------------------

    /// Ascent and descent in whole pixels as Windows lays the face out at `size`, read from
    /// its file; `None` when the file gives none (or there is no file).
    fn gdi_line(&mut self, font: Option<&FontDef>, size: i32, bold: bool) -> Option<(i32, i32)> {
        let face = crate::gdi::FontFace::of(font, bold);
        let key = match &face {
            crate::gdi::FontFace::StandIn { .. } => face.file()?,
            crate::gdi::FontFace::SkinFile { font, .. } => font.id.to_string(),
            crate::gdi::FontFace::Default { .. } => return None,
        };
        let m = match self.gdi_lines.get(&key) {
            Some(m) => m.clone(),
            None => {
                let bytes: Option<Rc<[u8]>> = match &face {
                    crate::gdi::FontFace::StandIn { .. } => self.host.font_file(&key),
                    crate::gdi::FontFace::SkinFile { font, .. } => self.files.read(&font.file),
                    crate::gdi::FontFace::Default { .. } => None,
                };
                let m = bytes.and_then(|b| crate::gdi::GdiLineMetrics::read(&b));
                self.gdi_lines.insert(key, m.clone());
                m
            }
        };
        m.map(|m| m.at(crate::gdi::em_size(size)))
    }

    /// Places `cmd` in its rectangle, as every renderer then draws it: across by the width the
    /// host measures, less the ticker's scroll; up and down by the font's line, Windows' where
    /// the file gives it, else the host's.
    pub(crate) fn place_text(&mut self, cmd: &mut crate::draw::TextCmd) {
        let r = cmd.rect;
        let width = self
            .host
            .text_advance(&cmd.text, cmd.font.as_ref(), cmd.size, cmd.bold);
        cmd.x = match cmd.align {
            crate::draw::Align::Left => r.left as f32,
            crate::draw::Align::Center => r.left as f32 + (r.width as f32 - width) / 2.0,
            crate::draw::Align::Right => r.right() as f32 - width,
        } - cmd.scroll as f32;
        let (asc, desc) = self
            .gdi_line(cmd.font.as_ref(), cmd.size, cmd.bold)
            .map(|(a, d)| (a as f32, d as f32))
            .or_else(|| {
                self.host
                    .line_metrics(cmd.font.as_ref(), cmd.size, cmd.bold)
            })
            .unwrap_or_else(|| {
                let em = crate::gdi::em_size(cmd.size);
                (em * DEFAULT_ASCENT, em * DEFAULT_DESCENT)
            });
        cmd.baseline = match cmd.valign {
            crate::draw::VAlign::Top => r.top as f32 + asc,
            crate::draw::VAlign::Center => {
                r.top as f32 + ((r.height as f32 - (asc + desc)) / 2.0).trunc() + asc
            }
            crate::draw::VAlign::Bottom => r.bottom() as f32 - desc,
        };
    }

    // ---- shapes and fitting ------------------------------------------------------------------

    /// The start of `layout`'s shaped window with its pictures fitted to their sheets and tinted
    /// for the color theme. A window keeps its shape until the skin changes, and a new theme
    /// changes it, so the same shape gives back the same command from frame to frame.
    pub(crate) fn window_start(
        &mut self,
        layout: ObjId,
        region: &Region,
        shape_at: i64,
    ) -> DrawCmd {
        if let Some((at, cmd)) = self.shaped.get(&layout) {
            if *at == shape_at {
                return cmd.clone();
            }
        }
        let mut fitted = region
            .map_pics(&mut |img| self.fit_region_image(img))
            .unwrap_or(Region::Op {
                base: None,
                add: Vec::new(),
                keep: Vec::new(),
                cut: Vec::new(),
            });
        let mut gammas: Vec<Option<Gamma>> = Vec::new();
        fitted.pics_mut(&mut |img| {
            gammas.push(Some(img.gamma.unwrap_or(Gamma {
                r: 0,
                g: 0,
                b: 0,
                boost: 0,
                gray: 0,
            })))
        });
        // tints are looked up through the runtime, so apply them in a second pass
        let mut groups: Vec<Option<Rc<str>>> = Vec::new();
        fitted.pics_mut(&mut |img| groups.push(img.bitmap.gamma_group.clone()));
        let tints: Vec<Option<Gamma>> = groups.iter().map(|g| self.gamma(g.as_deref())).collect();
        let mut i = 0;
        fitted.pics_mut(&mut |img| {
            img.gamma = tints[i];
            i += 1;
        });
        let cmd = DrawCmd::WindowStart(fitted);
        self.shaped.insert(layout, (shape_at, cmd.clone()));
        cmd
    }

    fn fit_region_image(
        &mut self,
        img: &crate::draw::RegionImage,
    ) -> Option<crate::draw::RegionImage> {
        let (bitmap, dst) = self.fit_part_to_sheet(&img.bitmap, img.dst)?;
        Some(crate::draw::RegionImage {
            bitmap,
            dst,
            alpha: img.alpha,
            gamma: img.gamma,
            smooth: img.smooth,
        })
    }

    /// The part of an image command that lies on its sheet, with the destination cut the same
    /// way: Winamp draws nothing for rows or columns a bitmap element asks for past the edge of
    /// its file. `None` when nothing is left.
    pub(crate) fn fit_part_to_sheet(
        &mut self,
        b: &BitmapDef,
        dst: Rect,
    ) -> Option<(BitmapDef, Rect)> {
        if !self.fits.contains_key(b) {
            if let Some(fit) = self.fit_part(b) {
                self.fits.insert(b.clone(), fit);
            }
        }
        match self.fits.get(b) {
            None | Some(Fit::Whole) => Some((b.clone(), dst)),
            Some(Fit::None) => None,
            Some(Fit::Cut {
                part,
                asked_w,
                asked_h,
            }) => {
                let fw = part.w.unwrap_or(*asked_w);
                let fh = part.h.unwrap_or(*asked_h);
                // a stretched picture keeps its scale: the destination shrinks by the same share
                let dw = if fw == *asked_w {
                    dst.width
                } else {
                    dst.width * fw / *asked_w
                };
                let dh = if fh == *asked_h {
                    dst.height
                } else {
                    dst.height * fh / *asked_h
                };
                if dw <= 0 || dh <= 0 {
                    None
                } else {
                    Some((part.clone(), Rect::new(dst.left, dst.top, dw, dh)))
                }
            }
        }
    }

    /// Works out once how much of the part `b` names lies on its sheet; a sheet whose size is
    /// not known yet leaves it to the next frame.
    fn fit_part(&mut self, b: &BitmapDef) -> Option<Fit> {
        let (sheet_w, sheet_h) = self.host.image_size(&b.file)?;
        let sw = b.w.unwrap_or(sheet_w - b.x);
        let sh = b.h.unwrap_or(sheet_h - b.y);
        let fw = sw.min(sheet_w - b.x);
        let fh = sh.min(sheet_h - b.y);
        let empty = fw <= 0 || fh <= 0;
        let outside = sw <= 0 || sh <= 0;
        Some(if empty || outside {
            Fit::None
        } else if fw == sw && fh == sh && b.w.is_some() && b.h.is_some() {
            Fit::Whole
        } else {
            Fit::Cut {
                part: b.part(b.x, b.y, Some(fw), Some(fh)),
                asked_w: sw,
                asked_h: sh,
            }
        })
    }

    // ---- objects -----------------------------------------------------------------------------

    /// The layout `o` is in.
    pub(crate) fn layout_object(&self, o: ObjId) -> Option<ObjId> {
        let mut at = Some(o);
        while let Some(id) = at {
            if self.objects[id].is_layout() {
                return Some(id);
            }
            at = self.objects[id].base.parent;
        }
        None
    }

    /// `findObject(id)` on a group: its children, then their subtrees.
    pub(crate) fn find_object(&self, group: ObjId, wanted: &str) -> Option<ObjId> {
        let lower = wanted.to_lowercase();
        let children = self.objects[group].children();
        for c in children {
            let o = &self.objects[*c];
            if o.base.id == lower
                || o.group
                    .as_ref()
                    .is_some_and(|g| g.instance_id.as_deref() == Some(lower.as_str()))
            {
                return Some(*c);
            }
        }
        for c in children {
            if self.objects[*c].group.is_some() {
                if let Some(f) = self.find_object(*c, &lower) {
                    return Some(f);
                }
            }
        }
        None
    }

    /// `findObject(id)` as Wasabi's windows do it: this object's children, then their subtrees,
    /// then up one parent at a time, so any object in the window is found from anywhere in it.
    pub(crate) fn find_object_chain(&self, o: ObjId, id: &str) -> Option<ObjId> {
        if self.objects[o].group.is_some() {
            if let Some(f) = self.find_object(o, id) {
                return Some(f);
            }
        }
        let mut p = self.objects[o].base.parent;
        while let Some(g) = p {
            if let Some(f) = self.find_object(g, id) {
                return Some(f);
            }
            p = self.objects[g].base.parent;
        }
        None
    }

    /// The child named by `embed_xui`, if any.
    pub(crate) fn embedded(&self, g: ObjId) -> Option<ObjId> {
        let e = self.objects[g].group.as_ref()?.embed_xui.clone()?;
        self.objects[g]
            .children()
            .iter()
            .copied()
            .find(|c| self.objects[*c].base.id == e)
            .or_else(|| self.find_object(g, &e))
    }

    /// Puts `child` into `group`.
    pub(crate) fn add_child(&mut self, group: ObjId, child: ObjId) {
        if let Some(old) = self.objects[child].base.parent {
            if let Some(g) = self.objects[old].group.as_mut() {
                g.children.retain(|c| *c != child);
            }
        }
        self.objects[child].base.parent = Some(group);
        self.objects[group]
            .group
            .as_mut()
            .expect("a group")
            .children
            .push(child);
        self.invalidate();
    }

    /// Puts an object into `group`, as a script does with `obj.init(group)`.
    pub(crate) fn init_into(&mut self, o: ObjId, group: ObjId) {
        self.add_child(group, o);
        self.relayout();
    }

    /// Whether `o` shows: in a window on screen, it and every group around it visible, not
    /// faded out, and with room to draw in.
    pub(crate) fn shows(&mut self, o: ObjId) -> bool {
        let b = &self.objects[o].base;
        let room = b.rect.width > 0 && b.rect.height > 0;
        if !room || !b.visible || b.effective_alpha <= 0 {
            return false;
        }
        let mut top = o;
        while let Some(up) = self.objects[top].base.parent {
            if !self.objects[up].base.visible {
                return false;
            }
            top = up;
        }
        self.objects[top].is_layout() && self.windows().contains(&top)
    }

    /// Whether `o` can take a touch at all. `faded` also lets in one drawn fully transparent.
    pub(crate) fn touchable(&self, o: ObjId, faded: bool) -> bool {
        let b = &self.objects[o].base;
        if !(b.visible
            && b.enabled
            && (faded || b.effective_alpha > 0)
            && b.rect.width > 0
            && b.rect.height > 0)
        {
            return false;
        }
        let mut p = b.parent;
        while let Some(g) = p {
            if !self.objects[g].base.visible {
                return false;
            }
            p = self.objects[g].base.parent;
        }
        true
    }

    /// Whether every group above `o` is shown.
    pub(crate) fn parent_shown(&self, o: ObjId) -> bool {
        let mut p = self.objects[o].base.parent;
        while let Some(g) = p {
            if !self.objects[g].base.visible {
                return false;
            }
            p = self.objects[g].base.parent;
        }
        true
    }

    /// onSetVisible for `o` and, down the tree, every child whose own flag is set.
    pub(crate) fn announce_visible(&mut self, o: ObjId, shown: bool) {
        self.dispatch(Handle::Gui(o), "onsetvisible", &[Value::Int(shown as i32)]);
        for c in self.objects[o].children().to_vec() {
            if self.objects[c].base.visible {
                self.announce_visible(c, shown);
            }
        }
    }

    /// Shows `o`.
    pub(crate) fn show_object(&mut self, o: ObjId) {
        if !self.objects[o].base.visible {
            self.objects[o].base.visible = true;
            self.shown_changed(o);
            if self.parent_shown(o) {
                self.announce_visible(o, true);
            }
        }
    }

    /// Hides `o`.
    pub(crate) fn hide_object(&mut self, o: ObjId) {
        if self.objects[o].base.visible {
            self.objects[o].base.visible = false;
            self.shown_changed(o);
            if self.parent_shown(o) {
                self.announce_visible(o, false);
            }
        }
    }

    /// A frame gives a hidden pane's room to the other one.
    fn shown_changed(&mut self, o: ObjId) {
        let in_frame = self.objects[o]
            .base
            .parent
            .is_some_and(|p| self.objects[p].base.tag == "wasabi:frame");
        if in_frame {
            self.relayout()
        } else {
            self.invalidate()
        }
    }

    /// `action;param;target` as a button or a double click does it.
    pub(crate) fn run_action(&mut self, o: ObjId, spec: &str) {
        let parts: Vec<&str> = spec.split(';').collect();
        let param = parts.get(1).map(|s| s.trim().to_string());
        let target = parts.get(2).map(|s| s.trim().to_string());
        self.action(o, parts[0].trim(), param.as_deref(), target.as_deref());
    }

    /// The window a TOGGLE names: a container id, a GUID alias such as "pl", or a component GUID.
    pub(crate) fn toggle_target(&self, id: &str) -> Option<ContainerId> {
        let guid = toggle_guid(id)
            .map(|g| g.to_string())
            .unwrap_or_else(|| id.trim_matches(['{', '}']).to_string());
        self.container(id).or_else(|| {
            self.containers.iter().position(|c| {
                c.attr("component").map(|v| {
                    v.to_lowercase()
                        .trim_start_matches("guid:")
                        .trim_matches(['{', '}'])
                        .to_string()
                }) == Some(guid.clone())
            })
        })
    }

    /// The short name of a window a TOGGLE names by short name or component GUID.
    pub(crate) fn window_kind(&self, id: &str) -> String {
        let bare = id.trim_matches(['{', '}']).to_lowercase();
        // a GUID or an alias ("library") by its first alias ("ml")
        let guid = toggle_guid(&bare)
            .map(|g| g.to_string())
            .unwrap_or_else(|| bare.clone());
        TOGGLE_GUIDS
            .iter()
            .find(|(_, g)| *g == guid)
            .map(|(k, _)| k.to_string())
            .unwrap_or(bare)
    }

    /// Whether a component, a window holder or a container with the attributes `attrs` shows
    /// the component `guid`, named by its `hold`, `param` or `component` attribute: by GUID or its alias.
    pub(crate) fn holds(attrs: &[(String, String)], guid: &str) -> bool {
        ["hold", "param", "component"].iter().any(|k| {
            attrs
                .iter()
                .find(|(key, _)| key == k)
                .is_some_and(|(_, v)| {
                    v.split(';').any(|p| {
                        let id = p.trim().to_lowercase();
                        let id = id.trim_start_matches("guid:").trim_matches(['{', '}']);
                        id == guid || toggle_guid(id) == Some(guid)
                    })
                })
        })
    }

    /// The windows on screen changed: tells the host when they did since the last time.
    fn tell_windows(&mut self) {
        let ids = self.window_ids();
        if ids != self.told_windows {
            self.told_windows = ids.clone();
            self.host.windows_changed(&ids);
        }
    }

    /// Lays the main window out now, for a script that asks where something is.
    pub fn ensure_layout(&mut self) {
        if self.needs_layout {
            self.do_layout();
        }
    }

    /// Lays out until scripts stop changing the tree (a few passes at most).
    pub(crate) fn do_layout(&mut self) {
        let mut passes = 0;
        loop {
            self.layout_pass();
            passes += 1;
            if !(self.needs_layout && passes < MAX_LAYOUT_PASSES) {
                break;
            }
        }
        self.tell_windows();
    }

    /// The stack's size, in layout pixels.
    pub fn stack_size(&mut self) -> (i32, i32) {
        if self.needs_layout {
            self.do_layout();
        }
        let rects: Vec<Rect> = self
            .windows()
            .into_iter()
            .map(|l| self.window_rect_of(l))
            .collect();
        (
            rects.iter().map(|r| r.right()).max().unwrap_or(0),
            rects.iter().map(|r| r.bottom()).max().unwrap_or(0),
        )
    }

    /// The part of the stack its windows show, in layout pixels: the stack with each window cut
    /// on its right to where it shows and on its bottom to the part Winamp docks and snaps to.
    pub fn drawn_size(&mut self) -> (i32, i32) {
        if self.needs_layout {
            self.do_layout();
        }
        let mut right = 0;
        let mut bottom = 0;
        for l in self.windows() {
            let r = self.window_rect_of(l);
            right = right.max(r.left + self.content_right(l));
            bottom = bottom.max(r.top + self.snapped_size(l).1);
        }
        (right, bottom)
    }

    /// The main window's drawing.
    pub fn draw_main(&mut self) -> Vec<DrawCmd> {
        match self.main_layout() {
            Some(l) => self.draw_layout(l),
            None => Vec::new(),
        }
    }

    /// Every window's drawing with its place in the stack; draw each translated to its rectangle.
    pub fn draw_windows(&mut self) -> Vec<(Rect, Vec<DrawCmd>)> {
        if self.needs_layout {
            self.do_layout();
        }
        self.windows()
            .into_iter()
            .map(|l| (self.window_rect_of(l), self.draw_layout(l)))
            .collect()
    }

    /// Every window's drawing as [`SkinRuntime::draw_windows`] gives it, with the object that
    /// drew each command, for a host that plans what it keeps between frames.
    pub fn draw_windows_owned(&mut self) -> Vec<DrawnWindow> {
        if self.needs_layout {
            self.do_layout();
        }
        self.windows()
            .into_iter()
            .map(|l| {
                let rect = self.window_rect_of(l);
                let (cmds, owners) = self.draw_layout_owned(l);
                DrawnWindow { rect, cmds, owners }
            })
            .collect()
    }

    /// The objects the last tick found moving by themselves (a ticker stepping, a visualizer, a
    /// seek thumb), taken once: a host's planner counts them live from their first change.
    pub fn take_tick_changed(&mut self) -> Vec<ObjId> {
        std::mem::take(&mut self.tick_changed)
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
