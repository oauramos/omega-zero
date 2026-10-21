use omega_store::{Cart, Core, Platform};
use omega_ui::{
    board_at, board_face, chip_face, chip_shadow_face, gba_shell_for, lid_at, lid_shadow_face,
    lift_of, on_board, padded, rom_marking, rom_marking_face, shelf_cart, slide_of, socket_face,
    CartFace, Placed, BOARD_H, BOARD_W, CART_H, CHIP_H, CHIP_U, CHIP_V, CHIP_W, DEFAULT_SHELL,
    LID_SHADOW_H, LID_SHADOW_W, LID_TURN, ROM_H, ROM_W, ROM_X, ROM_Y, SHADOW_H, SHADOW_W,
    SLIDE_SHARE, SLIDE_UP, SOCKET_H, SOCKET_U, SOCKET_V, SOCKET_W,
};
use omega_ui::{out_h, stage_x, stage_y, BAR_H, SFC_SHELL, SNES_SHELL, TOP_BAR_H};

fn cart(stem: &str, code: &str) -> Cart {
    Cart {
        platform: Platform::Gba,
        stem: stem.into(),
        rom: format!("Games/GBA/{stem}.gba").into(),
        label: None,
        code: code.into(),
        shell: None,
        title: stem.to_uppercase(),
    }
}

fn rgb(face: &CartFace, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * face.w + x) * 4) as usize;
    [face.rgba[i], face.rgba[i + 1], face.rgba[i + 2]]
}

fn near(a: [u8; 3], b: [u8; 3]) -> bool {
    (0..3).all(|k| (a[k] as i32 - b[k] as i32).abs() <= 3)
}

const EMERALD: &str = "Pokemon - Emerald Version (USA, Europe)";

#[test]
fn the_marking_is_the_title_stacked() {
    assert_eq!(rom_marking(EMERALD), ["POKEMON", "EMERALD", "VERSION"]);
}

#[test]
fn a_long_title_folds_its_tail_into_the_third_line() {
    assert_eq!(
        rom_marking("Advance Wars 2 - Black Hole Rising"),
        ["ADVANCE", "WARS 2", "BLACK HOLE RISING"]
    );
}

#[test]
fn bracketed_tags_never_reach_the_chip() {
    assert_eq!(
        rom_marking("Pokemon - LeafGreen Version (USA, Europe) (Rev 1)"),
        ["POKEMON", "LEAFGREEN", "VERSION"]
    );
    assert_eq!(rom_marking("Metroid Fusion"), ["METROID", "FUSION"]);
}

#[test]
fn the_marking_is_drawn_and_stays_on_the_chip() {
    for stem in [
        EMERALD,
        "Metroid Fusion",
        "Advance Wars 2 - Black Hole Rising",
        "Pokemon - LeafGreen Version (USA, Europe) (Rev 1)",
    ] {
        let face = rom_marking_face(stem);
        assert_eq!((face.w, face.h), (ROM_W, ROM_H));
        let alpha = |x: u32, y: u32| face.rgba[((y * face.w + x) * 4 + 3) as usize];
        assert!(
            (0..face.h).any(|y| (0..face.w).any(|x| alpha(x, y) > 128)),
            "{stem} left no marking"
        );
        for y in 0..face.h {
            assert_eq!(alpha(0, y), 0, "{stem} ran off the left of the chip");
            assert_eq!(
                alpha(face.w - 1, y),
                0,
                "{stem} ran off the right of the chip"
            );
        }
    }
}

#[test]
fn the_cell_carries_no_print() {
    let face = board_face(&cart(EMERALD, "BPEE"));
    let dark = (24..37)
        .flat_map(|y| (146..159).map(move |x| (x, y)))
        .filter(|&(x, y)| (x as i32 - 152).pow(2) + (y as i32 - 30).pow(2) <= 30)
        .filter(|&(x, y)| rgb(&face, x, y)[0] < 150)
        .count();
    assert_eq!(dark, 0, "the cell has {dark} dark pixels of print");
}

#[test]
fn the_board_is_drawn_a_texel_to_an_art_pixel() {
    let face = board_face(&cart(EMERALD, "BPEE"));
    assert_eq!((face.w, face.h), (BOARD_W, BOARD_H));
    assert_eq!((BOARD_W, BOARD_H), (186, 104));
    let rest = board_at(1.0);
    assert_eq!(
        (rest.w, rest.h),
        (372.0, 208.0),
        "the board does not rest at 2x"
    );
}

#[test]
fn the_back_shell_is_the_carts_own_plastic() {
    let emerald = board_face(&cart(EMERALD, "BPEE"));
    assert!(
        near(rgb(&emerald, 5, 70), gba_shell_for("BPEE").colour),
        "Emerald's rim is {:?}",
        rgb(&emerald, 5, 70)
    );
    let unknown = board_face(&cart("Homebrew", ""));
    assert!(
        near(rgb(&unknown, 5, 70), DEFAULT_SHELL.colour),
        "an unknown cart's rim is {:?}",
        rgb(&unknown, 5, 70)
    );
}

#[test]
fn the_board_itself_is_drawn() {
    let face = board_face(&cart(EMERALD, "BPEE"));
    assert!(
        near(rgb(&face, 128, 45), [0x3a, 0x9a, 0x3c]),
        "the board is {:?}",
        rgb(&face, 128, 45)
    );
}

fn alpha(face: &CartFace, x: u32, y: u32) -> u8 {
    face.rgba[((y * face.w + x) * 4 + 3) as usize]
}

/// Every pixel of a pixel-art face is there or not: no edge is part covered.
fn assert_flat(face: &CartFace, what: &str) {
    for (i, px) in face.rgba.chunks_exact(4).enumerate() {
        assert!(
            px[3] == 0 || px[3] == 255,
            "{what} is part covered at {},{}: alpha {}",
            i as u32 % face.w,
            i as u32 / face.w,
            px[3]
        );
    }
}

#[test]
fn every_face_of_the_picker_is_drawn_without_soft_edges() {
    assert_flat(&board_face(&cart(EMERALD, "BPEE")), "the board");
    assert_flat(&rom_marking_face(EMERALD), "the marking");
    for core in Core::ALL {
        assert_flat(&socket_face(core), "a socket");
        assert_flat(&chip_face(Some(core)), "a chip");
    }
    assert_flat(&chip_face(None), "the blank chip");
    assert_flat(&chip_shadow_face(), "the chip's shadow");
    assert_flat(&lid_shadow_face(), "the lid's shadow");
}

#[test]
fn the_marking_is_printed_on_the_rom_chip() {
    let face = board_face(&cart(EMERALD, "BPEE"));
    let ink = [0xbd, 0xbd, 0xbd];
    let inked = (ROM_Y..ROM_Y + ROM_H)
        .flat_map(|y| (ROM_X..ROM_X + ROM_W).map(move |x| (x, y)))
        .filter(|&(x, y)| rgb(&face, x, y) == ink)
        .count();
    assert!(
        inked > 100,
        "the ROM chip carries {inked} pixels of marking"
    );
}

/// A point on the 720x480 mockup, where it lands on this canvas.
fn mock(x: f32, y: f32) -> (f32, f32) {
    (x + stage_x(), y + stage_y())
}

#[test]
fn the_open_cart_starts_as_the_shelf_cart_and_lands_where_the_mockup_has_it() {
    let shelf = shelf_cart();
    assert_eq!(
        (shelf.x, shelf.w, shelf.h),
        (mock(180.0, 0.0).0, 360.0, 212.0)
    );
    assert_eq!(
        shelf.y + shelf.h / 2.0,
        (stage_y() + TOP_BAR_H + out_h() as f32 - BAR_H) / 2.0,
        "not centred between the bar across the top and the bar across the bottom"
    );
    assert_eq!(board_at(0.0), shelf_cart());
    let (x, y) = mock(174.0, 140.0);
    assert_eq!(
        board_at(1.0),
        Placed {
            x,
            y,
            w: 372.0,
            h: 208.0
        }
    );
    assert_eq!(board_at(1.7), board_at(1.0), "an overshoot kept growing");
}

#[test]
fn the_lid_leaves_level_and_rests_turned() {
    assert_eq!(lid_at(0.0), (shelf_cart(), 0.0));
    let (rest, turn) = lid_at(1.0);
    let (x, y) = mock(288.0, 44.0);
    assert_eq!(
        rest,
        Placed {
            x,
            y,
            w: 144.0,
            h: 81.0
        }
    );
    assert_eq!(turn, LID_TURN);
    assert!(
        (LID_TURN.to_degrees() + 5.0).abs() < 1e-4,
        "{}",
        LID_TURN.to_degrees()
    );
}

#[test]
fn the_beats_split_one_progress() {
    assert_eq!((slide_of(0.0), lift_of(0.0)), (0.0, 0.0));
    assert_eq!((slide_of(SLIDE_SHARE), lift_of(SLIDE_SHARE)), (1.0, 0.0));
    assert_eq!((slide_of(1.0), lift_of(1.0)), (1.0, 1.0));
    assert!((slide_of(SLIDE_SHARE / 2.0) - 0.5).abs() < 1e-5);
}

#[test]
fn the_back_half_stays_on_the_shelf_while_the_front_slides() {
    for p in [0.0, 0.1, 0.25, SLIDE_SHARE] {
        assert_eq!(board_at(p), shelf_cart(), "the back moved at progress {p}");
    }
}

#[test]
fn the_front_slides_up_a_fifth_before_it_lifts() {
    assert_eq!(SLIDE_UP, CART_H as f32 / 5.0);
    assert!(
        shelf_cart().y - SLIDE_UP > lid_at(1.0).0.y,
        "the slide ends above where the lid rests, so the lift would carry it back down"
    );
    let shelf = shelf_cart();
    let (slid, turn) = lid_at(SLIDE_SHARE);
    assert_eq!(
        slid,
        Placed {
            y: shelf.y - SLIDE_UP,
            ..shelf
        }
    );
    assert_eq!(turn, 0.0);
    let (half, turn) = lid_at(SLIDE_SHARE / 2.0);
    assert!(half.y < shelf.y && half.y > slid.y && turn == 0.0 && half.w == shelf.w);
}

#[test]
fn the_lift_starts_where_the_slide_ends() {
    let (slid, _) = lid_at(SLIDE_SHARE);
    let (next, _) = lid_at(SLIDE_SHARE + 0.001);
    assert!((next.y - slid.y).abs() < 0.5 && (next.w - slid.w).abs() < 0.5);
}

#[test]
fn the_boards_art_pixels_land_on_the_panel_at_twice() {
    let board = board_at(1.0);
    assert_eq!(on_board(board, 0.0, 0.0), mock(174.0, 140.0));
    assert_eq!(on_board(board, 186.0, 104.0), mock(546.0, 348.0));
    let (x, y) = on_board(board, 1.0, 1.0);
    assert_eq!((x - board.x, y - board.y), (2.0, 2.0));
}

#[test]
fn the_sockets_and_the_chip_sit_on_whole_art_pixels_of_the_board() {
    let whole = |v: f32| v.fract() == 0.0;
    for i in 0..2 {
        assert!(
            whole(SOCKET_U[i]) && whole(CHIP_U[i]),
            "socket {i} is off the grid"
        );
        assert!(
            SOCKET_U[i] < CHIP_U[i] && CHIP_U[i] + CHIP_W as f32 <= SOCKET_U[i] + SOCKET_W as f32,
            "the chip overhangs socket {i}"
        );
        assert!(SOCKET_U[i] + SOCKET_W as f32 <= BOARD_W as f32);
    }
    assert!(whole(SOCKET_V) && whole(CHIP_V));
    assert!(SOCKET_V < CHIP_V && CHIP_V + CHIP_H as f32 <= SOCKET_V + SOCKET_H as f32);
    assert!(
        SOCKET_U[0] + SOCKET_W as f32 <= SOCKET_U[1],
        "the sockets overlap"
    );
}

#[test]
fn a_socket_prints_its_cores_name_in_flat_half_strength_ink() {
    // the silkscreen's colour halfway to the board's
    let ink = [0x94, 0xc7, 0x91];
    let printed = |core: Core| -> Vec<(u32, u32)> {
        let face = socket_face(core);
        assert_eq!((face.w, face.h), (SOCKET_W, SOCKET_H));
        (0..face.h)
            .flat_map(|y| (0..face.w).map(move |x| (x, y)))
            .filter(|&(x, y)| alpha(&face, x, y) == 255 && rgb(&face, x, y) == ink)
            .collect()
    };
    let (mgba, gpsp) = (printed(Core::Mgba), printed(Core::Gpsp));
    assert!(
        mgba.len() > 20 && gpsp.len() > 20,
        "a socket's name is barely there"
    );
    assert_ne!(mgba, gpsp, "both sockets print the same name");
}

#[test]
fn the_chip_is_level_and_needs_no_room_to_turn() {
    for core in [None, Some(Core::Mgba), Some(Core::Gpsp)] {
        let face = chip_face(core);
        assert_eq!((face.w, face.h), (CHIP_W, CHIP_H));
        assert_eq!(alpha(&face, 0, 0), 0, "{core:?}'s pins reach its corner");
        assert_eq!(
            alpha(&face, 0, CHIP_H / 2),
            255,
            "{core:?}'s body does not reach its side"
        );
    }
}

#[test]
fn a_seated_chip_wears_its_name_and_a_flying_one_is_blank() {
    let light = |face: &CartFace| {
        face.rgba
            .chunks_exact(4)
            .filter(|p| p[0] > 200 && p[1] > 200 && p[2] > 200 && p[3] > 200)
            .count()
    };
    assert!(
        light(&chip_face(Some(Core::Mgba))) > 20,
        "mGBA left no name"
    );
    assert_eq!(light(&chip_face(None)), 0, "the blank chip says something");
    assert_ne!(
        chip_face(Some(Core::Mgba)).rgba,
        chip_face(Some(Core::Gpsp)).rgba,
        "both cores wear the same mark"
    );
}

#[test]
fn padding_moves_nothing() {
    let face = chip_face(Some(Core::Gpsp));
    let more = padded(&face, 3);
    assert_eq!((more.w, more.h), (face.w + 6, face.h + 6));
    for (x, y) in [(0, 0), (10, 7), (face.w - 1, face.h - 1)] {
        let a = ((y * face.w + x) * 4) as usize;
        let b = (((y + 3) * more.w + x + 3) * 4) as usize;
        assert_eq!(face.rgba[a..a + 4], more.rgba[b..b + 4]);
    }
}

#[test]
fn a_shadow_is_a_checker_of_black_with_its_corners_clear() {
    for (s, (w, h)) in [
        (chip_shadow_face(), (SHADOW_W, SHADOW_H)),
        (lid_shadow_face(), (LID_SHADOW_W, LID_SHADOW_H)),
    ] {
        assert_eq!((s.w, s.h), (w, h));
        for y in 0..h {
            for x in 0..w {
                if alpha(&s, x, y) == 0 {
                    continue;
                }
                assert_eq!(rgb(&s, x, y), [0, 0, 0], "a shadow pixel is not black");
                assert_eq!((x + y) % 2, 0, "{x},{y} is off the checker");
            }
        }
        for (x, y) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] {
            assert_eq!(alpha(&s, x, y), 0, "a shadow's corner {x},{y} is inked");
        }
        let middle = (0..w).filter(|&x| alpha(&s, x, h / 2) == 255).count() as u32;
        assert!(
            middle >= w / 2 - 1,
            "a shadow's middle row is a half shade: {middle} of {w}"
        );
    }
}

#[test]
fn the_seated_chips_name_is_printed_in_whole_pixels() {
    let red = |face: &CartFace, x: u32, y: u32| face.rgba[((y * face.w + x) * 4) as usize];
    for core in [Core::Mgba, Core::Gpsp] {
        let named = chip_face(Some(core));
        let blank = chip_face(None);
        let mut inks: Vec<u8> = (0..named.h)
            .flat_map(|y| (0..named.w).map(move |x| (x, y)))
            .filter(|&(x, y)| alpha(&named, x, y) == 255 && red(&named, x, y) != red(&blank, x, y))
            .map(|(x, y)| red(&named, x, y))
            .collect();
        assert!(inks.len() > 10, "{core:?}'s name is barely there");
        inks.sort_unstable();
        inks.dedup();
        assert_eq!(inks.len(), 1, "{core:?}'s name has soft edges: {inks:?}");
    }
}

fn snes_cart(stem: &str) -> Cart {
    Cart {
        platform: Platform::Snes,
        stem: stem.into(),
        rom: format!("Games/SNES/{stem}.sfc").into(),
        label: None,
        code: String::new(),
        shell: None,
        title: String::new(),
    }
}

/// How many runs of gold cross row `y` of `face`: the contacts along the board's edge.
fn contacts(face: &CartFace, y: u32) -> usize {
    let gold = |x: u32| near(rgb(face, x, y), [0xd4, 0xa9, 0x3c]);
    (0..face.w)
        .filter(|&x| gold(x) && (x == 0 || !gold(x - 1)))
        .count()
}

#[test]
fn a_snes_cart_opens_on_a_snes_board_in_its_own_plastic() {
    let gba = board_face(&cart(EMERALD, "BPEE"));
    let ntsc = board_face(&snes_cart("Super Metroid (USA)"));
    let sfc = board_face(&snes_cart("Rockman X (Japan)"));
    for (name, face) in [("American", &ntsc), ("Super Famicom", &sfc)] {
        assert_eq!((face.w, face.h), (BOARD_W, BOARD_H), "the {name} board");
        assert_flat(face, "the SNES board");
        assert_eq!(
            (0..face.h).map(|y| contacts(face, y)).max(),
            Some(23),
            "the {name} board has not the SNES cart's 23 contacts along its edge"
        );
    }
    assert_eq!((0..gba.h).map(|y| contacts(&gba, y)).max(), Some(30));
    assert!(
        near(rgb(&ntsc, 1, 50), SNES_SHELL.colour),
        "the American cart's shell is not its grey"
    );
    assert!(
        near(rgb(&sfc, 1, 50), SFC_SHELL.colour),
        "the Super Famicom cart's shell is not its grey"
    );
    // the Super Famicom's top corners are round, the American cart's square
    assert_eq!(alpha(&sfc, 1, 1), 0);
    assert_eq!(alpha(&ntsc, 1, 1), 255);
}

#[test]
fn the_snes_rom_chip_carries_its_marking_where_the_gba_s_does() {
    let face = board_face(&snes_cart("Super Metroid (USA)"));
    let ink = [0xbd, 0xbd, 0xbd];
    let inked = (ROM_Y..ROM_Y + ROM_H)
        .flat_map(|y| (ROM_X..ROM_X + ROM_W).map(move |x| (x, y)))
        .filter(|&(x, y)| rgb(&face, x, y) == ink)
        .count();
    assert!(
        inked > 100,
        "the ROM chip carries {inked} pixels of marking"
    );
}

#[test]
fn the_snes_boards_sockets_sit_on_whole_art_pixels_and_hold_their_chip() {
    let s = omega_ui::SNES_SOCKETS;
    let whole = |v: f32| v.fract() == 0.0;
    for i in 0..2 {
        assert!(
            whole(s.u[i]) && whole(s.chip_u(i)),
            "socket {i} is off the grid"
        );
        assert!(
            s.u[i] < s.chip_u(i) && s.chip_u(i) + s.chip_w as f32 <= s.u[i] + s.w as f32,
            "the chip overhangs socket {i}"
        );
        assert!(s.u[i] + s.w as f32 <= BOARD_W as f32);
    }
    assert!(s.v < s.chip_v() && s.chip_v() + s.chip_h as f32 <= s.v + s.h as f32);
    assert!(s.u[0] + s.w as f32 <= s.u[1], "the sockets overlap");
    assert_eq!(s.shadow_w(), s.chip_w + 4);
    assert_eq!(omega_ui::sockets_for(Platform::Snes), s);
    assert_eq!(omega_ui::sockets_for(Platform::Gba), omega_ui::GBA_SOCKETS);
    let gba = omega_ui::GBA_SOCKETS;
    assert_eq!(
        (gba.u, gba.v, gba.w, gba.h, gba.chip_w, gba.chip_h),
        (SOCKET_U, SOCKET_V, SOCKET_W, SOCKET_H, CHIP_W, CHIP_H),
        "the GBA board's sockets moved"
    );
    assert_eq!(gba.chip_u(0), CHIP_U[0]);
    assert_eq!(gba.chip_v(), CHIP_V);
    assert_eq!(gba.shadow_w(), SHADOW_W);
}

#[test]
fn each_core_is_made_for_its_boards_sockets() {
    for core in Core::ALL {
        let s = omega_ui::sockets_of(core);
        let socket = socket_face(core);
        let chip = chip_face(Some(core));
        assert_eq!((socket.w, socket.h), (s.w, s.h), "{core:?}'s socket");
        assert_eq!((chip.w, chip.h), (s.chip_w, s.chip_h), "{core:?}'s chip");
    }
    for (core, sockets) in [
        (Core::Mgba, omega_ui::GBA_SOCKETS),
        (Core::Snes9x, omega_ui::SNES_SOCKETS),
    ] {
        let blank = omega_ui::chip_face_in(sockets, None);
        assert_eq!((blank.w, blank.h), (sockets.chip_w, sockets.chip_h));
        assert_flat(&blank, "a blank chip");
        let shadow = omega_ui::chip_shadow_face_in(sockets);
        assert_eq!((shadow.w, shadow.h), (sockets.shadow_w(), SHADOW_H));
        assert_eq!(
            omega_ui::sockets_of(core),
            sockets,
            "{core:?} is on the wrong board"
        );
    }
    assert_eq!(
        chip_face(None).w,
        CHIP_W,
        "the GBA's blank chip changed size"
    );
}

/// Every core's name is whole on its chip and on its socket, with room either side of it, as
/// mGBA's has on the GBA board: four pixels before it and three after.
#[test]
fn every_cores_name_fits_its_chip_with_room_either_side() {
    let red = |face: &CartFace, x: u32, y: u32| face.rgba[((y * face.w + x) * 4) as usize];
    let ink_across = |named: &CartFace, blank: &CartFace| -> (u32, u32) {
        let xs: Vec<u32> = (0..named.h)
            .flat_map(|y| (0..named.w).map(move |x| (x, y)))
            .filter(|&(x, y)| alpha(named, x, y) == 255 && red(named, x, y) != red(blank, x, y))
            .map(|(x, _)| x)
            .collect();
        (
            *xs.iter().min().expect("no name"),
            *xs.iter().max().expect("no name"),
        )
    };
    let shown = |core: Core| -> Vec<String> {
        omega_ui::kit::pixel::letters(core.text(), omega_ui::kit::pixel::ADVANCE)
    };
    for core in Core::ALL {
        let s = omega_ui::sockets_of(core);
        let named = chip_face(Some(core));
        let (left, right) = ink_across(&named, &omega_ui::chip_face_in(s, None));
        assert!(
            left >= 3 && named.w - 1 - right >= 3,
            "{core:?}'s name {:?} runs from {left} to {right} on a chip {} wide",
            core.text(),
            named.w
        );
        assert!(
            core.text().chars().count() <= 6,
            "{:?} is too long for any chip",
            core.text()
        );
        assert!(!shown(core).is_empty());
    }
    assert_eq!(
        Core::ALL.map(Core::text),
        ["mGBA", "gpSP", "Snes9x", "Faust", "PCSX", "PCE", "FCEU", "GPGX"]
    );
}

#[test]
fn the_snes_board_leaves_the_sockets_places_bare() {
    let face = board_face(&snes_cart("Super Metroid (USA)"));
    let bare = [[0x3a, 0x9a, 0x3c], [0x58, 0xb3, 0x53]];
    let s = omega_ui::SNES_SOCKETS;
    for u in s.u {
        for y in s.v as u32..s.v as u32 + s.h {
            for x in u as u32..u as u32 + s.w {
                let c = rgb(&face, x, y);
                assert!(
                    bare.iter().any(|b| near(c, *b)),
                    "something {c:?} is in a socket's place at {x},{y}"
                );
            }
        }
    }
}

#[test]
fn a_cart_of_another_shape_opens_from_its_own_place_to_a_lid_of_its_shape() {
    use omega_ui::{lid_of, model_box, model_cart_at, slide_up, CartModel};
    assert_eq!(
        model_cart_at(CartModel::Gba, 10.0, 0.9),
        omega_ui::shelf_cart_at(10.0, 0.9)
    );
    assert_eq!(slide_up(CartModel::Gba), SLIDE_UP);
    for model in CartModel::ALL {
        let (mw, mh) = model_box(model);
        let shelf = model_cart_at(model, 100.0, 1.0);
        assert_eq!((shelf.w, shelf.h), (mw as f32, mh as f32), "{model:?}");
        assert_eq!(lid_of(model, shelf, 0.0), (shelf, 0.0), "{model:?}");
        let (rest, turn) = lid_of(model, shelf, 1.0);
        assert_eq!(turn, LID_TURN);
        let (gba, _) = lid_at(1.0);
        assert!(
            rest.h <= gba.h + 0.01,
            "{model:?}'s lid stands taller than the GBA's"
        );
        assert!(
            (rest.y + rest.h - gba.y - gba.h).abs() < 0.01,
            "{model:?}'s lid rests on another foot"
        );
        assert!(
            (rest.x + rest.w / 2.0 - gba.x - gba.w / 2.0).abs() < 0.01,
            "{model:?}'s lid is off the board's middle"
        );
        // squashed as the GBA's lid is squashed from its cart, and no more
        let squash = |w: f32, h: f32, cw: f32, ch: f32| (w / h) / (cw / ch);
        let gba_squash = squash(gba.w, gba.h, omega_ui::CART_W as f32, CART_H as f32);
        assert!(
            (squash(rest.w, rest.h, mw as f32, mh as f32) - gba_squash).abs() < 0.01,
            "{model:?}'s lid is not its cart's shape"
        );
    }
}
