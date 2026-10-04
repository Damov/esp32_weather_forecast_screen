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

// Timeouts. This fixed value is shared by the operations below.
pub const TIMEOUTS: [u32; 5] = [0, 1, 5, 10, 30];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    // Backlight state after the hardware accepted an illuminated duty.
    On,
    // Backlight is blanked while display contents and application processing continue.
    Off,
}
pub struct ScreenController {
    // State. Stored as State.
    state: State,
    // Duty. Stored as Option<u8>.
    duty: Option<u8>,
    // Last activity. Stored as u64.
    last_activity: u64,
    // Timeout. Stored as u32.
    timeout: u32,
    // Night awake. Stored as bool.
    night_awake: bool,
    // Activity before sync. Stored as bool.
    activity_before_sync: bool,
    // Consume touch. Stored as bool.
    consume_touch: bool,
    // Wake requested. Stored as bool.
    wake_requested: bool,
    // Blank retry after. Stored as u64.
    blank_retry_after: u64,
    // Write retry after. Stored as u64.
    write_retry_after: u64,
}
impl ScreenController {
    /// Creates an initially illuminated backlight controller.
    ///
    /// # Arguments
    ///
    /// * `settings` (`&Settings`) - Current device preferences controlling time, brightness,
    ///   and inactivity behavior.
    /// * `now` (`u64`) - Monotonic timestamp in milliseconds, using the same origin as stored
    ///   deadlines.
    ///
    /// # Returns
    ///
    /// `Self` - On controller with no committed duty value and an inactivity baseline at now.
    pub fn new(settings: &Settings, now: u64) -> Self {
        Self {
            // Initialize state from the supplied value.
            state: State::On,
            // Leave duty unavailable until a later operation supplies it.
            duty: None,
            // Initialize last activity from the supplied value.
            last_activity: now,
            // Initialize timeout from the supplied value.
            timeout: settings.sleep_minutes,
            // Initialize night awake as disabled/inactive.
            night_awake: false,
            // Initialize activity before sync as disabled/inactive.
            activity_before_sync: false,
            // Initialize consume touch as disabled/inactive.
            consume_touch: false,
            // Initialize wake requested as disabled/inactive.
            wake_requested: false,
            // Start blank retry after at zero; later operations update it as needed.
            blank_retry_after: 0,
            // Start write retry after at zero; later operations update it as needed.
            write_retry_after: 0,
        }
    }
    /// Returns the last successfully committed screen state.
    ///
    /// # Arguments
    ///
    /// * `self` (`&ScreenController`) - Committed backlight state, activity timers, and pending
    ///   wake/retry state. Borrowed without changing it.
    ///
    /// # Returns
    ///
    /// `State` - On or Off, reflecting accepted hardware brightness updates.
    pub fn state(&self) -> State {
        // Return state as the value of this block.
        self.state
    }
    /// Checks the enabled night schedule in the saved location's timezone.
    ///
    /// # Arguments
    ///
    /// * `settings` (`&Settings`) - Current device preferences controlling time, brightness,
    ///   and inactivity behavior.
    /// * `epoch` (`Option<i64>`) - UTC Unix timestamp in seconds; None means synchronized time
    ///   is unavailable.
    ///
    /// # Returns
    ///
    /// `bool` - True from 22:00 through 05:59 when night mode and valid local time are
    /// available; false otherwise.
    fn nighttime(settings: &Settings, epoch: Option<i64>) -> bool {
        // Return whether the local 22:00-06:00 backlight schedule is enabled and the optional value
        // exists and satisfies the additional predicate as the value of this block.
        settings.night_mode
            && settings
                .hour(epoch)
                .is_some_and(|hour| !(6..22).contains(&hour))
    }
    /// Records activity and requests a wakeup, including a nighttime override.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut ScreenController`) - Committed backlight state, activity timers, and
    ///   pending wake/retry state. Mutated in place.
    /// * `settings` (`&Settings`) - Current device preferences controlling time, brightness,
    ///   and inactivity behavior.
    /// * `epoch` (`Option<i64>`) - UTC Unix timestamp in seconds; None means synchronized time
    ///   is unavailable.
    /// * `now` (`u64`) - Monotonic timestamp in milliseconds, using the same origin as stored
    ///   deadlines.
    ///
    /// # Returns
    ///
    /// `()` - No value; resets the inactivity timer and records activity before time
    /// synchronization.
    fn activity(&mut self, settings: &Settings, epoch: Option<i64>, now: u64) {
        // Set last activity to monotonic time used to compare activity and request deadlines.
        self.last_activity = now;
        // Update activity before sync using hour result for the surrounding operation is
        // unavailable, retaining the accumulated state for subsequent steps.
        self.activity_before_sync |= settings.hour(epoch).is_none();
        // Activity during the configured local night overrides that night's automatic blanking.
        if Self::nighttime(settings, epoch) {
            // Set night awake to the enabled state.
            self.night_awake = true;
        }
        // Set wake requested to the enabled state.
        self.wake_requested = true;
    }
    /// Return true for every event belonging to the gesture that wakes the screen.
    ///
    /// Records touch activity and consumes the entire gesture that wakes a dark screen.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut ScreenController`) - Committed backlight state, activity timers, and
    ///   pending wake/retry state. Mutated in place.
    /// * `settings` (`&Settings`) - Current device preferences controlling time, brightness,
    ///   and inactivity behavior.
    /// * `epoch` (`Option<i64>`) - UTC Unix timestamp in seconds; None means synchronized time
    ///   is unavailable.
    /// * `now` (`u64`) - Monotonic timestamp in milliseconds, using the same origin as stored
    ///   deadlines.
    /// * `released` (`bool`) - Whether this event ends the current touch gesture.
    ///
    /// # Returns
    ///
    /// `bool` - True for events that must not activate UI controls; false for ordinary
    /// illuminated-screen gestures.
    pub fn touch(
        &mut self,
        settings: &Settings,
        epoch: Option<i64>,
        now: u64,
        released: bool,
    ) -> bool {
        // A gesture beginning in darkness is reserved for waking and must not activate a UI
        // control.
        if self.state == State::Off {
            // Set consume touch to the enabled state.
            self.consume_touch = true;
        }
        // Records activity and requests a wakeup, including a nighttime override.
        self.activity(settings, epoch, now);
        // Keep whether this touch belongs to a waking gesture that must not reach UI actions in
        // this local variable for the following operations.
        let consume = self.consume_touch;
        // Release ends the consumed waking gesture, allowing the next independent touch to reach
        // the UI.
        if released {
            // Set consume touch to the disabled state.
            self.consume_touch = false;
        }
        // Return whether this touch belongs to a waking gesture that must not reach UI actions as
        // the value of this block.
        consume
    }
    /// Updates inactivity and wake requests from preferences, calibration, touch, and presence.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut ScreenController`) - Committed backlight state, activity timers, and
    ///   pending wake/retry state. Mutated in place.
    /// * `settings` (`&Settings`) - Current device preferences controlling time, brightness,
    ///   and inactivity behavior.
    /// * `epoch` (`Option<i64>`) - UTC Unix timestamp in seconds; None means synchronized time
    ///   is unavailable.
    /// * `now` (`u64`) - Monotonic timestamp in milliseconds, using the same origin as stored
    ///   deadlines.
    /// * `protected` (`bool`) - Whether calibration or another protected interaction prevents
    ///   blanking.
    /// * `presence` (`bool`) - Combined fresh UART or OUT presence indication.
    /// * `touch_pressed` (`bool`) - Raw touch interrupt assertion; only illuminated-screen
    ///   contact counts as activity here.
    ///
    /// # Returns
    ///
    /// `()` - No value; records qualifying activity and resets nighttime overrides during
    /// daytime.
    pub fn observe(
        &mut self,
        settings: &Settings,
        epoch: Option<i64>,
        now: u64,
        protected: bool,
        presence: bool,
        touch_pressed: bool,
    ) {
        // Keep whether the selected local time lies within the enabled nighttime schedule in this
        // local variable for the following operations.
        let night = Self::nighttime(settings, epoch);
        // Once local time becomes available, apply earlier activity to the current night override
        // rather than forgetting it.
        if settings.hour(epoch).is_some() && self.activity_before_sync {
            // Set night awake to whether the selected local time lies within the enabled nighttime
            // schedule.
            self.night_awake = night;
            // Set activity before sync to the disabled state.
            self.activity_before_sync = false;
        }
        // Daytime ends the prior night's override; a future night gets its own schedule decision.
        if !night {
            // Set night awake to the disabled state.
            self.night_awake = false;
        }
        // Changing the inactivity interval restarts the activity baseline rather than immediately
        // expiring the old timer.
        if self.timeout != settings.sleep_minutes {
            // Set timeout to inactivity interval in minutes.
            self.timeout = settings.sleep_minutes;
            // Records activity and requests a wakeup, including a nighttime override.
            self.activity(settings, epoch, now);
        }
        // While dark, a valid debounced Down event wakes instead of noisy PENIRQ.
        // Calibration, illuminated-screen touch, or enabled radar presence counts as activity;
        // ordinary network traffic does not.
        if protected
            || (self.state == State::On && touch_pressed)
            || (settings.wake_on_presence && presence)
        {
            // Records activity and requests a wakeup, including a nighttime override.
            self.activity(settings, epoch, now);
        }
    }
    /// Checks whether inactivity or an unoverridden night schedule requires blanking.
    ///
    /// # Arguments
    ///
    /// * `self` (`&ScreenController`) - Committed backlight state, activity timers, and pending
    ///   wake/retry state. Borrowed without changing it.
    /// * `settings` (`&Settings`) - Current device preferences controlling time, brightness,
    ///   and inactivity behavior.
    /// * `epoch` (`Option<i64>`) - UTC Unix timestamp in seconds; None means synchronized time
    ///   is unavailable.
    /// * `now` (`u64`) - Monotonic timestamp in milliseconds, using the same origin as stored
    ///   deadlines.
    ///
    /// # Returns
    ///
    /// `bool` - True when blanking is due and the retry delay has elapsed; false otherwise.
    fn due(&self, settings: &Settings, epoch: Option<i64>, now: u64) -> bool {
        // A failed blanking attempt is still within its sixty-second retry delay.
        if now < self.blank_retry_after {
            // Leave this function now with the disabled state; later statements are skipped.
            return false;
        }
        // A finite timeout uses elapsed monotonic inactivity; Never instead relies on an enabled,
        // unoverridden night schedule.
        if settings.sleep_minutes != 0 {
            // Return elapsed difference clamped at zero instead of unsigned underflow has reached
            // or exceeded the supplied value converted into the requested representation multiplied
            // by 60_000 as the value of this block.
            now.saturating_sub(self.last_activity) >= u64::from(settings.sleep_minutes) * 60_000
        } else {
            // Return checks the enabled night schedule in the saved location's timezone and the
            // inverse of night awake as the value of this block.
            Self::nighttime(settings, epoch) && !self.night_awake
        }
    }
    /// Commit screen state only after the hardware accepts its brightness.
    /// Failed blanking retries after a minute; other write failures back off a second.
    ///
    /// Writes required brightness and commits state only after hardware success.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut ScreenController`) - Committed backlight state, activity timers, and
    ///   pending wake/retry state. Mutated in place.
    /// * `settings` (`&Settings`) - Current device preferences controlling time, brightness,
    ///   and inactivity behavior.
    /// * `epoch` (`Option<i64>`) - UTC Unix timestamp in seconds; None means synchronized time
    ///   is unavailable.
    /// * `now` (`u64`) - Monotonic timestamp in milliseconds, using the same origin as stored
    ///   deadlines.
    /// * `protected` (`bool`) - Whether calibration or another protected interaction prevents
    ///   blanking.
    /// * `write` (`impl FnMut(u8) -> Result<(), E>`) - Brightness callback receiving duty in
    ///   0-255; returns Ok(()) after hardware acceptance or Err(E) on failure.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Caller-defined error returned by the hardware or persistence callback.
    ///
    /// # Returns
    ///
    /// `Result<Option<State>, E>` - Ok(Some(state)) after a successful brightness write,
    /// Ok(None) when unchanged or backing off, or Err from the writer.
    ///
    /// # Errors
    ///
    /// Propagates writer errors; failed blanking retries after 60 seconds and other writes back
    /// off for one second.
    pub fn update<E>(
        &mut self,
        settings: &Settings,
        epoch: Option<i64>,
        now: u64,
        protected: bool,
        mut write: impl FnMut(u8) -> Result<(), E>,
    ) -> Result<Option<State>, E> {
        // Keep requested screen state or calibration position, selected before applying its side
        // effects in this local variable for the following operations.
        let target = match self.state {
            // Handle the State Off if protected || self.wake requested case: apply the
            // state-specific behavior shown here.
            State::Off if protected || self.wake_requested => State::On,
            // Handle the State On if !protected && self.due(settings, epoch, now) case: apply the
            // state-specific behavior shown here.
            State::On if !protected && self.due(settings, epoch, now) => State::Off,
            // Handle the other case: apply the state-specific behavior shown here.
            other => other,
        };
        // Keep proposed value checked or applied by this operation in this local variable for the
        // following operations.
        let value = if target == State::Off {
            0
        // Calibration must remain readable, so protected interaction uses at least half brightness.
        } else if protected {
            // Produce the larger value, enforcing the lower bound.
            settings.brightness.max(128)
        } else {
            // Return backlight duty in 0-255 as the value of this block.
            settings.brightness
        };
        // Skip writes during error backoff or when both the committed screen state and duty already
        // match.
        if now < self.write_retry_after || (target == self.state && self.duty == Some(value)) {
            // Leave this function now with a successful result carrying no available value; later
            // statements are skipped.
            return Ok(None);
        }
        // The hardware rejected brightness; preserve the previously committed state and schedule a
        // retry.
        if let Err(error) = write(value) {
            // A blanked screen needs zero duty; an illuminated screen uses the configured or
            // protected brightness.
            if target == State::Off {
                // Set blank retry after to the advanced value clamped at the integer limit instead
                // of overflow.
                self.blank_retry_after = now.saturating_add(60_000);
            }
            // Set write retry after to the advanced value clamped at the integer limit instead of
            // overflow.
            self.write_retry_after = now.saturating_add(1000);
            // Set wake requested to the disabled state.
            self.wake_requested = false;
            // Leave this function now with a failure result for the surrounding operation; later
            // statements are skipped.
            return Err(error);
        }
        // Set state to requested screen state or calibration position, selected before applying its
        // side effects.
        self.state = target;
        // Set duty to an available value for proposed value checked or applied by this operation.
        self.duty = Some(value);
        // Set wake requested to the disabled state.
        self.wake_requested = false;
        // Set write retry after to 0.
        self.write_retry_after = 0;
        // Return success; the caller receives an available value for requested screen state or
        // calibration position, selected before applying its side effects.
        Ok(Some(target))
    }
}

///////////////////////////////////////////////////////////////////////////////////////
//                                    UNIT TESTS                                     //
///////////////////////////////////////////////////////////////////////////////////////

/// Unit tests and supporting fixtures for screen behavior.
#[cfg(test)]
mod tests {
    use super::*;
    /// Builds UTC-based screen preferences with a chosen timeout.
    ///
    /// # Arguments
    ///
    /// * `minutes` (`u32`) - Inactivity interval in minutes; zero means Never.
    ///
    /// # Returns
    ///
    /// `Settings` - Settings fixture with brightness 80 and the supplied inactivity interval.
    fn settings(minutes: u32) -> Settings {
        // Keep preference or service fixture configured for this test scenario in this local
        // variable for the following operations.
        let mut s = Settings {
            // Initialize inactivity interval in minutes; zero disables ordinary inactivity blanking
            // from the supplied value.
            sleep_minutes: minutes,
            // Initialize backlight duty in 0-255; zero turns illumination off from the supplied
            // value.
            brightness: 80,
            ..Settings::default()
        };
        // Set IANA timezone identifier used for local clocks and forecast dates to into result for
        // the surrounding operation.
        s.location.timezone = "UTC".into();
        // Return preference or service fixture configured for this test scenario as the value of
        // this block.
        s
    }
    /// Constructs a UTC timestamp for a fixed screen-scheduling test date.
    ///
    /// # Arguments
    ///
    /// * `hour` (`u32`) - Hour of day in 0-23.
    ///
    /// # Returns
    ///
    /// `i64` - Unix seconds for the specified hour on 2 October 2026.
    ///
    /// # Panics
    ///
    /// Panics if the fixed fixture, test timestamp, or simulated service cannot be constructed.
    fn epoch(hour: u32) -> i64 {
        use chrono::TimeZone;
        // Execute timestamp for the required fixture or invariant value, panicking if unavailable.
        chrono::Utc
            .with_ymd_and_hms(2026, 10, 2, hour, 0, 0)
            .unwrap()
            .timestamp()
    }
    /// Runs a screen update using a successful brightness recorder.
    ///
    /// # Arguments
    ///
    /// * `c` (`&mut ScreenController`) - Mutable controller whose brightness writes are
    ///   recorded by the test.
    /// * `s` (`&Settings`) - Preferences used by the test screen controller.
    /// * `time` (`Option<i64>`) - UTC Unix timestamp in seconds; None means time is unavailable
    ///   when optional.
    /// * `now` (`u64`) - Monotonic timestamp in milliseconds, using the same origin as stored
    ///   deadlines.
    /// * `protected` (`bool`) - Whether calibration or another protected interaction prevents
    ///   blanking.
    ///
    /// # Returns
    ///
    /// `Vec<u8>` - Vector of brightness duties requested during the update, possibly empty.
    fn update(
        c: &mut ScreenController,
        s: &Settings,
        time: Option<i64>,
        now: u64,
        protected: bool,
    ) -> Vec<u8> {
        // Keep record of transferred rectangles or persistence attempts used to verify side effects
        // in this local variable for the following operations.
        let mut writes = vec![];
        // Require the value guaranteed by this test fixture or internal invariant; unexpected
        // absence panics.
        c.update(s, time, now, protected, |value| {
            // Append the new entry to the collection, preserving the order in which values arrive.
            writes.push(value);
            // Run the < , ()> operation with the supplied inputs.
            Ok::<_, ()>(())
        })
        .unwrap();
        // Return record of transferred rectangles or persistence attempts used to verify side
        // effects as the value of this block.
        writes
    }
    /// Verifies each supported inactivity interval and blanking at the expected daytime or
    /// nighttime deadline.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `()` - No value; completes when the expected test assertions hold.
    ///
    /// # Panics
    ///
    /// Assertions or fixture assumptions panic if the tested behavior is violated.
    #[test]
    fn every_preset_and_night_deadlines() {
        // Visit each entry in filter result for the surrounding operation; the loop binding
        // provides its value or index for this iteration.
        for minutes in TIMEOUTS.into_iter().filter(|m| *m != 0) {
            // Run each listed scenario or target in the declared order; the loop variables select
            // the data for that case.
            for hour in [12, 23] {
                // Keep preference or service fixture configured for this test scenario in this
                // local variable for the following operations.
                let mut s = settings(minutes);
                // Set whether the local 22:00-06:00 backlight schedule is enabled to the enabled
                // state.
                s.night_mode = true;
                // Keep credential or controller fixture used by the surrounding test in this local
                // variable for the following operations.
                let mut c = ScreenController::new(&s, 100);
                // Verify that writes required brightness and commits state only after hardware
                // success exactly matches the formatted text or fixture created by vec.
                assert_eq!(update(&mut c, &s, Some(epoch(hour)), 100, false), vec![80]);
                // Keep monotonic time at which the pending operation expires in this local variable
                // for the following operations.
                let deadline = 100 + u64::from(minutes) * 60_000;
                // Verify writes required brightness and commits state only after hardware success
                // is empty. A violation means the tested behavior is incorrect.
                assert!(update(&mut c, &s, Some(epoch(hour)), deadline - 1, false).is_empty());
                // Verify that writes required brightness and commits state only after hardware
                // success exactly matches the formatted text or fixture created by vec.
                assert_eq!(
                    update(&mut c, &s, Some(epoch(hour)), deadline, false),
                    vec![0]
                );
                // Verify that returns the last successfully committed screen state exactly matches
                // Off state.
                assert_eq!(c.state(), State::Off);
                // Verify writes required brightness and commits state only after hardware success
                // is empty. A violation means the tested behavior is incorrect.
                assert!(update(&mut c, &s, Some(epoch(hour)), deadline + 1, false).is_empty());
            }
        }
        // Keep preference or service fixture configured for this test scenario in this local
        // variable for the following operations.
        let s = settings(0);
        // Keep credential or controller fixture used by the surrounding test in this local variable
        // for the following operations.
        let mut c = ScreenController::new(&s, 0);
        // Writes required brightness and commits state only after hardware success.
        update(&mut c, &s, None, 0, false);
        // Verify writes required brightness and commits state only after hardware success is empty.
        // A violation means the tested behavior is incorrect.
        assert!(update(&mut c, &s, Some(epoch(23)), u64::MAX, false).is_empty());
        // Verify that returns the last successfully committed screen state exactly matches On
        // state.
        assert_eq!(c.state(), State::On);
    }
    /// Verifies that waking restores brightness and consumes every event through the gesture's
    /// release.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `()` - No value; completes when the expected test assertions hold.
    ///
    /// # Panics
    ///
    /// Assertions or fixture assumptions panic if the tested behavior is violated.
    #[test]
    fn touch_wake_consumes_entire_gesture_and_restores_brightness() {
        // Keep preference or service fixture configured for this test scenario in this local
        // variable for the following operations.
        let mut s = settings(1);
        // Keep credential or controller fixture used by the surrounding test in this local variable
        // for the following operations.
        let mut c = ScreenController::new(&s, 0);
        // Writes required brightness and commits state only after hardware success.
        update(&mut c, &s, None, 0, false);
        // Writes required brightness and commits state only after hardware success.
        update(&mut c, &s, None, 60_000, false);
        // Verify records touch activity and consumes the entire gesture that wakes a dark screen. A
        // violation means the tested behavior is incorrect.
        assert!(c.touch(&s, None, 60_020, false));
        // Verify that returns the last successfully committed screen state exactly matches Off
        // state.
        assert_eq!(c.state(), State::Off);
        // Verify that writes required brightness and commits state only after hardware success
        // exactly matches the formatted text or fixture created by vec.
        assert_eq!(update(&mut c, &s, None, 60_020, false), vec![80]);
        // Verify records touch activity and consumes the entire gesture that wakes a dark screen. A
        // violation means the tested behavior is incorrect.
        assert!(c.touch(&s, None, 60_040, false));
        // Verify records touch activity and consumes the entire gesture that wakes a dark screen. A
        // violation means the tested behavior is incorrect.
        assert!(c.touch(&s, None, 60_060, true));
        // Verify the inverse of records touch activity and consumes the entire gesture that wakes a
        // dark screen. A violation means the tested behavior is incorrect.
        assert!(!c.touch(&s, None, 60_080, false));
        // Set backlight duty in 0-255 to 120.
        s.brightness = 120;
        // Verify that writes required brightness and commits state only after hardware success
        // exactly matches the formatted text or fixture created by vec.
        assert_eq!(update(&mut c, &s, None, 60_080, false), vec![120]);
        // Updates inactivity and wake requests from preferences, calibration, touch, and presence.
        c.observe(&s, None, 120_080, false, false, true);
        // Verify writes required brightness and commits state only after hardware success is empty.
        // A violation means the tested behavior is incorrect.
        assert!(update(&mut c, &s, None, 120_080, false).is_empty());
    }
    /// Verifies enabled versus disabled radar wake, protected activity, and inactivity reset
    /// after timeout changes.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `()` - No value; completes when the expected test assertions hold.
    ///
    /// # Panics
    ///
    /// Assertions or fixture assumptions panic if the tested behavior is violated.
    #[test]
    fn presence_switch_protection_and_timeout_changes() {
        // Keep preference or service fixture configured for this test scenario in this local
        // variable for the following operations.
        let mut s = settings(1);
        // Keep credential or controller fixture used by the surrounding test in this local variable
        // for the following operations.
        let mut c = ScreenController::new(&s, 0);
        // Writes required brightness and commits state only after hardware success.
        update(&mut c, &s, None, 0, false);
        // Writes required brightness and commits state only after hardware success.
        update(&mut c, &s, None, 60_000, false);
        // Set whether radar presence counts as activity and can restore the backlight to the
        // disabled state.
        s.wake_on_presence = false;
        // Updates inactivity and wake requests from preferences, calibration, touch, and presence.
        c.observe(&s, None, 60_001, false, true, true);
        // Verify writes required brightness and commits state only after hardware success is empty.
        // A violation means the tested behavior is incorrect.
        assert!(update(&mut c, &s, None, 60_001, false).is_empty());
        // Verify that returns the last successfully committed screen state exactly matches Off
        // state.
        assert_eq!(c.state(), State::Off);
        // Set whether radar presence counts as activity and can restore the backlight to the
        // enabled state.
        s.wake_on_presence = true;
        // Updates inactivity and wake requests from preferences, calibration, touch, and presence.
        c.observe(&s, None, 60_020, false, true, false);
        // Verify that writes required brightness and commits state only after hardware success
        // exactly matches the formatted text or fixture created by vec.
        assert_eq!(update(&mut c, &s, None, 60_020, false), vec![80]);
        // Updates inactivity and wake requests from preferences, calibration, touch, and presence.
        c.observe(&s, None, 120_020, false, true, false);
        // Verify writes required brightness and commits state only after hardware success is empty.
        // A violation means the tested behavior is incorrect.
        assert!(update(&mut c, &s, None, 120_020, false).is_empty());
        // Verify that writes required brightness and commits state only after hardware success
        // exactly matches the formatted text or fixture created by vec.
        assert_eq!(update(&mut c, &s, None, 180_020, false), vec![0]);
        // Updates inactivity and wake requests from preferences, calibration, touch, and presence.
        c.observe(&s, None, 180_040, true, false, false);
        // Verify that writes required brightness and commits state only after hardware success
        // exactly matches the formatted text or fixture created by vec.
        assert_eq!(update(&mut c, &s, None, 180_040, true), vec![128]);
        // Updates inactivity and wake requests from preferences, calibration, touch, and presence.
        c.observe(&s, None, 240_040, true, false, false);
        // Verify writes required brightness and commits state only after hardware success is empty.
        // A violation means the tested behavior is incorrect.
        assert!(update(&mut c, &s, None, 240_040, true).is_empty());
        // Set inactivity interval in minutes to 5.
        s.sleep_minutes = 5;
        // Updates inactivity and wake requests from preferences, calibration, touch, and presence.
        c.observe(&s, None, 240_060, false, false, false);
        // Verify that writes required brightness and commits state only after hardware success
        // exactly matches the formatted text or fixture created by vec.
        assert_eq!(update(&mut c, &s, None, 240_060, false), vec![80]);
        // Verify writes required brightness and commits state only after hardware success is empty.
        // A violation means the tested behavior is incorrect.
        assert!(update(&mut c, &s, None, 540_059, false).is_empty());
        // Verify that writes required brightness and commits state only after hardware success
        // exactly matches the formatted text or fixture created by vec.
        assert_eq!(update(&mut c, &s, None, 540_060, false), vec![0]);
    }
    /// Verifies nighttime activity overrides and that morning alone does not wake a previously
    /// blanked screen.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `()` - No value; completes when the expected test assertions hold.
    ///
    /// # Panics
    ///
    /// Assertions or fixture assumptions panic if the tested behavior is violated.
    #[test]
    fn night_override_and_morning_stays_dark_until_activity() {
        // Keep preference or service fixture configured for this test scenario in this local
        // variable for the following operations.
        let mut s = settings(0);
        // Set whether the local 22:00-06:00 backlight schedule is enabled to the enabled state.
        s.night_mode = true;
        // Keep credential or controller fixture used by the surrounding test in this local variable
        // for the following operations.
        let mut c = ScreenController::new(&s, 0);
        // Updates inactivity and wake requests from preferences, calibration, touch, and presence.
        c.observe(&s, Some(epoch(23)), 0, false, false, false);
        // Verify that writes required brightness and commits state only after hardware success
        // exactly matches the formatted text or fixture created by vec.
        assert_eq!(update(&mut c, &s, Some(epoch(23)), 0, false), vec![0]);
        // Updates inactivity and wake requests from preferences, calibration, touch, and presence.
        c.observe(&s, Some(epoch(6)), 7 * 3600_000, false, false, false);
        // Verify writes required brightness and commits state only after hardware success is empty.
        // A violation means the tested behavior is incorrect.
        assert!(update(&mut c, &s, Some(epoch(6)), 7 * 3600_000, false).is_empty());
        // Verify that returns the last successfully committed screen state exactly matches Off
        // state.
        assert_eq!(c.state(), State::Off);
        // Verify records touch activity and consumes the entire gesture that wakes a dark screen. A
        // violation means the tested behavior is incorrect.
        assert!(c.touch(&s, Some(epoch(6)), 7 * 3600_000 + 20, false));
        // Verify that writes required brightness and commits state only after hardware success
        // exactly matches the formatted text or fixture created by vec.
        assert_eq!(
            update(&mut c, &s, Some(epoch(6)), 7 * 3600_000 + 20, false),
            vec![80]
        );
        // Records touch activity and consumes the entire gesture that wakes a dark screen.
        c.touch(&s, Some(epoch(6)), 7 * 3600_000 + 40, true);
        // Updates inactivity and wake requests from preferences, calibration, touch, and presence.
        c.observe(&s, Some(epoch(23)), 24 * 3600_000, false, false, false);
        // Verify that writes required brightness and commits state only after hardware success
        // exactly matches the formatted text or fixture created by vec.
        assert_eq!(
            update(&mut c, &s, Some(epoch(23)), 24 * 3600_000, false),
            vec![0]
        );
        // Records touch activity and consumes the entire gesture that wakes a dark screen.
        c.touch(&s, Some(epoch(23)), 24 * 3600_000 + 20, false);
        // Writes required brightness and commits state only after hardware success.
        update(&mut c, &s, Some(epoch(23)), 24 * 3600_000 + 20, false);
        // Verify writes required brightness and commits state only after hardware success is empty.
        // A violation means the tested behavior is incorrect.
        assert!(update(&mut c, &s, Some(epoch(23)), 48 * 3600_000, false).is_empty());
        // Keep credential or controller fixture used by the surrounding test in this local variable
        // for the following operations.
        let mut c = ScreenController::new(&s, 0);
        // Records touch activity and consumes the entire gesture that wakes a dark screen.
        c.touch(&s, None, 1, false);
        // Records touch activity and consumes the entire gesture that wakes a dark screen.
        c.touch(&s, None, 2, true);
        // Updates inactivity and wake requests from preferences, calibration, touch, and presence.
        c.observe(&s, Some(epoch(23)), 3, false, false, false);
        // Verify that writes required brightness and commits state only after hardware success
        // exactly matches the formatted text or fixture created by vec.
        assert_eq!(update(&mut c, &s, Some(epoch(23)), 3, false), vec![80]);
    }
    /// Verifies that failed brightness writes preserve committed state and allow retries after
    /// the required backoff.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `()` - No value; completes when the expected test assertions hold.
    ///
    /// # Panics
    ///
    /// Assertions or fixture assumptions panic if the tested behavior is violated.
    #[test]
    fn hardware_failures_preserve_state_and_remain_retryable() {
        // Keep preference or service fixture configured for this test scenario in this local
        // variable for the following operations.
        let s = settings(1);
        // Keep credential or controller fixture used by the surrounding test in this local variable
        // for the following operations.
        let mut c = ScreenController::new(&s, 0);
        // Writes required brightness and commits state only after hardware success.
        update(&mut c, &s, None, 0, false);
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(c.update(&s, None, 60_000, false, |_| Err(())).is_err());
        // Verify that returns the last successfully committed screen state exactly matches On
        // state.
        assert_eq!(c.state(), State::On);
        // Verify writes required brightness and commits state only after hardware success is empty.
        // A violation means the tested behavior is incorrect.
        assert!(update(&mut c, &s, None, 119_999, false).is_empty());
        // Verify that writes required brightness and commits state only after hardware success
        // exactly matches the formatted text or fixture created by vec.
        assert_eq!(update(&mut c, &s, None, 120_000, false), vec![0]);
        // Records touch activity and consumes the entire gesture that wakes a dark screen.
        c.touch(&s, None, 120_020, false);
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(c.update(&s, None, 120_020, false, |_| Err(())).is_err());
        // Verify that returns the last successfully committed screen state exactly matches Off
        // state.
        assert_eq!(c.state(), State::Off);
        // Records touch activity and consumes the entire gesture that wakes a dark screen.
        c.touch(&s, None, 120_060, true);
        // Verify writes required brightness and commits state only after hardware success is empty.
        // A violation means the tested behavior is incorrect.
        assert!(update(&mut c, &s, None, 120_060, false).is_empty());
        // Records touch activity and consumes the entire gesture that wakes a dark screen.
        c.touch(&s, None, 121_020, false);
        // Verify that writes required brightness and commits state only after hardware success
        // exactly matches the formatted text or fixture created by vec.
        assert_eq!(update(&mut c, &s, None, 121_020, false), vec![80]);
        // Verify records touch activity and consumes the entire gesture that wakes a dark screen. A
        // violation means the tested behavior is incorrect.
        assert!(c.touch(&s, None, 121_040, true));
        // Verify the inverse of records touch activity and consumes the entire gesture that wakes a
        // dark screen. A violation means the tested behavior is incorrect.
        assert!(!c.touch(&s, None, 121_060, false));
    }
}
