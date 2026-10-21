//! The PC Engine's HuCard on the shelf. With HUCARD_OUT set, writes the faces there as PNGs to
//! look at, each at four times its size: a HuCard and a TurboChip, with label art and without.

use omega_store::{scan, Cart, Platform};
use omega_ui::{
    cart_face, hucard_silhouette, label_colour, label_text, model_box, model_of, model_shadow,
    seated_box, shell_for, CartFace, CartModel, Finish, HUCARD_SHELL, TURBOCHIP_SHELL,
};
use tempfile::TempDir;

const ART: [u8; 3] = [0x20, 0xc0, 0x40];

/// A card with a HuCard image on it for each of `stems`, and label art for those in `labelled`.
fn root(stems: &[&str], labelled: &[&str]) -> TempDir {
    let d = tempfile::tempdir().expect("tempdir");
    let games = d.path().join("Games/PCE");
    std::fs::create_dir_all(&games).unwrap();
    for stem in stems {
        std::fs::write(games.join(format!("{stem}.pce")), vec![0u8; 256 * 1024]).unwrap();
    }
    let labels = d.path().join("Labels/PCE");
    std::fs::create_dir_all(&labels).unwrap();
    for stem in labelled {
        let f = std::fs::File::create(labels.join(format!("{stem}.png"))).unwrap();
        let mut enc = png::Encoder::new(std::io::BufWriter::new(f), 30, 42);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let rgb: Vec<u8> = (0..30 * 42).flat_map(|_| ART).collect();
        enc.write_header().unwrap().write_image_data(&rgb).unwrap();
    }
    d
}

fn cart<'a>(carts: &'a [Cart], stem: &str) -> &'a Cart {
    carts
        .iter()
        .find(|c| c.stem == stem)
        .unwrap_or_else(|| panic!("{stem} was not scanned"))
}

fn pixel(face: &CartFace, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * face.w + x) * 4) as usize;
    [face.rgba[i], face.rgba[i + 1], face.rgba[i + 2]]
}

fn alpha(face: &CartFace, x: u32, y: u32) -> u8 {
    face.rgba[((y * face.w + x) * 4 + 3) as usize]
}

/// The box of the opaque pixels of `face` that are `colour`.
fn colour_box(face: &CartFace, colour: [u8; 3]) -> Option<(u32, u32, u32, u32)> {
    let mut b: Option<(u32, u32, u32, u32)> = None;
    for y in 0..face.h {
        for x in 0..face.w {
            if pixel(face, x, y) == colour && alpha(face, x, y) == 255 {
                b = Some(match b {
                    None => (x, y, x, y),
                    Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
                });
            }
        }
    }
    b
}

#[test]
fn a_pc_engine_game_stands_as_a_hucard_the_real_card_s_shape() {
    let d = root(&["Bonk's Adventure (USA)"], &[]);
    let carts = scan(d.path()).unwrap();
    let c = cart(&carts, "Bonk's Adventure (USA)");
    assert_eq!(c.platform, Platform::Pce);
    assert_eq!(model_of(c), CartModel::HuCard);
    let (w, h) = model_box(CartModel::HuCard);
    assert_eq!(seated_box(Platform::Pce), (w, h));
    let (aspect, real) = (w as f32 / h as f32, 54.0 / 85.0);
    assert!(
        (aspect / real - 1.0).abs() < 0.01,
        "the face is {aspect}:1, the real card {real}:1"
    );
    assert!(
        w % 4 == 0 && h % 2 == 0,
        "the face, {w}x{h}, does not halve onto the art grid"
    );
    assert!(
        CartModel::HuCard.thick() < CartModel::Gba.thick() / 2.0,
        "a HuCard is a card, not a cart"
    );
}

#[test]
fn a_hucard_face_is_clipped_to_its_round_corners_and_backed_by_its_own_shadow() {
    let d = root(&["Bonk's Adventure (USA)"], &["Bonk's Adventure (USA)"]);
    let carts = scan(d.path()).unwrap();
    let face = cart_face(cart(&carts, "Bonk's Adventure (USA)"));
    assert_eq!((face.w, face.h), model_box(CartModel::HuCard));
    let mask = hucard_silhouette(face.w, face.h);
    for (i, (px, cover)) in face.rgba.chunks_exact(4).zip(&mask).enumerate() {
        assert!(
            px[3] <= *cover,
            "pixel {i} is {} opaque outside the outline's {cover}",
            px[3]
        );
    }
    for (x, y) in [
        (0, 0),
        (face.w - 1, 0),
        (0, face.h - 1),
        (face.w - 1, face.h - 1),
    ] {
        assert_eq!(alpha(&face, x, y), 0, "the corner at {x},{y} is not round");
    }
    assert!(alpha(&face, face.w / 2, face.h / 2) > 250);
    let shadow = model_shadow(CartModel::HuCard);
    for (px, cover) in shadow.rgba.chunks_exact(4).zip(&mask) {
        assert_eq!(&px[..3], &[0, 0, 0], "the shadow is not black");
        assert_eq!(px[3], *cover, "the shadow is not the card's shape");
    }
}

#[test]
fn a_hucard_label_fills_the_card_above_its_grip() {
    let d = root(&["Bonk's Adventure (USA)"], &["Bonk's Adventure (USA)"]);
    let carts = scan(d.path()).unwrap();
    let face = cart_face(cart(&carts, "Bonk's Adventure (USA)"));
    let (x0, y0, x1, y1) = colour_box(&face, ART).expect("the label art was not pasted");
    let (lw, lh) = ((x1 - x0 + 1) as f32, (y1 - y0 + 1) as f32);
    let (fw, fh) = (face.w as f32, face.h as f32);
    assert!(
        lw / fw > 0.75 && lh / fh > 0.6,
        "the label is {lw}x{lh} on a {fw}x{fh} face"
    );
    assert!(
        ((x0 + x1 + 1) as f32 / 2.0 - fw / 2.0).abs() <= 1.0,
        "the label is not across the middle"
    );
    assert!(
        (y1 as f32) < fh * 0.8,
        "the label runs down over the grip, to {y1}"
    );
}

#[test]
fn a_hucard_with_no_label_art_is_labelled_with_its_name() {
    let d = root(&["Bonk's Adventure (USA)"], &[]);
    let carts = scan(d.path()).unwrap();
    let c = cart(&carts, "Bonk's Adventure (USA)");
    let face = cart_face(c);
    let bg = label_colour(&label_text(c));
    let (x0, y0, x1, y1) = colour_box(&face, bg).expect("no generated label");
    let inked = (y0..=y1)
        .flat_map(|y| (x0..=x1).map(move |x| (x, y)))
        .filter(|&(x, y)| pixel(&face, x, y) != bg)
        .count();
    assert!(inked > 100, "no title on its label");
}

#[test]
fn a_turbochip_is_black_and_a_hucard_pale() {
    let d = root(
        &[
            "Bonk's Adventure (USA)",
            "PC Genjin (Japan)",
            "Gomola Speed",
        ],
        &[],
    );
    let carts = scan(d.path()).unwrap();
    assert_eq!(
        shell_for(cart(&carts, "Bonk's Adventure (USA)")),
        TURBOCHIP_SHELL
    );
    assert_eq!(shell_for(cart(&carts, "PC Genjin (Japan)")), HUCARD_SHELL);
    assert_eq!(shell_for(cart(&carts, "Gomola Speed")), HUCARD_SHELL);
    let lum = |c: [u8; 3]| c.iter().map(|v| u32::from(*v)).sum::<u32>();
    assert!(lum(HUCARD_SHELL.colour) > lum(TURBOCHIP_SHELL.colour) + 300);
    for s in [HUCARD_SHELL, TURBOCHIP_SHELL] {
        assert_eq!(s.finish, Finish::Solid);
    }
}

#[test]
fn render_hucards() {
    let Ok(out) = std::env::var("HUCARD_OUT") else {
        return;
    };
    let stems = ["Bonk's Adventure (USA)", "PC Genjin (Japan)"];
    let d = root(&stems, &["PC Genjin (Japan)"]);
    let carts = scan(d.path()).unwrap();
    for stem in stems {
        let face = cart_face(cart(&carts, stem));
        let zoom = 4;
        let (w, h) = (face.w * zoom, face.h * zoom);
        let mut px = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let s = (((y / zoom) * face.w + x / zoom) * 4) as usize;
                let d = ((y * w + x) * 4) as usize;
                px[d..d + 4].copy_from_slice(&face.rgba[s..s + 4]);
            }
        }
        let path = std::path::Path::new(&out).join(format!("{stem}.png"));
        let f = std::fs::File::create(path).unwrap();
        let mut e = png::Encoder::new(std::io::BufWriter::new(f), w, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().unwrap().write_image_data(&px).unwrap();
    }
}
