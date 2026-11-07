// SPDX-License-Identifier: GPL-3.0-or-later
//! A classic skin's main window, drawn pixel for pixel as Winamp draws it (as
//! Winamp 5.666 shows it in Winamp 5.666 in a VM).

use std::rc::Rc;

use super::QuietHost;
use crate::draw::{Align, DrawCmd, Rect, Region};
use crate::files::MemoryFiles;
use crate::host::{Clock, SkinSetup};
use crate::player::FakePlayer;
use crate::runtime::SkinRuntime;

const SHEETS: [&str; 15] = [
    "main.bmp",
    "cbuttons.bmp",
    "titlebar.bmp",
    "text.bmp",
    "numbers.bmp",
    "playpaus.bmp",
    "monoster.bmp",
    "posbar.bmp",
    "volume.bmp",
    "balance.bmp",
    "shufrep.bmp",
    "eqmain.bmp",
    "pledit.bmp",
    "pledit.txt",
    "region.txt",
];

const REGION: &str =
    "[Normal]\nNumPoints=8\nPointList=0,2, 2,2, 2,0, 273,0, 273,2, 275,2, 275,116, 0,116\n";

fn classic() -> SkinRuntime {
    let mut files = MemoryFiles::new();
    for s in SHEETS {
        files.insert(s, &b"x"[..]);
    }
    files.insert("region.txt", REGION.as_bytes());
    let clock: Clock = Rc::new(|| 0);
    let mut rt = SkinRuntime::new(
        Box::new(files),
        Box::new(QuietHost),
        Box::new(FakePlayer::new(clock.clone())),
        SkinSetup::default(),
        clock,
    );
    rt.load();
    rt
}

/// The blits of `cmds` from `file`: (source left, top, width, height, dest left, top).
fn blits(cmds: &[DrawCmd], file: &str) -> Vec<(i32, i32, i32, i32, i32, i32)> {
    cmds.iter()
        .filter_map(|c| match c {
            DrawCmd::Image { bitmap, dst, .. } if &*bitmap.file == file => Some((
                bitmap.x,
                bitmap.y,
                bitmap.w.unwrap_or(0),
                bitmap.h.unwrap_or(0),
                dst.left,
                dst.top,
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn a_classic_skin_gets_a_main_window_of_275_by_116() {
    let mut rt = classic();
    let main = rt.main_window().expect("a main window");
    assert_eq!(rt.window_size(main), (275, 116));
    assert_eq!(rt.window_rect(main), Rect::new(0, 0, 275, 116));
}

#[test]
fn stopped_the_main_window_shows_blank_time_and_no_rates() {
    let mut rt = classic();
    let cmds = rt.draw_main();
    assert_eq!(blits(&cmds, "main.bmp"), vec![(0, 0, 275, 116, 0, 0)]);
    let bar = blits(&cmds, "titlebar.bmp");
    assert_eq!(bar[0], (27, 0, 275, 14, 0, 0), "the active title bar");
    assert!(bar.contains(&(304, 0, 8, 43, 10, 22)), "the clutter bar");
    assert_eq!(bar.len(), 2, "the strip's buttons until one is pressed");
    let time = blits(&cmds, "numbers.bmp");
    assert_eq!(
        time,
        vec![
            (90, 0, 9, 13, 36, 26),
            (9, 6, 5, 1, 40, 32),
            (90, 0, 9, 13, 48, 26),
            (90, 0, 9, 13, 60, 26),
            (90, 0, 9, 13, 78, 26),
            (90, 0, 9, 13, 90, 26),
        ],
        "five blank digits and no minus sign"
    );
    assert_eq!(
        blits(&cmds, "monoster.bmp"),
        vec![(29, 12, 28, 12, 212, 41), (0, 12, 29, 12, 239, 41)],
        "both lights out stopped"
    );
    assert!(
        blits(&cmds, "posbar.bmp").is_empty(),
        "no position bar stopped"
    );
    let state = blits(&cmds, "playpaus.bmp");
    assert_eq!(state, vec![(18, 0, 9, 9, 26, 28), (27, 0, 2, 9, 24, 28)]);
    let buttons = blits(&cmds, "cbuttons.bmp");
    assert_eq!(
        buttons,
        vec![(0, 0, 114, 18, 16, 88), (114, 0, 22, 16, 136, 89)]
    );
}

#[test]
fn the_title_row_spells_the_track_in_the_bitmap_font_across_the_row() {
    let mut rt = classic();
    let cmds = rt.draw_main();
    let text: Vec<_> = blits(&cmds, "text.bmp")
        .into_iter()
        .filter(|b| b.5 == 27)
        .collect();
    let title = rt.player.title();
    let length = rt.player.length_ms() / 1000;
    let expect = format!("1. {title} ({}:{:02})", length / 60, length % 60);
    assert!(
        expect.chars().count() * 5 >= 147,
        "the demo title is long: it scrolls"
    );
    // from its start, cell by cell, 5 wide
    for (i, c) in expect.chars().take(12).enumerate() {
        let (cx, cy) = crate::classic::font::cell(c);
        assert_eq!(
            text[i],
            (cx, cy, 5, 6, 111 + i as i32 * 5, 27),
            "cell {i} is {c:?}"
        );
    }
    // the row is covered from 111 to 269 and no further
    let covered: i32 = text.iter().map(|b| b.2).sum();
    assert_eq!(covered, 269 - 111);
    assert_eq!(text.last().map(|b| b.4 + b.2), Some(269));
}

#[test]
fn playing_shows_the_time_the_rates_the_channels_and_the_position() {
    let mut rt = classic();
    rt.player.play();
    rt.tick();
    let cmds = rt.draw_main();
    let time = blits(&cmds, "numbers.bmp");
    assert_eq!(time[0], (90, 0, 9, 13, 36, 26));
    assert_eq!(time.len(), 5, "a blank and four digits");
    assert_eq!(time[1].4, 48);
    assert_eq!(time[4].4, 90);
    let kbps = rt.player.bitrate_kbps();
    let rates = blits(&cmds, "text.bmp");
    let at = |x: i32| rates.iter().find(|b| b.4 == x && b.5 == 43).copied();
    assert_eq!(at(121), Some(((kbps % 10) * 5, 6, 5, 6, 121, 43)));
    assert!(
        at(156).is_some() && at(161).is_some(),
        "the sample rate's two digits"
    );
    assert_eq!(blits(&cmds, "monoster.bmp").len(), 2);
    let bar = blits(&cmds, "posbar.bmp");
    assert_eq!(bar[0], (0, 0, 248, 10, 16, 72));
    assert_eq!(
        (bar[1].0, bar[1].1, bar[1].2, bar[1].3, bar[1].5),
        (248, 0, 29, 10, 72)
    );
}

/// Every box of `r`, however it is nested.
fn boxes(r: &Region, out: &mut Vec<Rect>) {
    match r {
        Region::Box(b) => out.push(*b),
        Region::Pic(_) => {}
        Region::Op {
            base,
            add,
            keep,
            cut,
        } => {
            if let Some(b) = base {
                boxes(b, out);
            }
            for r in add.iter().chain(keep).chain(cut) {
                boxes(r, out);
            }
        }
    }
}

#[test]
fn region_txt_shapes_the_main_window() {
    let mut rt = classic();
    let cmds = rt.draw_main();
    let Some(DrawCmd::WindowStart(region)) = cmds.first() else {
        panic!("a shaped window first: {:?}", cmds.first())
    };
    let mut rows = Vec::new();
    boxes(region, &mut rows);
    assert_eq!(
        rows[0],
        Rect::new(2, 0, 271, 1),
        "the corners cut on the first row"
    );
    assert_eq!(rows[2], Rect::new(0, 2, 275, 1));
    assert_eq!(rows.len(), 116);
}

#[test]
fn the_equalizer_and_playlist_open_under_the_main_window() {
    let mut rt = classic();
    assert!(rt.open_window("equalizer"));
    assert!(rt.open_window("pledit"));
    let eq = rt.window_of("equalizer").expect("the equalizer");
    let pl = rt.window_of("pledit").expect("the playlist");
    rt.ensure_layout();
    assert_eq!(rt.window_rect(eq), Rect::new(0, 116, 275, 116));
    assert_eq!(rt.window_rect(pl), Rect::new(0, 232, 275, 116));
    let cmds = rt.draw_main();
    let toggles: Vec<_> = blits(&cmds, "shufrep.bmp")
        .into_iter()
        .filter(|b| b.5 == 58)
        .collect();
    assert_eq!(
        toggles,
        vec![(0, 73, 23, 12, 219, 58), (23, 73, 23, 12, 242, 58)],
        "both lit"
    );
}

#[test]
fn the_equalizer_draws_its_strip_buttons_and_flat_sliders() {
    let mut rt = classic();
    assert!(rt.open_window("equalizer"));
    rt.ensure_layout();
    let windows = rt.draw_windows();
    let (_, win) = windows
        .iter()
        .find(|(r, _)| r.top == 116)
        .expect("drawn under the main window");
    let eq_blits = blits(win, "eqmain.bmp");
    eprintln!(
        "preamp {} bands {:?}",
        rt.player.eq_preamp(),
        rt.player.eq_bands()
    );
    eprintln!("eq blits: {eq_blits:?}");
    assert_eq!(eq_blits[0], (0, 0, 275, 116, 0, 0));
    assert!(
        eq_blits.contains(&(0, 149, 275, 14, 0, 0)),
        "the inactive strip"
    );
    assert!(eq_blits.contains(&(10, 119, 25, 12, 14, 18)), "ON out");
    assert!(eq_blits.contains(&(224, 164, 44, 12, 217, 18)), "PRESETS");
    // a flat band (128 of 0..255 is the slider's 31 of 0..63: frame 14) with its thumb mid-way
    assert!(
        eq_blits.contains(&(13, 229, 14, 63, 21, 38)),
        "the preamp's frame"
    );
    assert!(eq_blits.contains(&(0, 164, 11, 11, 22, 63)), "its thumb");
    assert!(
        eq_blits.contains(&(13, 229, 14, 63, 78, 38)),
        "the first band"
    );
}

#[test]
fn the_playlist_frame_at_275_by_116_is_corners_a_title_and_a_bottom_bar() {
    let mut rt = classic();
    assert!(rt.open_window("pledit"));
    rt.ensure_layout();
    let windows = rt.draw_windows();
    let (_, win) = windows
        .iter()
        .find(|(r, _)| r.top == 116)
        .expect("under the main window");
    let pl = blits(win, "pledit.bmp");
    // the title bar: five tiles (125 px) split around the title, the odd one as 12 + 13
    assert_eq!(
        &pl[..9],
        &[
            (0, 21, 25, 20, 0, 0),
            (127, 21, 12, 20, 25, 0),
            (127, 21, 25, 20, 37, 0),
            (127, 21, 25, 20, 62, 0),
            (26, 21, 100, 20, 87, 0),
            (127, 21, 13, 20, 187, 0),
            (127, 21, 25, 20, 200, 0),
            (127, 21, 25, 20, 225, 0),
            (153, 21, 25, 20, 250, 0),
        ]
    );
    // two rows of sides, then the bottom bar's two parts at y 78
    assert_eq!(
        &pl[9..13],
        &[
            (0, 42, 12, 29, 0, 20),
            (31, 42, 5, 29, 255, 20),
            (44, 42, 7, 29, 268, 20),
            (36, 42, 8, 29, 260, 20),
        ],
        "a row of sides with the scrollbar's track"
    );
    assert_eq!(pl[13].5, 49);
    assert_eq!(
        pl[17],
        (52, 53, 8, 18, 260, 20),
        "the scrollbar's thumb at the top"
    );
    assert_eq!(
        &pl[18..],
        &[(0, 72, 125, 38, 0, 78), (126, 72, 150, 38, 125, 78)]
    );
    // the list's background under the frame, from pledit.txt
    // the window's black fill first, then the list's
    let fills: Vec<Rect> = win
        .iter()
        .filter_map(|c| match c {
            DrawCmd::Fill { rect, .. } => Some(*rect),
            _ => None,
        })
        .collect();
    assert_eq!(fills.get(1), Some(&Rect::new(12, 20, 243, 58)));
}

#[test]
fn the_playlist_lists_its_rows_with_the_length_at_the_right() {
    let mut rt = classic();
    assert!(rt.open_window("pledit"));
    rt.ensure_layout();
    let windows = rt.draw_windows();
    let (_, win) = windows
        .iter()
        .find(|(r, _)| r.top == 116)
        .expect("under the main window");
    let texts: Vec<(String, Rect, Align)> = win
        .iter()
        .filter_map(|c| match c {
            DrawCmd::Text(t) => Some((t.text.clone(), t.rect, t.align)),
            _ => None,
        })
        .collect();
    let title = rt.player.title();
    let length = rt.player.length_ms() / 1000;
    assert_eq!(texts.len(), 2);
    assert_eq!(
        texts[0],
        (
            format!("1. {title}"),
            Rect::new(12, 20, 243, 14),
            Align::Left
        )
    );
    assert_eq!(
        texts[1],
        (
            format!("{}:{:02}", length / 60, length % 60),
            Rect::new(12, 20, 243, 14),
            Align::Right
        )
    );
}

#[test]
fn the_playlist_bottom_shows_the_times_in_text_cells() {
    let mut rt = classic();
    assert!(rt.open_window("pledit"));
    rt.ensure_layout();
    let windows = rt.draw_windows();
    let (_, win) = windows
        .iter()
        .find(|(r, _)| r.top == 116)
        .expect("under the main window");
    let cells: Vec<_> = blits(win, "text.bmp");
    let length = rt.player.length_ms() / 1000;
    let line = format!("0:00/{}:{:02}", length / 60, length % 60);
    // 18 cells from (132, 88), the text then blanks
    let top: Vec<_> = cells.iter().filter(|b| b.5 == 88).collect();
    assert_eq!(top.len(), 18);
    for (i, c) in line.chars().enumerate() {
        assert_eq!(
            (top[i].0, top[i].1),
            crate::classic::font::cell(c),
            "cell {i}"
        );
        assert_eq!(top[i].4, 132 + i as i32 * 5);
    }
    assert_eq!((top[17].0, top[17].1), crate::classic::font::cell(' '));
    // the song's time: blanks while stopped, at (189, 101) 4 wide then five 5 wide
    let low: Vec<_> = cells.iter().filter(|b| b.5 == 101).collect();
    assert_eq!(low.len(), 6);
    assert_eq!(low[0], &(142, 0, 4, 6, 189, 101));
    assert_eq!(low[5], &(142, 0, 5, 6, 216, 101));
}
