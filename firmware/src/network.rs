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

fn list(wifi: &mut EspWifi<'static>) -> Result<Vec<Network>, Failure> {
    wifi.driver_mut()
        .get_scan_result()
        .map_err(|_| Failure::Scan)
        .map(|aps| {
            aps.into_iter()
                .map(|ap| Network {
                    ssid: ap.ssid.to_string(),
                    rssi: ap.signal_strength,
                    security: match ap.auth_method {
                        Some(AuthMethod::WPA2Personal) => Security::Wpa2,
                        Some(AuthMethod::WPA3Personal) => Security::Wpa3,
                        Some(AuthMethod::WPA2WPA3Personal) => Security::Mixed,
                        _ => Security::Unsupported,
                    },
                })
                .collect()
        })
}
fn configure(wifi: &mut EspWifi<'static>, c: &Credentials) -> Result<(), Failure> {
    storage::use_ram(wifi).map_err(|_| Failure::Connection)?;
    let _ = wifi.disconnect();
    let authmode = match c.security {
        Security::Wpa2 => wifi_auth_mode_t_WIFI_AUTH_WPA2_PSK,
        Security::Wpa3 => wifi_auth_mode_t_WIFI_AUTH_WPA3_PSK,
        Security::Mixed => wifi_auth_mode_t_WIFI_AUTH_WPA2_WPA3_PSK,
        Security::Unsupported => return Err(Failure::Connection),
    };
    let mut config = wifi_config_t::default();
    // SDK makes its own RAM-only copy. Wipe this temporary native structure
    // immediately after the call; do not format or inspect its contents.
    unsafe {
        let sta = &mut config.sta;
        sta.ssid[..c.ssid.len()].copy_from_slice(c.ssid.as_bytes());
        sta.password[..c.password.len()].copy_from_slice(c.password.as_bytes());
        sta.threshold.rssi = -127;
        sta.threshold.authmode = authmode;
        sta.pmf_cfg.capable = true;
        sta.pmf_cfg.required = c.security == Security::Wpa3;
        sta.sae_pwe_h2e = wifi_sae_pwe_method_t_WPA3_SAE_PWE_BOTH;
    }
    let code = unsafe { esp_wifi_set_config(wifi_interface_t_WIFI_IF_STA, &mut config) };
    unsafe {
        std::slice::from_raw_parts_mut(
            (&mut config as *mut wifi_config_t).cast::<u8>(),
            std::mem::size_of::<wifi_config_t>(),
        )
        .zeroize();
    }
    if code != ESP_OK {
        return Err(Failure::Connection);
    }
    Ok(())
}
fn clear_driver(wifi: &mut EspWifi<'static>) {
    let _ = wifi.disconnect();
    let _ = wifi.driver_mut().stop_scan();
    // If returning to RAM fails, disconnect but never write a blank flash profile.
    if storage::use_ram(wifi).is_err() {
        return;
    }
    let mut blank = wifi_config_t::default();
    unsafe {
        esp_wifi_set_config(wifi_interface_t_WIFI_IF_STA, &mut blank);
    }
}
fn authentication_reason(reason: u16) -> bool {
    matches!(reason, 15 | 16 | 23 | 202 | 204)
}
pub fn run(
    modem: Modem<'static>,
    nvs: EspDefaultNvsPartition,
    rx: Receiver<Command>,
    tx: Sender<Event>,
    online: Arc<std::sync::atomic::AtomicBool>,
) {
    let result = run_inner(modem, nvs, rx, &tx, &online);
    online.store(false, Ordering::Relaxed);
    if result.is_err() {
        let _ = tx.send(Event::Fatal(
            "Wi-Fi initialization failed. Restart the device or check firmware configuration.",
        ));
    }
}
fn run_inner(
    modem: Modem<'static>,
    nvs: EspDefaultNvsPartition,
    rx: Receiver<Command>,
    tx: &Sender<Event>,
    online: &std::sync::atomic::AtomicBool,
) -> anyhow::Result<()> {
    let sysloop = EspSystemEventLoop::take()?;
    let reason = Arc::new(AtomicU16::new(0));
    let callback_reason = reason.clone();
    let _subscription = sysloop.subscribe::<WifiEvent, _>(move |event| {
        if let WifiEvent::StaDisconnected(data) = event {
            callback_reason.store(data.reason(), Ordering::Relaxed);
        }
    })?;
    // SDK initialization loads the native saved station profile. Set mode only:
    // a default ClientConfiguration would overwrite the loaded credentials.
    let mut wifi = EspWifi::new(modem, sysloop, Some(nvs))?;
    storage::use_ram(&mut wifi).map_err(anyhow::Error::msg)?;
    esp_idf_svc::sys::esp!(unsafe { esp_wifi_set_mode(wifi_mode_t_WIFI_MODE_STA) })?;
    wifi.start()?;
    let backend = EspBackend { wifi, reason };
    let mut service = match Service::new(backend) {
        Ok(service) => service,
        Err(text) => {
            let _ = tx.send(Event::Fatal(text));
            return Ok(());
        }
    };

    tx.send(service.ready())?;
    let started = Instant::now();
    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(command) => {
                if let Some(event) = service.command(command, started.elapsed().as_millis() as u64)
                {
                    tx.send(event)?;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
        }
        if let Some(event) = service.poll(started.elapsed().as_millis() as u64) {
            tx.send(event)?;
        }
        online.store(service.connected(), Ordering::Relaxed);
    }
    Ok(())
}

struct EspBackend {
    wifi: EspWifi<'static>,
    reason: Arc<AtomicU16>,
}
impl Backend for EspBackend {
    fn clear(&mut self) {
        clear_driver(&mut self.wifi);
        self.reason.store(0, Ordering::Relaxed);
    }
    fn begin_scan(&mut self) -> Result<(), Failure> {
        self.wifi
            .start_scan(&Default::default(), false)
            .map_err(|_| Failure::Scan)
    }
    fn scan_result(&mut self) -> Result<Option<Vec<Network>>, Failure> {
        if self
            .wifi
            .driver()
            .is_scan_done()
            .map_err(|_| Failure::Scan)?
        {
            list(&mut self.wifi).map(Some)
        } else {
            Ok(None)
        }
    }
    fn configure(&mut self, c: &Credentials) -> Result<(), Failure> {
        configure(&mut self.wifi, c)?;
        self.reason.store(0, Ordering::Relaxed);
        Ok(())
    }
    fn connect(&mut self) -> Result<(), Failure> {
        self.wifi.connect().map_err(|_| Failure::Connection)
    }
    fn associated(&self) -> bool {
        self.wifi.is_connected().unwrap_or(false)
    }
    fn up(&self) -> bool {
        self.wifi.is_up().unwrap_or(false)
    }
    fn authentication_failed(&self) -> bool {
        authentication_reason(self.reason.load(Ordering::Relaxed))
    }
    fn network_missing(&self) -> bool {
        self.reason.load(Ordering::Relaxed) == 201
    }
}
impl CredentialStore for EspBackend {
    fn forget(&mut self) -> Result<(), &'static str> {
        storage::forget(&mut self.wifi)
    }
    fn load(&self) -> Result<Option<Credentials>, &'static str> {
        storage::load(&self.wifi)
    }
    fn save(&mut self, c: &Credentials) -> Result<(), &'static str> {
        storage::save(&mut self.wifi, c)
    }
}
