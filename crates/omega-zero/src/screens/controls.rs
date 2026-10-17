//! Controls, opened with a SELECT tap on the shelf and titled CONTROLS: what each button does
//! on the shelf and in a game of the shelf's system, as `App::apply` and the gestures in
//! omega-input do it. A PlayStation game has L2 and R2 of its own, so it fast-forwards on
//! SELECT+R2, has nothing to rewind it and swaps discs on SELECT+MENU; only the GBA and the Game
//! Boys link and have a colour correction; only the GBA and the SNES have a core to pick; only
//! the Game Boys leave L1 and R1 to the picture's size.
//!
//! It is a lone panel, as flow.html's `#screen=controls`: SHELF and IN GAME side by side, each a
//! heading and then a line a button, the buttons in the frame's bright colour and what they do
//! in cream, on the grid flow.css's .keymap sets. It is baked once for each system and palette.

use omega_input::{Action, Btn};
use omega_store::{Core, Platform};
use omega_ui::kit::palette::Palette;
use omega_ui::kit::panel::panel_face;
use omega_ui::kit::pixel::ADVANCE;
use omega_ui::kit::row::panel_height;
use omega_ui::kit::text::text_face;
use omega_ui::{Draw, UndoFace};

use crate::app::{App, Phase};
use crate::nav::Dir;
use crate::screens::lone::{over, Lone};

/// A group of controls: its name, and each control's keys and what they do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlsGroup {
    pub name: &'static str,
    pub lines: Vec<(&'static str, &'static str)>,
}

/// What the Controls screen says, word for word as it is drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlsView {
    pub title: &'static str,
    pub groups: Vec<ControlsGroup>,
}

impl ControlsView {
    /// What the buttons do on the shelf of `platform`'s games, and in one of them.
    pub fn new(platform: Platform) -> Self {
        let mut shelf = vec![("A", "PLAY"), ("HOLD A", "CLEAN START")];
        if Core::for_platform(platform).len() > 1 {
            shelf.push(("X", "CORE"));
        }
        shelf.extend([
            ("START", "SETTINGS"),
            ("SELECT", "CONTROLS"),
            ("◀▶", "BROWSE"),
            ("▲▼", "LETTER"),
            ("L1 R1", "SHELF"),
        ]);

        let mut game = Vec::new();
        if platform.game_owns_l2_r2() {
            game.extend([
                ("SELECT+R2", "FAST FORWARD"),
                ("SELECT+R2 ×2", "LATCH FAST FWD"),
            ]);
        } else {
            if platform.rewinds() {
                game.push(("L2", "REWIND"));
            }
            game.extend([("R2", "FAST FORWARD"), ("R2 ×2", "LATCH FAST FWD")]);
        }
        game.extend([("MENU ×2", "SAVE STATES"), ("HOLD MENU", "EJECT")]);
        match platform {
            Platform::Gba | Platform::Gb | Platform::Gbc => game.push(("SELECT+MENU", "LINK")),
            Platform::Ps1 => game.push(("SELECT+MENU", "DISC")),
            Platform::Snes => {}
        }
        game.extend([
            ("SELECT+▲▼", "BRIGHTNESS"),
            ("SELECT+◀▶", "BLUE LIGHT"),
            ("SELECT+L1", "LOAD STATE"),
            ("SELECT+R1", "SAVE STATE"),
        ]);
        if platform.colour_correction() {
            game.push(("SELECT+Y", "COLOUR"));
        }
        if platform.lcd_ghosting() {
            game.push(("SELECT+X", "GHOSTING"));
        }
        if platform.owns_shoulders() {
            game.push(("L1 R1", "PICTURE SIZE"));
        }
        game.extend([("VOL+ VOL-", "MUTE"), ("HOLD POWER", "POWER MENU")]);

        ControlsView {
            title: "CONTROLS",
            groups: vec![
                ControlsGroup {
                    name: "SHELF",
                    lines: shelf,
                },
                ControlsGroup {
                    name: "IN GAME",
                    lines: game,
                },
            ],
        }
    }
}

impl Default for ControlsView {
    fn default() -> Self {
        ControlsView::new(Platform::default())
    }
}

/// How wide the panel is, in art pixels: flow.css's .controls-panel.
pub const WIDTH: u32 = 304;

/// The room a column gives, in letters, as flow.html's --keys and --does: its buttons, then
/// what they do, a letter apart.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Column {
    pub keys: u32,
    pub does: u32,
}

/// SHELF's column, then IN GAME's, `COLUMN_GAP` apart: together the well's whole width.
pub const COLUMNS: [Column; 2] = [Column { keys: 6, does: 11 }, Column { keys: 12, does: 15 }];
pub const COLUMN_GAP: u32 = 16;

/// A heading's line and the 2 under it, then a pixel to the first line; each line after that
/// a line and a pixel under the one before.
const FIRST_LINE: u32 = 13;
pub const PITCH: u32 = 11;

impl Column {
    /// How wide the column is in art pixels.
    pub fn width(self) -> u32 {
        letters(self.keys + 1 + self.does)
    }

    /// Where what a button does starts, from the column's left.
    pub fn does_x(self) -> u32 {
        letters(self.keys + 1)
    }
}

fn letters(n: u32) -> u32 {
    n * ADVANCE as u32
}

/// Where each column starts across the well.
pub fn column_xs() -> [u32; 2] {
    [0, COLUMNS[0].width() + COLUMN_GAP]
}

/// How tall the panel's well is for `view`: its headings, and as many lines as its longest
/// column, and a pixel under the last.
pub fn content_height(view: &ControlsView) -> u32 {
    let lines = view.groups.iter().map(|g| g.lines.len()).max().unwrap_or(0) as u32;
    FIRST_LINE + PITCH * lines
}

/// Where line `i` of a column stands, below the well's top.
pub fn line_y(i: usize) -> u32 {
    FIRST_LINE + PITCH * i as u32
}

/// The CONTROLS panel as flow.html draws it, in `pal`.
pub fn controls_face(view: &ControlsView, pal: &Palette) -> UndoFace {
    let mut panel = panel_face(
        WIDTH,
        panel_height(content_height(view)),
        view.title,
        true,
        pal,
    );
    let [wx, wy, ..] = panel.well;
    for ((group, column), x) in view.groups.iter().zip(COLUMNS).zip(column_xs()) {
        over(&mut panel.face, &text_face(group.name, pal.dim), wx + x, wy);
        for (i, (keys, does)) in group.lines.iter().enumerate() {
            let y = wy + line_y(i);
            over(&mut panel.face, &text_face(keys, pal.frame_hi), wx + x, y);
            let face = text_face(does, pal.cream);
            over(&mut panel.face, &face, wx + x + column.does_x(), y);
        }
    }
    panel.face
}

impl App {
    pub(crate) fn controls_input(&mut self, action: Action) {
        if matches!(action, Action::GbaDown(Btn::B) | Action::SelectTap) {
            self.go(Phase::Shelf, Dir::Back);
        }
    }

    /// What the Controls screen says, while it is open: for the system of the shelf it was
    /// opened on.
    pub fn controls_view(&self) -> Option<ControlsView> {
        let platform = self.shelves[self.shelf_at].0;
        matches!(self.phase, Phase::Controls).then(|| ControlsView::new(platform))
    }

    pub fn set_controls_panel(&mut self, panel: Lone) {
        self.controls_panel = Some(panel);
    }

    pub(crate) fn draw_controls(&self, out: &mut Vec<Draw>) {
        if let Some(panel) = &self.controls_panel {
            panel.draw(out);
        }
    }
}
