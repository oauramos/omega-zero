mod app_state;
mod atomic;
mod backdrop;
pub mod cart_shell;
mod config;
mod core;
mod font;
pub mod gb;
mod gba;
pub mod ini;
pub mod md;
mod platform;
pub mod ps1;
mod region;
mod ring;
mod scan;
pub mod snes;
mod stamp;
mod theme;

pub use app_state::{
    read_app_state, write_app_state, AppState, BLUE_LIGHT_MAX, BRIGHTNESS_MAX, FF_SPEEDS,
    FF_SPEED_DEFAULT, UTC_OFFSET_MAX, UTC_OFFSET_MIN, VOLUME_MAX,
};
pub use atomic::{atomic_write, atomic_write_dated};
pub use backdrop::{colour_index, BackdropStyle, BACKDROP_COLOURS};
pub use cart_shell::{Outline, ShellChoice, ShellFinish, CART_SHELL_FILE, LABELS_SHELL_FILE};
pub use config::{move_config, CONFIG_DIR};
pub use core::{
    core_for, core_for_platform, read_selected_cores, write_selected_core, Core, SELECTED_CORE_FILE,
};
pub use font::Font;
pub use gba::{header_clean, header_code, header_title};
pub use platform::Platform;
pub use region::Region;
pub use ring::{StateEntry, StateRing, RING_MAX};
pub use scan::{initial, is_hidden, scan, sort_key, Cart, StoreError};
pub use stamp::{
    civil_from_days, days_from_civil, days_in_month, format_stamp, parse_stamp, stamp_now,
};
pub use theme::{Theme, THEME_FILE};
