//! The panel's 120 Hz, as omega-zero asks for it: on by default wherever the panel is one known
//! to run at it, kept off when the panel refused it or lost it, and left alone on any other.

use crate::common;

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::{app_playing_in, boot, tmp_root_with_carts};
use omega_input::Action;
use omega_power::{Battery, Charge, LedState, Platform, Power};
use omega_store::{read_app_state, write_app_state, AppState};
use omega_zero::app::App;

/// A panel that records each refresh it is asked for, and refuses 120 Hz when told to, or
/// loses it when the lid opens.
struct RefreshPanel {
    supported: bool,
    refuse: bool,
    lose_on_open: bool,
    high: bool,
    asked: Arc<Mutex<Vec<bool>>>,
}

impl Platform for RefreshPanel {
    fn set_backlight(&mut self, _: u8) {}
    fn battery(&self) -> Option<Battery> {
        None
    }
    fn charge(&self) -> Charge {
        Charge::Unknown
    }
    fn set_led(&mut self, _: LedState) {}
    fn poweroff(&mut self) -> ! {
        panic!("test panel")
    }
    fn restart(&mut self) -> ! {
        panic!("test panel")
    }
    fn root(&self) -> &Path {
        Path::new("sdcard")
    }
    fn now(&self) -> i64 {
        1_800_000_000
    }
    fn set_clock(&mut self, _: i64) {}
    fn set_rumble(&mut self, _: u16) {}

    fn supports_high_refresh(&self) -> bool {
        self.supported
    }

    fn set_high_refresh(&mut self, high: bool) -> bool {
        self.asked.lock().unwrap().push(high);
        let ok = !(high && self.refuse);
        self.high = high && ok;
        ok
    }

    fn high_refresh_enabled(&self) -> bool {
        self.high
    }

    fn resume_display(&mut self) {
        if self.lose_on_open {
            self.high = false;
        }
    }
}

fn panel(a: &mut App, supported: bool, refuse: bool, lose_on_open: bool) -> Arc<Mutex<Vec<bool>>> {
    let asked = Arc::new(Mutex::new(Vec::new()));
    a.set_power(Power::new(
        Box::new(RefreshPanel {
            supported,
            refuse,
            lose_on_open,
            high: false,
            asked: asked.clone(),
        }),
        Duration::from_secs(60),
    ));
    asked
}

#[test]
fn a_supported_panel_runs_at_120_hz_by_default() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    let asked = panel(&mut a, true, false, false);
    assert_eq!(*asked.lock().unwrap(), [true]);
    assert!(a.high_refresh());
    a.apply(Action::LcdGhostingToggle);
    assert!(read_app_state(d.path()).high_refresh);
}

#[test]
fn an_unsupported_panel_is_never_asked() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    let asked = panel(&mut a, false, false, false);
    assert!(asked.lock().unwrap().is_empty());
    assert!(!a.high_refresh());
}

#[test]
fn a_refused_request_is_written_down_as_60_hz() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    let asked = panel(&mut a, true, true, false);
    assert_eq!(*asked.lock().unwrap(), [true]);
    assert!(!a.high_refresh());
    a.apply(Action::LcdGhostingToggle);
    assert!(
        !read_app_state(d.path()).high_refresh,
        "a panel that refused 120 Hz would be asked again at every boot"
    );
}

#[test]
fn a_card_set_to_60_hz_by_hand_keeps_the_panel_at_60() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_app_state(
        d.path(),
        &AppState {
            high_refresh: false,
            ..AppState::default()
        },
    )
    .unwrap();
    let mut a = boot(d.path());
    let asked = panel(&mut a, true, false, false);
    assert_eq!(*asked.lock().unwrap(), [false]);
    assert!(!a.high_refresh());
}

#[test]
fn a_panel_that_lost_120_hz_over_a_doze_is_written_down_as_60_hz() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    panel(&mut a, true, false, true);
    assert!(a.high_refresh());
    a.apply(Action::LidClose);
    a.apply(Action::LidOpen);
    assert!(!a.high_refresh());
    assert!(!read_app_state(d.path()).high_refresh);
}

#[test]
fn a_panel_that_kept_120_hz_over_a_doze_stays_there() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    let asked = panel(&mut a, true, false, false);
    a.apply(Action::LidClose);
    a.apply(Action::LidOpen);
    assert!(a.high_refresh());
    assert_eq!(
        *asked.lock().unwrap(),
        [true],
        "the lid asked for the refresh again"
    );
}

#[test]
fn the_loops_own_clock_ticks_at_the_games_period_only_while_the_game_is_paced_by_it() {
    use omega_input::Millis;
    use omega_store::Platform;
    use omega_zero::app::Phase;
    use omega_zero::pacing::{FrameClock, UI_PERIOD};
    use omega_zero::session::Session;
    use std::time::Instant;

    let d = crate::keys::root_with(Platform::Gba, "Emerald");
    write_app_state(
        d.path(),
        &AppState {
            cart: Some("Emerald".into()),
            cart_platform: Some(Platform::Gba),
            clock_set: true,
            ..AppState::default()
        },
    )
    .unwrap();
    let mut s = Session::boot(d.path().to_path_buf());
    assert_eq!(
        s.frame_clock(),
        FrameClock::default(),
        "no game, no game clock"
    );
    let mut now: Millis = 0;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !(matches!(s.app().phase(), Phase::Playing { .. }) && s.game_visible()) {
        assert!(Instant::now() < deadline, "the game never came up");
        now += 16;
        s.feed([], now);
        s.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        s.frame_clock().period,
        UI_PERIOD,
        "a game running on its own clock gave the loop its period"
    );
    s.set_driven(true);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !s.emu().is_some_and(|e| e.locked()) {
        assert!(
            Instant::now() < deadline,
            "the game never locked to the loop"
        );
        s.step_emulator(UI_PERIOD, Duration::from_millis(20));
    }
    let period = s.frame_clock().period;
    assert_eq!(period, s.emu().unwrap().frame_period());
    assert!(
        period > Duration::from_micros(16_400) && period < Duration::from_micros(17_000),
        "a handheld game's period is {period:?}"
    );
}
