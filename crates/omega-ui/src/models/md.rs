//! The Mega Drive cart, in its two shells: the Mega Drive's of Japan and Europe, and the
//! Genesis's of America.
//!
//! Both are drawn 4 units to the millimetre. The Mega Drive cart is 112 by 130 mm, its top
//! corners rolled round, its label in a round-cornered well over the top of its face and a
//! raised bar across its foot where the console's name is moulded; the Genesis cart is 110 by
//! 135 mm and squarer, three grip ridges across its top and its label in a well under them.
//! Both are 20 mm thick and black. The ridges, bar and wells are relief in the face, not
//! geometry. Both faces are drawn about 1.78 pixels to the millimetre, as the NES carts are.

use std::sync::OnceLock;

use omega_store::{Cart, Outline, Region};

use crate::cart::{Board, Spec, PAD};
use crate::silhouette::{depth_map, rasterise_detail, rasterise_svg, Detail};

const MEGA_DRIVE_SVG: &str = include_str!("../../assets/megadrive_cart.svg");
const MEGA_DRIVE_DETAIL_SVG: &str = include_str!("../../assets/megadrive_cart_detail.svg");
const GENESIS_SVG: &str = include_str!("../../assets/genesis_cart.svg");
const GENESIS_DETAIL_SVG: &str = include_str!("../../assets/genesis_cart_detail.svg");

/// The Mega Drive cart shells a model is built for.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum MdShell {
    /// The Mega Drive's, of Japan and Europe.
    MegaDrive,
    /// The Genesis's, of America.
    Genesis,
}

/// How thick each cart is, as a share of its width: 20 mm on the Mega Drive cart's 112 and the
/// Genesis cart's 110.
pub const MEGA_DRIVE_THICK: f32 = 20.0 / 112.0;
pub const GENESIS_THICK: f32 = 20.0 / 110.0;

/// How big the cart stands on the shelf, as a share of its own size: its face is made at the
/// size it stands at, so its label lands a texel to a pixel there.
pub const SHELF_SCALE: f32 = 1.0;

/// The Mega Drive cart's outline, right half, in drawing units (448 by 520), clockwise from the
/// top: its round top corner in three cuts and its foot in two. The left half is its mirror.
const MEGA_DRIVE_HALF: &[(f32, f32)] = &[
    (408.0, 0.0),
    (428.0, 5.36),
    (442.64, 20.0),
    (448.0, 40.0),
    (448.0, 512.0),
    (445.66, 517.66),
    (440.0, 520.0),
];
const MEGA_DRIVE_UNITS: (f32, f32) = (448.0, 520.0);

/// The Genesis cart's, right half, in drawing units (440 by 540): its top corner in three cuts
/// and its foot in two.
const GENESIS_HALF: &[(f32, f32)] = &[
    (430.0, 0.0),
    (435.0, 1.34),
    (438.66, 5.0),
    (440.0, 10.0),
    (440.0, 534.0),
    (438.24, 538.24),
    (434.0, 540.0),
];
const GENESIS_UNITS: (f32, f32) = (440.0, 540.0);

/// The size of each face in pixels. Each is a multiple of four across, so it halves onto the art
/// grid in the middle of the row, and an even number down.
pub const MEGA_DRIVE_W: u32 = 200;
pub const MEGA_DRIVE_H: u32 = 232;
pub const GENESIS_W: u32 = 196;
pub const GENESIS_H: u32 = 240;

/// The label on each face, in its pixels: the inside of its well. The Mega Drive cart's is 92 mm
/// by 85 across the top of its face, its corners round; the Genesis cart's 86 by 79 under its
/// grip ridges.
const MEGA_DRIVE_LABEL: (u32, u32, u32, u32) = (18, 18, 164, 152);
const MEGA_DRIVE_LABEL_ROUND: u32 = 5;
const GENESIS_LABEL: (u32, u32, u32, u32) = (22, 38, 152, 140);

/// How far in from the outline a clear shell's plastic lightens at its rim, in pixels.
const RIM: u32 = 5;

/// The shell `cart` comes in: the one `cart_shell.ini` names, else the Genesis cart for a game
/// from America and the Mega Drive's for any other.
pub fn shell_of(cart: &Cart) -> MdShell {
    match cart.shell.map(|c| c.outline) {
        Some(Outline::MegaDrive) => MdShell::MegaDrive,
        Some(Outline::Genesis) => MdShell::Genesis,
        _ => match omega_store::md::region(&cart.rom, &cart.stem) {
            Some(Region::Usa) => MdShell::Genesis,
            _ => MdShell::MegaDrive,
        },
    }
}

/// The cart's outline in drawing units, all the way round clockwise, and the drawing's size.
pub(crate) fn outline(shell: MdShell) -> (Vec<(f32, f32)>, (f32, f32)) {
    let (half, units) = match shell {
        MdShell::MegaDrive => (MEGA_DRIVE_HALF, MEGA_DRIVE_UNITS),
        MdShell::Genesis => (GENESIS_HALF, GENESIS_UNITS),
    };
    let mut all = half.to_vec();
    all.extend(half.iter().rev().map(|&(x, y)| (units.0 - x, y)));
    (all, units)
}

/// How thick `shell` is, as a share of its width.
pub(crate) fn thick(shell: MdShell) -> f32 {
    match shell {
        MdShell::MegaDrive => MEGA_DRIVE_THICK,
        MdShell::Genesis => GENESIS_THICK,
    }
}

/// The cart's silhouette `w` by `h`, as coverage: opaque inside the outline.
pub fn silhouette(shell: MdShell, w: u32, h: u32) -> Vec<u8> {
    let svg = match shell {
        MdShell::MegaDrive => MEGA_DRIVE_SVG,
        MdShell::Genesis => GENESIS_SVG,
    };
    rasterise_svg(svg, w, h).unwrap_or_else(|| vec![255; (w * h) as usize])
}

/// The size of the cart's face in pixels.
pub(crate) fn face_box(shell: MdShell) -> (u32, u32) {
    match shell {
        MdShell::MegaDrive => (MEGA_DRIVE_W, MEGA_DRIVE_H),
        MdShell::Genesis => (GENESIS_W, GENESIS_H),
    }
}

fn mask(shell: MdShell) -> &'static [u8] {
    static MEGA_DRIVE: OnceLock<Vec<u8>> = OnceLock::new();
    static GENESIS: OnceLock<Vec<u8>> = OnceLock::new();
    let lock = match shell {
        MdShell::MegaDrive => &MEGA_DRIVE,
        MdShell::Genesis => &GENESIS,
    };
    lock.get_or_init(|| {
        let (w, h) = face_box(shell);
        silhouette(shell, w, h)
    })
}

fn depth(shell: MdShell) -> &'static [u8] {
    static MEGA_DRIVE: OnceLock<Vec<u8>> = OnceLock::new();
    static GENESIS: OnceLock<Vec<u8>> = OnceLock::new();
    let lock = match shell {
        MdShell::MegaDrive => &MEGA_DRIVE,
        MdShell::Genesis => &GENESIS,
    };
    lock.get_or_init(|| {
        let (w, h) = face_box(shell);
        depth_map(mask(shell), w as usize, h as usize)
    })
}

fn detail(shell: MdShell) -> &'static Detail {
    static MEGA_DRIVE: OnceLock<Detail> = OnceLock::new();
    static GENESIS: OnceLock<Detail> = OnceLock::new();
    let (lock, svg) = match shell {
        MdShell::MegaDrive => (&MEGA_DRIVE, MEGA_DRIVE_DETAIL_SVG),
        MdShell::Genesis => (&GENESIS, GENESIS_DETAIL_SVG),
    };
    lock.get_or_init(|| {
        let (w, h) = face_box(shell);
        rasterise_detail(svg, w, h).unwrap_or_else(|| Detail::blank(w, h))
    })
}

/// How the cart's face is laid out. A clear shell shows the board inside: across the cart, its
/// contacts along its foot.
pub(crate) fn spec(shell: MdShell) -> Spec {
    let (w, h) = face_box(shell);
    let (label, label_round) = match shell {
        MdShell::MegaDrive => (MEGA_DRIVE_LABEL, MEGA_DRIVE_LABEL_ROUND),
        MdShell::Genesis => (GENESIS_LABEL, 0),
    };
    Spec {
        w,
        h,
        label,
        label_round,
        mask: mask(shell),
        depth: depth(shell),
        detail: detail(shell),
        max_h: (label.3 - 2 * PAD) as f32,
        rim: RIM,
        board: Board {
            x: (0.1, 0.9),
            top: 0.12,
            pins: (0.15, 0.85),
            contacts_from: 0.9,
            traces_from: 0.82,
        },
    }
}
