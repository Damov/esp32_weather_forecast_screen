// ============================================================================= //
// File          : weather_ui.rs                                                 //
// License       : GPL-3.0-only                                                  //
// Created       : 2026-10-04                                                    //
// Last modified : 2026-10-04                                                    //
// Author        : Daniel Kharlamov                                              //
// Last editor   : Daniel Kharlamov <daniel.kharlamov@googlemail.com>            //
// ============================================================================= //
//                                                                               //
// Summary:                                                                      //
// --------                                                                      //
// Clock, weekly, and hourly weather views for the portrait UI.                  //
//                                                                               //
// Module:                                                                       //
// -------                                                                       //
// Renders localized weather cards, analog and digital clocks, forecast tables,  //
// presence, attribution, and the settings gear. Uses buffered region            //
// comparisons to transmit changed pixels and recover after display errors.      //
// Shares rendering between device and host previews; embedded assets retain     //
// their own licences.                                                           //
// ============================================================================= //

//! Three portrait weather views, shared by the board and host previews.
use crate::{
    i18n::{self, Language},
    model::App,
    theme::{
        self, Backdrop, Font, BODY, CLOCK, FORECAST_BODY, FORECAST_SMALL, FORECAST_TITLE, SMALL,
        TITLE,
    },
    weather::{self, View},
    weather_icons,
};
use chrono::{Datelike, NaiveDate, Timelike};
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle, Rectangle},
};

pub fn clock_bounds() -> Rectangle {
    Rectangle::new(Point::new(12, 54), Size::new(296, 262))
}
/// Compose old/new strips in RAM and transfer only pixels whose final color changes.
pub struct ClockBuffer {
    pixels: Vec<Rgb565>,
    old_pixels: Vec<Rgb565>,
    previous: Option<(ClockContent, Rectangle)>,
    previous_weather: Option<(WeatherContent, Rectangle)>,
    previous_overview: Option<(OverviewContent, Rectangle)>,
}
impl ClockBuffer {
    const ROWS: u32 = 16;

    pub fn new() -> Result<Self, std::collections::TryReserveError> {
        fn allocate() -> Result<Vec<Rgb565>, std::collections::TryReserveError> {
            let count = clock_bounds().size.width as usize * ClockBuffer::ROWS as usize;
            let mut pixels = Vec::new();
            pixels.try_reserve_exact(count)?;
            pixels.resize(count, Rgb565::BLACK);
            Ok(pixels)
        }
        Ok(Self {
            pixels: allocate()?,
            old_pixels: allocate()?,
            previous: None,
            previous_weather: None,
            previous_overview: None,
        })
    }

    pub fn invalidate(&mut self) {
        self.previous = None;
        self.previous_weather = None;
        self.previous_overview = None;
    }

    /// Call only after a successful complete weather-page draw.
    pub fn remember(&mut self, app: &App) {
        self.invalidate();
        if app.weather.view != View::Clock {
            self.previous_overview = Some((OverviewContent::from_app(app), overview_bounds()));
            return;
        }
        self.previous = Some((ClockContent::from_app(app), clock_bounds()));
        self.previous_weather = Some((
            WeatherContent::from_app(app),
            Rectangle::new(Point::zero(), Size::new(320, 480)),
        ));
    }

    pub fn render<D: DrawTarget<Color = Rgb565>>(
        &mut self,
        display: &mut D,
        app: &App,
    ) -> Result<(), D::Error> {
        let bounds = clock_bounds().intersection(&display.bounding_box());
        let content = ClockContent::from_app(app);
        let previous = self.previous.take().filter(|(_, area)| *area == bounds);
        if previous.as_ref().is_some_and(|(old, _)| *old == content) {
            self.previous = previous;
            return Ok(());
        }
        render_region(
            display,
            bounds,
            &content,
            previous.as_ref().map(|(old, _)| old),
            &mut self.pixels,
            &mut self.old_pixels,
        )?;
        self.previous = Some((content, bounds));
        Ok(())
    }
    pub fn render_weather<D: DrawTarget<Color = Rgb565>>(
        &mut self,
        display: &mut D,
        app: &App,
    ) -> Result<(), D::Error> {
        if app.weather.view != View::Clock {
            let bounds = overview_bounds().intersection(&display.bounding_box());
            let content = OverviewContent::from_app(app);
            let previous = self
                .previous_overview
                .take()
                .filter(|(_, area)| *area == bounds);
            if previous.as_ref().is_some_and(|(old, _)| *old == content) {
                self.previous_overview = previous;
                return Ok(());
            }
            render_region(
                display,
                bounds,
                &content,
                previous.as_ref().map(|(old, _)| old),
                &mut self.pixels,
                &mut self.old_pixels,
            )?;
            self.previous_overview = Some((content, bounds));
            return Ok(());
        }
        let bounds = display.bounding_box();
        let content = WeatherContent::from_app(app);
        let previous = self
            .previous_weather
            .take()
            .filter(|(_, area)| *area == bounds);
        if previous.as_ref().is_some_and(|(old, _)| *old == content) {
            self.previous_weather = previous;
            return Ok(());
        }
        for area in weather_bounds() {
            render_region(
                display,
                area.intersection(&bounds),
                &content,
                previous.as_ref().map(|(old, _)| old),
                &mut self.pixels,
                &mut self.old_pixels,
            )?;
        }
        self.previous_weather = Some((content, bounds));
        Ok(())
    }
}
trait StripContent {
    fn render<D: DrawTarget<Color = Rgb565>>(&self, display: &mut D) -> Result<(), D::Error>;
}
fn render_region<D: DrawTarget<Color = Rgb565>, C: StripContent>(
    display: &mut D,
    bounds: Rectangle,
    content: &C,
    previous: Option<&C>,
    pixels: &mut [Rgb565],
    old_pixels: &mut [Rgb565],
) -> Result<(), D::Error> {
    for offset in (0..bounds.size.height).step_by(ClockBuffer::ROWS as usize) {
        let area = Rectangle::new(
            bounds.top_left + Point::new(0, offset as i32),
            Size::new(
                bounds.size.width,
                ClockBuffer::ROWS.min(bounds.size.height - offset),
            ),
        );
        let count = (area.size.width * area.size.height) as usize;
        compose_strip(area, &mut pixels[..count], content);
        if let Some(old) = previous {
            compose_strip(area, &mut old_pixels[..count], old);
            let width = area.size.width as usize;
            for row in 0..area.size.height as usize {
                let base = row * width;
                let mut x = 0;
                while x < width {
                    if pixels[base + x] == old_pixels[base + x] {
                        x += 1;
                        continue;
                    }
                    let start = x;
                    while x < width && pixels[base + x] != old_pixels[base + x] {
                        x += 1;
                    }
                    let run = Rectangle::new(
                        area.top_left + Point::new(start as i32, row as i32),
                        Size::new((x - start) as u32, 1),
                    );
                    // On any write error the taken snapshot stays invalid, so recovery
                    // redraws the affected regions instead of comparing against a partial frame.
                    display
                        .fill_contiguous(&run, pixels[base + start..base + x].iter().copied())?;
                }
            }
        } else {
            display.fill_contiguous(&area, pixels[..count].iter().copied())?;
        }
    }
    Ok(())
}
fn compose_strip<C: StripContent>(area: Rectangle, pixels: &mut [Rgb565], content: &C) {
    let mut strip = ClockStrip { area, pixels };
    match content.render(&mut strip) {
        Ok(()) => {}
        Err(never) => match never {},
    }
}
struct ClockStrip<'a> {
    area: Rectangle,
    pixels: &'a mut [Rgb565],
}
impl Dimensions for ClockStrip<'_> {
    fn bounding_box(&self) -> Rectangle {
        self.area
    }
}
impl DrawTarget for ClockStrip<'_> {
    type Color = Rgb565;
    type Error = std::convert::Infallible;
    fn draw_iter<I: IntoIterator<Item = Pixel<Rgb565>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Self::Error> {
        for Pixel(point, color) in pixels {
            if self.area.contains(point) {
                let relative = point - self.area.top_left;
                self.pixels
                    [relative.y as usize * self.area.size.width as usize + relative.x as usize] =
                    color;
            }
        }
        Ok(())
    }
}
fn text<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    font: &Font,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    panel: bool,
) -> Result<(), D::Error> {
    text_on(
        display,
        font,
        text,
        x,
        y,
        width,
        if panel {
            Backdrop::Solid(theme::PANEL)
        } else {
            Backdrop::Gradient
        },
    )
}
fn text_on<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    font: &Font,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    backdrop: Backdrop,
) -> Result<(), D::Error> {
    let text = theme::clipped(text, font, width);
    font.draw(
        display,
        &text,
        Point::new(x, y),
        theme::TEXT,
        backdrop,
        width,
    )
}

fn center<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    font: &Font,
    value: &str,
    y: i32,
) -> Result<(), D::Error> {
    text(
        display,
        font,
        value,
        (320 - font.width(value).min(296) as i32) / 2,
        y,
        296,
        false,
    )
}
pub fn weekday(language: Language, date: NaiveDate) -> &'static str {
    const DAYS: [[&str; 7]; 8] = [
        ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
        ["Lun", "Mar", "Mié", "Jue", "Vie", "Sáb", "Dom"],
        ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"],
        ["Lun", "Mar", "Mer", "Jeu", "Ven", "Sam", "Dim"],
        ["Пн", "Вт", "Ср", "Чт", "Пт", "Сб", "Нд"],
        ["Mån", "Tis", "Ons", "Tor", "Fre", "Lör", "Sön"],
        ["Lun", "Mar", "Mer", "Gio", "Ven", "Sab", "Dom"],
        ["Пн", "Вт", "Ср", "Чт", "Пт", "Сб", "Вс"],
    ];
    DAYS[language.index()][date.weekday().num_days_from_monday() as usize]
}
#[derive(PartialEq, Eq)]
struct ClockContent {
    time: String,
    period: Option<String>,
    date: String,
    synchronized: bool,
    hands: Option<(u32, u32, u32)>,
}
impl ClockContent {
    fn from_app(app: &App) -> Self {
        let settings = &app.preferences.current;
        let time = weather::local(settings, app.preferences.epoch);
        Self {
            time: time
                .map(|t| {
                    t.format(if settings.clock_24h {
                        "%H:%M:%S"
                    } else {
                        "%I:%M:%S"
                    })
                    .to_string()
                })
                .unwrap_or_else(|| "--:--:--".into()),
            period: time
                .filter(|_| !settings.clock_24h)
                .map(|t| t.format("%p").to_string()),
            date: time
                .map(|t| {
                    format!(
                        "{} {}",
                        weekday(settings.language, t.date_naive()),
                        t.format("%d.%m.%Y")
                    )
                })
                .unwrap_or_else(|| i18n::tr(settings.language, "Waiting for local time...").into()),
            synchronized: time.is_some(),
            hands: time.map(|t| (t.hour(), t.minute(), t.second())),
        }
    }
    fn render<D: DrawTarget<Color = Rgb565>>(&self, display: &mut D) -> Result<(), D::Error> {
        let mut display = display.clipped(&clock_bounds());
        theme::background(&mut display)?;
        render_analog(&mut display, self.hands)?;
        // Six fixed 28px digit cells and two 10px colon cells: 188px centered.
        // Center each glyph inside its cell, never the varying-width whole string.
        let mut x = (320 - (6 * 28 + 2 * 10)) / 2;
        for (index, ch) in self.time.chars().enumerate() {
            let cell_width = if index == 2 || index == 5 { 10 } else { 28 };
            let mut bytes = [0; 4];
            let glyph = ch.encode_utf8(&mut bytes);
            let width = CLOCK.width(glyph).min(cell_width);
            text(
                &mut display,
                &CLOCK,
                glyph,
                x + ((cell_width - width) / 2) as i32,
                214,
                width,
                false,
            )?;
            x += cell_width as i32;
        }
        if let Some(period) = &self.period {
            center(&mut display, &BODY, period, 266)?;
        }
        center(
            &mut display,
            if self.synchronized { &TITLE } else { &BODY },
            &self.date,
            290,
        )
    }
}
impl StripContent for ClockContent {
    fn render<D: DrawTarget<Color = Rgb565>>(&self, display: &mut D) -> Result<(), D::Error> {
        ClockContent::render(self, display)
    }
}
pub fn weather_bounds() -> [Rectangle; 2] {
    [
        Rectangle::new(Point::new(12, 38), Size::new(296, 16)),
        Rectangle::new(Point::new(12, 320), Size::new(296, 116)),
    ]
}
#[derive(PartialEq, Eq)]
struct WeatherContent {
    temperature: String,
    unit: &'static str,
    feels: String,
    icon: usize,
    status: String,
}
impl WeatherContent {
    fn from_app(app: &App) -> Self {
        let settings = &app.preferences.current;
        let current = app.weather.forecast.as_ref().map(|f| &f.current);
        let unit = if settings.fahrenheit { "F" } else { "C" };
        Self {
            temperature: weather::degrees(
                current.and_then(|c| c.temperature_2m),
                settings.fahrenheit,
            ),
            unit,
            feels: format!(
                "{} {}{unit}",
                i18n::tr(settings.language, "Feels like"),
                weather::degrees(
                    current.and_then(|c| c.apparent_temperature),
                    settings.fahrenheit
                )
            ),
            icon: weather::symbol(
                current.and_then(|c| c.weather_code),
                current.and_then(|c| c.is_day),
            ),
            status: i18n::tr(settings.language, status(app)).into(),
        }
    }
}
impl StripContent for WeatherContent {
    fn render<D: DrawTarget<Color = Rgb565>>(&self, display: &mut D) -> Result<(), D::Error> {
        theme::background(display)?;
        theme::rounded_radius(
            display,
            Rectangle::new(Point::new(12, 320), Size::new(296, 116)),
            theme::WEATHER_CARD,
            12,
        )?;
        let backdrop = Backdrop::Solid(theme::WEATHER_CARD);
        weather_icons::draw_on(display, self.icon, Point::new(28, 334), true, backdrop)?;
        let number = self.temperature.trim_end_matches('°');
        text_on(display, &CLOCK, number, 136, 332, 132, backdrop)?;
        text_on(
            display,
            &TITLE,
            &format!("°{}", self.unit),
            136 + CLOCK.width(number) as i32 + 4,
            352,
            40,
            backdrop,
        )?;
        let feels = theme::clipped(&self.feels, &TITLE, 272);
        text_on(
            display,
            &TITLE,
            &feels,
            (320 - TITLE.width(&feels) as i32) / 2,
            396,
            272,
            backdrop,
        )?;
        center(display, &SMALL, &self.status, 38)?;
        Ok(())
    }
}
fn dial_point(turns: f64, radius: f64) -> Point {
    let angle = turns * std::f64::consts::TAU;
    Point::new(
        160 + (angle.sin() * radius).round() as i32,
        132 - (angle.cos() * radius).round() as i32,
    )
}
fn hand_turns(hour: u32, minute: u32, second: u32) -> [f64; 3] {
    [
        ((hour % 12) as f64 + minute as f64 / 60.0 + second as f64 / 3600.0) / 12.0,
        (minute as f64 + second as f64 / 60.0) / 60.0,
        second as f64 / 60.0,
    ]
}
fn render_analog<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    hands: Option<(u32, u32, u32)>,
) -> Result<(), D::Error> {
    Circle::new(Point::new(86, 58), 149)
        .into_styled(PrimitiveStyle::with_stroke(theme::TEXT, 1))
        .draw(display)?;
    for tick in 0..60 {
        let hour = tick % 5 == 0;
        Line::new(
            dial_point(tick as f64 / 60.0, if hour { 62.0 } else { 68.0 }),
            dial_point(tick as f64 / 60.0, 71.0),
        )
        .into_styled(PrimitiveStyle::with_stroke(
            theme::TEXT,
            if hour { 2 } else { 1 },
        ))
        .draw(display)?;
    }
    if let Some((hour, minute, second)) = hands {
        let turns = hand_turns(hour, minute, second);
        for (index, radius, width, color) in [
            (0, 40.0, 5, theme::TEXT),
            (1, 57.0, 3, theme::TEXT),
            (2, 64.0, 1, theme::rgb(0xffa64d)),
        ] {
            Line::new(Point::new(160, 132), dial_point(turns[index], radius))
                .into_styled(PrimitiveStyle::with_stroke(color, width))
                .draw(display)?;
        }
        Circle::new(Point::new(157, 129), 7)
            .into_styled(PrimitiveStyle::with_fill(theme::TEXT))
            .draw(display)?;
    }
    Ok(())
}

pub fn render_clock<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    app: &App,
) -> Result<(), D::Error> {
    ClockContent::from_app(app).render(display)
}

fn status(app: &App) -> &'static str {
    if app.weather.forecast.is_some() {
        if app.weather.outdated(app.preferences.epoch) {
            "Weather outdated"
        } else {
            ""
        }
    } else if app.weather.failed {
        "Weather unavailable. Retrying..."
    } else {
        "Loading weather..."
    }
}
pub fn overview_bounds() -> Rectangle {
    Rectangle::new(Point::new(12, 38), Size::new(296, 391))
}
#[derive(PartialEq, Eq)]
struct OverviewRow {
    precipitation: Option<String>,
    label: String,
    icon: usize,
    left: String,
    right: String,
}
#[derive(PartialEq, Eq)]
struct OverviewContent {
    current: WeatherContent,
    week: bool,
    title: String,
    left_heading: String,
    right_heading: String,
    rows: Vec<OverviewRow>,
}
impl OverviewContent {
    fn from_app(app: &App) -> Self {
        let settings = &app.preferences.current;
        let lang = settings.language;
        let week = app.weather.view == View::Week;
        let today = weather::local(settings, app.preferences.epoch).map(|t| t.date_naive());
        let rows = (0..7)
            .map(|row| {
                if week {
                    let date =
                        today.and_then(|d| d.checked_add_days(chrono::Days::new(row as u64)));
                    let entry = app
                        .weather
                        .forecast
                        .as_ref()
                        .and_then(|f| today.and_then(|today| f.day(today, row)));
                    OverviewRow {
                        precipitation: Some(
                            entry
                                .and_then(|d| d.precipitation_probability_mean)
                                .map(|p| format!("{}%", p.round() as u8))
                                .unwrap_or_else(|| "--%".into()),
                        ),
                        label: date
                            .map(|d| weekday(lang, d).to_owned())
                            .unwrap_or_else(|| "--".into()),
                        icon: weather::symbol(entry.and_then(|d| d.code), Some(1)),
                        left: weather::degrees(entry.and_then(|d| d.low), settings.fahrenheit),
                        right: weather::degrees(entry.and_then(|d| d.high), settings.fahrenheit),
                    }
                } else {
                    let entry = app.weather.forecast.as_ref().and_then(|f| {
                        app.preferences
                            .epoch
                            .and_then(|now| f.upcoming(now).nth(row))
                    });
                    let time = entry.and_then(|h| weather::local(settings, Some(h.time)));
                    OverviewRow {
                        precipitation: None,
                        label: time
                            .map(|t| {
                                t.format(if settings.clock_24h {
                                    "%H:%M"
                                } else {
                                    "%I:%M%p"
                                })
                                .to_string()
                            })
                            .unwrap_or_else(|| "--:--".into()),
                        icon: weather::symbol(
                            entry.and_then(|h| h.code),
                            entry.and_then(|h| h.is_day),
                        ),
                        left: entry
                            .and_then(|h| h.precipitation)
                            .map(|p| format!("{p}%"))
                            .unwrap_or_else(|| "--%".into()),
                        right: weather::degrees(
                            entry.and_then(|h| h.temperature),
                            settings.fahrenheit,
                        ),
                    }
                }
            })
            .collect();
        Self {
            current: WeatherContent::from_app(app),
            week,
            title: i18n::tr(lang, if week { "Week" } else { "Next 7 hours" }).into(),
            left_heading: if week { i18n::tr(lang, "Min") } else { "%" }.into(),
            right_heading: if week {
                i18n::tr(lang, "Max")
            } else if settings.fahrenheit {
                "F"
            } else {
                "C"
            }
            .into(),
            rows,
        }
    }
}
const OVERVIEW_ROW_HEIGHT: u32 = 34;
fn overview_table_bounds() -> Rectangle {
    Rectangle::new(
        Point::new(12, 187),
        Size::new(296, 7 * OVERVIEW_ROW_HEIGHT + 4),
    )
}
impl StripContent for OverviewContent {
    fn render<D: DrawTarget<Color = Rgb565>>(&self, display: &mut D) -> Result<(), D::Error> {
        let mut display = display.clipped(&overview_bounds());
        theme::background(&mut display)?;
        weather_icons::draw(
            &mut display,
            self.current.icon,
            Point::new(12, 38),
            true,
            false,
        )?;
        text(
            &mut display,
            &FORECAST_TITLE,
            &format!("{}{}", self.current.temperature, self.current.unit),
            100,
            40,
            204,
            false,
        )?;
        text(
            &mut display,
            &FORECAST_BODY,
            &self.current.feels,
            100,
            80,
            204,
            false,
        )?;
        text(
            &mut display,
            &FORECAST_SMALL,
            &self.current.status,
            100,
            108,
            208,
            false,
        )?;
        text(
            &mut display,
            &FORECAST_BODY,
            &self.title,
            12,
            132,
            296,
            false,
        )?;
        if self.week {
            text(&mut display, &FORECAST_SMALL, "%", 112, 161, 68, false)?;
        }
        text(
            &mut display,
            &FORECAST_SMALL,
            &self.left_heading,
            if self.week { 184 } else { 174 },
            161,
            if self.week { 58 } else { 68 },
            false,
        )?;
        text(
            &mut display,
            &FORECAST_SMALL,
            &self.right_heading,
            246,
            161,
            58,
            false,
        )?;
        let table = overview_table_bounds();
        theme::rounded(&mut display, table, theme::WEATHER_CARD)?;
        let backdrop = Backdrop::Solid(theme::WEATHER_CARD);
        for (row, entry) in self.rows.iter().enumerate() {
            let y = table.top_left.y + 2 + row as i32 * OVERVIEW_ROW_HEIGHT as i32;
            let label_font = if self.week {
                &FORECAST_BODY
            } else {
                &FORECAST_SMALL
            };
            text_on(
                &mut display,
                label_font,
                &entry.label,
                if self.week { 16 } else { 20 },
                y + ((OVERVIEW_ROW_HEIGHT - label_font.height) / 2) as i32,
                if self.week { 56 } else { 112 },
                backdrop,
            )?;
            weather_icons::draw_on(
                &mut display,
                entry.icon,
                Point::new(if self.week { 76 } else { 134 }, y + 1),
                false,
                backdrop,
            )?;
            let value_y = y + ((OVERVIEW_ROW_HEIGHT - FORECAST_BODY.height) / 2) as i32;
            if let Some(precipitation) = &entry.precipitation {
                text_on(
                    &mut display,
                    &FORECAST_BODY,
                    precipitation,
                    112,
                    value_y,
                    68,
                    backdrop,
                )?;
            }
            text_on(
                &mut display,
                &FORECAST_BODY,
                &entry.left,
                if self.week { 184 } else { 174 },
                value_y,
                if self.week { 58 } else { 68 },
                backdrop,
            )?;
            text_on(
                &mut display,
                &FORECAST_BODY,
                &entry.right,
                246,
                value_y,
                58,
                backdrop,
            )?;
        }
        Ok(())
    }
}

pub fn render<D: DrawTarget<Color = Rgb565>>(display: &mut D, app: &App) -> Result<(), D::Error> {
    theme::background(display)?;
    let settings = &app.preferences.current;
    let overview = app.weather.view != View::Clock;
    text(
        display,
        &BODY,
        &settings.location.display_name(),
        12,
        12,
        256,
        false,
    )?;
    crate::ui::render_presence(display, app)?;
    if overview {
        OverviewContent::from_app(app).render(display)?;
    } else {
        render_clock(display, app)?;
        let content = WeatherContent::from_app(app);
        for area in weather_bounds() {
            content.render(&mut display.clipped(&area))?;
        }
    }
    text(
        display,
        &SMALL,
        "Open-Meteo | CC BY 4.0",
        12,
        451,
        248,
        false,
    )?;
    draw_settings_gear(display)?;
    Ok(())
}
fn draw_settings_gear<D: DrawTarget<Color = Rgb565>>(display: &mut D) -> Result<(), D::Error> {
    // MIT Tabler Icons derivative; see assets/ui/NOTICE.txt and MIT.txt.
    const ALPHA: &[u8] = include_bytes!("../assets/ui/settings-38.alpha");
    let area = Rectangle::new(Point::new(269, 439), Size::new(38, 38))
        .intersection(&display.bounding_box());
    let pixels = area.points().map(|p| {
        let alpha = ALPHA[((p.y - 439) * 38 + p.x - 269) as usize];
        theme::blend(
            theme::background_at(p.y),
            theme::TEXT,
            ((u16::from(alpha) * 15 + 127) / 255) as u8,
        )
    });
    display.fill_contiguous(&area, pixels)
}

#[cfg(test)]
mod analog_tests {
    use super::*;
    #[test]
    fn weekly_probability_rounding_missing_values_and_units() {
        let mut app = App::default();
        app.weather.view = View::Week;
        app.preferences.current.location.timezone = "UTC".into();
        app.preferences.epoch = Some(1_790_899_200);
        app.weather.forecast =
            Some(weather::parse(include_bytes!("../tests/fixtures/weather.json")).unwrap());
        for fahrenheit in [false, true] {
            app.preferences.current.fahrenheit = fahrenheit;
            for (probability, expected) in [
                (Some(0.0), "0%"),
                (Some(12.5), "13%"),
                (Some(35.4), "35%"),
                (Some(100.0), "100%"),
                (None, "--%"),
            ] {
                app.weather.forecast.as_mut().unwrap().days[0].precipitation_probability_mean =
                    probability;
                assert_eq!(
                    OverviewContent::from_app(&app).rows[0]
                        .precipitation
                        .as_deref(),
                    Some(expected)
                );
            }
        }
        app.weather.forecast = None;
        assert!(OverviewContent::from_app(&app)
            .rows
            .iter()
            .all(|row| row.precipitation.as_deref() == Some("--%")));
        app.weather.view = View::Hours;
        assert!(OverviewContent::from_app(&app)
            .rows
            .iter()
            .all(|row| row.precipitation.is_none()));
    }

    #[test]
    fn forecast_labels_and_values_fit_the_larger_fonts() {
        for language in Language::ALL {
            assert!(
                FORECAST_BODY.width(i18n::tr(language, "Next 7 hours")) <= 296,
                "title does not fit: {language:?}"
            );
            for day in 0..7 {
                let date = NaiveDate::from_ymd_opt(2026, 9, 28).unwrap() + chrono::Days::new(day);
                assert!(
                    FORECAST_BODY.width(weekday(language, date)) <= 56,
                    "{}: {}px",
                    weekday(language, date),
                    FORECAST_BODY.width(weekday(language, date))
                );
            }
        }
        assert!(FORECAST_BODY.width("100%") <= 68);
        for temperature in ["14°C", "-18°C", "100°F", "--°C"] {
            let label = format!(
                "{} {temperature}",
                i18n::tr(Language::Ukrainian, "Feels like")
            );
            assert!(
                FORECAST_BODY.width(&label) <= 204,
                "Ukrainian feels-like value is clipped: {label}"
            );
        }
        assert!(SMALL.width("Open-Meteo | CC BY 4.0") <= 248);
        for fahrenheit in [false, true] {
            for temperature in [None, Some(-40.0), Some(-25.0), Some(0.0), Some(55.0)] {
                assert!(FORECAST_BODY.width(&weather::degrees(temperature, fahrenheit)) <= 58);
            }
        }
        let mut app = App::default();
        app.weather.view = View::Hours;
        app.preferences.epoch = Some(1_790_899_200);
        app.weather.forecast =
            Some(weather::parse(include_bytes!("../tests/fixtures/weather.json")).unwrap());
        for zone in [
            "Europe/Berlin",
            "Europe/London",
            "America/New_York",
            "Asia/Kathmandu",
        ] {
            app.preferences.current.location.timezone = zone.into();
            for clock_24h in [false, true] {
                app.preferences.current.clock_24h = clock_24h;
                let rows = OverviewContent::from_app(&app).rows;
                assert_eq!(rows.len(), 7);
                for row in rows {
                    assert!(FORECAST_SMALL.width(&row.label) <= 112, "{}", row.label);
                    assert!(
                        row.label
                            .chars()
                            .all(|ch| ch.is_ascii_digit() || ":-APM".contains(ch)),
                        "hourly labels must contain only time: {}",
                        row.label
                    );
                }
            }
        }
        assert!(FORECAST_SMALL.height <= OVERVIEW_ROW_HEIGHT);
        assert!(FORECAST_BODY.height <= OVERVIEW_ROW_HEIGHT);
        assert!(overview_table_bounds().bottom_right().unwrap().y < 442);
    }

    #[test]
    fn clockwise_cardinal_positions_and_interpolated_hands() {
        assert_eq!(dial_point(0.0, 64.0), Point::new(160, 68));
        assert_eq!(dial_point(0.25, 64.0), Point::new(224, 132));
        assert_eq!(dial_point(0.5, 64.0), Point::new(160, 196));
        assert_eq!(dial_point(0.75, 64.0), Point::new(96, 132));
        assert_eq!(hand_turns(0, 0, 0), hand_turns(12, 0, 0));
        assert_eq!(hand_turns(3, 0, 0)[0], 0.25);
        let before = hand_turns(11, 59, 59);
        assert!(before.iter().all(|angle| *angle < 1.0 && *angle > 0.98));
        assert_eq!(hand_turns(12, 0, 0), [0.0; 3]);
        let quarter = hand_turns(3, 15, 30);
        assert!((quarter[0] - 3.2583333333333333 / 12.0).abs() < 1e-12);
        assert_eq!(quarter[1], 15.5 / 60.0);
        assert_eq!(quarter[2], 0.5);
    }
}
