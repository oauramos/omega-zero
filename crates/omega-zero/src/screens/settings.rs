//! Settings, opened with START on the shelf: a modal holding CONFIGS, whose values left and
//! right change in place, and GENERAL, whose rows open a screen of their own.

use omega_input::{Action, Btn};
use omega_store::FF_SPEEDS;
use omega_ui::{date_time_text, Draw, QuickValue, RowShown, SettingsFaces, SettingsMenu};

pub use omega_ui::{SettingsGroup, SettingsRow};

use crate::app::{App, Phase};
use crate::background_screen::BackgroundScreen;
use crate::bluetooth_screen::{BluetoothScreen, Summary as BluetoothSummary};
use crate::label_screen::LabelsScreen;
use crate::nav::Dir;
use crate::screens::clock::clock_screen;
use crate::wifi_screen::{Summary, WifiScreen};

impl App {
    /// The lit row, while Settings is open.
    pub fn settings_row(&self) -> Option<SettingsRow> {
        match self.phase {
            Phase::Settings { row } => Some(row),
            _ => None,
        }
    }

    /// What a config row is set to; a row that opens a screen shows none of these.
    pub fn settings_value(&self, row: SettingsRow) -> Option<QuickValue> {
        match row {
            SettingsRow::FastForward => QuickValue::speed(self.state.ff_speed),
            SettingsRow::FastForwardSound => Some(QuickValue::flag(self.state.ff_sound)),
            SettingsRow::ColourCorrection => Some(QuickValue::flag(self.state.colour_correction)),
            SettingsRow::LcdGhosting => Some(QuickValue::flag(self.state.lcd_ghosting)),
            SettingsRow::Rumble => Some(QuickValue::flag(self.state.rumble)),
            SettingsRow::Labels
            | SettingsRow::DateTime
            | SettingsRow::Network
            | SettingsRow::Bluetooth
            | SettingsRow::Background
            | SettingsRow::About => None,
        }
    }

    /// What a row says on its right: a config's value, the date and time, the network joined
    /// or the Bluetooth device connected (in capitals, as flow.html prints them), else ON, or
    /// OFF with the radio off.
    pub fn settings_text(&self, row: SettingsRow) -> Option<String> {
        match row {
            SettingsRow::DateTime => Some(date_time_text(self.wall_secs())),
            SettingsRow::Network => Some(match self.wifi_summary() {
                Summary::Off => QuickValue::Off.text().to_string(),
                Summary::On => QuickValue::On.text().to_string(),
                Summary::Joined(name) => name.to_uppercase(),
            }),
            SettingsRow::Bluetooth => Some(match self.bluetooth_summary() {
                BluetoothSummary::Off => QuickValue::Off.text().to_string(),
                BluetoothSummary::On => QuickValue::On.text().to_string(),
                BluetoothSummary::Connected(name) => name.to_uppercase(),
            }),
            _ => self.settings_value(row).map(|v| v.text().to_string()),
        }
    }

    /// Every row Settings shows, as it shows it now: what each says and which is lit. Its
    /// faces are baked from this.
    pub fn settings_shown(&self) -> Vec<RowShown> {
        let lit = self.settings_row();
        self.settings_rows()
            .into_iter()
            .map(|row| RowShown {
                row,
                value: self.settings_text(row),
                lit: lit == Some(row),
            })
            .collect()
    }

    pub fn set_settings_faces(&mut self, faces: SettingsFaces) {
        self.settings_faces = Some(faces);
    }

    pub fn settings_faces(&self) -> Option<&SettingsFaces> {
        self.settings_faces.as_ref()
    }

    pub(crate) fn open_settings(&mut self) {
        self.go(
            Phase::Settings {
                row: SettingsRow::ALL[0],
            },
            Dir::Aside,
        );
    }

    /// The rows Settings lists on this handheld: Network only where there is a Wi-Fi radio,
    /// Bluetooth only where there is Bluetooth.
    pub fn settings_rows(&self) -> Vec<SettingsRow> {
        SettingsRow::ALL
            .into_iter()
            .filter(|r| match r {
                SettingsRow::Network => self.wifi.present(),
                SettingsRow::Bluetooth => self.bluetooth.present(),
                _ => true,
            })
            .collect()
    }

    pub(crate) fn settings_input(&mut self, row: SettingsRow, action: Action) {
        let rows = self.settings_rows();
        let row = match action {
            Action::GbaDown(Btn::Up) => row.up_in(&rows),
            Action::GbaDown(Btn::Down) => row.down_in(&rows),
            Action::GbaDown(Btn::Left) => return self.change_setting(row, false),
            Action::GbaDown(Btn::Right) => return self.change_setting(row, true),
            Action::GbaDown(Btn::A) => return self.open_settings_row(row),
            Action::GbaDown(Btn::B | Btn::Start) | Action::QuickMenu => {
                return self.go(Phase::Shelf, Dir::FromAside);
            }
            _ => return,
        };
        self.phase = Phase::Settings { row };
    }

    fn open_settings_row(&mut self, row: SettingsRow) {
        let to = match row {
            SettingsRow::Labels => Phase::Labels(LabelsScreen::default()),
            SettingsRow::DateTime => clock_screen(self.utc_secs(), self.state.utc_offset_min, true),
            SettingsRow::Network => Phase::Network(WifiScreen::default()),
            SettingsRow::Bluetooth => Phase::Bluetooth(BluetoothScreen::default()),
            SettingsRow::Background => Phase::Background(BackgroundScreen::default()),
            SettingsRow::About => Phase::About,
            SettingsRow::FastForward
            | SettingsRow::FastForwardSound
            | SettingsRow::ColourCorrection
            | SettingsRow::LcdGhosting
            | SettingsRow::Rumble => return,
        };
        self.go(to, Dir::Deeper);
    }

    pub(crate) fn change_setting(&mut self, row: SettingsRow, right: bool) {
        let s = &mut self.state;
        match row {
            SettingsRow::FastForward => {
                let to = ff_next(s.ff_speed, right);
                if to == s.ff_speed {
                    return;
                }
                s.ff_speed = to;
            }
            SettingsRow::FastForwardSound => s.ff_sound = !s.ff_sound,
            SettingsRow::ColourCorrection => {
                s.colour_correction = !s.colour_correction;
                self.colour_pending = Some(s.colour_correction);
            }
            SettingsRow::LcdGhosting => {
                s.lcd_ghosting = !s.lcd_ghosting;
                self.ghosting_pending = Some(s.lcd_ghosting);
            }
            SettingsRow::Rumble => s.rumble = !s.rumble,
            SettingsRow::Labels
            | SettingsRow::DateTime
            | SettingsRow::Network
            | SettingsRow::Bluetooth
            | SettingsRow::Background
            | SettingsRow::About => return,
        }
        self.persist();
    }

    /// The modal with `row` lit, over the backdrop, which shows round it.
    pub(crate) fn draw_settings(&self, row: SettingsRow, out: &mut Vec<Draw>) {
        SettingsMenu {
            row,
            rows: &self.settings_rows(),
            faces: self.settings_faces.as_ref(),
        }
        .draw(out)
    }
}

fn ff_next(from: u8, right: bool) -> u8 {
    let at = FF_SPEEDS.iter().position(|&v| v == from).unwrap_or(0);
    let to = if right { at + 1 } else { at.saturating_sub(1) };
    FF_SPEEDS[to.min(FF_SPEEDS.len() - 1)]
}
