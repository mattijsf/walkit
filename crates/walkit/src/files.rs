// SPDX-License-Identifier: GPL-3.0-or-later

//! The files of a skin, looked up by path without regard to case or slash direction, and the
//! ways a host stacks them: a base under a skin (Winamp's wasabi base under a modern skin, a
//! base skin under a classic one), and the sibling skins of a Skins folder a path may reach into.

use std::collections::BTreeMap;
use std::rc::Rc;

/// The files of one skin. Paths are compared case-insensitively with either slash.
pub trait SkinFiles {
    /// The bytes of the file at `path`, or `None` when the skin has no such file.
    fn read(&self, path: &str) -> Option<Rc<[u8]>>;

    /// Whether the skin has a file at `path`.
    fn exists(&self, path: &str) -> bool {
        self.read(path).is_some()
    }

    /// Every path in the skin, normalized (see [`normalize`]).
    fn names(&self) -> Vec<String>;
}

/// A path as the skin's files key it: forward slashes, no leading slash, lowercase, with empty
/// and `.` segments dropped.
pub fn normalize(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches('/')
        .to_lowercase()
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect::<Vec<_>>()
        .join("/")
}

/// The folder a skin sits in inside an archive, as a normalized prefix ending in `/`, or `""`
/// when it sits at the root. A modern skin's folder holds `skin.xml`; an archive without one
/// is a classic skin when a folder holds `main.bmp` (some classic skins are zipped inside a
/// folder). The first such file found decides which folder; `None` when there is neither.
pub fn skin_folder<'a>(names: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let normalized: Vec<String> = names.into_iter().map(normalize).collect();
    ["skin.xml", "main.bmp"].into_iter().find_map(|file| {
        if normalized.iter().any(|n| n == file) {
            return Some(String::new());
        }
        let tail = format!("/{file}");
        normalized
            .iter()
            .find(|n| n.ends_with(&tail))
            .map(|n| n[..n.len() - file.len()].to_string())
    })
}

/// A skin held in memory: its files by normalized path. A host fills it from an archive it
/// unpacked, applying the limits it chooses, and strips the skin's folder (see [`skin_folder`]).
#[derive(Default, Clone)]
pub struct MemoryFiles {
    entries: BTreeMap<String, Rc<[u8]>>,
}

impl MemoryFiles {
    /// No files.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `bytes` at `path`; a later file at the same path replaces the earlier one.
    pub fn insert(&mut self, path: &str, bytes: impl Into<Rc<[u8]>>) {
        self.entries.insert(normalize(path), bytes.into());
    }

    /// The same files with the folder `prefix` (normalized, ending in `/`) stripped, and the
    /// files outside it dropped. Unchanged for an empty prefix.
    #[must_use]
    pub fn in_folder(self, prefix: &str) -> Self {
        if prefix.is_empty() {
            return self;
        }
        let entries = self
            .entries
            .into_iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k[prefix.len()..].to_string(), v))
            .collect();
        Self { entries }
    }

    /// How many files it holds.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether it holds no file.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl SkinFiles for MemoryFiles {
    fn read(&self, path: &str) -> Option<Rc<[u8]>> {
        self.entries.get(&normalize(path)).cloned()
    }

    fn names(&self) -> Vec<String> {
        self.entries.keys().cloned().collect()
    }
}

/// Where the base under a skin lives among its paths.
pub const BASE_PREFIX: &str = "wasabi-base/";

/// A skin's files with a base behind them: paths under [`BASE_PREFIX`] read from the base,
/// everything else from the skin. Under a modern skin the base is Winamp's wasabi base (the
/// `Plugins\freeform\xml\wasabi` folder, zipped), which Winamp draws lists, scrollbars and
/// frames a skin does not define from. Under a classic skin it is a classic base skin (such as
/// Winamp's base skin), which gives every bitmap and cursor the skin leaves out.
pub struct LayeredFiles {
    skin: Box<dyn SkinFiles>,
    base: Box<dyn SkinFiles>,
}

impl LayeredFiles {
    /// `skin` over `base`.
    pub fn new(skin: Box<dyn SkinFiles>, base: Box<dyn SkinFiles>) -> Self {
        Self { skin, base }
    }
}

impl SkinFiles for LayeredFiles {
    fn read(&self, path: &str) -> Option<Rc<[u8]>> {
        let n = normalize(path);
        if let Some(rest) = n.strip_prefix(BASE_PREFIX) {
            self.base.read(rest)
        } else {
            self.skin.read(path)
        }
    }

    fn names(&self) -> Vec<String> {
        let mut names = self.skin.names();
        names.extend(
            self.base
                .names()
                .into_iter()
                .map(|n| format!("{BASE_PREFIX}{n}")),
        );
        names
    }
}

/// How a Skins folder answers a sibling's folder name: its files, or `None` for no such skin.
pub type SiblingLookup = Box<dyn Fn(&str) -> Option<Rc<dyn SkinFiles>>>;

/// A skin beside the other skins in a Skins folder: a path `../<name>/...` or
/// `@SKINSPATH@/<name>/...` reads from that sibling (Bento takes its scripts and most of its
/// XML from "Big Bento"). Siblings are named by their folder name lowercased without spaces
/// ("bigbento"); the lookup gives the one by that name, or `None`, so a host opens a sibling
/// only when a path names it.
pub struct SkinsFolderFiles {
    skin: Box<dyn SkinFiles>,
    siblings: SiblingLookup,
}

impl SkinsFolderFiles {
    /// `skin` with `siblings` answering a folder name.
    pub fn new(skin: Box<dyn SkinFiles>, siblings: SiblingLookup) -> Self {
        Self { skin, siblings }
    }

    fn sibling(&self, path: &str) -> Option<(Rc<dyn SkinFiles>, String)> {
        let full = normalize(path);
        let n = full.strip_prefix("@skinspath@/").unwrap_or(&full);
        let parts: Vec<&str> = n.split('/').collect();
        let start = if parts.first() == Some(&"..") {
            1
        } else if n != full {
            0
        } else {
            return None;
        };
        let name = parts.get(start)?.replace(' ', "");
        let files = (self.siblings)(&name)?;
        Some((files, parts[start + 1..].join("/")))
    }
}

impl SkinFiles for SkinsFolderFiles {
    fn read(&self, path: &str) -> Option<Rc<[u8]>> {
        if let Some((files, p)) = self.sibling(path) {
            if let Some(bytes) = files.read(&p) {
                return Some(bytes);
            }
        }
        self.skin.read(path)
    }

    fn names(&self) -> Vec<String> {
        self.skin.names()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_normalize_to_one_spelling() {
        assert_eq!(normalize("\\Xml\\./Player.XML"), "xml/player.xml");
        assert_eq!(normalize("a//b"), "a/b");
    }

    #[test]
    fn the_skin_folder_is_found() {
        assert_eq!(
            skin_folder(["Skin/skin.xml", "Skin/a.png"]).as_deref(),
            Some("skin/")
        );
        assert_eq!(skin_folder(["skin.xml"]).as_deref(), Some(""));
        assert_eq!(skin_folder(["a.png"]), None);
        // a classic skin zipped inside a folder
        assert_eq!(
            skin_folder([
                "Win-98_Ruby/",
                "Win-98_Ruby/Main.bmp",
                "Win-98_Ruby/pledit.txt"
            ])
            .as_deref(),
            Some("win-98_ruby/")
        );
        assert_eq!(
            skin_folder(["main.bmp", "extra/main.bmp"]).as_deref(),
            Some("")
        );
        // skin.xml decides over main.bmp
        assert_eq!(
            skin_folder(["old/main.bmp", "new/skin.xml"]).as_deref(),
            Some("new/")
        );
    }

    #[test]
    fn a_sibling_path_reads_the_sibling() {
        let mut big = MemoryFiles::new();
        big.insert("scripts/a.maki", b"big".to_vec());
        let big: Rc<dyn SkinFiles> = Rc::new(big);
        let mut skin = MemoryFiles::new();
        skin.insert("skin.xml", b"<x/>".to_vec());
        let files = SkinsFolderFiles::new(
            Box::new(skin),
            Box::new(move |name| {
                if name == "bigbento" {
                    Some(big.clone())
                } else {
                    None
                }
            }),
        );
        assert_eq!(&*files.read("../Big Bento/scripts/a.maki").unwrap(), b"big");
        assert_eq!(
            &*files.read("@SKINSPATH@/BigBento/scripts/a.maki").unwrap(),
            b"big"
        );
        assert!(files.read("../other/scripts/a.maki").is_none());
        assert!(files.read("skin.xml").is_some());
    }

    #[test]
    fn the_base_reads_under_its_prefix() {
        let mut base = MemoryFiles::new();
        base.insert("wasabi.xml", b"base".to_vec());
        let mut skin = MemoryFiles::new();
        skin.insert("skin.xml", b"skin".to_vec());
        let files = LayeredFiles::new(Box::new(skin), Box::new(base));
        assert_eq!(&*files.read("wasabi-base/Wasabi.xml").unwrap(), b"base");
        assert_eq!(&*files.read("skin.xml").unwrap(), b"skin");
        assert!(files
            .names()
            .contains(&"wasabi-base/wasabi.xml".to_string()));
    }
}
