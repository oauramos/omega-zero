//! Drives the whole frontend headless through a fixed script and writes a PNG of every `shot`,
//! so a change to how omega-zero draws can be checked pixel for pixel against the build before it.
//!
//! `screens OUT [BASELINE]` writes OUT/<name>.png. With BASELINE, every shot is also compared
//! with BASELINE/<name>.png and the run fails on the first byte that differs. SCREENS_SCRIPT
//! names a file to run in place of the built-in script. After the script, the opening and the
//! start notice it gives way to are drawn on a card of their own.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use omega_gfx::{out_h, out_w, Bezel, Compositor, Draw, HeadlessSurface};
use omega_input::{Btn, InputSource, Millis, RawEvent};
use omega_power::SimPlatform;
use omega_store::{write_app_state, write_selected_core, AppState, Core, Platform};
use omega_ui::BEZEL_FILL;
use omega_zero::frontend::Frontend;
use omega_zero::label_sync::{LabelJob, LabelJobs, LabelNews, Quota};
use omega_zero::opening::Facts;
use omega_zero::video_mode::{screen, screen_for, VideoMode};
use omega_zero::wifi::{SimWifi, WifiJob, WifiJobs, WifiNews};

const FPS: f64 = 60.0;

const CLOCK: i64 = 1_790_784_000;

/// The longest a settle waits on the worker threads before it goes on without them.
const SETTLE: Duration = Duration::from_secs(2);

const LIT_FRAMES: u32 = 600;

/// How many frames the desktop's radio takes to answer a scan, as the handheld's takes seconds.
const SCAN_FRAMES: u32 = 30;

const SCRIPT: &str = "
wait 1
shot boot-logo
wait 15
shot boot-fade-half
wait 15
shot boot-fade-end
wait 89
settle
shot shelf-gba
frames shelf-drift 90
tap right
wait 40
shot shelf-scrolled
tap left
wait 40
down start
wait 8
shot slide-to-settings
up start
wait 60
down b
wait 8
shot slide-to-shelf
up b
wait 60
tap select
wait 8
shot slide-to-controls
wait 60
tap b
wait 60
tap x
settle
wait 90
shot core-picker
tap b
wait 60
tap start
wait 60
shot settings
tap down
wait 10
tap down
wait 10
tap down
wait 10
tap down
wait 10
tap down
wait 10
tap down
wait 10
tap down
wait 10
tap down
wait 10
tap down
wait 10
tap a
settle
wait 30
shot background
tap right
wait 10
settle
wait 10
shot background-bits
tap right
wait 10
tap down
wait 10
tap right
wait 10
tap right
wait 10
settle
wait 10
shot background-solid
tap left
wait 10
tap left
wait 10
tap up
wait 10
tap right
wait 10
tap up
wait 10
tap right
wait 10
settle
wait 10
shot theme-thin
tap b
wait 30
tap b
wait 60
settle
shot shelf-thin
tap start
wait 60
tap down
wait 10
tap down
wait 10
tap down
wait 10
tap down
wait 10
tap down
wait 10
tap down
wait 10
tap down
wait 10
tap down
wait 10
tap down
wait 10
tap a
settle
wait 30
tap up
wait 10
tap left
wait 10
tap b
wait 30
tap down
wait 10
settle
shot settings-scrolled
tap up
wait 10
tap up
wait 10
tap up
wait 10
down a
wait 8
shot slide-to-network
up a
settle
wait 30
shot network-off
tap a
wait 30
settle
wait 30
shot network-unscanned
tap x
wait 4
settle
shot network-scanning
wait 40
settle
shot network-list
tap down
wait 10
tap down
wait 10
tap down
wait 10
tap a
settle
wait 30
shot keyboard
tap a
wait 6
tap x
wait 6
tap right
wait 6
tap a
wait 10
settle
shot keyboard-typed
tap start
wait 10
shot keyboard-refused
tap y
tap y
tap y
tap y
tap y
tap y
tap start
wait 30
settle
wait 30
shot network-joined
tap up
wait 10
tap right
wait 10
settle
shot network-ssh-off
tap left
wait 10
tap left
wait 10
settle
shot network-ssh-info
tap a
settle
wait 30
shot ssh
tap b
wait 30
tap b
wait 30
settle
wait 10
shot settings-network
tap down
wait 10
settle
shot settings-bluetooth
tap a
settle
wait 30
shot bluetooth-off
tap a
wait 30
settle
wait 30
shot bluetooth
tap x
wait 10
settle
shot bluetooth-looking
wait 730
settle
shot bluetooth-found
tap down
wait 10
tap down
wait 10
tap down
wait 10
tap a
wait 10
settle
shot bluetooth-pairing
tap up
wait 10
tap up
wait 10
tap up
wait 10
tap a
wait 10
tap b
wait 30
tap up
wait 10
tap up
wait 10
tap a
settle
wait 60
shot clock
tap b
wait 60
tap up
wait 10
tap a
settle
wait 30
shot labels
tap a
wait 6
tap a
wait 6
settle
shot labels-keyboard
tap start
wait 6
tap a
wait 6
tap start
wait 10
settle
wait 30
shot labels-signed-in
tap a
wait 10
settle
wait 30
shot labels-working
tap a
wait 10
tap b
wait 30
tap down
wait 10
tap down
wait 10
tap down
wait 10
tap down
wait 10
tap down
wait 10
tap a
settle
wait 90
look about
tap b
wait 60
tap b
wait 60
tap select
wait 60
settle
shot controls
tap b
wait 60
tap r1
wait 60
shot shelf-gb
tap l1
wait 60
tap a
frames insert 120
lit
wait 240
lit
look game-gba
tap volup
wait 12
look hud-volume
wait 180
down select
tap r1
up select
wait 60
look toast-saved
wait 180
down select
wait 2
tap menu
wait 2
up select
settle
wait 90
shot link
tap b
wait 120
tap menu 4
wait 4
tap menu 4
settle
wait 90
look switcher
tap b
wait 90
hold menu 80
frames eject 180
shot ejected
tap r1
wait 60
tap a
wait 120
lit
wait 240
lit
look game-gb
tap l1
wait 30
look game-gb-stretch
tap r1
wait 30
hold power 80
wait 60
shot power-menu
tap a
wait 30
shot restarting
boot
wait 120
lit
hold power 80
wait 60
tap down
wait 10
tap a
wait 30
shot powering-off
";

/// A ScreenScraper that answers a sync with one in progress, for the Labels screen's shots.
#[derive(Default)]
struct ShownLabels(Vec<LabelNews>);

impl LabelJobs for ShownLabels {
    fn present(&self) -> bool {
        true
    }

    fn ask(&mut self, job: LabelJob) {
        if let LabelJob::Sync { .. } = job {
            self.0.push(LabelNews::Quota(Quota {
                left: 19_953,
                per_day: 20_000,
            }));
            self.0.push(LabelNews::Working {
                done: 11,
                total: 67,
                title: "Metroid Fusion".into(),
            });
        }
    }

    fn news(&mut self) -> Vec<LabelNews> {
        std::mem::take(&mut self.0)
    }
}

/// The desktop's radio with each scan's answer held for `SCAN_FRAMES`, so a shot can show the
/// Network screen while it looks.
#[derive(Default)]
struct HeldScans {
    sim: SimWifi,
    /// What the radio said alongside a scan, told at once.
    told: Vec<WifiNews>,
    /// The networks a scan heard, and how many more frames before they are told.
    held: Option<(Vec<WifiNews>, u32)>,
}

impl WifiJobs for HeldScans {
    fn present(&self) -> bool {
        self.sim.present()
    }

    fn ask(&mut self, job: WifiJob) {
        let scan = job == WifiJob::Scan;
        self.sim.ask(job);
        if scan {
            let (seen, rest): (Vec<_>, Vec<_>) = self
                .sim
                .news()
                .into_iter()
                .partition(|n| matches!(n, WifiNews::Seen(_)));
            self.told.extend(rest);
            self.held = Some((seen, SCAN_FRAMES));
        }
    }

    fn news(&mut self) -> Vec<WifiNews> {
        let mut news = std::mem::take(&mut self.told);
        news.extend(self.sim.news());
        if let Some((seen, frames)) = &mut self.held {
            if *frames == 0 {
                news.append(seen);
                self.held = None;
            } else {
                *frames -= 1;
            }
        }
        news
    }
}

enum Step {
    Settle,
    Lit,
    Wait(u32),
    Down(Btn),
    Up(Btn),
    Shot(String),
    Look(String),
    /// Wait `n` frames like `wait`, writing each one out as OUT/<name>-NNN.png.
    Frames(String, u32),
    /// Boot the frontend again on the same card, as the handheld does after a restart.
    Boot,
    /// The core refuses the cart going in, as it does a broken ROM, so the cart comes back out
    /// with the alert on it.
    Fail,
}

fn btn(name: &str) -> Btn {
    match name {
        "up" => Btn::Up,
        "down" => Btn::Down,
        "left" => Btn::Left,
        "right" => Btn::Right,
        "a" => Btn::A,
        "b" => Btn::B,
        "x" => Btn::X,
        "y" => Btn::Y,
        "l1" => Btn::L1,
        "r1" => Btn::R1,
        "l2" => Btn::L2,
        "r2" => Btn::R2,
        "start" => Btn::Start,
        "select" => Btn::Select,
        "menu" => Btn::Menu,
        "volup" => Btn::VolUp,
        "voldown" => Btn::VolDown,
        "power" => Btn::Power,
        other => panic!("unknown button {other}"),
    }
}

fn parse(script: &str) -> Vec<Step> {
    let mut steps = Vec::new();
    for line in script.lines() {
        let words: Vec<&str> = line.split('#').next().unwrap().split_whitespace().collect();
        let n = |i: usize, default: u32| words.get(i).map_or(default, |w| w.parse().unwrap());
        match words.as_slice() {
            [] => {}
            ["wait", ..] => steps.push(Step::Wait(n(1, 1))),
            ["settle"] => steps.push(Step::Settle),
            ["boot"] => steps.push(Step::Boot),
            ["fail"] => steps.push(Step::Fail),
            ["lit"] => steps.push(Step::Lit),
            ["tap", b, ..] | ["hold", b, ..] => {
                steps.push(Step::Down(btn(b)));
                steps.push(Step::Wait(n(2, 6)));
                steps.push(Step::Up(btn(b)));
            }
            ["down", b] => steps.push(Step::Down(btn(b))),
            ["up", b] => steps.push(Step::Up(btn(b))),
            ["shot", name] => steps.push(Step::Shot(name.to_string())),
            ["look", name] => steps.push(Step::Look(name.to_string())),
            ["frames", name, ..] => steps.push(Step::Frames(name.to_string(), n(2, 1))),
            _ => panic!("cannot read {line:?}"),
        }
    }
    steps
}

#[derive(Default)]
struct Scripted(Vec<RawEvent>);

impl InputSource for Scripted {
    fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
        std::mem::take(&mut self.0)
    }
}

fn tick(
    frontend: &mut Frontend,
    compositor: &mut Compositor,
    input: &mut Scripted,
    frame: &mut u64,
) {
    let loading = std::time::Instant::now();
    while frontend.core_settling() && loading.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(5));
    }
    *frame += 1;
    let now = (*frame as f64 * 1000.0 / FPS) as Millis;
    frontend.advance_at(input, now, (1.0 / FPS) as f32);
    frontend.step_emulator(Duration::from_secs_f64(1.0 / FPS), Duration::from_secs(1));
    frontend.compose(compositor);
}

/// Composes until the worker threads have handed back what the script asked of them, so a shot
/// shows the faces they build rather than the frame before those arrive.
fn settle(frontend: &mut Frontend, compositor: &mut Compositor) {
    let start = Instant::now();
    frontend.compose(compositor);
    while !frontend.settled() && start.elapsed() < SETTLE {
        std::thread::sleep(Duration::from_millis(2));
        frontend.compose(compositor);
    }
}

/// The frontend booted on the card at `root`, its faces up and the boot logo over its first
/// frames, as the handheld boots it.
fn booted(root: &Path, compositor: &mut Compositor) -> Frontend {
    let mut frontend = Frontend::boot(Box::new(SimPlatform::at(root.into()).stopped_at(CLOCK)));
    frontend.upload_faces(compositor);
    let system = root.join("System");
    if let Some(logo) = omega_zero::bootlogo::shown(&system, (out_w(), out_h())) {
        frontend.show_boot_logo(compositor, &logo);
    }
    frontend.drive_emulator();
    frontend
        .app_mut()
        .set_label_jobs(Box::new(ShownLabels::default()));
    frontend
        .app_mut()
        .set_wifi_jobs(Box::new(HeldScans::default()));
    frontend
}

fn card(dir: &Path) {
    for sub in omega_zero::root::DIRS {
        std::fs::create_dir_all(dir.join(sub)).expect("create the card's folders");
    }
    // the boot logo omega-zero ships for this canvas, as the card carries it
    let logo = format!("bootlogo-{}x{}.bmp", out_w(), out_h());
    let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../card/System");
    std::fs::copy(shipped.join(&logo), dir.join("System").join(&logo)).expect("copy the boot logo");
    for title in ["Emerald", "Fire Red", "Minish Cap", "Zelda"] {
        let mut rom = vec![0u8; 0x100];
        let header = title.to_uppercase();
        rom[0xa0..0xa0 + header.len().min(12)]
            .copy_from_slice(&header.as_bytes()[..header.len().min(12)]);
        std::fs::write(dir.join(format!("Games/GBA/{title}.gba")), rom).expect("write a GBA rom");
    }
    for title in ["Tetris", "Links Awakening"] {
        let mut rom = vec![0u8; 0x150];
        let header = title.to_uppercase();
        rom[0x134..0x134 + header.len().min(11)]
            .copy_from_slice(&header.as_bytes()[..header.len().min(11)]);
        std::fs::write(dir.join(format!("Games/GB/{title}.gb")), rom).expect("write a GB rom");
    }
    write_app_state(
        dir,
        &AppState {
            clock_set: true,
            ..Default::default()
        },
    )
    .expect("write omega-zero.state");
}

/// The mock core's pattern moves with however many frames its thread ran, so every shot shows
/// this fixed picture in the game texture instead.
fn still_frame() -> Vec<u8> {
    sized_frame(omega_retro::GBA_W, omega_retro::GBA_H)
}

/// The same fixed picture at any size, for a SNES or PlayStation game's frame.
fn sized_frame(w: u32, h: u32) -> Vec<u8> {
    let mut xrgb = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let check = if (x / 8 + y / 8) % 2 == 0 { 0xff } else { 0x30 };
            xrgb.extend_from_slice(&[
                check,
                (y * 255 / (h - 1)) as u8,
                (x * 255 / (w - 1)) as u8,
                0xff,
            ]);
        }
    }
    xrgb
}

/// The canvas as the panel shows it. Its alpha is what blending left there, which the panel
/// never shows: a translucent plate over the backdrop would come out looking pale in a
/// picture viewer, so every pixel is written opaque.
fn panel_frame(compositor: &Compositor) -> Vec<u8> {
    let mut rgba = compositor.read_frame();
    for px in rgba.chunks_exact_mut(4) {
        px[3] = 0xff;
    }
    rgba
}

fn write_png(path: &Path, rgba: &[u8]) {
    let file = std::fs::File::create(path).expect("create the png");
    let mut e = png::Encoder::new(std::io::BufWriter::new(file), out_w(), out_h());
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()
        .expect("png header")
        .write_image_data(rgba)
        .expect("png data");
}

fn read_png(path: &Path) -> Option<(u32, u32, Vec<u8>)> {
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).ok()?));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    Some((info.width, info.height, buf))
}

fn compare(name: &str, rgba: &[u8], baseline: &Path) -> Result<(), String> {
    let path = baseline.join(format!("{name}.png"));
    let Some((w, h, want)) = read_png(&path) else {
        return Err(format!("{name}: no baseline at {}", path.display()));
    };
    if (w, h) != (out_w(), out_h()) {
        return Err(format!(
            "{name}: baseline is {w}x{h}, the shot is {}x{}",
            out_w(),
            out_h()
        ));
    }
    let differ = rgba
        .chunks_exact(4)
        .zip(want.chunks_exact(4))
        .filter(|(a, b)| a != b)
        .count();
    match differ {
        0 => Ok(()),
        n => Err(format!("{name}: {n} pixels differ from the baseline")),
    }
}

/// The opening with all its lines up, the start notice after it with its prompt up, and the
/// shelf A puts it away to, on a card whose last start, Mega Man X's on Supafaust, took
/// omega-zero down. Each is compared with BASELINE's only where BASELINE has one: a baseline
/// from before they were drawn has none.
fn notice_shots(
    compositor: &mut Compositor,
    out: &Path,
    baseline: Option<&Path>,
    failures: &mut Vec<String>,
) {
    let root = tempfile::tempdir().expect("a scratch card");
    card(root.path());
    let stem = "Mega Man X (U) (V1.1) [!]";
    std::fs::write(
        root.path().join(format!("Games/SNES/{stem}.sfc")),
        vec![0u8; 0x8000],
    )
    .expect("write a SNES rom");
    write_selected_core(root.path(), stem, Core::Supafaust).expect("choose its core");
    write_app_state(
        root.path(),
        &AppState {
            clock_set: true,
            cart: Some(stem.into()),
            cart_platform: Some(Platform::Snes),
            ..Default::default()
        },
    )
    .expect("write omega-zero.state");
    std::fs::write(root.path().join("Config/.starting"), format!("SNES/{stem}"))
        .expect("note the game starting");
    let mut frontend = booted(root.path(), compositor);
    frontend.start_opening(&Facts::default());
    let mut input = Scripted::default();
    let mut frame = 0u64;
    let mut shoot = |name: &str, frontend: &mut Frontend, compositor: &mut Compositor| {
        settle(frontend, compositor);
        frontend.compose(compositor);
        let rgba = panel_frame(compositor);
        write_png(&out.join(format!("{name}.png")), &rgba);
        if let Some(baseline) = baseline.filter(|b| b.join(format!("{name}.png")).exists()) {
            if let Err(e) = compare(name, &rgba, baseline) {
                failures.push(e);
            }
        }
    };
    for _ in 0..600 {
        tick(&mut frontend, compositor, &mut input, &mut frame);
    }
    shoot("opening", &mut frontend, compositor);
    while !frontend.app().start_notice_prompted() && frame < 1200 {
        tick(&mut frontend, compositor, &mut input, &mut frame);
    }
    shoot("start-notice", &mut frontend, compositor);
    input.0.push(RawEvent::Down(Btn::A));
    tick(&mut frontend, compositor, &mut input, &mut frame);
    input.0.push(RawEvent::Up(Btn::A));
    for _ in 0..60 {
        tick(&mut frontend, compositor, &mut input, &mut frame);
    }
    shoot("start-notice-shelf", &mut frontend, compositor);
}

fn main() {
    let args: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    let (out, baseline) = match args.as_slice() {
        [out] => (out.clone(), None),
        [out, baseline] => (out.clone(), Some(baseline.clone())),
        _ => {
            eprintln!("usage: screens OUT [BASELINE]");
            std::process::exit(2);
        }
    };
    std::fs::create_dir_all(&out).expect("create the output folder");
    let root = tempfile::tempdir().expect("a scratch card");
    card(root.path());
    std::env::set_var("OMEGA_SILENT", "1");
    std::env::remove_var("OMEGA_CORE");

    let surface = HeadlessSurface::new().expect("headless GL");
    let mut compositor = Compositor::new(&surface).expect("compositor");
    let mut frontend = booted(root.path(), &mut compositor);

    // SCREENS_SCRIPT runs another script in place of the built-in one, for one-off previews.
    let script = std::env::var("SCREENS_SCRIPT")
        .map(|path| std::fs::read_to_string(path).expect("read SCREENS_SCRIPT"))
        .unwrap_or_else(|_| SCRIPT.to_string());
    let steps = parse(&script);
    let shots: Vec<String> = steps
        .iter()
        .filter_map(|s| match s {
            Step::Shot(name) => Some(name.clone()),
            _ => None,
        })
        .collect();
    let mut input = Scripted::default();
    let mut frame = 0u64;
    let mut failures = Vec::new();
    for step in steps {
        match step {
            Step::Wait(n) => {
                for _ in 0..n {
                    tick(&mut frontend, &mut compositor, &mut input, &mut frame);
                }
            }
            Step::Frames(name, n) => {
                for i in 0..n {
                    tick(&mut frontend, &mut compositor, &mut input, &mut frame);
                    let rgba = panel_frame(&compositor);
                    write_png(&out.join(format!("{name}-{i:03}.png")), &rgba);
                }
            }
            Step::Lit => {
                let mut left = LIT_FRAMES;
                while frontend.app().screen_power() < 1.0 && left > 0 {
                    tick(&mut frontend, &mut compositor, &mut input, &mut frame);
                    left -= 1;
                }
            }
            Step::Down(b) => input.0.push(RawEvent::Down(b)),
            Step::Up(b) => input.0.push(RawEvent::Up(b)),
            Step::Settle => settle(&mut frontend, &mut compositor),
            Step::Boot => frontend = booted(root.path(), &mut compositor),
            Step::Fail => frontend.app_mut().on_core_failed(),
            Step::Shot(name) | Step::Look(name) => {
                settle(&mut frontend, &mut compositor);
                compositor.upload_game(&still_frame());
                frontend.compose(&mut compositor);
                let rgba = panel_frame(&compositor);
                write_png(&out.join(format!("{name}.png")), &rgba);
                if let (Some(baseline), true) = (&baseline, shots.contains(&name)) {
                    if let Err(e) = compare(&name, &rgba, baseline) {
                        failures.push(e);
                    }
                }
            }
        }
    }
    drop(frontend);
    notice_shots(&mut compositor, &out, baseline.as_deref(), &mut failures);
    for platform in [Platform::Gba, Platform::Gb] {
        for mode in [VideoMode::Actual, VideoMode::Stretch] {
            for power in [0.05, 0.5, 1.0] {
                let name = format!("view-{}-{}-{power}", platform.dir_name(), mode.as_str());
                let screen = screen(platform, mode);
                compositor.upload_game(&still_frame());
                compositor.set_game_view(screen.view);
                compositor.set_bezel((!screen.bands.is_empty()).then(|| Bezel {
                    bands: screen.bands,
                    fill: BEZEL_FILL,
                    ..Bezel::default()
                }));
                compositor.set_screen_power(power);
                compositor.begin_frame();
                compositor.draw_list(&[Draw::Game]);
                let rgba = panel_frame(&compositor);
                write_png(&out.join(format!("{name}.png")), &rgba);
                if let Some(baseline) = &baseline {
                    if let Err(e) = compare(&name, &rgba, baseline) {
                        failures.push(e);
                    }
                }
            }
        }
    }
    // the consoles at the sizes they draw; a baseline from before they were drawn has none
    for (platform, (w, h)) in [
        (Platform::Snes, (256, 224)),
        (Platform::Snes, (512, 448)),
        (Platform::Ps1, (320, 240)),
        (Platform::Ps1, (640, 480)),
    ] {
        for mode in [VideoMode::Actual, VideoMode::Stretch] {
            let name = format!("view-{}-{}-{w}x{h}", platform.dir_name(), mode.as_str());
            let screen = screen_for(platform, mode, (w, h));
            compositor.upload_game_sized(&sized_frame(w, h), (w, h));
            compositor.set_game_view(screen.view);
            compositor.set_bezel((!screen.bands.is_empty()).then(|| Bezel {
                bands: screen.bands,
                fill: BEZEL_FILL,
                ..Bezel::default()
            }));
            compositor.set_screen_power(1.0);
            compositor.begin_frame();
            compositor.draw_list(&[Draw::Game]);
            let rgba = panel_frame(&compositor);
            write_png(&out.join(format!("{name}.png")), &rgba);
            if let Some(baseline) = baseline
                .as_ref()
                .filter(|b| b.join(format!("{name}.png")).exists())
            {
                if let Err(e) = compare(&name, &rgba, baseline) {
                    failures.push(e);
                }
            }
        }
    }
    if failures.is_empty() {
        eprintln!("screens: wrote {}", out.display());
        return;
    }
    for f in &failures {
        eprintln!("screens: {f}");
    }
    std::process::exit(1);
}
