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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
/// UART measurements: distances in centimetres and sensor energy on its 0..100 scale.
/// `connected` and `present` describe fresh UART reports, independently of OUT.
pub struct RadarReading {
    pub connected: bool,
    pub present: bool,
    pub moving_target: bool,
    pub stationary_target: bool,
    pub moving_distance_cm: u16,
    pub moving_energy: u8,
    pub stationary_distance_cm: u16,
    pub stationary_energy: u8,
    pub detection_distance_cm: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PresenceStatus {
    Present,
    Absent,
    #[default]
    Unavailable,
}
impl PresenceStatus {
    pub fn from_reading(reading: RadarReading, out_high: bool) -> Self {
        if out_high || (reading.connected && reading.present) {
            Self::Present
        } else if reading.connected {
            Self::Absent
        } else {
            Self::Unavailable
        }
    }
    pub fn present(self) -> bool {
        self == Self::Present
    }
}

pub struct Parser {
    buffer: [u8; 64],
    length: usize,
    last_valid: Option<u64>,
    reading: RadarReading,
}
impl Default for Parser {
    fn default() -> Self {
        Self {
            buffer: [0; 64],
            length: 0,
            last_valid: None,
            reading: RadarReading::default(),
        }
    }
}
impl Parser {
    pub fn reading(&self) -> RadarReading {
        self.reading
    }
    pub fn feed(&mut self, bytes: &[u8], now: u64) {
        for &byte in bytes {
            if self.length == self.buffer.len() {
                self.discard(1);
            }
            self.buffer[self.length] = byte;
            self.length += 1;
            self.process(now);
        }
    }
    pub fn tick(&mut self, now: u64) {
        if self
            .last_valid
            .is_some_and(|time| now.saturating_sub(time) >= 1000)
        {
            self.reading = RadarReading::default();
            self.last_valid = None;
            self.length = 0;
        }
    }
    fn discard(&mut self, count: usize) {
        self.buffer.copy_within(count..self.length, 0);
        self.length -= count;
    }
    fn process(&mut self, now: u64) {
        while self.length >= 4 {
            if self.buffer[..4] != [0xf4, 0xf3, 0xf2, 0xf1] {
                self.discard(1);
                continue;
            }
            if self.length < 6 {
                return;
            }
            let size = u16::from_le_bytes([self.buffer[4], self.buffer[5]]) as usize;
            if !(13..=54).contains(&size) {
                self.discard(1);
                continue;
            }
            let total = size + 10;
            if self.length < total {
                return;
            }
            if self.buffer[6 + size..total] != [0xf8, 0xf7, 0xf6, 0xf5] {
                self.discard(1);
                continue;
            }
            let p = &self.buffer[6..6 + size];
            if matches!(p[0], 1 | 2) && p[1] == 0xaa && p[size - 2..] == [0x55, 0] && p[2] <= 3 {
                self.reading = RadarReading {
                    connected: true,
                    present: p[2] != 0,
                    moving_target: p[2] & 1 != 0,
                    stationary_target: p[2] & 2 != 0,
                    moving_distance_cm: u16::from_le_bytes([p[3], p[4]]),
                    moving_energy: p[5],
                    stationary_distance_cm: u16::from_le_bytes([p[6], p[7]]),
                    stationary_energy: p[8],
                    detection_distance_cm: u16::from_le_bytes([p[9], p[10]]),
                };
                self.last_valid = Some(now);
            }
            self.discard(total);
        }
    }
}

#[cfg(target_os = "espidf")]
pub mod device {
    use super::{Parser, PresenceStatus, RadarReading};
    use esp_idf_svc::hal::{
        gpio::{AnyInputPin, AnyOutputPin, Input, PinDriver, Pull},
        uart::{config::Config, UartRxDriver},
        units::Hertz,
    };
    pub struct Radar {
        uart: Option<UartRxDriver<'static>>,
        out: Option<PinDriver<'static, Input>>,
        parser: Parser,
        read_failed: bool,
    }
    impl Radar {
        pub fn new(
            uart: esp_idf_svc::hal::uart::UART1<'static>,
            rx: esp_idf_svc::hal::gpio::Gpio35<'static>,
            out: esp_idf_svc::hal::gpio::Gpio32<'static>,
        ) -> Self {
            let out = PinDriver::input(out, Pull::Down)
                .map_err(|error| {
                    log::warn!("Radar OUT initialization failed: {error}");
                })
                .ok();
            let uart = UartRxDriver::new(
                uart,
                rx,
                None::<AnyInputPin>,
                None::<AnyOutputPin>,
                &Config {
                    baudrate: Hertz(256_000),
                    rx_fifo_size: 4096,
                    queue_size: 0,
                    ..Config::new()
                },
            )
            .map_err(|error| {
                log::warn!("Radar UART initialization failed: {error}");
            })
            .ok();
            Self {
                uart,
                out,
                parser: Parser::default(),
                read_failed: false,
            }
        }
        pub fn reading(&self) -> RadarReading {
            self.parser.reading()
        }
        pub fn poll(&mut self, now: u64) -> PresenceStatus {
            if let Some(uart) = &self.uart {
                let mut bytes = [0; 128];
                for _ in 0..8 {
                    match uart.read(&mut bytes, 0) {
                        Ok(0) => break,
                        Ok(count) => {
                            self.parser.feed(&bytes[..count], now);
                            self.read_failed = false;
                        }
                        // ESP-IDF HAL reports an empty nonblocking read as timeout.
                        // Retain fragmented reports and let the one-second timer expire them.
                        Err(error) if error.code() == esp_idf_svc::sys::ESP_ERR_TIMEOUT => break,
                        Err(error) => {
                            if !self.read_failed {
                                log::warn!("Radar UART read failed: {error}");
                            }
                            self.read_failed = true;
                            self.parser = Parser::default();
                            break;
                        }
                    }
                }
            }
            self.parser.tick(now);
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

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(kind: u8, state: u8) -> Vec<u8> {
        let mut payload = vec![
            kind, 0xaa, state, 0x34, 0x12, 75, 0x78, 0x56, 30, 0x9a, 0x02,
        ];
        if kind == 1 {
            payload.extend_from_slice(&[
                8, 8, 10, 20, 30, 40, 50, 60, 70, 80, 90, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]);
        }
        payload.extend_from_slice(&[0x55, 0]);
        let mut frame = vec![0xf4, 0xf3, 0xf2, 0xf1];
        frame.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        frame.extend_from_slice(&payload);
        frame.extend_from_slice(&[0xf8, 0xf7, 0xf6, 0xf5]);
        frame
    }
    #[test]
    fn basic_and_engineering_target_states_and_little_endian_metrics() {
        for kind in [1, 2] {
            for state in 0..4 {
                let mut parser = Parser::default();
                parser.feed(&frame(kind, state), 0);
                let reading = parser.reading();
                assert!(reading.connected);
                assert_eq!(reading.present, state != 0);
                assert_eq!(reading.moving_target, state & 1 != 0);
                assert_eq!(reading.stationary_target, state & 2 != 0);
                assert_eq!(reading.moving_distance_cm, 0x1234);
                assert_eq!(reading.moving_energy, 75);
                assert_eq!(reading.stationary_distance_cm, 0x5678);
                assert_eq!(reading.stationary_energy, 30);
                assert_eq!(reading.detection_distance_cm, 0x029a);
            }
        }
    }
    #[test]
    fn fragmented_consecutive_reports_and_noise() {
        let mut parser = Parser::default();
        parser.feed(&[4, 3, 2, 1, 0xf4, 0xf3], 0);
        for byte in frame(2, 1) {
            parser.feed(&[byte], 100);
        }
        assert!(parser.reading().moving_target);
        let reports = [frame(1, 2), frame(2, 0)].concat();
        parser.feed(&reports, 200);
        assert!(parser.reading().connected);
        assert!(!parser.reading().present);
    }
    #[test]
    fn invalid_frames_resynchronize_without_publishing_presence() {
        for offset in [0, 4, 6, 7, 8, 17, 18, 19, 22] {
            let mut parser = Parser::default();
            let mut bad = frame(2, 3);
            bad[offset] = 0xff;
            parser.feed(&bad, 0);
            assert!(!parser.reading().connected, "offset {offset}");
            parser.feed(&frame(2, 2), 100);
            assert!(parser.reading().stationary_target, "offset {offset}");
        }
        let mut parser = Parser::default();
        parser.feed(&[0xff; 500], 0);
        parser.feed(&frame(2, 1), 100);
        assert!(parser.reading().moving_target);
    }
    #[test]
    fn stale_reports_clear_measurements_and_out_remains_independent() {
        let mut parser = Parser::default();
        parser.feed(&frame(2, 3), 100);
        parser.tick(1099);
        assert!(parser.reading().present);
        parser.feed(&[0xff; 20], 1099);
        parser.tick(1100);
        assert_eq!(parser.reading(), RadarReading::default());
        assert_eq!(
            PresenceStatus::from_reading(parser.reading(), false),
            PresenceStatus::Unavailable
        );
        assert_eq!(
            PresenceStatus::from_reading(parser.reading(), true),
            PresenceStatus::Present
        );
        parser.feed(&frame(2, 0), 1200);
        assert_eq!(
            PresenceStatus::from_reading(parser.reading(), false),
            PresenceStatus::Absent
        );
        assert_eq!(
            PresenceStatus::from_reading(parser.reading(), true),
            PresenceStatus::Present
        );
        parser.feed(&frame(2, 1), 1300);
        assert_eq!(
            PresenceStatus::from_reading(parser.reading(), false),
            PresenceStatus::Present
        );
    }
}
