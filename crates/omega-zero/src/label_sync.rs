//! Cart labels from ScreenScraper. Each cart without a label is looked up by its ROM's CRC
//! with the player's own ScreenScraper account, and the scan of its real label, from the
//! ROM's own region first, is saved where the shelf looks for it. A cart ScreenScraper has
//! no label scan for keeps the label omega-zero draws.
//!
//! A PlayStation game is looked up disc by disc, never reading a disc image whole: by its cue
//! sheet's CRC, the serial printed on it, or its file's name and size (see [`lookups`]). Each
//! disc keeps the scan of its own printed face, and the first disc's is its game's as well.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use omega_store::{ps1, Cart, Platform, Region};

use crate::json::Json;

const API: &str = "https://api.screenscraper.fr/api2";
pub const SOFTNAME: &str = "omega-zero";
pub const ACCOUNT_FILE: &str = "Config/screenscraper.conf";

/// The widest label scan worth fetching for `platform`: half as wide again as its widest face's
/// label, never drawn bigger than a texel to a pixel, which leaves room for a scan of a wider
/// shape to still cover its faces. The cutter shrinks the scan to what they need once cut.
pub fn fetch_width(platform: Platform) -> u32 {
    let widest = omega_ui::face_labels(platform)
        .iter()
        .map(|&(w, _)| w)
        .max()
        .unwrap_or(0);
    (widest * 3).div_ceil(2)
}

/// The key ScreenScraper gives a program, separate from the player's account and required
/// with it on every request. It comes from whoever builds omega-zero, never from the source, and
/// build.rs masks it so it is not plain text in the binary: a courtesy, not a secret.
pub fn dev_key() -> Option<(&'static str, &'static str)> {
    static KEY: std::sync::OnceLock<Option<(String, String)>> = std::sync::OnceLock::new();
    KEY.get_or_init(|| {
        let (id, pw) = option_env!("OMEGA_SS_KEY")?.split_once(':')?;
        Some((unmask(id)?, unmask(pw)?))
    })
    .as_ref()
    .map(|(id, pw)| (id.as_str(), pw.as_str()))
}

/// Reads back what build.rs masked: each byte, as hex, crossed with a byte that steps on.
pub fn unmask(hex: &str) -> Option<String> {
    let bytes = (0..hex.len())
        .step_by(2)
        .enumerate()
        .map(|(i, at)| {
            let b = u8::from_str_radix(hex.get(at..at + 2)?, 16).ok()?;
            Some(b ^ 0x5a_u8.wrapping_add((i as u8).wrapping_mul(31)))
        })
        .collect::<Option<Vec<u8>>>()?;
    String::from_utf8(bytes).ok()
}

/// The player's ScreenScraper account, kept in Config/screenscraper.conf.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Account {
    pub user: String,
    pub password: String,
}

impl Account {
    pub fn load(root: &Path) -> Option<Account> {
        let text = std::fs::read_to_string(root.join(ACCOUNT_FILE)).ok()?;
        let field = |key: &str| {
            text.lines()
                .find_map(|l| l.strip_prefix(key)?.strip_prefix('='))
                .map(str::to_string)
        };
        let account = Account {
            user: field("user")?,
            password: field("password")?,
        };
        (!account.user.is_empty()).then_some(account)
    }

    pub fn save(&self, root: &Path) -> std::io::Result<()> {
        let path = root.join(ACCOUNT_FILE);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = format!("user={}\npassword={}\n", self.user, self.password);
        omega_store::atomic_write(&path, text.as_bytes())
    }

    pub fn forget(root: &Path) {
        let _ = std::fs::remove_file(root.join(ACCOUNT_FILE));
    }
}

/// A cart, or one disc of a PlayStation game, to find a label for.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Want {
    pub platform: Platform,
    /// The cart's name on the shelf, which a label saved is announced under.
    pub stem: String,
    /// The file looked up: the ROM, or the one disc of a PlayStation game.
    pub rom: PathBuf,
    /// The GBA header's game code, whose last letter is the cart's region, or the disc's
    /// serial; empty when there is none, or for a disc after the first, whose serial is read
    /// when it is looked up.
    pub code: String,
    /// Which of its game's discs `rom` is, from 0, and how many discs the game has: a cart is
    /// the one disc of its game.
    pub disc: usize,
    pub discs: usize,
}

impl Want {
    /// A cart that is its game's only disc.
    pub fn cart(cart: &Cart) -> Want {
        Want {
            platform: cart.platform,
            stem: cart.stem.clone(),
            rom: cart.rom.clone(),
            code: cart.code.clone(),
            disc: 0,
            discs: 1,
        }
    }

    /// Where a label found is saved: the cart's own label, or a disc's own one, the first
    /// disc's being its game's as well.
    pub fn label_paths(&self, root: &Path) -> Vec<PathBuf> {
        let own = label_path(root, self.platform, &self.stem);
        if self.platform != Platform::Ps1 {
            return vec![own];
        }
        let mut paths = vec![ps1::disc_label(root, &self.rom)];
        if self.disc == 0 && !paths.contains(&own) {
            paths.push(own);
        }
        paths
    }

    /// The name shown while this is looked up: the cart's without its tags, and which disc.
    pub fn title(&self) -> String {
        let name = title(&self.stem);
        if self.discs > 1 {
            format!("{name}, disc {}", self.disc + 1)
        } else {
            name
        }
    }
}

/// What a sync looks up of `carts`: each cart with no label, and each disc of a PlayStation
/// game that has none of its own, the first disc also when its game has none. Without a
/// `root` a disc counts as labelled when its game is.
pub fn wants<'a>(root: Option<&Path>, carts: impl IntoIterator<Item = &'a Cart>) -> Vec<Want> {
    let mut out = Vec::new();
    for cart in carts {
        if cart.platform != Platform::Ps1 {
            if cart.label.is_none() {
                out.push(Want::cart(cart));
            }
            continue;
        }
        let discs = ps1::discs(&cart.rom);
        for (i, disc) in discs.iter().enumerate() {
            let own = match root {
                Some(root) => ps1::disc_label(root, disc).is_file(),
                None => cart.label.is_some(),
            };
            if own && (i > 0 || cart.label.is_some()) {
                continue;
            }
            out.push(Want {
                rom: disc.clone(),
                code: if i == 0 {
                    cart.code.clone()
                } else {
                    String::new()
                },
                disc: i,
                discs: discs.len(),
                ..Want::cart(cart)
            });
        }
    }
    out
}

pub fn label_path(root: &Path, platform: Platform, stem: &str) -> PathBuf {
    root.join("Labels")
        .join(platform.dir_name())
        .join(format!("{stem}.png"))
}

/// ScreenScraper's number for each system.
pub fn system_id(platform: Platform) -> u32 {
    match platform {
        Platform::Gba => 12,
        Platform::Gb => 9,
        Platform::Gbc => 10,
        Platform::Snes => 4,
        Platform::Ps1 => 57,
        Platform::Pce => 31,
        Platform::Nes => 3,
        Platform::Md => 1,
    }
}

const TAG_REGIONS: [(&str, &str); 16] = [
    ("USA", "us"),
    ("Europe", "eu"),
    ("Japan", "jp"),
    ("World", "wor"),
    ("France", "fr"),
    ("Germany", "de"),
    ("Spain", "sp"),
    ("Italy", "it"),
    ("Australia", "au"),
    ("Brazil", "br"),
    ("Korea", "kr"),
    ("China", "cn"),
    ("Netherlands", "nl"),
    ("Sweden", "se"),
    ("UK", "uk"),
    ("Canada", "ca"),
];

/// Where ScreenScraper keeps the labels of European releases: under `eu` for some systems, and
/// under each country for others, the SNES among them.
const EUROPE: [&str; 9] = ["eu", "uk", "fr", "de", "sp", "it", "nl", "se", "au"];

/// The labels of North America's releases, whose SNES carts are the American shape, Brazil's
/// among them.
const AMERICA: [&str; 3] = ["us", "ca", "br"];

const CODE_REGIONS: [(char, &str); 10] = [
    ('E', "us"),
    ('P', "eu"),
    ('J', "jp"),
    ('D', "de"),
    ('F', "fr"),
    ('S', "sp"),
    ('I', "it"),
    ('U', "au"),
    ('K', "kr"),
    ('C', "cn"),
];

/// The regions to take a label from, best first: those named in the ROM's file name, then
/// the one its game code says, then a world release and the three big ones.
pub fn regions(stem: &str, code: &str) -> Vec<&'static str> {
    let mut out = tag_regions(stem);
    if let Some(letter) = code.chars().nth(3) {
        if let Some((_, r)) = CODE_REGIONS.iter().find(|(c, _)| *c == letter) {
            out.push(r);
        }
    }
    out.extend(["wor", "us", "eu", "jp"]);
    once_each(out)
}

/// The regions the tags of a file name name, in the order they come.
fn tag_regions(stem: &str) -> Vec<&'static str> {
    stem.split(['(', ')'])
        .skip(1)
        .step_by(2)
        .flat_map(|t| t.split(','))
        .filter_map(|tag| {
            TAG_REGIONS
                .iter()
                .find(|(name, _)| *name == tag.trim())
                .map(|(_, r)| *r)
        })
        .collect()
}

fn once_each(mut regions: Vec<&'static str>) -> Vec<&'static str> {
    let mut seen = Vec::new();
    regions.retain(|r| {
        let first = !seen.contains(r);
        seen.push(*r);
        first
    });
    regions
}

/// ScreenScraper's name for a region.
fn region_code(region: Region) -> Option<&'static str> {
    match region {
        Region::Usa => Some("us"),
        Region::Europe => Some("eu"),
        Region::Japan => Some("jp"),
        Region::Other => None,
    }
}

/// The regions to take the label of `want` from, best first.
///
/// A SNES cart's label must suit the shell it is drawn in, the American one for a game from
/// the USA and the Super Famicom's for any other, the European cart being shaped like the
/// Super Famicom's: the regions its file name names, then the one its header says, then the
/// others whose carts are its shape, then a world release, the American labels coming last on a
/// Super Famicom shell and the rest last on an American one. ScreenScraper keeps the SNES's
/// European labels under each country rather than under `eu`, so a European cart looks through
/// them all before a Japanese one.
///
/// A PlayStation disc's are those its file name names, then the one its serial says (and for a
/// European disc each European country after it), then a world release and the three big ones.
/// A Game Boy's are as [`regions`] says.
pub fn label_regions(want: &Want) -> Vec<&'static str> {
    let mut out = tag_regions(&want.stem);
    match want.platform {
        Platform::Gba | Platform::Gb | Platform::Gbc => return regions(&want.stem, &want.code),
        Platform::Snes | Platform::Md => {
            let region = match want.platform {
                Platform::Md => omega_store::md::region(&want.rom, &want.stem),
                _ => omega_store::snes::region(&want.rom, &want.stem),
            };
            out.extend(region.and_then(region_code));
            if region == Some(Region::Usa) {
                out.retain(|r| AMERICA.contains(r) || *r == "wor");
                out.extend(AMERICA);
                out.push("wor");
                out.extend(EUROPE);
                out.push("jp");
            } else {
                out.retain(|r| !AMERICA.contains(r));
                if region == Some(Region::Europe) {
                    out.extend(EUROPE);
                    out.push("jp");
                } else {
                    out.push("jp");
                    out.extend(EUROPE);
                }
                out.push("wor");
                out.extend(AMERICA);
            }
        }
        Platform::Ps1 => {
            let region = ps1::serial_region(&want.code);
            out.extend(region.and_then(region_code));
            if region == Some(Region::Europe) {
                out.extend(EUROPE);
            }
            out.extend(["wor", "us", "eu", "jp"]);
        }
        // the label suits the card it is drawn on, as the shell does: an American game's black
        // TurboChip takes an American label, any other game's HuCard a Japanese one
        Platform::Pce => {
            if Region::from_tags(&want.stem) == Some(Region::Usa) {
                out.retain(|r| AMERICA.contains(r) || *r == "wor");
                out.extend(["us", "wor", "jp", "eu"]);
            } else {
                out.retain(|r| !AMERICA.contains(r));
                out.extend(["jp", "wor", "eu", "us"]);
            }
        }
        // the label suits the cart it is drawn on: a Japanese game's Famicom cart takes a
        // Japanese label, any other game's grey NES cart its own region's, then the other's
        Platform::Nes => match Region::from_tags(&want.stem) {
            Some(Region::Japan) => {
                out.retain(|r| *r == "jp");
                out.extend(["jp", "wor", "us", "eu"]);
            }
            Some(Region::Europe) => {
                out.retain(|r| *r != "jp");
                out.extend(EUROPE);
                out.extend(["wor", "us", "jp"]);
            }
            _ => {
                out.retain(|r| *r != "jp");
                out.extend(["us", "wor", "eu", "jp"]);
            }
        },
    }
    once_each(out)
}

/// The CRC-32 a ROM is known by.
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, entry) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xedb8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        *entry = c;
    }
    !bytes.iter().fold(!0u32, |c, &b| {
        table[((c ^ b as u32) & 0xff) as usize] ^ (c >> 8)
    })
}

fn escape(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// What ScreenScraper is asked about one ROM or disc.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Lookup {
    pub system: u32,
    /// ScreenScraper's kind of file: `rom` for a cart, `iso` for a disc image.
    pub kind: &'static str,
    /// The CRC-32 of the file, for a file small enough to read whole: a cart, or a disc's cue
    /// sheet, which ScreenScraper knows by its own CRC. A disc image is never read whole.
    pub crc: Option<u32>,
    pub file_name: String,
    pub size: u64,
    /// The serial printed on a disc, such as `SLUS-00594`, the way ScreenScraper writes it.
    pub serial: Option<String>,
}

/// The largest disc file read whole for its CRC: a cue sheet is a few hundred bytes, and any
/// image far more than this.
const CUE_MAX: u64 = 64 * 1024;

/// The copier header some SNES and PC Engine dumps start with, which leaves the file 512 bytes past a whole
/// number of KiB.
const COPIER: u64 = 512;

/// The tag an iNES image starts with, and the header it starts, which No-Intro's CRCs, and so
/// most of ScreenScraper's, leave out.
const INES: &[u8] = b"NES\x1a";
const INES_HEADER: usize = 16;

/// The ways `want` is asked about, tried in turn while ScreenScraper knows none of them.
///
/// A cart is asked about by the CRC of all of it, its name and size; a SNES or PC Engine dump
/// with a copier header in front, or an NES image with its iNES header, which ScreenScraper keeps
/// both with and without, also by the CRC and size of the ROM after it. A PlayStation disc is
/// never read whole, hundreds of megabytes taking far too long on the console: it is asked about by its cue sheet's CRC (ScreenScraper keeps
/// the cue sheets of the discs it knows, a few hundred bytes each), then by the serial printed
/// on it, then by its file's name and size alone, which ScreenScraper matches as best it can.
/// Nothing when the file cannot be read.
pub fn lookups(want: &Want) -> Vec<Lookup> {
    let file_name = want
        .rom
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let system = system_id(want.platform);
    if want.platform != Platform::Ps1 {
        let Ok(bytes) = std::fs::read(&want.rom) else {
            return Vec::new();
        };
        let whole = Lookup {
            system,
            kind: "rom",
            crc: Some(crc32(&bytes)),
            file_name,
            size: bytes.len() as u64,
            serial: None,
        };
        let mut out = vec![whole.clone()];
        let copied = matches!(want.platform, Platform::Snes | Platform::Pce);
        let header = if copied && whole.size % 1024 == COPIER {
            COPIER as usize
        } else if want.platform == Platform::Nes && bytes.starts_with(INES) {
            INES_HEADER
        } else {
            0
        };
        if header > 0 && bytes.len() > header {
            let rom = &bytes[header..];
            out.push(Lookup {
                crc: Some(crc32(rom)),
                size: rom.len() as u64,
                ..whole
            });
        }
        return out;
    }
    let Ok(meta) = std::fs::metadata(&want.rom) else {
        return Vec::new();
    };
    let named = Lookup {
        system,
        kind: "iso",
        crc: None,
        file_name,
        size: meta.len(),
        serial: None,
    };
    let mut out = Vec::new();
    let is_cue = want
        .rom
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("cue"));
    if is_cue && meta.len() <= CUE_MAX {
        if let Ok(sheet) = std::fs::read(&want.rom) {
            out.push(Lookup {
                crc: Some(crc32(&sheet)),
                ..named.clone()
            });
        }
    }
    let serial = Some(want.code.clone())
        .filter(|c| !c.is_empty())
        .or_else(|| ps1::serial(&want.rom));
    if serial.is_some() {
        out.push(Lookup {
            serial,
            ..named.clone()
        });
    }
    out.push(named);
    out
}

pub fn game_url(key: (&str, &str), account: &Account, rom: &Lookup) -> String {
    let mut params = vec![
        ("devid", key.0.to_string()),
        ("devpassword", key.1.to_string()),
        ("softname", SOFTNAME.to_string()),
        ("output", "json".to_string()),
        ("ssid", account.user.clone()),
        ("sspassword", account.password.clone()),
    ];
    if let Some(crc) = rom.crc {
        params.push(("crc", format!("{crc:08X}")));
    }
    if let Some(serial) = &rom.serial {
        params.push(("serialnum", serial.clone()));
    }
    params.extend([
        ("systemeid", rom.system.to_string()),
        ("romtype", rom.kind.to_string()),
        ("romnom", rom.file_name.clone()),
        ("romtaille", rom.size.to_string()),
    ]);
    let query: Vec<String> = params
        .iter()
        .map(|(k, v)| format!("{k}={}", escape(v)))
        .collect();
    format!("{API}/jeuInfos.php?{}", query.join("&"))
}

/// A media URL from a reply, asked for as a PNG no wider than `platform`'s labels need.
pub fn media_url(url: &str, platform: Platform) -> String {
    format!("{url}&maxwidth={}&outputformat=png", fetch_width(platform))
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Media {
    pub kind: String,
    pub region: String,
    pub url: String,
    /// Which disc of a game on several the media shows, from 1, when the reply says.
    pub support: Option<u64>,
}

/// What ScreenScraper knows of a game it found.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Found {
    pub medias: Vec<Media>,
    /// Which disc of its game the file asked about is, from 1, when the reply says.
    pub disc: Option<u64>,
}

/// Whose credentials ScreenScraper turned away.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Who {
    Player,
    Program,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Reply {
    Game(Found),
    NotFound,
    Refused(Who),
    /// Too many requests at once: worth one more try after a pause.
    Busy,
    /// The account's lookups for today are spent.
    Spent,
    /// The API is closed, offline, or has blacklisted this version of omega-zero.
    Down,
    Other(u16),
}

/// How many lookups the account has left today.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Quota {
    pub left: u64,
    pub per_day: u64,
}

pub fn read_reply(status: u16, body: &[u8]) -> (Reply, Option<Quota>) {
    let text = String::from_utf8_lossy(body);
    if text.contains("Erreur de login") {
        let who = if text.to_lowercase().contains("développeur") {
            Who::Program
        } else {
            Who::Player
        };
        return (Reply::Refused(who), None);
    }
    let reply = match status {
        200 => None,
        403 => Some(Reply::Refused(Who::Program)),
        404 | 431 => Some(Reply::NotFound),
        429 => Some(Reply::Busy),
        430 => Some(Reply::Spent),
        401 | 423 | 426 => Some(Reply::Down),
        s => Some(Reply::Other(s)),
    };
    if let Some(reply) = reply {
        return (reply, None);
    }
    let Some(json) = Json::parse(&text) else {
        return (Reply::Other(status), None);
    };
    let response = json.get("response");
    let quota = response.and_then(|r| r.get("ssuser")).and_then(|u| {
        let per_day = u.get("maxrequestsperday")?.count()?;
        let today = u.get("requeststoday").and_then(Json::count).unwrap_or(0);
        Some(Quota {
            left: per_day.saturating_sub(today),
            per_day,
        })
    });
    let Some(game) = response.and_then(|r| r.get("jeu")) else {
        return (Reply::NotFound, quota);
    };
    let medias = game
        .get("medias")
        .map(Json::list)
        .unwrap_or_default()
        .iter()
        // the game's own media, not those of the system or the company it names
        .filter(|m| {
            m.get("parent")
                .and_then(Json::text)
                .is_none_or(|p| p == "jeu")
        })
        .filter_map(|m| {
            Some(Media {
                kind: m.get("type")?.text()?.to_string(),
                region: m
                    .get("region")
                    .and_then(Json::text)
                    .unwrap_or_default()
                    .to_string(),
                url: m.get("url")?.text()?.to_string(),
                support: m.get("support").and_then(Json::count),
            })
        })
        .collect();
    let disc = game
        .get("rom")
        .and_then(|r| r.get("romnumsupport"))
        .and_then(Json::count)
        .filter(|n| *n >= 1);
    (Reply::Game(Found { medias, disc }), quota)
}

/// The scan of the cart's own label, from the first of `regions` that has one, else from
/// any region.
pub fn pick_label<'a>(medias: &'a [Media], regions: &[&str]) -> Option<&'a Media> {
    pick(medias, |m| m.kind == "support-texture", regions)
}

/// Of the media that are `wanted`, the one from the first of `regions` that has one, else
/// from any region.
fn pick<'a>(
    medias: &'a [Media],
    wanted: impl Fn(&Media) -> bool,
    regions: &[&str],
) -> Option<&'a Media> {
    let scans: Vec<&Media> = medias.iter().filter(|m| wanted(m)).collect();
    regions
        .iter()
        .find_map(|r| scans.iter().find(|m| m.region == *r))
        .or_else(|| scans.first())
        .copied()
}

/// The kinds of media that show what `platform`'s shelf draws, best first. A cart's is the
/// scan of its label (`support-2D` being a cart ScreenScraper draws around that label). A
/// disc's is the scan of its printed face, round and seen square-on, which ScreenScraper keeps
/// as `support-texture` and draws `support-2D` from, so the drawn one is taken only when there
/// is no scan.
pub fn label_kinds(platform: Platform) -> &'static [&'static str] {
    match platform {
        Platform::Ps1 => &["support-texture", "support-2D"],
        _ => &["support-texture"],
    }
}

/// The media `want`'s label is taken from: of the best of [`label_kinds`] there is, the one
/// from the best of [`label_regions`]. A disc takes only the media of its own disc, as the
/// reply numbers it or else as its game's playlist does, media the reply does not number
/// counting as the first disc's.
pub fn pick_media<'a>(found: &'a Found, want: &Want) -> Option<&'a Media> {
    let regions = label_regions(want);
    let disc = found.disc.unwrap_or(want.disc as u64 + 1);
    let own_disc = |m: &Media| want.platform != Platform::Ps1 || m.support.unwrap_or(1) == disc;
    label_kinds(want.platform)
        .iter()
        .find_map(|kind| pick(&found.medias, |m| m.kind == *kind && own_disc(m), &regions))
}

/// How a sync went.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Tally {
    pub saved: usize,
    /// On ScreenScraper, but with no scan of its label.
    pub bare: usize,
    /// Not on ScreenScraper at all, such as a ROM hack.
    pub unknown: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SyncFail {
    /// This build of omega-zero was made without ScreenScraper's key.
    NoKey,
    Refused(Who),
    Spent,
    Down,
    Offline,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LabelNews {
    Working {
        done: usize,
        total: usize,
        title: String,
    },
    Saved {
        platform: Platform,
        stem: String,
    },
    Quota(Quota),
    Finished(Tally),
    Failed(SyncFail),
}

pub trait Http {
    /// The status and body of a GET, or nothing when the server could not be reached.
    fn get(&mut self, url: &str) -> Option<(u16, Vec<u8>)>;
}

/// HTTPS through the curl BaseOS ships, with the URL, which carries both passwords, handed
/// over on stdin rather than where `ps` would show it.
pub struct Curl;

impl Http for Curl {
    fn get(&mut self, url: &str) -> Option<(u16, Vec<u8>)> {
        let mut child = Command::new("curl")
            .args([
                "-s",
                "-L",
                "-m",
                "60",
                "-A",
                SOFTNAME,
                "-w",
                "\n%{http_code}",
                "-K",
                "-",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let config = format!(
            "url = \"{}\"\n",
            url.replace('\\', "\\\\").replace('"', "\\\"")
        );
        child.stdin.take()?.write_all(config.as_bytes()).ok()?;
        let out = child.wait_with_output().ok()?;
        let split = out.stdout.iter().rposition(|&b| b == b'\n')?;
        let status: u16 = std::str::from_utf8(&out.stdout[split + 1..])
            .ok()?
            .trim()
            .parse()
            .ok()?;
        (status != 0).then(|| (status, out.stdout[..split].to_vec()))
    }
}

fn is_png(bytes: &[u8]) -> bool {
    png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .is_ok_and(|r| r.info().width > 0 && r.info().height > 0)
}

fn keep(path: &Path, png: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    omega_store::atomic_write(path, png)
}

/// What ScreenScraper says to one request, asked once more after a pause when it is busy;
/// nothing when it cannot be reached.
fn ask(
    http: &mut dyn Http,
    url: &str,
    pause: &mut dyn FnMut(Duration),
    news: &mut dyn FnMut(LabelNews),
) -> Option<Reply> {
    let mut reply = Reply::Busy;
    for attempt in 0..2 {
        let (status, body) = http.get(url)?;
        let quota;
        (reply, quota) = read_reply(status, &body);
        if let Some(q) = quota {
            news(LabelNews::Quota(q));
        }
        if reply != Reply::Busy || attempt > 0 {
            break;
        }
        pause(Duration::from_secs(2));
    }
    Some(reply)
}

/// One sync: whose account, which carts, and the card their labels go on.
pub struct Sync<'a> {
    pub key: (&'a str, &'a str),
    pub account: &'a Account,
    pub root: &'a Path,
    pub wants: &'a [Want],
}

/// Looks up each cart and saves the label scans found, telling `news` as it goes. Stops early
/// when `stop` is set, and for good on any failure the next cart would meet too.
pub fn sync(
    job: &Sync,
    http: &mut dyn Http,
    stop: &AtomicBool,
    pause: &mut dyn FnMut(Duration),
    news: &mut dyn FnMut(LabelNews),
) {
    let Sync {
        key,
        account,
        root,
        wants,
    } = *job;
    let mut tally = Tally::default();
    for (done, want) in wants.iter().enumerate() {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        news(LabelNews::Working {
            done,
            total: wants.len(),
            title: want.title(),
        });
        let mut asked = None;
        for rom in lookups(want) {
            let url = game_url(key, account, &rom);
            let Some(reply) = ask(http, &url, pause, news) else {
                return news(LabelNews::Failed(SyncFail::Offline));
            };
            let unknown = reply == Reply::NotFound;
            asked = Some(reply);
            if !unknown {
                break;
            }
        }
        match asked {
            Some(Reply::Game(found)) => {
                let scan = pick_media(&found, want)
                    .and_then(|m| http.get(&media_url(&m.url, want.platform)))
                    .filter(|(status, body)| *status == 200 && is_png(body));
                let kept = scan.is_some_and(|(_, png)| {
                    want.label_paths(root)
                        .iter()
                        .all(|path| keep(path, &png).is_ok())
                });
                if kept {
                    tally.saved += 1;
                    news(LabelNews::Saved {
                        platform: want.platform,
                        stem: want.stem.clone(),
                    });
                } else {
                    tally.bare += 1;
                }
            }
            Some(Reply::NotFound | Reply::Other(_) | Reply::Busy) | None => tally.unknown += 1,
            Some(Reply::Refused(who)) => return news(LabelNews::Failed(SyncFail::Refused(who))),
            Some(Reply::Spent) => return news(LabelNews::Failed(SyncFail::Spent)),
            Some(Reply::Down) => return news(LabelNews::Failed(SyncFail::Down)),
        }
    }
    news(LabelNews::Finished(tally));
}

/// A cart's name for showing while it is looked up: its file name without the tags.
pub fn title(stem: &str) -> String {
    stem.split(" (").next().unwrap_or(stem).trim().to_string()
}

pub enum LabelJob {
    Sync { account: Account, wants: Vec<Want> },
    Stop,
}

pub trait LabelJobs: Send {
    /// This build can reach ScreenScraper at all.
    fn present(&self) -> bool;

    fn ask(&mut self, job: LabelJob);

    fn news(&mut self) -> Vec<LabelNews>;
}

/// Syncs on a thread of its own, one cart at a time, which is all a free account allows.
pub struct Worker {
    jobs: Sender<(Account, Vec<Want>)>,
    news: Receiver<LabelNews>,
    stop: Arc<AtomicBool>,
}

impl Worker {
    pub fn spawn(root: PathBuf, key: (&'static str, &'static str)) -> Worker {
        let (jobs, rx) = channel::<(Account, Vec<Want>)>();
        let (tx, news) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        std::thread::spawn(move || {
            for (account, wants) in rx {
                stopping.store(false, Ordering::Relaxed);
                let mut tell = |n| {
                    let _ = tx.send(n);
                };
                let job = Sync {
                    key,
                    account: &account,
                    root: &root,
                    wants: &wants,
                };
                sync(
                    &job,
                    &mut Curl,
                    &stopping,
                    &mut std::thread::sleep,
                    &mut tell,
                );
            }
        });
        Worker { jobs, news, stop }
    }
}

impl LabelJobs for Worker {
    fn present(&self) -> bool {
        true
    }

    fn ask(&mut self, job: LabelJob) {
        match job {
            LabelJob::Sync { account, wants } => {
                let _ = self.jobs.send((account, wants));
            }
            LabelJob::Stop => self.stop.store(true, Ordering::Relaxed),
        }
    }

    fn news(&mut self) -> Vec<LabelNews> {
        self.news.try_iter().collect()
    }
}

/// A build without ScreenScraper's key: it says so whenever asked to sync.
#[derive(Default)]
pub struct NoKey {
    news: Vec<LabelNews>,
}

impl LabelJobs for NoKey {
    fn present(&self) -> bool {
        false
    }

    fn ask(&mut self, job: LabelJob) {
        if let LabelJob::Sync { .. } = job {
            self.news.push(LabelNews::Failed(SyncFail::NoKey));
        }
    }

    fn news(&mut self) -> Vec<LabelNews> {
        std::mem::take(&mut self.news)
    }
}

pub fn label_jobs(root: &Path) -> Box<dyn LabelJobs> {
    match dev_key() {
        Some(key) => Box::new(Worker::spawn(root.to_path_buf(), key)),
        None => Box::new(NoKey::default()),
    }
}
