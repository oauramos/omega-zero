use std::path::Path;

use omega_store::gb::Class;
use omega_store::{Cart, Platform, ShellFinish};

use crate::models::md::MdShell;
use crate::models::nes::NesShell;
use crate::models::snes::SnesShell;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Finish {
    Solid,
    Translucent,
    Glitter,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Shell {
    pub colour: [u8; 3],
    pub finish: Finish,
}

pub const DEFAULT_SHELL: Shell = Shell {
    colour: [0x35, 0x35, 0x3a],
    finish: Finish::Solid,
};

const fn shell(colour: [u8; 3], finish: Finish) -> Shell {
    Shell { colour, finish }
}

const EXACT: &[(&str, Shell)] = &[
    ("AXV", shell([0xc2, 0x33, 0x2e], Finish::Translucent)),
    ("AXP", shell([0x2f, 0x5c, 0xc0], Finish::Translucent)),
    ("BPE", shell([0x24, 0x9c, 0x60], Finish::Translucent)),
    ("BPR", shell([0xd8, 0x52, 0x24], Finish::Solid)),
    ("BPG", shell([0x63, 0xb0, 0x44], Finish::Solid)),
    ("U3I", CLEAR),
    ("U32", CLEAR),
    ("U33", CLEAR),
    ("V49", shell([0xa9, 0x51, 0x3b], Finish::Solid)),
    ("KYGE", YOSHI),
    ("KYGP", YOSHI),
    ("RZW", shell([0x5f, 0x62, 0x64], Finish::Translucent)),
    ("RZWJ", shell([0xec, 0xee, 0xe8], Finish::Solid)),
];

const CLEAR: Shell = shell([0xd9, 0xdb, 0xd8], Finish::Translucent);
const YOSHI: Shell = shell([0x2f, 0x8f, 0x4e], Finish::Solid);

const FAMILY: &[(u8, Shell)] = &[(b'M', shell([0xc6, 0xc6, 0xc9], Finish::Solid))];

pub const DMG_SHELL: Shell = shell([0x9a, 0x97, 0x8f], Finish::Solid);

pub const DUAL_MODE_SHELL: Shell = shell([0x33, 0x30, 0x31], Finish::Solid);

pub const GB_CLEAR_SHELL: Shell = shell([0x7c, 0x7a, 0x8a], Finish::Translucent);

/// The American SNES cart's grey, a shade darker and cooler than the Super Famicom's.
pub const SNES_SHELL: Shell = shell([0x8b, 0x8a, 0x91], Finish::Solid);

/// The Super Famicom cart's light grey, which Europe's carts share.
/// The Mega Drive cart of Japan and Europe, and the Genesis cart of America, both black.
pub const MEGA_DRIVE_SHELL: Shell = shell([0x22, 0x22, 0x25], Finish::Solid);
pub const GENESIS_SHELL: Shell = shell([0x2a, 0x2a, 0x2d], Finish::Solid);

/// The NES Game Pak, in its grey.
pub const NES_SHELL: Shell = shell([0x8f, 0x8e, 0x93], Finish::Solid);

/// The colours Famicom carts came in, each game's in one of its own: Nintendo's yellow and
/// white, Hudson's and Namco's reds and blues, Konami's black and the rest.
pub const FAMICOM_SHELLS: [Shell; 8] = [
    shell([0xe8, 0xc3, 0x3a], Finish::Solid),
    shell([0xea, 0xe6, 0xdc], Finish::Solid),
    shell([0xc8, 0x36, 0x30], Finish::Solid),
    shell([0x2f, 0x58, 0xa8], Finish::Solid),
    shell([0x24, 0x24, 0x27], Finish::Solid),
    shell([0x3f, 0x8f, 0x4a], Finish::Solid),
    shell([0xe0, 0x7a, 0x2c], Finish::Solid),
    shell([0x9a, 0x9a, 0x9e], Finish::Solid),
];

/// A HuCard from Japan, the PC Engine's, its plastic a pale grey around the label.
pub const HUCARD_SHELL: Shell = shell([0xcf, 0xcf, 0xca], Finish::Solid);
/// An American TurboChip, the TurboGrafx-16's HuCard, in black.
pub const TURBOCHIP_SHELL: Shell = shell([0x2c, 0x2c, 0x30], Finish::Solid);

pub const SFC_SHELL: Shell = shell([0xb7, 0xb6, 0xbb], Finish::Solid);

const GB_CODES: &[(&str, Shell)] = &[
    ("AAU", shell([0xb3, 0x8b, 0x3a], Finish::Solid)),
    ("AAUJ", DUAL_MODE_SHELL),
    ("AAX", shell([0xa9, 0xaa, 0xa7], Finish::Solid)),
    ("AAXJ", DUAL_MODE_SHELL),
    ("BYT", CRYSTAL),
    ("BXT", CRYSTAL),
    ("KTN", KIRBY),
    ("KKK", KIRBY),
    ("KCE", shell([0x1f, 0x9f, 0xb6], Finish::Translucent)),
    ("VCA", shell([0xf0, 0xa9, 0x5e], Finish::Translucent)),
    ("VPHJ", shell([0xe2, 0xb4, 0x13], Finish::Solid)),
    ("BMG", DUAL_MODE_SHELL),
];

const GB_OVERSEAS_TITLES: &[(&str, Shell)] = &[
    ("POKEMON RED", shell([0xc0, 0x28, 0x2c], Finish::Solid)),
    ("POKEMON BLU", shell([0x2b, 0x3a, 0x88], Finish::Solid)),
    ("POKEMON YEL", shell([0xe9, 0xa8, 0x26], Finish::Solid)),
];

const CRYSTAL: Shell = shell([0x86, 0xb9, 0xbf], Finish::Glitter);
const KIRBY: Shell = shell([0xec, 0x94, 0xb4], Finish::Translucent);

pub fn shell_for(cart: &Cart) -> Shell {
    if let Some(choice) = cart.shell {
        let finish = match choice.finish {
            ShellFinish::Solid => Finish::Solid,
            ShellFinish::Clear => Finish::Translucent,
            ShellFinish::Glitter => Finish::Glitter,
        };
        return shell(choice.colour, finish);
    }
    match cart.platform {
        Platform::Gba => gba_shell_for(&cart.code),
        Platform::Gb | Platform::Gbc => gb_shell_for(&cart.rom),
        Platform::Snes => match crate::models::snes::shell_of(cart) {
            SnesShell::Ntsc => SNES_SHELL,
            SnesShell::Famicom => SFC_SHELL,
        },
        Platform::Ps1 => DEFAULT_SHELL,
        Platform::Md => match crate::models::md::shell_of(cart) {
            MdShell::MegaDrive => MEGA_DRIVE_SHELL,
            MdShell::Genesis => GENESIS_SHELL,
        },
        Platform::Nes => match crate::models::nes::shell_of(cart) {
            NesShell::Nes => NES_SHELL,
            NesShell::Famicom => famicom_shell_for(&cart.stem),
        },
        Platform::Pce => match omega_store::Region::from_tags(&cart.stem) {
            Some(omega_store::Region::Usa) => TURBOCHIP_SHELL,
            _ => HUCARD_SHELL,
        },
    }
}

/// The colour a Famicom game's cart comes in: always the same one for the same game, its name
/// without its tags picking it, so every release of a game shares one.
pub fn famicom_shell_for(stem: &str) -> Shell {
    let name = stem.split(['(', '[']).next().unwrap_or(stem).trim();
    let mut h: u32 = 0x811c_9dc5;
    for b in name.to_ascii_lowercase().bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    FAMICOM_SHELLS[(h % FAMICOM_SHELLS.len() as u32) as usize]
}

pub fn gba_shell_for(code: &str) -> Shell {
    lookup(code, EXACT, FAMILY)
}

fn gb_shell_for(rom: &Path) -> Shell {
    let Some(h) = omega_store::gb::header(rom) else {
        return flag_shell(omega_store::gb::class(rom));
    };
    let by_title = || {
        GB_OVERSEAS_TITLES
            .iter()
            .find(|(t, _)| !h.japan && *t == h.title)
            .map(|(_, s)| *s)
    };
    by_code(&h.code, GB_CODES)
        .or_else(by_title)
        .unwrap_or_else(|| flag_shell(h.class()))
}

fn flag_shell(class: Class) -> Shell {
    match class {
        Class::Original => DMG_SHELL,
        Class::DualMode => DUAL_MODE_SHELL,
        Class::ColourOnly => GB_CLEAR_SHELL,
    }
}

pub fn table_keys() -> Vec<&'static str> {
    EXACT.iter().map(|(k, _)| *k).collect()
}

pub fn gb_table_shells() -> Vec<Shell> {
    GB_CODES
        .iter()
        .chain(GB_OVERSEAS_TITLES)
        .map(|(_, s)| *s)
        .collect()
}

pub fn lookup_order_is_exact_then_family_then_default() -> bool {
    const A: Shell = shell([1, 1, 1], Finish::Solid);
    const B: Shell = shell([2, 2, 2], Finish::Solid);
    let exact = [("MSK", A)];
    let family = [(b'M', B)];
    lookup("MSKE", &exact, &family) == A
        && lookup("MPOE", &exact, &family) == B
        && lookup("ZZZZ", &exact, &family) == DEFAULT_SHELL
}

fn lookup(code: &str, exact: &[(&str, Shell)], family: &[(u8, Shell)]) -> Shell {
    if let Some(s) = by_code(code, exact) {
        return s;
    }
    if let Some(first) = code.as_bytes().first() {
        if let Some((_, s)) = family.iter().find(|(k, _)| k == first) {
            return *s;
        }
    }
    DEFAULT_SHELL
}

fn by_code(code: &str, table: &[(&str, Shell)]) -> Option<Shell> {
    let whole: String = code.chars().take(4).collect();
    let prefix: String = code.chars().take(3).collect();
    [whole, prefix]
        .iter()
        .find_map(|key| table.iter().find(|(k, _)| k == key).map(|(_, s)| *s))
}

pub fn shell_presets() -> Vec<(&'static str, Shell)> {
    const GBA: &[(&str, &str)] = &[
        ("Pokémon Ruby", "AXV"),
        ("Pokémon Sapphire", "AXP"),
        ("Pokémon Emerald", "BPE"),
        ("Pokémon FireRed", "BPR"),
        ("Pokémon LeafGreen", "BPG"),
        ("Boktai clear", "U3I"),
        ("Drill Dozer red", "V49"),
        ("Yoshi green", "KYGE"),
        ("WarioWare smoke", "RZW"),
        ("Made in Wario white", "RZWJ"),
    ];
    const GB: &[(&str, &str)] = &[
        ("Pokémon Gold", "AAU"),
        ("Pokémon Silver", "AAX"),
        ("Pokémon Crystal", "BYT"),
        ("Kirby pink", "KTN"),
        ("Command Master turquoise", "KCE"),
        ("Chee-Chai Alien orange", "VCA"),
        ("Pinball yellow", "VPHJ"),
    ];
    let mut out = vec![
        ("Game Boy grey", DMG_SHELL),
        ("Game Boy black", DUAL_MODE_SHELL),
        ("Game Boy Color clear", GB_CLEAR_SHELL),
        ("Advance charcoal", DEFAULT_SHELL),
        ("Advance Video grey", FAMILY[0].1),
        ("Super Nintendo grey", SNES_SHELL),
        ("Super Famicom grey", SFC_SHELL),
    ];
    out.extend(
        GBA.iter()
            .filter_map(|(n, k)| Some((*n, by_code(k, EXACT)?))),
    );
    out.extend(
        GB.iter()
            .filter_map(|(n, k)| Some((*n, by_code(k, GB_CODES)?))),
    );
    out.extend(
        [
            ("Pokémon Red", "POKEMON RED"),
            ("Pokémon Blue", "POKEMON BLU"),
            ("Pokémon Yellow", "POKEMON YEL"),
        ]
        .iter()
        .filter_map(|(n, t)| {
            let s = GB_OVERSEAS_TITLES.iter().find(|(k, _)| k == t)?.1;
            Some((*n, s))
        }),
    );
    out
}
