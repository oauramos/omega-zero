use std::path::{Path, PathBuf};

pub const DIRS: [&str; 49] = [
    "BIOS",
    "Config",
    "Games",
    "Games/GBA",
    "Games/GB",
    "Games/GBC",
    "Games/SNES",
    "Games/PS1",
    "Games/PCE",
    "Games/NES",
    "Games/MD",
    "Labels",
    "Labels/GBA",
    "Labels/GB",
    "Labels/GBC",
    "Labels/SNES",
    "Labels/PS1",
    "Labels/PCE",
    "Labels/NES",
    "Labels/MD",
    "Overlays",
    "Overlays/GBA",
    "Overlays/GB",
    "Overlays/GBC",
    "Overlays/SNES",
    "Overlays/PS1",
    "Overlays/PCE",
    "Overlays/NES",
    "Overlays/MD",
    "Saves",
    "Saves/GBA",
    "Saves/GB",
    "Saves/GBC",
    "Saves/SNES",
    "Saves/PS1",
    "Saves/PCE",
    "Saves/NES",
    "Saves/MD",
    "States",
    "States/GBA",
    "States/GB",
    "States/GBC",
    "States/SNES",
    "States/PS1",
    "States/PCE",
    "States/NES",
    "States/MD",
    "System",
    "Wallpapers",
];

pub fn ensure(root: &Path) {
    for sub in DIRS {
        let _ = std::fs::create_dir_all(root.join(sub));
    }
    if let Err(e) = omega_store::move_config(root) {
        eprintln!("omega-zero: config: {e}");
    }
}

pub fn bios_dir(root: &Path) -> PathBuf {
    root.join("BIOS")
}

const BIOS_FILE: &str = "gba_bios.bin";

const BIOS_BYTES: u64 = 16 * 1024;
const BIOS_FIRST_BYTE: u8 = 0x18;

pub fn has_real_bios(root: &Path) -> bool {
    let Ok(mut f) = std::fs::File::open(bios_dir(root).join(BIOS_FILE)) else {
        return false;
    };
    if !f.metadata().is_ok_and(|m| m.len() == BIOS_BYTES) {
        return false;
    }
    let mut first = [0u8; 1];
    std::io::Read::read_exact(&mut f, &mut first).is_ok() && first[0] == BIOS_FIRST_BYTE
}

pub fn saves_dir(root: &Path) -> PathBuf {
    root.join("Saves")
}
