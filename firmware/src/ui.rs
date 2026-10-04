// ============================================================================= //
// File          : ui.rs                                                         //
// License       : MIT                                                           //
// Created       : 2026-10-04                                                    //
// Last modified : 2026-10-04                                                    //
// Author        : Daniel Kharlamov                                              //
// Last editor   : Daniel Kharlamov <daniel.kharlamov@googlemail.com>            //
// ============================================================================= //
//                                                                               //
// Summary:                                                                      //
// --------                                                                      //
// Wi-Fi interface controls and shared application rendering.                    //
//                                                                               //
// Module:                                                                       //
// -------                                                                       //
// Builds hit-testable controls and maps user actions to application commands.   //
// Renders connection pages, password keyboards, startup and calibration         //
// prompts, and presence indicators. Delegates settings overlays and weather     //
// views to their renderers while preserving shared UI geometry.                 //
// ============================================================================= //

//! Wi-Fi interface controls and shared application rendering.

use crate::{
    model::{App, Command, Page},
    theme::{self, Backdrop, BODY, SMALL, TITLE},
};
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle, Rectangle},
};
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    // Represent cycleweather as a distinct selectable state or action; the matching handler
    // determines its effect.
    CycleWeather,
    // Represent opensettings as a distinct selectable state or action; the matching handler
    // determines its effect.
    OpenSettings,
    // Represent closesettings as a distinct selectable state or action; the matching handler
    // determines its effect.
    CloseSettings,
    // Represent settingsback as a distinct selectable state or action; the matching handler
    // determines its effect.
    SettingsBack,
    // Represent brightness as a distinct selectable state or action; the matching handler
    // determines its effect.
    Brightness,
    // Represent night as a distinct selectable state or action; the matching handler determines its
    // effect.
    Night,
    // Represent wakeonpresence as a distinct selectable state or action; the matching handler
    // determines its effect.
    WakeOnPresence,
    // Represent sleeptimeout as a distinct selectable state or action; the matching handler
    // determines its effect.
    SleepTimeout,
    // Represent selectsleeptimeout as a distinct selectable state or action; the matching handler
    // determines its effect.
    SelectSleepTimeout(u32),
    // Represent units as a distinct selectable state or action; the matching handler determines its
    // effect.
    Units,
    // Current weather alongside analog and digital local clocks.
    Clock,
    // Represent languages as a distinct selectable state or action; the matching handler determines
    // its effect.
    Languages,
    // Represent language as a distinct selectable state or action; the matching handler determines
    // its effect.
    Language(crate::i18n::Language),
    // Represent location as a distinct selectable state or action; the matching handler determines
    // its effect.
    Location,
    // Represent locationback as a distinct selectable state or action; the matching handler
    // determines its effect.
    LocationBack,
    // Represent citycharacter as a distinct selectable state or action; the matching handler
    // determines its effect.
    CityCharacter(char),
    // Represent cityshift as a distinct selectable state or action; the matching handler determines
    // its effect.
    CityShift,
    // Represent citysymbols as a distinct selectable state or action; the matching handler
    // determines its effect.
    CitySymbols,
    // Represent cityaccents as a distinct selectable state or action; the matching handler
    // determines its effect.
    CityAccents,
    // Represent cityalphabet as a distinct selectable state or action; the matching handler
    // determines its effect.
    CityAlphabet,
    // Represent cityspace as a distinct selectable state or action; the matching handler determines
    // its effect.
    CitySpace,
    // Represent citydelete as a distinct selectable state or action; the matching handler
    // determines its effect.
    CityDelete,
    // Represent searchcity as a distinct selectable state or action; the matching handler
    // determines its effect.
    SearchCity,
    // Represent selectlocation as a distinct selectable state or action; the matching handler
    // determines its effect.
    SelectLocation(usize),
    // Represent locationsprevious as a distinct selectable state or action; the matching handler
    // determines its effect.
    LocationsPrevious,
    // Represent locationsnext as a distinct selectable state or action; the matching handler
    // determines its effect.
    LocationsNext,
    // Represent savelocation as a distinct selectable state or action; the matching handler
    // determines its effect.
    SaveLocation,
    // Represent resetprompt as a distinct selectable state or action; the matching handler
    // determines its effect.
    ResetPrompt,
    // Represent resetconfirm as a distinct selectable state or action; the matching handler
    // determines its effect.
    ResetConfirm,
    // Represent calibration as a distinct selectable state or action; the matching handler
    // determines its effect.
    Calibration,
    // Represent startcalibration as a distinct selectable state or action; the matching handler
    // determines its effect.
    StartCalibration,
    // Represent setup as a distinct selectable state or action; the matching handler determines its
    // effect.
    Setup,
    // Retry the retained saved profile rather than persisting a new candidate.
    Retry,
    // Represent refresh as a distinct selectable state or action; the matching handler determines
    // its effect.
    Refresh,
    // Represent back as a distinct selectable state or action; the matching handler determines its
    // effect.
    Back,
    // Represent select as a distinct selectable state or action; the matching handler determines
    // its effect.
    Select(usize),
    // Represent previous as a distinct selectable state or action; the matching handler determines
    // its effect.
    Previous,
    // Represent next as a distinct selectable state or action; the matching handler determines its
    // effect.
    Next,
    // Connect using candidate credentials and carry the generation needed to reject stale events.
    Connect,
    // Represent show as a distinct selectable state or action; the matching handler determines its
    // effect.
    Show,
    // Represent character as a distinct selectable state or action; the matching handler determines
    // its effect.
    Character(char),
    // Represent shift as a distinct selectable state or action; the matching handler determines its
    // effect.
    Shift,
    // Represent symbols as a distinct selectable state or action; the matching handler determines
    // its effect.
    Symbols,
    // Represent delete as a distinct selectable state or action; the matching handler determines
    // its effect.
    Delete,
    // Represent space as a distinct selectable state or action; the matching handler determines its
    // effect.
    Space,
}
pub struct Control {
    // Screen-coordinate rectangle limiting the drawing operation. Stored as Rectangle.
    pub bounds: Rectangle,
    // Text identifying the control or forecast entry. Stored as String.
    pub label: String,
    // Action. Stored as Action.
    pub action: Action,
    // Enabled. Stored as bool.
    pub enabled: bool,
}
/// Creates a rectangular UI control with its label, action, and enabled state.
///
/// # Arguments
///
/// * `x` (`i32`) - Horizontal screen coordinate in pixels.
/// * `y` (`i32`) - Vertical screen coordinate in pixels.
/// * `w` (`u32`) - Control width in pixels.
/// * `h` (`u32`) - Control height in pixels.
/// * `label` (`impl Into<String>`) - Text converted into the control's owned label.
/// * `action` (`Action`) - Action selected by the user or hit tester.
/// * `enabled` (`bool`) - Whether the control accepts user input.
///
/// # Returns
///
/// `Control` - Control with the supplied pixel bounds and an owned label.
fn control(
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    label: impl Into<String>,
    action: Action,
    enabled: bool,
) -> Control {
    Control {
        // Initialize screen-coordinate rectangle limiting the drawing operation from the supplied
        // value.
        bounds: Rectangle::new(Point::new(x, y), Size::new(w, h)),
        // Initialize text identifying the control or forecast entry from the supplied value.
        label: label.into(),
        // Initialize action from the supplied value.
        action,
        // Initialize enabled from the supplied value.
        enabled,
    }
}
/// Builds controls for the current application page or settings overlay.
///
/// # Arguments
///
/// * `app` (`&App`) - Application model supplying the active page, preferences, weather,
///   and presence state.
///
/// # Returns
///
/// `Vec<Control>` - Visible controls with shared rendering and hit-test geometry; may be
/// empty.
pub fn controls(app: &App) -> Vec<Control> {
    // An open settings overlay owns the visible controls, so delegate instead of exposing
    // underlying Wi-Fi actions.
    if app.preferences.overlay.is_some() {
        // Leave this function now with builds controls for the current application page or settings
        // overlay; later statements are skipped.
        return crate::settings_ui::controls(app);
    }
    // Keep controls collected for the current Wi-Fi page or settings overlay in this local variable
    // for the following operations.
    let mut out = vec![];
    // Choose the appropriate path for currently selected Wi-Fi navigation page; each arm handles
    // one supported case.
    match app.page {
        // Handle the Page Problem case: apply the state-specific behavior shown here.
        Page::Problem => {
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(control(
                12,
                300,
                296,
                52,
                "Set up new Wi-Fi",
                Action::Setup,
                true,
            ));
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(control(
                12,
                364,
                296,
                40,
                // A retained profile allows reconnecting without asking for new credentials.
                if app.saved_ssid.is_some() {
                    "Retry"
                } else {
                    "Scan again"
                },
                Action::Retry,
                true,
            ));
        }
        // Handle the Page Connected case: apply the state-specific behavior shown here.
        Page::Connected => {
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(control(
                260,
                427,
                53,
                53,
                "Settings",
                Action::OpenSettings,
                true,
            ));
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(control(0, 40, 320, 387, "", Action::CycleWeather, true));
        }
        // Handle the Page Networks case: apply the state-specific behavior shown here.
        Page::Networks => {
            // Visit each entry in enumerate result for the surrounding operation; the loop binding
            // provides its value or index for this iteration.
            for (row, n) in app.networks.iter().skip(app.offset).take(5).enumerate() {
                // Append the new entry to the collection, preserving the order in which values
                // arrive.
                out.push(control(
                    12,
                    112 + row as i32 * 54,
                    296,
                    48,
                    format!(
                        "{}  {}",
                        clipped(
                            &n.ssid,
                            276 - BODY.width(&format!("  {}", n.security.label()))
                        ),
                        n.security.label()
                    ),
                    Action::Select(app.offset + row),
                    !app.scanning,
                ));
            }
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(control(
                12,
                388,
                140,
                40,
                "Previous",
                Action::Previous,
                app.offset > 0 && !app.scanning,
            ));
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(control(
                168,
                388,
                140,
                40,
                "Next",
                Action::Next,
                app.offset + 5 < app.networks.len() && !app.scanning,
            ));
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(control(12, 438, 140, 40, "Back", Action::Back, true));
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(control(
                168,
                438,
                140,
                40,
                "Refresh",
                Action::Refresh,
                !app.scanning,
            ));
        }
        // Handle the Page Password case: apply the state-specific behavior shown here.
        Page::Password => {
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(control(
                12,
                130,
                296,
                40,
                // Display only the fitting suffix in zeroizing temporary storage; otherwise render
                // masking stars.
                if app.show_password {
                    "Hide password"
                } else {
                    "Show password"
                },
                Action::Show,
                true,
            ));
            // Keep four strings defining the selected keyboard's rows of printable characters in
            // this local variable for the following operations.
            let rows: [&str; 4] = match app.keyboard {
                // Handle the 1 case: apply the state-specific behavior shown here.
                1 => ["QWERTYUIOP", "ASDFGHJKL", "ZXCVBNM", "0123456789"],
                // Handle the 2 case: apply the state-specific behavior shown here.
                2 => ["!@#$%^&*()", "-_=+[]{}", ";:'\"\\|,.", "<>/?`~"],
                // Handle remaining cases with the fallback, preserving safe behavior for
                // unsupported or irrelevant input.
                _ => ["qwertyuiop", "asdfghjkl", "zxcvbnm", "0123456789"],
            };
            // Visit each entry in enumerate result for the surrounding operation; the loop binding
            // provides its value or index for this iteration.
            for (row, chars) in rows.iter().enumerate() {
                // Keep pixel or UTF-8 starting position chosen to center keys or fit the visible
                // password suffix in this local variable for the following operations.
                let start = (320 - chars.len() as i32 * 32) / 2;
                // Visit each entry in enumerate result for the surrounding operation; the loop
                // binding provides its value or index for this iteration.
                for (col, ch) in chars.chars().enumerate() {
                    // Append the new entry to the collection, preserving the order in which values
                    // arrive.
                    out.push(control(
                        start + col as i32 * 32 + 1,
                        234 + row as i32 * 40,
                        30,
                        38,
                        ch.to_string(),
                        Action::Character(ch),
                        true,
                    ));
                }
            }
            // Run each listed scenario or target in the declared order; the loop variables select
            // the data for that case.
            for (i, label, action) in [
                (0, "Shift", Action::Shift),
                (1, "#+=", Action::Symbols),
                (2, "Space", Action::Space),
                (3, "Del", Action::Delete),
            ] {
                // Append the new entry to the collection, preserving the order in which values
                // arrive.
                out.push(control(i * 80 + 1, 396, 78, 38, label, action, true));
            }
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(control(12, 438, 140, 40, "Back", Action::Back, true));
            // Append the new entry to the collection, preserving the order in which values arrive.
            out.push(control(
                168,
                438,
                140,
                40,
                "Connect",
                Action::Connect,
                app.password.len() >= 8,
            ));
        }
        // Handle the Page Connecting case: Append the new entry to the collection, preserving the
        // order in which values arrive.
        Page::Connecting => out.push(control(12, 390, 296, 52, "Cancel", Action::Back, true)),
        // Handle remaining cases with the fallback, preserving safe behavior for unsupported or
        // irrelevant input.
        _ => {}
    }
    // Check whether currently selected Wi-Fi navigation page equals Problem state.
    if app.page == Page::Problem {
        // Append the new entry to the collection, preserving the order in which values arrive.
        out.push(control(
            12,
            422,
            296,
            40,
            "Settings",
            Action::OpenSettings,
            true,
        ));
    }
    // Visit each entry in controls collected for the current Wi-Fi page or settings overlay; the
    // loop binding provides its value or index for this iteration.
    for button in &mut out {
        // Check whether the inverse of the formatted text or fixture created by matches.
        if !matches!(button.action, Action::Character(_)) {
            // Set text identifying the control or forecast entry to an owned text value rather than
            // a borrowed view.
            button.label =
                crate::i18n::tr(app.preferences.current.language, &button.label).to_owned();
        }
    }
    // Return controls collected for the current Wi-Fi page or settings overlay as the value of this
    // block.
    out
}
/// Finds the first enabled control containing a screen point.
///
/// # Arguments
///
/// * `app` (`&App`) - Application model supplying the active page, preferences, weather,
///   and presence state.
/// * `x` (`i32`) - Horizontal screen coordinate in pixels.
/// * `y` (`i32`) - Vertical screen coordinate in pixels.
///
/// # Returns
///
/// `Option<Action>` - Some action for an enabled hit; None outside controls or over
/// disabled controls.
pub fn hit(app: &App, x: i32, y: i32) -> Option<Action> {
    // Produce the transformed value or entries produced by the closure.
    controls(app)
        .into_iter()
        .find(|b| b.enabled && b.bounds.contains(Point::new(x, y)))
        .map(|b| b.action)
}
/// Applies a Wi-Fi or shared navigation action to the application model.
///
/// # Arguments
///
/// * `app` (`&mut App`) - Application model supplying the active page, preferences,
///   weather, and presence state.
/// * `action` (`Action`) - Action selected by the user or hit tester.
///
/// # Returns
///
/// `Option<Command>` - Some worker command for network operations; None for local changes,
/// unsupported actions, or failed credential validation.
pub fn activate(app: &mut App, action: Action) -> Option<Command> {
    // Choose the appropriate path for action; each arm handles one supported case.
    match action {
        // Handle the Action CycleWeather case: apply the state-specific behavior shown here.
        Action::CycleWeather => {
            // Set view to next result for the surrounding operation.
            app.weather.view = app.weather.view.next();
            // Return no available value as the value of this block.
            None
        }
        // Handle the Action Setup | Action Refresh case: Return an available setup result for the
        // surrounding operation.
        Action::Setup | Action::Refresh => Some(app.setup()),
        // Handle the Action Retry case: apply the state-specific behavior shown here.
        Action::Retry => {
            // A retained profile allows reconnecting without asking for new credentials.
            if app.saved_ssid.is_some() {
                // Return an available retry result for the surrounding operation.
                Some(app.retry())
            } else {
                // Set whether an asynchronous access-point scan is pending to the enabled state.
                app.scanning = true;
                // Update request identifier used to reject results belonging to cancelled or
                // replaced work using 1, retaining the accumulated state for subsequent steps.
                app.generation += 1;
                // Return an available Scan result for the surrounding operation.
                Some(Command::Scan(app.generation))
            }
        }
        // Handle the Action Back case: Return an available back result for the surrounding
        // operation.
        Action::Back => Some(app.back()),
        // Handle the Action Connect case: Execute connect for application state used by navigation,
        // preferences, weather, and rendering.
        Action::Connect => app.connect(),
        // Handle the Action Select(index) case: apply the state-specific behavior shown here.
        Action::Select(index) => {
            // Execute select for application state used by navigation, preferences, weather, and
            // rendering.
            app.select(index);
            // Return no available value as the value of this block.
            None
        }
        // Handle the Action Previous case: apply the state-specific behavior shown here.
        Action::Previous => {
            // Set current starting position within a paginated list to elapsed difference clamped
            // at zero instead of unsigned underflow.
            app.offset = app.offset.saturating_sub(5);
            // Return no available value as the value of this block.
            None
        }
        // Handle the Action Next case: apply the state-specific behavior shown here.
        Action::Next => {
            // Another five-entry access-point page exists, so advancing pagination is meaningful.
            if app.offset + 5 < app.networks.len() {
                // Update current starting position within a paginated list using 5, retaining the
                // accumulated state for subsequent steps.
                app.offset += 5;
            }
            // Return no available value as the value of this block.
            None
        }
        // Handle the Action Show case: apply the state-specific behavior shown here.
        Action::Show => {
            // Set whether entered password characters are visible instead of masked to the inverse
            // of whether entered password characters are visible instead of masked.
            app.show_password = !app.show_password;
            // Return no available value as the value of this block.
            None
        }
        // Handle the Action Character(ch) case: apply the state-specific behavior shown here.
        Action::Character(ch) => {
            // Execute type char for application state used by navigation, preferences, weather, and
            // rendering.
            app.type_char(ch);
            // Return no available value as the value of this block.
            None
        }
        // Handle the Action Space case: apply the state-specific behavior shown here.
        Action::Space => {
            // Execute type char for application state used by navigation, preferences, weather, and
            // rendering.
            app.type_char(' ');
            // Return no available value as the value of this block.
            None
        }
        // Handle the Action Delete case: apply the state-specific behavior shown here.
        Action::Delete => {
            // Remove the final character so the delete action operates on a complete Unicode
            // character.
            app.password.pop();
            // Return no available value as the value of this block.
            None
        }
        // Handle the Action Shift case: apply the state-specific behavior shown here.
        Action::Shift => {
            // Set selected on-screen keyboard layout to the value selected by the following
            // condition and its alternatives.
            app.keyboard = if app.keyboard == 1 { 0 } else { 1 };
            // Return no available value as the value of this block.
            None
        }
        // Handle the Action Symbols case: apply the state-specific behavior shown here.
        Action::Symbols => {
            // Set selected on-screen keyboard layout to the value selected by the following
            // condition and its alternatives.
            app.keyboard = if app.keyboard == 2 { 0 } else { 2 };
            // Return no available value as the value of this block.
            None
        }
        // Handle remaining cases with the fallback, preserving safe behavior for unsupported or
        // irrelevant input.
        _ => None,
    }
}
/// Clips text using the shared body font and sanitizes control characters.
///
/// # Arguments
///
/// * `text` (`&str`) - Text to translate, measure, clip, or draw.
/// * `width` (`u32`) - Available drawing or text width in pixels.
///
/// # Returns
///
/// `String` - Owned body-font text fitting the supplied pixel width.
fn clipped(text: &str, width: u32) -> String {
    // Clips text using the shared body font and sanitizes control characters.
    theme::clipped(text, &BODY, width)
}
/// Draws body-font text across bounded lines on the gradient background.
///
/// # Arguments
///
/// * `display` (`&mut D`) - Mutable RGB565 draw target receiving the rendering operations.
/// * `text` (`&str`) - Text to translate, measure, clip, or draw.
/// * `y` (`i32`) - Vertical screen coordinate in pixels.
/// * `max_lines` (`usize`) - Maximum number of output or drawn lines; zero permits no
///   lines.
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
/// Propagates text drawing errors.
fn wrapped<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    text: &str,
    y: i32,
    max_lines: usize,
) -> Result<(), D::Error> {
    // Visit each entry in enumerate result for the surrounding operation; the loop binding provides
    // its value or index for this iteration.
    for (i, line) in theme::wrap(text, &BODY, 288, max_lines).iter().enumerate() {
        // Send the prepared shape or text pixels to the current drawing destination.
        BODY.draw(
            display,
            line,
            Point::new(16, y + i as i32 * BODY.height as i32),
            theme::TEXT,
            Backdrop::Gradient,
            288,
        )?;
    }
    // Return success after the required side effects are complete.
    Ok(())
}
/// Renders the active Wi-Fi page, settings overlay, or connected weather view.
///
/// # Arguments
///
/// * `display` (`&mut D`) - Mutable RGB565 draw target receiving the rendering operations.
/// * `app` (`&App`) - Application model supplying the active page, preferences, weather,
///   and presence state.
///
/// # Type Parameters
///
/// * `D` - RGB565 drawing destination; its associated error is propagated by fallible
///   renderers.
///
/// # Returns
///
/// `Result<(), D::Error>` - Ok after rendering; Err from the draw target.
///
/// # Errors
///
/// Propagates drawing failures from the active renderer.
pub fn render<D: DrawTarget<Color = Rgb565>>(display: &mut D, app: &App) -> Result<(), D::Error> {
    // An open settings overlay owns the visible controls, so delegate instead of exposing
    // underlying Wi-Fi actions.
    if app.preferences.overlay.is_some() {
        // Leave this function now with renders the active Wi-Fi page, settings overlay, or
        // connected weather view; later statements are skipped.
        return crate::settings_ui::render(display, app);
    }
    // A connected station shows the selected weather view rather than another Wi-Fi setup page.
    if app.page == Page::Connected {
        // Leave this function now with renders the active Wi-Fi page, settings overlay, or
        // connected weather view; later statements are skipped.
        return crate::weather_ui::render(display, app);
    }
    // Keep selected language used to translate this page's labels in this local variable for the
    // following operations.
    let lang = app.preferences.current.language;
    // Run the background operation with the supplied inputs.
    theme::background(display)?;
    // Keep localized page heading selected from the active Wi-Fi navigation state in this local
    // variable for the following operations.
    let title = match app.page {
        // Handle the Page Networks case: apply the state-specific behavior shown here.
        Page::Networks => "Choose Wi-Fi",
        // Handle the Page Password case: apply the state-specific behavior shown here.
        Page::Password => "Wi-Fi password",
        // Handle the Page Connected case: apply the state-specific behavior shown here.
        Page::Connected => "Connected",
        // Handle the Page Fatal case: apply the state-specific behavior shown here.
        Page::Fatal => "Setup required",
        // Handle remaining cases with the fallback, preserving safe behavior for unsupported or
        // irrelevant input.
        _ => "Wi-Fi setup",
    };
    // Send the prepared shape or text pixels to the current drawing destination.
    TITLE.draw(
        display,
        crate::i18n::tr(lang, title),
        Point::new(16, 12),
        theme::TEXT,
        Backdrop::Gradient,
        // Check whether checks whether the active page includes a presence indicator.
        if presence_visible(app) { 248 } else { 288 },
    )?;
    // Restores the indicator background and draws the current presence colour when visible.
    render_presence(display, app)?;
    // Choose the appropriate path for currently selected Wi-Fi navigation page; each arm handles
    // one supported case.
    match app.page {
        // Handle the Page Password case: apply the state-specific behavior shown here.
        Page::Password => {
            // Check whether an available access point currently selected for candidate setup
            // matches the requested case.
            if let Some(n) = &app.selected {
                // Draws body-font text across bounded lines on the gradient background.
                wrapped(display, &n.ssid, 44, 2)?;
            }
            // Run the rounded operation with the supplied inputs.
            theme::rounded(
                display,
                Rectangle::new(Point::new(12, 86), Size::new(296, 34)),
                theme::PANEL,
            )?;
            // Measure borrowed suffixes; never copy an unmasked password into an ordinary String.
            // Keep temporary zeroizing display text containing either password characters or
            // masking stars in this local variable for the following operations.
            let field = Zeroizing::new(if app.show_password {
                // Keep pixel or UTF-8 starting position chosen to center keys or fit the visible
                // password suffix in this local variable for the following operations.
                let start = app
                    .password
                    .char_indices()
                    .map(|(i, _)| i)
                    .find(|&i| BODY.width(&app.password[i..]) <= 280)
                    .unwrap_or(app.password.len());
                // Produce an owned text value rather than a borrowed view.
                app.password[start..].to_owned()
            } else {
                // Keep number of masking stars that fit inside the password field in this local
                // variable for the following operations.
                let count = (0..=app.password.len())
                    .rev()
                    .find(|&n| BODY.width(&"*".repeat(n)) <= 280)
                    .unwrap_or(0);
                // Execute repeat for the specified message, format, or data literal.
                "*".repeat(count)
            });
            // Send the prepared shape or text pixels to the current drawing destination.
            BODY.draw(
                display,
                &field,
                Point::new(20, 94),
                theme::TEXT,
                Backdrop::Solid(theme::PANEL),
                280,
            )?;
            // Draws body-font text across bounded lines on the gradient background.
            wrapped(display, crate::i18n::tr(lang, &app.message), 180, 3)?;
        }
        // Handle the Page Networks case: apply the state-specific behavior shown here.
        Page::Networks => wrapped(display, crate::i18n::tr(lang, &app.message), 46, 3)?,
        // Handle the Page Problem | Page Connecting | Page Connected case: apply the state-specific
        // behavior shown here.
        Page::Problem | Page::Connecting | Page::Connected => {
            // Check whether an available or result for the surrounding operation matches the
            // requested case.
            if let Some(ssid) = app
                .selected
                .as_ref()
                .map(|n| &n.ssid)
                .or(app.saved_ssid.as_ref())
            {
                // Draws body-font text across bounded lines on the gradient background.
                wrapped(
                    display,
                    &format!("{}: {ssid}", crate::i18n::tr(lang, "Network")),
                    64,
                    3,
                )?;
            }
            // Draws body-font text across bounded lines on the gradient background.
            wrapped(display, crate::i18n::tr(lang, &app.message), 154, 6)?;
            // The access-point list is still being refreshed; show progress rather than implying
            // the current list is final.
            if app.scanning {
                // Send the prepared shape or text pixels to the current drawing destination.
                SMALL.draw(
                    display,
                    crate::i18n::tr(lang, "Scanning..."),
                    Point::new(16, 280),
                    theme::SECONDARY,
                    Backdrop::Gradient,
                    288,
                )?;
            }
        }
        // Handle the Page Fatal case: apply the state-specific behavior shown here.
        Page::Fatal => wrapped(
            display,
            crate::i18n::tr(lang, "Device error. Please restart."),
            76,
            12,
        )?,
        // Handle remaining cases with the fallback, preserving safe behavior for unsupported or
        // irrelevant input.
        _ => wrapped(display, crate::i18n::tr(lang, &app.message), 76, 12)?,
    }
    // Visit each entry in builds controls for the current application page or settings overlay; the
    // loop binding provides its value or index for this iteration.
    for b in controls(app) {
        // Keep enabled panel colour or muted disabled-control colour blended with the page
        // background in this local variable for the following operations.
        let fill = if b.enabled {
            theme::PANEL
        } else {
            // Run the blend operation with the supplied inputs.
            theme::blend(theme::PANEL, theme::background_at(b.bounds.top_left.y), 9)
        };
        // Run the rounded operation with the supplied inputs.
        theme::rounded(display, b.bounds, fill)?;
        // Keep sanitized or clipped text prepared for the available drawing area in this local
        // variable for the following operations.
        let text = clipped(&b.label, b.bounds.size.width.saturating_sub(10));
        // Keep horizontal coordinate or current position along the row in this local variable for
        // the following operations.
        let x = b.bounds.top_left.x + (b.bounds.size.width as i32 - BODY.width(&text) as i32) / 2;
        // Send the prepared shape or text pixels to the current drawing destination.
        BODY.draw(
            display,
            &text,
            Point::new(
                x,
                b.bounds.top_left.y + (b.bounds.size.height as i32 - BODY.height as i32) / 2,
            ),
            // Enabled controls use the normal panel colour; disabled controls use subdued styling
            // and cannot be activated.
            if b.enabled { theme::TEXT } else { theme::MUTED },
            Backdrop::Solid(fill),
            b.bounds.size.width.saturating_sub(10),
        )?;
    }
    // Return success after the required side effects are complete.
    Ok(())
}

/// Checks whether the active page includes a presence indicator.
///
/// # Arguments
///
/// * `app` (`&App`) - Application model supplying the active page, preferences, weather,
///   and presence state.
///
/// # Returns
///
/// `bool` - True for the Settings overlay or unobscured Problem, Connecting, and Connected
/// pages; false otherwise.
pub fn presence_visible(app: &App) -> bool {
    // Return overlay equals an available value for crate settings Overlay Settings or
    // `app.preferences.overlay` is unavailable and the formatted text or fixture created by matches
    // as the value of this block.
    app.preferences.overlay == Some(crate::settings::Overlay::Settings)
        || (app.preferences.overlay.is_none()
            && matches!(app.page, Page::Problem | Page::Connecting | Page::Connected))
}

/// Returns the rectangle reserved for presence updates.
///
/// # Arguments
///
/// None.
///
/// # Returns
///
/// `Rectangle` - Pixel bounds covering the indicator and its background.
pub fn presence_bounds() -> Rectangle {
    // Construct the value from the supplied configuration or coordinates for use by the surrounding
    // operation.
    Rectangle::new(Point::new(280, 12), Size::new(28, 25))
}

/// Restores the indicator background and draws the current presence colour when visible.
///
/// # Arguments
///
/// * `display` (`&mut D`) - Mutable RGB565 draw target receiving the rendering operations.
/// * `app` (`&App`) - Application model supplying the active page, preferences, weather,
///   and presence state.
///
/// # Type Parameters
///
/// * `D` - RGB565 drawing destination; its associated error is propagated by fallible
///   renderers.
///
/// # Returns
///
/// `Result<(), D::Error>` - Ok after drawing or when hidden; Err from the draw target.
///
/// # Errors
///
/// Propagates background or indicator drawing errors.
pub fn render_presence<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    app: &App,
) -> Result<(), D::Error> {
    // This page does not display the presence indicator; avoid changing unrelated pixels.
    if !presence_visible(app) {
        // Leave this function now with a successful result carrying (); later statements are
        // skipped.
        return Ok(());
    }
    // Keep screen-coordinate rectangle limiting the drawing operation in this local variable for
    // the following operations.
    let bounds = presence_bounds();
    // Visit each entry in the value computed by the following expression; the loop binding provides
    // its value or index for this iteration.
    for y in bounds.top_left.y..bounds.top_left.y + bounds.size.height as i32 {
        // Fill the specified rectangle with one colour, propagating target errors when applicable.
        display.fill_solid(
            &Rectangle::new(
                Point::new(bounds.top_left.x, y),
                Size::new(bounds.size.width, 1),
            ),
            theme::background_at(y),
        )?;
    }
    // Keep RGB565 colour used for this drawing operation in this local variable for the following
    // operations.
    let color = match app.radar {
        // Handle the crate radar PresenceStatus Present case: Run the rgb operation with the
        // supplied inputs.
        crate::radar::PresenceStatus::Present => theme::rgb(0x2ecc71),
        // Handle the crate radar PresenceStatus Absent case: Run the rgb operation with the
        // supplied inputs.
        crate::radar::PresenceStatus::Absent => theme::rgb(0xe74c3c),
        // Handle the crate radar PresenceStatus Unavailable case: Run the rgb operation with the
        // supplied inputs.
        crate::radar::PresenceStatus::Unavailable => theme::rgb(0xb9b9b9),
    };
    // Send the prepared shape or text pixels to the current drawing destination.
    Circle::new(Point::new(288, 16), 16)
        .into_styled(PrimitiveStyle::with_fill(color))
        .draw(display)
}

/// Also used by host previews: styling never changes calibration target positions.
///
/// Draws a startup/calibration title, optional body, and optional target cross.
///
/// # Arguments
///
/// * `display` (`&mut D`) - Mutable RGB565 draw target receiving the rendering operations.
/// * `message` (`&str`) - Prompt text, optionally containing a newline between title and
///   body.
/// * `target` (`Option<(i32, i32)>`) - Optional calibration cross coordinates in screen
///   pixels; None draws no cross.
///
/// # Type Parameters
///
/// * `D` - RGB565 drawing destination; its associated error is propagated by fallible
///   renderers.
///
/// # Returns
///
/// `Result<(), D::Error>` - Ok after drawing; Err from the draw target. Target positions
/// are not changed by prompt styling.
///
/// # Errors
///
/// Propagates prompt text, background, and target drawing failures.
pub fn render_prompt<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    message: &str,
    target: Option<(i32, i32)>,
) -> Result<(), D::Error> {
    // Run the background operation with the supplied inputs.
    theme::background(display)?;
    // Keep prompt split once into title and optional multiline body in this local variable for the
    // following operations.
    let mut parts = message.splitn(2, '\n');
    // Keep measured title lines fitting the prompt's available width in this local variable for the
    // following operations.
    let titles = theme::wrap(parts.next().unwrap_or_default(), &TITLE, 288, 2);
    // Visit each entry in enumerate result for the surrounding operation; the loop binding provides
    // its value or index for this iteration.
    for (i, title) in titles.iter().enumerate() {
        // Send the prepared shape or text pixels to the current drawing destination.
        TITLE.draw(
            display,
            title,
            Point::new(16, 90 + i as i32 * TITLE.height as i32),
            theme::TEXT,
            Backdrop::Gradient,
            288,
        )?;
    }
    // A prompt body follows the title; wrap it below the title without changing calibration target
    // coordinates.
    if let Some(body) = parts.next() {
        // Draws body-font text across bounded lines on the gradient background.
        wrapped(
            display,
            body,
            100 + titles.len() as i32 * TITLE.height as i32,
            5,
        )?;
    }
    // Draw the yellow cross only when the caller supplies a calibration target.
    if let Some((x, y)) = target {
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for (a, b) in [((x - 10, y), (x + 10, y)), ((x, y - 10), (x, y + 10))] {
            // Send the prepared shape or text pixels to the current drawing destination.
            Line::new(Point::new(a.0, a.1), Point::new(b.0, b.1))
                .into_styled(PrimitiveStyle::with_stroke(Rgb565::YELLOW, 2))
                .draw(display)?;
        }
    }
    // Return success after the required side effects are complete.
    Ok(())
}

///////////////////////////////////////////////////////////////////////////////////////
//                                    UNIT TESTS                                     //
///////////////////////////////////////////////////////////////////////////////////////

/// Unit tests and supporting fixtures for ui behavior.
#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::mock_display::MockDisplay;
    /// Verifies that controls fit the portrait display and hit testing uses the same rectangles
    /// as rendering.
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
    fn controls_fit_portrait_and_hit_the_same_geometry() {
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App::default();
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for page in [
            Page::Problem,
            Page::Networks,
            Page::Password,
            Page::Connecting,
            Page::Connected,
        ] {
            // Set currently selected Wi-Fi navigation page to currently selected Wi-Fi navigation
            // page.
            app.page = page;
            // Append the supplied text to the existing string without replacing its earlier
            // contents.
            app.password.push_str("password");
            // Visit each entry in builds controls for the current application page or settings
            // overlay; the loop binding provides its value or index for this iteration.
            for b in controls(&app) {
                // Verify horizontal coordinate or current position along the row has reached or
                // exceeded 0 and vertical coordinate or current screen row has reached or exceeded
                // 0. A violation means the tested behavior is incorrect.
                assert!(b.bounds.top_left.x >= 0 && b.bounds.top_left.y >= 0);
                // Verify horizontal coordinate or current position along the row plus the numeric
                // value converted to the required arithmetic or indexing type does not exceed 320.
                // A violation means the tested behavior is incorrect.
                assert!(b.bounds.top_left.x + b.bounds.size.width as i32 <= 320);
                // Verify vertical coordinate or current screen row plus the numeric value converted
                // to the required arithmetic or indexing type does not exceed 480. A violation
                // means the tested behavior is incorrect.
                assert!(b.bounds.top_left.y + b.bounds.size.height as i32 <= 480);
                // Enabled controls use the normal panel colour; disabled controls use subdued
                // styling and cannot be activated.
                if b.enabled {
                    // Verify that finds the first enabled control containing a screen point exactly
                    // matches an available value for action.
                    assert_eq!(
                        hit(&app, b.bounds.top_left.x + 2, b.bounds.top_left.y + 2),
                        Some(b.action)
                    );
                }
            }
        }
    }
    /// Verifies weather-view cycling and retention of the selected view through Wi-Fi recovery.
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
    fn weather_tap_cycle_and_connection_recovery_preserve_the_view() {
        use crate::{settings::Overlay, weather::View};
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App {
            // Initialize currently selected Wi-Fi navigation page from the supplied value.
            page: Page::Connected,
            ..App::default()
        };
        // Set name of the successfully persisted station profile, if one exists to an available
        // value for into result for the surrounding operation.
        app.saved_ssid = Some("test".into());
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for view in [View::Week, View::Hours, View::Clock] {
            // Verify that finds the first enabled control containing a screen point exactly matches
            // an available value for Action CycleWeather.
            assert_eq!(hit(&app, 160, 200), Some(Action::CycleWeather));
            // Applies a Wi-Fi or shared navigation action to the application model.
            activate(&mut app, Action::CycleWeather);
            // Verify that view exactly matches view.
            assert_eq!(app.weather.view, view);
        }
        // Verify that finds the first enabled control containing a screen point exactly matches no
        // available value.
        assert_eq!(hit(&app, 20, 450), None);
        // Verify that finds the first enabled control containing a screen point exactly matches an
        // available value for Action OpenSettings.
        assert_eq!(hit(&app, 280, 450), Some(Action::OpenSettings));
        // Set view to View Hours.
        app.weather.view = View::Hours;
        // Set overlay to an available value for Overlay Settings.
        app.preferences.overlay = Some(Overlay::Settings);
        // Verify that finds the first enabled control containing a screen point differs from an
        // available value for Action CycleWeather.
        assert_ne!(hit(&app, 160, 200), Some(Action::CycleWeather));
        // Applies a Wi-Fi or shared navigation action to the application model.
        crate::settings_ui::activate(&mut app, Action::CloseSettings, 0);
        // Verify that view exactly matches View Hours.
        assert_eq!(app.weather.view, View::Hours);
        // Keep operation requested from the Wi-Fi worker in this local variable for the following
        // operations.
        let command = app.handle(crate::model::Event::Lost, 0);
        // Verify the formatted text or fixture created by matches. A violation means the tested
        // behavior is incorrect.
        assert!(matches!(command, Some(Command::Retry(_))));
        // Verify that currently selected Wi-Fi navigation page exactly matches Connecting state.
        assert_eq!(app.page, Page::Connecting);
        // Execute handle for application state used by navigation, preferences, weather, and
        // rendering.
        app.handle(
            crate::model::Event::Connected(app.generation, "test".into()),
            1,
        );
        // Verify that currently selected Wi-Fi navigation page exactly matches Connected state.
        assert_eq!(app.page, Page::Connected);
        // Verify that view exactly matches View Hours.
        assert_eq!(app.weather.view, View::Hours);
    }

    /// Verifies that all supported printable ASCII passphrase characters can be entered through
    /// keyboard controls.
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
    fn every_printable_password_character_is_reachable() {
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App {
            // Initialize currently selected Wi-Fi navigation page from the supplied value.
            page: Page::Password,
            ..App::default()
        };
        // Keep characters reachable through all password keyboard modes, including space in this
        // local variable for the following operations.
        let mut keys = vec![' '];
        // Visit each entry in 0..3; the loop binding provides its value or index for this
        // iteration.
        for mode in 0..3 {
            // Set selected on-screen keyboard layout to mode.
            app.keyboard = mode;
            // Visit each entry in builds controls for the current application page or settings
            // overlay; the loop binding provides its value or index for this iteration.
            for c in controls(&app) {
                // Check whether an available action matches the requested case.
                if let Action::Character(ch) = c.action {
                    // Append the new entry to the collection, preserving the order in which values
                    // arrive.
                    keys.push(ch);
                }
            }
        }
        // Visit each entry in 32u8..=126; the loop binding provides its value or index for this
        // iteration.
        for byte in 32u8..=126 {
            // Verify the value falls within the stated range or belongs to the supported set. A
            // violation means the tested behavior is incorrect.
            assert!(
                keys.contains(&(byte as char)),
                "Missing character {}",
                byte as char
            );
        }
    }
    /// Verifies successful rendering of every Wi-Fi page with representative application state.
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
    fn pages_render_without_failure() {
        // Keep drawing destination receiving RGB565 pixels in this local variable for the following
        // operations.
        let mut display = MockDisplay::new();
        // Allow the mock target to repaint pixels so repeated rendering can be tested.
        display.set_allow_overdraw(true);
        // Let the mock target ignore clipped off-screen pixels while testing real page geometry.
        display.set_allow_out_of_bounds_drawing(true);
        // Keep application state used by navigation, preferences, weather, and rendering in this
        // local variable for the following operations.
        let mut app = App::default();
        // Visit each entry in crate i18n Language ALL; the loop binding provides its value or index
        // for this iteration.
        for language in crate::i18n::Language::ALL {
            // Set selected language for interface text and location search to selected language for
            // interface text and location search.
            app.preferences.current.language = language;
            // Run each listed scenario or target in the declared order; the loop variables select
            // the data for that case.
            for page in [
                Page::Starting,
                Page::Problem,
                Page::Networks,
                Page::Password,
                Page::Connecting,
                Page::Connected,
                Page::Fatal,
            ] {
                // Set currently selected Wi-Fi navigation page to currently selected Wi-Fi
                // navigation page.
                app.page = page;
                // Require the value guaranteed by this test fixture or internal invariant;
                // unexpected absence panics.
                render(&mut display, &app).unwrap();
            }
        }
    }
}
