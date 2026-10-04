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

pub mod i18n;
pub mod location;
pub mod model;
pub mod radar;
pub mod screen;
pub mod service;
pub mod settings;
#[cfg(target_os = "espidf")]
pub mod settings_store;
pub mod settings_ui;
pub mod theme;
pub mod touch;
pub mod ui;
pub mod weather;
pub mod weather_icons;
pub mod weather_ui;

#[cfg(target_os = "espidf")]
pub mod board;
#[cfg(target_os = "espidf")]
pub mod network;
pub mod storage;
