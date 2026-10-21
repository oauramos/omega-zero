//! The NES carts on the shelf: the Game Pak and the Famicom cart. With NES_OUT set, writes the
//! faces there as PNGs to look at, each at four times its size, with label art and without.

use omega_store::{scan, Cart, Platform};
use omega_ui::{
    cart_face, famicom_shell_for, label_colour, label_text, model_box, model_of, model_shadow,
    nes_silhouette, shell_for, CartFace, CartModel, Finish, NesShell, FAMICOM_SHELLS, NES_SHELL,
};
use tempfile::TempDir;

const ART: [u8; 3] = [0x20, 0xc0, 0x40];

/// A card with an iNES image on it for each of `stems`, label art for those in `labelled`, and
/// `cart_shell` as its cart_shell.ini.
fn root(stems: &[&str], labelled: &[&str], cart_shell: &str) -> TempDir {
    let d = tempfile::tempdir().expect("tempdir");
    let games = d.path().join("Games/NES");
    std::fs::create_dir_all(&games).unwrap();
    for stem in stems {
        let mut image = b"NES\x1a\x02\x01".to_vec();
        image.resize(16 + 40 * 1024, 0);
        std::fs::write(games.join(format!("{stem}.nes")), image).unwrap();
    }
    let labels = d.path().join("Labels/NES");
    std::fs::create_dir_all(&labels).unwrap();
    for stem in labelled {
        let f = std::fs::File::create(labels.join(format!("{stem}.png"))).unwrap();
        let mut enc = png::Encoder::new(std::io::BufWriter::new(f), 40, 30);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let rgb: Vec<u8> = (0..40 * 30).flat_map(|_| ART).collect();
        enc.write_header().unwrap().write_image_data(&rgb).unwrap();
    }
    std::fs::create_dir_all(d.path().join("Config")).unwrap();
    std::fs::write(d.path().join("Config/cart_shell.ini"), cart_shell).unwrap();
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
fn a_japanese_game_stands_as_a_famicom_cart_and_any_other_as_the_game_pak() {
    let d = root(
        &[
            "Super Mario Bros. 3 (USA)",
            "Super Mario Bros. 3 (Europe)",
            "Super Mario Bros. 3 (Japan)",
            "Tetris",
            "Forced (Japan)",
        ],
        &[],
        "Forced (Japan) = nes 8f8e93 solid\n",
    );
    let carts = scan(d.path()).unwrap();
    for (stem, shell) in [
        ("Super Mario Bros. 3 (USA)", NesShell::Nes),
        ("Super Mario Bros. 3 (Europe)", NesShell::Nes),
        ("Super Mario Bros. 3 (Japan)", NesShell::Famicom),
        ("Tetris", NesShell::Nes),
        ("Forced (Japan)", NesShell::Nes),
    ] {
        let c = cart(&carts, stem);
        assert_eq!(c.platform, Platform::Nes);
        assert_eq!(model_of(c), CartModel::Nes(shell), "{stem}");
    }
}

#[test]
fn each_nes_shell_is_the_real_cart_s_shape_the_game_pak_the_bigger() {
    let (nw, nh) = model_box(CartModel::Nes(NesShell::Nes));
    let (fw, fh) = model_box(CartModel::Nes(NesShell::Famicom));
    assert!(nw > fw && nh > fh, "the Game Pak is not the bigger");
    for (name, (w, h), real) in [
        ("Game Pak", (nw, nh), 120.0 / 133.0),
        ("Famicom", (fw, fh), 108.0 / 71.0),
    ] {
        let aspect = w as f32 / h as f32;
        assert!(
            (aspect / real - 1.0).abs() < 0.01,
            "the {name} face is {aspect}:1, the real cart {real}:1"
        );
        assert!(
            w % 4 == 0 && h % 2 == 0,
            "the {name} face, {w}x{h}, does not halve onto the art grid"
        );
    }
    // the two carts are the same scale: the Game Pak's 120 mm and the Famicom cart's 108
    let (per_mm_nes, per_mm_famicom) = (nw as f32 / 120.0, fw as f32 / 108.0);
    assert!((per_mm_nes / per_mm_famicom - 1.0).abs() < 0.01);
}

#[test]
fn an_nes_face_is_clipped_to_its_own_silhouette_and_backed_by_its_own_shadow() {
    let d = root(
        &["Super Mario Bros. 3 (USA)", "Super Mario Bros. 3 (Japan)"],
        &[],
        "",
    );
    let carts = scan(d.path()).unwrap();
    for (stem, shell) in [
        ("Super Mario Bros. 3 (USA)", NesShell::Nes),
        ("Super Mario Bros. 3 (Japan)", NesShell::Famicom),
    ] {
        let face = cart_face(cart(&carts, stem));
        let model = CartModel::Nes(shell);
        assert_eq!((face.w, face.h), model_box(model));
        let mask = nes_silhouette(shell, face.w, face.h);
        for (px, cover) in face.rgba.chunks_exact(4).zip(&mask) {
            assert!(px[3] <= *cover, "{stem}: opaque outside its outline");
        }
        assert_eq!(alpha(&face, 0, 0), 0, "{stem}: the corner is opaque");
        assert!(alpha(&face, face.w / 2, face.h / 2) > 250);
        let shadow = model_shadow(model);
        for (px, cover) in shadow.rgba.chunks_exact(4).zip(&mask) {
            assert_eq!(&px[..3], &[0, 0, 0], "{stem}: the shadow is not black");
            assert_eq!(px[3], *cover, "{stem}: the shadow is not the cart's shape");
        }
    }
}

#[test]
fn an_nes_label_fills_its_well_where_the_real_cart_has_it() {
    let stems = ["Super Mario Bros. 3 (USA)", "Super Mario Bros. 3 (Japan)"];
    let d = root(&stems, &stems, "");
    let carts = scan(d.path()).unwrap();
    // the share of the face's width and height the label takes, and how far down it may end
    for (stem, (wide, tall), ends) in [
        ("Super Mario Bros. 3 (USA)", (0.75, 0.48), 0.62),
        ("Super Mario Bros. 3 (Japan)", (0.85, 0.65), 0.85),
    ] {
        let face = cart_face(cart(&carts, stem));
        let (x0, y0, x1, y1) = colour_box(&face, ART).expect("the label art was not pasted");
        let (lw, lh) = ((x1 - x0 + 1) as f32, (y1 - y0 + 1) as f32);
        let (fw, fh) = (face.w as f32, face.h as f32);
        assert!(
            lw / fw > wide && lh / fh > tall,
            "{stem}: the label is {lw}x{lh} on a {fw}x{fh} face"
        );
        assert!(
            ((x0 + x1 + 1) as f32 / 2.0 - fw / 2.0).abs() <= 1.0,
            "{stem}: the label is not across the middle"
        );
        assert!((y1 as f32) < fh * ends, "{stem}: the label runs to {y1}");
    }
}

#[test]
fn an_nes_cart_with_no_label_art_is_labelled_with_its_name() {
    let stems = ["Super Mario Bros. 3 (USA)", "Super Mario Bros. 3 (Japan)"];
    let d = root(&stems, &[], "");
    let carts = scan(d.path()).unwrap();
    for stem in stems {
        let c = cart(&carts, stem);
        let face = cart_face(c);
        let bg = label_colour(&label_text(c));
        let (x0, y0, x1, y1) = colour_box(&face, bg).expect("no generated label");
        let inked = (y0..=y1)
            .flat_map(|y| (x0..=x1).map(move |x| (x, y)))
            .filter(|&(x, y)| pixel(&face, x, y) != bg)
            .count();
        assert!(inked > 100, "{stem}: no title on its label");
    }
}

#[test]
fn a_game_pak_is_grey_and_a_famicom_cart_its_game_s_own_colour() {
    let d = root(
        &[
            "Super Mario Bros. 3 (USA)",
            "Super Mario Bros. 3 (Japan)",
            "Super Mario Bros. 3 (Japan) (Rev 1)",
            "Chosen (Japan)",
        ],
        &[],
        "Chosen (Japan) = auto c23f3a solid\n",
    );
    let carts = scan(d.path()).unwrap();
    assert_eq!(
        shell_for(cart(&carts, "Super Mario Bros. 3 (USA)")),
        NES_SHELL
    );
    let famicom = shell_for(cart(&carts, "Super Mario Bros. 3 (Japan)"));
    assert!(FAMICOM_SHELLS.contains(&famicom));
    assert_eq!(
        shell_for(cart(&carts, "Super Mario Bros. 3 (Japan) (Rev 1)")),
        famicom,
        "two releases of one game come in two colours"
    );
    assert_eq!(
        shell_for(cart(&carts, "Chosen (Japan)")).colour,
        [0xc2, 0x3f, 0x3a],
        "cart_shell.ini's colour is not used"
    );
    let shown: std::collections::HashSet<[u8; 3]> = (0..64)
        .map(|i| famicom_shell_for(&format!("Game {i} (Japan)")).colour)
        .collect();
    assert!(shown.len() >= 6, "the Famicom shelf is nearly one colour");
    for s in FAMICOM_SHELLS.iter().chain([&NES_SHELL]) {
        assert_eq!(s.finish, Finish::Solid);
    }
}

#[test]
fn render_nes_carts() {
    let Ok(out) = std::env::var("NES_OUT") else {
        return;
    };
    let stems = [
        "Super Mario Bros. 3 (USA)",
        "Kid Icarus (Europe)",
        "Super Mario Bros. 3 (Japan)",
        "Ganbare Goemon (Japan)",
    ];
    let d = root(
        &stems,
        &["Super Mario Bros. 3 (USA)", "Super Mario Bros. 3 (Japan)"],
        "",
    );
    let carts = scan(d.path()).unwrap();
    for stem in stems {
        let face = cart_face(cart(&carts, stem));
        let zoom = 3;
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
