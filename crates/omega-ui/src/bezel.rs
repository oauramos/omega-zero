use std::path::{Path, PathBuf};

use omega_gfx::{out_h, out_w};
use omega_store::Platform;

use crate::plate::UndoFace;

/// The console's body, where a picture that does not fill the canvas leaves it bare: the
/// dark grey of a GBA's lens around its screen.
pub const BEZEL_FILL: [f32; 4] = [
    0x19 as f32 / 255.0,
    0x19 as f32 / 255.0,
    0x19 as f32 / 255.0,
    1.0,
];

/// The grey a mark without colours of its own is printed in.
pub const BEZEL_INK: [u8; 3] = [0x8a, 0x8a, 0x92];

/// How wide the screen's black rim is where the picture meets the body.
pub const RIM_W: f32 = 5.0;

const GBA_MARK: &[u8] = include_bytes!("../assets/bezel_gba.png");
const GB_MARK: &[u8] = include_bytes!("../assets/bezel_gb.png");
const GBC_MARK: &[u8] = include_bytes!("../assets/bezel_gbc.png");

/// How tall the mark is printed in a band `band_h` tall.
pub fn mark_height(band_h: f32) -> u32 {
    (band_h * 0.625).round().clamp(16.0, 36.0) as u32
}

/// The console's wordmark for `platform`, `height` pixels tall: in its own white and black
/// where it has them, otherwise in [`BEZEL_INK`].
pub fn bezel_mark_face(platform: Platform, height: u32) -> Option<UndoFace> {
    let png = match platform {
        Platform::Gba => GBA_MARK,
        Platform::Gb => GB_MARK,
        Platform::Gbc => GBC_MARK,
        Platform::Snes | Platform::Ps1 | Platform::Pce | Platform::Nes | Platform::Md => {
            return None
        }
    };
    let mark = decode(png)?;
    let (sw, sh) = (mark.w, mark.h);
    let h = height.clamp(1, sh);
    let w = ((sw as f32 * h as f32 / sh as f32).round() as u32).max(1);
    let alpha = shrink(&mark.alpha, (sw, sh), (w, h));
    let rgba = match &mark.grey {
        Some(grey) => {
            let lit: Vec<u8> = grey
                .iter()
                .zip(&mark.alpha)
                .map(|(g, a)| (*g as u32 * *a as u32 / 255) as u8)
                .collect();
            shrink(&lit, (sw, sh), (w, h))
                .iter()
                .zip(&alpha)
                .flat_map(|(l, a)| {
                    let g = (*l as u32 * 255)
                        .checked_div(*a as u32)
                        .map_or(0, |g| g.min(255) as u8);
                    [g, g, g, *a]
                })
                .collect()
        }
        None => alpha
            .iter()
            .flat_map(|a| [BEZEL_INK[0], BEZEL_INK[1], BEZEL_INK[2], *a])
            .collect(),
    };
    Some(UndoFace { rgba, w, h })
}

/// Where a `w` by `h` mark is printed in `band`: across its middle, centred in a narrow band
/// and a little under its top edge in a deep one, as under a console's screen.
pub fn mark_rect(band: [f32; 4], w: u32, h: u32) -> [f32; 4] {
    let (w, h) = (w as f32, h as f32);
    let x = (band[0] + (band[2] - w) / 2.0).round();
    let y = if band[3] <= 80.0 {
        band[1] + (band[3] - h) / 2.0
    } else {
        band[1] + band[3] * 0.3
    };
    [x, y.round(), w, h]
}

/// The screen's black rim along the side of `picture` that `band` meets, reaching round its
/// corners, and what is left of the band beside it. A band that meets the picture nowhere
/// has no rim.
pub fn rim(band: [f32; 4], picture: [f32; 4]) -> ([f32; 4], Option<[f32; 4]>) {
    let [bx, by, bw, bh] = band;
    let [px, py, pw, ph] = picture;
    let meets = |a: f32, b: f32| (a - b).abs() < 0.5;
    let across = || {
        let x0 = bx.max(px - RIM_W);
        (x0, bw.min(px + pw + RIM_W - bx) - (x0 - bx))
    };
    let down = || {
        let y0 = by.max(py - RIM_W);
        (y0, bh.min(py + ph + RIM_W - by) - (y0 - by))
    };
    if meets(by, py + ph) {
        let (r, (x, w)) = (RIM_W.min(bh), across());
        ([bx, by + r, bw, bh - r], Some([x, by, w, r]))
    } else if meets(by + bh, py) {
        let (r, (x, w)) = (RIM_W.min(bh), across());
        ([bx, by, bw, bh - r], Some([x, by + bh - r, w, r]))
    } else if meets(bx + bw, px) {
        let (r, (y, h)) = (RIM_W.min(bw), down());
        ([bx, by, bw - r, bh], Some([bx + bw - r, y, r, h]))
    } else if meets(bx, px + pw) {
        let (r, (y, h)) = (RIM_W.min(bw), down());
        ([bx + r, by, bw - r, bh], Some([bx, y, r, h]))
    } else {
        (band, None)
    }
}

struct Mark {
    w: u32,
    h: u32,
    alpha: Vec<u8>,
    /// The grey of each pixel, for a mark drawn in its own white and black.
    grey: Option<Vec<u8>>,
}

fn decode(png: &[u8]) -> Option<Mark> {
    let mut reader = png::Decoder::new(std::io::Cursor::new(png))
        .read_info()
        .ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    let (w, h) = (info.width, info.height);
    let n = (w * h) as usize;
    match info.color_type {
        png::ColorType::Grayscale => {
            buf.truncate(n);
            Some(Mark {
                w,
                h,
                alpha: buf,
                grey: None,
            })
        }
        png::ColorType::GrayscaleAlpha => Some(Mark {
            w,
            h,
            alpha: buf[..2 * n].iter().skip(1).step_by(2).copied().collect(),
            grey: Some(buf[..2 * n].iter().step_by(2).copied().collect()),
        }),
        _ => None,
    }
}

/// Averages every source pixel under each destination pixel, so a mark drawn large keeps its
/// thin strokes when it is printed small.
fn shrink(src: &[u8], (sw, sh): (u32, u32), (w, h): (u32, u32)) -> Vec<u8> {
    let (fx, fy) = (sw as f32 / w as f32, sh as f32 / h as f32);
    let mut out = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        let (y0, y1) = (y as f32 * fy, (y + 1) as f32 * fy);
        for x in 0..w {
            let (x0, x1) = (x as f32 * fx, (x + 1) as f32 * fx);
            let (mut sum, mut area) = (0.0f32, 0.0f32);
            for sy in y0.floor() as u32..(y1.ceil() as u32).min(sh) {
                let cover_y = (y1.min(sy as f32 + 1.0) - y0.max(sy as f32)).max(0.0);
                for sx in x0.floor() as u32..(x1.ceil() as u32).min(sw) {
                    let cover_x = (x1.min(sx as f32 + 1.0) - x0.max(sx as f32)).max(0.0);
                    let a = cover_x * cover_y;
                    sum += src[(sy * sw + sx) as usize] as f32 * a;
                    area += a;
                }
            }
            out.push((sum / area.max(f32::EPSILON)).round() as u8);
        }
    }
    out
}

/// The overlay for `platform` on this canvas, from `root`: Overlays/<system>/<w>x<h>.png,
/// laid over the whole canvas, so it is only used at exactly the canvas's size.
pub fn overlay_face(root: &Path, platform: Platform) -> Option<UndoFace> {
    let (w, h) = (out_w(), out_h());
    let path = overlay_path(root, platform);
    let rgba = crate::art::exact(&path, w, h)?;
    Some(UndoFace { rgba, w, h })
}

pub fn overlay_path(root: &Path, platform: Platform) -> PathBuf {
    root.join("Overlays")
        .join(platform.dir_name())
        .join(format!("{}x{}.png", out_w(), out_h()))
}

/// The most of the game an overlay may hide: a frame drawn for another layout, or with an
/// opaque centre, would cover the picture, so it is refused.
const OVERLAY_COVER: f32 = 0.01;

/// Whether a `w` by `h` RGBA overlay leaves `picture` clear, so the game shows through it.
pub fn overlay_clear(rgba: &[u8], w: u32, h: u32, picture: [f32; 4]) -> bool {
    let x0 = picture[0].max(0.0).round() as u32;
    let y0 = picture[1].max(0.0).round() as u32;
    let x1 = ((picture[0] + picture[2]).round().max(0.0) as u32).min(w);
    let y1 = ((picture[1] + picture[3]).round().max(0.0) as u32).min(h);
    if x1 <= x0 || y1 <= y0 || rgba.len() < (w * h * 4) as usize {
        return false;
    }
    let total = ((x1 - x0) * (y1 - y0)) as f32;
    let covered = (y0..y1)
        .flat_map(|y| (x0..x1).map(move |x| (y * w + x) as usize * 4 + 3))
        .filter(|&a| rgba[a] > 8)
        .count() as f32;
    covered / total <= OVERLAY_COVER
}
