// ============================================================================= //
// File          : preview.rs                                                    //
// License       : GPL-3.0-only                                                  //
// Created       : 2026-10-04                                                    //
// Last modified : 2026-10-04                                                    //
// Author        : Daniel Kharlamov                                              //
// Last editor   : Daniel Kharlamov <daniel.kharlamov@googlemail.com>            //
// ============================================================================= //
//                                                                               //
// Summary:                                                                      //
// --------                                                                      //
// Host rendering and visual checks for the firmware interface.                  //
//                                                                               //
// Module:                                                                       //
// -------                                                                       //
// Renders synthetic Wi-Fi, settings, calibration, presence, and weather         //
// scenarios with the real UI on a host RGB565 canvas. Writes PPM preview images //
// and tests partial redraws, clock transitions, and display error recovery      //
// without connected hardware.                                                   //
// ============================================================================= //

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
    /// Reports the dimensions of the host preview canvas.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Canvas`) - Host RGB565 preview framebuffer. Borrowed without changing it.
    ///
    /// # Returns
    ///
    /// `Size` - Fixed portrait size of 320 by 480 pixels.
    fn size(&self) -> Size {
        // Construct the value from the supplied configuration or coordinates for use by the
        // surrounding operation.
        Size::new(320, 480)
    }
}
impl DrawTarget for Canvas {
    type Color = Rgb565;
    type Error = Infallible;
    /// Writes in-bounds RGB565 pixels to the host canvas.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Canvas`) - Host RGB565 preview framebuffer. Mutated in place.
    /// * `pixels` (`I`) - Iterable screen-coordinate pixels and RGB565 colours to draw.
    ///
    /// # Type Parameters
    ///
    /// * `I` - Input iterable with the item type required by this function's iterator bound.
    ///
    /// # Returns
    ///
    /// `Result<(), Self::Error>` - Always Ok; ignores off-screen pixels. The canvas error type
    /// is Infallible.
    fn draw_iter<I: IntoIterator<Item = Pixel<Rgb565>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Self::Error> {
        // Visit each entry in row-major RGB565 colour buffer or iterator; the loop binding provides
        // its value or index for this iteration.
        for Pixel(p, c) in pixels {
            // Check whether the value falls within the stated range or belongs to the supported set
            // and the value falls within the stated range or belongs to the supported set.
            if (0..320).contains(&p.x) && (0..480).contains(&p.y) {
                // Set the selected entry from 0 to credential or controller fixture used by the
                // surrounding test.
                self.0[(p.y * 320 + p.x) as usize] = c;
            }
        }
        // Return success after the required side effects are complete.
        Ok(())
    }
}
/// Renders synthetic interface scenarios and writes PPM previews without connected
/// hardware.
///
/// # Arguments
///
/// None.
///
/// # Returns
///
/// `std::io::Result<()>` - Ok after all preview files are written; Err for filesystem or
/// image write failure.
///
/// # Errors
///
/// Propagates output directory creation and PPM file I/O errors.
///
/// # Panics
///
/// Panics if synthetic forecast fixtures cannot be parsed or an infallible canvas render
/// unexpectedly fails.
fn main() -> std::io::Result<()> {
    // Keep preview output directory supplied on the command line or selected by the default path in
    // this local variable for the following operations.
    let dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "/tmp/weather-wifi-preview".into()),
    );
    // Run the create dir all operation with the supplied inputs.
    fs::create_dir_all(&dir)?;
    // Run each listed scenario or target in the declared order; the loop variables select the data
    // for that case.
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
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App {
            // Initialize currently selected Wi-Fi navigation page from the supplied value.
            page,
            // Initialize selected on-screen keyboard layout from the supplied value.
            keyboard,
            // Initialize whether entered password characters are visible instead of masked from the
            // supplied value.
            show_password,
            ..App::default()
        };
        // Set name of the successfully persisted station profile, if one exists to an available
        // value for into result for the surrounding operation.
        app.saved_ssid = Some("Café École Straße München".into());
        // Set user-facing status or error text to into result for the surrounding operation.
        app.message = match page {
            // Handle the Page Starting case: apply the state-specific behavior shown here.
            Page::Starting => "Checking saved Wi-Fi settings...",
            // Handle the Page Problem case: apply the state-specific behavior shown here.
            Page::Problem => "The previously selected Wi-Fi network is unavailable. Please set up a new network or retry.",
            // Handle the Page Networks case: apply the state-specific behavior shown here.
            Page::Networks => "Select a secured 2.4 GHz network.",
            // Handle the Page Password case: apply the state-specific behavior shown here.
            Page::Password => "Enter the Wi-Fi password.",
            // Handle the Page Connecting case: apply the state-specific behavior shown here.
            Page::Connecting => "Connecting to the selected network...",
            // Handle the Page Connected case: apply the state-specific behavior shown here.
            Page::Connected => "Wi-Fi connected.",
            // Handle the Page Fatal case: apply the state-specific behavior shown here.
            Page::Fatal => "Device storage could not be read. Restart and check the device storage.",
        }.into();
        // Set nearby access points available for selection to the entries collected from the
        // preceding iterator.
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
            // Initialize Wi-Fi network name, limited to 32 encoded bytes by station configuration
            // from the supplied value.
            ssid: (*ssid).into(),
            // Initialize supported Wi-Fi authentication mode from the supplied value.
            security: Security::Wpa2,
            // Initialize received signal strength in dBm; less-negative values represent stronger
            // signals from the supplied value.
            rssi: -30 - i as i8 * 5,
        })
        .collect();
        // Check whether currently selected Wi-Fi navigation page equals Password state.
        if page == Page::Password {
            // Set access point currently selected for candidate setup to an available value for an
            // owned copy of the selected entry from nearby access points available for selection.
            app.selected = Some(app.networks[0].clone());
            // Synthetic test password exercises the widest printable characters.
            // Check whether human-readable label for the selected object equals the specified
            // message, format, or data literal.
            app.password.push_str(if name == "password-short" {
                "short"
            } else {
                "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW"
            });
        }
        // Check whether human-readable label for the selected object equals the specified message,
        // format, or data literal.
        if name == "scanning" {
            // Set whether an asynchronous access-point scan is pending to the enabled state.
            app.scanning = true;
        }
        // Check whether human-readable label for the selected object equals the specified message,
        // format, or data literal.
        if name == "no-networks" {
            // Set name of the successfully persisted station profile, if one exists to no available
            // value.
            app.saved_ssid = None;
            // Set user-facing status or error text to into result for the surrounding operation.
            app.message = "No Wi-Fi networks are available nearby.".into();
        }
        // Keep host-side framebuffer representing the 320-by-480 display in this local variable for
        // the following operations.
        let mut canvas = Canvas(vec![Rgb565::BLACK; 320 * 480]);
        // Require the value guaranteed by this test fixture or internal invariant; unexpected
        // absence panics.
        ui::render(&mut canvas, &app).unwrap();
        // Writes a host RGB565 canvas as a binary RGB888 PPM image.
        save(&dir, name, canvas)?;
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for language in [Language::Russian, Language::Ukrainian] {
            // Set selected language for interface text and location search to selected language for
            // interface text and location search.
            app.preferences.current.language = language;
            // Keep host-side framebuffer representing the 320-by-480 display in this local variable
            // for the following operations.
            let mut canvas = Canvas(vec![Rgb565::BLACK; 320 * 480]);
            // Require the value guaranteed by this test fixture or internal invariant; unexpected
            // absence panics.
            ui::render(&mut canvas, &app).unwrap();
            // Writes a host RGB565 canvas as a binary RGB888 PPM image.
            save(&dir, &format!("{name}-{}", language.code()), canvas)?;
        }
    }
    // Run each listed scenario or target in the declared order; the loop variables select the data
    // for that case.
    for (name, message, target) in [
        ("startup-calibration", "Starting weather display...\nHold BOOT now to calibrate touch.", None),
        ("calibration-corner", "Touch calibration\nTap the yellow cross with a stylus.\nLift the stylus between targets.", Some((24, 24))),
        ("calibration-center", "Verify calibration\nTap the yellow cross at the center.", Some((160, 240))),
        ("calibration-error", "Calibration did not align.\nPlease try the targets again.", None),
    ] {
        // Keep host-side framebuffer representing the 320-by-480 display in this local variable for
        // the following operations.
        let mut canvas = Canvas(vec![Rgb565::BLACK; 320 * 480]);
        // Require the value guaranteed by this test fixture or internal invariant; unexpected
        // absence panics.
        ui::render_prompt(&mut canvas, message, target).unwrap();
        // Writes a host RGB565 canvas as a binary RGB888 PPM image.
        save(&dir, name, canvas)?;
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for language in [Language::Russian, Language::Ukrainian] {
            // Keep host-side framebuffer representing the 320-by-480 display in this local variable
            // for the following operations.
            let mut canvas = Canvas(vec![Rgb565::BLACK; 320 * 480]);
            // Require the value guaranteed by this test fixture or internal invariant; unexpected
            // absence panics.
            ui::render_prompt(&mut canvas, &weather_forecast_firmware::i18n::multiline(language, message), target).unwrap();
            // Writes a host RGB565 canvas as a binary RGB888 PPM image.
            save(&dir, &format!("{name}-{}", language.code()), canvas)?;
        }
    }
    // Visit each entry in weather_forecast_firmware i18n Language ALL; the loop binding provides
    // its value or index for this iteration.
    for language in weather_forecast_firmware::i18n::Language::ALL {
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
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
            // Keep application state used by navigation, preferences, weather, and rendering in
            // this local variable for the following operations.
            let mut app = App::default();
            // Set selected language for interface text and location search to selected language for
            // interface text and location search.
            app.preferences.current.language = language;
            // Set overlay to an available value for overlay.
            app.preferences.overlay = Some(overlay);
            // Set UTC Unix timestamp in seconds to an available value for 1_790_942_400.
            app.preferences.epoch = Some(1_790_942_400);
            // Set results to the formatted text or fixture created by vec.
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
            // Set result selected to an available value for 0.
            app.preferences.result_selected = Some(0);
            // Set city search text or encoded API request being examined to into result for the
            // surrounding operation.
            app.preferences.query = "London".into();
            // Check whether human-readable label for the selected object equals the specified
            // message, format, or data literal.
            if name == "location-input" {
                // Remove accumulated entries or transient state before beginning the next
                // operation.
                app.preferences.query.clear();
                // Check whether the formatted text or fixture created by matches.
                if matches!(language, Language::Russian | Language::Ukrainian) {
                    // Set city cyrillic to the enabled state.
                    app.preferences.city_cyrillic = true;
                    // Set selected on-screen keyboard layout to 4.
                    app.preferences.keyboard = 4;
                }
                // Set human-readable label for the selected object to into result for the
                // surrounding operation.
                app.preferences.current.location.name =
                    "Frankfurt am Main, Hessen, Deutschland".into();
            }
            // Keep host-side framebuffer representing the 320-by-480 display in this local variable
            // for the following operations.
            let mut canvas = Canvas(vec![Rgb565::BLACK; 320 * 480]);
            // Require the value guaranteed by this test fixture or internal invariant; unexpected
            // absence panics.
            ui::render(&mut canvas, &app).unwrap();
            // Writes a host RGB565 canvas as a binary RGB888 PPM image.
            save(&dir, &format!("{name}-{}", language.code()), canvas)?;
        }
    }
    // Run each listed scenario or target in the declared order; the loop variables select the data
    // for that case.
    for language in [Language::Russian, Language::Ukrainian] {
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for (name, mode, cyrillic) in [
            ("cyrillic-lower", 4, true),
            ("cyrillic-upper", 5, true),
            ("latin", 0, false),
            ("symbols", 2, true),
            ("accents", 3, true),
        ] {
            // Keep application state used by navigation, preferences, weather, and rendering in
            // this local variable for the following operations.
            let mut app = App::default();
            // Set selected language for interface text and location search to selected language for
            // interface text and location search.
            app.preferences.current.language = language;
            // Set overlay to an available value for weather_forecast_firmware settings Overlay
            // LocationInput.
            app.preferences.overlay =
                Some(weather_forecast_firmware::settings::Overlay::LocationInput);
            // Set city cyrillic to cyrillic.
            app.preferences.city_cyrillic = cyrillic;
            // Set selected on-screen keyboard layout to mode.
            app.preferences.keyboard = mode;
            // Set city search text or encoded API request being examined to into result for the
            // surrounding operation.
            app.preferences.query = if language == Language::Ukrainian {
                "Київ"
            } else {
                "Москва Ё"
            }
            .into();
            // Keep host-side framebuffer representing the 320-by-480 display in this local variable
            // for the following operations.
            let mut canvas = Canvas(vec![Rgb565::BLACK; 320 * 480]);
            // Require the value guaranteed by this test fixture or internal invariant; unexpected
            // absence panics.
            ui::render(&mut canvas, &app).unwrap();
            // Writes a host RGB565 canvas as a binary RGB888 PPM image.
            save(
                &dir,
                &format!("location-{name}-{}", language.code()),
                canvas,
            )?;
        }
    }
    // Visit each entry in weather_forecast_firmware i18n Language ALL; the loop binding provides
    // its value or index for this iteration.
    for language in weather_forecast_firmware::i18n::Language::ALL {
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
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
            // Keep application state used by navigation, preferences, weather, and rendering in
            // this local variable for the following operations.
            let mut app = App {
                // Initialize combined presence indication from UART reports and the OUT pin from
                // the supplied value.
                radar: status,
                ..App::default()
            };
            // Set selected language for interface text and location search to selected language for
            // interface text and location search.
            app.preferences.current.language = language;
            // Set overlay to an available value for weather_forecast_firmware settings Overlay
            // Settings.
            app.preferences.overlay = Some(weather_forecast_firmware::settings::Overlay::Settings);
            // Set whether radar presence counts as activity and can restore the backlight to
            // present result for the surrounding operation.
            app.preferences.current.wake_on_presence = status.present();
            // Set UTC Unix timestamp in seconds to an available value for 1_700_000_000.
            app.preferences.epoch = Some(1_700_000_000);
            // Keep host-side framebuffer representing the 320-by-480 display in this local variable
            // for the following operations.
            let mut canvas = Canvas(vec![Rgb565::BLACK; 320 * 480]);
            // Require the value guaranteed by this test fixture or internal invariant; unexpected
            // absence panics.
            ui::render(&mut canvas, &app).unwrap();
            // Writes a host RGB565 canvas as a binary RGB888 PPM image.
            save(&dir, &format!("radar-{name}-{}", language.code()), canvas)?;
        }
    }
    // Visit each entry in Language ALL; the loop binding provides its value or index for this
    // iteration.
    for language in Language::ALL {
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for view in [
            weather_forecast_firmware::weather::View::Clock,
            weather_forecast_firmware::weather::View::Week,
            weather_forecast_firmware::weather::View::Hours,
        ] {
            // Run each listed scenario or target in the declared order; the loop variables select
            // the data for that case.
            for scenario in [
                "ready",
                "loading",
                "error",
                "outdated",
                "night",
                "cold",
                "long-location",
            ] {
                // Keep application state used by navigation, preferences, weather, and rendering in
                // this local variable for the following operations.
                let mut app = App {
                    // Initialize currently selected Wi-Fi navigation page from the supplied value.
                    page: Page::Connected,
                    ..App::default()
                };
                // Set selected language for interface text and location search to selected language
                // for interface text and location search.
                app.preferences.current.language = language;
                // Set whether local time uses 24-hour rather than 12-hour formatting to selected
                // language for interface text and location search equals Language German.
                app.preferences.current.clock_24h = language == Language::German;
                // Set whether displayed temperatures are converted from cached Celsius to
                // Fahrenheit to selected language for interface text and location search equals
                // Language English.
                app.preferences.current.fahrenheit = language == Language::English;
                // Set UTC Unix timestamp in seconds to an available value for 1_790_899_200 plus
                // the value selected by the following condition and its alternatives.
                app.preferences.epoch = Some(
                    1_790_899_200
                        // Check whether scenario equals the specified message, format, or data
                        // literal.
                        + if scenario == "night" {
                            // Return 23 multiplied by 3600 as the value of this block.
                            23 * 3600
                        } else {
                            // Return 12 multiplied by 3600 as the value of this block.
                            12 * 3600
                        },
                );
                // Set view to view.
                app.weather.view = view;
                // Check whether scenario differs from the specified message, format, or data
                // literal and scenario differs from the specified message, format, or data literal.
                if scenario != "loading" && scenario != "error" {
                    // Keep validated current, daily, and hourly weather data in this local variable
                    // for the following operations.
                    let mut forecast = weather_forecast_firmware::weather::parse(include_bytes!(
                        "../tests/fixtures/weather.json"
                    ))
                    .unwrap();
                    // Check whether scenario equals the specified message, format, or data literal.
                    if scenario == "cold" {
                        // Set temperature 2m to an available value for 12.5 (the numeric value used
                        // here).
                        forecast.current.temperature_2m = Some(-12.5);
                        // Set apparent temperature to an available value for 18.2 (the numeric
                        // value used here).
                        forecast.current.apparent_temperature = Some(-18.2);
                    }
                    // Check whether scenario equals the specified message, format, or data literal.
                    if scenario == "night" {
                        // Set is day to an available value for 0.
                        forecast.current.is_day = Some(0);
                        // Set weather code to an available value for 0.
                        forecast.current.weather_code = Some(0);
                    }
                    // Set validated current, daily, and hourly weather data to an available value
                    // for validated current, daily, and hourly weather data.
                    app.weather.forecast = Some(forecast);
                }
                // Set failed to scenario equals the specified message, format, or data literal or
                // scenario equals the specified message, format, or data literal.
                app.weather.failed = scenario == "error" || scenario == "outdated";
                // Check whether scenario equals the specified message, format, or data literal.
                if scenario == "long-location" {
                    // Set human-readable label for the selected object to into result for the
                    // surrounding operation.
                    app.preferences.current.location.name =
                        "Café École Straße München, Bayern, Deutschland".into();
                    // Set optional city-and-country label that keeps the weather header concise to
                    // no available value.
                    app.preferences.current.location.compact_name = None;
                }
                // Keep host-side framebuffer representing the 320-by-480 display in this local
                // variable for the following operations.
                let mut canvas = Canvas(vec![Rgb565::BLACK; 320 * 480]);
                // Require the value guaranteed by this test fixture or internal invariant;
                // unexpected absence panics.
                ui::render(&mut canvas, &app).unwrap();
                // Writes a host RGB565 canvas as a binary RGB888 PPM image.
                save(
                    &dir,
                    &format!("weather-{:?}-{scenario}-{}", view, language.code()),
                    canvas,
                )?;
            }
        }
    }
    // Friendly examples for the README; stress-test previews above remain available.
    // Run each listed scenario or target in the declared order; the loop variables select the data
    // for that case.
    for page in [Page::Networks, Page::Password] {
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App {
            // Initialize currently selected Wi-Fi navigation page from the supplied value.
            page,
            ..App::default()
        };
        // Set nearby access points available for selection to the entries collected from the
        // preceding iterator.
        app.networks = ["Home Wi-Fi", "Office", "Guest"]
            .into_iter()
            .map(|ssid| Network {
                // Initialize Wi-Fi network name, limited to 32 encoded bytes by station
                // configuration from the supplied value.
                ssid: ssid.into(),
                // Initialize supported Wi-Fi authentication mode from the supplied value.
                security: Security::Wpa2,
                // Initialize received signal strength in dBm; less-negative values represent
                // stronger signals from the supplied value.
                rssi: -45,
            })
            .collect();
        // Keep human-readable label for the selected object in this local variable for the
        // following operations.
        let name = if page == Page::Networks {
            "guide-networks"
        } else {
            "guide-password"
        };
        // Set user-facing status or error text to into result for the surrounding operation.
        app.message = if page == Page::Networks {
            "Select a secured 2.4 GHz network."
        } else {
            "Enter the Wi-Fi password."
        }
        .into();
        // Check whether currently selected Wi-Fi navigation page equals Password state.
        if page == Page::Password {
            // Set access point currently selected for candidate setup to an available value for an
            // owned copy of the selected entry from nearby access points available for selection.
            app.selected = Some(app.networks[0].clone());
            // Append the supplied text to the existing string without replacing its earlier
            // contents.
            app.password.push_str("ExampleOnly123");
        }
        // Keep host-side framebuffer representing the 320-by-480 display in this local variable for
        // the following operations.
        let mut canvas = Canvas(vec![Rgb565::BLACK; 320 * 480]);
        // Require the value guaranteed by this test fixture or internal invariant; unexpected
        // absence panics.
        ui::render(&mut canvas, &app).unwrap();
        // Writes a host RGB565 canvas as a binary RGB888 PPM image.
        save(&dir, name, canvas)?;
    }
    // Emit a diagnostic identifying the generated preview output.
    println!("UI previews written to {}", dir.display());
    // Return success after the required side effects are complete.
    Ok(())
}

/// Writes a host RGB565 canvas as a binary RGB888 PPM image.
///
/// # Arguments
///
/// * `dir` (`&std::path::Path`) - Destination directory for generated PPM images.
/// * `name` (`&str`) - Output image basename; the writer appends the .ppm extension.
/// * `canvas` (`Canvas`) - Owned portrait RGB565 image to write.
///
/// # Returns
///
/// `std::io::Result<()>` - Ok after the named image is flushed to disk; Err for file
/// creation or write failure.
///
/// # Errors
///
/// Propagates filesystem and buffered writer errors.
fn save(dir: &std::path::Path, name: &str, canvas: Canvas) -> std::io::Result<()> {
    // Keep buffered writer for the binary PPM file, reducing individual filesystem writes in this
    // local variable for the following operations.
    let mut image = BufWriter::new(fs::File::create(dir.join(format!("{name}.ppm")))?);
    // Write the complete supplied byte slice, retrying partial writes or returning the I/O failure.
    image.write_all(b"P6\n320 480\n255\n")?;
    // Visit each entry in 0; the loop binding provides its value or index for this iteration.
    for pixel in canvas.0 {
        // Keep 24-bit conversion of one RGB565 pixel for the PPM red/green/blue bytes in this local
        // variable for the following operations.
        let rgb = Rgb888::from(pixel);
        // Write the complete supplied byte slice, retrying partial writes or returning the I/O
        // failure.
        image.write_all(&[rgb.r(), rgb.g(), rgb.b()])?;
    }
    // Flush buffered PPM output so file I/O failures are reported before declaring the preview
    // complete.
    image.flush()
}

///////////////////////////////////////////////////////////////////////////////////////
//                                    UNIT TESTS                                     //
///////////////////////////////////////////////////////////////////////////////////////

/// Unit tests and supporting fixtures for preview behavior.
#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::primitives::Rectangle;
    use weather_forecast_firmware::{settings::Overlay, settings_ui};

    /// Verifies that presence-only redraws match a full render and preserve every pixel outside
    /// the indicator bounds.
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
    fn presence_updates_match_full_render_and_leave_every_other_pixel_unchanged() {
        use weather_forecast_firmware::radar::PresenceStatus;
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for overlay in [None, Some(Overlay::Settings)] {
            // Run each listed scenario or target in the declared order; the loop variables select
            // the data for that case.
            for page in [Page::Problem, Page::Connected] {
                // Keep application state used by navigation, preferences, weather, and rendering in
                // this local variable for the following operations.
                let mut app = App {
                    // Initialize currently selected Wi-Fi navigation page from the supplied value.
                    page,
                    ..App::default()
                };
                // Set overlay to overlay.
                app.preferences.overlay = overlay;
                // Keep framebuffer produced by the partial-rendering path under test in this local
                // variable for the following operations.
                let mut actual = Canvas(vec![Rgb565::BLACK; 320 * 480]);
                // Require the value guaranteed by this test fixture or internal invariant;
                // unexpected absence panics.
                ui::render(&mut actual, &app).unwrap();
                // Run each listed scenario or target in the declared order; the loop variables
                // select the data for that case.
                for status in [
                    PresenceStatus::Present,
                    PresenceStatus::Absent,
                    PresenceStatus::Unavailable,
                ] {
                    // Obtain an owned clone of prior state used to detect changes without redrawing
                    // unchanged content; reference-counted handles continue to share their
                    // underlying state.
                    let previous = actual.0.clone();
                    // Set combined presence indication from UART reports and the OUT pin to status
                    // displayed to the reader instead of silent failure.
                    app.radar = status;
                    // Require the value guaranteed by this test fixture or internal invariant;
                    // unexpected absence panics.
                    ui::render_presence(&mut actual, &app).unwrap();
                    // Keep reference framebuffer produced by rendering the complete page in this
                    // local variable for the following operations.
                    let mut expected = Canvas(vec![Rgb565::BLACK; 320 * 480]);
                    // Require the value guaranteed by this test fixture or internal invariant;
                    // unexpected absence panics.
                    ui::render(&mut expected, &app).unwrap();
                    // Verify that 0 exactly matches 0.
                    assert_eq!(actual.0, expected.0);
                    // Visit each entry in enumerate result for the surrounding operation; the loop
                    // binding provides its value or index for this iteration.
                    for (index, pixel) in actual.0.iter().enumerate() {
                        // Keep coordinate sample associated with the current touch or pixel
                        // operation in this local variable for the following operations.
                        let point = Point::new((index % 320) as i32, (index / 320) as i32);
                        // Check whether the inverse of the value falls within the stated range or
                        // belongs to the supported set.
                        if !ui::presence_bounds().contains(point) {
                            // Verify that pixel exactly matches the selected entry from prior state
                            // used to detect changes without redrawing unchanged content.
                            assert_eq!(*pixel, previous[index]);
                        }
                    }
                }
            }
        }
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let app = App {
            // Initialize currently selected Wi-Fi navigation page from the supplied value.
            page: Page::Password,
            ..App::default()
        };
        // Keep framebuffer produced by the partial-rendering path under test in this local variable
        // for the following operations.
        let mut actual = Canvas(vec![Rgb565::BLACK; 320 * 480]);
        // Require the value guaranteed by this test fixture or internal invariant; unexpected
        // absence panics.
        ui::render(&mut actual, &app).unwrap();
        // Obtain an owned clone of prior state used to detect changes without redrawing unchanged
        // content; reference-counted handles continue to share their underlying state.
        let previous = actual.0.clone();
        // Require the value guaranteed by this test fixture or internal invariant; unexpected
        // absence panics.
        ui::render_presence(&mut actual, &app).unwrap();
        // Verify that 0 exactly matches prior state used to detect changes without redrawing
        // unchanged content.
        assert_eq!(actual.0, previous);
    }

    /// Verifies that clock and forecast updates continue while the backlight is off and that
    /// waking preserves the selected page and frame.
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
    fn screen_blanking_keeps_weather_and_clock_current_without_changing_page() {
        use weather_forecast_firmware::{
            screen::{ScreenController, State},
            weather::{self, View},
            weather_ui,
        };
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for view in [View::Clock, View::Week, View::Hours] {
            // Keep application state used by navigation, preferences, weather, and rendering in
            // this local variable for the following operations.
            let mut app = App {
                // Initialize currently selected Wi-Fi navigation page from the supplied value.
                page: Page::Connected,
                ..App::default()
            };
            // Set inactivity interval in minutes to 1.
            app.preferences.current.sleep_minutes = 1;
            // Set whether radar presence counts as activity and can restore the backlight to the
            // disabled state.
            app.preferences.current.wake_on_presence = false;
            // Set UTC Unix timestamp in seconds to an available value for 1_790_899_200 plus 12
            // multiplied by 3600.
            app.preferences.epoch = Some(1_790_899_200 + 12 * 3600);
            // Set view to view.
            app.weather.view = view;
            // Execute adopt for forecast cache, selected view, and refresh scheduler.
            app.weather.adopt(&app.preferences.current.location);
            // Keep the returned components: `id` holds request generation carried by a command or
            // response, `location` holds selected coordinates, location labels, and IANA timezone
            // in this local variable for the following operations.
            let (id, location) = app.weather.request(0, true).unwrap();
            // Execute receive for forecast cache, selected view, and refresh scheduler.
            app.weather.receive(
                id,
                &location,
                Ok(weather::parse(include_bytes!("../tests/fixtures/weather.json")).unwrap()),
                0,
                app.preferences.epoch,
            );
            // Keep backlight state machine controlling inactivity and wake behavior in this local
            // variable for the following operations.
            let mut screen = ScreenController::new(&app.preferences.current, 0);
            // Keep backlight duty in 0-255; zero turns illumination off in this local variable for
            // the following operations.
            let mut brightness = 255;
            // Require the value guaranteed by this test fixture or internal invariant; unexpected
            // absence panics.
            screen
                .update(
                    &app.preferences.current,
                    app.preferences.epoch,
                    0,
                    false,
                    |v| {
                        // Set backlight duty in 0-255; zero turns illumination off to v.
                        brightness = v;
                        // Run the < , ()> operation with the supplied inputs.
                        Ok::<_, ()>(())
                    },
                )
                .unwrap();
            // Keep host-side framebuffer representing the 320-by-480 display in this local variable
            // for the following operations.
            let mut canvas = Canvas(vec![Rgb565::BLACK; 320 * 480]);
            // Require the value guaranteed by this test fixture or internal invariant; unexpected
            // absence panics.
            ui::render(&mut canvas, &app).unwrap();
            // Require the value guaranteed by this test fixture or internal invariant; unexpected
            // absence panics.
            screen
                .update(
                    &app.preferences.current,
                    app.preferences.epoch,
                    60_000,
                    false,
                    |v| {
                        // Set backlight duty in 0-255; zero turns illumination off to v.
                        brightness = v;
                        // Run the < , ()> operation with the supplied inputs.
                        Ok::<_, ()>(())
                    },
                )
                .unwrap();
            // Verify that backlight duty in 0-255; zero turns illumination off exactly matches 0.
            assert_eq!(brightness, 0);
            // Verify that state result for the surrounding operation exactly matches Off state.
            assert_eq!(screen.state(), State::Off);
            // Check whether view equals View Clock.
            if view == View::Clock {
                // Obtain an owned clone of snapshot taken before the operation so later changes can
                // be compared; reference-counted handles continue to share their underlying state.
                let before = canvas.0.clone();
                // Execute update time for editable preferences and their saved rollback baseline.
                app.preferences
                    .update_time(app.preferences.epoch.map(|t| t + 1));
                // Require the value guaranteed by this test fixture or internal invariant;
                // unexpected absence panics.
                weather_ui::render_clock(&mut canvas, &app).unwrap();
                // Verify that snapshot taken before the operation so later changes can be compared
                // differs from 0.
                assert_ne!(before, canvas.0);
            }
            // Obtain an owned clone of snapshot taken before the operation so later changes can be
            // compared; reference-counted handles continue to share their underlying state.
            let before = canvas.0.clone();
            // Keep the returned components: `id` holds request generation carried by a command or
            // response, `location` holds selected coordinates, location labels, and IANA timezone
            // in this local variable for the following operations.
            let (id, location) = app.weather.request(600_000, true).unwrap();
            // Keep replacement forecast fixture used to exercise updates while the screen is dark
            // in this local variable for the following operations.
            let mut refreshed =
                weather::parse(include_bytes!("../tests/fixtures/weather.json")).unwrap();
            // Set temperature 2m to an available value for 22.0 (the numeric value used here).
            refreshed.current.temperature_2m = Some(22.0);
            // Set high to an available value for 25.0 (the numeric value used here).
            refreshed.days.iter_mut().for_each(|d| d.high = Some(25.0));
            // Execute for each for iter mut result for the surrounding operation.
            refreshed
                .hours
                .iter_mut()
                // Set temperature to an available value for 23.0 (the numeric value used here).
                .for_each(|h| h.temperature = Some(23.0));
            // Verify receive result for the surrounding operation. A violation means the tested
            // behavior is incorrect.
            assert!(app.weather.receive(
                id,
                &location,
                Ok(refreshed),
                600_001,
                app.preferences.epoch
            ));
            // Require the value guaranteed by this test fixture or internal invariant; unexpected
            // absence panics.
            ui::render(&mut canvas, &app).unwrap();
            // Verify that snapshot taken before the operation so later changes can be compared
            // differs from 0.
            assert_ne!(before, canvas.0);
            // Verify that state result for the surrounding operation exactly matches Off state.
            assert_eq!(screen.state(), State::Off);
            // Obtain an owned clone of most recent framebuffer snapshot retained for the next
            // comparison; reference-counted handles continue to share their underlying state.
            let latest = canvas.0.clone();
            // Verify touch result for the surrounding operation. A violation means the tested
            // behavior is incorrect.
            assert!(screen.touch(
                &app.preferences.current,
                app.preferences.epoch,
                600_020,
                false
            ));
            // Require the value guaranteed by this test fixture or internal invariant; unexpected
            // absence panics.
            screen
                .update(
                    &app.preferences.current,
                    app.preferences.epoch,
                    600_020,
                    false,
                    |v| {
                        // Set backlight duty in 0-255; zero turns illumination off to v.
                        brightness = v;
                        // Run the < , ()> operation with the supplied inputs.
                        Ok::<_, ()>(())
                    },
                )
                .unwrap();
            // Verify that backlight duty in 0-255; zero turns illumination off exactly matches
            // backlight duty in 0-255.
            assert_eq!(brightness, app.preferences.current.brightness);
            // Verify that state result for the surrounding operation exactly matches On state.
            assert_eq!(screen.state(), State::On);
            // Verify that currently selected Wi-Fi navigation page exactly matches Connected state.
            assert_eq!(app.page, Page::Connected);
            // Verify that view exactly matches view.
            assert_eq!(app.weather.view, view);
            assert_eq!(canvas.0, latest); // No initialization or repaint required on wake.
        }
    }

    /// Verifies that second-by-second clock updates match complete page renders across the
    /// tested time transitions.
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
    fn weather_seconds_update_matches_complete_render() {
        use weather_forecast_firmware::{
            weather::{self, View},
            weather_ui,
        };
        // Keep reusable differential-rendering strips and prior snapshots under test in this local
        // variable for the following operations.
        let mut buffer = weather_ui::ClockBuffer::new().unwrap();
        // Visit each entry in Language ALL; the loop binding provides its value or index for this
        // iteration.
        for language in Language::ALL {
            // Run each listed scenario or target in the declared order; the loop variables select
            // the data for that case.
            for clock_24h in [false, true] {
                // Keep application state used by navigation, preferences, weather, and rendering in
                // this local variable for the following operations.
                let mut app = App {
                    // Initialize currently selected Wi-Fi navigation page from the supplied value.
                    page: Page::Connected,
                    ..App::default()
                };
                // Set selected language for interface text and location search to selected language
                // for interface text and location search.
                app.preferences.current.language = language;
                // Set whether local time uses 24-hour rather than 12-hour formatting to whether
                // local time uses 24-hour rather than 12-hour formatting.
                app.preferences.current.clock_24h = clock_24h;
                // Set view to View Clock.
                app.weather.view = View::Clock;
                // Set validated current, daily, and hourly weather data to an available value for
                // the required fixture or invariant value, panicking if unavailable.
                app.weather.forecast =
                    Some(weather::parse(include_bytes!("../tests/fixtures/weather.json")).unwrap());
                // Run each listed scenario or target in the declared order; the loop variables
                // select the data for that case.
                for initial in [
                    1_790_899_200 + 12 * 3600 + 59,
                    1_790_899_200 + 11 * 3600 + 3599,
                    1_790_899_200 + 22 * 3600 + 3599,
                    1_790_899_200 + 24 * 3600 - 1,
                ] {
                    // Set UTC Unix timestamp in seconds to an available value for initial complete
                    // framebuffer used to establish a rendering baseline.
                    app.preferences.epoch = Some(initial);
                    // Keep framebuffer produced by the partial-rendering path under test in this
                    // local variable for the following operations.
                    let mut actual = Canvas(vec![Rgb565::BLACK; 320 * 480]);
                    // Require the value guaranteed by this test fixture or internal invariant;
                    // unexpected absence panics.
                    ui::render(&mut actual, &app).unwrap();
                    // Execute remember for reusable differential-rendering strips and prior
                    // snapshots under test.
                    buffer.remember(&app);
                    // Obtain an owned clone of prior state used to detect changes without redrawing
                    // unchanged content; reference-counted handles continue to share their
                    // underlying state.
                    let previous = actual.0.clone();
                    // Set UTC Unix timestamp in seconds to an available value for initial complete
                    // framebuffer used to establish a rendering baseline plus 1.
                    app.preferences.epoch = Some(initial + 1);
                    // Require the value guaranteed by this test fixture or internal invariant;
                    // unexpected absence panics.
                    buffer.render(&mut actual, &app).unwrap();
                    // Keep reference framebuffer produced by rendering the complete page in this
                    // local variable for the following operations.
                    let mut expected = Canvas(vec![Rgb565::BLACK; 320 * 480]);
                    // Require the value guaranteed by this test fixture or internal invariant;
                    // unexpected absence panics.
                    ui::render(&mut expected, &app).unwrap();
                    // Verify that 0 exactly matches 0.
                    assert_eq!(actual.0, expected.0);
                    // Visit each entry in enumerate result for the surrounding operation; the loop
                    // binding provides its value or index for this iteration.
                    for (i, pixel) in actual.0.iter().enumerate() {
                        // Check whether the inverse of the value falls within the stated range or
                        // belongs to the supported set.
                        if !weather_ui::clock_bounds()
                            .contains(Point::new((i % 320) as i32, (i / 320) as i32))
                        {
                            // Verify that pixel exactly matches the selected entry from prior state
                            // used to detect changes without redrawing unchanged content.
                            assert_eq!(*pixel, previous[i]);
                        }
                    }
                }
            }
        }
    }

    /// Verifies exact changed-pixel clock transfers, unchanged-frame suppression, and a full
    /// regional redraw after an injected transfer failure.
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
    fn clock_updates_write_exactly_changed_pixels_and_recover_from_errors() {
        use embedded_graphics::primitives::Rectangle;
        use weather_forecast_firmware::weather_ui;
        struct Display {
            // Host-side framebuffer representing the 320-by-480 display. Stored as Canvas.
            canvas: Canvas,
            // Record of transferred rectangles or persistence attempts used to verify side effects.
            // Stored as Vec<Rectangle>.
            writes: Vec<Rectangle>,
            // Optional transfer count at which the simulated display injects an SPI failure. Stored
            // as Option<usize>.
            fail_after: Option<usize>,
        }
        impl OriginDimensions for Display {
            /// Reports the dimensions of the instrumented test display.
            ///
            /// # Arguments
            ///
            /// * `self` (`&Display`) - Receiver state used by this operation. Borrowed without changing
            ///   it.
            ///
            /// # Returns
            ///
            /// `Size` - Fixed portrait size of 320 by 480 pixels.
            fn size(&self) -> Size {
                // Construct the value from the supplied configuration or coordinates for use by the
                // surrounding operation.
                Size::new(320, 480)
            }
        }
        impl DrawTarget for Display {
            type Color = Rgb565;
            type Error = &'static str;
            /// Rejects uncomposed pixel drawing in the differential-rendering test display.
            ///
            /// # Arguments
            ///
            /// * `self` (`&mut Display`) - Receiver state used by this operation. Mutated in place.
            /// * `_` (`I`, argument 2) - Pixel iterator intentionally ignored; this test display
            ///   rejects individual drawing operations.
            ///
            /// # Type Parameters
            ///
            /// * `I` - Input iterable with the item type required by this function's iterator bound.
            ///
            /// # Returns
            ///
            /// `Result<(), Self::Error>` - No successful return; deliberately panics to enforce
            /// composed strip transfers.
            ///
            /// # Panics
            ///
            /// Always panics in the instrumented test display to reject individual pixel drawing.
            fn draw_iter<I: IntoIterator<Item = Pixel<Rgb565>>>(
                &mut self,
                _: I,
            ) -> Result<(), Self::Error> {
                // Stop this test immediately: reaching this path violates the expected fixture or
                // API behavior.
                panic!("Clock must send composed strips, not individual drawing operations")
            }
            /// Rejects visible clearing operations in the differential-rendering test display.
            ///
            /// # Arguments
            ///
            /// * `self` (`&mut Display`) - Receiver state used by this operation. Mutated in place.
            /// * `_` (`&Rectangle`, argument 2) - Rectangle intentionally ignored; this test display
            ///   rejects solid fills.
            /// * `_` (`Rgb565`, argument 3) - Fill colour intentionally ignored; this test display
            ///   rejects solid fills.
            ///
            /// # Returns
            ///
            /// `Result<(), Self::Error>` - No successful return; deliberately panics to prevent a
            /// clear-before-draw pass.
            ///
            /// # Panics
            ///
            /// Always panics in the instrumented test display to reject clearing passes.
            fn fill_solid(&mut self, _: &Rectangle, _: Rgb565) -> Result<(), Self::Error> {
                // Stop this test immediately: reaching this path violates the expected fixture or
                // API behavior.
                panic!("Clock must never clear the LCD before drawing text")
            }
            /// Records composed transfers and forwards them to the canvas, with optional simulated
            /// failure.
            ///
            /// # Arguments
            ///
            /// * `self` (`&mut Display`) - Receiver state used by this operation. Mutated in place.
            /// * `area` (`&Rectangle`) - Screen-coordinate rectangle of the composed pixel transfer, in
            ///   pixels.
            /// * `colors` (`I`) - Row-major RGB565 colours for the supplied rectangle.
            ///
            /// # Type Parameters
            ///
            /// * `I` - Input iterable with the item type required by this function's iterator bound.
            ///
            /// # Returns
            ///
            /// `Result<(), Self::Error>` - Ok after a complete transfer; Err with SPI failed at the
            /// configured failure point.
            ///
            /// # Errors
            ///
            /// Returns the injected SPI failure when fail_after matches the recorded write count.
            ///
            /// # Panics
            ///
            /// Panics if the supplied colour count does not equal the rectangle's pixel area.
            fn fill_contiguous<I: IntoIterator<Item = Rgb565>>(
                &mut self,
                area: &Rectangle,
                colors: I,
            ) -> Result<(), Self::Error> {
                // Check whether `self.fail_after == Some(self.writes.len())`.
                if self.fail_after == Some(self.writes.len()) {
                    // Leave this function now with a failure result for the surrounding operation;
                    // later statements are skipped.
                    return Err("SPI failed");
                }
                // Keep row-major colours supplied for a rectangle transfer in this local variable
                // for the following operations.
                let colors: Vec<_> = colors.into_iter().collect();
                // Verify that the length of row-major colours supplied for a rectangle transfer
                // exactly matches the numeric value converted to the required arithmetic or
                // indexing type.
                assert_eq!(colors.len(), (area.size.width * area.size.height) as usize);
                // Append the new entry to the collection, preserving the order in which values
                // arrive.
                self.writes.push(*area);
                // Require the value guaranteed by this test fixture or internal invariant;
                // unexpected absence panics.
                self.canvas.fill_contiguous(area, colors).unwrap();
                // Return success after the required side effects are complete.
                Ok(())
            }
        }
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App {
            // Initialize currently selected Wi-Fi navigation page from the supplied value.
            page: Page::Connected,
            ..App::default()
        };
        // Set IANA timezone identifier used for local clocks and forecast dates to into result for
        // the surrounding operation.
        app.preferences.current.location.timezone = "UTC".into();
        // Keep reusable differential-rendering strips and prior snapshots under test in this local
        // variable for the following operations.
        let mut buffer = weather_ui::ClockBuffer::new().unwrap();
        // Keep drawing destination receiving RGB565 pixels in this local variable for the following
        // operations.
        let mut display = Display {
            // Initialize host-side framebuffer representing the 320-by-480 display from the
            // supplied value.
            canvas: Canvas(vec![Rgb565::BLACK; 320 * 480]),
            // Initialize record of transferred rectangles or persistence attempts used to verify
            // side effects from the supplied value.
            writes: vec![],
            // Leave optional transfer count at which the simulated display injects an SPI failure
            // unavailable until a later operation supplies it.
            fail_after: None,
        };
        // First draw initializes the clock; subsequent writes must match exactly
        // the set of changed final pixels, including sync loss and rollovers.
        // Visit each entry in enumerate result for the surrounding operation; the loop binding
        // provides its value or index for this iteration.
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
            // Obtain an owned clone of snapshot taken before the operation so later changes can be
            // compared; reference-counted handles continue to share their underlying state.
            let before = display.canvas.0.clone();
            // Set UTC Unix timestamp in seconds to UTC Unix timestamp in seconds; an optional value
            // is unavailable before time synchronization.
            app.preferences.epoch = epoch;
            // Remove accumulated entries or transient state before beginning the next operation.
            display.writes.clear();
            // Require the value guaranteed by this test fixture or internal invariant; unexpected
            // absence panics.
            buffer.render(&mut display, &app).unwrap();
            // Keep reference framebuffer produced by rendering the complete page in this local
            // variable for the following operations.
            let mut expected = Canvas(vec![Rgb565::BLACK; 320 * 480]);
            // Require the value guaranteed by this test fixture or internal invariant; unexpected
            // absence panics.
            weather_ui::render_clock(&mut expected, &app).unwrap();
            // Verify that 0 exactly matches 0.
            assert_eq!(display.canvas.0, expected.0);
            // Keep per-pixel transfer counters proving changed pixels are written exactly once in
            // this local variable for the following operations.
            let mut written = vec![0u8; 320 * 480];
            // Visit each entry in record of transferred rectangles or persistence attempts used to
            // verify side effects; the loop binding provides its value or index for this iteration.
            for area in &display.writes {
                // Visit each entry in points result for the surrounding operation; the loop binding
                // provides its value or index for this iteration.
                for point in area.points() {
                    // Keep row-major framebuffer offset derived from the pixel's x and y
                    // coordinates in this local variable for the following operations.
                    let i = (point.y * 320 + point.x) as usize;
                    // Verify the value falls within the stated range or belongs to the supported
                    // set. A violation means the tested behavior is incorrect.
                    assert!(weather_ui::clock_bounds().contains(point));
                    // Update the selected entry from per-pixel transfer counters proving changed
                    // pixels are written exactly once using 1, retaining the accumulated state for
                    // subsequent steps.
                    written[i] += 1;
                }
            }
            // Check whether zero-based position of the selected entry equals 0.
            if index == 0 {
                // Verify that the length of record of transferred rectangles or persistence
                // attempts used to verify side effects exactly matches 17.
                assert_eq!(display.writes.len(), 17);
                // Verify that count result for the surrounding operation exactly matches 296
                // multiplied by 262.
                assert_eq!(written.iter().filter(|n| **n == 1).count(), 296 * 262);
            } else {
                // Visit each entry in 0..written.len(); the loop binding provides its value or
                // index for this iteration.
                for i in 0..written.len() {
                    // Verify that the selected entry from per-pixel transfer counters proving
                    // changed pixels are written exactly once exactly matches the supplied value
                    // converted into the requested representation.
                    assert_eq!(written[i], u8::from(before[i] != expected.0[i]));
                }
            }
            // Remove accumulated entries or transient state before beginning the next operation.
            display.writes.clear();
            // Require the value guaranteed by this test fixture or internal invariant; unexpected
            // absence panics.
            buffer.render(&mut display, &app).unwrap();
            // Verify record of transferred rectangles or persistence attempts used to verify side
            // effects is empty. A violation means the tested behavior is incorrect.
            assert!(display.writes.is_empty());
        }
        // A partially transmitted frame invalidates the baseline. Recovery must
        // rewrite the whole clock even if the requested content changes again.
        // Set UTC Unix timestamp in seconds to an available value for 1_790_899_200.
        app.preferences.epoch = Some(1_790_899_200);
        // Remove accumulated entries or transient state before beginning the next operation.
        display.writes.clear();
        // Set optional transfer count at which the simulated display injects an SPI failure to an
        // available value for 1.
        display.fail_after = Some(1);
        // Verify that render result for the surrounding operation exactly matches a failure result
        // for the surrounding operation.
        assert_eq!(buffer.render(&mut display, &app), Err("SPI failed"));
        // Verify that the length of record of transferred rectangles or persistence attempts used
        // to verify side effects exactly matches 1.
        assert_eq!(display.writes.len(), 1);
        // Set optional transfer count at which the simulated display injects an SPI failure to no
        // available value.
        display.fail_after = None;
        // Remove accumulated entries or transient state before beginning the next operation.
        display.writes.clear();
        // Set UTC Unix timestamp in seconds to an available value for 1_790_899_201.
        app.preferences.epoch = Some(1_790_899_201);
        // Require the value guaranteed by this test fixture or internal invariant; unexpected
        // absence panics.
        buffer.render(&mut display, &app).unwrap();
        // Verify that the length of record of transferred rectangles or persistence attempts used
        // to verify side effects exactly matches 17.
        assert_eq!(display.writes.len(), 17);
        // Keep reference framebuffer produced by rendering the complete page in this local variable
        // for the following operations.
        let mut expected = Canvas(vec![Rgb565::BLACK; 320 * 480]);
        // Require the value guaranteed by this test fixture or internal invariant; unexpected
        // absence panics.
        weather_ui::render_clock(&mut expected, &app).unwrap();
        // Verify that 0 exactly matches 0.
        assert_eq!(display.canvas.0, expected.0);
        // Formatting/language/timezone changes use the same final-pixel comparison.
        // Visit each entry in Language ALL; the loop binding provides its value or index for this
        // iteration.
        for language in Language::ALL {
            // Run each listed scenario or target in the declared order; the loop variables select
            // the data for that case.
            for clock_24h in [false, true] {
                // Obtain an owned clone of snapshot taken before the operation so later changes can
                // be compared; reference-counted handles continue to share their underlying state.
                let before = display.canvas.0.clone();
                // Set selected language for interface text and location search to selected language
                // for interface text and location search.
                app.preferences.current.language = language;
                // Set whether local time uses 24-hour rather than 12-hour formatting to whether
                // local time uses 24-hour rather than 12-hour formatting.
                app.preferences.current.clock_24h = clock_24h;
                // Set IANA timezone identifier used for local clocks and forecast dates to into
                // result for the surrounding operation.
                app.preferences.current.location.timezone = "Europe/Berlin".into();
                // Remove accumulated entries or transient state before beginning the next
                // operation.
                display.writes.clear();
                // Require the value guaranteed by this test fixture or internal invariant;
                // unexpected absence panics.
                buffer.render(&mut display, &app).unwrap();
                // Keep reference framebuffer produced by rendering the complete page in this local
                // variable for the following operations.
                let mut expected = Canvas(vec![Rgb565::BLACK; 320 * 480]);
                // Require the value guaranteed by this test fixture or internal invariant;
                // unexpected absence panics.
                weather_ui::render_clock(&mut expected, &app).unwrap();
                // Verify that 0 exactly matches 0.
                assert_eq!(display.canvas.0, expected.0);
                // Keep per-pixel transfer counters proving changed pixels are written exactly once
                // in this local variable for the following operations.
                let mut written = vec![0u8; 320 * 480];
                // Visit each entry in record of transferred rectangles or persistence attempts used
                // to verify side effects; the loop binding provides its value or index for this
                // iteration.
                for area in &display.writes {
                    // Visit each entry in points result for the surrounding operation; the loop
                    // binding provides its value or index for this iteration.
                    for point in area.points() {
                        // Update the selected entry from per-pixel transfer counters proving
                        // changed pixels are written exactly once using 1, retaining the
                        // accumulated state for subsequent steps.
                        written[(point.y * 320 + point.x) as usize] += 1;
                    }
                }
                // Visit each entry in 0..written.len(); the loop binding provides its value or
                // index for this iteration.
                for i in 0..written.len() {
                    // Verify that the selected entry from per-pixel transfer counters proving
                    // changed pixels are written exactly once exactly matches the supplied value
                    // converted into the requested representation.
                    assert_eq!(written[i], u8::from(before[i] != expected.0[i]));
                }
            }
        }
        // Full redraw establishes a fresh baseline; leaving the page discards it.
        // Execute remember for reusable differential-rendering strips and prior snapshots under
        // test.
        buffer.remember(&app);
        // Remove accumulated entries or transient state before beginning the next operation.
        display.writes.clear();
        // Require the value guaranteed by this test fixture or internal invariant; unexpected
        // absence panics.
        buffer.render(&mut display, &app).unwrap();
        // Verify record of transferred rectangles or persistence attempts used to verify side
        // effects is empty. A violation means the tested behavior is incorrect.
        assert!(display.writes.is_empty());
        // Execute invalidate for reusable differential-rendering strips and prior snapshots under
        // test.
        buffer.invalidate();
        // Require the value guaranteed by this test fixture or internal invariant; unexpected
        // absence panics.
        buffer.render(&mut display, &app).unwrap();
        // Verify that the length of record of transferred rectangles or persistence attempts used
        // to verify side effects exactly matches 17.
        assert_eq!(display.writes.len(), 17);
    }

    /// Verifies differential weather transfers across views, languages, and units, including
    /// unchanged data and recovery from an injected SPI failure.
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
    fn weather_updates_write_only_changed_pixels_and_recover() {
        use embedded_graphics::primitives::Rectangle;
        use weather_forecast_firmware::{weather, weather_ui};
        struct Display {
            // Host-side framebuffer representing the 320-by-480 display. Stored as Canvas.
            canvas: Canvas,
            // Record of transferred rectangles or persistence attempts used to verify side effects.
            // Stored as Vec<Rectangle>.
            writes: Vec<Rectangle>,
            // Optional transfer count at which the simulated display injects an SPI failure. Stored
            // as Option<usize>.
            fail_after: Option<usize>,
        }
        impl OriginDimensions for Display {
            /// Reports the dimensions of the instrumented test display.
            ///
            /// # Arguments
            ///
            /// * `self` (`&Display`) - Receiver state used by this operation. Borrowed without changing
            ///   it.
            ///
            /// # Returns
            ///
            /// `Size` - Fixed portrait size of 320 by 480 pixels.
            fn size(&self) -> Size {
                // Construct the value from the supplied configuration or coordinates for use by the
                // surrounding operation.
                Size::new(320, 480)
            }
        }
        impl DrawTarget for Display {
            type Color = Rgb565;
            type Error = &'static str;
            /// Rejects uncomposed pixel drawing in the differential-rendering test display.
            ///
            /// # Arguments
            ///
            /// * `self` (`&mut Display`) - Receiver state used by this operation. Mutated in place.
            /// * `_` (`I`, argument 2) - Pixel iterator intentionally ignored; this test display
            ///   rejects individual drawing operations.
            ///
            /// # Type Parameters
            ///
            /// * `I` - Input iterable with the item type required by this function's iterator bound.
            ///
            /// # Returns
            ///
            /// `Result<(), Self::Error>` - No successful return; deliberately panics to enforce
            /// composed strip transfers.
            ///
            /// # Panics
            ///
            /// Always panics in the instrumented test display to reject individual pixel drawing.
            fn draw_iter<I: IntoIterator<Item = Pixel<Rgb565>>>(
                &mut self,
                _: I,
            ) -> Result<(), Self::Error> {
                // Stop this test immediately: reaching this path violates the expected fixture or
                // API behavior.
                panic!("Weather must send composed strips, not individual drawing operations")
            }
            /// Rejects visible clearing operations in the differential-rendering test display.
            ///
            /// # Arguments
            ///
            /// * `self` (`&mut Display`) - Receiver state used by this operation. Mutated in place.
            /// * `_` (`&Rectangle`, argument 2) - Rectangle intentionally ignored; this test display
            ///   rejects solid fills.
            /// * `_` (`Rgb565`, argument 3) - Fill colour intentionally ignored; this test display
            ///   rejects solid fills.
            ///
            /// # Returns
            ///
            /// `Result<(), Self::Error>` - No successful return; deliberately panics to prevent a
            /// clear-before-draw pass.
            ///
            /// # Panics
            ///
            /// Always panics in the instrumented test display to reject clearing passes.
            fn fill_solid(&mut self, _: &Rectangle, _: Rgb565) -> Result<(), Self::Error> {
                // Stop this test immediately: reaching this path violates the expected fixture or
                // API behavior.
                panic!("Weather must never clear the LCD before drawing text")
            }
            /// Records composed transfers and forwards them to the canvas, with optional simulated
            /// failure.
            ///
            /// # Arguments
            ///
            /// * `self` (`&mut Display`) - Receiver state used by this operation. Mutated in place.
            /// * `area` (`&Rectangle`) - Screen-coordinate rectangle of the composed pixel transfer, in
            ///   pixels.
            /// * `colors` (`I`) - Row-major RGB565 colours for the supplied rectangle.
            ///
            /// # Type Parameters
            ///
            /// * `I` - Input iterable with the item type required by this function's iterator bound.
            ///
            /// # Returns
            ///
            /// `Result<(), Self::Error>` - Ok after a complete transfer; Err with SPI failed at the
            /// configured failure point.
            ///
            /// # Errors
            ///
            /// Returns the injected SPI failure when fail_after matches the recorded write count.
            ///
            /// # Panics
            ///
            /// Panics if the supplied colour count does not equal the rectangle's pixel area.
            fn fill_contiguous<I: IntoIterator<Item = Rgb565>>(
                &mut self,
                area: &Rectangle,
                colors: I,
            ) -> Result<(), Self::Error> {
                // Check whether `self.fail_after == Some(self.writes.len())`.
                if self.fail_after == Some(self.writes.len()) {
                    // Leave this function now with a failure result for the surrounding operation;
                    // later statements are skipped.
                    return Err("SPI failed");
                }
                // Keep row-major colours supplied for a rectangle transfer in this local variable
                // for the following operations.
                let colors: Vec<_> = colors.into_iter().collect();
                // Verify that the length of row-major colours supplied for a rectangle transfer
                // exactly matches the numeric value converted to the required arithmetic or
                // indexing type.
                assert_eq!(colors.len(), (area.size.width * area.size.height) as usize);
                // Append the new entry to the collection, preserving the order in which values
                // arrive.
                self.writes.push(*area);
                // Require the value guaranteed by this test fixture or internal invariant;
                // unexpected absence panics.
                self.canvas.fill_contiguous(area, colors).unwrap();
                // Return success after the required side effects are complete.
                Ok(())
            }
        }
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for view in [
            weather::View::Clock,
            weather::View::Week,
            weather::View::Hours,
        ] {
            // Visit each entry in Language ALL; the loop binding provides its value or index for
            // this iteration.
            for language in Language::ALL {
                // Run each listed scenario or target in the declared order; the loop variables
                // select the data for that case.
                for fahrenheit in [false, true] {
                    // Keep application state used by navigation, preferences, weather, and
                    // rendering in this local variable for the following operations.
                    let mut app = App {
                        // Initialize currently selected Wi-Fi navigation page from the supplied
                        // value.
                        page: Page::Connected,
                        ..App::default()
                    };
                    // Set view to view.
                    app.weather.view = view;
                    // Set selected language for interface text and location search to selected
                    // language for interface text and location search.
                    app.preferences.current.language = language;
                    // Set whether local time uses 24-hour rather than 12-hour formatting to the
                    // inverse of whether displayed temperatures are converted from cached Celsius
                    // to Fahrenheit.
                    app.preferences.current.clock_24h = !fahrenheit;
                    // Set whether displayed temperatures are converted from cached Celsius to
                    // Fahrenheit to whether displayed temperatures are converted from cached
                    // Celsius to Fahrenheit.
                    app.preferences.current.fahrenheit = fahrenheit;
                    // Set UTC Unix timestamp in seconds to an available value for 1_790_899_200
                    // plus 12 multiplied by 3600.
                    app.preferences.epoch = Some(1_790_899_200 + 12 * 3600);
                    // Keep initial complete framebuffer used to establish a rendering baseline in
                    // this local variable for the following operations.
                    let mut initial = Canvas(vec![Rgb565::BLACK; 320 * 480]);
                    // Require the value guaranteed by this test fixture or internal invariant;
                    // unexpected absence panics.
                    ui::render(&mut initial, &app).unwrap();
                    // Check whether view differs from weather View Clock.
                    if view != weather::View::Clock {
                        // Verify that the selected entry from 0 exactly matches
                        // weather_forecast_firmware theme WEATHER_CARD.
                        assert_eq!(
                            initial.0[200 * 320 + 13],
                            weather_forecast_firmware::theme::WEATHER_CARD
                        );
                    }
                    // Keep drawing destination receiving RGB565 pixels in this local variable for
                    // the following operations.
                    let mut display = Display {
                        // Initialize host-side framebuffer representing the 320-by-480 display from
                        // the supplied value.
                        canvas: initial,
                        // Initialize record of transferred rectangles or persistence attempts used
                        // to verify side effects from the supplied value.
                        writes: vec![],
                        // Leave optional transfer count at which the simulated display injects an
                        // SPI failure unavailable until a later operation supplies it.
                        fail_after: None,
                    };
                    // Keep reusable differential-rendering strips and prior snapshots under test in
                    // this local variable for the following operations.
                    let mut buffer = weather_ui::ClockBuffer::new().unwrap();
                    // Execute remember for reusable differential-rendering strips and prior
                    // snapshots under test.
                    buffer.remember(&app);
                    // Visit each entry in 0..15; the loop binding provides its value or index for
                    // this iteration.
                    for step in 0..15 {
                        // Choose the appropriate path for step; each arm handles one supported
                        // case.
                        match step {
                            // Set failed to the enabled state.
                            0 => app.weather.failed = true,
                            // Handle the 1 case: apply the state-specific behavior shown here.
                            1 => {
                                // Set failed to the disabled state.
                                app.weather.failed = false;
                                // Set validated current, daily, and hourly weather data to an
                                // available value for the required fixture or invariant value,
                                // panicking if unavailable.
                                app.weather.forecast = Some(
                                    weather::parse(include_bytes!(
                                        "../tests/fixtures/weather.json"
                                    ))
                                    .unwrap(),
                                );
                            }
                            // Handle the 2 case: apply the state-specific behavior shown here.
                            2 => {
                                // Keep mutable current-conditions fixture modified to trigger a
                                // weather-card update in this local variable for the following
                                // operations.
                                let current = &mut app.weather.forecast.as_mut().unwrap().current;
                                // Set temperature 2m to an available value for 12.5 (the numeric
                                // value used here).
                                current.temperature_2m = Some(-12.5);
                                // Set apparent temperature to an available value for 18.2 (the
                                // numeric value used here).
                                current.apparent_temperature = Some(-18.2);
                                // Set weather code to an available value for 61.
                                current.weather_code = Some(61);
                            }
                            // Handle the 3 case: apply the state-specific behavior shown here.
                            3 => {
                                // Keep mutable current-conditions fixture modified to trigger a
                                // weather-card update in this local variable for the following
                                // operations.
                                let current = &mut app.weather.forecast.as_mut().unwrap().current;
                                // Set temperature 2m to no available value.
                                current.temperature_2m = None;
                                // Set apparent temperature to no available value.
                                current.apparent_temperature = None;
                                // Set weather code to no available value.
                                current.weather_code = None;
                            }
                            // Set failed to the enabled state.
                            4 => app.weather.failed = true,
                            // Set failed to the disabled state.
                            5 => app.weather.failed = false,
                            // Handle the 6 case: apply the state-specific behavior shown here.
                            6 => {
                                // Set fetched epoch to the transformed value or entries produced by
                                // the closure.
                                app.weather.fetched_epoch = app.preferences.epoch.map(|t| t - 1200)
                            }
                            // Handle the 7 case: apply the state-specific behavior shown here.
                            7 => {
                                // Keep validated current, daily, and hourly weather data in this
                                // local variable for the following operations.
                                let forecast = app.weather.forecast.as_mut().unwrap();
                                // Visit each entry in days; the loop binding provides its value or
                                // index for this iteration.
                                for day in &mut forecast.days {
                                    // Set low to an available value for 25.0 (the numeric value
                                    // used here).
                                    day.low = Some(-25.0);
                                    // Set high to an available value for 30.0 (the numeric value
                                    // used here).
                                    day.high = Some(30.0);
                                    // Set code to an available value for 95.
                                    day.code = Some(95);
                                }
                                // Visit each entry in hours; the loop binding provides its value or
                                // index for this iteration.
                                for hour in &mut forecast.hours {
                                    // Set temperature to an available value for 15.0 (the numeric
                                    // value used here).
                                    hour.temperature = Some(-15.0);
                                    // Set precipitation to an available value for 100.
                                    hour.precipitation = Some(100);
                                    // Set code to an available value for 71.
                                    hour.code = Some(71);
                                }
                            }
                            // Handle the 8 case: apply the state-specific behavior shown here.
                            8 => {
                                // Keep validated current, daily, and hourly weather data in this
                                // local variable for the following operations.
                                let forecast = app.weather.forecast.as_mut().unwrap();
                                // Visit each entry in days; the loop binding provides its value or
                                // index for this iteration.
                                for day in &mut forecast.days {
                                    // Set low to no available value.
                                    day.low = None;
                                    // Set high to no available value.
                                    day.high = None;
                                    // Set code to no available value.
                                    day.code = None;
                                }
                                // Visit each entry in hours; the loop binding provides its value or
                                // index for this iteration.
                                for hour in &mut forecast.hours {
                                    // Set temperature to no available value.
                                    hour.temperature = None;
                                    // Set precipitation to no available value.
                                    hour.precipitation = None;
                                    // Set code to no available value.
                                    hour.code = None;
                                }
                            }
                            // Set UTC Unix timestamp in seconds to the transformed value or entries
                            // produced by the closure.
                            9 => app.preferences.epoch = app.preferences.epoch.map(|t| t + 3600),
                            // Set UTC Unix timestamp in seconds to the transformed value or entries
                            // produced by the closure.
                            10 => app.preferences.epoch = app.preferences.epoch.map(|t| t + 86400),
                            // Handle the 11 case: apply the state-specific behavior shown here.
                            11 => {
                                // Visit each entry in days; the loop binding provides its value or
                                // index for this iteration.
                                for day in &mut app.weather.forecast.as_mut().unwrap().days {
                                    // Set precipitation probability mean to an available value for
                                    // 100.0 (the numeric value used here).
                                    day.precipitation_probability_mean = Some(100.0);
                                }
                            }
                            // Handle the 12 case: apply the state-specific behavior shown here.
                            12 => {
                                // Visit each entry in days; the loop binding provides its value or
                                // index for this iteration.
                                for day in &mut app.weather.forecast.as_mut().unwrap().days {
                                    // Set precipitation probability mean to no available value.
                                    day.precipitation_probability_mean = None;
                                }
                            }
                            // Set UTC Unix timestamp in seconds to no available value.
                            13 => app.preferences.epoch = None,
                            // Handle remaining cases with the fallback, preserving safe behavior
                            // for unsupported or irrelevant input.
                            _ => {
                                // Set validated current, daily, and hourly weather data to no
                                // available value.
                                app.weather.forecast = None;
                                // Set failed to the disabled state.
                                app.weather.failed = false;
                                // Set fetched epoch to no available value.
                                app.weather.fetched_epoch = None;
                            }
                        }
                        // Check whether view equals weather View Clock.
                        if view == weather::View::Clock {
                            // Isolate weather updates from the independently refreshed clock.
                            // Require the value guaranteed by this test fixture or internal
                            // invariant; unexpected absence panics.
                            buffer.render(&mut display, &app).unwrap();
                        }
                        // Obtain an owned clone of snapshot taken before the operation so later
                        // changes can be compared; reference-counted handles continue to share
                        // their underlying state.
                        let before = display.canvas.0.clone();
                        // Remove accumulated entries or transient state before beginning the next
                        // operation.
                        display.writes.clear();
                        // Require the value guaranteed by this test fixture or internal invariant;
                        // unexpected absence panics.
                        buffer.render_weather(&mut display, &app).unwrap();
                        // Check whether the value falls within the stated range or belongs to the
                        // supported set.
                        if (11..=12).contains(&step) {
                            // Check whether view equals weather View Week.
                            if view == weather::View::Week {
                                // Verify the inverse of record of transferred rectangles or
                                // persistence attempts used to verify side effects is empty. A
                                // violation means the tested behavior is incorrect.
                                assert!(!display.writes.is_empty());
                                // Visit each entry in record of transferred rectangles or
                                // persistence attempts used to verify side effects; the loop
                                // binding provides its value or index for this iteration.
                                for area in &display.writes {
                                    // Verify every entry satisfies the required condition. A
                                    // violation means the tested behavior is incorrect.
                                    assert!(
                                        area.points().all(|point| (112..180).contains(&point.x)
                                            && (187..429).contains(&point.y)),
                                        "probability-only updates must stay in their column"
                                    );
                                }
                            } else {
                                // Verify record of transferred rectangles or persistence attempts
                                // used to verify side effects is empty. A violation means the
                                // tested behavior is incorrect.
                                assert!(display.writes.is_empty());
                            }
                        }
                        // Keep reference framebuffer produced by rendering the complete page in
                        // this local variable for the following operations.
                        let mut expected = Canvas(vec![Rgb565::BLACK; 320 * 480]);
                        // Require the value guaranteed by this test fixture or internal invariant;
                        // unexpected absence panics.
                        ui::render(&mut expected, &app).unwrap();
                        // Verify 0 equals 0. A violation means the tested behavior is incorrect.
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
                        // Keep per-pixel transfer counters proving changed pixels are written
                        // exactly once in this local variable for the following operations.
                        let mut written = vec![0u8; 320 * 480];
                        // Visit each entry in record of transferred rectangles or persistence
                        // attempts used to verify side effects; the loop binding provides its value
                        // or index for this iteration.
                        for area in &display.writes {
                            // Visit each entry in points result for the surrounding operation; the
                            // loop binding provides its value or index for this iteration.
                            for point in area.points() {
                                // Verify the value selected by the following condition and its
                                // alternatives. A violation means the tested behavior is incorrect.
                                assert!(if view == weather::View::Clock {
                                    weather_ui::weather_bounds()
                                        .iter()
                                        .any(|bounds| bounds.contains(point))
                                } else {
                                    weather_ui::overview_bounds().contains(point)
                                });
                                // Update the selected entry from per-pixel transfer counters
                                // proving changed pixels are written exactly once using 1,
                                // retaining the accumulated state for subsequent steps.
                                written[(point.y * 320 + point.x) as usize] += 1;
                            }
                        }
                        // Visit each entry in 0..written.len(); the loop binding provides its value
                        // or index for this iteration.
                        for i in 0..written.len() {
                            // Verify that the selected entry from per-pixel transfer counters
                            // proving changed pixels are written exactly once exactly matches the
                            // supplied value converted into the requested representation.
                            assert_eq!(written[i], u8::from(before[i] != expected.0[i]));
                        }
                        // Remove accumulated entries or transient state before beginning the next
                        // operation.
                        display.writes.clear();
                        // Require the value guaranteed by this test fixture or internal invariant;
                        // unexpected absence panics.
                        buffer.render_weather(&mut display, &app).unwrap();
                        // Verify record of transferred rectangles or persistence attempts used to
                        // verify side effects is empty. A violation means the tested behavior is
                        // incorrect.
                        assert!(display.writes.is_empty());
                    }
                    // Both regions can update in one loop, with independent baselines.
                    // Set validated current, daily, and hourly weather data to an available value
                    // for the required fixture or invariant value, panicking if unavailable.
                    app.weather.forecast = Some(
                        weather::parse(include_bytes!("../tests/fixtures/weather.json")).unwrap(),
                    );
                    // Set UTC Unix timestamp in seconds to an available value for 1_790_899_200
                    // plus `12` multiplied by `3600` plus 1.
                    app.preferences.epoch = Some(1_790_899_200 + 12 * 3600 + 1);
                    // Remove accumulated entries or transient state before beginning the next
                    // operation.
                    display.writes.clear();
                    // Set optional transfer count at which the simulated display injects an SPI
                    // failure to an available value for 1.
                    display.fail_after = Some(1);
                    // Verify that render weather result for the surrounding operation exactly
                    // matches a failure result for the surrounding operation.
                    assert_eq!(buffer.render_weather(&mut display, &app), Err("SPI failed"));
                    // Verify that the length of record of transferred rectangles or persistence
                    // attempts used to verify side effects exactly matches 1.
                    assert_eq!(display.writes.len(), 1);
                    // Set optional transfer count at which the simulated display injects an SPI
                    // failure to no available value.
                    display.fail_after = None;
                    // Remove accumulated entries or transient state before beginning the next
                    // operation.
                    display.writes.clear();
                    // Require the value guaranteed by this test fixture or internal invariant;
                    // unexpected absence panics.
                    buffer.render_weather(&mut display, &app).unwrap();
                    // Verify that the length of record of transferred rectangles or persistence
                    // attempts used to verify side effects exactly matches the value selected by
                    // the following condition and its alternatives.
                    assert_eq!(
                        display.writes.len(),
                        if view == weather::View::Clock { 9 } else { 25 }
                    );
                    // Check whether view equals weather View Clock.
                    if view == weather::View::Clock {
                        // Require the value guaranteed by this test fixture or internal invariant;
                        // unexpected absence panics.
                        buffer.render(&mut display, &app).unwrap();
                    }
                    // Keep reference framebuffer produced by rendering the complete page in this
                    // local variable for the following operations.
                    let mut expected = Canvas(vec![Rgb565::BLACK; 320 * 480]);
                    // Require the value guaranteed by this test fixture or internal invariant;
                    // unexpected absence panics.
                    ui::render(&mut expected, &app).unwrap();
                    // Verify 0 equals 0. A violation means the tested behavior is incorrect.
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
                    // Execute invalidate for reusable differential-rendering strips and prior
                    // snapshots under test.
                    buffer.invalidate();
                    // Execute remember for reusable differential-rendering strips and prior
                    // snapshots under test.
                    buffer.remember(&app);
                    // Remove accumulated entries or transient state before beginning the next
                    // operation.
                    display.writes.clear();
                    // Require the value guaranteed by this test fixture or internal invariant;
                    // unexpected absence panics.
                    buffer.render_weather(&mut display, &app).unwrap();
                    // Verify record of transferred rectangles or persistence attempts used to
                    // verify side effects is empty. A violation means the tested behavior is
                    // incorrect.
                    assert!(display.writes.is_empty());
                }
            }
        }
    }

    /// Verifies that forecast range rollovers and view changes produce the same pixels as
    /// complete renders.
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
    fn overview_rollovers_and_view_changes_match_complete_renders() {
        use weather_forecast_firmware::{weather, weather_ui};
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App {
            // Initialize currently selected Wi-Fi navigation page from the supplied value.
            page: Page::Connected,
            ..App::default()
        };
        // Set IANA timezone identifier used for local clocks and forecast dates to into result for
        // the surrounding operation.
        app.preferences.current.location.timezone = "Europe/Berlin".into();
        // Keep reusable differential-rendering strips and prior snapshots under test in this local
        // variable for the following operations.
        let mut buffer = weather_ui::ClockBuffer::new().unwrap();
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for timestamp in [
            "2026-03-29T00:59:59Z",
            "2026-10-25T00:59:59Z",
            "2026-09-30T21:59:59Z",
        ] {
            // Keep UTC Unix timestamp in seconds; an optional value is unavailable before time
            // synchronization in this local variable for the following operations.
            let epoch = chrono::DateTime::parse_from_rfc3339(timestamp)
                .unwrap()
                .timestamp();
            // Keep local calendar date used for daily forecast selection in this local variable for
            // the following operations.
            let date = chrono::DateTime::from_timestamp(epoch, 0)
                .unwrap()
                .date_naive();
            // Keep validated current, daily, and hourly weather data in this local variable for the
            // following operations.
            let mut forecast =
                weather::parse(include_bytes!("../tests/fixtures/weather.json")).unwrap();
            // Set hours to the entries collected from the preceding iterator.
            forecast.hours = (0..10)
                .map(|i| weather::Hour {
                    // Initialize timestamp or formatted clock value used by the current view from
                    // the supplied value.
                    time: epoch + 1 + i * 3600,
                    // Initialize temperature from the supplied value.
                    temperature: Some(i as f64),
                    // Initialize precipitation from the supplied value.
                    precipitation: Some(i as u8 * 10),
                    // Initialize code from the supplied value.
                    code: Some(0),
                    // Initialize is day from the supplied value.
                    is_day: Some(1),
                })
                .collect();
            // Set days to the entries collected from the preceding iterator.
            forecast.days = (0..8)
                .map(|i| weather::Day {
                    // Initialize local calendar date used for daily forecast selection from the
                    // supplied value.
                    date: date + chrono::Days::new(i),
                    // Initialize low from the supplied value.
                    low: Some(i as f64),
                    // Initialize high from the supplied value.
                    high: Some(i as f64 + 10.0),
                    // Initialize precipitation probability mean from the supplied value.
                    precipitation_probability_mean: Some(i as f64 * 10.0),
                    // Initialize code from the supplied value.
                    code: Some(0),
                })
                .collect();
            // Set validated current, daily, and hourly weather data to an available value for
            // validated current, daily, and hourly weather data.
            app.weather.forecast = Some(forecast);
            // Run each listed scenario or target in the declared order; the loop variables select
            // the data for that case.
            for clock_24h in [false, true] {
                // Set whether local time uses 24-hour rather than 12-hour formatting to whether
                // local time uses 24-hour rather than 12-hour formatting.
                app.preferences.current.clock_24h = clock_24h;
                // Run each listed scenario or target in the declared order; the loop variables
                // select the data for that case.
                for view in [
                    weather::View::Week,
                    weather::View::Hours,
                    weather::View::Clock,
                    weather::View::Hours,
                ] {
                    // Set view to view.
                    app.weather.view = view;
                    // Set UTC Unix timestamp in seconds to an available value for UTC Unix
                    // timestamp in seconds; an optional value is unavailable before time
                    // synchronization.
                    app.preferences.epoch = Some(epoch);
                    // Keep framebuffer produced by the partial-rendering path under test in this
                    // local variable for the following operations.
                    let mut actual = Canvas(vec![Rgb565::BLACK; 320 * 480]);
                    // Require the value guaranteed by this test fixture or internal invariant;
                    // unexpected absence panics.
                    ui::render(&mut actual, &app).unwrap();
                    // Execute invalidate for reusable differential-rendering strips and prior
                    // snapshots under test.
                    buffer.invalidate();
                    // Execute remember for reusable differential-rendering strips and prior
                    // snapshots under test.
                    buffer.remember(&app);
                    // Set UTC Unix timestamp in seconds to an available value for UTC Unix
                    // timestamp in seconds; an optional value is unavailable before time
                    // synchronization plus 1.
                    app.preferences.epoch = Some(epoch + 1);
                    // Require the value guaranteed by this test fixture or internal invariant;
                    // unexpected absence panics.
                    buffer.render_weather(&mut actual, &app).unwrap();
                    // Check whether view equals weather View Clock.
                    if view == weather::View::Clock {
                        // Require the value guaranteed by this test fixture or internal invariant;
                        // unexpected absence panics.
                        buffer.render(&mut actual, &app).unwrap();
                    }
                    // Keep reference framebuffer produced by rendering the complete page in this
                    // local variable for the following operations.
                    let mut expected = Canvas(vec![Rgb565::BLACK; 320 * 480]);
                    // Require the value guaranteed by this test fixture or internal invariant;
                    // unexpected absence panics.
                    ui::render(&mut expected, &app).unwrap();
                    // Verify that 0 exactly matches 0.
                    assert_eq!(actual.0, expected.0, "{timestamp} {view:?}");
                }
            }
        }
    }

    /// Verifies that analog and digital clock updates stay consistent with full renders through
    /// daylight-saving transitions.
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
    fn analog_and_digital_updates_match_across_dst_transitions() {
        use weather_forecast_firmware::weather_ui;
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App {
            // Initialize currently selected Wi-Fi navigation page from the supplied value.
            page: Page::Connected,
            ..App::default()
        };
        // Set IANA timezone identifier used for local clocks and forecast dates to into result for
        // the surrounding operation.
        app.preferences.current.location.timezone = "Europe/Berlin".into();
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for timestamp in ["2026-03-29T00:59:59Z", "2026-10-25T00:59:59Z"] {
            // Keep UTC Unix timestamp in seconds; an optional value is unavailable before time
            // synchronization in this local variable for the following operations.
            let epoch = chrono::DateTime::parse_from_rfc3339(timestamp)
                .unwrap()
                .timestamp();
            // Set UTC Unix timestamp in seconds to an available value for UTC Unix timestamp in
            // seconds; an optional value is unavailable before time synchronization.
            app.preferences.epoch = Some(epoch);
            // Keep framebuffer produced by the partial-rendering path under test in this local
            // variable for the following operations.
            let mut actual = Canvas(vec![Rgb565::BLACK; 320 * 480]);
            // Require the value guaranteed by this test fixture or internal invariant; unexpected
            // absence panics.
            ui::render(&mut actual, &app).unwrap();
            // Keep reusable differential-rendering strips and prior snapshots under test in this
            // local variable for the following operations.
            let mut buffer = weather_ui::ClockBuffer::new().unwrap();
            // Execute remember for reusable differential-rendering strips and prior snapshots under
            // test.
            buffer.remember(&app);
            // Set UTC Unix timestamp in seconds to an available value for UTC Unix timestamp in
            // seconds; an optional value is unavailable before time synchronization plus 1.
            app.preferences.epoch = Some(epoch + 1);
            // Require the value guaranteed by this test fixture or internal invariant; unexpected
            // absence panics.
            buffer.render(&mut actual, &app).unwrap();
            // Keep reference framebuffer produced by rendering the complete page in this local
            // variable for the following operations.
            let mut expected = Canvas(vec![Rgb565::BLACK; 320 * 480]);
            // Require the value guaranteed by this test fixture or internal invariant; unexpected
            // absence panics.
            ui::render(&mut expected, &app).unwrap();
            // Verify that 0 exactly matches 0.
            assert_eq!(actual.0, expected.0);
        }
    }

    /// Verifies that second updates leave unrelated digital clock cells stationary, including
    /// narrow digits and placeholder layouts.
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
    fn seconds_leave_other_clock_cells_stationary() {
        use weather_forecast_firmware::weather_ui;
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App {
            // Initialize currently selected Wi-Fi navigation page from the supplied value.
            page: Page::Connected,
            ..App::default()
        };
        // Set IANA timezone identifier used for local clocks and forecast dates to into result for
        // the surrounding operation.
        app.preferences.current.location.timezone = "UTC".into();
        // Set whether local time uses 24-hour rather than 12-hour formatting to the enabled state.
        app.preferences.current.clock_24h = true;
        // Keep fixed UTC Unix-second baseline used to verify digital clock cell stability in this
        // local variable for the following operations.
        let start = 1_790_899_200 + 12 * 3600 + 34 * 60;
        // Set UTC Unix timestamp in seconds to an available value for fixed UTC Unix-second
        // baseline used to verify digital clock cell stability.
        app.preferences.epoch = Some(start);
        // Keep complete rendered clock frame from which second-only updates begin in this local
        // variable for the following operations.
        let mut baseline = Canvas(vec![Rgb565::BLACK; 320 * 480]);
        // Require the value guaranteed by this test fixture or internal invariant; unexpected
        // absence panics.
        weather_ui::render_clock(&mut baseline, &app).unwrap();
        // Keep reusable differential-rendering strips and prior snapshots under test in this local
        // variable for the following operations.
        let mut buffer = weather_ui::ClockBuffer::new().unwrap();
        // Execute remember for reusable differential-rendering strips and prior snapshots under
        // test.
        buffer.remember(&app);
        // Keep framebuffer produced by the partial-rendering path under test in this local variable
        // for the following operations.
        let mut actual = Canvas(baseline.0.clone());
        // Visit each entry in 1..60; the loop binding provides its value or index for this
        // iteration.
        for second in 1..60 {
            // Set UTC Unix timestamp in seconds to an available value for fixed UTC Unix-second
            // baseline used to verify digital clock cell stability plus second within the minute.
            app.preferences.epoch = Some(start + second);
            // Require the value guaranteed by this test fixture or internal invariant; unexpected
            // absence panics.
            buffer.render(&mut actual, &app).unwrap();
            // Keep reference framebuffer produced by rendering the complete page in this local
            // variable for the following operations.
            let mut expected = Canvas(vec![Rgb565::BLACK; 320 * 480]);
            // Require the value guaranteed by this test fixture or internal invariant; unexpected
            // absence panics.
            weather_ui::render_clock(&mut expected, &app).unwrap();
            // Verify that 0 exactly matches 0.
            assert_eq!(actual.0, expected.0);
            // Visit each entry in 238..316; the loop binding provides its value or index for this
            // iteration.
            for y in 238..316 {
                // Visit each entry in 12..308; the loop binding provides its value or index for
                // this iteration.
                for x in 12..308 {
                    // Check whether the inverse of the value falls within the stated range or
                    // belongs to the supported set.
                    if !(198..254).contains(&x) {
                        // Keep row-major framebuffer offset derived from the pixel's x and y
                        // coordinates in this local variable for the following operations.
                        let i = y * 320 + x;
                        // Verify that the selected entry from 0 exactly matches the selected entry
                        // from 0.
                        assert_eq!(
                            actual.0[i], baseline.0[i],
                            "Other cells moved at second {second}"
                        );
                    }
                }
            }
        }
        // The layout envelope is symmetric even for narrow ones and placeholders.
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for epoch in [Some(1_790_899_200 + 11 * 3600 + 11 * 60 + 11), None] {
            // Set UTC Unix timestamp in seconds to UTC Unix timestamp in seconds; an optional value
            // is unavailable before time synchronization.
            app.preferences.epoch = epoch;
            // Keep host-side framebuffer representing the 320-by-480 display in this local variable
            // for the following operations.
            let mut canvas = Canvas(vec![Rgb565::BLACK; 320 * 480]);
            // Require the value guaranteed by this test fixture or internal invariant; unexpected
            // absence panics.
            weather_ui::render_clock(&mut canvas, &app).unwrap();
            // Visit each entry in 238..287; the loop binding provides its value or index for this
            // iteration.
            for y in 238..287 {
                // Visit each entry in 12..308; the loop binding provides its value or index for
                // this iteration.
                for x in 12..308 {
                    // Check whether the inverse of the value falls within the stated range or
                    // belongs to the supported set.
                    if !(66..254).contains(&x) {
                        // Verify that the selected entry from 0 exactly matches background at
                        // result for the surrounding operation.
                        assert_eq!(
                            canvas.0[y * 320 + x],
                            weather_forecast_firmware::theme::background_at(y as i32)
                        );
                    }
                }
            }
        }
    }

    /// Verifies that a clipped settings-clock update matches a full render without changing
    /// surrounding controls.
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
    fn clock_update_matches_full_render_without_touching_controls() {
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App::default();
        // Set overlay to an available value for Overlay Settings.
        app.preferences.overlay = Some(Overlay::Settings);
        // Set UTC Unix timestamp in seconds to an available value for 1_700_000_000.
        app.preferences.epoch = Some(1_700_000_000);
        // Keep framebuffer produced by the partial-rendering path under test in this local variable
        // for the following operations.
        let mut actual = Canvas(vec![Rgb565::BLACK; 320 * 480]);
        // Require the value guaranteed by this test fixture or internal invariant; unexpected
        // absence panics.
        ui::render(&mut actual, &app).unwrap();
        // Obtain an owned clone of prior state used to detect changes without redrawing unchanged
        // content; reference-counted handles continue to share their underlying state.
        let previous = actual.0.clone();
        // Set UTC Unix timestamp in seconds to an available value for 1_700_000_060.
        app.preferences.epoch = Some(1_700_000_060);
        // Keep screen-coordinate rectangle limiting the drawing operation in this local variable
        // for the following operations.
        let bounds = Rectangle::new(Point::new(16, 40), Size::new(288, 32));
        // Require the value guaranteed by this test fixture or internal invariant; unexpected
        // absence panics.
        settings_ui::render(&mut actual.clipped(&bounds), &app).unwrap();
        // Keep reference framebuffer produced by rendering the complete page in this local variable
        // for the following operations.
        let mut expected = Canvas(vec![Rgb565::BLACK; 320 * 480]);
        // Require the value guaranteed by this test fixture or internal invariant; unexpected
        // absence panics.
        ui::render(&mut expected, &app).unwrap();
        // Verify that 0 exactly matches 0.
        assert_eq!(actual.0, expected.0);
        // Visit each entry in enumerate result for the surrounding operation; the loop binding
        // provides its value or index for this iteration.
        for (index, pixel) in actual.0.iter().enumerate() {
            // Keep coordinate sample associated with the current touch or pixel operation in this
            // local variable for the following operations.
            let point = Point::new((index % 320) as i32, (index / 320) as i32);
            // Check whether the inverse of the value falls within the stated range or belongs to
            // the supported set.
            if !bounds.contains(point) {
                // Verify that pixel exactly matches the selected entry from prior state used to
                // detect changes without redrawing unchanged content.
                assert_eq!(*pixel, previous[index]);
            }
        }
    }
}
