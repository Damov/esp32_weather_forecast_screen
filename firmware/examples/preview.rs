//! Render the actual firmware UI on the host, without a connected board.
use embedded_graphics::{
    pixelcolor::{Rgb565, Rgb888},
    prelude::*,
};
use std::{
    convert::Infallible,
    fs,
    io::{BufWriter, Write},
    path::PathBuf,
};
use weather_forecast_firmware::{
    i18n::Language,
    model::{App, Network, Page, Security},
    ui,
};

struct Canvas(Vec<Rgb565>);
impl OriginDimensions for Canvas {
    fn size(&self) -> Size {
        Size::new(320, 480)
    }
}
impl DrawTarget for Canvas {
    type Color = Rgb565;
    type Error = Infallible;
    fn draw_iter<I: IntoIterator<Item = Pixel<Rgb565>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Self::Error> {
        for Pixel(p, c) in pixels {
            if (0..320).contains(&p.x) && (0..480).contains(&p.y) {
                self.0[(p.y * 320 + p.x) as usize] = c;
            }
        }
        Ok(())
    }
}
fn main() -> std::io::Result<()> {
    let dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "/tmp/weather-wifi-preview".into()),
    );
    fs::create_dir_all(&dir)?;
    for (name, page, keyboard, show_password) in [
        ("starting", Page::Starting, 0, false),
        ("unavailable", Page::Problem, 0, false),
        ("no-networks", Page::Problem, 0, false),
        ("networks", Page::Networks, 0, false),
        ("scanning", Page::Networks, 0, false),
        ("password", Page::Password, 0, false),
        ("password-visible", Page::Password, 0, true),
        ("keyboard-uppercase", Page::Password, 1, false),
        ("keyboard-symbols", Page::Password, 2, false),
        ("password-short", Page::Password, 0, false),
        ("connecting", Page::Connecting, 0, false),
        ("connected", Page::Connected, 0, false),
        ("fatal", Page::Fatal, 0, false),
    ] {
        let mut app = App {
            page,
            keyboard,
            show_password,
            ..App::default()
        };
        app.saved_ssid = Some("Café École Straße München".into());
        app.message = match page {
            Page::Starting => "Checking saved Wi-Fi settings...",
            Page::Problem => "The previously selected Wi-Fi network is unavailable. Please set up a new network or retry.",
            Page::Networks => "Select a secured 2.4 GHz network.",
            Page::Password => "Enter the Wi-Fi password.",
            Page::Connecting => "Connecting to the selected network...",
            Page::Connected => "Wi-Fi connected.",
            Page::Fatal => "Device storage could not be read. Restart and check the device storage.",
        }.into();
        app.networks = [
            "Café École Straße München",
            "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW",
            "Home Wi-Fi",
            "Office",
            "École",
            "Guest",
            "Router",
            "Workshop",
        ]
        .iter()
        .enumerate()
        .map(|(i, ssid)| Network {
            ssid: (*ssid).into(),
            security: Security::Wpa2,
            rssi: -30 - i as i8 * 5,
        })
        .collect();
        if page == Page::Password {
            app.selected = Some(app.networks[0].clone());
            // Synthetic test password exercises the widest printable characters.
            app.password.push_str(if name == "password-short" {
                "short"
            } else {
                "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW"
            });
        }
        if name == "scanning" {
            app.scanning = true;
        }
        if name == "no-networks" {
            app.saved_ssid = None;
            app.message = "No Wi-Fi networks are available nearby.".into();
        }
        let mut canvas = Canvas(vec![Rgb565::BLACK; 320 * 480]);
        ui::render(&mut canvas, &app).unwrap();
        save(&dir, name, canvas)?;
        for language in [Language::Russian, Language::Ukrainian] {
            app.preferences.current.language = language;
            let mut canvas = Canvas(vec![Rgb565::BLACK; 320 * 480]);
            ui::render(&mut canvas, &app).unwrap();
            save(&dir, &format!("{name}-{}", language.code()), canvas)?;
        }
    }
    for (name, message, target) in [
        ("startup-calibration", "Starting weather display...\nHold BOOT now to calibrate touch.", None),
        ("calibration-corner", "Touch calibration\nTap the yellow cross with a stylus.\nLift the stylus between targets.", Some((24, 24))),
        ("calibration-center", "Verify calibration\nTap the yellow cross at the center.", Some((160, 240))),
        ("calibration-error", "Calibration did not align.\nPlease try the targets again.", None),
    ] {
        let mut canvas = Canvas(vec![Rgb565::BLACK; 320 * 480]);
        ui::render_prompt(&mut canvas, message, target).unwrap();
        save(&dir, name, canvas)?;
        for language in [Language::Russian, Language::Ukrainian] {
            let mut canvas = Canvas(vec![Rgb565::BLACK; 320 * 480]);
            ui::render_prompt(&mut canvas, &weather_forecast_firmware::i18n::multiline(language, message), target).unwrap();
            save(&dir, &format!("{name}-{}", language.code()), canvas)?;
        }
    }
    for language in weather_forecast_firmware::i18n::Language::ALL {
        for (name, overlay) in [
            (
                "settings",
                weather_forecast_firmware::settings::Overlay::Settings,
            ),
            (
                "sleep-timeout",
                weather_forecast_firmware::settings::Overlay::SleepTimeout,
            ),
            (
                "languages",
                weather_forecast_firmware::settings::Overlay::Languages,
            ),
            (
                "location-input",
                weather_forecast_firmware::settings::Overlay::LocationInput,
            ),
            (
                "location-results",
                weather_forecast_firmware::settings::Overlay::LocationResults,
            ),
            (
                "reset",
                weather_forecast_firmware::settings::Overlay::ResetWifi,
            ),
            (
                "calibration-intro",
                weather_forecast_firmware::settings::Overlay::CalibrationIntro,
            ),
            (
                "calibration-error",
                weather_forecast_firmware::settings::Overlay::CalibrationError,
            ),
        ] {
            let mut app = App::default();
            app.preferences.current.language = language;
            app.preferences.overlay = Some(overlay);
            app.preferences.epoch = Some(1_790_942_400);
            app.preferences.results = vec![
                weather_forecast_firmware::settings::Location {
                    name: "London, England, United Kingdom".into(),
                    ..Default::default()
                },
                weather_forecast_firmware::settings::Location {
                    name: "London, Ontario, Canada".into(),
                    compact_name: Some("London, Canada".into()),
                    latitude: 42.9834,
                    longitude: -81.233,
                    timezone: "America/Toronto".into(),
                },
            ];
            app.preferences.result_selected = Some(0);
            app.preferences.query = "London".into();
            if name == "location-input" {
                app.preferences.query.clear();
                if matches!(language, Language::Russian | Language::Ukrainian) {
                    app.preferences.city_cyrillic = true;
                    app.preferences.keyboard = 4;
                }
                app.preferences.current.location.name =
                    "Frankfurt am Main, Hessen, Deutschland".into();
            }
            let mut canvas = Canvas(vec![Rgb565::BLACK; 320 * 480]);
            ui::render(&mut canvas, &app).unwrap();
            save(&dir, &format!("{name}-{}", language.code()), canvas)?;
        }
    }
    for language in [Language::Russian, Language::Ukrainian] {
        for (name, mode, cyrillic) in [
            ("cyrillic-lower", 4, true),
            ("cyrillic-upper", 5, true),
            ("latin", 0, false),
            ("symbols", 2, true),
            ("accents", 3, true),
        ] {
            let mut app = App::default();
            app.preferences.current.language = language;
            app.preferences.overlay =
                Some(weather_forecast_firmware::settings::Overlay::LocationInput);
            app.preferences.city_cyrillic = cyrillic;
            app.preferences.keyboard = mode;
            app.preferences.query = if language == Language::Ukrainian {
                "Київ"
            } else {
                "Москва Ё"
            }
            .into();
            let mut canvas = Canvas(vec![Rgb565::BLACK; 320 * 480]);
            ui::render(&mut canvas, &app).unwrap();
            save(
                &dir,
                &format!("location-{name}-{}", language.code()),
                canvas,
            )?;
        }
    }
    for language in weather_forecast_firmware::i18n::Language::ALL {
        for (name, status) in [
            (
                "present",
                weather_forecast_firmware::radar::PresenceStatus::Present,
            ),
            (
                "absent",
                weather_forecast_firmware::radar::PresenceStatus::Absent,
            ),
            (
                "unavailable",
                weather_forecast_firmware::radar::PresenceStatus::Unavailable,
            ),
        ] {
            let mut app = App {
                radar: status,
                ..App::default()
            };
            app.preferences.current.language = language;
            app.preferences.overlay = Some(weather_forecast_firmware::settings::Overlay::Settings);
            app.preferences.current.wake_on_presence = status.present();
            app.preferences.epoch = Some(1_700_000_000);
            let mut canvas = Canvas(vec![Rgb565::BLACK; 320 * 480]);
            ui::render(&mut canvas, &app).unwrap();
            save(&dir, &format!("radar-{name}-{}", language.code()), canvas)?;
        }
    }
    for language in Language::ALL {
        for view in [
            weather_forecast_firmware::weather::View::Clock,
            weather_forecast_firmware::weather::View::Week,
            weather_forecast_firmware::weather::View::Hours,
        ] {
            for scenario in [
                "ready",
                "loading",
                "error",
                "outdated",
                "night",
                "cold",
                "long-location",
            ] {
                let mut app = App {
                    page: Page::Connected,
                    ..App::default()
                };
                app.preferences.current.language = language;
                app.preferences.current.clock_24h = language == Language::German;
                app.preferences.current.fahrenheit = language == Language::English;
                app.preferences.epoch = Some(
                    1_790_899_200
                        + if scenario == "night" {
                            23 * 3600
                        } else {
                            12 * 3600
                        },
                );
                app.weather.view = view;
                if scenario != "loading" && scenario != "error" {
                    let mut forecast = weather_forecast_firmware::weather::parse(include_bytes!(
                        "../tests/fixtures/weather.json"
                    ))
                    .unwrap();
                    if scenario == "cold" {
                        forecast.current.temperature_2m = Some(-12.5);
                        forecast.current.apparent_temperature = Some(-18.2);
                    }
                    if scenario == "night" {
                        forecast.current.is_day = Some(0);
                        forecast.current.weather_code = Some(0);
                    }
                    app.weather.forecast = Some(forecast);
                }
                app.weather.failed = scenario == "error" || scenario == "outdated";
                if scenario == "long-location" {
                    app.preferences.current.location.name =
                        "Café École Straße München, Bayern, Deutschland".into();
                    app.preferences.current.location.compact_name = None;
                }
                let mut canvas = Canvas(vec![Rgb565::BLACK; 320 * 480]);
                ui::render(&mut canvas, &app).unwrap();
                save(
                    &dir,
                    &format!("weather-{:?}-{scenario}-{}", view, language.code()),
                    canvas,
                )?;
            }
        }
    }
    // Friendly examples for the README; stress-test previews above remain available.
    for page in [Page::Networks, Page::Password] {
        let mut app = App {
            page,
            ..App::default()
        };
        app.networks = ["Home Wi-Fi", "Office", "Guest"]
            .into_iter()
            .map(|ssid| Network {
                ssid: ssid.into(),
                security: Security::Wpa2,
                rssi: -45,
            })
            .collect();
        let name = if page == Page::Networks {
            "guide-networks"
        } else {
            "guide-password"
        };
        app.message = if page == Page::Networks {
            "Select a secured 2.4 GHz network."
        } else {
            "Enter the Wi-Fi password."
        }
        .into();
        if page == Page::Password {
            app.selected = Some(app.networks[0].clone());
            app.password.push_str("ExampleOnly123");
        }
        let mut canvas = Canvas(vec![Rgb565::BLACK; 320 * 480]);
        ui::render(&mut canvas, &app).unwrap();
        save(&dir, name, canvas)?;
    }
    println!("UI previews written to {}", dir.display());
    Ok(())
}

fn save(dir: &std::path::Path, name: &str, canvas: Canvas) -> std::io::Result<()> {
    let mut image = BufWriter::new(fs::File::create(dir.join(format!("{name}.ppm")))?);
    image.write_all(b"P6\n320 480\n255\n")?;
    for pixel in canvas.0 {
        let rgb = Rgb888::from(pixel);
        image.write_all(&[rgb.r(), rgb.g(), rgb.b()])?;
    }
    image.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::primitives::Rectangle;
    use weather_forecast_firmware::{settings::Overlay, settings_ui};

    #[test]
    fn presence_updates_match_full_render_and_leave_every_other_pixel_unchanged() {
        use weather_forecast_firmware::radar::PresenceStatus;
        for overlay in [None, Some(Overlay::Settings)] {
            for page in [Page::Problem, Page::Connected] {
                let mut app = App {
                    page,
                    ..App::default()
                };
                app.preferences.overlay = overlay;
                let mut actual = Canvas(vec![Rgb565::BLACK; 320 * 480]);
                ui::render(&mut actual, &app).unwrap();
                for status in [
                    PresenceStatus::Present,
                    PresenceStatus::Absent,
                    PresenceStatus::Unavailable,
                ] {
                    let previous = actual.0.clone();
                    app.radar = status;
                    ui::render_presence(&mut actual, &app).unwrap();
                    let mut expected = Canvas(vec![Rgb565::BLACK; 320 * 480]);
                    ui::render(&mut expected, &app).unwrap();
                    assert_eq!(actual.0, expected.0);
                    for (index, pixel) in actual.0.iter().enumerate() {
                        let point = Point::new((index % 320) as i32, (index / 320) as i32);
                        if !ui::presence_bounds().contains(point) {
                            assert_eq!(*pixel, previous[index]);
                        }
                    }
                }
            }
        }
        let app = App {
            page: Page::Password,
            ..App::default()
        };
        let mut actual = Canvas(vec![Rgb565::BLACK; 320 * 480]);
        ui::render(&mut actual, &app).unwrap();
        let previous = actual.0.clone();
        ui::render_presence(&mut actual, &app).unwrap();
        assert_eq!(actual.0, previous);
    }

    #[test]
    fn screen_blanking_keeps_weather_and_clock_current_without_changing_page() {
        use weather_forecast_firmware::{
            screen::{ScreenController, State},
            weather::{self, View},
            weather_ui,
        };
        for view in [View::Clock, View::Week, View::Hours] {
            let mut app = App {
                page: Page::Connected,
                ..App::default()
            };
            app.preferences.current.sleep_minutes = 1;
            app.preferences.current.wake_on_presence = false;
            app.preferences.epoch = Some(1_790_899_200 + 12 * 3600);
            app.weather.view = view;
            app.weather.adopt(&app.preferences.current.location);
            let (id, location) = app.weather.request(0, true).unwrap();
            app.weather.receive(
                id,
                &location,
                Ok(weather::parse(include_bytes!("../tests/fixtures/weather.json")).unwrap()),
                0,
                app.preferences.epoch,
            );
            let mut screen = ScreenController::new(&app.preferences.current, 0);
            let mut brightness = 255;
            screen
                .update(
                    &app.preferences.current,
                    app.preferences.epoch,
                    0,
                    false,
                    |v| {
                        brightness = v;
                        Ok::<_, ()>(())
                    },
                )
                .unwrap();
            let mut canvas = Canvas(vec![Rgb565::BLACK; 320 * 480]);
            ui::render(&mut canvas, &app).unwrap();
            screen
                .update(
                    &app.preferences.current,
                    app.preferences.epoch,
                    60_000,
                    false,
                    |v| {
                        brightness = v;
                        Ok::<_, ()>(())
                    },
                )
                .unwrap();
            assert_eq!(brightness, 0);
            assert_eq!(screen.state(), State::Off);
            if view == View::Clock {
                let before = canvas.0.clone();
                app.preferences
                    .update_time(app.preferences.epoch.map(|t| t + 1));
                weather_ui::render_clock(&mut canvas, &app).unwrap();
                assert_ne!(before, canvas.0);
            }
            let before = canvas.0.clone();
            let (id, location) = app.weather.request(600_000, true).unwrap();
            let mut refreshed =
                weather::parse(include_bytes!("../tests/fixtures/weather.json")).unwrap();
            refreshed.current.temperature_2m = Some(22.0);
            refreshed.days.iter_mut().for_each(|d| d.high = Some(25.0));
            refreshed
                .hours
                .iter_mut()
                .for_each(|h| h.temperature = Some(23.0));
            assert!(app.weather.receive(
                id,
                &location,
                Ok(refreshed),
                600_001,
                app.preferences.epoch
            ));
            ui::render(&mut canvas, &app).unwrap();
            assert_ne!(before, canvas.0);
            assert_eq!(screen.state(), State::Off);
            let latest = canvas.0.clone();
            assert!(screen.touch(
                &app.preferences.current,
                app.preferences.epoch,
                600_020,
                false
            ));
            screen
                .update(
                    &app.preferences.current,
                    app.preferences.epoch,
                    600_020,
                    false,
                    |v| {
                        brightness = v;
                        Ok::<_, ()>(())
                    },
                )
                .unwrap();
            assert_eq!(brightness, app.preferences.current.brightness);
            assert_eq!(screen.state(), State::On);
            assert_eq!(app.page, Page::Connected);
            assert_eq!(app.weather.view, view);
            assert_eq!(canvas.0, latest); // No initialization or repaint required on wake.
        }
    }

    #[test]
    fn weather_seconds_update_matches_complete_render() {
        use weather_forecast_firmware::{
            weather::{self, View},
            weather_ui,
        };
        let mut buffer = weather_ui::ClockBuffer::new().unwrap();
        for language in Language::ALL {
            for clock_24h in [false, true] {
                let mut app = App {
                    page: Page::Connected,
                    ..App::default()
                };
                app.preferences.current.language = language;
                app.preferences.current.clock_24h = clock_24h;
                app.weather.view = View::Clock;
                app.weather.forecast =
                    Some(weather::parse(include_bytes!("../tests/fixtures/weather.json")).unwrap());
                for initial in [
                    1_790_899_200 + 12 * 3600 + 59,
                    1_790_899_200 + 11 * 3600 + 3599,
                    1_790_899_200 + 22 * 3600 + 3599,
                    1_790_899_200 + 24 * 3600 - 1,
                ] {
                    app.preferences.epoch = Some(initial);
                    let mut actual = Canvas(vec![Rgb565::BLACK; 320 * 480]);
                    ui::render(&mut actual, &app).unwrap();
                    buffer.remember(&app);
                    let previous = actual.0.clone();
                    app.preferences.epoch = Some(initial + 1);
                    buffer.render(&mut actual, &app).unwrap();
                    let mut expected = Canvas(vec![Rgb565::BLACK; 320 * 480]);
                    ui::render(&mut expected, &app).unwrap();
                    assert_eq!(actual.0, expected.0);
                    for (i, pixel) in actual.0.iter().enumerate() {
                        if !weather_ui::clock_bounds()
                            .contains(Point::new((i % 320) as i32, (i / 320) as i32))
                        {
                            assert_eq!(*pixel, previous[i]);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn clock_updates_write_exactly_changed_pixels_and_recover_from_errors() {
        use embedded_graphics::primitives::Rectangle;
        use weather_forecast_firmware::weather_ui;
        struct Display {
            canvas: Canvas,
            writes: Vec<Rectangle>,
            fail_after: Option<usize>,
        }
        impl OriginDimensions for Display {
            fn size(&self) -> Size {
                Size::new(320, 480)
            }
        }
        impl DrawTarget for Display {
            type Color = Rgb565;
            type Error = &'static str;
            fn draw_iter<I: IntoIterator<Item = Pixel<Rgb565>>>(
                &mut self,
                _: I,
            ) -> Result<(), Self::Error> {
                panic!("Clock must send composed strips, not individual drawing operations")
            }
            fn fill_solid(&mut self, _: &Rectangle, _: Rgb565) -> Result<(), Self::Error> {
                panic!("Clock must never clear the LCD before drawing text")
            }
            fn fill_contiguous<I: IntoIterator<Item = Rgb565>>(
                &mut self,
                area: &Rectangle,
                colors: I,
            ) -> Result<(), Self::Error> {
                if self.fail_after == Some(self.writes.len()) {
                    return Err("SPI failed");
                }
                let colors: Vec<_> = colors.into_iter().collect();
                assert_eq!(colors.len(), (area.size.width * area.size.height) as usize);
                self.writes.push(*area);
                self.canvas.fill_contiguous(area, colors).unwrap();
                Ok(())
            }
        }
        let mut app = App {
            page: Page::Connected,
            ..App::default()
        };
        app.preferences.current.location.timezone = "UTC".into();
        let mut buffer = weather_ui::ClockBuffer::new().unwrap();
        let mut display = Display {
            canvas: Canvas(vec![Rgb565::BLACK; 320 * 480]),
            writes: vec![],
            fail_after: None,
        };
        // First draw initializes the clock; subsequent writes must match exactly
        // the set of changed final pixels, including sync loss and rollovers.
        for (index, epoch) in [
            None,
            Some(1_790_899_200),
            Some(1_790_899_201),
            Some(1_790_899_259),
            Some(1_790_899_260),
            Some(1_790_899_200 + 12 * 3600 - 1),
            Some(1_790_899_200 + 12 * 3600),
            Some(1_790_899_200 + 24 * 3600 - 1),
            Some(1_790_899_200 + 24 * 3600),
            None,
        ]
        .into_iter()
        .enumerate()
        {
            let before = display.canvas.0.clone();
            app.preferences.epoch = epoch;
            display.writes.clear();
            buffer.render(&mut display, &app).unwrap();
            let mut expected = Canvas(vec![Rgb565::BLACK; 320 * 480]);
            weather_ui::render_clock(&mut expected, &app).unwrap();
            assert_eq!(display.canvas.0, expected.0);
            let mut written = vec![0u8; 320 * 480];
            for area in &display.writes {
                for point in area.points() {
                    let i = (point.y * 320 + point.x) as usize;
                    assert!(weather_ui::clock_bounds().contains(point));
                    written[i] += 1;
                }
            }
            if index == 0 {
                assert_eq!(display.writes.len(), 17);
                assert_eq!(written.iter().filter(|n| **n == 1).count(), 296 * 262);
            } else {
                for i in 0..written.len() {
                    assert_eq!(written[i], u8::from(before[i] != expected.0[i]));
                }
            }
            display.writes.clear();
            buffer.render(&mut display, &app).unwrap();
            assert!(display.writes.is_empty());
        }
        // A partially transmitted frame invalidates the baseline. Recovery must
        // rewrite the whole clock even if the requested content changes again.
        app.preferences.epoch = Some(1_790_899_200);
        display.writes.clear();
        display.fail_after = Some(1);
        assert_eq!(buffer.render(&mut display, &app), Err("SPI failed"));
        assert_eq!(display.writes.len(), 1);
        display.fail_after = None;
        display.writes.clear();
        app.preferences.epoch = Some(1_790_899_201);
        buffer.render(&mut display, &app).unwrap();
        assert_eq!(display.writes.len(), 17);
        let mut expected = Canvas(vec![Rgb565::BLACK; 320 * 480]);
        weather_ui::render_clock(&mut expected, &app).unwrap();
        assert_eq!(display.canvas.0, expected.0);
        // Formatting/language/timezone changes use the same final-pixel comparison.
        for language in Language::ALL {
            for clock_24h in [false, true] {
                let before = display.canvas.0.clone();
                app.preferences.current.language = language;
                app.preferences.current.clock_24h = clock_24h;
                app.preferences.current.location.timezone = "Europe/Berlin".into();
                display.writes.clear();
                buffer.render(&mut display, &app).unwrap();
                let mut expected = Canvas(vec![Rgb565::BLACK; 320 * 480]);
                weather_ui::render_clock(&mut expected, &app).unwrap();
                assert_eq!(display.canvas.0, expected.0);
                let mut written = vec![0u8; 320 * 480];
                for area in &display.writes {
                    for point in area.points() {
                        written[(point.y * 320 + point.x) as usize] += 1;
                    }
                }
                for i in 0..written.len() {
                    assert_eq!(written[i], u8::from(before[i] != expected.0[i]));
                }
            }
        }
        // Full redraw establishes a fresh baseline; leaving the page discards it.
        buffer.remember(&app);
        display.writes.clear();
        buffer.render(&mut display, &app).unwrap();
        assert!(display.writes.is_empty());
        buffer.invalidate();
        buffer.render(&mut display, &app).unwrap();
        assert_eq!(display.writes.len(), 17);
    }

    #[test]
    fn weather_updates_write_only_changed_pixels_and_recover() {
        use embedded_graphics::primitives::Rectangle;
        use weather_forecast_firmware::{weather, weather_ui};
        struct Display {
            canvas: Canvas,
            writes: Vec<Rectangle>,
            fail_after: Option<usize>,
        }
        impl OriginDimensions for Display {
            fn size(&self) -> Size {
                Size::new(320, 480)
            }
        }
        impl DrawTarget for Display {
            type Color = Rgb565;
            type Error = &'static str;
            fn draw_iter<I: IntoIterator<Item = Pixel<Rgb565>>>(
                &mut self,
                _: I,
            ) -> Result<(), Self::Error> {
                panic!("Weather must send composed strips, not individual drawing operations")
            }
            fn fill_solid(&mut self, _: &Rectangle, _: Rgb565) -> Result<(), Self::Error> {
                panic!("Weather must never clear the LCD before drawing text")
            }
            fn fill_contiguous<I: IntoIterator<Item = Rgb565>>(
                &mut self,
                area: &Rectangle,
                colors: I,
            ) -> Result<(), Self::Error> {
                if self.fail_after == Some(self.writes.len()) {
                    return Err("SPI failed");
                }
                let colors: Vec<_> = colors.into_iter().collect();
                assert_eq!(colors.len(), (area.size.width * area.size.height) as usize);
                self.writes.push(*area);
                self.canvas.fill_contiguous(area, colors).unwrap();
                Ok(())
            }
        }
        for view in [
            weather::View::Clock,
            weather::View::Week,
            weather::View::Hours,
        ] {
            for language in Language::ALL {
                for fahrenheit in [false, true] {
                    let mut app = App {
                        page: Page::Connected,
                        ..App::default()
                    };
                    app.weather.view = view;
                    app.preferences.current.language = language;
                    app.preferences.current.clock_24h = !fahrenheit;
                    app.preferences.current.fahrenheit = fahrenheit;
                    app.preferences.epoch = Some(1_790_899_200 + 12 * 3600);
                    let mut initial = Canvas(vec![Rgb565::BLACK; 320 * 480]);
                    ui::render(&mut initial, &app).unwrap();
                    if view != weather::View::Clock {
                        assert_eq!(
                            initial.0[200 * 320 + 13],
                            weather_forecast_firmware::theme::WEATHER_CARD
                        );
                    }
                    let mut display = Display {
                        canvas: initial,
                        writes: vec![],
                        fail_after: None,
                    };
                    let mut buffer = weather_ui::ClockBuffer::new().unwrap();
                    buffer.remember(&app);
                    for step in 0..15 {
                        match step {
                            0 => app.weather.failed = true,
                            1 => {
                                app.weather.failed = false;
                                app.weather.forecast = Some(
                                    weather::parse(include_bytes!(
                                        "../tests/fixtures/weather.json"
                                    ))
                                    .unwrap(),
                                );
                            }
                            2 => {
                                let current = &mut app.weather.forecast.as_mut().unwrap().current;
                                current.temperature_2m = Some(-12.5);
                                current.apparent_temperature = Some(-18.2);
                                current.weather_code = Some(61);
                            }
                            3 => {
                                let current = &mut app.weather.forecast.as_mut().unwrap().current;
                                current.temperature_2m = None;
                                current.apparent_temperature = None;
                                current.weather_code = None;
                            }
                            4 => app.weather.failed = true,
                            5 => app.weather.failed = false,
                            6 => {
                                app.weather.fetched_epoch = app.preferences.epoch.map(|t| t - 1200)
                            }
                            7 => {
                                let forecast = app.weather.forecast.as_mut().unwrap();
                                for day in &mut forecast.days {
                                    day.low = Some(-25.0);
                                    day.high = Some(30.0);
                                    day.code = Some(95);
                                }
                                for hour in &mut forecast.hours {
                                    hour.temperature = Some(-15.0);
                                    hour.precipitation = Some(100);
                                    hour.code = Some(71);
                                }
                            }
                            8 => {
                                let forecast = app.weather.forecast.as_mut().unwrap();
                                for day in &mut forecast.days {
                                    day.low = None;
                                    day.high = None;
                                    day.code = None;
                                }
                                for hour in &mut forecast.hours {
                                    hour.temperature = None;
                                    hour.precipitation = None;
                                    hour.code = None;
                                }
                            }
                            9 => app.preferences.epoch = app.preferences.epoch.map(|t| t + 3600),
                            10 => app.preferences.epoch = app.preferences.epoch.map(|t| t + 86400),
                            11 => {
                                for day in &mut app.weather.forecast.as_mut().unwrap().days {
                                    day.precipitation_probability_mean = Some(100.0);
                                }
                            }
                            12 => {
                                for day in &mut app.weather.forecast.as_mut().unwrap().days {
                                    day.precipitation_probability_mean = None;
                                }
                            }
                            13 => app.preferences.epoch = None,
                            _ => {
                                app.weather.forecast = None;
                                app.weather.failed = false;
                                app.weather.fetched_epoch = None;
                            }
                        }
                        if view == weather::View::Clock {
                            // Isolate weather updates from the independently refreshed clock.
                            buffer.render(&mut display, &app).unwrap();
                        }
                        let before = display.canvas.0.clone();
                        display.writes.clear();
                        buffer.render_weather(&mut display, &app).unwrap();
                        if (11..=12).contains(&step) {
                            if view == weather::View::Week {
                                assert!(!display.writes.is_empty());
                                for area in &display.writes {
                                    assert!(
                                        area.points().all(|point| (112..180).contains(&point.x)
                                            && (187..429).contains(&point.y)),
                                        "probability-only updates must stay in their column"
                                    );
                                }
                            } else {
                                assert!(display.writes.is_empty());
                            }
                        }
                        let mut expected = Canvas(vec![Rgb565::BLACK; 320 * 480]);
                        ui::render(&mut expected, &app).unwrap();
                        assert!(
                            display.canvas.0 == expected.0,
                            "{view:?} {language:?} F={fahrenheit} mismatch at {:?}",
                            display
                                .canvas
                                .0
                                .iter()
                                .zip(&expected.0)
                                .position(|(a, b)| a != b)
                        );
                        let mut written = vec![0u8; 320 * 480];
                        for area in &display.writes {
                            for point in area.points() {
                                assert!(if view == weather::View::Clock {
                                    weather_ui::weather_bounds()
                                        .iter()
                                        .any(|bounds| bounds.contains(point))
                                } else {
                                    weather_ui::overview_bounds().contains(point)
                                });
                                written[(point.y * 320 + point.x) as usize] += 1;
                            }
                        }
                        for i in 0..written.len() {
                            assert_eq!(written[i], u8::from(before[i] != expected.0[i]));
                        }
                        display.writes.clear();
                        buffer.render_weather(&mut display, &app).unwrap();
                        assert!(display.writes.is_empty());
                    }
                    // Both regions can update in one loop, with independent baselines.
                    app.weather.forecast = Some(
                        weather::parse(include_bytes!("../tests/fixtures/weather.json")).unwrap(),
                    );
                    app.preferences.epoch = Some(1_790_899_200 + 12 * 3600 + 1);
                    display.writes.clear();
                    display.fail_after = Some(1);
                    assert_eq!(buffer.render_weather(&mut display, &app), Err("SPI failed"));
                    assert_eq!(display.writes.len(), 1);
                    display.fail_after = None;
                    display.writes.clear();
                    buffer.render_weather(&mut display, &app).unwrap();
                    assert_eq!(
                        display.writes.len(),
                        if view == weather::View::Clock { 9 } else { 25 }
                    );
                    if view == weather::View::Clock {
                        buffer.render(&mut display, &app).unwrap();
                    }
                    let mut expected = Canvas(vec![Rgb565::BLACK; 320 * 480]);
                    ui::render(&mut expected, &app).unwrap();
                    assert!(
                        display.canvas.0 == expected.0,
                        "{view:?} {language:?} F={fahrenheit} mismatch at {:?}",
                        display
                            .canvas
                            .0
                            .iter()
                            .zip(&expected.0)
                            .position(|(a, b)| a != b)
                    );
                    buffer.invalidate();
                    buffer.remember(&app);
                    display.writes.clear();
                    buffer.render_weather(&mut display, &app).unwrap();
                    assert!(display.writes.is_empty());
                }
            }
        }
    }

    #[test]
    fn overview_rollovers_and_view_changes_match_complete_renders() {
        use weather_forecast_firmware::{weather, weather_ui};
        let mut app = App {
            page: Page::Connected,
            ..App::default()
        };
        app.preferences.current.location.timezone = "Europe/Berlin".into();
        let mut buffer = weather_ui::ClockBuffer::new().unwrap();
        for timestamp in [
            "2026-03-29T00:59:59Z",
            "2026-10-25T00:59:59Z",
            "2026-09-30T21:59:59Z",
        ] {
            let epoch = chrono::DateTime::parse_from_rfc3339(timestamp)
                .unwrap()
                .timestamp();
            let date = chrono::DateTime::from_timestamp(epoch, 0)
                .unwrap()
                .date_naive();
            let mut forecast =
                weather::parse(include_bytes!("../tests/fixtures/weather.json")).unwrap();
            forecast.hours = (0..10)
                .map(|i| weather::Hour {
                    time: epoch + 1 + i * 3600,
                    temperature: Some(i as f64),
                    precipitation: Some(i as u8 * 10),
                    code: Some(0),
                    is_day: Some(1),
                })
                .collect();
            forecast.days = (0..8)
                .map(|i| weather::Day {
                    date: date + chrono::Days::new(i),
                    low: Some(i as f64),
                    high: Some(i as f64 + 10.0),
                    precipitation_probability_mean: Some(i as f64 * 10.0),
                    code: Some(0),
                })
                .collect();
            app.weather.forecast = Some(forecast);
            for clock_24h in [false, true] {
                app.preferences.current.clock_24h = clock_24h;
                for view in [
                    weather::View::Week,
                    weather::View::Hours,
                    weather::View::Clock,
                    weather::View::Hours,
                ] {
                    app.weather.view = view;
                    app.preferences.epoch = Some(epoch);
                    let mut actual = Canvas(vec![Rgb565::BLACK; 320 * 480]);
                    ui::render(&mut actual, &app).unwrap();
                    buffer.invalidate();
                    buffer.remember(&app);
                    app.preferences.epoch = Some(epoch + 1);
                    buffer.render_weather(&mut actual, &app).unwrap();
                    if view == weather::View::Clock {
                        buffer.render(&mut actual, &app).unwrap();
                    }
                    let mut expected = Canvas(vec![Rgb565::BLACK; 320 * 480]);
                    ui::render(&mut expected, &app).unwrap();
                    assert_eq!(actual.0, expected.0, "{timestamp} {view:?}");
                }
            }
        }
    }

    #[test]
    fn analog_and_digital_updates_match_across_dst_transitions() {
        use weather_forecast_firmware::weather_ui;
        let mut app = App {
            page: Page::Connected,
            ..App::default()
        };
        app.preferences.current.location.timezone = "Europe/Berlin".into();
        for timestamp in ["2026-03-29T00:59:59Z", "2026-10-25T00:59:59Z"] {
            let epoch = chrono::DateTime::parse_from_rfc3339(timestamp)
                .unwrap()
                .timestamp();
            app.preferences.epoch = Some(epoch);
            let mut actual = Canvas(vec![Rgb565::BLACK; 320 * 480]);
            ui::render(&mut actual, &app).unwrap();
            let mut buffer = weather_ui::ClockBuffer::new().unwrap();
            buffer.remember(&app);
            app.preferences.epoch = Some(epoch + 1);
            buffer.render(&mut actual, &app).unwrap();
            let mut expected = Canvas(vec![Rgb565::BLACK; 320 * 480]);
            ui::render(&mut expected, &app).unwrap();
            assert_eq!(actual.0, expected.0);
        }
    }

    #[test]
    fn seconds_leave_other_clock_cells_stationary() {
        use weather_forecast_firmware::weather_ui;
        let mut app = App {
            page: Page::Connected,
            ..App::default()
        };
        app.preferences.current.location.timezone = "UTC".into();
        app.preferences.current.clock_24h = true;
        let start = 1_790_899_200 + 12 * 3600 + 34 * 60;
        app.preferences.epoch = Some(start);
        let mut baseline = Canvas(vec![Rgb565::BLACK; 320 * 480]);
        weather_ui::render_clock(&mut baseline, &app).unwrap();
        let mut buffer = weather_ui::ClockBuffer::new().unwrap();
        buffer.remember(&app);
        let mut actual = Canvas(baseline.0.clone());
        for second in 1..60 {
            app.preferences.epoch = Some(start + second);
            buffer.render(&mut actual, &app).unwrap();
            let mut expected = Canvas(vec![Rgb565::BLACK; 320 * 480]);
            weather_ui::render_clock(&mut expected, &app).unwrap();
            assert_eq!(actual.0, expected.0);
            for y in 214..316 {
                for x in 12..308 {
                    if !(198..254).contains(&x) {
                        let i = y * 320 + x;
                        assert_eq!(
                            actual.0[i], baseline.0[i],
                            "Other cells moved at second {second}"
                        );
                    }
                }
            }
        }
        // The layout envelope is symmetric even for narrow ones and placeholders.
        for epoch in [Some(1_790_899_200 + 11 * 3600 + 11 * 60 + 11), None] {
            app.preferences.epoch = epoch;
            let mut canvas = Canvas(vec![Rgb565::BLACK; 320 * 480]);
            weather_ui::render_clock(&mut canvas, &app).unwrap();
            for y in 214..263 {
                for x in 12..308 {
                    if !(66..254).contains(&x) {
                        assert_eq!(
                            canvas.0[y * 320 + x],
                            weather_forecast_firmware::theme::background_at(y as i32)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn clock_update_matches_full_render_without_touching_controls() {
        let mut app = App::default();
        app.preferences.overlay = Some(Overlay::Settings);
        app.preferences.epoch = Some(1_700_000_000);
        let mut actual = Canvas(vec![Rgb565::BLACK; 320 * 480]);
        ui::render(&mut actual, &app).unwrap();
        let previous = actual.0.clone();
        app.preferences.epoch = Some(1_700_000_060);
        let bounds = Rectangle::new(Point::new(16, 40), Size::new(288, 32));
        settings_ui::render(&mut actual.clipped(&bounds), &app).unwrap();
        let mut expected = Canvas(vec![Rgb565::BLACK; 320 * 480]);
        ui::render(&mut expected, &app).unwrap();
        assert_eq!(actual.0, expected.0);
        for (index, pixel) in actual.0.iter().enumerate() {
            let point = Point::new((index % 320) as i32, (index / 320) as i32);
            if !bounds.contains(point) {
                assert_eq!(*pixel, previous[index]);
            }
        }
    }
}
