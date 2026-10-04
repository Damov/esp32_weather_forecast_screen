// ============================================================================= //
// File          : lib.rs                                                        //
// License       : GPL-3.0-only                                                  //
// Created       : 2026-10-04                                                    //
// Last modified : 2026-10-04                                                    //
// Author        : Daniel Kharlamov                                              //
// Last editor   : Daniel Kharlamov <daniel.kharlamov@googlemail.com>            //
// ============================================================================= //
//                                                                               //
// Summary:                                                                      //
// --------                                                                      //
// Shared firmware library and platform-specific module exports.                 //
//                                                                               //
// Module:                                                                       //
// -------                                                                       //
// Exposes application state, settings, radar parsing, touch handling,           //
// translations, weather processing, and UI rendering for both host and device   //
// use. Conditionally exports ESP-IDF board, networking, and settings storage    //
// modules for device builds.                                                    //
// ============================================================================= //

//! Shared weather-display firmware library.
//!
//! Hardware-independent state, parsing, and rendering are available to host tests;
//! ESP-IDF builds additionally expose device board, network, and storage adapters.

/// Language selection and translations for the device interface.
pub mod i18n;
/// Location search and the device HTTP request worker.
pub mod location;
/// Application state and Wi-Fi setup commands and events.
pub mod model;

/// Passive HLK-LD2410C radar parsing and presence detection.
pub mod radar;
/// Backlight blanking and wake control without stopping the CPU.
pub mod screen;
/// Hardware-independent Wi-Fi connection service.
pub mod service;
/// Persistent preferences and settings overlay state.
pub mod settings;
/// ESP-IDF NVS storage and legacy import for device settings.
#[cfg(target_os = "espidf")]
pub mod settings_store;
/// Settings overlay controls, rendering, and user actions.
pub mod settings_ui;
/// Shared UI colours, backgrounds, and bitmap font rendering.
pub mod theme;
/// Resistive touch filtering, gestures, and calibration.
pub mod touch;
/// Wi-Fi interface controls and shared application rendering.
pub mod ui;
/// Open-Meteo forecast processing and refresh scheduling.
pub mod weather;
/// Embedded Meteocons weather bitmap rendering.
pub mod weather_icons;
/// Clock, weekly, and hourly weather views for the portrait UI.
pub mod weather_ui;

/// ESP32 board initialization, display, touch, and radar access.
#[cfg(target_os = "espidf")]
pub mod board;
/// ESP-IDF Wi-Fi backend and connection worker.
#[cfg(target_os = "espidf")]
pub mod network;
/// Native ESP-IDF Wi-Fi credential persistence.
pub mod storage;
