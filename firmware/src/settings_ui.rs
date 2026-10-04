// ============================================================================= //
// File          : settings_ui.rs                                                //
// License       : GPL-3.0-only                                                  //
// Created       : 2026-10-04                                                    //
// Last modified : 2026-10-04                                                    //
// Author        : Daniel Kharlamov                                              //
// Last editor   : Daniel Kharlamov <daniel.kharlamov@googlemail.com>            //
// ============================================================================= //
//                                                                               //
// Summary:                                                                      //
// --------                                                                      //
// Settings overlay controls, rendering, and user actions.                       //
//                                                                               //
// Module:                                                                       //
// -------                                                                       //
// Builds settings controls and renders localized overlays for language,         //
// location, brightness, clock, temperature units, screen timeout, presence      //
// wakeup, Wi-Fi reset, and touch calibration. Updates preference state and      //
// emits save, search, reset, or calibration effects for the application to      //
// execute.                                                                      //
// ============================================================================= //

//! Settings controls and effects. Wi-Fi state remains underneath these overlays.
use crate::{
    i18n::{tr, Language},
    location,
    model::{App, Command},
    settings::{Overlay, Settings},
    theme::{self, Backdrop, BODY, SMALL, TITLE},
    touch::Wizard,
    ui::{self, Action, Control},
};
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, PrimitiveStyle, Rectangle},
};

pub enum Effect {
    // Represent wifi as a distinct selectable state or action; the matching handler determines its
    // effect.
    Wifi(Command),
    // Represent persist as a distinct selectable state or action; the matching handler determines
    // its effect.
    Persist(Settings),
    // Represent search as a distinct selectable state or action; the matching handler determines
    // its effect.
    Search(location::Request),
}
/// Creates a settings control using pixel bounds and an owned label.
///
/// # Arguments
///
/// * `x` (`i32`) - Horizontal screen coordinate in pixels.
/// * `y` (`i32`) - Vertical screen coordinate in pixels.
/// * `width` (`u32`) - Available drawing or text width in pixels.
/// * `height` (`u32`) - Control height in pixels.
/// * `label` (`impl Into<String>`) - Text converted into the control's owned label.
/// * `action` (`Action`) - Action selected by the user or hit tester.
/// * `enabled` (`bool`) - Whether the control accepts user input.
///
/// # Returns
///
/// `Control` - Control containing the supplied action and enabled state.
fn button(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    label: impl Into<String>,
    action: Action,
    enabled: bool,
) -> Control {
    Control {
        // Initialize screen-coordinate rectangle limiting the drawing operation from the supplied
        // value.
        bounds: Rectangle::new(Point::new(x, y), Size::new(width, height)),
        // Initialize text identifying the control or forecast entry from the supplied value.
        label: label.into(),
        // Initialize action from the supplied value.
        action,
        // Initialize enabled from the supplied value.
        enabled,
    }
}
/// Formats an inactivity timeout in the selected interface language.
///
/// # Arguments
///
/// * `language` (`Language`) - Supported language used for translated labels or API
///   requests.
/// * `minutes` (`u32`) - Inactivity interval in minutes; zero means Never.
///
/// # Returns
///
/// `String` - Localized Never for zero, or a minute count with its localized unit.
pub fn timeout_label(language: Language, minutes: u32) -> String {
    // Check whether minutes equals 0.
    if minutes == 0 {
        // Produce into result for the surrounding operation.
        tr(language, "Never").into()
    } else {
        format!("{minutes} {}", tr(language, "min"))
    }
}
/// Builds controls for the current settings overlay and preference state.
///
/// # Arguments
///
/// * `app` (`&App`) - Application model supplying the active page, preferences, weather,
///   and presence state.
///
/// # Returns
///
/// `Vec<Control>` - Vector of buttons, keyboard keys, and settings actions; empty when the
/// overlay has no controls.
pub fn controls(app: &App) -> Vec<Control> {
    // Keep short-lived borrow of preference state being rendered or changed in this local variable
    // for the following operations.
    let p = &app.preferences;
    // Keep controls collected for the active settings overlay in this local variable for the
    // following operations.
    let mut out = Vec::new();
    // Choose the appropriate path for overlay; each arm handles one supported case.
    match p.overlay {
        // Handle the Some(Overlay Settings) case: apply the state-specific behavior shown here.
        Some(Overlay::Settings) => {
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(button(
                136,
                72,
                172,
                40,
                "",
                Action::Brightness,
                !p.read_only,
            ));
            // Run each listed scenario or target in the declared order; the loop variables select
            // the data for that case.
            for (y, label, action) in [
                (116, "Night mode", Action::Night),
                (158, "Fahrenheit", Action::Units),
                (200, "24-hour time", Action::Clock),
                (242, "Wake on presence", Action::WakeOnPresence),
            ] {
                // Append the new entry to the collection, preserving the order in which values
                // arrive.
                out.push(button(252, y, 56, 32, label, action, !p.read_only));
            }
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(button(
                142,
                286,
                166,
                32,
                timeout_label(p.current.language, p.current.sleep_minutes),
                Action::SleepTimeout,
                !p.read_only,
            ));
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(button(
                142,
                330,
                166,
                36,
                p.current.language.name(),
                Action::Languages,
                !p.read_only,
            ));
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(button(
                12,
                370,
                140,
                32,
                "Change location",
                Action::Location,
                !p.read_only,
            ));
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(button(
                168,
                370,
                140,
                32,
                "Reset Wi-Fi",
                Action::ResetPrompt,
                true,
            ));
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(button(
                12,
                408,
                140,
                30,
                "Change Wi-Fi",
                Action::Setup,
                true,
            ));
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(button(
                168,
                408,
                140,
                30,
                "Screen calibration",
                Action::Calibration,
                true,
            ));
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(button(
                12,
                446,
                296,
                32,
                "Close",
                Action::CloseSettings,
                true,
            ));
        }
        // Handle the Some(Overlay SleepTimeout) case: apply the state-specific behavior shown here.
        Some(Overlay::SleepTimeout) => {
            // Visit each entry in enumerate result for the surrounding operation; the loop binding
            // provides its value or index for this iteration.
            for (index, minutes) in crate::screen::TIMEOUTS.into_iter().enumerate() {
                // Append the new entry to the collection, preserving the order in which values
                // arrive.
                out.push(button(
                    12,
                    76 + index as i32 * 56,
                    296,
                    44,
                    timeout_label(p.current.language, minutes),
                    Action::SelectSleepTimeout(minutes),
                    !p.read_only,
                ));
            }
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(button(12, 438, 296, 40, "Back", Action::SettingsBack, true));
        }
        // Handle the Some(Overlay Languages) case: apply the state-specific behavior shown here.
        Some(Overlay::Languages) => {
            // Visit each entry in enumerate result for the surrounding operation; the loop binding
            // provides its value or index for this iteration.
            for (i, language) in Language::ALL.into_iter().enumerate() {
                // Append the new entry to the collection, preserving the order in which values
                // arrive.
                out.push(button(
                    12,
                    70 + i as i32 * 46,
                    296,
                    40,
                    language.name(),
                    Action::Language(language),
                    true,
                ));
            }
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(button(12, 438, 296, 40, "Back", Action::SettingsBack, true));
        }
        // Handle the Some(Overlay LocationInput) case: apply the state-specific behavior shown
        // here.
        Some(Overlay::LocationInput) => {
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(button(
                12,
                134,
                140,
                32,
                "Accents",
                Action::CityAccents,
                true,
            ));
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(button(
                168,
                134,
                140,
                32,
                // Check whether city cyrillic.
                if p.city_cyrillic { "ABC" } else { "АБВ" },
                Action::CityAlphabet,
                true,
            ));
            // Keep ordered keyboard or forecast rows used by the current layout in this local
            // variable for the following operations.
            let rows = match p.keyboard {
                // Handle the 4 if p.current.language == Language Ukrainian case: apply the
                // state-specific behavior shown here.
                4 if p.current.language == Language::Ukrainian => {
                    // Return the ordered sample/byte array as the value of this block.
                    ["йцукенгшщзхї", "фівапролджє", "ячсмитьбюґ'", "0123456789"]
                }
                // Handle the 5 if p.current.language == Language Ukrainian case: apply the
                // state-specific behavior shown here.
                5 if p.current.language == Language::Ukrainian => {
                    // Return the ordered sample/byte array as the value of this block.
                    ["ЙЦУКЕНГШЩЗХЇ", "ФІВАПРОЛДЖЄ", "ЯЧСМИТЬБЮҐ'", "0123456789"]
                }
                // Handle the 4 case: apply the state-specific behavior shown here.
                4 => ["йцукенгшщзхъ", "фывапролджэ", "ячсмитьбюё", "0123456789"],
                // Handle the 5 case: apply the state-specific behavior shown here.
                5 => ["ЙЦУКЕНГШЩЗХЪ", "ФЫВАПРОЛДЖЭ", "ЯЧСМИТЬБЮЁ", "0123456789"],
                // Handle the 1 case: apply the state-specific behavior shown here.
                1 => ["QWERTYUIOP", "ASDFGHJKL", "ZXCVBNM", "0123456789"],
                // Handle the 2 case: apply the state-specific behavior shown here.
                2 => ["!@#$%^&*()", "-_=+[]{}", ";:'\"\\|,.", "<>/?`~"],
                // Handle the 3 case: apply the state-specific behavior shown here.
                3 => ["äöüÄÖÜßåÅ", "éèêëàâçñ", "ìíòóùúœ", "ğĞşŞıİ"],
                // Handle remaining cases with the fallback, preserving safe behavior for
                // unsupported or irrelevant input.
                _ => ["qwertyuiop", "asdfghjkl", "zxcvbnm", "0123456789"],
            };
            // Visit each entry in enumerate result for the surrounding operation; the loop binding
            // provides its value or index for this iteration.
            for (row, chars) in rows.into_iter().enumerate() {
                // Keep number of elements involved in the current operation in this local variable
                // for the following operations.
                let count = chars.chars().count() as i32;
                // Keep keyboard cell position used to place the next character control in this
                // local variable for the following operations.
                let cell = 32.min(320 / count);
                // Keep initial pixel position or timestamp used as the baseline for this operation
                // in this local variable for the following operations.
                let start = (320 - count * cell) / 2;
                // Visit each entry in enumerate result for the surrounding operation; the loop
                // binding provides its value or index for this iteration.
                for (col, ch) in chars.chars().enumerate() {
                    // Append the new entry to the collection, preserving the order in which values
                    // arrive.
                    out.push(button(
                        start + col as i32 * cell + 1,
                        234 + row as i32 * 40,
                        (cell - 2) as u32,
                        38,
                        ch.to_string(),
                        Action::CityCharacter(ch),
                        true,
                    ));
                }
            }
            // Run each listed scenario or target in the declared order; the loop variables select
            // the data for that case.
            for (i, label, action) in [
                (0, "Shift", Action::CityShift),
                (1, "#+=", Action::CitySymbols),
                (2, "Space", Action::CitySpace),
                (3, "Del", Action::CityDelete),
            ] {
                // Append the new entry to the collection, preserving the order in which values
                // arrive.
                out.push(button(i * 80 + 1, 396, 78, 38, label, action, true));
            }
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(button(
                12,
                438,
                140,
                40,
                "Cancel",
                Action::SettingsBack,
                true,
            ));
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(button(
                168,
                438,
                140,
                40,
                "Search",
                Action::SearchCity,
                p.query.trim().chars().count() >= 2,
            ));
        }
        // Handle the Some(Overlay LocationResults) case: apply the state-specific behavior shown
        // here.
        Some(Overlay::LocationResults) => {
            // Visit each entry in enumerate result for the surrounding operation; the loop binding
            // provides its value or index for this iteration.
            for (row, location) in p.results.iter().skip(p.result_offset).take(5).enumerate() {
                // Append the new entry to the collection, preserving the order in which values
                // arrive.
                out.push(button(
                    12,
                    112 + row as i32 * 54,
                    296,
                    48,
                    &location.name,
                    Action::SelectLocation(p.result_offset + row),
                    !p.searching,
                ));
            }
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(button(
                12,
                388,
                140,
                40,
                "Previous",
                Action::LocationsPrevious,
                p.result_offset > 0 && !p.searching,
            ));
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(button(
                168,
                388,
                140,
                40,
                "Next",
                Action::LocationsNext,
                p.result_offset + 5 < p.results.len() && !p.searching,
            ));
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(button(12, 438, 140, 40, "Back", Action::LocationBack, true));
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(button(
                168,
                438,
                140,
                40,
                "Save",
                Action::SaveLocation,
                p.result_selected.is_some() && !p.searching,
            ));
        }
        // Handle the Some(Overlay ResetWifi) case: apply the state-specific behavior shown here.
        Some(Overlay::ResetWifi) => {
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(button(
                12,
                350,
                296,
                40,
                "Reset",
                Action::ResetConfirm,
                !p.reset_pending,
            ));
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(button(
                12,
                438,
                296,
                40,
                "Cancel",
                Action::SettingsBack,
                !p.reset_pending,
            ));
        }
        // Handle the Some(Overlay CalibrationIntro | Overlay CalibrationError) case: apply the
        // state-specific behavior shown here.
        Some(Overlay::CalibrationIntro | Overlay::CalibrationError) => {
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(button(
                12,
                350,
                296,
                40,
                // Check whether overlay equals an available value for Overlay CalibrationIntro.
                if p.overlay == Some(Overlay::CalibrationIntro) {
                    "Start"
                } else {
                    "Retry"
                },
                Action::StartCalibration,
                true,
            ));
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(button(12, 438, 296, 40, "Back", Action::SettingsBack, true));
        }
        // Handle the Some(Overlay CalibrationRunning) | None case: apply the state-specific
        // behavior shown here.
        Some(Overlay::CalibrationRunning) | None => {}
    }
    // Return controls collected for the active settings overlay as the value of this block.
    out
}
/// Requests persistence of current preferences unless storage is read-only.
///
/// # Arguments
///
/// * `app` (`&mut App`) - Application model supplying the active page, preferences,
///   weather, and presence state.
///
/// # Returns
///
/// `Option<Effect>` - Some persistence effect carrying a cloned profile; None with rollback
/// and error status when read-only.
fn persist(app: &mut App) -> Option<Effect> {
    // Preference loading failed earlier; roll back the edit instead of overwriting unreadable
    // storage.
    if app.preferences.read_only {
        // Execute saved for editable preferences and their saved rollback baseline.
        app.preferences.saved(false);
        // Leave this function now with no available value; later statements are skipped.
        return None;
    }
    // Return an available Persist result for the surrounding operation.
    Some(Effect::Persist(app.preferences.current.clone()))
}
/// Applies a settings action and produces any required external effect.
///
/// # Arguments
///
/// * `app` (`&mut App`) - Application model supplying the active page, preferences,
///   weather, and presence state.
/// * `action` (`Action`) - Action selected by the user or hit tester.
/// * `now` (`u64`) - Monotonic timestamp in milliseconds, using the same origin as stored
///   deadlines.
///
/// # Returns
///
/// `Option<Effect>` - Some persistence, search, or Wi-Fi effect; None for local updates,
/// calibration setup, invalid actions, or rejected input.
pub fn activate(app: &mut App, action: Action, now: u64) -> Option<Effect> {
    // Choose the appropriate path for action; each arm handles one supported case.
    match action {
        // Handle the Action OpenSettings case: apply the state-specific behavior shown here.
        Action::OpenSettings => {
            // Execute leave for editable preferences and their saved rollback baseline.
            app.preferences.leave(Some(Overlay::Settings));
        }
        // Handle the Action CloseSettings case: Execute leave for editable preferences and their
        // saved rollback baseline.
        Action::CloseSettings => app.preferences.leave(None),
        // Handle the Action SettingsBack case: Execute leave for editable preferences and their
        // saved rollback baseline.
        Action::SettingsBack => app.preferences.leave(Some(Overlay::Settings)),
        // Handle the Action Night case: apply the state-specific behavior shown here.
        Action::Night => {
            // Set whether the local 22:00-06:00 backlight schedule is enabled to the inverse of
            // whether the local 22:00-06:00 backlight schedule is enabled.
            app.preferences.current.night_mode = !app.preferences.current.night_mode;
            // Leave this function now with requests persistence of current preferences unless
            // storage is read-only; later statements are skipped.
            return persist(app);
        }
        // Handle the Action WakeOnPresence case: apply the state-specific behavior shown here.
        Action::WakeOnPresence => {
            // Set whether radar presence counts as activity and can restore the backlight to the
            // inverse of whether radar presence counts as activity and can restore the backlight.
            app.preferences.current.wake_on_presence = !app.preferences.current.wake_on_presence;
            // Leave this function now with requests persistence of current preferences unless
            // storage is read-only; later statements are skipped.
            return persist(app);
        }
        // Handle the Action Units case: apply the state-specific behavior shown here.
        Action::Units => {
            // Set whether displayed temperatures are converted from cached Celsius to Fahrenheit to
            // the inverse of whether displayed temperatures are converted from cached Celsius to
            // Fahrenheit.
            app.preferences.current.fahrenheit = !app.preferences.current.fahrenheit;
            // Leave this function now with requests persistence of current preferences unless
            // storage is read-only; later statements are skipped.
            return persist(app);
        }
        // Handle the Action Clock case: apply the state-specific behavior shown here.
        Action::Clock => {
            // Set whether local time uses 24-hour rather than 12-hour formatting to the inverse of
            // whether local time uses 24-hour rather than 12-hour formatting.
            app.preferences.current.clock_24h = !app.preferences.current.clock_24h;
            // Leave this function now with requests persistence of current preferences unless
            // storage is read-only; later statements are skipped.
            return persist(app);
        }
        // Handle the Action SleepTimeout case: Execute leave for editable preferences and their
        // saved rollback baseline.
        Action::SleepTimeout => app.preferences.leave(Some(Overlay::SleepTimeout)),
        // Handle the Action SelectSleepTimeout(minutes) case: apply the state-specific behavior
        // shown here.
        Action::SelectSleepTimeout(minutes) => {
            // Check whether the value falls within the stated range or belongs to the supported
            // set.
            if crate::screen::TIMEOUTS.contains(&minutes) {
                // Set inactivity interval in minutes to minutes.
                app.preferences.current.sleep_minutes = minutes;
                // Set after save to an available value for Overlay Settings.
                app.preferences.after_save = Some(Overlay::Settings);
                // Leave this function now with requests persistence of current preferences unless
                // storage is read-only; later statements are skipped.
                return persist(app);
            }
        }
        // Handle the Action Languages case: Execute leave for editable preferences and their saved
        // rollback baseline.
        Action::Languages => app.preferences.leave(Some(Overlay::Languages)),
        // Handle the Action Language(language) case: apply the state-specific behavior shown here.
        Action::Language(language) => {
            // Set selected language for interface text and location search to selected language for
            // interface text and location search.
            app.preferences.current.language = language;
            // Set after save to an available value for Overlay Settings.
            app.preferences.after_save = Some(Overlay::Settings);
            // Leave this function now with requests persistence of current preferences unless
            // storage is read-only; later statements are skipped.
            return persist(app);
        }
        // Handle the Action Location case: apply the state-specific behavior shown here.
        Action::Location => {
            // Execute leave for editable preferences and their saved rollback baseline.
            app.preferences.leave(Some(Overlay::LocationInput));
            // Remove accumulated entries or transient state before beginning the next operation.
            app.preferences.query.clear();
            // Set city cyrillic to the formatted text or fixture created by matches.
            app.preferences.city_cyrillic = matches!(
                app.preferences.current.language,
                Language::Russian | Language::Ukrainian
            );
            // Set selected on-screen keyboard layout to the value selected by the following
            // condition and its alternatives.
            app.preferences.keyboard = if app.preferences.city_cyrillic { 4 } else { 0 };
        }
        // Handle the Action LocationBack case: Execute leave for editable preferences and their
        // saved rollback baseline.
        Action::LocationBack => app.preferences.leave(Some(Overlay::LocationInput)),
        // Handle the Action CityCharacter(ch) case: apply the state-specific behavior shown here.
        Action::CityCharacter(ch) => {
            // Check whether count result for the surrounding operation is less than 64 and the
            // inverse of is control result for the surrounding operation.
            if app.preferences.query.chars().count() < 64 && !ch.is_control() {
                // Append the new entry to the collection, preserving the order in which values
                // arrive.
                app.preferences.query.push(ch);
            }
        }
        // Handle the Action CitySpace case: apply the state-specific behavior shown here.
        Action::CitySpace => {
            // Check whether count result for the surrounding operation is less than 64.
            if app.preferences.query.chars().count() < 64 {
                // Append the new entry to the collection, preserving the order in which values
                // arrive.
                app.preferences.query.push(' ');
            }
        }
        // Handle the Action CityDelete case: apply the state-specific behavior shown here.
        Action::CityDelete => {
            // Remove the final character so the delete action operates on a complete Unicode
            // character.
            app.preferences.query.pop();
        }
        // Handle the Action CityShift case: apply the state-specific behavior shown here.
        Action::CityShift => {
            // Keep short-lived borrow of preference state being rendered or changed in this local
            // variable for the following operations.
            let p = &mut app.preferences;
            // Keep keyboard mode for lowercase letters in the selected alphabet in this local
            // variable for the following operations.
            let lower = if p.city_cyrillic { 4 } else { 0 };
            // Keep keyboard mode for uppercase letters in the selected alphabet in this local
            // variable for the following operations.
            let upper = if p.city_cyrillic { 5 } else { 1 };
            // Set selected on-screen keyboard layout to the value selected by the following
            // condition and its alternatives.
            p.keyboard = if p.keyboard == upper { lower } else { upper };
        }
        // Handle the Action CitySymbols case: apply the state-specific behavior shown here.
        Action::CitySymbols => {
            // Keep short-lived borrow of preference state being rendered or changed in this local
            // variable for the following operations.
            let p = &mut app.preferences;
            // Set selected on-screen keyboard layout to the value selected by the following
            // condition and its alternatives.
            p.keyboard = if p.keyboard == 2 {
                // Check whether city cyrillic.
                if p.city_cyrillic {
                    4
                } else {
                    0
                }
            } else {
                2
            };
        }
        // Handle the Action CityAccents case: apply the state-specific behavior shown here.
        Action::CityAccents => {
            // Keep short-lived borrow of preference state being rendered or changed in this local
            // variable for the following operations.
            let p = &mut app.preferences;
            // Set selected on-screen keyboard layout to the value selected by the following
            // condition and its alternatives.
            p.keyboard = if p.keyboard == 3 {
                // Check whether city cyrillic.
                if p.city_cyrillic {
                    4
                } else {
                    0
                }
            } else {
                3
            };
        }
        // Handle the Action CityAlphabet case: apply the state-specific behavior shown here.
        Action::CityAlphabet => {
            // Keep short-lived borrow of preference state being rendered or changed in this local
            // variable for the following operations.
            let p = &mut app.preferences;
            // Set city cyrillic to the inverse of city cyrillic.
            p.city_cyrillic = !p.city_cyrillic;
            // Set selected on-screen keyboard layout to the value selected by the following
            // condition and its alternatives.
            p.keyboard = if p.city_cyrillic { 4 } else { 0 };
        }
        // Handle the Action SearchCity case: apply the state-specific behavior shown here.
        Action::SearchCity => {
            // Require at least two visible Unicode characters before starting a city search.
            if app.preferences.query.trim().chars().count() < 2 {
                // Set status displayed to the reader instead of silent failure to the specified
                // message, format, or data literal.
                app.preferences.status = "Use at least two characters.";
                // Leave this function now with no available value; later statements are skipped.
                return None;
            }
            // Keep short-lived borrow of preference state being rendered or changed in this local
            // variable for the following operations.
            let p = &mut app.preferences;
            // Execute leave for short-lived borrow of preference state being rendered or changed.
            p.leave(Some(Overlay::LocationResults));
            // Remove accumulated entries or transient state before beginning the next operation.
            p.results.clear();
            // Set result selected to no available value.
            p.result_selected = None;
            // Set result offset to 0.
            p.result_offset = 0;
            // Set searching to the enabled state.
            p.searching = true;
            // Set search deadline to monotonic milliseconds used to establish search and
            // calibration deadlines plus 10_000.
            p.search_deadline = now + 10_000;
            // Set status displayed to the reader instead of silent failure to the specified
            // message, format, or data literal.
            p.status = "Searching...";
            // Leave this function now with an available value for Search result for the surrounding
            // operation; later statements are skipped.
            return Some(Effect::Search(location::Request::Search {
                // Initialize request generation carried by a command or response from the supplied
                // value.
                id: p.request_id,
                // Initialize city search text or encoded API request being examined from the
                // supplied value.
                query: p.query.clone(),
                // Initialize selected language for interface text and location search from the
                // supplied value.
                language: p.current.language,
            }));
        }
        // Handle the Action SelectLocation(index) case: apply the state-specific behavior shown
        // here.
        Action::SelectLocation(index) => {
            // Accept only a result index still present in the current search response.
            if index < app.preferences.results.len() {
                // Set result selected to an available value for zero-based position of the selected
                // entry.
                app.preferences.result_selected = Some(index);
            }
        }
        // Handle the Action LocationsPrevious case: apply the state-specific behavior shown here.
        Action::LocationsPrevious => {
            // Set result offset to elapsed difference clamped at zero instead of unsigned
            // underflow.
            app.preferences.result_offset = app.preferences.result_offset.saturating_sub(5);
        }
        // Handle the Action LocationsNext case: apply the state-specific behavior shown here.
        Action::LocationsNext => {
            // Another page of up to five results exists; do not scroll beyond the last candidate.
            if app.preferences.result_offset + 5 < app.preferences.results.len() {
                // Update result offset using 5, retaining the accumulated state for subsequent
                // steps.
                app.preferences.result_offset += 5;
            }
        }
        // Handle the Action SaveLocation case: apply the state-specific behavior shown here.
        Action::SaveLocation => {
            // Keep short-lived borrow of preference state being rendered or changed in this local
            // variable for the following operations.
            let p = &mut app.preferences;
            // Set selected coordinates, location labels, and IANA timezone to an owned copy of get
            // result for the surrounding operation.
            p.current.location = p.results.get(p.result_selected?)?.clone();
            // Set after save to an available value for Overlay Settings.
            p.after_save = Some(Overlay::Settings);
            // Leave this function now with requests persistence of current preferences unless
            // storage is read-only; later statements are skipped.
            return persist(app);
        }
        // Handle the Action ResetPrompt case: Execute leave for editable preferences and their
        // saved rollback baseline.
        Action::ResetPrompt => app.preferences.leave(Some(Overlay::ResetWifi)),
        // Handle the Action ResetConfirm case: apply the state-specific behavior shown here.
        Action::ResetConfirm => {
            // Keep short-lived borrow of preference state being rendered or changed in this local
            // variable for the following operations.
            let p = &mut app.preferences;
            // Set reset pending to the enabled state.
            p.reset_pending = true;
            // Set status displayed to the reader instead of silent failure to the specified
            // message, format, or data literal.
            p.status = "Resetting Wi-Fi...";
            // Update request identifier used to reject results belonging to cancelled or replaced
            // work using 1, retaining the accumulated state for subsequent steps.
            app.generation += 1;
            // Set next application reconnection time in monotonic seconds, if scheduled to no
            // available value.
            app.retry_at = None;
            // Leave this function now with an available value for Wifi result for the surrounding
            // operation; later statements are skipped.
            return Some(Effect::Wifi(Command::Reset(app.generation)));
        }
        // Handle the Action Calibration case: Execute leave for editable preferences and their
        // saved rollback baseline.
        Action::Calibration => app.preferences.leave(Some(Overlay::CalibrationIntro)),
        // Handle the Action StartCalibration case: apply the state-specific behavior shown here.
        Action::StartCalibration => {
            // Set interactive calibration state, including collected corners and a timeout to an
            // available value for new result for the surrounding operation.
            app.preferences.wizard = Some(Wizard::new(now));
            // Execute leave for editable preferences and their saved rollback baseline.
            app.preferences.leave(Some(Overlay::CalibrationRunning));
        }
        // Handle the Action Brightness case: apply the state-specific behavior shown here.
        Action::Brightness => {}
        // Handle the Action Setup case: apply the state-specific behavior shown here.
        Action::Setup => {
            // Execute leave for editable preferences and their saved rollback baseline.
            app.preferences.leave(None);
            // Leave this function now with the transformed value or entries produced by the
            // closure; later statements are skipped.
            return ui::activate(app, Action::Setup).map(Effect::Wifi);
        }
        // Leave this function now with the transformed value or entries produced by the closure;
        // later statements are skipped.
        other => return ui::activate(app, other).map(Effect::Wifi),
    }
    // Return no available value as the value of this block.
    None
}
/// Previews brightness while dragging and requests persistence when released.
///
/// # Arguments
///
/// * `app` (`&mut App`) - Application model supplying the active page, preferences,
///   weather, and presence state.
/// * `x` (`i32`) - Horizontal screen coordinate in pixels.
/// * `released` (`bool`) - Whether this event ends the current touch gesture.
///
/// # Returns
///
/// `Option<Effect>` - Some persistence effect for changed settings on release; None while
/// previewing, unchanged, read-only, or outside Settings.
pub fn slider(app: &mut App, x: i32, released: bool) -> Option<Effect> {
    // Keep short-lived borrow of preference state being rendered or changed in this local variable
    // for the following operations.
    let p = &mut app.preferences;
    // Brightness dragging is valid only on the writable main settings page.
    if p.overlay != Some(Overlay::Settings) || p.read_only {
        // Leave this function now with no available value; later statements are skipped.
        return None;
    }
    // A drag ended; request persistence now instead of writing flash for every movement sample.
    if released {
        // Set slider active to the disabled state.
        p.slider_active = false;
        // Persist only a changed preference profile, avoiding an unnecessary write when the slider
        // returned to its saved value.
        if p.current != p.saved {
            // Leave this function now with requests persistence of current preferences unless
            // storage is read-only; later statements are skipped.
            return persist(app);
        }
    } else {
        // Set slider active to the enabled state.
        p.slider_active = true;
        // Set backlight duty in 0-255 to the numeric value converted to the required arithmetic or
        // indexing type.
        p.current.brightness = (1 + ((x - 144).clamp(0, 156) * 254 + 78) / 156) as u8;
    }
    // Return no available value as the value of this block.
    None
}
/// Translates, clips, and draws one settings label in the selected font.
///
/// # Arguments
///
/// * `display` (`&mut D`) - Mutable RGB565 draw target receiving the rendering operations.
/// * `app` (`&App`) - Application model supplying the active page, preferences, weather,
///   and presence state.
/// * `text` (`&str`) - Text to translate, measure, clip, or draw.
/// * `x` (`i32`) - Horizontal screen coordinate in pixels.
/// * `y` (`i32`) - Vertical screen coordinate in pixels.
/// * `small` (`bool`) - Whether to use the small font rather than the body font.
/// * `width` (`u32`) - Available drawing or text width in pixels.
///
/// # Type Parameters
///
/// * `D` - RGB565 drawing destination; its associated error is propagated by fallible
///   renderers.
///
/// # Returns
///
/// `Result<(), D::Error>` - Ok after drawing; Err from the draw target.
///
/// # Errors
///
/// Propagates font drawing failures.
fn text<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    app: &App,
    text: &str,
    x: i32,
    y: i32,
    small: bool,
    width: u32,
) -> Result<(), D::Error> {
    // Keep small or body font chosen for this label's available space in this local variable for
    // the following operations.
    let font = if small { &SMALL } else { &BODY };
    // Keep sanitized or clipped text prepared for the available drawing area in this local variable
    // for the following operations.
    let text = theme::clipped(tr(app.preferences.current.language, text), font, width);
    // Send the prepared shape or text pixels to the current drawing destination.
    font.draw(
        display,
        &text,
        Point::new(x, y),
        theme::TEXT,
        Backdrop::Gradient,
        width,
    )
}
/// Translates and wraps a settings message into a limited number of lines.
///
/// # Arguments
///
/// * `display` (`&mut D`) - Mutable RGB565 draw target receiving the rendering operations.
/// * `app` (`&App`) - Application model supplying the active page, preferences, weather,
///   and presence state.
/// * `value` (`&str`) - Text to format or draw.
/// * `y` (`i32`) - Vertical screen coordinate in pixels.
/// * `lines` (`usize`) - Maximum number of wrapped message lines.
///
/// # Type Parameters
///
/// * `D` - RGB565 drawing destination; its associated error is propagated by fallible
///   renderers.
///
/// # Returns
///
/// `Result<(), D::Error>` - Ok after drawing; Err from the draw target.
///
/// # Errors
///
/// Propagates message text drawing failures.
fn message<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    app: &App,
    value: &str,
    y: i32,
    lines: usize,
) -> Result<(), D::Error> {
    // Visit each entry in enumerate result for the surrounding operation; the loop binding provides
    // its value or index for this iteration.
    for (i, line) in theme::wrap(
        tr(app.preferences.current.language, value),
        &BODY,
        288,
        lines,
    )
    .iter()
    .enumerate()
    {
        // Send the prepared shape or text pixels to the current drawing destination.
        BODY.draw(
            display,
            line,
            Point::new(16, y + i as i32 * 18),
            theme::TEXT,
            Backdrop::Gradient,
            288,
        )?;
    }
    // Return success after the required side effects are complete.
    Ok(())
}
/// Draws the active settings overlay, including controls and feedback.
///
/// # Arguments
///
/// * `display` (`&mut D`) - Mutable RGB565 draw target receiving the rendering operations.
/// * `app` (`&App`) - Application model supplying the active page, preferences, weather,
///   and presence state.
///
/// # Type Parameters
///
/// * `D` - RGB565 drawing destination; its associated error is propagated by fallible
///   renderers.
///
/// # Returns
///
/// `Result<(), D::Error>` - Ok after rendering; Err from the draw target.
///
/// # Errors
///
/// Propagates settings drawing failures.
pub fn render<D: DrawTarget<Color = Rgb565>>(display: &mut D, app: &App) -> Result<(), D::Error> {
    // Keep short-lived borrow of preference state being rendered or changed in this local variable
    // for the following operations.
    let p = &app.preferences;
    // Keep selected interface language used for translated labels in this local variable for the
    // following operations.
    let lang = p.current.language;
    // The calibration wizard owns this overlay and its target prompts instead of ordinary settings
    // buttons.
    if p.overlay == Some(Overlay::CalibrationRunning) {
        // Check whether an available interactive calibration state, including collected corners and
        // a timeout matches the requested case.
        if let Some(wizard) = &p.wizard {
            // Keep the returned components: `title` holds page heading selected for the active
            // navigation state, `body` holds synthetic API response body used to exercise parsing
            // in this local variable for the following operations.
            let (title, body) = if wizard.stage < 4 {
                (
                    "Touch calibration",
                    "Tap the yellow cross. Lift the stylus before the next target. BOOT cancels.",
                )
            } else {
                (
                    "Verify calibration",
                    "Tap the yellow cross at the center. BOOT cancels.",
                )
            };
            // Leave this function now with render prompt result for the surrounding operation;
            // later statements are skipped.
            return ui::render_prompt(
                display,
                &format!("{}\n{}", tr(lang, title), tr(lang, body)),
                Some(wizard.target()),
            );
        }
    }
    // Run the background operation with the supplied inputs.
    theme::background(display)?;
    // Keep page heading selected for the active navigation state in this local variable for the
    // following operations.
    let title = match p.overlay {
        // Handle the Some(Overlay Languages) case: apply the state-specific behavior shown here.
        Some(Overlay::Languages) => "Choose a language",
        // Handle the Some(Overlay SleepTimeout) case: apply the state-specific behavior shown here.
        Some(Overlay::SleepTimeout) => "Screen off after",
        // Handle the Some(Overlay LocationInput) case: apply the state-specific behavior shown
        // here.
        Some(Overlay::LocationInput) => "Change location",
        // Handle the Some(Overlay LocationResults) case: apply the state-specific behavior shown
        // here.
        Some(Overlay::LocationResults) => "Search results",
        // Handle the Some(Overlay ResetWifi) case: apply the state-specific behavior shown here.
        Some(Overlay::ResetWifi) => "Reset Wi-Fi",
        // Handle the Some(Overlay CalibrationIntro | Overlay CalibrationError) case: apply the
        // state-specific behavior shown here.
        Some(Overlay::CalibrationIntro | Overlay::CalibrationError) => "Screen calibration",
        // Handle remaining cases with the fallback, preserving safe behavior for unsupported or
        // irrelevant input.
        _ => "Settings",
    };
    // Keep page heading selected for the active navigation state in this local variable for the
    // following operations.
    let title = theme::clipped(tr(lang, title), &TITLE, 288);
    // Send the prepared shape or text pixels to the current drawing destination.
    TITLE.draw(
        display,
        &title,
        Point::new(16, 12),
        theme::TEXT,
        Backdrop::Gradient,
        288,
    )?;
    // Choose the appropriate path for overlay; each arm handles one supported case.
    match p.overlay {
        // Handle the Some(Overlay Settings) case: apply the state-specific behavior shown here.
        Some(Overlay::Settings) => {
            // No warning is hiding the settings clock; show local time rather than status feedback.
            if p.status.is_empty() {
                // Translates, clips, and draws one settings label in the selected font.
                text(display, app, &p.current.clock(p.epoch).unwrap_or_else(|| tr(lang, "Waiting for time synchronization.").into()), 16, 44, true, 288)?;
            } else {
                // Visit each entry in enumerate result for the surrounding operation; the loop
                // binding provides its value or index for this iteration.
                for (i, line) in theme::wrap(tr(lang, p.status), &SMALL, 288, 2).iter().enumerate() {
                    // Send the prepared shape or text pixels to the current drawing destination.
                    SMALL.draw(display, line, Point::new(16, 40 + i as i32 * 15), theme::TEXT, Backdrop::Gradient, 288)?;
                }
            }
            // Run each listed scenario or target in the declared order; the loop variables select
            // the data for that case.
            for (label, y) in [("Brightness", 84), ("Night mode", 124), ("Fahrenheit", 166), ("24-hour time", 208), ("Wake on presence", 250), ("Screen off after", 294), ("Language", 342)] {
                // Check whether `label == "Wake on presence" || label == "Screen off after"`.
                if label == "Wake on presence" || label == "Screen off after" {
                    // Visit each entry in enumerate result for the surrounding operation; the loop
                    // binding provides its value or index for this iteration.
                    for (index, line) in theme::wrap(tr(lang, label), &SMALL, if label == "Screen off after" { 120 } else { 224 }, 2).iter().enumerate() {
                        // Check whether text identifying the control or forecast entry equals the
                        // specified message, format, or data literal.
                        SMALL.draw(display, line, Point::new(16, if label == "Screen off after" { 284 } else { 242 } + index as i32 * 16), theme::TEXT, Backdrop::Gradient, 224)?;
                    }
                } else {
                    // Check whether text identifying the control or forecast entry equals the
                    // specified message, format, or data literal.
                    text(display, app, label, 16, y, true, if label == "Location" { 288 } else { 124 })?;
                }
            }

        }
        // Handle the Some(Overlay LocationInput) case: apply the state-specific behavior shown
        // here.
        Some(Overlay::LocationInput) => {
            // Run the rounded operation with the supplied inputs.
            theme::rounded(display, Rectangle::new(Point::new(12, 92), Size::new(296, 34)), theme::PANEL)?;
            // Keep initial pixel position or timestamp used as the baseline for this operation in
            // this local variable for the following operations.
            let start = p.query.char_indices().map(|(i, _)| i).find(|&i| BODY.width(&p.query[i..]) <= 280).unwrap_or(p.query.len());
            // Send the prepared shape or text pixels to the current drawing destination.
            BODY.draw(display, &p.query[start..], Point::new(20, 100), theme::TEXT, Backdrop::Solid(theme::PANEL), 280)?;
            // Visit each entry in enumerate result for the surrounding operation; the loop binding
            // provides its value or index for this iteration.
            for (i, line) in theme::wrap(&p.current.location.name, &BODY, 288, 2).iter().enumerate() {
                // Send the prepared shape or text pixels to the current drawing destination.
                BODY.draw(display, line, Point::new(16, 44 + i as i32 * 19), theme::TEXT, Backdrop::Gradient, 288)?;
            }
            // An empty city field needs an input hint; otherwise display the entered query.
            if p.query.is_empty() {
                // Translates, clips, and draws one settings label in the selected font.
                text(display, app, "Enter a city or postal code.", 20, 100, true, 280)?;
            }
            // Translates and wraps a settings message into a limited number of lines.
            message(display, app, p.status, 176, 2)?;
            // Translates, clips, and draws one settings label in the selected font.
            text(display, app, "Open-Meteo / GeoNames", 16, 216, true, 288)?;
        }
        // Handle the Some(Overlay LocationResults) case: apply the state-specific behavior shown
        // here.
        Some(Overlay::LocationResults) => {
            // Show actionable search feedback without repeating the normal selection prompt.
            if !p.status.is_empty() && p.status != "Select a location." { message(display, app, p.status, 46, 2)?; }
            // Check whether an available result selected matches the requested case.
            else if let Some(index) = p.result_selected { message(display, app, &p.results[index].name, 46, 2)?; }
            // Translates and wraps a settings message into a limited number of lines.
            else { message(display, app, "Select a location.", 46, 2)?; }
            // Translates, clips, and draws one settings label in the selected font.
            text(display, app, "Open-Meteo / GeoNames", 16, 90, true, 288)?;
        }
        // Handle the Some(Overlay SleepTimeout) case: apply the state-specific behavior shown here.
        Some(Overlay::SleepTimeout) => message(display, app, p.status, 370, 3)?,
        // Handle the Some(Overlay ResetWifi) case: apply the state-specific behavior shown here.
        Some(Overlay::ResetWifi) => {
            // Translates and wraps a settings message into a limited number of lines.
            message(display, app, "Forget the saved Wi-Fi network? Settings and calibration will be kept.", 76, 10)?;
            // Translates and wraps a settings message into a limited number of lines.
            message(display, app, p.status, 272, 4)?;
        }
        // Handle the Some(Overlay CalibrationIntro) case: apply the state-specific behavior shown
        // here.
        Some(Overlay::CalibrationIntro) => message(display, app, "Tap four corner targets, then the center. Lift the stylus between targets. Press BOOT to cancel.", 76, 12)?,
        // Handle the Some(Overlay CalibrationError) case: apply the state-specific behavior shown
        // here.
        Some(Overlay::CalibrationError) => message(display, app, p.status, 76, 12)?,
        // Handle remaining cases with the fallback, preserving safe behavior for unsupported or
        // irrelevant input.
        _ => {}
    }
    // Run the render presence operation with the supplied inputs.
    crate::ui::render_presence(display, app)?;
    // Visit each entry in builds controls for the current settings overlay and preference state;
    // the loop binding provides its value or index for this iteration.
    for button in controls(app) {
        // Check whether action equals Action Brightness.
        if button.action == Action::Brightness {
            // Run the rounded operation with the supplied inputs.
            theme::rounded(
                display,
                Rectangle::new(Point::new(144, 89), Size::new(156, 6)),
                theme::MUTED,
            )?;
            // Keep horizontal coordinate or current position along the row in this local variable
            // for the following operations.
            let x = 144 + ((p.current.brightness as i32 - 1) * 156 / 254);
            // Send the prepared shape or text pixels to the current drawing destination.
            Circle::new(Point::new(x - 8, 84), 16)
                .into_styled(PrimitiveStyle::with_fill(theme::TEXT))
                .draw(display)?;
            // Skip the remainder of this iteration and wait for or inspect the next input.
            continue;
        }
        // Keep optional on/off value for a toggle control; non-toggle buttons have no switch value
        // in this local variable for the following operations.
        let switch = match button.action {
            // Handle the Action Night case: Return an available whether the local 22:00-06:00
            // backlight schedule is enabled.
            Action::Night => Some(p.current.night_mode),
            // Handle the Action Units case: Return an available whether displayed temperatures are
            // converted from cached Celsius to Fahrenheit.
            Action::Units => Some(p.current.fahrenheit),
            // Handle the Action Clock case: Return an available whether local time uses 24-hour
            // rather than 12-hour formatting.
            Action::Clock => Some(p.current.clock_24h),
            // Handle the Action WakeOnPresence case: Return an available whether radar presence
            // counts as activity and can restore the backlight.
            Action::WakeOnPresence => Some(p.current.wake_on_presence),
            // Handle remaining cases with the fallback, preserving safe behavior for unsupported or
            // irrelevant input.
            _ => None,
        };
        // Check whether an available optional on/off value for a toggle control; non-toggle buttons
        // have no switch value matches the requested case.
        if let Some(on) = switch {
            // Keep selected panel colour distinguishing enabled, selected, or disabled controls in
            // this local variable for the following operations.
            let fill = if on {
                // Run the rgb operation with the supplied inputs.
                theme::rgb(0x2878ae)
            } else {
                theme::PANEL
            };
            // Run the rounded operation with the supplied inputs.
            theme::rounded(display, button.bounds, fill)?;
            // Keep horizontal coordinate or current position along the row in this local variable
            // for the following operations.
            let x = button.bounds.top_left.x + if on { 30 } else { 4 };
            // Send the prepared shape or text pixels to the current drawing destination.
            Circle::new(Point::new(x, button.bounds.top_left.y + 6), 20)
                .into_styled(PrimitiveStyle::with_fill(theme::TEXT))
                .draw(display)?;
            // Skip the remainder of this iteration and wait for or inspect the next input.
            continue;
        }
        // Keep access point currently selected for candidate setup in this local variable for the
        // following operations.
        let selected = matches!(button.action, Action::Language(language) if language == lang)
            || matches!(button.action, Action::SelectLocation(index) if Some(index) == p.result_selected)
            || matches!(button.action, Action::SelectSleepTimeout(minutes) if minutes == p.current.sleep_minutes);
        // Keep selected panel colour distinguishing enabled, selected, or disabled controls in this
        // local variable for the following operations.
        let fill = if matches!(button.action, Action::ResetPrompt | Action::ResetConfirm) {
            // Run the rgb operation with the supplied inputs.
            theme::rgb(0xc03939)
        // Check whether access point currently selected for candidate setup.
        } else if selected {
            // Run the rgb operation with the supplied inputs.
            theme::rgb(0x2878ae)
        // Check whether enabled.
        } else if button.enabled {
            theme::PANEL
        } else {
            // Run the blend operation with the supplied inputs.
            theme::blend(
                theme::PANEL,
                theme::background_at(button.bounds.top_left.y),
                9,
            )
        };
        // Run the rounded operation with the supplied inputs.
        theme::rounded(display, button.bounds, fill)?;
        // Keep text identifying the control or forecast entry in this local variable for the
        // following operations.
        let label = if matches!(
            button.action,
            Action::SelectLocation(_)
                | Action::Language(_)
                | Action::Languages
                | Action::CityCharacter(_)
        ) {
            // Execute as str for text identifying the control or forecast entry.
            button.label.as_str()
        } else {
            // Run the tr operation with the supplied inputs.
            tr(lang, &button.label)
        };
        // Check whether the formatted text or fixture created by matches and overlay equals an
        // available value for Overlay Settings.
        if matches!(button.action, Action::Setup | Action::Calibration)
            && p.overlay == Some(Overlay::Settings)
        {
            // Keep measured text lines that fit the available width and line count in this local
            // variable for the following operations.
            let lines = theme::wrap(label, &SMALL, 130, 2);
            // Visit each entry in enumerate result for the surrounding operation; the loop binding
            // provides its value or index for this iteration.
            for (index, line) in lines.iter().enumerate() {
                // Send the prepared shape or text pixels to the current drawing destination.
                SMALL.draw(
                    display,
                    line,
                    Point::new(
                        button.bounds.top_left.x + (140 - SMALL.width(line) as i32) / 2,
                        button.bounds.top_left.y
                            + (30 - lines.len() as i32 * 15) / 2
                            + index as i32 * 15,
                    ),
                    theme::TEXT,
                    Backdrop::Solid(fill),
                    130,
                )?;
            }
            // Skip the remainder of this iteration and wait for or inspect the next input.
            continue;
        }
        // Keep text identifying the control or forecast entry in this local variable for the
        // following operations.
        let label = theme::clipped(label, &SMALL, button.bounds.size.width.saturating_sub(10));
        // Send the prepared shape or text pixels to the current drawing destination.
        SMALL.draw(
            display,
            &label,
            Point::new(
                button.bounds.top_left.x
                    + (button.bounds.size.width as i32 - SMALL.width(&label) as i32) / 2,
                button.bounds.top_left.y
                    + (button.bounds.size.height as i32 - SMALL.height as i32) / 2,
            ),
            // Check whether enabled.
            if button.enabled {
                theme::TEXT
            } else {
                theme::MUTED
            },
            Backdrop::Solid(fill),
            button.bounds.size.width.saturating_sub(10),
        )?;
    }
    // Return success after the required side effects are complete.
    Ok(())
}

///////////////////////////////////////////////////////////////////////////////////////
//                                    UNIT TESTS                                     //
///////////////////////////////////////////////////////////////////////////////////////

/// Unit tests and supporting fixtures for settings ui behavior.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Event, Page};

    /// Verifies Ukrainian language selection, city input modes, and localized geocoding request
    /// behavior.
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
    fn ukrainian_selection_keyboard_and_search() {
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App::default();
        // Set overlay to an available value for Overlay Languages.
        app.preferences.overlay = Some(Overlay::Languages);
        // Keep one visible control with bounds, label, enabled state, and action in this local
        // variable for the following operations.
        let button = controls(&app)
            .into_iter()
            .find(|b| b.action == Action::Language(Language::Ukrainian))
            .unwrap();
        // Verify that hit result for the surrounding operation exactly matches an available value
        // for action.
        assert_eq!(
            ui::hit(&app, button.bounds.center().x, button.bounds.center().y),
            Some(button.action)
        );
        // Keep Some(Effect::Persist(settings)) in this local variable for the following operations.
        let Some(Effect::Persist(settings)) = activate(&mut app, button.action, 0) else {
            // Stop this test immediately: reaching this path violates the expected fixture or API
            // behavior.
            panic!("expected persistence")
        };
        // Verify that selected language for interface text and location search exactly matches
        // Language Ukrainian.
        assert_eq!(settings.language, Language::Ukrainian);
        // Execute saved for editable preferences and their saved rollback baseline.
        app.preferences.saved(false);
        // Verify that selected language for interface text and location search exactly matches
        // Language English.
        assert_eq!(app.preferences.current.language, Language::English);
        // Applies a settings action and produces any required external effect.
        activate(&mut app, button.action, 0);
        // Execute saved for editable preferences and their saved rollback baseline.
        app.preferences.saved(true);
        // Verify that selected language for interface text and location search exactly matches
        // Language Ukrainian.
        assert_eq!(
            crate::settings::Settings::decode(&app.preferences.saved.encode().unwrap())
                .unwrap()
                .language,
            Language::Ukrainian
        );
        // Verify that formats an inactivity timeout in the selected interface language exactly
        // matches the specified message, format, or data literal.
        assert_eq!(timeout_label(Language::Ukrainian, 5), "5 хв");
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::Location, 0);
        // Verify city cyrillic. A violation means the tested behavior is incorrect.
        assert!(app.preferences.city_cyrillic);
        // Verify that selected on-screen keyboard layout exactly matches 4.
        assert_eq!(app.preferences.keyboard, 4);
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for (mode, alphabet) in [
            (4, "абвгґдеєжзиіїйклмнопрстуфхцчшщьюя'"),
            (5, "АБВГҐДЕЄЖЗИІЇЙКЛМНОПРСТУФХЦЧШЩЬЮЯ'"),
        ] {
            // Set selected on-screen keyboard layout to mode.
            app.preferences.keyboard = mode;
            // Keep visible controls generated for the current page or overlay in this local
            // variable for the following operations.
            let buttons = controls(&app);
            // Visit each entry in chars result for the surrounding operation; the loop binding
            // provides its value or index for this iteration.
            for ch in alphabet.chars() {
                // Keep one visible control with bounds, label, enabled state, and action in this
                // local variable for the following operations.
                let button = buttons
                    .iter()
                    .find(|b| b.action == Action::CityCharacter(ch))
                    .unwrap();
                // Verify that hit result for the surrounding operation exactly matches an available
                // value for action.
                assert_eq!(
                    ui::hit(&app, button.bounds.center().x, button.bounds.center().y),
                    Some(button.action)
                );
            }
            // Visit each entry in chars result for the surrounding operation; the loop binding
            // provides its value or index for this iteration.
            for ch in "ыъэёЫЪЭЁ".chars() {
                // Verify the inverse of at least one entry satisfies the stated condition. A
                // violation means the tested behavior is incorrect.
                assert!(!buttons
                    .iter()
                    .any(|b| b.action == Action::CityCharacter(ch)));
            }
            // Visit each entry in visible controls generated for the current page or overlay; the
            // loop binding provides its value or index for this iteration.
            for button in buttons {
                // Verify horizontal coordinate or current position along the row has reached or
                // exceeded 0. A violation means the tested behavior is incorrect.
                assert!(button.bounds.top_left.x >= 0);
                // Verify horizontal coordinate or current position along the row plus the numeric
                // value converted to the required arithmetic or indexing type does not exceed 320.
                // A violation means the tested behavior is incorrect.
                assert!(button.bounds.top_left.x + button.bounds.size.width as i32 <= 320);
            }
        }
        // Set selected on-screen keyboard layout to 4.
        app.preferences.keyboard = 4;
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::CityShift, 0);
        // Verify that selected on-screen keyboard layout exactly matches 5.
        assert_eq!(app.preferences.keyboard, 5);
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::CityShift, 0);
        // Verify that selected on-screen keyboard layout exactly matches 4.
        assert_eq!(app.preferences.keyboard, 4);
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for action in [Action::CitySymbols, Action::CityAccents] {
            // Applies a settings action and produces any required external effect.
            activate(&mut app, action, 0);
            // Applies a settings action and produces any required external effect.
            activate(&mut app, action, 0);
            // Verify that selected on-screen keyboard layout exactly matches 4.
            assert_eq!(app.preferences.keyboard, 4);
        }
        // Visit each entry in chars result for the surrounding operation; the loop binding provides
        // its value or index for this iteration.
        for ch in "Київ Ґ Є І Ї'".chars() {
            // Applies a settings action and produces any required external effect.
            activate(&mut app, Action::CityCharacter(ch), 0);
        }
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::CityAlphabet, 0);
        // Verify that selected on-screen keyboard layout exactly matches 0.
        assert_eq!(app.preferences.keyboard, 0);
        // Verify that city search text or encoded API request being examined exactly matches the
        // specified message, format, or data literal.
        assert_eq!(app.preferences.query, "Київ Ґ Є І Ї'");
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::CityAlphabet, 0);
        // Verify that selected on-screen keyboard layout exactly matches 4.
        assert_eq!(app.preferences.keyboard, 4);
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::CityDelete, 0);
        // Verify that city search text or encoded API request being examined exactly matches the
        // specified message, format, or data literal.
        assert_eq!(app.preferences.query, "Київ Ґ Є І Ї");
        // Set city search text or encoded API request being examined to into result for the
        // surrounding operation.
        app.preferences.query = "Київ".into();
        // Keep Some(Effect::Search(location::Request::Search {             query, language, .. }))
        // in this local variable for the following operations.
        let Some(Effect::Search(location::Request::Search {
            query, language, ..
        })) = activate(&mut app, Action::SearchCity, 0)
        else {
            // Stop this test immediately: reaching this path violates the expected fixture or API
            // behavior.
            panic!("expected search")
        };
        // Verify that selected language for interface text and location search exactly matches
        // Language Ukrainian.
        assert_eq!(language, Language::Ukrainian);
        // Verify ends with result for the surrounding operation. A violation means the tested
        // behavior is incorrect.
        assert!(location::url(&query, language).ends_with("&language=uk"));
        // Verify the value falls within the stated range or belongs to the supported set. A
        // violation means the tested behavior is incorrect.
        assert!(location::url(&query, language).contains("%D0%9A%D0%B8%D1%97%D0%B2"));
    }
    /// Verifies successful Russian language persistence and rollback when saving fails.
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
    fn russian_language_selection_persists_and_rolls_back() {
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App::default();
        // Set overlay to an available value for Overlay Languages.
        app.preferences.overlay = Some(Overlay::Languages);
        // Keep one visible control with bounds, label, enabled state, and action in this local
        // variable for the following operations.
        let button = controls(&app)
            .into_iter()
            .find(|b| b.action == Action::Language(Language::Russian))
            .unwrap();
        // Verify that hit result for the surrounding operation exactly matches an available value
        // for action.
        assert_eq!(
            ui::hit(&app, button.bounds.center().x, button.bounds.center().y),
            Some(button.action)
        );
        // Keep Some(Effect::Persist(settings)) in this local variable for the following operations.
        let Some(Effect::Persist(settings)) = activate(&mut app, button.action, 0) else {
            // Stop this test immediately: reaching this path violates the expected fixture or API
            // behavior.
            panic!("language must be persisted")
        };
        // Verify that selected language for interface text and location search exactly matches
        // Language Russian.
        assert_eq!(settings.language, Language::Russian);
        // Verify that formats an inactivity timeout in the selected interface language exactly
        // matches the specified message, format, or data literal.
        assert_eq!(timeout_label(Language::Russian, 5), "5 мин");
        // Execute saved for editable preferences and their saved rollback baseline.
        app.preferences.saved(false);
        // Verify that selected language for interface text and location search exactly matches
        // Language English.
        assert_eq!(app.preferences.current.language, Language::English);
        // Applies a settings action and produces any required external effect.
        activate(&mut app, button.action, 0);
        // Execute saved for editable preferences and their saved rollback baseline.
        app.preferences.saved(true);
        // Keep value decoded after persistence or recreated from the prior saved profile in this
        // local variable for the following operations.
        let restored =
            crate::settings::Settings::decode(&app.preferences.saved.encode().unwrap()).unwrap();
        // Verify that selected language for interface text and location search exactly matches
        // Language Russian.
        assert_eq!(
            crate::settings::Preferences::new(restored, false)
                .current
                .language,
            Language::Russian
        );
    }

    /// Verifies complete Russian alphabet input, keyboard mode transitions, and character-count
    /// limits for Unicode city names.
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
    fn russian_city_keyboard_covers_alphabet_modes_and_unicode_limits() {
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App::default();
        // Set selected language for interface text and location search to Language Russian.
        app.preferences.current.language = Language::Russian;
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::Location, 0);
        // Verify city cyrillic. A violation means the tested behavior is incorrect.
        assert!(app.preferences.city_cyrillic);
        // Verify that selected on-screen keyboard layout exactly matches 4.
        assert_eq!(app.preferences.keyboard, 4);
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for (mode, alphabet) in [
            (4, "абвгдеёжзийклмнопрстуфхцчшщъыьэюя"),
            (5, "АБВГДЕЁЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЫЬЭЮЯ"),
        ] {
            // Set selected on-screen keyboard layout to mode.
            app.preferences.keyboard = mode;
            // Keep visible controls generated for the current page or overlay in this local
            // variable for the following operations.
            let buttons = controls(&app);
            // Visit each entry in chars result for the surrounding operation; the loop binding
            // provides its value or index for this iteration.
            for ch in alphabet.chars() {
                // Keep one visible control with bounds, label, enabled state, and action in this
                // local variable for the following operations.
                let button = buttons
                    .iter()
                    .find(|b| b.action == Action::CityCharacter(ch))
                    .unwrap();
                // Verify that hit result for the surrounding operation exactly matches an available
                // value for CityCharacter result for the surrounding operation.
                assert_eq!(
                    ui::hit(&app, button.bounds.center().x, button.bounds.center().y),
                    Some(Action::CityCharacter(ch))
                );
            }
            // Visit each entry in visible controls generated for the current page or overlay; the
            // loop binding provides its value or index for this iteration.
            for button in buttons {
                // Verify horizontal coordinate or current position along the row has reached or
                // exceeded 0. A violation means the tested behavior is incorrect.
                assert!(button.bounds.top_left.x >= 0);
                // Verify horizontal coordinate or current position along the row plus the numeric
                // value converted to the required arithmetic or indexing type does not exceed 320.
                // A violation means the tested behavior is incorrect.
                assert!(button.bounds.top_left.x + button.bounds.size.width as i32 <= 320);
            }
        }
        // Set selected on-screen keyboard layout to 4.
        app.preferences.keyboard = 4;
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::CityShift, 0);
        // Verify that selected on-screen keyboard layout exactly matches 5.
        assert_eq!(app.preferences.keyboard, 5);
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::CityShift, 0);
        // Verify that selected on-screen keyboard layout exactly matches 4.
        assert_eq!(app.preferences.keyboard, 4);
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for action in [Action::CitySymbols, Action::CityAccents] {
            // Applies a settings action and produces any required external effect.
            activate(&mut app, action, 0);
            // Applies a settings action and produces any required external effect.
            activate(&mut app, action, 0);
            // Verify that selected on-screen keyboard layout exactly matches 4.
            assert_eq!(app.preferences.keyboard, 4);
        }
        // Set city search text or encoded API request being examined to into result for the
        // surrounding operation.
        app.preferences.query = "Москва Ё".into();
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::CityAlphabet, 0);
        // Verify that selected on-screen keyboard layout exactly matches 0.
        assert_eq!(app.preferences.keyboard, 0);
        // Verify that city search text or encoded API request being examined exactly matches the
        // specified message, format, or data literal.
        assert_eq!(app.preferences.query, "Москва Ё");
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for action in [Action::CitySymbols, Action::CityAccents] {
            // Applies a settings action and produces any required external effect.
            activate(&mut app, action, 0);
            // Applies a settings action and produces any required external effect.
            activate(&mut app, action, 0);
            // Verify that selected on-screen keyboard layout exactly matches 0.
            assert_eq!(app.preferences.keyboard, 0);
        }
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::CityAlphabet, 0);
        // Verify that selected on-screen keyboard layout exactly matches 4.
        assert_eq!(app.preferences.keyboard, 4);
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::CityDelete, 0);
        // Verify that city search text or encoded API request being examined exactly matches the
        // specified message, format, or data literal.
        assert_eq!(app.preferences.query, "Москва ");
        // Set city search text or encoded API request being examined to repeat result for the
        // surrounding operation.
        app.preferences.query = "ё".repeat(63);
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::CityCharacter('Ё'), 0);
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::CityCharacter('Я'), 0);
        // Verify that count result for the surrounding operation exactly matches 64.
        assert_eq!(app.preferences.query.chars().count(), 64);
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::CityDelete, 0);
        // Verify that city search text or encoded API request being examined exactly matches repeat
        // result for the surrounding operation.
        assert_eq!(app.preferences.query, "ё".repeat(63));
        // Set city search text or encoded API request being examined to into result for the
        // surrounding operation.
        app.preferences.query = "Москва".into();
        // Keep Some(Effect::Search(location::Request::Search {             query, language, .. }))
        // in this local variable for the following operations.
        let Some(Effect::Search(location::Request::Search {
            query, language, ..
        })) = activate(&mut app, Action::SearchCity, 0)
        else {
            // Stop this test immediately: reaching this path violates the expected fixture or API
            // behavior.
            panic!("expected search")
        };
        // Verify that selected language for interface text and location search exactly matches
        // Language Russian.
        assert_eq!(language, Language::Russian);
        // Verify that city search text or encoded API request being examined exactly matches the
        // specified message, format, or data literal.
        assert_eq!(query, "Москва");
    }

    /// Verifies that settings navigation retains access to Wi-Fi setup and touch calibration
    /// controls.
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
    fn gear_footer_and_settings_keep_wifi_and_calibration_accessible() {
        // Visit each entry in Language ALL; the loop binding provides its value or index for this
        // iteration.
        for language in Language::ALL {
            // Keep application state used by navigation, preferences, weather, and rendering in
            // this local variable for the following operations.
            let mut app = App {
                // Initialize currently selected Wi-Fi navigation page from the supplied value.
                page: Page::Connected,
                ..App::default()
            };
            // Set selected language for interface text and location search to selected language for
            // interface text and location search.
            app.preferences.current.language = language;
            // Run each listed scenario or target in the declared order; the loop variables select
            // the data for that case.
            for view in [
                crate::weather::View::Clock,
                crate::weather::View::Week,
                crate::weather::View::Hours,
            ] {
                // Set view to view.
                app.weather.view = view;
                // Run each listed scenario or target in the declared order; the loop variables
                // select the data for that case.
                for (x, y) in [(288, 458), (260, 427), (312, 427), (260, 479), (312, 479)] {
                    // Verify that hit result for the surrounding operation exactly matches an
                    // available value for Action OpenSettings.
                    assert_eq!(ui::hit(&app, x, y), Some(Action::OpenSettings));
                }
                // Run each listed scenario or target in the declared order; the loop variables
                // select the data for that case.
                for (x, y) in [(259, 458), (313, 458), (288, 480), (160, 427)] {
                    // Verify that hit result for the surrounding operation exactly matches no
                    // available value.
                    assert_eq!(ui::hit(&app, x, y), None);
                }
                // Verify that hit result for the surrounding operation exactly matches an available
                // value for Action CycleWeather.
                assert_eq!(ui::hit(&app, 288, 426), Some(Action::CycleWeather));
                // Verify that hit result for the surrounding operation exactly matches no available
                // value.
                assert_eq!(ui::hit(&app, 20, 458), None);
                // Verify that hit result for the surrounding operation exactly matches an available
                // value for Action CycleWeather.
                assert_eq!(ui::hit(&app, 160, 300), Some(Action::CycleWeather));
            }
            // Applies a settings action and produces any required external effect.
            activate(&mut app, Action::OpenSettings, 0);
            // Keep rendered control list inspected by the settings tests in this local variable for
            // the following operations.
            let actions = controls(&app);
            // Keep Wi-Fi setup button found at its expected settings-footer position in this local
            // variable for the following operations.
            let wifi = actions
                .iter()
                .find(|b| b.bounds.contains(Point::new(80, 423)))
                .unwrap();
            // Keep calibration button found at its expected settings-footer position in this local
            // variable for the following operations.
            let calibration = actions
                .iter()
                .find(|b| b.bounds.contains(Point::new(238, 423)))
                .unwrap();
            // Verify that action exactly matches Action Setup.
            assert_eq!(wifi.action, Action::Setup);
            // Verify that action exactly matches Action Calibration.
            assert_eq!(calibration.action, Action::Calibration);
            // Verify at least one entry satisfies the stated condition. A violation means the
            // tested behavior is incorrect.
            assert!(actions.iter().any(|b| b.action == Action::ResetPrompt));
            // Verify at least one entry satisfies the stated condition. A violation means the
            // tested behavior is incorrect.
            assert!(actions.iter().any(|b| b.action == Action::CloseSettings));
            // Applies a settings action and produces any required external effect.
            activate(&mut app, calibration.action.clone(), 1);
            // Verify that overlay exactly matches an available value for Overlay CalibrationIntro.
            assert_eq!(app.preferences.overlay, Some(Overlay::CalibrationIntro));
            // Applies a settings action and produces any required external effect.
            activate(&mut app, Action::SettingsBack, 2);
            // Verify the formatted text or fixture created by matches. A violation means the tested
            // behavior is incorrect.
            assert!(matches!(
                activate(&mut app, wifi.action.clone(), 3),
                Some(Effect::Wifi(_))
            ));
            // Verify overlay is unavailable. A violation means the tested behavior is incorrect.
            assert!(app.preferences.overlay.is_none());
            // Verify that currently selected Wi-Fi navigation page differs from Connected state.
            assert_ne!(app.page, Page::Connected);
        }
    }
    /// Verifies timeout selection, immediate persistence, rollback on failure, and retained
    /// location display.
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
    fn timeout_selection_saves_and_rolls_back_without_hiding_saved_location() {
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App::default();
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::OpenSettings, 0);
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::SleepTimeout, 1);
        // Verify that overlay exactly matches an available value for Overlay SleepTimeout.
        assert_eq!(app.preferences.overlay, Some(Overlay::SleepTimeout));
        // Verify the formatted text or fixture created by matches. A violation means the tested
        // behavior is incorrect.
        assert!(matches!(
            activate(&mut app, Action::SelectSleepTimeout(10), 2),
            Some(Effect::Persist(_))
        ));
        // Execute saved for editable preferences and their saved rollback baseline.
        app.preferences.saved(false);
        // Verify that inactivity interval in minutes exactly matches 5.
        assert_eq!(app.preferences.current.sleep_minutes, 5);
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::SleepTimeout, 3);
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::SelectSleepTimeout(0), 4);
        // Execute saved for editable preferences and their saved rollback baseline.
        app.preferences.saved(true);
        // Verify that inactivity interval in minutes exactly matches 0.
        assert_eq!(app.preferences.saved.sleep_minutes, 0);
        // Verify applies a settings action and produces any required external effect is
        // unavailable. A violation means the tested behavior is incorrect.
        assert!(activate(&mut app, Action::SelectSleepTimeout(2), 5).is_none());
        // Set whether preference writes are disabled after a storage read failure to the enabled
        // state.
        app.preferences.read_only = true;
        // Verify applies a settings action and produces any required external effect is
        // unavailable. A violation means the tested behavior is incorrect.
        assert!(activate(&mut app, Action::SelectSleepTimeout(30), 6).is_none());
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::Location, 7);
        // Verify that overlay exactly matches an available value for Overlay LocationInput.
        assert_eq!(app.preferences.overlay, Some(Overlay::LocationInput));
        // Verify the inverse of human-readable label for the selected object is empty. A violation
        // means the tested behavior is incorrect.
        assert!(!app.preferences.current.location.name.is_empty());
    }
    /// Verifies immediate presence-wake persistence and restoration of the saved value after
    /// failure.
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
    fn presence_switch_saves_immediately_and_rolls_back_on_failure() {
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App::default();
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::OpenSettings, 0);
        // Verify at least one entry satisfies the stated condition. A violation means the tested
        // behavior is incorrect.
        assert!(controls(&app)
            .iter()
            .any(|button| button.action == Action::WakeOnPresence && button.enabled));
        // Keep Some(Effect::Persist(settings)) in this local variable for the following operations.
        let Some(Effect::Persist(settings)) = activate(&mut app, Action::WakeOnPresence, 0) else {
            // Stop this test immediately: reaching this path violates the expected fixture or API
            // behavior.
            panic!("missing save");
        };
        // Verify the inverse of whether radar presence counts as activity and can restore the
        // backlight. A violation means the tested behavior is incorrect.
        assert!(!settings.wake_on_presence);
        // Execute saved for editable preferences and their saved rollback baseline.
        app.preferences.saved(false);
        // Verify whether radar presence counts as activity and can restore the backlight. A
        // violation means the tested behavior is incorrect.
        assert!(app.preferences.current.wake_on_presence);
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::WakeOnPresence, 1);
        // Execute saved for editable preferences and their saved rollback baseline.
        app.preferences.saved(true);
        // Verify the inverse of whether radar presence counts as activity and can restore the
        // backlight. A violation means the tested behavior is incorrect.
        assert!(!app.preferences.saved.wake_on_presence);
        // Set whether preference writes are disabled after a storage read failure to the enabled
        // state.
        app.preferences.read_only = true;
        // Verify the inverse of enabled. A violation means the tested behavior is incorrect.
        assert!(
            !controls(&app)
                .iter()
                .find(|button| button.action == Action::WakeOnPresence)
                .unwrap()
                .enabled
        );
        // Verify applies a settings action and produces any required external effect is
        // unavailable. A violation means the tested behavior is incorrect.
        assert!(activate(&mut app, Action::WakeOnPresence, 2).is_none());
        // Verify the inverse of whether radar presence counts as activity and can restore the
        // backlight. A violation means the tested behavior is incorrect.
        assert!(!app.preferences.current.wake_on_presence);
    }
    /// Verifies that connection events preserve settings overlays and closing returns to the
    /// latest underlying page.
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
    fn overlays_survive_wifi_events_and_close_to_latest_page() {
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App::default();
        // Execute handle for application state used by navigation, preferences, weather, and
        // rendering.
        app.handle(Event::Ready(Some("Home".into())), 0);
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::OpenSettings, 0);
        // Execute handle for application state used by navigation, preferences, weather, and
        // rendering.
        app.handle(Event::Connected(app.generation, "Home".into()), 1);
        // Verify that overlay exactly matches an available value for Overlay Settings.
        assert_eq!(app.preferences.overlay, Some(Overlay::Settings));
        // Execute handle for application state used by navigation, preferences, weather, and
        // rendering.
        app.handle(Event::Lost, 2);
        // Verify that overlay exactly matches an available value for Overlay Settings.
        assert_eq!(app.preferences.overlay, Some(Overlay::Settings));
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::CloseSettings, 2000);
        // Verify that currently selected Wi-Fi navigation page exactly matches Connecting state.
        assert_eq!(app.page, Page::Connecting);
        // Verify overlay is unavailable. A violation means the tested behavior is incorrect.
        assert!(app.preferences.overlay.is_none());
    }
    /// Verifies live brightness preview, persistence only on release, and rollback when saving
    /// fails.
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
    fn slider_previews_and_saves_only_on_release_and_failure_rolls_back() {
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App::default();
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::OpenSettings, 0);
        // Verify previews brightness while dragging and requests persistence when released is
        // unavailable. A violation means the tested behavior is incorrect.
        assert!(slider(&mut app, 144, false).is_none());
        // Verify that backlight duty in 0-255 exactly matches 1.
        assert_eq!(app.preferences.current.brightness, 1);
        // Verify that backlight duty in 0-255 exactly matches 255.
        assert_eq!(app.preferences.saved.brightness, 255);
        // Verify previews brightness while dragging and requests persistence when released is
        // unavailable. A violation means the tested behavior is incorrect.
        assert!(slider(&mut app, 222, false).is_none());
        // Verify that backlight duty in 0-255 exactly matches 128.
        assert_eq!(app.preferences.current.brightness, 128);
        // Verify the formatted text or fixture created by matches. A violation means the tested
        // behavior is incorrect.
        assert!(matches!(
            slider(&mut app, 0, true),
            Some(Effect::Persist(_))
        ));
        // Execute saved for editable preferences and their saved rollback baseline.
        app.preferences.saved(false);
        // Verify that backlight duty in 0-255 exactly matches 255.
        assert_eq!(app.preferences.current.brightness, 255);
        // Set whether preference writes are disabled after a storage read failure to the enabled
        // state.
        app.preferences.read_only = true;
        // Verify previews brightness while dragging and requests persistence when released is
        // unavailable. A violation means the tested behavior is incorrect.
        assert!(slider(&mut app, 144, false).is_none());
        // Verify that backlight duty in 0-255 exactly matches 255.
        assert_eq!(app.preferences.current.brightness, 255);
    }
    /// Verifies that Wi-Fi reset requires explicit confirmation and calibration requires a
    /// separate start action.
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
    fn reset_requires_confirmation_and_calibration_requires_start() {
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App {
            // Initialize name of the successfully persisted station profile, if one exists from the
            // supplied value.
            saved_ssid: Some("Home".into()),
            ..App::default()
        };
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::ResetPrompt, 0);
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::SettingsBack, 0);
        // Verify that as deref result for the surrounding operation exactly matches an available
        // value for the specified message, format, or data literal.
        assert_eq!(app.saved_ssid.as_deref(), Some("Home"));
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::Calibration, 0);
        // Verify interactive calibration state, including collected corners and a timeout is
        // unavailable. A violation means the tested behavior is incorrect.
        assert!(app.preferences.wizard.is_none());
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::StartCalibration, 100);
        // Verify that overlay exactly matches an available value for Overlay CalibrationRunning.
        assert_eq!(app.preferences.overlay, Some(Overlay::CalibrationRunning));
        // Verify interactive calibration state, including collected corners and a timeout is
        // available. A violation means the tested behavior is incorrect.
        assert!(app.preferences.wizard.is_some());
        // Applies a settings action and produces any required external effect.
        activate(&mut app, Action::ResetPrompt, 0);
        // Verify the formatted text or fixture created by matches. A violation means the tested
        // behavior is incorrect.
        assert!(matches!(
            activate(&mut app, Action::ResetConfirm, 0),
            Some(Effect::Wifi(Command::Reset(_)))
        ));
    }
    /// Verifies usable control geometry and expected actions for every language and settings
    /// overlay.
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
    fn every_locale_and_overlay_has_valid_controls() {
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App::default();
        // Visit each entry in Language ALL; the loop binding provides its value or index for this
        // iteration.
        for language in Language::ALL {
            // Set selected language for interface text and location search to selected language for
            // interface text and location search.
            app.preferences.current.language = language;
            // Run each listed scenario or target in the declared order; the loop variables select
            // the data for that case.
            for overlay in [
                Overlay::Settings,
                Overlay::Languages,
                Overlay::SleepTimeout,
                Overlay::LocationInput,
                Overlay::LocationResults,
                Overlay::ResetWifi,
                Overlay::CalibrationIntro,
                Overlay::CalibrationError,
            ] {
                // Set overlay to an available value for overlay.
                app.preferences.overlay = Some(overlay);
                // Visit each entry in builds controls for the current settings overlay and
                // preference state; the loop binding provides its value or index for this
                // iteration.
                for control in controls(&app) {
                    // Verify horizontal coordinate or current position along the row has reached or
                    // exceeded 0 and vertical coordinate or current screen row has reached or
                    // exceeded 0. A violation means the tested behavior is incorrect.
                    assert!(control.bounds.top_left.x >= 0 && control.bounds.top_left.y >= 0);
                    // Verify horizontal coordinate or current position along the row plus the
                    // numeric value converted to the required arithmetic or indexing type does not
                    // exceed 320. A violation means the tested behavior is incorrect.
                    assert!(control.bounds.top_left.x + control.bounds.size.width as i32 <= 320);
                    // Verify vertical coordinate or current screen row plus the numeric value
                    // converted to the required arithmetic or indexing type does not exceed 480. A
                    // violation means the tested behavior is incorrect.
                    assert!(control.bounds.top_left.y + control.bounds.size.height as i32 <= 480);
                    // Check whether enabled.
                    if control.enabled {
                        // Verify that hit result for the surrounding operation exactly matches an
                        // available value for action.
                        assert_eq!(
                            ui::hit(&app, control.bounds.center().x, control.bounds.center().y),
                            Some(control.action)
                        );
                    }
                }
            }
        }
    }
}
