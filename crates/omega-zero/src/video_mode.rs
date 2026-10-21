use std::path::Path;

use omega_gfx::{canvas, Canvas, Filter, GameView, SCALE, SRC_H, SRC_W, WHOLE_TEXTURE};
use omega_store::{ini, Platform};

pub const VIDEO_MODE_FILE: &str = "Config/video_mode.ini";

const _: () = assert!(
    SRC_W == omega_retro::GBA_W && SRC_H == omega_retro::GBA_H,
    "the handheld layouts are not drawn for the GBA screen a Game Boy picture is centred in"
);

/// How a game's picture is scaled. A handheld's is its actual size or stretched over the
/// canvas. A television's console's is its actual size, at the largest whole scale the canvas
/// holds; a television's 4:3, as large as the canvas holds; or stretched over all of it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum VideoMode {
    #[default]
    Actual,
    Aspect,
    Stretch,
}

impl VideoMode {
    pub fn as_str(self) -> &'static str {
        match self {
            VideoMode::Actual => "actual",
            VideoMode::Aspect => "aspect",
            VideoMode::Stretch => "stretch",
        }
    }

    pub fn parse(s: &str) -> Option<VideoMode> {
        match s.trim().to_ascii_lowercase().as_str() {
            "actual" => Some(VideoMode::Actual),
            "aspect" => Some(VideoMode::Aspect),
            "stretch" => Some(VideoMode::Stretch),
            _ => None,
        }
    }

    /// The mode a game of `platform` starts in: the PlayStation's 4:3, whose games change their
    /// picture's size between menus and play and would otherwise jump with it; anything else's
    /// actual size.
    pub fn default_for(platform: Platform) -> VideoMode {
        match platform {
            Platform::Ps1 => VideoMode::Aspect,
            _ => VideoMode::Actual,
        }
    }

    /// The mode SELECT+Y goes to from this one on a television's console: actual size, 4:3,
    /// stretched and round again.
    pub fn next(self) -> VideoMode {
        match self {
            VideoMode::Actual => VideoMode::Aspect,
            VideoMode::Aspect => VideoMode::Stretch,
            VideoMode::Stretch => VideoMode::Actual,
        }
    }
}

/// The mode every game of `platform` is played in: the one its system was last left in, kept
/// under the system's folder name, else its system's first.
pub fn video_mode_for(root: &Path, platform: Platform) -> VideoMode {
    ini::value(root, VIDEO_MODE_FILE, platform.dir_name())
        .as_deref()
        .and_then(VideoMode::parse)
        .unwrap_or_else(|| VideoMode::default_for(platform))
}

pub fn write_video_mode(root: &Path, platform: Platform, mode: VideoMode) -> std::io::Result<()> {
    ini::write(root, VIDEO_MODE_FILE, platform.dir_name(), mode.as_str())
}

/// The part of the game texture a picture mode shows: a Game Boy game's own window when it is
/// stretched, otherwise all of it. A SNES or PlayStation picture fills the texture, which takes
/// each frame's size.
pub fn source_rect(platform: Platform, mode: VideoMode) -> [f32; 4] {
    if is_console(platform) {
        return WHOLE_TEXTURE;
    }
    let (w, h) = platform.picture();
    if mode != VideoMode::Stretch || (w, h) == (SRC_W, SRC_H) || w > SRC_W || h > SRC_H {
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
    matches!(
        platform,
        Platform::Snes | Platform::Ps1 | Platform::Pce | Platform::Nes | Platform::Md
    )
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
        Platform::Gba
        | Platform::Snes
        | Platform::Ps1
        | Platform::Pce
        | Platform::Nes
        | Platform::Md => {
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
                VideoMode::Actual | VideoMode::Aspect => lcd(whole, actual),
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

fn lcd(rect: [f32; 4], uv: [f32; 4]) -> GameView {
    GameView {
        rect,
        uv,
        filter: Filter::Lcd3x,
    }
}

/// The SNES's lines are 256 pixels wide, its hi-res modes' 512 pixels half as wide each.
const SNES_W: f32 = 256.0;

/// The Mega Drive's are 320 pixels wide; its narrower mode's 256 are wider dots across the same
/// picture, as a television showed them.
const MD_W: f32 = 320.0;

/// A television's console's game on a television's screen, centred with the console's body
/// around it, every frame size drawn through the sharp filter into the same rect.
///
/// The SNES at its actual size is twice its 256 pixels and its lines, 512x448 (512x478 for a
/// game showing its overscan), on every canvas: 3x would not fit any of them. A hi-res frame,
/// 512 pixels a line, or an interlaced one, twice the lines, fills the same rect. Kept to its
/// aspect, any console takes a television's 4:3, as large as the canvas holds; stretched, it
/// fills the canvas.
///
/// The NES and the PC Engine are laid out as the SNES is. The NES's lines are 256 pixels, and it
/// shows the 224 of them a television did; the PC Engine's are as wide, and its wider modes, up
/// to 512 pixels a line, are finer dots across the same picture.
///
/// The Mega Drive's actual size is its 320 pixel lines at a whole scale, 640x448 at twice, its
/// narrower mode's 256 pixels filling the same rect.
///
/// The PlayStation at its actual size is its lines at the largest whole scale the canvas holds,
/// a television's 4:3 across: its usual 320x240 is 640x480 on 640x480 and 720x480, as its 4:3
/// is there.
fn console_screen(c: Canvas, platform: Platform, mode: VideoMode, frame: (u32, u32)) -> Screen {
    let (cw, ch) = (c.w as f32, c.h as f32);
    let (w, h) = match (platform, mode) {
        (Platform::Ps1, VideoMode::Actual) => ps1_whole(cw, ch, frame.1),
        (Platform::Md, VideoMode::Actual) => whole(cw, ch, MD_W, frame.1),
        (_, VideoMode::Actual) => whole(cw, ch, SNES_W, frame.1),
        (_, VideoMode::Aspect) => four_three(cw, ch),
        (_, VideoMode::Stretch) => (cw, ch),
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

/// A picture whose lines are `line_w` pixels wide at the largest whole scale the canvas holds,
/// for a frame `lines` high: an interlaced frame's lines count as the half they each are of the
/// picture.
fn whole(cw: f32, ch: f32, line_w: f32, lines: u32) -> (f32, f32) {
    let lines = match lines {
        0 => 224,
        l if l > 256 => l / 2,
        l => l,
    } as f32;
    let scale = (cw / line_w).min(ch / lines).floor().max(1.0);
    let (w, h) = (line_w * scale, lines * scale);
    if w <= cw && h <= ch {
        (w, h)
    } else {
        four_three(cw, ch)
    }
}

/// The PlayStation picture at the largest whole scale of its lines the canvas holds, as wide as
/// a television's 4:3 makes it: an interlaced frame's lines count as the half they each are.
fn ps1_whole(cw: f32, ch: f32, lines: u32) -> (f32, f32) {
    let lines = match lines {
        0 => 240,
        l if l > 288 => l / 2,
        l => l,
    } as f32;
    let h = lines * (ch / lines).floor().max(1.0);
    let w = (h * 4.0 / 3.0).round();
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
