// SPDX-License-Identifier: GPL-3.0-or-later

//! The scenario file: what each run is made of.

use std::path::Path;

use serde_json::Value;

/// One run of a skin: the skin, the songs the player shows, and what the run does.
#[derive(Debug, Clone)]
pub struct Scenario {
    /// The trace's name.
    pub name: String,
    /// The `.wal` file in the skins folder.
    pub wal: String,
    /// Winamp's getSkinName(): the skin's folder name.
    pub skin: String,
    /// The song titles.
    pub playlist: Vec<String>,
    /// The color theme put on first, if any.
    pub theme: Option<String>,
    /// The view the windows stack in, if any.
    pub view: Option<(i32, i32)>,
    /// The windows opened at the start.
    pub show: Vec<String>,
    /// The main window's size, if resized.
    pub main_size: Option<(i32, i32)>,
    /// The objects tapped, in order.
    pub taps: Vec<String>,
    /// Whether the host has a media library.
    pub have_library: bool,
    /// Whether the default element set is left out.
    pub no_defaults: bool,
    /// Whether the sibling skins are left out.
    pub no_siblings: bool,
}

/// The scenarios in the JSON file at `path`.
pub fn read(path: &Path) -> Result<Vec<Scenario>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let json: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let Some(arr) = json.as_array() else {
        return Err("not a JSON list".into());
    };
    let strings = |o: &Value, key: &str| -> Option<Vec<String>> {
        o.get(key)?.as_array().map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
    };
    let pair = |o: &Value, key: &str| -> Option<(i32, i32)> {
        let a = o.get(key)?.as_array()?;
        if a.len() != 2 {
            return None;
        }
        Some((a[0].as_i64()? as i32, a[1].as_i64()? as i32))
    };
    let string = |o: &Value, key: &str| -> Result<String, String> {
        o.get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| format!("a scenario has no {key}"))
    };
    arr.iter()
        .map(|o| {
            Ok(Scenario {
                name: string(o, "name")?,
                wal: string(o, "wal")?,
                skin: string(o, "skin")?.trim_end_matches(".wal").to_string(),
                playlist: strings(o, "playlist").unwrap_or_else(|| vec!["Andamp".to_string()]),
                theme: o
                    .get("theme")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                view: pair(o, "view"),
                show: strings(o, "show").unwrap_or_default(),
                main_size: pair(o, "mainsize"),
                taps: strings(o, "taps").unwrap_or_default(),
                have_library: !o
                    .get("noLibrary")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                no_defaults: o
                    .get("noDefaults")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                no_siblings: o
                    .get("noSiblings")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            })
        })
        .collect()
}
