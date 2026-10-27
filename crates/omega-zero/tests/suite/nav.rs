use crate::common;

use std::mem::discriminant;
use std::sync::{Arc, Mutex};

use common::{app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};
use omega_input::{Action, Btn, Millis};
use omega_store::{write_app_state, AppState};
use omega_ui::kit::palette::palette;
use omega_ui::{
    out_h, out_w, settings_frame, Draw, SettingsFaces, SettingsRow, TexId, SPRITE_PX, TOP_BAR_H,
};
use omega_zero::app::{App, Phase};
use omega_zero::label_screen::LabelsScreen;
use omega_zero::nav::{
    across, dropping, even, offsets, progress, widest_cart, Dir, End, Offsets, SLIDE_MS,
};
use omega_zero::screens::lone::Lone;
use omega_zero::wifi::{Seen, WifiJob, WifiJobs, WifiNews};
use omega_zero::wifi_screen::{Sel, WifiScreen};
use tempfile::TempDir;

fn whole(px: f32) -> bool {
    px % SPRITE_PX as f32 == 0.0
}

#[test]
fn a_slide_eases_from_nothing_to_all_of_it_in_its_time() {
    assert_eq!(progress(1_000, 1_000), 0.0);
    assert_eq!(progress(1_000, 1_000 + SLIDE_MS / 2), 0.5);
    assert_eq!(progress(1_000, 1_000 + SLIDE_MS), 1.0);
    assert_eq!(progress(1_000, 1_000 + 10 * SLIDE_MS), 1.0, "it went past");
    assert_eq!(progress(1_000, 900), 0.0, "it went back before it started");
    assert!(
        progress(1_000, 1_000 + SLIDE_MS / 4) < 0.25,
        "it does not ease in"
    );
}

#[test]
fn going_deeper_the_screen_left_goes_right_and_the_new_one_comes_from_the_left() {
    let w = 720.0;
    let leave = |p| across(Dir::Deeper, End::Leaving, p, w);
    let arrive = |p| across(Dir::Deeper, End::Arriving, p, w);
    assert_eq!((leave(0.0), arrive(0.0)), (0.0, -720.0));
    assert_eq!((leave(0.5), arrive(0.5)), (360.0, -360.0));
    assert_eq!((leave(1.0), arrive(1.0)), (720.0, 0.0));
}

#[test]
fn going_back_the_screen_left_goes_left_and_the_one_before_comes_from_the_right() {
    let w = 640.0;
    let leave = |p| across(Dir::Back, End::Leaving, p, w);
    let arrive = |p| across(Dir::Back, End::Arriving, p, w);
    assert_eq!((leave(0.0), arrive(0.0)), (0.0, 640.0));
    assert_eq!((leave(0.5), arrive(0.5)), (-320.0, 320.0));
    assert_eq!((leave(1.0), arrive(1.0)), (-640.0, 0.0));
}

#[test]
fn out_to_settings_the_shelf_goes_left_and_settings_comes_from_the_right() {
    let w = 720.0;
    let leave = |p| across(Dir::Aside, End::Leaving, p, w);
    let arrive = |p| across(Dir::Aside, End::Arriving, p, w);
    assert_eq!((leave(0.0), arrive(0.0)), (0.0, 720.0));
    assert_eq!((leave(0.5), arrive(0.5)), (-360.0, 360.0));
    assert_eq!((leave(1.0), arrive(1.0)), (-720.0, 0.0));
}

#[test]
fn back_from_settings_it_goes_right_and_the_shelf_comes_from_the_left() {
    let w = 640.0;
    let leave = |p| across(Dir::FromAside, End::Leaving, p, w);
    let arrive = |p| across(Dir::FromAside, End::Arriving, p, w);
    assert_eq!((leave(0.0), arrive(0.0)), (0.0, -640.0));
    assert_eq!((leave(0.5), arrive(0.5)), (320.0, -320.0));
    assert_eq!((leave(1.0), arrive(1.0)), (640.0, 0.0));
}

#[test]
fn every_offset_is_whole_art_pixels() {
    for w in [640.0, 720.0] {
        for dir in [Dir::Deeper, Dir::Back, Dir::Aside, Dir::FromAside] {
            for end in [End::Leaving, End::Arriving] {
                for step in 0..=SLIDE_MS {
                    let o = offsets(dir, end, progress(0, step), w);
                    for v in [o.dx, o.carts_dx, o.bar_dy] {
                        assert!(whole(v), "{v} at {step} ms of {dir:?} {end:?} on {w}");
                    }
                }
            }
        }
    }
    assert_eq!(even(3.0), 4.0);
    assert_eq!(even(2.9), 2.0);
    assert_eq!(even(-1.2), -2.0);
}

/// The power screens slide down rather than across: going deeper, the screen left behind drops
/// out of the bottom as the new one comes down from the top, the shelf as one; back, the mirror.
#[test]
fn a_dropping_slide_goes_down_in_and_up_out_with_nothing_across() {
    let h = out_h() as f32;
    let down = |dy: f32| Offsets {
        dy,
        carts_dy: dy,
        bar_dy: dy,
        ..Offsets::NONE
    };
    for (dir, end, at_half) in [
        (Dir::Deeper, End::Leaving, h / 2.0),
        (Dir::Deeper, End::Arriving, -h / 2.0),
        (Dir::Back, End::Leaving, -h / 2.0),
        (Dir::Back, End::Arriving, h / 2.0),
    ] {
        assert_eq!(dropping(dir, end, 0.5, h), down(at_half), "{dir:?} {end:?}");
    }
    assert_eq!(dropping(Dir::Deeper, End::Leaving, 0.0, h), Offsets::NONE);
    assert_eq!(
        dropping(Dir::Deeper, End::Leaving, 1.0, h),
        down(h),
        "not gone"
    );
    assert_eq!(
        dropping(Dir::Deeper, End::Arriving, 0.0, h),
        down(-h),
        "not off the top"
    );
    assert_eq!(dropping(Dir::Deeper, End::Arriving, 1.0, h), Offsets::NONE);
    for step in 0..=SLIDE_MS {
        let o = dropping(Dir::Deeper, End::Arriving, progress(0, step), h);
        assert!(whole(o.dy), "off the art grid at {step} ms: {}", o.dy);
    }
}

/// The shelf slides in two layers: its carts across, the way the slide goes, and its top bar
/// up off the screen. Nothing of it goes down any more: there is no slot under it to drop.
#[test]
fn the_shelf_leaves_in_two_layers_and_comes_back_the_same_ways() {
    let w = out_w() as f32;
    let span = w + widest_cart();
    // leaving for Controls
    let leave = |p| offsets(Dir::Deeper, End::Leaving, p, w);
    assert_eq!(leave(0.0), Offsets::NONE);
    let half = leave(0.5);
    assert_eq!(half.carts_dx, even(span / 2.0), "the carts go right");
    assert_eq!(half.bar_dy, -TOP_BAR_H / 2.0, "the top bar goes up");
    assert_eq!(
        (half.dy, half.carts_dy),
        (0.0, 0.0),
        "something of the shelf goes down"
    );
    let gone = leave(1.0);
    assert_eq!(
        (gone.carts_dx, gone.bar_dy, gone.carts_dy),
        (span, -TOP_BAR_H, 0.0)
    );
    assert!(
        gone.carts_dx >= w + widest_cart(),
        "the neighbour peeking in at the left edge is still on the screen"
    );
    // coming back from it
    let back = |p| offsets(Dir::Back, End::Arriving, p, w);
    assert_eq!(
        (back(0.0).carts_dx, back(0.0).bar_dy, back(0.0).carts_dy),
        (span, -TOP_BAR_H, 0.0),
        "the shelf does not come back from where it went"
    );
    assert_eq!(
        back(0.5).carts_dx,
        even(span / 2.0),
        "the carts come from the right"
    );
    assert_eq!(
        back(0.5).bar_dy,
        -TOP_BAR_H / 2.0,
        "the top bar comes from the top"
    );
    assert_eq!(back(0.5).carts_dy, 0.0, "the carts come up from below");
    assert_eq!(back(1.0).carts_dx, 0.0);
    assert_eq!(back(1.0).bar_dy, 0.0);
}

#[test]
fn the_shelf_leaves_for_settings_with_its_carts_the_other_way_and_comes_back_from_the_left() {
    let w = out_w() as f32;
    let span = w + widest_cart();
    // leaving for Settings
    let leave = |p| offsets(Dir::Aside, End::Leaving, p, w);
    assert_eq!(leave(0.0), Offsets::NONE);
    let half = leave(0.5);
    assert_eq!(half.carts_dx, -even(span / 2.0), "the carts go left");
    assert_eq!(half.bar_dy, -TOP_BAR_H / 2.0, "the top bar goes up");
    assert_eq!(
        (half.dy, half.carts_dy),
        (0.0, 0.0),
        "something of the shelf goes down"
    );
    let gone = leave(1.0);
    assert_eq!(
        (gone.carts_dx, gone.bar_dy, gone.carts_dy),
        (-span, -TOP_BAR_H, 0.0)
    );
    assert!(
        gone.carts_dx <= -(w + widest_cart()),
        "the neighbour peeking in at the right edge is still on the screen"
    );
    // coming back from it
    let back = |p| offsets(Dir::FromAside, End::Arriving, p, w);
    assert_eq!(
        (back(0.0).carts_dx, back(0.0).bar_dy, back(0.0).carts_dy),
        (-span, -TOP_BAR_H, 0.0),
        "the shelf does not come back from where it went"
    );
    assert_eq!(
        back(0.5).carts_dx,
        -even(span / 2.0),
        "the carts come from the left"
    );
    assert_eq!(
        back(0.5).bar_dy,
        -TOP_BAR_H / 2.0,
        "the top bar comes from the top"
    );
    assert_eq!(back(1.0), Offsets::NONE);
}

/// A radio that is on, hears one network with a lock, and joins nothing.
#[derive(Clone, Default)]
struct Radio(Arc<Mutex<Vec<WifiNews>>>);

impl WifiJobs for Radio {
    fn present(&self) -> bool {
        true
    }

    fn ask(&mut self, job: WifiJob) {
        let mut news = self.0.lock().unwrap();
        match job {
            WifiJob::Up => news.push(WifiNews::Started(true)),
            WifiJob::Scan => news.push(WifiNews::Seen(vec![Seen {
                ssid: b"Home".to_vec(),
                rssi: -48,
                secured: true,
            }])),
            _ => {}
        }
    }

    fn news(&mut self) -> Vec<WifiNews> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

fn on_shelf() -> (TempDir, App) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_app_state(
        d.path(),
        &AppState {
            clock_set: true,
            wifi: true,
            ..AppState::default()
        },
    )
    .unwrap();
    let (mut a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    a.set_wifi_jobs(Box::new(Radio::default()));
    a.tick_ms(16);
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    (d, a)
}

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

/// What a move left the app on, the way it slid and from what.
fn moved(a: &App) -> (Phase, Dir, Phase) {
    let slide = a.slide().expect("the move did not slide");
    (a.phase().clone(), slide.dir, slide.from.clone())
}

#[track_caller]
fn slid(a: &App, to: &Phase, dir: Dir, from: &Phase) {
    let (now, how, was) = moved(a);
    assert_eq!(
        (discriminant(&now), how, discriminant(&was)),
        (discriminant(to), dir, discriminant(from)),
        "slid {how:?} from {was:?} to {now:?}"
    );
}

fn settings(row: SettingsRow) -> Phase {
    Phase::Settings { row }
}

fn walk_to(a: &mut App, row: SettingsRow) {
    assert!(
        a.settings_row().is_some(),
        "not on Settings: {:?}",
        a.phase()
    );
    for _ in SettingsRow::ALL {
        if a.settings_row() == Some(row) {
            return;
        }
        press(a, Btn::Down);
    }
    assert_eq!(a.settings_row(), Some(row), "the bar went round without it");
}

#[test]
fn every_move_between_screens_slides_the_way_it_goes() {
    let (_d, mut a) = on_shelf();
    let shelf = Phase::Shelf;

    a.apply(Action::SelectTap);
    slid(&a, &Phase::Controls, Dir::Deeper, &shelf);
    press(&mut a, Btn::B);
    slid(&a, &shelf, Dir::Back, &Phase::Controls);
    a.apply(Action::SelectTap);
    a.apply(Action::SelectTap);
    slid(&a, &shelf, Dir::Back, &Phase::Controls);

    for close in [Btn::B, Btn::Start] {
        press(&mut a, Btn::Start);
        slid(&a, &settings(SettingsRow::FastForward), Dir::Aside, &shelf);
        press(&mut a, close);
        slid(
            &a,
            &shelf,
            Dir::FromAside,
            &settings(SettingsRow::FastForward),
        );
    }
    press(&mut a, Btn::Start);
    a.apply(Action::QuickMenu);
    slid(
        &a,
        &shelf,
        Dir::FromAside,
        &settings(SettingsRow::FastForward),
    );

    press(&mut a, Btn::Start);
    press(&mut a, Btn::Down);
    assert!(a.slide().is_none(), "moving the bar slid the screen");

    let clock = Phase::SetClock {
        picker: omega_ui::ClockPicker::local(0, 0),
        seed: 0,
        from_menu: true,
    };
    for (row, opened) in [
        (SettingsRow::Labels, Phase::Labels(LabelsScreen::default())),
        (SettingsRow::DateTime, clock.clone()),
        (SettingsRow::Network, Phase::Network(WifiScreen::default())),
        (
            SettingsRow::Background,
            Phase::Background(Default::default()),
        ),
        (SettingsRow::About, Phase::About),
    ] {
        walk_to(&mut a, row);
        press(&mut a, Btn::A);
        slid(&a, &opened, Dir::Deeper, &settings(row));
        press(&mut a, Btn::B);
        slid(&a, &settings(row), Dir::Back, &opened);
        assert_eq!(a.settings_row(), Some(row), "B came back to another row");
    }

    walk_to(&mut a, SettingsRow::DateTime);
    press(&mut a, Btn::A);
    press(&mut a, Btn::A);
    slid(&a, &settings(SettingsRow::DateTime), Dir::Back, &clock);
}

#[test]
fn ssh_and_the_passphrase_slide_in_over_network_and_back() {
    let (_d, mut a) = on_shelf();
    press(&mut a, Btn::Start);
    walk_to(&mut a, SettingsRow::Network);
    press(&mut a, Btn::A);
    // X asks the radio to look, and the next tick hears what it found
    press(&mut a, Btn::X);
    a.tick_ms(1_000);
    let network = Phase::Network(WifiScreen::default());
    let at = |a: &App| match a.phase() {
        Phase::Network(s) => s.sel.clone(),
        other => panic!("not on Network: {other:?}"),
    };
    while at(&a) != Sel::Ssh {
        press(&mut a, Btn::Down);
    }
    // Y opens the row's panel
    press(&mut a, Btn::Y);
    let ssh = moved(&a).0;
    slid(&a, &ssh, Dir::Deeper, &network);
    press(&mut a, Btn::B);
    slid(&a, &network, Dir::Back, &ssh);
    assert_eq!(at(&a), Sel::Ssh, "B came back to another row");

    press(&mut a, Btn::Down);
    assert_eq!(at(&a), Sel::Network(b"Home".to_vec()));
    press(&mut a, Btn::A);
    let (now, dir, from) = moved(&a);
    assert!(
        matches!(&now, Phase::Network(s) if s.typing.is_some()),
        "no keyboard"
    );
    assert_eq!(dir, Dir::Deeper);
    assert!(
        matches!(&from, Phase::Network(s) if s.typing.is_none()),
        "the keyboard did not slide in over the list: {from:?}"
    );
    press(&mut a, Btn::B);
    let (now, dir, from) = moved(&a);
    assert!(matches!(&now, Phase::Network(s) if s.typing.is_none()));
    assert_eq!(dir, Dir::Back);
    assert!(
        matches!(&from, Phase::Network(s) if s.typing.is_some()),
        "the keyboard did not slide away: {from:?}"
    );
}

#[test]
fn the_account_keyboard_slides_in_and_its_password_step_does_not() {
    let (_d, mut a) = on_shelf();
    press(&mut a, Btn::Start);
    walk_to(&mut a, SettingsRow::Labels);
    press(&mut a, Btn::A);
    press(&mut a, Btn::A);
    let (_, dir, from) = moved(&a);
    assert_eq!(dir, Dir::Deeper);
    assert!(matches!(&from, Phase::Labels(s) if s.typing.is_none()));
    a.finish_slide();
    press(&mut a, Btn::A);
    press(&mut a, Btn::Start);
    assert!(
        a.typing().is_some_and(|t| t.prompt.contains("password")),
        "the user name did not move on to the password"
    );
    assert!(a.slide().is_none(), "the password step slid");
    press(&mut a, Btn::B);
    let (_, dir, from) = moved(&a);
    assert_eq!(dir, Dir::Back);
    assert!(matches!(&from, Phase::Labels(s) if s.typing.is_some()));
}

#[test]
fn a_press_during_a_slide_finishes_it_and_acts() {
    let (_d, mut a) = on_shelf();
    a.apply_at(Action::GbaDown(Btn::Start), 5_000);
    assert!(a.slide().is_some());
    a.apply_at(Action::GbaDown(Btn::Down), 5_050);
    assert!(a.slide().is_none(), "the press left the slide running");
    assert_eq!(
        a.settings_row(),
        Some(SettingsRow::FastForwardSound),
        "the press was spent on the slide"
    );
    a.apply_at(Action::GbaDown(Btn::B), 5_100);
    slid(
        &a,
        &Phase::Shelf,
        Dir::FromAside,
        &settings(SettingsRow::FastForwardSound),
    );
    a.apply_at(Action::GbaDown(Btn::Start), 5_120);
    slid(
        &a,
        &settings(SettingsRow::FastForward),
        Dir::Aside,
        &Phase::Shelf,
    );
    assert_eq!(a.slide().unwrap().started, 5_120);
}

#[test]
fn a_release_does_not_end_a_slide() {
    let (_d, mut a) = on_shelf();
    a.apply(Action::GbaDown(Btn::Start));
    a.apply(Action::GbaUp(Btn::Start));
    assert!(a.slide().is_some(), "START's own release ended its slide");
    a.finish_slide();
    a.apply(Action::GbaDown(Btn::B));
    a.apply(Action::GbaDown(Btn::Select));
    a.apply(Action::SelectTap);
    assert!(matches!(a.phase(), Phase::Controls));
    a.apply(Action::GbaUp(Btn::Select));
    assert!(
        a.slide().is_some(),
        "the release SELECT hands the game after its tap ended the slide"
    );
}

#[test]
fn a_slide_ends_when_its_time_is_up() {
    let (_d, mut a) = on_shelf();
    press(&mut a, Btn::Start);
    a.update((SLIDE_MS - 20) as f32 / 1000.0);
    assert!(a.slide().is_some(), "it ended early");
    a.update(0.04);
    assert!(a.slide().is_none(), "it outlasted its time");
}

#[test]
fn a_volume_press_during_a_slide_leaves_it_sliding() {
    let (_d, mut a) = on_shelf();
    press(&mut a, Btn::Start);
    a.apply(Action::VolumeDown);
    assert!(a.slide().is_some(), "the volume ended the slide");
}

/// Settings' faces as fake textures: its modal is 100.
fn fake_settings_faces(a: &mut App) {
    let id = TexId::from_raw;
    let frame = settings_frame(omega_ui::configs_in(&SettingsRow::ALL), palette(0));
    let titles = [(id(110), [42, 11]), (id(111), [42, 11])];
    a.set_settings_faces(SettingsFaces::new(&frame, id(100), titles));
}

fn frame(a: &App) -> Vec<Draw> {
    let mut out = Vec::new();
    a.draw(&mut out);
    out
}

fn x_of(out: &[Draw], tex: TexId) -> Option<f32> {
    out.iter().find_map(|d| match *d {
        Draw::Tex { x, tex: t, .. } if t == tex => Some(x),
        Draw::Spun { x, tex: t, .. } if t == tex => Some(x),
        Draw::Turned { x, tex: t, .. } if t == tex => Some(x),
        _ => None,
    })
}

/// Whether the top bar is drawn `dy` down from its place, across the whole screen.
fn top_bar_at(a: &App, out: &[Draw], dy: f32) -> bool {
    let w = out_w() as f32;
    out.iter().any(|d| {
        matches!(*d, Draw::Rect { x, y, w: bw, colour, .. }
            if x == 0.0 && y == dy && bw == w && colour == a.chrome_colours().housing)
    })
}

#[test]
fn halfway_to_settings_and_back_the_carts_and_settings_go_the_other_way_round() {
    let (_d, mut a) = on_shelf();
    let carts = [TexId::from_raw(900), TexId::from_raw(901)];
    a.set_faces(carts.to_vec());
    fake_settings_faces(&mut a);
    let label = TexId::from_raw(100);
    let cart = carts[0];
    let rest = x_of(&frame(&a), cart).expect("the selected cart is not drawn");
    let w = out_w() as f32;

    let at: Millis = 10_000;
    a.apply_at(Action::GbaDown(Btn::Start), at);
    a.finish_slide();
    let rest_label = x_of(&frame(&a), label).expect("Settings' modal is not drawn");

    // B: Settings goes back out to the right, the carts come back from the left
    a.apply_at(Action::GbaDown(Btn::B), at + 1_000);
    a.tick_ms(at + 1_000 + SLIDE_MS / 2);
    let out = frame(&a);
    let half = offsets(Dir::FromAside, End::Arriving, 0.5, w);
    assert!(half.carts_dx < 0.0);
    assert_eq!(
        x_of(&out, cart),
        Some(rest + half.carts_dx),
        "the carts are not halfway back from the left"
    );
    assert_eq!(
        x_of(&out, label),
        Some(rest_label + w / 2.0),
        "Settings is not halfway off to the right"
    );
    assert!(
        top_bar_at(&a, &out, -TOP_BAR_H / 2.0),
        "the top bar is not halfway down from the top"
    );

    // START: the carts go out to the left, Settings comes in from the right
    a.finish_slide();
    a.apply_at(Action::GbaDown(Btn::Start), at + 2_000);
    a.tick_ms(at + 2_000 + SLIDE_MS / 2);
    let out = frame(&a);
    let half = offsets(Dir::Aside, End::Leaving, 0.5, w);
    assert!(half.carts_dx < 0.0);
    assert_eq!(
        x_of(&out, cart),
        Some(rest + half.carts_dx),
        "the carts are not halfway off to the left"
    );
    assert_eq!(
        x_of(&out, label),
        Some(rest_label + w / 2.0),
        "Settings is not halfway in from the right"
    );
    assert!(
        top_bar_at(&a, &out, -TOP_BAR_H / 2.0),
        "the top bar is not halfway up off the screen"
    );
}

#[test]
fn halfway_to_controls_and_back_both_screens_are_drawn_where_the_slide_has_them() {
    let (_d, mut a) = on_shelf();
    let carts = [TexId::from_raw(900), TexId::from_raw(901)];
    a.set_faces(carts.to_vec());
    let panel = TexId::from_raw(801);
    a.set_controls_panel(Lone {
        tex: panel,
        w: 200,
        h: 62,
    });
    let cart = carts[0];
    let rest = x_of(&frame(&a), cart).expect("the selected cart is not drawn");
    let w = out_w() as f32;

    let at: Millis = 10_000;
    a.apply_at(Action::SelectTap, at);
    a.finish_slide();
    let rest_panel = x_of(&frame(&a), panel).expect("the Controls panel is not drawn");

    // B: the panel goes back out to the left, the carts come back from the right
    a.apply_at(Action::GbaDown(Btn::B), at + 1_000);
    a.tick_ms(at + 1_000 + SLIDE_MS / 2);
    let out = frame(&a);
    let half = offsets(Dir::Back, End::Arriving, 0.5, w);
    assert!(half.carts_dx > 0.0);
    assert_eq!(
        x_of(&out, cart),
        Some(rest + half.carts_dx),
        "the carts are not halfway back from the right"
    );
    assert_eq!(
        x_of(&out, panel),
        Some(rest_panel - w / 2.0),
        "Controls is not halfway off to the left"
    );
    assert!(
        top_bar_at(&a, &out, -TOP_BAR_H / 2.0),
        "the top bar is not halfway down from the top"
    );

    // SELECT: the carts go out to the right, the panel comes in from the left
    a.finish_slide();
    a.apply_at(Action::SelectTap, at + 2_000);
    a.tick_ms(at + 2_000 + SLIDE_MS / 2);
    let out = frame(&a);
    let half = offsets(Dir::Deeper, End::Leaving, 0.5, w);
    assert!(half.carts_dx > 0.0);
    assert_eq!(
        x_of(&out, cart),
        Some(rest + half.carts_dx),
        "the carts are not halfway off to the right"
    );
    assert_eq!(
        x_of(&out, panel),
        Some(rest_panel - w / 2.0),
        "Controls is not halfway in from the left"
    );
    assert!(
        top_bar_at(&a, &out, -TOP_BAR_H / 2.0),
        "the top bar is not halfway up off the screen"
    );
}

/// Settings comes in from the right, but going deeper from it is as it always was: it leaves to
/// the right as Network comes in from the left, and comes back from the right.
#[test]
fn halfway_to_network_settings_still_goes_out_to_the_right_and_comes_back_from_it() {
    let (_d, mut a) = on_shelf();
    fake_settings_faces(&mut a);
    let label = TexId::from_raw(100);
    press(&mut a, Btn::Start);
    a.finish_slide();
    walk_to(&mut a, SettingsRow::Network);
    let rest = x_of(&frame(&a), label).expect("Settings' modal is not drawn");
    let w = out_w() as f32;
    let network = Phase::Network(WifiScreen::default());

    let at: Millis = 10_000;
    a.apply_at(Action::GbaDown(Btn::A), at);
    slid(&a, &network, Dir::Deeper, &settings(SettingsRow::Network));
    a.tick_ms(at + SLIDE_MS / 2);
    assert_eq!(offsets(Dir::Deeper, End::Leaving, 0.5, w).dx, w / 2.0);
    assert_eq!(
        x_of(&frame(&a), label),
        Some(rest + w / 2.0),
        "Settings is not halfway off to the right"
    );

    a.finish_slide();
    a.apply_at(Action::GbaDown(Btn::B), at + 1_000);
    slid(&a, &settings(SettingsRow::Network), Dir::Back, &network);
    a.tick_ms(at + 1_000 + SLIDE_MS / 2);
    assert_eq!(
        x_of(&frame(&a), label),
        Some(rest + w / 2.0),
        "Settings is not halfway back from the right"
    );
}

#[test]
fn the_backdrop_stays_still_under_a_slide() {
    let (_d, mut a) = on_shelf();
    let pattern = TexId::from_raw(700);
    let style = a.backdrop().style;
    a.set_backdrop_face(pattern, out_w() + 40, style, a.backdrop().colour);
    let still = x_of(&frame(&a), pattern);
    assert!(still.is_some(), "the shelf has no pattern under it");
    a.apply(Action::GbaDown(Btn::Start));
    let out = frame(&a);
    let drawn = out
        .iter()
        .filter(|d| matches!(**d, Draw::Tex { tex, .. } if tex == pattern))
        .count();
    assert_eq!(
        drawn, 1,
        "the backdrop was drawn {drawn} times under the slide"
    );
    assert_eq!(
        x_of(&out, pattern),
        still,
        "the backdrop moved with a screen"
    );
}
