// ============================================================================= //
// File          : model.rs                                                      //
// License       : GPL-3.0-only                                                  //
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

use zeroize::{Zeroize, Zeroizing};

pub const CONNECT_TIMEOUT_SECS: u64 = 20;
pub const RETRY_SECS: u64 = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Security {
    Wpa2,
    Wpa3,
    Mixed,
    Unsupported,
}
impl Security {
    pub fn label(self) -> &'static str {
        match self {
            Self::Wpa2 => "WPA2",
            Self::Wpa3 => "WPA3",
            Self::Mixed => "WPA2/3",
            Self::Unsupported => "Unsupported",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Network {
    pub ssid: String,
    pub security: Security,
    pub rssi: i8,
}

// Deliberately no Debug: commands and records must never log secrets.
pub struct Credentials {
    pub ssid: String,
    pub security: Security,
    pub password: Zeroizing<String>,
}
impl Credentials {
    pub fn new(network: &Network, password: &str) -> Result<Self, &'static str> {
        if network.ssid.is_empty() || network.ssid.len() > 32 {
            return Err("Invalid network name.");
        }
        if network.security == Security::Unsupported {
            return Err("This network security is unsupported.");
        }
        if !(8..=63).contains(&password.len()) || !password.bytes().all(|b| (32..=126).contains(&b))
        {
            return Err("Use a password of 8-63 characters.");
        }
        Ok(Self {
            ssid: network.ssid.clone(),
            security: network.security,
            password: Zeroizing::new(password.to_owned()),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Missing,
    NoNetworks,
    Authentication,
    IpTimeout,
    Scan,
    Connection,
    Save,
}
impl Failure {
    pub fn text(self) -> &'static str {
        match self {
            Self::Missing => "The saved network is unavailable.",
            Self::NoNetworks => "No Wi-Fi networks found.",
            Self::Authentication => "Authentication failed. Check the password.",
            Self::IpTimeout => "Could not obtain an IP address.",
            Self::Scan => "Wi-Fi scan failed. Please retry.",
            Self::Connection => "Could not connect. Please retry.",
            Self::Save => "Could not save Wi-Fi. Please try again.",
        }
    }
}

pub enum Command {
    Scan(u64),
    Retry(u64),
    Connect(u64, Credentials),
    Suspend(u64),
    Reset(u64),
}
pub enum Event {
    Ready(Option<String>),
    Fatal(&'static str),
    Scanned(u64, Result<Vec<Network>, Failure>),
    Connected(u64, String),
    Failed(u64, Failure),
    Lost,
    Reset(u64, bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Starting,
    Problem,
    Networks,
    Password,
    Connecting,
    Connected,
    Fatal,
}

pub struct App {
    pub weather: crate::weather::Weather,
    pub radar: crate::radar::PresenceStatus,
    pub preferences: crate::settings::Preferences,
    pub page: Page,
    pub saved_ssid: Option<String>,
    pub selected: Option<Network>,
    pub networks: Vec<Network>,
    pub password: Zeroizing<String>,
    pub show_password: bool,
    pub message: String,
    pub offset: usize,
    pub keyboard: u8,
    pub scanning: bool,
    pub generation: u64,
    pub retry_at: Option<u64>,
}
impl Default for App {
    fn default() -> Self {
        Self {
            weather: Default::default(),
            radar: Default::default(),
            preferences: Default::default(),
            page: Page::Starting,
            saved_ssid: None,
            selected: None,
            networks: vec![],
            password: Zeroizing::new(String::new()),
            show_password: false,
            message: "Starting Wi-Fi...".into(),
            offset: 0,
            keyboard: 0,
            scanning: false,
            generation: 0,
            retry_at: None,
        }
    }
}
impl App {
    fn next(&mut self) -> u64 {
        self.generation += 1;
        self.generation
    }
    pub fn handle(&mut self, event: Event, now: u64) -> Option<Command> {
        if self.page == Page::Fatal && !matches!(&event, Event::Fatal(_)) {
            return None;
        }
        match event {
            Event::Reset(id, success) if id == self.generation => {
                self.preferences.reset_pending = false;
                if success {
                    self.saved_ssid = None;
                    self.selected = None;
                    self.clear_secret();
                    self.preferences.leave(None);
                    self.page = Page::Problem;
                    self.retry_at = None;
                    self.message = "No Wi-Fi network is configured.".into();
                    self.scanning = true;
                    return Some(Command::Scan(self.next()));
                }
                self.preferences.status = "Wi-Fi reset failed. Please retry.";
                if self.saved_ssid.is_some() {
                    self.page = Page::Problem;
                    self.retry_at = Some(now + RETRY_SECS);
                }
            }
            Event::Ready(ssid) => {
                self.saved_ssid = ssid;
                if self.saved_ssid.is_some() {
                    return Some(self.retry());
                }
                self.page = Page::Problem;
                self.message = "No Wi-Fi network is configured.".into();
                self.scanning = true;
                let id = self.next();
                return Some(Command::Scan(id));
            }
            Event::Fatal(text) => {
                self.clear_secret();
                self.next();
                self.page = Page::Fatal;
                self.message = text.into();
                self.retry_at = None;
            }
            Event::Scanned(id, result) if id == self.generation => {
                self.scanning = false;
                match result {
                    Ok(mut networks) => {
                        networks.retain(|n| !n.ssid.is_empty());
                        networks.sort_by_key(|n| std::cmp::Reverse(n.rssi));
                        // Same name and security represent the same selectable network.
                        let mut unique: Vec<Network> = vec![];
                        for n in networks {
                            if !unique
                                .iter()
                                .any(|v| v.ssid == n.ssid && v.security == n.security)
                            {
                                unique.push(n);
                            }
                        }
                        self.networks = unique;
                        self.offset = 0;
                        if self.page == Page::Networks {
                            self.message = if self.networks.is_empty() {
                                Failure::NoNetworks.text().into()
                            } else {
                                "Select a secured 2.4 GHz network.".into()
                            };
                        } else if self.saved_ssid.is_none() && self.networks.is_empty() {
                            self.message =
                                "No Wi-Fi is configured. No Wi-Fi networks found.".into();
                        }
                    }
                    Err(reason) => self.message = reason.text().into(),
                }
            }
            Event::Connected(id, ssid) if id == self.generation => {
                self.saved_ssid = Some(ssid);
                self.page = Page::Connected;
                self.message = "Wi-Fi connected.".into();
                self.retry_at = None;
                self.clear_secret();
            }
            Event::Failed(id, reason) if id == self.generation => {
                self.message = reason.text().into();
                if self.selected.is_some() {
                    self.page = Page::Password;
                } else {
                    self.page = Page::Problem;
                    self.retry_at = Some(now + RETRY_SECS);
                }
            }
            Event::Lost if self.page == Page::Connected => return Some(self.retry()),
            _ => {}
        }
        None
    }
    fn clear_secret(&mut self) {
        self.password.zeroize();
        self.show_password = false;
    }
    pub fn setup(&mut self) -> Command {
        self.clear_secret();
        self.selected = None;
        self.retry_at = None;
        self.page = Page::Networks;
        self.scanning = true;
        self.message = "Scanning...".into();
        self.offset = 0;
        let id = self.next();
        Command::Scan(id)
    }
    pub fn retry(&mut self) -> Command {
        self.clear_secret();
        self.selected = None;
        self.page = Page::Connecting;
        self.message = "Reconnecting...".into();
        self.retry_at = None;
        let id = self.next();
        Command::Retry(id)
    }
    pub fn tick(&mut self, now: u64) -> Option<Command> {
        if self.page == Page::Problem
            && self.saved_ssid.is_some()
            && self.retry_at.is_some_and(|t| now >= t)
        {
            // Background retries retain the problem page and its setup button.
            self.retry_at = None;
            let id = self.next();
            return Some(Command::Retry(id));
        }
        None
    }
    pub fn select(&mut self, index: usize) {
        if let Some(network) = self.networks.get(index) {
            if network.security == Security::Unsupported {
                self.message = "Open, WEP and enterprise Wi-Fi are unsupported.".into();
                return;
            }
            self.selected = Some(network.clone());
            self.clear_secret();
            self.page = Page::Password;
            self.keyboard = 0;
            self.message = "Enter the Wi-Fi password.".into();
        }
    }
    pub fn connect(&mut self) -> Option<Command> {
        let credentials = Credentials::new(self.selected.as_ref()?, &self.password);
        match credentials {
            Ok(c) => {
                self.clear_secret();
                self.page = Page::Connecting;
                self.message = "Connecting...".into();
                let id = self.next();
                Some(Command::Connect(id, c))
            }
            Err(text) => {
                self.message = text.into();
                None
            }
        }
    }
    pub fn back(&mut self) -> Command {
        self.clear_secret();
        if self.selected.take().is_some() {
            self.page = Page::Networks;
            self.message = "Select a secured 2.4 GHz network.".into();
        } else {
            self.page = Page::Problem;
            self.message = if self.saved_ssid.is_some() {
                "Wi-Fi setup cancelled.".into()
            } else {
                "No Wi-Fi network is configured.".into()
            };
            self.retry_at = Some(0);
        }
        self.scanning = false;
        let id = self.next();
        Command::Suspend(id)
    }
    pub fn type_char(&mut self, ch: char) {
        if ch.is_ascii() && !ch.is_ascii_control() && self.password.len() < 63 {
            self.password.push(ch);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn network() -> Network {
        Network {
            ssid: "Home".into(),
            security: Security::Wpa2,
            rssi: -30,
        }
    }
    #[test]
    fn password_validation() {
        assert!(Credentials::new(&network(), "short").is_err());
        assert!(Credentials::new(&network(), &"a".repeat(64)).is_err());
        assert!(Credentials::new(&network(), "password\n").is_err());
        assert!(Credentials::new(&network(), "secret123").is_ok());
    }
    #[test]
    fn no_configuration_empty_scan_and_scan_failure() {
        let mut app = App::default();
        assert!(matches!(
            app.handle(Event::Ready(None), 0),
            Some(Command::Scan(_))
        ));
        app.handle(Event::Scanned(app.generation, Ok(vec![])), 0);
        assert!(app.message.contains("No Wi-Fi networks"));
        app.handle(Event::Scanned(app.generation, Err(Failure::Scan)), 0);
        assert_eq!(app.message, Failure::Scan.text());
    }
    #[test]
    fn failures_preserve_saved_network_and_cancel_clears_secret() {
        let mut app = App::default();
        app.handle(Event::Ready(Some("Old".into())), 0);
        app.setup();
        app.networks = vec![network()];
        app.select(0);
        app.password.push_str("secret123");
        app.connect();
        app.handle(Event::Failed(app.generation, Failure::Authentication), 20);
        assert_eq!(app.page, Page::Password);
        assert_eq!(app.saved_ssid.as_deref(), Some("Old"));
        assert!(app.password.is_empty());
        app.password.push_str("secret123");
        app.show_password = true;
        app.back();
        assert!(app.password.is_empty());
        assert!(!app.show_password);
    }
    #[test]
    fn reconnect_delay_background_retry_and_stale_events() {
        let mut app = App::default();
        app.handle(Event::Ready(Some("Home".into())), 0);
        app.handle(Event::Failed(app.generation, Failure::Missing), 20);
        assert!(app.tick(49).is_none());
        assert!(matches!(app.tick(50), Some(Command::Retry(_))));
        assert_eq!(app.page, Page::Problem);
        let stale = app.generation;
        app.setup();
        app.handle(Event::Connected(stale, "Home".into()), 51);
        assert_eq!(app.page, Page::Networks);
    }
    #[test]
    fn successful_connection_and_loss() {
        let mut app = App::default();
        app.handle(Event::Ready(Some("Home".into())), 0);
        app.handle(Event::Connected(app.generation, "Home".into()), 2);
        assert_eq!(app.page, Page::Connected);
        assert!(matches!(
            app.handle(Event::Lost, 3),
            Some(Command::Retry(_))
        ));
        assert_eq!(app.page, Page::Connecting);
    }
    #[test]
    fn sorted_deduplicated_and_password_bounds() {
        let mut app = App::default();
        app.setup();
        let mut weak = network();
        weak.rssi = -80;
        app.handle(Event::Scanned(app.generation, Ok(vec![weak, network()])), 0);
        assert_eq!(app.networks.len(), 1);
        assert_eq!(app.networks[0].rssi, -30);
        for _ in 0..100 {
            app.type_char('a');
        }
        app.type_char('\n');
        assert_eq!(app.password.len(), 63);
        app.handle(Event::Fatal("Storage unavailable"), 0);
        assert!(app.password.is_empty());
        app.handle(Event::Ready(Some("Home".into())), 1);
        assert_eq!(app.page, Page::Fatal);
    }
}
