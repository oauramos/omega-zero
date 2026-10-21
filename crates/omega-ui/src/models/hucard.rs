//! The PC Engine's HuCard: a card the size of a credit card's short side by its long one, 54 by
//! 85 mm, stood upright with its contact edge at the foot, the way it goes into the console.
//!
//! It is drawn 4 units to the millimetre, 216 by 340. Its corners are rounded 3 mm; its label
//! fills a shallow well across most of its face, and under the label three grip ridges run
//! across it above the contact edge. The ridges and the well are relief in the face, not
//! geometry. The card is barely thicker than a credit card, 2.5 mm, so on the shelf it is all
//! face.

use std::sync::OnceLock;

use crate::cart::{Board, Spec, PAD};
use crate::silhouette::{depth_map, rasterise_detail, rasterise_svg, Detail};

const SVG: &str = include_str!("../../assets/hucard.svg");
const DETAIL_SVG: &str = include_str!("../../assets/hucard_detail.svg");

/// How thick the card is, as a share of its width: 2.5 mm on its 54.
pub const THICK: f32 = 2.5 / 54.0;

/// How big the card stands on the shelf, as a share of its own size: its face is made at the
/// size it stands at, so its label lands a texel to a pixel there.
pub const SHELF_SCALE: f32 = 1.0;

/// The card's outline, right half, in drawing units (216 by 340), clockwise from the top: each
/// round corner in three cuts. The left half is its mirror.
const HALF: &[(f32, f32)] = &[
    (204.0, 0.0),
    (210.0, 1.61),
    (214.39, 6.0),
    (216.0, 12.0),
    (216.0, 328.0),
    (214.39, 334.0),
    (210.0, 338.39),
    (204.0, 340.0),
];
const UNITS: (f32, f32) = (216.0, 340.0);

/// The size of the face in pixels: about 2.75 pixels to the millimetre, as the SNES carts are
/// drawn, so the card stands as tall as a SNES cart beside it and a little narrower than half
/// one. Its half is an even number of pixels across, so it stands on whole art pixels in the
/// middle of the row.
pub const W: u32 = 148;
pub const H: u32 = 234;

/// The label, in the face's pixels: the well's inside, 50 mm by 62 from 2 mm in at the sides
/// and 4 mm down from the top, its corners a little round. It reaches almost to the card's
/// edges, as a real HuCard's does, which leaves a generated title room for nine letters a line.
const LABEL: (u32, u32, u32, u32) = (6, 12, 136, 170);
const LABEL_ROUND: u32 = 3;

/// How far in from the outline a clear card's plastic lightens at its rim, in pixels.
const RIM: u32 = 4;

/// The card's outline in drawing units, all the way round clockwise, and the drawing's size.
pub(crate) fn outline() -> (Vec<(f32, f32)>, (f32, f32)) {
    let mut all = HALF.to_vec();
    all.extend(HALF.iter().rev().map(|&(x, y)| (UNITS.0 - x, y)));
    (all, UNITS)
}

/// The card's silhouette `w` by `h`, as coverage: opaque inside the outline.
pub fn silhouette(w: u32, h: u32) -> Vec<u8> {
    rasterise_svg(SVG, w, h).unwrap_or_else(|| vec![255; (w * h) as usize])
}

fn mask() -> &'static [u8] {
    static MASK: OnceLock<Vec<u8>> = OnceLock::new();
    MASK.get_or_init(|| silhouette(W, H))
}

fn depth() -> &'static [u8] {
    static DEPTH: OnceLock<Vec<u8>> = OnceLock::new();
    DEPTH.get_or_init(|| depth_map(mask(), W as usize, H as usize))
}

fn detail() -> &'static Detail {
    static DETAIL: OnceLock<Detail> = OnceLock::new();
    DETAIL.get_or_init(|| rasterise_detail(DETAIL_SVG, W, H).unwrap_or_else(|| Detail::blank(W, H)))
}

/// How the card's face is laid out. A clear card shows the chip inside: a board across its
/// middle, its contacts in a row along its foot.
pub(crate) fn spec() -> Spec {
    Spec {
        w: W,
        h: H,
        label: LABEL,
        label_round: LABEL_ROUND,
        mask: mask(),
        depth: depth(),
        detail: detail(),
        max_h: (LABEL.3 - 2 * PAD) as f32,
        rim: RIM,
        board: Board {
            x: (0.1, 0.9),
            top: 0.08,
            pins: (0.12, 0.88),
            contacts_from: 0.94,
            traces_from: 0.88,
        },
    }
}
