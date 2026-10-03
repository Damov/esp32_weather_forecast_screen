//! MIT-licensed Meteocons bitmap derivatives; see assets/weather/NOTICE.txt.
use crate::theme;
use embedded_graphics::{pixelcolor::Rgb565, prelude::*, primitives::Rectangle};
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
pub fn draw<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    index: usize,
    origin: Point,
    large: bool,
    panel: bool,
) -> Result<(), D::Error> {
    draw_on(
        display,
        index,
        origin,
        large,
        if panel {
            theme::Backdrop::Solid(theme::PANEL)
        } else {
            theme::Backdrop::Gradient
        },
    )
}
pub fn draw_on<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    index: usize,
    origin: Point,
    large: bool,
    backdrop: theme::Backdrop,
) -> Result<(), D::Error> {
    let size = if large { 72 } else { 32 };
    let data = if large {
        ICONS_72[index.min(12)]
    } else {
        ICONS_32[index.min(12)]
    };
    let bounds =
        Rectangle::new(origin, Size::new(size, size)).intersection(&display.bounding_box());
    let pixels = (bounds.top_left.y..bounds.top_left.y + bounds.size.height as i32).flat_map(|y| {
        (bounds.top_left.x..bounds.top_left.x + bounds.size.width as i32).map(move |x| {
            let offset = (((y - origin.y) as u32 * size + (x - origin.x) as u32) * 3) as usize;
            let color = u16::from_le_bytes([data[offset], data[offset + 1]]);
            let foreground = Rgb565::new(
                ((color >> 11) & 31) as u8,
                ((color >> 5) & 63) as u8,
                (color & 31) as u8,
            );
            theme::blend(
                backdrop.at(y),
                foreground,
                ((data[offset + 2] as u16 * 15 + 127) / 255) as u8,
            )
        })
    });
    display.fill_contiguous(&bounds, pixels)
}
