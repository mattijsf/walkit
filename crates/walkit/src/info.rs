// SPDX-License-Identifier: GPL-3.0-or-later

//! What a skin says about itself, read from its files: who made it and what is in it, for a
//! host that shows a skin's details (a skin browser, an about box, a command-line listing).
//!
//! [`modern`] reads a modern skin's `<skininfo>`, its format version, containers, scripts,
//! fonts and the Winamp plugins it reaches into; [`classic`] reads which of a classic skin's
//! bitmaps it has and what its `pledit.txt`, `viscolor.txt` and `region.txt` say; [`texts`]
//! gives the author's text files and [`museum_readme`] picks a readme as the Winamp Skin
//! Museum does. Text is decoded as skins write it ([`decode`]).
//!
//! Pass the skin's own files: a [`LayeredFiles`](crate::files::LayeredFiles) also lists its
//! base's files, which are not the skin's. What the skin does once it runs (its windows, its
//! colour themes, the tags nothing defines) is the [`SkinRuntime`](crate::SkinRuntime)'s to
//! tell.

use std::rc::Rc;

use crate::classic::{pledit, region, viscolor};
use crate::draw::Rgb;
use crate::files::SkinFiles;
use crate::xml::{self, XmlNode};

/// The twelve bitmaps Winamp draws a classic skin's three windows from. `nums_ex.bmp` and
/// `balance.bmp` are not among them: `numbers.bmp` and `volume.bmp` stand in for them.
pub const CLASSIC_SHEETS: [&str; 12] = [
    "main.bmp",
    "cbuttons.bmp",
    "titlebar.bmp",
    "shufrep.bmp",
    "text.bmp",
    "volume.bmp",
    "monoster.bmp",
    "playpaus.bmp",
    "posbar.bmp",
    "numbers.bmp",
    "eqmain.bmp",
    "pledit.bmp",
];

/// The bitmaps a classic skin may add to the [`CLASSIC_SHEETS`].
pub const CLASSIC_EXTRAS: [&str; 8] = [
    "nums_ex.bmp",
    "eq_ex.bmp",
    "balance.bmp",
    "gen.bmp",
    "genex.bmp",
    "video.bmp",
    "mb.bmp",
    "avs.bmp",
];

/// What a classic skin holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassicInfo {
    /// The [`CLASSIC_SHEETS`] the skin does not have, in that order.
    pub missing_sheets: Vec<String>,
    /// The [`CLASSIC_EXTRAS`] it has, in that order.
    pub extras: Vec<String>,
    /// The playlist's colours and font from `pledit.txt`; `None` without one.
    pub playlist: Option<pledit::PleditStyle>,
    /// The 24 vis colours from `viscolor.txt`; empty without one.
    pub vis_colors: Vec<Rgb>,
    /// The sections of `region.txt`, the windows it shapes (such as `normal` or `equalizer`),
    /// sorted; empty without one.
    pub regions: Vec<String>,
    /// The cursor files (`.cur`, `.ani`).
    pub cursors: Vec<String>,
}

/// What a modern skin holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModernInfo {
    /// `<skininfo>`: what the author says of the skin.
    pub skin_info: SkinInfo,
    /// The version of the skin format: the `version` of the root element of `skin.xml`
    /// (`WinampAbstractionLayer` or `WasabiXML`).
    pub format_version: Option<String>,
    /// The containers the skin's XML files define, each id once (the first, ids compared
    /// without regard to case).
    pub containers: Vec<Container>,
    /// The compiled scripts (`.maki`).
    pub scripts: Vec<String>,
    /// The TrueType font files (`.ttf`, `.otf`).
    pub fonts: Vec<String>,
    /// The Winamp plugins the skin's includes reach into, by folder name:
    /// `.../Plugins/classicPro/engine/load.xml` names `classicPro`. Such a skin is made for the
    /// plugin, and its windows are in the plugin's files. Sorted, each once.
    pub plugins: Vec<String>,
}

/// `<skininfo>`. A field is the element's text, or else its `text` attribute; `None` when
/// both are empty or the element is missing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkinInfo {
    /// `<name>`.
    pub name: Option<String>,
    /// `<version>`.
    pub version: Option<String>,
    /// `<author>`.
    pub author: Option<String>,
    /// `<comment>`.
    pub comment: Option<String>,
    /// `<email>`.
    pub email: Option<String>,
    /// `<homepage>`.
    pub homepage: Option<String>,
    /// `<screenshot>`: the screenshot Winamp shows.
    pub screenshot: Option<String>,
    /// `<wacupscreenshot>`: the screenshot WACUP shows.
    pub wacup_screenshot: Option<String>,
}

/// A `<container>` of a modern skin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Container {
    /// Its `id`.
    pub id: String,
    /// Its `name`, the title Winamp gives its window.
    pub name: Option<String>,
    /// Whether it opens when the skin loads: `default_visible` is not `0`.
    pub default_visible: bool,
}

/// A text file of the author's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text {
    /// Its path among the skin's files.
    pub file: String,
    /// Its text, decoded by [`decode`].
    pub text: String,
}

/// What `files`, a classic skin's, hold.
pub fn classic(files: &dyn SkinFiles) -> ClassicInfo {
    let text = |name: &str| files.read(name).map(|b| decode(&b));
    let mut regions: Vec<String> = text("region.txt")
        .map(|t| region::parse(&t).into_keys().collect())
        .unwrap_or_default();
    regions.sort();
    ClassicInfo {
        missing_sheets: CLASSIC_SHEETS
            .iter()
            .filter(|s| !files.exists(s))
            .map(ToString::to_string)
            .collect(),
        extras: CLASSIC_EXTRAS
            .iter()
            .filter(|s| files.exists(s))
            .map(ToString::to_string)
            .collect(),
        playlist: text("pledit.txt").map(|t| pledit::parse(&t)),
        vis_colors: text("viscolor.txt")
            .map(|t| viscolor::parse(&t).to_vec())
            .unwrap_or_default(),
        regions,
        cursors: ending(&files.names(), &[".cur", ".ani"]),
    }
}

/// What `files`, a modern skin's, hold. Every XML file among them is read, in the order of
/// [`SkinFiles::names`], whether or not `skin.xml` includes it.
pub fn modern(files: &dyn SkinFiles) -> ModernInfo {
    let names = files.names();
    let xml_texts: Vec<String> = names
        .iter()
        .filter(|n| n.to_lowercase().ends_with(".xml"))
        .filter_map(|n| files.read(n))
        // as the runtime reads a skin's XML
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .collect();
    let mut nodes = Vec::new();
    for text in &xml_texts {
        for root in xml::parse(text) {
            walk(&root, &mut nodes);
        }
    }
    let format_version = files
        .read("skin.xml")
        .map(|b| decode(&b))
        .and_then(|t| {
            xml::parse(&t)
                .into_iter()
                .find(|r| r.name == "winampabstractionlayer" || r.name == "wasabixml")
        })
        .and_then(|r| r.get("version").map(str::to_string));
    let skin_info = nodes
        .iter()
        .find(|n| n.name == "skininfo")
        .map(|n| skin_info(n))
        .unwrap_or_default();
    let mut containers: Vec<Container> = Vec::new();
    for node in nodes.iter().filter(|n| n.name == "container") {
        let Some(id) = node.get("id").filter(|id| !id.is_empty()) else {
            continue;
        };
        if containers.iter().any(|c| c.id.eq_ignore_ascii_case(id)) {
            continue;
        }
        containers.push(Container {
            id: id.to_string(),
            name: node.get("name").map(str::to_string),
            default_visible: node.get("default_visible").is_none_or(|v| v.trim() != "0"),
        });
    }
    ModernInfo {
        skin_info,
        format_version,
        containers,
        scripts: ending(&names, &[".maki"]),
        fonts: ending(&names, &[".ttf", ".otf"]),
        plugins: plugins(&xml_texts),
    }
}

/// The fields of a `<skininfo>` element.
fn skin_info(info: &XmlNode) -> SkinInfo {
    let field = |key: &str| {
        let n = info.children.iter().find(|c| c.name == key)?;
        let value = if n.text.is_empty() {
            n.get("text").unwrap_or_default()
        } else {
            n.text.as_str()
        };
        (!value.is_empty()).then(|| value.to_string())
    };
    SkinInfo {
        name: field("name"),
        version: field("version"),
        author: field("author"),
        comment: field("comment"),
        email: field("email"),
        homepage: field("homepage"),
        screenshot: field("screenshot"),
        wacup_screenshot: field("wacupscreenshot"),
    }
}

/// The plugin folders the `<include>`s in `xml_texts` reach into, sorted, each once.
fn plugins(xml_texts: &[String]) -> Vec<String> {
    let mut plugins = Vec::new();
    for text in xml_texts {
        let lower = text.to_ascii_lowercase();
        for (at, _) in lower.match_indices("<include") {
            let tag = &text[at..];
            let tag = &tag[..tag.find('>').unwrap_or(tag.len())];
            let path = tag.replace('\\', "/");
            let Some(start) = path.to_ascii_lowercase().find("/plugins/") else {
                continue;
            };
            let name: String = path[start + "/plugins/".len()..]
                .chars()
                .take_while(|c| *c != '/' && *c != '"' && *c != '\'')
                .collect();
            if !name.is_empty() && !plugins.contains(&name) {
                plugins.push(name);
            }
        }
    }
    plugins.sort();
    plugins
}

/// The author's text files among `files`: each `.txt`, `.nfo` or `.diz` that neither Winamp
/// nor a plugin reads (see [`is_author_text`]) and that holds more than white space, in the
/// order of [`SkinFiles::names`].
pub fn texts(files: &dyn SkinFiles) -> Vec<Text> {
    files
        .names()
        .into_iter()
        .filter(|f| is_author_text(f))
        .filter_map(|file| {
            let text = decode(&files.read(&file)?);
            (!text.trim().is_empty()).then_some(Text { file, text })
        })
        .collect()
}

/// The names the museum never takes for a readme: what Winamp and its plugins read, and
/// files download sites add. The museum's pattern anchors only its last name at the end.
const NOT_README: [&str; 13] = [
    "dialogs.txt",
    "genex.txt",
    "genexinfo.txt",
    "gen_gslyrics.txt",
    "region.txt",
    "pledit.txt",
    "viscolor.txt",
    "winampmb.txt",
    "\"gen_ex help.txt",
    "mbinner.txt",
    "winampskins.info.txt",
    "albumlist.txt",
    "covertag.txt",
];

/// Whether the Winamp Skin Museum takes the archive entry `name` (its full path in the
/// archive) for a readme. `getReadme` in the webamp repository's
/// `packages/skin-database/data/SkinModel.ts` tests the name against
/// `/(file_id\.diz)|(\.txt)$/i` and not against its list of other files, which it matches
/// anywhere in the name but for its last.
pub fn is_museum_readme(name: &str) -> bool {
    let name = name.to_lowercase();
    let readme =
        name.contains("file_id.diz") || name.rsplit_once('.').is_some_and(|(_, e)| e == "txt");
    let not =
        NOT_README.iter().any(|n| name.contains(n)) || name.ends_with("1001winampskins.com.txt");
    readme && !not
}

/// The readme the Winamp Skin Museum shows: the first of `names` that [`is_museum_readme`].
///
/// The museum goes through the archive's entries in the archive's order, so pass the names in
/// that order (the host has it). [`SkinFiles::names`] is sorted, and with more than one
/// candidate the first in sorted order may not be the museum's.
pub fn museum_readme<'a>(names: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    names.into_iter().find(|n| is_museum_readme(n))
}

/// Whether the file at `path` is a text of the author's: a `.txt`, `.nfo` or `.diz` that
/// neither Winamp nor a plugin reads (such as `pledit.txt` or `region.txt`).
pub fn is_author_text(path: &str) -> bool {
    let name = path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .to_ascii_lowercase();
    let text = [".txt", ".nfo", ".diz"].iter().any(|e| name.ends_with(e));
    let read_by_winamp = NOT_README.contains(&name.as_str())
        || ["viscolor.txt", "gen_ex help.txt", "1001winampskins.com.txt"].contains(&name.as_str());
    text && !read_by_winamp
}

/// What Windows' Western code page (1252) has at 0x80 to 0x9f, where Latin-1 has control
/// characters; the five it leaves out stay as they are.
const CP1252_HIGH: [char; 32] = [
    '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}',
    '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
];

/// Text as skins write it: UTF-8, or else Windows' Western code page (1252). A leading byte
/// order mark is dropped and line ends become `\n`.
pub fn decode(bytes: &[u8]) -> String {
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => bytes
            .iter()
            .map(|&b| match b {
                0x80..=0x9f => CP1252_HIGH[usize::from(b - 0x80)],
                b => char::from(b),
            })
            .collect(),
    };
    text.trim_start_matches('\u{FEFF}')
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}

/// Every element under `node`, itself first.
fn walk(node: &Rc<XmlNode>, out: &mut Vec<Rc<XmlNode>>) {
    out.push(node.clone());
    for c in &node.children {
        walk(c, out);
    }
}

/// The names whose endings are one of `endings` (lowercase).
fn ending(names: &[String], endings: &[&str]) -> Vec<String> {
    names
        .iter()
        .filter(|f| {
            let lower = f.to_ascii_lowercase();
            endings.iter().any(|e| lower.ends_with(e))
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::MemoryFiles;

    fn files(entries: &[(&str, &[u8])]) -> MemoryFiles {
        let mut files = MemoryFiles::new();
        for (path, bytes) in entries {
            files.insert(path, bytes.to_vec());
        }
        files
    }

    #[test]
    fn a_modern_skin_tells_who_made_it() {
        let skin = files(&[
            (
                "skin.xml",
                br#"<?xml version="1.0"?>
<WinampAbstractionLayer version="1.36">
  <skininfo>
    <version>1.2</version>
    <name>Sample</name>
    <author text="Someone"/>
    <comment></comment>
    <email>a@example.com</email>
    <homepage>https://example.com</homepage>
    <screenshot>shot.png</screenshot>
    <wacupscreenshot>wacup.png</wacupscreenshot>
  </skininfo>
  <include file="xml/player.xml"/>
  <include file="..\..\Plugins\classicPro\engine\load.xml"/>
</WinampAbstractionLayer>"#,
            ),
            (
                "xml/player.xml",
                br#"<container id="main" name="Main Window"><layout id="normal"/></container>
<container id="PL" default_visible="0"/>
<container id="pl" name="again"/>
<container id=""/>
<include file="../Plugins/Albumart/x.xml"/>"#,
            ),
            ("scripts/a.maki", b""),
            ("fonts/Nice.TTF", b""),
        ]);
        let m = modern(&skin);
        assert_eq!(
            m.skin_info,
            SkinInfo {
                name: Some("Sample".into()),
                version: Some("1.2".into()),
                author: Some("Someone".into()),
                comment: None,
                email: Some("a@example.com".into()),
                homepage: Some("https://example.com".into()),
                screenshot: Some("shot.png".into()),
                wacup_screenshot: Some("wacup.png".into()),
            }
        );
        assert_eq!(m.format_version.as_deref(), Some("1.36"));
        assert_eq!(
            m.containers,
            [
                Container {
                    id: "main".into(),
                    name: Some("Main Window".into()),
                    default_visible: true,
                },
                Container {
                    id: "PL".into(),
                    name: None,
                    default_visible: false,
                },
            ]
        );
        assert_eq!(m.scripts, ["scripts/a.maki"]);
        assert_eq!(m.fonts, ["fonts/nice.ttf"]);
        assert_eq!(m.plugins, ["Albumart", "classicPro"]);
    }

    #[test]
    fn an_old_skin_names_its_format_wasabixml() {
        let skin = files(&[("skin.xml", br#"<WasabiXML version="0.8"></WasabiXML>"#)]);
        let m = modern(&skin);
        assert_eq!(m.format_version.as_deref(), Some("0.8"));
        assert_eq!(m.skin_info, SkinInfo::default());
        assert!(m.containers.is_empty());
    }

    #[test]
    fn a_classic_skin_tells_what_it_has() {
        let skin = files(&[
            ("main.bmp", b""),
            ("cbuttons.bmp", b""),
            ("gen.bmp", b""),
            ("nums_ex.bmp", b""),
            ("pledit.txt", b"[Text]\r\nNormal=#00FF00\r\nFont=Tahoma\r\n"),
            ("region.txt", b"[Normal]\nNumPoints=4\nPointList=0,0,275,0,275,116,0,116\n[Equalizer]\nNumPoints=4\nPointList=0,0,275,0,275,116,0,116\n"),
            ("viscolor.txt", b"1,2,3\n"),
            ("normal.cur", b""),
            ("eqslid.ANI", b""),
        ]);
        let c = classic(&skin);
        assert_eq!(
            c.missing_sheets,
            [
                "titlebar.bmp",
                "shufrep.bmp",
                "text.bmp",
                "volume.bmp",
                "monoster.bmp",
                "playpaus.bmp",
                "posbar.bmp",
                "numbers.bmp",
                "eqmain.bmp",
                "pledit.bmp",
            ]
        );
        assert_eq!(c.extras, ["nums_ex.bmp", "gen.bmp"]);
        let playlist = c.playlist.unwrap();
        assert_eq!(playlist.normal, Rgb(0x00ff00));
        assert_eq!(playlist.font, "Tahoma");
        assert_eq!(c.vis_colors.len(), 24);
        assert_eq!(c.vis_colors[0], Rgb(0x010203));
        assert_eq!(c.regions, ["equalizer", "normal"]);
        assert_eq!(c.cursors, ["eqslid.ani", "normal.cur"]);
    }

    #[test]
    fn a_classic_skin_without_its_texts_has_none() {
        let c = classic(&files(&[("main.bmp", b"")]));
        assert_eq!(c.playlist, None);
        assert!(c.vis_colors.is_empty());
        assert!(c.regions.is_empty());
        assert!(c.cursors.is_empty());
    }

    #[test]
    fn text_decodes_as_utf8_or_else_windows_1252() {
        assert_eq!(
            decode("\u{FEFF}café\r\nline\rend".as_bytes()),
            "café\nline\nend"
        );
        // 0x93 and 0x94 are curly quotes in 1252, 0xe9 is é in both 1252 and Latin-1
        assert_eq!(decode(b"\x93caf\xe9\x94 \x80 \x81"), "“café” € \u{81}");
    }

    #[test]
    fn the_museum_picks_a_readme_as_it_does() {
        assert!(is_museum_readme("Readme.TXT"));
        assert!(is_museum_readme("skin/FILE_ID.DIZ"));
        assert!(is_museum_readme("file_id.diz.bak"));
        assert!(!is_museum_readme("pledit.txt"));
        assert!(!is_museum_readme("skin/region.txt"));
        // the other names match anywhere in the name
        assert!(!is_museum_readme("old_pledit.txt.txt"));
        assert!(!is_museum_readme("www.1001winampskins.com.txt"));
        assert!(!is_museum_readme("notes.nfo"));
        assert_eq!(
            museum_readme(["main.bmp", "pledit.txt", "zz.txt", "aa.txt"]),
            Some("zz.txt")
        );
        assert_eq!(museum_readme(["main.bmp"]), None);
    }

    #[test]
    fn the_authors_texts_leave_out_what_winamp_reads() {
        let skin = files(&[
            ("readme.txt", b"Hello\r\n"),
            ("info.NFO", b"\xa9 me"),
            ("blank.txt", b"  \n"),
            ("pledit.txt", b"[Text]"),
            ("viscolor.txt", b"0,0,0"),
            ("gen_ex help.txt", b"x"),
            ("main.bmp", b""),
        ]);
        assert_eq!(
            texts(&skin),
            [
                Text {
                    file: "info.nfo".into(),
                    text: "© me".into(),
                },
                Text {
                    file: "readme.txt".into(),
                    text: "Hello\n".into(),
                },
            ]
        );
        assert!(is_author_text("Skin\\Notes.diz"));
        assert!(!is_author_text("region.txt"));
    }
}
