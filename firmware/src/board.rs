// ============================================================================= //
// File          : board.rs                                                      //
// License       : MIT                                                           //
// Created       : 2026-10-04                                                    //
// Last modified : 2026-10-04                                                    //
// Author        : Daniel Kharlamov                                              //
// Last editor   : Daniel Kharlamov <daniel.kharlamov@googlemail.com>            //
// ============================================================================= //
//                                                                               //
// Summary:                                                                      //
// --------                                                                      //
// ESP32 board initialization, display, touch, and radar access.                 //
//                                                                               //
// Module:                                                                       //
// -------                                                                       //
// Initializes the shared SPI display and touch controller, backlight, radar,    //
// and board pins. Loads or performs touch calibration, maps touch events, and   //
// exposes full and partial UI rendering, brightness control, and radar polling  //
// to the application.                                                           //
// ============================================================================= //

//! Freenove ESP32 board peripherals and device UI integration.
//!
//! Initializes the LCD, shared-bus touch controller, calibration storage, radar,
//! and PWM backlight, and exposes their operations to the application loop.

use crate::{
    i18n::Language,
    model::App,
    touch::{self, Calibration, Press, RawPoint},
    ui,
};
use esp_idf_svc::{
    hal::{
        delay::Ets,
        gpio::{Input, Output, PinDriver, Pull},
        ledc::{config::TimerConfig, LedcDriver, LedcTimerDriver, LowSpeed},
        peripherals::Peripherals,
        spi::{
            config::{Config, DriverConfig},
            SpiDeviceDriver, SpiDriver,
        },
        units::Hertz,
    },
    nvs::{EspDefaultNvs, EspDefaultNvsPartition},
};
use mipidsi::{
    interface::SpiInterface,
    options::{ColorInversion, ColorOrder, Orientation},
    Builder,
};
use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

/// ST7796 display initialization for the Freenove board.
mod display;
use display::FreenoveSt7796;
type Spi = SpiDeviceDriver<'static, Arc<SpiDriver<'static>>>;
pub type Lcd = mipidsi::Display<
    SpiInterface<'static, Spi, PinDriver<'static, Output>>,
    FreenoveSt7796,
    mipidsi::NoResetPin,
>;
pub struct Board {
    // Combined presence indication from UART reports and the OUT pin. Stored as
    // crate::radar::device::Radar.
    radar: crate::radar::device::Radar,
    // Initialized 320-by-480 ST7796 display with the board's portrait colour/orientation settings.
    // Stored as Lcd.
    lcd: Lcd,
    // Reusable old/new strip buffers enabling changed-pixel display updates. Stored as
    // crate::weather_ui::ClockBuffer.
    clock_buffer: crate::weather_ui::ClockBuffer,
    // XPT2046 SPI device, configured for 2.5 MHz bidirectional sampling. Stored as Spi.
    touch: Spi,
    // GPIO27 PWM channel controlling LCD illumination. Stored as LedcDriver<'static>.
    backlight: LedcDriver<'static>,
    // Retained LEDC timer handle needed by the backlight channel. Stored as
    // Arc<LedcTimerDriver<'static, LowSpeed>>.
    _timer: Arc<LedcTimerDriver<'static, LowSpeed>>,
    // GPIO0 active-low BOOT button requesting recalibration or cancelling the wizard. Stored as
    // PinDriver<'static, Input>.
    boot: PinDriver<'static, Input>,
    // GPIO36 active-low touch interrupt; assertions require sample validation before actions.
    // Stored as PinDriver<'static, Input>.
    touch_irq: PinDriver<'static, Input>,
    // GPIO4 output held low to silence the board's audio path. Stored as PinDriver<'static,
    // Output>.
    _audio: PinDriver<'static, Output>,
    // Touch_cal NVS namespace used to persist the portrait mapping. Stored as EspDefaultNvs.
    calibration_store: EspDefaultNvs,
    // Active mapping from raw touch samples to portrait screen pixels. Stored as Calibration.
    calibration: Calibration,
    // Touch debouncer that suppresses repeated actions during a held gesture. Stored as Press.
    press: Press,
}
impl Board {
    /// Initializes display, touch, backlight, calibration storage, and radar on the Freenove
    /// board.
    ///
    /// # Arguments
    ///
    /// * `p` (`Peripherals`) - Owned ESP32 peripherals and pins used to initialize this board.
    /// * `nvs` (`EspDefaultNvsPartition`) - NVS partition or namespace used to access persisted
    ///   values.
    /// * `language` (`Language`) - Supported language used for translated labels or API
    ///   requests.
    /// * `brightness` (`u8`) - Backlight PWM duty in 0-255; startup uses at least 128 during
    ///   calibration.
    ///
    /// # Returns
    ///
    /// `anyhow::Result<(Self, esp_idf_svc::hal::modem::Modem<'static>)>` - Ok with the board
    /// and unused Wi-Fi modem; Err for peripheral, display, calibration storage, or allocation
    /// failure. Radar initialization failures are logged and tolerated.
    ///
    /// # Errors
    ///
    /// Propagates HAL and storage errors and reports display or calibration failures without
    /// erasing NVS.
    pub fn new(
        p: Peripherals,
        nvs: EspDefaultNvsPartition,
        language: Language,
        brightness: u8,
    ) -> anyhow::Result<(Self, esp_idf_svc::hal::modem::Modem<'static>)> {
        // LCD and XPT2046 share HSPI with independent chip selects and speeds.
        // Keep shared HSPI bus used by LCD and touch devices with independent chip selects in this
        // local variable for the following operations.
        let bus = Arc::new(SpiDriver::new(
            p.spi2,
            p.pins.gpio14,
            p.pins.gpio13,
            Some(p.pins.gpio12),
            &DriverConfig::new(),
        )?);
        // Keep LCD SPI device, configured for 40 MHz write-only transfers in this local variable
        // for the following operations.
        let spi = SpiDeviceDriver::new(
            bus.clone(),
            Some(p.pins.gpio15),
            &Config::new().baudrate(Hertz(40_000_000)).write_only(true),
        )?;
        // Keep XPT2046 SPI device, configured for 2.5 MHz bidirectional sampling in this local
        // variable for the following operations.
        let mut touch = SpiDeviceDriver::new(
            bus,
            Some(p.pins.gpio33),
            &Config::new().baudrate(Hertz(2_500_000)),
        )?;
        // Keep LCD command/data output connected to GPIO2 in this local variable for the following
        // operations.
        let dc = PinDriver::output(p.pins.gpio2)?;
        // Keep leaked 2048-byte transfer buffer whose lifetime matches the firmware process in this
        // local variable for the following operations.
        let buffer = Box::leak(vec![0u8; 2048].into_boxed_slice());
        // Keep mipidsi SPI transport combining the LCD device, command/data pin, and transfer
        // buffer in this local variable for the following operations.
        let interface = SpiInterface::new(spi, dc, buffer);
        // Keep initialized 320-by-480 ST7796 display with the board's portrait colour/orientation
        // settings in this local variable for the following operations.
        let mut lcd = Builder::new(FreenoveSt7796, interface)
            .display_size(320, 480)
            .orientation(Orientation::default().flip_horizontal())
            .color_order(ColorOrder::Bgr)
            .invert_colors(ColorInversion::Normal)
            .init(&mut Ets)
            .map_err(|_| anyhow::anyhow!("Display initialization failed"))?;
        // Keep shared 5 kHz LEDC timer retained while the backlight PWM channel is active in this
        // local variable for the following operations.
        let timer = Arc::new(LedcTimerDriver::new(
            p.ledc.timer0,
            &TimerConfig::new().frequency(Hertz(5000)),
        )?);
        // Keep GPIO27 PWM channel controlling LCD illumination in this local variable for the
        // following operations.
        let mut backlight = LedcDriver::new(p.ledc.channel0, timer.clone(), p.pins.gpio27)?;
        // Ask the PWM driver to apply the requested backlight brightness.
        backlight.set_duty(u32::from(brightness.max(128)))?;
        // Keep GPIO36 active-low touch interrupt; assertions require sample validation before
        // actions in this local variable for the following operations.
        let touch_irq = PinDriver::input(p.pins.gpio36, Pull::Floating)?;
        // Keep audio output explicitly driven low during startup in this local variable for the
        // following operations.
        let mut audio = PinDriver::output(p.pins.gpio4)?;
        // Drive the output low, keeping the board audio path silent.
        audio.set_low()?;
        // Keep GPIO0 active-low BOOT button requesting recalibration or cancelling the wizard in
        // this local variable for the following operations.
        let boot = PinDriver::input(p.pins.gpio0, Pull::Up)?;
        // Keep result of reading the touch_cal namespace and its optional portrait calibration in
        // this local variable for the following operations.
        let loaded = (|| -> Result<_, esp_idf_svc::sys::EspError> {
            // Keep persistent storage adapter used by this subsystem in this local variable for the
            // following operations.
            let store = EspDefaultNvs::new(nvs, "touch_cal", true)?;
            // Keep byte buffer used to decode, encode, or transfer the surrounding data in this
            // local variable for the following operations.
            let mut bytes = [0; 17];
            // Keep previously stored and validated portrait calibration, if available in this local
            // variable for the following operations.
            let saved = if store.blob_len("portrait")? == Some(bytes.len()) {
                // Produce the next fallible/optional stage, skipping it when an earlier stage is
                // unavailable.
                store
                    .get_blob("portrait", &mut bytes)?
                    .and_then(Calibration::decode)
            } else {
                // Return no available value as the value of this block.
                None
            };
            // Return success; the caller receives the ordered tuple of related values.
            Ok((store, saved))
        })();
        // Keep the returned components: `store` holds persistent storage adapter used by this
        // subsystem, `saved` holds previously stored and validated portrait calibration, if
        // available in this local variable for the following operations.
        let (store, saved) = match loaded {
            // Continue with the successful result, using its validated value in this case.
            Ok(loaded) => loaded,
            // Handle a failed operation here rather than treating its value as valid.
            Err(_) => {
                // Translates a startup or calibration prompt and draws it with an optional target.
                Self::prompt(
                    language,
                    &mut lcd,
                    "Touch storage could not be read.\nRestart and check device storage.",
                    None,
                )?;
                // Return an unrecoverable error immediately instead of continuing with invalid
                // hardware or storage state.
                anyhow::bail!("Touch calibration storage failed; NVS was not erased");
            }
        };
        // Keep whether a BOOT hold during startup requests replacing the stored mapping in this
        // local variable for the following operations.
        let recalibrate = if saved.is_some() {
            // Shows the startup prompt and checks whether BOOT requests recalibration.
            Self::startup(language, &mut lcd, &boot)?
        } else {
            false
        };
        // Keep active mapping from raw touch samples to portrait screen pixels in this local
        // variable for the following operations.
        let calibration = match saved.filter(|_| !recalibrate) {
            // Handle the Some(cal) case: apply the state-specific behavior shown here.
            Some(cal) => cal,
            // Handle the None case: apply the state-specific behavior shown here.
            None => {
                // Keep validated touch-coordinate transform in this local variable for the
                // following operations.
                let cal = Self::calibrate(language, &mut lcd, &mut touch)?;
                // Persistence failed; show an error rather than proceeding with a mapping that
                // would be lost at restart.
                if store.set_blob("portrait", &cal.encode()).is_err() {
                    // Translates a startup or calibration prompt and draws it with an optional
                    // target.
                    Self::prompt(
                        language,
                        &mut lcd,
                        "Touch calibration could not be saved.\nRestart and check device storage.",
                        None,
                    )?;
                    // Return an unrecoverable error immediately instead of continuing with invalid
                    // hardware or storage state.
                    anyhow::bail!("Touch calibration save failed; NVS was not erased");
                }
                // Return validated touch-coordinate transform as the value of this block.
                cal
            }
        };
        // Return success; the caller receives the ordered tuple of related values.
        Ok((
            Self {
                // Initialize combined presence indication from UART reports and the OUT pin from
                // the supplied value.
                radar: crate::radar::device::Radar::new(p.uart1, p.pins.gpio35, p.pins.gpio32),
                // Initialize initialized 320-by-480 ST7796 display with the board's portrait
                // colour/orientation settings from the supplied value.
                lcd,
                // Initialize reusable old/new strip buffers enabling changed-pixel display updates
                // from the supplied value.
                clock_buffer: crate::weather_ui::ClockBuffer::new()
                    .map_err(|_| anyhow::anyhow!("Clock buffer allocation failed"))?,
                // Initialize XPT2046 SPI device, configured for 2.5 MHz bidirectional sampling from
                // the supplied value.
                touch,
                // Initialize GPIO27 PWM channel controlling LCD illumination from the supplied
                // value.
                backlight,
                // Initialize retained LEDC timer handle needed by the backlight channel from the
                // supplied value.
                _timer: timer,
                // Initialize GPIO0 active-low BOOT button requesting recalibration or cancelling
                // the wizard from the supplied value.
                boot,
                // Initialize GPIO36 active-low touch interrupt; assertions require sample
                // validation before actions from the supplied value.
                touch_irq,
                // Initialize GPIO4 output held low to silence the board's audio path from the
                // supplied value.
                _audio: audio,
                // Initialize touch_cal NVS namespace used to persist the portrait mapping from the
                // supplied value.
                calibration_store: store,
                // Initialize active mapping from raw touch samples to portrait screen pixels from
                // the supplied value.
                calibration,
                // Initialize touch debouncer that suppresses repeated actions during a held gesture
                // from the supplied value.
                press: Press::default(),
            },
            p.modem,
        ))
    }
    /// Release holds left by older deep-sleep firmware before HAL initialization.
    ///
    /// Releases retained GPIO holds left by older deep-sleep firmware before HAL
    /// initialization.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `()` - No value; resets the board's held pins and removes RTC configuration from
    /// touch/radar inputs. Call before acquiring peripherals.
    pub fn release_sleep_holds() {
        use esp_idf_svc::sys::*;
        // SAFETY: called before peripherals are acquired; only this board's retained pins are touched.
        unsafe {
            // Reset the board pin state left by older deep-sleep firmware before the HAL takes
            // ownership.
            gpio_deep_sleep_hold_dis();
            // Run each listed scenario or target in the declared order; the loop variables select
            // the data for that case.
            for pin in [27, 15, 33, 4] {
                // Reset the board pin state left by older deep-sleep firmware before the HAL takes
                // ownership.
                gpio_set_direction(pin, gpio_mode_t_GPIO_MODE_OUTPUT);
                // Reset the board pin state left by older deep-sleep firmware before the HAL takes
                // ownership.
                gpio_set_level(pin, u32::from(pin == 15 || pin == 33));
                // Reset the board pin state left by older deep-sleep firmware before the HAL takes
                // ownership.
                gpio_hold_dis(pin);
            }
            // Reset the board pin state left by older deep-sleep firmware before the HAL takes
            // ownership.
            rtc_gpio_deinit(32);
            // Reset the board pin state left by older deep-sleep firmware before the HAL takes
            // ownership.
            rtc_gpio_deinit(36);
        }
    }
    /// Reads the active-low touch interrupt pin without debouncing.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Board`) - Receiver state used by this operation. Borrowed without changing
    ///   it.
    ///
    /// # Returns
    ///
    /// `bool` - True when PENIRQ is low; false otherwise. This is not a validated touch
    /// coordinate.
    pub fn touch_pressed(&self) -> bool {
        // Execute is low for GPIO36 active-low touch interrupt.
        self.touch_irq.is_low()
    }
    /// Translates a startup or calibration prompt and draws it with an optional target.
    ///
    /// # Arguments
    ///
    /// * `language` (`Language`) - Supported language used for translated labels or API
    ///   requests.
    /// * `lcd` (`&mut Lcd`) - Initialized LCD draw target for startup or calibration prompts.
    /// * `message` (`&str`) - Prompt text, optionally containing a newline between title and
    ///   body.
    /// * `target` (`Option<(i32, i32)>`) - Optional calibration cross coordinates in screen
    ///   pixels; None draws no cross.
    ///
    /// # Returns
    ///
    /// `anyhow::Result<()>` - Ok after drawing; Err with a display update error.
    ///
    /// # Errors
    ///
    /// Returns an error when the display rejects a drawing operation.
    fn prompt(
        language: Language,
        lcd: &mut Lcd,
        message: &str,
        target: Option<(i32, i32)>,
    ) -> anyhow::Result<()> {
        // Execute map err for render prompt result for the surrounding operation.
        ui::render_prompt(lcd, &crate::i18n::multiline(language, message), target)
            .map_err(|_| anyhow::anyhow!("Display update failed"))
    }

    /// Shows the startup prompt and checks whether BOOT requests recalibration.
    ///
    /// # Arguments
    ///
    /// * `language` (`Language`) - Supported language used for translated labels or API
    ///   requests.
    /// * `lcd` (`&mut Lcd`) - Initialized LCD draw target for startup or calibration prompts.
    /// * `boot` (`&PinDriver<'static, Input>`) - Active-low BOOT input used to request
    ///   recalibration.
    ///
    /// # Returns
    ///
    /// `anyhow::Result<bool>` - Ok(true) after BOOT is held for one second during the
    /// three-second window, Ok(false) otherwise; Err for drawing failure.
    ///
    /// # Errors
    ///
    /// Propagates startup prompt display errors.
    fn startup(
        language: Language,
        lcd: &mut Lcd,
        boot: &PinDriver<'static, Input>,
    ) -> anyhow::Result<bool> {
        // Translates a startup or calibration prompt and draws it with an optional target.
        Self::prompt(
            language,
            lcd,
            "Starting weather display...\nHold BOOT now to calibrate touch.",
            None,
        )?;
        // Keep monotonic starting instant, independent of wall-clock corrections in this local
        // variable for the following operations.
        let started = Instant::now();
        // Keep instant when the current continuous BOOT hold began in this local variable for the
        // following operations.
        let mut held_since = None;
        // Limit the optional BOOT recalibration prompt to a three-second startup window.
        while started.elapsed() < Duration::from_secs(3) {
            // The user is holding the active-low BOOT button; begin or continue timing a continuous
            // hold.
            if boot.is_low() {
                // Keep whether a touch is already treated as an active gesture in this local
                // variable for the following operations.
                let held = held_since.get_or_insert_with(Instant::now);
                // A one-second continuous BOOT hold confirms the explicit recalibration request.
                if held.elapsed() >= Duration::from_secs(1) {
                    // Leave this function now with a successful result carrying the enabled state;
                    // later statements are skipped.
                    return Ok(true);
                }
            } else {
                // Set instant when the current continuous BOOT hold began to no available value.
                held_since = None;
            }
            // Pause this worker briefly rather than busy-spinning while waiting for time or touch
            // changes.
            thread::sleep(Duration::from_millis(20));
        }
        // Return success; the caller receives the disabled state.
        Ok(false)
    }
    /// Performs one XPT2046 SPI conversion and extracts its 12-bit result.
    ///
    /// # Arguments
    ///
    /// * `touch` (`&mut Spi`) - SPI device for the shared-bus XPT2046 touch controller.
    /// * `command` (`u8`) - XPT2046 control byte selecting the requested ADC conversion.
    ///
    /// # Returns
    ///
    /// `Result<u16, esp_idf_svc::sys::EspError>` - Ok with an ADC value in 0-4095; Err with the
    /// SPI transfer error.
    ///
    /// # Errors
    ///
    /// Propagates SPI transfer failures.
    fn adc(touch: &mut Spi, command: u8) -> Result<u16, esp_idf_svc::sys::EspError> {
        // Keep three received SPI bytes containing the controller's 12-bit ADC response in this
        // local variable for the following operations.
        let mut response = [0; 3];
        // Execute transfer for XPT2046 SPI device, configured for 2.5 MHz bidirectional sampling.
        touch.transfer(&mut response, &[command, 0, 0])?;
        // Return success; the caller receives `(u16::from(response[1]) << 8)` combined with
        // `u16::from(response[2])` shifted right by 3 masked with 0x0fff.
        Ok(((u16::from(response[1]) << 8) | u16::from(response[2])) >> 3 & 0x0fff)
    }
    /// Reads five coordinate and pressure samples and applies touch filtering.
    ///
    /// # Arguments
    ///
    /// * `touch` (`&mut Spi`) - SPI device for the shared-bus XPT2046 touch controller.
    ///
    /// # Returns
    ///
    /// `Result<Option<RawPoint>, esp_idf_svc::sys::EspError>` - Ok(Some(point)) for accepted
    /// contact, Ok(None) for absent or rejected samples, or Err for SPI failure.
    ///
    /// # Errors
    ///
    /// Propagates coordinate or pressure conversion failures.
    fn raw(touch: &mut Spi) -> Result<Option<RawPoint>, esp_idf_svc::sys::EspError> {
        // Keep five coordinate/pressure samples passed through the noise filter in this local
        // variable for the following operations.
        let mut samples = [(RawPoint { x: 0, y: 0 }, 0); 5];
        // Visit each entry in five coordinate/pressure samples passed through the noise filter; the
        // loop binding provides its value or index for this iteration.
        for sample in &mut samples {
            // Keep first XPT2046 pressure conversion used to estimate contact strength in this
            // local variable for the following operations.
            let z1 = Self::adc(touch, 0xb0)?;
            // Keep second pressure conversion combined with z1 to reject weak contact in this local
            // variable for the following operations.
            let z2 = Self::adc(touch, 0xc0)?;
            // Keep 12-bit raw horizontal ADC reading before calibration in this local variable for
            // the following operations.
            let x = Self::adc(touch, 0xd0)?;
            // Keep 12-bit raw vertical ADC reading before calibration in this local variable for
            // the following operations.
            let y = Self::adc(touch, 0x90)?;
            // Set sample to the ordered tuple of related values.
            *sample = (RawPoint { x, y }, z1 + 4095 - z2);
        }
        // Return success; the caller receives filter result for the surrounding operation.
        Ok(touch::filter(samples))
    }
    /// Waits for a debounced touch press, polling at 20-millisecond intervals.
    ///
    /// # Arguments
    ///
    /// * `touch` (`&mut Spi`) - SPI device for the shared-bus XPT2046 touch controller.
    /// * `press` (`&mut Press`) - Mutable touch debouncer retaining contact and release
    ///   history.
    ///
    /// # Returns
    ///
    /// `anyhow::Result<RawPoint>` - Ok with the next accepted raw point; Err for touch SPI
    /// failure. Waiting has no deadline.
    ///
    /// # Errors
    ///
    /// Propagates touch-read failures.
    fn next_press(touch: &mut Spi, press: &mut Press) -> anyhow::Result<RawPoint> {
        // Keep processing events or samples until an explicit break, return, or channel shutdown
        // ends this loop.
        loop {
            // A debounced new press is available; return its raw coordinates to the calibration
            // collector.
            if let Some(point) = press.update(Self::raw(touch)?) {
                // Leave this function now with a successful result carrying coordinate sample
                // associated with the current touch or pixel operation; later statements are
                // skipped.
                return Ok(point);
            }
            // Pause this worker briefly rather than busy-spinning while waiting for time or touch
            // changes.
            thread::sleep(Duration::from_millis(20));
        }
    }
    /// Guides blocking corner calibration and center verification until a valid mapping is
    /// obtained.
    ///
    /// # Arguments
    ///
    /// * `language` (`Language`) - Supported language used for translated labels or API
    ///   requests.
    /// * `lcd` (`&mut Lcd`) - Initialized LCD draw target for startup or calibration prompts.
    /// * `touch` (`&mut Spi`) - SPI device for the shared-bus XPT2046 touch controller.
    ///
    /// # Returns
    ///
    /// `anyhow::Result<Calibration>` - Ok with a verified mapping after release; Err for touch
    /// or display failure. Invalid geometry repeats the wizard.
    ///
    /// # Errors
    ///
    /// Propagates prompt drawing and SPI read failures.
    fn calibrate(
        language: Language,
        lcd: &mut Lcd,
        touch: &mut Spi,
    ) -> anyhow::Result<Calibration> {
        // Keep touch debouncer that suppresses repeated actions during a held gesture in this local
        // variable for the following operations.
        let mut press = Press::default();
        // Keep processing events or samples until an explicit break, return, or channel shutdown
        // ends this loop.
        loop {
            // Keep ordered raw corner samples used to fit touch calibration in this local variable
            // for the following operations.
            let mut points = [RawPoint { x: 0, y: 0 }; 4];
            // Visit each entry in enumerate result for the surrounding operation; the loop binding
            // provides its value or index for this iteration.
            for (i, target) in touch::TARGETS.into_iter().enumerate() {
                // Translates a startup or calibration prompt and draws it with an optional target.
                Self::prompt(language, lcd, "Touch calibration\nTap the yellow cross with a stylus.\nLift the stylus between targets.", Some(target))?;
                // Set the selected entry from ordered raw corner samples used to fit touch
                // calibration to waits for a debounced touch press, polling at 20-millisecond
                // intervals.
                points[i] = Self::next_press(touch, &mut press)?;
            }
            // The four raw corners produced a valid mapping; independently verify it using the
            // center target.
            if let Some(cal) = Calibration::fit(points) {
                // Translates a startup or calibration prompt and draws it with an optional target.
                Self::prompt(
                    language,
                    lcd,
                    "Verify calibration\nTap the yellow cross at the center.",
                    Some(touch::CENTER),
                )?;
                // The center press agrees within twenty pixels, so this mapping can be accepted.
                if cal.verify_center(Self::next_press(touch, &mut press)?) {
                    // Wait for release before the Wi-Fi page accepts touches.
                    // Visit each entry in 0..3; the loop binding provides its value or index for
                    // this iteration.
                    for _ in 0..3 {
                        // Wait until the calibration touch is lifted so it cannot activate the next
                        // Wi-Fi page.
                        while Self::raw(touch)?.is_some() {
                            // Pause this worker briefly rather than busy-spinning while waiting for
                            // time or touch changes.
                            thread::sleep(Duration::from_millis(20));
                        }
                        // Pause this worker briefly rather than busy-spinning while waiting for
                        // time or touch changes.
                        thread::sleep(Duration::from_millis(20));
                    }
                    // Leave this function now with a successful result carrying validated
                    // touch-coordinate transform; later statements are skipped.
                    return Ok(cal);
                }
            }
            // Translates a startup or calibration prompt and draws it with an optional target.
            Self::prompt(
                language,
                lcd,
                "Calibration did not align.\nPlease try the targets again.",
                None,
            )?;
            // Pause this worker briefly rather than busy-spinning while waiting for time or touch
            // changes.
            thread::sleep(Duration::from_secs(2));
        }
    }
    /// Draws the full application page and refreshes the weather snapshot after success.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Board`) - Receiver state used by this operation. Mutated in place.
    /// * `app` (`&App`) - Application model supplying the active page, preferences, weather,
    ///   and presence state.
    ///
    /// # Returns
    ///
    /// `anyhow::Result<()>` - Ok after rendering; Err for display failure. Invalidates the
    /// prior partial-render baseline before drawing.
    ///
    /// # Errors
    ///
    /// Returns a display update error when full-page rendering fails.
    pub fn render(&mut self, app: &App) -> anyhow::Result<()> {
        // Execute invalidate for reusable old/new strip buffers enabling changed-pixel display
        // updates.
        self.clock_buffer.invalidate();
        // Execute map err for draws the full application page and refreshes the weather snapshot
        // after success.
        ui::render(&mut self.lcd, app).map_err(|_| anyhow::anyhow!("Display update failed"))?;
        // Cache a differential baseline only after a successful full draw of the unobscured
        // connected weather page.
        if app.page == crate::model::Page::Connected && app.preferences.overlay.is_none() {
            // Execute remember for reusable old/new strip buffers enabling changed-pixel display
            // updates.
            self.clock_buffer.remember(app);
        }
        // Return success after the required side effects are complete.
        Ok(())
    }

    /// Updates changed clock pixels using the cached rendering baseline.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Board`) - Receiver state used by this operation. Mutated in place.
    /// * `app` (`&App`) - Application model supplying the active page, preferences, weather,
    ///   and presence state.
    ///
    /// # Returns
    ///
    /// `anyhow::Result<()>` - Ok after the update or when unchanged; Err for display failure.
    ///
    /// # Errors
    ///
    /// Returns a clock display update error and leaves the failed baseline invalidated.
    pub fn render_weather_clock(&mut self, app: &App) -> anyhow::Result<()> {
        // Execute map err for draws the full application page and refreshes the weather snapshot
        // after success.
        self.clock_buffer
            .render(&mut self.lcd, app)
            .map_err(|_| anyhow::anyhow!("Clock display update failed"))
    }

    /// Updates changed weather or forecast pixels using the cached baseline.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Board`) - Receiver state used by this operation. Mutated in place.
    /// * `app` (`&App`) - Application model supplying the active page, preferences, weather,
    ///   and presence state.
    ///
    /// # Returns
    ///
    /// `anyhow::Result<()>` - Ok after the update or when unchanged; Err for display failure.
    ///
    /// # Errors
    ///
    /// Returns a weather display update error and leaves the failed baseline invalidated.
    pub fn render_weather(&mut self, app: &App) -> anyhow::Result<()> {
        // Execute map err for updates changed weather or forecast pixels using the cached baseline.
        self.clock_buffer
            .render_weather(&mut self.lcd, app)
            .map_err(|_| anyhow::anyhow!("Weather display update failed"))
    }

    /// Redraws only the clipped settings-clock area, preserving surrounding controls.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Board`) - Receiver state used by this operation. Mutated in place.
    /// * `app` (`&App`) - Application model supplying the active page, preferences, weather,
    ///   and presence state.
    ///
    /// # Returns
    ///
    /// `anyhow::Result<()>` - Ok after drawing; Err for display failure.
    ///
    /// # Errors
    ///
    /// Returns a display update error if the clipped render fails.
    pub fn render_settings_clock(&mut self, app: &App) -> anyhow::Result<()> {
        use embedded_graphics::{prelude::*, primitives::Rectangle};
        // Clipping preserves screen coordinates and leaves all controls untouched.
        // Keep screen-coordinate rectangle limiting the drawing operation in this local variable
        // for the following operations.
        let bounds = Rectangle::new(Point::new(16, 40), Size::new(288, 32));
        // Execute map err for draws the full application page and refreshes the weather snapshot
        // after success.
        crate::settings_ui::render(&mut self.lcd.clipped(&bounds), app)
            .map_err(|_| anyhow::anyhow!("Display update failed"))
    }
    /// Writes the requested duty to the backlight PWM driver.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Board`) - Receiver state used by this operation. Mutated in place.
    /// * `value` (`u8`) - Requested backlight PWM duty in 0-255; zero turns the backlight off.
    ///
    /// # Returns
    ///
    /// `anyhow::Result<()>` - Ok when the driver accepts the duty; Err for a PWM driver
    /// failure.
    ///
    /// # Errors
    ///
    /// Propagates backlight driver errors.
    pub fn brightness(&mut self, value: u8) -> anyhow::Result<()> {
        // Ask the PWM driver to apply the requested backlight brightness.
        self.backlight.set_duty(u32::from(value))?;
        // Return success after the required side effects are complete.
        Ok(())
    }
    /// Polls UART radar reports and the independent OUT signal.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Board`) - Receiver state used by this operation. Mutated in place.
    /// * `now` (`u64`) - Monotonic timestamp in milliseconds, using the same origin as stored
    ///   deadlines.
    ///
    /// # Returns
    ///
    /// `crate::radar::PresenceStatus` - Current combined presence status after stale-report
    /// expiry.
    pub fn poll_radar(&mut self, now: u64) -> crate::radar::PresenceStatus {
        // Execute poll for combined presence indication from UART reports and the OUT pin.
        self.radar.poll(now)
    }
    /// Copies the latest radar UART measurements.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Board`) - Receiver state used by this operation. Borrowed without changing
    ///   it.
    ///
    /// # Returns
    ///
    /// `crate::radar::RadarReading` - Current parsed reading, independent of the OUT signal.
    pub fn radar_reading(&self) -> crate::radar::RadarReading {
        // Execute reading for combined presence indication from UART reports and the OUT pin.
        self.radar.reading()
    }
    /// Redraws the presence indicator when visible on the current page.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Board`) - Receiver state used by this operation. Mutated in place.
    /// * `app` (`&App`) - Application model supplying the active page, preferences, weather,
    ///   and presence state.
    ///
    /// # Returns
    ///
    /// `anyhow::Result<()>` - Ok after drawing or when hidden; Err for display failure.
    ///
    /// # Errors
    ///
    /// Returns a presence display update error when drawing fails.
    pub fn render_presence(&mut self, app: &App) -> anyhow::Result<()> {
        // Execute map err for redraws the presence indicator when visible on the current page.
        ui::render_presence(&mut self.lcd, app)
            .map_err(|_| anyhow::anyhow!("Presence display update failed"))
    }
    /// Reads filtered touch samples and advances gesture debouncing.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Board`) - Receiver state used by this operation. Mutated in place.
    ///
    /// # Returns
    ///
    /// `Result<Option<crate::touch::TouchPhase>, esp_idf_svc::sys::EspError>` - Ok(Some(phase))
    /// for Down, Move, or Up, Ok(None) while waiting, or Err for SPI failure.
    ///
    /// # Errors
    ///
    /// Propagates touch SPI read errors.
    pub fn touch_event(
        &mut self,
    ) -> Result<Option<crate::touch::TouchPhase>, esp_idf_svc::sys::EspError> {
        // Keep coordinate sample associated with the current touch or pixel operation in this local
        // variable for the following operations.
        let point = Self::raw(&mut self.touch)?;
        // Return success; the caller receives event result for the surrounding operation.
        Ok(self.press.event(point))
    }
    /// Maps a raw point through the active portrait calibration.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Board`) - Receiver state used by this operation. Borrowed without changing
    ///   it.
    /// * `point` (`RawPoint`) - Raw touch point; None represents absent or rejected contact
    ///   when optional.
    ///
    /// # Returns
    ///
    /// `Option<(i32, i32)>` - Some bounded screen coordinates in pixels; None for invalid or
    /// out-of-range raw points.
    pub fn map(&self, point: RawPoint) -> Option<(i32, i32)> {
        // Maps a raw point through the active portrait calibration.
        self.calibration.map(point)
    }
    /// Feeds touch and BOOT cancellation state to the active calibration wizard.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Board`) - Receiver state used by this operation. Mutated in place.
    /// * `wizard` (`&mut crate::touch::Wizard`) - Mutable calibration wizard retaining stage,
    ///   samples, and deadline.
    /// * `now` (`u64`) - Monotonic timestamp in milliseconds, using the same origin as stored
    ///   deadlines.
    ///
    /// # Returns
    ///
    /// `anyhow::Result<crate::touch::WizardResult>` - Ok with the wizard result; Err for
    /// touch-read failure.
    ///
    /// # Errors
    ///
    /// Propagates SPI failures while collecting the raw touch point.
    pub fn poll_calibration(
        &mut self,
        wizard: &mut crate::touch::Wizard,
        now: u64,
    ) -> anyhow::Result<crate::touch::WizardResult> {
        // Return success; the caller receives update result for the surrounding operation.
        Ok(wizard.update(Self::raw(&mut self.touch)?, self.boot.is_low(), now))
    }
    /// Consumes the remaining gesture and persists an optional verified mapping.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Board`) - Receiver state used by this operation. Mutated in place.
    /// * `candidate` (`Option<Calibration>`) - Verified replacement mapping; None cancels
    ///   without persisting a new map.
    ///
    /// # Returns
    ///
    /// `anyhow::Result<()>` - Ok when no candidate is supplied or persistence succeeds; Err on
    /// save failure with the previous map retained.
    ///
    /// # Errors
    ///
    /// Propagates NVS calibration write failures.
    pub fn finish_calibration(&mut self, candidate: Option<Calibration>) -> anyhow::Result<()> {
        // Set touch debouncer that suppresses repeated actions during a held gesture to until
        // release result for the surrounding operation.
        self.press = Press::until_release();
        // A verified replacement mapping exists; persist it before making it active.
        if let Some(cal) = candidate {
            // Run the commit calibration operation with the supplied inputs.
            crate::touch::commit_calibration(&mut self.calibration, cal, |bytes| {
                // Write the encoded record to its NVS key; a reported failure leaves activation or
                // rollback to the caller.
                self.calibration_store.set_blob("portrait", bytes)
            })?;
        }
        // Return success after the required side effects are complete.
        Ok(())
    }
}
