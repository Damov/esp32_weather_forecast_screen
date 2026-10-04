// ============================================================================= //
// File          : settings_store.rs                                             //
// License       : MIT                                                           //
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
    // ESP-IDF nonvolatile-storage partition or namespace handle. Stored as EspDefaultNvs.
    nvs: EspDefaultNvs,
}
impl Store {
    /// Opens the writable app_settings NVS namespace without erasing storage.
    ///
    /// # Arguments
    ///
    /// * `partition` (`EspDefaultNvsPartition`) - NVS partition containing the new settings and
    ///   legacy namespaces.
    ///
    /// # Returns
    ///
    /// `Result<Self, &'static str>` - Ok with the settings store; Err with the settings read
    /// failure message.
    ///
    /// # Errors
    ///
    /// Returns an error when the namespace cannot be opened.
    pub fn open(partition: EspDefaultNvsPartition) -> Result<Self, &'static str> {
        // Return success; the caller receives a constructed Self value using these fields.
        Ok(Self {
            // Initialize ESP-IDF nonvolatile-storage partition or namespace handle from the
            // supplied value.
            nvs: EspDefaultNvs::new(partition, "app_settings", true)
                .map_err(|_| "Settings could not be read.")?,
        })
    }
    /// Loads a current profile or reads compatible legacy weather values when no profile
    /// exists.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Store`) - Opened settings NVS namespace. Borrowed without changing it.
    /// * `partition` (`EspDefaultNvsPartition`) - NVS partition containing the new settings and
    ///   legacy namespaces.
    ///
    /// # Returns
    ///
    /// `Result<Settings, &'static str>` - Ok with decoded, imported, or factory settings; Err
    /// for unreadable or invalid data. Legacy imports do not write storage.
    ///
    /// # Errors
    ///
    /// Rejects oversized or invalid current profiles and propagates NVS read failures; a
    /// missing legacy namespace yields defaults.
    pub fn load(&self, partition: EspDefaultNvsPartition) -> Result<Settings, &'static str> {
        // Check whether an available map err result for the surrounding operation matches the
        // requested case.
        if let Some(length) = self
            .nvs
            .blob_len("profile")
            .map_err(|_| "Settings could not be read.")?
        {
            // Check whether length is greater than 2048.
            if length > 2048 {
                // Leave this function now with a failure result for the surrounding operation;
                // later statements are skipped.
                return Err("Settings could not be read.");
            }
            // Keep byte buffer used to decode, encode, or transfer the surrounding data in this
            // local variable for the following operations.
            let mut bytes = vec![0; length];
            // Execute map err for get blob result for the surrounding operation.
            self.nvs
                .get_blob("profile", &mut bytes)
                .map_err(|_| "Settings could not be read.")?;
            // Leave this function now with decode result for the surrounding operation; later
            // statements are skipped.
            return Settings::decode(&bytes);
        }
        // Keep older optional preference fields read without modifying their original storage in
        // this local variable for the following operations.
        let legacy = match EspDefaultNvs::new(partition, "weather", false) {
            // Continue with the successful result, using its validated value in this case.
            Ok(nvs) => nvs,
            // Handle a failed operation here rather than treating its value as valid.
            Err(error) if error.code() == esp_idf_svc::sys::ESP_ERR_NVS_NOT_FOUND => {
                // Leave this function now with a successful result carrying the type's documented
                // default state; later statements are skipped.
                return Ok(Settings::default())
            }
            // Leave this function now with a failure result for the surrounding operation; later
            // statements are skipped.
            Err(_) => return Err("Settings could not be read."),
        };
        // Keep closure translating an SDK read failure into the stable non-erasing settings error
        // in this local variable for the following operations.
        let error = |_| "Settings could not be read.";
        // Return success; the caller receives from legacy result for the surrounding operation.
        Ok(Settings::from_legacy(LegacySettings {
            // Initialize backlight duty in 0-255; zero turns illumination off from the supplied
            // value.
            brightness: legacy.get_u32("brightness").map_err(error)?,
            // Initialize whether displayed temperatures are converted from cached Celsius to
            // Fahrenheit from the supplied value.
            fahrenheit: legacy.get_u8("useFahrenheit").map_err(error)?,
            // Initialize whether local time uses 24-hour rather than 12-hour formatting from the
            // supplied value.
            clock_24h: legacy.get_u8("use24Hour").map_err(error)?,
            // Initialize whether the local 22:00-06:00 backlight schedule is enabled from the
            // supplied value.
            night_mode: legacy.get_u8("useNightMode").map_err(error)?,
            // Initialize inactivity interval in minutes; zero disables ordinary inactivity blanking
            // from the supplied value.
            sleep_minutes: legacy.get_u32("standbyMinutes").map_err(error)?,
            // Initialize whether radar presence counts as activity and can restore the backlight
            // from the supplied value.
            wake_on_presence: legacy.get_u8("wakePresence").map_err(error)?,
            // Initialize selected language for interface text and location search from the supplied
            // value.
            language: legacy.get_u32("language").map_err(error)?,
            // Initialize human-readable label for the selected object from the supplied value.
            name: legacy_string(&legacy, "location")?,
            // Initialize latitude in degrees north, bounded to -90 through 90 from the supplied
            // value.
            latitude: legacy_string(&legacy, "latitude")?.and_then(|v| v.parse().ok()),
            // Initialize longitude in degrees east, bounded to -180 through 180 from the supplied
            // value.
            longitude: legacy_string(&legacy, "longitude")?.and_then(|v| v.parse().ok()),
        }))
    }
    /// Validates and writes a JSON preference profile to NVS.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Store`) - Opened settings NVS namespace. Borrowed without changing it.
    /// * `settings` (`&Settings`) - Current device preferences controlling time, brightness,
    ///   and inactivity behavior.
    ///
    /// # Returns
    ///
    /// `Result<(), &'static str>` - Ok after the profile write; Err with the settings storage
    /// failure message.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid settings, serialization failure, or NVS write failure.
    pub fn save(&self, settings: &Settings) -> Result<(), &'static str> {
        // Keep byte buffer used to decode, encode, or transfer the surrounding data in this local
        // variable for the following operations.
        let bytes = settings.encode()?;
        // Execute map err for set blob result for the surrounding operation.
        self.nvs
            .set_blob("profile", &bytes)
            .map_err(|_| "Settings storage failed.")
    }
}
/// Reads a bounded optional string from a legacy NVS key.
///
/// # Arguments
///
/// * `nvs` (`&EspDefaultNvs`) - NVS partition or namespace used to access persisted values.
/// * `key` (`&str`) - Legacy NVS string key to read.
///
/// # Returns
///
/// `Result<Option<String>, &'static str>` - Ok(Some(string)) for a stored value, Ok(None)
/// when absent, or Err for a read failure.
///
/// # Errors
///
/// Returns an error for NVS failures or values requiring more than 512 bytes.
fn legacy_string(nvs: &EspDefaultNvs, key: &str) -> Result<Option<String>, &'static str> {
    // Keep Some(length) in this local variable for the following operations.
    let Some(length) = nvs
        .str_len(key)
        .map_err(|_| "Settings could not be read.")?
    else {
        // Leave this function now with a successful result carrying no available value; later
        // statements are skipped.
        return Ok(None);
    };
    // Check whether length is greater than 512.
    if length > 512 {
        // Leave this function now with a failure result for the surrounding operation; later
        // statements are skipped.
        return Err("Settings could not be read.");
    }
    // Keep byte buffer used to decode, encode, or transfer the surrounding data in this local
    // variable for the following operations.
    let mut bytes = vec![0; length];
    // Return success; the caller receives the transformed value or entries produced by the closure.
    Ok(nvs
        .get_str(key, &mut bytes)
        .map_err(|_| "Settings could not be read.")?
        .map(str::to_owned))
}
