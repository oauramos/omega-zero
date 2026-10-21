use std::collections::HashMap;
use std::path::Path;

use crate::Platform;

pub const SELECTED_CORE_FILE: &str = "Config/selected_core.ini";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Core {
    #[default]
    Mgba,
    Gpsp,
    Snes9x,
    Supafaust,
    PcsxRearmed,
    PceFast,
    Fceumm,
    GenesisPlusGx,
}

impl Core {
    pub const ALL: [Core; 8] = [
        Core::Mgba,
        Core::Gpsp,
        Core::Snes9x,
        Core::Supafaust,
        Core::PcsxRearmed,
        Core::PceFast,
        Core::Fceumm,
        Core::GenesisPlusGx,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Core::Mgba => "mgba",
            Core::Gpsp => "gpsp",
            Core::Snes9x => "snes9x",
            Core::Supafaust => "supafaust",
            Core::PcsxRearmed => "pcsx_rearmed",
            Core::PceFast => "pce_fast",
            Core::Fceumm => "fceumm",
            Core::GenesisPlusGx => "genesis_plus_gx",
        }
    }

    /// The libretro name its library is built under, `<name>_libretro.so`.
    pub fn library(&self) -> &'static str {
        match self {
            Core::Supafaust => "mednafen_supafaust",
            Core::PceFast => "mednafen_pce_fast",
            other => other.as_str(),
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    /// The core's name as its chip in the core picker wears it, in its own case: as long as the
    /// chip has room for, six letters at most, so Supafaust is Faust, FCEUmm is FCEU and Genesis Plus GX
    /// is GPGX.
    pub fn text(self) -> &'static str {
        match self {
            Core::Mgba => "mGBA",
            Core::Gpsp => "gpSP",
            Core::Snes9x => "Snes9x",
            Core::Supafaust => "Faust",
            Core::PcsxRearmed => "PCSX",
            Core::PceFast => "PCE",
            Core::Fceumm => "FCEU",
            Core::GenesisPlusGx => "GPGX",
        }
    }

    /// The core's whole name, in its own case, as About credits it.
    pub fn name(self) -> &'static str {
        match self {
            Core::Mgba => "mGBA",
            Core::Gpsp => "gpSP",
            Core::Snes9x => "Snes9x",
            Core::Supafaust => "Supafaust",
            Core::PcsxRearmed => "PCSX-ReARMed",
            Core::PceFast => "Beetle PCE Fast",
            Core::Fceumm => "FCEUmm",
            Core::GenesisPlusGx => "Genesis Plus GX",
        }
    }

    pub fn parse(s: &str) -> Option<Core> {
        match s.trim().to_ascii_lowercase().as_str() {
            "mgba" => Some(Core::Mgba),
            "gpsp" => Some(Core::Gpsp),
            "snes9x" => Some(Core::Snes9x),
            "supafaust" => Some(Core::Supafaust),
            "pcsx_rearmed" => Some(Core::PcsxRearmed),
            "pce_fast" => Some(Core::PceFast),
            "fceumm" => Some(Core::Fceumm),
            "genesis_plus_gx" => Some(Core::GenesisPlusGx),
            _ => None,
        }
    }

    pub fn runs(self, platform: Platform) -> bool {
        Core::for_platform(platform).contains(&self)
    }

    /// The cores that run `platform`, the default first, in the order the core picker's
    /// sockets show them.
    pub fn for_platform(platform: Platform) -> &'static [Core] {
        match platform {
            Platform::Gba => &[Core::Mgba, Core::Gpsp],
            Platform::Gb | Platform::Gbc => &[Core::Mgba],
            Platform::Snes => &[Core::Snes9x, Core::Supafaust],
            Platform::Ps1 => &[Core::PcsxRearmed],
            Platform::Pce => &[Core::PceFast],
            Platform::Nes => &[Core::Fceumm],
            Platform::Md => &[Core::GenesisPlusGx],
        }
    }

    pub fn default_for(platform: Platform) -> Core {
        Core::for_platform(platform)[0]
    }
}

pub fn read_selected_cores(root: &Path) -> HashMap<String, Core> {
    crate::ini::read(root, SELECTED_CORE_FILE)
        .into_iter()
        .filter_map(|(stem, name)| Core::parse(&name).map(|core| (stem, core)))
        .collect()
}

pub fn core_for(root: &Path, stem: &str) -> Core {
    crate::ini::value(root, SELECTED_CORE_FILE, stem)
        .as_deref()
        .and_then(Core::parse)
        .unwrap_or_default()
}

pub fn core_for_platform(root: &Path, stem: &str, platform: Platform) -> Core {
    crate::ini::value(root, SELECTED_CORE_FILE, stem)
        .as_deref()
        .and_then(Core::parse)
        .filter(|core| core.runs(platform))
        .unwrap_or_else(|| Core::default_for(platform))
}

pub fn write_selected_core(root: &Path, stem: &str, core: Core) -> std::io::Result<()> {
    crate::ini::write(root, SELECTED_CORE_FILE, stem, core.as_str())
}
