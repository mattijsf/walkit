// SPDX-License-Identifier: GPL-3.0-or-later
//! A classic skin's main window, drawn pixel for pixel as Winamp draws it (as
//! Winamp 5.666 shows it in the reference VM).

use std::rc::Rc;

use super::QuietHost;
use crate::draw::FontDef;
use crate::draw::{Align, DrawCmd, Rect, Region, Rgb};
use crate::files::MemoryFiles;
use crate::host::{Clock, SkinHost, SkinSetup};
use crate::player::{FakePlayer, Player, Status, Track};
use crate::runtime::SkinRuntime;
use std::cell::RefCell;

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
    let clock: Clock = Rc::new(|| 0);
    classic_with(FakePlayer::new(clock.clone()), clock)
}

fn classic_with(player: FakePlayer, clock: Clock) -> SkinRuntime {
    let mut files = MemoryFiles::new();
    for s in SHEETS {
        files.insert(s, &b"x"[..]);
    }
    files.insert("region.txt", REGION.as_bytes());
    let mut rt = SkinRuntime::new(
        Box::new(files),
        Box::new(QuietHost),
        Box::new(player),
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
fn stopped_a_nums_ex_skin_shows_blank_cells_and_a_dash() {
    let clock: Clock = Rc::new(|| 0);
    let mut files = MemoryFiles::new();
    for s in SHEETS {
        files.insert(s, &b"x"[..]);
    }
    files.insert("nums_ex.bmp", &b"x"[..]);
    files.insert("region.txt", REGION.as_bytes());
    let mut rt = SkinRuntime::new(
        Box::new(files),
        Box::new(QuietHost),
        Box::new(FakePlayer::new(clock.clone())),
        SkinSetup::default(),
        clock,
    );
    rt.load();
    let cmds = rt.draw_main();
    assert!(blits(&cmds, "numbers.bmp").is_empty(), "nums_ex.bmp wins");
    assert_eq!(
        blits(&cmds, "nums_ex.bmp"),
        vec![
            (90, 0, 9, 13, 36, 26),
            (9, 6, 5, 1, 40, 32),
            (90, 0, 9, 13, 48, 26),
            (90, 0, 9, 13, 60, 26),
            (90, 0, 9, 13, 78, 26),
            (90, 0, 9, 13, 90, 26),
        ]
    );
}

#[test]
fn stopped_the_main_window_shows_blank_time_and_no_rates() {
    let mut rt = classic();
    let cmds = rt.draw_main();
    assert_eq!(blits(&cmds, "main.bmp"), vec![(0, 0, 275, 116, 0, 0)]);
    let bar = blits(&cmds, "titlebar.bmp");
    assert_eq!(bar[0], (27, 0, 275, 14, 0, 0), "the active title bar");
    assert!(bar.contains(&(304, 0, 8, 43, 10, 22)), "the clutter bar");
    // the clutter bar's A and D unlit, as Winamp draws them over it
    assert!(bar.contains(&(305, 11, 7, 8, 11, 33)));
    assert!(bar.contains(&(305, 27, 7, 6, 11, 49)));
    assert_eq!(bar.len(), 4, "the strip's own buttons until one is pressed");
    assert!(
        blits(&cmds, "numbers.bmp").is_empty(),
        "a numbers.bmp skin leaves main.bmp's time area stopped, as Winamp's base skin"
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
    // the row is covered from 111 up to 265 and no further: Winamp leaves the box's corner be
    let covered: i32 = text.iter().map(|b| b.2).sum();
    assert_eq!(covered, 265 - 111);
    assert_eq!(text.last().map(|b| b.4 + b.2), Some(265));
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
    let khz = rt.player.sample_rate_khz();
    assert_eq!(at(156), Some(((khz / 10 % 10) * 5, 6, 5, 6, 156, 43)));
    assert_eq!(at(161), Some(((khz % 10) * 5, 6, 5, 6, 161, 43)));
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
    // at this host's 9.6 px a letter, the title ends on "..." 4 px before the length
    assert_eq!(title, "Andamp - Modern skin demo");
    assert_eq!(
        texts[0],
        (
            "1. Andamp - Moder...".to_string(),
            Rect::new(12, 22, 243, 15),
            Align::Left
        )
    );
    assert_eq!(
        texts[1],
        (
            format!("{}:{:02}", length / 60, length % 60),
            Rect::new(12, 22, 243, 15),
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

#[test]
fn the_main_shade_bar_is_the_strip_with_small_time_digits() {
    let mut rt = classic();
    let c = rt.container("main").expect("the main container");
    rt.switch_to(c, "shade");
    rt.ensure_layout();
    let main = rt.main_window().expect("a main window");
    assert_eq!(rt.window_size(main), (275, 14));
    let cmds = rt.draw_main();
    let bar = blits(&cmds, "titlebar.bmp");
    assert_eq!(
        bar,
        vec![(27, 29, 275, 14, 0, 0)],
        "the active shade strip alone while stopped"
    );
    let cells = blits(&cmds, "text.bmp");
    assert_eq!(
        cells,
        vec![
            (142, 0, 3, 6, 126, 4),
            (142, 0, 3, 6, 130, 4),
            (142, 0, 5, 6, 134, 4),
            (142, 0, 5, 6, 139, 4),
            (142, 0, 5, 6, 147, 4),
            (142, 0, 5, 6, 152, 4),
        ],
        "blank time cells"
    );
    rt.player.play();
    rt.tick();
    let cmds = rt.draw_main();
    let bar = blits(&cmds, "titlebar.bmp");
    assert!(
        bar.contains(&(0, 36, 17, 7, 226, 4)),
        "the mini position bar"
    );
    assert!(
        bar.contains(&(17, 36, 3, 7, 227, 4)),
        "its thumb at the start"
    );
}

#[test]
fn the_equalizer_and_playlist_shade_bars() {
    let mut rt = classic();
    assert!(rt.open_window("equalizer"));
    assert!(rt.open_window("pledit"));
    let eq = rt.window_of("equalizer").expect("the equalizer");
    let pl = rt.window_of("pledit").expect("the playlist");
    rt.set_window_layout(eq, "shade");
    rt.set_window_layout(pl, "shade");
    rt.ensure_layout();
    let windows = rt.draw_windows();
    let eq_win = &windows
        .iter()
        .find(|(r, _)| r.top == 116)
        .expect("the equalizer bar")
        .1;
    let eq_blits = blits(eq_win, "eq_ex.bmp");
    assert_eq!(eq_blits[0], (0, 15, 275, 14, 0, 0), "the inactive strip");
    // volume 200: the mark at 61 + 73 from the third look; balance 0 (128): at 164 + 19
    assert_eq!(eq_blits[1], (1 + 2 * 3, 30, 3, 7, 134, 4));
    assert_eq!(eq_blits[2], (11 + 3, 30, 3, 7, 183, 4));
    let pl_win = &windows
        .iter()
        .find(|(r, _)| r.top == 130)
        .expect("the playlist bar under it")
        .1;
    let pl_blits = blits(pl_win, "pledit.bmp");
    // 275 wide: the left piece (active: switched to last), eight tiles, no remainder, the
    // right piece at 225
    assert_eq!(pl_blits[0], (72, 42, 25, 14, 0, 0));
    assert_eq!(pl_blits[1], (72, 57, 25, 14, 25, 0));
    assert_eq!(pl_blits.len(), 10);
    assert_eq!(pl_blits[9], (99, 42, 50, 14, 225, 0));
    // the song's line: "1. TITLE" from x 4, the length's cells ending at 246, blanks between
    let cells = blits(pl_win, "text.bmp");
    let title = rt.player.title();
    let line = format!("1. {title}");
    for (i, c) in line.chars().enumerate() {
        assert_eq!(
            (cells[i].0, cells[i].1, cells[i].4),
            (font_cell(c).0, font_cell(c).1, 4 + i as i32 * 5),
            "cell {i}"
        );
    }
    let length = rt.player.length_ms() / 1000;
    let time = format!("{}:{:02}", length / 60, length % 60);
    // the length starts at the first grid cell at or past 275 - 29 - 20 = 226: 229
    let first = cells
        .iter()
        .find(|b| b.4 == 229)
        .expect("the length's first cell");
    assert_eq!((first.0, first.1), font_cell(time.chars().next().unwrap()));
    assert_eq!(
        cells.last().map(|b| b.4),
        Some(244),
        "its last cell, nothing after at this width"
    );
}

fn font_cell(c: char) -> (i32, i32) {
    crate::classic::font::cell(c)
}

#[test]
fn presses_on_the_main_window_play_toggle_windows_and_shade_it() {
    let mut rt = classic();
    rt.ensure_layout();
    // a press on Play shows it pressed, a release on it plays
    rt.pointer_down(50, 97, 1);
    let cmds = rt.draw_main();
    assert!(
        blits(&cmds, "cbuttons.bmp").contains(&(23, 18, 23, 18, 39, 88)),
        "Play pressed"
    );
    assert!(rt.move_target().is_none(), "a button moves nothing");
    rt.pointer_up(50, 97);
    assert_eq!(rt.player.status(), Status::Playing);
    // the EQ toggle opens the equalizer, pressing it again closes it
    rt.pointer_down(230, 63, 2);
    rt.pointer_up(230, 63);
    let eq = rt.container("equalizer").expect("the equalizer");
    assert!(rt.is_open(eq));
    rt.pointer_down(230, 63, 3);
    rt.pointer_up(230, 63);
    assert!(!rt.is_open(eq));
    // a press on the title bar is what moves the window
    rt.pointer_down(100, 7, 4);
    assert!(rt.move_target().is_some());
    rt.pointer_up(100, 7);
    // the shade button switches to the bar, the bar's button back
    rt.pointer_down(258, 7, 5);
    rt.pointer_up(258, 7);
    rt.ensure_layout();
    let main = rt.main_window().expect("the main window");
    assert_eq!(rt.window_size(main), (275, 14));
    rt.pointer_down(258, 7, 6);
    rt.pointer_up(258, 7);
    rt.ensure_layout();
    let main = rt.main_window().expect("the main window");
    assert_eq!(rt.window_size(main), (275, 116));
    // a double tap on the title bar shades it too
    rt.double_tap(100, 7, 7);
    rt.ensure_layout();
    let main = rt.main_window().expect("the main window");
    assert_eq!(rt.window_size(main), (275, 14));
}

#[test]
fn the_volume_and_a_band_follow_the_finger() {
    let mut rt = classic();
    rt.ensure_layout();
    rt.pointer_down(114, 63, 1);
    assert_eq!(
        rt.player.volume(),
        0,
        "the thumb's left end at the slider's start"
    );
    let cmds = rt.draw_main();
    assert!(
        blits(&cmds, "volume.bmp").contains(&(0, 422, 14, 11, 107, 58)),
        "the thumb pressed"
    );
    rt.pointer_move(165, 63);
    assert_eq!(
        rt.player.volume(),
        255,
        "the thumb's middle at its last spot"
    );
    rt.pointer_move(168, 63);
    assert_eq!(rt.player.volume(), 255);
    rt.pointer_up(168, 63);
    assert!(rt.open_window("equalizer"));
    rt.ensure_layout();
    // the first band at x 78 under the main window, taken beside its thumb: the band's step
    // is (y - 5 - 39) * 64 / 52, 0 at y 44 and up, 63 at y 96 and down
    rt.pointer_down(85, 116 + 43, 2);
    assert_eq!(rt.player.eq_bands()[0], 255, "the top is full boost");
    rt.pointer_move(85, 116 + 96);
    assert_eq!(rt.player.eq_bands()[0], 0, "the bottom is full cut");
    rt.pointer_up(85, 116 + 96);
    assert_eq!(rt.player.eq_bands()[0], 0, "the release keeps the value");
}

#[test]
fn a_press_on_the_time_shows_the_time_left() {
    let mut rt = classic();
    rt.ensure_layout();
    rt.player.play();
    rt.pointer_down(60, 30, 1);
    rt.pointer_up(60, 30);
    let cmds = rt.draw_main();
    let nums = blits(&cmds, "numbers.bmp");
    assert!(
        nums.contains(&(20, 6, 5, 1, 38, 32)),
        "the minus sign: {nums:?}"
    );
    rt.pointer_down(96, 39, 2);
    rt.pointer_up(96, 39);
    let cmds = rt.draw_main();
    assert!(!blits(&cmds, "numbers.bmp").contains(&(20, 6, 5, 1, 38, 32)));
    rt.pointer_down(97, 32, 3);
    rt.pointer_up(97, 32);
    rt.pointer_down(60, 25, 4);
    rt.pointer_up(60, 25);
    let cmds = rt.draw_main();
    assert!(
        !blits(&cmds, "numbers.bmp").contains(&(20, 6, 5, 1, 38, 32)),
        "presses just outside the time do nothing"
    );
}

#[test]
fn a_press_on_a_playlist_row_selects_it_and_a_double_tap_plays_it() {
    let clock: Clock = Rc::new(|| 0);
    let mut player = FakePlayer::new(clock.clone());
    player.playlist = vec![
        Track {
            title: "One".into(),
            length_ms: 60_000,
        },
        Track {
            title: "Two".into(),
            length_ms: 60_000,
        },
    ];
    let mut rt = classic_with(player, clock);
    assert!(rt.open_window("pledit"));
    rt.ensure_layout();
    // the playlist under the main window; its second row at y 34..48 of it
    rt.pointer_down(60, 116 + 40, 1);
    rt.pointer_up(60, 116 + 40);
    let windows = rt.draw_windows();
    let (_, cmds) = windows
        .iter()
        .find(|(r, _)| r.top == 116)
        .expect("the playlist window under the main one");
    let selected = cmds.iter().any(|c| match c {
        DrawCmd::Fill { rect, .. } => rect.top == 37 && rect.height == 15,
        _ => false,
    });
    assert!(selected, "the row's background");
    rt.double_tap(60, 116 + 40, 2);
    assert_eq!(rt.player.track_index(), 1);
    assert_eq!(rt.player.status(), Status::Playing);
}

#[test]
fn the_grip_at_the_playlist_corner_sizes_it() {
    let mut rt = classic();
    assert!(rt.open_window("pledit"));
    rt.ensure_layout();
    rt.pointer_down(270, 116 + 111, 1);
    rt.pointer_move(290, 116 + 141);
    rt.pointer_up(290, 116 + 141);
    rt.ensure_layout();
    let pl = rt.container("pledit").expect("the playlist");
    let l = rt.container_layout(pl, "normal").expect("its layout");
    let size = rt.layout_size(l);
    assert_eq!(
        size,
        (300, 174),
        "grown by the drag, rounded up to 25 by 29 steps"
    );
}

#[test]
fn stacked_to_fill_the_view_the_playlist_grip_sizes_nothing() {
    let mut rt = classic();
    assert!(rt.open_window("pledit"));
    rt.set_presentation(&crate::host::Presentation {
        view_size: Some((275, 400)),
        stretch_last: true,
        ..rt.presentation()
    });
    assert_eq!(pl_drag(&mut rt, (270, 280), (320, 300)), (275, 284));
}

#[test]
fn stacked_to_fill_the_view_a_playlist_sized_apart_fills_it_all_the_same() {
    let mut rt = classic();
    assert!(rt.open_window("pledit"));
    assert_eq!(pl_drag(&mut rt, (270, 111), (296, 112)), (325, 145));
    rt.set_presentation(&crate::host::Presentation {
        view_size: Some((275, 400)),
        stretch_last: true,
        ..rt.presentation()
    });
    rt.ensure_layout();
    let pl = rt.window_rect(rt.window_of("pledit").expect("its window"));
    assert_eq!((pl.width, pl.top + pl.height), (275, 400));
}

/// The playlist's size after a drag from `from` to `to`, points of its window.
fn pl_drag(rt: &mut SkinRuntime, from: (i32, i32), to: (i32, i32)) -> (i32, i32) {
    rt.ensure_layout();
    let pl = rt.container("pledit").expect("the playlist");
    let l = rt.container_layout(pl, "normal").expect("its layout");
    let top = rt
        .window_rect(rt.window_of("pledit").expect("its window"))
        .top;
    rt.pointer_down(from.0, top + from.1, 1);
    rt.pointer_move(to.0, top + to.1);
    rt.pointer_up(to.0, top + to.1);
    rt.ensure_layout();
    rt.layout_size(l)
}

#[test]
fn the_playlist_sizes_in_winamps_steps() {
    // the drags of the reference VM (Winamp 5.666, base skin), points in the playlist window
    let mut rt = classic();
    assert!(rt.open_window("pledit"));
    assert_eq!(
        pl_drag(&mut rt, (270, 111), (271, 111)),
        (300, 116),
        "one pixel wider is a step"
    );
    assert_eq!(pl_drag(&mut rt, (295, 111), (296, 112)), (325, 145));
    assert_eq!(
        pl_drag(&mut rt, (320, 140), (270, 94)),
        (275, 116),
        "not under 275 by 116"
    );
}

#[test]
fn the_playlist_grip_is_the_corners_triangle() {
    let mut rt = classic();
    assert!(rt.open_window("pledit"));
    // 19 from the right and 10 from the bottom: 29, inside
    assert_eq!(pl_drag(&mut rt, (256, 106), (281, 106)), (300, 116));
    // 19 and 12: 31, outside; the press lands on the window
    assert_eq!(pl_drag(&mut rt, (281, 104), (306, 104)), (300, 116));
}

#[test]
fn the_playlist_shade_bar_sizes_its_width() {
    let mut rt = classic();
    assert!(rt.open_window("pledit"));
    rt.ensure_layout();
    let pl = rt.container("pledit").expect("the playlist");
    rt.switch_to(pl, "shade");
    rt.ensure_layout();
    let shade = rt.container_layout(pl, "shade").expect("its shade");
    let top = rt
        .window_rect(rt.window_of("pledit").expect("its window"))
        .top;
    // x 247..254 of the bar (w-28..w-21) is the grip
    rt.pointer_down(250, top + 7, 1);
    rt.pointer_move(260, top + 9);
    rt.pointer_up(260, top + 9);
    rt.ensure_layout();
    assert_eq!(rt.layout_size(shade), (300, 14));
}

#[test]
fn a_press_on_the_vis_area_switches_to_the_oscilloscope() {
    use crate::classic::vis::{scope, ScopeStyle, HEIGHT, WIDTH};
    // a quarter-scale sine: rows 6..13 around the middle, as the VM showed for the same tone
    let wave: Vec<f32> = (0..576)
        .map(|i| 0.244 * (i as f32 / 576.0 * std::f32::consts::TAU).sin())
        .collect();
    let px = scope(&wave, ScopeStyle::Lines);
    let rows_used: Vec<usize> = (0..HEIGHT)
        .filter(|y| (0..WIDTH).any(|x| px[y * WIDTH + x] >= 18))
        .collect();
    assert_eq!(rows_used.first(), Some(&6));
    assert_eq!(rows_used.last(), Some(&13));
    // the middle rows in the first scope color, the far rows in the third
    assert_eq!(
        px[9 * WIDTH],
        18,
        "the first sample sits on the middle line"
    );
    assert!((0..WIDTH).any(|x| px[13 * WIDTH + x] == 20));
    // the first column is the sample alone; a later one runs from the sample before
    assert_eq!((0..HEIGHT).filter(|y| px[y * WIDTH] >= 18).count(), 1);
    // silence shows the grid alone: the odd rows' even columns
    let quiet = scope(&[0.0; 576], ScopeStyle::Lines);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            assert_eq!(quiet[y * WIDTH + x], u8::from(y % 2 == 1 && x % 2 == 0));
        }
    }
    // the press cycles the mode
    let mut rt = classic();
    rt.ensure_layout();
    rt.player.play();
    rt.pointer_down(60, 40, 1);
    rt.pointer_up(60, 40);
    assert_eq!(rt.classic_vis_mode, crate::classic::vis::VisMode::Scope);
    rt.pointer_down(60, 39, 9);
    rt.pointer_up(60, 39);
    rt.pointer_down(60, 62, 9);
    rt.pointer_up(60, 62);
    assert_eq!(
        rt.classic_vis_mode,
        crate::classic::vis::VisMode::Scope,
        "a press just outside the rect does nothing"
    );
    rt.pointer_down(60, 50, 2);
    rt.pointer_up(60, 50);
    assert_eq!(rt.classic_vis_mode, crate::classic::vis::VisMode::Off);
    // off, the area shows the palette's background while something plays
    let cmds = rt.draw_main();
    let area = cmds.iter().find_map(|c| match c {
        DrawCmd::Pixels { argb, .. } => Some(argb.clone()),
        _ => None,
    });
    let background = 0xff00_0000 | rt.classic_palette[0].0;
    assert!(
        area.is_some_and(|px| px.iter().all(|p| *p == background)),
        "the background alone while off"
    );
}

#[test]
fn the_shade_bar_shows_a_mini_analyzer_and_oscilloscope() {
    use crate::classic::vis::{scope_shade, ClassicVis, ScopeStyle, VisStyle, SHADE_WIDTH};
    let mut vis = ClassicVis::default();
    // every band full: thick bars of three columns, five rows in colors 17 down to 5, no peak
    // row left to show; the fourth column of each group and the 38th column empty
    let px = vis.frame_shade(0, &[1.0; 75], &VisStyle::default());
    let col = |x: usize| (0..5).map(|y| px[y * SHADE_WIDTH + x]).collect::<Vec<_>>();
    assert_eq!(col(0), vec![5, 8, 11, 14, 17]);
    assert_eq!(col(2), vec![5, 8, 11, 14, 17]);
    assert_eq!(col(3), vec![0; 5]);
    // the last group averages three bands and nothing for the fourth: three rows, the peak over
    assert_eq!(col(36), vec![0, 23, 11, 14, 17]);
    assert_eq!(col(37), vec![0; 5]);
    // silence after: the bars fall off and the peak marks show in color 23
    let mut vis = ClassicVis::default();
    let _ = vis.frame_shade(0, &[0.6; 75], &VisStyle::default());
    let px = vis.frame_shade(25, &[0.0; 75], &VisStyle::default());
    assert!(
        (0..5).any(|y| px[y * SHADE_WIDTH] == 23),
        "a peak: {:?}",
        col(0)
    );
    // a quarter-scale sine on the five rows: the strip shows the window's first half, the
    // sine's rise, on the middle row and the one above
    let wave: Vec<f32> = (0..576)
        .map(|i| 0.244 * (i as f32 / 576.0 * std::f32::consts::TAU).sin())
        .collect();
    let px = scope_shade(&wave, ScopeStyle::Lines);
    let rows: Vec<usize> = (0..5)
        .filter(|y| (0..SHADE_WIDTH).any(|x| px[y * SHADE_WIDTH + x] == 18))
        .collect();
    assert_eq!(rows, vec![1, 2]);
    assert!(px.iter().all(|p| *p == 0 || *p == 18));
    // the strip is drawn on the bar while playing
    let mut rt = classic();
    let c = rt.container("main").expect("the main container");
    rt.switch_to(c, "shade");
    rt.ensure_layout();
    rt.player.play();
    let cmds = rt.draw_main();
    let strip = cmds.iter().find_map(|c| match c {
        DrawCmd::Pixels { rect, .. } => Some(*rect),
        _ => None,
    });
    assert_eq!(strip, Some(Rect::new(79, 5, 38, 5)));
}

#[test]
fn the_playlist_scrolls_by_a_drag_on_its_rows_and_by_its_thumb() {
    let clock: Clock = Rc::new(|| 0);
    let mut player = FakePlayer::new(clock.clone());
    player.playlist = (0..20)
        .map(|i| Track {
            title: format!("Song {i}"),
            length_ms: 60_000,
        })
        .collect();
    let mut rt = classic_with(player, clock);
    assert!(rt.open_window("pledit"));
    rt.ensure_layout();
    // 116 tall: three rows show (as in the VM), seventeen lie past the window; the thumb starts
    // at the top
    assert_eq!(rt.classic_pl_rows(116), 3);
    assert_eq!(rt.classic_pl_thumb(116, 20, 0), 0);
    assert_eq!(rt.classic_pl_thumb(116, 20, 17), 40);
    assert_eq!(rt.classic_pl_thumb(116, 20, 8), 18);
    // a drag up on the rows by two rows scrolls two rows; the release picks nothing
    rt.pointer_down(60, 116 + 60, 1);
    rt.pointer_move(60, 116 + 30);
    rt.pointer_up(60, 116 + 30);
    assert_eq!(rt.classic_pl_scroll, 2);
    assert_eq!(rt.classic_selected_row, None);
    // the rows drawn start at the third song, the thumb a quarter of the way
    let windows = rt.draw_windows();
    let (_, cmds) = windows
        .iter()
        .find(|(r, _)| r.top == 116)
        .expect("the playlist window");
    let texts: Vec<String> = cmds
        .iter()
        .filter_map(|c| match c {
            DrawCmd::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(texts.first().map(String::as_str), Some("3. Song 2"));
    assert_eq!(texts.len(), 6, "three rows, each a title and a length");
    assert!(blits(cmds, "pledit.bmp").contains(&(52, 53, 8, 18, 260, 24)));
    // a tap on the second row picks the fourth song, whose length leads the bottom line
    rt.pointer_down(60, 116 + 40, 2);
    rt.pointer_up(60, 116 + 40);
    assert_eq!(rt.classic_selected_row, Some(3));
    let windows = rt.draw_windows();
    let (_, cmds) = windows
        .iter()
        .find(|(r, _)| r.top == 116)
        .expect("the playlist");
    let line: String = blits(cmds, "text.bmp")
        .iter()
        .filter(|b| b.5 == 116 - 28 && b.4 >= 275 - 143)
        .map(|b| {
            "0123456789:/ "
                .chars()
                .find(|c| crate::classic::font::cell(*c) == (b.0, b.1))
                .unwrap_or('?')
        })
        .collect();
    assert_eq!(line.trim_end(), "1:00/20:00");
    // a double tap plays it
    rt.double_tap(60, 116 + 40, 3);
    assert_eq!(rt.player.track_index(), 3);
    // the track answers to x 268, not at 259
    let before = rt.classic_pl_scroll;
    rt.pointer_down(259, 116 + 60, 8);
    rt.pointer_up(259, 116 + 60);
    assert_eq!(
        rt.classic_pl_scroll, before,
        "x 259 lies between the rows and the track"
    );
    rt.pointer_down(268, 116 + 60, 9);
    rt.pointer_up(268, 116 + 60);
    assert!(
        rt.classic_pl_scroll > before,
        "x 268 is the track's last column"
    );
    rt.classic_pl_scroll = before;
    // the thumb dragged to the track's end shows the last rows
    rt.pointer_down(262, 116 + 30, 4);
    let windows = rt.draw_windows();
    let (_, cmds) = windows
        .iter()
        .find(|(r, _)| r.top == 116)
        .expect("the playlist");
    assert!(
        blits(cmds, "pledit.bmp")
            .iter()
            .any(|b| b.0 == 61 && b.1 == 53),
        "held"
    );
    rt.pointer_move(262, 116 + 70);
    rt.pointer_up(262, 116 + 70);
    assert_eq!(rt.classic_pl_scroll, 17);
}

#[test]
fn the_equalizer_buttons_light_and_press() {
    let mut rt = classic();
    assert!(rt.open_window("equalizer"));
    rt.ensure_layout();
    let eq_cmds = |rt: &mut SkinRuntime| {
        let windows = rt.draw_windows();
        let (_, cmds) = windows
            .into_iter()
            .find(|(r, _)| r.top == 116)
            .expect("the eq");
        blits(&cmds, "eqmain.bmp")
    };
    // ON lights on release, AUTO too; each shows pressed from the sheet's right half while held
    rt.pointer_down(26, 116 + 24, 1);
    assert!(
        eq_cmds(&mut rt).contains(&(128, 119, 25, 12, 14, 18)),
        "ON pressed"
    );
    rt.pointer_up(26, 116 + 24);
    assert!(rt.player.eq_enabled());
    assert!(
        eq_cmds(&mut rt).contains(&(69, 119, 25, 12, 14, 18)),
        "ON lit"
    );
    rt.pointer_down(55, 116 + 24, 2);
    assert!(
        eq_cmds(&mut rt).contains(&(153, 119, 33, 12, 39, 18)),
        "AUTO pressed"
    );
    rt.pointer_up(55, 116 + 24);
    assert!(
        eq_cmds(&mut rt).contains(&(94, 119, 33, 12, 39, 18)),
        "AUTO lit"
    );
    rt.pointer_down(55, 116 + 24, 3);
    assert!(
        eq_cmds(&mut rt).contains(&(212, 119, 33, 12, 39, 18)),
        "AUTO lit and pressed"
    );
    rt.pointer_up(55, 116 + 24);
    assert!(
        eq_cmds(&mut rt).contains(&(35, 119, 33, 12, 39, 18)),
        "AUTO out"
    );
}

#[test]
fn the_title_scrolls_a_cell_every_step_and_the_time_blinks_while_paused() {
    use crate::classic::main::{PAUSE_BLINK_MS, TITLE_STEP_MS};
    let now = Rc::new(std::cell::Cell::new(0i64));
    let at = now.clone();
    let clock: Clock = Rc::new(move || at.get());
    let mut player = FakePlayer::new(clock.clone());
    Player::set_track(
        &mut player,
        "A rather long song title that scrolls across the window",
        600_000,
    );
    let mut rt = classic_with(player, clock);
    rt.ensure_layout();
    assert!(rt.classic_title_scrolls());
    // the first cell of the row is the title's "1" at step 0, the "." at step 1 (5 px on)
    let first_cell = |rt: &mut SkinRuntime| {
        let cmds = rt.draw_main();
        blits(&cmds, "text.bmp")
            .into_iter()
            .find(|b| b.4 == 111 && b.5 == 27)
            .map(|b| (b.0, b.1))
    };
    assert_eq!(first_cell(&mut rt), Some(crate::classic::font::cell('1')));
    assert_eq!(rt.next_due_at(), Some(TITLE_STEP_MS));
    now.set(TITLE_STEP_MS);
    assert_eq!(first_cell(&mut rt), Some(crate::classic::font::cell('.')));
    now.set(TITLE_STEP_MS * 2);
    assert_eq!(first_cell(&mut rt), Some(crate::classic::font::cell(' ')));
    // paused, the digits show in one turn and go dark in the next
    rt.player.play();
    rt.player.pause();
    now.set(0);
    let digits = |rt: &mut SkinRuntime| {
        let cmds = rt.draw_main();
        blits(&cmds, "numbers.bmp")
            .into_iter()
            .filter(|b| b.0 != 90 && b.3 == 13)
            .count()
    };
    assert_eq!(digits(&mut rt), 4, "minutes and seconds lit");
    assert_eq!(rt.next_due_at(), Some(TITLE_STEP_MS));
    now.set(PAUSE_BLINK_MS);
    assert_eq!(digits(&mut rt), 0, "dark");
    now.set(PAUSE_BLINK_MS * 2);
    assert_eq!(digits(&mut rt), 4, "lit again");
    // a short title does not scroll, and playing, the time stays lit
    rt.player.set_track("Short", 600_000);
    rt.player.play();
    now.set(PAUSE_BLINK_MS);
    assert!(!rt.classic_title_scrolls());
    assert_eq!(digits(&mut rt), 4);
    assert_eq!(
        rt.next_due_at(),
        Some(PAUSE_BLINK_MS + 25),
        "playing: the next frame"
    );
}

#[test]
fn the_vis_styles_color_bars_and_lines_as_winamp_options_do() {
    use crate::classic::vis::{
        scope, AnalyzerStyle, ClassicVis, ScopeStyle, VisStyle, HEIGHT, WIDTH,
    };
    let column = |px: &[u8; WIDTH * HEIGHT], x: usize| -> Vec<u8> {
        (0..HEIGHT).map(|y| px[y * WIDTH + x]).collect()
    };
    // fire: a bar of 8 shown rows (10 less the two under the picture) burns from color 3 at
    // its top down to 10 at the picture's bottom row; line: one color, 17 less the height
    let levels: Vec<f32> = (0..75).map(|_| 10.0 / 15.0).collect();
    let fire = VisStyle {
        analyzer: AnalyzerStyle::Fire,
        ..VisStyle::default()
    };
    let px = ClassicVis::default().frame(0, &levels, &fire);
    assert_eq!(&column(&px, 0)[8..], &[3, 4, 5, 6, 7, 8, 9, 10]);
    let line = VisStyle {
        analyzer: AnalyzerStyle::Line,
        ..VisStyle::default()
    };
    let px = ClassicVis::default().frame(0, &levels, &line);
    assert_eq!(&column(&px, 0)[8..], &[7; 8]);
    // thin bars: every column, its own band; no peaks when they are off
    let mut uneven = vec![0.0f32; 75];
    uneven[3] = 1.0;
    let thin = VisStyle {
        thin_bars: true,
        peaks: false,
        ..VisStyle::default()
    };
    let px = ClassicVis::default().frame(0, &uneven, &thin);
    // a full bar reaches row 3 (its lowest two rows lie under the picture)
    assert_eq!(
        &column(&px, 3)[3..],
        &[3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
    );
    assert!(!px.contains(&23), "no peak marks");
    let px = ClassicVis::default().frame(0, &uneven, &VisStyle::default());
    assert_eq!(
        column(&px, 3),
        vec![0; 16],
        "thick: the fourth column stays empty"
    );
    // dots: one pixel per column; solid: a run to the middle
    let wave: Vec<f32> = (0..576)
        .map(|i| 0.244 * (i as f32 / 576.0 * std::f32::consts::TAU).sin())
        .collect();
    let dots = scope(&wave, ScopeStyle::Dots);
    for x in 0..75 {
        assert_eq!(column(&dots, x).iter().filter(|p| **p >= 18).count(), 1);
    }
    let solid = scope(&wave, ScopeStyle::Solid);
    let top = column(&solid, 18);
    let lit: Vec<usize> = (0..HEIGHT).filter(|y| top[*y] >= 18).collect();
    assert_eq!(lit.first(), Some(&6), "the sine's crest");
    assert_eq!(lit.last(), Some(&9), "down to the middle value 8's row");
}

/// A host whose always on top switch is on, for the clutter bar's A.
struct OnTopHost;

impl SkinHost for OnTopHost {
    fn image_size(&mut self, _file: &str) -> Option<(i32, i32)> {
        None
    }

    fn text_width(&mut self, text: &str, _font: Option<&FontDef>, size: i32, _bold: bool) -> i32 {
        text.chars().count() as i32 * size * 6 / 10
    }

    fn log(&mut self, _message: &str) {}

    fn invalidate(&mut self) {}

    fn window_open(&mut self, kind: &str) -> Option<bool> {
        (kind == "ontop").then_some(true)
    }
}

#[test]
fn the_clutter_bar_shows_a_held_letter_and_lights_a_while_always_on_top() {
    let clock: Clock = Rc::new(|| 0);
    let mut files = MemoryFiles::new();
    for s in SHEETS {
        files.insert(s, &b"x"[..]);
    }
    let mut rt = SkinRuntime::new(
        Box::new(files),
        Box::new(OnTopHost),
        Box::new(FakePlayer::new(clock.clone())),
        SkinSetup::default(),
        clock,
    );
    rt.load();
    rt.ensure_layout();
    let bar = |rt: &mut SkinRuntime| -> Vec<(i32, i32, i32, i32, i32, i32)> {
        let cmds = rt.draw_main();
        blits(&cmds, "titlebar.bmp")
            .into_iter()
            .filter(|b| b.4 == 10 || b.4 == 11)
            .collect()
    };
    assert_eq!(
        bar(&mut rt),
        vec![
            (304, 0, 8, 43, 10, 22),
            (313, 55, 7, 8, 11, 33),
            (305, 27, 7, 6, 11, 49)
        ],
        "at rest: A lit, D not"
    );
    rt.pointer_down(15, 44, 1);
    assert_eq!(
        bar(&mut rt),
        vec![
            (320, 44, 8, 43, 10, 22),
            (313, 55, 7, 8, 11, 33),
            (305, 27, 7, 6, 11, 49)
        ],
        "I held"
    );
    rt.pointer_up(15, 44);
    rt.pointer_down(15, 36, 2);
    assert_eq!(
        bar(&mut rt),
        vec![(312, 44, 8, 43, 10, 22), (305, 27, 7, 6, 11, 49)],
        "A held: its column, no lit A over it"
    );
    rt.pointer_up(15, 36);
}

#[test]
fn shuffle_and_repeat_show_off_pressed_on_and_on_pressed_from_the_top_of_their_sheet() {
    let mut rt = classic();
    rt.ensure_layout();
    let rows = |rt: &mut SkinRuntime| -> Vec<i32> {
        let cmds = rt.draw_main();
        blits(&cmds, "shufrep.bmp")
            .into_iter()
            .filter(|b| b.5 == 89)
            .map(|b| b.1)
            .collect()
    };
    assert_eq!(rows(&mut rt), vec![0, 0], "both off");
    rt.pointer_down(225, 96, 1);
    assert_eq!(rows(&mut rt), vec![0, 15], "repeat held, off");
    rt.pointer_up(225, 96);
    assert_eq!(rows(&mut rt), vec![0, 30], "repeat on");
    rt.pointer_down(225, 96, 2);
    assert_eq!(rows(&mut rt), vec![0, 45], "repeat held, on");
    rt.pointer_up(225, 96);
    rt.pointer_down(185, 96, 3);
    rt.pointer_up(185, 96);
    assert_eq!(rows(&mut rt), vec![30, 0], "shuffle on, repeat off");
}

#[test]
fn shuffle_and_repeat_take_presses_on_their_probed_boxes() {
    let mut rt = classic();
    rt.ensure_layout();
    let tap = |rt: &mut SkinRuntime, x: i32, y: i32, seq: i32| {
        rt.pointer_down(x, y, seq);
        rt.pointer_up(x, y);
    };
    tap(&mut rt, 164, 96, 1);
    assert!(!rt.player.shuffle(), "x 164 lies outside shuffle");
    tap(&mut rt, 165, 96, 2);
    assert!(rt.player.shuffle());
    tap(&mut rt, 185, 89, 3);
    assert!(rt.player.shuffle(), "y 89 lies outside shuffle");
    tap(&mut rt, 209, 102, 4);
    assert!(!rt.player.shuffle());
    tap(&mut rt, 239, 102, 5);
    assert!(rt.player.repeat(), "repeat reaches x 239, y 102");
    tap(&mut rt, 240, 96, 6);
    assert!(rt.player.repeat(), "x 240 lies outside repeat");
}

#[test]
fn the_control_buttons_and_toggles_take_presses_on_their_probed_boxes() {
    let mut rt = classic();
    rt.ensure_layout();
    let tap = |rt: &mut SkinRuntime, x: i32, y: i32, seq: i32| {
        rt.pointer_down(x, y, seq);
        rt.pointer_up(x, y);
    };
    // play from its box's corners, nothing from a row above or below
    tap(&mut rt, 38, 87, 1);
    assert_eq!(rt.player.status(), Status::Playing);
    tap(&mut rt, 84, 106, 2);
    assert_eq!(
        rt.player.status(),
        Status::Stopped,
        "x 84 is stop's first column"
    );
    tap(&mut rt, 60, 86, 3);
    assert_eq!(
        rt.player.status(),
        Status::Stopped,
        "y 86 lies above the buttons"
    );
    tap(&mut rt, 59, 106, 4);
    assert_eq!(
        rt.player.status(),
        Status::Playing,
        "x 59 is play's last column"
    );
    tap(&mut rt, 83, 107, 5);
    assert_eq!(
        rt.player.status(),
        Status::Playing,
        "y 107 lies below the buttons"
    );
    tap(&mut rt, 83, 97, 6);
    assert_eq!(
        rt.player.status(),
        Status::Paused,
        "x 83 is pause's last column"
    );
    // the toggles: EQ to x 242, PL from 243, both to y 70
    tap(&mut rt, 242, 70, 7);
    let eq = rt.container("equalizer").expect("the equalizer");
    let pl = rt.container("pledit").expect("the playlist");
    assert!(rt.is_open(eq));
    assert!(!rt.is_open(pl));
    tap(&mut rt, 243, 58, 8);
    assert!(rt.is_open(pl));
    tap(&mut rt, 253, 71, 9);
    assert!(rt.is_open(pl), "y 71 lies below the toggles");
}

#[test]
fn the_equalizer_controls_take_presses_on_their_probed_boxes() {
    let mut rt = classic();
    assert!(rt.open_window("equalizer"));
    rt.ensure_layout();
    let tap = |rt: &mut SkinRuntime, x: i32, y: i32, seq: i32| {
        rt.pointer_down(x, y, seq);
        rt.pointer_up(x, y);
    };
    tap(&mut rt, 14, 116 + 30, 1);
    assert!(rt.player.eq_enabled(), "ON reaches y 30");
    tap(&mut rt, 39, 116 + 18, 2);
    assert!(rt.classic_eq_auto, "AUTO starts at x 39");
    tap(&mut rt, 72, 116 + 30, 3);
    assert!(!rt.classic_eq_auto, "AUTO reaches x 72, y 30");
    tap(&mut rt, 26, 116 + 31, 4);
    assert!(rt.player.eq_enabled(), "y 31 lies below ON");
    // the first band's box ends at x 90; 91 is dead
    let flat = rt.player.eq_bands()[0];
    tap(&mut rt, 91, 116 + 50, 5);
    assert_eq!(rt.player.eq_bands()[0], flat);
    tap(&mut rt, 90, 116 + 50, 6);
    assert_ne!(rt.player.eq_bands()[0], flat);
}

#[test]
fn the_title_buttons_take_presses_on_their_probed_boxes() {
    let mut rt = classic();
    assert!(rt.open_window("pledit"));
    assert!(rt.open_window("equalizer"));
    rt.ensure_layout();
    let tap = |rt: &mut SkinRuntime, x: i32, y: i32, seq: i32| {
        rt.pointer_down(x, y, seq);
        rt.pointer_up(x, y);
    };
    let pl = rt.container("pledit").expect("the playlist");
    let eq = rt.container("equalizer").expect("the equalizer");
    // the playlist sits at y 116, the equalizer under it at 232
    tap(&mut rt, 264, 116 + 12, 1);
    rt.ensure_layout();
    assert_eq!(
        rt.window_size(rt.window_id_of(rt.container_layout(pl, "shade").unwrap()))
            .1,
        14,
        "x 264, y 12 shades the playlist"
    );
    tap(&mut rt, 255, 116 + 3, 2);
    rt.ensure_layout();
    assert!(rt.is_open(pl));
    assert_eq!(
        rt.containers[pl].current,
        rt.container_layout(pl, "normal"),
        "x 255 shades it back"
    );
    tap(&mut rt, 254, 116 + 7, 3);
    rt.ensure_layout();
    assert_eq!(
        rt.containers[pl].current,
        rt.container_layout(pl, "normal"),
        "x 254 is dead on the playlist"
    );
    tap(&mut rt, 265, 116 + 7, 4);
    assert!(!rt.is_open(pl), "x 265 closes the playlist");
    rt.ensure_layout();
    // the equalizer now sits at y 116: its shade box starts at 253, its close at 264
    tap(&mut rt, 253, 116 + 12, 5);
    rt.ensure_layout();
    assert_eq!(rt.containers[eq].current, rt.container_layout(eq, "shade"));
    tap(&mut rt, 263, 116 + 3, 6);
    rt.ensure_layout();
    assert_eq!(rt.containers[eq].current, rt.container_layout(eq, "normal"));
    tap(&mut rt, 273, 116 + 12, 7);
    assert!(!rt.is_open(eq), "x 273, y 12 closes the equalizer");
}

#[test]
fn the_sliders_take_presses_on_their_probed_rows() {
    let mut rt = classic();
    rt.ensure_layout();
    rt.player.set_volume(128);
    let tap = |rt: &mut SkinRuntime, x: i32, y: i32, seq: i32| {
        rt.pointer_down(x, y, seq);
        rt.pointer_up(x, y);
    };
    tap(&mut rt, 120, 57, 1);
    assert_eq!(rt.player.volume(), 128, "y 57 lies above the volume");
    tap(&mut rt, 120, 58, 2);
    assert_eq!(rt.player.volume(), 30);
    tap(&mut rt, 175, 66, 3);
    assert_eq!(
        rt.player.volume(),
        255,
        "x 175, y 66 is the volume's far corner"
    );
    tap(&mut rt, 140, 67, 4);
    assert_eq!(rt.player.volume(), 255, "y 67 lies below the volume");
    tap(&mut rt, 177, 66, 5);
    assert_eq!(rt.player.pan(), -127);
    tap(&mut rt, 216, 63, 6);
    assert_eq!(rt.player.pan(), -127, "x 216 lies past the balance");
    rt.player.play();
    tap(&mut rt, 140, 72, 7);
    assert_eq!(
        rt.player.position_ms(),
        0,
        "y 72 lies above the position bar"
    );
    tap(&mut rt, 140, 73, 8);
    assert!(rt.player.position_ms() > 0, "y 73 seeks");
    tap(&mut rt, 17, 76, 9);
    assert!(
        rt.player.position_ms() > 0,
        "x 17 lies before the position bar"
    );
    tap(&mut rt, 18, 76, 10);
    assert_eq!(rt.player.position_ms(), 0, "x 18 seeks to the start");
}

#[test]
fn the_shade_bars_take_presses_on_their_probed_boxes() {
    let mut rt = classic();
    let c = rt.container("main").expect("the main container");
    rt.switch_to(c, "shade");
    assert!(rt.open_window("equalizer"));
    let eq = rt.container("equalizer").expect("the equalizer");
    rt.switch_to(eq, "shade");
    rt.ensure_layout();
    let tap = |rt: &mut SkinRuntime, x: i32, y: i32, seq: i32| {
        rt.pointer_down(x, y, seq);
        rt.pointer_up(x, y);
    };
    // the main bar's transport: play from its top row, stop from its last column
    tap(&mut rt, 177, 2, 1);
    assert_eq!(rt.player.status(), Status::Playing);
    tap(&mut rt, 203, 11, 2);
    assert_eq!(rt.player.status(), Status::Stopped);
    tap(&mut rt, 180, 12, 3);
    assert_eq!(
        rt.player.status(),
        Status::Stopped,
        "y 12 lies below the buttons"
    );
    tap(&mut rt, 181, 7, 4);
    // the mini position bar from x 228; 227 is dead
    tap(&mut rt, 227, 7, 5);
    assert_eq!(rt.player.position_ms(), 0);
    tap(&mut rt, 236, 12, 6);
    assert!(rt.player.position_ms() > 0, "x 236, y 12 seeks");
    // the equalizer's bar at y 14: volume to x 162, balance from 163, rows 3..=11
    rt.player.set_volume(128);
    tap(&mut rt, 162, 14 + 3, 7);
    assert_eq!(rt.player.volume(), 255);
    tap(&mut rt, 163, 14 + 11, 8);
    assert_eq!(rt.player.volume(), 255, "x 163 belongs to the balance");
    assert_eq!(rt.player.pan(), -127);
    tap(&mut rt, 100, 14 + 2, 9);
    assert_eq!(rt.player.volume(), 255, "y 2 lies above the sliders");
}

#[test]
fn the_equalizer_bar_sliders_follow_winamp() {
    // values read back from Winamp 5.666 in the VM while dragging the base skin's bar
    let mut rt = classic();
    let c = rt.container("main").expect("the main container");
    rt.switch_to(c, "shade");
    assert!(rt.open_window("equalizer"));
    let eq = rt.container("equalizer").expect("the equalizer");
    rt.switch_to(eq, "shade");
    rt.ensure_layout();
    rt.pointer_down(70, 14 + 7, 1);
    assert_eq!(rt.player.volume(), 23);
    for (x, v) in [(140, 209), (155, 249), (157, 255), (162, 255)] {
        rt.pointer_move(x, 14 + 7);
        assert_eq!(rt.player.volume(), v, "volume at x {x}");
    }
    rt.pointer_up(162, 14 + 7);
    rt.pointer_down(183, 14 + 7, 2);
    assert_eq!(rt.player.pan(), -12);
    // 8 either side of the middle snaps to it
    for (x, b) in [(184, 0), (186, 0), (187, 12), (206, 127), (183, -12)] {
        rt.pointer_move(x, 14 + 7);
        assert_eq!(rt.player.pan(), b, "balance at x {x}");
    }
    rt.pointer_up(183, 14 + 7);
    assert_eq!(rt.player.pan(), -12, "the bar's release keeps the balance");
}

#[test]
fn a_drag_across_the_bands_sets_each_band_it_reaches() {
    // as Winamp 5.666 in the VM: band 1 taken beside its thumb, then a sideways drag
    let mut rt = classic();
    assert!(rt.open_window("equalizer"));
    rt.ensure_layout();
    let step = |rt: &SkinRuntime, i: usize| crate::classic::eq::band_step(rt.player.eq_bands()[i]);
    let preamp = rt.player.eq_preamp();
    rt.pointer_down(84, 116 + 80, 1);
    assert_eq!(step(&rt, 0), 44);
    rt.pointer_move(84, 116 + 60);
    assert_eq!(step(&rt, 0), 19);
    // past band 1's column it lets go, with the value of where the finger left it
    rt.pointer_move(102, 116 + 60);
    assert_eq!(step(&rt, 0), 19);
    rt.pointer_move(102, 116 + 85);
    assert_eq!(
        (step(&rt, 0), step(&rt, 1)),
        (19, 50),
        "band 2 takes the drag"
    );
    rt.pointer_move(120, 116 + 90);
    assert_eq!(step(&rt, 1), 56, "band 2 set from where the finger left it");
    rt.pointer_move(120, 116 + 95);
    assert_eq!(step(&rt, 2), 62, "band 3 takes the drag");
    rt.pointer_up(120, 116 + 95);
    assert_eq!(step(&rt, 3), 31, "band 4 never reached");
    assert_eq!(rt.player.eq_preamp(), preamp);
    // a band keeps the drag within 3 columns left and 15 right of its own
    rt.pointer_down(84, 116 + 80, 2);
    rt.pointer_move(93, 116 + 60);
    rt.pointer_move(93, 116 + 71);
    assert_eq!(step(&rt, 0), 33, "x 93 still drags band 1");
    rt.pointer_up(93, 116 + 71);
}

#[test]
fn the_playlist_rows_take_presses_up_to_the_scrollbar() {
    let clock: Clock = Rc::new(|| 0);
    let mut player = FakePlayer::new(clock.clone());
    player.playlist = vec![
        Track {
            title: "One".into(),
            length_ms: 60_000,
        },
        Track {
            title: "Two".into(),
            length_ms: 60_000,
        },
    ];
    let mut rt = classic_with(player, clock);
    assert!(rt.open_window("pledit"));
    rt.ensure_layout();
    let tap = |rt: &mut SkinRuntime, x: i32, y: i32, seq: i32| {
        rt.pointer_down(x, y, seq);
        rt.pointer_up(x, y);
    };
    tap(&mut rt, 11, 116 + 26, 1);
    assert_eq!(rt.classic_selected_row, None, "x 11 lies left of the rows");
    tap(&mut rt, 250, 116 + 20, 2);
    assert_eq!(rt.classic_selected_row, Some(0));
    tap(&mut rt, 255, 116 + 40, 3);
    assert_eq!(rt.classic_selected_row, Some(1));
    tap(&mut rt, 256, 116 + 26, 4);
    assert_eq!(rt.classic_selected_row, Some(1), "x 256 lies past the rows");
    tap(&mut rt, 60, 116 + 19, 5);
    assert_eq!(rt.classic_selected_row, Some(1), "y 19 lies above the rows");
}

#[test]
fn a_playing_classic_window_is_due_again_every_frame() {
    let mut rt = classic();
    rt.ensure_layout();
    assert_ne!(rt.next_due_at(), Some(25), "stopped, no frame is due");
    rt.player.play();
    assert_eq!(
        rt.next_due_at(),
        Some(25),
        "the vis, time and position bar move while playing"
    );
    assert!(
        rt.frames_wanted(),
        "a host that paces by the screen keeps ticking"
    );
    assert!(rt.tick(), "and each tick changes the window");
    rt.player.stop();
    assert_ne!(rt.next_due_at(), Some(25));
    assert!(!rt.frames_wanted());
}

/// A host that keeps the actions the skin sends it, and where on screen each came from.
struct RecordingHost(Rc<RefCell<Vec<String>>>, Froms);

/// Where each host action came from, in order.
type Froms = Rc<RefCell<Vec<Option<crate::host::ActionFrom>>>>;

impl SkinHost for RecordingHost {
    fn image_size(&mut self, _file: &str) -> Option<(i32, i32)> {
        None
    }

    fn text_width(&mut self, text: &str, _font: Option<&FontDef>, size: i32, bold: bool) -> i32 {
        text.chars().count() as i32 * size * if bold { 7 } else { 6 } / 10
    }

    fn log(&mut self, _message: &str) {}

    fn invalidate(&mut self) {}

    fn host_action(
        &mut self,
        action: &str,
        param: Option<&str>,
        from: Option<crate::host::ActionFrom>,
    ) {
        self.0.borrow_mut().push(
            format!("{action} {}", param.unwrap_or(""))
                .trim_end()
                .to_string(),
        );
        self.1.borrow_mut().push(from);
    }
}

fn classic_recording() -> (SkinRuntime, Rc<RefCell<Vec<String>>>) {
    let (rt, actions, _) = classic_recording_froms();
    (rt, actions)
}

fn classic_recording_froms() -> (SkinRuntime, Rc<RefCell<Vec<String>>>, Froms) {
    let actions = Rc::new(RefCell::new(Vec::new()));
    let froms: Froms = Rc::new(RefCell::new(Vec::new()));
    let mut files = MemoryFiles::new();
    for s in SHEETS {
        files.insert(s, &b"x"[..]);
    }
    files.insert("region.txt", REGION.as_bytes());
    let clock: Clock = Rc::new(|| 0);
    let mut rt = SkinRuntime::new(
        Box::new(files),
        Box::new(RecordingHost(actions.clone(), froms.clone())),
        Box::new(FakePlayer::new(clock.clone())),
        SkinSetup {
            host_actions: true,
            ..SkinSetup::default()
        },
        clock,
    );
    rt.load();
    (rt, actions, froms)
}

#[test]
fn a_menu_button_tells_the_host_its_box_and_where_the_finger_went_down() {
    let (mut rt, actions, froms) = classic_recording_froms();
    assert!(rt.open_window("equalizer"));
    rt.ensure_layout();
    // the equalizer under the main window: PRESETS answers x 217..=261, y 18..=30
    rt.pointer_down(230, 116 + 24, 1);
    rt.pointer_up(230, 116 + 24);
    assert_eq!(*actions.borrow(), vec!["EQ_PRESETS"]);
    assert_eq!(
        froms.borrow()[0],
        Some(crate::host::ActionFrom {
            rect: Rect::new(217, 116 + 18, 45, 13),
            point: Some((230, 116 + 24)),
        })
    );
}

#[test]
fn the_playlist_buttons_pop_their_menus_and_the_pick_goes_to_the_host() {
    let (mut rt, actions) = classic_recording();
    assert!(rt.open_window("pledit"));
    rt.ensure_layout();
    // ADD held: the bar and three pictures stack over it, the one under the finger pressed
    rt.pointer_down(20, 116 + 95, 1);
    rt.pointer_move(20, 116 + 55);
    let windows = rt.draw_windows();
    let (_, cmds) = windows
        .iter()
        .find(|(r, _)| r.top == 116)
        .expect("the playlist");
    let pl = blits(cmds, "pledit.bmp");
    assert!(pl.contains(&(48, 111, 3, 54, 11, 50)), "the bar: {pl:?}");
    assert!(
        pl.contains(&(23, 111, 22, 18, 14, 50)),
        "the top picture pressed"
    );
    assert!(
        pl.contains(&(0, 130, 22, 18, 14, 68)),
        "the middle picture at rest"
    );
    assert!(pl.contains(&(0, 149, 22, 18, 14, 86)));
    rt.pointer_up(20, 116 + 55);
    assert_eq!(
        actions.borrow().last().map(String::as_str),
        Some("PLAYLIST add_url")
    );
    // REM has four pictures; a release beside them picks nothing
    rt.pointer_down(50, 116 + 95, 2);
    rt.pointer_move(50, 116 + 40);
    let windows = rt.draw_windows();
    let (_, cmds) = windows
        .iter()
        .find(|(r, _)| r.top == 116)
        .expect("the playlist");
    let pl = blits(cmds, "pledit.bmp");
    assert!(pl.contains(&(100, 111, 3, 72, 40, 32)), "the taller bar");
    assert!(
        pl.contains(&(77, 168, 22, 18, 43, 32)),
        "the top picture pressed"
    );
    rt.pointer_move(120, 116 + 40);
    rt.pointer_up(120, 116 + 40);
    assert_eq!(actions.borrow().len(), 1, "nothing picked");
    // REM's pictures top to bottom: misc, all, crop, the selected songs
    for (y, pick) in [
        (40, "remove_misc"),
        (58, "remove_all"),
        (76, "crop"),
        (94, "remove_selected"),
    ] {
        rt.pointer_down(50, 116 + 95, 5);
        rt.pointer_move(50, 116 + y);
        rt.pointer_up(50, 116 + y);
        assert_eq!(
            actions.borrow().last().map(String::as_str),
            Some(&*format!("PLAYLIST {pick}"))
        );
    }
    // LIST sits at the right; its lowest picture loads a list
    rt.pointer_down(240, 116 + 95, 3);
    rt.pointer_move(240, 116 + 92);
    rt.pointer_up(240, 116 + 92);
    assert_eq!(
        actions.borrow().last().map(String::as_str),
        Some("PLAYLIST list_load")
    );
    // SEL's middle picture clears the selection here and tells the host, which then hears
    // that no row is selected
    rt.classic_selected_row = Some(0);
    rt.pointer_down(80, 116 + 95, 4);
    rt.pointer_move(80, 116 + 72);
    rt.pointer_up(80, 116 + 72);
    assert_eq!(rt.classic_selected_row, None);
    let n = actions.borrow().len();
    assert_eq!(
        actions.borrow()[n - 2..],
        ["PLAYLIST select_none", "PLAYLIST_SELECT"]
    );
}

#[test]
fn a_row_tapped_in_the_playlist_is_the_selection_the_host_hears_of() {
    let (mut rt, actions) = classic_recording();
    assert!(rt.open_window("pledit"));
    rt.ensure_layout();
    // the first row, as a_press_on_a_playlist_row_selects_it presses it
    rt.pointer_down(30, 116 + 25, 1);
    rt.pointer_up(30, 116 + 25);
    assert_eq!(rt.classic_selected_row, Some(0));
    assert_eq!(*actions.borrow(), vec!["PLAYLIST_SELECT 0"]);
    // the same row again tells the host nothing new
    rt.pointer_down(30, 116 + 25, 2);
    rt.pointer_up(30, 116 + 25);
    assert_eq!(actions.borrow().len(), 1);
}

#[test]
fn in_the_skins_letters_the_rows_are_one_block_and_a_row_they_cannot_spell_keeps_the_font() {
    let clock: Clock = Rc::new(|| 0);
    let mut player = FakePlayer::new(clock.clone());
    player.playlist = vec![
        Track {
            title: "One".into(),
            length_ms: 60_000,
        },
        Track {
            title: "Кино".into(),
            length_ms: 60_000,
        },
    ];
    let mut rt = classic_with(player, clock);
    assert!(rt.open_window("pledit"));
    // a text.bmp whose A cell has one lit pixel on plain paper
    let mut sheet = vec![0xff10_1010u32; 155 * 18];
    sheet[0] = 0xff30_f030;
    rt.classic_list_ink = crate::classic::listfont::ListInk::of(&sheet, 155, 18).map(Rc::new);
    rt.classic_list_ink_read = true;
    let texts = |rt: &mut SkinRuntime| -> (Vec<String>, Vec<Rect>) {
        let windows = rt.draw_windows();
        let (_, cmds) = windows
            .iter()
            .find(|(r, _)| r.top == 116)
            .expect("the playlist");
        let words = cmds
            .iter()
            .filter_map(|c| match c {
                DrawCmd::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect();
        let blocks = cmds
            .iter()
            .filter_map(|c| match c {
                DrawCmd::Pixels { rect, .. } => Some(*rect),
                _ => None,
            })
            .collect();
        (words, blocks)
    };
    let (words, blocks) = texts(&mut rt);
    assert!(
        words.contains(&"1. One".to_string()),
        "off, every row in the font"
    );
    assert!(blocks.is_empty());

    rt.set_presentation(&crate::host::Presentation {
        skin_list_font: true,
        ..rt.presentation()
    });
    let (words, blocks) = texts(&mut rt);
    assert!(
        !words.contains(&"1. One".to_string()),
        "spelled in the letters"
    );
    assert!(
        words.contains(&"2. Кино".to_string()),
        "the letters cannot spell it"
    );
    assert_eq!(
        blocks,
        vec![Rect::new(12, 22, 275 - 32, 3 * 15)],
        "over the list, in the window"
    );
}

#[test]
fn the_playlist_rows_ask_for_winamps_em_of_twelve_pixels() {
    let clock: Clock = Rc::new(|| 0);
    let mut player = FakePlayer::new(clock.clone());
    player.playlist = vec![Track {
        title: "One".into(),
        length_ms: 60_000,
    }];
    let mut rt = classic_with(player, clock);
    assert!(rt.open_window("pledit"));
    let windows = rt.draw_windows();
    let (_, cmds) = windows
        .iter()
        .find(|(r, _)| r.top == 116)
        .expect("the playlist");
    let sizes: Vec<i32> = cmds
        .iter()
        .filter_map(|c| match c {
            DrawCmd::Text(t) => Some(t.size),
            _ => None,
        })
        .collect();
    // a size is points to a host: 16 of them are 12 pixels
    assert_eq!(sizes, vec![16, 16], "the title and the length");
}

#[test]
fn taller_rows_from_the_host_show_fewer_and_a_tap_picks_the_row_under_it() {
    let clock: Clock = Rc::new(|| 0);
    let mut player = FakePlayer::new(clock.clone());
    player.playlist = (0..10)
        .map(|i| Track {
            title: format!("Song {i}"),
            length_ms: 60_000,
        })
        .collect();
    let mut rt = classic_with(player, clock);
    assert!(rt.open_window("pledit"));
    rt.ensure_layout();
    assert_eq!(rt.classic_pl_rows(116), 3, "Winamp's 15-pixel rows");
    rt.set_presentation(&crate::host::Presentation {
        list_row_extra: 12,
        ..rt.presentation()
    });
    assert_eq!(rt.classic_row(), 27);
    assert_eq!(rt.classic_pl_rows(116), 2, "(116 - 60) / 27");
    let windows = rt.draw_windows();
    let (_, cmds) = windows
        .iter()
        .find(|(r, _)| r.top == 116)
        .expect("the playlist");
    let boxes: Vec<Rect> = cmds
        .iter()
        .filter_map(|c| match c {
            DrawCmd::Text(t) if t.align == Align::Left => Some(t.rect),
            _ => None,
        })
        .collect();
    assert_eq!(
        boxes,
        vec![Rect::new(12, 22, 243, 27), Rect::new(12, 49, 243, 27)]
    );
    // a tap low in the second row picks the second song
    rt.pointer_down(30, 116 + 22 + 27 + 20, 1);
    rt.pointer_up(30, 116 + 22 + 27 + 20);
    assert_eq!(rt.classic_selected_row, Some(1));
}

/// The playlist's pledit.bmp blits, as the stack draws them now.
fn playlist_blits(rt: &mut SkinRuntime) -> Vec<(i32, i32, i32, i32, i32, i32)> {
    let windows = rt.draw_windows();
    let (_, cmds) = windows
        .iter()
        .find(|(r, _)| r.top == 116)
        .expect("the playlist");
    blits(cmds, "pledit.bmp")
}

#[test]
fn under_touch_a_tap_on_a_playlist_button_leaves_its_menu_open_for_a_second_tap() {
    let (mut rt, actions) = classic_recording();
    rt.set_presentation(&crate::host::Presentation {
        touch: true,
        ..rt.presentation()
    });
    assert!(rt.open_window("pledit"));
    rt.ensure_layout();
    let tap = |rt: &mut SkinRuntime, x: i32, y: i32, seq: i32| {
        rt.pointer_down(x, y, seq);
        rt.pointer_up(x, y);
    };
    // a tap on ADD: nothing picked, the bar and its three pictures stay up
    tap(&mut rt, 20, 116 + 95, 1);
    assert!(actions.borrow().is_empty(), "a tap picks nothing");
    let pl = playlist_blits(&mut rt);
    assert!(
        pl.contains(&(48, 111, 3, 54, 11, 50)),
        "the bar stays: {pl:?}"
    );
    assert!(
        pl.contains(&(0, 111, 22, 18, 14, 50)),
        "the top picture at rest"
    );
    // the top picture lights while the second finger holds it and picks on release
    rt.pointer_down(20, 116 + 55, 2);
    assert!(
        playlist_blits(&mut rt).contains(&(23, 111, 22, 18, 14, 50)),
        "pressed"
    );
    rt.pointer_up(20, 116 + 55);
    assert_eq!(*actions.borrow(), vec!["PLAYLIST add_url"]);
    assert!(
        !playlist_blits(&mut rt).contains(&(48, 111, 3, 54, 11, 50)),
        "a pick closes the menu"
    );
    // a tap beside an open menu closes it and does nothing else
    tap(&mut rt, 50, 116 + 95, 3);
    assert!(
        playlist_blits(&mut rt).contains(&(100, 111, 3, 72, 40, 32)),
        "REM open"
    );
    tap(&mut rt, 200, 116 + 40, 4);
    assert!(
        !playlist_blits(&mut rt).contains(&(100, 111, 3, 72, 40, 32)),
        "closed"
    );
    assert_eq!(actions.borrow().len(), 1, "and nothing picked");
    // a press on a picture that slides off before the release closes the menu, picking nothing
    tap(&mut rt, 80, 116 + 95, 6);
    assert!(
        playlist_blits(&mut rt).contains(&(150, 111, 3, 54, 69, 50)),
        "SEL open"
    );
    rt.pointer_down(80, 116 + 72, 7);
    rt.pointer_move(200, 116 + 72);
    rt.pointer_up(200, 116 + 72);
    assert!(
        !playlist_blits(&mut rt).contains(&(150, 111, 3, 54, 69, 50)),
        "SEL closed"
    );
    assert_eq!(actions.borrow().len(), 1, "nothing picked");
    // holding and sliding to a picture still picks it on release, as with a mouse
    rt.pointer_down(50, 116 + 95, 5);
    rt.pointer_move(50, 116 + 58);
    rt.pointer_up(50, 116 + 58);
    assert_eq!(
        actions.borrow().last().map(String::as_str),
        Some("PLAYLIST remove_all")
    );
}

#[test]
fn with_a_mouse_a_click_on_a_playlist_button_picks_its_lowest_picture_and_closes() {
    let (mut rt, actions) = classic_recording();
    assert!(rt.open_window("pledit"));
    rt.ensure_layout();
    rt.pointer_down(20, 116 + 95, 1);
    rt.pointer_up(20, 116 + 95);
    assert_eq!(*actions.borrow(), vec!["PLAYLIST add_file"]);
    assert!(!playlist_blits(&mut rt).contains(&(48, 111, 3, 54, 11, 50)));
}

#[test]
fn the_menu_button_the_clutter_bar_and_presets_reach_the_host() {
    let (mut rt, actions) = classic_recording();
    assert!(rt.open_window("equalizer"));
    rt.ensure_layout();
    let tap = |rt: &mut SkinRuntime, x: i32, y: i32, seq: i32| {
        rt.pointer_down(x, y, seq);
        rt.pointer_up(x, y);
    };
    tap(&mut rt, 10, 7, 1);
    tap(&mut rt, 13, 25, 2);
    tap(&mut rt, 13, 41, 3);
    tap(&mut rt, 13, 58, 4);
    tap(&mut rt, 240, 116 + 24, 5);
    assert_eq!(
        *actions.borrow(),
        vec![
            "MENU",
            "MENU wa5:options",
            "TRACKINFO",
            "VIS_MENU",
            "EQ_PRESETS"
        ]
    );
}

#[test]
fn a_finger_wobbling_within_the_touch_slop_keeps_its_clutter_letter() {
    let (mut rt, actions) = classic_recording();
    rt.ensure_layout();
    // I is 40..47; the finger lands low on it and wobbles 3 px down onto D, within the slop
    rt.pointer_down(15, 46, 1);
    rt.pointer_move(15, 48);
    rt.pointer_move(15, 49);
    rt.pointer_up(15, 49);
    // further than the slop the press has left its letter, as a mouse leaves it
    rt.pointer_down(15, 42, 2);
    rt.pointer_move(15, 52);
    rt.pointer_up(15, 52);
    assert_eq!(*actions.borrow(), vec!["TRACKINFO"]);
}

#[test]
fn the_labels_between_preamp_and_bands_set_all_ten_bands() {
    let mut rt = classic();
    assert!(rt.open_window("equalizer"));
    rt.ensure_layout();
    let tap = |rt: &mut SkinRuntime, x: i32, y: i32, seq: i32| {
        rt.pointer_down(x, y, seq);
        rt.pointer_up(x, y);
    };
    rt.player.set_eq_preamp(200);
    // "+12" sends every band to the top, "-12" to the bottom, "0" back to the middle
    tap(&mut rt, 55, 116 + 37, 1);
    assert!(rt.player.eq_bands().iter().take(10).all(|v| *v == 255));
    tap(&mut rt, 42, 116 + 101, 2);
    assert!(rt.player.eq_bands().iter().take(10).all(|v| *v == 0));
    tap(&mut rt, 67, 116 + 65, 3);
    assert!(rt.player.eq_bands().iter().take(10).all(|v| *v == 128));
    assert_eq!(rt.player.eq_preamp(), 200, "the preamp stays");
    // a press on a label that lets go elsewhere changes nothing, nor does a press beside it
    tap(&mut rt, 55, 116 + 37, 4);
    rt.pointer_down(55, 116 + 70, 5);
    rt.pointer_up(55, 116 + 80);
    assert!(rt.player.eq_bands().iter().take(10).all(|v| *v == 255));
    tap(&mut rt, 68, 116 + 70, 6);
    tap(&mut rt, 55, 116 + 64, 7);
    assert!(rt.player.eq_bands().iter().take(10).all(|v| *v == 255));
}

#[test]
fn a_band_near_the_middle_snaps_to_it() {
    let mut rt = classic();
    assert!(rt.open_window("equalizer"));
    rt.ensure_layout();
    // with the band at the top, a press beside its thumb at y 69 or 70 gives the steps 30 or
    // 32 and lands on the middle; at 68 or 71 (29 and 33) it does not
    for (seq, y, snaps) in [(1, 69, true), (2, 70, true), (3, 68, false), (4, 71, false)] {
        rt.player.set_eq_band(0, 255);
        rt.pointer_down(85, 116 + y, seq);
        rt.pointer_up(85, 116 + y);
        assert_eq!(rt.player.eq_bands()[0] == 128, snaps, "y {y}");
    }
    // a press on the thumb keeps it where it is
    rt.player.set_eq_band(0, 255);
    rt.pointer_down(85, 116 + 42, 5);
    rt.pointer_up(85, 116 + 42);
    assert_eq!(rt.player.eq_bands()[0], 255);
}

#[test]
fn the_control_buttons_hand_the_press_along_and_any_bare_place_moves_the_window() {
    let mut rt = classic();
    rt.ensure_layout();
    // pressed on play, slid to stop and let go there: stop it is
    rt.player.play();
    rt.pointer_down(45, 97, 1);
    rt.pointer_move(70, 97);
    let cmds = rt.draw_main();
    assert!(
        blits(&cmds, "cbuttons.bmp").contains(&(46, 18, 23, 18, 62, 88)),
        "pause shows pressed under the finger"
    );
    rt.pointer_move(95, 97);
    rt.pointer_up(95, 97);
    assert_eq!(rt.player.status(), Status::Stopped);
    // a press on a place without a control moves the window, as Winamp's easy move does
    rt.pointer_down(200, 45, 2);
    assert!(rt.move_target().is_some());
    rt.pointer_up(200, 45);
    // the vis area is such a place while nothing plays; playing, the press itself steps the mode
    rt.pointer_down(60, 50, 3);
    assert!(
        rt.move_target().is_some(),
        "stopped: the vis moves the window"
    );
    rt.pointer_up(60, 50);
    assert_eq!(rt.classic_vis_mode, crate::classic::vis::VisMode::Analyzer);
    rt.player.play();
    rt.pointer_down(60, 50, 4);
    assert_eq!(
        rt.classic_vis_mode,
        crate::classic::vis::VisMode::Scope,
        "on the press"
    );
    rt.pointer_up(60, 50);
    assert_eq!(rt.classic_vis_mode, crate::classic::vis::VisMode::Scope);
}

#[test]
fn the_sliders_keep_their_grab_tell_their_value_and_the_balance_snaps() {
    let mut rt = classic();
    rt.ensure_layout();
    rt.player.set_volume(128);
    // taken on the thumb (x 132..=146 at 128) the volume stays; beside it, the middle jumps there
    rt.pointer_down(140, 63, 1);
    assert!((rt.player.volume() - 128).abs() <= 5);
    assert_eq!(rt.classic_status.as_deref(), Some("Volume: 49%"));
    rt.pointer_up(140, 63);
    assert_eq!(rt.classic_status, None, "the song's title comes back");
    rt.pointer_down(170, 63, 2);
    assert_eq!(rt.player.volume(), 255);
    assert_eq!(rt.classic_status.as_deref(), Some("Volume: 100%"));
    rt.pointer_up(170, 63);
    // the balance: a step beside the middle snaps to it while dragging; let go within 26 of it,
    // it goes to the middle
    rt.pointer_down(196, 63, 3);
    rt.pointer_move(197, 63);
    assert_eq!(rt.player.pan(), 0);
    assert_eq!(rt.classic_status.as_deref(), Some("Balance: Center"));
    rt.pointer_move(194, 63);
    assert!(rt.player.pan() < 0 && rt.player.pan() > -27);
    assert_eq!(rt.classic_status.as_deref(), Some("Balance: 16% left"));
    rt.pointer_up(194, 63);
    assert_eq!(rt.player.pan(), 0, "let go near the middle");
    rt.pointer_down(196, 63, 4);
    rt.pointer_move(210, 63);
    rt.pointer_up(210, 63);
    assert!(rt.player.pan() > 26, "further out it stays");
    // the position bar, pressed beside its thumb, takes the thumb's middle and tells the seek
    rt.player.play();
    rt.pointer_down(140, 77, 5);
    // the fake player's song is 3:35; x 140 is step 109 of 218, half of it
    assert_eq!(
        rt.classic_status.as_deref(),
        Some("Seek To: 01:47/03:35 (49%)")
    );
    rt.pointer_up(140, 77);
    assert!((rt.player.position_ms() - 107_500).abs() < 1000);
    // an equalizer band tells its dB, the preamp at the middle "+0.0"
    assert!(rt.open_window("equalizer"));
    rt.ensure_layout();
    rt.pointer_down(85, 116 + 45, 6);
    assert_eq!(rt.classic_status.as_deref(), Some("EQ: 70Hz: +11.6 dB"));
    rt.pointer_up(85, 116 + 45);
    rt.pointer_down(27, 116 + 69, 7);
    assert_eq!(rt.classic_status.as_deref(), Some("EQ: Preamp: +0.0 dB"));
    rt.pointer_up(27, 116 + 69);
}

#[test]
fn the_title_scrolls_by_hand_and_waits_two_seconds_after() {
    use crate::classic::main::TITLE_STEP_MS;
    let now = Rc::new(std::cell::Cell::new(0i64));
    let at = now.clone();
    let clock: Clock = Rc::new(move || at.get());
    let mut player = FakePlayer::new(clock.clone());
    Player::set_track(
        &mut player,
        "A rather long song title that scrolls across the window",
        600_000,
    );
    let mut rt = classic_with(player, clock);
    rt.ensure_layout();
    let _ = rt.draw_main();
    assert_eq!(rt.classic_title_scroll(), 0);
    // dragged 20 px to the left: the title moves on 20 px and holds
    rt.pointer_down(200, 30, 1);
    rt.pointer_move(180, 30);
    assert_eq!(rt.classic_title_scroll(), 20);
    now.set(1000);
    assert_eq!(rt.classic_title_scroll(), 20, "held under the finger");
    rt.pointer_up(180, 30);
    // two seconds of waiting, then a cell per step again
    now.set(2900);
    assert_eq!(rt.classic_title_scroll(), 20);
    assert_eq!(rt.next_due_at(), Some(3000));
    now.set(3000 + TITLE_STEP_MS);
    assert_eq!(rt.classic_title_scroll(), 25);
}

#[test]
fn the_logo_double_taps_and_the_shade_bar_reach_what_winamp_does() {
    let (mut rt, actions) = classic_recording();
    rt.ensure_layout();
    let tap = |rt: &mut SkinRuntime, x: i32, y: i32, seq: i32| {
        rt.pointer_down(x, y, seq);
        rt.pointer_up(x, y);
    };
    tap(&mut rt, 258, 98, 1);
    rt.double_tap(150, 30, 2);
    rt.player.play();
    tap(&mut rt, 60, 50, 3);
    rt.double_tap(60, 50, 4);
    assert_eq!(
        rt.classic_vis_mode,
        crate::classic::vis::VisMode::Analyzer,
        "back to before the taps"
    );
    assert_eq!(*actions.borrow(), vec!["ABOUT", "TRACKINFO", "VIS_FS"]);
    // the shade bar: its vis steps the mode on the press, its buttons split at 176
    let c = rt.container("main").expect("the main container");
    rt.switch_to(c, "shade");
    rt.ensure_layout();
    rt.pointer_down(100, 7, 5);
    assert_eq!(rt.classic_vis_mode, crate::classic::vis::VisMode::Scope);
    rt.pointer_up(100, 7);
    tap(&mut rt, 200, 7, 6);
    assert_eq!(rt.player.status(), Status::Stopped);
    tap(&mut rt, 176, 7, 7);
    assert_eq!(rt.player.status(), Status::Playing, "x 176 is play's");
}

#[test]
fn the_playlist_small_transport_time_arrows_and_button_double_taps() {
    let now = Rc::new(std::cell::Cell::new(0i64));
    let at = now.clone();
    let clock: Clock = Rc::new(move || at.get());
    let mut player = FakePlayer::new(clock.clone());
    player.playlist = (0..20)
        .map(|i| Track {
            title: format!("Song {i}"),
            length_ms: 600_000,
        })
        .collect();
    let actions = Rc::new(RefCell::new(Vec::new()));
    let mut files = MemoryFiles::new();
    for s in SHEETS {
        files.insert(s, &b"x"[..]);
    }
    files.insert("region.txt", REGION.as_bytes());
    let mut rt = SkinRuntime::new(
        Box::new(files),
        Box::new(RecordingHost(actions.clone(), Rc::default())),
        Box::new(player),
        SkinSetup {
            host_actions: true,
            ..SkinSetup::default()
        },
        clock,
    );
    rt.load();
    assert!(rt.open_window("pledit"));
    rt.ensure_layout();
    // the small transport acts on the press: play (x w-137..=w-128), stop (w-117..=w-109)
    rt.pointer_down(275 - 144 + 10, 116 + 104, 1);
    assert_eq!(rt.player.status(), Status::Playing, "on the press");
    rt.pointer_up(275 - 144 + 10, 116 + 104);
    rt.pointer_down(275 - 144 + 30, 116 + 104, 2);
    rt.pointer_up(275 - 144 + 30, 116 + 104);
    assert_eq!(rt.player.status(), Status::Stopped);
    rt.pointer_down(275 - 144 + 10, 116 + 104, 8);
    rt.pointer_up(275 - 144 + 10, 116 + 104);
    // the time shows what is left, with text.bmp's minus where the blank goes
    rt.pointer_down(275 - 70, 116 + 101, 3);
    rt.pointer_up(275 - 70, 116 + 101);
    assert!(rt.classic_remaining);
    let windows = rt.draw_windows();
    let (_, cmds) = windows
        .iter()
        .find(|(r, _)| r.top == 116)
        .expect("the playlist");
    assert!(blits(cmds, "text.bmp").contains(&(75, 6, 3, 6, 275 - 82, 116 - 15)));
    // the arrows above the corner scroll a row: down at h-31..=h-27, up at h-36..=h-32
    rt.pointer_down(264, 116 + 87, 4);
    rt.pointer_up(264, 116 + 87);
    assert_eq!(rt.classic_pl_scroll, 1);
    rt.pointer_down(264, 116 + 82, 5);
    rt.pointer_up(264, 116 + 82);
    assert_eq!(rt.classic_pl_scroll, 0);
    // a double tap on a bottom button picks its lowest item
    rt.double_tap(20, 116 + 95, 6);
    rt.double_tap(80, 116 + 95, 7);
    assert_eq!(
        *actions.borrow(),
        vec!["PLAYLIST add_file", "PLAYLIST select_all"]
    );
}

#[test]
fn the_eq_texts_match_the_reference_vm_gestures() {
    let mut rt = classic();
    assert!(rt.open_window("equalizer"));
    rt.ensure_layout();
    // the VM: band 1 at the middle taken on its thumb at y 63, dragged to 53: "+5.1 dB"
    rt.pointer_down(85, 116 + 63, 1);
    rt.pointer_move(85, 116 + 53);
    assert_eq!(rt.classic_status.as_deref(), Some("EQ: 70Hz: +5.1 dB"));
    rt.pointer_up(85, 116 + 53);
    // the VM: the preamp at the middle taken beside its thumb at y 58, dragged to 69: "+0.0 dB"
    rt.pointer_down(27, 116 + 58, 2);
    rt.pointer_move(27, 116 + 69);
    assert_eq!(rt.classic_status.as_deref(), Some("EQ: Preamp: +0.0 dB"));
    rt.pointer_up(27, 116 + 69);
}

/// A blit: source left, top, width, height, then dest left, top.
type Blit = (i32, i32, i32, i32, i32, i32);

/// The blits of the library window, opened and sized to `w` x `h` by its grip.
fn library_at(w: i32, h: i32) -> (SkinRuntime, Vec<Blit>) {
    let mut rt = classic();
    assert!(rt.open_window("library"));
    rt.ensure_layout();
    let at = rt.window_rect(rt.window_of("library").expect("the library"));
    let (cx, cy) = (at.right() - 5, at.bottom() - 5);
    rt.pointer_down(cx, cy, 1);
    rt.pointer_move(at.left + w - 5, at.top + h - 5);
    rt.pointer_up(at.left + w - 5, at.top + h - 5);
    rt.ensure_layout();
    let at = rt.window_rect(rt.window_of("library").expect("the library"));
    assert_eq!((at.width, at.height), (w, h));
    let windows = rt.draw_windows();
    let (_, cmds) = windows
        .iter()
        .find(|(r, _)| *r == at)
        .expect("the library drawn");
    let local = blits(cmds, "gen.bmp");
    (rt, local)
}

#[test]
fn the_library_title_lays_out_as_the_embed_window_does() {
    // no font here: "Winamp Library" is 14 gaps of 5, 70 wide, on a plate of 75; at 775 wide
    // 24 fills, 12 on each side
    let (_, b) = library_at(775, 319);
    let title: Vec<_> = b.iter().filter(|b| b.5 == 0 && b.3 == 20).collect();
    let xs: Vec<i32> = title.iter().map(|b| b.4).collect();
    let mut want = vec![0];
    want.extend((0..12).map(|i| 25 + i * 25));
    want.extend([325, 350, 375, 400, 425]);
    want.extend((0..12).map(|i| 450 + i * 25));
    want.push(750);
    assert_eq!(xs, want);
    assert_eq!(title[0].0, 0, "the left corner");
    assert_eq!(title[13].0, 26, "the plate's left end");
    assert_eq!(title[14].0, 52, "the plate");
    assert_eq!(title[17].0, 78, "the plate's right end");
    assert_eq!(title.last().unwrap().0, 130, "the right corner");
    assert!(
        !b.iter().any(|b| b.2 == 9 && b.3 == 9),
        "close shows from the right corner until held"
    );
}

#[test]
fn the_library_frame_tiles_its_sides_and_bottom() {
    let (_, b) = library_at(775, 319);
    // (319 - 58) / 29 = 9 side tiles from y 20, no rest
    let left: Vec<_> = b
        .iter()
        .filter(|b| b.0 == 127 && b.1 == 42)
        .map(|b| b.5)
        .collect();
    assert_eq!(left, (0..9).map(|i| 20 + i * 29).collect::<Vec<_>>());
    assert!(b.contains(&(158, 42, 11, 24, 0, 281)), "bottom left corner");
    assert!(
        b.contains(&(170, 42, 8, 24, 767, 281)),
        "bottom right corner"
    );
    assert!(b.contains(&(0, 42, 125, 14, 0, 305)), "bottom bar left");
    assert!(b.contains(&(0, 57, 125, 14, 650, 305)), "bottom bar right");
    assert_eq!(
        b.iter().filter(|b| b.0 == 127 && b.1 == 72).count(),
        21,
        "(775-250)/25 fills"
    );
    assert!(b.contains(&(118, 72, 7, 7, 762, 307)), "the grip");
}

#[test]
fn the_library_sizes_in_steps_and_holds_the_hosts_area() {
    let (mut rt, _) = library_at(300, 145);
    let client = rt.classic_client_rect("library").expect("a gen window");
    assert_eq!(
        client,
        Rect::new(11, 20, 300 - 19, 145 - 34),
        "in the window"
    );
    assert_eq!(rt.classic_client_rect("main"), None);
}

#[test]
fn the_library_closes_on_a_release_over_its_close_button() {
    let mut rt = classic();
    assert!(rt.open_window("library"));
    rt.ensure_layout();
    let at = rt.window_rect(rt.window_of("library").expect("the library"));
    rt.pointer_down(at.right() - 5, at.top + 7, 1);
    rt.pointer_up(at.right() - 5, at.top + 7);
    rt.ensure_layout();
    let c = rt.container("library").expect("the container");
    assert!(!rt.is_open(c), "closed");
}

/// The video window's blits and its rect, opened at its default 275 x 232.
fn video_blits(rt: &mut SkinRuntime) -> (Rect, Vec<Blit>) {
    rt.ensure_layout();
    let at = rt.window_rect(rt.window_of("video").expect("the video window"));
    let windows = rt.draw_windows();
    let (_, cmds) = windows.iter().find(|(r, _)| *r == at).expect("drawn");
    (at, blits(cmds, "video.bmp"))
}

#[test]
fn the_video_window_frames_with_video_bmp() {
    let mut rt = classic();
    assert!(rt.open_window("video"));
    let (at, b) = video_blits(&mut rt);
    assert_eq!((at.width, at.height), (275, 232));
    // (275 - 150) / 25 = 5 fills: 12 and two on the left, 13 and two on the right
    let title: Vec<_> = b
        .iter()
        .filter(|b| b.5 == 0)
        .map(|b| (b.0, b.2, b.4))
        .collect();
    let row = title[0].0;
    assert_eq!(row, 0);
    assert_eq!(
        title,
        vec![
            (0, 25, 0),
            (127, 12, 25),
            (127, 25, 37),
            (127, 25, 62),
            (26, 100, 87),
            (127, 13, 187),
            (127, 25, 200),
            (127, 25, 225),
            (153, 25, 250),
        ]
    );
    // (232 - 58) / 29 = 6 side tiles; the bottom bar 38 tall at 194
    assert_eq!(b.iter().filter(|b| b.0 == 127 && b.1 == 42).count(), 6);
    assert!(b.contains(&(0, 42, 125, 38, 0, 194)));
    assert!(b.contains(&(127, 81, 25, 38, 125, 194)));
    assert!(b.contains(&(0, 81, 125, 38, 150, 194)));
}

#[test]
fn the_video_buttons_slide_and_answer_on_release() {
    let (mut rt, actions) = classic_recording();
    assert!(rt.open_window("video"));
    rt.ensure_layout();
    let at = rt.window_rect(rt.window_of("video").expect("the video window"));
    let y = at.top + 232 - 20;
    // pressed on full screen, slid to 2x: 2x shows held (x 39, from 39 + 149)
    rt.pointer_down(at.left + 12, y, 1);
    rt.pointer_move(at.left + 42, y);
    let (_, b) = video_blits(&mut rt);
    assert!(b.contains(&(188, 42, 15, 18, 39, 232 - 29)), "{b:?}");
    rt.pointer_up(at.left + 42, y);
    assert_eq!(actions.borrow().last().map(String::as_str), Some("VID_2X"));
    let c = rt.container("video").expect("the container");
    // close on release over x w-10..w-1, y 3..12
    rt.pointer_down(at.left + 270, at.top + 5, 1);
    rt.pointer_up(at.left + 270, at.top + 5);
    assert!(!rt.is_open(c));
}

#[test]
fn the_video_status_line_takes_its_colors_from_pledit() {
    let mut rt = classic();
    assert!(rt.open_window("video"));
    rt.ensure_layout();
    let (r, fg, bg) = rt.classic_status_line("video").expect("a video window");
    assert_eq!(r, Rect::new(92, 232 - 27, 275 - 25 - 92, 14));
    assert_eq!((fg, bg), (Rgb(0x00ff00), Rgb(0)));
}

#[test]
fn the_2_91_style_has_its_name_and_smaller_playlist_rows() {
    use crate::classic::style::ClassicStyle;
    let clock: Clock = Rc::new(|| 0);
    let mut player = FakePlayer::new(clock.clone());
    player.title.clear();
    let mut rt = classic_with(player, clock);
    assert!(rt.open_window("pledit"));
    rt.ensure_layout();
    assert_eq!(rt.classic_title_text(), "Winamp 5.666");
    rt.set_classic_style(ClassicStyle::from_version("2.91").unwrap());
    assert_eq!(rt.classic_title_text(), "Winamp 2.91");
    assert_eq!(rt.classic_row(), 13, "Arial at 10");
    rt.set_classic_style(ClassicStyle::Winamp5666);
    assert_eq!(rt.classic_row(), 15);
}

#[test]
fn a_wheel_notch_scrolls_as_far_as_each_winamp_does() {
    use crate::classic::style::ClassicStyle;
    let clock: Clock = Rc::new(|| 0);
    let mut player = FakePlayer::new(clock.clone());
    player.playlist = (0..40)
        .map(|i| Track {
            title: format!("Song {i}"),
            length_ms: 60_000,
        })
        .collect();
    let mut rt = classic_with(player, clock);
    assert!(rt.open_window("pledit"));
    rt.ensure_layout();
    rt.wheel_classic_playlist(1);
    assert_eq!(rt.classic_pl_scroll, 2);
    rt.set_classic_style(ClassicStyle::Winamp291);
    rt.wheel_classic_playlist(1);
    assert_eq!(rt.classic_pl_scroll, 10);
}

#[test]
fn a_smaller_playlist_font_draws_smaller_text_in_shorter_rows() {
    let mut rt = classic();
    assert!(rt.open_window("pledit"));
    rt.ensure_layout();
    assert_eq!((rt.classic_row(), rt.classic_row_font()), (15, 16));
    rt.set_classic_list_font_size(9);
    assert_eq!((rt.classic_row(), rt.classic_row_font()), (11, 12));
    assert_eq!(rt.classic_pl_rows(116), 5, "(116 - 60) / 11");
    let sizes: Vec<i32> = rt
        .draw_windows()
        .iter()
        .flat_map(|(_, cmds)| cmds.iter())
        .filter_map(|c| match c {
            DrawCmd::Text(t) => Some(t.size),
            _ => None,
        })
        .collect();
    assert!(
        !sizes.is_empty() && sizes.iter().all(|&s| s == 12),
        "{sizes:?}"
    );
}

#[test]
fn the_stack_names_winamps_cursor_under_the_pointer() {
    use crate::classic::cursor::CURSOR_FILES;
    let mut rt = classic();
    assert!(rt.open_window("pledit"));
    rt.ensure_layout();
    let name = |rt: &mut SkinRuntime, x, y| rt.classic_cursor_at(x, y).map(|s| CURSOR_FILES[s].0);
    assert_eq!(name(&mut rt, 100, 5), Some("titlebar.cur"));
    assert_eq!(name(&mut rt, 120, 62), Some("volbal.cur"));
    assert_eq!(
        name(&mut rt, 270, 116 + 110),
        Some("psize.cur"),
        "the playlist's corner"
    );
    assert_eq!(name(&mut rt, 100, 116 + 50), Some("pnormal.cur"));
    assert_eq!(name(&mut rt, 400, 400), None, "off the windows");
    // the test skin's files are not cursors: the host shows its own
    let slot = rt.classic_cursor_at(100, 5).expect("a slot");
    assert!(rt.classic_cursor(slot).is_none());
}

#[test]
fn a_cursor_the_skin_lacks_is_winamps_own_of_its_kind_from_the_base_skin() {
    use crate::classic::cursor::{tests::cur, CURSOR_FILES};
    let mut files = MemoryFiles::new();
    for s in SHEETS {
        files.insert(s, &b"x"[..]);
    }
    // the skin's own close cursor does not stand in for the equalizer's; Winamp's does
    files.insert("close.cur", cur((0, 1)));
    files.insert("wasabi-base/close.cur", cur((1, 0)));
    let clock: Clock = Rc::new(|| 0);
    let mut rt = SkinRuntime::new(
        Box::new(files),
        Box::new(QuietHost),
        Box::new(FakePlayer::new(clock.clone())),
        SkinSetup::default(),
        clock,
    );
    rt.load();
    let slot = |file| CURSOR_FILES.iter().position(|(f, _)| *f == file).unwrap();
    let hot = |rt: &mut SkinRuntime, file| {
        rt.classic_cursor(slot(file))
            .map(|c| (c.frames[0].hot_x, c.frames[0].hot_y))
    };
    assert_eq!(hot(&mut rt, "eqclose.cur"), Some((1, 0)), "the base skin's");
    assert_eq!(hot(&mut rt, "close.cur"), Some((0, 1)), "the skin's own");
    assert_eq!(hot(&mut rt, "normal.cur"), None, "no arrow in either");
}

#[test]
fn stacked_in_a_tall_view_the_classic_playlist_stretches_to_the_bottom() {
    let mut rt = classic();
    assert!(rt.open_window("pledit"));
    rt.set_presentation(&crate::host::Presentation {
        view_size: Some((275, 400)),
        stretch_last: true,
        ..rt.presentation()
    });
    rt.ensure_layout();
    let windows = rt.draw_windows();
    let pl = windows
        .iter()
        .map(|(r, _)| *r)
        .find(|r| r.top == 116)
        .expect("the playlist under the main window");
    // all 284 left under the main window
    assert_eq!(pl.height, 284);
}

fn apart(rt: &mut SkinRuntime) {
    rt.set_presentation(&crate::host::Presentation {
        free_windows: true,
        viewport: (1600, 1200),
        ..rt.presentation()
    });
    rt.ensure_layout();
}

#[test]
fn a_window_at_twice_its_scale_draws_takes_presses_and_docks_twice_as_large() {
    let mut rt = classic();
    assert!(rt.open_window("equalizer"));
    apart(&mut rt);
    let main = rt.main_window().expect("the main window");
    let eq = rt.window_of("equalizer").expect("the equalizer");
    assert_eq!(rt.window_rect(eq), Rect::new(0, 116, 275, 116));

    assert!(rt.set_window_scale("main", 2.0));
    rt.ensure_layout();
    assert_eq!(rt.window_rect(main), Rect::new(0, 0, 550, 232));
    assert_eq!(
        rt.window_rect(eq),
        Rect::new(0, 232, 275, 116),
        "the equalizer docked under it follows its bottom edge"
    );
    let drawn = rt.draw_windows();
    let (_, cmds) = drawn
        .iter()
        .find(|(r, _)| r.width == 550)
        .expect("the main window drawn");
    assert!(
        blits(cmds, "main.bmp").contains(&(0, 0, 275, 116, 0, 0)),
        "the same part of the picture"
    );
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            DrawCmd::Image { dst, .. } if *dst == Rect::new(0, 0, 550, 232)
        )),
        "stretched over the window"
    );
    // Play sits at twice its place
    rt.pointer_down(100, 194, 1);
    rt.pointer_up(100, 194);
    assert_eq!(rt.player.status(), Status::Playing);

    assert!(rt.set_window_scale("main", 1.0));
    rt.ensure_layout();
    assert_eq!(rt.window_rect(main), Rect::new(0, 0, 275, 116));
    assert_eq!(rt.window_rect(eq), Rect::new(0, 116, 275, 116));
    assert!(!rt.set_window_scale("nothing", 2.0));
}

#[test]
fn a_window_at_any_scale_keeps_its_drag_and_its_grip() {
    let mut rt = classic();
    assert!(rt.open_window("pledit"));
    apart(&mut rt);
    let pl = rt.window_of("pledit").expect("the playlist");
    assert!(rt.set_window_scale("pledit", 1.5));
    rt.ensure_layout();
    let r = rt.window_rect(pl);
    assert_eq!((r.width, r.height), (413, 174));
    // its grip: the corner of the window on screen, a 25 x 29 step at the scale
    rt.pointer_down(r.right() - 3, r.bottom() - 3, 1);
    rt.pointer_move(r.right() - 3 + 38, r.bottom() - 3 + 44);
    rt.pointer_up(r.right() - 3 + 38, r.bottom() - 3 + 44);
    rt.ensure_layout();
    assert_eq!(
        rt.window_size(pl),
        (300, 145),
        "one step each way, in layout pixels"
    );
    let moved = rt.window_rect(pl);
    assert_eq!((moved.width, moved.height), (450, 218));
}

#[test]
fn a_drag_ends_where_the_finger_lifts_even_with_no_move_there() {
    let mut rt = classic();
    apart(&mut rt);
    let main = rt.main_window().expect("the main window");
    rt.place_window(main, 300, 200);
    rt.ensure_layout();
    // the title bar, clear of its buttons
    rt.pointer_down(360, 206, 1);
    rt.pointer_move(400, 226);
    rt.pointer_up(402, 227);
    rt.ensure_layout();
    let r = rt.window_rect(main);
    assert_eq!((r.left, r.top), (342, 221));
}

#[test]
fn a_drag_of_docked_windows_only_places_them_again() {
    let mut rt = classic();
    assert!(rt.open_window("equalizer"));
    assert!(rt.open_window("pledit"));
    apart(&mut rt);
    rt.tick();
    let first = rt.draw_windows_owned();
    assert_eq!(rt.take_placed_only(), None, "drawn and nothing moved");
    let main = rt.main_window().expect("the main window");
    let r = rt.window_rect(main);
    // the press and the drag's first step may redraw: the title bar lights up
    rt.pointer_down(r.left + 60, r.top + 6, 1);
    rt.tick();
    rt.draw_windows_owned();
    rt.pointer_move(r.left + 70, r.top + 11);
    rt.tick();
    rt.draw_windows_owned();
    rt.pointer_move(r.left + 80, r.top + 16);
    rt.tick();
    let placed = rt.take_placed_only().expect("only moved");
    assert_eq!(placed.len(), first.len());
    for (p, w) in placed.iter().zip(&first) {
        let moved = Rect::new(
            w.rect.left + 20,
            w.rect.top + 10,
            w.rect.width,
            w.rect.height,
        );
        assert_eq!(p.1, moved, "every docked window went along");
    }
    assert_eq!(rt.take_placed_only(), None, "taken once");
    rt.pointer_up(r.left + 80, r.top + 16);
}

#[test]
fn the_visualizer_window_holds_the_hosts_visualizer_in_its_frame() {
    let mut rt = classic();
    apart(&mut rt);
    assert_eq!(rt.vis_rect(), None);
    assert!(rt.open_window("avs"));
    assert!(rt.set_window_title("avs", "Milkdrop"));
    rt.ensure_layout();
    let avs = rt.window_of("avs").expect("the visualizer window");
    let r = rt.window_rect(avs);
    assert_eq!((r.width, r.height), (275, 232));
    assert_eq!(
        rt.vis_rect(),
        Some(Rect::new(r.left + 11, r.top + 20, 275 - 19, 232 - 34))
    );
    rt.set_window_scale("avs", 2.0);
    rt.ensure_layout();
    let r = rt.window_rect(avs);
    assert_eq!(
        rt.vis_rect(),
        Some(Rect::new(
            r.left + 22,
            r.top + 40,
            2 * (275 - 19),
            2 * (232 - 34)
        ))
    );
}

#[test]
fn double_size_keeps_every_window_docked_there_and_back() {
    let mut rt = classic();
    for c in ["equalizer", "pledit", "avs"] {
        assert!(rt.open_window(c));
    }
    apart(&mut rt);
    let main = rt.main_window().expect("the main window");
    let eq = rt.window_of("equalizer").expect("the equalizer");
    let pl = rt.window_of("pledit").expect("the playlist");
    let avs = rt.window_of("avs").expect("the visualizer");
    rt.place_window(avs, 275, 0);
    rt.ensure_layout();
    let tops = |rt: &mut SkinRuntime| {
        [main, eq, pl, avs].map(|w| {
            let r = rt.window_rect(w);
            (r.left, r.top, r.width, r.height)
        })
    };
    let start = tops(&mut rt);
    assert_eq!(
        start,
        [
            (0, 0, 275, 116),
            (0, 116, 275, 116),
            (0, 232, 275, 116),
            (275, 0, 275, 232)
        ]
    );

    rt.set_window_scale("main", 2.0);
    rt.set_window_scale("equalizer", 2.0);
    rt.ensure_layout();
    assert_eq!(
        tops(&mut rt),
        [
            (0, 0, 550, 232),
            (0, 232, 550, 232),
            (0, 464, 275, 116),
            (550, 0, 275, 232)
        ]
    );

    rt.set_window_scale("main", 1.0);
    rt.set_window_scale("equalizer", 1.0);
    rt.ensure_layout();
    assert_eq!(tops(&mut rt), start);
}

#[test]
fn between_title_steps_nothing_is_due_and_only_the_clock_windows_draw_again() {
    use crate::classic::main::TITLE_STEP_MS;
    let now = Rc::new(std::cell::Cell::new(0i64));
    let at = now.clone();
    let clock: Clock = Rc::new(move || at.get());
    let mut player = FakePlayer::new(clock.clone());
    Player::set_track(
        &mut player,
        "A rather long song title that scrolls across the window",
        600_000,
    );
    let mut rt = classic_with(player, clock);
    assert!(rt.open_window("equalizer"));
    assert!(rt.open_window("pledit"));
    assert!(rt.tick(), "the first tick draws");
    let first = rt.draw_windows();
    now.set(TITLE_STEP_MS / 2);
    assert!(
        !rt.tick(),
        "halfway to the title's next step nothing changes"
    );
    now.set(TITLE_STEP_MS);
    assert!(rt.tick(), "the title steps");
    let stepped = rt.draw_windows();
    assert_ne!(
        first[0].1, stepped[0].1,
        "the main window drew its title on"
    );
    assert_eq!(first[1].1, stepped[1].1, "the equalizer kept its drawing");
    // what a clock-only draw kept is what a fresh drawing gives
    rt.repaint();
    assert_eq!(rt.draw_windows()[1].1, stepped[1].1);
    // a balance the host changes draws at once, playing or not
    rt.player.set_pan(60);
    assert!(rt.tick());
}
