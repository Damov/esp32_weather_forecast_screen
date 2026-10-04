// ============================================================================= //
// File          : build.rs                                                      //
// License       : MIT                                                           //
// Created       : 2026-10-04                                                    //
// Last modified : 2026-10-04                                                    //
// Author        : Daniel Kharlamov                                              //
// Last editor   : Daniel Kharlamov <daniel.kharlamov@googlemail.com>            //
// ============================================================================= //
//                                                                               //
// Summary:                                                                      //
// --------                                                                      //
// ESP-IDF build integration for the firmware crate.                             //
//                                                                               //
// Module:                                                                       //
// -------                                                                       //
// Declares the ESP-IDF NVS encryption configuration flag for Cargo checks. For  //
// ESP-IDF targets, forwards the SDK build environment through embuild; host     //
// builds skip SDK integration.                                                  //
// ============================================================================= //

//! Configures Cargo checks and integrates the ESP-IDF build environment for device targets.

/// Registers the NVS encryption configuration flag and exports ESP-IDF build settings on
/// device targets.
///
/// # Arguments
///
/// None.
///
/// # Returns
///
/// `()` - No value; prints Cargo build directives. Host builds skip SDK environment setup.
fn main() {
    // Emit Cargo build instructions consumed by the build system.
    println!("cargo:rustc-check-cfg=cfg(esp_idf_nvs_encryption)");
    // Check whether as deref result for the surrounding operation equals a successful result
    // carrying the specified message, format, or data literal.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("espidf") {
        // Run the output operation with the supplied inputs.
        embuild::espidf::sysenv::output();
    }
}
