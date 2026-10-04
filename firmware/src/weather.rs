// ============================================================================= //
// File          : weather.rs                                                    //
// License       : GPL-3.0-only                                                  //
// Created       : 2026-10-04                                                    //
// Last modified : 2026-10-04                                                    //
// Author        : Daniel Kharlamov                                              //
// Last editor   : Daniel Kharlamov <daniel.kharlamov@googlemail.com>            //
// ============================================================================= //
//                                                                               //
// Summary:                                                                      //
// --------                                                                      //
// Open-Meteo forecast processing and refresh scheduling.                        //
//                                                                               //
// Module:                                                                       //
// -------                                                                       //
// Builds forecast requests and parses bounded current, daily, and hourly data.  //
// Tracks location changes, request deadlines, retries, stale responses, and     //
// forecast freshness. Selects upcoming entries, converts temperatures and local //
// times, and maps weather codes to symbols independently of hardware.           //
//                                                                               //
// Note:                                                                         //
// -----                                                                         //
// Bounded Open-Meteo forecasts and refresh scheduling, independent of hardware. //
// ============================================================================= //

//! Open-Meteo forecast processing and refresh scheduling.

use crate::settings::{Location, Settings};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::Deserialize;

// Use one stable failure message so parser and transport errors can be shown consistently.
pub const FAILED: &str = "Weather unavailable. Retrying...";
// Ten-minute delay between successful forecast refreshes, in milliseconds. This fixed value is
// shared by the operations below.
pub const REFRESH_MS: u64 = 600_000;
// Sixty-second delay before retrying a failed forecast request. This fixed value is shared by the
// operations below.
pub const RETRY_MS: u64 = 60_000;
const REQUEST_TIMEOUT_MS: u64 = 30_000; // Includes one queued HTTPS operation.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    // Current weather alongside analog and digital local clocks.
    Clock,
    // Seven local calendar-day rows showing daily forecasts.
    Week,
    // At most seven future hourly forecast entries.
    Hours,
}
impl View {
    /// Cycles the weather page from Clock to Week to Hours and back.
    ///
    /// # Arguments
    ///
    /// * `self` (`View`) - Receiver state used by this operation. Passed by value.
    ///
    /// # Returns
    ///
    /// `Self` - Next weather view in the fixed navigation order.
    pub fn next(self) -> Self {
        // Choose the appropriate path for self; each arm handles one supported case.
        match self {
            // Handle the Self Clock case: apply the state-specific behavior shown here.
            Self::Clock => Self::Week,
            // Handle the Self Week case: apply the state-specific behavior shown here.
            Self::Week => Self::Hours,
            // Handle the Self Hours case: apply the state-specific behavior shown here.
            Self::Hours => Self::Clock,
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
pub struct Current {
    // Timestamp or formatted clock value used by the current view. Stored as i64.
    pub time: i64,
    // Optional current air temperature in degrees Celsius. Stored as Option<f64>.
    pub temperature_2m: Option<f64>,
    // Optional feels-like temperature in degrees Celsius. Stored as Option<f64>.
    pub apparent_temperature: Option<f64>,
    // Optional Open-Meteo condition code mapped to a retained weather symbol. Stored as
    // Option<u16>.
    pub weather_code: Option<u16>,
    // Optional API flag: zero is night and one is daytime. Stored as Option<u8>.
    pub is_day: Option<u8>,
}
#[derive(Clone, Debug)]
pub struct Day {
    // Local calendar date used for daily forecast selection. Stored as NaiveDate.
    pub date: NaiveDate,
    // Optional daily minimum temperature in degrees Celsius. Stored as Option<f64>.
    pub low: Option<f64>,
    // Optional daily maximum temperature in degrees Celsius. Stored as Option<f64>.
    pub high: Option<f64>,
    // Optional daily mean precipitation probability in percent, validated within 0-100. Stored as
    // Option<f64>.
    pub precipitation_probability_mean: Option<f64>,
    // Code. Stored as Option<u16>.
    pub code: Option<u16>,
}
#[derive(Clone, Debug)]
pub struct Hour {
    // Timestamp or formatted clock value used by the current view. Stored as i64.
    pub time: i64,
    // Optional hourly temperature in degrees Celsius. Stored as Option<f64>.
    pub temperature: Option<f64>,
    // Optional hourly precipitation probability in percent. Stored as Option<u8>.
    pub precipitation: Option<u8>,
    // Code. Stored as Option<u16>.
    pub code: Option<u16>,
    // Optional API flag: zero is night and one is daytime. Stored as Option<u8>.
    pub is_day: Option<u8>,
}
#[derive(Clone, Debug)]
pub struct Forecast {
    // Current conditions with optional measurements retained as missing-data placeholders. Stored
    // as Current.
    pub current: Current,
    // Validated consecutive daily entries used by the Week table. Stored as Vec<Day>.
    pub days: Vec<Day>,
    // Validated consecutive hourly entries used by the Hours table. Stored as Vec<Hour>.
    pub hours: Vec<Hour>,
}
impl Forecast {
    /// Finds the daily forecast for a displayed date offset.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Forecast`) - Receiver state used by this operation. Borrowed without
    ///   changing it.
    /// * `today` (`NaiveDate`) - Current local calendar date used as the forecast row baseline.
    /// * `row` (`usize`) - Zero-based number of days after today for a displayed forecast row.
    ///
    /// # Returns
    ///
    /// `Option<&Day>` - Some matching day; None if the date overflows or no forecast entry
    /// matches.
    pub fn day(&self, today: NaiveDate, row: usize) -> Option<&Day> {
        // Keep local calendar date used for daily forecast selection in this local variable for the
        // following operations.
        let date = today.checked_add_days(chrono::Days::new(row as u64))?;
        // Execute find for iter result for the surrounding operation.
        self.days.iter().find(|d| d.date == date)
    }
    /// Selects at most seven hourly entries strictly after the supplied instant.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Forecast`) - Receiver state used by this operation. Borrowed without
    ///   changing it.
    /// * `epoch` (`i64`) - UTC Unix timestamp in seconds; None means synchronized time is
    ///   unavailable.
    ///
    /// # Returns
    ///
    /// `impl Iterator<Item = &Hour>` - Borrowing iterator over future hourly entries in their
    /// stored order; may be empty.
    pub fn upcoming(&self, epoch: i64) -> impl Iterator<Item = &Hour> {
        // Execute take for filter result for the surrounding operation.
        self.hours.iter().filter(move |h| h.time > epoch).take(7)
    }
}
#[derive(Deserialize)]
struct Daily {
    #[serde(default)]
    // Optional daily mean precipitation probability in percent, validated within 0-100. Stored as
    // Option<Vec<Option<f64>>>.
    precipitation_probability_mean: Option<Vec<Option<f64>>>,
    // Timestamp or formatted clock value used by the current view. Stored as Vec<i64>.
    time: Vec<i64>,
    // Temperature 2m min. Stored as Vec<Option<f64>>.
    temperature_2m_min: Vec<Option<f64>>,
    // Temperature 2m max. Stored as Vec<Option<f64>>.
    temperature_2m_max: Vec<Option<f64>>,
    // Optional Open-Meteo condition code mapped to a retained weather symbol. Stored as
    // Vec<Option<u16>>.
    weather_code: Vec<Option<u16>>,
}
#[derive(Deserialize)]
struct Hourly {
    // Timestamp or formatted clock value used by the current view. Stored as Vec<i64>.
    time: Vec<i64>,
    // Optional current air temperature in degrees Celsius. Stored as Vec<Option<f64>>.
    temperature_2m: Vec<Option<f64>>,
    // Parallel API array of hourly precipitation probabilities. Stored as Vec<Option<u8>>.
    precipitation_probability: Vec<Option<u8>>,
    // Optional Open-Meteo condition code mapped to a retained weather symbol. Stored as
    // Vec<Option<u16>>.
    weather_code: Vec<Option<u16>>,
    // Optional API flag: zero is night and one is daytime. Stored as Vec<Option<u8>>.
    is_day: Vec<Option<u8>>,
}
#[derive(Deserialize)]
struct Wire {
    // API local-time offset in seconds applied when deriving daily calendar dates. Stored as i64.
    utc_offset_seconds: i64,
    // Current conditions with optional measurements retained as missing-data placeholders. Stored
    // as Current.
    current: Current,
    // Daily. Stored as Daily.
    daily: Daily,
    // Hourly. Stored as Hourly.
    hourly: Hourly,
}

/// Builds an eight-day Celsius forecast URL with the selected coordinates and encoded
/// timezone.
///
/// # Arguments
///
/// * `location` (`&Location`) - Coordinates, names, and IANA timezone of the selected
///   forecast location.
///
/// # Returns
///
/// `String` - Open-Meteo HTTPS URL requesting current, daily, and hourly fields with Unix
/// timestamps.
pub fn url(location: &Location) -> String {
    // An eighth day keeps midnight rollover usable while the replacement is fetched.
    // Keep IANA timezone identifier used for local clocks and forecast dates in this local variable
    // for the following operations.
    let timezone: String = location
        .timezone
        .bytes()
        .map(|b| {
            // Retain URL-unreserved timezone bytes and percent-encode slashes or other delimiters
            // before building the query.
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                // Produce an owned text value rather than a borrowed view.
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    format!("https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&timezone={timezone}&timeformat=unixtime&forecast_days=8&temperature_unit=celsius&current=temperature_2m,apparent_temperature,weather_code,is_day&daily=temperature_2m_min,temperature_2m_max,weather_code,precipitation_probability_mean&hourly=temperature_2m,precipitation_probability,weather_code,is_day", location.latitude, location.longitude)
}
/// Validates a forecast Unix timestamp against the supported year range.
///
/// # Arguments
///
/// * `time` (`i64`) - UTC Unix timestamp in seconds; None means time is unavailable when
///   optional.
///
/// # Returns
///
/// `Result<DateTime<Utc>, &'static str>` - Ok with UTC time in years 2020-2100; Err with
/// the weather failure message.
///
/// # Errors
///
/// Rejects unrepresentable timestamps or years outside 2020-2100.
fn timestamp(time: i64) -> Result<DateTime<Utc>, &'static str> {
    // Execute ok or for filter result for the surrounding operation.
    DateTime::from_timestamp(time, 0)
        .filter(|t| (2020..=2100).contains(&t.year()))
        .ok_or(FAILED)
}
/// Checks an optional Celsius value for finite supported bounds.
///
/// # Arguments
///
/// * `value` (`Option<f64>`) - Optional temperature in degrees Celsius; None represents a
///   missing measurement.
///
/// # Returns
///
/// `bool` - True for missing values or finite temperatures from -150 to 100 Celsius; false
/// otherwise.
fn temperature(value: Option<f64>) -> bool {
    // Accept missing temperatures; supplied values must be finite and between -150 and 100 degrees
    // Celsius.
    value.is_none_or(|v| v.is_finite() && (-150.0..=100.0).contains(&v))
}
/// Parses bounded forecast JSON and validates array lengths, values, and time ordering.
///
/// # Arguments
///
/// * `bytes` (`&[u8]`) - UTF-8 API JSON response bytes; accepted input is at most 32 KiB.
///
/// # Returns
///
/// `Result<Forecast, &'static str>` - Ok with current, daily, and hourly forecasts; Err
/// with the weather failure message.
///
/// # Errors
///
/// Rejects bodies over 32 KiB, invalid JSON, inconsistent arrays, unsupported timestamps or
/// offsets, invalid values, or nonconsecutive daily/hourly entries.
pub fn parse(bytes: &[u8]) -> Result<Forecast, &'static str> {
    // Reject oversized forecast JSON before parsing so cached data cannot grow without bound.
    if bytes.len() > 32768 {
        // Leave this function now with a failure result for the surrounding operation; later
        // statements are skipped.
        return Err(FAILED);
    }
    // Keep deserialized forecast envelope, validated before publishing any measurements in this
    // local variable for the following operations.
    let wire: Wire = serde_json::from_slice(bytes).map_err(|_| FAILED)?;
    // Keep daily wire arrays, indexed together after length checks in this local variable for the
    // following operations.
    let d = wire.daily;
    // Keep hourly wire arrays, indexed together after length checks in this local variable for the
    // following operations.
    let h = wire.hourly;
    // Keep number of daily wire entries; seven or eight are supported in this local variable for
    // the following operations.
    let dn = d.time.len();
    // Keep number of hourly wire entries; seven through 192 are supported in this local variable
    // for the following operations.
    let hn = h.time.len();
    // Reject inconsistent daily/hourly array lengths, unsupported time offsets, invalid
    // temperatures, or invalid day flags before indexing any parallel arrays.
    if d.precipitation_probability_mean
        .as_ref()
        .is_some_and(|values| values.len() != dn)
        || !(7..=8).contains(&dn)
        || !(7..=192).contains(&hn)
        || [
            d.temperature_2m_min.len(),
            d.temperature_2m_max.len(),
            d.weather_code.len(),
        ]
        .iter()
        .any(|n| *n != dn)
        || [
            h.temperature_2m.len(),
            h.precipitation_probability.len(),
            h.weather_code.len(),
            h.is_day.len(),
        ]
        .iter()
        .any(|n| *n != hn)
        || !(-43200..=50400).contains(&wire.utc_offset_seconds)
        || !temperature(wire.current.temperature_2m)
        || !temperature(wire.current.apparent_temperature)
        || wire.current.is_day.is_some_and(|v| v > 1)
    {
        // Leave this function now with a failure result for the surrounding operation; later
        // statements are skipped.
        return Err(FAILED);
    }
    // Validates a forecast Unix timestamp against the supported year range.
    timestamp(wire.current.time)?;
    // Keep validated consecutive daily entries used by the Week table in this local variable for
    // the following operations.
    let mut days: Vec<Day> = Vec::with_capacity(dn);
    // Visit each entry in 0..dn; the loop binding provides its value or index for this iteration.
    for i in 0..dn {
        // Open-Meteo documents adding utc_offset_seconds for daily UNIX dates.
        // Keep local calendar date used for daily forecast selection in this local variable for the
        // following operations.
        let date = timestamp(
            d.time[i]
                .checked_add(wire.utc_offset_seconds)
                .ok_or(FAILED)?,
        )?
        .date_naive();
        // Keep optional daily mean precipitation probability in percent, validated within 0-100 in
        // this local variable for the following operations.
        let precipitation_probability_mean = d
            .precipitation_probability_mean
            .as_ref()
            .and_then(|values| values[i]);
        // Reject invalid daily probabilities or temperatures, reversed minimum/maximum values, and
        // nonconsecutive local calendar dates.
        if precipitation_probability_mean
            .is_some_and(|v| !v.is_finite() || !(0.0..=100.0).contains(&v))
            || !temperature(d.temperature_2m_min[i])
            || !temperature(d.temperature_2m_max[i])
            || d.temperature_2m_min[i]
                .zip(d.temperature_2m_max[i])
                .is_some_and(|(lo, hi)| lo > hi)
            || days.last().is_some_and(|v| v.date.succ_opt() != Some(date))
        {
            // Leave this function now with a failure result for the surrounding operation; later
            // statements are skipped.
            return Err(FAILED);
        }
        // Append the new entry to the collection, preserving the order in which values arrive.
        days.push(Day {
            // Initialize local calendar date used for daily forecast selection from the supplied
            // value.
            date,
            // Initialize optional daily minimum temperature in degrees Celsius from the supplied
            // value.
            low: d.temperature_2m_min[i],
            // Initialize optional daily maximum temperature in degrees Celsius from the supplied
            // value.
            high: d.temperature_2m_max[i],
            // Initialize optional daily mean precipitation probability in percent, validated within
            // 0-100 from the supplied value.
            precipitation_probability_mean,
            // Initialize code from the supplied value.
            code: d.weather_code[i],
        });
    }
    // Keep validated consecutive hourly entries used by the Hours table in this local variable for
    // the following operations.
    let mut hours: Vec<Hour> = Vec::with_capacity(hn);
    // Visit each entry in 0..hn; the loop binding provides its value or index for this iteration.
    for i in 0..hn {
        // Validates a forecast Unix timestamp against the supported year range.
        timestamp(h.time[i])?;
        // Reject out-of-range hourly measurements, invalid day flags, and timestamps that are not
        // exactly one hour apart.
        if !temperature(h.temperature_2m[i])
            || h.precipitation_probability[i].is_some_and(|v| v > 100)
            || h.is_day[i].is_some_and(|v| v > 1)
            || hours.last().is_some_and(|v| h.time[i] - v.time != 3600)
        {
            // Leave this function now with a failure result for the surrounding operation; later
            // statements are skipped.
            return Err(FAILED);
        }
        // Append the new entry to the collection, preserving the order in which values arrive.
        hours.push(Hour {
            // Initialize timestamp or formatted clock value used by the current view from the
            // supplied value.
            time: h.time[i],
            // Initialize optional hourly temperature in degrees Celsius from the supplied value.
            temperature: h.temperature_2m[i],
            // Initialize optional hourly precipitation probability in percent from the supplied
            // value.
            precipitation: h.precipitation_probability[i],
            // Initialize code from the supplied value.
            code: h.weather_code[i],
            // Initialize optional API flag: zero is night and one is daytime from the supplied
            // value.
            is_day: h.is_day[i],
        });
    }
    // Return success; the caller receives a constructed Forecast value using these fields.
    Ok(Forecast {
        // Initialize current conditions with optional measurements retained as missing-data
        // placeholders from the supplied value.
        current: wire.current,
        // Initialize validated consecutive daily entries used by the Week table from the supplied
        // value.
        days,
        // Initialize validated consecutive hourly entries used by the Hours table from the supplied
        // value.
        hours,
    })
}

#[derive(Default)]
pub struct Weather {
    // Selected weather page: clock, seven-day forecast, or next seven hours. Stored as View.
    pub view: View,
    // Validated current, daily, and hourly weather data. Stored as Option<Forecast>.
    pub forecast: Option<Forecast>,
    // Whether the last refresh failed; cached same-location data may still be displayed. Stored as
    // bool.
    pub failed: bool,
    // UTC Unix seconds of the last successful fetch, used for twenty-minute staleness checks.
    // Stored as Option<i64>.
    pub fetched_epoch: Option<i64>,
    // Selected coordinates, location labels, and IANA timezone. Stored as Option<Location>.
    location: Option<Location>,
    // Request identifier used to reject results belonging to cancelled or replaced work. Stored as
    // u64.
    generation: u64,
    // Optional active generation and monotonic request-start timestamp. Stored as Option<(u64,
    // u64)>.
    pending: Option<(u64, u64)>,
    // Next allowed forecast attempt in monotonic milliseconds. Stored as u64.
    next_at: u64,
    // Shared indication that Wi-Fi has an assigned IP connection. Stored as bool.
    online: bool,
}
impl Weather {
    /// Adopts a changed location and invalidates its previous forecast and pending request.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Weather`) - Receiver state used by this operation. Mutated in place.
    /// * `location` (`&Location`) - Coordinates, names, and IANA timezone of the selected
    ///   forecast location.
    ///
    /// # Returns
    ///
    /// `bool` - True when the location changes; false when the existing location already
    /// matches.
    pub fn adopt(&mut self, location: &Location) -> bool {
        // The selected location is unchanged, so preserve the current forecast and pending request.
        if self.location.as_ref() == Some(location) {
            // Leave this function now with the disabled state; later statements are skipped.
            return false;
        }
        // Set selected coordinates, location labels, and IANA timezone to an available value for an
        // owned copy of selected coordinates, location labels, and IANA timezone.
        self.location = Some(location.clone());
        // Update request identifier used to reject results belonging to cancelled or replaced work
        // using 1, retaining the accumulated state for subsequent steps.
        self.generation += 1;
        // Set optional active generation and monotonic request-start timestamp to no available
        // value.
        self.pending = None;
        // Set validated current, daily, and hourly weather data to no available value.
        self.forecast = None;
        // Set UTC Unix seconds of the last successful fetch, used for twenty-minute staleness
        // checks to no available value.
        self.fetched_epoch = None;
        // Set whether the last refresh failed to the disabled state.
        self.failed = false;
        // Set next allowed forecast attempt in monotonic milliseconds to 0.
        self.next_at = 0;
        true
    }
    /// Tracks connectivity and expires a forecast request after 30 seconds.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Weather`) - Receiver state used by this operation. Mutated in place.
    /// * `now` (`u64`) - Monotonic timestamp in milliseconds, using the same origin as stored
    ///   deadlines.
    /// * `ready` (`bool`) - Whether connectivity and required time synchronization permit
    ///   forecast requests.
    ///
    /// # Returns
    ///
    /// `()` - No value; schedules immediate refresh on reconnection and a 60-second retry after
    /// expiry.
    pub fn observe(&mut self, now: u64, ready: bool) {
        // Connectivity just became usable; permit an immediate refresh instead of waiting for the
        // old schedule.
        if ready && !self.online {
            // Set next allowed forecast attempt in monotonic milliseconds to 0.
            self.next_at = 0;
        }
        // Set shared indication that Wi-Fi has an assigned IP connection to whether connectivity
        // and synchronized time permit the next request.
        self.online = ready;
        // The queued or active forecast request exceeded thirty seconds; clear it and schedule the
        // failure retry.
        if self
            .pending
            .is_some_and(|(_, start)| now.saturating_sub(start) >= REQUEST_TIMEOUT_MS)
        {
            // Set optional active generation and monotonic request-start timestamp to no available
            // value.
            self.pending = None;
            // Set whether the last refresh failed to the enabled state.
            self.failed = true;
            // Set next allowed forecast attempt in monotonic milliseconds to monotonic millisecond
            // timestamp used by forecast deadlines and retries plus sixty-second delay before
            // retrying a failed forecast request.
            self.next_at = now + RETRY_MS;
        }
    }
    /// Creates a forecast request when connected, due, and idle with a resolved location.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Weather`) - Receiver state used by this operation. Mutated in place.
    /// * `now` (`u64`) - Monotonic timestamp in milliseconds, using the same origin as stored
    ///   deadlines.
    /// * `ready` (`bool`) - Whether connectivity and required time synchronization permit
    ///   forecast requests.
    ///
    /// # Returns
    ///
    /// `Option<(u64, Location)>` - Some generation identifier and cloned location; None when
    /// offline, pending, not due, or missing a location/timezone.
    pub fn request(&mut self, now: u64, ready: bool) -> Option<(u64, Location)> {
        // Tracks connectivity and expires a forecast request after 30 seconds.
        self.observe(now, ready);
        // Do not enqueue duplicate, offline, or premature forecast work; one active request is
        // sufficient.
        if !ready || self.pending.is_some() || now < self.next_at {
            // Leave this function now with no available value; later statements are skipped.
            return None;
        }
        // Obtain an owned clone of selected coordinates, location labels, and IANA timezone;
        // reference-counted handles continue to share their underlying state.
        let location = self.location.as_ref()?.clone();
        // An imported location still needs timezone resolution before a local-calendar forecast can
        // be requested.
        if location.timezone.is_empty() {
            // Leave this function now with no available value; later statements are skipped.
            return None;
        }
        // Update request identifier used to reject results belonging to cancelled or replaced work
        // using 1, retaining the accumulated state for subsequent steps.
        self.generation += 1;
        // Set optional active generation and monotonic request-start timestamp to an available
        // value for the ordered tuple of related values.
        self.pending = Some((self.generation, now));
        // Return an available the ordered tuple of related values.
        Some((self.generation, location))
    }
    /// Releases an unqueued request and schedules another attempt after one second.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Weather`) - Receiver state used by this operation. Mutated in place.
    /// * `now` (`u64`) - Monotonic timestamp in milliseconds, using the same origin as stored
    ///   deadlines.
    ///
    /// # Returns
    ///
    /// `()` - No value; clears the pending request and updates the retry deadline.
    pub fn queue_full(&mut self, now: u64) {
        // Set optional active generation and monotonic request-start timestamp to no available
        // value.
        self.pending = None;
        // Set next allowed forecast attempt in monotonic milliseconds to monotonic millisecond
        // timestamp used by forecast deadlines and retries plus 1000.
        self.next_at = now + 1000;
    }
    /// Accepts a matching forecast response and schedules refresh or retry.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Weather`) - Receiver state used by this operation. Mutated in place.
    /// * `id` (`u64`) - Request generation identifier used to match responses with pending
    ///   work.
    /// * `location` (`&Location`) - Coordinates, names, and IANA timezone of the selected
    ///   forecast location.
    /// * `result` (`Result<Forecast, &'static str>`) - Worker response containing validated
    ///   data or a static failure message.
    /// * `now` (`u64`) - Monotonic timestamp in milliseconds, using the same origin as stored
    ///   deadlines.
    /// * `epoch` (`Option<i64>`) - UTC Unix timestamp in seconds; None means synchronized time
    ///   is unavailable.
    ///
    /// # Returns
    ///
    /// `bool` - True when active-request state changes, including expiry; false for a stale
    /// identifier or changed location. Failed refreshes retain the previous forecast.
    pub fn receive(
        &mut self,
        id: u64,
        location: &Location,
        result: Result<Forecast, &'static str>,
        now: u64,
        epoch: Option<i64>,
    ) -> bool {
        // Ignore obsolete responses after request replacement, cancellation, or a location change.
        if self.pending.map(|v| v.0) != Some(id) || self.location.as_ref() != Some(location) {
            // Leave this function now with the disabled state; later statements are skipped.
            return false;
        }
        // The queued or active forecast request exceeded thirty seconds; clear it and schedule the
        // failure retry.
        if self
            .pending
            .is_some_and(|(_, start)| now.saturating_sub(start) >= REQUEST_TIMEOUT_MS)
        {
            // Set optional active generation and monotonic request-start timestamp to no available
            // value.
            self.pending = None;
            // Set whether the last refresh failed to the enabled state.
            self.failed = true;
            // Set next allowed forecast attempt in monotonic milliseconds to monotonic millisecond
            // timestamp used by forecast deadlines and retries plus sixty-second delay before
            // retrying a failed forecast request.
            self.next_at = now + RETRY_MS;
            // Leave this function now with the enabled state; later statements are skipped.
            return true;
        }
        // Set optional active generation and monotonic request-start timestamp to no available
        // value.
        self.pending = None;
        // Set whether the last refresh failed to the operation failed.
        self.failed = result.is_err();
        // Set next allowed forecast attempt in monotonic milliseconds to monotonic millisecond
        // timestamp used by forecast deadlines and retries plus the value selected by the following
        // condition and its alternatives.
        self.next_at = now + if self.failed { RETRY_MS } else { REFRESH_MS };
        // Replace cached measurements only on success; a failed refresh retains the old
        // same-location forecast.
        if let Ok(forecast) = result {
            // Set validated current, daily, and hourly weather data to an available value for
            // validated current, daily, and hourly weather data.
            self.forecast = Some(forecast);
            // Set UTC Unix seconds of the last successful fetch, used for twenty-minute staleness
            // checks to UTC Unix timestamp in seconds; an optional value is unavailable before time
            // synchronization.
            self.fetched_epoch = epoch;
        }
        true
    }
    /// Checks whether a refresh failed or the last successful fetch is at least 20 minutes old.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Weather`) - Receiver state used by this operation. Borrowed without changing
    ///   it.
    /// * `epoch` (`Option<i64>`) - UTC Unix timestamp in seconds; None means synchronized time
    ///   is unavailable.
    ///
    /// # Returns
    ///
    /// `bool` - True for failed refresh or known stale age; false otherwise, including unknown
    /// age without failure.
    pub fn outdated(&self, epoch: Option<i64>) -> bool {
        // Return whether the last refresh failed or the optional value exists and satisfies the
        // additional predicate as the value of this block.
        self.failed
            || epoch
                .zip(self.fetched_epoch)
                .is_some_and(|(now, fetched)| now.saturating_sub(fetched) >= 1200)
    }
    /// Builds a change key for visible forecast ranges and freshness.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Weather`) - Receiver state used by this operation. Borrowed without changing
    ///   it.
    /// * `settings` (`&Settings`) - Current device preferences controlling time, brightness,
    ///   and inactivity behavior.
    /// * `epoch` (`Option<i64>`) - UTC Unix timestamp in seconds; None means synchronized time
    ///   is unavailable.
    ///
    /// # Returns
    ///
    /// `(Option<NaiveDate>, Option<i64>, bool)` - Local date, next future hourly timestamp in
    /// Unix seconds, and outdated flag; missing date/hour components are None.
    pub fn range_key(
        &self,
        settings: &Settings,
        epoch: Option<i64>,
    ) -> (Option<NaiveDate>, Option<i64>, bool) {
        // Keep local calendar date used for daily forecast selection in this local variable for the
        // following operations.
        let date = local(settings, epoch).map(|t| t.date_naive());
        // Keep hour value or hourly forecast position used by the surrounding calculation in this
        // local variable for the following operations.
        let hour = self
            .forecast
            .as_ref()
            .and_then(|f| epoch.and_then(|now| f.upcoming(now).next()))
            .map(|h| h.time);
        // Return the ordered tuple of related values as the value of this block.
        (date, hour, self.outdated(epoch))
    }
}
/// Converts an optional UTC instant into the configured location's timezone.
///
/// # Arguments
///
/// * `settings` (`&Settings`) - Current device preferences controlling time, brightness,
///   and inactivity behavior.
/// * `epoch` (`Option<i64>`) - UTC Unix timestamp in seconds; None means synchronized time
///   is unavailable.
///
/// # Returns
///
/// `Option<DateTime<chrono_tz::Tz>>` - Some zoned datetime; None for missing or invalid
/// time or an unknown timezone.
pub fn local(settings: &Settings, epoch: Option<i64>) -> Option<DateTime<chrono_tz::Tz>> {
    // Keep validated IANA timezone used to convert UTC into local calendar values in this local
    // variable for the following operations.
    let zone = settings.location.timezone.parse::<chrono_tz::Tz>().ok()?;
    // Return an available with timezone result for the surrounding operation.
    Some(DateTime::<Utc>::from_timestamp(epoch?, 0)?.with_timezone(&zone))
}
/// Formats an optional Celsius temperature as rounded Celsius or Fahrenheit.
///
/// # Arguments
///
/// * `value` (`Option<f64>`) - Optional temperature in degrees Celsius; None represents a
///   missing measurement.
/// * `fahrenheit` (`bool`) - Whether to convert the stored Celsius value to Fahrenheit for
///   display.
///
/// # Returns
///
/// `String` - Temperature text with a degree sign, or -- degrees when the value is missing.
pub fn degrees(value: Option<f64>, fahrenheit: bool) -> String {
    // Execute unwrap or else for the transformed value or entries produced by the closure.
    value
        .map(|v| format!("{:.0}°", if fahrenheit { v * 1.8 + 32.0 } else { v }))
        .unwrap_or_else(|| "--°".into())
}

/// Index into the MIT-licensed Meteocons subset (see assets/weather/NOTICE.txt).
///
/// Maps a weather code and day flag to the retained Meteocons subset.
///
/// # Arguments
///
/// * `code` (`Option<u16>`) - Optional Open-Meteo weather code; unknown or missing values
///   select the unavailable symbol.
/// * `day` (`Option<u8>`) - Optional API day flag; Some(0) selects night symbols, other
///   values select day variants.
///
/// # Returns
///
/// `usize` - Symbol index 0-12; missing or unknown codes use index 12 and only day flag 0
/// selects night variants.
pub fn symbol(code: Option<u16>, day: Option<u8>) -> usize {
    // Keep whether the API explicitly marks current conditions as night in this local variable for
    // the following operations.
    let night = day == Some(0);
    // Choose the appropriate path for code; each arm handles one supported case.
    match code {
        // Handle the Some(0) case: apply the state-specific behavior shown here.
        Some(0) => {
            // Select the night-specific clear or partly cloudy symbol only when the API explicitly
            // reports darkness.
            if night {
                1
            } else {
                0
            }
        }
        // Handle the Some(1 | 2) case: apply the state-specific behavior shown here.
        Some(1 | 2) => {
            // Select the night-specific clear or partly cloudy symbol only when the API explicitly
            // reports darkness.
            if night {
                3
            } else {
                2
            }
        }
        // Handle the Some(3) case: apply the state-specific behavior shown here.
        Some(3) => 4,
        // Handle the Some(45 | 48) case: apply the state-specific behavior shown here.
        Some(45 | 48) => 5,
        // Handle the Some(51 | 53 | 55) case: apply the state-specific behavior shown here.
        Some(51 | 53 | 55) => 6,
        // Handle the Some(56 | 57 | 66 | 67) case: apply the state-specific behavior shown here.
        Some(56 | 57 | 66 | 67) => 7,
        // Handle the Some(61 | 63 | 65 | 80 | 81 | 82) case: apply the state-specific behavior
        // shown here.
        Some(61 | 63 | 65 | 80 | 81 | 82) => 8,
        // Handle the Some(71 | 73 | 75 | 77 | 85 | 86) case: apply the state-specific behavior
        // shown here.
        Some(71 | 73 | 75 | 77 | 85 | 86) => 9,
        // Handle the Some(95) case: apply the state-specific behavior shown here.
        Some(95) => 10,
        // Handle the Some(96 | 99) case: apply the state-specific behavior shown here.
        Some(96 | 99) => 11,
        // Handle remaining cases with the fallback, preserving safe behavior for unsupported or
        // irrelevant input.
        _ => 12,
    }
}

///////////////////////////////////////////////////////////////////////////////////////
//                                    UNIT TESTS                                     //
///////////////////////////////////////////////////////////////////////////////////////

/// Unit tests and supporting fixtures for weather behavior.
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    /// Loads the forecast JSON fixture as a mutable value for validation tests.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `serde_json::Value` - Parsed JSON fixture that tests can modify before invoking the
    /// forecast parser.
    ///
    /// # Panics
    ///
    /// Panics if the fixed fixture, test timestamp, or simulated service cannot be constructed.
    fn fixture() -> serde_json::Value {
        // Keep initial pixel position or timestamp used as the baseline for this operation in this
        // local variable for the following operations.
        let start = Utc
            .with_ymd_and_hms(2026, 10, 2, 0, 0, 0)
            .unwrap()
            .timestamp();
        serde_json::json!({"utc_offset_seconds":0,"current":{"time":start,"temperature_2m":15,"apparent_temperature":14,"weather_code":0,"is_day":1},
          "daily":{"time":(0..8).map(|i| start+i*86400).collect::<Vec<_>>(),"temperature_2m_min":vec![Some(10);8],"temperature_2m_max":vec![Some(18);8],"weather_code":vec![Some(2);8]},
          "hourly":{"time":(0..192).map(|i| start+i*3600).collect::<Vec<_>>(),"temperature_2m":vec![Some(15);192],"precipitation_probability":vec![Some(20);192],"weather_code":vec![Some(2);192],"is_day":vec![Some(1);192]}})
    }
    /// Parses the checked-in forecast fixture for weather state tests.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `Forecast` - Validated forecast populated with the fixture's current, daily, and hourly
    /// entries.
    ///
    /// # Panics
    ///
    /// Panics if the fixed fixture, test timestamp, or simulated service cannot be constructed.
    pub fn sample() -> Forecast {
        // Require the value guaranteed by this test fixture or internal invariant; unexpected
        // absence panics.
        parse(&serde_json::to_vec(&fixture()).unwrap()).unwrap()
    }
    /// Verifies optional daily mean precipitation probability, compatible missing fields, and
    /// rejection of invalid values or array lengths.
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
    fn daily_mean_probability_parsing_and_optional_compatibility() {
        // Keep wire response or packed asset bytes decoded by the surrounding parser in this local
        // variable for the following operations.
        let mut data = fixture();
        // Older responses without the new field remain usable.
        // Verify every entry satisfies the required condition. A violation means the tested
        // behavior is incorrect.
        assert!(parse(&serde_json::to_vec(&data).unwrap())
            .unwrap()
            .days
            .iter()
            .all(|day| day.precipitation_probability_mean.is_none()));
        // Set the selected entry from the selected entry from wire response or packed asset bytes
        // decoded by the surrounding parser to the formatted text or fixture created by
        // serde_json::json.
        data["daily"]["precipitation_probability_mean"] =
            serde_json::json!([0, 12.5, 35.4, 100, null, 20, 30, 40]);
        // Keep validated current, daily, and hourly weather data in this local variable for the
        // following operations.
        let forecast = parse(&serde_json::to_vec(&data).unwrap()).unwrap();
        // Visit each entry in zip result for the surrounding operation; the loop binding provides
        // its value or index for this iteration.
        for (day, expected) in forecast.days.iter().zip([
            Some(0.0),
            Some(12.5),
            Some(35.4),
            Some(100.0),
            None,
            Some(20.0),
            Some(30.0),
            Some(40.0),
        ]) {
            // Verify that optional daily mean precipitation probability in percent, validated
            // within 0-100 exactly matches reference framebuffer produced by rendering the complete
            // page.
            assert_eq!(day.precipitation_probability_mean, expected);
        }
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for invalid in [
            serde_json::json!([]),
            serde_json::json!([10]),
            serde_json::json!(vec![0; 9]),
        ] {
            // Set the selected entry from the selected entry from wire response or packed asset
            // bytes decoded by the surrounding parser to deliberately unsupported data or the
            // associated failure message.
            data["daily"]["precipitation_probability_mean"] = invalid;
            // Verify the operation failed. A violation means the tested behavior is incorrect.
            assert!(parse(&serde_json::to_vec(&data).unwrap()).is_err());
        }
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for invalid in [-0.1, 100.1] {
            // Set the selected entry from the selected entry from wire response or packed asset
            // bytes decoded by the surrounding parser to the formatted text or fixture created by
            // serde_json::json.
            data["daily"]["precipitation_probability_mean"] = serde_json::json!(vec![invalid; 8]);
            // Verify the operation failed. A violation means the tested behavior is incorrect.
            assert!(parse(&serde_json::to_vec(&data).unwrap()).is_err());
        }
        // Set the selected entry from the selected entry from wire response or packed asset bytes
        // decoded by the surrounding parser to serde_json Value Null.
        data["daily"]["precipitation_probability_mean"] = serde_json::Value::Null;
        // Verify every entry satisfies the required condition. A violation means the tested
        // behavior is incorrect.
        assert!(parse(&serde_json::to_vec(&data).unwrap())
            .unwrap()
            .days
            .iter()
            .all(|day| day.precipitation_probability_mean.is_none()));
        // Keep city search text or encoded API request being examined in this local variable for
        // the following operations.
        let query = url(&Location::default());
        // Verify at least one entry satisfies the stated condition. A violation means the tested
        // behavior is incorrect.
        assert!(query
            .split('&')
            .find(|part| part.starts_with("daily="))
            .unwrap()
            .split(',')
            .any(|field| field == "precipitation_probability_mean"));
    }

    /// Verifies forecast body and value bounds while allowing supported optional measurements
    /// to be absent.
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
    fn bounds_and_missing_values() {
        // Keep wire response or packed asset bytes decoded by the surrounding parser in this local
        // variable for the following operations.
        let mut data = fixture();
        // Set the selected entry from the selected entry from wire response or packed asset bytes
        // decoded by the surrounding parser to serde_json Value Null.
        data["current"]["temperature_2m"] = serde_json::Value::Null;
        // Set the selected entry from the selected entry from the selected entry from wire response
        // or packed asset bytes decoded by the surrounding parser to the formatted text or fixture
        // created by serde_json::json.
        data["hourly"]["weather_code"][0] = serde_json::json!(999);
        // Verify the operation succeeded. A violation means the tested behavior is incorrect.
        assert!(parse(&serde_json::to_vec(&data).unwrap()).is_ok());
        // Set the selected entry from the selected entry from the selected entry from wire response
        // or packed asset bytes decoded by the surrounding parser to the formatted text or fixture
        // created by serde_json::json.
        data["hourly"]["precipitation_probability"][0] = serde_json::json!(101);
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(parse(&serde_json::to_vec(&data).unwrap()).is_err());
        // Set wire response or packed asset bytes decoded by the surrounding parser to loads the
        // forecast JSON fixture as a mutable value for validation tests.
        data = fixture();
        // Set the selected entry from the selected entry from the selected entry from wire response
        // or packed asset bytes decoded by the surrounding parser to an owned copy of the selected
        // entry from the selected entry from the selected entry from `data`.
        data["daily"]["time"][1] = data["daily"]["time"][0].clone();
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(parse(&serde_json::to_vec(&data).unwrap()).is_err());
        // Set wire response or packed asset bytes decoded by the surrounding parser to loads the
        // forecast JSON fixture as a mutable value for validation tests.
        data = fixture();
        // Set the selected entry from the selected entry from wire response or packed asset bytes
        // decoded by the surrounding parser to the formatted text or fixture created by
        // serde_json::json.
        data["hourly"]["temperature_2m"] = serde_json::json!([]);
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(parse(&serde_json::to_vec(&data).unwrap()).is_err());
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(parse(&vec![0; 32769]).is_err());
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(parse(b"{}").is_err());
        // Verify that formats an optional Celsius temperature as rounded Celsius or Fahrenheit
        // exactly matches the specified message, format, or data literal.
        assert_eq!(degrees(None, false), "--°");
        // Verify that formats an optional Celsius temperature as rounded Celsius or Fahrenheit
        // exactly matches the specified message, format, or data literal.
        assert_eq!(degrees(Some(0.0), true), "32°");
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for code in [
            0, 1, 2, 3, 45, 48, 51, 53, 55, 56, 57, 61, 63, 65, 66, 67, 71, 73, 75, 77, 80, 81, 82,
            85, 86, 95, 96, 99,
        ] {
            // Verify maps a weather code and day flag to the retained Meteocons subset is less than
            // 12. A violation means the tested behavior is incorrect.
            assert!(symbol(Some(code), Some(1)) < 12);
        }
        // Verify that maps a weather code and day flag to the retained Meteocons subset exactly
        // matches 12.
        assert_eq!(symbol(Some(999), None), 12);
        // Verify that maps a weather code and day flag to the retained Meteocons subset differs
        // from maps a weather code and day flag to the retained Meteocons subset.
        assert_ne!(symbol(Some(0), Some(1)), symbol(Some(0), Some(0)));
    }
    /// Verifies refresh intervals and rejection of obsolete forecast responses after location
    /// or generation changes.
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
    fn refresh_and_stale_requests() {
        // Keep location fixture used to test request-generation and refresh behavior in this local
        // variable for the following operations.
        let loc = Location::default();
        // Keep forecast state machine under test in this local variable for the following
        // operations.
        let mut w = Weather::default();
        // Adopts a changed location and invalidates its previous forecast and pending request.
        w.adopt(&loc);
        // Verify creates a forecast request when connected, due, and idle with a resolved location
        // is unavailable. A violation means the tested behavior is incorrect.
        assert!(w.request(0, false).is_none());
        // Keep the returned components: `id` holds request generation carried by a command or
        // response in this local variable for the following operations.
        let (id, _) = w.request(1, true).unwrap();
        // Verify creates a forecast request when connected, due, and idle with a resolved location
        // is unavailable. A violation means the tested behavior is incorrect.
        assert!(w.request(2, true).is_none());
        // Verify the inverse of accepts a matching forecast response and schedules refresh or
        // retry. A violation means the tested behavior is incorrect.
        assert!(!w.receive(id + 1, &loc, Ok(sample()), 3, Some(1)));
        // Verify accepts a matching forecast response and schedules refresh or retry. A violation
        // means the tested behavior is incorrect.
        assert!(w.receive(id, &loc, Ok(sample()), 3, Some(1)));
        // Verify creates a forecast request when connected, due, and idle with a resolved location
        // is unavailable. A violation means the tested behavior is incorrect.
        assert!(w.request(REFRESH_MS, true).is_none());
        // Keep the returned components: `id` holds request generation carried by a command or
        // response in this local variable for the following operations.
        let (id, _) = w.request(REFRESH_MS + 3, true).unwrap();
        // Accepts a matching forecast response and schedules refresh or retry.
        w.receive(id, &loc, Err(FAILED), REFRESH_MS + 4, Some(2));
        // Verify whether the last refresh failed and validated current, daily, and hourly weather
        // data is available. A violation means the tested behavior is incorrect.
        assert!(w.failed && w.forecast.is_some());
        // Verify creates a forecast request when connected, due, and idle with a resolved location
        // is unavailable. A violation means the tested behavior is incorrect.
        assert!(w.request(REFRESH_MS + 5, true).is_none());
        // Verify creates a forecast request when connected, due, and idle with a resolved location
        // is available. A violation means the tested behavior is incorrect.
        assert!(w.request(REFRESH_MS + 4 + RETRY_MS, true).is_some());
        // Obtain an owned clone of changed location fixture used to invalidate an earlier request;
        // reference-counted handles continue to share their underlying state.
        let mut other = loc.clone();
        // Set human-readable label for the selected object to into result for the surrounding
        // operation.
        other.name = "Other".into();
        // Adopts a changed location and invalidates its previous forecast and pending request.
        w.adopt(&other);
        // Verify validated current, daily, and hourly weather data is unavailable. A violation
        // means the tested behavior is incorrect.
        assert!(w.forecast.is_none());
        // Verify the inverse of accepts a matching forecast response and schedules refresh or
        // retry. A violation means the tested behavior is incorrect.
        assert!(!w.receive(id, &loc, Ok(sample()), 900000, Some(2)));
        // Keep the returned components: `id` holds request generation carried by a command or
        // response in this local variable for the following operations.
        let (id, _) = w.request(900001, true).unwrap();
        // Verify creates a forecast request when connected, due, and idle with a resolved location
        // is unavailable. A violation means the tested behavior is incorrect.
        assert!(w.request(930001, true).is_none());
        // Verify whether the last refresh failed. A violation means the tested behavior is
        // incorrect.
        assert!(w.failed);
        // Verify the inverse of accepts a matching forecast response and schedules refresh or
        // retry. A violation means the tested behavior is incorrect.
        assert!(!w.receive(id, &other, Ok(sample()), 930002, Some(2)));
        // Verify creates a forecast request when connected, due, and idle with a resolved location
        // is available. A violation means the tested behavior is incorrect.
        assert!(w.request(990001, true).is_some());
    }
    /// Verifies request expiry, delayed retry after queue pressure, and immediate refresh after
    /// reconnection.
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
    fn deadline_queue_pressure_and_reconnection() {
        // Keep selected coordinates, location labels, and IANA timezone in this local variable for
        // the following operations.
        let location = Location::default();
        // Keep forecast cache, selected view, and refresh scheduler in this local variable for the
        // following operations.
        let mut weather = Weather::default();
        // Adopts a changed location and invalidates its previous forecast and pending request.
        weather.adopt(&location);
        // Keep the returned components: `id` holds request generation carried by a command or
        // response in this local variable for the following operations.
        let (id, _) = weather.request(0, true).unwrap();
        // Verify accepts a matching forecast response and schedules refresh or retry. A violation
        // means the tested behavior is incorrect.
        assert!(weather.receive(id, &location, Ok(sample()), REQUEST_TIMEOUT_MS, Some(1)));
        // Verify whether the last refresh failed and validated current, daily, and hourly weather
        // data is unavailable. A violation means the tested behavior is incorrect.
        assert!(weather.failed && weather.forecast.is_none());
        // Verify creates a forecast request when connected, due, and idle with a resolved location
        // is unavailable. A violation means the tested behavior is incorrect.
        assert!(weather.request(REQUEST_TIMEOUT_MS + 1, true).is_none());
        // Creates a forecast request when connected, due, and idle with a resolved location.
        weather.request(REQUEST_TIMEOUT_MS + 1, false);
        // Keep the returned components: `id` holds request generation carried by a command or
        // response in this local variable for the following operations.
        let (id, _) = weather.request(REQUEST_TIMEOUT_MS + 2, true).unwrap();
        // Releases an unqueued request and schedules another attempt after one second.
        weather.queue_full(REQUEST_TIMEOUT_MS + 2);
        // Verify the inverse of accepts a matching forecast response and schedules refresh or
        // retry. A violation means the tested behavior is incorrect.
        assert!(!weather.receive(id, &location, Ok(sample()), REQUEST_TIMEOUT_MS + 3, Some(1)));
        // Verify creates a forecast request when connected, due, and idle with a resolved location
        // is unavailable. A violation means the tested behavior is incorrect.
        assert!(weather.request(REQUEST_TIMEOUT_MS + 1001, true).is_none());
        // Keep the returned components: `id` holds request generation carried by a command or
        // response in this local variable for the following operations.
        let (id, _) = weather.request(REQUEST_TIMEOUT_MS + 1002, true).unwrap();
        // Verify accepts a matching forecast response and schedules refresh or retry. A violation
        // means the tested behavior is incorrect.
        assert!(weather.receive(
            id,
            &location,
            Ok(sample()),
            REQUEST_TIMEOUT_MS + 1003,
            Some(1)
        ));
        // Tracks connectivity and expires a forecast request after 30 seconds.
        weather.observe(REQUEST_TIMEOUT_MS + 1004, true);
        // Verify creates a forecast request when connected, due, and idle with a resolved location
        // is unavailable. A violation means the tested behavior is incorrect.
        assert!(weather.request(REQUEST_TIMEOUT_MS + 1005, true).is_none());
        // Tracks connectivity and expires a forecast request after 30 seconds.
        weather.observe(REQUEST_TIMEOUT_MS + 1006, false);
        // Verify creates a forecast request when connected, due, and idle with a resolved location
        // is available. A violation means the tested behavior is incorrect.
        assert!(weather.request(REQUEST_TIMEOUT_MS + 1007, true).is_some());
    }

    /// Verifies seven-entry forecast selection through midnight, daylight-saving changes, and
    /// fractional timezone offsets.
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
    fn seven_entries_midnight_dst_and_fractional_offsets() {
        // Keep validated current, daily, and hourly weather data in this local variable for the
        // following operations.
        let mut forecast = sample();
        // Keep current local date used to select the first daily row in this local variable for the
        // following operations.
        let today = forecast.days[0].date;
        // Verify that count result for the surrounding operation exactly matches 7.
        assert_eq!((0..7).filter_map(|row| forecast.day(today, row)).count(), 7);
        // Keep next local calendar date, used to verify midnight forecast rollover in this local
        // variable for the following operations.
        let tomorrow = today.succ_opt().unwrap();
        // Verify that local calendar date used for daily forecast selection exactly matches next
        // local calendar date, used to verify midnight forecast rollover.
        assert_eq!(forecast.day(tomorrow, 0).unwrap().date, tomorrow);
        // Verify that local calendar date used for daily forecast selection exactly matches local
        // calendar date used for daily forecast selection.
        assert_eq!(
            forecast.day(tomorrow, 6).unwrap().date,
            forecast.days[7].date
        );
        // Verify finds the daily forecast for a displayed date offset is unavailable. A violation
        // means the tested behavior is incorrect.
        assert!(forecast.day(tomorrow, 7).is_none());
        // Keep device preferences controlling location, language, units, brightness, and clock
        // format in this local variable for the following operations.
        let mut settings = Settings::default();
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for (zone, start) in [
            (
                "Europe/Berlin",
                Utc.with_ymd_and_hms(2026, 10, 25, 0, 0, 0)
                    .unwrap()
                    .timestamp(),
            ),
            (
                "Europe/Berlin",
                Utc.with_ymd_and_hms(2026, 3, 29, 0, 0, 0)
                    .unwrap()
                    .timestamp(),
            ),
            (
                "Asia/Kathmandu",
                Utc.with_ymd_and_hms(2026, 10, 2, 0, 15, 0)
                    .unwrap()
                    .timestamp(),
            ),
        ] {
            // Set IANA timezone identifier used for local clocks and forecast dates to into result
            // for the surrounding operation.
            settings.location.timezone = zone.into();
            // Visit each entry in enumerate result for the surrounding operation; the loop binding
            // provides its value or index for this iteration.
            for (i, hour) in forecast.hours.iter_mut().enumerate() {
                // Set timestamp or formatted clock value used by the current view to initial pixel
                // position or timestamp used as the baseline for this operation plus the numeric
                // value converted to the required arithmetic or indexing type multiplied by 3600.
                hour.time = start + i as i64 * 3600;
            }
            // Keep subsequent forecast entries or the next UTF-8 boundary, depending on the
            // surrounding calculation in this local variable for the following operations.
            let next: Vec<_> = forecast.upcoming(start - 1).collect();
            // Verify that the length of subsequent forecast entries or the next UTF-8 boundary,
            // depending on the surrounding calculation exactly matches 7.
            assert_eq!(next.len(), 7);
            // Verify that timestamp or formatted clock value used by the current view exactly
            // matches initial pixel position or timestamp used as the baseline for this operation.
            assert_eq!(next[0].time, start);
            // Verify that timestamp or formatted clock value used by the current view exactly
            // matches initial pixel position or timestamp used as the baseline for this operation
            // plus 3600.
            assert_eq!(forecast.upcoming(start).next().unwrap().time, start + 3600);
            // Check whether validated IANA timezone used to convert UTC into local calendar values
            // equals the specified message, format, or data literal.
            if zone == "Asia/Kathmandu" {
                // Verify that an owned text value rather than a borrowed view exactly matches the
                // specified message, format, or data literal.
                assert_eq!(
                    local(&settings, Some(next[0].time))
                        .unwrap()
                        .format("%H:%M")
                        .to_string(),
                    "06:00"
                );
            } else {
                // Keep first selected local-hour label used to check timezone or DST behavior in
                // this local variable for the following operations.
                let first = local(&settings, Some(next[0].time))
                    .unwrap()
                    .format("%H:%M %Z")
                    .to_string();
                // Keep second within the minute in this local variable for the following
                // operations.
                let second = local(&settings, Some(next[1].time))
                    .unwrap()
                    .format("%H:%M %Z")
                    .to_string();
                // Verify that first selected local-hour label used to check timezone or DST
                // behavior differs from second within the minute.
                assert_ne!(first, second);
                // Check whether first selected local-hour label used to check timezone or DST
                // behavior equals the specified message, format, or data literal.
                if first == "02:00 CEST" {
                    // Verify that second within the minute exactly matches the specified message,
                    // format, or data literal.
                    assert_eq!(second, "02:00 CET");
                } else {
                    // Verify that first selected local-hour label used to check timezone or DST
                    // behavior exactly matches the specified message, format, or data literal.
                    assert_eq!(first, "01:00 CET");
                    // Verify that second within the minute exactly matches the specified message,
                    // format, or data literal.
                    assert_eq!(second, "03:00 CEST");
                }
            }
        }
        // Verify the value falls within the stated range or belongs to the supported set. A
        // violation means the tested behavior is incorrect.
        assert!(url(&settings.location).contains("timezone=Asia%2FKathmandu"));
        // Keep wire response or packed asset bytes decoded by the surrounding parser in this local
        // variable for the following operations.
        let mut data = fixture();
        // Set the selected entry from wire response or packed asset bytes decoded by the
        // surrounding parser to the formatted text or fixture created by serde_json::json.
        data["utc_offset_seconds"] = serde_json::json!(20700);
        // Keep mutable daily timestamp array deliberately adjusted by parser tests in this local
        // variable for the following operations.
        let times = data["daily"]["time"].as_array_mut().unwrap();
        // Visit each entry in mutable daily timestamp array deliberately adjusted by parser tests;
        // the loop binding provides its value or index for this iteration.
        for time in times {
            // Set timestamp or formatted clock value used by the current view to the formatted text
            // or fixture created by serde_json::json.
            *time = serde_json::json!(time.as_i64().unwrap() - 20700);
        }
        // Verify that local calendar date used for daily forecast selection exactly matches current
        // local date used to select the first daily row.
        assert_eq!(
            parse(&serde_json::to_vec(&data).unwrap()).unwrap().days[0].date,
            today
        );
    }

    /// Verifies timezone conversion and hourly range changes at local and daylight-saving
    /// boundaries.
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
    fn local_time_dst_and_hour_boundaries() {
        // Keep device preferences controlling location, language, units, brightness, and clock
        // format in this local variable for the following operations.
        let mut settings = Settings::default();
        // Set IANA timezone identifier used for local clocks and forecast dates to into result for
        // the surrounding operation.
        settings.location.timezone = "Europe/Berlin".into();
        // Keep snapshot taken before the operation so later changes can be compared in this local
        // variable for the following operations.
        let before = Utc
            .with_ymd_and_hms(2026, 10, 25, 0, 0, 0)
            .unwrap()
            .timestamp();
        // Verify that an owned text value rather than a borrowed view exactly matches the specified
        // message, format, or data literal.
        assert_eq!(
            local(&settings, Some(before))
                .unwrap()
                .format("%H:%M %Z")
                .to_string(),
            "02:00 CEST"
        );
        // Verify that an owned text value rather than a borrowed view exactly matches the specified
        // message, format, or data literal.
        assert_eq!(
            local(&settings, Some(before + 3600))
                .unwrap()
                .format("%H:%M %Z")
                .to_string(),
            "02:00 CET"
        );
        // Set IANA timezone identifier used for local clocks and forecast dates to into result for
        // the surrounding operation.
        settings.location.timezone = "Asia/Kathmandu".into();
        // Verify that an owned text value rather than a borrowed view exactly matches the specified
        // message, format, or data literal.
        assert_eq!(
            local(&settings, Some(before))
                .unwrap()
                .format("%H:%M")
                .to_string(),
            "05:45"
        );
        // Keep forecast state machine under test in this local variable for the following
        // operations.
        let mut w = Weather::default();
        // Set validated current, daily, and hourly weather data to an available value for parses
        // the checked-in forecast fixture for weather state tests.
        w.forecast = Some(sample());
        // Keep initial pixel position or timestamp used as the baseline for this operation in this
        // local variable for the following operations.
        let start = w.forecast.as_ref().unwrap().hours[0].time;
        // Verify that 1 exactly matches an available value for initial pixel position or timestamp
        // used as the baseline for this operation plus 3600.
        assert_eq!(w.range_key(&settings, Some(start)).1, Some(start + 3600));
        // Verify that count result for the surrounding operation exactly matches 7.
        assert_eq!(
            w.forecast
                .as_ref()
                .unwrap()
                .hours
                .iter()
                .filter(|h| h.time > start + 86399)
                .take(7)
                .count(),
            7
        );
    }
}
