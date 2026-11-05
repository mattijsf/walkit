// SPDX-License-Identifier: GPL-3.0-or-later

//! The objects scripts make (`new Timer`, `new PopupMenu`, ...), the skin's `Config` items, and
//! the one door every script method call goes through: `maki_call`.

use std::collections::HashMap;

use super::SkinRuntime;
use crate::gui::{Kind, Obj};
use crate::host::{MenuItem, SkinMenu};
use crate::maki::{stdapi, Handle, Value};

/// Objects scripts made with `new`, at most.
const MAX_CREATED: i64 = 20_000;
/// A list stops growing here: a script that adds without end is stopped.
const MAX_LIST_ITEMS: usize = 100_000;
/// Submenu levels shown; deeper ones are left out.
pub(crate) const MENU_MAX_DEPTH: usize = 8;
/// Items shown in one menu and its submenus together; later ones are left out.
pub(crate) const MENU_MAX_ITEMS: i32 = 512;
/// Characters shown of an item's words.
const MENU_MAX_TEXT: usize = 128;
/// A handler that sets its own attribute again stops echoing here.
const MAX_NESTED: i32 = 4;
/// Winamp 5.666's build number.
const BUILD: i32 = 3516;

/// One entry of a script's popup menu.
#[derive(Debug, Clone)]
pub(crate) enum MenuEntry {
    Command {
        id: i32,
        text: String,
        checked: bool,
        disabled: bool,
    },
    Submenu {
        text: String,
        menu: Option<usize>,
    },
    Separator,
}

/// An object a script made, or one of the skin's settings objects.
#[derive(Debug, Clone)]
pub(crate) enum Made {
    /// `new Timer`: fires `onTimer` every `delay` milliseconds while started.
    Timer {
        delay: i32,
        running: bool,
        next_at: i64,
    },
    /// `new PopupMenu`: commands, separators and submenus the script adds.
    PopupMenu { entries: Vec<MenuEntry> },
    /// `new Map`: reads values from a greyscale bitmap.
    Map { bitmap: Option<String> },
    /// `new List`: a plain list of values.
    List { items: Vec<Value> },
    /// The skin's own settings.
    Config,
    /// One `Config` item: its attributes by lowercase name.
    ConfigItem {
        name: String,
        guid: Option<String>,
        attributes: Vec<(String, usize)>,
    },
    /// A skin setting.
    ConfigAttribute {
        item: usize,
        name: String,
        data: String,
        made: bool,
        dispatching: i32,
    },
    /// Winamp's own settings, grouped by GUID.
    WinampConfig { groups: Vec<(String, usize)> },
    /// One group of Winamp's settings.
    WinampConfigGroup { values: HashMap<String, String> },
    /// `Application`: who runs the skin.
    Application,
    /// Anything else a script makes: every call answers nothing, so the script keeps running.
    Stub {
        name: String,
        data: HashMap<String, String>,
    },
}

/// A menu item's words for the phone: without the keyboard shortcut Windows shows after a tab,
/// and without the `&` that marks the underlined letter (`&&` is a plain `&`).
pub(crate) fn shown_text(text: &str) -> String {
    let words = text.split_once('\t').map(|(w, _)| w).unwrap_or(text);
    let chars: Vec<char> = words.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '&' {
            i += 1;
        }
        if i < chars.len() {
            out.push(chars[i]);
        }
        i += 1;
    }
    out.trim().chars().take(MENU_MAX_TEXT).collect()
}

/// Whether `s` is written as a GUID, with or without braces.
pub(crate) fn is_guid(s: &str) -> bool {
    let t = s.trim();
    let t = t
        .strip_prefix('{')
        .map(|r| r.strip_suffix('}').unwrap_or(r))
        .unwrap_or(t);
    let parts: Vec<&str> = t.split('-').collect();
    let lens = [8, 4, 4, 4, 12];
    parts.len() == 5
        && parts
            .iter()
            .zip(lens.iter())
            .all(|(p, l)| p.len() == *l && p.bytes().all(|b| b.is_ascii_hexdigit()))
        && (!s.trim().starts_with('{') || s.trim().ends_with('}'))
}

/// Whether two GUIDs are the same, written with or without braces.
fn same_guid(a: Option<&str>, b: &str) -> bool {
    a.is_some_and(|a| {
        a.trim_matches(['{', '}'])
            .eq_ignore_ascii_case(b.trim_matches(['{', '}']))
    })
}

/// A value read as a number as Winamp reads one from text: an optional sign and the leading digits, 0 for none.
pub(crate) fn wtoi(s: &str) -> i32 {
    let t = s.trim_start();
    let neg = t.starts_with('-');
    let digits = t.strip_prefix(['-', '+']).unwrap_or(t);
    let mut n: i64 = 0;
    for c in digits.chars() {
        let Some(d) = c.to_digit(10) else { break };
        if n >= i32::MAX as i64 {
            break;
        }
        n = n * 10 + d as i64;
    }
    let n = if neg { -n } else { n };
    n.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

fn arg(args: &[Value], i: usize) -> Value {
    args.get(i).cloned().unwrap_or(Value::Null)
}

impl SkinRuntime {
    /// `new Timer`, `new PopupMenu`, and so on. `None` when the class cannot be made here.
    pub(crate) fn create(&mut self, guid: &str) -> Option<Handle> {
        let cls = stdapi::class(&guid.to_lowercase())?;
        self.created += 1;
        if self.created > MAX_CREATED {
            if self.created == MAX_CREATED + 1 {
                self.host.log(&format!(
                    "scripts made more than {MAX_CREATED} objects; no more"
                ));
            }
            return None;
        }
        let made = match cls.name.to_lowercase().as_str() {
            "timer" => Made::Timer {
                delay: 0,
                running: false,
                next_at: 0,
            },
            "popupmenu" => Made::PopupMenu {
                entries: Vec::new(),
            },
            "map" => Made::Map { bitmap: None },
            "list" => Made::List { items: Vec::new() },
            "config" => Made::Config,
            "winampconfig" => Made::WinampConfig { groups: Vec::new() },
            "group" => {
                let id = self.objects.len();
                self.objects.push(Obj::new("group", Kind::Group, true));
                return Some(Handle::Gui(id));
            }
            "layer" => {
                let id = self.objects.len();
                self.objects
                    .push(Obj::new("layer", Kind::Layer(Default::default()), false));
                return Some(Handle::Gui(id));
            }
            "application" => Made::Application,
            _ => Made::Stub {
                name: cls.name.to_string(),
                data: HashMap::new(),
            },
        };
        Some(self.push_made(made))
    }

    fn push_made(&mut self, m: Made) -> Handle {
        self.made.push(m);
        Handle::Made(self.made.len() - 1)
    }

    /// Whether `h` is a stand-in object; see [`SkinRuntime::another_stub`].
    pub(crate) fn is_stub(&self, h: Handle) -> bool {
        matches!(h, Handle::Made(i) if matches!(self.made.get(i), Some(Made::Stub { .. })))
    }

    /// A stand-in object hands out more of them, so a script keeps going through objects this runtime lacks.
    pub(crate) fn another_stub(&mut self) -> Handle {
        self.push_made(Made::Stub {
            name: "Object".into(),
            data: HashMap::new(),
        })
    }

    /// A script's `delete`: the object stops what it does (a timer no longer fires).
    pub(crate) fn delete_object(&mut self, h: Handle) {
        if let Handle::Made(i) = h {
            if matches!(self.made.get(i), Some(Made::Timer { .. })) {
                self.timer_stop(i);
            }
        }
    }

    /// The class name the reference engine's objects carry, for log lines that name a receiver.
    pub(crate) fn kotlin_name(&self, h: Handle) -> String {
        match h {
            Handle::Gui(o) => {
                let obj = &self.objects[o];
                match &obj.kind {
                    Kind::Unknown { .. } => "Unknown",
                    Kind::Group => "Group",
                    Kind::Frame(_) => "FrameSplitter",
                    Kind::Bucket(_) => "ComponentBucket",
                    Kind::XFade(_) => "GroupXFade",
                    Kind::Vis(_) => "Vis",
                    Kind::EqVis(_) => "EqVis",
                    Kind::ColorThemeList(_) => "ColorThemeList",
                    Kind::Gradient(_) => "Gradient",
                    Kind::PlaylistView(_) => "PlaylistView",
                    Kind::LayoutStatus(_) => "LayoutStatus",
                    Kind::Layout(_) => "Layout",
                    Kind::Layer(_) => "Layer",
                    Kind::AnimatedLayer(..) => "AnimatedLayer",
                    Kind::Button(b) => match b.kind {
                        crate::gui::ButtonKind::Plain => "Button",
                        crate::gui::ButtonKind::Toggle { .. } => "ToggleButton",
                        crate::gui::ButtonKind::NStates { .. } => "NStatesButton",
                    },
                    Kind::Text(_) => "Text",
                    Kind::Slider(_) => "Slider",
                    Kind::Status(_) => "Status",
                    Kind::Grid(_) => "Grid",
                    Kind::ProgressGrid(..) => "ProgressGrid",
                    Kind::AlbumArt { .. } => "AlbumArt",
                    Kind::RectObj(_) => "RectObj",
                    Kind::Images(_) => "Images",
                    Kind::Menu(_) => "MenuButton",
                }
                .to_string()
            }
            Handle::Container(_) => "Container".into(),
            Handle::System(_) => "SystemObject".into(),
            Handle::Made(i) => match &self.made[i] {
                Made::Timer { .. } => "Timer",
                Made::PopupMenu { .. } => "PopupMenu",
                Made::Map { .. } => "MakiMap",
                Made::List { .. } => "MakiList",
                Made::Config => "ConfigObj",
                Made::ConfigItem { .. } => "ConfigItemObj",
                Made::ConfigAttribute { .. } => "ConfigAttributeObj",
                Made::WinampConfig { .. } => "WinampConfigObj",
                Made::WinampConfigGroup { .. } => "WinampConfigGroupObj",
                Made::Application => "ApplicationObj",
                Made::Stub { .. } => "Stub",
            }
            .to_string(),
        }
    }

    /// Runs a method named in `std.mi` (lowercase) on `receiver`. `None` when the method is unknown there.
    pub(crate) fn maki_call(
        &mut self,
        receiver: Handle,
        name: &str,
        args: &[Value],
    ) -> Option<Value> {
        match receiver {
            Handle::Gui(o) => self.gui_maki_call(o, name, args),
            Handle::Container(c) => self.container_maki_call(c, name, args),
            Handle::System(s) => self.system_maki_call(s, name, args),
            Handle::Made(i) => self.made_maki_call(i, name, args),
        }
    }

    // ---- timers ------------------------------------------------------------------------------

    fn timer_start(&mut self, i: usize) {
        let now = self.now();
        let Made::Timer {
            delay,
            running,
            next_at,
        } = &mut self.made[i]
        else {
            return;
        };
        if *delay <= 0 {
            return;
        }
        *running = true;
        *next_at = now + *delay as i64;
        // started again while running, it only starts its wait over
        if !self.timers.contains(&i) {
            self.timers.push(i);
        }
    }

    fn timer_stop(&mut self, i: usize) {
        if let Made::Timer { running, .. } = &mut self.made[i] {
            *running = false;
        }
        self.timers.retain(|t| *t != i);
    }

    /// A running timer's next firing, or `None` for one that is stopped.
    pub(crate) fn timer_next_at(&self, i: usize) -> Option<i64> {
        match self.made.get(i) {
            Some(Made::Timer {
                running: true,
                next_at,
                ..
            }) => Some(*next_at),
            _ => None,
        }
    }

    /// Fires the timers due at `t`.
    pub(crate) fn fire_timers(&mut self, t: i64) {
        let due: Vec<usize> = self
            .timers
            .iter()
            .copied()
            .filter(|i| matches!(self.made.get(*i), Some(Made::Timer { running: true, next_at, .. }) if t >= *next_at))
            .collect();
        for i in due {
            if let Made::Timer { delay, next_at, .. } = &mut self.made[i] {
                *next_at = t + (*delay).max(1) as i64;
            }
            self.timer_fires += 1;
            self.dispatch(Handle::Made(i), "ontimer", &[]);
        }
    }

    // ---- menus -------------------------------------------------------------------------------

    /// The menu as the host shows it, as it stands now. It holds at most `MENU_MAX_ITEMS` items
    /// in all and `MENU_MAX_DEPTH` levels of submenus, so a menu that holds itself ends.
    pub(crate) fn popup_to_menu(&self, i: usize) -> SkinMenu {
        let mut left = MENU_MAX_ITEMS;
        self.menu_snapshot(i, 0, &mut left)
    }

    fn menu_snapshot(&self, i: usize, depth: usize, left: &mut i32) -> SkinMenu {
        let mut items = Vec::new();
        let Some(Made::PopupMenu { entries }) = self.made.get(i) else {
            return SkinMenu { items };
        };
        for e in entries {
            if *left <= 0 {
                break;
            }
            let item = match e {
                MenuEntry::Command {
                    id,
                    text,
                    checked,
                    disabled,
                } => Some(MenuItem::Command {
                    id: *id,
                    text: shown_text(text),
                    checked: *checked,
                    disabled: *disabled,
                }),
                MenuEntry::Submenu { text, menu } => {
                    menu.filter(|_| depth < MENU_MAX_DEPTH).map(|sub| {
                        *left -= 1;
                        MenuItem::Submenu {
                            text: shown_text(text),
                            menu: self.menu_snapshot(sub, depth + 1, left),
                        }
                    })
                }
                MenuEntry::Separator => Some(MenuItem::Separator),
            };
            // a submenu counted itself before its items
            if let Some(item) = item {
                if !matches!(item, MenuItem::Submenu { .. }) {
                    *left -= 1;
                }
                items.push(item);
            }
        }
        SkinMenu { items }
    }

    // ---- config ------------------------------------------------------------------------------

    /// A `Config` item by name or GUID; `guid` given finds the item made with that GUID, and
    /// makes it when it does not exist yet, so getItem(GUID) and newItem(name, GUID) give one
    /// item in either order. An item getItem made under a name takes the GUID a later newItem
    /// of that name gives it.
    pub(crate) fn config_item(&mut self, key: &str, guid: Option<&str>) -> Option<usize> {
        let k = key.to_lowercase();
        if let Some(&item) = self
            .config_items
            .iter()
            .find(|(n, _)| **n == k)
            .map(|(_, i)| i)
        {
            return Some(self.named_item(&k, item, guid));
        }
        let found = self
            .config_item_by_guid(key)
            .or_else(|| guid.and_then(|g| self.config_item_by_guid(g)));
        if let Some(found) = found {
            if guid.is_some() {
                self.config_items.push((k, found));
            }
            return Some(found);
        }
        let g = guid?;
        let item = self.push_made(Made::ConfigItem {
            name: key.to_string(),
            guid: Some(g.to_string()),
            attributes: Vec::new(),
        });
        let Handle::Made(i) = item else { return None };
        self.config_items.push((k, i));
        self.config_items.push((g.to_lowercase(), i));
        self.config_changed();
        Some(i)
    }

    /// `named`, found under `k`: it takes `guid` when it has none yet, or gives way to the item that holds `guid`.
    fn named_item(&mut self, k: &str, named: usize, guid: Option<&str>) -> usize {
        let Some(guid) = guid else { return named };
        let named_guid = match &self.made[named] {
            Made::ConfigItem { guid, .. } => guid.clone(),
            _ => None,
        };
        if !is_guid(guid) || named_guid.as_deref().is_some_and(is_guid) {
            return named;
        }
        if let Some(holder) = self.config_item_by_guid(guid) {
            if let Some(entry) = self.config_items.iter_mut().find(|(n, _)| n == k) {
                entry.1 = holder;
            }
            return holder;
        }
        if let Made::ConfigItem { guid: g, .. } = &mut self.made[named] {
            *g = Some(guid.to_string());
        }
        self.config_items.push((guid.to_lowercase(), named));
        self.config_changed();
        named
    }

    pub(crate) fn config_item_by_guid(&self, guid: &str) -> Option<usize> {
        self.config_items
            .iter()
            .map(|(_, i)| *i)
            .find(|i| match &self.made[*i] {
                Made::ConfigItem { guid: g, .. } => same_guid(g.as_deref(), guid),
                _ => false,
            })
    }

    /// The distinct Config items, in the order they were made.
    fn config_item_list(&self) -> Vec<usize> {
        let mut out = Vec::new();
        for (_, i) in &self.config_items {
            if !out.contains(i) {
                out.push(*i);
            }
        }
        out
    }

    /// A config attribute a script made (Bento's "Show Playlist Tab"): a cfgattrib names it, and
    /// the button and the script then share one value. With none of that name, the one attribute
    /// whose name differs only in a note in brackets at its end.
    pub(crate) fn script_attribute(&self, name: &str) -> Option<usize> {
        let k = name.to_lowercase();
        for i in self.config_item_list() {
            if let Made::ConfigItem { attributes, .. } = &self.made[i] {
                if let Some((_, a)) = attributes.iter().find(|(n, _)| *n == k) {
                    return Some(*a);
                }
            }
        }
        let bare = without_note(&k);
        if bare == k {
            return None;
        }
        let mut found: Vec<usize> = Vec::new();
        for i in self.config_item_list() {
            if let Made::ConfigItem { attributes, .. } = &self.made[i] {
                for (_, a) in attributes {
                    if let Made::ConfigAttribute {
                        made: true, name, ..
                    } = &self.made[*a]
                    {
                        if without_note(&name.to_lowercase()) == bare && !found.contains(a) {
                            found.push(*a);
                        }
                    }
                }
            }
        }
        if found.len() == 1 {
            Some(found[0])
        } else {
            None
        }
    }

    /// The data of attribute `a`.
    pub(crate) fn attribute_data(&self, a: usize) -> String {
        match &self.made[a] {
            Made::ConfigAttribute { data, .. } => data.clone(),
            _ => String::new(),
        }
    }

    /// `setData` on attribute `a`: every setData tells the listeners, the same value included.
    pub(crate) fn attribute_set_data(&mut self, a: usize, value: &str) {
        let (was, name) = match &self.made[a] {
            Made::ConfigAttribute { data, name, .. } => (data.clone(), name.clone()),
            _ => return,
        };
        if let Made::ConfigAttribute { data, .. } = &mut self.made[a] {
            *data = value.to_string();
        }
        if value != was {
            self.config_changed();
        }
        // a button that shows the value shows the new one
        if self
            .stored_settings
            .insert(name, value.to_string())
            .as_deref()
            != Some(value)
        {
            self.settings_changed();
            self.invalidate();
        }
        let nested = match &self.made[a] {
            Made::ConfigAttribute { dispatching, .. } => *dispatching,
            _ => 0,
        };
        if nested < MAX_NESTED {
            if let Made::ConfigAttribute { dispatching, .. } = &mut self.made[a] {
                *dispatching += 1;
            }
            self.dispatch(Handle::Made(a), "ondatachanged", &[]);
            if let Made::ConfigAttribute { dispatching, .. } = &mut self.made[a] {
                *dispatching -= 1;
            }
        }
    }

    // ---- the made objects' methods -----------------------------------------------------------

    fn made_maki_call(&mut self, i: usize, name: &str, args: &[Value]) -> Option<Value> {
        let kind = self.made[i].clone();
        Some(match kind {
            Made::Timer { delay, running, .. } => match name {
                "getclassname" => Value::str("Timer"),
                "getid" => Value::str(""),
                "setdelay" => {
                    let d = arg(args, 0).as_int();
                    let now = self.now();
                    if let Made::Timer {
                        delay,
                        next_at,
                        running,
                        ..
                    } = &mut self.made[i]
                    {
                        *delay = d;
                        if *running {
                            *next_at = now + d as i64;
                        }
                    }
                    Value::Null
                }
                "getdelay" => Value::Int(delay),
                "start" => {
                    self.timer_start(i);
                    Value::Null
                }
                "stop" => {
                    self.timer_stop(i);
                    Value::Null
                }
                "isrunning" => Value::Bool(running),
                "getskipped" => Value::Int(0),
                _ => return None,
            },
            Made::PopupMenu { .. } => match name {
                "getclassname" => Value::str("PopupMenu"),
                "getid" => Value::str(""),
                // addCommand(text, id, checked, disabled)
                "addcommand" => {
                    let e = MenuEntry::Command {
                        id: arg(args, 1).as_int(),
                        text: arg(args, 0).as_string().to_string(),
                        checked: args.get(2).is_some_and(|a| a.truthy()),
                        disabled: args.get(3).is_some_and(|a| a.truthy()),
                    };
                    if let Made::PopupMenu { entries } = &mut self.made[i] {
                        entries.push(e);
                    }
                    Value::Null
                }
                // addSubMenu(menu, text)
                "addsubmenu" => {
                    let menu = match arg(args, 0).as_object() {
                        Some(Handle::Made(m))
                            if matches!(self.made.get(m), Some(Made::PopupMenu { .. })) =>
                        {
                            Some(m)
                        }
                        _ => None,
                    };
                    let e = MenuEntry::Submenu {
                        text: arg(args, 1).as_string().to_string(),
                        menu,
                    };
                    if let Made::PopupMenu { entries } = &mut self.made[i] {
                        entries.push(e);
                    }
                    Value::Null
                }
                "addseparator" => {
                    if let Made::PopupMenu { entries } = &mut self.made[i] {
                        entries.push(MenuEntry::Separator);
                    }
                    Value::Null
                }
                "checkcommand" | "disablecommand" => {
                    let id = arg(args, 0).as_int();
                    let on = arg(args, 1).truthy();
                    if let Made::PopupMenu { entries } = &mut self.made[i] {
                        if let Some(MenuEntry::Command {
                            checked, disabled, ..
                        }) = entries
                            .iter_mut()
                            .find(|e| matches!(e, MenuEntry::Command { id: cid, .. } if *cid == id))
                        {
                            if name == "checkcommand" {
                                *checked = on;
                            } else {
                                *disabled = on;
                            }
                        }
                    }
                    Value::Null
                }
                "popatmouse" | "popatxy" => {
                    let menu = self.popup_to_menu(i);
                    Value::Int(self.menu_choice(&menu))
                }
                "getnumcommands" => match &self.made[i] {
                    Made::PopupMenu { entries } => Value::Int(entries.len() as i32),
                    _ => Value::Int(0),
                },
                _ => return None,
            },
            Made::Map { bitmap } => {
                // ARGB at a point of the loaded bitmap element, or none outside it
                let pixel = |rt: &mut SkinRuntime, x: i32, y: i32| -> Option<u32> {
                    let def = rt.bitmap(bitmap.as_deref()?)?;
                    let (w, h) = rt.bitmap_size(&def.id)?;
                    if x < 0 || x >= w || y < 0 || y >= h {
                        return None;
                    }
                    rt.host.pixel(&def.file, def.x + x, def.y + y)
                };
                match name {
                    "getclassname" => Value::str("Map"),
                    "getid" => Value::str(""),
                    "loadmap" => {
                        let b = arg(args, 0).as_string().to_string();
                        if let Made::Map { bitmap } = &mut self.made[i] {
                            *bitmap = Some(b);
                        }
                        Value::Null
                    }
                    // a map is a greyscale picture; its value at a point is that grey
                    "getvalue" => Value::Int(
                        pixel(self, arg(args, 0).as_int(), arg(args, 1).as_int())
                            .map(|p| ((p >> 16) & 0xff) as i32)
                            .unwrap_or(0),
                    ),
                    "getargbvalue" => Value::Int(
                        pixel(self, arg(args, 0).as_int(), arg(args, 1).as_int())
                            .map(|p| match arg(args, 2).as_int() {
                                0 => (p & 0xff) as i32,
                                1 => ((p >> 8) & 0xff) as i32,
                                2 => ((p >> 16) & 0xff) as i32,
                                _ => ((p >> 24) & 0xff) as i32,
                            })
                            .unwrap_or(0),
                    ),
                    "inregion" => Value::Bool(
                        pixel(self, arg(args, 0).as_int(), arg(args, 1).as_int())
                            .is_some_and(|p| (p >> 24) > 0),
                    ),
                    "getwidth" => Value::Int(
                        bitmap
                            .as_deref()
                            .and_then(|b| self.bitmap_size(b))
                            .map(|s| s.0)
                            .unwrap_or(0),
                    ),
                    "getheight" => Value::Int(
                        bitmap
                            .as_deref()
                            .and_then(|b| self.bitmap_size(b))
                            .map(|s| s.1)
                            .unwrap_or(0),
                    ),
                    "getregion" => Value::Null,
                    _ => return None,
                }
            }
            Made::List { items } => match name {
                "getclassname" => Value::str("List"),
                "getid" => Value::str(""),
                "additem" => {
                    if items.len() < MAX_LIST_ITEMS {
                        if let Made::List { items } = &mut self.made[i] {
                            items.push(arg(args, 0));
                        }
                        Value::Null
                    } else {
                        self.script_error = Some(format!("list longer than {MAX_LIST_ITEMS}"));
                        Value::Null
                    }
                }
                "removeitem" => {
                    let at = arg(args, 0).as_int();
                    if let Made::List { items } = &mut self.made[i] {
                        if at >= 0 && (at as usize) < items.len() {
                            items.remove(at as usize);
                        }
                    }
                    Value::Null
                }
                "enumitem" => usize::try_from(arg(args, 0).as_int())
                    .ok()
                    .and_then(|k| items.get(k).cloned())
                    .unwrap_or(Value::Null),
                "finditem" => {
                    let wanted = arg(args, 0).as_string().to_lowercase();
                    Value::Int(
                        items
                            .iter()
                            .position(|v| v.as_string().to_lowercase() == wanted)
                            .map(|p| p as i32)
                            .unwrap_or(-1),
                    )
                }
                "finditem2" => {
                    let wanted = arg(args, 0).as_object();
                    Value::Int(
                        items
                            .iter()
                            .position(|v| v.as_object() == wanted)
                            .map(|p| p as i32)
                            .unwrap_or(-1),
                    )
                }
                "getnumitems" => Value::Int(items.len() as i32),
                "removeall" => {
                    if let Made::List { items } = &mut self.made[i] {
                        items.clear();
                    }
                    Value::Null
                }
                _ => return None,
            },
            Made::Config => match name {
                "getclassname" => Value::str("Config"),
                "getid" => Value::str(""),
                // an item asked for before any script made it is made now; scripts load in any order
                "getitem" | "getitembyguid" => {
                    let k = arg(args, 0).as_string();
                    Value::of(self.config_item(&k, Some(&k)).map(Handle::Made))
                }
                "newitem" => {
                    let k = arg(args, 0).as_string();
                    let g = arg(args, 1).as_string();
                    Value::of(self.config_item(&k, Some(&g)).map(Handle::Made))
                }
                _ => return None,
            },
            Made::ConfigItem {
                name: item_name,
                guid,
                attributes,
            } => match name {
                "getclassname" => Value::str("ConfigItem"),
                "getid" => Value::str(""),
                // Winamp's own items carry attributes no script made; an unknown one reads empty
                "getattribute" => {
                    let k = arg(args, 0).as_string().to_lowercase();
                    if let Some((_, a)) = attributes.iter().find(|(n, _)| *n == k) {
                        Value::Obj(Handle::Made(*a))
                    } else {
                        self.config_changed();
                        let a = self.new_attribute(i, &arg(args, 0).as_string(), "");
                        if let Made::ConfigItem { attributes, .. } = &mut self.made[i] {
                            attributes.push((k, a));
                        }
                        Value::Obj(Handle::Made(a))
                    }
                }
                // one only read so far takes the default and moves to where the script makes it
                "newattribute" => {
                    let k = arg(args, 0).as_string().to_lowercase();
                    let default = arg(args, 1).as_string().to_string();
                    let existing = attributes.iter().find(|(n, _)| *n == k).map(|(_, a)| *a);
                    let a = match existing {
                        Some(a) => a,
                        None => self.new_attribute(i, &arg(args, 0).as_string(), &default),
                    };
                    let made = matches!(self.made[a], Made::ConfigAttribute { made: true, .. });
                    if !made {
                        if let Made::ConfigItem { attributes, .. } = &mut self.made[i] {
                            attributes.retain(|(n, _)| *n != k);
                            attributes.push((k, a));
                        }
                        // a stored value still wins over the default
                        let stored = self
                            .stored_settings
                            .get(&arg(args, 0).as_string().to_string())
                            .cloned();
                        if let Made::ConfigAttribute { made, data, .. } = &mut self.made[a] {
                            *made = true;
                            *data = stored.unwrap_or(default);
                        }
                        self.config_changed();
                    }
                    Value::Obj(Handle::Made(a))
                }
                "getguid" => Value::string(guid.unwrap_or_default()),
                "getname" => Value::string(item_name),
                _ => return None,
            },
            Made::ConfigAttribute {
                item,
                name: attr_name,
                data,
                ..
            } => match name {
                "getclassname" => Value::str("ConfigAttribute"),
                "getid" => Value::str(""),
                "getdata" => Value::string(data),
                "setdata" => {
                    let v = arg(args, 0).as_string().to_string();
                    self.attribute_set_data(i, &v);
                    Value::Null
                }
                "getparentitem" => Value::Obj(Handle::Made(item)),
                "getattributename" => Value::string(attr_name),
                _ => return None,
            },
            Made::WinampConfig { groups } => match name {
                "getclassname" => Value::str("WinampConfig"),
                "getid" => Value::str(""),
                "getgroup" => {
                    let k = arg(args, 0).as_string().to_lowercase();
                    if let Some((_, g)) = groups.iter().find(|(n, _)| *n == k) {
                        return Some(Value::Obj(Handle::Made(*g)));
                    }
                    let Handle::Made(g) = self.push_made(Made::WinampConfigGroup {
                        values: HashMap::new(),
                    }) else {
                        return None;
                    };
                    if let Made::WinampConfig { groups } = &mut self.made[i] {
                        groups.push((k, g));
                    }
                    Value::Obj(Handle::Made(g))
                }
                _ => return None,
            },
            Made::WinampConfigGroup { values } => match name {
                "getclassname" => Value::str("WinampConfigGroup"),
                "getid" => Value::str(""),
                "getbool" => Value::Bool(
                    values
                        .get(&arg(args, 0).as_string().to_lowercase())
                        .is_some_and(|v| v != "0" && !v.is_empty()),
                ),
                "getint" => Value::Int(
                    values
                        .get(&arg(args, 0).as_string().to_lowercase())
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0),
                ),
                "getstring" => Value::string(
                    values
                        .get(&arg(args, 0).as_string().to_lowercase())
                        .cloned()
                        .unwrap_or_default(),
                ),
                "setbool" | "setint" | "setstring" => {
                    let k = arg(args, 0).as_string().to_lowercase();
                    let v = args
                        .get(1)
                        .map(|a| a.as_string().to_string())
                        .unwrap_or_default();
                    if let Made::WinampConfigGroup { values } = &mut self.made[i] {
                        values.insert(k, v);
                    }
                    Value::Null
                }
                _ => return None,
            },
            Made::Application => match name {
                "getclassname" => Value::str("Application"),
                "getapplicationname" => Value::str("Winamp"),
                "getversionstring" => Value::str("Winamp 5.666"),
                "getversionnumberstring" => Value::str("5.666"),
                "getbuildnumber" => Value::Int(BUILD),
                "getguid" => Value::str(""),
                "getcommandline" | "getapplicationpath" | "getsettingspath" | "getworkingpath" => {
                    Value::str("")
                }
                "isshuttingdown" => Value::Bool(false),
                "shutdown" | "cancelshutdown" | "setworkingpath" => Value::Null,
                _ => return None,
            },
            Made::Stub {
                name: class_name,
                data,
            } => match name {
                "getclassname" => Value::string(class_name),
                "getid" => Value::str(""),
                "setdata" => {
                    let v = arg(args, 0).as_string().to_string();
                    if let Made::Stub { data, .. } = &mut self.made[i] {
                        data.insert("data".into(), v);
                    }
                    Value::Null
                }
                // an unset config attribute reads as off
                "getdata" => Value::string(data.get("data").cloned().unwrap_or_else(|| "0".into())),
                _ => Value::Null,
            },
        })
    }

    /// A new attribute `name` of item `item` with `default`; a stored value wins over the default.
    fn new_attribute(&mut self, item: usize, name: &str, default: &str) -> usize {
        let data = self
            .stored_settings
            .get(name)
            .cloned()
            .unwrap_or_else(|| default.to_string());
        let Handle::Made(a) = self.push_made(Made::ConfigAttribute {
            item,
            name: name.to_string(),
            data,
            made: false,
            dispatching: 0,
        }) else {
            unreachable!()
        };
        a
    }

    /// A numeric config attribute; repeat is 0 off, 1 the playlist, -1 the song.
    pub(crate) fn config_value(&mut self, name: &str) -> i32 {
        match name.to_lowercase().as_str() {
            "repeat" => self.player.repeat() as i32,
            k => {
                if let Some(a) = self.script_attribute(name) {
                    let data = self.attribute_data(a);
                    if !data.trim().is_empty() {
                        return wtoi(&data);
                    }
                }
                match self.config_values.get(k) {
                    Some(v) => *v,
                    None => self.config_flag(name) as i32,
                }
            }
        }
    }

    pub(crate) fn set_config_value(&mut self, name: &str, value: i32) {
        match name.to_lowercase().as_str() {
            "repeat" => self.player.set_repeat(value != 0),
            k => {
                if let Some(a) = self.script_attribute(name) {
                    self.attribute_set_data(a, &value.to_string());
                    return;
                }
                self.config_values.insert(k.to_string(), value);
                self.config_flags.insert(k.to_string(), value != 0);
            }
        }
    }

    pub(crate) fn config_flag(&mut self, name: &str) -> bool {
        match name.to_lowercase().as_str() {
            "shuffle" => self.player.shuffle(),
            "repeat" => self.player.repeat(),
            "enable crossfading" | "crossfade" => self.player.crossfade(),
            "enable equalizer" | "eq" => self.player.eq_enabled(),
            k => match self.script_attribute(name) {
                Some(a) => wtoi(&self.attribute_data(a)) == 1,
                None => self.config_flags.get(k).copied().unwrap_or(false),
            },
        }
    }

    pub(crate) fn set_config_flag(&mut self, name: &str, on: bool) {
        match name.to_lowercase().as_str() {
            "shuffle" => self.player.set_shuffle(on),
            "repeat" => self.player.set_repeat(on),
            "enable crossfading" | "crossfade" => self.player.set_crossfade(on),
            "enable equalizer" | "eq" => self.player.set_eq_enabled(on),
            k => match self.script_attribute(name) {
                Some(a) => self.attribute_set_data(a, if on { "1" } else { "0" }),
                None => {
                    self.config_flags.insert(k.to_string(), on);
                }
            },
        }
    }

    /// The containers' script methods.
    fn container_maki_call(&mut self, c: usize, name: &str, args: &[Value]) -> Option<Value> {
        Some(match name {
            "getclassname" => Value::str("Container"),
            "getid" => Value::string(self.containers[c].id.clone()),
            "getname" => Value::string(self.containers[c].name.clone()),
            "getlayout" => Value::of(
                self.container_layout(c, &arg(args, 0).as_string())
                    .map(Handle::Gui),
            ),
            "getcurlayout" => Value::of(self.containers[c].current.map(Handle::Gui)),
            "getnumlayouts" => Value::Int(self.containers[c].layouts.len() as i32),
            "enumlayout" => Value::of(
                usize::try_from(arg(args, 0).as_int())
                    .ok()
                    .and_then(|i| self.containers[c].layouts.get(i).copied())
                    .map(Handle::Gui),
            ),
            "switchtolayout" => {
                let id = arg(args, 0).as_string();
                self.switch_to(c, &id);
                Value::Null
            }
            // the same open and close as a button's
            "show" => {
                self.show_container(c, true);
                Value::Null
            }
            "hide" | "close" => {
                self.show_container(c, false);
                Value::Null
            }
            "toggle" => {
                let open = self.is_open(c);
                self.show_container(c, !open);
                Value::Null
            }
            "isvisible" => Value::Bool(self.is_open(c)),
            // moving the container moves the layout it shows
            "setxmlparam" => {
                if let Some(l) = self.containers[c].current {
                    let (vx, vy) = self.objects[l]
                        .layout()
                        .map(|d| (d.view_x, d.view_y))
                        .unwrap_or((0, 0));
                    let (w, h) = self.layout_size(l);
                    match arg(args, 0).as_string().to_lowercase().as_str() {
                        "x" => self.layout_move_to(l, arg(args, 1).as_int(), vy, w, h),
                        "y" => self.layout_move_to(l, vx, arg(args, 1).as_int(), w, h),
                        _ => {}
                    }
                }
                Value::Null
            }
            "getxmlparam" => Value::string(
                self.containers[c]
                    .attr(&arg(args, 0).as_string().to_lowercase())
                    .unwrap_or("")
                    .to_string(),
            ),
            "setname" => Value::Null,
            "isdynamic" => Value::Bool(false),
            "getguid" => Value::string(
                self.containers[c]
                    .attr("component")
                    .unwrap_or("")
                    .to_string(),
            ),
            "setlocation" => {
                if let Some(l) = self.containers[c].current {
                    let (w, h) = self.layout_size(l);
                    self.layout_move_to(l, arg(args, 0).as_int(), arg(args, 1).as_int(), w, h);
                }
                Value::Null
            }
            "centerlayouts" => Value::Null,
            _ => return None,
        })
    }
}

/// `name` without a note in brackets at its end ("animate (disabled if ...)" to "animate").
fn without_note(name: &str) -> String {
    let t = name.trim_end();
    if let Some(open) = t.rfind('(') {
        if t.ends_with(')') && !t[open + 1..t.len() - 1].contains(['(', ')']) {
            return t[..open].trim_end().to_string();
        }
    }
    name.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guids_and_numbers_read_as_winamp_reads_them() {
        assert!(is_guid("{1828D28F-78DD-4647-8532-EBA504B8FC04}"));
        assert!(is_guid("1828d28f-78dd-4647-8532-eba504b8fc04"));
        assert!(!is_guid("hello"));
        assert_eq!(wtoi(" -12abc"), -12);
        assert_eq!(wtoi("x"), 0);
        assert_eq!(shown_text("&Open\tCtrl+O"), "Open");
        assert_eq!(shown_text("A && B"), "A & B");
        assert_eq!(without_note("animate (disabled if x)"), "animate");
        assert_eq!(without_note("plain"), "plain");
    }
}
