//! Native station credentials stored in unencrypted ESP-IDF NVS.
#[cfg(any(target_os = "espidf", test))]
use crate::model::{Credentials, Network, Security};

/// Native fields are fixed-size byte arrays, including a possible 32-byte SSID
/// without a trailing NUL. Borrow the password directly, avoiding extra copies.
#[cfg(any(target_os = "espidf", test))]
pub(crate) fn decode_station(
    ssid: &[u8; 32],
    password: &[u8; 64],
    security: Security,
) -> Result<Option<Credentials>, &'static str> {
    let ssid_end = ssid.iter().position(|b| *b == 0).unwrap_or(ssid.len());
    if ssid_end == 0 {
        return Ok(None);
    }
    let password_end = password
        .iter()
        .position(|b| *b == 0)
        .unwrap_or(password.len());
    let invalid = "Saved Wi-Fi data is invalid or unsupported. Storage was not erased.";
    let ssid = std::str::from_utf8(&ssid[..ssid_end]).map_err(|_| invalid)?;
    let password = std::str::from_utf8(&password[..password_end]).map_err(|_| invalid)?;
    Credentials::new(
        &Network {
            ssid: ssid.to_owned(),
            security,
            rssi: 0,
        },
        password,
    )
    .map(Some)
    .map_err(|_| invalid)
}

// Keep restoration separate from result propagation: a failed write must still
// leave subsequent connection attempts in RAM mode.
#[cfg(any(target_os = "espidf", test))]
fn with_flash_storage(
    mut set_flash: impl FnMut(bool) -> Result<(), &'static str>,
    write: impl FnOnce() -> Result<(), &'static str>,
) -> Result<(), &'static str> {
    set_flash(true)?;
    let written = write();
    let restored = set_flash(false);
    written.and(restored)
}

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
        fn drop(&mut self) {
            unsafe {
                std::slice::from_raw_parts_mut(
                    (&mut self.0 as *mut wifi_config_t).cast::<u8>(),
                    std::mem::size_of::<wifi_config_t>(),
                )
                .zeroize();
            }
        }
    }
    fn checked(code: i32) -> Result<(), &'static str> {
        if code == ESP_OK {
            Ok(())
        } else {
            Err("Wi-Fi storage failed. Storage was not erased.")
        }
    }
    fn read() -> Result<StationConfig, &'static str> {
        let mut config = StationConfig(wifi_config_t::default());
        checked(unsafe { esp_wifi_get_config(wifi_interface_t_WIFI_IF_STA, &mut config.0) })?;
        Ok(config)
    }
    pub fn use_ram(_wifi: &mut EspWifi<'_>) -> Result<(), &'static str> {
        checked(unsafe { esp_wifi_set_storage(wifi_storage_t_WIFI_STORAGE_RAM) })
    }
    pub fn load(_wifi: &EspWifi<'_>) -> Result<Option<Credentials>, &'static str> {
        let config = read()?;
        let sta = unsafe { &config.0.sta };
        // Arduino profiles can leave the minimum-auth threshold OPEN or WPA_PSK;
        // a supported passphrase is still usable with our WPA2 minimum threshold.
        let security = match sta.threshold.authmode {
            mode if mode == wifi_auth_mode_t_WIFI_AUTH_WPA3_PSK => Security::Wpa3,
            mode if mode == wifi_auth_mode_t_WIFI_AUTH_WPA2_WPA3_PSK => Security::Mixed,
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
            _ => Security::Unsupported,
        };
        decode_station(&sta.ssid, &sta.password, security)
    }
    pub fn forget(_wifi: &mut EspWifi<'_>) -> Result<(), &'static str> {
        let mut config = StationConfig(wifi_config_t::default());
        with_flash_storage(
            |flash| {
                checked(unsafe {
                    esp_wifi_set_storage(if flash {
                        wifi_storage_t_WIFI_STORAGE_FLASH
                    } else {
                        wifi_storage_t_WIFI_STORAGE_RAM
                    })
                })
            },
            || checked(unsafe { esp_wifi_set_config(wifi_interface_t_WIFI_IF_STA, &mut config.0) }),
        )
    }
    pub fn save(wifi: &mut EspWifi<'_>, credentials: &Credentials) -> Result<(), &'static str> {
        if !wifi.is_up().unwrap_or(false) {
            return Err("Connect to Wi-Fi before saving its settings.");
        }
        let mut config = read()?;
        let sta = unsafe { &config.0.sta };
        let saved = decode_station(&sta.ssid, &sta.password, credentials.security)?
            .ok_or("No Wi-Fi configuration to save.")?;
        if saved.ssid != credentials.ssid
            || saved.password.as_str() != credentials.password.as_str()
        {
            return Err("Wi-Fi configuration changed before saving.");
        }
        // Other configuration paths also require RAM explicitly, so even a
        // failed restoration cannot cause them to overwrite flash settings.
        with_flash_storage(
            |flash| {
                checked(unsafe {
                    esp_wifi_set_storage(if flash {
                        wifi_storage_t_WIFI_STORAGE_FLASH
                    } else {
                        wifi_storage_t_WIFI_STORAGE_RAM
                    })
                })
            },
            || checked(unsafe { esp_wifi_set_config(wifi_interface_t_WIFI_IF_STA, &mut config.0) }),
        )
    }
}
#[cfg(target_os = "espidf")]
pub use native::{forget, load, save, use_ram};

#[cfg(test)]
mod tests {
    use super::*;
    fn fields(ssid: &str, password: &str) -> ([u8; 32], [u8; 64]) {
        let mut name = [0; 32];
        let mut pass = [0; 64];
        name[..ssid.len()].copy_from_slice(ssid.as_bytes());
        pass[..password.len()].copy_from_slice(password.as_bytes());
        (name, pass)
    }
    #[test]
    fn empty_native_profile_needs_setup() {
        assert!(decode_station(&[0; 32], &[0; 64], Security::Wpa2)
            .unwrap()
            .is_none());
    }
    #[test]
    fn native_profile_preserves_full_ssid_and_password() {
        let (ssid, password) = fields(&"s".repeat(32), &"p".repeat(63));
        let c = decode_station(&ssid, &password, Security::Wpa3)
            .unwrap()
            .unwrap();
        assert_eq!(c.ssid, "s".repeat(32));
        assert_eq!(c.password.as_str(), "p".repeat(63));
        assert_eq!(c.security, Security::Wpa3);
    }
    #[test]
    fn unsupported_and_malformed_native_profiles_are_rejected() {
        let (mut ssid, password) = fields("Home", "password123");
        assert!(decode_station(&ssid, &password, Security::Unsupported).is_err());
        assert!(decode_station(&ssid, &[b'p'; 64], Security::Wpa2).is_err());
        assert!(decode_station(&ssid, &[0; 64], Security::Wpa2).is_err());
        ssid[0] = 0xff;
        assert!(decode_station(&ssid, &password, Security::Wpa2).is_err());
    }
    #[test]
    fn successful_and_failed_writes_always_restore_ram() {
        use std::cell::RefCell;
        for write_result in [Ok(()), Err("write failed")] {
            let calls = RefCell::new(Vec::new());
            let result = with_flash_storage(
                |flash| {
                    calls.borrow_mut().push(if flash { "flash" } else { "ram" });
                    Ok(())
                },
                || {
                    calls.borrow_mut().push("write");
                    write_result
                },
            );
            assert_eq!(result, write_result);
            assert_eq!(*calls.borrow(), ["flash", "write", "ram"]);
        }
    }
    #[test]
    fn flash_mode_failure_prevents_writing() {
        let result = with_flash_storage(|_| Err("mode failed"), || panic!("must not write"));
        assert_eq!(result, Err("mode failed"));
    }
    #[test]
    fn ram_restoration_failure_is_reported() {
        let result = with_flash_storage(
            |flash| if flash { Ok(()) } else { Err("mode failed") },
            || Ok(()),
        );
        assert_eq!(result, Err("mode failed"));
    }
}
