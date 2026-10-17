//! How the device's loop is paced while the panel runs at 120 Hz. At 60 Hz each swap waits for
//! the one refresh a game frame has, and the loop is paced by the swap. At 120 Hz a swap comes
//! back after half as long, so counting swaps would run every game twice as fast: there the
//! loop keeps its own clock instead, ticking the emulator once a game period and presenting
//! that frame at the next of the panel's refreshes, which come twice as often.

use std::time::{Duration, Instant};

/// How long a tick stands for on the shelf and in the menus, and for a core whose rate is not
/// near 60: the 60 Hz the frontend has always presented at.
pub const UI_PERIOD: Duration = Duration::from_nanos(16_666_667);

/// What the loop is paced by: how long each tick of the emulator stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameClock {
    pub period: Duration,
}

impl Default for FrameClock {
    fn default() -> Self {
        Self { period: UI_PERIOD }
    }
}

/// How long one of a core's frames lasts at `fps`, or the UI's period for a rate that is none.
pub fn native_period(fps: f64) -> Duration {
    if fps.is_finite() && fps > 0.0 {
        Duration::try_from_secs_f64(1.0 / fps)
            .ok()
            .filter(|period| !period.is_zero() && period.as_nanos() <= u128::from(u64::MAX))
            .unwrap_or(UI_PERIOD)
    } else {
        UI_PERIOD
    }
}

/// Native game deadlines do not follow swap returns. At most two frames may be
/// committed before one presentation, compensating a slightly slower panel.
pub struct FramePacer {
    clock: FrameClock,
    next: Instant,
    pub dropped: u64,
}

impl FramePacer {
    pub fn new(now: Instant) -> Self {
        Self {
            clock: FrameClock::default(),
            next: now,
            dropped: 0,
        }
    }

    fn sync(&mut self, clock: FrameClock, now: Instant) {
        if clock.period != self.clock.period {
            self.next = now;
        }
        self.clock = clock;
    }

    pub fn deadline(&mut self, clock: FrameClock, now: Instant) -> Instant {
        self.sync(clock, now);
        self.next
    }

    pub fn due(&mut self, clock: FrameClock, now: Instant) -> u32 {
        self.sync(clock, now);
        let Some(late) = now.checked_duration_since(self.next) else {
            return 0;
        };
        let debt = late.as_nanos() / clock.period.as_nanos();
        if debt > 2 {
            self.dropped += debt as u64;
            self.next = now;
            1
        } else {
            (debt as u32 + 1).min(2)
        }
    }

    pub fn stepped(&mut self, clock: FrameClock, now: Instant) {
        self.sync(clock, now);
        self.next += clock.period;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_core_rates_fall_back_to_ui_rate() {
        for fps in [0.0, -1.0, f64::NAN, f64::INFINITY, 1e-20, 1e30] {
            assert_eq!(native_period(fps), UI_PERIOD);
        }
        assert_eq!(native_period(50.0), Duration::from_millis(20));
    }

    fn simulate(panel_hz: f64) -> (u64, u64, u64) {
        let start = Instant::now();
        let mut now = start;
        let clock = FrameClock {
            period: native_period(59.7275),
        };
        let mut pacer = FramePacer::new(start);
        let panel = Duration::from_secs_f64(1.0 / panel_hz);
        let mut committed = 0;
        let mut presented = 0;
        while now < start + Duration::from_secs(60) {
            now = now.max(pacer.deadline(clock, now));
            let due = pacer.due(clock, now);
            for _ in 0..due {
                committed += 1;
                now += Duration::from_millis(2);
                pacer.stepped(clock, now);
            }
            if due > 0 {
                presented += 1;
                now += Duration::from_millis(1);
                let scans = (now - start).as_nanos() / panel.as_nanos() + 1;
                now = start + panel * scans as u32;
            }
        }
        (committed, presented, pacer.dropped)
    }

    #[test]
    fn actual_steps_stay_native_at_slow_60_and_120_hz_swaps() {
        for hz in [59.56, 60.0, 120.15] {
            let (committed, presented, dropped) = simulate(hz);
            assert!(
                (committed as f64 - 60.0 * 59.7275).abs() <= 2.0,
                "{hz}: {committed}"
            );
            assert_eq!(dropped, 0, "{hz}: ordinary panel drift lost game time");
            assert!(presented <= committed);
            if hz < 59.7275 {
                assert!(
                    presented < committed,
                    "slow scanout did not skip a presentation"
                );
            }
        }
    }

    #[test]
    fn a_120_hz_panel_presents_once_a_game_frame_not_once_a_refresh() {
        let (committed, presented, _) = simulate(120.15);
        assert_eq!(
            presented, committed,
            "a fast panel presented a frame twice, or skipped one"
        );
    }

    #[test]
    fn long_stalls_rebase_but_minor_debt_gets_two_steps() {
        let start = Instant::now();
        let clock = FrameClock::default();
        let mut pacer = FramePacer::new(start);
        assert_eq!(pacer.due(clock, start + clock.period), 2);
        let later = start + Duration::from_secs(2);
        assert_eq!(pacer.due(clock, later), 1);
        assert!(pacer.dropped > 100);
        pacer.stepped(clock, later);
        assert_eq!(pacer.due(clock, later), 0);
    }

    #[test]
    fn changing_between_ui_and_native_period_resets_the_phase() {
        let start = Instant::now();
        let mut pacer = FramePacer::new(start);
        let native = FrameClock {
            period: native_period(50.0),
        };
        let resumed = start + Duration::from_secs(20);
        assert_eq!(pacer.due(native, resumed), 1);
        pacer.stepped(native, resumed);
        assert_eq!(pacer.due(native, resumed), 0);
        let menu = resumed + Duration::from_millis(5);
        assert_eq!(pacer.due(FrameClock::default(), menu), 1);
        assert_eq!(pacer.dropped, 0, "a mode change was treated as game debt");
    }
}
