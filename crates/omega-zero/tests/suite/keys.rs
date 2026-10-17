//! What the buttons the systems disagree about do, through the session as the handheld drives
//! it: a PlayStation game is handed L2 and R2 and fast-forwards on SELECT+R2, with nothing to
//! rewind it; a SNES game rewinds and fast-forwards on L2 and R2 as a GBA game does.

use crate::common;

use std::time::{Duration, Instant};

use omega_input::{Btn, Keys, Millis, RawEvent};
use omega_retro::ButtonMask;
use omega_store::{write_app_state, AppState, Platform};
use omega_ui::{CornerBadge, HudKind};
use omega_zero::app::Phase;
use omega_zero::session::Session;
use tempfile::TempDir;

/// A card with one game of `platform` on it, `stem`, its file as the shelf finds it.
pub fn root_with(platform: Platform, stem: &str) -> TempDir {
    root_with_games(platform, &[stem])
}

/// A card with the games `stems` of `platform` on it. With two or more the handheld boots onto
/// the shelf; a card with one game puts it straight in.
pub fn root_with_games(platform: Platform, stems: &[&str]) -> TempDir {
    let d = common::tmp_root_with_carts(&[]);
    let dir = d.path().join("Games").join(platform.dir_name());
    std::fs::create_dir_all(&dir).expect("the system's folder");
    for stem in stems {
        let rom = dir.join(format!("{stem}.{}", platform.extensions()[0]));
        std::fs::write(rom, vec![0u8; 0x8000]).expect("write the game");
    }
    d
}

/// A session playing `stem`, of `platform`, on the mock core, with the game drawn.
fn playing(d: &TempDir, platform: Platform, stem: &str) -> (Session, Millis) {
    write_app_state(
        d.path(),
        &AppState {
            cart: Some(stem.into()),
            cart_platform: Some(platform),
            clock_set: true,
            utc_offset_min: 0,
            ..Default::default()
        },
    )
    .expect("write omega-zero.state");
    let mut s = Session::boot(d.path().to_path_buf());
    let mut now: Millis = 0;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !(matches!(s.app().phase(), Phase::Playing { .. }) && s.game_visible()) {
        assert!(Instant::now() < deadline, "the game never came up");
        step(&mut s, &mut now, None);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(s.app().platform(), platform);
    (s, now)
}

fn step(s: &mut Session, now: &mut Millis, ev: Option<RawEvent>) {
    *now += 16;
    s.feed(ev, *now);
    s.update(1.0 / 60.0);
}

fn held(s: &Session) -> u16 {
    s.emu().expect("a game has a core").input().0
}

fn rewinding(s: &Session) -> bool {
    s.app()
        .hud()
        .gauge()
        .is_some_and(|g| g.kind == HudKind::Rewind)
}

#[test]
fn each_system_says_what_its_buttons_do() {
    let d = common::tmp_root_with_carts(&["Emerald"]);
    let mut a = common::boot(d.path());
    let keys = |a: &mut omega_zero::app::App, p: Platform| {
        a.set_platform(p);
        a.keys()
    };
    for p in [Platform::Gba, Platform::Gb, Platform::Gbc] {
        assert_eq!(keys(&mut a, p), Keys::HANDHELD, "{p:?}");
    }
    assert_eq!(
        keys(&mut a, Platform::Snes),
        Keys {
            game_l2_r2: false,
            rewind: true,
            colour: false,
            ghosting: false,
        }
    );
    assert_eq!(
        keys(&mut a, Platform::Ps1),
        Keys {
            game_l2_r2: true,
            rewind: false,
            colour: false,
            ghosting: false,
        }
    );
}

#[test]
fn a_playstation_game_is_handed_l2_and_r2_and_never_rewinds() {
    let d = root_with(Platform::Ps1, "Ridge Racer");
    let (mut s, mut now) = playing(&d, Platform::Ps1, "Ridge Racer");

    step(&mut s, &mut now, Some(RawEvent::Down(Btn::L2)));
    for _ in 0..30 {
        step(&mut s, &mut now, None);
    }
    assert_ne!(held(&s) & ButtonMask::L2, 0, "L2 never reached the game");
    assert!(!rewinding(&s), "L2 rewound a PlayStation game");
    step(&mut s, &mut now, Some(RawEvent::Up(Btn::L2)));
    assert_eq!(held(&s) & ButtonMask::L2, 0, "the game was left holding L2");

    step(&mut s, &mut now, Some(RawEvent::Down(Btn::R2)));
    assert_ne!(held(&s) & ButtonMask::R2, 0, "R2 never reached the game");
    assert_eq!(s.app().ff_badge(), None, "R2 alone fast-forwarded");
    step(&mut s, &mut now, Some(RawEvent::Up(Btn::R2)));
    assert_eq!(held(&s) & ButtonMask::R2, 0, "the game was left holding R2");
}

#[test]
fn select_and_r2_fast_forward_a_playstation_game() {
    let d = root_with(Platform::Ps1, "Ridge Racer");
    let (mut s, mut now) = playing(&d, Platform::Ps1, "Ridge Racer");

    step(&mut s, &mut now, Some(RawEvent::Down(Btn::Select)));
    step(&mut s, &mut now, Some(RawEvent::Down(Btn::R2)));
    assert!(
        matches!(s.app().ff_badge(), Some(CornerBadge::FastForward { .. })),
        "SELECT+R2 did not fast-forward: {:?}",
        s.app().ff_badge()
    );
    assert_eq!(
        held(&s) & (ButtonMask::R2 | ButtonMask::SELECT),
        0,
        "the chord reached the game"
    );
    step(&mut s, &mut now, Some(RawEvent::Up(Btn::R2)));
    step(&mut s, &mut now, Some(RawEvent::Up(Btn::Select)));
    assert_eq!(s.app().ff_badge(), None, "the fast forward outlived R2");
}

#[test]
fn a_snes_game_rewinds_on_l2_and_fast_forwards_on_r2() {
    let d = root_with(Platform::Snes, "Super Metroid");
    let (mut s, mut now) = playing(&d, Platform::Snes, "Super Metroid");

    step(&mut s, &mut now, Some(RawEvent::Down(Btn::R2)));
    assert!(s.app().ff_badge().is_some(), "R2 did not fast-forward");
    assert_eq!(held(&s) & ButtonMask::R2, 0, "R2 reached the game");
    step(&mut s, &mut now, Some(RawEvent::Up(Btn::R2)));
    assert_eq!(s.app().ff_badge(), None);

    step(&mut s, &mut now, Some(RawEvent::Down(Btn::L2)));
    for _ in 0..30 {
        step(&mut s, &mut now, None);
    }
    assert!(rewinding(&s), "L2 did not rewind a SNES game");
    assert_eq!(held(&s) & ButtonMask::L2, 0, "L2 reached the game");
    step(&mut s, &mut now, Some(RawEvent::Up(Btn::L2)));
}

#[test]
fn a_gba_game_keeps_l2_and_r2_from_the_game() {
    let d = common::tmp_root_with_carts(&["Emerald"]);
    let (mut s, mut now) = playing(&d, Platform::Gba, "Emerald");
    step(&mut s, &mut now, Some(RawEvent::Down(Btn::R2)));
    assert!(s.app().ff_badge().is_some(), "R2 did not fast-forward");
    step(&mut s, &mut now, Some(RawEvent::Down(Btn::L2)));
    assert_eq!(
        held(&s) & (ButtonMask::L2 | ButtonMask::R2),
        0,
        "a GBA game was handed a shoulder it does not have"
    );
}

#[test]
fn select_and_x_toggle_a_handheld_games_ghosting_and_never_reach_it() {
    let d = root_with(Platform::Gba, "Emerald");
    let (mut s, mut now) = playing(&d, Platform::Gba, "Emerald");
    let before = s.app().lcd_ghosting();
    step(&mut s, &mut now, Some(RawEvent::Down(Btn::Select)));
    step(&mut s, &mut now, Some(RawEvent::Down(Btn::X)));
    assert_eq!(s.app().lcd_ghosting(), !before, "SELECT+X did not flip it");
    assert_eq!(
        held(&s) & (ButtonMask::X | ButtonMask::SELECT),
        0,
        "the chord reached the game"
    );
    assert_eq!(
        s.app().toast(),
        Some(match before {
            true => omega_ui::Toast::GhostingOff,
            false => omega_ui::Toast::GhostingOn,
        })
    );
    step(&mut s, &mut now, Some(RawEvent::Up(Btn::X)));
    step(&mut s, &mut now, Some(RawEvent::Up(Btn::Select)));
    for _ in 0..10 {
        step(&mut s, &mut now, None);
    }
    assert_eq!(held(&s), 0, "the chord left a button held");
    assert_eq!(
        omega_store::read_app_state(d.path()).lcd_ghosting,
        !before,
        "the shortcut never reached the card"
    );
    step(&mut s, &mut now, Some(RawEvent::Down(Btn::X)));
    assert_ne!(
        held(&s) & ButtonMask::X,
        0,
        "X alone never reached the game"
    );
}

#[test]
fn select_and_x_are_a_snes_games_own() {
    let d = root_with(Platform::Snes, "Mario");
    let (mut s, mut now) = playing(&d, Platform::Snes, "Mario");
    let before = s.app().lcd_ghosting();
    step(&mut s, &mut now, Some(RawEvent::Down(Btn::Select)));
    step(&mut s, &mut now, Some(RawEvent::Down(Btn::X)));
    assert_eq!(
        s.app().lcd_ghosting(),
        before,
        "a SNES game's SELECT+X was taken"
    );
    assert_eq!(
        held(&s) & (ButtonMask::X | ButtonMask::SELECT),
        ButtonMask::X | ButtonMask::SELECT,
        "SELECT+X never reached the game"
    );
}
