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

/// Returns the rectangle reserved for clock rendering.
///
/// # Arguments
///
/// None.
///
/// # Returns
///
/// `Rectangle` - Clock region in portrait screen pixels.
pub fn clock_bounds() -> Rectangle {
    // Allocates two reusable strip buffers for differential rendering.
    Rectangle::new(Point::new(12, 54), Size::new(296, 262))
}
/// Compose old/new strips in RAM and transfer only pixels whose final color changes.
pub struct ClockBuffer {
    // Row-major RGB565 colour buffer or iterator. Stored as Vec<Rgb565>.
    pixels: Vec<Rgb565>,
    // Reusable strip buffer holding the prior content for exact colour comparison. Stored as
    // Vec<Rgb565>.
    old_pixels: Vec<Rgb565>,
    // Last successful clock snapshot, or the prior regional content selected for comparison. Stored
    // as Option<(ClockContent, Rectangle)>.
    previous: Option<(ClockContent, Rectangle)>,
    // Last successful current-weather snapshot and its drawing bounds. Stored as
    // Option<(WeatherContent, Rectangle)>.
    previous_weather: Option<(WeatherContent, Rectangle)>,
    // Last successful forecast-table snapshot and its drawing bounds. Stored as
    // Option<(OverviewContent, Rectangle)>.
    previous_overview: Option<(OverviewContent, Rectangle)>,
}
impl ClockBuffer {
    // Maximum sixteen rows composed per strip, limiting ram usage instead of allocating a full
    // framebuffer. This fixed value is shared by the operations below.
    const ROWS: u32 = 16;

    /// Allocates two reusable strip buffers for differential rendering.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `Result<Self, std::collections::TryReserveError>` - Ok with empty clock/weather/overview
    /// baselines; Err if either strip allocation fails.
    ///
    /// # Errors
    ///
    /// Propagates TryReserveError from fallible vector reservation.
    pub fn new() -> Result<Self, std::collections::TryReserveError> {
        /// Allocates one RGB565 strip buffer without allocating a full framebuffer.
        ///
        /// # Arguments
        ///
        /// None.
        ///
        /// # Returns
        ///
        /// `Result<Vec<Rgb565>, std::collections::TryReserveError>` - Ok with a black-initialized
        /// buffer sized for 16 clock-region rows; Err on reservation failure.
        ///
        /// # Errors
        ///
        /// Propagates TryReserveError from vector reservation.
        fn allocate() -> Result<Vec<Rgb565>, std::collections::TryReserveError> {
            // Keep number of elements involved in the current operation in this local variable for
            // the following operations.
            let count = clock_bounds().size.width as usize * ClockBuffer::ROWS as usize;
            // Keep row-major RGB565 colour buffer or iterator in this local variable for the
            // following operations.
            let mut pixels = Vec::new();
            // Reserve the needed heap space fallibly so allocation failure can be returned to the
            // caller.
            pixels.try_reserve_exact(count)?;
            // Set the buffer length and initialize new elements to the supplied colour.
            pixels.resize(count, Rgb565::BLACK);
            // Return success; the caller receives row-major RGB565 colour buffer or iterator.
            Ok(pixels)
        }
        // Return success; the caller receives a constructed Self value using these fields.
        Ok(Self {
            // Initialize row-major RGB565 colour buffer or iterator from the supplied value.
            pixels: allocate()?,
            // Initialize reusable strip buffer holding the prior content for exact colour
            // comparison from the supplied value.
            old_pixels: allocate()?,
            // Leave last successful clock snapshot, or the prior regional content selected for
            // comparison unavailable until a later operation supplies it.
            previous: None,
            // Leave last successful current-weather snapshot and its drawing bounds unavailable
            // until a later operation supplies it.
            previous_weather: None,
            // Leave last successful forecast-table snapshot and its drawing bounds unavailable
            // until a later operation supplies it.
            previous_overview: None,
        })
    }

    /// Discards all differential-rendering baselines.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut ClockBuffer`) - Reusable strip buffers and last successfully rendered
    ///   content snapshots. Mutated in place.
    ///
    /// # Returns
    ///
    /// `()` - No value; subsequent updates must establish fresh clock, weather, and overview
    /// snapshots.
    pub fn invalidate(&mut self) {
        // Set last successful clock snapshot, or the prior regional content selected for comparison
        // to no available value.
        self.previous = None;
        // Set last successful current-weather snapshot and its drawing bounds to no available
        // value.
        self.previous_weather = None;
        // Set last successful forecast-table snapshot and its drawing bounds to no available value.
        self.previous_overview = None;
    }

    /// Call only after a successful complete weather-page draw.
    ///
    /// Records the baseline after a successful full weather-page draw.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut ClockBuffer`) - Reusable strip buffers and last successfully rendered
    ///   content snapshots. Mutated in place.
    /// * `app` (`&App`) - Application model supplying the active page, preferences, weather,
    ///   and presence state.
    ///
    /// # Returns
    ///
    /// `()` - No value; caches the current view's rendered content. Call only after the full
    /// draw succeeds.
    pub fn remember(&mut self, app: &App) {
        // Discards all differential-rendering baselines.
        self.invalidate();
        // Week and Hours pages use a table snapshot rather than the separate clock/current-weather
        // regions.
        if app.weather.view != View::Clock {
            // Set last successful forecast-table snapshot and its drawing bounds to an available
            // value for the ordered tuple of related values.
            self.previous_overview = Some((OverviewContent::from_app(app), overview_bounds()));
            // Leave this function now after completing the required side effects; later statements
            // are skipped.
            return;
        }
        // Set last successful clock snapshot, or the prior regional content selected for comparison
        // to an available value for the ordered tuple of related values.
        self.previous = Some((ClockContent::from_app(app), clock_bounds()));
        // Set last successful current-weather snapshot and its drawing bounds to an available value
        // for the ordered tuple of related values.
        self.previous_weather = Some((
            WeatherContent::from_app(app),
            Rectangle::new(Point::zero(), Size::new(320, 480)),
        ));
    }

    /// Compares clock strips with the last snapshot and transfers changed pixel runs.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut ClockBuffer`) - Reusable strip buffers and last successfully rendered
    ///   content snapshots. Mutated in place.
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
    /// `Result<(), D::Error>` - Ok after the update or if unchanged; Err from the draw target.
    /// A failed transfer leaves the clock baseline invalidated.
    ///
    /// # Errors
    ///
    /// Propagates pixel transfer errors; the next update redraws the affected region.
    pub fn render<D: DrawTarget<Color = Rgb565>>(
        &mut self,
        display: &mut D,
        app: &App,
    ) -> Result<(), D::Error> {
        // Keep screen-coordinate rectangle limiting the drawing operation in this local variable
        // for the following operations.
        let bounds = clock_bounds().intersection(&display.bounding_box());
        // Keep formatted content snapshot independent of the physical display transport in this
        // local variable for the following operations.
        let content = ClockContent::from_app(app);
        // Keep last successful clock snapshot, or the prior regional content selected for
        // comparison in this local variable for the following operations.
        let previous = self.previous.take().filter(|(_, area)| *area == bounds);
        // The formatted content equals the last successful snapshot; restore the baseline and skip
        // all display writes.
        if previous.as_ref().is_some_and(|(old, _)| *old == content) {
            // Set last successful clock snapshot, or the prior regional content selected for
            // comparison to last successful clock snapshot, or the prior regional content selected
            // for comparison.
            self.previous = previous;
            // Leave this function now with a successful result carrying (); later statements are
            // skipped.
            return Ok(());
        }
        // Composes current and previous strips and transmits only changed runs, or complete strips
        // without a baseline.
        render_region(
            display,
            bounds,
            &content,
            previous.as_ref().map(|(old, _)| old),
            &mut self.pixels,
            &mut self.old_pixels,
        )?;
        // Set last successful clock snapshot, or the prior regional content selected for comparison
        // to an available value for the ordered tuple of related values.
        self.previous = Some((content, bounds));
        // Return success after the required side effects are complete.
        Ok(())
    }
    /// Updates changed weather cards or overview-table pixels for the selected view.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut ClockBuffer`) - Reusable strip buffers and last successfully rendered
    ///   content snapshots. Mutated in place.
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
    /// `Result<(), D::Error>` - Ok after the update or if unchanged; Err from the draw target.
    /// Failed transfers invalidate the affected baseline.
    ///
    /// # Errors
    ///
    /// Propagates pixel transfer errors so later updates can recover with a fresh redraw.
    pub fn render_weather<D: DrawTarget<Color = Rgb565>>(
        &mut self,
        display: &mut D,
        app: &App,
    ) -> Result<(), D::Error> {
        // Week and Hours pages use a table snapshot rather than the separate clock/current-weather
        // regions.
        if app.weather.view != View::Clock {
            // Keep screen-coordinate rectangle limiting the drawing operation in this local
            // variable for the following operations.
            let bounds = overview_bounds().intersection(&display.bounding_box());
            // Keep formatted content snapshot independent of the physical display transport in this
            // local variable for the following operations.
            let content = OverviewContent::from_app(app);
            // Keep last successful clock snapshot, or the prior regional content selected for
            // comparison in this local variable for the following operations.
            let previous = self
                .previous_overview
                .take()
                .filter(|(_, area)| *area == bounds);
            // The formatted content equals the last successful snapshot; restore the baseline and
            // skip all display writes.
            if previous.as_ref().is_some_and(|(old, _)| *old == content) {
                // Set last successful forecast-table snapshot and its drawing bounds to last
                // successful clock snapshot, or the prior regional content selected for comparison.
                self.previous_overview = previous;
                // Leave this function now with a successful result carrying (); later statements
                // are skipped.
                return Ok(());
            }
            // Composes current and previous strips and transmits only changed runs, or complete
            // strips without a baseline.
            render_region(
                display,
                bounds,
                &content,
                previous.as_ref().map(|(old, _)| old),
                &mut self.pixels,
                &mut self.old_pixels,
            )?;
            // Set last successful forecast-table snapshot and its drawing bounds to an available
            // value for the ordered tuple of related values.
            self.previous_overview = Some((content, bounds));
            // Leave this function now with a successful result carrying (); later statements are
            // skipped.
            return Ok(());
        }
        // Keep screen-coordinate rectangle limiting the drawing operation in this local variable
        // for the following operations.
        let bounds = display.bounding_box();
        // Keep formatted content snapshot independent of the physical display transport in this
        // local variable for the following operations.
        let content = WeatherContent::from_app(app);
        // Keep last successful clock snapshot, or the prior regional content selected for
        // comparison in this local variable for the following operations.
        let previous = self
            .previous_weather
            .take()
            .filter(|(_, area)| *area == bounds);
        // The formatted content equals the last successful snapshot; restore the baseline and skip
        // all display writes.
        if previous.as_ref().is_some_and(|(old, _)| *old == content) {
            // Set last successful current-weather snapshot and its drawing bounds to last
            // successful clock snapshot, or the prior regional content selected for comparison.
            self.previous_weather = previous;
            // Leave this function now with a successful result carrying (); later statements are
            // skipped.
            return Ok(());
        }
        // Visit each entry in returns the disjoint rectangles reserved for status and
        // current-weather updates; the loop binding provides its value or index for this iteration.
        for area in weather_bounds() {
            // Composes current and previous strips and transmits only changed runs, or complete
            // strips without a baseline.
            render_region(
                display,
                area.intersection(&bounds),
                &content,
                previous.as_ref().map(|(old, _)| old),
                &mut self.pixels,
                &mut self.old_pixels,
            )?;
        }
        // Set last successful current-weather snapshot and its drawing bounds to an available value
        // for the ordered tuple of related values.
        self.previous_weather = Some((content, bounds));
        // Return success after the required side effects are complete.
        Ok(())
    }
}
trait StripContent {
    /// Draws the represented content into an RGB565 target, including clipped strip targets.
    ///
    /// # Arguments
    ///
    /// * `self` (`&StripContent`) - Receiver state used by this operation. Borrowed without
    ///   changing it.
    /// * `display` (`&mut D`) - Mutable RGB565 draw target receiving the rendering operations.
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
    /// Propagates content drawing errors.
    fn render<D: DrawTarget<Color = Rgb565>>(&self, display: &mut D) -> Result<(), D::Error>;
}
/// Composes current and previous strips and transmits only changed runs, or complete strips
/// without a baseline.
///
/// # Arguments
///
/// * `display` (`&mut D`) - Mutable RGB565 draw target receiving the rendering operations.
/// * `bounds` (`Rectangle`) - Rectangle to draw or update in screen pixels.
/// * `content` (`&C`) - Current content renderer used to compose clipped strips.
/// * `previous` (`Option<&C>`) - Previous rendered content, or None when a full regional
///   redraw is required.
/// * `pixels` (`&mut [Rgb565]`) - Reusable RAM buffer sized for the composed rectangle or
///   strip.
/// * `old_pixels` (`&mut [Rgb565]`) - Reusable RAM buffer for previous strips; must hold
///   each composed strip.
///
/// # Type Parameters
///
/// * `D` - RGB565 drawing destination; its associated error is propagated by fallible
///   renderers.
/// * `C` - Content renderer implementing StripContent for in-memory strip composition.
///
/// # Returns
///
/// `Result<(), D::Error>` - Ok after transfers; Err from the draw target.
///
/// # Errors
///
/// Propagates the first contiguous pixel transfer failure.
///
/// # Panics
///
/// Panics if either reusable buffer is too small for a composed strip.
fn render_region<D: DrawTarget<Color = Rgb565>, C: StripContent>(
    display: &mut D,
    bounds: Rectangle,
    content: &C,
    previous: Option<&C>,
    pixels: &mut [Rgb565],
    old_pixels: &mut [Rgb565],
) -> Result<(), D::Error> {
    // Visit each entry in step by result for the surrounding operation; the loop binding provides
    // its value or index for this iteration.
    for offset in (0..bounds.size.height).step_by(ClockBuffer::ROWS as usize) {
        // Keep screen-coordinate rectangle currently being rendered or transferred in this local
        // variable for the following operations.
        let area = Rectangle::new(
            bounds.top_left + Point::new(0, offset as i32),
            Size::new(
                bounds.size.width,
                ClockBuffer::ROWS.min(bounds.size.height - offset),
            ),
        );
        // Keep number of elements involved in the current operation in this local variable for the
        // following operations.
        let count = (area.size.width * area.size.height) as usize;
        // Renders a clipped content strip into the supplied RAM buffer.
        compose_strip(area, &mut pixels[..count], content);
        // A trusted prior snapshot exists, enabling exact old/new pixel comparison instead of a
        // complete regional transfer.
        if let Some(old) = previous {
            // Renders a clipped content strip into the supplied RAM buffer.
            compose_strip(area, &mut old_pixels[..count], old);
            // Keep horizontal extent in pixels in this local variable for the following operations.
            let width = area.size.width as usize;
            // Visit each entry in 0..area.size.height as usize; the loop binding provides its value
            // or index for this iteration.
            for row in 0..area.size.height as usize {
                // Locate the current row in the strip buffer so horizontal changed-pixel runs
                // remain within that row.
                let base = row * width;
                // Keep horizontal coordinate or current position along the row in this local
                // variable for the following operations.
                let mut x = 0;
                // Inspect the remaining columns in this strip row, keeping horizontal transfers
                // inside the row.
                while x < width {
                    // This final pixel colour is unchanged; move to the next column without sending
                    // it over SPI.
                    if pixels[base + x] == old_pixels[base + x] {
                        // Update horizontal coordinate or current position along the row using 1,
                        // retaining the accumulated state for subsequent steps.
                        x += 1;
                        // Skip the remainder of this iteration and wait for or inspect the next
                        // input.
                        continue;
                    }
                    // Keep first changed pixel within the current horizontal transfer run in this
                    // local variable for the following operations.
                    let start = x;
                    // Extend the changed-pixel run until the row ends or the next unchanged colour
                    // is found.
                    while x < width && pixels[base + x] != old_pixels[base + x] {
                        // Update horizontal coordinate or current position along the row using 1,
                        // retaining the accumulated state for subsequent steps.
                        x += 1;
                    }
                    // Describe one consecutive horizontal run whose final colours differ; only
                    // those pixels need an SPI transfer.
                    let run = Rectangle::new(
                        area.top_left + Point::new(start as i32, row as i32),
                        Size::new((x - start) as u32, 1),
                    );
                    // On any write error the taken snapshot stays invalid, so recovery
                    // redraws the affected regions instead of comparing against a partial frame.
                    // Transfer a composed row-major colour rectangle, avoiding a visible
                    // clear-before-draw pass.
                    display
                        .fill_contiguous(&run, pixels[base + start..base + x].iter().copied())?;
                }
            }
        } else {
            // Transfer a composed row-major colour rectangle, avoiding a visible clear-before-draw
            // pass.
            display.fill_contiguous(&area, pixels[..count].iter().copied())?;
        }
    }
    // Return success after the required side effects are complete.
    Ok(())
}
/// Renders a clipped content strip into the supplied RAM buffer.
///
/// # Arguments
///
/// * `area` (`Rectangle`) - Screen-coordinate rectangle of the composed pixel transfer, in
///   pixels.
/// * `pixels` (`&mut [Rgb565]`) - Reusable RAM buffer sized for the composed rectangle or
///   strip.
/// * `content` (`&C`) - Current content renderer used to compose clipped strips.
///
/// # Type Parameters
///
/// * `C` - Content renderer implementing StripContent for in-memory strip composition.
///
/// # Returns
///
/// `()` - No value; fills the buffer through an infallible in-memory draw target.
///
/// # Panics
///
/// Panics if the supplied pixel buffer is too small for the rectangle.
fn compose_strip<C: StripContent>(area: Rectangle, pixels: &mut [Rgb565], content: &C) {
    // Keep in-memory target clipped to the current screen-coordinate rectangle in this local
    // variable for the following operations.
    let mut strip = ClockStrip { area, pixels };
    // Choose the appropriate path for compares clock strips with the last snapshot and transfers
    // changed pixel runs; each arm handles one supported case.
    match content.render(&mut strip) {
        // Continue with the successful result, using its validated value in this case.
        Ok(()) => {}
        // Choose the appropriate path for never; each arm handles one supported case.
        Err(never) => match never {},
    }
}
struct ClockStrip<'a> {
    // Screen-coordinate rectangle currently being rendered or transferred. Stored as Rectangle.
    area: Rectangle,
    // Row-major RGB565 colour buffer or iterator. Stored as &'a mut [Rgb565].
    pixels: &'a mut [Rgb565],
}
impl Dimensions for ClockStrip<'_> {
    /// Reports the strip's screen-coordinate rectangle.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Self`) - Receiver state used by this operation. Borrowed without changing
    ///   it.
    ///
    /// # Returns
    ///
    /// `Rectangle` - Bounds used to clip drawing into the strip buffer.
    fn bounding_box(&self) -> Rectangle {
        // Return screen-coordinate rectangle currently being rendered or transferred as the value
        // of this block.
        self.area
    }
}
impl DrawTarget for ClockStrip<'_> {
    type Color = Rgb565;
    type Error = std::convert::Infallible;
    /// Copies pixels inside the strip bounds into its row-major RAM buffer.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Self`) - Receiver state used by this operation. Mutated in place.
    /// * `pixels` (`I`) - Iterable screen-coordinate pixels and RGB565 colours to draw.
    ///
    /// # Type Parameters
    ///
    /// * `I` - Input iterable with the item type required by this function's iterator bound.
    ///
    /// # Returns
    ///
    /// `Result<(), Self::Error>` - Always Ok; ignores pixels outside the strip. The draw target
    /// error type is Infallible.
    fn draw_iter<I: IntoIterator<Item = Pixel<Rgb565>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Self::Error> {
        // Visit each entry in row-major RGB565 colour buffer or iterator; the loop binding provides
        // its value or index for this iteration.
        for Pixel(point, color) in pixels {
            // Copy only pixels belonging to this clipped strip into its row-major RAM buffer.
            if self.area.contains(point) {
                // Keep pixel position measured from the strip's top-left corner in this local
                // variable for the following operations.
                let relative = point - self.area.top_left;
                // Set the selected entry from row-major RGB565 colour buffer or iterator to RGB565
                // colour used for this drawing operation.
                self.pixels
                    [relative.y as usize * self.area.size.width as usize + relative.x as usize] =
                    color;
            }
        }
        // Return success after the required side effects are complete.
        Ok(())
    }
}
/// Clips weather text and draws it on the panel or gradient backdrop.
///
/// # Arguments
///
/// * `display` (`&mut D`) - Mutable RGB565 draw target receiving the rendering operations.
/// * `font` (`&Font`) - Proportional bitmap font used to measure and draw text.
/// * `text` (`&str`) - Text to translate, measure, clip, or draw.
/// * `x` (`i32`) - Horizontal screen coordinate in pixels.
/// * `y` (`i32`) - Vertical screen coordinate in pixels.
/// * `width` (`u32`) - Available drawing or text width in pixels.
/// * `panel` (`bool`) - Whether to use the solid panel backdrop instead of the page
///   gradient.
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
    font: &Font,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    panel: bool,
) -> Result<(), D::Error> {
    // Clips weather text and draws it against an explicit backdrop.
    text_on(
        display,
        font,
        text,
        x,
        y,
        width,
        // Select the solid panel background when text is inside a panel; otherwise preserve the
        // page gradient.
        if panel {
            // Run the Solid operation with the supplied inputs.
            Backdrop::Solid(theme::PANEL)
        } else {
            Backdrop::Gradient
        },
    )
}
/// Clips weather text and draws it against an explicit backdrop.
///
/// # Arguments
///
/// * `display` (`&mut D`) - Mutable RGB565 draw target receiving the rendering operations.
/// * `font` (`&Font`) - Proportional bitmap font used to measure and draw text.
/// * `text` (`&str`) - Text to translate, measure, clip, or draw.
/// * `x` (`i32`) - Horizontal screen coordinate in pixels.
/// * `y` (`i32`) - Vertical screen coordinate in pixels.
/// * `width` (`u32`) - Available drawing or text width in pixels.
/// * `backdrop` (`Backdrop`) - Solid colour or vertical gradient used behind antialiased
///   pixels.
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
fn text_on<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    font: &Font,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    backdrop: Backdrop,
) -> Result<(), D::Error> {
    // Keep sanitized or clipped text prepared for the available drawing area in this local variable
    // for the following operations.
    let text = theme::clipped(text, font, width);
    // Send the prepared shape or text pixels to the current drawing destination.
    font.draw(
        display,
        &text,
        Point::new(x, y),
        theme::TEXT,
        backdrop,
        width,
    )
}

/// Centers a clipped weather label within the portrait page.
///
/// # Arguments
///
/// * `display` (`&mut D`) - Mutable RGB565 draw target receiving the rendering operations.
/// * `font` (`&Font`) - Proportional bitmap font used to measure and draw text.
/// * `value` (`&str`) - Text to format or draw.
/// * `y` (`i32`) - Vertical screen coordinate in pixels.
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
/// Propagates centered text drawing failures.
fn center<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    font: &Font,
    value: &str,
    y: i32,
) -> Result<(), D::Error> {
    // Clips weather text and draws it on the panel or gradient backdrop.
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
/// Selects the abbreviated weekday in the configured language.
///
/// # Arguments
///
/// * `language` (`Language`) - Supported language used for translated labels or API
///   requests.
/// * `date` (`NaiveDate`) - Local calendar date whose weekday is displayed.
///
/// # Returns
///
/// `&'static str` - Static localized abbreviation for the supplied calendar date.
pub fn weekday(language: Language, date: NaiveDate) -> &'static str {
    // Store weekday abbreviations by language index and Monday-first weekday index.
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
    // Timestamp or formatted clock value used by the current view. Stored as String.
    time: String,
    // Optional AM/PM label; omitted in 24-hour mode. Stored as Option<String>.
    period: Option<String>,
    // Local calendar date used for daily forecast selection. Stored as String.
    date: String,
    // Whether SNTP has supplied trusted time for TLS and local clock calculations. Stored as bool.
    synchronized: bool,
    // Optional local hour, minute, and second values used to position analog hands. Stored as
    // Option<(u32, u32, u32)>.
    hands: Option<(u32, u32, u32)>,
}
impl ClockContent {
    /// Builds formatted clock content and analog hand values from application preferences.
    ///
    /// # Arguments
    ///
    /// * `app` (`&App`) - Application model supplying the active page, preferences, weather,
    ///   and presence state.
    ///
    /// # Returns
    ///
    /// `Self` - Clock snapshot with localized date, formatted time, and placeholders when local
    /// time is unavailable.
    fn from_app(app: &App) -> Self {
        // Keep device preferences controlling location, language, units, brightness, and clock
        // format in this local variable for the following operations.
        let settings = &app.preferences.current;
        // Keep timestamp or formatted clock value used by the current view in this local variable
        // for the following operations.
        let time = weather::local(settings, app.preferences.epoch);
        Self {
            // Initialize timestamp or formatted clock value used by the current view from the
            // supplied value.
            time: time
                .map(|t| {
                    // Select 24-hour formatting when enabled; otherwise keep the corresponding
                    // 12-hour layout.
                    t.format(if settings.clock_24h {
                        "%H:%M:%S"
                    } else {
                        "%I:%M:%S"
                    })
                    .to_string()
                })
                .unwrap_or_else(|| "--:--:--".into()),
            // Initialize optional AM/PM label; omitted in 24-hour mode from the supplied value.
            period: time
                .filter(|_| !settings.clock_24h)
                .map(|t| t.format("%p").to_string()),
            // Initialize local calendar date used for daily forecast selection from the supplied
            // value.
            date: time
                .map(|t| {
                    format!(
                        "{} {}",
                        weekday(settings.language, t.date_naive()),
                        t.format("%d.%m.%Y")
                    )
                })
                .unwrap_or_else(|| i18n::tr(settings.language, "Waiting for local time...").into()),
            // Initialize whether SNTP has supplied trusted time for TLS and local clock
            // calculations from the supplied value.
            synchronized: time.is_some(),
            // Initialize optional local hour, minute, and second values used to position analog
            // hands from the supplied value.
            hands: time.map(|t| (t.hour(), t.minute(), t.second())),
        }
    }
    /// Renders the analog dial, fixed-width digital cells, period, and date within the clock
    /// region.
    ///
    /// # Arguments
    ///
    /// * `self` (`&ClockContent`) - Receiver state used by this operation. Borrowed without
    ///   changing it.
    /// * `display` (`&mut D`) - Mutable RGB565 draw target receiving the rendering operations.
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
    /// Propagates clock background, dial, or text drawing errors.
    fn render<D: DrawTarget<Color = Rgb565>>(&self, display: &mut D) -> Result<(), D::Error> {
        // Keep drawing destination receiving RGB565 pixels in this local variable for the following
        // operations.
        let mut display = display.clipped(&clock_bounds());
        // Run the background operation with the supplied inputs.
        theme::background(&mut display)?;
        // Draws the dial ticks and optional hour, minute, and second hands.
        render_analog(&mut display, self.hands)?;
        // Six fixed 28px digit cells and two 10px colon cells: 188px centered.
        // Center each glyph inside its cell, never the varying-width whole string.
        // Keep horizontal coordinate or current position along the row in this local variable for
        // the following operations.
        let mut x = (320 - (6 * 28 + 2 * 10)) / 2;
        // Visit each entry in enumerate result for the surrounding operation; the loop binding
        // provides its value or index for this iteration.
        for (index, ch) in self.time.chars().enumerate() {
            // Reserve a fixed 10-pixel cell for each colon and 28 pixels for each digit, preventing
            // horizontal clock jitter.
            let cell_width = if index == 2 || index == 5 { 10 } else { 28 };
            // Keep four-byte UTF-8 scratch buffer for encoding one digital clock character in this
            // local variable for the following operations.
            let mut bytes = [0; 4];
            // Keep selected font character and its bitmap metrics in this local variable for the
            // following operations.
            let glyph = ch.encode_utf8(&mut bytes);
            // Keep horizontal extent in pixels in this local variable for the following operations.
            let width = CLOCK.width(glyph).min(cell_width);
            // Clips weather text and draws it on the panel or gradient backdrop.
            text(
                &mut display,
                &CLOCK,
                glyph,
                x + ((cell_width - width) / 2) as i32,
                214,
                width,
                false,
            )?;
            // Update horizontal coordinate or current position along the row using the numeric
            // value converted to the required arithmetic or indexing type, retaining the
            // accumulated state for subsequent steps.
            x += cell_width as i32;
        }
        // Draw AM/PM only for a 12-hour clock with available local time.
        if let Some(period) = &self.period {
            // Centers a clipped weather label within the portrait page.
            center(&mut display, &BODY, period, 266)?;
        }
        // Centers a clipped weather label within the portrait page.
        center(
            &mut display,
            // Use the date's prominent font only when local time is valid; unresolved time displays
            // a smaller waiting message.
            if self.synchronized { &TITLE } else { &BODY },
            &self.date,
            290,
        )
    }
}
impl StripContent for ClockContent {
    /// Renders the analog dial, fixed-width digital cells, period, and date within the clock
    /// region.
    ///
    /// # Arguments
    ///
    /// * `self` (`&ClockContent`) - Receiver state used by this operation. Borrowed without
    ///   changing it.
    /// * `display` (`&mut D`) - Mutable RGB565 draw target receiving the rendering operations.
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
    /// Propagates clock background, dial, or text drawing errors.
    fn render<D: DrawTarget<Color = Rgb565>>(&self, display: &mut D) -> Result<(), D::Error> {
        // Compares clock strips with the last snapshot and transfers changed pixel runs.
        ClockContent::render(self, display)
    }
}
/// Returns the disjoint rectangles reserved for status and current-weather updates.
///
/// # Arguments
///
/// None.
///
/// # Returns
///
/// `[Rectangle; 2]` - Two portrait pixel regions that exclude the clock and page controls.
pub fn weather_bounds() -> [Rectangle; 2] {
    // Return the ordered sample/byte array as the value of this block.
    [
        Rectangle::new(Point::new(12, 38), Size::new(296, 16)),
        Rectangle::new(Point::new(12, 320), Size::new(296, 116)),
    ]
}
#[derive(PartialEq, Eq)]
struct WeatherContent {
    // Temperature. Stored as String.
    temperature: String,
    // Single-character temperature unit selected from the preferences. Stored as &'static str.
    unit: &'static str,
    // Formatted feels-like temperature label, clipped to fit the weather card. Stored as String.
    feels: String,
    // Index of the retained Meteocons symbol representing the forecast condition. Stored as usize.
    icon: usize,
    // Status displayed to the reader instead of silent failure. Stored as String.
    status: String,
}
impl WeatherContent {
    /// Builds current-weather labels, icon selection, and freshness status.
    ///
    /// # Arguments
    ///
    /// * `app` (`&App`) - Application model supplying the active page, preferences, weather,
    ///   and presence state.
    ///
    /// # Returns
    ///
    /// `Self` - Weather snapshot with converted temperature and placeholders for missing
    /// measurements.
    fn from_app(app: &App) -> Self {
        // Keep device preferences controlling location, language, units, brightness, and clock
        // format in this local variable for the following operations.
        let settings = &app.preferences.current;
        // Keep current state or measurement being prepared for display in this local variable for
        // the following operations.
        let current = app.weather.forecast.as_ref().map(|f| &f.current);
        // Keep single-character temperature unit selected from the preferences in this local
        // variable for the following operations.
        let unit = if settings.fahrenheit { "F" } else { "C" };
        Self {
            // Initialize temperature from the supplied value.
            temperature: weather::degrees(
                current.and_then(|c| c.temperature_2m),
                settings.fahrenheit,
            ),
            // Initialize single-character temperature unit selected from the preferences from the
            // supplied value.
            unit,
            // Initialize formatted feels-like temperature label, clipped to fit the weather card
            // from the supplied value.
            feels: format!(
                "{} {}{unit}",
                i18n::tr(settings.language, "Feels like"),
                weather::degrees(
                    current.and_then(|c| c.apparent_temperature),
                    settings.fahrenheit
                )
            ),
            // Initialize index of the retained Meteocons symbol representing the forecast condition
            // from the supplied value.
            icon: weather::symbol(
                current.and_then(|c| c.weather_code),
                current.and_then(|c| c.is_day),
            ),
            // Initialize status displayed to the reader instead of silent failure from the supplied
            // value.
            status: i18n::tr(settings.language, status(app)).into(),
        }
    }
}
impl StripContent for WeatherContent {
    /// Draws the current-weather card and its status line.
    ///
    /// # Arguments
    ///
    /// * `self` (`&WeatherContent`) - Receiver state used by this operation. Borrowed without
    ///   changing it.
    /// * `display` (`&mut D`) - Mutable RGB565 draw target receiving the rendering operations.
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
    /// Propagates weather background, icon, or text drawing errors.
    fn render<D: DrawTarget<Color = Rgb565>>(&self, display: &mut D) -> Result<(), D::Error> {
        // Run the background operation with the supplied inputs.
        theme::background(display)?;
        // Run the rounded radius operation with the supplied inputs.
        theme::rounded_radius(
            display,
            Rectangle::new(Point::new(12, 320), Size::new(296, 116)),
            theme::WEATHER_CARD,
            12,
        )?;
        // Keep solid or gradient background used to blend antialiased pixels in this local variable
        // for the following operations.
        let backdrop = Backdrop::Solid(theme::WEATHER_CARD);
        // Run the draw on operation with the supplied inputs.
        weather_icons::draw_on(display, self.icon, Point::new(28, 334), true, backdrop)?;
        // Keep temperature text without its degree suffix, drawn separately from the unit in this
        // local variable for the following operations.
        let number = self.temperature.trim_end_matches('°');
        // Clips weather text and draws it against an explicit backdrop.
        text_on(display, &CLOCK, number, 136, 332, 132, backdrop)?;
        // Clips weather text and draws it against an explicit backdrop.
        text_on(
            display,
            &TITLE,
            &format!("°{}", self.unit),
            136 + CLOCK.width(number) as i32 + 4,
            352,
            40,
            backdrop,
        )?;
        // Keep formatted feels-like temperature label, clipped to fit the weather card in this
        // local variable for the following operations.
        let feels = theme::clipped(&self.feels, &TITLE, 272);
        // Clips weather text and draws it against an explicit backdrop.
        text_on(
            display,
            &TITLE,
            &feels,
            (320 - TITLE.width(&feels) as i32) / 2,
            396,
            272,
            backdrop,
        )?;
        // Centers a clipped weather label within the portrait page.
        center(display, &SMALL, &self.status, 38)?;
        // Return success after the required side effects are complete.
        Ok(())
    }
}
/// Computes a clockwise clock-dial position from an angular turn and radius.
///
/// # Arguments
///
/// * `turns` (`f64`) - Clockwise fraction of a full revolution; zero is twelve o'clock.
/// * `radius` (`f64`) - Distance from the dial center in pixels.
///
/// # Returns
///
/// `Point` - Rounded screen point around center (160, 132); zero turns points upward.
fn dial_point(turns: f64, radius: f64) -> Point {
    // Keep clockwise dial angle in radians, calculated from fractional turns in this local variable
    // for the following operations.
    let angle = turns * std::f64::consts::TAU;
    // Allocates two reusable strip buffers for differential rendering.
    Point::new(
        160 + (angle.sin() * radius).round() as i32,
        132 - (angle.cos() * radius).round() as i32,
    )
}
/// Computes interpolated clockwise hour, minute, and second hand rotations.
///
/// # Arguments
///
/// * `hour` (`u32`) - Hour of day in 0-23.
/// * `minute` (`u32`) - Minute within the hour in 0-59.
/// * `second` (`u32`) - Second within the minute in 0-59.
///
/// # Returns
///
/// `[f64; 3]` - Three fractional turns ordered hour, minute, second; hour rotation includes
/// minute and second progress.
fn hand_turns(hour: u32, minute: u32, second: u32) -> [f64; 3] {
    // Return the ordered sample/byte array as the value of this block.
    [
        ((hour % 12) as f64 + minute as f64 / 60.0 + second as f64 / 3600.0) / 12.0,
        (minute as f64 + second as f64 / 60.0) / 60.0,
        second as f64 / 60.0,
    ]
}
/// Draws the dial ticks and optional hour, minute, and second hands.
///
/// # Arguments
///
/// * `display` (`&mut D`) - Mutable RGB565 draw target receiving the rendering operations.
/// * `hands` (`Option<(u32, u32, u32)>`) - Optional local hour, minute, and second values;
///   None omits the hands.
///
/// # Type Parameters
///
/// * `D` - RGB565 drawing destination; its associated error is propagated by fallible
///   renderers.
///
/// # Returns
///
/// `Result<(), D::Error>` - Ok after drawing; Err from the draw target. Missing hand values
/// leave the dial without hands.
///
/// # Errors
///
/// Propagates dial and hand drawing failures.
fn render_analog<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    hands: Option<(u32, u32, u32)>,
) -> Result<(), D::Error> {
    // Send the prepared shape or text pixels to the current drawing destination.
    Circle::new(Point::new(86, 58), 149)
        .into_styled(PrimitiveStyle::with_stroke(theme::TEXT, 1))
        .draw(display)?;
    // Visit each entry in 0..60; the loop binding provides its value or index for this iteration.
    for tick in 0..60 {
        // Keep whether the current dial tick is a major five-minute/hour marker in this local
        // variable for the following operations.
        let hour = tick % 5 == 0;
        // Send the prepared shape or text pixels to the current drawing destination.
        Line::new(
            // Major five-minute ticks use a longer, thicker mark than the intervening minute ticks.
            dial_point(tick as f64 / 60.0, if hour { 62.0 } else { 68.0 }),
            dial_point(tick as f64 / 60.0, 71.0),
        )
        .into_styled(PrimitiveStyle::with_stroke(
            theme::TEXT,
            // Major five-minute ticks use a longer, thicker mark than the intervening minute ticks.
            if hour { 2 } else { 1 },
        ))
        .draw(display)?;
    }
    // Draw hands only when local time is available; the dial itself remains visible while waiting
    // for synchronization.
    if let Some((hour, minute, second)) = hands {
        // Keep fractional clockwise rotations for hour, minute, and second hands in this local
        // variable for the following operations.
        let turns = hand_turns(hour, minute, second);
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for (index, radius, width, color) in [
            (0, 40.0, 5, theme::TEXT),
            (1, 57.0, 3, theme::TEXT),
            (2, 64.0, 1, theme::rgb(0xffa64d)),
        ] {
            // Send the prepared shape or text pixels to the current drawing destination.
            Line::new(Point::new(160, 132), dial_point(turns[index], radius))
                .into_styled(PrimitiveStyle::with_stroke(color, width))
                .draw(display)?;
        }
        // Send the prepared shape or text pixels to the current drawing destination.
        Circle::new(Point::new(157, 129), 7)
            .into_styled(PrimitiveStyle::with_fill(theme::TEXT))
            .draw(display)?;
    }
    // Return success after the required side effects are complete.
    Ok(())
}

/// Renders clock content from the current application state without differential
/// comparison.
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
/// `Result<(), D::Error>` - Ok after drawing; Err from the draw target.
///
/// # Errors
///
/// Propagates clock rendering failures.
pub fn render_clock<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    app: &App,
) -> Result<(), D::Error> {
    // Compares clock strips with the last snapshot and transfers changed pixel runs.
    ClockContent::from_app(app).render(display)
}

/// Chooses the weather freshness, loading, or failure message.
///
/// # Arguments
///
/// * `app` (`&App`) - Application model supplying the active page, preferences, weather,
///   and presence state.
///
/// # Returns
///
/// `&'static str` - Static status text, or an empty string for an available current
/// forecast.
fn status(app: &App) -> &'static str {
    // Cached weather can still be displayed even after a refresh failure; its freshness determines
    // the status text.
    if app.weather.forecast.is_some() {
        // Warn that the retained forecast is stale rather than presenting it as newly fetched data.
        if app.weather.outdated(app.preferences.epoch) {
            "Weather outdated"
        } else {
            ""
        }
    // Without cached data, a failed request shows retry feedback instead of a loading message.
    } else if app.weather.failed {
        "Weather unavailable. Retrying..."
    } else {
        "Loading weather..."
    }
}
/// Returns the rectangle reserved for weekly or hourly overview updates.
///
/// # Arguments
///
/// None.
///
/// # Returns
///
/// `Rectangle` - Portrait pixel bounds for overview content excluding page controls.
pub fn overview_bounds() -> Rectangle {
    // Allocates two reusable strip buffers for differential rendering.
    Rectangle::new(Point::new(12, 38), Size::new(296, 391))
}
#[derive(PartialEq, Eq)]
struct OverviewRow {
    // Precipitation. Stored as Option<String>.
    precipitation: Option<String>,
    // Localized weekday or local hour identifying this forecast row. Stored as String.
    label: String,
    // Index of the retained Meteocons symbol representing the forecast condition. Stored as usize.
    icon: usize,
    // Formatted minimum temperature or precipitation probability, depending on the view. Stored as
    // String.
    left: String,
    // Formatted maximum or hourly temperature, depending on the view. Stored as String.
    right: String,
}
#[derive(PartialEq, Eq)]
struct OverviewContent {
    // Current state or measurement being prepared for display. Stored as WeatherContent.
    current: WeatherContent,
    // Whether the overview uses daily minimum/maximum rows rather than upcoming hours. Stored as
    // bool.
    week: bool,
    // Localized overview title distinguishing Week from Next 7 hours. Stored as String.
    title: String,
    // Localized heading identifying the left forecast value column. Stored as String.
    left_heading: String,
    // Unit or localized heading identifying the right forecast value column. Stored as String.
    right_heading: String,
    // Seven formatted forecast entries, with placeholders when the API has no matching data. Stored
    // as Vec<OverviewRow>.
    rows: Vec<OverviewRow>,
}
impl OverviewContent {
    /// Builds localized weekly or hourly table rows and current-weather content.
    ///
    /// # Arguments
    ///
    /// * `app` (`&App`) - Application model supplying the active page, preferences, weather,
    ///   and presence state.
    ///
    /// # Returns
    ///
    /// `Self` - Overview snapshot for seven rows, with placeholders for missing forecast
    /// entries.
    fn from_app(app: &App) -> Self {
        // Keep device preferences controlling location, language, units, brightness, and clock
        // format in this local variable for the following operations.
        let settings = &app.preferences.current;
        // Keep selected interface language used by weekday headings and labels in this local
        // variable for the following operations.
        let lang = settings.language;
        // Keep whether the overview uses daily minimum/maximum rows rather than upcoming hours in
        // this local variable for the following operations.
        let week = app.weather.view == View::Week;
        // Keep today in this local variable for the following operations.
        let today = weather::local(settings, app.preferences.epoch).map(|t| t.date_naive());
        // Keep seven formatted forecast entries, with placeholders when the API has no matching
        // data in this local variable for the following operations.
        let rows = (0..7)
            .map(|row| {
                // Daily rows select local dates and minimum/maximum temperatures; hourly rows
                // select future instants and precipitation/temperature.
                if week {
                    // Keep local calendar date used for daily forecast selection in this local
                    // variable for the following operations.
                    let date =
                        today.and_then(|d| d.checked_add_days(chrono::Days::new(row as u64)));
                    // Keep optional daily or hourly forecast record for the row being formatted in
                    // this local variable for the following operations.
                    let entry = app
                        .weather
                        .forecast
                        .as_ref()
                        .and_then(|f| today.and_then(|today| f.day(today, row)));
                    OverviewRow {
                        // Initialize precipitation from the supplied value.
                        precipitation: Some(
                            entry
                                .and_then(|d| d.precipitation_probability_mean)
                                .map(|p| format!("{}%", p.round() as u8))
                                .unwrap_or_else(|| "--%".into()),
                        ),
                        // Initialize localized weekday or local hour identifying this forecast row
                        // from the supplied value.
                        label: date
                            .map(|d| weekday(lang, d).to_owned())
                            .unwrap_or_else(|| "--".into()),
                        // Initialize index of the retained Meteocons symbol representing the
                        // forecast condition from the supplied value.
                        icon: weather::symbol(entry.and_then(|d| d.code), Some(1)),
                        // Initialize formatted minimum temperature or precipitation probability,
                        // depending on the view from the supplied value.
                        left: weather::degrees(entry.and_then(|d| d.low), settings.fahrenheit),
                        // Initialize formatted maximum or hourly temperature, depending on the view
                        // from the supplied value.
                        right: weather::degrees(entry.and_then(|d| d.high), settings.fahrenheit),
                    }
                } else {
                    // Keep optional daily or hourly forecast record for the row being formatted in
                    // this local variable for the following operations.
                    let entry = app.weather.forecast.as_ref().and_then(|f| {
                        // Produce the next fallible/optional stage, skipping it when an earlier
                        // stage is unavailable.
                        app.preferences
                            .epoch
                            .and_then(|now| f.upcoming(now).nth(row))
                    });
                    // Keep timestamp or formatted clock value used by the current view in this
                    // local variable for the following operations.
                    let time = entry.and_then(|h| weather::local(settings, Some(h.time)));
                    OverviewRow {
                        // Leave precipitation unavailable until a later operation supplies it.
                        precipitation: None,
                        // Initialize localized weekday or local hour identifying this forecast row
                        // from the supplied value.
                        label: time
                            .map(|t| {
                                // Select 24-hour formatting when enabled; otherwise keep the
                                // corresponding 12-hour layout.
                                t.format(if settings.clock_24h {
                                    "%H:%M"
                                } else {
                                    "%I:%M%p"
                                })
                                .to_string()
                            })
                            .unwrap_or_else(|| "--:--".into()),
                        // Initialize index of the retained Meteocons symbol representing the
                        // forecast condition from the supplied value.
                        icon: weather::symbol(
                            entry.and_then(|h| h.code),
                            entry.and_then(|h| h.is_day),
                        ),
                        // Initialize formatted minimum temperature or precipitation probability,
                        // depending on the view from the supplied value.
                        left: entry
                            .and_then(|h| h.precipitation)
                            .map(|p| format!("{p}%"))
                            .unwrap_or_else(|| "--%".into()),
                        // Initialize formatted maximum or hourly temperature, depending on the view
                        // from the supplied value.
                        right: weather::degrees(
                            entry.and_then(|h| h.temperature),
                            settings.fahrenheit,
                        ),
                    }
                }
            })
            .collect();
        Self {
            // Initialize current state or measurement being prepared for display from the supplied
            // value.
            current: WeatherContent::from_app(app),
            // Initialize whether the overview uses daily minimum/maximum rows rather than upcoming
            // hours from the supplied value.
            week,
            // Daily rows select local dates and minimum/maximum temperatures; hourly rows select
            // future instants and precipitation/temperature.
            title: i18n::tr(lang, if week { "Week" } else { "Next 7 hours" }).into(),
            // Daily rows select local dates and minimum/maximum temperatures; hourly rows select
            // future instants and precipitation/temperature.
            left_heading: if week { i18n::tr(lang, "Min") } else { "%" }.into(),
            // Daily rows select local dates and minimum/maximum temperatures; hourly rows select
            // future instants and precipitation/temperature.
            right_heading: if week {
                // Run the tr operation with the supplied inputs.
                i18n::tr(lang, "Max")
            // Convert displayed temperatures and headings to Fahrenheit while the cached
            // measurements remain Celsius.
            } else if settings.fahrenheit {
                "F"
            } else {
                "C"
            }
            .into(),
            // Initialize seven formatted forecast entries, with placeholders when the API has no
            // matching data from the supplied value.
            rows,
        }
    }
}
// 34-pixel vertical spacing allocated to each forecast row. This fixed value is shared by the
// operations below.
const OVERVIEW_ROW_HEIGHT: u32 = 34;
/// Returns the rounded forecast table's rectangle.
///
/// # Arguments
///
/// None.
///
/// # Returns
///
/// `Rectangle` - Portrait pixel bounds sized for seven overview rows.
fn overview_table_bounds() -> Rectangle {
    // Allocates two reusable strip buffers for differential rendering.
    Rectangle::new(
        Point::new(12, 187),
        Size::new(296, 7 * OVERVIEW_ROW_HEIGHT + 4),
    )
}
impl StripContent for OverviewContent {
    /// Draws the current-weather summary and seven-row daily or hourly forecast table.
    ///
    /// # Arguments
    ///
    /// * `self` (`&OverviewContent`) - Receiver state used by this operation. Borrowed without
    ///   changing it.
    /// * `display` (`&mut D`) - Mutable RGB565 draw target receiving the rendering operations.
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
    /// Propagates overview background, panel, icon, and text drawing errors.
    fn render<D: DrawTarget<Color = Rgb565>>(&self, display: &mut D) -> Result<(), D::Error> {
        // Keep drawing destination receiving RGB565 pixels in this local variable for the following
        // operations.
        let mut display = display.clipped(&overview_bounds());
        // Run the background operation with the supplied inputs.
        theme::background(&mut display)?;
        // Send the prepared shape or text pixels to the current drawing destination.
        weather_icons::draw(
            &mut display,
            self.current.icon,
            Point::new(12, 38),
            true,
            false,
        )?;
        // Clips weather text and draws it on the panel or gradient backdrop.
        text(
            &mut display,
            &FORECAST_TITLE,
            &format!("{}{}", self.current.temperature, self.current.unit),
            100,
            40,
            204,
            false,
        )?;
        // Clips weather text and draws it on the panel or gradient backdrop.
        text(
            &mut display,
            &FORECAST_BODY,
            &self.current.feels,
            100,
            80,
            204,
            false,
        )?;
        // Clips weather text and draws it on the panel or gradient backdrop.
        text(
            &mut display,
            &FORECAST_SMALL,
            &self.current.status,
            100,
            108,
            208,
            false,
        )?;
        // Clips weather text and draws it on the panel or gradient backdrop.
        text(
            &mut display,
            &FORECAST_BODY,
            &self.title,
            12,
            132,
            296,
            false,
        )?;
        // Choose the daily table's column layout rather than the hourly table's layout.
        if self.week {
            // Clips weather text and draws it on the panel or gradient backdrop.
            text(&mut display, &FORECAST_SMALL, "%", 112, 161, 68, false)?;
        }
        // Clips weather text and draws it on the panel or gradient backdrop.
        text(
            &mut display,
            &FORECAST_SMALL,
            &self.left_heading,
            // Choose the daily table's column layout rather than the hourly table's layout.
            if self.week { 184 } else { 174 },
            161,
            // Choose the daily table's column layout rather than the hourly table's layout.
            if self.week { 58 } else { 68 },
            false,
        )?;
        // Clips weather text and draws it on the panel or gradient backdrop.
        text(
            &mut display,
            &FORECAST_SMALL,
            &self.right_heading,
            246,
            161,
            58,
            false,
        )?;
        // Keep rounded rectangle containing the seven forecast rows in this local variable for the
        // following operations.
        let table = overview_table_bounds();
        // Run the rounded operation with the supplied inputs.
        theme::rounded(&mut display, table, theme::WEATHER_CARD)?;
        // Keep solid or gradient background used to blend antialiased pixels in this local variable
        // for the following operations.
        let backdrop = Backdrop::Solid(theme::WEATHER_CARD);
        // Visit each entry in enumerate result for the surrounding operation; the loop binding
        // provides its value or index for this iteration.
        for (row, entry) in self.rows.iter().enumerate() {
            // Keep vertical coordinate or current screen row in this local variable for the
            // following operations.
            let y = table.top_left.y + 2 + row as i32 * OVERVIEW_ROW_HEIGHT as i32;
            // Keep font selected to fit weekday names or longer hourly labels in this local
            // variable for the following operations.
            let label_font = if self.week {
                &FORECAST_BODY
            } else {
                &FORECAST_SMALL
            };
            // Clips weather text and draws it against an explicit backdrop.
            text_on(
                &mut display,
                label_font,
                &entry.label,
                // Choose the daily table's column layout rather than the hourly table's layout.
                if self.week { 16 } else { 20 },
                y + ((OVERVIEW_ROW_HEIGHT - label_font.height) / 2) as i32,
                // Choose the daily table's column layout rather than the hourly table's layout.
                if self.week { 56 } else { 112 },
                backdrop,
            )?;
            // Run the draw on operation with the supplied inputs.
            weather_icons::draw_on(
                &mut display,
                entry.icon,
                // Choose the daily table's column layout rather than the hourly table's layout.
                Point::new(if self.week { 76 } else { 134 }, y + 1),
                false,
                backdrop,
            )?;
            // Keep vertically centered baseline position of the forecast values in their row in
            // this local variable for the following operations.
            let value_y = y + ((OVERVIEW_ROW_HEIGHT - FORECAST_BODY.height) / 2) as i32;
            // The weekly mean-probability column is optional; hourly rows have their probability in
            // the left value column instead.
            if let Some(precipitation) = &entry.precipitation {
                // Clips weather text and draws it against an explicit backdrop.
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
            // Clips weather text and draws it against an explicit backdrop.
            text_on(
                &mut display,
                &FORECAST_BODY,
                &entry.left,
                // Choose the daily table's column layout rather than the hourly table's layout.
                if self.week { 184 } else { 174 },
                value_y,
                // Choose the daily table's column layout rather than the hourly table's layout.
                if self.week { 58 } else { 68 },
                backdrop,
            )?;
            // Clips weather text and draws it against an explicit backdrop.
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
        // Return success after the required side effects are complete.
        Ok(())
    }
}

/// Draws the complete weather page, location, selected view, attribution, presence, and
/// settings gear.
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
/// `Result<(), D::Error>` - Ok after drawing; Err from the draw target.
///
/// # Errors
///
/// Propagates page, clock, forecast, icon, or control drawing failures.
pub fn render<D: DrawTarget<Color = Rgb565>>(display: &mut D, app: &App) -> Result<(), D::Error> {
    // Run the background operation with the supplied inputs.
    theme::background(display)?;
    // Keep device preferences controlling location, language, units, brightness, and clock format
    // in this local variable for the following operations.
    let settings = &app.preferences.current;
    // Keep whether the selected page is Week or Hours rather than the Clock view in this local
    // variable for the following operations.
    let overview = app.weather.view != View::Clock;
    // Clips weather text and draws it on the panel or gradient backdrop.
    text(
        display,
        &BODY,
        &settings.location.display_name(),
        12,
        12,
        256,
        false,
    )?;
    // Run the render presence operation with the supplied inputs.
    crate::ui::render_presence(display, app)?;
    // Render the seven-row Week/Hours overview instead of the analog/digital Clock page.
    if overview {
        // Compares clock strips with the last snapshot and transfers changed pixel runs.
        OverviewContent::from_app(app).render(display)?;
    } else {
        // Renders clock content from the current application state without differential comparison.
        render_clock(display, app)?;
        // Keep formatted content snapshot independent of the physical display transport in this
        // local variable for the following operations.
        let content = WeatherContent::from_app(app);
        // Visit each entry in returns the disjoint rectangles reserved for status and
        // current-weather updates; the loop binding provides its value or index for this iteration.
        for area in weather_bounds() {
            // Compares clock strips with the last snapshot and transfers changed pixel runs.
            content.render(&mut display.clipped(&area))?;
        }
    }
    // Clips weather text and draws it on the panel or gradient backdrop.
    text(
        display,
        &SMALL,
        "Open-Meteo | CC BY 4.0",
        12,
        451,
        248,
        false,
    )?;
    // Composites the retained Tabler settings alpha mask on the page gradient.
    draw_settings_gear(display)?;
    // Return success after the required side effects are complete.
    Ok(())
}
/// Composites the retained Tabler settings alpha mask on the page gradient.
///
/// # Arguments
///
/// * `display` (`&mut D`) - Mutable RGB565 draw target receiving the rendering operations.
///
/// # Type Parameters
///
/// * `D` - RGB565 drawing destination; its associated error is propagated by fallible
///   renderers.
///
/// # Returns
///
/// `Result<(), D::Error>` - Ok after drawing the clipped gear; Err from the draw target.
///
/// # Errors
///
/// Propagates contiguous gear pixel transfer errors; the asset retains its MIT notice.
fn draw_settings_gear<D: DrawTarget<Color = Rgb565>>(display: &mut D) -> Result<(), D::Error> {
    // MIT Tabler Icons derivative; see assets/ui/NOTICE.txt and MIT.txt.
    // Retained 38-by-38 tabler settings-gear opacity mask; each byte is alpha coverage. This fixed
    // value is shared by the operations below.
    const ALPHA: &[u8] = include_bytes!("../assets/ui/settings-38.alpha");
    // Keep screen-coordinate rectangle currently being rendered or transferred in this local
    // variable for the following operations.
    let area = Rectangle::new(Point::new(269, 439), Size::new(38, 38))
        .intersection(&display.bounding_box());
    // Keep row-major RGB565 colour buffer or iterator in this local variable for the following
    // operations.
    let pixels = area.points().map(|p| {
        // Keep per-pixel gear opacity in 0-255, converted to four-bit blend coverage in this local
        // variable for the following operations.
        let alpha = ALPHA[((p.y - 439) * 38 + p.x - 269) as usize];
        // Run the blend operation with the supplied inputs.
        theme::blend(
            theme::background_at(p.y),
            theme::TEXT,
            ((u16::from(alpha) * 15 + 127) / 255) as u8,
        )
    });
    // Transfer a composed row-major colour rectangle, avoiding a visible clear-before-draw pass.
    display.fill_contiguous(&area, pixels)
}

///////////////////////////////////////////////////////////////////////////////////////
//                                    UNIT TESTS                                     //
///////////////////////////////////////////////////////////////////////////////////////

/// Tests for forecast label layout, probability formatting, and clock hand geometry.
#[cfg(test)]
mod analog_tests {
    use super::*;
    /// Verifies weekly probability rounding, missing-value placeholders, and temperature-unit
    /// independence.
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
    fn weekly_probability_rounding_missing_values_and_units() {
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App::default();
        // Set view to View Week.
        app.weather.view = View::Week;
        // Set IANA timezone identifier used for local clocks and forecast dates to into result for
        // the surrounding operation.
        app.preferences.current.location.timezone = "UTC".into();
        // Set UTC Unix timestamp in seconds to an available value for 1_790_899_200.
        app.preferences.epoch = Some(1_790_899_200);
        // Set validated current, daily, and hourly weather data to an available value for the
        // required fixture or invariant value, panicking if unavailable.
        app.weather.forecast =
            Some(weather::parse(include_bytes!("../tests/fixtures/weather.json")).unwrap());
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for fahrenheit in [false, true] {
            // Set whether displayed temperatures are converted from cached Celsius to Fahrenheit to
            // whether displayed temperatures are converted from cached Celsius to Fahrenheit.
            app.preferences.current.fahrenheit = fahrenheit;
            // Run each listed scenario or target in the declared order; the loop variables select
            // the data for that case.
            for (probability, expected) in [
                (Some(0.0), "0%"),
                (Some(12.5), "13%"),
                (Some(35.4), "35%"),
                (Some(100.0), "100%"),
                (None, "--%"),
            ] {
                // Set precipitation probability mean to probability.
                app.weather.forecast.as_mut().unwrap().days[0].precipitation_probability_mean =
                    probability;
                // Verify that as deref result for the surrounding operation exactly matches an
                // available value for reference framebuffer produced by rendering the complete
                // page.
                assert_eq!(
                    OverviewContent::from_app(&app).rows[0]
                        .precipitation
                        .as_deref(),
                    Some(expected)
                );
            }
        }
        // Set validated current, daily, and hourly weather data to no available value.
        app.weather.forecast = None;
        // Verify every entry satisfies the required condition. A violation means the tested
        // behavior is incorrect.
        assert!(OverviewContent::from_app(&app)
            .rows
            .iter()
            .all(|row| row.precipitation.as_deref() == Some("--%")));
        // Set view to View Hours.
        app.weather.view = View::Hours;
        // Verify every entry satisfies the required condition. A violation means the tested
        // behavior is incorrect.
        assert!(OverviewContent::from_app(&app)
            .rows
            .iter()
            .all(|row| row.precipitation.is_none()));
    }

    /// Verifies that localized headings, weekday labels, temperatures, and hourly times fit the
    /// larger forecast fonts.
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
    fn forecast_labels_and_values_fit_the_larger_fonts() {
        // Visit each entry in Language ALL; the loop binding provides its value or index for this
        // iteration.
        for language in Language::ALL {
            // Verify width result for the surrounding operation does not exceed 296. A violation
            // means the tested behavior is incorrect.
            assert!(
                FORECAST_BODY.width(i18n::tr(language, "Next 7 hours")) <= 296,
                "title does not fit: {language:?}"
            );
            // Visit each entry in 0..7; the loop binding provides its value or index for this
            // iteration.
            for day in 0..7 {
                // Keep local calendar date used for daily forecast selection in this local variable
                // for the following operations.
                let date = NaiveDate::from_ymd_opt(2026, 9, 28).unwrap() + chrono::Days::new(day);
                // Verify width result for the surrounding operation does not exceed 56. A violation
                // means the tested behavior is incorrect.
                assert!(
                    FORECAST_BODY.width(weekday(language, date)) <= 56,
                    "{}: {}px",
                    weekday(language, date),
                    FORECAST_BODY.width(weekday(language, date))
                );
            }
        }
        // Verify width result for the surrounding operation does not exceed 68. A violation means
        // the tested behavior is incorrect.
        assert!(FORECAST_BODY.width("100%") <= 68);
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for temperature in ["14°C", "-18°C", "100°F", "--°C"] {
            // Keep localized weekday or local hour identifying this forecast row in this local
            // variable for the following operations.
            let label = format!(
                "{} {temperature}",
                i18n::tr(Language::Ukrainian, "Feels like")
            );
            // Verify width result for the surrounding operation does not exceed 204. A violation
            // means the tested behavior is incorrect.
            assert!(
                FORECAST_BODY.width(&label) <= 204,
                "Ukrainian feels-like value is clipped: {label}"
            );
        }
        // Verify width result for the surrounding operation does not exceed 248. A violation means
        // the tested behavior is incorrect.
        assert!(SMALL.width("Open-Meteo | CC BY 4.0") <= 248);
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for fahrenheit in [false, true] {
            // Run each listed scenario or target in the declared order; the loop variables select
            // the data for that case.
            for temperature in [None, Some(-40.0), Some(-25.0), Some(0.0), Some(55.0)] {
                // Verify width result for the surrounding operation does not exceed 58. A violation
                // means the tested behavior is incorrect.
                assert!(FORECAST_BODY.width(&weather::degrees(temperature, fahrenheit)) <= 58);
            }
        }
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App::default();
        // Set view to View Hours.
        app.weather.view = View::Hours;
        // Set UTC Unix timestamp in seconds to an available value for 1_790_899_200.
        app.preferences.epoch = Some(1_790_899_200);
        // Set validated current, daily, and hourly weather data to an available value for the
        // required fixture or invariant value, panicking if unavailable.
        app.weather.forecast =
            Some(weather::parse(include_bytes!("../tests/fixtures/weather.json")).unwrap());
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for zone in [
            "Europe/Berlin",
            "Europe/London",
            "America/New_York",
            "Asia/Kathmandu",
        ] {
            // Set IANA timezone identifier used for local clocks and forecast dates to into result
            // for the surrounding operation.
            app.preferences.current.location.timezone = zone.into();
            // Run each listed scenario or target in the declared order; the loop variables select
            // the data for that case.
            for clock_24h in [false, true] {
                // Set whether local time uses 24-hour rather than 12-hour formatting to whether
                // local time uses 24-hour rather than 12-hour formatting.
                app.preferences.current.clock_24h = clock_24h;
                // Keep seven formatted forecast entries, with placeholders when the API has no
                // matching data in this local variable for the following operations.
                let rows = OverviewContent::from_app(&app).rows;
                // Verify that the length of seven formatted forecast entries, with placeholders
                // when the API has no matching data exactly matches 7.
                assert_eq!(rows.len(), 7);
                // Visit each entry in seven formatted forecast entries, with placeholders when the
                // API has no matching data; the loop binding provides its value or index for this
                // iteration.
                for row in rows {
                    // Verify width result for the surrounding operation does not exceed 112. A
                    // violation means the tested behavior is incorrect.
                    assert!(FORECAST_SMALL.width(&row.label) <= 112, "{}", row.label);
                    // Verify every entry satisfies the required condition. A violation means the
                    // tested behavior is incorrect.
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
        // Verify vertical extent in pixels does not exceed 34-pixel vertical spacing allocated to
        // each forecast row. A violation means the tested behavior is incorrect.
        assert!(FORECAST_SMALL.height <= OVERVIEW_ROW_HEIGHT);
        // Verify vertical extent in pixels does not exceed 34-pixel vertical spacing allocated to
        // each forecast row. A violation means the tested behavior is incorrect.
        assert!(FORECAST_BODY.height <= OVERVIEW_ROW_HEIGHT);
        // Verify vertical coordinate or current screen row is less than 442. A violation means the
        // tested behavior is incorrect.
        assert!(overview_table_bounds().bottom_right().unwrap().y < 442);
    }

    /// Verifies clockwise cardinal dial positions and fractional hour/minute hand
    /// interpolation.
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
    fn clockwise_cardinal_positions_and_interpolated_hands() {
        // Verify that computes a clockwise clock-dial position from an angular turn and radius
        // exactly matches allocates two reusable strip buffers for differential rendering.
        assert_eq!(dial_point(0.0, 64.0), Point::new(160, 68));
        // Verify that computes a clockwise clock-dial position from an angular turn and radius
        // exactly matches allocates two reusable strip buffers for differential rendering.
        assert_eq!(dial_point(0.25, 64.0), Point::new(224, 132));
        // Verify that computes a clockwise clock-dial position from an angular turn and radius
        // exactly matches allocates two reusable strip buffers for differential rendering.
        assert_eq!(dial_point(0.5, 64.0), Point::new(160, 196));
        // Verify that computes a clockwise clock-dial position from an angular turn and radius
        // exactly matches allocates two reusable strip buffers for differential rendering.
        assert_eq!(dial_point(0.75, 64.0), Point::new(96, 132));
        // Verify that computes interpolated clockwise hour, minute, and second hand rotations
        // exactly matches computes interpolated clockwise hour, minute, and second hand rotations.
        assert_eq!(hand_turns(0, 0, 0), hand_turns(12, 0, 0));
        // Verify that the selected entry from computes interpolated clockwise hour, minute, and
        // second hand rotations exactly matches 0.25 (the numeric value used here).
        assert_eq!(hand_turns(3, 0, 0)[0], 0.25);
        // Keep snapshot taken before the operation so later changes can be compared in this local
        // variable for the following operations.
        let before = hand_turns(11, 59, 59);
        // Verify every entry satisfies the required condition. A violation means the tested
        // behavior is incorrect.
        assert!(before.iter().all(|angle| *angle < 1.0 && *angle > 0.98));
        // Verify that computes interpolated clockwise hour, minute, and second hand rotations
        // exactly matches the ordered sample/byte array.
        assert_eq!(hand_turns(12, 0, 0), [0.0; 3]);
        // Keep interpolated hand positions at the test's quarter-past time in this local variable
        // for the following operations.
        let quarter = hand_turns(3, 15, 30);
        // Verify abs result for the surrounding operation is less than 1e-12 (the numeric value
        // used here). A violation means the tested behavior is incorrect.
        assert!((quarter[0] - 3.2583333333333333 / 12.0).abs() < 1e-12);
        // Verify that the selected entry from interpolated hand positions at the test's
        // quarter-past time exactly matches 15.5 (the numeric value used here) divided by 60.0 (the
        // numeric value used here).
        assert_eq!(quarter[1], 15.5 / 60.0);
        // Verify that the selected entry from interpolated hand positions at the test's
        // quarter-past time exactly matches 0.5 (the numeric value used here).
        assert_eq!(quarter[2], 0.5);
    }
}
