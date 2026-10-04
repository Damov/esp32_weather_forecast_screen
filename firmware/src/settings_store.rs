// ============================================================================= //
// File          : settings_store.rs                                             //
// License       : GPL-3.0-only                                                  //
// Created       : 2026-10-04                                                    //
// Last modified : 2026-10-04                                                    //
// Author        : Daniel Kharlamov                                              //
// Last editor   : Daniel Kharlamov <daniel.kharlamov@googlemail.com>            //
// ============================================================================= //
//                                                                               //
// Summary:                                                                      //
// --------                                                                      //
// ESP-IDF NVS storage and legacy import for device settings.                    //
//                                                                               //
// Module:                                                                       //
// -------                                                                       //
// Loads and saves validated preference profiles in the app_settings NVS         //
// namespace. Imports older weather namespace values when no current profile     //
// exists, keeping legacy reads separate from new profile writes and returning   //
// storage or decoding failures to the caller.                                   //
// ============================================================================= //

//! Independent NVS profile with read-only imports from the legacy weather namespace.
use crate::settings::{LegacySettings, Settings};
use esp_idf_svc::nvs::{EspDefaultNvs, EspDefaultNvsPartition};
pub struct Store {
    nvs: EspDefaultNvs,
}
impl Store {
    pub fn open(partition: EspDefaultNvsPartition) -> Result<Self, &'static str> {
        Ok(Self {
            nvs: EspDefaultNvs::new(partition, "app_settings", true)
                .map_err(|_| "Settings could not be read.")?,
        })
    }
    pub fn load(&self, partition: EspDefaultNvsPartition) -> Result<Settings, &'static str> {
        if let Some(length) = self
            .nvs
            .blob_len("profile")
            .map_err(|_| "Settings could not be read.")?
        {
            if length > 2048 {
                return Err("Settings could not be read.");
            }
            let mut bytes = vec![0; length];
            self.nvs
                .get_blob("profile", &mut bytes)
                .map_err(|_| "Settings could not be read.")?;
            return Settings::decode(&bytes);
        }
        let legacy = match EspDefaultNvs::new(partition, "weather", false) {
            Ok(nvs) => nvs,
            Err(error) if error.code() == esp_idf_svc::sys::ESP_ERR_NVS_NOT_FOUND => {
                return Ok(Settings::default())
            }
            Err(_) => return Err("Settings could not be read."),
        };
        let error = |_| "Settings could not be read.";
        Ok(Settings::from_legacy(LegacySettings {
            brightness: legacy.get_u32("brightness").map_err(error)?,
            fahrenheit: legacy.get_u8("useFahrenheit").map_err(error)?,
            clock_24h: legacy.get_u8("use24Hour").map_err(error)?,
            night_mode: legacy.get_u8("useNightMode").map_err(error)?,
            sleep_minutes: legacy.get_u32("standbyMinutes").map_err(error)?,
            wake_on_presence: legacy.get_u8("wakePresence").map_err(error)?,
            language: legacy.get_u32("language").map_err(error)?,
            name: legacy_string(&legacy, "location")?,
            latitude: legacy_string(&legacy, "latitude")?.and_then(|v| v.parse().ok()),
            longitude: legacy_string(&legacy, "longitude")?.and_then(|v| v.parse().ok()),
        }))
    }
    pub fn save(&self, settings: &Settings) -> Result<(), &'static str> {
        let bytes = settings.encode()?;
        self.nvs
            .set_blob("profile", &bytes)
            .map_err(|_| "Settings storage failed.")
    }
}
fn legacy_string(nvs: &EspDefaultNvs, key: &str) -> Result<Option<String>, &'static str> {
    let Some(length) = nvs
        .str_len(key)
        .map_err(|_| "Settings could not be read.")?
    else {
        return Ok(None);
    };
    if length > 512 {
        return Err("Settings could not be read.");
    }
    let mut bytes = vec![0; length];
    Ok(nvs
        .get_str(key, &mut bytes)
        .map_err(|_| "Settings could not be read.")?
        .map(str::to_owned))
}
