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
    Wifi(Command),
    Persist(Settings),
    Search(location::Request),
}
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
        bounds: Rectangle::new(Point::new(x, y), Size::new(width, height)),
        label: label.into(),
        action,
        enabled,
    }
}
pub fn timeout_label(language: Language, minutes: u32) -> String {
    if minutes == 0 {
        tr(language, "Never").into()
    } else {
        format!("{minutes} {}", tr(language, "min"))
    }
}
pub fn controls(app: &App) -> Vec<Control> {
    let p = &app.preferences;
    let mut out = Vec::new();
    match p.overlay {
        Some(Overlay::Settings) => {
            out.push(button(
                136,
                72,
                172,
                40,
                "",
                Action::Brightness,
                !p.read_only,
            ));
            for (y, label, action) in [
                (116, "Night mode", Action::Night),
                (158, "Fahrenheit", Action::Units),
                (200, "24-hour time", Action::Clock),
                (242, "Wake on presence", Action::WakeOnPresence),
            ] {
                out.push(button(252, y, 56, 32, label, action, !p.read_only));
            }
            out.push(button(
                142,
                286,
                166,
                32,
                timeout_label(p.current.language, p.current.sleep_minutes),
                Action::SleepTimeout,
                !p.read_only,
            ));
            out.push(button(
                142,
                330,
                166,
                36,
                p.current.language.name(),
                Action::Languages,
                !p.read_only,
            ));
            out.push(button(
                12,
                370,
                140,
                32,
                "Change location",
                Action::Location,
                !p.read_only,
            ));
            out.push(button(
                168,
                370,
                140,
                32,
                "Reset Wi-Fi",
                Action::ResetPrompt,
                true,
            ));
            out.push(button(
                12,
                408,
                140,
                30,
                "Change Wi-Fi",
                Action::Setup,
                true,
            ));
            out.push(button(
                168,
                408,
                140,
                30,
                "Screen calibration",
                Action::Calibration,
                true,
            ));
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
        Some(Overlay::SleepTimeout) => {
            for (index, minutes) in crate::screen::TIMEOUTS.into_iter().enumerate() {
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
            out.push(button(12, 438, 296, 40, "Back", Action::SettingsBack, true));
        }
        Some(Overlay::Languages) => {
            for (i, language) in Language::ALL.into_iter().enumerate() {
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
            out.push(button(12, 438, 296, 40, "Back", Action::SettingsBack, true));
        }
        Some(Overlay::LocationInput) => {
            out.push(button(
                12,
                134,
                140,
                32,
                "Accents",
                Action::CityAccents,
                true,
            ));
            out.push(button(
                168,
                134,
                140,
                32,
                if p.city_cyrillic { "ABC" } else { "АБВ" },
                Action::CityAlphabet,
                true,
            ));
            let rows = match p.keyboard {
                4 if p.current.language == Language::Ukrainian => {
                    ["йцукенгшщзхї", "фівапролджє", "ячсмитьбюґ'", "0123456789"]
                }
                5 if p.current.language == Language::Ukrainian => {
                    ["ЙЦУКЕНГШЩЗХЇ", "ФІВАПРОЛДЖЄ", "ЯЧСМИТЬБЮҐ'", "0123456789"]
                }
                4 => ["йцукенгшщзхъ", "фывапролджэ", "ячсмитьбюё", "0123456789"],
                5 => ["ЙЦУКЕНГШЩЗХЪ", "ФЫВАПРОЛДЖЭ", "ЯЧСМИТЬБЮЁ", "0123456789"],
                1 => ["QWERTYUIOP", "ASDFGHJKL", "ZXCVBNM", "0123456789"],
                2 => ["!@#$%^&*()", "-_=+[]{}", ";:'\"\\|,.", "<>/?`~"],
                3 => ["äöüÄÖÜßåÅ", "éèêëàâçñ", "ìíòóùúœ", "ğĞşŞıİ"],
                _ => ["qwertyuiop", "asdfghjkl", "zxcvbnm", "0123456789"],
            };
            for (row, chars) in rows.into_iter().enumerate() {
                let count = chars.chars().count() as i32;
                let cell = 32.min(320 / count);
                let start = (320 - count * cell) / 2;
                for (col, ch) in chars.chars().enumerate() {
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
            for (i, label, action) in [
                (0, "Shift", Action::CityShift),
                (1, "#+=", Action::CitySymbols),
                (2, "Space", Action::CitySpace),
                (3, "Del", Action::CityDelete),
            ] {
                out.push(button(i * 80 + 1, 396, 78, 38, label, action, true));
            }
            out.push(button(
                12,
                438,
                140,
                40,
                "Cancel",
                Action::SettingsBack,
                true,
            ));
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
        Some(Overlay::LocationResults) => {
            for (row, location) in p.results.iter().skip(p.result_offset).take(5).enumerate() {
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
            out.push(button(
                12,
                388,
                140,
                40,
                "Previous",
                Action::LocationsPrevious,
                p.result_offset > 0 && !p.searching,
            ));
            out.push(button(
                168,
                388,
                140,
                40,
                "Next",
                Action::LocationsNext,
                p.result_offset + 5 < p.results.len() && !p.searching,
            ));
            out.push(button(12, 438, 140, 40, "Back", Action::LocationBack, true));
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
        Some(Overlay::ResetWifi) => {
            out.push(button(
                12,
                350,
                296,
                40,
                "Reset",
                Action::ResetConfirm,
                !p.reset_pending,
            ));
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
        Some(Overlay::CalibrationIntro | Overlay::CalibrationError) => {
            out.push(button(
                12,
                350,
                296,
                40,
                if p.overlay == Some(Overlay::CalibrationIntro) {
                    "Start"
                } else {
                    "Retry"
                },
                Action::StartCalibration,
                true,
            ));
            out.push(button(12, 438, 296, 40, "Back", Action::SettingsBack, true));
        }
        Some(Overlay::CalibrationRunning) | None => {}
    }
    out
}
fn persist(app: &mut App) -> Option<Effect> {
    if app.preferences.read_only {
        app.preferences.saved(false);
        return None;
    }
    Some(Effect::Persist(app.preferences.current.clone()))
}
pub fn activate(app: &mut App, action: Action, now: u64) -> Option<Effect> {
    match action {
        Action::OpenSettings => {
            app.preferences.leave(Some(Overlay::Settings));
        }
        Action::CloseSettings => app.preferences.leave(None),
        Action::SettingsBack => app.preferences.leave(Some(Overlay::Settings)),
        Action::Night => {
            app.preferences.current.night_mode = !app.preferences.current.night_mode;
            return persist(app);
        }
        Action::WakeOnPresence => {
            app.preferences.current.wake_on_presence = !app.preferences.current.wake_on_presence;
            return persist(app);
        }
        Action::Units => {
            app.preferences.current.fahrenheit = !app.preferences.current.fahrenheit;
            return persist(app);
        }
        Action::Clock => {
            app.preferences.current.clock_24h = !app.preferences.current.clock_24h;
            return persist(app);
        }
        Action::SleepTimeout => app.preferences.leave(Some(Overlay::SleepTimeout)),
        Action::SelectSleepTimeout(minutes) => {
            if crate::screen::TIMEOUTS.contains(&minutes) {
                app.preferences.current.sleep_minutes = minutes;
                app.preferences.after_save = Some(Overlay::Settings);
                return persist(app);
            }
        }
        Action::Languages => app.preferences.leave(Some(Overlay::Languages)),
        Action::Language(language) => {
            app.preferences.current.language = language;
            app.preferences.after_save = Some(Overlay::Settings);
            return persist(app);
        }
        Action::Location => {
            app.preferences.leave(Some(Overlay::LocationInput));
            app.preferences.query.clear();
            app.preferences.city_cyrillic = matches!(
                app.preferences.current.language,
                Language::Russian | Language::Ukrainian
            );
            app.preferences.keyboard = if app.preferences.city_cyrillic { 4 } else { 0 };
        }
        Action::LocationBack => app.preferences.leave(Some(Overlay::LocationInput)),
        Action::CityCharacter(ch) => {
            if app.preferences.query.chars().count() < 64 && !ch.is_control() {
                app.preferences.query.push(ch);
            }
        }
        Action::CitySpace => {
            if app.preferences.query.chars().count() < 64 {
                app.preferences.query.push(' ');
            }
        }
        Action::CityDelete => {
            app.preferences.query.pop();
        }
        Action::CityShift => {
            let p = &mut app.preferences;
            let lower = if p.city_cyrillic { 4 } else { 0 };
            let upper = if p.city_cyrillic { 5 } else { 1 };
            p.keyboard = if p.keyboard == upper { lower } else { upper };
        }
        Action::CitySymbols => {
            let p = &mut app.preferences;
            p.keyboard = if p.keyboard == 2 {
                if p.city_cyrillic {
                    4
                } else {
                    0
                }
            } else {
                2
            };
        }
        Action::CityAccents => {
            let p = &mut app.preferences;
            p.keyboard = if p.keyboard == 3 {
                if p.city_cyrillic {
                    4
                } else {
                    0
                }
            } else {
                3
            };
        }
        Action::CityAlphabet => {
            let p = &mut app.preferences;
            p.city_cyrillic = !p.city_cyrillic;
            p.keyboard = if p.city_cyrillic { 4 } else { 0 };
        }
        Action::SearchCity => {
            if app.preferences.query.trim().chars().count() < 2 {
                app.preferences.status = "Use at least two characters.";
                return None;
            }
            let p = &mut app.preferences;
            p.leave(Some(Overlay::LocationResults));
            p.results.clear();
            p.result_selected = None;
            p.result_offset = 0;
            p.searching = true;
            p.search_deadline = now + 10_000;
            p.status = "Searching...";
            return Some(Effect::Search(location::Request::Search {
                id: p.request_id,
                query: p.query.clone(),
                language: p.current.language,
            }));
        }
        Action::SelectLocation(index) => {
            if index < app.preferences.results.len() {
                app.preferences.result_selected = Some(index);
            }
        }
        Action::LocationsPrevious => {
            app.preferences.result_offset = app.preferences.result_offset.saturating_sub(5);
        }
        Action::LocationsNext => {
            if app.preferences.result_offset + 5 < app.preferences.results.len() {
                app.preferences.result_offset += 5;
            }
        }
        Action::SaveLocation => {
            let p = &mut app.preferences;
            p.current.location = p.results.get(p.result_selected?)?.clone();
            p.after_save = Some(Overlay::Settings);
            return persist(app);
        }
        Action::ResetPrompt => app.preferences.leave(Some(Overlay::ResetWifi)),
        Action::ResetConfirm => {
            let p = &mut app.preferences;
            p.reset_pending = true;
            p.status = "Resetting Wi-Fi...";
            app.generation += 1;
            app.retry_at = None;
            return Some(Effect::Wifi(Command::Reset(app.generation)));
        }
        Action::Calibration => app.preferences.leave(Some(Overlay::CalibrationIntro)),
        Action::StartCalibration => {
            app.preferences.wizard = Some(Wizard::new(now));
            app.preferences.leave(Some(Overlay::CalibrationRunning));
        }
        Action::Brightness => {}
        Action::Setup => {
            app.preferences.leave(None);
            return ui::activate(app, Action::Setup).map(Effect::Wifi);
        }
        other => return ui::activate(app, other).map(Effect::Wifi),
    }
    None
}
pub fn slider(app: &mut App, x: i32, released: bool) -> Option<Effect> {
    let p = &mut app.preferences;
    if p.overlay != Some(Overlay::Settings) || p.read_only {
        return None;
    }
    if released {
        p.slider_active = false;
        if p.current != p.saved {
            return persist(app);
        }
    } else {
        p.slider_active = true;
        p.current.brightness = (1 + ((x - 144).clamp(0, 156) * 254 + 78) / 156) as u8;
    }
    None
}
fn text<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    app: &App,
    text: &str,
    x: i32,
    y: i32,
    small: bool,
    width: u32,
) -> Result<(), D::Error> {
    let font = if small { &SMALL } else { &BODY };
    let text = theme::clipped(tr(app.preferences.current.language, text), font, width);
    font.draw(
        display,
        &text,
        Point::new(x, y),
        theme::TEXT,
        Backdrop::Gradient,
        width,
    )
}
fn message<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    app: &App,
    value: &str,
    y: i32,
    lines: usize,
) -> Result<(), D::Error> {
    for (i, line) in theme::wrap(
        tr(app.preferences.current.language, value),
        &BODY,
        288,
        lines,
    )
    .iter()
    .enumerate()
    {
        BODY.draw(
            display,
            line,
            Point::new(16, y + i as i32 * 18),
            theme::TEXT,
            Backdrop::Gradient,
            288,
        )?;
    }
    Ok(())
}
pub fn render<D: DrawTarget<Color = Rgb565>>(display: &mut D, app: &App) -> Result<(), D::Error> {
    let p = &app.preferences;
    let lang = p.current.language;
    if p.overlay == Some(Overlay::CalibrationRunning) {
        if let Some(wizard) = &p.wizard {
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
            return ui::render_prompt(
                display,
                &format!("{}\n{}", tr(lang, title), tr(lang, body)),
                Some(wizard.target()),
            );
        }
    }
    theme::background(display)?;
    let title = match p.overlay {
        Some(Overlay::Languages) => "Choose a language",
        Some(Overlay::SleepTimeout) => "Screen off after",
        Some(Overlay::LocationInput) => "Change location",
        Some(Overlay::LocationResults) => "Search results",
        Some(Overlay::ResetWifi) => "Reset Wi-Fi",
        Some(Overlay::CalibrationIntro | Overlay::CalibrationError) => "Screen calibration",
        _ => "Settings",
    };
    let title = theme::clipped(tr(lang, title), &TITLE, 288);
    TITLE.draw(
        display,
        &title,
        Point::new(16, 12),
        theme::TEXT,
        Backdrop::Gradient,
        288,
    )?;
    match p.overlay {
        Some(Overlay::Settings) => {
            if p.status.is_empty() {
                text(display, app, &p.current.clock(p.epoch).unwrap_or_else(|| tr(lang, "Waiting for time synchronization.").into()), 16, 44, true, 288)?;
            } else {
                for (i, line) in theme::wrap(tr(lang, p.status), &SMALL, 288, 2).iter().enumerate() {
                    SMALL.draw(display, line, Point::new(16, 40 + i as i32 * 15), theme::TEXT, Backdrop::Gradient, 288)?;
                }
            }
            for (label, y) in [("Brightness", 84), ("Night mode", 124), ("Fahrenheit", 166), ("24-hour time", 208), ("Wake on presence", 250), ("Screen off after", 294), ("Language", 342)] {
                if label == "Wake on presence" || label == "Screen off after" {
                    for (index, line) in theme::wrap(tr(lang, label), &SMALL, if label == "Screen off after" { 120 } else { 224 }, 2).iter().enumerate() {
                        SMALL.draw(display, line, Point::new(16, if label == "Screen off after" { 284 } else { 242 } + index as i32 * 16), theme::TEXT, Backdrop::Gradient, 224)?;
                    }
                } else {
                    text(display, app, label, 16, y, true, if label == "Location" { 288 } else { 124 })?;
                }
            }

        }
        Some(Overlay::LocationInput) => {
            theme::rounded(display, Rectangle::new(Point::new(12, 92), Size::new(296, 34)), theme::PANEL)?;
            let start = p.query.char_indices().map(|(i, _)| i).find(|&i| BODY.width(&p.query[i..]) <= 280).unwrap_or(p.query.len());
            BODY.draw(display, &p.query[start..], Point::new(20, 100), theme::TEXT, Backdrop::Solid(theme::PANEL), 280)?;
            for (i, line) in theme::wrap(&p.current.location.name, &BODY, 288, 2).iter().enumerate() {
                BODY.draw(display, line, Point::new(16, 44 + i as i32 * 19), theme::TEXT, Backdrop::Gradient, 288)?;
            }
            if p.query.is_empty() {
                text(display, app, "Enter a city or postal code.", 20, 100, true, 280)?;
            }
            message(display, app, p.status, 176, 2)?;
            text(display, app, "Open-Meteo / GeoNames", 16, 216, true, 288)?;
        }
        Some(Overlay::LocationResults) => {
            if !p.status.is_empty() && p.status != "Select a location." { message(display, app, p.status, 46, 2)?; }
            else if let Some(index) = p.result_selected { message(display, app, &p.results[index].name, 46, 2)?; }
            else { message(display, app, "Select a location.", 46, 2)?; }
            text(display, app, "Open-Meteo / GeoNames", 16, 90, true, 288)?;
        }
        Some(Overlay::SleepTimeout) => message(display, app, p.status, 370, 3)?,
        Some(Overlay::ResetWifi) => {
            message(display, app, "Forget the saved Wi-Fi network? Settings and calibration will be kept.", 76, 10)?;
            message(display, app, p.status, 272, 4)?;
        }
        Some(Overlay::CalibrationIntro) => message(display, app, "Tap four corner targets, then the center. Lift the stylus between targets. Press BOOT to cancel.", 76, 12)?,
        Some(Overlay::CalibrationError) => message(display, app, p.status, 76, 12)?,
        _ => {}
    }
    crate::ui::render_presence(display, app)?;
    for button in controls(app) {
        if button.action == Action::Brightness {
            theme::rounded(
                display,
                Rectangle::new(Point::new(144, 89), Size::new(156, 6)),
                theme::MUTED,
            )?;
            let x = 144 + ((p.current.brightness as i32 - 1) * 156 / 254);
            Circle::new(Point::new(x - 8, 84), 16)
                .into_styled(PrimitiveStyle::with_fill(theme::TEXT))
                .draw(display)?;
            continue;
        }
        let switch = match button.action {
            Action::Night => Some(p.current.night_mode),
            Action::Units => Some(p.current.fahrenheit),
            Action::Clock => Some(p.current.clock_24h),
            Action::WakeOnPresence => Some(p.current.wake_on_presence),
            _ => None,
        };
        if let Some(on) = switch {
            let fill = if on {
                theme::rgb(0x2878ae)
            } else {
                theme::PANEL
            };
            theme::rounded(display, button.bounds, fill)?;
            let x = button.bounds.top_left.x + if on { 30 } else { 4 };
            Circle::new(Point::new(x, button.bounds.top_left.y + 6), 20)
                .into_styled(PrimitiveStyle::with_fill(theme::TEXT))
                .draw(display)?;
            continue;
        }
        let selected = matches!(button.action, Action::Language(language) if language == lang)
            || matches!(button.action, Action::SelectLocation(index) if Some(index) == p.result_selected)
            || matches!(button.action, Action::SelectSleepTimeout(minutes) if minutes == p.current.sleep_minutes);
        let fill = if matches!(button.action, Action::ResetPrompt | Action::ResetConfirm) {
            theme::rgb(0xc03939)
        } else if selected {
            theme::rgb(0x2878ae)
        } else if button.enabled {
            theme::PANEL
        } else {
            theme::blend(
                theme::PANEL,
                theme::background_at(button.bounds.top_left.y),
                9,
            )
        };
        theme::rounded(display, button.bounds, fill)?;
        let label = if matches!(
            button.action,
            Action::SelectLocation(_)
                | Action::Language(_)
                | Action::Languages
                | Action::CityCharacter(_)
        ) {
            button.label.as_str()
        } else {
            tr(lang, &button.label)
        };
        if matches!(button.action, Action::Setup | Action::Calibration)
            && p.overlay == Some(Overlay::Settings)
        {
            let lines = theme::wrap(label, &SMALL, 130, 2);
            for (index, line) in lines.iter().enumerate() {
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
            continue;
        }
        let label = theme::clipped(label, &SMALL, button.bounds.size.width.saturating_sub(10));
        SMALL.draw(
            display,
            &label,
            Point::new(
                button.bounds.top_left.x
                    + (button.bounds.size.width as i32 - SMALL.width(&label) as i32) / 2,
                button.bounds.top_left.y
                    + (button.bounds.size.height as i32 - SMALL.height as i32) / 2,
            ),
            if button.enabled {
                theme::TEXT
            } else {
                theme::MUTED
            },
            Backdrop::Solid(fill),
            button.bounds.size.width.saturating_sub(10),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Event, Page};

    #[test]
    fn ukrainian_selection_keyboard_and_search() {
        let mut app = App::default();
        app.preferences.overlay = Some(Overlay::Languages);
        let button = controls(&app)
            .into_iter()
            .find(|b| b.action == Action::Language(Language::Ukrainian))
            .unwrap();
        assert_eq!(
            ui::hit(&app, button.bounds.center().x, button.bounds.center().y),
            Some(button.action)
        );
        let Some(Effect::Persist(settings)) = activate(&mut app, button.action, 0) else {
            panic!("expected persistence")
        };
        assert_eq!(settings.language, Language::Ukrainian);
        app.preferences.saved(false);
        assert_eq!(app.preferences.current.language, Language::English);
        activate(&mut app, button.action, 0);
        app.preferences.saved(true);
        assert_eq!(
            crate::settings::Settings::decode(&app.preferences.saved.encode().unwrap())
                .unwrap()
                .language,
            Language::Ukrainian
        );
        assert_eq!(timeout_label(Language::Ukrainian, 5), "5 хв");
        activate(&mut app, Action::Location, 0);
        assert!(app.preferences.city_cyrillic);
        assert_eq!(app.preferences.keyboard, 4);
        for (mode, alphabet) in [
            (4, "абвгґдеєжзиіїйклмнопрстуфхцчшщьюя'"),
            (5, "АБВГҐДЕЄЖЗИІЇЙКЛМНОПРСТУФХЦЧШЩЬЮЯ'"),
        ] {
            app.preferences.keyboard = mode;
            let buttons = controls(&app);
            for ch in alphabet.chars() {
                let button = buttons
                    .iter()
                    .find(|b| b.action == Action::CityCharacter(ch))
                    .unwrap();
                assert_eq!(
                    ui::hit(&app, button.bounds.center().x, button.bounds.center().y),
                    Some(button.action)
                );
            }
            for ch in "ыъэёЫЪЭЁ".chars() {
                assert!(!buttons
                    .iter()
                    .any(|b| b.action == Action::CityCharacter(ch)));
            }
            for button in buttons {
                assert!(button.bounds.top_left.x >= 0);
                assert!(button.bounds.top_left.x + button.bounds.size.width as i32 <= 320);
            }
        }
        app.preferences.keyboard = 4;
        activate(&mut app, Action::CityShift, 0);
        assert_eq!(app.preferences.keyboard, 5);
        activate(&mut app, Action::CityShift, 0);
        assert_eq!(app.preferences.keyboard, 4);
        for action in [Action::CitySymbols, Action::CityAccents] {
            activate(&mut app, action, 0);
            activate(&mut app, action, 0);
            assert_eq!(app.preferences.keyboard, 4);
        }
        for ch in "Київ Ґ Є І Ї'".chars() {
            activate(&mut app, Action::CityCharacter(ch), 0);
        }
        activate(&mut app, Action::CityAlphabet, 0);
        assert_eq!(app.preferences.keyboard, 0);
        assert_eq!(app.preferences.query, "Київ Ґ Є І Ї'");
        activate(&mut app, Action::CityAlphabet, 0);
        assert_eq!(app.preferences.keyboard, 4);
        activate(&mut app, Action::CityDelete, 0);
        assert_eq!(app.preferences.query, "Київ Ґ Є І Ї");
        app.preferences.query = "Київ".into();
        let Some(Effect::Search(location::Request::Search {
            query, language, ..
        })) = activate(&mut app, Action::SearchCity, 0)
        else {
            panic!("expected search")
        };
        assert_eq!(language, Language::Ukrainian);
        assert!(location::url(&query, language).ends_with("&language=uk"));
        assert!(location::url(&query, language).contains("%D0%9A%D0%B8%D1%97%D0%B2"));
    }
    #[test]
    fn russian_language_selection_persists_and_rolls_back() {
        let mut app = App::default();
        app.preferences.overlay = Some(Overlay::Languages);
        let button = controls(&app)
            .into_iter()
            .find(|b| b.action == Action::Language(Language::Russian))
            .unwrap();
        assert_eq!(
            ui::hit(&app, button.bounds.center().x, button.bounds.center().y),
            Some(button.action)
        );
        let Some(Effect::Persist(settings)) = activate(&mut app, button.action, 0) else {
            panic!("language must be persisted")
        };
        assert_eq!(settings.language, Language::Russian);
        assert_eq!(timeout_label(Language::Russian, 5), "5 мин");
        app.preferences.saved(false);
        assert_eq!(app.preferences.current.language, Language::English);
        activate(&mut app, button.action, 0);
        app.preferences.saved(true);
        let restored =
            crate::settings::Settings::decode(&app.preferences.saved.encode().unwrap()).unwrap();
        assert_eq!(
            crate::settings::Preferences::new(restored, false)
                .current
                .language,
            Language::Russian
        );
    }

    #[test]
    fn russian_city_keyboard_covers_alphabet_modes_and_unicode_limits() {
        let mut app = App::default();
        app.preferences.current.language = Language::Russian;
        activate(&mut app, Action::Location, 0);
        assert!(app.preferences.city_cyrillic);
        assert_eq!(app.preferences.keyboard, 4);
        for (mode, alphabet) in [
            (4, "абвгдеёжзийклмнопрстуфхцчшщъыьэюя"),
            (5, "АБВГДЕЁЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЫЬЭЮЯ"),
        ] {
            app.preferences.keyboard = mode;
            let buttons = controls(&app);
            for ch in alphabet.chars() {
                let button = buttons
                    .iter()
                    .find(|b| b.action == Action::CityCharacter(ch))
                    .unwrap();
                assert_eq!(
                    ui::hit(&app, button.bounds.center().x, button.bounds.center().y),
                    Some(Action::CityCharacter(ch))
                );
            }
            for button in buttons {
                assert!(button.bounds.top_left.x >= 0);
                assert!(button.bounds.top_left.x + button.bounds.size.width as i32 <= 320);
            }
        }
        app.preferences.keyboard = 4;
        activate(&mut app, Action::CityShift, 0);
        assert_eq!(app.preferences.keyboard, 5);
        activate(&mut app, Action::CityShift, 0);
        assert_eq!(app.preferences.keyboard, 4);
        for action in [Action::CitySymbols, Action::CityAccents] {
            activate(&mut app, action, 0);
            activate(&mut app, action, 0);
            assert_eq!(app.preferences.keyboard, 4);
        }
        app.preferences.query = "Москва Ё".into();
        activate(&mut app, Action::CityAlphabet, 0);
        assert_eq!(app.preferences.keyboard, 0);
        assert_eq!(app.preferences.query, "Москва Ё");
        for action in [Action::CitySymbols, Action::CityAccents] {
            activate(&mut app, action, 0);
            activate(&mut app, action, 0);
            assert_eq!(app.preferences.keyboard, 0);
        }
        activate(&mut app, Action::CityAlphabet, 0);
        assert_eq!(app.preferences.keyboard, 4);
        activate(&mut app, Action::CityDelete, 0);
        assert_eq!(app.preferences.query, "Москва ");
        app.preferences.query = "ё".repeat(63);
        activate(&mut app, Action::CityCharacter('Ё'), 0);
        activate(&mut app, Action::CityCharacter('Я'), 0);
        assert_eq!(app.preferences.query.chars().count(), 64);
        activate(&mut app, Action::CityDelete, 0);
        assert_eq!(app.preferences.query, "ё".repeat(63));
        app.preferences.query = "Москва".into();
        let Some(Effect::Search(location::Request::Search {
            query, language, ..
        })) = activate(&mut app, Action::SearchCity, 0)
        else {
            panic!("expected search")
        };
        assert_eq!(language, Language::Russian);
        assert_eq!(query, "Москва");
    }

    #[test]
    fn gear_footer_and_settings_keep_wifi_and_calibration_accessible() {
        for language in Language::ALL {
            let mut app = App {
                page: Page::Connected,
                ..App::default()
            };
            app.preferences.current.language = language;
            for view in [
                crate::weather::View::Clock,
                crate::weather::View::Week,
                crate::weather::View::Hours,
            ] {
                app.weather.view = view;
                assert_eq!(ui::hit(&app, 288, 458), Some(Action::OpenSettings));
                assert_eq!(ui::hit(&app, 20, 458), None);
                assert_eq!(ui::hit(&app, 160, 300), Some(Action::CycleWeather));
            }
            activate(&mut app, Action::OpenSettings, 0);
            let actions = controls(&app);
            let wifi = actions
                .iter()
                .find(|b| b.bounds.contains(Point::new(80, 423)))
                .unwrap();
            let calibration = actions
                .iter()
                .find(|b| b.bounds.contains(Point::new(238, 423)))
                .unwrap();
            assert_eq!(wifi.action, Action::Setup);
            assert_eq!(calibration.action, Action::Calibration);
            assert!(actions.iter().any(|b| b.action == Action::ResetPrompt));
            assert!(actions.iter().any(|b| b.action == Action::CloseSettings));
            activate(&mut app, calibration.action.clone(), 1);
            assert_eq!(app.preferences.overlay, Some(Overlay::CalibrationIntro));
            activate(&mut app, Action::SettingsBack, 2);
            assert!(matches!(
                activate(&mut app, wifi.action.clone(), 3),
                Some(Effect::Wifi(_))
            ));
            assert!(app.preferences.overlay.is_none());
            assert_ne!(app.page, Page::Connected);
        }
    }
    #[test]
    fn timeout_selection_saves_and_rolls_back_without_hiding_saved_location() {
        let mut app = App::default();
        activate(&mut app, Action::OpenSettings, 0);
        activate(&mut app, Action::SleepTimeout, 1);
        assert_eq!(app.preferences.overlay, Some(Overlay::SleepTimeout));
        assert!(matches!(
            activate(&mut app, Action::SelectSleepTimeout(10), 2),
            Some(Effect::Persist(_))
        ));
        app.preferences.saved(false);
        assert_eq!(app.preferences.current.sleep_minutes, 5);
        activate(&mut app, Action::SleepTimeout, 3);
        activate(&mut app, Action::SelectSleepTimeout(0), 4);
        app.preferences.saved(true);
        assert_eq!(app.preferences.saved.sleep_minutes, 0);
        assert!(activate(&mut app, Action::SelectSleepTimeout(2), 5).is_none());
        app.preferences.read_only = true;
        assert!(activate(&mut app, Action::SelectSleepTimeout(30), 6).is_none());
        activate(&mut app, Action::Location, 7);
        assert_eq!(app.preferences.overlay, Some(Overlay::LocationInput));
        assert!(!app.preferences.current.location.name.is_empty());
    }
    #[test]
    fn presence_switch_saves_immediately_and_rolls_back_on_failure() {
        let mut app = App::default();
        activate(&mut app, Action::OpenSettings, 0);
        assert!(controls(&app)
            .iter()
            .any(|button| button.action == Action::WakeOnPresence && button.enabled));
        let Some(Effect::Persist(settings)) = activate(&mut app, Action::WakeOnPresence, 0) else {
            panic!("missing save");
        };
        assert!(!settings.wake_on_presence);
        app.preferences.saved(false);
        assert!(app.preferences.current.wake_on_presence);
        activate(&mut app, Action::WakeOnPresence, 1);
        app.preferences.saved(true);
        assert!(!app.preferences.saved.wake_on_presence);
        app.preferences.read_only = true;
        assert!(
            !controls(&app)
                .iter()
                .find(|button| button.action == Action::WakeOnPresence)
                .unwrap()
                .enabled
        );
        assert!(activate(&mut app, Action::WakeOnPresence, 2).is_none());
        assert!(!app.preferences.current.wake_on_presence);
    }
    #[test]
    fn overlays_survive_wifi_events_and_close_to_latest_page() {
        let mut app = App::default();
        app.handle(Event::Ready(Some("Home".into())), 0);
        activate(&mut app, Action::OpenSettings, 0);
        app.handle(Event::Connected(app.generation, "Home".into()), 1);
        assert_eq!(app.preferences.overlay, Some(Overlay::Settings));
        app.handle(Event::Lost, 2);
        assert_eq!(app.preferences.overlay, Some(Overlay::Settings));
        activate(&mut app, Action::CloseSettings, 2000);
        assert_eq!(app.page, Page::Connecting);
        assert!(app.preferences.overlay.is_none());
    }
    #[test]
    fn slider_previews_and_saves_only_on_release_and_failure_rolls_back() {
        let mut app = App::default();
        activate(&mut app, Action::OpenSettings, 0);
        assert!(slider(&mut app, 144, false).is_none());
        assert_eq!(app.preferences.current.brightness, 1);
        assert_eq!(app.preferences.saved.brightness, 255);
        assert!(slider(&mut app, 222, false).is_none());
        assert_eq!(app.preferences.current.brightness, 128);
        assert!(matches!(
            slider(&mut app, 0, true),
            Some(Effect::Persist(_))
        ));
        app.preferences.saved(false);
        assert_eq!(app.preferences.current.brightness, 255);
        app.preferences.read_only = true;
        assert!(slider(&mut app, 144, false).is_none());
        assert_eq!(app.preferences.current.brightness, 255);
    }
    #[test]
    fn reset_requires_confirmation_and_calibration_requires_start() {
        let mut app = App {
            saved_ssid: Some("Home".into()),
            ..App::default()
        };
        activate(&mut app, Action::ResetPrompt, 0);
        activate(&mut app, Action::SettingsBack, 0);
        assert_eq!(app.saved_ssid.as_deref(), Some("Home"));
        activate(&mut app, Action::Calibration, 0);
        assert!(app.preferences.wizard.is_none());
        activate(&mut app, Action::StartCalibration, 100);
        assert_eq!(app.preferences.overlay, Some(Overlay::CalibrationRunning));
        assert!(app.preferences.wizard.is_some());
        activate(&mut app, Action::ResetPrompt, 0);
        assert!(matches!(
            activate(&mut app, Action::ResetConfirm, 0),
            Some(Effect::Wifi(Command::Reset(_)))
        ));
    }
    #[test]
    fn every_locale_and_overlay_has_valid_controls() {
        let mut app = App::default();
        for language in Language::ALL {
            app.preferences.current.language = language;
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
                app.preferences.overlay = Some(overlay);
                for control in controls(&app) {
                    assert!(control.bounds.top_left.x >= 0 && control.bounds.top_left.y >= 0);
                    assert!(control.bounds.top_left.x + control.bounds.size.width as i32 <= 320);
                    assert!(control.bounds.top_left.y + control.bounds.size.height as i32 <= 480);
                    if control.enabled {
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
