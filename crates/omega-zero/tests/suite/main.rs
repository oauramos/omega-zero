//! omega-zero's tests, built as one binary. Built one per file, linking and starting them took
//! longer than running them. A test that needs a process to itself stays a file of its own in
//! tests/, beside this folder: core and e2e point OMEGA_CORE at a real core for the whole
//! process, and emu runs against the clock.

#[path = "../common/mod.rs"]
mod common;

mod about;
mod app;
mod audio;
mod background_screen;
mod bar;
mod bios;
mod bluetooth;
mod bluetooth_screen;
mod boot;
mod boot_fade;
mod bootlogo;
mod build_info;
mod cable;
mod cable_session;
mod clock;
mod controls;
mod core_options;
mod core_picker;
mod disc_screen;
mod discs;
mod dist;
mod drc;
mod eject;
mod evdev;
mod face_builder;
mod fastforward;
mod flush;
mod frame_size;
mod frames;
mod gpsp;
mod headphones;
mod high_refresh;
mod input;
mod keyboard;
mod keys;
mod label_cuts;
mod label_screen;
mod label_sync;
mod led;
mod levels;
mod link_kind;
mod link_screen;
mod link_session;
mod link_start;
mod link_ui;
mod lone;
mod mgba_gb_link;
mod mgba_link;
mod nav;
mod opening;
mod picker_systems;
mod platform;
mod play;
mod play_rules;
mod polaroids;
mod power;
mod power_screens;
mod refusal;
mod render;
mod render_about;
mod render_bar;
mod render_bluetooth;
mod render_boot_logo;
mod render_colour_correction;
mod render_consoles;
mod render_gb_palette;
mod render_hud;
mod render_keyboard;
mod render_link_screen;
mod render_lone;
mod render_network;
mod render_opening;
mod render_overlay;
mod render_picture_modes;
mod render_power_wake;
mod render_settings;
mod render_shelf;
mod render_shelves;
mod render_start_notice;
mod render_top_bar;
mod resample;
mod resume;
mod rewind;
mod rumble;
mod screen;
mod settings;
mod sfx;
mod single;
mod ssh;
mod ssh_password;
mod start_notice;
mod swap;
mod thumb;
mod toast;
mod top_bar;
mod volume;
mod wallpaper;
mod wifi;
mod wifi_screen;

/// A file in this folder that the list above leaves out is never compiled, let alone run.
#[test]
fn every_file_in_the_folder_is_in_the_suite() {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/suite");
    let listed = include_str!("main.rs");
    for entry in std::fs::read_dir(&folder).expect("the suite's folder") {
        let path = entry.expect("a file in the suite's folder").path();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if path.extension().is_some_and(|e| e == "rs") && stem != "main" {
            assert!(
                listed.contains(&format!("\nmod {stem};")),
                "tests/suite/{stem}.rs is missing from tests/suite/main.rs"
            );
        }
    }
}
