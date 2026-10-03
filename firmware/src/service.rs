//! Hardware-independent connection worker; time is monotonic milliseconds.
use crate::model::{Command, Credentials, Event, Failure, Network, CONNECT_TIMEOUT_SECS};

pub trait Backend: CredentialStore {
    fn clear(&mut self);
    fn begin_scan(&mut self) -> Result<(), Failure>;
    fn scan_result(&mut self) -> Result<Option<Vec<Network>>, Failure>;
    fn configure(&mut self, c: &Credentials) -> Result<(), Failure>;
    fn connect(&mut self) -> Result<(), Failure>;
    fn associated(&self) -> bool;
    fn up(&self) -> bool;
    fn authentication_failed(&self) -> bool;
    fn network_missing(&self) -> bool {
        false
    }
}
pub trait CredentialStore {
    fn load(&self) -> Result<Option<Credentials>, &'static str>;
    fn save(&mut self, c: &Credentials) -> Result<(), &'static str>;
    fn forget(&mut self) -> Result<(), &'static str>;
}
enum Job {
    Scan {
        id: u64,
        deadline: u64,
    },
    Connect {
        id: u64,
        candidate: Option<Credentials>,
        deadline: u64,
        scanning: bool,
        unavailable: Option<Failure>,
        next_attempt: u64,
        associated: bool,
    },
}
pub struct Service<B> {
    backend: B,
    saved: Option<Credentials>,
    job: Option<Job>,
    connected: bool,
}
impl<B: Backend> Service<B> {
    pub fn new(backend: B) -> Result<Self, &'static str> {
        let saved = backend.load()?;
        Ok(Self {
            backend,
            saved,
            job: None,
            connected: false,
        })
    }
    pub fn connected(&self) -> bool {
        self.connected && self.backend.up()
    }
    pub fn ready(&self) -> Event {
        Event::Ready(self.saved.as_ref().map(|c| c.ssid.clone()))
    }
    pub fn command(&mut self, command: Command, now: u64) -> Option<Event> {
        self.backend.clear();
        self.connected = false;
        self.job = None;
        match command {
            Command::Reset(id) => {
                let success = self.backend.forget().is_ok();
                if success {
                    self.saved = None;
                }
                Some(Event::Reset(id, success))
            }
            Command::Suspend(_) => None,
            Command::Scan(id) => match self.backend.begin_scan() {
                Ok(()) => {
                    self.job = Some(Job::Scan {
                        id,
                        deadline: now + 10_000,
                    });
                    None
                }
                Err(f) => Some(Event::Scanned(id, Err(f))),
            },
            Command::Retry(id) => {
                if self.saved.is_none() {
                    return Some(Event::Failed(id, Failure::Connection));
                }
                if let Err(f) = self.backend.begin_scan() {
                    return Some(Event::Failed(id, f));
                }
                self.job = Some(Job::Connect {
                    id,
                    candidate: None,
                    deadline: now + CONNECT_TIMEOUT_SECS * 1000,
                    scanning: true,
                    unavailable: None,
                    next_attempt: now,
                    associated: false,
                });
                None
            }
            Command::Connect(id, c) => {
                if let Err(f) = self.backend.configure(&c) {
                    return Some(Event::Failed(id, f));
                }
                self.job = Some(Job::Connect {
                    id,
                    candidate: Some(c),
                    deadline: now + CONNECT_TIMEOUT_SECS * 1000,
                    scanning: false,
                    unavailable: None,
                    next_attempt: now,
                    associated: false,
                });
                None
            }
        }
    }
    pub fn poll(&mut self, now: u64) -> Option<Event> {
        let event = match self.job.as_mut() {
            Some(Job::Scan { id, deadline }) => match self.backend.scan_result() {
                Ok(Some(networks)) => Some(Event::Scanned(*id, Ok(networks))),
                Err(f) => Some(Event::Scanned(*id, Err(f))),
                Ok(None) if now >= *deadline => {
                    self.backend.clear();
                    Some(Event::Scanned(*id, Err(Failure::Scan)))
                }
                Ok(None) => None,
            },
            Some(Job::Connect {
                id,
                candidate,
                deadline,
                scanning,
                unavailable,
                next_attempt,
                associated,
            }) => {
                if *scanning {
                    match self.backend.scan_result() {
                        Err(f) => {
                            *scanning = false;
                            *unavailable = Some(f);
                        }
                        Ok(Some(networks)) => {
                            *scanning = false;
                            let c = candidate
                                .as_ref()
                                .or(self.saved.as_ref())
                                .expect("connection profile");
                            if networks.is_empty() {
                                *unavailable = Some(Failure::NoNetworks);
                            } else if !networks.iter().any(|n| n.ssid == c.ssid) {
                                *unavailable = Some(Failure::Missing);
                            } else if let Err(f) = self.backend.configure(c) {
                                *unavailable = Some(f);
                            }
                        }
                        Ok(None) => {}
                    }
                }
                *associated |= self.backend.associated();
                if !*scanning && unavailable.is_none() && self.backend.up() {
                    if let Some(c) = candidate.take() {
                        if self.backend.save(&c).is_err() {
                            self.backend.clear();
                            Some(Event::Failed(*id, Failure::Save))
                        } else {
                            let ssid = c.ssid.clone();
                            self.saved = Some(c);
                            self.connected = true;
                            Some(Event::Connected(*id, ssid))
                        }
                    } else {
                        self.connected = true;
                        Some(Event::Connected(
                            *id,
                            self.saved.as_ref().expect("saved profile").ssid.clone(),
                        ))
                    }
                } else if now >= *deadline {
                    let failure = unavailable.unwrap_or_else(|| {
                        if *scanning {
                            Failure::Scan
                        } else if self.backend.authentication_failed() {
                            Failure::Authentication
                        } else if self.backend.network_missing() {
                            Failure::Missing
                        } else if *associated {
                            Failure::IpTimeout
                        } else {
                            Failure::Connection
                        }
                    });
                    self.backend.clear();
                    Some(Event::Failed(*id, failure))
                } else {
                    if !*scanning
                        && unavailable.is_none()
                        && !self.backend.associated()
                        && now >= *next_attempt
                        && !self.backend.authentication_failed()
                    {
                        // Errors are retried inside the same deadline; no UI blocking.
                        let _ = self.backend.connect();
                        *next_attempt = now + 2000;
                    }
                    None
                }
            }
            None if self.connected && !self.backend.up() => {
                self.connected = false;
                Some(Event::Lost)
            }
            _ => None,
        };
        if event.is_some() {
            self.job = None;
        }
        event
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Security;
    struct FakeBackend {
        scan: Option<Result<Vec<Network>, Failure>>,
        associated: bool,
        up: bool,
        auth_failed: bool,
        attempts: usize,
        store: FakeStore,
    }
    impl Backend for FakeBackend {
        fn clear(&mut self) {
            self.up = false;
            self.associated = false;
            self.auth_failed = false;
        }
        fn begin_scan(&mut self) -> Result<(), Failure> {
            Ok(())
        }
        fn scan_result(&mut self) -> Result<Option<Vec<Network>>, Failure> {
            self.scan.take().transpose()
        }
        fn configure(&mut self, _: &Credentials) -> Result<(), Failure> {
            Ok(())
        }
        fn connect(&mut self) -> Result<(), Failure> {
            self.attempts += 1;
            Ok(())
        }
        fn associated(&self) -> bool {
            self.associated
        }
        fn up(&self) -> bool {
            self.up
        }
        fn authentication_failed(&self) -> bool {
            self.auth_failed
        }
    }
    struct FakeStore {
        ssid: [u8; 32],
        password: [u8; 64],
        fail: bool,
        writes: usize,
    }
    impl CredentialStore for FakeBackend {
        fn forget(&mut self) -> Result<(), &'static str> {
            if self.store.fail {
                return Err("reset failed");
            }
            self.store.ssid.fill(0);
            self.store.password.fill(0);
            Ok(())
        }
        fn load(&self) -> Result<Option<Credentials>, &'static str> {
            crate::storage::decode_station(&self.store.ssid, &self.store.password, Security::Wpa2)
        }
        fn save(&mut self, c: &Credentials) -> Result<(), &'static str> {
            self.store.writes += 1;
            if self.store.fail {
                return Err("storage failed");
            }
            self.store.ssid.fill(0);
            self.store.password.fill(0);
            self.store.ssid[..c.ssid.len()].copy_from_slice(c.ssid.as_bytes());
            self.store.password[..c.password.len()].copy_from_slice(c.password.as_bytes());
            Ok(())
        }
    }
    fn c(ssid: &str) -> Credentials {
        Credentials::new(
            &Network {
                ssid: ssid.into(),
                security: Security::Wpa2,
                rssi: -30,
            },
            "password123",
        )
        .unwrap()
    }
    fn service() -> Service<FakeBackend> {
        let mut ssid = [0; 32];
        ssid[..3].copy_from_slice(b"Old");
        let mut password = [0; 64];
        password[..11].copy_from_slice(b"password123");
        Service::new(FakeBackend {
            scan: None,
            associated: false,
            up: false,
            auth_failed: false,
            attempts: 0,
            store: FakeStore {
                ssid,
                password,
                fail: false,
                writes: 0,
            },
        })
        .unwrap()
    }
    #[test]
    fn save_only_after_ip_then_survive_restart() {
        let mut s = service();
        s.command(Command::Connect(1, c("New")), 0);
        s.poll(0);
        s.backend.associated = true;
        s.poll(1000);
        assert_eq!(s.backend.store.writes, 0);
        s.backend.up = true;
        assert!(matches!(s.poll(2000), Some(Event::Connected(1, _))));
        assert_eq!(s.backend.load().unwrap().unwrap().ssid, "New");
        let restarted = Service::new(s.backend).unwrap();
        assert!(matches!(restarted.ready(),Event::Ready(Some(ssid)) if ssid=="New"));
    }
    #[test]
    fn authentication_failure_never_writes_or_replaces_old() {
        let mut s = service();
        s.command(Command::Connect(1, c("New")), 0);
        s.poll(0);
        s.backend.auth_failed = true;
        assert!(s.poll(19_999).is_none());
        assert!(matches!(
            s.poll(20_000),
            Some(Event::Failed(1, Failure::Authentication))
        ));
        assert_eq!(s.backend.store.writes, 0);
        assert_eq!(s.saved.as_ref().unwrap().ssid, "Old");
    }
    #[test]
    fn storage_failure_preserves_old_and_disconnects_candidate() {
        let mut s = service();
        s.backend.store.fail = true;
        s.command(Command::Connect(1, c("New")), 0);
        s.backend.up = true;
        assert!(matches!(s.poll(100), Some(Event::Failed(1, Failure::Save))));
        assert!(!s.backend.up);
        assert_eq!(s.saved.as_ref().unwrap().ssid, "Old");
        assert_eq!(s.backend.load().unwrap().unwrap().ssid, "Old");
    }
    #[test]
    fn cancel_and_scan_do_not_save_a_connected_candidate() {
        let mut s = service();
        s.command(Command::Connect(1, c("New")), 0);
        s.backend.up = true;
        s.command(Command::Suspend(2), 10);
        assert!(s.poll(20).is_none());
        assert_eq!(s.backend.store.writes, 0);
        s.command(Command::Scan(3), 30);
        s.backend.scan = Some(Err(Failure::Scan));
        assert!(matches!(
            s.poll(40),
            Some(Event::Scanned(3, Err(Failure::Scan)))
        ));
    }
    #[test]
    fn missing_network_and_no_networks_wait_full_recovery_window() {
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
            let mut s = service();
            s.command(Command::Retry(1), 0);
            s.backend.scan = Some(Ok(networks));
            assert!(s.poll(1000).is_none());
            assert!(matches!(s.poll(20_000),Some(Event::Failed(1,f)) if f==failure));
            assert_eq!(s.backend.attempts, 0);
        }
    }
    #[test]
    fn dhcp_timeout_and_connection_loss_are_distinct() {
        let mut s = service();
        s.command(Command::Connect(1, c("New")), 0);
        s.backend.associated = true;
        assert!(matches!(
            s.poll(20_000),
            Some(Event::Failed(1, Failure::IpTimeout))
        ));
        s.command(Command::Connect(2, c("New")), 30_000);
        s.backend.up = true;
        s.poll(30_100);
        s.backend.up = false;
        assert!(matches!(s.poll(30_200), Some(Event::Lost)));
        assert!(s.poll(30_300).is_none());
    }
}
