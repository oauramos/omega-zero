//! The core picker: the cart opened over the shelf, its lid lifted off the board, and the
//! board drawn as pixel art. The back of the shell is the cart's own plastic, the board sits in
//! it with the ROM chip marked with the game's title, and two sockets are printed with the
//! cores' names, one of them holding the chip.
//!
//! Every face here is drawn at art resolution, one texel to an art pixel, in a few flat colours
//! with outlines and no anti-aliasing: a shade is a flat band or a checker dither. They are
//! uploaded with nearest filtering and drawn at `SPRITE_PX` (2x), so at rest every edge lands on
//! the panel's 2px grid. Sizes and places on the board are in art pixels.

use crate::stage::{stage_x, stage_y};
use omega_gfx::out_w;
use omega_store::{Cart, Core, Platform};

use crate::cart::{clean_label, model_box, CartFace, CART_H, CART_W};
use crate::kit::pixel::{blank, fill, ink, letters, ACCENT, ADVANCE, CAP, LINE};
use crate::models::snes::{self, SnesShell};
use crate::motion::ease;
use crate::plate::UndoFace;
use crate::shelf::{foot_y, model_scale};
use crate::shell::shell_for;
use crate::spin::CartModel;
use crate::sprite::SPRITE_PX;

/// The board's face, the back of the shell and the board in it.
pub const BOARD_W: u32 = 186;
pub const BOARD_H: u32 = 104;

/// The ROM chip's body on the board, which the marking is printed over.
pub const ROM_X: u32 = 17;
pub const ROM_Y: u32 = 24;
pub const ROM_W: u32 = 58;
pub const ROM_H: u32 = 44;

type Rgb = [u8; 3];

const _: () = assert!(ROM_X + ROM_W <= BOARD_W && ROM_Y + ROM_H <= BOARD_H);

/// The board's own colours: green board, gold contacts, black chips on silver pins, white
/// print. The shell's come from the cart.
const PCB: Rgb = [0x3a, 0x9a, 0x3c];
const PCB_EDGE: Rgb = [0x2e, 0x7f, 0x30];
/// A trace, copper under the board's lacquer: a lighter green.
const TRACE: Rgb = [0x58, 0xb3, 0x53];
const VIA: Rgb = [0x74, 0xc4, 0x66];
const HOLE: Rgb = [0x1a, 0x3d, 0x1a];
const SILK: Rgb = [0xee, 0xf5, 0xe6];
const GOLD: Rgb = [0xd4, 0xa9, 0x3c];
const GOLD_DARK: Rgb = [0xa6, 0x80, 0x2a];
const BODY: Rgb = [0x1a, 0x1a, 0x1a];
const BODY_HI: Rgb = [0x33, 0x33, 0x33];
const PIN: Rgb = [0xb3, 0xb3, 0xb8];
const PIN_DARK: Rgb = [0x7d, 0x7d, 0x84];
const PART: Rgb = [0xb8, 0x90, 0x5e];
const PART_END: Rgb = [0xcf, 0xd0, 0xd4];
const CELL: Rgb = [0xbf, 0xc0, 0xc6];
const CELL_RING: Rgb = [0xa5, 0xa6, 0xac];
const CELL_EDGE: Rgb = [0x8c, 0x8d, 0x93];

/// The ROM's marking: as many letters to a line as its body holds at one face pixel to an
/// art pixel, three lines at most.
const MARK_LINE_CHARS: usize = 9;
const MARK_LINES: usize = 3;
const MARK_INK: Rgb = [0xbd, 0xbd, 0xbd];

/// The rows of a line of Pixel Code above its capitals, and the capitals themselves.
const ACCENT_ROWS: i32 = ACCENT as i32;
const CAP_ROWS: u32 = CAP as u32;

pub fn rom_marking(stem: &str) -> Vec<String> {
    let mut title: Vec<String> = Vec::new();
    for word in clean_label(stem).to_uppercase().split_whitespace() {
        match title.last_mut() {
            Some(line) if line.len() + 1 + word.len() <= MARK_LINE_CHARS => {
                line.push(' ');
                line.push_str(word);
            }
            _ => title.push(word.to_string()),
        }
    }
    if title.len() > MARK_LINES {
        let tail = title.split_off(MARK_LINES - 1).join(" ");
        title.push(tail);
    }
    title
}

/// What of a marking line fits on the chip: its words while they fit, else its first
/// `MARK_LINE_CHARS` letters.
fn marked(line: &str) -> String {
    let mut kept = String::new();
    for word in line.split(' ') {
        let longer = match kept.is_empty() {
            true => word.to_string(),
            false => format!("{kept} {word}"),
        };
        if longer.chars().count() > MARK_LINE_CHARS {
            break;
        }
        kept = longer;
    }
    match kept.is_empty() {
        true => line.chars().take(MARK_LINE_CHARS).collect(),
        false => kept,
    }
}

/// The title printed on the ROM chip: its lines in Pixel Code, one face pixel to an art
/// pixel, each centred across the chip, and the block of them centred down it.
pub fn rom_marking_face(stem: &str) -> CartFace {
    let title: Vec<String> = rom_marking(stem).iter().map(|l| marked(l)).collect();
    let mut face = blank(ROM_W, ROM_H);
    let tall = (title.len() as u32 * LINE).saturating_sub(LINE - CAP_ROWS);
    let mut top = ROM_H.saturating_sub(tall) as i32 / 2 - ACCENT_ROWS;
    for line in &title {
        let x = (ROM_W as i32 - ink_width(line)) / 2;
        ink(&mut face, &letters(line, ADVANCE), x, top, MARK_INK);
        top += LINE as i32;
    }
    cart_face(face)
}

/// How wide `line`'s ink is in Pixel Code at one face pixel to an art pixel: its letters but
/// the blank column after the last.
fn ink_width(line: &str) -> i32 {
    (line.chars().count() * ADVANCE) as i32 - 1
}

/// The colours of a shell's plastic: the plastic itself, the floor inside it, its walls in
/// shade, and the deepest shade, for outlines and holes.
struct Plastic {
    face: Rgb,
    floor: Rgb,
    wall: Rgb,
    wall_deep: Rgb,
    deep: Rgb,
}

impl Plastic {
    fn of(colour: Rgb) -> Self {
        Plastic {
            face: colour,
            floor: shade(colour, 0.62),
            wall: shade(colour, 0.52),
            wall_deep: shade(colour, 0.44),
            deep: shade(colour, 0.35),
        }
    }
}

/// The open cart: the back of its shell, the board in it, the ROM chip marked with the game's
/// title. A SNES cart opens on a SNES board in its own shell.
pub fn board_face(cart: &Cart) -> CartFace {
    let plastic = Plastic::of(shell_for(cart).colour);
    let mut face = blank(BOARD_W, BOARD_H);
    if cart.platform == Platform::Snes {
        snes_board(&mut face, &plastic, snes::shell_of(cart));
    } else {
        shell(&mut face, &plastic);
        pcb(&mut face, &plastic);
        rom_chip(&mut face);
        parts(&mut face);
        latches(&mut face, &plastic);
    }
    let mut face = cart_face(face);
    over(&mut face, &rom_marking_face(&cart.stem), ROM_X, ROM_Y);
    face
}

/// How far in from each side row `y` of a shape `h` tall is, its top corners rounded by
/// `top` and its bottom ones by `bottom` (the insets of each corner's rows, from its end).
fn rounded(y: i32, h: i32, top: &[i32], bottom: &[i32]) -> i32 {
    let from_top = top.get(y as usize).copied().unwrap_or(0);
    let from_bottom = bottom.get((h - 1 - y) as usize).copied().unwrap_or(0);
    from_top.max(from_bottom)
}

/// The back of the shell: its ears across the top, as wide as the board, the body under them
/// a little narrower; the floor inside, its rim three pixels wide; the walls on the top and
/// left of it in shade, as the light falls from the top right.
fn shell(face: &mut UndoFace, p: &Plastic) {
    const EAR_H: i32 = 22;
    const BODY_IN: i32 = 4;
    const RIM: i32 = 3;
    let (w, h) = (BOARD_W as i32, BOARD_H as i32);
    let side = |y: i32| match y < EAR_H {
        true => rounded(y, EAR_H, &[6, 4, 2, 1, 1], &[1]),
        false => BODY_IN + rounded(y - EAR_H, h - EAR_H, &[], &[3, 1, 1]),
    };
    let inside = |x: i32, y: i32| (0..h).contains(&y) && x >= side(y) && x < w - side(y);
    // the plastic, outlined in its deepest shade wherever it meets the air
    for y in 0..h {
        for x in side(y)..w - side(y) {
            let edge = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .any(|(dx, dy)| !inside(x + dx, y + dy));
            dot(face, x, y, if edge { p.deep } else { p.face });
        }
    }
    let (ear_h, body_h) = (EAR_H - RIM, h - RIM);
    for y in RIM..body_h {
        let side = match y < ear_h {
            true => RIM + rounded(y - RIM, ear_h - RIM, &[4, 2, 1], &[]),
            false => BODY_IN + RIM + rounded(y - ear_h, body_h - ear_h, &[], &[2, 1]),
        };
        let shade = match y - RIM {
            0 | 1 => p.wall_deep,
            2..=4 => p.wall,
            _ => p.floor,
        };
        fill(face, [side, y, w - 2 * side, 1], shade);
        if y - RIM > 4 {
            fill(face, [side, y, 3, 1], p.wall);
        }
    }
}

/// The board's rect inside the shell.
const PCB_AT: [i32; 4] = [12, 20, 162, 76];

/// The board itself: green, with a darker edge and its corners cut, a checker of shadow on the
/// floor under its right and bottom edges, the post the shell's screw goes through biting into
/// its top, its traces, and the gold contacts along its bottom edge.
fn pcb(face: &mut UndoFace, p: &Plastic) {
    let [x, y, w, h] = PCB_AT;
    checker(face, [x + 2, y + h, w, 2], p.deep);
    checker(face, [x + w, y + 2, 2, h], p.deep);
    fill(face, [x, y, w, h], PCB_EDGE);
    fill(face, [x + 1, y + 1, w - 2, h - 2], PCB);
    for (cx, cy) in [
        (x, y),
        (x + w - 1, y),
        (x, y + h - 1),
        (x + w - 1, y + h - 1),
    ] {
        dot(face, cx, cy, p.floor);
    }

    // the screw post, in the floor's colour where it bites the board, the board's edge round it
    let (bx, by) = (93, 20);
    disc_where(face, bx, by, 9, PCB_EDGE, |_, py| py > y);
    disc(face, bx, by, 8, p.floor);
    disc(face, bx, by, 6, p.deep);
    disc(face, bx, by, 5, p.face);
    disc(face, bx - 1, by - 1, 2, lighter(p.face));
    disc(face, bx, by, 2, p.deep);

    for trace in TRACES {
        path(face, trace, TRACE);
    }
    for &(x, y) in VIAS {
        via(face, x, y);
    }

    // thirty contacts, three pixels of gold and two of board, down to the board's edge
    for n in 0..30 {
        let x = 19 + n * 5;
        fill(face, [x, 78, 3, 18], GOLD);
        fill(face, [x + 2, 79, 1, 17], GOLD_DARK);
        dot(face, x, 78, PCB);
        dot(face, x + 2, 78, PCB);
    }
}

/// The traces, as polylines from point to point, each leg across, down or at 45 degrees.
const TRACES: &[&[(i32, i32)]] = &[
    // a bus from the ROM's pins over to the first socket's
    &[(78, 35), (84, 35), (87, 38), (87, 41)],
    &[(78, 32), (86, 32), (90, 36), (90, 41)],
    &[(78, 29), (88, 29), (93, 34), (93, 41)],
    // down past the ROM to the contacts
    &[(78, 61), (79, 62), (79, 77)],
    &[(78, 57), (80, 57), (82, 59), (82, 66), (85, 69), (85, 77)],
    // from the small chip down to the second socket
    &[(127, 38), (128, 39), (138, 39), (139, 40), (139, 41)],
    &[(133, 38), (134, 37), (139, 37), (141, 39), (141, 41)],
    // from the cell to the contacts on the right edge
    &[(160, 30), (165, 30), (170, 35), (170, 51)],
    // the sockets' pins down to the contacts
    &[(90, 67), (90, 77)],
    &[(95, 67), (95, 77)],
    &[(100, 67), (100, 77)],
    &[(105, 67), (105, 77)],
    &[(110, 67), (110, 77)],
    &[(140, 67), (140, 77)],
    &[(145, 67), (145, 77)],
    &[(150, 67), (150, 77)],
    &[(155, 67), (155, 77)],
    &[(160, 67), (160, 77)],
    &[(165, 67), (165, 77)],
    // the transistors between the sockets
    &[(120, 72), (120, 77)],
    &[(126, 72), (126, 77)],
    &[(132, 72), (132, 77)],
];

const VIAS: &[(i32, i32)] = &[(107, 23), (126, 52), (59, 72), (22, 72)];

/// The ROM chip on the left of the board: a black body with a pin's mark at its corner and a
/// row of pins down each side.
fn rom_chip(face: &mut UndoFace) {
    let [x, y, w, h] = [ROM_X as i32, ROM_Y as i32, ROM_W as i32, ROM_H as i32];
    for n in 0..20 {
        let py = y + 3 + n * 2;
        fill(face, [x - 2, py, 2, 1], PIN);
        fill(face, [x + w, py, 2, 1], PIN);
    }
    body(face, [x, y, w, h]);
    fill(face, [x + 3, y + h - 5, 2, 2], BODY_HI);
}

/// A chip's black body, its corners cut and its top edge catching the light.
fn body(face: &mut UndoFace, [x, y, w, h]: [i32; 4]) {
    fill(face, [x + 1, y, w - 2, h], BODY);
    fill(face, [x, y + 1, w, h - 2], BODY);
    fill(face, [x + 1, y, w - 2, 1], BODY_HI);
}

/// What else is on the board: the small chip and the cell at the top right, a pair of
/// capacitors in their printed outlines, the three transistors between the sockets, and four
/// contacts on the right edge.
fn parts(face: &mut UndoFace) {
    // the small chip, its pins above and below
    for n in 0..8 {
        let x = 112 + n * 3;
        fill(face, [x, 23, 1, 2], PIN);
        fill(face, [x, 35, 1, 2], PIN);
    }
    body(face, [110, 25, 24, 10]);
    dot(face, 112, 32, BODY_HI);

    // the cell
    disc(face, 152, 30, 7, CELL_EDGE);
    disc(face, 152, 30, 6, CELL);
    ring(face, 152, 30, 4, CELL_RING);
    dot(face, 149, 26, SILK);
    dot(face, 150, 26, SILK);
    dot(face, 148, 27, SILK);

    capacitor(face, 99, 32);
    capacitor(face, 38, 71);

    // three transistors, a lead above and two below
    for n in 0..3 {
        let x = 118 + n * 6;
        dot(face, x + 2, 68, PIN);
        fill(face, [x + 1, 69, 3, 2], BODY);
        dot(face, x + 1, 71, PIN);
        dot(face, x + 3, 71, PIN);
    }

    for n in 0..4 {
        fill(face, [171, 54 + n * 4, 3, 1], GOLD);
    }
}

/// A capacitor lying across in its dashed outline: a tan body between silver ends.
fn capacitor(face: &mut UndoFace, x: i32, y: i32) {
    dashed(face, [x - 2, y - 2, 10, 7]);
    fill(face, [x, y, 6, 3], PART);
    fill(face, [x, y, 1, 3], PART_END);
    fill(face, [x + 5, y, 1, 3], PART_END);
}

/// A printed outline round a part, every other pixel of it inked.
fn dashed(face: &mut UndoFace, [x, y, w, h]: [i32; 4]) {
    for i in 0..w {
        if i % 2 == 0 {
            dot(face, x + i, y, SILK);
            dot(face, x + i, y + h - 1, SILK);
        }
    }
    for j in 0..h {
        if j % 2 == 0 {
            dot(face, x, y + j, SILK);
            dot(face, x + w - 1, y + j, SILK);
        }
    }
}

/// A via: a ring of bare copper with the hole through the board in it.
fn via(face: &mut UndoFace, x: i32, y: i32) {
    fill(face, [x - 1, y - 1, 3, 3], VIA);
    dot(face, x, y, HOLE);
}

/// The shell's two clips over the board's sides.
fn latches(face: &mut UndoFace, p: &Plastic) {
    for x in [8, BOARD_W as i32 - 14] {
        fill(face, [x, 43, 6, 8], p.deep);
        fill(face, [x + 1, 44, 4, 6], p.face);
        fill(face, [x + 1, 44, 4, 1], lighter(p.face));
    }
}

/// `face`'s pixel at `x, y` in `c`.
fn dot(face: &mut UndoFace, x: i32, y: i32, c: Rgb) {
    fill(face, [x, y, 1, 1], c);
}

/// Every other pixel of the box in `c`, a checker that starts inked at its top left: the
/// dither the 16-bit consoles drew a half shade and a shadow with.
fn checker(face: &mut UndoFace, [x, y, w, h]: [i32; 4], c: Rgb) {
    for j in 0..h {
        for i in 0..w {
            if (i + j) % 2 == 0 {
                dot(face, x + i, y + j, c);
            }
        }
    }
}

/// Whether the pixel `dx, dy` from a circle's centre is in it: a pixel circle `r` across from
/// its centre, round enough at every size without an anti-aliased edge.
fn in_circle(dx: i32, dy: i32, r: i32) -> bool {
    dx * dx + dy * dy <= r * r + r
}

fn disc(face: &mut UndoFace, cx: i32, cy: i32, r: i32, c: Rgb) {
    disc_where(face, cx, cy, r, c, |_, _| true);
}

/// The pixels of a disc that `keep` keeps.
fn disc_where(
    face: &mut UndoFace,
    cx: i32,
    cy: i32,
    r: i32,
    c: Rgb,
    keep: impl Fn(i32, i32) -> bool,
) {
    for dy in -r..=r {
        for dx in -r..=r {
            if in_circle(dx, dy, r) && keep(cx + dx, cy + dy) {
                dot(face, cx + dx, cy + dy, c);
            }
        }
    }
}

/// The edge of a disc, one pixel wide.
fn ring(face: &mut UndoFace, cx: i32, cy: i32, r: i32, c: Rgb) {
    for dy in -r..=r {
        for dx in -r..=r {
            let inside = |dx: i32, dy: i32| in_circle(dx, dy, r);
            let edge = !inside(dx + 1, dy)
                || !inside(dx - 1, dy)
                || !inside(dx, dy + 1)
                || !inside(dx, dy - 1);
            if inside(dx, dy) && edge {
                dot(face, cx + dx, cy + dy, c);
            }
        }
    }
}

/// A one-pixel line through `points`, each leg across, down or at 45 degrees.
fn path(face: &mut UndoFace, points: &[(i32, i32)], c: Rgb) {
    for leg in points.windows(2) {
        let ((x0, y0), (x1, y1)) = (leg[0], leg[1]);
        let steps = (x1 - x0).abs().max((y1 - y0).abs());
        for s in 0..=steps {
            dot(
                face,
                x0 + (x1 - x0).signum() * s,
                y0 + (y1 - y0).signum() * s,
                c,
            );
        }
    }
}

fn shade(c: Rgb, f: f32) -> Rgb {
    c.map(|v| (v as f32 * f).round().clamp(0.0, 255.0) as u8)
}

/// `c` a quarter of the way to white: where light catches the plastic.
fn lighter(c: Rgb) -> Rgb {
    c.map(|v| v + (255 - v) / 4)
}

fn cart_face(face: UndoFace) -> CartFace {
    CartFace {
        rgba: face.rgba,
        w: face.w,
        h: face.h,
    }
}

fn over(dst: &mut CartFace, src: &CartFace, x: u32, y: u32) {
    for row in 0..src.h {
        for col in 0..src.w {
            let s = ((row * src.w + col) * 4) as usize;
            let (dx, dy) = (x + col, y + row);
            if src.rgba[s + 3] == 0 || dx >= dst.w || dy >= dst.h {
                continue;
            }
            let d = ((dy * dst.w + dx) * 4) as usize;
            dst.rgba[d..d + 4].copy_from_slice(&src.rgba[s..s + 4]);
        }
    }
}

pub fn board_x() -> f32 {
    174.0 + stage_x()
}

/// The open board stands between the lid resting above it and the legend under it, all of it
/// between the bar across the top and the bar across the bottom.
pub fn board_y() -> f32 {
    140.0 + stage_y()
}

pub const TURN_PAD: u32 = 2;

pub const LID_TURN: f32 = -5.0 * std::f32::consts::PI / 180.0;

/// How high a chip hops between the sockets, in art pixels: it rises a whole pixel at a time.
pub const HOP_LIFT: f32 = 8.0;

pub const SLIDE_SHARE: f32 = 160.0 / 420.0;
/// How far a GBA cart slides up off the board before its lid lifts away: short of where the lid
/// comes to rest, so the lift still carries it up. A quarter would take it past the lid's rest.
pub const SLIDE_UP: f32 = CART_H as f32 / 5.0;

/// How far a cart of `model` slides up off the board before its lid lifts away: a fifth of its
/// height, as a GBA cart's `SLIDE_UP` is.
pub fn slide_up(model: CartModel) -> f32 {
    model_box(model).1 as f32 / 5.0
}

pub fn slide_of(progress: f32) -> f32 {
    ease((progress / SLIDE_SHARE).clamp(0.0, 1.0))
}

pub fn lift_of(progress: f32) -> f32 {
    ease(((progress - SLIDE_SHARE) / (1.0 - SLIDE_SHARE)).clamp(0.0, 1.0))
}

/// The GBA board's sockets' top left corners, mGBA's then gpSP's.
pub const SOCKET_U: [f32; 2] = [84.0, 136.0];
pub const SOCKET_V: f32 = 42.0;
pub const SOCKET_W: u32 = 32;
pub const SOCKET_H: u32 = 24;

/// The chip's top left corner seated in each of the GBA board's sockets: a pixel in from the
/// socket's.
pub const CHIP_U: [f32; 2] = [SOCKET_U[0] + 1.0, SOCKET_U[1] + 1.0];
pub const CHIP_V: f32 = SOCKET_V + 1.0;
pub const CHIP_W: u32 = 30;
pub const CHIP_H: u32 = 22;

/// The shadow under a chip in the air, and where its top left is from the chip's at rest: on
/// the board round the chip's foot, two pixels wider than the chip either side.
pub const SHADOW_W: u32 = CHIP_W + 4;
pub const SHADOW_H: u32 = 6;
pub const SHADOW_AT: [f32; 2] = [-2.0, 18.0];

/// The shadow under the lid where it rests over the board.
pub const LID_SHADOW_W: u32 = 84;
pub const LID_SHADOW_H: u32 = 10;

/// Where a board's two sockets are and how big they and the chip are, in art pixels: the GBA's,
/// whose cores' names are four letters, or the SNES's, wider for six.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Sockets {
    /// The sockets' top left corners across the board, in the order the platform lists its
    /// cores, the default first.
    pub u: [f32; 2],
    pub v: f32,
    pub w: u32,
    pub h: u32,
    pub chip_w: u32,
    pub chip_h: u32,
}

impl Sockets {
    /// The chip's top left corner seated in socket `slot`: a pixel in from the socket's.
    pub fn chip_u(&self, slot: usize) -> f32 {
        self.u[slot] + 1.0
    }

    pub fn chip_v(&self) -> f32 {
        self.v + 1.0
    }

    /// The shadow under the chip in the air: two pixels wider than the chip either side.
    pub fn shadow_w(&self) -> u32 {
        self.chip_w + 4
    }
}

/// The GBA board's sockets, mGBA's then gpSP's.
pub const GBA_SOCKETS: Sockets = Sockets {
    u: SOCKET_U,
    v: SOCKET_V,
    w: SOCKET_W,
    h: SOCKET_H,
    chip_w: CHIP_W,
    chip_h: CHIP_H,
};

/// The SNES board's sockets, Snes9x's then Supafaust's: as tall as the GBA's and a dozen pixels
/// wider, so a six-letter name has the room on its chip a four-letter one has on the GBA's,
/// four pixels before it and three after.
pub const SNES_SOCKETS: Sockets = Sockets {
    u: [78.0, 126.0],
    v: SOCKET_V,
    w: SOCKET_W + 12,
    h: SOCKET_H,
    chip_w: CHIP_W + 12,
    chip_h: CHIP_H,
};

/// The sockets on the board of a `platform` cart.
pub fn sockets_for(platform: Platform) -> Sockets {
    match platform {
        Platform::Snes => SNES_SOCKETS,
        Platform::Gba
        | Platform::Gb
        | Platform::Gbc
        | Platform::Ps1
        | Platform::Pce
        | Platform::Nes
        | Platform::Md => GBA_SOCKETS,
    }
}

/// The sockets `core` is seated among: those of the first system it runs.
pub fn sockets_of(core: Core) -> Sockets {
    match core {
        Core::Snes9x | Core::Supafaust => SNES_SOCKETS,
        Core::Mgba
        | Core::Gpsp
        | Core::PcsxRearmed
        | Core::PceFast
        | Core::Fceumm
        | Core::GenesisPlusGx => GBA_SOCKETS,
    }
}

/// How far down a chip its name's capitals start, under the accent rows of its line.
const NAME_Y: i32 = 7;
const CHIP_INK: Rgb = [0xf2, 0xf2, 0xf2];
/// The socket's name, printed in the silkscreen at half strength over the board: half of each,
/// flattened.
const SOCKET_INK: Rgb = [0x94, 0xc7, 0x91];

/// Where `name` starts across a chip `w` wide: in the middle, its odd pixel on the left.
fn name_x(name: &str, w: u32) -> i32 {
    (w as i32 - ink_width(name) + 1) / 2
}

/// Where the GBA cart's lid rests over the board: across the board's middle, its foot a little
/// above the board's top. A cart of another shape keeps the foot and the middle.
const LID_REST: Placed = Placed {
    x: 288.0,
    y: 44.0,
    w: 144.0,
    h: 81.0,
};

/// Where the lid of a cart of `model` rests: the GBA cart's lid scaled as the cart is to a GBA
/// cart, and shrunk to the GBA lid's height where it would stand taller.
fn lid_rest_of(model: CartModel) -> Placed {
    let (mw, mh) = model_box(model);
    let w = LID_REST.w * mw as f32 / CART_W as f32;
    let h = LID_REST.h * mh as f32 / CART_H as f32;
    let fit = (LID_REST.h / h).min(1.0);
    let (w, h) = (w * fit, h * fit);
    Placed {
        x: LID_REST.x + (LID_REST.w - w) / 2.0 + stage_x(),
        y: LID_REST.y + LID_REST.h - h + stage_y(),
        w,
        h,
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Placed {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

pub fn shelf_cart() -> Placed {
    shelf_cart_at((out_w() - CART_W) as f32 / 2.0, 1.0)
}

pub fn shelf_cart_at(x: f32, scale: f32) -> Placed {
    model_cart_at(CartModel::Gba, x, scale)
}

/// Where a cart of `model` stands on the shelf with its left edge at `x`, drawn `scale` of its
/// own size (as `Shelf::selected_at` says): on the floor the shelf stands it on.
pub fn model_cart_at(model: CartModel, x: f32, scale: f32) -> Placed {
    let (mw, mh) = model_box(model);
    let (w, h) = (mw as f32 * scale, mh as f32 * scale);
    Placed {
        x,
        y: foot_y(mh as f32 * model_scale(model)) - h,
        w,
        h,
    }
}

pub fn board_at(progress: f32) -> Placed {
    board_from(shelf_cart(), progress)
}

/// Where the board is drawn: from the shelf cart's place to its rest, where it is its face at
/// `SPRITE_PX` to a texel on whole art pixels.
pub fn board_from(shelf: Placed, progress: f32) -> Placed {
    let rest = Placed {
        x: board_x(),
        y: board_y(),
        w: (BOARD_W * SPRITE_PX) as f32,
        h: (BOARD_H * SPRITE_PX) as f32,
    };
    lerp(shelf, rest, lift_of(progress))
}

pub fn lid_at(progress: f32) -> (Placed, f32) {
    lid_from(shelf_cart(), progress)
}

pub fn lid_from(shelf: Placed, progress: f32) -> (Placed, f32) {
    lid_of(CartModel::Gba, shelf, progress)
}

/// Where the lid of a cart of `model` standing at `shelf` is, and how far it is turned, `progress`
/// of the way open: slid up off the board, then lifted to its rest.
pub fn lid_of(model: CartModel, shelf: Placed, progress: f32) -> (Placed, f32) {
    let slid = Placed {
        y: shelf.y - slide_up(model) * slide_of(progress),
        ..shelf
    };
    let lift = lift_of(progress);
    (lerp(slid, lid_rest_of(model), lift), LID_TURN * lift)
}

/// Where the art pixel `u, v` of the board lands on the screen with the board at `board`.
pub fn on_board(board: Placed, u: f32, v: f32) -> (f32, f32) {
    (
        board.x + u * board.w / BOARD_W as f32,
        board.y + v * board.h / BOARD_H as f32,
    )
}

/// How much bigger than at rest the board is drawn: 1 at rest, where a texel is `SPRITE_PX`.
pub fn board_zoom(board: Placed) -> f32 {
    board.w / (BOARD_W * SPRITE_PX) as f32
}

pub fn grown(p: Placed, by: f32) -> Placed {
    Placed {
        x: p.x - by,
        y: p.y - by,
        w: p.w + 2.0 * by,
        h: p.h + 2.0 * by,
    }
}

fn lerp(a: Placed, b: Placed, t: f32) -> Placed {
    Placed {
        x: a.x + (b.x - a.x) * t,
        y: a.y + (b.y - a.y) * t,
        w: a.w + (b.w - a.w) * t,
        h: a.h + (b.h - a.h) * t,
    }
}

pub fn padded(face: &CartFace, pad: u32) -> CartFace {
    let (w, h) = (face.w + 2 * pad, face.h + 2 * pad);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let stride = (face.w * 4) as usize;
    for row in 0..face.h {
        let from = (row * face.w * 4) as usize;
        let to = (((row + pad) * w + pad) * 4) as usize;
        rgba[to..to + stride].copy_from_slice(&face.rgba[from..from + stride]);
    }
    CartFace { rgba, w, h }
}

/// A socket on the board, clear where the board shows through: its printed outline with the
/// notch for the chip's first pin, the gold pads above and below it, and the core's name
/// printed in it, where the chip seated in it wears its own. It is the size of the sockets
/// `core` is seated among.
pub fn socket_face(core: Core) -> CartFace {
    let sockets = sockets_of(core);
    let mut face = blank(sockets.w, sockets.h);
    let (w, h) = (sockets.w as i32, sockets.h as i32);
    for x in (3..w - 2).step_by(2) {
        fill(&mut face, [x, 0, 1, 2], GOLD);
        fill(&mut face, [x, h - 2, 1, 2], GOLD);
    }
    let mid = w / 2;
    fill(&mut face, [0, 2, mid - 3, 1], SILK);
    fill(&mut face, [mid + 3, 2, w - mid - 3, 1], SILK);
    path(
        &mut face,
        &[(mid - 3, 3), (mid - 2, 4), (mid + 2, 4), (mid + 3, 3)],
        SILK,
    );
    fill(&mut face, [0, h - 3, w, 1], SILK);
    fill(&mut face, [0, 2, 1, h - 4], SILK);
    fill(&mut face, [w - 1, 2, 1, h - 4], SILK);
    let name = core.text();
    let x = name_x(name, sockets.chip_w);
    print(&mut face, name, x + 1, NAME_Y + 1, SOCKET_INK);
    cart_face(face)
}

/// The chip of the GBA board, its black body between rows of pins, with the core's name printed
/// on it, or none in the air between the sockets. A named chip is the size of the sockets its
/// core is seated among.
pub fn chip_face(core: Option<Core>) -> CartFace {
    let sockets = core.map_or(GBA_SOCKETS, sockets_of);
    chip_face_in(sockets, core)
}

/// The chip that fits `sockets`, with `core`'s name on it or none.
pub fn chip_face_in(sockets: Sockets, core: Option<Core>) -> CartFace {
    let mut face = blank(sockets.chip_w, sockets.chip_h);
    let (w, h) = (sockets.chip_w as i32, sockets.chip_h as i32);
    for x in (2..w - 1).step_by(2) {
        fill(&mut face, [x, 0, 1, 2], PIN);
        fill(&mut face, [x, h - 2, 1, 2], PIN);
        dot(&mut face, x, 1, PIN_DARK);
    }
    body(&mut face, [0, 2, w, h - 4]);
    fill(&mut face, [2, h - 7, 2, 2], BODY_HI);
    if let Some(core) = core {
        let name = core.text();
        print(
            &mut face,
            name,
            name_x(name, sockets.chip_w),
            NAME_Y,
            CHIP_INK,
        );
    }
    cart_face(face)
}

/// `line` in Pixel Code with its capitals' top left at `x, y`.
fn print(face: &mut UndoFace, line: &str, x: i32, y: i32, colour: Rgb) {
    ink(face, &letters(line, ADVANCE), x, y - ACCENT_ROWS, colour);
}

/// A shadow `w` by `h`: a pixel ellipse filled with a checker of black, the rest clear.
fn shadow(w: u32, h: u32) -> CartFace {
    let mut face = blank(w, h);
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    for y in 0..h {
        for x in 0..w {
            let dx = (x as f32 + 0.5 - cx) / cx;
            let dy = (y as f32 + 0.5 - cy) / cy;
            if dx * dx + dy * dy <= 1.0 && (x + y) % 2 == 0 {
                fill(&mut face, [x as i32, y as i32, 1, 1], [0, 0, 0]);
            }
        }
    }
    cart_face(face)
}

pub fn chip_shadow_face() -> CartFace {
    chip_shadow_face_in(GBA_SOCKETS)
}

/// The shadow under the chip that fits `sockets`, in the air.
pub fn chip_shadow_face_in(sockets: Sockets) -> CartFace {
    shadow(sockets.shadow_w(), SHADOW_H)
}

pub fn lid_shadow_face() -> CartFace {
    shadow(LID_SHADOW_W, LID_SHADOW_H)
}

/// The SNES board in its shell, laid out as the GBA's is, so the ROM chip carries its marking
/// in the same place and the sockets stand where they stand there: the back of the shell, its
/// corners as the cart's, and a screw post in each bottom corner; the board's wide body with a
/// notch in either side for the shell's ribs, and the tab across the middle of its foot with
/// the cart's 23 contacts; the mask ROM lying across, the lockout chip and the decoder, the
/// battery for the saves, and the board's name in its silkscreen.
fn snes_board(face: &mut UndoFace, p: &Plastic, shell: SnesShell) {
    snes_shell(face, p, shell);
    snes_pcb(face, p);
    snes_rom_chip(face);
    snes_parts(face);
}

/// The board's body inside the shell, and the tab its contacts are on, under the body.
const SNES_PCB: [i32; 4] = [12, 10, 162, 72];
const SNES_TAB: [i32; 4] = [35, 82, 116, 16];
/// The screw posts' middles, in the shell's bottom corners either side of the tab.
const SNES_POSTS: [(i32, i32); 2] = [(20, 92), (165, 92)];
/// A resistor's colour bands, and an electrolytic capacitor's can and its stripe.
const BAND_RED: Rgb = [0xc0, 0x3a, 0x30];
const BAND_DARK: Rgb = [0x3a, 0x2a, 0x22];
const CAN: Rgb = [0x2c, 0x3e, 0x8c];
const CAN_STRIPE: Rgb = [0x9a, 0xa8, 0xd8];

/// The back of a SNES shell: the plastic outlined in its deepest shade, the Super Famicom's top
/// corners round and the American cart's square; the floor inside, its top and left walls in
/// shade as the light falls from the top right; and the two screw posts.
fn snes_shell(face: &mut UndoFace, p: &Plastic, shell: SnesShell) {
    const RIM: i32 = 3;
    let (w, h) = (BOARD_W as i32, BOARD_H as i32);
    let (top, foot): (&[i32], &[i32]) = match shell {
        SnesShell::Famicom => (&[6, 4, 2, 1, 1], &[2, 1]),
        SnesShell::Ntsc => (&[2, 1], &[2, 1]),
    };
    let side = |y: i32| rounded(y, h, top, foot);
    let inside = |x: i32, y: i32| (0..h).contains(&y) && x >= side(y) && x < w - side(y);
    for y in 0..h {
        for x in side(y)..w - side(y) {
            let edge = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .any(|(dx, dy)| !inside(x + dx, y + dy));
            dot(face, x, y, if edge { p.deep } else { p.face });
        }
    }
    let inner = |y: i32| RIM + rounded(y - RIM, h - 2 * RIM, &top[top.len().min(2)..], &[1]);
    for y in RIM..h - RIM {
        let side = inner(y);
        let shade = match y - RIM {
            0 | 1 => p.wall_deep,
            2..=4 => p.wall,
            _ => p.floor,
        };
        fill(face, [side, y, w - 2 * side, 1], shade);
        if y - RIM > 4 {
            fill(face, [side, y, 3, 1], p.wall);
        }
    }
    for (cx, cy) in SNES_POSTS {
        checker(face, [cx - 5, cy + 6, 12, 2], p.deep);
        disc(face, cx, cy, 6, p.deep);
        disc(face, cx, cy, 5, p.face);
        disc(face, cx - 1, cy - 1, 2, lighter(p.face));
        disc(face, cx, cy, 2, p.deep);
    }
}

/// The board: its body and its tab in green, edged darker, shadow on the floor under its right
/// and bottom edges, its notches, its traces and vias, and the 23 contacts down the tab.
fn snes_pcb(face: &mut UndoFace, p: &Plastic) {
    let [x, y, w, h] = SNES_PCB;
    let [tx, ty, tw, th] = SNES_TAB;
    checker(face, [x + w, y + 2, 2, h], p.deep);
    checker(face, [x + 2, y + h, tx - x - 2, 2], p.deep);
    checker(face, [tx + tw, y + h, x + w - tx - tw + 2, 2], p.deep);
    checker(face, [tx + tw, ty + 2, 2, th - 2], p.deep);
    fill(face, [x, y, w, h], PCB_EDGE);
    fill(face, [tx, ty - 1, tw, th + 1], PCB_EDGE);
    fill(face, [x + 1, y + 1, w - 2, h - 2], PCB);
    fill(face, [tx + 1, ty - 1, tw - 2, th], PCB);
    for (cx, cy) in [
        (x, y),
        (x + w - 1, y),
        (tx, ty + th - 1),
        (tx + tw - 1, ty + th - 1),
    ] {
        dot(face, cx, cy, p.floor);
    }
    // the notches the shell's ribs hold the board by
    for nx in [x, x + w - 3] {
        fill(face, [nx, y + 16, 3, 6], p.floor);
        let edge = if nx == x { nx + 3 } else { nx - 1 };
        fill(face, [edge, y + 16, 1, 6], PCB_EDGE);
    }

    for trace in SNES_TRACES {
        path(face, trace, TRACE);
    }
    for &(x, y) in SNES_VIAS {
        via(face, x, y);
    }

    // 23 contacts, three pixels of gold and two of board, down to the tab's edge
    for n in 0..23 {
        let cx = tx + 2 + n * 5;
        fill(face, [cx, ty + 2, 3, th - 2], GOLD);
        fill(face, [cx + 2, ty + 3, 1, th - 3], GOLD_DARK);
        dot(face, cx, ty + 2, PCB);
        dot(face, cx + 2, ty + 2, PCB);
    }
}

const SNES_TRACES: &[&[(i32, i32)]] = &[
    // the ROM's address and data lines over to the first socket
    &[(76, 30), (82, 30), (86, 34), (86, 41)],
    &[(76, 34), (80, 34), (83, 37), (83, 41)],
    &[(76, 38), (78, 38), (80, 40), (80, 41)],
    // the ROM's bottom row of pins down to the tab
    &[(40, 71), (40, 83)],
    &[(46, 71), (46, 83)],
    &[(52, 71), (52, 83)],
    &[(58, 71), (58, 77), (61, 80), (61, 83)],
    &[(64, 71), (64, 75), (71, 82), (71, 83)],
    &[(70, 71), (70, 73), (78, 81), (78, 83)],
    // the lockout chip and the decoder, along under them and down past the battery
    &[
        (97, 26),
        (97, 29),
        (110, 29),
        (113, 32),
        (125, 32),
        (128, 29),
        (128, 26),
    ],
    &[
        (138, 23),
        (142, 27),
        (142, 36),
        (145, 39),
        (171, 39),
        (171, 60),
    ],
    // the sockets' pins down to the tab
    &[(90, 67), (90, 83)],
    &[(95, 67), (95, 83)],
    &[(100, 67), (100, 83)],
    &[(105, 67), (105, 83)],
    &[(110, 67), (110, 83)],
    &[(140, 67), (140, 76), (136, 80), (136, 83)],
    &[(145, 67), (145, 76), (141, 80), (141, 83)],
    &[(150, 67), (150, 76), (146, 80), (146, 83)],
];

const SNES_VIAS: &[(i32, i32)] = &[(79, 26), (110, 38), (131, 76), (30, 66), (171, 62)];

/// The mask ROM, a long DIP lying across the board: a black body, a row of pins along its top
/// and its bottom, the notch at its first pin's end.
fn snes_rom_chip(face: &mut UndoFace) {
    let [x, y, w, h] = [ROM_X as i32, ROM_Y as i32, ROM_W as i32, ROM_H as i32];
    for n in 0..18 {
        let px = x + 3 + n * 3;
        fill(face, [px, y - 2, 2, 2], PIN);
        fill(face, [px, y + h, 2, 2], PIN);
        dot(face, px + 1, y - 1, PIN_DARK);
    }
    body(face, [x, y, w, h]);
    fill(face, [x, y + h / 2 - 3, 2, 6], BODY_HI);
    fill(face, [x + 3, y + h - 5, 2, 2], BODY_HI);
}

/// The rest of the board: the lockout chip and the decoder, small DIPs with their names in the
/// silkscreen; the battery in its holder; an electrolytic capacitor; a resistor between the
/// sockets; ceramic capacitors in their printed outlines; and the board's name.
fn snes_parts(face: &mut UndoFace) {
    for (x, name) in [(86, "CIC"), (114, "139")] {
        for n in 0..8 {
            let px = x + 1 + n * 3;
            fill(face, [px, 13, 1, 2], PIN);
            fill(face, [px, 23, 1, 2], PIN);
        }
        body(face, [x, 15, 24, 8]);
        dot(face, x + 2, 20, BODY_HI);
        if x == 86 {
            print(face, name, x + 2, 28, SILK);
        }
    }

    // the battery: its holder, the cell, and the tab soldered over it
    disc(face, 158, 26, 10, BODY);
    disc(face, 158, 26, 9, CELL_EDGE);
    disc(face, 158, 26, 8, CELL);
    ring(face, 158, 26, 6, CELL_RING);
    fill(face, [155, 15, 7, 4], PIN);
    fill(face, [155, 18, 7, 1], PIN_DARK);

    disc(face, 143, 18, 3, CAN);
    fill(face, [141, 16, 1, 5], CAN_STRIPE);

    // a resistor between the sockets: leads, a tan body, three bands
    fill(face, [117, 71, 14, 1], PIN);
    fill(face, [119, 70, 10, 3], PART);
    for (bx, c) in [(121, BAND_RED), (123, BAND_DARK), (126, BAND_RED)] {
        fill(face, [bx, 70, 1, 3], c);
    }

    capacitor(face, 20, 74);
    capacitor(face, 162, 74);

    print(face, "SHVC-1A3M", 17, 12, SILK);
}
