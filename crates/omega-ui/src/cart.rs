use std::path::PathBuf;

use omega_store::gb::Class;
use omega_store::{Cart, Outline, Platform};

use crate::label_cut::label_art;
use crate::models::md::{self, MdShell};
use crate::models::nes::{self, NesShell};
use crate::models::{disc, hucard, snes, snes::SnesShell};
use crate::shell::{shell_for, Finish, Shell};
use crate::silhouette::{
    cart_depth, cart_mask, detail_mask, gb_cart_depth, gb_cart_mask, gb_detail_mask, Detail,
    GbShell,
};
use crate::spin::CartModel;
use crate::text;

pub const CART_W: u32 = 360;
pub const CART_H: u32 = (CART_W * SEATED_H + SEATED_W / 2) / SEATED_W;

pub const GB_CART_W: u32 = 312;

pub const GB_CART_H: u32 = (GB_CART_W * GB_SEATED_H + SEATED_W / 2) / SEATED_W;

pub const fn label_panel(w: u32, h: u32) -> (u32, u32, u32, u32) {
    (
        (w * 135 + 500) / 1000,
        (h * 231 + 500) / 1000,
        (w * 875 + 500) / 1000,
        (h * 882 + 500) / 1000,
    )
}

pub const LABEL_X: u32 = label_panel(CART_W, CART_H).0;
pub const LABEL_Y: u32 = label_panel(CART_W, CART_H).1;
pub const LABEL_W: u32 = label_panel(CART_W, CART_H).2 - LABEL_X;
pub const LABEL_H: u32 = label_panel(CART_W, CART_H).3 - LABEL_Y;

pub const fn gb_label_panel(w: u32, h: u32) -> (u32, u32, u32, u32) {
    (
        (w * 147 + 500) / 1000,
        (h * 291 + 500) / 1000,
        (w * 854 + 500) / 1000,
        (h * 868 + 500) / 1000,
    )
}

pub const GB_LABEL_X: u32 = gb_label_panel(GB_CART_W, GB_CART_H).0;
pub const GB_LABEL_Y: u32 = gb_label_panel(GB_CART_W, GB_CART_H).1;
pub const GB_LABEL_W: u32 = gb_label_panel(GB_CART_W, GB_CART_H).2 - GB_LABEL_X;
pub const GB_LABEL_H: u32 = gb_label_panel(GB_CART_W, GB_CART_H).3 - GB_LABEL_Y;

pub(crate) const PAD: u32 = 10;
const MAX_LINES: usize = 3;
/// A generated label's title is set in Pixel Code at one art pixel (two of the face's pixels) to
/// one of the face's, the size every word in the UI is set at, on up to three lines.
const MAX_PX: f32 = 18.0;
const MIN_PX: f32 = 18.0;

const RIM: u32 = (4 * CART_W + SEATED_W / 2) / SEATED_W;
const GB_RIM: u32 = (7 * GB_CART_W + SEATED_W / 2) / SEATED_W;

pub struct CartFace {
    pub rgba: Vec<u8>,
    pub w: u32,
    pub h: u32,
}

/// How a model's face is laid out: its size in pixels, where its label goes, its outline and
/// relief, and the board a clear shell shows.
pub(crate) struct Spec {
    pub(crate) w: u32,
    pub(crate) h: u32,
    pub(crate) label: (u32, u32, u32, u32),
    /// How round the label's corners are, in pixels, its well's bevel round with them: none, a
    /// square label, but on the Super Famicom cart.
    pub(crate) label_round: u32,
    pub(crate) mask: &'static [u8],
    pub(crate) depth: &'static [u8],
    pub(crate) detail: &'static Detail,
    pub(crate) max_h: f32,
    pub(crate) rim: u32,
    pub(crate) board: Board,
}

pub(crate) struct Board {
    pub(crate) x: (f32, f32),
    pub(crate) top: f32,
    pub(crate) pins: (f32, f32),
    pub(crate) contacts_from: f32,
    pub(crate) traces_from: f32,
}

pub fn gb_shell_of(cart: &Cart) -> Option<GbShell> {
    match cart.platform {
        Platform::Gba
        | Platform::Snes
        | Platform::Ps1
        | Platform::Pce
        | Platform::Nes
        | Platform::Md => None,
        Platform::Gb | Platform::Gbc => Some(match cart.shell.map(|c| c.outline) {
            Some(Outline::Notched) => GbShell::Notched,
            Some(Outline::Rounded) => GbShell::Rounded,
            _ => match omega_store::gb::class(&cart.rom) {
                Class::ColourOnly => GbShell::Rounded,
                Class::Original | Class::DualMode => GbShell::Notched,
            },
        }),
    }
}

/// The model `cart` stands on the shelf as.
pub fn model_of(cart: &Cart) -> CartModel {
    match cart.platform {
        Platform::Gba => CartModel::Gba,
        Platform::Gb | Platform::Gbc => {
            CartModel::Gb(gb_shell_of(cart).unwrap_or(GbShell::Notched))
        }
        Platform::Snes => CartModel::Snes(snes::shell_of(cart)),
        Platform::Ps1 => CartModel::Disc,
        Platform::Pce => CartModel::HuCard,
        Platform::Nes => CartModel::Nes(nes::shell_of(cart)),
        Platform::Md => CartModel::Md(md::shell_of(cart)),
    }
}

/// The size of `model`'s face in pixels, the size it is drawn at in the handheld.
pub fn model_box(model: CartModel) -> (u32, u32) {
    let s = spec(model);
    (s.w, s.h)
}

/// `model`'s outline in black, to back a cart that is not drawn as a model.
pub fn model_shadow(model: CartModel) -> CartFace {
    let s = spec(model);
    shadow(s.w, s.h, s.mask)
}

pub(crate) fn spec(model: CartModel) -> Spec {
    match model {
        CartModel::Snes(shell) => snes::spec(shell),
        CartModel::Disc => disc::spec(),
        CartModel::HuCard => hucard::spec(),
        CartModel::Nes(shell) => nes::spec(shell),
        CartModel::Md(shell) => md::spec(shell),
        CartModel::Gba => Spec {
            w: CART_W,
            h: CART_H,
            label: (LABEL_X, LABEL_Y, LABEL_W, LABEL_H),
            label_round: 0,
            mask: cart_mask(),
            depth: cart_depth(),
            detail: detail_mask(),
            max_h: (LABEL_H - 2 * PAD) as f32,
            rim: RIM,
            board: Board {
                x: (0.135, 0.875),
                top: LABEL_Y as f32 / CART_H as f32,
                pins: (0.17, 0.84),
                contacts_from: 0.85,
                traces_from: 0.76,
            },
        },
        CartModel::Gb(shell) => Spec {
            w: GB_CART_W,
            h: GB_CART_H,
            label: (GB_LABEL_X, GB_LABEL_Y, GB_LABEL_W, GB_LABEL_H),
            label_round: 0,
            mask: gb_cart_mask(shell),
            depth: gb_cart_depth(shell),
            detail: gb_detail_mask(shell),
            max_h: (GB_LABEL_H - 2 * PAD) as f32,
            rim: GB_RIM,
            board: Board {
                x: (0.075, 0.925),
                top: 0.07,
                pins: (0.1, 0.9),
                contacts_from: 0.905,
                traces_from: 0.84,
            },
        },
    }
}

pub const SEATED_W: u32 = 240;
const SEATED_H: u32 = 141;
const GB_SEATED_H: u32 = 259;

pub fn seated_box(platform: Platform) -> (u32, u32) {
    match platform {
        Platform::Gba => (SEATED_W, SEATED_H),
        Platform::Md => md::face_box(MdShell::MegaDrive),
        Platform::Nes => nes::face_box(NesShell::Nes),
        Platform::Pce => (hucard::W, hucard::H),
        Platform::Gb | Platform::Gbc => (SEATED_W, GB_SEATED_H),
        Platform::Snes => {
            let (_, (w, h)) = snes::outline(SnesShell::Famicom);
            (w.round() as u32, h.round() as u32)
        }
        Platform::Ps1 => {
            let (_, (w, h)) = crate::spin::outline(CartModel::Disc);
            (w.round() as u32, h.round() as u32)
        }
    }
}

/// The size of a face for `platform` in pixels: its first model's, where it has several.
pub fn cart_box(platform: Platform) -> (u32, u32) {
    model_box(match platform {
        Platform::Gba => CartModel::Gba,
        Platform::Md => CartModel::Md(MdShell::MegaDrive),
        Platform::Pce => CartModel::HuCard,
        Platform::Nes => CartModel::Nes(NesShell::Nes),
        Platform::Gb | Platform::Gbc => CartModel::Gb(GbShell::Notched),
        Platform::Snes => CartModel::Snes(SnesShell::Famicom),
        Platform::Ps1 => CartModel::Disc,
    })
}

pub fn cart_shadow() -> CartFace {
    shadow(CART_W, CART_H, cart_mask())
}

pub fn gb_cart_shadow(shell: GbShell) -> CartFace {
    shadow(GB_CART_W, GB_CART_H, gb_cart_mask(shell))
}

fn shadow(w: u32, h: u32, mask: &[u8]) -> CartFace {
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for cover in mask {
        rgba.extend_from_slice(&[0, 0, 0, *cover]);
    }
    CartFace { rgba, w, h }
}

/// The label scans `cart`'s face is drawn with, each with the size it is drawn at there.
pub(crate) fn label_scans(cart: &Cart) -> Vec<(PathBuf, u32, u32)> {
    let model = model_of(cart);
    if model == CartModel::Disc {
        return disc::label_scans(cart);
    }
    let (_, _, w, h) = spec(model).label;
    cart.label.iter().map(|p| (p.clone(), w, h)).collect()
}

/// The sizes `platform`'s label scans are drawn at, one for each of its models: a Game Boy cart
/// can wear either outline and a SNES cart either shell.
pub fn face_labels(platform: Platform) -> Vec<(u32, u32)> {
    CartModel::ALL
        .into_iter()
        .filter(|model| match model {
            CartModel::Gba => platform == Platform::Gba,
            CartModel::Md(_) => platform == Platform::Md,
            CartModel::Nes(_) => platform == Platform::Nes,
            CartModel::HuCard => platform == Platform::Pce,
            CartModel::Gb(_) => matches!(platform, Platform::Gb | Platform::Gbc),
            CartModel::Snes(_) => platform == Platform::Snes,
            CartModel::Disc => platform == Platform::Ps1,
        })
        .map(|model| match model {
            CartModel::Disc => (disc::DISC_PX, disc::DISC_PX),
            _ => {
                let (_, _, w, h) = spec(model).label;
                (w, h)
            }
        })
        .collect()
}

/// `cart`'s face, its label drawn from the cut kept for its scan, or from the scan itself when
/// there is no cut of it yet.
pub fn cart_face(cart: &Cart) -> CartFace {
    face_of(cart, true)
}

/// `cart`'s face in a generated label whatever scan it has: what a boot draws until the scan
/// is cut.
pub fn generated_face(cart: &Cart) -> CartFace {
    face_of(cart, false)
}

fn face_of(cart: &Cart, scans: bool) -> CartFace {
    let model = model_of(cart);
    if model == CartModel::Disc {
        if let Some(face) = disc::face(cart, scans) {
            return face;
        }
    }
    let s = spec(model);
    let shell = shell_for(cart);
    let mut face = shell_face(&s, &shell);
    let (_, _, lw, lh) = s.label;
    let scan = cart.label.as_deref().filter(|_| scans);
    let label = match scan.and_then(|p| label_art(p, lw, lh)) {
        Some(rgba) => rgba,
        None => generated_label(&s, &label_text(cart)),
    };
    mould_detail(&s, &mut face);
    recess_label(&s, &mut face, &shell);
    paste_label(&s, &mut face, &label);
    clip_to_silhouette(&s, &mut face);
    face
}

fn clip_to_silhouette(s: &Spec, face: &mut CartFace) {
    for (px, cover) in face.rgba.chunks_exact_mut(4).zip(s.mask) {
        px[3] = ((px[3] as u32 * *cover as u32 + 127) / 255) as u8;
    }
}

pub fn label_colour(title: &str) -> [u8; 3] {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in title.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hsv_to_rgb((h % 360) as f32, 0.52, 0.74)
}

pub fn label_text(cart: &Cart) -> String {
    clean_label(&cart.stem)
}

pub fn clean_label(stem: &str) -> String {
    let mut bare = String::with_capacity(stem.len());
    let mut depth = 0u32;
    for ch in stem.chars() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => bare.push(ch),
            _ => {}
        }
    }

    let mut out = String::with_capacity(bare.len());
    for word in bare.split_whitespace().filter(|w| *w != "-") {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    if out.is_empty() {
        stem.to_string()
    } else {
        out
    }
}

pub fn label_tags(stem: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0u32;
    let mut cur = String::new();
    for ch in stem.chars() {
        match ch {
            '(' | '[' => {
                depth += 1;
                if depth == 1 {
                    cur.clear();
                    continue;
                }
            }
            ')' | ']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let t = cur.trim();
                    if !t.is_empty() {
                        out.push(t.to_string());
                    }
                    continue;
                }
            }
            _ => {}
        }
        if depth >= 1 {
            cur.push(ch);
        }
    }
    out
}

fn shell_face(s: &Spec, shell: &Shell) -> CartFace {
    let mut rgba = Vec::with_capacity((s.w * s.h * 4) as usize);
    let edge = rim_colour(through(shell.colour, shell.colour));
    for (i, depth) in s.depth.iter().enumerate() {
        let (x, y) = (i as u32 % s.w, i as u32 / s.w);
        let c = match shell.finish {
            Finish::Solid => shell.colour,
            Finish::Translucent | Finish::Glitter => {
                let body = through(shell.colour, inside(s, x, y).unwrap_or(shell.colour));
                let c = lerp(edge, body, (*depth as u32).min(s.rim), s.rim);
                if shell.finish == Finish::Glitter && fleck(x, y) {
                    lerp(c, [0xff; 3], 100, 255)
                } else {
                    c
                }
            }
        };
        rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
    }
    CartFace {
        rgba,
        w: s.w,
        h: s.h,
    }
}

const SURFACE: u32 = 150;

const BOARD: [u8; 3] = [0x2c, 0x96, 0x52];
const TRACE: [u8; 3] = [0x5a, 0xb4, 0x74];
const GOLD: [u8; 3] = [0xe6, 0xb4, 0x46];

fn through(plastic: [u8; 3], behind: [u8; 3]) -> [u8; 3] {
    let filter =
        |a: [u8; 3], b: [u8; 3]| [0, 1, 2].map(|c| (a[c] as u32 * b[c] as u32 / 255) as u8);
    lerp(
        filter(plastic, behind),
        filter(plastic, plastic),
        SURFACE,
        255,
    )
}

fn inside(s: &Spec, x: u32, y: u32) -> Option<[u8; 3]> {
    let b = &s.board;
    let (fx, fy) = (x as f32 / s.w as f32, y as f32 / s.h as f32);
    if !(b.x.0..b.x.1).contains(&fx) || fy < b.top {
        return None;
    }
    let (c0, c1) = b.pins;
    let pitch = (c1 - c0) / 32.0;
    let on_contact = (c0..c1).contains(&fx) && ((fx - c0) % pitch) < pitch * 0.6;
    if on_contact && fy >= b.contacts_from {
        return Some(GOLD);
    }
    if on_contact && ((fx - c0) % pitch) < pitch * 0.2 && fy >= b.traces_from {
        Some(TRACE)
    } else {
        Some(BOARD)
    }
}

fn fleck(x: u32, y: u32) -> bool {
    let mut h = x.wrapping_mul(0x9e37_79b9) ^ y.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h.is_multiple_of(100)
}

fn rim_colour(base: [u8; 3]) -> [u8; 3] {
    let mean = (base[0] as u16 + base[1] as u16 + base[2] as u16) / 3;
    base.map(|c| {
        let grey = (3 * c as u16 + mean) / 4;
        (grey + (255 - grey) * 2 / 5) as u8
    })
}

fn lerp(a: [u8; 3], b: [u8; 3], num: u32, den: u32) -> [u8; 3] {
    let mut out = [0u8; 3];
    for c in 0..3 {
        out[c] = ((a[c] as u32 * (den - num) + b[c] as u32 * num) / den) as u8;
    }
    out
}

const BEVEL: u32 = 3;

fn mould_detail(s: &Spec, face: &mut CartFace) {
    let mix = |px: &mut [u8], to: [u8; 3], a: u32| {
        for c in 0..3 {
            px[c] = ((to[c] as u32 * a + px[c] as u32 * (255 - a) + 127) / 255) as u8;
        }
    };
    let sides = s.detail.shadow.iter().zip(&s.detail.highlight);
    for (px, (shade, light)) in face.rgba.chunks_exact_mut(4).zip(sides) {
        let under = [px[0], px[1], px[2]];
        if *shade > 0 {
            mix(px, under.map(|c| (c as f32 * 0.62) as u8), *shade as u32);
        }
        if *light > 0 {
            mix(
                px,
                under.map(|c| c + ((255 - c) as f32 * 0.24) as u8),
                *light as u32,
            );
        }
    }
}

fn recess_label(s: &Spec, face: &mut CartFace, shell: &Shell) {
    let shade = |c: [u8; 3], f: f32| -> [u8; 3] {
        [
            (c[0] as f32 * f).clamp(0.0, 255.0) as u8,
            (c[1] as f32 * f).clamp(0.0, 255.0) as u8,
            (c[2] as f32 * f).clamp(0.0, 255.0) as u8,
        ]
    };
    let dark = shade(shell.colour, 0.55);
    let lit = shade(shell.colour, 1.45);

    let (lx, ly, lw, lh) = s.label;
    let (w, h) = (s.w, s.h);
    let bevel = (BEVEL * w + SEATED_W / 2) / SEATED_W;
    let (x0, y0) = (lx - bevel, ly - bevel);
    let (x1, y1) = (lx + lw + bevel, ly + lh + bevel);
    let clear = shell.finish != Finish::Solid;
    let mut put = |x: u32, y: u32, upper: bool| {
        if x >= w || y >= h {
            return;
        }
        let d = ((y * w + x) * 4) as usize;
        let c = if clear {
            let under = [face.rgba[d], face.rgba[d + 1], face.rgba[d + 2]];
            shade(under, if upper { 0.7 } else { 1.18 })
        } else if upper {
            dark
        } else {
            lit
        };
        face.rgba[d] = c[0];
        face.rgba[d + 1] = c[1];
        face.rgba[d + 2] = c[2];
    };
    let round = s.label_round;
    for y in y0..y1 {
        for x in x0..x1 {
            let inside = match round {
                0 => (lx..lx + lw).contains(&x) && (ly..ly + lh).contains(&y),
                _ => in_round_rect(x, y, s.label, round),
            };
            if inside {
                continue;
            }
            // a round label's well is round with it, its corners as far out as its sides
            if round > 0 && !in_round_rect(x, y, (x0, y0, x1 - x0, y1 - y0), round + bevel) {
                continue;
            }
            let from_top = y.saturating_sub(y0);
            let from_left = x.saturating_sub(x0);
            let from_bottom = y1.saturating_sub(y + 1);
            let from_right = x1.saturating_sub(x + 1);
            let upper = from_top.min(from_left);
            let lower = from_bottom.min(from_right);
            put(x, y, upper <= lower);
        }
    }
}

/// Whether the face's pixel `x, y` is inside the rect `rx, ry, rw, rh` with its corners
/// rounded `r` pixels: decided for each two-pixel cell of the face as a whole, so a round
/// corner steps on the art grid as everything else does.
fn in_round_rect(x: u32, y: u32, (rx, ry, rw, rh): (u32, u32, u32, u32), r: u32) -> bool {
    if !(rx..rx + rw).contains(&x) || !(ry..ry + rh).contains(&y) {
        return false;
    }
    let (cx, cy) = ((x & !1) as f32 + 1.0, (y & !1) as f32 + 1.0);
    let r = r as f32;
    let dx = (rx as f32 + r - cx).max(cx - (rx + rw) as f32 + r).max(0.0);
    let dy = (ry as f32 + r - cy).max(cy - (ry + rh) as f32 + r).max(0.0);
    dx * dx + dy * dy <= r * r
}

fn paste_label(s: &Spec, face: &mut CartFace, label: &[u8]) {
    let (lx, ly, lw, lh) = s.label;
    for y in 0..lh {
        for x in 0..lw {
            let src = ((y * lw + x) * 4) as usize;
            let a = label[src + 3] as u32;
            if a == 0 {
                continue;
            }
            if s.label_round > 0 && !in_round_rect(x + lx, y + ly, s.label, s.label_round) {
                continue;
            }
            let d = (((y + ly) * s.w + x + lx) * 4) as usize;
            for c in 0..3 {
                face.rgba[d + c] =
                    ((label[src + c] as u32 * a + face.rgba[d + c] as u32 * (255 - a) + 127) / 255)
                        as u8;
            }
        }
    }
}

fn generated_label(s: &Spec, title: &str) -> Vec<u8> {
    let (_, _, lw, lh) = s.label;
    let bg = label_colour(title);
    let mut rgba = Vec::with_capacity((lw * lh * 4) as usize);
    for _ in 0..lw * lh {
        rgba.extend_from_slice(&[bg[0], bg[1], bg[2], 255]);
    }

    if let Some(font) = text::label_font() {
        let layout = text::fit_box(
            font,
            title,
            (lw - 2 * PAD) as f32,
            s.max_h,
            MAX_LINES,
            MAX_PX,
            MIN_PX,
        );
        text::draw_centred(&mut rgba, lw, lh, &layout, ink(bg));
    }
    rgba
}

fn ink(bg: [u8; 3]) -> [u8; 3] {
    let luma = 0.2126 * bg[0] as f32 + 0.7152 * bg[1] as f32 + 0.0722 * bg[2] as f32;
    if luma > 140.0 {
        [0x1a, 0x18, 0x16]
    } else {
        [0xf4, 0xf1, 0xea]
    }
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [u8; 3] {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
    ]
}
