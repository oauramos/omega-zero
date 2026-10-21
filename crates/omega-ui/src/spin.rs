use omega_gfx::{out_h, spun_x, Draw, MeshId, MeshVertex, TexId};

use crate::models::disc::StackMeshes;
use crate::models::md::{self, MdShell};
use crate::models::nes::{self, NesShell};
use crate::models::{disc, hucard, snes, snes::SnesShell};
use crate::silhouette::GbShell;

/// How thick a cart is, as a share of its width: a Game Pak is about 8 mm thick and 57 wide.
pub const THICK: f32 = 0.14;

/// How dark a cart's side is against its face, from the back of the side to the front.
const SIDE_SHADE: [f32; 2] = [0.34, 0.52];

/// The side is built from slices of the face at most this far apart across the canvas, close
/// enough that the steps between them do not show.
const SLICE_PX: f32 = 0.75;
const MAX_SLICES: usize = 16;

/// A side narrower than this is not drawn at all.
const MIN_SIDE_PX: f32 = 0.5;

/// How far in from the edge of its box a cart's outline comes, as a share of its width: past
/// that, a slice of the face is always hidden behind the face, so it is not drawn there.
pub const OUTLINE_INSET: f32 = 0.13;

/// How a cart is held: turned `yaw` radians about its upright axis, and lifted `lift` canvas
/// pixels off where it would stand.
#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub struct Pose {
    pub yaw: f32,
    pub lift: f32,
}

impl Pose {
    pub const REST: Pose = Pose {
        yaw: 0.0,
        lift: 0.0,
    };

    /// This pose `k` of the way back to rest.
    pub fn toward_rest(self, k: f32) -> Pose {
        let keep = 1.0 - k.clamp(0.0, 1.0);
        Pose {
            yaw: self.yaw * keep,
            lift: self.lift * keep,
        }
    }
}

/// The shapes a game stands on the shelf as: a cart in one of its shells, or a disc.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum CartModel {
    Gba,
    Gb(GbShell),
    Snes(SnesShell),
    Disc,
    HuCard,
    Nes(NesShell),
    Md(MdShell),
}

impl CartModel {
    pub const ALL: [CartModel; 11] = [
        CartModel::Gba,
        CartModel::Gb(GbShell::Notched),
        CartModel::Gb(GbShell::Rounded),
        CartModel::Snes(SnesShell::Ntsc),
        CartModel::Snes(SnesShell::Famicom),
        CartModel::Disc,
        CartModel::HuCard,
        CartModel::Nes(NesShell::Nes),
        CartModel::Nes(NesShell::Famicom),
        CartModel::Md(MdShell::MegaDrive),
        CartModel::Md(MdShell::Genesis),
    ];

    /// Its place in [`CartModel::ALL`].
    pub fn index(self) -> usize {
        match self {
            CartModel::Gba => 0,
            CartModel::Gb(GbShell::Notched) => 1,
            CartModel::Gb(GbShell::Rounded) => 2,
            CartModel::Snes(SnesShell::Ntsc) => 3,
            CartModel::Snes(SnesShell::Famicom) => 4,
            CartModel::Disc => 5,
            CartModel::HuCard => 6,
            CartModel::Nes(NesShell::Nes) => 7,
            CartModel::Nes(NesShell::Famicom) => 8,
            CartModel::Md(MdShell::MegaDrive) => 9,
            CartModel::Md(MdShell::Genesis) => 10,
        }
    }

    /// How thick it is, as a share of its width.
    pub fn thick(self) -> f32 {
        match self {
            CartModel::Gba | CartModel::Gb(_) => THICK,
            CartModel::Snes(_) => snes::THICK,
            CartModel::Disc => disc::THICK,
            CartModel::HuCard => hucard::THICK,
            CartModel::Nes(shell) => nes::thick(shell),
            CartModel::Md(shell) => md::thick(shell),
        }
    }
}

/// The models a shelf draws its carts with, one per model, once they are on the GPU, and the
/// ones it draws a game on several discs with.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct CartMeshes {
    models: [MeshId; CartModel::ALL.len()],
    stacks: Option<StackMeshes>,
}

impl CartMeshes {
    /// Every model's mesh, each made by `make`.
    pub fn build(mut make: impl FnMut(CartModel) -> MeshId) -> CartMeshes {
        CartMeshes {
            models: CartModel::ALL.map(&mut make),
            stacks: None,
        }
    }

    /// Every model's mesh and every stack's, each handed to `make` to put on the GPU: what the
    /// frontend draws with.
    pub fn upload(mut make: impl FnMut(&[MeshVertex]) -> MeshId) -> CartMeshes {
        CartMeshes::build(|model| make(&cart_mesh(model))).with_stacks(StackMeshes::build(make))
    }

    /// These, and `stacks` to draw a game on several discs with. Without them such a game is
    /// one model with every disc's face laid across it.
    pub fn with_stacks(self, stacks: StackMeshes) -> CartMeshes {
        CartMeshes {
            stacks: Some(stacks),
            ..self
        }
    }

    pub fn of(&self, model: CartModel) -> MeshId {
        self.models[model.index()]
    }

    /// The models stacks of discs are drawn with, once they are set.
    pub fn stacks(&self) -> Option<StackMeshes> {
        self.stacks
    }
}

/// A GBA cart's outline in the units of its drawing (240 by 141), right half, going round
/// clockwise from the top: the grip's corners cut in three, the step down to the body, and the
/// body's own corners cut in two. The left half is its mirror.
const GBA_HALF: &[(f32, f32)] = &[
    (229.3, 0.0),
    (234.9, 2.2),
    (238.8, 7.4),
    (240.0, 11.85),
    (240.0, 25.6),
    (237.6, 28.6),
    (233.5, 28.89),
    (233.5, 136.7),
    (232.2, 139.4),
    (229.0, 141.0),
];

/// A Game Boy pak's outline in the units of its drawing (240 by 259), all the way round
/// clockwise from the top left: the notched corner of the grey paks.
const GB_OUTLINE: &[(f32, f32)] = &[
    (9.8, 0.0),
    (208.9, 0.0),
    (211.9, 1.2),
    (213.1, 4.0),
    (213.1, 11.5),
    (228.7, 11.5),
    (232.1, 13.0),
    (233.5, 16.5),
    (233.5, 255.6),
    (232.2, 258.0),
    (229.2, 259.0),
    (10.8, 259.0),
    (7.8, 258.0),
    (6.5, 255.6),
    (6.5, 2.4),
    (7.5, 0.7),
];

/// A Game Boy Color pak's: its two round top corners in three cuts each.
const GBC_OUTLINE: &[(f32, f32)] = &[
    (24.5, 0.0),
    (215.5, 0.0),
    (224.5, 2.41),
    (231.1, 9.0),
    (233.5, 18.0),
    (233.5, 255.6),
    (232.2, 258.0),
    (229.2, 259.0),
    (10.8, 259.0),
    (7.8, 258.0),
    (6.5, 255.6),
    (6.5, 18.0),
    (8.9, 9.0),
    (15.5, 2.41),
];

/// The face is laid over this many cells across and down: textures are mapped without
/// perspective, so a face in one piece would bend its label along the diagonal as it turns, the
/// way big polygons did on the PlayStation. A few cells keep a little of that.
const FACE_CELLS: (usize, usize) = (4, 3);

/// How far in from the outline, in drawing units, a side takes its colour from the face.
const SIDE_SAMPLE: f32 = 9.0;

/// How much light the back edge of a side gets against its front edge.
const SIDE_BACK_SHADE: f32 = 0.72;

/// The GBA cart's outline in drawing units, all the way round clockwise, and the size of the
/// drawing.
pub(crate) fn gba_outline() -> (Vec<(f32, f32)>, (f32, f32)) {
    let mut all: Vec<(f32, f32)> = GBA_HALF.to_vec();
    all.extend(GBA_HALF.iter().rev().map(|&(x, y)| (240.0 - x, y)));
    (all, (240.0, 141.0))
}

/// A cart's outline in drawing units, all the way round clockwise, and the size of the drawing.
pub fn outline(model: CartModel) -> (Vec<(f32, f32)>, (f32, f32)) {
    match model {
        CartModel::Gba => gba_outline(),
        CartModel::Gb(GbShell::Notched) => (GB_OUTLINE.to_vec(), (240.0, 259.0)),
        CartModel::Gb(GbShell::Rounded) => (GBC_OUTLINE.to_vec(), (240.0, 259.0)),
        CartModel::Snes(shell) => snes::outline(shell),
        CartModel::Disc => disc::outline(),
        CartModel::HuCard => hucard::outline(),
        CartModel::Nes(shell) => nes::outline(shell),
        CartModel::Md(shell) => md::outline(shell),
    }
}

/// A model in few polygons, the way a PlayStation game would have modelled it.
pub fn cart_mesh(model: CartModel) -> Vec<MeshVertex> {
    match model {
        CartModel::Disc => disc::mesh(),
        _ => {
            let (points, units) = outline(model);
            extruded(&points, units, model.thick())
        }
    }
}

/// A cart in few polygons: its face, the whole texture laid over a few cells, whose outline
/// the texture's own alpha cuts; and its sides, one flat polygon for each cut of its outline
/// `points` (in drawing units, the drawing `uw` by `uh`), back to `thick`, each taking its
/// colour from the face just inside its edge. Sides come first, so the face covers them.
pub(crate) fn extruded(points: &[(f32, f32)], (uw, uh): (f32, f32), thick: f32) -> Vec<MeshVertex> {
    let mut out = Vec::new();
    for (i, &a) in points.iter().enumerate() {
        let b = points[(i + 1) % points.len()];
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-3 {
            continue;
        }
        let normal = [dy / len, -dx / len, 0.0];
        let sample = |p: (f32, f32)| {
            [
                (p.0 - normal[0] * SIDE_SAMPLE) / uw,
                (p.1 - normal[1] * SIDE_SAMPLE) / uh,
            ]
        };
        let corner = |p: (f32, f32), z: f32, shade: f32| MeshVertex {
            xyz: [p.0 / uw, p.1 / uh, z],
            normal,
            uv: sample(p),
            shade,
        };
        let (a0, b0) = (corner(a, 0.0, 1.0), corner(b, 0.0, 1.0));
        let (a1, b1) = (
            corner(a, thick, SIDE_BACK_SHADE),
            corner(b, thick, SIDE_BACK_SHADE),
        );
        out.extend([a0, b0, b1, a0, b1, a1]);
    }
    let (cols, rows) = FACE_CELLS;
    let at = |c: usize, r: usize| {
        let (u, v) = (c as f32 / cols as f32, r as f32 / rows as f32);
        MeshVertex {
            xyz: [u, v, 0.0],
            normal: [0.0, 0.0, -1.0],
            uv: [u, v],
            shade: 1.0,
        }
    };
    for r in 0..rows {
        for c in 0..cols {
            let (p, q, s, t) = (at(c, r), at(c + 1, r), at(c + 1, r + 1), at(c, r + 1));
            out.extend([p, q, s, p, s, t]);
        }
    }
    out
}

/// A cart at `x, y, w, h` as its model, `mesh`, textured with its face, tinted `colour`, turned
/// `turn` radians in the canvas and spun `yaw` about its upright axis.
pub fn posed_cart(
    (x, y, w, h): (f32, f32, f32, f32),
    mesh: MeshId,
    tex: TexId,
    colour: [f32; 4],
    turn: f32,
    yaw: f32,
) -> Draw {
    Draw::Model {
        x,
        y,
        w,
        h,
        mesh,
        tex,
        colour,
        turn,
        yaw,
    }
}

/// A cart face at `x, y, w, h`, turned `turn` radians in the canvas and spun `yaw` about its
/// upright axis. An unspun face is a plain `Tex` or `Turned`, exactly as before carts turned.
pub fn posed_face(
    (x, y, w, h): (f32, f32, f32, f32),
    tex: TexId,
    alpha: f32,
    turn: f32,
    yaw: f32,
) -> Draw {
    match (yaw == 0.0, turn == 0.0) {
        (true, true) => Draw::Tex {
            x,
            y,
            w,
            h,
            tex,
            alpha,
        },
        (true, false) => Draw::Turned {
            x,
            y,
            w,
            h,
            tex,
            alpha,
            turn,
        },
        (false, _) => Draw::Spun {
            x,
            y,
            w,
            h,
            tex,
            colour: [1.0, 1.0, 1.0, alpha],
            turn,
            yaw,
            depth: 0.0,
            clip: None,
        },
    }
}

/// The side of a cart whose face is drawn by `posed_face` with the same rect, turn and yaw: slices of
/// the face, darkened by `shade` and drawn `alpha`, stepping back to the cart's thickness on
/// the side that turns towards the eye. They go out before the face, which covers all but the
/// side. A cart turned in the canvas draws every slice whole, since its side is not upright.
pub fn push_cart_side(
    out: &mut Vec<Draw>,
    (x, y, w, h): (f32, f32, f32, f32),
    tex: TexId,
    turn: f32,
    yaw: f32,
    shade: f32,
    alpha: f32,
) {
    let thick = THICK * w;
    let side = thick * yaw.sin().abs();
    if side < MIN_SIDE_PX || alpha <= 0.0 {
        return;
    }
    let slices = ((side / SLICE_PX).ceil() as usize).clamp(1, MAX_SLICES);
    let clip = (turn == 0.0).then(|| side_band(x, w, yaw, thick));
    for k in (1..=slices).rev() {
        let f = k as f32 / slices as f32;
        let dark = shade * (SIDE_SHADE[1] + (SIDE_SHADE[0] - SIDE_SHADE[1]) * f);
        out.push(Draw::Spun {
            x,
            y,
            w,
            h,
            tex,
            colour: [dark, dark, dark, alpha],
            turn,
            yaw,
            depth: thick * f,
            clip,
        });
    }
}

/// The upright band of the canvas where the side of a cart `w` wide at `x`, spun `yaw`, can
/// show: from as far in as its outline comes on the side nearer the eye, out to where the
/// back of the cart reaches.
fn side_band(x: f32, w: f32, yaw: f32, thick: f32) -> [f32; 4] {
    let half = w / 2.0;
    let near = if yaw.sin() > 0.0 { -half } else { half };
    let edge = x + half + spun_x(near, yaw, 0.0);
    let back = x + half + spun_x(near, yaw, thick);
    let inset = OUTLINE_INSET * w;
    let (left, right) = match near < 0.0 {
        true => (back.min(edge) - 1.0, edge + inset + 1.0),
        false => (edge - inset - 1.0, back.max(edge) + 1.0),
    };
    [left, 0.0, right - left, out_h() as f32]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cart::{CART_H, CART_W, GB_CART_H, GB_CART_W};
    use crate::silhouette::{cart_mask, gb_cart_mask, GbShell};

    /// The furthest any row of `mask` keeps in from either side of its box.
    fn widest_inset(mask: &[u8], w: usize, h: usize) -> usize {
        (0..h)
            .filter_map(|y| {
                let row = &mask[y * w..(y + 1) * w];
                let first = row.iter().position(|&c| c > 0)?;
                let last = row.iter().rposition(|&c| c > 0)?;
                Some(first.max(w - 1 - last))
            })
            .max()
            .unwrap_or(0)
    }

    #[test]
    fn every_cart_outline_stays_within_the_band_its_side_is_drawn_in() {
        let (ntsc, sfc) = (snes::spec(SnesShell::Ntsc), snes::spec(SnesShell::Famicom));
        let (_, (_, units)) = snes::outline(SnesShell::Ntsc);
        // the American cart's column stands proud of its shoulders: only the rows above them
        // come in further, and only by the step
        let shoulder = (snes::NTSC_SHOULDER / units * ntsc.h as f32).ceil() as u32;
        for (name, mask, w, h, from) in [
            ("GBA", cart_mask(), CART_W, CART_H, 0),
            (
                "GB",
                gb_cart_mask(GbShell::Notched),
                GB_CART_W,
                GB_CART_H,
                0,
            ),
            (
                "GBC",
                gb_cart_mask(GbShell::Rounded),
                GB_CART_W,
                GB_CART_H,
                0,
            ),
            ("SNES", ntsc.mask, ntsc.w, ntsc.h, shoulder),
            ("SFC", sfc.mask, sfc.w, sfc.h, 0),
        ] {
            let rows = &mask[(from * w) as usize..];
            let inset = widest_inset(rows, w as usize, (h - from) as usize) as f32;
            assert!(
                inset <= OUTLINE_INSET * w as f32,
                "the {name} outline comes {inset} px in, past the {} px its side is drawn in",
                OUTLINE_INSET * w as f32
            );
        }
        let step = widest_inset(ntsc.mask, ntsc.w as usize, shoulder as usize) as f32;
        assert!(
            step <= 0.18 * ntsc.w as f32,
            "the American cart's shoulders come {step} px in, more than its column's step"
        );
    }

    /// Only the Super Famicom, HuCard, Famicom and Mega Drive labels are round at their corners:
    /// every other shape's goes through the square label's drawing untouched.
    #[test]
    fn only_the_super_famicom_hucard_famicom_and_mega_drive_labels_are_round() {
        for model in CartModel::ALL {
            let round = crate::cart::spec(model).label_round;
            match model {
                CartModel::Snes(SnesShell::Famicom)
                | CartModel::HuCard
                | CartModel::Nes(NesShell::Famicom)
                | CartModel::Md(MdShell::MegaDrive) => assert!(round > 0),
                _ => assert_eq!(round, 0, "the {model:?} label is round"),
            }
        }
    }

    /// A side takes its colour from the face just inside its edge, so that has to be the
    /// cart's plastic, not the clear corner beyond its outline.
    #[test]
    fn every_side_takes_its_colour_from_the_plastic_inside_its_edge() {
        for model in CartModel::ALL {
            if model == CartModel::Disc {
                continue;
            }
            let s = crate::cart::spec(model);
            let cover = |u: f32, v: f32| {
                let (x, y) = ((u * s.w as f32) as u32, (v * s.h as f32) as u32);
                s.mask[(y.min(s.h - 1) * s.w + x.min(s.w - 1)) as usize]
            };
            for v in cart_mesh(model).iter().filter(|v| v.normal[2] == 0.0) {
                let [u, t] = v.uv;
                assert!(
                    cover(u, t) > 192,
                    "a side of the {model:?} model takes its colour from {u},{t}, outside the cart"
                );
            }
        }
    }
}
