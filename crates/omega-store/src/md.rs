//! A Mega Drive ROM's header: the 256 bytes from 0x100 the console checks for "SEGA", with the
//! game's titles and the regions it was sold in.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::Region;

/// What a Mega Drive ROM's header says about it.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Header {
    pub title: String,
    pub region: Option<Region>,
}

/// Where the header starts, and its fields from there: the system name, which starts with
/// "SEGA" (or a space and "SEGA" on a few carts), the title for Japan and the title for
/// everywhere else, and the regions, three letters of J, U and E or the newer hex digit of
/// them.
const AT: u64 = 0x100;
const LEN: usize = 0x100;
const SYSTEM: usize = 0x00;
const DOMESTIC: usize = 0x20;
const OVERSEAS: usize = 0x50;
const TITLE_LEN: usize = 0x30;
const REGIONS: usize = 0xf0;
const REGIONS_LEN: usize = 3;

impl Header {
    /// The header of the ROM in `rom`, the whole file as it is on the card.
    pub fn parse(rom: &[u8]) -> Option<Header> {
        let at = AT as usize;
        read(rom.get(at..at + LEN)?.try_into().ok()?)
    }
}

/// The header of the ROM at `path`, if it has one that reads.
pub fn header(path: &Path) -> Option<Header> {
    let mut f = File::open(path).ok()?;
    let mut block = [0u8; LEN];
    f.seek(SeekFrom::Start(AT)).ok()?;
    f.read_exact(&mut block).ok()?;
    read(&block)
}

/// Where the game at `path`, named `stem`, comes from: a region in its file name's tags wins
/// over its header.
pub fn region(path: &Path, stem: &str) -> Option<Region> {
    Region::from_tags(stem).or_else(|| header(path).and_then(|h| h.region))
}

fn read(block: &[u8; LEN]) -> Option<Header> {
    let system = &block[SYSTEM..SYSTEM + 16];
    if !(system.starts_with(b"SEGA") || system.starts_with(b" SEGA")) {
        return None;
    }
    let title = |at: usize| {
        let text: String = block[at..at + TITLE_LEN]
            .iter()
            .map(|&b| if b.is_ascii_graphic() { b as char } else { ' ' })
            .collect();
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    };
    let overseas = title(OVERSEAS);
    Some(Header {
        title: if overseas.is_empty() {
            title(DOMESTIC)
        } else {
            overseas
        },
        region: regions(&block[REGIONS..REGIONS + REGIONS_LEN]),
    })
}

/// The region a cart was made for, from the regions its header names: a cart sold only in
/// Japan is Japan's and one sold only in Europe Europe's; one sold in America, alone or with
/// the others, is taken for America's, whose carts most such games are found on.
fn regions(field: &[u8]) -> Option<Region> {
    let (mut japan, mut usa, mut europe) = (false, false, false);
    let letters = field.iter().filter(|b| !b.is_ascii_whitespace());
    let codes: Vec<u8> = letters.copied().collect();
    match codes.as_slice() {
        // the newer form, one hex digit: 1 Japan, 4 America, 8 Europe, added together
        [digit] if digit.is_ascii_hexdigit() && !matches!(digit, b'E' | b'e') => {
            let bits = (*digit as char).to_digit(16)?;
            japan = bits & 0x1 != 0;
            usa = bits & 0x4 != 0;
            europe = bits & 0x8 != 0;
        }
        codes => {
            for c in codes {
                match c.to_ascii_uppercase() {
                    b'J' => japan = true,
                    b'U' => usa = true,
                    b'E' => europe = true,
                    _ => {}
                }
            }
        }
    }
    match (japan, usa, europe) {
        (_, true, _) => Some(Region::Usa),
        (true, false, false) => Some(Region::Japan),
        (false, false, true) => Some(Region::Europe),
        (true, false, true) => Some(Region::Japan),
        (false, false, false) => None,
    }
}
