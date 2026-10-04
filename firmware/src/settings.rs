// ============================================================================= //
// File          : settings.rs                                                   //
// License       : GPL-3.0-only                                                  //
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
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact_name: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub timezone: String,
}
impl Default for Location {
    fn default() -> Self {
        Self {
            name: "London".into(),
            compact_name: Some("London, United Kingdom".into()),
            latitude: 51.5074,
            longitude: -0.1278,
            timezone: "Europe/London".into(),
        }
    }
}
impl Location {
    pub fn display_name(&self) -> String {
        if let Some(name) = &self.compact_name {
            return name.clone();
        }
        let parts: Vec<_> = self.name.split(',').map(str::trim).collect();
        if parts.len() >= 3 {
            format!("{}, {}", parts[0], parts[parts.len() - 1])
        } else {
            self.name.clone()
        }
    }

    pub fn valid(&self) -> bool {
        !self.name.trim().is_empty()
            && self.name.len() <= 256
            && !self.name.chars().any(char::is_control)
            && self.compact_name.as_ref().is_none_or(|name| {
                !name.trim().is_empty() && name.len() <= 256 && !name.chars().any(char::is_control)
            })
            && self.latitude.is_finite()
            && (-90.0..=90.0).contains(&self.latitude)
            && self.longitude.is_finite()
            && (-180.0..=180.0).contains(&self.longitude)
            && (self.timezone.is_empty() || self.timezone.parse::<chrono_tz::Tz>().is_ok())
    }
}
fn sleep_minutes_default() -> u32 {
    5
}

fn presence_wake_default() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub version: u8,
    pub brightness: u8,
    pub night_mode: bool,
    #[serde(default = "sleep_minutes_default")]
    pub sleep_minutes: u32,
    #[serde(default = "presence_wake_default")]
    pub wake_on_presence: bool,
    pub fahrenheit: bool,
    pub clock_24h: bool,
    pub language: Language,
    pub location: Location,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            brightness: 255,
            night_mode: false,
            sleep_minutes: 5,
            wake_on_presence: true,
            fahrenheit: false,
            clock_24h: false,
            language: Language::English,
            location: Location::default(),
        }
    }
}
#[derive(Default)]
pub struct LegacySettings {
    pub brightness: Option<u32>,
    pub fahrenheit: Option<u8>,
    pub clock_24h: Option<u8>,
    pub night_mode: Option<u8>,
    pub sleep_minutes: Option<u32>,
    pub wake_on_presence: Option<u8>,
    pub language: Option<u32>,
    pub name: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}
impl Settings {
    pub fn from_legacy(legacy: LegacySettings) -> Self {
        let mut value = Self {
            brightness: legacy
                .brightness
                .and_then(|v| u8::try_from(v).ok())
                .filter(|v| *v > 0)
                .unwrap_or(255),
            fahrenheit: legacy.fahrenheit == Some(1),
            clock_24h: legacy.clock_24h == Some(1),
            night_mode: legacy.night_mode == Some(1),
            sleep_minutes: legacy
                .sleep_minutes
                .filter(|value| crate::screen::TIMEOUTS.contains(value))
                .unwrap_or(5),
            wake_on_presence: legacy.wake_on_presence.is_none_or(|value| value == 1),
            // Unsupported legacy language index 4 falls back to English.
            language: match legacy.language {
                Some(4) => Language::English,
                value => Language::from_index(value.unwrap_or(0)).unwrap_or(Language::English),
            },
            ..Self::default()
        };
        if let (Some(name), Some(latitude), Some(longitude)) =
            (legacy.name, legacy.latitude, legacy.longitude)
        {
            let location = Location {
                name,
                compact_name: None,
                latitude,
                longitude,
                timezone: String::new(),
            };
            if location.valid() {
                value.location = location;
            }
        }
        value
    }

    pub fn valid(&self) -> bool {
        self.version == 1
            && self.brightness > 0
            && self.location.valid()
            && crate::screen::TIMEOUTS.contains(&self.sleep_minutes)
    }
    pub fn encode(&self) -> Result<Vec<u8>, &'static str> {
        if !self.valid() {
            return Err("Settings storage failed.");
        }
        serde_json::to_vec(self).map_err(|_| "Settings storage failed.")
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() > 2048 {
            return Err("Settings could not be read.");
        }
        let value: Self =
            serde_json::from_slice(bytes).map_err(|_| "Settings could not be read.")?;
        if !value.valid() {
            return Err("Settings could not be read.");
        }
        Ok(value)
    }
    pub fn hour(&self, epoch: Option<i64>) -> Option<u32> {
        let zone = self.location.timezone.parse::<chrono_tz::Tz>().ok()?;
        Some(
            DateTime::<Utc>::from_timestamp(epoch?, 0)?
                .with_timezone(&zone)
                .hour(),
        )
    }
    pub fn clock(&self, epoch: Option<i64>) -> Option<String> {
        let zone = self.location.timezone.parse::<chrono_tz::Tz>().ok()?;
        let time = DateTime::<Utc>::from_timestamp(epoch?, 0)?.with_timezone(&zone);
        Some(
            time.format(if self.clock_24h { "%H:%M" } else { "%I:%M %p" })
                .to_string(),
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    Settings,
    Languages,
    SleepTimeout,
    LocationInput,
    LocationResults,
    ResetWifi,
    CalibrationIntro,
    CalibrationRunning,
    CalibrationError,
}

pub struct Preferences {
    pub saved: Settings,
    pub current: Settings,
    pub overlay: Option<Overlay>,
    pub status: &'static str,
    pub read_only: bool,
    pub query: String,
    pub keyboard: u8,
    pub city_cyrillic: bool,
    pub results: Vec<Location>,
    pub result_selected: Option<usize>,
    pub result_offset: usize,
    pub request_id: u64,
    pub searching: bool,
    pub search_deadline: u64,
    pub slider_active: bool,
    pub reset_pending: bool,
    pub epoch: Option<i64>,
    pub wizard: Option<Wizard>,
    pub after_save: Option<Overlay>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self::new(Settings::default(), false)
    }
}
impl Preferences {
    /// Refresh the time while repainting only changes visible in the minute clock.
    pub fn update_time(&mut self, epoch: Option<i64>) -> bool {
        let previous = self.current.clock(self.epoch);
        self.epoch = epoch;
        self.overlay == Some(Overlay::Settings)
            && self.status.is_empty()
            && previous != self.current.clock(self.epoch)
    }

    pub fn new(saved: Settings, read_only: bool) -> Self {
        Self {
            current: saved.clone(),
            saved,
            overlay: None,
            status: if read_only {
                "Settings could not be read."
            } else {
                ""
            },
            read_only,
            query: String::new(),
            keyboard: 0,
            city_cyrillic: false,
            results: Vec::new(),
            result_selected: None,
            result_offset: 0,
            request_id: 0,
            searching: false,
            search_deadline: 0,
            slider_active: false,
            reset_pending: false,
            epoch: None,
            wizard: None,
            after_save: None,
        }
    }
    pub fn leave(&mut self, overlay: Option<Overlay>) {
        self.request_id += 1;
        self.searching = false;
        self.overlay = overlay;
        self.status = if self.read_only {
            "Settings could not be read."
        } else {
            ""
        };
    }
    pub fn saved(&mut self, success: bool) {
        if success {
            self.saved = self.current.clone();
            self.status = "";
            if let Some(next) = self.after_save.take() {
                self.leave(Some(next));
            }
        } else {
            self.current = self.saved.clone();
            self.status = "Settings storage failed.";
            self.after_save = None;
        }
    }
    pub fn tick(&mut self, now: u64) -> bool {
        if self.searching && now >= self.search_deadline {
            self.request_id += 1;
            self.searching = false;
            self.status = "Location search failed. Please retry.";
            return true;
        }
        false
    }
    pub fn locations(&mut self, id: u64, result: Result<Vec<Location>, &'static str>) {
        if id != self.request_id
            || !self.searching
            || self.overlay != Some(Overlay::LocationResults)
        {
            return;
        }
        self.searching = false;
        match result {
            Ok(locations) => {
                self.results = locations;
                self.result_selected = None;
                self.result_offset = 0;
                self.status = if self.results.is_empty() {
                    "No locations found."
                } else {
                    "Select a location."
                };
            }
            Err(message) => self.status = message,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ukrainian_round_trip_and_removed_turkish_falls_back_to_english() {
        let settings = Settings {
            language: Language::Ukrainian,
            brightness: 127,
            fahrenheit: true,
            clock_24h: true,
            ..Settings::default()
        };
        let encoded = settings.encode().unwrap();
        assert_eq!(Settings::decode(&encoded).unwrap(), settings);
        let mut old = serde_json::to_value(&settings).unwrap();
        assert_eq!(old["language"], "Ukrainian");
        old["language"] = "Turkish".into();
        let migrated = Settings::decode(&serde_json::to_vec(&old).unwrap()).unwrap();
        let expected = Settings {
            language: Language::English,
            ..settings
        };
        assert_eq!(migrated, expected);
        assert_eq!(
            serde_json::to_value(migrated).unwrap()["language"],
            "English"
        );
        for index in 0..8 {
            let legacy = Settings::from_legacy(LegacySettings {
                language: Some(index),
                brightness: Some(127),
                clock_24h: Some(1),
                ..Default::default()
            });
            assert_eq!(
                legacy.language,
                if index == 4 {
                    Language::English
                } else {
                    Language::from_index(index).unwrap()
                }
            );
            assert_eq!(legacy.brightness, 127);
            assert!(legacy.clock_24h);
        }
        old["language"] = "Unknown".into();
        assert!(Settings::decode(&serde_json::to_vec(&old).unwrap()).is_err());
    }
    #[test]
    fn compact_location_names_preserve_old_profiles_and_round_trip() {
        let mut location = Location {
            compact_name: None,
            ..Location::default()
        };
        for (name, expected) in [
            ("München, Bayern, Deutschland", "München, Deutschland"),
            ("Paris, France", "Paris, France"),
            ("London", "London"),
            ("City, District, Region, Country", "City, Country"),
        ] {
            location.name = name.into();
            assert_eq!(location.display_name(), expected);
        }
        location.compact_name = Some("Ort, Land".into());
        assert_eq!(location.display_name(), "Ort, Land");
        let settings = Settings {
            location,
            ..Settings::default()
        };
        let encoded = settings.encode().unwrap();
        let decoded = Settings::decode(&encoded).unwrap();
        assert_eq!(decoded.location.compact_name.as_deref(), Some("Ort, Land"));
        let mut old: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        old["location"]
            .as_object_mut()
            .unwrap()
            .remove("compact_name");
        let decoded = Settings::decode(&serde_json::to_vec(&old).unwrap()).unwrap();
        assert!(decoded.location.compact_name.is_none());
        assert_eq!(decoded.location.display_name(), "City, Country");
        assert_eq!(decoded.version, 1);
        let mut invalid = decoded.location;
        invalid.compact_name = Some("\n".into());
        assert!(!invalid.valid());
    }

    #[test]
    fn sleep_timeout_compatibility_validation_and_legacy_import() {
        let mut old = serde_json::to_value(Settings::default()).unwrap();
        old.as_object_mut().unwrap().remove("sleep_minutes");
        assert_eq!(
            Settings::decode(&serde_json::to_vec(&old).unwrap())
                .unwrap()
                .sleep_minutes,
            5
        );
        for minutes in crate::screen::TIMEOUTS {
            let s = Settings {
                sleep_minutes: minutes,
                ..Settings::default()
            };
            assert_eq!(
                Settings::decode(&s.encode().unwrap())
                    .unwrap()
                    .sleep_minutes,
                minutes
            );
            assert_eq!(
                Settings::from_legacy(LegacySettings {
                    sleep_minutes: Some(minutes),
                    ..Default::default()
                })
                .sleep_minutes,
                minutes
            );
        }
        assert!(Settings {
            sleep_minutes: 2,
            ..Settings::default()
        }
        .encode()
        .is_err());
        assert_eq!(
            Settings::from_legacy(LegacySettings {
                sleep_minutes: Some(999),
                ..Default::default()
            })
            .sleep_minutes,
            5
        );
    }
    #[test]
    fn presence_wake_setting_preserves_old_profiles_and_roundtrips() {
        let settings = Settings {
            wake_on_presence: false,
            ..Settings::default()
        };
        assert!(
            !Settings::decode(&settings.encode().unwrap())
                .unwrap()
                .wake_on_presence
        );
        let mut old = serde_json::to_value(&settings).unwrap();
        old.as_object_mut().unwrap().remove("wake_on_presence");
        let imported = Settings::decode(&serde_json::to_vec(&old).unwrap()).unwrap();
        assert!(imported.wake_on_presence);
        assert_eq!(imported.location, settings.location);
        assert!(Settings::from_legacy(LegacySettings::default()).wake_on_presence);
    }

    #[test]
    fn clock_refreshes_only_when_visible_text_changes() {
        let mut preferences = Preferences {
            overlay: Some(Overlay::Settings),
            ..Preferences::default()
        };
        assert!(!preferences.update_time(None));
        assert!(preferences.update_time(Some(epoch(12, 0))));
        for second in 1..60 {
            assert!(!preferences.update_time(Some(epoch(12, 0) + second)));
        }
        assert!(preferences.update_time(Some(epoch(12, 1))));
        preferences.status = "Calibration saved.";
        assert!(!preferences.update_time(Some(epoch(12, 2))));
        preferences.status = "";
        preferences.overlay = Some(Overlay::Languages);
        assert!(!preferences.update_time(Some(epoch(12, 3))));
        assert_eq!(preferences.epoch, Some(epoch(12, 3)));
    }

    use chrono::TimeZone;
    fn epoch(hour: u32, minute: u32) -> i64 {
        Utc.with_ymd_and_hms(2026, 10, 2, hour, minute, 0)
            .unwrap()
            .timestamp()
    }
    #[test]
    fn settings_roundtrip_validation_and_save_rollback() {
        let settings = Settings::default();
        assert_eq!(
            Settings::decode(&settings.encode().unwrap()).unwrap(),
            settings
        );
        let mut preferences = Preferences::new(settings.clone(), false);
        preferences.current.language = Language::French;
        preferences.current.brightness = 10;
        preferences.saved(false);
        assert_eq!(preferences.current, settings);
        assert_eq!(preferences.status, "Settings storage failed.");
        preferences.current.language = Language::Ukrainian;
        preferences.saved(true);
        assert_eq!(preferences.saved.language, Language::Ukrainian);
        let mut invalid = settings;
        invalid.version = 2;
        assert!(invalid.encode().is_err());
        invalid.version = 1;
        invalid.location.latitude = f64::NAN;
        assert!(invalid.encode().is_err());
        assert!(Settings::decode(b"{invalid").is_err());
        assert!(Settings::decode(&vec![0; 2049]).is_err());
    }

    #[test]
    fn timezone_dst_formats_and_unknown_legacy_timezone() {
        let mut settings = Settings::default();
        let before = Utc
            .with_ymd_and_hms(2026, 3, 29, 0, 59, 0)
            .unwrap()
            .timestamp();
        let after = before + 60;
        settings.clock_24h = true;
        assert_eq!(settings.clock(Some(before)).as_deref(), Some("00:59"));
        assert_eq!(settings.clock(Some(after)).as_deref(), Some("02:00"));
        settings.clock_24h = false;
        assert_eq!(settings.clock(Some(after)).as_deref(), Some("02:00 AM"));
        settings.location.timezone.clear();
        assert!(settings.valid());
        assert!(settings.hour(Some(after)).is_none());
    }
    #[test]
    fn canceled_stale_and_timed_out_location_results_do_not_change_settings() {
        let mut p = Preferences::default();
        p.leave(Some(Overlay::LocationResults));
        p.searching = true;
        p.search_deadline = 10_000;
        let id = p.request_id;
        p.leave(Some(Overlay::Settings));
        p.locations(id, Ok(vec![Location::default()]));
        assert!(p.results.is_empty());
        p.leave(Some(Overlay::LocationResults));
        p.searching = true;
        p.search_deadline = 10_000;
        let id = p.request_id;
        assert!(!p.tick(9_999));
        assert!(p.tick(10_000));
        p.locations(id, Ok(vec![Location::default()]));
        assert!(p.results.is_empty());
        assert_eq!(p.status, "Location search failed. Please retry.");
    }
}

#[cfg(test)]
mod legacy_tests {
    use super::*;
    #[test]
    fn imports_legacy_keys_without_assuming_a_timezone_or_modifying_the_source() {
        let settings = Settings::from_legacy(LegacySettings {
            brightness: Some(128),
            fahrenheit: Some(1),
            clock_24h: Some(1),
            night_mode: Some(1),
            sleep_minutes: Some(10),
            wake_on_presence: Some(0),
            language: Some(2),
            name: Some("Berlin".into()),
            latitude: Some(52.52),
            longitude: Some(13.405),
        });
        assert!(settings.valid());
        assert_eq!(settings.language, Language::German);
        assert_eq!(settings.brightness, 128);
        assert!(settings.fahrenheit && settings.clock_24h && settings.night_mode);
        assert!(!settings.wake_on_presence);
        assert_eq!(settings.location.name, "Berlin");
        assert!(settings.location.timezone.is_empty());
        assert!(settings.hour(Some(1_700_000_000)).is_none());
        let fallback = Settings::from_legacy(LegacySettings {
            brightness: Some(999),
            language: Some(99),
            name: Some("Bad".into()),
            latitude: Some(999.0),
            longitude: Some(0.0),
            ..Default::default()
        });
        assert_eq!(fallback, Settings::default());
    }
}
