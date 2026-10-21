//! The Mega Drive carts on the shelf: the Mega Drive's and the Genesis's. With MD_OUT set,
//! writes the faces there as PNGs to look at, each at three times its size, with label art and
//! without.

use omega_store::{scan, Cart, Platform};
use omega_ui::{
    cart_face, label_colour, label_text, md_silhouette, model_box, model_of, model_shadow,
    shell_for, CartFace, CartModel, Finish, MdShell, GENESIS_SHELL, MEGA_DRIVE_SHELL,
};
use tempfile::TempDir;

const ART: [u8; 3] = [0x20, 0xc0, 0x40];

/// A ROM with a Mega Drive header sold in `regions`.
fn rom(regions: &[u8]) -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    rom[0x100..0x200].fill(b' ');
    rom[0x100..0x10f].copy_from_slice(b"SEGA MEGA DRIVE");
    rom[0x150..0x155].copy_from_slice(b"TITLE");
    rom[0x1f0..0x1f0 + regions.len()].copy_from_slice(regions);
    rom
}

/// A card with a ROM for each of `games` (its name and the regions its header names), label
/// art for those in `labelled`, and `cart_shell` as its cart_shell.ini.
fn root(games: &[(&str, &[u8])], labelled: &[&str], cart_shell: &str) -> TempDir {
    let d = tempfile::tempdir().expect("tempdir");
    let dir = d.path().join("Games/MD");
    std::fs::create_dir_all(&dir).unwrap();
    for (stem, regions) in games {
        std::fs::write(dir.join(format!("{stem}.md")), rom(regions)).unwrap();
    }
    let labels = d.path().join("Labels/MD");
    std::fs::create_dir_all(&labels).unwrap();
    for stem in labelled {
        let f = std::fs::File::create(labels.join(format!("{stem}.png"))).unwrap();
        let mut enc = png::Encoder::new(std::io::BufWriter::new(f), 40, 36);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let rgb: Vec<u8> = (0..40 * 36).flat_map(|_| ART).collect();
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
fn an_american_game_stands_as_a_genesis_cart_and_any_other_as_the_mega_drive_s() {
    let d = root(
        &[
            ("Sonic the Hedgehog (USA, Europe)", b"JUE"),
            ("Puyo Puyo (Japan)", b"J"),
            ("Probotector (Europe)", b"E"),
            ("Untagged American", b"U"),
            ("Untagged Japanese", b"J"),
            ("Forced (USA)", b"U"),
        ],
        &[],
        "Forced (USA) = megadrive 222225 solid\n",
    );
    let carts = scan(d.path()).unwrap();
    for (stem, shell) in [
        ("Sonic the Hedgehog (USA, Europe)", MdShell::Genesis),
        ("Puyo Puyo (Japan)", MdShell::MegaDrive),
        ("Probotector (Europe)", MdShell::MegaDrive),
        ("Untagged American", MdShell::Genesis),
        ("Untagged Japanese", MdShell::MegaDrive),
        ("Forced (USA)", MdShell::MegaDrive),
    ] {
        let c = cart(&carts, stem);
        assert_eq!(c.platform, Platform::Md);
        assert_eq!(model_of(c), CartModel::Md(shell), "{stem}");
    }
}

#[test]
fn each_mega_drive_shell_is_the_real_cart_s_shape_at_the_nes_carts_scale() {
    for (name, shell, real) in [
        ("Mega Drive", MdShell::MegaDrive, (112.0, 130.0)),
        ("Genesis", MdShell::Genesis, (110.0, 135.0)),
    ] {
        let (w, h) = model_box(CartModel::Md(shell));
        let aspect = w as f32 / h as f32;
        assert!(
            (aspect / (real.0 / real.1) - 1.0).abs() < 0.01,
            "the {name} face is {aspect}:1"
        );
        assert!(
            w % 4 == 0 && h % 2 == 0,
            "the {name} face does not halve onto the grid"
        );
        let per_mm = w as f32 / real.0;
        assert!(
            (per_mm / (212.0 / 120.0) - 1.0).abs() < 0.02,
            "{name}: {per_mm} px/mm"
        );
        assert!(CartModel::Md(shell).thick() > 0.17);
    }
}

#[test]
fn a_mega_drive_face_is_clipped_to_its_own_silhouette_and_backed_by_its_own_shadow() {
    let d = root(
        &[
            ("Sonic the Hedgehog (USA)", b"U"),
            ("Puyo Puyo (Japan)", b"J"),
        ],
        &[],
        "",
    );
    let carts = scan(d.path()).unwrap();
    for (stem, shell) in [
        ("Sonic the Hedgehog (USA)", MdShell::Genesis),
        ("Puyo Puyo (Japan)", MdShell::MegaDrive),
    ] {
        let face = cart_face(cart(&carts, stem));
        let model = CartModel::Md(shell);
        assert_eq!((face.w, face.h), model_box(model));
        let mask = md_silhouette(shell, face.w, face.h);
        for (px, cover) in face.rgba.chunks_exact(4).zip(&mask) {
            assert!(px[3] <= *cover, "{stem}: opaque outside its outline");
        }
        assert_eq!(alpha(&face, 0, 0), 0, "{stem}: the corner is opaque");
        assert!(alpha(&face, face.w / 2, face.h / 2) > 250);
        let shadow = model_shadow(model);
        for (px, cover) in shadow.rgba.chunks_exact(4).zip(&mask) {
            assert_eq!(px[3], *cover, "{stem}: the shadow is not the cart's shape");
        }
    }
    // the Mega Drive cart's top corners are rolled far rounder than the Genesis cart's
    let (md, genesis) = (
        md_silhouette(MdShell::MegaDrive, 200, 232),
        md_silhouette(MdShell::Genesis, 200, 232),
    );
    assert_eq!(
        md[200 * 4 + 4],
        0,
        "the Mega Drive cart's top corner is not round"
    );
    assert!(
        genesis[200 * 4 + 4] > 200,
        "the Genesis cart's top corner is round"
    );
}

#[test]
fn a_mega_drive_label_fills_its_well_where_the_real_cart_has_it() {
    let stems = ["Sonic the Hedgehog (USA)", "Puyo Puyo (Japan)"];
    let d = root(&[(stems[0], b"U"), (stems[1], b"J")], &stems, "");
    let carts = scan(d.path()).unwrap();
    // the share of the face the label takes, and where on the face it starts and ends
    for (stem, (wide, tall), starts, ends) in [
        ("Sonic the Hedgehog (USA)", (0.75, 0.55), 0.14, 0.76),
        ("Puyo Puyo (Japan)", (0.8, 0.62), 0.06, 0.75),
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
        assert!(
            (y0 as f32) > fh * starts,
            "{stem}: the label starts at {y0}"
        );
        assert!((y1 as f32) < fh * ends, "{stem}: the label runs to {y1}");
    }
}

#[test]
fn a_mega_drive_cart_with_no_label_art_is_labelled_with_its_name() {
    let stems = ["Sonic the Hedgehog (USA)", "Puyo Puyo (Japan)"];
    let d = root(&[(stems[0], b"U"), (stems[1], b"J")], &[], "");
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
fn both_mega_drive_carts_are_black() {
    let d = root(
        &[
            ("Sonic the Hedgehog (USA)", b"U"),
            ("Puyo Puyo (Japan)", b"J"),
            ("Chosen (Japan)", b"J"),
        ],
        &[],
        "Chosen (Japan) = auto c23f3a solid\n",
    );
    let carts = scan(d.path()).unwrap();
    assert_eq!(
        shell_for(cart(&carts, "Sonic the Hedgehog (USA)")),
        GENESIS_SHELL
    );
    assert_eq!(
        shell_for(cart(&carts, "Puyo Puyo (Japan)")),
        MEGA_DRIVE_SHELL
    );
    assert_eq!(
        shell_for(cart(&carts, "Chosen (Japan)")).colour,
        [0xc2, 0x3f, 0x3a],
        "cart_shell.ini's colour is not used"
    );
    for s in [MEGA_DRIVE_SHELL, GENESIS_SHELL] {
        assert_eq!(s.finish, Finish::Solid);
        assert!(s.colour.iter().all(|c| *c < 0x40), "{s:?} is not black");
    }
}

#[test]
fn render_md_carts() {
    let Ok(out) = std::env::var("MD_OUT") else {
        return;
    };
    let games: [(&str, &[u8]); 4] = [
        ("Sonic the Hedgehog (USA)", b"U"),
        ("Streets of Rage 2 (USA)", b"U"),
        ("Puyo Puyo (Japan)", b"J"),
        ("Probotector (Europe)", b"E"),
    ];
    let d = root(
        &games,
        &["Sonic the Hedgehog (USA)", "Puyo Puyo (Japan)"],
        "",
    );
    let carts = scan(d.path()).unwrap();
    for (stem, _) in games {
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
