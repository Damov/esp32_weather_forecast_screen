// ============================================================================= //
// File          : model.rs                                                      //
// License       : MIT                                                           //
// Created       : 2026-10-04                                                    //
// Last modified : 2026-10-04                                                    //
// Author        : Daniel Kharlamov                                              //
// Last editor   : Daniel Kharlamov <daniel.kharlamov@googlemail.com>            //
// ============================================================================= //
//                                                                               //
// Summary:                                                                      //
// --------                                                                      //
// Application state and Wi-Fi setup commands and events.                        //
//                                                                               //
// Module:                                                                       //
// -------                                                                       //
// Defines networks, security modes, protected credentials, connection failures, //
// and UI pages. Maintains application state, processes worker events, filters   //
// network choices, validates password input, and generates connection and retry //
// commands while clearing secrets when no longer needed.                        //
// ============================================================================= //

//! Application and Wi-Fi setup state shared by device and host code.
//!
//! Uses request generations to reject obsolete events and zeroizing storage for
//! entered credentials while retaining weather and settings navigation state.

use zeroize::{Zeroize, Zeroizing};

// Allow twenty seconds for station association and IP readiness before reporting failure.
pub const CONNECT_TIMEOUT_SECS: u64 = 20;
// Wait thirty monotonic seconds before the application retries a failed saved-profile connection.
pub const RETRY_SECS: u64 = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Security {
    // Supported WPA2 personal security requiring a printable passphrase.
    Wpa2,
    // Supported WPA3 personal security with the native protected-management settings.
    Wpa3,
    // Supported WPA2/WPA3 transition mode.
    Mixed,
    // Open, WEP, enterprise, or unrecognized security is excluded from setup.
    Unsupported,
}
impl Security {
    /// Formats a Wi-Fi security mode for the interface.
    ///
    /// # Arguments
    ///
    /// * `self` (`Security`) - Wi-Fi authentication classification. Passed by value.
    ///
    /// # Returns
    ///
    /// `&'static str` - Static WPA2, WPA3, mixed, or unsupported label.
    pub fn label(self) -> &'static str {
        // Choose the appropriate path for self; each arm handles one supported case.
        match self {
            // Handle the Self Wpa2 case: apply the state-specific behavior shown here.
            Self::Wpa2 => "WPA2",
            // Handle the Self Wpa3 case: apply the state-specific behavior shown here.
            Self::Wpa3 => "WPA3",
            // Handle the Self Mixed case: apply the state-specific behavior shown here.
            Self::Mixed => "WPA2/3",
            // Handle the Self Unsupported case: apply the state-specific behavior shown here.
            Self::Unsupported => "Unsupported",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Network {
    // Wi-Fi network name, limited to 32 encoded bytes by station configuration. Stored as String.
    pub ssid: String,
    // Supported Wi-Fi authentication mode. Stored as Security.
    pub security: Security,
    // Received signal strength in dBm; less-negative values represent stronger signals. Stored as
    // i8.
    pub rssi: i8,
}

// Deliberately no Debug: commands and records must never log secrets.
pub struct Credentials {
    // Wi-Fi network name, limited to 32 encoded bytes by station configuration. Stored as String.
    pub ssid: String,
    // Supported Wi-Fi authentication mode. Stored as Security.
    pub security: Security,
    // Wi-Fi passphrase stored in a buffer that is cleared when no longer needed. Stored as
    // Zeroizing<String>.
    pub password: Zeroizing<String>,
}
impl Credentials {
    /// Validates network details and copies the password into zeroizing storage.
    ///
    /// # Arguments
    ///
    /// * `network` (`&Network`) - Scanned network providing SSID and supported security mode.
    /// * `password` (`&str`) - Borrowed printable ASCII passphrase; valid credentials require
    ///   8-63 bytes.
    ///
    /// # Returns
    ///
    /// `Result<Self, &'static str>` - Ok with owned credentials; Err with a validation message.
    ///
    /// # Errors
    ///
    /// Rejects empty or oversized SSIDs, unsupported security, and passwords outside 8-63
    /// printable ASCII bytes.
    pub fn new(network: &Network, password: &str) -> Result<Self, &'static str> {
        // Reject empty network names or names exceeding the native station's 32-byte SSID field.
        if network.ssid.is_empty() || network.ssid.len() > 32 {
            // Leave this function now with a failure result for the surrounding operation; later
            // statements are skipped.
            return Err("Invalid network name.");
        }
        // Do not accept open, WEP, enterprise, or unrecognized security as a supported personal
        // network.
        if network.security == Security::Unsupported {
            // Leave this function now with a failure result for the surrounding operation; later
            // statements are skipped.
            return Err("This network security is unsupported.");
        }
        // Accept only an 8-63 byte printable ASCII passphrase, matching the station credential
        // policy.
        if !(8..=63).contains(&password.len()) || !password.bytes().all(|b| (32..=126).contains(&b))
        {
            // Leave this function now with a failure result for the surrounding operation; later
            // statements are skipped.
            return Err("Use a password of 8-63 characters.");
        }
        // Return success; the caller receives a constructed Self value using these fields.
        Ok(Self {
            // Initialize Wi-Fi network name, limited to 32 encoded bytes by station configuration
            // from the supplied value.
            ssid: network.ssid.clone(),
            // Initialize supported Wi-Fi authentication mode from the supplied value.
            security: network.security,
            // Initialize Wi-Fi passphrase stored in a buffer that is cleared when no longer needed
            // from the supplied value.
            password: Zeroizing::new(password.to_owned()),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    // The configured access point was not found within the recovery window.
    Missing,
    // The completed scan returned no visible access points.
    NoNetworks,
    // Association failed because authentication was rejected.
    Authentication,
    // The station associated but did not obtain an IP address before its deadline.
    IpTimeout,
    // Scanning failed, or a scan command requests a new access-point list.
    Scan,
    // The station could not complete its connection attempt.
    Connection,
    // Connected credentials could not be persisted safely.
    Save,
}
impl Failure {
    /// Maps a connection failure to its user-facing message.
    ///
    /// # Arguments
    ///
    /// * `self` (`Failure`) - Connection failure classification. Passed by value.
    ///
    /// # Returns
    ///
    /// `&'static str` - Static English message describing the failure.
    pub fn text(self) -> &'static str {
        // Choose the appropriate path for self; each arm handles one supported case.
        match self {
            // Handle the Self Missing case: apply the state-specific behavior shown here.
            Self::Missing => "The saved network is unavailable.",
            // Handle the Self NoNetworks case: apply the state-specific behavior shown here.
            Self::NoNetworks => "No Wi-Fi networks found.",
            // Handle the Self Authentication case: apply the state-specific behavior shown here.
            Self::Authentication => "Authentication failed. Check the password.",
            // Handle the Self IpTimeout case: apply the state-specific behavior shown here.
            Self::IpTimeout => "Could not obtain an IP address.",
            // Handle the Self Scan case: apply the state-specific behavior shown here.
            Self::Scan => "Wi-Fi scan failed. Please retry.",
            // Handle the Self Connection case: apply the state-specific behavior shown here.
            Self::Connection => "Could not connect. Please retry.",
            // Handle the Self Save case: apply the state-specific behavior shown here.
            Self::Save => "Could not save Wi-Fi. Please try again.",
        }
    }
}

pub enum Command {
    // Scanning failed, or a scan command requests a new access-point list.
    Scan(u64),
    // Retry the retained saved profile rather than persisting a new candidate.
    Retry(u64),
    // Connect using candidate credentials and carry the generation needed to reject stale events.
    Connect(u64, Credentials),
    // Cancel pending connection work without deleting the saved station profile.
    Suspend(u64),
    // Explicitly clear saved Wi-Fi credentials and report whether the write succeeded.
    Reset(u64),
}
pub enum Event {
    // Worker startup result containing the saved SSID when a usable profile exists.
    Ready(Option<String>),
    // Unrecoverable error requiring a visible problem page instead of continued setup.
    Fatal(&'static str),
    // Completed scan result associated with its request generation.
    Scanned(u64, Result<Vec<Network>, Failure>),
    // Successfully connected profile or the page showing current weather.
    Connected(u64, String),
    // Worker failure associated with the operation that produced it.
    Failed(u64, Failure),
    // An established IP connection has disappeared and recovery should begin.
    Lost,
    // Explicitly clear saved Wi-Fi credentials and report whether the write succeeded.
    Reset(u64, bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    // Startup page while saved credentials and hardware readiness are checked.
    Starting,
    // Recoverable connection problem page retaining setup and retry controls.
    Problem,
    // Access-point selection page with scan and pagination controls.
    Networks,
    // Candidate passphrase entry page; sensitive text is cleared on cancellation.
    Password,
    // Connection-progress page while the worker attempts association and IP readiness.
    Connecting,
    // Successfully connected profile or the page showing current weather.
    Connected,
    // Unrecoverable error requiring a visible problem page instead of continued setup.
    Fatal,
}

pub struct App {
    // Forecast cache, selected view, and refresh scheduler. Stored as crate::weather::Weather.
    pub weather: crate::weather::Weather,
    // Combined presence indication from UART reports and the OUT pin. Stored as
    // crate::radar::PresenceStatus.
    pub radar: crate::radar::PresenceStatus,
    // Editable preferences and their saved rollback baseline. Stored as
    // crate::settings::Preferences.
    pub preferences: crate::settings::Preferences,
    // Currently selected Wi-Fi navigation page. Stored as Page.
    pub page: Page,
    // Name of the successfully persisted station profile, if one exists. Stored as Option<String>.
    pub saved_ssid: Option<String>,
    // Access point currently selected for candidate setup. Stored as Option<Network>.
    pub selected: Option<Network>,
    // Nearby access points available for selection. Stored as Vec<Network>.
    pub networks: Vec<Network>,
    // Wi-Fi passphrase stored in a buffer that is cleared when no longer needed. Stored as
    // Zeroizing<String>.
    pub password: Zeroizing<String>,
    // Whether entered password characters are visible instead of masked. Stored as bool.
    pub show_password: bool,
    // User-facing status or error text. Stored as String.
    pub message: String,
    // Current starting position within a paginated list. Stored as usize.
    pub offset: usize,
    // Selected on-screen keyboard layout. Stored as u8.
    pub keyboard: u8,
    // Whether an asynchronous access-point scan is pending. Stored as bool.
    pub scanning: bool,
    // Request identifier used to reject results belonging to cancelled or replaced work. Stored as
    // u64.
    pub generation: u64,
    // Next application reconnection time in monotonic seconds, if scheduled. Stored as Option<u64>.
    pub retry_at: Option<u64>,
}
impl Default for App {
    /// Creates the initial application state for startup.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `Self` - Application on the Starting page with default preferences and no selected
    /// network or password.
    fn default() -> Self {
        Self {
            // Initialize forecast cache, selected view, and refresh scheduler from the supplied
            // value.
            weather: Default::default(),
            // Initialize combined presence indication from UART reports and the OUT pin from the
            // supplied value.
            radar: Default::default(),
            // Initialize editable preferences and their saved rollback baseline from the supplied
            // value.
            preferences: Default::default(),
            // Initialize currently selected Wi-Fi navigation page from the supplied value.
            page: Page::Starting,
            // Leave name of the successfully persisted station profile, if one exists unavailable
            // until a later operation supplies it.
            saved_ssid: None,
            // Leave access point currently selected for candidate setup unavailable until a later
            // operation supplies it.
            selected: None,
            // Initialize nearby access points available for selection from the supplied value.
            networks: vec![],
            // Initialize Wi-Fi passphrase stored in a buffer that is cleared when no longer needed
            // from the supplied value.
            password: Zeroizing::new(String::new()),
            // Initialize whether entered password characters are visible instead of masked as
            // disabled/inactive.
            show_password: false,
            // Initialize user-facing status or error text from the supplied value.
            message: "Starting Wi-Fi...".into(),
            // Start current starting position within a paginated list at zero; later operations
            // update it as needed.
            offset: 0,
            // Start selected on-screen keyboard layout at zero; later operations update it as
            // needed.
            keyboard: 0,
            // Initialize whether an asynchronous access-point scan is pending as disabled/inactive.
            scanning: false,
            // Start request identifier used to reject results belonging to cancelled or replaced
            // work at zero; later operations update it as needed.
            generation: 0,
            // Leave next application reconnection time in monotonic seconds, if scheduled
            // unavailable until a later operation supplies it.
            retry_at: None,
        }
    }
}
impl App {
    /// Advances the generation used to reject obsolete connection events.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut App`) - Application navigation, connection generation, and protected
    ///   password state. Mutated in place.
    ///
    /// # Returns
    ///
    /// `u64` - New request generation identifier.
    fn next(&mut self) -> u64 {
        // Advance the request generation so later events from replaced work can be recognized and
        // ignored.
        self.generation += 1;
        // Return request identifier used to reject results belonging to cancelled or replaced work
        // as the value of this block.
        self.generation
    }
    /// Applies worker events and schedules follow-up commands while rejecting stale
    /// generations.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut App`) - Application navigation, connection generation, and protected
    ///   password state. Mutated in place.
    /// * `event` (`Event`) - Owned connection worker event; generation checks reject obsolete
    ///   results.
    /// * `now` (`u64`) - Monotonic application time in seconds; uses the retry scheduler's time
    ///   origin.
    ///
    /// # Returns
    ///
    /// `Option<Command>` - Some follow-up command when needed; None for local updates, ignored
    /// events, or no further work.
    pub fn handle(&mut self, event: Event, now: u64) -> Option<Command> {
        // Once an unrecoverable error is visible, ordinary connection events must not replace that
        // diagnostic page.
        if self.page == Page::Fatal && !matches!(&event, Event::Fatal(_)) {
            // Leave this function now with no available value; later statements are skipped.
            return None;
        }
        // Choose the appropriate path for asynchronous connection result delivered to the
        // application; each arm handles one supported case.
        match event {
            // Handle the Event Reset(id, success) if id == self.generation case: apply the
            // state-specific behavior shown here.
            Event::Reset(id, success) if id == self.generation => {
                // Set reset pending to the disabled state.
                self.preferences.reset_pending = false;
                // Check whether whether the required operation completed without error.
                if success {
                    // Set name of the successfully persisted station profile, if one exists to no
                    // available value.
                    self.saved_ssid = None;
                    // Set access point currently selected for candidate setup to no available
                    // value.
                    self.selected = None;
                    // Zeroizes the entered password and disables password visibility.
                    self.clear_secret();
                    // Execute leave for editable preferences and their saved rollback baseline.
                    self.preferences.leave(None);
                    // Set currently selected Wi-Fi navigation page to Problem state.
                    self.page = Page::Problem;
                    // Set next application reconnection time in monotonic seconds, if scheduled to
                    // no available value.
                    self.retry_at = None;
                    // Set user-facing status or error text to into result for the surrounding
                    // operation.
                    self.message = "No Wi-Fi network is configured.".into();
                    // Set whether an asynchronous access-point scan is pending to the enabled
                    // state.
                    self.scanning = true;
                    // Leave this function now with an available value for Scan result for the
                    // surrounding operation; later statements are skipped.
                    return Some(Command::Scan(self.next()));
                }
                // Set status displayed to the reader instead of silent failure to the specified
                // message, format, or data literal.
                self.preferences.status = "Wi-Fi reset failed. Please retry.";
                // A previously saved profile is available, allowing a reconnect rather than
                // requiring new credentials.
                if self.saved_ssid.is_some() {
                    // Set currently selected Wi-Fi navigation page to Problem state.
                    self.page = Page::Problem;
                    // Set next application reconnection time in monotonic seconds, if scheduled to
                    // an available value for monotonic time used to compare activity and request
                    // deadlines plus RETRY SECS.
                    self.retry_at = Some(now + RETRY_SECS);
                }
            }
            // Handle the Event Ready(ssid) case: apply the state-specific behavior shown here.
            Event::Ready(ssid) => {
                // Set name of the successfully persisted station profile, if one exists to Wi-Fi
                // network name, limited to 32 encoded bytes by station configuration.
                self.saved_ssid = ssid;
                // A previously saved profile is available, allowing a reconnect rather than
                // requiring new credentials.
                if self.saved_ssid.is_some() {
                    // Leave this function now with an available value for starts reconnecting with
                    // the saved network profile; later statements are skipped.
                    return Some(self.retry());
                }
                // Set currently selected Wi-Fi navigation page to Problem state.
                self.page = Page::Problem;
                // Set user-facing status or error text to into result for the surrounding
                // operation.
                self.message = "No Wi-Fi network is configured.".into();
                // Set whether an asynchronous access-point scan is pending to the enabled state.
                self.scanning = true;
                // Keep request generation carried by a command or response in this local variable
                // for the following operations.
                let id = self.next();
                // Leave this function now with an available value for Scan result for the
                // surrounding operation; later statements are skipped.
                return Some(Command::Scan(id));
            }
            // Handle the Event Fatal(text) case: apply the state-specific behavior shown here.
            Event::Fatal(text) => {
                // Zeroizes the entered password and disables password visibility.
                self.clear_secret();
                // Advances the generation used to reject obsolete connection events.
                self.next();
                // Set currently selected Wi-Fi navigation page to Fatal state.
                self.page = Page::Fatal;
                // Set user-facing status or error text to into result for the surrounding
                // operation.
                self.message = text.into();
                // Set next application reconnection time in monotonic seconds, if scheduled to no
                // available value.
                self.retry_at = None;
            }
            // Handle the Event Scanned(id, result) if id == self.generation case: apply the
            // state-specific behavior shown here.
            Event::Scanned(id, result) if id == self.generation => {
                // Set whether an asynchronous access-point scan is pending to the disabled state.
                self.scanning = false;
                // Choose the appropriate path for success or failure produced by the operation,
                // retained for later handling; each arm handles one supported case.
                match result {
                    // Continue with the successful result, using its validated value in this case.
                    Ok(mut networks) => {
                        // Keep only entries that satisfy the predicate, discarding unusable values.
                        networks.retain(|n| !n.ssid.is_empty());
                        // Order access points by descending signal strength so the strongest
                        // choices appear first.
                        networks.sort_by_key(|n| std::cmp::Reverse(n.rssi));
                        // Same name and security represent the same selectable network.
                        // Keep access-point list retaining only one entry for each SSID/security
                        // pair in this local variable for the following operations.
                        let mut unique: Vec<Network> = vec![];
                        // Visit each entry in nearby access points available for selection; the
                        // loop binding provides its value or index for this iteration.
                        for n in networks {
                            // Keep one selectable entry per SSID/security combination even if
                            // several radios advertise it.
                            if !unique
                                .iter()
                                .any(|v| v.ssid == n.ssid && v.security == n.security)
                            {
                                // Append the new entry to the collection, preserving the order in
                                // which values arrive.
                                unique.push(n);
                            }
                        }
                        // Set nearby access points available for selection to access-point list
                        // retaining only one entry for each SSID/security pair.
                        self.networks = unique;
                        // Set current starting position within a paginated list to 0.
                        self.offset = 0;
                        // Update the scan-selection message only when the user is viewing the
                        // network list.
                        if self.page == Page::Networks {
                            // Set user-facing status or error text to the value selected by the
                            // following condition and its alternatives.
                            self.message = if self.networks.is_empty() {
                                // Produce into result for the surrounding operation.
                                Failure::NoNetworks.text().into()
                            } else {
                                // Produce into result for the surrounding operation.
                                "Select a secured 2.4 GHz network.".into()
                            };
                        // There is neither a configured profile nor a visible access point; show
                        // both facts in setup feedback.
                        } else if self.saved_ssid.is_none() && self.networks.is_empty() {
                            // Set user-facing status or error text to into result for the
                            // surrounding operation.
                            self.message =
                                "No Wi-Fi is configured. No Wi-Fi networks found.".into();
                        }
                    }
                    // Set user-facing status or error text to into result for the surrounding
                    // operation.
                    Err(reason) => self.message = reason.text().into(),
                }
            }
            // Handle the Event Connected(id, ssid) if id == self.generation case: apply the
            // state-specific behavior shown here.
            Event::Connected(id, ssid) if id == self.generation => {
                // Set name of the successfully persisted station profile, if one exists to an
                // available value for Wi-Fi network name, limited to 32 encoded bytes by station
                // configuration.
                self.saved_ssid = Some(ssid);
                // Set currently selected Wi-Fi navigation page to Connected state.
                self.page = Page::Connected;
                // Set user-facing status or error text to into result for the surrounding
                // operation.
                self.message = "Wi-Fi connected.".into();
                // Set next application reconnection time in monotonic seconds, if scheduled to no
                // available value.
                self.retry_at = None;
                // Zeroizes the entered password and disables password visibility.
                self.clear_secret();
            }
            // Handle the Event Failed(id, reason) if id == self.generation case: apply the
            // state-specific behavior shown here.
            Event::Failed(id, reason) if id == self.generation => {
                // Set user-facing status or error text to into result for the surrounding
                // operation.
                self.message = reason.text().into();
                // The failure belongs to a candidate setup attempt, so return to password entry
                // instead of silently retrying it.
                if self.selected.is_some() {
                    // Set currently selected Wi-Fi navigation page to Password state.
                    self.page = Page::Password;
                } else {
                    // Set currently selected Wi-Fi navigation page to Problem state.
                    self.page = Page::Problem;
                    // Set next application reconnection time in monotonic seconds, if scheduled to
                    // an available value for monotonic time used to compare activity and request
                    // deadlines plus RETRY SECS.
                    self.retry_at = Some(now + RETRY_SECS);
                }
            }
            // Leave this function now with an available value for starts reconnecting with the
            // saved network profile; later statements are skipped.
            Event::Lost if self.page == Page::Connected => return Some(self.retry()),
            // Handle remaining cases with the fallback, preserving safe behavior for unsupported or
            // irrelevant input.
            _ => {}
        }
        // Return no available value as the value of this block.
        None
    }
    /// Zeroizes the entered password and disables password visibility.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut App`) - Application navigation, connection generation, and protected
    ///   password state. Mutated in place.
    ///
    /// # Returns
    ///
    /// `()` - No value; clears the password buffer and visibility flag.
    fn clear_secret(&mut self) {
        // Overwrite sensitive bytes rather than merely forgetting the buffer length.
        self.password.zeroize();
        // Set whether entered password characters are visible instead of masked to the disabled
        // state.
        self.show_password = false;
    }
    /// Enters network setup and starts a fresh scan.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut App`) - Application navigation, connection generation, and protected
    ///   password state. Mutated in place.
    ///
    /// # Returns
    ///
    /// `Command` - Scan command with a new generation; clears secrets, selection, pagination,
    /// and retry state.
    pub fn setup(&mut self) -> Command {
        // Zeroizes the entered password and disables password visibility.
        self.clear_secret();
        // Set access point currently selected for candidate setup to no available value.
        self.selected = None;
        // Set next application reconnection time in monotonic seconds, if scheduled to no available
        // value.
        self.retry_at = None;
        // Set currently selected Wi-Fi navigation page to Networks state.
        self.page = Page::Networks;
        // Set whether an asynchronous access-point scan is pending to the enabled state.
        self.scanning = true;
        // Set user-facing status or error text to into result for the surrounding operation.
        self.message = "Scanning...".into();
        // Set current starting position within a paginated list to 0.
        self.offset = 0;
        // Keep request generation carried by a command or response in this local variable for the
        // following operations.
        let id = self.next();
        // Run the Scan operation with the supplied inputs.
        Command::Scan(id)
    }
    /// Starts reconnecting with the saved network profile.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut App`) - Application navigation, connection generation, and protected
    ///   password state. Mutated in place.
    ///
    /// # Returns
    ///
    /// `Command` - Retry command with a new generation; enters Connecting and clears entered
    /// secrets.
    pub fn retry(&mut self) -> Command {
        // Zeroizes the entered password and disables password visibility.
        self.clear_secret();
        // Set access point currently selected for candidate setup to no available value.
        self.selected = None;
        // Set currently selected Wi-Fi navigation page to Connecting state.
        self.page = Page::Connecting;
        // Set user-facing status or error text to into result for the surrounding operation.
        self.message = "Reconnecting...".into();
        // Set next application reconnection time in monotonic seconds, if scheduled to no available
        // value.
        self.retry_at = None;
        // Keep request generation carried by a command or response in this local variable for the
        // following operations.
        let id = self.next();
        // Run the Retry operation with the supplied inputs.
        Command::Retry(id)
    }
    /// Schedules a due background retry while retaining the problem page.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut App`) - Application navigation, connection generation, and protected
    ///   password state. Mutated in place.
    /// * `now` (`u64`) - Monotonic application time in seconds; uses the retry scheduler's time
    ///   origin.
    ///
    /// # Returns
    ///
    /// `Option<Command>` - Some retry command when a saved profile's retry deadline is reached;
    /// None otherwise.
    pub fn tick(&mut self, now: u64) -> Option<Command> {
        // A recoverable problem with saved credentials has reached its thirty-second retry
        // deadline.
        if self.page == Page::Problem
            && self.saved_ssid.is_some()
            && self.retry_at.is_some_and(|t| now >= t)
        {
            // Background retries retain the problem page and its setup button.
            // Set next application reconnection time in monotonic seconds, if scheduled to no
            // available value.
            self.retry_at = None;
            // Keep request generation carried by a command or response in this local variable for
            // the following operations.
            let id = self.next();
            // Leave this function now with an available value for Retry result for the surrounding
            // operation; later statements are skipped.
            return Some(Command::Retry(id));
        }
        // Return no available value as the value of this block.
        None
    }
    /// Selects a supported scanned network and opens password entry.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut App`) - Application navigation, connection generation, and protected
    ///   password state. Mutated in place.
    /// * `index` (`usize`) - Zero-based index into the scanned network list; out-of-range
    ///   selections are ignored.
    ///
    /// # Returns
    ///
    /// `()` - No value; ignores out-of-range indices and displays a warning for unsupported
    /// security.
    pub fn select(&mut self, index: usize) {
        // Only process a selection that still refers to an existing scan entry.
        if let Some(network) = self.networks.get(index) {
            // Do not accept open, WEP, enterprise, or unrecognized security as a supported personal
            // network.
            if network.security == Security::Unsupported {
                // Set user-facing status or error text to into result for the surrounding
                // operation.
                self.message = "Open, WEP and enterprise Wi-Fi are unsupported.".into();
                // Leave this function now after completing the required side effects; later
                // statements are skipped.
                return;
            }
            // Set access point currently selected for candidate setup to an available value for an
            // owned copy of scanned access-point identity and security information.
            self.selected = Some(network.clone());
            // Zeroizes the entered password and disables password visibility.
            self.clear_secret();
            // Set currently selected Wi-Fi navigation page to Password state.
            self.page = Page::Password;
            // Set selected on-screen keyboard layout to 0.
            self.keyboard = 0;
            // Set user-facing status or error text to into result for the surrounding operation.
            self.message = "Enter the Wi-Fi password.".into();
        }
    }
    /// Validates entered credentials and starts a candidate connection.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut App`) - Application navigation, connection generation, and protected
    ///   password state. Mutated in place.
    ///
    /// # Returns
    ///
    /// `Option<Command>` - Some connection command after validation; None if selection is
    /// absent or invalid. Validation failures update the message.
    pub fn connect(&mut self) -> Option<Command> {
        // Keep validated candidate network name, security mode, and protected passphrase in this
        // local variable for the following operations.
        let credentials = Credentials::new(self.selected.as_ref()?, &self.password);
        // Choose the appropriate path for validated candidate network name, security mode, and
        // protected passphrase; each arm handles one supported case.
        match credentials {
            // Continue with the successful result, using its validated value in this case.
            Ok(c) => {
                // Zeroizes the entered password and disables password visibility.
                self.clear_secret();
                // Set currently selected Wi-Fi navigation page to Connecting state.
                self.page = Page::Connecting;
                // Set user-facing status or error text to into result for the surrounding
                // operation.
                self.message = "Connecting...".into();
                // Keep request generation carried by a command or response in this local variable
                // for the following operations.
                let id = self.next();
                // Return an available Connect result for the surrounding operation.
                Some(Command::Connect(id, c))
            }
            // Handle a failed operation here rather than treating its value as valid.
            Err(text) => {
                // Set user-facing status or error text to into result for the surrounding
                // operation.
                self.message = text.into();
                // Return no available value as the value of this block.
                None
            }
        }
    }
    /// Cancels setup work, clears secrets, and returns to the preceding page.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut App`) - Application navigation, connection generation, and protected
    ///   password state. Mutated in place.
    ///
    /// # Returns
    ///
    /// `Command` - Suspend command with a new generation, invalidating pending connection work.
    pub fn back(&mut self) -> Command {
        // Zeroizes the entered password and disables password visibility.
        self.clear_secret();
        // Cancel candidate password entry and return to the network list, clearing the selection as
        // it is consumed.
        if self.selected.take().is_some() {
            // Set currently selected Wi-Fi navigation page to Networks state.
            self.page = Page::Networks;
            // Set user-facing status or error text to into result for the surrounding operation.
            self.message = "Select a secured 2.4 GHz network.".into();
        } else {
            // Set currently selected Wi-Fi navigation page to Problem state.
            self.page = Page::Problem;
            // Set user-facing status or error text to the value selected by the following condition
            // and its alternatives.
            self.message = if self.saved_ssid.is_some() {
                // Produce into result for the surrounding operation.
                "Wi-Fi setup cancelled.".into()
            } else {
                // Produce into result for the surrounding operation.
                "No Wi-Fi network is configured.".into()
            };
            // Set next application reconnection time in monotonic seconds, if scheduled to an
            // available value for 0.
            self.retry_at = Some(0);
        }
        // Set whether an asynchronous access-point scan is pending to the disabled state.
        self.scanning = false;
        // Keep request generation carried by a command or response in this local variable for the
        // following operations.
        let id = self.next();
        // Run the Suspend operation with the supplied inputs.
        Command::Suspend(id)
    }
    /// Appends a printable ASCII character while enforcing the password limit.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut App`) - Application navigation, connection generation, and protected
    ///   password state. Mutated in place.
    /// * `ch` (`char`) - Unicode scalar value to process.
    ///
    /// # Returns
    ///
    /// `()` - No value; ignores unsupported characters and input beyond 63 bytes.
    pub fn type_char(&mut self, ch: char) {
        // Append only printable ASCII while the passphrase remains shorter than 63 bytes.
        if ch.is_ascii() && !ch.is_ascii_control() && self.password.len() < 63 {
            // Append the new entry to the collection, preserving the order in which values arrive.
            self.password.push(ch);
        }
    }
}

///////////////////////////////////////////////////////////////////////////////////////
//                                    UNIT TESTS                                     //
///////////////////////////////////////////////////////////////////////////////////////

/// Unit tests and supporting fixtures for model behavior.
#[cfg(test)]
mod tests {
    use super::*;
    /// Builds a supported synthetic Wi-Fi network for model tests.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `Network` - WPA2 network fixture used to exercise selection and credential validation.
    fn network() -> Network {
        Network {
            // Initialize Wi-Fi network name, limited to 32 encoded bytes by station configuration
            // from the supplied value.
            ssid: "Home".into(),
            // Initialize supported Wi-Fi authentication mode from the supplied value.
            security: Security::Wpa2,
            // Initialize received signal strength in dBm; less-negative values represent stronger
            // signals from the supplied value.
            rssi: -30,
        }
    }
    /// Verifies rejection of short or invalid passwords and acceptance of supported printable
    /// passphrases.
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
    fn password_validation() {
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(Credentials::new(&network(), "short").is_err());
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(Credentials::new(&network(), &"a".repeat(64)).is_err());
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(Credentials::new(&network(), "password\n").is_err());
        // Verify the operation succeeded. A violation means the tested behavior is incorrect.
        assert!(Credentials::new(&network(), "secret123").is_ok());
    }
    /// Verifies startup without saved credentials and the distinct model messages for empty
    /// scans and scan errors.
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
    fn no_configuration_empty_scan_and_scan_failure() {
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App::default();
        // Verify the formatted text or fixture created by matches. A violation means the tested
        // behavior is incorrect.
        assert!(matches!(
            app.handle(Event::Ready(None), 0),
            Some(Command::Scan(_))
        ));
        // Applies worker events and schedules follow-up commands while rejecting stale generations.
        app.handle(Event::Scanned(app.generation, Ok(vec![])), 0);
        // Verify the value falls within the stated range or belongs to the supported set. A
        // violation means the tested behavior is incorrect.
        assert!(app.message.contains("No Wi-Fi networks"));
        // Applies worker events and schedules follow-up commands while rejecting stale generations.
        app.handle(Event::Scanned(app.generation, Err(Failure::Scan)), 0);
        // Verify that user-facing status or error text exactly matches maps a connection failure to
        // its user-facing message.
        assert_eq!(app.message, Failure::Scan.text());
    }
    /// Verifies that failed candidate setup retains the saved network and cancellation clears
    /// the entered password.
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
    fn failures_preserve_saved_network_and_cancel_clears_secret() {
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App::default();
        // Applies worker events and schedules follow-up commands while rejecting stale generations.
        app.handle(Event::Ready(Some("Old".into())), 0);
        // Enters network setup and starts a fresh scan.
        app.setup();
        // Set nearby access points available for selection to the formatted text or fixture created
        // by vec.
        app.networks = vec![network()];
        // Selects a supported scanned network and opens password entry.
        app.select(0);
        // Append the supplied text to the existing string without replacing its earlier contents.
        app.password.push_str("secret123");
        // Validates entered credentials and starts a candidate connection.
        app.connect();
        // Applies worker events and schedules follow-up commands while rejecting stale generations.
        app.handle(Event::Failed(app.generation, Failure::Authentication), 20);
        // Verify that currently selected Wi-Fi navigation page exactly matches Password state.
        assert_eq!(app.page, Page::Password);
        // Verify that as deref result for the surrounding operation exactly matches an available
        // value for the specified message, format, or data literal.
        assert_eq!(app.saved_ssid.as_deref(), Some("Old"));
        // Verify Wi-Fi passphrase stored in a buffer that is cleared when no longer needed is
        // empty. A violation means the tested behavior is incorrect.
        assert!(app.password.is_empty());
        // Append the supplied text to the existing string without replacing its earlier contents.
        app.password.push_str("secret123");
        // Set whether entered password characters are visible instead of masked to the enabled
        // state.
        app.show_password = true;
        // Cancels setup work, clears secrets, and returns to the preceding page.
        app.back();
        // Verify Wi-Fi passphrase stored in a buffer that is cleared when no longer needed is
        // empty. A violation means the tested behavior is incorrect.
        assert!(app.password.is_empty());
        // Verify the inverse of whether entered password characters are visible instead of masked.
        // A violation means the tested behavior is incorrect.
        assert!(!app.show_password);
    }
    /// Verifies delayed background retries and rejection of events from obsolete request
    /// generations.
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
    fn reconnect_delay_background_retry_and_stale_events() {
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App::default();
        // Applies worker events and schedules follow-up commands while rejecting stale generations.
        app.handle(Event::Ready(Some("Home".into())), 0);
        // Applies worker events and schedules follow-up commands while rejecting stale generations.
        app.handle(Event::Failed(app.generation, Failure::Missing), 20);
        // Verify schedules a due background retry while retaining the problem page is unavailable.
        // A violation means the tested behavior is incorrect.
        assert!(app.tick(49).is_none());
        // Verify the formatted text or fixture created by matches. A violation means the tested
        // behavior is incorrect.
        assert!(matches!(app.tick(50), Some(Command::Retry(_))));
        // Verify that currently selected Wi-Fi navigation page exactly matches Problem state.
        assert_eq!(app.page, Page::Problem);
        // Keep earlier request generation deliberately retained to test obsolete-result rejection
        // in this local variable for the following operations.
        let stale = app.generation;
        // Enters network setup and starts a fresh scan.
        app.setup();
        // Applies worker events and schedules follow-up commands while rejecting stale generations.
        app.handle(Event::Connected(stale, "Home".into()), 51);
        // Verify that currently selected Wi-Fi navigation page exactly matches Networks state.
        assert_eq!(app.page, Page::Networks);
    }
    /// Verifies model transitions after a successful connection and a later link-loss event.
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
    fn successful_connection_and_loss() {
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App::default();
        // Applies worker events and schedules follow-up commands while rejecting stale generations.
        app.handle(Event::Ready(Some("Home".into())), 0);
        // Applies worker events and schedules follow-up commands while rejecting stale generations.
        app.handle(Event::Connected(app.generation, "Home".into()), 2);
        // Verify that currently selected Wi-Fi navigation page exactly matches Connected state.
        assert_eq!(app.page, Page::Connected);
        // Verify the formatted text or fixture created by matches. A violation means the tested
        // behavior is incorrect.
        assert!(matches!(
            app.handle(Event::Lost, 3),
            Some(Command::Retry(_))
        ));
        // Verify that currently selected Wi-Fi navigation page exactly matches Connecting state.
        assert_eq!(app.page, Page::Connecting);
    }
    /// Verifies sorted, deduplicated scan choices and the maximum printable password input
    /// length.
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
    fn sorted_deduplicated_and_password_bounds() {
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App::default();
        // Enters network setup and starts a fresh scan.
        app.setup();
        // Keep network fixture with a lower signal level used to verify sorting in this local
        // variable for the following operations.
        let mut weak = network();
        // Set received signal strength in dBm to 80.
        weak.rssi = -80;
        // Applies worker events and schedules follow-up commands while rejecting stale generations.
        app.handle(Event::Scanned(app.generation, Ok(vec![weak, network()])), 0);
        // Verify that the length of nearby access points available for selection exactly matches 1.
        assert_eq!(app.networks.len(), 1);
        // Verify that received signal strength in dBm exactly matches 30.
        assert_eq!(app.networks[0].rssi, -30);
        // Visit each entry in 0..100; the loop binding provides its value or index for this
        // iteration.
        for _ in 0..100 {
            // Appends a printable ASCII character while enforcing the password limit.
            app.type_char('a');
        }
        // Appends a printable ASCII character while enforcing the password limit.
        app.type_char('\n');
        // Verify that the length of Wi-Fi passphrase stored in a buffer that is cleared when no
        // longer needed exactly matches 63.
        assert_eq!(app.password.len(), 63);
        // Applies worker events and schedules follow-up commands while rejecting stale generations.
        app.handle(Event::Fatal("Storage unavailable"), 0);
        // Verify Wi-Fi passphrase stored in a buffer that is cleared when no longer needed is
        // empty. A violation means the tested behavior is incorrect.
        assert!(app.password.is_empty());
        // Applies worker events and schedules follow-up commands while rejecting stale generations.
        app.handle(Event::Ready(Some("Home".into())), 1);
        // Verify that currently selected Wi-Fi navigation page exactly matches Fatal state.
        assert_eq!(app.page, Page::Fatal);
    }
}
