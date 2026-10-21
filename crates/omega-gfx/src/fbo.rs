use crate::canvas::{out_h, out_w};
use crate::draw::{Draw, Sprites, TexId};
use crate::grade::blue_light_gain;
use crate::pipeline::{GamePass, GameView, Look};
use crate::quad::Quad;
use crate::shaders::{BLIT_FRAG, BLIT_VERT, BLIT_VERT_TURNED};
use crate::surface::{blit_rect, panel_rect, GfxError, Rotation, Surface};

pub const BACKDROP: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// The console's body around a picture that does not fill the canvas: the bands it leaves,
/// painted `fill`, the screen's black rims where they meet the picture, the marks printed on
/// them, and an overlay laid over all of it.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Bezel {
    pub bands: Vec<[f32; 4]>,
    pub fill: [f32; 4],
    pub rims: Vec<[f32; 4]>,
    pub marks: Vec<(TexId, [f32; 4])>,
    pub overlay: Option<TexId>,
}

pub struct Compositor {
    fbo: gl::types::GLuint,
    tex: gl::types::GLuint,
    blit: gl::types::GLuint,
    u_gain: gl::types::GLint,
    gain: [f32; 3],
    shake: f32,
    quad: Quad,
    game: GamePass,
    sprites: Sprites,
    bezel: Option<Bezel>,
    rotation: Rotation,
    turned: Option<Turned>,
}

/// The blit for a panel on its side, built the first time one is asked for.
struct Turned {
    prog: gl::types::GLuint,
    u_gain: gl::types::GLint,
    u_turn: gl::types::GLint,
}

impl Compositor {
    pub fn new(surface: &dyn Surface) -> Result<Self, GfxError> {
        crate::gl::load(surface);
        let blit = crate::shaders::program(BLIT_VERT, BLIT_FRAG)?;
        let tex = crate::gl::texture(
            out_w(),
            out_h(),
            gl::NEAREST,
            gl::CLAMP_TO_EDGE,
            gl::RGBA,
            None,
        );
        unsafe {
            let mut fbo = 0;
            gl::GenFramebuffers(1, &mut fbo);
            gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                tex,
                0,
            );
            let status = gl::CheckFramebufferStatus(gl::FRAMEBUFFER);
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            if status != gl::FRAMEBUFFER_COMPLETE {
                gl::DeleteFramebuffers(1, &fbo);
                gl::DeleteTextures(1, &tex);
                gl::DeleteProgram(blit);
                return Err(GfxError::Framebuffer(status));
            }
            gl::UseProgram(blit);
            gl::Uniform1i(crate::gl::uniform_location(blit, "u_tex"), 0);
            let u_gain = crate::gl::uniform_location(blit, "u_gain");

            Ok(Compositor {
                fbo,
                tex,
                blit,
                u_gain,
                gain: blue_light_gain(0),
                shake: 0.0,
                quad: Quad::new(),
                game: GamePass::new()?,
                sprites: Sprites::new()?,
                bezel: None,
                rotation: Rotation::R0,
                turned: None,
            })
        }
    }

    /// Uploads a GBA-sized picture, 240x160, as the game.
    pub fn upload_game(&mut self, xrgb8888: &[u8]) {
        self.game.upload(xrgb8888);
    }

    /// Uploads a picture of any size, `size` pixels wide and high, as the game: the game
    /// texture takes the picture's size, so its views show all of it.
    pub fn upload_game_sized(&mut self, xrgb8888: &[u8], size: (u32, u32)) {
        self.game.upload_sized(xrgb8888, size);
    }

    /// The size of the picture last uploaded as the game.
    pub fn game_size(&self) -> (u32, u32) {
        self.game.size()
    }

    pub fn draw_game(&mut self) {
        self.game.draw(&self.quad);
    }

    pub fn draw_list(&mut self, items: &[Draw]) {
        let mut from = 0;
        for (i, item) in items.iter().enumerate() {
            match *item {
                Draw::Game => {
                    self.sprites.draw(&items[from..i], &self.quad);
                    self.game.draw(&self.quad);
                    self.draw_bezel(true);
                }
                Draw::Shot { tex } => {
                    self.sprites.draw(&items[from..i], &self.quad);
                    if let (Some(gl_tex), Some(size)) =
                        (self.sprites.source(tex), self.sprites.source_size(tex))
                    {
                        self.game.draw_still(gl_tex, size, &self.quad);
                    }
                    self.draw_bezel(false);
                }
                _ => continue,
            }
            from = i + 1;
        }
        self.sprites.draw(&items[from..], &self.quad);
    }

    pub fn create_texture(&mut self, w: u32, h: u32, rgba: &[u8]) -> TexId {
        self.sprites.create_texture(w, h, rgba)
    }

    pub fn create_mesh(&mut self, vertices: &[crate::draw::MeshVertex]) -> crate::draw::MeshId {
        self.sprites.create_mesh(vertices)
    }

    pub fn create_texture_nearest(&mut self, w: u32, h: u32, rgba: &[u8]) -> TexId {
        self.sprites.create_texture_nearest(w, h, rgba)
    }

    pub fn update_texture(&mut self, id: TexId, w: u32, h: u32, rgba: &[u8]) {
        self.sprites.update_texture(id, w, h, rgba);
    }

    pub fn set_blue_light(&mut self, step: u8) {
        self.gain = blue_light_gain(step);
    }

    pub fn set_game_look(&mut self, look: Look) {
        self.game.set_look(look);
    }

    pub fn set_screen_power(&mut self, t: f32) {
        self.game.set_power(t);
    }

    pub fn set_game_source_rect(&mut self, rect: [f32; 4]) {
        self.game.set_source_rect(rect);
    }

    pub fn set_game_view(&mut self, view: GameView) {
        self.game.set_view(view);
    }

    pub fn set_still_view(&mut self, view: GameView) {
        self.game.set_still_view(view);
    }

    pub fn set_bezel(&mut self, bezel: Option<Bezel>) {
        self.bezel = bezel;
    }

    /// The bezel over the picture just drawn. A screenshot gets the bare bands: whatever is
    /// drawn over it, such as the switcher's legend, would sit on top of the marks.
    fn draw_bezel(&self, marked: bool) {
        let Some(b) = &self.bezel else {
            return;
        };
        let mut items: Vec<Draw> = b
            .bands
            .iter()
            .map(|r| Draw::Rect {
                x: r[0],
                y: r[1],
                w: r[2],
                h: r[3],
                colour: b.fill,
            })
            .collect();
        items.extend(b.rims.iter().map(|r| Draw::Rect {
            x: r[0],
            y: r[1],
            w: r[2],
            h: r[3],
            colour: [0.0, 0.0, 0.0, 1.0],
        }));
        items.extend(b.marks.iter().filter(|_| marked).map(|(tex, r)| Draw::Tex {
            x: r[0],
            y: r[1],
            w: r[2],
            h: r[3],
            tex: *tex,
            alpha: 1.0,
        }));
        if let Some(tex) = b.overlay {
            items.push(Draw::Tex {
                x: 0.0,
                y: 0.0,
                w: out_w() as f32,
                h: out_h() as f32,
                tex,
                alpha: 1.0,
            });
        }
        self.sprites.draw(&items, &self.quad);
    }

    pub fn set_shake(&mut self, dx: f32) {
        self.shake = dx;
    }

    pub fn read_frame(&self) -> Vec<u8> {
        let stride = out_w() as usize * 4;
        let mut buf = vec![0u8; stride * out_h() as usize];
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo);
            gl::PixelStorei(gl::PACK_ALIGNMENT, 1);
            gl::ReadPixels(
                0,
                0,
                out_w() as i32,
                out_h() as i32,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                buf.as_mut_ptr() as *mut std::ffi::c_void,
            );
        }
        let mut top_down = Vec::with_capacity(buf.len());
        for row in buf.chunks_exact(stride).rev() {
            top_down.extend_from_slice(row);
        }
        top_down
    }

    pub fn begin_frame(&mut self) {
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo);
            gl::Viewport(0, 0, out_w() as i32, out_h() as i32);
            gl::ClearColor(BACKDROP[0], BACKDROP[1], BACKDROP[2], BACKDROP[3]);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }
    }

    /// Turns the picture for a panel mounted on its side.
    pub fn set_rotation(&mut self, rotation: Rotation) -> Result<(), GfxError> {
        if rotation != Rotation::R0 && self.turned.is_none() {
            let prog = crate::shaders::program(BLIT_VERT_TURNED, BLIT_FRAG)?;
            unsafe {
                gl::UseProgram(prog);
                gl::Uniform1i(crate::gl::uniform_location(prog, "u_tex"), 0);
            }
            self.turned = Some(Turned {
                prog,
                u_gain: crate::gl::uniform_location(prog, "u_gain"),
                u_turn: crate::gl::uniform_location(prog, "u_turn"),
            });
        }
        self.rotation = rotation;
        Ok(())
    }

    pub fn end_frame(&mut self, window: (u32, u32)) {
        if let (Some(t), true) = (&self.turned, self.rotation != Rotation::R0) {
            let (x, y, w, h) = panel_rect(window, self.rotation, self.shake);
            let turn = self.rotation.uv_turn();
            unsafe {
                gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
                gl::Viewport(0, 0, window.0 as i32, window.1 as i32);
                gl::ClearColor(0.0, 0.0, 0.0, 1.0);
                gl::Clear(gl::COLOR_BUFFER_BIT);
                gl::Viewport(x, y, w, h);
                gl::UseProgram(t.prog);
                gl::ActiveTexture(gl::TEXTURE0);
                gl::BindTexture(gl::TEXTURE_2D, self.tex);
                gl::Uniform3f(t.u_gain, self.gain[0], self.gain[1], self.gain[2]);
                gl::UniformMatrix2fv(t.u_turn, 1, gl::FALSE, turn.as_ptr());
            }
            self.quad.draw();
            return;
        }
        let (x, y, w, h) = blit_rect(window, self.shake);
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            gl::Viewport(0, 0, window.0 as i32, window.1 as i32);
            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            gl::Viewport(x, y, w, h);
            gl::UseProgram(self.blit);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, self.tex);
            gl::Uniform3f(self.u_gain, self.gain[0], self.gain[1], self.gain[2]);
        }
        self.quad.draw();
    }
}

impl Drop for Compositor {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteFramebuffers(1, &self.fbo);
            gl::DeleteTextures(1, &self.tex);
            gl::DeleteProgram(self.blit);
            if let Some(t) = &self.turned {
                gl::DeleteProgram(t.prog);
            }
        }
    }
}
