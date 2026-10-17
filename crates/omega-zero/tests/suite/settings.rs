use crate::common;

use common::{
    app_booting_at, app_booting_with_clock, app_playing_in, tmp_root_with_carts, Clock,
    CLOCK_IS_SET,
};
use std::sync::{Arc, Mutex};

use omega_input::{Action, Btn};
use omega_store::{read_app_state, write_app_state, AppState};
use omega_ui::kit::layout::modal_rect;
use omega_ui::kit::palette::palette;
use omega_ui::kit::row::ROW_H;
use omega_ui::{
    date_time_text, out_w, settings_frame, settings_row_rects, Draw, HudFace, QuickValue,
    SettingsFaces, SettingsGroup, SettingsRow, TexId, SPRITE_PX,
};
use omega_zero::app::{App, Phase};
use omega_zero::wifi::{Status, WifiJob, WifiJobs, WifiNews};
use omega_zero::wifi_screen::Summary;
use tempfile::TempDir;

fn on_carousel_with(state: AppState) -> (TempDir, App, Clock) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_app_state(
        d.path(),
        &AppState {
            clock_set: true,
            ..state
        },
    )
    .expect("write omega-zero.state");
    let (a, clock) = app_booting_at(d.path(), CLOCK_IS_SET);
    assert!(
        matches!(a.phase(), Phase::Shelf),
        "not on the carousel: {:?}",
        a.phase()
    );
    (d, a, clock)
}

fn on_carousel() -> (TempDir, App, Clock) {
    on_carousel_with(AppState::default())
}

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn open_at(a: &mut App, row: SettingsRow) {
    press(a, Btn::Start);
    for _ in 0..row.index() {
        press(a, Btn::Down);
    }
    assert_eq!(a.settings_row(), Some(row), "the bar never reached {row:?}");
    a.finish_slide();
}

#[test]
fn start_opens_settings_with_its_top_row_selected_every_time() {
    let (_d, mut a, _) = on_carousel();
    press(&mut a, Btn::Start);
    assert_eq!(a.settings_row(), Some(SettingsRow::FastForward));
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Start);
    press(&mut a, Btn::Start);
    assert_eq!(
        a.settings_row(),
        Some(SettingsRow::FastForward),
        "the menu opened where it was last left"
    );
}

#[test]
fn menu_opens_nothing_on_the_carousel() {
    let (_d, mut a, _) = on_carousel();
    a.apply(Action::QuickMenu);
    assert!(
        matches!(a.phase(), Phase::Shelf),
        "MENU still opened {:?}",
        a.phase()
    );
    assert_eq!(a.settings_row(), None);
}

#[test]
fn start_b_or_menu_closes_it_back_onto_the_carousel_where_you_were() {
    for close in [
        Action::GbaDown(Btn::Start),
        Action::GbaDown(Btn::B),
        Action::QuickMenu,
    ] {
        let (_d, mut a, _) = on_carousel();
        press(&mut a, Btn::Right);
        assert_eq!(
            a.selected_stem(),
            Some("Fusion"),
            "the carousel never moved"
        );
        press(&mut a, Btn::Start);
        a.apply(close);
        assert!(
            matches!(a.phase(), Phase::Shelf),
            "{close:?} left {:?}",
            a.phase()
        );
        assert_eq!(a.selected_stem(), Some("Fusion"), "{close:?} lost the cart");
    }
}

#[test]
fn settings_is_only_on_the_carousel() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    for button in [Action::GbaDown(Btn::Start), Action::QuickMenu] {
        a.apply(button);
        assert!(
            matches!(a.phase(), Phase::Playing { .. }),
            "{button:?} raised Settings over a game: {:?}",
            a.phase()
        );
    }
    assert_eq!(a.settings_row(), None);
}

#[test]
fn up_and_down_move_the_bar_and_wrap_at_the_ends() {
    let (_d, mut a, _) = on_carousel();
    press(&mut a, Btn::Start);
    press(&mut a, Btn::Up);
    assert_eq!(
        a.settings_row(),
        Some(SettingsRow::About),
        "up from the top did not wrap to the bottom"
    );
    for want in [
        SettingsRow::FastForward,
        SettingsRow::FastForwardSound,
        SettingsRow::ColourCorrection,
        SettingsRow::LcdGhosting,
        SettingsRow::Rumble,
        SettingsRow::Labels,
        SettingsRow::DateTime,
        SettingsRow::Network,
        SettingsRow::Bluetooth,
        SettingsRow::Background,
        SettingsRow::About,
        SettingsRow::FastForward,
    ] {
        press(&mut a, Btn::Down);
        assert_eq!(a.settings_row(), Some(want));
    }
}

#[test]
fn fast_forward_steps_through_its_speeds_and_saves_each_one() {
    let (d, mut a, _) = on_carousel();
    open_at(&mut a, SettingsRow::FastForward);
    for (btn, want) in [
        (Btn::Left, 4),
        (Btn::Left, 3),
        (Btn::Left, 2),
        (Btn::Left, 2),
        (Btn::Right, 3),
        (Btn::Right, 4),
        (Btn::Right, 6),
        (Btn::Right, 6),
        (Btn::Left, 4),
    ] {
        press(&mut a, btn);
        assert_eq!(a.ff_speed(), want, "{btn:?}");
        assert_eq!(read_app_state(d.path()).ff_speed, want, "not on the card");
        assert_eq!(
            a.settings_value(SettingsRow::FastForward),
            QuickValue::speed(want)
        );
    }
}

#[test]
fn rumble_and_fast_forward_sound_flip_on_either_arrow_and_save() {
    let (d, mut a, _) = on_carousel();
    let card = |d: &TempDir| {
        let s = read_app_state(d.path());
        (s.ff_sound, s.rumble)
    };
    open_at(&mut a, SettingsRow::FastForwardSound);
    press(&mut a, Btn::Left);
    assert_eq!(card(&d), (true, true));
    assert_eq!(
        a.settings_value(SettingsRow::FastForwardSound),
        Some(QuickValue::On)
    );
    press(&mut a, Btn::Right);
    assert_eq!(card(&d), (false, true));
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Right);
    assert_eq!(card(&d), (false, false));
    assert!(!a.rumble_enabled());
    assert_eq!(a.settings_value(SettingsRow::Rumble), Some(QuickValue::Off));
    press(&mut a, Btn::Left);
    assert_eq!(card(&d), (false, true));
}

#[test]
fn colour_correction_flips_on_either_arrow_and_saves() {
    let (d, mut a, _) = on_carousel();
    open_at(&mut a, SettingsRow::ColourCorrection);
    assert!(
        !a.colour_correction(),
        "the row did not open on the default, which is off"
    );
    assert_eq!(
        a.settings_value(SettingsRow::ColourCorrection),
        Some(QuickValue::Off)
    );
    for (btn, want) in [
        (Btn::Right, true),
        (Btn::Left, false),
        (Btn::Left, true),
        (Btn::Right, false),
    ] {
        press(&mut a, btn);
        assert_eq!(a.colour_correction(), want, "{btn:?}");
        assert_eq!(
            read_app_state(d.path()).colour_correction,
            want,
            "{btn:?} never reached the card"
        );
        assert_eq!(
            a.settings_value(SettingsRow::ColourCorrection),
            Some(QuickValue::flag(want))
        );
    }
}

#[test]
fn lcd_ghosting_is_on_by_default_flips_on_either_arrow_and_saves() {
    let (d, mut a, _) = on_carousel();
    open_at(&mut a, SettingsRow::LcdGhosting);
    assert!(
        a.lcd_ghosting(),
        "the row did not open on the default, which is on"
    );
    assert_eq!(
        a.settings_value(SettingsRow::LcdGhosting),
        Some(QuickValue::On)
    );
    assert_eq!(SettingsRow::LcdGhosting.label(), "LCD Ghosting");
    assert_eq!(SettingsRow::LcdGhosting.group(), SettingsGroup::Configs);
    assert!(SettingsRow::LcdGhosting.carets() && !SettingsRow::LcdGhosting.opens());
    for (btn, want) in [
        (Btn::Right, false),
        (Btn::Left, true),
        (Btn::Left, false),
        (Btn::Right, true),
    ] {
        press(&mut a, btn);
        assert_eq!(a.lcd_ghosting(), want, "{btn:?}");
        assert_eq!(
            read_app_state(d.path()).lcd_ghosting,
            want,
            "{btn:?} never reached the card"
        );
        assert_eq!(
            a.settings_value(SettingsRow::LcdGhosting),
            Some(QuickValue::flag(want))
        );
        assert_eq!(
            a.take_lcd_ghosting(),
            Some(want),
            "never queued for the core"
        );
        assert_eq!(a.take_colour_correction(), None);
    }
}

#[test]
fn lcd_ghosting_stands_under_colour_correction() {
    let at = |r| SettingsRow::ALL.iter().position(|x| *x == r).unwrap();
    assert_eq!(
        at(SettingsRow::LcdGhosting),
        at(SettingsRow::ColourCorrection) + 1
    );
}

#[test]
fn colour_correction_leaves_the_settings_around_it_alone() {
    let (d, mut a, _) = on_carousel();
    open_at(&mut a, SettingsRow::ColourCorrection);
    press(&mut a, Btn::Right);
    let s = read_app_state(d.path());
    assert!(s.colour_correction, "the row never took");
    assert_eq!(
        (s.ff_speed, s.ff_sound, s.rumble),
        (
            AppState::default().ff_speed,
            AppState::default().ff_sound,
            AppState::default().rumble
        ),
        "the row reached a setting that is not its own"
    );
}

#[test]
fn the_arrows_change_nothing_on_a_row_that_opens() {
    let (d, mut a, _) = on_carousel();
    let before =
        std::fs::read(d.path().join("Config/omega-zero.state")).expect("read omega-zero.state");
    for row in [SettingsRow::DateTime, SettingsRow::About] {
        open_at(&mut a, row);
        press(&mut a, Btn::Left);
        press(&mut a, Btn::Right);
        assert_eq!(a.settings_row(), Some(row), "an arrow left {row:?}");
        a.apply(Action::QuickMenu);
    }
    assert_eq!(
        std::fs::read(d.path().join("Config/omega-zero.state")).expect("read omega-zero.state"),
        before,
        "an arrow on a row that opens wrote the card"
    );
}

#[test]
fn a_on_about_opens_the_label_and_b_comes_back_to_the_menu() {
    let (_d, mut a, _) = on_carousel();
    open_at(&mut a, SettingsRow::About);
    press(&mut a, Btn::A);
    assert!(matches!(a.phase(), Phase::About), "{:?}", a.phase());
    press(&mut a, Btn::B);
    assert_eq!(a.settings_row(), Some(SettingsRow::About));
}

#[test]
fn a_on_date_and_time_opens_the_clock_at_the_time_it_already_has() {
    let (_d, mut a, _) = on_carousel_with(AppState {
        utc_offset_min: -300,
        ..AppState::default()
    });
    open_at(&mut a, SettingsRow::DateTime);
    press(&mut a, Btn::A);
    let picker = a.picker().expect("A on Date & Time did not open the clock");
    assert_eq!(picker.offset_min(), -300, "the offset started over");
    assert_eq!(
        picker.secs(),
        CLOCK_IS_SET - CLOCK_IS_SET % 60,
        "the picker does not show the minute the clock is in"
    );
}

#[test]
fn confirming_the_clock_from_the_menu_sets_it_and_comes_back_to_the_menu() {
    let (d, mut a, clock) = on_carousel_with(AppState {
        utc_offset_min: -300,
        ..AppState::default()
    });
    open_at(&mut a, SettingsRow::DateTime);
    press(&mut a, Btn::A);
    press(&mut a, Btn::Up);
    for _ in 0..5 {
        press(&mut a, Btn::Right);
    }
    press(&mut a, Btn::Down);
    let changed = a.picker().expect("not on the clock").secs() - (CLOCK_IS_SET - CLOCK_IS_SET % 60);
    press(&mut a, Btn::A);
    assert_eq!(
        a.settings_row(),
        Some(SettingsRow::DateTime),
        "{:?}",
        a.phase()
    );
    assert_eq!(
        clock.get(),
        CLOCK_IS_SET + changed,
        "the platform clock was not set"
    );
    let s = read_app_state(d.path());
    assert_eq!(s.utc_offset_min, -330, "the offset was not saved");
    assert!(s.clock_set);
}

#[test]
fn b_on_the_clock_from_the_menu_comes_back_without_changing_anything() {
    let (d, mut a, clock) = on_carousel_with(AppState {
        utc_offset_min: -300,
        ..AppState::default()
    });
    open_at(&mut a, SettingsRow::DateTime);
    press(&mut a, Btn::A);
    press(&mut a, Btn::Up);
    for _ in 0..5 {
        press(&mut a, Btn::Right);
    }
    press(&mut a, Btn::Down);
    press(&mut a, Btn::B);
    assert_eq!(
        a.settings_row(),
        Some(SettingsRow::DateTime),
        "{:?}",
        a.phase()
    );
    assert_eq!(clock.get(), CLOCK_IS_SET, "B set the clock");
    assert_eq!(
        read_app_state(d.path()).utc_offset_min,
        -300,
        "B saved the offset"
    );
}

#[test]
fn the_first_boot_clock_still_has_no_way_back() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let (mut a, _) = app_booting_with_clock(d.path());
    press(&mut a, Btn::B);
    a.apply(Action::QuickMenu);
    press(&mut a, Btn::Start);
    assert!(
        matches!(a.phase(), Phase::SetClock { .. }),
        "left the first boot clock: {:?}",
        a.phase()
    );
}

#[test]
fn brightness_and_volume_still_answer_over_settings() {
    let (d, mut a, _) = on_carousel();
    open_at(&mut a, SettingsRow::FastForward);
    let before = read_app_state(d.path());
    a.apply(Action::BrightnessUp);
    a.apply(Action::VolumeDown);
    let after = read_app_state(d.path());
    assert_eq!(
        (after.brightness, after.volume),
        (before.brightness + 1, before.volume - 5)
    );
    assert_eq!(
        a.settings_row(),
        Some(SettingsRow::FastForward),
        "a level moved the menu"
    );
    let gauge = a.hud().gauge().expect("the level shows no gauge");
    let tex = TexId::from_raw(700);
    let face = HudFace { tex, w: 157, h: 15 };
    a.hud_mut().set_gauge_face(gauge, face);
    let out = frame(&a);
    assert!(
        out.iter()
            .any(|d| matches!(*d, Draw::Tex { tex: t, .. } if t == tex)),
        "the level's gauge is not drawn over the menu"
    );
}

/// Settings' faces as fake textures where the real ones go: the frame 100, the lit titles 110
/// and 111, and each row 200 and its index.
fn fake_faces(a: &mut App) -> SettingsFaces {
    let id = TexId::from_raw;
    let frame = settings_frame(6, palette(0));
    let mut faces = SettingsFaces::new(&frame, id(100), [(id(110), [42, 11]), (id(111), [42, 11])]);
    for row in a.settings_rows() {
        faces.rows[row.index()] = Some((id(200 + row.index()), [270, ROW_H]));
    }
    a.set_settings_faces(faces.clone());
    faces
}

fn frame(a: &App) -> Vec<Draw> {
    let mut out = Vec::new();
    a.draw(&mut out);
    out
}

fn placed(out: &[Draw], id: usize) -> Option<[f32; 4]> {
    out.iter().find_map(|d| match *d {
        Draw::Tex {
            x, y, w, h, tex, ..
        } if tex == TexId::from_raw(id) => Some([x, y, w, h]),
        _ => None,
    })
}

fn drawn(out: &[Draw], id: usize) -> bool {
    placed(out, id).is_some()
}

/// An art-pixel rect as the screen rect a face drawn there at 2x covers.
fn at_2x([x, y, w, h]: [u32; 4]) -> [f32; 4] {
    [x, y, w, h].map(|v| (v * SPRITE_PX) as f32)
}

#[test]
fn the_modal_fills_the_page_and_the_lit_title_follows_the_cursor() {
    let (_d, mut a, _) = on_carousel();
    let faces = fake_faces(&mut a);
    press(&mut a, Btn::Start);
    a.finish_slide();
    for row in a.settings_rows() {
        assert_eq!(a.settings_row(), Some(row));
        let out = frame(&a);
        assert_eq!(
            placed(&out, 100),
            Some(at_2x(modal_rect())),
            "{row:?}: the frame"
        );
        let (configs, general) = (drawn(&out, 110), drawn(&out, 111));
        match row.group() {
            SettingsGroup::Configs => assert!(configs && !general, "{row:?} lit GENERAL"),
            SettingsGroup::General => assert!(general && !configs, "{row:?} lit CONFIGS"),
        }
        let g = row.group().index();
        assert_eq!(placed(&out, 110 + g), Some(at_2x(faces.titles[g].at)));
        press(&mut a, Btn::Down);
    }
}

#[test]
fn the_title_lights_over_the_frame_and_the_rows_over_both() {
    let (_d, mut a, _) = on_carousel();
    fake_faces(&mut a);
    open_at(&mut a, SettingsRow::FastForward);
    let out = frame(&a);
    let at = |id: usize| {
        out.iter()
            .position(|d| matches!(*d, Draw::Tex { tex, .. } if tex == TexId::from_raw(id)))
            .unwrap_or_else(|| panic!("{id} is not drawn"))
    };
    assert!(at(100) < at(110), "the lit title is under the frame");
    assert!(at(110) < at(200), "a row is under the title");
}

#[test]
fn every_row_is_drawn_in_its_panels_well() {
    let (_d, mut a, _) = on_carousel();
    let faces = fake_faces(&mut a);
    open_at(&mut a, SettingsRow::FastForward);
    let out = frame(&a);
    for (row, rect) in settings_row_rects(&a.settings_rows(), faces.wells) {
        let [x, y, ..] = at_2x(rect);
        assert_eq!(
            placed(&out, 200 + row.index()),
            Some([x, y, 540.0, (2 * ROW_H) as f32]),
            "{row:?}"
        );
        let [wx, wy, ww, wh] = faces.wells[row.group().index()];
        assert!(
            rect[1] >= wy && rect[1] + ROW_H <= wy + wh && rect[0] + 1 == wx && rect[2] == ww + 2,
            "{row:?} is not in its well"
        );
    }
}

#[test]
fn settings_no_longer_lays_a_plate_over_the_backdrop() {
    let (_d, mut a, _) = on_carousel();
    fake_faces(&mut a);
    open_at(&mut a, SettingsRow::FastForward);
    let see_through = frame(&a).into_iter().any(
        |d| matches!(d, Draw::Rect { w, h, colour, .. } if w >= out_w() as f32 && h >= 100.0 && colour[3] < 1.0),
    );
    assert!(
        !see_through,
        "a plate still darkens the backdrop under Settings"
    );
}

#[test]
fn the_rows_say_what_is_set_in_flows_capitals() {
    let (_d, mut a, clock) = on_carousel_with(AppState {
        ff_speed: 4,
        ff_sound: true,
        colour_correction: false,
        rumble: true,
        ..AppState::default()
    });
    open_at(&mut a, SettingsRow::FastForward);
    let said = |a: &App| SettingsRow::ALL.map(|r| a.settings_text(r));
    let text = |s: &str| Some(s.to_string());
    let clock_text = text(&date_time_text(a.wall_secs()));
    assert_eq!(
        said(&a),
        [
            text("4X"),
            text("ON"),
            text("OFF"),
            text("ON"),
            text("ON"),
            None,
            clock_text,
            text("OFF"),
            text("OFF"),
            None,
            None,
        ]
    );
    // the clock's row follows the clock
    clock.advance(3_600);
    assert_eq!(
        a.settings_text(SettingsRow::DateTime),
        text(&date_time_text(a.wall_secs()))
    );
}

#[test]
fn only_the_cursors_row_is_shown_lit() {
    let (_d, mut a, _) = on_carousel();
    open_at(&mut a, SettingsRow::Background);
    let shown = a.settings_shown();
    assert_eq!(
        shown.iter().map(|s| s.row).collect::<Vec<_>>(),
        a.settings_rows()
    );
    for s in &shown {
        assert_eq!(s.lit, s.row == SettingsRow::Background, "{:?}", s.row);
        assert_eq!(s.value, a.settings_text(s.row), "{:?}", s.row);
    }
}

/// A radio that is there or not, and says what the test hands it.
#[derive(Clone, Default)]
struct Radio(Arc<Mutex<(bool, Vec<WifiNews>)>>);

impl WifiJobs for Radio {
    fn present(&self) -> bool {
        !self.0.lock().unwrap().0
    }

    fn ask(&mut self, _job: WifiJob) {}

    fn news(&mut self) -> Vec<WifiNews> {
        std::mem::take(&mut self.0.lock().unwrap().1)
    }
}

#[test]
fn network_names_the_network_joined_in_capitals() {
    let (_d, mut a, _) = on_carousel_with(AppState {
        wifi: true,
        ..AppState::default()
    });
    let radio = Radio::default();
    a.set_wifi_jobs(Box::new(radio.clone()));
    open_at(&mut a, SettingsRow::Network);
    assert_eq!(
        a.settings_text(SettingsRow::Network),
        Some("ON".into()),
        "on, none joined"
    );
    radio.0.lock().unwrap().1.push(WifiNews::Status(Status {
        enabled: true,
        state: "COMPLETED".into(),
        ssid: Some(b"Home".to_vec()),
        ..Status::default()
    }));
    a.tick_ms(1_000);
    assert_eq!(a.wifi_summary(), Summary::Joined("Home".into()));
    assert_eq!(a.settings_text(SettingsRow::Network), Some("HOME".into()));
}

#[test]
fn without_a_radio_network_is_not_listed_drawn_or_stepped_onto() {
    let (_d, mut a, _) = on_carousel();
    let radio = Radio::default();
    radio.0.lock().unwrap().0 = true;
    a.set_wifi_jobs(Box::new(radio));
    let faces = fake_faces(&mut a);
    assert!(!a.settings_rows().contains(&SettingsRow::Network));
    assert!(a
        .settings_shown()
        .iter()
        .all(|s| s.row != SettingsRow::Network));
    open_at(&mut a, SettingsRow::FastForward);
    for _ in 0..SettingsRow::DateTime.index() {
        press(&mut a, Btn::Down);
    }
    assert_eq!(a.settings_row(), Some(SettingsRow::DateTime));
    press(&mut a, Btn::Down);
    assert_eq!(
        a.settings_row(),
        Some(SettingsRow::Bluetooth),
        "the cursor landed on Network"
    );
    let out = frame(&a);
    assert!(!drawn(&out, 200 + SettingsRow::Network.index()));
    // Bluetooth and the rows after it close up under Date & Time; the panels stay where they were
    let date = placed(&out, 200 + SettingsRow::DateTime.index()).unwrap();
    let bluetooth = placed(&out, 200 + SettingsRow::Bluetooth.index()).unwrap();
    assert_eq!(bluetooth[1], date[1] + (2 * ROW_H) as f32);
    assert_eq!(placed(&out, 100), Some(at_2x(modal_rect())));
    assert_eq!(placed(&out, 111), Some(at_2x(faces.titles[1].at)));
}

#[test]
fn nothing_is_drawn_before_its_faces_are_made() {
    let (_d, mut a, _) = on_carousel();
    open_at(&mut a, SettingsRow::FastForward);
    assert!(a.settings_faces().is_none());
    let out = frame(&a);
    assert!(
        !out.iter().any(|d| matches!(d, Draw::Tex { .. })),
        "Settings drew faces it was never given"
    );
}
