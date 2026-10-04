// ============================================================================= //
// File          : theme.rs                                                      //
// License       : MIT                                                           //
// Created       : 2026-10-04                                                    //
// Last modified : 2026-10-04                                                    //
// Author        : Daniel Kharlamov                                              //
// Last editor   : Daniel Kharlamov <daniel.kharlamov@googlemail.com>            //
// ============================================================================= //
//                                                                               //
// Summary:                                                                      //
// --------                                                                      //
// Shared UI colours, backgrounds, and bitmap font rendering.                    //
//                                                                               //
// Module:                                                                       //
// -------                                                                       //
// Defines the interface palette, gradient backgrounds, rounded panels, and      //
// backdrop blending. Renders proportional antialiased bitmap fonts with kerning //
// and Unicode glyph lookup, and provides text clipping and wrapping. Font       //
// assets retain their own licences and attribution notices.                     //
// ============================================================================= //

//! UI colours and small, shared proportional-font rendering helpers.
use embedded_graphics::{
    pixelcolor::{Rgb565, Rgb888},
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle, RoundedRectangle},
};

// Solid background colour behind current weather and forecast tables. This fixed value is shared by
// the operations below.
pub const WEATHER_CARD: Rgb565 = rgb(0x397ca8);
// Solid background colour shared by interactive controls. This fixed value is shared by the
// operations below.
pub const PANEL: Rgb565 = rgb(0x5e9bc8);
// Primary foreground colour for readable labels. This fixed value is shared by the operations
// below.
pub const TEXT: Rgb565 = rgb(0xffffff);
// Secondary foreground colour used for less prominent content. This fixed value is shared by the
// operations below.
pub const SECONDARY: Rgb565 = rgb(0xe4ffff);
// Subdued colour used to distinguish disabled or secondary text. This fixed value is shared by the
// operations below.
pub const MUTED: Rgb565 = rgb(0xb9ecff);
// Shared four-pixel corner radius for interface panels. This fixed value is shared by the
// operations below.
pub const RADIUS: u32 = 4;

/// Converts a packed 24-bit RGB colour to rounded RGB565 channel values.
///
/// # Arguments
///
/// * `hex` (`u32`) - Packed 0xRRGGBB colour; bits above the low 24 are ignored.
///
/// # Returns
///
/// `Rgb565` - RGB565 colour using the low 24 bits of the supplied value.
pub const fn rgb(hex: u32) -> Rgb565 {
    // Construct the value from the supplied configuration or coordinates for use by the surrounding
    // operation.
    Rgb565::new(
        ((((hex >> 16) & 255) * 31 + 127) / 255) as u8,
        ((((hex >> 8) & 255) * 63 + 127) / 255) as u8,
        (((hex & 255) * 31 + 127) / 255) as u8,
    )
}

/// Samples the vertical interface gradient at a screen row.
///
/// # Arguments
///
/// * `y` (`i32`) - Vertical screen coordinate in pixels.
///
/// # Returns
///
/// `Rgb565` - RGB565 background colour; rows outside 0-479 are clamped to the endpoint
/// colours.
pub fn background_at(y: i32) -> Rgb565 {
    // Keep vertical coordinate or current screen row in this local variable for the following
    // operations.
    let y = y.clamp(0, 479) as u32;
    // Keep per-channel interpolation calculation used for a gradient or opacity blend in this local
    // variable for the following operations.
    let channel = |a: u32, b: u32| ((a * (479 - y) + b * y + 239) / 479) as u8;
    // Produce into result for the surrounding operation.
    Rgb888::new(channel(76, 166), channel(140, 205), channel(185, 236)).into()
}

/// Fills the draw target bounds with the vertical interface gradient.
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
/// `Result<(), D::Error>` - Ok after drawing all visible rows; Err from the draw target.
///
/// # Errors
///
/// Propagates the first failed row fill.
pub fn background<D: DrawTarget<Color = Rgb565>>(display: &mut D) -> Result<(), D::Error> {
    // Keep screen-coordinate rectangle limiting the drawing operation in this local variable for
    // the following operations.
    let bounds = display.bounding_box();
    // Visit each entry in the value computed by the following expression; the loop binding provides
    // its value or index for this iteration.
    for y in bounds.top_left.y..bounds.top_left.y + bounds.size.height as i32 {
        // Fill the specified rectangle with one colour, propagating target errors when applicable.
        display.fill_solid(
            &Rectangle::new(
                Point::new(bounds.top_left.x, y),
                Size::new(bounds.size.width, 1),
            ),
            background_at(y),
        )?;
    }
    // Return success after the required side effects are complete.
    Ok(())
}

/// Draws a solid panel using the shared corner radius.
///
/// # Arguments
///
/// * `display` (`&mut D`) - Mutable RGB565 draw target receiving the rendering operations.
/// * `bounds` (`Rectangle`) - Rectangle to draw or update in screen pixels.
/// * `fill` (`Rgb565`) - RGB565 solid panel colour.
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
/// Propagates panel drawing failures.
pub fn rounded<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    bounds: Rectangle,
    fill: Rgb565,
) -> Result<(), D::Error> {
    // Draws a solid panel with the supplied corner radius.
    rounded_radius(display, bounds, fill, RADIUS)
}

/// Draws a solid panel with the supplied corner radius.
///
/// # Arguments
///
/// * `display` (`&mut D`) - Mutable RGB565 draw target receiving the rendering operations.
/// * `bounds` (`Rectangle`) - Rectangle to draw or update in screen pixels.
/// * `fill` (`Rgb565`) - RGB565 solid panel colour.
/// * `radius` (`u32`) - Equal corner radius in pixels.
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
/// Propagates rounded-rectangle drawing failures.
pub fn rounded_radius<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    bounds: Rectangle,
    fill: Rgb565,
    radius: u32,
) -> Result<(), D::Error> {
    // Composes clipped antialiased text against its backdrop and sends the final pixel band.
    RoundedRectangle::with_equal_corners(bounds, Size::new(radius, radius))
        .into_styled(PrimitiveStyle::with_fill(fill))
        .draw(display)?;
    // Return success after the required side effects are complete.
    Ok(())
}

#[derive(Clone, Copy)]
pub enum Backdrop {
    // Sample the vertical page gradient at each pixel row.
    Gradient,
    // Blend antialiased pixels against a fixed RGB565 panel colour.
    Solid(Rgb565),
}
impl Backdrop {
    /// Samples a solid or gradient backdrop at a given row.
    ///
    /// # Arguments
    ///
    /// * `self` (`Backdrop`) - Solid or gradient background selection. Passed by value.
    /// * `y` (`i32`) - Vertical screen coordinate in pixels.
    ///
    /// # Returns
    ///
    /// `Rgb565` - Solid backdrop colour or the interface gradient colour at that row.
    pub(crate) fn at(self, y: i32) -> Rgb565 {
        // Choose the appropriate path for self; each arm handles one supported case.
        match self {
            // Handle the Self Gradient case: Samples the vertical interface gradient at a screen
            // row.
            Self::Gradient => background_at(y),
            // Handle the Self Solid(color) case: apply the state-specific behavior shown here.
            Self::Solid(color) => color,
        }
    }
}

// Font data remains OFL-1.1, separately from the MIT firmware code. See assets/fonts/NOTICE.txt and OFL.txt.
pub struct Font {
    // Packed sixteen-byte glyph records sorted by Unicode codepoint. Stored as &'static [u8].
    glyphs: &'static [u8],
    // Packed four-bit glyph coverage values retained under the font asset licence. Stored as
    // &'static [u8].
    bitmap: &'static [u8],
    // Packed three-byte pair adjustments in sixteenth-pixel units. Stored as &'static [u8].
    kerning: &'static [u8],
    // Full vertical pixel band reserved when drawing this font. Stored as u32.
    pub height: u32,
    // Vertical baseline within the font's drawing band. Stored as i32.
    baseline: i32,
}

macro_rules! font {
    ($size:literal, $height:literal, $baseline:literal) => {
        Font {
            glyphs: include_bytes!(concat!("../assets/fonts/montserrat-", $size, ".glyphs")),
            bitmap: include_bytes!(concat!("../assets/fonts/montserrat-", $size, ".bitmap")),
            kerning: include_bytes!(concat!("../assets/fonts/montserrat-", $size, ".kerning")),
            height: $height,
            baseline: $baseline,
        }
    };
}
// Montserrat subset used for compact labels at 12-pixel source size. This fixed value is shared by
// the operations below.
pub const SMALL: Font = font!("12", 16, 13);
// Montserrat subset used for ordinary interface text at 14-pixel source size. This fixed value is
// shared by the operations below.
pub const BODY: Font = font!("14", 19, 15);
// Montserrat subset used for prominent labels at 20-pixel source size. This fixed value is shared
// by the operations below.
pub const TITLE: Font = font!("20", 25, 20);
// 19-pixel font subset used for compact forecast annotations. This fixed value is shared by the
// operations below.
pub const FORECAST_SMALL: Font = font!("19", 23, 19);
// 22-pixel font subset used for forecast rows and headings. This fixed value is shared by the
// operations below.
pub const FORECAST_BODY: Font = font!("22", 26, 21);
// 31-pixel font subset used for the current forecast temperature. This fixed value is shared by the
// operations below.
pub const FORECAST_TITLE: Font = font!("31", 38, 31);
// 42-pixel font subset used for digital clock digits. This fixed value is shared by the operations
// below.
pub const CLOCK: Font = font!("42", 49, 40);

struct Glyph {
    // Glyph record index or row-major destination pixel index in the surrounding operation. Stored
    // as usize.
    index: usize,
    // Current starting position within a paginated list. Stored as usize.
    offset: usize,
    // Advance. Stored as i32.
    advance: i32,
    // Horizontal extent in pixels. Stored as usize.
    width: usize,
    // Full vertical pixel band reserved when drawing this font. Stored as usize.
    height: usize,
    // Horizontal coordinate or current position along the row. Stored as i32.
    x: i32,
    // Vertical coordinate or current screen row. Stored as i32.
    y: i32,
}

impl Font {
    /// Looks up a Unicode glyph and falls back to the question-mark glyph when absent.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Font`) - Embedded glyph, bitmap, and kerning data with layout metrics.
    ///   Borrowed without changing it.
    /// * `ch` (`char`) - Unicode scalar value to process.
    ///
    /// # Returns
    ///
    /// `Glyph` - Decoded glyph metrics and bitmap offset from the embedded font records.
    ///
    /// # Panics
    ///
    /// Panics if embedded glyph records are malformed or the required question-mark fallback is
    /// missing.
    fn glyph(&self, ch: char) -> Glyph {
        // Keep glyph record index or row-major destination pixel index in the surrounding operation
        // in this local variable for the following operations.
        let index = search(self.glyphs, 16, ch as u32, |record| {
            // Run the from le bytes operation with the supplied inputs.
            u32::from_le_bytes(record[..4].try_into().unwrap())
        })
        .unwrap_or_else(|_| {
            // ASCII '?' is always present. Do not replace accented Latin with blanks.
            // Require the value guaranteed by this test fixture or internal invariant; unexpected
            // absence panics.
            self.glyphs
                .chunks_exact(16)
                .position(|record| {
                    // Return the integer decoded from little-endian bytes equals the numeric value
                    // converted to the required arithmetic or indexing type as the value of this
                    // block.
                    u32::from_le_bytes(record[..4].try_into().unwrap()) == '?' as u32
                })
                .unwrap()
        });
        // Keep packed sixteen-byte metadata slice for the selected glyph in this local variable for
        // the following operations.
        let record = &self.glyphs[index * 16..][..16];
        Glyph {
            // Initialize glyph record index or row-major destination pixel index in the surrounding
            // operation from the supplied value.
            index,
            // Initialize current starting position within a paginated list from the supplied value.
            offset: u32::from_le_bytes(record[4..8].try_into().unwrap()) as usize,
            // Initialize advance from the supplied value.
            advance: u16::from_le_bytes(record[8..10].try_into().unwrap()) as i32,
            // Initialize horizontal extent in pixels from the supplied value.
            width: record[10] as usize,
            // Initialize full vertical pixel band reserved when drawing this font from the supplied
            // value.
            height: record[11] as usize,
            // Initialize horizontal coordinate or current position along the row from the supplied
            // value.
            x: record[12] as i8 as i32,
            // Initialize vertical coordinate or current screen row from the supplied value.
            y: record[13] as i8 as i32,
        }
    }
    /// Looks up the signed spacing adjustment between two glyph indices.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Font`) - Embedded glyph, bitmap, and kerning data with layout metrics.
    ///   Borrowed without changing it.
    /// * `a` (`usize`) - Left glyph index for the kerning pair.
    /// * `b` (`usize`) - Right glyph index for the kerning pair.
    ///
    /// # Returns
    ///
    /// `i32` - Kerning adjustment in sixteenth-pixel units, or zero when the pair is absent.
    fn kern(&self, a: usize, b: usize) -> i32 {
        // Combine the two glyph indices into one sortable kerning-pair key; each index occupies
        // eight bits.
        let key = (a << 8) | b;
        // Execute unwrap or for the transformed value or entries produced by the closure.
        search(self.kerning, 3, key as u32, |pair| {
            // Return `pair[0] as u32` shifted left by 8 combined with the numeric value converted
            // to the required arithmetic or indexing type as the value of this block.
            ((pair[0] as u32) << 8) | pair[1] as u32
        })
        .map(|index| self.kerning[index * 3 + 2] as i8 as i32)
        .unwrap_or(0)
    }
    /// Computes rounded pen advance including optional next-character kerning.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Font`) - Embedded glyph, bitmap, and kerning data with layout metrics.
    ///   Borrowed without changing it.
    /// * `glyph` (`&Glyph`) - Decoded glyph metrics, including advance in sixteenth-pixel
    ///   units.
    /// * `next` (`Option<char>`) - Optional following character used to determine pair kerning.
    ///
    /// # Returns
    ///
    /// `i32` - Horizontal advance in pixels.
    fn advance(&self, glyph: &Glyph, next: Option<char>) -> i32 {
        // Return `glyph.advance` plus `next .map(|ch| self.kern(glyph.index, self.glyph(ch).index))
        // .unwrap_or(0)` plus 8 shifted right by 4 as the value of this block.
        (glyph.advance
            + next
                .map(|ch| self.kern(glyph.index, self.glyph(ch).index))
                .unwrap_or(0)
            + 8)
            >> 4
    }
    /// Measures text bearings and total pen advance using glyph metrics and kerning.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Font`) - Embedded glyph, bitmap, and kerning data with layout metrics.
    ///   Borrowed without changing it.
    /// * `text` (`&str`) - Text to translate, measure, clip, or draw.
    ///
    /// # Returns
    ///
    /// `(i32, i32)` - Left and right pixel extents relative to the initial pen; includes
    /// negative bearings.
    fn extent(&self, text: &str) -> (i32, i32) {
        // Keep peekable Unicode iterator allowing spacing to consider the next character in this
        // local variable for the following operations.
        let mut chars = text.chars().peekable();
        // Keep the returned components: `pen` holds current horizontal drawing position after
        // accumulated advances, `left` holds leftmost text bearing relative to the initial pen; may
        // be negative, `right` holds rightmost visible edge or final pen advance, whichever is
        // farther in this local variable for the following operations.
        let (mut pen, mut left, mut right) = (0, 0, 0);
        // Repeat while an available next result for the surrounding operation matches the requested
        // case.
        while let Some(ch) = chars.next() {
            // Keep decoded bitmap position, bearings, dimensions, and advance for one character in
            // this local variable for the following operations.
            let glyph = self.glyph(ch);
            // Set leftmost text bearing relative to the initial pen; may be negative to the smaller
            // of the value and its upper bound.
            left = left.min(pen + glyph.x);
            // Set rightmost visible edge or final pen advance, whichever is farther to the larger
            // value, enforcing the lower bound.
            right = right.max(pen + glyph.x + glyph.width as i32);
            // Update current horizontal drawing position after accumulated advances using computes
            // rounded pen advance including optional next-character kerning, retaining the
            // accumulated state for subsequent steps.
            pen += self.advance(&glyph, chars.peek().copied());
        }
        // Return the ordered tuple of related values as the value of this block.
        (left, right.max(pen))
    }
    /// Measures the complete visible width of a proportional text string.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Font`) - Embedded glyph, bitmap, and kerning data with layout metrics.
    ///   Borrowed without changing it.
    /// * `text` (`&str`) - Text to translate, measure, clip, or draw.
    ///
    /// # Returns
    ///
    /// `u32` - Nonnegative pixel width including bearings and final advance.
    pub fn width(&self, text: &str) -> u32 {
        // Keep the returned components: `left` holds leftmost text bearing relative to the initial
        // pen; may be negative, `right` holds rightmost visible edge or final pen advance,
        // whichever is farther in this local variable for the following operations.
        let (left, right) = self.extent(text);
        (right - left).max(0) as u32
    }
    /// Composes clipped antialiased text against its backdrop and sends the final pixel band.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Font`) - Embedded glyph, bitmap, and kerning data with layout metrics.
    ///   Borrowed without changing it.
    /// * `display` (`&mut D`) - Mutable RGB565 draw target receiving the rendering operations.
    /// * `text` (`&str`) - Text to translate, measure, clip, or draw.
    /// * `origin` (`Point`) - Top-left drawing origin in screen pixels.
    /// * `color` (`Rgb565`) - RGB565 foreground colour.
    /// * `backdrop` (`Backdrop`) - Solid colour or vertical gradient used behind antialiased
    ///   pixels.
    /// * `max_width` (`u32`) - Maximum visible text width in pixels.
    ///
    /// # Type Parameters
    ///
    /// * `D` - RGB565 drawing destination; its associated error is propagated by fallible
    ///   renderers.
    ///
    /// # Returns
    ///
    /// `Result<(), D::Error>` - Ok after drawing or for an empty clipped area; Err from the
    /// draw target.
    ///
    /// # Errors
    ///
    /// Propagates contiguous pixel transfer failures.
    ///
    /// # Panics
    ///
    /// Panics if embedded font metadata references bitmap data outside the retained asset.
    pub fn draw<D: DrawTarget<Color = Rgb565>>(
        &self,
        display: &mut D,
        text: &str,
        origin: Point,
        color: Rgb565,
        backdrop: Backdrop,
        max_width: u32,
    ) -> Result<(), D::Error> {
        // Keep the returned components: `left` holds leftmost text bearing relative to the initial
        // pen; may be negative, `right` holds rightmost visible edge or final pen advance,
        // whichever is farther in this local variable for the following operations.
        let (left, right) = self.extent(text);
        // Intersect the text band with the target bounds and maximum width so no off-screen pixel
        // is buffered or written.
        let area = Rectangle::new(
            origin,
            Size::new(((right - left).max(0) as u32).min(max_width), self.height),
        )
        .intersection(&display.bounding_box());
        // Clipping removed the entire text band; no allocation or display transfer is necessary.
        if area.size.width == 0 || area.size.height == 0 {
            // Leave this function now with a successful result carrying (); later statements are
            // skipped.
            return Ok(());
        }
        // At most one 320x49 text band (~31 KiB), never a framebuffer or stack allocation.
        // Keep row-major RGB565 colour buffer or iterator in this local variable for the following
        // operations.
        let mut pixels: Vec<Rgb565> = (0..area.size.height)
            .flat_map(|y| {
                // Run the repeat n operation with the supplied inputs.
                std::iter::repeat_n(
                    backdrop.at(area.top_left.y + y as i32),
                    area.size.width as usize,
                )
            })
            .collect();
        // Keep peekable Unicode iterator allowing spacing to consider the next character in this
        // local variable for the following operations.
        let mut chars = text.chars().peekable();
        // Keep current horizontal drawing position after accumulated advances in this local
        // variable for the following operations.
        let mut pen = origin.x - left;
        // Repeat while an available next result for the surrounding operation matches the requested
        // case.
        while let Some(ch) = chars.next() {
            // Keep decoded bitmap position, bearings, dimensions, and advance for one character in
            // this local variable for the following operations.
            let glyph = self.glyph(ch);
            // Visit each entry in 0..glyph.height; the loop binding provides its value or index for
            // this iteration.
            for y in 0..glyph.height {
                // Visit each entry in 0..glyph.width; the loop binding provides its value or index
                // for this iteration.
                for x in 0..glyph.width {
                    // Keep coordinate sample associated with the current touch or pixel operation
                    // in this local variable for the following operations.
                    let point = Point::new(
                        pen + glyph.x + x as i32,
                        origin.y + self.baseline - glyph.y - glyph.height as i32 + y as i32,
                    );
                    // This glyph pixel lies outside the visible text band; skip it instead of
                    // indexing outside the buffer.
                    if !area.contains(point) {
                        // Skip the remainder of this iteration and wait for or inspect the next
                        // input.
                        continue;
                    }
                    // Convert glyph-local x/y into a row-major pixel index; each stored byte holds
                    // two alpha samples.
                    let bit = y * glyph.width + x;
                    // Fetch the packed opacity byte containing this pixel; even pixels use its high
                    // nibble and odd pixels its low nibble.
                    let byte = self.bitmap[glyph.offset + bit / 2];
                    // Select the high or low four-bit opacity sample for this glyph pixel; zero
                    // preserves the existing background.
                    let coverage = if bit % 2 == 0 { byte >> 4 } else { byte & 15 };
                    // Fully transparent glyph pixels preserve the existing backdrop without a blend
                    // calculation.
                    if coverage == 0 {
                        // Skip the remainder of this iteration and wait for or inspect the next
                        // input.
                        continue;
                    }
                    // Keep glyph record index or row-major destination pixel index in the
                    // surrounding operation in this local variable for the following operations.
                    let index = (point.y - area.top_left.y) as usize * area.size.width as usize
                        + (point.x - area.top_left.x) as usize;
                    // Set the selected entry from row-major RGB565 colour buffer or iterator to
                    // blends RGB565 channels using a four-bit coverage value.
                    pixels[index] = blend(pixels[index], color, coverage);
                }
            }
            // Update current horizontal drawing position after accumulated advances using computes
            // rounded pen advance including optional next-character kerning, retaining the
            // accumulated state for subsequent steps.
            pen += self.advance(&glyph, chars.peek().copied());
            // The pen passed the allowed text width; further glyphs cannot contribute visible
            // pixels.
            if pen >= origin.x + max_width as i32 {
                // Stop this loop now; the enclosing function continues with the work after the
                // loop.
                break;
            }
        }
        // Transfer a composed row-major colour rectangle, avoiding a visible clear-before-draw
        // pass.
        display.fill_contiguous(&area, pixels)
    }
}

/// Blends RGB565 channels using a four-bit coverage value.
///
/// # Arguments
///
/// * `background` (`Rgb565`) - Underlying RGB565 colour.
/// * `foreground` (`Rgb565`) - RGB565 colour mixed into the background.
/// * `coverage` (`u8`) - Four-bit alpha coverage in 0-15; larger values favour the
///   foreground.
///
/// # Returns
///
/// `Rgb565` - Rounded RGB565 mixture; coverage 0 selects background and 15 selects
/// foreground.
///
/// # Panics
///
/// Coverage must be in 0-15; values outside that range can overflow arithmetic and panic in
/// checked builds.
pub fn blend(background: Rgb565, foreground: Rgb565, coverage: u8) -> Rgb565 {
    // Keep four-bit coverage converted to u16 so blend multiplication has enough range in this
    // local variable for the following operations.
    let a = u16::from(coverage);
    // Keep per-channel interpolation calculation used for a gradient or opacity blend in this local
    // variable for the following operations.
    let channel = |bg: u8, fg: u8| ((u16::from(bg) * (15 - a) + u16::from(fg) * a + 7) / 15) as u8;
    // Construct the value from the supplied configuration or coordinates for use by the surrounding
    // operation.
    Rgb565::new(
        channel(background.r(), foreground.r()),
        channel(background.g(), foreground.g()),
        channel(background.b(), foreground.b()),
    )
}

/// Binary-searches fixed-stride sorted records using a caller-supplied key extractor.
///
/// # Arguments
///
/// * `bytes` (`&[u8]`) - Packed sorted fixed-stride records; trailing incomplete record
///   bytes are not searched.
/// * `stride` (`usize`) - Nonzero record size in bytes; records must be sorted by the
///   extracted key.
/// * `key` (`u32`) - Numeric lookup key supplied to the sorted record search.
/// * `read` (`impl Fn(&[u8]) -> u32`) - Key extractor called with one complete record
///   slice; returns its u32 search key.
///
/// # Returns
///
/// `Result<usize, usize>` - Ok with the matching record index, or Err with the insertion
/// index when the key is absent.
///
/// # Errors
///
/// Err is a search miss rather than an operational failure.
///
/// # Panics
///
/// Panics if stride is zero, or if the caller's record-key extractor panics.
fn search(
    bytes: &[u8],
    stride: usize,
    key: u32,
    read: impl Fn(&[u8]) -> u32,
) -> Result<usize, usize> {
    // Keep the returned components: `low` holds first record position that could still contain the
    // search key, `high` holds exclusive end of the remaining record-index search interval in this
    // local variable for the following operations.
    let (mut low, mut high) = (0, bytes.len() / stride);
    // The binary-search interval still contains candidates; stop when its insertion position is
    // determined.
    while low < high {
        // Keep midpoint of the remaining record-index search interval in this local variable for
        // the following operations.
        let middle = (low + high) / 2;
        // Choose the appropriate path for cmp result for the surrounding operation; each arm
        // handles one supported case.
        match read(&bytes[middle * stride..][..stride]).cmp(&key) {
            // Set first record position that could still contain the search key to midpoint of the
            // remaining record-index search interval plus 1.
            std::cmp::Ordering::Less => low = middle + 1,
            // Set exclusive end of the remaining record-index search interval to midpoint of the
            // remaining record-index search interval.
            std::cmp::Ordering::Greater => high = middle,
            // Leave this function now with a successful result carrying midpoint of the remaining
            // record-index search interval; later statements are skipped.
            std::cmp::Ordering::Equal => return Ok(middle),
        }
    }
    // Return the failure result so the caller can display an error, retry, or preserve the saved
    // baseline.
    Err(low)
}

/// Sanitizes control characters and truncates text to a measured pixel width.
///
/// # Arguments
///
/// * `text` (`&str`) - Text to translate, measure, clip, or draw.
/// * `font` (`&Font`) - Proportional bitmap font used to measure and draw text.
/// * `width` (`u32`) - Available drawing or text width in pixels.
///
/// # Returns
///
/// `String` - Owned text that fits the width, using two trailing dots when truncation
/// permits them.
pub fn clipped(text: &str, font: &Font, width: u32) -> String {
    // Keep text with control characters replaced, avoiding unwanted layout behavior in this local
    // variable for the following operations.
    let clean: String = text
        .chars()
        // Replace control characters with spaces so arbitrary text cannot disturb single-line
        // layout.
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    // The whole sanitized label fits, so return it without a truncation marker.
    if font.width(&clean) <= width {
        // Leave this function now with text with control characters replaced, avoiding unwanted
        // layout behavior; later statements are skipped.
        return clean;
    }
    // Keep success or failure produced by the operation, retained for later handling in this local
    // variable for the following operations.
    let mut result = String::new();
    // Visit each entry in chars result for the surrounding operation; the loop binding provides its
    // value or index for this iteration.
    for ch in clean.chars() {
        // Measure the next candidate character together with the two-dot truncation marker before
        // accepting it.
        let candidate = format!("{result}{ch}..");
        // The next character plus the two-dot suffix would not fit; stop before exceeding the
        // allotted width.
        if font.width(&candidate) > width {
            // Stop this loop now; the enclosing function continues with the work after the loop.
            break;
        }
        // Append the new entry to the collection, preserving the order in which values arrive.
        result.push(ch);
    }
    // Append the truncation marker only when even the marker fits inside the available width.
    if font.width("..") <= width {
        // Append the supplied text to the existing string without replacing its earlier contents.
        result.push_str("..");
    }
    // Return success or failure produced by the operation, retained for later handling as the value
    // of this block.
    result
}

/// Wraps measured text at word or character boundaries and limits the number of lines.
///
/// # Arguments
///
/// * `text` (`&str`) - Text to translate, measure, clip, or draw.
/// * `font` (`&Font`) - Proportional bitmap font used to measure and draw text.
/// * `width` (`u32`) - Available drawing or text width in pixels.
/// * `max_lines` (`usize`) - Maximum number of output or drawn lines; zero permits no
///   lines.
///
/// # Returns
///
/// `Vec<String>` - Owned lines fitting the requested width; excess text is clipped in the
/// last permitted line and the vector may be empty.
pub fn wrap(text: &str, font: &Font, width: u32, max_lines: usize) -> Vec<String> {
    // Keep text with control characters replaced, avoiding unwanted layout behavior in this local
    // variable for the following operations.
    let clean: String = text
        .chars()
        // Preserve explicit paragraph breaks while replacing other control characters with spaces.
        .map(|c| if c.is_control() && c != '\n' { ' ' } else { c })
        .collect();
    // Keep measured text lines that fit the available width and line count in this local variable
    // for the following operations.
    let mut lines = Vec::new();
    // Keep remaining text not yet assigned to wrapped lines in this local variable for the
    // following operations.
    let mut rest = clean.as_str();
    // Continue wrapping while text remains and the caller's maximum line count has not been
    // reached.
    while !rest.is_empty() && lines.len() < max_lines {
        // Keep portion before the next explicit newline, wrapped independently in this local
        // variable for the following operations.
        let paragraph = rest.split('\n').next().unwrap();
        // This entire paragraph fits on one line; preserve it without splitting a word.
        if font.width(paragraph) <= width {
            // Append the new entry to the collection, preserving the order in which values arrive.
            lines.push(paragraph.to_owned());
            // Set remaining text not yet assigned to wrapped lines to the available value or the
            // documented fallback when absent.
            rest = rest.get(paragraph.len() + 1..).unwrap_or_default();
            // Skip the remainder of this iteration and wait for or inspect the next input.
            continue;
        }
        // Keep last UTF-8 character boundary that fits the available pixel width in this local
        // variable for the following operations.
        let mut end = 0;
        // Keep most recent usable word boundary, if one fits before the width limit in this local
        // variable for the following operations.
        let mut space = None;
        // Visit each entry in char indices result for the surrounding operation; the loop binding
        // provides its value or index for this iteration.
        for (i, ch) in paragraph.char_indices() {
            // Keep UTF-8 byte boundary immediately after the current character in this local
            // variable for the following operations.
            let next = i + ch.len_utf8();
            // The next complete Unicode character would exceed the measured line width.
            if font.width(&paragraph[..next]) > width {
                // Stop this loop now; the enclosing function continues with the work after the
                // loop.
                break;
            }
            // Set last UTF-8 character boundary that fits the available pixel width to UTF-8 byte
            // boundary immediately after the current character.
            end = next;
            // Remember a word boundary inside the line so wrapping can prefer whole words over
            // splitting characters.
            if ch == ' ' && i > 0 {
                // Set most recent usable word boundary, if one fits before the width limit to an
                // available value for i.
                space = Some(i);
            }
        }
        // No character fits the remaining width; stop rather than creating an empty-progress loop.
        if end == 0 {
            // Stop this loop now; the enclosing function continues with the work after the loop.
            break;
        }
        // Keep chosen word or character boundary at which to split the current line in this local
        // variable for the following operations.
        let split = space.unwrap_or(end);
        // Append the new entry to the collection, preserving the order in which values arrive.
        lines.push(paragraph[..split].to_owned());
        // Set remaining text not yet assigned to wrapped lines to trim start matches result for the
        // surrounding operation.
        rest = rest[split..].trim_start_matches(' ');
    }
    // More text remains after the final permitted line, so mark truncation in that last line.
    if !rest.is_empty() {
        // Only rewrite the final line if a line was actually produced.
        if let Some(last) = lines.last_mut() {
            // Set last to sanitizes control characters and truncates text to a measured pixel
            // width.
            *last = clipped(&format!("{last}.."), font, width);
        }
    }
    // Return measured text lines that fit the available width and line count as the value of this
    // block.
    lines
}

///////////////////////////////////////////////////////////////////////////////////////
//                                    UNIT TESTS                                     //
///////////////////////////////////////////////////////////////////////////////////////

/// Unit tests and supporting fixtures for theme behavior.
#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::mock_display::MockDisplay;

    /// Verifies font coverage, proportional measurements, kerning, and glyph bearings for
    /// supported characters.
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
    fn proportional_metrics_kerning_and_font_coverage() {
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for font in [
            &SMALL,
            &BODY,
            &TITLE,
            &FORECAST_SMALL,
            &FORECAST_BODY,
            &FORECAST_TITLE,
        ] {
            // Verify measures the complete visible width of a proportional text string is greater
            // than measures the complete visible width of a proportional text string. A violation
            // means the tested behavior is incorrect.
            assert!(font.width("WWW") > font.width("iii"));
            // Verify looks up the signed spacing adjustment between two glyph indices is less than
            // 0. A violation means the tested behavior is incorrect.
            assert!(font.kern(font.glyph('A').index, font.glyph('V').index) < 0);
            // Verify measures the complete visible width of a proportional text string does not
            // exceed measures the complete visible width of a proportional text string plus
            // measures the complete visible width of a proportional text string. A violation means
            // the tested behavior is incorrect.
            assert!(font.width("AV") <= font.width("A") + font.width("V"));
            // Verify the length of packed sixteen-byte glyph records sorted by Unicode codepoint
            // divided by 16 is less than 256. A violation means the tested behavior is incorrect.
            assert!(font.glyphs.len() / 16 < 256);
            // Visit each entry in chain result for the surrounding operation; the loop binding
            // provides its value or index for this iteration.
            for cp in (32..=126)
                .chain([
                    176, 201, 223, 233, 252, 8594, 0x401, 0x451, 0x490, 0x491, 0x404, 0x454, 0x406,
                    0x456, 0x407, 0x457,
                ])
                .chain(0x410..=0x44f)
            {
                // Keep decoded bitmap position, bearings, dimensions, and advance for one character
                // in this local variable for the following operations.
                let glyph = font.glyph(char::from_u32(cp).unwrap());
                // Keep packed sixteen-byte metadata slice for the selected glyph in this local
                // variable for the following operations.
                let record = &font.glyphs[glyph.index * 16..];
                // Verify that the integer decoded from little-endian bytes exactly matches cp.
                assert_eq!(u32::from_le_bytes(record[..4].try_into().unwrap()), cp);
                // Verify current starting position within a paginated list plus div ceil result for
                // the surrounding operation does not exceed the length of packed four-bit glyph
                // coverage values retained under the font asset licence. A violation means the
                // tested behavior is incorrect.
                assert!(
                    glyph.offset + (glyph.width * glyph.height).div_ceil(2) <= font.bitmap.len()
                );
                // Verify vertical baseline within the font's drawing band minus vertical coordinate
                // or current screen row minus the numeric value converted to the required
                // arithmetic or indexing type has reached or exceeded 0. A violation means the
                // tested behavior is incorrect.
                assert!(font.baseline - glyph.y - glyph.height as i32 >= 0);
                // Verify vertical baseline within the font's drawing band minus vertical coordinate
                // or current screen row does not exceed the numeric value converted to the required
                // arithmetic or indexing type. A violation means the tested behavior is incorrect.
                assert!(font.baseline - glyph.y <= font.height as i32);
            }
            // Visit each entry in chunks exact result for the surrounding operation; the loop
            // binding provides its value or index for this iteration.
            for record in font.glyphs.chunks_exact(16) {
                // Keep one complete Unicode character, avoiding partial UTF-8 text boundaries in
                // this local variable for the following operations.
                let ch =
                    char::from_u32(u32::from_le_bytes(record[..4].try_into().unwrap())).unwrap();
                // Keep decoded bitmap position, bearings, dimensions, and advance for one character
                // in this local variable for the following operations.
                let glyph = font.glyph(ch);
                // Bearings extending left of the pen must be included in clipping/centering.
                // Verify measures the complete visible width of a proportional text string has
                // reached or exceeded the numeric value converted to the required arithmetic or
                // indexing type. A violation means the tested behavior is incorrect.
                assert!(font.width(&ch.to_string()) >= glyph.width as u32);
            }
            // Visit each entry in filter result for the surrounding operation; the loop binding
            // provides its value or index for this iteration.
            for ch in include_str!("i18n.rs")
                .chars()
                .filter(|ch| !ch.is_control())
            {
                // Keep decoded bitmap position, bearings, dimensions, and advance for one character
                // in this local variable for the following operations.
                let glyph = font.glyph(ch);
                // Keep packed sixteen-byte metadata slice for the selected glyph in this local
                // variable for the following operations.
                let record = &font.glyphs[glyph.index * 16..];
                // Verify that the integer decoded from little-endian bytes exactly matches the
                // numeric value converted to the required arithmetic or indexing type.
                assert_eq!(
                    u32::from_le_bytes(record[..4].try_into().unwrap()),
                    ch as u32,
                    "missing translated character {ch}"
                );
                // Verify vertical baseline within the font's drawing band minus vertical coordinate
                // or current screen row minus the numeric value converted to the required
                // arithmetic or indexing type has reached or exceeded 0. A violation means the
                // tested behavior is incorrect.
                assert!(font.baseline - glyph.y - glyph.height as i32 >= 0);
                // Verify vertical baseline within the font's drawing band minus vertical coordinate
                // or current screen row does not exceed the numeric value converted to the required
                // arithmetic or indexing type. A violation means the tested behavior is incorrect.
                assert!(font.baseline - glyph.y <= font.height as i32);
            }
            // Verify that glyph record index or row-major destination pixel index in the
            // surrounding operation exactly matches glyph record index or row-major destination
            // pixel index in the surrounding operation.
            assert_eq!(font.glyph('中').index, font.glyph('?').index);
        }
    }

    /// Verifies that wrapped and clipped Unicode text respects proportional-font pixel limits.
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
    fn wrapping_and_truncation_fit_variable_width_and_unicode() {
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for text in [
            "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW",
            "Café École Straße München",
            "line one\nline two\nline three",
        ] {
            // Keep sanitized label truncated to the font's available pixel width in this local
            // variable for the following operations.
            let clipped = clipped(text, &BODY, 100);
            // Verify measures the complete visible width of a proportional text string does not
            // exceed 100. A violation means the tested behavior is incorrect.
            assert!(BODY.width(&clipped) <= 100);
            // Keep measured text lines that fit the available width and line count in this local
            // variable for the following operations.
            let lines = wrap(text, &BODY, 100, 2);
            // Verify the inverse of measured text lines that fit the available width and line count
            // is empty and the length of measured text lines that fit the available width and line
            // count does not exceed 2. A violation means the tested behavior is incorrect.
            assert!(!lines.is_empty() && lines.len() <= 2);
            // Verify every entry satisfies the required condition. A violation means the tested
            // behavior is incorrect.
            assert!(lines.iter().all(|line| BODY.width(line) <= 100));
        }
        // Verify that wraps measured text at word or character boundaries and limits the number of
        // lines exactly matches the ordered sample/byte array.
        assert_eq!(wrap("one\ntwo", &BODY, 100, 3), ["one", "two"]);
        // Verify wraps measured text at word or character boundaries and limits the number of lines
        // is empty. A violation means the tested behavior is incorrect.
        assert!(wrap("", &BODY, 100, 3).is_empty());
    }

    /// Verifies gradient endpoint colours and retained background pixels outside rounded
    /// panels.
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
    fn gradient_and_rounded_corners_keep_the_background_visible() {
        // Verify that samples the vertical interface gradient at a screen row exactly matches
        // converts a packed 24-bit RGB colour to rounded RGB565 channel values.
        assert_eq!(background_at(0), rgb(0x4c8cb9));
        // Verify that samples the vertical interface gradient at a screen row exactly matches
        // converts a packed 24-bit RGB colour to rounded RGB565 channel values.
        assert_eq!(background_at(479), rgb(0xa6cdec));
        // Keep drawing destination receiving RGB565 pixels in this local variable for the following
        // operations.
        let mut display = MockDisplay::new();
        // Allow the mock target to repaint pixels so repeated rendering can be tested.
        display.set_allow_overdraw(true);
        // Require the value guaranteed by this test fixture or internal invariant; unexpected
        // absence panics.
        background(&mut display).unwrap();
        // Keep screen-coordinate rectangle limiting the drawing operation in this local variable
        // for the following operations.
        let bounds = Rectangle::new(Point::new(10, 10), Size::new(40, 30));
        // Require the value guaranteed by this test fixture or internal invariant; unexpected
        // absence panics.
        rounded(&mut display, bounds, PANEL).unwrap();
        // Verify that get pixel result for the surrounding operation exactly matches an available
        // value for samples the vertical interface gradient at a screen row.
        assert_eq!(
            display.get_pixel(Point::new(10, 10)),
            Some(background_at(10))
        );
        // Verify that get pixel result for the surrounding operation exactly matches an available
        // value for solid background colour shared by interactive controls.
        assert_eq!(display.get_pixel(Point::new(30, 10)), Some(PANEL));
        // Verify that get pixel result for the surrounding operation exactly matches an available
        // value for solid background colour shared by interactive controls.
        assert_eq!(display.get_pixel(Point::new(30, 25)), Some(PANEL));
    }

    /// Verifies antialiased text blending and clipping against the draw target bounds.
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
    fn antialiased_text_blends_and_clips_to_the_target() {
        // Keep drawing destination receiving RGB565 pixels in this local variable for the following
        // operations.
        let mut display = MockDisplay::new();
        // Allow the mock target to repaint pixels so repeated rendering can be tested.
        display.set_allow_overdraw(true);
        // Require the value guaranteed by this test fixture or internal invariant; unexpected
        // absence panics.
        background(&mut display).unwrap();
        // Require the value guaranteed by this test fixture or internal invariant; unexpected
        // absence panics.
        BODY.draw(
            &mut display,
            "Café WW",
            Point::new(48, 50),
            TEXT,
            Backdrop::Gradient,
            12,
        )
        .unwrap();
        // Verify that get pixel result for the surrounding operation exactly matches an available
        // value for samples the vertical interface gradient at a screen row.
        assert_eq!(
            display.get_pixel(Point::new(60, 50)),
            Some(background_at(50))
        );
        // Verify that blends RGB565 channels using a four-bit coverage value exactly matches solid
        // background colour shared by interactive controls.
        assert_eq!(blend(PANEL, TEXT, 0), PANEL);
        // Verify that blends RGB565 channels using a four-bit coverage value exactly matches
        // primary foreground colour for readable labels.
        assert_eq!(blend(PANEL, TEXT, 15), TEXT);
        // Verify that blends RGB565 channels using a four-bit coverage value differs from solid
        // background colour shared by interactive controls.
        assert_ne!(blend(PANEL, TEXT, 7), PANEL);
        // Verify that blends RGB565 channels using a four-bit coverage value differs from primary
        // foreground colour for readable labels.
        assert_ne!(blend(PANEL, TEXT, 7), TEXT);
    }
}
