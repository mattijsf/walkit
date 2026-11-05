// SPDX-License-Identifier: GPL-3.0-or-later

//! The engine on skins of the reference corpus, checked against what the reference engine does
//! on the same skins where the trace cannot show it. The corpus is the `.wal` files of Andamp's
//! `modernskin/src/test/resources` (`WALKIT_CORPUS_SKINS`, or `../andamp/...` beside this
//! repository); without it every test here passes without checking anything.

#![cfg(feature = "native")]

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use walkit::files::SkinFiles;
use walkit::host::{Clock, OptionEntry, OptionKey, SkinSetup};
use walkit::player::FakePlayer;
use walkit::SkinRuntime;
use walkit_trace::archive::{decoder, read_wal};
use walkit_trace::TraceHost;

/// The skin's files, read by the host and the engine both.
struct Shared(Rc<dyn SkinFiles>);

impl SkinFiles for Shared {
    fn read(&self, path: &str) -> Option<Rc<[u8]>> {
        self.0.read(path)
    }

    fn names(&self) -> Vec<String> {
        self.0.names()
    }
}

/// The corpus folder, or `None` with a line on stderr when it is not here.
fn corpus() -> Option<PathBuf> {
    let dir = match std::env::var_os("WALKIT_CORPUS_SKINS") {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../andamp/modernskin/src/test/resources"),
    };
    if dir.is_dir() {
        Some(dir)
    } else {
        eprintln!("no corpus at {}: nothing checked", dir.display());
        None
    }
}

/// `wal` of the corpus loaded and ticked five times on a clock stuck at 0, as the reference
/// tests run it; `None` without the corpus.
fn runtime(wal: &str) -> Option<SkinRuntime> {
    let path = corpus()?.join(wal);
    let files: Rc<dyn SkinFiles> =
        Rc::new(read_wal(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())));
    let clock: Clock = Rc::new(|| 0);
    let host = TraceHost::new(
        files.clone(),
        Rc::new(RefCell::new(String::new())),
        decoder(),
        None,
    );
    let mut rt = SkinRuntime::new(
        Box::new(Shared(files)),
        Box::new(host),
        Box::new(FakePlayer::new(clock.clone())),
        SkinSetup::default(),
        clock,
    );
    rt.load();
    for _ in 0..5 {
        rt.tick();
    }
    Some(rt)
}

fn sub(entries: &[OptionEntry], wanted: &str) -> Vec<OptionEntry> {
    entries
        .iter()
        .find_map(|e| match e {
            OptionEntry::Submenu { text, entries } if text == wanted => Some(entries.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("a submenu {wanted}"))
}

/// The one toggle reading `wanted`: its key, whether it is checked, and whether it is a radio button.
fn toggle(entries: &[OptionEntry], wanted: &str) -> (OptionKey, bool, bool) {
    entries
        .iter()
        .find_map(|e| match e {
            OptionEntry::Toggle {
                key,
                text,
                checked,
                radio,
            } if text == wanted => Some((key.clone(), *checked, *radio)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("a toggle {wanted}"))
}

fn submenu_texts(entries: &[OptionEntry]) -> Vec<String> {
    entries
        .iter()
        .filter_map(|e| match e {
            OptionEntry::Submenu { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn drawers(rt: &SkinRuntime) -> Vec<OptionEntry> {
    sub(&sub(&rt.skin_options().entries, "Winamp Modern"), "Drawers")
}

#[test]
fn winamp_modern_lists_its_options_as_winamp_does_and_its_scripts_keep_a_drawer_pair_to_one_choice()
{
    let Some(mut rt) = runtime("WinampModern566.wal") else {
        return;
    };
    let top = rt.skin_options().entries;
    assert_eq!(submenu_texts(&top), ["Winamp Modern"]);
    assert_eq!(top.len(), 1);
    let modern = sub(&top, "Winamp Modern");
    assert_eq!(
        submenu_texts(&modern),
        [
            "Drawers",
            "Menus",
            "Main Windowshade Mode",
            "Vis Shortcut Button",
            "Notifications",
            "Songticker"
        ]
    );
    assert!(!toggle(&sub(&modern, "Drawers"), "Animate Config Drawer").1);

    let (top_key, was_ticked, _) = toggle(&drawers(&rt), "Open Video/Vis from the top");
    if was_ticked {
        let (bottom_key, _, _) = toggle(&drawers(&rt), "Open Video/Vis from the bottom");
        rt.toggle_skin_option(&bottom_key);
    }
    assert!(!toggle(&drawers(&rt), "Open Video/Vis from the top").1);

    assert!(rt.toggle_skin_option(&top_key));
    assert!(toggle(&drawers(&rt), "Open Video/Vis from the top").1);
    assert!(!toggle(&drawers(&rt), "Open Video/Vis from the bottom").1);

    rt.toggle_skin_option(&top_key);
    assert!(toggle(&drawers(&rt), "Open Video/Vis from the top").1);
}

#[test]
fn winamp_moderns_drawer_pair_are_radio_buttons_as_its_own_page_draws_them_and_its_other_options_are_not(
) {
    let Some(rt) = runtime("WinampModern566.wal") else {
        return;
    };
    let drawers = drawers(&rt);

    assert!(toggle(&drawers, "Open Video/Vis from the top").2);
    assert!(toggle(&drawers, "Open Video/Vis from the bottom").2);
    assert!(!toggle(&drawers, "Animate Config Drawer").2);
}

#[test]
fn big_bento_lists_eight_entries_at_the_top() {
    let Some(rt) = runtime("BigBento.wal") else {
        return;
    };
    assert_eq!(rt.skin_options().entries.len(), 8);
}
