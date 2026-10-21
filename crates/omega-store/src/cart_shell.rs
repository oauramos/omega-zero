use std::collections::HashMap;

use unicode_normalization::UnicodeNormalization;

pub const CART_SHELL_FILE: &str = "Config/cart_shell.ini";
pub const LABELS_SHELL_FILE: &str = "Labels/cart_shell.ini";

/// The shape a cart is drawn in, where its system has more than one: `auto` leaves it to the
/// ROM. The Game Boy pak comes notched (the grey and black paks) or rounded (the Game Boy
/// Color's); the SNES cart comes as the American cart (`ntsc`) or the Super Famicom's (`sfc`),
/// which Europe's carts share. A shape of another system's is no choice and draws as `auto`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Outline {
    Auto,
    Notched,
    Rounded,
    Ntsc,
    Sfc,
    /// The NES Game Pak of America and Europe.
    Nes,
    /// Japan's Famicom cart.
    Famicom,
    /// The Mega Drive cart of Japan and Europe.
    MegaDrive,
    /// The Genesis cart of America.
    Genesis,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ShellFinish {
    Solid,
    Clear,
    Glitter,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct ShellChoice {
    pub outline: Outline,
    pub colour: [u8; 3],
    pub finish: ShellFinish,
}

impl ShellChoice {
    pub fn parse(value: &str) -> Option<ShellChoice> {
        let mut words = value.split_whitespace();
        let outline = match words.next()? {
            "auto" => Outline::Auto,
            "notched" => Outline::Notched,
            "rounded" => Outline::Rounded,
            "ntsc" => Outline::Ntsc,
            "sfc" => Outline::Sfc,
            "nes" => Outline::Nes,
            "famicom" => Outline::Famicom,
            "megadrive" => Outline::MegaDrive,
            "genesis" => Outline::Genesis,
            _ => return None,
        };
        let hex = words.next()?;
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let colour = [0, 2, 4].map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0));
        let finish = match words.next()? {
            "solid" => ShellFinish::Solid,
            "clear" => ShellFinish::Clear,
            "glitter" => ShellFinish::Glitter,
            _ => return None,
        };
        words.next().is_none().then_some(ShellChoice {
            outline,
            colour,
            finish,
        })
    }

    pub fn to_value(&self) -> String {
        let outline = match self.outline {
            Outline::Auto => "auto",
            Outline::Notched => "notched",
            Outline::Rounded => "rounded",
            Outline::Ntsc => "ntsc",
            Outline::Sfc => "sfc",
            Outline::Nes => "nes",
            Outline::Famicom => "famicom",
            Outline::MegaDrive => "megadrive",
            Outline::Genesis => "genesis",
        };
        let finish = match self.finish {
            ShellFinish::Solid => "solid",
            ShellFinish::Clear => "clear",
            ShellFinish::Glitter => "glitter",
        };
        let [r, g, b] = self.colour;
        format!("{outline} {r:02x}{g:02x}{b:02x} {finish}")
    }
}

pub fn key(stem: &str) -> String {
    stem.nfc().collect()
}

pub fn choices(text: &str) -> HashMap<String, ShellChoice> {
    crate::ini::parse(text)
        .into_iter()
        .filter_map(|(stem, value)| Some((key(&stem), ShellChoice::parse(&value)?)))
        .collect()
}

pub fn layered(system: &str, labels: &str) -> HashMap<String, ShellChoice> {
    let mut out = choices(system);
    for (stem, value) in crate::ini::parse(labels) {
        if value.trim() == "auto" {
            out.remove(&key(&stem));
        } else if let Some(choice) = ShellChoice::parse(&value) {
            out.insert(key(&stem), choice);
        }
    }
    out
}
