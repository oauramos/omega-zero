use std::fmt;
use std::path::{Path, PathBuf};

use crate::gba::{header_code, header_title};
use crate::platform::Platform;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Cart {
    pub platform: Platform,
    pub stem: String,
    pub rom: PathBuf,
    pub label: Option<PathBuf>,
    pub title: String,
    pub code: String,
    pub shell: Option<crate::ShellChoice>,
}

#[derive(Debug)]
pub enum StoreError {
    Io(std::io::Error),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        StoreError::Io(e)
    }
}

pub fn scan(root: &Path) -> Result<Vec<Cart>, StoreError> {
    let read = |file: &str| std::fs::read_to_string(root.join(file)).unwrap_or_default();
    let shells = crate::cart_shell::layered(
        &read(crate::CART_SHELL_FILE),
        &read(crate::LABELS_SHELL_FILE),
    );
    let mut carts = Vec::new();
    for platform in Platform::ALL {
        let dir = root.join("Games").join(platform.dir_name());
        let found = match platform {
            Platform::Ps1 => crate::ps1::games(root, &dir)
                .map(|games| games.into_iter().map(|g| (g.stem, g.rom)).collect()),
            _ => flat(&dir, platform),
        };
        let found: Vec<(String, PathBuf)> = match found {
            Ok(found) => found,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                eprintln!("omega-zero: scan: {}: {e}", dir.display());
                continue;
            }
        };
        for (stem, rom) in found {
            let label = label_of(root, platform, &stem, &rom);
            let (title, code) = match platform {
                Platform::Gba => (
                    header_title(&rom).unwrap_or_default(),
                    header_code(&rom).unwrap_or_default(),
                ),
                Platform::Gb | Platform::Gbc => {
                    (crate::gb::title(&rom).unwrap_or_default(), String::new())
                }
                Platform::Snes => (
                    crate::snes::header(&rom)
                        .map(|h| h.title)
                        .unwrap_or_default(),
                    String::new(),
                ),
                Platform::Ps1 => (String::new(), crate::ps1::serial(&rom).unwrap_or_default()),
                // a HuCard, or an iNES image, carries no header to read a title or code from
                Platform::Pce | Platform::Nes => (String::new(), String::new()),
                Platform::Md => (
                    crate::md::header(&rom).map(|h| h.title).unwrap_or_default(),
                    String::new(),
                ),
            };
            carts.push(Cart {
                platform,
                shell: shells.get(&crate::cart_shell::key(&stem)).copied(),
                stem,
                title,
                code,
                label,
                rom,
            });
        }
    }
    carts.sort_by(|a, b| {
        (a.platform as u8, sort_key(&a.stem)).cmp(&(b.platform as u8, sort_key(&b.stem)))
    });
    Ok(carts)
}

/// The ROMs directly in `dir` that `platform` takes, each with its file's name.
fn flat(dir: &Path, platform: Platform) -> std::io::Result<Vec<(String, PathBuf)>> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let Ok(entry) = entry else {
            continue;
        };
        let rom = entry.path();
        if is_hidden(&rom) || !rom.is_file() || !platform.accepts(&rom) {
            continue;
        }
        if let Some(stem) = rom.file_stem().and_then(|s| s.to_str()) {
            found.push((stem.to_string(), rom));
        }
    }
    Ok(found)
}

/// The label of the cart named `stem`: `Labels/<system>/<stem>.png`, or for a PlayStation game
/// that has none under its own name, its first disc's.
fn label_of(root: &Path, platform: Platform, stem: &str, rom: &Path) -> Option<PathBuf> {
    let own = root
        .join("Labels")
        .join(platform.dir_name())
        .join(format!("{stem}.png"));
    if own.is_file() {
        return Some(own);
    }
    if platform != Platform::Ps1 {
        return None;
    }
    let first = crate::ps1::discs(rom).into_iter().next()?;
    Some(crate::ps1::disc_label(root, &first)).filter(|l| l.is_file())
}

pub fn sort_key(stem: &str) -> (u8, String) {
    (group_of(stem), stem.to_uppercase())
}

fn group_of(stem: &str) -> u8 {
    match stem.chars().find(|c| !c.is_whitespace()) {
        Some(c) if c.is_ascii_digit() => 0,
        Some(c) if c.is_alphabetic() => 1,
        _ => 2,
    }
}

pub fn initial(stem: &str) -> char {
    match group_of(stem) {
        1 => stem
            .chars()
            .find(|c| !c.is_whitespace())
            .and_then(|c| c.to_uppercase().next())
            .unwrap_or('#'),
        _ => '#',
    }
}

pub fn is_hidden(p: &Path) -> bool {
    p.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with('.'))
}
