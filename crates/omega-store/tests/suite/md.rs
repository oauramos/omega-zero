use omega_store::md::{header, region, Header};
use omega_store::{scan, Platform, Region};

/// A ROM with a Mega Drive header naming `overseas` (and `domestic` for Japan), sold in
/// `regions`, behind 256 bytes of vectors.
fn rom(system: &[u8], domestic: &str, overseas: &str, regions: &[u8]) -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    let field = |rom: &mut Vec<u8>, at: usize, len: usize, text: &[u8]| {
        rom[at..at + len].fill(b' ');
        rom[at..at + text.len()].copy_from_slice(text);
    };
    field(&mut rom, 0x100, 16, system);
    field(&mut rom, 0x120, 48, domestic.as_bytes());
    field(&mut rom, 0x150, 48, overseas.as_bytes());
    field(&mut rom, 0x1f0, 16, regions);
    rom
}

#[test]
fn a_header_gives_the_overseas_title_and_where_the_cart_was_sold() {
    let h = Header::parse(&rom(
        b"SEGA GENESIS",
        "SONIC THE HEDGEHOG",
        "SONIC   THE    HEDGEHOG",
        b"JUE",
    ))
    .unwrap();
    assert_eq!(h.title, "SONIC THE HEDGEHOG", "the title's padding is kept");
    assert_eq!(h.region, Some(Region::Usa));
    let japan = Header::parse(&rom(b"SEGA MEGA DRIVE", "PUYO PUYO", "", b"J")).unwrap();
    assert_eq!(
        japan.title, "PUYO PUYO",
        "no overseas title, and the domestic one not taken"
    );
    assert_eq!(japan.region, Some(Region::Japan));
    let europe = Header::parse(&rom(b" SEGA MEGA DRIVE", "", "PROBOTECTOR", b"E")).unwrap();
    assert_eq!(
        europe.region,
        Some(Region::Europe),
        "a space before SEGA is a header too"
    );
    assert!(Header::parse(&rom(b"NOT A CART", "", "X", b"J")).is_none());
    assert!(Header::parse(&[0u8; 0x80]).is_none());
}

#[test]
fn the_newer_hex_region_is_read_as_its_bits() {
    let of = |code: &[u8]| Header::parse(&rom(b"SEGA", "", "X", code)).unwrap().region;
    assert_eq!(of(b"1"), Some(Region::Japan));
    assert_eq!(of(b"4"), Some(Region::Usa));
    assert_eq!(of(b"8"), Some(Region::Europe));
    assert_eq!(of(b"F"), Some(Region::Usa));
    assert_eq!(of(b"9"), Some(Region::Japan));
    assert_eq!(
        of(b"E"),
        Some(Region::Europe),
        "E alone is Europe's letter, not a hex digit"
    );
    assert_eq!(of(b"   "), None);
}

#[test]
fn a_mega_drive_game_is_scanned_with_its_header_s_title_and_a_tag_wins_its_region() {
    let d = tempfile::tempdir().unwrap();
    let games = d.path().join("Games/MD");
    std::fs::create_dir_all(&games).unwrap();
    let sonic = games.join("Sonic the Hedgehog (Japan).md");
    std::fs::write(&sonic, rom(b"SEGA", "", "SONIC THE HEDGEHOG", b"JUE")).unwrap();
    std::fs::write(games.join("Notes.txt"), b"not a game").unwrap();
    let carts = scan(d.path()).unwrap();
    assert_eq!(carts.len(), 1);
    assert_eq!(carts[0].platform, Platform::Md);
    assert_eq!(carts[0].title, "SONIC THE HEDGEHOG");
    assert_eq!(header(&sonic).unwrap().region, Some(Region::Usa));
    assert_eq!(
        region(&sonic, "Sonic the Hedgehog (Japan)"),
        Some(Region::Japan)
    );
    assert_eq!(region(&sonic, "Sonic the Hedgehog"), Some(Region::Usa));
}
