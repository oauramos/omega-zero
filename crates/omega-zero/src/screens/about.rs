//! About, opened from Settings' ABOUT: what omega-zero is, the handheld it runs on and what it is
//! made with, as assets/ui/flow.html's `#screen=about` draws it. It is a modal like Settings':
//! OMEGA-ZERO, as tall as its rows, each a name and a value as SSH's lines are, and CREDITS under
//! it taking the rest, a line for each. Nothing on it is
//! picked, so both titles are lit and no row is; B goes back to Settings.
//!
//! The modal and everything in it are one face at art resolution, baked again only when what it
//! says (the battery) or the palette changes, and drawn as one quad where every modal goes.

use std::sync::OnceLock;

use omega_input::{Action, Btn};
use omega_power::{Battery, Charge};
use omega_ui::kit::layout::{modal_rect, panel_height, screen_px};
use omega_ui::kit::modal::{modal_face, modal_layout};
use omega_ui::kit::palette::Palette;
use omega_ui::kit::panel::panel_face;
use omega_ui::kit::pixel::{quad, LINE};
use omega_ui::kit::row::{row_face, row_rect, Row, ROW_H};
use omega_ui::kit::text::text_face;
use omega_ui::{Draw, SettingsRow, UndoFace};

use crate::app::{App, Phase};
use crate::build_info::Build;
use crate::nav::Dir;
use crate::screens::lone::{fitted, over, Lone, NOTE_GAP, NOTE_X};

/// The panels' titles, top first.
pub const TITLE: &str = "OMEGA-ZERO";
pub const CREDITS_TITLE: &str = "CREDITS";

/// Who made it.
pub const AUTHOR: &str = "@OAURAMOS";

/// What omega-zero is made with and on, a line each, each name in its own case: the cores first,
/// with their licences, Snes9x's and Genesis Plus GX's allowing no commercial use. The longest is as long as a line
/// can be in the credits' well at 640x480, 44 letters, and the well holds eight. No hyphen
/// stands between two small letters: Chrome sets one a pixel lower there than Rust can.
pub const CREDITS: [&str; 8] = [
    "Emulation by mGBA (MPL-2.0), gpSP (GPL-2.0),",
    "Snes9x, Genesis Plus GX (no commercial use),",
    "Mednafen's Supafaust, Beetle PCE Fast,",
    "PCSX-ReARMed, FCEUmm (all GPL-2.0).",
    "BaseOS by pvaibhav, Pixel Code by Qwerasd,",
    "panel mask from GigaHerz's LCD3x.",
    "Labels: ScreenScraper. Built with Claude.",
    "Free software under the GPL-3.0. See README.",
];

/// What a row says when the handheld does not.
const UNKNOWN: &str = "UNKNOWN";

/// The room under the credits' last line: 5, so its capitals stand as far from the well's
/// bottom as the first line's from its top (flow.css's `.credits`).
const CREDITS_FOOT: u32 = 5;

/// What About shows: OMEGA-ZERO's rows, a name and a value each, top first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AboutView {
    pub rows: Vec<(&'static str, String)>,
}

impl AboutView {
    /// About for `build`, on the handheld BaseOS names `handheld`, its battery `battery`.
    pub fn new(build: &Build, handheld: Option<&str>, battery: Option<Battery>) -> AboutView {
        AboutView {
            rows: vec![
                ("VERSION", build.version.to_string()),
                ("BUILD", build.shown()),
                ("AUTHOR", AUTHOR.to_string()),
                ("HANDHELD", handheld.unwrap_or(UNKNOWN).to_uppercase()),
                ("BATTERY", battery_words(battery)),
            ],
        }
    }
}

/// The battery as its row says it: how full it is, and CHARGING while it charges or CHARGED
/// once it is full on the charger.
pub fn battery_words(battery: Option<Battery>) -> String {
    let Some(b) = battery else {
        return UNKNOWN.to_string();
    };
    match b.charge {
        Charge::Charging => format!("{}% CHARGING", b.percent),
        Charge::Full => format!("{}% CHARGED", b.percent),
        Charge::Discharging | Charge::Unknown => format!("{}%", b.percent),
    }
}

/// The handheld this is, as BaseOS names it in /etc/baseos-release: its model, else the target
/// its image was built for; None off BaseOS. Read once.
pub fn handheld() -> Option<&'static str> {
    static NAME: OnceLock<Option<String>> = OnceLock::new();
    NAME.get_or_init(|| {
        omega_power::baseos_release().and_then(|release| release.model.or(release.target))
    })
    .as_deref()
}

/// Where OMEGA-ZERO and CREDITS go in a modal `w` by `h` art pixels, `[x, y, w, h]` in it, for
/// `rows` rows: OMEGA-ZERO as tall as its rows, as flow.css's `omega-panel.hug`, and CREDITS
/// taking the rest, starting on OMEGA-ZERO's last row so the two share one stripe.
pub fn about_panels(w: u32, h: u32, rows: usize) -> [[u32; 4]; 2] {
    let [x, y, pw, ph] = modal_layout(w, h, &[1])[0];
    let hug = panel_height(rows as u32 * ROW_H).min(ph);
    [[x, y, pw, hug], [x, y + hug - 1, pw, ph + 1 - hug]]
}

/// How tall the credits are in their well: `NOTE_GAP` above the first line, a line each, and
/// `CREDITS_FOOT` under the last.
pub fn credits_height() -> u32 {
    NOTE_GAP + CREDITS.len() as u32 * LINE + CREDITS_FOOT
}

/// The About modal showing `view`, in `pal`, as big as `modal_rect()`.
pub fn about_face(view: &AboutView, pal: &Palette) -> UndoFace {
    let [_, _, w, h] = modal_rect();
    let mut face = modal_face(w, h, pal);
    let [top, rest] = about_panels(w, h, view.rows.len());

    let [x, y, pw, ph] = top;
    let mut panel = panel_face(pw, ph, TITLE, true, pal);
    for (i, (label, value)) in view.rows.iter().enumerate() {
        let [rx, ry, rw, _] = row_rect(panel.well, i as u32);
        let row = Row {
            label,
            value: Some(value),
            ..Row::default()
        };
        over(&mut panel.face, &row_face(rw, &row, pal), rx, ry);
    }
    over(&mut face, &panel.face, x, y);

    let [x, y, pw, ph] = rest;
    let mut panel = panel_face(pw, ph, CREDITS_TITLE, true, pal);
    let [wx, wy, ww, _] = panel.well;
    let room = ww.saturating_sub(NOTE_X);
    for (i, line) in CREDITS.iter().enumerate() {
        let text = text_face(&fitted(line, room), pal.label);
        over(
            &mut panel.face,
            &text,
            wx + NOTE_X,
            wy + NOTE_GAP + i as u32 * LINE,
        );
    }
    over(&mut face, &panel.face, x, y);
    face
}

impl App {
    pub(crate) fn about_input(&mut self, action: Action) {
        if action == Action::GbaDown(Btn::B) || action == Action::QuickMenu {
            self.go(
                Phase::Settings {
                    row: SettingsRow::About,
                },
                Dir::Back,
            )
        }
    }

    /// What About shows, while it is open.
    pub fn about_view(&self) -> Option<AboutView> {
        match self.phase {
            Phase::About => Some(AboutView::new(&Build::current(), handheld(), self.battery)),
            _ => None,
        }
    }

    pub fn set_about_modal(&mut self, modal: Lone) {
        self.about_modal = Some(modal);
    }

    /// The modal where every modal goes, at two screen pixels to an art pixel.
    pub(crate) fn draw_about(&self, out: &mut Vec<Draw>) {
        if let Some(modal) = &self.about_modal {
            let [x, y, _, _] = modal_rect();
            out.push(quad(
                modal.tex,
                modal.w,
                modal.h,
                screen_px(x),
                screen_px(y),
            ));
        }
    }
}
