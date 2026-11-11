// SPDX-License-Identifier: GPL-3.0-or-later

//! Loading a skin: its XML with every include expanded, the definitions it collects, and the
//! objects it builds from containers, layouts and groupdefs.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use super::{SkinRuntime, MAX_INCLUDE_DEPTH, MAX_NODES, MAX_OBJECTS, MAX_XML_DEPTH, PLAYLIST_GUID};
use crate::draw::{BitmapDef, BitmapFontDef, FontDef, Gamma};
use crate::files::BASE_PREFIX;
use crate::gui::{
    to_int_lenient, AnimData, Behavior, BucketData, ButtonData, ButtonKind, EqVisData, FrameData,
    GradientData, GridData, ImagesData, Kind, LayerData, LayoutData, LayoutStatusData, MenuData,
    Obj, ObjId, PlaylistData, ProgressData, RectData, SliderData, StatusData, ThemeListData,
    XFadeData,
};
use crate::maki::{Handle, SystemObject};
use crate::text::TextData;
use crate::xml::{self, XmlNode};

/// Everything a groupdef holds, and its attributes that are no params of the group.
const ALL_CONTENT: [&str; 3] = ["params", "xui", "scripts"];
const DEF_ATTRS: [&str; 7] = [
    "id",
    "xuitag",
    "inherit_group",
    "inherit_content",
    "inherit_params",
    "behavior",
    "contentrect",
];
const FILE_NODES: [&str; 4] = ["bitmap", "bitmapfont", "truetypefont", "script"];
const GUI_ATTRS: [&str; 20] = [
    "id",
    "x",
    "y",
    "w",
    "h",
    "relatx",
    "relaty",
    "relatw",
    "relath",
    "visible",
    "alpha",
    "ghost",
    "sysregion",
    "tooltip",
    "fitparent",
    "background",
    "drawbackground",
    "instance_id",
    "instanceid",
    "name",
];
const KNOWN_CLASSES: [(&str, &str); 18] = [
    ("grid", "GuiObject"),
    ("progressgrid", "GuiObject"),
    ("rect", "GuiObject"),
    ("albumart", "AlbumArtLayer"),
    ("layoutstatus", "LayoutStatus"),
    ("menu", "Menu"),
    ("browser", "Browser"),
    ("edit", "Edit"),
    ("checkbox", "CheckBox"),
    ("dropdownlist", "DropDownList"),
    ("guilist", "GuiList"),
    ("guitree", "GuiTree"),
    ("tabsheet", "TabSheet"),
    ("wasabi:tabsheet", "TabSheet"),
    ("playlisteditor", "GuiObject"),
    ("wasabi:titlebar", "GuiObject"),
    ("mouseredir", "MouseRedir"),
    ("images", "GuiObject"),
];

/// `<sendparams>`, `<hideobject>` and `<showobject>` met while a group fills, with how many
/// children the group had then.
type Meta = Vec<(Rc<XmlNode>, usize)>;

fn node_key(n: &Rc<XmlNode>) -> usize {
    Rc::as_ptr(n) as usize
}

fn dir_of(path: &str) -> String {
    path.rsplit_once('/')
        .map(|(d, _)| d.to_string())
        .unwrap_or_default()
}

fn utf8(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

impl SkinRuntime {
    /// Loads `skin.xml` and everything it includes, builds the windows and starts the scripts.
    pub fn load(&mut self) {
        // Winamp's wasabi base first, when the app has it (files under wasabi-base/): a
        // skin's definitions then override it, as they do in Winamp
        if let Some(base_xml) = self.files.read(&format!("{BASE_PREFIX}wasabi.xml")) {
            let base_nodes = self.expand(
                xml::parse(&utf8(&base_xml)),
                BASE_PREFIX.trim_end_matches('/'),
                &[],
                0,
            );
            self.collecting_base = true;
            for node in base_nodes {
                let prefixed = prefix_files(&node, BASE_PREFIX);
                self.collect_defs(&prefixed);
            }
            self.collecting_base = false;
        }
        // a classic skin's windows are generated around its bitmaps
        if crate::classic::is_classic(&*self.files) {
            self.classic_skin = true;
            let inner =
                std::mem::replace(&mut self.files, Box::new(crate::files::MemoryFiles::new()));
            self.files = Box::new(crate::classic::ClassicFiles::new(inner));
            self.classic_sheets = Rc::new(crate::classic::Sheets::of(&*self.files));
            if let Some(text) = self.files.read("region.txt") {
                self.classic_regions = crate::classic::region::parse(&utf8(&text));
            }
            if let Some(text) = self.files.read("pledit.txt") {
                self.classic_pledit = crate::classic::pledit::parse(&utf8(&text));
            }
            if let Some(text) = self.files.read("viscolor.txt") {
                self.classic_palette = crate::classic::viscolor::parse(&utf8(&text));
            }
        }
        let root_text = self
            .files
            .read("skin.xml")
            .map(|b| utf8(&b))
            .expect("no skin.xml");
        let root = xml::parse(&root_text);
        if let Some(v) = root
            .iter()
            .find(|n| n.name == "winampabstractionlayer" || n.name == "wasabixml")
            .and_then(|n| n.get("version"))
            .and_then(|v| crate::maki::value::parse_double(v.trim()))
        {
            self.wal_version = v;
        }
        let expanded = self.expand(root, "", &[], 0);
        for node in &expanded {
            self.collect_defs(node);
        }
        for node in &expanded {
            self.build_top(node, None);
        }
        self.do_layout();
        // the main window where the phone draws it, before a script reads its place
        for c in 0..self.containers.len() {
            self.fit_viewport(c);
        }
        self.scripts_started = true;
        for s in 0..self.systems.len() {
            self.start_script(s);
        }
        for c in 0..self.containers.len() {
            if let Some(l) = self.containers[c].current {
                self.dispatch(crate::maki::Handle::Gui(l), "onstartup", &[]);
            }
        }
        // a bucket's script picks its first entry from here (Winamp Modern's options menu)
        self.start_buckets();
        // the layouts on screen hear that they are, once every script runs
        for c in 0..self.containers.len() {
            if self.containers[c].visible {
                if let Some(l) = self.containers[c].current {
                    self.announce_visible(l, true);
                }
            }
        }
        self.ensure_layout();
        for o in 0..self.objects.len() {
            if matches!(self.objects[o].kind, Kind::AnimatedLayer(..)) {
                self.anim_start_if_auto(o);
            }
        }
        // the main window has the focus from the start, so the first press moves no focus
        if self.focused.is_none() {
            self.focused = self.main_layout();
        }
        self.relayout();
    }

    /// Winamp's placeholders in attribute values: `@HAVE_LIBRARY@` is "1" when the Media
    /// Library is there, `@SKINSPATH@` the skins folder (resolved by the files).
    fn substituted(&self, attrs: &[(String, String)]) -> Vec<(String, String)> {
        if !attrs.iter().any(|(_, v)| v.contains("@HAVE_")) {
            return attrs.to_vec();
        }
        attrs
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    v.replace("@HAVE_LIBRARY@", if self.have_library { "1" } else { "0" })
                        .replace("@HAVE_VIDEO@", "1"),
                )
            })
            .collect()
    }

    /// Replaces every `<include>` by the nodes of the file it names, relative to the including
    /// file. A file that includes itself (directly or round about), includes nested past the
    /// limit and anything past the node limit are left out.
    fn expand(
        &mut self,
        nodes: Vec<Rc<XmlNode>>,
        dir: &str,
        chain: &[String],
        depth: usize,
    ) -> Vec<Rc<XmlNode>> {
        let mut out = Vec::new();
        for n in nodes {
            self.expanded_nodes += 1;
            if self.expanded_nodes > MAX_NODES || depth > MAX_XML_DEPTH {
                if self.expanded_nodes == MAX_NODES + 1 {
                    self.host
                        .log(&format!("skin too large: stopped at {MAX_NODES} nodes"));
                }
                continue;
            }
            if n.name == "include" {
                let Some(file) = n.get("file") else { continue };
                let file = file.replace('\\', "/");
                let file = file
                    .strip_prefix("@DEFAULTSKINPATH@")
                    .unwrap_or(&file)
                    .to_string();
                let candidates = [
                    if dir.is_empty() {
                        file.clone()
                    } else {
                        format!("{dir}/{file}")
                    },
                    file.clone(),
                ];
                let path = candidates.iter().find(|p| self.files.exists(p)).cloned();
                match path {
                    None => self
                        .host
                        .log(&format!("include not found: {file} (from {dir})")),
                    Some(path) if chain.contains(&path) || chain.len() >= MAX_INCLUDE_DEPTH => {
                        self.host.log(&format!(
                            "include skipped: {file} (includes itself or nests too deep)"
                        ));
                    }
                    Some(path) => {
                        let text = utf8(&self.files.read(&path).expect("exists"));
                        let mut next = chain.to_vec();
                        next.push(path.clone());
                        out.extend(self.expand(
                            xml::parse(&text),
                            &dir_of(&path),
                            &next,
                            depth + 1,
                        ));
                    }
                }
            } else {
                let children = self.expand(n.children.clone(), dir, chain, depth + 1);
                out.push(Rc::new(XmlNode {
                    name: n.name.clone(),
                    attrs: self.substituted(&n.attrs),
                    children,
                    text: n.text.clone(),
                }));
            }
        }
        out
    }

    fn collect_defs(&mut self, node: &Rc<XmlNode>) {
        match node.name.as_str() {
            "bitmap" => {
                if let (Some(id), Some(given)) = (node.get("id"), node.get("file")) {
                    // a picture Winamp makes itself: a made-up name the hosts' files answer
                    let file = if given
                        .rsplit('/')
                        .next()
                        .is_some_and(|n| n.eq_ignore_ascii_case("$solid"))
                    {
                        crate::generated::solid_name(
                            node.get("w").and_then(to_int_lenient),
                            node.get("h").and_then(to_int_lenient),
                            node.get("color"),
                            node.get("alpha").and_then(to_int_lenient),
                        )
                    } else {
                        given.to_string()
                    };
                    let key = id.to_lowercase();
                    // a picture defined under an old name (studio.button.upperLeft) is the
                    // picture its alias names from then on, as a later definition of that name
                    // would be: MMD3 defines both, and Winamp draws its buttons with the old ones
                    let names: Vec<String> = std::iter::once(key.clone())
                        .chain(self.aliases.get(&key).cloned())
                        .collect();
                    for key in names {
                        if self.collecting_base {
                            self.base_bitmaps.insert(key.clone());
                        } else {
                            self.base_bitmaps.remove(&key);
                        }
                        let def = BitmapDef::new(
                            &key,
                            &file,
                            node.get("x").and_then(to_int_lenient).unwrap_or(0),
                            node.get("y").and_then(to_int_lenient).unwrap_or(0),
                            node.get("w").and_then(to_int_lenient),
                            node.get("h").and_then(to_int_lenient),
                            node.get("gammagroup").map(|g| g.to_lowercase()).as_deref(),
                        );
                        self.bitmaps.insert(key, def);
                    }
                    self.bitmap_lookups.clear();
                }
            }
            "bitmapfont" => {
                if let (Some(id), Some(file)) = (node.get("id"), node.get("file")) {
                    let key = id.to_lowercase();
                    self.bitmap_fonts.insert(
                        key.clone(),
                        BitmapFontDef {
                            id: Rc::from(key.as_str()),
                            file: Rc::from(file),
                            char_width: node.get("charwidth").and_then(to_int_lenient).unwrap_or(5),
                            char_height: node
                                .get("charheight")
                                .and_then(to_int_lenient)
                                .unwrap_or(6),
                            h_spacing: node.get("hspacing").and_then(to_int_lenient).unwrap_or(0),
                            v_spacing: node.get("vspacing").and_then(to_int_lenient).unwrap_or(0),
                            ox: 0,
                            oy: 0,
                            gamma_group: node
                                .get("gammagroup")
                                .map(|g| Rc::from(g.to_lowercase().as_str())),
                        },
                    );
                    self.resolved_fonts.remove(&key);
                }
            }
            "truetypefont" => {
                if let (Some(id), Some(file)) = (node.get("id"), node.get("file")) {
                    let key = id.to_lowercase();
                    self.fonts.insert(key.clone(), FontDef::new(&key, file));
                }
            }
            "color" => {
                if let Some(id) = node.get("id") {
                    let key = id.to_lowercase();
                    if self.collecting_base {
                        self.base_colors.insert(key.clone());
                    } else {
                        self.base_colors.remove(&key);
                    }
                    if let Some(v) = node.get("value") {
                        self.define_color(&key, v);
                    }
                    if let Some(g) = node.get("gammagroup") {
                        self.color_groups.insert(key, g.to_lowercase());
                    }
                }
            }
            "gammaset" => {
                if let Some(id) = node.get("id") {
                    let groups: HashMap<String, Gamma> = node
                        .children
                        .iter()
                        .filter(|c| c.name == "gammagroup")
                        .map(|g| {
                            let v: Vec<i32> = g
                                .get("value")
                                .unwrap_or("0,0,0")
                                .split(',')
                                .map(|p| p.trim().parse().unwrap_or(0))
                                .collect();
                            (
                                g.get("id").unwrap_or("").to_lowercase(),
                                Gamma {
                                    r: v.first().copied().unwrap_or(0),
                                    g: v.get(1).copied().unwrap_or(0),
                                    b: v.get(2).copied().unwrap_or(0),
                                    boost: g.get("boost").and_then(to_int_lenient).unwrap_or(0),
                                    gray: g.get("gray").and_then(to_int_lenient).unwrap_or(0),
                                },
                            )
                        })
                        .collect();
                    match self.gamma_sets.iter_mut().find(|(n, _)| n == id) {
                        Some(entry) => entry.1 = groups,
                        None => self.gamma_sets.push((id.to_string(), groups)),
                    }
                    if self.current_gamma_set.is_none() {
                        self.current_gamma_set = Some(id.to_string());
                    }
                    self.gammas.clear();
                }
            }
            "elementalias" => {
                if let (Some(id), Some(target)) = (node.get("id"), node.get("target")) {
                    self.aliases
                        .insert(id.to_lowercase(), target.to_lowercase());
                    self.bitmap_lookups.clear();
                }
            }
            "groupdef" => {
                if let Some(id) = node.get("id") {
                    let key = id.to_lowercase();
                    if let Some(earlier) = self.group_defs.insert(key.clone(), node.clone()) {
                        self.earlier_defs.insert(node_key(node), earlier);
                    }
                    if self.collecting_base {
                        self.base_group_defs.insert(node_key(node));
                    }
                    if let Some(tag) = node.get("xuitag") {
                        self.xui_tags.insert(tag.to_lowercase(), key.clone());
                    }
                    if let Some(wt) = node.get("windowtype") {
                        let list = self
                            .window_types
                            .entry(wt.trim().to_lowercase())
                            .or_default();
                        if !list.contains(&key) {
                            list.push(key.clone());
                        }
                    }
                }
            }
            // getSkinName() is the skin's folder name, which the host sets; <skininfo> names it only when it did not
            "skininfo" if self.skin_name.is_empty() => {
                if let Some(n) = node.children.iter().find(|c| c.name == "name") {
                    self.skin_name = if n.text.is_empty() {
                        n.get("text").unwrap_or(&self.skin_name).to_string()
                    } else {
                        n.text.clone()
                    };
                }
            }
            _ => {}
        }
        for c in &node.children {
            self.collect_defs(c);
        }
    }

    /// A `<color>`'s value under the lowercase id `k`. A value that is no RGB names another
    /// color; the later definition wins in either form.
    fn define_color(&mut self, k: &str, v: &str) {
        if let Some(rgb) = crate::gui::parse_rgb(v) {
            self.colors.insert(k.to_string(), rgb);
            self.color_aliases.remove(k);
        } else {
            self.color_aliases.insert(k.to_string(), v.to_lowercase());
            self.colors.remove(k);
        }
    }

    fn build_top(&mut self, node: &Rc<XmlNode>, parent: Option<ObjId>) {
        match node.name.as_str() {
            "wasabixml"
            | "winampabstractionlayer"
            | "elements"
            | "scripts"
            | "skininfo"
            | "gammaset"
            | "accelerators" => {
                let walk_into = node.name == "scripts"
                    || !["elements", "skininfo", "gammaset", "accelerators"]
                        .contains(&node.name.as_str());
                if walk_into {
                    for c in &node.children {
                        self.build_top(c, parent);
                    }
                }
            }
            "container" => self.build_container(node),
            "script" => self.attach_script(node, parent),
            "groupdef" | "bitmap" | "bitmapfont" | "truetypefont" | "color" | "elementalias"
            | "colorthemes:list" | "colorthemes:mgr" => {}
            _ => self.host.log(&format!("top-level <{}> ignored", node.name)),
        }
    }

    fn build_container(&mut self, node: &Rc<XmlNode>) {
        let Some(id) = node.get("id").map(|s| s.to_lowercase()) else {
            return;
        };
        let c = self.containers.len();
        self.containers
            .push(crate::gui::Container::new(&id, node.attrs.clone()));
        for child in &node.children {
            match child.name.as_str() {
                "layout" => {
                    let layout = self.new_layout(c);
                    self.fill_group(layout, child, child.get("id"));
                    self.containers[c].layouts.push(layout);
                }
                "script" => self.attach_script(child, None),
                "scripts" => {
                    for s in &child.children {
                        self.attach_script(s, None);
                    }
                }
                "groupdef" => {}
                _ => self
                    .host
                    .log(&format!("<{}> in container {id} ignored", child.name)),
            }
        }
        let layouts = self.containers[c].layouts.clone();
        self.containers[c].current = layouts
            .iter()
            .copied()
            .find(|l| self.objects[*l].base.id == "normal")
            .or(layouts.first().copied());
        // a new main window, for what is worked out from the windows
        self.change_count += 1;
    }

    /// A new layout of container `c`, placed at the container's default position.
    fn new_layout(&mut self, c: usize) -> ObjId {
        let window_id = self.new_window_id();
        let mut data = LayoutData::new(window_id);
        data.container = Some(c);
        data.view_x = self.containers[c].default_x;
        data.view_y = self.containers[c].default_y;
        let id = self.objects.len();
        self.objects
            .push(Obj::new("layout", Kind::Layout(Box::new(data)), true));
        self.register_layout(id);
        id
    }

    /// Applies a groupdef, its inherited chain first, then the node's attributes and children.
    pub(crate) fn fill_group(&mut self, group: ObjId, node: &Rc<XmlNode>, def_id: Option<&str>) {
        let mut meta: Meta = Vec::new();
        if let Some(def_id) = def_id {
            if let Some(def) = self.group_defs.get(&def_id.to_lowercase()).cloned() {
                self.apply_group_def(group, &def, &ALL_CONTENT, &mut meta, &mut HashSet::new());
            }
        }
        self.set_params(group, &node.attrs);
        for c in &node.children {
            self.build_child(c, group, &mut meta);
        }
        self.apply_meta(group, &meta);
    }

    /// Builds `def` into `group`, only the `want`ed parts ("params", "xui" objects, "scripts").
    /// The base comes first: `inherit_group`, else the groupdef this one redefined. What of the
    /// base is used is `inherit_content` ("1" all, "0" none, or a list; objects and scripts by
    /// default with an `inherit_group`, nothing without), plus its params unless `inherit_params="0"`.
    fn apply_group_def(
        &mut self,
        group: ObjId,
        def: &Rc<XmlNode>,
        want: &[&str],
        meta: &mut Meta,
        seen: &mut HashSet<usize>,
    ) {
        if !seen.insert(node_key(def)) {
            return;
        }
        if self.base_group_defs.contains(&node_key(def)) {
            self.base_usage.insert(format!(
                "group:{}",
                def.get("id").unwrap_or("").to_lowercase()
            ));
        }
        if let Some(b) = behavior_of(def) {
            if let Some(g) = self.objects[group].group.as_mut() {
                g.behavior = Some(b);
            }
        }
        let own_id = def.get("id").map(|s| s.to_lowercase());
        let named = def
            .get("inherit_group")
            .map(|s| s.to_lowercase())
            .filter(|n| Some(n) != own_id.as_ref());
        let base = named
            .as_ref()
            .and_then(|n| self.group_defs.get(n).cloned())
            .or_else(|| self.earlier_defs.get(&node_key(def)).cloned());
        if let Some(base) = base {
            let content: Vec<String> =
                match def.get("inherit_content").map(|c| c.trim().to_lowercase()) {
                    None => {
                        if named.is_some() {
                            vec!["xui".into(), "scripts".into()]
                        } else {
                            vec![]
                        }
                    }
                    Some(c) if c == "1" => vec!["xui".into(), "scripts".into()],
                    Some(c) if c == "0" => vec![],
                    Some(c) => c.split([';', ',']).map(|s| s.trim().to_string()).collect(),
                };
            let mut inherited = content;
            if def.get("inherit_params").map(|p| p.trim()) != Some("0") {
                inherited.push("params".into());
            }
            let wanted: Vec<&str> = want
                .iter()
                .copied()
                .filter(|w| inherited.iter().any(|i| i == w))
                .collect();
            self.apply_group_def(group, &base, &wanted, meta, seen);
        }
        if want.contains(&"params") {
            for (k, v) in &def.attrs {
                if !DEF_ATTRS.contains(&k.as_str()) {
                    self.set_param(group, k, v);
                }
            }
        }
        for c in &def.children {
            let part = if c.name == "script" { "scripts" } else { "xui" };
            if want.contains(&part) {
                self.build_child(c, group, meta);
            }
        }
    }

    fn new_object(&mut self, tag: &str, kind: Kind) -> ObjId {
        let grouped = super::is_grouped(&kind);
        let id = self.objects.len();
        self.objects.push(Obj::new(tag, kind, grouped));
        id
    }

    fn build_child(&mut self, node: &Rc<XmlNode>, parent: ObjId, meta: &mut Meta) {
        self.built += 1;
        if self.built > MAX_OBJECTS {
            if self.built == MAX_OBJECTS + 1 {
                self.host
                    .log(&format!("skin too large: stopped at {MAX_OBJECTS} objects"));
            }
            return;
        }
        let name = node.name.as_str();
        // Wasabi's tags (Wasabi:Button, Wasabi:Text, ...) are xui groups of the wasabi base when it
        // defines them: a framed button inside a group, and so on
        if name.starts_with("wasabi:") {
            if let Some(def_id) = self.xui_tags.get(name).cloned() {
                self.build_xui(node, parent, &def_id);
                return;
            }
        }
        let kind = match name {
            "layer" => Kind::Layer(LayerData::default()),
            "animatedlayer" => Kind::AnimatedLayer(LayerData::default(), AnimData::default()),
            "button" | "wasabi:button" => Kind::Button(ButtonData::new(ButtonKind::Plain)),
            "nstatesbutton" => Kind::Button(ButtonData::new(ButtonKind::NStates {
                states: 2,
                values: None,
                cfg_attrib: None,
                local_state: 0,
                auto_elements: true,
            })),
            "togglebutton" => Kind::Button(ButtonData::new(ButtonKind::Toggle {
                cfg_attrib: None,
                cfg_val: 1,
                auto_toggle: true,
            })),
            "text" | "wasabi:text" => Kind::Text(Box::default()),
            // a songticker is a text that shows the song name and its length unless told otherwise
            "songticker" => Kind::Text(Box::new(TextData::song_ticker())),
            "slider" | "wasabi:hslider" | "wasabi:vslider" => Kind::Slider(SliderData::default()),
            "status" => Kind::Status(StatusData::default()),
            "grid" => Kind::Grid(GridData::default()),
            "progressgrid" => Kind::ProgressGrid(GridData::default(), ProgressData::default()),
            "albumart" => Kind::AlbumArt { not_found: None },
            "rect" => Kind::RectObj(RectData::default()),
            "images" => Kind::Images(ImagesData::default()),
            "menu" => Kind::Menu(MenuData::default()),
            // Winamp's playlist editor, which a skin embeds as a component, or a window
            // holder for it (Bento): the playlist view either way
            "component" | "windowholder" => {
                if Self::holds(&node.attrs, PLAYLIST_GUID) {
                    Kind::PlaylistView(PlaylistData::default())
                } else {
                    let g = self.new_object(name, Kind::Group);
                    self.fill_group(g, node, node.get("id"));
                    self.add_child(parent, g);
                    return;
                }
            }
            "wasabi:frame" | "frame" => {
                let f = self.new_object(name, Kind::Frame(FrameData::default()));
                self.fill_group(f, node, None);
                self.add_child(parent, f);
                return;
            }
            "componentbucket" => {
                let b = self.new_object(name, Kind::Bucket(BucketData::default()));
                self.fill_group(b, node, node.get("id"));
                self.bucket_fill(b);
                self.add_child(parent, b);
                self.starting_buckets.push(b);
                return;
            }
            "groupxfade" => {
                let g = self.new_object(name, Kind::XFade(XFadeData::default()));
                self.fill_group(g, node, node.get("id"));
                self.add_child(parent, g);
                return;
            }
            "group" | "cfggroup" => {
                let g = self.new_object(name, Kind::Group);
                self.fill_group(g, node, node.get("id"));
                self.add_child(parent, g);
                return;
            }
            "vis" => Kind::Vis(Box::default()),
            "classicwindow" => Kind::ClassicWindow(crate::classic::ClassicWindow::Main),
            "eqvis" => Kind::EqVis(EqVisData::default()),
            "colorthemes:list" => Kind::ColorThemeList(ThemeListData::default()),
            "gradient" => Kind::Gradient(GradientData::default()),
            "layoutstatus" => {
                let o = self.new_object(name, Kind::LayoutStatus(LayoutStatusData::default()));
                self.set_params(o, &node.attrs);
                self.status_field(o);
                self.add_child(parent, o);
                return;
            }
            "layout" => {
                let pid = self.objects[parent].base.id.clone();
                self.host.log(&format!("nested <layout> in {pid} ignored"));
                return;
            }
            "script" => {
                self.attach_script(node, Some(parent));
                return;
            }
            "scripts" => {
                for c in &node.children {
                    self.attach_script(c, Some(parent));
                }
                return;
            }
            "sendparams" | "hideobject" | "showobject" => {
                // with how many children the group had then: it reaches only objects made before it
                meta.push((node.clone(), self.objects[parent].children().len()));
                return;
            }
            "bitmap" | "bitmapfont" | "color" | "truetypefont" | "elementalias" | "elements"
            | "groupdef" | "gammaset" | "snappoint" | "accelerators" | "include" => return,
            _ => {
                if let Some(def_id) = self.xui_tags.get(name).cloned() {
                    self.build_xui_with_content(node, parent, &def_id);
                    return;
                }
                let class_name = KNOWN_CLASSES
                    .iter()
                    .find(|(t, _)| *t == name)
                    .map(|(_, c)| *c)
                    .unwrap_or("GuiObject");
                if name.contains(':') || !KNOWN_CLASSES.iter().any(|(t, _)| *t == name) {
                    self.host
                        .log(&format!("<{name}> unknown, placed as {class_name}"));
                }
                Kind::Unknown {
                    class_name: class_name.to_string(),
                }
            }
        };
        let o = self.new_object(name, kind);
        self.set_params(o, &node.attrs);
        self.add_child(parent, o);
    }

    /// A xui element with the group its `content` attribute names inside it, when no script of
    /// the element runs: the standard frame scripts build the content group themselves, but
    /// only when they run.
    fn build_xui_with_content(&mut self, node: &Rc<XmlNode>, parent: ObjId, def_id: &str) {
        let g = self.build_xui(node, parent, def_id);
        let Some(content) = node.get("content") else {
            return;
        };
        if !self.objects[g]
            .group
            .as_ref()
            .expect("a group")
            .system_objects
            .is_empty()
        {
            return;
        }
        let inner = self.new_object("group", Kind::Group);
        let spec = Rc::new(XmlNode {
            name: "group".into(),
            attrs: vec![
                ("id".into(), content.to_string()),
                ("x".into(), "0".into()),
                ("y".into(), "0".into()),
                ("w".into(), "0".into()),
                ("h".into(), "0".into()),
                ("relatw".into(), "1".into()),
                ("relath".into(), "1".into()),
            ],
            children: Vec::new(),
            text: String::new(),
        });
        self.fill_group(inner, &spec, Some(content));
        self.add_child(g, inner);
    }

    /// A xui element: a group made from the definition that names `node`'s tag. Its GUI
    /// attributes place the group; the others go to its scripts as xui parameters and to the
    /// child the group embeds.
    fn build_xui(&mut self, node: &Rc<XmlNode>, parent: ObjId, def_id: &str) -> ObjId {
        let g = self.new_object(&node.name, Kind::Group);
        let gui: Vec<(String, String)> = node
            .attrs
            .iter()
            .filter(|(k, _)| GUI_ATTRS.contains(&k.as_str()) || k == "move")
            .cloned()
            .collect();
        let rest: Vec<(String, String)> = node
            .attrs
            .iter()
            .filter(|(k, _)| !(GUI_ATTRS.contains(&k.as_str()) || k == "move"))
            .cloned()
            .collect();
        let spec = Rc::new(node.with_attrs(gui));
        self.fill_group(g, &spec, Some(def_id));
        for (k, v) in &rest {
            self.objects[g]
                .group
                .as_mut()
                .expect("a group")
                .xui_params
                .push((k.clone(), v.clone()));
        }
        if let Some(e) = self.embedded(g) {
            self.set_params(e, &rest);
        }
        self.add_child(parent, g);
        if !self.group_defs.contains_key(&def_id.to_lowercase()) {
            self.host
                .log(&format!("xui {}: no groupdef {def_id}", node.name));
        }
        g
    }

    /// `<hideobject>`, `<showobject>`, `<sendparams>`: applied where they stand in the XML, so
    /// they reach the objects made before them.
    fn apply_meta(&mut self, group: ObjId, meta: &Meta) {
        for (m, before) in meta {
            let Some(targets) = m.get("target") else {
                continue;
            };
            let targets: Vec<String> = targets
                .split(';')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect();
            let existing: Vec<ObjId> = self.objects[group]
                .children()
                .iter()
                .take(*before)
                .copied()
                .collect();
            let scope = m
                .get("group")
                .and_then(|g| self.find_meta_target(&existing, g))
                .filter(|o| self.objects[*o].group.is_some());
            for t in targets {
                let target = match scope {
                    Some(s) => self.find_object(s, &t),
                    None => self.find_meta_target(&existing, &t),
                };
                if let Some(target) = target {
                    self.apply_meta_to(m, target);
                }
            }
        }
    }

    /// The object `id` names among `objs`, by id or instance id, else anywhere inside one of their groups.
    fn find_meta_target(&self, objs: &[ObjId], id: &str) -> Option<ObjId> {
        let lower = id.to_lowercase();
        if let Some(o) = objs.iter().copied().find(|o| {
            let obj = &self.objects[*o];
            obj.base.id == lower
                || obj
                    .group
                    .as_ref()
                    .is_some_and(|g| g.instance_id.as_deref() == Some(lower.as_str()))
        }) {
            return Some(o);
        }
        for o in objs {
            if self.objects[*o].group.is_some() {
                if let Some(f) = self.find_object(*o, &lower) {
                    return Some(f);
                }
            }
        }
        None
    }

    fn apply_meta_to(&mut self, m: &Rc<XmlNode>, target: ObjId) {
        match m.name.as_str() {
            "hideobject" => self.hide_object(target),
            "showobject" => self.show_object(target),
            "sendparams" => {
                for (k, v) in &m.attrs {
                    if k != "target" && k != "group" {
                        self.set_param(target, k, v);
                    }
                }
            }
            _ => {}
        }
    }

    fn attach_script(&mut self, node: &Rc<XmlNode>, group: Option<ObjId>) {
        let Some(file) = node.get("file") else { return };
        if let Some(rest) = file.strip_prefix(BASE_PREFIX) {
            self.base_usage.insert(format!("script:{rest}"));
        }
        let key = file.to_lowercase();
        let program = if let Some(p) = self.programs.get(&key) {
            p.clone()
        } else {
            let short = format!("scripts/{}", file.rsplit('/').next().unwrap_or(file));
            let bytes = self.files.read(file).or_else(|| self.files.read(&short));
            let program = match bytes {
                None => {
                    self.host.log(&format!("script not found: {file}"));
                    None
                }
                // the parser reports any damaged file as an error
                Some(bytes) => match crate::maki::parser::parse(&bytes, file) {
                    Ok(p) => Some(Rc::new(p)),
                    Err(e) => {
                        self.host.log(&format!("script {file}: {e}"));
                        None
                    }
                },
            };
            self.programs.insert(key, program.clone());
            program
        };
        let Some(program) = program else { return };
        let s = self.systems.len();
        self.systems.push(SystemObject {
            group,
            param: node.get("param").unwrap_or("").to_string(),
            script_id: file.to_string(),
            started: false,
        });
        let script = crate::maki::Script::new(program, Handle::System(s), |guid| self.create(guid));
        self.scripts.push(script);
        if let Some(g) = group {
            if let Some(gd) = self.objects[g].group.as_mut() {
                gd.system_objects.push(s);
            }
        }
        // a script in a group made later starts once that group has been laid out
        if self.scripts_started {
            self.pending_starts.push(s);
            self.relayout();
        }
    }

    /// `System.newGroup(id)`: a fresh instance of a groupdef, not yet in any parent.
    pub(crate) fn instantiate_group(&mut self, id: &str) -> Option<ObjId> {
        if !self.group_defs.contains_key(&id.to_lowercase()) {
            return None;
        }
        let g = self.new_object("group", Kind::Group);
        let spec = Rc::new(XmlNode {
            name: "group".into(),
            attrs: vec![("id".into(), id.to_string())],
            children: Vec::new(),
            text: String::new(),
        });
        self.fill_group(g, &spec, Some(id));
        Some(g)
    }

    /// A status bar's field between its buttons: the skin's `wasabi.statusbar` group, else
    /// what Winamp's own status bar group draws, the object frame; none with `bg="0"`.
    fn status_field(&mut self, o: ObjId) {
        if matches!(&self.objects[o].kind, Kind::LayoutStatus(d) if !d.background) {
            return;
        }
        let field = self
            .instantiate_group("wasabi.statusbar")
            .or_else(|| self.instantiate_group("wasabi.objectframe.group"));
        if let Some(g) = field {
            self.add_child(o, g);
        }
    }

    /// Scripts that have yet to start, started after a layout pass.
    pub(crate) fn take_pending_starts(&mut self) -> Vec<usize> {
        std::mem::take(&mut self.pending_starts)
    }
}

/// What a groupdef asks for with `behavior="..."`.
fn behavior_of(def: &XmlNode) -> Option<Behavior> {
    match def.get("behavior").map(|b| b.to_lowercase()).as_deref() {
        Some("standardframe") => {
            let area: Vec<i32> = def
                .get("contentrect")
                .map(|r| {
                    r.split(',')
                        .map(|p| p.trim().parse().unwrap_or(0))
                        .collect()
                })
                .filter(|v: &Vec<i32>| v.len() == 4)
                .unwrap_or_else(|| vec![0, 0, 0, 0]);
            Some(Behavior::StandardFrame {
                area: [area[0], area[1], area[2], area[3]],
            })
        }
        Some("titlebar") => Some(Behavior::TitleBar {
            pad_left: 0,
            pad_right: 0,
        }),
        _ => None,
    }
}

/// The same tree with every `file` attribute under `prefix`: the wasabi base's pictures live there.
fn prefix_files(node: &Rc<XmlNode>, prefix: &str) -> Rc<XmlNode> {
    let attrs = if FILE_NODES.contains(&node.name.as_str()) && node.has("file") {
        node.attrs
            .iter()
            .map(|(k, v)| {
                if k == "file" {
                    (
                        k.clone(),
                        format!("{prefix}{}", v.replace('\\', "/").trim_start_matches('/')),
                    )
                } else {
                    (k.clone(), v.clone())
                }
            })
            .collect()
    } else {
        node.attrs.clone()
    };
    Rc::new(XmlNode {
        name: node.name.clone(),
        attrs,
        children: node
            .children
            .iter()
            .map(|c| prefix_files(c, prefix))
            .collect(),
        text: node.text.clone(),
    })
}
