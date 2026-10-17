mod canvas;
mod displaycal;
mod draw;
mod fbdev;
mod fbo;
mod gl;
mod grade;
#[cfg(target_os = "macos")]
mod headless;
#[cfg(feature = "host")]
mod host;
mod lcd3x;
mod pipeline;
mod power;
mod quad;
mod shaders;
mod surface;

pub use canvas::{canvas, out_h, out_w, set_canvas, Canvas, CANVAS_ENV};
pub use draw::{spun_x, Draw, MeshId, MeshVertex, TexId, FOCAL};
pub use fbdev::{egl_error, panel_mode, panel_size, FbdevSurface};
pub use fbo::{Bezel, Compositor, BACKDROP};
pub use grade::{blue_light_gain, BLUE_LIGHT_MAX};
#[cfg(target_os = "macos")]
pub use headless::HeadlessSurface;
#[cfg(feature = "host")]
pub use host::{host_rotation, HostSurface};
pub use lcd3x::{lcd3x_mask, mask_texture_rgba8};
pub use pipeline::{Filter, GameView, SCALE, SRC_H, SRC_W, WHOLE_TEXTURE};
pub use power::{screen_brightness, screen_glow, screen_rect_in, screen_scale, screen_width};
pub use surface::{
    blit_rect, blit_rect_fit, fit_rect, fit_scale, panel_rect, GfxError, Rotation, Surface,
};
