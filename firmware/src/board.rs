// ============================================================================= //
// File          : board.rs                                                      //
// License       : GPL-3.0-only                                                  //
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

mod display;
use display::FreenoveSt7796;
type Spi = SpiDeviceDriver<'static, Arc<SpiDriver<'static>>>;
pub type Lcd = mipidsi::Display<
    SpiInterface<'static, Spi, PinDriver<'static, Output>>,
    FreenoveSt7796,
    mipidsi::NoResetPin,
>;
pub struct Board {
    radar: crate::radar::device::Radar,
    lcd: Lcd,
    clock_buffer: crate::weather_ui::ClockBuffer,
    touch: Spi,
    backlight: LedcDriver<'static>,
    _timer: Arc<LedcTimerDriver<'static, LowSpeed>>,
    boot: PinDriver<'static, Input>,
    touch_irq: PinDriver<'static, Input>,
    _audio: PinDriver<'static, Output>,
    calibration_store: EspDefaultNvs,
    calibration: Calibration,
    press: Press,
}
impl Board {
    pub fn new(
        p: Peripherals,
        nvs: EspDefaultNvsPartition,
        language: Language,
        brightness: u8,
    ) -> anyhow::Result<(Self, esp_idf_svc::hal::modem::Modem<'static>)> {
        // LCD and XPT2046 share HSPI with independent chip selects and speeds.
        let bus = Arc::new(SpiDriver::new(
            p.spi2,
            p.pins.gpio14,
            p.pins.gpio13,
            Some(p.pins.gpio12),
            &DriverConfig::new(),
        )?);
        let spi = SpiDeviceDriver::new(
            bus.clone(),
            Some(p.pins.gpio15),
            &Config::new().baudrate(Hertz(40_000_000)).write_only(true),
        )?;
        let mut touch = SpiDeviceDriver::new(
            bus,
            Some(p.pins.gpio33),
            &Config::new().baudrate(Hertz(2_500_000)),
        )?;
        let dc = PinDriver::output(p.pins.gpio2)?;
        let buffer = Box::leak(vec![0u8; 2048].into_boxed_slice());
        let interface = SpiInterface::new(spi, dc, buffer);
        let mut lcd = Builder::new(FreenoveSt7796, interface)
            .display_size(320, 480)
            .orientation(Orientation::default().flip_horizontal())
            .color_order(ColorOrder::Bgr)
            .invert_colors(ColorInversion::Normal)
            .init(&mut Ets)
            .map_err(|_| anyhow::anyhow!("Display initialization failed"))?;
        let timer = Arc::new(LedcTimerDriver::new(
            p.ledc.timer0,
            &TimerConfig::new().frequency(Hertz(5000)),
        )?);
        let mut backlight = LedcDriver::new(p.ledc.channel0, timer.clone(), p.pins.gpio27)?;
        backlight.set_duty(u32::from(brightness.max(128)))?;
        let touch_irq = PinDriver::input(p.pins.gpio36, Pull::Floating)?;
        let mut audio = PinDriver::output(p.pins.gpio4)?;
        audio.set_low()?;
        let boot = PinDriver::input(p.pins.gpio0, Pull::Up)?;
        let loaded = (|| -> Result<_, esp_idf_svc::sys::EspError> {
            let store = EspDefaultNvs::new(nvs, "touch_cal", true)?;
            let mut bytes = [0; 17];
            let saved = if store.blob_len("portrait")? == Some(bytes.len()) {
                store
                    .get_blob("portrait", &mut bytes)?
                    .and_then(Calibration::decode)
            } else {
                None
            };
            Ok((store, saved))
        })();
        let (store, saved) = match loaded {
            Ok(loaded) => loaded,
            Err(_) => {
                Self::prompt(
                    language,
                    &mut lcd,
                    "Touch storage could not be read.\nRestart and check device storage.",
                    None,
                )?;
                anyhow::bail!("Touch calibration storage failed; NVS was not erased");
            }
        };
        let recalibrate = if saved.is_some() {
            Self::startup(language, &mut lcd, &boot)?
        } else {
            false
        };
        let calibration = match saved.filter(|_| !recalibrate) {
            Some(cal) => cal,
            None => {
                let cal = Self::calibrate(language, &mut lcd, &mut touch)?;
                if store.set_blob("portrait", &cal.encode()).is_err() {
                    Self::prompt(
                        language,
                        &mut lcd,
                        "Touch calibration could not be saved.\nRestart and check device storage.",
                        None,
                    )?;
                    anyhow::bail!("Touch calibration save failed; NVS was not erased");
                }
                cal
            }
        };
        Ok((
            Self {
                radar: crate::radar::device::Radar::new(p.uart1, p.pins.gpio35, p.pins.gpio32),
                lcd,
                clock_buffer: crate::weather_ui::ClockBuffer::new()
                    .map_err(|_| anyhow::anyhow!("Clock buffer allocation failed"))?,
                touch,
                backlight,
                _timer: timer,
                boot,
                touch_irq,
                _audio: audio,
                calibration_store: store,
                calibration,
                press: Press::default(),
            },
            p.modem,
        ))
    }
    /// Release holds left by older deep-sleep firmware before HAL initialization.
    pub fn release_sleep_holds() {
        use esp_idf_svc::sys::*;
        // SAFETY: called before peripherals are acquired; only this board's retained pins are touched.
        unsafe {
            gpio_deep_sleep_hold_dis();
            for pin in [27, 15, 33, 4] {
                gpio_set_direction(pin, gpio_mode_t_GPIO_MODE_OUTPUT);
                gpio_set_level(pin, u32::from(pin == 15 || pin == 33));
                gpio_hold_dis(pin);
            }
            rtc_gpio_deinit(32);
            rtc_gpio_deinit(36);
        }
    }
    pub fn touch_pressed(&self) -> bool {
        self.touch_irq.is_low()
    }
    fn prompt(
        language: Language,
        lcd: &mut Lcd,
        message: &str,
        target: Option<(i32, i32)>,
    ) -> anyhow::Result<()> {
        ui::render_prompt(lcd, &crate::i18n::multiline(language, message), target)
            .map_err(|_| anyhow::anyhow!("Display update failed"))
    }

    fn startup(
        language: Language,
        lcd: &mut Lcd,
        boot: &PinDriver<'static, Input>,
    ) -> anyhow::Result<bool> {
        Self::prompt(
            language,
            lcd,
            "Starting weather display...\nHold BOOT now to calibrate touch.",
            None,
        )?;
        let started = Instant::now();
        let mut held_since = None;
        while started.elapsed() < Duration::from_secs(3) {
            if boot.is_low() {
                let held = held_since.get_or_insert_with(Instant::now);
                if held.elapsed() >= Duration::from_secs(1) {
                    return Ok(true);
                }
            } else {
                held_since = None;
            }
            thread::sleep(Duration::from_millis(20));
        }
        Ok(false)
    }
    fn adc(touch: &mut Spi, command: u8) -> Result<u16, esp_idf_svc::sys::EspError> {
        let mut response = [0; 3];
        touch.transfer(&mut response, &[command, 0, 0])?;
        Ok(((u16::from(response[1]) << 8) | u16::from(response[2])) >> 3 & 0x0fff)
    }
    fn raw(touch: &mut Spi) -> Result<Option<RawPoint>, esp_idf_svc::sys::EspError> {
        let mut samples = [(RawPoint { x: 0, y: 0 }, 0); 5];
        for sample in &mut samples {
            let z1 = Self::adc(touch, 0xb0)?;
            let z2 = Self::adc(touch, 0xc0)?;
            let x = Self::adc(touch, 0xd0)?;
            let y = Self::adc(touch, 0x90)?;
            *sample = (RawPoint { x, y }, z1 + 4095 - z2);
        }
        Ok(touch::filter(samples))
    }
    fn next_press(touch: &mut Spi, press: &mut Press) -> anyhow::Result<RawPoint> {
        loop {
            if let Some(point) = press.update(Self::raw(touch)?) {
                return Ok(point);
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
    fn calibrate(
        language: Language,
        lcd: &mut Lcd,
        touch: &mut Spi,
    ) -> anyhow::Result<Calibration> {
        let mut press = Press::default();
        loop {
            let mut points = [RawPoint { x: 0, y: 0 }; 4];
            for (i, target) in touch::TARGETS.into_iter().enumerate() {
                Self::prompt(language, lcd, "Touch calibration\nTap the yellow cross with a stylus.\nLift the stylus between targets.", Some(target))?;
                points[i] = Self::next_press(touch, &mut press)?;
            }
            if let Some(cal) = Calibration::fit(points) {
                Self::prompt(
                    language,
                    lcd,
                    "Verify calibration\nTap the yellow cross at the center.",
                    Some(touch::CENTER),
                )?;
                if cal.verify_center(Self::next_press(touch, &mut press)?) {
                    // Wait for release before the Wi-Fi page accepts touches.
                    for _ in 0..3 {
                        while Self::raw(touch)?.is_some() {
                            thread::sleep(Duration::from_millis(20));
                        }
                        thread::sleep(Duration::from_millis(20));
                    }
                    return Ok(cal);
                }
            }
            Self::prompt(
                language,
                lcd,
                "Calibration did not align.\nPlease try the targets again.",
                None,
            )?;
            thread::sleep(Duration::from_secs(2));
        }
    }
    pub fn render(&mut self, app: &App) -> anyhow::Result<()> {
        self.clock_buffer.invalidate();
        ui::render(&mut self.lcd, app).map_err(|_| anyhow::anyhow!("Display update failed"))?;
        if app.page == crate::model::Page::Connected && app.preferences.overlay.is_none() {
            self.clock_buffer.remember(app);
        }
        Ok(())
    }

    pub fn render_weather_clock(&mut self, app: &App) -> anyhow::Result<()> {
        self.clock_buffer
            .render(&mut self.lcd, app)
            .map_err(|_| anyhow::anyhow!("Clock display update failed"))
    }

    pub fn render_weather(&mut self, app: &App) -> anyhow::Result<()> {
        self.clock_buffer
            .render_weather(&mut self.lcd, app)
            .map_err(|_| anyhow::anyhow!("Weather display update failed"))
    }

    pub fn render_settings_clock(&mut self, app: &App) -> anyhow::Result<()> {
        use embedded_graphics::{prelude::*, primitives::Rectangle};
        // Clipping preserves screen coordinates and leaves all controls untouched.
        let bounds = Rectangle::new(Point::new(16, 40), Size::new(288, 32));
        crate::settings_ui::render(&mut self.lcd.clipped(&bounds), app)
            .map_err(|_| anyhow::anyhow!("Display update failed"))
    }
    pub fn brightness(&mut self, value: u8) -> anyhow::Result<()> {
        self.backlight.set_duty(u32::from(value))?;
        Ok(())
    }
    pub fn poll_radar(&mut self, now: u64) -> crate::radar::PresenceStatus {
        self.radar.poll(now)
    }
    pub fn radar_reading(&self) -> crate::radar::RadarReading {
        self.radar.reading()
    }
    pub fn render_presence(&mut self, app: &App) -> anyhow::Result<()> {
        ui::render_presence(&mut self.lcd, app)
            .map_err(|_| anyhow::anyhow!("Presence display update failed"))
    }
    pub fn touch_event(
        &mut self,
    ) -> Result<Option<crate::touch::TouchPhase>, esp_idf_svc::sys::EspError> {
        let point = Self::raw(&mut self.touch)?;
        Ok(self.press.event(point))
    }
    pub fn map(&self, point: RawPoint) -> Option<(i32, i32)> {
        self.calibration.map(point)
    }
    pub fn poll_calibration(
        &mut self,
        wizard: &mut crate::touch::Wizard,
        now: u64,
    ) -> anyhow::Result<crate::touch::WizardResult> {
        Ok(wizard.update(Self::raw(&mut self.touch)?, self.boot.is_low(), now))
    }
    pub fn finish_calibration(&mut self, candidate: Option<Calibration>) -> anyhow::Result<()> {
        self.press = Press::until_release();
        if let Some(cal) = candidate {
            crate::touch::commit_calibration(&mut self.calibration, cal, |bytes| {
                self.calibration_store.set_blob("portrait", bytes)
            })?;
        }
        Ok(())
    }
}
