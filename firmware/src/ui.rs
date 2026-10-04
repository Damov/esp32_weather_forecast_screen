// ============================================================================= //
// File          : ui.rs                                                         //
// License       : GPL-3.0-only                                                  //
// Created       : 2026-10-04                                                    //
// Last modified : 2026-10-04                                                    //
// Author        : Daniel Kharlamov                                              //
// Last editor   : Daniel Kharlamov <daniel.kharlamov@googlemail.com>            //
// ============================================================================= //
//                                                                               //
// Summary:                                                                      //
// --------                                                                      //
// Wi-Fi interface controls and shared application rendering.                    //
//                                                                               //
// Module:                                                                       //
// -------                                                                       //
// Builds hit-testable controls and maps user actions to application commands.   //
// Renders connection pages, password keyboards, startup and calibration         //
// prompts, and presence indicators. Delegates settings overlays and weather     //
// views to their renderers while preserving shared UI geometry.                 //
// ============================================================================= //

use crate::{
    model::{App, Command, Page},
    theme::{self, Backdrop, BODY, SMALL, TITLE},
};
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle, Rectangle},
};
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    CycleWeather,
    OpenSettings,
    CloseSettings,
    SettingsBack,
    Brightness,
    Night,
    WakeOnPresence,
    SleepTimeout,
    SelectSleepTimeout(u32),
    Units,
    Clock,
    Languages,
    Language(crate::i18n::Language),
    Location,
    LocationBack,
    CityCharacter(char),
    CityShift,
    CitySymbols,
    CityAccents,
    CityAlphabet,
    CitySpace,
    CityDelete,
    SearchCity,
    SelectLocation(usize),
    LocationsPrevious,
    LocationsNext,
    SaveLocation,
    ResetPrompt,
    ResetConfirm,
    Calibration,
    StartCalibration,
    Setup,
    Retry,
    Refresh,
    Back,
    Select(usize),
    Previous,
    Next,
    Connect,
    Show,
    Character(char),
    Shift,
    Symbols,
    Delete,
    Space,
}
pub struct Control {
    pub bounds: Rectangle,
    pub label: String,
    pub action: Action,
    pub enabled: bool,
}
fn control(
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    label: impl Into<String>,
    action: Action,
    enabled: bool,
) -> Control {
    Control {
        bounds: Rectangle::new(Point::new(x, y), Size::new(w, h)),
        label: label.into(),
        action,
        enabled,
    }
}
pub fn controls(app: &App) -> Vec<Control> {
    if app.preferences.overlay.is_some() {
        return crate::settings_ui::controls(app);
    }
    let mut out = vec![];
    match app.page {
        Page::Problem => {
            out.push(control(
                12,
                300,
                296,
                52,
                "Set up new Wi-Fi",
                Action::Setup,
                true,
            ));
            out.push(control(
                12,
                364,
                296,
                40,
                if app.saved_ssid.is_some() {
                    "Retry"
                } else {
                    "Scan again"
                },
                Action::Retry,
                true,
            ));
        }
        Page::Connected => {
            out.push(control(
                260,
                427,
                53,
                53,
                "Settings",
                Action::OpenSettings,
                true,
            ));
            out.push(control(0, 40, 320, 387, "", Action::CycleWeather, true));
        }
        Page::Networks => {
            for (row, n) in app.networks.iter().skip(app.offset).take(5).enumerate() {
                out.push(control(
                    12,
                    112 + row as i32 * 54,
                    296,
                    48,
                    format!(
                        "{}  {}",
                        clipped(
                            &n.ssid,
                            276 - BODY.width(&format!("  {}", n.security.label()))
                        ),
                        n.security.label()
                    ),
                    Action::Select(app.offset + row),
                    !app.scanning,
                ));
            }
            out.push(control(
                12,
                388,
                140,
                40,
                "Previous",
                Action::Previous,
                app.offset > 0 && !app.scanning,
            ));
            out.push(control(
                168,
                388,
                140,
                40,
                "Next",
                Action::Next,
                app.offset + 5 < app.networks.len() && !app.scanning,
            ));
            out.push(control(12, 438, 140, 40, "Back", Action::Back, true));
            out.push(control(
                168,
                438,
                140,
                40,
                "Refresh",
                Action::Refresh,
                !app.scanning,
            ));
        }
        Page::Password => {
            out.push(control(
                12,
                130,
                296,
                40,
                if app.show_password {
                    "Hide password"
                } else {
                    "Show password"
                },
                Action::Show,
                true,
            ));
            let rows: [&str; 4] = match app.keyboard {
                1 => ["QWERTYUIOP", "ASDFGHJKL", "ZXCVBNM", "0123456789"],
                2 => ["!@#$%^&*()", "-_=+[]{}", ";:'\"\\|,.", "<>/?`~"],
                _ => ["qwertyuiop", "asdfghjkl", "zxcvbnm", "0123456789"],
            };
            for (row, chars) in rows.iter().enumerate() {
                let start = (320 - chars.len() as i32 * 32) / 2;
                for (col, ch) in chars.chars().enumerate() {
                    out.push(control(
                        start + col as i32 * 32 + 1,
                        234 + row as i32 * 40,
                        30,
                        38,
                        ch.to_string(),
                        Action::Character(ch),
                        true,
                    ));
                }
            }
            for (i, label, action) in [
                (0, "Shift", Action::Shift),
                (1, "#+=", Action::Symbols),
                (2, "Space", Action::Space),
                (3, "Del", Action::Delete),
            ] {
                out.push(control(i * 80 + 1, 396, 78, 38, label, action, true));
            }
            out.push(control(12, 438, 140, 40, "Back", Action::Back, true));
            out.push(control(
                168,
                438,
                140,
                40,
                "Connect",
                Action::Connect,
                app.password.len() >= 8,
            ));
        }
        Page::Connecting => out.push(control(12, 390, 296, 52, "Cancel", Action::Back, true)),
        _ => {}
    }
    if app.page == Page::Problem {
        out.push(control(
            12,
            422,
            296,
            40,
            "Settings",
            Action::OpenSettings,
            true,
        ));
    }
    for button in &mut out {
        if !matches!(button.action, Action::Character(_)) {
            button.label =
                crate::i18n::tr(app.preferences.current.language, &button.label).to_owned();
        }
    }
    out
}
pub fn hit(app: &App, x: i32, y: i32) -> Option<Action> {
    controls(app)
        .into_iter()
        .find(|b| b.enabled && b.bounds.contains(Point::new(x, y)))
        .map(|b| b.action)
}
pub fn activate(app: &mut App, action: Action) -> Option<Command> {
    match action {
        Action::CycleWeather => {
            app.weather.view = app.weather.view.next();
            None
        }
        Action::Setup | Action::Refresh => Some(app.setup()),
        Action::Retry => {
            if app.saved_ssid.is_some() {
                Some(app.retry())
            } else {
                app.scanning = true;
                app.generation += 1;
                Some(Command::Scan(app.generation))
            }
        }
        Action::Back => Some(app.back()),
        Action::Connect => app.connect(),
        Action::Select(index) => {
            app.select(index);
            None
        }
        Action::Previous => {
            app.offset = app.offset.saturating_sub(5);
            None
        }
        Action::Next => {
            if app.offset + 5 < app.networks.len() {
                app.offset += 5;
            }
            None
        }
        Action::Show => {
            app.show_password = !app.show_password;
            None
        }
        Action::Character(ch) => {
            app.type_char(ch);
            None
        }
        Action::Space => {
            app.type_char(' ');
            None
        }
        Action::Delete => {
            app.password.pop();
            None
        }
        Action::Shift => {
            app.keyboard = if app.keyboard == 1 { 0 } else { 1 };
            None
        }
        Action::Symbols => {
            app.keyboard = if app.keyboard == 2 { 0 } else { 2 };
            None
        }
        _ => None,
    }
}
fn clipped(text: &str, width: u32) -> String {
    theme::clipped(text, &BODY, width)
}
fn wrapped<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    text: &str,
    y: i32,
    max_lines: usize,
) -> Result<(), D::Error> {
    for (i, line) in theme::wrap(text, &BODY, 288, max_lines).iter().enumerate() {
        BODY.draw(
            display,
            line,
            Point::new(16, y + i as i32 * BODY.height as i32),
            theme::TEXT,
            Backdrop::Gradient,
            288,
        )?;
    }
    Ok(())
}
pub fn render<D: DrawTarget<Color = Rgb565>>(display: &mut D, app: &App) -> Result<(), D::Error> {
    if app.preferences.overlay.is_some() {
        return crate::settings_ui::render(display, app);
    }
    if app.page == Page::Connected {
        return crate::weather_ui::render(display, app);
    }
    let lang = app.preferences.current.language;
    theme::background(display)?;
    let title = match app.page {
        Page::Networks => "Choose Wi-Fi",
        Page::Password => "Wi-Fi password",
        Page::Connected => "Connected",
        Page::Fatal => "Setup required",
        _ => "Wi-Fi setup",
    };
    TITLE.draw(
        display,
        crate::i18n::tr(lang, title),
        Point::new(16, 12),
        theme::TEXT,
        Backdrop::Gradient,
        if presence_visible(app) { 248 } else { 288 },
    )?;
    render_presence(display, app)?;
    match app.page {
        Page::Password => {
            if let Some(n) = &app.selected {
                wrapped(display, &n.ssid, 44, 2)?;
            }
            theme::rounded(
                display,
                Rectangle::new(Point::new(12, 86), Size::new(296, 34)),
                theme::PANEL,
            )?;
            // Measure borrowed suffixes; never copy an unmasked password into an ordinary String.
            let field = Zeroizing::new(if app.show_password {
                let start = app
                    .password
                    .char_indices()
                    .map(|(i, _)| i)
                    .find(|&i| BODY.width(&app.password[i..]) <= 280)
                    .unwrap_or(app.password.len());
                app.password[start..].to_owned()
            } else {
                let count = (0..=app.password.len())
                    .rev()
                    .find(|&n| BODY.width(&"*".repeat(n)) <= 280)
                    .unwrap_or(0);
                "*".repeat(count)
            });
            BODY.draw(
                display,
                &field,
                Point::new(20, 94),
                theme::TEXT,
                Backdrop::Solid(theme::PANEL),
                280,
            )?;
            wrapped(display, crate::i18n::tr(lang, &app.message), 180, 3)?;
        }
        Page::Networks => wrapped(display, crate::i18n::tr(lang, &app.message), 46, 3)?,
        Page::Problem | Page::Connecting | Page::Connected => {
            if let Some(ssid) = app
                .selected
                .as_ref()
                .map(|n| &n.ssid)
                .or(app.saved_ssid.as_ref())
            {
                wrapped(
                    display,
                    &format!("{}: {ssid}", crate::i18n::tr(lang, "Network")),
                    64,
                    3,
                )?;
            }
            wrapped(display, crate::i18n::tr(lang, &app.message), 154, 6)?;
            if app.scanning {
                SMALL.draw(
                    display,
                    crate::i18n::tr(lang, "Scanning..."),
                    Point::new(16, 280),
                    theme::SECONDARY,
                    Backdrop::Gradient,
                    288,
                )?;
            }
        }
        Page::Fatal => wrapped(
            display,
            crate::i18n::tr(lang, "Device error. Please restart."),
            76,
            12,
        )?,
        _ => wrapped(display, crate::i18n::tr(lang, &app.message), 76, 12)?,
    }
    for b in controls(app) {
        let fill = if b.enabled {
            theme::PANEL
        } else {
            theme::blend(theme::PANEL, theme::background_at(b.bounds.top_left.y), 9)
        };
        theme::rounded(display, b.bounds, fill)?;
        let text = clipped(&b.label, b.bounds.size.width.saturating_sub(10));
        let x = b.bounds.top_left.x + (b.bounds.size.width as i32 - BODY.width(&text) as i32) / 2;
        BODY.draw(
            display,
            &text,
            Point::new(
                x,
                b.bounds.top_left.y + (b.bounds.size.height as i32 - BODY.height as i32) / 2,
            ),
            if b.enabled { theme::TEXT } else { theme::MUTED },
            Backdrop::Solid(fill),
            b.bounds.size.width.saturating_sub(10),
        )?;
    }
    Ok(())
}

pub fn presence_visible(app: &App) -> bool {
    app.preferences.overlay == Some(crate::settings::Overlay::Settings)
        || (app.preferences.overlay.is_none()
            && matches!(app.page, Page::Problem | Page::Connecting | Page::Connected))
}

pub fn presence_bounds() -> Rectangle {
    Rectangle::new(Point::new(280, 12), Size::new(28, 25))
}

pub fn render_presence<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    app: &App,
) -> Result<(), D::Error> {
    if !presence_visible(app) {
        return Ok(());
    }
    let bounds = presence_bounds();
    for y in bounds.top_left.y..bounds.top_left.y + bounds.size.height as i32 {
        display.fill_solid(
            &Rectangle::new(
                Point::new(bounds.top_left.x, y),
                Size::new(bounds.size.width, 1),
            ),
            theme::background_at(y),
        )?;
    }
    let color = match app.radar {
        crate::radar::PresenceStatus::Present => theme::rgb(0x2ecc71),
        crate::radar::PresenceStatus::Absent => theme::rgb(0xe74c3c),
        crate::radar::PresenceStatus::Unavailable => theme::rgb(0xb9b9b9),
    };
    Circle::new(Point::new(288, 16), 16)
        .into_styled(PrimitiveStyle::with_fill(color))
        .draw(display)
}

/// Also used by host previews: styling never changes calibration target positions.
pub fn render_prompt<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    message: &str,
    target: Option<(i32, i32)>,
) -> Result<(), D::Error> {
    theme::background(display)?;
    let mut parts = message.splitn(2, '\n');
    let titles = theme::wrap(parts.next().unwrap_or_default(), &TITLE, 288, 2);
    for (i, title) in titles.iter().enumerate() {
        TITLE.draw(
            display,
            title,
            Point::new(16, 90 + i as i32 * TITLE.height as i32),
            theme::TEXT,
            Backdrop::Gradient,
            288,
        )?;
    }
    if let Some(body) = parts.next() {
        wrapped(
            display,
            body,
            100 + titles.len() as i32 * TITLE.height as i32,
            5,
        )?;
    }
    if let Some((x, y)) = target {
        for (a, b) in [((x - 10, y), (x + 10, y)), ((x, y - 10), (x, y + 10))] {
            Line::new(Point::new(a.0, a.1), Point::new(b.0, b.1))
                .into_styled(PrimitiveStyle::with_stroke(Rgb565::YELLOW, 2))
                .draw(display)?;
        }
    }
    Ok(())
}

///////////////////////////////////////////////////////////////////////////////////////
//                                    UNIT TESTS                                     //
///////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::mock_display::MockDisplay;
    #[test]
    fn controls_fit_portrait_and_hit_the_same_geometry() {
        let mut app = App::default();
        for page in [
            Page::Problem,
            Page::Networks,
            Page::Password,
            Page::Connecting,
            Page::Connected,
        ] {
            app.page = page;
            app.password.push_str("password");
            for b in controls(&app) {
                assert!(b.bounds.top_left.x >= 0 && b.bounds.top_left.y >= 0);
                assert!(b.bounds.top_left.x + b.bounds.size.width as i32 <= 320);
                assert!(b.bounds.top_left.y + b.bounds.size.height as i32 <= 480);
                if b.enabled {
                    assert_eq!(
                        hit(&app, b.bounds.top_left.x + 2, b.bounds.top_left.y + 2),
                        Some(b.action)
                    );
                }
            }
        }
    }
    #[test]
    fn weather_tap_cycle_and_connection_recovery_preserve_the_view() {
        use crate::{settings::Overlay, weather::View};
        let mut app = App {
            page: Page::Connected,
            ..App::default()
        };
        app.saved_ssid = Some("test".into());
        for view in [View::Week, View::Hours, View::Clock] {
            assert_eq!(hit(&app, 160, 200), Some(Action::CycleWeather));
            activate(&mut app, Action::CycleWeather);
            assert_eq!(app.weather.view, view);
        }
        assert_eq!(hit(&app, 20, 450), None);
        assert_eq!(hit(&app, 280, 450), Some(Action::OpenSettings));
        app.weather.view = View::Hours;
        app.preferences.overlay = Some(Overlay::Settings);
        assert_ne!(hit(&app, 160, 200), Some(Action::CycleWeather));
        crate::settings_ui::activate(&mut app, Action::CloseSettings, 0);
        assert_eq!(app.weather.view, View::Hours);
        let command = app.handle(crate::model::Event::Lost, 0);
        assert!(matches!(command, Some(Command::Retry(_))));
        assert_eq!(app.page, Page::Connecting);
        app.handle(
            crate::model::Event::Connected(app.generation, "test".into()),
            1,
        );
        assert_eq!(app.page, Page::Connected);
        assert_eq!(app.weather.view, View::Hours);
    }

    #[test]
    fn every_printable_password_character_is_reachable() {
        let mut app = App {
            page: Page::Password,
            ..App::default()
        };
        let mut keys = vec![' '];
        for mode in 0..3 {
            app.keyboard = mode;
            for c in controls(&app) {
                if let Action::Character(ch) = c.action {
                    keys.push(ch);
                }
            }
        }
        for byte in 32u8..=126 {
            assert!(
                keys.contains(&(byte as char)),
                "Missing character {}",
                byte as char
            );
        }
    }
    #[test]
    fn pages_render_without_failure() {
        let mut display = MockDisplay::new();
        display.set_allow_overdraw(true);
        display.set_allow_out_of_bounds_drawing(true);
        let mut app = App::default();
        for language in crate::i18n::Language::ALL {
            app.preferences.current.language = language;
            for page in [
                Page::Starting,
                Page::Problem,
                Page::Networks,
                Page::Password,
                Page::Connecting,
                Page::Connected,
                Page::Fatal,
            ] {
                app.page = page;
                render(&mut display, &app).unwrap();
            }
        }
    }
}
