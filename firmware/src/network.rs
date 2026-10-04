// ============================================================================= //
// File          : network.rs                                                    //
// License       : GPL-3.0-only                                                  //
// Created       : 2026-10-04                                                    //
// Last modified : 2026-10-04                                                    //
// Author        : Daniel Kharlamov                                              //
// Last editor   : Daniel Kharlamov <daniel.kharlamov@googlemail.com>            //
// ============================================================================= //
//                                                                               //
// Summary:                                                                      //
// --------                                                                      //
// ESP-IDF Wi-Fi backend and connection worker.                                  //
//                                                                               //
// Module:                                                                       //
// -------                                                                       //
// Initializes the Wi-Fi driver and event handling, scans networks, configures   //
// station credentials, and tracks association and IP readiness. Implements the  //
// connection service backend and credential storage operations, publishing      //
// events and online status to the main application.                             //
// ============================================================================= //

//! ESP-IDF station backend and background Wi-Fi command worker.
//!
//! Adapts driver status and native credential storage to the connection service,
//! reporting connection events and shared IP readiness to the application.

use crate::{
    model::{Command, Credentials, Event, Failure, Network, Security},
    service::{Backend, CredentialStore, Service},
    storage,
};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::modem::Modem,
    nvs::EspDefaultNvsPartition,
    sys::*,
    wifi::{AuthMethod, EspWifi, WifiEvent},
};
use std::{
    sync::{
        atomic::{AtomicU16, Ordering},
        mpsc::{Receiver, Sender},
        Arc,
    },
    time::{Duration, Instant},
};
use zeroize::Zeroize;

/// Converts completed ESP-IDF scan results to application network records.
///
/// # Arguments
///
/// * `wifi` (`&mut EspWifi<'static>`) - Initialized station driver used for configuration
///   or credential persistence.
///
/// # Returns
///
/// `Result<Vec<Network>, Failure>` - Ok with SSIDs, signal levels, and supported security
/// classifications; Err with Failure::Scan.
///
/// # Errors
///
/// Returns Failure::Scan if the driver cannot provide scan results.
fn list(wifi: &mut EspWifi<'static>) -> Result<Vec<Network>, Failure> {
    // Produce the transformed value or entries produced by the closure.
    wifi.driver_mut()
        .get_scan_result()
        .map_err(|_| Failure::Scan)
        .map(|aps| {
            // Produce the entries collected from the preceding iterator.
            aps.into_iter()
                .map(|ap| Network {
                    // Initialize Wi-Fi network name, limited to 32 encoded bytes by station
                    // configuration from the supplied value.
                    ssid: ap.ssid.to_string(),
                    // Initialize received signal strength in dBm; less-negative values represent
                    // stronger signals from the supplied value.
                    rssi: ap.signal_strength,
                    // Choose the appropriate path for auth method; each arm handles one supported
                    // case.
                    security: match ap.auth_method {
                        // Handle the Some(AuthMethod WPA2Personal) case: apply the state-specific
                        // behavior shown here.
                        Some(AuthMethod::WPA2Personal) => Security::Wpa2,
                        // Handle the Some(AuthMethod WPA3Personal) case: apply the state-specific
                        // behavior shown here.
                        Some(AuthMethod::WPA3Personal) => Security::Wpa3,
                        // Handle the Some(AuthMethod WPA2WPA3Personal) case: apply the
                        // state-specific behavior shown here.
                        Some(AuthMethod::WPA2WPA3Personal) => Security::Mixed,
                        // Handle remaining cases with the fallback, preserving safe behavior for
                        // unsupported or irrelevant input.
                        _ => Security::Unsupported,
                    },
                })
                .collect()
        })
}
/// Applies validated credentials to the RAM station profile and wipes its temporary native
/// copy.
///
/// # Arguments
///
/// * `wifi` (`&mut EspWifi<'static>`) - Initialized station driver used for configuration
///   or credential persistence.
/// * `c` (`&Credentials`) - Validated candidate credentials to configure or persist; never
///   log their password.
///
/// # Returns
///
/// `Result<(), Failure>` - Ok when configuration succeeds; Err with Failure::Connection.
///
/// # Errors
///
/// Returns an error for RAM mode selection failure, unsupported security, or native
/// configuration failure.
///
/// # Panics
///
/// Panics if the credential fields exceed the native arrays; supply credentials validated
/// by Credentials::new.
fn configure(wifi: &mut EspWifi<'static>, c: &Credentials) -> Result<(), Failure> {
    // Execute map err for use ram result for the surrounding operation.
    storage::use_ram(wifi).map_err(|_| Failure::Connection)?;
    // Perform the best-effort operation and intentionally ignore its result: Stop the current
    // association before changing or suspending a station profile.
    let _ = wifi.disconnect();
    // Keep native ESP-IDF authentication threshold selected from the validated security mode in
    // this local variable for the following operations.
    let authmode = match c.security {
        // Handle the Security Wpa2 case: apply the state-specific behavior shown here.
        Security::Wpa2 => wifi_auth_mode_t_WIFI_AUTH_WPA2_PSK,
        // Handle the Security Wpa3 case: apply the state-specific behavior shown here.
        Security::Wpa3 => wifi_auth_mode_t_WIFI_AUTH_WPA3_PSK,
        // Handle the Security Mixed case: apply the state-specific behavior shown here.
        Security::Mixed => wifi_auth_mode_t_WIFI_AUTH_WPA2_WPA3_PSK,
        // Leave this function now with a failure result for the surrounding operation; later
        // statements are skipped.
        Security::Unsupported => return Err(Failure::Connection),
    };
    // Keep temporary native station configuration; its credential bytes are wiped after SDK use in
    // this local variable for the following operations.
    let mut config = wifi_config_t::default();
    // SDK makes its own RAM-only copy. Wipe this temporary native structure
    // immediately after the call; do not format or inspect its contents.
    unsafe {
        // Keep station member of the native configuration union receiving SSID and password in this
        // local variable for the following operations.
        let sta = &mut config.sta;
        // Copy the supplied bytes into an equally sized destination slice.
        sta.ssid[..c.ssid.len()].copy_from_slice(c.ssid.as_bytes());
        // Copy the supplied bytes into an equally sized destination slice.
        sta.password[..c.password.len()].copy_from_slice(c.password.as_bytes());
        // Set received signal strength in dBm to 127.
        sta.threshold.rssi = -127;
        // Set native ESP-IDF authentication threshold selected from the validated security mode to
        // native ESP-IDF authentication threshold selected from the validated security mode.
        sta.threshold.authmode = authmode;
        // Set capable to the enabled state.
        sta.pmf_cfg.capable = true;
        // Set required to supported Wi-Fi authentication mode equals Wpa3 state.
        sta.pmf_cfg.required = c.security == Security::Wpa3;
        // Set sae pwe h2e to wifi sae pwe method t WPA3 SAE PWE BOTH.
        sta.sae_pwe_h2e = wifi_sae_pwe_method_t_WPA3_SAE_PWE_BOTH;
    }
    // Keep ESP-IDF status returned by the native station configuration call in this local variable
    // for the following operations.
    let code = unsafe { esp_wifi_set_config(wifi_interface_t_WIFI_IF_STA, &mut config) };
    unsafe {
        // Overwrite sensitive bytes rather than merely forgetting the buffer length.
        std::slice::from_raw_parts_mut(
            (&mut config as *mut wifi_config_t).cast::<u8>(),
            std::mem::size_of::<wifi_config_t>(),
        )
        .zeroize();
    }
    // The native configuration call failed; do not report the candidate station profile as ready.
    if code != ESP_OK {
        // Leave this function now with a failure result for the surrounding operation; later
        // statements are skipped.
        return Err(Failure::Connection);
    }
    // Return success after the required side effects are complete.
    Ok(())
}
/// Disconnects, stops scanning, and clears only the RAM station profile.
///
/// # Arguments
///
/// * `wifi` (`&mut EspWifi<'static>`) - Initialized station driver used for configuration
///   or credential persistence.
///
/// # Returns
///
/// `()` - No value; driver errors are ignored and flash credentials are left untouched.
fn clear_driver(wifi: &mut EspWifi<'static>) {
    // Perform the best-effort operation and intentionally ignore its result: Stop the current
    // association before changing or suspending a station profile.
    let _ = wifi.disconnect();
    // Perform the best-effort operation and intentionally ignore its result: Cancel an in-progress
    // access-point scan before replacing its job.
    let _ = wifi.driver_mut().stop_scan();
    // If returning to RAM fails, disconnect but never write a blank flash profile.
    // RAM-only mode could not be established; return without writing a blank profile that might
    // erase flash credentials.
    if storage::use_ram(wifi).is_err() {
        // Leave this function now after completing the required side effects; later statements are
        // skipped.
        return;
    }
    // Keep empty native station configuration applied only after RAM mode is confirmed in this
    // local variable for the following operations.
    let mut blank = wifi_config_t::default();
    unsafe {
        // Call the native ESP-IDF station operation; surrounding checks determine how its status is
        // handled.
        esp_wifi_set_config(wifi_interface_t_WIFI_IF_STA, &mut blank);
    }
}
/// Classifies selected ESP-IDF disconnect reason codes as authentication failures.
///
/// # Arguments
///
/// * `reason` (`u16`) - ESP-IDF station disconnect reason code.
///
/// # Returns
///
/// `bool` - True for codes 15, 16, 23, 202, or 204; false otherwise.
fn authentication_reason(reason: u16) -> bool {
    matches!(reason, 15 | 16 | 23 | 202 | 204)
}
/// Runs the Wi-Fi worker and reports fatal initialization failures to the application.
///
/// # Arguments
///
/// * `modem` (`Modem<'static>`) - Owned ESP32 Wi-Fi modem peripheral for station
///   initialization.
/// * `nvs` (`EspDefaultNvsPartition`) - NVS partition or namespace used to access persisted
///   values.
/// * `rx` (`Receiver<Command>`) - Owned channel receiving Wi-Fi commands until shutdown.
/// * `tx` (`Sender<Event>`) - Channel publishing connection events to the main application.
/// * `online` (`Arc<std::sync::atomic::AtomicBool>`) - Shared flag indicating Wi-Fi IP
///   readiness.
///
/// # Returns
///
/// `()` - No value; clears shared online status on exit and attempts to publish a fatal
/// event after worker failure.
pub fn run(
    modem: Modem<'static>,
    nvs: EspDefaultNvsPartition,
    rx: Receiver<Command>,
    tx: Sender<Event>,
    online: Arc<std::sync::atomic::AtomicBool>,
) {
    // Keep success or failure produced by the operation, retained for later handling in this local
    // variable for the following operations.
    let result = run_inner(modem, nvs, rx, &tx, &online);
    // Publish the new shared atomic value for the worker or application to observe.
    online.store(false, Ordering::Relaxed);
    // The worker failed unexpectedly; notify the main application and clear shared online status.
    if result.is_err() {
        // Perform the best-effort operation and intentionally ignore its result: Deliver the event
        // or command to its worker/application channel; this send waits if required by the channel.
        let _ = tx.send(Event::Fatal(
            "Wi-Fi initialization failed. Restart the device or check firmware configuration.",
        ));
    }
}
/// Initializes ESP-IDF Wi-Fi and processes commands and service events until channel
/// shutdown.
///
/// # Arguments
///
/// * `modem` (`Modem<'static>`) - Owned ESP32 Wi-Fi modem peripheral for station
///   initialization.
/// * `nvs` (`EspDefaultNvsPartition`) - NVS partition or namespace used to access persisted
///   values.
/// * `rx` (`Receiver<Command>`) - Owned channel receiving Wi-Fi commands until shutdown.
/// * `tx` (`&Sender<Event>`) - Channel publishing connection events to the main
///   application.
/// * `online` (`&std::sync::atomic::AtomicBool`) - Shared flag indicating Wi-Fi IP
///   readiness.
///
/// # Returns
///
/// `anyhow::Result<()>` - Ok on orderly command-channel closure or handled credential-load
/// failure; Err for initialization or event-channel failure.
///
/// # Errors
///
/// Propagates event-loop, Wi-Fi, storage-mode, and channel send errors; invalid saved
/// credentials are reported as a fatal event.
fn run_inner(
    modem: Modem<'static>,
    nvs: EspDefaultNvsPartition,
    rx: Receiver<Command>,
    tx: &Sender<Event>,
    online: &std::sync::atomic::AtomicBool,
) -> anyhow::Result<()> {
    // Keep system event loop used by ESP-IDF Wi-Fi and disconnect notifications in this local
    // variable for the following operations.
    let sysloop = EspSystemEventLoop::take()?;
    // Keep atomic disconnect-reason code shared with the Wi-Fi callback in this local variable for
    // the following operations.
    let reason = Arc::new(AtomicU16::new(0));
    // Obtain an owned clone of shared disconnect-reason handle retained by the Wi-Fi event
    // callback; reference-counted handles continue to share their underlying state.
    let callback_reason = reason.clone();
    // Keep retained event subscription; dropping it would stop disconnect-reason updates in this
    // local variable for the following operations.
    let _subscription = sysloop.subscribe::<WifiEvent, _>(move |event| {
        // Record station disconnect reasons so the service can distinguish authentication rejection
        // from missing access points.
        if let WifiEvent::StaDisconnected(data) = event {
            // Publish the new shared atomic value for the worker or application to observe.
            callback_reason.store(data.reason(), Ordering::Relaxed);
        }
    })?;
    // SDK initialization loads the native saved station profile. Set mode only:
    // a default ClientConfiguration would overwrite the loaded credentials.
    // Keep initialized ESP-IDF station driver or the corresponding setup control under test in this
    // local variable for the following operations.
    let mut wifi = EspWifi::new(modem, sysloop, Some(nvs))?;
    // Execute map err for use ram result for the surrounding operation.
    storage::use_ram(&mut wifi).map_err(anyhow::Error::msg)?;
    // Convert the native ESP-IDF status into a Rust result so initialization failure can be
    // propagated.
    esp_idf_svc::sys::esp!(unsafe { esp_wifi_set_mode(wifi_mode_t_WIFI_MODE_STA) })?;
    // Start the configured Wi-Fi station driver before processing application commands.
    wifi.start()?;
    // Keep adapter providing network operations and credential persistence in this local variable
    // for the following operations.
    let backend = EspBackend { wifi, reason };
    // Keep hardware-independent Wi-Fi job scheduler in this local variable for the following
    // operations.
    let mut service = match Service::new(backend) {
        // Continue with the successful result, using its validated value in this case.
        Ok(service) => service,
        // Handle a failed operation here rather than treating its value as valid.
        Err(text) => {
            // Perform the best-effort operation and intentionally ignore its result: Deliver the
            // event or command to its worker/application channel; this send waits if required by
            // the channel.
            let _ = tx.send(Event::Fatal(text));
            // Leave this function now with a successful result carrying (); later statements are
            // skipped.
            return Ok(());
        }
    };

    // Deliver the event or command to its worker/application channel; this send waits if required
    // by the channel.
    tx.send(service.ready())?;
    // Keep monotonic starting instant, independent of wall-clock corrections in this local variable
    // for the following operations.
    let started = Instant::now();
    // Keep processing events or samples until an explicit break, return, or channel shutdown ends
    // this loop.
    loop {
        // Choose the appropriate path for recv timeout result for the surrounding operation; each
        // arm handles one supported case.
        match rx.recv_timeout(Duration::from_millis(50)) {
            // Continue with the successful result, using its validated value in this case.
            Ok(command) => {
                // Check whether an available command result for the surrounding operation matches
                // the requested case.
                if let Some(event) = service.command(command, started.elapsed().as_millis() as u64)
                {
                    // Deliver the event or command to its worker/application channel; this send
                    // waits if required by the channel.
                    tx.send(event)?;
                }
            }
            // Stop this loop now; the enclosing function continues with the work after the loop.
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            // Handle a failed operation here rather than treating its value as valid.
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
        }
        // Check whether an available poll result for the surrounding operation matches the
        // requested case.
        if let Some(event) = service.poll(started.elapsed().as_millis() as u64) {
            // Deliver the event or command to its worker/application channel; this send waits if
            // required by the channel.
            tx.send(event)?;
        }
        // Publish the new shared atomic value for the worker or application to observe.
        online.store(service.connected(), Ordering::Relaxed);
    }
    // Return success after the required side effects are complete.
    Ok(())
}

struct EspBackend {
    // Initialized ESP-IDF station driver or the corresponding setup control under test. Stored as
    // EspWifi<'static>.
    wifi: EspWifi<'static>,
    // Atomic disconnect-reason code shared with the Wi-Fi callback. Stored as Arc<AtomicU16>.
    reason: Arc<AtomicU16>,
}
impl Backend for EspBackend {
    /// Disconnects and clears transient RAM configuration and the cached disconnect reason.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut EspBackend`) - ESP-IDF station driver and cached disconnect reason.
    ///   Mutated in place.
    ///
    /// # Returns
    ///
    /// `()` - No value; persisted credentials are retained.
    fn clear(&mut self) {
        // Disconnects, stops scanning, and clears only the RAM station profile.
        clear_driver(&mut self.wifi);
        // Publish the new shared atomic value for the worker or application to observe.
        self.reason.store(0, Ordering::Relaxed);
    }
    /// Starts a nonblocking ESP-IDF network scan.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut EspBackend`) - ESP-IDF station driver and cached disconnect reason.
    ///   Mutated in place.
    ///
    /// # Returns
    ///
    /// `Result<(), Failure>` - Ok when the driver accepts scanning; Err with Failure::Scan.
    ///
    /// # Errors
    ///
    /// Returns Failure::Scan if scanning cannot start.
    fn begin_scan(&mut self) -> Result<(), Failure> {
        // Execute map err for start scan result for the surrounding operation.
        self.wifi
            .start_scan(&Default::default(), false)
            .map_err(|_| Failure::Scan)
    }
    /// Checks driver scan completion and converts available results.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut EspBackend`) - ESP-IDF station driver and cached disconnect reason.
    ///   Mutated in place.
    ///
    /// # Returns
    ///
    /// `Result<Option<Vec<Network>>, Failure>` - Ok(Some(networks)) when complete, Ok(None)
    /// while pending, or Err with Failure::Scan.
    ///
    /// # Errors
    ///
    /// Returns Failure::Scan when scan status or results cannot be read.
    fn scan_result(&mut self) -> Result<Option<Vec<Network>>, Failure> {
        // Only retrieve scan results after the driver reports completion; otherwise keep the
        // asynchronous job pending.
        if self
            .wifi
            .driver()
            .is_scan_done()
            .map_err(|_| Failure::Scan)?
        {
            // Produce the transformed value or entries produced by the closure.
            list(&mut self.wifi).map(Some)
        } else {
            // Return success; the caller receives no available value.
            Ok(None)
        }
    }
    /// Applies RAM-only credentials and clears the previous disconnect reason.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut EspBackend`) - ESP-IDF station driver and cached disconnect reason.
    ///   Mutated in place.
    /// * `c` (`&Credentials`) - Validated candidate credentials to configure or persist; never
    ///   log their password.
    ///
    /// # Returns
    ///
    /// `Result<(), Failure>` - Ok after configuration; Err with Failure::Connection.
    ///
    /// # Errors
    ///
    /// Propagates the RAM station configuration failure.
    ///
    /// # Panics
    ///
    /// Panics if the credential fields exceed the native arrays; supply credentials validated
    /// by Credentials::new.
    fn configure(&mut self, c: &Credentials) -> Result<(), Failure> {
        // Applies validated credentials to the RAM station profile and wipes its temporary native
        // copy.
        configure(&mut self.wifi, c)?;
        // Publish the new shared atomic value for the worker or application to observe.
        self.reason.store(0, Ordering::Relaxed);
        // Return success after the required side effects are complete.
        Ok(())
    }
    /// Requests a Wi-Fi connection attempt through ESP-IDF.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut EspBackend`) - ESP-IDF station driver and cached disconnect reason.
    ///   Mutated in place.
    ///
    /// # Returns
    ///
    /// `Result<(), Failure>` - Ok when the request succeeds; Err with Failure::Connection.
    ///
    /// # Errors
    ///
    /// Returns Failure::Connection if the driver rejects the attempt.
    fn connect(&mut self) -> Result<(), Failure> {
        // Execute map err for requests a Wi-Fi connection attempt through ESP-IDF.
        self.wifi.connect().map_err(|_| Failure::Connection)
    }
    /// Checks the driver's association status.
    ///
    /// # Arguments
    ///
    /// * `self` (`&EspBackend`) - ESP-IDF station driver and cached disconnect reason. Borrowed
    ///   without changing it.
    ///
    /// # Returns
    ///
    /// `bool` - True when associated; false when disconnected or the status query fails.
    fn associated(&self) -> bool {
        // Execute unwrap or for is connected result for the surrounding operation.
        self.wifi.is_connected().unwrap_or(false)
    }
    /// Checks whether the driver reports an assigned IP connection.
    ///
    /// # Arguments
    ///
    /// * `self` (`&EspBackend`) - ESP-IDF station driver and cached disconnect reason. Borrowed
    ///   without changing it.
    ///
    /// # Returns
    ///
    /// `bool` - True when up; false when unavailable or the status query fails.
    fn up(&self) -> bool {
        // Execute unwrap or for is up result for the surrounding operation.
        self.wifi.is_up().unwrap_or(false)
    }
    /// Classifies the most recently recorded station disconnect reason.
    ///
    /// # Arguments
    ///
    /// * `self` (`&EspBackend`) - ESP-IDF station driver and cached disconnect reason. Borrowed
    ///   without changing it.
    ///
    /// # Returns
    ///
    /// `bool` - True for the recognized authentication failure codes.
    fn authentication_failed(&self) -> bool {
        // Classifies selected ESP-IDF disconnect reason codes as authentication failures.
        authentication_reason(self.reason.load(Ordering::Relaxed))
    }
    /// Checks for the ESP-IDF access-point-not-found reason.
    ///
    /// # Arguments
    ///
    /// * `self` (`&EspBackend`) - ESP-IDF station driver and cached disconnect reason. Borrowed
    ///   without changing it.
    ///
    /// # Returns
    ///
    /// `bool` - True when the cached disconnect reason is 201; false otherwise.
    fn network_missing(&self) -> bool {
        // Return loads the station profile through the native storage adapter equals 201 as the
        // value of this block.
        self.reason.load(Ordering::Relaxed) == 201
    }
}
impl CredentialStore for EspBackend {
    /// Clears persisted station credentials through the native storage adapter.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut EspBackend`) - ESP-IDF station driver and cached disconnect reason.
    ///   Mutated in place.
    ///
    /// # Returns
    ///
    /// `Result<(), &'static str>` - Ok when clearing succeeds; Err with the storage failure
    /// message.
    ///
    /// # Errors
    ///
    /// Propagates native storage mode or configuration errors.
    fn forget(&mut self) -> Result<(), &'static str> {
        // Clears persisted station credentials through the native storage adapter.
        storage::forget(&mut self.wifi)
    }
    /// Loads the station profile through the native storage adapter.
    ///
    /// # Arguments
    ///
    /// * `self` (`&EspBackend`) - ESP-IDF station driver and cached disconnect reason. Borrowed
    ///   without changing it.
    ///
    /// # Returns
    ///
    /// `Result<Option<Credentials>, &'static str>` - Ok(Some(credentials)) for a valid profile,
    /// Ok(None) when empty, or Err for unreadable or invalid data.
    ///
    /// # Errors
    ///
    /// Propagates native profile read or validation errors.
    fn load(&self) -> Result<Option<Credentials>, &'static str> {
        // Loads the station profile through the native storage adapter.
        storage::load(&self.wifi)
    }
    /// Persists the connected station profile after checking candidate credentials.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut EspBackend`) - ESP-IDF station driver and cached disconnect reason.
    ///   Mutated in place.
    /// * `c` (`&Credentials`) - Validated candidate credentials to configure or persist; never
    ///   log their password.
    ///
    /// # Returns
    ///
    /// `Result<(), &'static str>` - Ok when persistence succeeds; Err when connection, profile
    /// validation, or storage fails.
    ///
    /// # Errors
    ///
    /// Propagates native credential persistence errors.
    fn save(&mut self, c: &Credentials) -> Result<(), &'static str> {
        // Persists the connected station profile after checking candidate credentials.
        storage::save(&mut self.wifi, c)
    }
}
