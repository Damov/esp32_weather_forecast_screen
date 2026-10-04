# Firmware development

This directory contains the Rust application, using ESP-IDF with standard-library support. See the [main README](../README.md#building-on-linux) for the Linux build and the [user guide](../USER_GUIDE.md) for installation and device operation. The [assembly guide](../printable_3D_model/README.md) covers physical wiring.

Run the commands in this document from the repository root.

## Build layout under Linux

Use `./build.sh` for device builds. It resolves `firmware/partitions.csv` to an absolute path in `firmware/target/esp32-partitions.sdkconfig`. That generated file supplies only the partition filename. `firmware/sdkconfig.defaults` supplies the remaining settings. Host tests and previews can use Cargo directly.

The build script forwards Cargo arguments and can be invoked by its path from another directory. Keep `firmware/Cargo.lock` in Git once generated for reproducible dependency resolution. Assets are checked in, so a normal device build does not need the font or icon conversion tools.

## Settings storage and compatibility

The versioned JSON profile is stored in NVS namespace `app_settings`, key `profile`, separately from Wi-Fi credentials and `touch_cal`. If no profile exists, the firmware reads compatible values from the legacy `weather` namespace without modifying its keys. The first preference change saves the imported profile. An imported location with no timezone is resolved online. No night schedule is applied until its timezone is known. Unreadable profiles show an error and disable preference writes without erasing storage.

Older version-1 profiles without the presence-wake field default to enabled. The legacy `weather/wakePresence` value is imported when available. Disabling wake changes backlight behaviour without stopping sensing or indicator updates.

## Backlight and time scheduling

**Screen off after** offers Never, 1, 5, 10, or 30 minutes. Inactivity uses a monotonic timer: touch, enabled radar presence, and calibration restart it. Background Wi-Fi traffic, clock updates, and HTTP responses do not. Calibration also keeps the backlight at least half brightness. Changing the timeout restarts the countdown. Old profiles without `sleep_minutes` default to five minutes. Supported timeout values are imported from the legacy `weather/standbyMinutes` key without modifying it.

When the timeout expires, only the LCD backlight turns off. The ESP32, LCD controller, Wi-Fi, radar, clock, and forecast worker keep running, and the screen contents continue updating. Touch or enabled presence restores the saved brightness without restarting the firmware, reconnecting Wi-Fi, or changing the selected page. The waking touch is consumed until release so it does not activate a button. This keeps information immediately available, with higher power consumption than deep sleep. A failed backlight-off write leaves the screen on and retries after 60 seconds. A failed restore leaves it off and allows another activity to retry.

Night mode defines 22:00–06:00 in the selected location's IANA timezone, including DST. With a finite timeout, the same inactivity interval applies day and night. With **Never**, inactivity blanking is disabled, but an enabled night schedule can still turn the backlight off. A touch or enabled presence overrides that night's schedule until the next daytime period is observed. SNTP supplies UTC time after Wi-Fi connects. The night schedule waits for synchronised time and a valid timezone. A dark screen stays dark after 06:00 until touch or enabled presence restores it. The stored `sleep_minutes` key and profile version remain unchanged for compatibility.

## Location search

Search runs in a worker over certificate-verified HTTPS, requesting up to 15 results in the selected language. Select a result and tap **Save** to store its name, coordinates, and IANA timezone. Responses are limited to 32 KiB. The screen times out after 10 seconds and ignores cancelled, stale, or late results. Searches require Wi-Fi and synchronised time. The saved location, timezone, units, language, and clock format apply to all weather views.

## Weather fetching and rendering

Search results save an optional `compact_name` for the city-and-country label. Older profiles remain compatible. For three-part names, weather pages omit the intermediate region. Single names remain unchanged when country information is unavailable. Search and Settings keep the full location name.

All pages use the saved location's IANA timezone, including daylight-saving changes, and honour Celsius/Fahrenheit, 12/24-hour time, and the eight supported languages. The clock waits for synchronised time and a known timezone. It never substitutes UTC for an unresolved local timezone. The six time digits use fixed-width cells centred as one group, so changing digits never shifts the other digits or colons. Seconds repaint only the clock region, without repainting weather, presence, or buttons. Updates compare the previous and new clock contents in two reusable RGB565 strip buffers (18.5 KiB total). Only runs of actually changed pixels are sent to the LCD. Unchanged content causes no display writes. On the Clock page, forecast and status updates also compare finished pixels, reusing the same buffers for the weather card and status line. These updates leave the clocks, location, attribution and settings gear untouched. Week and Hours use the same differential updates for their current weather, status, and seven forecast rows, including hour and date rollovers. Their tables use the Clock weather card's darker blue. Text uses 19/22/31px fonts, with seven entries still visible without scrolling. The shared buffers keep the same 18.5 KiB allocation. Page changes and returns from Settings seed a new snapshot after a complete draw. Failed transfers invalidate the affected snapshot so a subsequent attempt restores that region. There is no visible background-clear pass during these updates and no PSRAM requirement. Settings retains its minute-only clock updates. Clock and HTTP traffic do not reset the inactivity timer.

Forecasts use the [Open-Meteo forecast API](https://open-meteo.com/en/docs) over certificate-verified HTTPS in the existing background worker. The firmware refreshes every ten minutes, retries errors after sixty seconds, and refreshes on reconnection or location change. Responses are limited to 32 KiB and HTTP operations to ten seconds, with a thirty-second application deadline allowing for a queued operation. Eight days of data are fetched to cover midnight rollover, while only seven daily and seven hourly entries are displayed. Temperatures are cached in Celsius and converted locally when units change.

Loading and failed requests have visible messages. Missing values show `--`. Unknown weather codes use a neutral unavailable symbol. Failed refreshes retain the previous same-location forecast and mark it outdated. Data also becomes outdated after twenty minutes without a successful refresh. Expired dates/hours are excluded, and unavailable future entries show placeholders. Changing the location clears the old forecast and ignores late responses. Forecasts are held only in RAM. If Wi-Fi drops, the existing connection status screen returns and reconnects. The selected weather page returns after connection succeeds.

## Weather symbols and UI icons

The white settings gear uses Tabler Icons' **settings** SVG by Paweł Kuna, under the MIT licence. The pinned source, revision, checksum, copyright and full licence are in `firmware/assets/ui/`. `tools/generate_ui_icons.py --check` verifies the generated alpha mask. See [Tabler Icons](https://github.com/tabler/tabler-icons). This symbol licence is separate from the CC BY 4.0 weather-data attribution.

Weather symbols are the static **flat Meteocons** collection by **Bas Milius**: [source repository](https://github.com/basmilius/meteocons), pinned npm package [`@meteocons/svg-static` 0.1.0](https://www.npmjs.com/package/@meteocons/svg-static/v/0.1.0).

**Licence: MIT**, with copyright **(c) 2020-present Bas Milius**. The licence permits free use, modification, embedding, and redistribution, including commercial use, provided the copyright and permission notice are retained.

The symbols retain their MIT licence independently of the MIT firmware. The complete [MIT licence](assets/weather/MIT.txt), [attribution and conversion notice](assets/weather/NOTICE.txt), [source/version/integrity manifest](assets/weather/sources.json), and [SHA-256 asset checksums](assets/weather/checksums.json) are included. Retain MIT.txt and NOTICE.txt with source and binary releases, along with the existing firmware and font notices. Credits do not imply endorsement.

The original SVG subset is retained under `firmware/assets/weather/svg/`. Thirteen symbols cover clear day/night, partly cloudy day/night, overcast, fog, drizzle, freezing precipitation, rain, snow, thunderstorms, thunderstorms with hail, and unavailable conditions. Symbols are converted to 32px forecast and 72px current-weather bitmaps with RGB565 colour and alpha transparency. Their shapes are preserved, and no runtime icon download or SVG library is required.

Regenerate or check assets using **CairoSVG 2.7.1**, **Cairo 1.16.0**, and **Pillow 9.0.1** (Cairo is a system library):

```sh
python3 -m venv --system-site-packages /tmp/weather-assets-venv
/tmp/weather-assets-venv/bin/pip install CairoSVG==2.7.1 Pillow==9.0.1
/tmp/weather-assets-venv/bin/python tools/generate_weather_icons.py
/tmp/weather-assets-venv/bin/python tools/generate_weather_icons.py --check
```

The converter verifies the archive's pinned SHA-512 integrity before reading it. CairoSVG and Pillow are build-time conversion tools with their own licences. They are not linked into the firmware. The clock adds a 42px Montserrat numeric subset under the existing OFL-1.1 font licence, generated by the font tool.

## HLK-LD2410C radar

**The HLK-LD2410C radar module’s CE status is unknown to this project. The existence of an EU declaration of conformity or US regulatory compliance documentation has not been verified. Check your local laws and regulatory requirements before using the radar. Use it at your own risk.**

The Rust receiver implements [Hi-Link’s LD2410C serial protocol V1.09](https://r0.hlktech.com/download/HLK-LD2410C-24G/1/LD2410C%20%E4%B8%B2%E5%8F%A3%E9%80%9A%E4%BF%A1%E5%8D%8F%E8%AE%AE%20V1.09.pdf). Firmware code is licensed under MIT.

| Radar connection | ESP32/display connection |
| --- | --- |
| TX | GPIO35 / GP35, UART1 receive |
| OUT | GPIO32 / P4 SDA, active-high presence input |
| RX | Unconnected. This firmware sends no sensor commands |
| GND | Common display ground |
| VCC | Existing regulated 5 V supply with capacity above 200 mA |

Sensor data lines are 3.3 V logic. UART1 runs at the sensor's factory setting of **256000 baud, 8N1**, leaving the CH340 USB connection on UART0 available for logs and flashing. GPIO39 belongs to the touch controller. UART0 and GPIO39 are not used for radar. The driver initialises after startup calibration and reads in bounded nonblocking batches from a 4096-byte receive buffer. No additional radar library or device configuration is required.

Both basic and engineering reports provide moving/stationary target flags, respective distances in centimetres, energy values, and detection distance. A fixed 64-byte parser validates framing and resynchronises after malformed data. After one second without a valid report, UART connectivity and all measurements are cleared. OUT is evaluated independently: an asserted OUT or a fresh positive UART report counts as presence. An inactive OUT does not override a positive UART report. Empty nonblocking reads retain partial frames. UART read errors clear their measurements and are retried on subsequent loops. Initialisation errors are logged without stopping the UI, and an available OUT input continues to work even if UART initialisation fails.

Presence from either a fresh positive UART report or OUT drives the saved wake preference. Sensing continues when wake is disabled and while the backlight is off. See the [user guide](../USER_GUIDE.md#presence-indicator) for indicator meanings
and operation.

## Appearance, fonts, and attribution

The UI uses a vertical gradient from `#4C8CB9` to `#A6CDEC`, white text, `#5E9BC8` panels, and 4-pixel rounded corners on buttons, keyboard keys, and input fields. Wi-Fi, settings, startup, and calibration screens share the same renderer and theme.

The original font is **Montserrat**, initially designed by **Julieta Ulanovsky** and developed by the Montserrat project contributors. Its origin repository is [JulietaUla/Montserrat](https://github.com/JulietaUla/Montserrat), and its published family page is [Montserrat on Google Fonts](https://fonts.google.com/specimen/Montserrat).

The firmware combines Montserrat Medium for standard characters and Montserrat Light for additional Latin characters, with 20-pixel headings, 14-pixel body/control text, and 12-pixel secondary labels, plus a 42-pixel numeric subset for the seconds clock. Week and Hours use complete 19/22/31-pixel Montserrat Medium subsets rasterised from the pinned TTF, including localised glyphs.

**Font licence: SIL Open Font Licence 1.1 (OFL-1.1).** The font assets retain this licence independently of the firmware's **MIT** licence. The OFL permits free use, embedding, modification, and redistribution, including with commercial software, subject to its conditions. It does not permit selling the font alone. The complete [OFL and copyright notice](assets/fonts/OFL.txt), [font attribution and modifications](assets/fonts/NOTICE.txt), and [LVGL MIT notice](assets/fonts/LVGL-MIT.txt) are included in this repository. Distribute these three files alongside the firmware, including binary releases. Retain the firmware code's separate MIT copyright and permission notice.

The converted bitmap assets are named **Weather Display UI Subset**, distinguishing them from the original font. The selected OFL notice declares no Reserved Font Names. The conversion preserves the LVGL ASCII glyphs and rasterises all non-ASCII UI glyphs directly from the pinned Montserrat Light TTF. Coverage is stored at four bits per pixel with sparse kerning pairs. The UI subset includes 216 characters: printable ASCII, 47 additional Latin letters and symbols, and all 66 uppercase/lowercase Russian letters, including Ё/ё, plus the eight additional Ukrainian letters Ґ/ґ, Є/є, І/і, and Ї/ї. The forecast subsets include 206 characters covering ASCII, localised messages, location-entry accents, and the complete Russian and Ukrainian alphabets. Other characters render as `?`. Font Awesome and weather icons are not included in this subset. Credits identify the sources and do not imply their authors endorse this project.

Reproducible source versions:

- [LVGL 9.2.2 fallback fonts](https://github.com/lvgl/lvgl/tree/7f07a129e8d77f4984fff8e623fd5be18ff42e74/src/font),
  commit `7f07a129e8d77f4984fff8e623fd5be18ff42e74`.
- [Montserrat Light TTF](https://github.com/JulietaUla/Montserrat/blob/555facfb2a18c72c3c0380f0d9c0f060453a9058/fonts/ttf/Montserrat-Light.ttf),
  commit `555facfb2a18c72c3c0380f0d9c0f060453a9058`, under the same OFL-1.1 notice.
- [Forecast Montserrat Medium TTF](https://github.com/JulietaUla/Montserrat/blob/555facfb2a18c72c3c0380f0d9c0f060453a9058/fonts/ttf/Montserrat-Medium.ttf),
  commit `555facfb2a18c72c3c0380f0d9c0f060453a9058`, under the same OFL-1.1 notice.
- [Published Montserrat OFL notice](https://github.com/google/fonts/blob/9710da1eacb3be272583c3224dcb70f9da6eadbb/ofl/montserrat/OFL.txt),
  Google Fonts commit `9710da1eacb3be272583c3224dcb70f9da6eadbb`.

The [font source manifest](assets/fonts/sources.json) records exact source URLs and SHA-256 checksums. Regenerate or verify the checked-in assets with Python with **Pillow 9.0.1 and FreeType 2.11.1** (both versions are checked):

```sh
python3 tools/generate_fonts.py
python3 tools/generate_fonts.py --check
```

ASCII and clock glyphs retain their original LVGL raster shapes. Non-ASCII UI glyphs are rasterised from Montserrat Light, and forecast glyphs from Montserrat Medium. Pillow is needed only for asset generation. Its [source and licence](https://github.com/python-pillow/Pillow/blob/9.0.1/LICENSE) are separate from the font licence.

The converter downloads pinned sources only on a cache miss and verifies their checksums. Font assets are embedded in flash. No font downloads or runtime font library are required on the device. Rendering uses a small text band rather than a full-screen framebuffer and requires no PSRAM.

Settings dependencies are pinned by `firmware/Cargo.lock`. The direct libraries [Serde](https://github.com/serde-rs/serde), [serde_json](https://github.com/serde-rs/json), [Chrono](https://github.com/chronotope/chrono), and [Chrono-TZ](https://github.com/chronotope/chrono-tz) offer MIT or Apache-2.0 licensing. Their upstream copyright and licence files are retained under [dependency notices](assets/licenses). Retain applicable notices when distributing the firmware. Chrono-TZ uses public-domain timezone data.

## Display and touch setup

The board uses classic ESP32 with 4 MiB flash, matching the chip identification from the verified original device backup. Its 4-inch ST7796 display uses SPI2 (HSPI): MOSI 13, MISO 12, SCLK 14, CS 15, DC 2, and active-high backlight 27. The XPT2046-compatible resistive touchscreen shares that bus with CS 33. Display transfers run at 40 MHz and touch transfers at 2.5 MHz, with separate chip selects. The display uses Freenove's ST7796 initialisation sequence, BGR colours, and portrait orientation. Software reset and a small SPI rendering buffer avoid needing PSRAM. See the [Freenove ESP32 Display resources](https://github.com/Freenove/Freenove_ESP32_Display).

Calibration is saved in NVS namespace `touch_cal`, key `portrait`. Missing, malformed or incompatible calibration starts guided setup. Storage failures report an error without erasing NVS. Touch filtering, pressure checks and press/release debounce produce one action per press. Calibration instructions are in the [user guide](../USER_GUIDE.md#recalibrating-touch).

The tracked partition table matches the original backup:

| Partition | Offset | Size |
| --- | --- | --- |
| NVS | `0x9000` | `0x5000` |
| OTA metadata | `0xe000` | `0x2000` |
| Application (`app0`, OTA slot 0) | `0x10000` | `0x300000` |
| SPIFFS | `0x310000` | `0xe0000` |
| Coredump | `0x3f0000` | `0x10000` |

No OTA update feature is implemented. Keeping these boundaries permits reuse of existing Wi-Fi settings and filesystem contents without a storage migration.

## Wi-Fi credential storage

SSID and password are stored **unencrypted** through ESP-IDF's native Wi-Fi configuration persistence. The SDK owns the NVS keys. This firmware does not use custom credential records or require HMAC keys or eFuse provisioning. Anyone able to read the device's flash can recover its saved password.

At startup, the SDK loads the existing station profile before connection settings are changed. Existing legacy settings can be reused when the original NVS partition is retained and the credentials meet the supported network/password requirements. An empty profile opens Wi-Fi setup. Invalid or unsupported profiles display a storage error without erasing saved data.

Connection attempts, scans, cancellation, and driver cleanup use RAM storage. Only after obtaining an IP address does the worker switch to flash storage and save the active station configuration, then restore RAM storage even if saving fails. Save errors disconnect the candidate and retain the previous selection in memory. Native SDK persistence does not provide a two-slot replacement guarantee across failed writes or power loss.

Password entry remains masked by default. Application password buffers and native configuration buffers are zeroised, and passwords are excluded from application logs. The Wi-Fi driver necessarily retains its active password in RAM. NVS initialisation never automatically erases a full, corrupt, or incompatible partition. Previously encrypted application profiles are not migrated or erased by this change. No scripts burn eFuses or erase/flash the device automatically.

The original full-flash backup remains separate from this port. This firmware requires the confirmed Freenove 4-inch ESP32 configuration. The ESP32-WROOM-32E module alone does not identify another manufacturer's display wiring.

## Tests and hardware acceptance

Run hardware-independent tests with the host Rust toolchain:

```sh
cargo +stable test --manifest-path firmware/Cargo.toml \
  --target x86_64-unknown-linux-gnu --no-default-features --lib
```

These tests exercise page transitions, keyboard characters and hit targets, stale events, reconnect deadlines, IP timeouts, cancellation, scan errors, and fake network/storage failures. They also verify that credentials are not saved before an IP address is obtained and that successful saves survive a simulated restart. Touch tests cover corner/centre mapping, swapped/inverted axes, invalid calibration, rejected noise and pressure, calibration persistence, and debounce. Theme tests verify proportional font metrics, kerning, glyph coverage, pixel-based wrapping and clipping, antialias blending, gradient endpoints, and rounded corners. Settings tests cover persistence and rollback, legacy settings imports, timezone/DST behaviour, screen-off deadlines, search bounds and stale results, confirmation flows, calibration timeouts and cancellation, and all language control layouts. Ukrainian tests cover the complete location keyboard, alphabet switching, search language and selection persistence. Compatibility tests cover the English fallback for unsupported legacy language selections. Radar tests exercise both report types and all target states, fragmentation, damaged frames, resynchronisation, timeout, OUT fallback, profile compatibility, and switch saving/rollback. Screen tests cover timeout boundaries, calibration and activity, Never and night overrides, staying dark after morning, consumed touches, restored brightness, and failed backlight writes with retry deadlines. Weather tests cover bounded parsing, missing values, unknown codes, refresh/retry deadlines, stale location responses, DST and fractional-offset timezones, and page navigation across settings and connection recovery. The preview tests verify that clipped settings-clock, seconds-clock, and presence updates match complete renders while leaving every other pixel unchanged. Forecast update tests cover all three views and every language/unit combination, assert that exactly the changed pixels are transferred, and verify no writes for unchanged contents and recovery after failed transfers. Daily probability tests cover fractional means, 0/100%, missing fields and null values, invalid ranges and array lengths. Probability-only updates are confined to the Week precipitation column. Larger-font checks cover all translated headings, numeric values, and hourly time labels without dates or timezone text. Rollover previews exercise midnight and both DST transitions in 12/24-hour formats. They also verify clock and forecast updates while dark and preservation of the current page and frame on wake.

```sh
cargo +stable test --manifest-path firmware/Cargo.toml \
  --target x86_64-unknown-linux-gnu --no-default-features --example preview
```

On the confirmed board, verify first-boot guided calibration, centre alignment, reboot persistence, and BOOT-triggered recalibration. Check red/green/blue, black, and white display output. Then verify all four screen corners, every keyboard mode, network paging, invalid passwords, successful setup, reboot persistence, turning the hotspot off/on, and replacing a network unsuccessfully. Verify loading an existing legacy profile while preserving its NVS partition. Inspect serial logs using a disposable test password to confirm it is never logged. A flash dump should contain the saved password in plaintext. Test storage errors and power loss during replacement. Real Wi-Fi interoperability, native persistence, and touch alignment require hardware testing.

Generate previews using the same renderer as the firmware:

```sh
cargo +stable run --manifest-path firmware/Cargo.toml \
  --target x86_64-unknown-linux-gnu --no-default-features --example preview
```

The example writes PPM images to `/tmp/weather-wifi-preview/`. An optional directory argument changes the destination. The previews cover every Wi-Fi page, keyboard modes, disabled controls, long and accented network names, password display, startup, and calibration, plus eight settings overlays in every supported language and three radar states per language. Weather previews include all three pages in every language, with ready, loading, error, outdated, night, and long-location scenarios. Russian and Ukrainian previews also cover Wi-Fi, startup, calibration, and all five city keyboard modes. Two additional English Wi-Fi previews provide concise examples for the user guide. Preview passwords and weather data are synthetic.

Hardware acceptance for backlight blanking requires flashing this build: verify each finite timeout, Never with night mode on/off, remaining dark after 06:00, touch wake without activating the underlying button, presence enabled/disabled, and calibration preventing blanking. Check that clock and forecasts update while dark and that wake restores the same page without a reboot or Wi-Fi reconnect. The target is under 100 ms from valid touch or presence to visible information. This latency requires measurement on the board.

Weather hardware acceptance requires the built firmware on the confirmed board: verify seconds without full-screen flicker, readable seven-row forecasts, tap cycling without triggering footer controls, settings changes, midnight/DST labels, responsive touch during HTTP requests, Wi-Fi loss/reconnection, and retained backlight-off/wake behaviour. Forecast unit tests and previews do not replace these checks.

## Refresh the documentation screenshots

After regenerating fonts when translations or keyboard characters change, render the UI and export the curated PNG screenshots used in the guide:

```sh
cargo +stable run --manifest-path firmware/Cargo.toml \
  --target x86_64-unknown-linux-gnu --no-default-features --example preview
python3 tools/generate_readme_images.py /tmp/weather-wifi-preview
```

The exporter uses Pillow to convert the renderer's PPM output without resizing and combines the three English weather pages into the README overview.

# Disclaimer

<b>This project and all associated files, documentation, and source code are provided “as is” without any express or implied warranties, including but not limited to the implied warranties of merchantability, fitness for a particular purpose, and non‑infringement. The author and contributors of this repository assume no responsibility or liability for any direct, indirect, incidental, or consequential damages that may occur through the use, modification, or distribution of the software and hardware designs contained herein. This includes, but is not limited to, hardware damage, data loss, malfunctioning devices, or personal injury that may arise from incorrect wiring, improper configuration, or misuse of the provided code and documentation. Users are encouraged to review, test, and verify all code before deploying it on any system. If you choose to use this project, you do so entirely at your own risk. By downloading, copying, modifying, or using any part of this project, you acknowledge that you have read, understood, and agree to this disclaimer.

The HLK-LD2410C radar module’s CE status is unknown to this project. The existence of an EU declaration of conformity or US regulatory compliance documentation has not been verified. Check your local laws and regulatory requirements before using the radar. Use it at your own risk.</b>
