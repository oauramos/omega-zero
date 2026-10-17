use std::time::{Duration, Instant};

use omega_gfx::{out_h, out_w, Bezel, Compositor, Draw, TexId};
use omega_input::{InputSource, Millis};
use omega_power::{Platform, Power};
use omega_store::Cart;
use omega_ui::{
    bezel_mark_face, chip_face, chip_face_in, chip_shadow_face, chip_shadow_face_in,
    generated_face, lid_shadow_face, mark_height, mark_rect, model_shadow, rim, socket_face,
    wallpaper_face, wears_label, CartFace, CartMeshes, CartModel, UndoFace, BEZEL_FILL,
    SNES_SOCKETS,
};
use omega_ui::{overlay_clear, overlay_face, overlay_path};

use crate::face_builder::FaceBuilder;
use crate::frontend::faces::upload_nearest;
use crate::label_cutter::LabelCutter;
use crate::list_view::ListView;
use crate::opening::Facts;
use crate::screens::about::AboutView;
use crate::session::Session;
use crate::wallpaper;

mod faces;
mod lone;
mod sync_about;
mod sync_background;
mod sync_bar;
mod sync_bluetooth;
mod sync_clock;
mod sync_controls;
mod sync_core_picker;
mod sync_disc;
mod sync_hud;
mod sync_info;
mod sync_keyboard;
mod sync_labels;
mod sync_link;
mod sync_network;
mod sync_opening;
mod sync_power;
mod sync_settings;
mod sync_start_notice;
mod sync_switcher;
mod sync_top_bar;

use lone::Baked;
use sync_about::sync_about;
use sync_background::{sync_background, BackdropTex};
use sync_bar::{sync_bar, BarTexes};
use sync_bluetooth::{sync_bluetooth, BluetoothTexes};
use sync_clock::{sync_clock, Clocks};
use sync_controls::sync_controls;
use sync_core_picker::sync_core_picker;
use sync_disc::{sync_disc, DiscTexes};
use sync_hud::{sync_hud, HudTexes};
use sync_info::{sync_info, InfoTexes};
use sync_keyboard::{sync_keyboard, KeyboardTexes};
use sync_labels::sync_labels;
use sync_link::{sync_link, LinkTexes};
use sync_network::{sync_network, NetworkTexes};
use sync_opening::{sync_opening, TerminalRows};
use sync_power::{sync_power, PowerPanels};
use sync_settings::{sync_settings, SettingsTexes};
use sync_start_notice::sync_start_notice;
use sync_switcher::{sync_switcher, Switcher};
use sync_top_bar::{sync_top_bar, TopBarTexes};

const DOZE_TIMEOUT: Duration = Duration::from_secs(180);

/// How many bytes of the faces the label workers hand back are uploaded in one frame, the rest
/// left for the frames after, so a burst of them does not hold a frame up: four Game Boy Advance
/// carts, or a stack of four discs. At sixty frames a second that is still 240 carts a second,
/// well inside the opening a boot plays over the shelf.
const FACE_BYTES_PER_FRAME: usize = 1 << 20;

pub struct Frontend {
    session: Session,
    start: Instant,
    last: Instant,
    draws: Vec<Draw>,
    polaroid_texes: Vec<TexId>,
    faces: FaceBuilder,
    /// Draws each cart's face from its label, the scans with no cut yet cut first, for compose
    /// to upload as they come back.
    cutter: LabelCutter,
    /// The link screen's art, LINK panel and bar.
    link: LinkTexes,
    /// The DISC screen's panel and bar.
    disc: DiscTexes,
    core_asked: Option<String>,
    core_board_tex: Option<TexId>,
    core_lid_tex: Option<TexId>,
    core_built: Option<String>,
    switcher: Switcher,
    clocks: Clocks,
    top_bar: TopBarTexes,
    /// About's modal, baked as one face.
    about: Baked<(AboutView, u8)>,
    settings: SettingsTexes,
    bezel_marks: Vec<BezelMark>,
    overlays: Vec<Overlay>,
    /// The Network modal's own faces.
    network_modal: NetworkTexes,
    /// The Bluetooth modal's own faces.
    bluetooth_modal: BluetoothTexes,
    /// The keyboard's window, field, keys and lit key.
    keyboard: KeyboardTexes,
    info: InfoTexes,
    labels: Baked<(ListView, u8)>,
    controls: Baked<(crate::screens::controls::ControlsView, u8)>,
    background: Baked<(ListView, u8)>,
    /// The POWER menu's panel and the one shown after its choice.
    power: PowerPanels,
    backdrop: BackdropTex,
    bar: BarTexes,
    /// The gauge's, the toast's and the corner badge's faces.
    hud: HudTexes,
    /// The boot logo's texture while it fades over the shelf.
    boot_logo: Option<TexId>,
    /// The opening's lines while it plays.
    opening: TerminalRows,
    /// The start notice's lines while it is shown.
    start_notice: TerminalRows,
    /// The system and size of the last game picture uploaded, which a SNES or PlayStation
    /// game is laid out by.
    picture: Option<(omega_store::Platform, (u32, u32))>,
}

/// Each cart's generated face for the boot, drawn on as many threads as there are cores, and
/// whether the cart wears a label scan, which the label workers draw it in after.
fn boot_faces(carts: &[&Cart]) -> Vec<(CartFace, bool)> {
    let face = |c: &Cart| (generated_face(c), wears_label(c));
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = carts.len().div_ceil(threads).max(1);
    std::thread::scope(|s| {
        let parts: Vec<_> = carts
            .chunks(chunk)
            .map(|part| s.spawn(move || part.iter().map(|c| face(c)).collect::<Vec<_>>()))
            .collect();
        parts
            .into_iter()
            .flat_map(|p| p.join().unwrap_or_else(|e| std::panic::resume_unwind(e)))
            .collect()
    })
}

/// A card's own overlay for one system, read once and checked against each layout it is
/// laid over: one that would hide the game is refused rather than drawn.
struct Overlay {
    platform: omega_store::Platform,
    face: Option<UndoFace>,
    tex: Option<TexId>,
    checked: Vec<([i32; 4], bool)>,
}

/// A console's wordmark, uploaded once for each system and height it has been printed at.
struct BezelMark {
    platform: omega_store::Platform,
    height: u32,
    tex: TexId,
    w: u32,
    h: u32,
}

impl Frontend {
    pub fn boot(platform: Box<dyn Platform>) -> Self {
        let now = Instant::now();
        let mut session = Session::boot(platform.root().to_path_buf());
        session
            .app_mut()
            .set_power(Power::new(platform, DOZE_TIMEOUT));
        Frontend {
            session,
            start: now,
            last: now,
            draws: Vec::new(),
            polaroid_texes: Vec::new(),
            faces: FaceBuilder::spawn(),
            cutter: LabelCutter::spawn(),
            link: LinkTexes::default(),
            disc: DiscTexes::default(),
            core_asked: None,
            core_board_tex: None,
            core_lid_tex: None,
            core_built: None,
            switcher: Switcher::default(),
            clocks: Clocks::default(),
            top_bar: TopBarTexes::default(),
            about: Baked::default(),
            settings: SettingsTexes::default(),
            bezel_marks: Vec::new(),
            overlays: Vec::new(),
            network_modal: NetworkTexes::default(),
            bluetooth_modal: BluetoothTexes::default(),
            keyboard: KeyboardTexes::default(),
            info: InfoTexes::default(),
            labels: Baked::default(),
            controls: Baked::default(),
            background: Baked::default(),
            power: PowerPanels::default(),
            backdrop: BackdropTex::default(),
            bar: BarTexes::default(),
            hud: HudTexes::default(),
            boot_logo: None,
            opening: TerminalRows::default(),
            start_notice: TerminalRows::default(),
            picture: None,
        }
    }

    /// Puts `logo`, the logo the bootloader left lit, over the shelf's first frames to fade
    /// into it, when the boot has landed on the shelf. It is uploaded at its own size with
    /// NEAREST filtering, as the kit's faces are, so it shows what the bootloader drew pixel for
    /// pixel.
    pub fn show_boot_logo(&mut self, compositor: &mut Compositor, logo: &UndoFace) {
        if !self.session.app().boot_fades() {
            return;
        }
        let tex = upload_nearest(compositor, &mut self.boot_logo, logo);
        self.session.app_mut().set_boot_logo(tex, logo.w, logo.h);
    }

    /// Plays the opening over the shelf's first frames, telling `facts`, when the boot has
    /// landed on the shelf. The boot logo, when it is shown, fades out over its black.
    pub fn start_opening(&mut self, facts: &Facts) {
        self.session.app_mut().start_opening(facts);
    }

    /// Once the boot logo has faded, its texture shrinks to a pixel and gives its memory back.
    fn drop_boot_logo(&mut self, compositor: &mut Compositor) {
        if self.session.app().boot_logo_alpha().is_some() {
            return;
        }
        if let Some(tex) = self.boot_logo.take() {
            compositor.update_texture(tex, 1, 1, &[0; 4]);
        }
    }

    /// Draws every cart's face. No label is decoded here, where hundreds of them would hold the
    /// boot logo up: every cart comes up in its generated face, and each one that wears a label
    /// scan is drawn in it on the label workers' threads, the carts on screen first, for compose
    /// to upload as they come back.
    pub fn upload_faces(&mut self, compositor: &mut Compositor) {
        let carts: Vec<&Cart> = self.session.app().carts().collect();
        let mut labelled = std::collections::HashSet::new();
        let faces = carts
            .iter()
            .zip(boot_faces(&carts))
            .map(|(c, (f, label))| {
                if label {
                    labelled.insert((c.platform as u8, c.stem.as_str()));
                }
                compositor.create_texture(f.w, f.h, &f.rgba)
            })
            .collect();
        for cart in self.session.app().carts_nearest_first() {
            if labelled.contains(&(cart.platform as u8, cart.stem.as_str())) {
                self.cutter.request(cart.clone());
            }
        }
        self.session.app_mut().set_faces(faces);
        // the core picker's parts are pixel art, a texel to an art pixel, kept sharp at 2x
        let mut sprite = |f: CartFace| compositor.create_texture_nearest(f.w, f.h, &f.rgba);
        let sockets = omega_store::Core::ALL
            .iter()
            .map(|c| sprite(socket_face(*c)))
            .collect();
        let chips = omega_store::Core::ALL
            .iter()
            .map(|c| sprite(chip_face(Some(*c))))
            .collect();
        let blank = sprite(chip_face(None));
        let shadows = [sprite(chip_shadow_face()), sprite(lid_shadow_face())];
        let wide = (
            sprite(chip_face_in(SNES_SOCKETS, None)),
            sprite(chip_shadow_face_in(SNES_SOCKETS)),
        );
        let app = self.session.app_mut();
        app.set_core_part_faces(sockets, chips, blank, shadows);
        app.set_chip_faces_for(SNES_SOCKETS, wide.0, wide.1);
        for model in CartModel::ALL {
            let shadow = model_shadow(model);
            let id = compositor.create_texture(shadow.w, shadow.h, &shadow.rgba);
            self.session.app_mut().set_model_shadow(model, id);
        }
        let meshes = CartMeshes::upload(|mesh| compositor.create_mesh(mesh));
        self.session.app_mut().set_cart_meshes(meshes);
        self.upload_wallpaper(compositor);
    }

    fn upload_wallpaper(&mut self, compositor: &mut Compositor) {
        let app = self.session.app();
        let seed = app.wall_secs().unsigned_abs();
        let Some(rgba) = app
            .root()
            .and_then(|root| wallpaper::pick(root, seed))
            .and_then(|path| wallpaper_face(&path))
        else {
            return;
        };
        let id = compositor.create_texture(out_w(), out_h(), &rgba);
        self.session.app_mut().set_wallpaper(id);
    }

    pub fn render(&mut self, compositor: &mut Compositor, window: (u32, u32)) {
        self.compose(compositor);
        compositor.end_frame(window);
    }

    pub fn compose(&mut self, compositor: &mut Compositor) {
        compositor.set_blue_light(self.session.app().blue_light());
        compositor.set_shake(self.session.app().screen_shake());
        compositor.set_screen_power(self.session.app().screen_power());
        let platform = self.session.app().platform();
        let frame = self.session.frame();
        if let Some(f) = &frame {
            self.picture = Some((platform, f.size()));
        }
        let size = match self.picture {
            Some((p, size)) if p == platform => size,
            _ => platform.picture(),
        };
        let screen = crate::video_mode::screen_for(platform, self.session.app().video_mode, size);
        compositor.set_game_view(screen.view);
        compositor.set_still_view(screen.still);
        let picture = crate::video_mode::picture_rect(platform, screen.view);
        let overlay = self.overlay(platform, picture, compositor);
        let bezel = self.bezel(platform, screen.bands, picture, overlay, compositor);
        compositor.set_bezel(bezel);
        compositor.begin_frame();
        if let Some(frame) = frame {
            compositor.upload_game_sized(&frame, frame.size());
            crate::latency::taken();
        }
        sync_clock(self.session.app_mut(), compositor, &mut self.clocks);
        sync_top_bar(self.session.app_mut(), compositor, &mut self.top_bar);
        sync_about(self.session.app_mut(), compositor, &mut self.about);
        sync_settings(self.session.app_mut(), compositor, &mut self.settings);
        sync_network(self.session.app_mut(), compositor, &mut self.network_modal);
        sync_bluetooth(
            self.session.app_mut(),
            compositor,
            &mut self.bluetooth_modal,
        );
        sync_keyboard(self.session.app_mut(), compositor, &mut self.keyboard);
        sync_info(self.session.app_mut(), compositor, &mut self.info);
        sync_controls(self.session.app_mut(), compositor, &mut self.controls);
        sync_labels(self.session.app_mut(), compositor, &mut self.labels);
        sync_background(
            self.session.app_mut(),
            compositor,
            &mut self.backdrop,
            &mut self.background,
        );
        sync_power(self.session.app_mut(), compositor, &mut self.power);
        sync_bar(self.session.app_mut(), compositor, &mut self.bar);
        sync_hud(self.session.app_mut(), compositor, &mut self.hud);
        sync_opening(self.session.app_mut(), compositor, &mut self.opening);
        sync_start_notice(self.session.app_mut(), compositor, &mut self.start_notice);
        self.relabel(compositor);
        sync_core_picker(
            self.session.app_mut(),
            compositor,
            &self.faces,
            &mut self.core_asked,
            &mut self.core_board_tex,
            &mut self.core_lid_tex,
            &mut self.core_built,
        );
        sync_link(self.session.app_mut(), compositor, &mut self.link);
        sync_disc(self.session.app_mut(), compositor, &mut self.disc);
        sync_switcher(
            self.session.app_mut(),
            compositor,
            &mut self.polaroid_texes,
            &mut self.switcher,
        );
        self.drop_boot_logo(compositor);
        self.draws.clear();
        self.session.app().draw(&mut self.draws);
        compositor.draw_list(&self.draws);
    }

    /// Uploads the faces the label workers have handed back onto the faces their carts are drawn
    /// on, up to [`FACE_BYTES_PER_FRAME`] of them a frame, and asks the workers for the face of
    /// each cart a label arrived for, which comes back in a later frame.
    fn relabel(&mut self, compositor: &mut Compositor) {
        for drawn in self.cutter.take(FACE_BYTES_PER_FRAME) {
            let Some(tex) = self.session.app().face_of(drawn.platform, &drawn.stem) else {
                continue;
            };
            let face = &drawn.face;
            compositor.update_texture(tex, face.w, face.h, &face.rgba);
            self.drop_core_lid(&drawn.stem);
        }
        for (cart, _) in self.session.app_mut().take_stale_faces() {
            self.cutter.request(cart);
        }
    }

    /// Drops the core picker's lid when it was made for the cart `stem`, whose face has just
    /// been drawn again, so the picker asks for it anew.
    fn drop_core_lid(&mut self, stem: &str) {
        if [&self.core_asked, &self.core_built]
            .iter()
            .any(|s| s.as_deref() == Some(stem))
        {
            self.core_asked = None;
            self.core_built = None;
        }
    }

    /// The console's body for the bands the picture leaves, rimmed in black where they meet
    /// it, with its wordmark printed on the deepest band that runs the width of the canvas.
    fn bezel(
        &mut self,
        platform: omega_store::Platform,
        bands: Vec<[f32; 4]>,
        picture: [f32; 4],
        overlay: Option<TexId>,
        compositor: &mut Compositor,
    ) -> Option<Bezel> {
        if overlay.is_some() {
            return Some(Bezel {
                bands,
                fill: BEZEL_FILL,
                rims: Vec::new(),
                marks: Vec::new(),
                overlay,
            });
        }
        if bands.is_empty() {
            return None;
        }
        let rims = bands.iter().filter_map(|b| rim(*b, picture).1).collect();
        let printed = bands
            .iter()
            .copied()
            .filter(|b| b[2] >= out_w() as f32)
            .max_by(|a, b| a[3].total_cmp(&b[3]))
            .map(|b| rim(b, picture).0);
        let marks = printed
            .and_then(|band| {
                let m = self.bezel_mark(platform, mark_height(band[3]), compositor)?;
                Some((m.tex, mark_rect(band, m.w, m.h)))
            })
            .into_iter()
            .collect();
        Some(Bezel {
            bands,
            fill: BEZEL_FILL,
            rims,
            marks,
            overlay: None,
        })
    }

    fn overlay(
        &mut self,
        platform: omega_store::Platform,
        picture: [f32; 4],
        compositor: &mut Compositor,
    ) -> Option<TexId> {
        let at = match self.overlays.iter().position(|o| o.platform == platform) {
            Some(at) => at,
            None => {
                let face = self
                    .session
                    .app()
                    .root()
                    .and_then(|root| overlay_face(root, platform));
                self.overlays.push(Overlay {
                    platform,
                    face,
                    tex: None,
                    checked: Vec::new(),
                });
                self.overlays.len() - 1
            }
        };
        let root = self.session.app().root().map(|r| r.to_path_buf());
        let o = &mut self.overlays[at];
        let face = o.face.as_ref()?;
        let key = picture.map(|v| v.round() as i32);
        let clear = match o.checked.iter().find(|(k, _)| *k == key) {
            Some((_, clear)) => *clear,
            None => {
                let clear = overlay_clear(&face.rgba, face.w, face.h, picture);
                if !clear {
                    let path = root.map(|r| overlay_path(&r, platform));
                    eprintln!(
                        "omega-zero: overlay: {} covers the game at {picture:?}, so it is not drawn",
                        path.map_or_else(String::new, |p| p.display().to_string())
                    );
                }
                o.checked.push((key, clear));
                clear
            }
        };
        if !clear {
            return None;
        }
        if o.tex.is_none() {
            o.tex = Some(compositor.create_texture(face.w, face.h, &face.rgba));
        }
        o.tex
    }

    fn bezel_mark(
        &mut self,
        platform: omega_store::Platform,
        height: u32,
        compositor: &mut Compositor,
    ) -> Option<&BezelMark> {
        let known = self
            .bezel_marks
            .iter()
            .position(|m| m.platform == platform && m.height == height);
        let at = match known {
            Some(at) => at,
            None => {
                let face = bezel_mark_face(platform, height)?;
                self.bezel_marks.push(BezelMark {
                    platform,
                    height,
                    tex: compositor.create_texture(face.w, face.h, &face.rgba),
                    w: face.w,
                    h: face.h,
                });
                self.bezel_marks.len() - 1
            }
        };
        self.bezel_marks.get(at)
    }

    pub fn drive_emulator(&mut self) {
        self.session.set_driven(true);
    }

    pub fn frame_clock(&self) -> crate::pacing::FrameClock {
        self.session.frame_clock()
    }

    /// Whether the panel runs at 120 Hz, so the loop keeps its own clock rather than count
    /// the swaps.
    pub fn high_refresh(&self) -> bool {
        self.session.app().high_refresh()
    }

    pub fn step_emulator(&self, present: Duration, timeout: Duration) -> bool {
        self.session.step_emulator(present, timeout)
    }

    pub fn advance(&mut self, input: &mut dyn InputSource) {
        let now = self.now();
        let events = input.poll(now);
        self.session.feed(events, now);
        let dt = self.last.elapsed().as_secs_f32();
        self.last = Instant::now();
        self.session.update(dt);
    }

    pub fn app(&self) -> &crate::app::App {
        self.session.app()
    }

    pub fn app_mut(&mut self) -> &mut crate::app::App {
        self.session.app_mut()
    }

    pub fn core_settling(&self) -> bool {
        self.session.core_settling()
    }

    /// Whether the worker threads have handed back all they were asked for: the core picker's
    /// faces for the cart the shelf is on, and every face asked of the label workers. compose
    /// takes them in.
    pub fn settled(&self) -> bool {
        self.core_asked == self.core_built && !self.drawing_labels() && !self.core_settling()
    }

    /// Whether a face asked of the label workers has yet to come back, or to be uploaded.
    pub fn drawing_labels(&self) -> bool {
        !self.cutter.idle()
    }

    pub fn advance_at(&mut self, input: &mut dyn InputSource, now: Millis, dt: f32) {
        let events = input.poll(now);
        self.session.feed(events, now);
        self.session.update(dt);
    }

    fn now(&self) -> Millis {
        self.start.elapsed().as_millis() as Millis
    }

    pub fn powering_off(&self) -> bool {
        self.session.app().ready_to_power_off()
    }

    pub fn restarting(&self) -> bool {
        self.session.app().ready_to_restart()
    }

    pub fn restart(&mut self) {
        self.session.app_mut().restart();
    }

    pub fn poweroff(&mut self) {
        self.session.app_mut().poweroff();
    }
}
