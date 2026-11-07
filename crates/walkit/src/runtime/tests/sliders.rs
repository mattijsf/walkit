// SPDX-License-Identifier: GPL-3.0-or-later

//! Modern sliders dragged as Winamp 5.666 drags them: values read back from Winamp Modern in
//! the VM (balance 77 x 13 with a 23-wide thumb, an EQ band 13 x 80 with a 23-tall thumb).

use super::runtime;
use crate::runtime::SkinRuntime;

const PLAYER_XML: &str = r#"<elements>
<bitmap id="thumb.h" file="h.png" x="0" y="0" w="23" h="13"/>
<bitmap id="thumb.v" file="v.png" x="0" y="0" w="13" h="23"/>
</elements>
<container id="main" name="t" default_visible="1"><layout id="normal" w="200" h="100">
<slider id="balance" action="PAN" x="0" y="0" w="77" h="13" thumb="thumb.h"/>
<slider id="volume" action="VOLUME" x="0" y="20" w="77" h="13" thumb="thumb.h"/>
<slider id="eq1" action="EQ_BAND" param="1" x="100" y="0" w="13" h="80" orientation="vertical" thumb="thumb.v"/>
<slider id="eq2" action="EQ_BAND" param="2" x="118" y="0" w="13" h="80" orientation="vertical" thumb="thumb.v"/>
<slider id="eq3" action="EQ_BAND" param="3" x="136" y="0" w="13" h="80" orientation="vertical" thumb="thumb.v"/>
<slider id="preamp" action="EQ_BAND" param="preamp" x="160" y="0" w="13" h="80" orientation="vertical" thumb="thumb.v"/>
</layout></container>"#;

fn loaded() -> SkinRuntime {
    let mut rt = runtime(PLAYER_XML);
    rt.load();
    rt.ensure_layout();
    rt
}

#[test]
fn the_balance_snaps_to_its_middle_within_its_reach() {
    let mut rt = loaded();
    // taken beside the thumb at the far left, then dragged right a pixel at a time: the VM
    // read -24, -19, then 0 for 7 pixels, then 19, 24
    rt.pointer_down(5, 6, 1);
    assert_eq!(rt.player.pan(), -127);
    let mut seen = Vec::new();
    for x in 30..=42 {
        rt.pointer_move(x, 6);
        seen.push(rt.player.pan());
    }
    rt.pointer_up(42, 6);
    assert_eq!(seen, [-28, -24, -19, 0, 0, 0, 0, 0, 0, 0, 19, 24, 28]);
}

#[test]
fn an_eq_band_snaps_to_flat_within_its_reach() {
    let mut rt = loaded();
    // dragged down a pixel at a time: flat for 8 pixels, as in the VM
    rt.pointer_down(106, 5, 1);
    let mut flat = Vec::new();
    for y in 30..=45 {
        rt.pointer_move(106, y);
        if rt.player.eq_bands()[0] == crate::runtime::EQ_FLAT {
            flat.push(y);
        }
    }
    rt.pointer_up(106, 45);
    assert_eq!(flat, (34..=41).collect::<Vec<_>>());
}

#[test]
fn the_volume_has_no_spot_to_snap_to() {
    let mut rt = loaded();
    rt.pointer_down(5, 26, 1);
    let mut seen = Vec::new();
    for x in 33..=39 {
        rt.pointer_move(x, 26);
        seen.push(rt.player.volume());
    }
    rt.pointer_up(39, 26);
    assert_eq!(seen, [113, 118, 122, 127, 132, 136, 141]);
}

#[test]
fn a_drag_across_the_eq_bands_hands_on_from_band_to_band() {
    // as Winamp Modern in the VM: band 1 dragged, then sideways onto band 2 and band 3, each
    // letting go as the next takes the drag
    let mut rt = loaded();
    let bands = |rt: &SkinRuntime| rt.player.eq_bands()[..3].to_vec();
    rt.pointer_down(106, 30, 1);
    rt.pointer_move(106, 25);
    let one = bands(&rt)[0];
    // between the bands band 1 keeps the drag
    rt.pointer_move(115, 30);
    let left = bands(&rt)[0];
    assert_ne!(left, one, "band 1 still follows the finger");
    rt.pointer_move(124, 25);
    let after = bands(&rt);
    assert_eq!(
        (after[0], after[1]),
        (left, one),
        "band 2 takes the drag where it was reached"
    );
    rt.pointer_move(124, 45);
    assert_eq!(bands(&rt)[0], left, "band 1 let go");
    assert!(bands(&rt)[1] < one, "band 2 follows the finger down");
    rt.pointer_move(142, 55);
    assert!(bands(&rt)[2] < bands(&rt)[1], "band 3 takes the drag");
    rt.pointer_up(142, 55);
}

#[test]
fn the_preamp_keeps_its_drag_over_the_bands() {
    let mut rt = loaded();
    let flat = rt.player.eq_bands()[0];
    let preamp = rt.player.eq_preamp();
    rt.pointer_down(166, 70, 1);
    rt.pointer_move(106, 70);
    assert_eq!(
        rt.player.eq_bands()[0],
        flat,
        "the band takes no drag from the preamp"
    );
    assert_ne!(rt.player.eq_preamp(), preamp);
    rt.pointer_up(106, 70);
}
