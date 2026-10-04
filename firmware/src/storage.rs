// ============================================================================= //
// File          : storage.rs                                                    //
// License       : MIT                                                           //
// Created       : 2026-10-04                                                    //
// Last modified : 2026-10-04                                                    //
// Author        : Daniel Kharlamov                                              //
// Last editor   : Daniel Kharlamov <daniel.kharlamov@googlemail.com>            //
// ============================================================================= //
//                                                                               //
// Summary:                                                                      //
// --------                                                                      //
// Native ESP-IDF Wi-Fi credential persistence.                                  //
//                                                                               //
// Module:                                                                       //
// -------                                                                       //
// Decodes and validates fixed-size native station profiles and loads, saves, or //
// forgets credentials through the Wi-Fi driver. Keeps normal configuration in   //
// RAM and switches to flash only for explicit persistence operations. Clears    //
// temporary secrets and tests profile decoding and storage mode restoration.    //
// ============================================================================= //

//! Native station credentials stored in unencrypted ESP-IDF NVS.
#[cfg(any(target_os = "espidf", test))]
use crate::model::{Credentials, Network, Security};

/// Native fields are fixed-size byte arrays, including a possible 32-byte SSID
/// without a trailing NUL. Borrow the password directly, avoiding extra copies.
///
/// Decodes fixed-size native SSID and password fields without copying the password first.
///
/// # Arguments
///
/// * `ssid` (`&[u8; 32]`) - Native fixed-size SSID field; all 32 bytes may be used without
///   a zero terminator.
/// * `password` (`&[u8; 64]`) - Native fixed-size password field, terminated by zero when
///   shorter than 64 bytes.
/// * `security` (`Security`) - Supported Wi-Fi authentication mode for the stored station
///   profile.
///
/// # Returns
///
/// `Result<Option<Credentials>, &'static str>` - Ok(Some(credentials)) for a supported
/// profile, Ok(None) for an empty SSID, or Err for invalid data.
///
/// # Errors
///
/// Rejects invalid UTF-8 or unsupported credential contents while leaving stored data
/// untouched.
#[cfg(any(target_os = "espidf", test))]
pub(crate) fn decode_station(
    ssid: &[u8; 32],
    password: &[u8; 64],
    security: Security,
) -> Result<Option<Credentials>, &'static str> {
    // Keep first zero byte in the SSID field, or the full 32-byte width when unterminated in this
    // local variable for the following operations.
    let ssid_end = ssid.iter().position(|b| *b == 0).unwrap_or(ssid.len());
    // An empty native network name means no configured station profile, not corrupted credentials.
    if ssid_end == 0 {
        // Leave this function now with a successful result carrying no available value; later
        // statements are skipped.
        return Ok(None);
    }
    // Keep first zero byte in the password field, or the full field width in this local variable
    // for the following operations.
    let password_end = password
        .iter()
        .position(|b| *b == 0)
        .unwrap_or(password.len());
    // Keep non-erasing validation error returned for unsupported or malformed saved credentials in
    // this local variable for the following operations.
    let invalid = "Saved Wi-Fi data is invalid or unsupported. Storage was not erased.";
    // Keep Wi-Fi network name, limited to 32 encoded bytes by station configuration in this local
    // variable for the following operations.
    let ssid = std::str::from_utf8(&ssid[..ssid_end]).map_err(|_| invalid)?;
    // Keep Wi-Fi passphrase stored in a buffer that is cleared when no longer needed in this local
    // variable for the following operations.
    let password = std::str::from_utf8(&password[..password_end]).map_err(|_| invalid)?;
    // Execute map err for the transformed value or entries produced by the closure.
    Credentials::new(
        &Network {
            // Initialize Wi-Fi network name, limited to 32 encoded bytes by station configuration
            // from the supplied value.
            ssid: ssid.to_owned(),
            // Initialize supported Wi-Fi authentication mode from the supplied value.
            security,
            // Start received signal strength in dBm; less-negative values represent stronger
            // signals at zero; later operations update it as needed.
            rssi: 0,
        },
        password,
    )
    .map(Some)
    .map_err(|_| invalid)
}

// Keep restoration separate from result propagation: a failed write must still
// leave subsequent connection attempts in RAM mode.
/// Enables flash writes temporarily and attempts to restore RAM mode after the write.
///
/// # Arguments
///
/// * `set_flash` (`impl FnMut(bool) -> Result<(), &'static str>`) - Storage-mode callback:
///   true enables flash, false restores RAM; returns Ok(()) or a static failure message.
/// * `write` (`impl FnOnce() -> Result<(), &'static str>`) - One-shot profile-write
///   callback with no arguments; returns Ok(()) or a static storage failure message.
///
/// # Returns
///
/// `Result<(), &'static str>` - Ok when enabling, writing, and restoration succeed; Err for
/// the first write error or a restoration error.
///
/// # Errors
///
/// Returns immediately if flash mode cannot be enabled; after a write attempt, restores RAM
/// mode even when writing fails.
#[cfg(any(target_os = "espidf", test))]
fn with_flash_storage(
    mut set_flash: impl FnMut(bool) -> Result<(), &'static str>,
    write: impl FnOnce() -> Result<(), &'static str>,
) -> Result<(), &'static str> {
    // Run the set flash operation with the supplied inputs.
    set_flash(true)?;
    // Keep the write outcome while still attempting RAM-mode restoration, including when the write
    // fails.
    let written = write();
    // Restore RAM-only station updates after the explicit flash write; its failure must also be
    // reported.
    let restored = set_flash(false);
    // Execute and for result of the attempted flash profile write; RAM restoration is attempted
    // even on error.
    written.and(restored)
}

/// Native ESP-IDF Wi-Fi profile access with temporary flash writes and zeroized buffers.
#[cfg(target_os = "espidf")]
mod native {
    use super::*;
    use esp_idf_svc::{sys::*, wifi::EspWifi};
    use zeroize::Zeroize;

    #[cfg(esp_idf_nvs_encryption)]
    compile_error!("Native Wi-Fi storage requires CONFIG_NVS_ENCRYPTION=n.");

    // Wipe on every return path, including SDK errors.
    struct StationConfig(wifi_config_t);
    impl Drop for StationConfig {
        /// Zeroizes the entire temporary native station configuration on destruction.
        ///
        /// # Arguments
        ///
        /// * `self` (`&mut StationConfig`) - Receiver state used by this operation. Mutated in
        ///   place.
        ///
        /// # Returns
        ///
        /// `()` - No value; clears all bytes, including credential fields, on every drop path.
        fn drop(&mut self) {
            unsafe {
                // Overwrite sensitive bytes rather than merely forgetting the buffer length.
                std::slice::from_raw_parts_mut(
                    (&mut self.0 as *mut wifi_config_t).cast::<u8>(),
                    std::mem::size_of::<wifi_config_t>(),
                )
                .zeroize();
            }
        }
    }
    /// Converts an ESP-IDF status code into the storage result type.
    ///
    /// # Arguments
    ///
    /// * `code` (`i32`) - ESP-IDF status code; ESP_OK indicates success.
    ///
    /// # Returns
    ///
    /// `Result<(), &'static str>` - Ok for ESP_OK; Err with the non-erasing storage failure
    /// message otherwise.
    ///
    /// # Errors
    ///
    /// Returns a storage failure message for every nonzero SDK status.
    fn checked(code: i32) -> Result<(), &'static str> {
        // Convert the SDK success status to Ok; all other codes become the non-erasing storage
        // failure message.
        if code == ESP_OK {
            // Return success after the required side effects are complete.
            Ok(())
        } else {
            // Return the failure result so the caller can display an error, retry, or preserve the
            // saved baseline.
            Err("Wi-Fi storage failed. Storage was not erased.")
        }
    }
    /// Reads the current station profile into a zeroizing temporary wrapper.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `Result<StationConfig, &'static str>` - Ok with the native station configuration; Err
    /// with the storage failure message.
    ///
    /// # Errors
    ///
    /// Returns an error if ESP-IDF cannot read station configuration.
    fn read() -> Result<StationConfig, &'static str> {
        // Keep temporary native Wi-Fi configuration wrapper whose bytes are zeroized when dropped
        // in this local variable for the following operations.
        let mut config = StationConfig(wifi_config_t::default());
        // Converts an ESP-IDF status code into the storage result type.
        checked(unsafe { esp_wifi_get_config(wifi_interface_t_WIFI_IF_STA, &mut config.0) })?;
        // Return success; the caller receives temporary native Wi-Fi configuration wrapper whose
        // bytes are zeroized when dropped.
        Ok(config)
    }
    /// Selects RAM-only Wi-Fi configuration storage.
    ///
    /// # Arguments
    ///
    /// * `_wifi` (`&mut EspWifi<'_>`) - Borrowed station driver establishing the active Wi-Fi
    ///   context; native SDK calls use the global driver.
    ///
    /// # Returns
    ///
    /// `Result<(), &'static str>` - Ok when RAM mode is selected; Err with the storage failure
    /// message.
    ///
    /// # Errors
    ///
    /// Returns an error if ESP-IDF rejects the storage mode change.
    pub fn use_ram(_wifi: &mut EspWifi<'_>) -> Result<(), &'static str> {
        // Converts an ESP-IDF status code into the storage result type.
        checked(unsafe { esp_wifi_set_storage(wifi_storage_t_WIFI_STORAGE_RAM) })
    }
    /// Reads and validates the native station profile, including compatible legacy auth
    /// thresholds.
    ///
    /// # Arguments
    ///
    /// * `_wifi` (`&EspWifi<'_>`) - Borrowed station driver establishing the active Wi-Fi
    ///   context; native SDK calls use the global driver.
    ///
    /// # Returns
    ///
    /// `Result<Option<Credentials>, &'static str>` - Ok(Some(credentials)) for a supported
    /// profile, Ok(None) when empty, or Err for read or validation failure.
    ///
    /// # Errors
    ///
    /// Returns an error for SDK read failures or malformed/unsupported credentials.
    pub fn load(_wifi: &EspWifi<'_>) -> Result<Option<Credentials>, &'static str> {
        // Keep temporary native Wi-Fi configuration wrapper whose bytes are zeroized when dropped
        // in this local variable for the following operations.
        let config = read()?;
        // Keep station portion of the native configuration union, borrowed without copying its
        // password in this local variable for the following operations.
        let sta = unsafe { &config.0.sta };
        // Arduino profiles can leave the minimum-auth threshold OPEN or WPA_PSK;
        // a supported passphrase is still usable with our WPA2 minimum threshold.
        // Keep supported Wi-Fi authentication mode in this local variable for the following
        // operations.
        let security = match sta.threshold.authmode {
            // Handle the mode if mode == wifi auth mode t WIFI AUTH WPA3 PSK case: apply the
            // state-specific behavior shown here.
            mode if mode == wifi_auth_mode_t_WIFI_AUTH_WPA3_PSK => Security::Wpa3,
            // Handle the mode if mode == wifi auth mode t WIFI AUTH WPA2 WPA3 PSK case: apply the
            // state-specific behavior shown here.
            mode if mode == wifi_auth_mode_t_WIFI_AUTH_WPA2_WPA3_PSK => Security::Mixed,
            // Handle the mode if [                 wifi auth mode t WIFI AUTH OPEN, wifi auth mode
            // t WIFI AUTH WPA PSK,                 wifi auth mode t WIFI AUTH WPA WPA2 PSK,
            // wifi auth mode t WIFI AUTH WPA2 PSK,             ] .contains(&mode) case: apply the
            // state-specific behavior shown here.
            mode if [
                wifi_auth_mode_t_WIFI_AUTH_OPEN,
                wifi_auth_mode_t_WIFI_AUTH_WPA_PSK,
                wifi_auth_mode_t_WIFI_AUTH_WPA_WPA2_PSK,
                wifi_auth_mode_t_WIFI_AUTH_WPA2_PSK,
            ]
            .contains(&mode) =>
            {
                Security::Wpa2
            }
            // Handle remaining cases with the fallback, preserving safe behavior for unsupported or
            // irrelevant input.
            _ => Security::Unsupported,
        };
        // Decodes fixed-size native SSID and password fields without copying the password first.
        decode_station(&sta.ssid, &sta.password, security)
    }
    /// Clears the station profile in flash and then restores RAM storage mode.
    ///
    /// # Arguments
    ///
    /// * `_wifi` (`&mut EspWifi<'_>`) - Borrowed station driver establishing the active Wi-Fi
    ///   context; native SDK calls use the global driver.
    ///
    /// # Returns
    ///
    /// `Result<(), &'static str>` - Ok after clearing and restoration; Err with the storage
    /// failure message.
    ///
    /// # Errors
    ///
    /// Returns an error for storage mode or profile writes; restoration is attempted after a
    /// write failure.
    pub fn forget(_wifi: &mut EspWifi<'_>) -> Result<(), &'static str> {
        // Keep temporary native Wi-Fi configuration wrapper whose bytes are zeroized when dropped
        // in this local variable for the following operations.
        let mut config = StationConfig(wifi_config_t::default());
        // Enables flash writes temporarily and attempts to restore RAM mode after the write.
        with_flash_storage(
            |flash| {
                // Converts an ESP-IDF status code into the storage result type.
                checked(unsafe {
                    // Choose flash storage only for the explicit write window; select RAM again for
                    // ordinary connection configuration.
                    esp_wifi_set_storage(if flash {
                        // Return wifi storage t WIFI STORAGE FLASH as the value of this block.
                        wifi_storage_t_WIFI_STORAGE_FLASH
                    } else {
                        // Return wifi storage t WIFI STORAGE RAM as the value of this block.
                        wifi_storage_t_WIFI_STORAGE_RAM
                    })
                })
            },
            // Call the native ESP-IDF station operation; surrounding checks determine how its
            // status is handled.
            || checked(unsafe { esp_wifi_set_config(wifi_interface_t_WIFI_IF_STA, &mut config.0) }),
        )
    }
    /// Checks the connected native profile against the candidate before persisting it.
    ///
    /// # Arguments
    ///
    /// * `wifi` (`&mut EspWifi<'_>`) - Initialized station driver used for configuration or
    ///   credential persistence.
    /// * `credentials` (`&Credentials`) - Validated candidate SSID, security mode, and
    ///   protected password; never log its contents.
    ///
    /// # Returns
    ///
    /// `Result<(), &'static str>` - Ok after flash persistence and RAM restoration; Err when
    /// connection, profile matching, or storage fails.
    ///
    /// # Errors
    ///
    /// Rejects disconnected, missing, changed, or invalid profiles and propagates SDK storage
    /// failures.
    pub fn save(wifi: &mut EspWifi<'_>, credentials: &Credentials) -> Result<(), &'static str> {
        // Only persist credentials after a confirmed IP connection; query errors are treated as not
        // connected.
        if !wifi.is_up().unwrap_or(false) {
            // Leave this function now with a failure result for the surrounding operation; later
            // statements are skipped.
            return Err("Connect to Wi-Fi before saving its settings.");
        }
        // Keep temporary native Wi-Fi configuration wrapper whose bytes are zeroized when dropped
        // in this local variable for the following operations.
        let mut config = read()?;
        // Keep station portion of the native configuration union, borrowed without copying its
        // password in this local variable for the following operations.
        let sta = unsafe { &config.0.sta };
        // Keep decoded current RAM station profile checked against the candidate before persistence
        // in this local variable for the following operations.
        let saved = decode_station(&sta.ssid, &sta.password, credentials.security)?
            .ok_or("No Wi-Fi configuration to save.")?;
        // The current driver profile no longer matches the successful candidate; refuse to persist
        // unrelated or stale credentials.
        if saved.ssid != credentials.ssid
            || saved.password.as_str() != credentials.password.as_str()
        {
            // Leave this function now with a failure result for the surrounding operation; later
            // statements are skipped.
            return Err("Wi-Fi configuration changed before saving.");
        }
        // Other configuration paths also require RAM explicitly, so even a
        // failed restoration cannot cause them to overwrite flash settings.
        // Enables flash writes temporarily and attempts to restore RAM mode after the write.
        with_flash_storage(
            |flash| {
                // Converts an ESP-IDF status code into the storage result type.
                checked(unsafe {
                    // Choose flash storage only for the explicit write window; select RAM again for
                    // ordinary connection configuration.
                    esp_wifi_set_storage(if flash {
                        // Return wifi storage t WIFI STORAGE FLASH as the value of this block.
                        wifi_storage_t_WIFI_STORAGE_FLASH
                    } else {
                        // Return wifi storage t WIFI STORAGE RAM as the value of this block.
                        wifi_storage_t_WIFI_STORAGE_RAM
                    })
                })
            },
            // Call the native ESP-IDF station operation; surrounding checks determine how its
            // status is handled.
            || checked(unsafe { esp_wifi_set_config(wifi_interface_t_WIFI_IF_STA, &mut config.0) }),
        )
    }
}
#[cfg(target_os = "espidf")]
pub use native::{forget, load, save, use_ram};

///////////////////////////////////////////////////////////////////////////////////////
//                                    UNIT TESTS                                     //
///////////////////////////////////////////////////////////////////////////////////////

/// Unit tests and supporting fixtures for storage behavior.
#[cfg(test)]
mod tests {
    use super::*;
    /// Copies a synthetic SSID and password into zero-filled native arrays.
    ///
    /// # Arguments
    ///
    /// * `ssid` (`&str`) - Synthetic network name copied into the test profile.
    /// * `password` (`&str`) - Borrowed printable ASCII passphrase; valid credentials require
    ///   8-63 bytes.
    ///
    /// # Returns
    ///
    /// `([u8; 32], [u8; 64])` - 32-byte SSID and 64-byte password arrays for station profile
    /// decoding tests.
    ///
    /// # Panics
    ///
    /// Panics if the synthetic SSID or password exceeds its fixed-size destination array.
    fn fields(ssid: &str, password: &str) -> ([u8; 32], [u8; 64]) {
        // Keep zero-initialized 32-byte SSID field used to construct a native profile fixture in
        // this local variable for the following operations.
        let mut name = [0; 32];
        // Keep zero-initialized 64-byte passphrase field used by decoding tests in this local
        // variable for the following operations.
        let mut pass = [0; 64];
        // Copy the supplied bytes into an equally sized destination slice.
        name[..ssid.len()].copy_from_slice(ssid.as_bytes());
        // Copy the supplied bytes into an equally sized destination slice.
        pass[..password.len()].copy_from_slice(password.as_bytes());
        // Return the ordered tuple of related values as the value of this block.
        (name, pass)
    }
    /// Verifies that an empty native SSID is decoded as no saved station profile.
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
    fn empty_native_profile_needs_setup() {
        // Verify the required fixture or invariant value, panicking if unavailable is unavailable.
        // A violation means the tested behavior is incorrect.
        assert!(decode_station(&[0; 32], &[0; 64], Security::Wpa2)
            .unwrap()
            .is_none());
    }
    /// Verifies decoding of maximum-width native SSIDs and retained passphrase contents.
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
    fn native_profile_preserves_full_ssid_and_password() {
        // Keep the returned components: `ssid` holds Wi-Fi network name, limited to 32 encoded
        // bytes by station configuration, `password` holds Wi-Fi passphrase stored in a buffer that
        // is cleared when no longer needed in this local variable for the following operations.
        let (ssid, password) = fields(&"s".repeat(32), &"p".repeat(63));
        // Keep credential or controller fixture used by the surrounding test in this local variable
        // for the following operations.
        let c = decode_station(&ssid, &password, Security::Wpa3)
            .unwrap()
            .unwrap();
        // Verify that Wi-Fi network name, limited to 32 encoded bytes by station configuration
        // exactly matches repeat result for the surrounding operation.
        assert_eq!(c.ssid, "s".repeat(32));
        // Verify that as str result for the surrounding operation exactly matches repeat result for
        // the surrounding operation.
        assert_eq!(c.password.as_str(), "p".repeat(63));
        // Verify that supported Wi-Fi authentication mode exactly matches Wpa3 state.
        assert_eq!(c.security, Security::Wpa3);
    }
    /// Verifies rejection of unsupported security, malformed UTF-8, and invalid native
    /// credentials.
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
    fn unsupported_and_malformed_native_profiles_are_rejected() {
        // Keep the returned components: `ssid` holds Wi-Fi network name, limited to 32 encoded
        // bytes by station configuration, `password` holds Wi-Fi passphrase stored in a buffer that
        // is cleared when no longer needed in this local variable for the following operations.
        let (mut ssid, password) = fields("Home", "password123");
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(decode_station(&ssid, &password, Security::Unsupported).is_err());
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(decode_station(&ssid, &[b'p'; 64], Security::Wpa2).is_err());
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(decode_station(&ssid, &[0; 64], Security::Wpa2).is_err());
        // Set the selected entry from Wi-Fi network name, limited to 32 encoded bytes by station
        // configuration to 0xff.
        ssid[0] = 0xff;
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(decode_station(&ssid, &password, Security::Wpa2).is_err());
    }
    /// Verifies attempted RAM restoration after both successful and failed flash writes.
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
    fn successful_and_failed_writes_always_restore_ram() {
        use std::cell::RefCell;
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for write_result in [Ok(()), Err("write failed")] {
            // Keep ordered log of flash-mode, write, and RAM-mode callbacks used to verify
            // restoration in this local variable for the following operations.
            let calls = RefCell::new(Vec::new());
            // Keep success or failure produced by the operation, retained for later handling in
            // this local variable for the following operations.
            let result = with_flash_storage(
                |flash| {
                    // Choose flash storage only for the explicit write window; select RAM again for
                    // ordinary connection configuration.
                    calls.borrow_mut().push(if flash { "flash" } else { "ram" });
                    // Return success after the required side effects are complete.
                    Ok(())
                },
                || {
                    // Append the new entry to the collection, preserving the order in which values
                    // arrive.
                    calls.borrow_mut().push("write");
                    // Return write result as the value of this block.
                    write_result
                },
            );
            // Verify that success or failure produced by the operation, retained for later handling
            // exactly matches write result.
            assert_eq!(result, write_result);
            // Verify that borrow result for the surrounding operation exactly matches the ordered
            // sample/byte array.
            assert_eq!(*calls.borrow(), ["flash", "write", "ram"]);
        }
    }
    /// Verifies that failure to enable flash mode prevents the profile write callback from
    /// running.
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
    fn flash_mode_failure_prevents_writing() {
        // Keep success or failure produced by the operation, retained for later handling in this
        // local variable for the following operations.
        let result = with_flash_storage(|_| Err("mode failed"), || panic!("must not write"));
        // Verify that success or failure produced by the operation, retained for later handling
        // exactly matches a failure result for the surrounding operation.
        assert_eq!(result, Err("mode failed"));
    }
    /// Verifies that a failed return to RAM mode is reported even when the profile write
    /// succeeded.
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
    fn ram_restoration_failure_is_reported() {
        // Keep success or failure produced by the operation, retained for later handling in this
        // local variable for the following operations.
        let result = with_flash_storage(
            // Choose flash storage only for the explicit write window; select RAM again for
            // ordinary connection configuration.
            |flash| if flash { Ok(()) } else { Err("mode failed") },
            || Ok(()),
        );
        // Verify that success or failure produced by the operation, retained for later handling
        // exactly matches a failure result for the surrounding operation.
        assert_eq!(result, Err("mode failed"));
    }
}
