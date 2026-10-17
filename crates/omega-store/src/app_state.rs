use std::path::{Path, PathBuf};

use crate::atomic::atomic_write;
use crate::backdrop::{colour_index, BackdropStyle, BACKDROP_COLOURS};
use crate::font::Font;
use crate::platform::Platform;

pub const BRIGHTNESS_MAX: u8 = 9;
pub const BLUE_LIGHT_MAX: u8 = 9;
pub const VOLUME_MAX: u8 = 100;

pub const UTC_OFFSET_MIN: i16 = -720;
pub const UTC_OFFSET_MAX: i16 = 840;

pub const FF_SPEEDS: [u8; 4] = [2, 3, 4, 6];

pub const FF_SPEED_DEFAULT: u8 = 6;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AppState {
    pub cart: Option<String>,
    pub cart_platform: Option<Platform>,
    pub brightness: u8,
    pub blue_light: u8,
    pub volume: u8,
    pub volume_hp: u8,
    pub muted: bool,
    pub muted_hp: bool,
    pub clock_set: bool,
    pub utc_offset_min: i16,
    pub rumble: bool,
    pub ff_speed: u8,
    pub ff_sound: bool,
    pub colour_correction: bool,
    /// The handhelds' cores blend each frame with the last, as the LCD the games were drawn
    /// for blurred them: flicker drawn as transparency shows as it did.
    pub lcd_ghosting: bool,
    /// The panel runs at 120 Hz where it can. On by default; written off when the panel
    /// refused it or lost it, and a card can set it off by hand to keep the panel at 60 Hz.
    pub high_refresh: bool,
    /// Home Wi-Fi is switched on.
    pub wifi: bool,
    /// SSH is switched on: BaseOS's dev images start its server at boot, and it is left
    /// running unless this is off.
    pub ssh: bool,
    /// Bluetooth is switched on: omega-zero brings the radio up at boot, which BaseOS leaves
    /// blocked.
    pub bluetooth: bool,
    /// The shelf's background, once one was picked. Until then a card with a wallpaper shows
    /// it, and any other the default pattern.
    pub backdrop: Option<BackdropStyle>,
    /// The pattern drifts.
    pub backdrop_moving: bool,
    /// Which of [`BACKDROP_COLOURS`] the pattern is in.
    pub backdrop_colour: u8,
    /// The font the bar's actions write their big words in.
    pub font: Font,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            cart: None,
            cart_platform: None,
            brightness: 5,
            blue_light: 0,
            volume: 60,
            volume_hp: 60,
            muted: false,
            muted_hp: false,
            clock_set: false,
            utc_offset_min: 0,
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
    }
}

fn state_path(root: &Path) -> PathBuf {
    root.join("Config").join("omega-zero.state")
}

pub fn read_app_state(root: &Path) -> AppState {
    std::fs::read(state_path(root))
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .and_then(|s| parse(&s))
        .unwrap_or_default()
}

pub fn write_app_state(root: &Path, s: &AppState) -> std::io::Result<()> {
    let text = format!(
        "cart={}\ncart_platform={}\nbrightness={}\nblue_light={}\nvolume={}\nvolume_hp={}\nmuted={}\nmuted_hp={}\nclock_set={}\nutc_offset_min={}\nrumble={}\nff_speed={}\nff_sound={}\ncolour_correction={}\nlcd_ghosting={}\nhigh_refresh={}\nwifi={}\nssh={}\nbluetooth={}\nbackdrop={}\nbackdrop_moving={}\nbackdrop_colour={}\nfont={}\n",
        s.cart.as_deref().unwrap_or(""),
        s.cart_platform.map_or(String::new(), platform_key),
        s.brightness,
        s.blue_light,
        s.volume,
        s.volume_hp,
        s.muted as u8,
        s.muted_hp as u8,
        s.clock_set as u8,
        s.utc_offset_min,
        s.rumble as u8,
        s.ff_speed,
        s.ff_sound as u8,
        s.colour_correction as u8,
        s.lcd_ghosting as u8,
        s.high_refresh as u8,
        s.wifi as u8,
        if s.ssh { "on" } else { "off" },
        if s.bluetooth { "on" } else { "off" },
        s.backdrop.map_or("", BackdropStyle::key),
        s.backdrop_moving as u8,
        BACKDROP_COLOURS[(s.backdrop_colour as usize).min(BACKDROP_COLOURS.len() - 1)],
        s.font.key()
    );
    atomic_write(&state_path(root), text.as_bytes())
}

fn parse(text: &str) -> Option<AppState> {
    let mut cart = None;
    let mut cart_platform = None;
    let mut brightness = None;
    let mut blue_light = None;
    let mut volume = None;
    let mut volume_hp = None;
    let mut muted = None;
    let mut muted_hp = None;
    let mut clock_set = None;
    let mut utc_offset_min = None;
    let mut rumble = None;
    let mut ff_speed = None;
    let mut ff_sound = None;
    let mut colour_correction = None;
    let mut lcd_ghosting = None;
    let mut high_refresh = None;
    let mut wifi = None;
    let mut ssh = None;
    let mut bluetooth = None;
    let mut backdrop = None;
    let mut backdrop_moving = None;
    let mut backdrop_colour = None;
    let mut font = None;
    for line in text.lines().filter(|l| !l.is_empty()) {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "cart" => cart = Some(value.to_string()),
            "cart_platform" => cart_platform = platform_value(value),
            "brightness" => brightness = Some(level(value, BRIGHTNESS_MAX)?),
            "blue_light" => blue_light = Some(level(value, BLUE_LIGHT_MAX)?),
            "volume" => volume = Some(level(value, VOLUME_MAX)?),
            "volume_hp" => volume_hp = Some(level(value, VOLUME_MAX)?),
            "muted" => muted = Some(level(value, 1)? == 1),
            "muted_hp" => muted_hp = Some(level(value, 1)? == 1),
            "clock_set" => clock_set = Some(level(value, 1)? == 1),
            "utc_offset_min" => utc_offset_min = Some(offset(value)?),
            "rumble" => rumble = flag(value),
            "ff_speed" => ff_speed = ff_speed_value(value),
            "ff_sound" => ff_sound = flag(value),
            "colour_correction" => colour_correction = flag(value),
            "lcd_ghosting" => lcd_ghosting = flag(value),
            // anything but a clear 1 keeps the panel at 60 Hz, the rate it has always run at
            "high_refresh" => high_refresh = Some(flag(value).unwrap_or(false)),
            "wifi" => wifi = flag(value),
            "ssh" => ssh = on_off(value),
            "bluetooth" => bluetooth = on_off(value),
            "backdrop" => backdrop = BackdropStyle::from_key(value),
            "backdrop_moving" => backdrop_moving = flag(value),
            "backdrop_colour" => backdrop_colour = colour_index(value),
            "font" => font = Font::from_key(value),
            _ => {}
        }
    }
    let cart = cart?;
    let fallback = AppState::default();
    Some(AppState {
        cart: (!cart.is_empty()).then_some(cart),
        cart_platform,
        brightness: brightness?,
        blue_light: blue_light?,
        volume: volume?,
        volume_hp: volume_hp.or(volume)?,
        muted: muted?,
        muted_hp: muted_hp.or(muted)?,
        clock_set: clock_set?,
        utc_offset_min: utc_offset_min?,
        rumble: rumble.unwrap_or(fallback.rumble),
        ff_speed: ff_speed.unwrap_or(fallback.ff_speed),
        ff_sound: ff_sound.unwrap_or(fallback.ff_sound),
        colour_correction: colour_correction.unwrap_or(fallback.colour_correction),
        lcd_ghosting: lcd_ghosting.unwrap_or(fallback.lcd_ghosting),
        high_refresh: high_refresh.unwrap_or(fallback.high_refresh),
        wifi: wifi.unwrap_or(fallback.wifi),
        ssh: ssh.unwrap_or(fallback.ssh),
        bluetooth: bluetooth.unwrap_or(fallback.bluetooth),
        backdrop,
        backdrop_moving: backdrop_moving.unwrap_or(fallback.backdrop_moving),
        backdrop_colour: backdrop_colour.unwrap_or(fallback.backdrop_colour),
        font: font.unwrap_or(fallback.font),
    })
}

fn platform_key(platform: Platform) -> String {
    platform.dir_name().to_ascii_lowercase()
}

fn platform_value(value: &str) -> Option<Platform> {
    Platform::ALL
        .into_iter()
        .find(|p| value.eq_ignore_ascii_case(p.dir_name()))
}

fn offset(value: &str) -> Option<i16> {
    value
        .parse()
        .ok()
        .filter(|n| (UTC_OFFSET_MIN..=UTC_OFFSET_MAX).contains(n))
}

fn ff_speed_value(value: &str) -> Option<u8> {
    value.parse().ok().filter(|n| FF_SPEEDS.contains(n))
}

fn level(value: &str, max: u8) -> Option<u8> {
    value.parse().ok().filter(|n| *n <= max)
}

fn flag(value: &str) -> Option<bool> {
    level(value, 1).map(|n| n == 1)
}

/// A setting written as a word, `on` or `off`.
fn on_off(value: &str) -> Option<bool> {
    match value {
        "on" => Some(true),
        "off" => Some(false),
        _ => None,
    }
}
