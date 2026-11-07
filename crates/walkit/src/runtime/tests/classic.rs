// SPDX-License-Identifier: GPL-3.0-or-later
//! A classic skin's main window, drawn pixel for pixel as Winamp draws it (as
//! Winamp 5.666 shows it in Winamp 5.666 in a VM).

use std::rc::Rc;

use super::QuietHost;
use crate::draw::{Align, DrawCmd, Rect, Region};
use crate::files::MemoryFiles;
use crate::host::{Clock, SkinSetup};
use crate::player::{FakePlayer, Player, Status, Track};
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
    // the first band at x 78 under the main window; the thumb's top at the finger less 5
    rt.pointer_down(85, 116 + 43, 2);
    assert_eq!(rt.player.eq_bands()[0], 255, "the top is full boost");
    rt.pointer_move(85, 116 + 95);
    assert_eq!(rt.player.eq_bands()[0], 0, "the bottom is full cut");
    rt.pointer_up(85, 116 + 95);
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
    rt.pointer_down(97, 39, 2);
    rt.pointer_up(97, 39);
    let cmds = rt.draw_main();
    assert!(!blits(&cmds, "numbers.bmp").contains(&(20, 6, 5, 1, 38, 32)));
    rt.pointer_down(98, 32, 3);
    rt.pointer_up(98, 32);
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
        DrawCmd::Fill { rect, .. } => rect.top == 34 && rect.height == 14,
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
    assert_eq!(size, (295, 146), "grown by the drag");
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
    // 116 tall: four rows show, sixteen lie past the window; the thumb starts at the top
    assert_eq!(SkinRuntime::classic_pl_rows(116), 4);
    assert_eq!(SkinRuntime::classic_pl_thumb(116, 20, 0), 0);
    assert_eq!(SkinRuntime::classic_pl_thumb(116, 20, 16), 40);
    assert_eq!(SkinRuntime::classic_pl_thumb(116, 20, 8), 20);
    // a drag up on the rows by two rows scrolls two rows; the release picks nothing
    rt.pointer_down(60, 116 + 60, 1);
    rt.pointer_move(60, 116 + 32);
    rt.pointer_up(60, 116 + 32);
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
    assert_eq!(texts.len(), 8, "four rows, each a title and a length");
    assert!(blits(cmds, "pledit.bmp").contains(&(52, 53, 8, 18, 260, 25)));
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
    assert_eq!(rt.classic_pl_scroll, 16);
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
    assert_eq!(rt.next_due_at(), None);
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
    // thin bars: one band per column; no peaks when they are off
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
    tap(&mut rt, 239, 103, 5);
    assert!(rt.player.repeat(), "repeat reaches x 239, y 103");
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
    tap(&mut rt, 83, 106, 2);
    assert_eq!(
        rt.player.status(),
        Status::Stopped,
        "x 83 is stop's first column"
    );
    tap(&mut rt, 60, 86, 3);
    assert_eq!(
        rt.player.status(),
        Status::Stopped,
        "y 86 lies above the buttons"
    );
    tap(&mut rt, 60, 106, 4);
    assert_eq!(rt.player.status(), Status::Playing);
    tap(&mut rt, 82, 107, 5);
    assert_eq!(
        rt.player.status(),
        Status::Playing,
        "y 107 lies below the buttons"
    );
    tap(&mut rt, 82, 97, 6);
    assert_eq!(
        rt.player.status(),
        Status::Paused,
        "x 82 is pause's last column"
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
