//! Settings, opened with START on the shelf, as assets/ui/flow.html's #screen=settings draws it:
//! an omega-modal filling the page above the bottom bar, holding CONFIGS, as tall as its rows,
//! and GENERAL under it, taking the rest. The panel holding the cursor has its title lit, and
//! the cursor's row is lit.
//!
//! What it costs to draw: the modal and both panels are one face, baked once per canvas and
//! palette with both titles dim; each panel's lit title is a face of its own, drawn over its dim
//! one for the panel the cursor is in; and each row is a face, baked again only when what it
//! says, whether it is lit or the palette changes. [`SettingsBaked`] keeps that account, so a
//! sync asks it what to bake and bakes nothing more.

use crate::draw::{out_h, out_w, Draw, TexId};
use crate::kit::layout::{modal_rect, panel_height, screen_px};
use crate::kit::modal::{modal_face, modal_layout};
use crate::kit::palette::Palette;
use crate::kit::panel::{panel_face, title_face};
use crate::kit::pixel::quad;
use crate::kit::row::{row_face, row_rect, row_width, Row, ROW_H};
use crate::plate::UndoFace;

/// The two groups the settings are listed in, one panel each.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SettingsGroup {
    /// How the games play: the settings left and right change in place, and the cart labels.
    Configs,
    /// The handheld itself: its clock, its network, its Bluetooth, its background and what it
    /// is.
    General,
}

impl SettingsGroup {
    pub const ALL: [SettingsGroup; 2] = [SettingsGroup::Configs, SettingsGroup::General];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            SettingsGroup::Configs => "Configs",
            SettingsGroup::General => "General",
        }
    }

    /// The group's panel title, in the capitals the tab prints.
    pub fn title(self) -> &'static str {
        match self {
            SettingsGroup::Configs => "CONFIGS",
            SettingsGroup::General => "GENERAL",
        }
    }
}

/// A row of the settings START opens on the shelf, in the order they are listed.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SettingsRow {
    FastForward,
    FastForwardSound,
    ColourCorrection,
    /// The handhelds' cores blend each frame with the last, as the LCD ghosted.
    LcdGhosting,
    Rumble,
    Labels,
    DateTime,
    Network,
    Bluetooth,
    /// THEME on the screen: the background, its motion and the colour everything takes, and the
    /// bar's font.
    Background,
    About,
}

impl SettingsRow {
    pub const ALL: [SettingsRow; 11] = [
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
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            SettingsRow::FastForward => "Fast Forward",
            SettingsRow::FastForwardSound => "Fast Forward Sound",
            SettingsRow::ColourCorrection => "Colour Correction",
            SettingsRow::LcdGhosting => "LCD Ghosting",
            SettingsRow::Rumble => "Rumble",
            SettingsRow::Labels => "Labels",
            SettingsRow::DateTime => "Date & Time",
            SettingsRow::Network => "Network",
            SettingsRow::Bluetooth => "Bluetooth",
            SettingsRow::Background => "Theme",
            SettingsRow::About => "About",
        }
    }

    pub fn group(self) -> SettingsGroup {
        match self {
            SettingsRow::FastForward
            | SettingsRow::FastForwardSound
            | SettingsRow::ColourCorrection
            | SettingsRow::LcdGhosting
            | SettingsRow::Rumble
            | SettingsRow::Labels => SettingsGroup::Configs,
            SettingsRow::DateTime
            | SettingsRow::Network
            | SettingsRow::Bluetooth
            | SettingsRow::Background
            | SettingsRow::About => SettingsGroup::General,
        }
    }

    /// A opens a screen of its own for this row, rather than left and right changing it: the
    /// row ends in a ›.
    pub fn opens(self) -> bool {
        matches!(
            self,
            SettingsRow::Labels
                | SettingsRow::DateTime
                | SettingsRow::Network
                | SettingsRow::Bluetooth
                | SettingsRow::Background
                | SettingsRow::About
        )
    }

    /// The row's value stands between a ◀ and a ▶ while it is lit: every row Left and Right
    /// change, Fast Forward's speeds and the ON/OFF switches alike, as on the Network screen.
    pub fn carets(self) -> bool {
        matches!(
            self,
            SettingsRow::FastForward
                | SettingsRow::FastForwardSound
                | SettingsRow::ColourCorrection
                | SettingsRow::LcdGhosting
                | SettingsRow::Rumble
        )
    }

    /// The row above this one among `rows`, the rows a handheld shows, wrapping at the top.
    pub fn up_in(self, rows: &[SettingsRow]) -> SettingsRow {
        let n = rows.len();
        let at = rows.iter().position(|r| *r == self).unwrap_or(0);
        rows[(at + n - 1) % n]
    }

    pub fn down_in(self, rows: &[SettingsRow]) -> SettingsRow {
        let at = rows.iter().position(|r| *r == self).unwrap_or(0);
        rows[(at + 1) % rows.len()]
    }

    pub fn up(self) -> SettingsRow {
        self.up_in(&SettingsRow::ALL)
    }

    pub fn down(self) -> SettingsRow {
        self.down_in(&SettingsRow::ALL)
    }
}

/// What a config row can be set to.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum QuickValue {
    Speed2,
    Speed3,
    Speed4,
    Speed6,
    On,
    Off,
}

impl QuickValue {
    pub const ALL: [QuickValue; 6] = [
        QuickValue::Speed2,
        QuickValue::Speed3,
        QuickValue::Speed4,
        QuickValue::Speed6,
        QuickValue::On,
        QuickValue::Off,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    /// The value as its row prints it, in flow.html's capitals.
    pub fn text(self) -> &'static str {
        match self {
            QuickValue::Speed2 => "2X",
            QuickValue::Speed3 => "3X",
            QuickValue::Speed4 => "4X",
            QuickValue::Speed6 => "6X",
            QuickValue::On => "ON",
            QuickValue::Off => "OFF",
        }
    }

    pub fn speed(frames: u8) -> Option<QuickValue> {
        match frames {
            2 => Some(QuickValue::Speed2),
            3 => Some(QuickValue::Speed3),
            4 => Some(QuickValue::Speed4),
            6 => Some(QuickValue::Speed6),
            _ => None,
        }
    }

    pub fn flag(on: bool) -> QuickValue {
        if on {
            QuickValue::On
        } else {
            QuickValue::Off
        }
    }
}

/// One row as Settings shows it now: what it says on its right, and whether the cursor is on
/// it. A row's face is baked from this and the palette, and from nothing else.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowShown {
    pub row: SettingsRow,
    pub value: Option<String>,
    pub lit: bool,
}

/// The modal and its two panels as one face, both titles dim, and where its parts land on the
/// screen, all in art pixels from the screen's top left.
pub struct SettingsFrame {
    /// The modal with its panels in it, at `modal_rect()`.
    pub face: UndoFace,
    /// Where each group's title is, by `SettingsGroup::index`: a `title_face` goes there.
    pub titles: [[u32; 4]; 2],
    /// Each group's well, by `SettingsGroup::index`: where its rows go.
    pub wells: [[u32; 4]; 2],
}

/// Where CONFIGS and GENERAL go in a modal `w` by `h` art pixels, as `[x, y, w, h]` in it:
/// CONFIGS as tall as its `configs` rows, as flow.css's `omega-panel.hug`, and GENERAL taking
/// the rest. GENERAL starts on CONFIGS' last row, so the two share one stripe.
pub fn settings_panels(w: u32, h: u32, configs: u32) -> [[u32; 4]; 2] {
    let [x, y, pw, ph] = modal_layout(w, h, &[1])[0];
    let hug = panel_height(configs * ROW_H).min(ph);
    [[x, y, pw, hug], [x, y + hug - 1, pw, ph + 1 - hug]]
}

/// How many of `rows` are in CONFIGS.
pub fn configs_in(rows: &[SettingsRow]) -> u32 {
    rows.iter()
        .filter(|r| r.group() == SettingsGroup::Configs)
        .count() as u32
}

/// The modal at `modal_rect()` with CONFIGS (holding `configs` rows) and GENERAL in it, drawn
/// in `pal`, both titles dim.
pub fn settings_frame(configs: u32, pal: &Palette) -> SettingsFrame {
    let [mx, my, mw, mh] = modal_rect();
    let mut face = modal_face(mw, mh, pal);
    let mut titles = [[0; 4]; 2];
    let mut wells = [[0; 4]; 2];
    let panels = settings_panels(mw, mh, configs);
    for (group, [px, py, pw, ph]) in SettingsGroup::ALL.into_iter().zip(panels) {
        let panel = panel_face(pw, ph, group.title(), false, pal);
        over(&mut face, &panel.face, px, py);
        let on_screen = |[x, y, w, h]: [u32; 4]| [mx + px + x, my + py + y, w, h];
        titles[group.index()] = on_screen(panel.title);
        wells[group.index()] = on_screen(panel.well);
    }
    SettingsFrame {
        face,
        titles,
        wells,
    }
}

/// `group`'s title lit, for the panel the cursor is in, to draw over its dim one.
pub fn settings_title_face(group: SettingsGroup, pal: &Palette) -> UndoFace {
    title_face(group.title(), true, pal)
}

/// How wide a row is in a well `well_w` art pixels wide: a pixel into either side of it.
pub fn settings_row_width(well_w: u32) -> u32 {
    row_width(well_w)
}

/// `shown`'s row as a face `w` art pixels wide, in `pal`: a › on a row that opens a screen,
/// the ◀ ▶ round a value that steps, lit where the cursor is.
pub fn settings_row_face(w: u32, shown: &RowShown, pal: &Palette) -> UndoFace {
    let row = Row {
        label: shown.row.label(),
        value: shown.value.as_deref(),
        lit: shown.lit,
        carets: shown.row.carets(),
        opens: shown.row.opens(),
        dim: false,
    };
    row_face(w, &row, pal)
}

/// Where each of `rows` goes, `[x, y, w, h]` in art pixels from the screen's top left: each
/// group's rows one under the other from the top of its well (`SettingsFrame::wells`), each a
/// pixel into the well's sides.
pub fn settings_row_rects(
    rows: &[SettingsRow],
    wells: [[u32; 4]; 2],
) -> Vec<(SettingsRow, [u32; 4])> {
    let mut down = [0; 2];
    rows.iter()
        .map(|&row| {
            let g = row.group().index();
            let at = row_rect(wells[g], down[g]);
            down[g] += 1;
            (row, at)
        })
        .collect()
}

/// `top` laid over `under` with its top left at (`x`, `y`) wherever it is not clear.
fn over(under: &mut UndoFace, top: &UndoFace, x: u32, y: u32) {
    for ty in 0..top.h.min(under.h.saturating_sub(y)) {
        for tx in 0..top.w.min(under.w.saturating_sub(x)) {
            let from = ((ty * top.w + tx) * 4) as usize;
            if top.rgba[from + 3] == 0 {
                continue;
            }
            let to = (((y + ty) * under.w + x + tx) * 4) as usize;
            under.rgba[to..to + 4].copy_from_slice(&top.rgba[from..from + 4]);
        }
    }
}

/// A face uploaded, and where it goes: `[x, y, w, h]` in art pixels from the screen's top left,
/// its size the face's own.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FaceAt {
    pub tex: TexId,
    pub at: [u32; 4],
}

impl FaceAt {
    /// The face at 2x, on the art grid.
    pub fn quad(&self) -> Draw {
        let [x, y, w, h] = self.at;
        quad(self.tex, w, h, screen_px(x), screen_px(y))
    }
}

/// What Settings is drawn with, uploaded.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingsFaces {
    /// The modal with both panels in it, their titles dim.
    pub frame: FaceAt,
    /// Each group's title lit, by `SettingsGroup::index`, over its dim one.
    pub titles: [FaceAt; 2],
    /// Each group's well, by `SettingsGroup::index`, in art pixels from the screen's top left.
    pub wells: [[u32; 4]; 2],
    /// Each row's face as it is now, by `SettingsRow::index`, `[w, h]` in art pixels; none for
    /// a row this handheld does not show.
    pub rows: [Option<(TexId, [u32; 2])>; SettingsRow::ALL.len()],
}

impl SettingsFaces {
    /// `frame` uploaded as `frame_tex` and the lit titles as `titles`, with no rows yet.
    pub fn new(frame: &SettingsFrame, frame_tex: TexId, titles: [(TexId, [u32; 2]); 2]) -> Self {
        let [x, y, ..] = modal_rect();
        let lit = |g: usize| {
            let (tex, [w, h]) = titles[g];
            let [tx, ty, ..] = frame.titles[g];
            FaceAt {
                tex,
                at: [tx, ty, w, h],
            }
        };
        SettingsFaces {
            frame: FaceAt {
                tex: frame_tex,
                at: [x, y, frame.face.w, frame.face.h],
            },
            titles: [lit(0), lit(1)],
            wells: frame.wells,
            rows: [None; SettingsRow::ALL.len()],
        }
    }
}

/// Settings over whatever is under it (the backdrop), with `row` lit.
pub struct SettingsMenu<'a> {
    pub row: SettingsRow,
    /// The rows this handheld shows: all of them, but for Network on one without a Wi-Fi radio
    /// and Bluetooth on one without Bluetooth.
    pub rows: &'a [SettingsRow],
    pub faces: Option<&'a SettingsFaces>,
}

impl SettingsMenu<'_> {
    pub fn draw(&self, out: &mut Vec<Draw>) {
        let Some(faces) = self.faces else {
            return;
        };
        out.push(faces.frame.quad());
        out.push(faces.titles[self.row.group().index()].quad());
        for (row, [x, y, ..]) in settings_row_rects(self.rows, faces.wells) {
            if let Some((tex, [w, h])) = faces.rows[row.index()] {
                out.push(quad(tex, w, h, screen_px(x), screen_px(y)));
            }
        }
    }
}

/// What Settings' faces were last baked from, so a sync bakes again only what has changed: the
/// frame and the titles when the canvas, the palette or how many rows CONFIGS holds changes, and
/// a row when what it shows (`RowShown`) changes, or the frame was baked again.
#[derive(Default)]
pub struct SettingsBaked {
    frame: Option<(u8, [u32; 2], u32)>,
    rows: [Option<RowShown>; SettingsRow::ALL.len()],
}

/// What a sync has to bake again.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Rebake {
    /// The frame and both lit titles.
    pub frame: bool,
    /// These rows, as they are to be shown.
    pub rows: Vec<RowShown>,
    /// Rows no longer shown, whose faces are to be dropped.
    pub gone: Vec<SettingsRow>,
}

impl SettingsBaked {
    /// What to bake again to show `shown` in palette `colour`, which is then taken as baked.
    pub fn rebake(&mut self, colour: u8, shown: &[RowShown]) -> Rebake {
        let rows: Vec<SettingsRow> = shown.iter().map(|s| s.row).collect();
        let key = (colour, [out_w(), out_h()], configs_in(&rows));
        let frame = self.frame != Some(key);
        if frame {
            self.frame = Some(key);
            self.rows = Default::default();
        }
        let gone = SettingsRow::ALL
            .into_iter()
            .filter(|r| self.rows[r.index()].is_some() && !rows.contains(r))
            .collect::<Vec<_>>();
        for r in &gone {
            self.rows[r.index()] = None;
        }
        let stale: Vec<RowShown> = shown
            .iter()
            .filter(|s| self.rows[s.row.index()].as_ref() != Some(*s))
            .cloned()
            .collect();
        for s in &stale {
            self.rows[s.row.index()] = Some(s.clone());
        }
        Rebake {
            frame,
            rows: stale,
            gone,
        }
    }
}
