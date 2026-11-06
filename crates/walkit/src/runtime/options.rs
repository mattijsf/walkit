// SPDX-License-Identifier: GPL-3.0-or-later

//! The options a skin's scripts list for Winamp's Options menu, read from the Config item
//! Winamp lists there, and a pick of one flipped as a click there flips it.

use std::collections::HashSet;

use super::maki::{is_guid, shown_text, wtoi, Made, MENU_MAX_DEPTH, MENU_MAX_ITEMS};
use super::SkinRuntime;
use crate::host::{OptionEntry, OptionKey, SkinOptions};
use crate::maki::{Handle, Value};
use crate::xml::XmlNode;

/// The Config item whose attributes Winamp lists in its Options menu (`CUSTOM_OPTIONSMENU_ITEMS`).
pub(crate) const OPTIONS_MENU: &str = "{1828D28F-78DD-4647-8532-EBA504B8FC04}";
/// The most attributes of one item read into the menu.
const OPTION_ATTRIBUTES: usize = 500;

impl SkinRuntime {
    /// Counts changes to the skin's Config items: an item or attribute made, an attribute made
    /// by newAttribute, or a value changed. A host compares it to [`SkinOptions::version`] to
    /// know whether [`SkinRuntime::skin_options`] would read differently.
    pub fn config_version(&self) -> i32 {
        self.config_version
    }

    /// The options the skin's scripts list for Winamp's Options menu, read from the Config
    /// item Winamp lists there only (the Windows menu item is not read). An attribute whose
    /// value is another item's GUID is a submenu named by the attribute, "-" is a separator,
    /// and any other is a toggle. Attributes only read with getAttribute are left out. Reading
    /// makes no item. A toggle is a radio button when a toggle button of the skin set to that
    /// attribute draws a picture whose id says "radio", as Winamp Modern's config drawer draws
    /// its choices.
    pub fn skin_options(&self) -> SkinOptions {
        let version = self.config_version;
        let Some(root) = self.config_item_by_guid(OPTIONS_MENU) else {
            return SkinOptions {
                version,
                entries: Vec::new(),
            };
        };
        let radio = self.radio_attributes();
        let mut left = MENU_MAX_ITEMS;
        let entries = self.option_entries(root, 0, &[root], &mut left, &radio);
        SkinOptions { version, entries }
    }

    /// The entries of `item` while `left` (the entries still allowed in the whole tree) lasts;
    /// `path` holds its ancestors and itself.
    fn option_entries(
        &self,
        item: usize,
        depth: usize,
        path: &[usize],
        left: &mut i32,
        radio: &HashSet<String>,
    ) -> Vec<OptionEntry> {
        let attributes: Vec<usize> = match &self.made[item] {
            Made::ConfigItem { attributes, .. } => attributes
                .iter()
                .take(OPTION_ATTRIBUTES)
                .map(|(_, a)| *a)
                .collect(),
            _ => Vec::new(),
        };
        let mut out: Vec<OptionEntry> = Vec::new();
        for a in attributes {
            let Made::ConfigAttribute {
                name,
                data,
                made: true,
                ..
            } = &self.made[a]
            else {
                continue;
            };
            if name.trim().is_empty() {
                continue;
            }
            if *left <= 0 {
                break;
            }
            if data != "-" {
                if let Some(e) = self.option_entry(item, name, data, depth, path, left, radio) {
                    out.push(e);
                }
            } else if !matches!(out.last(), None | Some(OptionEntry::Separator)) {
                // none at the start and none twice; one at the end is dropped below
                out.push(OptionEntry::Separator);
                *left -= 1;
            }
        }
        if matches!(out.last(), Some(OptionEntry::Separator)) {
            out.pop();
            *left += 1;
        }
        out
    }

    /// The entry the attribute `name` of `item` with value `data` makes, counted off `left`: a
    /// submenu when its value names a Config item, or `None` when that item is on `path`, too
    /// deep or lists nothing; `None` for a GUID that names no item; else a toggle.
    #[allow(clippy::too_many_arguments)]
    fn option_entry(
        &self,
        item: usize,
        name: &str,
        data: &str,
        depth: usize,
        path: &[usize],
        left: &mut i32,
        radio: &HashSet<String>,
    ) -> Option<OptionEntry> {
        if is_guid(data) {
            let sub = self.config_item_by_guid(data)?;
            return self.option_submenu(name, sub, depth, path, left, radio);
        }
        let guid = match &self.made[item] {
            Made::ConfigItem { guid, .. } => guid.clone().unwrap_or_default(),
            _ => String::new(),
        };
        *left -= 1;
        Some(OptionEntry::Toggle {
            key: OptionKey {
                item: guid.clone(),
                attribute: name.to_lowercase(),
            },
            text: shown_text(name),
            checked: wtoi(data) != 0,
            radio: radio.contains(&radio_key(&guid, name)),
        })
    }

    /// The submenu the attribute `name` opens onto `sub`, or `None` when `sub` is on `path`,
    /// too deep or lists nothing.
    fn option_submenu(
        &self,
        name: &str,
        sub: usize,
        depth: usize,
        path: &[usize],
        left: &mut i32,
        radio: &HashSet<String>,
    ) -> Option<OptionEntry> {
        if path.contains(&sub) || depth >= MENU_MAX_DEPTH {
            return None;
        }
        // a submenu counts itself before its entries
        *left -= 1;
        let mut deeper = path.to_vec();
        deeper.push(sub);
        let entries = self.option_entries(sub, depth + 1, &deeper, left, radio);
        if entries.is_empty() {
            *left += 1;
            return None;
        }
        Some(OptionEntry::Submenu {
            text: shown_text(name),
            entries,
        })
    }

    /// The attributes a toggle button of the skin shows with a radio picture, as
    /// [`radio_key`] names them.
    fn radio_attributes(&self) -> HashSet<String> {
        fn walk(n: &XmlNode, out: &mut HashSet<String>) {
            if n.name == "togglebutton" {
                if let Some((item, name)) = n.get("cfgattrib").and_then(|a| a.split_once(';')) {
                    let pictures = [n.get("image"), n.get("activeimage"), n.get("downimage")];
                    if pictures
                        .iter()
                        .flatten()
                        .any(|p| p.to_lowercase().contains("radio"))
                    {
                        out.insert(radio_key(item, name));
                    }
                }
            }
            for c in &n.children {
                walk(c, out);
            }
        }
        let mut out = HashSet::new();
        for def in self.group_defs.values() {
            walk(def, &mut out);
        }
        out
    }

    /// Flips the option `key` names as a pick from Winamp's menu does: "1" over a value that
    /// reads as 0, "0" over any other. It is set with setData, so every script hears
    /// onDataChanged and the value is kept with the skin's settings. False for an unknown key,
    /// a separator, or a value that is a GUID, a submenu's or not.
    pub fn toggle_skin_option(&mut self, key: &OptionKey) -> bool {
        let Some(item) = self.config_item_by_guid(&key.item) else {
            return false;
        };
        let a = match &self.made[item] {
            Made::ConfigItem { attributes, .. } => attributes
                .iter()
                .find(|(n, _)| *n == key.attribute)
                .map(|(_, a)| *a),
            _ => None,
        };
        let Some(a) = a else { return false };
        let Made::ConfigAttribute {
            data, made: true, ..
        } = &self.made[a]
        else {
            return false;
        };
        if data == "-" || is_guid(data) {
            return false;
        }
        let value = if wtoi(data) != 0 { "0" } else { "1" };
        self.maki_call(Handle::Made(a), "setdata", &[Value::str(value)]);
        true
    }
}

/// An attribute as a toggle button's `cfgattrib` names it: the item's GUID without braces and
/// the name, lowercased.
fn radio_key(guid: &str, name: &str) -> String {
    let item = guid.trim().trim_matches(|c| c == '{' || c == '}');
    format!("{};{}", item.to_lowercase(), name.trim().to_lowercase())
}
