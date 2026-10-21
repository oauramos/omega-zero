//! The screen look each system's games are drawn through, kept per system.

use std::path::Path;

use omega_gfx::Look;
use omega_store::{ini, Platform};

pub const SHADER_FILE: &str = "Config/shader.ini";

/// The looks SELECT+X goes round for `platform`, none first: a handheld's LCD stripes, pixel
/// grid and round dots, or a television's scanlines and CRT.
pub fn looks_for(platform: Platform) -> &'static [Look] {
    match platform {
        Platform::Gba | Platform::Gb | Platform::Gbc => {
            &[Look::Off, Look::Lcd, Look::Grid, Look::Dot]
        }
        Platform::Snes | Platform::Ps1 => &[Look::Off, Look::Scanlines, Look::Crt],
    }
}

/// The look after `look` for `platform`, round to none after the last.
pub fn next_look(platform: Platform, look: Look) -> Look {
    let looks = looks_for(platform);
    let at = looks.iter().position(|l| *l == look).unwrap_or(0);
    looks[(at + 1) % looks.len()]
}

/// The look every game of `platform` is drawn through: the one its system was last left in,
/// kept under the system's folder name, else none. A look the system does not offer is none.
pub fn shader_for(root: &Path, platform: Platform) -> Look {
    ini::value(root, SHADER_FILE, platform.dir_name())
        .as_deref()
        .and_then(Look::parse)
        .filter(|l| looks_for(platform).contains(l))
        .unwrap_or_default()
}

pub fn write_shader(root: &Path, platform: Platform, look: Look) -> std::io::Result<()> {
    ini::write(root, SHADER_FILE, platform.dir_name(), look.as_str())
}
