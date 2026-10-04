// ============================================================================= //
// File          : build.rs                                                      //
// License       : GPL-3.0-only                                                  //
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

fn main() {
    println!("cargo:rustc-check-cfg=cfg(esp_idf_nvs_encryption)");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("espidf") {
        embuild::espidf::sysenv::output();
    }
}
