// ============================================================================= //
// File          : weather_icons.rs                                              //
// License       : GPL-3.0-only                                                  //
// Created       : 2026-10-04                                                    //
// Last modified : 2026-10-04                                                    //
// Author        : Daniel Kharlamov                                              //
// Last editor   : Daniel Kharlamov <daniel.kharlamov@googlemail.com>            //
// ============================================================================= //
//                                                                               //
// Summary:                                                                      //
// --------                                                                      //
// Embedded Meteocons weather bitmap rendering.                                  //
//                                                                               //
// Module:                                                                       //
// -------                                                                       //
// Selects embedded weather symbols at small and large sizes, decodes RGB565     //
// colour and alpha coverage, and draws them against solid or gradient           //
// backdrops. The bitmap derivatives retain their Meteocons MIT licence and      //
// attribution notice.                                                           //
//                                                                               //
// Note:                                                                         //
// -----                                                                         //
// MIT-licensed Meteocons bitmap derivatives; see assets/weather/NOTICE.txt.     //
// ============================================================================= //

//! Embedded Meteocons weather bitmap rendering.

use crate::theme;
use embedded_graphics::{pixelcolor::Rgb565, prelude::*, primitives::Rectangle};
// Retain thirteen 32-pixel Meteocons assets in weather-code index order; each pixel stores colour
// and alpha.
const ICONS_32: [&[u8]; 13] = [
    include_bytes!("../assets/weather/clear-day-32.bitmap"),
    include_bytes!("../assets/weather/clear-night-32.bitmap"),
    include_bytes!("../assets/weather/partly-cloudy-day-32.bitmap"),
    include_bytes!("../assets/weather/partly-cloudy-night-32.bitmap"),
    include_bytes!("../assets/weather/overcast-32.bitmap"),
    include_bytes!("../assets/weather/fog-32.bitmap"),
    include_bytes!("../assets/weather/drizzle-32.bitmap"),
    include_bytes!("../assets/weather/sleet-32.bitmap"),
    include_bytes!("../assets/weather/rain-32.bitmap"),
    include_bytes!("../assets/weather/snow-32.bitmap"),
    include_bytes!("../assets/weather/thunderstorms-32.bitmap"),
    include_bytes!("../assets/weather/thunderstorms-hail-32.bitmap"),
    include_bytes!("../assets/weather/not-available-32.bitmap"),
];
// Retain the corresponding thirteen 72-pixel assets for current-weather cards.
const ICONS_72: [&[u8]; 13] = [
    include_bytes!("../assets/weather/clear-day-72.bitmap"),
    include_bytes!("../assets/weather/clear-night-72.bitmap"),
    include_bytes!("../assets/weather/partly-cloudy-day-72.bitmap"),
    include_bytes!("../assets/weather/partly-cloudy-night-72.bitmap"),
    include_bytes!("../assets/weather/overcast-72.bitmap"),
    include_bytes!("../assets/weather/fog-72.bitmap"),
    include_bytes!("../assets/weather/drizzle-72.bitmap"),
    include_bytes!("../assets/weather/sleet-72.bitmap"),
    include_bytes!("../assets/weather/rain-72.bitmap"),
    include_bytes!("../assets/weather/snow-72.bitmap"),
    include_bytes!("../assets/weather/thunderstorms-72.bitmap"),
    include_bytes!("../assets/weather/thunderstorms-hail-72.bitmap"),
    include_bytes!("../assets/weather/not-available-72.bitmap"),
];
/// Draws a retained Meteocons bitmap against the panel or gradient background.
///
/// # Arguments
///
/// * `display` (`&mut D`) - Mutable RGB565 draw target receiving the rendering operations.
/// * `index` (`usize`) - Weather symbol index in 0-12; larger values are clamped to the
///   unavailable symbol.
/// * `origin` (`Point`) - Top-left drawing origin in screen pixels.
/// * `large` (`bool`) - Whether to use the 72-pixel icon rather than the 32-pixel icon.
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
/// `Result<(), D::Error>` - Ok after drawing the clipped icon; Err from the draw target.
///
/// # Errors
///
/// Propagates contiguous icon pixel transfer failures.
pub fn draw<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    index: usize,
    origin: Point,
    large: bool,
    panel: bool,
) -> Result<(), D::Error> {
    // Composites a small or large Meteocons bitmap against an explicit backdrop.
    draw_on(
        display,
        index,
        origin,
        large,
        // Check whether panel.
        if panel {
            // Run the Solid operation with the supplied inputs.
            theme::Backdrop::Solid(theme::PANEL)
        } else {
            theme::Backdrop::Gradient
        },
    )
}
/// Composites a small or large Meteocons bitmap against an explicit backdrop.
///
/// # Arguments
///
/// * `display` (`&mut D`) - Mutable RGB565 draw target receiving the rendering operations.
/// * `index` (`usize`) - Weather symbol index in 0-12; larger values are clamped to the
///   unavailable symbol.
/// * `origin` (`Point`) - Top-left drawing origin in screen pixels.
/// * `large` (`bool`) - Whether to use the 72-pixel icon rather than the 32-pixel icon.
/// * `backdrop` (`theme::Backdrop`) - Solid colour or vertical gradient used behind
///   antialiased pixels.
///
/// # Type Parameters
///
/// * `D` - RGB565 drawing destination; its associated error is propagated by fallible
///   renderers.
///
/// # Returns
///
/// `Result<(), D::Error>` - Ok after drawing; Err from the draw target. Indices above 12
/// select the unavailable symbol.
///
/// # Errors
///
/// Propagates contiguous bitmap transfer failures.
pub fn draw_on<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    index: usize,
    origin: Point,
    large: bool,
    backdrop: theme::Backdrop,
) -> Result<(), D::Error> {
    // Keep selected icon dimension in pixels: 32 for small icons or 72 for large icons in this
    // local variable for the following operations.
    let size = if large { 72 } else { 32 };
    // Keep wire response or packed asset bytes decoded by the surrounding parser in this local
    // variable for the following operations.
    let data = if large {
        ICONS_72[index.min(12)]
    } else {
        ICONS_32[index.min(12)]
    };
    // Keep screen-coordinate rectangle limiting the drawing operation in this local variable for
    // the following operations.
    let bounds =
        Rectangle::new(origin, Size::new(size, size)).intersection(&display.bounding_box());
    // Keep row-major RGB565 colour buffer or iterator in this local variable for the following
    // operations.
    let pixels = (bounds.top_left.y..bounds.top_left.y + bounds.size.height as i32).flat_map(|y| {
        // Produce the transformed value or entries produced by the closure.
        (bounds.top_left.x..bounds.top_left.x + bounds.size.width as i32).map(move |x| {
            // Convert icon-local x/y into a three-byte pixel record offset: two little-endian
            // RGB565 bytes followed by alpha.
            let offset = (((y - origin.y) as u32 * size + (x - origin.x) as u32) * 3) as usize;
            // Decode the stored little-endian RGB565 colour before extracting its red, green, and
            // blue channels.
            let color = u16::from_le_bytes([data[offset], data[offset + 1]]);
            // Keep RGB565 foreground channels decoded from the selected bitmap pixel in this local
            // variable for the following operations.
            let foreground = Rgb565::new(
                ((color >> 11) & 31) as u8,
                ((color >> 5) & 63) as u8,
                (color & 31) as u8,
            );
            // Run the blend operation with the supplied inputs.
            theme::blend(
                backdrop.at(y),
                foreground,
                ((data[offset + 2] as u16 * 15 + 127) / 255) as u8,
            )
        })
    });
    // Transfer a composed row-major colour rectangle, avoiding a visible clear-before-draw pass.
    display.fill_contiguous(&bounds, pixels)
}
