//! About, opened from Settings: a modal like Settings', OMEGA-ZERO's rows (the version, the
//! build, the author, the handheld and its battery) and the CREDITS under them, held pixel for
//! pixel to flow.html in Chrome (about/ref/, made by about/make-refs.sh from
//! assets/ui/flow.html) at the canvas the tests run at, in Night and in Amber. It only slides,
//! in and back out like every screen, over the backdrop that stays where it is under all of
//! them.
//!
//! Set ABOUT_OUT to a folder to have each face written there as a PNG.

use crate::common;

use common::{app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};
use omega_input::{Action, Btn};
use omega_power::{Battery, Charge};
use omega_store::{colour_index, write_app_state, AppState, BackdropStyle};
use omega_ui::kit::layout::{modal_rect, screen_px};
use omega_ui::kit::palette::palette;
use omega_ui::kit::panel::panel_face;
use omega_ui::kit::pixel::text_width;
use omega_ui::{backdrop_shift, out_h, out_w, Draw, SettingsRow, TexId, UndoFace};
use omega_zero::app::{App, Phase};
use omega_zero::build_info::Build;
use omega_zero::nav::{offsets, progress, Dir, End, SLIDE_MS};
use omega_zero::screens::about::{
    about_face, about_panels, battery_words, credits_height, AboutView, AUTHOR, CREDITS,
    CREDITS_TITLE, TITLE,
};
use omega_zero::screens::lone::{Lone, NOTE_X};
use tempfile::TempDir;

const MODAL: TexId = TexId::from_raw(800);
const PATTERN: TexId = TexId::from_raw(700);

/// The build flow.html's About shows.
fn flow_build() -> Build {
    Build {
        version: "0.0.2",
        hash: "68cdbc4",
        dirty: false,
        date: "2026-10-11",
    }
}

/// What flow.html's About shows: on an RG35XX SP, 86% and charging.
fn flow_view() -> AboutView {
    let battery = Battery {
        percent: 86,
        charge: Charge::Charging,
    };
    AboutView::new(&flow_build(), Some("RG35XX SP"), Some(battery))
}

fn read_png(path: &str) -> UndoFace {
    let file = std::fs::File::open(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().expect("a PNG");
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("its pixels");
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type.samples() {
        4 => buf,
        _ => buf
            .chunks(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
    };
    UndoFace {
        rgba,
        w: info.width,
        h: info.height,
    }
}

/// The colour of `face`'s pixel at `x, y`, None where it is clear.
fn px(face: &UndoFace, x: u32, y: u32) -> Option<[u8; 3]> {
    let i = ((y * face.w + x) * 4) as usize;
    (face.rgba[i + 3] != 0).then(|| [face.rgba[i], face.rgba[i + 1], face.rgba[i + 2]])
}

#[test]
fn about_is_flows_pixel_for_pixel() {
    let canvas = format!("{}x{}", out_w(), out_h());
    for name in ["night", "amber"] {
        let colour = colour_index(name).expect("a backdrop colour");
        let ours = about_face(&flow_view(), palette(colour));
        if let Ok(dir) = std::env::var("ABOUT_OUT") {
            crate::lone::write(&format!("{dir}/about-{canvas}-{name}.png"), &ours);
        }
        let path = format!(
            "{}/tests/suite/about/ref/about-{canvas}-{name}.png",
            env!("CARGO_MANIFEST_DIR")
        );
        let shot = read_png(&path);
        assert_eq!((ours.w, ours.h), (shot.w, shot.h), "{name}'s size");
        let [_, _, w, h] = modal_rect();
        assert_eq!(
            (ours.w, ours.h),
            (w, h),
            "the modal is not modal_rect()'s size"
        );
        let off: Vec<_> = (0..ours.h)
            .flat_map(|y| (0..ours.w).map(move |x| (x, y)))
            .filter_map(|(x, y)| {
                let c = px(&ours, x, y)?;
                let want = px(&shot, x, y).expect("an opaque screenshot");
                (c != want).then_some((x, y, c, want))
            })
            .collect();
        assert!(
            off.is_empty(),
            "{canvas} {name}: {} art pixels differ from flow.html's, first (x, y, ours, Chrome's) {:?}",
            off.len(),
            &off[..off.len().min(8)]
        );
    }
}

#[test]
fn the_rows_say_the_version_the_build_the_author_the_handheld_and_its_battery() {
    let rows = |view: AboutView| -> Vec<(&str, String)> { view.rows };
    assert_eq!(
        rows(flow_view()),
        [
            ("VERSION", "0.0.2".to_string()),
            ("BUILD", "68CDBC4".to_string()),
            ("AUTHOR", "@OAURAMOS".to_string()),
            ("HANDHELD", "RG35XX SP".to_string()),
            ("BATTERY", "86% CHARGING".to_string()),
        ]
    );
    assert_eq!(AUTHOR, "@OAURAMOS");

    // a build with changes not committed is marked, and a handheld BaseOS does not name, or a
    // battery that says nothing, is said to be unknown rather than left blank
    let dirty = Build {
        dirty: true,
        ..flow_build()
    };
    let view = AboutView::new(&dirty, Some("rg35xxsp"), None);
    assert_eq!(view.rows[1].1, "68CDBC4-DIRTY");
    assert_eq!(view.rows[3].1, "RG35XXSP", "a name is shown in capitals");
    assert_eq!(view.rows[4].1, "UNKNOWN");
    assert_eq!(
        AboutView::new(&flow_build(), None, None).rows[3].1,
        "UNKNOWN"
    );
}

#[test]
fn the_battery_says_how_full_it_is_and_whether_it_charges() {
    let at = |percent, charge| battery_words(Some(Battery { percent, charge }));
    assert_eq!(at(86, Charge::Charging), "86% CHARGING");
    assert_eq!(at(100, Charge::Full), "100% CHARGED");
    assert_eq!(at(42, Charge::Discharging), "42%");
    assert_eq!(at(42, Charge::Unknown), "42%");
    assert_eq!(battery_words(None), "UNKNOWN");
}

#[test]
fn every_credit_is_kept_and_slot_is_not_named() {
    let all = CREDITS.join("\n");
    for owed in [
        "mGBA (MPL-2.0)",
        "gpSP (GPL-2.0)",
        "Snes9x, Genesis Plus GX (no commercial use)",
        "Mednafen's Supafaust",
        "Beetle PCE Fast",
        "PCSX-ReARMed, FCEUmm (all GPL-2.0)",
        "BaseOS by pvaibhav",
        "Pixel Code by Qwerasd",
        "GigaHerz's LCD3x",
        "ScreenScraper",
        "Claude",
        "GPL-3.0",
        "README",
    ] {
        assert!(
            all.contains(owed),
            "the credits do not mention {owed}: {all}"
        );
    }
    assert!(
        !all.contains("slot"),
        "slot is named on the handheld, which only the repository does: {all}"
    );
    assert_eq!((TITLE, CREDITS_TITLE), ("OMEGA-ZERO", "CREDITS"));
}

/// Every line of the credits is whole in its well, on this canvas, under the rows: none is cut
/// short, and the last stands clear of the well's bottom.
#[test]
fn the_credits_fit_whole_under_the_rows() {
    let [_, _, w, h] = modal_rect();
    let view = flow_view();
    let [_, [_, _, pw, ph]] = about_panels(w, h, view.rows.len());
    let well = panel_face(pw, ph, CREDITS_TITLE, true, palette(0)).well;
    assert!(
        credits_height() <= well[3],
        "the credits are {} tall in a well {} tall",
        credits_height(),
        well[3]
    );
    for line in CREDITS {
        assert!(
            text_width(line) <= well[2] - NOTE_X,
            "{line:?} is cut short in a well {} wide",
            well[2]
        );
    }
}

fn on_settings() -> (TempDir, App) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_app_state(
        d.path(),
        &AppState {
            clock_set: true,
            ..AppState::default()
        },
    )
    .unwrap();
    let (mut a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    a.tick_ms(16);
    let [_, _, w, h] = modal_rect();
    a.set_about_modal(Lone { tex: MODAL, w, h });
    a.set_backdrop_face(PATTERN, out_w() + 40, BackdropStyle::Grid, 0);
    press(&mut a, Btn::Start);
    a.finish_slide();
    (d, a)
}

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn frame(a: &App) -> Vec<Draw> {
    let mut out = Vec::new();
    a.draw(&mut out);
    out
}

/// Where `tex` is first drawn, and its place in the frame.
fn placed(out: &[Draw], tex: TexId) -> Option<(usize, f32, f32)> {
    out.iter().enumerate().find_map(|(i, d)| match *d {
        Draw::Tex { x, y, tex: t, .. } if t == tex => Some((i, x, y)),
        _ => None,
    })
}

/// Halfway through the slide under way: the modal is moved with its screen, and the backdrop
/// is drawn once, before it, where it is when nothing slides.
#[track_caller]
fn halfway(a: &mut App, dir: Dir, end: End) {
    let started = a.slide().expect("the move did not slide").started;
    a.tick_ms(started + SLIDE_MS / 2);
    let out = frame(a);
    let dx = offsets(dir, end, progress(started, a.now()), out_w() as f32).dx;
    assert_ne!(dx, 0.0, "halfway, the modal is home");
    let (at, x, _) = placed(&out, MODAL).expect("the modal is not drawn while it slides");
    assert_eq!(
        x,
        screen_px(modal_rect()[0]) + dx,
        "the modal does not slide with its screen"
    );
    let (under, px, _) = placed(&out, PATTERN).expect("no backdrop under the slide");
    assert!(under < at, "the backdrop is drawn over the modal");
    assert_eq!(
        px,
        -(backdrop_shift(BackdropStyle::Grid, true, a.now()) as f32),
        "the backdrop slid"
    );
    let patterns = out
        .iter()
        .filter(|d| matches!(**d, Draw::Tex { tex, .. } if tex == PATTERN))
        .count();
    assert_eq!(patterns, 1, "a backdrop for each end of the slide");
}

#[test]
fn the_modal_only_slides_in_and_out_over_a_backdrop_that_stays() {
    let (_d, mut a) = on_settings();
    while a.settings_row() != Some(SettingsRow::About) {
        press(&mut a, Btn::Down);
    }
    press(&mut a, Btn::A);
    assert!(matches!(a.phase(), Phase::About));
    assert!(a.about_view().is_some(), "About open shows nothing");
    halfway(&mut a, Dir::Deeper, End::Arriving);

    a.finish_slide();
    let (_, x, y) = placed(&frame(&a), MODAL).expect("the modal at rest");
    let [mx, my, _, _] = modal_rect();
    assert_eq!(
        (x, y),
        (screen_px(mx), screen_px(my)),
        "the modal did not come to rest where every modal goes"
    );

    press(&mut a, Btn::B);
    assert_eq!(a.settings_row(), Some(SettingsRow::About));
    assert!(a.about_view().is_none(), "About shows once it is left");
    halfway(&mut a, Dir::Back, End::Leaving);
}
