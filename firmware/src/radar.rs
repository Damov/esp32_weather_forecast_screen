// ============================================================================= //
// File          : radar.rs                                                      //
// License       : GPL-3.0-only                                                  //
// Created       : 2026-10-04                                                    //
// Last modified : 2026-10-04                                                    //
// Author        : Daniel Kharlamov                                              //
// Last editor   : Daniel Kharlamov <daniel.kharlamov@googlemail.com>            //
// ============================================================================= //
//                                                                               //
// Summary:                                                                      //
// --------                                                                      //
// Passive HLK-LD2410C radar parsing and presence detection.                     //
//                                                                               //
// Module:                                                                       //
// -------                                                                       //
// Parses basic and engineering UART reports using HLK protocol V1.09, validates //
// frames, recovers from serial noise, and extracts target distances and  energy //
// readings. Clears stale readings after one second. On  ESP32, initializes  and //
// polls the UART receiver  and OUT pin,  combining their  signals into present, //
// absent, or unavailable presence  status. Includes unit tests for parsing, re- //
// covery, expiry, and presence handling.                                        //
// ============================================================================= //

//! Passive HLK-LD2410C radar parsing and presence detection.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
/// UART measurements: distances in centimetres and sensor energy on its 0..100 scale.
/// `connected` and `present` describe fresh UART reports, independently of OUT.
pub struct RadarReading {
    // Whether a usable UART report or network connection is currently available. Stored as bool.
    pub connected: bool,
    // Whether a fresh report indicates at least one detected target. Stored as bool.
    pub present: bool,
    // Whether the report's moving-target bit is set. Stored as bool.
    pub moving_target: bool,
    // Whether the report's stationary-target bit is set. Stored as bool.
    pub stationary_target: bool,
    // Little-endian moving-target distance in centimetres. Stored as u16.
    pub moving_distance_cm: u16,
    // Moving-target energy on the sensor's 0-100 scale. Stored as u8.
    pub moving_energy: u8,
    // Little-endian stationary-target distance in centimetres. Stored as u16.
    pub stationary_distance_cm: u16,
    // Stationary-target energy on the sensor's 0-100 scale. Stored as u8.
    pub stationary_energy: u8,
    // Sensor's detection distance in centimetres. Stored as u16.
    pub detection_distance_cm: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PresenceStatus {
    // At least one fresh UART target or an asserted OUT signal indicates presence.
    Present,
    // A fresh UART report indicates no targets and OUT is not asserted.
    Absent,
    #[default]
    // No fresh UART report or independent positive OUT signal is available.
    Unavailable,
}
impl PresenceStatus {
    /// Combines fresh UART presence with the independent active-high OUT signal.
    ///
    /// # Arguments
    ///
    /// * `reading` (`RadarReading`) - Latest UART measurements, including connectivity and
    ///   target presence flags.
    /// * `out_high` (`bool`) - Current active-high OUT pin level, independent of UART report
    ///   freshness.
    ///
    /// # Returns
    ///
    /// `Self` - Present if either source reports presence, Absent for a fresh negative UART
    /// reading, otherwise Unavailable.
    pub fn from_reading(reading: RadarReading, out_high: bool) -> Self {
        // Treat either an asserted OUT pin or a fresh positive UART report as presence; neither
        // signal must override the other negatively.
        if out_high || (reading.connected && reading.present) {
            Self::Present
        // A fresh negative UART report can distinguish confirmed absence from unavailable sensor
        // data.
        } else if reading.connected {
            Self::Absent
        } else {
            Self::Unavailable
        }
    }
    /// Checks whether the combined status indicates presence.
    ///
    /// # Arguments
    ///
    /// * `self` (`PresenceStatus`) - Combined UART and OUT presence status. Passed by value.
    ///
    /// # Returns
    ///
    /// `bool` - True only for Present; false for Absent and Unavailable.
    pub fn present(self) -> bool {
        // Return self equals Present state as the value of this block.
        self == Self::Present
    }
}

pub struct Parser {
    // Fixed 64-byte UART buffer; it holds incomplete frames until enough bytes arrive. Stored as
    // [u8; 64].
    buffer: [u8; 64],
    // Number of valid bytes currently stored in the UART buffer. Stored as usize.
    length: usize,
    // Monotonic millisecond timestamp of the last successfully decoded report. Stored as
    // Option<u64>.
    last_valid: Option<u64>,
    // Latest decoded target flags, distances, and energy measurements. Stored as RadarReading.
    reading: RadarReading,
}
impl Default for Parser {
    /// Creates an empty radar parser with no valid report.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `Self` - Parser with an empty 64-byte buffer, no freshness timestamp, and default
    /// measurements.
    fn default() -> Self {
        Self {
            // Initialize fixed 64-byte UART buffer; it holds incomplete frames until enough bytes
            // arrive from the supplied value.
            buffer: [0; 64],
            // Start number of valid bytes currently stored in the UART buffer at zero; later
            // operations update it as needed.
            length: 0,
            // Leave monotonic millisecond timestamp of the last successfully decoded report
            // unavailable until a later operation supplies it.
            last_valid: None,
            // Initialize latest decoded target flags, distances, and energy measurements from the
            // supplied value.
            reading: RadarReading::default(),
        }
    }
}
impl Parser {
    /// Copies the latest UART measurements without consulting OUT.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Parser`) - Radar frame buffer, latest measurements, and freshness timestamp.
    ///   Borrowed without changing it.
    ///
    /// # Returns
    ///
    /// `RadarReading` - Latest validated report, or default disconnected measurements before a
    /// report or after expiry.
    pub fn reading(&self) -> RadarReading {
        // Return latest decoded target flags, distances, and energy measurements as the value of
        // this block.
        self.reading
    }
    /// Buffers received bytes and processes complete reports, recovering from noise.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Parser`) - Radar frame buffer, latest measurements, and freshness
    ///   timestamp. Mutated in place.
    /// * `bytes` (`&[u8]`) - Received UART bytes, which may contain noise, partial frames, or
    ///   consecutive reports.
    /// * `now` (`u64`) - Monotonic timestamp in milliseconds, using the same origin as stored
    ///   deadlines.
    ///
    /// # Returns
    ///
    /// `()` - No value; updates buffered fragments, measurements, and the valid-report
    /// timestamp.
    pub fn feed(&mut self, bytes: &[u8], now: u64) {
        // Read each incoming UART byte in order, allowing fragmented and consecutive reports to
        // share the same parser.
        for &byte in bytes {
            // The 64-byte buffer is full; discard its oldest byte before appending another so noisy
            // input cannot overflow it.
            if self.length == self.buffer.len() {
                // Removes a consumed prefix from the parser buffer.
                self.discard(1);
            }
            // Append the received byte to the first free buffer slot without disturbing earlier
            // partial-frame data.
            self.buffer[self.length] = byte;
            // Update the buffered-byte count so the next parser pass knows which bytes are valid.
            self.length += 1;
            // Validates complete basic or engineering frames and resynchronizes malformed input.
            self.process(now);
        }
    }
    /// Expires UART measurements after one second without a valid report.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Parser`) - Radar frame buffer, latest measurements, and freshness
    ///   timestamp. Mutated in place.
    /// * `now` (`u64`) - Monotonic timestamp in milliseconds, using the same origin as stored
    ///   deadlines.
    ///
    /// # Returns
    ///
    /// `()` - No value; clears measurements and buffered bytes once the freshness deadline is
    /// reached.
    pub fn tick(&mut self, now: u64) {
        // One second has elapsed since a valid UART report; clear stale measurements and any
        // incomplete bytes.
        if self
            .last_valid
            .is_some_and(|time| now.saturating_sub(time) >= 1000)
        {
            // Publish the validated report, or replace it with disconnected zeroed measurements
            // when the report expires.
            self.reading = RadarReading::default();
            // Record the current millisecond freshness timestamp, or clear it when measurements
            // expire.
            self.last_valid = None;
            // Update the buffered-byte count so the next parser pass knows which bytes are valid.
            self.length = 0;
        }
    }
    /// Removes a consumed prefix from the parser buffer.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Parser`) - Radar frame buffer, latest measurements, and freshness
    ///   timestamp. Mutated in place.
    /// * `count` (`usize`) - Number of buffered prefix bytes to remove; must not exceed the
    ///   buffered length.
    ///
    /// # Returns
    ///
    /// `()` - No value; shifts remaining bytes to the beginning and reduces the buffered
    /// length.
    ///
    /// # Panics
    ///
    /// Panics if count exceeds the buffered length; callers must discard only existing bytes.
    fn discard(&mut self, count: usize) {
        // Move the unconsumed bytes to the front of the buffer so the next frame can be assembled.
        self.buffer.copy_within(count..self.length, 0);
        // Update the buffered-byte count so the next parser pass knows which bytes are valid.
        self.length -= count;
    }
    /// Validates complete basic or engineering frames and resynchronizes malformed input.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Parser`) - Radar frame buffer, latest measurements, and freshness
    ///   timestamp. Mutated in place.
    /// * `now` (`u64`) - Monotonic timestamp in milliseconds, using the same origin as stored
    ///   deadlines.
    ///
    /// # Returns
    ///
    /// `()` - No value; publishes valid measurements and retains incomplete frames for later
    /// input.
    fn process(&mut self, now: u64) {
        // At least four bytes are available to compare against the protocol header; shorter
        // fragments must wait.
        while self.length >= 4 {
            // The buffered prefix is not the radar header; advance by one byte and search again
            // instead of trusting noise.
            if self.buffer[..4] != [0xf4, 0xf3, 0xf2, 0xf1] {
                // Removes a consumed prefix from the parser buffer.
                self.discard(1);
                // Skip the remainder of this iteration and wait for or inspect the next input.
                continue;
            }
            // The header is present but the two length bytes have not arrived; retain the fragment
            // for the next feed.
            if self.length < 6 {
                // Leave this function now after completing the required side effects; later
                // statements are skipped.
                return;
            }
            // Decode the two little-endian length bytes; the payload must fit the fixed 64-byte
            // frame buffer.
            let size = u16::from_le_bytes([self.buffer[4], self.buffer[5]]) as usize;
            // The declared payload length cannot be a supported report or fit the buffer; resume
            // header search.
            if !(13..=54).contains(&size) {
                // Removes a consumed prefix from the parser buffer.
                self.discard(1);
                // Skip the remainder of this iteration and wait for or inspect the next input.
                continue;
            }
            // Include the four-byte header, two-byte length, and four-byte footer in the complete
            // frame size.
            let total = size + 10;
            // The declared frame is incomplete; wait for more bytes without dropping a valid
            // prefix.
            if self.length < total {
                // Leave this function now after completing the required side effects; later
                // statements are skipped.
                return;
            }
            // The expected footer does not match, so this candidate frame must be rejected and
            // resynchronized.
            if self.buffer[6 + size..total] != [0xf8, 0xf7, 0xf6, 0xf5] {
                // Removes a consumed prefix from the parser buffer.
                self.discard(1);
                // Skip the remainder of this iteration and wait for or inspect the next input.
                continue;
            }
            // Borrow only the payload bytes so target fields can be read at protocol-defined
            // offsets without copying.
            let p = &self.buffer[6..6 + size];
            // Accept only basic/engineering reports with valid payload markers and target-state
            // bits; publish measurements only after all checks pass.
            if matches!(p[0], 1 | 2) && p[1] == 0xaa && p[size - 2..] == [0x55, 0] && p[2] <= 3 {
                // Publish the validated report, or replace it with disconnected zeroed measurements
                // when the report expires.
                self.reading = RadarReading {
                    // Initialize whether a usable UART report or network connection is currently
                    // available as enabled/active.
                    connected: true,
                    // Initialize whether a fresh report indicates at least one detected target from
                    // the supplied value.
                    present: p[2] != 0,
                    // Initialize whether the report's moving-target bit is set from the supplied
                    // value.
                    moving_target: p[2] & 1 != 0,
                    // Initialize whether the report's stationary-target bit is set from the
                    // supplied value.
                    stationary_target: p[2] & 2 != 0,
                    // Initialize little-endian moving-target distance in centimetres from the
                    // supplied value.
                    moving_distance_cm: u16::from_le_bytes([p[3], p[4]]),
                    // Initialize moving-target energy on the sensor's 0-100 scale from the supplied
                    // value.
                    moving_energy: p[5],
                    // Initialize little-endian stationary-target distance in centimetres from the
                    // supplied value.
                    stationary_distance_cm: u16::from_le_bytes([p[6], p[7]]),
                    // Initialize stationary-target energy on the sensor's 0-100 scale from the
                    // supplied value.
                    stationary_energy: p[8],
                    // Initialize sensor's detection distance in centimetres from the supplied
                    // value.
                    detection_distance_cm: u16::from_le_bytes([p[9], p[10]]),
                };
                // Record the current millisecond freshness timestamp, or clear it when measurements
                // expire.
                self.last_valid = Some(now);
            }
            // Removes a consumed prefix from the parser buffer.
            self.discard(total);
        }
    }
}

/// ESP-IDF UART and GPIO adapter for passive HLK-LD2410C presence sensing.
#[cfg(target_os = "espidf")]
pub mod device {
    use super::{Parser, PresenceStatus, RadarReading};
    use esp_idf_svc::hal::{
        gpio::{AnyInputPin, AnyOutputPin, Input, PinDriver, Pull},
        uart::{config::Config, UartRxDriver},
        units::Hertz,
    };
    pub struct Radar {
        // UART receiver dedicated to passive radar reports. Stored as
        // Option<UartRxDriver<'static>>.
        uart: Option<UartRxDriver<'static>>,
        // Active-high radar GPIO driver, independent of UART connectivity. Stored as
        // Option<PinDriver<'static, Input>>.
        out: Option<PinDriver<'static, Input>>,
        // Bounded UART frame decoder retaining partial reports and the latest measurements. Stored
        // as Parser.
        parser: Parser,
        // Whether a UART failure has already been logged, preventing repeated warnings. Stored as
        // bool.
        read_failed: bool,
    }
    impl Radar {
        /// Initializes the UART1 receiver at 256000 baud and the active-high OUT input.
        ///
        /// # Arguments
        ///
        /// * `uart` (`esp_idf_svc::hal::uart::UART1<'static>`) - Owned UART1 peripheral dedicated
        ///   to receiving radar reports.
        /// * `rx` (`esp_idf_svc::hal::gpio::Gpio35<'static>`) - GPIO35 input connected to the radar
        ///   TX pin; radar RX remains unconnected.
        /// * `out` (`esp_idf_svc::hal::gpio::Gpio32<'static>`) - GPIO32 active-high presence input,
        ///   initialized with a pull-down.
        ///
        /// # Returns
        ///
        /// `Self` - Radar with independently optional UART and OUT drivers; initialization errors
        /// are logged without aborting.
        pub fn new(
            uart: esp_idf_svc::hal::uart::UART1<'static>,
            rx: esp_idf_svc::hal::gpio::Gpio35<'static>,
            out: esp_idf_svc::hal::gpio::Gpio32<'static>,
        ) -> Self {
            // Keep active-high radar GPIO driver, independent of UART connectivity in this local
            // variable for the following operations.
            let out = PinDriver::input(out, Pull::Down)
                .map_err(|error| {
                    // Log this recoverable hardware failure; sensing or the interface may continue
                    // with reduced functionality.
                    log::warn!("Radar OUT initialization failed: {error}");
                })
                .ok();
            // Keep UART receiver dedicated to passive radar reports in this local variable for the
            // following operations.
            let uart = UartRxDriver::new(
                uart,
                rx,
                None::<AnyInputPin>,
                None::<AnyOutputPin>,
                &Config {
                    // Initialize factory radar serial rate of 256000 bits per second from the
                    // supplied value.
                    baudrate: Hertz(256_000),
                    // Initialize 4096-byte UART receive buffer absorbing bursts while the UI is
                    // busy from the supplied value.
                    rx_fifo_size: 4096,
                    // Start UART event queue length; zero avoids an unused event queue at zero;
                    // later operations update it as needed.
                    queue_size: 0,
                    ..Config::new()
                },
            )
            .map_err(|error| {
                // Log this recoverable hardware failure; sensing or the interface may continue with
                // reduced functionality.
                log::warn!("Radar UART initialization failed: {error}");
            })
            .ok();
            Self {
                // Initialize UART receiver dedicated to passive radar reports from the supplied
                // value.
                uart,
                // Initialize active-high radar GPIO driver, independent of UART connectivity from
                // the supplied value.
                out,
                // Initialize bounded UART frame decoder retaining partial reports and the latest
                // measurements from the supplied value.
                parser: Parser::default(),
                // Initialize whether a UART failure has already been logged, preventing repeated
                // warnings as disabled/inactive.
                read_failed: false,
            }
        }
        /// Copies the latest parsed UART measurements.
        ///
        /// # Arguments
        ///
        /// * `self` (`&Radar`) - Optional UART/OUT drivers and passive radar parser. Borrowed
        ///   without changing it.
        ///
        /// # Returns
        ///
        /// `RadarReading` - Current UART reading; OUT presence is reported separately by poll.
        pub fn reading(&self) -> RadarReading {
            // Copies the latest UART measurements without consulting OUT.
            self.parser.reading()
        }
        /// Reads a bounded batch of UART data, expires stale reports, and samples OUT.
        ///
        /// # Arguments
        ///
        /// * `self` (`&mut Radar`) - Optional UART/OUT drivers and passive radar parser. Mutated in
        ///   place.
        /// * `now` (`u64`) - Monotonic timestamp in milliseconds, using the same origin as stored
        ///   deadlines.
        ///
        /// # Returns
        ///
        /// `PresenceStatus` - Combined presence status. UART read errors clear measurements; an
        /// available OUT input remains usable.
        pub fn poll(&mut self, now: u64) -> PresenceStatus {
            // Read serial data only if UART initialization succeeded; the independent OUT input can
            // still work without UART.
            if let Some(uart) = &self.uart {
                // Keep byte buffer used to decode, encode, or transfer the surrounding data in this
                // local variable for the following operations.
                let mut bytes = [0; 128];
                // Read at most eight UART chunks per UI iteration so radar traffic cannot
                // monopolize the main loop.
                for _ in 0..8 {
                    // Choose the appropriate path for read result for the surrounding operation;
                    // each arm handles one supported case.
                    match uart.read(&mut bytes, 0) {
                        // Stop this loop now; the enclosing function continues with the work after
                        // the loop.
                        Ok(0) => break,
                        // Continue with the successful result, using its validated value in this
                        // case.
                        Ok(count) => {
                            // Buffers received bytes and processes complete reports, recovering
                            // from noise.
                            self.parser.feed(&bytes[..count], now);
                            // Set whether a UART failure has already been logged, preventing
                            // repeated warnings to the disabled state.
                            self.read_failed = false;
                        }
                        // ESP-IDF HAL reports an empty nonblocking read as timeout.
                        // Retain fragmented reports and let the one-second timer expire them.
                        // Stop this loop now; the enclosing function continues with the work after
                        // the loop.
                        Err(error) if error.code() == esp_idf_svc::sys::ESP_ERR_TIMEOUT => break,
                        // Handle a failed operation here rather than treating its value as valid.
                        Err(error) => {
                            // Log the first consecutive UART failure only, avoiding a warning on
                            // every UI-loop iteration.
                            if !self.read_failed {
                                // Log this recoverable hardware failure; sensing or the interface
                                // may continue with reduced functionality.
                                log::warn!("Radar UART read failed: {error}");
                            }
                            // Set whether a UART failure has already been logged, preventing
                            // repeated warnings to the enabled state.
                            self.read_failed = true;
                            // Set bounded UART frame decoder retaining partial reports and the
                            // latest measurements to the type's documented default state.
                            self.parser = Parser::default();
                            // Stop this loop now; the enclosing function continues with the work
                            // after the loop.
                            break;
                        }
                    }
                }
            }
            // Expires UART measurements after one second without a valid report.
            self.parser.tick(now);
            // Combines fresh UART presence with the independent active-high OUT signal.
            PresenceStatus::from_reading(
                self.parser.reading(),
                self.out.as_ref().is_some_and(|pin| pin.is_high()),
            )
        }
    }
}

///////////////////////////////////////////////////////////////////////////////////////
//                                    UNIT TESTS                                     //
///////////////////////////////////////////////////////////////////////////////////////

/// Unit tests and supporting fixtures for radar behavior.
#[cfg(test)]
mod tests {
    use super::*;
    /// Constructs a synthetic basic or engineering radar report for parser tests.
    ///
    /// # Arguments
    ///
    /// * `kind` (`u8`) - Report kind: 1 for engineering data, 2 for basic data.
    /// * `state` (`u8`) - Synthetic target flag bits: 0 absent, 1 moving, 2 stationary, or 3
    ///   both.
    ///
    /// # Returns
    ///
    /// `Vec<u8>` - Framed bytes containing fixed little-endian distances and energy values;
    /// kind 1 includes engineering data.
    fn frame(kind: u8, state: u8) -> Vec<u8> {
        // Keep synthetic report contents, including known little-endian distance test values in
        // this local variable for the following operations.
        let mut payload = vec![
            kind, 0xaa, state, 0x34, 0x12, 75, 0x78, 0x56, 30, 0x9a, 0x02,
        ];
        // Check whether kind equals 1.
        if kind == 1 {
            // Append the supplied bytes in order to the existing buffer.
            payload.extend_from_slice(&[
                8, 8, 10, 20, 30, 40, 50, 60, 70, 80, 90, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]);
        }
        // Append the supplied bytes in order to the existing buffer.
        payload.extend_from_slice(&[0x55, 0]);
        // Keep complete synthetic UART report including header, length, payload, and footer in this
        // local variable for the following operations.
        let mut frame = vec![0xf4, 0xf3, 0xf2, 0xf1];
        // Append the supplied bytes in order to the existing buffer.
        frame.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        // Append the supplied bytes in order to the existing buffer.
        frame.extend_from_slice(&payload);
        // Append the supplied bytes in order to the existing buffer.
        frame.extend_from_slice(&[0xf8, 0xf7, 0xf6, 0xf5]);
        // Return complete synthetic UART report including header, length, payload, and footer as
        // the value of this block.
        frame
    }
    /// Verifies all target states in basic and engineering reports and correct little-endian
    /// distance and energy decoding.
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
    fn basic_and_engineering_target_states_and_little_endian_metrics() {
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for kind in [1, 2] {
            // Visit each entry in 0..4; the loop binding provides its value or index for this
            // iteration.
            for state in 0..4 {
                // Keep bounded UART frame decoder retaining partial reports and the latest
                // measurements in this local variable for the following operations.
                let mut parser = Parser::default();
                // Buffers received bytes and processes complete reports, recovering from noise.
                parser.feed(&frame(kind, state), 0);
                // Keep latest decoded target flags, distances, and energy measurements in this
                // local variable for the following operations.
                let reading = parser.reading();
                // Verify whether a usable UART report or network connection is currently available.
                // A violation means the tested behavior is incorrect.
                assert!(reading.connected);
                // Verify that whether a fresh report indicates at least one detected target exactly
                // matches state differs from 0.
                assert_eq!(reading.present, state != 0);
                // Verify that whether the report's moving-target bit is set exactly matches state
                // masked with 1 differs from 0.
                assert_eq!(reading.moving_target, state & 1 != 0);
                // Verify that whether the report's stationary-target bit is set exactly matches
                // state masked with 2 differs from 0.
                assert_eq!(reading.stationary_target, state & 2 != 0);
                // Verify that little-endian moving-target distance in centimetres exactly matches
                // 0x1234.
                assert_eq!(reading.moving_distance_cm, 0x1234);
                // Verify that moving-target energy on the sensor's 0-100 scale exactly matches 75.
                assert_eq!(reading.moving_energy, 75);
                // Verify that little-endian stationary-target distance in centimetres exactly
                // matches 0x5678.
                assert_eq!(reading.stationary_distance_cm, 0x5678);
                // Verify that stationary-target energy on the sensor's 0-100 scale exactly matches
                // 30.
                assert_eq!(reading.stationary_energy, 30);
                // Verify that sensor's detection distance in centimetres exactly matches 0x029a.
                assert_eq!(reading.detection_distance_cm, 0x029a);
            }
        }
    }
    /// Verifies parser recovery from leading noise, bytewise fragmentation, and consecutive
    /// valid reports.
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
    fn fragmented_consecutive_reports_and_noise() {
        // Keep bounded UART frame decoder retaining partial reports and the latest measurements in
        // this local variable for the following operations.
        let mut parser = Parser::default();
        // Buffers received bytes and processes complete reports, recovering from noise.
        parser.feed(&[4, 3, 2, 1, 0xf4, 0xf3], 0);
        // Visit each entry in constructs a synthetic basic or engineering radar report for parser
        // tests; the loop binding provides its value or index for this iteration.
        for byte in frame(2, 1) {
            // Buffers received bytes and processes complete reports, recovering from noise.
            parser.feed(&[byte], 100);
        }
        // Verify whether the report's moving-target bit is set. A violation means the tested
        // behavior is incorrect.
        assert!(parser.reading().moving_target);
        // Keep consecutive synthetic frames used to verify decoding without artificial gaps in this
        // local variable for the following operations.
        let reports = [frame(1, 2), frame(2, 0)].concat();
        // Buffers received bytes and processes complete reports, recovering from noise.
        parser.feed(&reports, 200);
        // Verify whether a usable UART report or network connection is currently available. A
        // violation means the tested behavior is incorrect.
        assert!(parser.reading().connected);
        // Verify the inverse of whether a fresh report indicates at least one detected target. A
        // violation means the tested behavior is incorrect.
        assert!(!parser.reading().present);
    }
    /// Verifies that corrupted frames do not publish presence and that a subsequent valid frame
    /// restores parsing.
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
    fn invalid_frames_resynchronize_without_publishing_presence() {
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for offset in [0, 4, 6, 7, 8, 17, 18, 19, 22] {
            // Keep bounded UART frame decoder retaining partial reports and the latest measurements
            // in this local variable for the following operations.
            let mut parser = Parser::default();
            // Keep copy of a valid frame modified to exercise framing rejection and
            // resynchronization in this local variable for the following operations.
            let mut bad = frame(2, 3);
            // Set the selected entry from copy of a valid frame modified to exercise framing
            // rejection and resynchronization to 0xff.
            bad[offset] = 0xff;
            // Buffers received bytes and processes complete reports, recovering from noise.
            parser.feed(&bad, 0);
            // Verify the inverse of whether a usable UART report or network connection is currently
            // available. A violation means the tested behavior is incorrect.
            assert!(!parser.reading().connected, "offset {offset}");
            // Buffers received bytes and processes complete reports, recovering from noise.
            parser.feed(&frame(2, 2), 100);
            // Verify whether the report's stationary-target bit is set. A violation means the
            // tested behavior is incorrect.
            assert!(parser.reading().stationary_target, "offset {offset}");
        }
        // Keep bounded UART frame decoder retaining partial reports and the latest measurements in
        // this local variable for the following operations.
        let mut parser = Parser::default();
        // Buffers received bytes and processes complete reports, recovering from noise.
        parser.feed(&[0xff; 500], 0);
        // Buffers received bytes and processes complete reports, recovering from noise.
        parser.feed(&frame(2, 1), 100);
        // Verify whether the report's moving-target bit is set. A violation means the tested
        // behavior is incorrect.
        assert!(parser.reading().moving_target);
    }
    /// Verifies exact one-second report expiry, cleared measurements, and independent OUT-based
    /// presence.
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
    fn stale_reports_clear_measurements_and_out_remains_independent() {
        // Keep bounded UART frame decoder retaining partial reports and the latest measurements in
        // this local variable for the following operations.
        let mut parser = Parser::default();
        // Buffers received bytes and processes complete reports, recovering from noise.
        parser.feed(&frame(2, 3), 100);
        // Expires UART measurements after one second without a valid report.
        parser.tick(1099);
        // Verify whether a fresh report indicates at least one detected target. A violation means
        // the tested behavior is incorrect.
        assert!(parser.reading().present);
        // Buffers received bytes and processes complete reports, recovering from noise.
        parser.feed(&[0xff; 20], 1099);
        // Expires UART measurements after one second without a valid report.
        parser.tick(1100);
        // Verify that copies the latest UART measurements without consulting OUT exactly matches
        // the type's documented default state.
        assert_eq!(parser.reading(), RadarReading::default());
        // Verify that combines fresh UART presence with the independent active-high OUT signal
        // exactly matches Unavailable state.
        assert_eq!(
            PresenceStatus::from_reading(parser.reading(), false),
            PresenceStatus::Unavailable
        );
        // Verify that combines fresh UART presence with the independent active-high OUT signal
        // exactly matches Present state.
        assert_eq!(
            PresenceStatus::from_reading(parser.reading(), true),
            PresenceStatus::Present
        );
        // Buffers received bytes and processes complete reports, recovering from noise.
        parser.feed(&frame(2, 0), 1200);
        // Verify that combines fresh UART presence with the independent active-high OUT signal
        // exactly matches Absent state.
        assert_eq!(
            PresenceStatus::from_reading(parser.reading(), false),
            PresenceStatus::Absent
        );
        // Verify that combines fresh UART presence with the independent active-high OUT signal
        // exactly matches Present state.
        assert_eq!(
            PresenceStatus::from_reading(parser.reading(), true),
            PresenceStatus::Present
        );
        // Buffers received bytes and processes complete reports, recovering from noise.
        parser.feed(&frame(2, 1), 1300);
        // Verify that combines fresh UART presence with the independent active-high OUT signal
        // exactly matches Present state.
        assert_eq!(
            PresenceStatus::from_reading(parser.reading(), false),
            PresenceStatus::Present
        );
    }
}
