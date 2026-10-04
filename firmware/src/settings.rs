// ============================================================================= //
// File          : settings.rs                                                   //
// License       : MIT                                                           //
// Created       : 2026-10-04                                                    //
// Last modified : 2026-10-04                                                    //
// Author        : Daniel Kharlamov                                              //
// Last editor   : Daniel Kharlamov <daniel.kharlamov@googlemail.com>            //
// ============================================================================= //
//                                                                               //
// Summary:                                                                      //
// --------                                                                      //
// Persistent preferences and settings overlay state.                            //
//                                                                               //
// Module:                                                                       //
// -------                                                                       //
// Defines and validates location and device preferences, serializes profiles,   //
// and imports legacy settings. Tracks settings overlays, save rollback,         //
// location search results, calibration state, and clock formatting. Handles     //
// stale requests and timeouts independently of Wi-Fi navigation.                //
// ============================================================================= //

//! Persisted preferences and settings overlays, independent of Wi-Fi navigation.
use crate::{i18n::Language, touch::Wizard};
use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Location {
    // Human-readable label for the selected object. Stored as String.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    // Optional city-and-country label that keeps the weather header concise. Stored as
    // Option<String>.
    pub compact_name: Option<String>,
    // Latitude in degrees north, bounded to -90 through 90. Stored as f64.
    pub latitude: f64,
    // Longitude in degrees east, bounded to -180 through 180. Stored as f64.
    pub longitude: f64,
    // IANA timezone identifier used for local clocks and forecast dates. Stored as String.
    pub timezone: String,
}
impl Default for Location {
    /// Creates the built-in London location.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `Self` - London coordinates, compact display name, and Europe/London timezone.
    fn default() -> Self {
        Self {
            // Initialize human-readable label for the selected object from the supplied value.
            name: "London".into(),
            // Initialize optional city-and-country label that keeps the weather header concise from
            // the supplied value.
            compact_name: Some("London, United Kingdom".into()),
            // Initialize latitude in degrees north, bounded to -90 through 90 from the supplied
            // value.
            latitude: 51.5074,
            // Initialize longitude in degrees east, bounded to -180 through 180 from the supplied
            // value.
            longitude: -0.1278,
            // Initialize IANA timezone identifier used for local clocks and forecast dates from the
            // supplied value.
            timezone: "Europe/London".into(),
        }
    }
}
impl Location {
    /// Chooses a compact location label for the weather header.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Location`) - Location names, coordinates, and timezone. Borrowed without
    ///   changing it.
    ///
    /// # Returns
    ///
    /// `String` - Owned compact name when present; otherwise city and final country component,
    /// or the original name.
    pub fn display_name(&self) -> String {
        // Prefer the explicit compact city/country label when the stored profile provides one.
        if let Some(name) = &self.compact_name {
            // Leave this function now with an owned copy of human-readable label for the selected
            // object; later statements are skipped.
            return name.clone();
        }
        // Keep text components separated before building a compact location label in this local
        // variable for the following operations.
        let parts: Vec<_> = self.name.split(',').map(str::trim).collect();
        // A full city/region/country label can omit intermediate regions in the small weather
        // header.
        if parts.len() >= 3 {
            format!("{}, {}", parts[0], parts[parts.len() - 1])
        } else {
            // Produce an owned copy of human-readable label for the selected object.
            self.name.clone()
        }
    }

    /// Validates names, geographic coordinates, and an optional IANA timezone.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Location`) - Location names, coordinates, and timezone. Borrowed without
    ///   changing it.
    ///
    /// # Returns
    ///
    /// `bool` - True for acceptable names of at most 256 bytes, finite in-range coordinates,
    /// and an empty or recognized timezone.
    pub fn valid(&self) -> bool {
        // Return whether this location/profile satisfies every supported name, coordinate,
        // timezone, version, and timeout constraint.
        !self.name.trim().is_empty()
            && self.name.len() <= 256
            && !self.name.chars().any(char::is_control)
            && self.compact_name.as_ref().is_none_or(|name| {
                // Return whether this location/profile satisfies every supported name, coordinate,
                // timezone, version, and timeout constraint.
                !name.trim().is_empty() && name.len() <= 256 && !name.chars().any(char::is_control)
            })
            && self.latitude.is_finite()
            && (-90.0..=90.0).contains(&self.latitude)
            && self.longitude.is_finite()
            && (-180.0..=180.0).contains(&self.longitude)
            && (self.timezone.is_empty() || self.timezone.parse::<chrono_tz::Tz>().is_ok())
    }
}
/// Supplies the inactivity default for older settings profiles.
///
/// # Arguments
///
/// None.
///
/// # Returns
///
/// `u32` - Five minutes.
fn sleep_minutes_default() -> u32 {
    5
}

/// Supplies the presence-wake default for older settings profiles.
///
/// # Arguments
///
/// None.
///
/// # Returns
///
/// `bool` - True, enabling radar presence wakeup.
fn presence_wake_default() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    // Persisted profile format version used to reject incompatible data. Stored as u8.
    pub version: u8,
    // Backlight duty in 0-255; zero turns illumination off. Stored as u8.
    pub brightness: u8,
    // Whether the local 22:00-06:00 backlight schedule is enabled. Stored as bool.
    pub night_mode: bool,
    #[serde(default = "sleep_minutes_default")]
    // Inactivity interval in minutes; zero disables ordinary inactivity blanking. Stored as u32.
    pub sleep_minutes: u32,
    #[serde(default = "presence_wake_default")]
    // Whether radar presence counts as activity and can restore the backlight. Stored as bool.
    pub wake_on_presence: bool,
    // Whether displayed temperatures are converted from cached Celsius to Fahrenheit. Stored as
    // bool.
    pub fahrenheit: bool,
    // Whether local time uses 24-hour rather than 12-hour formatting. Stored as bool.
    pub clock_24h: bool,
    // Selected language for interface text and location search. Stored as Language.
    pub language: Language,
    // Selected coordinates, location labels, and IANA timezone. Stored as Location.
    pub location: Location,
}
impl Default for Settings {
    /// Creates the version-1 factory preferences.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `Self` - Full brightness, five-minute inactivity timeout, enabled presence wakeup, and
    /// default language and location.
    fn default() -> Self {
        Self {
            // Initialize persisted profile format version used to reject incompatible data from the
            // supplied value.
            version: 1,
            // Initialize backlight duty in 0-255; zero turns illumination off from the supplied
            // value.
            brightness: 255,
            // Initialize whether the local 22:00-06:00 backlight schedule is enabled as
            // disabled/inactive.
            night_mode: false,
            // Initialize inactivity interval in minutes; zero disables ordinary inactivity blanking
            // from the supplied value.
            sleep_minutes: 5,
            // Initialize whether radar presence counts as activity and can restore the backlight as
            // enabled/active.
            wake_on_presence: true,
            // Initialize whether displayed temperatures are converted from cached Celsius to
            // Fahrenheit as disabled/inactive.
            fahrenheit: false,
            // Initialize whether local time uses 24-hour rather than 12-hour formatting as
            // disabled/inactive.
            clock_24h: false,
            // Initialize selected language for interface text and location search from the supplied
            // value.
            language: Language::English,
            // Initialize selected coordinates, location labels, and IANA timezone from the supplied
            // value.
            location: Location::default(),
        }
    }
}
#[derive(Default)]
pub struct LegacySettings {
    // Backlight duty in 0-255; zero turns illumination off. Stored as Option<u32>.
    pub brightness: Option<u32>,
    // Whether displayed temperatures are converted from cached Celsius to Fahrenheit. Stored as
    // Option<u8>.
    pub fahrenheit: Option<u8>,
    // Whether local time uses 24-hour rather than 12-hour formatting. Stored as Option<u8>.
    pub clock_24h: Option<u8>,
    // Whether the local 22:00-06:00 backlight schedule is enabled. Stored as Option<u8>.
    pub night_mode: Option<u8>,
    // Inactivity interval in minutes; zero disables ordinary inactivity blanking. Stored as
    // Option<u32>.
    pub sleep_minutes: Option<u32>,
    // Whether radar presence counts as activity and can restore the backlight. Stored as
    // Option<u8>.
    pub wake_on_presence: Option<u8>,
    // Selected language for interface text and location search. Stored as Option<u32>.
    pub language: Option<u32>,
    // Human-readable label for the selected object. Stored as Option<String>.
    pub name: Option<String>,
    // Latitude in degrees north, bounded to -90 through 90. Stored as Option<f64>.
    pub latitude: Option<f64>,
    // Longitude in degrees east, bounded to -180 through 180. Stored as Option<f64>.
    pub longitude: Option<f64>,
}
impl Settings {
    /// Imports compatible legacy values while replacing invalid values with defaults.
    ///
    /// # Arguments
    ///
    /// * `legacy` (`LegacySettings`) - Owned optional values imported from the older weather
    ///   settings namespace.
    ///
    /// # Returns
    ///
    /// `Self` - Validated default-based preferences; imported locations retain an empty
    /// timezone for later resolution.
    pub fn from_legacy(legacy: LegacySettings) -> Self {
        // Keep decoded or imported preference profile, validated before it is returned in this
        // local variable for the following operations.
        let mut value = Self {
            // Initialize backlight duty in 0-255; zero turns illumination off from the supplied
            // value.
            brightness: legacy
                .brightness
                .and_then(|v| u8::try_from(v).ok())
                .filter(|v| *v > 0)
                .unwrap_or(255),
            // Initialize whether displayed temperatures are converted from cached Celsius to
            // Fahrenheit from the supplied value.
            fahrenheit: legacy.fahrenheit == Some(1),
            // Initialize whether local time uses 24-hour rather than 12-hour formatting from the
            // supplied value.
            clock_24h: legacy.clock_24h == Some(1),
            // Initialize whether the local 22:00-06:00 backlight schedule is enabled from the
            // supplied value.
            night_mode: legacy.night_mode == Some(1),
            // Initialize inactivity interval in minutes; zero disables ordinary inactivity blanking
            // from the supplied value.
            sleep_minutes: legacy
                .sleep_minutes
                .filter(|value| crate::screen::TIMEOUTS.contains(value))
                .unwrap_or(5),
            // Initialize whether radar presence counts as activity and can restore the backlight
            // from the supplied value.
            wake_on_presence: legacy.wake_on_presence.is_none_or(|value| value == 1),
            // Unsupported legacy language index 4 falls back to English.
            // Choose the appropriate path for selected language for interface text and location
            // search; each arm handles one supported case.
            language: match legacy.language {
                // Handle the Some(4) case: apply the state-specific behavior shown here.
                Some(4) => Language::English,
                // Handle the value case: Execute unwrap or for from index result for the
                // surrounding operation.
                value => Language::from_index(value.unwrap_or(0)).unwrap_or(Language::English),
            },
            ..Self::default()
        };
        // Check whether an available the ordered tuple of related values matches the requested
        // case.
        if let (Some(name), Some(latitude), Some(longitude)) =
            (legacy.name, legacy.latitude, legacy.longitude)
        {
            // Keep selected coordinates, location labels, and IANA timezone in this local variable
            // for the following operations.
            let location = Location {
                // Initialize human-readable label for the selected object from the supplied value.
                name,
                // Leave optional city-and-country label that keeps the weather header concise
                // unavailable until a later operation supplies it.
                compact_name: None,
                // Initialize latitude in degrees north, bounded to -90 through 90 from the supplied
                // value.
                latitude,
                // Initialize longitude in degrees east, bounded to -180 through 180 from the
                // supplied value.
                longitude,
                // Initialize IANA timezone identifier used for local clocks and forecast dates from
                // the supplied value.
                timezone: String::new(),
            };
            // Check whether validates names, geographic coordinates, and an optional IANA timezone.
            if location.valid() {
                // Set selected coordinates, location labels, and IANA timezone to selected
                // coordinates, location labels, and IANA timezone.
                value.location = location;
            }
        }
        // Return decoded or imported preference profile, validated before it is returned as the
        // value of this block.
        value
    }

    /// Checks profile version, brightness, location, and inactivity timeout.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Settings`) - Device preference profile and selected location. Borrowed
    ///   without changing it.
    ///
    /// # Returns
    ///
    /// `bool` - True for version 1, nonzero brightness, a valid location, and a supported
    /// timeout.
    pub fn valid(&self) -> bool {
        // Return whether this location/profile satisfies every supported name, coordinate,
        // timezone, version, and timeout constraint.
        self.version == 1
            && self.brightness > 0
            && self.location.valid()
            && crate::screen::TIMEOUTS.contains(&self.sleep_minutes)
    }
    /// Serializes a validated profile to JSON.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Settings`) - Device preference profile and selected location. Borrowed
    ///   without changing it.
    ///
    /// # Returns
    ///
    /// `Result<Vec<u8>, &'static str>` - Ok with JSON bytes; Err with the storage failure
    /// message.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid preferences or JSON serialization failure.
    pub fn encode(&self) -> Result<Vec<u8>, &'static str> {
        // Reject invalid settings before serialization so unsupported profiles are never
        // intentionally written.
        if !self.valid() {
            // Leave this function now with a failure result for the surrounding operation; later
            // statements are skipped.
            return Err("Settings storage failed.");
        }
        // Execute map err for to vec result for the surrounding operation.
        serde_json::to_vec(self).map_err(|_| "Settings storage failed.")
    }
    /// Decodes and validates a JSON profile no larger than 2048 bytes.
    ///
    /// # Arguments
    ///
    /// * `bytes` (`&[u8]`) - UTF-8 JSON profile bytes; accepted input is at most 2048 bytes.
    ///
    /// # Returns
    ///
    /// `Result<Self, &'static str>` - Ok with supported preferences; Err with the settings read
    /// failure message.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, malformed, or unsupported profiles.
    pub fn decode(bytes: &[u8]) -> Result<Self, &'static str> {
        // Limit profile decoding to two KiB before allocating or parsing arbitrary saved data.
        if bytes.len() > 2048 {
            // Leave this function now with a failure result for the surrounding operation; later
            // statements are skipped.
            return Err("Settings could not be read.");
        }
        // Keep decoded or imported preference profile, validated before it is returned in this
        // local variable for the following operations.
        let value: Self =
            serde_json::from_slice(bytes).map_err(|_| "Settings could not be read.")?;
        // The decoded profile must still satisfy version, brightness, location, and timeout rules.
        if !value.valid() {
            // Leave this function now with a failure result for the surrounding operation; later
            // statements are skipped.
            return Err("Settings could not be read.");
        }
        // Return success; the caller receives decoded or imported preference profile, validated
        // before it is returned.
        Ok(value)
    }
    /// Converts synchronized UTC time to the saved location's local hour.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Settings`) - Device preference profile and selected location. Borrowed
    ///   without changing it.
    /// * `epoch` (`Option<i64>`) - UTC Unix timestamp in seconds; None means synchronized time
    ///   is unavailable.
    ///
    /// # Returns
    ///
    /// `Option<u32>` - Some hour in 0-23; None when time or timezone is missing or invalid.
    pub fn hour(&self, epoch: Option<i64>) -> Option<u32> {
        // Keep validated IANA timezone used to convert UTC timestamps into local time in this local
        // variable for the following operations.
        let zone = self.location.timezone.parse::<chrono_tz::Tz>().ok()?;
        // Return an available converts synchronized UTC time to the saved location's local hour.
        Some(
            DateTime::<Utc>::from_timestamp(epoch?, 0)?
                .with_timezone(&zone)
                .hour(),
        )
    }
    /// Formats the local minute clock using the selected 12- or 24-hour preference.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Settings`) - Device preference profile and selected location. Borrowed
    ///   without changing it.
    /// * `epoch` (`Option<i64>`) - UTC Unix timestamp in seconds; None means synchronized time
    ///   is unavailable.
    ///
    /// # Returns
    ///
    /// `Option<String>` - Some formatted clock string; None when time or timezone is
    /// unavailable or invalid.
    pub fn clock(&self, epoch: Option<i64>) -> Option<String> {
        // Keep validated IANA timezone used to convert UTC timestamps into local time in this local
        // variable for the following operations.
        let zone = self.location.timezone.parse::<chrono_tz::Tz>().ok()?;
        // Keep timestamp or formatted clock value used by the current view in this local variable
        // for the following operations.
        let time = DateTime::<Utc>::from_timestamp(epoch?, 0)?.with_timezone(&zone);
        // Return an available an owned text value rather than a borrowed view.
        Some(
            // Check whether whether local time uses 24-hour rather than 12-hour formatting.
            time.format(if self.clock_24h { "%H:%M" } else { "%I:%M %p" })
                .to_string(),
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    // Represent settings as a distinct selectable state or action; the matching handler determines
    // its effect.
    Settings,
    // Represent languages as a distinct selectable state or action; the matching handler determines
    // its effect.
    Languages,
    // Represent sleeptimeout as a distinct selectable state or action; the matching handler
    // determines its effect.
    SleepTimeout,
    // Represent locationinput as a distinct selectable state or action; the matching handler
    // determines its effect.
    LocationInput,
    // Represent locationresults as a distinct selectable state or action; the matching handler
    // determines its effect.
    LocationResults,
    // Represent resetwifi as a distinct selectable state or action; the matching handler determines
    // its effect.
    ResetWifi,
    // Represent calibrationintro as a distinct selectable state or action; the matching handler
    // determines its effect.
    CalibrationIntro,
    // Represent calibrationrunning as a distinct selectable state or action; the matching handler
    // determines its effect.
    CalibrationRunning,
    // Represent calibrationerror as a distinct selectable state or action; the matching handler
    // determines its effect.
    CalibrationError,
}

pub struct Preferences {
    // Last successful persistent preference profile. Stored as Settings.
    pub saved: Settings,
    // Editable preference profile; it can be rolled back to saved after a failed write. Stored as
    // Settings.
    pub current: Settings,
    // Active settings subpage, independent of the underlying Wi-Fi page. Stored as Option<Overlay>.
    pub overlay: Option<Overlay>,
    // Settings feedback text; an error can replace the otherwise visible local clock. Stored as
    // &'static str.
    pub status: &'static str,
    // Whether preference writes are disabled after a storage read failure. Stored as bool.
    pub read_only: bool,
    // City search text entered using the settings keyboard. Stored as String.
    pub query: String,
    // Selected on-screen keyboard layout. Stored as u8.
    pub keyboard: u8,
    // Whether the city keyboard uses the selected Russian/Ukrainian alphabet. Stored as bool.
    pub city_cyrillic: bool,
    // Accepted geocoding locations from the active request. Stored as Vec<Location>.
    pub results: Vec<Location>,
    // Optional selected result index; no location is saved until the user chooses one. Stored as
    // Option<usize>.
    pub result_selected: Option<usize>,
    // First visible location result in the paginated list. Stored as usize.
    pub result_offset: usize,
    // Search generation incremented on cancellation or replacement. Stored as u64.
    pub request_id: u64,
    // Whether the current location-results page is waiting for an API response. Stored as bool.
    pub searching: bool,
    // Monotonic millisecond limit for the current search. Stored as u64.
    pub search_deadline: u64,
    // Whether an in-progress brightness drag should continue receiving touch moves. Stored as bool.
    pub slider_active: bool,
    // Whether a confirmed Wi-Fi reset is waiting for its worker result. Stored as bool.
    pub reset_pending: bool,
    // UTC Unix timestamp in seconds; an optional value is unavailable before time synchronization.
    // Stored as Option<i64>.
    pub epoch: Option<i64>,
    // Interactive calibration state, including collected corners and a timeout. Stored as
    // Option<Wizard>.
    pub wizard: Option<Wizard>,
    // Settings subpage opened only after successful persistence. Stored as Option<Overlay>.
    pub after_save: Option<Overlay>,
}
impl Default for Preferences {
    /// Creates writable preference state from factory settings.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `Self` - Preferences with default saved and current values and no active overlay.
    fn default() -> Self {
        // Creates editable preference state from a loaded profile.
        Self::new(Settings::default(), false)
    }
}
impl Preferences {
    /// Refresh the time while repainting only changes visible in the minute clock.
    ///
    /// Stores synchronized time and detects visible settings-clock changes.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Preferences`) - Current and saved settings, overlays, and pending
    ///   location-search state. Mutated in place.
    /// * `epoch` (`Option<i64>`) - UTC Unix timestamp in seconds; None means synchronized time
    ///   is unavailable.
    ///
    /// # Returns
    ///
    /// `bool` - True if the unobscured settings clock needs repainting; false otherwise.
    pub fn update_time(&mut self, epoch: Option<i64>) -> bool {
        // Keep previous formatted minute clock used to suppress unnecessary repaints in this local
        // variable for the following operations.
        let previous = self.current.clock(self.epoch);
        // Set UTC Unix timestamp in seconds to UTC Unix timestamp in seconds; an optional value is
        // unavailable before time synchronization.
        self.epoch = epoch;
        // Return active settings subpage, independent of the underlying Wi-Fi page equals an
        // available value for `Overlay::Settings` and settings feedback text is empty and previous
        // formatted minute clock used to suppress unnecessary repaints differs from formats the
        // local minute clock using the selected 12- or 24-hour preference as the value of this
        // block.
        self.overlay == Some(Overlay::Settings)
            && self.status.is_empty()
            && previous != self.current.clock(self.epoch)
    }

    /// Creates editable preference state from a loaded profile.
    ///
    /// # Arguments
    ///
    /// * `saved` (`Settings`) - Previously loaded or factory preference profile used as the
    ///   rollback baseline.
    /// * `read_only` (`bool`) - Whether persistence is disabled because stored settings could
    ///   not be read.
    ///
    /// # Returns
    ///
    /// `Self` - Preferences containing saved and current copies, with an error status when
    /// writes are disabled.
    pub fn new(saved: Settings, read_only: bool) -> Self {
        Self {
            // Initialize editable preference profile; it can be rolled back to saved after a failed
            // write from the supplied value.
            current: saved.clone(),
            // Initialize last successful persistent preference profile from the supplied value.
            saved,
            // Leave active settings subpage, independent of the underlying Wi-Fi page unavailable
            // until a later operation supplies it.
            overlay: None,
            // Check whether whether preference writes are disabled after a storage read failure.
            status: if read_only {
                "Settings could not be read."
            } else {
                ""
            },
            // Initialize whether preference writes are disabled after a storage read failure from
            // the supplied value.
            read_only,
            // Initialize city search text entered using the settings keyboard from the supplied
            // value.
            query: String::new(),
            // Start selected on-screen keyboard layout at zero; later operations update it as
            // needed.
            keyboard: 0,
            // Initialize whether the city keyboard uses the selected Russian/Ukrainian alphabet as
            // disabled/inactive.
            city_cyrillic: false,
            // Initialize accepted geocoding locations from the active request from the supplied
            // value.
            results: Vec::new(),
            // Leave optional selected result index; no location is saved until the user chooses one
            // unavailable until a later operation supplies it.
            result_selected: None,
            // Start first visible location result in the paginated list at zero; later operations
            // update it as needed.
            result_offset: 0,
            // Start search generation incremented on cancellation or replacement at zero; later
            // operations update it as needed.
            request_id: 0,
            // Initialize whether the current location-results page is waiting for an API response
            // as disabled/inactive.
            searching: false,
            // Start monotonic millisecond limit for the current search at zero; later operations
            // update it as needed.
            search_deadline: 0,
            // Initialize whether an in-progress brightness drag should continue receiving touch
            // moves as disabled/inactive.
            slider_active: false,
            // Initialize whether a confirmed Wi-Fi reset is waiting for its worker result as
            // disabled/inactive.
            reset_pending: false,
            // Leave UTC Unix timestamp in seconds; an optional value is unavailable before time
            // synchronization unavailable until a later operation supplies it.
            epoch: None,
            // Leave interactive calibration state, including collected corners and a timeout
            // unavailable until a later operation supplies it.
            wizard: None,
            // Leave settings subpage opened only after successful persistence unavailable until a
            // later operation supplies it.
            after_save: None,
        }
    }
    /// Changes overlays and invalidates outstanding location searches.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Preferences`) - Current and saved settings, overlays, and pending
    ///   location-search state. Mutated in place.
    /// * `overlay` (`Option<Overlay>`) - Next settings overlay, or None to return to the
    ///   underlying application page.
    ///
    /// # Returns
    ///
    /// `()` - No value; increments the request identifier, stops searching, and resets the
    /// status according to read-only state.
    pub fn leave(&mut self, overlay: Option<Overlay>) {
        // Update search generation incremented on cancellation or replacement using 1, retaining
        // the accumulated state for subsequent steps.
        self.request_id += 1;
        // Set whether the current location-results page is waiting for an API response to the
        // disabled state.
        self.searching = false;
        // Set active settings subpage, independent of the underlying Wi-Fi page to active settings
        // subpage, independent of the underlying Wi-Fi page.
        self.overlay = overlay;
        // Set settings feedback text to the value selected by the following condition and its
        // alternatives.
        self.status = if self.read_only {
            "Settings could not be read."
        } else {
            ""
        };
    }
    /// Accepts successful persistence or rolls current preferences back after failure.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Preferences`) - Current and saved settings, overlays, and pending
    ///   location-search state. Mutated in place.
    /// * `success` (`bool`) - Whether the external persistence operation completed
    ///   successfully.
    ///
    /// # Returns
    ///
    /// `()` - No value; updates the saved baseline and next overlay on success, or restores the
    /// baseline and displays an error.
    pub fn saved(&mut self, success: bool) {
        // Persistence succeeded; make current preferences the new rollback baseline and apply any
        // queued overlay transition.
        if success {
            // Set last successful persistent preference profile to an owned copy of editable
            // preference profile.
            self.saved = self.current.clone();
            // Set settings feedback text to the specified message, format, or data literal.
            self.status = "";
            // Navigate only after a successful write, consuming the queued transition so it cannot
            // run twice.
            if let Some(next) = self.after_save.take() {
                // Changes overlays and invalidates outstanding location searches.
                self.leave(Some(next));
            }
        } else {
            // Set editable preference profile to an owned copy of last successful persistent
            // preference profile.
            self.current = self.saved.clone();
            // Set settings feedback text to the specified message, format, or data literal.
            self.status = "Settings storage failed.";
            // Set settings subpage opened only after successful persistence to no available value.
            self.after_save = None;
        }
    }
    /// Expires a location search whose deadline has passed.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Preferences`) - Current and saved settings, overlays, and pending
    ///   location-search state. Mutated in place.
    /// * `now` (`u64`) - Monotonic timestamp in milliseconds, using the same origin as stored
    ///   deadlines.
    ///
    /// # Returns
    ///
    /// `bool` - True when a timeout changes the search state and message; false otherwise.
    pub fn tick(&mut self, now: u64) -> bool {
        // The active location search exceeded ten seconds; invalidate its generation and show retry
        // feedback.
        if self.searching && now >= self.search_deadline {
            // Update search generation incremented on cancellation or replacement using 1,
            // retaining the accumulated state for subsequent steps.
            self.request_id += 1;
            // Set whether the current location-results page is waiting for an API response to the
            // disabled state.
            self.searching = false;
            // Set settings feedback text to the specified message, format, or data literal.
            self.status = "Location search failed. Please retry.";
            // Leave this function now with the enabled state; later statements are skipped.
            return true;
        }
        false
    }
    /// Applies a matching location-search response to the active results overlay.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Preferences`) - Current and saved settings, overlays, and pending
    ///   location-search state. Mutated in place.
    /// * `id` (`u64`) - Request generation identifier used to match responses with pending
    ///   work.
    /// * `result` (`Result<Vec<Location>, &'static str>`) - Worker response containing
    ///   validated data or a static failure message.
    ///
    /// # Returns
    ///
    /// `()` - No value; ignores stale responses and updates results, selection, pagination, and
    /// status for the active search.
    pub fn locations(&mut self, id: u64, result: Result<Vec<Location>, &'static str>) {
        // Ignore cancelled, obsolete, or hidden search results so they cannot overwrite the current
        // preference workflow.
        if id != self.request_id
            || !self.searching
            || self.overlay != Some(Overlay::LocationResults)
        {
            // Leave this function now after completing the required side effects; later statements
            // are skipped.
            return;
        }
        // Set whether the current location-results page is waiting for an API response to the
        // disabled state.
        self.searching = false;
        // Choose the appropriate path for success or failure produced by the operation, retained
        // for later handling; each arm handles one supported case.
        match result {
            // Continue with the successful result, using its validated value in this case.
            Ok(locations) => {
                // Set accepted geocoding locations from the active request to validated search
                // candidates retaining full selection labels and compact weather labels.
                self.results = locations;
                // Set optional selected result index to no available value.
                self.result_selected = None;
                // Set first visible location result in the paginated list to 0.
                self.result_offset = 0;
                // Set settings feedback text to the value selected by the following condition and
                // its alternatives.
                self.status = if self.results.is_empty() {
                    "No locations found."
                } else {
                    "Select a location."
                };
            }
            // Set settings feedback text to user-facing status or error text.
            Err(message) => self.status = message,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////////////
//                                    UNIT TESTS                                     //
///////////////////////////////////////////////////////////////////////////////////////

/// Unit tests and supporting fixtures for settings behavior.
#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies Ukrainian preference serialization and English fallback for the removed legacy
    /// Turkish slot.
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
    fn ukrainian_round_trip_and_removed_turkish_falls_back_to_english() {
        // Keep device preferences controlling location, language, units, brightness, and clock
        // format in this local variable for the following operations.
        let settings = Settings {
            // Initialize selected language for interface text and location search from the supplied
            // value.
            language: Language::Ukrainian,
            // Initialize backlight duty in 0-255; zero turns illumination off from the supplied
            // value.
            brightness: 127,
            // Initialize whether displayed temperatures are converted from cached Celsius to
            // Fahrenheit as enabled/active.
            fahrenheit: true,
            // Initialize whether local time uses 24-hour rather than 12-hour formatting as
            // enabled/active.
            clock_24h: true,
            ..Settings::default()
        };
        // Keep serialized byte record used to verify persistence compatibility in this local
        // variable for the following operations.
        let encoded = settings.encode().unwrap();
        // Verify that the required fixture or invariant value, panicking if unavailable exactly
        // matches device preferences controlling location, language, units, brightness, and clock
        // format.
        assert_eq!(Settings::decode(&encoded).unwrap(), settings);
        // Keep earlier profile or snapshot retained to verify backward compatibility in this local
        // variable for the following operations.
        let mut old = serde_json::to_value(&settings).unwrap();
        // Verify that the selected entry from earlier profile or snapshot retained to verify
        // backward compatibility exactly matches the specified message, format, or data literal.
        assert_eq!(old["language"], "Ukrainian");
        // Set the selected entry from earlier profile or snapshot retained to verify backward
        // compatibility to into result for the surrounding operation.
        old["language"] = "Turkish".into();
        // Keep profile after importing supported legacy values and applying compatibility defaults
        // in this local variable for the following operations.
        let migrated = Settings::decode(&serde_json::to_vec(&old).unwrap()).unwrap();
        // Keep reference framebuffer produced by rendering the complete page in this local variable
        // for the following operations.
        let expected = Settings {
            // Initialize selected language for interface text and location search from the supplied
            // value.
            language: Language::English,
            ..settings
        };
        // Verify that profile after importing supported legacy values and applying compatibility
        // defaults exactly matches reference framebuffer produced by rendering the complete page.
        assert_eq!(migrated, expected);
        // Verify that the selected entry from the required fixture or invariant value, panicking if
        // unavailable exactly matches the specified message, format, or data literal.
        assert_eq!(
            serde_json::to_value(migrated).unwrap()["language"],
            "English"
        );
        // Visit each entry in 0..8; the loop binding provides its value or index for this
        // iteration.
        for index in 0..8 {
            // Keep older optional preference fields read without modifying their original storage
            // in this local variable for the following operations.
            let legacy = Settings::from_legacy(LegacySettings {
                // Initialize selected language for interface text and location search from the
                // supplied value.
                language: Some(index),
                // Initialize backlight duty in 0-255; zero turns illumination off from the supplied
                // value.
                brightness: Some(127),
                // Initialize whether local time uses 24-hour rather than 12-hour formatting from
                // the supplied value.
                clock_24h: Some(1),
                ..Default::default()
            });
            // Verify that selected language for interface text and location search exactly matches
            // the value selected by the following condition and its alternatives.
            assert_eq!(
                legacy.language,
                if index == 4 {
                    Language::English
                } else {
                    Language::from_index(index).unwrap()
                }
            );
            // Verify that backlight duty in 0-255 exactly matches 127.
            assert_eq!(legacy.brightness, 127);
            // Verify whether local time uses 24-hour rather than 12-hour formatting. A violation
            // means the tested behavior is incorrect.
            assert!(legacy.clock_24h);
        }
        // Set the selected entry from earlier profile or snapshot retained to verify backward
        // compatibility to into result for the surrounding operation.
        old["language"] = "Unknown".into();
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(Settings::decode(&serde_json::to_vec(&old).unwrap()).is_err());
    }
    /// Verifies compact location name persistence and compatibility with profiles lacking the
    /// optional field.
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
    fn compact_location_names_preserve_old_profiles_and_round_trip() {
        // Keep selected coordinates, location labels, and IANA timezone in this local variable for
        // the following operations.
        let mut location = Location {
            // Leave optional city-and-country label that keeps the weather header concise
            // unavailable until a later operation supplies it.
            compact_name: None,
            ..Location::default()
        };
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for (name, expected) in [
            ("München, Bayern, Deutschland", "München, Deutschland"),
            ("Paris, France", "Paris, France"),
            ("London", "London"),
            ("City, District, Region, Country", "City, Country"),
        ] {
            // Set human-readable label for the selected object to into result for the surrounding
            // operation.
            location.name = name.into();
            // Verify that chooses a compact location label for the weather header exactly matches
            // reference framebuffer produced by rendering the complete page.
            assert_eq!(location.display_name(), expected);
        }
        // Set optional city-and-country label that keeps the weather header concise to an available
        // value for into result for the surrounding operation.
        location.compact_name = Some("Ort, Land".into());
        // Verify that chooses a compact location label for the weather header exactly matches the
        // specified message, format, or data literal.
        assert_eq!(location.display_name(), "Ort, Land");
        // Keep device preferences controlling location, language, units, brightness, and clock
        // format in this local variable for the following operations.
        let settings = Settings {
            // Initialize selected coordinates, location labels, and IANA timezone from the supplied
            // value.
            location,
            ..Settings::default()
        };
        // Keep serialized byte record used to verify persistence compatibility in this local
        // variable for the following operations.
        let encoded = settings.encode().unwrap();
        // Keep validated value reconstructed from its serialized record in this local variable for
        // the following operations.
        let decoded = Settings::decode(&encoded).unwrap();
        // Verify that as deref result for the surrounding operation exactly matches an available
        // value for the specified message, format, or data literal.
        assert_eq!(decoded.location.compact_name.as_deref(), Some("Ort, Land"));
        // Keep earlier profile or snapshot retained to verify backward compatibility in this local
        // variable for the following operations.
        let mut old: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        // Execute remove for the required fixture or invariant value, panicking if unavailable.
        old["location"]
            .as_object_mut()
            .unwrap()
            .remove("compact_name");
        // Keep validated value reconstructed from its serialized record in this local variable for
        // the following operations.
        let decoded = Settings::decode(&serde_json::to_vec(&old).unwrap()).unwrap();
        // Verify optional city-and-country label that keeps the weather header concise is
        // unavailable. A violation means the tested behavior is incorrect.
        assert!(decoded.location.compact_name.is_none());
        // Verify that chooses a compact location label for the weather header exactly matches the
        // specified message, format, or data literal.
        assert_eq!(decoded.location.display_name(), "City, Country");
        // Verify that persisted profile format version used to reject incompatible data exactly
        // matches 1.
        assert_eq!(decoded.version, 1);
        // Keep deliberately unsupported data or the associated failure message in this local
        // variable for the following operations.
        let mut invalid = decoded.location;
        // Set optional city-and-country label that keeps the weather header concise to an available
        // value for into result for the surrounding operation.
        invalid.compact_name = Some("\n".into());
        // Verify the inverse of validates names, geographic coordinates, and an optional IANA
        // timezone. A violation means the tested behavior is incorrect.
        assert!(!invalid.valid());
    }

    /// Verifies supported inactivity values, defaults for older profiles, invalid-value
    /// rejection, and legacy timeout imports.
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
    fn sleep_timeout_compatibility_validation_and_legacy_import() {
        // Keep earlier profile or snapshot retained to verify backward compatibility in this local
        // variable for the following operations.
        let mut old = serde_json::to_value(Settings::default()).unwrap();
        // Execute remove for the required fixture or invariant value, panicking if unavailable.
        old.as_object_mut().unwrap().remove("sleep_minutes");
        // Verify that inactivity interval in minutes exactly matches 5.
        assert_eq!(
            Settings::decode(&serde_json::to_vec(&old).unwrap())
                .unwrap()
                .sleep_minutes,
            5
        );
        // Visit each entry in crate screen TIMEOUTS; the loop binding provides its value or index
        // for this iteration.
        for minutes in crate::screen::TIMEOUTS {
            // Keep preference or service fixture configured for this test scenario in this local
            // variable for the following operations.
            let s = Settings {
                // Initialize inactivity interval in minutes; zero disables ordinary inactivity
                // blanking from the supplied value.
                sleep_minutes: minutes,
                ..Settings::default()
            };
            // Verify that inactivity interval in minutes exactly matches minutes.
            assert_eq!(
                Settings::decode(&s.encode().unwrap())
                    .unwrap()
                    .sleep_minutes,
                minutes
            );
            // Verify that inactivity interval in minutes exactly matches minutes.
            assert_eq!(
                Settings::from_legacy(LegacySettings {
                    sleep_minutes: Some(minutes),
                    ..Default::default()
                })
                .sleep_minutes,
                minutes
            );
        }
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(Settings {
            sleep_minutes: 2,
            ..Settings::default()
        }
        .encode()
        .is_err());
        // Verify that inactivity interval in minutes exactly matches 5.
        assert_eq!(
            Settings::from_legacy(LegacySettings {
                sleep_minutes: Some(999),
                ..Default::default()
            })
            .sleep_minutes,
            5
        );
    }
    /// Verifies enabled wake defaults for older profiles and persistence of both presence-wake
    /// settings.
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
    fn presence_wake_setting_preserves_old_profiles_and_roundtrips() {
        // Keep device preferences controlling location, language, units, brightness, and clock
        // format in this local variable for the following operations.
        let settings = Settings {
            // Initialize whether radar presence counts as activity and can restore the backlight as
            // disabled/inactive.
            wake_on_presence: false,
            ..Settings::default()
        };
        // Verify the inverse of whether radar presence counts as activity and can restore the
        // backlight. A violation means the tested behavior is incorrect.
        assert!(
            !Settings::decode(&settings.encode().unwrap())
                .unwrap()
                .wake_on_presence
        );
        // Keep earlier profile or snapshot retained to verify backward compatibility in this local
        // variable for the following operations.
        let mut old = serde_json::to_value(&settings).unwrap();
        // Execute remove for the required fixture or invariant value, panicking if unavailable.
        old.as_object_mut().unwrap().remove("wake_on_presence");
        // Keep preference profile built from compatible legacy values in this local variable for
        // the following operations.
        let imported = Settings::decode(&serde_json::to_vec(&old).unwrap()).unwrap();
        // Verify whether radar presence counts as activity and can restore the backlight. A
        // violation means the tested behavior is incorrect.
        assert!(imported.wake_on_presence);
        // Verify that selected coordinates, location labels, and IANA timezone exactly matches
        // selected coordinates, location labels, and IANA timezone.
        assert_eq!(imported.location, settings.location);
        // Verify whether radar presence counts as activity and can restore the backlight. A
        // violation means the tested behavior is incorrect.
        assert!(Settings::from_legacy(LegacySettings::default()).wake_on_presence);
    }

    /// Verifies that the settings clock requests repainting only when its visible formatted
    /// text changes.
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
    fn clock_refreshes_only_when_visible_text_changes() {
        // Keep editable preferences and their saved rollback baseline in this local variable for
        // the following operations.
        let mut preferences = Preferences {
            // Initialize active settings subpage, independent of the underlying Wi-Fi page from the
            // supplied value.
            overlay: Some(Overlay::Settings),
            ..Preferences::default()
        };
        // Verify the inverse of stores synchronized time and detects visible settings-clock
        // changes. A violation means the tested behavior is incorrect.
        assert!(!preferences.update_time(None));
        // Verify stores synchronized time and detects visible settings-clock changes. A violation
        // means the tested behavior is incorrect.
        assert!(preferences.update_time(Some(epoch(12, 0))));
        // Visit each entry in 1..60; the loop binding provides its value or index for this
        // iteration.
        for second in 1..60 {
            // Verify the inverse of stores synchronized time and detects visible settings-clock
            // changes. A violation means the tested behavior is incorrect.
            assert!(!preferences.update_time(Some(epoch(12, 0) + second)));
        }
        // Verify stores synchronized time and detects visible settings-clock changes. A violation
        // means the tested behavior is incorrect.
        assert!(preferences.update_time(Some(epoch(12, 1))));
        // Set settings feedback text to the specified message, format, or data literal.
        preferences.status = "Calibration saved.";
        // Verify the inverse of stores synchronized time and detects visible settings-clock
        // changes. A violation means the tested behavior is incorrect.
        assert!(!preferences.update_time(Some(epoch(12, 2))));
        // Set settings feedback text to the specified message, format, or data literal.
        preferences.status = "";
        // Set active settings subpage, independent of the underlying Wi-Fi page to an available
        // value for Overlay Languages.
        preferences.overlay = Some(Overlay::Languages);
        // Verify the inverse of stores synchronized time and detects visible settings-clock
        // changes. A violation means the tested behavior is incorrect.
        assert!(!preferences.update_time(Some(epoch(12, 3))));
        // Verify that UTC Unix timestamp in seconds exactly matches an available value for creates
        // a fixed UTC timestamp for clock-format tests.
        assert_eq!(preferences.epoch, Some(epoch(12, 3)));
    }

    use chrono::TimeZone;
    /// Creates a fixed UTC timestamp for clock-format tests.
    ///
    /// # Arguments
    ///
    /// * `hour` (`u32`) - Hour of day in 0-23.
    /// * `minute` (`u32`) - Minute within the hour in 0-59.
    ///
    /// # Returns
    ///
    /// `i64` - Unix timestamp in seconds for the supplied test hour and minute.
    ///
    /// # Panics
    ///
    /// Panics if the fixed fixture, test timestamp, or simulated service cannot be constructed.
    fn epoch(hour: u32, minute: u32) -> i64 {
        // Execute timestamp for the required fixture or invariant value, panicking if unavailable.
        Utc.with_ymd_and_hms(2026, 10, 2, hour, minute, 0)
            .unwrap()
            .timestamp()
    }
    /// Verifies preference serialization and validation, successful save baselines, and
    /// rollback after persistence failure.
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
    fn settings_roundtrip_validation_and_save_rollback() {
        // Keep device preferences controlling location, language, units, brightness, and clock
        // format in this local variable for the following operations.
        let settings = Settings::default();
        // Verify that the required fixture or invariant value, panicking if unavailable exactly
        // matches device preferences controlling location, language, units, brightness, and clock
        // format.
        assert_eq!(
            Settings::decode(&settings.encode().unwrap()).unwrap(),
            settings
        );
        // Keep editable preferences and their saved rollback baseline in this local variable for
        // the following operations.
        let mut preferences = Preferences::new(settings.clone(), false);
        // Set selected language for interface text and location search to Language French.
        preferences.current.language = Language::French;
        // Set backlight duty in 0-255 to 10.
        preferences.current.brightness = 10;
        // Accepts successful persistence or rolls current preferences back after failure.
        preferences.saved(false);
        // Verify that editable preference profile exactly matches device preferences controlling
        // location, language, units, brightness, and clock format.
        assert_eq!(preferences.current, settings);
        // Verify that settings feedback text exactly matches the specified message, format, or data
        // literal.
        assert_eq!(preferences.status, "Settings storage failed.");
        // Set selected language for interface text and location search to Language Ukrainian.
        preferences.current.language = Language::Ukrainian;
        // Accepts successful persistence or rolls current preferences back after failure.
        preferences.saved(true);
        // Verify that selected language for interface text and location search exactly matches
        // Language Ukrainian.
        assert_eq!(preferences.saved.language, Language::Ukrainian);
        // Keep deliberately unsupported data or the associated failure message in this local
        // variable for the following operations.
        let mut invalid = settings;
        // Set persisted profile format version used to reject incompatible data to 2.
        invalid.version = 2;
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(invalid.encode().is_err());
        // Set persisted profile format version used to reject incompatible data to 1.
        invalid.version = 1;
        // Set latitude in degrees north, bounded to -90 through 90 to f64 NAN.
        invalid.location.latitude = f64::NAN;
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(invalid.encode().is_err());
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(Settings::decode(b"{invalid").is_err());
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(Settings::decode(&vec![0; 2049]).is_err());
    }

    /// Verifies local clock formats across daylight-saving changes and behavior with an
    /// unresolved legacy timezone.
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
    fn timezone_dst_formats_and_unknown_legacy_timezone() {
        // Keep device preferences controlling location, language, units, brightness, and clock
        // format in this local variable for the following operations.
        let mut settings = Settings::default();
        // Keep snapshot taken before the operation so later changes can be compared in this local
        // variable for the following operations.
        let before = Utc
            .with_ymd_and_hms(2026, 3, 29, 0, 59, 0)
            .unwrap()
            .timestamp();
        // Keep resulting state retained for comparison with the earlier baseline in this local
        // variable for the following operations.
        let after = before + 60;
        // Set whether local time uses 24-hour rather than 12-hour formatting to the enabled state.
        settings.clock_24h = true;
        // Verify that as deref result for the surrounding operation exactly matches an available
        // value for the specified message, format, or data literal.
        assert_eq!(settings.clock(Some(before)).as_deref(), Some("00:59"));
        // Verify that as deref result for the surrounding operation exactly matches an available
        // value for the specified message, format, or data literal.
        assert_eq!(settings.clock(Some(after)).as_deref(), Some("02:00"));
        // Set whether local time uses 24-hour rather than 12-hour formatting to the disabled state.
        settings.clock_24h = false;
        // Verify that as deref result for the surrounding operation exactly matches an available
        // value for the specified message, format, or data literal.
        assert_eq!(settings.clock(Some(after)).as_deref(), Some("02:00 AM"));
        // Remove accumulated entries or transient state before beginning the next operation.
        settings.location.timezone.clear();
        // Verify validates names, geographic coordinates, and an optional IANA timezone. A
        // violation means the tested behavior is incorrect.
        assert!(settings.valid());
        // Verify converts synchronized UTC time to the saved location's local hour is unavailable.
        // A violation means the tested behavior is incorrect.
        assert!(settings.hour(Some(after)).is_none());
    }
    /// Verifies that cancelled, stale, and expired searches cannot replace saved location
    /// preferences.
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
    fn canceled_stale_and_timed_out_location_results_do_not_change_settings() {
        // Keep short-lived borrow or raw point used by the surrounding preference/touch operation
        // in this local variable for the following operations.
        let mut p = Preferences::default();
        // Changes overlays and invalidates outstanding location searches.
        p.leave(Some(Overlay::LocationResults));
        // Set whether the current location-results page is waiting for an API response to the
        // enabled state.
        p.searching = true;
        // Set monotonic millisecond limit for the current search to 10_000.
        p.search_deadline = 10_000;
        // Keep request generation carried by a command or response in this local variable for the
        // following operations.
        let id = p.request_id;
        // Changes overlays and invalidates outstanding location searches.
        p.leave(Some(Overlay::Settings));
        // Applies a matching location-search response to the active results overlay.
        p.locations(id, Ok(vec![Location::default()]));
        // Verify accepted geocoding locations from the active request is empty. A violation means
        // the tested behavior is incorrect.
        assert!(p.results.is_empty());
        // Changes overlays and invalidates outstanding location searches.
        p.leave(Some(Overlay::LocationResults));
        // Set whether the current location-results page is waiting for an API response to the
        // enabled state.
        p.searching = true;
        // Set monotonic millisecond limit for the current search to 10_000.
        p.search_deadline = 10_000;
        // Keep request generation carried by a command or response in this local variable for the
        // following operations.
        let id = p.request_id;
        // Verify the inverse of expires a location search whose deadline has passed. A violation
        // means the tested behavior is incorrect.
        assert!(!p.tick(9_999));
        // Verify expires a location search whose deadline has passed. A violation means the tested
        // behavior is incorrect.
        assert!(p.tick(10_000));
        // Applies a matching location-search response to the active results overlay.
        p.locations(id, Ok(vec![Location::default()]));
        // Verify accepted geocoding locations from the active request is empty. A violation means
        // the tested behavior is incorrect.
        assert!(p.results.is_empty());
        // Verify that settings feedback text exactly matches the specified message, format, or data
        // literal.
        assert_eq!(p.status, "Location search failed. Please retry.");
    }
}

///////////////////////////////////////////////////////////////////////////////////////
//                                    UNIT TESTS                                     //
///////////////////////////////////////////////////////////////////////////////////////

/// Tests for read-only legacy preference imports and compatibility defaults.
#[cfg(test)]
mod legacy_tests {
    use super::*;
    /// Verifies compatible legacy preference import, unresolved imported timezones, and
    /// retention of the original legacy values.
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
    fn imports_legacy_keys_without_assuming_a_timezone_or_modifying_the_source() {
        // Keep device preferences controlling location, language, units, brightness, and clock
        // format in this local variable for the following operations.
        let settings = Settings::from_legacy(LegacySettings {
            // Initialize backlight duty in 0-255; zero turns illumination off from the supplied
            // value.
            brightness: Some(128),
            // Initialize whether displayed temperatures are converted from cached Celsius to
            // Fahrenheit from the supplied value.
            fahrenheit: Some(1),
            // Initialize whether local time uses 24-hour rather than 12-hour formatting from the
            // supplied value.
            clock_24h: Some(1),
            // Initialize whether the local 22:00-06:00 backlight schedule is enabled from the
            // supplied value.
            night_mode: Some(1),
            // Initialize inactivity interval in minutes; zero disables ordinary inactivity blanking
            // from the supplied value.
            sleep_minutes: Some(10),
            // Initialize whether radar presence counts as activity and can restore the backlight
            // from the supplied value.
            wake_on_presence: Some(0),
            // Initialize selected language for interface text and location search from the supplied
            // value.
            language: Some(2),
            // Initialize human-readable label for the selected object from the supplied value.
            name: Some("Berlin".into()),
            // Initialize latitude in degrees north, bounded to -90 through 90 from the supplied
            // value.
            latitude: Some(52.52),
            // Initialize longitude in degrees east, bounded to -180 through 180 from the supplied
            // value.
            longitude: Some(13.405),
        });
        // Verify validates names, geographic coordinates, and an optional IANA timezone. A
        // violation means the tested behavior is incorrect.
        assert!(settings.valid());
        // Verify that selected language for interface text and location search exactly matches
        // Language German.
        assert_eq!(settings.language, Language::German);
        // Verify that backlight duty in 0-255 exactly matches 128.
        assert_eq!(settings.brightness, 128);
        // Verify whether displayed temperatures are converted from cached Celsius to Fahrenheit and
        // whether local time uses 24-hour rather than 12-hour formatting and whether the local
        // 22:00-06:00 backlight schedule is enabled. A violation means the tested behavior is
        // incorrect.
        assert!(settings.fahrenheit && settings.clock_24h && settings.night_mode);
        // Verify the inverse of whether radar presence counts as activity and can restore the
        // backlight. A violation means the tested behavior is incorrect.
        assert!(!settings.wake_on_presence);
        // Verify that human-readable label for the selected object exactly matches the specified
        // message, format, or data literal.
        assert_eq!(settings.location.name, "Berlin");
        // Verify IANA timezone identifier used for local clocks and forecast dates is empty. A
        // violation means the tested behavior is incorrect.
        assert!(settings.location.timezone.is_empty());
        // Verify converts synchronized UTC time to the saved location's local hour is unavailable.
        // A violation means the tested behavior is incorrect.
        assert!(settings.hour(Some(1_700_000_000)).is_none());
        // Keep default value selected when an older record omits a newer preference in this local
        // variable for the following operations.
        let fallback = Settings::from_legacy(LegacySettings {
            // Initialize backlight duty in 0-255; zero turns illumination off from the supplied
            // value.
            brightness: Some(999),
            // Initialize selected language for interface text and location search from the supplied
            // value.
            language: Some(99),
            // Initialize human-readable label for the selected object from the supplied value.
            name: Some("Bad".into()),
            // Initialize latitude in degrees north, bounded to -90 through 90 from the supplied
            // value.
            latitude: Some(999.0),
            // Initialize longitude in degrees east, bounded to -180 through 180 from the supplied
            // value.
            longitude: Some(0.0),
            ..Default::default()
        });
        // Verify that default value selected when an older record omits a newer preference exactly
        // matches the type's documented default state.
        assert_eq!(fallback, Settings::default());
    }
}
