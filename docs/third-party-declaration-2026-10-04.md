# Third-party sources and licences declaration

ESP32 weather forecast screen

**Declaration date:** 2026-10-04

This declaration identifies third-party software, visual assets and data
used by the project or its build tools. The project's firmware source code
is licensed under MIT. Third-party components retain their own
copyright notices and licence terms. This declaration provides attribution
and source references; it does not replace the upstream licence texts or
grant additional permissions.

The [collected release notices](licensing/THIRD-PARTY-NOTICES.txt) retain original
licence and copyright texts, supplemental source notices and certificate source
materials. The [release workflow](FIRMWARE_RELEASE_LICENSES.md) explains offline
validation and distribution packaging; its checks are not legal certification.

## Fonts

Montserrat was originally designed by Julieta Ulanovsky and is developed
by the Montserrat project contributors. The display uses modified bitmap
subsets of Montserrat Light and Medium at 12, 14, 19, 20, 22, 31 and 42 pixels.
These assets are named Weather Display UI Subset and remain under the SIL
Open Font Licence 1.1. Their source includes the Montserrat project at
commit 555facfb2a18c72c3c0380f0d9c0f060453a9058 and LVGL 9.2.2 font files
at commit 7f07a129e8d77f4984fff8e623fd5be18ff42e74. LVGL source material
has an additional MIT notice. The font subsets contain no Font Awesome
or weather icon glyphs.

Sources:

- [Montserrat](https://github.com/JulietaUla/Montserrat)
- [LVGL font sources](https://github.com/lvgl/lvgl/tree/7f07a129e8d77f4984fff8e623fd5be18ff42e74/src/font)

The exact font sources and checksums are in [firmware/assets/fonts/sources.json](../firmware/assets/fonts/sources.json).
Attribution, modification details and licence texts are in that directory's
[NOTICE.txt](../firmware/assets/fonts/NOTICE.txt), [OFL.txt](../firmware/assets/fonts/OFL.txt) and [LVGL-MIT.txt](../firmware/assets/fonts/LVGL-MIT.txt). Retain those notices with distributions
of the font assets. Modified fonts remain under OFL-1.1; the project's MIT
licence does not replace the font licence.

## Weather and interface symbols

Weather symbols are the flat Meteocons collection by Bas Milius, sourced
from @meteocons/svg-static version 0.1.0 under MIT. Thirteen SVG symbols
are converted to bitmap assets at 32 and 72 pixels, using RGB565 colour
and alpha coverage. Source SVGs, attribution, checksums and the MIT text
are retained under [firmware/assets/weather/](../firmware/assets/weather/).

Source: [Meteocons](https://github.com/basmilius/meteocons)

The settings gear is the settings SVG from Tabler Icons by Paweł Kuna,
revision 74929e50416e2b7c0abb8368cdc74bdcb2560ab6, under MIT. It is converted
to a 38-pixel alpha mask. The SVG, source metadata, attribution and licence
are retained under [firmware/assets/ui/](../firmware/assets/ui/).

Source: [Tabler Icons](https://github.com/tabler/tabler-icons)

## Display initialization

The ST7796 initialization sequence is adapted from Bodmer's TFT_eSPI 2.5.43,
as distributed with Freenove's display resources. The source file in that
package is identical to TFT_eSPI's upstream TFT_Drivers/ST7796_Init.h.
The TFT_eSPI licence file records MIT/BSD conditions and includes
Copyright (c) 2023 Bodmer. The unchanged upstream notice is retained in
[TFT_eSPI.txt](../firmware/assets/licenses/TFT_eSPI.txt). Keep this notice,
including its applicable copyright, licence conditions and disclaimer, with
source distributions and binary releases containing the adapted initialization
code. For binary releases, include it in the accompanying documentation or
other materials, such as the firmware release archive.

Sources:

- [TFT_eSPI ST7796 initialization](https://github.com/Bodmer/TFT_eSPI/blob/V2.5.43/TFT_Drivers/ST7796_Init.h)
- [TFT_eSPI licence](https://github.com/Bodmer/TFT_eSPI/blob/V2.5.43/license.txt)
- [Freenove display resources](https://github.com/Freenove/Freenove_ESP32_Display)

Freenove's overall resource package has separate CC BY-NC-SA 3.0 terms.
The TFT_eSPI attribution here covers the identified library source, not
permission to reuse other Freenove documentation or resources under MIT.

## Rust dependencies

The direct libraries are log, serde, serde_json, chrono, chrono-tz,
embedded-graphics, zeroize, esp-idf-svc, embedded-hal, mipidsi and anyhow,
with embuild used during compilation. [firmware/Cargo.toml](../firmware/Cargo.toml) defines these
dependencies and [firmware/Cargo.lock](../firmware/Cargo.lock) records their resolved versions.

[third-party-rust-2026-10-04.csv](third-party-rust-2026-10-04.csv) lists the 194 external package versions in
the lockfile, their declared licences, source repositories, package archive
URLs, SHA-256 checksums and licence files included in the archives. This
includes transitive dependencies, host tools and packages for other targets;
it is not a list of components necessarily present in every firmware image.
The `available_license_option` column identifies an available licence path
where upstream offers alternatives; it does not relicense a package.

The declared terms include MIT, Apache-2.0, BSD-3-Clause, ISC and Zlib.
unicode-ident additionally includes Unicode-3.0 terms, whose notice must
be retained for the covered material. regex-syntax also includes separately
licensed Unicode tables whose original notices are retained. Copyright and applicable licence
texts remain required even for packages without a separate licence file
inside the package archive. Retained notices for Chrono, Chrono-TZ, Serde
and serde_json are under [firmware/assets/licenses/](../firmware/assets/licenses/). Chrono-TZ also uses
public-domain IANA time-zone data.

## ESP-IDF and Espressif binary libraries

The firmware targets ESP32 using ESP-IDF v5.5.5 and the esp Rust toolchain.
ESP-IDF's original open source code is under Apache-2.0. Other SDK sources
include FreeRTOS (MIT), lwIP and wpa_supplicant (BSD), TLSF (BSD-3-Clause),
Newlib (component-specific terms) and Mbed TLS (Apache-2.0 or GPL-2.0-or-later,
with Apache-2.0 selected for this distribution).
Individual source notices and upstream licence texts govern each component.

Sources:

- [ESP-IDF v5.5.5](https://github.com/espressif/esp-idf/tree/v5.5.5)
- [ESP-IDF copyrights and licences](https://docs.espressif.com/projects/esp-idf/en/v5.5.5/esp32/COPYRIGHT.html)

[third-party-esp-idf-2026-10-04.json](third-party-esp-idf-2026-10-04.json) records the SDK's upstream submodule
repositories, exact revisions and licence source links. It includes optional
components and other chip targets; inclusion depends on build configuration.

The normal ESP32 WLAN build links precompiled Wi-Fi and PHY libraries whose
complete implementation source is not publicly available. The SDK selects:

Wi-Fi: espressif/esp32-wifi-lib, Apache-2.0,
revision 22bb3517b08760ed43b889013f8a122db0d46da0.

- [esp32-wifi-lib source repository](https://github.com/espressif/esp32-wifi-lib/tree/22bb3517b08760ed43b889013f8a122db0d46da0)

PHY: espressif/esp-phy-lib, Apache-2.0,
revision 59c1234e929212aec0fdda75769b759951235536.

- [esp-phy-lib source repository](https://github.com/espressif/esp-phy-lib/tree/59c1234e929212aec0fdda75769b759951235536)

Configuration-dependent coexistence libraries come from esp-coex-lib under
Apache-2.0, revision 79e618f033e926d134decc6d95bb1b1850bd032c.

- [esp-coex-lib source repository](https://github.com/espressif/esp-coex-lib/tree/79e618f033e926d134decc6d95bb1b1850bd032c)

Publishing the project's open source code does not change these libraries'
source availability. The project's MIT licence permits distribution of the
firmware without requiring disclosure of those libraries' implementation
source. Binary distributions must retain the project's MIT copyright and
permission notice and satisfy each included component's upstream terms,
including applicable licence, copyright and NOTICE requirements. This
declaration does not replace those terms or grant exemptions from them.

## HTTPS certificate data

HTTPS verification uses ESP-IDF's full root CA certificate bundle, derived
from Mozilla certificate data dated 2025-02-25 in the pinned SDK. The
converted Mozilla certificate store is under MPL 2.0. Preserve applicable
notices and provide recipients with access to the MPL-covered data source
when distributing the generated bundle. The data's licence does not replace
the project's own code licence.

Sources:

- [ESP-IDF certificate data](https://github.com/espressif/esp-idf/blob/v5.5.5/components/mbedtls/esp_crt_bundle/cacrt_all.pem)
- [Mozilla CA certificate extraction and licence](https://curl.se/docs/caextract.html)
- [MPL 2.0 distribution guidance](https://www.mozilla.org/en-US/MPL/2.0/FAQ/)

## Weather and location data

Weather forecasts and current conditions come from Open-Meteo. Location
search uses Open-Meteo's geocoding service with location data from GeoNames.
Open-Meteo supplies data under CC BY 4.0. The display selects forecast
entries, rounds temperatures, converts units and maps weather codes to
symbols. Data attribution and identification of modifications accompany
redistribution of the data.

Sources and terms:

- [Open-Meteo](https://open-meteo.com/)
- [GeoNames](https://www.geonames.org/)
- [Open-Meteo data licence](https://open-meteo.com/en/licence)
- [Open-Meteo service terms](https://open-meteo.com/en/terms)

The free public API permits non-commercial service use and imposes request
limits. Commercial service use requires an appropriate arrangement or another
provider. These conditions govern API access; they do not impose a
non-commercial restriction on the project's MIT-licensed source code.

## Build and asset tools

The development workflow uses Rust/Cargo, rustup, espup, ldproxy, CMake,
Ninja, Python, libclang and esptool. Asset conversion uses Pillow 9.0.1,
FreeType 2.11.1, CairoSVG 2.7.1, Cairo 1.16.0 and cairocffi. Generated fonts
and symbols are checked into the repository, so ordinary firmware builds
do not require these asset converters. The tools retain their own upstream
licences; using a separate build, conversion or flashing tool does not
replace the licences of the project's source or assets.

This dated declaration and its inventories identify the dependency versions
and sources recorded here. Applicable upstream copyright, licence and
NOTICE texts remain authoritative and must be retained where required.
