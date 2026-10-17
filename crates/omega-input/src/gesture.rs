use crate::{Btn, Millis, RawEvent};

pub const SELECT_CHORD_MS: Millis = 600;

pub const SELECT_TAP_MS: Millis = 50;
pub const MENU_TAP_MS: Millis = 250;
pub const MENU_DOUBLE_TAP_MS: Millis = 350;
pub const MENU_HOLD_MS: Millis = 1000;
pub const FF_DOUBLE_TAP_MS: Millis = 250;
pub const MUTE_CHORD_MS: Millis = 200;
pub const POWER_HOLD_MS: Millis = 1000;

pub const VOLUME_REPEAT_DELAY_MS: Millis = 400;
pub const VOLUME_REPEAT_MS: Millis = 120;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Action {
    GbaDown(Btn),
    GbaUp(Btn),
    ShelfLeft,
    ShelfRight,
    Insert,
    Eject,
    Polaroids,
    SaveState,
    LoadState,
    RewindStart,
    RewindStop,
    FfStart,
    FfStop,
    BrightnessUp,
    BrightnessDown,
    BlueLightUp,
    BlueLightDown,
    VolumeUp,
    VolumeDown,
    QuickMenu,
    /// SELECT pressed and let go on its own, before it could become a chord. It comes after
    /// the `GbaDown(Select)` the tap also hands the game, and is never sent for a SELECT held
    /// past the chord window.
    SelectTap,
    GameMenu,
    MuteToggle,
    ColourCorrectionToggle,
    LcdGhostingToggle,
    PowerPress,
    PowerTap,
    PowerHold,
    PowerOff,
    LidClose,
    LidOpen,
}

/// What the buttons the systems disagree about do for the game being played. The handhelds'
/// layout is the default: L2 rewinds, R2 fast-forwards (pressed twice quickly, it keeps going),
/// SELECT+Y toggles the colour correction and SELECT+X the LCD's ghosting. A game with L2 and R2 of its own, as the
/// PlayStation's have, is handed them, and fast forward moves to SELECT+R2: the same hold, and
/// the same double press, with SELECT held.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Keys {
    /// L2 and R2 are the game's, and fast forward is SELECT+R2.
    pub game_l2_r2: bool,
    /// Holding L2 rewinds, or SELECT+L2 where L2 is the game's. Off, nothing rewinds.
    pub rewind: bool,
    /// SELECT+Y toggles the colour correction. Off, a Y under SELECT is the game's.
    pub colour: bool,
    /// SELECT+X toggles the LCD's ghosting. Off, an X under SELECT is the game's.
    pub ghosting: bool,
}

impl Keys {
    /// The handhelds': L2 rewinds, R2 fast-forwards, SELECT+Y is the colour correction and
    /// SELECT+X the LCD's ghosting.
    pub const HANDHELD: Keys = Keys {
        game_l2_r2: false,
        rewind: true,
        colour: true,
        ghosting: true,
    };
}

impl Default for Keys {
    fn default() -> Self {
        Keys::HANDHELD
    }
}

#[derive(Copy, Clone, Default)]
enum Select {
    #[default]
    Idle,
    Held {
        since: Millis,
        chorded: bool,
        sent: Option<Millis>,
    },
    ReleaseDue(Millis),
}

#[derive(Default)]
pub struct Gestures {
    keys: Keys,
    select: Select,
    chord_held: u16,
    menu_down_at: Option<Millis>,
    menu_last_tap: Option<Millis>,
    menu_eject_fired: bool,
    power_down_at: Option<Millis>,
    power_hold_fired: bool,
    vol_up_at: Option<Millis>,
    vol_down_at: Option<Millis>,
    vol_up_ramp: Option<Millis>,
    vol_down_ramp: Option<Millis>,
    mute_fired: bool,
    ff_on: bool,
    ff_latched: bool,
    ff_latching_press: bool,
    r2_refused: bool,
    r2_last_release: Option<Millis>,
    rewinding: bool,
}

fn ramp_due(down: Option<Millis>, last: Option<Millis>, now: Millis) -> bool {
    let Some(down) = down else {
        return false;
    };
    if now.saturating_sub(down) < VOLUME_REPEAT_DELAY_MS {
        return false;
    }
    last.is_none_or(|l| now.saturating_sub(l) >= VOLUME_REPEAT_MS)
}

impl Gestures {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ff_latched(&self) -> bool {
        self.ff_latched
    }

    /// What the buttons do from now on, for the game being played. A change lets go of a rewind
    /// or a fast forward under way, and of a shoulder's chord still held, so nothing is left
    /// running under buttons that now mean something else.
    pub fn set_keys(&mut self, keys: Keys) -> Vec<Action> {
        if self.keys == keys {
            return Vec::new();
        }
        self.keys = keys;
        let mut out = self.rewind_stop();
        out.extend(self.ff_clear());
        self.r2_refused = false;
        self.r2_last_release = None;
        self.chord_held &= !(L2_CHORD | R2_CHORD);
        out
    }

    pub fn keys(&self) -> Keys {
        self.keys
    }

    pub fn drop_ff_latch(&mut self) -> Vec<Action> {
        if !self.ff_latched {
            return Vec::new();
        }
        self.ff_clear()
    }

    pub fn feed(&mut self, ev: RawEvent, now: Millis) -> Vec<Action> {
        match ev {
            RawEvent::Down(b) => self.down(b, now),
            RawEvent::Up(b) => self.up(b, now),
        }
    }

    pub fn tick(&mut self, now: Millis) -> Vec<Action> {
        let mut out = Vec::new();
        if let Select::ReleaseDue(due) = self.select {
            if now >= due {
                self.select = Select::Idle;
                out.push(Action::GbaUp(Btn::Select));
            }
        }
        if let Select::Held {
            since,
            chorded: false,
            sent: None,
        } = self.select
        {
            if now.saturating_sub(since) >= SELECT_CHORD_MS {
                out.extend(self.hand_over_select(now));
            }
        }
        if let Some(d) = self.menu_down_at {
            if !self.menu_eject_fired && now.saturating_sub(d) >= MENU_HOLD_MS {
                self.menu_eject_fired = true;
                out.push(Action::Eject);
            }
        }
        if let Some(d) = self.power_down_at {
            if !self.power_hold_fired && now.saturating_sub(d) >= POWER_HOLD_MS {
                self.power_hold_fired = true;
                out.push(Action::PowerHold);
            }
        }
        if !self.mute_fired {
            if ramp_due(self.vol_up_at, self.vol_up_ramp, now) {
                self.vol_up_ramp = Some(now);
                out.push(Action::VolumeUp);
            }
            if ramp_due(self.vol_down_at, self.vol_down_ramp, now) {
                self.vol_down_ramp = Some(now);
                out.push(Action::VolumeDown);
            }
        }
        out
    }

    fn down(&mut self, b: Btn, now: Millis) -> Vec<Action> {
        match b {
            Btn::Select => self.select_down(now),
            Btn::Menu => self.menu_down(now),
            Btn::Power => {
                self.power_down_at = Some(now);
                self.power_hold_fired = false;
                vec![Action::PowerPress]
            }
            Btn::Lid => vec![Action::LidClose],
            Btn::VolUp | Btn::VolDown => self.volume_press(b, now),
            Btn::L2 if !self.keys.game_l2_r2 => match self.keys.rewind {
                true => self.rewind_start(),
                false => Vec::new(),
            },
            Btn::R2 if !self.keys.game_l2_r2 => self.ff_down(now),
            _ => {
                if let (true, Some((bit, action))) = (self.chording(now), self.chord(b)) {
                    self.mark_chorded();
                    self.chord_held |= bit;
                    return match action {
                        Action::RewindStart => self.rewind_start(),
                        Action::FfStart => self.ff_down(now),
                        action => vec![action],
                    };
                }
                let mut out = self.hand_over_select(now);
                out.push(Action::GbaDown(b));
                out
            }
        }
    }

    fn up(&mut self, b: Btn, now: Millis) -> Vec<Action> {
        match b {
            Btn::Select => self.select_up(now),
            Btn::Menu => self.menu_up(now),
            Btn::Power => self.power_up(),
            Btn::Lid => vec![Action::LidOpen],
            Btn::VolUp | Btn::VolDown => self.volume_release(b),
            Btn::L2 if !self.keys.game_l2_r2 => self.rewind_stop(),
            Btn::R2 if !self.keys.game_l2_r2 => self.ff_up(now),
            _ => {
                if let Some((bit, action)) = self.chord(b) {
                    if self.chord_held & bit != 0 {
                        self.chord_held &= !bit;
                        return match action {
                            Action::RewindStart => self.rewind_stop(),
                            Action::FfStart => self.ff_up(now),
                            _ => Vec::new(),
                        };
                    }
                }
                vec![Action::GbaUp(b)]
            }
        }
    }

    /// What `b` does under SELECT, and the bit its chord holds while it is down: most are a
    /// press, and L2 and R2, where they are the game's, a hold that rewinds or fast-forwards
    /// until the shoulder is let go.
    fn chord(&self, b: Btn) -> Option<(u16, Action)> {
        Some(match b {
            Btn::Up => (1, Action::BrightnessUp),
            Btn::Down => (2, Action::BrightnessDown),
            Btn::Left => (4, Action::BlueLightDown),
            Btn::Right => (8, Action::BlueLightUp),
            Btn::L1 => (16, Action::LoadState),
            Btn::R1 => (32, Action::SaveState),
            Btn::Y if self.keys.colour => (64, Action::ColourCorrectionToggle),
            Btn::X if self.keys.ghosting => (X_CHORD, Action::LcdGhostingToggle),
            Btn::L2 if self.keys.game_l2_r2 && self.keys.rewind => (L2_CHORD, Action::RewindStart),
            Btn::R2 if self.keys.game_l2_r2 => (R2_CHORD, Action::FfStart),
            _ => return None,
        })
    }

    fn chording(&self, now: Millis) -> bool {
        match self.select {
            Select::Held { since, chorded, .. } => {
                chorded || now.saturating_sub(since) < SELECT_CHORD_MS
            }
            _ => false,
        }
    }

    fn mark_chorded(&mut self) {
        if let Select::Held { chorded, .. } = &mut self.select {
            *chorded = true;
        }
    }

    fn select_down(&mut self, now: Millis) -> Vec<Action> {
        let mut out = Vec::new();
        if matches!(self.select, Select::ReleaseDue(_)) {
            out.push(Action::GbaUp(Btn::Select));
        }
        if !matches!(self.select, Select::Held { .. }) {
            self.select = Select::Held {
                since: now,
                chorded: false,
                sent: None,
            };
        }
        out
    }

    fn hand_over_select(&mut self, now: Millis) -> Vec<Action> {
        match &mut self.select {
            Select::Held {
                chorded: false,
                sent: sent @ None,
                ..
            } => {
                *sent = Some(now);
                vec![Action::GbaDown(Btn::Select)]
            }
            _ => Vec::new(),
        }
    }

    fn select_up(&mut self, now: Millis) -> Vec<Action> {
        let Select::Held { chorded, sent, .. } = std::mem::take(&mut self.select) else {
            return Vec::new();
        };
        match (sent, chorded) {
            (Some(at), _) if now.saturating_sub(at) >= SELECT_TAP_MS => {
                vec![Action::GbaUp(Btn::Select)]
            }
            (Some(at), _) => {
                self.select = Select::ReleaseDue(at + SELECT_TAP_MS);
                Vec::new()
            }
            (None, true) => Vec::new(),
            (None, false) => {
                self.select = Select::ReleaseDue(now + SELECT_TAP_MS);
                vec![Action::GbaDown(Btn::Select), Action::SelectTap]
            }
        }
    }

    fn menu_down(&mut self, now: Millis) -> Vec<Action> {
        if self.chording(now) {
            self.mark_chorded();
            self.menu_down_at = None;
            self.menu_last_tap = None;
            return vec![Action::GameMenu];
        }
        if let Some(tap) = self.menu_last_tap {
            if now.saturating_sub(tap) <= MENU_DOUBLE_TAP_MS {
                self.menu_last_tap = None;
                self.menu_down_at = None;
                return vec![Action::Polaroids];
            }
        }
        self.menu_down_at = Some(now);
        self.menu_eject_fired = false;
        Vec::new()
    }

    fn menu_up(&mut self, now: Millis) -> Vec<Action> {
        let Some(d) = self.menu_down_at.take() else {
            return Vec::new();
        };
        let ejected = self.menu_eject_fired;
        self.menu_eject_fired = false;
        let tapped = !ejected && now.saturating_sub(d) < MENU_TAP_MS;
        self.menu_last_tap = tapped.then_some(now);
        match tapped {
            true => vec![Action::QuickMenu],
            false => Vec::new(),
        }
    }

    fn power_up(&mut self) -> Vec<Action> {
        let held = self.power_hold_fired;
        self.power_down_at = None;
        self.power_hold_fired = false;
        vec![if held {
            Action::PowerOff
        } else {
            Action::PowerTap
        }]
    }

    fn volume_press(&mut self, b: Btn, now: Millis) -> Vec<Action> {
        let (mine, other, action) = match b {
            Btn::VolUp => (&mut self.vol_up_at, self.vol_down_at, Action::VolumeUp),
            _ => (&mut self.vol_down_at, self.vol_up_at, Action::VolumeDown),
        };
        *mine = Some(now);
        match b {
            Btn::VolUp => self.vol_up_ramp = None,
            _ => self.vol_down_ramp = None,
        }
        let mut out = vec![action];
        let paired = other.is_some_and(|t| now.abs_diff(t) <= MUTE_CHORD_MS);
        if paired && !self.mute_fired {
            self.mute_fired = true;
            out.push(Action::MuteToggle);
        }
        out
    }

    fn volume_release(&mut self, b: Btn) -> Vec<Action> {
        match b {
            Btn::VolUp => (self.vol_up_at, self.vol_up_ramp) = (None, None),
            _ => (self.vol_down_at, self.vol_down_ramp) = (None, None),
        }
        if self.vol_up_at.is_none() && self.vol_down_at.is_none() {
            self.mute_fired = false;
        }
        Vec::new()
    }

    fn rewind_start(&mut self) -> Vec<Action> {
        if self.rewinding {
            return Vec::new();
        }
        let mut out = Vec::new();
        out.extend(self.ff_clear());
        self.rewinding = true;
        out.push(Action::RewindStart);
        out
    }

    fn rewind_stop(&mut self) -> Vec<Action> {
        if !self.rewinding {
            return Vec::new();
        }
        self.rewinding = false;
        vec![Action::RewindStop]
    }

    fn ff_down(&mut self, now: Millis) -> Vec<Action> {
        if self.rewinding {
            self.r2_refused = true;
            return Vec::new();
        }
        if self.ff_latched {
            self.ff_latching_press = false;
        } else {
            let double = self
                .r2_last_release
                .is_some_and(|rel| now.saturating_sub(rel) <= FF_DOUBLE_TAP_MS);
            self.ff_latched = double;
            self.ff_latching_press = double;
        }
        if self.ff_on {
            return Vec::new();
        }
        self.ff_on = true;
        vec![Action::FfStart]
    }

    fn ff_up(&mut self, now: Millis) -> Vec<Action> {
        if std::mem::take(&mut self.r2_refused) {
            return Vec::new();
        }
        self.r2_last_release = Some(now);
        if self.ff_latching_press {
            self.ff_latching_press = false;
            return Vec::new();
        }
        self.ff_clear()
    }

    fn ff_clear(&mut self) -> Vec<Action> {
        self.ff_latched = false;
        self.ff_latching_press = false;
        if !self.ff_on {
            return Vec::new();
        }
        self.ff_on = false;
        vec![Action::FfStop]
    }
}

/// The chord bits of SELECT+L2 and SELECT+R2, held for as long as the shoulder is.
const L2_CHORD: u16 = 128;
const R2_CHORD: u16 = 256;
/// The chord bit of SELECT+X.
const X_CHORD: u16 = 512;
