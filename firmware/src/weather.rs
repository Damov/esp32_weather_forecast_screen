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

use crate::settings::{Location, Settings};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::Deserialize;

pub const FAILED: &str = "Weather unavailable. Retrying...";
pub const REFRESH_MS: u64 = 600_000;
pub const RETRY_MS: u64 = 60_000;
const REQUEST_TIMEOUT_MS: u64 = 30_000; // Includes one queued HTTPS operation.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    Clock,
    Week,
    Hours,
}
impl View {
    pub fn next(self) -> Self {
        match self {
            Self::Clock => Self::Week,
            Self::Week => Self::Hours,
            Self::Hours => Self::Clock,
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
pub struct Current {
    pub time: i64,
    pub temperature_2m: Option<f64>,
    pub apparent_temperature: Option<f64>,
    pub weather_code: Option<u16>,
    pub is_day: Option<u8>,
}
#[derive(Clone, Debug)]
pub struct Day {
    pub date: NaiveDate,
    pub low: Option<f64>,
    pub high: Option<f64>,
    pub precipitation_probability_mean: Option<f64>,
    pub code: Option<u16>,
}
#[derive(Clone, Debug)]
pub struct Hour {
    pub time: i64,
    pub temperature: Option<f64>,
    pub precipitation: Option<u8>,
    pub code: Option<u16>,
    pub is_day: Option<u8>,
}
#[derive(Clone, Debug)]
pub struct Forecast {
    pub current: Current,
    pub days: Vec<Day>,
    pub hours: Vec<Hour>,
}
impl Forecast {
    pub fn day(&self, today: NaiveDate, row: usize) -> Option<&Day> {
        let date = today.checked_add_days(chrono::Days::new(row as u64))?;
        self.days.iter().find(|d| d.date == date)
    }
    pub fn upcoming(&self, epoch: i64) -> impl Iterator<Item = &Hour> {
        self.hours.iter().filter(move |h| h.time > epoch).take(7)
    }
}
#[derive(Deserialize)]
struct Daily {
    #[serde(default)]
    precipitation_probability_mean: Option<Vec<Option<f64>>>,
    time: Vec<i64>,
    temperature_2m_min: Vec<Option<f64>>,
    temperature_2m_max: Vec<Option<f64>>,
    weather_code: Vec<Option<u16>>,
}
#[derive(Deserialize)]
struct Hourly {
    time: Vec<i64>,
    temperature_2m: Vec<Option<f64>>,
    precipitation_probability: Vec<Option<u8>>,
    weather_code: Vec<Option<u16>>,
    is_day: Vec<Option<u8>>,
}
#[derive(Deserialize)]
struct Wire {
    utc_offset_seconds: i64,
    current: Current,
    daily: Daily,
    hourly: Hourly,
}

pub fn url(location: &Location) -> String {
    // An eighth day keeps midnight rollover usable while the replacement is fetched.
    let timezone: String = location
        .timezone
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    format!("https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&timezone={timezone}&timeformat=unixtime&forecast_days=8&temperature_unit=celsius&current=temperature_2m,apparent_temperature,weather_code,is_day&daily=temperature_2m_min,temperature_2m_max,weather_code,precipitation_probability_mean&hourly=temperature_2m,precipitation_probability,weather_code,is_day", location.latitude, location.longitude)
}
fn timestamp(time: i64) -> Result<DateTime<Utc>, &'static str> {
    DateTime::from_timestamp(time, 0)
        .filter(|t| (2020..=2100).contains(&t.year()))
        .ok_or(FAILED)
}
fn temperature(value: Option<f64>) -> bool {
    value.is_none_or(|v| v.is_finite() && (-150.0..=100.0).contains(&v))
}
pub fn parse(bytes: &[u8]) -> Result<Forecast, &'static str> {
    if bytes.len() > 32768 {
        return Err(FAILED);
    }
    let wire: Wire = serde_json::from_slice(bytes).map_err(|_| FAILED)?;
    let d = wire.daily;
    let h = wire.hourly;
    let dn = d.time.len();
    let hn = h.time.len();
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
        return Err(FAILED);
    }
    timestamp(wire.current.time)?;
    let mut days: Vec<Day> = Vec::with_capacity(dn);
    for i in 0..dn {
        // Open-Meteo documents adding utc_offset_seconds for daily UNIX dates.
        let date = timestamp(
            d.time[i]
                .checked_add(wire.utc_offset_seconds)
                .ok_or(FAILED)?,
        )?
        .date_naive();
        let precipitation_probability_mean = d
            .precipitation_probability_mean
            .as_ref()
            .and_then(|values| values[i]);
        if precipitation_probability_mean
            .is_some_and(|v| !v.is_finite() || !(0.0..=100.0).contains(&v))
            || !temperature(d.temperature_2m_min[i])
            || !temperature(d.temperature_2m_max[i])
            || d.temperature_2m_min[i]
                .zip(d.temperature_2m_max[i])
                .is_some_and(|(lo, hi)| lo > hi)
            || days.last().is_some_and(|v| v.date.succ_opt() != Some(date))
        {
            return Err(FAILED);
        }
        days.push(Day {
            date,
            low: d.temperature_2m_min[i],
            high: d.temperature_2m_max[i],
            precipitation_probability_mean,
            code: d.weather_code[i],
        });
    }
    let mut hours: Vec<Hour> = Vec::with_capacity(hn);
    for i in 0..hn {
        timestamp(h.time[i])?;
        if !temperature(h.temperature_2m[i])
            || h.precipitation_probability[i].is_some_and(|v| v > 100)
            || h.is_day[i].is_some_and(|v| v > 1)
            || hours.last().is_some_and(|v| h.time[i] - v.time != 3600)
        {
            return Err(FAILED);
        }
        hours.push(Hour {
            time: h.time[i],
            temperature: h.temperature_2m[i],
            precipitation: h.precipitation_probability[i],
            code: h.weather_code[i],
            is_day: h.is_day[i],
        });
    }
    Ok(Forecast {
        current: wire.current,
        days,
        hours,
    })
}

#[derive(Default)]
pub struct Weather {
    pub view: View,
    pub forecast: Option<Forecast>,
    pub failed: bool,
    pub fetched_epoch: Option<i64>,
    location: Option<Location>,
    generation: u64,
    pending: Option<(u64, u64)>,
    next_at: u64,
    online: bool,
}
impl Weather {
    pub fn adopt(&mut self, location: &Location) -> bool {
        if self.location.as_ref() == Some(location) {
            return false;
        }
        self.location = Some(location.clone());
        self.generation += 1;
        self.pending = None;
        self.forecast = None;
        self.fetched_epoch = None;
        self.failed = false;
        self.next_at = 0;
        true
    }
    pub fn observe(&mut self, now: u64, ready: bool) {
        if ready && !self.online {
            self.next_at = 0;
        }
        self.online = ready;
        if self
            .pending
            .is_some_and(|(_, start)| now.saturating_sub(start) >= REQUEST_TIMEOUT_MS)
        {
            self.pending = None;
            self.failed = true;
            self.next_at = now + RETRY_MS;
        }
    }
    pub fn request(&mut self, now: u64, ready: bool) -> Option<(u64, Location)> {
        self.observe(now, ready);
        if !ready || self.pending.is_some() || now < self.next_at {
            return None;
        }
        let location = self.location.as_ref()?.clone();
        if location.timezone.is_empty() {
            return None;
        }
        self.generation += 1;
        self.pending = Some((self.generation, now));
        Some((self.generation, location))
    }
    pub fn queue_full(&mut self, now: u64) {
        self.pending = None;
        self.next_at = now + 1000;
    }
    pub fn receive(
        &mut self,
        id: u64,
        location: &Location,
        result: Result<Forecast, &'static str>,
        now: u64,
        epoch: Option<i64>,
    ) -> bool {
        if self.pending.map(|v| v.0) != Some(id) || self.location.as_ref() != Some(location) {
            return false;
        }
        if self
            .pending
            .is_some_and(|(_, start)| now.saturating_sub(start) >= REQUEST_TIMEOUT_MS)
        {
            self.pending = None;
            self.failed = true;
            self.next_at = now + RETRY_MS;
            return true;
        }
        self.pending = None;
        self.failed = result.is_err();
        self.next_at = now + if self.failed { RETRY_MS } else { REFRESH_MS };
        if let Ok(forecast) = result {
            self.forecast = Some(forecast);
            self.fetched_epoch = epoch;
        }
        true
    }
    pub fn outdated(&self, epoch: Option<i64>) -> bool {
        self.failed
            || epoch
                .zip(self.fetched_epoch)
                .is_some_and(|(now, fetched)| now.saturating_sub(fetched) >= 1200)
    }
    pub fn range_key(
        &self,
        settings: &Settings,
        epoch: Option<i64>,
    ) -> (Option<NaiveDate>, Option<i64>, bool) {
        let date = local(settings, epoch).map(|t| t.date_naive());
        let hour = self
            .forecast
            .as_ref()
            .and_then(|f| epoch.and_then(|now| f.upcoming(now).next()))
            .map(|h| h.time);
        (date, hour, self.outdated(epoch))
    }
}
pub fn local(settings: &Settings, epoch: Option<i64>) -> Option<DateTime<chrono_tz::Tz>> {
    let zone = settings.location.timezone.parse::<chrono_tz::Tz>().ok()?;
    Some(DateTime::<Utc>::from_timestamp(epoch?, 0)?.with_timezone(&zone))
}
pub fn degrees(value: Option<f64>, fahrenheit: bool) -> String {
    value
        .map(|v| format!("{:.0}°", if fahrenheit { v * 1.8 + 32.0 } else { v }))
        .unwrap_or_else(|| "--°".into())
}

/// Index into the MIT-licensed Meteocons subset (see assets/weather/NOTICE.txt).
pub fn symbol(code: Option<u16>, day: Option<u8>) -> usize {
    let night = day == Some(0);
    match code {
        Some(0) => {
            if night {
                1
            } else {
                0
            }
        }
        Some(1 | 2) => {
            if night {
                3
            } else {
                2
            }
        }
        Some(3) => 4,
        Some(45 | 48) => 5,
        Some(51 | 53 | 55) => 6,
        Some(56 | 57 | 66 | 67) => 7,
        Some(61 | 63 | 65 | 80 | 81 | 82) => 8,
        Some(71 | 73 | 75 | 77 | 85 | 86) => 9,
        Some(95) => 10,
        Some(96 | 99) => 11,
        _ => 12,
    }
}

///////////////////////////////////////////////////////////////////////////////////////
//                                    UNIT TESTS                                     //
///////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    fn fixture() -> serde_json::Value {
        let start = Utc
            .with_ymd_and_hms(2026, 10, 2, 0, 0, 0)
            .unwrap()
            .timestamp();
        serde_json::json!({"utc_offset_seconds":0,"current":{"time":start,"temperature_2m":15,"apparent_temperature":14,"weather_code":0,"is_day":1},
          "daily":{"time":(0..8).map(|i| start+i*86400).collect::<Vec<_>>(),"temperature_2m_min":vec![Some(10);8],"temperature_2m_max":vec![Some(18);8],"weather_code":vec![Some(2);8]},
          "hourly":{"time":(0..192).map(|i| start+i*3600).collect::<Vec<_>>(),"temperature_2m":vec![Some(15);192],"precipitation_probability":vec![Some(20);192],"weather_code":vec![Some(2);192],"is_day":vec![Some(1);192]}})
    }
    pub fn sample() -> Forecast {
        parse(&serde_json::to_vec(&fixture()).unwrap()).unwrap()
    }
    #[test]
    fn daily_mean_probability_parsing_and_optional_compatibility() {
        let mut data = fixture();
        // Older responses without the new field remain usable.
        assert!(parse(&serde_json::to_vec(&data).unwrap())
            .unwrap()
            .days
            .iter()
            .all(|day| day.precipitation_probability_mean.is_none()));
        data["daily"]["precipitation_probability_mean"] =
            serde_json::json!([0, 12.5, 35.4, 100, null, 20, 30, 40]);
        let forecast = parse(&serde_json::to_vec(&data).unwrap()).unwrap();
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
            assert_eq!(day.precipitation_probability_mean, expected);
        }
        for invalid in [
            serde_json::json!([]),
            serde_json::json!([10]),
            serde_json::json!(vec![0; 9]),
        ] {
            data["daily"]["precipitation_probability_mean"] = invalid;
            assert!(parse(&serde_json::to_vec(&data).unwrap()).is_err());
        }
        for invalid in [-0.1, 100.1] {
            data["daily"]["precipitation_probability_mean"] = serde_json::json!(vec![invalid; 8]);
            assert!(parse(&serde_json::to_vec(&data).unwrap()).is_err());
        }
        data["daily"]["precipitation_probability_mean"] = serde_json::Value::Null;
        assert!(parse(&serde_json::to_vec(&data).unwrap())
            .unwrap()
            .days
            .iter()
            .all(|day| day.precipitation_probability_mean.is_none()));
        let query = url(&Location::default());
        assert!(query
            .split('&')
            .find(|part| part.starts_with("daily="))
            .unwrap()
            .split(',')
            .any(|field| field == "precipitation_probability_mean"));
    }

    #[test]
    fn bounds_and_missing_values() {
        let mut data = fixture();
        data["current"]["temperature_2m"] = serde_json::Value::Null;
        data["hourly"]["weather_code"][0] = serde_json::json!(999);
        assert!(parse(&serde_json::to_vec(&data).unwrap()).is_ok());
        data["hourly"]["precipitation_probability"][0] = serde_json::json!(101);
        assert!(parse(&serde_json::to_vec(&data).unwrap()).is_err());
        data = fixture();
        data["daily"]["time"][1] = data["daily"]["time"][0].clone();
        assert!(parse(&serde_json::to_vec(&data).unwrap()).is_err());
        data = fixture();
        data["hourly"]["temperature_2m"] = serde_json::json!([]);
        assert!(parse(&serde_json::to_vec(&data).unwrap()).is_err());
        assert!(parse(&vec![0; 32769]).is_err());
        assert!(parse(b"{}").is_err());
        assert_eq!(degrees(None, false), "--°");
        assert_eq!(degrees(Some(0.0), true), "32°");
        for code in [
            0, 1, 2, 3, 45, 48, 51, 53, 55, 56, 57, 61, 63, 65, 66, 67, 71, 73, 75, 77, 80, 81, 82,
            85, 86, 95, 96, 99,
        ] {
            assert!(symbol(Some(code), Some(1)) < 12);
        }
        assert_eq!(symbol(Some(999), None), 12);
        assert_ne!(symbol(Some(0), Some(1)), symbol(Some(0), Some(0)));
    }
    #[test]
    fn refresh_and_stale_requests() {
        let loc = Location::default();
        let mut w = Weather::default();
        w.adopt(&loc);
        assert!(w.request(0, false).is_none());
        let (id, _) = w.request(1, true).unwrap();
        assert!(w.request(2, true).is_none());
        assert!(!w.receive(id + 1, &loc, Ok(sample()), 3, Some(1)));
        assert!(w.receive(id, &loc, Ok(sample()), 3, Some(1)));
        assert!(w.request(REFRESH_MS, true).is_none());
        let (id, _) = w.request(REFRESH_MS + 3, true).unwrap();
        w.receive(id, &loc, Err(FAILED), REFRESH_MS + 4, Some(2));
        assert!(w.failed && w.forecast.is_some());
        assert!(w.request(REFRESH_MS + 5, true).is_none());
        assert!(w.request(REFRESH_MS + 4 + RETRY_MS, true).is_some());
        let mut other = loc.clone();
        other.name = "Other".into();
        w.adopt(&other);
        assert!(w.forecast.is_none());
        assert!(!w.receive(id, &loc, Ok(sample()), 900000, Some(2)));
        let (id, _) = w.request(900001, true).unwrap();
        assert!(w.request(930001, true).is_none());
        assert!(w.failed);
        assert!(!w.receive(id, &other, Ok(sample()), 930002, Some(2)));
        assert!(w.request(990001, true).is_some());
    }
    #[test]
    fn deadline_queue_pressure_and_reconnection() {
        let location = Location::default();
        let mut weather = Weather::default();
        weather.adopt(&location);
        let (id, _) = weather.request(0, true).unwrap();
        assert!(weather.receive(id, &location, Ok(sample()), REQUEST_TIMEOUT_MS, Some(1)));
        assert!(weather.failed && weather.forecast.is_none());
        assert!(weather.request(REQUEST_TIMEOUT_MS + 1, true).is_none());
        weather.request(REQUEST_TIMEOUT_MS + 1, false);
        let (id, _) = weather.request(REQUEST_TIMEOUT_MS + 2, true).unwrap();
        weather.queue_full(REQUEST_TIMEOUT_MS + 2);
        assert!(!weather.receive(id, &location, Ok(sample()), REQUEST_TIMEOUT_MS + 3, Some(1)));
        assert!(weather.request(REQUEST_TIMEOUT_MS + 1001, true).is_none());
        let (id, _) = weather.request(REQUEST_TIMEOUT_MS + 1002, true).unwrap();
        assert!(weather.receive(
            id,
            &location,
            Ok(sample()),
            REQUEST_TIMEOUT_MS + 1003,
            Some(1)
        ));
        weather.observe(REQUEST_TIMEOUT_MS + 1004, true);
        assert!(weather.request(REQUEST_TIMEOUT_MS + 1005, true).is_none());
        weather.observe(REQUEST_TIMEOUT_MS + 1006, false);
        assert!(weather.request(REQUEST_TIMEOUT_MS + 1007, true).is_some());
    }

    #[test]
    fn seven_entries_midnight_dst_and_fractional_offsets() {
        let mut forecast = sample();
        let today = forecast.days[0].date;
        assert_eq!((0..7).filter_map(|row| forecast.day(today, row)).count(), 7);
        let tomorrow = today.succ_opt().unwrap();
        assert_eq!(forecast.day(tomorrow, 0).unwrap().date, tomorrow);
        assert_eq!(
            forecast.day(tomorrow, 6).unwrap().date,
            forecast.days[7].date
        );
        assert!(forecast.day(tomorrow, 7).is_none());
        let mut settings = Settings::default();
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
            settings.location.timezone = zone.into();
            for (i, hour) in forecast.hours.iter_mut().enumerate() {
                hour.time = start + i as i64 * 3600;
            }
            let next: Vec<_> = forecast.upcoming(start - 1).collect();
            assert_eq!(next.len(), 7);
            assert_eq!(next[0].time, start);
            assert_eq!(forecast.upcoming(start).next().unwrap().time, start + 3600);
            if zone == "Asia/Kathmandu" {
                assert_eq!(
                    local(&settings, Some(next[0].time))
                        .unwrap()
                        .format("%H:%M")
                        .to_string(),
                    "06:00"
                );
            } else {
                let first = local(&settings, Some(next[0].time))
                    .unwrap()
                    .format("%H:%M %Z")
                    .to_string();
                let second = local(&settings, Some(next[1].time))
                    .unwrap()
                    .format("%H:%M %Z")
                    .to_string();
                assert_ne!(first, second);
                if first == "02:00 CEST" {
                    assert_eq!(second, "02:00 CET");
                } else {
                    assert_eq!(first, "01:00 CET");
                    assert_eq!(second, "03:00 CEST");
                }
            }
        }
        assert!(url(&settings.location).contains("timezone=Asia%2FKathmandu"));
        let mut data = fixture();
        data["utc_offset_seconds"] = serde_json::json!(20700);
        let times = data["daily"]["time"].as_array_mut().unwrap();
        for time in times {
            *time = serde_json::json!(time.as_i64().unwrap() - 20700);
        }
        assert_eq!(
            parse(&serde_json::to_vec(&data).unwrap()).unwrap().days[0].date,
            today
        );
    }

    #[test]
    fn local_time_dst_and_hour_boundaries() {
        let mut settings = Settings::default();
        settings.location.timezone = "Europe/Berlin".into();
        let before = Utc
            .with_ymd_and_hms(2026, 10, 25, 0, 0, 0)
            .unwrap()
            .timestamp();
        assert_eq!(
            local(&settings, Some(before))
                .unwrap()
                .format("%H:%M %Z")
                .to_string(),
            "02:00 CEST"
        );
        assert_eq!(
            local(&settings, Some(before + 3600))
                .unwrap()
                .format("%H:%M %Z")
                .to_string(),
            "02:00 CET"
        );
        settings.location.timezone = "Asia/Kathmandu".into();
        assert_eq!(
            local(&settings, Some(before))
                .unwrap()
                .format("%H:%M")
                .to_string(),
            "05:45"
        );
        let mut w = Weather::default();
        w.forecast = Some(sample());
        let start = w.forecast.as_ref().unwrap().hours[0].time;
        assert_eq!(w.range_key(&settings, Some(start)).1, Some(start + 3600));
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
