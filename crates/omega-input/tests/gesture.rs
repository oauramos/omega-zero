use omega_input::RawEvent::{Down, Up};
use omega_input::{
    Action::*, Btn, Btn::*, Gestures, Keys, RawEvent, MENU_HOLD_MS, POWER_HOLD_MS, SELECT_CHORD_MS,
    SELECT_TAP_MS, VOLUME_REPEAT_DELAY_MS, VOLUME_REPEAT_MS,
};

#[test]
fn a_select_that_becomes_a_chord_never_reaches_the_game() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(g.feed(Down(Btn::Up), 120), vec![BrightnessUp]);
    assert!(g.feed(Up(Btn::Up), 160).is_empty());
    assert!(g.feed(Up(Select), 900).is_empty());
    assert!(g.tick(5_000).is_empty());
}

#[test]
fn a_chord_keeps_both_keys_from_the_game() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(g.feed(Down(R1), 50), vec![SaveState]);
    assert!(
        g.feed(Up(R1), 60).is_empty(),
        "the chord key reached the game"
    );
    assert!(
        g.feed(Up(Select), 70).is_empty(),
        "the chord's SELECT reached the game"
    );
}

#[test]
fn a_select_held_past_the_window_reaches_the_game_then() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert!(g.tick(SELECT_CHORD_MS - 1).is_empty());
    assert_eq!(g.tick(SELECT_CHORD_MS), vec![GbaDown(Select)]);
    assert!(g.tick(SELECT_CHORD_MS + 100).is_empty());
    assert_eq!(g.feed(Up(Select), 2_000), vec![GbaUp(Select)]);
}

#[test]
fn a_game_button_under_select_hands_select_over_first() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(g.feed(Down(A), 100), vec![GbaDown(Select), GbaDown(A)]);
    assert_eq!(g.feed(Up(A), 200), vec![GbaUp(A)]);
    assert!(
        g.tick(SELECT_CHORD_MS).is_empty(),
        "SELECT was pressed twice"
    );
    assert_eq!(g.feed(Up(Select), 900), vec![GbaUp(Select)]);
}

#[test]
fn a_held_select_keeps_chording_long_past_the_window() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    assert_eq!(g.feed(Down(Btn::Up), 100), vec![BrightnessUp]);
    g.feed(Up(Btn::Up), 150);
    assert_eq!(
        g.feed(Down(Btn::Up), SELECT_CHORD_MS + 500),
        vec![BrightnessUp],
        "the second nudge of the brightness reached the game as a direction"
    );
}

#[test]
fn select_chords_map_to_all_four_axes() {
    for (btn, want) in [
        (Btn::Up, BrightnessUp),
        (Btn::Down, BrightnessDown),
        (Right, BlueLightUp),
        (Left, BlueLightDown),
    ] {
        let mut g = Gestures::new();
        g.feed(RawEvent::Down(Select), 0);
        assert_eq!(g.feed(RawEvent::Down(btn), 10), vec![want]);
    }
}

#[test]
fn a_select_tap_reaches_the_game_on_its_release() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(g.feed(Up(Select), 80), vec![GbaDown(Select), SelectTap]);
    assert!(g.tick(80 + SELECT_TAP_MS - 1).is_empty());
    assert_eq!(g.tick(80 + SELECT_TAP_MS), vec![GbaUp(Select)]);
    assert!(g.tick(5_000).is_empty());
}

#[test]
fn a_select_press_inside_the_tap_window_hands_back_the_release_it_interrupted() {
    let mut g = Gestures::new();
    let mut log = Vec::new();
    log.extend(g.feed(Down(Select), 0));
    log.extend(g.feed(Up(Select), 20));
    log.extend(g.feed(Down(Select), 20 + SELECT_TAP_MS - 1));
    log.extend(g.feed(Down(Btn::Up), 100));
    log.extend(g.feed(Up(Btn::Up), 140));
    log.extend(g.feed(Up(Select), 200));
    for t in 200..2_000 {
        log.extend(g.tick(t));
    }
    let downs = log.iter().filter(|a| **a == GbaDown(Select)).count();
    let ups = log.iter().filter(|a| **a == GbaUp(Select)).count();
    assert_eq!(
        (downs, ups),
        (1, 1),
        "the core was handed {downs} SELECT press(es) and {ups} release(s): {log:?}"
    );
    assert_eq!(
        log.iter()
            .rfind(|a| matches!(a, GbaDown(Select) | GbaUp(Select))),
        Some(&GbaUp(Select)),
        "the pad was left holding SELECT: {log:?}"
    );
    assert!(
        log.contains(&BrightnessUp),
        "the chord off the second press stopped working: {log:?}"
    );
}

#[test]
fn menu_single_tap_opens_the_quick_menu() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Menu), 0).is_empty(), "acted on the press");
    assert_eq!(g.feed(Up(Menu), 100), vec![QuickMenu]);
    assert!(g.tick(451).is_empty(), "fired a second time on the timer");
}

#[test]
fn a_menu_hold_is_not_also_a_tap() {
    let mut g = Gestures::new();
    g.feed(Down(Menu), 0);
    assert_eq!(g.tick(MENU_HOLD_MS), vec![Eject]);
    assert!(g.feed(Up(Menu), MENU_HOLD_MS + 50).is_empty());
}

#[test]
fn menu_double_tap_opens_polaroids_immediately() {
    let mut g = Gestures::new();
    g.feed(Down(Menu), 0);
    g.feed(Up(Menu), 100);
    assert_eq!(g.feed(Down(Menu), 300), vec![Polaroids]);
}

#[test]
fn menu_hold_ejects_at_the_hold_time_and_not_before() {
    let mut g = Gestures::new();
    g.feed(Down(Menu), 0);
    assert!(g.tick(MENU_HOLD_MS - 1).is_empty());
    assert_eq!(g.tick(MENU_HOLD_MS), vec![Eject]);
}

#[test]
fn menu_hold_released_early_ejects_nothing() {
    let mut g = Gestures::new();
    g.feed(Down(Menu), 0);
    g.feed(Up(Menu), MENU_HOLD_MS - 200);
    assert!(g.tick(3000).is_empty());
}

#[test]
fn r2_hold_is_momentary() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(R2), 0), vec![FfStart]);
    assert_eq!(g.feed(Up(R2), 900), vec![FfStop]);
}

#[test]
fn r2_double_tap_latches_and_single_press_clears() {
    let mut g = Gestures::new();
    g.feed(Down(R2), 0);
    g.feed(Up(R2), 50);
    g.feed(Down(R2), 100);
    assert!(g.feed(Up(R2), 150).is_empty());
    g.feed(Down(R2), 5000);
    assert_eq!(g.feed(Up(R2), 5050), vec![FfStop]);
}

#[test]
fn power_flushes_on_the_press_and_locks_on_the_release() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(Btn::Power), 0), vec![PowerPress]);
    assert_eq!(g.feed(Up(Btn::Power), 80), vec![PowerTap]);
}

#[test]
fn power_held_past_the_threshold_raises_the_menu_and_the_release_does_nothing() {
    let mut g = Gestures::new();
    g.feed(Down(Btn::Power), 0);
    assert!(g.tick(POWER_HOLD_MS - 1).is_empty());
    assert_eq!(g.tick(POWER_HOLD_MS), vec![PowerHold]);
    assert!(g.tick(4000).is_empty(), "the hold fires once, not per tick");
    assert_eq!(g.feed(Up(Btn::Power), 4500), vec![PowerOff]);
}

#[test]
fn a_press_just_short_of_the_threshold_is_a_lock() {
    let mut g = Gestures::new();
    g.feed(Down(Btn::Power), 0);
    assert!(g.tick(POWER_HOLD_MS - 1).is_empty());
    assert_eq!(g.feed(Up(Btn::Power), POWER_HOLD_MS - 1), vec![PowerTap]);
}

#[test]
fn a_press_the_rewind_turned_away_is_not_half_of_a_latch() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(L2), 0), vec![RewindStart]);
    assert!(
        g.feed(Down(R2), 50).is_empty(),
        "the rewind let a fast forward start under it"
    );
    assert!(g.feed(Up(R2), 100).is_empty());
    assert_eq!(g.feed(Up(L2), 150), vec![RewindStop]);
    assert_eq!(g.feed(Down(R2), 200), vec![FfStart]);
    assert!(!g.ff_latched(), "a single press latched fast forward");
    assert_eq!(
        g.feed(Up(R2), 400),
        vec![FfStop],
        "the finger came off R2 and the speed stayed"
    );
}

#[test]
fn rewind_beats_latched_fast_forward() {
    let mut g = Gestures::new();
    g.feed(Down(R2), 0);
    g.feed(Up(R2), 50);
    g.feed(Down(R2), 100);
    g.feed(Up(R2), 150);
    assert_eq!(g.feed(Down(L2), 200), vec![FfStop, RewindStart]);
}

#[test]
fn past_the_window_select_and_the_next_key_are_the_games() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert!(
        g.tick(400).is_empty(),
        "SELECT was handed over inside its window"
    );
    assert_eq!(g.tick(SELECT_CHORD_MS), vec![GbaDown(Select)]);
    assert_eq!(
        g.feed(Down(Btn::Up), SELECT_CHORD_MS + 1),
        vec![GbaDown(Btn::Up)]
    );
}

#[test]
fn a_chord_landing_late_is_still_a_chord() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    g.tick(400);
    assert_eq!(g.feed(Down(R1), 400), vec![SaveState]);
    assert!(
        g.tick(5_000).is_empty(),
        "the tick handed something over behind a late chord"
    );
}

#[test]
fn a_select_tap_too_short_to_be_polled_is_held_on_to() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(g.feed(Up(Select), 20), vec![GbaDown(Select), SelectTap]);
    assert!(g.tick(20 + SELECT_TAP_MS - 1).is_empty());
    assert_eq!(g.tick(20 + SELECT_TAP_MS), vec![GbaUp(Select)]);
    assert!(
        g.tick(5_000).is_empty(),
        "the release was handed over twice"
    );
}

/// Every action a press and release of SELECT alone hands on, from its press at 0 until well
/// after it is let go at `up`.
fn lone_select(up: u64) -> Vec<omega_input::Action> {
    let mut g = Gestures::new();
    let mut log = g.feed(Down(Select), 0);
    for t in 1..up {
        log.extend(g.tick(t));
    }
    log.extend(g.feed(Up(Select), up));
    for t in up..up + 1_000 {
        log.extend(g.tick(t));
    }
    log
}

#[test]
fn a_lone_select_tap_says_it_was_tapped_once() {
    let log = lone_select(80);
    assert_eq!(log, vec![GbaDown(Select), SelectTap, GbaUp(Select)]);
}

#[test]
fn a_select_held_past_the_chord_window_is_not_a_tap() {
    let log = lone_select(SELECT_CHORD_MS + 200);
    assert!(
        !log.contains(&SelectTap),
        "a held SELECT was taken for a tap: {log:?}"
    );
    assert_eq!(log, vec![GbaDown(Select), GbaUp(Select)]);
}

#[test]
fn a_select_chord_is_not_a_tap() {
    let mut g = Gestures::new();
    let mut log = g.feed(Down(Select), 0);
    log.extend(g.feed(Down(Btn::Up), 50));
    log.extend(g.feed(Up(Btn::Up), 90));
    log.extend(g.feed(Up(Select), 120));
    log.extend(g.tick(5_000));
    assert_eq!(log, vec![BrightnessUp], "SELECT+Up was taken for a tap");
}

#[test]
fn a_select_the_game_has_seen_ends_on_its_own_release() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    assert_eq!(g.tick(SELECT_CHORD_MS), vec![GbaDown(Select)]);
    assert_eq!(
        g.feed(Up(Select), SELECT_CHORD_MS + SELECT_TAP_MS),
        vec![GbaUp(Select)]
    );
    assert!(g.tick(5_000).is_empty());
}

#[test]
fn both_volume_keys_together_mute_once() {
    let mut g = Gestures::new();
    g.feed(Down(VolUp), 0);
    let out = g.feed(Down(VolDown), 80);
    assert!(out.contains(&MuteToggle), "the pair did not mute");
    assert!(
        g.tick(400).is_empty(),
        "it kept firing while both were held"
    );
}

#[test]
fn volume_keys_far_apart_are_not_a_chord() {
    let mut g = Gestures::new();
    g.feed(Down(VolUp), 0);
    let out = g.feed(Down(VolDown), 400);
    assert!(!out.contains(&MuteToggle), "two separate presses muted");
}

#[test]
fn the_chord_arrives_behind_the_press_that_completed_it() {
    let mut g = Gestures::new();
    g.feed(Down(VolUp), 0);
    assert_eq!(g.feed(Down(VolDown), 80), vec![VolumeDown, MuteToggle]);
}

#[test]
fn releasing_both_rearms_the_chord() {
    let mut g = Gestures::new();
    g.feed(Down(VolUp), 0);
    g.feed(Down(VolDown), 80);
    g.feed(Up(VolUp), 200);
    g.feed(Up(VolDown), 220);
    g.feed(Down(VolUp), 1_000);
    assert!(g.feed(Down(VolDown), 1_050).contains(&MuteToggle));
}

#[test]
fn a_held_volume_key_repeats() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(VolUp), 0), vec![VolumeUp]);
    assert!(
        g.tick(VOLUME_REPEAT_DELAY_MS - 1).is_empty(),
        "it repeated before the ramp was due"
    );
    assert_eq!(g.tick(VOLUME_REPEAT_DELAY_MS), vec![VolumeUp]);
    assert!(g.tick(VOLUME_REPEAT_DELAY_MS + 1).is_empty());
    assert_eq!(
        g.tick(VOLUME_REPEAT_DELAY_MS + VOLUME_REPEAT_MS),
        vec![VolumeUp]
    );
}

#[test]
fn a_released_volume_key_stops_repeating() {
    let mut g = Gestures::new();
    g.feed(Down(VolDown), 0);
    assert_eq!(g.tick(VOLUME_REPEAT_DELAY_MS), vec![VolumeDown]);
    assert!(g.feed(Up(VolDown), VOLUME_REPEAT_DELAY_MS + 10).is_empty());
    assert!(
        g.tick(VOLUME_REPEAT_DELAY_MS * 4).is_empty(),
        "a key nobody is holding kept ramping"
    );
}

#[test]
fn the_mute_chord_does_not_ramp() {
    let mut g = Gestures::new();
    g.feed(Down(VolUp), 0);
    assert!(g.feed(Down(VolDown), 50).contains(&MuteToggle));
    assert!(
        g.tick(VOLUME_REPEAT_DELAY_MS * 3).is_empty(),
        "the mute chord ramped the volume while it was held"
    );
}

#[test]
fn each_press_starts_its_own_ramp() {
    let mut g = Gestures::new();
    g.feed(Down(VolUp), 0);
    g.tick(VOLUME_REPEAT_DELAY_MS);
    g.feed(Up(VolUp), VOLUME_REPEAT_DELAY_MS + 5);
    assert_eq!(g.feed(Down(VolUp), 5_000), vec![VolumeUp]);
    assert!(
        g.tick(5_000 + VOLUME_REPEAT_DELAY_MS - 1).is_empty(),
        "the new press repeated early"
    );
    assert_eq!(g.tick(5_000 + VOLUME_REPEAT_DELAY_MS), vec![VolumeUp]);
}

#[test]
fn x_without_select_is_still_the_games_x() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(X), 0), vec![GbaDown(Btn::X)]);
    assert_eq!(g.feed(Up(X), 40), vec![GbaUp(Btn::X)]);
}

#[test]
fn select_and_menu_open_the_in_game_menu() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    assert_eq!(g.feed(Down(Menu), 50), vec![GameMenu]);
    assert!(
        g.feed(Up(Menu), 150).is_empty(),
        "the release landed as a second gesture on top of the menu"
    );
    assert!(g.feed(Up(Select), 200).is_empty());
}

#[test]
fn a_chorded_menu_never_arms_the_eject_hold() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    assert_eq!(g.feed(Down(Menu), 50), vec![GameMenu]);
    assert!(
        g.tick(50 + MENU_HOLD_MS + 1).is_empty(),
        "holding the menu open ejected the cart"
    );
}

#[test]
fn a_chord_after_a_recent_menu_tap_is_still_the_menu() {
    let mut g = Gestures::new();
    g.feed(Down(Menu), 0);
    assert_eq!(g.feed(Up(Menu), 100), vec![QuickMenu]);
    g.feed(Down(Select), 150);
    assert_eq!(
        g.feed(Down(Menu), 200),
        vec![GameMenu],
        "a recent tap turned the chord into the switcher"
    );
}

#[test]
fn menu_under_a_select_the_game_already_has_is_not_the_menu() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(g.tick(SELECT_CHORD_MS), vec![GbaDown(Select)]);
    assert!(
        g.feed(Down(Menu), 700).is_empty(),
        "a SELECT the game already owns still chorded"
    );
    assert_eq!(g.feed(Up(Menu), 800), vec![QuickMenu]);
}

#[test]
fn the_menu_button_still_works_after_a_chord() {
    let mut g = Gestures::new();
    g.feed(Down(Menu), 0);
    assert_eq!(g.feed(Up(Menu), 100), vec![QuickMenu]);
    g.feed(Down(Select), 150);
    assert_eq!(g.feed(Down(Menu), 200), vec![GameMenu]);
    assert!(g.feed(Up(Menu), 250).is_empty());
    assert!(g.feed(Up(Select), 260).is_empty());
    assert!(
        g.feed(Down(Menu), 400).is_empty(),
        "the tap before the chord was still standing as half of a double tap"
    );
    assert_eq!(
        g.feed(Up(Menu), 450),
        vec![QuickMenu],
        "the chord left the menu button dead"
    );
}

#[test]
fn select_and_y_toggles_colour_correction_and_costs_the_game_nothing() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(g.feed(Down(Y), 10), vec![ColourCorrectionToggle]);
    assert!(g.feed(Up(Y), 40).is_empty());
    assert!(g.feed(Up(Select), 200).is_empty());
}

#[test]
fn y_on_its_own_is_untouched_by_the_colour_chord() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(Y), 0), vec![GbaDown(Y)]);
    assert_eq!(g.feed(Up(Y), 40), vec![GbaUp(Y)]);
}

#[test]
fn select_and_x_toggles_lcd_ghosting_and_costs_the_game_nothing() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(g.feed(Down(X), 10), vec![LcdGhostingToggle]);
    assert!(
        g.feed(Up(X), 40).is_empty(),
        "the chord's X reached the game"
    );
    assert!(g.feed(Up(Select), 200).is_empty());
    assert!(g.tick(5_000).is_empty());
}

#[test]
fn x_on_its_own_is_untouched_by_the_ghosting_chord() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(X), 0), vec![GbaDown(X)]);
    assert_eq!(g.feed(Up(X), 40), vec![GbaUp(X)]);
}

#[test]
fn select_x_and_select_y_are_two_chords_under_one_select() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(g.feed(Down(X), 10), vec![LcdGhostingToggle]);
    assert_eq!(g.feed(Down(Y), 20), vec![ColourCorrectionToggle]);
    assert!(g.feed(Up(X), 30).is_empty());
    assert!(g.feed(Up(Y), 40).is_empty());
    assert!(g.feed(Up(Select), 50).is_empty());
}

#[test]
fn an_x_held_for_the_game_before_select_is_let_go_to_the_game() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(X), 0), vec![GbaDown(X)]);
    assert!(g.feed(Down(Select), 10).is_empty());
    assert_eq!(g.feed(Up(X), 20), vec![GbaUp(X)]);
}

/// The PlayStation's keys: its games have L2 and R2, nothing rewinds them, and a television's
/// picture has no colour correction nor an LCD's ghosting.
const PS1: Keys = Keys {
    game_l2_r2: true,
    rewind: false,
    colour: false,
    ghosting: false,
};

/// The SNES's keys: L2 rewinds and R2 fast-forwards as on the handhelds, and a television's
/// picture has no colour correction nor an LCD's ghosting.
const SNES: Keys = Keys {
    game_l2_r2: false,
    rewind: true,
    colour: false,
    ghosting: false,
};

fn with_keys(keys: Keys) -> Gestures {
    let mut g = Gestures::new();
    assert!(
        g.set_keys(keys).is_empty(),
        "a pad nobody touched had something to let go"
    );
    g
}

#[test]
fn the_handhelds_keys_are_the_default() {
    assert_eq!(Gestures::new().keys(), Keys::HANDHELD);
    assert_eq!(Keys::default(), Keys::HANDHELD);
    let mut g = Gestures::new();
    assert!(g.set_keys(Keys::HANDHELD).is_empty());
    assert_eq!(g.feed(Down(L2), 0), vec![RewindStart]);
    assert_eq!(g.feed(Up(L2), 100), vec![RewindStop]);
    assert_eq!(g.feed(Down(R2), 200), vec![FfStart]);
    assert_eq!(g.feed(Up(R2), 300), vec![FfStop]);
}

#[test]
fn a_playstation_game_has_l2_and_r2_of_its_own() {
    let mut g = with_keys(PS1);
    assert_eq!(g.feed(Down(L2), 0), vec![GbaDown(L2)]);
    assert_eq!(g.feed(Down(R2), 10), vec![GbaDown(R2)]);
    assert_eq!(g.feed(Up(R2), 900), vec![GbaUp(R2)]);
    assert_eq!(g.feed(Up(L2), 950), vec![GbaUp(L2)]);
    assert!(g.tick(5_000).is_empty());
}

#[test]
fn nothing_rewinds_a_playstation_game() {
    let mut g = with_keys(PS1);
    let mut log = g.feed(Down(L2), 0);
    log.extend(g.feed(Up(L2), 200));
    log.extend(g.feed(Down(Select), 1_000));
    log.extend(g.feed(Down(L2), 1_050));
    log.extend(g.feed(Up(L2), 1_200));
    log.extend(g.feed(Up(Select), 1_300));
    log.extend(g.tick(5_000));
    assert!(
        !log.iter().any(|a| matches!(a, RewindStart | RewindStop)),
        "L2 rewound a PlayStation game: {log:?}"
    );
    assert_eq!(
        log,
        vec![
            GbaDown(L2),
            GbaUp(L2),
            GbaDown(Select),
            GbaDown(L2),
            GbaUp(L2),
            GbaUp(Select)
        ],
        "SELECT+L2 did not reach the game"
    );
}

#[test]
fn select_and_r2_fast_forward_a_playstation_game_while_r2_is_held() {
    let mut g = with_keys(PS1);
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(g.feed(Down(R2), 60), vec![FfStart]);
    assert!(
        g.feed(Up(Select), 200).is_empty(),
        "the chord's SELECT reached the game"
    );
    assert!(g.tick(2_000).is_empty(), "letting SELECT go stopped it");
    assert_eq!(g.feed(Up(R2), 2_100), vec![FfStop]);
    assert!(!g.ff_latched());
    assert!(g.tick(5_000).is_empty());
}

#[test]
fn select_and_r2_twice_keeps_a_playstation_game_fast_and_once_more_stops_it() {
    let mut g = with_keys(PS1);
    g.feed(Down(Select), 0);
    assert_eq!(g.feed(Down(R2), 50), vec![FfStart]);
    assert_eq!(g.feed(Up(R2), 100), vec![FfStop]);
    assert_eq!(g.feed(Down(R2), 200), vec![FfStart]);
    assert!(
        g.feed(Up(R2), 250).is_empty(),
        "the second press did not latch"
    );
    assert!(g.ff_latched());
    assert!(g.feed(Up(Select), 300).is_empty());

    // R2 on its own is the game's, and leaves the latch be
    assert_eq!(g.feed(Down(R2), 2_000), vec![GbaDown(R2)]);
    assert_eq!(g.feed(Up(R2), 2_100), vec![GbaUp(R2)]);
    assert!(g.ff_latched(), "the game's R2 dropped the latch");

    g.feed(Down(Select), 5_000);
    assert!(g.feed(Down(R2), 5_050).is_empty());
    assert_eq!(g.feed(Up(R2), 5_100), vec![FfStop]);
    assert!(!g.ff_latched());
}

#[test]
fn a_select_tap_still_reaches_a_playstation_game() {
    let mut g = with_keys(PS1);
    let mut log = g.feed(Down(Select), 0);
    log.extend(g.feed(Up(Select), 80));
    for t in 80..1_000 {
        log.extend(g.tick(t));
    }
    assert_eq!(log, vec![GbaDown(Select), SelectTap, GbaUp(Select)]);

    let mut g = with_keys(PS1);
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(g.tick(SELECT_CHORD_MS), vec![GbaDown(Select)]);
    assert_eq!(
        g.feed(Down(R2), SELECT_CHORD_MS + 10),
        vec![GbaDown(R2)],
        "SELECT held for the game, then R2, is the game's SELECT+R2"
    );
}

#[test]
fn select_and_y_is_the_games_where_there_is_no_colour_correction() {
    for keys in [PS1, SNES] {
        let mut g = with_keys(keys);
        assert!(g.feed(Down(Select), 0).is_empty());
        assert_eq!(g.feed(Down(Y), 10), vec![GbaDown(Select), GbaDown(Y)]);
        assert_eq!(g.feed(Up(Y), 40), vec![GbaUp(Y)]);
    }
}

#[test]
fn select_and_x_is_the_games_where_there_is_no_lcd_ghosting() {
    for keys in [PS1, SNES] {
        let mut g = with_keys(keys);
        assert!(g.feed(Down(Select), 0).is_empty());
        assert_eq!(g.feed(Down(X), 10), vec![GbaDown(Select), GbaDown(X)]);
        assert_eq!(g.feed(Up(X), 40), vec![GbaUp(X)]);
    }
}

#[test]
fn a_snes_game_rewinds_and_fast_forwards_as_a_handheld_game_does() {
    let mut g = with_keys(SNES);
    assert_eq!(g.feed(Down(L2), 0), vec![RewindStart]);
    assert_eq!(g.feed(Up(L2), 100), vec![RewindStop]);
    g.feed(Down(R2), 1_000);
    g.feed(Up(R2), 1_050);
    g.feed(Down(R2), 1_100);
    assert!(g.feed(Up(R2), 1_150).is_empty());
    assert!(g.ff_latched());
    assert!(g.feed(Down(Select), 2_000).is_empty());
    assert_eq!(g.feed(Down(R1), 2_050), vec![SaveState]);
    assert_eq!(g.feed(Down(L1), 2_100), vec![LoadState]);
}

#[test]
fn new_keys_let_go_of_a_rewind_and_a_fast_forward_under_way() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(L2), 0), vec![RewindStart]);
    assert_eq!(g.set_keys(PS1), vec![RewindStop]);
    assert_eq!(g.feed(Up(L2), 100), vec![GbaUp(L2)]);
    assert_eq!(g.feed(Down(L2), 200), vec![GbaDown(L2)]);

    let mut g = with_keys(PS1);
    g.feed(Down(Select), 0);
    assert_eq!(g.feed(Down(R2), 10), vec![FfStart]);
    assert_eq!(g.set_keys(Keys::HANDHELD), vec![FfStop]);
    assert!(g.set_keys(Keys::HANDHELD).is_empty());
    assert_eq!(
        g.feed(Down(R2), 2_000),
        vec![FfStart],
        "R2 stayed held as the chord it was"
    );
}
