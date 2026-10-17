use crate::common;

use omega_input::{Action, Btn, Millis, RawEvent};
use omega_store::Platform;
use omega_zero::app::Phase;
use omega_zero::screens::controls::ControlsView;
use omega_zero::session::Session;

fn on_shelf() -> (tempfile::TempDir, Session) {
    let d = common::tmp_root_with_carts(&["Emerald", "Fusion"]);
    let (s, _motor) = common::session_with_platform(d.path());
    assert!(
        matches!(s.app().phase(), Phase::Shelf),
        "not on the shelf: {:?}",
        s.app().phase()
    );
    (d, s)
}

fn feed(s: &mut Session, at: Millis, events: &[RawEvent]) {
    s.feed(events.iter().copied(), at);
}

fn tap_select(s: &mut Session, at: Millis) {
    feed(s, at, &[RawEvent::Down(Btn::Select)]);
    feed(s, at + 80, &[RawEvent::Up(Btn::Select)]);
    feed(s, at + 200, &[]);
}

#[test]
fn a_select_tap_on_the_shelf_opens_controls_and_another_closes_it() {
    let (_d, mut s) = on_shelf();
    tap_select(&mut s, 1_000);
    assert!(
        matches!(s.app().phase(), Phase::Controls),
        "a SELECT tap left {:?}",
        s.app().phase()
    );
    tap_select(&mut s, 2_000);
    assert!(
        matches!(s.app().phase(), Phase::Shelf),
        "a second SELECT tap left {:?}",
        s.app().phase()
    );
}

#[test]
fn b_closes_controls() {
    let (_d, mut s) = on_shelf();
    tap_select(&mut s, 1_000);
    assert!(matches!(s.app().phase(), Phase::Controls));
    s.app_mut().apply(Action::GbaDown(Btn::B));
    assert!(matches!(s.app().phase(), Phase::Shelf));
}

#[test]
fn a_select_held_for_the_game_opens_nothing() {
    let (_d, mut s) = on_shelf();
    feed(&mut s, 1_000, &[RawEvent::Down(Btn::Select)]);
    feed(&mut s, 1_700, &[]);
    feed(&mut s, 1_800, &[RawEvent::Up(Btn::Select)]);
    feed(&mut s, 2_000, &[]);
    assert!(
        matches!(s.app().phase(), Phase::Shelf),
        "a held SELECT left {:?}",
        s.app().phase()
    );
}

#[test]
fn a_select_chord_opens_nothing() {
    let (_d, mut s) = on_shelf();
    let before = s.app().brightness();
    feed(&mut s, 1_000, &[RawEvent::Down(Btn::Select)]);
    feed(&mut s, 1_050, &[RawEvent::Down(Btn::Up)]);
    feed(&mut s, 1_100, &[RawEvent::Up(Btn::Up)]);
    feed(&mut s, 1_150, &[RawEvent::Up(Btn::Select)]);
    feed(&mut s, 1_400, &[]);
    assert_ne!(s.app().brightness(), before, "the chord did not fire");
    assert!(
        matches!(s.app().phase(), Phase::Shelf),
        "SELECT+Up left {:?}",
        s.app().phase()
    );
}

#[test]
fn a_select_tap_in_a_game_opens_nothing() {
    let d = common::tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = common::app_playing_in(d.path(), "Emerald");
    a.apply(Action::GbaDown(Btn::Select));
    a.apply(Action::SelectTap);
    assert!(
        matches!(a.phase(), Phase::Playing { .. }),
        "a SELECT tap left the game for {:?}",
        a.phase()
    );
}

/// The shelf's lines for a system with a core to pick, the GBA's and the SNES's.
const SHELF_WITH_CORES: [(&str, &str); 8] = [
    ("A", "PLAY"),
    ("HOLD A", "CLEAN START"),
    ("X", "CORE"),
    ("START", "SETTINGS"),
    ("SELECT", "CONTROLS"),
    ("◀▶", "BROWSE"),
    ("▲▼", "LETTER"),
    ("L1 R1", "SHELF"),
];

/// What each button does on a GBA game's shelf and in one, as `App::apply` and the gestures do
/// it, in the words CONTROLS draws: trimmed to flow.html's grid, each column's actions as many
/// letters as it has room for.
#[test]
fn controls_says_what_the_shelf_and_a_gba_game_do_with_each_button() {
    let view = ControlsView::new(Platform::Gba);
    assert_eq!(view, ControlsView::default());
    assert_eq!(view.title, "CONTROLS");
    let names: Vec<_> = view.groups.iter().map(|g| g.name).collect();
    assert_eq!(names, ["SHELF", "IN GAME"]);
    assert_eq!(view.groups[0].lines, SHELF_WITH_CORES);
    assert_eq!(
        view.groups[1].lines,
        [
            ("L2", "REWIND"),
            ("R2", "FAST FORWARD"),
            ("R2 ×2", "LATCH FAST FWD"),
            ("MENU ×2", "SAVE STATES"),
            ("HOLD MENU", "EJECT"),
            ("SELECT+MENU", "LINK"),
            ("SELECT+▲▼", "BRIGHTNESS"),
            ("SELECT+◀▶", "BLUE LIGHT"),
            ("SELECT+L1", "LOAD STATE"),
            ("SELECT+R1", "SAVE STATE"),
            ("SELECT+Y", "COLOUR"),
            ("SELECT+X", "GHOSTING"),
            ("VOL+ VOL-", "MUTE"),
            ("HOLD POWER", "POWER MENU"),
        ]
    );
}

#[test]
fn a_game_boy_has_no_core_to_pick_and_leaves_l1_and_r1_to_the_picture() {
    for platform in [Platform::Gb, Platform::Gbc] {
        let view = ControlsView::new(platform);
        assert!(
            !view.groups[0].lines.contains(&("X", "CORE")),
            "{platform:?} offers a core to pick"
        );
        assert_eq!(view.groups[0].lines.len(), SHELF_WITH_CORES.len() - 1);
        let game = &view.groups[1].lines;
        for line in [
            ("L2", "REWIND"),
            ("SELECT+MENU", "LINK"),
            ("SELECT+Y", "COLOUR"),
            ("SELECT+X", "GHOSTING"),
            ("L1 R1", "PICTURE SIZE"),
        ] {
            assert!(game.contains(&line), "{platform:?} has no {line:?}");
        }
    }
}

#[test]
fn a_snes_game_rewinds_and_fast_forwards_but_neither_links_nor_corrects_colour() {
    let view = ControlsView::new(Platform::Snes);
    assert_eq!(view.groups[0].lines, SHELF_WITH_CORES);
    assert_eq!(
        view.groups[1].lines,
        [
            ("L2", "REWIND"),
            ("R2", "FAST FORWARD"),
            ("R2 ×2", "LATCH FAST FWD"),
            ("MENU ×2", "SAVE STATES"),
            ("HOLD MENU", "EJECT"),
            ("SELECT+▲▼", "BRIGHTNESS"),
            ("SELECT+◀▶", "BLUE LIGHT"),
            ("SELECT+L1", "LOAD STATE"),
            ("SELECT+R1", "SAVE STATE"),
            ("VOL+ VOL-", "MUTE"),
            ("HOLD POWER", "POWER MENU"),
        ]
    );
}

#[test]
fn a_playstation_game_fast_forwards_on_select_r2_and_swaps_discs() {
    let view = ControlsView::new(Platform::Ps1);
    assert!(!view.groups[0].lines.contains(&("X", "CORE")));
    assert_eq!(
        view.groups[1].lines,
        [
            ("SELECT+R2", "FAST FORWARD"),
            ("SELECT+R2 ×2", "LATCH FAST FWD"),
            ("MENU ×2", "SAVE STATES"),
            ("HOLD MENU", "EJECT"),
            ("SELECT+MENU", "DISC"),
            ("SELECT+▲▼", "BRIGHTNESS"),
            ("SELECT+◀▶", "BLUE LIGHT"),
            ("SELECT+L1", "LOAD STATE"),
            ("SELECT+R1", "SAVE STATE"),
            ("VOL+ VOL-", "MUTE"),
            ("HOLD POWER", "POWER MENU"),
        ]
    );
}

#[test]
fn every_systems_controls_are_in_letters_the_label_font_has() {
    for platform in Platform::ALL {
        let view = ControlsView::new(platform);
        for (keys, does) in view.groups.iter().flat_map(|g| g.lines.iter()) {
            assert!(
                omega_ui::text::label_font().is_some_and(|font| format!("{keys}  {does}")
                    .chars()
                    .all(|c| c == ' ' || font.lookup_glyph_index(c) != 0)),
                "{platform:?}'s {keys} {does} has a character the label font has no glyph for"
            );
        }
    }
}

#[test]
fn controls_speaks_for_the_shelf_it_was_opened_on() {
    let d = crate::keys::root_with_games(Platform::Ps1, &["Ridge Racer", "Wipeout"]);
    let (mut s, _motor) = common::session_with_platform(d.path());
    tap_select(&mut s, 1_000);
    assert!(matches!(s.app().phase(), Phase::Controls));
    assert_eq!(
        s.app().controls_view(),
        Some(ControlsView::new(Platform::Ps1))
    );
}
