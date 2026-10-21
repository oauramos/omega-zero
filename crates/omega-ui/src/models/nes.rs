//! The NES cart, in its two shells: the grey Game Pak of America and Europe, and Japan's
//! Famicom cart, which came in a colour of its game's own.
//!
//! Both are drawn 4 units to the millimetre. The Game Pak is 120 by 133 mm and 20 mm thick, stood
//! upright with its label in a well across the top of its face and five grip ridges under it; the
//! Famicom cart is 108 by 71 mm and 17 mm thick, lying across, its label filling most of its face
//! and grip grooves down its sides. The ridges, grooves and wells are relief in the face, not
//! geometry. Both faces are drawn about 1.77 pixels to the millimetre, a little under the SNES
//! carts', so the Game Pak stands about as tall as a SNES cart and the Famicom cart as much the
//! smaller as it is in the hand.

use std::sync::OnceLock;

use omega_store::{Cart, Outline, Region};

use crate::cart::{Board, Spec, PAD};
use crate::silhouette::{depth_map, rasterise_detail, rasterise_svg, Detail};

const NES_SVG: &str = include_str!("../../assets/nes_cart.svg");
const NES_DETAIL_SVG: &str = include_str!("../../assets/nes_cart_detail.svg");
const FAMICOM_SVG: &str = include_str!("../../assets/famicom_cart.svg");
const FAMICOM_DETAIL_SVG: &str = include_str!("../../assets/famicom_cart_detail.svg");

/// The NES cart shells a model is built for.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum NesShell {
    /// The Game Pak of America and Europe.
    Nes,
    /// Japan's Famicom cart.
    Famicom,
}

/// How thick each cart is, as a share of its width: 20 mm on the Game Pak's 120 and 17 on the
/// Famicom cart's 108.
pub const NES_THICK: f32 = 20.0 / 120.0;
pub const FAMICOM_THICK: f32 = 17.0 / 108.0;

/// How big the cart stands on the shelf, as a share of its own size: its face is made at the
/// size it stands at, so its label lands a texel to a pixel there.
pub const SHELF_SCALE: f32 = 1.0;

/// The Game Pak's outline, right half, in drawing units (480 by 532), clockwise from the top:
/// each round corner in three cuts. The left half is its mirror.
const NES_HALF: &[(f32, f32)] = &[
    (472.0, 0.0),
    (476.0, 1.07),
    (478.93, 4.0),
    (480.0, 8.0),
    (480.0, 524.0),
    (478.93, 528.0),
    (476.0, 530.93),
    (472.0, 532.0),
];
const NES_UNITS: (f32, f32) = (480.0, 532.0);

/// The Famicom cart's, right half, in drawing units (432 by 284): its round top corner in
/// three cuts and its foot in two.
const FAMICOM_HALF: &[(f32, f32)] = &[
    (420.0, 0.0),
    (426.0, 1.61),
    (430.39, 6.0),
    (432.0, 12.0),
    (432.0, 280.0),
    (430.83, 282.83),
    (428.0, 284.0),
];
const FAMICOM_UNITS: (f32, f32) = (432.0, 284.0);

/// The size of each face in pixels. Each is a multiple of four across, so it halves onto the art
/// grid in the middle of the row, and an even number down.
pub const NES_W: u32 = 212;
pub const NES_H: u32 = 236;
pub const FAMICOM_W: u32 = 192;
pub const FAMICOM_H: u32 = 126;

/// The label on each face, in its pixels: the inside of its well. The Game Pak's is 95 mm by 68
/// across the top of its face; the Famicom cart's 95 by 50 across most of its, its corners a
/// little round.
const NES_LABEL: (u32, u32, u32, u32) = (22, 20, 168, 120);
const FAMICOM_LABEL: (u32, u32, u32, u32) = (12, 12, 168, 88);
const FAMICOM_LABEL_ROUND: u32 = 3;

/// How far in from the outline a clear shell's plastic lightens at its rim, in pixels.
const RIM: u32 = 5;

/// The shell `cart` comes in: the one `cart_shell.ini` names, else the Famicom cart for a game
/// from Japan and the Game Pak for any other.
pub fn shell_of(cart: &Cart) -> NesShell {
    match cart.shell.map(|c| c.outline) {
        Some(Outline::Nes) => NesShell::Nes,
        Some(Outline::Famicom) => NesShell::Famicom,
        _ => match Region::from_tags(&cart.stem) {
            Some(Region::Japan) => NesShell::Famicom,
            _ => NesShell::Nes,
        },
    }
}

/// The cart's outline in drawing units, all the way round clockwise, and the drawing's size.
pub(crate) fn outline(shell: NesShell) -> (Vec<(f32, f32)>, (f32, f32)) {
    let (half, units) = match shell {
        NesShell::Nes => (NES_HALF, NES_UNITS),
        NesShell::Famicom => (FAMICOM_HALF, FAMICOM_UNITS),
    };
    let mut all = half.to_vec();
    all.extend(half.iter().rev().map(|&(x, y)| (units.0 - x, y)));
    (all, units)
}

/// How thick `shell` is, as a share of its width.
pub(crate) fn thick(shell: NesShell) -> f32 {
    match shell {
        NesShell::Nes => NES_THICK,
        NesShell::Famicom => FAMICOM_THICK,
    }
}

/// The cart's silhouette `w` by `h`, as coverage: opaque inside the outline.
pub fn silhouette(shell: NesShell, w: u32, h: u32) -> Vec<u8> {
    let svg = match shell {
        NesShell::Nes => NES_SVG,
        NesShell::Famicom => FAMICOM_SVG,
    };
    rasterise_svg(svg, w, h).unwrap_or_else(|| vec![255; (w * h) as usize])
}

/// The size of the cart's face in pixels.
pub(crate) fn face_box(shell: NesShell) -> (u32, u32) {
    match shell {
        NesShell::Nes => (NES_W, NES_H),
        NesShell::Famicom => (FAMICOM_W, FAMICOM_H),
    }
}

fn mask(shell: NesShell) -> &'static [u8] {
    static NES: OnceLock<Vec<u8>> = OnceLock::new();
    static FAMICOM: OnceLock<Vec<u8>> = OnceLock::new();
    let lock = match shell {
        NesShell::Nes => &NES,
        NesShell::Famicom => &FAMICOM,
    };
    lock.get_or_init(|| {
        let (w, h) = face_box(shell);
        silhouette(shell, w, h)
    })
}

fn depth(shell: NesShell) -> &'static [u8] {
    static NES: OnceLock<Vec<u8>> = OnceLock::new();
    static FAMICOM: OnceLock<Vec<u8>> = OnceLock::new();
    let lock = match shell {
        NesShell::Nes => &NES,
        NesShell::Famicom => &FAMICOM,
    };
    lock.get_or_init(|| {
        let (w, h) = face_box(shell);
        depth_map(mask(shell), w as usize, h as usize)
    })
}

fn detail(shell: NesShell) -> &'static Detail {
    static NES: OnceLock<Detail> = OnceLock::new();
    static FAMICOM: OnceLock<Detail> = OnceLock::new();
    let (lock, svg) = match shell {
        NesShell::Nes => (&NES, NES_DETAIL_SVG),
        NesShell::Famicom => (&FAMICOM, FAMICOM_DETAIL_SVG),
    };
    lock.get_or_init(|| {
        let (w, h) = face_box(shell);
        rasterise_detail(svg, w, h).unwrap_or_else(|| Detail::blank(w, h))
    })
}

/// How the cart's face is laid out. A clear shell shows the board inside: across the cart, its
/// contacts along its foot.
pub(crate) fn spec(shell: NesShell) -> Spec {
    let (w, h) = face_box(shell);
    let (label, label_round) = match shell {
        NesShell::Nes => (NES_LABEL, 0),
        NesShell::Famicom => (FAMICOM_LABEL, FAMICOM_LABEL_ROUND),
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
            top: 0.1,
            pins: (0.15, 0.85),
            contacts_from: 0.88,
            traces_from: 0.8,
        },
    }
}
