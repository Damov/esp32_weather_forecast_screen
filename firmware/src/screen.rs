// ============================================================================= //
// File          : screen.rs                                                     //
// License       : GPL-3.0-only                                                  //
// Created       : 2026-10-04                                                    //
// Last modified : 2026-10-04                                                    //
// Author        : Daniel Kharlamov                                              //
// Last editor   : Daniel Kharlamov <daniel.kharlamov@googlemail.com>            //
// ============================================================================= //
//                                                                               //
// Summary:                                                                      //
// --------                                                                      //
// Backlight blanking and wake control without stopping the CPU.                 //
//                                                                               //
// Module:                                                                       //
// -------                                                                       //
// Tracks screen activity and applies configured inactivity and nighttime rules. //
// Handles touch gestures and optional radar presence wakeups, restores          //
// brightness, and retries failed backlight updates. Display contents,           //
// networking, and application processing remain active while the backlight is   //
// off.                                                                          //
// ============================================================================= //

//! Backlight-only blanking. The CPU, display contents and network stay active.
use crate::settings::Settings;

pub const TIMEOUTS: [u32; 5] = [0, 1, 5, 10, 30];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    On,
    Off,
}
pub struct ScreenController {
    state: State,
    duty: Option<u8>,
    last_activity: u64,
    timeout: u32,
    night_awake: bool,
    activity_before_sync: bool,
    consume_touch: bool,
    wake_requested: bool,
    blank_retry_after: u64,
    write_retry_after: u64,
}
impl ScreenController {
    pub fn new(settings: &Settings, now: u64) -> Self {
        Self {
            state: State::On,
            duty: None,
            last_activity: now,
            timeout: settings.sleep_minutes,
            night_awake: false,
            activity_before_sync: false,
            consume_touch: false,
            wake_requested: false,
            blank_retry_after: 0,
            write_retry_after: 0,
        }
    }
    pub fn state(&self) -> State {
        self.state
    }
    fn nighttime(settings: &Settings, epoch: Option<i64>) -> bool {
        settings.night_mode
            && settings
                .hour(epoch)
                .is_some_and(|hour| !(6..22).contains(&hour))
    }
    fn activity(&mut self, settings: &Settings, epoch: Option<i64>, now: u64) {
        self.last_activity = now;
        self.activity_before_sync |= settings.hour(epoch).is_none();
        if Self::nighttime(settings, epoch) {
            self.night_awake = true;
        }
        self.wake_requested = true;
    }
    /// Return true for every event belonging to the gesture that wakes the screen.
    pub fn touch(
        &mut self,
        settings: &Settings,
        epoch: Option<i64>,
        now: u64,
        released: bool,
    ) -> bool {
        if self.state == State::Off {
            self.consume_touch = true;
        }
        self.activity(settings, epoch, now);
        let consume = self.consume_touch;
        if released {
            self.consume_touch = false;
        }
        consume
    }
    pub fn observe(
        &mut self,
        settings: &Settings,
        epoch: Option<i64>,
        now: u64,
        protected: bool,
        presence: bool,
        touch_pressed: bool,
    ) {
        let night = Self::nighttime(settings, epoch);
        if settings.hour(epoch).is_some() && self.activity_before_sync {
            self.night_awake = night;
            self.activity_before_sync = false;
        }
        if !night {
            self.night_awake = false;
        }
        if self.timeout != settings.sleep_minutes {
            self.timeout = settings.sleep_minutes;
            self.activity(settings, epoch, now);
        }
        // While dark, a valid debounced Down event wakes instead of noisy PENIRQ.
        if protected
            || (self.state == State::On && touch_pressed)
            || (settings.wake_on_presence && presence)
        {
            self.activity(settings, epoch, now);
        }
    }
    fn due(&self, settings: &Settings, epoch: Option<i64>, now: u64) -> bool {
        if now < self.blank_retry_after {
            return false;
        }
        if settings.sleep_minutes != 0 {
            now.saturating_sub(self.last_activity) >= u64::from(settings.sleep_minutes) * 60_000
        } else {
            Self::nighttime(settings, epoch) && !self.night_awake
        }
    }
    /// Commit screen state only after the hardware accepts its brightness.
    /// Failed blanking retries after a minute; other write failures back off a second.
    pub fn update<E>(
        &mut self,
        settings: &Settings,
        epoch: Option<i64>,
        now: u64,
        protected: bool,
        mut write: impl FnMut(u8) -> Result<(), E>,
    ) -> Result<Option<State>, E> {
        let target = match self.state {
            State::Off if protected || self.wake_requested => State::On,
            State::On if !protected && self.due(settings, epoch, now) => State::Off,
            other => other,
        };
        let value = if target == State::Off {
            0
        } else if protected {
            settings.brightness.max(128)
        } else {
            settings.brightness
        };
        if now < self.write_retry_after || (target == self.state && self.duty == Some(value)) {
            return Ok(None);
        }
        if let Err(error) = write(value) {
            if target == State::Off {
                self.blank_retry_after = now.saturating_add(60_000);
            }
            self.write_retry_after = now.saturating_add(1000);
            self.wake_requested = false;
            return Err(error);
        }
        self.state = target;
        self.duty = Some(value);
        self.wake_requested = false;
        self.write_retry_after = 0;
        Ok(Some(target))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn settings(minutes: u32) -> Settings {
        let mut s = Settings {
            sleep_minutes: minutes,
            brightness: 80,
            ..Settings::default()
        };
        s.location.timezone = "UTC".into();
        s
    }
    fn epoch(hour: u32) -> i64 {
        use chrono::TimeZone;
        chrono::Utc
            .with_ymd_and_hms(2026, 10, 2, hour, 0, 0)
            .unwrap()
            .timestamp()
    }
    fn update(
        c: &mut ScreenController,
        s: &Settings,
        time: Option<i64>,
        now: u64,
        protected: bool,
    ) -> Vec<u8> {
        let mut writes = vec![];
        c.update(s, time, now, protected, |value| {
            writes.push(value);
            Ok::<_, ()>(())
        })
        .unwrap();
        writes
    }
    #[test]
    fn every_preset_and_night_deadlines() {
        for minutes in TIMEOUTS.into_iter().filter(|m| *m != 0) {
            for hour in [12, 23] {
                let mut s = settings(minutes);
                s.night_mode = true;
                let mut c = ScreenController::new(&s, 100);
                assert_eq!(update(&mut c, &s, Some(epoch(hour)), 100, false), vec![80]);
                let deadline = 100 + u64::from(minutes) * 60_000;
                assert!(update(&mut c, &s, Some(epoch(hour)), deadline - 1, false).is_empty());
                assert_eq!(
                    update(&mut c, &s, Some(epoch(hour)), deadline, false),
                    vec![0]
                );
                assert_eq!(c.state(), State::Off);
                assert!(update(&mut c, &s, Some(epoch(hour)), deadline + 1, false).is_empty());
            }
        }
        let s = settings(0);
        let mut c = ScreenController::new(&s, 0);
        update(&mut c, &s, None, 0, false);
        assert!(update(&mut c, &s, Some(epoch(23)), u64::MAX, false).is_empty());
        assert_eq!(c.state(), State::On);
    }
    #[test]
    fn touch_wake_consumes_entire_gesture_and_restores_brightness() {
        let mut s = settings(1);
        let mut c = ScreenController::new(&s, 0);
        update(&mut c, &s, None, 0, false);
        update(&mut c, &s, None, 60_000, false);
        assert!(c.touch(&s, None, 60_020, false));
        assert_eq!(c.state(), State::Off);
        assert_eq!(update(&mut c, &s, None, 60_020, false), vec![80]);
        assert!(c.touch(&s, None, 60_040, false));
        assert!(c.touch(&s, None, 60_060, true));
        assert!(!c.touch(&s, None, 60_080, false));
        s.brightness = 120;
        assert_eq!(update(&mut c, &s, None, 60_080, false), vec![120]);
        c.observe(&s, None, 120_080, false, false, true);
        assert!(update(&mut c, &s, None, 120_080, false).is_empty());
    }
    #[test]
    fn presence_switch_protection_and_timeout_changes() {
        let mut s = settings(1);
        let mut c = ScreenController::new(&s, 0);
        update(&mut c, &s, None, 0, false);
        update(&mut c, &s, None, 60_000, false);
        s.wake_on_presence = false;
        c.observe(&s, None, 60_001, false, true, true);
        assert!(update(&mut c, &s, None, 60_001, false).is_empty());
        assert_eq!(c.state(), State::Off);
        s.wake_on_presence = true;
        c.observe(&s, None, 60_020, false, true, false);
        assert_eq!(update(&mut c, &s, None, 60_020, false), vec![80]);
        c.observe(&s, None, 120_020, false, true, false);
        assert!(update(&mut c, &s, None, 120_020, false).is_empty());
        assert_eq!(update(&mut c, &s, None, 180_020, false), vec![0]);
        c.observe(&s, None, 180_040, true, false, false);
        assert_eq!(update(&mut c, &s, None, 180_040, true), vec![128]);
        c.observe(&s, None, 240_040, true, false, false);
        assert!(update(&mut c, &s, None, 240_040, true).is_empty());
        s.sleep_minutes = 5;
        c.observe(&s, None, 240_060, false, false, false);
        assert_eq!(update(&mut c, &s, None, 240_060, false), vec![80]);
        assert!(update(&mut c, &s, None, 540_059, false).is_empty());
        assert_eq!(update(&mut c, &s, None, 540_060, false), vec![0]);
    }
    #[test]
    fn night_override_and_morning_stays_dark_until_activity() {
        let mut s = settings(0);
        s.night_mode = true;
        let mut c = ScreenController::new(&s, 0);
        c.observe(&s, Some(epoch(23)), 0, false, false, false);
        assert_eq!(update(&mut c, &s, Some(epoch(23)), 0, false), vec![0]);
        c.observe(&s, Some(epoch(6)), 7 * 3600_000, false, false, false);
        assert!(update(&mut c, &s, Some(epoch(6)), 7 * 3600_000, false).is_empty());
        assert_eq!(c.state(), State::Off);
        assert!(c.touch(&s, Some(epoch(6)), 7 * 3600_000 + 20, false));
        assert_eq!(
            update(&mut c, &s, Some(epoch(6)), 7 * 3600_000 + 20, false),
            vec![80]
        );
        c.touch(&s, Some(epoch(6)), 7 * 3600_000 + 40, true);
        c.observe(&s, Some(epoch(23)), 24 * 3600_000, false, false, false);
        assert_eq!(
            update(&mut c, &s, Some(epoch(23)), 24 * 3600_000, false),
            vec![0]
        );
        c.touch(&s, Some(epoch(23)), 24 * 3600_000 + 20, false);
        update(&mut c, &s, Some(epoch(23)), 24 * 3600_000 + 20, false);
        assert!(update(&mut c, &s, Some(epoch(23)), 48 * 3600_000, false).is_empty());
        let mut c = ScreenController::new(&s, 0);
        c.touch(&s, None, 1, false);
        c.touch(&s, None, 2, true);
        c.observe(&s, Some(epoch(23)), 3, false, false, false);
        assert_eq!(update(&mut c, &s, Some(epoch(23)), 3, false), vec![80]);
    }
    #[test]
    fn hardware_failures_preserve_state_and_remain_retryable() {
        let s = settings(1);
        let mut c = ScreenController::new(&s, 0);
        update(&mut c, &s, None, 0, false);
        assert!(c.update(&s, None, 60_000, false, |_| Err(())).is_err());
        assert_eq!(c.state(), State::On);
        assert!(update(&mut c, &s, None, 119_999, false).is_empty());
        assert_eq!(update(&mut c, &s, None, 120_000, false), vec![0]);
        c.touch(&s, None, 120_020, false);
        assert!(c.update(&s, None, 120_020, false, |_| Err(())).is_err());
        assert_eq!(c.state(), State::Off);
        c.touch(&s, None, 120_060, true);
        assert!(update(&mut c, &s, None, 120_060, false).is_empty());
        c.touch(&s, None, 121_020, false);
        assert_eq!(update(&mut c, &s, None, 121_020, false), vec![80]);
        assert!(c.touch(&s, None, 121_040, true));
        assert!(!c.touch(&s, None, 121_060, false));
    }
}
