// ============================================================================= //
// File          : main.rs                                                       //
// License       : GPL-3.0-only                                                  //
// Created       : 2026-10-04                                                    //
// Last modified : 2026-10-04                                                    //
// Author        : Daniel Kharlamov                                              //
// Last editor   : Daniel Kharlamov <daniel.kharlamov@googlemail.com>            //
// ============================================================================= //
//                                                                               //
// Summary:                                                                      //
// --------                                                                      //
// ESP32 firmware startup and the main application loop.                         //
//                                                                               //
// Module:                                                                       //
// -------                                                                       //
// Initializes logging, storage, board peripherals, and background Wi-Fi and     //
// HTTP workers. Coordinates application events, touch input, settings           //
// persistence, forecast refreshes, synchronized time, radar presence, and       //
// backlight control. Schedules full or partial UI redraws and handles device    //
// errors.                                                                       //
// ============================================================================= //

//! ESP32 application entry point and main event loop.
//!
//! Coordinates background network and HTTP workers with touch, settings, radar,
//! backlight scheduling, and full or partial display updates.

use esp_idf_svc::{hal::peripherals::Peripherals, nvs::EspDefaultNvsPartition};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use weather_forecast_firmware::{
    board::Board,
    location,
    model::{App, Command, Event, Page},
    network,
    screen::ScreenController,
    settings::{Overlay, Preferences, Settings},
    settings_store::Store,
    settings_ui::{self, Effect},
    touch::{TouchPhase, WizardResult},
    ui,
};

/// Starts firmware services and coordinates device events, weather, settings, touch,
/// presence, and rendering.
///
/// # Arguments
///
/// None.
///
/// # Returns
///
/// `anyhow::Result<()>` - Ok if the application returns normally; Err on unrecoverable
/// startup or device operations. Normal operation runs continuously.
///
/// # Errors
///
/// Propagates initialization, thread-spawn, and fallible device operations; recoverable
/// failures are reflected in application state.
fn main() -> anyhow::Result<()> {
    // Link the ESP-IDF compatibility patches required by the Rust service bindings.
    esp_idf_svc::sys::link_patches();
    // Route log messages through the SDK logger for device diagnostics.
    esp_idf_svc::log::EspLogger::initialize_default();
    // Release GPIO holds left by older firmware before acquiring HAL peripherals.
    Board::release_sleep_holds();
    // Keep ESP-IDF nonvolatile-storage partition or namespace handle in this local variable for the
    // following operations.
    let nvs = EspDefaultNvsPartition::take_with(false)
        .map_err(|_| anyhow::anyhow!("NVS initialization failed; storage was not erased"))?;
    // Keep persistent storage adapter used by this subsystem in this local variable for the
    // following operations.
    let store = Store::open(nvs.clone());
    // Keep result of opening and decoding saved preferences; failure enables read-only mode in this
    // local variable for the following operations.
    let loaded = store
        .as_ref()
        .map_err(|_| "Settings could not be read.")
        .and_then(|store| store.load(nvs.clone()));
    // Disable preference writes when loading failed, preserving unreadable storage instead of
    // overwriting it with defaults.
    let read_only = loaded.is_err();
    // Keep device preferences controlling location, language, units, brightness, and clock format
    // in this local variable for the following operations.
    let settings = loaded.unwrap_or_else(|_| Settings::default());
    // Keep the returned components: `board` holds initialized Freenove peripherals providing
    // display, touch, radar, and backlight access, `modem` holds remaining Wi-Fi modem peripheral
    // handed to the network worker in this local variable for the following operations.
    let (mut board, modem) = Board::new(
        Peripherals::take()?,
        nvs.clone(),
        settings.language,
        settings.brightness,
    )?;
    // Keep the returned components: `sender` holds initial Wi-Fi command sender, renamed for its
    // application role, `rx` holds Wi-Fi command receiver moved into the network worker in this
    // local variable for the following operations.
    let (sender, rx) = mpsc::channel();
    // Keep sending end of the Wi-Fi command channel in this local variable for the following
    // operations.
    let commands = sender;
    // Keep the returned components: `tx` holds Wi-Fi event sender moved into the network worker,
    // `events` holds Wi-Fi event receiver polled by the main loop in this local variable for the
    // following operations.
    let (tx, events) = mpsc::channel();
    // Keep shared indication that Wi-Fi has an assigned IP connection in this local variable for
    // the following operations.
    let online = Arc::new(AtomicBool::new(false));
    // Obtain an owned clone of shared Wi-Fi readiness handle moved into the connection worker;
    // reference-counted handles continue to share their underlying state.
    let network_online = online.clone();
    // Start the configured worker thread; the closure owns the captured handles and runs
    // independently of the UI loop.
    thread::Builder::new()
        .name("wifi".into())
        .stack_size(16384)
        .spawn(move || network::run(modem, nvs, rx, tx, network_online))?;
    // Keep the returned components: `requests` holds one-slot HTTP work queue preventing unbounded
    // pending searches, `request_rx` holds receiving end of the HTTP work queue moved into the
    // location worker in this local variable for the following operations.
    let (requests, request_rx) = mpsc::sync_channel(1);
    // Keep the returned components: `response_tx` holds HTTP result sender moved into the location
    // worker, `responses` holds HTTP result receiver polled by the main loop in this local variable
    // for the following operations.
    let (response_tx, responses) = mpsc::channel();
    // Keep whether SNTP has supplied trusted time for TLS and local clock calculations in this
    // local variable for the following operations.
    let synchronized = Arc::new(AtomicBool::new(false));
    // Obtain an owned clone of shared SNTP readiness handle moved into the HTTP worker;
    // reference-counted handles continue to share their underlying state.
    let http_sync = synchronized.clone();
    // Obtain an owned clone of shared Wi-Fi readiness handle moved into the HTTP worker;
    // reference-counted handles continue to share their underlying state.
    let http_online = online.clone();
    // Start the configured worker thread; the closure owns the captured handles and runs
    // independently of the UI loop.
    thread::Builder::new()
        .name("location".into())
        .stack_size(16384)
        .spawn(move || location::run(request_rx, response_tx, http_online, http_sync))?;
    // Keep application state used by navigation, preferences, weather, and rendering in this local
    // variable for the following operations.
    let mut app = App {
        // Initialize editable preferences and their saved rollback baseline from the supplied
        // value.
        preferences: Preferences::new(settings, read_only),
        ..App::default()
    };
    // Keep monotonic starting instant, independent of wall-clock corrections in this local variable
    // for the following operations.
    let started = Instant::now();
    // Keep backlight state machine retaining inactivity, night overrides, and hardware retry
    // deadlines in this local variable for the following operations.
    let mut screen = ScreenController::new(&app.preferences.current, 0);
    // Keep whether the entire visible page needs to be redrawn in this local variable for the
    // following operations.
    let mut dirty = true;
    // Keep whether the minute-only settings clock needs to be repainted in this local variable for
    // the following operations.
    let mut clock_dirty = false;
    // Keep whether the analog/digital weather clock needs a differential update in this local
    // variable for the following operations.
    let mut weather_clock_dirty = false;
    // Keep whether current weather or the forecast table needs a differential update in this local
    // variable for the following operations.
    let mut weather_dirty = false;
    // Keep whether the presence indicator changed and needs a small redraw in this local variable
    // for the following operations.
    let mut presence_dirty = false;
    // Keep whether touch I/O has entered the fatal error state in this local variable for the
    // following operations.
    let mut touch_failed = false;
    // Keep last whole monotonic second processed, preventing repeated clock work within one second
    // in this local variable for the following operations.
    let mut last_second = u64::MAX;
    // Keep next monotonic millisecond deadline for resolving an imported location's timezone in
    // this local variable for the following operations.
    let mut resolve_at = 0;
    // Keep identifier used to reject stale timezone-resolution responses in this local variable for
    // the following operations.
    let mut resolve_id = 0;
    // Keep processing events or samples until an explicit break, return, or channel shutdown ends
    // this loop.
    loop {
        // Keep elapsed monotonic milliseconds supplied to radar, touch, weather, and backlight
        // controllers in this local variable for the following operations.
        let milliseconds = started.elapsed().as_millis() as u64;
        // Keep elapsed monotonic seconds used by the Wi-Fi model's reconnect scheduler in this
        // local variable for the following operations.
        let now = milliseconds / 1000;
        // Keep combined radar presence used by the indicator and optional screen wake policy in
        // this local variable for the following operations.
        let presence = board.poll_radar(milliseconds);
        // Update whether the presence indicator changed and needs a small redraw using combined
        // radar presence used by the indicator and optional screen wake policy differs from
        // combined presence indication from UART reports and the OUT pin, retaining the accumulated
        // state for subsequent steps.
        presence_dirty |= presence != app.radar;
        // Set combined presence indication from UART reports and the OUT pin to combined radar
        // presence used by the indicator and optional screen wake policy.
        app.radar = presence;
        // Check whether elapsed monotonic seconds used by the Wi-Fi model's reconnect scheduler
        // differs from last whole monotonic second processed, preventing repeated clock work within
        // one second.
        if now != last_second {
            // Set last whole monotonic second processed, preventing repeated clock work within one
            // second to elapsed monotonic seconds used by the Wi-Fi model's reconnect scheduler.
            last_second = now;
            // Keep UTC Unix timestamp in seconds; an optional value is unavailable before time
            // synchronization in this local variable for the following operations.
            let epoch = if synchronized.load(Ordering::Relaxed) {
                // Produce the transformed value or entries produced by the closure.
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .ok()
                    .map(|time| time.as_secs() as i64)
            } else {
                // Return no available value as the value of this block.
                None
            };
            // Keep prior state used to detect changes without redrawing unchanged content in this
            // local variable for the following operations.
            let previous = app
                .weather
                .range_key(&app.preferences.current, app.preferences.epoch);
            // Keep current state or measurement being prepared for display in this local variable
            // for the following operations.
            let current = app.weather.range_key(&app.preferences.current, epoch);
            // Keep whether the connected weather page is currently unobscured by a settings overlay
            // in this local variable for the following operations.
            let weather_visible = app.page == Page::Connected && app.preferences.overlay.is_none();
            // Check whether `weather_visible && previous != current`.
            if weather_visible && previous != current {
                // Marks weather dirty only when the connected weather page is visible.
                mark_weather_update(&app, &mut weather_dirty);
            }
            // Update whether the analog/digital weather clock needs a differential update using
            // whether the connected weather page is currently unobscured by a settings overlay and
            // view equals weather_forecast_firmware weather View Clock and local result for the
            // surrounding operation differs from local result for the surrounding operation,
            // retaining the accumulated state for subsequent steps.
            weather_clock_dirty |= weather_visible
                && app.weather.view == weather_forecast_firmware::weather::View::Clock
                && weather_forecast_firmware::weather::local(&app.preferences.current, epoch)
                    != weather_forecast_firmware::weather::local(
                        &app.preferences.current,
                        app.preferences.epoch,
                    );
            // Update whether the minute-only settings clock needs to be repainted using update time
            // result for the surrounding operation, retaining the accumulated state for subsequent
            // steps.
            clock_dirty |= app.preferences.update_time(epoch);
        }
        // Execute observe for backlight state machine retaining inactivity, night overrides, and
        // hardware retry deadlines.
        screen.observe(
            &app.preferences.current,
            app.preferences.epoch,
            milliseconds,
            app.preferences.overlay.is_some_and(|o| {
                matches!(
                    o,
                    Overlay::CalibrationIntro
                        | Overlay::CalibrationRunning
                        | Overlay::CalibrationError
                )
            }) || touch_failed
                || app.page == Page::Fatal,
            app.radar.present(),
            board.touch_pressed(),
        );
        // Visit each entry in try iter result for the surrounding operation; the loop binding
        // provides its value or index for this iteration.
        for event in events.try_iter() {
            // Keep overlay in this local variable for the following operations.
            let overlay = app.preferences.overlay;
            // Keep status displayed to the reader instead of silent failure in this local variable
            // for the following operations.
            let status = app.preferences.status;
            // Check whether an available handle result for the surrounding operation matches the
            // requested case.
            if let Some(command) = app.handle(event, now) {
                // Perform the best-effort operation and intentionally ignore its result: Deliver
                // the event or command to its worker/application channel; this send waits if
                // required by the channel.
                let _ = commands.send(command);
            }
            // Update whether the entire visible page needs to be redrawn using overlay is
            // unavailable or overlay differs from overlay or status displayed to the reader instead
            // of silent failure differs from status displayed to the reader instead of silent
            // failure, retaining the accumulated state for subsequent steps.
            dirty |= overlay.is_none()
                || overlay != app.preferences.overlay
                || status != app.preferences.status;
        }
        // Keep whether adopting the selected location cleared the old forecast and pending request
        // in this local variable for the following operations.
        let location_changed = app.weather.adopt(&app.preferences.current.location);
        // Update whether the entire visible page needs to be redrawn using whether adopting the
        // selected location cleared the old forecast and pending request and currently selected
        // Wi-Fi navigation page equals Connected state and overlay is unavailable, retaining the
        // accumulated state for subsequent steps.
        dirty |=
            location_changed && app.page == Page::Connected && app.preferences.overlay.is_none();
        // Visit each entry in try iter result for the surrounding operation; the loop binding
        // provides its value or index for this iteration.
        for response in responses.try_iter() {
            // Choose the appropriate path for parsed service result or hardware reply being
            // handled; each arm handles one supported case.
            match response {
                // Handle the location Response Forecast(id, location, result) case: apply the
                // state-specific behavior shown here.
                location::Response::Forecast(id, location, result) => {
                    // Keep whether the handled response or user action requires a display update in
                    // this local variable for the following operations.
                    let changed = app.weather.receive(
                        id,
                        &location,
                        result,
                        milliseconds,
                        app.preferences.epoch,
                    );
                    // Check whether whether the handled response or user action requires a display
                    // update.
                    if changed {
                        // Marks weather dirty only when the connected weather page is visible.
                        mark_weather_update(&app, &mut weather_dirty);
                    }
                    // Skip the remainder of this iteration and wait for or inspect the next input.
                    continue;
                }
                // Handle the location Response Locations(id, result) case: Execute locations for
                // editable preferences and their saved rollback baseline.
                location::Response::Locations(id, result) => app.preferences.locations(id, result),
                // Handle the location Response Timezone(id, Ok(location))                     if id
                // == resolve id                         &&
                // app.preferences.current.location.timezone.is empty()                         &&
                // location.name == app.preferences.current.location.name                         &&
                // location.latitude == app.preferences.current.location.latitude
                // && location.longitude == app.preferences.current.location.longitude case: apply
                // the state-specific behavior shown here.
                location::Response::Timezone(id, Ok(location))
                    if id == resolve_id
                        && app.preferences.current.location.timezone.is_empty()
                        && location.name == app.preferences.current.location.name
                        && location.latitude == app.preferences.current.location.latitude
                        && location.longitude == app.preferences.current.location.longitude =>
                {
                    // Set selected coordinates, location labels, and IANA timezone to selected
                    // coordinates, location labels, and IANA timezone.
                    app.preferences.current.location = location;
                    // Dispatches a settings effect to Wi-Fi, location search, or preference
                    // persistence.
                    apply(
                        Effect::Persist(app.preferences.current.clone()),
                        &mut app,
                        store.as_ref().ok(),
                        &commands,
                        &requests,
                    );
                }
                // Handle remaining cases with the fallback, preserving safe behavior for
                // unsupported or irrelevant input.
                _ => {}
            }
            // Set whether the entire visible page needs to be redrawn to the enabled state.
            dirty = true;
        }
        // Check whether `app.preferences.current.location.timezone.is_empty() &&
        // !app.preferences.read_only && milliseconds >= resolve_at &&
        // online.load(Ordering::Relaxed) && synchronized.load(Ordering::Re`.
        if app.preferences.current.location.timezone.is_empty()
            && !app.preferences.read_only
            && milliseconds >= resolve_at
            && online.load(Ordering::Relaxed)
            && synchronized.load(Ordering::Relaxed)
        {
            // Set next monotonic millisecond deadline for resolving an imported location's timezone
            // to elapsed monotonic milliseconds supplied to radar, touch, weather, and backlight
            // controllers plus 60_000.
            resolve_at = milliseconds + 60_000;
            // Update identifier used to reject stale timezone-resolution responses using 1,
            // retaining the accumulated state for subsequent steps.
            resolve_id += 1;
            // Perform the best-effort operation and intentionally ignore its result: Submit work
            // without blocking the UI; a full or disconnected queue is handled by the surrounding
            // code.
            let _ = requests.try_send(location::Request::Timezone {
                // Initialize request generation carried by a command or response from the supplied
                // value.
                id: resolve_id,
                // Initialize selected coordinates, location labels, and IANA timezone from the
                // supplied value.
                location: app.preferences.current.location.clone(),
            });
        }
        // Keep previous weather failure state used to detect a status change in this local variable
        // for the following operations.
        let was_failed = app.weather.failed;
        // Keep whether connectivity and synchronized time permit the next request in this local
        // variable for the following operations.
        let ready = online.load(Ordering::Relaxed)
            && synchronized.load(Ordering::Relaxed)
            && app.page == Page::Connected;
        // Execute observe for forecast cache, selected view, and refresh scheduler.
        app.weather.observe(milliseconds, ready);
        // Location editing takes priority over scheduled forecast work.
        // Check whether an available flatten result for the surrounding operation matches the
        // requested case.
        if let Some((id, location)) = app
            .preferences
            .overlay
            .is_none()
            .then(|| app.weather.request(milliseconds, ready))
            .flatten()
        {
            // Check whether the operation failed.
            if requests
                // Initialize request generation carried by a command or response from the supplied
                // value.
                .try_send(location::Request::Forecast { id, location })
                .is_err()
            {
                // Execute queue full for forecast cache, selected view, and refresh scheduler.
                app.weather.queue_full(milliseconds);
            }
        }
        // Check whether previous weather failure state used to detect a status change differs from
        // failed.
        if was_failed != app.weather.failed {
            // Marks weather dirty only when the connected weather page is visible.
            mark_weather_update(&app, &mut weather_dirty);
        }
        // Update whether the entire visible page needs to be redrawn using tick result for the
        // surrounding operation, retaining the accumulated state for subsequent steps.
        dirty |= app.preferences.tick(milliseconds);
        // Check whether an available tick result for the surrounding operation matches the
        // requested case.
        if let Some(command) = app.tick(now) {
            // Perform the best-effort operation and intentionally ignore its result: Deliver the
            // event or command to its worker/application channel; this send waits if required by
            // the channel.
            let _ = commands.send(command);
            // Update whether the entire visible page needs to be redrawn using overlay is
            // unavailable, retaining the accumulated state for subsequent steps.
            dirty |= app.preferences.overlay.is_none();
        }
        // Keep whether a calibration overlay protects the backlight from ordinary blanking in this
        // local variable for the following operations.
        let calibrating = matches!(
            app.preferences.overlay,
            Some(
                Overlay::CalibrationIntro | Overlay::CalibrationRunning | Overlay::CalibrationError
            )
        );
        // Check whether overlay equals an available value for Overlay CalibrationRunning.
        if app.preferences.overlay == Some(Overlay::CalibrationRunning) {
            // Keep success or failure produced by the operation, retained for later handling in
            // this local variable for the following operations.
            let result = board.poll_calibration(
                app.preferences.wizard.as_mut().expect("calibration state"),
                milliseconds,
            );
            // Choose the appropriate path for success or failure produced by the operation,
            // retained for later handling; each arm handles one supported case.
            match result {
                // Continue with the successful result, using its validated value in this case.
                Ok(WizardResult::Waiting) => {}
                // Set whether the entire visible page needs to be redrawn to the enabled state.
                Ok(WizardResult::Advanced) => dirty = true,
                // Continue with the successful result, using its validated value in this case.
                Ok(WizardResult::Verified(cal)) => {
                    // Keep whether the required operation completed without error in this local
                    // variable for the following operations.
                    let success = board.finish_calibration(Some(cal)).is_ok();
                    // Set interactive calibration state, including collected corners and a timeout
                    // to no available value.
                    app.preferences.wizard = None;
                    // Execute leave for editable preferences and their saved rollback baseline.
                    app.preferences.leave(Some(Overlay::Settings));
                    // Set status displayed to the reader instead of silent failure to the value
                    // selected by the following condition and its alternatives.
                    app.preferences.status = if success {
                        "Calibration saved."
                    } else {
                        "Calibration could not be saved."
                    };
                    // Set whether the entire visible page needs to be redrawn to the enabled state.
                    dirty = true;
                }
                // Continue with the successful result, using its validated value in this case.
                Ok(WizardResult::Invalid) => {
                    // Execute finish calibration for initialized Freenove peripherals providing
                    // display, touch, radar, and backlight access.
                    board.finish_calibration(None)?;
                    // Set interactive calibration state, including collected corners and a timeout
                    // to no available value.
                    app.preferences.wizard = None;
                    // Execute leave for editable preferences and their saved rollback baseline.
                    app.preferences.leave(Some(Overlay::CalibrationError));
                    // Set status displayed to the reader instead of silent failure to the specified
                    // message, format, or data literal.
                    app.preferences.status = "Calibration did not align. Please retry.";
                    // Set whether the entire visible page needs to be redrawn to the enabled state.
                    dirty = true;
                }
                // Continue with the successful result, using its validated value in this case.
                Ok(result) => {
                    // Execute finish calibration for initialized Freenove peripherals providing
                    // display, touch, radar, and backlight access.
                    board.finish_calibration(None)?;
                    // Set interactive calibration state, including collected corners and a timeout
                    // to no available value.
                    app.preferences.wizard = None;
                    // Execute leave for editable preferences and their saved rollback baseline.
                    app.preferences.leave(Some(Overlay::Settings));
                    // Set status displayed to the reader instead of silent failure to the value
                    // selected by the following condition and its alternatives.
                    app.preferences.status = if matches!(result, WizardResult::Cancelled) {
                        "Calibration cancelled."
                    } else {
                        "Calibration timed out."
                    };
                    // Set whether the entire visible page needs to be redrawn to the enabled state.
                    dirty = true;
                }
                // Handle a failed operation here rather than treating its value as valid.
                Err(_) => {
                    // Enters the fatal touch-error page and requests suspension of Wi-Fi work.
                    fail_touch(&mut app, now, &commands);
                    // Set whether touch I/O has entered the fatal error state to the enabled state.
                    touch_failed = true;
                    // Set whether the entire visible page needs to be redrawn to the enabled state.
                    dirty = true;
                }
            }
        } else {
            // Choose the appropriate path for touch event result for the surrounding operation;
            // each arm handles one supported case.
            match board.touch_event() {
                // Continue with the successful result, using its validated value in this case.
                Ok(Some(phase)) => {
                    // Keep whether the debounced touch phase begins a new gesture in this local
                    // variable for the following operations.
                    let down = matches!(phase, TouchPhase::Down(_));
                    // Keep whether the current touch phase ends the gesture in this local variable
                    // for the following operations.
                    let released = matches!(phase, TouchPhase::Up);
                    // Check whether the inverse of touch result for the surrounding operation.
                    if !screen.touch(
                        &app.preferences.current,
                        app.preferences.epoch,
                        milliseconds,
                        released,
                    ) {
                        // Keep brightness before processing a drag, used to detect a live-preview
                        // change in this local variable for the following operations.
                        let old_brightness = app.preferences.current.brightness;
                        // Keep settings action requiring persistence, a search, or a Wi-Fi worker
                        // command in this local variable for the following operations.
                        let effect = match phase {
                            // Handle the TouchPhase Up if app.preferences.slider active case: apply
                            // the state-specific behavior shown here.
                            TouchPhase::Up if app.preferences.slider_active => {
                                // Run the slider operation with the supplied inputs.
                                settings_ui::slider(&mut app, 0, true)
                            }
                            // Handle the TouchPhase Down(point) | TouchPhase Move(point) case:
                            // apply the state-specific behavior shown here.
                            TouchPhase::Down(point) | TouchPhase::Move(point) => {
                                // Check whether an available the transformed value or entries
                                // produced by the closure matches the requested case.
                                if let Some((x, y)) = board.map(point) {
                                    // Check whether slider active or whether the debounced touch
                                    // phase begins a new gesture and `ui::hit(&app, x, y)` equals
                                    // `Some(ui::Action::Brightness)`.
                                    if app.preferences.slider_active
                                        || (down
                                            && ui::hit(&app, x, y) == Some(ui::Action::Brightness))
                                    {
                                        // Run the slider operation with the supplied inputs.
                                        settings_ui::slider(&mut app, x, false)
                                    // Check whether whether the debounced touch phase begins a new
                                    // gesture.
                                    } else if down {
                                        // Produce the next fallible/optional stage, skipping it
                                        // when an earlier stage is unavailable.
                                        ui::hit(&app, x, y).and_then(|action| {
                                            // Run the activate operation with the supplied inputs.
                                            settings_ui::activate(&mut app, action, milliseconds)
                                        })
                                    } else {
                                        // Return no available value as the value of this block.
                                        None
                                    }
                                } else {
                                    // Return no available value as the value of this block.
                                    None
                                }
                            }
                            // Handle remaining cases with the fallback, preserving safe behavior
                            // for unsupported or irrelevant input.
                            _ => None,
                        };
                        // Keep whether the handled response or user action requires a display
                        // update in this local variable for the following operations.
                        let changed = effect.is_some();
                        // Check whether an available settings action requiring persistence, a
                        // search, or a Wi-Fi worker command matches the requested case.
                        if let Some(effect) = effect {
                            // Dispatches a settings effect to Wi-Fi, location search, or preference
                            // persistence.
                            apply(effect, &mut app, store.as_ref().ok(), &commands, &requests);
                        }
                        // Update whether the entire visible page needs to be redrawn using whether
                        // the handled response or user action requires a display update or whether
                        // the debounced touch phase begins a new gesture or brightness before
                        // processing a drag, used to detect a live-preview change differs from
                        // backlight duty in 0-255, retaining the accumulated state for subsequent
                        // steps.
                        dirty |=
                            changed || down || old_brightness != app.preferences.current.brightness;
                    }
                }
                // Continue with the successful result, using its validated value in this case.
                Ok(None) => {}
                // Handle a failed operation here rather than treating its value as valid.
                Err(_) if !touch_failed => {
                    // Enters the fatal touch-error page and requests suspension of Wi-Fi work.
                    fail_touch(&mut app, now, &commands);
                    // Set whether touch I/O has entered the fatal error state to the enabled state.
                    touch_failed = true;
                    // Set whether the entire visible page needs to be redrawn to the enabled state.
                    dirty = true;
                }
                // Handle a failed operation here rather than treating its value as valid.
                Err(_) => {}
            }
        }
        // Execute observe for backlight state machine retaining inactivity, night overrides, and
        // hardware retry deadlines.
        screen.observe(
            &app.preferences.current,
            app.preferences.epoch,
            milliseconds,
            calibrating || touch_failed || app.page == Page::Fatal,
            app.radar.present(),
            board.touch_pressed(),
        );
        // Keep committed screen state before attempting a backlight update in this local variable
        // for the following operations.
        let previous_screen = screen.state();
        // Choose the appropriate path for update result for the surrounding operation; each arm
        // handles one supported case.
        match screen.update(
            &app.preferences.current,
            app.preferences.epoch,
            milliseconds,
            calibrating || touch_failed || app.page == Page::Fatal,
            |value| board.brightness(value),
        ) {
            // Continue with the successful result, using its validated value in this case.
            Ok(result) => {
                // Check whether the optional value exists and satisfies the additional predicate.
                if result.is_some_and(|state| state != previous_screen) {
                    log::info!(
                        "Screen {:?}; background updates remain active",
                        screen.state()
                    );
                }
                // Check whether `result.is_some() && app.preferences.status == "Screen brightness
                // update failed. Retrying."`.
                if result.is_some()
                    && app.preferences.status == "Screen brightness update failed. Retrying."
                {
                    // Set status displayed to the reader instead of silent failure to the specified
                    // message, format, or data literal.
                    app.preferences.status = "";
                    // Update whether the entire visible page needs to be redrawn using overlay is
                    // available, retaining the accumulated state for subsequent steps.
                    dirty |= app.preferences.overlay.is_some();
                }
            }
            // Handle a failed operation here rather than treating its value as valid.
            Err(error) => {
                // Log this recoverable hardware failure; sensing or the interface may continue with
                // reduced functionality.
                log::warn!("Backlight update failed: {error}");
                // Set status displayed to the reader instead of silent failure to the specified
                // message, format, or data literal.
                app.preferences.status = "Screen brightness update failed. Retrying.";
                // Update whether the entire visible page needs to be redrawn using overlay is
                // available, retaining the accumulated state for subsequent steps.
                dirty |= app.preferences.overlay.is_some();
            }
        }
        // Check whether whether the entire visible page needs to be redrawn.
        if dirty {
            // Execute render for initialized Freenove peripherals providing display, touch, radar,
            // and backlight access.
            board.render(&app)?;
            // Set whether the entire visible page needs to be redrawn to the disabled state.
            dirty = false;
        } else {
            // Check whether whether current weather or the forecast table needs a differential
            // update.
            if weather_dirty {
                // Execute render weather for initialized Freenove peripherals providing display,
                // touch, radar, and backlight access.
                board.render_weather(&app)?;
            }
            // Check whether whether the minute-only settings clock needs to be repainted and
            // overlay equals an available value for Overlay Settings.
            if clock_dirty && app.preferences.overlay == Some(Overlay::Settings) {
                // Execute render settings clock for initialized Freenove peripherals providing
                // display, touch, radar, and backlight access.
                board.render_settings_clock(&app)?;
            }
            // Check whether whether the analog/digital weather clock needs a differential update
            // and currently selected Wi-Fi navigation page equals Connected state and overlay is
            // unavailable.
            if weather_clock_dirty
                && app.page == Page::Connected
                && app.preferences.overlay.is_none()
            {
                // Execute render weather clock for initialized Freenove peripherals providing
                // display, touch, radar, and backlight access.
                board.render_weather_clock(&app)?;
            }
            // Check whether whether the presence indicator changed and needs a small redraw.
            if presence_dirty {
                // Execute render presence for initialized Freenove peripherals providing display,
                // touch, radar, and backlight access.
                board.render_presence(&app)?;
            }
        }
        // Set whether the minute-only settings clock needs to be repainted to the disabled state.
        clock_dirty = false;
        // Set whether the analog/digital weather clock needs a differential update to the disabled
        // state.
        weather_clock_dirty = false;
        // Set whether current weather or the forecast table needs a differential update to the
        // disabled state.
        weather_dirty = false;
        // Set whether the presence indicator changed and needs a small redraw to the disabled
        // state.
        presence_dirty = false;
        // Pause this worker briefly rather than busy-spinning while waiting for time or touch
        // changes.
        thread::sleep(Duration::from_millis(20));
    }
}
/// Marks weather dirty only when the connected weather page is visible.
///
/// # Arguments
///
/// * `app` (`&App`) - Application model supplying the active page, preferences, weather,
///   and presence state.
/// * `weather_dirty` (`&mut bool`) - Redraw flag set when visible weather content needs an
///   update.
///
/// # Returns
///
/// `()` - No value; sets the supplied redraw flag when no settings overlay hides the
/// weather page.
fn mark_weather_update(app: &App, weather_dirty: &mut bool) {
    // Check whether currently selected Wi-Fi navigation page equals Connected state and overlay is
    // unavailable.
    if app.page == Page::Connected && app.preferences.overlay.is_none() {
        // Set whether current weather or the forecast table needs a differential update to the
        // enabled state.
        *weather_dirty = true;
    }
}

/// Dispatches a settings effect to Wi-Fi, location search, or preference persistence.
///
/// # Arguments
///
/// * `effect` (`Effect`) - Owned settings effect to execute outside the UI state handler.
/// * `app` (`&mut App`) - Application model supplying the active page, preferences,
///   weather, and presence state.
/// * `store` (`Option<&Store>`) - Available preference store, or None when opening storage
///   failed.
/// * `commands` (`&mpsc::Sender<Command>`) - Channel used to submit Wi-Fi commands to the
///   connection worker.
/// * `requests` (`&mpsc::SyncSender<location::Request>`) - Bounded channel used to submit
///   search or forecast work without blocking the UI.
///
/// # Returns
///
/// `()` - No value; updates preference save/search status for failures. Wi-Fi send errors
/// are ignored.
fn apply(
    effect: Effect,
    app: &mut App,
    store: Option<&Store>,
    commands: &mpsc::Sender<Command>,
    requests: &mpsc::SyncSender<location::Request>,
) {
    // Choose the appropriate path for settings action requiring persistence, a search, or a Wi-Fi
    // worker command; each arm handles one supported case.
    match effect {
        // Handle the Effect Wifi(command) case: apply the state-specific behavior shown here.
        Effect::Wifi(command) => {
            // Perform the best-effort operation and intentionally ignore its result: Deliver the
            // event or command to its worker/application channel; this send waits if required by
            // the channel.
            let _ = commands.send(command);
        }
        // Handle the Effect Search(request) case: apply the state-specific behavior shown here.
        Effect::Search(request) => {
            // Check whether the operation failed.
            if requests.try_send(request).is_err() {
                // Set searching to the disabled state.
                app.preferences.searching = false;
                // Set status displayed to the reader instead of silent failure to the specified
                // message, format, or data literal.
                app.preferences.status = "Location search failed. Please retry.";
            }
        }
        // Handle the Effect Persist(settings) case: apply the state-specific behavior shown here.
        Effect::Persist(settings) => {
            // Keep whether the required operation completed without error in this local variable
            // for the following operations.
            let success = !app.preferences.read_only
                && store.is_some_and(|store| store.save(&settings).is_ok());
            // Execute saved for editable preferences and their saved rollback baseline.
            app.preferences.saved(success);
        }
    }
}
/// Enters the fatal touch-error page and requests suspension of Wi-Fi work.
///
/// # Arguments
///
/// * `app` (`&mut App`) - Application model supplying the active page, preferences,
///   weather, and presence state.
/// * `now` (`u64`) - Monotonic application time in seconds; uses the retry scheduler's time
///   origin.
/// * `commands` (`&mpsc::Sender<Command>`) - Channel used to submit Wi-Fi commands to the
///   connection worker.
///
/// # Returns
///
/// `()` - No value; closes overlays, clears sensitive model state through the fatal event,
/// and attempts to send a suspension command.
fn fail_touch(app: &mut App, now: u64, commands: &mpsc::Sender<Command>) {
    // Execute leave for editable preferences and their saved rollback baseline.
    app.preferences.leave(None);
    // Execute handle for application state used by navigation, preferences, weather, and rendering.
    app.handle(
        Event::Fatal("Touch input failed. Check the display connection and restart."),
        now,
    );
    // Perform the best-effort operation and intentionally ignore its result: Deliver the event or
    // command to its worker/application channel; this send waits if required by the channel.
    let _ = commands.send(weather_forecast_firmware::model::Command::Suspend(
        app.generation,
    ));
}
