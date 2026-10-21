use crate::canvas::{out_h, out_w};
use crate::lcd3x::mask_texture_rgba8;
use crate::power::{screen_brightness, screen_glow, screen_rect_in};
use crate::quad::Quad;
use crate::shaders::{GAME_FRAG, GAME_SHARP_FRAG, RECT_VERT};
use crate::surface::GfxError;

/// The whole-number scale the LCD mask is drawn for, three panel pixels to a game pixel.
pub const SCALE: u32 = 3;
/// A GBA frame, the size the game texture starts at and the handheld layouts are drawn for. Game
/// Boy pictures arrive centred inside it; a SNES or PlayStation picture brings its own size, and
/// the texture takes it.
pub const SRC_W: u32 = 240;
pub const SRC_H: u32 = 160;

pub const WHOLE_TEXTURE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Filter {
    /// Three panel pixels to a game pixel, through the LCD mask.
    Lcd3x,
    /// Any other scale, each game pixel kept square with a one pixel blend where two meet.
    Sharp,
}

/// A look laid over the game's picture as it is drawn, whatever its filter: none, the
/// handhelds' LCD stripes, pixel grid or round dots, or a television's scanlines or CRT.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Look {
    #[default]
    Off,
    Lcd,
    Grid,
    Dot,
    Scanlines,
    Crt,
}

impl Look {
    pub const ALL: [Look; 6] = [
        Look::Off,
        Look::Lcd,
        Look::Grid,
        Look::Dot,
        Look::Scanlines,
        Look::Crt,
    ];

    /// The look's number in the look shader's `u_mode`.
    fn mode(self) -> f32 {
        match self {
            Look::Off => 0.0,
            Look::Lcd => 1.0,
            Look::Grid => 2.0,
            Look::Dot => 3.0,
            Look::Scanlines => 4.0,
            Look::Crt => 5.0,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Look::Off => "off",
            Look::Lcd => "lcd",
            Look::Grid => "grid",
            Look::Dot => "dot",
            Look::Scanlines => "scanlines",
            Look::Crt => "crt",
        }
    }

    pub fn parse(s: &str) -> Option<Look> {
        Look::ALL
            .into_iter()
            .find(|l| s.trim().eq_ignore_ascii_case(l.as_str()))
    }
}

/// How the game is laid on the canvas: the part of the game texture shown, the rect it fills
/// with the screen on, and how it is scaled into it.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct GameView {
    pub rect: [f32; 4],
    pub uv: [f32; 4],
    pub filter: Filter,
}

impl GameView {
    /// `uv` of the game texture over the whole canvas, through the LCD mask: the only view
    /// there was on 720x480.
    pub fn canvas(uv: [f32; 4]) -> GameView {
        GameView {
            rect: [0.0, 0.0, out_w() as f32, out_h() as f32],
            uv,
            filter: Filter::Lcd3x,
        }
    }
}

struct Lcd {
    prog: gl::types::GLuint,
    u_rect: gl::types::GLint,
    u_bright: gl::types::GLint,
    u_glow: gl::types::GLint,
    u_uv: gl::types::GLint,
    u_src: gl::types::GLint,
}

struct Sharp {
    prog: gl::types::GLuint,
    u_rect: gl::types::GLint,
    u_bright: gl::types::GLint,
    u_glow: gl::types::GLint,
    u_uv: gl::types::GLint,
    u_scale: gl::types::GLint,
    /// The size of the texture drawn, in texels.
    u_src: gl::types::GLint,
}

/// The look shader's program and uniforms: the sharp view's, and the look.
struct LookPass {
    sharp: Sharp,
    u_mode: gl::types::GLint,
}

pub struct GamePass {
    lcd: Lcd,
    sharp: Sharp,
    looks: LookPass,
    look: Look,
    game: gl::types::GLuint,
    /// The game texture's width and height, the last picture's.
    size: (u32, u32),
    mask: gl::types::GLuint,
    power: f32,
    view: Option<GameView>,
    still: Option<GameView>,
}

fn uniform(prog: gl::types::GLuint, name: &str) -> gl::types::GLint {
    crate::gl::uniform_location(prog, name)
}

impl GamePass {
    pub fn new() -> Result<Self, GfxError> {
        let lcd = crate::shaders::program(RECT_VERT, GAME_FRAG)?;
        let sharp = crate::shaders::program(RECT_VERT, GAME_SHARP_FRAG)?;
        let looks = crate::shaders::program(RECT_VERT, crate::shaders::GAME_LOOK_FRAG)?;
        let game = crate::gl::texture(SRC_W, SRC_H, gl::NEAREST, gl::CLAMP_TO_EDGE, gl::BGRA, None);
        let mask = crate::gl::texture(
            3,
            3,
            gl::NEAREST,
            gl::REPEAT,
            gl::RGBA,
            Some(&mask_texture_rgba8()),
        );
        unsafe {
            gl::UseProgram(lcd);
            gl::Uniform1i(uniform(lcd, "u_game"), 0);
            gl::Uniform1i(uniform(lcd, "u_mask"), 1);
            gl::Uniform2f(uniform(lcd, "u_target"), out_w() as f32, out_h() as f32);
            gl::UseProgram(sharp);
            gl::Uniform1i(uniform(sharp, "u_game"), 0);
            gl::Uniform2f(uniform(sharp, "u_target"), out_w() as f32, out_h() as f32);
            gl::UseProgram(looks);
            gl::Uniform1i(uniform(looks, "u_game"), 0);
            gl::Uniform2f(uniform(looks, "u_target"), out_w() as f32, out_h() as f32);
        }
        Ok(GamePass {
            lcd: Lcd {
                prog: lcd,
                u_rect: uniform(lcd, "u_rect"),
                u_bright: uniform(lcd, "u_bright"),
                u_glow: uniform(lcd, "u_glow"),
                u_uv: uniform(lcd, "u_uv"),
                u_src: uniform(lcd, "u_src"),
            },
            sharp: Sharp {
                prog: sharp,
                u_rect: uniform(sharp, "u_rect"),
                u_bright: uniform(sharp, "u_bright"),
                u_glow: uniform(sharp, "u_glow"),
                u_uv: uniform(sharp, "u_uv"),
                u_scale: uniform(sharp, "u_scale"),
                u_src: uniform(sharp, "u_src"),
            },
            looks: LookPass {
                sharp: Sharp {
                    prog: looks,
                    u_rect: uniform(looks, "u_rect"),
                    u_bright: uniform(looks, "u_bright"),
                    u_glow: uniform(looks, "u_glow"),
                    u_uv: uniform(looks, "u_uv"),
                    u_scale: uniform(looks, "u_scale"),
                    u_src: uniform(looks, "u_src"),
                },
                u_mode: uniform(looks, "u_mode"),
            },
            look: Look::Off,
            game,
            size: (SRC_W, SRC_H),
            mask,
            power: 1.0,
            view: None,
            still: None,
        })
    }

    /// The look the game's picture, and a still of it, is drawn through.
    pub fn set_look(&mut self, look: Look) {
        self.look = look;
    }

    pub fn set_power(&mut self, t: f32) {
        self.power = t.clamp(0.0, 1.0);
    }

    /// Shows `rect` of the game texture over the whole canvas, as omega-zero did before a canvas
    /// could be any other shape.
    pub fn set_source_rect(&mut self, rect: [f32; 4]) {
        self.view = Some(GameView::canvas(rect));
    }

    pub fn set_view(&mut self, view: GameView) {
        self.view = Some(view);
    }

    /// The view a still, a save state's screenshot, is shown in.
    pub fn set_still_view(&mut self, view: GameView) {
        self.still = Some(view);
    }

    /// A GBA-sized picture, 240x160.
    pub fn upload(&mut self, xrgb8888: &[u8]) {
        self.upload_sized(xrgb8888, (SRC_W, SRC_H));
    }

    /// A picture `size` pixels wide and high. The texture is made again at that size when the
    /// picture's size changes, as a PlayStation game's does between its menus and its game, and
    /// otherwise only written over, as a GBA frame always was.
    pub fn upload_sized(&mut self, xrgb8888: &[u8], size: (u32, u32)) {
        let (w, h) = size;
        if w == 0 || h == 0 || xrgb8888.len() < (w * h * 4) as usize {
            return;
        }
        let pixels = xrgb8888.as_ptr() as *const std::ffi::c_void;
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, self.game);
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            if size == self.size {
                gl::TexSubImage2D(
                    gl::TEXTURE_2D,
                    0,
                    0,
                    0,
                    w as i32,
                    h as i32,
                    gl::BGRA,
                    gl::UNSIGNED_BYTE,
                    pixels,
                );
            } else {
                gl::TexImage2D(
                    gl::TEXTURE_2D,
                    0,
                    crate::gl::internal_format(gl::BGRA),
                    w as i32,
                    h as i32,
                    0,
                    gl::BGRA,
                    gl::UNSIGNED_BYTE,
                    pixels,
                );
                self.size = size;
            }
        }
    }

    /// The game texture's width and height, the last picture's.
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    pub fn draw(&self, quad: &Quad) {
        let view = self.view.unwrap_or_else(|| GameView::canvas(WHOLE_TEXTURE));
        self.draw_source(self.game, self.size, quad, view);
    }

    /// A still, `size` texels, through the still's view.
    pub fn draw_still(&self, tex: gl::types::GLuint, size: (u32, u32), quad: &Quad) {
        let view = self
            .still
            .unwrap_or_else(|| GameView::canvas(WHOLE_TEXTURE));
        self.draw_source(tex, size, quad, view);
    }

    fn draw_source(&self, tex: gl::types::GLuint, src: (u32, u32), quad: &Quad, view: GameView) {
        let (src_w, src_h) = (src.0 as f32, src.1 as f32);
        let (x, y, w, h) = screen_rect_in(self.power, view.rect);
        let uv = view.uv;
        let sampled = if self.look == Look::Off {
            view.filter
        } else {
            Filter::Sharp
        };
        let sharp = if self.look == Look::Off {
            &self.sharp
        } else {
            unsafe {
                gl::UseProgram(self.looks.sharp.prog);
                gl::Uniform1f(self.looks.u_mode, self.look.mode());
            }
            &self.looks.sharp
        };
        match sampled {
            Filter::Lcd3x => unsafe {
                gl::UseProgram(self.lcd.prog);
                gl::Uniform4f(self.lcd.u_rect, x, y, w, h);
                gl::Uniform4f(self.lcd.u_uv, uv[0], uv[1], uv[2], uv[3]);
                gl::Uniform2f(
                    self.lcd.u_src,
                    view.rect[2] / SCALE as f32,
                    view.rect[3] / SCALE as f32,
                );
                gl::Uniform1f(self.lcd.u_bright, screen_brightness(self.power));
                gl::Uniform1f(self.lcd.u_glow, screen_glow(self.power));
                gl::ActiveTexture(gl::TEXTURE0);
                gl::BindTexture(gl::TEXTURE_2D, tex);
                gl::ActiveTexture(gl::TEXTURE1);
                gl::BindTexture(gl::TEXTURE_2D, self.mask);
                gl::ActiveTexture(gl::TEXTURE0);
            },
            Filter::Sharp => unsafe {
                gl::UseProgram(sharp.prog);
                gl::Uniform4f(sharp.u_rect, x, y, w, h);
                gl::Uniform4f(sharp.u_uv, uv[0], uv[1], uv[2], uv[3]);
                gl::Uniform2f(sharp.u_src, src_w, src_h);
                gl::Uniform2f(
                    sharp.u_scale,
                    view.rect[2] / (uv[2] * src_w),
                    view.rect[3] / (uv[3] * src_h),
                );
                gl::Uniform1f(sharp.u_bright, screen_brightness(self.power));
                gl::Uniform1f(sharp.u_glow, screen_glow(self.power));
                gl::ActiveTexture(gl::TEXTURE0);
                gl::BindTexture(gl::TEXTURE_2D, tex);
                filter(gl::LINEAR);
            },
        }
        quad.draw();
        if sampled == Filter::Sharp {
            unsafe { filter(gl::NEAREST) };
        }
    }
}

/// GLES2 has no sampler objects, so the sharp view sets the bound texture's own filter for
/// its draw and puts it back after.
unsafe fn filter(f: gl::types::GLenum) {
    gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, f as i32);
    gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, f as i32);
}

impl Drop for GamePass {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteTextures(1, &self.game);
            gl::DeleteTextures(1, &self.mask);
            gl::DeleteProgram(self.lcd.prog);
            gl::DeleteProgram(self.sharp.prog);
            gl::DeleteProgram(self.looks.sharp.prog);
        }
    }
}
