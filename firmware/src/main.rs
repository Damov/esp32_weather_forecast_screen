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

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    Board::release_sleep_holds();
    let nvs = EspDefaultNvsPartition::take_with(false)
        .map_err(|_| anyhow::anyhow!("NVS initialization failed; storage was not erased"))?;
    let store = Store::open(nvs.clone());
    let loaded = store
        .as_ref()
        .map_err(|_| "Settings could not be read.")
        .and_then(|store| store.load(nvs.clone()));
    let read_only = loaded.is_err();
    let settings = loaded.unwrap_or_else(|_| Settings::default());
    let (mut board, modem) = Board::new(
        Peripherals::take()?,
        nvs.clone(),
        settings.language,
        settings.brightness,
    )?;
    let (sender, rx) = mpsc::channel();
    let commands = sender;
    let (tx, events) = mpsc::channel();
    let online = Arc::new(AtomicBool::new(false));
    let network_online = online.clone();
    thread::Builder::new()
        .name("wifi".into())
        .stack_size(16384)
        .spawn(move || network::run(modem, nvs, rx, tx, network_online))?;
    let (requests, request_rx) = mpsc::sync_channel(1);
    let (response_tx, responses) = mpsc::channel();
    let synchronized = Arc::new(AtomicBool::new(false));
    let http_sync = synchronized.clone();
    let http_online = online.clone();
    thread::Builder::new()
        .name("location".into())
        .stack_size(16384)
        .spawn(move || location::run(request_rx, response_tx, http_online, http_sync))?;
    let mut app = App {
        preferences: Preferences::new(settings, read_only),
        ..App::default()
    };
    let started = Instant::now();
    let mut screen = ScreenController::new(&app.preferences.current, 0);
    let mut dirty = true;
    let mut clock_dirty = false;
    let mut weather_clock_dirty = false;
    let mut weather_dirty = false;
    let mut presence_dirty = false;
    let mut touch_failed = false;
    let mut last_second = u64::MAX;
    let mut resolve_at = 0;
    let mut resolve_id = 0;
    loop {
        let milliseconds = started.elapsed().as_millis() as u64;
        let now = milliseconds / 1000;
        let presence = board.poll_radar(milliseconds);
        presence_dirty |= presence != app.radar;
        app.radar = presence;
        if now != last_second {
            last_second = now;
            let epoch = if synchronized.load(Ordering::Relaxed) {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .ok()
                    .map(|time| time.as_secs() as i64)
            } else {
                None
            };
            let previous = app
                .weather
                .range_key(&app.preferences.current, app.preferences.epoch);
            let current = app.weather.range_key(&app.preferences.current, epoch);
            let weather_visible = app.page == Page::Connected && app.preferences.overlay.is_none();
            if weather_visible && previous != current {
                mark_weather_update(&app, &mut weather_dirty);
            }
            weather_clock_dirty |= weather_visible
                && app.weather.view == weather_forecast_firmware::weather::View::Clock
                && weather_forecast_firmware::weather::local(&app.preferences.current, epoch)
                    != weather_forecast_firmware::weather::local(
                        &app.preferences.current,
                        app.preferences.epoch,
                    );
            clock_dirty |= app.preferences.update_time(epoch);
        }
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
        for event in events.try_iter() {
            let overlay = app.preferences.overlay;
            let status = app.preferences.status;
            if let Some(command) = app.handle(event, now) {
                let _ = commands.send(command);
            }
            dirty |= overlay.is_none()
                || overlay != app.preferences.overlay
                || status != app.preferences.status;
        }
        let location_changed = app.weather.adopt(&app.preferences.current.location);
        dirty |=
            location_changed && app.page == Page::Connected && app.preferences.overlay.is_none();
        for response in responses.try_iter() {
            match response {
                location::Response::Forecast(id, location, result) => {
                    let changed = app.weather.receive(
                        id,
                        &location,
                        result,
                        milliseconds,
                        app.preferences.epoch,
                    );
                    if changed {
                        mark_weather_update(&app, &mut weather_dirty);
                    }
                    continue;
                }
                location::Response::Locations(id, result) => app.preferences.locations(id, result),
                location::Response::Timezone(id, Ok(location))
                    if id == resolve_id
                        && app.preferences.current.location.timezone.is_empty()
                        && location.name == app.preferences.current.location.name
                        && location.latitude == app.preferences.current.location.latitude
                        && location.longitude == app.preferences.current.location.longitude =>
                {
                    app.preferences.current.location = location;
                    apply(
                        Effect::Persist(app.preferences.current.clone()),
                        &mut app,
                        store.as_ref().ok(),
                        &commands,
                        &requests,
                    );
                }
                _ => {}
            }
            dirty = true;
        }
        if app.preferences.current.location.timezone.is_empty()
            && !app.preferences.read_only
            && milliseconds >= resolve_at
            && online.load(Ordering::Relaxed)
            && synchronized.load(Ordering::Relaxed)
        {
            resolve_at = milliseconds + 60_000;
            resolve_id += 1;
            let _ = requests.try_send(location::Request::Timezone {
                id: resolve_id,
                location: app.preferences.current.location.clone(),
            });
        }
        let was_failed = app.weather.failed;
        let ready = online.load(Ordering::Relaxed)
            && synchronized.load(Ordering::Relaxed)
            && app.page == Page::Connected;
        app.weather.observe(milliseconds, ready);
        // Location editing takes priority over scheduled forecast work.
        if let Some((id, location)) = app
            .preferences
            .overlay
            .is_none()
            .then(|| app.weather.request(milliseconds, ready))
            .flatten()
        {
            if requests
                .try_send(location::Request::Forecast { id, location })
                .is_err()
            {
                app.weather.queue_full(milliseconds);
            }
        }
        if was_failed != app.weather.failed {
            mark_weather_update(&app, &mut weather_dirty);
        }
        dirty |= app.preferences.tick(milliseconds);
        if let Some(command) = app.tick(now) {
            let _ = commands.send(command);
            dirty |= app.preferences.overlay.is_none();
        }
        let calibrating = matches!(
            app.preferences.overlay,
            Some(
                Overlay::CalibrationIntro | Overlay::CalibrationRunning | Overlay::CalibrationError
            )
        );
        if app.preferences.overlay == Some(Overlay::CalibrationRunning) {
            let result = board.poll_calibration(
                app.preferences.wizard.as_mut().expect("calibration state"),
                milliseconds,
            );
            match result {
                Ok(WizardResult::Waiting) => {}
                Ok(WizardResult::Advanced) => dirty = true,
                Ok(WizardResult::Verified(cal)) => {
                    let success = board.finish_calibration(Some(cal)).is_ok();
                    app.preferences.wizard = None;
                    app.preferences.leave(Some(Overlay::Settings));
                    app.preferences.status = if success {
                        "Calibration saved."
                    } else {
                        "Calibration could not be saved."
                    };
                    dirty = true;
                }
                Ok(WizardResult::Invalid) => {
                    board.finish_calibration(None)?;
                    app.preferences.wizard = None;
                    app.preferences.leave(Some(Overlay::CalibrationError));
                    app.preferences.status = "Calibration did not align. Please retry.";
                    dirty = true;
                }
                Ok(result) => {
                    board.finish_calibration(None)?;
                    app.preferences.wizard = None;
                    app.preferences.leave(Some(Overlay::Settings));
                    app.preferences.status = if matches!(result, WizardResult::Cancelled) {
                        "Calibration cancelled."
                    } else {
                        "Calibration timed out."
                    };
                    dirty = true;
                }
                Err(_) => {
                    fail_touch(&mut app, now, &commands);
                    touch_failed = true;
                    dirty = true;
                }
            }
        } else {
            match board.touch_event() {
                Ok(Some(phase)) => {
                    let down = matches!(phase, TouchPhase::Down(_));
                    let released = matches!(phase, TouchPhase::Up);
                    if !screen.touch(
                        &app.preferences.current,
                        app.preferences.epoch,
                        milliseconds,
                        released,
                    ) {
                        let old_brightness = app.preferences.current.brightness;
                        let effect = match phase {
                            TouchPhase::Up if app.preferences.slider_active => {
                                settings_ui::slider(&mut app, 0, true)
                            }
                            TouchPhase::Down(point) | TouchPhase::Move(point) => {
                                if let Some((x, y)) = board.map(point) {
                                    if app.preferences.slider_active
                                        || (down
                                            && ui::hit(&app, x, y) == Some(ui::Action::Brightness))
                                    {
                                        settings_ui::slider(&mut app, x, false)
                                    } else if down {
                                        ui::hit(&app, x, y).and_then(|action| {
                                            settings_ui::activate(&mut app, action, milliseconds)
                                        })
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        };
                        let changed = effect.is_some();
                        if let Some(effect) = effect {
                            apply(effect, &mut app, store.as_ref().ok(), &commands, &requests);
                        }
                        dirty |=
                            changed || down || old_brightness != app.preferences.current.brightness;
                    }
                }
                Ok(None) => {}
                Err(_) if !touch_failed => {
                    fail_touch(&mut app, now, &commands);
                    touch_failed = true;
                    dirty = true;
                }
                Err(_) => {}
            }
        }
        screen.observe(
            &app.preferences.current,
            app.preferences.epoch,
            milliseconds,
            calibrating || touch_failed || app.page == Page::Fatal,
            app.radar.present(),
            board.touch_pressed(),
        );
        let previous_screen = screen.state();
        match screen.update(
            &app.preferences.current,
            app.preferences.epoch,
            milliseconds,
            calibrating || touch_failed || app.page == Page::Fatal,
            |value| board.brightness(value),
        ) {
            Ok(result) => {
                if result.is_some_and(|state| state != previous_screen) {
                    log::info!(
                        "Screen {:?}; background updates remain active",
                        screen.state()
                    );
                }
                if result.is_some()
                    && app.preferences.status == "Screen brightness update failed. Retrying."
                {
                    app.preferences.status = "";
                    dirty |= app.preferences.overlay.is_some();
                }
            }
            Err(error) => {
                log::warn!("Backlight update failed: {error}");
                app.preferences.status = "Screen brightness update failed. Retrying.";
                dirty |= app.preferences.overlay.is_some();
            }
        }
        if dirty {
            board.render(&app)?;
            dirty = false;
        } else {
            if weather_dirty {
                board.render_weather(&app)?;
            }
            if clock_dirty && app.preferences.overlay == Some(Overlay::Settings) {
                board.render_settings_clock(&app)?;
            }
            if weather_clock_dirty
                && app.page == Page::Connected
                && app.preferences.overlay.is_none()
            {
                board.render_weather_clock(&app)?;
            }
            if presence_dirty {
                board.render_presence(&app)?;
            }
        }
        clock_dirty = false;
        weather_clock_dirty = false;
        weather_dirty = false;
        presence_dirty = false;
        thread::sleep(Duration::from_millis(20));
    }
}
fn mark_weather_update(app: &App, weather_dirty: &mut bool) {
    if app.page == Page::Connected && app.preferences.overlay.is_none() {
        *weather_dirty = true;
    }
}

fn apply(
    effect: Effect,
    app: &mut App,
    store: Option<&Store>,
    commands: &mpsc::Sender<Command>,
    requests: &mpsc::SyncSender<location::Request>,
) {
    match effect {
        Effect::Wifi(command) => {
            let _ = commands.send(command);
        }
        Effect::Search(request) => {
            if requests.try_send(request).is_err() {
                app.preferences.searching = false;
                app.preferences.status = "Location search failed. Please retry.";
            }
        }
        Effect::Persist(settings) => {
            let success = !app.preferences.read_only
                && store.is_some_and(|store| store.save(&settings).is_ok());
            app.preferences.saved(success);
        }
    }
}
fn fail_touch(app: &mut App, now: u64, commands: &mpsc::Sender<Command>) {
    app.preferences.leave(None);
    app.handle(
        Event::Fatal("Touch input failed. Check the display connection and restart."),
        now,
    );
    let _ = commands.send(weather_forecast_firmware::model::Command::Suspend(
        app.generation,
    ));
}
