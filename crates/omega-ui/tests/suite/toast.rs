use omega_ui::kit::gauge::GAUGE_H;
use omega_ui::kit::palette::palette;
use omega_ui::kit::toast::TOAST_H;
use omega_ui::{out_w, Draw, Hud, HudFace, HudKind, TexId, Toast, SCREEN_MARGIN};

const TOAST_TEX: TexId = TexId::from_raw(800);
const GAUGE_TEX: TexId = TexId::from_raw(700);

/// `h` given a face for the toast it last said, as the frontend draws it.
fn with_toast_face(h: &mut Hud) {
    let toast = h.last_toast().expect("nothing said");
    let face = toast.face(palette(0));
    let face = HudFace {
        tex: TOAST_TEX,
        w: face.w,
        h: face.h,
    };
    h.set_toast_face(toast, face);
}

fn drawn(h: &Hud, now: u64) -> Vec<Draw> {
    let mut out = Vec::new();
    h.draw(now, &mut out);
    out
}

fn toast_draw(out: &[Draw]) -> Option<(f32, f32, f32, f32)> {
    out.iter().find_map(|d| match *d {
        Draw::Tex {
            x, y, w, h, tex, ..
        } if tex == TOAST_TEX => Some((x, y, w, h)),
        _ => None,
    })
}

#[test]
fn saving_and_loading_say_which_one_happened() {
    assert_eq!(Toast::StateSaved.text(), "State saved");
    assert_eq!(Toast::StateLoaded.text(), "State loaded");
    assert_eq!(Toast::StateSaved.label(), "SAVE STATE");
    assert_eq!(Toast::StateLoaded.label(), "SAVE STATE");
}

#[test]
fn the_link_shortcut_on_the_wrong_core_says_to_switch() {
    assert_eq!(Toast::NeedsGpsp.text(), "Please switch to gpSP");
    assert_eq!(Toast::NeedsGpsp.label(), "LINK");
}

#[test]
fn a_cart_gpsp_cannot_link_says_there_is_no_link() {
    assert_eq!(Toast::NoLink.text(), "No link support");
    assert_eq!(Toast::NoLink.label(), "LINK");
}

#[test]
fn colour_correction_says_whether_it_is_on() {
    assert_eq!(Toast::ColourOn.text(), "Correction on");
    assert_eq!(Toast::ColourOff.text(), "Correction off");
    assert_eq!(Toast::ColourOn.label(), "COLOUR");
    assert_eq!(Toast::ColourOff.label(), "COLOUR");
}

#[test]
fn lcd_ghosting_says_whether_it_is_on() {
    assert_eq!(Toast::GhostingOn.text(), "Ghosting on");
    assert_eq!(Toast::GhostingOff.text(), "Ghosting off");
    assert_eq!(Toast::GhostingOn.label(), "GHOSTING");
    assert_eq!(Toast::GhostingOff.label(), "GHOSTING");
}

#[test]
fn every_link_toast_is_said_under_link() {
    for t in [
        Toast::NeedsGpsp,
        Toast::NoLink,
        Toast::LinkEnded,
        Toast::PeerEnded,
        Toast::BiosMismatch,
    ] {
        assert_eq!(t.label(), "LINK", "{t:?}");
    }
}

#[test]
fn the_banner_says_what_happened_and_never_what_is_on_screen() {
    assert_eq!(
        Toast::ALL,
        [
            Toast::StateSaved,
            Toast::StateLoaded,
            Toast::NeedsGpsp,
            Toast::NoLink,
            Toast::LinkEnded,
            Toast::PeerEnded,
            Toast::BiosMismatch,
            Toast::ColourOn,
            Toast::ColourOff,
            Toast::GhostingOn,
            Toast::GhostingOff,
            Toast::OneDisc,
            Toast::Disc(1),
        ],
        "a banner was added or dropped"
    );
    for (i, t) in Toast::ALL.iter().enumerate() {
        assert_eq!(t.index(), i, "{t:?} does not answer to its own place");
    }
    assert_eq!(Toast::Disc(3).index(), Toast::Disc(1).index());
}

#[test]
fn a_disc_toast_says_which_disc_went_in_under_disc() {
    assert_eq!(Toast::Disc(1).text(), "Disc 1 inserted");
    assert_eq!(Toast::Disc(2).text(), "Disc 2 inserted");
    assert_eq!(Toast::Disc(9).text(), "Disc 9 inserted");
    assert_eq!(Toast::Disc(0).text(), "Disc inserted");
    assert_eq!(Toast::Disc(12).text(), "Disc inserted");
    assert_eq!(Toast::OneDisc.text(), "Only one disc");
    for t in [Toast::OneDisc, Toast::Disc(2)] {
        assert_eq!(t.label(), "DISC", "{t:?}");
    }
    assert_ne!(
        Toast::Disc(2).face(palette(0)).rgba,
        Toast::Disc(3).face(palette(0)).rgba,
        "two discs' toasts look the same"
    );
}

#[test]
fn every_toast_is_one_line_in_a_panel_of_one_height() {
    let pal = palette(0);
    for t in Toast::ALL {
        let f = t.face(pal);
        assert_eq!(f.h, TOAST_H, "{t:?} is not one line tall");
        assert_eq!(f.w % 2, 0, "{t:?} is an odd width and cannot be centred");
        // the panel is opaque from edge to edge but for its four corner pixels
        let clear: Vec<(u32, u32)> = (0..f.h)
            .flat_map(|y| (0..f.w).map(move |x| (x, y)))
            .filter(|&(x, y)| f.rgba[((y * f.w + x) * 4 + 3) as usize] == 0)
            .collect();
        let (r, b) = (f.w - 1, f.h - 1);
        assert_eq!(clear, vec![(0, 0), (r, 0), (0, b), (r, b)], "{t:?}");
    }
}

#[test]
fn a_toast_follows_the_palette() {
    let (night, amber) = (palette(0), palette(7));
    let face = Toast::StateSaved.face(amber);
    assert_ne!(face.rgba, Toast::StateSaved.face(night).rgba);
    assert!(
        face.rgba
            .chunks(4)
            .any(|p| p[3] == 255 && p[..3] == amber.cream),
        "the words are not in the palette's cream"
    );
}

#[test]
fn a_toast_fades_on_the_same_curve_as_the_gauge() {
    let mut h = Hud::new();
    h.toast(Toast::StateSaved, 1_000);
    assert!(h.toast_visible(2_499));
    assert!(!h.toast_visible(2_500));
}

#[test]
fn saying_the_same_thing_twice_re_shows_it_rather_than_stacking() {
    let mut h = Hud::new();
    h.toast(Toast::StateSaved, 1_000);
    h.toast(Toast::StateSaved, 2_400);
    assert_eq!(h.said(3_400), Some(Toast::StateSaved), "it did not re-show");
    assert_eq!(h.said(3_900), None, "it never faded");
}

#[test]
fn a_second_banner_replaces_the_first() {
    let mut h = Hud::new();
    h.toast(Toast::StateSaved, 1_000);
    h.toast(Toast::LinkEnded, 1_100);
    assert_eq!(h.said(1_200), Some(Toast::LinkEnded));
    with_toast_face(&mut h);
    assert_eq!(drawn(&h, 1_200).len(), 1);
}

#[test]
fn a_toast_is_centred_at_the_top_on_the_grid() {
    let mut h = Hud::new();
    h.toast(Toast::StateSaved, 0);
    with_toast_face(&mut h);
    let (x, y, w, th) = toast_draw(&drawn(&h, 0)).expect("the toast is not drawn");
    assert_eq!(x + w / 2.0, out_w() as f32 / 2.0);
    assert_eq!((x % 2.0, y % 2.0), (0.0, 0.0), "off the art grid");
    assert_eq!(y, SCREEN_MARGIN);
    assert_eq!(th, (TOAST_H * 2) as f32, "not drawn at 2x");
}

#[test]
fn a_toast_stands_alone_without_a_plate() {
    let mut h = Hud::new();
    h.toast(Toast::StateSaved, 0);
    with_toast_face(&mut h);
    let out = drawn(&h, 0);
    assert!(
        !out.iter().any(|d| matches!(d, Draw::Rect { .. })),
        "a plate came back behind the toast"
    );
}

#[test]
fn a_toast_takes_the_gauges_place() {
    let mut h = Hud::new();
    h.show(HudKind::Volume, 50, false, 0);
    let gauge = h.gauge().expect("a gauge");
    let face = HudFace {
        tex: GAUGE_TEX,
        w: 157,
        h: GAUGE_H,
    };
    h.set_gauge_face(gauge, face);
    h.toast(Toast::StateSaved, 0);
    with_toast_face(&mut h);
    let out = drawn(&h, 0);
    assert!(toast_draw(&out).is_some(), "the toast is not drawn");
    assert!(
        !out.iter()
            .any(|d| matches!(*d, Draw::Tex { tex, .. } if tex == GAUGE_TEX)),
        "the gauge is still drawn underneath the toast"
    );
}
