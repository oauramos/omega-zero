use std::ffi::{c_int, c_ulong, c_void};
use std::fs;
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::refresh::PanelRefresh;
use crate::{Battery, Charge, LedState, Platform};

const TOP_STEP: u32 = 9;

const DEV_INPUT: &str = "/dev/input";

const RUN_DIR: &str = "/run";

pub fn uptime_seconds(s: &str) -> Option<String> {
    let first = s.split_whitespace().next()?;
    if !first.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    if !first.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(first.to_string())
}

pub fn record_first_frame(run: &Path, uptime: &str) {
    let marker = run.join("boot-first-frame");
    let again = marker.exists();
    if !again {
        let _ = fs::write(&marker, format!("{uptime}\n"));
    }
    let label = if again {
        "first-frame-again"
    } else {
        "first-frame"
    };
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(run.join("boot-trace"))
    {
        let _ = writeln!(f, "{uptime} {label}");
    }
}

pub fn trace_first_frame() {
    let Ok(raw) = fs::read_to_string("/proc/uptime") else {
        return;
    };
    let Some(up) = uptime_seconds(&raw) else {
        return;
    };
    record_first_frame(Path::new(RUN_DIR), &up);
}

const EV_FF: u16 = 0x15;
const FF_RUMBLE: u16 = 0x50;
const EVIOCSFF: c_ulong = 0x4030_4580;

const DISPDBG_MAX: u32 = 255;

enum Backlight {
    Class(PathBuf),
    Dispdbg(PathBuf),
}

enum Led {
    Multi(PathBuf),
    Mono { dir: PathBuf, max: u32 },
}

pub struct DevicePlatform {
    root: PathBuf,
    sysfs: PathBuf,
    backlight: Option<Backlight>,
    max_brightness: u32,
    battery: Option<PathBuf>,
    rtc: Option<PathBuf>,
    motor: Option<Motor>,
    led: Option<Led>,
    refresh: Option<PanelRefresh>,
}

impl DevicePlatform {
    /// The handheld omega-zero runs on, its model the one BASEOS_TARGET names, else the one
    /// /etc/baseos-release does.
    pub fn new(root: PathBuf) -> Self {
        let target = std::env::var("BASEOS_TARGET")
            .ok()
            .or_else(|| crate::baseos_release().and_then(|r| r.target));
        DevicePlatform::probe(Path::new("/sys"), root, target.as_deref())
    }

    /// The handheld under `sysfs`. Its panel's refresh is only ever driven where `target`, the
    /// BaseOS model, is one whose panel is known, and the device tree says it is that panel.
    pub fn probe(sysfs: &Path, root: PathBuf, target: Option<&str>) -> Self {
        let class = first_dir(&sysfs.join("class/backlight"), |d| {
            d.join("brightness").is_file()
        });
        let max_brightness = class
            .as_ref()
            .and_then(|d| read_number(&d.join("max_brightness")))
            .unwrap_or(DISPDBG_MAX);
        let backlight = match class {
            Some(dir) => Some(Backlight::Class(dir)),
            None => {
                let dbg = sysfs.join("kernel/debug/dispdbg");
                dbg.join("param")
                    .is_file()
                    .then_some(Backlight::Dispdbg(dbg))
            }
        };
        let battery = first_dir(&sysfs.join("class/power_supply"), is_battery);
        let rtc = first_dir(&sysfs.join("class/rtc"), |_| true);
        let motor = Motor::open(sysfs);
        let led = first_dir(&sysfs.join("class/leds"), |d| {
            d.join("multi_intensity").is_file() || d.join("brightness").is_file()
        })
        .map(|dir| {
            if dir.join("multi_intensity").is_file() {
                Led::Multi(dir)
            } else {
                let max = read_number(&dir.join("max_brightness")).unwrap_or(255);
                Led::Mono { dir, max }
            }
        });
        DevicePlatform {
            root,
            sysfs: sysfs.to_path_buf(),
            backlight,
            max_brightness,
            battery,
            rtc,
            motor,
            led,
            refresh: target.and_then(|target| PanelRefresh::probe(sysfs, target)),
        }
    }

    fn breadcrumb(&self, line: &str) {
        use std::io::Write;
        if let Ok(mut f) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.root.join("ags-shutdown-trace.log"))
        {
            let _ = writeln!(f, "{} {line}", self.now());
            let _ = f.sync_all();
        }
    }

    pub fn trace_boot(&self) {
        self.breadcrumb("boot: platform up, trace working");
    }

    pub fn report(&self) -> String {
        let leaf = |p: &Path| {
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.display().to_string())
        };
        let name = |p: &Option<PathBuf>, absent: &str| match p {
            Some(p) => leaf(p),
            None => absent.to_string(),
        };
        let backlight = match &self.backlight {
            Some(Backlight::Class(dir)) => leaf(dir),
            Some(Backlight::Dispdbg(_)) => "dispdbg".to_string(),
            None => "no backlight".to_string(),
        };
        let motor = match &self.motor {
            Some(m) => format!("motor {}", m.name()),
            None => "no motor".to_string(),
        };
        let refresh = match self.refresh {
            Some(_) => "120 Hz panel",
            None => "60 Hz panel",
        };
        format!(
            "backlight {} (0 to {}), battery {}, {}, {motor}, {refresh}",
            backlight,
            self.max_brightness,
            name(&self.battery, "no battery"),
            name(&self.rtc, "no rtc"),
        )
    }
}

fn ran(what: &str, result: std::io::Result<std::process::ExitStatus>) {
    match result {
        Ok(status) if status.success() => eprintln!("omega-zero: {what} ok"),
        Ok(status) => eprintln!("omega-zero: {what} {status}"),
        Err(e) => eprintln!("omega-zero: {what}: {e}"),
    }
}

pub fn has_bit(mask: &str, bit: u16) -> bool {
    let words: Vec<&str> = mask.split_whitespace().collect();
    let from_end = usize::from(bit) / 64;
    let Some(word) = words.len().checked_sub(from_end + 1).map(|i| words[i]) else {
        return false;
    };
    u64::from_str_radix(word, 16).is_ok_and(|w| w >> (bit % 64) & 1 == 1)
}

pub fn rumble_node(sysfs: &Path) -> Option<String> {
    let dir = first_dir(&sysfs.join("class/input"), |node| {
        fs::read_to_string(node.join("device/capabilities/ff"))
            .is_ok_and(|ff| has_bit(&ff, FF_RUMBLE))
    })?;
    Some(dir.file_name()?.to_string_lossy().into_owned())
}

pub fn motor_change(strength: u16, running: bool) -> Option<bool> {
    match (strength > 0, running) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => None,
    }
}

/// The rumble motor: a force feedback node where the kernel offers one, or the PMIC's motor
/// pin, which the Anbernic kernel exposes as a power supply's `moto` file instead.
enum Motor {
    Ff(FfMotor),
    Pin(PinMotor),
}

impl Motor {
    fn open(sysfs: &Path) -> Option<Motor> {
        FfMotor::open(sysfs)
            .map(Motor::Ff)
            .or_else(|| PinMotor::find(sysfs).map(Motor::Pin))
    }

    fn name(&self) -> String {
        match self {
            Motor::Ff(m) => m.name.clone(),
            Motor::Pin(m) => m.path.display().to_string(),
        }
    }

    fn running(&self) -> bool {
        match self {
            Motor::Ff(m) => m.running,
            Motor::Pin(m) => m.running,
        }
    }

    fn play(&mut self, on: bool) {
        match self {
            Motor::Ff(m) => m.play(on),
            Motor::Pin(m) => m.play(on),
        }
    }
}

pub fn motor_pin(sysfs: &Path) -> Option<PathBuf> {
    first_dir(&sysfs.join("class/power_supply"), |d| {
        d.join("moto").is_file()
    })
    .map(|d| d.join("moto"))
}

struct PinMotor {
    path: PathBuf,
    running: bool,
}

impl PinMotor {
    fn find(sysfs: &Path) -> Option<PinMotor> {
        Some(PinMotor {
            path: motor_pin(sysfs)?,
            running: false,
        })
    }

    fn play(&mut self, on: bool) {
        match fs::write(&self.path, if on { "1" } else { "0" }) {
            Ok(()) => self.running = on,
            Err(e) => eprintln!("omega-zero: rumble {}: {e}", self.path.display()),
        }
    }
}

impl Drop for PinMotor {
    fn drop(&mut self) {
        if self.running {
            self.play(false);
        }
    }
}

struct FfMotor {
    node: fs::File,
    name: String,
    id: i16,
    running: bool,
}

#[repr(C)]
#[derive(Default)]
struct FfTrigger {
    button: u16,
    interval: u16,
}

#[repr(C)]
#[derive(Default)]
struct FfReplay {
    length: u16,
    delay: u16,
}

#[repr(C)]
#[derive(Default)]
struct FfRumble {
    strong: u16,
    weak: u16,
}

#[repr(C)]
struct FfEffect {
    kind: u16,
    id: i16,
    direction: u16,
    trigger: FfTrigger,
    replay: FfReplay,
    _align: u16,
    rumble: FfRumble,
    _tail: [u8; 28],
}

#[repr(C)]
struct FfEvent {
    sec: i64,
    usec: i64,
    kind: u16,
    code: u16,
    value: i32,
}

extern "C" {
    fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
}

impl FfMotor {
    fn open(sysfs: &Path) -> Option<FfMotor> {
        let name = rumble_node(sysfs)?;
        let node = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(Path::new(DEV_INPUT).join(&name))
            .map_err(|e| eprintln!("omega-zero: rumble {name}: {e}"))
            .ok()?;
        let mut effect = FfEffect {
            kind: FF_RUMBLE,
            id: -1,
            direction: 0,
            trigger: FfTrigger::default(),
            replay: FfReplay::default(),
            _align: 0,
            rumble: FfRumble {
                strong: u16::MAX,
                weak: u16::MAX,
            },
            _tail: [0; 28],
        };
        let rc = unsafe {
            ioctl(
                node.as_raw_fd(),
                EVIOCSFF,
                &mut effect as *mut FfEffect as *mut c_void,
            )
        };
        if rc < 0 {
            eprintln!(
                "omega-zero: rumble {name}: {}",
                std::io::Error::last_os_error()
            );
            return None;
        }
        Some(FfMotor {
            node,
            name,
            id: effect.id,
            running: false,
        })
    }

    fn play(&mut self, on: bool) {
        let ev = FfEvent {
            sec: 0,
            usec: 0,
            kind: EV_FF,
            code: self.id as u16,
            value: i32::from(on),
        };
        let bytes = unsafe {
            std::slice::from_raw_parts(&ev as *const FfEvent as *const u8, size_of::<FfEvent>())
        };
        match (&self.node).write_all(bytes) {
            Ok(()) => self.running = on,
            Err(e) => eprintln!("omega-zero: rumble: {e}"),
        }
    }
}

impl Drop for FfMotor {
    fn drop(&mut self) {
        if self.running {
            self.play(false);
        }
    }
}

fn first_dir(parent: &Path, keep: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    let mut names: Vec<PathBuf> = fs::read_dir(parent)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    names.sort();
    names.into_iter().find(|p| keep(p))
}

fn is_battery(dir: &Path) -> bool {
    fs::read_to_string(dir.join("type")).is_ok_and(|t| t.trim() == "Battery")
        && dir.join("capacity").is_file()
}

fn read_number(path: &Path) -> Option<u32> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn charge_at(dir: &Path) -> Charge {
    let Ok(text) = fs::read_to_string(dir.join("status")) else {
        return Charge::Unknown;
    };
    match text.trim() {
        "Charging" => Charge::Charging,
        "Discharging" => Charge::Discharging,
        "Full" => Charge::Full,
        _ => Charge::Unknown,
    }
}

fn headphones_in(gpio: &str) -> bool {
    gpio.lines()
        .find(|l| l.contains("|Headphone detection"))
        .and_then(|l| l.rsplit_once(')'))
        .is_some_and(|(_, state)| state.split_whitespace().nth(1) == Some("hi"))
}

/// Takes Wi-Fi and Bluetooth down before the machine is taken down. The RTL8821CS's driver hangs
/// the kernel's own shutdown, and so the reboot, when it has just gone into its power saving, as
/// it does after scanning for networks (Settings > Network): rcK runs to its end, every process
/// gone, and the console never resets. So the driver is let go of here, from user space, while
/// it is still awake: wpa_supplicant and the DHCP client stop first (wpa_supplicant puts the
/// interface back up as it goes, and a TERM is sometimes not enough for wpa_cli), each wireless
/// interface goes down, and its device is unbound from the driver, which takes it down cleanly,
/// so the kernel's shutdown has nothing of it left to take down.
fn quiet_radios() {
    let pids = processes(&["wpa_supplicant", "wpa_cli", "udhcpc"]);
    for signal in ["-TERM", "-KILL"] {
        let left: Vec<u32> = pids
            .iter()
            .copied()
            .filter(|pid| Path::new(&format!("/proc/{pid}")).exists())
            .collect();
        for pid in &left {
            let _ = Command::new("kill")
                .args([signal, &pid.to_string()])
                .status();
        }
        // a moment for them to go, wpa_supplicant leaving the network as it does
        for _ in 0..10 {
            if left
                .iter()
                .all(|pid| !Path::new(&format!("/proc/{pid}")).exists())
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    // each interface's device and its driver, read before any is unbound, when they go
    let mut unbind: Vec<(PathBuf, String)> = Vec::new();
    for iface in wireless_interfaces() {
        let device = fs::read_link(iface.join("device"))
            .ok()
            .and_then(|d| d.file_name()?.to_str().map(str::to_string));
        let driver = fs::canonicalize(iface.join("device/driver")).ok();
        if let Some(name) = iface.file_name().and_then(|n| n.to_str()) {
            let _ = Command::new("ifconfig").args([name, "down"]).status();
        }
        if let (Some(driver), Some(device)) = (driver, device) {
            if !unbind.contains(&(driver.clone(), device.clone())) {
                unbind.push((driver, device));
            }
        }
    }
    for (driver, device) in &unbind {
        let _ = fs::write(driver.join("unbind"), device);
    }
    if Path::new("/sys/class/bluetooth/hci0").exists() {
        let _ = Command::new("hciconfig").args(["hci0", "down"]).status();
    }
}

/// The handheld's wireless network interfaces, as /sys/class/net lists them.
fn wireless_interfaces() -> Vec<PathBuf> {
    fs::read_dir("/sys/class/net")
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.join("wireless").exists())
        .collect()
}

/// The processes running any of the programs `names`, as /proc names them.
fn processes(names: &[&str]) -> Vec<u32> {
    fs::read_dir("/proc")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let pid: u32 = entry.file_name().to_str()?.parse().ok()?;
            let comm = fs::read_to_string(entry.path().join("comm")).ok()?;
            names.contains(&comm.trim()).then_some(pid)
        })
        .collect()
}

impl Platform for DevicePlatform {
    fn supports_high_refresh(&self) -> bool {
        self.refresh.is_some()
    }

    fn set_high_refresh(&mut self, high: bool) -> bool {
        self.refresh.as_mut().is_some_and(|r| r.set(high))
    }

    fn high_refresh_enabled(&self) -> bool {
        self.refresh.as_ref().is_some_and(PanelRefresh::enabled)
    }

    fn resume_display(&mut self) {
        if let Some(refresh) = &mut self.refresh {
            refresh.resume();
        }
    }

    fn headphones(&self) -> bool {
        fs::read_to_string(self.sysfs.join("kernel/debug/gpio")).is_ok_and(|g| headphones_in(&g))
    }

    fn set_backlight(&mut self, step: u8) {
        let Some(backlight) = &self.backlight else {
            return;
        };
        let step = u32::from(step).min(TOP_STEP);
        let value = self.max_brightness * step / TOP_STEP;
        let write = |file: PathBuf, body: String| {
            if let Err(e) = fs::write(&file, body) {
                eprintln!("omega-zero: {}: {e}", file.display());
            }
        };
        match backlight {
            Backlight::Class(dir) => write(dir.join("brightness"), value.to_string()),
            Backlight::Dispdbg(dir) => {
                write(dir.join("name"), "lcd0".to_string());
                write(dir.join("command"), "setbl".to_string());
                write(dir.join("param"), value.to_string());
                write(dir.join("start"), "1".to_string());
            }
        }
    }

    fn battery(&self) -> Option<Battery> {
        let dir = self.battery.as_ref()?;
        let percent = read_number(&dir.join("capacity"))?;
        Some(Battery {
            percent: percent.min(100) as u8,
            charge: charge_at(dir),
        })
    }

    fn charge(&self) -> Charge {
        self.battery
            .as_ref()
            .map_or(Charge::Unknown, |dir| charge_at(dir))
    }

    fn set_led(&mut self, state: LedState) {
        let Some(led) = &self.led else {
            return;
        };
        match led {
            Led::Multi(dir) => {
                let (r, g, b) = match state {
                    LedState::Off => (0, 0, 0),
                    LedState::Running | LedState::Charged => (0, 255, 0),
                    LedState::Low => (255, 0, 0),
                    LedState::Charging => (255, 140, 0),
                };
                let _ = fs::write(dir.join("multi_intensity"), format!("{r} {g} {b}\n"));
                let _ = fs::write(dir.join("brightness"), "255\n");
            }
            Led::Mono { dir, max } => {
                let on = !matches!(state, LedState::Off | LedState::Low);
                let value = if on { *max } else { 0 };
                let _ = fs::write(dir.join("brightness"), format!("{value}\n"));
            }
        }
    }

    fn restart(&mut self) -> ! {
        self.breadcrumb("restart: reached omega-zero, about to sync");
        quiet_radios();
        let _ = Command::new("sync").status();
        self.breadcrumb("restart: sync returned, about to signal init");
        let _ = Command::new("reboot").status();
        std::thread::sleep(Duration::from_secs(10));
        std::process::exit(0)
    }

    fn poweroff(&mut self) -> ! {
        self.breadcrumb("poweroff: reached omega-zero, about to sync");
        quiet_radios();
        let _ = Command::new("sync").status();
        self.breadcrumb("poweroff: sync returned, about to signal init");
        let _ = Command::new("poweroff").status();
        self.breadcrumb("poweroff: signalled init, waiting for it to take the machine down");
        std::thread::sleep(Duration::from_secs(10));
        std::process::exit(0)
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn now(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }

    fn set_clock(&mut self, secs: i64) {
        ran(
            "date",
            Command::new("date").arg(format!("-s@{secs}")).status(),
        );
        ran(
            "hwclock",
            Command::new("hwclock").args(["-w", "-u"]).status(),
        );
    }

    fn relink_adb(&mut self) -> bool {
        let gadget = self.sysfs.join("kernel/config/usb_gadget/g1/UDC");
        if !gadget.is_file() {
            return false;
        }
        let Some(udc) = first_dir(&self.sysfs.join("class/udc"), |_| true) else {
            return false;
        };
        let Some(name) = udc.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            return false;
        };
        let _ = fs::write(&gadget, "\n");
        match fs::write(&gadget, &name) {
            Ok(()) => true,
            Err(e) => {
                eprintln!("omega-zero: adb {}: {e}", gadget.display());
                false
            }
        }
    }

    fn set_rumble(&mut self, strength: u16) {
        let Some(motor) = &mut self.motor else {
            return;
        };
        if let Some(on) = motor_change(strength, motor.running()) {
            motor.play(on);
        }
    }
}
