use std::path::Path;

use omega_gfx::{canvas, Canvas, Filter, GameView, SCALE, SRC_H, SRC_W, WHOLE_TEXTURE};
use omega_store::{ini, Platform, ScreenFilter};

pub const VIDEO_MODE_FILE: &str = "Config/video_mode.ini";

const _: () = assert!(
    SRC_W == omega_retro::GBA_W && SRC_H == omega_retro::GBA_H,
    "the handheld layouts are not drawn for the GBA screen a Game Boy picture is centred in"
);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum VideoMode {
    #[default]
    Actual,
    Stretch,
}

impl VideoMode {
    pub fn as_str(self) -> &'static str {
        match self {
            VideoMode::Actual => "actual",
            VideoMode::Stretch => "stretch",
        }
    }

    pub fn parse(s: &str) -> Option<VideoMode> {
        match s.trim().to_ascii_lowercase().as_str() {
            "actual" => Some(VideoMode::Actual),
            "stretch" => Some(VideoMode::Stretch),
            _ => None,
        }
    }
}

pub fn video_mode_for(root: &Path, stem: &str) -> VideoMode {
    ini::value(root, VIDEO_MODE_FILE, stem)
        .as_deref()
        .and_then(VideoMode::parse)
        .unwrap_or_default()
}

pub fn write_video_mode(root: &Path, stem: &str, mode: VideoMode) -> std::io::Result<()> {
    ini::write(root, VIDEO_MODE_FILE, stem, mode.as_str())
}

/// The part of the game texture a picture mode shows: a Game Boy game's own window when it is
/// stretched, otherwise all of it. A SNES or PlayStation picture fills the texture, which takes
/// each frame's size.
pub fn source_rect(platform: Platform, mode: VideoMode) -> [f32; 4] {
    if is_console(platform) {
        return WHOLE_TEXTURE;
    }
    let (w, h) = platform.picture();
    if mode == VideoMode::Actual || (w, h) == (SRC_W, SRC_H) || w > SRC_W || h > SRC_H {
        return WHOLE_TEXTURE;
    }
    let x = SRC_W.saturating_sub(w) / 2;
    let y = SRC_H.saturating_sub(h) / 2;
    [
        x as f32 / SRC_W as f32,
        y as f32 / SRC_H as f32,
        w as f32 / SRC_W as f32,
        h as f32 / SRC_H as f32,
    ]
}

/// How the game sits on the canvas for one picture mode: the live view, the view a save
/// state's screenshot is shown in, and the bands of canvas the picture leaves for the
/// console's body.
#[derive(Clone, PartialEq, Debug)]
pub struct Screen {
    pub view: GameView,
    pub still: GameView,
    pub bands: Vec<[f32; 4]>,
}

pub fn screen(platform: Platform, mode: VideoMode) -> Screen {
    screen_on(canvas(), platform, mode)
}

/// The screen for a game whose last picture was `frame` pixels wide and high. Only a SNES or
/// PlayStation game's layout follows its frames; a handheld's is always the GBA screen's.
pub fn screen_for(platform: Platform, mode: VideoMode, frame: (u32, u32)) -> Screen {
    screen_on_frame(canvas(), platform, mode, frame)
}

/// Whether `platform` is a console played on a television, whose pictures come in every size
/// and are laid out by their own, rather than a handheld drawn for the GBA's screen.
fn is_console(platform: Platform) -> bool {
    matches!(platform, Platform::Snes | Platform::Ps1)
}

/// The GBA frame at three panel pixels to a game pixel, which every canvas but 640x480 holds.
const FRAME: (f32, f32) = ((SRC_W * SCALE) as f32, (SRC_H * SCALE) as f32);

/// Above this much spare height the frame stops sitting on the top edge and stands a little
/// above the middle, as a screen does in a console's face.
const TOP_ROOM: f32 = 120.0;

/// The screen on canvas `c` before a game has drawn: a SNES or PlayStation game is laid out for
/// its system's usual picture.
pub fn screen_on(c: Canvas, platform: Platform, mode: VideoMode) -> Screen {
    screen_on_frame(c, platform, mode, platform.picture())
}

/// The screen on canvas `c` for a game whose last picture was `frame`.
pub fn screen_on_frame(
    c: Canvas,
    platform: Platform,
    mode: VideoMode,
    frame: (u32, u32),
) -> Screen {
    if is_console(platform) {
        return console_screen(c, platform, mode, frame);
    }
    let uv = source_rect(platform, mode);
    let actual = source_rect(platform, VideoMode::Actual);
    let (cw, ch) = (c.w as f32, c.h as f32);
    if cw >= FRAME.0 && ch >= FRAME.1 {
        let spare = ch - FRAME.1;
        let top = if spare < TOP_ROOM {
            0.0
        } else {
            (spare * 3.0 / 8.0).round()
        };
        let rect = [(cw - FRAME.0) / 2.0, top, FRAME.0, FRAME.1];
        return Screen {
            view: lcd(rect, uv),
            still: lcd(rect, actual),
            bands: around(c, rect),
        };
    }
    match platform {
        Platform::Gba | Platform::Snes | Platform::Ps1 => {
            let h = (cw * SRC_H as f32 / SRC_W as f32).round();
            let view = GameView {
                rect: [0.0, 0.0, cw, h],
                uv: actual,
                filter: Filter::Sharp,
            };
            Screen {
                view,
                still: view,
                bands: around(c, view.rect),
            }
        }
        Platform::Gb | Platform::Gbc => {
            let whole = [(cw - FRAME.0) / 2.0, (ch - FRAME.1) / 2.0, FRAME.0, FRAME.1];
            let view = match mode {
                VideoMode::Actual => lcd(whole, actual),
                VideoMode::Stretch => lcd([0.0, 0.0, cw, ch], uv),
            };
            Screen {
                view,
                still: lcd(whole, actual),
                bands: around(c, view.rect),
            }
        }
    }
}

/// `screen` as Settings' Screen Filter draws it: SHARP takes every picture through the sharp
/// filter, the ones through the LCD mask too; LCD leaves it as it is.
pub fn filtered(mut screen: Screen, filter: ScreenFilter) -> Screen {
    if filter == ScreenFilter::Sharp {
        screen.view.filter = Filter::Sharp;
        screen.still.filter = Filter::Sharp;
    }
    screen
}

fn lcd(rect: [f32; 4], uv: [f32; 4]) -> GameView {
    GameView {
        rect,
        uv,
        filter: Filter::Lcd3x,
    }
}

/// The SNES's lines are 256 pixels wide, its hi-res modes' 512 pixels half as wide each.
const SNES_W: f32 = 256.0;

/// A SNES or PlayStation game on a television's screen, centred with the console's body around
/// it, every frame size drawn through the sharp filter into the same rect.
///
/// The SNES at its actual size is twice its 256 pixels and its lines, 512x448 (512x478 for a
/// game showing its overscan), on every canvas: 3x would not fit any of them. A hi-res frame,
/// 512 pixels a line, or an interlaced one, twice the lines, fills the same rect. Stretched, the
/// SNES takes a television's 4:3, as large as the canvas holds.
///
/// The PlayStation at its actual size is that 4:3: 640x480 on 640x480, where its usual 320x240
/// is exactly 2x, centred on 720x480, and 720x540 across 720x720. Stretched, it fills the
/// canvas.
fn console_screen(c: Canvas, platform: Platform, mode: VideoMode, frame: (u32, u32)) -> Screen {
    let (cw, ch) = (c.w as f32, c.h as f32);
    let (w, h) = match (platform, mode) {
        (Platform::Snes, VideoMode::Actual) => snes_whole(cw, ch, frame.1),
        (Platform::Ps1, VideoMode::Stretch) => (cw, ch),
        _ => four_three(cw, ch),
    };
    let rect = [((cw - w) / 2.0).round(), ((ch - h) / 2.0).round(), w, h];
    let view = GameView {
        rect,
        uv: WHOLE_TEXTURE,
        filter: Filter::Sharp,
    };
    Screen {
        view,
        still: view,
        bands: around(c, rect),
    }
}

/// The SNES picture at the largest whole scale the canvas holds, for a frame `lines` high: an
/// interlaced frame's lines count as the half they each are of the picture.
fn snes_whole(cw: f32, ch: f32, lines: u32) -> (f32, f32) {
    let lines = match lines {
        0 => 224,
        l if l > 256 => l / 2,
        l => l,
    } as f32;
    let scale = (cw / SNES_W).min(ch / lines).floor().max(1.0);
    let (w, h) = (SNES_W * scale, lines * scale);
    if w <= cw && h <= ch {
        (w, h)
    } else {
        four_three(cw, ch)
    }
}

/// The largest 4:3 rect the canvas holds.
fn four_three(cw: f32, ch: f32) -> (f32, f32) {
    if cw * 3.0 > ch * 4.0 {
        ((ch * 4.0 / 3.0).round(), ch)
    } else {
        (cw, (cw * 3.0 / 4.0).round())
    }
}

/// The parts of the canvas outside `r`.
fn around(c: Canvas, r: [f32; 4]) -> Vec<[f32; 4]> {
    let (cw, ch) = (c.w as f32, c.h as f32);
    let (x0, y0) = (r[0].max(0.0), r[1].max(0.0));
    let (x1, y1) = ((r[0] + r[2]).min(cw), (r[1] + r[3]).min(ch));
    [
        [0.0, 0.0, cw, y0],
        [0.0, y1, cw, ch - y1],
        [0.0, y0, x0, y1 - y0],
        [x1, y0, cw - x1, y1 - y0],
    ]
    .into_iter()
    .filter(|b| b[2] > 0.0 && b[3] > 0.0)
    .collect()
}

/// Where the game's own picture lands on the canvas in `view`: the whole frame for a GBA
/// game, and only its 160x144 window for a Game Boy one, whose frame is bordered.
pub fn picture_rect(platform: Platform, view: GameView) -> [f32; 4] {
    let window = source_rect(platform, VideoMode::Stretch);
    let [rx, ry, rw, rh] = view.rect;
    let [u, v, uw, vh] = view.uv;
    let x0 = rx + (window[0] - u) / uw * rw;
    let y0 = ry + (window[1] - v) / vh * rh;
    let x1 = rx + (window[0] + window[2] - u) / uw * rw;
    let y1 = ry + (window[1] + window[3] - v) / vh * rh;
    [x0, y0, x1 - x0, y1 - y0]
}
