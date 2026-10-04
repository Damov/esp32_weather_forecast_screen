// ============================================================================= //
// File          : service.rs                                                    //
// License       : GPL-3.0-only                                                  //
// Created       : 2026-10-04                                                    //
// Last modified : 2026-10-04                                                    //
// Author        : Daniel Kharlamov                                              //
// Last editor   : Daniel Kharlamov <daniel.kharlamov@googlemail.com>            //
// ============================================================================= //
//                                                                               //
// Summary:                                                                      //
// --------                                                                      //
// Hardware-independent Wi-Fi connection service.                                //
//                                                                               //
// Module:                                                                       //
// -------                                                                       //
// Defines backend and credential storage interfaces and drives scanning,        //
// connection, recovery, and cancellation with monotonic time. Saves candidate   //
// credentials only after an IP connection succeeds and reports authentication,  //
// network, timeout, and storage failures. Tests the lifecycle with a simulated  //
// backend.                                                                      //
// ============================================================================= //

//! Hardware-independent connection worker; time is monotonic milliseconds.
use crate::model::{Command, Credentials, Event, Failure, Network, CONNECT_TIMEOUT_SECS};

pub trait Backend: CredentialStore {
    /// Cancels current scanning or connection work without erasing persisted credentials.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Backend`) - Receiver state used by this operation. Mutated in place.
    ///
    /// # Returns
    ///
    /// `()` - No value; resets transient backend connection state.
    fn clear(&mut self);
    /// Starts a nonblocking network scan.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Backend`) - Receiver state used by this operation. Mutated in place.
    ///
    /// # Returns
    ///
    /// `Result<(), Failure>` - Ok when scanning starts; Err with the backend scan failure.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend cannot start scanning.
    fn begin_scan(&mut self) -> Result<(), Failure>;
    /// Checks whether an asynchronous scan has completed.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Backend`) - Receiver state used by this operation. Mutated in place.
    ///
    /// # Returns
    ///
    /// `Result<Option<Vec<Network>>, Failure>` - Ok(Some(networks)) on completion, Ok(None)
    /// while pending, or Err for scan failure.
    ///
    /// # Errors
    ///
    /// Returns an error when retrieving or completing the scan fails.
    fn scan_result(&mut self) -> Result<Option<Vec<Network>>, Failure>;
    /// Applies candidate credentials without persisting them.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Backend`) - Receiver state used by this operation. Mutated in place.
    /// * `c` (`&Credentials`) - Validated candidate credentials to configure or persist; never
    ///   log their password.
    ///
    /// # Returns
    ///
    /// `Result<(), Failure>` - Ok when the RAM connection profile is ready; Err with a
    /// configuration failure.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend cannot configure the supplied credentials.
    fn configure(&mut self, c: &Credentials) -> Result<(), Failure>;
    /// Requests association with the configured network.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Backend`) - Receiver state used by this operation. Mutated in place.
    ///
    /// # Returns
    ///
    /// `Result<(), Failure>` - Ok when the attempt is accepted; Err for a connection-start
    /// failure.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend cannot start the connection attempt.
    fn connect(&mut self) -> Result<(), Failure>;
    /// Checks whether the station is associated with an access point.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Backend`) - Receiver state used by this operation. Borrowed without changing
    ///   it.
    ///
    /// # Returns
    ///
    /// `bool` - True for an active association, which does not necessarily imply an IP address.
    fn associated(&self) -> bool;
    /// Checks whether the network has an IP connection.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Backend`) - Receiver state used by this operation. Borrowed without changing
    ///   it.
    ///
    /// # Returns
    ///
    /// `bool` - True when the backend considers the station ready for network traffic.
    fn up(&self) -> bool;
    /// Checks whether the current attempt has reported an authentication failure.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Backend`) - Receiver state used by this operation. Borrowed without changing
    ///   it.
    ///
    /// # Returns
    ///
    /// `bool` - True when credential authentication has failed; false otherwise.
    fn authentication_failed(&self) -> bool;
    /// Checks whether the access point is unavailable.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Backend`) - Receiver state used by this operation. Borrowed without changing
    ///   it.
    ///
    /// # Returns
    ///
    /// `bool` - False by default; implementations may report a missing access point.
    fn network_missing(&self) -> bool {
        false
    }
}
pub trait CredentialStore {
    /// Loads previously persisted station credentials.
    ///
    /// # Arguments
    ///
    /// * `self` (`&CredentialStore`) - Receiver state used by this operation. Borrowed without
    ///   changing it.
    ///
    /// # Returns
    ///
    /// `Result<Option<Credentials>, &'static str>` - Ok(Some(credentials)) for a saved profile,
    /// Ok(None) for no profile, or Err for a storage or validation failure.
    ///
    /// # Errors
    ///
    /// Returns an error for unreadable or invalid persisted credentials.
    fn load(&self) -> Result<Option<Credentials>, &'static str>;
    /// Persists credentials after a successful IP connection.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut CredentialStore`) - Receiver state used by this operation. Mutated in
    ///   place.
    /// * `c` (`&Credentials`) - Validated candidate credentials to configure or persist; never
    ///   log their password.
    ///
    /// # Returns
    ///
    /// `Result<(), &'static str>` - Ok after persistence; Err with the storage failure message.
    ///
    /// # Errors
    ///
    /// Returns an error if the credentials cannot be persisted.
    fn save(&mut self, c: &Credentials) -> Result<(), &'static str>;
    /// Removes the saved station profile.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut CredentialStore`) - Receiver state used by this operation. Mutated in
    ///   place.
    ///
    /// # Returns
    ///
    /// `Result<(), &'static str>` - Ok when the profile is cleared; Err when storage cannot be
    /// updated.
    ///
    /// # Errors
    ///
    /// Returns an error if the saved profile cannot be removed.
    fn forget(&mut self) -> Result<(), &'static str>;
}
enum Job {
    // Scanning failed, or a scan command requests a new access-point list.
    Scan {
        // Request generation carried by a command or response. Stored as u64.
        id: u64,
        // Monotonic time at which the pending operation expires. Stored as u64.
        deadline: u64,
    },
    // Connect using candidate credentials and carry the generation needed to reject stale events.
    Connect {
        // Request generation carried by a command or response. Stored as u64.
        id: u64,
        // New credentials awaiting successful IP readiness before persistence. Stored as
        // Option<Credentials>.
        candidate: Option<Credentials>,
        // Monotonic time at which the pending operation expires. Stored as u64.
        deadline: u64,
        // Whether a reconnect job is still finding its saved access point. Stored as bool.
        scanning: bool,
        // Scan or configuration failure retained until the full recovery deadline. Stored as
        // Option<Failure>.
        unavailable: Option<Failure>,
        // Monotonic millisecond time for the next association attempt. Stored as u64.
        next_attempt: u64,
        // Whether the connection ever associated during this job, distinguishing DHCP timeout.
        // Stored as bool.
        associated: bool,
    },
}
pub struct Service<B> {
    // Adapter providing network operations and credential persistence. Stored as B.
    backend: B,
    // Validated persisted credentials retained for reconnects and unaffected by failed candidates.
    // Stored as Option<Credentials>.
    saved: Option<Credentials>,
    // Optional asynchronous Scan or Connect job being advanced by poll. Stored as Option<Job>.
    job: Option<Job>,
    // Whether a completed service job has reached IP readiness. Stored as bool.
    connected: bool,
}
impl<B: Backend> Service<B> {
    /// Creates a connection service and loads its saved credentials.
    ///
    /// # Arguments
    ///
    /// * `backend` (`B`) - Owned network and credential-store implementation used by the
    ///   connection service.
    ///
    /// # Type Parameters
    ///
    /// * `B` - Network backend implementing Backend and CredentialStore, retained by the service.
    ///
    /// # Returns
    ///
    /// `Result<Self, &'static str>` - Ok with an idle service; Err from the backend credential
    /// loader.
    ///
    /// # Errors
    ///
    /// Returns the storage or validation failure reported while loading credentials.
    pub fn new(backend: B) -> Result<Self, &'static str> {
        // Keep validated persisted credentials retained for reconnects and unaffected by failed
        // candidates in this local variable for the following operations.
        let saved = backend.load()?;
        // Return success; the caller receives a constructed Self value using these fields.
        Ok(Self {
            // Initialize adapter providing network operations and credential persistence from the
            // supplied value.
            backend,
            // Initialize validated persisted credentials retained for reconnects and unaffected by
            // failed candidates from the supplied value.
            saved,
            // Leave optional asynchronous Scan or Connect job being advanced by poll unavailable
            // until a later operation supplies it.
            job: None,
            // Initialize whether a completed service job has reached IP readiness as
            // disabled/inactive.
            connected: false,
        })
    }
    /// Checks the service connection flag and current backend readiness.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Self`) - Receiver state used by this operation. Borrowed without changing
    ///   it.
    ///
    /// # Type Parameters
    ///
    /// * `B` - Network backend implementing Backend and CredentialStore, retained by the service.
    ///
    /// # Returns
    ///
    /// `bool` - True only when the service has connected and the backend is still up.
    pub fn connected(&self) -> bool {
        // Return whether a completed service job has reached IP readiness and checks whether the
        // network has an IP connection as the value of this block.
        self.connected && self.backend.up()
    }
    /// Builds the startup event describing the saved network.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Self`) - Receiver state used by this operation. Borrowed without changing
    ///   it.
    ///
    /// # Type Parameters
    ///
    /// * `B` - Network backend implementing Backend and CredentialStore, retained by the service.
    ///
    /// # Returns
    ///
    /// `Event` - Ready event containing the saved SSID, or None when no profile exists.
    pub fn ready(&self) -> Event {
        // Run the Ready operation with the supplied inputs.
        Event::Ready(self.saved.as_ref().map(|c| c.ssid.clone()))
    }
    /// Cancels prior work and starts the requested scan, retry, connection, reset, or
    /// suspension.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Self`) - Receiver state used by this operation. Mutated in place.
    /// * `command` (`Command`) - Owned Wi-Fi command, including its request generation and any
    ///   candidate credentials.
    /// * `now` (`u64`) - Monotonic timestamp in milliseconds, using the same origin as stored
    ///   deadlines.
    ///
    /// # Type Parameters
    ///
    /// * `B` - Network backend implementing Backend and CredentialStore, retained by the service.
    ///
    /// # Returns
    ///
    /// `Option<Event>` - Some immediate event for completion or failure; None when work is
    /// pending or suspended.
    pub fn command(&mut self, command: Command, now: u64) -> Option<Event> {
        // Cancels current scanning or connection work without erasing persisted credentials.
        self.backend.clear();
        // Set whether a completed service job has reached IP readiness to the disabled state.
        self.connected = false;
        // Replace or clear the active asynchronous job, preventing completed or cancelled work from
        // being processed again.
        self.job = None;
        // Choose the appropriate path for operation requested from the Wi-Fi worker; each arm
        // handles one supported case.
        match command {
            // Handle the Command Reset(id) case: apply the state-specific behavior shown here.
            Command::Reset(id) => {
                // Keep whether the required operation completed without error in this local
                // variable for the following operations.
                let success = self.backend.forget().is_ok();
                // Check whether whether the required operation completed without error.
                if success {
                    // Set validated persisted credentials retained for reconnects and unaffected by
                    // failed candidates to no available value.
                    self.saved = None;
                }
                // Return an available Reset result for the surrounding operation.
                Some(Event::Reset(id, success))
            }
            // Handle the Command Suspend( ) case: apply the state-specific behavior shown here.
            Command::Suspend(_) => None,
            // Choose the appropriate path for starts a nonblocking network scan; each arm handles
            // one supported case.
            Command::Scan(id) => match self.backend.begin_scan() {
                // Continue with the successful result, using its validated value in this case.
                Ok(()) => {
                    // Replace or clear the active asynchronous job, preventing completed or
                    // cancelled work from being processed again.
                    self.job = Some(Job::Scan {
                        // Initialize request generation carried by a command or response from the
                        // supplied value.
                        id,
                        // Initialize monotonic time at which the pending operation expires from the
                        // supplied value.
                        deadline: now + 10_000,
                    });
                    // Return no available value as the value of this block.
                    None
                }
                // Handle a failed operation here rather than treating its value as valid.
                Err(f) => Some(Event::Scanned(id, Err(f))),
            },
            // Handle the Command Retry(id) case: apply the state-specific behavior shown here.
            Command::Retry(id) => {
                // A retry needs a retained profile; fail immediately if no saved credentials exist.
                if self.saved.is_none() {
                    // Leave this function now with an available value for Failed result for the
                    // surrounding operation; later statements are skipped.
                    return Some(Event::Failed(id, Failure::Connection));
                }
                // The asynchronous scan could not start; return an immediate failure event for this
                // request generation.
                if let Err(f) = self.backend.begin_scan() {
                    // Leave this function now with an available value for Failed result for the
                    // surrounding operation; later statements are skipped.
                    return Some(Event::Failed(id, f));
                }
                // Replace or clear the active asynchronous job, preventing completed or cancelled
                // work from being processed again.
                self.job = Some(Job::Connect {
                    // Initialize request generation carried by a command or response from the
                    // supplied value.
                    id,
                    // Leave new credentials awaiting successful IP readiness before persistence
                    // unavailable until a later operation supplies it.
                    candidate: None,
                    // Initialize monotonic time at which the pending operation expires from the
                    // supplied value.
                    deadline: now + CONNECT_TIMEOUT_SECS * 1000,
                    // Initialize whether a reconnect job is still finding its saved access point as
                    // enabled/active.
                    scanning: true,
                    // Leave scan or configuration failure retained until the full recovery deadline
                    // unavailable until a later operation supplies it.
                    unavailable: None,
                    // Initialize monotonic millisecond time for the next association attempt from
                    // the supplied value.
                    next_attempt: now,
                    // Initialize whether the connection ever associated during this job,
                    // distinguishing DHCP timeout as disabled/inactive.
                    associated: false,
                });
                // Return no available value as the value of this block.
                None
            }
            // Handle the Command Connect(id, c) case: apply the state-specific behavior shown here.
            Command::Connect(id, c) => {
                // Candidate RAM configuration failed before association, so no connected job should
                // be scheduled.
                if let Err(f) = self.backend.configure(&c) {
                    // Leave this function now with an available value for Failed result for the
                    // surrounding operation; later statements are skipped.
                    return Some(Event::Failed(id, f));
                }
                // Replace or clear the active asynchronous job, preventing completed or cancelled
                // work from being processed again.
                self.job = Some(Job::Connect {
                    // Initialize request generation carried by a command or response from the
                    // supplied value.
                    id,
                    // Initialize new credentials awaiting successful IP readiness before
                    // persistence from the supplied value.
                    candidate: Some(c),
                    // Initialize monotonic time at which the pending operation expires from the
                    // supplied value.
                    deadline: now + CONNECT_TIMEOUT_SECS * 1000,
                    // Initialize whether a reconnect job is still finding its saved access point as
                    // disabled/inactive.
                    scanning: false,
                    // Leave scan or configuration failure retained until the full recovery deadline
                    // unavailable until a later operation supplies it.
                    unavailable: None,
                    // Initialize monotonic millisecond time for the next association attempt from
                    // the supplied value.
                    next_attempt: now,
                    // Initialize whether the connection ever associated during this job,
                    // distinguishing DHCP timeout as disabled/inactive.
                    associated: false,
                });
                // Return no available value as the value of this block.
                None
            }
        }
    }
    /// Advances scan and connection jobs, retries attempts, and detects deadlines or link loss.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Self`) - Receiver state used by this operation. Mutated in place.
    /// * `now` (`u64`) - Monotonic timestamp in milliseconds, using the same origin as stored
    ///   deadlines.
    ///
    /// # Type Parameters
    ///
    /// * `B` - Network backend implementing Backend and CredentialStore, retained by the service.
    ///
    /// # Returns
    ///
    /// `Option<Event>` - Some completion, failure, or lost-connection event; None while no
    /// event is ready. Successful candidates are persisted only after IP readiness.
    ///
    /// # Panics
    ///
    /// Internal connection jobs require a saved or candidate profile; a violated service
    /// invariant panics.
    pub fn poll(&mut self, now: u64) -> Option<Event> {
        // Keep asynchronous connection result delivered to the application in this local variable
        // for the following operations.
        let event = match self.job.as_mut() {
            // Choose the appropriate path for checks whether an asynchronous scan has completed;
            // each arm handles one supported case.
            Some(Job::Scan { id, deadline }) => match self.backend.scan_result() {
                // Continue with the successful result, using its validated value in this case.
                Ok(Some(networks)) => Some(Event::Scanned(*id, Ok(networks))),
                // Handle a failed operation here rather than treating its value as valid.
                Err(f) => Some(Event::Scanned(*id, Err(f))),
                // Continue with the successful result, using its validated value in this case.
                Ok(None) if now >= *deadline => {
                    // Cancels current scanning or connection work without erasing persisted
                    // credentials.
                    self.backend.clear();
                    // Return an available Scanned result for the surrounding operation.
                    Some(Event::Scanned(*id, Err(Failure::Scan)))
                }
                // Continue with the successful result, using its validated value in this case.
                Ok(None) => None,
            },
            // Handle the Some(Job Connect {                 id,                 candidate,
            // deadline,                 scanning,                 unavailable,                 next
            // attempt,                 associated,             }) case: apply the state-specific
            // behavior shown here.
            Some(Job::Connect {
                id,
                candidate,
                deadline,
                scanning,
                unavailable,
                next_attempt,
                associated,
            }) => {
                // A saved-profile reconnect first scans for its access point before attempting
                // association.
                if *scanning {
                    // Choose the appropriate path for checks whether an asynchronous scan has
                    // completed; each arm handles one supported case.
                    match self.backend.scan_result() {
                        // Handle a failed operation here rather than treating its value as valid.
                        Err(f) => {
                            // Set whether a reconnect job is still finding its saved access point
                            // to the disabled state.
                            *scanning = false;
                            // Set scan or configuration failure retained until the full recovery
                            // deadline to an available value for f.
                            *unavailable = Some(f);
                        }
                        // Continue with the successful result, using its validated value in this
                        // case.
                        Ok(Some(networks)) => {
                            // Set whether a reconnect job is still finding its saved access point
                            // to the disabled state.
                            *scanning = false;
                            // Keep validated candidate profile supplied to configuration or
                            // persistence in this local variable for the following operations.
                            let c = candidate
                                .as_ref()
                                .or(self.saved.as_ref())
                                .expect("connection profile");
                            // The scan saw no access points; retain that reason until the full
                            // recovery window expires.
                            if networks.is_empty() {
                                // Set scan or configuration failure retained until the full
                                // recovery deadline to an available value for Failure NoNetworks.
                                *unavailable = Some(Failure::NoNetworks);
                            // Nearby networks exist but the requested SSID is absent; classify it
                            // as missing rather than a password failure.
                            } else if !networks.iter().any(|n| n.ssid == c.ssid) {
                                // Set scan or configuration failure retained until the full
                                // recovery deadline to an available value for Failure Missing.
                                *unavailable = Some(Failure::Missing);
                            // The selected saved-profile configuration failed; retain the reason
                            // for the job's eventual failure event.
                            } else if let Err(f) = self.backend.configure(c) {
                                // Set scan or configuration failure retained until the full
                                // recovery deadline to an available value for f.
                                *unavailable = Some(f);
                            }
                        }
                        // Continue with the successful result, using its validated value in this
                        // case.
                        Ok(None) => {}
                    }
                }
                // Update whether the connection ever associated during this job, distinguishing
                // DHCP timeout using checks whether the station is associated with an access point,
                // retaining the accumulated state for subsequent steps.
                *associated |= self.backend.associated();
                // Scanning/configuration succeeded and the station now has an IP connection; the
                // job can complete.
                if !*scanning && unavailable.is_none() && self.backend.up() {
                    // This successful connection used new credentials; consume and persist them
                    // only after IP readiness.
                    if let Some(c) = candidate.take() {
                        // Do not treat an unpersisted candidate as saved; disconnect it and report
                        // a storage failure.
                        if self.backend.save(&c).is_err() {
                            // Cancels current scanning or connection work without erasing persisted
                            // credentials.
                            self.backend.clear();
                            // Return an available Failed result for the surrounding operation.
                            Some(Event::Failed(*id, Failure::Save))
                        } else {
                            // Obtain an owned clone of saved network name encoded into the fixed
                            // 32-byte native profile field; reference-counted handles continue to
                            // share their underlying state.
                            let ssid = c.ssid.clone();
                            // Set validated persisted credentials retained for reconnects and
                            // unaffected by failed candidates to an available value for validated
                            // candidate profile supplied to configuration or persistence.
                            self.saved = Some(c);
                            // Set whether a completed service job has reached IP readiness to the
                            // enabled state.
                            self.connected = true;
                            // Return an available Connected result for the surrounding operation.
                            Some(Event::Connected(*id, ssid))
                        }
                    } else {
                        // Set whether a completed service job has reached IP readiness to the
                        // enabled state.
                        self.connected = true;
                        // Return an available Connected result for the surrounding operation.
                        Some(Event::Connected(
                            *id,
                            self.saved.as_ref().expect("saved profile").ssid.clone(),
                        ))
                    }
                // The connection recovery window expired; report the most specific known failure.
                } else if now >= *deadline {
                    // Keep reason reported when the current operation cannot complete in this local
                    // variable for the following operations.
                    let failure = unavailable.unwrap_or_else(|| {
                        // A saved-profile reconnect first scans for its access point before
                        // attempting association.
                        if *scanning {
                            Failure::Scan
                        // Authentication was explicitly rejected, which is more informative than a
                        // generic connection timeout.
                        } else if self.backend.authentication_failed() {
                            Failure::Authentication
                        // The driver reports a missing access point, so do not blame the entered
                        // passphrase.
                        } else if self.backend.network_missing() {
                            Failure::Missing
                        // Association happened but IP readiness never arrived; classify the
                        // deadline as a DHCP/IP timeout.
                        } else if *associated {
                            Failure::IpTimeout
                        } else {
                            Failure::Connection
                        }
                    });
                    // Cancels current scanning or connection work without erasing persisted
                    // credentials.
                    self.backend.clear();
                    // Return an available Failed result for the surrounding operation.
                    Some(Event::Failed(*id, failure))
                } else {
                    // Retry association only while the job remains viable, the two-second spacing
                    // has elapsed, and authentication has not been rejected.
                    if !*scanning
                        && unavailable.is_none()
                        && !self.backend.associated()
                        && now >= *next_attempt
                        && !self.backend.authentication_failed()
                    {
                        // Errors are retried inside the same deadline; no UI blocking.
                        // Perform the best-effort operation and intentionally ignore its result:
                        // Requests association with the configured network.
                        let _ = self.backend.connect();
                        // Set monotonic millisecond time for the next association attempt to
                        // monotonic milliseconds used by scan and connection recovery deadlines
                        // plus 2000.
                        *next_attempt = now + 2000;
                    }
                    // Return no available value as the value of this block.
                    None
                }
            }
            // Handle the None if self.connected && !self.backend.up() case: apply the
            // state-specific behavior shown here.
            None if self.connected && !self.backend.up() => {
                // Set whether a completed service job has reached IP readiness to the disabled
                // state.
                self.connected = false;
                // Return an available Event Lost.
                Some(Event::Lost)
            }
            // Handle remaining cases with the fallback, preserving safe behavior for unsupported or
            // irrelevant input.
            _ => None,
        };
        // Check whether asynchronous connection result delivered to the application is available.
        if event.is_some() {
            // Replace or clear the active asynchronous job, preventing completed or cancelled work
            // from being processed again.
            self.job = None;
        }
        // Return asynchronous connection result delivered to the application as the value of this
        // block.
        event
    }
}

///////////////////////////////////////////////////////////////////////////////////////
//                                    UNIT TESTS                                     //
///////////////////////////////////////////////////////////////////////////////////////

/// Unit tests and supporting fixtures for service behavior.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Security;
    struct FakeBackend {
        // Optional simulated scan result consumed by the next result query. Stored as
        // Option<Result<Vec<Network>, Failure>>.
        scan: Option<Result<Vec<Network>, Failure>>,
        // Whether the connection ever associated during this job, distinguishing DHCP timeout.
        // Stored as bool.
        associated: bool,
        // Simulated assigned-IP flag; credentials must not be saved before it becomes true. Stored
        // as bool.
        up: bool,
        // Injected authentication-failure flag used by recovery tests. Stored as bool.
        auth_failed: bool,
        // Number of association requests issued, used to verify retries. Stored as usize.
        attempts: usize,
        // Simulated native station profile with injected write failures and counters. Stored as
        // FakeStore.
        store: FakeStore,
    }
    impl Backend for FakeBackend {
        /// Clears simulated association, IP readiness, and authentication failure flags.
        ///
        /// # Arguments
        ///
        /// * `self` (`&mut FakeBackend`) - Simulated network flags, queued scan result, and native
        ///   credential store. Mutated in place.
        ///
        /// # Returns
        ///
        /// `()` - No value; retains the simulated persistent profile.
        fn clear(&mut self) {
            // Set simulated assigned-IP flag to the disabled state.
            self.up = false;
            // Set whether the connection ever associated during this job, distinguishing DHCP
            // timeout to the disabled state.
            self.associated = false;
            // Set injected authentication-failure flag used by recovery tests to the disabled
            // state.
            self.auth_failed = false;
        }
        /// Accepts a simulated scan request without changing its queued result.
        ///
        /// # Arguments
        ///
        /// * `self` (`&mut FakeBackend`) - Simulated network flags, queued scan result, and native
        ///   credential store. Mutated in place.
        ///
        /// # Returns
        ///
        /// `Result<(), Failure>` - Always Ok; no driver operation is performed.
        fn begin_scan(&mut self) -> Result<(), Failure> {
            // Return success after the required side effects are complete.
            Ok(())
        }
        /// Consumes the queued simulated scan result.
        ///
        /// # Arguments
        ///
        /// * `self` (`&mut FakeBackend`) - Simulated network flags, queued scan result, and native
        ///   credential store. Mutated in place.
        ///
        /// # Returns
        ///
        /// `Result<Option<Vec<Network>>, Failure>` - Ok(Some(networks)) for queued success,
        /// Ok(None) if absent, or Err for queued failure.
        ///
        /// # Errors
        ///
        /// Propagates the failure stored in the synthetic scan result.
        fn scan_result(&mut self) -> Result<Option<Vec<Network>>, Failure> {
            // Execute transpose for the optional value removed from its slot, preventing accidental
            // reuse.
            self.scan.take().transpose()
        }
        /// Accepts test credentials without configuring hardware.
        ///
        /// # Arguments
        ///
        /// * `self` (`&mut FakeBackend`) - Simulated network flags, queued scan result, and native
        ///   credential store. Mutated in place.
        /// * `_` (`&Credentials`, argument 2) - Candidate credentials intentionally ignored by this
        ///   simulated backend.
        ///
        /// # Returns
        ///
        /// `Result<(), Failure>` - Always Ok; the unnamed credential argument is intentionally
        /// ignored.
        fn configure(&mut self, _: &Credentials) -> Result<(), Failure> {
            // Return success after the required side effects are complete.
            Ok(())
        }
        /// Records one simulated connection attempt.
        ///
        /// # Arguments
        ///
        /// * `self` (`&mut FakeBackend`) - Simulated network flags, queued scan result, and native
        ///   credential store. Mutated in place.
        ///
        /// # Returns
        ///
        /// `Result<(), Failure>` - Always Ok; increments the attempt counter.
        fn connect(&mut self) -> Result<(), Failure> {
            // Update number of association requests issued, used to verify retries using 1,
            // retaining the accumulated state for subsequent steps.
            self.attempts += 1;
            // Return success after the required side effects are complete.
            Ok(())
        }
        /// Reads the simulated association flag.
        ///
        /// # Arguments
        ///
        /// * `self` (`&FakeBackend`) - Simulated network flags, queued scan result, and native
        ///   credential store. Borrowed without changing it.
        ///
        /// # Returns
        ///
        /// `bool` - Current association flag.
        fn associated(&self) -> bool {
            // Return whether the connection ever associated during this job, distinguishing DHCP
            // timeout as the value of this block.
            self.associated
        }
        /// Reads the simulated IP-readiness flag.
        ///
        /// # Arguments
        ///
        /// * `self` (`&FakeBackend`) - Simulated network flags, queued scan result, and native
        ///   credential store. Borrowed without changing it.
        ///
        /// # Returns
        ///
        /// `bool` - Current IP-readiness flag.
        fn up(&self) -> bool {
            // Return simulated assigned-IP flag as the value of this block.
            self.up
        }
        /// Reads the simulated authentication-failure flag.
        ///
        /// # Arguments
        ///
        /// * `self` (`&FakeBackend`) - Simulated network flags, queued scan result, and native
        ///   credential store. Borrowed without changing it.
        ///
        /// # Returns
        ///
        /// `bool` - Current authentication-failure flag.
        fn authentication_failed(&self) -> bool {
            // Return injected authentication-failure flag used by recovery tests as the value of
            // this block.
            self.auth_failed
        }
    }
    struct FakeStore {
        // Saved network name encoded into the fixed 32-byte native profile field. Stored as [u8;
        // 32].
        ssid: [u8; 32],
        // Saved passphrase encoded into the fixed 64-byte native profile field. Stored as [u8; 64].
        password: [u8; 64],
        // Whether simulated profile persistence or reset should fail. Stored as bool.
        fail: bool,
        // Record of transferred rectangles or persistence attempts used to verify side effects.
        // Stored as usize.
        writes: usize,
    }
    impl CredentialStore for FakeBackend {
        /// Clears simulated native credential arrays unless failure is injected.
        ///
        /// # Arguments
        ///
        /// * `self` (`&mut FakeBackend`) - Simulated network flags, queued scan result, and native
        ///   credential store. Mutated in place.
        ///
        /// # Returns
        ///
        /// `Result<(), &'static str>` - Ok after clearing; Err with reset failed when the store
        /// failure flag is enabled.
        ///
        /// # Errors
        ///
        /// Returns the injected reset failure without clearing arrays.
        fn forget(&mut self) -> Result<(), &'static str> {
            // Inject a storage error before replacing the simulated persisted profile, allowing
            // rollback behavior to be verified.
            if self.store.fail {
                // Leave this function now with a failure result for the surrounding operation;
                // later statements are skipped.
                return Err("reset failed");
            }
            // Overwrite all elements with the requested value, producing a predictable initial or
            // cleared state.
            self.store.ssid.fill(0);
            // Overwrite all elements with the requested value, producing a predictable initial or
            // cleared state.
            self.store.password.fill(0);
            // Return success after the required side effects are complete.
            Ok(())
        }
        /// Decodes the simulated native station credential arrays.
        ///
        /// # Arguments
        ///
        /// * `self` (`&FakeBackend`) - Simulated network flags, queued scan result, and native
        ///   credential store. Borrowed without changing it.
        ///
        /// # Returns
        ///
        /// `Result<Option<Credentials>, &'static str>` - Ok(Some(credentials)) for a valid profile,
        /// Ok(None) if empty, or Err if decoding fails.
        ///
        /// # Errors
        ///
        /// Propagates native credential validation failures.
        fn load(&self) -> Result<Option<Credentials>, &'static str> {
            // Run the decode station operation with the supplied inputs.
            crate::storage::decode_station(&self.store.ssid, &self.store.password, Security::Wpa2)
        }
        /// Records a simulated write and copies candidate credentials unless failure is injected.
        ///
        /// # Arguments
        ///
        /// * `self` (`&mut FakeBackend`) - Simulated network flags, queued scan result, and native
        ///   credential store. Mutated in place.
        /// * `c` (`&Credentials`) - Validated candidate credentials to configure or persist; never
        ///   log their password.
        ///
        /// # Returns
        ///
        /// `Result<(), &'static str>` - Ok after replacing arrays; Err with storage failed when
        /// injection is enabled.
        ///
        /// # Errors
        ///
        /// Returns the injected storage failure after counting the write attempt.
        ///
        /// # Panics
        ///
        /// Panics if manually constructed test credentials exceed the native SSID or password
        /// arrays.
        fn save(&mut self, c: &Credentials) -> Result<(), &'static str> {
            // Update record of transferred rectangles or persistence attempts used to verify side
            // effects using 1, retaining the accumulated state for subsequent steps.
            self.store.writes += 1;
            // Inject a storage error before replacing the simulated persisted profile, allowing
            // rollback behavior to be verified.
            if self.store.fail {
                // Leave this function now with a failure result for the surrounding operation;
                // later statements are skipped.
                return Err("storage failed");
            }
            // Overwrite all elements with the requested value, producing a predictable initial or
            // cleared state.
            self.store.ssid.fill(0);
            // Overwrite all elements with the requested value, producing a predictable initial or
            // cleared state.
            self.store.password.fill(0);
            // Copy the supplied bytes into an equally sized destination slice.
            self.store.ssid[..c.ssid.len()].copy_from_slice(c.ssid.as_bytes());
            // Copy the supplied bytes into an equally sized destination slice.
            self.store.password[..c.password.len()].copy_from_slice(c.password.as_bytes());
            // Return success after the required side effects are complete.
            Ok(())
        }
    }
    /// Constructs valid WPA2 credentials for a simulated service request.
    ///
    /// # Arguments
    ///
    /// * `ssid` (`&str`) - Synthetic network name copied into the test profile.
    ///
    /// # Returns
    ///
    /// `Credentials` - Credentials using the supplied SSID and the fixed test password.
    ///
    /// # Panics
    ///
    /// Panics if the supplied synthetic SSID fails credential validation.
    fn c(ssid: &str) -> Credentials {
        // Require the value guaranteed by this test fixture or internal invariant; unexpected
        // absence panics.
        Credentials::new(
            &Network {
                // Initialize saved network name encoded into the fixed 32-byte native profile field
                // from the supplied value.
                ssid: ssid.into(),
                // Initialize supported Wi-Fi authentication mode from the supplied value.
                security: Security::Wpa2,
                // Initialize received signal strength in dBm; less-negative values represent
                // stronger signals from the supplied value.
                rssi: -30,
            },
            "password123",
        )
        .unwrap()
    }
    /// Creates a simulated service with a previously saved network.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `Service<FakeBackend>` - Idle service backed by a controllable fake network and
    /// credential store.
    ///
    /// # Panics
    ///
    /// Panics if the fixed fixture, test timestamp, or simulated service cannot be constructed.
    fn service() -> Service<FakeBackend> {
        // Keep saved network name encoded into the fixed 32-byte native profile field in this local
        // variable for the following operations.
        let mut ssid = [0; 32];
        // Copy the supplied bytes into an equally sized destination slice.
        ssid[..3].copy_from_slice(b"Old");
        // Keep saved passphrase encoded into the fixed 64-byte native profile field in this local
        // variable for the following operations.
        let mut password = [0; 64];
        // Copy the supplied bytes into an equally sized destination slice.
        password[..11].copy_from_slice(b"password123");
        // Require the value guaranteed by this test fixture or internal invariant; unexpected
        // absence panics.
        Service::new(FakeBackend {
            // Leave optional simulated scan result consumed by the next result query unavailable
            // until a later operation supplies it.
            scan: None,
            // Initialize whether the connection ever associated during this job, distinguishing
            // DHCP timeout as disabled/inactive.
            associated: false,
            // Initialize simulated assigned-IP flag; credentials must not be saved before it
            // becomes true as disabled/inactive.
            up: false,
            // Initialize injected authentication-failure flag used by recovery tests as
            // disabled/inactive.
            auth_failed: false,
            // Start number of association requests issued, used to verify retries at zero; later
            // operations update it as needed.
            attempts: 0,
            // Initialize simulated native station profile with injected write failures and counters
            // from the supplied value.
            store: FakeStore {
                // Initialize saved network name encoded into the fixed 32-byte native profile field
                // from the supplied value.
                ssid,
                // Initialize saved passphrase encoded into the fixed 64-byte native profile field
                // from the supplied value.
                password,
                // Initialize whether simulated profile persistence or reset should fail as
                // disabled/inactive.
                fail: false,
                // Start record of transferred rectangles or persistence attempts used to verify
                // side effects at zero; later operations update it as needed.
                writes: 0,
            },
        })
        .unwrap()
    }
    /// Verifies that credentials are saved only after IP readiness and can be loaded by a
    /// restarted service.
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
    fn save_only_after_ip_then_survive_restart() {
        // Keep preference or service fixture configured for this test scenario in this local
        // variable for the following operations.
        let mut s = service();
        // Cancels prior work and starts the requested scan, retry, connection, reset, or
        // suspension.
        s.command(Command::Connect(1, c("New")), 0);
        // Advances scan and connection jobs, retries attempts, and detects deadlines or link loss.
        s.poll(0);
        // Set whether the connection ever associated during this job, distinguishing DHCP timeout
        // to the enabled state.
        s.backend.associated = true;
        // Advances scan and connection jobs, retries attempts, and detects deadlines or link loss.
        s.poll(1000);
        // Verify that record of transferred rectangles or persistence attempts used to verify side
        // effects exactly matches 0.
        assert_eq!(s.backend.store.writes, 0);
        // Set simulated assigned-IP flag to the enabled state.
        s.backend.up = true;
        // Verify the formatted text or fixture created by matches. A violation means the tested
        // behavior is incorrect.
        assert!(matches!(s.poll(2000), Some(Event::Connected(1, _))));
        // Verify that saved network name encoded into the fixed 32-byte native profile field
        // exactly matches the specified message, format, or data literal.
        assert_eq!(s.backend.load().unwrap().unwrap().ssid, "New");
        // Keep new service instance reading the profile saved by the previous instance in this
        // local variable for the following operations.
        let restarted = Service::new(s.backend).unwrap();
        // Verify the formatted text or fixture created by matches. A violation means the tested
        // behavior is incorrect.
        assert!(matches!(restarted.ready(),Event::Ready(Some(ssid)) if ssid=="New"));
    }
    /// Verifies that failed authentication never writes candidate credentials or replaces the
    /// previous saved profile.
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
    fn authentication_failure_never_writes_or_replaces_old() {
        // Keep preference or service fixture configured for this test scenario in this local
        // variable for the following operations.
        let mut s = service();
        // Cancels prior work and starts the requested scan, retry, connection, reset, or
        // suspension.
        s.command(Command::Connect(1, c("New")), 0);
        // Advances scan and connection jobs, retries attempts, and detects deadlines or link loss.
        s.poll(0);
        // Set injected authentication-failure flag used by recovery tests to the enabled state.
        s.backend.auth_failed = true;
        // Verify advances scan and connection jobs, retries attempts, and detects deadlines or link
        // loss is unavailable. A violation means the tested behavior is incorrect.
        assert!(s.poll(19_999).is_none());
        // Verify the formatted text or fixture created by matches. A violation means the tested
        // behavior is incorrect.
        assert!(matches!(
            s.poll(20_000),
            Some(Event::Failed(1, Failure::Authentication))
        ));
        // Verify that record of transferred rectangles or persistence attempts used to verify side
        // effects exactly matches 0.
        assert_eq!(s.backend.store.writes, 0);
        // Verify that saved network name encoded into the fixed 32-byte native profile field
        // exactly matches the specified message, format, or data literal.
        assert_eq!(s.saved.as_ref().unwrap().ssid, "Old");
    }
    /// Verifies that failed persistence retains the old profile and disconnects the candidate
    /// connection.
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
    fn storage_failure_preserves_old_and_disconnects_candidate() {
        // Keep preference or service fixture configured for this test scenario in this local
        // variable for the following operations.
        let mut s = service();
        // Set whether simulated profile persistence or reset should fail to the enabled state.
        s.backend.store.fail = true;
        // Cancels prior work and starts the requested scan, retry, connection, reset, or
        // suspension.
        s.command(Command::Connect(1, c("New")), 0);
        // Set simulated assigned-IP flag to the enabled state.
        s.backend.up = true;
        // Verify the formatted text or fixture created by matches. A violation means the tested
        // behavior is incorrect.
        assert!(matches!(s.poll(100), Some(Event::Failed(1, Failure::Save))));
        // Verify the inverse of simulated assigned-IP flag. A violation means the tested behavior
        // is incorrect.
        assert!(!s.backend.up);
        // Verify that saved network name encoded into the fixed 32-byte native profile field
        // exactly matches the specified message, format, or data literal.
        assert_eq!(s.saved.as_ref().unwrap().ssid, "Old");
        // Verify that saved network name encoded into the fixed 32-byte native profile field
        // exactly matches the specified message, format, or data literal.
        assert_eq!(s.backend.load().unwrap().unwrap().ssid, "Old");
    }
    /// Verifies that cancelling or replacing work with a scan prevents candidate persistence.
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
    fn cancel_and_scan_do_not_save_a_connected_candidate() {
        // Keep preference or service fixture configured for this test scenario in this local
        // variable for the following operations.
        let mut s = service();
        // Cancels prior work and starts the requested scan, retry, connection, reset, or
        // suspension.
        s.command(Command::Connect(1, c("New")), 0);
        // Set simulated assigned-IP flag to the enabled state.
        s.backend.up = true;
        // Cancels prior work and starts the requested scan, retry, connection, reset, or
        // suspension.
        s.command(Command::Suspend(2), 10);
        // Verify advances scan and connection jobs, retries attempts, and detects deadlines or link
        // loss is unavailable. A violation means the tested behavior is incorrect.
        assert!(s.poll(20).is_none());
        // Verify that record of transferred rectangles or persistence attempts used to verify side
        // effects exactly matches 0.
        assert_eq!(s.backend.store.writes, 0);
        // Cancels prior work and starts the requested scan, retry, connection, reset, or
        // suspension.
        s.command(Command::Scan(3), 30);
        // Set optional simulated scan result consumed by the next result query to an available
        // value for a failure result for the surrounding operation.
        s.backend.scan = Some(Err(Failure::Scan));
        // Verify the formatted text or fixture created by matches. A violation means the tested
        // behavior is incorrect.
        assert!(matches!(
            s.poll(40),
            Some(Event::Scanned(3, Err(Failure::Scan)))
        ));
    }
    /// Verifies that missing or empty scan results are reported only after the full connection
    /// recovery deadline.
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
    fn missing_network_and_no_networks_wait_full_recovery_window() {
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for (networks, failure) in [
            (vec![], Failure::NoNetworks),
            (
                vec![Network {
                    ssid: "Other".into(),
                    security: Security::Wpa2,
                    rssi: -10,
                }],
                Failure::Missing,
            ),
        ] {
            // Keep preference or service fixture configured for this test scenario in this local
            // variable for the following operations.
            let mut s = service();
            // Cancels prior work and starts the requested scan, retry, connection, reset, or
            // suspension.
            s.command(Command::Retry(1), 0);
            // Set optional simulated scan result consumed by the next result query to an available
            // value for a successful result carrying nearby access points available for selection.
            s.backend.scan = Some(Ok(networks));
            // Verify advances scan and connection jobs, retries attempts, and detects deadlines or
            // link loss is unavailable. A violation means the tested behavior is incorrect.
            assert!(s.poll(1000).is_none());
            // Verify the formatted text or fixture created by matches. A violation means the tested
            // behavior is incorrect.
            assert!(matches!(s.poll(20_000),Some(Event::Failed(1,f)) if f==failure));
            // Verify that number of association requests issued, used to verify retries exactly
            // matches 0.
            assert_eq!(s.backend.attempts, 0);
        }
    }
    /// Verifies distinct events for failure to obtain an IP address and loss of an established
    /// connection.
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
    fn dhcp_timeout_and_connection_loss_are_distinct() {
        // Keep preference or service fixture configured for this test scenario in this local
        // variable for the following operations.
        let mut s = service();
        // Cancels prior work and starts the requested scan, retry, connection, reset, or
        // suspension.
        s.command(Command::Connect(1, c("New")), 0);
        // Set whether the connection ever associated during this job, distinguishing DHCP timeout
        // to the enabled state.
        s.backend.associated = true;
        // Verify the formatted text or fixture created by matches. A violation means the tested
        // behavior is incorrect.
        assert!(matches!(
            s.poll(20_000),
            Some(Event::Failed(1, Failure::IpTimeout))
        ));
        // Cancels prior work and starts the requested scan, retry, connection, reset, or
        // suspension.
        s.command(Command::Connect(2, c("New")), 30_000);
        // Set simulated assigned-IP flag to the enabled state.
        s.backend.up = true;
        // Advances scan and connection jobs, retries attempts, and detects deadlines or link loss.
        s.poll(30_100);
        // Set simulated assigned-IP flag to the disabled state.
        s.backend.up = false;
        // Verify the formatted text or fixture created by matches. A violation means the tested
        // behavior is incorrect.
        assert!(matches!(s.poll(30_200), Some(Event::Lost)));
        // Verify advances scan and connection jobs, retries attempts, and detects deadlines or link
        // loss is unavailable. A violation means the tested behavior is incorrect.
        assert!(s.poll(30_300).is_none());
    }
}
