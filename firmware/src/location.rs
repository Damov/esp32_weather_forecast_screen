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

//! Location search and the device HTTP request worker.

use crate::{i18n::Language, settings::Location};
use serde::Deserialize;

pub enum Request {
    // Represent forecast as a distinct selectable state or action; the matching handler determines
    // its effect.
    Forecast {
        // Request generation carried by a command or response. Stored as u64.
        id: u64,
        // Selected coordinates, location labels, and IANA timezone. Stored as Location.
        location: Location,
    },
    // Represent search as a distinct selectable state or action; the matching handler determines
    // its effect.
    Search {
        // Request generation carried by a command or response. Stored as u64.
        id: u64,
        // City search text or encoded API request being examined. Stored as String.
        query: String,
        // Selected language for interface text and location search. Stored as Language.
        language: Language,
    },
    // Represent timezone as a distinct selectable state or action; the matching handler determines
    // its effect.
    Timezone {
        // Request generation carried by a command or response. Stored as u64.
        id: u64,
        // Selected coordinates, location labels, and IANA timezone. Stored as Location.
        location: Location,
    },
}
pub enum Response {
    // Represent forecast as a distinct selectable state or action; the matching handler determines
    // its effect.
    Forecast(
        u64,
        Location,
        Result<crate::weather::Forecast, &'static str>,
    ),
    // Represent locations as a distinct selectable state or action; the matching handler determines
    // its effect.
    Locations(u64, Result<Vec<Location>, &'static str>),
    // Represent timezone as a distinct selectable state or action; the matching handler determines
    // its effect.
    Timezone(u64, Result<Location, &'static str>),
}
// Use one stable failure message so parser and transport errors can be shown consistently.
const FAILED: &str = "Location search failed. Please retry.";
/// Builds a percent-encoded Open-Meteo geocoding URL for up to 15 results.
///
/// # Arguments
///
/// * `query` (`&str`) - User-entered city search text, trimmed and percent-encoded for the
///   API.
/// * `language` (`Language`) - Supported language used for translated labels or API
///   requests.
///
/// # Returns
///
/// `String` - HTTPS URL containing the trimmed query and requested language code.
pub fn url(query: &str, language: Language) -> String {
    // Keep trimmed UTF-8 query converted bytewise to URL-safe text and percent escapes in this
    // local variable for the following operations.
    let encoded: String = query
        .trim()
        .bytes()
        .map(|b| {
            // Keep URL-unreserved bytes unchanged; percent-encode every other UTF-8 byte so user
            // input cannot add query parameters.
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                // Produce an owned text value rather than a borrowed view.
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
    // API-level error indicator, defaulting to false when absent. Stored as bool.
    error: bool,
    #[serde(default)]
    // Bounded list of geocoding candidates; at most 15 results are accepted. Stored as
    // Vec<ResultLocation>.
    results: Vec<ResultLocation>,
}
#[derive(Deserialize)]
struct ResultLocation {
    // Human-readable label for the selected object. Stored as String.
    name: String,
    #[serde(default)]
    // Administrative region retained in the full selection label. Stored as String.
    admin1: String,
    #[serde(default)]
    // Country name used to distinguish cities with identical names. Stored as String.
    country: String,
    // Latitude in degrees north, bounded to -90 through 90. Stored as f64.
    latitude: f64,
    // Longitude in degrees east, bounded to -180 through 180. Stored as f64.
    longitude: f64,
    // IANA timezone identifier used for local clocks and forecast dates. Stored as String.
    timezone: String,
}
/// Parses and validates bounded geocoding results while retaining full and compact location
/// names.
///
/// # Arguments
///
/// * `bytes` (`&[u8]`) - UTF-8 API JSON response bytes; accepted input is at most 32 KiB.
///
/// # Returns
///
/// `Result<Vec<Location>, &'static str>` - Ok with validated locations, including an empty
/// vector when results are absent; Err with a search failure message.
///
/// # Errors
///
/// Returns an error for invalid JSON, API errors, bodies over 32 KiB, more than 15 results,
/// or invalid coordinates, names, or timezones.
pub fn parse(bytes: &[u8]) -> Result<Vec<Location>, &'static str> {
    // Reject response bodies larger than 32 KiB before deserialization, bounding API memory use.
    if bytes.len() > 32768 {
        // Leave this function now with a failure result for the surrounding operation; later
        // statements are skipped.
        return Err(FAILED);
    }
    // Keep deserialized geocoding envelope with API error flag and location records in this local
    // variable for the following operations.
    let data: Results = serde_json::from_slice(bytes).map_err(|_| FAILED)?;
    // Reject an API error or an unexpectedly large candidate list rather than using untrusted
    // results.
    if data.error || data.results.len() > 15 {
        // Leave this function now with a failure result for the surrounding operation; later
        // statements are skipped.
        return Err(FAILED);
    }
    // Produce the entries collected from the preceding iterator.
    data.results
        .into_iter()
        .map(|v| {
            // Keep trimmed city name used to form the compact display label in this local variable
            // for the following operations.
            let city = v.name.trim();
            // Keep country name used to distinguish cities with identical names in this local
            // variable for the following operations.
            let country = v.country.trim();
            // Keep optional city-and-country label that keeps the weather header concise in this
            // local variable for the following operations.
            let compact_name = if country.is_empty() || city == country {
                // Produce an owned text value rather than a borrowed view.
                city.to_owned()
            } else {
                format!("{city}, {country}")
            };
            // Keep human-readable label for the selected object in this local variable for the
            // following operations.
            let mut name = v.name;
            // Run each listed scenario or target in the declared order; the loop variables select
            // the data for that case.
            for suffix in [&v.admin1, &v.country] {
                // Append only new nonempty administrative/country detail to the full selection
                // label.
                if !suffix.is_empty() && !name.ends_with(suffix) {
                    // Append the supplied text to the existing string without replacing its earlier
                    // contents.
                    name.push_str(", ");
                    // Append the supplied text to the existing string without replacing its earlier
                    // contents.
                    name.push_str(suffix);
                }
            }
            // Keep selected coordinates, location labels, and IANA timezone in this local variable
            // for the following operations.
            let location = Location {
                // Initialize human-readable label for the selected object from the supplied value.
                name,
                // Initialize optional city-and-country label that keeps the weather header concise
                // from the supplied value.
                compact_name: Some(compact_name),
                // Initialize latitude in degrees north, bounded to -90 through 90 from the supplied
                // value.
                latitude: v.latitude,
                // Initialize longitude in degrees east, bounded to -180 through 180 from the
                // supplied value.
                longitude: v.longitude,
                // Initialize IANA timezone identifier used for local clocks and forecast dates from
                // the supplied value.
                timezone: v.timezone,
            };
            // A usable API location must have valid labels, coordinates, and a recognized nonempty
            // timezone.
            if location.timezone.is_empty() || !location.valid() {
                // Leave this function now with a failure result for the surrounding operation;
                // later statements are skipped.
                return Err(FAILED);
            }
            // Return success; the caller receives selected coordinates, location labels, and IANA
            // timezone.
            Ok(location)
        })
        .collect()
}

/// Runs the device HTTP worker and starts SNTP after Wi-Fi becomes available.
///
/// # Arguments
///
/// * `rx` (`std::sync::mpsc::Receiver<Request>`) - Owned channel receiving forecast,
///   geocoding, and timezone requests until shutdown.
/// * `tx` (`std::sync::mpsc::Sender<Response>`) - Channel publishing request results to the
///   main application.
/// * `online` (`std::sync::Arc<std::sync::atomic::AtomicBool>`) - Shared flag indicating
///   Wi-Fi IP readiness.
/// * `synchronized` (`std::sync::Arc<std::sync::atomic::AtomicBool>`) - Shared SNTP flag;
///   the worker sets it once time has synchronized.
///
/// # Returns
///
/// `()` - No value; runs until the request or response channel disconnects. Sends failures
/// for requests made before connectivity or time synchronization.
#[cfg(target_os = "espidf")]
pub fn run(
    rx: std::sync::mpsc::Receiver<Request>,
    tx: std::sync::mpsc::Sender<Response>,
    online: std::sync::Arc<std::sync::atomic::AtomicBool>,
    synchronized: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    use std::sync::atomic::Ordering;
    // Keep optional live time-synchronization client started after Wi-Fi initialization in this
    // local variable for the following operations.
    let mut sntp = None;
    // Keep processing events or samples until an explicit break, return, or channel shutdown ends
    // this loop.
    loop {
        // Wi-Fi initializes lwIP before reporting an assigned IP address.
        // Start SNTP only after Wi-Fi has initialized the network stack, and retain one client
        // rather than restarting it every poll.
        if sntp.is_none() && online.load(Ordering::Relaxed) {
            // Obtain an owned clone of shared synchronization flag captured by the SNTP completion
            // callback; reference-counted handles continue to share their underlying state.
            let sync = synchronized.clone();
            // Set optional live time-synchronization client started after Wi-Fi initialization to
            // ok result for the surrounding operation.
            sntp = esp_idf_svc::sntp::EspSntp::new_with_callback(&Default::default(), move |_| {
                // Publish the new shared atomic value for the worker or application to observe.
                sync.store(true, Ordering::Relaxed);
            })
            .ok();
        }
        // Keep work item received by the HTTP or networking worker in this local variable for the
        // following operations.
        let request = match rx.recv_timeout(std::time::Duration::from_millis(250)) {
            // Continue with the successful result, using its validated value in this case.
            Ok(request) => request,
            // Skip the remainder of this iteration and wait for or inspect the next input.
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            // Stop this loop now; the enclosing function continues with the work after the loop.
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        };
        // Keep whether connectivity and synchronized time permit the next request in this local
        // variable for the following operations.
        let ready = if !online.load(Ordering::Relaxed) {
            // Return the failure result so the caller can display an error, retry, or preserve the
            // saved baseline.
            Err("Connect to Wi-Fi to search.")
        // Wait for synchronized time before certificate-verified HTTPS, avoiding invalid
        // clock-based certificate checks.
        } else if !synchronized.load(Ordering::Relaxed) {
            // Return the failure result so the caller can display an error, retry, or preserve the
            // saved baseline.
            Err("Waiting for time synchronization.")
        } else {
            // Return success after the required side effects are complete.
            Ok(())
        };
        // Keep parsed service result or hardware reply being handled in this local variable for the
        // following operations.
        let response = match request {
            // Handle the Request Forecast { id, location } case: apply the state-specific behavior
            // shown here.
            Request::Forecast { id, location } => {
                // Keep accumulated response bytes or parsed worker outcome for the surrounding
                // request in this local variable for the following operations.
                let result = ready
                    .and_then(|_| fetch(&crate::weather::url(&location)))
                    .and_then(|bytes| crate::weather::parse(&bytes));
                // Run the Forecast operation with the supplied inputs.
                Response::Forecast(id, location, result)
            }
            // Handle the Request Search {                 id,                 query, language,
            // } case: Run the Locations operation with the supplied inputs.
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
            // Handle the Request Timezone { id, mut location } case: apply the state-specific
            // behavior shown here.
            Request::Timezone { id, mut location } => {
                // Keep HTTPS request address for the selected service operation in this local
                // variable for the following operations.
                let url = format!("https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&timezone=auto&forecast_days=1", location.latitude, location.longitude);
                // Keep accumulated response bytes or parsed worker outcome for the surrounding
                // request in this local variable for the following operations.
                let result = ready.and_then(|_| fetch(&url)).and_then(|bytes| {
                    #[derive(Deserialize)]
                    struct Zone {
                        // IANA timezone identifier used for local clocks and forecast dates. Stored
                        // as String.
                        timezone: String,
                    }
                    // Keep decoded API timezone response used to complete an imported location in
                    // this local variable for the following operations.
                    let zone: Zone = serde_json::from_slice(&bytes).map_err(|_| FAILED)?;
                    // Set IANA timezone identifier used for local clocks and forecast dates to IANA
                    // timezone identifier used for local clocks and forecast dates.
                    location.timezone = zone.timezone;
                    // A usable API location must have valid labels, coordinates, and a recognized
                    // nonempty timezone.
                    if location.timezone.is_empty() || !location.valid() {
                        // Leave this function now with a failure result for the surrounding
                        // operation; later statements are skipped.
                        return Err(FAILED);
                    }
                    // Return success; the caller receives selected coordinates, location labels,
                    // and IANA timezone.
                    Ok(location)
                });
                // Run the Timezone operation with the supplied inputs.
                Response::Timezone(id, result)
            }
        };
        // The main application no longer receives results, so the HTTP worker should shut down.
        if tx.send(response).is_err() {
            // Stop this loop now; the enclosing function continues with the work after the loop.
            break;
        }
    }
}
/// Fetches a certificate-verified JSON body with a ten-second deadline and 32 KiB limit.
///
/// # Arguments
///
/// * `url` (`&str`) - HTTPS endpoint to fetch using the SDK certificate bundle.
///
/// # Returns
///
/// `Result<Vec<u8>, &'static str>` - Ok with response bytes; Err with the location request
/// failure message.
///
/// # Errors
///
/// Returns an error for connection or read failures, non-200 status, oversized data, or the
/// elapsed deadline.
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
    // Keep certificate-verified ESP-IDF HTTP connection with a ten-second timeout in this local
    // variable for the following operations.
    let mut http = EspHttpConnection::new(&Configuration {
        // Initialize per-connection ten-second HTTP timeout from the supplied value.
        timeout: Some(Duration::from_secs(10)),
        // Initialize SDK callback enabling verification against the retained CA certificate bundle
        // from the supplied value.
        crt_bundle_attach: Some(sys::esp_crt_bundle_attach),
        ..Default::default()
    })
    .map_err(|_| FAILED)?;
    // Keep monotonic starting instant, independent of wall-clock corrections in this local variable
    // for the following operations.
    let started = Instant::now();
    // Execute map err for initiate request result for the surrounding operation.
    http.initiate_request(Method::Get, url, &[("Accept", "application/json")])
        .map_err(|_| FAILED)?;
    // Execute map err for initiate response result for the surrounding operation.
    http.initiate_response().map_err(|_| FAILED)?;
    // Only a successful HTTP response is accepted as forecast or geocoding JSON.
    if http.status() != 200 {
        // Leave this function now with a failure result for the surrounding operation; later
        // statements are skipped.
        return Err(FAILED);
    }
    // Reject a declared oversized body before reading it; streaming growth is checked again if the
    // header is absent or invalid.
    if http
        .header("Content-Length")
        .and_then(|v| v.parse::<usize>().ok())
        .is_some_and(|n| n > 32768)
    {
        // Leave this function now with a failure result for the surrounding operation; later
        // statements are skipped.
        return Err(FAILED);
    }
    // Keep accumulated response bytes or parsed worker outcome for the surrounding request in this
    // local variable for the following operations.
    let mut result = Vec::new();
    // Keep 512-byte HTTP read buffer, keeping response growth under the 32 KiB limit in this local
    // variable for the following operations.
    let mut buffer = [0u8; 512];
    // Keep processing events or samples until an explicit break, return, or channel shutdown ends
    // this loop.
    loop {
        // Stop a slow HTTP operation after ten seconds instead of occupying the worker
        // indefinitely.
        if started.elapsed() >= Duration::from_secs(10) {
            // Leave this function now with a failure result for the surrounding operation; later
            // statements are skipped.
            return Err(FAILED);
        }
        // Keep number of response bytes returned by the latest HTTP read; zero marks end of body in
        // this local variable for the following operations.
        let count = http.read(&mut buffer).map_err(|_| FAILED)?;
        // A zero-byte read marks the end of the HTTP body; return the accumulated response.
        if count == 0 {
            // Leave this function now with a successful result carrying accumulated response bytes
            // or parsed worker outcome for the surrounding request; later statements are skipped.
            return Ok(result);
        }
        // The next chunk would exceed the 32 KiB response budget, even if Content-Length was
        // missing or misleading.
        if result.len() + count > 32768 {
            // Leave this function now with a failure result for the surrounding operation; later
            // statements are skipped.
            return Err(FAILED);
        }
        // Append the supplied bytes in order to the existing buffer.
        result.extend_from_slice(&buffer[..count]);
    }
}

///////////////////////////////////////////////////////////////////////////////////////
//                                    UNIT TESTS                                     //
///////////////////////////////////////////////////////////////////////////////////////

/// Unit tests and supporting fixtures for location behavior.
#[cfg(test)]
mod tests {
    use super::*;
    /// Verifies that search results retain regional detail for selection while compact labels
    /// contain city and country only.
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
    fn search_keeps_region_for_selection_but_uses_city_and_country_for_display() {
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
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
            // Keep synthetic API response body used to exercise parsing in this local variable for
            // the following operations.
            let body = serde_json::json!({"results": [{"name": "München", "admin1": region,
                "country": country, "latitude": 48.1, "longitude": 11.6, "timezone": "Europe/Berlin"}]});
            // Keep validated search candidates retaining full selection labels and compact weather
            // labels in this local variable for the following operations.
            let locations = parse(&serde_json::to_vec(&body).unwrap()).unwrap();
            // Verify that human-readable label for the selected object exactly matches full.
            assert_eq!(locations[0].name, full);
            // Verify that display name result for the surrounding operation exactly matches
            // compact.
            assert_eq!(locations[0].display_name(), compact);
        }
    }

    /// Verifies percent encoding of Unicode and query delimiters so search text cannot inject
    /// additional URL parameters.
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
    fn encodes_unicode_and_delimiters_without_query_injection() {
        // Keep HTTPS request address for the selected service operation in this local variable for
        // the following operations.
        let url = url(" Café &x=1 ", Language::French);
        // Verify the value falls within the stated range or belongs to the supported set. A
        // violation means the tested behavior is incorrect.
        assert!(url.contains("name=Caf%C3%A9%20%26x%3D1&count=15&language=fr"));
        // Verify the value falls within the stated range or belongs to the supported set. A
        // violation means the tested behavior is incorrect.
        assert!(super::url("Москва", Language::Russian)
            .contains("name=%D0%9C%D0%BE%D1%81%D0%BA%D0%B2%D0%B0&count=15&language=ru"));
    }
    /// Verifies valid and empty geocoding responses and rejection of malformed, oversized, or
    /// geographically invalid data.
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
    fn parses_results_and_rejects_bad_data() {
        // Keep deserialized geocoding envelope with API error flag and location records in this
        // local variable for the following operations.
        let data = br#"{"results":[{"name":"London","country":"United Kingdom","latitude":51.5,"longitude":-0.1,"timezone":"Europe/London"}]}"#;
        // Keep accumulated response bytes or parsed worker outcome for the surrounding request in
        // this local variable for the following operations.
        let result = parse(data).unwrap();
        // Verify that human-readable label for the selected object exactly matches the specified
        // message, format, or data literal.
        assert_eq!(result[0].name, "London, United Kingdom");
        // Verify that IANA timezone identifier used for local clocks and forecast dates exactly
        // matches the specified message, format, or data literal.
        assert_eq!(result[0].timezone, "Europe/London");
        // Verify the required fixture or invariant value, panicking if unavailable is empty. A
        // violation means the tested behavior is incorrect.
        assert!(parse(br#"{"generationtime_ms":0.1}"#).unwrap().is_empty());
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(parse(br#"{"error":true}"#).is_err());
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(parse(b"invalid JSON").is_err());
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(parse(&vec![0; 32769]).is_err());
        // Keep deliberately unsupported data or the associated failure message in this local
        // variable for the following operations.
        let invalid = String::from_utf8(data.to_vec())
            .unwrap()
            .replace("Europe/London", "Not/AZone");
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(parse(invalid.as_bytes()).is_err());
        // Keep deliberately unsupported data or the associated failure message in this local
        // variable for the following operations.
        let invalid = String::from_utf8(data.to_vec())
            .unwrap()
            .replace("51.5", "91.5");
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(parse(invalid.as_bytes()).is_err());
    }
}
