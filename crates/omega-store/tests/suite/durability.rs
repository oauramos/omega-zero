use crate::common;

use common::tmp_root;
use omega_store::{
    atomic_write, read_app_state, write_app_state, AppState, BackdropStyle, Font, Platform,
    BACKDROP_COLOURS, FF_SPEEDS, FF_SPEED_DEFAULT,
};
use tempfile::tempdir;

#[test]
fn atomic_write_leaves_no_partial_file_and_no_temp_behind() {
    let d = tempdir().unwrap();
    let p = d.path().join("x.bin");
    atomic_write(&p, b"first").unwrap();
    atomic_write(&p, &vec![7u8; 4_000_000]).unwrap();
    assert_eq!(std::fs::read(&p).unwrap().len(), 4_000_000);
    let strays: Vec<_> = std::fs::read_dir(d.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name() != "x.bin")
        .collect();
    assert!(strays.is_empty(), "temp files left behind: {strays:?}");
}

#[test]
fn a_name_the_card_accepts_is_a_name_that_can_be_written() {
    let d = tempdir().unwrap();
    for len in [8usize, 100, 200, 239, 240, 245, 250, 251] {
        let p = d.path().join(format!("{}.sav", "a".repeat(len)));
        if std::fs::write(&p, b"control").is_err() {
            continue;
        }
        atomic_write(&p, b"save")
            .unwrap_or_else(|e| panic!("{len} character name: {e}, and a plain write took it"));
        assert_eq!(std::fs::read(&p).unwrap(), b"save");
    }
}

#[test]
fn a_long_multibyte_name_is_written_rather_than_panicking() {
    let d = tempdir().unwrap();
    for pad in 0..3 {
        let p = d
            .path()
            .join(format!("{}{}.sav", "x".repeat(pad), "ポ".repeat(82)));
        if std::fs::write(&p, b"control").is_err() {
            continue;
        }
        atomic_write(&p, b"save")
            .unwrap_or_else(|e| panic!("a Japanese cart name, pad {pad}: {e}"));
        assert_eq!(std::fs::read(&p).unwrap(), b"save");
    }
}

#[test]
fn corrupt_app_state_reads_as_default_rather_than_panicking() {
    let d = tmp_root();
    std::fs::write(
        d.path().join("Config/omega-zero.state"),
        b"\x00\xff not json",
    )
    .unwrap();
    assert_eq!(read_app_state(d.path()), AppState::default());
}

#[test]
fn app_state_round_trips_including_a_stem_with_an_equals_sign() {
    let d = tmp_root();
    let s = AppState {
        cart: Some("Cheats = On".into()),
        brightness: 3,
        blue_light: 9,
        volume: 71,
        muted: true,
        clock_set: true,
        utc_offset_min: 0,
        ..AppState::default()
    };
    write_app_state(d.path(), &s).unwrap();
    assert_eq!(read_app_state(d.path()), s);
}

#[test]
fn a_app_state_missing_a_key_reads_as_default_not_half_populated() {
    let d = tmp_root();
    std::fs::write(
        d.path().join("Config/omega-zero.state"),
        "cart=Emerald\nbrightness=3\nblue_light=1\n",
    )
    .unwrap();
    assert_eq!(read_app_state(d.path()), AppState::default());
}

#[test]
fn an_out_of_range_level_reads_as_default() {
    let d = tmp_root();
    for body in [
        "cart=\nbrightness=10\nblue_light=1\nvolume=50\n",
        "cart=\nbrightness=3\nblue_light=10\nvolume=50\n",
        "cart=\nbrightness=3\nblue_light=1\nvolume=101\n",
    ] {
        std::fs::write(d.path().join("Config/omega-zero.state"), body).unwrap();
        assert_eq!(
            read_app_state(d.path()),
            AppState::default(),
            "accepted {body:?}"
        );
    }
}

#[test]
fn a_first_boot_is_neither_dark_nor_silent() {
    let d = tmp_root();
    let s = read_app_state(d.path());
    assert!(s.cart.is_none());
    assert!(s.brightness > 0, "boots with the backlight off");
    assert!(s.volume > 0, "boots muted");
}

#[test]
fn app_state_round_trips_a_negative_utc_offset() {
    let d = tmp_root();
    let s = AppState {
        cart: None,
        brightness: 5,
        blue_light: 0,
        volume: 60,
        muted: false,
        clock_set: true,
        utc_offset_min: -450,
        ..AppState::default()
    };
    write_app_state(d.path(), &s).unwrap();
    assert_eq!(read_app_state(d.path()).utc_offset_min, -450);
}

#[test]
fn a_line_the_reader_does_not_know_is_skipped() {
    let d = tmp_root();
    std::fs::write(
        d.path().join("Config/omega-zero.state"),
        "cart=Emerald\nbrightness=3\nfrom_a_later_build=7\nblue_light=1\nvolume=40\nmuted=1\n\
         no equals sign at all\nclock_set=1\nutc_offset_min=-300\n",
    )
    .unwrap();
    let s = read_app_state(d.path());
    assert_eq!(
        s.cart.as_deref(),
        Some("Emerald"),
        "the file was thrown away"
    );
    assert_eq!(
        (
            s.brightness,
            s.blue_light,
            s.volume,
            s.muted,
            s.clock_set,
            s.utc_offset_min
        ),
        (3, 1, 40, true, true, -300)
    );
}

#[test]
fn a_first_boot_rumbles_and_fast_forwards_silently_at_the_default() {
    let s = AppState::default();
    assert!(s.rumble, "boots with the motor off");
    assert_eq!(s.ff_speed, FF_SPEED_DEFAULT);
    assert!(!s.ff_sound, "boots with fast forward audible");
    assert!(!s.colour_correction, "boots with the picture tinted");
    assert!(s.lcd_ghosting, "boots with the LCD's ghosting off");
    assert!(s.high_refresh, "boots asking the panel for 60 Hz");
    assert!(!s.wifi, "boots with the radio on");
}

#[test]
fn a_card_from_before_the_settings_keeps_all_its_values() {
    let d = tmp_root();
    std::fs::write(
        d.path().join("Config/omega-zero.state"),
        "cart=Emerald\nbrightness=3\nblue_light=1\nvolume=40\nmuted=1\nclock_set=1\nutc_offset_min=-300\n",
    )
    .unwrap();
    assert_eq!(
        read_app_state(d.path()),
        AppState {
            cart: Some("Emerald".into()),
            cart_platform: None,
            brightness: 3,
            blue_light: 1,
            volume: 40,
            volume_hp: 40,
            muted: true,
            muted_hp: true,
            clock_set: true,
            utc_offset_min: -300,
            rumble: true,
            ff_speed: FF_SPEED_DEFAULT,
            ff_sound: false,
            colour_correction: false,
            lcd_ghosting: true,
            high_refresh: true,
            wifi: false,
            ssh: true,
            bluetooth: false,
            backdrop: None,
            backdrop_moving: true,
            backdrop_colour: 0,
            font: Font::Bold,
        }
    );
}

#[test]
fn the_quick_menu_settings_round_trip_as_their_own_lines() {
    let d = tmp_root();
    let s = AppState {
        clock_set: true,
        rumble: false,
        ff_speed: 2,
        ff_sound: true,
        colour_correction: true,
        wifi: true,
        ..AppState::default()
    };
    write_app_state(d.path(), &s).unwrap();
    assert_eq!(read_app_state(d.path()), s);
    let text = std::fs::read_to_string(d.path().join("Config/omega-zero.state")).unwrap();
    for line in [
        "rumble=0",
        "ff_speed=2",
        "ff_sound=1",
        "colour_correction=1",
        "wifi=1",
    ] {
        assert!(text.lines().any(|l| l == line), "no {line} in {text:?}");
    }
}

#[test]
fn lcd_ghosting_round_trips_as_its_own_line_and_is_on_when_absent() {
    for on in [false, true] {
        let d = tmp_root();
        let s = AppState {
            clock_set: true,
            lcd_ghosting: on,
            ..AppState::default()
        };
        write_app_state(d.path(), &s).unwrap();
        assert_eq!(
            read_app_state(d.path()),
            s,
            "ghosting {on} did not come back"
        );
        let text = std::fs::read_to_string(d.path().join("Config/omega-zero.state")).unwrap();
        let want = format!("lcd_ghosting={}", on as u8);
        assert!(text.lines().any(|l| l == want), "no {want} in {text:?}");
    }
    let d = tmp_root();
    let known = "cart=Emerald\nbrightness=3\nblue_light=1\nvolume=40\nmuted=0\nclock_set=1\nutc_offset_min=0\n";
    for (extra, want) in [
        ("", true),
        ("lcd_ghosting=0\n", false),
        ("lcd_ghosting=1\n", true),
        ("lcd_ghosting=2\n", true),
        ("lcd_ghosting=\n", true),
        ("lcd_ghosting=mix\n", true),
    ] {
        std::fs::write(
            d.path().join("Config/omega-zero.state"),
            format!("{known}{extra}"),
        )
        .unwrap();
        let s = read_app_state(d.path());
        assert_eq!(s.lcd_ghosting, want, "{extra:?}");
        assert_eq!(
            s.cart.as_deref(),
            Some("Emerald"),
            "{extra:?} lost the card"
        );
    }
}

#[test]
fn high_refresh_is_on_by_default_and_only_a_clear_1_keeps_it_on() {
    let d = tmp_root();
    let known = "cart=Emerald\nbrightness=3\nblue_light=1\nvolume=40\nmuted=0\nclock_set=1\nutc_offset_min=0\n";
    for (extra, want) in [
        ("", true),
        ("high_refresh=1\n", true),
        ("high_refresh=0\n", false),
        ("high_refresh=2\n", false),
        ("high_refresh=\n", false),
    ] {
        std::fs::write(
            d.path().join("Config/omega-zero.state"),
            format!("{known}{extra}"),
        )
        .unwrap();
        assert_eq!(read_app_state(d.path()).high_refresh, want, "{extra:?}");
    }
    for on in [false, true] {
        let s = AppState {
            clock_set: true,
            high_refresh: on,
            ..AppState::default()
        };
        write_app_state(d.path(), &s).unwrap();
        assert_eq!(read_app_state(d.path()), s);
    }
}

#[test]
fn every_speed_the_row_offers_round_trips_as_its_own_number() {
    for speed in FF_SPEEDS {
        let d = tmp_root();
        let s = AppState {
            clock_set: true,
            ff_speed: speed,
            ..AppState::default()
        };
        write_app_state(d.path(), &s).unwrap();
        assert_eq!(read_app_state(d.path()), s, "{speed}x did not come back");
        let text = std::fs::read_to_string(d.path().join("Config/omega-zero.state")).unwrap();
        let want = format!("ff_speed={speed}");
        assert!(text.lines().any(|l| l == want), "no {want} in {text:?}");
    }
}

#[test]
fn an_older_build_reads_the_new_speed_as_its_own_default() {
    for speed in FF_SPEEDS.iter().filter(|&&n| n > 4) {
        assert!(
            !(2..=4).contains(speed),
            "{speed}x is inside the range an older build accepts, so it would read as a speed"
        );
    }
    assert_eq!(AppState::default().ff_speed, FF_SPEED_DEFAULT);
    assert!(
        FF_SPEEDS.contains(&FF_SPEED_DEFAULT),
        "the default is not one of the speeds the row offers"
    );
}

#[test]
fn an_out_of_range_setting_falls_back_to_its_default() {
    let d = tmp_root();
    let known = "cart=Emerald\nbrightness=3\nblue_light=1\nvolume=40\nmuted=0\nclock_set=1\nutc_offset_min=0\n";
    for bad in [
        "rumble=2\nff_speed=5\nff_sound=9\n",
        "rumble=\nff_speed=1\nff_sound=on\n",
        "rumble=-1\nff_speed=0\nff_sound=-1\n",
        "ff_speed=x\n",
        "colour_correction=2\n",
        "colour_correction=\n",
        "colour_correction=Auto\n",
        "wifi=2\n",
        "wifi=on\n",
        "ff_speed=5\n",
        "ff_speed=7\n",
        "ff_speed=8\n",
        "ff_speed=255\n",
    ] {
        std::fs::write(
            d.path().join("Config/omega-zero.state"),
            format!("{known}{bad}"),
        )
        .unwrap();
        let s = read_app_state(d.path());
        assert_eq!(
            (
                s.rumble,
                s.ff_speed,
                s.ff_sound,
                s.colour_correction,
                s.wifi
            ),
            (true, FF_SPEED_DEFAULT, false, false, false),
            "accepted {bad:?}"
        );
        assert_eq!(
            (s.cart.as_deref(), s.brightness, s.volume, s.clock_set),
            (Some("Emerald"), 3, 40, true),
            "{bad:?} took the rest of the card with it"
        );
    }
}

#[test]
fn every_platform_round_trips_as_its_own_line() {
    for platform in Platform::ALL {
        let d = tmp_root();
        let s = AppState {
            cart: Some("Tetris".into()),
            cart_platform: Some(platform),
            clock_set: true,
            ..AppState::default()
        };
        write_app_state(d.path(), &s).unwrap();
        assert_eq!(
            read_app_state(d.path()),
            s,
            "{platform:?} did not come back"
        );
        let text = std::fs::read_to_string(d.path().join("Config/omega-zero.state")).unwrap();
        let want = format!("cart_platform={}", platform.dir_name().to_lowercase());
        assert!(text.lines().any(|l| l == want), "no {want} in {text:?}");
    }
}

#[test]
fn an_empty_slot_writes_an_empty_platform() {
    let d = tmp_root();
    let s = AppState {
        clock_set: true,
        ..AppState::default()
    };
    write_app_state(d.path(), &s).unwrap();
    let text = std::fs::read_to_string(d.path().join("Config/omega-zero.state")).unwrap();
    assert!(
        text.lines().any(|l| l == "cart_platform="),
        "no empty cart_platform line in {text:?}"
    );
    assert_eq!(read_app_state(d.path()).cart_platform, None);
}

#[test]
fn an_unreadable_platform_reads_as_a_card_that_never_said() {
    let d = tmp_root();
    let known = "cart=Tetris\nbrightness=3\nblue_light=1\nvolume=40\nmuted=0\nclock_set=1\nutc_offset_min=0\n";
    for (line, want) in [
        ("cart_platform=\n", None),
        ("cart_platform=nes\n", None),
        ("cart_platform=gameboy\n", None),
        ("cart_platform=0\n", None),
        ("cart_platform=nds\n", None),
        ("cart_platform=GBA\n", Some(Platform::Gba)),
        ("cart_platform=Gbc\n", Some(Platform::Gbc)),
    ] {
        std::fs::write(
            d.path().join("Config/omega-zero.state"),
            format!("{known}{line}"),
        )
        .unwrap();
        let s = read_app_state(d.path());
        assert_eq!(s.cart_platform, want, "read {line:?} wrong");
        assert_eq!(
            (s.cart.as_deref(), s.brightness, s.volume, s.clock_set),
            (Some("Tetris"), 3, 40, true),
            "{line:?} took the rest of the card with it"
        );
    }
}

#[test]
fn an_offset_outside_the_range_of_real_zones_reads_as_default() {
    let d = tmp_root();
    for body in [
        "cart=\nbrightness=5\nblue_light=0\nvolume=60\nmuted=0\nclock_set=1\nutc_offset_min=900\n",
        "cart=\nbrightness=5\nblue_light=0\nvolume=60\nmuted=0\nclock_set=1\nutc_offset_min=-780\n",
    ] {
        std::fs::write(d.path().join("Config/omega-zero.state"), body).unwrap();
        assert_eq!(read_app_state(d.path()), AppState::default(), "{body}");
    }
}

#[test]
fn the_background_round_trips_as_its_own_lines() {
    for style in BackdropStyle::ALL {
        for (colour, name) in BACKDROP_COLOURS.iter().enumerate() {
            let d = tmp_root();
            let s = AppState {
                clock_set: true,
                backdrop: Some(style),
                backdrop_moving: colour % 2 == 0,
                backdrop_colour: colour as u8,
                ..AppState::default()
            };
            write_app_state(d.path(), &s).unwrap();
            assert_eq!(read_app_state(d.path()), s);
            let text = std::fs::read_to_string(d.path().join("Config/omega-zero.state")).unwrap();
            assert!(text.contains(&format!("backdrop={}\n", style.key())));
            assert!(text.contains(&format!("backdrop_colour={name}\n")));
        }
    }
}

#[test]
fn a_card_that_never_picked_a_background_reads_as_not_having_picked_one() {
    let d = tmp_root();
    std::fs::write(
        d.path().join("Config/omega-zero.state"),
        "cart=\nbrightness=3\nblue_light=1\nvolume=40\nmuted=0\nclock_set=1\nutc_offset_min=0\n",
    )
    .unwrap();
    let s = read_app_state(d.path());
    assert_eq!(s.backdrop, None);
    assert!(s.backdrop_moving, "a pattern starts out drifting");
    assert_eq!(s.backdrop_colour, 0);
    let d = tmp_root();
    write_app_state(d.path(), &AppState::default()).unwrap();
    assert_eq!(
        read_app_state(d.path()).backdrop,
        None,
        "an empty line picked one"
    );
}

#[test]
fn a_background_the_reader_does_not_know_falls_back_to_the_default() {
    let d = tmp_root();
    std::fs::write(
        d.path().join("Config/omega-zero.state"),
        "cart=\nbrightness=3\nblue_light=1\nvolume=40\nmuted=0\nclock_set=1\nutc_offset_min=0\nbackdrop=plaid\nbackdrop_moving=7\nbackdrop_colour=mauve\n",
    )
    .unwrap();
    let s = read_app_state(d.path());
    assert_eq!(
        (s.backdrop, s.backdrop_moving, s.backdrop_colour),
        (None, true, 0)
    );
    assert_eq!(s.brightness, 3, "a bad background line sank the rest");
}

#[test]
fn the_font_round_trips_as_its_own_line() {
    for font in Font::ALL {
        let d = tmp_root();
        let s = AppState {
            clock_set: true,
            font,
            ..AppState::default()
        };
        write_app_state(d.path(), &s).unwrap();
        assert_eq!(read_app_state(d.path()), s, "{font:?} did not come back");
        let text = std::fs::read_to_string(d.path().join("Config/omega-zero.state")).unwrap();
        let want = format!("font={}", font.key());
        assert!(text.lines().any(|l| l == want), "no {want} in {text:?}");
    }
    assert_eq!(
        Font::ALL.map(Font::key),
        ["bold", "thin"],
        "the keys a card keeps changed"
    );
}

#[test]
fn ssh_round_trips_as_on_or_off() {
    for ssh in [true, false] {
        let d = tmp_root();
        let s = AppState {
            clock_set: true,
            ssh,
            ..AppState::default()
        };
        write_app_state(d.path(), &s).unwrap();
        assert_eq!(read_app_state(d.path()), s, "ssh {ssh} did not come back");
        let text = std::fs::read_to_string(d.path().join("Config/omega-zero.state")).unwrap();
        let want = if ssh { "ssh=on" } else { "ssh=off" };
        assert!(text.lines().any(|l| l == want), "no {want} in {text:?}");
    }
}

#[test]
fn ssh_is_on_unless_the_card_says_off() {
    assert!(AppState::default().ssh, "a first boot switches SSH off");
    let d = tmp_root();
    let known =
        "cart=\nbrightness=3\nblue_light=1\nvolume=40\nmuted=0\nclock_set=1\nutc_offset_min=0\n";
    for (line, on) in [
        ("", true),
        ("ssh=on\n", true),
        ("ssh=off\n", false),
        ("ssh=\n", true),
        ("ssh=0\n", true),
        ("ssh=OFF\n", true),
    ] {
        std::fs::write(
            d.path().join("Config/omega-zero.state"),
            format!("{known}{line}"),
        )
        .unwrap();
        let s = read_app_state(d.path());
        assert_eq!(s.ssh, on, "{line:?}");
        assert_eq!(
            s.brightness, 3,
            "{line:?} took the rest of the card with it"
        );
    }
}

#[test]
fn bluetooth_round_trips_as_on_or_off() {
    for bluetooth in [true, false] {
        let d = tmp_root();
        let s = AppState {
            clock_set: true,
            bluetooth,
            ..AppState::default()
        };
        write_app_state(d.path(), &s).unwrap();
        assert_eq!(
            read_app_state(d.path()),
            s,
            "bluetooth {bluetooth} did not come back"
        );
        let text = std::fs::read_to_string(d.path().join("Config/omega-zero.state")).unwrap();
        let want = if bluetooth {
            "bluetooth=on"
        } else {
            "bluetooth=off"
        };
        assert!(text.lines().any(|l| l == want), "no {want} in {text:?}");
    }
}

#[test]
fn bluetooth_is_off_unless_the_card_says_on() {
    assert!(
        !AppState::default().bluetooth,
        "a first boot switches Bluetooth on"
    );
    let d = tmp_root();
    let known =
        "cart=\nbrightness=3\nblue_light=1\nvolume=40\nmuted=0\nclock_set=1\nutc_offset_min=0\n";
    for (line, on) in [
        ("", false),
        ("bluetooth=on\n", true),
        ("bluetooth=off\n", false),
        ("bluetooth=\n", false),
        ("bluetooth=1\n", false),
        ("bluetooth=ON\n", false),
    ] {
        std::fs::write(
            d.path().join("Config/omega-zero.state"),
            format!("{known}{line}"),
        )
        .unwrap();
        let s = read_app_state(d.path());
        assert_eq!(s.bluetooth, on, "{line:?}");
        assert_eq!(
            s.brightness, 3,
            "{line:?} took the rest of the card with it"
        );
    }
}

#[test]
fn a_font_the_reader_does_not_know_or_never_saw_is_bold() {
    let d = tmp_root();
    let known =
        "cart=\nbrightness=3\nblue_light=1\nvolume=40\nmuted=0\nclock_set=1\nutc_offset_min=0\n";
    for line in ["", "font=\n", "font=Thin\n", "font=light\n", "font=1\n"] {
        std::fs::write(
            d.path().join("Config/omega-zero.state"),
            format!("{known}{line}"),
        )
        .unwrap();
        let s = read_app_state(d.path());
        assert_eq!(s.font, Font::Bold, "read {line:?} as {:?}", s.font);
        assert_eq!(
            s.brightness, 3,
            "{line:?} took the rest of the card with it"
        );
    }
    assert_eq!(AppState::default().font, Font::Bold);
}
