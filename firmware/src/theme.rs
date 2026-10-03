//! UI colours and small, shared proportional-font rendering helpers.
use embedded_graphics::{
    pixelcolor::{Rgb565, Rgb888},
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle, RoundedRectangle},
};

pub const WEATHER_CARD: Rgb565 = rgb(0x397ca8);
pub const PANEL: Rgb565 = rgb(0x5e9bc8);
pub const TEXT: Rgb565 = rgb(0xffffff);
pub const SECONDARY: Rgb565 = rgb(0xe4ffff);
pub const MUTED: Rgb565 = rgb(0xb9ecff);
pub const RADIUS: u32 = 4;

pub const fn rgb(hex: u32) -> Rgb565 {
    Rgb565::new(
        ((((hex >> 16) & 255) * 31 + 127) / 255) as u8,
        ((((hex >> 8) & 255) * 63 + 127) / 255) as u8,
        (((hex & 255) * 31 + 127) / 255) as u8,
    )
}

pub fn background_at(y: i32) -> Rgb565 {
    let y = y.clamp(0, 479) as u32;
    let channel = |a: u32, b: u32| ((a * (479 - y) + b * y + 239) / 479) as u8;
    Rgb888::new(channel(76, 166), channel(140, 205), channel(185, 236)).into()
}

pub fn background<D: DrawTarget<Color = Rgb565>>(display: &mut D) -> Result<(), D::Error> {
    let bounds = display.bounding_box();
    for y in bounds.top_left.y..bounds.top_left.y + bounds.size.height as i32 {
        display.fill_solid(
            &Rectangle::new(
                Point::new(bounds.top_left.x, y),
                Size::new(bounds.size.width, 1),
            ),
            background_at(y),
        )?;
    }
    Ok(())
}

pub fn rounded<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    bounds: Rectangle,
    fill: Rgb565,
) -> Result<(), D::Error> {
    rounded_radius(display, bounds, fill, RADIUS)
}

pub fn rounded_radius<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    bounds: Rectangle,
    fill: Rgb565,
    radius: u32,
) -> Result<(), D::Error> {
    RoundedRectangle::with_equal_corners(bounds, Size::new(radius, radius))
        .into_styled(PrimitiveStyle::with_fill(fill))
        .draw(display)?;
    Ok(())
}

#[derive(Clone, Copy)]
pub enum Backdrop {
    Gradient,
    Solid(Rgb565),
}
impl Backdrop {
    pub(crate) fn at(self, y: i32) -> Rgb565 {
        match self {
            Self::Gradient => background_at(y),
            Self::Solid(color) => color,
        }
    }
}

// Font data is OFL-1.1, not GPL. See assets/fonts/NOTICE.txt and OFL.txt.
pub struct Font {
    glyphs: &'static [u8],
    bitmap: &'static [u8],
    kerning: &'static [u8],
    pub height: u32,
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
pub const SMALL: Font = font!("12", 16, 13);
pub const BODY: Font = font!("14", 19, 15);
pub const TITLE: Font = font!("20", 25, 20);
pub const FORECAST_SMALL: Font = font!("19", 23, 19);
pub const FORECAST_BODY: Font = font!("22", 26, 21);
pub const FORECAST_TITLE: Font = font!("31", 38, 31);
pub const CLOCK: Font = font!("42", 49, 40);

struct Glyph {
    index: usize,
    offset: usize,
    advance: i32,
    width: usize,
    height: usize,
    x: i32,
    y: i32,
}

impl Font {
    fn glyph(&self, ch: char) -> Glyph {
        let index = search(self.glyphs, 16, ch as u32, |record| {
            u32::from_le_bytes(record[..4].try_into().unwrap())
        })
        .unwrap_or_else(|_| {
            // ASCII '?' is always present. Do not replace accented Latin with blanks.
            self.glyphs
                .chunks_exact(16)
                .position(|record| {
                    u32::from_le_bytes(record[..4].try_into().unwrap()) == '?' as u32
                })
                .unwrap()
        });
        let record = &self.glyphs[index * 16..][..16];
        Glyph {
            index,
            offset: u32::from_le_bytes(record[4..8].try_into().unwrap()) as usize,
            advance: u16::from_le_bytes(record[8..10].try_into().unwrap()) as i32,
            width: record[10] as usize,
            height: record[11] as usize,
            x: record[12] as i8 as i32,
            y: record[13] as i8 as i32,
        }
    }
    fn kern(&self, a: usize, b: usize) -> i32 {
        let key = (a << 8) | b;
        search(self.kerning, 3, key as u32, |pair| {
            ((pair[0] as u32) << 8) | pair[1] as u32
        })
        .map(|index| self.kerning[index * 3 + 2] as i8 as i32)
        .unwrap_or(0)
    }
    fn advance(&self, glyph: &Glyph, next: Option<char>) -> i32 {
        (glyph.advance
            + next
                .map(|ch| self.kern(glyph.index, self.glyph(ch).index))
                .unwrap_or(0)
            + 8)
            >> 4
    }
    fn extent(&self, text: &str) -> (i32, i32) {
        let mut chars = text.chars().peekable();
        let (mut pen, mut left, mut right) = (0, 0, 0);
        while let Some(ch) = chars.next() {
            let glyph = self.glyph(ch);
            left = left.min(pen + glyph.x);
            right = right.max(pen + glyph.x + glyph.width as i32);
            pen += self.advance(&glyph, chars.peek().copied());
        }
        (left, right.max(pen))
    }
    pub fn width(&self, text: &str) -> u32 {
        let (left, right) = self.extent(text);
        (right - left).max(0) as u32
    }
    pub fn draw<D: DrawTarget<Color = Rgb565>>(
        &self,
        display: &mut D,
        text: &str,
        origin: Point,
        color: Rgb565,
        backdrop: Backdrop,
        max_width: u32,
    ) -> Result<(), D::Error> {
        let (left, right) = self.extent(text);
        let area = Rectangle::new(
            origin,
            Size::new(((right - left).max(0) as u32).min(max_width), self.height),
        )
        .intersection(&display.bounding_box());
        if area.size.width == 0 || area.size.height == 0 {
            return Ok(());
        }
        // At most one 320x49 text band (~31 KiB), never a framebuffer or stack allocation.
        let mut pixels: Vec<Rgb565> = (0..area.size.height)
            .flat_map(|y| {
                std::iter::repeat_n(
                    backdrop.at(area.top_left.y + y as i32),
                    area.size.width as usize,
                )
            })
            .collect();
        let mut chars = text.chars().peekable();
        let mut pen = origin.x - left;
        while let Some(ch) = chars.next() {
            let glyph = self.glyph(ch);
            for y in 0..glyph.height {
                for x in 0..glyph.width {
                    let point = Point::new(
                        pen + glyph.x + x as i32,
                        origin.y + self.baseline - glyph.y - glyph.height as i32 + y as i32,
                    );
                    if !area.contains(point) {
                        continue;
                    }
                    let bit = y * glyph.width + x;
                    let byte = self.bitmap[glyph.offset + bit / 2];
                    let coverage = if bit % 2 == 0 { byte >> 4 } else { byte & 15 };
                    if coverage == 0 {
                        continue;
                    }
                    let index = (point.y - area.top_left.y) as usize * area.size.width as usize
                        + (point.x - area.top_left.x) as usize;
                    pixels[index] = blend(pixels[index], color, coverage);
                }
            }
            pen += self.advance(&glyph, chars.peek().copied());
            if pen >= origin.x + max_width as i32 {
                break;
            }
        }
        display.fill_contiguous(&area, pixels)
    }
}

pub fn blend(background: Rgb565, foreground: Rgb565, coverage: u8) -> Rgb565 {
    let a = u16::from(coverage);
    let channel = |bg: u8, fg: u8| ((u16::from(bg) * (15 - a) + u16::from(fg) * a + 7) / 15) as u8;
    Rgb565::new(
        channel(background.r(), foreground.r()),
        channel(background.g(), foreground.g()),
        channel(background.b(), foreground.b()),
    )
}

fn search(
    bytes: &[u8],
    stride: usize,
    key: u32,
    read: impl Fn(&[u8]) -> u32,
) -> Result<usize, usize> {
    let (mut low, mut high) = (0, bytes.len() / stride);
    while low < high {
        let middle = (low + high) / 2;
        match read(&bytes[middle * stride..][..stride]).cmp(&key) {
            std::cmp::Ordering::Less => low = middle + 1,
            std::cmp::Ordering::Greater => high = middle,
            std::cmp::Ordering::Equal => return Ok(middle),
        }
    }
    Err(low)
}

pub fn clipped(text: &str, font: &Font, width: u32) -> String {
    let clean: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if font.width(&clean) <= width {
        return clean;
    }
    let mut result = String::new();
    for ch in clean.chars() {
        let candidate = format!("{result}{ch}..");
        if font.width(&candidate) > width {
            break;
        }
        result.push(ch);
    }
    if font.width("..") <= width {
        result.push_str("..");
    }
    result
}

pub fn wrap(text: &str, font: &Font, width: u32, max_lines: usize) -> Vec<String> {
    let clean: String = text
        .chars()
        .map(|c| if c.is_control() && c != '\n' { ' ' } else { c })
        .collect();
    let mut lines = Vec::new();
    let mut rest = clean.as_str();
    while !rest.is_empty() && lines.len() < max_lines {
        let paragraph = rest.split('\n').next().unwrap();
        if font.width(paragraph) <= width {
            lines.push(paragraph.to_owned());
            rest = rest.get(paragraph.len() + 1..).unwrap_or_default();
            continue;
        }
        let mut end = 0;
        let mut space = None;
        for (i, ch) in paragraph.char_indices() {
            let next = i + ch.len_utf8();
            if font.width(&paragraph[..next]) > width {
                break;
            }
            end = next;
            if ch == ' ' && i > 0 {
                space = Some(i);
            }
        }
        if end == 0 {
            break;
        }
        let split = space.unwrap_or(end);
        lines.push(paragraph[..split].to_owned());
        rest = rest[split..].trim_start_matches(' ');
    }
    if !rest.is_empty() {
        if let Some(last) = lines.last_mut() {
            *last = clipped(&format!("{last}.."), font, width);
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::mock_display::MockDisplay;

    #[test]
    fn proportional_metrics_kerning_and_font_coverage() {
        for font in [
            &SMALL,
            &BODY,
            &TITLE,
            &FORECAST_SMALL,
            &FORECAST_BODY,
            &FORECAST_TITLE,
        ] {
            assert!(font.width("WWW") > font.width("iii"));
            assert!(font.kern(font.glyph('A').index, font.glyph('V').index) < 0);
            assert!(font.width("AV") <= font.width("A") + font.width("V"));
            assert!(font.glyphs.len() / 16 < 256);
            for cp in (32..=126)
                .chain([
                    176, 201, 223, 233, 252, 8594, 0x401, 0x451, 0x490, 0x491, 0x404, 0x454, 0x406,
                    0x456, 0x407, 0x457,
                ])
                .chain(0x410..=0x44f)
            {
                let glyph = font.glyph(char::from_u32(cp).unwrap());
                let record = &font.glyphs[glyph.index * 16..];
                assert_eq!(u32::from_le_bytes(record[..4].try_into().unwrap()), cp);
                assert!(
                    glyph.offset + (glyph.width * glyph.height).div_ceil(2) <= font.bitmap.len()
                );
                assert!(font.baseline - glyph.y - glyph.height as i32 >= 0);
                assert!(font.baseline - glyph.y <= font.height as i32);
            }
            for record in font.glyphs.chunks_exact(16) {
                let ch =
                    char::from_u32(u32::from_le_bytes(record[..4].try_into().unwrap())).unwrap();
                let glyph = font.glyph(ch);
                // Bearings extending left of the pen must be included in clipping/centering.
                assert!(font.width(&ch.to_string()) >= glyph.width as u32);
            }
            for ch in include_str!("i18n.rs")
                .chars()
                .filter(|ch| !ch.is_control())
            {
                let glyph = font.glyph(ch);
                let record = &font.glyphs[glyph.index * 16..];
                assert_eq!(
                    u32::from_le_bytes(record[..4].try_into().unwrap()),
                    ch as u32,
                    "missing translated character {ch}"
                );
                assert!(font.baseline - glyph.y - glyph.height as i32 >= 0);
                assert!(font.baseline - glyph.y <= font.height as i32);
            }
            assert_eq!(font.glyph('中').index, font.glyph('?').index);
        }
    }

    #[test]
    fn wrapping_and_truncation_fit_variable_width_and_unicode() {
        for text in [
            "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW",
            "Café École Straße München",
            "line one\nline two\nline three",
        ] {
            let clipped = clipped(text, &BODY, 100);
            assert!(BODY.width(&clipped) <= 100);
            let lines = wrap(text, &BODY, 100, 2);
            assert!(!lines.is_empty() && lines.len() <= 2);
            assert!(lines.iter().all(|line| BODY.width(line) <= 100));
        }
        assert_eq!(wrap("one\ntwo", &BODY, 100, 3), ["one", "two"]);
        assert!(wrap("", &BODY, 100, 3).is_empty());
    }

    #[test]
    fn gradient_and_rounded_corners_keep_the_background_visible() {
        assert_eq!(background_at(0), rgb(0x4c8cb9));
        assert_eq!(background_at(479), rgb(0xa6cdec));
        let mut display = MockDisplay::new();
        display.set_allow_overdraw(true);
        background(&mut display).unwrap();
        let bounds = Rectangle::new(Point::new(10, 10), Size::new(40, 30));
        rounded(&mut display, bounds, PANEL).unwrap();
        assert_eq!(
            display.get_pixel(Point::new(10, 10)),
            Some(background_at(10))
        );
        assert_eq!(display.get_pixel(Point::new(30, 10)), Some(PANEL));
        assert_eq!(display.get_pixel(Point::new(30, 25)), Some(PANEL));
    }

    #[test]
    fn antialiased_text_blends_and_clips_to_the_target() {
        let mut display = MockDisplay::new();
        display.set_allow_overdraw(true);
        background(&mut display).unwrap();
        BODY.draw(
            &mut display,
            "Café WW",
            Point::new(48, 50),
            TEXT,
            Backdrop::Gradient,
            12,
        )
        .unwrap();
        assert_eq!(
            display.get_pixel(Point::new(60, 50)),
            Some(background_at(50))
        );
        assert_eq!(blend(PANEL, TEXT, 0), PANEL);
        assert_eq!(blend(PANEL, TEXT, 15), TEXT);
        assert_ne!(blend(PANEL, TEXT, 7), PANEL);
        assert_ne!(blend(PANEL, TEXT, 7), TEXT);
    }
}
