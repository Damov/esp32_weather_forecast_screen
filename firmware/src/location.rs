// ============================================================================= //
// File          : location.rs                                                   //
// License       : GPL-3.0-only                                                  //
// Created       : 2026-10-04                                                    //
// Last modified : 2026-10-04                                                    //
// Author        : Daniel Kharlamov                                              //
// Last editor   : Daniel Kharlamov <daniel.kharlamov@googlemail.com>            //
// ============================================================================= //
//                                                                               //
// Summary:                                                                      //
// --------                                                                      //
// Location search and the device HTTP request worker.                           //
//                                                                               //
// Module:                                                                       //
// -------                                                                       //
// Encodes Open-Meteo geocoding requests and validates bounded location results. //
// Defines forecast, search, and timezone request and response messages. On      //
// ESP32, runs time synchronization and bounded HTTPS requests in a worker,      //
// gated by connectivity and synchronized time.                                  //
// ============================================================================= //

use crate::{i18n::Language, settings::Location};
use serde::Deserialize;

pub enum Request {
    Forecast {
        id: u64,
        location: Location,
    },
    Search {
        id: u64,
        query: String,
        language: Language,
    },
    Timezone {
        id: u64,
        location: Location,
    },
}
pub enum Response {
    Forecast(
        u64,
        Location,
        Result<crate::weather::Forecast, &'static str>,
    ),
    Locations(u64, Result<Vec<Location>, &'static str>),
    Timezone(u64, Result<Location, &'static str>),
}
const FAILED: &str = "Location search failed. Please retry.";
pub fn url(query: &str, language: Language) -> String {
    let encoded: String = query
        .trim()
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={encoded}&count=15&language={}",
        language.code()
    )
}
#[derive(Deserialize)]
struct Results {
    #[serde(default)]
    error: bool,
    #[serde(default)]
    results: Vec<ResultLocation>,
}
#[derive(Deserialize)]
struct ResultLocation {
    name: String,
    #[serde(default)]
    admin1: String,
    #[serde(default)]
    country: String,
    latitude: f64,
    longitude: f64,
    timezone: String,
}
pub fn parse(bytes: &[u8]) -> Result<Vec<Location>, &'static str> {
    if bytes.len() > 32768 {
        return Err(FAILED);
    }
    let data: Results = serde_json::from_slice(bytes).map_err(|_| FAILED)?;
    if data.error || data.results.len() > 15 {
        return Err(FAILED);
    }
    data.results
        .into_iter()
        .map(|v| {
            let city = v.name.trim();
            let country = v.country.trim();
            let compact_name = if country.is_empty() || city == country {
                city.to_owned()
            } else {
                format!("{city}, {country}")
            };
            let mut name = v.name;
            for suffix in [&v.admin1, &v.country] {
                if !suffix.is_empty() && !name.ends_with(suffix) {
                    name.push_str(", ");
                    name.push_str(suffix);
                }
            }
            let location = Location {
                name,
                compact_name: Some(compact_name),
                latitude: v.latitude,
                longitude: v.longitude,
                timezone: v.timezone,
            };
            if location.timezone.is_empty() || !location.valid() {
                return Err(FAILED);
            }
            Ok(location)
        })
        .collect()
}

#[cfg(target_os = "espidf")]
pub fn run(
    rx: std::sync::mpsc::Receiver<Request>,
    tx: std::sync::mpsc::Sender<Response>,
    online: std::sync::Arc<std::sync::atomic::AtomicBool>,
    synchronized: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    use std::sync::atomic::Ordering;
    let mut sntp = None;
    loop {
        // Wi-Fi initializes lwIP before reporting an assigned IP address.
        if sntp.is_none() && online.load(Ordering::Relaxed) {
            let sync = synchronized.clone();
            sntp = esp_idf_svc::sntp::EspSntp::new_with_callback(&Default::default(), move |_| {
                sync.store(true, Ordering::Relaxed);
            })
            .ok();
        }
        let request = match rx.recv_timeout(std::time::Duration::from_millis(250)) {
            Ok(request) => request,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        };
        let ready = if !online.load(Ordering::Relaxed) {
            Err("Connect to Wi-Fi to search.")
        } else if !synchronized.load(Ordering::Relaxed) {
            Err("Waiting for time synchronization.")
        } else {
            Ok(())
        };
        let response = match request {
            Request::Forecast { id, location } => {
                let result = ready
                    .and_then(|_| fetch(&crate::weather::url(&location)))
                    .and_then(|bytes| crate::weather::parse(&bytes));
                Response::Forecast(id, location, result)
            }
            Request::Search {
                id,
                query,
                language,
            } => Response::Locations(
                id,
                ready
                    .and_then(|_| fetch(&url(&query, language)))
                    .and_then(|bytes| parse(&bytes)),
            ),
            Request::Timezone { id, mut location } => {
                let url = format!("https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&timezone=auto&forecast_days=1", location.latitude, location.longitude);
                let result = ready.and_then(|_| fetch(&url)).and_then(|bytes| {
                    #[derive(Deserialize)]
                    struct Zone {
                        timezone: String,
                    }
                    let zone: Zone = serde_json::from_slice(&bytes).map_err(|_| FAILED)?;
                    location.timezone = zone.timezone;
                    if location.timezone.is_empty() || !location.valid() {
                        return Err(FAILED);
                    }
                    Ok(location)
                });
                Response::Timezone(id, result)
            }
        };
        if tx.send(response).is_err() {
            break;
        }
    }
}
#[cfg(target_os = "espidf")]
fn fetch(url: &str) -> Result<Vec<u8>, &'static str> {
    use esp_idf_svc::{
        http::{
            client::{Configuration, EspHttpConnection},
            Method,
        },
        sys,
    };
    use std::time::{Duration, Instant};
    let mut http = EspHttpConnection::new(&Configuration {
        timeout: Some(Duration::from_secs(10)),
        crt_bundle_attach: Some(sys::esp_crt_bundle_attach),
        ..Default::default()
    })
    .map_err(|_| FAILED)?;
    let started = Instant::now();
    http.initiate_request(Method::Get, url, &[("Accept", "application/json")])
        .map_err(|_| FAILED)?;
    http.initiate_response().map_err(|_| FAILED)?;
    if http.status() != 200 {
        return Err(FAILED);
    }
    if http
        .header("Content-Length")
        .and_then(|v| v.parse::<usize>().ok())
        .is_some_and(|n| n > 32768)
    {
        return Err(FAILED);
    }
    let mut result = Vec::new();
    let mut buffer = [0u8; 512];
    loop {
        if started.elapsed() >= Duration::from_secs(10) {
            return Err(FAILED);
        }
        let count = http.read(&mut buffer).map_err(|_| FAILED)?;
        if count == 0 {
            return Ok(result);
        }
        if result.len() + count > 32768 {
            return Err(FAILED);
        }
        result.extend_from_slice(&buffer[..count]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_keeps_region_for_selection_but_uses_city_and_country_for_display() {
        for (region, country, full, compact) in [
            (
                "Bayern",
                "Deutschland",
                "München, Bayern, Deutschland",
                "München, Deutschland",
            ),
            ("Bayern", "", "München, Bayern", "München"),
            (
                "",
                "Deutschland",
                "München, Deutschland",
                "München, Deutschland",
            ),
        ] {
            let body = serde_json::json!({"results": [{"name": "München", "admin1": region,
                "country": country, "latitude": 48.1, "longitude": 11.6, "timezone": "Europe/Berlin"}]});
            let locations = parse(&serde_json::to_vec(&body).unwrap()).unwrap();
            assert_eq!(locations[0].name, full);
            assert_eq!(locations[0].display_name(), compact);
        }
    }

    #[test]
    fn encodes_unicode_and_delimiters_without_query_injection() {
        let url = url(" Café &x=1 ", Language::French);
        assert!(url.contains("name=Caf%C3%A9%20%26x%3D1&count=15&language=fr"));
        assert!(super::url("Москва", Language::Russian)
            .contains("name=%D0%9C%D0%BE%D1%81%D0%BA%D0%B2%D0%B0&count=15&language=ru"));
    }
    #[test]
    fn parses_results_and_rejects_bad_data() {
        let data = br#"{"results":[{"name":"London","country":"United Kingdom","latitude":51.5,"longitude":-0.1,"timezone":"Europe/London"}]}"#;
        let result = parse(data).unwrap();
        assert_eq!(result[0].name, "London, United Kingdom");
        assert_eq!(result[0].timezone, "Europe/London");
        assert!(parse(br#"{"generationtime_ms":0.1}"#).unwrap().is_empty());
        assert!(parse(br#"{"error":true}"#).is_err());
        assert!(parse(b"invalid JSON").is_err());
        assert!(parse(&vec![0; 32769]).is_err());
        let invalid = String::from_utf8(data.to_vec())
            .unwrap()
            .replace("Europe/London", "Not/AZone");
        assert!(parse(invalid.as_bytes()).is_err());
        let invalid = String::from_utf8(data.to_vec())
            .unwrap()
            .replace("51.5", "91.5");
        assert!(parse(invalid.as_bytes()).is_err());
    }
}
