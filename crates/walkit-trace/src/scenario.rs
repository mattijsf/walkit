// SPDX-License-Identifier: GPL-3.0-or-later

//! One run of a skin: the skin, the songs the player shows, and what the run does. A scenario
//! file is a JSON list of them; a single scenario also travels as a few `key=value` lines
//! (`spec`), which needs no JSON reader on the receiving side.

/// One run of a skin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scenario {
    /// The trace's name.
    pub name: String,
    /// The `.wal` file in the skins folder.
    pub wal: String,
    /// Winamp's `getSkinName()`: the skin's folder name.
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

impl Scenario {
    /// The scenario as `key=value` lines: lists joined with `|`, pairs as `WxH`, flags as `1`.
    pub fn spec(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("name={}\n", self.name));
        s.push_str(&format!("wal={}\n", self.wal));
        s.push_str(&format!("skin={}\n", self.skin));
        s.push_str(&format!("playlist={}\n", self.playlist.join("|")));
        if let Some(t) = &self.theme {
            s.push_str(&format!("theme={t}\n"));
        }
        if let Some((w, h)) = self.view {
            s.push_str(&format!("view={w}x{h}\n"));
        }
        if !self.show.is_empty() {
            s.push_str(&format!("show={}\n", self.show.join("|")));
        }
        if let Some((w, h)) = self.main_size {
            s.push_str(&format!("mainsize={w}x{h}\n"));
        }
        if !self.taps.is_empty() {
            s.push_str(&format!("taps={}\n", self.taps.join("|")));
        }
        if !self.have_library {
            s.push_str("nolibrary=1\n");
        }
        if self.no_defaults {
            s.push_str("nodefaults=1\n");
        }
        if self.no_siblings {
            s.push_str("nosiblings=1\n");
        }
        s
    }

    /// A scenario from its `spec` lines; what is missing takes the reference run's defaults.
    pub fn from_spec(spec: &str) -> Scenario {
        let mut sc = Scenario {
            name: String::new(),
            wal: String::new(),
            skin: String::new(),
            playlist: vec!["Andamp".to_string()],
            theme: None,
            view: None,
            show: Vec::new(),
            main_size: None,
            taps: Vec::new(),
            have_library: true,
            no_defaults: false,
            no_siblings: false,
        };
        let list = |v: &str| -> Vec<String> {
            v.split('|')
                .filter(|s| !s.is_empty())
                .map(std::string::ToString::to_string)
                .collect()
        };
        let pair = |v: &str| -> Option<(i32, i32)> {
            let (w, h) = v.split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        };
        for line in spec.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            match k {
                "name" => sc.name = v.to_string(),
                "wal" => sc.wal = v.to_string(),
                "skin" => sc.skin = v.to_string(),
                "playlist" => sc.playlist = list(v),
                "theme" => sc.theme = Some(v.to_string()),
                "view" => sc.view = pair(v),
                "show" => sc.show = list(v),
                "mainsize" => sc.main_size = pair(v),
                "taps" => sc.taps = list(v),
                "nolibrary" => sc.have_library = v != "1",
                "nodefaults" => sc.no_defaults = v == "1",
                "nosiblings" => sc.no_siblings = v == "1",
                _ => {}
            }
        }
        sc
    }
}

/// The scenarios in a JSON list: fields `name`, `wal`, `skin`, `playlist`, `theme`, `view`,
/// `show`, `mainsize`, `taps`, `noLibrary`, `noDefaults`, `noSiblings`.
#[cfg(feature = "native")]
pub fn parse_json(text: &str) -> Result<Vec<Scenario>, String> {
    use serde_json::Value;
    let json: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let Some(arr) = json.as_array() else {
        return Err("not a JSON list".into());
    };
    let strings = |o: &Value, key: &str| -> Option<Vec<String>> {
        o.get(key)?.as_array().map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(std::string::ToString::to_string))
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
            .map(std::string::ToString::to_string)
            .ok_or_else(|| format!("a scenario has no {key}"))
    };
    let flag = |o: &Value, key: &str| {
        o.get(key)
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
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
                    .map(std::string::ToString::to_string),
                view: pair(o, "view"),
                show: strings(o, "show").unwrap_or_default(),
                main_size: pair(o, "mainsize"),
                taps: strings(o, "taps").unwrap_or_default(),
                have_library: !flag(o, "noLibrary"),
                no_defaults: flag(o, "noDefaults"),
                no_siblings: flag(o, "noSiblings"),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spec_round_trips() {
        let sc = Scenario {
            name: "x".into(),
            wal: "x.wal".into(),
            skin: "X".into(),
            playlist: vec!["A".into(), "B".into()],
            theme: Some("Copper".into()),
            view: Some((354, 564)),
            show: vec!["pledit".into()],
            main_size: None,
            taps: vec!["pl".into(), "winshade".into()],
            have_library: false,
            no_defaults: true,
            no_siblings: false,
        };
        assert_eq!(Scenario::from_spec(&sc.spec()), sc);
    }
}
