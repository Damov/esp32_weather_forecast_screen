# ESP32 weather forecast screen

A desktop weather display built around a 4-inch ESP32 touchscreen. It shows the local time, current weather and daily and hourly forecasts from Open-Meteo. The project includes Rust firmware for the Freenove FNK0103S display with an ESP32-WROOM-32E module, plus a 3D-printable enclosure and stand. An optional HLK-LD2410C radar detects presence to wake the screen.

<p align="center">
<img src="images/20260929_134300.jpg" alt="Weather display in its printed enclosure and stand" width="400">
</p>

The photograph shows the assembled display running an older version of the firmware. The screenshots below show the current interface with example data.

## The interface

The 320 × 480 portrait display has a blue gradient background, white Montserrat text and darker blue weather panels. The Clock page combines an analogue dial with digital time and a local date. A rounded card below shows the weather symbol, current temperature and feels-like temperature.

The Week page shows today and the next six days, with weather symbols, precipitation probabilities and minimum and maximum temperatures. The Hours page shows the next seven hourly forecasts, with temperatures and precipitation probabilities. Both fit seven rows on the screen without scrolling. A settings gear and weather source credit sit at the bottom of each page.

<p align="center">
<img src="images/weather-overview.png" alt="Current Clock, Week and Hours pages, from left to right" width="800">
</p>

## Printing, assembly and use

The [printing and assembly guide](printable_3D_model/README.md) includes the model files, parts list, print settings and radar wiring. The [illustrated user guide](USER_GUIDE.md) covers firmware installation, first startup, Wi-Fi, touchscreen controls, location search and settings.

## Building on Linux

Install [Rust with rustup](https://rustup.rs/) and the Linux host dependencies listed in the [esp-rs prerequisites](https://github.com/esp-rs/esp-idf-template#generate-the-project), including CMake, Ninja, Python and libclang. Then install the ESP32 toolchain and linker proxy:

```sh
cargo install espup --locked
espup install
cargo install ldproxy --locked
```

In each build shell, load the environment created by espup. Its default path is:

```sh
. "$HOME/export-esp.sh"
```

From the repository root, build the release firmware:

```sh
./build.sh
```

The project selects the `esp` Rust toolchain, the `xtensa-esp32-espidf` target and ESP-IDF `v5.5.5`. The first build needs internet access to download the SDK and tools. Use a shell without an `IDF_PATH` override so the pinned SDK is used.

The release ELF is written to `firmware/target/xtensa-esp32-espidf/release/weather-forecast-firmware`. The script forwards extra Cargo arguments, such as `./build.sh --locked` once `firmware/Cargo.lock` exists. It only builds the software.

See the [firmware README](firmware/README.md) for implementation details, host tests, asset generation and documentation previews. Installation steps are in the [user guide](USER_GUIDE.md#installing-the-firmware-under-linux).

## Building on Windows

**TODO:** To be written in near future!

## Licences and credits

The firmware is licensed under [GPL-3.0-only](LICENSE). This project was inspired by [Aura](https://github.com/Surrey-Homeware/Aura).

The weather symbols are Bas Milius's flat [Meteocons](https://github.com/basmilius/meteocons), distributed under the [MIT licence](firmware/assets/weather/MIT.txt). The
[conversion notice](firmware/assets/weather/NOTICE.txt) describes the embedded bitmap versions. The settings gear comes from Paweł Kuna's [Tabler Icons](https://github.com/tabler/tabler-icons), also under [MIT](firmware/assets/ui/MIT.txt), with its own [attribution notice](firmware/assets/ui/NOTICE.txt).

The fonts are derived from [Montserrat](https://github.com/JulietaUla/Montserrat), originally designed by Julieta Ulanovsky and developed by the Montserrat project contributors. They retain the [SIL Open Font Licence 1.1](firmware/assets/fonts/OFL.txt). The [font notice](firmware/assets/fonts/NOTICE.txt) records the subset and bitmap conversions. The [LVGL MIT notice](firmware/assets/fonts/LVGL-MIT.txt) applies to the LVGL source material used in those conversions.

Keep the symbol and font licence and attribution files with source and binary releases. These assets retain their own licences alongside the firmware's GPL licence. Other retained dependency notices are in [the dependency notices directory](firmware/assets/licenses).

Weather and location data come from Open-Meteo, with location data from [GeoNames](https://www.geonames.org/). Open-Meteo data is attributed under [CC BY 4.0](https://open-meteo.com/en/licence). The display selects forecast entries, rounds temperatures, converts units and maps weather codes to symbols. The free public API has separate [service terms](https://open-meteo.com/en/terms), including non-commercial use and request limits.

## TODO

- [ ] Reuse the display’s BOOT button to trigger screen recalibration.
- [ ] Add a complete factory reset capability using the display’s RST button.

# Disclaimer

<b>This project and all associated files, documentation, and source code are provided “as is” without any express or implied warranties, including but not limited to the implied warranties of merchantability, fitness for a particular purpose, and non‑infringement. The author and contributors of this repository assume no responsibility or liability for any direct, indirect, incidental, or consequential damages that may occur through the use, modification, or distribution of the software and hardware designs contained herein. This includes, but is not limited to, hardware damage, data loss, malfunctioning devices, or personal injury that may arise from incorrect wiring, improper configuration, or misuse of the provided code and documentation. Users are encouraged to review, test, and verify all code before deploying it on any system. If you choose to use this project, you do so entirely at your own risk. By downloading, copying, modifying, or using any part of this project, you acknowledge that you have read, understood, and agree to this disclaimer.

The HLK-LD2410C radar module’s CE status is unknown to this project. The existence of an EU declaration of conformity or US regulatory compliance documentation has not been verified. Check your local laws and regulatory requirements before using the radar. Use it at your own risk.</b>

